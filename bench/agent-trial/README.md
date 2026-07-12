# MTL agent-writability trial (`T_agent-trial`)

This harness measures whether an LLM can **write correct MTL programs** from a
cold language reference, and at what **total token cost** relative to writing
the same programs in Python. It is a data-and-Python harness only: nothing here
is in the Rust build graph or CI gates.

## What it measures

For each of 10 tasks, two arms are run head-to-head:

- **MTL arm** — the model is given `docs/mtl-quickref.md` and a task prompt, and
  must return an MTL program that leaves the correct result on the stack.
- **Python arm** — the model is given the same task and must return a
  `solve(...)` function.

The token-cost proxy is **visible output tokens** (the program the model emits),
counted with `tokcount` under `o200k_base` and `cl100k_base`. Hidden reasoning
tokens are excluded — both arms are measured identically, so the comparison is
fair. The model-under-test is the session's Claude model.

## Files

| File | Purpose |
|---|---|
| `tasks.json` | Machine spec: per-task I/O vectors for both arms (`set = T_agent-trial`). |
| `validate_one.py` | Deterministic validator for one (task, arm, program). Emits one JSON line. |
| `payload.json` | Prompts for solver agents: full quickref text + per-task MTL/Python prompts (no solutions). |
| `reference_python/<task>.py` | Correct reference `solve()` per task (harness self-test). |
| `reference_mtl/<task>.mtl` | Known-good MTL solution per task (harness self-test). |
| `results/` | Holds `results.jsonl` (per-attempt records) and `REPORT.md` (added later). |

`docs/mtl-quickref.md` (outside this dir) is the cold-agent language reference
handed to the MTL arm. Its token cost is recorded in `payload.json`
(`quickref_tokens_o200k` / `quickref_tokens_cl100k`).

## Protocol

1. **Cold generation.** For each task and arm, prompt the model-under-test with
   the arm prompt (MTL arm also receives the full quickref). No tool use; the
   model returns **only the program** — an MTL word string or a Python
   `solve(...)` definition.
2. **Deterministic validation.** Run `validate_one.py --task <id> --arm <mtl|python>`
   feeding the returned program. It runs every I/O vector and emits one JSON
   line with `ok`, `error_type`, `error_detail`, `failing_vector`, and program
   token counts.
3. **Repair loop.** On failure, feed the real fault back to the model — for the
   MTL arm this is the actual `mtlrun` `FAULT:`/stack/next diagnostic (or the
   parser error, or the wrong-output got-vs-expected), for the Python arm the
   exception's last line or wrong-output diff. Allow up to **N = 5** repair
   attempts, re-validating after each.
4. **Record.** Append each attempt to `results/results.jsonl`. Metrics
   (pass@1, pass@N, tokens-to-correct per arm, MTL-vs-Python ratio) are computed
   by `report.py` into `results/REPORT.md`. `report.py` is a stub for now and
   will be added later.

## Running the validator

```
# MTL arm, program from a file:
python3 validate_one.py --task gcd --arm mtl --program-file reference_mtl/gcd.mtl

# Python arm, program from stdin:
cat reference_python/gcd.py | python3 validate_one.py --task gcd --arm python --program-stdin
```

The validator locates `mtlrun` at `/workspace/target/debug/mtlrun` or
`./target/debug/mtlrun`. Build it first with:

```
cargo build --bin mtlrun -p mtl-bench-validate
```

`validate_one.py` always exits 0; the verdict is in the emitted JSON.

## MTL input/output conventions

- Inputs are provided by **prepending** literal(s) to the candidate program
  (`input_prefix + program`), executed against an empty initial stack — exactly
  as `mtlrun` and the corpus tests do. A list argument is one quotation, e.g.
  `[1 2 3]`; two ints are `a b` with `b` on top.
- `mtlrun` prints `HALT: <stack bottom..top>` on success (ints as decimal,
  quotations as `[a b c]`, empty stack as `<empty>`), or `FAULT: <Kind>` with
  `stack:`/`next:` diagnostics. A program passes a vector iff the first output
  line equals `HALT: <expected_halt>`.

## Harness integrity & methodology note

This section records exactly how the reported numbers were produced, and the one
integrity fix applied during the run, so the results can be audited from the
on-disk records alone.

### Cold solver protocol

- **Model under test.** The solver is the session's Claude model,
  `claude-opus-4-8`. It is *cold*: it has no prior exposure to MTL beyond the
  in-prompt reference.
- **Prompt.** Each attempt is a single prompt containing the quickref (MTL arm
  only), the task description, and the task's I/O vectors. No tool use is
  permitted except reading the quickref that is already in-prompt; the solver
  does **not** run code, does not self-test, and returns **only a program** — an
  MTL word string, or a Python `solve(...)` definition.
- **Trials.** Each (task, arm) is run for `N = 5` repair attempts per trial, with
  3 independent trials per arm.
- **Repair feedback.** On a failed attempt the solver is handed the *real*
  `mtlrun` fault — the actual `FAULT: <Kind>` with its `stack:` state and `next:`
  continuation (or the parser error, or the got-vs-expected wrong-output diff) —
  and asked to continue. Nothing about the fault is summarized or judged by
  another model; the interpreter's own diagnostic text is fed back verbatim.
- **Python arm** is identical in every respect *except* that it receives **no
  language reference** — Python is a warm language for the model, so charging a
  language-acquisition cost to it would be unfair. Python programs are validated
  by actually executing `solve()` against the vectors.
- **Token proxy.** The cost metric is `tokcount` (`o200k_base` primary) of the
  emitted program only. Hidden reasoning tokens are excluded, and both arms are
  measured with the identical counter, so the comparison is apples-to-apples.

### Integrity fix (LLM-validator false positives)

The first pass used an LLM *in the loop* as a validator: it read the
`validate_one.py` JSON and schema-*judged* the `ok` field rather than trusting
the deterministic verdict. On three MTL cells
(`climbing_stairs`/mtl/t2, `palindrome_number`/mtl/t1, `reverse_list`/mtl/t2)
that judge emitted **false positives** — it declared success and stopped the
repair loop early on programs the interpreter had actually faulted or cut short.
The discrepancy was caught by **deterministic re-validation**: re-running
`validate_one.py` over every stored `program` and comparing the fresh
`ok`/`error_type` to the recorded verdict. The three affected cells were re-run
under a strictly deterministic verdict — `validate_one.py`'s own JSON parsed in
code, never an LLM — and **all three now solve**
(`climbing_stairs` mtl t2 solves on attempt 1; `palindrome_number` mtl t1 solves
on attempt 1; `reverse_list` mtl t2 needs three attempts: parse, parse, then ok).
Every number in `REPORT.md`/`metrics.json` derives from the on-disk per-attempt
records, and the full record set re-validates today with **0 mismatches**
(67/67 records).

### Scope

Cold-only (no warm or fine-tuned MTL arm), **10 tasks** (5 frozen `T_v0` +
5 solved tier-2 tasks), 3 trials per arm, a single model under test. The output
token metric is a visible-program proxy and excludes hidden reasoning. These
bounds are intentional; the trial answers the narrow question "can a cold LLM
write correct MTL from a ~2k-token reference, and at what token cost vs Python,"
not the broader question of warmed-up or fine-tuned fluency.
