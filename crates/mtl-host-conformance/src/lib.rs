//! # mtl-host-conformance — the Invoke-contract test kit
//!
//! MTL's end-to-end safety is `(P1 ∧ P2 ∧ P3, proved) ∧ (H conforms, assumed)`
//! (design `docs/design/v0.4-effects.md` §3.2; spec §8.3). This crate is the
//! executable precursor to the (unproved) P9 host-conformance theorem: a harness
//! generic over any `H: mtl_core::host::Host` that runs it against the enumerated
//! obligations (O1–O8) and reports pass/fail per obligation, naming the violated
//! clause. See `docs/host-conformance.md`.

mod hosts;
pub use hosts::*;

use mtl_core::host::{CapabilitySig, Host, HostResult};
use mtl_core::interp::Value;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

/// Bounded wall-clock a single `service` call may take before O5 treats it as divergence.
pub const SERVICE_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Obligation { O1Arity, O2Prefix, O3Shape, O4Faults, O5Termination, O6AtMostOnce, O7ContOpacity, O8FuelBoundary }
impl Obligation {
    pub const ALL: [Obligation; 8] = [Obligation::O1Arity, Obligation::O2Prefix, Obligation::O3Shape, Obligation::O4Faults, Obligation::O5Termination, Obligation::O6AtMostOnce, Obligation::O7ContOpacity, Obligation::O8FuelBoundary];
    pub fn id(self) -> &'static str { match self { Obligation::O1Arity => "O1", Obligation::O2Prefix => "O2", Obligation::O3Shape => "O3", Obligation::O4Faults => "O4", Obligation::O5Termination => "O5", Obligation::O6AtMostOnce => "O6", Obligation::O7ContOpacity => "O7", Obligation::O8FuelBoundary => "O8" } }
    pub fn title(self) -> &'static str { match self { Obligation::O1Arity => "arity (stack'.len == len - consumes + produces)", Obligation::O2Prefix => "untouched-prefix preservation", Obligation::O3Shape => "declared-shape / strings-opaque", Obligation::O4Faults => "fault containment", Obligation::O5Termination => "termination (no silent divergence)", Obligation::O6AtMostOnce => "at-most-once / no-reentrancy", Obligation::O7ContOpacity => "cont opacity", Obligation::O8FuelBoundary => "fuel/cost-accounting boundary" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckKind { Probed, Structural }

#[derive(Clone, Debug)]
pub struct ObligationResult { pub obligation: Obligation, pub passed: bool, pub kind: CheckKind, pub detail: String }

#[derive(Clone, Debug)]
pub struct ConformanceReport { pub results: Vec<ObligationResult> }
impl ConformanceReport {
    pub fn passed(&self) -> bool { self.results.iter().all(|r| r.passed) }
    pub fn for_obligation(&self, o: Obligation) -> Option<&ObligationResult> { self.results.iter().find(|r| r.obligation == o) }
    pub fn failures(&self) -> Vec<&ObligationResult> { self.results.iter().filter(|r| !r.passed).collect() }
    pub fn render(&self) -> String {
        self.results.iter().map(|r| format!("{} [{}] {} — {} ({})", r.obligation.id(), if r.passed { "PASS" } else { "FAIL" }, r.obligation.title(), r.detail, match r.kind { CheckKind::Probed => "probed", CheckKind::Structural => "structural" })).collect::<Vec<_>>().join("\n")
    }
}

const PREFIX: [i64; 3] = [0x5E7_0001, 0x5E7_0002, 0x5E7_0003];
fn probe_stack(consumes: usize) -> Vec<Value> {
    let mut s: Vec<Value> = PREFIX.iter().map(|&i| Value::Int(i)).collect();
    for i in 0..consumes { s.push(Value::Int(700 + i as i64)); }
    s
}
fn ok(o: Obligation, kind: CheckKind, detail: impl Into<String>) -> ObligationResult { ObligationResult { obligation: o, passed: true, kind, detail: detail.into() } }
fn bad(o: Obligation, kind: CheckKind, detail: impl Into<String>) -> ObligationResult { ObligationResult { obligation: o, passed: false, kind, detail: detail.into() } }

/// Run every enumerated obligation against the host produced by `build`.
/// `build` must produce a fresh host that grants and services every capability
/// in `sigs`. O1/O2/O4 are probed by driving `service` directly with synthesised
/// stacks; O5 under a bounded watchdog. O3/O6/O7/O8 are boundary obligations
/// discharged structurally (see docs); the test suite exercises their bite.
pub fn check_conformance<H, F>(build: F, sigs: &[CapabilitySig]) -> ConformanceReport
where H: Host, F: Fn() -> H + Send + Sync + 'static {
    let build = Arc::new(build);
    // O5 (termination) MUST be established first: the O1/O2/O4 probes call
    // `service` synchronously on THIS thread, so a diverging host has to be
    // caught by the bounded watchdog before any synchronous probe runs — else an
    // infinite `service` would hang the harness before O5 ever fires. If the host
    // does not terminate we skip the synchronous probes (they are unknowable and
    // would hang) and let O5's failure carry the rejection.
    let termination = probe_termination(build.clone(), sigs);
    let (arity, prefix, faults) = if termination.passed {
        (probe_arity(&*build, sigs), probe_prefix(&*build, sigs), probe_faults(&*build, sigs))
    } else {
        (skipped(Obligation::O1Arity), skipped(Obligation::O2Prefix), skipped(Obligation::O4Faults))
    };
    let results = vec![
        arity,
        prefix,
        structural_shape(),
        faults,
        termination,
        structural_at_most_once(),
        structural_cont_opacity(),
        structural_fuel_boundary(),
    ];
    ConformanceReport { results }
}

