//! # mtl-arena — production arena execution backend
//!
//! A clean, maintainable rewrite of the `mtl-arena-spike` measurement vehicle
//! into a production-hygiene arena backend for the MTL concatenative core, built
//! to the v0.5 design (`docs/design/v0.5-refactor.md`). It is the sequential
//! foundation the host seam, conformance policing, and (future) speculation build
//! on.
//!
//! ## What it is
//!
//! An interned, persistent, O(1)-fork continuation engine that is **bit-identical**
//! to the reference interpreter (`mtl_core::interp`). The three representational
//! pillars (design §3.1):
//!
//! * [`QuoteArena`] — one flat `tape: Vec<Word>` interning every quote body.
//!   [`QuoteId`] is a `{start,len}` slice; list tails are O(1) sub-slices.
//! * [`StackArena`] — a persistent, structurally shared cons-list stack.
//! * [`ContArena`] — the continuation as a **persistent segment cons-list + local
//!   cursor**. Reading the next word is a cursor bump (O(1)); prepending a quote
//!   freezes the current head and pushes a child segment (≤2 allocs, no tail
//!   copy). This is the fix for the measured O(n²) front-pop / re-emit pathologies.
//!
//! [`VmState`] (stack + cont + cursor = three `u32`s, 12 bytes, `Copy`) is the
//! whole machine position, so **fork is a 12-byte copy** independent of depth.
//!
//! ## Opt-in, never a silent substitute
//!
//! This crate is a *separate, explicit* entry point ([`run_arena`] / [`arena_step`]).
//! It never replaces `interp::run` — the reference interpreter remains the default
//! twin and the oracle of truth. Parity is proven by the differential oracle
//! (`tests/oracle.rs`, 47/47) and the fault-corpus parity test
//! (`tests/fault_parity.rs`); the arena "does not ship" unless those are green
//! (design §5 drift-accounting gate).
//!
//! ## Production hygiene
//!
//! The step/exec loop is **total**: no `unwrap`/`expect`/`unreachable!`/`panic!`/
//! `todo!` and no panicking array index (matching `interp.rs`'s "never panics"
//! contract). Every stack/tape access and refinement yields a [`Fault`] or a
//! `debug_assert`-guarded internal `Option`. u32 address-space overflow returns a
//! clean [`Fault::Overflow`] rather than wrapping (design §3.4). Memory is
//! reclaimed generationally via high-water [`Mark`]s and truncate-reset
//! ([`Vm::mark`] / [`Vm::reset_to`]).
//!
//! ## Reification boundary
//!
//! Anything crossing out of a generation is reified to OWNED reference types (the
//! shape the rest of the system consumes, design §11.4): [`Vm::reify_stack`] →
//! `Vec<interp::Value>`, [`Vm::reify_cont`] → `Vec<interp::Word>`, and
//! [`Vm::fault_info`] → `interp::FaultInfo`. [`ArenaRun::outcome`] assembles these
//! into an [`Outcome`] identical in shape to `interp::Outcome`.

mod arena;
pub mod host;
mod prim;
mod run;
mod types;
mod vm;

// ---- arena types (the policed internal mirrors; pub for conformance + oracle) --
pub use types::{Fault, Prim, ProgWord, QuoteId, Value, Word};

// ---- policed reflection surface (design §5: the arena Prim mirror, exposed so
//      `mtl-conformance` can assert names/order/arity vs the manifest without
//      re-reading engine internals). ----
pub use types::{arena_prim_arity, arena_prim_name, ARENA_PRIMS};

// ---- arenas + machine position ----
pub use arena::{
    ContArena, ContPtr, Mark, QuoteArena, StackArena, StackPtr, VmState,
};

// ---- the VM + driver surface ----
pub use run::{arena_step, run_arena, ArenaEnd, ArenaRun, Outcome, Step};
pub use vm::Vm;
