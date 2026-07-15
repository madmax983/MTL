#![no_main]
//! Parser totality + round-trip fuzz target.
//!
//! Property (from `crates/mtl-syntax/tests/p4_roundtrip.rs`, moved from a fixed
//! proptest budget to a sustained fuzz budget):
//!
//!   * **Totality** — `parse` NEVER panics on any input bytes (interpreted as a
//!     UTF-8 string, lossily), and neither does `print`.
//!   * **P4 round-trip on the parser image** — if `parse(src)` succeeds, then
//!     `parse(print(&prog))` must succeed and equal `prog` (the parser's output
//!     is always in the well-formed, round-tripping domain), and `print` is
//!     idempotent on it.
//!
//! Any panic here is a totality bug (ties #19 eliminate-panic-sites). Any
//! round-trip disagreement is a P4 refinement bug in the production
//! parser/printer relative to the machine-checked `p4_verus.rs` model.

use libfuzzer_sys::fuzz_target;
use mtl_syntax::{parse, print};

fuzz_target!(|data: &[u8]| {
    // Interpret raw bytes as source text. `from_utf8_lossy` keeps this total
    // over arbitrary bytes while still exercising the full lexer/parser.
    let src = String::from_utf8_lossy(data);

    let prog = match parse(&src) {
        Ok(p) => p,
        Err(_) => return, // a rejected input is fine; we only care it didn't panic
    };

    // parse succeeded => prog is in the parser's image (well-formed domain).
    let printed = print(&prog);
    let reparsed = parse(&printed).expect("print(parse(src)) must re-parse");
    assert_eq!(prog, reparsed, "P4 round-trip broke: parse(print(p)) != p");

    // Idempotence of print on the canonical form.
    let printed2 = print(&reparsed);
    assert_eq!(printed, printed2, "print is not idempotent on canonical form");
});
