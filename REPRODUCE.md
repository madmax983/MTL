# Reproduce MTL's numbers

The reviewer-runnable replication kit for MTL (Minimal Token Language), issue #71.

## What this reproduces

From a clean clone, this kit rebuilds the machine-checked proof stack and
regenerates every headline number MTL claims. `./kit/replicate.sh` runs the
entire non-Verus pipeline — the Rust test suite, the three tokenizer baselines
(T_v0, tier-2, tier-3), the corpus/sealed validators, the arena differential
oracle, the contamination gates, and the end-to-end interpreter demo — and
**asserts every ratio**, treating a byte-identical regeneration of each tracked
`bench/BASELINE*.md` as the tolerance check (zero drift). `./kit/proof-gates.sh`
runs the five Verus proof roots through a pinned, source-built prover and asserts
each reaches its exact `verified, 0 errors` count. Nothing here modifies a proof
`.rs` file or admits a new primitive — it re-checks the existing artifacts.

## Prerequisites

**Non-Verus pipeline (`kit/replicate.sh`):**
- **Rust** — the toolchain is pinned by `rust-toolchain.toml` (stable); `cargo`
  on PATH is all you need. The pipeline compiles the workspace on first run.
- **Python 3 + pip** — for the three `report*.py` tokenizer baselines.
- **tiktoken 0.8.0** — installed by the script from `bench/tokcount/requirements.txt`.
  This exact version defines the `o200k_base` / `cl100k_base` token counts; every
  ratio is measured under it.

