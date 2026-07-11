# MTL tokenizer-measurement harness (`bench/`)

## What this is

A **stage-1 static-program token-measurement harness** for the MTL north-star
metric: **>=3x token reduction versus idiomatic Python at equal-or-better agent
success**. It measures how many tokens each solution's *source* consumes under
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
- **Versioned task sets.** This corpus is **`T_v0`**. Freezing the task-set
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

Five seed micro-tier tasks, all expressible in MTL v0 primitives (no string
tasks — v0 has no string primitives):

| Task | Category | Split | Recursion? |
| --- | --- | --- | :---: |
| affine | arithmetic-pipeline | dev | no |
| rev3 | stack-shuffle | dev | no |
| is_even | predicate | dev | no |
| factorial | recursion | dev | yes |
| gcd | recursion | dev | yes |

Each task directory holds `task.md`, `mtl/solution.mtl` (+ `mtl/NOTES.md`),
`python-idiomatic/solution.py`, and `python-minified/solution.py`. The manifest
is `tokcount/tasks.json` (version tag `T_v0`).

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

- **MTL solutions are unvalidated** until the interpreter (Track B) lands. No MTL
  program here has been executed; correctness is a best-effort structural claim.
  **Token counts are exact regardless of correctness.**
- The **recursion** solutions (factorial, gcd) are **structural sketches** — the
  `:!` self-application ordering is not yet interpreter-checked; treat their
  token counts as indicative.
- The **Python** solutions are honest idiomatic/minified pairs — ordinary code a
  competent Python author would write, not adversarially inflated.
- If a tokenizer's vocab cannot be loaded (e.g. blocked network download), the
  report prints `TOKENIZER UNAVAILABLE: <reason>` in that column and still
  produces the table structure. It never fabricates numbers.
