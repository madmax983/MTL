# MTL v0.4 "effects" round — Measurements summary

**Discipline: measurements first.** This file consolidates every token table in
`bench/design-v0.4/` and gives one evidence-backed reading under each. It exists
to decide two v0.4 questions: **(Q1) does the Tier-3 suite demand `Str` in the
core, or do handles + capabilities suffice?** and **(Q2) which new glyphs, if
any, pay for themselves?**

- **Tokenizer path used: real `tiktoken` 0.8.0**, encodings `o200k_base` +
  `cl100k_base` (the pinned bench set). Vocab fetched successfully through the
  proxy; verified by reproducing two known v0.3 counts exactly (`0[+](` → 4/4,
  `[>0=][0][][$]|` → 9/9). **No fallback / no estimated tokenizer was used.**
- Method identical to prior rounds: `bench/tokcount/tokcount.py`,
  `len(enc.encode(text))`, single trailing newline stripped, run from `bench/`.
- **All MTL programs are hand-traced, design-stage, NOT interpreter-validated.**
  reverse and the agentic straight-line/loop tasks are exact hand-traces; the RLE
  and multi-slot loop programs are representative design estimates (±4 band, per
  the v0.3 two_sum precedent). Every token number is a real `tokcount` run;
  commands are in each artifact's "Reproduce" block and in the three
  `measure.py` / `glyphs_sweep.py` scripts.

---

## 1. String tasks — the Str-in-core question (`strings/`)

Reproduce: `python3 design-v0.4/strings/measure.py`.

| program | o200k | cl100k | new core primitives |
|---|---:|---:|---|
| reverse — Python idiomatic | 10 | 10 | — |
| reverse — MTL **Var A** (fold-gen + `str-cons`) | **5** | **5** | `Str` value + `str-cons` + fold-generalisation |
| reverse — MTL Var A (linrec + 2 str prims) | 18 | 18 | `Str` value + `str-cons` + `str-uncons` |
| reverse — MTL **Var B2** (host codepoint-list) | **4** | **4** | **none** (= corpus `reverse_list` `[][~;](`) |
| rle — Python idiomatic | 85 | 84 | — |
| rle — MTL **Var A** (`str-cons`+`num-to-str`+concat) | **28** | **28** | `Str` value + 3 str prims (incl. int→dec `num-to-str`) |
| rle — MTL **Var B2** core (pairs, existing prims) | 31 | 30 | **none**; rendering is a host capability |

*(Var A RLE and Var B2 RLE are design estimates, ±4 band; reverse rows are exact.)*

**What the numbers say.**
- **reverse does not need `Str` in the core, and host-side is strictly better.**
  Var B2 is `[][~;](` — the *existing* `reverse_list` solution, **4 tokens, zero
  new primitives**. It beats Var A (5 tokens + a whole `Str` constructor) on
  tokens *and* on proof cost. Reversal is a structural permutation of a sequence;
  once a string is a sequence (a codepoint `Quote`), there is nothing to add.
- **RLE does not need `Str` in the core either — and here tokens don't even
  decide it.** Var A (28/28) and Var B2 (31/30) are within the ±4 estimate band
  of each other, so the ~3-token difference is noise. The tie-breaker is
  proof-surface cost: Var A buys the `Str` value constructor **plus an in-core
  integer→decimal `num-to-str` routine** to save ≈0 net tokens. RLE's only
  genuinely algorithmic half — detecting runs — needs nothing but `Int` compare
  over a sequence (no `Str`). Its expensive half — rendering `"a3b2c1"` — is
  exactly what a host formatter already does. Split at that seam and the core
  stays `Int | Quote`.
- **Headline string-task numbers: reverse 10→5 (Var A) / 10→4 (host); RLE 85→28
  (Var A) / 85→31 (host-compute).** Both tasks compress well against Python
  regardless — but the compression comes from the *sequence/fold* machinery MTL
  already has, **not** from adding `Str`.

---

## 2. Tier-3 agentic suite (`agentic/`)

Reproduce: `python3 design-v0.4/agentic/measure.py`. Eight tasks; full contracts
in `agentic/<name>.md`; index in `agentic/suite.md`.

| task | Py o200k/cl100k | MTL o200k/cl100k | needs in-core `Str`? |
|---|---:|---:|:--:|
| echo_line | 8 / 8 | 3 / 3 | No |
| grep_filter | 20 / 20 | 12 / 12 | No |
| agent_loop | 24 / 24 | 10 / 10 | No |
| json_field | 13 / 13 | 5 / 5 | No |
| two_tool_pipeline | 10 / 10 | 5 / 5 | No |
| retry_on_fault | 30 / 30 | 12 / 12 | No |
| map_lines_tool | 15 / 15 | 9 / 9 | No |
| word_count | 11 / 11 | 11 / 11 | No\* |
| **TOTAL** | **131 / 131** | **67 / 67** | **0 / 8** |

**Aggregate: 1.96× (o200k) / 1.96× (cl100k).**  \* only if `tokenize` is refused
as a capability (Framing 2 in `word_count.md`).

