//! Fault-corpus parity: for each of the 5 canonical fault programs, assert that
//! the arena's reified `FaultInfo` is BIT-IDENTICAL to the interpreter's — same
//! `Fault`, same `stack: Vec<Value>`, same `cont: Vec<Word>` (the faulting word is
//! `cont[0]` in both). This pins the reification boundary (`Vm::fault_info` →
//! `interp::FaultInfo`) that the host / LLM-repair workers build on.

mod common;

use common::{conv_word, fault_cases, value_to_word, Case};
use mtl_arena as arena;
use mtl_core::interp as itp;

const FUEL: u64 = 50_000_000;

fn check_fault_parity(case: &Case) -> Result<(), String> {
    let mut full: Vec<itp::Word> = case.init.iter().map(value_to_word).collect();
    full.extend(case.prog.iter().cloned());

    let itp_out = itp::run(itp::Vm::new(full.clone()), FUEL);

    let prog_arena: Vec<arena::ProgWord> = full.iter().map(conv_word).collect();
    let arena_out = arena::run_arena(&prog_arena, FUEL).outcome();

    match (itp_out, arena_out) {
        (itp::Outcome::Fault(fi_itp), arena::Outcome::Fault(fi_arena)) => {
            // Both are `mtl_core::interp::FaultInfo`; compare the whole shape.
            if fi_itp == fi_arena {
                Ok(())
            } else {
                Err(format!(
                    "{}: FaultInfo differs\n  interp: fault={:?}\n          stack={:?}\n          cont={:?}\n  arena:  fault={:?}\n          stack={:?}\n          cont={:?}",
                    case.name,
                    fi_itp.fault, fi_itp.stack, fi_itp.cont,
                    fi_arena.fault, fi_arena.stack, fi_arena.cont,
                ))
            }
        }
        (i, a) => Err(format!(
            "{}: expected both to fault, got\n  interp: {:?}\n  arena:  {:?}",
            case.name, i, a
        )),
    }
}

#[test]
fn fault_info_parity() {
    let cases = fault_cases();
    let total = cases.len();
    assert_eq!(total, 5, "fault corpus size drifted from the documented 5 cases");
    let mut failures = Vec::new();
    for c in &cases {
        if let Err(e) = check_fault_parity(c) {
            failures.push(e);
        }
    }
    println!("fault parity: {}/{} FaultInfos are bit-identical", total - failures.len(), total);
    if !failures.is_empty() {
        panic!(
            "{} / {} arena FaultInfos DIVERGED from the interpreter:\n{}",
            failures.len(),
            total,
            failures.join("\n")
        );
    }
}
