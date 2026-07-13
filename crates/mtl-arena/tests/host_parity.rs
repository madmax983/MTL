//! Host-seam parity tests for the arena driver ([`mtl_arena::host::arena_drive`]).
//!
//! Two obligations, both against the SAME `mtl_core::host::Host` seam the
//! reference interpreter uses:
//!
//!   1. `an_endless_capability_loop_is_cancelled_by_the_global_budget` — the
//!      cancellation test ported verbatim from `mtl-core/tests/invoke_host.rs:205`,
//!      run against `arena_drive`: an endless capability loop must be `Cancelled`
//!      by the single global fuel budget, identically to interp.
//!
//!   2. Differential host test — for a representative subset of the invoke_host
//!      corpus (identity resume, marker Resume round-trip, MULTIPLE Invokes, a
//!      HostFault, and a core fault after a serviced Invoke), assert that
//!      `arena_drive` returns the BIT-IDENTICAL `RunResult` that
//!      `mtl_core::host::drive` returns for the same program + the same mock host.
//!
//! The mock hosts (`EchoHost`, `MarkerHost`, `DenyHost`) are ported from
//! `invoke_host.rs` unchanged — they implement `mtl_core::host::Host`, which the
//! arena driver reuses verbatim (no parallel copy), so a single host impl drives
//! either backend.

mod common;

use common::conv_word;
use mtl_arena::host::arena_drive;
use mtl_core::host::{drive as itp_drive, Host, HostCode, HostResult, RunResult};
use mtl_core::interp::build::*;
use mtl_core::interp::{Value, Vm, Word};

const FUEL: u64 = 10_000;

fn i(n: i64) -> Value {
    Value::Int(n)
}

// ---- toy hosts (ported from invoke_host.rs) -----------------------------

/// Identity/echo host: `service("echo", stack)` resumes with the stack unchanged.
/// Any other name is `NotGranted`. Counts calls so tests can assert servicing.
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

/// Marker host: pushes a sentinel `Int(marker)` onto the resume stack, so a test
/// can observe that the RESUME stack (not just the snapshot) flows back in.
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

/// Ungranted host: refuses everything with `NotGranted`.
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

/// Run `prog_itp` through the arena driver, converting the reference AST into the
/// arena's `ProgWord` input form first.
fn run_arena_host(prog_itp: &[Word], fuel: u64, host: &mut dyn Host) -> RunResult {
    let prog_arena: Vec<mtl_arena::ProgWord> = prog_itp.iter().map(conv_word).collect();
    arena_drive(&prog_arena, fuel, host)
}

// ========================================================================
// 1. GLOBAL FUEL BUDGET — an endless capability loop is cancelled, not hung.
//    Ported from invoke_host.rs:205, run against `arena_drive`.
// ========================================================================

#[test]
fn an_endless_capability_loop_is_cancelled_by_the_global_budget() {
    // A self-reproducing quotation that invokes a capability and then re-runs
    // itself, forever. The toy host ALWAYS Resumes (echo is identity), so only the
    // GLOBAL fuel budget can stop it.
    //
    //   body = [ echo, dup, apply ]  : invoke the cap, duplicate self, re-run self.
    //   prog = [ [body], dup, apply ]: push the self-quote, then launch it.
    let body = vec![call("echo"), dup(), apply()];
    let prog = vec![quote(body), dup(), apply()];

    const BUDGET: u64 = 1_000;

    let mut host = EchoHost::default();
    let result = run_arena_host(&prog, BUDGET, &mut host);

    // The global budget is exhausted at a step boundary -> Cancelled: the arena
    // driver returns promptly instead of spinning, exactly like interp.
    assert_eq!(result, RunResult::Cancelled);

    // The host WAS serviced (the loop genuinely looped) but a BOUNDED number of
    // times: every service is followed by >= 1 fuel-charged in-core step.
    assert!(host.calls > 0, "the endless loop should have invoked the host at least once");
    assert!(
        host.calls as u64 <= BUDGET,
        "services ({}) must be bounded by the global budget ({BUDGET})",
        host.calls
    );
}

// ========================================================================
// 2. DIFFERENTIAL HOST — arena_drive == host::drive for the same host+program.
// ========================================================================

/// A single self-check against the interpreter oracle: `arena_drive` and
/// `host::drive` must return the identical `RunResult` for `prog` given two fresh,
/// identical `host_arena`/`host_itp` instances.
fn assert_same_runresult(
    label: &str,
    prog: &[Word],
    host_arena: &mut dyn Host,
    host_itp: &mut dyn Host,
) {
    let arena_res = run_arena_host(prog, FUEL, host_arena);
    let itp_res = itp_drive(Vm::new(prog.to_vec()), FUEL, host_itp);
    assert_eq!(
        arena_res, itp_res,
        "{label}: arena_drive diverged from host::drive\n  arena: {arena_res:?}\n  interp: {itp_res:?}"
    );
}

