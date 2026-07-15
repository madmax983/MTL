//! The public run entry — routes the impure driver loop to the selected engine.
//! The DEFAULT is the arena backend ([`drive_arena`], wrapping
//! [`mtl_arena::host::arena_drive`]); the reference interpreter
//! ([`drive_interp`], wrapping [`mtl_core::host::drive`]) stays reachable as the
//! differential anchor. Both drivers share the SAME [`HostShim`] seam and are
//! parity-pinned bit-for-bit (`tests/arena_backend.rs`), so the flip is
//! host-observationally invisible.
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

/// The user-facing engine selector, re-exported from `mtl-arena` (default
/// [`Engine::Arena`]). See [`drive_with`].
pub use mtl_arena::Engine;

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
/// Drive on the DEFAULT engine (the arena, [`Engine::Arena`]). Equivalent to
/// `drive_with(Engine::default(), ...)`. The arena refinement obligation is
/// discharged (machine-checked, unconditional), so it is the default execution
/// path; the reference interpreter stays reachable via [`drive_interp`] /
/// `drive_with(Engine::Interp, ...)` as the differential anchor.
pub fn drive(
    program: Vec<Word>,
    initial_stack: Vec<Value>,
    fuel: u64,
    reg: &mut Registry,
    ctx: &mut HostCtx,
) -> RunResult {
    drive_with(Engine::default(), program, initial_stack, fuel, reg, ctx)
}

/// Drive `program` on the explicitly selected [`Engine`]. Both engines share the
/// SAME [`HostShim`] seam (grant → meter → effect → record) and the same global
/// fuel budget, and are parity-pinned bit-for-bit in `tests/arena_backend.rs`.
pub fn drive_with(
    engine: Engine,
    program: Vec<Word>,
    initial_stack: Vec<Value>,
    fuel: u64,
    reg: &mut Registry,
    ctx: &mut HostCtx,
) -> RunResult {
    match engine {
        Engine::Arena => drive_arena(program, initial_stack, fuel, reg, ctx),
        Engine::Interp => drive_interp(program, initial_stack, fuel, reg, ctx),
    }
}

/// Drive on the DEFAULT arena backend ([`mtl_arena::host::arena_drive`]).
///
/// `arena_drive` starts from an empty `VmState`, so a non-empty `initial_stack`
/// is threaded by prepending it to the program as leading pushes
/// ([`mtl_arena::prog_from_interp_with_stack`]) — the same encoding the corpus
/// and differential oracle use. `tier3run` (the sole in-tree caller) always
/// passes an empty initial stack, so for it this is a direct drop-in; the
/// prepend keeps the wrapper fully general.
pub fn drive_arena(
    program: Vec<Word>,
    initial_stack: Vec<Value>,
    fuel: u64,
    reg: &mut Registry,
    ctx: &mut HostCtx,
) -> RunResult {
    let prog = mtl_arena::prog_from_interp_with_stack(&initial_stack, &program);
    let mut shim = HostShim::new(reg, ctx);
    mtl_arena::host::arena_drive(&prog, fuel, &mut shim)
}

/// Drive on the reference interpreter ([`mtl_core::host::drive`]) — the
/// differential anchor, reachable behind an explicit selection. This is the
/// original delegation kept intact: it is the twin the arena is checked against,
/// never deleted.
pub fn drive_interp(
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
