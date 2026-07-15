# MTL tokenizer-measurement harness (`bench/`)

> **Restated (v0.8, 2026-07-15): the ≥3x compression gate is retired.** The
> 3.72x–3.92x figures below are **in-sample dev corpora** only. The first
> out-of-sample, held-out (sealed) measurement is **1.67x (o200k) / 1.72x
> (cl100k)** — below the ≥3x bar — so the gate did **not** generalize and has
> been retired. Compression is now framed as a **niche** property (2–4x on
> loop/fold-heavy tasks, ≤1x on scans and where Python has a terse builtin), not
> a headline. What *does* generalize: agent writability (**100% pass@5** held-out)
> and per-solution economics (**CSPM 2.124x** held-out, widening from dev). See
> [`BASELINE-SEALED.md`](BASELINE-SEALED.md) and
> [`../docs/design/v0.8-generalization.md`](../docs/design/v0.8-generalization.md).
> The in-sample numbers below are preserved as recorded.

## What this is

A **stage-1 static-program token-measurement harness** for the MTL north-star
metric (token reduction versus idiomatic Python at equal-or-better agent
success). The original **≥3x Abrash gate is retired out-of-sample** — it held
only on in-sample dev corpora (see the banner above). It measures how many
tokens each solution's *source* consumes under
public LLM tokenizers, so we can track whether MTL's dense glyph encoding
actually buys the reduction the project is betting on.

This harness is **stage 1 only**. It measures static program tokens. It does
**not** yet measure agent success rate or attempts-to-correct. See "The full
intended metric" below.

## What's measured today

Static tokens of the program **source** under two OpenAI `tiktoken` encodings,
used as a public proxy for LLM tokenization:

- **`o200k_base`** — GPT-4o family.
- **`cl100k_base`** — GPT-4 / GPT-3.5 family.

For each task we hold three variants — `mtl`, `python-idiomatic`,
`python-minified` — and count tokens for each under each encoding. The headline
is the **MTL reduction ratio vs idiomatic Python** (higher = better for MTL).

**Headline (stage 1, static program tokens) — in-sample dev corpora only; see
the retirement banner at the top of this file:** on interpreter-validated
static-token counts, the frozen-`T_v0` **v0.2** solution set reaches **3.72x** vs
idiomatic Python (**>=3x gate MET in-sample**), where the frozen **v0.1** solutions were
**2.11x** (NOT MET). On the tier-2 probe set the aggregate rises from **v0.2
1.91x (o200k) / 1.89x (cl100k)** to **v0.3 3.87x (o200k) / 3.92x (cl100k)** vs
idiomatic Python — the **>=3x gate is MET on the tier-2 probe set too, in-sample
only** (held-out this drops to **1.67x**; the gate is retired — see
[`BASELINE-SEALED.md`](BASELINE-SEALED.md) and
[`../docs/design/v0.8-generalization.md`](../docs/design/v0.8-generalization.md)),
and `single_number` moved from an inexpressible wall to solved via `$` xor. This is
still stage 1 (static program tokens); the full `E[tokens x attempts]` metric
remains future work. No agent-success-rate claims.

Counting policy: a single trailing newline is stripped from every file before
counting, applied **uniformly** to all variants so cross-variant comparisons are
fair. The `.mtl` files contain **only** the token-measured program on a single
line — no comments — so the count is honest. Prose lives in a sidecar
`mtl/NOTES.md` next to each solution.

## Claude tokenizer stub

There is **no pinned public implementation of the Claude tokenizer**. The
intended future approach is to count via the Anthropic API usage fields
(`input_tokens` / `output_tokens`) returned on a request. Until that is wired up,
`count_claude` is a deliberate stub:

```python
def count_claude(text: str) -> int:
    """Count tokens under the Claude tokenizer.

    NOT IMPLEMENTED. The Claude tokenizer has no pinned public implementation.
    The intended approach is to submit `text` to the Anthropic API and read the
    `usage.input_tokens` field from the response (or use a token-counting
    endpoint if available), then pin the model + API version used. Until that is
    wired up, this raises so no fake numbers leak into the baseline.
    """
    raise NotImplementedError(
        "Claude tokenizer has no pinned public implementation; count via the "
        "Anthropic API usage fields (input_tokens/output_tokens) once wired up."
    )
```

(The stub lives here in the README as the design of record; the measured
harness intentionally ships only the two public tiktoken encodings today.)

