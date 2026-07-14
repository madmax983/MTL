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

/// A stack node value is "tape-valid" when a `Quote` payload references a region
/// inside the tape (`id.end() <= tape.len()`). `Int` payloads are unconstrained.
/// This is what makes an old stack pointer's α independent of a tape EXTENSION
/// (the Cons/Cat intern) — the arena analogue of "every reachable quote points at
/// interned tape". Kept OUT of `wf` (so M1/M2/M3a are untouched); threaded as an
/// explicit precondition where a tape-growing prim needs to frame the old stack.
pub open spec fn snode_val_wf(vm: ModelVm, i: int) -> bool {
    match vm.snodes[i].value {
        ModelValue::Quote(id) => id.start + id.len <= vm.tape.len(),
        _ => true,
    }
}

/// Every (non-sentinel) stack node holds a tape-valid value. Preserved by every
/// append-only op (pushes only tape-valid quotes; the tape only grows).
pub open spec fn wf_svals(vm: ModelVm) -> bool {
    forall|i: int| 1 <= i < vm.snodes.len() ==> #[trigger] snode_val_wf(vm, i)
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

// ============================================================
// 4b. M3b infrastructure — tape-extension frames, region-copy, and the
//     Cat/Cons intern models (blueprint §3.5/§3.6), plus pop helpers.
//
//   * length of an α'd range,
//   * α of one copied word / a copied contiguous region (extend_from_within),
//   * α of an OLD stack/cont pointer is stable under a tape EXTENSION,
//   * model_try_cat / model_try_cons (append-only intern) + their α laws,
//   * value_to_word bridge, and pop1/pop2 operand-extraction helpers.
// NO admit/assume/external cheats.
// ============================================================

/// |α(tape[lo..hi])| == hi - lo (each word contributes exactly one SpecWord).
pub proof fn lemma_alpha_words_len(vm: ModelVm, lo: nat, hi: nat)
    requires
        lo <= hi,
    ensures
        alpha_words(vm, lo, hi).len() == hi - lo,
    decreases (hi - lo) as nat,
{
    if lo >= hi {
    } else {
        lemma_alpha_words_len(vm, lo + 1, hi);
    }
}

/// α of one word copied verbatim to a position PAST the old tape equals α of the
/// source word. The copied `PushQuote(id)` still points at the old region (id.end
/// <= src < old_len <= dst), so its guard fires and the sub-quote α is tape-stable.
pub proof fn lemma_alpha_word_copy(vm: ModelVm, vm2: ModelVm, s: nat, d: nat)
    requires
        wf_tape(vm),
        s < vm.tape.len(),
        vm.tape.len() <= d,
        d < vm2.tape.len(),
        vm2.tape[d as int] == vm.tape[s as int],
        vm2.calls == vm.calls,
        forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j],
    ensures
        alpha_word(vm2, d) == alpha_word(vm, s),
{
    assert(wf_tape_word(vm, s as int));
    assert(vm.tape.len() <= vm2.tape.len());
    match vm.tape[s as int] {
        ModelWord::PushQuote(id) => {
            assert(id.start + id.len <= s);
            assert(id.start + id.len <= d);
            lemma_alpha_words_frame(vm, vm2, id.start, id.start + id.len);
        },
        ModelWord::Call(k) => {
            assert(k < vm.calls.len());
            assert(vm2.calls[k as int] == vm.calls[k as int]);
        },
        _ => {},
    }
}

/// α of a contiguous region copied verbatim (via extend_from_within) to a position
/// PAST the old tape equals α of the source region. Twin of the frame lemma, but
/// across a translation. Consumes `lemma_alpha_word_copy` + head-peel.
pub proof fn lemma_alpha_words_copy(vm: ModelVm, vm2: ModelVm, slo: nat, shi: nat, dlo: nat)
    requires
        wf_tape(vm),
        slo <= shi,
        shi <= vm.tape.len(),
        vm.tape.len() <= dlo,
        vm2.calls == vm.calls,
        forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j],
        (dlo + (shi - slo)) as int <= vm2.tape.len(),
        forall|k: int| 0 <= k < shi - slo ==> #[trigger] vm2.tape[dlo + k] == vm.tape[slo + k],
    ensures
        alpha_words(vm2, dlo, (dlo + (shi - slo)) as nat) == alpha_words(vm, slo, shi),
    decreases (shi - slo) as nat,
{
    let dhi = (dlo + (shi - slo)) as nat;
    if slo >= shi {
        assert(alpha_words(vm2, dlo, dhi) =~= Seq::<SpecWord>::empty());
        assert(alpha_words(vm, slo, shi) =~= Seq::<SpecWord>::empty());
    } else {
        // head word
        assert(vm2.tape[dlo as int] == vm.tape[slo as int]) by {
            assert(vm2.tape[dlo + 0int] == vm.tape[slo + 0int]);
        }
        lemma_alpha_word_copy(vm, vm2, slo, dlo);
        // tail: shift the copy-correspondence by one
        assert forall|k: int| 0 <= k < shi - (slo + 1) implies
            #[trigger] vm2.tape[(dlo + 1) + k] == vm.tape[(slo + 1) + k] by {
            assert(vm2.tape[dlo + (k + 1)] == vm.tape[slo + (k + 1)]);
        }
        lemma_alpha_words_copy(vm, vm2, slo + 1, shi, dlo + 1);
        lemma_alpha_words_head(vm2, dlo, dhi);
        lemma_alpha_words_head(vm, slo, shi);
        assert(alpha_words(vm2, dlo, dhi)
            =~= seq![alpha_word(vm, slo)] + alpha_words(vm, slo + 1, shi));
    }
}

/// α of an OLD stack pointer is unchanged by a tape EXTENSION (snodes/calls fixed,
/// tape only grows). Needs `wf_svals`: every reachable node's Quote value points
/// inside the OLD tape, so each `alpha_value` is tape-stable via the frame lemma.
pub proof fn lemma_alpha_stack_tape_frame(vm: ModelVm, vm2: ModelVm, ptr: nat)
    requires
        wf_stack(vm),
        wf_tape(vm),
        wf_svals(vm),
        ptr < vm.snodes.len(),
        vm2.snodes == vm.snodes,
        vm2.calls == vm.calls,
        vm.tape.len() <= vm2.tape.len(),
        forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j],
    ensures
        alpha_stack(vm2, ptr) == alpha_stack(vm, ptr),
    decreases ptr,
{
    if ptr == 0 {
    } else {
        assert(vm.snodes[ptr as int].parent < ptr);
        let nd = vm.snodes[ptr as int];
        lemma_alpha_stack_tape_frame(vm, vm2, nd.parent);
        assert(snode_val_wf(vm, ptr as int));
        match nd.value {
            ModelValue::Quote(id) => {
                assert(id.start + id.len <= vm.tape.len());
                lemma_alpha_words_frame(vm, vm2, id.start, id.start + id.len);
            },
            _ => {},
        }
    }
}

/// α of an OLD continuation pointer is unchanged by a tape EXTENSION. Uses only
/// the existing `wf` (each cont segment already has `qend <= tape.len()`).
pub proof fn lemma_alpha_cont_tape_frame(vm: ModelVm, vm2: ModelVm, ptr: nat, off: nat)
    requires
        wf_cont(vm),
        wf_tape(vm),
        ptr < vm.cnodes.len(),
        vm2.cnodes == vm.cnodes,
        vm2.calls == vm.calls,
        vm.tape.len() <= vm2.tape.len(),
        forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j],
    ensures
        alpha_cont(vm2, ptr, off) == alpha_cont(vm, ptr, off),
    decreases ptr,
{
    if ptr == 0 {
    } else {
        assert(wf_cont_node(vm, ptr as int));
        let nd = vm.cnodes[ptr as int];
        let seg_len = nd.qend - nd.qstart;
        let parent_off = vm.cnodes[nd.parent as int].off;
        lemma_alpha_cont_tape_frame(vm, vm2, nd.parent, parent_off);
        if (off as int) < seg_len {
            assert(nd.qend <= vm.tape.len());
            lemma_alpha_words_frame(vm, vm2, nd.qstart + off, nd.qend);
        }
    }
}

// ------------------------------------------------------------
// §3.6  value_to_word bridge + Cat/Cons intern models.
// ------------------------------------------------------------

/// Spec twin of `types::value_to_word` on a MODEL value.
pub open spec fn model_value_to_word(v: ModelValue) -> ModelWord {
    match v {
        ModelValue::Int(n) => ModelWord::PushInt(n),
        ModelValue::Quote(id) => ModelWord::PushQuote(id),
    }
}

/// α of a value-word matches the ghost `value_to_word` on the α'd value.
pub proof fn lemma_value_to_word_alpha(vm: ModelVm, v: ModelValue)
    ensures
        alpha_word_val(vm, model_value_to_word(v)) == value_to_word(alpha_value(vm, v)),
{
}

/// Spec twin of `Vm::try_cat` (vm.rs 68-77): append copies of both bodies at the
/// tape end. The model has an unbounded tape, so it never overflows (the u32
/// ceiling is the deferred capacity precondition, blueprint §4.4).
pub open spec fn model_try_cat(vm: ModelVm, a: ModelQuoteId, b: ModelQuoteId)
    -> (ModelVm, ModelQuoteId) {
    let seg = vm.tape.subrange(a.start as int, (a.start + a.len) as int)
            + vm.tape.subrange(b.start as int, (b.start + b.len) as int);
    let vm2 = ModelVm { tape: vm.tape + seg, ..vm };
    (vm2, ModelQuoteId { start: vm.tape.len(), len: a.len + b.len })
}

/// Spec twin of `Vm::try_cons` (vm.rs 82-91): push `head`, then a copy of `q`.
pub open spec fn model_try_cons(vm: ModelVm, head: ModelWord, q: ModelQuoteId)
    -> (ModelVm, ModelQuoteId) {
    let seg = seq![head] + vm.tape.subrange(q.start as int, (q.start + q.len) as int);
    let vm2 = ModelVm { tape: vm.tape + seg, ..vm };
    (vm2, ModelQuoteId { start: vm.tape.len(), len: q.len + 1 })
}

/// try_cat α law: alpha_quote of the interned copy == alpha_quote(a) ++ alpha_quote(b);
/// the tape only grows (frames old stack/cont), snodes/cnodes/calls fixed, wf kept.
pub proof fn lemma_model_try_cat(vm: ModelVm, a: ModelQuoteId, b: ModelQuoteId)
    requires
        wf(vm),
        a.start + a.len <= vm.tape.len(),
        b.start + b.len <= vm.tape.len(),
    ensures
        ({
            let (vm2, id) = model_try_cat(vm, a, b);
            &&& wf(vm2)
            &&& vm2.snodes == vm.snodes
            &&& vm2.cnodes == vm.cnodes
            &&& vm2.calls == vm.calls
            &&& vm.tape.len() <= vm2.tape.len()
            &&& (forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j])
            &&& id.start == vm.tape.len()
            &&& id.start + id.len == vm2.tape.len()
            &&& alpha_quote(vm2, id) == alpha_quote(vm, a) + alpha_quote(vm, b)
        }),
{
    let (vm2, id) = model_try_cat(vm, a, b);
    let ol = vm.tape.len();
    let seg = vm.tape.subrange(a.start as int, (a.start + a.len) as int)
            + vm.tape.subrange(b.start as int, (b.start + b.len) as int);
    assert(seg.len() == a.len + b.len);
    assert(vm2.tape.len() == ol + a.len + b.len);
    // old prefix preserved.
    assert forall|j: int| 0 <= j < ol implies #[trigger] vm2.tape[j] == vm.tape[j] by {
        assert(vm2.tape[j] == (vm.tape + seg)[j]);
    }
    // copy-correspondence for the two sub-regions.
    assert forall|k: int| 0 <= k < a.len implies #[trigger] vm2.tape[(ol + k) as int] == vm.tape[(a.start + k) as int] by {
        assert(seg[k] == vm.tape.subrange(a.start as int, (a.start + a.len) as int)[k]);
    }
    assert forall|k: int| 0 <= k < b.len implies #[trigger] vm2.tape[(ol + a.len + k) as int] == vm.tape[(b.start + k) as int] by {
        assert(seg[a.len + k] == vm.tape.subrange(b.start as int, (b.start + b.len) as int)[k]);
    }
    // wf(vm2): every appended word is a verbatim copy, hence wf at its new index.
    assert forall|i: int| ol <= i < vm2.tape.len() implies #[trigger] wf_tape_word(vm2, i) by {
        if i < ol + a.len {
            lemma_copied_word_wf(vm, vm2, (a.start + (i - ol)) as nat, i as nat);
        } else {
            lemma_copied_word_wf(vm, vm2, (b.start + (i - ol - a.len)) as nat, i as nat);
        }
    }
    lemma_model_intern_wf(vm, vm2);
    // α of the interned region.
    lemma_alpha_words_split(vm2, ol, (ol + a.len) as nat, (ol + a.len + b.len) as nat);
    lemma_alpha_words_copy(vm, vm2, a.start, a.start + a.len, ol);
    lemma_alpha_words_copy(vm, vm2, b.start, b.start + b.len, (ol + a.len) as nat);
    assert(alpha_quote(vm2, id) =~= alpha_quote(vm, a) + alpha_quote(vm, b));
}

/// try_cons α law: alpha_quote of the interned copy == [α(head)] ++ alpha_quote(q).
/// Requires the head word to be tape-valid (a `PushQuote` head must reference the
/// old tape) so its guard fires at the fresh position.
pub proof fn lemma_model_try_cons(vm: ModelVm, head: ModelWord, q: ModelQuoteId)
    requires
        wf(vm),
        q.start + q.len <= vm.tape.len(),
        (head matches ModelWord::PushQuote(hid) ==> hid.start + hid.len <= vm.tape.len()),
        (head matches ModelWord::Call(k) ==> k < vm.calls.len()),
    ensures
        ({
            let (vm2, id) = model_try_cons(vm, head, q);
            &&& wf(vm2)
            &&& vm2.snodes == vm.snodes
            &&& vm2.cnodes == vm.cnodes
            &&& vm2.calls == vm.calls
            &&& vm.tape.len() <= vm2.tape.len()
            &&& (forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j])
            &&& id.start == vm.tape.len()
            &&& id.start + id.len == vm2.tape.len()
            &&& alpha_quote(vm2, id) == seq![alpha_word_val(vm, head)] + alpha_quote(vm, q)
        }),
{
    let (vm2, id) = model_try_cons(vm, head, q);
    let ol = vm.tape.len();
    let seg = seq![head] + vm.tape.subrange(q.start as int, (q.start + q.len) as int);
    assert(seg.len() == q.len + 1);
    assert(vm2.tape.len() == ol + q.len + 1);
    assert(vm2.tape[ol as int] == head) by {
        assert(vm2.tape[ol as int] == (vm.tape + seg)[ol as int]);
        assert(seg[0] == head);
    }
    assert forall|j: int| 0 <= j < ol implies #[trigger] vm2.tape[j] == vm.tape[j] by {
        assert(vm2.tape[j] == (vm.tape + seg)[j]);
    }
    assert forall|k: int| 0 <= k < q.len implies #[trigger] vm2.tape[(ol + 1 + k) as int] == vm.tape[(q.start + k) as int] by {
        assert(seg[1 + k] == vm.tape.subrange(q.start as int, (q.start + q.len) as int)[k]);
    }
    // wf(vm2): head word at `ol` (value-word, tape-valid), then verbatim copies.
    assert forall|i: int| ol <= i < vm2.tape.len() implies #[trigger] wf_tape_word(vm2, i) by {
        if i == ol {
            assert(vm2.tape[i] == head);
            match head {
                ModelWord::PushQuote(hid) => { assert(hid.start + hid.len <= ol); },
                ModelWord::Call(k) => { assert(k < vm.calls.len()); },
                _ => {},
            }
        } else {
            lemma_copied_word_wf(vm, vm2, (q.start + (i - ol - 1)) as nat, i as nat);
        }
    }
    lemma_model_intern_wf(vm, vm2);
    // α: split off the single head word, then copy the tail.
    lemma_alpha_words_split(vm2, ol, (ol + 1) as nat, (ol + q.len + 1) as nat);
    // head word α.
    lemma_alpha_words_head(vm2, ol, (ol + 1) as nat);
    assert(alpha_words(vm2, (ol + 1) as nat, (ol + 1) as nat) =~= Seq::<SpecWord>::empty());
    assert(alpha_words(vm2, ol, (ol + 1) as nat) =~= seq![alpha_word(vm2, ol)]);
    lemma_alpha_word_head_intern(vm, vm2, head, ol);
    // tail copy.
    lemma_alpha_words_copy(vm, vm2, q.start, q.start + q.len, (ol + 1) as nat);
    assert(alpha_quote(vm2, id)
        =~= seq![alpha_word_val(vm, head)] + alpha_quote(vm, q));
}

/// The freshly-appended head word of a cons, at position `ol == old tape.len()`,
/// has α equal to `alpha_word_val(vm, head)` (its guard fires because a PushQuote
/// head references the old tape).
pub proof fn lemma_alpha_word_head_intern(vm: ModelVm, vm2: ModelVm, head: ModelWord, ol: nat)
    requires
        wf_tape(vm),
        ol == vm.tape.len(),
        ol < vm2.tape.len(),
        vm2.tape[ol as int] == head,
        vm2.calls == vm.calls,
        forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j],
        (head matches ModelWord::PushQuote(hid) ==> hid.start + hid.len <= vm.tape.len()),
    ensures
        alpha_word(vm2, ol) == alpha_word_val(vm, head),
{
    match head {
        ModelWord::PushQuote(hid) => {
            assert(hid.start + hid.len <= ol);
            lemma_alpha_words_frame(vm, vm2, hid.start, hid.start + hid.len);
        },
        _ => {},
    }
}

/// A word copied verbatim to a position past the old tape is wf at its new index
/// (its PushQuote target lies in the old tape, hence strictly below the new index).
pub proof fn lemma_copied_word_wf(vm: ModelVm, vm2: ModelVm, src: nat, dst: nat)
    requires
        wf_tape(vm),
        src < vm.tape.len(),
        vm.tape.len() <= dst,
        dst < vm2.tape.len(),
        vm2.tape[dst as int] == vm.tape[src as int],
        vm2.calls == vm.calls,
    ensures
        wf_tape_word(vm2, dst as int),
{
    assert(wf_tape_word(vm, src as int));
    match vm.tape[src as int] {
        ModelWord::PushQuote(id) => { assert(id.start + id.len <= src); },
        ModelWord::Call(k) => { assert(k < vm.calls.len()); },
        _ => {},
    }
}

/// wf preservation for an intern that only APPENDS to the tape (snodes/cnodes/calls
/// unchanged). The caller supplies the per-word wf of the appended region; the old
/// prefix's wf and the stack/cont invariants carry over from `wf(vm)`.
pub proof fn lemma_model_intern_wf(vm: ModelVm, vm2: ModelVm)
    requires
        wf(vm),
        vm2.snodes == vm.snodes,
        vm2.cnodes == vm.cnodes,
        vm2.calls == vm.calls,
        vm.tape.len() <= vm2.tape.len(),
        forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j],
        // every appended word is wf against vm2 at its own index.
        forall|i: int| vm.tape.len() <= i < vm2.tape.len() ==> #[trigger] wf_tape_word(vm2, i),
    ensures
        wf(vm2),
{
    assert(wf_stack(vm2));
    assert(wf_cont(vm2)) by {
        assert forall|i: int| 1 <= i < vm2.cnodes.len() implies #[trigger] wf_cont_node(vm2, i) by {
            assert(wf_cont_node(vm, i));
            assert(vm.cnodes[i].qend <= vm.tape.len());
        }
    }
    assert(wf_tape(vm2)) by {
        assert forall|i: int| 0 <= i < vm2.tape.len() implies #[trigger] wf_tape_word(vm2, i) by {
            if i < vm.tape.len() {
                assert(wf_tape_word(vm, i));
                assert(vm2.tape[i] == vm.tape[i]);
            }
        }
    }
}

// ------------------------------------------------------------
// M3c infrastructure — literal-segment intern (`try_alloc`) for the setup
// segments of the splice/control prims. Append-only tape extension by a literal
// `Seq<ModelWord>`; α of the interned region is the pointwise value-α.
// ------------------------------------------------------------

/// A word is "intern-wf" against `vm` when a `PushQuote` target lies inside the
/// current tape and a `Call` index is valid — the condition under which appending
/// it to the tape keeps `wf_tape`.
pub open spec fn word_intern_wf(vm: ModelVm, w: ModelWord) -> bool {
    &&& (w matches ModelWord::PushQuote(hid) ==> hid.start + hid.len <= vm.tape.len())
    &&& (w matches ModelWord::Call(k) ==> k < vm.calls.len())
}

/// Spec twin of `Vm::try_alloc(&[..])` (vm.rs 129-141): append a literal segment
/// at the tape end, returning its fresh `QuoteId`. The model tape is unbounded so
/// there is no overflow (capacity is the deferred §4.4 predicate).
pub open spec fn model_try_alloc(vm: ModelVm, words: Seq<ModelWord>)
    -> (ModelVm, ModelQuoteId) {
    let vm2 = ModelVm { tape: vm.tape + words, ..vm };
    (vm2, ModelQuoteId { start: vm.tape.len(), len: words.len() })
}

/// α of one word freshly interned at any position `j` past the old tape equals its
/// value-α (its `PushQuote` target lies in the old tape, so the guard fires).
pub proof fn lemma_alpha_word_intern_at(vm: ModelVm, vm2: ModelVm, w: ModelWord, j: nat)
    requires
        wf_tape(vm),
        vm.tape.len() <= j,
        j < vm2.tape.len(),
        vm2.tape[j as int] == w,
        vm2.calls == vm.calls,
        forall|k: int| 0 <= k < vm.tape.len() ==> vm2.tape[k] == vm.tape[k],
        word_intern_wf(vm, w),
    ensures
        alpha_word(vm2, j) == alpha_word_val(vm, w),
{
    match w {
        ModelWord::PushQuote(hid) => {
            assert(hid.start + hid.len <= vm.tape.len());
            lemma_alpha_words_frame(vm, vm2, hid.start, hid.start + hid.len);
        },
        _ => {},
    }
}

/// α of a freshly-interned literal segment `words` (interned at `base` past the old
/// tape) equals the pointwise value-α of its words. The reusable engine for every
/// setup segment (Dip/PrimRec/Times/LinRec/Fold).
pub proof fn lemma_alpha_words_intern_seq(
    vm: ModelVm, vm2: ModelVm, words: Seq<ModelWord>, base: nat,
)
    requires
        wf_tape(vm),
        vm.tape.len() <= base,
        base + words.len() <= vm2.tape.len(),
        vm2.calls == vm.calls,
        forall|k: int| 0 <= k < vm.tape.len() ==> vm2.tape[k] == vm.tape[k],
        forall|i: int| 0 <= i < words.len() ==> vm2.tape[base + i] == words[i],
        forall|i: int| 0 <= i < words.len() ==> word_intern_wf(vm, #[trigger] words[i]),
    ensures
        alpha_words(vm2, base, (base + words.len()) as nat)
            == Seq::new(words.len(), |i: int| alpha_word_val(vm, words[i])),
    decreases words.len(),
{
    let goal = Seq::new(words.len(), |i: int| alpha_word_val(vm, words[i]));
    if words.len() == 0 {
        assert(alpha_words(vm2, base, base) =~= Seq::<SpecWord>::empty());
        assert(goal =~= Seq::<SpecWord>::empty());
    } else {
        let l = words.len();
        assert(vm2.tape[base as int] == words[0]) by {
            assert(vm2.tape[base + 0int] == words[0]);
        }
        assert(word_intern_wf(vm, words[0]));
        lemma_alpha_word_intern_at(vm, vm2, words[0], base);
        let tw = words.subrange(1, l as int);
        assert(tw.len() == l - 1);
        assert forall|i: int| 0 <= i < tw.len() implies vm2.tape[(base + 1) + i] == tw[i] by {
            assert(tw[i] == words[i + 1]);
            assert(vm2.tape[base + (i + 1)] == words[i + 1]);
        }
        assert forall|i: int| 0 <= i < tw.len() implies word_intern_wf(vm, #[trigger] tw[i]) by {
            assert(tw[i] == words[i + 1]);
        }
        lemma_alpha_words_intern_seq(vm, vm2, tw, base + 1);
        lemma_alpha_words_head(vm2, base, (base + l) as nat);
        let tgoal = Seq::new(tw.len(), |i: int| alpha_word_val(vm, tw[i]));
        assert(alpha_words(vm2, (base + 1) as nat, (base + l) as nat) == tgoal);
        assert(alpha_words(vm2, base, (base + l) as nat)
            =~= seq![alpha_word_val(vm, words[0])] + tgoal);
        assert(goal =~= seq![alpha_word_val(vm, words[0])] + tgoal) by {
            assert(goal.len() == l);
            assert forall|i: int| 0 <= i < l implies
                #[trigger] goal[i] == (seq![alpha_word_val(vm, words[0])] + tgoal)[i] by {
                if i == 0 {
                } else {
                    assert(tgoal[i - 1] == alpha_word_val(vm, tw[i - 1]));
                    assert(tw[i - 1] == words[i]);
                }
            }
        }
    }
}

/// `wf` + frame facts for a literal-segment intern (the α is via
/// `lemma_alpha_words_intern_seq`).
pub proof fn lemma_model_try_alloc(vm: ModelVm, words: Seq<ModelWord>)
    requires
        wf(vm),
        forall|i: int| 0 <= i < words.len() ==> word_intern_wf(vm, #[trigger] words[i]),
    ensures
        ({
            let (vm2, id) = model_try_alloc(vm, words);
            &&& wf(vm2)
            &&& vm2.snodes == vm.snodes
            &&& vm2.cnodes == vm.cnodes
            &&& vm2.calls == vm.calls
            &&& vm.tape.len() <= vm2.tape.len()
            &&& (forall|j: int| 0 <= j < vm.tape.len() ==> vm2.tape[j] == vm.tape[j])
            &&& id.start == vm.tape.len()
            &&& id.len == words.len()
            &&& id.start + id.len == vm2.tape.len()
            &&& (forall|i: int|
                0 <= i < words.len() ==> vm2.tape[(vm.tape.len() + i) as int] == words[i])
        }),
{
    let (vm2, id) = model_try_alloc(vm, words);
    let ol = vm.tape.len();
    assert(vm2.tape.len() == ol + words.len());
    assert forall|j: int| 0 <= j < ol implies #[trigger] vm2.tape[j] == vm.tape[j] by {
        assert(vm2.tape[j] == (vm.tape + words)[j]);
    }
    assert forall|i: int| 0 <= i < words.len() implies
        vm2.tape[(ol + i) as int] == words[i] by {
        assert(vm2.tape[(ol + i) as int] == (vm.tape + words)[(ol + i) as int]);
    }
    assert forall|i: int| ol <= i < vm2.tape.len() implies #[trigger] wf_tape_word(vm2, i) by {
        let k = i - ol;
        assert(vm2.tape[i] == words[k]);
        assert(word_intern_wf(vm, words[k]));
        match words[k] {
            ModelWord::PushQuote(hid) => { assert(hid.start + hid.len <= ol); },
            ModelWord::Call(cc) => { assert(cc < vm.calls.len()); },
            _ => {},
        }
    }
    lemma_model_intern_wf(vm, vm2);
}

// ------------------------------------------------------------
// Pop1 / Pop2 operand-extraction helpers (blueprint §3.2). Package the repeated
// `lemma_alpha_stack_pop1` chain used by every binary/unary prim.
// ------------------------------------------------------------

pub proof fn lemma_pop1(vm: ModelVm, s: nat)
    requires
        wf_stack(vm),
        s < vm.snodes.len(),
        alpha_stack(vm, s).len() >= 1,
    ensures
        s != 0,
        vm.snodes[s as int].parent < vm.snodes.len(),
        ({
            let n = alpha_stack(vm, s).len() as int;
            let rp = vm.snodes[s as int].parent;
            &&& alpha_stack(vm, rp) == alpha_stack(vm, s).subrange(0, n - 1)
            &&& alpha_value(vm, vm.snodes[s as int].value) == alpha_stack(vm, s)[n - 1]
        }),
{
    let astk = alpha_stack(vm, s);
    let n = astk.len() as int;
    assert(s != 0) by {
        if s == 0 { assert(alpha_stack(vm, s) =~= Seq::<SpecValue>::empty()); }
    }
    assert(vm.snodes[s as int].parent < s);
    lemma_alpha_stack_pop1(vm, s);
    assert(astk.last() == astk[n - 1]);
}

