# MTL Held-Out (Sealed) Results — issue #53

**The first out-of-sample MTL measurement.** A 15-task sealed split was authored
blind (task semantics only, no spec/quickref/corpus access — see
`bench/sealed/AUTHORSHIP.md`), frozen before any solution existed (freeze commit
`2992216`, seal commit `456292b`, tag `sealed-freeze-issue-53`), then unsealed
exactly once, post-freeze, to author reference solutions and run the held-out
static-compression and cold-agent batteries. Every number below traces to a
checked-in artifact.

## 1. TL;DR — headline verdict up front

- **Held-out static compression ≈ 1.67x (o200k) / 1.72x (cl100k)** over the 14
  algorithmically-correct sealed tasks (dev-parity constructed-stack validation).
  This is **BELOW the ≥3x Abrash gate** and **below the in-sample dev figure of
  3.72x (micro T_v0) – 3.87x/3.92x (tier-2)**.
- **Held-out agent success: 100% pass@5 (MTL) = 100% pass@5 (Python)** on the 7
  text-feedable sealed tasks — **equal to dev** (dev was 100% = 100%).
- **The ≥3x compression gate does NOT survive out-of-sample.** The agent-writability
  (pass@5) result does.
- Per issue #53's **MEASURE gate**, an honestly-measured miss accompanied by a
  post-mortem is a **successful outcome** for #53: the deliverable is the
  trustworthy held-out number, not a number that clears a bar. The gate result is
  reported as measured. **Nothing in the frozen language was changed** to chase it
  (no glyph, primitive, interpreter semantic, spec clause, or quickref entry was
  added or modified in response to any sealed task — §10.1).

One honest nuance carried throughout: while the **static** compression fell, the
**agent-trial marginal efficiency** (correct-solutions-per-million-tokens, MTL/Py)
actually *rose* out-of-sample, from 1.274 (dev) to 2.124 (sealed). The static
aggregate is dragged down by short scalar-arithmetic tasks where idiomatic Python
is already terse; those tasks are not in the text-feedable trial subset.

## 2. Sealed set composition

- **15 tasks**, tier split **6 micro / 6 tier-2 / 3 tier-3** (`bench/sealed/tasks.json`).
- **Blind authorship.** Tasks were authored from task semantics alone, with a
  documented information barrier: no reading of `docs/mtl-spec.md`, the quickref,
  any glyph/primitive docs, the dev corpus, or any `.mtl` file. Only the list of
  existing dev task *names* was consulted, for de-duplication. Full provenance and
  the avoid-list check: `bench/sealed/AUTHORSHIP.md`.
- **Disjointness proof (mechanical).** `bench/dataset/tests/contamination.rs`:
  - `sealed_disjoint_from_dev` — the 15 sealed tasks are hash-disjoint AND
    id-disjoint from the 10 agent-trial dev tasks (io_hash sets do not intersect;
    id sets do not intersect).
  - `planted_dev_task_collision_is_caught` — reproduces a real dev task's exact I/O
    and proves the gate *bites*.
  - `manifest_matches_sealed_tasks` — the committed salted manifest is reproducible
    from `bench/sealed/tasks.json`.
- **Freeze record.** Seal commit `456292bb3a0b930f51c49fa50e37c339ae4eaf59`
  (sealed set + salted manifest fixed); freeze commit `2992216` (measurement freeze
  declared); tag `sealed-freeze-issue-53`. Verify with:
  `git log -- bench/sealed/tasks.json bench/dataset/sealed/sealed.manifest.json`
  (fixed at the seal commit, unchanged after except filling `freeze_commit`) and
  `cargo test -p mtl-datagen manifest_matches_sealed_tasks`. See
  `bench/sealed/FREEZE.md`.

## 3. Held-out static compression

Validation is **dev-parity**: solutions run on the real `mtl-core` interpreter with
a **constructed input stack** (real `Value::Int` / `int_list`, negatives included),
bypassing the text lexer — exactly how the dev `BASELINE-TIER2` 3.87x is validated.
A task is *algorithmically correct* iff its committed frozen-glyph solution passes
**all** its vectors (incl. negatives) to HALT with the expected stack
(`bench/validate/tests/sealed.rs::committed_solutions_pass_all_vectors_constructed_stack`).
`ratio = py_idiomatic_tokens / mtl_tokens`; token counts from `bench/tokcount`
(tiktoken 0.8.0, one trailing newline stripped). Source of truth:
`bench/sealed/results/static_tokens.json` + `bench/sealed/corpus/<task>/`.