## Anti-gaming design (from external review)

- **Corpus splits: `train` / `dev` / `sealed-eval`.** Glyph choices must **not**
  be tuned on `sealed-eval`. `sealed-eval` is withheld (not committed) and only
  run at gate-decision time, so we cannot overfit the glyph set to the eval
  tasks. In **v0, only `dev` is populated**; `train` and `sealed-eval` are
  **reserved-empty** to prevent glyph overfitting.
- **Versioned task sets.** This corpus is **`T_v0.2`** (frozen `T_v0` tasks
  retained); the **tier-2** probe set now additionally carries a **`T_v0.3`**
  solution layer (frozen `T_v0` gate tasks and their v0.1/v0.2 sets unchanged).
  Freezing the task-set
  version prevents silently swapping tasks to flatter the numbers. Any change to
  the task set bumps the version.
- **The full intended metric.** The real target is
  `E[total inference tokens to correct]` under a **warm/cold agent protocol**:
  - *cold* = no MTL context in the prompt;
  - *warm* = spec/primer in context.
  The harness measures **static program tokens today — that is stage 1.**
  Stages 2+ add agent **success rate** and **attempts-to-correct**, yielding the
  real `E[tokens x attempts]` headline from spec section 10.

## Corpus

The task set is now **`T_v0.2`**. The frozen `T_v0` tasks are retained
unchanged; **v0.2** adds an **interpreter-validated** solution set built on the
new recursion primitives `&` (primrec), `.` (times), `|` (linrec), `>` (uncons),
carried in a `mtl-v0.2/` solution directory alongside the retained frozen v0.1
solutions. It also adds three **dev** tasks (fib, sum_to, power).

The **tier-2** probe set now additionally carries a **v0.3** solution layer in
`mtl-v0.3/` directories (11 tasks), built on the new sequence primitives `(`
(fold) and `$` (xor) and interpreter-validated by
`bench/validate/tests/tier2_v03.rs`. This is what lifts the tier-2 aggregate from
v0.2 1.91x/1.89x to v0.3 3.87x (o200k) / 3.92x (cl100k); `single_number`, a wall
through v0.2, is solved via `$` xor (`[>0=][0][][$]|`, 9 tokens).

Five seed micro-tier tasks, all expressible in MTL v0 primitives (no string
tasks — v0 has no string primitives):

| Task | Category | Split | Recursion? |
| --- | --- | --- | :---: |
| affine | arithmetic-pipeline | dev | no |
| rev3 | stack-shuffle | dev | no |
| is_even | predicate | dev | no |
| factorial | recursion | dev | yes |
| gcd | recursion | dev | yes |

Plus v0.2 dev tasks: fib, sum_to, power.

Each task directory holds `task.md`, `mtl/solution.mtl` (+ `mtl/NOTES.md`),
`python-idiomatic/solution.py`, and `python-minified/solution.py`. The manifest
is `tokcount/tasks.json` (version tag `T_v0.2`).

## How to run

From the repo root:

```
pip3 install -r bench/tokcount/requirements.txt
python3 bench/tokcount/report.py
```

`report.py` prints a markdown report to stdout and writes `bench/BASELINE.md`.
It resolves paths relative to the script, so it runs from any cwd.

You can also count a single file or string:

```
python3 -m tokcount.tokcount "3*7+"          # literal string (run from bench/)
python3 -m tokcount.tokcount path/to/file    # a file that exists
echo "hello" | python3 -m tokcount.tokcount  # stdin
```

## Honesty caveats

- **MTL solutions are now parse-and-execute validated.** Both the v0.2 and v0.1
  solution sets are run through the interpreter by
  `bench/validate/tests/corpus.rs` against per-task I/O vectors, so correctness is
  checked rather than asserted; the tier-2 v0.3 fold/xor set is likewise validated
  by `bench/validate/tests/tier2_v03.rs`. **Token counts are exact regardless.**
- The **recursion** solutions (factorial, gcd) are validated via the v0.2
  recursion primitives (`&` primrec, `.` times, `|` linrec, `>` uncons).
- The **Python** solutions are honest idiomatic/minified pairs — ordinary code a
  competent Python author would write, not adversarially inflated.
- If a tokenizer's vocab cannot be loaded (e.g. blocked network download), the
  report prints `TOKENIZER UNAVAILABLE: <reason>` in that column and still
  produces the table structure. It never fabricates numbers.
