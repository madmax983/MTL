//! P4c — Arena backend refinement against the FROZEN `spec_step` semantics.
//! ========================================================================
//!
//! A faithful **Verus model** (a "mirrored twin", in the P4/P5 `#[path] mod
//! mtl_core;` style) of the `crates/mtl-arena` machine, and — ultimately — the
//! one-step simulation theorem
//!
//!     alpha_state(model_arena_step(vm, pos)) == spec_step(alpha_state(vm, pos))
//!
//! This is NOT a verusification of the production `crates/mtl-arena` Rust; it is a
//! pure-spec twin of its data model (flat `Seq` arenas with integer back-references)
//! plus an abstraction function α into the ghost `SpecState`. `mtl_core.rs` is
//! pulled in UNMODIFIED and re-verified alongside, exactly like `p5_universality.rs`
//! and `checker_verus.rs`, so this proof is pinned to the same `spec_step` P2
//! discharged.
//!
//! ## MILESTONE STATUS: M1 — VERIFIED (model + wf + α, terminating)
//!
//! This file is the M0/M1 layer (design blueprint §1, §2, §5): the model
//! datatypes, the well-formedness invariants, and the abstraction functions α with
//! their termination measures, plus a single trivial smoke lemma. It passes Verus
//! cleanly: all four recursive α functions (`alpha_word`, `alpha_words`,
//! `alpha_stack`, `alpha_cont`) have their `decreases` clauses ACCEPTED
//! (termination proven via the wf-guarded lexicographic measures below), and the
//! smoke lemma verifies. There are NO `admit()`/`assume()`/`external_body` cheats.
//!
//! Verify with:
//!   verus crates/mtl-arena/proofs/arena_verus.rs
//! (re-verifies `mtl_core.rs` alongside; the arena layer adds the α/wf items.)
//!
//! Not cargo-compiled (verus-tool only), matching the p4/p5 contract — it cannot
//! perturb the production build.

use vstd::prelude::*;

#[path = "../../mtl-core/src/mtl_core.rs"]
mod mtl_core;
use mtl_core::*;

