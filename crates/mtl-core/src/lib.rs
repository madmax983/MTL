//! # mtl-core
//!
//! Core reference semantics for **MTL** — a concatenative stack language whose
//! semantics are formally verified in [Verus](https://github.com/verus-lang/verus).
//!
//! The machine-checked ghost model, `spec_step`, and proofs (P1/P3) live in
//! [`src/mtl_core.rs`](./mtl_core.rs). That file is a self-contained Verus
//! artifact pinned to **Verus 0.2026.07.05**; it is checked by the `verus`
//! tool, not compiled by `cargo` (it depends on `vstd` and the `verus!`
//! macro). This stub keeps the crate buildable on stable Rust.
//!
//! See the language spec in [`docs/mtl-spec.md`](../../docs/mtl-spec.md).

/// The Verus version this crate's proofs are pinned to.
pub const VERUS_VERSION: &str = "0.2026.07.05";

/// The MTL executable interpreter (Track B).
///
/// The cargo-compiled, stable-Rust runtime for the MTL concatenative core: exec
/// AST types, `exec_step`, and the fuel-bounded `run`. It is the runtime mirror
/// of the Verus-verified `exec_step`/`run` in `src/mtl_core.rs` (which is checked
/// by `verus`, not compiled by `cargo`). See the module docs for the split.
pub mod interp;
