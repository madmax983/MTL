//! Reachable-state compaction: a stop-the-world **copying** pass that re-interns
//! only the *live* arena cells into fresh arenas and remaps every live handle,
//! so a long-lived / streaming host that rarely resets stays flat in memory
//! instead of ratcheting the tape + node arenas up to the `u32` ceiling
//! (issue #51). This is the cross-generation answer that truncate-only
//! generational reset ([`Vm::reset_to`]) cannot give.
//!
//! It is **opt-in and off by default** (see [`CompactPolicy::Off`]); the default
//! [`crate::run_arena`] / [`crate::host::arena_drive`] paths never call it and are
//! byte-for-byte unchanged. Correctness is established the way everything else in
//! the arena is — **differentially**, against the reference interpreter — not by a
//! Verus obligation (AC#9): the extended oracle forces a compaction at every
//! interior safe point and asserts the reified terminal is bit-identical to
//! `mtl_core::interp::run`.
//!
//! ## Liveness definition (AC#1)
//!
//! The **live roots** of a compaction are:
//!   1. the set of in-flight [`VmState`]s held by the driver frontier (each a
//!      `{stack: StackPtr, cont: ContPtr, cursor}` position), **and**
//!   2. the *static-code region below the generation floor* — the base-program
//!      [`Mark`] captured once after the program is compiled and prepended. Every
//!      cell below the floor (tape word, call name, stack node, cont node) is
//!      treated as immortal and preserved verbatim, at its original handle.
//!
//! A cell is **live** iff it is reachable from a live root:
//!   * a **cont node** is reachable via `parent` chains from a root's `cont`;
//!     each reachable cont node keeps its `tape[qstart..qend]` segment live;
//!   * a **stack node** is reachable via `parent` chains from a root's `stack`;
//!   * a **tape word** is reachable if it lies in a live cont segment or a live
//!     stack node's `Quote` value, **transitively** through every
//!     `Word::PushQuote(QuoteId)` it contains (quote bodies point at earlier,
//!     lower tape — the append-only tape has no forward references, so the
//!     closure is a finite DAG walk);
//!   * a **call name** is reachable if some live tape word is `Word::Call(idx)`.
//!
//! `cursor` (the root's) and each cont node's `off` are **relative** offsets into
//! a segment; copying a segment preserves its length, so these offsets are carried
//! through unchanged. Already-reified owned data (`reify_stack` / `reify_cont`
//! `Vec<interp::…>`) is **not** a root — the reification-before-reset invariant
//! ([`crate::arena`] module docs) is preserved verbatim: reification still happens
//! before any state leaves a generation, and compaction only reorganizes the
//! storage a *live* handle points at.
//!
//! ## The copying pass (AC#2)
//!
//! A fresh [`Vm`] is seeded with the immortal below-floor prefix of every arena
//! (cloned verbatim, so below-floor handles keep identity). Then, for each live
//! root, the stack cons-chain, the cont segment cons-chain, and the transitive
//! tape/call closure of everything they reference are **copied** above the floor
//! into the fresh arenas. Four remap tables (tape spans, call indices, stack
//! ptrs, cont ptrs) are threaded so that:
//!   * structural sharing between roots is preserved (a shared stack/cont node is
//!     copied once and both new roots point at the one copy);
//!   * every live handle in every returned [`VmState`] — and every cross-reference
//!     *inside* a copied cell (`PushQuote`, `Call`, cont `parent`, stack `parent`,
//!     cont `{qstart,qend}`) — is rewritten through the tables. No live
//!     `QuoteId` / `StackPtr` / `ContPtr` / call index dangles.
//!
//! Because a copy is keyed on `(start, len)`, identical spans dedupe; distinct but
//! overlapping spans are copied independently (structural sharing across *different*
//! slices is not reconstructed — correctness holds, some duplication is possible,
//! bounded by the live set). The observable result — the reified stack, the
//! reified continuation, any fault info — is invariant across a compaction (AC#6).

use std::collections::HashMap;

