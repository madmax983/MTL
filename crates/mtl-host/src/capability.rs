//! Capabilities — named host words with a declared stack effect and fault
//! contract (design §3). A capability pops its declared inputs off the MTL
//! stack, does host work (interning/resolving handles, charging the meter for
//! output), and pushes its declared outputs. The [`Registry`] is the grant set:
//! a capability that is not registered is unreachable (the driver refuses it).

use std::collections::HashMap;

use mtl_core::interp::Value;

use crate::host::{HostCtx, HostFault};

/// A declared stack effect `( in_arity -- out_arity )`, used for a light
/// host-conformance check (design §3.2 clause 1): after servicing, the stack
/// must have grown by `out_arity - in_arity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackEffect {
    pub in_arity: usize,
    pub out_arity: usize,
}

impl StackEffect {
    pub const fn new(in_arity: usize, out_arity: usize) -> Self {
        StackEffect { in_arity, out_arity }
    }
}

/// The lightweight discriminant of a [`HostFault`] a capability may raise (its
/// declared fault set, design §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    BudgetExhausted,
    OutputCapExceeded,
    UnknownCapability,
    ToolError,
    InputClosed,
}

/// The host closure implementing a capability: pop inputs, do host work, push
/// outputs; charge the meter for any bytes it emits.
pub type CapFn = Box<dyn FnMut(&mut HostCtx, &mut Vec<Value>) -> Result<(), HostFault>>;

/// A named capability: its declared effect, its declared fault set, and its
/// implementation.
pub struct Capability {
    pub name: String,
    pub effect: StackEffect,
    pub faults: Vec<FaultKind>,
    pub run: CapFn,
}

impl Capability {
    pub fn new(
        name: impl Into<String>,
        effect: StackEffect,
        faults: Vec<FaultKind>,
        run: CapFn,
    ) -> Self {
        Capability {
            name: name.into(),
            effect,
            faults,
            run,
        }
    }
}

/// The capability grant set. Only registered names are reachable.
#[derive(Default)]
pub struct Registry {
    map: HashMap<String, Capability>,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            map: HashMap::new(),
        }
    }

    /// Grant a capability (overwrites any prior grant of the same name).
    pub fn register(&mut self, cap: Capability) {
        self.map.insert(cap.name.clone(), cap);
    }

    /// Whether `name` is granted.
    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// Mutable access to a granted capability (its `run` is `FnMut`).
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Capability> {
        self.map.get_mut(name)
    }

    /// Confine the grant set to exactly the names in `allowed`, dropping every
    /// other capability. This realizes capability confinement: after `retain`,
    /// any `Call` to a removed name is unreachable and faults `NotGranted`.
    pub fn retain(&mut self, allowed: &[&str]) {
        self.map.retain(|k, _| allowed.contains(&k.as_str()));
    }

    /// The number of granted capabilities.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the grant set is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
