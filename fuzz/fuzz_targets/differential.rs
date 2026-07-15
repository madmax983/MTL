#![no_main]
//! Interp-vs-arena differential fuzz target (the Engine seam).
//!
//! Generates a well-typed MTL program AST from the fuzzer's bytes and runs it
//! through BOTH execution engines under a shared fuel bound:
//!
//!   * `mtl_core::interp::run` — the reference interpreter (the oracle of truth).
//!   * `mtl_arena::run_arena`  — the production arena backend (the default engine).
//!
//! It then asserts the two agree on terminal kind, fault kind, and final stack.
//! This is the sustained-fuzz form of the fixed-corpus differential oracle in
//! `crates/mtl-arena/tests/oracle.rs` (148 cases) and the `run_refines_reference`
//! proptest in `crates/mtl-core/tests/interpreter.rs`.
//!
//! A DIVERGENCE here is a refinement bug: the machine-checked arena refinement
//! proof (`crates/mtl-arena/proofs/arena_verus.rs`, α(arena_step) = spec_step)
//! claims exactly this agreement holds. The fuzzer is the adversarial net under
//! that proof. Any panic is also a totality bug in either engine.

use libfuzzer_sys::fuzz_target;
use mtl_fuzz::{differential, gen_program};

fuzz_target!(|data: &[u8]| {
    let prog = gen_program(data);
    if let Err(msg) = differential(&prog) {
        panic!("ENGINE DIVERGENCE (interp vs arena)\nprogram: {:?}\n{}", prog, msg);
    }
});