use crate::arena::{
    ContArena, ContNode, ContPtr, Mark, QuoteArena, StackArena, StackPtr, VmState, EMPTY_STACK,
    MAX_ADDR, NIL_CONT,
};
use crate::types::{QuoteId, Value, Word};
use crate::vm::Vm;

/// When compaction fires, checked at a generation-safe point (the top of a driver
/// loop, between atomic [`crate::arena_step`]s — never mid-step). The metric is the
/// number of **above-floor** cells (tape words + stack nodes + cont nodes) that
/// have accumulated since the generation floor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompactPolicy {
    /// Never compact. The default; the driver behaves byte-for-byte like the
    /// truncate-only backend (AC#4).
    #[default]
    Off,
    /// Compact at **every** safe point that has anything above the floor. This is
    /// the "threshold = 0 / forced at every safe point" mode the differential
    /// oracle uses to prove semantics are preserved across a compaction (AC#5).
    Always,
    /// Compact once the above-floor cell count exceeds this bound (AC#3). A larger
    /// bound trades a higher memory ceiling for fewer compaction pauses.
    Threshold(usize),
}

impl CompactPolicy {
    /// Whether a compaction should fire now, given the live arena and the floor.
    pub(crate) fn triggered(&self, vm: &Vm, floor: Mark) -> bool {
        match self {
            CompactPolicy::Off => false,
            CompactPolicy::Always => above_floor_cells(vm, floor) > 0,
            CompactPolicy::Threshold(n) => above_floor_cells(vm, floor) > *n,
        }
    }
}

/// Total cells allocated above the generation floor (the compaction-eligible set).
#[inline]
pub(crate) fn above_floor_cells(vm: &Vm, floor: Mark) -> usize {
    vm.quotes.tape.len().saturating_sub(floor.tape)
        + vm.stack.nodes.len().saturating_sub(floor.stack_nodes)
        + vm.cont.nodes.len().saturating_sub(floor.cont_nodes)
}

/// Before/after arena sizes for one compaction — the measurement surface for the
/// steady-state-bound demonstration (AC#7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactStats {
    pub tape_before: usize,
    pub tape_after: usize,
    pub stack_before: usize,
    pub stack_after: usize,
    pub cont_before: usize,
    pub cont_after: usize,
    pub calls_before: usize,
    pub calls_after: usize,
}

impl CompactStats {
    /// Total cells (tape + stack + cont) reclaimed by this compaction.
    pub fn cells_reclaimed(&self) -> usize {
        (self.tape_before + self.stack_before + self.cont_before)
            .saturating_sub(self.tape_after + self.stack_after + self.cont_after)
    }
}

/// Compact `old` with respect to `floor` and the live `roots`, returning a fresh
/// [`Vm`] holding only the immortal below-floor prefix plus the live above-floor
/// closure, the `roots` remapped into it (same order), and the size delta.
///
/// `floor` MUST be a real high-water [`Mark`] of `old` (`floor.* <= old.*`), and
/// every handle in `roots` must be valid in `old`. The caller applies the returned
/// [`VmState`]s to its frontier and drops the old [`Vm`].
pub fn compact(old: &Vm, floor: Mark, roots: &[VmState]) -> (Vm, Vec<VmState>, CompactStats) {
    debug_assert!(floor.tape <= old.quotes.tape.len(), "floor.tape in the future");
    debug_assert!(floor.calls <= old.quotes.calls.len(), "floor.calls in the future");
    debug_assert!(floor.stack_nodes <= old.stack.nodes.len(), "floor.stack_nodes in the future");
    debug_assert!(floor.cont_nodes <= old.cont.nodes.len(), "floor.cont_nodes in the future");

    // Seed the fresh Vm with the immortal below-floor prefix of every arena. Every
    // below-floor handle keeps its identity (a below-floor cell can only reference
    // other below-floor cells, since it was written before the floor was taken).
    let mut new = Vm {
        quotes: QuoteArena {
            tape: old.quotes.tape[..floor.tape].to_vec(),
            calls: old.quotes.calls[..floor.calls].to_vec(),
        },
        stack: StackArena { nodes: old.stack.nodes[..floor.stack_nodes.max(1)].to_vec() },
        cont: ContArena { nodes: old.cont.nodes[..floor.cont_nodes.max(1)].to_vec() },
    };

    let mut cx = Copier {
        old,
        floor,
        span_map: HashMap::new(),
        call_map: HashMap::new(),
        stack_map: HashMap::new(),
        cont_map: HashMap::new(),
    };

    let new_roots: Vec<VmState> = roots
        .iter()
        .map(|st| VmState {
            stack: cx.copy_stack(&mut new, st.stack),
            cont: cx.copy_cont(&mut new, st.cont),
            // `cursor` is a relative offset into the (length-preserving) copy of
            // the head cont segment — carried through unchanged.
            cursor: st.cursor,
        })
        .collect();

    let stats = CompactStats {
        tape_before: old.quotes.tape.len(),
        tape_after: new.quotes.tape.len(),
        stack_before: old.stack.nodes.len(),
        stack_after: new.stack.nodes.len(),
        cont_before: old.cont.nodes.len(),
        cont_after: new.cont.nodes.len(),
        calls_before: old.quotes.calls.len(),
        calls_after: new.quotes.calls.len(),
    };

    (new, new_roots, stats)
}

