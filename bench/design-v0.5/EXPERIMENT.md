# MTL v0.5 — Speculation admission experiment (measured)

Status: **experiment result**. This document runs the pre-registered
admission experiment specified in `docs/design/v0.5-refactor.md` §4.4 and
reports the verdict against the pre-registered decision rule. Semantics are
**FROZEN**: no core / syntax / host / perf verified file is touched. The
Arm-B side is driven by a **non-production** `SpecDriver` prototype
(`crates/mtl-arena-spike::spec`) over the arena spike; the experiment rig
lives entirely under `bench/design-v0.5/experiment/`.

The design doc is authoritative on the arm labels, and this report follows
it: **Arm A = the LLM attempt-loop (the current approach); Arm B = VM-level
speculative search (the thing under test).** (An earlier coordinator brief
swapped these labels; they are corrected here to match §4.4.)

---

## 1. Methodology

### The two arms

- **Arm A — LLM attempt-loop (current approach).** The model receives a fixed
  protocol prompt (the `quickref.md` MTL language manual + the task
  statement), emits one MTL program, and the host validates it with
  **deterministic `mtlrun`** (never an LLM judge — the validator is exact
  string/stack equality against the target). On a fault the model would
  receive the fault-carries-state snapshot (reified `Fault` + stack) and retry,
  verbatim, up to 5 attempts. Tokens (input + output, summed across all
  attempts) and wall-clock are measured to first validated solution.
- **Arm B — VM-level speculative search.** The model frames the search space
  **once** (alphabet, bounds, start, target — one prompt, one short
  completion). Then the `SpecDriver` enumerates/prunes candidate branches over
  cloned `VmState`s under a single global fuel budget `B = 50,000,000`,
  culling on fault at step speed with **no LLM in the inner loop**, dry-running
  `Invoke`s. It returns the first candidate whose final stack validates against
  the target (the *same* `mtlrun` predicate Arm A is held to).

### Fixed protocol / pinning

- **Model-under-test:** Claude Sonnet (pinned; recorded as `"model": "sonnet"`
  in every Arm-A trial file).
- **Token metric:** `o200k_base` (tiktoken) token counts, the pre-registered
  proxy. `quickref.md` = **2244 tokens**.
- **Validation:** deterministic `mtlrun` final-stack equality in both arms —
  identical bar, no model judgement.
- **Trials:** 3 per task per arm (Arm-A trials are near-identical; medians
  reported). Arm-B VM timing is best-of-3 median ms.

### Pre-analysis validity fix (blind, not p-hacking)

Two of the five pre-registered tasks have **provably infeasible** targets in
their declared spaces:

- **(c)** target `x = 1000` for `x := a*x + b`, `x0 = 1`, `a∈[1,8]`,
  `b∈[-8,8]`, `n∈[1,16]`: exhaustive enumeration + oracle confirms **no**
  `(a,b,n)` reaches 1000; the nearest reachable value is **1093**.
- **(e)** target `[4,3,2,1]` (full reversal) from `[1,2,3,4]` over
  `{swap,rot,over,drop,dup}`, length ≤ 8: the 4-element routing subgroup
  reachable by these ops has **diameter 2**, so no permutation needs ≥3 ops
  and the full reversal is unreachable at any length in this alphabet.

Both were found infeasible **before any Arm-A result was seen** — the fix is
blind to outcomes, so it is a validity correction, not outcome-tuning. The
corrected variants swap in reachable targets on the *identical* search spaces:

- **(c2)** target `1093` (nearest reachable; min-solution e.g. `1 6[3*1+].`).
- **(e2)** target `[1,2,3,4,4,4,4]` (min-solution `:::`, length 3).

**Primary verdict** honors the pre-registered five, with **c** and **e**
scored as **Arm-B-Exhausted non-wins** (Arm B correctly reports no solution
exists; Arm A was not run against an impossible target). **c2** and **e2** are
reported as **labeled secondary** results.

---

## 2. Per-task results

### 2.1 Primary tasks (pre-registered a, b, c, d, e)

Token columns are `o200k_base`. Arm A w/quickref = `enc(quickref) +
enc(statement) + enc(program)` (median of 3 trials). Arm A wo/quickref =
`enc(statement) + enc(program)` (sensitivity: isolates loop-vs-oneshot from
the language-manual overhead). Arm B = `enc(statement) +
enc(framing_completion)`. Wall-clock: Arm A = median Σ(t_end−t_start) over
trials; Arm B = VM search median ms (the one framing LLM call ≈ Arm A's one
generation in latency — see §2.3).

