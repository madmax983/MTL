//! MTL core — Verus specification (SPEC phase of TAVDD).
//!
//! Settled in this revision (verified, not asserted):
//!   * Division/modulo pinned to TRUNCATING semantics, matching
//!     i64::checked_div / checked_rem exactly (incl. MIN/-1 -> Overflow).
//!   * All primitive arms filled; the match is exhaustive with no wildcard.
//!   * Deep view (exec Word/Value -> ghost model) implemented; the
//!     nested-quotation termination hole is closed.
//!
//! Verify with:  verus mtl_core.rs
//! (Windows path fallback: C:\Users\markm\verus\verus.exe mtl_core.rs)

use vstd::prelude::*;

verus! {

// ============================================================
// 1. Ghost model
// ============================================================

#[derive(Copy, Clone)]
pub enum SpecPrim {
    Dup, Drop, Swap, Rot, Over,
    Apply, Cat, Cons, Dip,
    Add, Sub, Mul, Div, Mod,
    Eq, Lt,
    If,
}

pub enum SpecWord {
    PushInt(int),
    PushQuote(Seq<SpecWord>),
    Prim(SpecPrim),
    Call(Seq<char>),          // host capability / definition name
}

pub enum SpecValue {
    Int(int),
    Quote(Seq<SpecWord>),
}

pub struct SpecState {
    pub stack: Seq<SpecValue>,
    pub cont: Seq<SpecWord>,
}

pub enum Error {
    Underflow,
    TypeMismatch,
    Overflow,
    DivByZero,
    UnknownWord,
}

pub enum SpecStep {
    Next(SpecState),
    Halt(Seq<SpecValue>),
    Fault(Error),
}

// ------------------------------------------------------------
// i64 range predicate — where the Overflow obligation lives.
// ------------------------------------------------------------
pub open spec fn in_i64(n: int) -> bool {
    -0x8000_0000_0000_0000 <= n <= 0x7FFF_FFFF_FFFF_FFFF
}

// ------------------------------------------------------------
// PINNED: truncating division & remainder (Rust semantics).
//
// SMT-LIB `/` and `%` on int are Euclidean; Rust's i64 `/` and `%`
// truncate toward zero. The spec models RUST, so P2 (refinement)
// never fights a semantics mismatch on negative operands:
//   trunc_div(-7, 2) == -3   (Euclidean would give -4)
//   trunc_mod(-7, 2) == -1   (remainder takes the sign of the dividend)
// Overflow faults mirror checked_div/checked_rem exactly:
//   b == 0                    -> DivByZero
//   a == i64::MIN && b == -1  -> Overflow   (for BOTH div and mod,
//     because checked_rem(MIN, -1) is None even though the
//     mathematical remainder is 0 — the spec matches the exec truth).
// ------------------------------------------------------------

pub open spec fn abs_int(a: int) -> int {
    if a >= 0 { a } else { -a }
}

pub open spec fn trunc_div(a: int, b: int) -> int
    recommends b != 0
{
    // Euclidean == floor == truncating on nonnegative operands,
    // so route through |a| / |b| and reattach the sign.
    let q = abs_int(a) / abs_int(b);
    if (a >= 0) == (b >= 0) { q } else { -q }
}

pub open spec fn trunc_mod(a: int, b: int) -> int
    recommends b != 0
{
    a - trunc_div(a, b) * b
}

// ============================================================
// 2. Total small-step semantics (transcription of spec §4.1)
//    Total by construction: every arm returns, no wildcard,
//    no partial match. This *is* invariant I1. No `decreases`
//    needed — spec_step does not recurse.
// ============================================================

pub open spec fn value_to_word(v: SpecValue) -> SpecWord {
    match v {
        SpecValue::Int(i) => SpecWord::PushInt(i),
        SpecValue::Quote(q) => SpecWord::PushQuote(q),
    }
}

pub open spec fn spec_step(s: SpecState) -> SpecStep {
    if s.cont.len() == 0 {
        SpecStep::Halt(s.stack)
    } else {
        let w = s.cont[0];
        let rest = s.cont.subrange(1, s.cont.len() as int);
        match w {
            SpecWord::PushInt(n) => SpecStep::Next(SpecState {
                stack: s.stack.push(SpecValue::Int(n)),
                cont: rest,
            }),
            SpecWord::PushQuote(q) => SpecStep::Next(SpecState {
                stack: s.stack.push(SpecValue::Quote(q)),
                cont: rest,
            }),
            SpecWord::Prim(p) => spec_step_prim(s.stack, p, rest),
            SpecWord::Call(_) =>
                // Core semantics: unknown. Host binding is a trusted
                // boundary outside the verified core (spec §8).
                SpecStep::Fault(Error::UnknownWord),
        }
    }
}

pub open spec fn spec_step_prim(
    stk: Seq<SpecValue>, p: SpecPrim, rest: Seq<SpecWord>,
) -> SpecStep {
    let n = stk.len() as int;
    match p {
        // ---------------- stack shuffling ----------------
        SpecPrim::Dup => {
            if n < 1 { SpecStep::Fault(Error::Underflow) }
            else {
                SpecStep::Next(SpecState {
                    stack: stk.push(stk[n - 1]),
                    cont: rest,
                })
            }
        }
        SpecPrim::Drop => {
            if n < 1 { SpecStep::Fault(Error::Underflow) }
            else {
                SpecStep::Next(SpecState {
                    stack: stk.subrange(0, n - 1),
                    cont: rest,
                })
            }
        }
        SpecPrim::Swap => {
            if n < 2 { SpecStep::Fault(Error::Underflow) }
            else {
                SpecStep::Next(SpecState {
                    stack: stk.subrange(0, n - 2)
                              .push(stk[n - 1])
                              .push(stk[n - 2]),
                    cont: rest,
                })
            }
        }
        SpecPrim::Rot => {
            // ( a b c -- b c a )
            if n < 3 { SpecStep::Fault(Error::Underflow) }
            else {
                SpecStep::Next(SpecState {
                    stack: stk.subrange(0, n - 3)
                              .push(stk[n - 2])
                              .push(stk[n - 1])
                              .push(stk[n - 3]),
                    cont: rest,
                })
            }
        }
        SpecPrim::Over => {
            // ( a b -- a b a )
            if n < 2 { SpecStep::Fault(Error::Underflow) }
            else {
                SpecStep::Next(SpecState {
                    stack: stk.push(stk[n - 2]),
                    cont: rest,
                })
            }
        }
        // ---------------- quotation algebra ----------------
        SpecPrim::Apply => {
            if n < 1 { SpecStep::Fault(Error::Underflow) }
            else {
                match stk[n - 1] {
                    // Splice into the continuation — flat step relation,
                    // proper tail calls (spec §4.2).
                    SpecValue::Quote(q) => SpecStep::Next(SpecState {
                        stack: stk.subrange(0, n - 1),
                        cont: q + rest,
                    }),
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        SpecPrim::Cat => {
            if n < 2 { SpecStep::Fault(Error::Underflow) }
            else {
                match (stk[n - 2], stk[n - 1]) {
                    (SpecValue::Quote(a), SpecValue::Quote(b)) =>
                        SpecStep::Next(SpecState {
                            stack: stk.subrange(0, n - 2)
                                      .push(SpecValue::Quote(a + b)),
                            cont: rest,
                        }),
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        SpecPrim::Cons => {
            if n < 2 { SpecStep::Fault(Error::Underflow) }
            else {
                match stk[n - 1] {
                    SpecValue::Quote(q) => SpecStep::Next(SpecState {
                        stack: stk.subrange(0, n - 2).push(SpecValue::Quote(
                            seq![value_to_word(stk[n - 2])] + q)),
                        cont: rest,
                    }),
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        SpecPrim::Dip => {
            // ( a [q] -- ... a ) : run q with a set aside, restore a.
            // The restore is compiled into the continuation itself, so
            // the "scoped borrow" reading in spec §14.4 is literal.
            if n < 2 { SpecStep::Fault(Error::Underflow) }
            else {
                match stk[n - 1] {
                    SpecValue::Quote(q) => SpecStep::Next(SpecState {
                        stack: stk.subrange(0, n - 2),
                        cont: q + seq![value_to_word(stk[n - 2])] + rest,
                    }),
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        // ---------------- arithmetic (checked, truncating) ----------------
        SpecPrim::Add => spec_arith(stk, rest, |a: int, b: int| a + b),
        SpecPrim::Sub => spec_arith(stk, rest, |a: int, b: int| a - b),
        SpecPrim::Mul => spec_arith(stk, rest, |a: int, b: int| a * b),
        SpecPrim::Div => spec_divmod(stk, rest, true),
        SpecPrim::Mod => spec_divmod(stk, rest, false),
        // ---------------- comparison ----------------
        SpecPrim::Eq => {
            if n < 2 { SpecStep::Fault(Error::Underflow) }
            else {
                match (stk[n - 2], stk[n - 1]) {
                    (SpecValue::Int(a), SpecValue::Int(b)) =>
                        SpecStep::Next(SpecState {
                            stack: stk.subrange(0, n - 2).push(
                                SpecValue::Int(if a == b { 1int } else { 0int })),
                            cont: rest,
                        }),
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        SpecPrim::Lt => {
            if n < 2 { SpecStep::Fault(Error::Underflow) }
            else {
                match (stk[n - 2], stk[n - 1]) {
                    (SpecValue::Int(a), SpecValue::Int(b)) =>
                        SpecStep::Next(SpecState {
                            stack: stk.subrange(0, n - 2).push(
                                SpecValue::Int(if a < b { 1int } else { 0int })),
                            cont: rest,
                        }),
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        // ---------------- branch ----------------
        SpecPrim::If => {
            if n < 3 { SpecStep::Fault(Error::Underflow) }
            else {
                match (stk[n - 3], stk[n - 2], stk[n - 1]) {
                    (SpecValue::Int(c), SpecValue::Quote(t), SpecValue::Quote(f)) =>
                        SpecStep::Next(SpecState {
                            stack: stk.subrange(0, n - 3),
                            cont: (if c != 0 { t } else { f }) + rest,
                        }),
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
    }
}

pub open spec fn spec_arith(
    stk: Seq<SpecValue>, rest: Seq<SpecWord>,
    op: spec_fn(int, int) -> int,
) -> SpecStep {
    let n = stk.len() as int;
    if n < 2 { SpecStep::Fault(Error::Underflow) }
    else {
        match (stk[n - 2], stk[n - 1]) {
            (SpecValue::Int(a), SpecValue::Int(b)) => {
                let r = op(a, b);
                if in_i64(r) {
                    SpecStep::Next(SpecState {
                        stack: stk.subrange(0, n - 2).push(SpecValue::Int(r)),
                        cont: rest,
                    })
                } else {
                    SpecStep::Fault(Error::Overflow)
                }
            }
            _ => SpecStep::Fault(Error::TypeMismatch),
        }
    }
}

// Div and Mod share fault structure exactly (checked_div/checked_rem
// have identical None conditions), so one definition serves both.
pub open spec fn spec_divmod(
    stk: Seq<SpecValue>, rest: Seq<SpecWord>, is_div: bool,
) -> SpecStep {
    let n = stk.len() as int;
    if n < 2 { SpecStep::Fault(Error::Underflow) }
    else {
        match (stk[n - 2], stk[n - 1]) {
            (SpecValue::Int(a), SpecValue::Int(b)) => {
                if b == 0 {
                    SpecStep::Fault(Error::DivByZero)
                } else if !in_i64(trunc_div(a, b)) {
                    // Only i64::MIN / -1 lands here; faults for BOTH
                    // div and mod, mirroring checked_rem.
                    SpecStep::Fault(Error::Overflow)
                } else {
                    let r = if is_div { trunc_div(a, b) } else { trunc_mod(a, b) };
                    SpecStep::Next(SpecState {
                        stack: stk.subrange(0, n - 2).push(SpecValue::Int(r)),
                        cont: rest,
                    })
                }
            }
            _ => SpecStep::Fault(Error::TypeMismatch),
        }
    }
}

// ============================================================
// 3. Exec side (GREEN phase implements; deep view is the bridge)
// ============================================================

pub enum Word {
    PushInt(i64),
    PushQuote(Vec<Word>),
    Prim(SpecPrim),
    Call(Vec<char>),
}

pub enum Value {
    Int(i64),
    Quote(Vec<Word>),
}

// Deep view: exec AST -> ghost model. Mutual recursion through the
// Vec field terminates via a lexicographic measure on Verus's
// built-in datatype height (elements of a Vec field sit strictly
// below the containing value), with seq length as the tiebreaker
// for the peeling recursion.
pub open spec fn view_word(w: Word) -> SpecWord
    decreases w, 0nat,
{
    match w {
        Word::PushInt(n) => SpecWord::PushInt(n as int),
        Word::PushQuote(q) => SpecWord::PushQuote(view_words(q@)),
        Word::Prim(p) => SpecWord::Prim(p),
        Word::Call(cs) => SpecWord::Call(cs@),
    }
}

pub open spec fn view_words(ws: Seq<Word>) -> Seq<SpecWord>
    decreases ws, ws.len(),
{
    if ws.len() == 0 {
        seq![]
    } else {
        seq![view_word(ws[0])] + view_words(ws.subrange(1, ws.len() as int))
    }
}

pub open spec fn view_value(v: Value) -> SpecValue {
    match v {
        Value::Int(n) => SpecValue::Int(n as int),
        Value::Quote(q) => SpecValue::Quote(view_words(q@)),
    }
}

pub struct Vm {
    pub stack: Vec<Value>,
    pub cont: Vec<Word>,
}

pub enum StepResult { Next, Halt, Fault(Error) }

// The exec step — GREEN phase. Refinement contract (P2):
//
// fn exec_step(vm: &mut Vm) -> (r: StepResult)
//     ensures match spec_step(old(vm).deep_view()) {
//         SpecStep::Next(s')  => r is Next  && vm.deep_view() == s',
//         SpecStep::Halt(_)   => r is Halt  && vm unchanged,
//         SpecStep::Fault(e)  => r == StepResult::Fault(e),
//     }
//
// Arithmetic implements via checked_add/sub/mul/div/rem — the spec's
// in_i64 / trunc_div structure above is deliberately isomorphic to
// the None-conditions of those intrinsics, so P2's arithmetic cases
// should discharge by case analysis without bespoke lemmas.
//
// Driver (fuel-bounded; termination NOT provable — MTL is TC):
// fn run(vm: &mut Vm, fuel: u64) -> (o: Outcome)
//     ensures o == FuelExhausted || o matches spec iteration <= fuel steps

// ============================================================
// 4. Proof obligations (spine proofs — PROOF phase)
// ============================================================

// P3 — Progress: every state is Next, Halt, or Fault.
// Discharged by totality of spec_step; stated as a named theorem so
// the property survives refactors as a regression guard.
pub proof fn p3_progress(s: SpecState)
    ensures
        spec_step(s) is Next
        || spec_step(s) is Halt
        || spec_step(s) is Fault,
{
}

// P1 — Determinism: spec_step is a spec fn, hence a function.
// The §4.1 rule patterns are non-overlapping by the match structure.

// --- Division semantics pinned: verified witnesses, not comments. ---
pub proof fn div_semantics_witnesses()
    ensures
        trunc_div(-7, 2) == -3,      // Euclidean would say -4
        trunc_mod(-7, 2) == -1,      // sign follows the dividend
        trunc_div(7, -2) == -3,
        trunc_mod(7, -2) == 1,
        trunc_div(-7, -2) == 3,
        trunc_mod(-7, -2) == -1,
        !in_i64(trunc_div(-0x8000_0000_0000_0000, -1)),  // MIN/-1 overflows
{
    assert(trunc_div(-7, 2) == -3) by (compute);
    assert(trunc_mod(-7, 2) == -1) by (compute);
    assert(trunc_div(7, -2) == -3) by (compute);
    assert(trunc_mod(7, -2) == 1) by (compute);
    assert(trunc_div(-7, -2) == 3) by (compute);
    assert(trunc_mod(-7, -2) == -1) by (compute);
    assert(!in_i64(trunc_div(-0x8000_0000_0000_0000, -1))) by (compute);
}

// --- Smoke theorem: the two-token Y idiom `: !` (dup, apply). ---
// From stack [Quote(q)] with continuation [dup, apply], two spec
// steps reach stack [Quote(q)] with continuation q — self-application
// splices the body while retaining the quotation. Unbounded recursion
// in two tokens, verified.
pub proof fn smoke_dup_apply(q: Seq<SpecWord>)
    ensures ({
        let s0 = SpecState {
            stack: seq![SpecValue::Quote(q)],
            cont: seq![SpecWord::Prim(SpecPrim::Dup), SpecWord::Prim(SpecPrim::Apply)],
        };
        &&& spec_step(s0) is Next
        &&& {
            let s1 = spec_step(s0)->Next_0;
            &&& spec_step(s1) is Next
            &&& spec_step(s1)->Next_0.stack =~= seq![SpecValue::Quote(q)]
            &&& spec_step(s1)->Next_0.cont =~= q
        }
    }),
{
}

// --- General div/mod correctness: the spine lemma P2's arithmetic
// cases lean on. Defining properties of truncating division:
//   a == q*b + r,  |r| < |b|,  r == 0 or sign(r) == sign(a).
pub proof fn trunc_divmod_correct(a: int, b: int)
    requires b != 0,
    ensures
        trunc_div(a, b) * b + trunc_mod(a, b) == a,
        abs_int(trunc_mod(a, b)) < abs_int(b),
        trunc_mod(a, b) == 0 || (trunc_mod(a, b) > 0) == (a > 0),
{
    let aa = abs_int(a);
    let ab = abs_int(b);
    let q = aa / ab;
    let r = aa % ab;
    assert(aa == ab * q + r && 0 <= r < ab) by (nonlinear_arith)
        requires aa >= 0, ab > 0, q == aa / ab, r == aa % ab;
    assert(trunc_div(a, b) * b + trunc_mod(a, b) == a);  // definitional
    assert(abs_int(trunc_mod(a, b)) < abs_int(b)) by (nonlinear_arith)
        requires aa == ab * q + r, 0 <= r < ab,
                 aa == abs_int(a), ab == abs_int(b), q == aa / ab,
                 b != 0;
    assert(trunc_mod(a, b) == 0 || (trunc_mod(a, b) > 0) == (a > 0))
        by (nonlinear_arith)
        requires aa == ab * q + r, 0 <= r < ab,
                 aa == abs_int(a), ab == abs_int(b), q == aa / ab,
                 b != 0;
}

// P5 — TC lock-step lemma (spec §6): Minsky simulation invariant R
// preserved by bounded MTL step sequences. Stated in PROOF phase after
// the Minsky spec machine is transcribed. Hard; scheduled last.

} // verus!

fn main() {}
