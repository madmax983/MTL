//! Interpreter-checked validation of every MTL corpus solution.
//!
//! For each task we load the ACTUAL `bench/corpus/<task>/mtl/solution.mtl`
//! (the same bytes the token harness counts), parse + convert it, run it on the
//! real `mtl-core` interpreter against honest input/output vectors, and assert
//! the terminal `Outcome::Halt(expected)`.

use std::path::PathBuf;

use mtl_bench_validate::{load_solution, run_program, Engine};
use mtl_core::interp::{Outcome, Value};

const FUEL: u64 = 100_000;

fn i(n: i64) -> Value {
    Value::Int(n)
}

/// Absolute path to `bench/corpus/<task>/mtl/solution.mtl` (v0.1 solution set).
fn solution_path(task: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR == bench/validate ; corpus lives at bench/corpus.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("corpus")
        .join(task)
        .join("mtl")
        .join("solution.mtl")
}

/// Absolute path to `bench/corpus/<task>/mtl-v0.2/solution.mtl` (v0.2 set).
fn solution_path_v2(task: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("corpus")
        .join(task)
        .join("mtl-v0.2")
        .join("solution.mtl")
}

/// Load, execute, and assert Halt(expected) for one (task, input, expected).
#[track_caller]
fn check(task: &str, input: Vec<Value>, expected: Vec<Value>) {
    check_path(task, solution_path(task), input, expected)
}

/// Same as [`check`] but loads the v0.2 solution set.
#[track_caller]
fn check_v2(task: &str, input: Vec<Value>, expected: Vec<Value>) {
    check_path(task, solution_path_v2(task), input, expected)
}

/// Load, execute, and assert Halt(expected) for one (task, path, input, expected).
#[track_caller]
fn check_path(task: &str, path: PathBuf, input: Vec<Value>, expected: Vec<Value>) {
    let prog = load_solution(&path).unwrap_or_else(|e| panic!("{task}: parse failed: {e}"));
    // Default engine is the arena (refinement-proved); interp stays reachable as
    // the differential anchor via `Engine::Interp`.
    let outcome = run_program(Engine::default(), &prog, &input, FUEL);
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

// --- v0.2 solution set (mtl-v0.2/), recursion primitives ---

#[test]
fn factorial_v02() {
    // [1][*]& (PrimRec): input [n] -> [n!], 0! = 1
    for (n, out) in [(0, 1), (1, 1), (2, 2), (3, 6), (5, 120), (6, 720)] {
        check_v2("factorial", vec![i(n)], vec![i(out)]);
    }
}

#[test]
fn gcd_v02() {
    // [:0=][_][~^%][]| (LinRec): input [a, b] (b on top) -> [gcd(a, b)]
    for (a, b, out) in [
        (12, 8, 4),
        (48, 36, 12),
        (17, 5, 1),
        (0, 5, 5),
        (5, 0, 5),
        (10, 10, 10),
    ] {
        check_v2("gcd", vec![i(a), i(b)], vec![i(out)]);
    }
}

#[test]
fn fib() {
    // 0 1@[~^+]._ (Times): input [n] -> [fib(n)], fib(0)=0, fib(1)=1
    for (n, out) in [(0, 0), (1, 1), (2, 1), (3, 2), (5, 5), (10, 55)] {
        check_v2("fib", vec![i(n)], vec![i(out)]);
    }
}

#[test]
fn sum_to() {
    // [0][+]& (PrimRec): input [n] -> [0 + 1 + ... + n]
    for (n, out) in [(0, 0), (1, 1), (3, 6), (10, 55)] {
        check_v2("sum_to", vec![i(n)], vec![i(out)]);
    }
}

#[test]
fn power() {
    // 1~[^*].~_ (Times): input [b, e] (e on top) -> [b^e], b^0 = 1
    for (b, e, out) in [(2, 0, 1), (2, 3, 8), (3, 4, 81), (5, 2, 25)] {
        check_v2("power", vec![i(b), i(e)], vec![i(out)]);
    }
}