### Per-task (all 15)

| task | tier | algo-correct | text-feedable | mtl o200k | py o200k | ratio o200k | mtl cl100k | py cl100k | ratio cl100k | gap class |
|---|---|:-:|:-:|--:|--:|--:|--:|--:|--:|---|
| seal_collatz_steps | micro | yes | yes | 23 | 52 | 2.26 | 24 | 52 | 2.17 | none |
| seal_digit_product | micro | yes | no | 28 | 29 | 1.04 | 28 | 29 | 1.04 | input_encoding(scalar) |
| seal_count_set_bits | micro | yes | no | 22 | 16 | 0.73 | 22 | 16 | 0.73 | input_encoding(scalar) |
| seal_triangular | micro | yes | yes | 6 | 17 | 2.83 | 6 | 17 | 2.83 | none |
| seal_int_sqrt | micro | yes | yes | 15 | 39 | 2.60 | 15 | 38 | 2.53 | none |
| seal_num_divisors | micro | yes | yes | 18 | 44 | 2.44 | 18 | 43 | 2.39 | none |
| seal_alternating_sum | tier2 | yes | no | 9 | 39 | 4.33 | 9 | 39 | 4.33 | input_encoding(list) |
| seal_running_max | tier2 | **NO** | no | — | — | — | — | — | — | algorithmic |
| seal_count_local_maxima | tier2 | yes | yes | 43 | 60 | 1.40 | 38 | 60 | 1.58 | none |
| seal_xor_reduce | tier2 | yes | yes | 3 | 25 | 8.33 | 3 | 25 | 8.33 | none |
| seal_max_adjacent_diff | tier2 | yes | no | 42 | 45 | 1.07 | 39 | 45 | 1.15 | input_encoding(list) |
| seal_dedup_adjacent | tier2 | yes | no | 20 | 37 | 1.85 | 19 | 37 | 1.95 | input_encoding(list) |
| seal_rle_flatten | tier3 | yes | no | 45 | 52 | 1.16 | 43 | 52 | 1.21 | input_encoding(list) |
| seal_digit_sum_base | tier3 | yes | yes | 23 | 37 | 1.61 | 22 | 36 | 1.64 | none |
| seal_min_running_balance | tier3 | yes | no | 22 | 41 | 1.86 | 22 | 41 | 1.86 | input_encoding(list) |

### Aggregates

**Primary — dev-parity aggregate (14 algorithmically-correct tasks)**, token-SUM
ratio, apples-to-apples with dev tier-2's 3.87x:

| scope | py o200k | mtl o200k | ratio o200k | py cl100k | mtl cl100k | ratio cl100k |
|---|--:|--:|--:|--:|--:|--:|
| overall (14) | 533 | 319 | **1.67** | 530 | 308 | **1.72** |
| micro (6) | 197 | 112 | 1.76 | 195 | 113 | 1.73 |
| tier2 (5) | 206 | 117 | 1.76 | 206 | 108 | 1.91 |
| tier3 (3) | 130 | 90 | 1.44 | 129 | 87 | 1.48 |

**Secondary — text-feedable subset (7 tasks, no negative inputs)**, the property
the cold-agent trial exercises:

| scope | py o200k | mtl o200k | ratio o200k | py cl100k | mtl cl100k | ratio cl100k |
|---|--:|--:|--:|--:|--:|--:|
| overall (7) | 274 | 131 | **2.09** | 271 | 126 | **2.15** |
| micro (4) | 152 | 62 | 2.45 | 150 | 63 | 2.38 |
| tier2 (2) | 85 | 46 | 1.85 | 85 | 41 | 2.07 |
| tier3 (1) | 37 | 23 | 1.61 | 36 | 22 | 1.64 |

## 4. Held-out agent trial

`T_sealed-trial`: cold agents solve the **7 text-feedable** sealed tasks, 3 trials
per arm, N=5 repair budget, deterministic validation
(`bench/sealed/agent-trial/validate_one.py`). Aggregated by the **same**
`bench/agent-trial/report.py` used for dev, pointed at the sealed per-attempt
records (42 cells = 7 tasks × 2 arms × 3 trials; 46 attempts) and the sealed
`payload.json` (cold quickref **4051** o200k / **4037** cl100k). Source of truth:
`bench/sealed/agent-trial/results/metrics.json` + `REPORT.md`.

