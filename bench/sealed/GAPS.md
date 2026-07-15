# GAPS — sealed set (issue #53)

Post-freeze unseal, **re-validated dev-parity**. This file reclassifies the 8
tasks that fail the *text* `mtlrun` harness after validating every sealed
solution the way the dev `BASELINE-TIER2` numbers are validated: on the real
`mtl-core` interpreter with a **constructed input stack** (real `Value::Int`s and
`int_list`, negatives and negative list elements included). See
`bench/validate/tests/sealed.rs` and `results/STATIC.md`.

Per issue #53 / §10.1, gaps are **recorded, not patched** — no glyph, primitive,
interpreter semantics, spec, or quickref was added, invented, or modified in
response to any sealed task. The only code added is a validation test.

## Why the text harness undercounts expressibility

The text harness lexes integer input (`printf '<input> <program>' | mtlrun`). The
frozen grammar has only **unsigned** integer literals and `-` always lexes to the
`Sub` primitive (quickref §Literals). So:

- a scalar vector `-24 ` lexes to `Sub 24` and faults `Underflow` on the empty
  initial stack;
- a list vector `[-5 -2 ...]` lexes to the quote `[Sub 5 Sub 2 ...]`, whose head
  is a primitive, so `>` / `(` / uncons fault `TypeMismatch`.

In both cases the fault fires **before the solution runs**. This is an
**input-encoding** limitation of the text boundary, not an algorithmic one: the
dev harness (and now `tests/sealed.rs`) builds the input stack directly as
runtime `Value`s, so negatives are presented fine.

## Reclassification of the 8 text-harness gaps

| class | count | tasks |
|---|--:|---|
| `input_encoding(scalar)` | 2 | seal_digit_product, seal_count_set_bits |
| `input_encoding(list)` | 5 | seal_alternating_sum, seal_max_adjacent_diff, seal_dedup_adjacent, seal_rle_flatten, seal_min_running_balance |
| `algorithmic` | 1 | seal_running_max |

- **`input_encoding(scalar)`** — the text harness cannot *type* a negative scalar
  (`-5` lexes to `Sub`). Trivially fixable at the text boundary as `0 N -`; it is
  **not** a language gap. Both tasks are **algorithmically correct** under
  constructed-stack validation and are now committed corpus solutions.
- **`input_encoding(list)`** — the text harness cannot *lex* negative list
  elements. This is a genuine text-encoding limitation, but the value is
  representable at runtime (`int_list(&[-5, ...])`), so all five tasks are
  **algorithmically correct** under constructed-stack validation and are now
  committed corpus solutions.
- **`algorithmic`** — truly a defect in the frozen glyph set / authored program,
  independent of input encoding. **Count: 1** (`seal_running_max`, below).

**Algorithmic gaps = 1** (not 0). The headline is therefore: *14/15 sealed tasks
are algorithmically expressible and correct in the frozen glyph set; 7 of the 8
text-harness "gaps" are pure input-encoding artifacts; the one remaining gap is a
real algorithmic error in the authored `seal_running_max` candidate.*

The `input_encoding(*)` tasks moved to committed corpus solutions
(`bench/sealed/corpus/<task>/`): each passes ALL its vectors — negatives included —
under `tests/sealed.rs`. They were authored post-freeze using only frozen glyphs.

## Remaining gap entry — `seal_running_max` (tier2, map) — ALGORITHMIC

**Spec.** Given a list of integers xs, return a list of the same length in which
element i is the maximum of xs[0..i] inclusive (the running / prefix maximum). For
an empty list return an empty list.

**Signature.** `f(xs) -> list[int]`

**Authored candidate (verbatim, NOT committed — wrong under full validation):**

```
0~[>0=][_[]][[^^<[~_][_]?]'[:]'][;]|
```

**Why it is a genuine algorithmic gap.** The candidate seeds the running maximum
with the literal `0`. For inputs whose running maximum is ever negative, `max(0,
x) = 0` dominates and the wrong value is emitted. Concretely the vector
`[-5 -2 -8 -1]` (real negative list elements, fed via constructed stack) yields
`[0 0 0 0]` instead of the expected `[-5 -2 -2 -1]`. It passed the *text* harness
earlier only because that harness could never present the all-negative vector —
the bug was masked by the very input-encoding limitation described above.

This is **not** an input-encoding artifact: `tests/sealed.rs::running_max_candidate_is_algorithmically_wrong`
feeds the negative list as a runtime `Value` and the candidate still computes the
wrong result. The algorithm is expressible in principle (seed from the first
element, as `seal_max_adjacent_diff` and `seal_min_running_balance` do), but the
**authored** candidate is incorrect and, per the honesty rules for this unseal, is
**recorded, not silently rewritten**. No committed solution exists for this task
and it is excluded from the algorithmically-correct compression aggregate.

Recorded, not patched (issue #53 / §10.1).
