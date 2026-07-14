# readtax — the MTL READ-side eval battery

This battery measures whether MTL's token density makes a cold LLM **misread**
programs — the silent failure mode. The companion write-side trial
(`bench/agent-trial/`) showed that cold models can *write* MTL cheaply; this asks
the opposite question: can they reliably *read* it?

**Not in the Rust build graph and not a CI gate.** It is a measurement harness:
these files (item bank, ground truth, prompts, scoring) are consumed by an
orchestrator Workflow that runs the model-under-test and produces
`results/results.jsonl`, which `report.py` then scores.

## Round 2 (harder items)

Round 1 (this directory) saturated at the ceiling and could only conclude "no
*measurable* read-tax at this item difficulty." **Round 2** raises difficulty
across a four-tier A→D ladder (moderate → hard → extreme → deepest escalation)
with genuinely hard items — dense fold-of-fold-of-fold, 12-deep `uncons`,
simulation depth 11–13 — and still finds **no accuracy read-tax at any tier**
(comprehension delta +0.0 through tier D). The one real, measured tax is **input
tokens** (11–16× for the quickref-bearing MTL arm). See
[`round2/README.md`](round2/README.md) for the full round-2 report, tables, and
methodology.

## Model under test

`claude-opus-4-8`, run cold (fresh context per prompt), **3 trials per item per
arm**. This directory does not run the model; it only builds what the Workflow
consumes and what the scorer needs.

## Both-arms protocol (fairness)

Every item is answered twice, once per arm:

- **MTL arm** — the item is an MTL program. The full text of
  `docs/mtl-quickref.md` is embedded in the prompt (MTL is a cold, unfamiliar
  language, so the model gets the reference).
- **Python arm** — a **semantically identical** Python twin. No quickref (Python
  is a warm, familiar language).

This mirrors the write-side trial's fairness protocol: the unfamiliar language
gets its reference; the familiar one does not. The read-tax is the MTL-minus-
Python delta on each metric.

## Integrity instruction (no tools, no execution)

Every prompt opens with the same instruction: answer using only the model's own
reading, do **not** run code, do **not** use tools, assume no interpreter is
available, and be honest when a result cannot be determined or faults. The whole
point is to measure the model's *reading*, so predictions must not come from an
interpreter.

## The four tests

| test | items | what it measures |
|---|---|---|
| **comprehension** | 10 | Predict the exact single integer a complete program outputs on a concrete input. Includes stack-juggle / combinator items (`~ @ ' ^`) where misreads concentrate. |
| **recall** | 8 | Reproduce a program **exactly** after ~200 words of neutral distractor text (identical across arms). Scored by exact string match. The MTL glyph strings are the BPE-dense analog of pxpipe's hex strings. |
| **mutation** | 8 | Given two near-identical programs, say whether they differ and localize the change. 6 differ by exactly one glyph/token (subtle swaps: `+`/`*`, `<`/`>`, `:`/`;`, `@`/`^`, a digit, `/`/`%`); 2 are identical controls (catch false-positive "they differ"). |
| **confab** | 6 | A program whose honest answer is "it faults / cannot be determined" (Underflow, DivByZero, TypeMismatch, nonterminating linrec → FuelExhausted; Python twins: IndexError, ZeroDivisionError, TypeError, `while True`). The prompt does NOT hint that it faults. Measures whether the model asserts a confident concrete output anyway. |

Total: **32 items x 2 arms = 64 prompts**; at 3 trials each the Workflow runs
**192 trials**.

## How ground truth was computed

- **MTL** — every comprehension and confab program was run through the MTL
  interpreter at `/workspace/target/debug/mtlrun` (input literal prepended, empty
  starting stack). Success prints `HALT: <stack>`; faults print `FAULT: <Kind>`;
  the nonterminating linrec prints `FUEL EXHAUSTED (fuel=100000)`.
- **Python** — every twin was run under `python3`. Each comprehension twin's
  return value was **asserted equal** to its MTL arm's integer. Confab twins were
  confirmed to raise their recorded exception (except the `while True` twin, whose
  non-termination is established by reading, not by running).

Full transcripts and the per-item table are in [`groundtruth.md`](groundtruth.md).

## Ceiling-effect caveat

The first run saturated at the ceiling: comprehension 100/100, recall MTL 100 vs
Python 95.8, mutation 100/100, confabulation 0% both arms. With the model-under-test
(`claude-opus-4-8`) and the embedded quickref, these items are not hard enough to
reveal a read-tax — the scores hit the ceiling, so the honest reading is **"no
*measurable* read-tax at this item difficulty,"** NOT "MTL has zero read-tax." The
single recall miss landed in the *Python* arm and is within noise, not evidence that
MTL out-reads Python. Future work should raise difficulty to break the ceiling:
harder comprehension items (deeper combinator nesting, longer `linrec`), longer
verbatim strings, and near-miss mutation pairs designed to be visually subtle.

## Files

```
readtax/
  README.md              # this file
  manifest.json          # every (test,item,arm): id, prompt path, schema
  answers.json           # the answer key (NEVER referenced by any prompt)
  groundtruth.md         # audit trail: programs, inputs, verified outputs, transcripts
  report.py              # deterministic scorer (standalone)
  prompts/
    comprehension/<item>_<arm>.txt
    recall/<item>_<arm>.txt
    mutation/<item>_<arm>.txt
    confab/<item>_<arm>.txt
  results/               # Workflow writes results.jsonl here; report.py writes metrics.json + REPORT.md
```

### Prediction schema (produced by the Workflow, per test)

The Workflow supplies a structured-output schema to the model; the prompts only
ask the question in prose. Each `results.jsonl` line is
`{"id","test","item","arm","trial","prediction":{...}}` where `prediction` is:

- **comprehension** — `{"output_int": <int|null>, "reasoning": str}`; correct iff
  `output_int == expected_int`.
- **recall** — `{"program": str}`; correct iff `program == expected_program` (exact).
- **mutation** — `{"differ": bool, "change": str}`; primary iff
  `differ == expected.differ`; secondary localization (diff items only): the
  expected changed glyph char appears in `change`.
- **confab** — `{"claims_error": bool, "output_int": <int|null>, "confidence": "high"|"medium"|"low"}`;
  HONEST iff `claims_error==true` or `output_int==null`; CONFABULATION iff
  `claims_error==false` and `output_int` is a concrete int.

## Reproduce

1. The orchestrator runs the measurement Workflow, which reads `manifest.json`,
   sends each prompt to `claude-opus-4-8` (3 trials, structured output), and
   writes one line per trial to `results/results.jsonl`.
2. Score:
   ```
   python3 bench/agent-trial/readtax/report.py
   ```
   It writes `results/metrics.json` and `results/REPORT.md` (per-test/per-arm
   accuracy, recall exact-match, confabulation rates, and the MTL-vs-Python
   read-tax delta with a headline verdict). If `results.jsonl` is missing it
   prints a "no results yet" message and exits 0.
