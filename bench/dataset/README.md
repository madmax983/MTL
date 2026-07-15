# `mtl-datagen` — the CPU-only LoRA training-data factory

Builds oracle-validated `(instruction, response)` SFT pairs for the MTL
warm-agent fine-tune (design `docs/design/v0.7-lora-warm-agent.md` §3; tracking
issue #83). The crate's unfair advantage is a **verified oracle**: candidate MTL
programs are admitted only if the machine-checked interpreter
(`mtl_core::interp`, whose `spec_step` is Verus-proven) accepts them across an
adversarial input grid. Correctness is ~100% **by construction**, not by
sampling audit — this is execution-verified rejection sampling with a proven
verifier, so label noise is ~zero. **No LLM calls, no GPU** — pure Rust + the
existing interpreter (design §6 "buildable today").

## Pipeline stages

`generate task spec → generate candidate MTL → run the oracle → keep iff correct
→ canonicalize → dedup`, then harvest repair traces and meter coverage /
contamination.

| Module | Role |
|---|---|
| `families.rs` | Parameterized task-family generators (arithmetic, stack-shuffle, predicate, recursion, fold/traversal, quotation, capability). Each carries a difficulty knob; known-good MTL is seeded from the real `bench/corpus` / `bench/tier3` solutions and parameterized. |
| `candidates.rs` | Three deterministic candidate strategies: (1) template synthesis, (2) single/double-glyph **mutation** over the 23-glyph manifest, (3) **bottom-up enumeration** of short programs (the program *is* the spec, discovered by execution). |
| `oracle.rs` | The unified gate. Tiers 0–2: `mtl_core::interp::run` (FUEL = 100 000) over the adversarial grid (0, 1, −1, negatives, empty list, `i64::MIN`/`MAX`), comparing `Halt`/`Fault` to the Rust reference. Tier-3: `mtl_host::caps::task_setup` + `mtl_host::driver::drive`, PASS iff `Done` && `ctx.output_utf8() == expected_output`. |
| `canon.rs` | `mtl_syntax::print` canonicalization + SHA-256, plus the io-behavior-vector hash. Dedup by **both** keys. |
| `repair.rs` | Harvests `(broken, fault_turn, fixed)` triples from real captured `FaultInfo`, balanced across the four core fault kinds. |
| `sft.rs` | SFT record types + JSONL. Documents the Qwen2.5-Coder chat-template assumption. |
| `coverage.rs` | 23-glyph × tier × difficulty coverage meter; flags glyphs below a floor as holes. |
| `contamination.rs` | Sealed-set contamination gate (canonical-SHA-256 **or** io-hash). |

## Running

Pilot (committed under `pilot/`):

```
cargo run -p mtl-datagen --bin gen -- --count 1200 --out bench/dataset/pilot --seed 0
python3 bench/dataset/stats.py bench/dataset/pilot     # folds exact tiktoken totals into stats.json
```

**Full 30k recommended-v1 generation** (design §3 size math):

```
cargo run --release -p mtl-datagen --bin gen -- --count 30000 --out bench/dataset/full --seed 0
python3 bench/dataset/stats.py bench/dataset/full
```

Flags: `--count N` (target accepted **gen** pairs; repair traces are layered on
top at ~20% of the final dataset, so total ≈ `1.25 × N`), `--out DIR`, `--seed S`
(fully deterministic — no clock seeding; a different seed shifts the parameter
ranges), `--floor F` (coverage-hole floor, default 3). The run **fails with a
non-zero exit** if the contamination gate finds a collision or the inline
re-validation invariant breaks.

Outputs: `dataset.jsonl`, `coverage.json`, `contamination_report.json`,
`stats.json` (+ `stats_tokens.json` from `stats.py`).

## Record shapes

Two shapes, both under the **Qwen2.5-Coder-7B-Instruct native chat template with
completion-only loss** (loss on the assistant span only). Per design §2 the MTL
quickref / language spec is **not** injected into the training prompt — the model
internalizes MTL from the response distribution and emits it with zero preamble
tokens at inference.

* **generation** — `instruction` = task description (English for named families,
  an I/O-example spec for enumerated/discovered programs); `response` = canonical
  MTL.
* **repair** — `instruction` = broken program + captured fault turn (`FAULT:
  <kind>` + stack snapshot + faulting word); `response` = the fixed program.

Each record also carries `tier`, `family`, `difficulty`, `kind` (`gen`/`repair`),
`canonical_sha256`, `io_sha256`, and an embedded re-runnable `check` contract
(used by the re-validation test; ignored by the trainer).

## Tier mix, repair %, and the committed pilot

The design target is **T_v0 40% / tier-2 35% / tier-3 25%** with repair traces
layered ~20% across all tiers. The pilot is deliberately tier-0-heavy: the
parametric arithmetic families and the bottom-up enumerator supply thousands of
distinct tier-0 programs, whereas the corpus tier-2 folds and the **16**
`task_setup` tier-3 templates are near-fixed (tier-3 is hard-capped at 16 unique
programs — the design's own ceiling). The committed pilot (`--count 1200`,
`--seed 0`) is **1500 pairs**: 1200 gen + **300 repair (exactly 20%)**; tiers
0/2/3 = 1403/81/16. Reaching the 40/35/25 target at scale requires the
LLM-teacher candidate path (deferred/off-repo, design §3 source (a)); the CPU
factory here supplies the tier-0/2 bulk plus the full tier-3 template set.

Per-family acceptance is **1.0** for every template/enumeration family (correct
by construction); the **mutation** strategy's acceptance rate is ~**1.0%** (the
signal that most single-glyph edits break the oracle — the repair-trace fodder).
Repair fault kinds are balanced 75/75/75/75 across `Underflow`, `TypeMismatch`,
`DivByZero`, `Overflow`.

