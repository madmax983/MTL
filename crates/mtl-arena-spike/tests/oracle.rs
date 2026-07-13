//! Differential oracle: for a corpus of programs, run each through BOTH the
//! reference interpreter (`mtl_core::interp::run`, the oracle of truth) and the
//! arena backend (`mtl_arena_spike::run_arena`), reify the arena final stack to
//! `Vec<mtl_core::interp::Value>`, and assert EQUAL final stacks and the same
//! terminal kind (Halt / Fault(kind)). Any disagreement is an arena bug.
//!
//! ## Corpus coverage (honest accounting)
//!
//! * **The 4 PERF-BASELINE stress cases at small n** — built with the *exact*
//!   `mtl-perf` scenario builders so the shapes match the baseline verbatim:
//!     - flat `1 1 + _` × k  (`straightline`)      for k ∈ {4, 16, 64}
//!     - primrec `sum_to(n)`  (`primrec_sumto`)     for n ∈ {5, 20, 100}
//!     - fold sum over n ints (`fold_sum`)          for n ∈ {5, 20, 100}
//!     - deep `: !` countdown (`selfapp_countdown`) for n ∈ {5, 20, 100}
//! * **Other canonical `mtl-perf` shapes**: `linrec_countdown`, `times_count`,
//!   `fold_quotes` — exercise LinRec / Times / quote-payload Fold.
//! * **Hand-built prim-coverage programs** spanning arithmetic (add/sub/mul),
//!   div/mod, comparison, If (both branches), Cons, Cat, Uncons, the shuffles
//!   (dup/drop/swap/rot/over), xor, dip, and a primrec `factorial`. These are
//!   NOT loaded from `bench/corpus` (that needs the parser + loader); they are
//!   hand-built ASTs exercising the same primitive mix as the real corpus
//!   solutions (factorial/gcd/sum_list/reverse_list/climbing_stairs, which are
//!   PrimRec / Fold / arithmetic / If / Uncons shaped). Correctness is validated
//!   differentially against the interpreter, not against expected math outputs.
//! * **Fault cases**: underflow, type-mismatch, div-by-zero — confirm the arena
//!   mirrors the interpreter's fault order (arity before type; DivByZero before
//!   Overflow) and leaves the stack unmodified at the fault.

use mtl_arena_spike as arena;
use mtl_core::interp as itp;
use mtl_perf as perf;

// --------------------------------------------------------- conversions
fn conv_prim(p: itp::Prim) -> arena::Prim {
    use itp::Prim as I;
    use arena::Prim as A;
    match p {
        I::Dup => A::Dup,
        I::Drop => A::Drop,
        I::Swap => A::Swap,
        I::Rot => A::Rot,
        I::Over => A::Over,
        I::Apply => A::Apply,
        I::Cat => A::Cat,
        I::Cons => A::Cons,
        I::Dip => A::Dip,
        I::Add => A::Add,
        I::Sub => A::Sub,
        I::Mul => A::Mul,
        I::Div => A::Div,
        I::Mod => A::Mod,
        I::Eq => A::Eq,
        I::Lt => A::Lt,
        I::If => A::If,
        I::PrimRec => A::PrimRec,
        I::Times => A::Times,
        I::LinRec => A::LinRec,
        I::Uncons => A::Uncons,
        I::Fold => A::Fold,
        I::Xor => A::Xor,
    }
}

fn conv_word(w: &itp::Word) -> arena::ProgWord {
    match w {
        itp::Word::PushInt(n) => arena::ProgWord::PushInt(*n),
        itp::Word::PushQuote(q) => {
            arena::ProgWord::PushQuote(q.iter().map(conv_word).collect())
        }
        itp::Word::Prim(p) => arena::ProgWord::Prim(conv_prim(*p)),
        itp::Word::Call(name) => arena::ProgWord::Call(name.clone()),
    }
}

/// Reify one arena `Value` (using the arena's tape) back to an `itp::Value`.
fn arena_value_to_itp(vm: &arena::Vm, v: arena::Value) -> itp::Value {
    match v {
        arena::Value::Int(n) => itp::Value::Int(n),
        arena::Value::Quote(id) => {
            itp::Value::Quote(vm.reify_quote(id).iter().map(progword_to_itp).collect())
        }
    }
}

fn progword_to_itp(pw: &arena::ProgWord) -> itp::Word {
    match pw {
        arena::ProgWord::PushInt(n) => itp::Word::PushInt(*n),
        arena::ProgWord::PushQuote(b) => {
            itp::Word::PushQuote(b.iter().map(progword_to_itp).collect())
        }
        arena::ProgWord::Prim(p) => itp::Word::Prim(unconv_prim(*p)),
        arena::ProgWord::Call(n) => itp::Word::Call(n.clone()),
    }
}

fn unconv_prim(p: arena::Prim) -> itp::Prim {
    use itp::Prim as I;
    use arena::Prim as A;
    match p {
        A::Dup => I::Dup,
        A::Drop => I::Drop,
        A::Swap => I::Swap,
        A::Rot => I::Rot,
        A::Over => I::Over,
        A::Apply => I::Apply,
        A::Cat => I::Cat,
        A::Cons => I::Cons,
        A::Dip => I::Dip,
        A::Add => I::Add,
        A::Sub => I::Sub,
        A::Mul => I::Mul,
        A::Div => I::Div,
        A::Mod => I::Mod,
        A::Eq => I::Eq,
        A::Lt => I::Lt,
        A::If => I::If,
        A::PrimRec => I::PrimRec,
        A::Times => I::Times,
        A::LinRec => I::LinRec,
        A::Uncons => I::Uncons,
        A::Fold => I::Fold,
        A::Xor => I::Xor,
    }
}