verus! {

// ============================================================
// 1. Model types — the spec-level representation of the arena.
//    (blueprint §1; production sources: mtl-arena/src/types.rs, arena.rs)
// ============================================================
//
// Integer modeling decision (blueprint §1.1): pointers and tape indices are
// `nat`; `i64` payloads are `int` (guarded by `mtl_core::in_i64` where the
// overflow obligation lives, in later milestones). Each arena is an UNBOUNDED
// `Seq`; the `u32` address ceiling is a SEPARATE capacity predicate (not modeled
// yet — M4/§4.4), so α is exact with no `as u32` wrap noise.
//
// Sentinels: index 0 of the stack arena is EMPTY_STACK; index 0 of the cont arena
// is NIL_CONT. We use the literal `0` inline (a spec `const` of type `nat` is
// avoided here only because the surrounding verus roots never use one — see
// m1-notes). `U32_MAX` (0xFFFF_FFFF) is deferred to the M4 capacity predicate.

/// Interned tape word — mirror of `types::Word`. `Call` carries the intern index
/// as a `nat` (production: `u32`). Reuses `mtl_core::SpecPrim` directly (identical
/// 23 variants / order, policed by the conformance crate) so `alpha_word` on a
/// `Prim` is the identity.
pub enum ModelWord {
    PushInt(int),
    PushQuote(ModelQuoteId),
    Prim(SpecPrim),
    Call(nat),
}

/// `{start, len}` slice into the tape — mirror of `types::QuoteId`.
/// `end() == start + len` exactly (the tape is unbounded + capacity-bounded, so
/// no `saturating_add` is needed as in the production `QuoteId::end`).
pub struct ModelQuoteId {
    pub start: nat,
    pub len: nat,
}

/// First-class value — mirror of `types::Value`.
pub enum ModelValue {
    Int(int),
    Quote(ModelQuoteId),
}

/// Stack-arena node — mirror of `arena::StackNode`. `parent` is a `StackPtr`.
pub struct ModelStackNode {
    pub value: ModelValue,
    pub parent: nat,
}

/// Continuation-arena node — mirror of `arena::ContNode`. "Run `tape[qstart..qend]`,
/// resuming at relative offset `off`."
pub struct ModelContNode {
    pub qstart: nat,
    pub qend: nat,
    pub off: nat,
    pub parent: nat,
}

/// The three arenas as flat `Seq`s. `snodes[0]` / `cnodes[0]` are the sentinels.
/// (`QuoteArena.tape`, `QuoteArena.calls`, `StackArena.nodes`, `ContArena.nodes`.)
pub struct ModelVm {
    pub tape: Seq<ModelWord>,
    pub calls: Seq<Seq<char>>,
    pub snodes: Seq<ModelStackNode>,
    pub cnodes: Seq<ModelContNode>,
}

/// The 12-byte machine position — mirror of `arena::VmState`
/// (`{stack, cont, cursor}`). Named `ModelVmState` per the M1 task; the blueprint
/// §1.2 calls this `ModelPos`.
pub struct ModelVmState {
    pub stack: nat,
    pub cont: nat,
    pub cursor: nat,
}

// ============================================================
// 1.3 Well-formedness invariants (blueprint §1.3).
//
// The acyclicity backbone that Verus gives P2 for free (a `Vec` field sits below
// its parent). Because the arenas are flat `Seq`s with integer back-references,
// we must STATE the "parent strictly below" / "quote target strictly earlier"
// facts; they are what make every α terminate and what the guarded recursions
// below encode.
// ============================================================

/// Stack arena: sentinel exists, and every node's parent is strictly below it.
pub open spec fn wf_stack(vm: ModelVm) -> bool {
    &&& vm.snodes.len() >= 1
    &&& forall|i: int| 1 <= i < vm.snodes.len() ==> #[trigger] vm.snodes[i].parent < i
}

/// Tape: every `PushQuote(id)` at index `i` references a region lying STRICTLY
/// BEFORE `i` (`id.end() <= i`), and every `Call(k)` has `k < calls.len()`.
/// Justified by `Vm::compile` / `try_alloc` interning nested quotes first
/// (vm.rs 129-141): a `PushQuote` is always appended AFTER its referent.
pub open spec fn wf_tape(vm: ModelVm) -> bool {
    forall|i: int| 0 <= i < vm.tape.len() ==> #[trigger] wf_tape_word(vm, i)
}

/// Per-word tape well-formedness (factored out so the `forall` has a clean
/// trigger). `PushInt`/`Prim` are unconstrained.
pub open spec fn wf_tape_word(vm: ModelVm, i: int) -> bool {
    match vm.tape[i] {
        ModelWord::PushQuote(id) => id.start + id.len <= i,
        ModelWord::Call(k) => k < vm.calls.len(),
        _ => true,
    }
}

/// Cont arena: sentinel exists; every node's parent is strictly below it, its
/// segment lies within the tape, and its frozen resume offset is within segment.
pub open spec fn wf_cont(vm: ModelVm) -> bool {
    &&& vm.cnodes.len() >= 1
    &&& forall|i: int| 1 <= i < vm.cnodes.len() ==> #[trigger] wf_cont_node(vm, i)
}

/// Per-node cont well-formedness.
pub open spec fn wf_cont_node(vm: ModelVm, i: int) -> bool {
    let nd = vm.cnodes[i];
    &&& nd.parent < i
    &&& nd.qstart <= nd.qend
    &&& nd.qend <= vm.tape.len()
    &&& nd.off <= nd.qend - nd.qstart
}

/// Whole-VM well-formedness (blueprint §1.3): the standing precondition of every
/// lemma.
pub open spec fn wf(vm: ModelVm) -> bool {
    &&& wf_stack(vm)
    &&& wf_tape(vm)
    &&& wf_cont(vm)
}

/// A position is well-formed against `vm` when its handles are in range and (for
/// the live head) its cursor is within the head segment (blueprint §1.3).
pub open spec fn wf_pos(vm: ModelVm, pos: ModelVmState) -> bool {
    &&& pos.stack < vm.snodes.len()
    &&& pos.cont < vm.cnodes.len()
    &&& (pos.cont != 0 ==> pos.cursor
        <= vm.cnodes[pos.cont as int].qend - vm.cnodes[pos.cont as int].qstart)
}

// ============================================================
// 2. The abstraction function α (blueprint §2).
//
// α maps a well-formed (ModelVm, ModelVmState) to a `SpecState`. It is the arena
// twin of `mtl_core::Vm::deep_view`. Every clause carries a `decreases` because
// the arena has no structural nesting for Verus to exploit.
//
// TERMINATION NOTE (important, flagged for M1-verify): spec functions cannot use
// `recommends` (or `wf`) to discharge a `decreases`. So each recursion that relies
// on a wf back-reference invariant (`parent < ptr`, `id.end() <= i`) is written
// with an explicit GUARD on that fact; the else-branch is UNREACHABLE under wf and
// exists only to make the measure provable WITHOUT an admit. See m1-notes.md.
// ============================================================

/// α on a value (blueprint §2.1). O(1) beyond the quote body.
pub open spec fn alpha_value(vm: ModelVm, v: ModelValue) -> SpecValue {
    match v {
        ModelValue::Int(n) => SpecValue::Int(n),
        ModelValue::Quote(id) => SpecValue::Quote(alpha_quote(vm, id)),
    }
}

/// α on ONE tape word at index `i` (blueprint §2.2). Measure component 2 (`1nat`)
/// marks "word", beating `alpha_words`' `0nat` at equal index.
///
/// The `PushQuote` guard `id.start + id.len <= i` is exactly `wf_tape`'s
/// `id.end() <= i`; it makes the cross-call measure `(id.end, 0, _) < (i, 1, 0)`
/// provable. The else-branch is unreachable under `wf_tape`.
pub open spec fn alpha_word(vm: ModelVm, i: nat) -> SpecWord
    recommends
        i < vm.tape.len(),
        wf_tape(vm),
    // Verus-accepted: 3-tuple lex measure (idx, kind, span). The PushQuote guard
    // below (id.end() <= i) makes the cross-call (id.end, 0, _) < (i, 1, 0).
    decreases i, 1nat, 0nat,
{
    match vm.tape[i as int] {
        ModelWord::PushInt(n) => SpecWord::PushInt(n),
        ModelWord::Prim(p) => SpecWord::Prim(p),
        ModelWord::Call(k) => SpecWord::Call(vm.calls[k as int]),
        ModelWord::PushQuote(id) => {
            if id.start + id.len <= i {
                SpecWord::PushQuote(alpha_words(vm, id.start, id.start + id.len))
            } else {
                // Unreachable under wf_tape (id.end() <= i). Guard discharges the
                // decreases; no admit.
                SpecWord::PushQuote(Seq::empty())
            }
        },
    }
}

/// α on the tape range `[lo, hi)`, HEAD-FIRST (so head-peel lemmas are cheap).
/// Blueprint §2.2. Measure `(hi, 0, hi - lo)`: component 1 (`0nat`) beats
/// `alpha_word`'s `1nat`; `hi - lo` is the intra-range peel tiebreaker.
pub open spec fn alpha_words(vm: ModelVm, lo: nat, hi: nat) -> Seq<SpecWord>
    recommends
        lo <= hi <= vm.tape.len(),
        wf_tape(vm),
    // Verus-accepted: 3-tuple lex measure, mutual with alpha_word. The `as nat`
    // cast in the third component is accepted in the decreases tuple.
    decreases hi, 0nat, (hi - lo) as nat,
{
    if lo >= hi {
        Seq::empty()
    } else {
        // alpha_word(lo): (lo,1,0) < (hi,0,_) since lo < hi (first component).
        // alpha_words(lo+1,hi): (hi,0,hi-lo-1) < (hi,0,hi-lo) (third component).
        seq![alpha_word(vm, lo)] + alpha_words(vm, lo + 1, hi)
    }
}

/// α on a whole quote by its `{start, len}` id (blueprint §2.2).
pub open spec fn alpha_quote(vm: ModelVm, id: ModelQuoteId) -> Seq<SpecWord> {
    alpha_words(vm, id.start, id.start + id.len)
}

/// α on the operand stack by pointer (blueprint §2.3). Mirrors `stack_values`,
/// appending the node value at the BACK (= top) AFTER recursing into the parent
/// (the `stack_values` `reverse()` is a reification artifact, NOT part of α — see
/// blueprint appendix fact 1). `wf_stack` (`parent < ptr`) makes this terminate.
pub open spec fn alpha_stack(vm: ModelVm, ptr: nat) -> Seq<SpecValue>
    recommends
        wf_stack(vm),
        ptr < vm.snodes.len(),
    decreases ptr,
{
    if ptr == 0 {
        Seq::empty()
    } else {
        let nd = vm.snodes[ptr as int];
        if nd.parent < ptr {
            alpha_stack(vm, nd.parent).push(alpha_value(vm, nd.value))
        } else {
            // Unreachable under wf_stack (parent < ptr). Guard discharges `decreases ptr`.
            Seq::empty()
        }
    }
}

/// α on the continuation — the hard one (blueprint §2.4). Mirrors `reify_cont`:
/// flatten the segment cons-list over the shared tape, threading `resume_off`
/// (the LIVE cursor for the head; each ancestor's FROZEN `off`). `wf_cont`
/// (`parent < ptr`) makes the parent-chain walk terminate; the inner `alpha_words`
/// terminates on its own measure.
pub open spec fn alpha_cont(vm: ModelVm, ptr: nat, resume_off: nat) -> Seq<SpecWord>
    recommends
        wf(vm),
        ptr < vm.cnodes.len(),
    decreases ptr,
{
    if ptr == 0 {
        Seq::empty()
    } else {
        let nd = vm.cnodes[ptr as int];
        let seg_len = nd.qend - nd.qstart;
        let seg = if (resume_off as int) < seg_len {
            // live/frozen resume point: emit tape[qstart + resume_off .. qend]
            alpha_words(vm, nd.qstart + resume_off, nd.qend)
        } else {
            // exhausted segment contributes nothing (reify_cont: `if off < seg_len`)
            Seq::empty()
        };
        if nd.parent < ptr {
            // Ancestor resumes at ITS frozen off (reify_cont: `off = parent.off`).
            // cnodes[0] is the NIL sentinel with off == 0, so this is uniform even
            // when nd.parent == NIL_CONT.
            let parent_off = vm.cnodes[nd.parent as int].off;
            seg + alpha_cont(vm, nd.parent, parent_off)
        } else {
            // Unreachable under wf_cont (parent < ptr). Guard discharges `decreases ptr`.
            seg
        }
    }
}

/// α on a whole machine state (blueprint §2.5). The arena twin of `Vm::deep_view`.
/// The head continuation node resumes at the LIVE `cursor`.
pub open spec fn alpha_state(vm: ModelVm, pos: ModelVmState) -> SpecState {
    SpecState {
        stack: alpha_stack(vm, pos.stack),
        cont: alpha_cont(vm, pos.cont, pos.cursor),
    }
}

// ============================================================
// 3. Smoke lemma (M1: at least one checkable proof).
// ============================================================

/// `alpha_value` of an `Int` round-trips to the ghost `SpecValue::Int`.
/// Trivial (definitional unfold); present so the skeleton has a real `proof fn`
/// with no admit/assume.
pub proof fn lemma_alpha_value_int_roundtrip(vm: ModelVm, n: int)
    ensures
        alpha_value(vm, ModelValue::Int(n)) == SpecValue::Int(n),
{
}

} // verus!

fn main() {}
