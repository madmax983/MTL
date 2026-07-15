//! Canonicalization + hashing for dedup and contamination.
//!
//! Two keys per accepted program (design §3 "two-key mechanical dedup"):
//!   1. the **program canonical-form SHA-256** — `mtl_syntax::print` is the
//!      byte-identical production mirror of the machine-proven `exec_print`, so
//!      the canonical bytes defeat formatting-variant leaks;
//!   2. the **io-behavior-vector hash** — SHA-256 over the sorted
//!      `(input → output)` pairs, catching semantic dupes a rephraser evades.

use sha2::{Digest, Sha256};

use crate::{value_repr, Expected, IoVector};

/// Canonicalize an MTL source string via the proven printer. Returns the
/// canonical string, or `None` if the source does not parse.
pub fn canonical(src: &str) -> Option<String> {
    mtl_syntax::parse(src).ok().map(|p| mtl_syntax::print(&p))
}

/// Lowercase hex SHA-256 of arbitrary bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Canonical form + its SHA-256, or `None` if the program does not parse.
pub fn canonical_sha(src: &str) -> Option<(String, String)> {
    let canon = canonical(src)?;
    let sha = sha256_hex(canon.as_bytes());
    Some((canon, sha))
}

/// The io-behavior-vector hash over a task's contract: SHA-256 of the sorted
/// `input => output` lines, so two programs with identical observable behavior
/// hash equally regardless of surface form.
pub fn io_hash(io: &[IoVector]) -> String {
    let mut lines: Vec<String> = io
        .iter()
        .map(|v| {
            let inp = v.input.iter().map(value_repr).collect::<Vec<_>>().join(" ");
            let out = match &v.expected {
                Expected::Halt(stack) => {
                    let s = stack.iter().map(value_repr).collect::<Vec<_>>().join(" ");
                    format!("HALT:{s}")
                }
                Expected::Fault => "FAULT".to_string(),
            };
            format!("{inp}=>{out}")
        })
        .collect();
    lines.sort();
    sha256_hex(lines.join("\n").as_bytes())
}