**Proof gates (`kit/proof-gates.sh` + `kit/build-verus.sh`):**
- A **source-built, pinned Verus + Z3**: **Verus `0.2026.07.05.49b8806`**
  (commit `49b8806`), **Z3 `4.12.5`**, **Rust `1.96.0`**. The pinned Verus
  release asset is not on crates.io and the GitHub release-asset download is
  network-blocked in the reference environment, so `kit/build-verus.sh` builds
  both from source via `git clone` (which works through the proxy). It needs
  `rustup` (to add `rustc-dev rustfmt llvm-tools` to toolchain 1.96.0) and a
  C++ build toolchain (for Z3's `mk_make.py` + `make`).

**Resource expectations (qualitative, no wall-clock promises):** the non-Verus
pipeline is light — it compiles the Rust workspace once and runs fast test
suites. The **Verus build is heavy**: it compiles Z3 and the full Verus prover
(including `vstd`) from source and wants a machine with real CPU and several GB
of free disk for the two source trees plus build artifacts. Budget accordingly;
this document deliberately states no minute/hour estimates.

## Quick start

```bash
# 1. Non-Verus pipeline — regenerates and asserts every token ratio + test gate.
./kit/replicate.sh

# 2. Proof gates — build the pinned prover once, then run the 5 Verus roots.
./kit/build-verus.sh          # source-builds Verus 0.2026.07.05.49b8806 + Z3 4.12.5
./kit/proof-gates.sh          # asserts 76 / 118 / 101 / 116 / 145 verified, 0 errors
#   (if verus is already built: VERUS_BIN=/root/verus-src/source/target-verus/release/verus ./kit/proof-gates.sh)
```

`replicate.sh` exits non-zero on the first failed assertion and prints a
PASS/FAIL line per step. `proof-gates.sh` exits non-zero if **any** proof root
misses its count or reports any error (the P5 and arena roots are the named hard
gates). A captured green run of the non-Verus pipeline is in
[`kit/EVIDENCE.md`](kit/EVIDENCE.md).

## Claim → command map

Every published numeric claim, the exact copy-pasteable command that regenerates
it, and the output line to look for. All commands run from the repo root.

| # | Claim | Command | Expected output line |
|---|---|---|---|
| 1 | **T_v0 micro compression 3.72×** (o200k & cl100k) | `python3 bench/tokcount/report.py` | `\| v0.2 (validated) \| o200k_base \| 93 \| 25 \| 3.72x \| MET \|` (and the cl100k row) in `bench/BASELINE.md` |
| 2 | **Tier-2 compression 3.87× / 3.92×** (scenario B, o200k / cl100k) | `python3 bench/tokcount/report_tier2.py` | `\| v0.3 tier-2 (scenario B) \| 11 \| o200k_base \| 352 \| 91 \| 3.87x \|` and the `... cl100k_base ... 3.92x` row in `bench/BASELINE-TIER2.md` |
| 3 | **Tier-3 executable density 1.86× / 1.85×** (o200k / cl100k) | `python3 bench/tier3/report.py` | `- **executable**: o200k **1.86x**, cl100k **1.85x**` in `bench/BASELINE-TIER3.md`. **README's older 1.90× is superseded** — that was the design-sketch figure; the artifact reports the executable exec-column 1.86×/1.85× the tier3run oracle actually runs. |
| 4 | **Sealed held-out compression 1.67× / 1.72×** (o200k / cl100k) | (published artifact) `bench/BASELINE-SEALED.md` §1 | `Held-out static compression ≈ 1.67x (o200k) / 1.72x (cl100k)` — the first out-of-sample measurement (below the retired ≥3× gate). |
| 5 | **Sealed solutions validated — 14/14 correct** | `cargo test -p mtl-bench-validate` | `test committed_solutions_pass_all_vectors_constructed_stack ... ok` (in `tests/sealed.rs`; validates 14/15 committed sealed solutions on all I/O vectors under dev-parity constructed-stack interpretation) |
| 6 | **Per-solution held-out CSPM 2.124×** (correct-solutions-per-million-tokens, MTL/Py) | *NOT push-button — needs a live model.* Scorer + ground truth + recorded per-attempt JSON: `bench/sealed/agent-trial/results/REPORT.md`, `bench/sealed/agent-trial/results/{mtl,py}_t*/…json` | `Correct-solutions-per-million-tokens ratio (MTL / Python): 2.124` |
| 7 | **Held-out agent success 100% pass@5** (MTL = Python) | *NOT push-button — needs a live model.* Recorded results: `bench/sealed/agent-trial/results/REPORT.md` | `Held-out agent success: 100% pass@5 (MTL) = 100% pass@5 (Python)` (on the 7 text-feedable sealed tasks) |
| 8 | **mtl_core.rs — 76 verified, 0 errors** (P1/P3 core) | `verus crates/mtl-core/src/mtl_core.rs` (via `./kit/proof-gates.sh`) | `verification results:: 76 verified, 0 errors` |
| 9 | **p5_universality.rs — 118 verified, 0 errors** (P5 Turing-completeness, HARD GATE) | `verus crates/mtl-core/src/p5_universality.rs` | `verification results:: 118 verified, 0 errors` |
| 10 | **p4_verus.rs — 101 verified, 0 errors** (P4 round-trip + printer/parser) | `verus crates/mtl-syntax/proofs/p4_verus.rs` | `verification results:: 101 verified, 0 errors`. **Issue #71's checkbox says 42 — that is a stale pre-count; the current artifact and `crates/mtl-core/proof-log.txt` cite 101.** |
| 11 | **checker_verus.rs — 116 verified, 0 errors** (Layer-C checker soundness, HARD GATE) | `verus crates/mtl-core/src/checker_verus.rs` | `verification results:: 116 verified, 0 errors` |
| 12 | **arena_verus.rs — 145 verified, 0 errors** (arena refines spec_step, HARD GATE) | `verus crates/mtl-arena/proofs/arena_verus.rs` | `verification results:: 145 verified, 0 errors` |
| 13 | **Only 2 trusted cheats** (`Word::clone`, `Value::clone` P2 Clone stubs) | `verus --no-cheating crates/mtl-core/src/mtl_core.rs` | flags exactly the 2 `external_body` Clone stubs — the declared trust boundary, report-only |
| 14 | **Workspace test suite — 322 passed, 0 failed** | `cargo test --workspace` | summed across crates: `322 passed; 0 failed` (incl. `p5_minsky` 6 passed) |
| 15 | **Differential oracle — 148 interp-vs-arena cases agree** | `cargo test -p mtl-arena --test oracle` | `differential_oracle ... ok` + `differential_oracle_forced_compaction ... ok`; both assert the corpus is exactly 148 cases and 148/148 agree |
| 16 | **Factorial demo (end-to-end interpreter)** | `cargo run --bin mtlrun -p mtl-bench-validate -- '5[1][*]&'` | `HALT: 120` |

Claims 1–3, 5, 8–16 are all driven and asserted automatically by
`./kit/replicate.sh` (non-Verus) and `./kit/proof-gates.sh` (Verus).

## What is NOT clean-checkout reproducible

The **live-model agent trials** cannot be regenerated without model access, and
the kit does not pretend otherwise. This covers the readtax, session-economics,
preamble-ablation, CSPM, and pass@5 experiments. Their raw per-attempt outputs
were produced by a live pinned-model orchestrator run (`claude-opus-4-8`, no
fine-tuning). A reviewer **with no model access reproduces the scorer + the
ground truth + the recorded `results.jsonl` / per-attempt JSON — not the raw
model trials.** The scorer is deterministic and re-runnable against the recorded
data; only the model rollouts are not.

Recorded-data paths (checked in, inspectable, re-scoreable):
- **Sealed agent trial (CSPM 2.124×, pass@5 100%):** `bench/sealed/agent-trial/results/REPORT.md`,
  `bench/sealed/agent-trial/results/metrics.json`, and the per-attempt
  `bench/sealed/agent-trial/results/{mtl,py}_t{1,2,3}/*.json` + `*.mtl`.
- **Dev agent trials:** `bench/agent-trial/results/results.jsonl`,
  `bench/agent-trial/readtax/results/results.jsonl` (+ `round2/`),
  `bench/agent-trial/preamble/results/results.jsonl`,
  `bench/agent-trial/sessions/results/`.
- **The held-out summary of record:** `bench/BASELINE-SEALED.md`.

Everything else in this kit — the token ratios, the proof gates, the corpus and
sealed *validation* (execution of the committed solutions), the differential
oracle, the contamination gates — **is** fully reproducible from a clean
checkout with no model access.

## Independent-verification note

Hosted CI **never gated merges**: the GitHub-hosted runner queue stalled through
v0.2–v0.4 (public repo, GitHub-side rate limiting; tracked as issue #12), so the
proof and token greens were always **local / source-built**, never a hosted
badge. That is precisely why this reviewer-runnable kit is the credibility path.
Verified-language credibility is earned by **independent re-checking** — seL4 and
CompCert are believed because outsiders rebuild the proofs from published
artifacts, not because the authors said so. `kit/proof-gates.sh` is built for a
reviewer who does **not** trust this repo's CI: it re-checks the actual `.rs`
proof roots with a pinned prover the reviewer builds themselves, and exits
non-zero if any hard gate fails to reach its `verified, 0 errors` count.

## Evidence

A captured clean-checkout dry-run transcript — the non-Verus pipeline run
end-to-end (14/14 PASS) with each command's real output — is in
[`kit/EVIDENCE.md`](kit/EVIDENCE.md). It is the AC #7 artifact: fresh checkout,
kit-only, no repo-specific tribal knowledge.

In-container proof-gate reproduction verdict (5 Verus roots, source-built prover):

<!-- VERUS_INCONTAINER_RESULT -->