| task | Arm-B outcome | winning program | cands | VM ms (med) | cand/s | oracle |
|------|---------------|-----------------|------:|------------:|-------:|-------:|
| a | **found** | `123:*15087-` | 124 | 0.176 | 705,624 | 124/124 |
| b | **found** | `3 5+:*` | 48 | 0.067 | 720,483 | 48/48 |
| c | **exhausted** (target infeasible) | (none) | 2,176 | 28.046 | 77,587 | 2176/2176 |
| d | **found** | `1 3+2*3+2*3+2*2*` | 318 | 1.468 | 216,638 | 318/318 |
| e | **exhausted** (target infeasible) | (none) | 488,280 | 1741.671 | 280,351 | 228828/228828 |

| task | Arm-A solved? | attempts | Arm-A program | Arm-A wall s | A tok w/qr | A tok wo/qr | B tok | ratio (w/qr) | ratio (wo/qr) | wall ratio |
|------|:-------------:|:--------:|---------------|-------------:|-----------:|------------:|------:|-------------:|--------------:|-----------:|
| a | yes | 1 | `123:*15087-` | 0.0052 | 2400 | 156 | 176 | **13.64×** | 0.89× | 0.03× |
| b | yes | 1 | `3 5+:*` | 10.8973 | 2388 | 144 | 167 | **14.30×** | 0.86× | 163.6× |
| c | — (target infeasible; not run) | — | — | — | — | — | 269 | — | — | — |
| d | yes | 1 | `1 3+2*3+2*3+2*2*` | 13.0486 | 2427 | 183 | 207 | **11.73×** | 0.88× | 8.89× |
| e | — (target infeasible; not run) | — | — | — | — | — | 213 | — | — | — |

On the three *feasible* primary tasks (a, b, d) the quickref-included token
ratio is **≥10× on all three** (13.64×, 14.30×, 11.73×) — clearing the
`≥3/5` bar of condition 1 by the letter. The wall-ratio column is included
for completeness; it is dominated by LLM-call jitter (§2.3, §5) and is **not**
a reliable ≥5× signal — Arm A's fastest trial on task a was 3.6 ms, its
slowest 11.7 s, both for the same one-shot solve.

### 2.2 Secondary tasks (validity-fixed c2, e2)

| task | Arm-B outcome | winning program | cands | VM ms (med) | cand/s | oracle |
|------|---------------|-----------------|------:|------------:|-------:|-------:|
| c2 | **found** | `1 4[5*3+].` | 488 | 2.364 | 206,419 | 488/488 |
| e2 | **found** | `1 2 3 4:^^` | 143 | 0.284 | 503,394 | 143/143 |

| task | Arm-A solved? | attempts | Arm-A program | Arm-A wall s | A tok w/qr | A tok wo/qr | B tok | ratio (w/qr) | ratio (wo/qr) | wall ratio |
|------|:-------------:|:--------:|---------------|-------------:|-----------:|------------:|------:|-------------:|--------------:|-----------:|
| c2 | yes | 1 | `1 6[3*1+].` | 12.3013 | 2469 | 225 | 274 | 9.01× | 0.82× | 5.20× |
| e2 | yes | 1 | `1 2 3 4:::` | 12.2548 | 2459 | 215 | 251 | 9.80× | 0.86× | 43.1× |

Note Arm A and Arm B found *different but equally valid* programs (e.g. e2:
Arm A `:::`, Arm B `:^^`, both length 3 reaching `[1,2,3,4,4,4,4]`), confirming
the validator, not a fixed answer key, is the bar. The secondary quickref-
included ratios (9.01×, 9.80×) sit just under 10× — the same qualitative
picture as the primary tasks.

---

## 3. The decisive empirical finding

**The LLM one-shot every task.** All **15 Arm-A trials** (tasks a, b, d, c2,
e2 × 3 trials) were **solved on attempt 1 — zero retries**
(`solved_at_attempt: 1`, single-element `attempts[]`, `fault_feedback: null`
in every file). Not one trial invoked the repair loop.

The experiment was designed to test speculation on a **search-hard regime** —
tasks where the attempt-loop needs many retries (or fails) and a VM enumerating
at hardware speed pulls ahead. **That regime was never triggered.** These
"search-shaped" tasks turned out to be **LLM-one-shottable** by a modern
model: the alphabets are small, the targets are cheap to reason about, and
Sonnet emitted a first-try valid MTL program every time. The attempt-loop's
inner mechanism — the thing Arm B is supposed to beat — did not run at all.

---

## 4. Verdict against the pre-registered rule

The rule (§4.4) admits speculation iff **BOTH**:

> **(1)** on ≥3/5 tasks, Arm B reaches first validated solution using **≥10×
> fewer LLM tokens OR ≥5× less wall-clock** than Arm A; **AND**
> **(2)** Arm B arena results agree with the reference interpreter on **100%**
> of executed candidates.

Scored by the letter:

- **Condition 2 (oracle) — MET.** Every executed candidate agreed with the
  reference interpreter: a 124/124, b 48/48, c 2176/2176, d 318/318, e
  228828/228828, c2 488/488, e2 143/143 — **100% on every task.** The
  SpecDriver introduced no evaluation discrepancy.