| Metric | MTL | Python |
|---|--:|--:|
| pass@5 (P(correct ≤ 5)) | **100.0%** (21/21) | **100.0%** (21/21) |
| Mean attempts to first correct | 1.19 | 1.00 |
| Median attempts to first correct | 1 | 1 |
| **Marginal** median output tokens/solution | **19** | **57** |
| Marginal mean output tokens/solution | 23.43 | 49.76 |
| Total output tokens (all cells, incl. failed repairs) | 492 | 1045 |
| **CSPM** (correct-solutions per 1e6 tokens) | **42682.93** | **20095.69** |

- **CSPM ratio (MTL / Python) = 2.124** — the marginal-efficiency edge (charges
  failed repairs, excludes the one-time quickref).
- **Marginal output-tokens/solution**: MTL is 3.0x tighter on the median (19 vs 57)
  and 2.12x tighter on the mean (23.43 vs 49.76). The per-solution compression
  survives out-of-sample.
- **Total accounting incl. cold quickref**: MTL pays 4051 o200k once. Cold total
  per solve MTL **4070** vs Python **57**. Amortized over the 7 sealed tasks the
  quickref adds 4051/7 = 578.7 tokens/task, so amortized MTL ≈ 597.7 (median) vs
  Python 57. Under full cold accounting Python dominates.
- **Break-even task count** = 4051 / (49.76 − 23.43) = 4051 / 26.33 = **≈154 tasks**
  (mean-savings basis; ≈107 tasks on the median-savings basis 57−19=38). Only after
  ~154 held-out solves does MTL's marginal per-solution savings repay the fixed cold
  quickref cost. Same framing as issue #80's economic model
  (`bench/agent-trial/sessions/session_econ.py`, `dO_breakeven_*`); here the fixed
  cost is the grown 4051-token quickref and the per-task output saving is 26.33
  tokens.
- **Repair.** 3 MTL cells failed attempt 1; all 3 repaired within budget (100%
  repair rate, mean 2.33 attempts). Python never failed attempt 1. Fault mix (MTL):
  1 Underflow, 1 parse, 2 wrong_output. Python: none.

`static_edge_survives_total_accounting = true` — but precisely: the flag is defined
on **CSPM** (marginal, quickref excluded), where MTL ≥ Python. On the **cold total**
view (quickref included) Python wins 57 vs 4070 until ~154 tasks amortize the
quickref. The marginal edge survives; the cold total-accounting edge does not.

## 5. THE DEV-vs-SEALED DELTA TABLE

Read this first. "Dev" = in-sample; "Sealed" = held-out (first out-of-sample MTL
measurement). Static rows compare the constructed-stack-validated aggregates
(sealed dev-parity 14-task vs dev tier-2 11-task, same validation method); the dev
micro T_v0 3.72x is noted where relevant.

| Metric | Dev (in-sample) | Sealed (held-out) | Delta (sealed − dev) | Verdict |
|---|---|---|---|---|
| Static compression, o200k | 3.87x (tier-2) · 3.72x (micro T_v0) | **1.67x** | −2.20x vs tier-2 | **BELOW ≥3x gate** — gate does NOT survive out-of-sample |
| Static compression, cl100k | 3.92x (tier-2) | **1.72x** | −2.20x | **BELOW ≥3x gate** |
| Per-tier compression, o200k | micro 3.72x · tier-2 3.87x | micro 1.76x · tier2 1.76x · tier3 1.44x | all tiers down | every tier below 3x out-of-sample |
| Agent pass@5, MTL | 100% | **100%** | 0 | **HOLDS** — success survives out-of-sample |
| Agent pass@5, Python | 100% | **100%** | 0 | HOLDS (control unchanged) |
| CSPM ratio (MTL / Python) | 1.274 | **2.124** | +0.850 | MTL marginal efficiency edge WIDENS out-of-sample |
| Median MTL output tokens/solution | 10 | 19 | +9 | sealed tasks longer; Python median rose 17 → 57 |
| Quickref cold cost (o200k) | 2157 | 4051 | +1894 | quickref grew as primitives were admitted → worsens cold total-accounting |

**One-line reading:** the ≥3x *static compression* gate fails out-of-sample (3.72–
3.87x → 1.67x), while *agent success* (pass@5 = 100%) and the *marginal* per-solution
token edge (CSPM ratio 1.274 → 2.124) both hold or improve. The claim that survives
is "MTL is agent-writable and marginally tighter per solution," not "MTL compresses
≥3x on unseen tasks."