#[test]
fn arena_drive_matches_interp_identity_resume() {
    // echo is identity: [5,6] echo + -> [11]. Proves post-Call words run on the
    // re-interned resume stack.
    let prog = vec![int(5), int(6), call("echo"), add()];
    let mut a = EchoHost::default();
    let mut b = EchoHost::default();
    assert_same_runresult("identity_resume", &prog, &mut a, &mut b);
    assert_eq!(a.calls, b.calls);
    // Cross-check the concrete outcome too, not just equality of the two runs.
    let mut c = EchoHost::default();
    assert_eq!(run_arena_host(&prog, FUEL, &mut c), RunResult::Done(vec![i(11)]));
}

#[test]
fn arena_drive_matches_interp_marker_resume_roundtrip() {
    // MarkerHost pushes a sentinel onto the resume stack: [10] emit -> [10,32] ->
    // add -> [42]. This is the Resume `Vec<Value>` round-trip through the arena
    // materialize/re-intern seam — the resume stack must survive re-interning.
    let prog = vec![int(10), call("emit"), add()];
    let mut a = MarkerHost { marker: 32, calls: 0 };
    let mut b = MarkerHost { marker: 32, calls: 0 };
    assert_same_runresult("marker_resume", &prog, &mut a, &mut b);
    let mut c = MarkerHost { marker: 32, calls: 0 };
    assert_eq!(run_arena_host(&prog, FUEL, &mut c), RunResult::Done(vec![i(42)]));
    assert_eq!(c.calls, 1);
}

#[test]
fn arena_drive_matches_interp_multiple_invokes() {
    // Two echo Calls interleaved with arithmetic — MULTI-Invoke Resume round-trip:
    //   3 4 echo +  -> 7 ;  10 echo *  -> 70.
    // Proves the arena driver reseeds the re-interned stack repeatedly.
    let prog = vec![int(3), int(4), call("echo"), add(), int(10), call("echo"), mul()];
    let mut a = EchoHost::default();
    let mut b = EchoHost::default();
    assert_same_runresult("multiple_invokes", &prog, &mut a, &mut b);
    let mut c = EchoHost::default();
    assert_eq!(run_arena_host(&prog, FUEL, &mut c), RunResult::Done(vec![i(70)]));
    assert_eq!(c.calls, 2);
}

#[test]
fn arena_drive_matches_interp_hostfault() {
    // `boom` is ungranted -> HostFaulted(NotGranted); the post-Call `add` NEVER
    // runs. Both backends must surface the identical HostFault.
    let prog = vec![int(1), call("boom"), add()];
    let mut a = DenyHost::default();
    let mut b = DenyHost::default();
    assert_same_runresult("hostfault", &prog, &mut a, &mut b);
    let mut c = DenyHost::default();
    assert_eq!(
        run_arena_host(&prog, FUEL, &mut c),
        RunResult::HostFaulted(HostCode::NotGranted)
    );
    assert_eq!(c.calls, 1);
}

#[test]
fn arena_drive_matches_interp_ungranted_via_echo() {
    // EchoHost grants only `echo`; `read` is NotGranted host-side. Confirms
    // grant/deny is a HOST decision surfaced identically on the arena.
    let prog = vec![int(1), call("read"), add()];
    let mut a = EchoHost::default();
    let mut b = EchoHost::default();
    assert_same_runresult("ungranted_via_echo", &prog, &mut a, &mut b);
}

#[test]
fn arena_drive_matches_interp_core_fault_after_serviced_invoke() {
    // A core fault that arises AFTER a successful host resume: echo (serviced)
    // then `add` on a 1-deep re-interned stack -> Underflow. Both backends must
    // surface Faulted(Underflow), not Done/HostFaulted.
    let prog = vec![int(7), call("echo"), add()];
    let mut a = EchoHost::default();
    let mut b = EchoHost::default();
    assert_same_runresult("core_fault_after_invoke", &prog, &mut a, &mut b);
    let mut c = EchoHost::default();
    assert_eq!(
        run_arena_host(&prog, FUEL, &mut c),
        RunResult::Faulted(mtl_core::interp::Fault::Underflow)
    );
    assert_eq!(c.calls, 1);
}

#[test]
fn arena_drive_matches_interp_core_fault_before_any_invoke() {
    // `add` on a single-element stack underflows in-core BEFORE any Call; the host
    // is never consulted. Both backends surface Faulted(Underflow).
    let prog = vec![int(1), add()];
    let mut a = EchoHost::default();
    let mut b = EchoHost::default();
    assert_same_runresult("core_fault_before_invoke", &prog, &mut a, &mut b);
    let mut c = EchoHost::default();
    assert_eq!(run_arena_host(&prog, FUEL, &mut c), RunResult::Faulted(mtl_core::interp::Fault::Underflow));
    assert_eq!(c.calls, 0, "the host must never be consulted for a pre-Call core fault");
}
