# Post-fix demonstration — negative-integer text input via `mtlrun --input`

> **Scope note.** This is a **post-fix demonstration** document. It records that
> the previously encoding-blocked sealed tasks now execute end-to-end through the
> `mtlrun` TEXT harness once negative/list inputs are supplied via the new
> `--input` flag. **`bench/BASELINE-SEALED.md` remains the frozen baseline** and
> is NOT edited by this work. No frozen sealed artifact, manifest, task vector,
> spec clause, glyph, primitive, or interpreter semantic was changed. The
> Option-A lexer decision (integer literals unsigned; `-` always lexes to `Sub`,
> spec §2.3) **STANDS** — this fix is purely at the harness input-encoding
> boundary.

## The problem (recap)

The frozen program lexer is unsigned: `-5` in program/input text lexes as the
`Sub` primitive applied to `5`, which faults before the solution runs. The
sealed post-mortem (PR #91) found 8 of 15 sealed tasks blocked in the TEXT
harness for this reason — they have committed, algorithmically-correct solutions
that only ran under the Rust-only constructed-stack path (`run_program` /
`Vm::with_stack`, exercised by `bench/validate/tests/sealed.rs`, 14/14). No CLI
exposed that path, so the text harness could not feed a negative scalar or a list
with negative elements.

Concretely, the old prepended-literal input path faults:

```
$ mtlrun "-24 <seal_digit_product solution>"
FAULT: Underflow
  stack: <empty>
  next:  [Sub 24 Dup 0 Lt ... ]

$ mtlrun "[-1 -2 -3] <seal_alternating_sum solution>"
FAULT: TypeMismatch
  stack: [Sub 1 Sub 2 Sub 3]
  next:  [Uncons 0 Eq ... ]
```

The list case is unfixable with the `0 N -` operational idiom: `[0 5 -]` is a
*quote of the words* `Sub 5`, whose head is `Sub`, so `uncons`/`fold` fault
`TypeMismatch`. Only a constructed-stack input path can express negative list
elements.

## The fix — design (b) + (a)

**(b) `mtlrun --input <spec>`** — a harness-level input decoder that builds the
initial VM stack directly from a **signed** input spec, bypassing the frozen
program lexer *for inputs only*. The program text is still parsed by the normal
unsigned lexer; only the `--input` value flows through the new decoder
`mtl_bench_validate::parse_input_stack`. This is NOT a language change — it is
exactly analogous to the constructed-stack path the Rust tests already use, and
a flat integer list decodes to `Value::Quote(vec![PushInt(..), ..])`
byte-identically to the `int_list` helper in `bench/validate/tests/sealed.rs`.

**(a) Documented convention** — `docs/mtl-quickref.md` (and a terse wrapper line
in `docs/mtl-quickref-min.md`) now cover: negative *constants* in program text
use `0 N -`; negative or list *inputs* use `mtlrun --input`.

### `--input` syntax

```
--input '<spec>'      (also accepts --input=<spec>)
```

- Whitespace-separated **top-level items**, pushed in order so item *i* lands at
  stack position *i* (bottom..top) — matching how the sealed `args` array seeds
  the constructed stack in `tests/sealed.rs`.
- An item is either a **signed integer** (`5`, `-24`) or a **bracketed list**
  (`[]`, `[5 2]`, `[-5 -2 -8 -1]`); lists nest, though the sealed vectors need
  only one level.
- Signed superset of the old unsigned input, so positive specs decode
  identically. Absent `--input`, `mtlrun` behaves exactly as before (empty stack).

Chosen because it round-trips the sealed `python.vectors` `args` shape directly
(scalar, list, and the `scalar list` pair used by `seal_min_running_balance`),
and constructs the same `Value`s the Rust tests seed — so the text path and the
constructed-stack Rust path agree by construction.

## Per-task re-run through the FIXED text path

Every vector of all 7 encoding-blocked tasks (with committed solutions) now
executes and produces the expected `HALT:` line — including the negative vectors
that previously faulted. The program text is the committed
`solution.mtl`, unchanged.

| Task | Kind | Sample negative vector | Before | After |
|------|------|------------------------|--------|-------|
| `seal_digit_product` | scalar | `--input '-24'` | faulted (`Underflow`, `Sub` on empty stack) | `HALT: 8` ✅ |
| `seal_count_set_bits` | scalar | `--input '-5'` | faulted (`Underflow`) | `HALT: 2` ✅ |
| `seal_alternating_sum` | list | `--input '[-1 -2 -3]'` | faulted (`TypeMismatch`, head `Sub`) | `HALT: -2` ✅ |
| `seal_max_adjacent_diff` | list | `--input '[-5 5]'` | faulted (`TypeMismatch`) | `HALT: 10` ✅ |
| `seal_dedup_adjacent` | list | `--input '[-1 -1 0 0 -1]'` | faulted (`TypeMismatch`) | `HALT: [-1 0 -1]` ✅ |
| `seal_rle_flatten` | list | `--input '[-1 -1 -1 0]'` | faulted (`TypeMismatch`) | `HALT: [-1 3 0 1]` ✅ |
| `seal_min_running_balance` | scalar+list | `--input '0 [-5 -5 20 -100]'` | faulted (`TypeMismatch`) | `HALT: -90` ✅ |

**All 7 tasks pass ALL of their vectors** (positive and negative), verified
programmatically against `bench/sealed/tasks.json` `python.vectors`.

### Reproducible invocations (real output)

```
$ PROG=$(cat bench/sealed/corpus/seal_digit_product/mtl/solution.mtl)
$ mtlrun --input '-24' "$PROG"
HALT: 8
$ mtlrun --input '-706' "$PROG"
HALT: 0

$ PROG=$(cat bench/sealed/corpus/seal_count_set_bits/mtl/solution.mtl)
$ mtlrun --input '-5' "$PROG"
HALT: 2
$ mtlrun --input '-256' "$PROG"
HALT: 1

$ PROG=$(cat bench/sealed/corpus/seal_alternating_sum/mtl/solution.mtl)
$ mtlrun --input '[-1 -2 -3]' "$PROG"
HALT: -2

$ PROG=$(cat bench/sealed/corpus/seal_max_adjacent_diff/mtl/solution.mtl)
$ mtlrun --input '[0 -100 100]' "$PROG"
HALT: 200

$ PROG=$(cat bench/sealed/corpus/seal_dedup_adjacent/mtl/solution.mtl)
$ mtlrun --input '[-1 -1 0 0 -1]' "$PROG"
HALT: [-1 0 -1]

$ PROG=$(cat bench/sealed/corpus/seal_rle_flatten/mtl/solution.mtl)
$ mtlrun --input '[-1 -1 -1 0]' "$PROG"
HALT: [-1 3 0 1]

$ PROG=$(cat bench/sealed/corpus/seal_min_running_balance/mtl/solution.mtl)
$ mtlrun --input '0 [-5 -5 20 -100]' "$PROG"
HALT: -90
```

## The 8th blocked task — `seal_running_max` — honest note

`seal_running_max` is **NOT** resolved by this fix, and this document does not
pretend otherwise. It has **no committed solution** because its authored
candidate is **algorithmically wrong**, independent of input encoding: it seeds
the running maximum at `0`, so an all-negative input such as `[-5 -2 -8 -1]`
yields `[0 0 0 0]` instead of the correct `[-5 -2 -2 -1]` (see
`bench/sealed/GAPS.md` §"Remaining gap entry"). This is pinned as a permanent
regression by `bench/validate/tests/sealed.rs::running_max_candidate_is_algorithmically_wrong`.
The encoding fix makes the input *expressible*; it does not and cannot fix a
seed-value bug in the algorithm. `seal_running_max` remains the single genuine
algorithmic gap.

## Files changed by this fix

- `bench/validate/src/lib.rs` — added `pub fn parse_input_stack(&str) -> Result<Vec<Value>, String>` (harness input decoder) + supporting `tokenize_input` / `parse_input_word` / `word_to_value`, plus 11 `#[cfg(test)]` unit tests.
- `bench/validate/src/bin/mtlrun.rs` — added `--input` / `--input=` flag wiring; seeds the constructed stack via `run_program`; header comment updated. Backward compatible (no `--input` → identical old empty-stack behavior).
- `docs/mtl-quickref.md` — documented the `0 N -` in-program idiom and the `--input` convention for negative/list inputs.
- `docs/mtl-quickref-min.md` — one terse wrapper line (frozen primitive block left byte-identical to the ablation-winner variant source).

## Verification

`cargo test --workspace` is green: **321 passed / 0 failed** (baseline 310 + 11
new decoder unit tests). The dataset contamination gates
`manifest_matches_sealed_tasks` and `sealed_disjoint_from_dev` still pass,
proving no frozen artifact/manifest was touched.
