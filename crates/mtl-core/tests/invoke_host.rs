//! MTL v0.4 host-seam tests (design `docs/design/v0.4-effects.md` §2.3, §7).
//!
//! Proves the two-machine drive loop end-to-end with a TOY in-test host, plus
//! golden `Invoke` cases. The capability registry + full host runtime live in a
//! sibling crate built against `mtl_core::host`; this file is the seam + a toy
//! host that proves the loop closes.
//!
//! Programs are built as ASTs directly (no parser). Capability names use plain
//! alphanumerics (`echo`, `emit`, `boom`) — the lexer treats `-`/`?` as
//! operators, so lexer-safe names are used even though these tests bypass it.

use mtl_core::host::{drive, CapabilitySig, Host, HostCode, HostResult, RunResult};
use mtl_core::interp::build::*;
use mtl_core::interp::{run, Fault, Outcome, Value, Vm};

const FUEL: u64 = 10_000;

fn i(n: i64) -> Value {
    Value::Int(n)
}

// ---- toy hosts ----------------------------------------------------------

/// Identity/echo host: `service("echo", stack)` resumes with the stack unchanged.
/// Any other name is `NotGranted`. Records how many times it was invoked so tests
/// can assert the loop serviced each `Invoke`.
#[derive(Default)]
struct EchoHost {
    calls: usize,
}

impl Host for EchoHost {
    fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult {
        self.calls += 1;
        match name {
            "echo" => HostResult::Resume(stack),
            _ => HostResult::HostFault(HostCode::NotGranted),
        }
    }
}

/// Marker host: `service("emit", stack)` pushes a sentinel `Int(marker)` so the
/// test can observe that the resume stack (not just the snapshot) flows back into
/// the core and post-Call words run against it.
struct MarkerHost {
    marker: i64,
    calls: usize,
}

impl Host for MarkerHost {
    fn service(&mut self, _name: &str, mut stack: Vec<Value>) -> HostResult {
        self.calls += 1;
        stack.push(Value::Int(self.marker));
        HostResult::Resume(stack)
    }
}

/// Ungranted host: refuses everything with `NotGranted`, and counts calls so we
/// can assert the loop stopped at the first refusal (no further core steps).
#[derive(Default)]
struct DenyHost {
    calls: usize,
}

impl Host for DenyHost {
    fn service(&mut self, _name: &str, _stack: Vec<Value>) -> HostResult {
        self.calls += 1;
        HostResult::HostFault(HostCode::NotGranted)
    }
}

// ========================================================================
// 1. GOLDEN — a bound name yields Outcome::Invoke with exact snapshot + cont.
// ========================================================================

#[test]
fn golden_bound_name_yields_invoke() {
    // Program: push 5, push 6, Call echo, then `add` afterwards. One `run(...)`
    // steps the two pushes and suspends AT the Call, carrying the full stack
    // snapshot and the continuation AFTER the consumed Call.
    let prog = vec![int(5), int(6), call("echo"), add()];
    match run(Vm::new(prog), FUEL) {
        Outcome::Invoke { name, stack, cont } => {
            assert_eq!(name, "echo".to_string());
            // Exact stack snapshot: both pushes happened, Call consumed nothing.
            assert_eq!(stack, vec![i(5), i(6)]);
            // Exact continuation: the tail AFTER the Call.
            assert_eq!(cont, vec![add()]);
        }
        other => panic!("expected Invoke, got {:?}", other),
    }
}

// ========================================================================
// 2. RESUME — the drive loop continues past the Call to Done.
// ========================================================================

#[test]
fn drive_resumes_past_call_to_done() {
    // echo is identity, so after servicing, the stack is [5, 6] and the post-Call
    // `add` runs against it -> [11]. Proves post-Call words execute after resume.
    let prog = vec![int(5), int(6), call("echo"), add()];
    let mut host = EchoHost::default();
    let result = drive(Vm::new(prog), FUEL, &mut host);
    assert_eq!(result, RunResult::Done(vec![i(11)]));
    assert_eq!(host.calls, 1);
}

#[test]
fn drive_resume_uses_host_returned_stack() {
    // MarkerHost pushes a sentinel; the post-Call `add` folds it in, proving the
    // RESUME stack (not the pre-Call snapshot) is what the core re-enters with.
    // [10] emit -> host pushes 32 -> [10, 32] -> add -> [42].
    let prog = vec![int(10), call("emit"), add()];
    let mut host = MarkerHost { marker: 32, calls: 0 };
    let result = drive(Vm::new(prog), FUEL, &mut host);
    assert_eq!(result, RunResult::Done(vec![i(42)]));
    assert_eq!(host.calls, 1);
}

// ========================================================================
// 3. HOSTFAULT — an ungranted name surfaces HostFaulted, no partial effect.
// ========================================================================

#[test]
fn drive_hostfault_surfaces_and_halts() {
    // `boom` is ungranted. The loop hands it to the host once, gets NotGranted,
    // and returns HostFaulted immediately — the post-Call `add` NEVER runs.
    let prog = vec![int(1), call("boom"), add()];
    let mut host = DenyHost::default();
    let result = drive(Vm::new(prog), FUEL, &mut host);
    assert_eq!(result, RunResult::HostFaulted(HostCode::NotGranted));
    // Exactly one service attempt; the loop stopped at the first refusal.
    assert_eq!(host.calls, 1);
}

