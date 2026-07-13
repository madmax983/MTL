# MTL v0.4.0 — Effects, and a Fully Verified Core

Release date: 2026-07-13

## The arc: 0.1 scaffold → 0.4 effects + complete P1–P5 proof suite

v0.1 shipped a specification and a Verus *skeleton*: P1 (determinism) and P3 (progress) held by construction, and everything else was either stubbed behind `external_body` or stated as a conjecture. v0.4 closes that gap. The core is now machine-checked end to end — determinism, interpreter refinement, progress, parser round-trip, and Turing-completeness are all proved, admit-free — and the language grew a capability-based effects layer without disturbing any of it.

## What's new since 0.1

### Full proof suite (P1–P5)

- **P2 (refinement)** discharged (PRs #20–#23): the `exec_step` interpreter provably faults exactly when the spec does and otherwise reaches the same next state. Seven functions that were `external_body` stubs are now verified; the core went from **22 verified to 76 verified, 0 errors**. Only two `Clone` stubs remain, and they are intended — Verus rejects derived `Clone` on the recursive quote type, and `--no-cheating` flags exactly those two and nothing else.
- **P4 (parser round-trip)** proved at model level (PR #25): `parse ∘ print = id` and `print ∘ parse = canonicalize` over a `Seq<char>` model — **42 verified, 0 errors** — with the shipped Vec-based parser pinned to the model by a differential proptest.
- **P5 (Turing completeness)** proved as a theorem (PR #29): a two-counter Minsky machine simulated in lock-step over `spec_step`, counters encoded as unary quotations so the state space is genuinely unbounded — **118 verified, 0 errors**, six theorems including a two-way fuel-quantified halting correspondence.

### Capability-based effects (v0.4)

A new `mtl-host` crate (PRs #24, #26, #27) adds a host runner above the pure core. The core gained a fourth outcome, `Invoke`, and now suspends at every capability call instead of faulting; the host services it behind a grant whitelist, with metering charged before the effect and clean between-step cancellation (no partial effects). Seven confinement tests pin the security posture. Crucially, the core threads no host state, so P1/P2/P3 survive the addition untouched.

### Measurement

T_v0 clears the Abrash gate at **3.72×** vs idiomatic Python; tier-2 at **3.87× / 3.92×**. The cold-LLM agent trial (with the read-tax battery from PR #28) shows **100% solve on both arms**, a **1.27× correct-solutions-per-token** win for MTL, and no measurable read-tax. Runtime baseline: ~35M interpreter steps/sec.

## Honest findings along the way

Real projects have real potholes. These are the ones worth remembering.

- **The CI that never ran.** For much of this work GitHub's hosted runners were stalled by GitHub-side rate-limiting — and before that, a workflow bug let the Verus job report **green while silently skipping** the verify step (a job-level `continue-on-error` swallowed a missing-toolchain failure, and the pinned Verus release asset 404'd on a truncated tag). Both bugs were fixed in the workflow, but the runners still haven't dispatched. So **every proof in this release was checked locally with source-built Verus**, not by CI. We would rather say that plainly than show a badge that means nothing.
- **Validation caught wrong solutions.** An LLM-in-the-loop validator in the agent trial emitted false positives on three MTL cells — solutions it called correct that weren't. Deterministic re-validation through the interpreter (`mtlrun` parse + execute) caught them; all 67 records were re-run and re-validated with zero mismatches. The lesson stuck: the oracle is the interpreter, never the model.
- **The drive() fuel bug.** The first host driver reset its fuel budget at each `Invoke` yield, so a program looping through a capability forever would never be cancelled. The fix (PR #27) makes fuel a **single cumulative budget across resumptions**, so an endless effect loop terminates as `Cancelled` instead of hanging.
- **A withdrawn Turing-completeness proof.** An early P5 sketch used `i64` counters — which give a finite state space and therefore do *not* prove Turing-completeness. The 2026-07-11 adversarial review caught it; the conjecture was withdrawn and rebuilt on unbounded `nat` counters. The final P5 is a genuine theorem, not the original hand-wave.
- **Review-driven repairs.** That same review drove a batch of fixes now recorded in the spec changelog: lexer ambiguity resolved (unsigned integer literals; `-` is subtraction), fault precedence pinned (arity → type → semantic), the two-machine host split adopted, §14 frozen as explicit future work, and the Verus pin corrected to its full commit-suffixed release.

## Verifying this release

```sh
cargo test --workspace
verus crates/mtl-core/src/mtl_core.rs        # 76 verified, 0 errors
verus crates/mtl-core/src/p5_universality.rs # 118 verified, 0 errors
verus crates/mtl-syntax/proofs/p4_verus.rs   # 42 verified, 0 errors
```

Verus pin: `0.2026.07.05.49b8806`. See the [README](../README.md) for the full build / verify / reproduce recipe.

## Known limitations & follow-ups

- Proofs are attested **locally**, not by CI (runner stall, above).
- The production interpreter and parser are **oracle-pinned to the verified model, not extracted** from it; P4 is model-level (b), not full refinement (c).
- **P6–P9** (tail-call space bound, heap acyclicity, resource/leak split, checker soundness) and the **§14 multiplicity checker** are future work.
- **Indexed access** is inexpressible — `two_sum` / `binary_search` are out until it lands.
- The agent trial is **cold-only**; no warm / fine-tuned arm yet.
