//! The sealed-set contamination gate + the issue-#53 disjointness proofs.
//!
//! Both directions of the gate:
//!   * a clean dataset (disjoint from the sealed manifest) passes;
//!   * a planted collision (canonical-SHA-256 OR io-hash) is CAUGHT.
//!
//! Plus the mechanical held-out proofs the AC requires:
//!   * a training row that reproduces a DEV/held-out task's exact I/O collides
//!     (`planted_dev_task_collision_is_caught`);
//!   * the 15 sealed tasks are hash-disjoint AND id-disjoint from the 10
//!     agent-trial dev tasks (`sealed_disjoint_from_dev`);
//!   * the committed manifest is reproducible from `bench/sealed/tasks.json`
//!     (`manifest_matches_sealed_tasks`).
//!
//! All fast: no dataset generation, just hashing over the committed task JSON.

use std::collections::HashSet;
use std::path::PathBuf;

use mtl_datagen::canon::io_hash;
use mtl_datagen::contamination::{gate, parse_manifest, parse_manifest_full, Item, SealedEntry};
use mtl_datagen::sealed_spec::{
    self, content_sha256, io_vectors_from_python, task_io_hash, SealedTask, SALT,
};

// ---- fixtures -------------------------------------------------------------

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn manifest_path() -> PathBuf {
    crate_dir().join("sealed").join("sealed.manifest.json")
}

fn sealed_entries() -> Vec<SealedEntry> {
    let data = std::fs::read_to_string(manifest_path()).expect("sealed manifest exists");
    parse_manifest(&data)
}

fn read_tasks(rel: &str) -> Vec<SealedTask> {
    // Resolve relative to the crate dir: bench/dataset/.. == bench.
    let path = crate_dir().join("..").join(rel);
    let data =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    sealed_spec::parse_tasks(&data).tasks
}

fn sealed_tasks() -> Vec<SealedTask> {
    read_tasks("sealed/tasks.json")
}

fn dev_tasks() -> Vec<SealedTask> {
    read_tasks("agent-trial/tasks.json")
}

// ---- 1. clean dataset passes ---------------------------------------------