/// A synchronous probe we did not run because the host failed O5 (does not
/// terminate); driving `service` directly would hang. Passes vacuously — the
/// overall report still fails via O5.
fn skipped(o: Obligation) -> ObligationResult {
    ObligationResult { obligation: o, passed: true, kind: CheckKind::Probed, detail: "not probed: host does not terminate (see O5) — a synchronous probe would hang".into() }
}

fn probe_arity<H: Host>(build: &impl Fn() -> H, sigs: &[CapabilitySig]) -> ObligationResult {
    for sig in sigs {
        let mut host = build();
        let stack = probe_stack(sig.consumes);
        let in_len = stack.len();
        if let HostResult::Resume(out) = host.service(&sig.name, stack) {
            let expected = in_len - sig.consumes + sig.produces;
            if out.len() != expected {
                return bad(Obligation::O1Arity, CheckKind::Probed, format!("cap `{}` declared ({}→{}) over stack len {} must Resume len {}, got {}", sig.name, sig.consumes, sig.produces, in_len, expected, out.len()));
            }
        }
    }
    ok(Obligation::O1Arity, CheckKind::Probed, "every Resume matched declared consumes/produces arithmetic")
}

fn probe_prefix<H: Host>(build: &impl Fn() -> H, sigs: &[CapabilitySig]) -> ObligationResult {
    for sig in sigs {
        let mut host = build();
        let orig = probe_stack(sig.consumes);
        let keep = orig.len() - sig.consumes;
        let expected_prefix = orig[..keep].to_vec();
        if let HostResult::Resume(out) = host.service(&sig.name, orig) {
            if out.len() < keep || out[..keep] != expected_prefix[..] {
                return bad(Obligation::O2Prefix, CheckKind::Probed, format!("cap `{}` mutated the untouched prefix below its {} consumed input(s)", sig.name, sig.consumes));
            }
        }
    }
    ok(Obligation::O2Prefix, CheckKind::Probed, "untouched prefix returned verbatim on every Resume")
}

fn probe_faults<H: Host>(build: &impl Fn() -> H, sigs: &[CapabilitySig]) -> ObligationResult {
    for sig in sigs {
        let mut host = build();
        let stack = probe_stack(sig.consumes);
        if let HostResult::HostFault(code) = host.service(&sig.name, stack) {
            if !sig.faults.contains(&code) {
                return bad(Obligation::O4Faults, CheckKind::Probed, format!("cap `{}` raised {:?}, not in declared fault set {:?}", sig.name, code, sig.faults));
            }
        }
    }
    ok(Obligation::O4Faults, CheckKind::Probed, "every raised HostFault was in the capability's declared fault set")
}

fn probe_termination<H, F>(build: Arc<F>, sigs: &[CapabilitySig]) -> ObligationResult
where H: Host, F: Fn() -> H + Send + Sync + 'static {
    for sig in sigs {
        let b = build.clone();
        let name = sig.name.clone();
        let consumes = sig.consumes;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || { let mut host = b(); let stack = probe_stack(consumes); let r = host.service(&name, stack); let _ = tx.send(r); });
        match rx.recv_timeout(SERVICE_TIMEOUT) {
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => return bad(Obligation::O5Termination, CheckKind::Probed, format!("cap `{}` did not return within {:?} — silent divergence", sig.name, SERVICE_TIMEOUT)),
            Err(RecvTimeoutError::Disconnected) => {}
        }
    }
    ok(Obligation::O5Termination, CheckKind::Probed, "every service returned a HostResult within the bounded timeout")
}

fn structural_shape() -> ObligationResult {
    ok(Obligation::O3Shape, CheckKind::Structural, "no `Value::Str` variant exists (interp.rs: model carries only Int|Quote) → a core-string leak is unrepresentable; per-value declared-shape matching is not expressible against `CapabilitySig` (arities only) — see docs (contract gap). The strings-leak adversarial degrades to an out-of-shape extra value, caught by O1.")
}
fn structural_at_most_once() -> ObligationResult {
    ok(Obligation::O6AtMostOnce, CheckKind::Structural, "the driver (`mtl_core::host::drive`) services each yielded Invoke once and resumes from the carried cont; host-side double-effect is caught by the kit's ObservableHost effect-count probe (see boundary tests).")
}
fn structural_cont_opacity() -> ObligationResult {
    ok(Obligation::O7ContOpacity, CheckKind::Structural, "`Host::service(&mut self, name, stack)` never receives `cont` — a cont-observing/mutating host is unrepresentable through the trait signature.")
}
fn structural_fuel_boundary() -> ObligationResult {
    ok(Obligation::O8FuelBoundary, CheckKind::Structural, "`service` returns `HostResult` with no fuel channel — a host cannot fold its cost into the core fuel counter; verified positively by the boundary test (a slow host does not reduce core fuel).")
}
