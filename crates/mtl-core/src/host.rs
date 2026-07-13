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

use crate::interp::{exec_step, Fault, Step, Value, Vm};

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
    /// The GLOBAL fuel budget was exhausted BETWEEN steps — a clean cancellation
    /// with no partial effect. `fuel` bounds the total in-core steps across the
    /// ENTIRE driven run (all inter-`Invoke` segments summed), so an endless loop
    /// that yields a capability every iteration is still cancelled (it can no
    /// longer defeat metering by resetting the budget at each `Invoke`). The core
    /// suspends only at step boundaries; a `Call` yields via `Invoke` and costs no
    /// fuel, so exhaustion never occurs mid-capability.
    Cancelled,
    /// The host refused or failed to service a capability.
    HostFaulted(HostCode),
}

/// The impure drive loop (design §2.3 / §7.1): step the verified pure core one
/// small step at a time and — when a `Call` yields [`Step::Invoke`] — hand
/// `(name, stack)` to the host, then resume in place from the suspended `cont`
/// with the host-returned stack.
///
/// ## Fuel — a GLOBAL budget across resumptions (design §7)
///
/// `fuel` is a pure in-core step counter that bounds the WHOLE driven run: the
/// total number of in-core steps summed across every inter-`Invoke` segment is
/// `<= fuel`. `drive` owns the step loop and decrements a single `remaining`
/// budget once per in-core step; servicing an `Invoke` does NOT reset it. When
/// `remaining` hits 0 at a step boundary, the run is [`RunResult::Cancelled`].
///
/// This is what makes metering total: a program that yields a capability inside a
/// non-terminating loop (e.g. a tier-3 `agent_loop` whose `done` never trips)
/// hits `Invoke` before any single segment exhausts fuel on every iteration, yet
/// the global budget still runs out — so the loop is cancelled instead of spinning
/// forever. Re-supplying `fuel` per segment (the old behaviour) let such a loop
/// defeat the §7 global-budget guarantee entirely.
///
/// An `Invoke` yield itself costs NO fuel: it is a clean boundary between steps
/// (design §7 — "fuel exhaustion can never occur mid-capability"; exhaustion
/// happens only at step boundaries). Host cost is **never** folded into `fuel`;
/// the core stays oblivious to host time/budget, which are metered host-side and
/// surface as a [`HostCode`] via [`HostResult::HostFault`] (design §6, Option B).
pub fn drive(mut vm: Vm, fuel: u64, host: &mut dyn Host) -> RunResult {
    // Single decreasing budget over the ENTIRE run; never reset across Invokes.
    let mut remaining = fuel;
    loop {
        // Exhaustion is checked at the step boundary, before executing the next
        // step (mirrors `interp::run`'s `steps >= fuel` guard): a clean
        // cancellation with no partial effect.
        if remaining == 0 {
            return RunResult::Cancelled;
        }
        match exec_step(&mut vm) {
            // An ordinary in-core step: charge exactly one unit of the budget.
            Step::Next => remaining -= 1,
            Step::Halt => return RunResult::Done(vm.stack),
            Step::Fault(fault) => return RunResult::Faulted(fault),
            // v0.4 effects: the core suspended at a `Call`. `exec_step` already
            // consumed the `Call` word (so `vm.cont` is the tail after it) and
            // left `vm.stack` as the snapshot. Service the capability WITHOUT
            // charging fuel, then resume in place with the host-returned stack.
            Step::Invoke(name) => {
                let snapshot = core::mem::take(&mut vm.stack);
                match host.service(&name, snapshot) {
                    HostResult::Resume(result_stack) => vm.stack = result_stack,
                    HostResult::HostFault(code) => return RunResult::HostFaulted(code),
                }
            }
        }
    }
}
