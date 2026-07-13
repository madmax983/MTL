//! The public run entry — a thin wrapper that FULLY DELEGATES the impure driver
//! loop to [`mtl_core::host::drive`] (design §2.3 `drive`, §7 cancellation).
//!
//! ## Reconciled onto `mtl_core::host::drive` (approach (a))
//!
//! This crate no longer hand-rolls a step loop. The verified-adjacent core owns
//! the driver: [`mtl_core::host::drive`] steps the pure core one small step at a
//! time, services each capability `Call` through the [`mtl_core::host::Host`]
//! seam, and enforces the GLOBAL fuel budget (a single cumulative step count
//! across ALL inter-`Invoke` segments — an `Invoke` yield costs no fuel, and
//! exhaustion at a step boundary is a clean [`RunResult::Cancelled`] with no
//! partial effect). That is exactly this crate's design §7 requirement, so a
//! runaway loop that yields a capability every iteration (e.g. Tier-3
//! `agent_loop` with an unreachable `done?` threshold) is now cancelled by the
//! core's budget instead of spinning forever.
//!
//! All this wrapper does is build the initial [`Vm`], wrap this crate's grant set
//! + host context in a [`HostShim`] (the single seam-conformant `Host` impl —
//! grant → metering-before-effect → run → record, see [`crate::core_bridge`]),
//! and hand both to `mtl_core::host::drive`. Fuel, cancellation, and
//! at-most-once servicing all live in the core.

use mtl_core::interp::{Value, Vm, Word};

/// The terminal result of a driven run, re-exported verbatim from the core seam
/// (`Done` | `Faulted` | `Cancelled` | `HostFaulted`). There is NO `Refused`
/// variant: "capability not granted" surfaces as
/// `HostFaulted(HostCode::NotGranted)` with no effect and no output.
pub use mtl_core::host::RunResult;

/// Re-exported so callers can name host-fault codes without depending on
/// `mtl_core::host` directly (`HostFaulted(HostCode::...)`).
pub use mtl_core::host::HostCode;

use crate::capability::Registry;
use crate::core_bridge::HostShim;
use crate::host::HostCtx;

/// Drive `program` (with `initial_stack`) to termination against the grant set
/// `reg` and host context `ctx`, bounded by `fuel` pure-core steps counted
/// GLOBALLY across all `Invoke` resumptions (design §7.1) — the budget is owned
/// and enforced by [`mtl_core::host::drive`].
///
/// The clean-yield ordering (grant → charge budget → run/charge bytes → record)
/// is enforced per capability inside [`HostShim::service`] (the real
/// [`mtl_core::host::Host`] seam). Fuel exhaustion is checked BETWEEN steps by
/// the core, so a cancel never tears a capability effect: `Cancelled` means no
/// partial effect.
///
/// `reg` and `ctx` are borrowed by the shim only for the duration of the drive,
/// so the caller still owns `ctx` afterwards and can read `ctx`'s emitted output
/// and per-capability call counts.
pub fn drive(
    program: Vec<Word>,
    initial_stack: Vec<Value>,
    fuel: u64,
    reg: &mut Registry,
    ctx: &mut HostCtx,
) -> RunResult {
    let vm = Vm::with_stack(initial_stack, program);
    let mut shim = HostShim::new(reg, ctx);
    mtl_core::host::drive(vm, fuel, &mut shim)
}