pub proof fn lemma_pop2(vm: ModelVm, s: nat)
    requires
        wf_stack(vm),
        s < vm.snodes.len(),
        alpha_stack(vm, s).len() >= 2,
    ensures
        s != 0,
        vm.snodes[s as int].parent != 0,
        vm.snodes[s as int].parent < vm.snodes.len(),
        vm.snodes[vm.snodes[s as int].parent as int].parent < vm.snodes.len(),
        ({
            let n = alpha_stack(vm, s).len() as int;
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            &&& alpha_stack(vm, rp) == alpha_stack(vm, s).subrange(0, n - 2)
            &&& alpha_value(vm, vm.snodes[s as int].value) == alpha_stack(vm, s)[n - 1]
            &&& alpha_value(vm, vm.snodes[p1 as int].value) == alpha_stack(vm, s)[n - 2]
        }),
{
    let astk = alpha_stack(vm, s);
    let n = astk.len() as int;
    assert(s != 0) by {
        if s == 0 { assert(alpha_stack(vm, s) =~= Seq::<SpecValue>::empty()); }
    }
    assert(vm.snodes[s as int].parent < s);
    lemma_alpha_stack_pop1(vm, s);
    let p1 = vm.snodes[s as int].parent;
    assert(alpha_stack(vm, p1) == astk.subrange(0, n - 1));
    assert(alpha_stack(vm, p1).len() == n - 1);
    assert(p1 != 0) by {
        if p1 == 0 { assert(alpha_stack(vm, p1) =~= Seq::<SpecValue>::empty()); }
    }
    assert(vm.snodes[p1 as int].parent < p1);
    lemma_alpha_stack_pop1(vm, p1);
    let rp = vm.snodes[p1 as int].parent;
    assert(alpha_stack(vm, rp) == alpha_stack(vm, p1).subrange(0, (n - 1) - 1));
    assert(alpha_stack(vm, p1).subrange(0, n - 2) =~= astk.subrange(0, n - 2));
    // top value == astk[n-1]
    assert(astk.last() == astk[n - 1]);
    // second value == astk[n-2]
    assert(alpha_stack(vm, p1).last() == alpha_stack(vm, p1)[n - 2]);
    assert(alpha_stack(vm, p1)[n - 2] == astk.subrange(0, n - 1)[n - 2]);
    assert(astk.subrange(0, n - 1)[n - 2] == astk[n - 2]);
}

// ------------------------------------------------------------
// M4 fault-parity infrastructure.
//
// Arity faults (Underflow) require relating the stack pointer-chain depth to
// `alpha_stack(..).len()`; type faults (TypeMismatch) require the operand
// constructor bridge (a model value is `Int`/`Quote` iff its α is). These are
// the "reverse" of the happy-path operand lemmas: instead of assuming the
// operands exist, they characterize WHEN they don't (short stack) or WHEN the
// wrong constructor is present.
// ------------------------------------------------------------

/// `alpha_stack(vm, ptr).len() == 0` exactly at the sentinel; otherwise the parent
/// chain has one fewer element. The engine for the arity-fault (Underflow) arms:
/// `n < k` forces some k-th ancestor ptr to be the `0` sentinel.
pub proof fn lemma_alpha_stack_len(vm: ModelVm, ptr: nat)
    requires
        wf_stack(vm),
        ptr < vm.snodes.len(),
    ensures
        (ptr == 0) <==> (alpha_stack(vm, ptr).len() == 0),
        ptr != 0 ==> {
            let par = vm.snodes[ptr as int].parent;
            &&& par < ptr
            &&& par < vm.snodes.len()
            &&& alpha_stack(vm, ptr).len() == alpha_stack(vm, par).len() + 1
        },
{
    if ptr == 0 {
        assert(alpha_stack(vm, ptr) =~= Seq::<SpecValue>::empty());
    } else {
        assert(vm.snodes[ptr as int].parent < ptr);
        lemma_alpha_stack_pop1(vm, ptr);
    }
}

/// The operand constructor bridge: a model value is an `Int` (resp. `Quote`) iff
/// its α is `SpecValue::Int` (resp. `SpecValue::Quote`). Used by the type-fault
/// (TypeMismatch) arms to transport the spec's failed constructor-match to the
/// model's `_` arm.
pub proof fn lemma_alpha_value_ctor(vm: ModelVm, v: ModelValue)
    ensures
        (v is Int) == (alpha_value(vm, v) is Int),
        (v is Quote) == (alpha_value(vm, v) is Quote),
{
    match v {
        ModelValue::Int(x) => {},
        ModelValue::Quote(id) => {},
    }
}

/// The tape-word constructor bridge: a model word is `PushInt`/`PushQuote` iff its
/// value-form α is. Used by the Uncons/Fold non-value-head fault arms to transport
/// the spec's "head not a value" TypeMismatch to the model's `_` tape-head arm.
pub proof fn lemma_alpha_word_val_ctor(vm: ModelVm, w: ModelWord)
    ensures
        (w is PushInt) == (alpha_word_val(vm, w) is PushInt),
        (w is PushQuote) == (alpha_word_val(vm, w) is PushQuote),
{
    match w {
        ModelWord::PushInt(x) => {},
        ModelWord::PushQuote(id) => {},
        ModelWord::Prim(pp) => {},
        ModelWord::Call(k) => {},
    }
}

/// pop-3 operand extraction (twin of `lemma_pop2`, one level deeper). Exposes the
/// three top model values as the top-three α elements and the depth-3 base ptr.
pub proof fn lemma_pop3(vm: ModelVm, s: nat)
    requires
        wf_stack(vm),
        s < vm.snodes.len(),
        alpha_stack(vm, s).len() >= 3,
    ensures
        s != 0,
        vm.snodes[s as int].parent != 0,
        vm.snodes[vm.snodes[s as int].parent as int].parent != 0,
        vm.snodes[s as int].parent < vm.snodes.len(),
        vm.snodes[vm.snodes[s as int].parent as int].parent < vm.snodes.len(),
        vm.snodes[vm.snodes[vm.snodes[s as int].parent as int].parent as int].parent < vm.snodes.len(),
        ({
            let n = alpha_stack(vm, s).len() as int;
            let p1 = vm.snodes[s as int].parent;
            let p2 = vm.snodes[p1 as int].parent;
            let rp = vm.snodes[p2 as int].parent;
            &&& alpha_stack(vm, rp) == alpha_stack(vm, s).subrange(0, n - 3)
            &&& alpha_value(vm, vm.snodes[s as int].value) == alpha_stack(vm, s)[n - 1]
            &&& alpha_value(vm, vm.snodes[p1 as int].value) == alpha_stack(vm, s)[n - 2]
            &&& alpha_value(vm, vm.snodes[p2 as int].value) == alpha_stack(vm, s)[n - 3]
        }),
{
    let astk = alpha_stack(vm, s);
    let n = astk.len() as int;
    lemma_pop2(vm, s);
    let p1 = vm.snodes[s as int].parent;
    // lemma_pop2 gives alpha_stack(vm,p1)==astk.subrange(0,n-1) via its rp facts,
    // but we re-derive p1's facts directly.
    assert(vm.snodes[s as int].parent < s);
    lemma_alpha_stack_pop1(vm, s);
    assert(alpha_stack(vm, p1) == astk.subrange(0, n - 1));
    assert(alpha_stack(vm, p1).len() == n - 1);
    assert(p1 != 0) by {
        if p1 == 0 { assert(alpha_stack(vm, p1) =~= Seq::<SpecValue>::empty()); }
    }
    assert(vm.snodes[p1 as int].parent < p1);
    lemma_alpha_stack_pop1(vm, p1);
    let p2 = vm.snodes[p1 as int].parent;
    assert(alpha_stack(vm, p2) == alpha_stack(vm, p1).subrange(0, (n - 1) - 1));
    assert(alpha_stack(vm, p1).subrange(0, n - 2) =~= astk.subrange(0, n - 2));
    assert(alpha_stack(vm, p2) == astk.subrange(0, n - 2));
    assert(alpha_stack(vm, p2).len() == n - 2);
    assert(p2 != 0) by {
        if p2 == 0 { assert(alpha_stack(vm, p2) =~= Seq::<SpecValue>::empty()); }
    }
    assert(vm.snodes[p2 as int].parent < p2);
    lemma_alpha_stack_pop1(vm, p2);
    let rp = vm.snodes[p2 as int].parent;
    assert(alpha_stack(vm, rp) == alpha_stack(vm, p2).subrange(0, (n - 2) - 1));
    assert(alpha_stack(vm, p2).subrange(0, n - 3) =~= astk.subrange(0, n - 3));
    assert(alpha_stack(vm, rp) == astk.subrange(0, n - 3));
    // values
    assert(astk.last() == astk[n - 1]);
    assert(alpha_stack(vm, p1).last() == alpha_value(vm, vm.snodes[p1 as int].value));
    assert(alpha_stack(vm, p1).last() == alpha_stack(vm, p1)[n - 2]);
    assert(alpha_stack(vm, p1)[n - 2] == astk.subrange(0, n - 1)[n - 2]);
    assert(astk.subrange(0, n - 1)[n - 2] == astk[n - 2]);
    assert(alpha_stack(vm, p2).last() == alpha_value(vm, vm.snodes[p2 as int].value));
    assert(alpha_stack(vm, p2).last() == alpha_stack(vm, p2)[n - 3]);
    assert(alpha_stack(vm, p2)[n - 3] == astk.subrange(0, n - 2)[n - 3]);
    assert(astk.subrange(0, n - 2)[n - 3] == astk[n - 3]);
}

/// pop-4 operand extraction (LinRec). Exposes the four top model values and the
/// depth-4 base ptr.
pub proof fn lemma_pop4(vm: ModelVm, s: nat)
    requires
        wf_stack(vm),
        s < vm.snodes.len(),
        alpha_stack(vm, s).len() >= 4,
    ensures
        s != 0,
        vm.snodes[s as int].parent != 0,
        vm.snodes[vm.snodes[s as int].parent as int].parent != 0,
        vm.snodes[vm.snodes[vm.snodes[s as int].parent as int].parent as int].parent != 0,
        vm.snodes[s as int].parent < vm.snodes.len(),
        vm.snodes[vm.snodes[s as int].parent as int].parent < vm.snodes.len(),
        vm.snodes[vm.snodes[vm.snodes[s as int].parent as int].parent as int].parent < vm.snodes.len(),
        vm.snodes[vm.snodes[vm.snodes[vm.snodes[s as int].parent as int].parent as int].parent as int].parent < vm.snodes.len(),
        ({
            let n = alpha_stack(vm, s).len() as int;
            let p1 = vm.snodes[s as int].parent;
            let p2 = vm.snodes[p1 as int].parent;
            let p3 = vm.snodes[p2 as int].parent;
            let rp = vm.snodes[p3 as int].parent;
            &&& alpha_stack(vm, rp) == alpha_stack(vm, s).subrange(0, n - 4)
            &&& alpha_value(vm, vm.snodes[s as int].value) == alpha_stack(vm, s)[n - 1]
            &&& alpha_value(vm, vm.snodes[p1 as int].value) == alpha_stack(vm, s)[n - 2]
            &&& alpha_value(vm, vm.snodes[p2 as int].value) == alpha_stack(vm, s)[n - 3]
            &&& alpha_value(vm, vm.snodes[p3 as int].value) == alpha_stack(vm, s)[n - 4]
        }),
{
    let astk = alpha_stack(vm, s);
    let n = astk.len() as int;
    lemma_pop3(vm, s);
    let p1 = vm.snodes[s as int].parent;
    let p2 = vm.snodes[p1 as int].parent;
    assert(alpha_stack(vm, p2) == astk.subrange(0, n - 2));
    assert(alpha_stack(vm, p2).len() == n - 2);
    // one more pop from p2.
    assert(vm.snodes[p2 as int].parent < p2);
    lemma_alpha_stack_pop1(vm, p2);
    let p3 = vm.snodes[p2 as int].parent;
    assert(alpha_stack(vm, p3) == alpha_stack(vm, p2).subrange(0, (n - 2) - 1));
    assert(alpha_stack(vm, p2).subrange(0, n - 3) =~= astk.subrange(0, n - 3));
    assert(alpha_stack(vm, p3) == astk.subrange(0, n - 3));
    assert(alpha_stack(vm, p3).len() == n - 3);
    assert(p3 != 0) by {
        if p3 == 0 { assert(alpha_stack(vm, p3) =~= Seq::<SpecValue>::empty()); }
    }
    assert(vm.snodes[p3 as int].parent < p3);
    lemma_alpha_stack_pop1(vm, p3);
    let rp = vm.snodes[p3 as int].parent;
    assert(alpha_stack(vm, rp) == alpha_stack(vm, p3).subrange(0, (n - 3) - 1));
    assert(alpha_stack(vm, p3).subrange(0, n - 4) =~= astk.subrange(0, n - 4));
    assert(alpha_stack(vm, rp) == astk.subrange(0, n - 4));
    // the fourth value.
    assert(alpha_stack(vm, p3).last() == alpha_value(vm, vm.snodes[p3 as int].value));
    assert(alpha_stack(vm, p3).last() == alpha_stack(vm, p3)[n - 4]);
    assert(alpha_stack(vm, p3)[n - 4] == astk.subrange(0, n - 3)[n - 4]);
    assert(astk.subrange(0, n - 3)[n - 4] == astk[n - 4]);
    // p3's three shallower values are the same as pop3's, but re-expose via subrange.
    assert(alpha_stack(vm, p2)[n - 3] == astk.subrange(0, n - 2)[n - 3]);
    assert(astk.subrange(0, n - 2)[n - 3] == astk[n - 3]);
}

// ============================================================
// 5. M3a — the MODEL arena step + refinement theorem SCAFFOLD.
//    (blueprint §4.0/§4.a; production sources: run.rs::arena_step,
//     vm.rs::next_word/exec_word/exec_prim, prim.rs Dup..Over.)
//
// This layer defines a FAITHFUL twin of production `arena_step`
// (read next word from the model cont with cursor-bump / segment-pop, then
// dispatch PushInt/PushQuote/Call/Prim) and proves the one-step refinement
// theorem for the SCAFFOLD group: the non-primitive cases (PushInt, PushQuote,
// Call->Invoke, Halt) and the pure stack prims Dup/Drop/Swap/Rot/Over.
//
// As of M3c all 23 prims are modeled faithfully in `model_exec_prim` (no
// `arbitrary()` remains) and `is_scaffold_prim` is TRUE for every prim; the
// theorem proves the full happy-path refinement square for all of them. The
// `is_scaffold_*` gate is retained (now vacuously satisfied) as the M4 seam.
//
// Fault parity (the arena's Underflow/TypeMismatch/Overflow vs. spec faults) is
// M4: for M3a the theorem carries a `!(spec_step(...) is Fault)` precondition,
// so only the happy-path (non-underflow) arms are proven. This is stated
// explicitly, not hidden.
//
// NO admit/assume/external cheats; consumes only the M2 lemmas above.
// ============================================================

/// Model Step outcome — the tag mirror of run.rs `Step`. The model is pure-spec,
/// so the resulting `(vm, pos)` is threaded functionally alongside this tag
/// (production threads it via `&mut`).
pub enum ModelStep {
    Next,
    Halt,
    Fault(Error),
    Invoke(Seq<char>),
}

/// α on a model word VALUE (not by tape index). Coincides with `alpha_word(vm, i)`
/// whenever `w == vm.tape[i]` and `wf_tape_word(vm, i)` (see `lemma_alpha_word_val`):
/// the `PushQuote` guard in `alpha_word` is exactly the wf fact, so on a well-formed
/// tape the two agree. `model_next_word` yields a word value, so the engine lemma is
/// stated in terms of this.
pub open spec fn alpha_word_val(vm: ModelVm, w: ModelWord) -> SpecWord {
    match w {
        ModelWord::PushInt(n) => SpecWord::PushInt(n),
        ModelWord::Prim(p) => SpecWord::Prim(p),
        ModelWord::Call(k) => SpecWord::Call(vm.calls[k as int]),
        ModelWord::PushQuote(id) => SpecWord::PushQuote(alpha_quote(vm, id)),
    }
}

/// The value-form and index-form of α on a tape word agree under `wf_tape`.
pub proof fn lemma_alpha_word_val(vm: ModelVm, i: nat)
    requires
        wf_tape(vm),
        i < vm.tape.len(),
    ensures
        alpha_word(vm, i) == alpha_word_val(vm, vm.tape[i as int]),
{
    assert(wf_tape_word(vm, i as int));
    match vm.tape[i as int] {
        ModelWord::PushQuote(id) => {
            assert(id.start + id.len <= i);
        },
        _ => {},
    }
}

// ------------------------------------------------------------
// §4.-1  model_next_word — spec twin of Vm::next_word (vm.rs 148-183).
// Read + consume the next word, popping exhausted segments; None at NIL (halt).
// The `parent < pos.cont` guard (wf_cont) discharges the `decreases` without a
// `requires`; the else-branch is unreachable under wf.
// ------------------------------------------------------------

pub open spec fn model_next_word(vm: ModelVm, pos: ModelVmState)
    -> Option<(ModelWord, ModelVmState)>
    decreases pos.cont,
{
    if pos.cont == 0 {
        None
    } else {
        let nd = vm.cnodes[pos.cont as int];
        let seg_len = nd.qend - nd.qstart;
        if (pos.cursor as int) < seg_len {
            let idx = nd.qstart + pos.cursor;
            Some((vm.tape[idx as int], ModelVmState { cursor: pos.cursor + 1, ..pos }))
        } else if nd.parent == 0 {
            None
        } else if nd.parent < pos.cont {
            let parent = vm.cnodes[nd.parent as int];
            model_next_word(vm, ModelVmState { cont: nd.parent, cursor: parent.off, ..pos })
        } else {
            // Unreachable under wf_cont (parent < ptr). Guard discharges `decreases`.
            None
        }
    }
}

/// The reusable engine: `model_next_word` refines the head/tail split of the
/// flattened continuation (blueprint §3.4). Consumes `lemma_alpha_cont_next_word`
/// (in-segment head-peel) and `lemma_alpha_cont_segpop` (segment-pop).
pub proof fn lemma_model_next_word(vm: ModelVm, pos: ModelVmState)
    requires
        wf(vm),
        wf_pos(vm, pos),
    ensures
        match model_next_word(vm, pos) {
            None => alpha_cont(vm, pos.cont, pos.cursor).len() == 0,
            Some((w, pos2)) => {
                let ac = alpha_cont(vm, pos.cont, pos.cursor);
                &&& wf_pos(vm, pos2)
                &&& pos2.stack == pos.stack
                &&& ac.len() > 0
                &&& alpha_word_val(vm, w) == ac[0]
                &&& alpha_cont(vm, pos2.cont, pos2.cursor) == ac.subrange(1, ac.len() as int)
                // M5: the emitted word is a verbatim tape word, hence intern-wf
                // (a `PushQuote` target / `Call` index references the live tape).
                &&& word_intern_wf(vm, w)
            },
        },
    decreases pos.cont,
{
    let ac = alpha_cont(vm, pos.cont, pos.cursor);
    if pos.cont == 0 {
        assert(ac =~= Seq::<SpecWord>::empty());
    } else {
        assert(wf_cont_node(vm, pos.cont as int));
        let nd = vm.cnodes[pos.cont as int];
        let seg_len = nd.qend - nd.qstart;
        if (pos.cursor as int) < seg_len {
            // In-segment: head-peel.
            lemma_alpha_cont_next_word(vm, pos);
            let idx = nd.qstart + pos.cursor;
            let pos2 = ModelVmState { cursor: pos.cursor + 1, ..pos };
            assert(idx < vm.tape.len());
            lemma_alpha_word_val(vm, idx);
            let tail = alpha_cont(vm, pos.cont, pos.cursor + 1);
            assert(ac =~= seq![alpha_word(vm, idx)] + tail);
            assert(ac[0] == alpha_word(vm, idx));
            assert(alpha_cont(vm, pos2.cont, pos2.cursor) == tail);
            assert(ac.subrange(1, ac.len() as int) =~= tail);
            assert(wf_pos(vm, pos2));
            // word_intern_wf: the emitted word is the verbatim tape word at `idx`,
            // so its per-word tape well-formedness (a weaker form of intern-wf) holds.
            assert(wf_tape_word(vm, idx as int));
            assert(vm.tape[idx as int] == vm.tape[idx as int]);
            assert(word_intern_wf(vm, vm.tape[idx as int]));
        } else if nd.parent == 0 {
            lemma_alpha_cont_segpop(vm, pos);
            assert(ac == alpha_cont(vm, nd.parent, vm.cnodes[nd.parent as int].off));
            assert(ac =~= Seq::<SpecWord>::empty());
        } else {
            assert(nd.parent < pos.cont);
            let parent = vm.cnodes[nd.parent as int];
            let posp = ModelVmState { cont: nd.parent, cursor: parent.off, ..pos };
            lemma_alpha_cont_segpop(vm, pos);
            assert(ac == alpha_cont(vm, posp.cont, posp.cursor));
            assert(wf_cont_node(vm, nd.parent as int));
            assert(wf_pos(vm, posp));
            lemma_model_next_word(vm, posp);
            assert(model_next_word(vm, pos) == model_next_word(vm, posp));
            // word_intern_wf carries over: the two calls return the identical Some((w, _)).
        }
    }
}

// ------------------------------------------------------------
// §4.a  model_push_node — spec twin of StackArena::push(parent, value)
// (arena.rs 114-119): append node {value, parent}, return the new index.
// ------------------------------------------------------------

pub open spec fn model_push_node(vm: ModelVm, ptr: nat, v: ModelValue) -> (ModelVm, nat) {
    (
        ModelVm { snodes: vm.snodes.push(ModelStackNode { value: v, parent: ptr }), ..vm },
        vm.snodes.len(),
    )
}

/// One stack push: preserves `wf`, appends `alpha_value(v)` at the top, and frames
/// α of every pre-existing stack ptr and cont ptr (the tape/calls/cnodes are
/// untouched). Consumes `lemma_alpha_stack_push` + `lemma_alpha_cont_frame`.
pub proof fn lemma_push_node(vm: ModelVm, ptr: nat, v: ModelValue)
    requires
        wf(vm),
        ptr < vm.snodes.len(),
    ensures
        ({
            let (vm2, np) = model_push_node(vm, ptr, v);
            &&& wf(vm2)
            &&& vm2.tape == vm.tape
            &&& vm2.calls == vm.calls
            &&& vm2.cnodes == vm.cnodes
            &&& np == vm.snodes.len()
            &&& vm2.snodes.len() == vm.snodes.len() + 1
            &&& alpha_stack(vm2, np) == alpha_stack(vm, ptr).push(alpha_value(vm, v))
            &&& (forall|q: nat| q < vm.snodes.len() ==> alpha_stack(vm2, q) == alpha_stack(vm, q))
            &&& (forall|cp: nat, off: nat|
                cp < vm.cnodes.len() ==> #[trigger] alpha_cont(vm2, cp, off) == alpha_cont(vm, cp, off))
        }),
{
    let (vm2, np) = model_push_node(vm, ptr, v);
    lemma_alpha_stack_push(vm, ptr, v);
    assert(wf_tape(vm2)) by {
        assert forall|i: int| 0 <= i < vm2.tape.len() implies #[trigger] wf_tape_word(vm2, i) by {
            assert(wf_tape_word(vm, i));
        }
    }
    assert(wf_cont(vm2)) by {
        assert forall|i: int| 1 <= i < vm2.cnodes.len() implies #[trigger] wf_cont_node(vm2, i) by {
            assert(wf_cont_node(vm, i));
        }
    }
    assert forall|cp: nat, off: nat| cp < vm.cnodes.len() implies
        #[trigger] alpha_cont(vm2, cp, off) == alpha_cont(vm, cp, off) by {
        lemma_alpha_cont_frame(vm, vm2, cp, off);
    }
}

// ------------------------------------------------------------
// §4.a  model_exec_prim — the 23 primitive dispatch. ALL 23 prims are modeled
// faithfully (twins of prim.rs): the stack shuffles (M3a), arithmetic /
// comparison / sequence (M3b), and the splice/control combinators
// Apply/Dip/If/Times/PrimRec/LinRec/Fold (M3c). No `arbitrary()` arm remains.
// ------------------------------------------------------------

// ------------------------------------------------------------
// §4.b  Arithmetic / comparison / xor model helpers. All pop TWO Int operands
// (second = a = snodes[parent], top = b = snodes[stack]); the fault order is
// arity (Underflow) -> type (TypeMismatch) -> semantic, faithful to prim.rs's
// `arith`/`divmod`/`cmp` and `spec_arith`/`spec_divmod`. Faults leave (vm, pos)
// UNCHANGED (the stack is only reassigned on the success arm).
// ------------------------------------------------------------

/// Spec twin of `Vm::arith` (prim.rs 336-351): checked binary op, Overflow when
/// the mathematical result leaves i64 range (mirrors `checked_add`/etc).
pub open spec fn model_arith(vm: ModelVm, pos: ModelVmState, op: spec_fn(int, int) -> int)
    -> (ModelStep, ModelVm, ModelVmState) {
    if pos.stack == 0 {
        (ModelStep::Fault(Error::Underflow), vm, pos)
    } else {
        let p1 = vm.snodes[pos.stack as int].parent;
        if p1 == 0 {
            (ModelStep::Fault(Error::Underflow), vm, pos)
        } else {
            let rest_ptr = vm.snodes[p1 as int].parent;
            match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                (ModelValue::Int(a), ModelValue::Int(b)) => {
                    let r = op(a, b);
                    if in_i64(r) {
                        let (vm2, np) = model_push_node(vm, rest_ptr, ModelValue::Int(r));
                        (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
                    } else {
                        (ModelStep::Fault(Error::Overflow), vm, pos)
                    }
                },
                _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
            }
        }
    }
}

/// Spec twin of `Vm::divmod` (prim.rs 353-374): DivByZero precedes Overflow, both
/// inside the both-Int arm; MIN/-1 faults Overflow for div AND mod (checked_rem).
pub open spec fn model_divmod(vm: ModelVm, pos: ModelVmState, is_div: bool)
    -> (ModelStep, ModelVm, ModelVmState) {
    if pos.stack == 0 {
        (ModelStep::Fault(Error::Underflow), vm, pos)
    } else {
        let p1 = vm.snodes[pos.stack as int].parent;
        if p1 == 0 {
            (ModelStep::Fault(Error::Underflow), vm, pos)
        } else {
            let rest_ptr = vm.snodes[p1 as int].parent;
            match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                (ModelValue::Int(a), ModelValue::Int(b)) => {
                    if b == 0 {
                        (ModelStep::Fault(Error::DivByZero), vm, pos)
                    } else if !in_i64(trunc_div(a, b)) {
                        (ModelStep::Fault(Error::Overflow), vm, pos)
                    } else {
                        let r = if is_div { trunc_div(a, b) } else { trunc_mod(a, b) };
                        let (vm2, np) = model_push_node(vm, rest_ptr, ModelValue::Int(r));
                        (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
                    }
                },
                _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
            }
        }
    }
}

/// Spec twin of `Vm::cmp` (prim.rs 376-389): push 1/0. Total (no semantic fault).
pub open spec fn model_cmp(vm: ModelVm, pos: ModelVmState, op: spec_fn(int, int) -> bool)
    -> (ModelStep, ModelVm, ModelVmState) {
    if pos.stack == 0 {
        (ModelStep::Fault(Error::Underflow), vm, pos)
    } else {
        let p1 = vm.snodes[pos.stack as int].parent;
        if p1 == 0 {
            (ModelStep::Fault(Error::Underflow), vm, pos)
        } else {
            let rest_ptr = vm.snodes[p1 as int].parent;
            match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                (ModelValue::Int(a), ModelValue::Int(b)) => {
                    let r: int = if op(a, b) { 1int } else { 0int };
                    let (vm2, np) = model_push_node(vm, rest_ptr, ModelValue::Int(r));
                    (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
                },
                _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
            }
        }
    }
}

/// Spec twin of the `Prim::Xor` arm (prim.rs 143-154): two's-complement i64 xor,
/// NO Overflow arm (`i64_bitxor` is total).
pub open spec fn model_xor(vm: ModelVm, pos: ModelVmState)
    -> (ModelStep, ModelVm, ModelVmState) {
    if pos.stack == 0 {
        (ModelStep::Fault(Error::Underflow), vm, pos)
    } else {
        let p1 = vm.snodes[pos.stack as int].parent;
        if p1 == 0 {
            (ModelStep::Fault(Error::Underflow), vm, pos)
        } else {
            let rest_ptr = vm.snodes[p1 as int].parent;
            match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                (ModelValue::Int(a), ModelValue::Int(b)) => {
                    let (vm2, np) = model_push_node(vm, rest_ptr, ModelValue::Int(i64_bitxor(a, b)));
                    (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
                },
                _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
            }
        }
    }
}