## 6. Post-mortem (MEASURE gate) — why compression dropped out-of-sample

Honest, self-critical analysis. The dev 3.72–3.87x was measured on tasks and a
solution set that co-evolved with the language; the sealed 1.67x is the same
measurement on tasks the language never saw. The gap is the benchmark-fitting the
sealed set was designed to expose.

- **(a) Fair, natural Python.** The Python arm is idiomatic (not code-golf). On
  sealed tasks that use standard library idioms, Python is genuinely short, so the
  ratio has less headroom.
- **(b) Sealed micro tasks skew to short arithmetic where Python is already terse.**
  `seal_triangular` is 6 vs 17 tok (2.83x — still good), but the *scalar-input*
  micro tasks are the drag: `seal_digit_product` 28 vs 29 (1.04x) and
  `seal_count_set_bits` 22 vs 16 (**0.73x — Python wins**). Idiomatic Python has
  built-in `bin(n).count("1")`-style terseness MTL cannot beat with explicit
  stack arithmetic. These two tasks pull the 14-task aggregate down materially.
- **(c) Control-flow-heavy tasks bloat MTL.** `seal_count_local_maxima` is 43 vs 60
  (1.40x) and `seal_rle_flatten` 45 vs 52 (1.16x): windowed scans and run-length
  state cost many stack-shuffle glyphs, eroding the fold advantage that dominates
  the dev tier-2 corpus.
