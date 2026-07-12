# v0.3 glyph assignment — `fold` and `xor` (measured, spec §11)

- **Status:** design stage. Glyphs are *provisional recommendations* from the spec
  §11 BPE measurement protocol; no `crates/`, no `ast.rs` `GLYPHS`, nothing frozen
  is touched. Final freeze happens in the design doc / ADR with this measurement diff.
- **Admitted set measured here:** `fold` (high-frequency: 8 corpus occurrences) +
  `xor` (1 occurrence). `empty?`/`len`/`pick`/`roll` are deferred; indexed-access is
  documented separately (its (a) `nth`/`len` glyphs are handled in §4 below only for
  the scenario-C aggregate).
- **Tokenizers (pinned):** tiktoken `o200k_base` + `cl100k_base`, tiktoken **0.8.0**
  in this env. Every count in this file is a real `bench/tokcount` / raw-tiktoken run.
  Raw token-lists in the Appendix.
- **Free ASCII set** (not in `ast.rs` `GLYPHS`, not `[` `]` delimiters, not the
  reserved `"` string introducer, not `[a-z]` host-names): `` #  $  (  )  \  `  {  } ``.
  Everything else of the 20 punctuation glyphs is taken (`: _ ~ @ ^ ! , ; ' + - * / %
  = < ? & . | >`).

---

## 1. The measurement (spec §11 protocol)

§11 requires the count be taken **corpus-level, in context**, under both pinned
tokenizers — never per-glyph — because BPE merging is context-dependent (`:!` may be
1 token while `:?` is 2). So each candidate glyph was substituted into the **actual
rewritten corpus solutions** at the position it really occupies, and the whole set
re-tokenized.

- **`fold`** is the final word of every fold-rewritten solution. In 6 of 8 it
  immediately follows a `]` (the close of the combine quote): `[+]G`, `[*]G`,
  `[_1+]G`, `…?]G`, `[~;]G`. In contains/count it follows `~` (`…0~G`). So the
  load-bearing bigram is **`]G`** (and secondarily `~G`).
- **`xor`** sits inside `[$]` in `single_number` = `[>0=][0][][G]|`, so its
  load-bearing bigram is **`[G`** (open-bracket + glyph), and `G]|` after.

### 1.1 `fold` glyph sweep — sum over the 8 fold-rewritten solutions

