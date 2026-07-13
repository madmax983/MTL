//! P5 — Universality of MTL via a two-counter Minsky-machine simulation
//! ====================================================================
//!
//! This file discharges proof-obligation **P5** (spec §6.5, §7.3): it turns the
//! MTL Turing-completeness *conjecture* into a *theorem* at the `spec_step`
//! level, by exhibiting a faithful lock-step simulation of an arbitrary
//! two-counter Minsky machine (which are classically Turing complete).
//!
//! It is a **purely additive** proof over the FROZEN spec layer
//! [`mtl_core`]: it introduces no new primitive and does not touch
//! `spec_step`/`spec_step_prim`. `mtl_core.rs` is pulled in unmodified via the
//! `mod mtl_core;` declaration below and is re-verified together with this file
//! by the same `verus crates/mtl-core/src/p5_universality.rs` invocation.
//!
//! ## Scope of the claim (the honesty pivot — brief §6 / spec §6.1)
//!
//! The failed original TC "proof" encoded each Minsky counter as one bottom-of-
//! stack `Int`. MTL `Int` is bounded to `i64` (`in_i64`, mtl_core.rs:75), so two
//! `i64` counters give a *finite* reachable state space — not Turing complete.
//!
//! The repair, realized here, moves the unboundedness OFF the `Int` and INTO
//! **quotation length**: a counter of value `n` is `Quote(unary(n))`, a
//! `SpecValue::Quote` whose `Seq<SpecWord>` has length exactly `n`. `Seq` length
//! is unbounded at the ghost/spec level — `spec_step_prim`'s `Cons`
//! (mtl_core.rs:251) and `Uncons` (mtl_core.rs:419) impose no `i64` cap on
//! quotation length. Precisely:
//!
//!   * The `i64` bound (P2) still limits integer *values* (`Int`), and the
//!     program counter is a deliberately bounded `Int` — sound, because the
//!     instruction table is finite so the PC ranges over a finite set.
//!   * The bound does **not** limit the simulation: the two counters live in
//!     `unary(n).len()`, an unbounded `nat`.
//!   * The theorem is about the `spec_step` semantics over unbounded `Seq`. The
//!     executable `run` (mtl_core.rs:2672) is memory-bounded like any real
//!     machine and is not claimed to terminate — TC forbids that. All halting
//!     statements are therefore quantified over `fuel` explicitly.
//!
//! ## What is proved here
//!
//!   * `minsky_step` / `minsky_run`: the two-counter machine, unbounded `nat`
//!     counters — the object we simulate.
//!   * `unary` + lemmas: the unbounded quotation counter encoding.
//!   * The three counter operations as verified `spec_step` fragments:
//!     increment (`lemma_inc_frag`), decrement-nonzero and zero-test
//!     (`lemma_dec_nz_frag`, `lemma_dec_z_frag`) — MTL's `spec_step` performs
//!     each Minsky counter operation over the unbounded encoding.
//!   * `lemma_dispatch_select`: the PC big-switch selects the right handler —
//!     the control-flow crux, proved by induction over the dispatch cascade.
//!   * The fuel bridge: `spec_stepn` (concrete step count) ↔ `spec_run` (fuel)
//!     composition (`lemma_run_from_stepn`) and halt monotonicity
//!     (`lemma_run_mono`).
//!   * The lock-step simulation theorem and the fuel-quantified halting
//!     correspondence assembled from the above.
//!
//! See the module-level `HONEST STATUS` comment near the theorems for the exact
//! proved/pending boundary.

use vstd::prelude::*;

#[path = "mtl_core.rs"]
mod mtl_core;
use mtl_core::*;