/// The mutable trace/copy state: the four old→new remap tables plus the source Vm
/// and the immortal floor.
struct Copier<'a> {
    old: &'a Vm,
    floor: Mark,
    /// `(old_start, len)` → new tape start, for above-floor spans (deduped).
    span_map: HashMap<(u32, u32), u32>,
    /// old call index → new call index, for above-floor call names.
    call_map: HashMap<u32, u32>,
    /// old [`StackPtr`] → new [`StackPtr`], for above-floor stack nodes.
    stack_map: HashMap<StackPtr, StackPtr>,
    /// old [`ContPtr`] → new [`ContPtr`], for above-floor cont nodes.
    cont_map: HashMap<ContPtr, ContPtr>,
}

impl<'a> Copier<'a> {
    /// Remap a call index. Below the floor → identity (immortal); otherwise copy
    /// the name once into the fresh table and remap.
    fn copy_call(&mut self, new: &mut Vm, idx: u32) -> u32 {
        if (idx as usize) < self.floor.calls {
            return idx;
        }
        if let Some(&ni) = self.call_map.get(&idx) {
            return ni;
        }
        let name = match self.old.quotes.calls.get(idx as usize) {
            Some(n) => n.clone(),
            None => {
                debug_assert!(false, "call index out of bounds in compaction");
                String::new()
            }
        };
        let ni = new.quotes.calls.len() as u32;
        debug_assert!(new.quotes.calls.len() <= MAX_ADDR, "compacted call table exceeded u32");
        new.quotes.calls.push(name);
        self.call_map.insert(idx, ni);
        ni
    }

    /// Copy a tape span `old_start .. old_start+len`, rewriting nested handles,
    /// and return the new start. Below-floor spans are immortal (identity, not
    /// re-copied — their nested targets are also below floor). Empty spans carry
    /// no tape, so their start is irrelevant and normalized to 0.
    fn copy_span(&mut self, new: &mut Vm, old_start: u32, len: u32) -> u32 {
        if len == 0 {
            return 0;
        }
        if (old_start as usize) < self.floor.tape {
            // Immortal: already present verbatim in the seeded below-floor prefix.
            return old_start;
        }
        if let Some(&ns) = self.span_map.get(&(old_start, len)) {
            return ns;
        }
        // Rewrite the words first (recursing into nested quotes, which append to
        // the new tape at lower addresses), then append this span contiguously.
        let mut ws: Vec<Word> = Vec::with_capacity(len as usize);
        for i in old_start..old_start.saturating_add(len) {
            let w = match self.old.quotes.tape.get(i as usize) {
                Some(w) => *w,
                None => {
                    debug_assert!(false, "tape index out of bounds in compaction");
                    Word::PushInt(0)
                }
            };
            let nw = match w {
                Word::PushInt(n) => Word::PushInt(n),
                Word::Prim(p) => Word::Prim(p),
                Word::Call(idx) => Word::Call(self.copy_call(new, idx)),
                Word::PushQuote(qid) => {
                    let ns = self.copy_span(new, qid.start, qid.len);
                    Word::PushQuote(QuoteId { start: ns, len: qid.len })
                }
            };
            ws.push(nw);
        }
        let ns = new.quotes.tape.len() as u32;
        debug_assert!(
            new.quotes.tape.len().saturating_add(ws.len()) <= MAX_ADDR,
            "compacted tape exceeded u32 (live set larger than the address space)"
        );
        new.quotes.tape.extend(ws);
        self.span_map.insert((old_start, len), ns);
        ns
    }