#[test]
fn drive_ungranted_via_echo_host_is_rejected() {
    // EchoHost only grants `echo`; a different name is NotGranted host-side.
    // Confirms grant/deny is a HOST decision (design §2.2), not a core fault.
    let prog = vec![int(1), call("read"), add()];
    let mut host = EchoHost::default();
    let result = drive(Vm::new(prog), FUEL, &mut host);
    assert_eq!(result, RunResult::HostFaulted(HostCode::NotGranted));
}

// ========================================================================
// 4. MULTIPLE INVOKES — the loop reseeds repeatedly and reaches Done.
// ========================================================================

#[test]
fn drive_services_multiple_invokes() {
    // Two echo Calls interleaved with arithmetic:
    //   3 4 echo +   -> 7        (echo identity, then add)
    //   10 echo *    -> 70
    // Both Invokes are serviced by the one loop; reseed works repeatedly.
    let prog = vec![
        int(3),
        int(4),
        call("echo"),
        add(),
        int(10),
        call("echo"),
        mul(),
    ];
    let mut host = EchoHost::default();
    let result = drive(Vm::new(prog), FUEL, &mut host);
    assert_eq!(result, RunResult::Done(vec![i(70)]));
    assert_eq!(host.calls, 2);
}

// ========================================================================
// 5. FAULT PRECEDENCE — a genuine core fault still surfaces as Faulted,
//    not swallowed by the drive loop.
// ========================================================================

#[test]
fn drive_core_fault_surfaces_as_faulted() {
    // `add` on a single-element stack underflows in-core BEFORE any Call. The
    // drive loop must surface RunResult::Faulted(Underflow), not Done/HostFaulted.
    let prog = vec![int(1), add()];
    let mut host = EchoHost::default();
    let result = drive(Vm::new(prog), FUEL, &mut host);
    assert_eq!(result, RunResult::Faulted(Fault::Underflow));
    // The host was never consulted — the fault is a pure-core event.
    assert_eq!(host.calls, 0);
}

#[test]
fn drive_core_fault_after_a_serviced_invoke() {
    // A fault that arises AFTER a successful host resume still surfaces cleanly:
    //   echo (serviced) then `add` on a 1-deep stack -> Underflow.
    let prog = vec![int(7), call("echo"), add()];
    let mut host = EchoHost::default();
    let result = drive(Vm::new(prog), FUEL, &mut host);
    assert_eq!(result, RunResult::Faulted(Fault::Underflow));
    assert_eq!(host.calls, 1);
}

// ========================================================================
// 6. GLOBAL FUEL BUDGET — an endless capability loop is cancelled, not hung.
// ========================================================================

#[test]
fn an_endless_capability_loop_is_cancelled_by_the_global_budget() {
    // Models a tier-3 `agent_loop` whose `done` condition never trips: a
    // self-reproducing quotation that invokes a capability and then re-runs
    // itself, forever. The toy host ALWAYS Resumes (echo is identity), so the
    // loop never terminates on its own — only the GLOBAL fuel budget can stop it.
    //
    //   body = [ echo, dup, apply ]  : invoke the cap, duplicate self, re-run self.
    //   prog = [ [body], dup, apply ]: push the self-quote, then launch it.
    //
    // Each iteration Invokes `echo` (which costs NO fuel) BEFORE the loop's two
    // in-core steps (`dup`, `apply`) run. Under the OLD per-segment fuel (the
    // bug), the core returned `Invoke` before exhausting any single segment on
    // EVERY iteration, so `run` never returned `FuelExhausted` and `drive` spun
    // at 100% CPU forever. Under the GLOBAL budget the summed in-core steps run
    // out and the loop is cancelled at a step boundary.
    let body = vec![call("echo"), dup(), apply()];
    let prog = vec![quote(body), dup(), apply()];

    // A real, finite global budget. It bounds the TOTAL in-core steps across all
    // resumptions, so the loop must terminate deterministically.
    const BUDGET: u64 = 1_000;

    let mut host = EchoHost::default();
    let result = drive(Vm::new(prog), BUDGET, &mut host);

    // The global budget is exhausted at a step boundary -> Cancelled. This is the
    // proof the hang is gone: `drive` returns promptly instead of spinning.
    assert_eq!(result, RunResult::Cancelled);

    // The host WAS serviced (the loop genuinely looped) but a BOUNDED number of
    // times: every service is followed by >= 1 fuel-charged in-core step, so the
    // service count can never exceed the global budget.
    assert!(
        host.calls > 0,
        "the endless loop should have invoked the host at least once"
    );
    assert!(
        host.calls as u64 <= BUDGET,
        "services ({}) must be bounded by the global budget ({BUDGET})",
        host.calls
    );
}

// ========================================================================
// SEAM SMOKE — the seam data types are usable as documented (CapabilitySig).
// ========================================================================

#[test]
fn capability_sig_is_plain_data() {
    // A capability signature is DATA (design §3): name + declared stack effect +
    // fault contract. The sibling host crate builds its registry from these.
    let sig = CapabilitySig {
        name: "emit".to_string(),
        consumes: 1,
        produces: 0,
        faults: vec![HostCode::OutputCapExceeded],
    };
    assert_eq!(sig.name, "emit");
    assert_eq!(sig.consumes, 1);
    assert_eq!(sig.produces, 0);
    assert_eq!(sig.faults, vec![HostCode::OutputCapExceeded]);

    // The full HostCode vocabulary is available to the sibling crate.
    let _all: Vec<HostCode> = vec![
        HostCode::InputClosed,
        HostCode::OutputCapExceeded,
        HostCode::BudgetExhausted,
        HostCode::ToolError,
        HostCode::Timeout,
        HostCode::NotGranted,
    ];
}
