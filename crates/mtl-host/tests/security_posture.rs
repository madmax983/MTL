//! Security-posture tests — the selling point of the v0.4 host half. Each test
//! name reads like a claim the crate proves about capability confinement,
//! metering, and clean cancellation (design §3, §6, §7).

use mtl_core::host::HostCode;
use mtl_core::interp::build::{call, drop as vdrop, dup, int, linrec, sub};
use mtl_core::interp::{Value, Word};
use mtl_host::capability::{Capability, Registry, StackEffect};
use mtl_host::caps;
use mtl_host::driver::{drive, RunResult};
use mtl_host::host::{HostCtx, HostFault, TaskFixture};

const FUEL: u64 = 100_000;

/// A program that reads one line and emits it `n` times: `readline dup..dup emit..emit`.
fn read_and_emit_n(n: usize) -> Vec<Word> {
    let mut prog = vec![call("readline")];
    for _ in 1..n {
        prog.push(dup());
    }
    for _ in 0..n {
        prog.push(call("emit"));
    }
    prog
}

/// A registry that grants ONLY `readline` (an inline reimplementation), so
/// `emit` is deliberately ungranted.
fn registry_readline_only() -> Registry {
    let mut reg = Registry::new();
    reg.register(Capability::new(
        "readline",
        StackEffect::new(0, 1),
        vec![],
        Box::new(|ctx: &mut HostCtx, stack: &mut Vec<Value>| {
            let line = ctx
                .fixture
                .lines
                .first()
                .cloned()
                .ok_or(HostFault::InputClosed)?;
            let h = ctx.handles.intern(line);
            stack.push(Value::Int(h));
            Ok(())
        }),
    ));
    reg
}

fn echo_fixture() -> TaskFixture {
    caps::fixture_echo_line()
}

#[test]
fn a_capability_not_granted_is_unreachable() {
    // `emit` is NOT in the registry. The program tries `readline emit`.
    // Post-reconciliation the core seam has no `Refused` outcome: an ungranted
    // capability surfaces host-side as `HostFaulted(HostCode::NotGranted)`, and
    // the guarantee is unchanged — no effect, no output.
    let mut reg = registry_readline_only();
    let mut ctx = HostCtx::new(echo_fixture());
    let prog = vec![call("readline"), call("emit")];
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert_eq!(r, RunResult::HostFaulted(HostCode::NotGranted));
    // The ungranted effect never happened: nothing was emitted.
    assert!(ctx.output_bytes().is_empty(), "ungranted emit produced output: {:?}", ctx.output_utf8());
}

#[test]
fn an_unknown_capability_is_refused_not_executed() {
    // A name that was never registered anywhere.
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(echo_fixture());
    let prog = vec![call("nope")];
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert_eq!(r, RunResult::HostFaulted(HostCode::NotGranted));
    // A NotGranted refusal is categorically distinct from a pure-core fault.
    assert!(!matches!(r, RunResult::Faulted(_)));
    assert!(ctx.output_bytes().is_empty());
}

#[test]
fn budget_exhaustion_cancels_with_no_partial_effect() {
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(echo_fixture());
    ctx.meter.set_call_budget("emit", 2);
    // Emit three times; the 3rd call is over budget.
    let prog = read_and_emit_n(3);
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert_eq!(r, RunResult::HostFaulted(HostCode::BudgetExhausted));
    // Exactly two lines emitted — the 3rd, over-budget call produced NO output.
    assert_eq!(ctx.output_lines(), vec!["hello world", "hello world"]);
    assert_eq!(ctx.calls_to("emit"), 2, "over-budget emit must not be serviced");
}

#[test]
fn output_byte_cap_is_never_exceeded() {
    let budget: u64 = 5;
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(echo_fixture());
    ctx.meter.set_byte_budget(budget);
    // "hello world\n" is 12 bytes > 5, so the emit must be refused wholesale.
    let prog = vec![call("readline"), call("emit")];
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert_eq!(r, RunResult::HostFaulted(HostCode::OutputCapExceeded));
    // The failing emit wrote nothing; the cap is never exceeded.
    assert!(ctx.output_bytes().len() as u64 <= budget);
    assert!(ctx.output_bytes().is_empty());
}

#[test]
fn granted_capability_is_reachable() {
    // Positive control: with `emit` granted, the same program emits.
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(echo_fixture());
    let prog = vec![call("readline"), call("emit")];
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert_eq!(r, RunResult::Done(vec![]));
    assert_eq!(ctx.output_utf8(), "hello world\n");
}

#[test]
fn fuel_exhaustion_between_steps_cancels_cleanly() {
    // An agent_loop whose threshold is never reached => non-terminating loop.
    let prog = mtl_host::load_solution(format!(
        "{}/../../bench/tier3/tasks/agent_loop/solution.mtl",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("parse agent_loop");
    let mut fixture = TaskFixture::default();
    fixture.initial_state = 0;
    fixture.done_threshold = i64::MAX; // never done
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(fixture);
    let r = drive(prog, vec![], 200, &mut reg, &mut ctx);
    assert_eq!(r, RunResult::Cancelled);
    // A between-steps cancel leaves no torn effect: agent_loop emits nothing.
    assert!(ctx.output_bytes().is_empty());
}

#[test]
fn each_capability_invocation_consumes_the_call_exactly_once() {
    // Emit three times with unlimited budget: exactly three services, three lines
    // — no double-service on resume (at-most-once).
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(echo_fixture());
    let prog = read_and_emit_n(3);
    let r = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    assert_eq!(r, RunResult::Done(vec![]));
    assert_eq!(ctx.calls_to("readline"), 1);
    assert_eq!(ctx.calls_to("emit"), 3);
    assert_eq!(ctx.output_lines().len(), 3);
}

// Reference the recursion builders so an accidental unused-import regression is
// caught (linrec/sub/int/vdrop are used to sanity-check the build API surface).
#[allow(dead_code)]
fn _api_surface() -> Vec<Word> {
    vec![int(1), linrec(), sub(), vdrop()]
}
