//! Interpreter-checked validation of the v0.3 Tier-2 dev corpus solutions.
//!
//! Each test loads the ACTUAL `bench/corpus/<task>/mtl-v0.3/solution.mtl` (the
//! same bytes the token harness counts), parses + converts it, and runs it on
//! the real `mtl-core` interpreter against the same honest input/output vectors
//! used for the v0.2 tier, then asserts the terminal `Outcome::Halt(expected)`.
//!
//! The v0.3 tier re-solves the 10 v0.2-solvable tasks using the sequence
//! primitives `(` (Fold) / `$` (Xor), and adds `single_number` (a v0.2 WALL now
//! cleared by xor) for 11 validated solutions. `palindrome_number` and
//! `climbing_stairs` are scalar tasks carried over verbatim from v0.2.

use std::path::PathBuf;

use mtl_bench_validate::load_solution;
use mtl_core::interp::{run, Outcome, Value, Vm, Word as IWord};

const FUEL: u64 = 100_000;

fn i(n: i64) -> Value {
    Value::Int(n)
}

/// Build a flat list literal `[e0 e1 ...]` as a runtime `Value::Quote` of
/// `PushInt` words — the shape `(`/`;`/`>` operate on.
fn q(elems: &[i64]) -> Value {
    Value::Quote(elems.iter().map(|&n| IWord::PushInt(n)).collect())
}

/// Absolute path to `bench/corpus/<task>/mtl-v0.3/solution.mtl`.
fn solution_path_v3(task: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("corpus")
        .join(task)
        .join("mtl-v0.3")
        .join("solution.mtl")
}

/// Load, execute, and assert `Halt(expected)` for one (task, input, expected).
#[track_caller]
fn check_v3(task: &str, input: Vec<Value>, expected: Vec<Value>) {
    let path = solution_path_v3(task);
    let prog = load_solution(&path).unwrap_or_else(|e| panic!("{task}: parse failed: {e}"));
    let outcome = run(Vm::with_stack(input.clone(), prog), FUEL);
    match outcome {
        Outcome::Halt(stack) => assert_eq!(
            stack, expected,
            "{task}: input {input:?} -> got {stack:?}, expected {expected:?}"
        ),
        other => panic!("{task}: input {input:?} did not Halt: {other:?}"),
    }
}

#[test]
fn sum_list() {
    // `0[+](` — Fold. [xs] -> [sum]
    check_v3("sum_list", vec![q(&[1, 2, 3])], vec![i(6)]);
    check_v3("sum_list", vec![q(&[5])], vec![i(5)]);
    check_v3("sum_list", vec![q(&[10, 20, 30, 40])], vec![i(100)]);
    check_v3("sum_list", vec![q(&[])], vec![i(0)]);
    check_v3("sum_list", vec![q(&[0])], vec![i(0)]);
}

#[test]
fn length_list() {
    // `0[_1+](` — Fold. [xs] -> [len]
    check_v3("length_list", vec![q(&[1, 2, 3])], vec![i(3)]);
    check_v3("length_list", vec![q(&[])], vec![i(0)]);
    check_v3("length_list", vec![q(&[7, 7, 7, 7, 7])], vec![i(5)]);
}

#[test]
fn product_list() {
    // `1[*](` — Fold. [xs] -> [product]
    check_v3("product_list", vec![q(&[1, 2, 3, 4])], vec![i(24)]);
    check_v3("product_list", vec![q(&[])], vec![i(1)]);
    check_v3("product_list", vec![q(&[5])], vec![i(5)]);
    check_v3("product_list", vec![q(&[2, 3, 0, 4])], vec![i(0)]);
}

#[test]
fn max_list() {
    // `>_~[^^<[~_][_]?](` — Fold. [xs] -> [max], non-empty only
    check_v3("max_list", vec![q(&[3, 1, 2])], vec![i(3)]);
    check_v3("max_list", vec![q(&[5])], vec![i(5)]);
    check_v3("max_list", vec![q(&[1, 9, 4, 9, 2])], vec![i(9)]);
    check_v3("max_list", vec![q(&[10, 20, 5])], vec![i(20)]);
}

#[test]
fn min_list() {
    // `>_~[^^<[_][~_]?](` — Fold. [xs] -> [min], non-empty only
    check_v3("min_list", vec![q(&[3, 1, 2])], vec![i(1)]);
    check_v3("min_list", vec![q(&[5])], vec![i(5)]);
    check_v3("min_list", vec![q(&[9, 4, 9, 2])], vec![i(2)]);
}

#[test]
fn reverse_list() {
    // `[][~;](` — Fold. [xs] -> [reversed] : the OUTPUT is a quotation
    check_v3("reverse_list", vec![q(&[1, 2, 3])], vec![q(&[3, 2, 1])]);
    check_v3("reverse_list", vec![q(&[])], vec![q(&[])]);
    check_v3("reverse_list", vec![q(&[7])], vec![q(&[7])]);
    check_v3("reverse_list", vec![q(&[1, 2, 3, 4])], vec![q(&[4, 3, 2, 1])]);
}

#[test]
fn contains() {
    // `[=+0~<];0~(` — Fold. [xs] x -> [0|1] (x on top)
    check_v3("contains", vec![q(&[1, 2, 3]), i(2)], vec![i(1)]);
    check_v3("contains", vec![q(&[1, 2, 3]), i(5)], vec![i(0)]);
    check_v3("contains", vec![q(&[]), i(5)], vec![i(0)]);
    check_v3("contains", vec![q(&[7]), i(7)], vec![i(1)]);
    check_v3("contains", vec![q(&[4, 4, 4]), i(4)], vec![i(1)]);
}

#[test]
fn count_occurrences() {
    // `[=+];0~(` — Fold. [xs] x -> [count] (x on top)
    check_v3("count_occurrences", vec![q(&[1, 2, 2, 3]), i(2)], vec![i(2)]);
    check_v3("count_occurrences", vec![q(&[1, 2, 3]), i(5)], vec![i(0)]);
    check_v3("count_occurrences", vec![q(&[]), i(5)], vec![i(0)]);
    check_v3("count_occurrences", vec![q(&[4, 4, 4]), i(4)], vec![i(3)]);
    check_v3("count_occurrences", vec![q(&[5, 5, 5, 5]), i(5)], vec![i(4)]);
}

#[test]
fn single_number() {
    // `[>0=][0][][$]|` — LinRec + Xor. WALL cleared in v0.3. [xs] -> [unique]
    check_v3("single_number", vec![q(&[4, 1, 2, 1, 2])], vec![i(4)]);
    check_v3("single_number", vec![q(&[2, 2, 7])], vec![i(7)]);
    check_v3("single_number", vec![q(&[99])], vec![i(99)]);
}

#[test]
fn palindrome_number() {
    // `0^[:1<][_=][:10%@10*+~10/][]|` — unchanged from v0.2 (scalar). [n] -> [0|1]
    for (n, out) in [(121, 1), (123, 0), (7, 1), (1221, 1), (10, 0), (0, 1)] {
        check_v3("palindrome_number", vec![i(n)], vec![i(out)]);
    }
}

#[test]
fn climbing_stairs() {
    // `1 1@[~^+]._` — unchanged from v0.2 (scalar). [n] -> [ways]
    for (n, out) in [(0, 1), (1, 1), (2, 2), (3, 3), (4, 5), (5, 8)] {
        check_v3("climbing_stairs", vec![i(n)], vec![i(out)]);
    }
}
