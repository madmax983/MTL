//! The unified verification gate — the REAL oracle.
//!
//! Tiers 0–2: run the candidate on the machine-checked reference interpreter
//! (`mtl_core::interp::run`, FUEL = 100_000) across the family's adversarial
//! input grid and compare each terminal `Outcome` against the reference
//! contract (`Halt(expected)` or `Fault`). Tier-3: build the capability oracle
//! with `mtl_host::caps::task_setup` and drive it with `mtl_host::driver::drive`
//! — PASS iff `Done` and `ctx.output_utf8() == expected_output`. Only PASS/HALT
//! candidates ever enter the dataset, so correctness is by construction.

use mtl_core::interp::{run, Outcome, Value, Vm};
use mtl_host::caps::{task_setup, TaskSetup};
use mtl_host::driver::{drive, RunResult};

use crate::sft::{Check, CheckVector};
use crate::{cell_to_value, value_to_cell, Expected, IoVector, TaskInstance, FUEL};

/// The oracle verdict, carrying a reason on rejection for diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Accept,
    Reject(String),
}

impl Verdict {
    pub fn is_accept(&self) -> bool {
        matches!(self, Verdict::Accept)
    }
}

/// Parse + convert a source string into an executable program, or `None`.
pub fn compile(src: &str) -> Option<Vec<mtl_core::interp::Word>> {
    mtl_syntax::parse(src)
        .ok()
        .map(|p| mtl_host::conv_program(&p))
}

/// Run `src` on `input` through the reference interpreter (tiers 0–2).
pub fn run_on(src: &str, input: &[Value]) -> Option<Outcome> {
    let prog = compile(src)?;
    Some(run(Vm::with_stack(input.to_vec(), prog), FUEL))
}

/// Gate a tier 0–2 candidate against a reference contract over the input grid.
pub fn gate_tier02(src: &str, io: &[IoVector]) -> Verdict {
    let prog = match compile(src) {
        Some(p) => p,
        None => return Verdict::Reject("parse error".into()),
    };
    for (idx, v) in io.iter().enumerate() {
        let outcome = run(Vm::with_stack(v.input.clone(), prog.clone()), FUEL);
        match (&v.expected, &outcome) {
            (Expected::Halt(exp), Outcome::Halt(got)) if got == exp => {}
            (Expected::Fault, Outcome::Fault(_)) => {}
            _ => {
                return Verdict::Reject(format!(
                    "vector[{idx}] input {:?}: want {:?}, got {:?}",
                    v.input, v.expected, outcome
                ))
            }
        }
    }
    Verdict::Accept
}

/// Gate a tier-3 capability candidate via the `task_setup` + `drive` oracle.
pub fn gate_tier3(src: &str, task: &str) -> Verdict {
    let TaskSetup {
        mut reg,
        mut ctx,
        expected_output,
    } = match task_setup(task) {
        Some(t) => t,
        None => return Verdict::Reject(format!("unknown tier-3 task {task}")),
    };
    let prog = match compile(src) {
        Some(p) => p,
        None => return Verdict::Reject("parse error".into()),
    };
    let verdict = drive(prog, vec![], FUEL, &mut reg, &mut ctx);
    match verdict {
        RunResult::Done(_) if ctx.output_utf8() == expected_output => Verdict::Accept,
        RunResult::Done(_) => Verdict::Reject(format!(
            "wrong output: got {:?}, want {:?}",
            ctx.output_utf8(),
            expected_output
        )),
        other => Verdict::Reject(format!("did not finish: {other:?}")),
    }
}

/// The unified gate: dispatch on tier. This is the single admission oracle used
/// by generation AND by the re-validation invariant test.
pub fn gate(inst: &TaskInstance) -> Verdict {
    match &inst.tier3_task {
        Some(task) => gate_tier3(&inst.program, task),
        None => gate_tier02(&inst.program, &inst.io),
    }
}

/// Build the embeddable re-validation [`Check`] contract for a tier 0–2 task.
pub fn check_from_io(io: &[IoVector]) -> Check {
    Check {
        task: None,
        vectors: io
            .iter()
            .map(|v| CheckVector {
                input: v.input.iter().map(value_to_cell).collect(),
                output: match &v.expected {
                    Expected::Halt(stack) => Some(stack.iter().map(value_to_cell).collect()),
                    Expected::Fault => None,
                },
            })
            .collect(),
    }
}

/// Reconstruct the io-contract from an embedded [`Check`] and re-run `response`
/// through the REAL oracle — the invariant the re-validation test asserts over
/// every committed row. Tier-3 goes through the capability oracle by task name.
pub fn check_ok(response: &str, check: &Check) -> bool {
    if let Some(task) = &check.task {
        return gate_tier3(response, task).is_accept();
    }
    let io: Vec<IoVector> = check
        .vectors
        .iter()
        .map(|cv| IoVector {
            input: cv.input.iter().map(cell_to_value).collect(),
            expected: match &cv.output {
                Some(cells) => Expected::Halt(cells.iter().map(cell_to_value).collect()),
                None => Expected::Fault,
            },
        })
        .collect();
    gate_tier02(response, &io).is_accept()
}
