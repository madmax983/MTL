//! Differential oracle: for a corpus of programs, run each through BOTH the
//! reference interpreter (`mtl_core::interp::run`, the oracle of truth) and the
//! arena backend (`mtl_arena::run_arena`), reify the arena final stack to
//! `Vec<mtl_core::interp::Value>`, and assert EQUAL final stacks and the same
//! terminal kind (Halt / Fault(kind)). Any disagreement is an arena bug.
//!
//! Ported (corpus + check logic) from the spike's `tests/oracle.rs`, adapted to
//! the production `ArenaRun` surface (`run.state.stack` + `ArenaEnd`) and WIDENED
//! well beyond the original 47 cases: the parametric perf-builder sweeps are
//! densified across depth/breadth and the hand-built arithmetic/comparison
//! coverage is swept over sign/magnitude, so the arena-vs-reference differential
//! oracle now exercises 148 programs. All 148/148 must agree.

mod common;

use common::*;
use mtl_arena as arena;
use mtl_core::interp as itp;
use mtl_perf as perf;

const FUEL: u64 = 50_000_000;

/// Build the full program (initial stack encoded as leading pushes) and run both
/// backends. Returns Ok on agreement, Err(description) on divergence.
fn check(case: &Case) -> Result<(), String> {
    // full = <init as pushes> ++ prog, run on an empty stack in both backends.
    let mut full: Vec<itp::Word> = case.init.iter().map(value_to_word).collect();
    full.extend(case.prog.iter().cloned());

    let itp_out = itp::run(itp::Vm::new(full.clone()), FUEL);

    let prog_arena: Vec<arena::ProgWord> = full.iter().map(conv_word).collect();
    let run = arena::run_arena(&prog_arena, FUEL);
    let arena_stack: Vec<itp::Value> = run
        .vm
        .stack_values(run.state.stack)
        .into_iter()
        .map(|v| arena_value_to_itp(&run.vm, v))
        .collect();

    match (&itp_out, &run.end) {
        (itp::Outcome::Halt(s_itp), arena::ArenaEnd::Halt) => {
            if *s_itp == arena_stack {
                Ok(())
            } else {
                Err(format!(
                    "{}: HALT stacks differ\n  interp: {:?}\n  arena:  {:?}",
                    case.name, s_itp, arena_stack
                ))
            }
        }
        (itp::Outcome::Fault(fi), arena::ArenaEnd::Fault(f)) => {
            let same_kind = fault_eq(fi.fault, *f);
            if same_kind && fi.stack == arena_stack {
                Ok(())
            } else {
                Err(format!(
                    "{}: FAULT differs\n  interp: {:?} stack {:?}\n  arena:  {:?} stack {:?}",
                    case.name, fi.fault, fi.stack, f, arena_stack
                ))
            }
        }
        (i, a) => Err(format!(
            "{}: terminal kind differs\n  interp: {:?}\n  arena:  {:?}",
            case.name, i, a
        )),
    }
}

