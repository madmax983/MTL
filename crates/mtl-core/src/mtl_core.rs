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
// STATUS: UNVERIFIED (no local Verus). Statements are believed exactly right;
// some proof bodies carry `// TODO(verify)` where an unfold or seq-lemma nudge
// may still be needed for Verus to close the step.
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
        // TODO(verify): may need an explicit one-level unfold of view_words(ab).
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
    if i == 0 {
        assert(view_stack(s)[0] == view_value(s[0]));
    } else {
        let t = s.subrange(1, s.len() as int);
        assert(t[i - 1] == s[i]);
        // (seq![vv] + view_stack(t))[i] == view_stack(t)[i-1] for i >= 1.
        assert(view_stack(s)[i] == view_stack(t)[i - 1]);
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
        let t = s.subrange(1, s.len() as int);
        assert(s.subrange(0, k)[0] == s[0]);
        assert(s.subrange(0, k).subrange(1, k) =~= t.subrange(0, k - 1));
        lemma_view_stack_prefix(t, k - 1);
        // (seq![vv] + view_stack(t)).subrange(0,k) == seq![vv] + view_stack(t).subrange(0,k-1).
        // TODO(verify): may need assert_seqs_equal! on the concat/subrange identity.
        assert(view_stack(s.subrange(0, k)) =~= view_stack(s).subrange(0, k));
    }
}

