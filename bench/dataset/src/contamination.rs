//! Sealed-set contamination gate (feeds issue #53).
//!
//! Loads a sealed manifest — the visible no-peek artifact, a JSON array of
//! `{task_id, tier, canonical_sha256, io_hash}` — and asserts that **no dataset
//! item collides** with any sealed entry by canonical-SHA-256 OR io-behavior
//! hash (design §3 "two-key mechanical dedup CI gate"). Emits
//! `contamination_report.json`; the gate is CI-runnable and fails the build on
//! any collision.
//!
//! The real sealed-eval split is reserved-empty in-repo (see
//! `bench/dataset/sealed/README.md`); this runs against a documented PLACEHOLDER
//! manifest of reserved canonical forms until #53 authors the real sealed tasks.

use serde::{Deserialize, Serialize};

/// One sealed-set entry (the visible manifest row).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealedEntry {
    pub task_id: String,
    pub tier: u8,
    pub canonical_sha256: String,
    pub io_hash: String,
}

/// One dataset item's two dedup keys.
#[derive(Clone, Debug)]
pub struct Item {
    pub canonical_sha256: String,
    pub io_hash: String,
}

/// A detected collision between a dataset item and a sealed entry.
#[derive(Clone, Debug, Serialize)]
pub struct Collision {
    pub task_id: String,
    /// "canonical_sha256" or "io_hash".
    pub key: String,
    pub value: String,
}

/// The serializable contamination report.
#[derive(Clone, Debug, Serialize)]
pub struct ContaminationReport {
    pub dataset_items: usize,
    pub sealed_items: usize,
    pub collisions: Vec<Collision>,
}

impl ContaminationReport {
    pub fn is_clean(&self) -> bool {
        self.collisions.is_empty()
    }
}

/// Parse a sealed manifest (JSON array) from text.
pub fn parse_manifest(data: &str) -> Vec<SealedEntry> {
    serde_json::from_str(data).expect("sealed manifest is a valid JSON array")
}

/// Run the contamination gate: collect every collision between the dataset
/// items and the sealed entries (by canonical SHA-256 OR io hash).
pub fn gate(items: &[Item], sealed: &[SealedEntry]) -> ContaminationReport {
    use std::collections::HashSet;
    let ds_sha: HashSet<&str> = items.iter().map(|i| i.canonical_sha256.as_str()).collect();
    let ds_io: HashSet<&str> = items.iter().map(|i| i.io_hash.as_str()).collect();

    let mut collisions = Vec::new();
    for e in sealed {
        if ds_sha.contains(e.canonical_sha256.as_str()) {
            collisions.push(Collision {
                task_id: e.task_id.clone(),
                key: "canonical_sha256".into(),
                value: e.canonical_sha256.clone(),
            });
        }
        if ds_io.contains(e.io_hash.as_str()) {
            collisions.push(Collision {
                task_id: e.task_id.clone(),
                key: "io_hash".into(),
                value: e.io_hash.clone(),
            });
        }
    }
    ContaminationReport {
        dataset_items: items.len(),
        sealed_items: sealed.len(),
        collisions,
    }
}
