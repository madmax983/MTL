# rle (run-length encoding) — MTL Variant B (strings host-side only)

**DESIGN STAGE — value-level hand-trace; token counts are design estimates
(±4 band, per the v0.3 two_sum precedent).** Value stays `Int | Quote` (no
`Str`). Strings are opaque host handles; string ops are capabilities (§8.2).

RLE is the more interesting case than reverse, because it has two halves with
opposite cost profiles: **run-DETECTION** (cheap Int compare over a sequence) and
**run-RENDERING** (`"a3b2c1"` — Int→decimal-string synthesis, the expensive
part). The host-side split lets each half live where it is cheapest.

## B1 — fully opaque; one capability

```
capability  str-rle : ( h -- h' )
program (MTL):   str-rle
```
~3 MTL tokens, **no in-core compute** — measures the host. Same category-error
caveat as reverse B1. In-core expressible? No (opaque handles).

## B2 — host marshals I/O; DETECTION in-core, RENDERING host-side (RECOMMENDED)

Split the algorithm at its natural seam:

```
capability  read-codepoints : ( -- [cp...] )        -- host: bytes -> Quote<Int>
capability  render-rle       : ( [[cp cnt]...] -- )  -- host: pairs -> "a3b2c1" bytes

program (MTL, in-core run-detection over the codepoint list -> [[cp cnt]...]):
    []~[^0=[[;1];][>_@^=[1+~;;][@[;1]]?]?](~[~;](
```

- **In-core compute: o200k 31 / cl100k 30** (design estimate, ±4 band), using
  **ONLY existing primitives** — `uncons >`, `cons ;`, `eq =`, `fold (`. No
  `Str` value, no `str-cons`, and crucially **no `num-to-str`**: the counts stay
  as `Int`s inside `[cp cnt]` pairs. The fold folds each codepoint into a
  reversed pair-list (bump the head pair's count on a match, else prepend a fresh
  `[cp 1]`); the trailing `~[~;](` restores order.
- **Rendering** (`[[97 3][98 2][99 1]] → "a3b2c1"`) is the `render-rle` host
  capability — **0 MTL tokens**, and it is exactly a host `"".join(chr(cp)+str(n)
  for cp,n in pairs)`. The `num-to-str` + concat that Variant A pays for **in the
  core** is deleted; it lives in the host formatter, next to the I/O it belongs
  with.
- **Hand-trace** (value level, `"aaabbc"` → codepoints `[97 97 97 98 98 99]`):
  fold builds reversed pairs `[[99 1][98 2][97 3]]`; `~[~;](` reverses →
  `[[97 3][98 2][99 1]]`; `render-rle` → `"a3b2c1"`. ✓  Edge `""`: `[]` in, fold
  yields `[]`, render → `""`. ✓

## Verdict for rle

| design | MTL tokens (o200k/cl100k) | new core primitives | renders in core? |
|---|---:|---|---|
| Variant A (str-cons + num-to-str + concat) | 28 / 28 (est) | Str value + 3 str prims | yes (`num-to-str`) |
| **B2, host codepoint-list + host render** | **31 / 30 (est)** | **none** | no (host formatter) |
| B1, opaque capability | ~3 | none (but a whole host RLE) | n/a |

**rle does NOT need `Str` in the core either.** The MTL token counts are within
noise of each other (28 vs 31 — inside the ±4 estimate band), so tokens do **not**
decide it. What decides it is proof-surface cost: Variant A buys the `Str` value
constructor **plus an in-core integer→decimal `num-to-str` routine** to save ~0
net tokens, while B2 keeps the core at `Int | Quote` and pushes rendering into
the host formatter that already exists for I/O. The run-detection — the only part
that is genuinely *algorithmic* — needs nothing but `Int` compare over a sequence.

## Reproduce
```
cd /home/user/MTL/bench
printf '%s' '[]~[^0=[[;1];][>_@^=[1+~;;][@[;1]]?]?](~[~;](' | python3 tokcount/tokcount.py  # 31/30
```
