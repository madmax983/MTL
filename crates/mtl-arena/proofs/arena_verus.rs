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

// ============================================================
// 4. M2 — view-homomorphism lemmas about α (blueprint §3).
//
// Grouped: (0) tape-eq frames [tape/calls unchanged], (5) tape-extension frame,
// (1) stack push, (2) stack frame/pop, (3) cont next_word head-peel + segment-pop,
// (5) cont frame, (4) ★ splice-as-segment-push (`prepend`).
//
// STATUS: M2. All statements below close under Verus with NO
// admit/assume/external cheats. wf preconditions are carried per the M1 note
// ("guarded-body unfold tax"): every α unfold asserts its wf guard to reach the
// real branch.
// ============================================================

// ------------------------------------------------------------
// §3.0  Tape-equality frames — when tape and calls are UNCHANGED (only the
// snodes/cnodes Vecs grow), α of every tape word/range/value is invariant.
// No wf and no in-range bound needed: Seq equality is pointwise, so the match
// arms coincide and only the PushQuote recursion threads through. These are the
// helpers the stack- and cont-frame lemmas (whose ops never touch the tape) call.
// ------------------------------------------------------------

pub proof fn lemma_alpha_word_tape_eq(vm: ModelVm, vm2: ModelVm, i: nat)
    requires
        vm2.tape == vm.tape,
        vm2.calls == vm.calls,
    ensures
        alpha_word(vm2, i) == alpha_word(vm, i),
    decreases i, 1nat, 0nat,
{
    match vm.tape[i as int] {
        ModelWord::PushQuote(id) => {
            if id.start + id.len <= i {
                lemma_alpha_words_tape_eq(vm, vm2, id.start, id.start + id.len);
            }
        },
        _ => {},
    }
}

pub proof fn lemma_alpha_words_tape_eq(vm: ModelVm, vm2: ModelVm, lo: nat, hi: nat)
    requires
        vm2.tape == vm.tape,
        vm2.calls == vm.calls,
    ensures
        alpha_words(vm2, lo, hi) == alpha_words(vm, lo, hi),
    decreases hi, 0nat, (hi - lo) as nat,
{
    if lo >= hi {
    } else {
        lemma_alpha_word_tape_eq(vm, vm2, lo);
        lemma_alpha_words_tape_eq(vm, vm2, lo + 1, hi);
    }
}

pub proof fn lemma_alpha_value_tape_eq(vm: ModelVm, vm2: ModelVm, v: ModelValue)
    requires
        vm2.tape == vm.tape,
        vm2.calls == vm.calls,
    ensures
        alpha_value(vm2, v) == alpha_value(vm, v),
{
    match v {
        ModelValue::Quote(id) => {
            lemma_alpha_words_tape_eq(vm, vm2, id.start, id.start + id.len);
        },
        _ => {},
    }
}

// ------------------------------------------------------------
// §3.1  Tape head-peel and split (twins of lemma_view_words_append).
// ------------------------------------------------------------

/// Head-peel: one word off the front of a non-empty range. Definitional unfold.
pub proof fn lemma_alpha_words_head(vm: ModelVm, lo: nat, hi: nat)
    requires
        lo < hi,
    ensures
        alpha_words(vm, lo, hi) == seq![alpha_word(vm, lo)] + alpha_words(vm, lo + 1, hi),
{
}

/// α distributes over a contiguous range split. Twin of lemma_view_words_append.
pub proof fn lemma_alpha_words_split(vm: ModelVm, lo: nat, mid: nat, hi: nat)
    requires
        lo <= mid <= hi,
    ensures
        alpha_words(vm, lo, hi) == alpha_words(vm, lo, mid) + alpha_words(vm, mid, hi),
    decreases (mid - lo) as nat,
{
    if lo >= mid {
        assert(alpha_words(vm, lo, mid) =~= Seq::<SpecWord>::empty());
        assert(lo == mid);
    } else {
        lemma_alpha_words_head(vm, lo, hi);
        lemma_alpha_words_head(vm, lo, mid);
        lemma_alpha_words_split(vm, lo + 1, mid, hi);
        assert(alpha_words(vm, lo, hi)
            =~= alpha_words(vm, lo, mid) + alpha_words(vm, mid, hi));
    }
}

