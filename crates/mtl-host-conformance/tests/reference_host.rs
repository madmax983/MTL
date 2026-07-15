//! The kit run against real mtl-host machinery: an owned host mirroring
//! `core_bridge::HostShim`'s service path over an owned Registry + HostCtx.

use mtl_core::host::{CapabilitySig, Host, HostCode, HostResult};
use mtl_core::interp::Value;
use mtl_host::capability::{Capability, FaultKind, Registry, StackEffect};
use mtl_host::core_bridge::map_host_fault;
use mtl_host::host::{HostCtx, TaskFixture};
use mtl_host_conformance::check_conformance;

struct OwnedShim { reg: Registry, ctx: HostCtx }
impl Host for OwnedShim {
    fn service(&mut self, name: &str, mut stack: Vec<Value>) -> HostResult {
        if !self.reg.contains(name) { self.ctx.note_denied(name); return HostResult::HostFault(HostCode::NotGranted); }
        if self.ctx.meter.charge_call(name).is_err() { return HostResult::HostFault(HostCode::BudgetExhausted); }
        // Disjoint borrows of separate fields of `self` (reg / ctx).
        let reg = &mut self.reg;
        let ctx = &mut self.ctx;
        let cap = reg.get_mut(name).expect("registered");
        match (cap.run)(ctx, &mut stack) {
            Ok(()) => { ctx.record_call(name); HostResult::Resume(stack) }
            Err(hf) => HostResult::HostFault(map_host_fault(hf)),
        }
    }
}

fn build_host() -> OwnedShim {
    let mut reg = Registry::new();
    reg.register(Capability::new("push1", StackEffect::new(0, 1), vec![], Box::new(|_c: &mut HostCtx, s: &mut Vec<Value>| { s.push(Value::Int(1)); Ok(()) })));
    reg.register(Capability::new("popone", StackEffect::new(1, 0), vec![], Box::new(|_c: &mut HostCtx, s: &mut Vec<Value>| { s.pop(); Ok(()) })));
    reg.register(Capability::new("combine", StackEffect::new(2, 1), vec![], Box::new(|_c: &mut HostCtx, s: &mut Vec<Value>| {
        let b = s.pop().unwrap_or(Value::Int(0)); let a = s.pop().unwrap_or(Value::Int(0));
        let sum = match (a, b) { (Value::Int(x), Value::Int(y)) => x.wrapping_add(y), _ => 0 };
        s.push(Value::Int(sum)); Ok(())
    })));
    // A 4th capability that legitimately raises its DECLARED fault, so O4's
    // pass path is non-vacuous: a 0 call-budget makes `charge_call` refuse with
    // BudgetExhausted on the first (and only) invocation, before the cap runs.
    reg.register(Capability::new("budgeted", StackEffect::new(0, 1), vec![FaultKind::BudgetExhausted], Box::new(|_c: &mut HostCtx, s: &mut Vec<Value>| { s.push(Value::Int(0)); Ok(()) })));
    let mut ctx = HostCtx::new(TaskFixture::default());
    ctx.meter.set_call_budget("budgeted", 0);
    OwnedShim { reg, ctx }
}

fn sigs() -> Vec<CapabilitySig> {
    vec![
        CapabilitySig { name: "push1".into(), consumes: 0, produces: 1, faults: vec![] },
        CapabilitySig { name: "popone".into(), consumes: 1, produces: 0, faults: vec![] },
        CapabilitySig { name: "combine".into(), consumes: 2, produces: 1, faults: vec![] },
        CapabilitySig { name: "budgeted".into(), consumes: 0, produces: 1, faults: vec![HostCode::BudgetExhausted] },
    ]
}

#[test]
fn the_standard_reference_host_passes_every_probed_obligation() {
    let report = check_conformance(build_host, &sigs());
    assert!(report.passed(), "reference host must pass all obligations:\n{}", report.render());
}
