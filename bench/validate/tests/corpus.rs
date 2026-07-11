//! Interpreter-checked validation of every MTL corpus solution.
//!
//! For each task we load the ACTUAL `bench/corpus/<task>/mtl/solution.mtl`
//! (the same bytes the token harness counts), parse + convert it, run it on the
//! real `mtl-core` interpreter against honest input/output vectors, and assert
//! the terminal `Outcome::Halt(expected)`.

use std::path::PathBuf;

use mtl_bench_validate::load_solution;
use mtl_core::interp::{run, Outcome, Value, Vm};

const FUEL: u64 = 100_000;

fn i(n: i64) -> Value {
    Value::Int(n)
}

/// Absolute path to `bench/corpus/<task>/mtl/solution.mtl`.
fn solution_path(task: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR == bench/validate ; corpus lives at bench/corpus.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("corpus")
        .join(task)
        .join("mtl")
        .join("solution.mtl")
}

/// Load, execute, and assert Halt(expected) for one (task, input, expected).
#[track_caller]
fn check(task: &str, input: Vec<Value>, expected: Vec<Value>) {
    let prog = load_solution(solution_path(task))
        .unwrap_or_else(|e| panic!("{task}: parse failed: {e}"));
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
fn affine() {
    // input [n] -> [3n + 7]
    for (n, out) in [(0, 7), (1, 10), (5, 22), (2, 13)] {
        check("affine", vec![i(n)], vec![i(out)]);
    }
}

#[test]
fn rev3() {
    // input [a, b, c] (c on top) -> [c, b, a]
    check("rev3", vec![i(1), i(2), i(3)], vec![i(3), i(2), i(1)]);
    check("rev3", vec![i(7), i(8), i(9)], vec![i(9), i(8), i(7)]);
}

#[test]
fn is_even() {
    // input [n] -> [1 if even else 0]
    for (n, out) in [(0, 1), (1, 0), (2, 1), (3, 0), (4, 1)] {
        check("is_even", vec![i(n)], vec![i(out)]);
    }
}

#[test]
fn factorial() {
    // input [n] -> [n!], 0! = 1
    for (n, out) in [(0, 1), (1, 1), (2, 2), (3, 6), (5, 120), (6, 720)] {
        check("factorial", vec![i(n)], vec![i(out)]);
    }
}

#[test]
fn gcd() {
    // input [a, b] (b on top) -> [gcd(a, b)], Euclid, gcd(a, 0) = a
    for (a, b, out) in [
        (12, 8, 4),
        (48, 36, 12),
        (17, 5, 1),
        (0, 5, 5),
        (5, 0, 5),
        (10, 10, 10),
    ] {
        check("gcd", vec![i(a), i(b)], vec![i(out)]);
    }
}