// ------------------------------------------------------------
// §3.5  Tape-extension frame — interning new words PAST an existing quote's span
// leaves α of that span unchanged (needed by the splice/intern prims, M3/M4).
// Requires wf_tape(vm): the invariant `PushQuote(id) at i ==> id.end() <= i`
// is exactly what makes an old word's α independent of the appended suffix.
// ------------------------------------------------------------

pub proof fn lemma_alpha_word_frame(vm: ModelVm, vm2: ModelVm, i: nat)
    requires
        wf_tape(vm),
        i < vm.tape.len(),
        vm.tape.len() <= vm2.tape.len(),
        forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j],
        forall|k: int| 0 <= k < vm.calls.len() ==> vm2.calls[k] == vm.calls[k],
    ensures
        alpha_word(vm2, i) == alpha_word(vm, i),
    decreases i, 1nat, 0nat,
{
    assert(wf_tape_word(vm, i as int));
    assert(vm2.tape[i as int] == vm.tape[i as int]);
    match vm.tape[i as int] {
        ModelWord::PushQuote(id) => {
            assert(id.start + id.len <= i);
            lemma_alpha_words_frame(vm, vm2, id.start, id.start + id.len);
        },
        ModelWord::Call(k) => {
            assert(k < vm.calls.len());
            assert(vm2.calls[k as int] == vm.calls[k as int]);
        },
        _ => {},
    }
}

pub proof fn lemma_alpha_words_frame(vm: ModelVm, vm2: ModelVm, lo: nat, hi: nat)
    requires
        wf_tape(vm),
        hi <= vm.tape.len(),
        vm.tape.len() <= vm2.tape.len(),
        forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j],
        forall|k: int| 0 <= k < vm.calls.len() ==> vm2.calls[k] == vm.calls[k],
    ensures
        alpha_words(vm2, lo, hi) == alpha_words(vm, lo, hi),
    decreases hi, 0nat, (hi - lo) as nat,
{
    if lo >= hi {
    } else {
        lemma_alpha_word_frame(vm, vm2, lo);
        lemma_alpha_words_frame(vm, vm2, lo + 1, hi);
    }
}

// ------------------------------------------------------------
// §3.2  Stack frame + push + pop (twins of lemma_view_stack_push / prefix).
// ------------------------------------------------------------

/// Appending stack nodes leaves α of any pre-existing stack ptr unchanged.
pub proof fn lemma_alpha_stack_frame(vm: ModelVm, vm2: ModelVm, ptr: nat)
    requires
        wf_stack(vm),
        ptr < vm.snodes.len(),
        vm.snodes.len() <= vm2.snodes.len(),
        forall|j: int| 0 <= j < vm.snodes.len() ==> vm2.snodes[j] == vm.snodes[j],
        vm2.tape == vm.tape,
        vm2.calls == vm.calls,
    ensures
        alpha_stack(vm2, ptr) == alpha_stack(vm, ptr),
    decreases ptr,
{
    if ptr == 0 {
    } else {
        assert(vm2.snodes[ptr as int] == vm.snodes[ptr as int]);
        assert(vm.snodes[ptr as int].parent < ptr);
        let nd = vm.snodes[ptr as int];
        lemma_alpha_stack_frame(vm, vm2, nd.parent);
        lemma_alpha_value_tape_eq(vm, vm2, nd.value);
    }
}

