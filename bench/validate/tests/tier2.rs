//! Interpreter-checked validation of the Tier-2 dev corpus (`T_tier2-dev`).
//!
//! Each test loads the ACTUAL `bench/corpus/<task>/mtl-v0.2/solution.mtl` (the
//! same bytes the token harness counts), parses + converts it, and runs it on
//! the real `mtl-core` interpreter against honest input/output vectors, then
//! asserts the terminal `Outcome::Halt(expected)`.
//!
//! Inputs are pushed as stack `Value`s (ints via `i`, list literals via `q`),
//! matching how a caller would prepend them; list OUTPUTS (reverse_list) are
//! compared against a constructed `Value::Quote`. Only the 10 EXPRESSIBLE
//! tier-2 tasks are executed here — the 3 walls (single_number, two_sum,
//! binary_search) carry no MTL solution and are documented in their WALL.md.

use std::path::PathBuf;

use mtl_bench_validate::load_solution;
use mtl_core::interp::{run, Outcome, Value, Vm, Word as IWord};

const FUEL: u64 = 100_000;

fn i(n: i64) -> Value {
    Value::Int(n)
}

/// Build a flat list literal `[e0 e1 ...]` as a runtime `Value::Quote` of
/// `PushInt` words — the shape `uncons`/`cons` operate on.
fn q(elems: &[i64]) -> Value {
    Value::Quote(elems.iter().map(|&n| IWord::PushInt(n)).collect())
}

/// Absolute path to `bench/corpus/<task>/mtl-v0.2/solution.mtl`.
fn solution_path_v2(task: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("corpus")
        .join(task)
        .join("mtl-v0.2")
        .join("solution.mtl")
}

/// Load, execute, and assert `Halt(expected)` for one (task, input, expected).
#[track_caller]
fn check_v2(task: &str, input: Vec<Value>, expected: Vec<Value>) {
    let path = solution_path_v2(task);
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
    // [xs] -> [sum]
    check_v2("sum_list", vec![q(&[1, 2, 3])], vec![i(6)]);
    check_v2("sum_list", vec![q(&[5])], vec![i(5)]);
    check_v2("sum_list", vec![q(&[10, 20, 30, 40])], vec![i(100)]);
    check_v2("sum_list", vec![q(&[])], vec![i(0)]);
    check_v2("sum_list", vec![q(&[0])], vec![i(0)]);
}

#[test]
fn length_list() {
    // [xs] -> [len]
    check_v2("length_list", vec![q(&[1, 2, 3])], vec![i(3)]);
    check_v2("length_list", vec![q(&[])], vec![i(0)]);
    check_v2("length_list", vec![q(&[7, 7, 7, 7, 7])], vec![i(5)]);
}

#[test]
fn product_list() {
    // [xs] -> [product]
    check_v2("product_list", vec![q(&[1, 2, 3, 4])], vec![i(24)]);
    check_v2("product_list", vec![q(&[])], vec![i(1)]);
    check_v2("product_list", vec![q(&[5])], vec![i(5)]);
    check_v2("product_list", vec![q(&[2, 3, 0, 4])], vec![i(0)]);
}

#[test]
fn max_list() {
    // [xs] -> [max], non-empty only
    check_v2("max_list", vec![q(&[3, 1, 2])], vec![i(3)]);
    check_v2("max_list", vec![q(&[5])], vec![i(5)]);
    check_v2("max_list", vec![q(&[1, 9, 4, 9, 2])], vec![i(9)]);
    check_v2("max_list", vec![q(&[10, 20, 5])], vec![i(20)]);
}

#[test]
fn min_list() {
    // [xs] -> [min], non-empty only
    check_v2("min_list", vec![q(&[3, 1, 2])], vec![i(1)]);
    check_v2("min_list", vec![q(&[5])], vec![i(5)]);
    check_v2("min_list", vec![q(&[9, 4, 9, 2])], vec![i(2)]);
}

#[test]
fn reverse_list() {
    // [xs] -> [reversed] : the OUTPUT is a quotation
    check_v2("reverse_list", vec![q(&[1, 2, 3])], vec![q(&[3, 2, 1])]);
    check_v2("reverse_list", vec![q(&[])], vec![q(&[])]);
    check_v2("reverse_list", vec![q(&[7])], vec![q(&[7])]);
    check_v2("reverse_list", vec![q(&[1, 2, 3, 4])], vec![q(&[4, 3, 2, 1])]);
}

#[test]
fn palindrome_number() {
    // [n] -> [1 if palindrome else 0]
    for (n, out) in [(121, 1), (123, 0), (7, 1), (1221, 1), (10, 0), (0, 1)] {
        check_v2("palindrome_number", vec![i(n)], vec![i(out)]);
    }
}

#[test]
fn climbing_stairs() {
    // [n] -> [ways]
    for (n, out) in [(0, 1), (1, 1), (2, 2), (3, 3), (4, 5), (5, 8)] {
        check_v2("climbing_stairs", vec![i(n)], vec![i(out)]);
    }
}

#[test]
fn contains() {
    // [xs] x -> [0|1] (x on top)
    check_v2("contains", vec![q(&[1, 2, 3]), i(2)], vec![i(1)]);
    check_v2("contains", vec![q(&[1, 2, 3]), i(5)], vec![i(0)]);
    check_v2("contains", vec![q(&[]), i(5)], vec![i(0)]);
    check_v2("contains", vec![q(&[7]), i(7)], vec![i(1)]);
    check_v2("contains", vec![q(&[4, 4, 4]), i(4)], vec![i(1)]);
}

#[test]
fn count_occurrences() {
    // [xs] x -> [count] (x on top)
    check_v2("count_occurrences", vec![q(&[1, 2, 2, 3]), i(2)], vec![i(2)]);
    check_v2("count_occurrences", vec![q(&[1, 2, 3]), i(5)], vec![i(0)]);
    check_v2("count_occurrences", vec![q(&[]), i(5)], vec![i(0)]);
    check_v2("count_occurrences", vec![q(&[4, 4, 4]), i(4)], vec![i(3)]);
    check_v2("count_occurrences", vec![q(&[5, 5, 5, 5]), i(5)], vec![i(4)]);
}
