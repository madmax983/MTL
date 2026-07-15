//! Sealed-set contamination gate (feeds issue #53).
//!
//! Loads the sealed manifest — the visible no-peek artifact
//! (`bench/dataset/sealed/sealed.manifest.json`, schema
//! `mtl-sealed-manifest/v2`) — and asserts that **no dataset item collides**
//! with any sealed entry by canonical-SHA-256 OR io-behavior hash (design §3
//! "two-key mechanical dedup CI gate"). Emits `contamination_report.json`; the
//! gate is CI-runnable and fails the build on any collision.
//!
//! The manifest is an OBJECT `{schema, salt, freeze_commit, generated_from,
//! entries:[...]}` derived from the real sealed task set at
//! `bench/sealed/tasks.json` (see [`crate::sealed_spec`]). Reference MTL
//! solutions are withheld until the post-freeze unseal, so every entry's
//! `canonical_sha256` is empty pre-freeze; the gate treats an empty
//! `canonical_sha256`/`io_hash` as "no key" and never registers a collision on
//! an empty string.

use serde::{Deserialize, Serialize};

/// One sealed-set entry (a manifest row).
///
/// * `content_sha256` — the salted tamper-evidence fingerprint of the task
///   spec (see [`crate::sealed_spec::content_sha256`]).
/// * `io_hash` — [`crate::canon::io_hash`] over the task's I/O vectors.
/// * `canonical_sha256` — the reference MTL solution's canonical SHA-256, empty
///   until the post-freeze unseal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealedEntry {
    pub task_id: String,
    pub tier: u8,
    #[serde(default)]
    pub content_sha256: String,
    pub io_hash: String,
    #[serde(default)]
    pub canonical_sha256: String,
}

/// The sealed manifest object (schema `mtl-sealed-manifest/v2`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealedManifest {
    pub schema: String,
    pub salt: String,
    #[serde(default)]
    pub freeze_commit: String,
    #[serde(default)]
    pub generated_from: String,
    pub entries: Vec<SealedEntry>,
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

/// Parse the sealed manifest object and return its `entries`. Keeps the legacy
/// signature so `gen::load_sealed` and the gate keep working.
pub fn parse_manifest(data: &str) -> Vec<SealedEntry> {
    parse_manifest_full(data).entries
}

/// Parse the full sealed manifest object (for callers needing the salt /
/// freeze_commit / schema).
pub fn parse_manifest_full(data: &str) -> SealedManifest {
    serde_json::from_str(data).expect("sealed manifest is a valid mtl-sealed-manifest/v2 object")
}

/// Run the contamination gate: collect every collision between the dataset
/// items and the sealed entries (by canonical SHA-256 OR io hash).
///
/// Empty keys never match: a sealed entry whose `canonical_sha256` is empty
/// (the pre-freeze state) or whose `io_hash` is empty is skipped for that key,
/// so an empty-string dataset item can never false-collide.
pub fn gate(items: &[Item], sealed: &[SealedEntry]) -> ContaminationReport {
    use std::collections::HashSet;
    let ds_sha: HashSet<&str> = items.iter().map(|i| i.canonical_sha256.as_str()).collect();
    let ds_io: HashSet<&str> = items.iter().map(|i| i.io_hash.as_str()).collect();

    let mut collisions = Vec::new();
    for e in sealed {
        if !e.canonical_sha256.is_empty() && ds_sha.contains(e.canonical_sha256.as_str()) {
            collisions.push(Collision {
                task_id: e.task_id.clone(),
                key: "canonical_sha256".into(),
                value: e.canonical_sha256.clone(),
            });
        }
        if !e.io_hash.is_empty() && ds_io.contains(e.io_hash.as_str()) {
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