    /// Remap one stack value, copying any referenced quote span.
    fn copy_value(&mut self, new: &mut Vm, v: Value) -> Value {
        match v {
            Value::Int(n) => Value::Int(n),
            Value::Quote(qid) => {
                let ns = self.copy_span(new, qid.start, qid.len);
                Value::Quote(QuoteId { start: ns, len: qid.len })
            }
        }
    }

    /// Copy the stack cons-chain reachable from `p`, preserving structural sharing
    /// (across roots and repeated calls) via `stack_map`. Iterative, so stack
    /// depth does not bound recursion. Below-floor nodes are immortal identity.
    fn copy_stack(&mut self, new: &mut Vm, p: StackPtr) -> StackPtr {
        // Walk up to the first already-handled node (empty / below-floor / mapped),
        // collecting the uncopied above-floor prefix top-first.
        let mut chain: Vec<StackPtr> = Vec::new();
        let mut cur = p;
        let base = loop {
            if cur == EMPTY_STACK {
                break EMPTY_STACK;
            }
            if (cur as usize) < self.floor.stack_nodes {
                break cur; // immortal identity
            }
            if let Some(&nc) = self.stack_map.get(&cur) {
                break nc;
            }
            let node = match self.old.stack.nodes.get(cur as usize) {
                Some(n) => *n,
                None => {
                    debug_assert!(false, "stack ptr out of bounds in compaction");
                    break EMPTY_STACK;
                }
            };
            chain.push(cur);
            cur = node.parent;
        };
        // Rebuild bottom-up: the deepest uncopied node first, so each new node's
        // parent already exists.
        let mut newp = base;
        for &oldp in chain.iter().rev() {
            let node = self.old.stack.nodes[oldp as usize];
            let nv = self.copy_value(new, node.value);
            newp = new.stack.push(newp, nv);
            self.stack_map.insert(oldp, newp);
        }
        newp
    }

    /// Copy the cont segment cons-chain reachable from `p`, preserving structural
    /// sharing via `cont_map`. Iterative. Below-floor nodes are immortal identity;
    /// each above-floor node's `tape[qstart..qend]` segment is copied (remapping
    /// nested handles) and `off` carried through unchanged.
    fn copy_cont(&mut self, new: &mut Vm, p: ContPtr) -> ContPtr {
        let mut chain: Vec<ContPtr> = Vec::new();
        let mut cur = p;
        let base = loop {
            if cur == NIL_CONT {
                break NIL_CONT;
            }
            if (cur as usize) < self.floor.cont_nodes {
                break cur; // immortal identity
            }
            if let Some(&nc) = self.cont_map.get(&cur) {
                break nc;
            }
            let node = match self.old.cont.nodes.get(cur as usize) {
                Some(n) => *n,
                None => {
                    debug_assert!(false, "cont ptr out of bounds in compaction");
                    break NIL_CONT;
                }
            };
            chain.push(cur);
            cur = node.parent;
        };
        let mut newp = base;
        for &oldp in chain.iter().rev() {
            let node = self.old.cont.nodes[oldp as usize];
            let seg_len = node.qend.saturating_sub(node.qstart);
            debug_assert!(seg_len > 0, "empty cont segment should not exist");
            let new_qstart = self.copy_span(new, node.qstart, seg_len);
            let nn = ContNode {
                qstart: new_qstart,
                qend: new_qstart.saturating_add(seg_len),
                off: node.off,
                parent: newp,
            };
            newp = new.cont.push(nn);
            self.cont_map.insert(oldp, newp);
        }
        newp
    }
}