/// Pushing `{value, parent=ptr}` maps to a ghost push at the BACK (= top), and
/// frames every pre-existing ptr. Twin of lemma_view_stack_push.
pub proof fn lemma_alpha_stack_push(vm: ModelVm, ptr: nat, v: ModelValue)
    requires
        wf_stack(vm),
        ptr < vm.snodes.len(),
    ensures
        ({
            let vm2 = ModelVm {
                snodes: vm.snodes.push(ModelStackNode { value: v, parent: ptr }),
                ..vm
            };
            &&& wf_stack(vm2)
            &&& alpha_stack(vm2, vm.snodes.len())
                == alpha_stack(vm, ptr).push(alpha_value(vm, v))
            &&& forall|q: nat| q < vm.snodes.len() ==> alpha_stack(vm2, q) == alpha_stack(vm, q)
        }),
{
    let node = ModelStackNode { value: v, parent: ptr };
    let vm2 = ModelVm { snodes: vm.snodes.push(node), ..vm };
    let l = vm.snodes.len();
    assert(vm2.snodes.len() == l + 1);
    assert(vm2.snodes[l as int] == node);
    // wf_stack(vm2): sentinel + each parent strictly below (new node's parent==ptr<l).
    assert(wf_stack(vm2)) by {
        assert forall|i: int| 1 <= i < vm2.snodes.len() implies
            #[trigger] vm2.snodes[i].parent < i by {
            if i < l {
                assert(vm2.snodes[i] == vm.snodes[i]);
                assert(vm.snodes[i].parent < i);
            } else {
                assert(i == l);
                assert(vm2.snodes[i] == node);
                assert(node.parent == ptr);
            }
        }
    }
    // frame every pre-existing ptr.
    assert forall|q: nat| q < vm.snodes.len() implies
        alpha_stack(vm2, q) == alpha_stack(vm, q) by {
        lemma_alpha_stack_frame(vm, vm2, q);
    }
    // the new-top unfold.
    lemma_alpha_stack_frame(vm, vm2, ptr);
    lemma_alpha_value_tape_eq(vm, vm2, v);
    assert(alpha_stack(vm2, l)
        == alpha_stack(vm2, ptr).push(alpha_value(vm2, v)));
}

/// Following `parent` once (one pop) drops the top ghost element and exposes it.
/// Twin of lemma_view_stack_prefix at k = len-1, plus the popped-value fact.
pub proof fn lemma_alpha_stack_pop1(vm: ModelVm, ptr: nat)
    requires
        wf_stack(vm),
        ptr < vm.snodes.len(),
        ptr != 0,
    ensures
        alpha_stack(vm, ptr).len() == alpha_stack(vm, vm.snodes[ptr as int].parent).len() + 1,
        alpha_stack(vm, vm.snodes[ptr as int].parent)
            == alpha_stack(vm, ptr).subrange(0, alpha_stack(vm, ptr).len() - 1),
        alpha_stack(vm, ptr).last() == alpha_value(vm, vm.snodes[ptr as int].value),
{
    assert(vm.snodes[ptr as int].parent < ptr);
    let nd = vm.snodes[ptr as int];
    let base = alpha_stack(vm, nd.parent);
    // alpha_stack(vm, ptr) == base.push(alpha_value(vm, nd.value)) by the guarded unfold.
    assert(alpha_stack(vm, ptr) == base.push(alpha_value(vm, nd.value)));
    assert(base.push(alpha_value(vm, nd.value)).subrange(0, base.len() as int) =~= base);
}

// ------------------------------------------------------------
// §3.5  Cont frame — appending cont nodes leaves α of a pre-existing cont ptr
// unchanged (the tape is untouched, so segment α is stable via tape-eq).
// ------------------------------------------------------------

pub proof fn lemma_alpha_cont_frame(vm: ModelVm, vm2: ModelVm, ptr: nat, off: nat)
    requires
        wf_cont(vm),
        ptr < vm.cnodes.len(),
        vm.cnodes.len() <= vm2.cnodes.len(),
        forall|j: int| 0 <= j < vm.cnodes.len() ==> vm2.cnodes[j] == vm.cnodes[j],
        vm2.tape == vm.tape,
        vm2.calls == vm.calls,
    ensures
        alpha_cont(vm2, ptr, off) == alpha_cont(vm, ptr, off),
    decreases ptr,
{
    if ptr == 0 {
    } else {
        assert(wf_cont_node(vm, ptr as int));
        assert(vm2.cnodes[ptr as int] == vm.cnodes[ptr as int]);
        let nd = vm.cnodes[ptr as int];
        assert(nd.parent < ptr);
        // parent index is below ptr < len, so it agrees across the frame.
        assert(vm2.cnodes[nd.parent as int] == vm.cnodes[nd.parent as int]);
        let parent_off = vm.cnodes[nd.parent as int].off;
        lemma_alpha_cont_frame(vm, vm2, nd.parent, parent_off);
        // segment α is tape-stable.
        let seg_len = nd.qend - nd.qstart;
        if (off as int) < seg_len {
            lemma_alpha_words_tape_eq(vm, vm2, nd.qstart + off, nd.qend);
        }
    }
}

