//! The three bump arenas (quotes / stack / continuation), the 12-byte [`VmState`]
//! position, and the generational reset machinery.
//!
//! ## u32 addressing (design §3.4)
//!
//! Every handle is a `u32` (~4.29e9 entries per generation); `Int` payloads keep
//! full `i64`. Allocation **checks for u32 overflow and returns `None`** (the
//! caller turns that into a clean [`crate::Fault::Overflow`]) — never a silent
//! wraparound. The node-push helpers additionally `debug_assert!` the node-count
//! ceiling: reaching `u32::MAX` nodes requires >4e9 fuel-charged steps, so it is
//! not a practical caller-reachable path, and the design explicitly bounds a
//! generation to `u32` (§3.4). The tape (which can grow by large slices via
//! `cat`/`cons`/`linrec`) is the real overflow risk and is fully checked.
//!
//! ## Generational reset (design §3.2)
//!
//! A *generation* is one driven run / search episode. [`Mark`] records the
//! high-water length of each arena at the start; [`crate::Vm::reset_to`]
//! reclaims by **truncating each arena back to its mark** (bump/reset — zero
//! per-object free). Invariants:
//!   * static code allocated *below* the mark survives the reset;
//!   * **no `QuoteId`/`StackPtr`/`ContPtr` allocated within a generation may
//!     escape a reset except via reification** to owned `Vec<Value>`/`Vec<Word>`
//!     (a winner stack, a fault snapshot, a host crossing). Reset invalidates
//!     every handle at or above the mark; using a stale one is a use-after-reset
//!     bug. This is a *by-construction* discipline enforced at the call site, not
//!     a runtime check.

use crate::types::{Value, Word};

// ------------------------------------------------------------------ QuoteArena

/// Interns all quote bodies in one flat `tape`. Call names are interned to
/// `calls`. `try_alloc` extends the tape; list tails are O(1) sub-slices.
#[derive(Clone, Debug, Default)]
pub struct QuoteArena {
    /// The interned tape of every quote body. Public for read-only reification.
    pub tape: Vec<Word>,
    pub(crate) calls: Vec<String>,
}

/// The maximum number of tape words / stack nodes / cont nodes addressable by a
/// `u32` handle. Allocation past this faults (tape) or is a `debug_assert`
/// invariant (nodes).
pub(crate) const MAX_ADDR: usize = u32::MAX as usize;

impl QuoteArena {
    pub(crate) fn new() -> Self {
        Self { tape: Vec::new(), calls: Vec::new() }
    }

    /// True iff appending `extra` more words would keep every tape index within
    /// `u32`. `start` (`= tape.len()`) and `start + extra` must both be `<= u32::MAX`.
    #[inline]
    pub(crate) fn tape_fits(&self, extra: usize) -> bool {
        self.tape.len().checked_add(extra).is_some_and(|end| end <= MAX_ADDR)
    }

    /// Intern a call name to a `u32` index (deduplicated). Names are addressed by
    /// `u32`; a fresh name past `u32::MAX` is a `debug_assert` invariant (not a
    /// practical path).
    #[inline]
    pub(crate) fn intern_call(&mut self, name: &str) -> u32 {
        if let Some(i) = self.calls.iter().position(|c| c == name) {
            i as u32
        } else {
            let i = self.calls.len();
            debug_assert!(i <= MAX_ADDR, "call-name intern table exceeded u32");
            self.calls.push(name.to_string());
            i as u32
        }
    }

    /// Read a tape word by index, or `None` if out of bounds. All engine tape
    /// access goes through here so there is no panicking index in the hot loop.
    #[inline]
    pub(crate) fn word_at(&self, i: u32) -> Option<Word> {
        self.tape.get(i as usize).copied()
    }
}

// ------------------------------------------------------------------ StackArena

/// A pointer into the [`StackArena`]; `0` is the empty-stack sentinel.
pub type StackPtr = u32;
pub(crate) const EMPTY_STACK: StackPtr = 0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct StackNode {
    pub(crate) value: Value,
    pub(crate) parent: StackPtr,
}

