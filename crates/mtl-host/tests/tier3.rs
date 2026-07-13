//! Tier-3 executable-solution tests (v0.4 design §8). Each test loads the real
//! `bench/tier3/tasks/<t>/solution.mtl` (the same bytes that are token-counted),
//! parses + converts it, builds the task's standard registry + fixture context,
//! drives it, and asserts the expected output / terminal result.

use mtl_core::interp::Value;
use mtl_host::caps;
use mtl_host::driver::{drive, RunResult};
use mtl_host::host::HostCtx;
use mtl_host::load_solution;

/// Absolute path to a task's solution.mtl.
fn solution_path(task: &str) -> String {
    format!(
        "{}/../../bench/tier3/tasks/{}/solution.mtl",
        env!("CARGO_MANIFEST_DIR"),
        task
    )
}

/// Load + parse + convert a task's solution.mtl into an executable program.
fn program(task: &str) -> Vec<mtl_core::interp::Word> {
    load_solution(solution_path(task))
        .unwrap_or_else(|e| panic!("parse {task}/solution.mtl failed: {e:?}"))
}

const FUEL: u64 = 100_000;

#[test]
fn echo_line() {
    let prog = program("echo_line");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_echo_line());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert_eq!(r, RunResult::Done(vec![]), "output so far: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "hello world\n");
}

#[test]
fn grep_filter() {
    let prog = program("grep_filter");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_grep_filter());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "apple\napricot\n", "output: {:?}", ctx.output_utf8());
}

#[test]
fn agent_loop() {
    let prog = program("agent_loop");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_agent_loop());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    match r {
        RunResult::Done(stack) => {
            assert_eq!(stack.last(), Some(&Value::Int(5)), "final stack: {stack:?}");
        }
        other => panic!("expected Done, got {other:?}"),
    }
}

#[test]
fn json_field() {
    let prog = program("json_field");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_json_field());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?}");
    assert_eq!(ctx.output_utf8(), "neo\n", "output: {:?}", ctx.output_utf8());
}

#[test]
fn two_tool_pipeline() {
    let prog = program("two_tool_pipeline");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_two_tool_pipeline());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?}");
    assert_eq!(ctx.output_utf8(), "parsed:q1\n", "output: {:?}", ctx.output_utf8());
}

#[test]
fn retry_on_fault() {
    let prog = program("retry_on_fault");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_retry_on_fault());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    match r {
        RunResult::Done(stack) => {
            // Flaky op succeeds on the 3rd attempt; its result (Int 3) is left.
            assert_eq!(stack, vec![Value::Int(3)], "final stack: {stack:?}");
        }
        other => panic!("expected Done, got {other:?}"),
    }
    assert_eq!(ctx.calls_to("tryop"), 3, "tryop call count");
    assert_eq!(ctx.calls_to("okp"), 3, "okp call count");
}

#[test]
fn map_lines_tool() {
    let prog = program("map_lines_tool");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_map_lines_tool());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "A\nB\nC\n", "output: {:?}", ctx.output_utf8());
}

#[test]
fn word_count() {
    let prog = program("word_count");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_word_count());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "4\n", "output: {:?}", ctx.output_utf8());
}

// silence unused import warning if HostCtx type is only used via caps.
#[allow(dead_code)]
fn _uses(_c: &HostCtx) {}
