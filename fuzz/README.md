# MTL fuzz harness

`cargo-fuzz` (libFuzzer) targets that turn MTL's existing round-trip and oracle
**properties** — until now exercised only on a fixed proptest budget — into a
sustained, adversarial panic / differential-disagreement net. This is the fuzz
leg of issue #58: it drives the proof-to-production boundary the Verus proofs
alone do not cover.

## Targets

| target | input | property | wired to |
|--------|-------|----------|----------|
| `parse_roundtrip` | raw bytes → source text (lossy UTF-8) | `parse` and `print` **never panic**; if `parse` succeeds, `parse(print(p)) == p` and `print` is idempotent | `crates/mtl-syntax/tests/p4_roundtrip.rs`, proven model `crates/mtl-syntax/proofs/p4_verus.rs` |
| `differential` | raw bytes → generated program AST | the reference interpreter (`mtl_core::interp::run`) and the production arena backend (`mtl_arena::run_arena`) **agree** on terminal kind, fault kind, and final stack (the Engine seam) | `crates/mtl-arena/tests/oracle.rs` (148-case oracle), proven refinement `crates/mtl-arena/proofs/arena_verus.rs`, `crates/mtl-core/tests/interpreter.rs` (`run_refines_reference`) |
| `parse_exec` | raw bytes → source text → parse → execute | full production pipeline: parse never panics, and on a successful parse both engines agree | ties parser to interpreter/arena end to end |

A **panic** = a totality bug (ties #19 eliminate-panic-sites). An **engine
divergence** = a refinement bug against the machine-checked arena proof.

## Run locally

Requires a nightly toolchain and libFuzzer (installed with `cargo install cargo-fuzz`).

```sh
# quick smoke (matches the CI budget: ~30s per target)
cargo +nightly fuzz run parse_roundtrip -- -max_total_time=30
cargo +nightly fuzz run differential     -- -max_total_time=30
cargo +nightly fuzz run parse_exec       -- -max_total_time=30

# a longer manual soak (recommended before a release): 10 min per target
cargo +nightly fuzz run differential -- -max_total_time=600

# reproduce a crash artifact the fuzzer saved
cargo +nightly fuzz run differential fuzz/artifacts/differential/crash-<hash>
```

## CI budget

CI runs each target for a bounded wall-clock budget per PR (see the `fuzz` job in
`.github/workflows/ci.yml`). libFuzzer exits non-zero on the first panic or
differential disagreement, so the job **fails** on any new finding and uploads
the crash artifact. The budget is a smoke, not an exhaustive soak; the table
above documents the longer local run for release gating.

## Corpus

`corpus/<target>/` holds seed inputs. The textual targets are seeded from the
spec examples and glyph coverage used in the golden tests; the `differential`
target is seeded with short byte strings that the AST generator decodes into
programs. libFuzzer grows each corpus as it discovers new coverage.
