//! Candidate-program generation — three deterministic, NO-LLM strategies.
//!
//! 1. **template/grammar synthesis** — the parameterized known-good programs the
//!    [`crate::families`] module already emits (constant-holes filled from family
//!    params). These are the templates; acceptance is ~100% by construction.
//! 2. **mutation** — single/double edits (delete / replace / insert / transpose)
//!    of a validated seed over the 23-glyph manifest. Most fail the oracle —
//!    that is the acceptance-rate signal AND the repair-trace fodder.
//! 3. **bottom-up enumeration** — enumerate short programs over a restricted
//!    glyph subset, run each on an input grid, and keep any that HALT
//!    deterministically: the program IS the spec, discovered by execution. A
//!    candidate cap keeps it terminating fast.

use mtl_core::interp::{Outcome, Value};

use crate::oracle::run_on;
use crate::{Expected, IoVector, TaskInstance};

/// The glyph alphabet used for mutation replacement/insertion (the 23 prims).
const MUT_GLYPHS: &[char] = &[
    ':', '_', '~', '@', '^', '!', ',', ';', '\'', '+', '-', '*', '/', '%', '=', '<', '?', '&', '.',
    '|', '>', '(', '$',
];

/// Produce single-glyph mutations of `src` (delete one char, replace one char
/// with a manifest glyph, insert a glyph, transpose two adjacent chars). Only
/// mutations that PARSE are returned; the caller runs the oracle to classify
/// them (still-correct / wrong / faulting). Deterministic order.
pub fn mutations(src: &str) -> Vec<String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    // delete
    for i in 0..chars.len() {
        let mut c = chars.clone();
        c.remove(i);
        push_if_parses(&mut out, &c);
    }
    // transpose adjacent
    for i in 0..chars.len().saturating_sub(1) {
        let mut c = chars.clone();
        c.swap(i, i + 1);
        push_if_parses(&mut out, &c);
    }
    // replace with a glyph
    for i in 0..chars.len() {
        for &g in MUT_GLYPHS {
            if chars[i] == g {
                continue;
            }
            let mut c = chars.clone();
            c[i] = g;
            push_if_parses(&mut out, &c);
        }
    }
    // insert a glyph
    for i in 0..=chars.len() {
        for &g in MUT_GLYPHS {
            let mut c = chars.clone();
            c.insert(i, g);
            push_if_parses(&mut out, &c);
        }
    }
    out
}

fn push_if_parses(out: &mut Vec<String>, chars: &[char]) {
    let s: String = chars.iter().collect();
    if s.is_empty() {
        return;
    }
    if mtl_syntax::parse(&s).is_ok() && !out.contains(&s) {
        out.push(s);
    }
}

/// Bottom-up enumeration of short programs over a restricted glyph subset. Each
/// program is run on a fixed 2-deep input grid; any that HALTs (non-empty
/// result) on EVERY grid input becomes a discovered task whose contract is its
/// observed behavior. `max_keep` caps output; enumeration order is fixed.
pub fn enumerate(max_keep: usize) -> Vec<TaskInstance> {
    // Restricted alphabet: stack shuffles, the three total arith ops, small ints.
    let toks: [&str; 9] = [":", "_", "~", "+", "-", "*", "$", "1", "2"];
    let grid: [Vec<i64>; 4] = [vec![3, 4], vec![5, 6], vec![-2, 7], vec![0, 1]];
    let mut kept: Vec<TaskInstance> = Vec::new();
    let mut seen_io: std::collections::HashSet<String> = std::collections::HashSet::new();

    // lengths 2..=4
    for len in 2..=4usize {
        let total = toks.len().pow(len as u32);
        for code in 0..total {
            if kept.len() >= max_keep {
                return kept;
            }
            let mut src = String::new();
            let mut c = code;
            for _ in 0..len {
                src.push_str(toks[c % toks.len()]);
                c /= toks.len();
            }
            // Run on every grid input; require deterministic Halt with output.
            let mut io: Vec<IoVector> = Vec::new();
            let mut ok = true;
            for inp in &grid {
                let stack: Vec<Value> = inp.iter().map(|n| Value::Int(*n)).collect();
                match run_on(&src, &stack) {
                    Some(Outcome::Halt(out)) if !out.is_empty() => {
                        io.push(IoVector {
                            input: stack,
                            expected: Expected::Halt(out),
                        });
                    }
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            // Dedup by observed behavior; skip identity-ish (output == input).
            let key = crate::canon::io_hash(&io);
            if !seen_io.insert(key) {
                continue;
            }
            // Skip trivial no-ops (output equals input on all vectors).
            let trivial = io.iter().all(|v| match &v.expected {
                Expected::Halt(o) => o == &v.input,
                Expected::Fault => false,
            });
            if trivial {
                continue;
            }
            let examples: Vec<String> = io
                .iter()
                .map(|v| {
                    let inp = v
                        .input
                        .iter()
                        .map(crate::value_repr)
                        .collect::<Vec<_>>()
                        .join(" ");
                    let out = match &v.expected {
                        Expected::Halt(o) => o
                            .iter()
                            .map(crate::value_repr)
                            .collect::<Vec<_>>()
                            .join(" "),
                        Expected::Fault => "FAULT".into(),
                    };
                    format!("[{inp}] -> [{out}]")
                })
                .collect();
            let description = format!(
                "Implement a stack program with exactly this behavior:\n{}",
                examples.join("\n")
            );
            kept.push(TaskInstance {
                family: "enumerated".into(),
                tier: 0,
                difficulty: (len as u32).saturating_sub(1),
                description,
                io,
                program: src,
                tier3_task: None,
            });
        }
    }
    kept
}
