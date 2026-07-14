## Round 2 — harder items

**Verdict:** modest/negligible read-tax on comprehension (delta +0.0 pts); see recall/mutation/confab and the per-tier table for where density bites.

### Per-test accuracy (primary metric)

| test | MTL acc | Python acc | delta (MTL-Py) |
|---|---|---|---|
| comprehension | 100.0% | 100.0% | +0.0 pts |
| recall | 100.0% (byte 100.0%) | 100.0% (byte 55.6%) | +0.0 pts |
| mutation | 100.0% | 100.0% | +0.0 pts |
| confab | 94.4% | 94.4% | +0.0 pts |

_Recall primary metric is content-normalized (newlines unified, trailing whitespace and leading/trailing blank lines stripped, common leading indentation removed); byte-exact rate is shown in parentheses as a secondary diagnostic. The Python byte-exact gap is an indentation artifact — the model echoed the prompt's display indent — not a content error, so primary recall normalizes it away._

### Per-test × per-tier accuracy (difficulty axis)

| test | tier | MTL acc | Python acc | delta (MTL-Py) |
|---|---|---|---|---|
| comprehension | A | 100.0% | 100.0% | +0.0 pts |
| comprehension | B | 100.0% | 100.0% | +0.0 pts |
| comprehension | C | 100.0% | 100.0% | +0.0 pts |
| comprehension | D | 100.0% | 100.0% | +0.0 pts |
| recall | A | 100.0% | 100.0% | +0.0 pts |
| recall | B | 100.0% | 100.0% | +0.0 pts |
| recall | C | 100.0% | 100.0% | +0.0 pts |
| recall | D | 100.0% | 100.0% | +0.0 pts |
| mutation | A | 100.0% | 100.0% | +0.0 pts |
| mutation | B | 100.0% | 100.0% | +0.0 pts |
| mutation | C | 100.0% | 100.0% | +0.0 pts |
| mutation | D | 100.0% | 100.0% | +0.0 pts |
| confab | A | 100.0% | 100.0% | +0.0 pts |
| confab | B | 100.0% | 100.0% | +0.0 pts |
| confab | C | 100.0% | 100.0% | +0.0 pts |
| confab | D | 83.3% | 83.3% | +0.0 pts |

### Confabulation guard (per arm)

| arm | honest-rate (fault items) | confab-rate (fault items) | control false-alarm-rate |
|---|---|---|---|
| mtl | 100.0% | 0.0% | 0.0% |
| python | 100.0% | 0.0% | 0.0% |

### Input token cost (mean prompt tokens, o200k_base)

| test | MTL | Python | MTL/Python ratio |
|---|---|---|---|
| comprehension | 2425.1 | 210.7 | 11.512 |
| recall | 570.4 | 570.9 | 0.999 |
| mutation | 2484.0 | 204.5 | 12.147 |
| confab | 2413.1 | 149.8 | 16.105 |