/// Spec twin of `Prim::Cons` (prim.rs 104-117): ( v [q] -- [v q] ). v = second,
/// q = top. Interns `value_to_word(v) :: q` then pushes the fresh quote.
pub open spec fn model_cons(vm: ModelVm, pos: ModelVmState)
    -> (ModelStep, ModelVm, ModelVmState) {
    if pos.stack == 0 {
        (ModelStep::Fault(Error::Underflow), vm, pos)
    } else {
        let p1 = vm.snodes[pos.stack as int].parent;
        if p1 == 0 {
            (ModelStep::Fault(Error::Underflow), vm, pos)
        } else {
            let rest_ptr = vm.snodes[p1 as int].parent;
            let v = vm.snodes[p1 as int].value;            // second
            match vm.snodes[pos.stack as int].value {       // top must be a quote
                ModelValue::Quote(qid) => {
                    let (vm_t, new_id) = model_try_cons(vm, model_value_to_word(v), qid);
                    let (vm2, np) = model_push_node(vm_t, rest_ptr, ModelValue::Quote(new_id));
                    (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
                },
                _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
            }
        }
    }
}

/// Spec twin of `Prim::Cat` (prim.rs 91-103): ( [a] [b] -- [a b] ). a = second,
/// b = top. Interns the concatenation then pushes the fresh quote.
pub open spec fn model_cat(vm: ModelVm, pos: ModelVmState)
    -> (ModelStep, ModelVm, ModelVmState) {
    if pos.stack == 0 {
        (ModelStep::Fault(Error::Underflow), vm, pos)
    } else {
        let p1 = vm.snodes[pos.stack as int].parent;
        if p1 == 0 {
            (ModelStep::Fault(Error::Underflow), vm, pos)
        } else {
            let rest_ptr = vm.snodes[p1 as int].parent;
            match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                (ModelValue::Quote(aid), ModelValue::Quote(bid)) => {
                    let (vm_t, new_id) = model_try_cat(vm, aid, bid);
                    let (vm2, np) = model_push_node(vm_t, rest_ptr, ModelValue::Quote(new_id));
                    (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
                },
                _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
            }
        }
    }
}

/// Spec twin of `Prim::Uncons` (prim.rs 253-288): ( [w ...] -- w [...] 1 ) | ( [] -- 0 ).
/// A non-value head (bare Prim/Call) faults TypeMismatch and leaves (vm, pos)
/// UNTOUCHED (the head is inspected before any commit).
pub open spec fn model_uncons(vm: ModelVm, pos: ModelVmState)
    -> (ModelStep, ModelVm, ModelVmState) {
    if pos.stack == 0 {
        (ModelStep::Fault(Error::Underflow), vm, pos)
    } else {
        let rest_ptr = vm.snodes[pos.stack as int].parent;   // base
        match vm.snodes[pos.stack as int].value {
            ModelValue::Quote(qid) => {
                if qid.len == 0 {
                    let (vm2, np) = model_push_node(vm, rest_ptr, ModelValue::Int(0int));
                    (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
                } else {
                    let tail = ModelQuoteId { start: qid.start + 1, len: (qid.len - 1) as nat };
                    match vm.tape[qid.start as int] {
                        ModelWord::PushInt(k) => {
                            let (vm1, s1) = model_push_node(vm, rest_ptr, ModelValue::Int(k));
                            let (vm2, s2) = model_push_node(vm1, s1, ModelValue::Quote(tail));
                            let (vm3, s3) = model_push_node(vm2, s2, ModelValue::Int(1int));
                            (ModelStep::Next, vm3, ModelVmState { stack: s3, ..pos })
                        },
                        ModelWord::PushQuote(hid) => {
                            let (vm1, s1) = model_push_node(vm, rest_ptr, ModelValue::Quote(hid));
                            let (vm2, s2) = model_push_node(vm1, s1, ModelValue::Quote(tail));
                            let (vm3, s3) = model_push_node(vm2, s2, ModelValue::Int(1int));
                            (ModelStep::Next, vm3, ModelVmState { stack: s3, ..pos })
                        },
                        _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
                    }
                }
            },
            _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
        }
    }
}

pub open spec fn model_exec_prim(vm: ModelVm, pos: ModelVmState, p: SpecPrim)
    -> (ModelStep, ModelVm, ModelVmState) {
    match p {
        SpecPrim::Dup => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let top = vm.snodes[pos.stack as int].value;
                let (vm2, np) = model_push_node(vm, pos.stack, top);
                (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
            }
        },
        SpecPrim::Drop => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let rest = vm.snodes[pos.stack as int].parent;
                (ModelStep::Next, vm, ModelVmState { stack: rest, ..pos })
            }
        },
        SpecPrim::Swap => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 == 0 {
                    (ModelStep::Fault(Error::Underflow), vm, pos)
                } else {
                    let b = vm.snodes[pos.stack as int].value; // top
                    let a = vm.snodes[p1 as int].value;         // second
                    let rest = vm.snodes[p1 as int].parent;
                    let (vm1, s1) = model_push_node(vm, rest, b);
                    let (vm2, s2) = model_push_node(vm1, s1, a);
                    (ModelStep::Next, vm2, ModelVmState { stack: s2, ..pos })
                }
            }
        },
        SpecPrim::Rot => {
            // ( a b c -- b c a )
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 == 0 {
                    (ModelStep::Fault(Error::Underflow), vm, pos)
                } else {
                    let p2 = vm.snodes[p1 as int].parent;
                    if p2 == 0 {
                        (ModelStep::Fault(Error::Underflow), vm, pos)
                    } else {
                        let c = vm.snodes[pos.stack as int].value; // top
                        let b = vm.snodes[p1 as int].value;         // second
                        let a = vm.snodes[p2 as int].value;         // third
                        let rest = vm.snodes[p2 as int].parent;
                        let (vm1, s1) = model_push_node(vm, rest, b);
                        let (vm2, s2) = model_push_node(vm1, s1, c);
                        let (vm3, s3) = model_push_node(vm2, s2, a);
                        (ModelStep::Next, vm3, ModelVmState { stack: s3, ..pos })
                    }
                }
            }
        },
        SpecPrim::Over => {
            // ( a b -- a b a )
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 == 0 {
                    (ModelStep::Fault(Error::Underflow), vm, pos)
                } else {
                    let a = vm.snodes[p1 as int].value; // second
                    let (vm2, np) = model_push_node(vm, pos.stack, a);
                    (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
                }
            }
        },
        // ---------- M3b: arithmetic / comparison / sequence ----------
        SpecPrim::Add => model_arith(vm, pos, |a: int, b: int| a + b),
        SpecPrim::Sub => model_arith(vm, pos, |a: int, b: int| a - b),
        SpecPrim::Mul => model_arith(vm, pos, |a: int, b: int| a * b),
        SpecPrim::Div => model_divmod(vm, pos, true),
        SpecPrim::Mod => model_divmod(vm, pos, false),
        SpecPrim::Eq => model_cmp(vm, pos, |a: int, b: int| a == b),
        SpecPrim::Lt => model_cmp(vm, pos, |a: int, b: int| a < b),
        SpecPrim::Xor => model_xor(vm, pos),
        SpecPrim::Cons => model_cons(vm, pos),
        SpecPrim::Cat => model_cat(vm, pos),
        SpecPrim::Uncons => model_uncons(vm, pos),
        // ---------- M3c: splice / control (continuation-building) ----------
        // Apply ( [q] -- ) : splice q's body onto the cont. cont := q ++ rest.
        SpecPrim::Apply => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let rest_ptr = vm.snodes[pos.stack as int].parent;
                match vm.snodes[pos.stack as int].value {
                    ModelValue::Quote(qid) => {
                        let pos_mid = ModelVmState { stack: rest_ptr, ..pos };
                        let (vm2, pos2) = model_prepend(vm, pos_mid, qid);
                        (ModelStep::Next, vm2, pos2)
                    },
                    _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
                }
            }
        },
        // Dip ( a [q] -- ... a ) : cont := q ++ [value_to_word(a)] ++ rest.
        // prim.rs interns [value_to_word(a)], prepends it, THEN prepends q.
        SpecPrim::Dip => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 == 0 {
                    (ModelStep::Fault(Error::Underflow), vm, pos)
                } else {
                    let rest_ptr = vm.snodes[p1 as int].parent;
                    let a = vm.snodes[p1 as int].value;    // second
                    match vm.snodes[pos.stack as int].value {  // top = quote
                        ModelValue::Quote(qid) => {
                            let (vm_a, seg_id) =
                                model_try_alloc(vm, seq![model_value_to_word(a)]);
                            let pos_mid = ModelVmState { stack: rest_ptr, ..pos };
                            let (vm_b, pos_b) = model_prepend(vm_a, pos_mid, seg_id);
                            let (vm_c, pos_c) = model_prepend(vm_b, pos_b, qid);
                            (ModelStep::Next, vm_c, pos_c)
                        },
                        _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
                    }
                }
            }
        },
        // If ( c [t] [f] -- ) : splice the selected branch. cont := branch ++ rest.
        SpecPrim::If => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 == 0 {
                    (ModelStep::Fault(Error::Underflow), vm, pos)
                } else {
                    let p2 = vm.snodes[p1 as int].parent;
                    if p2 == 0 {
                        (ModelStep::Fault(Error::Underflow), vm, pos)
                    } else {
                        let rest_ptr = vm.snodes[p2 as int].parent;
                        match (vm.snodes[p2 as int].value,   // cond
                               vm.snodes[p1 as int].value,   // t
                               vm.snodes[pos.stack as int].value) {  // f
                            (ModelValue::Int(c), ModelValue::Quote(qt), ModelValue::Quote(qf)) => {
                                let branch = if c != 0 { qt } else { qf };
                                let pos_mid = ModelVmState { stack: rest_ptr, ..pos };
                                let (vm2, pos2) = model_prepend(vm, pos_mid, branch);
                                (ModelStep::Next, vm2, pos2)
                            },
                            _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
                        }
                    }
                }
            }
        },
        // Times ( n [Q] -- ... ) : k<=0 no-op; k>0 cont := Q ++ [k-1,[Q],times] ++ rest.
        SpecPrim::Times => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 == 0 {
                    (ModelStep::Fault(Error::Underflow), vm, pos)
                } else {
                    let rest_ptr = vm.snodes[p1 as int].parent;
                    match (vm.snodes[p1 as int].value,        // n
                           vm.snodes[pos.stack as int].value) {  // q
                        (ModelValue::Int(k), ModelValue::Quote(qid)) => {
                            let pos_mid = ModelVmState { stack: rest_ptr, ..pos };
                            if k <= 0 {
                                (ModelStep::Next, vm, pos_mid)
                            } else {
                                let setup = seq![
                                    ModelWord::PushInt(k - 1),
                                    ModelWord::PushQuote(qid),
                                    ModelWord::Prim(SpecPrim::Times)
                                ];
                                let (vm_a, seg_id) = model_try_alloc(vm, setup);
                                let (vm_b, pos_b) = model_prepend(vm_a, pos_mid, seg_id);
                                let (vm_c, pos_c) = model_prepend(vm_b, pos_b, qid);
                                (ModelStep::Next, vm_c, pos_c)
                            }
                        },
                        _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
                    }
                }
            }
        },
        // PrimRec ( n [I] [C] -- r ) : k<=0 cont := I ++ rest; k>0
        // cont := [k,k-1,[I],[C],primrec] ++ C ++ rest.
        SpecPrim::PrimRec => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 == 0 {
                    (ModelStep::Fault(Error::Underflow), vm, pos)
                } else {
                    let p2 = vm.snodes[p1 as int].parent;
                    if p2 == 0 {
                        (ModelStep::Fault(Error::Underflow), vm, pos)
                    } else {
                        let rest_ptr = vm.snodes[p2 as int].parent;
                        match (vm.snodes[p2 as int].value,   // n
                               vm.snodes[p1 as int].value,   // qi
                               vm.snodes[pos.stack as int].value) {  // qc
                            (ModelValue::Int(k), ModelValue::Quote(qi), ModelValue::Quote(qc)) => {
                                let pos_mid = ModelVmState { stack: rest_ptr, ..pos };
                                if k <= 0 {
                                    let (vm2, pos2) = model_prepend(vm, pos_mid, qi);
                                    (ModelStep::Next, vm2, pos2)
                                } else {
                                    let setup = seq![
                                        ModelWord::PushInt(k),
                                        ModelWord::PushInt(k - 1),
                                        ModelWord::PushQuote(qi),
                                        ModelWord::PushQuote(qc),
                                        ModelWord::Prim(SpecPrim::PrimRec)
                                    ];
                                    let (vm_a, seg_id) = model_try_alloc(vm, setup);
                                    let (vm_b, pos_b) = model_prepend(vm_a, pos_mid, qc);
                                    let (vm_c, pos_c) = model_prepend(vm_b, pos_b, seg_id);
                                    (ModelStep::Next, vm_c, pos_c)
                                }
                            },
                            _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
                        }
                    }
                }
            }
        },
        // LinRec ( [P] [T] [R1] [R2] -- ... ) : desugars into If.
        // else_q := R1 ++ [[P],[T],[R1],[R2],linrec] ++ R2 ;
        // cont := P ++ [[T],[else_q],If] ++ rest.
        SpecPrim::LinRec => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 == 0 {
                    (ModelStep::Fault(Error::Underflow), vm, pos)
                } else {
                    let p2 = vm.snodes[p1 as int].parent;
                    if p2 == 0 {
                        (ModelStep::Fault(Error::Underflow), vm, pos)
                    } else {
                        let p3 = vm.snodes[p2 as int].parent;
                        if p3 == 0 {
                            (ModelStep::Fault(Error::Underflow), vm, pos)
                        } else {
                            let rest_ptr = vm.snodes[p3 as int].parent;
                            match (vm.snodes[p3 as int].value,   // qp
                                   vm.snodes[p2 as int].value,   // qt
                                   vm.snodes[p1 as int].value,   // qr1
                                   vm.snodes[pos.stack as int].value) {  // qr2
                                (ModelValue::Quote(qp), ModelValue::Quote(qt),
                                 ModelValue::Quote(qr1), ModelValue::Quote(qr2)) => {
                                    let pos_mid = ModelVmState { stack: rest_ptr, ..pos };
                                    let else_seg =
                                        vm.tape.subrange(qr1.start as int, (qr1.start + qr1.len) as int)
                                        + seq![
                                            ModelWord::PushQuote(qp),
                                            ModelWord::PushQuote(qt),
                                            ModelWord::PushQuote(qr1),
                                            ModelWord::PushQuote(qr2),
                                            ModelWord::Prim(SpecPrim::LinRec)
                                        ]
                                        + vm.tape.subrange(qr2.start as int, (qr2.start + qr2.len) as int);
                                    let (vm_e, else_id) = model_try_alloc(vm, else_seg);
                                    let seg = seq![
                                        ModelWord::PushQuote(qt),
                                        ModelWord::PushQuote(else_id),
                                        ModelWord::Prim(SpecPrim::If)
                                    ];
                                    let (vm_a, seg_id) = model_try_alloc(vm_e, seg);
                                    let (vm_b, pos_b) = model_prepend(vm_a, pos_mid, seg_id);
                                    let (vm_c, pos_c) = model_prepend(vm_b, pos_b, qp);
                                    (ModelStep::Next, vm_c, pos_c)
                                },
                                _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
                            }
                        }
                    }
                }
            }
        },
        // Fold ( [seq] init [C] -- r ) LEFT fold. Empty seq pushes init;
        // else cont := [[tail],init,head] ++ C ++ [[C],fold] ++ rest.
        SpecPrim::Fold => {
            if pos.stack == 0 {
                (ModelStep::Fault(Error::Underflow), vm, pos)
            } else {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 == 0 {
                    (ModelStep::Fault(Error::Underflow), vm, pos)
                } else {
                    let p2 = vm.snodes[p1 as int].parent;
                    if p2 == 0 {
                        (ModelStep::Fault(Error::Underflow), vm, pos)
                    } else {
                        let rest_ptr = vm.snodes[p2 as int].parent;
                        let init = vm.snodes[p1 as int].value;    // second
                        match (vm.snodes[p2 as int].value,        // seq
                               vm.snodes[pos.stack as int].value) {  // combine
                            (ModelValue::Quote(qs), ModelValue::Quote(qc)) => {
                                let pos_mid = ModelVmState { stack: rest_ptr, ..pos };
                                if qs.len == 0 {
                                    let (vm2, np) = model_push_node(vm, rest_ptr, init);
                                    (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
                                } else {
                                    let head = vm.tape[qs.start as int];
                                    match head {
                                        ModelWord::PushInt(_) | ModelWord::PushQuote(_) => {
                                            let tail = ModelQuoteId {
                                                start: qs.start + 1, len: (qs.len - 1) as nat,
                                            };
                                            let seg_c = seq![
                                                ModelWord::PushQuote(qc),
                                                ModelWord::Prim(SpecPrim::Fold)
                                            ];
                                            let seg_a = seq![
                                                ModelWord::PushQuote(tail),
                                                model_value_to_word(init),
                                                head
                                            ];
                                            let (vm_c1, seg_c_id) = model_try_alloc(vm, seg_c);
                                            let (vm_a1, seg_a_id) = model_try_alloc(vm_c1, seg_a);
                                            let (vm_1, pos_1) = model_prepend(vm_a1, pos_mid, seg_c_id);
                                            let (vm_2, pos_2) = model_prepend(vm_1, pos_1, qc);
                                            let (vm_3, pos_3) = model_prepend(vm_2, pos_2, seg_a_id);
                                            (ModelStep::Next, vm_3, pos_3)
                                        },
                                        _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
                                    }
                                }
                            },
                            _ => (ModelStep::Fault(Error::TypeMismatch), vm, pos),
                        }
                    }
                }
            }
        },
    }
}