/// Persistent, structurally shared operand stack. `nodes[0]` is the empty-stack
/// sentinel; a push allocates a node, a pop follows `parent`. Forking a stack is
/// just copying its [`StackPtr`].
#[derive(Clone, Debug)]
pub struct StackArena {
    pub(crate) nodes: Vec<StackNode>,
}

impl StackArena {
    pub(crate) fn new() -> Self {
        // index 0 = sentinel (value unused).
        Self { nodes: vec![StackNode { value: Value::Int(0), parent: 0 }] }
    }

    #[inline]
    pub(crate) fn node_at(&self, p: StackPtr) -> Option<StackNode> {
        self.nodes.get(p as usize).copied()
    }

    #[inline]
    pub(crate) fn push(&mut self, parent: StackPtr, value: Value) -> StackPtr {
        let idx = self.nodes.len();
        debug_assert!(idx <= MAX_ADDR, "stack node count exceeded u32 (generation limit)");
        self.nodes.push(StackNode { value, parent });
        idx as u32
    }
}

// ------------------------------------------------------------------- ContArena

/// A continuation segment: "run `tape[qstart..qend]`, resuming at relative offset
/// `off`" (`off` is relative to `qstart`; `0` = start of the segment).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContNode {
    pub(crate) qstart: u32,
    pub(crate) qend: u32,
    pub(crate) off: u32,
    pub(crate) parent: ContPtr,
}

/// A pointer into the [`ContArena`]; `0` = NIL (empty continuation → halt).
pub type ContPtr = u32;
pub(crate) const NIL_CONT: ContPtr = 0;

/// The continuation as a persistent segment cons-list. Prepending a quote freezes
/// the current head (capturing its resume offset) and pushes a child segment —
/// ≤2 node allocs, O(1), **no tail copy**. This is the fix for the measured
/// O(n²) front-pop pathology.
#[derive(Clone, Debug)]
pub struct ContArena {
    pub(crate) nodes: Vec<ContNode>,
}

impl ContArena {
    pub(crate) fn new() -> Self {
        Self { nodes: vec![ContNode { qstart: 0, qend: 0, off: 0, parent: 0 }] }
    }

    #[inline]
    pub(crate) fn node_at(&self, p: ContPtr) -> Option<ContNode> {
        self.nodes.get(p as usize).copied()
    }

    #[inline]
    pub(crate) fn push(&mut self, n: ContNode) -> ContPtr {
        let idx = self.nodes.len();
        debug_assert!(idx <= MAX_ADDR, "cont node count exceeded u32 (generation limit)");
        self.nodes.push(n);
        idx as u32
    }
}

// --------------------------------------------------------------------- VmState

/// The entire mutable machine *position*: three `u32`s = 12 bytes, `Copy`.
///
/// **Fork = copy this struct, O(1)** — independent of stack depth or continuation
/// size. This is the whole point of the arena design.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmState {
    pub stack: StackPtr,
    pub cont: ContPtr,
    pub cursor: u32,
}

impl VmState {
    /// The empty machine position: empty stack, NIL continuation, cursor 0.
    #[inline]
    pub fn initial() -> Self {
        VmState { stack: EMPTY_STACK, cont: NIL_CONT, cursor: 0 }
    }
}

// ------------------------------------------------------------ generational mark

/// A generation high-water mark: the length of each arena at the start of a
/// driven run. Feed it back to [`crate::Vm::reset_to`] to reclaim everything
/// allocated since. See the module docs for the reification-before-reset
/// invariant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    /// Tape length (interned quote words) at the mark.
    pub tape: usize,
    /// Call-name intern table length at the mark.
    pub calls: usize,
    /// Stack-node count at the mark.
    pub stack_nodes: usize,
    /// Continuation-node count at the mark.
    pub cont_nodes: usize,
}
