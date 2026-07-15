//! The sealed-set contamination gate, both directions:
//!   * a clean dataset (disjoint from the sealed manifest) passes;
//!   * a planted collision (a dataset item whose canonical hash equals a sealed
//!     entry) is CAUGHT.

use std::path::PathBuf;

use mtl_datagen::contamination::{gate, parse_manifest, Item, SealedEntry};

fn sealed() -> Vec<SealedEntry> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("sealed")
        .join("sealed.manifest.json");
    let data = std::fs::read_to_string(path).expect("sealed manifest exists");
    parse_manifest(&data)
}

#[test]
fn clean_dataset_passes() {
    let sealed = sealed();
    assert!(
        !sealed.is_empty(),
        "placeholder sealed manifest must be populated"
    );
    // Items with hashes that cannot match any sealed entry.
    let items = vec![
        Item {
            canonical_sha256: "0".repeat(64),
            io_hash: "1".repeat(64),
        },
        Item {
            canonical_sha256: "2".repeat(64),
            io_hash: "3".repeat(64),
        },
    ];
    let report = gate(&items, &sealed);
    assert!(
        report.is_clean(),
        "clean dataset flagged: {:?}",
        report.collisions
    );
    assert_eq!(report.sealed_items, sealed.len());
    assert_eq!(report.dataset_items, 2);
}

#[test]
fn planted_canonical_collision_is_caught() {
    let sealed = sealed();
    // Plant an item whose canonical SHA-256 equals a sealed entry's.
    let victim = &sealed[0];
    let items = vec![Item {
        canonical_sha256: victim.canonical_sha256.clone(),
        io_hash: "f".repeat(64),
    }];
    let report = gate(&items, &sealed);
    assert!(
        !report.is_clean(),
        "planted canonical collision was not caught"
    );
    assert!(report
        .collisions
        .iter()
        .any(|c| c.key == "canonical_sha256" && c.value == victim.canonical_sha256));
}

#[test]
fn planted_io_collision_is_caught() {
    let sealed = sealed();
    // Plant an item whose io hash equals a sealed entry's (behavior-vector leak).
    let victim = &sealed[0];
    let items = vec![Item {
        canonical_sha256: "e".repeat(64),
        io_hash: victim.io_hash.clone(),
    }];
    let report = gate(&items, &sealed);
    assert!(!report.is_clean(), "planted io collision was not caught");
    assert!(report
        .collisions
        .iter()
        .any(|c| c.key == "io_hash" && c.value == victim.io_hash));
}

#[test]
fn committed_pilot_report_is_clean() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("pilot")
        .join("contamination_report.json");
    let data = std::fs::read_to_string(path).expect("pilot contamination report exists");
    let v: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(
        v["collisions"].as_array().map(|a| a.len()),
        Some(0),
        "committed pilot has contamination collisions"
    );
}
