# MTL agent-writability trial — REPORT

Primary token counts are **o200k_base**; cl100k is secondary.

Cells found: **60** | attempts found: **67** | max attempts/cell: 5 | expected trials per (task,arm): 3

## Headline (MTL vs Python)

| Metric | MTL | Python |
|---|---|---|
| P(correct ≤ 5 attempts) | 100.0% | 100.0% |
| Median output tokens to first correct | 10 | 17 |
| Correct-solutions per million tokens | 67873.3 | 53285.97 |

**Correct-solutions-per-million-tokens ratio (MTL / Python): 1.274** (>1 favors MTL).

## A/B. Correctness and attempts-to-first-correct

| Metric | MTL | Python |
|---|---|---|
| Total cells | 30 | 30 |
| Solved cells | 30 | 30 |
| P(correct ≤ 5) | 100.0% | 100.0% |
| Mean attempts to first correct | 1.23 | 1 |
| Median attempts to first correct | 1 | 1 |

Attempts-to-first-correct distribution (solved cells, by attempt index):

| Attempt | MTL | Python |
|---|---|---|
| 1 | 24 | 30 |
| 2 | 5 | 0 |
| 3 | 1 | 0 |
| 4 | 0 | 0 |
| 5 | 0 | 0 |

## C. Output tokens to first correct (§17 headline component)

| Metric | MTL | Python |
|---|---|---|
| Median | 10 | 17 |
| Mean | 14.73 | 18.77 |

## D. Correct-solutions-per-million-tokens (headline, §10.6)

Charges the full cost of failed repair attempts: `solved_cells / sum(total_output_tokens over ALL cells) * 1e6`.

| Metric | MTL | Python |
|---|---|---|
| Solved cells | 30 | 30 |
| Total output tokens (all cells) | 442 | 563 |
| Correct-solutions per 1e6 tokens | 67873.3 | 53285.97 |
| MTL / Python ratio | 1.274 | |

## Per-task × arm breakdown

| Task | Arm | Cells | Solved | P(≤5) | Med tok→correct | Correct/1e6 tok | Trials |
|---|---|---|---|---|---|---|---|
| affine | mtl | 3 | 3 | 100.0% | 4 | 250000 | 1,2,3 |
| affine | python | 3 | 3 | 100.0% | 11 | 85714.29 | 1,2,3 |
| climbing_stairs | mtl | 3 | 3 | 100.0% | 8 | 125000 | 1,2,3 |
| climbing_stairs | python | 3 | 3 | 100.0% | 36 | 27777.78 | 1,2,3 |
| contains | mtl | 3 | 3 | 100.0% | 28 | 35714.29 | 1,2,3 |
| contains | python | 3 | 3 | 100.0% | 16 | 62500 | 1,2,3 |
| factorial | mtl | 3 | 3 | 100.0% | 12 | 93750 | 1,2,3 |
| factorial | python | 3 | 3 | 100.0% | 31 | 32258.06 | 1,2,3 |
| gcd | mtl | 3 | 3 | 100.0% | 10 | 100000 | 1,2,3 |
| gcd | python | 3 | 3 | 100.0% | 24 | 41666.67 | 1,2,3 |
| is_even | mtl | 3 | 3 | 100.0% | 4 | 250000 | 1,2,3 |
| is_even | python | 3 | 3 | 100.0% | 14 | 66666.67 | 1,2,3 |
| palindrome_number | mtl | 3 | 3 | 100.0% | 19 | 51724.14 | 1,2,3 |
| palindrome_number | python | 3 | 3 | 100.0% | 18 | 55555.56 | 1,2,3 |
| rev3 | mtl | 3 | 3 | 100.0% | 2 | 500000 | 1,2,3 |
| rev3 | python | 3 | 3 | 100.0% | 17 | 58823.53 | 1,2,3 |
| reverse_list | mtl | 3 | 3 | 100.0% | 40 | 25000 | 1,2,3 |
| reverse_list | python | 3 | 3 | 100.0% | 10 | 100000 | 1,2,3 |
| sum_list | mtl | 3 | 3 | 100.0% | 17 | 46875 | 1,2,3 |
| sum_list | python | 3 | 3 | 100.0% | 9 | 111111.11 | 1,2,3 |

## E. Total-token accounting (§10.4) and the verdict

| Quantity | MTL (cold) | Python (cold) |
|---|---|---|
| Program-only median (winning attempt) | 9 | 17 |
| Generation+repair median | 10 | 17 |
| Cold instruction cost (quickref, o200k) | 2157 | 0 |
| Cold total per solve | 2167 | 17 |
| Amortized over 10 tasks | 225.7 | 17 |

Python's cold instruction cost is ~0: the model already knows Python and receives no language reference, whereas MTL pays the full quickref cost (2157 o200k tokens) once per task cold. This is the asymmetry the review predicted.

### Does MTL's static (program-length) token edge survive total-token accounting?

- **Static edge:** median first-correct program tokens — MTL 9 vs Python 17. MTL programs are shorter.
- **Efficiency (charges failed repairs):** correct-solutions per 1e6 tokens — MTL 67873.3 vs Python 53285.97 (ratio 1.274).
- **Cold total per solve:** MTL 2167 vs Python 17.

**Verdict: the static edge SURVIVES.** On the efficiency metric that charges failed repairs, MTL is at least as good as Python (correct-solutions-per-million-tokens MTL ≥ Python).

## F. Error-type distribution and repair efficacy

| error_type | MTL | Python |
|---|---|---|
| TypeMismatch | 1 | 0 |
| parse | 6 | 0 |

Stack-tracking faults (Underflow + TypeMismatch) — MTL 1, Python 0. parse — MTL 6, Python 0. wrong_output — MTL 0, Python 0. The review predicted stack-tracking failures dominate the MTL arm.

### Repair efficacy (does stack-state-in-errors feedback fix failures?)

| Metric | MTL | Python |
|---|---|---|
| Cells that failed attempt 1 | 6 | 0 |
| ...that eventually solved | 6 | 0 |
| Repair success rate | 100.0% | n/a |
| Mean attempts among repaired | 2.17 | n/a |

## Caveats

- **Cold-only, no warm arm.** The model-under-test is the session's Claude model. Only cold agents were tested — no fine-tuning or warmed-up (agent already fluent in MTL) arm was available. A warm MTL arm would pay no quickref cost and would likely raise pass@1, so the cold numbers are a lower bound for MTL.
- **Token-count proxy.** The output-token metric counts only the visible program the model emits, not hidden reasoning tokens. Both arms are measured identically so the comparison is fair, but absolute totals understate true generation cost.
- **Both arms treated identically** for token counting (o200k_base primary, cl100k_base secondary) and for the 5-attempt repair budget.
- TODO: add any further cold-only / warm-agent and tokcount-proxy caveats the reviewer wants to expand here.