- **(d) Dev-admitted primitives were benchmark-fitting.** The cheap dev wins lean on
  primitives like `(` (Fold) and `$` (Xor) that were admitted under dev-corpus token
  pressure (see `bench/BASELINE-TIER2.md`: single_number's WALL "cleared in v0.3 by
  `$`"). `$` pays off spectacularly on the one sealed task shaped like its motivating
  case (`seal_xor_reduce`, 8.33x) but does nothing for the scalar-arithmetic and
  scan-shaped sealed tasks — classic overfitting the blind set doesn't reward.
- **(e) The quickref grew, worsening cold total-accounting.** As primitives were
  admitted the cold quickref grew **2157 → 4051 o200k tokens** (+1894). That fixed
  cost is what pushes the break-even out to ~154 held-out tasks and makes the cold
  total-per-solve 4070 vs Python's 57. More primitives improved in-sample static
  compression but made the cold economics *worse*, not better.

**Net.** The static ≥3x gate does not generalize; it was partly an artifact of
co-evolving tasks, solutions, and primitives. What *does* generalize is agent
writability (100% pass@5) and a real — even widened — marginal per-solution token
edge (CSPM ratio 2.124). Reporting the miss with this taxonomy is the anti-gaming
payoff #53 was built to produce.

## 7. Findings the blind sealed set surfaced

The blind set found real things the in-sample harness had hidden:

- **(i) No negative-integer input encoding in the text/REPL harness.** In the frozen
  grammar `-5` lexes as the `Sub` primitive (there is no negative literal), so the
  text `mtlrun` harness faults *before the solution runs* on any negative scalar or
  negative list element. The dev Rust-stack harness masked this by building inputs as
  runtime `Value`s. **7 of the 8 text-harness "gaps" are input-encoding artifacts**:
  2 `input_encoding(scalar)` (trivially fixable at the text boundary as `0 N -`; not
  a language gap) and 5 `input_encoding(list)` (a genuine text-encoding limit, but
  the values are representable as runtime `Value`s, so the solutions are
  algorithmically correct under constructed stack). See `bench/sealed/GAPS.md`.
- **(ii) A genuine algorithmic bug the text harness had hidden.** Full
  constructed-stack validation caught that the authored `seal_running_max` candidate
  seeds the running maximum at `0`, so an all-negative input `[-5 -2 -8 -1]` yields
  `[0 0 0 0]` instead of `[-5 -2 -2 -1]`. It "passed" the text harness earlier only
  because that harness could never present the all-negative vector — the input-
  encoding limit masked the bug. This is the **1 real algorithmic gap**: **14/15
  sealed tasks are algorithmically correct**, 1 is a genuine defect in the authored
  program. Pinned in `bench/validate/tests/sealed.rs::running_max_candidate_is_algorithmically_wrong`
  and `bench/sealed/GAPS.md`.

Per §10.1, **nothing was patched** — no glyph, primitive, semantic, spec, or
quickref changed in response to any gap; only validation tests were added.

## 8. Contamination-gate proof

- The manifest `bench/dataset/sealed/sealed.manifest.json` is real and salted:
  schema `mtl-sealed-manifest/v2`, salt `mtl-sealed-v1:issue-53`, 15 entries, each
  with `content_sha256` and `io_hash` (`canonical_sha256` withheld pre-freeze).
- `cargo test -p mtl-datagen` proves, mechanically:
  - `sealed_disjoint_from_dev` — sealed ↔ dev are hash- and id-disjoint.
  - `planted_dev_task_collision_is_caught` — the gate **bites** on a training row that
    reproduces a dev task's exact I/O behavior.
  - `manifest_matches_sealed_tasks` — the committed manifest is reproducible from
    `bench/sealed/tasks.json` (schema/salt/hashes all re-derived and asserted equal).
  - `clean_dataset_passes`, `committed_pilot_report_is_clean` — the pilot dataset is
    clean across the 15 sealed items.
- Run: `cargo test -p mtl-datagen` (all in `bench/dataset/tests/contamination.rs`).

## 9. Protocol judgment calls (documented honestly)

- **(a) Reference solutions ARE committed post-freeze.** For verifiability and
  apples-to-apples with dev (which commits solutions), the 14 correct sealed
  solutions live in `bench/sealed/corpus/<task>/`. Consequently the sealed set is now
  **"spent"** for future held-out use — a *fresh* blind set is needed for the next
  held-out measurement.
- **(b) Static validation uses dev-parity constructed stacks** (not the text
  harness), so the sealed compression number is comparable to dev's tier-2 3.87x
  (also constructed-stack-validated).
- **(c) The cold agent trial covers the 7 text-feedable tasks only.** The 8
  negative-input tasks are excluded by the text-harness input-encoding limitation
  (§7-i), not by any solver failure.
- **(d) Freeze + one-time unseal happened within a single session.** The
  tamper-evident salted manifest + freeze commit provide the fixed-in-advance
  guarantee; the multi-week seal window envisioned by the protocol is compressed into
  one session. The disjointness and manifest-reproducibility tests are what make the
  "fixed before solutions existed" claim checkable, not the wall-clock gap.

## 10. Reproduce

Every number above re-derives from committed artifacts:

```sh
# 0. On the frozen branch at/after the freeze commit.
git checkout issue-53-sealed-eval

# 1. Contamination gate + disjointness + manifest reproducibility (§8, §2).
cargo test -p mtl-datagen                 # contamination.rs: all 8 tests
cargo test -p mtl-datagen manifest_matches_sealed_tasks

# 2. Held-out static compression — constructed-stack validation (§3).
cargo test -p mtl-bench-validate --test sealed
#   committed_solutions_pass_all_vectors_constructed_stack  -> 14/14 correct
#   running_max_candidate_is_algorithmically_wrong          -> the 1 algorithmic gap
# Token counts (ground truth) for any solution:
python3 bench/tokcount/tokcount.py count_file bench/sealed/corpus/seal_triangular/*.mtl
# Aggregates: bench/sealed/results/static_tokens.json  (+ STATIC.md)

# 3. Held-out cold-agent trial aggregation (§4) — same report.py as dev.
#    (per-attempt records live under bench/sealed/agent-trial/results/<arm>_t<n>/)
python3 bench/agent-trial/report.py \
  --records-dir <flattened per-attempt *_a*.json> \
  --payload     bench/sealed/agent-trial/payload.json \
  --out         bench/sealed/agent-trial/results/REPORT.md \
  --json-out    bench/sealed/agent-trial/results/metrics.json
# Re-validate any candidate program deterministically:
python3 bench/sealed/agent-trial/validate_one.py --help

# 4. Whole workspace still green.
cargo test --workspace
```

Artifacts referenced: `bench/sealed/results/static_tokens.json` · `STATIC.md` ·
`bench/sealed/GAPS.md` · `AUTHORSHIP.md` · `FREEZE.md` ·
`bench/sealed/agent-trial/results/metrics.json` · `REPORT.md` ·
`bench/dataset/sealed/sealed.manifest.json` ·
`bench/dataset/tests/contamination.rs` · `bench/validate/tests/sealed.rs` ·
dev comparators `bench/BASELINE.md` · `bench/BASELINE-TIER2.md` ·
`bench/agent-trial/results/metrics.json`.
