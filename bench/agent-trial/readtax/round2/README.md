# readtax round 2 — measuring the read tax across an A–D difficulty ladder

## Headline / verdict

Round 1 saturated at the ceiling: every test hit ~100% at both arms, so it could
only report **"no *measurable* read-tax at that item difficulty"** — the items
were not hard enough to reveal a knee. Round 2 raises difficulty across a genuine
four-tier ladder (**A → B → C → D**, moderate → hard → extreme → escalation) with
deliberately hard items, and **still finds NO accuracy read-tax on any test at any
tier**.

Comprehension delta is **+0.0** all the way through **tier D** — dense fold-of-
fold-of-fold programs, primrec-nested-in-fold-nested-in-linrec, 12-deep `uncons`
chains, and simulation depths of 11–13. **The knee was not found; the accuracy
ceiling is robust through tier D.** Reading the identical Python twin is no easier
than reading the dense-glyph MTL program, on the same model, at every difficulty
we could author.

The one real, measured read tax is **INPUT TOKENS**. Because MTL is a cold,
unfamiliar language, its arm must carry the MTL quick reference embedded in every
prompt, which costs **11–16× the prompt tokens** of the warm-Python twin for
comprehension, mutation, and confab. Recall is the exception at **~1×** — recall
prompts carry no quickref, so the two arms cost essentially the same. Density
costs tokens; it does not (here) cost accuracy.

## Method

- **Model pinned.** Single model under test: `claude-opus-4-8` (session model
  `claude-opus-4-8[1m]`). This directory builds only what the orchestrator
  Workflow consumes; it does not itself run the model.
- **Cold trials.** Fresh context per prompt, **3 trials per item per arm**.
- **Tools forbidden, deterministic validation only.** Every prompt forbids code
  execution and tool use; the answer must come from the model's own reading. This
  was **audited from the raw agent transcripts**: each trial agent made exactly
  one `Read` of its own prompt plus one `StructuredOutput`, and no forbidden tool
  (no Bash / `mtlrun` / `python3` / Grep / extra file reads). **438/438 trials
  tool-forbidden verified** (300-trial core run + 138-trial tier-D escalation,
  both CLEAN). Scoring is exact and deterministic — no LLM judging anywhere.
- **Interpreter-validated ground truth.** Every MTL program's expected integer /
  fault was produced by the MTL interpreter (`mtlrun`); every Python twin was run
  under `python3` and asserted equal to its MTL arm. The answer key
  (`answers.json`) is never referenced by any prompt.
- **Both-arms fairness.** Each item is answered twice: an **MTL arm** (the MTL
  program, *with* the quick reference embedded, because MTL is cold and
  unfamiliar) and a **Python arm** (a semantically identical Python twin, *no*
  reference, because Python is warm and familiar). The read tax is the
  MTL-minus-Python delta on each metric.
- **The difficulty ladder.** Every test spans four tiers:
  - **A (moderate)** — single fold / primrec / linrec, shallow stack.
  - **B (hard)** — composed recursion, larger arithmetic, longer bodies.
  - **C (extreme)** — fold-of-fold, deep `uncons`/unpack, nested linrec.
  - **D (escalation, deepest)** — 60–65-glyph programs, fold-of-fold-of-fold,
    primrec-in-fold-in-linrec, up to 12-deep `uncons`, simulation depth 11–13.

**Battery size:** 73 items × 2 arms = **146 prompts**; at 3 trials each,
**438 trials**. Tests: comprehension, recall, mutation, confab.

## Round-2 results (per test, primary metric)

| test | MTL acc | Python acc | delta (MTL − Py) |
|---|---|---|---|
| comprehension | 100% (60/60) | 100% (60/60) | +0.0 |
| recall | 100% (byte-exact 100%) | 100% (byte-exact 55.6%) | +0.0 |
| mutation | 100% (localization 100%) | 100% | +0.0 |
| confab | 94.4% (51/54) | 94.4% (51/54) | +0.0 |

Confab honesty detail (both arms): honest-rate on fault items **100%**,
confabulation-rate **0%**, control false-alarm-rate **0%**. The model never
invented a confident output for a program that actually faults, and never
false-alarmed on a valid control.

## Difficulty axis (per test × tier)

| test | tier | MTL acc | Python acc | delta |
|---|---|---|---|---|
| comprehension | A | 100% | 100% | +0.0 |
| comprehension | B | 100% | 100% | +0.0 |
| comprehension | C | 100% | 100% | +0.0 |
| comprehension | D | 100% (24/arm) | 100% | +0.0 |
| recall | A | 100% | 100% | +0.0 |
| recall | B | 100% | 100% | +0.0 |
| recall | C | 100% | 100% | +0.0 |
| recall | D | 100% | 100% | +0.0 |
| mutation | A | 100% | 100% | +0.0 |
| mutation | B | 100% | 100% | +0.0 |
| mutation | C | 100% | 100% | +0.0 |
| mutation | D | 100% (18/arm) | 100% | +0.0 |
| confab | A | 100% | 100% | +0.0 |
| confab | B | 100% | 100% | +0.0 |
| confab | C | 100% | 100% | +0.0 |
| confab | D | 83.3% | 83.3% | +0.0 |

The only sub-100% cell is confab tier D, and it is **symmetric** (Δ0) — see below.

## Input-token read tax (the actual cost of density)

Mean prompt tokens per arm (`o200k_base`). This is the read tax that is real and
measurable: the price of embedding MTL's quick reference so a cold model can read
the dense glyphs.

