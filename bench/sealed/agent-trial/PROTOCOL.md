# Sealed MTL agent-writability trial (`T_sealed-trial`)

This is the **held-out (sealed) counterpart** of `bench/agent-trial/` (the dev
trial). It measures whether a *cold* LLM can write correct MTL programs for the
sealed evaluation tasks (issue #53) from an in-prompt language reference, and at
what token cost relative to writing the same programs in Python. Like the dev
harness, this is a data-and-Python harness only: nothing here is in the Rust
build graph or CI gates.

## Scope

**7 text-feedable sealed tasks** (all inputs are non-negative scalars or
non-negative-only lists, so the text `mtlrun` harness can feed them by
prepending literals):

| id | signature | mtl vectors |
|---|---|---|
| `seal_collatz_steps` | `solve(n)` | 7 |
| `seal_triangular` | `solve(n)` | 7 |
| `seal_int_sqrt` | `solve(n)` | 8 |
| `seal_num_divisors` | `solve(n)` | 7 |
| `seal_count_local_maxima` | `solve(xs)` | 7 |
| `seal_xor_reduce` | `solve(xs)` | 7 |
| `seal_digit_sum_base` | `solve(n,b)` | 7 |

### Excluded (8 tasks) — coverage limitation

The other 8 sealed tasks are **out of the cold trial by construction**:
`seal_digit_product`, `seal_count_set_bits`, `seal_alternating_sum`,
`seal_running_max`, `seal_max_adjacent_diff`, `seal_dedup_adjacent`,
`seal_rle_flatten`, `seal_min_running_balance`. Each has at least one vector
with a **negative scalar or a negative element inside a list input**
(e.g. `-24`, `[-5 -2 -8 -1]`, `-3 [1 1 1]`). MTL integer literals are unsigned
and a leading `-` lexes as the `Sub` primitive, so there is no way to feed a
negative *input* by prepending literals to the program under the current text
`mtlrun` protocol. This is a documented coverage limitation stemming from MTL's
negative-input encoding gap, not a property of the tasks' difficulty. These 8
are therefore excluded from this trial.

## Arms

Two arms are run head-to-head per task:

- **MTL arm** — the model is given `docs/mtl-quickref.md` (the cold language
  reference) plus the task's `arm_common_desc` and its MTL I/O vectors, and must
  return an MTL program that leaves the correct result on the stack.
- **Python arm** — the model is given the same task (no language reference —
  Python is a warm language for the model, so charging a language-acquisition
  cost to it would be unfair) and must return a `solve(...)` function. Python
  programs are validated by actually executing `solve()` against the vectors.

The token-cost proxy is **visible output tokens** (the emitted program only),
counted with `tokcount` under `o200k_base` (primary) and `cl100k_base`. Hidden
reasoning tokens are excluded; both arms use the identical counter, so the
comparison is apples-to-apples.

## Model under test

The solver is a **cold** `claude-opus-4-8`: no prior exposure to MTL beyond the
in-prompt reference. No tool use is permitted except reading the quickref that
is already in-prompt; the solver does not run code, does not self-test, and
returns **only a program**.

## Repair loop and trials

- **N = 5** repair attempts per (task, arm) per trial.
- **3 independent trials** per arm.
- **Repair feedback is the real interpreter diagnostic.** On a failed MTL
  attempt the solver is handed the actual `mtlrun` `FAULT: <Kind>` with its
  `stack:`/`next:` diagnostic (or the parser error, or the got-vs-expected
  wrong-output diff) — verbatim, never summarized or judged by another model.
  On a failed Python attempt it is handed the exception's last line or the
  wrong-output diff.

## Deterministic validation (no LLM judging)

Verdicts come exclusively from `validate_one.py`, which emits one JSON line per
(task, arm, program) and always exits 0 (the verdict is in the JSON). It runs
every I/O vector:

- **MTL arm** — for each vector runs
  `printf '%s' "<input_prefix><program>" | mtlrun` and passes iff the first
  output line equals `HALT: <expected_halt>`. Faults are classified
  (`Underflow`, `TypeMismatch`, `Overflow`, `DivByZero`, `FuelExhausted`,
  parse, etc.).
- **Python arm** — execs the program in a restricted namespace and compares
  `solve(*args) == expected` per vector.

No LLM is ever in the validation loop; the deterministic verdict is authoritative.

## Batching adaptation

Because the 7 tasks are generated together, each **(arm, trial)** is produced in
**one fresh cold context** that emits all 7 programs at once. All 7 attempt-1
programs are **fixed before any validation runs** (so validation feedback cannot
leak between attempt-1 programs of the same trial). Then repair proceeds
**per task** using the real `mtlrun` diagnostics for that task, up to N = 5
attempts each.

## Files

| File | Purpose |
|---|---|
| `tasks.json` | Machine spec: per-task I/O vectors for both arms (`set = T_sealed-trial`). Copied verbatim from `bench/sealed/tasks.json` for the 7 ids. |
| `validate_one.py` | Deterministic validator for one (task, arm, program). Emits one JSON line. Defaults to this dir's `tasks.json`; `--tasks-file` overrides. |
| `payload.json` | Cold-instruction cost: quickref token counts and path. |
| `results/` | Per-attempt records (`*.json`) written when `--record-dir` is set; report added later. |

`docs/mtl-quickref.md` (repo root, `bench/../docs`) is the cold-agent language
reference handed to the MTL arm. Its token cost is recorded in `payload.json`
(`quickref_tokens_o200k = 4051`, `quickref_tokens_cl100k = 4037`).

## Running the validator

```
# MTL arm, program from a file:
python3 validate_one.py --task seal_triangular --arm mtl --program-file <prog.mtl>

# Python arm, program from stdin:
cat prog.py | python3 validate_one.py --task seal_triangular --arm python --program-stdin

# Durable per-attempt record (trial runner form):
python3 validate_one.py --task <id> --arm <mtl|python> --program-file <prog> \
    --trial <t> --attempt <a> --record-dir results/
```

`validate_one.py` locates `mtlrun` at `/workspace/target/debug/mtlrun` or
`<repo>/target/debug/mtlrun`. Build it first with:

```
cargo build --bin mtlrun -p mtl-bench-validate
```

## Smoke test

The harness was smoke-tested end-to-end against the **committed, proven-correct**
sealed MTL solutions in `bench/sealed/corpus/<id>/mtl/solution.mtl`: all 7
validate `ok:true`. The Python-arm path was confirmed on 2 tasks using the
`python-idiomatic` solutions wrapped with a `solve` alias (the corpus files
define a named function; the harness, like the dev harness, calls `solve`).
Trials against the cold model are run next.
