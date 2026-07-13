# tokcount — MTL token-measurement harness

Static program-source token counts under the OpenAI `tiktoken` encodings
(`o200k_base`, `cl100k_base`) used as a public proxy for LLM tokenization. See
`report.py` for the headline token-reduction report and `tokcount.py` for the
core counting module. `tiktoken` is pinned in `requirements.txt`.

## Tokenizer-drift guard (`drift.py`)

### What it is

`drift.py` maintains a checked-in **profile** (`token_profile.json`) of exact
token counts for:

- every individual MTL glyph/primitive (a fixed, checked-in glyph list), each
  counted as an isolated string; and
- every corpus program the harness enumerates (via `report.load_tasks()` /
  `tasks.json`).

It also pins the `tiktoken` version and the encoding names. `--check`
recomputes these counts and fails if anything changed.

### Why it matters

Tokenizer/vocabulary drift across model generations can silently move token-count
economics — the whole token-economy thesis rests on counts that a vocab change can
shift underneath us. So we pin a profile and fail loud on any change, without
asserting a specific multiplier as established fact.

For calibration, the related-work note records what pxpipe actually reports: its
**~4.6× is a *density* figure** (tokens saved on dense content), and its
cross-generation *semantic-recall* effect was **not statistically significant**
(two-proportion z ≈ 0.76, p ≈ 0.45). We deliberately do not claim a "~4× knee shift
between generations" as an established multiplier — the point is only that
generation-to-generation drift is real enough that a one-time count must never be
trusted silently. Checking the profile in makes any drift a conscious, reviewed
event (new `tiktoken`, new/edited corpus program, or a genuine vocab change) instead
of a silent one. See [`docs/notes/related-work.md`](../../docs/notes/related-work.md)
for the accurate citations.

### How to run

```sh
python3 bench/tokcount/drift.py --check     # default: diff vs checked-in profile; exits nonzero on drift
python3 bench/tokcount/drift.py --update    # (re)write token_profile.json (alias: --write)
```

`--check` prints a human-readable diff of any changed count (glyph/program,
encoding, old -> new) plus any `tiktoken` version mismatch, and exits nonzero.
When counts are unchanged it prints `no drift` and exits 0.

**Network graceful degradation:** `tiktoken` lazily downloads its vocab files on
first use. If an encoder cannot load (the common CI failure), `--check` does
**not** report false drift — it prints `SKIPPED: tokenizer unavailable (...)`
and exits 0. Only real count changes with successfully-loaded encoders count as
drift. (`--update` refuses to write a profile when encoders are unavailable, so
it never bakes in null counts.)

### CI

The `tokreport` workflow runs `drift.py --check` as a **non-blocking** step
(the job sets `continue-on-error: true`), right after the token report. It is
informational and never gates a merge; it surfaces drift for review. After an
intentional change, regenerate with `--update` and commit `token_profile.json`.
