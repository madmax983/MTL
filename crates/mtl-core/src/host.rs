//! MTL v0.4 host seam — the two-machine boundary (design `docs/design/v0.4-effects.md`).
//!
//! This is the **unverified** host seam that sits *above* the formally verified
//! pure core ([`crate::interp::run`]). It is part of the trusted computing base
//! (TCB): the core proves P1/P2/P3 and is total up to fuel, but every actual
//! effect (I/O, tools, RNG, resource metering) happens here, in arbitrary Rust
//! that is *assumed* to conform to the host contract (design §3), not proved.
//!
//! Keep this module MINIMAL. The **capability registry + full host runtime live
//! in a sibling crate** built AGAINST this seam; this file is just the seam plus
//! a toy in-test host (see `tests/invoke_host.rs`) proving the drive loop.
//!
//! ## The channel (design §2.3, §2.4)
//!
//! The *only* channel between core and host is the [`crate::interp::Outcome::Invoke`]
//! value: it carries `(name, stack_snapshot, cont)` OUT of the core, and the host
//! returns [`HostResult::Resume`] or [`HostResult::HostFault`] back IN. Host state
//! never enters the core; `cont` is opaque to the host, which hands it back
//! untouched at resume.

use crate::interp::{run, Fault, FaultInfo, Outcome, Value, Vm};

/// Host-side fault codes (design §3.1 / §6). These are raised by the host runner,
/// never by the pure core — grant/deny and resource metering are host decisions
/// (design §2.2: `Error::UnknownWord` no longer arises in-core from `Call`).
///
/// `NotGranted` lets the host reject an ungranted capability host-side: the core
/// yields on *every* `Call` and does not decide whether `name` is granted (§2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostCode {
    /// An input/state capability was invoked after its source was closed.
    InputClosed,
    /// An emit-style capability would exceed the output-byte cap (design §6b).
    OutputCapExceeded,
    /// A per-capability call budget was exhausted (design §6a).
    BudgetExhausted,
    /// A host tool/capability failed internally.
    ToolError,
    /// The host bounded its own service time and gave up (design §7, review §19).
    Timeout,
    /// The invoked capability name is not granted to this run.
    NotGranted,
}

/// The two-machine return value (design §2.3): the host services a capability and
/// either resumes the core with a new stack, or raises a host fault.
///
/// `host_state` stays host-local and never crosses into the core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostResult {
    /// Service succeeded; resume the core with this stack (bottom .. top). The
    /// core re-enters from the suspended `cont`.
    Resume(Vec<Value>),
    /// Service was refused or failed host-side; abort the drive with this code.
    HostFault(HostCode),
}

/// The service seam (design §2.3). A host runner implements this to service the
/// capability named in an [`Outcome::Invoke`]. `stack` is the immutable snapshot
/// carried out of the core (bottom .. top); the implementation returns the stack
/// to resume with, or a [`HostCode`] fault.
pub trait Host {
    /// Service capability `name` against the snapshot `stack`. This is where THE
    /// EFFECT happens (impure). The `cont` is held by [`drive`] and never passed
    /// here — it is opaque to the host.
    fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult;
}

/// A capability signature as DATA (design §3): a name, a declared stack effect
/// (values consumed → produced), and a fault contract (the [`HostCode`]s it may
/// raise). Declarations live host-side — they are not core artifacts. This is the
/// minimal shape the sibling host crate builds its registry from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilitySig {
    /// The capability's host word name (matched against `Outcome::Invoke.name`).
    pub name: String,
    /// Declared stack effect, in: values popped by the capability.
    pub consumes: usize,
    /// Declared stack effect, out: values pushed by the capability.
    pub produces: usize,
    /// Declared fault contract: the host-fault codes this capability may raise.
    pub faults: Vec<HostCode>,
}

/// Terminal result of driving a program to completion across host resumptions
/// (design §2.3 / §7.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunResult {
    /// The program halted; final stack (bottom .. top).
    Done(Vec<Value>),
    /// The pure core faulted (an ordinary in-core fault, e.g. underflow).
    Faulted(Fault),
    /// Fuel ran out BETWEEN steps — a clean cancellation with no partial effect
    /// (the core suspends only at step boundaries; a `Call` yields via `Invoke`,
    /// not `FuelExhausted`).
    Cancelled,
    /// The host refused or failed to service a capability.
    HostFaulted(HostCode),
}

/// The impure drive loop (design §2.3 / §7.1): run the verified pure core to a
/// boundary, and — on [`Outcome::Invoke`] — hand `(name, stack)` to the host,
/// then re-seed a fresh `run` from the returned stack and the suspended `cont`.
///
/// ## Fuel (design §6, Option B)
///
/// `fuel` is a pure in-core step counter. The SAME `fuel` is re-supplied to each
/// fresh `run` across resumptions; host cost is **never** folded into it. The core
/// remains oblivious to host time/budget — those are metered host-side and surface
/// as a [`HostCode`] via [`HostResult::HostFault`].
pub fn drive(mut vm: Vm, fuel: u64, host: &mut dyn Host) -> RunResult {
    loop {
        match run(vm, fuel) {
            Outcome::Halt(stk) => return RunResult::Done(stk),
            Outcome::Fault(FaultInfo { fault, .. }) => return RunResult::Faulted(fault),
            // FuelExhausted is BETWEEN steps: clean cancellation, no partial effect.
            Outcome::FuelExhausted { .. } => return RunResult::Cancelled,
            Outcome::Invoke { name, stack, cont } => match host.service(&name, stack) {
                // Fresh re-entry from cont with the host-returned stack.
                HostResult::Resume(result_stack) => {
                    vm = Vm {
                        stack: result_stack,
                        cont,
                    };
                }
                HostResult::HostFault(code) => return RunResult::HostFaulted(code),
            },
        }
    }
}
