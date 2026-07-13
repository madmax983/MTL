# v0.4 glyph BPE measurements (in context, spec §11)

**DESIGN STAGE.** Per the §11 assignment protocol, every candidate glyph is
measured **inside a small representative program** (never as a bare char — BPE
merges are context-dependent) under **o200k_base + cl100k_base (tiktoken 0.8.0)**.
Reproduce: `python3 design-v0.4/glyphs_sweep.py` from `bench/`.

**Free ASCII set.** After v0.3 spent `(` (fold) and `$` (xor), the remaining free
word-glyph chars are: **`#`  `)`  `\`  `` ` ``  `{`  `}`** (six). The v0.4 design
might introduce glyphs for: the string primitives (IF `Str` goes in the core), a
capability/invoke marker, and a metering/budget annotation.

## Candidate sweep (in-context token cost)

Each row is the min across the six free chars; full per-char output in
`glyphs_sweep.py`. Templates show the glyph as `G` in its real position.

### If `Str` in core — string primitives (only relevant under Variant A)

| primitive | template (G = glyph) | best glyph(s) | in-context o200k/cl100k | verdict |
|---|---|---|---:|---|
| `str-cons` | `""[~G](` | **`` ` ``** (4) | **4 / 4** (`` ` `` beats `# ) \ { }` at 5) | pick `` ` `` **if** Str-in-core |
| `str-uncons` | `""~[G0=][_][G_[~#]'][]|` | `#` `\` `` ` `` `{` (14) | 14 / 14 (`) }` = 15) | pick `#` if Str-in-core |
| `num-to-str` | `@G#,@` | `#` `)` `\` (3) | 3 / 3 (`` ` `` `{` `}` = 4) | pick `\`* if Str-in-core |

\* distinct-from-others constraint: `str-cons=`` ` ``, `str-uncons=#`,
`num-to-str=\` is a conflict-free optimal-or-near-optimal assignment. But see the
verdict below — **the measurements recommend NOT putting `Str` in the core**, so
these glyphs are contingency picks, not commitments. `\` also carries the
JSON-escape hazard the v0.3 doc flagged (`\\` inside double-quoted prompts).

### Regardless of Str — effects-round glyphs

| candidate | template (G = glyph) | best glyph(s) | in-context o200k/cl100k | verdict |
|---|---|---|---:|---|
| invoke sigil | `read-input fetch parse Gemit` | all six tie (6) | 6 / 6 | **Reject — not glyph-worthy** |
| budget annotation | `G100 read-state[done?][][step][]|` | all six tie (12) | 12 / 12 | **Conditional** — pick `` ` `` or `#` |

## What the numbers say

- **Invoke sigil is not glyph-worthy — reject it.** Capabilities are already
  `Call` words (bare names). The bare pipeline `read-input fetch parse emit`
  is **5 tokens** (see `agentic/two_tool_pipeline.md`); prefixing every name
  with *any* free char pushes `parse emit` → `parse Gemit` and **adds** a token
  (6) while merging with nothing (all six chars tie — no BPE help). A sigil
  spends a scarce glyph to make programs *longer*. Keep capabilities as bare
  `Call` names; no new glyph.
- **Budget annotation is the one plausibly worth a glyph — but only if metering
  becomes a surface feature.** All six free chars tie in-context (12), so the
  choice is on secondary criteria exactly as in v0.3: keep `{ }` in reserve for a
  future array literal, avoid `\` (JSON-escape hazard). That leaves **`` ` `` or
  `#`** as the budget glyph. It is *conditional* on v0.4 actually specifying
  resource metering (fuel/heap/call-budget are all future work, review §19); if
  metering stays a driver-level number (as fuel is today), **no glyph is needed
  at all** — a budget is not part of a program's text.
- **The string-primitive glyphs are contingency-only.** They exist for
  completeness; the string-task and agentic measurements (see `MEASUREMENTS.md`)
  say `Str` should stay **host-side**, so `str-cons`/`str-uncons`/`num-to-str`
  should not be assigned glyphs in v0.4. Spending 1–3 of the six remaining scarce
  chars on a value constructor the benchmark does not demand is exactly the
  anti-tarpit failure v0.3 warns against.
- **Net v0.4 glyph recommendation: introduce ZERO or ONE new glyph.** Zero if
  metering stays driver-level and `Str` stays host-side (the measurement-backed
  default). At most one — a budget annotation `` ` `` / `#` — and only if v0.4
  commits to in-program metering.