// ------------------------------------------------------------
// §3.4  Cont head-peel (next_word): consuming one in-segment word peels α's head.
// ------------------------------------------------------------

pub proof fn lemma_alpha_cont_next_word(vm: ModelVm, pos: ModelVmState)
    requires
        wf(vm),
        wf_pos(vm, pos),
        pos.cont != 0,
        (pos.cursor as int)
            < vm.cnodes[pos.cont as int].qend - vm.cnodes[pos.cont as int].qstart,
    ensures
        alpha_cont(vm, pos.cont, pos.cursor)
            == seq![alpha_word(vm, vm.cnodes[pos.cont as int].qstart + pos.cursor)]
               + alpha_cont(vm, pos.cont, pos.cursor + 1),
{
    let c = pos.cont;
    assert(wf_cont_node(vm, c as int));
    let h = vm.cnodes[c as int];
    let seg_len = h.qend - h.qstart;
    let head_idx = h.qstart + pos.cursor;
    let poff = vm.cnodes[h.parent as int].off;
    let tail_seg = alpha_words(vm, head_idx + 1, h.qend);
    let rest_cont = alpha_cont(vm, h.parent, poff);

    // Unfold the pre-state head at the live cursor.
    assert(head_idx < h.qend);
    lemma_alpha_words_head(vm, head_idx, h.qend);
    assert(alpha_cont(vm, c, pos.cursor)
        == (seq![alpha_word(vm, head_idx)] + tail_seg) + rest_cont);

    // Unfold the post-consume head at cursor + 1: its segment equals `tail_seg`.
    if ((pos.cursor + 1) as int) < seg_len {
        assert(alpha_cont(vm, c, pos.cursor + 1) == tail_seg + rest_cont);
    } else {
        assert((pos.cursor + 1) as int == seg_len);
        assert(head_idx + 1 == h.qend);
        assert(tail_seg =~= Seq::<SpecWord>::empty());
        assert(alpha_cont(vm, c, pos.cursor + 1) == rest_cont);
    }
    assert(alpha_cont(vm, c, pos.cursor)
        =~= seq![alpha_word(vm, head_idx)] + alpha_cont(vm, c, pos.cursor + 1));
}

/// Segment-pop: an exhausted head (cursor at segment end) contributes nothing, so
/// α equals that of the parent resumed at its frozen offset (models `next_word`'s
/// pop-to-parent).
pub proof fn lemma_alpha_cont_segpop(vm: ModelVm, pos: ModelVmState)
    requires
        wf(vm),
        wf_pos(vm, pos),
        pos.cont != 0,
        (pos.cursor as int)
            >= vm.cnodes[pos.cont as int].qend - vm.cnodes[pos.cont as int].qstart,
    ensures
        alpha_cont(vm, pos.cont, pos.cursor)
            == alpha_cont(vm, vm.cnodes[pos.cont as int].parent,
                          vm.cnodes[vm.cnodes[pos.cont as int].parent as int].off),
{
    let c = pos.cont;
    assert(wf_cont_node(vm, c as int));
    let h = vm.cnodes[c as int];
    assert(h.parent < c);
    // cursor >= seg_len => the head segment is empty; α falls through to the parent.
    assert(alpha_cont(vm, c, pos.cursor)
        =~= alpha_cont(vm, h.parent, vm.cnodes[h.parent as int].off));
}

// ------------------------------------------------------------
// §3.3  ★ Splice-as-segment-push: `prepend` a quote onto the continuation.
// The crux lemma for every splice primitive (Apply/Dip/If/PrimRec/Times/LinRec/
// Fold). Mirrors Vm::prepend (vm.rs 190-211): freeze the old head (off := cursor),
// push a child segment for the quote, retarget cont/cursor.
// ------------------------------------------------------------