/// Spec twin of `Vm::exec_word` (vm.rs 255-274).
pub open spec fn model_exec_word(vm: ModelVm, pos: ModelVmState, w: ModelWord)
    -> (ModelStep, ModelVm, ModelVmState) {
    match w {
        ModelWord::PushInt(n) => {
            let (vm2, np) = model_push_node(vm, pos.stack, ModelValue::Int(n));
            (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
        },
        ModelWord::PushQuote(id) => {
            let (vm2, np) = model_push_node(vm, pos.stack, ModelValue::Quote(id));
            (ModelStep::Next, vm2, ModelVmState { stack: np, ..pos })
        },
        ModelWord::Call(k) => (ModelStep::Invoke(vm.calls[k as int]), vm, pos),
        ModelWord::Prim(p) => model_exec_prim(vm, pos, p),
    }
}

/// Spec twin of `run.rs::arena_step` (run.rs 92-108): read the next word
/// (`model_next_word`); on None -> Halt; else `model_exec_word`, and on a Fault
/// RESTORE the pre-step position (run.rs 100-105) while keeping the (append-only)
/// vm.
pub open spec fn model_arena_step(vm: ModelVm, pos: ModelVmState)
    -> (ModelStep, ModelVm, ModelVmState) {
    match model_next_word(vm, pos) {
        None => (ModelStep::Halt, vm, pos),
        Some((w, pos1)) => {
            let (r, vm2, pos2) = model_exec_word(vm, pos1, w);
            match r {
                ModelStep::Fault(f) => (ModelStep::Fault(f), vm2, pos),
                _ => (r, vm2, pos2),
            }
        },
    }
}

// ------------------------------------------------------------
// Scaffold gate — the refinement theorem quantifies only over the M3a step set:
// Halt, PushInt, PushQuote, Call, and the prims Dup/Drop/Swap/Rot/Over.
// ------------------------------------------------------------

pub open spec fn is_scaffold_prim(p: SpecPrim) -> bool {
    match p {
        // M3a stack shuffles
        SpecPrim::Dup => true,
        SpecPrim::Drop => true,
        SpecPrim::Swap => true,
        SpecPrim::Rot => true,
        SpecPrim::Over => true,
        // M3b arithmetic / comparison / sequence
        SpecPrim::Add => true,
        SpecPrim::Sub => true,
        SpecPrim::Mul => true,
        SpecPrim::Div => true,
        SpecPrim::Mod => true,
        SpecPrim::Eq => true,
        SpecPrim::Lt => true,
        SpecPrim::Xor => true,
        SpecPrim::Cons => true,
        SpecPrim::Cat => true,
        SpecPrim::Uncons => true,
        // M3c splice / control
        SpecPrim::Apply => true,
        SpecPrim::Dip => true,
        SpecPrim::If => true,
        SpecPrim::Times => true,
        SpecPrim::PrimRec => true,
        SpecPrim::LinRec => true,
        SpecPrim::Fold => true,
    }
}

pub open spec fn is_scaffold_word(w: ModelWord) -> bool {
    match w {
        ModelWord::Prim(p) => is_scaffold_prim(p),
        _ => true,
    }
}

pub open spec fn is_scaffold_step(vm: ModelVm, pos: ModelVmState) -> bool {
    match model_next_word(vm, pos) {
        None => true,
        Some((w, _)) => is_scaffold_word(w),
    }
}

// ------------------------------------------------------------
// §4.0/§4.a  The refinement theorem — SCAFFOLD group.
//
// For a wf model state, `model_arena_step` refines `spec_step ∘ alpha_state`
// (the α∘step == step∘α square), restricted to the scaffold step set and the
// non-fault (happy) path. Mirrors `mtl_core::exec_step`'s ensures (mtl_core
// 866-876). M3b/M3c extend by dropping the `is_scaffold_step` gate one prim
// group at a time; M4 drops the `!Fault` precondition (fault parity).
// ------------------------------------------------------------

// ------------------------------------------------------------
// M4  Fault parity (the `!Fault` precondition dropped).
//
// When `spec_step_prim(astk, p, rest)` is a Fault, `model_arena_step(vm, pos)`
// faults with the SAME `Error` and leaves the machine literally untouched
// (`vm2 == vm && pos2 == pos`), honoring the normative precedence
// (arity→Underflow before type→TypeMismatch; DivByZero before Overflow). This is
// the arena analogue of P2's "final(vm) == old(vm) on fault".
//
// Both faults are proven side by side: the arity fault is derived from
// `lemma_alpha_stack_len` (a short α-stack forces some k-th ancestor ptr to be the
// `0` sentinel, so the model's pop-chain returns Underflow); the type fault from
// the constructor bridges (`lemma_alpha_value_ctor` / `lemma_alpha_word_val_ctor`)
// carrying the spec's failed constructor-match to the model's `_` arm; the
// semantic faults (arith Overflow, div/mod DivByZero/Overflow) share the SAME
// `in_i64`/`b==0`/`trunc_div` checks as the spec (they are shared fault parity,
// not arena-only). NO divergence was found: every arm faults exactly where the
// spec does, same kind, same precedence.
//
// Split into two proof fns (control prims in `thm_prim_fault_ctrl`) so no single
// function blows the per-function Z3 rlimit.
// ------------------------------------------------------------

pub open spec fn is_ctrl_fault_prim(p: SpecPrim) -> bool {
    p is Dip || p is If || p is Times || p is PrimRec || p is LinRec || p is Fold
}

/// Fault parity for the continuation-splice/control prims Dip/If/Times/PrimRec/
/// LinRec/Fold. Their fault arms are arity + type only (the splice/intern happens
/// on the SUCCESS path, so on fault the model returns immediately with the
/// untouched vm/pos).
pub proof fn thm_prim_fault_ctrl(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        is_ctrl_fault_prim(p),
        astk == alpha_stack(vm, pos.stack),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        spec_step_prim(astk, p, rest) is Fault,
    ensures
        ({
            let sf = spec_step_prim(astk, p, rest);
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r == ModelStep::Fault(sf->Fault_0)
            &&& vm2 == vm
            &&& pos2 == pos
        }),
{
    let sf = spec_step_prim(astk, p, rest);
    let e = sf->Fault_0;
    let s = pos1.stack;
    assert(alpha_stack(vm, s) == astk);
    match p {
        SpecPrim::Dip => {
            // ( a [q] -- ) : arity 2, TOP must be a Quote.
            lemma_alpha_stack_len(vm, s);
            if n < 2 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    assert(n == 1);
                    lemma_alpha_stack_len(vm, p1);
                    assert(p1 == 0);
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop2(vm, s);
                let b = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, b);
                assert(!(astk[n - 1] is Quote)) by {
                    if astk[n - 1] is Quote { assert(spec_step_prim(astk, p, rest) is Next); }
                };
                assert(!(b is Quote));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        SpecPrim::Times => {
            // ( n [Q] -- ) : arity 2, (Int, Quote).
            lemma_alpha_stack_len(vm, s);
            if n < 2 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    assert(n == 1);
                    lemma_alpha_stack_len(vm, p1);
                    assert(p1 == 0);
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop2(vm, s);
                let p1 = vm.snodes[s as int].parent;
                let nn = vm.snodes[p1 as int].value;
                let q = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, nn);
                lemma_alpha_value_ctor(vm, q);
                assert(!(astk[n - 2] is Int && astk[n - 1] is Quote)) by {
                    if astk[n - 2] is Int && astk[n - 1] is Quote {
                        assert(spec_step_prim(astk, p, rest) is Next);
                    }
                };
                assert(!(nn is Int && q is Quote));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        SpecPrim::If => {
            // ( c [t] [f] -- ) : arity 3, (Int, Quote, Quote).
            lemma_alpha_stack_len(vm, s);
            if n < 3 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    lemma_alpha_stack_len(vm, p1);
                    if p1 == 0 {
                    } else {
                        let p2 = vm.snodes[p1 as int].parent;
                        assert(alpha_stack(vm, p2).len() == n - 2);
                        assert(n == 2);
                        lemma_alpha_stack_len(vm, p2);
                        assert(p2 == 0);
                    }
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop3(vm, s);
                let p1 = vm.snodes[s as int].parent;
                let p2 = vm.snodes[p1 as int].parent;
                let cond = vm.snodes[p2 as int].value;
                let t = vm.snodes[p1 as int].value;
                let f = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, cond);
                lemma_alpha_value_ctor(vm, t);
                lemma_alpha_value_ctor(vm, f);
                assert(!(astk[n - 3] is Int && astk[n - 2] is Quote && astk[n - 1] is Quote)) by {
                    if astk[n - 3] is Int && astk[n - 2] is Quote && astk[n - 1] is Quote {
                        assert(spec_step_prim(astk, p, rest) is Next);
                    }
                };
                assert(!(cond is Int && t is Quote && f is Quote));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        SpecPrim::PrimRec => {
            // ( n [I] [C] -- r ) : arity 3, (Int, Quote, Quote).
            lemma_alpha_stack_len(vm, s);
            if n < 3 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    lemma_alpha_stack_len(vm, p1);
                    if p1 == 0 {
                    } else {
                        let p2 = vm.snodes[p1 as int].parent;
                        assert(alpha_stack(vm, p2).len() == n - 2);
                        assert(n == 2);
                        lemma_alpha_stack_len(vm, p2);
                        assert(p2 == 0);
                    }
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop3(vm, s);
                let p1 = vm.snodes[s as int].parent;
                let p2 = vm.snodes[p1 as int].parent;
                let k = vm.snodes[p2 as int].value;
                let qi = vm.snodes[p1 as int].value;
                let qc = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, k);
                lemma_alpha_value_ctor(vm, qi);
                lemma_alpha_value_ctor(vm, qc);
                assert(!(astk[n - 3] is Int && astk[n - 2] is Quote && astk[n - 1] is Quote)) by {
                    if astk[n - 3] is Int && astk[n - 2] is Quote && astk[n - 1] is Quote {
                        assert(spec_step_prim(astk, p, rest) is Next);
                    }
                };
                assert(!(k is Int && qi is Quote && qc is Quote));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        SpecPrim::LinRec => {
            // ( [P] [T] [R1] [R2] -- ) : arity 4, all Quotes.
            lemma_alpha_stack_len(vm, s);
            if n < 4 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    lemma_alpha_stack_len(vm, p1);
                    if p1 == 0 {
                    } else {
                        let p2 = vm.snodes[p1 as int].parent;
                        assert(alpha_stack(vm, p2).len() == n - 2);
                        lemma_alpha_stack_len(vm, p2);
                        if p2 == 0 {
                        } else {
                            let p3 = vm.snodes[p2 as int].parent;
                            assert(alpha_stack(vm, p3).len() == n - 3);
                            assert(n == 3);
                            lemma_alpha_stack_len(vm, p3);
                            assert(p3 == 0);
                        }
                    }
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop4(vm, s);
                let p1 = vm.snodes[s as int].parent;
                let p2 = vm.snodes[p1 as int].parent;
                let p3 = vm.snodes[p2 as int].parent;
                let qp = vm.snodes[p3 as int].value;
                let qt = vm.snodes[p2 as int].value;
                let qr1 = vm.snodes[p1 as int].value;
                let qr2 = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, qp);
                lemma_alpha_value_ctor(vm, qt);
                lemma_alpha_value_ctor(vm, qr1);
                lemma_alpha_value_ctor(vm, qr2);
                assert(!(astk[n - 4] is Quote && astk[n - 3] is Quote && astk[n - 2] is Quote
                        && astk[n - 1] is Quote)) by {
                    if astk[n - 4] is Quote && astk[n - 3] is Quote && astk[n - 2] is Quote
                        && astk[n - 1] is Quote {
                        assert(spec_step_prim(astk, p, rest) is Next);
                    }
                };
                assert(!(qp is Quote && qt is Quote && qr1 is Quote && qr2 is Quote));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        SpecPrim::Fold => {
            // ( [seq] init [C] -- r ) : arity 3, seq/combine Quotes, then non-value head.
            lemma_alpha_stack_len(vm, s);
            if n < 3 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    lemma_alpha_stack_len(vm, p1);
                    if p1 == 0 {
                    } else {
                        let p2 = vm.snodes[p1 as int].parent;
                        assert(alpha_stack(vm, p2).len() == n - 2);
                        assert(n == 2);
                        lemma_alpha_stack_len(vm, p2);
                        assert(p2 == 0);
                    }
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop3(vm, s);
                let p1 = vm.snodes[s as int].parent;
                let p2 = vm.snodes[p1 as int].parent;
                let seqv = vm.snodes[p2 as int].value;   // stk[n-3]
                let combv = vm.snodes[s as int].value;    // stk[n-1]
                lemma_alpha_value_ctor(vm, seqv);
                lemma_alpha_value_ctor(vm, combv);
                if !(astk[n - 3] is Quote) || !(astk[n - 1] is Quote) {
                    assert(!(seqv is Quote) || !(combv is Quote));
                    assert(sf == SpecStep::Fault(Error::TypeMismatch));
                    assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
                } else {
                    // both seq and combine are Quotes; the fault is a non-value seq head.
                    let qsw = astk[n - 3]->Quote_0;
                    assert(astk[n - 3] == SpecValue::Quote(qsw));
                    assert(seqv is Quote);
                    let qs = seqv->Quote_0;
                    assert(seqv == ModelValue::Quote(qs));
                    assert(alpha_quote(vm, qs) == qsw);
                    assert(snode_val_wf(vm, p2 as int));
                    assert(qs.start + qs.len <= vm.tape.len());
                    let he = (qs.start + qs.len) as nat;
                    lemma_alpha_words_len(vm, qs.start, he);
                    assert(qsw.len() == qs.len);
                    assert(qsw.len() > 0) by {
                        if qsw.len() == 0 { assert(spec_step_prim(astk, p, rest) is Next); }
                    };
                    assert(qs.len > 0);
                    assert(qs.start < vm.tape.len());
                    lemma_alpha_words_head(vm, qs.start, he);
                    assert(qsw == seq![alpha_word(vm, qs.start)] + alpha_words(vm, (qs.start + 1) as nat, he));
                    assert(qsw[0] == alpha_word(vm, qs.start));
                    lemma_alpha_word_val(vm, qs.start);
                    assert(qsw[0] == alpha_word_val(vm, vm.tape[qs.start as int]));
                    assert(!(qsw[0] is PushInt) && !(qsw[0] is PushQuote)) by {
                        if qsw[0] is PushInt || qsw[0] is PushQuote {
                            assert(spec_step_prim(astk, p, rest) is Next);
                        }
                    };
                    lemma_alpha_word_val_ctor(vm, vm.tape[qs.start as int]);
                    assert(!(vm.tape[qs.start as int] is PushInt) && !(vm.tape[qs.start as int] is PushQuote));
                    assert(sf == SpecStep::Fault(Error::TypeMismatch));
                    assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        _ => {
            assert(false);
        },
    }
    assert(model_exec_word(vm, pos1, ModelWord::Prim(p)) == (ModelStep::Fault(e), vm, pos1));
    assert(model_arena_step(vm, pos) == (ModelStep::Fault(e), vm, pos));
}

/// Fault parity for the non-control prims (stack shuffles, arithmetic/comparison,
/// Cat/Cons/Uncons, Apply). Dispatches the 6 control prims to
/// `thm_prim_fault_ctrl`.
pub proof fn thm_prim_fault(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        astk == alpha_stack(vm, pos.stack),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        spec_step_prim(astk, p, rest) is Fault,
    ensures
        ({
            let sf = spec_step_prim(astk, p, rest);
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r == ModelStep::Fault(sf->Fault_0)
            &&& vm2 == vm
            &&& pos2 == pos
        }),
{
    let sf = spec_step_prim(astk, p, rest);
    let e = sf->Fault_0;
    let s = pos1.stack;
    assert(alpha_stack(vm, s) == astk);
    match p {
        // ------- control prims: delegate -------
        SpecPrim::Dip | SpecPrim::If | SpecPrim::Times | SpecPrim::PrimRec
        | SpecPrim::LinRec | SpecPrim::Fold => {
            thm_prim_fault_ctrl(vm, pos, pos1, p, astk, rest, n);
        },
        // ------- arity-only prims (only Underflow) -------
        SpecPrim::Dup => {
            lemma_alpha_stack_len(vm, s);
            assert(n < 1) by {
                if n >= 1 { assert(spec_step_prim(astk, p, rest) is Next); }
            };
            assert(s == 0);
            assert(sf == SpecStep::Fault(Error::Underflow));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        SpecPrim::Drop => {
            lemma_alpha_stack_len(vm, s);
            assert(n < 1) by {
                if n >= 1 { assert(spec_step_prim(astk, p, rest) is Next); }
            };
            assert(s == 0);
            assert(sf == SpecStep::Fault(Error::Underflow));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        SpecPrim::Swap => {
            lemma_alpha_stack_len(vm, s);
            assert(n < 2) by {
                if n >= 2 { assert(spec_step_prim(astk, p, rest) is Next); }
            };
            if s == 0 {
            } else {
                let p1 = vm.snodes[s as int].parent;
                assert(alpha_stack(vm, p1).len() == n - 1);
                assert(n == 1);
                lemma_alpha_stack_len(vm, p1);
                assert(p1 == 0);
            }
            assert(sf == SpecStep::Fault(Error::Underflow));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        SpecPrim::Over => {
            lemma_alpha_stack_len(vm, s);
            assert(n < 2) by {
                if n >= 2 { assert(spec_step_prim(astk, p, rest) is Next); }
            };
            if s == 0 {
            } else {
                let p1 = vm.snodes[s as int].parent;
                assert(alpha_stack(vm, p1).len() == n - 1);
                assert(n == 1);
                lemma_alpha_stack_len(vm, p1);
                assert(p1 == 0);
            }
            assert(sf == SpecStep::Fault(Error::Underflow));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        SpecPrim::Rot => {
            lemma_alpha_stack_len(vm, s);
            assert(n < 3) by {
                if n >= 3 { assert(spec_step_prim(astk, p, rest) is Next); }
            };
            if s == 0 {
            } else {
                let p1 = vm.snodes[s as int].parent;
                assert(alpha_stack(vm, p1).len() == n - 1);
                lemma_alpha_stack_len(vm, p1);
                if p1 == 0 {
                } else {
                    let p2 = vm.snodes[p1 as int].parent;
                    assert(alpha_stack(vm, p2).len() == n - 2);
                    assert(n == 2);
                    lemma_alpha_stack_len(vm, p2);
                    assert(p2 == 0);
                }
            }
            assert(sf == SpecStep::Fault(Error::Underflow));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        // ------- Apply: arity 1, TOP Quote -------
        SpecPrim::Apply => {
            lemma_alpha_stack_len(vm, s);
            if n < 1 {
                assert(s == 0);
                assert(sf == SpecStep::Fault(Error::Underflow));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop1(vm, s);
                let b = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, b);
                assert(!(astk[n - 1] is Quote)) by {
                    if astk[n - 1] is Quote { assert(spec_step_prim(astk, p, rest) is Next); }
                };
                assert(!(b is Quote));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        // ------- Cat: arity 2, both Quote -------
        SpecPrim::Cat => {
            lemma_alpha_stack_len(vm, s);
            if n < 2 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    assert(n == 1);
                    lemma_alpha_stack_len(vm, p1);
                    assert(p1 == 0);
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop2(vm, s);
                let p1 = vm.snodes[s as int].parent;
                let a = vm.snodes[p1 as int].value;
                let b = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, a);
                lemma_alpha_value_ctor(vm, b);
                assert(!(astk[n - 2] is Quote && astk[n - 1] is Quote)) by {
                    if astk[n - 2] is Quote && astk[n - 1] is Quote {
                        assert(spec_step_prim(astk, p, rest) is Next);
                    }
                };
                assert(!(a is Quote && b is Quote));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        // ------- Cons: arity 2, TOP Quote -------
        SpecPrim::Cons => {
            lemma_alpha_stack_len(vm, s);
            if n < 2 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    assert(n == 1);
                    lemma_alpha_stack_len(vm, p1);
                    assert(p1 == 0);
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop2(vm, s);
                let b = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, b);
                assert(!(astk[n - 1] is Quote)) by {
                    if astk[n - 1] is Quote { assert(spec_step_prim(astk, p, rest) is Next); }
                };
                assert(!(b is Quote));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        // ------- Eq / Lt / Xor: arity 2, both Int, NO semantic fault -------
        SpecPrim::Eq | SpecPrim::Lt | SpecPrim::Xor => {
            lemma_alpha_stack_len(vm, s);
            if n < 2 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    assert(n == 1);
                    lemma_alpha_stack_len(vm, p1);
                    assert(p1 == 0);
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop2(vm, s);
                let p1 = vm.snodes[s as int].parent;
                let a = vm.snodes[p1 as int].value;
                let b = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, a);
                lemma_alpha_value_ctor(vm, b);
                assert(!(astk[n - 2] is Int && astk[n - 1] is Int)) by {
                    if astk[n - 2] is Int && astk[n - 1] is Int {
                        assert(spec_step_prim(astk, p, rest) is Next);
                    }
                };
                assert(!(a is Int && b is Int));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        // ------- Add / Sub / Mul: arity 2, both Int, Overflow -------
        SpecPrim::Add | SpecPrim::Sub | SpecPrim::Mul => {
            lemma_alpha_stack_len(vm, s);
            if n < 2 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    assert(n == 1);
                    lemma_alpha_stack_len(vm, p1);
                    assert(p1 == 0);
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else if astk[n - 2] is Int && astk[n - 1] is Int {
                let a = astk[n - 2]->Int_0;
                let b = astk[n - 1]->Int_0;
                lemma_binop_int(vm, s, astk, n, a, b);
                // spec faults with both Int => Overflow (the op result leaves i64).
                let r = match p {
                    SpecPrim::Add => a + b,
                    SpecPrim::Sub => a - b,
                    _ => a * b,
                };
                assert(!in_i64(r)) by {
                    if in_i64(r) { assert(spec_step_prim(astk, p, rest) is Next); }
                };
                assert(sf == SpecStep::Fault(Error::Overflow));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Overflow), vm, pos1));
            } else {
                lemma_pop2(vm, s);
                let p1 = vm.snodes[s as int].parent;
                let a = vm.snodes[p1 as int].value;
                let b = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, a);
                lemma_alpha_value_ctor(vm, b);
                assert(!(a is Int && b is Int));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        // ------- Div / Mod: arity 2, both Int, DivByZero BEFORE Overflow -------
        SpecPrim::Div | SpecPrim::Mod => {
            lemma_alpha_stack_len(vm, s);
            if n < 2 {
                assert(sf == SpecStep::Fault(Error::Underflow));
                if s == 0 {
                } else {
                    let p1 = vm.snodes[s as int].parent;
                    assert(alpha_stack(vm, p1).len() == n - 1);
                    assert(n == 1);
                    lemma_alpha_stack_len(vm, p1);
                    assert(p1 == 0);
                }
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else if astk[n - 2] is Int && astk[n - 1] is Int {
                let a = astk[n - 2]->Int_0;
                let b = astk[n - 1]->Int_0;
                lemma_binop_int(vm, s, astk, n, a, b);
                if b == 0 {
                    assert(sf == SpecStep::Fault(Error::DivByZero));
                    assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::DivByZero), vm, pos1));
                } else {
                    assert(!in_i64(trunc_div(a, b))) by {
                        if in_i64(trunc_div(a, b)) { assert(spec_step_prim(astk, p, rest) is Next); }
                    };
                    assert(sf == SpecStep::Fault(Error::Overflow));
                    assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Overflow), vm, pos1));
                }
            } else {
                lemma_pop2(vm, s);
                let p1 = vm.snodes[s as int].parent;
                let a = vm.snodes[p1 as int].value;
                let b = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, a);
                lemma_alpha_value_ctor(vm, b);
                assert(!(a is Int && b is Int));
                assert(sf == SpecStep::Fault(Error::TypeMismatch));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
        // ------- Uncons: arity 1, TOP Quote, non-value head -------
        SpecPrim::Uncons => {
            lemma_alpha_stack_len(vm, s);
            if n < 1 {
                assert(s == 0);
                assert(sf == SpecStep::Fault(Error::Underflow));
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::Underflow), vm, pos1));
            } else {
                lemma_pop1(vm, s);
                let bv = vm.snodes[s as int].value;
                lemma_alpha_value_ctor(vm, bv);
                if !(astk[n - 1] is Quote) {
                    assert(!(bv is Quote));
                    assert(sf == SpecStep::Fault(Error::TypeMismatch));
                    assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
                } else {
                    let qw = astk[n - 1]->Quote_0;
                    assert(astk[n - 1] == SpecValue::Quote(qw));
                    assert(bv is Quote);
                    let qid = bv->Quote_0;
                    assert(bv == ModelValue::Quote(qid));
                    assert(alpha_quote(vm, qid) == qw);
                    assert(snode_val_wf(vm, s as int));
                    assert(qid.start + qid.len <= vm.tape.len());
                    let he = (qid.start + qid.len) as nat;
                    lemma_alpha_words_len(vm, qid.start, he);
                    assert(qw.len() == qid.len);
                    assert(qw.len() > 0) by {
                        if qw.len() == 0 { assert(spec_step_prim(astk, p, rest) is Next); }
                    };
                    assert(qid.len > 0);
                    assert(qid.start < vm.tape.len());
                    lemma_alpha_words_head(vm, qid.start, he);
                    assert(qw == seq![alpha_word(vm, qid.start)] + alpha_words(vm, (qid.start + 1) as nat, he));
                    assert(qw[0] == alpha_word(vm, qid.start));
                    lemma_alpha_word_val(vm, qid.start);
                    assert(qw[0] == alpha_word_val(vm, vm.tape[qid.start as int]));
                    assert(!(qw[0] is PushInt) && !(qw[0] is PushQuote)) by {
                        if qw[0] is PushInt || qw[0] is PushQuote {
                            assert(spec_step_prim(astk, p, rest) is Next);
                        }
                    };
                    lemma_alpha_word_val_ctor(vm, vm.tape[qid.start as int]);
                    assert(!(vm.tape[qid.start as int] is PushInt)
                        && !(vm.tape[qid.start as int] is PushQuote));
                    assert(sf == SpecStep::Fault(Error::TypeMismatch));
                    assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(Error::TypeMismatch), vm, pos1));
                }
            }
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Fault(e), vm, pos1));
        },
    }
    assert(model_exec_word(vm, pos1, ModelWord::Prim(p)) == model_exec_prim(vm, pos1, p));
    assert(model_arena_step(vm, pos) == (ModelStep::Fault(e), vm, pos));
}

pub proof fn thm_arena_refines_spec_scaffold(vm: ModelVm, pos: ModelVmState)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        // NOTE: the `is_scaffold_step` gate is GONE. All 23 prims are modeled and
        // both refined (happy path) and fault-matched (M4), so the theorem is now
        // UNCONDITIONAL over all inputs (fault + non-fault) for any wf state.
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            match spec_step(alpha_state(vm, pos)) {
                SpecStep::Next(s2) => r is Next && wf(vm2) && wf_pos(vm2, pos2)
                    && alpha_state(vm2, pos2) == s2,
                SpecStep::Halt(_) => r is Halt,
                // M4 fault parity: same Error kind, and the machine is left
                // literally untouched (α unchanged) — run.rs's `*st = saved` and
                // prim.rs's stack-unchanged-on-fault contract.
                SpecStep::Fault(e) => r == ModelStep::Fault(e) && vm2 == vm && pos2 == pos,
                SpecStep::Invoke(nm, stk, ct) => r is Invoke && r->Invoke_0 == nm
                    && alpha_state(vm2, pos2) == (SpecState { stack: stk, cont: ct }),
            }
        }),
{
    lemma_model_next_word(vm, pos);
    let astk = alpha_stack(vm, pos.stack);
    let ac = alpha_cont(vm, pos.cont, pos.cursor);
    let sstate = alpha_state(vm, pos);
    assert(sstate.stack == astk);
    assert(sstate.cont == ac);

    match model_next_word(vm, pos) {
        None => {
            // Halt: ac empty => spec_step is Halt; model_arena_step returns Halt.
            assert(ac.len() == 0);
            assert(model_arena_step(vm, pos) == (ModelStep::Halt, vm, pos));
            assert(spec_step(sstate) is Halt);
        },
        Some((w, pos1)) => {
            assert(ac.len() > 0);
            let rest = ac.subrange(1, ac.len() as int);
            assert(alpha_word_val(vm, w) == ac[0]);
            assert(alpha_cont(vm, pos1.cont, pos1.cursor) == rest);
            assert(pos1.stack == pos.stack);
            assert(pos1.stack < vm.snodes.len());

            match w {
                ModelWord::PushInt(n) => {
                    assert(ac[0] == SpecWord::PushInt(n));
                    let (vm2, np) = model_push_node(vm, pos1.stack, ModelValue::Int(n));
                    lemma_push_node(vm, pos1.stack, ModelValue::Int(n));
                    assert(alpha_value(vm, ModelValue::Int(n)) == SpecValue::Int(n));
                    assert(alpha_stack(vm2, np) == astk.push(SpecValue::Int(n)));
                    assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
                    let pos2 = ModelVmState { stack: np, ..pos1 };
                    assert(model_exec_word(vm, pos1, w) == (ModelStep::Next, vm2, pos2));
                    assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
                    assert(wf_pos(vm2, pos2));
                    assert(spec_step(sstate)
                        == SpecStep::Next(SpecState { stack: astk.push(SpecValue::Int(n)), cont: rest }));
                    assert(alpha_state(vm2, pos2)
                        == (SpecState { stack: astk.push(SpecValue::Int(n)), cont: rest }));
                },
                ModelWord::PushQuote(id) => {
                    assert(ac[0] == SpecWord::PushQuote(alpha_quote(vm, id)));
                    let (vm2, np) = model_push_node(vm, pos1.stack, ModelValue::Quote(id));
                    lemma_push_node(vm, pos1.stack, ModelValue::Quote(id));
                    assert(alpha_value(vm, ModelValue::Quote(id))
                        == SpecValue::Quote(alpha_quote(vm, id)));
                    assert(alpha_stack(vm2, np) == astk.push(SpecValue::Quote(alpha_quote(vm, id))));
                    assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
                    let pos2 = ModelVmState { stack: np, ..pos1 };
                    assert(model_exec_word(vm, pos1, w) == (ModelStep::Next, vm2, pos2));
                    assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
                    assert(wf_pos(vm2, pos2));
                    assert(spec_step(sstate)
                        == SpecStep::Next(SpecState {
                            stack: astk.push(SpecValue::Quote(alpha_quote(vm, id))), cont: rest }));
                    assert(alpha_state(vm2, pos2)
                        == (SpecState {
                            stack: astk.push(SpecValue::Quote(alpha_quote(vm, id))), cont: rest }));
                },
                ModelWord::Call(k) => {
                    assert(ac[0] == SpecWord::Call(vm.calls[k as int]));
                    // model: (Invoke(calls[k]), vm, pos1); spec: Invoke(calls[k], astk, rest).
                    assert(model_exec_word(vm, pos1, w)
                        == (ModelStep::Invoke(vm.calls[k as int]), vm, pos1));
                    assert(model_arena_step(vm, pos)
                        == (ModelStep::Invoke(vm.calls[k as int]), vm, pos1));
                    assert(spec_step(sstate) == SpecStep::Invoke(vm.calls[k as int], astk, rest));
                    assert(alpha_state(vm, pos1) == (SpecState { stack: astk, cont: rest }));
                },
                ModelWord::Prim(p) => {
                    assert(ac[0] == SpecWord::Prim(p));
                    assert(is_scaffold_prim(p));
                    let n = astk.len() as int;
                    assert(model_next_word(vm, pos)
                        == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)));
                    assert(spec_step(sstate) == spec_step_prim(astk, p, rest));
                    if spec_step_prim(astk, p, rest) is Fault {
                        // M4 fault parity: same kind, machine untouched.
                        thm_prim_fault(vm, pos, pos1, p, astk, rest, n);
                    } else {
                        thm_prim_scaffold(vm, pos, pos1, p, astk, ac, rest, n);
                    }
                },
            }
        },
    }
}

/// Operand extraction for a binary prim whose two operands are BOTH `Int`. Given
/// `astk[n-2] == Int(a)` and `astk[n-1] == Int(b)`, exposes the model nodes as
/// `Int(a)` / `Int(b)` and `alpha_stack(rp) == astk[..n-2]`.
pub proof fn lemma_binop_int(
    vm: ModelVm, s: nat, astk: Seq<SpecValue>, n: int, a: int, b: int,
)
    requires
        wf_stack(vm),
        s < vm.snodes.len(),
        astk == alpha_stack(vm, s),
        n == astk.len(),
        n >= 2,
        astk[n - 2] == SpecValue::Int(a),
        astk[n - 1] == SpecValue::Int(b),
    ensures
        s != 0,
        vm.snodes[s as int].parent != 0,
        ({
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            &&& rp < vm.snodes.len()
            &&& vm.snodes[p1 as int].value == ModelValue::Int(a)
            &&& vm.snodes[s as int].value == ModelValue::Int(b)
            &&& alpha_stack(vm, rp) == astk.subrange(0, n - 2)
        }),
{
    lemma_pop2(vm, s);
    let p1 = vm.snodes[s as int].parent;
    let bv = vm.snodes[s as int].value;
    let av = vm.snodes[p1 as int].value;
    assert(alpha_value(vm, bv) == SpecValue::Int(b));
    assert(alpha_value(vm, av) == SpecValue::Int(a));
    match bv {
        ModelValue::Int(x) => { assert(alpha_value(vm, bv) == SpecValue::Int(x)); },
        ModelValue::Quote(id) => { assert(alpha_value(vm, bv) == SpecValue::Quote(alpha_quote(vm, id))); },
    }
    match av {
        ModelValue::Int(x) => { assert(alpha_value(vm, av) == SpecValue::Int(x)); },
        ModelValue::Quote(id) => { assert(alpha_value(vm, av) == SpecValue::Quote(alpha_quote(vm, id))); },
    }
}