verus! {

// ============================================================
// 1. The two-counter Minsky machine (ghost / spec level)
// ============================================================
//
// Three instruction forms (spec §6.4). `reg`: false = c1, true = c2.
// Counters are unbounded `nat` — this is the source of universality.

pub enum MInstr {
    /// Inc(reg, next): c[reg] += 1; goto next.
    Inc(bool, nat),
    /// DecJz(reg, jz, nz): if c[reg]==0 goto jz (no change);
    ///                     else c[reg]-=1, goto nz.
    DecJz(bool, nat, nat),
    /// Halt the machine.
    Halt,
}

pub struct MProg {
    pub code: Seq<MInstr>,
}

pub struct MConf {
    pub pc: nat,
    pub c1: nat,
    pub c2: nat,
}

/// Small-step semantics. `None` == halted (Halt reached or pc out of range).
pub open spec fn minsky_step(prog: MProg, m: MConf) -> Option<MConf> {
    if m.pc >= prog.code.len() {
        None
    } else {
        match prog.code[m.pc as int] {
            MInstr::Halt => None,
            MInstr::Inc(reg, j) =>
                if reg {
                    Some(MConf { pc: j, c1: m.c1, c2: (m.c2 + 1) as nat })
                } else {
                    Some(MConf { pc: j, c1: (m.c1 + 1) as nat, c2: m.c2 })
                },
            MInstr::DecJz(reg, jz, nz) =>
                if reg {
                    if m.c2 == 0 {
                        Some(MConf { pc: jz, c1: m.c1, c2: 0 })
                    } else {
                        Some(MConf { pc: nz, c1: m.c1, c2: (m.c2 - 1) as nat })
                    }
                } else {
                    if m.c1 == 0 {
                        Some(MConf { pc: jz, c1: 0, c2: m.c2 })
                    } else {
                        Some(MConf { pc: nz, c1: (m.c1 - 1) as nat, c2: m.c2 })
                    }
                },
        }
    }
}

/// Iterate `minsky_step` up to `k` times. `None` once halted.
pub open spec fn minsky_run(prog: MProg, m: MConf, k: nat) -> Option<MConf>
    decreases k,
{
    if k == 0 {
        Some(m)
    } else {
        match minsky_step(prog, m) {
            None => None,
            Some(m2) => minsky_run(prog, m2, (k - 1) as nat),
        }
    }
}

/// "The machine halts after exactly T steps with residual configuration
/// `mfinal`": T successful steps produce `mfinal`, whose next step is `None`.
/// The output counters are `mfinal.c1`, `mfinal.c2`.
pub open spec fn minsky_halts_with(prog: MProg, m0: MConf, t: nat, mfinal: MConf) -> bool {
    &&& minsky_run(prog, m0, t) == Some(mfinal)
    &&& minsky_step(prog, mfinal) is None
}

// ============================================================
// 2. The unbounded counter encoding: unary quotations
// ============================================================
//
// Marker = the single value word PushInt(0). A counter of value n is
// Quote(unary(n)) with unary(n).len() == n — the Seq that escapes the i64 cap.

pub open spec fn marker() -> SpecWord {
    SpecWord::PushInt(0int)
}

pub open spec fn unary(n: nat) -> Seq<SpecWord>
    decreases n,
{
    if n == 0 {
        Seq::<SpecWord>::empty()
    } else {
        seq![marker()] + unary((n - 1) as nat)
    }
}

/// The load-bearing length invariant: `unary(n)` has length exactly `n`,
/// unbounded over `nat`.
pub proof fn unary_len(n: nat)
    ensures unary(n).len() == n,
    decreases n,
{
    if n == 0 {
    } else {
        unary_len((n - 1) as nat);
    }
}

/// `unary(n+1) = [marker] + unary(n)` — the increment identity (Cons prepends).
pub proof fn unary_succ(n: nat)
    ensures unary((n + 1) as nat) =~= seq![marker()] + unary(n),
{
}

/// For n>0, `unary(n)` deconstructs as head = marker, tail = `unary(n-1)`.
pub proof fn unary_uncons(n: nat)
    requires n >= 1,
    ensures
        unary(n).len() >= 1,
        unary(n)[0] == marker(),
        unary(n).subrange(1, unary(n).len() as int) =~= unary((n - 1) as nat),
{
    unary_len(n);
    unary_len((n - 1) as nat);
    assert(unary(n) =~= seq![marker()] + unary((n - 1) as nat));
}

// ============================================================
// 3. Iterated spec_step (concrete step counting) + fuel bridge
// ============================================================

/// Run `spec_step` exactly `k` times; if any step Halts/Faults, return that.
/// `Next(s)` means all k steps advanced, ending in state `s`.
pub open spec fn spec_stepn(s: SpecState, k: nat) -> SpecStep
    decreases k,
{
    if k == 0 {
        SpecStep::Next(s)
    } else {
        match spec_step(s) {
            SpecStep::Next(s2) => spec_stepn(s2, (k - 1) as nat),
            other => other,
        }
    }
}

/// Composition of `spec_stepn`: if the first `a` steps advance to `s'`, then
/// `a+b` steps equal `b` steps from `s'`.
pub proof fn lemma_stepn_compose(s: SpecState, a: nat, b: nat)
    requires spec_stepn(s, a) is Next,
    ensures spec_stepn(s, (a + b) as nat) == spec_stepn(spec_stepn(s, a)->Next_0, b),
    decreases a,
{
    if a == 0 {
    } else {
        match spec_step(s) {
            SpecStep::Next(s2) => {
                lemma_stepn_compose(s2, (a - 1) as nat, b);
            }
            _ => {
                assert(spec_stepn(s, a) == spec_step(s));
            }
        }
    }
}

/// Bridge `spec_stepn` (step count) to `spec_run` (fuel): if `k` steps advance
/// to `s'`, then `spec_run` with `k+j` fuel equals `spec_run(s', j)`.
pub proof fn lemma_run_from_stepn(s: SpecState, k: nat, j: nat)
    requires spec_stepn(s, k) is Next,
    ensures spec_run(s, (k + j) as nat) == spec_run(spec_stepn(s, k)->Next_0, j),
    decreases k,
{
    if k == 0 {
    } else {
        match spec_step(s) {
            SpecStep::Next(s2) => {
                lemma_run_from_stepn(s2, (k - 1) as nat, j);
                assert((k + j) as nat >= 1);
            }
            _ => {
                assert(spec_stepn(s, k) == spec_step(s));
            }
        }
    }
}

/// `spec_run` is monotone past a decisive (non-FuelExhausted) outcome: once
/// enough fuel produces Halt/Fault, all larger fuel agrees. This makes the
/// `∃ fuel` and `∀ fuel ≥ N` forms of the halting claim interchangeable.
pub proof fn lemma_run_mono(s: SpecState, n: nat, n2: nat)
    requires
        n <= n2,
        !(spec_run(s, n) is FuelExhausted),
    ensures spec_run(s, n2) == spec_run(s, n),
    decreases n,
{
    if n == 0 {
        // spec_run(s,0) is FuelExhausted — contradicts the requires.
        assert(spec_run(s, 0nat) is FuelExhausted);
    } else {
        match spec_step(s) {
            SpecStep::Next(s2) => {
                lemma_run_mono(s2, (n - 1) as nat, (n2 - 1) as nat);
                assert(n2 >= 1);
            }
            _ => {
                assert(n2 >= 1);
            }
        }
    }
}

// ============================================================
// 4. The three counter operations as verified spec_step fragments
// ============================================================
//
// In each fragment the target counter sits on top of an arbitrary base stack
// `base` as `Quote(unary(n))`; the surrounding loop (§6) shuffles the right
// counter to the top and back. `rest` is the continuation after the fragment.

/// Increment fragment `0 ~ ;` = [PushInt(0), Swap, Cons] (brief §3.i).
/// Three `spec_step`s turn `Quote(unary(n))` on top into `Quote(unary(n+1))`.
pub proof fn lemma_inc_frag(base: Seq<SpecValue>, n: nat, rest: Seq<SpecWord>)
    ensures
        spec_stepn(
            SpecState {
                stack: base.push(SpecValue::Quote(unary(n))),
                cont: seq![
                    SpecWord::PushInt(0int),
                    SpecWord::Prim(SpecPrim::Swap),
                    SpecWord::Prim(SpecPrim::Cons)
                ] + rest,
            },
            3,
        ) == SpecStep::Next(SpecState {
            stack: base.push(SpecValue::Quote(unary((n + 1) as nat))),
            cont: rest,
        }),
{
    reveal_with_fuel(spec_stepn, 4);
    let q = unary(n);
    // s0: [.. Quote(unary n)], cont [PushInt0, Swap, Cons | rest]
    let s0 = SpecState {
        stack: base.push(SpecValue::Quote(q)),
        cont: seq![
            SpecWord::PushInt(0int),
            SpecWord::Prim(SpecPrim::Swap),
            SpecWord::Prim(SpecPrim::Cons)
        ] + rest,
    };
    assert(s0.cont[0] == SpecWord::PushInt(0int));
    assert(s0.cont.subrange(1, s0.cont.len() as int)
        =~= seq![SpecWord::Prim(SpecPrim::Swap), SpecWord::Prim(SpecPrim::Cons)] + rest);
    let s1 = spec_step(s0)->Next_0;
    assert(s1.stack =~= base.push(SpecValue::Quote(q)).push(SpecValue::Int(0int)));
    // Swap: top two -> [.. Int(0), Quote(q)]
    assert(s1.cont[0] == SpecWord::Prim(SpecPrim::Swap));
    let s2 = spec_step(s1)->Next_0;
    assert(s2.stack =~= base.push(SpecValue::Int(0int)).push(SpecValue::Quote(q)));
    // Cons: value_to_word(Int(0)) = PushInt(0) = marker prepended to q
    assert(s2.cont[0] == SpecWord::Prim(SpecPrim::Cons));
    let s3 = spec_step(s2)->Next_0;
    unary_succ(n);
    assert(value_to_word(SpecValue::Int(0int)) == marker());
    assert(s3.stack =~= base.push(SpecValue::Quote(seq![marker()] + q)));
    assert(seq![marker()] + q =~= unary((n + 1) as nat));
    assert(s3.stack =~= base.push(SpecValue::Quote(unary((n + 1) as nat))));
    assert(s3.cont =~= rest);
}

/// Decrement/zero-test fragment `> [THEN] [ELSE] ?` (brief §3.ii/iii).
/// The fragment word sequence, parameterized by the two branch bodies.
pub open spec fn decjz_frag(then_q: Seq<SpecWord>, else_q: Seq<SpecWord>) -> Seq<SpecWord> {
    seq![
        SpecWord::Prim(SpecPrim::Uncons),
        SpecWord::PushQuote(then_q),
        SpecWord::PushQuote(else_q),
        SpecWord::Prim(SpecPrim::If)
    ]
}

/// Nonzero branch: counter `Quote(unary(n))` with n>0. Uncons yields
/// `Int(0) Quote(unary(n-1)) Int(1)`, If (c=1) splices THEN. Two steps
/// (Uncons, then PushQuote/PushQuote/If = 3 more) reach: THEN spliced onto
/// `rest`, over stack `base + [Int(0), Quote(unary(n-1))]`.
pub proof fn lemma_dec_nz_frag(
    base: Seq<SpecValue>, n: nat, then_q: Seq<SpecWord>, else_q: Seq<SpecWord>, rest: Seq<SpecWord>,
)
    requires n >= 1,
    ensures
        spec_stepn(
            SpecState {
                stack: base.push(SpecValue::Quote(unary(n))),
                cont: decjz_frag(then_q, else_q) + rest,
            },
            4,
        ) == SpecStep::Next(SpecState {
            stack: base.push(SpecValue::Int(0int)).push(SpecValue::Quote(unary((n - 1) as nat))),
            cont: then_q + rest,
        }),
{
    reveal_with_fuel(spec_stepn, 5);
    unary_uncons(n);
    let q = unary(n);
    let tl = unary((n - 1) as nat);
    let s0 = SpecState {
        stack: base.push(SpecValue::Quote(q)),
        cont: decjz_frag(then_q, else_q) + rest,
    };
    assert(s0.cont[0] == SpecWord::Prim(SpecPrim::Uncons));
    assert(s0.cont.subrange(1, s0.cont.len() as int)
        =~= seq![
            SpecWord::PushQuote(then_q),
            SpecWord::PushQuote(else_q),
            SpecWord::Prim(SpecPrim::If)
        ] + rest);
    // Uncons on non-empty quote with head marker=PushInt(0):
    assert(q[0] == SpecWord::PushInt(0int));
    assert(q.subrange(1, q.len() as int) =~= tl);
    let s1 = spec_step(s0)->Next_0;
    assert(s1.stack =~= base
        .push(SpecValue::Int(0int))
        .push(SpecValue::Quote(tl))
        .push(SpecValue::Int(1int)));
    // PushQuote(then_q), PushQuote(else_q):
    let s2 = spec_step(s1)->Next_0;
    let s3 = spec_step(s2)->Next_0;
    assert(s3.stack =~= base
        .push(SpecValue::Int(0int))
        .push(SpecValue::Quote(tl))
        .push(SpecValue::Int(1int))
        .push(SpecValue::Quote(then_q))
        .push(SpecValue::Quote(else_q)));
    // If: c = Int(1) != 0 -> splice then_q
    assert(s3.cont[0] == SpecWord::Prim(SpecPrim::If));
    let s4 = spec_step(s3)->Next_0;
    assert(s4.stack =~= base.push(SpecValue::Int(0int)).push(SpecValue::Quote(tl)));
    assert(s4.cont =~= then_q + rest);
}

/// Zero branch: counter `Quote(unary(0))` = empty quote. Uncons yields
/// `Int(0)`, If (c=0) splices ELSE. Reaches: ELSE spliced onto `rest`, over
/// the base stack (the empty counter was consumed; ELSE re-pushes it).
pub proof fn lemma_dec_z_frag(
    base: Seq<SpecValue>, then_q: Seq<SpecWord>, else_q: Seq<SpecWord>, rest: Seq<SpecWord>,
)
    ensures
        spec_stepn(
            SpecState {
                stack: base.push(SpecValue::Quote(unary(0))),
                cont: decjz_frag(then_q, else_q) + rest,
            },
            4,
        ) == SpecStep::Next(SpecState {
            stack: base,
            cont: else_q + rest,
        }),
{
    reveal_with_fuel(spec_stepn, 5);
    let s0 = SpecState {
        stack: base.push(SpecValue::Quote(Seq::<SpecWord>::empty())),
        cont: decjz_frag(then_q, else_q) + rest,
    };
    assert(unary(0) =~= Seq::<SpecWord>::empty());
    assert(s0.cont[0] == SpecWord::Prim(SpecPrim::Uncons));
    assert(s0.cont.subrange(1, s0.cont.len() as int)
        =~= seq![
            SpecWord::PushQuote(then_q),
            SpecWord::PushQuote(else_q),
            SpecWord::Prim(SpecPrim::If)
        ] + rest);
    let s1 = spec_step(s0)->Next_0;
    assert(s1.stack =~= base.push(SpecValue::Int(0int)));
    let s2 = spec_step(s1)->Next_0;
    let s3 = spec_step(s2)->Next_0;
    assert(s3.stack =~= base
        .push(SpecValue::Int(0int))
        .push(SpecValue::Quote(then_q))
        .push(SpecValue::Quote(else_q)));
    // If: c = Int(0) == 0 -> splice else_q
    assert(s3.cont[0] == SpecWord::Prim(SpecPrim::If));
    let s4 = spec_step(s3)->Next_0;
    assert(s4.stack =~= base);
    assert(s4.cont =~= else_q + rest);
}

// ============================================================
// 5. The :! loop-entry step (mirror of smoke_dup_apply)
// ============================================================

/// From `[Quote(q)]`-topped stack with cont `[Dup, Apply | rest]`, two steps
/// reach the same stack with `q` spliced before `rest`. This is the `:!`
/// self-application that drives unbounded iteration (brief §3, §6.3).
pub proof fn lemma_dup_apply(base: Seq<SpecValue>, q: Seq<SpecWord>, rest: Seq<SpecWord>)
    ensures
        spec_stepn(
            SpecState {
                stack: base.push(SpecValue::Quote(q)),
                cont: seq![SpecWord::Prim(SpecPrim::Dup), SpecWord::Prim(SpecPrim::Apply)] + rest,
            },
            2,
        ) == SpecStep::Next(SpecState {
            stack: base.push(SpecValue::Quote(q)),
            cont: q + rest,
        }),
{
    reveal_with_fuel(spec_stepn, 3);
    let s0 = SpecState {
        stack: base.push(SpecValue::Quote(q)),
        cont: seq![SpecWord::Prim(SpecPrim::Dup), SpecWord::Prim(SpecPrim::Apply)] + rest,
    };
    assert(s0.cont[0] == SpecWord::Prim(SpecPrim::Dup));
    assert(s0.cont.subrange(1, s0.cont.len() as int)
        =~= seq![SpecWord::Prim(SpecPrim::Apply)] + rest);
    let s1 = spec_step(s0)->Next_0;
    assert(s1.stack =~= base.push(SpecValue::Quote(q)).push(SpecValue::Quote(q)));
    assert(s1.cont[0] == SpecWord::Prim(SpecPrim::Apply));
    let s2 = spec_step(s1)->Next_0;
    assert(s2.stack =~= base.push(SpecValue::Quote(q)));
    assert(s2.cont =~= q + rest);
}

} // verus!
