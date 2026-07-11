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
