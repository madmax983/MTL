//! Reconciled onto `mtl_core::host::drive` — the host runner IS the core driver;
//! this crate provides only the capability `Host` impl (registry + metering +
//! handles). Fuel/cancellation/at-most-once are owned by the verified-adjacent
//! core (`mtl_core::host`, design `docs/design/v0.4-effects.md` §2, §7).
//!
//! ## Reconciliation approach: (a) — implement `mtl_core::host::Host` and FULLY
//! DELEGATE the driver loop to `mtl_core::host::drive`
//!
//! The pure core suspends at every capability `Call(name)` and yields an
//! [`mtl_core::interp::Step::Invoke`]`(name)` / [`mtl_core::interp::Outcome::Invoke`]
//! `{ name, stack, cont }`. The core ships the impure driver
//! [`mtl_core::host::drive`], which steps the pure core, services one
//! [`mtl_core::host::Host`] call per `Invoke`, and owns the GLOBAL fuel budget
//! (a single cumulative step count across ALL inter-`Invoke` segments; an
//! `Invoke` yield costs no fuel; exhaustion at a step boundary ⇒
//! [`mtl_core::host::RunResult::Cancelled`]). That global budget is exactly this
//! crate's design §7 requirement, so we no longer keep a hand-rolled loop: the
//! public entry ([`crate::driver::drive`]) constructs a `Vm` + a [`HostShim`] and
//! calls `mtl_core::host::drive` directly.
//!
//! This module is now purely the SERVICING seam: [`HostShim`] implements
//! [`mtl_core::host::Host`] over this crate's grant set ([`Registry`]) and impure
//! context ([`HostCtx`]). One `service` call performs exactly one capability
//! service with metering-before-effect ordering (design §3, §6, §7):
//!
//!   1. **grant check** — `name` not in the registry ⇒ `HostFault(NotGranted)`
//!      with NO effect and NO output (an ungranted capability is unreachable).
//!      The pure core yields on EVERY `Call` and never decides grant/deny
//!      (design §2.2); the host shim owns the NotGranted path.
//!   2. **charge the call budget BEFORE any effect** — `BudgetExhausted` ⇒
//!      `HostFault(BudgetExhausted)` without running the capability (clean
//!      cancel, no partial effect).
//!   3. **run the capability** — it pops inputs, charges output bytes itself
//!      (atomic), and pushes outputs; a failure ⇒ `HostFault(<mapped code>)`
//!      having written nothing (e.g. an `OutputCapExceeded` emitted no bytes).
//!   4. on success, **record the service** and `Resume` with the new stack. The
//!      `Call` is consumed exactly once by the core at the yield point, so the
//!      host never re-drives a serviced `Call` (at-most-once).
//!
//! The fault taxonomy is mapped onto the core's [`HostCode`] here: this crate's
//! richer [`HostFault`] (which carries strings) is the internal capability
//! contract, and [`map_host_fault`] projects it onto the core's flat code.

use mtl_core::host::{Host, HostCode, HostResult};
use mtl_core::interp::Value;

use crate::capability::Registry;
use crate::host::{HostCtx, HostFault};
use crate::meter::MeterError;

/// Project this crate's internal [`HostFault`] onto the core seam's flat
/// [`HostCode`] (design §3.1 / §6). The string payloads on `ToolError` /
/// `UnknownCapability` are host-local diagnostics that do not cross the seam.
///
/// * `UnknownCapability` ⇒ `NotGranted` — the seam has no separate "unknown"
///   code; an unserviceable name is, semantically, one that was not granted.
///   (In practice the grant check in [`HostShim::service`] short-circuits this
///   before any capability runs, so it is only a total-function fallback.)
pub fn map_host_fault(hf: HostFault) -> HostCode {
    match hf {
        HostFault::BudgetExhausted => HostCode::BudgetExhausted,
        HostFault::OutputCapExceeded => HostCode::OutputCapExceeded,
        HostFault::InputClosed => HostCode::InputClosed,
        HostFault::ToolError(_) => HostCode::ToolError,
        HostFault::UnknownCapability(_) => HostCode::NotGranted,
    }
}

/// The host runner as a [`mtl_core::host::Host`]: a thin shim borrowing this
/// crate's grant set ([`Registry`]) and impure context ([`HostCtx`]). One
/// `service` call performs exactly one capability service, enforcing grant,
/// metering, and the declared stack effect — see the module docs for the
/// ordering it guarantees.
///
/// The shim borrows `ctx` (rather than owning it) so the caller still owns the
/// [`HostCtx`] after [`mtl_core::host::drive`] returns and can read the emitted
/// output / per-capability call counts (the tests rely on this).
pub struct HostShim<'a> {
    /// The capability grant set. A name absent here is unreachable.
    pub reg: &'a mut Registry,
    /// The impure host context (handles, meter, output, fixtures, call log).
    pub ctx: &'a mut HostCtx,
}

impl<'a> HostShim<'a> {
    /// Wrap a registry + context for one drive.
    pub fn new(reg: &'a mut Registry, ctx: &'a mut HostCtx) -> Self {
        HostShim { reg, ctx }
    }
}

impl Host for HostShim<'_> {
    fn service(&mut self, name: &str, mut stack: Vec<Value>) -> HostResult {
        // (1) grant check — an ungranted capability is unreachable: no effect,
        // no output. The host shim owns the NotGranted path (design §2.2).
        if !self.reg.contains(name) {
            // Record which capability the (confined) program illegally reached
            // for, so the oracle can report `NotGranted <name>`.
            self.ctx.note_denied(name);
            return HostResult::HostFault(HostCode::NotGranted);
        }

        // (2) charge the call budget BEFORE any effect. On refusal nothing is
        // run and nothing is written (clean cancel, no partial effect).
        match self.ctx.meter.charge_call(name) {
            Ok(()) => {}
            Err(MeterError::BudgetExhausted) => {
                return HostResult::HostFault(HostCode::BudgetExhausted)
            }
            Err(MeterError::OutputCapExceeded) => {
                // charge_call never raises this, but stay total.
                return HostResult::HostFault(HostCode::OutputCapExceeded)
            }
        }

        // (3) service the capability. It pops inputs and pushes outputs on the
        // snapshot stack and charges output bytes itself (atomic).
        let cap = self
            .reg
            .get_mut(name)
            .expect("contains() checked just above");
        let effect = cap.effect;
        let pre_len = stack.len();
        if let Err(hf) = (cap.run)(self.ctx, &mut stack) {
            return HostResult::HostFault(map_host_fault(hf));
        }

        // Light host-conformance check (design §3.2 clause 1): the stack must
        // have grown by out_arity - in_arity.
        let expected = pre_len as isize - effect.in_arity as isize + effect.out_arity as isize;
        debug_assert_eq!(
            stack.len() as isize,
            expected,
            "capability `{name}` violated its declared stack effect ({} -- {})",
            effect.in_arity,
            effect.out_arity
        );

        // (4) record the service (at-most-once: the core consumed the Call at
        // the yield point, so the host never re-drives a serviced Call).
        self.ctx.record_call(name);
        HostResult::Resume(stack)
    }
}