/// The Prim(p) dispatch of the scaffold theorem, split out to keep the top
/// theorem readable. Proves Dup/Drop/Swap/Rot/Over (M3a) and
/// Add/Sub/Mul/Div/Mod/Eq/Lt/Xor/Cons/Cat/Uncons (M3b); the non-scaffold `_` arm
/// is unreachable (`is_scaffold_prim(p)`).
// rlimit headroom: this dispatcher is large (11 inlined prim arms); the M4
// fault-parity lemmas now in scope perturb Z3's search enough to nudge it past
// the default budget. Bumping the per-function rlimit (NOT a soundness cheat —
// no admit/assume) keeps it green.
#[verifier::rlimit(100)]
pub proof fn thm_prim_scaffold(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    ac: Seq<SpecWord>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        is_scaffold_prim(p),
        astk == alpha_stack(vm, pos.stack),
        ac == alpha_cont(vm, pos.cont, pos.cursor),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        alpha_cont(vm, pos1.cont, pos1.cursor) == rest,
        rest == ac.subrange(1, ac.len() as int),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        !(spec_step_prim(astk, p, rest) is Fault),
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r is Next
            &&& wf(vm2)
            &&& wf_pos(vm2, pos2)
            &&& spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2))
        }),
{
    // model_arena_step == the (non-fault) result of model_exec_prim(vm, pos1, p).
    // Establish n>=k underflow-freedom per prim from `!(... is Fault)`.
    match p {
        SpecPrim::Dup => {
            assert(n >= 1);
            let s = pos1.stack;
            assert(s != 0) by {
                if s == 0 {
                    assert(alpha_stack(vm, s) =~= Seq::<SpecValue>::empty());
                }
            }
            let top = vm.snodes[s as int].value;
            lemma_alpha_stack_pop1(vm, s);
            assert(astk.last() == alpha_value(vm, top));
            assert(astk.last() == astk[n - 1]);
            let (vm2, np) = model_push_node(vm, s, top);
            lemma_push_node(vm, s, top);
            assert(alpha_stack(vm2, np) == astk.push(astk[n - 1]));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_word(vm, pos1, ModelWord::Prim(p)) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(wf_pos(vm2, pos2));
            assert(alpha_state(vm2, pos2)
                == (SpecState { stack: astk.push(astk[n - 1]), cont: rest }));
        },
        SpecPrim::Drop => {
            assert(n >= 1);
            let s = pos1.stack;
            assert(s != 0) by {
                if s == 0 {
                    assert(alpha_stack(vm, s) =~= Seq::<SpecValue>::empty());
                }
            }
            lemma_alpha_stack_pop1(vm, s);
            let rest_ptr = vm.snodes[s as int].parent;
            assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 1));
            assert(rest_ptr < vm.snodes.len());
            let pos2 = ModelVmState { stack: rest_ptr, ..pos1 };
            assert(model_exec_word(vm, pos1, ModelWord::Prim(p)) == (ModelStep::Next, vm, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm, pos2));
            assert(wf_pos(vm, pos2));
            assert(alpha_state(vm, pos2)
                == (SpecState { stack: astk.subrange(0, n - 1), cont: rest }));
        },
        SpecPrim::Swap => {
            assert(n >= 2);
            let s = pos1.stack;
            assert(s != 0) by {
                if s == 0 {
                    assert(alpha_stack(vm, s) =~= Seq::<SpecValue>::empty());
                }
            }
            let p1 = vm.snodes[s as int].parent;
            lemma_alpha_stack_pop1(vm, s);
            assert(alpha_stack(vm, p1) == astk.subrange(0, n - 1));
            assert(alpha_stack(vm, p1).len() == n - 1);
            assert(p1 != 0) by {
                if p1 == 0 {
                    assert(alpha_stack(vm, p1) =~= Seq::<SpecValue>::empty());
                }
            }
            let b = vm.snodes[s as int].value; // top
            let a = vm.snodes[p1 as int].value; // second
            let rest_ptr = vm.snodes[p1 as int].parent;
            assert(astk.last() == alpha_value(vm, b));
            assert(astk.last() == astk[n - 1]);
            lemma_alpha_stack_pop1(vm, p1);
            assert(alpha_stack(vm, rest_ptr) == alpha_stack(vm, p1).subrange(0, (n - 1) - 1));
            assert(alpha_stack(vm, p1).subrange(0, n - 2) =~= astk.subrange(0, n - 2));
            assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 2));
            assert(alpha_stack(vm, p1).last() == alpha_value(vm, a));
            assert(alpha_stack(vm, p1).last() == astk.subrange(0, n - 1)[n - 2]);
            assert(astk.subrange(0, n - 1)[n - 2] == astk[n - 2]);
            assert(alpha_value(vm, a) == astk[n - 2]);
            assert(rest_ptr < vm.snodes.len());

            let (vm1, s1) = model_push_node(vm, rest_ptr, b);
            lemma_push_node(vm, rest_ptr, b);
            assert(alpha_stack(vm1, s1) == astk.subrange(0, n - 2).push(astk[n - 1]));
            let (vm2, s2) = model_push_node(vm1, s1, a);
            assert(s1 < vm1.snodes.len());
            lemma_push_node(vm1, s1, a);
            lemma_alpha_value_tape_eq(vm, vm1, a);
            assert(alpha_value(vm1, a) == astk[n - 2]);
            assert(alpha_stack(vm2, s2)
                == astk.subrange(0, n - 2).push(astk[n - 1]).push(astk[n - 2]));
            // cont frame chain vm2 <- vm1 <- vm.
            assert(alpha_cont(vm1, pos1.cont, pos1.cursor) == rest);
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            let pos2 = ModelVmState { stack: s2, ..pos1 };
            assert(model_exec_word(vm, pos1, ModelWord::Prim(p)) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(vm2.cnodes == vm.cnodes);
            assert(wf_pos(vm2, pos2));
            assert(alpha_state(vm2, pos2)
                == (SpecState {
                    stack: astk.subrange(0, n - 2).push(astk[n - 1]).push(astk[n - 2]),
                    cont: rest }));
        },
        SpecPrim::Rot => {
            assert(n >= 3);
            let s = pos1.stack;
            assert(s != 0) by {
                if s == 0 {
                    assert(alpha_stack(vm, s) =~= Seq::<SpecValue>::empty());
                }
            }
            let p1 = vm.snodes[s as int].parent;
            lemma_alpha_stack_pop1(vm, s);
            assert(alpha_stack(vm, p1) == astk.subrange(0, n - 1));
            assert(alpha_stack(vm, p1).len() == n - 1);
            assert(p1 != 0) by {
                if p1 == 0 {
                    assert(alpha_stack(vm, p1) =~= Seq::<SpecValue>::empty());
                }
            }
            let p2 = vm.snodes[p1 as int].parent;
            lemma_alpha_stack_pop1(vm, p1);
            assert(alpha_stack(vm, p2) == alpha_stack(vm, p1).subrange(0, (n - 1) - 1));
            assert(alpha_stack(vm, p1).subrange(0, n - 2) =~= astk.subrange(0, n - 2));
            assert(alpha_stack(vm, p2) == astk.subrange(0, n - 2));
            assert(alpha_stack(vm, p2).len() == n - 2);
            assert(p2 != 0) by {
                if p2 == 0 {
                    assert(alpha_stack(vm, p2) =~= Seq::<SpecValue>::empty());
                }
            }
            let c = vm.snodes[s as int].value; // top
            let b = vm.snodes[p1 as int].value; // second
            let a = vm.snodes[p2 as int].value; // third
            let rest_ptr = vm.snodes[p2 as int].parent;
            // c == astk[n-1]
            assert(astk.last() == alpha_value(vm, c));
            assert(astk.last() == astk[n - 1]);
            // b == astk[n-2]
            assert(alpha_stack(vm, p1).last() == alpha_value(vm, b));
            assert(alpha_stack(vm, p1).last() == astk.subrange(0, n - 1)[n - 2]);
            assert(astk.subrange(0, n - 1)[n - 2] == astk[n - 2]);
            assert(alpha_value(vm, b) == astk[n - 2]);
            // a == astk[n-3]
            lemma_alpha_stack_pop1(vm, p2);
            assert(alpha_stack(vm, rest_ptr) == alpha_stack(vm, p2).subrange(0, (n - 2) - 1));
            assert(alpha_stack(vm, p2).subrange(0, n - 3) =~= astk.subrange(0, n - 3));
            assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 3));
            assert(alpha_stack(vm, p2).last() == alpha_value(vm, a));
            assert(alpha_stack(vm, p2).last() == astk.subrange(0, n - 2)[n - 3]);
            assert(astk.subrange(0, n - 2)[n - 3] == astk[n - 3]);
            assert(alpha_value(vm, a) == astk[n - 3]);
            assert(rest_ptr < vm.snodes.len());

            let (vm1, s1) = model_push_node(vm, rest_ptr, b);
            lemma_push_node(vm, rest_ptr, b);
            assert(alpha_stack(vm1, s1) == astk.subrange(0, n - 3).push(astk[n - 2]));
            let (vm2, s2) = model_push_node(vm1, s1, c);
            assert(s1 < vm1.snodes.len());
            lemma_push_node(vm1, s1, c);
            lemma_alpha_value_tape_eq(vm, vm1, c);
            assert(alpha_value(vm1, c) == astk[n - 1]);
            assert(alpha_stack(vm2, s2)
                == astk.subrange(0, n - 3).push(astk[n - 2]).push(astk[n - 1]));
            let (vm3, s3) = model_push_node(vm2, s2, a);
            assert(s2 < vm2.snodes.len());
            lemma_push_node(vm2, s2, a);
            lemma_alpha_value_tape_eq(vm, vm2, a);
            assert(alpha_value(vm2, a) == astk[n - 3]);
            assert(alpha_stack(vm3, s3)
                == astk.subrange(0, n - 3).push(astk[n - 2]).push(astk[n - 1]).push(astk[n - 3]));
            // cont frame chain vm3 <- vm2 <- vm1 <- vm.
            assert(alpha_cont(vm1, pos1.cont, pos1.cursor) == rest);
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_cont(vm3, pos1.cont, pos1.cursor) == rest);
            let pos2 = ModelVmState { stack: s3, ..pos1 };
            assert(model_exec_word(vm, pos1, ModelWord::Prim(p)) == (ModelStep::Next, vm3, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm3, pos2));
            assert(vm3.cnodes == vm.cnodes);
            assert(wf_pos(vm3, pos2));
            assert(alpha_state(vm3, pos2)
                == (SpecState {
                    stack: astk.subrange(0, n - 3).push(astk[n - 2]).push(astk[n - 1]).push(astk[n - 3]),
                    cont: rest }));
        },
        SpecPrim::Over => {
            assert(n >= 2);
            let s = pos1.stack;
            assert(s != 0) by {
                if s == 0 {
                    assert(alpha_stack(vm, s) =~= Seq::<SpecValue>::empty());
                }
            }
            let p1 = vm.snodes[s as int].parent;
            lemma_alpha_stack_pop1(vm, s);
            assert(alpha_stack(vm, p1) == astk.subrange(0, n - 1));
            assert(alpha_stack(vm, p1).len() == n - 1);
            assert(p1 != 0) by {
                if p1 == 0 {
                    assert(alpha_stack(vm, p1) =~= Seq::<SpecValue>::empty());
                }
            }
            let a = vm.snodes[p1 as int].value; // second
            lemma_alpha_stack_pop1(vm, p1);
            assert(alpha_stack(vm, p1).last() == alpha_value(vm, a));
            assert(alpha_stack(vm, p1).last() == astk.subrange(0, n - 1)[n - 2]);
            assert(astk.subrange(0, n - 1)[n - 2] == astk[n - 2]);
            assert(alpha_value(vm, a) == astk[n - 2]);
            let (vm2, np) = model_push_node(vm, s, a);
            lemma_push_node(vm, s, a);
            assert(alpha_stack(vm2, np) == astk.push(astk[n - 2]));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_word(vm, pos1, ModelWord::Prim(p)) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(wf_pos(vm2, pos2));
            assert(alpha_state(vm2, pos2)
                == (SpecState { stack: astk.push(astk[n - 2]), cont: rest }));
        },
        SpecPrim::Add => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Int && astk[n - 1] is Int) by {
                if !(astk[n - 2] is Int && astk[n - 1] is Int) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let a = astk[n - 2]->Int_0;
            let b = astk[n - 1]->Int_0;
            assert(astk[n - 2] == SpecValue::Int(a));
            assert(astk[n - 1] == SpecValue::Int(b));
            lemma_binop_int(vm, s, astk, n, a, b);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            assert(in_i64(a + b)) by {
                if !in_i64(a + b) { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Overflow)); }
            };
            let (vm2, np) = model_push_node(vm, rp, ModelValue::Int(a + b));
            lemma_push_node(vm, rp, ModelValue::Int(a + b));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_stack(vm2, np) == astk.subrange(0, n - 2).push(SpecValue::Int(a + b)));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Int(a + b)), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Sub => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Int && astk[n - 1] is Int) by {
                if !(astk[n - 2] is Int && astk[n - 1] is Int) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let a = astk[n - 2]->Int_0;
            let b = astk[n - 1]->Int_0;
            assert(astk[n - 2] == SpecValue::Int(a));
            assert(astk[n - 1] == SpecValue::Int(b));
            lemma_binop_int(vm, s, astk, n, a, b);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            assert(in_i64(a - b)) by {
                if !in_i64(a - b) { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Overflow)); }
            };
            let (vm2, np) = model_push_node(vm, rp, ModelValue::Int(a - b));
            lemma_push_node(vm, rp, ModelValue::Int(a - b));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_stack(vm2, np) == astk.subrange(0, n - 2).push(SpecValue::Int(a - b)));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Int(a - b)), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Mul => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Int && astk[n - 1] is Int) by {
                if !(astk[n - 2] is Int && astk[n - 1] is Int) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let a = astk[n - 2]->Int_0;
            let b = astk[n - 1]->Int_0;
            assert(astk[n - 2] == SpecValue::Int(a));
            assert(astk[n - 1] == SpecValue::Int(b));
            lemma_binop_int(vm, s, astk, n, a, b);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            assert(in_i64(a * b)) by {
                if !in_i64(a * b) { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Overflow)); }
            };
            let (vm2, np) = model_push_node(vm, rp, ModelValue::Int(a * b));
            lemma_push_node(vm, rp, ModelValue::Int(a * b));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_stack(vm2, np) == astk.subrange(0, n - 2).push(SpecValue::Int(a * b)));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Int(a * b)), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Div => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Int && astk[n - 1] is Int) by {
                if !(astk[n - 2] is Int && astk[n - 1] is Int) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let a = astk[n - 2]->Int_0;
            let b = astk[n - 1]->Int_0;
            assert(astk[n - 2] == SpecValue::Int(a));
            assert(astk[n - 1] == SpecValue::Int(b));
            lemma_binop_int(vm, s, astk, n, a, b);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            assert(b != 0) by {
                if b == 0 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::DivByZero)); }
            };
            assert(in_i64(trunc_div(a, b))) by {
                if !in_i64(trunc_div(a, b)) { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Overflow)); }
            };
            let (vm2, np) = model_push_node(vm, rp, ModelValue::Int(trunc_div(a, b)));
            lemma_push_node(vm, rp, ModelValue::Int(trunc_div(a, b)));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_stack(vm2, np) == astk.subrange(0, n - 2).push(SpecValue::Int(trunc_div(a, b))));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Int(trunc_div(a, b))), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Mod => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Int && astk[n - 1] is Int) by {
                if !(astk[n - 2] is Int && astk[n - 1] is Int) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let a = astk[n - 2]->Int_0;
            let b = astk[n - 1]->Int_0;
            assert(astk[n - 2] == SpecValue::Int(a));
            assert(astk[n - 1] == SpecValue::Int(b));
            lemma_binop_int(vm, s, astk, n, a, b);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            assert(b != 0) by {
                if b == 0 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::DivByZero)); }
            };
            assert(in_i64(trunc_div(a, b))) by {
                if !in_i64(trunc_div(a, b)) { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Overflow)); }
            };
            let (vm2, np) = model_push_node(vm, rp, ModelValue::Int(trunc_mod(a, b)));
            lemma_push_node(vm, rp, ModelValue::Int(trunc_mod(a, b)));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_stack(vm2, np) == astk.subrange(0, n - 2).push(SpecValue::Int(trunc_mod(a, b))));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Int(trunc_mod(a, b))), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Eq => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Int && astk[n - 1] is Int) by {
                if !(astk[n - 2] is Int && astk[n - 1] is Int) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let a = astk[n - 2]->Int_0;
            let b = astk[n - 1]->Int_0;
            assert(astk[n - 2] == SpecValue::Int(a));
            assert(astk[n - 1] == SpecValue::Int(b));
            lemma_binop_int(vm, s, astk, n, a, b);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            let (vm2, np) = model_push_node(vm, rp, ModelValue::Int(if a == b { 1int } else { 0int }));
            lemma_push_node(vm, rp, ModelValue::Int(if a == b { 1int } else { 0int }));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_stack(vm2, np) == astk.subrange(0, n - 2).push(SpecValue::Int(if a == b { 1int } else { 0int })));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Int(if a == b { 1int } else { 0int })), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Lt => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Int && astk[n - 1] is Int) by {
                if !(astk[n - 2] is Int && astk[n - 1] is Int) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let a = astk[n - 2]->Int_0;
            let b = astk[n - 1]->Int_0;
            assert(astk[n - 2] == SpecValue::Int(a));
            assert(astk[n - 1] == SpecValue::Int(b));
            lemma_binop_int(vm, s, astk, n, a, b);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            let (vm2, np) = model_push_node(vm, rp, ModelValue::Int(if a < b { 1int } else { 0int }));
            lemma_push_node(vm, rp, ModelValue::Int(if a < b { 1int } else { 0int }));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_stack(vm2, np) == astk.subrange(0, n - 2).push(SpecValue::Int(if a < b { 1int } else { 0int })));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Int(if a < b { 1int } else { 0int })), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Xor => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Int && astk[n - 1] is Int) by {
                if !(astk[n - 2] is Int && astk[n - 1] is Int) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let a = astk[n - 2]->Int_0;
            let b = astk[n - 1]->Int_0;
            assert(astk[n - 2] == SpecValue::Int(a));
            assert(astk[n - 1] == SpecValue::Int(b));
            lemma_binop_int(vm, s, astk, n, a, b);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            let (vm2, np) = model_push_node(vm, rp, ModelValue::Int(i64_bitxor(a, b)));
            lemma_push_node(vm, rp, ModelValue::Int(i64_bitxor(a, b)));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_stack(vm2, np) == astk.subrange(0, n - 2).push(SpecValue::Int(i64_bitxor(a, b))));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Int(i64_bitxor(a, b))), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Cons => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 1] is Quote) by {
                if !(astk[n - 1] is Quote) { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch)); }
            };
            let qw = astk[n - 1]->Quote_0;
            assert(astk[n - 1] == SpecValue::Quote(qw));
            lemma_pop2(vm, s);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            let av = vm.snodes[p1 as int].value;   // second = v
            let bv = vm.snodes[s as int].value;    // top = quote
            assert(alpha_value(vm, bv) == SpecValue::Quote(qw));
            assert(alpha_value(vm, av) == astk[n - 2]);
            assert(alpha_stack(vm, rp) == astk.subrange(0, n - 2));
            let qid = bv->Quote_0;
            assert(bv is Quote) by {
                match bv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, bv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(bv == ModelValue::Quote(qid));
            assert(alpha_quote(vm, qid) == qw);
            assert(snode_val_wf(vm, s as int));
            assert(qid.start + qid.len <= vm.tape.len());
            let head = model_value_to_word(av);
            assert(head matches ModelWord::PushQuote(hid) ==> hid.start + hid.len <= vm.tape.len()) by {
                match av {
                    ModelValue::Quote(hid) => { assert(snode_val_wf(vm, p1 as int)); },
                    _ => {},
                }
            };
            assert(head matches ModelWord::Call(k) ==> k < vm.calls.len());
            lemma_model_try_cons(vm, head, qid);
            let (vm_t, new_id) = model_try_cons(vm, head, qid);
            lemma_value_to_word_alpha(vm, av);
            assert(alpha_word_val(vm, head) == value_to_word(astk[n - 2]));
            assert(alpha_quote(vm_t, new_id) == seq![value_to_word(astk[n - 2])] + qw);
            lemma_alpha_stack_tape_frame(vm, vm_t, rp);
            lemma_alpha_cont_tape_frame(vm, vm_t, pos1.cont, pos1.cursor);
            assert(alpha_stack(vm_t, rp) == astk.subrange(0, n - 2));
            assert(alpha_cont(vm_t, pos1.cont, pos1.cursor) == rest);
            let (vm2, np) = model_push_node(vm_t, rp, ModelValue::Quote(new_id));
            lemma_push_node(vm_t, rp, ModelValue::Quote(new_id));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_value(vm_t, ModelValue::Quote(new_id))
                == SpecValue::Quote(seq![value_to_word(astk[n - 2])] + qw));
            assert(alpha_stack(vm2, np)
                == astk.subrange(0, n - 2).push(SpecValue::Quote(seq![value_to_word(astk[n - 2])] + qw)));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Quote(seq![value_to_word(astk[n - 2])] + qw)), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Cat => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Quote && astk[n - 1] is Quote) by {
                if !(astk[n - 2] is Quote && astk[n - 1] is Quote) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let aw = astk[n - 2]->Quote_0;
            let bw = astk[n - 1]->Quote_0;
            assert(astk[n - 2] == SpecValue::Quote(aw));
            assert(astk[n - 1] == SpecValue::Quote(bw));
            lemma_pop2(vm, s);
            let p1 = vm.snodes[s as int].parent;
            let rp = vm.snodes[p1 as int].parent;
            let av = vm.snodes[p1 as int].value;   // second = a
            let bv = vm.snodes[s as int].value;    // top = b
            assert(alpha_value(vm, av) == SpecValue::Quote(aw));
            assert(alpha_value(vm, bv) == SpecValue::Quote(bw));
            assert(alpha_stack(vm, rp) == astk.subrange(0, n - 2));
            let aid = av->Quote_0;
            let bid = bv->Quote_0;
            assert(av is Quote) by {
                match av {
                    ModelValue::Int(x) => { assert(alpha_value(vm, av) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(bv is Quote) by {
                match bv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, bv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(av == ModelValue::Quote(aid));
            assert(bv == ModelValue::Quote(bid));
            assert(alpha_quote(vm, aid) == aw);
            assert(alpha_quote(vm, bid) == bw);
            assert(snode_val_wf(vm, p1 as int));
            assert(aid.start + aid.len <= vm.tape.len());
            assert(snode_val_wf(vm, s as int));
            assert(bid.start + bid.len <= vm.tape.len());
            lemma_model_try_cat(vm, aid, bid);
            let (vm_t, new_id) = model_try_cat(vm, aid, bid);
            assert(alpha_quote(vm_t, new_id) == aw + bw);
            lemma_alpha_stack_tape_frame(vm, vm_t, rp);
            lemma_alpha_cont_tape_frame(vm, vm_t, pos1.cont, pos1.cursor);
            assert(alpha_stack(vm_t, rp) == astk.subrange(0, n - 2));
            assert(alpha_cont(vm_t, pos1.cont, pos1.cursor) == rest);
            let (vm2, np) = model_push_node(vm_t, rp, ModelValue::Quote(new_id));
            lemma_push_node(vm_t, rp, ModelValue::Quote(new_id));
            let pos2 = ModelVmState { stack: np, ..pos1 };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_value(vm_t, ModelValue::Quote(new_id)) == SpecValue::Quote(aw + bw));
            assert(alpha_stack(vm2, np) == astk.subrange(0, n - 2).push(SpecValue::Quote(aw + bw)));
            assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 2).push(SpecValue::Quote(aw + bw)), cont: rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
        },
        SpecPrim::Uncons => {
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 1) by {
                if n < 1 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 1] is Quote) by {
                if !(astk[n - 1] is Quote) { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch)); }
            };
            let qw = astk[n - 1]->Quote_0;
            assert(astk[n - 1] == SpecValue::Quote(qw));
            lemma_pop1(vm, s);
            let rest_ptr = vm.snodes[s as int].parent;
            let bv = vm.snodes[s as int].value;
            assert(alpha_value(vm, bv) == SpecValue::Quote(qw));
            assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 1));
            let qid = bv->Quote_0;
            assert(bv is Quote) by {
                match bv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, bv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(bv == ModelValue::Quote(qid));
            assert(alpha_quote(vm, qid) == qw);
            assert(snode_val_wf(vm, s as int));
            assert(qid.start + qid.len <= vm.tape.len());
            let he = (qid.start + qid.len) as nat;
            lemma_alpha_words_len(vm, qid.start, he);
            assert(qw.len() == qid.len);
            if qid.len == 0 {
                assert(qw.len() == 0);
                let (vm2, np) = model_push_node(vm, rest_ptr, ModelValue::Int(0int));
                lemma_push_node(vm, rest_ptr, ModelValue::Int(0int));
                let pos2 = ModelVmState { stack: np, ..pos1 };
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
                assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
                assert(alpha_stack(vm2, np) == astk.subrange(0, n - 1).push(SpecValue::Int(0int)));
                assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
                assert(alpha_state(vm2, pos2)
                    == SpecState { stack: astk.subrange(0, n - 1).push(SpecValue::Int(0int)), cont: rest });
                assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
            } else {
                assert(qw.len() > 0);
                assert(qid.start < vm.tape.len());
                lemma_alpha_words_head(vm, qid.start, he);
                assert(qw == seq![alpha_word(vm, qid.start)] + alpha_words(vm, (qid.start + 1) as nat, he));
                assert(qw[0] == alpha_word(vm, qid.start));
                lemma_alpha_word_val(vm, qid.start);
                let tail_id = ModelQuoteId { start: qid.start + 1, len: (qid.len - 1) as nat };
                assert(tail_id.start + tail_id.len == he);
                assert(alpha_quote(vm, tail_id) == qw.subrange(1, qw.len() as int)) by {
                    assert(alpha_quote(vm, tail_id) == alpha_words(vm, (qid.start + 1) as nat, he));
                    assert(qw.subrange(1, qw.len() as int) =~= alpha_words(vm, (qid.start + 1) as nat, he));
                };
                match vm.tape[qid.start as int] {
                    ModelWord::PushInt(k) => {
                        assert(alpha_word_val(vm, vm.tape[qid.start as int]) == SpecWord::PushInt(k));
                        assert(qw[0] == SpecWord::PushInt(k));
                        let (vm1, s1) = model_push_node(vm, rest_ptr, ModelValue::Int(k));
                        lemma_push_node(vm, rest_ptr, ModelValue::Int(k));
                        let (vm2, s2) = model_push_node(vm1, s1, ModelValue::Quote(tail_id));
                        lemma_push_node(vm1, s1, ModelValue::Quote(tail_id));
                        lemma_alpha_value_tape_eq(vm, vm1, ModelValue::Quote(tail_id));
                        let (vm3, s3) = model_push_node(vm2, s2, ModelValue::Int(1int));
                        lemma_push_node(vm2, s2, ModelValue::Int(1int));
                        let pos2 = ModelVmState { stack: s3, ..pos1 };
                        assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm3, pos2));
                        assert(model_arena_step(vm, pos) == (ModelStep::Next, vm3, pos2));
                        assert(alpha_stack(vm1, s1) == astk.subrange(0, n - 1).push(SpecValue::Int(k)));
                        assert(alpha_value(vm1, ModelValue::Quote(tail_id)) == SpecValue::Quote(qw.subrange(1, qw.len() as int)));
                        assert(alpha_stack(vm2, s2)
                            == astk.subrange(0, n - 1).push(SpecValue::Int(k)).push(SpecValue::Quote(qw.subrange(1, qw.len() as int))));
                        assert(alpha_stack(vm3, s3)
                            == astk.subrange(0, n - 1).push(SpecValue::Int(k)).push(SpecValue::Quote(qw.subrange(1, qw.len() as int))).push(SpecValue::Int(1int)));
                        assert(alpha_cont(vm1, pos1.cont, pos1.cursor) == rest);
                        assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
                        assert(alpha_cont(vm3, pos1.cont, pos1.cursor) == rest);
                        assert(alpha_state(vm3, pos2)
                            == SpecState { stack: astk.subrange(0, n - 1).push(SpecValue::Int(k)).push(SpecValue::Quote(qw.subrange(1, qw.len() as int))).push(SpecValue::Int(1int)), cont: rest });
                        assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm3, pos2)));
                    },
                    ModelWord::PushQuote(hid) => {
                        assert(alpha_word_val(vm, vm.tape[qid.start as int]) == SpecWord::PushQuote(alpha_quote(vm, hid)));
                        assert(qw[0] == SpecWord::PushQuote(alpha_quote(vm, hid)));
                        let (vm1, s1) = model_push_node(vm, rest_ptr, ModelValue::Quote(hid));
                        lemma_push_node(vm, rest_ptr, ModelValue::Quote(hid));
                        let (vm2, s2) = model_push_node(vm1, s1, ModelValue::Quote(tail_id));
                        lemma_push_node(vm1, s1, ModelValue::Quote(tail_id));
                        lemma_alpha_value_tape_eq(vm, vm1, ModelValue::Quote(tail_id));
                        lemma_alpha_value_tape_eq(vm, vm1, ModelValue::Quote(hid));
                        let (vm3, s3) = model_push_node(vm2, s2, ModelValue::Int(1int));
                        lemma_push_node(vm2, s2, ModelValue::Int(1int));
                        let pos2 = ModelVmState { stack: s3, ..pos1 };
                        assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm3, pos2));
                        assert(model_arena_step(vm, pos) == (ModelStep::Next, vm3, pos2));
                        assert(alpha_stack(vm1, s1) == astk.subrange(0, n - 1).push(SpecValue::Quote(alpha_quote(vm, hid))));
                        assert(alpha_value(vm1, ModelValue::Quote(tail_id)) == SpecValue::Quote(qw.subrange(1, qw.len() as int)));
                        assert(alpha_stack(vm2, s2)
                            == astk.subrange(0, n - 1).push(SpecValue::Quote(alpha_quote(vm, hid))).push(SpecValue::Quote(qw.subrange(1, qw.len() as int))));
                        assert(alpha_stack(vm3, s3)
                            == astk.subrange(0, n - 1).push(SpecValue::Quote(alpha_quote(vm, hid))).push(SpecValue::Quote(qw.subrange(1, qw.len() as int))).push(SpecValue::Int(1int)));
                        assert(alpha_cont(vm1, pos1.cont, pos1.cursor) == rest);
                        assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
                        assert(alpha_cont(vm3, pos1.cont, pos1.cursor) == rest);
                        assert(alpha_state(vm3, pos2)
                            == SpecState { stack: astk.subrange(0, n - 1).push(SpecValue::Quote(alpha_quote(vm, hid))).push(SpecValue::Quote(qw.subrange(1, qw.len() as int))).push(SpecValue::Int(1int)), cont: rest });
                        assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm3, pos2)));
                    },
                    _ => {
                        match vm.tape[qid.start as int] {
                            ModelWord::Prim(pp) => { assert(alpha_word_val(vm, vm.tape[qid.start as int]) == SpecWord::Prim(pp)); },
                            ModelWord::Call(cc) => { assert(alpha_word_val(vm, vm.tape[qid.start as int]) == SpecWord::Call(vm.calls[cc as int])); },
                            _ => {},
                        }
                        assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                        assert(false);
                    },
                }
            }
        },
        SpecPrim::Apply => {
            thm_prim_apply(vm, pos, pos1, p, astk, ac, rest, n);
        },
        SpecPrim::Dip => {
            thm_prim_dip(vm, pos, pos1, p, astk, ac, rest, n);
        },
        SpecPrim::If => {
            thm_prim_if(vm, pos, pos1, p, astk, ac, rest, n);
        },
        SpecPrim::Times => {
            thm_prim_times(vm, pos, pos1, p, astk, ac, rest, n);
        },
        SpecPrim::PrimRec => {
            thm_prim_primrec(vm, pos, pos1, p, astk, ac, rest, n);
        },
        SpecPrim::LinRec => {
            thm_prim_linrec(vm, pos, pos1, p, astk, ac, rest, n);
        },
        SpecPrim::Fold => {
            thm_prim_fold(vm, pos, pos1, p, astk, ac, rest, n);
        },
        _ => {
            assert(false);
        },
    }
}

