//! The fuel-bounded driver ([`run_arena`]) and the single-step entry point
//! ([`arena_step`]).
//!
//! Fuel semantics are IDENTICAL to `interp::run`: a single decreasing global
//! budget, `fuel` counts executed words (segment pops are free), `steps >= fuel`
//! at a step boundary → [`Outcome::FuelExhausted`], and the budget is never reset
//! mid-run. `fuel == 0` returns `FuelExhausted` immediately.

use crate::arena::VmState;
use crate::types::{Fault, ProgWord};
use crate::vm::{StepR, Vm};
use mtl_core::interp as itp;

/// Result of a single small step. Mirrors `interp::Step`.
///
/// On [`Step::Fault`], [`arena_step`] restores `st` to the **pre-step** position
/// (the faulting word is `reify_cont(st)[0]`), exactly like `interp::exec_step`
/// leaves `vm` holding the pre-step state. On [`Step::Invoke`] and [`Step::Next`]
/// the `Call`/word has been consumed and `st` has advanced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Next,
    Halt,
    Fault(Fault),
    Invoke(String),
}

/// The terminal kind of a driven arena run (lightweight tag; pair with
/// [`ArenaRun::state`] + the [`Vm`] reify helpers, or call [`ArenaRun::outcome`]
/// for the fully reified, reference-typed view).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArenaEnd {
    Halt,
    Fault(Fault),
    FuelExhausted,
    Invoke(String),
}

/// Full result of [`run_arena`]: the arena (kept alive so a host seam can marshal
/// crossing values and resume), the terminal kind, the [`VmState`] to reify from,
/// and the executed-word count.
#[derive(Clone, Debug)]
pub struct ArenaRun {
    pub vm: Vm,
    pub end: ArenaEnd,
    /// The machine position to reify from for this terminal:
    ///   * `Halt` / `FuelExhausted` / `Invoke` → the live post-step position;
    ///   * `Fault` → the restored **pre-step** position (faulting word at
    ///     `reify_cont(state)[0]`, matching `interp`).
    pub state: VmState,
    /// Executed words (segment pops are free) — same counting as `interp::run`.
    pub steps: u64,
}

impl ArenaRun {
    /// Reify this run into the reference-typed [`Outcome`], identical in shape to
    /// `mtl_core::interp::Outcome`. This is the parity view the differential oracle
    /// and the fault corpus compare against `interp::run`.
    pub fn outcome(&self) -> Outcome {
        match &self.end {
            ArenaEnd::Halt => Outcome::Halt(self.vm.reify_stack(self.state.stack)),
            ArenaEnd::Fault(f) => Outcome::Fault(self.vm.fault_info(&self.state, *f)),
            ArenaEnd::FuelExhausted => Outcome::FuelExhausted {
                stack: self.vm.reify_stack(self.state.stack),
                cont: self.vm.reify_cont(&self.state),
            },
            ArenaEnd::Invoke(name) => Outcome::Invoke {
                name: name.clone(),
                stack: self.vm.reify_stack(self.state.stack),
                cont: self.vm.reify_cont(&self.state),
            },
        }
    }
}

/// Terminal outcome of a fuel-bounded [`run_arena`], reified into reference types.
/// Structurally identical to `mtl_core::interp::Outcome` (same payload types), so
/// it is directly comparable to an `interp::run` result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Halt(Vec<itp::Value>),
    Fault(itp::FaultInfo),
    FuelExhausted { stack: Vec<itp::Value>, cont: Vec<itp::Word> },
    Invoke { name: String, stack: Vec<itp::Value>, cont: Vec<itp::Word> },
}

/// Execute exactly one small step, mutating `st` in place.
///
/// TOTAL: no panic sites. Returns [`Step::Halt`] when the continuation is empty.
/// On [`Step::Fault`], `st` is restored to its pre-step value so the faulting word
/// is recoverable at `reify_cont(st)[0]` (mirrors `interp::exec_step`).
pub fn arena_step(vm: &mut Vm, st: &mut VmState) -> Step {
    let saved = *st;
    let w = match vm.next_word(st) {
        Some(w) => w,
        None => return Step::Halt,
    };
    match vm.exec_word(st, w) {
        StepR::Next => Step::Next,
        StepR::Fault(f) => {
            // Leave st holding the pre-step position: the faulting word is the
            // head of the remaining continuation, exactly as interp reports it.
            *st = saved;
            Step::Fault(f)
        }
        StepR::Invoke(name) => Step::Invoke(name),
    }
}

/// Fuel-bounded arena driver. Mirrors `interp::run`: `fuel` counts executed words
/// (segment pops are free), the budget is a single decreasing global count, and
/// every fault is terminal. The returned [`ArenaRun`] keeps the arena alive for
/// reification / host resume; call [`ArenaRun::outcome`] for the reference-typed
/// [`Outcome`].
pub fn run_arena(prog: &[ProgWord], fuel: u64) -> ArenaRun {
    let mut vm = Vm::new();
    let mut st = VmState::initial();
    // Compile can only fail by exhausting the u32 tape address space (design
    // §3.4) — unreachable for any realistic program; report it as a clean
    // Overflow fault rather than panicking.
    match vm.compile(prog) {
        Some(pid) => vm.prepend(&mut st, pid),
        None => {
            return ArenaRun { vm, end: ArenaEnd::Fault(Fault::Overflow), state: st, steps: 0 };
        }
    }

    let mut steps: u64 = 0;
    loop {
        if steps >= fuel {
            return ArenaRun { vm, end: ArenaEnd::FuelExhausted, state: st, steps };
        }
        match arena_step(&mut vm, &mut st) {
            Step::Next => steps += 1,
            Step::Halt => return ArenaRun { vm, end: ArenaEnd::Halt, state: st, steps },
            Step::Fault(f) => {
                // arena_step restored st to the pre-step position.
                return ArenaRun { vm, end: ArenaEnd::Fault(f), state: st, steps };
            }
            Step::Invoke(name) => {
                return ArenaRun { vm, end: ArenaEnd::Invoke(name), state: st, steps };
            }
        }
    }
}