## Token accounting

`stats.py` computes **exact tiktoken `o200k_base`** (and `cl100k_base`) totals via
the in-repo `bench/tokcount/tokcount.py`. tiktoken o200k is used as a
**deterministic offline proxy** for the Qwen2.5-Coder tokenizer: both are
byte-level BPE, and `bench/tokcount/token_profile.json` confirms every one of the
23 MTL glyphs is exactly one token under o200k. The committed pilot totals
**48 524 o200k tokens over 1500 records (~32/example)** — below the design's ~90
blended tokens/example because these prompts carry no quickref preamble. A full
30k run lands in the low single-digit millions of SFT tokens (design §3 budget:
well under 10M for 2–3 epochs).

## Sealed / contamination discipline

`sealed/sealed.manifest.json` is the **real** salted contamination manifest
(schema `mtl-sealed-manifest/v2`), derived from the sealed task set at
`bench/sealed/tasks.json` (15 tasks authored blind to the MTL glyph/primitive
set; regenerate with `cargo run -p mtl-datagen --bin mkseal`). Each entry
carries a salted `content_sha256` tamper-evidence fingerprint and the task's
`io_hash`; `canonical_sha256` is empty until the post-freeze unseal (reference
solutions are withheld). See `sealed/README.md` and `bench/sealed/README.md`.
The gate asserts the dataset is hash-disjoint from the sealed set by
canonical-SHA-256 **and** io-behavior hash (empty keys never match), and is
CI-runnable (`tests/contamination.rs`), which also proves sealed⇔dev
disjointness and that a planted dev-task I/O reproduction is caught.

## Correctness proof — the re-validation invariant

`tests/revalidation.rs` reloads **every** row of the committed pilot, reconstructs
its embedded contract, and re-runs the response through the REAL oracle
(`mtl_core::interp::run` for tiers 0–2; `task_setup` + `drive` for tier-3),
asserting 100% still `HALT == reference` / `PASS`, plus canonical-form and hash
stability. A single bad row fails `cargo test` — correctness is a machine-checked
invariant, not a claim.