/// The spec twin of `Vm::prepend` (vm.rs 190-211).
pub open spec fn model_prepend(vm: ModelVm, pos: ModelVmState, id: ModelQuoteId)
    -> (ModelVm, ModelVmState) {
    if id.len == 0 {
        (vm, pos)
    } else if pos.cont == 0 {
        let child = ModelContNode { qstart: id.start, qend: id.start + id.len, off: 0, parent: 0 };
        let vm2 = ModelVm { cnodes: vm.cnodes.push(child), ..vm };
        let pos2 = ModelVmState { cont: vm.cnodes.len(), cursor: 0, ..pos };
        (vm2, pos2)
    } else {
        let h = vm.cnodes[pos.cont as int];
        let frozen = ModelContNode { qstart: h.qstart, qend: h.qend, off: pos.cursor, parent: h.parent };
        let child = ModelContNode { qstart: id.start, qend: id.start + id.len, off: 0, parent: vm.cnodes.len() };
        let vm2 = ModelVm { cnodes: vm.cnodes.push(frozen).push(child), ..vm };
        let pos2 = ModelVmState { cont: vm.cnodes.len() + 1, cursor: 0, ..pos };
        (vm2, pos2)
    }
}

pub proof fn lemma_alpha_cont_prepend(vm: ModelVm, pos: ModelVmState, id: ModelQuoteId)
    requires
        wf(vm),
        wf_pos(vm, pos),
        id.start + id.len <= vm.tape.len(),
    ensures
        ({
            let (vm2, pos2) = model_prepend(vm, pos, id);
            &&& wf(vm2)
            &&& wf_pos(vm2, pos2)
            &&& alpha_cont(vm2, pos2.cont, pos2.cursor)
                == alpha_quote(vm, id) + alpha_cont(vm, pos.cont, pos.cursor)
            &&& alpha_stack(vm2, pos2.stack) == alpha_stack(vm, pos.stack)
        }),
{
    let (vm2, pos2) = model_prepend(vm, pos, id);

    if id.len == 0 {
        // No-op. alpha_quote(vm, id) is empty (span [start, start)).
        assert(alpha_quote(vm, id) =~= Seq::<SpecWord>::empty());
        assert(alpha_cont(vm2, pos2.cont, pos2.cursor)
            =~= alpha_quote(vm, id) + alpha_cont(vm, pos.cont, pos.cursor));
        return;
    }

    // Non-empty quote: the tape/calls/snodes are untouched (only cnodes grows),
    // so wf_stack, wf_tape, and the stack-side goal are stable.
    let cn = vm.cnodes;
    let l = cn.len();
    assert(vm2.tape == vm.tape);
    assert(vm2.calls == vm.calls);
    assert(vm2.snodes == vm.snodes);
    assert(wf_stack(vm2));
    assert(wf_tape(vm2)) by {
        assert forall|i: int| 0 <= i < vm2.tape.len() implies
            #[trigger] wf_tape_word(vm2, i) by {
            assert(wf_tape_word(vm, i));
        }
    }
    lemma_alpha_stack_frame(vm, vm2, pos.stack);

    // alpha_quote(vm, id) via tape-eq (used for the child's live segment).
    if pos.cont == 0 {
        let child = ModelContNode { qstart: id.start, qend: id.start + id.len, off: 0, parent: 0 };
        assert(vm2.cnodes == cn.push(child));
        assert(vm2.cnodes.len() == l + 1);
        assert(vm2.cnodes[l as int] == child);
        assert(pos2.cont == l);
        assert(l >= 1);
        // wf_cont(vm2).
        assert(wf_cont(vm2)) by {
            assert forall|i: int| 1 <= i < vm2.cnodes.len() implies
                #[trigger] wf_cont_node(vm2, i) by {
                if i < l {
                    assert(vm2.cnodes[i] == cn[i]);
                    assert(wf_cont_node(vm, i));
                } else {
                    assert(i == l);
                    assert(vm2.cnodes[i] == child);
                }
            }
        }
        // alpha_cont(vm2, l, 0): child segment then NIL parent.
        assert(alpha_quote(vm, id) == alpha_words(vm, id.start, id.start + id.len));
        lemma_alpha_words_tape_eq(vm, vm2, id.start, id.start + id.len);
        assert(alpha_cont(vm2, 0, vm2.cnodes[0].off) =~= Seq::<SpecWord>::empty());
        assert(alpha_cont(vm2, l, 0)
            =~= alpha_words(vm2, id.start, id.start + id.len));
        assert(alpha_cont(vm, pos.cont, pos.cursor) =~= Seq::<SpecWord>::empty());
        assert(alpha_cont(vm2, pos2.cont, pos2.cursor)
            =~= alpha_quote(vm, id) + alpha_cont(vm, pos.cont, pos.cursor));
        // wf_pos(vm2, pos2).
        assert(wf_pos(vm2, pos2));
        return;
    }

    // ---- The crux: non-NIL head. ----
    assert(wf_cont_node(vm, pos.cont as int));
    let c = pos.cont;
    let h = cn[c as int];
    let frozen = ModelContNode { qstart: h.qstart, qend: h.qend, off: pos.cursor, parent: h.parent };
    let child = ModelContNode { qstart: id.start, qend: id.start + id.len, off: 0, parent: l };
    let cn1 = cn.push(frozen);
    assert(vm2.cnodes == cn1.push(child));
    assert(vm2.cnodes.len() == l + 2);
    assert(cn1.len() == l + 1);
    assert(cn1[l as int] == frozen);
    assert(vm2.cnodes[l as int] == frozen);
    assert(vm2.cnodes[(l + 1) as int] == child);
    assert(forall|j: int| 0 <= j < l ==> vm2.cnodes[j] == cn[j]);
    assert(pos2.cont == l + 1);

    // h.parent < c < l  (wf_cont on c ; wf_pos gives c < len == l).
    assert(h.parent < c);
    assert(c < l);
    assert(h.parent < l);
    let poff = cn[h.parent as int].off;
    assert(vm2.cnodes[h.parent as int] == cn[h.parent as int]);

    // wf_cont(vm2): old nodes (< l) unchanged; frozen (l); child (l+1).
    assert(wf_cont(vm2)) by {
        assert forall|i: int| 1 <= i < vm2.cnodes.len() implies
            #[trigger] wf_cont_node(vm2, i) by {
            if i < l {
                assert(vm2.cnodes[i] == cn[i]);
                assert(wf_cont_node(vm, i));
            } else if i == l {
                assert(vm2.cnodes[i] == frozen);
                assert(h.parent < c);   // frozen.parent == h.parent < c < l == i
                assert(c < l);
                // frozen segment/off bounds inherited from h (wf_cont_node(vm, c)) + wf_pos.
                assert(h.qstart <= h.qend);
                assert(h.qend <= vm.tape.len());
                assert((pos.cursor as int) <= h.qend - h.qstart);
            } else {
                assert(i == l + 1);
                assert(vm2.cnodes[i] == child);   // parent == l < l+1 == i
            }
        }
    }

    // wf_pos(vm2, pos2): pos2.cont == l+1 < l+2; cursor 0 <= child seg length.
    assert(wf_pos(vm2, pos2));

    // ---- α equalities. ----
    // (i) frame the untouched sub-chain below the old head.
    lemma_alpha_cont_frame(vm, vm2, h.parent, poff);

    // (ii) the frozen node at the live cursor reproduces the old head's α.
    //      Both unfold to  seg(cursor) + alpha_cont(_, h.parent, poff), with equal
    //      segment (tape-eq) and equal tail (frame above).
    let seg_len = h.qend - h.qstart;
    if (pos.cursor as int) < seg_len {
        lemma_alpha_words_tape_eq(vm, vm2, h.qstart + pos.cursor, h.qend);
    }
    assert(alpha_cont(vm2, l, pos.cursor) =~= alpha_cont(vm, c, pos.cursor));

    // (iii) the child node at offset 0 emits the whole quote, then the frozen node.
    assert(alpha_quote(vm, id) == alpha_words(vm, id.start, id.start + id.len));
    lemma_alpha_words_tape_eq(vm, vm2, id.start, id.start + id.len);
    assert(vm2.cnodes[l as int].off == pos.cursor);
    assert(alpha_cont(vm2, l + 1, 0)
        =~= alpha_words(vm2, id.start, id.start + id.len) + alpha_cont(vm2, l, pos.cursor));

    // Assemble.
    assert(alpha_cont(vm2, pos2.cont, pos2.cursor)
        =~= alpha_quote(vm, id) + alpha_cont(vm, pos.cont, pos.cursor));
}

} // verus!

fn main() {}
