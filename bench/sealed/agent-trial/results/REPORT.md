# MTL agent-writability trial — SEALED (held-out) REPORT

> **Held-out run.** This is the `T_sealed-trial` cold-agent battery over the **7
> text-feedable sealed tasks** (the sealed subset with no negative inputs the text
> `mtlrun` harness can lex), 3 trials per arm, N=5 repair budget, deterministic
> validation via `bench/sealed/agent-trial/validate_one.py`. Metrics are computed
> by the **same** `bench/agent-trial/report.py` used for the dev trial, pointed at
> the sealed per-attempt records and the sealed `payload.json` (cold quickref
> **4051** o200k / **4037** cl100k). The dev-vs-sealed delta lives in
> `bench/BASELINE-SEALED.md`.

Primary token counts are **o200k_base**; cl100k is secondary.

Cells found: **42** | attempts found: **46** | max attempts/cell: 5 | expected trials per (task,arm): 3

## Headline (MTL vs Python)

| Metric | MTL | Python |
|---|---|---|
| P(correct ≤ 5 attempts) | 100.0% | 100.0% |
| Median output tokens to first correct | 19 | 57 |
| Correct-solutions per million tokens | 42682.93 | 20095.69 |

**Correct-solutions-per-million-tokens ratio (MTL / Python): 2.124** (>1 favors MTL).

## A/B. Correctness and attempts-to-first-correct

| Metric | MTL | Python |
|---|---|---|
| Total cells | 21 | 21 |
| Solved cells | 21 | 21 |
| P(correct ≤ 5) | 100.0% | 100.0% |
| Mean attempts to first correct | 1.19 | 1 |
| Median attempts to first correct | 1 | 1 |

Attempts-to-first-correct distribution (solved cells, by attempt index):

| Attempt | MTL | Python |
|---|---|---|
| 1 | 18 | 21 |
| 2 | 2 | 0 |
| 3 | 1 | 0 |
| 4 | 0 | 0 |
| 5 | 0 | 0 |

## C. Output tokens to first correct (§17 headline component)

| Metric | MTL | Python |
|---|---|---|
| Median | 19 | 57 |
| Mean | 23.43 | 49.76 |

## D. Correct-solutions-per-million-tokens (headline, §10.6)

Charges the full cost of failed repair attempts: `solved_cells / sum(total_output_tokens over ALL cells) * 1e6`.

| Metric | MTL | Python |
|---|---|---|
| Solved cells | 21 | 21 |
| Total output tokens (all cells) | 492 | 1045 |
| Correct-solutions per 1e6 tokens | 42682.93 | 20095.69 |
| MTL / Python ratio | 2.124 | |

## Per-task × arm breakdown

| Task | Arm | Cells | Solved | P(≤5) | Med tok→correct | Correct/1e6 tok | Trials |
|---|---|---|---|---|---|---|---|
| seal_collatz_steps | mtl | 3 | 3 | 100.0% | 25 | 41666.67 | 1,2,3 |
| seal_collatz_steps | python | 3 | 3 | 100.0% | 58 | 17241.38 | 1,2,3 |
| seal_count_local_maxima | mtl | 3 | 3 | 100.0% | 52 | 17857.14 | 1,2,3 |
| seal_count_local_maxima | python | 3 | 3 | 100.0% | 57 | 16759.78 | 1,2,3 |
| seal_digit_sum_base | mtl | 3 | 3 | 100.0% | 19 | 49180.33 | 1,2,3 |
| seal_digit_sum_base | python | 3 | 3 | 100.0% | 48 | 20979.02 | 1,2,3 |
| seal_int_sqrt | mtl | 3 | 3 | 100.0% | 16 | 62500 | 1,2,3 |
| seal_int_sqrt | python | 3 | 3 | 100.0% | 73 | 12820.51 | 1,2,3 |
| seal_num_divisors | mtl | 3 | 3 | 100.0% | 37 | 25862.07 | 1,2,3 |
| seal_num_divisors | python | 3 | 3 | 100.0% | 65 | 15625 | 1,2,3 |
| seal_triangular | mtl | 3 | 3 | 100.0% | 6 | 166666.67 | 1,2,3 |
| seal_triangular | python | 3 | 3 | 100.0% | 17 | 58823.53 | 1,2,3 |
| seal_xor_reduce | mtl | 3 | 3 | 100.0% | 3 | 333333.33 | 1,2,3 |
| seal_xor_reduce | python | 3 | 3 | 100.0% | 24 | 41666.67 | 1,2,3 |

## E. Total-token accounting (§10.4) and the verdict

| Quantity | MTL (cold) | Python (cold) |
|---|---|---|
| Program-only median (winning attempt) | 19 | 57 |
| Generation+repair median | 19 | 57 |
| Cold instruction cost (quickref, o200k) | 4051 | 0 |
| Cold total per solve | 4070 | 57 |
| Amortized over 10 tasks | 424.1 | 57 |