#[test]
fn clean_dataset_passes() {
    let sealed = sealed_entries();
    assert!(!sealed.is_empty(), "sealed manifest must be populated");
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

// ---- 2. planted io collision is caught -----------------------------------

#[test]
fn planted_io_collision_is_caught() {
    let sealed = sealed_entries();
    // Plant an item whose io hash equals a sealed entry's (behavior-vector leak).
    let victim = &sealed[0];
    assert!(!victim.io_hash.is_empty(), "sealed entry has an io_hash");
    let items = vec![Item {
        canonical_sha256: "e".repeat(64),
        io_hash: victim.io_hash.clone(),
    }];
    let report = gate(&items, &sealed);
    assert!(!report.is_clean(), "planted io collision was not caught");
    assert!(
        report
            .collisions
            .iter()
            .any(|c| c.key == "io_hash" && c.value == victim.io_hash),
        "expected io_hash collision, got {:?}",
        report.collisions
    );
}

// ---- 3. planted canonical collision is caught (mechanism, non-empty key) --

#[test]
fn planted_canonical_collision_is_caught() {
    // The committed manifest's canonical_sha256 is empty pre-freeze, so test the
    // mechanism directly with a locally-populated canonical key.
    let sealed = vec![SealedEntry {
        task_id: "seal_probe".into(),
        tier: 0,
        content_sha256: "a".repeat(64),
        io_hash: "b".repeat(64),
        canonical_sha256: "c".repeat(64),
    }];
    let items = vec![Item {
        canonical_sha256: "c".repeat(64),
        io_hash: "f".repeat(64),
    }];
    let report = gate(&items, &sealed);
    assert!(
        !report.is_clean(),
        "planted canonical collision was not caught"
    );
    assert!(
        report
            .collisions
            .iter()
            .any(|c| c.key == "canonical_sha256" && c.value == "c".repeat(64)),
        "expected canonical_sha256 collision, got {:?}",
        report.collisions
    );
}

// ---- 4. empty canonical does not false-match -----------------------------

#[test]
fn empty_canonical_does_not_false_match() {
    // A sealed entry with an empty canonical_sha256 (the pre-freeze state) must
    // not collide with a dataset item that also has an empty canonical_sha256.
    let sealed = vec![SealedEntry {
        task_id: "seal_probe".into(),
        tier: 0,
        content_sha256: "a".repeat(64),
        io_hash: "b".repeat(64),
        canonical_sha256: String::new(),
    }];
    let items = vec![Item {
        canonical_sha256: String::new(),
        io_hash: "d".repeat(64),
    }];
    let report = gate(&items, &sealed);
    assert!(
        report.is_clean(),
        "empty canonical_sha256 false-matched: {:?}",
        report.collisions
    );
}

// ---- 5. HEADLINE: a planted dev-task I/O reproduction is caught -----------

#[test]
fn planted_dev_task_collision_is_caught() {
    // Take a real held-out DEV task (agent-trial `affine`), build its I/O the
    // SAME way sealed entries are built, and prove the gate bites when a
    // training row reproduces that exact I/O behavior.
    let dev = dev_tasks();
    let affine = dev
        .iter()
        .find(|t| t.id == "affine")
        .expect("agent-trial has an `affine` micro task");
    let dev_io = io_hash(&io_vectors_from_python(&affine.python.vectors));

    // The gate is run against a sealed manifest carrying that dev io_hash (as if
    // the dev task were part of the held-out set) and a dataset item that leaks
    // the exact same behavior.
    let sealed = vec![SealedEntry {
        task_id: "affine".into(),
        tier: 0,
        content_sha256: String::new(),
        io_hash: dev_io.clone(),
        canonical_sha256: String::new(),
    }];
    let items = vec![Item {
        canonical_sha256: "1".repeat(64),
        io_hash: dev_io.clone(),
    }];
    let report = gate(&items, &sealed);
    assert!(
        !report.is_clean(),
        "planted dev-task I/O reproduction was not caught"
    );
    assert!(
        report
            .collisions
            .iter()
            .any(|c| c.key == "io_hash" && c.value == dev_io),
        "expected io_hash collision on the dev task, got {:?}",
        report.collisions
    );
}

// ---- 6. sealed set is disjoint from the dev set --------------------------

#[test]
fn sealed_disjoint_from_dev() {
    let sealed = sealed_tasks();
    let dev = dev_tasks();
    assert_eq!(sealed.len(), 15, "expected 15 sealed tasks");
    assert_eq!(dev.len(), 10, "expected 10 agent-trial dev tasks");

    // io_hash sets (same builder for both).
    let sealed_io: HashSet<String> = sealed.iter().map(task_io_hash).collect();
    let dev_io: HashSet<String> = dev.iter().map(task_io_hash).collect();
    assert_eq!(sealed_io.len(), 15, "sealed io_hashes are internally unique");
    assert_eq!(dev_io.len(), 10, "dev io_hashes are internally unique");
    let io_overlap: Vec<&String> = sealed_io.intersection(&dev_io).collect();
    assert!(
        io_overlap.is_empty(),
        "sealed/dev io_hash overlap: {io_overlap:?}"
    );

    // id sets.
    let sealed_ids: HashSet<&str> = sealed.iter().map(|t| t.id.as_str()).collect();
    let dev_ids: HashSet<&str> = dev.iter().map(|t| t.id.as_str()).collect();
    let id_overlap: Vec<&&str> = sealed_ids.intersection(&dev_ids).collect();
    assert!(id_overlap.is_empty(), "sealed/dev id overlap: {id_overlap:?}");
}

// ---- 7. committed manifest is reproducible from the task set -------------

#[test]
fn manifest_matches_sealed_tasks() {
    let manifest = {
        let data = std::fs::read_to_string(manifest_path()).expect("sealed manifest exists");
        parse_manifest_full(&data)
    };
    assert_eq!(manifest.schema, "mtl-sealed-manifest/v2");
    assert_eq!(manifest.salt, SALT);
    assert_eq!(manifest.salt, "mtl-sealed-v1:issue-53");
    assert_eq!(manifest.entries.len(), 15, "expected 15 manifest entries");

    let tasks = sealed_tasks();
    assert_eq!(tasks.len(), manifest.entries.len());

    for (task, entry) in tasks.iter().zip(manifest.entries.iter()) {
        assert_eq!(entry.task_id, task.id, "entry order matches task order");
        assert_eq!(entry.tier, task.tier_num, "tier == tier_num for {}", task.id);
        assert_eq!(
            entry.content_sha256,
            content_sha256(SALT, task),
            "content_sha256 reproduces for {}",
            task.id
        );
        assert_eq!(
            entry.io_hash,
            task_io_hash(task),
            "io_hash reproduces for {}",
            task.id
        );
        assert!(
            entry.canonical_sha256.is_empty(),
            "canonical_sha256 is withheld pre-freeze for {}",
            task.id
        );
    }
}

// ---- 8. committed pilot report is clean against the new manifest ---------

#[test]
fn committed_pilot_report_is_clean() {
    let path = crate_dir().join("pilot").join("contamination_report.json");
    let data = std::fs::read_to_string(path).expect("pilot contamination report exists");
    let v: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(
        v["collisions"].as_array().map(|a| a.len()),
        Some(0),
        "committed pilot has contamination collisions"
    );
}
