//! The oracle gate accepts known-good candidates and rejects known-bad ones,
//! for both a tier-0 (pure interpreter) task and a tier-3 (capability) task.

use mtl_core::interp::Value;
use mtl_datagen::oracle::{gate_tier02, gate_tier3};
use mtl_datagen::{Expected, IoVector};

fn vi(n: i64) -> Value {
    Value::Int(n)
}

/// Affine contract [n] -> [3n + 7] over a few inputs (incl. a negative).
fn affine_io() -> Vec<IoVector> {
    [(0, 7), (1, 10), (5, 22), (-2, 1)]
        .into_iter()
        .map(|(n, out)| IoVector {
            input: vec![vi(n)],
            expected: Expected::Halt(vec![vi(out)]),
        })
        .collect()
}

#[test]
fn tier0_accepts_good_rejects_bad() {
    let io = affine_io();
    // Correct: 3*n + 7.
    assert!(gate_tier02("3*7+", &io).is_accept());
    // Wrong constant: 3*n + 8 -> wrong output.
    assert!(!gate_tier02("3*8+", &io).is_accept());
    // Wrong multiplier: 4*n + 7.
    assert!(!gate_tier02("4*7+", &io).is_accept());
    // Faulting candidate: `+` underflows on a 1-deep stack.
    assert!(!gate_tier02("+", &io).is_accept());
    // Non-parsing candidate is rejected, not panicking.
    assert!(!gate_tier02("3*7+]", &io).is_accept());
}

#[test]
fn tier0_fault_contract_requires_fault() {
    // Contract: divide-by-zero input MUST fault.
    let io = vec![
        IoVector {
            input: vec![vi(6), vi(3)],
            expected: Expected::Halt(vec![vi(2)]),
        },
        IoVector {
            input: vec![vi(6), vi(0)],
            expected: Expected::Fault,
        },
    ];
    // `/` divides and correctly faults on y==0.
    assert!(gate_tier02("/", &io).is_accept());
    // A program that halts (wrong) on the y==0 input is rejected.
    assert!(!gate_tier02("_", &io).is_accept());
}

#[test]
fn tier3_accepts_good_rejects_bad() {
    // echo_line expects "hello world\n".
    assert!(gate_tier3("readline emit", "echo_line").is_accept());
    // Missing the emit -> no output -> reject.
    assert!(!gate_tier3("readline", "echo_line").is_accept());
    // Unknown task -> reject (no panic).
    assert!(!gate_tier3("readline emit", "no_such_task").is_accept());

    // confined_grep is a distinct task with the same seed program shape.
    assert!(gate_tier3("readlines 0[linehit[emit][_]?](_", "confined_grep").is_accept());
    // A wrong program for it (echo) produces the wrong output.
    assert!(!gate_tier3("readline emit", "confined_grep").is_accept());
}

// ---- v0.8 broad-distribution families: every emitted instance must gate ----

#[test]
fn v08_new_families_all_gate() {
    use mtl_datagen::families::family_groups;
    use mtl_datagen::oracle::gate;
    let mut checked = 0usize;
    let mut per_fam: std::collections::BTreeMap<String, usize> = Default::default();
    for group in family_groups(0) {
        for inst in group {
            if inst.family == "bitdigit" || inst.family == "scan" {
                let v = gate(&inst);
                assert!(
                    v.is_accept(),
                    "{} :: {} rejected: {:?}",
                    inst.family,
                    inst.program,
                    v
                );
                *per_fam.entry(inst.family.clone()).or_default() += 1;
                checked += 1;
            }
        }
    }
    // Ensure both new families actually produced instances.
    assert!(
        per_fam.get("bitdigit").copied().unwrap_or(0) >= 8,
        "bitdigit too few"
    );
    assert!(
        per_fam.get("scan").copied().unwrap_or(0) >= 8,
        "scan too few"
    );
    assert!(
        checked >= 16,
        "expected >=16 new-family instances, got {checked}"
    );
}
