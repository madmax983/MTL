//! Arena host-driven driver — the OPT-IN, behaviourally-identical twin of
//! [`mtl_core::host::drive`] running on the arena backend.
//!
//! ## What this is (and is not)
//!
//! [`arena_drive`] steps the arena engine ([`arena_step`]) one small step at a
//! time and, at each capability `Call`, crosses the two-machine boundary through
//! the **same** [`mtl_core::host::Host`] seam the reference interpreter uses. It
//! REUSES `mtl_core::host`'s [`Host`] trait, [`RunResult`], [`HostResult`], and
//! [`HostCode`] verbatim — it does NOT define parallel copies — so a host
//! capability program run through `arena_drive` is behaviourally interchangeable
//! with the same program run through `mtl_core::host::drive`: same `Invoke`
//! `Vec<Value>` boundary, same `RunResult`/`HostCode`, same global-fuel /
//! cancellation guarantees. The differential-host test (`tests/host_parity.rs`)
//! pins this: `arena_drive` returns the SAME `RunResult` as `host::drive` for the
//! same program + mock host.
//!
//! It is a **separate, explicitly-selected** entry point. It NEVER silently
//! substitutes `mtl_core::host::drive`; a caller opts in by calling
//! `arena_drive` (or, in `mtl-host`, `driver::drive_arena`) by name.
//!
//! ## The Invoke `Vec<Value>` materialize / re-intern seam (design §3.3)
//!
//! `mtl_core::host::drive` presents the operand stack to the host with
//! `core::mem::take(&mut vm.stack)` — a zero-copy move of an owned `Vec<Value>`,
//! because the reference interpreter's stack already IS a `Vec<Value>`. The arena
//! stack is an interned, structurally-shared cons-list, so it cannot be moved out
//! for free; instead the seam **materializes** it:
//!
//!   * OUT (at `Invoke`): [`Vm::reify_stack`] walks the stack cons-list and
//!     produces an owned `Vec<interp::Value>` (`bottom .. top`), recursively
//!     reifying any `Quote` bodies out of the tape. This is a **full stack copy
//!     out**, O(stack depth + total quote size).
//!   * IN (on `Resume`): [`Vm::reintern_stack`] interns the host-returned
//!     `Vec<Value>` back into a FRESH arena stack segment (quote bodies appended
//!     to the tape, call names de-duplicated), O(stack depth + total quote size).
//!
//! So each host crossing costs a full stack copy out **plus** a re-intern back in
//! — O(stack depth) per `Invoke`, versus the interpreter's O(1) move. This is the
//! deliberate, documented price of a backend-agnostic host boundary: the host sees
//! owned reference `Value`s and never an arena handle, so the same `Host` impl
//! drives either backend. The cost is paid once per `Invoke`, never on the hot
//! in-core loop (design §3.3: "O(crossing) once per Invoke, NOT on hot loop").
//!
//! ## Fuel — identical to `interp` / `host::drive`
//!
//! A single decreasing global budget (`remaining`), never reset across `Invoke`s.
//! Each in-core step (`Step::Next`) charges exactly one unit; an `Invoke` yield
//! costs NO fuel (clean boundary — no torn effect). `remaining == 0` at a step
//! boundary → [`RunResult::Cancelled`], exactly as `host::drive`. An endless
//! capability loop is therefore cancelled by the global budget, not hung.

use crate::run::{arena_step, Step};
use crate::types::{to_itp_fault, ProgWord};
use crate::vm::Vm;
use crate::VmState;
use mtl_core::host::{Host, HostResult, RunResult};
use mtl_core::interp as itp;

/// Drive `prog` to termination on the ARENA backend against `host`, mirroring
/// [`mtl_core::host::drive`] outcome-for-outcome.
///
/// This is the arena's opt-in host driver. It compiles `prog` into the arena,
/// then loops [`arena_step`]; at each capability `Call` it materializes the arena
/// operand stack to an owned `Vec<interp::Value>` (the documented cost boundary,
/// see the module docs), hands it to `host.service`, and on
/// [`HostResult::Resume`] re-interns the returned stack into a fresh arena
/// segment and resumes in place from the suspended continuation.
///
/// Outcome mapping (identical to `host::drive`):
///   * `Halt` → [`RunResult::Done`] with the reified final stack;
///   * in-core `Fault` → [`RunResult::Faulted`];
///   * global fuel exhausted at a step boundary → [`RunResult::Cancelled`];
///   * [`HostResult::HostFault`] → [`RunResult::HostFaulted`].
///
/// TOTAL: no `unwrap`/`expect`/`unreachable!`/`panic!` on any path. The only arena
/// failure modes (u32 tape overflow at compile or re-intern time, design §3.4)
/// surface as a clean [`RunResult::Faulted`] with `interp::Fault::Overflow`.
pub fn arena_drive(prog: &[ProgWord], fuel: u64, host: &mut dyn Host) -> RunResult {
    let mut vm = Vm::new();
    let mut st = VmState::initial();

    // Compile the program into the tape. The only failure is u32 tape-address
    // overflow (design §3.4) — unreachable for any realistic program; report it
    // as a clean Overflow fault rather than panicking. (`host::drive` takes an
    // already-built `Vm` so has no compile step; this is the sole arena-only
    // failure mode, and it maps onto the shared `RunResult::Faulted` contract.)
    match vm.compile(prog) {
        Some(pid) => vm.prepend(&mut st, pid),
        None => return RunResult::Faulted(itp::Fault::Overflow),
    }

    // Single decreasing budget over the ENTIRE run; never reset across Invokes
    // (identical accounting to `host::drive`).
    let mut remaining = fuel;
    loop {
        // Exhaustion is checked at the step boundary, before executing the next
        // step: a clean cancellation with no partial effect.
        if remaining == 0 {
            return RunResult::Cancelled;
        }
        match arena_step(&mut vm, &mut st) {
            // An ordinary in-core step: charge exactly one unit of the budget.
            Step::Next => remaining -= 1,
            Step::Halt => return RunResult::Done(vm.reify_stack(st.stack)),
            Step::Fault(f) => return RunResult::Faulted(to_itp_fault(f)),
            // The arena suspended at a `Call`. `arena_step` already consumed the
            // `Call` word (so `st.cont`/`st.cursor` point AFTER it). Materialize
            // the whole operand stack OUT as an owned `Vec<Value>` (the documented
            // O(stack depth) cost boundary), service WITHOUT charging fuel, then
            // re-intern the host-returned stack and resume in place.
            Step::Invoke(name) => {
                let snapshot = vm.reify_stack(st.stack);
                match host.service(&name, snapshot) {
                    HostResult::Resume(result_stack) => match vm.reintern_stack(&result_stack) {
                        Some(ptr) => st.stack = ptr,
                        // Re-interning the resumed stack overflowed the u32 tape
                        // (design §3.4). Not reachable for realistic stacks;
                        // surface as a clean Overflow fault, never a panic.
                        None => return RunResult::Faulted(itp::Fault::Overflow),
                    },
                    HostResult::HostFault(code) => return RunResult::HostFaulted(code),
                }
            }
        }
    }
}
