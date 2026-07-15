//! # mtl-datagen — the CPU-only LoRA training-data factory
//!
//! Builds oracle-validated `(instruction, response)` SFT pairs for the MTL
//! warm-agent fine-tune (design `docs/design/v0.7-lora-warm-agent.md` §3, issue
//! #83). The unfair advantage is a *verified oracle*: candidate MTL programs are
//! admitted **only** if the machine-checked interpreter (`mtl_core::interp`,
//! whose `spec_step` is Verus-proven) accepts them, so correctness is ~100% by
//! construction, not by sampling audit. This is execution-verified rejection
//! sampling with a proven verifier — label noise is ~zero.
//!
//! ## Pipeline stages
//!
//! `generate task spec → generate candidate MTL → run the oracle → keep iff
//! correct → canonicalize → dedup`, then harvest repair traces and meter
//! coverage / contamination.
//!
//! * [`families`]   — parameterized task-family generators (arithmetic, stack,
//!   predicate, recursion, fold/traversal, capability), each with difficulty
//!   knobs; the known-good MTL is seeded from the real `bench/corpus` /
//!   `bench/tier3` solutions and parameterized where sensible.
//! * [`candidates`] — three deterministic candidate strategies (template
//!   synthesis, mutation, bottom-up enumeration); **no LLM calls**.
//! * [`oracle`]     — the unified gate. Tiers 0–2 via `mtl_core::interp::run`
//!   (FUEL = 100_000) over an adversarial input grid; tier-3 via
//!   `mtl_host::caps::task_setup` + `mtl_host::driver::drive`. Only PASS/HALT
//!   candidates enter the dataset.
//! * [`canon`]      — `mtl_syntax::print` canonicalization + SHA-256, plus the
//!   io-behavior-vector hash used for semantic dedup / contamination.
//! * [`repair`]     — harvest `(broken, fault_turn, fixed)` triples, balanced
//!   across the four core fault kinds, from real captured `FaultInfo`.
//! * [`sft`]        — SFT record types + JSONL serialization.
//! * [`coverage`]   — the 23-glyph × tier × difficulty coverage meter.
//! * [`contamination`] — the sealed-set contamination gate (SHA-256 + io-hash).

pub mod candidates;
pub mod canon;
pub mod contamination;
pub mod coverage;
pub mod families;
pub mod oracle;
pub mod repair;
pub mod sealed_spec;
pub mod sft;

use mtl_core::interp::{Prim as IPrim, Value, Word as IWord};

/// The fuel bound for every oracle run (design §6: FUEL = 100_000).
pub const FUEL: u64 = 100_000;

/// The 23 canonical primitive glyphs (order = manifest indices 0..=22).
pub const GLYPHS: [char; 23] = [
    ':', '_', '~', '@', '^', '!', ',', ';', '\'', '+', '-', '*', '/', '%', '=', '<', '?', '&', '.',
    '|', '>', '(', '$',
];

/// What the oracle must observe for one input vector.
#[derive(Clone, Debug, PartialEq)]
pub enum Expected {
    /// The run must `Halt` with exactly this final stack (bottom..top).
    Halt(Vec<Value>),
    /// The run must `Fault` (any core fault kind) — boundary/adversarial input.
    Fault,
}

/// One input→behavior vector in a task's contract.
#[derive(Clone, Debug)]
pub struct IoVector {
    pub input: Vec<Value>,
    pub expected: Expected,
}

/// A generated task instance: a known-good program plus its verified contract.
#[derive(Clone, Debug)]
pub struct TaskInstance {
    pub family: String,
    pub tier: u8,
    pub difficulty: u32,
    pub description: String,
    pub io: Vec<IoVector>,
    /// The known-good MTL program (seed / template fill / discovered program).
    pub program: String,
    /// `Some(task_name)` for tier-3 capability tasks (gated via `task_setup`).
    pub tier3_task: Option<String>,
}

/// Map an interpreter primitive to its canonical glyph (the 23-row manifest).
pub fn iprim_glyph(p: &IPrim) -> char {
    match p {
        IPrim::Dup => ':',
        IPrim::Drop => '_',
        IPrim::Swap => '~',
        IPrim::Rot => '@',
        IPrim::Over => '^',
        IPrim::Apply => '!',
        IPrim::Cat => ',',
        IPrim::Cons => ';',
        IPrim::Dip => '\'',
        IPrim::Add => '+',
        IPrim::Sub => '-',
        IPrim::Mul => '*',
        IPrim::Div => '/',
        IPrim::Mod => '%',
        IPrim::Eq => '=',
        IPrim::Lt => '<',
        IPrim::If => '?',
        IPrim::PrimRec => '&',
        IPrim::Times => '.',
        IPrim::LinRec => '|',
        IPrim::Uncons => '>',
        IPrim::Fold => '(',
        IPrim::Xor => '$',
    }
}

/// Render one interpreter word in canonical glyph form (used in fault turns and
/// io reprs): ints decimal, quotes `[ ... ]`, prims as glyphs, calls by name.
pub fn word_repr(w: &IWord) -> String {
    match w {
        IWord::PushInt(n) => n.to_string(),
        IWord::PushQuote(q) => {
            let inner: Vec<String> = q.iter().map(word_repr).collect();
            format!("[{}]", inner.join(" "))
        }
        IWord::Prim(p) => iprim_glyph(p).to_string(),
        IWord::Call(name) => name.clone(),
    }
}

/// Render one value in canonical form (`mtlrun`-style): ints decimal, quotes
/// `[ ... ]`.
pub fn value_repr(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Quote(q) => {
            let inner: Vec<String> = q.iter().map(word_repr).collect();
            format!("[{}]", inner.join(" "))
        }
    }
}

/// Render a whole stack (bottom..top) space-joined, `<empty>` if empty.
pub fn stack_repr(stack: &[Value]) -> String {
    if stack.is_empty() {
        "<empty>".to_string()
    } else {
        stack.iter().map(value_repr).collect::<Vec<_>>().join(" ")
    }
}

/// Build a `Value::Quote` list of integer literals (an MTL "list" input).
pub fn int_list(xs: &[i64]) -> Value {
    Value::Quote(xs.iter().map(|n| IWord::PushInt(*n)).collect())
}

/// Serialize a `Value` to a JSON "cell": an integer, or an array of integers for
/// a list (quote of int literals). Used to embed a re-runnable reference
/// contract in each SFT record for the re-validation invariant. Contracts in
/// this crate only ever hold ints and flat int-lists.
pub fn value_to_cell(v: &Value) -> serde_json::Value {
    match v {
        Value::Int(n) => serde_json::json!(n),
        Value::Quote(q) => {
            let ints: Vec<i64> = q
                .iter()
                .map(|w| match w {
                    IWord::PushInt(n) => *n,
                    _ => panic!("non-int quote element in a dataset contract"),
                })
                .collect();
            serde_json::json!(ints)
        }
    }
}

/// Inverse of [`value_to_cell`]: a JSON number → `Int`, a JSON array → int-list.
pub fn cell_to_value(c: &serde_json::Value) -> Value {
    if let Some(n) = c.as_i64() {
        Value::Int(n)
    } else if let Some(arr) = c.as_array() {
        let ints: Vec<i64> = arr.iter().map(|x| x.as_i64().expect("int cell")).collect();
        int_list(&ints)
    } else {
        panic!("unexpected cell shape: {c}")
    }
}

/// A tiny deterministic PRNG (splitmix64) — seeded, no clock, fully reproducible.
#[derive(Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}
