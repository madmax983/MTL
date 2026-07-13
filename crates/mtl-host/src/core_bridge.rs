//! ADAPTER SEAM — when `crates/mtl-core` lands `SpecStep::Invoke` + a driver
//! returning `Outcome::Invoke`, replace this peek-based stepping with a match on
//! that outcome; the rest of the crate is unaffected.
//!
//! The v0.4 design (docs/design/v0.4-effects.md §2) is a two-machine split: the
//! pure core suspends at every `Call(name)` and yields an `Invoke` event to the
//! unverified host runner, which services the capability and resumes. That
//! `Outcome::Invoke` variant is being added by a sibling session (branch
//! v04-core) and is NOT merged yet. Rather than wait for it, this module
//! synthesizes the "Invoke event" against *today's* `interp` by PEEKING
//! `vm.cont.first()`: today `exec_step` faults with `UnknownWord` on a `Call`
//! *without consuming it* (the `Call` stays at `cont[0]`), so we can detect the
//! yield point by inspection and hand it to the driver, which consumes the
//! `Call` itself after servicing. This isolates the entire reconciliation with
//! v04-core to this one file.

use mtl_core::interp::{exec_step, Fault, Step, Value, Vm, Word};

/// The event the pure core reaches: it either terminates (halt/fault/fuel) or
/// suspends at a capability `Call` (`Invoke`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    /// The continuation emptied; carries the final stack.
    Halt(Vec<Value>),
    /// A pure-core fault (a real bug in the program).
    Fault(Fault),
    /// The core suspended at `Call(name)` — the yield point. The `Call` is NOT
    /// consumed; the driver consumes it after servicing.
    Invoke { name: String },
    /// Fuel ran out between steps (clean cancel, no partial effect).
    FuelExhausted,
}

/// Advance the pure machine until it halts, faults, exhausts fuel, or reaches a
/// `Call` (the `Invoke` yield point).
///
/// `steps_used` is incremented per pure step consumed and is compared against
/// `fuel_remaining` *between* steps, so fuel exhaustion is always clean
/// (design §7.1). On reaching a `Call`, the machine is left untouched with the
/// `Call` at `cont[0]`.
pub fn next_event(vm: &mut Vm, fuel_remaining: u64, steps_used: &mut u64) -> CoreEvent {
    loop {
        if *steps_used >= fuel_remaining {
            return CoreEvent::FuelExhausted;
        }
        // Peek the head without consuming: a capability Call is the yield point.
        if let Some(Word::Call(name)) = vm.cont.first() {
            return CoreEvent::Invoke { name: name.clone() };
        }
        match exec_step(vm) {
            Step::Next => *steps_used += 1,
            Step::Halt => return CoreEvent::Halt(vm.stack.clone()),
            Step::Fault(f) => return CoreEvent::Fault(f),
        }
    }
}
