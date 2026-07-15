//! Dev-parity (constructed-stack) validation of the sealed eval set (issue #53).
//!
//! The sealed *text* harness (`printf '<input> <program>' | mtlrun`) lexes its
//! integer inputs, and the frozen grammar has no negative literal (`-` lexes to
//! the `Sub` primitive). That harness therefore cannot feed a negative scalar
//! (`-5`) or a list with negative elements (`[-5 -2 ...]`): the vector faults
//! before the solution runs. That is an INPUT-ENCODING artifact, not an
//! algorithmic one.
//!
//! This test validates the sealed solutions exactly the way the dev
//! `BASELINE-TIER2` numbers are validated (`tests/tier2.rs`): it builds the
//! input stack from real `Value::Int`s (negatives included) and `int_list`
//! (negative list elements included), runs each committed solution to HALT, and
//! compares the final stack to the constructed expected value(s). It reads the
//! honest I/O vectors from `bench/sealed/tasks.json` (`python.vectors`).
//!
//! A solution counts as ALGORITHMICALLY CORRECT iff it passes ALL of its
//! vectors under constructed-stack interpretation. 14/15 committed sealed
//! solutions pass; `seal_running_max` has no committed solution because its
//! authored candidate is algorithmically WRONG (it seeds the running maximum at
//! `0`, so an all-negative input such as `[-5 -2 -8 -1]` yields `[0 0 0 0]`
//! instead of `[-5 -2 -2 -1]`). That candidate is asserted to FAIL here so the
//! known gap is a permanent regression check — it is recorded, not patched
//! (issue #53). No frozen glyph/primitive/semantics was modified.

use std::path::PathBuf;

use mtl_bench_validate::{conv_program, run_program, Engine};
use mtl_core::interp::{Outcome, Value, Word as IWord};
use mtl_syntax::parse;

const FUEL: u64 = 100_000;

/// Build a flat list literal `[e0 e1 ...]` as a runtime `Value::Quote` of
/// `PushInt` words — negatives included. Mirrors `mtl_dataset::int_list` and the
/// `q` helper in `tests/tier2.rs`.
fn int_list(xs: &[i64]) -> Value {
    Value::Quote(xs.iter().map(|&n| IWord::PushInt(n)).collect())
}

fn sealed_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("sealed")
}

/// The `seal_running_max` candidate from GAPS.md — kept verbatim so the known
/// algorithmic bug (seed = 0) is pinned as a regression. NOT committed as a
/// corpus solution because it is wrong under full validation.
const RUNNING_MAX_BROKEN_CANDIDATE: &str = "0~[>0=][_[]][[^^<[~_][_]?]'[:]'][;]|";

/// Read a committed corpus solution, stripping one trailing newline.
fn committed(id: &str) -> Option<String> {
    let p = sealed_root()
        .join("corpus")
        .join(id)
        .join("mtl")
        .join("solution.mtl");
    std::fs::read_to_string(&p)
        .ok()
        .map(|s| s.strip_suffix('\n').unwrap_or(&s).to_string())
}

/// Convert one `python.vectors` cell (int or int-list) to a runtime `Value`.
fn cell_to_value(c: &serde_json::Value) -> Value {
    if let Some(n) = c.as_i64() {
        Value::Int(n)
    } else if let Some(arr) = c.as_array() {
        let ints: Vec<i64> = arr
            .iter()
            .map(|e| e.as_i64().expect("list element must be an integer"))
            .collect();
        int_list(&ints)
    } else {
        panic!("unexpected vector cell {c:?}")
    }
}

/// Run one program against one constructed-stack vector; return whether it
/// `Halt`s with exactly the expected stack.
fn vector_passes(prog: &[IWord], input: &[Value], expected: &[Value]) -> bool {
    matches!(
        run_program(Engine::default(), prog, input, FUEL),
        Outcome::Halt(ref stack) if stack == expected
    )
}

/// Parse tasks.json once and return `(id, tier, program-source-if-committed,
/// vectors)` per task.
fn load_tasks() -> serde_json::Value {
    let raw = std::fs::read_to_string(sealed_root().join("tasks.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

/// Every COMMITTED sealed solution passes ALL its vectors under constructed-stack
/// interpretation (negatives included) — the dev-parity contract.
#[test]
fn committed_solutions_pass_all_vectors_constructed_stack() {
    let json = load_tasks();
    let tasks = json["tasks"].as_array().unwrap();
    let mut checked = 0;
    for task in tasks {
        let id = task["id"].as_str().unwrap();
        let Some(src) = committed(id) else {
            // Only seal_running_max is intentionally uncommitted (see the
            // dedicated regression test below).
            assert_eq!(
                id, "seal_running_max",
                "task {id} has no committed solution but is not the known gap"
            );
            continue;
        };
        let prog = conv_program(&parse(&src).unwrap_or_else(|e| panic!("{id}: parse: {e}")));
        for v in task["python"]["vectors"].as_array().unwrap() {
            let input: Vec<Value> = v["args"].as_array().unwrap().iter().map(cell_to_value).collect();
            let expected = vec![cell_to_value(&v["expected"])];
            let outcome = run_program(Engine::default(), &prog, &input, FUEL);
            match outcome {
                Outcome::Halt(ref stack) => assert_eq!(
                    stack, &expected,
                    "{id}: args {:?} -> got {stack:?}, expected {expected:?}",
                    v["args"]
                ),
                other => panic!("{id}: args {:?} did not Halt: {other:?}", v["args"]),
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 14, "expected 14 committed algorithmically-correct tasks");
}

/// `seal_running_max`'s authored candidate is algorithmically WRONG: seeding the
/// running maximum at `0` breaks all-negative inputs. This pins that gap — the
/// candidate must FAIL at least one vector under full (constructed-stack)
/// validation. Recorded, not patched (issue #53).
#[test]
fn running_max_candidate_is_algorithmically_wrong() {
    let json = load_tasks();
    let task = json["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "seal_running_max")
        .unwrap();
    let prog = conv_program(&parse(RUNNING_MAX_BROKEN_CANDIDATE).unwrap());
    let mut all_pass = true;
    for v in task["python"]["vectors"].as_array().unwrap() {
        let input: Vec<Value> = v["args"].as_array().unwrap().iter().map(cell_to_value).collect();
        let expected = vec![cell_to_value(&v["expected"])];
        if !vector_passes(&prog, &input, &expected) {
            all_pass = false;
        }
    }
    assert!(
        !all_pass,
        "running_max candidate unexpectedly passed ALL vectors — if it is now \
         correct, commit it as a corpus solution and move it out of GAPS.md"
    );

    // Concretely: the all-negative vector is the one it gets wrong.
    let neg = int_list(&[-5, -2, -8, -1]);
    let expected = vec![int_list(&[-5, -2, -2, -1])];
    assert!(
        !vector_passes(&prog, &[neg], &expected),
        "running_max candidate should mishandle the all-negative vector"
    );
}