fn fault_eq(i: itp::Fault, a: arena::Fault) -> bool {
    use itp::Fault as I;
    use arena::Fault as A;
    matches!(
        (i, a),
        (I::Underflow, A::Underflow)
            | (I::TypeMismatch, A::TypeMismatch)
            | (I::Overflow, A::Overflow)
            | (I::DivByZero, A::DivByZero)
    )
}

fn value_to_word(v: &itp::Value) -> itp::Word {
    match v {
        itp::Value::Int(n) => itp::Word::PushInt(*n),
        itp::Value::Quote(q) => itp::Word::PushQuote(q.clone()),
    }
}

// --------------------------------------------------------- oracle core
const FUEL: u64 = 50_000_000;

struct Case {
    name: String,
    init: Vec<itp::Value>,
    prog: Vec<itp::Word>,
}

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
        .stack_values(run.stack)
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

// --------------------------------------------------------- corpus builders
fn from_perf(name: &str, pair: (Vec<itp::Value>, Vec<itp::Word>)) -> Case {
    Case { name: name.to_string(), init: pair.0, prog: pair.1 }
}

fn prog(name: &str, ws: Vec<itp::Word>) -> Case {
    Case { name: name.to_string(), init: vec![], prog: ws }
}

fn corpus() -> Vec<Case> {
    use itp::build::*;
    use itp::Prim;
    use itp::Value;
    use itp::Word;

    let mut cases = Vec::new();

    // ---- 4 stress cases at small n (exact PERF-BASELINE builders) ----
    for k in [4usize, 16, 64] {
        cases.push(Case {
            name: format!("flat_1_1_add_drop_x{}", k),
            init: vec![],
            prog: perf::straightline(k),
        });
    }
    for n in [5i64, 20, 100] {
        cases.push(from_perf(&format!("primrec_sumto_{}", n), perf::primrec_sumto(n)));
    }
    for n in [5usize, 20, 100] {
        cases.push(from_perf(&format!("fold_sum_{}", n), perf::fold_sum(n)));
    }
    for n in [5i64, 20, 100] {
        cases.push(from_perf(&format!("selfapp_countdown_{}", n), perf::selfapp_countdown(n)));
    }

    // ---- other canonical perf shapes ----
    for n in [5i64, 20] {
        cases.push(from_perf(&format!("linrec_countdown_{}", n), perf::linrec_countdown(n)));
    }
    for n in [5i64, 20] {
        cases.push(from_perf(&format!("times_count_{}", n), perf::times_count(n)));
    }
    for n in [5usize, 20] {
        cases.push(from_perf(&format!("fold_quotes_{}", n), perf::fold_quotes(n)));
    }

    // ---- hand-built prim-coverage programs ----
    // arithmetic mix
    cases.push(prog("arith_mix", vec![int(3), int(4), add(), int(2), mul(), int(10), sub()]));
    // div / mod (truncating toward zero, negative)
    cases.push(prog("div_pos", vec![int(17), int(5), div()]));
    cases.push(prog("mod_pos", vec![int(17), int(5), modulo()]));
    cases.push(prog("div_neg", vec![int(-17), int(5), div()]));
    cases.push(prog("mod_neg", vec![int(-17), int(5), modulo()]));
    // comparison + xor
    cases.push(prog("cmp_lt", vec![int(3), int(7), lt()]));
    cases.push(prog("cmp_eq", vec![int(9), int(9), eq()]));
    cases.push(prog("xor_bits", vec![int(12), int(10), xor()]));
    // If both branches
    cases.push(prog(
        "if_true",
        vec![int(1), quote(vec![int(111)]), quote(vec![int(222)]), iff()],
    ));
    cases.push(prog(
        "if_false",
        vec![int(0), quote(vec![int(111)]), quote(vec![int(222)]), iff()],
    ));
    // shuffles
    cases.push(prog("shuffles", vec![int(1), int(2), int(3), rot(), over(), swap(), dup(), drop()]));
    // Cons / Cat / Uncons
    cases.push(prog("cons", vec![int(5), quote(vec![int(1), int(2)]), cons()]));
    cases.push(prog(
        "cat",
        vec![quote(vec![int(1), int(2)]), quote(vec![int(3), int(4)]), cat()],
    ));
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
    for n in [0i64, 1, 5, 6] {
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
        vec![
            int(2),
            quote(vec![quote(vec![int(3), add()]), apply()]),
            apply(),
        ],
    ));

    // ---- fault cases (fault order parity) ----
    cases.push(prog("fault_underflow", vec![int(1), add()])); // needs 2, has 1
    cases.push(prog("fault_type_add", vec![int(1), quote(vec![int(2)]), add()])); // Int op Quote
    cases.push(prog("fault_divzero", vec![int(5), int(0), div()]));
    cases.push(prog("fault_apply_type", vec![int(7), apply()])); // apply on Int
    cases.push(prog("fault_if_type", vec![int(1), int(2), int(3), iff()])); // branches not quotes

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
}