- **Condition 1 (efficiency) — MET (via tokens).** Under the pre-registered
  quickref-included protocol, Arm B uses **≥10× fewer tokens on 3 of the 5
  tasks** — a=13.64×, b=14.30×, d=11.73× (c and e are Arm-B-Exhausted
  infeasible-target non-wins). Three tasks clear ≥10×; the `≥3/5` letter is
  satisfied. (The ≥5× wall-clock alternative is *not* met anywhere — see §5.)

**By the letter, the rule is SATISFIED.** Both hard gates pass.

---

## 5. Failure analysis (the load-bearing caveat)

**The token win is entirely a prompt-overhead artifact, not evidence of
search efficiency.** Read this before acting on §4.

Because the LLM one-shots every task (§3), **Arm A never invokes the repair
loop.** So Arm B is not beating iterative guess-and-check — there was no
iteration to beat. What Arm B is actually beating is *"a one-shot solve that
happens to carry a ~2244-token language manual (`quickref.md`) in its prompt."*
The entire ~12–14× token gap is that fixed manual: Arm A's per-task payload is
2244 (quickref) + ~150 (statement) + ~10 (program) tokens, and Arm B simply
doesn't send the manual. The search mechanism contributes nothing to the win.

Three facts make this unambiguous:

1. **The sensitivity column inverts the result.** With `quickref.md` excluded
   (Arm A tok = statement + program only), the ratios flip to **0.82–0.89×
   across all five tasks** — Arm A becomes *cheaper* than Arm B, because Arm
   B's one-shot framing completion is about the same size as Arm A's
   statement+program and Arm A no longer carries the manual. **Speculation
   wins 0/5 once the language-reference overhead is removed.**
2. **Speculation never wins on wall-clock.** Both arms are a **single LLM
   call** (Arm A: one generation; Arm B: one framing call, ~the same
   single-call latency). The VM search then *adds* time on top of that call
   — sub-millisecond for a/b/e2, ~1.5–2.4 ms for d/c2, ~28 ms / ~1.7 s for the
   exhausted infeasible c/e. It never *replaces* LLM round-trips (there were
   none to replace), so it can only add latency, not save it. The **≥5×
   wall-clock bar is met nowhere.** The wall-ratio table columns are dominated
   by LLM-call jitter (task a Arm-A trials ranged 3.6 ms → 11.7 s for the
   identical one-shot), not by any Arm-B mechanism.
3. **The mechanism under test was never exercised.** The rule's letter is met
   *only* through the language-reference overhead, not the search-efficiency
   mechanism the experiment was designed to measure.

So: **rule met by the letter, but the result is an artifact.**

---

## 6. Recommendation

**This is an artifactual / inconclusive pass. Do NOT green-light building
host-layer speculation on this evidence.**

The intended discriminator — genuinely LLM-hard search tasks where the
attempt-loop needs ≥5 retries or fails outright — **was not exercised**,
because a modern model solved every task in one shot. The token "win" is a
prompt-overhead artifact that **inverts** under the quickref-excluded
sensitivity analysis and **never appears on wall-clock**. Admitting
speculation on this basis would be admitting it for a reason (a 2244-token
manual in Arm A's prompt) that has nothing to do with what speculation does.

What the experiment *does* establish, and stands:

- **The machinery is sound and correct.** The `SpecDriver` prototype runs, and
  the differential oracle agrees with the reference interpreter on **100% of
  every executed candidate** across all seven tasks — condition 2 is a genuine,
  clean pass. The `SpecDriver` correctly reports **Exhausted** on the two
  provably-infeasible targets (c, e) rather than hallucinating a solution.
- **The value case is unproven.** Correctness ≠ earning implementation.

**If the question is worth settling, re-run on tasks that are demonstrably
hard for the model-under-test** — tasks with a measured multi-retry Arm-A
loop, or a sub-100% one-shot solve rate. Only there could speculation show a
non-artifactual advantage (real avoided LLM round-trips, real wall-clock
savings). The current task set is too easy to discriminate the two arms.

**Keep this decoupled from the arena backend's own admission (PR #33).** The
measured O(n²) perf kills in `docs/design/v0.5-refactor.md` §§2–3, 6 (144× on
`sum_to(10k)`, 754× flat front-pop, O(1) fork) stand **independently** of this
experiment. This speculation result neither supports nor undermines the arena
backend's gated-yes recommendation; it speaks only to the separate question of
whether *host-layer speculative search* earns implementation — and the honest
answer, on this evidence, is **not yet proven**.

---

## Verdict, one line

**Rule met by the letter (oracle 100%; ≥10× tokens on 3/5 via the
quickref-included protocol), but the result is a prompt-overhead artifact
that inverts under the sensitivity analysis and never appears on wall-clock —
speculation does NOT earn implementation on this evidence; re-run on
demonstrably LLM-hard tasks before any admission.**
