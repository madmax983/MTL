//! Sealed task set → manifest derivation (issue #53).
//!
//! Reads the real sealed task set at `bench/sealed/tasks.json` (the source of
//! truth: 15 tasks authored blind to the MTL glyph/primitive set — see
//! `bench/sealed/AUTHORSHIP.md`) and derives the salted contamination manifest
//! consumed by [`crate::contamination`]. Every derivation here is deterministic
//! — no randomness, no clock — so a third party can recompute the committed
//! manifest byte-for-byte with `cargo run -p mtl-datagen --bin mkseal`.
//!
//! Three hashes per task:
//! * `content_sha256` — the SALTED tamper-evidence fingerprint of the task spec
//!   (see [`content_sha256`]).
//! * `io_hash` — [`crate::canon::io_hash`] over I/O vectors built from the
//!   task's `python.vectors` (the SAME hash the live dataset gate uses, so a
//!   training row reproducing a sealed task's exact I/O collides).
//! * `canonical_sha256` — the reference MTL solution's canonical SHA-256, left
//!   empty until the post-freeze unseal.

use serde::{Deserialize, Serialize};

use mtl_core::interp::Value;

use crate::canon::{io_hash, sha256_hex};
use crate::contamination::{SealedEntry, SealedManifest};
use crate::{int_list, Expected, IoVector};

/// The FIXED, documented salt for the sealed manifest (schema
/// `mtl-sealed-manifest/v2`). Reproducibility depends on this being constant.
pub const SALT: &str = "mtl-sealed-v1:issue-53";

/// The manifest schema tag.
pub const SCHEMA: &str = "mtl-sealed-manifest/v2";

/// Top-level `bench/sealed/tasks.json` shape.
#[derive(Clone, Debug, Deserialize)]
pub struct SealedTaskFile {
    pub set: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub split: String,
    #[serde(default)]
    pub note: String,
    pub tasks: Vec<SealedTask>,
}

/// One task. Shared shape for the sealed set and the agent-trial dev set: the
/// dev file omits `tier`/`tier_num`/`category`/`signature`, so those default.
#[derive(Clone, Debug, Deserialize)]
pub struct SealedTask {
    pub id: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub tier_num: u8,
    #[serde(default)]
    pub category: String,
    pub arm_common_desc: String,
    #[serde(default)]
    pub signature: String,
    pub mtl: MtlPart,
    pub python: PyPart,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MtlPart {
    pub vectors: Vec<MtlVector>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MtlVector {
    pub input_prefix: String,
    pub expected_halt: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PyPart {
    pub signature: String,
    pub vectors: Vec<PyVector>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PyVector {
    pub args: Vec<serde_json::Value>,
    pub expected: serde_json::Value,
}

/// Parse `bench/sealed/tasks.json` (or the agent-trial file — the shared task
/// shape is a superset that deserializes either).
pub fn parse_tasks(data: &str) -> SealedTaskFile {
    serde_json::from_str(data).expect("sealed tasks.json is a valid task file")
}

/// Convert one JSON "cell" (an int, or a flat array of ints for a list) into an
/// MTL [`Value`]: ints become `Value::Int`, lists become a `Value::Quote` of
/// int literals (an MTL list). Panics on any other shape.
pub fn cell_to_value(cell: &serde_json::Value) -> Value {
    if let Some(n) = cell.as_i64() {
        Value::Int(n)
    } else if let Some(arr) = cell.as_array() {
        let ints: Vec<i64> = arr
            .iter()
            .map(|x| {
                x.as_i64()
                    .expect("sealed/dev list elements are integers")
            })
            .collect();
        int_list(&ints)
    } else {
        panic!("unsupported JSON cell (not int or int-list): {cell}");
    }
}

/// Build [`IoVector`]s from a task's `python.vectors`: each `args` entry becomes
/// an input [`Value`]; `expected` becomes a single output value wrapped in
/// `Expected::Halt`. This is the SAME builder used for both the sealed manifest
/// and the disjointness / planted-collision proofs, so the resulting
/// [`crate::canon::io_hash`] is comparable across sets.
pub fn io_vectors_from_python(vectors: &[PyVector]) -> Vec<IoVector> {
    vectors
        .iter()
        .map(|v| {
            let input: Vec<Value> = v.args.iter().map(cell_to_value).collect();
            let out = cell_to_value(&v.expected);
            IoVector {
                input,
                expected: Expected::Halt(vec![out]),
            }
        })
        .collect()
}

/// The io-behavior-vector hash for one task, over its `python.vectors`.
pub fn task_io_hash(task: &SealedTask) -> String {
    io_hash(&io_vectors_from_python(&task.python.vectors))
}

/// The deterministic canonical spec struct that the salted `content_sha256` is
/// computed over. Field order + serde shape are load-bearing (the hash is over
/// its `serde_json::to_vec` bytes).
#[derive(Serialize)]
struct CanonicalSpec<'a> {
    task_id: &'a str,
    tier: u8,
    prompt: &'a str,
    signature: &'a str,
    vectors: Vec<String>,
}

/// The SALTED tamper-evidence fingerprint of a task spec:
///
/// ```text
/// content_sha256 = sha256_hex( salt.as_bytes() ++ [0x00] ++ canonical_spec_bytes )
/// ```
///
/// where `canonical_spec_bytes = serde_json::to_vec(&{ task_id, tier,
/// prompt: arm_common_desc, signature, vectors })` and `vectors` is the
/// lexicographically-sorted `Vec<String>` of `"{input_prefix}=>{expected_halt}"`
/// over the task's MTL vectors. A third party recomputes it from `tasks.json`
/// alone — no reference solution required.
pub fn content_sha256(salt: &str, task: &SealedTask) -> String {
    let mut vectors: Vec<String> = task
        .mtl
        .vectors
        .iter()
        .map(|v| format!("{}=>{}", v.input_prefix, v.expected_halt))
        .collect();
    vectors.sort();
    let spec = CanonicalSpec {
        task_id: &task.id,
        tier: task.tier_num,
        prompt: &task.arm_common_desc,
        signature: &task.signature,
        vectors,
    };
    let spec_bytes = serde_json::to_vec(&spec).expect("canonical spec serializes");
    let mut buf = Vec::with_capacity(salt.len() + 1 + spec_bytes.len());
    buf.extend_from_slice(salt.as_bytes());
    buf.push(0x00);
    buf.extend_from_slice(&spec_bytes);
    sha256_hex(&buf)
}

/// Derive one manifest entry from a task. `canonical_sha256` is empty (reference
/// solutions are withheld until the post-freeze unseal).
pub fn entry_from_task(salt: &str, task: &SealedTask) -> SealedEntry {
    SealedEntry {
        task_id: task.id.clone(),
        tier: task.tier_num,
        content_sha256: content_sha256(salt, task),
        io_hash: task_io_hash(task),
        canonical_sha256: String::new(),
    }
}

/// Derive the full sealed manifest object from the parsed task file.
pub fn manifest_from_tasks(
    tasks: &SealedTaskFile,
    salt: &str,
    generated_from: &str,
) -> SealedManifest {
    let entries = tasks
        .tasks
        .iter()
        .map(|t| entry_from_task(salt, t))
        .collect();
    SealedManifest {
        schema: SCHEMA.to_string(),
        salt: salt.to_string(),
        freeze_commit: String::new(),
        generated_from: generated_from.to_string(),
        entries,
    }
}
