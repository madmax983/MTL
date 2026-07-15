//! Repair-trace harvesting — `(broken, fault_turn, fixed)` triples.
//!
//! Design §3: pair a near-miss mutation that makes the oracle FAULT with the
//! accepted valid program that fixes it. The captured interpreter fault (kind +
//! stack snapshot + faulting word) is the "error turn"; the valid program is the
//! target. The fault state is REAL — captured from `mtl_core::interp::run`'s
//! `Outcome::Fault(FaultInfo{fault, stack, cont})`, whose `cont[0]` is the
//! faulting word. Traces are balanced across the four core fault kinds
//! (`Underflow`, `TypeMismatch`, `Overflow`, `DivByZero`) — the rarer
//! `Overflow`/`DivByZero` are covered by targeted parametric constructors so no
//! kind is starved.

use mtl_core::interp::{Fault, FaultInfo, Outcome, Value};

use crate::candidates::mutations;
use crate::oracle::run_on;
use crate::{stack_repr, word_repr, Expected, IoVector};

/// One harvested repair trace.
#[derive(Clone, Debug)]
pub struct RepairTrace {
    pub broken: String,
    pub fixed: String,
    pub fault_turn: String,
    pub fault_kind: Fault,
    /// The fixed program's contract (for the io-behavior hash).
    pub io: Vec<IoVector>,
    pub tier: u8,
    pub difficulty: u32,
}

/// Format a captured fault as the "error turn" the model learns to read
/// (mirrors the `mtlrun` FAULT rendering: kind + stack + faulting word).
pub fn fault_turn(fi: &FaultInfo) -> String {
    let next = fi
        .cont
        .first()
        .map(word_repr)
        .unwrap_or_else(|| "<end>".to_string());
    format!(
        "FAULT: {:?}\n  stack: {}\n  next:  {}",
        fi.fault,
        stack_repr(&fi.stack),
        next
    )
}

/// The repair instruction turn: the broken program + its fault, asking for a fix.
pub fn instruction(broken: &str, turn: &str) -> String {
    format!(
        "The following MTL program faulted. Return a corrected MTL program that \
         performs the intended computation.\n\nProgram:\n{broken}\n\nResult:\n{turn}"
    )
}

/// Try to harvest a repair trace from an accepted valid program by mutating it
/// until it FAULTS on `input` (an input on which the valid program halts). If
/// `want` is `Some(kind)`, only accept that fault kind. Returns the first hit.
pub fn from_mutation(
    fixed: &str,
    input: &[Value],
    io: &[IoVector],
    tier: u8,
    difficulty: u32,
    want: Option<Fault>,
) -> Option<RepairTrace> {
    for m in mutations(fixed) {
        if m == fixed {
            continue;
        }
        if let Some(Outcome::Fault(fi)) = run_on(&m, input) {
            if let Some(k) = want {
                if fi.fault != k {
                    continue;
                }
            }
            return Some(RepairTrace {
                broken: m,
                fixed: fixed.to_string(),
                fault_turn: fault_turn(&fi),
                fault_kind: fi.fault,
                io: io.to_vec(),
                tier,
                difficulty,
            });
        }
    }
    None
}

/// A targeted, by-construction repair trace for a specific fault kind, so all
/// four kinds are guaranteed present and balanced. The fixed program halts from
/// an EMPTY stack; the broken program is a one-glyph near-miss that faults.
pub fn targeted(kind: Fault, k: i64) -> Option<RepairTrace> {
    let (broken, fixed, result): (String, String, i64) = match kind {
        // Drop an operand -> Underflow.  fixed: `x y+`  broken: `x+`
        Fault::Underflow => {
            let x = k.rem_euclid(90) + 1;
            let y = (k * 7).rem_euclid(90) + 1;
            (format!("{x}+"), format!("{x} {y}+"), x + y)
        }
        // Zero the divisor -> DivByZero.  fixed: `x y/`  broken: `x 0/`
        Fault::DivByZero => {
            let x = k.rem_euclid(900) + 10;
            let y = k.rem_euclid(9) + 2;
            (format!("{x} 0/"), format!("{x} {y}/"), x / y)
        }
        // Multiply a near-max value by 2 -> Overflow.  fixed: `big 1*`  broken: `big 2*`.
        // Vary `big` across the upper half so `big*2` always overflows, giving many
        // distinct pairs (so Overflow is not starved relative to the other kinds).
        Fault::Overflow => {
            let big = i64::MAX - k.rem_euclid(4_000_000_000).wrapping_mul(1_000_003);
            (format!("{big} 2*"), format!("{big} 1*"), big)
        }
        // Apply a non-quote -> TypeMismatch.  fixed: `[k]!`  broken: `k!`
        Fault::TypeMismatch => {
            let v = k.rem_euclid(1000);
            (format!("{v}!"), format!("[{v}]!"), v)
        }
    };
    // Capture the REAL fault from the broken program (empty initial stack).
    let fi = match run_on(&broken, &[]) {
        Some(Outcome::Fault(fi)) if fi.fault == kind => fi,
        _ => return None,
    };
    // Confirm the fixed program halts to the intended result.
    match run_on(&fixed, &[]) {
        Some(Outcome::Halt(s)) if s == vec![Value::Int(result)] => {}
        _ => return None,
    }
    let io = vec![IoVector {
        input: vec![],
        expected: Expected::Halt(vec![Value::Int(result)]),
    }];
    Some(RepairTrace {
        broken,
        fixed,
        fault_turn: fault_turn(&fi),
        fault_kind: kind,
        io,
        tier: 0,
        difficulty: 1,
    })
}
