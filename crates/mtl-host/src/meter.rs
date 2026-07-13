//! Resource metering — the host-side meter of v0.4 design §6 (Option B: a
//! separate bytes/budget meter alongside the pure core's fuel, debited only at
//! `Invoke` boundaries). Two orthogonal host limits live here:
//!
//!   * **per-capability call budgets** — at most `N` invocations of a named
//!     capability per run;
//!   * **total output-byte cap** — at most `M` bytes emitted across the run.
//!
//! CRUCIAL ATOMICITY: every charge is checked *before* the effect and, on
//! failure, spends nothing and mutates nothing. This is what makes cancellation
//! (§7) leave no partial effect — a refused charge means the capability is never
//! serviced and no bytes are written.

use std::collections::HashMap;

/// Why a meter charge was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterError {
    /// A per-capability call budget hit zero.
    BudgetExhausted,
    /// An output-byte charge would exceed the remaining total byte budget.
    OutputCapExceeded,
}

/// The host-side meter. Both limits default to *unlimited* until explicitly set.
#[derive(Debug, Clone)]
pub struct Meter {
    /// Remaining calls per capability name. A name **absent** from the map is
    /// **unlimited** (never charged). Set an explicit budget with
    /// [`Meter::set_call_budget`].
    call_budget: HashMap<String, u64>,
    /// Remaining total output byte budget. Starts at `u64::MAX` (unlimited).
    bytes_remaining: u64,
}

impl Default for Meter {
    fn default() -> Self {
        Meter {
            call_budget: HashMap::new(),
            bytes_remaining: u64::MAX,
        }
    }
}

impl Meter {
    /// A meter with both limits unlimited.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set (or replace) the remaining call budget for a capability. Once set,
    /// that capability may be invoked at most `n` more times this run.
    pub fn set_call_budget(&mut self, name: impl Into<String>, n: u64) {
        self.call_budget.insert(name.into(), n);
    }

    /// Set the total output-byte budget for the run.
    pub fn set_byte_budget(&mut self, n: u64) {
        self.bytes_remaining = n;
    }

    /// Remaining total output byte budget.
    pub fn bytes_remaining(&self) -> u64 {
        self.bytes_remaining
    }

    /// Charge one invocation of `name`. If the name has an explicit budget and
    /// it is `0`, refuse with [`MeterError::BudgetExhausted`] and spend nothing;
    /// otherwise decrement (or no-op for an unbudgeted name) and return `Ok`.
    pub fn charge_call(&mut self, name: &str) -> Result<(), MeterError> {
        match self.call_budget.get_mut(name) {
            None => Ok(()), // unbudgeted => unlimited
            Some(0) => Err(MeterError::BudgetExhausted),
            Some(remaining) => {
                *remaining -= 1;
                Ok(())
            }
        }
    }

    /// Charge `n` output bytes. ATOMIC: if `n` exceeds the remaining budget,
    /// refuse with [`MeterError::OutputCapExceeded`] and DO NOT mutate; else
    /// subtract and return `Ok`.
    pub fn charge_bytes(&mut self, n: u64) -> Result<(), MeterError> {
        if n > self.bytes_remaining {
            Err(MeterError::OutputCapExceeded)
        } else {
            self.bytes_remaining -= n;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbudgeted_call_is_unlimited() {
        let mut m = Meter::new();
        for _ in 0..1000 {
            assert_eq!(m.charge_call("emit"), Ok(()));
        }
    }

    #[test]
    fn call_budget_counts_down_then_refuses() {
        let mut m = Meter::new();
        m.set_call_budget("emit", 2);
        assert_eq!(m.charge_call("emit"), Ok(()));
        assert_eq!(m.charge_call("emit"), Ok(()));
        assert_eq!(m.charge_call("emit"), Err(MeterError::BudgetExhausted));
        // A different, unbudgeted name is unaffected.
        assert_eq!(m.charge_call("step"), Ok(()));
    }

    #[test]
    fn byte_charge_is_atomic_on_refusal() {
        let mut m = Meter::new();
        m.set_byte_budget(5);
        // Over-budget charge refuses AND spends nothing.
        assert_eq!(m.charge_bytes(6), Err(MeterError::OutputCapExceeded));
        assert_eq!(m.bytes_remaining(), 5);
        // An exact-fit charge succeeds and drains to zero.
        assert_eq!(m.charge_bytes(5), Ok(()));
        assert_eq!(m.bytes_remaining(), 0);
        // Now even a 1-byte charge refuses, still spending nothing.
        assert_eq!(m.charge_bytes(1), Err(MeterError::OutputCapExceeded));
        assert_eq!(m.bytes_remaining(), 0);
    }

    #[test]
    fn zero_byte_charge_always_ok() {
        let mut m = Meter::new();
        m.set_byte_budget(0);
        assert_eq!(m.charge_bytes(0), Ok(()));
    }
}
