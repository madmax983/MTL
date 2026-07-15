//! Executable evidence for boundary obligations O6/O7/O8.

use mtl_core::host::{drive, CapabilitySig, RunResult};
use mtl_core::interp::build::{add, call, int};
use mtl_core::interp::{Value, Vm, Word};
use mtl_host_conformance::{ConformingHost, DoubleServiceHost, ObservableHost, SlowHost};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn one_sig(name: &str, c: usize, p: usize) -> Vec<CapabilitySig> {
    vec![CapabilitySig { name: name.into(), consumes: c, produces: p, faults: vec![] }]
}

#[test]
fn o6_conforming_host_performs_one_effect_per_invoke() {
    let sigs = one_sig("eff", 0, 0);
    let counter = Arc::new(AtomicU64::new(0));
    let mut host = ConformingHost::with_counter(&sigs, counter.clone());
    let r = drive(Vm::new(vec![call("eff")]), 100, &mut host);
    assert_eq!(r, RunResult::Done(vec![]));
    assert_eq!(host.effect_count(), 1, "conforming host must act at-most-once per Invoke");
}

#[test]
fn o6_double_service_host_is_caught_by_effect_count() {
    let sigs = one_sig("eff", 0, 0);
    let counter = Arc::new(AtomicU64::new(0));
    let mut host = DoubleServiceHost::with_counter(&sigs, counter.clone());
    let r = drive(Vm::new(vec![call("eff")]), 100, &mut host);
    assert_eq!(r, RunResult::Done(vec![]));
    assert!(host.effect_count() > 1, "double-service host must record >1 effect for one Invoke (rejected)");
}

#[test]
fn o7_continuation_after_call_executes_proving_cont_opacity() {
    let sigs = one_sig("id", 0, 0);
    let mut host = ConformingHost::from_sigs(&sigs);
    let prog: Vec<Word> = vec![int(20), int(22), call("id"), add()];
    let r = drive(Vm::new(prog), 100, &mut host);
    assert_eq!(r, RunResult::Done(vec![Value::Int(42)]), "cont after Call must survive servicing");
}

#[test]
fn o8_host_wall_clock_is_not_charged_to_core_fuel() {
    let sigs = one_sig("slow", 0, 0);
    let delay = Duration::from_millis(120);
    let mut host = SlowHost::from_sigs(&sigs, delay);
    let prog: Vec<Word> = vec![int(1), int(2), add(), call("slow")];
    let start = Instant::now();
    // Fuel counts in-core Step::Next only: push, push, add = 3 charges. `drive`
    // checks `remaining == 0` at the top of the loop BEFORE each step, so the
    // Call (Step::Invoke, 0 fuel) and the final Halt (0 fuel) each still need the
    // guard to pass with remaining > 0. Three Next steps drain fuel 4→1, leaving
    // remaining == 1 to admit the Invoke and Halt boundaries. fuel == 3 would
    // trip the guard after the 3rd Next and Cancel before the slow host ever
    // runs; 4 is the true minimum for this program (brief's "3" was off by one).
    let r = drive(Vm::new(prog), 4, &mut host);
    let elapsed = start.elapsed();
    assert_eq!(r, RunResult::Done(vec![Value::Int(3)]), "fuel==core-steps must complete despite slow host");
    assert!(elapsed >= delay, "the host genuinely burned wall-clock ({:?})", elapsed);
}
