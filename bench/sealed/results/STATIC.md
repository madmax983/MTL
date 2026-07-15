# Held-out static compression — sealed set (issue #53)

- Measured: 2026-07-15  |  Freeze commit: `456292bb3a0b930f51c49fa50e37c339ae4eaf59`
- Tokenizer: tiktoken 0.8.0, encodings o200k_base & cl100k_base
- Token counts via `bench/tokcount/tokcount.py` `count_file` (strips one trailing newline).
- `ratio = py_idiomatic_tokens / mtl_tokens` (higher = MTL more compressed).

## Expressibility

A task is **expressible** iff a frozen-glyph MTL program passes `mtlrun` on **every** one of its I/O vectors under the literal harness contract (`printf '%s' "<input_prefix><program>"`, first line `== HALT: <expected_halt>`).

- **Expressible: 7/15** (pass all literal vectors).
- **Gap: 8/15** — every gap is caused solely by **negative-integer input encoding**: the frozen grammar has no negative literal (`-` lexes to `Sub`), so a vector like `-24 ` or `[-5 -2 ...]` faults (`Underflow` / `TypeMismatch`) *before* the solution runs. The algorithms are expressible — each gap candidate passes all its non-negative vectors (see `GAPS.md`).
- **Algorithmically expressible: 15/15** in the frozen glyph set.

## Per-task tokens (expressible tasks — committed solutions)

| task | tier | mtl o200k | py o200k | ratio o200k | mtl cl100k | py cl100k | ratio cl100k |
|---|---|--:|--:|--:|--:|--:|--:|
| seal_collatz_steps | micro | 23 | 52 | 2.26 | 24 | 52 | 2.17 |
| seal_triangular | micro | 6 | 17 | 2.83 | 6 | 17 | 2.83 |
| seal_int_sqrt | micro | 15 | 39 | 2.6 | 15 | 38 | 2.53 |
| seal_num_divisors | micro | 18 | 44 | 2.44 | 18 | 43 | 2.39 |
| seal_count_local_maxima | tier2 | 43 | 60 | 1.4 | 38 | 60 | 1.58 |
| seal_xor_reduce | tier2 | 3 | 25 | 8.33 | 3 | 25 | 8.33 |
| seal_digit_sum_base | tier3 | 23 | 37 | 1.61 | 22 | 36 | 1.64 |

### Aggregate (token-SUM ratio), expressible tasks only

| scope | py o200k | mtl o200k | ratio o200k | py cl100k | mtl cl100k | ratio cl100k |
|---|--:|--:|--:|--:|--:|--:|
| overall (7) | 274 | 131 | 2.09 | 271 | 126 | 2.15 |
| micro (4) | 152 | 62 | 2.45 | 150 | 63 | 2.38 |
| tier2 (2) | 85 | 46 | 1.85 | 85 | 41 | 2.07 |
| tier3 (1) | 37 | 23 | 1.61 | 36 | 22 | 1.64 |

### Supplementary — all 15 (algorithmic; uses validated gap candidates)

Compression if negative integers were expressible as input. Each gap candidate is a frozen-glyph program that passes every non-negative vector for its task.

| scope | py o200k | mtl o200k | ratio o200k | py cl100k | mtl cl100k | ratio cl100k |
|---|--:|--:|--:|--:|--:|--:|
| overall (15) | 573 | 340 | 1.69 | 570 | 329 | 1.73 |
| micro | 196 | 112 | 1.75 | 194 | 113 | 1.72 |
| tier2 | 247 | 138 | 1.79 | 247 | 129 | 1.91 |
| tier3 | 130 | 90 | 1.44 | 129 | 87 | 1.48 |

### Full per-task token table (all 15)

| task | tier | expr | mtl o200k | py o200k | ratio o200k | mtl cl100k | py cl100k | ratio cl100k |
|---|---|:-:|--:|--:|--:|--:|--:|--:|
| seal_collatz_steps | micro | yes | 23 | 52 | 2.26 | 24 | 52 | 2.17 |
| seal_digit_product | micro | gap | 28 | 28 | 1.0 | 28 | 28 | 1.0 |
| seal_count_set_bits | micro | gap | 22 | 16 | 0.73 | 22 | 16 | 0.73 |
| seal_triangular | micro | yes | 6 | 17 | 2.83 | 6 | 17 | 2.83 |
| seal_int_sqrt | micro | yes | 15 | 39 | 2.6 | 15 | 38 | 2.53 |
| seal_num_divisors | micro | yes | 18 | 44 | 2.44 | 18 | 43 | 2.39 |
| seal_alternating_sum | tier2 | gap | 9 | 39 | 4.33 | 9 | 39 | 4.33 |
| seal_running_max | tier2 | gap | 21 | 42 | 2.0 | 21 | 42 | 2.0 |
| seal_count_local_maxima | tier2 | yes | 43 | 60 | 1.4 | 38 | 60 | 1.58 |
| seal_xor_reduce | tier2 | yes | 3 | 25 | 8.33 | 3 | 25 | 8.33 |
| seal_max_adjacent_diff | tier2 | gap | 42 | 44 | 1.05 | 39 | 44 | 1.13 |
| seal_dedup_adjacent | tier2 | gap | 20 | 37 | 1.85 | 19 | 37 | 1.95 |
| seal_rle_flatten | tier3 | gap | 45 | 52 | 1.16 | 43 | 52 | 1.21 |
| seal_digit_sum_base | tier3 | yes | 23 | 37 | 1.61 | 22 | 36 | 1.64 |
| seal_min_running_balance | tier3 | gap | 22 | 41 | 1.86 | 22 | 41 | 1.86 |

_Machine-generated from `results/static_tokens.json`; token counts are ground truth from `bench/tokcount`. For gap rows, mtl tokens are the validated candidate (not committed as a solution file per issue #53)._