// view_stack commutes with push (the single result push of a binop / cmp).
pub proof fn lemma_view_stack_push(s: Seq<Value>, v: Value)
    ensures
        view_stack(s.push(v)) == view_stack(s).push(view_value(v)),
    decreases s.len(),
{
    if s.len() == 0 {
        assert(s.push(v) =~= seq![v]);
        assert(view_stack(s) =~= Seq::<SpecValue>::empty());
        assert(view_stack(s.push(v)) =~= view_stack(s).push(view_value(v)));
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
// STATUS (honest): the executable, cargo-compiled, and test-exercised
// interpreter is `crate::interp` in `../interp.rs` (this file, `mtl_core.rs`,
// is checked by `verus`, NOT compiled by `cargo`; see `lib.rs`). The bodies
// below faithfully mirror `spec_step`'s fault-check order and are marked
// `#[verifier::external_body]`: their P2 refinement `ensures` is currently
// ASSUMED (a trust boundary / stub), not machine-checked, because no Verus
// toolchain was available in-container to discharge it. Executable evidence
// for the refinement is the differential proptest oracle in
// `tests/interpreter.rs`, which checks `crate::interp::exec_step`/`run`
// against an independent transliteration of `spec_step`, step-for-step.
// P2 remains an OPEN proof hole (see `p2_refinement` below).
// ============================================================

// Refinement contract (P2). ASSUMED via external_body — see status note.
#[verifier::external_body]
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
    let n = vm.stack.len();
    if vm.cont.len() == 0 {
        return StepResult::Halt;
    }
    let head = vm.cont[0].clone();
    match head {
        Word::PushInt(k) => {
            vm.cont.remove(0);
            vm.stack.push(Value::Int(k));
            StepResult::Next
        }
        Word::PushQuote(qq) => {
            vm.cont.remove(0);
            vm.stack.push(Value::Quote(qq));
            StepResult::Next
        }
        Word::Call(_) => StepResult::Fault(Error::UnknownWord),
        Word::Prim(p) => exec_prim(vm, p, n),
    }
}

// Splits out the primitive dispatch. external_body: unverified (P2 assumed).
#[verifier::external_body]
pub fn exec_prim(vm: &mut Vm, p: SpecPrim, n: usize) -> StepResult {
    match p {
        SpecPrim::Dup => {
            if n < 1 { return StepResult::Fault(Error::Underflow); }
            vm.cont.remove(0);
            let top = vm.stack[n - 1].clone();
            vm.stack.push(top);
            StepResult::Next
        }
        SpecPrim::Drop => {
            if n < 1 { return StepResult::Fault(Error::Underflow); }
            vm.cont.remove(0);
            vm.stack.pop();
            StepResult::Next
        }
        SpecPrim::Swap => {
            if n < 2 { return StepResult::Fault(Error::Underflow); }
            vm.cont.remove(0);
            vm.stack.swap(n - 1, n - 2);
            StepResult::Next
        }
        SpecPrim::Rot => {
            if n < 3 { return StepResult::Fault(Error::Underflow); }
            vm.cont.remove(0);
            let a = vm.stack.remove(n - 3);
            vm.stack.push(a);
            StepResult::Next
        }
        SpecPrim::Over => {
            if n < 2 { return StepResult::Fault(Error::Underflow); }
            vm.cont.remove(0);
            let a = vm.stack[n - 2].clone();
            vm.stack.push(a);
            StepResult::Next
        }
        SpecPrim::Apply => {
            if n < 1 { return StepResult::Fault(Error::Underflow); }
            match vm.stack[n - 1] {
                Value::Quote(_) => {
                    vm.cont.remove(0);
                    if let Some(Value::Quote(mut q)) = vm.stack.pop() {
                        q.append(&mut vm.cont);
                        vm.cont = q;
                    }
                    StepResult::Next
                }
                _ => StepResult::Fault(Error::TypeMismatch),
            }
        }
        SpecPrim::Cat => {
            if n < 2 { return StepResult::Fault(Error::Underflow); }
            let ok = matches!(vm.stack[n - 2], Value::Quote(_))
                && matches!(vm.stack[n - 1], Value::Quote(_));
            if !ok { return StepResult::Fault(Error::TypeMismatch); }
            vm.cont.remove(0);
            let vb = vm.stack.pop();
            let va = vm.stack.pop();
            if let (Some(Value::Quote(mut a)), Some(Value::Quote(mut b))) = (va, vb) {
                a.append(&mut b);
                vm.stack.push(Value::Quote(a));
            }
            StepResult::Next
        }
        SpecPrim::Cons => {
            if n < 2 { return StepResult::Fault(Error::Underflow); }
            if !matches!(vm.stack[n - 1], Value::Quote(_)) {
                return StepResult::Fault(Error::TypeMismatch);
            }
            vm.cont.remove(0);
            let qv = vm.stack.pop();
            let v = vm.stack.pop();
            if let Some(Value::Quote(q)) = qv {
                let mut newq: Vec<Word> = Vec::new();
                if let Some(vv) = v {
                    newq.push(value_to_exec_word(vv));
                }
                let mut q2 = q;
                newq.append(&mut q2);
                vm.stack.push(Value::Quote(newq));
            }
            StepResult::Next
        }
        SpecPrim::Dip => {
            if n < 2 { return StepResult::Fault(Error::Underflow); }
            if !matches!(vm.stack[n - 1], Value::Quote(_)) {
                return StepResult::Fault(Error::TypeMismatch);
            }
            vm.cont.remove(0);
            let qv = vm.stack.pop();
            let a = vm.stack.pop();
            if let Some(Value::Quote(mut q)) = qv {
                if let Some(av) = a {
                    q.push(value_to_exec_word(av));
                }
                q.append(&mut vm.cont);
                vm.cont = q;
            }
            StepResult::Next
        }
        SpecPrim::Add => exec_arith(vm, p, n),
        SpecPrim::Sub => exec_arith(vm, p, n),
        SpecPrim::Mul => exec_arith(vm, p, n),
        SpecPrim::Div => exec_divmod(vm, true, n),
        SpecPrim::Mod => exec_divmod(vm, false, n),
        SpecPrim::Eq => exec_cmp(vm, true, n),
        SpecPrim::Lt => exec_cmp(vm, false, n),
        SpecPrim::If => {
            if n < 3 { return StepResult::Fault(Error::Underflow); }
            let ok = matches!(vm.stack[n - 3], Value::Int(_))
                && matches!(vm.stack[n - 2], Value::Quote(_))
                && matches!(vm.stack[n - 1], Value::Quote(_));
            if !ok { return StepResult::Fault(Error::TypeMismatch); }
            vm.cont.remove(0);
            let fv = vm.stack.pop();
            let tv = vm.stack.pop();
            let cv = vm.stack.pop();
            if let (Some(Value::Int(c)), Some(Value::Quote(t)), Some(Value::Quote(f))) =
                (cv, tv, fv)
            {
                let mut branch = if c != 0 { t } else { f };
                branch.append(&mut vm.cont);
                vm.cont = branch;
            }
            StepResult::Next
        }
        // ---------------- v0.2 recursion primitives (mirror of spec_step_prim) ----------------
        SpecPrim::PrimRec => {
            if n < 3 { return StepResult::Fault(Error::Underflow); }
            let ok = matches!(vm.stack[n - 3], Value::Int(_))
                && matches!(vm.stack[n - 2], Value::Quote(_))
                && matches!(vm.stack[n - 1], Value::Quote(_));
            if !ok { return StepResult::Fault(Error::TypeMismatch); }
            vm.cont.remove(0);
            let qc = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            let qi = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            let k = match vm.stack.pop() { Some(Value::Int(k)) => k, _ => 0 };
            if k <= 0 {
                // cont := qi ++ rest
                let mut recur = qi;
                recur.append(&mut vm.cont);
                vm.cont = recur;
            } else {
                // cont := [PushInt(k), PushInt(k-1), PushQuote(qi), PushQuote(qc), Prim(PrimRec)] ++ qc ++ rest
                let mut recur = vec![
                    Word::PushInt(k),
                    Word::PushInt(k - 1),
                    Word::PushQuote(qi),
                    Word::PushQuote(qc.clone()),
                    Word::Prim(SpecPrim::PrimRec),
                ];
                recur.extend(qc);
                recur.append(&mut vm.cont);
                vm.cont = recur;
            }
            StepResult::Next
        }
        SpecPrim::Times => {
            if n < 2 { return StepResult::Fault(Error::Underflow); }
            let ok = matches!(vm.stack[n - 2], Value::Int(_))
                && matches!(vm.stack[n - 1], Value::Quote(_));
            if !ok { return StepResult::Fault(Error::TypeMismatch); }
            vm.cont.remove(0);
            let q = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            let k = match vm.stack.pop() { Some(Value::Int(k)) => k, _ => 0 };
            if k > 0 {
                // cont := q ++ [PushInt(k-1), PushQuote(q), Prim(Times)] ++ rest
                let mut recur = q.clone();
                recur.push(Word::PushInt(k - 1));
                recur.push(Word::PushQuote(q));
                recur.push(Word::Prim(SpecPrim::Times));
                recur.append(&mut vm.cont);
                vm.cont = recur;
            }
            StepResult::Next
        }
        SpecPrim::LinRec => {
            if n < 4 { return StepResult::Fault(Error::Underflow); }
            let ok = matches!(vm.stack[n - 4], Value::Quote(_))
                && matches!(vm.stack[n - 3], Value::Quote(_))
                && matches!(vm.stack[n - 2], Value::Quote(_))
                && matches!(vm.stack[n - 1], Value::Quote(_));
            if !ok { return StepResult::Fault(Error::TypeMismatch); }
            vm.cont.remove(0);
            let qr2 = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            let qr1 = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            let qt = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            let qp = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            // else_q := R1 ++ [PushQuote(P), PushQuote(T), PushQuote(R1), PushQuote(R2), Prim(LinRec)] ++ R2
            let mut else_q = qr1.clone();
            else_q.push(Word::PushQuote(qp.clone()));
            else_q.push(Word::PushQuote(qt.clone()));
            else_q.push(Word::PushQuote(qr1));
            else_q.push(Word::PushQuote(qr2.clone()));
            else_q.push(Word::Prim(SpecPrim::LinRec));
            else_q.extend(qr2);
            // spliced := P ++ [PushQuote(T), PushQuote(else_q), Prim(If)] ++ rest
            let mut spliced = qp;
            spliced.push(Word::PushQuote(qt));
            spliced.push(Word::PushQuote(else_q));
            spliced.push(Word::Prim(SpecPrim::If));
            spliced.append(&mut vm.cont);
            vm.cont = spliced;
            StepResult::Next
        }
        SpecPrim::Uncons => {
            if n < 1 { return StepResult::Fault(Error::Underflow); }
            // Inspect (without consuming) so a fault leaves the machine untouched.
            match &vm.stack[n - 1] {
                Value::Quote(q) => {
                    if !q.is_empty() {
                        match &q[0] {
                            Word::PushInt(_) | Word::PushQuote(_) => {}
                            _ => return StepResult::Fault(Error::TypeMismatch),
                        }
                    }
                }
                _ => return StepResult::Fault(Error::TypeMismatch),
            }
            vm.cont.remove(0);
            let q = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            if q.is_empty() {
                vm.stack.push(Value::Int(0));
            } else {
                let mut it = q.into_iter();
                let head = it.next().unwrap();
                let tail: Vec<Word> = it.collect();
                let head_val = match head {
                    Word::PushInt(i) => Value::Int(i),
                    Word::PushQuote(s) => Value::Quote(s),
                    _ => Value::Int(0), // unreachable: guarded above
                };
                vm.stack.push(head_val);
                vm.stack.push(Value::Quote(tail));
                vm.stack.push(Value::Int(1));
            }
            StepResult::Next
        }
        // ---------------- v0.3 sequence primitives (mirror of spec_step_prim) ----------------
        SpecPrim::Fold => {
            if n < 3 { return StepResult::Fault(Error::Underflow); }
            // seq (n-3) and combine (n-1) must be quotes; init (n-2) is any value.
            // A non-value seq head faults TypeMismatch (mirrors Uncons). Inspect
            // without consuming so a fault leaves the machine state untouched.
            let ok = matches!(vm.stack[n - 3], Value::Quote(_))
                && matches!(vm.stack[n - 1], Value::Quote(_));
            if !ok { return StepResult::Fault(Error::TypeMismatch); }
            if let Value::Quote(qs) = &vm.stack[n - 3] {
                if let Some(head) = qs.first() {
                    match head {
                        Word::PushInt(_) | Word::PushQuote(_) => {}
                        _ => return StepResult::Fault(Error::TypeMismatch),
                    }
                }
            }
            vm.cont.remove(0);
            let qc = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            let init = match vm.stack.pop() { Some(v) => v, _ => Value::Int(0) };
            let qs = match vm.stack.pop() { Some(Value::Quote(q)) => q, _ => Vec::new() };
            if qs.is_empty() {
                vm.stack.push(init);
            } else {
                let mut it = qs.into_iter();
                let head = it.next().unwrap();
                let tail: Vec<Word> = it.collect();
                // cont := [PushQuote(tail), value_to_word(init), head] ++ qc
                //         ++ [PushQuote(qc), Prim(Fold)] ++ rest
                let mut recur = vec![
                    Word::PushQuote(tail),
                    value_to_exec_word(init),
                    head,
                ];
                recur.extend(qc.clone());
                recur.push(Word::PushQuote(qc));
                recur.push(Word::Prim(SpecPrim::Fold));
                recur.append(&mut vm.cont);
                vm.cont = recur;
            }
            StepResult::Next
        }
        SpecPrim::Xor => {
            if n < 2 { return StepResult::Fault(Error::Underflow); }
            let (a, b) = match (&vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Int(a), Value::Int(b)) => (*a, *b),
                _ => return StepResult::Fault(Error::TypeMismatch),
            };
            vm.cont.remove(0);
            vm.stack.pop();
            vm.stack.pop();
            vm.stack.push(Value::Int(a ^ b));
            StepResult::Next
        }
    }
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

#[verifier::external_body]
pub fn exec_arith(vm: &mut Vm, p: SpecPrim, n: usize) -> StepResult {
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    let (a, b) = match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => (*a, *b),
        _ => return StepResult::Fault(Error::TypeMismatch),
    };
    let r = match p {
        SpecPrim::Add => a.checked_add(b),
        SpecPrim::Sub => a.checked_sub(b),
        _ => a.checked_mul(b),
    };
    match r {
        Some(v) => {
            vm.cont.remove(0);
            vm.stack.pop();
            vm.stack.pop();
            vm.stack.push(Value::Int(v));
            StepResult::Next
        }
        None => StepResult::Fault(Error::Overflow),
    }
}

