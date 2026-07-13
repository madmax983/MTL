//! The impure driver loop, living *above* the pure verified core (design §2.3
//! `drive`). It sources `Invoke` events from [`core_bridge::next_event`],
//! enforces the capability grant set and meter, services capabilities, and
//! implements the at-most-once / clean-cancel semantics of design §7.

use mtl_core::interp::{Value, Vm, Word};

use crate::capability::Registry;
use crate::core_bridge::{next_event, CoreEvent};
use crate::host::{HostCtx, HostFault};
use crate::meter::MeterError;

/// The terminal result of a driven run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunResult {
    /// The program halted; carries the final stack.
    Done(Vec<Value>),
    /// A pure-core fault (a real bug in the program).
    Faulted(mtl_core::interp::Fault),
    /// A capability signalled a host fault (incl. Budget/OutputCap exhaustion).
    HostFaulted(HostFault),
    /// A `Call` to a capability NOT in the registry — ungranted, unreachable.
    /// Nothing was executed and no output emitted.
    Refused { capability: String },
    /// Fuel exhausted BETWEEN steps — clean cancel, no partial effect.
    Cancelled,
}

/// Drive `program` (with `initial_stack`) to termination against the grant set
/// `reg` and host context `ctx`, bounded by `fuel` pure-core steps.
///
/// The clean-yield ordering (design §7.1) is enforced per `Invoke`:
///   1. reach `Invoke(name)` — pure, no effect yet, `Call` still at `cont[0]`;
///   2. if `name` is not granted → `Refused` (the `Call` is NOT consumed, no
///      effect happens — an ungranted capability is unreachable);
///   3. charge the call budget → on `BudgetExhausted` return `HostFaulted`
///      WITHOUT consuming the `Call` or running the capability (clean cancel);
///   4. run the capability (it charges output bytes itself, atomically) → on
///      `Err(hf)` return `HostFaulted(hf)` (an `OutputCapExceeded` wrote nothing);
///   5. on success, consume the `Call` exactly once and continue (at-most-once).
pub fn drive(
    program: Vec<Word>,
    initial_stack: Vec<Value>,
    fuel: u64,
    reg: &mut Registry,
    ctx: &mut HostCtx,
) -> RunResult {
    let mut vm = Vm::with_stack(initial_stack, program);
    let mut steps_used: u64 = 0;

    loop {
        match next_event(&mut vm, fuel, &mut steps_used) {
            CoreEvent::Halt(stack) => return RunResult::Done(stack),
            CoreEvent::Fault(f) => return RunResult::Faulted(f),
            CoreEvent::FuelExhausted => return RunResult::Cancelled,
            CoreEvent::Invoke { name } => {
                // (2) grant check — ungranted is unreachable, nothing runs.
                if !reg.contains(&name) {
                    return RunResult::Refused { capability: name };
                }
                // (3) charge the call budget BEFORE any effect.
                match ctx.meter.charge_call(&name) {
                    Ok(()) => {}
                    Err(MeterError::BudgetExhausted) => {
                        return RunResult::HostFaulted(HostFault::BudgetExhausted)
                    }
                    Err(MeterError::OutputCapExceeded) => {
                        // charge_call never raises this, but stay total.
                        return RunResult::HostFaulted(HostFault::OutputCapExceeded)
                    }
                }
                // (4) service the capability. It pops inputs and pushes outputs
                // on vm.stack and charges output bytes itself.
                let pre_len = vm.stack.len();
                let effect = {
                    let cap = reg
                        .get_mut(&name)
                        .expect("contains() checked just above");
                    let e = cap.effect;
                    match (cap.run)(ctx, &mut vm.stack) {
                        Ok(()) => e,
                        Err(hf) => return RunResult::HostFaulted(hf),
                    }
                };
                // Light host-conformance check (design §3.2 clause 1): the stack
                // must have grown by out_arity - in_arity.
                let expected = pre_len as isize - effect.in_arity as isize
                    + effect.out_arity as isize;
                debug_assert_eq!(
                    vm.stack.len() as isize,
                    expected,
                    "capability `{name}` violated its declared stack effect \
                     ({} -- {})",
                    effect.in_arity,
                    effect.out_arity
                );
                // (5) consume the Call exactly once (at-most-once service).
                ctx.record_call(&name);
                debug_assert!(matches!(vm.cont.first(), Some(Word::Call(n)) if *n == name));
                vm.cont.remove(0);
            }
        }
    }
}