pub proof fn thm_prim_linrec(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    ac: Seq<SpecWord>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        p == SpecPrim::LinRec,
        astk == alpha_stack(vm, pos.stack),
        ac == alpha_cont(vm, pos.cont, pos.cursor),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        alpha_cont(vm, pos1.cont, pos1.cursor) == rest,
        rest == ac.subrange(1, ac.len() as int),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        !(spec_step_prim(astk, p, rest) is Fault),
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r is Next
            &&& wf(vm2)
            &&& wf_pos(vm2, pos2)
            &&& spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2))
        }),
{
    let s = pos1.stack;
    assert(alpha_stack(vm, s) == astk);
    assert(n >= 4) by {
        if n < 4 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
    };
    assert(astk[n - 4] is Quote && astk[n - 3] is Quote && astk[n - 2] is Quote && astk[n - 1] is Quote) by {
        if !(astk[n - 4] is Quote && astk[n - 3] is Quote && astk[n - 2] is Quote && astk[n - 1] is Quote) {
            assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
        }
    };
    let qpw = astk[n - 4]->Quote_0;
    let qtw = astk[n - 3]->Quote_0;
    let qr1w = astk[n - 2]->Quote_0;
    let qr2w = astk[n - 1]->Quote_0;
    assert(astk[n - 4] == SpecValue::Quote(qpw));
    assert(astk[n - 3] == SpecValue::Quote(qtw));
    assert(astk[n - 2] == SpecValue::Quote(qr1w));
    assert(astk[n - 1] == SpecValue::Quote(qr2w));
    // pop4 chain: s (top=qr2), p1 (qr1), p2 (qt), p3 (qp), rest_ptr.
    let p1 = vm.snodes[s as int].parent;
    lemma_alpha_stack_pop1(vm, s);
    assert(alpha_stack(vm, p1) == astk.subrange(0, n - 1));
    assert(alpha_stack(vm, p1).len() == n - 1);
    assert(p1 != 0) by { if p1 == 0 { assert(alpha_stack(vm, p1) =~= Seq::<SpecValue>::empty()); } }
    let p2 = vm.snodes[p1 as int].parent;
    lemma_alpha_stack_pop1(vm, p1);
    assert(alpha_stack(vm, p2) == alpha_stack(vm, p1).subrange(0, (n - 1) - 1));
    assert(alpha_stack(vm, p1).subrange(0, n - 2) =~= astk.subrange(0, n - 2));
    assert(alpha_stack(vm, p2) == astk.subrange(0, n - 2));
    assert(alpha_stack(vm, p2).len() == n - 2);
    assert(p2 != 0) by { if p2 == 0 { assert(alpha_stack(vm, p2) =~= Seq::<SpecValue>::empty()); } }
    let p3 = vm.snodes[p2 as int].parent;
    lemma_alpha_stack_pop1(vm, p2);
    assert(alpha_stack(vm, p3) == alpha_stack(vm, p2).subrange(0, (n - 2) - 1));
    assert(alpha_stack(vm, p2).subrange(0, n - 3) =~= astk.subrange(0, n - 3));
    assert(alpha_stack(vm, p3) == astk.subrange(0, n - 3));
    assert(alpha_stack(vm, p3).len() == n - 3);
    assert(p3 != 0) by { if p3 == 0 { assert(alpha_stack(vm, p3) =~= Seq::<SpecValue>::empty()); } }
    let rest_ptr = vm.snodes[p3 as int].parent;
    lemma_alpha_stack_pop1(vm, p3);
    assert(alpha_stack(vm, rest_ptr) == alpha_stack(vm, p3).subrange(0, (n - 3) - 1));
    assert(alpha_stack(vm, p3).subrange(0, n - 4) =~= astk.subrange(0, n - 4));
    assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 4));
    let qr2v = vm.snodes[s as int].value;   // top = qr2
    let qr1v = vm.snodes[p1 as int].value;  // qr1
    let qtv = vm.snodes[p2 as int].value;   // qt
    let qpv = vm.snodes[p3 as int].value;   // qp
    assert(astk.last() == alpha_value(vm, qr2v));
    assert(astk.last() == astk[n - 1]);
    assert(alpha_value(vm, qr2v) == SpecValue::Quote(qr2w));
    assert(alpha_stack(vm, p1).last() == alpha_value(vm, qr1v));
    assert(alpha_stack(vm, p1).last() == astk.subrange(0, n - 1)[n - 2]);
    assert(astk.subrange(0, n - 1)[n - 2] == astk[n - 2]);
    assert(alpha_value(vm, qr1v) == SpecValue::Quote(qr1w));
    assert(alpha_stack(vm, p2).last() == alpha_value(vm, qtv));
    assert(alpha_stack(vm, p2).last() == astk.subrange(0, n - 2)[n - 3]);
    assert(astk.subrange(0, n - 2)[n - 3] == astk[n - 3]);
    assert(alpha_value(vm, qtv) == SpecValue::Quote(qtw));
    assert(alpha_stack(vm, p3).last() == alpha_value(vm, qpv));
    assert(alpha_stack(vm, p3).last() == astk.subrange(0, n - 3)[n - 4]);
    assert(astk.subrange(0, n - 3)[n - 4] == astk[n - 4]);
    assert(alpha_value(vm, qpv) == SpecValue::Quote(qpw));
    let qp = qpv->Quote_0;
    let qt = qtv->Quote_0;
    let qr1 = qr1v->Quote_0;
    let qr2 = qr2v->Quote_0;
    assert(qpv is Quote) by { match qpv { ModelValue::Int(x) => { assert(alpha_value(vm, qpv) == SpecValue::Int(x)); }, ModelValue::Quote(id) => {}, } };
    assert(qtv is Quote) by { match qtv { ModelValue::Int(x) => { assert(alpha_value(vm, qtv) == SpecValue::Int(x)); }, ModelValue::Quote(id) => {}, } };
    assert(qr1v is Quote) by { match qr1v { ModelValue::Int(x) => { assert(alpha_value(vm, qr1v) == SpecValue::Int(x)); }, ModelValue::Quote(id) => {}, } };
    assert(qr2v is Quote) by { match qr2v { ModelValue::Int(x) => { assert(alpha_value(vm, qr2v) == SpecValue::Int(x)); }, ModelValue::Quote(id) => {}, } };
    assert(qpv == ModelValue::Quote(qp));
    assert(qtv == ModelValue::Quote(qt));
    assert(qr1v == ModelValue::Quote(qr1));
    assert(qr2v == ModelValue::Quote(qr2));
    assert(alpha_quote(vm, qp) == qpw);
    assert(alpha_quote(vm, qt) == qtw);
    assert(alpha_quote(vm, qr1) == qr1w);
    assert(alpha_quote(vm, qr2) == qr2w);
    assert(snode_val_wf(vm, p3 as int));
    assert(qp.start + qp.len <= vm.tape.len());
    assert(snode_val_wf(vm, p2 as int));
    assert(qt.start + qt.len <= vm.tape.len());
    assert(snode_val_wf(vm, p1 as int));
    assert(qr1.start + qr1.len <= vm.tape.len());
    assert(snode_val_wf(vm, s as int));
    assert(qr2.start + qr2.len <= vm.tape.len());
    let pos_mid = ModelVmState { stack: rest_ptr, ..pos1 };
    assert(wf_pos(vm, pos_mid));
    assert(alpha_cont(vm, pos_mid.cont, pos_mid.cursor) == rest);
    let ol = vm.tape.len();
    let aA = vm.tape.subrange(qr1.start as int, (qr1.start + qr1.len) as int);
    let bB = seq![
        ModelWord::PushQuote(qp),
        ModelWord::PushQuote(qt),
        ModelWord::PushQuote(qr1),
        ModelWord::PushQuote(qr2),
        ModelWord::Prim(SpecPrim::LinRec)
    ];
    let cC = vm.tape.subrange(qr2.start as int, (qr2.start + qr2.len) as int);
    let else_seg = aA + bB + cC;
    assert(aA.len() == qr1.len);
    assert(cC.len() == qr2.len);
    assert(bB.len() == 5);
    assert(else_seg.len() == qr1.len + 5 + qr2.len);
    // per-region index facts on else_seg.
    assert forall|k: int| 0 <= k < qr1.len implies
        #[trigger] else_seg[k] == vm.tape[(qr1.start + k) as int] by {
        assert(else_seg[k] == (aA + bB)[k]);
        assert((aA + bB)[k] == aA[k]);
    };
    assert forall|j: int| 0 <= j < 5 implies
        #[trigger] else_seg[(qr1.len + j) as int] == bB[j] by {
        assert(else_seg[(qr1.len + j) as int] == (aA + bB)[(qr1.len + j) as int]);
        assert((aA + bB)[(qr1.len + j) as int] == bB[j]);
    };
    assert forall|k: int| 0 <= k < qr2.len implies
        #[trigger] else_seg[(qr1.len + 5 + k) as int] == vm.tape[(qr2.start + k) as int] by {
        assert(else_seg[(qr1.len + 5 + k) as int] == cC[k]);
        assert(cC[k] == vm.tape[(qr2.start + k) as int]);
    };
    // bB is intern-wf against vm.
    assert forall|j: int| 0 <= j < bB.len() implies word_intern_wf(vm, #[trigger] bB[j]) by {
        assert(bB[0] == ModelWord::PushQuote(qp));
        assert(bB[1] == ModelWord::PushQuote(qt));
        assert(bB[2] == ModelWord::PushQuote(qr1));
        assert(bB[3] == ModelWord::PushQuote(qr2));
        assert(bB[4] == ModelWord::Prim(SpecPrim::LinRec));
        if j == 0 {
        } else if j == 1 {
        } else if j == 2 {
        } else if j == 3 {
        } else {
            assert(j == 4);
        }
    };
    // else_seg is intern-wf against vm.
    assert forall|i: int| 0 <= i < else_seg.len() implies word_intern_wf(vm, #[trigger] else_seg[i]) by {
        if i < qr1.len {
            assert(else_seg[i] == vm.tape[(qr1.start + i) as int]);
            assert(wf_tape_word(vm, (qr1.start + i) as int));
            match vm.tape[(qr1.start + i) as int] {
                ModelWord::PushQuote(hid) => { assert(hid.start + hid.len <= qr1.start + i); },
                ModelWord::Call(cc) => { assert(cc < vm.calls.len()); },
                _ => {},
            }
        } else if i < qr1.len + 5 {
            assert(else_seg[i] == bB[i - qr1.len]);
            assert(word_intern_wf(vm, bB[i - qr1.len]));
        } else {
            let k = i - qr1.len - 5;
            assert(else_seg[i] == vm.tape[(qr2.start + k) as int]);
            assert(wf_tape_word(vm, (qr2.start + k) as int));
            match vm.tape[(qr2.start + k) as int] {
                ModelWord::PushQuote(hid) => { assert(hid.start + hid.len <= qr2.start + k); },
                ModelWord::Call(cc) => { assert(cc < vm.calls.len()); },
                _ => {},
            }
        }
    };
    lemma_model_try_alloc(vm, else_seg);
    let (vm_e, else_id) = model_try_alloc(vm, else_seg);
    assert(else_id.start == ol && else_id.len == qr1.len + 5 + qr2.len);
    // copy correspondences vm -> vm_e for the two tape regions.
    assert forall|k: int| 0 <= k < qr1.len implies
        #[trigger] vm_e.tape[(ol + k) as int] == vm.tape[(qr1.start + k) as int] by {
        assert(vm_e.tape[(ol + k) as int] == else_seg[k]);
    };
    assert forall|k: int| 0 <= k < qr2.len implies
        #[trigger] vm_e.tape[(ol + qr1.len + 5 + k) as int] == vm.tape[(qr2.start + k) as int] by {
        assert(vm_e.tape[(ol + qr1.len + 5 + k) as int] == else_seg[(qr1.len + 5 + k) as int]);
    };
    assert forall|j: int| 0 <= j < 5 implies
        #[trigger] vm_e.tape[(ol + qr1.len + j) as int] == bB[j] by {
        assert(vm_e.tape[(ol + qr1.len + j) as int] == else_seg[(qr1.len + j) as int]);
    };
    // else_q α by splitting the interned region into qr1-copy | 5-literal | qr2-copy.
    lemma_alpha_words_copy(vm, vm_e, qr1.start, qr1.start + qr1.len, ol);
    assert(alpha_words(vm_e, ol, (ol + qr1.len) as nat) == qr1w);
    lemma_alpha_words_intern_seq(vm, vm_e, bB, (ol + qr1.len) as nat);
    let bB_alpha = seq![
        SpecWord::PushQuote(qpw),
        SpecWord::PushQuote(qtw),
        SpecWord::PushQuote(qr1w),
        SpecWord::PushQuote(qr2w),
        SpecWord::Prim(SpecPrim::LinRec)
    ];
    assert(alpha_words(vm_e, (ol + qr1.len) as nat, (ol + qr1.len + 5) as nat) == bB_alpha) by {
        assert(Seq::new(bB.len(), |j: int| alpha_word_val(vm, bB[j])) =~= bB_alpha);
    };
    lemma_alpha_words_copy(vm, vm_e, qr2.start, qr2.start + qr2.len, (ol + qr1.len + 5) as nat);
    assert(alpha_words(vm_e, (ol + qr1.len + 5) as nat, (ol + qr1.len + 5 + qr2.len) as nat) == qr2w);
    lemma_alpha_words_split(vm_e, ol, (ol + qr1.len) as nat, (ol + qr1.len + 5 + qr2.len) as nat);
    lemma_alpha_words_split(vm_e, (ol + qr1.len) as nat, (ol + qr1.len + 5) as nat, (ol + qr1.len + 5 + qr2.len) as nat);
    let else_q = qr1w + bB_alpha + qr2w;
    assert(alpha_quote(vm_e, else_id) == else_q) by {
        assert(alpha_quote(vm_e, else_id)
            == alpha_words(vm_e, ol, (ol + qr1.len + 5 + qr2.len) as nat));
        assert(qr1w + (bB_alpha + qr2w) =~= qr1w + bB_alpha + qr2w);
    };
    // seg = [PushQuote(qt), PushQuote(else_id), If], interned in vm_e.
    let seg = seq![
        ModelWord::PushQuote(qt),
        ModelWord::PushQuote(else_id),
        ModelWord::Prim(SpecPrim::If)
    ];
    assert(qt.start + qt.len <= vm_e.tape.len());
    assert(else_id.start + else_id.len == vm_e.tape.len());
    assert forall|i: int| 0 <= i < seg.len() implies word_intern_wf(vm_e, #[trigger] seg[i]) by {
        assert(seg[0] == ModelWord::PushQuote(qt));
        assert(seg[1] == ModelWord::PushQuote(else_id));
        assert(seg[2] == ModelWord::Prim(SpecPrim::If));
    };
    lemma_model_try_alloc(vm_e, seg);
    let (vm_a, seg_id) = model_try_alloc(vm_e, seg);
    lemma_alpha_words_intern_seq(vm_e, vm_a, seg, vm_e.tape.len());
    let seg_alpha = seq![
        SpecWord::PushQuote(qtw),
        SpecWord::PushQuote(else_q),
        SpecWord::Prim(SpecPrim::If)
    ];
    assert(alpha_word_val(vm_e, ModelWord::PushQuote(qt)) == SpecWord::PushQuote(qtw)) by {
        lemma_alpha_words_frame(vm, vm_e, qt.start, qt.start + qt.len);
    };
    assert(alpha_word_val(vm_e, ModelWord::PushQuote(else_id)) == SpecWord::PushQuote(else_q));
    assert(alpha_words(vm_a, vm_e.tape.len(), (vm_e.tape.len() + 3) as nat) == seg_alpha) by {
        assert(Seq::new(seg.len(), |i: int| alpha_word_val(vm_e, seg[i])) =~= seg_alpha);
    };
    assert(seg_id.start == vm_e.tape.len() && seg_id.len == 3);
    assert(alpha_quote(vm_a, seg_id) == seg_alpha);
    // frame stack + cont to vm_a.
    assert(wf_pos(vm_a, pos_mid));
    lemma_alpha_stack_tape_frame(vm, vm_a, rest_ptr);
    lemma_alpha_cont_tape_frame(vm, vm_a, pos1.cont, pos1.cursor);
    assert(alpha_stack(vm_a, rest_ptr) == astk.subrange(0, n - 4));
    assert(alpha_cont(vm_a, pos_mid.cont, pos_mid.cursor) == rest);
    // prepend seg_id on vm_a.
    assert(seg_id.start + seg_id.len <= vm_a.tape.len());
    lemma_alpha_cont_prepend(vm_a, pos_mid, seg_id);
    let (vm_b, pos_b) = model_prepend(vm_a, pos_mid, seg_id);
    assert(alpha_cont(vm_b, pos_b.cont, pos_b.cursor) == seg_alpha + rest);
    assert(alpha_stack(vm_b, pos_b.stack) == astk.subrange(0, n - 4));
    // prepend qp on vm_b.
    assert(qp.start + qp.len <= vm_b.tape.len());
    assert(wf_pos(vm_b, pos_b));
    lemma_alpha_cont_prepend(vm_b, pos_b, qp);
    let (vm_c, pos_c) = model_prepend(vm_b, pos_b, qp);
    assert(alpha_quote(vm_b, qp) == qpw) by {
        lemma_alpha_words_frame(vm, vm_b, qp.start, qp.start + qp.len);
    };
    assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm_c, pos_c));
    assert(model_arena_step(vm, pos) == (ModelStep::Next, vm_c, pos_c));
    assert(alpha_cont(vm_c, pos_c.cont, pos_c.cursor) == qpw + (seg_alpha + rest));
    assert(qpw + (seg_alpha + rest) =~= (qpw + seg_alpha) + rest);
    assert(alpha_stack(vm_c, pos_c.stack) == astk.subrange(0, n - 4));
    assert(alpha_state(vm_c, pos_c)
        == SpecState { stack: astk.subrange(0, n - 4), cont: (qpw + seg_alpha) + rest });
    assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm_c, pos_c)));
}

pub proof fn thm_prim_fold(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    ac: Seq<SpecWord>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        p == SpecPrim::Fold,
        astk == alpha_stack(vm, pos.stack),
        ac == alpha_cont(vm, pos.cont, pos.cursor),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        alpha_cont(vm, pos1.cont, pos1.cursor) == rest,
        rest == ac.subrange(1, ac.len() as int),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        !(spec_step_prim(astk, p, rest) is Fault),
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r is Next
            &&& wf(vm2)
            &&& wf_pos(vm2, pos2)
            &&& spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2))
        }),
{
    let s = pos1.stack;
    assert(alpha_stack(vm, s) == astk);
    assert(n >= 3) by {
        if n < 3 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
    };
    assert(astk[n - 3] is Quote && astk[n - 1] is Quote) by {
        if !(astk[n - 3] is Quote && astk[n - 1] is Quote) {
            assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
        }
    };
    let qsw = astk[n - 3]->Quote_0;
    let qcw = astk[n - 1]->Quote_0;
    assert(astk[n - 3] == SpecValue::Quote(qsw));
    assert(astk[n - 1] == SpecValue::Quote(qcw));
    // pop3 chain: s (top=combine), p1 (init), p2 (seq), rest_ptr.
    let p1 = vm.snodes[s as int].parent;
    lemma_alpha_stack_pop1(vm, s);
    assert(alpha_stack(vm, p1) == astk.subrange(0, n - 1));
    assert(alpha_stack(vm, p1).len() == n - 1);
    assert(p1 != 0) by {
        if p1 == 0 { assert(alpha_stack(vm, p1) =~= Seq::<SpecValue>::empty()); }
    }
    let p2 = vm.snodes[p1 as int].parent;
    lemma_alpha_stack_pop1(vm, p1);
    assert(alpha_stack(vm, p2) == alpha_stack(vm, p1).subrange(0, (n - 1) - 1));
    assert(alpha_stack(vm, p1).subrange(0, n - 2) =~= astk.subrange(0, n - 2));
    assert(alpha_stack(vm, p2) == astk.subrange(0, n - 2));
    assert(alpha_stack(vm, p2).len() == n - 2);
    assert(p2 != 0) by {
        if p2 == 0 { assert(alpha_stack(vm, p2) =~= Seq::<SpecValue>::empty()); }
    }
    let rest_ptr = vm.snodes[p2 as int].parent;
    lemma_alpha_stack_pop1(vm, p2);
    assert(alpha_stack(vm, rest_ptr) == alpha_stack(vm, p2).subrange(0, (n - 2) - 1));
    assert(alpha_stack(vm, p2).subrange(0, n - 3) =~= astk.subrange(0, n - 3));
    assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 3));
    let cbv = vm.snodes[s as int].value;    // top = combine
    let initv = vm.snodes[p1 as int].value; // second = init
    let qsv = vm.snodes[p2 as int].value;   // third = seq
    assert(astk.last() == alpha_value(vm, cbv));
    assert(astk.last() == astk[n - 1]);
    assert(alpha_value(vm, cbv) == SpecValue::Quote(qcw));
    assert(alpha_stack(vm, p1).last() == alpha_value(vm, initv));
    assert(alpha_stack(vm, p1).last() == astk.subrange(0, n - 1)[n - 2]);
    assert(astk.subrange(0, n - 1)[n - 2] == astk[n - 2]);
    assert(alpha_value(vm, initv) == astk[n - 2]);
    assert(alpha_stack(vm, p2).last() == alpha_value(vm, qsv));
    assert(alpha_stack(vm, p2).last() == astk.subrange(0, n - 2)[n - 3]);
    assert(astk.subrange(0, n - 2)[n - 3] == astk[n - 3]);
    assert(alpha_value(vm, qsv) == SpecValue::Quote(qsw));
    let qc = cbv->Quote_0;
    let qs = qsv->Quote_0;
    assert(cbv is Quote) by {
        match cbv {
            ModelValue::Int(x) => { assert(alpha_value(vm, cbv) == SpecValue::Int(x)); },
            ModelValue::Quote(id) => {},
        }
    };
    assert(qsv is Quote) by {
        match qsv {
            ModelValue::Int(x) => { assert(alpha_value(vm, qsv) == SpecValue::Int(x)); },
            ModelValue::Quote(id) => {},
        }
    };
    assert(cbv == ModelValue::Quote(qc));
    assert(qsv == ModelValue::Quote(qs));
    assert(alpha_quote(vm, qc) == qcw);
    assert(alpha_quote(vm, qs) == qsw);
    assert(snode_val_wf(vm, s as int));
    assert(qc.start + qc.len <= vm.tape.len());
    assert(snode_val_wf(vm, p2 as int));
    assert(qs.start + qs.len <= vm.tape.len());
    let pos_mid = ModelVmState { stack: rest_ptr, ..pos1 };
    assert(wf_pos(vm, pos_mid));
    assert(alpha_cont(vm, pos_mid.cont, pos_mid.cursor) == rest);
    let qse = (qs.start + qs.len) as nat;
    lemma_alpha_words_len(vm, qs.start, qse);
    assert(qsw.len() == qs.len);
    if qs.len == 0 {
        assert(qsw.len() == 0);
        let (vm2, np) = model_push_node(vm, rest_ptr, initv);
        lemma_push_node(vm, rest_ptr, initv);
        let pos2 = ModelVmState { stack: np, ..pos1 };
        assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
        assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
        assert(alpha_stack(vm2, np) == astk.subrange(0, n - 3).push(astk[n - 2]));
        assert(alpha_cont(vm2, pos1.cont, pos1.cursor) == rest);
        assert(alpha_state(vm2, pos2)
            == SpecState { stack: astk.subrange(0, n - 3).push(astk[n - 2]), cont: rest });
        assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
    } else {
        assert(qs.start < vm.tape.len());
        let head = vm.tape[qs.start as int];
        assert(wf_tape_word(vm, qs.start as int));
        lemma_alpha_words_head(vm, qs.start, qse);
        lemma_alpha_word_val(vm, qs.start);
        assert(qsw[0] == alpha_word_val(vm, head));
        assert(head is PushInt || head is PushQuote) by {
            if !(head is PushInt || head is PushQuote) {
                match head {
                    ModelWord::Prim(pp) => { assert(alpha_word_val(vm, head) == SpecWord::Prim(pp)); },
                    ModelWord::Call(cc) => { assert(alpha_word_val(vm, head) == SpecWord::Call(vm.calls[cc as int])); },
                    _ => {},
                }
                assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
            }
        };
        let tail = ModelQuoteId { start: qs.start + 1, len: (qs.len - 1) as nat };
        assert(tail.start + tail.len == qse);
        assert(alpha_quote(vm, tail) == qsw.subrange(1, qsw.len() as int)) by {
            assert(alpha_quote(vm, tail) == alpha_words(vm, (qs.start + 1) as nat, qse));
            assert(qsw.subrange(1, qsw.len() as int) =~= alpha_words(vm, (qs.start + 1) as nat, qse));
        };
        let seg_c = seq![ModelWord::PushQuote(qc), ModelWord::Prim(SpecPrim::Fold)];
        let seg_a = seq![ModelWord::PushQuote(tail), model_value_to_word(initv), head];
        // seg_c intern-wf against vm.
        assert forall|i: int| 0 <= i < seg_c.len() implies word_intern_wf(vm, #[trigger] seg_c[i]) by {
            assert(seg_c[0] == ModelWord::PushQuote(qc));
            assert(seg_c[1] == ModelWord::Prim(SpecPrim::Fold));
        };
        lemma_model_try_alloc(vm, seg_c);
        let (vm_c1, seg_c_id) = model_try_alloc(vm, seg_c);
        // seg_a intern-wf against vm_c1 (tape only grew).
        assert(word_intern_wf(vm, model_value_to_word(initv))) by {
            match initv {
                ModelValue::Quote(iid) => { assert(snode_val_wf(vm, p1 as int)); },
                _ => {},
            }
        };
        assert forall|i: int| 0 <= i < seg_a.len() implies word_intern_wf(vm_c1, #[trigger] seg_a[i]) by {
            assert(seg_a[0] == ModelWord::PushQuote(tail));
            assert(seg_a[1] == model_value_to_word(initv));
            assert(seg_a[2] == head);
            assert(vm.tape.len() <= vm_c1.tape.len());
            match head {
                ModelWord::PushQuote(hid) => { assert(hid.start + hid.len <= qs.start); },
                ModelWord::Call(cc) => { assert(cc < vm.calls.len()); },
                _ => {},
            }
            match model_value_to_word(initv) {
                ModelWord::PushQuote(iid) => { assert(word_intern_wf(vm, model_value_to_word(initv))); },
                _ => {},
            }
        };
        lemma_model_try_alloc(vm_c1, seg_a);
        let (vm_a1, seg_a_id) = model_try_alloc(vm_c1, seg_a);
        // seg_c α (referenced against vm).
        lemma_alpha_words_intern_seq(vm, vm_c1, seg_c, vm.tape.len());
        let seg_c_alpha = seq![SpecWord::PushQuote(qcw), SpecWord::Prim(SpecPrim::Fold)];
        assert(alpha_words(vm_c1, vm.tape.len(), (vm.tape.len() + 2) as nat) == seg_c_alpha) by {
            assert(Seq::new(seg_c.len(), |i: int| alpha_word_val(vm, seg_c[i])) =~= seg_c_alpha);
        };
        assert(seg_c_id.start == vm.tape.len() && seg_c_id.len == 2);
        assert(alpha_quote(vm_c1, seg_c_id) == seg_c_alpha);
        // seg_a α (referenced against vm, interned at vm_c1.tape.len()).
        assert forall|i: int| 0 <= i < seg_a.len() implies word_intern_wf(vm, #[trigger] seg_a[i]) by {
            assert(seg_a[0] == ModelWord::PushQuote(tail));
            assert(seg_a[1] == model_value_to_word(initv));
            assert(seg_a[2] == head);
            match head {
                ModelWord::PushQuote(hid) => { assert(hid.start + hid.len <= qs.start); },
                ModelWord::Call(cc) => { assert(cc < vm.calls.len()); },
                _ => {},
            }
        };
        lemma_value_to_word_alpha(vm, initv);
        lemma_alpha_words_intern_seq(vm, vm_a1, seg_a, vm_c1.tape.len());
        let seg_a_alpha = seq![
            SpecWord::PushQuote(qsw.subrange(1, qsw.len() as int)),
            value_to_word(astk[n - 2]),
            qsw[0]
        ];
        assert(alpha_words(vm_a1, vm_c1.tape.len(), (vm_c1.tape.len() + 3) as nat) == seg_a_alpha) by {
            assert(Seq::new(seg_a.len(), |i: int| alpha_word_val(vm, seg_a[i])) =~= seg_a_alpha);
        };
        assert(seg_a_id.start == vm_c1.tape.len() && seg_a_id.len == 3);
        assert(alpha_quote(vm_a1, seg_a_id) == seg_a_alpha);
        // frame stack + cont to vm_a1.
        assert(wf_pos(vm_a1, pos_mid));
        lemma_alpha_stack_tape_frame(vm, vm_a1, rest_ptr);
        lemma_alpha_cont_tape_frame(vm, vm_a1, pos1.cont, pos1.cursor);
        assert(alpha_stack(vm_a1, rest_ptr) == astk.subrange(0, n - 3));
        assert(alpha_cont(vm_a1, pos_mid.cont, pos_mid.cursor) == rest);
        // prepend seg_c_id on vm_a1.
        assert(alpha_quote(vm_a1, seg_c_id) == seg_c_alpha) by {
            lemma_alpha_words_frame(vm_c1, vm_a1, seg_c_id.start, seg_c_id.start + seg_c_id.len);
        };
        assert(seg_c_id.start + seg_c_id.len <= vm_a1.tape.len());
        lemma_alpha_cont_prepend(vm_a1, pos_mid, seg_c_id);
        let (vm_1, pos_1) = model_prepend(vm_a1, pos_mid, seg_c_id);
        assert(alpha_cont(vm_1, pos_1.cont, pos_1.cursor) == seg_c_alpha + rest);
        assert(alpha_stack(vm_1, pos_1.stack) == astk.subrange(0, n - 3));
        // prepend qc on vm_1.
        assert(qc.start + qc.len <= vm_1.tape.len());
        assert(wf_pos(vm_1, pos_1));
        lemma_alpha_cont_prepend(vm_1, pos_1, qc);
        let (vm_2, pos_2) = model_prepend(vm_1, pos_1, qc);
        assert(alpha_quote(vm_1, qc) == qcw) by {
            lemma_alpha_words_frame(vm, vm_1, qc.start, qc.start + qc.len);
        };
        assert(alpha_cont(vm_2, pos_2.cont, pos_2.cursor) == qcw + (seg_c_alpha + rest));
        assert(alpha_stack(vm_2, pos_2.stack) == astk.subrange(0, n - 3));
        // prepend seg_a_id on vm_2.
        assert(seg_a_id.start + seg_a_id.len <= vm_2.tape.len());
        assert(wf_pos(vm_2, pos_2));
        lemma_alpha_cont_prepend(vm_2, pos_2, seg_a_id);
        let (vm_3, pos_3) = model_prepend(vm_2, pos_2, seg_a_id);
        assert(alpha_quote(vm_2, seg_a_id) == seg_a_alpha) by {
            lemma_alpha_words_frame(vm_a1, vm_2, seg_a_id.start, seg_a_id.start + seg_a_id.len);
        };
        assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm_3, pos_3));
        assert(model_arena_step(vm, pos) == (ModelStep::Next, vm_3, pos_3));
        assert(alpha_cont(vm_3, pos_3.cont, pos_3.cursor)
            == seg_a_alpha + (qcw + (seg_c_alpha + rest)));
        assert(alpha_stack(vm_3, pos_3.stack) == astk.subrange(0, n - 3));
        // ghost recur = seg_a_alpha + qcw + seg_c_alpha ; cont = recur + rest.
        assert(seg_a_alpha + (qcw + (seg_c_alpha + rest))
            =~= (seg_a_alpha + qcw + seg_c_alpha) + rest);
        assert(alpha_state(vm_3, pos_3)
            == SpecState {
                stack: astk.subrange(0, n - 3),
                cont: (seg_a_alpha + qcw + seg_c_alpha) + rest });
        assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm_3, pos_3)));
    }
}