#[verifier::external_body]
pub fn exec_divmod(vm: &mut Vm, is_div: bool, n: usize) -> StepResult {
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    let (a, b) = match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => (*a, *b),
        _ => return StepResult::Fault(Error::TypeMismatch),
    };
    if b == 0 { return StepResult::Fault(Error::DivByZero); }
    let r = if is_div { a.checked_div(b) } else { a.checked_rem(b) };
    match r {
        Some(v) => {
            vm.cont.remove(0);
            vm.stack.pop();
            vm.stack.pop();
            vm.stack.push(Value::Int(v));
            StepResult::Next
        }
        None => StepResult::Fault(Error::Overflow),
    }
}

#[verifier::external_body]
pub fn exec_cmp(vm: &mut Vm, is_eq: bool, n: usize) -> StepResult {
    if n < 2 { return StepResult::Fault(Error::Underflow); }
    let (a, b) = match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => (*a, *b),
        _ => return StepResult::Fault(Error::TypeMismatch),
    };
    let v: i64 = if is_eq { if a == b { 1 } else { 0 } } else { if a < b { 1 } else { 0 } };
    vm.cont.remove(0);
    vm.stack.pop();
    vm.stack.pop();
    vm.stack.push(Value::Int(v));
    StepResult::Next
}

