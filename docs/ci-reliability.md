# CI reliability, coverage, and fuzzing (issue #58)

This note records the root-cause of the historical CI runner stall and the
durable mitigations applied to `.github/workflows/ci.yml`, plus the two new
jobs (coverage, fuzz). It is the falsifiable-diagnosis artifact required by
issue #58 AC-1.

## 1. Diagnosis — why the gate never ran

`docs/RELEASE-NOTES-0.4.0.md` ("The CI that never ran"; "Proofs are attested
**locally**, not by CI (runner stall)") records that **every proof in v0.4.0 was
checked on a maintainer's laptop with source-built Verus**, never by CI. Three
distinct failures stacked up:

1. **GitHub-side runner non-dispatch (the dominant cause).** GitHub's hosted
   runners were rate-limited / queue-starved and never dispatched the jobs. A
   gate that never runs cannot gate a merge — it silently *un*-gates it, which
   is worse than no badge because it implies coverage that isn't there.
2. **A green-while-skipped bug (already fixed, pre-#58).** A job-level
   `continue-on-error` on the `verus` job once swallowed a missing-Rust-toolchain
   failure: `verus --version` died, the verify step was *skipped*, and the job
   still reported green. Fixed by making toolchain health a **hard, fail-fast
   gate** (the `verus --version` smoke step, NO `continue-on-error`) while only
   the proof *verification outcome* stays advisory. **Preserved by #58.**
3. **A 404 on the pinned release asset (already fixed, pre-#58).** The Verus
   release tag carries a commit suffix (`release/0.2026.07.05.49b8806`, asset
   `verus-0.2026.07.05.49b8806-x86-linux.zip`); the bare `0.2026.07.05` URL
   404'd, so the install step failed before Verus ever ran. Fixed by pinning the
   full suffixed id. **Preserved by #58.**

The residual, unaddressed cause after (2) and (3) were fixed was (1): the runners
simply weren't dispatching. That is what #58 targets.

## 2. Mitigations applied

The queue-starvation cause (1) is fought on two fronts — **reduce the load we put
on the queue** and **make each job self-limiting so one stuck leg cannot wedge
the rest**:

| Mitigation | Where | Effect on the stall |
|---|---|---|
| **`concurrency` group + `cancel-in-progress`** | workflow top-level | A re-push to a PR cancels that PR's older in-flight run, freeing the runner slot immediately instead of both sitting in the queue. Keyed on `github.head_ref \|\| github.run_id`, so **pushes to `main`/`bootstrap` get a unique group and are never cancelled** — default-branch runs always complete. |
| **Path filter (`changes` job)** | gates `verus`, `coverage`, `fuzz` | A docs-only diff (`docs/**`, `**/*.md`, `LICENSE`, `.gitignore`) skips the three heavy jobs entirely, so a README edit never consumes a Verus-sized runner slot. The fast `cargo` job has **no** path gate, so a required status check still always reports. |
| **`timeout-minutes` on every job** | all jobs | A genuinely stuck runner turns the job **RED** at a bounded deadline (cargo 20m, verus 45m, coverage 30m, fuzz 25m, changes 5m) instead of hanging the queue indefinitely. |
| **Bounded retry on the Verus download** | `verus` install step | The single most stall-prone step (a network fetch that historically 404'd and is subject to rate-limiting) now retries 4× with 2/4/8/16 s backoff, and only fetches on a cache miss. |
| **Caching** | `cargo`, `coverage`, `fuzz` (`Swatinem/rust-cache`); `verus` (`actions/cache` on the release zip) | Shorter jobs hold a runner slot for less wall-clock time, reducing queue pressure. The immutable Verus release zip is cached by version key, so a hit skips the network entirely. |
| **Job splitting** | `cargo` vs `verus` vs `coverage` vs `fuzz` | Already-separate jobs (kept): a stall or failure in the heavy `verus` leg does not block the fast `cargo` gate from reporting. |

### Not done here (operational / out of scope)

- **Self-hosted runner for the heavy `verus` job.** The issue lists this as *one
  of* several acceptable durable mitigations ("a self-hosted runner … **and/or**
  per-job `timeout-minutes` + retry … **and/or** job-splitting"). Provisioning a
  self-hosted runner is an infra/operational task (a machine, a registration
  token, a security posture for running untrusted PR code) outside a workflow
  edit, so this change applies the retry/timeout/split/concurrency/caching
  mitigations instead. **Until a self-hosted verifier lands, proof evidence
  remains local source-built Verus** — the README "CI status (read the counts
  honestly)" note is deliberately preserved and NOT replaced with a green badge.

## 3. New jobs

- **`coverage`** — `cargo-llvm-cov` over the workspace, **advisory** (never gates
  the merge). Publishes a per-file line/region table to the step summary and an
  HTML + lcov artifact. Success-metric surfaces (issue #58 target ≥ 70%):
  `mtl-core/src/interp.rs`, `mtl-core/src/host.rs`, and the checker
  `mtl-check/src/lib.rs`. First local baseline (this branch):

  | surface | line coverage |
  |---|---|
  | `crates/mtl-core/src/interp.rs` | **93.89%** |
  | `crates/mtl-core/src/host.rs` | **100.00%** |
  | `crates/mtl-check/src/lib.rs` (checker) | **79.13%** |
  | workspace TOTAL | 60.53% |

  The core surface clears the ≥ 70% target; the baseline is to be ratcheted, not
  lowered.

- **`fuzz`** — a `cargo-fuzz` (libFuzzer) smoke that **gates on findings**: it
  fails on the first panic or engine divergence. Three targets, wired to MTL's
  existing round-trip/oracle properties (see `fuzz/README.md`):
  - `parse_roundtrip` — `parse`/`print` totality + P4 round-trip
    (`crates/mtl-syntax/tests/p4_roundtrip.rs`, proven model `p4_verus.rs`).
  - `differential` — interp-vs-arena agreement across the Engine seam
    (`crates/mtl-arena/tests/oracle.rs`, proven refinement `arena_verus.rs`).
  - `parse_exec` — the full source → parse → execute pipeline.

  **Resource-exhaustion caveat.** Step-fuel bounds steps, not memory: an
  adversarial quote-doubling loop grows structure exponentially per step and
  both engines copy on `cat`. The fuzz job therefore runs under libFuzzer
  `-rss_limit_mb` + `-timeout` guards; a saved crash artifact from those is an
  adversarial *resource-exhaustion* input (a proof-to-production / #19 concern),
  triaged separately from a genuine panic or divergence. The differential harness
  also skips the arena's u32-capacity `Overflow` boundary, which the reference
  interpreter has no matching cap for.

## 4. Preserved gate semantics (no regression — issue #58 AC-7)

Unchanged by this issue:

- **Toolchain health** (`verus --version` smoke) — HARD, fail-fast gate.
- **P5 Turing-completeness** (`p5_universality.rs`) — HARD gate.
- **Layer C checker soundness M1+M2+M3+M4** (`checker_verus.rs`, 116/0) — HARD gate.
- **Arena refinement** (`arena_verus.rs`, 145/0) — HARD gate.
- **Core P1/P3** and **P4** verification outcomes — advisory (`continue-on-error`),
  visible-but-non-blocking.
- **`--no-cheating` honesty audits** — report-only; still surface the 2 intended
  P2 `external_body` stubs and would flag any *new* cheat.
- **`tokreport`** (network-dependent) and **`perf-smoke`** (compile-only, path-gated)
  — unchanged and non-blocking.