pub proof fn thm_prim_times(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    ac: Seq<SpecWord>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        p == SpecPrim::Times,
        astk == alpha_stack(vm, pos.stack),
        ac == alpha_cont(vm, pos.cont, pos.cursor),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        alpha_cont(vm, pos1.cont, pos1.cursor) == rest,
        rest == ac.subrange(1, ac.len() as int),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        !(spec_step_prim(astk, p, rest) is Fault),
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r is Next
            &&& wf(vm2)
            &&& wf_pos(vm2, pos2)
            &&& spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2))
        }),
{
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 2] is Int && astk[n - 1] is Quote) by {
                if !(astk[n - 2] is Int && astk[n - 1] is Quote) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let k = astk[n - 2]->Int_0;
            let qw = astk[n - 1]->Quote_0;
            assert(astk[n - 2] == SpecValue::Int(k));
            assert(astk[n - 1] == SpecValue::Quote(qw));
            lemma_pop2(vm, s);
            let p1 = vm.snodes[s as int].parent;
            let rest_ptr = vm.snodes[p1 as int].parent;
            let kv = vm.snodes[p1 as int].value;   // second = n
            let bv = vm.snodes[s as int].value;    // top = q
            assert(alpha_value(vm, bv) == SpecValue::Quote(qw));
            assert(alpha_value(vm, kv) == SpecValue::Int(k));
            assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 2));
            let qid = bv->Quote_0;
            assert(bv is Quote) by {
                match bv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, bv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(bv == ModelValue::Quote(qid));
            assert(alpha_quote(vm, qid) == qw);
            assert(kv == ModelValue::Int(k)) by {
                match kv {
                    ModelValue::Int(x) => {},
                    ModelValue::Quote(id) => { assert(alpha_value(vm, kv) == SpecValue::Quote(alpha_quote(vm, id))); },
                }
            };
            assert(snode_val_wf(vm, s as int));
            assert(qid.start + qid.len <= vm.tape.len());
            let pos_mid = ModelVmState { stack: rest_ptr, ..pos1 };
            assert(wf_pos(vm, pos_mid));
            assert(alpha_cont(vm, pos_mid.cont, pos_mid.cursor) == rest);
            if k <= 0 {
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm, pos_mid));
                assert(model_arena_step(vm, pos) == (ModelStep::Next, vm, pos_mid));
                assert(alpha_stack(vm, pos_mid.stack) == astk.subrange(0, n - 2));
                assert(alpha_state(vm, pos_mid)
                    == SpecState { stack: astk.subrange(0, n - 2), cont: rest });
                assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm, pos_mid)));
            } else {
                let setup = seq![
                    ModelWord::PushInt(k - 1),
                    ModelWord::PushQuote(qid),
                    ModelWord::Prim(SpecPrim::Times)
                ];
                assert forall|i: int| 0 <= i < setup.len() implies word_intern_wf(vm, #[trigger] setup[i]) by {
                    assert(setup[0] == ModelWord::PushInt(k - 1));
                    assert(setup[1] == ModelWord::PushQuote(qid));
                    assert(setup[2] == ModelWord::Prim(SpecPrim::Times));
                };
                lemma_model_try_alloc(vm, setup);
                let (vm_a, seg_id) = model_try_alloc(vm, setup);
                lemma_alpha_words_intern_seq(vm, vm_a, setup, vm.tape.len());
                let mid = seq![
                    SpecWord::PushInt(k - 1),
                    SpecWord::PushQuote(qw),
                    SpecWord::Prim(SpecPrim::Times)
                ];
                assert(alpha_quote(vm_a, seg_id) == mid) by {
                    assert(alpha_quote(vm_a, seg_id)
                        == alpha_words(vm_a, seg_id.start, (seg_id.start + seg_id.len) as nat));
                    assert(Seq::new(setup.len(), |i: int| alpha_word_val(vm, setup[i])) =~= mid);
                };
                assert(wf_pos(vm_a, pos_mid));
                lemma_alpha_stack_tape_frame(vm, vm_a, rest_ptr);
                lemma_alpha_cont_tape_frame(vm, vm_a, pos1.cont, pos1.cursor);
                assert(alpha_stack(vm_a, rest_ptr) == astk.subrange(0, n - 2));
                assert(alpha_cont(vm_a, pos_mid.cont, pos_mid.cursor) == rest);
                lemma_alpha_cont_prepend(vm_a, pos_mid, seg_id);
                let (vm_b, pos_b) = model_prepend(vm_a, pos_mid, seg_id);
                assert(alpha_cont(vm_b, pos_b.cont, pos_b.cursor) == mid + rest);
                assert(alpha_stack(vm_b, pos_b.stack) == astk.subrange(0, n - 2));
                assert(qid.start + qid.len <= vm_b.tape.len());
                assert(wf_pos(vm_b, pos_b));
                lemma_alpha_cont_prepend(vm_b, pos_b, qid);
                let (vm_c, pos_c) = model_prepend(vm_b, pos_b, qid);
                assert(alpha_quote(vm_b, qid) == qw) by {
                    lemma_alpha_words_frame(vm, vm_b, qid.start, qid.start + qid.len);
                };
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm_c, pos_c));
                assert(model_arena_step(vm, pos) == (ModelStep::Next, vm_c, pos_c));
                assert(alpha_cont(vm_c, pos_c.cont, pos_c.cursor) == qw + (mid + rest));
                assert(qw + (mid + rest) =~= (qw + mid) + rest);
                assert(alpha_stack(vm_c, pos_c.stack) == astk.subrange(0, n - 2));
                assert(alpha_state(vm_c, pos_c)
                    == SpecState { stack: astk.subrange(0, n - 2), cont: (qw + mid) + rest });
                assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm_c, pos_c)));
            }
}

pub proof fn thm_prim_primrec(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    ac: Seq<SpecWord>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        p == SpecPrim::PrimRec,
        astk == alpha_stack(vm, pos.stack),
        ac == alpha_cont(vm, pos.cont, pos.cursor),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        alpha_cont(vm, pos1.cont, pos1.cursor) == rest,
        rest == ac.subrange(1, ac.len() as int),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        !(spec_step_prim(astk, p, rest) is Fault),
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r is Next
            &&& wf(vm2)
            &&& wf_pos(vm2, pos2)
            &&& spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2))
        }),
{
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 3) by {
                if n < 3 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 3] is Int && astk[n - 2] is Quote && astk[n - 1] is Quote) by {
                if !(astk[n - 3] is Int && astk[n - 2] is Quote && astk[n - 1] is Quote) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let k = astk[n - 3]->Int_0;
            let qiw = astk[n - 2]->Quote_0;
            let qcw = astk[n - 1]->Quote_0;
            assert(astk[n - 3] == SpecValue::Int(k));
            assert(astk[n - 2] == SpecValue::Quote(qiw));
            assert(astk[n - 1] == SpecValue::Quote(qcw));
            let p1 = vm.snodes[s as int].parent;
            lemma_alpha_stack_pop1(vm, s);
            assert(alpha_stack(vm, p1) == astk.subrange(0, n - 1));
            assert(alpha_stack(vm, p1).len() == n - 1);
            assert(p1 != 0) by {
                if p1 == 0 { assert(alpha_stack(vm, p1) =~= Seq::<SpecValue>::empty()); }
            }
            let p2 = vm.snodes[p1 as int].parent;
            lemma_alpha_stack_pop1(vm, p1);
            assert(alpha_stack(vm, p2) == alpha_stack(vm, p1).subrange(0, (n - 1) - 1));
            assert(alpha_stack(vm, p1).subrange(0, n - 2) =~= astk.subrange(0, n - 2));
            assert(alpha_stack(vm, p2) == astk.subrange(0, n - 2));
            assert(alpha_stack(vm, p2).len() == n - 2);
            assert(p2 != 0) by {
                if p2 == 0 { assert(alpha_stack(vm, p2) =~= Seq::<SpecValue>::empty()); }
            }
            let rest_ptr = vm.snodes[p2 as int].parent;
            lemma_alpha_stack_pop1(vm, p2);
            assert(alpha_stack(vm, rest_ptr) == alpha_stack(vm, p2).subrange(0, (n - 2) - 1));
            assert(alpha_stack(vm, p2).subrange(0, n - 3) =~= astk.subrange(0, n - 3));
            assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 3));
            let qcv = vm.snodes[s as int].value;   // top = qc
            let qiv = vm.snodes[p1 as int].value;  // second = qi
            let kv = vm.snodes[p2 as int].value;   // third = n
            assert(astk.last() == alpha_value(vm, qcv));
            assert(astk.last() == astk[n - 1]);
            assert(alpha_value(vm, qcv) == SpecValue::Quote(qcw));
            assert(alpha_stack(vm, p1).last() == alpha_value(vm, qiv));
            assert(alpha_stack(vm, p1).last() == astk.subrange(0, n - 1)[n - 2]);
            assert(astk.subrange(0, n - 1)[n - 2] == astk[n - 2]);
            assert(alpha_value(vm, qiv) == SpecValue::Quote(qiw));
            assert(alpha_stack(vm, p2).last() == alpha_value(vm, kv));
            assert(alpha_stack(vm, p2).last() == astk.subrange(0, n - 2)[n - 3]);
            assert(astk.subrange(0, n - 2)[n - 3] == astk[n - 3]);
            assert(alpha_value(vm, kv) == SpecValue::Int(k));
            assert(kv == ModelValue::Int(k)) by {
                match kv {
                    ModelValue::Int(x) => {},
                    ModelValue::Quote(id) => { assert(alpha_value(vm, kv) == SpecValue::Quote(alpha_quote(vm, id))); },
                }
            };
            let qi = qiv->Quote_0;
            let qc = qcv->Quote_0;
            assert(qiv is Quote) by {
                match qiv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, qiv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(qcv is Quote) by {
                match qcv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, qcv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(qiv == ModelValue::Quote(qi));
            assert(qcv == ModelValue::Quote(qc));
            assert(alpha_quote(vm, qi) == qiw);
            assert(alpha_quote(vm, qc) == qcw);
            assert(snode_val_wf(vm, p1 as int));
            assert(qi.start + qi.len <= vm.tape.len());
            assert(snode_val_wf(vm, s as int));
            assert(qc.start + qc.len <= vm.tape.len());
            let pos_mid = ModelVmState { stack: rest_ptr, ..pos1 };
            assert(wf_pos(vm, pos_mid));
            assert(alpha_cont(vm, pos_mid.cont, pos_mid.cursor) == rest);
            if k <= 0 {
                lemma_alpha_cont_prepend(vm, pos_mid, qi);
                let (vm2, pos2) = model_prepend(vm, pos_mid, qi);
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
                assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
                assert(alpha_cont(vm2, pos2.cont, pos2.cursor) == qiw + rest);
                assert(alpha_stack(vm2, pos2.stack) == astk.subrange(0, n - 3));
                assert(alpha_state(vm2, pos2)
                    == SpecState { stack: astk.subrange(0, n - 3), cont: qiw + rest });
                assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
            } else {
                let setup = seq![
                    ModelWord::PushInt(k),
                    ModelWord::PushInt(k - 1),
                    ModelWord::PushQuote(qi),
                    ModelWord::PushQuote(qc),
                    ModelWord::Prim(SpecPrim::PrimRec)
                ];
                assert forall|i: int| 0 <= i < setup.len() implies word_intern_wf(vm, #[trigger] setup[i]) by {
                    assert(setup[0] == ModelWord::PushInt(k));
                    assert(setup[1] == ModelWord::PushInt(k - 1));
                    assert(setup[2] == ModelWord::PushQuote(qi));
                    assert(setup[3] == ModelWord::PushQuote(qc));
                    assert(setup[4] == ModelWord::Prim(SpecPrim::PrimRec));
                };
                lemma_model_try_alloc(vm, setup);
                let (vm_a, seg_id) = model_try_alloc(vm, setup);
                lemma_alpha_words_intern_seq(vm, vm_a, setup, vm.tape.len());
                let setup_alpha = seq![
                    SpecWord::PushInt(k),
                    SpecWord::PushInt(k - 1),
                    SpecWord::PushQuote(qiw),
                    SpecWord::PushQuote(qcw),
                    SpecWord::Prim(SpecPrim::PrimRec)
                ];
                assert(alpha_quote(vm_a, seg_id) == setup_alpha) by {
                    assert(alpha_quote(vm_a, seg_id)
                        == alpha_words(vm_a, seg_id.start, (seg_id.start + seg_id.len) as nat));
                    assert(Seq::new(setup.len(), |i: int| alpha_word_val(vm, setup[i])) =~= setup_alpha);
                };
                assert(wf_pos(vm_a, pos_mid));
                lemma_alpha_stack_tape_frame(vm, vm_a, rest_ptr);
                lemma_alpha_cont_tape_frame(vm, vm_a, pos1.cont, pos1.cursor);
                assert(alpha_stack(vm_a, rest_ptr) == astk.subrange(0, n - 3));
                assert(alpha_cont(vm_a, pos_mid.cont, pos_mid.cursor) == rest);
                assert(qc.start + qc.len <= vm_a.tape.len());
                lemma_alpha_cont_prepend(vm_a, pos_mid, qc);
                let (vm_b, pos_b) = model_prepend(vm_a, pos_mid, qc);
                assert(alpha_quote(vm_a, qc) == qcw) by {
                    lemma_alpha_words_frame(vm, vm_a, qc.start, qc.start + qc.len);
                };
                assert(alpha_cont(vm_b, pos_b.cont, pos_b.cursor) == qcw + rest);
                assert(alpha_stack(vm_b, pos_b.stack) == astk.subrange(0, n - 3));
                assert(seg_id.start + seg_id.len <= vm_b.tape.len());
                assert(wf_pos(vm_b, pos_b));
                lemma_alpha_cont_prepend(vm_b, pos_b, seg_id);
                let (vm_c, pos_c) = model_prepend(vm_b, pos_b, seg_id);
                assert(alpha_quote(vm_b, seg_id) == setup_alpha) by {
                    lemma_alpha_words_frame(vm_a, vm_b, seg_id.start, seg_id.start + seg_id.len);
                };
                assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm_c, pos_c));
                assert(model_arena_step(vm, pos) == (ModelStep::Next, vm_c, pos_c));
                assert(alpha_cont(vm_c, pos_c.cont, pos_c.cursor) == setup_alpha + (qcw + rest));
                assert(setup_alpha + (qcw + rest) =~= (setup_alpha + qcw) + rest);
                assert(alpha_stack(vm_c, pos_c.stack) == astk.subrange(0, n - 3));
                assert(alpha_state(vm_c, pos_c)
                    == SpecState { stack: astk.subrange(0, n - 3), cont: (setup_alpha + qcw) + rest });
                assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm_c, pos_c)));
            }
}

pub proof fn thm_prim_apply(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    ac: Seq<SpecWord>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        p == SpecPrim::Apply,
        astk == alpha_stack(vm, pos.stack),
        ac == alpha_cont(vm, pos.cont, pos.cursor),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        alpha_cont(vm, pos1.cont, pos1.cursor) == rest,
        rest == ac.subrange(1, ac.len() as int),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        !(spec_step_prim(astk, p, rest) is Fault),
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r is Next
            &&& wf(vm2)
            &&& wf_pos(vm2, pos2)
            &&& spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2))
        }),
{
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 1) by {
                if n < 1 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 1] is Quote) by {
                if !(astk[n - 1] is Quote) { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch)); }
            };
            let qw = astk[n - 1]->Quote_0;
            assert(astk[n - 1] == SpecValue::Quote(qw));
            lemma_pop1(vm, s);
            let rest_ptr = vm.snodes[s as int].parent;
            let bv = vm.snodes[s as int].value;
            assert(alpha_value(vm, bv) == SpecValue::Quote(qw));
            assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 1));
            let qid = bv->Quote_0;
            assert(bv is Quote) by {
                match bv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, bv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(bv == ModelValue::Quote(qid));
            assert(alpha_quote(vm, qid) == qw);
            assert(snode_val_wf(vm, s as int));
            assert(qid.start + qid.len <= vm.tape.len());
            let pos_mid = ModelVmState { stack: rest_ptr, ..pos1 };
            assert(wf_pos(vm, pos_mid));
            assert(alpha_cont(vm, pos_mid.cont, pos_mid.cursor) == rest);
            lemma_alpha_cont_prepend(vm, pos_mid, qid);
            let (vm2, pos2) = model_prepend(vm, pos_mid, qid);
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_cont(vm2, pos2.cont, pos2.cursor) == qw + rest);
            assert(alpha_stack(vm2, pos2.stack) == astk.subrange(0, n - 1));
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 1), cont: qw + rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
}

pub proof fn thm_prim_dip(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    ac: Seq<SpecWord>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        p == SpecPrim::Dip,
        astk == alpha_stack(vm, pos.stack),
        ac == alpha_cont(vm, pos.cont, pos.cursor),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        alpha_cont(vm, pos1.cont, pos1.cursor) == rest,
        rest == ac.subrange(1, ac.len() as int),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        !(spec_step_prim(astk, p, rest) is Fault),
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r is Next
            &&& wf(vm2)
            &&& wf_pos(vm2, pos2)
            &&& spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2))
        }),
{
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 2) by {
                if n < 2 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 1] is Quote) by {
                if !(astk[n - 1] is Quote) { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch)); }
            };
            let qw = astk[n - 1]->Quote_0;
            assert(astk[n - 1] == SpecValue::Quote(qw));
            lemma_pop2(vm, s);
            let p1 = vm.snodes[s as int].parent;
            let rest_ptr = vm.snodes[p1 as int].parent;
            let av = vm.snodes[p1 as int].value;   // second = a
            let bv = vm.snodes[s as int].value;    // top = q
            assert(alpha_value(vm, bv) == SpecValue::Quote(qw));
            assert(alpha_value(vm, av) == astk[n - 2]);
            assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 2));
            let qid = bv->Quote_0;
            assert(bv is Quote) by {
                match bv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, bv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(bv == ModelValue::Quote(qid));
            assert(alpha_quote(vm, qid) == qw);
            assert(snode_val_wf(vm, s as int));
            assert(qid.start + qid.len <= vm.tape.len());
            let aw = model_value_to_word(av);
            assert(word_intern_wf(vm, aw)) by {
                match av {
                    ModelValue::Quote(hid) => { assert(snode_val_wf(vm, p1 as int)); },
                    _ => {},
                }
            };
            lemma_value_to_word_alpha(vm, av);
            assert(alpha_word_val(vm, aw) == value_to_word(astk[n - 2]));
            let segw = seq![aw];
            assert forall|i: int| 0 <= i < segw.len() implies word_intern_wf(vm, #[trigger] segw[i]) by {
                assert(segw[0] == aw);
            };
            lemma_model_try_alloc(vm, segw);
            let (vm_a, seg_id) = model_try_alloc(vm, segw);
            lemma_alpha_words_intern_seq(vm, vm_a, segw, vm.tape.len());
            assert(seg_id.start == vm.tape.len() && seg_id.len == 1);
            assert(alpha_quote(vm_a, seg_id) == seq![value_to_word(astk[n - 2])]) by {
                assert(alpha_quote(vm_a, seg_id) == alpha_words(vm_a, seg_id.start, (seg_id.start + seg_id.len) as nat));
                assert(Seq::new(segw.len(), |i: int| alpha_word_val(vm, segw[i]))
                    =~= seq![value_to_word(astk[n - 2])]);
            };
            let pos_mid = ModelVmState { stack: rest_ptr, ..pos1 };
            assert(wf_pos(vm_a, pos_mid));
            lemma_alpha_stack_tape_frame(vm, vm_a, rest_ptr);
            lemma_alpha_cont_tape_frame(vm, vm_a, pos1.cont, pos1.cursor);
            assert(alpha_stack(vm_a, rest_ptr) == astk.subrange(0, n - 2));
            assert(alpha_cont(vm_a, pos_mid.cont, pos_mid.cursor) == rest);
            lemma_alpha_cont_prepend(vm_a, pos_mid, seg_id);
            let (vm_b, pos_b) = model_prepend(vm_a, pos_mid, seg_id);
            assert(alpha_cont(vm_b, pos_b.cont, pos_b.cursor)
                == seq![value_to_word(astk[n - 2])] + rest);
            assert(alpha_stack(vm_b, pos_b.stack) == astk.subrange(0, n - 2));
            assert(qid.start + qid.len <= vm_b.tape.len());
            assert(wf_pos(vm_b, pos_b));
            lemma_alpha_cont_prepend(vm_b, pos_b, qid);
            let (vm_c, pos_c) = model_prepend(vm_b, pos_b, qid);
            assert(alpha_quote(vm_b, qid) == qw) by {
                lemma_alpha_words_frame(vm, vm_b, qid.start, qid.start + qid.len);
            };
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm_c, pos_c));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm_c, pos_c));
            assert(alpha_cont(vm_c, pos_c.cont, pos_c.cursor)
                == qw + (seq![value_to_word(astk[n - 2])] + rest));
            assert(qw + (seq![value_to_word(astk[n - 2])] + rest)
                =~= qw + seq![value_to_word(astk[n - 2])] + rest);
            assert(alpha_stack(vm_c, pos_c.stack) == astk.subrange(0, n - 2));
            assert(alpha_state(vm_c, pos_c)
                == SpecState {
                    stack: astk.subrange(0, n - 2),
                    cont: qw + seq![value_to_word(astk[n - 2])] + rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm_c, pos_c)));
}

pub proof fn thm_prim_if(
    vm: ModelVm,
    pos: ModelVmState,
    pos1: ModelVmState,
    p: SpecPrim,
    astk: Seq<SpecValue>,
    ac: Seq<SpecWord>,
    rest: Seq<SpecWord>,
    n: int,
)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        wf_pos(vm, pos1),
        p == SpecPrim::If,
        astk == alpha_stack(vm, pos.stack),
        ac == alpha_cont(vm, pos.cont, pos.cursor),
        pos1.stack == pos.stack,
        pos1.stack < vm.snodes.len(),
        alpha_cont(vm, pos1.cont, pos1.cursor) == rest,
        rest == ac.subrange(1, ac.len() as int),
        n == astk.len(),
        model_next_word(vm, pos) == Some::<(ModelWord, ModelVmState)>((ModelWord::Prim(p), pos1)),
        !(spec_step_prim(astk, p, rest) is Fault),
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step(vm, pos);
            &&& r is Next
            &&& wf(vm2)
            &&& wf_pos(vm2, pos2)
            &&& spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2))
        }),
{
            let s = pos1.stack;
            assert(alpha_stack(vm, s) == astk);
            assert(n >= 3) by {
                if n < 3 { assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::Underflow)); }
            };
            assert(astk[n - 3] is Int && astk[n - 2] is Quote && astk[n - 1] is Quote) by {
                if !(astk[n - 3] is Int && astk[n - 2] is Quote && astk[n - 1] is Quote) {
                    assert(spec_step_prim(astk, p, rest) == SpecStep::Fault(Error::TypeMismatch));
                }
            };
            let cval = astk[n - 3]->Int_0;
            let tw = astk[n - 2]->Quote_0;
            let fw = astk[n - 1]->Quote_0;
            assert(astk[n - 3] == SpecValue::Int(cval));
            assert(astk[n - 2] == SpecValue::Quote(tw));
            assert(astk[n - 1] == SpecValue::Quote(fw));
            let p1 = vm.snodes[s as int].parent;
            lemma_alpha_stack_pop1(vm, s);
            assert(alpha_stack(vm, p1) == astk.subrange(0, n - 1));
            assert(alpha_stack(vm, p1).len() == n - 1);
            assert(p1 != 0) by {
                if p1 == 0 { assert(alpha_stack(vm, p1) =~= Seq::<SpecValue>::empty()); }
            }
            let p2 = vm.snodes[p1 as int].parent;
            lemma_alpha_stack_pop1(vm, p1);
            assert(alpha_stack(vm, p2) == alpha_stack(vm, p1).subrange(0, (n - 1) - 1));
            assert(alpha_stack(vm, p1).subrange(0, n - 2) =~= astk.subrange(0, n - 2));
            assert(alpha_stack(vm, p2) == astk.subrange(0, n - 2));
            assert(alpha_stack(vm, p2).len() == n - 2);
            assert(p2 != 0) by {
                if p2 == 0 { assert(alpha_stack(vm, p2) =~= Seq::<SpecValue>::empty()); }
            }
            let rest_ptr = vm.snodes[p2 as int].parent;
            lemma_alpha_stack_pop1(vm, p2);
            assert(alpha_stack(vm, rest_ptr) == alpha_stack(vm, p2).subrange(0, (n - 2) - 1));
            assert(alpha_stack(vm, p2).subrange(0, n - 3) =~= astk.subrange(0, n - 3));
            assert(alpha_stack(vm, rest_ptr) == astk.subrange(0, n - 3));
            let fv = vm.snodes[s as int].value;    // top = f
            let tv = vm.snodes[p1 as int].value;   // second = t
            let cv = vm.snodes[p2 as int].value;   // third = cond
            assert(astk.last() == alpha_value(vm, fv));
            assert(astk.last() == astk[n - 1]);
            assert(alpha_value(vm, fv) == SpecValue::Quote(fw));
            assert(alpha_stack(vm, p1).last() == alpha_value(vm, tv));
            assert(alpha_stack(vm, p1).last() == astk.subrange(0, n - 1)[n - 2]);
            assert(astk.subrange(0, n - 1)[n - 2] == astk[n - 2]);
            assert(alpha_value(vm, tv) == SpecValue::Quote(tw));
            assert(alpha_stack(vm, p2).last() == alpha_value(vm, cv));
            assert(alpha_stack(vm, p2).last() == astk.subrange(0, n - 2)[n - 3]);
            assert(astk.subrange(0, n - 2)[n - 3] == astk[n - 3]);
            assert(alpha_value(vm, cv) == SpecValue::Int(cval));
            assert(cv == ModelValue::Int(cval)) by {
                match cv {
                    ModelValue::Int(x) => {},
                    ModelValue::Quote(id) => { assert(alpha_value(vm, cv) == SpecValue::Quote(alpha_quote(vm, id))); },
                }
            };
            let qt = tv->Quote_0;
            let qf = fv->Quote_0;
            assert(tv is Quote) by {
                match tv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, tv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(fv is Quote) by {
                match fv {
                    ModelValue::Int(x) => { assert(alpha_value(vm, fv) == SpecValue::Int(x)); },
                    ModelValue::Quote(id) => {},
                }
            };
            assert(tv == ModelValue::Quote(qt));
            assert(fv == ModelValue::Quote(qf));
            assert(alpha_quote(vm, qt) == tw);
            assert(alpha_quote(vm, qf) == fw);
            assert(snode_val_wf(vm, p1 as int));
            assert(snode_val_wf(vm, s as int));
            let branch = if cval != 0 { qt } else { qf };
            let bw = if cval != 0 { tw } else { fw };
            assert(alpha_quote(vm, branch) == bw);
            assert(branch.start + branch.len <= vm.tape.len());
            let pos_mid = ModelVmState { stack: rest_ptr, ..pos1 };
            assert(wf_pos(vm, pos_mid));
            assert(alpha_cont(vm, pos_mid.cont, pos_mid.cursor) == rest);
            lemma_alpha_cont_prepend(vm, pos_mid, branch);
            let (vm2, pos2) = model_prepend(vm, pos_mid, branch);
            assert(model_exec_prim(vm, pos1, p) == (ModelStep::Next, vm2, pos2));
            assert(model_arena_step(vm, pos) == (ModelStep::Next, vm2, pos2));
            assert(alpha_cont(vm2, pos2.cont, pos2.cursor) == bw + rest);
            assert(alpha_stack(vm2, pos2.stack) == astk.subrange(0, n - 3));
            assert(alpha_state(vm2, pos2)
                == SpecState { stack: astk.subrange(0, n - 3), cont: bw + rest });
            assert(spec_step_prim(astk, p, rest) == SpecStep::Next(alpha_state(vm2, pos2)));
}

// ============================================================
// 6. M4 part 3 — the u32 address-space overflow (honest capacity story).
//
// The ghost `spec_step` uses UNBOUNDED `Seq`s and NEVER faults on capacity; it
// always does `q ++ rest`, `a ++ b`, etc. Production, by contrast, interns into a
// `Vec` addressed by `u32`, so `try_alloc`/`try_cat`/`try_cons`/`try_linrec_else`
// and `prepend` return `Fault::Overflow` when a fresh index would exceed
// `u32::MAX` (prim.rs `alloc_or_overflow!`, design §3.4). The arena model above is
// likewise unbounded, so model-vs-spec refinement + fault parity hold
// UNCONDITIONALLY (`thm_arena_refines_spec_scaffold`). This section closes the gap
// to production HONESTLY: it does NOT pretend the ghost has a capacity fault it
// lacks. Instead it models production's u32 ceiling as arena-side graceful
// degradation — a clean `Fault::Overflow` — cleanly gated by a `capacity`
// predicate, and proves BOTH directions.
//
// `capacity(vm, pos)` is defined DIRECTLY over the ACTUAL post-step arena computed
// by `model_arena_step`: the step's interning/prepend allocations keep every arena
// `Seq` length within the u32 address ceiling. This is faithful by construction —
// no re-derivation of per-prim allocation sizes can drift from the real model —
// and is exactly the design §4.4 predicate ("this step's allocations keep tape
// length and node counts <= u32::MAX"). On a semantic fault the post-step arena
// equals the pre-step arena (fault parity's `vm2 == vm`), which fits u32 for any
// reachable state, so a semantic fault is NEVER masked as Overflow — matching
// production, which faults semantically BEFORE any allocation.
// ============================================================

/// The u32 address ceiling as a `nat` predicate: `n` is a valid arena `Seq` length
/// (a fresh element lands at index `n`, which must be a valid `u32` address).
pub open spec fn fits_u32(n: nat) -> bool {
    n <= 0xFFFF_FFFF
}

