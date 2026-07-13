# reverse — MTL Variant B (strings host-side only)

**DESIGN STAGE — hand-traced, not interpreter-validated.** Value stays
`Int | Quote` (no `Str` variant). Strings are **opaque host handles**; string
operations are **capabilities** invoked via the §8.2 two-machine split
(`Invoke(name, stack, cont)` → host runs the capability → `Resume(stack, …)`).

Two host-side designs, from most opaque to most in-core.

## B1 — strings fully opaque; the whole algorithm is one capability

The string never enters the core as anything but a handle `h`. Reversal is a
single host capability:

```
capability  str-reverse : ( h -- h' )     -- host allocates reversed string
program (MTL):   str-reverse
```

- **MTL tokens: ~3** (`str-reverse` as a `Call` name) — but the core does **no
  compute**. All work is in the host (TCB). This is a *category error* for the
  benchmark: the headline metric (correct solutions per token) would be
  measuring the host's Python, not MTL. Admissible only if reversal is genuinely
  a host-provided service you are orchestrating, not the task under test.
- **In-core expressible? No.** With opaque handles the core literally cannot see
  characters; there is no in-core algorithm at all.

## B2 — host marshals string ↔ codepoint-list; compute is in-core (RECOMMENDED)

The host provides two thin **I/O-boundary** capabilities; the reversal itself
runs in the core over a `Quote` of `Int` codepoints, using **only existing
primitives** — no `Str` value, no new primitive:

```
capability  read-codepoints : ( -- [cp...] )   -- host: bytes  -> Quote<Int>
capability  emit-codepoints : ( [cp...] -- )   -- host: Quote<Int> -> bytes out

program (MTL, in-core reversal of the codepoint list):
    [][~;](
```

`[][~;](` **is the frozen corpus `reverse_list` solution verbatim** — reversal
of a sequence is reversal of a sequence regardless of whether the elements are
"characters." Full agent shape (read, compute, emit):

```
    read-codepoints  [][~;](  emit-codepoints
```

- **In-core compute: `[][~;](` — o200k 4 / cl100k 4, ZERO new primitives.**
- The only string-specific code (`read-codepoints`, `emit-codepoints`) is host
  marshaling, living where I/O already lives (the TCB). It is written once and
  reused by every string task.
- **Hand-trace** (codepoints of `"abc"` = `[97 98 99]`): `read-codepoints` →
  `[97 98 99]`; `[][~;](` → fold prepend → `[99 98 97]`; `emit-codepoints` →
  `"cba"`. ✓

## Verdict for reverse

| design | MTL tokens (o200k/cl100k) | new core primitives | in-core compute? |
|---|---:|---|---|
| Variant A, fold-gen + str-cons | 5 / 5 | Str value + `str-cons` + fold-gen | yes |
| Variant A, linrec + 2 str prims | 18 / 18 | Str value + `str-cons` + `str-uncons` | yes |
| **B2, host codepoint-list** | **4 / 4** | **none** | **yes (existing prims)** |
| B1, opaque capability | ~3 | none (but a whole host reverser) | no |

**reverse does NOT need `Str` in the core.** B2 is *both* cheaper in tokens (4 vs
5) *and* free of proof-surface cost — it reuses `reverse_list` unchanged. The
only honest reason to prefer Variant A is if strings must be first-class values
for *other* reasons; reverse alone does not justify the `Str` constructor.

## Reproduce
```
cd /home/user/MTL/bench
printf '%s' '[][~;](' | python3 tokcount/tokcount.py     # 4/4  (in-core B2)
```