**What the numbers say.**
- **0 of 8 agentic tasks require `Str` in the core.** Every task pipes opaque
  string *handles* through capabilities; the only in-core compute is `Int`/`Quote`
  control flow. The one "stringy" task, `word_count`, is a `fold` length once
  `tokenize` is a capability.
- **Compression is control-flow-driven.** The wins are the loops — `agent_loop`
  24→10 (2.4×), `retry_on_fault` 30→12 (2.5×) — where `linrec`/`fold` replace
  `while`/`for` + `def`. Capability-name-dominated tasks barely move:
  `word_count` **ties** 11/11 because long `Call` names (`read-text`, `tokenize`,
  `emit-int`) cost several BPE tokens each.
- **1.96× is modest** — essentially the v0.3 tier-2 v0.2 baseline (1.91×), far
  below the fold-driven 3.87× headline. Agentic glue is where MTL compression is
  *thinnest*; the MTL case for this suite rests on **capability confinement /
  safety** (§8.1), not on token compression.

---

## 3. Glyphs (`glyphs.md`)

Reproduce: `python3 design-v0.4/glyphs_sweep.py`. Free chars after v0.3:
`#  )  \  `` ` ``  {  }`.

| candidate | best glyph(s) | in-context o200k/cl100k | verdict |
|---|---|---:|---|
| `str-cons` (`""[~G](`) | `` ` `` | 4 / 4 | contingency (only if Str-in-core) |
| `str-uncons` (linrec pred) | `#`/`\`/`` ` ``/`{` | 14 / 14 | contingency |
| `num-to-str` (`@G#,@`) | `#`/`)`/`\` | 3 / 3 | contingency |
| invoke sigil (`…parse Gemit`) | all tie | 6 / 6 | **Reject — not glyph-worthy** |
| budget annotation (`G100 …`) | all tie | 12 / 12 | **Conditional** (`` ` `` or `#`) |

**What the numbers say.**
- **Invoke sigil: reject.** Bare `Call` names already cost 5 tokens for a
  4-capability pipeline; a sigil makes programs *longer* (6) and merges with
  nothing. Keep capabilities as bare names — no glyph.
- **Budget annotation: the only plausibly-worth-it glyph, and only if v0.4
  actually specifies in-program metering** (fuel/heap/call-budget are all future
  work). All free chars tie in-context, so pick on reserve/escape grounds:
  `` ` `` or `#` (keep `{ }` for arrays, avoid `\`). If metering stays
  driver-level (as fuel is now), **no glyph is needed**.
- **String-primitive glyphs are contingency-only** — the §1 evidence says keep
  `Str` host-side, so they should not be assigned.
- **Net: v0.4 should introduce ZERO or ONE new glyph** (at most a budget marker,
  conditionally).

---

## 4. The verdict — does Tier-3 demand `Str` in the core?

**No. The measurements say handles + capabilities suffice; keep `Str` out of the
core.** The evidence, in one place:

1. **Coverage: 0 of 8 agentic tasks need in-core `Str`** (§2). All string data
   flows as opaque handles through capabilities; in-core work is `Int`/`Quote`
   control flow.
2. **The two named string tasks don't need it either** (§1). `reverse` is
   *cheaper* host-side (4 vs 5 tokens, zero new primitives — it is literally
   `reverse_list`). `rle` is token-neutral (28 vs 31, within the band), so its
   decision falls to proof cost — which favors host-side, because Var A must add
   the `Str` constructor **and** an in-core `num-to-str` to break even.
3. **Proof cost is real and one-directional** (briefing §E.4/E.5). A `Str` leaf
   touches both `Clone` impls + their view-preservation `ensures`, every
   `spec_step_prim` fault-precedence arm, the parser, and P2 lockstep for each
   Str-consuming primitive — for a token payoff the benchmark data says is ≈0.
4. **Token compression on this workload is control-flow-driven, not
   string-driven** (§2). Adding `Str` would spend scarce glyphs and proof budget
   on the part of the workload where MTL is *already* competitive without it.

**My read for the synthesis step:** recommend **Variant B (strings host-side)**
for v0.4 — specifically the **B2 shape**: the host marshals strings ↔ codepoint
`Quote`s at the I/O boundary and owns rendering/tokenizing as capabilities, while
the core does the `Int`/`Quote` compute with the *existing* primitive set. This
keeps `Value = Int | Quote`, spends no new glyphs on strings, and closes the
reverse/RLE and agentic tasks with what the language already has. The v0.4 effort
is better spent on the **effects/capability** surface (the §8.2 two-machine split,
the `HostFault → Resume` contract that `retry_on_fault` needs, and — conditionally
— a metering annotation) than on a `Str` value the numbers do not justify.

If `Str` is ever admitted, it should be for a *non-token* reason (e.g. a task
class that genuinely needs in-core character predicates), admitted the way
`uncons` was — on an explicit coverage rationale, not a compression one — and the
contingency glyphs in §3 are ready.
