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

// ---------------------------------------------------------------------------
// v0.4 expansion: 8 new tasks (multi-cap, budget-aware, fault-handling,
// string-handle pipelines, and capability confinement).
// ---------------------------------------------------------------------------

/// Multi-cap: read every line, and for each that hits the predicate,
/// transform (uppercase) then emit; drop the misses.
#[test]
fn transform_hits() {
    let prog = program("transform_hits");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_transform_hits());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "APPLE\nAPRICOT\n", "output: {:?}", ctx.output_utf8());
}

/// Budget-aware: `emit` has a call budget of 2, and the program deliberately
/// emits only the first two lines, halting cleanly.
#[test]
fn emit_budget() {
    let prog = program("emit_budget");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_emit_budget());
    ctx.meter.set_call_budget("emit", 2);
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "one\ntwo\n", "output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.calls_to("emit"), 2, "emit call count");

    // A solution that over-emits (all four lines) would fault BudgetExhausted on
    // the 3rd emit — exactly two effects occur, the over-budget call writes
    // nothing. Verify that path with an ad-hoc over-emitting program.
    let overshoot = mtl_host::conv_program(
        &mtl_syntax::parse("readlines 0[emit](_").expect("parse overshoot"),
    );
    let (mut reg2, mut ctx2) = caps::standard_registry_and_ctx(caps::fixture_emit_budget());
    ctx2.meter.set_call_budget("emit", 2);
    let r2 = drive(overshoot, vec![], FUEL, &mut reg2, &mut ctx2);
    assert!(
        matches!(r2, RunResult::HostFaulted(mtl_host::driver::HostCode::BudgetExhausted)),
        "over-emitting must fault BudgetExhausted, got {r2:?}"
    );
    assert_eq!(ctx2.output_utf8(), "one\ntwo\n", "exactly 2 effects before budget refusal");
}

/// Fault-handling via control flow: every `nextline` is guarded by `endp` in a
/// linrec loop, so the faulting `nextline` is never reached at end-of-input.
#[test]
fn guarded_read() {
    let prog = program("guarded_read");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_guarded_read());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "x\ny\nz\n", "output: {:?}", ctx.output_utf8());

    // The point of the guard: a naive over-read (`3[nextline emit].`, one more
    // read than there are lines would be `4[nextline emit].`) faults InputClosed.
    let naive = mtl_host::conv_program(
        &mtl_syntax::parse("4[nextline emit].").expect("parse naive"),
    );
    let (mut reg2, mut ctx2) = caps::standard_registry_and_ctx(caps::fixture_guarded_read());
    let r2 = drive(naive, vec![], FUEL, &mut reg2, &mut ctx2);
    assert!(
        matches!(r2, RunResult::HostFaulted(mtl_host::driver::HostCode::InputClosed)),
        "unguarded over-read must fault InputClosed, got {r2:?}"
    );
}

/// String-handle pipeline: read two lines and emit their concatenation.
#[test]
fn concat_lines() {
    let prog = program("concat_lines");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_concat_lines());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "foobar\n", "output: {:?}", ctx.output_utf8());
}

/// String-handle pipeline: emit the line at index 2 of the read list.
#[test]
fn select_line() {
    let prog = program("select_line");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_select_line());
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "c\n", "output: {:?}", ctx.output_utf8());
}

/// Confinement: granted ONLY {readline, emit}. The reference solution stays in
/// grant and produces "hello\n".
#[test]
fn confined_echo() {
    let prog = program("confined_echo");
    let (mut reg, mut ctx) =
        caps::restricted_registry_and_ctx(caps::fixture_confined_echo(), &["readline", "emit"]);
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "hello\n", "output: {:?}", ctx.output_utf8());

    // Confinement negative control: a solution reaching for an ungranted cap
    // (`transform`) faults NotGranted with no effect, and names the offender.
    let escape = mtl_host::conv_program(
        &mtl_syntax::parse("readline transform emit").expect("parse escape"),
    );
    let (mut reg2, mut ctx2) =
        caps::restricted_registry_and_ctx(caps::fixture_confined_echo(), &["readline", "emit"]);
    let r2 = drive(escape, vec![], FUEL, &mut reg2, &mut ctx2);
    assert_eq!(
        r2,
        RunResult::HostFaulted(mtl_host::driver::HostCode::NotGranted),
        "ungranted transform must fault NotGranted"
    );
    assert_eq!(ctx2.last_denied.as_deref(), Some("transform"), "denied cap name");
    assert_eq!(ctx2.output_utf8(), "", "no output on the confined escape path");
}

/// Confinement: granted ONLY {readlines, linehit, emit}. The grep reference
/// stays in grant and produces the two hits.
#[test]
fn confined_grep() {
    let prog = program("confined_grep");
    let (mut reg, mut ctx) = caps::restricted_registry_and_ctx(
        caps::fixture_confined_grep(),
        &["readlines", "linehit", "emit"],
    );
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "cat\ncar\n", "output: {:?}", ctx.output_utf8());

    // Negative control: `transform` is ungranted here too.
    let escape = mtl_host::conv_program(
        &mtl_syntax::parse("readlines 0[transform emit](_").expect("parse escape"),
    );
    let (mut reg2, mut ctx2) = caps::restricted_registry_and_ctx(
        caps::fixture_confined_grep(),
        &["readlines", "linehit", "emit"],
    );
    let r2 = drive(escape, vec![], FUEL, &mut reg2, &mut ctx2);
    assert_eq!(
        r2,
        RunResult::HostFaulted(mtl_host::driver::HostCode::NotGranted),
        "ungranted transform must fault NotGranted"
    );
    assert_eq!(ctx2.last_denied.as_deref(), Some("transform"), "denied cap name");
}

/// Budget-aware multi-cap: `emit` budget = 2, and the grep has exactly 2 hits.
#[test]
fn budget_grep() {
    let prog = program("budget_grep");
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(caps::fixture_budget_grep());
    ctx.meter.set_call_budget("emit", 2);
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert!(matches!(r, RunResult::Done(_)), "result: {r:?} output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.output_utf8(), "ant\nart\n", "output: {:?}", ctx.output_utf8());
    assert_eq!(ctx.calls_to("emit"), 2, "emit call count");
}

// silence unused import warning if HostCtx type is only used via caps.
#[allow(dead_code)]
fn _uses(_c: &HostCtx) {}
