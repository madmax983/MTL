//! MTL interpreter test suite (Track B).
//!
//! Layers (spec §7.4 / §13 test plan):
//!   * `oracle`      — an independent, obviously-correct Rust transliteration of
//!                     `spec_step` (i128 arithmetic + range check). NOT the Verus
//!                     ghost model; a second implementation used as a differential
//!                     reference.
//!   * `golden`      — every spec example, incl. the `: !` two-step self-application.
//!   * `boundary`    — i64 overflow, truncating div/mod on negatives + i64::MIN
//!                     edges, empty-stack underflow for each arity.
//!   * `precedence`  — explicit fault-precedence documentation tests.
//!   * `differential`— proptest: random Vm states + programs; assert `exec_step`
//!                     agrees with the oracle step-for-step, and `run` agrees on
//!                     terminal outcomes.
//!
//! Programs are built as ASTs directly (no dependency on a parser — Track A).

use mtl_core::interp::build::*;
use mtl_core::interp::{exec_step, run, Fault, Outcome, Prim, Step, Value, Vm, Word};

// ============================================================
// Reference oracle — independent transliteration of spec_step.
// ============================================================
mod oracle {
    use super::*;

    /// Outcome of one reference step.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum RefStep {
        Next { stack: Vec<Value>, cont: Vec<Word> },
        Halt(Vec<Value>),
        Fault(Fault),
    }

    fn in_i64(n: i128) -> bool {
        n >= i64::MIN as i128 && n <= i64::MAX as i128
    }

    fn value_to_word(v: &Value) -> Word {
        match v {
            Value::Int(i) => Word::PushInt(*i),
            Value::Quote(q) => Word::PushQuote(q.clone()),
        }
    }

    /// One reference small step. Byte-for-byte mirror of `spec_step` /
    /// `spec_step_prim` in `mtl_core.rs`, computed independently (i128 math).
    pub fn ref_step(stack: &[Value], cont: &[Word]) -> RefStep {
        if cont.is_empty() {
            return RefStep::Halt(stack.to_vec());
        }
        let rest: Vec<Word> = cont[1..].to_vec();
        match &cont[0] {
            Word::PushInt(n) => {
                let mut s = stack.to_vec();
                s.push(Value::Int(*n));
                RefStep::Next { stack: s, cont: rest }
            }
            Word::PushQuote(q) => {
                let mut s = stack.to_vec();
                s.push(Value::Quote(q.clone()));
                RefStep::Next { stack: s, cont: rest }
            }
            Word::Call(_) => RefStep::Fault(Fault::UnknownWord),
            Word::Prim(p) => ref_prim(stack, *p, rest),
        }
    }

    fn next(stack: Vec<Value>, cont: Vec<Word>) -> RefStep {
        RefStep::Next { stack, cont }
    }

    fn ref_prim(stack: &[Value], p: Prim, rest: Vec<Word>) -> RefStep {
        let n = stack.len();
        let s = stack; // read-only view
        match p {
            Prim::Dup => {
                if n < 1 {
                    return RefStep::Fault(Fault::Underflow);
                }
                let mut ns = s.to_vec();
                ns.push(s[n - 1].clone());
                next(ns, rest)
            }
            Prim::Drop => {
                if n < 1 {
                    return RefStep::Fault(Fault::Underflow);
                }
                next(s[..n - 1].to_vec(), rest)
            }
            Prim::Swap => {
                if n < 2 {
                    return RefStep::Fault(Fault::Underflow);
                }
                let mut ns = s[..n - 2].to_vec();
                ns.push(s[n - 1].clone());
                ns.push(s[n - 2].clone());
                next(ns, rest)
            }
            Prim::Rot => {
                // ( a b c -- b c a )
                if n < 3 {
                    return RefStep::Fault(Fault::Underflow);
                }
                let mut ns = s[..n - 3].to_vec();
                ns.push(s[n - 2].clone());
                ns.push(s[n - 1].clone());
                ns.push(s[n - 3].clone());
                next(ns, rest)
            }
            Prim::Over => {
                // ( a b -- a b a )
                if n < 2 {
                    return RefStep::Fault(Fault::Underflow);
                }
                let mut ns = s.to_vec();
                ns.push(s[n - 2].clone());
                next(ns, rest)
            }
            Prim::Apply => {
                if n < 1 {
                    return RefStep::Fault(Fault::Underflow);
                }
                match &s[n - 1] {
                    Value::Quote(q) => {
                        let mut cont = q.clone();
                        cont.extend(rest);
                        next(s[..n - 1].to_vec(), cont)
                    }
                    _ => RefStep::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Cat => {
                if n < 2 {
                    return RefStep::Fault(Fault::Underflow);
                }
                match (&s[n - 2], &s[n - 1]) {
                    (Value::Quote(a), Value::Quote(b)) => {
                        let mut ab = a.clone();
                        ab.extend(b.clone());
                        let mut ns = s[..n - 2].to_vec();
                        ns.push(Value::Quote(ab));
                        next(ns, rest)
                    }
                    _ => RefStep::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Cons => {
                // ( v [q] -- [v q] )
                if n < 2 {
                    return RefStep::Fault(Fault::Underflow);
                }
                match &s[n - 1] {
                    Value::Quote(q) => {
                        let mut newq = vec![value_to_word(&s[n - 2])];
                        newq.extend(q.clone());
                        let mut ns = s[..n - 2].to_vec();
                        ns.push(Value::Quote(newq));
                        next(ns, rest)
                    }
                    _ => RefStep::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Dip => {
                // ( a [q] -- ... a ) : cont := q ++ [Push(a)] ++ rest
                if n < 2 {
                    return RefStep::Fault(Fault::Underflow);
                }
                match &s[n - 1] {
                    Value::Quote(q) => {
                        let mut cont = q.clone();
                        cont.push(value_to_word(&s[n - 2]));
                        cont.extend(rest);
                        next(s[..n - 2].to_vec(), cont)
                    }
                    _ => RefStep::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Add => ref_arith(s, rest, |a, b| a + b),
            Prim::Sub => ref_arith(s, rest, |a, b| a - b),
            Prim::Mul => ref_arith(s, rest, |a, b| a * b),
            Prim::Div => ref_divmod(s, rest, true),
            Prim::Mod => ref_divmod(s, rest, false),
            Prim::Eq => ref_cmp(s, rest, |a, b| a == b),
            Prim::Lt => ref_cmp(s, rest, |a, b| a < b),
            Prim::If => {
                // ( c [t] [f] -- ... )
                if n < 3 {
                    return RefStep::Fault(Fault::Underflow);
                }
                match (&s[n - 3], &s[n - 2], &s[n - 1]) {
                    (Value::Int(c), Value::Quote(t), Value::Quote(f)) => {
                        let branch = if *c != 0 { t } else { f };
                        let mut cont = branch.clone();
                        cont.extend(rest);
                        next(s[..n - 3].to_vec(), cont)
                    }
                    _ => RefStep::Fault(Fault::TypeMismatch),
                }
            }
            // ---------------- v0.2 recursion primitives ----------------
            Prim::PrimRec => {
                // ( n [I] [C] -- r )
                if n < 3 {
                    return RefStep::Fault(Fault::Underflow);
                }
                match (&s[n - 3], &s[n - 2], &s[n - 1]) {
                    (Value::Int(k), Value::Quote(qi), Value::Quote(qc)) => {
                        let (k, qi, qc) = (*k, qi.clone(), qc.clone());
                        let base = s[..n - 3].to_vec();
                        if k <= 0 {
                            let mut cont = qi;
                            cont.extend(rest);
                            next(base, cont)
                        } else {
                            let mut cont = vec![
                                Word::PushInt(k),
                                Word::PushInt(k - 1),
                                Word::PushQuote(qi),
                                Word::PushQuote(qc.clone()),
                                Word::Prim(Prim::PrimRec),
                            ];
                            cont.extend(qc);
                            cont.extend(rest);
                            next(base, cont)
                        }
                    }
                    _ => RefStep::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Times => {
                // ( n [Q] -- ... )
                if n < 2 {
                    return RefStep::Fault(Fault::Underflow);
                }
                match (&s[n - 2], &s[n - 1]) {
                    (Value::Int(k), Value::Quote(q)) => {
                        let (k, q) = (*k, q.clone());
                        let base = s[..n - 2].to_vec();
                        if k <= 0 {
                            next(base, rest)
                        } else {
                            let mut cont = q.clone();
                            cont.push(Word::PushInt(k - 1));
                            cont.push(Word::PushQuote(q));
                            cont.push(Word::Prim(Prim::Times));
                            cont.extend(rest);
                            next(base, cont)
                        }
                    }
                    _ => RefStep::Fault(Fault::TypeMismatch),
                }
            }
            Prim::LinRec => {
                // ( [P] [T] [R1] [R2] -- ... ) — desugars into If.
                if n < 4 {
                    return RefStep::Fault(Fault::Underflow);
                }
                match (&s[n - 4], &s[n - 3], &s[n - 2], &s[n - 1]) {
                    (Value::Quote(qp), Value::Quote(qt), Value::Quote(qr1), Value::Quote(qr2)) => {
                        let (qp, qt, qr1, qr2) =
                            (qp.clone(), qt.clone(), qr1.clone(), qr2.clone());
                        let base = s[..n - 4].to_vec();
                        let mut else_q = qr1.clone();
                        else_q.push(Word::PushQuote(qp.clone()));
                        else_q.push(Word::PushQuote(qt.clone()));
                        else_q.push(Word::PushQuote(qr1));
                        else_q.push(Word::PushQuote(qr2.clone()));
                        else_q.push(Word::Prim(Prim::LinRec));
                        else_q.extend(qr2);
                        let mut cont = qp;
                        cont.push(Word::PushQuote(qt));
                        cont.push(Word::PushQuote(else_q));
                        cont.push(Word::Prim(Prim::If));
                        cont.extend(rest);
                        next(base, cont)
                    }
                    _ => RefStep::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Uncons => {
                // ( [w ...] -- w [...] 1 ) | ( [] -- 0 )
                if n < 1 {
                    return RefStep::Fault(Fault::Underflow);
                }
                match &s[n - 1] {
                    Value::Quote(q) => {
                        let base = s[..n - 1].to_vec();
                        if q.is_empty() {
                            let mut ns = base;
                            ns.push(Value::Int(0));
                            next(ns, rest)
                        } else {
                            let tail = q[1..].to_vec();
                            match &q[0] {
                                Word::PushInt(i) => {
                                    let mut ns = base;
                                    ns.push(Value::Int(*i));
                                    ns.push(Value::Quote(tail));
                                    ns.push(Value::Int(1));
                                    next(ns, rest)
                                }
                                Word::PushQuote(sq) => {
                                    let mut ns = base;
                                    ns.push(Value::Quote(sq.clone()));
                                    ns.push(Value::Quote(tail));
                                    ns.push(Value::Int(1));
                                    next(ns, rest)
                                }
                                _ => RefStep::Fault(Fault::TypeMismatch),
                            }
                        }
                    }
                    _ => RefStep::Fault(Fault::TypeMismatch),
                }
            }
        }
    }

    fn ref_arith(s: &[Value], rest: Vec<Word>, op: fn(i128, i128) -> i128) -> RefStep {
        let n = s.len();
        if n < 2 {
            return RefStep::Fault(Fault::Underflow);
        }
        match (&s[n - 2], &s[n - 1]) {
            (Value::Int(a), Value::Int(b)) => {
                let r = op(*a as i128, *b as i128);
                if in_i64(r) {
                    let mut ns = s[..n - 2].to_vec();
                    ns.push(Value::Int(r as i64));
                    next(ns, rest)
                } else {
                    RefStep::Fault(Fault::Overflow)
                }
            }
            _ => RefStep::Fault(Fault::TypeMismatch),
        }
    }

    fn ref_divmod(s: &[Value], rest: Vec<Word>, is_div: bool) -> RefStep {
        let n = s.len();
        if n < 2 {
            return RefStep::Fault(Fault::Underflow);
        }
        match (&s[n - 2], &s[n - 1]) {
            (Value::Int(a), Value::Int(b)) => {
                let (a, b) = (*a as i128, *b as i128);
                if b == 0 {
                    return RefStep::Fault(Fault::DivByZero);
                }
                // i128 `/` and `%` truncate toward zero (Rust semantics).
                let q = a / b;
                if !in_i64(q) {
                    // Only i64::MIN / -1; faults for BOTH div and mod.
                    return RefStep::Fault(Fault::Overflow);
                }
                let r = if is_div { q } else { a % b };
                let mut ns = s[..n - 2].to_vec();
                ns.push(Value::Int(r as i64));
                next(ns, rest)
            }
            _ => RefStep::Fault(Fault::TypeMismatch),
        }
    }

    fn ref_cmp(s: &[Value], rest: Vec<Word>, op: fn(i64, i64) -> bool) -> RefStep {
        let n = s.len();
        if n < 2 {
            return RefStep::Fault(Fault::Underflow);
        }
        match (&s[n - 2], &s[n - 1]) {
            (Value::Int(a), Value::Int(b)) => {
                let r = if op(*a, *b) { 1 } else { 0 };
                let mut ns = s[..n - 2].to_vec();
                ns.push(Value::Int(r));
                next(ns, rest)
            }
            _ => RefStep::Fault(Fault::TypeMismatch),
        }
    }

    /// Reference driver mirroring `run`.
    pub fn ref_run(mut stack: Vec<Value>, mut cont: Vec<Word>, fuel: u64) -> Outcome {
        let mut steps = 0u64;
        loop {
            if steps >= fuel {
                return Outcome::FuelExhausted { stack, cont };
            }
            match ref_step(&stack, &cont) {
                RefStep::Next { stack: ns, cont: nc } => {
                    stack = ns;
                    cont = nc;
                    steps += 1;
                }
                RefStep::Halt(s) => return Outcome::Halt(s),
                RefStep::Fault(f) => {
                    return Outcome::Fault(mtl_core::interp::FaultInfo {
                        fault: f,
                        stack,
                        cont,
                    })
                }
            }
        }
    }
}

// ---- small helpers ------------------------------------------------------

/// Run one exec step from an explicit (stack, cont); return (result, new vm).
fn exec_once(stack: Vec<Value>, cont: Vec<Word>) -> (Step, Vm) {
    let mut vm = Vm::with_stack(stack, cont);
    let r = exec_step(&mut vm);
    (r, vm)
}

fn i(n: i64) -> Value {
    Value::Int(n)
}
fn q(ws: Vec<Word>) -> Value {
    Value::Quote(ws)
}

const FUEL: u64 = 10_000;

fn run_prog(stack: Vec<Value>, prog: Vec<Word>) -> Outcome {
    run(Vm::with_stack(stack, prog), FUEL)
}

fn expect_halt(stack: Vec<Value>, prog: Vec<Word>) -> Vec<Value> {
    match run_prog(stack, prog) {
        Outcome::Halt(s) => s,
        other => panic!("expected Halt, got {:?}", other),
    }
}

fn expect_fault(stack: Vec<Value>, prog: Vec<Word>) -> mtl_core::interp::FaultInfo {
    match run_prog(stack, prog) {
        Outcome::Fault(fi) => fi,
        other => panic!("expected Fault, got {:?}", other),
    }
}

// ============================================================
// GOLDEN — every spec example.
// ============================================================
mod golden {
    use super::*;

    #[test]
    fn empty_program_halts_with_input_stack() {
        assert_eq!(expect_halt(vec![i(1), i(2)], vec![]), vec![i(1), i(2)]);
    }

    #[test]
    fn push_int_and_quote() {
        assert_eq!(
            expect_halt(vec![], vec![int(7), quote(vec![int(1)])]),
            vec![i(7), q(vec![int(1)])]
        );
    }

    // ---- stack shuffling (spec §4.1) ----
    #[test]
    fn dup_op() {
        assert_eq!(expect_halt(vec![i(5)], vec![dup()]), vec![i(5), i(5)]);
    }
    #[test]
    fn drop_() {
        assert_eq!(expect_halt(vec![i(5), i(9)], vec![drop()]), vec![i(5)]);
    }
    #[test]
    fn swap_op() {
        assert_eq!(expect_halt(vec![i(1), i(2)], vec![swap()]), vec![i(2), i(1)]);
    }
    #[test]
    fn rot_op() {
        // ( a b c -- b c a )
        assert_eq!(
            expect_halt(vec![i(1), i(2), i(3)], vec![rot()]),
            vec![i(2), i(3), i(1)]
        );
    }
    #[test]
    fn over_op() {
        // ( a b -- a b a )
        assert_eq!(
            expect_halt(vec![i(1), i(2)], vec![over()]),
            vec![i(1), i(2), i(1)]
        );
    }

    // ---- quotation algebra ----
    #[test]
    fn apply_splices() {
        // 3 [4 +] ! -> 7
        assert_eq!(
            expect_halt(vec![], vec![int(3), quote(vec![int(4), add()]), apply()]),
            vec![i(7)]
        );
    }
    #[test]
    fn cat_op() {
        // [1] [2] , -> [1 2]  ; then apply to observe 1 2
        assert_eq!(
            expect_halt(
                vec![],
                vec![quote(vec![int(1)]), quote(vec![int(2)]), cat(), apply()]
            ),
            vec![i(1), i(2)]
        );
    }
    #[test]
    fn cons_op() {
        // 5 [+] ; -> [5 +]  ; run on 10 -> 15
        assert_eq!(
            expect_halt(
                vec![i(10)],
                vec![int(5), quote(vec![add()]), cons(), apply()]
            ),
            vec![i(15)]
        );
    }
    #[test]
    fn dip_op() {
        // ( a [q] -- ... a ): [10 20] [5 +] ' -> [15 20]
        assert_eq!(
            expect_halt(
                vec![i(10), i(20)],
                vec![quote(vec![int(5), add()]), dip()]
            ),
            vec![i(15), i(20)]
        );
    }

    // ---- arithmetic ----
    #[test]
    fn add_sub_mul() {
        assert_eq!(expect_halt(vec![i(3), i(4)], vec![add()]), vec![i(7)]);
        assert_eq!(expect_halt(vec![i(10), i(3)], vec![sub()]), vec![i(7)]);
        assert_eq!(expect_halt(vec![i(3), i(4)], vec![mul()]), vec![i(12)]);
    }
    #[test]
    fn div_mod_truncate_toward_zero() {
        assert_eq!(expect_halt(vec![i(7), i(2)], vec![div()]), vec![i(3)]);
        assert_eq!(expect_halt(vec![i(-7), i(2)], vec![div()]), vec![i(-3)]); // not -4
        assert_eq!(expect_halt(vec![i(-7), i(2)], vec![modulo()]), vec![i(-1)]); // sign of dividend
        assert_eq!(expect_halt(vec![i(7), i(-2)], vec![div()]), vec![i(-3)]);
        assert_eq!(expect_halt(vec![i(7), i(-2)], vec![modulo()]), vec![i(1)]);
        assert_eq!(expect_halt(vec![i(-7), i(-2)], vec![div()]), vec![i(3)]);
        assert_eq!(expect_halt(vec![i(-7), i(-2)], vec![modulo()]), vec![i(-1)]);
    }

    // ---- comparison ----
    #[test]
    fn eq_lt() {
        assert_eq!(expect_halt(vec![i(3), i(3)], vec![eq()]), vec![i(1)]);
        assert_eq!(expect_halt(vec![i(3), i(4)], vec![eq()]), vec![i(0)]);
        assert_eq!(expect_halt(vec![i(3), i(4)], vec![lt()]), vec![i(1)]);
        assert_eq!(expect_halt(vec![i(4), i(3)], vec![lt()]), vec![i(0)]);
    }

    // ---- branch ----
    #[test]
    fn if_true_false() {
        // c [t] [f] ?
        assert_eq!(
            expect_halt(
                vec![i(1)],
                vec![quote(vec![int(10)]), quote(vec![int(20)]), iff()]
            ),
            vec![i(10)]
        );
        assert_eq!(
            expect_halt(
                vec![i(0)],
                vec![quote(vec![int(10)]), quote(vec![int(20)]), iff()]
            ),
            vec![i(20)]
        );
    }

    // ---- the `: !` two-step self-application (spec §6.2, smoke_dup_apply) ----
    #[test]
    fn dup_apply_two_step_self_application() {
        // From [Quote(qbody)] with cont [dup, apply], two steps reach
        // stack [Quote(qbody)] with cont == qbody: the quote is retained while
        // its body splices into the continuation.
        let qbody = vec![int(42)];
        let start_stack = vec![q(qbody.clone())];
        let start_cont = vec![dup(), apply()];

        // step 1: dup
        let (r1, vm1) = exec_once(start_stack.clone(), start_cont.clone());
        assert_eq!(r1, Step::Next);
        assert_eq!(vm1.stack, vec![q(qbody.clone()), q(qbody.clone())]);
        assert_eq!(vm1.cont, vec![apply()]);

        // step 2: apply -> quote retained, body spliced
        let (r2, vm2) = exec_once(vm1.stack, vm1.cont);
        assert_eq!(r2, Step::Next);
        assert_eq!(vm2.stack, vec![q(qbody.clone())]);
        assert_eq!(vm2.cont, qbody); // cont == q, exactly (splice, no growth)
    }

    // ---- a real self-applying loop using `: !` (countdown to zero) ----
    // L = [ swap dup 0 = [swap drop] [1 - swap dup !] ? ]
    // program(c) = c [L] : !   ; expected final stack [0]
    fn countdown_program(c: i64) -> Vec<Word> {
        let l = vec![
            swap(),
            dup(),
            int(0),
            eq(),
            quote(vec![swap(), drop()]),                       // then: c==0
            quote(vec![int(1), sub(), swap(), dup(), apply()]), // else: recurse
            iff(),
        ];
        vec![int(c), quote(l), dup(), apply()]
    }

    #[test]
    fn self_applying_countdown_loop() {
        assert_eq!(expect_halt(vec![], countdown_program(0)), vec![i(0)]);
        assert_eq!(expect_halt(vec![], countdown_program(1)), vec![i(0)]);
        assert_eq!(expect_halt(vec![], countdown_program(5)), vec![i(0)]);
        assert_eq!(expect_halt(vec![], countdown_program(100)), vec![i(0)]);
    }

    #[test]
    fn loop_fuel_exhaustion_is_resumable() {
        // With too little fuel the loop reports FuelExhausted, carrying live state.
        match run(Vm::with_stack(vec![], countdown_program(1000)), 5) {
            Outcome::FuelExhausted { cont, .. } => assert!(!cont.is_empty()),
            other => panic!("expected FuelExhausted, got {:?}", other),
        }
    }
}

// ============================================================
// RECURSION — v0.2 primitives: primrec, times, linrec, uncons.
// Includes the design doc's worked example programs (§7):
//   factorial-via-primrec, gcd-via-linrec, fib-via-times.
// ============================================================
mod recursion {
    use super::*;

    // ---- primrec ( n [I] [C] -- r ) ----

    // factorial `[1][*]&`: base I=[1], combine C=[*] (design §7).
    fn factorial(n: i64) -> Vec<Value> {
        expect_halt(
            vec![i(n)],
            vec![quote(vec![int(1)]), quote(vec![mul()]), primrec()],
        )
    }

    #[test]
    fn factorial_via_primrec() {
        assert_eq!(factorial(0), vec![i(1)]);
        assert_eq!(factorial(1), vec![i(1)]);
        assert_eq!(factorial(2), vec![i(2)]);
        assert_eq!(factorial(3), vec![i(6)]);
        assert_eq!(factorial(4), vec![i(24)]);
        assert_eq!(factorial(5), vec![i(120)]);
    }

    // sum_to `[0][+]&`: base I=[0], combine C=[+] (design §7).
    fn sum_to(n: i64) -> Vec<Value> {
        expect_halt(
            vec![i(n)],
            vec![quote(vec![int(0)]), quote(vec![add()]), primrec()],
        )
    }

    #[test]
    fn sum_to_via_primrec() {
        assert_eq!(sum_to(0), vec![i(0)]);
        assert_eq!(sum_to(1), vec![i(1)]);
        assert_eq!(sum_to(3), vec![i(6)]); // 3+2+1+0
        assert_eq!(sum_to(10), vec![i(55)]);
    }

    #[test]
    fn primrec_nonpositive_count_runs_base() {
        // n<=0 discards the count and runs I on the base stack (total, no fault).
        assert_eq!(
            expect_halt(
                vec![i(0)],
                vec![quote(vec![int(42)]), quote(vec![mul()]), primrec()]
            ),
            vec![i(42)]
        );
        assert_eq!(
            expect_halt(
                vec![i(-5)],
                vec![quote(vec![int(42)]), quote(vec![mul()]), primrec()]
            ),
            vec![i(42)]
        );
        assert_eq!(
            expect_halt(
                vec![i(i64::MIN)],
                vec![quote(vec![int(7)]), quote(vec![mul()]), primrec()]
            ),
            vec![i(7)]
        );
    }

    #[test]
    fn primrec_keeps_count_available_to_combine() {
        // The combine sees (count, subresult). With I=[] and C=[+], the fold is
        // n + (n-1) + ... + 1 + 0 == sum_to(n); confirms n is on the stack for C.
        assert_eq!(
            expect_halt(
                vec![i(4)],
                vec![quote(vec![int(0)]), quote(vec![add()]), primrec()]
            ),
            vec![i(10)]
        );
    }

    // ---- times ( n [Q] -- ... ) ----

    // fib `01@[~^+].` then `_`: seed 0 1, rotate count up, step [swap over add]
    // n times, drop the spare accumulator (design §7.2).
    fn fib(n: i64) -> Vec<Value> {
        expect_halt(
            vec![i(n)],
            vec![
                int(0),
                int(1),
                rot(),
                quote(vec![swap(), over(), add()]),
                times(),
                drop(),
            ],
        )
    }

    #[test]
    fn fib_via_times() {
        assert_eq!(fib(0), vec![i(0)]);
        assert_eq!(fib(1), vec![i(1)]);
        assert_eq!(fib(2), vec![i(1)]);
        assert_eq!(fib(3), vec![i(2)]);
        assert_eq!(fib(5), vec![i(5)]);
        assert_eq!(fib(10), vec![i(55)]);
    }

    #[test]
    fn times_counts_iterations() {
        // Run [1 +] exactly n times over accumulator 0 -> n.
        let prog = |n: i64| vec![int(n), quote(vec![int(1), add()]), times()];
        assert_eq!(expect_halt(vec![i(0)], prog(3)), vec![i(3)]);
        assert_eq!(expect_halt(vec![i(0)], prog(1)), vec![i(1)]);
    }

    #[test]
    fn times_nonpositive_count_is_noop() {
        // n<=0 is a no-op: Q never runs; the count and quote are consumed.
        assert_eq!(
            expect_halt(vec![i(99)], vec![int(0), quote(vec![int(1), add()]), times()]),
            vec![i(99)]
        );
        assert_eq!(
            expect_halt(vec![i(99)], vec![int(-4), quote(vec![int(1), add()]), times()]),
            vec![i(99)]
        );
        assert_eq!(
            expect_halt(
                vec![i(99)],
                vec![int(i64::MIN), quote(vec![int(1), add()]), times()]
            ),
            vec![i(99)]
        );
    }

    // power `1~[^*].~_`: seed acc 1 under (b e), run [over mul] e times (design §7).
    // Here written explicitly: stack (b), acc 1, count e, step [over mul].
    #[test]
    fn power_via_times() {
        // b^e: acc starts 1, each step multiplies acc by b (over then mul).
        // stack layout: [b, 1], then push e, push [over mul], times, then drop b.
        let power = |b: i64, e: i64| {
            expect_halt(
                vec![i(b), i(1)],
                vec![int(e), quote(vec![over(), mul()]), times(), swap(), drop()],
            )
        };
        assert_eq!(power(2, 0), vec![i(1)]);
        assert_eq!(power(2, 3), vec![i(8)]);
        assert_eq!(power(3, 4), vec![i(81)]);
        assert_eq!(power(5, 2), vec![i(25)]);
    }

    // ---- linrec ( [P] [T] [R1] [R2] -- ... ) ----

    // gcd `[:0=][_][~^%][]|`: P tests top==0 nondestructively, T drops the 0,
    // R1 is [swap over mod], R2 is [] (tail recursion) (design §7, §3.3).
    fn gcd(a: i64, b: i64) -> Vec<Value> {
        expect_halt(
            vec![i(a), i(b)],
            vec![
                quote(vec![dup(), int(0), eq()]), // P
                quote(vec![drop()]),              // T
                quote(vec![swap(), over(), modulo()]), // R1
                quote(vec![]),                    // R2 = []
                linrec(),
            ],
        )
    }

    #[test]
    fn gcd_via_linrec() {
        assert_eq!(gcd(12, 8), vec![i(4)]);
        assert_eq!(gcd(48, 36), vec![i(12)]);
        assert_eq!(gcd(17, 5), vec![i(1)]);
        assert_eq!(gcd(100, 0), vec![i(100)]); // base immediately: top is 0
        assert_eq!(gcd(7, 21), vec![i(7)]);
    }

    #[test]
    fn linrec_desugars_through_if() {
        // A linrec with R2=[] and a base that fires immediately observes that the
        // T-branch runs (inherits If's verified branch semantics). P=[1] (always
        // true), T=[drop] -> drops the flag-provider... use a neutral T=[] here.
        // Countdown: P=[:] copies top as flag; when 0, stop; else 1- and recurse.
        // Simpler: immediate base. P pushes nonzero, T is identity, R never runs.
        assert_eq!(
            expect_halt(
                vec![i(5)],
                vec![
                    quote(vec![int(1)]),          // P: push truthy flag
                    quote(vec![]),                // T: identity
                    quote(vec![int(0)]),          // R1 (unused)
                    quote(vec![]),                // R2
                    linrec(),
                ]
            ),
            vec![i(5)]
        );
    }

    // ---- uncons ( [w ...] -- w [...] 1 ) | ( [] -- 0 ) ----

    #[test]
    fn uncons_empty_pushes_flag_zero() {
        assert_eq!(
            expect_halt(vec![], vec![quote(vec![]), uncons()]),
            vec![i(0)]
        );
    }

    #[test]
    fn uncons_int_head() {
        // [7 8 9] > -> 7 [8 9] 1
        assert_eq!(
            expect_halt(
                vec![],
                vec![quote(vec![int(7), int(8), int(9)]), uncons()]
            ),
            vec![i(7), q(vec![int(8), int(9)]), i(1)]
        );
    }

    #[test]
    fn uncons_quote_head() {
        // [[1] 2] > -> [1] [2] 1  (head is itself a quotation value)
        assert_eq!(
            expect_halt(
                vec![],
                vec![quote(vec![quote(vec![int(1)]), int(2)]), uncons()]
            ),
            vec![q(vec![int(1)]), q(vec![int(2)]), i(1)]
        );
    }

    #[test]
    fn uncons_singleton_leaves_empty_tail() {
        // [5] > -> 5 [] 1
        assert_eq!(
            expect_halt(vec![], vec![quote(vec![int(5)]), uncons()]),
            vec![i(5), q(vec![]), i(1)]
        );
    }

    #[test]
    fn uncons_roundtrips_with_cons() {
        // cons then uncons is identity on (v, [q]) up to the flag: 5 [8] ; > -> 5 [8] 1
        assert_eq!(
            expect_halt(
                vec![],
                vec![int(5), quote(vec![int(8)]), cons(), uncons()]
            ),
            vec![i(5), q(vec![int(8)]), i(1)]
        );
    }
}

// ============================================================
// BOUNDARY — overflow, div/mod edges, underflow per arity.
// ============================================================
mod boundary {
    use super::*;

    #[test]
    fn empty_program_empty_stack() {
        assert_eq!(expect_halt(vec![], vec![]), vec![]);
    }

    #[test]
    fn deeply_nested_quotations() {
        // Build [[[[...]]]] 12 deep, push, halt; deep-view / clone must not panic.
        let mut w = quote(vec![]);
        for _ in 0..12 {
            w = quote(vec![w]);
        }
        let out = expect_halt(vec![], vec![w.clone()]);
        assert_eq!(out.len(), 1);
    }

    // ---- checked-arithmetic overflow ----
    #[test]
    fn add_overflow() {
        let fi = expect_fault(vec![i(i64::MAX), i(1)], vec![add()]);
        assert_eq!(fi.fault, Fault::Overflow);
    }
    #[test]
    fn sub_overflow() {
        let fi = expect_fault(vec![i(i64::MIN), i(1)], vec![sub()]);
        assert_eq!(fi.fault, Fault::Overflow);
    }
    #[test]
    fn mul_overflow() {
        let fi = expect_fault(vec![i(i64::MAX), i(2)], vec![mul()]);
        assert_eq!(fi.fault, Fault::Overflow);
    }
    #[test]
    fn add_at_max_boundary_ok() {
        assert_eq!(
            expect_halt(vec![i(i64::MAX - 1), i(1)], vec![add()]),
            vec![i(i64::MAX)]
        );
    }

    // ---- div/mod i64::MIN edges (checked_div/checked_rem None) ----
    #[test]
    fn min_div_neg_one_overflows() {
        let fi = expect_fault(vec![i(i64::MIN), i(-1)], vec![div()]);
        assert_eq!(fi.fault, Fault::Overflow);
    }
    #[test]
    fn min_mod_neg_one_overflows() {
        // Mathematical remainder is 0, but checked_rem(MIN,-1) is None: spec models
        // the exec truth -> Overflow (spec §4.1 div/mod note).
        let fi = expect_fault(vec![i(i64::MIN), i(-1)], vec![modulo()]);
        assert_eq!(fi.fault, Fault::Overflow);
    }
    #[test]
    fn min_div_one_ok() {
        assert_eq!(
            expect_halt(vec![i(i64::MIN), i(1)], vec![div()]),
            vec![i(i64::MIN)]
        );
    }
    #[test]
    fn div_by_zero() {
        let fi = expect_fault(vec![i(5), i(0)], vec![div()]);
        assert_eq!(fi.fault, Fault::DivByZero);
        let fi = expect_fault(vec![i(5), i(0)], vec![modulo()]);
        assert_eq!(fi.fault, Fault::DivByZero);
    }

    // ---- underflow for each arity ----
    #[test]
    fn underflow_arity_1() {
        for p in [Prim::Dup, Prim::Drop, Prim::Apply] {
            let fi = expect_fault(vec![], vec![prim(p)]);
            assert_eq!(fi.fault, Fault::Underflow, "prim {:?}", p);
            // fault carries state: faulting word retained at cont[0]
            assert_eq!(fi.cont, vec![prim(p)]);
            assert_eq!(fi.stack, vec![]);
        }
    }
    #[test]
    fn underflow_arity_2() {
        for p in [
            Prim::Swap,
            Prim::Over,
            Prim::Cat,
            Prim::Cons,
            Prim::Dip,
            Prim::Add,
            Prim::Sub,
            Prim::Mul,
            Prim::Div,
            Prim::Mod,
            Prim::Eq,
            Prim::Lt,
        ] {
            let fi = expect_fault(vec![i(1)], vec![prim(p)]);
            assert_eq!(fi.fault, Fault::Underflow, "prim {:?}", p);
        }
    }
    #[test]
    fn underflow_arity_3() {
        for p in [Prim::Rot, Prim::If] {
            let fi = expect_fault(vec![i(1), i(2)], vec![prim(p)]);
            assert_eq!(fi.fault, Fault::Underflow, "prim {:?}", p);
        }
    }

    // ---- v0.2 recursion primitive arity underflow ----
    #[test]
    fn underflow_uncons_arity_1() {
        // uncons needs 1.
        let fi = expect_fault(vec![], vec![uncons()]);
        assert_eq!(fi.fault, Fault::Underflow);
        assert_eq!(fi.cont, vec![uncons()]);
        assert_eq!(fi.stack, vec![]);
    }
    #[test]
    fn underflow_times_arity_2() {
        // times needs 2; 1 item -> Underflow before types are inspected.
        let fi = expect_fault(vec![q(vec![])], vec![times()]);
        assert_eq!(fi.fault, Fault::Underflow);
    }
    #[test]
    fn underflow_primrec_arity_3() {
        // primrec needs 3.
        let fi = expect_fault(vec![i(1), q(vec![])], vec![primrec()]);
        assert_eq!(fi.fault, Fault::Underflow);
    }
    #[test]
    fn underflow_linrec_arity_4() {
        // linrec needs 4.
        let fi = expect_fault(vec![q(vec![]), q(vec![]), q(vec![])], vec![linrec()]);
        assert_eq!(fi.fault, Fault::Underflow);
    }

    // ---- i64 edges: primrec/times count boundaries ----
    #[test]
    fn primrec_count_i64_min_runs_base_no_overflow() {
        // k = i64::MIN is <= 0, so the base runs; k-1 is never computed => no panic.
        assert_eq!(
            expect_halt(
                vec![i(i64::MIN)],
                vec![quote(vec![int(1)]), quote(vec![mul()]), primrec()]
            ),
            vec![i(1)]
        );
    }
    #[test]
    fn times_count_i64_min_is_noop_no_overflow() {
        assert_eq!(
            expect_halt(
                vec![i(7)],
                vec![int(i64::MIN), quote(vec![int(1), add()]), times()]
            ),
            vec![i(7)]
        );
    }
    #[test]
    fn primrec_combine_overflow_propagates() {
        // factorial with a large-ish count overflows i64 inside the combine `*`;
        // the Overflow fault comes from Mul, not from primrec's counting.
        let fi = expect_fault(
            vec![i(25)],
            vec![quote(vec![int(1)]), quote(vec![mul()]), primrec()],
        );
        assert_eq!(fi.fault, Fault::Overflow); // 25! >> i64::MAX
    }

    #[test]
    fn unknown_word_faults_with_state() {
        // Call in the pure v0.1 core -> UnknownWord (spec §8), state carried.
        let fi = expect_fault(vec![i(1)], vec![int(2), call("emit")]);
        assert_eq!(fi.fault, Fault::UnknownWord);
        assert_eq!(fi.stack, vec![i(1), i(2)]);
        assert_eq!(fi.cont, vec![call("emit")]); // faulting word at cont[0]
    }
}

// ============================================================
// PRECEDENCE — fault ordering documentation (adversarial review).
// The model carries only Int|Quote (no Str), so a Quote stands in for
// the "wrong non-Int type" of the guidance's `Str` cases.
// ============================================================
mod precedence {
    use super::*;

    #[test]
    fn arity_beats_type_single_wrong_operand() {
        // [Quote] + Add: one operand, wrong type. Arity (Underflow) wins over
        // the type check that would otherwise fire. (Guidance: `[Str] + Add`.)
        let fi = expect_fault(vec![q(vec![])], vec![add()]);
        assert_eq!(fi.fault, Fault::Underflow);
    }

    #[test]
    fn type_mismatch_when_arity_satisfied() {
        // [Quote, Int] + Add: arity ok, bottom operand wrong type -> TypeMismatch.
        // (Guidance: `[Str, Int] + Add`.)
        let fi = expect_fault(vec![q(vec![]), i(1)], vec![add()]);
        assert_eq!(fi.fault, Fault::TypeMismatch);
    }

    #[test]
    fn type_mismatch_beats_div_by_zero() {
        // [Quote, Int(0)] Div: the type match is the OUTER guard, so TypeMismatch
        // outranks DivByZero even though the divisor is 0.
        let fi = expect_fault(vec![q(vec![]), i(0)], vec![div()]);
        assert_eq!(fi.fault, Fault::TypeMismatch);
        // Contrast: both Int, divisor 0 -> DivByZero.
        let fi = expect_fault(vec![i(5), i(0)], vec![div()]);
        assert_eq!(fi.fault, Fault::DivByZero);
    }

    #[test]
    fn div_by_zero_beats_overflow() {
        // [Int(MIN), Int(0)] Div: b==0 is checked before the MIN/-1 overflow check.
        let fi = expect_fault(vec![i(i64::MIN), i(0)], vec![div()]);
        assert_eq!(fi.fault, Fault::DivByZero);
        // Contrast: [Int(MIN), Int(-1)] -> Overflow.
        let fi = expect_fault(vec![i(i64::MIN), i(-1)], vec![div()]);
        assert_eq!(fi.fault, Fault::Overflow);
    }

    #[test]
    fn type_mismatch_beats_overflow() {
        // Overflow can only be reached inside the both-Int arm; a non-Int operand
        // yields TypeMismatch first, never Overflow.
        let fi = expect_fault(vec![q(vec![]), q(vec![])], vec![add()]);
        assert_eq!(fi.fault, Fault::TypeMismatch);
    }

    #[test]
    fn arith_overflow_after_type_ok() {
        // Both Int, result out of range -> Overflow (the only remaining fault).
        let fi = expect_fault(vec![i(i64::MAX), i(1)], vec![add()]);
        assert_eq!(fi.fault, Fault::Overflow);
    }

    // ---- v0.2 recursion primitives: arity (Underflow) before types (TypeMismatch) ----

    #[test]
    fn primrec_arity_beats_type() {
        // Only 2 items, and both are the wrong shape for the count slot: arity
        // (needs 3) wins over the type check.
        let fi = expect_fault(vec![q(vec![]), q(vec![])], vec![primrec()]);
        assert_eq!(fi.fault, Fault::Underflow);
    }
    #[test]
    fn primrec_type_mismatch_when_arity_satisfied() {
        // 3 items but the count slot (stk[n-3]) is a Quote, not an Int.
        let fi = expect_fault(
            vec![q(vec![]), q(vec![]), q(vec![])],
            vec![primrec()],
        );
        assert_eq!(fi.fault, Fault::TypeMismatch);
        // And a non-Quote combine slot also mismatches.
        let fi = expect_fault(vec![i(1), q(vec![]), i(2)], vec![primrec()]);
        assert_eq!(fi.fault, Fault::TypeMismatch);
    }
    #[test]
    fn times_arity_beats_type() {
        // 1 item, wrong type: arity (needs 2) wins.
        let fi = expect_fault(vec![i(1)], vec![times()]);
        assert_eq!(fi.fault, Fault::Underflow);
    }
    #[test]
    fn times_type_mismatch_when_arity_satisfied() {
        // 2 items but top is not a Quote.
        let fi = expect_fault(vec![i(3), i(1)], vec![times()]);
        assert_eq!(fi.fault, Fault::TypeMismatch);
        // count slot not an Int.
        let fi = expect_fault(vec![q(vec![]), q(vec![])], vec![times()]);
        assert_eq!(fi.fault, Fault::TypeMismatch);
    }
    #[test]
    fn linrec_arity_beats_type() {
        // 3 items (need 4), one wrong type: arity wins.
        let fi = expect_fault(vec![i(1), q(vec![]), q(vec![])], vec![linrec()]);
        assert_eq!(fi.fault, Fault::Underflow);
    }
    #[test]
    fn linrec_type_mismatch_when_arity_satisfied() {
        // 4 items but one is not a Quote.
        let fi = expect_fault(
            vec![q(vec![]), i(1), q(vec![]), q(vec![])],
            vec![linrec()],
        );
        assert_eq!(fi.fault, Fault::TypeMismatch);
    }
    #[test]
    fn uncons_type_mismatch_on_non_quote() {
        // arity ok (1), operand is an Int, not a Quote -> TypeMismatch.
        let fi = expect_fault(vec![i(9)], vec![uncons()]);
        assert_eq!(fi.fault, Fault::TypeMismatch);
    }
    #[test]
    fn uncons_type_mismatch_on_non_value_head() {
        // A quotation whose head is a bare Prim (not PushInt/PushQuote) is not a
        // value; uncons faults TypeMismatch (the design's faithful reading).
        let fi = expect_fault(vec![q(vec![add()])], vec![uncons()]);
        assert_eq!(fi.fault, Fault::TypeMismatch);
        // Faulting word retained, machine state untouched.
        assert_eq!(fi.cont, vec![uncons()]);
        assert_eq!(fi.stack, vec![q(vec![add()])]);
        // A Call head also faults.
        let fi = expect_fault(vec![q(vec![call("emit")])], vec![uncons()]);
        assert_eq!(fi.fault, Fault::TypeMismatch);
    }
    #[test]
    fn uncons_value_head_variants_ok() {
        // Contrast: PushInt and PushQuote heads are values -> Next, not a fault.
        assert_eq!(
            expect_halt(vec![], vec![quote(vec![int(3), int(4)]), uncons()]),
            vec![i(3), q(vec![int(4)]), i(1)]
        );
    }
}

// ============================================================
// DIFFERENTIAL ORACLE — proptest exec_step/run vs. reference.
// ============================================================
mod differential {
    use super::*;
    use proptest::prelude::*;

    fn arb_prim() -> impl Strategy<Value = Prim> {
        prop_oneof![
            Just(Prim::Dup),
            Just(Prim::Drop),
            Just(Prim::Swap),
            Just(Prim::Rot),
            Just(Prim::Over),
            Just(Prim::Apply),
            Just(Prim::Cat),
            Just(Prim::Cons),
            Just(Prim::Dip),
            Just(Prim::Add),
            Just(Prim::Sub),
            Just(Prim::Mul),
            Just(Prim::Div),
            Just(Prim::Mod),
            Just(Prim::Eq),
            Just(Prim::Lt),
            Just(Prim::If),
            Just(Prim::PrimRec),
            Just(Prim::Times),
            Just(Prim::LinRec),
            Just(Prim::Uncons),
        ]
    }

    // Ints biased toward small values and boundary cases so arithmetic actually
    // fires (and occasionally overflows / hits MIN/-1).
    fn arb_int() -> impl Strategy<Value = i64> {
        prop_oneof![
            5 => -8i64..8,
            2 => any::<i64>(),
            1 => Just(i64::MIN),
            1 => Just(i64::MAX),
            1 => Just(-1i64),
            1 => Just(0i64),
        ]
    }

    fn arb_word() -> impl Strategy<Value = Word> {
        let leaf = prop_oneof![
            8 => arb_int().prop_map(Word::PushInt),
            8 => arb_prim().prop_map(Word::Prim),
            1 => "[a-z]".prop_map(Word::Call),
        ];
        leaf.prop_recursive(3, 24, 4, |inner| {
            prop::collection::vec(inner, 0..4).prop_map(Word::PushQuote)
        })
    }

    fn arb_value() -> impl Strategy<Value = Value> {
        prop_oneof![
            3 => arb_int().prop_map(Value::Int),
            2 => prop::collection::vec(arb_word(), 0..4).prop_map(Value::Quote),
        ]
    }

    fn arb_stack() -> impl Strategy<Value = Vec<Value>> {
        prop::collection::vec(arb_value(), 0..6)
    }

    fn arb_prog() -> impl Strategy<Value = Vec<Word>> {
        prop::collection::vec(arb_word(), 0..12)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]

        /// exec_step agrees with the reference oracle STEP-FOR-STEP: same result
        /// kind, same fault kind, same successor (stack, cont) — up to `fuel`.
        #[test]
        fn exec_step_refines_reference(
            init_stack in arb_stack(),
            prog in arb_prog(),
        ) {
            let fuel = 200u64;
            let mut stack = init_stack;
            let mut cont = prog;
            for _ in 0..fuel {
                let refr = oracle::ref_step(&stack, &cont);
                let (st, vm) = exec_once(stack.clone(), cont.clone());
                match refr {
                    oracle::RefStep::Halt(s) => {
                        prop_assert_eq!(st, Step::Halt);
                        prop_assert_eq!(&vm.stack, &s);
                        prop_assert!(vm.cont.is_empty());
                        break;
                    }
                    oracle::RefStep::Fault(f) => {
                        prop_assert_eq!(st, Step::Fault(f));
                        break;
                    }
                    oracle::RefStep::Next { stack: ns, cont: nc } => {
                        prop_assert_eq!(st, Step::Next);
                        prop_assert_eq!(&vm.stack, &ns);
                        prop_assert_eq!(&vm.cont, &nc);
                        stack = ns;
                        cont = nc;
                    }
                }
            }
        }

        /// `run` agrees with the reference driver on the terminal outcome.
        #[test]
        fn run_refines_reference(
            init_stack in arb_stack(),
            prog in arb_prog(),
        ) {
            let fuel = 500u64;
            let got = run(Vm::with_stack(init_stack.clone(), prog.clone()), fuel);
            let want = oracle::ref_run(init_stack, prog, fuel);
            prop_assert_eq!(got, want);
        }

        /// Totality (I2/P3 against the real binary): exec_step never panics and
        /// always returns one of the three outcomes on arbitrary input.
        #[test]
        fn exec_step_is_total(
            init_stack in arb_stack(),
            prog in arb_prog(),
        ) {
            let (st, _vm) = exec_once(init_stack, prog);
            match st {
                Step::Next | Step::Halt | Step::Fault(_) => {}
            }
        }
    }
}
