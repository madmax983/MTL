# Primitive mirror codegen (issue #46)

## Problem

Each of the 23 v0.x primitives is declared in several independent "mirror"
surfaces. Cross-mirror drift — one surface silently disagreeing with another — is
the project's #1 named architecture risk. PR #32 made
`crates/mtl-syntax/src/manifest.rs` the single checked source of truth and shipped
`mtl-conformance` to **detect** drift after the fact. Issue #46 closes the
remaining gap — **prevention**: the mechanical mirrors are now *generated* from
one canonical table, so they cannot be hand-written out of agreement in the first
place.

## Mechanism

The single canonical table lives in one place — the
`mtl_syntax::for_each_primitive!` x-macro in `manifest.rs`. It holds the 23 rows
`(index, Name, glyph, arity, stack_effect)`. Every generated mirror passes a small
local callback `macro_rules!` that receives the rows and expands them into that
mirror's shape. This keeps all generated code **checked-in and IDE-friendly**
(rust-analyzer expands `macro_rules!`), with no `build.rs`/`OUT_DIR` artifact and
— critically — **no proc-macro dependency entering the verified core**. Editing a
row propagates to every generated mirror on the next `cargo build`.

## Mirror inventory and verdicts

The issue enumerates six mirrors, plus the arena as a policed seventh. Below is
each surface, its location, and the verdict: **generated** (a pure function of the
manifest, emitted at compile time) or **hand-written + policed** (with the reason
it must stay independent).

| # | Mirror | Location | Verdict | Reason |
|---|--------|----------|---------|--------|
| 1 | parser `Prim` enum | `mtl-syntax/src/ast.rs` | **Generated** | pure function of the table (variant list + order) |
| 2 | `GLYPHS` + `prim_to_glyph`/`glyph_to_prim` | `mtl-syntax/src/ast.rs` | **Generated** | `GLYPHS` emitted from the table; `prim_to_glyph` reads the generated `manifest::meta_of`; `glyph_to_prim` scans the generated `GLYPHS` |
| 3 | interp `Prim` enum | `mtl-core/src/interp.rs` | **Hand-written + policed** | kept a genuinely independent mirror so the differential oracle has something to disagree with; `mtl-core` has no `mtl-syntax` dependency and is the verified-adjacent runtime. Policed by `interp_prim_names_match_manifest`, `interp_prim_exhaustive` |
| 4 | Verus `SpecPrim` enum | `mtl-core/src/mtl_core.rs` | **Hand-written + policed** | the verified core must not gain a codegen/proc-macro dependency (recorded architecture-review compromise). Policed by `verus_specprim_matches_manifest` |
| 5 | `mtl-bench-validate::conv` opcode map | `bench/validate/src/lib.rs` | **Generated** | the `syntax::Prim → interp::Prim` match is a pure function of the table |
| 6 | `mtl-host::conv` opcode map | `mtl-host/src/lib.rs` | **Generated** | same as #5 |
| 7 | arena `Prim` mirror (`ARENA_PRIMS`, `arena_prim_name`, `arena_prim_arity`) | `mtl-arena/src/types.rs` | **Hand-written + policed** | generatable in principle, but kept independent for the same assurance reason as the interp mirror: the arena is the differential-oracle counterpart and must be able to *disagree*. Policed by `tests/arena_conformance.rs` (names, count, arity, exhaustiveness, differential oracle) |

**Result: 4 of 6 named mirrors generated** (#1, #2, #5, #6), meeting the issue's
`>= 4/6` target. The two hand-written mirrors (#3 interp, #4 Verus `SpecPrim`) and
the arena mirror (#7) remain comparison-policed.

## The Verus arena model (`crates/mtl-arena/proofs/arena_verus.rs`, PR #81)

PR #81 landed a Verus refinement model of the arena. It is a **Verus proof root**
(`verus! { … }`, `use vstd`) and is therefore hand-written by hard rule — Verus
proof text is never generated. It is **not a new primitive-name mirror**: it does
not declare its own `Prim`/`SpecPrim` variant list — it `#[path]`-includes
`mtl_core.rs` and **reuses `mtl_core::SpecPrim` directly** (see its `ModelWord`
comment: "Reuses `mtl_core::SpecPrim` directly … policed by the conformance
crate"). So it rides transitively on mirror #4 and needs no separate codegen or
conformance surface; `verus_specprim_matches_manifest` already backstops the one
`SpecPrim` it borrows.

## Backstops that remain

Codegen *reduces* the hand-written surface; it does not retire the police:

- Every generated mirror keeps a per-mirror **smoke assertion** in
  `mtl-conformance` (`syntax_prim_names_match_manifest`,
  `glyphs_agree_with_manifest`, `conv_fns_agree`) so a codegen bug is caught, not
  assumed away.
- `mirror_generation_propagates` (conformance test #9) re-derives the rows
  straight from `for_each_primitive!` and asserts every generated mirror reflects
  them — the propagation backstop rooted at the single source of truth.
- The compile-time guards survive: `meta_of` is generated as a no-wildcard
  exhaustive match, and `PRIMITIVES`/`ALL_PRIMS`/`ARENA_PRIMS` stay fixed `[_; 23]`
  arrays.