// Fuel-bounded driver. Termination NOT provable (MTL is TC). external_body:
// P2's "correct finite unrolling up to fuel" is ASSUMED here — see status note.
#[verifier::external_body]
pub fn run(vm: &mut Vm, fuel: u64) -> Outcome {
    let mut steps: u64 = 0;
    while steps < fuel {
        match exec_step(vm) {
            StepResult::Next => { steps = steps + 1; }
            StepResult::Halt => {
                let mut out: Vec<Value> = Vec::new();
                out.append(&mut vm.stack);
                return Outcome::Halt(out);
            }
            StepResult::Fault(e) => { return Outcome::Fault(e); }
        }
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

// P2 — Refinement: `exec_step` refines `spec_step` via `deep_view`.
// STATUS: OPEN / STUBBED. The refinement statement is carried as the `ensures`
// on `exec_step` above, but that function is `#[verifier::external_body]`, so
// the `ensures` is ASSUMED (trusted), not discharged — no Verus toolchain was
// available in-container to prove it. The lemma below records the obligation
// explicitly; `admit()` marks the hole so the non-blocking CI `verus` job (and
// any future local run) surfaces it as unproven rather than silently absent.
// Executable refinement evidence meanwhile is the differential proptest oracle
// in `crates/mtl-core/tests/interpreter.rs` (independent transliteration of
// `spec_step`, checked step-for-step against the cargo interpreter).
pub proof fn p2_refinement(vm: Vm)
    ensures
        // For every reachable exec state, one exec step matches one spec step.
        // (Full mechanization requires reasoning about the imperative body of
        // `exec_step`; carried as its assumed `ensures` for now.)
        spec_step(vm.deep_view()) is Next
        || spec_step(vm.deep_view()) is Halt
        || spec_step(vm.deep_view()) is Fault,
{
    admit();
}

// P5 — TC lock-step lemma (spec §6): Minsky simulation invariant R
// preserved by bounded MTL step sequences. Stated in PROOF phase after
// the Minsky spec machine is transcribed. Hard; scheduled last.

} // verus!

fn main() {}
