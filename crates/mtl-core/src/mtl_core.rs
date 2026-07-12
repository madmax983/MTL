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
use vstd::arithmetic::div_mod::{rust_div, rust_rem, lemma_fundamental_div_mod};

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
    // v0.2 recursion primitives (design: docs/design/v0.2-recursion-primitives.md).
    // primrec/times: bounded, total, terminating (count strictly decreases).
    // linrec: partial, DESUGARS into If (inherits verified branch semantics).
    // uncons: structural quotation deconstructor (the TC-proof enabler, §6.2).
    PrimRec, Times, LinRec, Uncons,
    // v0.3 sequence primitives (design: docs/design/v0.3-sequences.md §3).
    // fold: native LEFT fold over a cons-list; terminating (the spine strictly
    //   shrinks, exactly like primrec's count) and total on a finite seq.
    // xor: total i64 two's-complement bitwise XOR (arity->type only, no Overflow).
    Fold, Xor,
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
// Bitwise XOR on the i64 two's-complement representation (v0.3 `xor`).
//
// MTL ints are i64 (spec §3), so `xor` is defined on the 64-bit two's-complement
// bit pattern — exactly Rust's `i64 ^ i64`. The stack invariant keeps operands
// in_i64, so the `as i64` casts are lossless and the result is ALWAYS in i64
// range: there is NO Overflow arm (contrast spec_arith, which checks in_i64).
// This is the spec twin of the exec `a ^ b` in `crate::interp` / `exec_prim`.
// ------------------------------------------------------------
pub open spec fn i64_bitxor(a: int, b: int) -> int {
    ((a as i64) ^ (b as i64)) as int
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
        // ---------------- v0.2 recursion primitives (spec §3 of the design) ----------------
        // primrec ( n [I] [C] -- r ): total primitive recursion on a natural count.
        // n<=0 runs I on the base stack; n>0 keeps n available and folds C over
        // the (n-1) subresult. Terminating: the count strictly decreases toward 0.
        SpecPrim::PrimRec => {
            if n < 3 { SpecStep::Fault(Error::Underflow) }
            else {
                match (stk[n - 3], stk[n - 2], stk[n - 1]) {
                    (SpecValue::Int(k), SpecValue::Quote(qi), SpecValue::Quote(qc)) => {
                        let base = stk.subrange(0, n - 3);
                        if k <= 0 {
                            // base: discard the count, run I on the base stack.
                            SpecStep::Next(SpecState { stack: base, cont: qi + rest })
                        } else {
                            // else: keep n, recurse on (n-1), then run C(n, sub).
                            // k>0 && k<=i64::MAX => 0 <= k-1 < k, so in_i64(k-1)
                            // holds: no Overflow arm is reachable here.
                            let recur = seq![
                                SpecWord::PushInt(k),
                                SpecWord::PushInt(k - 1),
                                SpecWord::PushQuote(qi),
                                SpecWord::PushQuote(qc),
                                SpecWord::Prim(SpecPrim::PrimRec)
                            ] + qc;
                            SpecStep::Next(SpecState { stack: base, cont: recur + rest })
                        }
                    }
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        // times ( n [Q] -- ... ): run Q exactly max(n,0) times, left to right.
        // Total and terminating (count decreases; n<=0 is a no-op).
        SpecPrim::Times => {
            if n < 2 { SpecStep::Fault(Error::Underflow) }
            else {
                match (stk[n - 2], stk[n - 1]) {
                    (SpecValue::Int(k), SpecValue::Quote(q)) => {
                        let base = stk.subrange(0, n - 2);
                        if k <= 0 {
                            SpecStep::Next(SpecState { stack: base, cont: rest })
                        } else {
                            // run Q once, then times(n-1) Q.
                            let recur = q + seq![
                                SpecWord::PushInt(k - 1),
                                SpecWord::PushQuote(q),
                                SpecWord::Prim(SpecPrim::Times)
                            ];
                            SpecStep::Next(SpecState { stack: base, cont: recur + rest })
                        }
                    }
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        // linrec ( [P] [T] [R1] [R2] -- ... ): general linear recursion. DESUGARS
        // into the existing If primitive — no new control operator — so it inherits
        // If's verified branch semantics. Partial, like Apply: termination depends
        // on P/R1 and is bounded by fuel.
        SpecPrim::LinRec => {
            if n < 4 { SpecStep::Fault(Error::Underflow) }
            else {
                match (stk[n - 4], stk[n - 3], stk[n - 2], stk[n - 1]) {
                    (SpecValue::Quote(qp), SpecValue::Quote(qt),
                     SpecValue::Quote(qr1), SpecValue::Quote(qr2)) => {
                        let base = stk.subrange(0, n - 4);
                        // else-branch: R1 ; (re-push the four quotes) linrec ; R2
                        let else_q = qr1 + seq![
                            SpecWord::PushQuote(qp),
                            SpecWord::PushQuote(qt),
                            SpecWord::PushQuote(qr1),
                            SpecWord::PushQuote(qr2),
                            SpecWord::Prim(SpecPrim::LinRec)
                        ] + qr2;
                        // continuation: P ; push T-quote ; push else-quote ; If
                        let spliced = qp + seq![
                            SpecWord::PushQuote(qt),
                            SpecWord::PushQuote(else_q),
                            SpecWord::Prim(SpecPrim::If)
                        ];
                        SpecStep::Next(SpecState { stack: base, cont: spliced + rest })
                    }
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        // uncons ( [w ...] -- w [...] 1 ) | ( [] -- 0 ): deconstruct a quotation.
        // Structural and affine — the input quote is consumed once and split, never
        // duplicated. A head word that is not itself a value (a bare Prim/Call, not
        // PushInt/PushQuote) faults TypeMismatch (the faithful reading of the design's
        // one open decision — see the implementer report).
        SpecPrim::Uncons => {
            if n < 1 { SpecStep::Fault(Error::Underflow) }
            else {
                match stk[n - 1] {
                    SpecValue::Quote(q) => {
                        let base = stk.subrange(0, n - 1);
                        if q.len() == 0 {
                            SpecStep::Next(SpecState {
                                stack: base.push(SpecValue::Int(0int)),
                                cont: rest,
                            })
                        } else {
                            let tail = SpecValue::Quote(q.subrange(1, q.len() as int));
                            match q[0] {
                                SpecWord::PushInt(i) => SpecStep::Next(SpecState {
                                    stack: base.push(SpecValue::Int(i)).push(tail).push(SpecValue::Int(1int)),
                                    cont: rest,
                                }),
                                SpecWord::PushQuote(s) => SpecStep::Next(SpecState {
                                    stack: base.push(SpecValue::Quote(s)).push(tail).push(SpecValue::Int(1int)),
                                    cont: rest,
                                }),
                                _ => SpecStep::Fault(Error::TypeMismatch),
                            }
                        }
                    }
                    _ => SpecStep::Fault(Error::TypeMismatch),
                }
            }
        }
        // ---------------- v0.3 sequence primitives (design §3) ----------------
        // fold ( [w0 w1 ...] init [C] -- r ): LEFT fold. init seeds the accumulator;
        // C ( acc w -- acc' ) is applied once per element, left to right. The
        // sequence is deconstructed head-first -- affine, like uncons (consumed
        // once, never duplicated); C is replicated along the spine -- multiplicative,
        // like C in primrec. Native recursion via re-emitting Fold (does NOT desugar
        // into linrec). Reuses the existing value_to_word helper to re-push `init`.
        //
        // TERMINATION (like primrec/times, NOT linrec): fold's own recursion is
        // well-founded on the spine length `qs.len()` -- each step recurses on
        // `tail` with `tail.len() == qs.len() - 1`, strictly decreasing toward the
        // empty-list base, so on any finite seq fold terminates in `len(seq)`
        // applications. (A divergent user quote C could still loop inside C, exactly
        // as a divergent C could inside primrec; that is C's concern, not fold's.)
        // Fault precedence: arity (Underflow) -> types (TypeMismatch, seq/C not
        // quotes OR a non-value seq head). No semantic-fault arm: fold performs no
        // arithmetic, so Overflow/DivByZero can arise only inside C, under C's rules.
        SpecPrim::Fold => {
            if n < 3 { SpecStep::Fault(Error::Underflow) }              // (1) arity  -> Underflow
            else {
                match (stk[n - 3], stk[n - 2], stk[n - 1]) {
                    // seq must be a Quote; init is ANY value; combine must be a Quote.
                    (SpecValue::Quote(qs), init, SpecValue::Quote(qc)) => {
                        let base = stk.subrange(0, n - 3);
                        if qs.len() == 0 {
                            // empty list: the result is the seed accumulator.
                            SpecStep::Next(SpecState { stack: base.push(init), cont: rest })
                        } else {
                            let tail = qs.subrange(1, qs.len() as int);    // head-first (uncons reading)
                            match qs[0] {
                                // Continuation, run on `base`:
                                //   [tail]  init  <push head>   C   [C] fold
                                SpecWord::PushInt(_) | SpecWord::PushQuote(_) => {
                                    let recur = seq![
                                        SpecWord::PushQuote(tail),
                                        value_to_word(init),
                                        qs[0]
                                    ] + qc + seq![
                                        SpecWord::PushQuote(qc),
                                        SpecWord::Prim(SpecPrim::Fold)
                                    ];
                                    SpecStep::Next(SpecState { stack: base, cont: recur + rest })
                                }
                                _ => SpecStep::Fault(Error::TypeMismatch),  // (2b) non-value head
                            }
                        }
                    }
                    _ => SpecStep::Fault(Error::TypeMismatch),              // (2a) seq/C not quotes
                }
            }
        }
        // xor ( a b -- a^b ): pop two Ints, push their i64 two's-complement XOR.
        // Structurally identical to Eq/Lt: TOTAL, arity -> type ordering only.
        // Unlike Add/Sub/Mul, (a ^ b) of two in-range i64 values is ALWAYS in
        // i64 range, so there is NO Overflow arm and NO DivByZero arm.
        SpecPrim::Xor => {
            if n < 2 { SpecStep::Fault(Error::Underflow) }        // (1) arity -> Underflow
            else {
                match (stk[n - 2], stk[n - 1]) {
                    (SpecValue::Int(a), SpecValue::Int(b)) =>
                        // (3) semantic: total. Bitwise XOR on the i64 two's-complement
                        // representation; no Overflow arm (cf. Eq/Lt).
                        SpecStep::Next(SpecState {
                            stack: stk.subrange(0, n - 2).push(
                                SpecValue::Int(i64_bitxor(a, b))),
                            cont: rest,
                        }),
                    _ => SpecStep::Fault(Error::TypeMismatch),        // (2) type -> TypeMismatch
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

// `Word` is RECURSIVE (`PushQuote(Vec<Word>)`), so an auto-`#[derive(Clone)]`
// is rejected by Verus as a cyclic self-reference in the derived impl. We hand
// off a trusted, hand-written impl marked `#[verifier::external_body]` — the
// exact discipline the exec twins (`exec_step`/`exec_prim`/…) already use — so
// Verus treats it as a trusted exec item and does NOT analyze the body for the
// cycle. The body is real deep-cloning Rust (recursing through the nested
// `Vec<Word>` via its own `Clone`). The `ensures` records clone-preserves-view
// (`view_word` is invariant under clone), matching this file's view-based
// equality idiom (cf. `deep_view() == s2` in `exec_step`).
impl Clone for Word {
    #[verifier::external_body]
    fn clone(&self) -> (res: Self)
        ensures
            view_word(res) == view_word(*self),
    {
        match self {
            Word::PushInt(n) => Word::PushInt(*n),
            Word::PushQuote(q) => Word::PushQuote(q.clone()),
            Word::Prim(p) => Word::Prim(*p),
            Word::Call(cs) => Word::Call(cs.clone()),
        }
    }
}

pub enum Value {
    Int(i64),
    Quote(Vec<Word>),
}

// `Value::Quote(Vec<Word>)` transitively embeds recursive `Word`, so its
// derived `Clone` is rejected for the same cyclic-self-reference reason. Same
// trusted external_body treatment; `ensures` records clone-preserves-view via
// `view_value`.
impl Clone for Value {
    #[verifier::external_body]
    fn clone(&self) -> (res: Self)
        ensures
            view_value(res) == view_value(*self),
    {
        match self {
            Value::Int(n) => Value::Int(*n),
            Value::Quote(q) => Value::Quote(q.clone()),
        }
    }
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

// ------------------------------------------------------------
// Deep view of the exec machine state into the ghost model.
// The refinement bridge for P2: exec `Vm` -> spec `SpecState`.
// ------------------------------------------------------------
pub open spec fn view_stack(s: Seq<Value>) -> Seq<SpecValue>
    decreases s.len(),
{
    if s.len() == 0 {
        seq![]
    } else {
        seq![view_value(s[0])] + view_stack(s.subrange(1, s.len() as int))
    }
}

impl Vm {
    pub open spec fn deep_view(self) -> SpecState {
        SpecState {
            stack: view_stack(self.stack@),
            cont: view_words(self.cont@),
        }
    }
}

// ============================================================
// Stage-0 view-homomorphism lemmas (P2 refinement helpers).
//
// view_words / view_stack are structure-preserving maps from the exec AST
// into the ghost model. The leaf and splicing refinement proofs need them to
// commute with the seq operations exec performs: append (cont splicing), push
// (stack push), prefix truncation (the two pops of a binop), and indexing
// (operand reads). All proved by head-peel induction mirroring the spec-fn
// definitions above, `decreases` on the seq length.
//
// STATUS: VERIFIED. All statements and proof bodies below close under Verus.
// ============================================================

// view_words is a monoid homomorphism: it distributes over concatenation.
// Every exec_prim arm that splices `q ++ rest` into the continuation needs this.
pub proof fn lemma_view_words_append(a: Seq<Word>, b: Seq<Word>)
    ensures
        view_words(a + b) == view_words(a) + view_words(b),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(a + b =~= b);
        assert(view_words(a) =~= Seq::<SpecWord>::empty());
    } else {
        let a_tail = a.subrange(1, a.len() as int);
        let ab = a + b;
        assert(ab.len() == a.len() + b.len());
        assert(ab[0] == a[0]);
        assert(ab.subrange(1, ab.len() as int) =~= a_tail + b);
        lemma_view_words_append(a_tail, b);
        // view_words(ab) unfolds via ab[0]==a[0] and the tail IH; view_words(a)
        // unfolds via a[0] and a_tail; the rest is associativity of Seq `+`.
        assert(view_words(ab) =~= view_words(a) + view_words(b));
    }
}

// view_stack preserves length (aligns the spec `subrange(0, n-2)` index with
// the exec stack after two pops).
pub proof fn lemma_view_stack_len(s: Seq<Value>)
    ensures
        view_stack(s).len() == s.len(),
    decreases s.len(),
{
    if s.len() == 0 {
    } else {
        lemma_view_stack_len(s.subrange(1, s.len() as int));
    }
}

// view_stack commutes with indexing: the ghost operand equals the view of the
// exec operand. Drives the spec `(Int, Int)` match from the exec match.
pub proof fn lemma_view_stack_index(s: Seq<Value>, i: int)
    requires
        0 <= i < s.len(),
    ensures
        view_stack(s)[i] == view_value(s[i]),
    decreases s.len(),
{
    let head = seq![view_value(s[0])];
    let t = s.subrange(1, s.len() as int);
    assert(view_stack(s) == head + view_stack(t));  // unfold (fuel)
    if i == 0 {
        assert((head + view_stack(t))[0] == head[0]);
    } else {
        lemma_view_stack_len(t);
        assert(t[i - 1] == s[i]);
        // (seq![vv] + view_stack(t))[i] == view_stack(t)[i-1] for i >= 1.
        assert((head + view_stack(t))[i] == view_stack(t)[i - 1]);
        lemma_view_stack_index(t, i - 1);
    }
}

// view_stack commutes with prefix truncation (the two pops of a binop).
pub proof fn lemma_view_stack_prefix(s: Seq<Value>, k: int)
    requires
        0 <= k <= s.len(),
    ensures
        view_stack(s.subrange(0, k)) == view_stack(s).subrange(0, k),
    decreases s.len(),
{
    if k == 0 {
        assert(s.subrange(0, 0) =~= Seq::<Value>::empty());
        assert(view_stack(s).subrange(0, 0) =~= Seq::<SpecValue>::empty());
    } else {
        let head = seq![view_value(s[0])];
        let t = s.subrange(1, s.len() as int);
        let p = s.subrange(0, k);
        assert(p[0] == s[0]);
        assert(p.subrange(1, k) =~= t.subrange(0, k - 1));
        assert(view_stack(p) == head + view_stack(p.subrange(1, k)));  // unfold p
        lemma_view_stack_prefix(t, k - 1);
        lemma_view_stack_len(t);
        assert(view_stack(s) == head + view_stack(t));  // unfold s
        // pure Seq identity: (head + X).subrange(0,k) == head + X.subrange(0,k-1)
        vstd::assert_seqs_equal!(
            (head + view_stack(t)).subrange(0, k)
                == head + view_stack(t).subrange(0, k - 1));
        assert(view_stack(p) =~= view_stack(s).subrange(0, k));
    }
}

// view_stack commutes with push (the single result push of a binop / cmp).
pub proof fn lemma_view_stack_push(s: Seq<Value>, v: Value)
    ensures
        view_stack(s.push(v)) == view_stack(s).push(view_value(v)),
    decreases s.len(),
{
    if s.len() == 0 {
        let sv = s.push(v);
        assert(sv =~= seq![v]);
        assert(sv.subrange(1, sv.len() as int) =~= Seq::<Value>::empty());
        assert(view_stack(sv) == seq![view_value(sv[0])]
            + view_stack(sv.subrange(1, sv.len() as int)));  // unfold
        assert(view_stack(s) =~= Seq::<SpecValue>::empty());
        assert(view_stack(sv) =~= view_stack(s).push(view_value(v)));
    } else {
        let t = s.subrange(1, s.len() as int);
        let sv = s.push(v);
        assert(sv[0] == s[0]);
        assert(sv.subrange(1, sv.len() as int) =~= t.push(v));
        lemma_view_stack_push(t, v);
        // seq![vv] + (view_stack(t).push(x)) == (seq![vv] + view_stack(t)).push(x).
        assert(view_stack(sv) =~= view_stack(s).push(view_value(v)));
    }
}

// Convenience combinator: the exact stack transform every binop / cmp leaf
// performs — pop two operands, push one result — pushed through view_stack.
pub proof fn lemma_view_stack_pop2_push(s: Seq<Value>, w: Value)
    requires
        s.len() >= 2,
    ensures
        view_stack(s.subrange(0, s.len() as int - 2).push(w))
            == view_stack(s).subrange(0, s.len() as int - 2).push(view_value(w)),
{
    let k = s.len() as int - 2;
    lemma_view_stack_push(s.subrange(0, k), w);
    lemma_view_stack_prefix(s, k);
}

// Terminal outcome of the fuel-bounded driver.
pub enum Outcome {
    Halt(Vec<Value>),
    Fault(Error),
    FuelExhausted,
}

// ============================================================
// The exec step and driver — GREEN phase.
//
// STATUS: `exec_step` and `run` are now machine-checked (P2 discharged for the
// one-step contract and its fuel-bounded unrolling). Their bodies faithfully
// mirror `spec_step`'s fault-check order and carry real refinement `ensures`
// proved against `spec_step` / `spec_run`. The only remaining P2 trust
// boundary is the two recursive-type `Clone` impls (unavoidable external_body,
// view-preserving); `p2_refinement` below is now a fully-proved corollary (no
// `admit`) that re-expresses this contract in strong iff/equality form.
// Complementary executable evidence is the differential proptest oracle in
// `tests/interpreter.rs`, which checks `crate::interp::exec_step`/`run`
// against an independent transliteration of `spec_step`, step-for-step.
// ============================================================

// Refinement contract (P2). VERIFIED: exec_step refines spec_step across
// deep_view. This `ensures` IS the real one-step P2 contract.
pub fn exec_step(vm: &mut Vm) -> (r: StepResult)
    ensures
        match spec_step(old(vm).deep_view()) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Halt(_) => r is Halt,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
        },
{
    // Faithful mirror of spec_step / spec_step_prim (see crate::interp for the
    // tested twin). Fault order: arity (Underflow) before type (TypeMismatch);
    // for div/mod, DivByZero before Overflow, both inside the both-Int arm.
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    let n = vm.stack.len();
    proof { lemma_view_words_len(c0); }
    if vm.cont.len() == 0 {
        // spec_step: deep_view().cont has length 0 (view_words preserves len),
        // so spec_step returns Halt.
        return StepResult::Halt;
    }
    // cont non-empty: relate the exec head to the spec head/tail.
    proof {
        lemma_view_words_index(c0, 0);   // view_words(c0)[0] == view_word(c0[0])
        lemma_view_words_tail(c0);       // view_words(tail c0) == (view_words c0) tail
    }
    let head = vm.cont[0].clone();  // view_word(head) == view_word(c0[0])
    match head {
        Word::PushInt(k) => {
            vm.cont.remove(0);
            vm.stack.push(Value::Int(k));
            proof {
                lemma_view_stack_push(s0, Value::Int(k));
                assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= s0.push(Value::Int(k)));
            }
            StepResult::Next
        }
        Word::PushQuote(qq) => {
            let ghost gqq = qq;
            vm.cont.remove(0);
            vm.stack.push(Value::Quote(qq));
            proof {
                lemma_view_stack_push(s0, Value::Quote(gqq));
                assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= s0.push(Value::Quote(gqq)));
            }
            StepResult::Next
        }
        Word::Call(_) => StepResult::Fault(Error::UnknownWord),
        Word::Prim(p) => {
            // exec_prim's precondition: n == stack.len(), cont non-empty,
            // cont[0] views as Prim(p). Its ensures is exactly the Prim arm of
            // spec_step, since spec_step(deep_view) == spec_step_prim(stack, p, rest).
            assert(view_word(c0[0]) == SpecWord::Prim(p));
            exec_prim(vm, p, n)
        }
    }
}

// exec_prim — primitive dispatch. Verified DISPATCHER: each arm delegates to a
// per-primitive helper whose refinement `ensures` mirrors the matching
// spec_step_prim arm across deep_view (same shape as exec_step's contract).
// The dispatcher carries no proof beyond routing; the obligations live in the
// helpers, or in the already-verified arithmetic/comparison leaves
// (exec_arith / exec_divmod / exec_cmp) for the Add..Lt arms.
pub fn exec_prim(vm: &mut Vm, p: SpecPrim, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
        view_word(old(vm).cont@[0]) == SpecWord::Prim(p),
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), p, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    match p {
        SpecPrim::Dup => exec_dup(vm, n),
        SpecPrim::Drop => exec_drop(vm, n),
        SpecPrim::Swap => exec_swap(vm, n),
        SpecPrim::Rot => exec_rot(vm, n),
        SpecPrim::Over => exec_over(vm, n),
        SpecPrim::Apply => exec_apply(vm, n),
        SpecPrim::Cat => exec_cat(vm, n),
        SpecPrim::Cons => exec_cons(vm, n),
        SpecPrim::Dip => exec_dip(vm, n),
        SpecPrim::Add => exec_arith(vm, p, n),
        SpecPrim::Sub => exec_arith(vm, p, n),
        SpecPrim::Mul => exec_arith(vm, p, n),
        SpecPrim::Div => exec_divmod(vm, true, n),
        SpecPrim::Mod => exec_divmod(vm, false, n),
        SpecPrim::Eq => exec_cmp(vm, true, n),
        SpecPrim::Lt => exec_cmp(vm, false, n),
        SpecPrim::If => exec_if(vm, n),
        SpecPrim::PrimRec => exec_primrec(vm, n),
        SpecPrim::Times => exec_times(vm, n),
        SpecPrim::LinRec => exec_linrec(vm, n),
        SpecPrim::Uncons => exec_uncons(vm, n),
        SpecPrim::Fold => exec_fold(vm, n),
        SpecPrim::Xor => exec_xor(vm, n),
    }
}

// ============================================================
// Per-primitive exec helpers. Each refines the matching spec_step_prim arm
// under deep_view. The EASY arms (stack shuffles, quote algebra, If, Xor) and
// the re-emission arms (PrimRec/Times/LinRec/Uncons/Fold) are all fully
// verified now — no external_body.
// ============================================================

// ---------------- stack shuffling (arity-only faults) ----------------
pub fn exec_dup(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Dup, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 1 { return StepResult::Fault(Error::Underflow); }
    let top = vm.stack[n - 1].clone();
    let ghost gtop = top;
    vm.cont.remove(0);
    vm.stack.push(top);
    proof {
        lemma_view_stack_index(s0, n as int - 1);
        lemma_view_stack_push(s0, gtop);
        assert(view_value(gtop) == view_stack(s0)[n as int - 1]);
        assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
        assert(vm.stack@ =~= s0.push(gtop));
    }
    StepResult::Next
}

pub fn exec_drop(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Drop, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 1 { return StepResult::Fault(Error::Underflow); }
    vm.cont.remove(0);
    vm.stack.pop();
    proof {
        lemma_view_stack_prefix(s0, n as int - 1);
        assert(vm.stack@ =~= s0.subrange(0, n as int - 1));
        assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
    }
    StepResult::Next
}

pub fn exec_swap(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Swap, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    // Behaviour-identical to `stack.swap(n-1, n-2)`, but expressed via pop/push
    // so only the push/prefix/index view lemmas are needed (Vec::swap has no
    // vstd spec). ( a b -- b a )
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    vm.cont.remove(0);
    let vb = vm.stack.pop();  // Some(s0[n-1])
    let va = vm.stack.pop();  // Some(s0[n-2])
    match (va, vb) {
        (Some(x), Some(y)) => {
            vm.stack.push(y);
            vm.stack.push(x);
            proof {
                let base = s0.subrange(0, n as int - 2);
                lemma_view_stack_index(s0, n as int - 2);
                lemma_view_stack_index(s0, n as int - 1);
                lemma_view_stack_prefix(s0, n as int - 2);
                lemma_view_stack_push(base, s0[n as int - 1]);
                lemma_view_stack_push(base.push(s0[n as int - 1]), s0[n as int - 2]);
                assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= base.push(s0[n as int - 1]).push(s0[n as int - 2]));
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

pub fn exec_rot(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Rot, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    // Behaviour-identical to `let a = stack.remove(n-3); stack.push(a)`, but via
    // pop/push so the view proof reduces to prefix + three pushes. ( a b c -- b c a )
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 3 { return StepResult::Fault(Error::Underflow); }
    vm.cont.remove(0);
    let vc = vm.stack.pop();  // Some(s0[n-1])
    let vb = vm.stack.pop();  // Some(s0[n-2])
    let va = vm.stack.pop();  // Some(s0[n-3])
    match (va, vb, vc) {
        (Some(a), Some(b), Some(c)) => {
            vm.stack.push(b);
            vm.stack.push(c);
            vm.stack.push(a);
            proof {
                let base = s0.subrange(0, n as int - 3);
                lemma_view_stack_index(s0, n as int - 3);
                lemma_view_stack_index(s0, n as int - 2);
                lemma_view_stack_index(s0, n as int - 1);
                lemma_view_stack_prefix(s0, n as int - 3);
                lemma_view_stack_push(base, s0[n as int - 2]);
                lemma_view_stack_push(base.push(s0[n as int - 2]), s0[n as int - 1]);
                lemma_view_stack_push(
                    base.push(s0[n as int - 2]).push(s0[n as int - 1]),
                    s0[n as int - 3]);
                assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= base
                    .push(s0[n as int - 2])
                    .push(s0[n as int - 1])
                    .push(s0[n as int - 3]));
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

pub fn exec_over(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Over, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    let a = vm.stack[n - 2].clone();
    let ghost ga = a;
    vm.cont.remove(0);
    vm.stack.push(a);
    proof {
        lemma_view_stack_index(s0, n as int - 2);
        lemma_view_stack_push(s0, ga);
        assert(view_value(ga) == view_stack(s0)[n as int - 2]);
        assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
        assert(vm.stack@ =~= s0.push(ga));
    }
    StepResult::Next
}

// ---------------- quotation algebra ----------------
pub fn exec_apply(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Apply, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 1 { return StepResult::Fault(Error::Underflow); }
    if !matches!(vm.stack[n - 1], Value::Quote(_)) {
        proof { lemma_view_stack_index(s0, n as int - 1); }
        return StepResult::Fault(Error::TypeMismatch);
    }
    vm.cont.remove(0);
    let qv = vm.stack.pop();  // Some(s0[n-1]) = Quote(q)
    match qv {
        Some(Value::Quote(mut q)) => {
            let ghost gq = q@;
            q.append(&mut vm.cont);  // q@ == gq + rest
            vm.cont = q;
            proof {
                lemma_view_stack_index(s0, n as int - 1);
                lemma_view_stack_prefix(s0, n as int - 1);
                lemma_view_words_append(gq, c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= s0.subrange(0, n as int - 1));
                assert(vm.cont@ =~= gq + c0.subrange(1, c0.len() as int));
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

pub fn exec_cat(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Cat, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    let ok = matches!(vm.stack[n - 2], Value::Quote(_))
        && matches!(vm.stack[n - 1], Value::Quote(_));
    if !ok {
        proof {
            lemma_view_stack_index(s0, n as int - 2);
            lemma_view_stack_index(s0, n as int - 1);
        }
        return StepResult::Fault(Error::TypeMismatch);
    }
    vm.cont.remove(0);
    let vb = vm.stack.pop();  // Some(s0[n-1]) = Quote(b)
    let va = vm.stack.pop();  // Some(s0[n-2]) = Quote(a)
    match (va, vb) {
        (Some(Value::Quote(mut a)), Some(Value::Quote(mut b))) => {
            let ghost ga = a@;
            let ghost gb = b@;
            a.append(&mut b);  // a@ == ga + gb
            let qval = Value::Quote(a);
            let ghost gqval = qval;
            vm.stack.push(qval);
            proof {
                lemma_view_stack_index(s0, n as int - 2);
                lemma_view_stack_index(s0, n as int - 1);
                lemma_view_stack_pop2_push(s0, gqval);
                lemma_view_words_append(ga, gb);
                assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= s0.subrange(0, n as int - 2).push(gqval));
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

pub fn exec_cons(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Cons, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    if !matches!(vm.stack[n - 1], Value::Quote(_)) {
        proof { lemma_view_stack_index(s0, n as int - 1); }
        return StepResult::Fault(Error::TypeMismatch);
    }
    vm.cont.remove(0);
    let qv = vm.stack.pop();  // Some(s0[n-1]) = Quote(q)
    let v = vm.stack.pop();   // Some(s0[n-2]) = any value
    match (v, qv) {
        (Some(vv), Some(Value::Quote(q))) => {
            let ghost gq = q@;
            let w = value_to_exec_word(vv);  // view_word(w) == value_to_word(view_value(vv))
            let ghost gw = w;
            let mut newq: Vec<Word> = Vec::new();
            newq.push(w);
            let mut q2 = q;
            newq.append(&mut q2);  // newq@ == seq![gw] + gq
            let qval = Value::Quote(newq);
            let ghost gqval = qval;
            vm.stack.push(qval);
            proof {
                lemma_view_stack_index(s0, n as int - 2);
                lemma_view_stack_index(s0, n as int - 1);
                lemma_view_stack_pop2_push(s0, gqval);
                reveal_with_fuel(view_words, 2);
                assert(seq![gw].subrange(1, 1) =~= Seq::<Word>::empty());
                assert(view_words(seq![gw]) =~= seq![view_word(gw)]);
                lemma_view_words_append(seq![gw], gq);
                assert(newq@ =~= seq![gw] + gq);
                assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= s0.subrange(0, n as int - 2).push(gqval));
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

pub fn exec_dip(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Dip, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    if !matches!(vm.stack[n - 1], Value::Quote(_)) {
        proof { lemma_view_stack_index(s0, n as int - 1); }
        return StepResult::Fault(Error::TypeMismatch);
    }
    vm.cont.remove(0);
    let qv = vm.stack.pop();  // Some(s0[n-1]) = Quote(q)
    let a = vm.stack.pop();   // Some(s0[n-2]) = the set-aside value
    match (a, qv) {
        (Some(av), Some(Value::Quote(mut q))) => {
            let ghost gq = q@;
            let w = value_to_exec_word(av);  // view_word(w) == value_to_word(view_value(av))
            let ghost gw = w;
            q.push(w);               // q@ == gq.push(gw)
            q.append(&mut vm.cont);  // q@ == gq.push(gw) + rest
            vm.cont = q;
            proof {
                lemma_view_stack_index(s0, n as int - 2);
                lemma_view_stack_index(s0, n as int - 1);
                lemma_view_stack_prefix(s0, n as int - 2);
                assert(gq.push(gw) =~= gq + seq![gw]);
                reveal_with_fuel(view_words, 2);
                assert(seq![gw].subrange(1, 1) =~= Seq::<Word>::empty());
                assert(view_words(seq![gw]) =~= seq![view_word(gw)]);
                lemma_view_words_append(gq, seq![gw]);
                lemma_view_words_append(gq.push(gw), c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= s0.subrange(0, n as int - 2));
                assert(vm.cont@ =~= gq.push(gw) + c0.subrange(1, c0.len() as int));
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

// ---------------- branch ----------------
pub fn exec_if(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::If, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 3 { return StepResult::Fault(Error::Underflow); }
    let ok = matches!(vm.stack[n - 3], Value::Int(_))
        && matches!(vm.stack[n - 2], Value::Quote(_))
        && matches!(vm.stack[n - 1], Value::Quote(_));
    if !ok {
        proof {
            lemma_view_stack_index(s0, n as int - 3);
            lemma_view_stack_index(s0, n as int - 2);
            lemma_view_stack_index(s0, n as int - 1);
        }
        return StepResult::Fault(Error::TypeMismatch);
    }
    proof {
        lemma_view_stack_index(s0, n as int - 3);
        lemma_view_stack_index(s0, n as int - 2);
        lemma_view_stack_index(s0, n as int - 1);
    }
    vm.cont.remove(0);
    let fv = vm.stack.pop();  // Some(s0[n-1]) = Quote(f)
    let tv = vm.stack.pop();  // Some(s0[n-2]) = Quote(t)
    let cv = vm.stack.pop();  // Some(s0[n-3]) = Int(c)
    match (cv, tv, fv) {
        (Some(Value::Int(c)), Some(Value::Quote(t)), Some(Value::Quote(f))) => {
            let mut branch = if c != 0 { t } else { f };
            let ghost gbranch = branch@;
            branch.append(&mut vm.cont);
            vm.cont = branch;
            proof {
                lemma_view_stack_prefix(s0, n as int - 3);
                lemma_view_words_append(gbranch, c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= s0.subrange(0, n as int - 3));
                assert(vm.cont@ =~= gbranch + c0.subrange(1, c0.len() as int));
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

// ---------------- total bitwise xor (cmp-shaped: NO Overflow arm) ----------------
pub fn exec_xor(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Xor, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    let (a, b) = match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => (*a, *b),
        _ => {
            proof {
                lemma_view_stack_index(s0, n as int - 2);
                lemma_view_stack_index(s0, n as int - 1);
            }
            return StepResult::Fault(Error::TypeMismatch);
        }
    };
    proof {
        lemma_view_stack_index(s0, n as int - 2);
        lemma_view_stack_index(s0, n as int - 1);
    }
    vm.cont.remove(0);
    vm.stack.pop();
    vm.stack.pop();
    vm.stack.push(Value::Int(a ^ b));
    proof {
        assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
        assert(vm.stack@ =~= s0.subrange(0, n as int - 2).push(Value::Int(a ^ b)));
        lemma_view_stack_pop2_push(s0, Value::Int(a ^ b));
        assert(view_stack(vm.stack@)
            == view_stack(s0).subrange(0, n as int - 2).push(SpecValue::Int((a ^ b) as int)));
        assert(view_stack(s0)[n as int - 2] == SpecValue::Int(a as int));
        assert(view_stack(s0)[n as int - 1] == SpecValue::Int(b as int));
        // total: exec pushes Int(a^b); spec pushes Int(i64_bitxor(a int, b int)).
        // i64_bitxor casts int->i64 (round-trips for in-range a, b), so agree.
        assert((a as int) as i64 == a) by (bit_vector);
        assert((b as int) as i64 == b) by (bit_vector);
        assert((a ^ b) as int == i64_bitxor(a as int, b as int));
    }
    StepResult::Next
}

// ============================================================
// Re-emission arms — FULLY VERIFIED. These splice a re-emitted recursive
// continuation and needed the append homomorphism plus a per-arm decreases;
// that pass is complete, so they are proved (no external_body).
// ============================================================

// ------------------------------------------------------------
// Arm-specific helper lemmas for exec_primrec (disjoint region).
// ------------------------------------------------------------

// view_words of a singleton peels to the singleton view (same inline idiom
// exec_cons/exec_dip use; hoisted so the 5-element PrimRec prefix can be
// decomposed word-by-word via the append homomorphism).
pub proof fn lemma_view_words_singleton(w: Word)
    ensures
        view_words(seq![w]) == seq![view_word(w)],
{
    reveal_with_fuel(view_words, 2);
    assert(seq![w].subrange(1, 1) =~= Seq::<Word>::empty());
    assert(view_words(seq![w]) =~= seq![view_word(w)]);
}

// view_words respects pointwise view_word equality. This is exactly what a
// `Vec::clone` of a `Vec<Word>` delivers (`cloned(a[i], b[i])` ==> view_word
// preserved, via the trusted `Word::clone` ensures), letting the re-pushed
// `qc.clone()` inside the PrimRec prefix carry the same view as the original.
pub proof fn lemma_view_words_pointwise(a: Seq<Word>, b: Seq<Word>)
    requires
        a.len() == b.len(),
        forall|i: int| 0 <= i < a.len() ==> view_word(a[i]) == view_word(b[i]),
    ensures
        view_words(a) == view_words(b),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(view_words(a) =~= view_words(b));
    } else {
        let at = a.subrange(1, a.len() as int);
        let bt = b.subrange(1, b.len() as int);
        assert forall|i: int| 0 <= i < at.len() implies view_word(at[i]) == view_word(bt[i]) by {
            assert(at[i] == a[i + 1]);
            assert(bt[i] == b[i + 1]);
        }
        lemma_view_words_pointwise(at, bt);
        assert(view_word(a[0]) == view_word(b[0]));
        assert(view_words(a) =~= seq![view_word(a[0])] + view_words(at));
        assert(view_words(b) =~= seq![view_word(b[0])] + view_words(bt));
    }
}

// PrimRec re-emission ( n [I] [C] -- r ). Discharged: mirrors the PrimRec arm of
// spec_step_prim across deep_view exactly. k<=0 splices I; k>0 re-emits the
// 5-word recursion prefix + C.
pub fn exec_primrec(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::PrimRec, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    let ghost rest_exec = c0.subrange(1, c0.len() as int);
    let ghost rest = view_words(rest_exec);
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 3 { return StepResult::Fault(Error::Underflow); }
    let ok = matches!(vm.stack[n - 3], Value::Int(_))
        && matches!(vm.stack[n - 2], Value::Quote(_))
        && matches!(vm.stack[n - 1], Value::Quote(_));
    if !ok {
        proof {
            lemma_view_stack_index(s0, n as int - 3);
            lemma_view_stack_index(s0, n as int - 2);
            lemma_view_stack_index(s0, n as int - 1);
        }
        return StepResult::Fault(Error::TypeMismatch);
    }
    proof {
        lemma_view_stack_index(s0, n as int - 3);
        lemma_view_stack_index(s0, n as int - 2);
        lemma_view_stack_index(s0, n as int - 1);
        lemma_view_stack_prefix(s0, n as int - 3);
    }
    vm.cont.remove(0);
    let qcv = vm.stack.pop();  // Some(s0[n-1]) = Quote(qc)
    let qiv = vm.stack.pop();  // Some(s0[n-2]) = Quote(qi)
    let kv = vm.stack.pop();   // Some(s0[n-3]) = Int(k)
    match (kv, qiv, qcv) {
        (Some(Value::Int(k)), Some(Value::Quote(qi)), Some(Value::Quote(mut qc))) => {
            let ghost gqi = qi@;
            let ghost gqc = qc@;
            if k <= 0 {
                let mut recur = qi;              // recur@ == gqi
                recur.append(&mut vm.cont);      // recur@ == gqi + rest_exec
                vm.cont = recur;
                proof {
                    lemma_view_words_append(gqi, rest_exec);
                    assert(vm.stack@ =~= s0.subrange(0, n as int - 3));
                    assert(vm.cont@ =~= gqi + rest_exec);
                    assert(view_stack(vm.stack@) =~= view_stack(s0).subrange(0, n as int - 3));
                    assert(view_words(vm.cont@) =~= view_words(gqi) + rest);
                }
            } else {
                let qc_clone = qc.clone();
                let ghost gqc_clone = qc_clone@;
                let mut recur: Vec<Word> = Vec::new();
                recur.push(Word::PushInt(k));
                recur.push(Word::PushInt(k - 1));
                recur.push(Word::PushQuote(qi));
                recur.push(Word::PushQuote(qc_clone));
                recur.push(Word::Prim(SpecPrim::PrimRec));
                let ghost five = recur@;
                recur.append(&mut qc);           // recur@ == five + gqc
                recur.append(&mut vm.cont);       // recur@ == (five + gqc) + rest_exec
                vm.cont = recur;
                proof {
                    // qc.clone() preserves the view of every word, so the
                    // re-pushed C-quote has the same deep view as the original.
                    assert forall|i: int| 0 <= i < gqc.len()
                        implies view_word(gqc_clone[i]) == view_word(gqc[i]) by {
                        assert(cloned::<Word>(gqc[i], gqc_clone[i]));
                    }
                    lemma_view_words_pointwise(gqc_clone, gqc);
                    // Per-word views of the 5-word prefix.
                    assert(view_word(five[0]) == SpecWord::PushInt(k as int));
                    assert(view_word(five[1]) == SpecWord::PushInt(k as int - 1));
                    assert(view_word(five[2]) == SpecWord::PushQuote(view_words(gqi)));
                    assert(view_word(five[3]) == SpecWord::PushQuote(view_words(gqc))) by {
                        lemma_view_words_pointwise(gqc_clone, gqc);
                    }
                    assert(view_word(five[4]) == SpecWord::Prim(SpecPrim::PrimRec));
                    // Decompose the 5-word prefix through the append homomorphism,
                    // right-associated so the lemma grouping matches `five`.
                    let t4 = seq![five[4]];
                    let t3 = seq![five[3]] + t4;
                    let t2 = seq![five[2]] + t3;
                    let t1 = seq![five[1]] + t2;
                    let t0 = seq![five[0]] + t1;
                    assert(five =~= t0);
                    lemma_view_words_singleton(five[0]);
                    lemma_view_words_singleton(five[1]);
                    lemma_view_words_singleton(five[2]);
                    lemma_view_words_singleton(five[3]);
                    lemma_view_words_singleton(five[4]);
                    lemma_view_words_append(seq![five[3]], t4);
                    lemma_view_words_append(seq![five[2]], t3);
                    lemma_view_words_append(seq![five[1]], t2);
                    lemma_view_words_append(seq![five[0]], t1);
                    // Splice: view_words((five + gqc) + rest_exec).
                    lemma_view_words_append(five, gqc);
                    lemma_view_words_append(five + gqc, rest_exec);
                    assert(vm.stack@ =~= s0.subrange(0, n as int - 3));
                    assert(vm.cont@ =~= (five + gqc) + rest_exec);
                    let five_spec = seq![
                        SpecWord::PushInt(k as int),
                        SpecWord::PushInt(k as int - 1),
                        SpecWord::PushQuote(view_words(gqi)),
                        SpecWord::PushQuote(view_words(gqc)),
                        SpecWord::Prim(SpecPrim::PrimRec)
                    ];
                    assert(view_words(five) =~= five_spec);
                    assert(view_stack(vm.stack@) =~= view_stack(s0).subrange(0, n as int - 3));
                    assert(view_words(vm.cont@) =~= (five_spec + view_words(gqc)) + rest);
                }
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

// ---- Times-arm helper lemmas (disjoint region; see integration note) ----

// view_words distributes over a single push (the exec `Vec::push`): the head-peel
// analogue of lemma_view_words_append specialised to a one-element suffix.
// Both surface forms are provided (consolidated from two sibling arms): the
// `+ seq![..]` append form and the `.push(..)` form are extensionally equal, so
// callers of either arm see the fact they need.
pub proof fn lemma_view_words_push(s: Seq<Word>, w: Word)
    ensures
        view_words(s.push(w)) == view_words(s) + seq![view_word(w)],
        view_words(s.push(w)) == view_words(s).push(view_word(w)),
{
    assert(s.push(w) =~= s + seq![w]);
    reveal_with_fuel(view_words, 2);
    assert(seq![w].subrange(1, 1) =~= Seq::<Word>::empty());
    assert(view_words(seq![w]) =~= seq![view_word(w)]);
    lemma_view_words_append(s, seq![w]);
    assert(view_words(s) + seq![view_word(w)] =~= view_words(s).push(view_word(w)));
}

// A single clone (`Word::clone`, external_body) preserves the deep view.
pub proof fn lemma_cloned_word(a: Word, b: Word)
    requires
        cloned::<Word>(a, b),
    ensures
        view_word(a) == view_word(b),
{
}

// view_words is congruent under element-wise view equality: two word-seqs whose
// elements agree under view_word have the same view_words. Bridges `Vec::clone`
// (element-wise `cloned`) to sequence-level view equality.
pub proof fn lemma_view_words_congr(a: Seq<Word>, b: Seq<Word>)
    requires
        a.len() == b.len(),
        forall|i: int| 0 <= i < a.len() ==> view_word(a[i]) == view_word(b[i]),
    ensures
        view_words(a) == view_words(b),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(view_words(a) =~= view_words(b));
    } else {
        let at = a.subrange(1, a.len() as int);
        let bt = b.subrange(1, b.len() as int);
        assert forall|i: int| 0 <= i < at.len() implies view_word(at[i]) == view_word(bt[i]) by {
            assert(at[i] == a[i + 1]);
            assert(bt[i] == b[i + 1]);
        }
        lemma_view_words_congr(at, bt);
        assert(view_words(a) == seq![view_word(a[0])] + view_words(at));
        assert(view_words(b) == seq![view_word(b[0])] + view_words(bt));
    }
}

// Times ( n [Q] -- ): k>0 re-emits `q ++ [k-1, [q], Times] ++ rest`; k<=0 no-op.
pub fn exec_times(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Times, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    let ok = matches!(vm.stack[n - 2], Value::Int(_))
        && matches!(vm.stack[n - 1], Value::Quote(_));
    if !ok {
        proof {
            lemma_view_stack_index(s0, n as int - 2);
            lemma_view_stack_index(s0, n as int - 1);
        }
        return StepResult::Fault(Error::TypeMismatch);
    }
    proof {
        lemma_view_stack_index(s0, n as int - 2);
        lemma_view_stack_index(s0, n as int - 1);
    }
    vm.cont.remove(0);
    let qv = vm.stack.pop();  // Some(s0[n-1]) = Quote(q)
    let kv = vm.stack.pop();  // Some(s0[n-2]) = Int(k)
    match (kv, qv) {
        (Some(Value::Int(k)), Some(Value::Quote(q))) => {
            let ghost gq = q@;
            let ghost grest = c0.subrange(1, c0.len() as int);
            proof { lemma_view_stack_prefix(s0, n as int - 2); }
            if k > 0 {
                let km1: i64 = k - 1;
                let mut recur = q.clone();
                let ghost g0 = recur@;
                recur.push(Word::PushInt(km1));
                let ghost g1 = recur@;
                recur.push(Word::PushQuote(q));
                let ghost g2 = recur@;
                recur.push(Word::Prim(SpecPrim::Times));
                let ghost g3 = recur@;
                recur.append(&mut vm.cont);  // recur@ == g3 + grest
                vm.cont = recur;
                proof {
                    // clone preserves the deep view element-wise -> seq-wise.
                    assert forall|i: int| 0 <= i < gq.len() implies
                        view_word(g0[i]) == view_word(gq[i]) by {
                        lemma_cloned_word(gq[i], g0[i]);
                    }
                    lemma_view_words_congr(g0, gq);
                    // Peel the three re-emitted words.
                    lemma_view_words_push(g0, g1[g0.len() as int]);
                    lemma_view_words_push(g1, g2[g1.len() as int]);
                    lemma_view_words_push(g2, g3[g2.len() as int]);
                    // The three pushed words, identified by their view.
                    assert(g1[g0.len() as int] == Word::PushInt(km1));
                    assert(g2[g1.len() as int] == Word::PushQuote(q));
                    assert(g3[g2.len() as int] == Word::Prim(SpecPrim::Times));
                    assert(km1 as int == (k as int) - 1);
                    // Splice with rest.
                    lemma_view_words_append(g3, grest);
                    // Stack: two pops leave base = s0[0..n-2].
                    assert(vm.stack@ =~= s0.subrange(0, n as int - 2));
                    assert(view_words(vm.cont@) =~= view_words(gq)
                        + seq![
                            SpecWord::PushInt((k as int) - 1),
                            SpecWord::PushQuote(view_words(gq)),
                            SpecWord::Prim(SpecPrim::Times)
                        ] + view_words(grest));
                }
            } else {
                proof {
                    assert(vm.stack@ =~= s0.subrange(0, n as int - 2));
                    assert(vm.cont@ =~= grest);
                }
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

// ============================================================
// Stage-2b re-emission helpers (view homomorphism on Word-seqs).
// These mirror the view_stack_* lemmas above but for `view_words`,
// and are shared by the two splicing arms exec_linrec / exec_fold.
// Placed immediately above exec_linrec (disjoint from other arms).
// ============================================================

// view_words preserves length.
pub proof fn lemma_view_words_len(s: Seq<Word>)
    ensures
        view_words(s).len() == s.len(),
    decreases s.len(),
{
    if s.len() == 0 {
    } else {
        lemma_view_words_len(s.subrange(1, s.len() as int));
    }
}

// view_words commutes with indexing.
pub proof fn lemma_view_words_index(s: Seq<Word>, i: int)
    requires
        0 <= i < s.len(),
    ensures
        view_words(s)[i] == view_word(s[i]),
    decreases s.len(),
{
    let head = seq![view_word(s[0])];
    let t = s.subrange(1, s.len() as int);
    assert(view_words(s) == head + view_words(t));  // unfold (fuel)
    if i == 0 {
        assert((head + view_words(t))[0] == head[0]);
    } else {
        lemma_view_words_len(t);
        assert(t[i - 1] == s[i]);
        assert((head + view_words(t))[i] == view_words(t)[i - 1]);
        lemma_view_words_index(t, i - 1);
    }
}

// view_words commutes with tail (drop-head).
pub proof fn lemma_view_words_tail(s: Seq<Word>)
    requires
        s.len() >= 1,
    ensures
        view_words(s.subrange(1, s.len() as int))
            == view_words(s).subrange(1, view_words(s).len() as int),
{
    let head = seq![view_word(s[0])];
    let t = s.subrange(1, s.len() as int);
    assert(view_words(s) == head + view_words(t));  // unfold
    lemma_view_words_len(s);
    lemma_view_words_len(t);
    assert((head + view_words(t)).subrange(1, (head + view_words(t)).len() as int)
        =~= view_words(t));
}

// Elementwise-equal word-seqs have equal views.
pub proof fn lemma_view_words_eq(a: Seq<Word>, b: Seq<Word>)
    requires
        a.len() == b.len(),
        forall|i| #![auto] 0 <= i < a.len() ==> view_word(a[i]) == view_word(b[i]),
    ensures
        view_words(a) == view_words(b),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(view_words(a) =~= view_words(b));
    } else {
        let ta = a.subrange(1, a.len() as int);
        let tb = b.subrange(1, b.len() as int);
        assert(forall|i| 0 <= i < ta.len() ==> ta[i] == a[i + 1] && tb[i] == b[i + 1]);
        lemma_view_words_eq(ta, tb);
        assert(view_word(a[0]) == view_word(b[0]));
        assert(view_words(a) == seq![view_word(a[0])] + view_words(ta));  // unfold
        assert(view_words(b) == seq![view_word(b[0])] + view_words(tb));  // unfold
    }
}

// Executable clone of a word-quote, with the view-preservation contract the
// splicing arms need. `Vec::clone` gives elementwise `cloned`, and `Word::clone`
// (external_body) ensures view_word is preserved; `lemma_view_words_eq` lifts
// that to the whole seq.
pub fn clone_words(v: &Vec<Word>) -> (res: Vec<Word>)
    ensures
        view_words(res@) == view_words(v@),
{
    let r = v.clone();
    proof {
        assert forall|i| #![auto] 0 <= i < v@.len() implies view_word(r@[i]) == view_word(v@[i]) by {
            assert(cloned::<Word>(v@[i], r@[i]));
        }
        lemma_view_words_eq(v@, r@);
    }
    r
}

// LinRec re-emission (desugars to If). Nested continuation splice: build the
// else-branch quote, then splice `P ; [T] ; [else_q] ; If` ahead of `rest`.
pub fn exec_linrec(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::LinRec, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 4 { return StepResult::Fault(Error::Underflow); }
    let ok = matches!(vm.stack[n - 4], Value::Quote(_))
        && matches!(vm.stack[n - 3], Value::Quote(_))
        && matches!(vm.stack[n - 2], Value::Quote(_))
        && matches!(vm.stack[n - 1], Value::Quote(_));
    if !ok {
        proof {
            lemma_view_stack_index(s0, n as int - 4);
            lemma_view_stack_index(s0, n as int - 3);
            lemma_view_stack_index(s0, n as int - 2);
            lemma_view_stack_index(s0, n as int - 1);
        }
        return StepResult::Fault(Error::TypeMismatch);
    }
    proof {
        lemma_view_stack_index(s0, n as int - 4);
        lemma_view_stack_index(s0, n as int - 3);
        lemma_view_stack_index(s0, n as int - 2);
        lemma_view_stack_index(s0, n as int - 1);
        lemma_view_stack_prefix(s0, n as int - 4);
    }
    vm.cont.remove(0);
    let qr2v = vm.stack.pop();  // s0[n-1] = Quote(qr2)
    let qr1v = vm.stack.pop();  // s0[n-2] = Quote(qr1)
    let qtv = vm.stack.pop();   // s0[n-3] = Quote(qt)
    let qpv = vm.stack.pop();   // s0[n-4] = Quote(qp)
    match (qpv, qtv, qr1v, qr2v) {
        (Some(Value::Quote(qp)), Some(Value::Quote(qt)),
         Some(Value::Quote(qr1)), Some(Value::Quote(qr2))) => {
            let ghost gp = qp@;
            let ghost gt = qt@;
            let ghost gr1 = qr1@;
            let ghost gr2 = qr2@;

            // ---- build else_q = qr1 ; [P] [T] [R1] [R2] linrec ; qr2 ----
            let mut else_q = clone_words(&qr1);  // view_words == view_words(gr1)
            let ghost eq0 = else_q@;
            let qpc = clone_words(&qp);
            let qtc = clone_words(&qt);
            let qr2c = clone_words(&qr2);
            let ghost w1 = Word::PushQuote(qpc);
            let ghost w2 = Word::PushQuote(qtc);
            let ghost w3 = Word::PushQuote(qr1);
            let ghost w4 = Word::PushQuote(qr2c);
            let ghost w5 = Word::Prim(SpecPrim::LinRec);
            else_q.push(Word::PushQuote(qpc));
            else_q.push(Word::PushQuote(qtc));
            else_q.push(Word::PushQuote(qr1));
            else_q.push(Word::PushQuote(qr2c));
            else_q.push(Word::Prim(SpecPrim::LinRec));
            let mut qr2m = qr2;
            else_q.append(&mut qr2m);  // else_q@ == eq0.push(w1..w5) + gr2

            proof {
                lemma_view_words_push(eq0, w1);
                lemma_view_words_push(eq0.push(w1), w2);
                lemma_view_words_push(eq0.push(w1).push(w2), w3);
                lemma_view_words_push(eq0.push(w1).push(w2).push(w3), w4);
                lemma_view_words_push(eq0.push(w1).push(w2).push(w3).push(w4), w5);
                lemma_view_words_append(
                    eq0.push(w1).push(w2).push(w3).push(w4).push(w5), gr2);
            }

            // ---- build spliced = qp ; [T] [else_q] If ; rest ----
            let mut spliced = qp;  // spliced@ == gp
            let ghost sp0 = spliced@;
            let ghost u1 = Word::PushQuote(qt);
            let ghost geq = else_q@;
            let ghost u2 = Word::PushQuote(else_q);
            let ghost u3 = Word::Prim(SpecPrim::If);
            spliced.push(Word::PushQuote(qt));
            spliced.push(Word::PushQuote(else_q));
            spliced.push(Word::Prim(SpecPrim::If));
            spliced.append(&mut vm.cont);  // spliced@ == sp0.push(u1).push(u2).push(u3) + rest
            vm.cont = spliced;

            proof {
                lemma_view_words_push(sp0, u1);
                lemma_view_words_push(sp0.push(u1), u2);
                lemma_view_words_push(sp0.push(u1).push(u2), u3);
                lemma_view_words_append(
                    sp0.push(u1).push(u2).push(u3), c0.subrange(1, c0.len() as int));

                // Spec-side else_q and spliced expressed in the same view terms.
                let qpv2 = view_words(gp);
                let qtv2 = view_words(gt);
                let qr1v2 = view_words(gr1);
                let qr2v2 = view_words(gr2);
                let spec_else = qr1v2 + seq![
                    SpecWord::PushQuote(qpv2),
                    SpecWord::PushQuote(qtv2),
                    SpecWord::PushQuote(qr1v2),
                    SpecWord::PushQuote(qr2v2),
                    SpecWord::Prim(SpecPrim::LinRec)
                ] + qr2v2;
                assert(view_words(geq) =~= spec_else);
                let spec_spliced = qpv2 + seq![
                    SpecWord::PushQuote(qtv2),
                    SpecWord::PushQuote(spec_else),
                    SpecWord::Prim(SpecPrim::If)
                ];
                assert(vm.stack@ =~= s0.subrange(0, n as int - 4));
                assert(view_words(vm.cont@)
                    =~= spec_spliced + view_words(c0.subrange(1, c0.len() as int)));
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

// Uncons ( [w..] -- w [..] 1 | [] -- 0 ): inspects the quote head WITHOUT
// consuming; a non-value head (bare Prim/Call) or non-Quote operand faults
// TypeMismatch leaving the machine untouched (fault is decided before any mutation).
pub fn exec_uncons(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Uncons, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 1 { return StepResult::Fault(Error::Underflow); }
    // (a) top must be a Quote.
    if !matches!(vm.stack[n - 1], Value::Quote(_)) {
        proof { lemma_view_stack_index(s0, n as int - 1); }
        return StepResult::Fault(Error::TypeMismatch);
    }
    proof { lemma_view_stack_index(s0, n as int - 1); }
    let ghost gq = s0[n - 1]->Quote_0@;
    // (b) a non-empty quote's head must itself be a value; else TypeMismatch,
    // machine untouched (we return before removing the cont head / popping).
    if let Value::Quote(q) = &vm.stack[n - 1] {
        assert(q@ == gq);
        if q.len() >= 1 {
            match &q[0] {
                Word::PushInt(_) | Word::PushQuote(_) => {}
                _ => {
                    proof {
                        lemma_view_words_len(gq);
                        lemma_view_words_index(gq, 0);
                    }
                    return StepResult::Fault(Error::TypeMismatch);
                }
            }
        }
    }
    // Past the guard: gq is a value-headed (or empty) quote.
    assert(gq.len() >= 1 ==> (gq[0] is PushInt || gq[0] is PushQuote));
    proof {
        lemma_view_words_len(gq);
        lemma_view_stack_prefix(s0, n as int - 1);
    }
    vm.cont.remove(0);
    let q = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
    assert(q@ == gq);
    assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
    if q.is_empty() {
        vm.stack.push(Value::Int(0));
        proof {
            lemma_view_stack_push(s0.subrange(0, n as int - 1), Value::Int(0));
        }
    } else {
        let mut q = q;
        let head = q.remove(0);  // head == gq[0]; q@ == gq.subrange(1, gq.len())
        let head_val = match head {
            Word::PushInt(i) => Value::Int(i),
            Word::PushQuote(s) => Value::Quote(s),
            _ => { assert(false); Value::Int(0) },  // unreachable by guard
        };
        let ghost ghv = head_val;
        let tail_val = Value::Quote(q);
        let ghost gtv = tail_val;
        vm.stack.push(head_val);
        let ghost g_after_head = vm.stack@;
        vm.stack.push(tail_val);
        let ghost g_after_tail = vm.stack@;
        vm.stack.push(Value::Int(1));
        proof {
            lemma_view_words_index(gq, 0);
            lemma_view_words_tail(gq);
            let base = s0.subrange(0, n as int - 1);
            lemma_view_stack_push(base, ghv);
            lemma_view_stack_push(g_after_head, gtv);
            lemma_view_stack_push(g_after_tail, Value::Int(1));
        }
    }
    StepResult::Next
}

// Fold re-emission (left fold). Empty seq -> push init; non-empty ->
// [tail] init <push head> C [C] Fold ; rest. The seq is deconstructed head-first
// (affine, like uncons); C is replicated along the spine (multiplicative).
pub fn exec_fold(vm: &mut Vm, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), SpecPrim::Fold, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 3 { return StepResult::Fault(Error::Underflow); }
    // (2a) seq / C must be quotes; init is any value.
    let ok = matches!(vm.stack[n - 3], Value::Quote(_))
        && matches!(vm.stack[n - 1], Value::Quote(_));
    if !ok {
        proof {
            lemma_view_stack_index(s0, n as int - 3);
            lemma_view_stack_index(s0, n as int - 1);
        }
        return StepResult::Fault(Error::TypeMismatch);
    }
    // (2b) non-value seq head -> TypeMismatch, machine untouched (no mutation yet).
    if let Value::Quote(qs) = &vm.stack[n - 3] {
        if !qs.is_empty() {
            match &qs[0] {
                Word::PushInt(_) | Word::PushQuote(_) => {}
                _ => {
                    proof {
                        lemma_view_stack_index(s0, n as int - 3);
                        lemma_view_stack_index(s0, n as int - 1);
                        lemma_view_words_len(qs@);
                        lemma_view_words_index(qs@, 0);
                    }
                    return StepResult::Fault(Error::TypeMismatch);
                }
            }
        }
    }
    proof {
        lemma_view_stack_index(s0, n as int - 3);
        lemma_view_stack_index(s0, n as int - 2);
        lemma_view_stack_index(s0, n as int - 1);
        lemma_view_stack_prefix(s0, n as int - 3);
    }
    vm.cont.remove(0);
    let qcv = vm.stack.pop();    // s0[n-1] = Quote(qc)
    let initv = vm.stack.pop();  // s0[n-2] = init (any)
    let qsv = vm.stack.pop();    // s0[n-3] = Quote(qs)
    match (qsv, initv, qcv) {
        (Some(Value::Quote(qs)), Some(init), Some(Value::Quote(qc))) => {
            let ghost gs = qs@;
            let ghost gc = qc@;
            let ghost vinit = view_value(init);
            proof { lemma_view_words_len(gs); }
            if qs.is_empty() {
                let ghost ginit = init;
                vm.stack.push(init);
                proof {
                    lemma_view_stack_push(s0.subrange(0, n as int - 3), ginit);
                    assert(vm.stack@ =~= s0.subrange(0, n as int - 3).push(ginit));
                    assert(view_words(gs).len() == 0);
                    assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
                }
            } else {
                // Deconstruct head-first: qsv = [head] ++ tail.
                let mut qsm = qs;
                let tail = qsm.split_off(1);  // qsm@ == gs.subrange(0,1); tail@ == gs.subrange(1,..)
                let head = qsm.pop().unwrap();  // head == gs[0]
                let hw = value_to_exec_word(init);  // view_word(hw) == value_to_word(vinit)

                let mut recur: Vec<Word> = Vec::new();
                let ghost r0 = recur@;  // empty
                let ghost x1 = Word::PushQuote(tail);
                let ghost x2 = hw;
                let ghost x3 = head;
                recur.push(Word::PushQuote(tail));
                recur.push(hw);
                recur.push(head);
                let ghost rA = recur@;  // == r0.push(x1).push(x2).push(x3)

                let mut qcc = clone_words(&qc);  // view_words(qcc@) == view_words(gc)
                let ghost gqcc = qcc@;
                proof { assert(view_words(gqcc) == view_words(gc)); }
                recur.append(&mut qcc);  // recur@ == rA + gqcc
                let ghost rB = recur@;

                let ghost y1 = Word::PushQuote(qc);
                let ghost y2 = Word::Prim(SpecPrim::Fold);
                recur.push(Word::PushQuote(qc));
                recur.push(Word::Prim(SpecPrim::Fold));
                recur.append(&mut vm.cont);  // recur@ == rB.push(y1).push(y2) + rest
                vm.cont = recur;

                proof {
                    // view_words(rA) == [PushQuote(tail-view), value_to_word(vinit), head-view]
                    lemma_view_words_push(r0, x1);
                    lemma_view_words_push(r0.push(x1), x2);
                    lemma_view_words_push(r0.push(x1).push(x2), x3);
                    // rB == rA + gqcc
                    lemma_view_words_append(rA, gqcc);
                    // trailing pushes + rest
                    lemma_view_words_push(rB, y1);
                    lemma_view_words_push(rB.push(y1), y2);
                    lemma_view_words_append(
                        rB.push(y1).push(y2), c0.subrange(1, c0.len() as int));

                    // Bridge head/tail views to the spec.
                    lemma_view_words_tail(gs);
                    lemma_view_words_index(gs, 0);

                    let qsview = view_words(gs);
                    let qcview = view_words(gc);
                    let spec_tail = qsview.subrange(1, qsview.len() as int);

                    // Element-level bridges for the three head words of `recur`.
                    assert(tail@ == gs.subrange(1, gs.len() as int));
                    assert(view_word(x1) == SpecWord::PushQuote(spec_tail));
                    assert(view_word(x2) == value_to_word(vinit));
                    assert(view_word(x3) == qsview[0]);
                    assert(view_words(rA) =~= seq![
                        SpecWord::PushQuote(spec_tail),
                        value_to_word(vinit),
                        qsview[0]
                    ]);
                    assert(view_words(gqcc) == qcview);
                    assert(view_word(y1) == SpecWord::PushQuote(qcview));

                    let spec_recur = seq![
                        SpecWord::PushQuote(spec_tail),
                        value_to_word(vinit),
                        qsview[0]
                    ] + qcview + seq![
                        SpecWord::PushQuote(qcview),
                        SpecWord::Prim(SpecPrim::Fold)
                    ];
                    assert(vm.stack@ =~= s0.subrange(0, n as int - 3));
                    assert(view_words(vm.cont@)
                        =~= spec_recur + view_words(c0.subrange(1, c0.len() as int)));
                }
            }
        }
        _ => { assert(false); }
    }
    StepResult::Next
}

// exec-side value_to_word (the interp twin of the spec `value_to_word`).
// P2 leaf refinement: the exec map refines the ghost `value_to_word` under
// the deep view. Both arms are definitional (view_word / view_value / the
// spec value_to_word agree structurally), so no proof body is needed.
pub fn value_to_exec_word(v: Value) -> (res: Word)
    ensures
        view_word(res) == value_to_word(view_value(v)),
{
    match v {
        Value::Int(k) => Word::PushInt(k),
        Value::Quote(q) => Word::PushQuote(q),
    }
}

// P2 leaf refinement of the Add/Sub/Mul arms of spec_step_prim (each an
// application of spec_arith with the +/-/* closure). The single non-view bridge
// is the checked-op fact, split out below so it is stated per operation:
//   a.checked_op(b) == Some(v) <=> in_i64(a int OP b int), and then
//   v as int == a int OP b int; == None <=> !in_i64(a int OP b int).
// We case on `p` so that inside each arm spec_step_prim reduces to the concrete
// spec_arith closure and the bridge assertions are well typed.
pub fn exec_arith(vm: &mut Vm, p: SpecPrim, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
        p is Add || p is Sub || p is Mul,
        view_word(old(vm).cont@[0]) == SpecWord::Prim(p),
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), p, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 2 {
        return StepResult::Fault(Error::Underflow);
    }
    let (a, b) = match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => (*a, *b),
        _ => {
            proof {
                lemma_view_stack_index(s0, n - 2);
                lemma_view_stack_index(s0, n - 1);
            }
            return StepResult::Fault(Error::TypeMismatch);
        }
    };
    proof {
        lemma_view_stack_index(s0, n - 2);
        lemma_view_stack_index(s0, n - 1);
    }
    let res = match p {
        SpecPrim::Add => a.checked_add(b),
        SpecPrim::Sub => a.checked_sub(b),
        _ => a.checked_mul(b),
    };
    match res {
        Some(v) => {
            vm.cont.remove(0);
            vm.stack.pop();
            vm.stack.pop();
            vm.stack.push(Value::Int(v));
            proof {
                // operand views are Int(a int), Int(b int).
                assert(view_stack(s0)[n - 2] == SpecValue::Int(a as int));
                assert(view_stack(s0)[n - 1] == SpecValue::Int(b as int));
                // checked-op bridge: Some(v) with the p-selected op means
                // v as int == (a int OP b int) and that value is in_i64.
                assert(match p {
                    SpecPrim::Add => v as int == a as int + b as int,
                    SpecPrim::Sub => v as int == a as int - b as int,
                    _ => v as int == a as int * b as int,
                });
                assert(in_i64(v as int));
                // continuation + stack fields.
                assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= s0.subrange(0, n - 2).push(Value::Int(v)));
                lemma_view_stack_pop2_push(s0, Value::Int(v));
                assert(view_stack(vm.stack@)
                    == view_stack(s0).subrange(0, n - 2).push(SpecValue::Int(v as int)));
            }
            StepResult::Next
        }
        None => {
            proof {
                assert(view_stack(s0)[n - 2] == SpecValue::Int(a as int));
                assert(view_stack(s0)[n - 1] == SpecValue::Int(b as int));
                // None with the p-selected op means the true int result is out of
                // i64 range, so spec_arith takes the Overflow arm (non-vacuous).
                assert(match p {
                    SpecPrim::Add => !in_i64(a as int + b as int),
                    SpecPrim::Sub => !in_i64(a as int - b as int),
                    _ => !in_i64(a as int * b as int),
                });
            }
            StepResult::Fault(Error::Overflow)
        }
    }
}

// Uniqueness of truncating division: any (q, r) with q*b + r == a, |r| < |b|,
// and r zero-or-same-sign-as-a is THE truncating quotient/remainder. Lets us
// identify two independently-derived truncating decompositions of the same a.
proof fn lemma_trunc_unique(a: int, b: int, q1: int, r1: int, q2: int, r2: int)
    requires
        b != 0,
        q1 * b + r1 == a,
        q2 * b + r2 == a,
        abs_int(r1) < abs_int(b),
        abs_int(r2) < abs_int(b),
        r1 == 0 || (r1 > 0) == (a > 0),
        r2 == 0 || (r2 > 0) == (a > 0),
    ensures
        q1 == q2,
        r1 == r2,
{
    let bb = abs_int(b);
    assert(bb > 0);
    assert(-bb < r1 < bb);
    assert(-bb < r2 < bb);
    // r1 and r2 lie on the same side of 0 (both share a's sign or are 0),
    // so their difference stays strictly within (-|b|, |b|). Linear.
    assert(-bb < r2 - r1 < bb);
    assert((q1 - q2) * b == r2 - r1) by (nonlinear_arith)
        requires q1 * b + r1 == a, q2 * b + r2 == a;
    // |(q1-q2)*b| < |b| with b != 0 forces q1 == q2, hence r1 == r2.
    assert(q1 == q2) by (nonlinear_arith)
        requires
            (q1 - q2) * b == r2 - r1,
            -bb < r2 - r1 < bb,
            bb > 0,
            b == bb || b == -bb;
}

// Value bridge: MTL's truncating trunc_div/trunc_mod coincide with vstd's
// rust_div/rust_rem (the model of Rust's `/`/`%` that checked_div/checked_rem
// return). Verus int `/`,`%` are Euclidean (SMT `div`/`mod`, remainder in
// [0,|b|)); rust_div/rust_rem wrap them into truncating form. We prove equality
// by showing rust_div/rust_rem form a valid truncating decomposition of `a`,
// then invoking uniqueness against trunc_divmod_correct.
proof fn lemma_trunc_is_rust(a: int, b: int)
    requires
        b != 0,
    ensures
        trunc_div(a, b) == rust_div(a, b),
        trunc_mod(a, b) == rust_rem(a, b),
{
    trunc_divmod_correct(a, b);
    lemma_fundamental_div_mod(a, b);        // a == b*(a/b) + a%b
    lemma_fundamental_div_mod(-a, b);       // -a == b*((-a)/b) + (-a)%b
    // Euclidean remainder range (SMT `mod`): 0 <= x%b < |b|.
    assert(0 <= a % b < abs_int(b));
    assert(0 <= (-a) % b < abs_int(b));
    let rd = rust_div(a, b);
    let rr = rust_rem(a, b);
    // rust decomposition: rd*b + rr == a (cases on sign of a, via fundamental).
    assert(rd * b + rr == a) by (nonlinear_arith)
        requires
            a == b * (a / b) + (a % b),
            (-a) == b * ((-a) / b) + ((-a) % b),
            rd == rust_div(a, b),
            rr == rust_rem(a, b);
    // rust remainder is bounded and zero-or-same-sign-as-a.
    assert(abs_int(rr) < abs_int(b));
    assert(rr == 0 || (rr > 0) == (a > 0));
    lemma_trunc_unique(a, b, trunc_div(a, b), trunc_mod(a, b), rd, rr);
}

// trunc_div stays in i64 EXCEPT at the single MIN/-1 point (|trunc_div| <= |a|,
// and the only way |a|/|b| reaches 2^63 with a positive sign is a==MIN, b==-1).
// This is exactly the boundary checked_div/checked_rem report as overflow.
proof fn lemma_trunc_div_in_range(a: int, b: int)
    requires
        in_i64(a),
        in_i64(b),
        b != 0,
        !(a == -0x8000_0000_0000_0000 && b == -1),
    ensures
        in_i64(trunc_div(a, b)),
{
    let aa = abs_int(a);
    let ab = abs_int(b);
    let q = aa / ab;
    let r = aa % ab;
    assert(aa == ab * q + r && 0 <= r < ab) by (nonlinear_arith)
        requires aa >= 0, ab > 0, q == aa / ab, r == aa % ab;
    assert(ab >= 1);
    assert(0 <= q) by (nonlinear_arith)
        requires aa == ab * q + r, 0 <= r < ab, ab >= 1, aa >= 0;
    assert(q <= aa) by (nonlinear_arith)
        requires aa == ab * q + r, 0 <= r, ab >= 1, q >= 0;
    assert(aa <= 0x8000_0000_0000_0000);
    if (a >= 0) == (b >= 0) {
        assert(trunc_div(a, b) == q);  // same sign -> +q
        if a == -0x8000_0000_0000_0000 {
            // same sign & a<0 => b<0; b != -1 => b <= -2 => |b| >= 2 => q <= |a|/2.
            assert(b <= -2);
            assert(ab >= 2);
            assert(2 * q <= aa) by (nonlinear_arith)
                requires aa == ab * q + r, 0 <= r, ab >= 2, q >= 0;
            assert(q <= 0x7FFF_FFFF_FFFF_FFFF);
        } else {
            // a != MIN and in_i64(a) => |a| <= MAX, and q <= |a|.
            assert(aa <= 0x7FFF_FFFF_FFFF_FFFF);
            assert(q <= 0x7FFF_FFFF_FFFF_FFFF);
        }
    } else {
        assert(trunc_div(a, b) == -q);  // opposite sign -> -q, in [MIN, 0].
        assert(-q >= -0x8000_0000_0000_0000);
    }
}

// P2 leaf refinement of the Div/Mod arms of spec_step_prim (both spec_divmod).
// Fault ordering is preserved exactly: arity (Underflow) -> type (TypeMismatch)
// -> DivByZero (b == 0) -> Overflow (only i64::MIN / -1). The b==0 check comes
// BEFORE the checked op, matching spec_divmod's `if b == 0` guard ahead of its
// `!in_i64(trunc_div(a,b))` guard.
pub fn exec_divmod(vm: &mut Vm, is_div: bool, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
        view_word(old(vm).cont@[0])
            == SpecWord::Prim(if is_div { SpecPrim::Div } else { SpecPrim::Mod }),
    ensures ({
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        let p = if is_div { SpecPrim::Div } else { SpecPrim::Mod };
        match spec_step_prim(view_stack(old(vm).stack@), p, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    if n < 2 {
        return StepResult::Fault(Error::Underflow);
    }
    let (a, b) = match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => (*a, *b),
        _ => {
            proof {
                lemma_view_stack_index(s0, n - 2);
                lemma_view_stack_index(s0, n - 1);
            }
            return StepResult::Fault(Error::TypeMismatch);
        }
    };
    proof {
        lemma_view_stack_index(s0, n - 2);
        lemma_view_stack_index(s0, n - 1);
    }
    // DivByZero BEFORE Overflow, mirroring spec_divmod's guard order.
    if b == 0 {
        return StepResult::Fault(Error::DivByZero);
    }
    let res = if is_div { a.checked_div(b) } else { a.checked_rem(b) };
    match res {
        Some(v) => {
            vm.cont.remove(0);
            vm.stack.pop();
            vm.stack.pop();
            vm.stack.push(Value::Int(v));
            proof {
                // Some(v) with b != 0 rules out the MIN/-1 overflow point.
                assert(!(a as int == -0x8000_0000_0000_0000 && b as int == -1));
                lemma_trunc_is_rust(a as int, b as int);
                lemma_trunc_div_in_range(a as int, b as int);
                // v is the truncating quotient/remainder = trunc_div/trunc_mod.
                assert(v as int == if is_div {
                    trunc_div(a as int, b as int)
                } else {
                    trunc_mod(a as int, b as int)
                });
                assert(in_i64(trunc_div(a as int, b as int)));
                // operands + fields.
                assert(view_stack(s0)[n - 2] == SpecValue::Int(a as int));
                assert(view_stack(s0)[n - 1] == SpecValue::Int(b as int));
                assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
                assert(vm.stack@ =~= s0.subrange(0, n - 2).push(Value::Int(v)));
                lemma_view_stack_pop2_push(s0, Value::Int(v));
                assert(view_stack(vm.stack@)
                    == view_stack(s0).subrange(0, n - 2).push(SpecValue::Int(v as int)));
            }
            StepResult::Next
        }
        None => {
            proof {
                // With b != 0, checked_div/checked_rem == None <=> a == MIN && b == -1.
                assert(a as int == -0x8000_0000_0000_0000 && b as int == -1);
                // trunc_div(MIN, -1) == 2^63, which is NOT in_i64 (non-vacuous: the
                // spec Overflow arm fires for the SAME MIN/-1 input, for both div and
                // mod, exactly as checked_rem(MIN,-1) is also None).
                assert(abs_int(a as int) == 0x8000_0000_0000_0000);
                assert(abs_int(b as int) == 1);
                assert((0x8000_0000_0000_0000int) / (1int) == 0x8000_0000_0000_0000) by (nonlinear_arith);
                assert(trunc_div(a as int, b as int) == 0x8000_0000_0000_0000);
                assert(!in_i64(trunc_div(a as int, b as int)));
                assert(view_stack(s0)[n - 2] == SpecValue::Int(a as int));
                assert(view_stack(s0)[n - 1] == SpecValue::Int(b as int));
            }
            StepResult::Fault(Error::Overflow)
        }
    }
}

// P2 leaf refinement of the Eq/Lt arms of spec_step_prim. The comparison
// leaves are the simplest binop shape: TOTAL (arity -> type ordering only, no
// Overflow/DivByZero arm), so the only bridge needed is the view plumbing.
// Precondition: `n` is the (un-mutated) operand-stack height and cont[0] is the
// matching Prim, so spec_step_prim(view_stack(stack), p, rest) is what
// spec_step dispatches to.
pub fn exec_cmp(vm: &mut Vm, is_eq: bool, n: usize) -> (r: StepResult)
    requires
        n == old(vm).stack.len(),
        old(vm).cont.len() >= 1,
        view_word(old(vm).cont@[0])
            == SpecWord::Prim(if is_eq { SpecPrim::Eq } else { SpecPrim::Lt }),
    ensures ({
        let p = if is_eq { SpecPrim::Eq } else { SpecPrim::Lt };
        let rest = view_words(old(vm).cont@.subrange(1, old(vm).cont@.len() as int));
        match spec_step_prim(view_stack(old(vm).stack@), p, rest) {
            SpecStep::Next(s2) => r is Next && final(vm).deep_view() == s2,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
            SpecStep::Halt(_) => false,
        }
    }),
{
    let ghost s0 = vm.stack@;
    let ghost c0 = vm.cont@;
    proof { lemma_view_stack_len(vm.stack@); }
    // (1) arity: spec n == view_stack(stack).len() == stack.len() == n.
    if n < 2 {
        return StepResult::Fault(Error::Underflow);
    }
    let (a, b) = match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => (*a, *b),
        _ => {
            // (2) type: a non-Int exec operand views to a non-Int SpecValue, so
            // the spec `(Int, Int)` match also falls through to TypeMismatch.
            proof {
                lemma_view_stack_index(s0, n - 2);
                lemma_view_stack_index(s0, n - 1);
            }
            return StepResult::Fault(Error::TypeMismatch);
        }
    };
    proof {
        lemma_view_stack_index(s0, n - 2);
        lemma_view_stack_index(s0, n - 1);
        // exec Int match ==> view_stack(s0)[n-2] == Int(a as int), likewise n-1;
        // and (a == b as i64) <=> (a as int == b as int), (a < b) <=> (a as int < b as int).
    }
    let v: i64 = if is_eq { if a == b { 1 } else { 0 } } else { if a < b { 1 } else { 0 } };
    vm.cont.remove(0);
    vm.stack.pop();
    vm.stack.pop();
    vm.stack.push(Value::Int(v));
    proof {
        // continuation field: remove(0) == subrange(1, len); view_words is a fn.
        assert(vm.cont@ =~= c0.subrange(1, c0.len() as int));
        // stack field: two pops then push == the pop2_push shape on s0.
        assert(vm.stack@ =~= s0.subrange(0, n - 2).push(Value::Int(v)));
        lemma_view_stack_pop2_push(s0, Value::Int(v));
        assert(view_stack(vm.stack@)
            == view_stack(s0).subrange(0, n - 2).push(SpecValue::Int(v as int)));
        // operand views are Int(a as int), Int(b as int).
        assert(view_stack(s0)[n - 2] == SpecValue::Int(a as int));
        assert(view_stack(s0)[n - 1] == SpecValue::Int(b as int));
        // v as int matches the spec Eq/Lt comparator on the int operands.
        assert(v as int == if is_eq {
            if (a as int) == (b as int) { 1int } else { 0int }
        } else {
            if (a as int) < (b as int) { 1int } else { 0int }
        });
    }
    StepResult::Next
}

// ------------------------------------------------------------
// Spec-level iterated step: the FAITHFUL iteration of `spec_step` that `run`
// refines. This does NOT alter spec_step or any existing spec semantics — it
// only NAMES the finite unrolling (up to `fuel` steps) that the exec `run`
// loop computes. Its result mirrors the thin exec `Outcome`
// (Halt(stack) / Fault(e) / FuelExhausted) — the only observations the exec
// driver can make.
// ------------------------------------------------------------
pub enum SpecOutcome {
    Halt(Seq<SpecValue>),
    Fault(Error),
    FuelExhausted,
}

pub open spec fn spec_run(s: SpecState, fuel: nat) -> SpecOutcome
    decreases fuel,
{
    if fuel == 0 {
        SpecOutcome::FuelExhausted
    } else {
        match spec_step(s) {
            SpecStep::Halt(stk) => SpecOutcome::Halt(stk),
            SpecStep::Fault(e) => SpecOutcome::Fault(e),
            SpecStep::Next(s2) => spec_run(s2, (fuel - 1) as nat),
        }
    }
}

// Fuel-bounded driver. Termination NOT provable (MTL is TC), but the finite
// unrolling up to `fuel` IS: VERIFIED that `run` refines `spec_run`, a faithful
// iteration of `spec_step`. The postcondition speaks only in terms of the thin
// `Outcome` (rich fault state is intentionally discarded — see roadmap §"Two
// Outcome types").
pub fn run(vm: &mut Vm, fuel: u64) -> (res: Outcome)
    ensures
        match spec_run(old(vm).deep_view(), fuel as nat) {
            SpecOutcome::Halt(stk) => res is Halt && view_stack((res->Halt_0)@) == stk,
            SpecOutcome::Fault(e) => res == Outcome::Fault(e),
            SpecOutcome::FuelExhausted => res is FuelExhausted,
        },
{
    let ghost s0 = old(vm).deep_view();
    let mut steps: u64 = 0;
    while steps < fuel
        invariant
            steps <= fuel,
            s0 == old(vm).deep_view(),
            spec_run(s0, fuel as nat) == spec_run(vm.deep_view(), (fuel - steps) as nat),
        decreases fuel - steps,
    {
        proof { lemma_view_words_len(vm.cont@); }
        if vm.cont.len() == 0 {
            // deep_view().cont is empty, so spec_step is Halt(stack): this loop
            // iteration's spec_run reduces to Halt of the current stack.
            let ghost pre = vm.deep_view();
            let ghost pre_stack = vm.stack@;
            let mut out: Vec<Value> = Vec::new();
            out.append(&mut vm.stack);
            proof {
                assert((fuel - steps) as nat >= 1);
                assert(pre.cont.len() == 0);
                assert(spec_step(pre) == SpecStep::Halt(pre.stack));
                assert(spec_run(pre, (fuel - steps) as nat) == SpecOutcome::Halt(pre.stack));
                assert(spec_run(s0, fuel as nat) == SpecOutcome::Halt(pre.stack));
                // out@ == the pre-drain stack; its view is the spec halt seq.
                assert(out@ =~= pre_stack);
                assert(view_stack(out@) == pre.stack);
            }
            return Outcome::Halt(out);
        }
        let ghost pre = vm.deep_view();
        proof {
            // cont non-empty => spec_step(pre) is Next or Fault, never Halt.
            assert(pre.cont.len() >= 1);
        }
        match exec_step(vm) {
            StepResult::Next => {
                proof {
                    assert((fuel - steps) as nat >= 1);
                    // spec_step(pre) == Next(vm.deep_view()); peel one step.
                    assert(spec_run(pre, (fuel - steps) as nat)
                        == spec_run(vm.deep_view(), (fuel - steps - 1) as nat));
                }
                steps = steps + 1;
            }
            StepResult::Halt => {
                proof { assert(false); }
            }
            StepResult::Fault(e) => {
                proof {
                    assert((fuel - steps) as nat >= 1);
                    assert(spec_step(pre) == SpecStep::Fault(e));
                    assert(spec_run(pre, (fuel - steps) as nat) == SpecOutcome::Fault(e));
                    assert(spec_run(s0, fuel as nat) == SpecOutcome::Fault(e));
                }
                return Outcome::Fault(e);
            }
        }
    }
    proof {
        assert(steps == fuel);
        assert(spec_run(vm.deep_view(), 0nat) == SpecOutcome::FuelExhausted);
    }
    Outcome::FuelExhausted
}

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

// --- Smoke theorems for the v0.2 recursion primitives (design §3, §10.2). ---
// These mirror the `smoke_dup_apply` style: a single symbolic spec step from a
// constructed state, asserting the exact successor. primrec/times get both the
// base (count exhausted) and step (recur expansion) cases — the count strictly
// decreases, which is a *stronger* guarantee than anything provable about `:!`.
// linrec gets only the one-step DESUGARING into `If` (no termination claim,
// since it is partial like `!`): this reduces linrec to the verified `If` arm.

// primrec base: k<=0 discards the count and runs I on the base stack.
pub proof fn smoke_primrec_base(qi: Seq<SpecWord>, qc: Seq<SpecWord>, k: int)
    requires k <= 0,
    ensures ({
        let s0 = SpecState {
            stack: seq![SpecValue::Int(k), SpecValue::Quote(qi), SpecValue::Quote(qc)],
            cont: seq![SpecWord::Prim(SpecPrim::PrimRec)],
        };
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack =~= Seq::<SpecValue>::empty()
        &&& spec_step(s0)->Next_0.cont =~= qi
    }),
{
}

// primrec step: k>0 keeps n, recurses on (n-1), then folds C over the subresult.
pub proof fn smoke_primrec_step(qi: Seq<SpecWord>, qc: Seq<SpecWord>, k: int)
    requires k > 0,
    ensures ({
        let s0 = SpecState {
            stack: seq![SpecValue::Int(k), SpecValue::Quote(qi), SpecValue::Quote(qc)],
            cont: seq![SpecWord::Prim(SpecPrim::PrimRec)],
        };
        let recur = seq![
            SpecWord::PushInt(k),
            SpecWord::PushInt(k - 1),
            SpecWord::PushQuote(qi),
            SpecWord::PushQuote(qc),
            SpecWord::Prim(SpecPrim::PrimRec)
        ] + qc;
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack =~= Seq::<SpecValue>::empty()
        &&& spec_step(s0)->Next_0.cont =~= recur
    }),
{
}

// times base: k<=0 is a no-op (Q is discarded, continuation is `rest`).
pub proof fn smoke_times_base(q: Seq<SpecWord>, k: int)
    requires k <= 0,
    ensures ({
        let s0 = SpecState {
            stack: seq![SpecValue::Int(k), SpecValue::Quote(q)],
            cont: seq![SpecWord::Prim(SpecPrim::Times)],
        };
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack =~= Seq::<SpecValue>::empty()
        &&& spec_step(s0)->Next_0.cont =~= Seq::<SpecWord>::empty()
    }),
{
}

// times step: k>0 runs Q once then times(n-1) Q.
pub proof fn smoke_times_step(q: Seq<SpecWord>, k: int)
    requires k > 0,
    ensures ({
        let s0 = SpecState {
            stack: seq![SpecValue::Int(k), SpecValue::Quote(q)],
            cont: seq![SpecWord::Prim(SpecPrim::Times)],
        };
        let recur = q + seq![
            SpecWord::PushInt(k - 1),
            SpecWord::PushQuote(q),
            SpecWord::Prim(SpecPrim::Times)
        ];
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack =~= Seq::<SpecValue>::empty()
        &&& spec_step(s0)->Next_0.cont =~= recur
    }),
{
}

// linrec desugaring: one step splices P then an `If` over T and the else-quote,
// reducing linrec to the verified `If` arm (no new control operator).
pub proof fn smoke_linrec_desugar(
    qp: Seq<SpecWord>, qt: Seq<SpecWord>, qr1: Seq<SpecWord>, qr2: Seq<SpecWord>,
)
    ensures ({
        let s0 = SpecState {
            stack: seq![
                SpecValue::Quote(qp), SpecValue::Quote(qt),
                SpecValue::Quote(qr1), SpecValue::Quote(qr2)
            ],
            cont: seq![SpecWord::Prim(SpecPrim::LinRec)],
        };
        let else_q = qr1 + seq![
            SpecWord::PushQuote(qp),
            SpecWord::PushQuote(qt),
            SpecWord::PushQuote(qr1),
            SpecWord::PushQuote(qr2),
            SpecWord::Prim(SpecPrim::LinRec)
        ] + qr2;
        let spliced = qp + seq![
            SpecWord::PushQuote(qt),
            SpecWord::PushQuote(else_q),
            SpecWord::Prim(SpecPrim::If)
        ];
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack =~= Seq::<SpecValue>::empty()
        &&& spec_step(s0)->Next_0.cont =~= spliced
    }),
{
}

// uncons empty: an empty quotation pushes only the flag 0.
pub proof fn smoke_uncons_empty()
    ensures ({
        let s0 = SpecState {
            stack: seq![SpecValue::Quote(Seq::<SpecWord>::empty())],
            cont: seq![SpecWord::Prim(SpecPrim::Uncons)],
        };
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack =~= seq![SpecValue::Int(0int)]
        &&& spec_step(s0)->Next_0.cont =~= Seq::<SpecWord>::empty()
    }),
{
}

// uncons head-int: a quote whose head is PushInt(i) splits into i, [tail], 1.
pub proof fn smoke_uncons_head_int(i: int, t: Seq<SpecWord>)
    ensures ({
        let q = seq![SpecWord::PushInt(i)] + t;
        let s0 = SpecState {
            stack: seq![SpecValue::Quote(q)],
            cont: seq![SpecWord::Prim(SpecPrim::Uncons)],
        };
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack
              =~= seq![SpecValue::Int(i), SpecValue::Quote(t), SpecValue::Int(1int)]
        &&& spec_step(s0)->Next_0.cont =~= Seq::<SpecWord>::empty()
    }),
{
    assert((seq![SpecWord::PushInt(i)] + t)[0] == SpecWord::PushInt(i));
    assert((seq![SpecWord::PushInt(i)] + t).subrange(1, (seq![SpecWord::PushInt(i)] + t).len() as int) =~= t);
}

// --- Smoke theorems for the v0.3 sequence primitives (design §3, §10.2). ---
// Same style as the v0.2 recursion primitives: a single symbolic spec step from
// a constructed state, asserting the exact successor. `fold` gets both the base
// (empty list) and step (non-empty re-emission) cases — the spine strictly
// shrinks (tail is one shorter than qs), which is the SAME well-founded measure
// primrec/times rely on (count -> 0), a STRONGER guarantee than linrec's (which
// gets no termination claim). `xor` gets the one-step total re-write, the trivial
// P2 lock-step pair (spec `i64_bitxor` vs exec `a ^ b`, agree definitionally).

// fold base: an empty sequence steps to `push init` (the seed accumulator).
pub proof fn smoke_fold_base(init: SpecValue, qc: Seq<SpecWord>)
    ensures ({
        let s0 = SpecState {
            stack: seq![
                SpecValue::Quote(Seq::<SpecWord>::empty()), init, SpecValue::Quote(qc)
            ],
            cont: seq![SpecWord::Prim(SpecPrim::Fold)],
        };
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack =~= seq![init]
        &&& spec_step(s0)->Next_0.cont =~= Seq::<SpecWord>::empty()
    }),
{
}

// fold step: a non-empty sequence [h | tail] steps to the native re-emission
//   [tail] init <push h> C [C] Fold  — the §3.1 desugar. The spine shrinks
// (tail.len() == qs.len()-1), the termination measure that makes fold total on
// a finite list, exactly like primrec's count strictly decreasing.
pub proof fn smoke_fold_step(h: int, tail: Seq<SpecWord>, init: SpecValue, qc: Seq<SpecWord>)
    ensures ({
        let qs = seq![SpecWord::PushInt(h)] + tail;
        let s0 = SpecState {
            stack: seq![SpecValue::Quote(qs), init, SpecValue::Quote(qc)],
            cont: seq![SpecWord::Prim(SpecPrim::Fold)],
        };
        let recur = seq![
            SpecWord::PushQuote(tail),
            value_to_word(init),
            SpecWord::PushInt(h)
        ] + qc + seq![
            SpecWord::PushQuote(qc),
            SpecWord::Prim(SpecPrim::Fold)
        ];
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack =~= Seq::<SpecValue>::empty()
        &&& spec_step(s0)->Next_0.cont =~= recur
    }),
{
    let qs = seq![SpecWord::PushInt(h)] + tail;
    assert(qs[0] == SpecWord::PushInt(h));
    assert(qs.subrange(1, qs.len() as int) =~= tail);
}

// xor: two Ints step to their i64 two's-complement XOR. Total (no Overflow arm),
// definitional lock-step of spec `i64_bitxor` against exec `a ^ b`.
pub proof fn smoke_xor(a: int, b: int)
    ensures ({
        let s0 = SpecState {
            stack: seq![SpecValue::Int(a), SpecValue::Int(b)],
            cont: seq![SpecWord::Prim(SpecPrim::Xor)],
        };
        &&& spec_step(s0) is Next
        &&& spec_step(s0)->Next_0.stack =~= seq![SpecValue::Int(i64_bitxor(a, b))]
        &&& spec_step(s0)->Next_0.cont =~= Seq::<SpecWord>::empty()
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

// P2 — Refinement theorem (real, non-vacuous). `exec_step` refines `spec_step`
// across `deep_view`, in strong iff/equality form, and one exec step is one
// iteration of `spec_run` (the faithful fuel-bounded iteration that `run`
// refines).
//
// SHAPE. `H` (the `requires`) is EXACTLY the machine-checked `ensures` Verus
// proved on `exec_step` above, instantiated at a concrete transition:
// pre-state `vm`, result `r`, post-state `vm2`. A `proof fn` cannot call the
// `&mut` exec fns, so the verified exec contract enters as this hypothesis; the
// lemma then re-expresses it in the human-readable P2 form and links it to
// `spec_run`.
//
// CONCLUSION (the `ensures`):
//   * FAULT iff, with error equality — exec faults exactly when the spec
//     faults, and with the SAME `Error`.
//   * HALT iff — exec halts exactly when the spec halts.
//   * NEXT successor EQUALITY — when the spec advances, exec advances and the
//     post-state's `deep_view` equals EXACTLY the spec successor.
//   * MULTI-STEP corollary — for all `fuel`, one exec step is one peel of
//     `spec_run`: `spec_run(pre, fuel+1)` reduces to `Halt`/`Fault` of the
//     spec outcome, or to `spec_run(post, fuel)` in the Next case. This is the
//     exact recurrence the `run` loop performs (see `run`'s Next arm), so it
//     ties this one-step lemma to the end-to-end fuel-bounded refinement.
//
// NON-VACUITY (self-audit). This is NOT the old total disjunction
// `spec_step(..) is Next || is Halt || is Fault`, which is vacuously true for
// ANY result because `SpecStep` is a total 3-constructor enum. Here:
//   - The NEXT clause PINS `vm2.deep_view() == spec_step(vm.deep_view())->Next_0`
//     by EQUALITY. Swap the spec successor for any other `SpecState` and the
//     conclusion becomes false (it names one specific state, not "some state").
//     A wrong post-state cannot satisfy it — the statement genuinely constrains.
//   - The FAULT clause is a genuine bi-implication with `Error` equality, not a
//     one-directional disjunct: it forbids exec faulting when the spec advances
//     and forbids a mismatched error code.
//   - The MULTI-STEP clause equates `spec_run(pre, fuel+1)` with a specific
//     value involving `post`; it is false for an unrelated `post`.
//
// HONESTY. This lemma is a COROLLARY. The self-contained P2 carriers are the
// verified `ensures` on `exec_step` (one step) and `run` (fuel-bounded,
// refining `spec_run`) — those hold with no hypothesis. This lemma names the
// refinement property in its strong iff/equality form and derives it from the
// exec contract `H`, additionally connecting one step to the `spec_run`
// iteration. Complementary executable evidence: the differential proptest
// oracle in `crates/mtl-core/tests/interpreter.rs`.
pub proof fn p2_refinement(vm: Vm, vm2: Vm, r: StepResult)
    requires
        // H: the verified one-step contract of `exec_step`, at (vm -> vm2, r).
        match spec_step(vm.deep_view()) {
            SpecStep::Next(s2) => r is Next && vm2.deep_view() == s2,
            SpecStep::Halt(_) => r is Halt,
            SpecStep::Fault(e) => r == StepResult::Fault(e),
        },
    ensures
        // FAULT iff, with error equality.
        (r is Fault) <==> (spec_step(vm.deep_view()) is Fault),
        (spec_step(vm.deep_view()) is Fault)
            ==> r == StepResult::Fault(spec_step(vm.deep_view())->Fault_0),
        // HALT iff.
        (r is Halt) <==> (spec_step(vm.deep_view()) is Halt),
        // NEXT: exec advances and the post-state deep_view PINS the spec successor.
        (spec_step(vm.deep_view()) is Next)
            ==> (r is Next && vm2.deep_view() == spec_step(vm.deep_view())->Next_0),
        // MULTI-STEP corollary: one exec step == one peel of `spec_run`.
        forall|fuel: nat| #![trigger spec_run(vm.deep_view(), fuel + 1)]
            spec_run(vm.deep_view(), fuel + 1) == (
                match spec_step(vm.deep_view()) {
                    SpecStep::Halt(stk) => SpecOutcome::Halt(stk),
                    SpecStep::Fault(e) => SpecOutcome::Fault(e),
                    SpecStep::Next(_) => spec_run(vm2.deep_view(), fuel),
                }),
{
    // One-step iff/equality facts follow from `H` by case analysis on the
    // total 3-constructor `SpecStep` (SMT resolves constructor distinctness of
    // `StepResult::{Next,Halt,Fault}`).
    assert forall|fuel: nat| #![trigger spec_run(vm.deep_view(), fuel + 1)]
        spec_run(vm.deep_view(), fuel + 1) == (
            match spec_step(vm.deep_view()) {
                SpecStep::Halt(stk) => SpecOutcome::Halt(stk),
                SpecStep::Fault(e) => SpecOutcome::Fault(e),
                SpecStep::Next(_) => spec_run(vm2.deep_view(), fuel),
            }) by {
        // `spec_run(s, fuel+1)` unfolds (fuel+1 > 0) to a match on
        // `spec_step(s)`; in the Next branch its successor is `vm2.deep_view()`
        // by `H`, so the recursive call is `spec_run(vm2.deep_view(), fuel)`.
        assert((fuel + 1 - 1) as nat == fuel);
    }
}

// P5 — TC lock-step lemma (spec §6): Minsky simulation invariant R
// preserved by bounded MTL step sequences. Stated in PROOF phase after
// the Minsky spec machine is transcribed. Hard; scheduled last.

} // verus!

fn main() {}