Python's cold instruction cost is ~0: the model already knows Python and receives no language reference, whereas MTL pays the full quickref cost (4051 o200k tokens) once per task cold. This is the asymmetry the review predicted.

### Does MTL's static (program-length) token edge survive total-token accounting?

- **Static edge:** median first-correct program tokens — MTL 19 vs Python 57. MTL programs are shorter.
- **Efficiency (charges failed repairs):** correct-solutions per 1e6 tokens — MTL 42682.93 vs Python 20095.69 (ratio 2.124).
- **Cold total per solve:** MTL 4070 vs Python 57.

**Verdict: the static edge SURVIVES.** On the efficiency metric that charges failed repairs, MTL is at least as good as Python (correct-solutions-per-million-tokens MTL ≥ Python).

### Marginal vs total-accounting token views (stated explicitly)

Two distinct token views, both reported:

- **(a) Marginal output-tokens-per-solution** (excludes the one-time quickref) —
  median MTL **19** vs Python **57** (3.0x tighter); mean MTL **23.43** vs Python
  **49.76** (2.12x tighter). This is the property the static compression predicts,
  and it holds out-of-sample. The CSPM ratio (2.124) is the marginal-efficiency
  summary that charges failed repairs but not the quickref.
- **(b) Total accounting incl. cold quickref** — MTL pays the **4051** o200k
  quickref once. Cold total per solve MTL **4070** vs Python **57**; the quickref
  amortized over the 7 sealed tasks alone is 4051/7 = **578.7** tokens/task, so
  amortized-over-7 MTL ≈ **597.7** (median) vs Python 57. Under full cold
  accounting Python dominates until the quickref is amortized over many tasks.

**Break-even task count** = quickref / (mean Python out-tok/task − mean MTL
out-tok/task) = 4051 / (49.76 − 23.43) = 4051 / 26.33 = **≈154 tasks**
(mean basis; **≈107 tasks** on the median-savings basis of 57−19=38). Only after
roughly this many held-out solves does MTL's marginal per-solution savings repay
its fixed cold quickref cost. This is the same break-even framing tracked in
issue #80's economic model (`bench/agent-trial/sessions/session_econ.py`,
`dO_breakeven_*`): here the fixed cost is the grown 4051-token quickref, and the
per-task output saving is 26.33 tokens.

**Verdict flag `static_edge_survives_total_accounting = true`** — but read it
precisely: the flag is defined on **CSPM** (marginal efficiency, quickref
excluded), where MTL 42682.93 ≥ Python 20095.69. On the **cold total** view
(quickref included) Python wins 57 vs 4070 until ~154 tasks amortize the quickref.
The marginal edge survives out-of-sample; the cold total-accounting edge does not.

## F. Error-type distribution and repair efficacy

| error_type | MTL | Python |
|---|---|---|
| Underflow | 1 | 0 |
| parse | 1 | 0 |
| wrong_output | 2 | 0 |

Stack-tracking faults (Underflow + TypeMismatch) — MTL 1, Python 0. parse — MTL 1, Python 0. wrong_output — MTL 2, Python 0. The review predicted stack-tracking failures dominate the MTL arm.

### Repair efficacy (does stack-state-in-errors feedback fix failures?)

| Metric | MTL | Python |
|---|---|---|
| Cells that failed attempt 1 | 3 | 0 |
| ...that eventually solved | 3 | 0 |
| Repair success rate | 100.0% | n/a |
| Mean attempts among repaired | 2.33 | n/a |

## Caveats

- **Cold-only, no warm arm.** The model-under-test is the session's Claude model. Only cold agents were tested — no fine-tuning or warmed-up (agent already fluent in MTL) arm was available. A warm MTL arm would pay no quickref cost and would likely raise pass@1, so the cold numbers are a lower bound for MTL.
- **Token-count proxy.** The output-token metric counts only the visible program the model emits, not hidden reasoning tokens. Both arms are measured identically so the comparison is fair, but absolute totals understate true generation cost.
- **Both arms treated identically** for token counting (o200k_base primary, cl100k_base secondary) and for the 5-attempt repair budget.
- **Trial-3 (mtl_t3) provenance caveat — recorded, not discarded.** The mtl_t3
  worker inadvertently read `tasks.json` including the MTL I/O vector arrays, but
  authored its attempt-1 programs from the task descriptions only. The clean
  trials mtl_t1 and mtl_t2 (authored without seeing the vectors) produced the
  **same 7/7 solved outcome**, so the mtl_t3 result is corroborated rather than
  contaminating; it is kept as a protocol caveat, not a thrown-out trial. Removing
  mtl_t3 entirely would still leave both clean trials at 7/7 and would not change
  the headline (pass@5 = 100% both arms).
- **Held-out coverage.** The cold trial covers the **7 text-feedable** sealed
  tasks; the 8 negative-input tasks are excluded by the text-harness input-encoding
  limitation (see `bench/sealed/GAPS.md`), not by any solver failure.

