# ICL-preamble ablation — cold-solver cell protocol (issue #73)

This documents the protocol for the workflow that RUNS the ablation. The setup
step (variants + token counts + aggregation scaffolding) is frozen; this file is
the contract for producing `results/results.jsonl`.

## What is being ablated

Five preamble variants (the in-context language reference handed to a cold MTL
solver), holding the language and the 10-task battery fixed:

| Variant | File | o200k tokens | Description |
|---|---|---|---|
| `v1_full` | `variants/v1_full.md` | 4051 | Verbatim `docs/mtl-quickref.md` (baseline). |
| `v2_grammar_only` | `variants/v2_grammar_only.md` | 2055 | Full pure-language spec minus worked examples (§6) and host capabilities (§7). |
| `v3_worked_examples_heavy` | `variants/v3_worked_examples_heavy.md` | 1675 | Primitive table + 8 worked examples, minimal prose. |
| `v4_compressed_minimal` | `variants/v4_compressed_minimal.md` | 487 | Token-golfed one-line-per-primitive table + one-line fault list. |
| `v5_task_adaptive` | `variants/v5_task_adaptive/<task>.md` | 376.4 mean | Per-task: only the primitives that task's reference solution uses. |

The 10 tasks (all PURE): `affine, rev3, is_even, factorial, gcd, sum_list,
reverse_list, palindrome_number, contains, climbing_stairs`.

## Cell definition

A **cell** is one `(variant, task, trial)` triple. Run **3 trials** per
`(variant, task)` (matching the base agent-trial). Total cells:
5 variants x 10 tasks x 3 trials = 150.

## Per-cell procedure

1. **Fresh subagent.** Spawn a new, context-isolated solver for every cell. It
   must carry no memory of MTL or of other cells — the ablation measures
   learning MTL *from the variant file alone*.
2. **The solver reads ONLY its variant file** as the language reference:
   - variants `v1`–`v4`: the single `variants/<variant>.md` file;
   - `v5_task_adaptive`: the single `variants/v5_task_adaptive/<task>.md` file.
   It also receives the task's MTL prompt from `payload.json` (`mtl_prompt`) and
   the task's I/O vectors from `tasks.json`. It must NOT read
   `docs/mtl-quickref.md`, the reference solutions, or any other variant.
3. **Solve.** The solver returns ONLY an MTL program (a word string), no prose.
4. **Validate deterministically** with the frozen validator (never an LLM judge):
   ```
   printf '%s' '<program>' | python3 bench/agent-trial/validate_one.py \
       --task <task> --arm mtl --program-stdin \
       --trial <trial> --attempt <attempt> \
       --record-dir bench/agent-trial/preamble/results/attempts
   ```
   The verdict is the emitted JSON's `ok` field. `mtlrun` must be built:
   `cargo build --bin mtlrun -p mtl-bench-validate`.
5. **Repair loop.** On `ok=false`, feed the validator's real diagnostic back to
   the solver **verbatim** — the `error_type` + `error_detail` (the actual
   `mtlrun` `FAULT:`/`stack:`/`next:` text, parser error, or got-vs-expected
   diff) and the `failing_vector`. Allow up to **N = 5** attempts total
   (attempt 1 = first try, attempts 2–5 = repairs). Re-validate after each.
   Stop early on the first `ok=true`.
6. **Record** one JSONL line per cell to `results/results.jsonl` (schema below).

## results/results.jsonl schema (one JSON object per line, one per cell)

| Field | Type | Meaning |
|---|---|---|
| `variant` | str | one of the 5 variant ids |
| `task` | str | one of the 10 task ids |
| `trial` | int | trial index (e.g. 1..3) |
| `solved` | bool | true iff some attempt reached `ok=true` |
| `attempts` | int | number of attempts made (1..5) |
| `first_correct_attempt` | int \| null | 1-based index of the first `ok=true` attempt, or null |
| `programs` | list[str] | the program emitted at each attempt, in order |
| `preamble_tokens` | int | o200k tokens of the variant file used for this cell |

For `v5_task_adaptive`, `preamble_tokens` is the per-task count from
`variant_tokens.json` (`v5_task_adaptive.per_task[task]`), NOT the mean.

## Metrics (computed by `aggregate.py`, then `pareto.py`)

- **solve_rate** — fraction of a variant's cells with `solved=true`.
- **tokens-to-first-correct** (per solved cell) —
  `preamble_tokens + sum of o200k(program)` over attempts up to and including
  the first correct one. `aggregate.py` reports the per-variant **median**.
- **Pareto frontier** — maximize `solve_rate`, minimize `preamble_tokens`;
  `pareto.py` plots all variants and marks the non-dominated set.

Do NOT run `aggregate.py`/`pareto.py` until `results.jsonl` exists.
