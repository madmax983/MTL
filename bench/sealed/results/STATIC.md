# Held-out static compression — sealed set (issue #53)

- Measured: 2026-07-15  |  Freeze commit: `456292bb3a0b930f51c49fa50e37c339ae4eaf59`
- Tokenizer: tiktoken 0.8.0, encodings o200k_base & cl100k_base
- Token counts via `bench/tokcount/tokcount.py count_file` (strips one trailing newline).
- `ratio = py_idiomatic_tokens / mtl_tokens` (higher = MTL more compressed).

## Validation model (dev-parity)

Solutions are validated the SAME way the dev `BASELINE-TIER2` numbers are: on the
real `mtl-core` interpreter with a **constructed input stack** — real `Value::Int`s
and `int_list` (negatives and negative list elements included), bypassing the text
lexer. A task is **algorithmically correct** iff its committed frozen-glyph solution
passes **all** of its `python.vectors` (including negatives) to `HALT` with the
expected stack. This is enforced by `bench/validate/tests/sealed.rs`.

This corrects the earlier measurement, which validated via the TEXT harness
(`printf '<input> <program>' | mtlrun`). That harness lexes integer input and the
frozen grammar has no negative literal (`-` lexes to the `Sub` primitive), so it
cannot feed negative scalars or negative list elements — 8/15 tasks faulted for an
**input-encoding** reason, not an algorithmic one.

## Headline

- **Algorithmically correct (constructed-stack): 14/15.**
- **1 genuine algorithmic gap:** `seal_running_max` — its authored candidate seeds the
  running maximum at `0`, so an all-negative input like `[-5 -2 -8 -1]` yields
  `[0 0 0 0]` instead of `[-5 -2 -2 -1]`. The candidate is **wrong under full
  validation**; recorded, not patched (issue #53).
- The other 7 text-harness "gaps" are **input-encoding artifacts**, all algorithmically
  correct under constructed stack: 2 `input_encoding(scalar)`, 5 `input_encoding(list)`.
- **Nothing was patched in the frozen language** — no glyph, primitive, interpreter
  semantics, spec, or quickref changed. Only a validation test was added.

## Primary — dev-parity aggregate (14 algorithmically-correct tasks)

Token-SUM ratio over the algorithmically-correct set; comparable to dev BASELINE-TIER2's
3.87x (also constructed-stack-validated).

| scope | py o200k | mtl o200k | ratio o200k | py cl100k | mtl cl100k | ratio cl100k |
|---|--:|--:|--:|--:|--:|--:|
| overall (14) | 533 | 319 | 1.67 | 530 | 308 | 1.72 |
| micro (6) | 197 | 112 | 1.76 | 195 | 113 | 1.73 |
| tier2 (5) | 206 | 117 | 1.76 | 206 | 108 | 1.91 |
| tier3 (3) | 130 | 90 | 1.44 | 129 | 87 | 1.48 |

## Secondary — text-feedable subset (7 tasks, no negative inputs)

Tasks with no negative inputs also pass the text `mtlrun` harness. This subset
reproduces the earlier 2.09x and is the property that matters for the cold-agent trial.

| scope | py o200k | mtl o200k | ratio o200k | py cl100k | mtl cl100k | ratio cl100k |
|---|--:|--:|--:|--:|--:|--:|
| overall (7) | 274 | 131 | 2.09 | 271 | 126 | 2.15 |
| micro (4) | 152 | 62 | 2.45 | 150 | 63 | 2.38 |
| tier2 (2) | 85 | 46 | 1.85 | 85 | 41 | 2.07 |
| tier3 (1) | 37 | 23 | 1.61 | 36 | 22 | 1.64 |

## Gap taxonomy

| class | count | meaning |
|---|--:|---|
| none | 7 | algorithmically correct AND text-feedable |
| input_encoding(scalar) | 2 | algorithmically correct; text harness cannot type a negative scalar (fixable as `0 N -`; NOT a language gap) |
| input_encoding(list) | 5 | algorithmically correct; text harness cannot lex negative list elements (representable as runtime Values) |
| algorithmic | 1 | `seal_running_max` authored candidate is wrong under full validation |

## Per-task table (all 15)

| task | tier | algo-correct | text-feedable | mtl o200k | py o200k | ratio o200k | mtl cl100k | py cl100k | ratio cl100k | gap class |
|---|---|:-:|:-:|--:|--:|--:|--:|--:|--:|---|
| seal_collatz_steps | micro | yes | yes | 23 | 52 | 2.26 | 24 | 52 | 2.17 | none |
| seal_digit_product | micro | yes | no | 28 | 29 | 1.04 | 28 | 29 | 1.04 | input_encoding(scalar) |
| seal_count_set_bits | micro | yes | no | 22 | 16 | 0.73 | 22 | 16 | 0.73 | input_encoding(scalar) |
| seal_triangular | micro | yes | yes | 6 | 17 | 2.83 | 6 | 17 | 2.83 | none |
| seal_int_sqrt | micro | yes | yes | 15 | 39 | 2.6 | 15 | 38 | 2.53 | none |
| seal_num_divisors | micro | yes | yes | 18 | 44 | 2.44 | 18 | 43 | 2.39 | none |
| seal_alternating_sum | tier2 | yes | no | 9 | 39 | 4.33 | 9 | 39 | 4.33 | input_encoding(list) |
| seal_running_max | tier2 | **NO** | no | — | — | — | — | — | — | algorithmic |
| seal_count_local_maxima | tier2 | yes | yes | 43 | 60 | 1.4 | 38 | 60 | 1.58 | none |
| seal_xor_reduce | tier2 | yes | yes | 3 | 25 | 8.33 | 3 | 25 | 8.33 | none |
| seal_max_adjacent_diff | tier2 | yes | no | 42 | 45 | 1.07 | 39 | 45 | 1.15 | input_encoding(list) |
| seal_dedup_adjacent | tier2 | yes | no | 20 | 37 | 1.85 | 19 | 37 | 1.95 | input_encoding(list) |
| seal_rle_flatten | tier3 | yes | no | 45 | 52 | 1.16 | 43 | 52 | 1.21 | input_encoding(list) |
| seal_digit_sum_base | tier3 | yes | yes | 23 | 37 | 1.61 | 22 | 36 | 1.64 | none |
| seal_min_running_balance | tier3 | yes | no | 22 | 41 | 1.86 | 22 | 41 | 1.86 | input_encoding(list) |

_Machine-generated from `results/static_tokens.json`; token counts are ground truth
from `bench/tokcount`. Every algorithmically-correct row traces to committed files in
`bench/sealed/corpus/<task>/`. `seal_running_max` has no committed solution (algorithmic
gap); its wrong candidate is pinned in `bench/validate/tests/sealed.rs` and `GAPS.md`._
