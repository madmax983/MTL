//! Reference hosts for the conformance kit: one obviously-conforming host and a
//! family of adversarial hosts that each violate exactly one Invoke-contract
//! obligation (the #32 perturbation discipline, applied to hosts).

use mtl_core::host::{CapabilitySig, Host, HostCode, HostResult};
use mtl_core::interp::{Value, Word};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
struct Decl { consumes: usize, produces: usize }

fn decls_of(sigs: &[CapabilitySig]) -> HashMap<String, Decl> {
    sigs.iter().map(|s| (s.name.clone(), Decl { consumes: s.consumes, produces: s.produces })).collect()
}

/// Apply `f` to the resumed stack, passing any `HostFault` through untouched.
/// Shared by the adversarial hosts that each perturb the `Resume` stack.
fn on_resume(r: HostResult, f: impl FnOnce(&mut Vec<Value>)) -> HostResult {
    match r { HostResult::Resume(mut s) => { f(&mut s); HostResult::Resume(s) } other => other }
}

/// A host that can report how many effects it performed (for the O6 probe).
pub trait ObservableHost: Host { fn effect_count(&self) -> u64; }

/// The obviously-conforming reference host: pops `consumes`, preserves the
/// untouched prefix verbatim, pushes `produces` fresh opaque Int handles.
pub struct ConformingHost { decls: HashMap<String, Decl>, effects: Arc<AtomicU64> }
impl ConformingHost {
    pub fn from_sigs(sigs: &[CapabilitySig]) -> Self { ConformingHost::with_counter(sigs, Arc::new(AtomicU64::new(0))) }
    pub fn with_counter(sigs: &[CapabilitySig], effects: Arc<AtomicU64>) -> Self { ConformingHost { decls: decls_of(sigs), effects } }
}
impl Host for ConformingHost {
    fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult {
        let Some(d) = self.decls.get(name) else { return HostResult::HostFault(HostCode::NotGranted); };
        self.effects.fetch_add(1, Ordering::SeqCst);
        let keep = stack.len().saturating_sub(d.consumes);
        let mut out: Vec<Value> = stack[..keep].to_vec();
        for i in 0..d.produces { out.push(Value::Int(i as i64)); }
        HostResult::Resume(out)
    }
}
impl ObservableHost for ConformingHost { fn effect_count(&self) -> u64 { self.effects.load(Ordering::SeqCst) } }

/// O1 violation: returns one MORE value than declared.
pub struct WrongArityHost(ConformingHost);
impl WrongArityHost { pub fn from_sigs(sigs: &[CapabilitySig]) -> Self { WrongArityHost(ConformingHost::from_sigs(sigs)) } }
impl Host for WrongArityHost {
    fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult {
        on_resume(self.0.service(name, stack), |s| s.push(Value::Int(-1)))
    }
}

/// O2 violation: mutates the untouched prefix (rewrites the stack bottom).
pub struct MutatePrefixHost(ConformingHost);
impl MutatePrefixHost { pub fn from_sigs(sigs: &[CapabilitySig]) -> Self { MutatePrefixHost(ConformingHost::from_sigs(sigs)) } }
impl Host for MutatePrefixHost {
    fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult {
        on_resume(self.0.service(name, stack), |s| { if let Some(v) = s.first_mut() { *v = Value::Int(0xBAD_BEEF); } })
    }
}

/// Leak violation: unbounded stack growth (residual undeclared values). Caught by O1.
pub struct LeakHost(ConformingHost);
impl LeakHost { pub fn from_sigs(sigs: &[CapabilitySig]) -> Self { LeakHost(ConformingHost::from_sigs(sigs)) } }
impl Host for LeakHost {
    fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult {
        on_resume(self.0.service(name, stack), |s| { for i in 0..3 { s.push(Value::Int(1000 + i)); } })
    }
}

/// O4 violation: raises a HostCode NOT in the declared fault set.
pub struct MisdeclareFaultHost { undeclared: HashMap<String, HostCode> }
impl MisdeclareFaultHost {
    pub fn from_sigs(sigs: &[CapabilitySig]) -> Self {
        const ALL: [HostCode; 6] = [HostCode::InputClosed, HostCode::OutputCapExceeded, HostCode::BudgetExhausted, HostCode::ToolError, HostCode::Timeout, HostCode::NotGranted];
        let undeclared = sigs.iter().map(|s| { let code = ALL.iter().copied().find(|c| !s.faults.contains(c)).unwrap_or(HostCode::ToolError); (s.name.clone(), code) }).collect();
        MisdeclareFaultHost { undeclared }
    }
}
impl Host for MisdeclareFaultHost {
    fn service(&mut self, name: &str, _stack: Vec<Value>) -> HostResult {
        HostResult::HostFault(self.undeclared.get(name).copied().unwrap_or(HostCode::ToolError))
    }
}

/// O5 violation: never returns (silent divergence). Sleeps rather than spins.
pub struct DivergeHost;
impl Host for DivergeHost { fn service(&mut self, _name: &str, _stack: Vec<Value>) -> HostResult { loop { std::thread::sleep(Duration::from_millis(25)); } } }

/// O6 violation: performs its observable effect TWICE per service call.
pub struct DoubleServiceHost { inner: ConformingHost, effects: Arc<AtomicU64> }
impl DoubleServiceHost { pub fn with_counter(sigs: &[CapabilitySig], effects: Arc<AtomicU64>) -> Self { DoubleServiceHost { inner: ConformingHost::from_sigs(sigs), effects } } }
impl Host for DoubleServiceHost {
    fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult { self.effects.fetch_add(2, Ordering::SeqCst); self.inner.service(name, stack) }
}
impl ObservableHost for DoubleServiceHost { fn effect_count(&self) -> u64 { self.effects.load(Ordering::SeqCst) } }

/// Strings-leak host: `Value` has no `Str`, so a core-string leak is
/// unrepresentable; the only vector is an extra out-of-shape value (a Quote
/// masquerading as string bytes). Degrades to an O1 violation.
pub struct StringsLeakHost(ConformingHost);
impl StringsLeakHost { pub fn from_sigs(sigs: &[CapabilitySig]) -> Self { StringsLeakHost(ConformingHost::from_sigs(sigs)) } }
impl Host for StringsLeakHost {
    fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult {
        on_resume(self.0.service(name, stack), |s| s.push(Value::Quote(vec![Word::PushInt(104), Word::PushInt(105)])))
    }
}

/// Slow-but-terminating host for the O8 boundary test: burns real wall-clock.
pub struct SlowHost { inner: ConformingHost, delay: Duration }
impl SlowHost { pub fn from_sigs(sigs: &[CapabilitySig], delay: Duration) -> Self { SlowHost { inner: ConformingHost::from_sigs(sigs), delay } } }
impl Host for SlowHost { fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult { std::thread::sleep(self.delay); self.inner.service(name, stack) } }
