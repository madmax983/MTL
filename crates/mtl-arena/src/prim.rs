//! `exec_prim`: the 23 primitives, with EXACT `interp.rs` semantics.
//!
//! Fault-check order mirrors `interp.rs` / `spec_step` byte-for-byte:
//!   * arity (`Underflow`) is always checked before type (`TypeMismatch`) — the
//!     `popN` helpers return `None` on underflow, so that arm runs first;
//!   * for `div`/`mod`, `DivByZero` is checked before `Overflow`, both inside the
//!     both-operands-`Int` arm (so a type mismatch outranks them);
//!   * for `add`/`sub`/`mul`, `Overflow` is checked after the type match.
//!
//! On any fault the stack is left UNCHANGED: the `popN` helpers are non-mutating
//! (they only read `parent` links), and `st.stack` is reassigned only on the
//! success arm.
//!
//! u32 address-space overflow while interning a fresh segment (`try_alloc` /
//! `try_cat` / `try_cons` / `try_linrec_else` returning `None`) surfaces as a
//! clean `Fault::Overflow` — never a silent wraparound (design §3.4). These arms
//! are unreachable for any realistic corpus (>4.29e9 tape words).

use crate::types::{value_to_word, Fault, Prim, QuoteId, Value, Word};
use crate::arena::VmState;
use crate::vm::{StepR, Vm};

/// A `None` from an allocation helper (u32 tape overflow) → `Fault::Overflow`.
macro_rules! alloc_or_overflow {
    ($e:expr) => {
        match $e {
            Some(id) => id,
            None => return StepR::Fault(Fault::Overflow),
        }
    };
}

