# MTL — Minimal Token Language

MTL is a concatenative, stack-based language whose writers are agents, not humans. Programs are point-free sequences of self-delimiting single-character words, chosen so that BPE tokenizers frequently merge adjacent primitives into one token — pushing the effective cost *below* one token per primitive. The reference semantics, interpreter refinement, parser round-trip, and Turing-completeness are all machine-checked in [Verus](https://github.com/verus-lang/verus). Full spec: [`docs/mtl-spec.md`](docs/mtl-spec.md).

## The one metric that matters

Over a task distribution `T` and a fixed tokenizer set (o200k_base, cl100k_base, Claude):

> minimize `E[t~T][ tok(sol(t, MTL)) ]`, subject to MTL being Turing complete.

The real headline is `E[tokens × attempts]`: a token-cheap language agents can't reliably write loses.

**The original gate (Abrash rule), stated honestly:**

> **≥3× token reduction vs. idiomatic Python, at equal-or-better agent success.** Below that, it's a curiosity and we say so.

**Verdict: the ≥3× gate FAILED out-of-sample.** It held only on dev corpora that co-evolved with the primitives — language and benchmarks were tuned together, so 3.7–3.9× was an in-sample artifact. On a blind held-out set the same measurement is **1.67×**; on a broad generated distribution ~1.7×; against fair terse Python ~1.03×. We report that plainly rather than defend the number.

**What MTL actually ships on** (each measured out-of-sample):
1. **Reliability** — 100% pass@5 agent writability on unseen tasks.
2. **Per-solution economics** — 2.124× correct-solutions-per-million-tokens held-out, widening with difficulty.
3. **Verified confinement** — a machine-checked core (5 proof roots) with capability confinement.

Compression survives as a *niche* property (2–4× on loop/fold shapes, ≤1× elsewhere), not the headline.

### Measured results

Compression measures two different numbers depending on the corpus, and they disagree — so we lead with the honest one, the blind held-out set.

| Corpus | Basis | Static compression vs. Python | Status |
|---|---|---|---|
| **Blind held-out (sealed)** | out-of-sample, never tuned against | **1.67×** | the number that generalizes |
| Generated distribution (1,145 shapes) | broad, mechanically sampled | ~1.7× | corroborates held-out |
| vs. fair terse Python | equal-effort human baseline | ~1.03× | compression ≈ break-even |
| T_v0 (frozen core), in-sample | co-evolved with primitives | 3.72× | in-sample only — see gate note |
| Tier-2 (11 tasks), in-sample | co-evolved with primitives | 3.87× / 3.92× | in-sample only — see gate note |

Compression is a **niche** property: 2–4× on loop/fold shapes, ≤1× on scans and builtin-heavy code. It is not the thesis. What generalizes, all measured out-of-sample:

- **Agent writability:** 100% pass@5 on unseen tasks — cold agents write correct MTL reliably.
- **Per-solution economics:** 2.124× correct-solutions-per-million-tokens on the held-out set, widening with task difficulty.
- **No measurable read-tax:** agents comprehend MTL as readily as they write it.
- **Verified core + confinement:** 5 machine-checked proof roots (semantics, interpreter refinement, parser round-trip, Turing-completeness) plus capability confinement.
- **Cheap cold start:** a 487-token preamble is all an agent needs to begin.

Also measured (in-sample dev corpora, retained for the record): in-sample static median 9 MTL tokens vs Python's 17; in-sample cold-LLM trial CSPM edge 1.27× (`claude-opus-4-8`, no fine-tuning, 10 tasks × 3 trials/arm, both arms 100% within 5 attempts) — the same correct-solutions-per-million-tokens metric as the 2.124× held-out figure, on the dev set rather than the sealed set; Tier-3 exec density 1.90× (its case is capability-confinement/safety, not compression); runtime ~35M interpreter steps/sec ([`crates/mtl-perf/PERF-BASELINE.md`](crates/mtl-perf/PERF-BASELINE.md); perf is an explicit non-goal).

Sources — held-out: [`bench/BASELINE-SEALED.md`](bench/BASELINE-SEALED.md), [`docs/design/v0.8-generalization.md`](docs/design/v0.8-generalization.md). In-sample (historical): [`bench/BASELINE.md`](bench/BASELINE.md), [`bench/BASELINE-TIER2.md`](bench/BASELINE-TIER2.md). Tier-3 capability-confinement case: [`bench/BASELINE-TIER3.md`](bench/BASELINE-TIER3.md). Cold-agent trial: [`bench/agent-trial/`](bench/agent-trial/).

## Proof scoreboard

All properties are machine-checked in Verus (pin `0.2026.07.05.49b8806`), admit-free. Counts are from the actual artifacts.

| # | Property | Statement | Status |
|---|---|---|---|
| **P1** | Determinism | `spec_step` is a total function; §4.1 rules non-overlapping, fault precedence faithful | ✅ by construction |
| **P2** | Refinement | `exec_step` faults exactly when `spec_step` does, else reaches the same next state | ✅ proved |
| **P3** | Progress | every state is Next, Halt, or Fault — no stuck states | ✅ proved |
| **P4** | Parser round-trip | `parse(print(p)) == p` and `print(parse(s)) == canonicalize(s)`, over the char-sequence model | ✅ proved (model level) |
| **P5** | Turing completeness | lock-step simulation of a two-counter Minsky machine, unbounded `nat` counters as unary quotations | ✅ proved |

Artifacts: core [`crates/mtl-core/src/mtl_core.rs`](crates/mtl-core/src/mtl_core.rs) → **76 verified, 0 errors** (P1–P3); [`crates/mtl-core/src/p5_universality.rs`](crates/mtl-core/src/p5_universality.rs) → **118 verified, 0 errors** (six theorems, two-way fuel-quantified halting); [`crates/mtl-syntax/proofs/p4_verus.rs`](crates/mtl-syntax/proofs/p4_verus.rs) → **42 verified, 0 errors**.

**Honest boundaries** (documented in-repo, not hidden):
- The **production interpreter is oracle-pinned, not extracted.** The verified core is a Verus model; the shipped Rust interpreter is tied to it by a differential proptest oracle, not proven in-tool to refine it.
- **P4 is model-level (b), not full refinement (c).** The proof is over a `Seq<char>` model; the shipped Vec-based parser is oracle-pinned to it.
- **Two `Clone` stubs remain** `external_body` (`Word::clone`, `Value::clone`) — Verus rejects derived Clone on the recursive quote type; `--no-cheating` flags exactly these two and nothing else.
- **P6–P9 are open** (tail-call space bound, heap acyclicity, resource/leak split, checker soundness) — see spec §7.5 / §14. Deliberately future work.

## The language at a glance

25 glyphs total: 23 single-character word-primitives plus the `[` `]` quotation delimiters.

- **Stack:** `:` dup · `_` drop · `~` swap · `@` rot · `^` over
- **Control / apply:** `!` apply · `'` dip · `?` if
- **Data:** `,` cat · `;` cons · `>` uncons
- **Arithmetic:** `+` `-` `*` `/` `%`
- **Comparison / bitwise:** `=` `<` · `$` xor
- **Recursion:** `&` primrec · `.` times · `|` linrec · `(` fold

Full table with stack effects and fault semantics: spec §5 ([`docs/mtl-spec.md`](docs/mtl-spec.md)) and the [quick reference](docs/mtl-quickref.md). Cold agents writing **pure-computation** MTL should start from the 487-token [minimal quick reference](docs/mtl-quickref-min.md) (the PR #88 ablation winner); reach for the full reference only when a task uses host capabilities.

## Effects & capabilities (v0.4)

Effects live entirely host-side, behind a single narrow channel. The pure verified core suspends at every capability call and yields a fourth outcome `Invoke(name, stack, cont)` instead of faulting; an unverified host runner services it and resumes the core. The core threads no host state and closes over nothing — which is what keeps P1/P2/P3 intact. Capabilities are a grant whitelist; metering is charged **before** the effect; cancellation (fuel or budget) happens only between steps, so no partial effect is possible; strings are opaque host-side `i64` handles (no `Value::Str` in the core). Confinement is covered by 7 tests in [`crates/mtl-host/tests/security_posture.rs`](crates/mtl-host/tests/security_posture.rs): not-granted is unreachable, budget exhaustion cancels with no partial effect, the output-byte cap is never exceeded, each invocation consumes its budget exactly once, and more.

## Arena execution backend (v0.5, now the default engine)

`crates/mtl-arena` is the **default execution backend** — an interned, persistent, O(1)-fork continuation engine that targets the reference interpreter's measured O(n²) hotspots (flat front-pop, primrec re-emission, fold tails). Its refinement obligation — that the arena refines `spec_step` — is **discharged as a machine-checked Verus proof** (`crates/mtl-arena/proofs/arena_verus.rs`, 145 verified / 0 errors, unconditional, admit/assume-free, with fault parity), so the arena is now the default execution path across the user-facing entry points (`mtlrun`, `tier3run`, the `mtl-host` driver, the corpus gate). The `mtl-core` interpreter is **not retired**: it remains the reference twin and **differential anchor**, reachable behind an explicit `--engine=interp` flag (or `mtl_host::driver::Engine::Interp` / `drive_interp` in the API). The engine selector defaults to `arena`; both backends share the same host seam (`mtl_arena::host::arena_drive` mirrors `mtl_core::host::drive` outcome-for-outcome, with the same global-fuel and cancellation guarantees). The arena adds **no new primitives and no new semantics**, so the language spec is unchanged, and the two engines are kept bit-identical by a continuously-run differential oracle (148 cases, **both engines**), fault-corpus (`FaultInfo`) parity, and host-driver parity — those twin runs are what make arena-as-default safe. Design and rationale: [`docs/design/v0.5-refactor.md`](docs/design/v0.5-refactor.md); see [`crates/mtl-perf/PERF-BASELINE.md`](crates/mtl-perf/PERF-BASELINE.md) and [`bench/design-v0.5/MEASUREMENTS.md`](bench/design-v0.5/MEASUREMENTS.md) for the measured speedups on the interpreter's O(n²) hotspots.

## Repository layout

```
MTL/
├── crates/
│   ├── mtl-core/     # verified reference semantics (Verus) + cargo interpreter + host seam + P5
│   ├── mtl-syntax/   # lexer, parser, canonical printer + P4 round-trip proof
│   ├── mtl-host/     # v0.4 host runner: capabilities, metering, handles (unverified, above the core)
│   ├── mtl-perf/     # runtime perf benchmarks (perf is a non-goal; measurement only)
│   └── mtl-arena/    # v0.5 arena execution backend — the DEFAULT engine (refinement-proved); interp reachable via --engine=interp as the differential anchor
├── bench/
│   ├── BASELINE.md, BASELINE-TIER2.md, BASELINE-TIER3.md   # compression measurements
│   ├── validate/     # parse+execute validation harness (mtlrun bin)
│   └── agent-trial/  # cold-LLM write trial + read-tax battery
├── docs/
│   ├── mtl-spec.md   # language specification (v0.4-draft)
│   ├── mtl-quickref.md
│   └── reviews/      # adversarial review
└── .github/workflows/  # ci.yml, perf-smoke.yml, tokreport.yml
```

## Build, test, verify

```sh
# build + test (stable Rust)
cargo test --workspace

# perf snapshot
cargo run --release --example perf_report -p mtl-perf

# formal verification (source-built Verus, pin 0.2026.07.05.49b8806)
verus crates/mtl-core/src/mtl_core.rs        # 76 verified, 0 errors
verus crates/mtl-core/src/p5_universality.rs # 118 verified, 0 errors
verus crates/mtl-syntax/proofs/p4_verus.rs   # 42 verified, 0 errors
```

Verus is not on crates.io. Build it from source at the pinned commit `0.2026.07.05.49b8806` (see the [verus-lang/verus releases](https://github.com/verus-lang/verus/releases)) and point `VERUS_Z3_PATH` at the bundled Z3 before invoking the `verus` binary above.

## Reproducing the benchmarks

```sh
# token counts vs Python (o200k_base / cl100k_base)
pip3 install -r bench/tokcount/requirements.txt
python3 bench/tokcount/report.py

# validate that every corpus MTL solution parses + executes
cargo run --bin mtlrun -p mtl-bench-validate
```

## CI status (read the counts honestly)

GitHub's hosted runners were stalled by GitHub-side rate-limiting through the v0.2–v0.4 work, so **CI did not gate these merges**. All verification evidence is **local, from source-built Verus** using the commands above. The workflow's P5 step is a hard gate and the core/P4 steps are advisory, but the runners haven't dispatched — so the proof counts here are reproducible locally, not (yet) attested by a green CI badge. We state this plainly rather than imply CI coverage that isn't there.

The workflow has since been hardened for reliability and given coverage + fuzz jobs (issue #58; root-cause and mitigations in [`docs/ci-reliability.md`](docs/ci-reliability.md)): per-job `timeout-minutes`, a `concurrency` group that cancels superseded PR runs (but never default-branch runs), a docs-only path filter, caching, and a bounded retry on the Verus download. A `coverage` job reports `cargo-llvm-cov` numbers (advisory), and a `fuzz` job runs the parser round-trip and the interp-vs-arena differential under `cargo-fuzz` (gating on any panic or divergence). **This does not change the honesty note above:** until a self-hosted verifier lands, proof evidence remains local source-built Verus — the reliability work reduces the stall, it does not by itself turn the badge green.

## Related work

Concatenative controls Forth and Joy, plus jq, a compact S-expression DSL, and idiomatic/minified/model-generated Python form the comparison panel (spec §10.3). The base `{dup, drop, cat, cons, apply}` follows Kerby's minimal concatenative core; the reference-counting direction draws on Perceus/Koka (spec §14, adversarial review).

## Roadmap — what's deliberately NOT done

- **§14 multiplicity checker + P6–P9** — frozen as future work; nothing in §14 is part of the verified core.
- **Warm agent trial** — the trial is cold-only; no fine-tuned MTL arm has been measured.
- **Indexed access** — `two_sum` / `binary_search` remain inexpressible (no O(1) random access); 2 of 13 tier-2 tasks hit this wall.
- **Definitions `#f[...]`** — deferred; not part of the core.
- **`Str` in the core** — strings stay host-side by measurement recommendation.
- **Continuation O(n²) refactor** — real cause identified (front-pop + re-emission); scoped to when a large-data workload is real, not urgent.
- **P4 level (c) / interpreter extraction** — full in-tool refinement of the shipped parser/interpreter is deferred; today they are oracle-pinned.

## License

See [LICENSE](LICENSE).
