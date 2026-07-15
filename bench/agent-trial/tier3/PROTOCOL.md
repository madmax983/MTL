# MTL Tier-3 capability cold-agent trial (`T_tier3-trial`)

This harness measures whether a cold LLM can **write correct Tier-3 MTL
programs** — programs that reach a host through *capabilities* (I/O, tools,
budgets, confinement) — from a cold language reference, and at what **total
token cost** relative to writing the same programs in Python. Like the tier-2
`T_agent-trial`, it is a data-and-Python harness: nothing here is in the Rust
build graph or CI gates. The only Rust dependency is the `tier3run` validator
binary, which embeds the real `mtl-host` runtime.

## What it measures

For each of 8 tasks, two arms are run head-to-head:

- **MTL arm** — the model is given `docs/mtl-quickref.md` (now v0.4, including the
  Host-capabilities section) and a task prompt, and must return an MTL program
  that produces the correct host output under the task's grant set and budget.
  This is a **capability** battery, so it keeps the **full** quickref: the
  cold-preamble ablation (PR #88) that adopted the 487-token
  `docs/mtl-quickref-min.md` as the pure-computation default was validated on
  pure tasks only and explicitly does **not** license dropping the
  Host-capabilities prose here. Division of labor: min quickref = pure
  computation; full quickref = pure computation + host capabilities. This arm's
  prompt composition is unchanged.
- **Python arm** — the model is given the same task plus the list of host stub
  functions, and must return a `solve()` function that calls those stubs.

The token-cost proxy is **visible output tokens** (the program the model emits),
counted with `tokcount` under `o200k_base` and `cl100k_base`. Hidden reasoning
tokens are excluded from both arms, so the comparison is fair. The
model-under-test is the session's Claude model.

## The 8 tasks

Every task is a small capability program. The three axes exercised are host I/O
(read/emit), **budgets** (a capped `emit`), and **confinement** (a restricted
grant set). `granted` is the grant set; `emit_budget` is the per-name call
budget on `emit` (null = unlimited).

| Task | Exercises | Granted | emit_budget | Expected output |
|---|---|---|---|---|
| `transform_hits` | map+filter over lines | all | — | `APPLE\nAPRICOT\n` |
| `emit_budget` | stop before budget | all | 2 | `one\ntwo\n` |
| `guarded_read` | guard `nextline` with `endp` | all | — | `x\ny\nz\n` |
| `concat_lines` | combine two handles | all | — | `foobar\n` |
| `select_line` | index a handle list | all | — | `c\n` |
| `confined_echo` | confinement (2 caps) | `readline`,`emit` | — | `hello\n` |
| `confined_grep` | confinement (3 caps) | `readlines`,`linehit`,`emit` | — | `cat\ncar\n` |
| `budget_grep` | filter within budget | all | 2 | `ant\nart\n` |

The authoritative machine spec (prompts, grant sets, budgets, expected outputs)
is `tasks.json`. The per-task host inputs (input lines and the `linehit`
predicate char) live in the runtimes: in `crates/mtl-host/src/caps/mod.rs` for
the MTL arm and in `validate_py.py`'s `FIXTURES` for the Python arm, kept
identical so both arms see the same fixture.

## Arms and cold-agent protocol

- **Model under test.** The solver is the session's Claude model. It is *cold*:
  no prior exposure to MTL beyond the in-prompt reference.
- **Closed-book (MTL arm).** The MTL arm's ONLY permitted reference is
  `docs/mtl-quickref.md`. Its Host-capabilities section is the sole documentation
  of capability calls, the grant model, budgets, and host faults. No other repo
  file may be read.
- **Python arm.** Gets the task prompt and the list of stub functions
  (`read_line`, `read_lines`, `emit`, `emit_int`, `line_hit`, `transform`,
  `next_line`, `end_p`, `concat`, `select`) — but **no language reference**,
  because Python is a warm language for the model and charging it a
  language-acquisition cost would be unfair.
- **Trials.** Each (task, arm) is run for `N = 2` independent trials. (Cost-scoped:
  2 trials/cell were actually run, down from the tier-2 trial's 3, to bound spend.)
- **Repair loop.** Each attempt is validated deterministically; on failure the
  solver is handed the **verbatim** oracle diagnostic and asked to continue, up
  to **5** attempts per trial. Nothing about the diagnostic is summarized or
  judged by another model.
- **Deterministic validation.** The MTL arm is validated by the real
  `mtl-host` runtime via `tier3run`; the Python arm by the symmetric
  `validate_py.py`. Both emit ONE verdict line from the same vocabulary and both
  always exit 0.

### Validators

```
# MTL arm — program on stdin, one verdict line:
printf '%s' 'readline emit' | ./target/debug/tier3run confined_echo      # -> PASS

# Python arm — program (defining solve()) on stdin:
python3 validate_py.py confined_echo < reference_solutions/python/confined_echo.py   # -> PASS
```

`tier3run <task>` reads an MTL program from stdin and prints `PASS`, or
`FAIL: <reason>` where reason is one of `wrong_output got=<r> want=<r>`,
`NotGranted <name>`, `BudgetExhausted`, `OutputCapExceeded`, `InputClosed`,
`ToolError`, `FAULT:<Kind>`, `Cancelled`, or `PARSE ERROR: <detail>`. It exits 0
(unknown task → stderr + exit 1). `validate_py.py <task>` mirrors that
vocabulary for the Python arm: `PASS`, `FAIL: wrong_output got=<repr> want=<repr>`,
`FAIL: NotGranted <name>`, `FAIL: BudgetExhausted`, `FAIL: InputClosed`, or
`FAIL: python_exception <ExcType>: <msg>`.

### Symmetric confinement

Both runtimes enforce the grant set the same way. In MTL a `Call` to an
ungranted name faults `NotGranted` and does nothing. In `validate_py.py` every
stub NAME is bound into the sandbox, but any capability outside the task's
`granted` list is bound to a stub that RAISES `NotGranted(<name>)` on call.
An ungranted-call ATTEMPT is therefore a loud, categorizable failure in both
arms, so the trial can count ungranted-call attempts on either side (this is the
central confinement metric on `confined_echo`/`confined_grep`).

## Metrics computed

- **P(correct≤5)** — probability a cell reaches PASS within the 5-attempt repair
  budget, per arm.
- **Attempts-to-first-correct** — mean attempts until the first PASS.
- **Output-tokens-to-first-correct** — `tokcount` (o200k primary) of the emitted
  programs summed up to and including the first PASS.
- **Correct-per-million-tokens (cspm)** — corrects per 1e6 output tokens, and the
  **MTL/Python cspm ratio**.
- **Error-type taxonomy** — counts by verdict category, including the NEW Tier-3
  modes: `NotGranted` (wrong-cap-name or grant-violation), `BudgetExhausted`
  (budget-blindness), plus `InputClosed`, `OutputCapExceeded`, `ToolError`,
  `wrong_output`, and parse errors.
- **Confinement observation** — count of ungranted-call attempts, especially on
  `confined_echo` and `confined_grep`, in both arms.

### Quickref cold-instruction token cost

The MTL arm pays the quickref token cost once per attempt (it is in-prompt). The
v0.4 quickref that adds the Host-capabilities section grew the reference from
the v0.3 baseline:

| Encoding | v0.3 (OLD) | v0.4 (NEW) |
|---|---|---|
| `o200k_base` | 2244 | 3926 |
| `cl100k_base` | 2234 | 3915 |

This one-time reference cost is charged only to the MTL arm; the Python arm
receives no language reference.

## Integrity notes

- **The oracle reveals only PASS/FAIL + diagnostic, never the reference.** No
  reference solution or expected internal state is shown to the cold agent; the
  agent sees only its own program's verdict line.
- **Grant violations fail loudly and count as failures.** A `NotGranted` (either
  arm) is a failed attempt, not a silent no-op that the agent could ignore.
- **Tool access could not be hard-disabled.** The cold agents run in an
  environment with repo tool access that could not be fully severed, so they were
  **instructed** to read only `docs/mtl-quickref.md` (MTL arm) / nothing (Python
  arm) and were **audited post-hoc** against their transcripts to confirm no
  other repo file (especially the reference solutions or `caps/mod.rs`) was read.

## Files

| File | Purpose |
|---|---|
| `tasks.json` | Machine spec: the 8 tasks (prompt, grant set, emit budget, expected output). |
| `validate_py.py` | Python-arm oracle, symmetric to `tier3run`. One verdict line, exits 0. |
| `PROTOCOL.md` | This methodology note. |
| `reference_solutions/mtl/<task>.mtl` | Known-good MTL solution per task (oracle self-test only). |
| `reference_solutions/python/<task>.py` | Known-good `solve()` per task (oracle self-test only). |
| `results/attempts/` | Per-cell JSON files dropped by the cold-agent runs. |

`docs/mtl-quickref.md` (outside this dir) is the cold-agent language reference
handed to the MTL arm. The reference solutions are for oracle self-test ONLY and
are never shown to the cold agents.

## Scope

Cold-only (no warm or fine-tuned MTL arm), **8 tasks**, 2 trials per arm (cost-scoped), a
single model under test, up to 5 repair attempts per cell. The output-token
metric is a visible-program proxy and excludes hidden reasoning. These bounds are
intentional: the trial answers the narrow question "can a cold LLM write correct
Tier-3 capability MTL from the v0.4 reference — respecting grant sets and budgets
— and at what token cost vs Python," not the broader question of warmed-up or
fine-tuned fluency.
