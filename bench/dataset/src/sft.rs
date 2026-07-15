//! SFT record types + JSONL serialization.
//!
//! ## Qwen2.5-Coder chat-template assumption
//!
//! The warm arm is a LoRA over `Qwen/Qwen2.5-Coder-7B-Instruct` (issue #83).
//! These records are the *content* of SFT turns, NOT pre-rendered text: at train
//! time each `instruction` becomes a `user` turn and each `response` an
//! `assistant` turn under Qwen2.5-Coder's **native chat template**, with
//! **completion-only loss** (loss on the assistant span only). Per design §2 the
//! MTL quickref / language spec is **NOT** injected into the training prompt —
//! the model internalizes MTL from the response distribution, emitting MTL with
//! zero preamble tokens at inference. The `instruction` therefore carries only
//! the task (English for named families, an I/O-example spec for
//! enumerated/discovered programs, or the broken program + fault turn for
//! repair). Downstream tooling renders the chat template; this crate stays
//! tokenizer-agnostic and only emits the raw pairs plus provenance metadata.

use serde::{Deserialize, Serialize};

/// One re-runnable reference vector: `input` cells and the required `output`
/// (`None` means the run must Fault on this input).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckVector {
    pub input: Vec<serde_json::Value>,
    pub output: Option<Vec<serde_json::Value>>,
}

/// The re-validation contract embedded in every record so the invariant test can
/// genuinely reload and re-run each program through the oracle. Tier-3 records
/// carry the capability `task` name; tiers 0–2 carry the io `vectors`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Check {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(default)]
    pub vectors: Vec<CheckVector>,
}

/// Whether this record teaches generation or self-repair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Gen,
    Repair,
}

/// One SFT record. `instruction`/`response` are the two chat turns; the
/// remaining fields are provenance for the coverage / contamination / curriculum
/// tooling (they are metadata, not part of the training prompt).
#[derive(Clone, Debug, Serialize)]
pub struct Record {
    pub instruction: String,
    pub response: String,
    pub tier: u8,
    pub family: String,
    pub difficulty: u32,
    pub kind: Kind,
    pub canonical_sha256: String,
    /// io-behavior-vector hash of the underlying task (dedup / contamination).
    pub io_sha256: String,
    /// Re-runnable reference contract (provenance, not part of the chat turns).
    pub check: Check,
}

/// Serialize a slice of records to JSONL (one compact JSON object per line).
pub fn to_jsonl(records: &[Record]) -> String {
    let mut s = String::new();
    for r in records {
        s.push_str(&serde_json::to_string(r).expect("record serializes"));
        s.push('\n');
    }
    s
}

/// Parse a JSONL string back into records (used by the re-validation test).
pub fn from_jsonl(data: &str) -> Vec<RecordOwned> {
    data.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<RecordOwned>(l).expect("valid record line"))
        .collect()
}

/// A deserialize-side mirror of [`Record`] (owned), for reloading the pilot.
#[derive(Clone, Debug, Deserialize)]
pub struct RecordOwned {
    pub instruction: String,
    pub response: String,
    pub tier: u8,
    pub family: String,
    pub difficulty: u32,
    pub kind: String,
    pub canonical_sha256: String,
    pub io_sha256: String,
    pub check: Check,
}