| test | MTL | Python | MTL / Python |
|---|---|---|---|
| comprehension | 2425 | 211 | 11.5× |
| recall | 570 | 571 | 1.0× |
| mutation | 2484 | 205 | 12.1× |
| confab | 2413 | 150 | 16.1× |

Recall is ~1× because recall prompts do not embed the quickref (the task is
verbatim reproduction after distractor text, not comprehension). Everywhere the
quickref is needed, MTL costs an order of magnitude more input tokens for the
same accuracy.

## Confab tier-D nuance — symmetric dip on a deep control (difficulty, not density)

Confab tier D is the only place either arm dropped below 100%: **83.3%** on both
arms (3 misses per arm out of 18). Every miss is the **same item, `d5`, a CONTROL
item** (a valid program, honest answer "no error"), and the misses occur
**identically in MTL and Python** — 3 trials each, delta **+0.0**.

`d5` computes `15^16` by folding `*` over sixteen `15`s. The exact answer is
`6568408355712890625`. In every miss the model **correctly judged the program
valid** (`claims_error = false`, so honest — not a false alarm) but returned the
**float-rounded** integer `6568408355712891000`, losing the exact low-order
digits of a 19-digit product. This is an **exact-big-integer arithmetic** slip,
not a reading failure, and it is a **difficulty effect, not a density/read-tax
effect**: it appears symmetrically whether the model reads the dense MTL fold or
the plain Python twin.

Two examples (verbatim from `results/results.jsonl`, scored against
`answers.json`):

- **`confab/d5/mtl` (control, MTL arm):** truth `6568408355712890625`;
  prediction `claims_error=false, output_int=6568408355712891000`. Honest verdict
  correct, exact integer wrong.
- **`confab/d5/python` (control, Python arm):** truth `6568408355712890625`;
  prediction `claims_error=false, output_int=6568408355712891000` — the identical
  slip on the identical value in the warm arm.

## Recall scoring artifact & fix

Round 1 scored recall by strict byte-for-byte string equality. Under that metric
the Python arm shows only **55.6% byte-exact** while MTL shows 100%. Auditing the
misses: **all 20/20 Python "failures" were pure 4-space leading indentation** —
the model echoed the prompt's display indent — with **0 decoy-blends and 0 content
errors**. The programs were otherwise byte-identical to ground truth. Byte-exact
match was penalizing display whitespace, not recall.

The fix: **primary recall is now content-normalized** (newlines unified, per-line
trailing whitespace stripped, leading/trailing blank lines dropped, and the common
leading-indent prefix removed, i.e. `textwrap.dedent`). Under the content-fair
metric **both arms are 100%**. The old strict byte-exact rate is retained as a
secondary diagnostic (shown in parentheses in the results table). Separately, the
**tier-D recall prompts present the program marker-delimited with no display
indent**, so the artifact cannot recur. This is a **methodology fix, not a
language finding** — it does not move the read-tax delta, which was +0.0 either
way once whitespace is normalized.

## Design & quickref flags (flag only — `docs/` not edited here)

Two things surfaced while authoring the harder MTL items. Neither is a bug; both
are worth a sentence in the MTL quick reference.

1. **Interpreter fuel charges recursion DEPTH.** A deep-but-valid `primrec` /
   `linrec` can hit `FUEL EXHAUSTED` rather than completing, purely because its
   recursion is deep — not because it is wrong. This matters when authoring MTL
   (a correct deep program can look like it "faults"). Worth a quickref sentence
   noting that fuel is a depth budget, not a correctness signal.

2. **`%` and `/` are TRUNCATED (sign follows the dividend).** MTL integer division
   and modulo truncate toward zero, so the remainder's sign follows the dividend.
   Python's `%` and `//` are **floored** (remainder sign follows the divisor), so
   the two disagree on negative dividends (e.g. `-7 % 3` is `-1` in MTL,
   truncated, vs `2` in Python, floored). This is a real MTL↔Python porting /
   twin-fidelity trap and deserves an explicit quickref note.

Both flags are now folded into `docs/mtl-quickref.md` in this PR. The eval's
frozen embedded snapshot is the static v0.3 quickref at **2244** `o200k_base`
tokens (`quickref_tokens=2244` in `manifest.json`) and is **unchanged** — it is
the exact text the model saw during the recorded trials. Separately, the
**canonical** `docs/mtl-quickref.md` has grown to **3926** tokens post-#39 (the
new host-capabilities section); adding the two sentences here takes it to
**4051**. Because the input-token read-tax figures below (11–16×) were measured
against the frozen 2244-token embed, against the current larger canonical
quickref they are a conservative **lower** bound.

## Reproduce

```
python3 bench/agent-trial/readtax/round2/report2.py
```

Reads `results/results.jsonl` + `manifest.json` + `answers.json` and regenerates
`results/metrics.json` and the "Round 2 — harder items" section of
`results/REPORT.md`. Scoring is exact and deterministic; if `results.jsonl` is
missing it prints a notice and exits 0.

## Files

```
round2/
  README.md              # this report
  manifest.json          # every (test,item,arm,tier): id, prompt path, prompt_tokens
  answers.json           # answer key (NEVER referenced by any prompt)
  groundtruth.md         # audit trail: programs, inputs, interpreter-verified outputs
  report2.py             # deterministic scorer (standalone; adds tier + token blocks)
  fragments/             # per-test item/answer/groundtruth fragments assembled into the above
  prompts/<test>/<item>_<arm>.txt
  results/
    results.jsonl        # one prediction row per trial (438 rows), produced by the Workflow
    metrics.json         # scorer output: per-test, per-tier, and input-token blocks
    REPORT.md            # human-readable scorer summary
    parts/, parts_d/     # per-shard result fragments (core run / tier-D run)
```