impl Vm {
    pub(crate) fn exec_prim(&mut self, st: &mut VmState, p: Prim) -> StepR {
        match p {
            // ------------------------------------------ stack shuffling
            Prim::Dup => {
                let Some((top, _)) = self.pop1(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                st.stack = self.push(st.stack, top);
                StepR::Next
            }
            Prim::Drop => {
                let Some((_, rest)) = self.pop1(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                st.stack = rest;
                StepR::Next
            }
            Prim::Swap => {
                let Some((a, b, rest)) = self.pop2(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                let s = self.push(rest, b);
                st.stack = self.push(s, a);
                StepR::Next
            }
            Prim::Rot => {
                // ( a b c -- b c a )
                let Some((a, b, c, rest)) = self.pop3(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                let s = self.push(rest, b);
                let s = self.push(s, c);
                st.stack = self.push(s, a);
                StepR::Next
            }
            Prim::Over => {
                // ( a b -- a b a )
                let Some((a, _b, _rest)) = self.pop2(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                st.stack = self.push(st.stack, a);
                StepR::Next
            }
            // ------------------------------------------ quotation algebra
            Prim::Apply => {
                let Some((top, rest)) = self.pop1(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                match top {
                    Value::Quote(q) => {
                        st.stack = rest;
                        self.prepend(st, q);
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Cat => {
                let Some((a, b, rest)) = self.pop2(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                match (a, b) {
                    (Value::Quote(qa), Value::Quote(qb)) => {
                        let id = alloc_or_overflow!(self.try_cat(qa, qb));
                        st.stack = self.push(rest, Value::Quote(id));
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Cons => {
                // ( v [q] -- [v q] )
                let Some((v, q, rest)) = self.pop2(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                match q {
                    Value::Quote(q) => {
                        let id = alloc_or_overflow!(self.try_cons(value_to_word(v), q));
                        st.stack = self.push(rest, Value::Quote(id));
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Dip => {
                // ( a [q] -- ... a ) : cont := q ++ [Push(a)] ++ rest
                let Some((a, q, rest)) = self.pop2(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                match q {
                    Value::Quote(q) => {
                        st.stack = rest;
                        let seg = alloc_or_overflow!(self.try_alloc(&[value_to_word(a)]));
                        self.prepend(st, seg);
                        self.prepend(st, q);
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            // ------------------------------------------ arithmetic
            Prim::Add => self.arith(st, |a, b| a.checked_add(b)),
            Prim::Sub => self.arith(st, |a, b| a.checked_sub(b)),
            Prim::Mul => self.arith(st, |a, b| a.checked_mul(b)),
            Prim::Div => self.divmod(st, true),
            Prim::Mod => self.divmod(st, false),
            // ------------------------------------------ comparison / xor
            Prim::Eq => self.cmp(st, |a, b| a == b),
            Prim::Lt => self.cmp(st, |a, b| a < b),
            Prim::Xor => {
                let Some((a, b, rest)) = self.pop2(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                match (a, b) {
                    (Value::Int(a), Value::Int(b)) => {
                        st.stack = self.push(rest, Value::Int(a ^ b));
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            // ------------------------------------------ branch
            Prim::If => {
                let Some((cond, t, f, rest)) = self.pop3(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                match (cond, t, f) {
                    (Value::Int(c), Value::Quote(qt), Value::Quote(qf)) => {
                        st.stack = rest;
                        let branch = if c != 0 { qt } else { qf };
                        self.prepend(st, branch);
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            // ------------------------------------------ v0.2 recursion
            Prim::PrimRec => {
                // ( n [I] [C] -- r )
                let Some((n, qi, qc, rest)) = self.pop3(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                match (n, qi, qc) {
                    (Value::Int(k), Value::Quote(qi), Value::Quote(qc)) => {
                        st.stack = rest;
                        if k <= 0 {
                            self.prepend(st, qi);
                        } else {
                            // cont := [k, k-1, [qi], [qc], primrec] ++ qc ++ rest.
                            // Prepend qc by reference (no |C| copy), then the tiny
                            // setup segment — O(1)/level. k >= 1 so k-1 cannot
                            // overflow.
                            let setup = alloc_or_overflow!(self.try_alloc(&[
                                Word::PushInt(k),
                                Word::PushInt(k - 1),
                                Word::PushQuote(qi),
                                Word::PushQuote(qc),
                                Word::Prim(Prim::PrimRec),
                            ]));
                            self.prepend(st, qc);
                            self.prepend(st, setup);
                        }
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Times => {
                // ( n [Q] -- ... )
                let Some((n, q, rest)) = self.pop2(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                match (n, q) {
                    (Value::Int(k), Value::Quote(q)) => {
                        st.stack = rest;
                        if k > 0 {
                            // cont := q ++ [k-1, [q], times] ++ rest. k >= 1 so
                            // k-1 cannot overflow.
                            let setup = alloc_or_overflow!(self.try_alloc(&[
                                Word::PushInt(k - 1),
                                Word::PushQuote(q),
                                Word::Prim(Prim::Times),
                            ]));
                            self.prepend(st, setup);
                            self.prepend(st, q);
                        }
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::LinRec => {
                // ( [P] [T] [R1] [R2] -- ... ) — desugars into If.
                let Some((qp, qt, qr1, qr2, rest)) = self.pop4(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                match (qp, qt, qr1, qr2) {
                    (
                        Value::Quote(qp),
                        Value::Quote(qt),
                        Value::Quote(qr1),
                        Value::Quote(qr2),
                    ) => {
                        st.stack = rest;
                        // else_q := R1 ++ [[P],[T],[R1],[R2],linrec] ++ R2
                        let else_q = alloc_or_overflow!(self.try_linrec_else(qp, qt, qr1, qr2));
                        // spliced := P ++ [[T], [else_q], If] ++ rest
                        let seg = alloc_or_overflow!(self.try_alloc(&[
                            Word::PushQuote(qt),
                            Word::PushQuote(else_q),
                            Word::Prim(Prim::If),
                        ]));
                        self.prepend(st, seg);
                        self.prepend(st, qp);
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Uncons => {
                // ( [w ...] -- w [...] 1 ) | ( [] -- 0 )
                let Some((top, rest)) = self.pop1(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                let q = match top {
                    Value::Quote(q) => q,
                    _ => return StepR::Fault(Fault::TypeMismatch),
                };
                // Inspect head without consuming: a bare Prim/Call head faults, and
                // the stack must be left untouched (so this check precedes any
                // commit).
                if q.len > 0 {
                    match self.word_at(q.start) {
                        Some(Word::PushInt(_)) | Some(Word::PushQuote(_)) => {}
                        _ => return StepR::Fault(Fault::TypeMismatch),
                    }
                }
                st.stack = rest;
                if q.len == 0 {
                    st.stack = self.push(st.stack, Value::Int(0));
                } else {
                    let head_val = match self.word_at(q.start) {
                        Some(Word::PushInt(k)) => Value::Int(k),
                        Some(Word::PushQuote(id)) => Value::Quote(id),
                        _ => return StepR::Fault(Fault::TypeMismatch),
                    };
                    // start < end (len > 0) so start+1 <= end <= u32::MAX; len > 0
                    // so len-1 cannot underflow.
                    let tail = QuoteId { start: q.start + 1, len: q.len - 1 };
                    st.stack = self.push(st.stack, head_val);
                    st.stack = self.push(st.stack, Value::Quote(tail));
                    st.stack = self.push(st.stack, Value::Int(1));
                }
                StepR::Next
            }
            // ------------------------------------------ v0.3 sequence
            Prim::Fold => {
                // ( [seq] init [C] -- r ) LEFT fold.
                let Some((seq, init, combine, rest)) = self.pop3(st.stack) else {
                    return StepR::Fault(Fault::Underflow);
                };
                let (qs, qc) = match (seq, combine) {
                    (Value::Quote(qs), Value::Quote(qc)) => (qs, qc),
                    _ => return StepR::Fault(Fault::TypeMismatch),
                };
                // Inspect seq head without consuming.
                if qs.len > 0 {
                    match self.word_at(qs.start) {
                        Some(Word::PushInt(_)) | Some(Word::PushQuote(_)) => {}
                        _ => return StepR::Fault(Fault::TypeMismatch),
                    }
                }
                st.stack = rest;
                if qs.len == 0 {
                    st.stack = self.push(st.stack, init);
                } else {
                    // cont := [PushQuote(tail), init_word, head] ++ qc
                    //         ++ [PushQuote(qc), Fold] ++ rest
                    let head = match self.word_at(qs.start) {
                        Some(w) => w,
                        None => {
                            debug_assert!(false, "fold head out of bounds");
                            return StepR::Fault(Fault::TypeMismatch);
                        }
                    };
                    let tail = QuoteId { start: qs.start + 1, len: qs.len - 1 };
                    let seg_c =
                        alloc_or_overflow!(self.try_alloc(&[Word::PushQuote(qc), Word::Prim(Prim::Fold)]));
                    let seg_a = alloc_or_overflow!(self.try_alloc(&[
                        Word::PushQuote(tail),
                        value_to_word(init),
                        head,
                    ]));
                    self.prepend(st, seg_c);
                    self.prepend(st, qc);
                    self.prepend(st, seg_a);
                }
                StepR::Next
            }
        }
    }

    #[inline]
    fn arith(&mut self, st: &mut VmState, op: fn(i64, i64) -> Option<i64>) -> StepR {
        let Some((a, b, rest)) = self.pop2(st.stack) else {
            return StepR::Fault(Fault::Underflow);
        };
        match (a, b) {
            (Value::Int(a), Value::Int(b)) => match op(a, b) {
                Some(r) => {
                    st.stack = self.push(rest, Value::Int(r));
                    StepR::Next
                }
                None => StepR::Fault(Fault::Overflow),
            },
            _ => StepR::Fault(Fault::TypeMismatch),
        }
    }

    #[inline]
    fn divmod(&mut self, st: &mut VmState, is_div: bool) -> StepR {
        let Some((a, b, rest)) = self.pop2(st.stack) else {
            return StepR::Fault(Fault::Underflow);
        };
        match (a, b) {
            (Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    return StepR::Fault(Fault::DivByZero);
                }
                let res = if is_div { a.checked_div(b) } else { a.checked_rem(b) };
                match res {
                    Some(r) => {
                        st.stack = self.push(rest, Value::Int(r));
                        StepR::Next
                    }
                    None => StepR::Fault(Fault::Overflow),
                }
            }
            _ => StepR::Fault(Fault::TypeMismatch),
        }
    }

    #[inline]
    fn cmp(&mut self, st: &mut VmState, op: fn(i64, i64) -> bool) -> StepR {
        let Some((a, b, rest)) = self.pop2(st.stack) else {
            return StepR::Fault(Fault::Underflow);
        };
        match (a, b) {
            (Value::Int(a), Value::Int(b)) => {
                let r = if op(a, b) { 1 } else { 0 };
                st.stack = self.push(rest, Value::Int(r));
                StepR::Next
            }
            _ => StepR::Fault(Fault::TypeMismatch),
        }
    }
}