fn corpus() -> Vec<Case> {
    use itp::build::*;
    use itp::Prim;
    use itp::Value;
    use itp::Word;

    let mut cases = Vec::new();

    // ---- parametric stress sweeps (exact PERF-BASELINE builders) ----
    // These reuse the proven perf builders across a WIDE, densified parameter
    // envelope (well within the tested/no-overflow ranges documented on each
    // builder) so the differential oracle exercises the full recursion/dispatch
    // machinery — flat front-pop, PrimRec re-emit, Fold spine, self-application
    // splice, linrec, Times — at many depths/breadths rather than a token few.
    for k in [1usize, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
        cases.push(Case {
            name: format!("flat_1_1_add_drop_x{}", k),
            init: vec![],
            prog: perf::straightline(k),
        });
    }
    for n in [0i64, 1, 2, 3, 5, 10, 20, 50, 100, 200, 500, 1000] {
        cases.push(from_perf(&format!("primrec_sumto_{}", n), perf::primrec_sumto(n)));
    }
    for n in [0usize, 1, 2, 3, 5, 10, 20, 50, 100, 200, 500, 1000] {
        cases.push(from_perf(&format!("fold_sum_{}", n), perf::fold_sum(n)));
    }
    for n in [0i64, 1, 2, 3, 5, 10, 20, 50, 100, 200, 500, 1000] {
        cases.push(from_perf(&format!("selfapp_countdown_{}", n), perf::selfapp_countdown(n)));
    }

    // ---- other canonical perf shapes (densified) ----
    for n in [0i64, 1, 2, 3, 5, 10, 20, 50, 100, 200] {
        cases.push(from_perf(&format!("linrec_countdown_{}", n), perf::linrec_countdown(n)));
    }
    for n in [0i64, 1, 2, 3, 5, 10, 20, 50, 100, 200] {
        cases.push(from_perf(&format!("times_count_{}", n), perf::times_count(n)));
    }
    for n in [0usize, 1, 2, 3, 5, 10, 20, 50, 100, 200] {
        cases.push(from_perf(&format!("fold_quotes_{}", n), perf::fold_quotes(n)));
    }

    // ---- hand-built prim-coverage programs ----
    // arithmetic mix
    cases.push(prog("arith_mix", vec![int(3), int(4), add(), int(2), mul(), int(10), sub()]));
    // div / mod (truncating toward zero) across a widened sign/magnitude sweep;
    // every divisor is nonzero (div-by-zero is a dedicated fault case) and no
    // pair triggers the i64::MIN / -1 overflow corner.
    for (a, b) in [
        (17i64, 5i64),
        (-17, 5),
        (17, -5),
        (-17, -5),
        (100, 7),
        (-100, 7),
        (1, 1),
        (0, 5),
        (999, 1000),
        (-999, 1000),
    ] {
        cases.push(prog(&format!("div_{}_{}", a, b), vec![int(a), int(b), div()]));
        cases.push(prog(&format!("mod_{}_{}", a, b), vec![int(a), int(b), modulo()]));
    }
    // comparison (lt / eq) + bitwise xor across a sign/magnitude sweep.
    for (a, b) in [
        (3i64, 7i64),
        (7, 3),
        (9, 9),
        (-5, 5),
        (0, 0),
        (12, 10),
        (-1, -1),
    ] {
        cases.push(prog(&format!("cmp_lt_{}_{}", a, b), vec![int(a), int(b), lt()]));
        cases.push(prog(&format!("cmp_eq_{}_{}", a, b), vec![int(a), int(b), eq()]));
        cases.push(prog(&format!("xor_{}_{}", a, b), vec![int(a), int(b), xor()]));
    }
    // If both branches, across truthy/falsy conditions.
    for c in [1i64, 0, -1, 42, 7] {
        cases.push(prog(
            &format!("if_cond_{}", c),
            vec![int(c), quote(vec![int(111)]), quote(vec![int(222)]), iff()],
        ));
    }
    // shuffles
    cases.push(prog(
        "shuffles",
        vec![int(1), int(2), int(3), rot(), over(), swap(), dup(), drop()],
    ));
    // Cons / Cat / Uncons
    cases.push(prog("cons", vec![int(5), quote(vec![int(1), int(2)]), cons()]));
    cases.push(prog("cat", vec![quote(vec![int(1), int(2)]), quote(vec![int(3), int(4)]), cat()]));
    cases.push(prog("uncons_nonempty", vec![quote(vec![int(7), int(8), int(9)]), uncons()]));
    cases.push(prog("uncons_empty", vec![quote(vec![]), uncons()]));
    cases.push(prog(
        "uncons_quote_head",
        vec![quote(vec![Word::PushQuote(vec![int(1)]), int(2)]), uncons()],
    ));
    // dip
    cases.push(prog("dip", vec![int(1), int(2), quote(vec![int(10), add()]), dip()]));
    // apply
    cases.push(prog("apply", vec![int(3), quote(vec![int(4), mul()]), apply()]));
    // primrec factorial: n [1] [*] &   (base 1, combinator multiplies by index)
    for n in [0i64, 1, 2, 3, 4, 5, 6, 7, 8] {
        cases.push(Case {
            name: format!("primrec_factorial_{}", n),
            init: vec![Value::Int(n)],
            prog: vec![
                Word::PushQuote(vec![int(1)]),
                Word::PushQuote(vec![Word::Prim(Prim::Mul)]),
                Word::Prim(Prim::PrimRec),
            ],
        });
    }
    // reverse via fold + cons: [seq] [] [swap cons] fold  (build reversed list)
    cases.push(prog(
        "fold_reverse",
        vec![
            quote(vec![int(1), int(2), int(3), int(4)]),
            quote(vec![]),
            quote(vec![swap(), cons()]),
            fold(),
        ],
    ));
    // nested quote round-trip through apply
    cases.push(prog(
        "nested_apply",
        vec![int(2), quote(vec![quote(vec![int(3), add()]), apply()]), apply()],
    ));

    // ---- fault cases (fault order parity) ----
    cases.extend(fault_cases());

    cases
}

#[test]
fn differential_oracle() {
    let cases = corpus();
    let total = cases.len();
    let mut passed = 0usize;
    let mut failures = Vec::new();
    for c in &cases {
        match check(c) {
            Ok(()) => passed += 1,
            Err(e) => failures.push(e),
        }
    }
    println!("differential oracle: {}/{} programs agree", passed, total);
    if !failures.is_empty() {
        panic!(
            "{} / {} arena programs DIVERGED from the interpreter:\n{}",
            failures.len(),
            total,
            failures.join("\n")
        );
    }
    assert_eq!(passed, total);
    assert_eq!(total, 148, "corpus size drifted from the documented 148 cases");
}