/// The honest capacity predicate (design §4.4): this step's allocations keep every
/// arena `Seq` (tape, cont nodes, stack nodes) within the u32 ceiling. Evaluated on
/// the ACTUAL post-step arena, so it is faithful to the model's real allocations by
/// construction.
pub open spec fn capacity(vm: ModelVm, pos: ModelVmState) -> bool {
    let (r, vm2, pos2) = model_arena_step(vm, pos);
    &&& fits_u32(vm2.tape.len())
    &&& fits_u32(vm2.cnodes.len())
    &&& fits_u32(vm2.snodes.len())
}

/// The PRODUCTION-faithful step: like `model_arena_step`, but when the step's
/// allocations would exceed the u32 ceiling it degrades gracefully to
/// `Fault::Overflow` with the machine left untouched — exactly production's
/// `alloc_or_overflow!` (return `Fault::Overflow`) followed by `arena_step`'s
/// `*st = saved` (restore the pre-step position). This is the faithful twin of the
/// production arena at the u32 boundary; the unbounded `model_arena_step` is its
/// idealization under `capacity`.
pub open spec fn model_arena_step_prod(vm: ModelVm, pos: ModelVmState)
    -> (ModelStep, ModelVm, ModelVmState) {
    let (r, vm2, pos2) = model_arena_step(vm, pos);
    if fits_u32(vm2.tape.len()) && fits_u32(vm2.cnodes.len()) && fits_u32(vm2.snodes.len()) {
        (r, vm2, pos2)
    } else {
        (ModelStep::Fault(Error::Overflow), vm, pos)
    }
}

/// The u32-honest refinement theorem. For every reachable (wf) state, the
/// production-faithful step either:
///   * (capacity holds) refines `spec_step ∘ alpha_state` EXACTLY, with full
///     semantic-fault parity (same kind, precedence, machine-untouched-on-fault)
///     — identical to `thm_arena_refines_spec_scaffold`; OR
///   * (an allocation would exceed u32::MAX) faults cleanly with `Overflow`,
///     leaving the machine untouched (`vm2 == vm && pos2 == pos`) — the AC's
///     "u32 overflow mapping cleanly to Fault::Overflow".
/// BOTH directions are proven. The ONLY divergence of the arena from the unbounded
/// ghost is this cleanly-gated capacity Overflow.
pub proof fn thm_arena_refines_spec_u32(vm: ModelVm, pos: ModelVmState)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
    ensures
        ({
            let (r, vm2, pos2) = model_arena_step_prod(vm, pos);
            if capacity(vm, pos) {
                match spec_step(alpha_state(vm, pos)) {
                    SpecStep::Next(s2) => r is Next && wf(vm2) && wf_pos(vm2, pos2)
                        && alpha_state(vm2, pos2) == s2,
                    SpecStep::Halt(_) => r is Halt,
                    SpecStep::Fault(e) => r == ModelStep::Fault(e) && vm2 == vm && pos2 == pos,
                    SpecStep::Invoke(nm, stk, ct) => r is Invoke && r->Invoke_0 == nm
                        && alpha_state(vm2, pos2) == (SpecState { stack: stk, cont: ct }),
                }
            } else {
                r == ModelStep::Fault(Error::Overflow) && vm2 == vm && pos2 == pos
            }
        }),
{
    // capacity-ok: model_arena_step_prod delegates to model_arena_step verbatim, so
    // the unconditional refinement + fault parity theorem discharges every arm.
    // !capacity: the wrapper's else-arm returns (Fault(Overflow), vm, pos) by def.
    thm_arena_refines_spec_scaffold(vm, pos);
}

// ============================================================
// 7. M5a — the multi-step DRIVER corollary.
//
// The one-step theorem (`thm_arena_refines_spec_scaffold`) proves α commutes with
// a SINGLE `model_arena_step`. This section lifts it to a WHOLE fuel-bounded run:
// the model driver `model_run` iterates `model_arena_step` until Halt/Fault/Invoke
// or fuel exhaustion, and — via α — refines the ghost `spec_run` (mtl_core) EXACTLY.
// This mirrors P2's `run` refining `spec_run` (and the `p2_refinement` multi-step
// clause), but here the driver is a pure-spec model, so it is stated as a `proof
// fn` proving `model_run == spec_run ∘ alpha_state`, by induction on fuel with the
// one-step theorem as the inductive step.
//
// The induction needs the standing invariant (`wf`, `wf_svals`, `wf_pos`) PRESERVED
// across a step; the one-step theorem already ensures `wf`/`wf_pos` of the
// successor, so this section additionally proves `wf_svals` preservation
// (`lemma_step_wf_svals`) — every reachable stack node keeps a tape-valid `Quote`
// under the append-only tape/stack growth of a step.
// ============================================================

/// A single stack push preserves `wf_svals` when the pushed value is tape-valid
/// (`Int` always is; `Quote(id)` needs `id.end() <= tape.len()`). The tape is
/// untouched by a push, so every pre-existing node stays valid too.
pub proof fn lemma_push_wf_svals(vm: ModelVm, ptr: nat, v: ModelValue)
    requires
        wf_svals(vm),
        (v matches ModelValue::Quote(id) ==> id.start + id.len <= vm.tape.len()),
    ensures
        wf_svals(model_push_node(vm, ptr, v).0),
{
    let vm2 = model_push_node(vm, ptr, v).0;
    assert(vm2.tape == vm.tape);
    assert forall|i: int| 1 <= i < vm2.snodes.len() implies #[trigger] snode_val_wf(vm2, i) by {
        if i < vm.snodes.len() {
            assert(vm2.snodes[i] == vm.snodes[i]);
            assert(snode_val_wf(vm, i));
        } else {
            // i == vm.snodes.len(): the freshly pushed node holds `v`.
            assert(vm2.snodes[i] == (ModelStackNode { value: v, parent: ptr }));
        }
    }
}

/// A push preserves BOTH `wf` and `wf_svals` (given the pushed value tape-valid),
/// and reports the new index / length / tape-unchanged — the reusable step for a
/// push-chain (`Swap`/`Rot`/`Uncons`).
pub proof fn lemma_push_wf_all(vm: ModelVm, ptr: nat, v: ModelValue)
    requires
        wf(vm),
        wf_svals(vm),
        ptr < vm.snodes.len(),
        (v matches ModelValue::Quote(id) ==> id.start + id.len <= vm.tape.len()),
    ensures
        ({
            let (vm2, np) = model_push_node(vm, ptr, v);
            &&& wf(vm2)
            &&& wf_svals(vm2)
            &&& np == vm.snodes.len()
            &&& vm2.snodes.len() == vm.snodes.len() + 1
            &&& vm2.tape == vm.tape
        }),
{
    lemma_push_node(vm, ptr, v);
    lemma_push_wf_svals(vm, ptr, v);
}

/// `wf_svals` is preserved by any append-only tape growth that leaves the stack
/// arena untouched — the shape of every control combinator (they grow only the
/// tape/cont arenas via `model_try_alloc` / `model_prepend`, both `..vm` on snodes).
pub proof fn lemma_snodes_frame_wf_svals(vm: ModelVm, vm2: ModelVm)
    requires
        wf_svals(vm),
        vm2.snodes == vm.snodes,
        vm.tape.len() <= vm2.tape.len(),
    ensures
        wf_svals(vm2),
{
    assert forall|i: int| 1 <= i < vm2.snodes.len() implies #[trigger] snode_val_wf(vm2, i) by {
        assert(vm2.snodes[i] == vm.snodes[i]);
        assert(snode_val_wf(vm, i));
    }
}

/// `model_exec_prim` preserves `wf_svals`: every stack push lands a tape-valid
/// value (an `Int`, a copy of an already-valid stack value, or a `Quote` whose
/// span lies inside the — possibly grown — tape), and the control combinators
/// leave the stack arena untouched while the tape only grows.
#[verifier::rlimit(400)]
pub proof fn lemma_exec_prim_wf_svals(vm: ModelVm, pos: ModelVmState, p: SpecPrim)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
    ensures
        wf_svals(model_exec_prim(vm, pos, p).1),
{
    // Every fault sub-arm returns `(Fault(..), vm, pos)`, so `wf_svals(vm)` closes
    // it; the interesting work is the success sub-arms below.
    match p {
        SpecPrim::Dup => {
            if pos.stack != 0 {
                let top = vm.snodes[pos.stack as int].value;
                assert(snode_val_wf(vm, pos.stack as int));
                lemma_push_wf_all(vm, pos.stack, top);
            }
        },
        SpecPrim::Drop => {},
        SpecPrim::Swap => {
            if pos.stack != 0 {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 != 0 {
                    let b = vm.snodes[pos.stack as int].value;
                    let a = vm.snodes[p1 as int].value;
                    let rest = vm.snodes[p1 as int].parent;
                    assert(p1 < vm.snodes.len());
                    assert(rest < vm.snodes.len());
                    assert(snode_val_wf(vm, pos.stack as int));
                    assert(snode_val_wf(vm, p1 as int));
                    lemma_push_wf_all(vm, rest, b);
                    let (vm1, s1) = model_push_node(vm, rest, b);
                    lemma_push_wf_all(vm1, s1, a);
                }
            }
        },
        SpecPrim::Rot => {
            if pos.stack != 0 {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 != 0 {
                    let p2 = vm.snodes[p1 as int].parent;
                    if p2 != 0 {
                        let c = vm.snodes[pos.stack as int].value;
                        let b = vm.snodes[p1 as int].value;
                        let a = vm.snodes[p2 as int].value;
                        let rest = vm.snodes[p2 as int].parent;
                        assert(p1 < vm.snodes.len() && p2 < vm.snodes.len() && rest < vm.snodes.len());
                        assert(snode_val_wf(vm, pos.stack as int));
                        assert(snode_val_wf(vm, p1 as int));
                        assert(snode_val_wf(vm, p2 as int));
                        lemma_push_wf_all(vm, rest, b);
                        let (vm1, s1) = model_push_node(vm, rest, b);
                        lemma_push_wf_all(vm1, s1, c);
                        let (vm2, s2) = model_push_node(vm1, s1, c);
                        lemma_push_wf_all(vm2, s2, a);
                    }
                }
            }
        },
        SpecPrim::Over => {
            if pos.stack != 0 {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 != 0 {
                    let a = vm.snodes[p1 as int].value;
                    assert(p1 < vm.snodes.len());
                    assert(snode_val_wf(vm, p1 as int));
                    lemma_push_wf_all(vm, pos.stack, a);
                }
            }
        },
        // Arithmetic / comparison / xor: push an `Int` (always tape-valid).
        SpecPrim::Add => lemma_arith_wf_svals(vm, pos, |a: int, b: int| a + b),
        SpecPrim::Sub => lemma_arith_wf_svals(vm, pos, |a: int, b: int| a - b),
        SpecPrim::Mul => lemma_arith_wf_svals(vm, pos, |a: int, b: int| a * b),
        SpecPrim::Div => lemma_divmod_wf_svals(vm, pos, true),
        SpecPrim::Mod => lemma_divmod_wf_svals(vm, pos, false),
        SpecPrim::Eq => lemma_cmp_wf_svals(vm, pos, |a: int, b: int| a == b),
        SpecPrim::Lt => lemma_cmp_wf_svals(vm, pos, |a: int, b: int| a < b),
        SpecPrim::Xor => {
            if pos.stack != 0 {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 != 0 {
                    let rest_ptr = vm.snodes[p1 as int].parent;
                    match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                        (ModelValue::Int(a), ModelValue::Int(b)) => {
                            assert(rest_ptr < vm.snodes.len());
                            lemma_push_wf_all(vm, rest_ptr, ModelValue::Int(i64_bitxor(a, b)));
                        },
                        _ => {},
                    }
                }
            }
        },
        SpecPrim::Cons => {
            if pos.stack != 0 {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 != 0 {
                    let rest_ptr = vm.snodes[p1 as int].parent;
                    let v = vm.snodes[p1 as int].value;
                    match vm.snodes[pos.stack as int].value {
                        ModelValue::Quote(qid) => {
                            assert(rest_ptr < vm.snodes.len());
                            assert(snode_val_wf(vm, pos.stack as int));   // qid tape-valid
                            assert(snode_val_wf(vm, p1 as int));          // v (if Quote) tape-valid
                            let head = model_value_to_word(v);
                            assert(word_intern_wf(vm, head));
                            lemma_model_try_cons(vm, head, qid);
                            let (vm_t, new_id) = model_try_cons(vm, head, qid);
                            lemma_snodes_frame_wf_svals(vm, vm_t);
                            assert(new_id.start + new_id.len == vm_t.tape.len());
                            lemma_push_wf_all(vm_t, rest_ptr, ModelValue::Quote(new_id));
                        },
                        _ => {},
                    }
                }
            }
        },
        SpecPrim::Cat => {
            if pos.stack != 0 {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 != 0 {
                    let rest_ptr = vm.snodes[p1 as int].parent;
                    match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                        (ModelValue::Quote(aid), ModelValue::Quote(bid)) => {
                            assert(rest_ptr < vm.snodes.len());
                            assert(snode_val_wf(vm, p1 as int));
                            assert(snode_val_wf(vm, pos.stack as int));
                            lemma_model_try_cat(vm, aid, bid);
                            let (vm_t, new_id) = model_try_cat(vm, aid, bid);
                            lemma_snodes_frame_wf_svals(vm, vm_t);
                            assert(new_id.start + new_id.len == vm_t.tape.len());
                            lemma_push_wf_all(vm_t, rest_ptr, ModelValue::Quote(new_id));
                        },
                        _ => {},
                    }
                }
            }
        },
        SpecPrim::Uncons => {
            if pos.stack != 0 {
                let rest_ptr = vm.snodes[pos.stack as int].parent;
                assert(rest_ptr < vm.snodes.len());
                match vm.snodes[pos.stack as int].value {
                    ModelValue::Quote(qid) => {
                        assert(snode_val_wf(vm, pos.stack as int));   // qid.end() <= tape.len()
                        if qid.len == 0 {
                            lemma_push_wf_all(vm, rest_ptr, ModelValue::Int(0int));
                        } else {
                            let tail = ModelQuoteId { start: qid.start + 1, len: (qid.len - 1) as nat };
                            assert(tail.start + tail.len == qid.start + qid.len);
                            assert(qid.start < vm.tape.len());
                            match vm.tape[qid.start as int] {
                                ModelWord::PushInt(k) => {
                                    lemma_push_wf_all(vm, rest_ptr, ModelValue::Int(k));
                                    let (vm1, s1) = model_push_node(vm, rest_ptr, ModelValue::Int(k));
                                    lemma_push_wf_all(vm1, s1, ModelValue::Quote(tail));
                                    let (vm2, s2) = model_push_node(vm1, s1, ModelValue::Quote(tail));
                                    lemma_push_wf_all(vm2, s2, ModelValue::Int(1int));
                                },
                                ModelWord::PushQuote(hid) => {
                                    assert(wf_tape_word(vm, qid.start as int));   // hid.end() <= qid.start
                                    lemma_push_wf_all(vm, rest_ptr, ModelValue::Quote(hid));
                                    let (vm1, s1) = model_push_node(vm, rest_ptr, ModelValue::Quote(hid));
                                    lemma_push_wf_all(vm1, s1, ModelValue::Quote(tail));
                                    let (vm2, s2) = model_push_node(vm1, s1, ModelValue::Quote(tail));
                                    lemma_push_wf_all(vm2, s2, ModelValue::Int(1int));
                                },
                                _ => {},
                            }
                        }
                    },
                    _ => {},
                }
            }
        },
        // Control combinators: no stack push (except Fold-empty), tape only grows,
        // stack arena untouched (`model_try_alloc` / `model_prepend` thread `..vm`).
        SpecPrim::Apply => lemma_ctrl_wf_svals(vm, pos, p),
        SpecPrim::Dip => lemma_ctrl_wf_svals(vm, pos, p),
        SpecPrim::If => lemma_ctrl_wf_svals(vm, pos, p),
        SpecPrim::Times => lemma_ctrl_wf_svals(vm, pos, p),
        SpecPrim::PrimRec => lemma_ctrl_wf_svals(vm, pos, p),
        SpecPrim::LinRec => lemma_ctrl_wf_svals(vm, pos, p),
        SpecPrim::Fold => lemma_ctrl_wf_svals(vm, pos, p),
    }
}

/// `model_arith` preserves `wf_svals` (success pushes an `Int`).
pub proof fn lemma_arith_wf_svals(vm: ModelVm, pos: ModelVmState, op: spec_fn(int, int) -> int)
    requires wf(vm), wf_svals(vm), wf_pos(vm, pos),
    ensures wf_svals(model_arith(vm, pos, op).1),
{
    if pos.stack != 0 {
        let p1 = vm.snodes[pos.stack as int].parent;
        if p1 != 0 {
            let rest_ptr = vm.snodes[p1 as int].parent;
            match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                (ModelValue::Int(a), ModelValue::Int(b)) => {
                    if in_i64(op(a, b)) {
                        assert(rest_ptr < vm.snodes.len());
                        lemma_push_wf_all(vm, rest_ptr, ModelValue::Int(op(a, b)));
                    }
                },
                _ => {},
            }
        }
    }
}

/// `model_divmod` preserves `wf_svals` (success pushes an `Int`).
pub proof fn lemma_divmod_wf_svals(vm: ModelVm, pos: ModelVmState, is_div: bool)
    requires wf(vm), wf_svals(vm), wf_pos(vm, pos),
    ensures wf_svals(model_divmod(vm, pos, is_div).1),
{
    if pos.stack != 0 {
        let p1 = vm.snodes[pos.stack as int].parent;
        if p1 != 0 {
            let rest_ptr = vm.snodes[p1 as int].parent;
            match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                (ModelValue::Int(a), ModelValue::Int(b)) => {
                    if b != 0 && in_i64(trunc_div(a, b)) {
                        let r = if is_div { trunc_div(a, b) } else { trunc_mod(a, b) };
                        assert(rest_ptr < vm.snodes.len());
                        lemma_push_wf_all(vm, rest_ptr, ModelValue::Int(r));
                    }
                },
                _ => {},
            }
        }
    }
}

/// `model_cmp` preserves `wf_svals` (success pushes an `Int` 1/0).
pub proof fn lemma_cmp_wf_svals(vm: ModelVm, pos: ModelVmState, op: spec_fn(int, int) -> bool)
    requires wf(vm), wf_svals(vm), wf_pos(vm, pos),
    ensures wf_svals(model_cmp(vm, pos, op).1),
{
    if pos.stack != 0 {
        let p1 = vm.snodes[pos.stack as int].parent;
        if p1 != 0 {
            let rest_ptr = vm.snodes[p1 as int].parent;
            match (vm.snodes[p1 as int].value, vm.snodes[pos.stack as int].value) {
                (ModelValue::Int(a), ModelValue::Int(b)) => {
                    let r: int = if op(a, b) { 1int } else { 0int };
                    assert(rest_ptr < vm.snodes.len());
                    lemma_push_wf_all(vm, rest_ptr, ModelValue::Int(r));
                },
                _ => {},
            }
        }
    }
}

/// The seven control combinators preserve `wf_svals`. `Fold` empty-seq pushes the
/// `init` value (a copy of an already tape-valid stack value); every other success
/// path leaves the stack arena untouched (`vm2.snodes == vm.snodes`) while the tape
/// only grows, so `lemma_snodes_frame_wf_svals` closes it. All fault arms return
/// `vm` untouched.
#[verifier::rlimit(1000)]
pub proof fn lemma_ctrl_wf_svals(vm: ModelVm, pos: ModelVmState, p: SpecPrim)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        p == SpecPrim::Apply || p == SpecPrim::Dip || p == SpecPrim::If
            || p == SpecPrim::Times || p == SpecPrim::PrimRec || p == SpecPrim::LinRec
            || p == SpecPrim::Fold,
    ensures
        wf_svals(model_exec_prim(vm, pos, p).1),
{
    // Match on `p` so each SMT query unfolds exactly ONE combinator (unfolding the
    // whole polymorphic `model_exec_prim` at once blows the rlimit). Apply/Dip/If/
    // Times/PrimRec/LinRec have NO stack push — in every branch (fault or success)
    // the stack arena is threaded `..vm` and the tape only grows, so the
    // snodes-frame lemma closes them. Fold's empty-seq branch is the sole control
    // push (it pushes `init`, a copy of an already tape-valid stack value).
    match p {
        SpecPrim::Fold => {
            if pos.stack != 0 {
                let p1 = vm.snodes[pos.stack as int].parent;
                if p1 != 0 {
                    let p2 = vm.snodes[p1 as int].parent;
                    if p2 != 0 {
                        let rest_ptr = vm.snodes[p2 as int].parent;
                        let init = vm.snodes[p1 as int].value;
                        match (vm.snodes[p2 as int].value, vm.snodes[pos.stack as int].value) {
                            (ModelValue::Quote(qs), ModelValue::Quote(qc)) => {
                                if qs.len == 0 {
                                    assert(rest_ptr < vm.snodes.len());
                                    assert(snode_val_wf(vm, p1 as int));   // init tape-valid
                                    lemma_push_wf_all(vm, rest_ptr, init);
                                } else {
                                    // nonempty: fault (vm) or control chain (snodes==, tape grows).
                                    let vm2 = model_exec_prim(vm, pos, p).1;
                                    assert(vm2.snodes == vm.snodes);
                                    assert(vm.tape.len() <= vm2.tape.len());
                                    lemma_snodes_frame_wf_svals(vm, vm2);
                                }
                            },
                            _ => {},   // fault: vm2 == vm.
                        }
                    }
                }
            }
        },
        _ => {
            // Apply/Dip/If/Times/PrimRec/LinRec: no push. All branches keep the stack
            // arena and grow the tape monotonically.
            let vm2 = model_exec_prim(vm, pos, p).1;
            assert(vm2.snodes == vm.snodes);
            assert(vm.tape.len() <= vm2.tape.len());
            lemma_snodes_frame_wf_svals(vm, vm2);
        },
    }
}

/// `model_exec_word` preserves `wf_svals`. `PushInt` / `Call` are trivial; a
/// `PushQuote(id)` pushes a `Quote` whose span lies in the tape (`word_intern_wf`);
/// a `Prim` delegates to `lemma_exec_prim_wf_svals`.
pub proof fn lemma_exec_word_wf_svals(vm: ModelVm, pos: ModelVmState, w: ModelWord)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
        word_intern_wf(vm, w),
    ensures
        wf_svals(model_exec_word(vm, pos, w).1),
{
    match w {
        ModelWord::PushInt(n) => {
            lemma_push_wf_all(vm, pos.stack, ModelValue::Int(n));
        },
        ModelWord::PushQuote(id) => {
            // word_intern_wf gives id.start + id.len <= vm.tape.len().
            lemma_push_wf_all(vm, pos.stack, ModelValue::Quote(id));
        },
        ModelWord::Call(_) => {
            // Invoke: vm2 == vm.
        },
        ModelWord::Prim(p) => {
            lemma_exec_prim_wf_svals(vm, pos, p);
        },
    }
}

/// One `model_arena_step` preserves `wf_svals`. On `Halt` (no next word) the vm is
/// unchanged; otherwise the result vm is exactly `model_exec_word`'s (the fault arm
/// of `model_arena_step` keeps the append-only vm and only restores `pos`).
pub proof fn lemma_step_wf_svals(vm: ModelVm, pos: ModelVmState)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
    ensures
        wf_svals(model_arena_step(vm, pos).1),
{
    lemma_model_next_word(vm, pos);
    match model_next_word(vm, pos) {
        None => {
            // model_arena_step == (Halt, vm, pos).
        },
        Some((w, pos1)) => {
            // lemma_model_next_word: wf_pos(vm, pos1) && word_intern_wf(vm, w).
            lemma_exec_word_wf_svals(vm, pos1, w);
            // model_arena_step(vm, pos).1 == model_exec_word(vm, pos1, w).1 in both
            // the fault arm (which keeps vm2) and the non-fault arm.
        },
    }
}

// ------------------------------------------------------------
// The model multi-step driver and the DRIVER COROLLARY.
// ------------------------------------------------------------

/// The model's fuel-bounded driver — the pure-spec twin of `run.rs::run_arena`'s
/// loop and the exact model mirror of mtl_core's `spec_run`. It iterates
/// `model_arena_step` up to `fuel` times, projecting the terminal machine state
/// through α into the thin `SpecOutcome` (the only observations a driver makes).
pub open spec fn model_run(vm: ModelVm, pos: ModelVmState, fuel: nat) -> SpecOutcome
    decreases fuel,
{
    if fuel == 0 {
        SpecOutcome::FuelExhausted
    } else {
        let (r, vm2, pos2) = model_arena_step(vm, pos);
        match r {
            ModelStep::Halt => SpecOutcome::Halt(alpha_stack(vm2, pos2.stack)),
            ModelStep::Fault(e) => SpecOutcome::Fault(e),
            ModelStep::Invoke(nm) => SpecOutcome::Invoke(
                nm,
                alpha_stack(vm2, pos2.stack),
                alpha_cont(vm2, pos2.cont, pos2.cursor),
            ),
            ModelStep::Next => model_run(vm2, pos2, (fuel - 1) as nat),
        }
    }
}

/// ★ THE MULTI-STEP DRIVER COROLLARY (M5a). For a well-formed initial state,
/// running the model driver for `fuel` steps and abstracting through α equals
/// running the ghost `spec_run` for `fuel` steps from `alpha_state` — i.e. α
/// commutes with the WHOLE execution, not just one step:
///
///     model_run(vm, pos, fuel) == spec_run(alpha_state(vm, pos), fuel)
///
/// Proven by induction on `fuel`, using the one-step theorem
/// `thm_arena_refines_spec_scaffold` (the commuting square) as the inductive step
/// and `lemma_step_wf_svals` to carry the `wf_svals` invariant across the step.
/// This is the arena twin of `run` refining `spec_run` in mtl_core (P2).
pub proof fn thm_arena_run_refines_spec(vm: ModelVm, pos: ModelVmState, fuel: nat)
    requires
        wf(vm),
        wf_svals(vm),
        wf_pos(vm, pos),
    ensures
        model_run(vm, pos, fuel) == spec_run(alpha_state(vm, pos), fuel),
    decreases fuel,
{
    if fuel == 0 {
        // Both sides are FuelExhausted.
    } else {
        thm_arena_refines_spec_scaffold(vm, pos);
        lemma_step_wf_svals(vm, pos);
        let s = alpha_state(vm, pos);
        let (r, vm2, pos2) = model_arena_step(vm, pos);
        match spec_step(s) {
            SpecStep::Halt(stk) => {
                // spec_step Halt => s.cont empty => model_next_word None => vm2==vm, pos2==pos.
                assert(stk == s.stack);
                assert(s.cont.len() == 0);
                lemma_model_next_word(vm, pos);
                assert(model_next_word(vm, pos) is None);
                assert(model_arena_step(vm, pos) == (ModelStep::Halt, vm, pos));
                assert(r == ModelStep::Halt);
                assert(alpha_stack(vm2, pos2.stack) == alpha_stack(vm, pos.stack));
                assert(alpha_stack(vm, pos.stack) == s.stack);
                assert(model_run(vm, pos, fuel) == SpecOutcome::Halt(stk));
                assert(spec_run(s, fuel) == SpecOutcome::Halt(stk));
            },
            SpecStep::Fault(e) => {
                // One-step theorem: r == Fault(e).
                assert(r == ModelStep::Fault(e));
                assert(model_run(vm, pos, fuel) == SpecOutcome::Fault(e));
                assert(spec_run(s, fuel) == SpecOutcome::Fault(e));
            },
            SpecStep::Invoke(nm, istk, ict) => {
                // One-step theorem: r is Invoke, r->Invoke_0 == nm, alpha_state(vm2,pos2) == {istk, ict}.
                assert(r is Invoke);
                assert(r->Invoke_0 == nm);
                assert(alpha_state(vm2, pos2) == (SpecState { stack: istk, cont: ict }));
                assert(alpha_stack(vm2, pos2.stack) == istk);
                assert(alpha_cont(vm2, pos2.cont, pos2.cursor) == ict);
                assert(model_run(vm, pos, fuel) == SpecOutcome::Invoke(nm, istk, ict));
                assert(spec_run(s, fuel) == SpecOutcome::Invoke(nm, istk, ict));
            },
            SpecStep::Next(s2) => {
                // One-step theorem: r is Next, wf(vm2), wf_pos(vm2,pos2), alpha_state(vm2,pos2)==s2.
                assert(r is Next);
                assert(wf(vm2) && wf_pos(vm2, pos2));
                assert(wf_svals(vm2));
                assert(alpha_state(vm2, pos2) == s2);
                // IH on the successor.
                thm_arena_run_refines_spec(vm2, pos2, (fuel - 1) as nat);
                assert(model_run(vm2, pos2, (fuel - 1) as nat)
                    == spec_run(alpha_state(vm2, pos2), (fuel - 1) as nat));
                assert(model_run(vm, pos, fuel) == model_run(vm2, pos2, (fuel - 1) as nat));
                assert(spec_run(s, fuel) == spec_run(s2, (fuel - 1) as nat));
            },
        }
    }
}

} // verus!

fn main() {}
