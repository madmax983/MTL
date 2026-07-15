//! The correctness-by-construction invariant, as a machine-checked test.
//!
//! Reloads EVERY record in the committed pilot `dataset.jsonl`, reconstructs its
//! embedded re-runnable contract, and re-runs the response program through the
//! REAL oracle (`mtl_core::interp::run` for tiers 0–2, `task_setup` + `drive`
//! for tier-3). A single row that no longer HALTs==reference / PASSes fails the
//! test. It also asserts every response is in canonical form and its stored
//! SHA-256 matches — so no formatting-variant or stale-hash row can slip in.

use std::path::PathBuf;

use mtl_datagen::canon::canonical_sha;
use mtl_datagen::oracle::check_ok;
use mtl_datagen::sft::{from_jsonl, Check};

fn pilot_jsonl() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("pilot")
        .join("dataset.jsonl")
}

#[test]
fn every_pilot_row_revalidates() {
    let data = std::fs::read_to_string(pilot_jsonl())
        .expect("committed pilot dataset.jsonl must exist (run `gen --out pilot`)");
    let records = from_jsonl(&data);
    assert!(records.len() >= 1000, "pilot too small: {}", records.len());

    let mut failures = Vec::new();
    for (i, r) in records.iter().enumerate() {
        // 1. canonical-form + hash stability.
        match canonical_sha(&r.response) {
            Some((canon, sha)) => {
                if canon != r.response {
                    failures.push(format!("row {i}: response not canonical: {:?}", r.response));
                    continue;
                }
                if sha != r.canonical_sha256 {
                    failures.push(format!("row {i}: canonical_sha256 mismatch"));
                    continue;
                }
            }
            None => {
                failures.push(format!(
                    "row {i}: response does not parse: {:?}",
                    r.response
                ));
                continue;
            }
        }
        // 2. genuine re-run through the oracle against the embedded contract.
        let check: &Check = &r.check;
        if !check_ok(&r.response, check) {
            failures.push(format!(
                "row {i} ({} tier {}): response {:?} FAILED oracle re-validation",
                r.family, r.tier, r.response
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} pilot rows failed re-validation:\n{}",
        failures.len(),
        records.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn pilot_has_repair_and_all_tiers() {
    let data = std::fs::read_to_string(pilot_jsonl()).expect("pilot exists");
    let records = from_jsonl(&data);
    let repair = records.iter().filter(|r| r.kind == "repair").count();
    let frac = repair as f64 / records.len() as f64;
    assert!(
        (0.15..=0.25).contains(&frac),
        "repair fraction {frac:.3} outside the ~20% band"
    );
    for tier in [0u8, 2, 3] {
        assert!(
            records.iter().any(|r| r.tier == tier),
            "pilot missing tier {tier}"
        );
    }
}