| candidate | o200k total | cl100k total | `]G` merges? |
|---|---:|---:|:--:|
| `(` | **56** | **54** | yes (`](`) |
| `{` | **56** | **54** | yes (`]{`) |
| `\` | **56** | **54** | yes (`]\`) |
| `}` | **56** | **54** | yes (`]}`) |
| `)` | **56** | **54** | yes (`])`) |
| `$` | 60 | 54 | no |
| `#` | 60 | 58 | no |
| `` ` `` | 60 | 58 | no |

**The five bracket-pairing / escape chars `( { \ } )` all tie at the optimum 56/54 —
4 tokens cheaper on both encodings than `#`.** The win is a single BPE fact: the
tokenizers carry `](`, `]{`, `]\`, `]}`, `])` as tokens, so the glyph fuses with the
preceding `]` at zero marginal cost; whereas `]#`, `]$`, `` ]` `` are not tokens, so
`#`/`$`/`` ` `` stay standalone while the orphaned `]` only partly absorbs leftward.

Per-task breakdown at the optimum (`fold = (`) vs the placeholder (`fold = #`):

| task | program (`fold=(`) | o200k `(` | o200k `#` | cl100k `(` | cl100k `#` |
|---|---|---:|---:|---:|---:|
| sum_list | `0[+](` | 4 | 4 | 4 | 4 |
| product_list | `1[*](` | **3** | 4 | **3** | 4 |
| length_list | `0[_1+](` | 5 | 5 | 5 | 5 |
| max_list | `>_~[^^<[~_][_]?](` | **11** | 12 | **10** | 11 |
| min_list | `>_~[^^<[_][~_]?](` | **12** | 13 | **11** | 12 |
| reverse_list | `[][~;](` | **4** | 5 | **4** | 5 |
| contains | `[=+0~<];0~(` | 10 | 10 | 10 | 10 |
| count_occurrences | `[=+];0~(` | 7 | 7 | 7 | 7 |
| **total** | | **56** | 60 | **54** | 58 |

The two `~G` tasks (contains/count) are glyph-insensitive (`~(`=`~#`=`~$`), as
expected — the saving is entirely on the six `]G` tasks.

### 1.2 `xor` glyph sweep — `single_number` = `[>0=][0][][G]|`

| candidate | o200k | cl100k | `[G` merges? |
|---|---:|---:|:--:|
| `$` | **9** | **9** | yes (`[$`) |
| `#` | **9** | **9** | yes (`[#`) |
| `(` | **9** | **9** | yes (`[(`) |
| `{` | **9** | **9** | yes (`[{`) |
| `\` | **9** | **9** | yes (`[\`) |
| `` ` `` | **9** | **9** | yes |
| `}` | 10 | 10 | no |
| `)` | 10 | 10 | no |
| `+` (taken) | 10 | 10 | no |

The bitwise worker's `[$` merge reproduces exactly: `[>0=][0][][$]|` →
`['[','>','0','=','][','0','][]','[$',']|']` = **9**, one token cheaper than the
arithmetic analogue `[+`-fold (`[+` does not merge → 10). Six of the eight free
chars fuse `[G`; only `}` and `)` miss and cost +1.

---

## 2. Interaction, and the recommendation

The two primitives want **overlapping** merge-friendly sets but must be **different
characters** (§11.4 frequency-weighting assigns distinct bigrams to distinct
primitives; and a shared char would be a lexer collision anyway):

- `fold` optimum set: `{ ( { \ } ) }`  (needs the `]G` merge).
- `xor` optimum set: `{ # $ ( { \ ` ` } `  (needs the `[G` merge).
- Intersection (both-optimal): `{ ( { \ }`.

Because the total is separable across the two files, **any (fold, xor) with
fold ∈ {( { \ } )} and xor ∈ {# $ ( { \ `} and fold ≠ xor hits the joint optimum
of 56/54 (fold) + 9/9 (xor).** The choice within the optima is therefore made on
secondary criteria (collision-avoidance with plausible future primitives,
robustness, generatability), not tokens.

### Recommended: **`fold = (`  ·  `xor = $`**

| primitive | glyph | key merge fact | corpus token cost |
|---|:--:|---|---|
| `fold` | **`(`** | `](` is one token in **both** encodings (`[][~;](`→`[][ ~ ; ](`); fuses with the combine-quote's closing `]` at zero marginal cost | **56 o200k / 54 cl100k** over 8 solutions (vs 60/58 for `#`) |
| `xor` | **`$`** | `[$` is one token in **both** encodings (`…][]​[$]|`); fuses with the opening `[` | **9 / 9** for `single_number` (vs 10/10 for `+`-fold) |

Rationale for the specific picks inside the optima:
- **`xor = $`** — canonical from the bitwise worker; `[$` merge is the reason
  `single_number` (9 tok) undercuts the arithmetic-fold shape. Spending `$` here
  frees nothing better for fold (fold cannot use `$`: `]$` does not merge → 60 o200k).
- **`fold = (`** — token-optimal (56/54); and among the five optimal chars it spends
  the one with the least future value: it keeps **`{ }` in reserve for possible
  future array/vector literals** (the indexed-access (b) option explicitly wants
  `{ }` as delimiters), and avoids **`\`**, which must be escaped (`\\`) inside JSON /
  double-quoted prompt strings — a real robustness/generatability hazard for a
  language whose whole point is being emitted by LLMs into JSON contexts.

### Rejected alternatives (and why)
- **`fold = #`** (the hand-trace placeholder) — costs **+4 tokens** on both encodings
  (60/58 vs 56/54). `]#` is not a BPE token. Retained only as the *generatability
  hedge* (see §3).
- **`fold = $`** — `]$` misses on o200k (60) though it ties on cl100k (54);
  encoding-inconsistent, and `$` is better spent on `xor`.
- **`fold = \`** — token-optimal but JSON/shell escape-hostile (`\\`), doubling its
  cost in embedded contexts and inviting model escaping errors.
- **`fold = { / }`** — token-optimal but burns a delimiter char reserved for a future
  array-literal syntax; don't spend a *pair* char on a primitive that needs a *single*
  symbol.
- **`xor = } / )`** — miss the `[G` merge, cost +1 (10 tok).
- **`xor = #`** — ties `$` at 9/9, viable, but `#`/`` ` `` are better held for a
  future primitive or as the fold generatability hedge; keeping `$`↔xor matches the
  already-measured bitwise ADR.

---

## 3. Generatability caveat (spec §11.8)

§11.8 warns the most *compressible* alphabet may not be the most *generatable*: a
tokenizer-optimal glyph can sit on weak learned priors. `#` (hash/comment) is a
strong-prior character across code corpora; a lone unmatched `(` is a weaker, more
surprising prior for a bracket-based concatenative language. We **cannot** run the
generatability ablation here (no model in the loop). So:

- **Token-optimal recommendation: `fold = (`, `xor = $`** (this file's headline, and
  the aggregate's headline).
- **Generatability hedge on the record: `fold = #`** costs a measured **+4 tokens**
  (headline 3.87×/3.92× → 3.71×/3.75×, still clearing 3× comfortably). If the agent
  trial shows `(` is mis-generated, fall back to `#` at that small, quantified cost.

The final pick is the design doc's call and should be revisited after the §10.1
generatability ablation, per §11.5 (re-run on any change, ADR the diff).

---

## 4. Collision + self-delimiting check

- **No collision with the 20 taken glyphs.** `(` and `$` are both in the free ASCII
  set and are not the `[` `]` delimiters nor the reserved `"`. Distinct from each
  other. ✓
- **Self-delimiting / whitespace-free (§2.2/§2.3).** Both are single ASCII punctuation
  words; the maximal-munch lexer scans each as its own symbol word exactly like the
  existing glyphs, so no whitespace is required around them. In the corpus, `(` only
  ever follows `]` or `~` and `$` only sits in `[$]` — neither is ever adjacent to a
  digit, so there is no `01`-style munch hazard (the failure mode that sank the
  pick/roll `n(` proposal). ✓
- **Indexed-access footnote (scenario C only):** if `nth`/`len` (option a) were later
  admitted alongside, `$` is taken by `xor`, so they would draw from the remaining
  free set (`#`, `\`, `` ` ``, `{`, `}`, `)`). Token impact is ≈0 — every free
  single-char glyph measured ~1 token — so the aggregate below re-measures
  binary_search with `len=#`, `nth=\` (37/37, unchanged from the indexed worker's
  `$`/`#` version).

---

## Appendix — raw token-lists (tiktoken 0.8.0)

```
fold  0[+](            o200k ['0','[','+','](']                     = 4
fold  0[+]#            o200k ['0','[','+]','#']                     = 4   (']' absorbs LEFT into '+]', '#' orphaned)
fold  [][~;](          o200k ['[][','~',';','](']                  = 4
fold  [][~;]#          o200k ['[][','~',';',']','#']               = 5   ('](' merge lost -> +1)
fold  [=+];0~(         o200k ['[','=','+','];','0','~','(']        = 7   (~G glyph-insensitive)
xor   [>0=][0][][$]|   o200k ['[','>','0','=','][','0','][]','[$',']|'] = 9
xor   [>0=][0][][#]|   o200k ['[','>','0','=','][','0','][]','[#',']|'] = 9
xor   [>0=][0][][+]|   o200k ['[','>','0','=','][','0','][]','[','+',']|'] = 10
```

Reproduce: `python3 bench/design-v0.3/glyphs/../../../` — see
`scratchpad/glyph_sweep.py` (off the `bench/validate` discovery path; `cargo test` /
frozen baselines unaffected).
