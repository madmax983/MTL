# MTL Tier-3 capability cold-agent trial — REPORT

Primary token counts are **o200k_base**; cl100k is secondary. Model under test: **claude-opus-4-8** (run cold). Deterministic validation via the real `mtl-host` runtime (`tier3run`) and the symmetric `validate_py.py`.

Cells found: **32** | attempts found: **38** | max attempts/cell: 5 | expected trials per (task,arm): 2

## Headline — can a cold LLM write capability-using MTL?

| Metric | MTL | Python |
|---|---|---|
| P(correct ≤ 5 attempts) | 100.0% | 100.0% |
| Median output tokens to first correct (o200k) | 12 | 20 |
| Correct-solutions per million tokens (cspm, o200k) | 90395.48 | 46109.51 |

**cspm ratio (MTL / Python): 1.96** (>1 favors MTL).

> **Verdict.** Both arms solved every cell within the 5-attempt repair budget. A cold LLM CAN write correct capability-using MTL from the v0.4 quickref alone, at a per-program token cost below the Python arm, and — the security headline — **no cold agent in either arm ever attempted a capability outside its grant**.

## Per-task solved table (both arms)

| Task | MTL solved/total (win-attempts) | Python solved/total (win-attempts) |
|---|---|---|
| `transform_hits` | 2/2 (1) | 2/2 (1,2) |
| `emit_budget` | 2/2 (2) | 2/2 (1,2) |
| `guarded_read` | 2/2 (1) | 2/2 (1) |
| `concat_lines` | 2/2 (2) | 2/2 (1) |
| `select_line` | 2/2 (1) | 2/2 (1) |
| `confined_echo` | 2/2 (1) | 2/2 (1) |
| `confined_grep` | 2/2 (1) | 2/2 (1) |
| `budget_grep` | 2/2 (1) | 2/2 (1) |

## A / B. Correctness and attempts-to-first-correct

| Metric | MTL | Python |
|---|---|---|
| Total cells | 16 | 16 |
| Solved cells | 16 | 16 |
| P(correct ≤ 5) | 100.0% | 100.0% |
| Mean attempts to first correct | 1.25 | 1.12 |
| Median attempts to first correct | 1 | 1 |

Attempts-to-first-correct distribution (solved cells, by attempt index):

| Attempt | MTL | Python |
|---|---|---|
| 1 | 12 | 14 |
| 2 | 4 | 2 |
| 3 | 0 | 0 |
| 4 | 0 | 0 |
| 5 | 0 | 0 |

## C. Output tokens to first correct

| Metric | MTL (o200k) | Python (o200k) | MTL (cl100k) | Python (cl100k) |
|---|---|---|---|---|
| Median | 12 | 20 | 12 | 20 |
| Mean | 11.06 | 21.69 | 11.06 | 21.56 |

## D. Correct-solutions-per-million-tokens (cspm)

Charges the full cost of failed repair attempts: `solved_cells / sum(o200k program tokens over ALL attempts of ALL cells) * 1e6`.

| Metric | MTL | Python |
|---|---|---|
| Solved cells | 16 | 16 |
| Total output tokens (all attempts, o200k) | 177 | 347 |
| Correct-solutions per 1e6 tokens | 90395.48 | 46109.51 |
| **MTL / Python ratio** | **1.96** | |

## E. Total-token accounting and the quickref cold tax

| Quantity | MTL (cold) | Python (cold) |
|---|---|---|
| Program-only median (winning attempt, o200k) | 10.5 | 20 |
| Generation+repair median (all attempts/cell, o200k) | 12 | 20 |
| Cold instruction cost — quickref v0.4 (o200k) | 3926 | 0 |
| Cold total per solve (v0.4) | 3936 | 20 |
| Amortized over 8 tasks (v0.4) | 501.25 | 20 |

**The quickref grew.** Adding the Host-capabilities section took the cold-instruction cost from the v0.3 baseline **2244** o200k tokens to the v0.4 **3926** o200k tokens (Δ +1682). PR #15's tier-2 trial paid a 2157-token quickref tax; this Tier-3 trial pays 3926 once per task cold. Amortized over the 8 tasks that is 501.25 o200k tokens/solve (v0.4) vs 291.0 (had the quickref stayed at v0.3). Python pays no language reference (warm language), so its cold instruction cost is 0.

## F. Error-type taxonomy (non-pass attempts)

| error bucket | MTL | Python |
|---|---|---|
| not_granted | 0 | 0 |
| budget_exhausted | 0 | 1 |
| input_closed | 0 | 0 |
| wrong_output | 4 | 1 |
| parse_error | 0 | 0 |
| core_fault | 0 | 0 |
| python_exception | 0 | 0 |
| tool_error | 0 | 0 |

The observed non-pass attempts were all single-repair-fixable. The two capability-specific traps actually seen:

- **MTL `readline`-doesn't-advance `wrong_output`** — on `emit_budget` and `concat_lines`, the first attempt used `readline`/`nextline` in a way that re-read the same handle (e.g. `got="one\none\n"` and `got="foofoo\n"`), then the repair switched to `readlines`+`select` and PASSed.
- **Python `solve()`-double-call artifact** — on `transform_hits` the first attempt emitted the output twice (`APPLE\nAPRICOT\nAPPLE\nAPRICOT\n`) by including a trailing `solve()` call in the returned body; and on `emit_budget` a first attempt tripped `BudgetExhausted` before the repair stopped at the cap. Both were fixed on the second attempt.

## G. Confinement observation (the security headline)

| Arm | Cells with an ungranted-call attempt | By task |
|---|---|---|
| mtl | 0 | — |
| python | 0 | — |

**Total ungranted-call attempts across all confined cells and both arms: 0.** No cold agent, MTL or Python, attempted a capability outside its grant; when told the grant, both arms stayed inside it. This holds on the confinement tasks (`confined_echo`, `confined_grep`) and everywhere else. And it holds *regardless of agent behavior*: both runtimes enforce confinement for free — a call to an ungranted capability is a loud `NotGranted` failure (a failed attempt, never a silent no-op), so an ungranted call could never have slipped through as a PASS even if an agent had tried one.

## Integrity notes

- **N=2 trials/cell** (PR #15's tier-2 trial used 3; reduced here for cost). Single model (`claude-opus-4-8`) run cold.
- **Deterministic re-validation.** Every solved cell's winning program was re-run through the real oracle at finalization: **32/32 solved cells re-validated PASS, 0 mismatches**; a sample of recorded FAIL attempts (`emit_budget`/`concat_lines` wrong_output, `emit_budget` python `BudgetExhausted`) reproduced their exact recorded verdict.
- **The oracle reveals only PASS/FAIL + diagnostic**, never the reference solution or expected internal state.
- **Tool access could not be hard-disabled.** Agents were *instructed* to read only `docs/mtl-quickref.md` (MTL arm) / nothing (Python arm); results are consistent with quickref-derivable idioms.

## Caveats

- **The tasks are small.** These are minimal capability programs; solve rates reflect quickref quality as much as raw model capability.
- **The quickref contains worked examples** for the grep/drain idioms, so several tasks were near-trivial given the reference. The 100% solve rate should be read as "the v0.4 quickref is sufficient for these idioms," not as an unbounded capability claim. Don't oversell.
- **Cold-only, no warm/fine-tuned MTL arm.** A warm arm would pay no quickref tax and is not measured here.
- **Token-count proxy.** Only the visible emitted program is counted, not hidden reasoning; both arms are measured identically so the comparison is fair, but absolute totals understate true generation cost.

