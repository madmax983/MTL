# Tier-3 agentic benchmark suite — index (v0.4, design stage)

The read-input / emit-output / tool-call shape that MTL 0.4.0 is measured on.
Eight tasks, each specified in `agentic/<name>.md` (I/O contract + Python sketch +
MTL sketch). **All MTL sketches are hand-traced, design-stage, representative**
(capability names are `Call` words; token counts real via `bench/tokcount`,
exact golf not guaranteed — the `retry_on_fault` and `grep_filter`/`map_lines`
loop shapes carry a ±band). Reproduce with `python3 design-v0.4/agentic/measure.py`
from `bench/`.

## Index + token table (o200k / cl100k)

| task | intent | key capabilities | Py o200k/cl100k | MTL o200k/cl100k | needs in-core `Str`? |
|---|---|---|---:|---:|:--:|
| echo_line | read a line, emit it | `read-line`,`emit` | 8 / 8 | 3 / 3 | **No** |
| grep_filter | emit lines a host predicate accepts | `read-lines`,`line-hit`,`emit` | 20 / 20 | 12 / 12 | **No** |
| agent_loop | call `step` until `done?` (fixpoint) | `read-state`,`done?`,`step` | 24 / 24 | 10 / 10 | **No** |
| json_field | extract a JSON field, emit it | `read-json`,`get-name`,`emit` | 13 / 13 | 5 / 5 | **No** |
| two_tool_pipeline | fetch → parse → emit | `read-input`,`fetch`,`parse`,`emit` | 10 / 10 | 5 / 5 | **No** |
| retry_on_fault | retry a flaky tool ≤N times | `try-op`,`ok?` | 30 / 30 | 12 / 12 | **No** |
| map_lines_tool | transform each line via a tool | `read-lines`,`transform`,`emit` | 15 / 15 | 9 / 9 | **No** |
| word_count | count words, emit the count | `read-text`,`tokenize`,`emit-int` | 11 / 11 | 11 / 11 | **No\*** |
| **TOTAL** | | | **131 / 131** | **67 / 67** | **0 / 8** |

**Aggregate (sum Py / sum MTL): 1.96× o200k / 1.96× cl100k.**

\* `word_count` needs in-core `Str` **only** if you refuse a `tokenize`
capability (Framing 2); with the capability it is a pure `fold` length. See
`word_count.md`.

## What the numbers say

- **0 of 8 tasks require `Str` in the core.** Every task pipes opaque string
  *handles* through capabilities; the only in-core compute is `Int`/`Quote`
  control flow (loop, filter, count, branch). The single "stringy" task
  (`word_count`) reduces to list-length once tokenization is a capability.
- **The compression is control-flow-driven, not capability-driven.** The big
  wins are the loops — `agent_loop` 24→10 (2.4×), `retry_on_fault` 30→12 (2.5×) —
  where MTL's `linrec`/`fold` replace Python's `while`/`for` + `def` scaffolding.
  The capability-call-dominated tasks compress far less: `word_count` is a **tie**
  (11/11) because long `Call` names (`read-text`,`tokenize`,`emit-int`) cost
  several BPE tokens each, cancelling the glyph-density edge.
- **1.96× aggregate is real but modest** — close to the v0.3 tier-2 v0.2 baseline
  (1.91×), and well below the fold-driven 3.87× headline. Agentic glue is where
  MTL's compression is *thinnest*, because capability names are ordinary words the
  tokenizer already handles well. The MTL case for this suite rests on *safety*
  (capability confinement, §8.1) far more than on token compression.
- **Implication for the Str decision:** the suite provides **no** task-coverage
  pressure to add `Str` to the core. Handles + capabilities cover all eight.
