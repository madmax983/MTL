# GAPS — sealed set (issue #53)

Post-freeze unseal. A task is a **gap** if no frozen-glyph MTL program passes `mtlrun` on **all** of its I/O vectors under the literal harness contract. Per issue #53 / §10.1, gaps are **recorded, not patched** — no glyph or primitive was added, invented, or modified in response to any sealed task.

## Summary

- **8/15 tasks are gaps.**
- **Every gap has the same single cause: negative-integer INPUT encoding.** MTL integer literals are unsigned and `-` always lexes to the `Sub` primitive (quickref §Literals). A scalar vector `-24 ` lexes to `Sub 24` and faults `Underflow` on the empty initial stack; a list vector `[-5 -2 ...]` lexes to the quote `[Sub 5 Sub 2 ...]`, whose head is a primitive, so `>`/`(`/uncons fault `TypeMismatch`. In both cases the fault fires **before the solution runs**, so no appended program can satisfy the vector.
- This is an **input-encoding gap, not an algorithmic/primitive gap.** All 15 task algorithms are expressible with the frozen glyph set: each candidate below passes every one of its **non-negative** vectors under `mtlrun`. The dev tier-2 measurement never hit this because that harness builds the input stack directly in Rust (`Vm::with_stack`, real `Value::Int(-5)`), bypassing the lexer; the sealed harness prepends the input as text and lexes it.
- The sealed vectors were **authored blind to the MTL glyph set** (`bench/sealed/AUTHORSHIP.md`), which deliberately included negative test data; the unsigned-literal grammar cannot present it at the input boundary.

Recorded, not patched (issue #53 / §10.1). Admitting a negative-literal lexer rule (or any primitive) here would re-open the freeze.

## Gap entries

### seal_digit_product  (micro, arithmetic)

**Spec.** Given an integer n, take its absolute value and multiply together all of its decimal digits. If n is 0 the product is 0. The sign of n is ignored.

**Signature.** `f(n) -> int`

**Missing frozen-glyph capability.** No negative-integer input encoding: the grammar has only unsigned literals and `-` = `Sub`. Failing vector(s): `-24`, `-706` — these lex to `Sub`-containing forms that fault before the program runs.

**Algorithmic expressibility (evidence it is NOT a primitive gap).** Frozen-glyph candidate program:

```
:0<[0~-][]?:0=[_0][1~[:0=][_][:10/~10%@*~][]|]?
```
Passes **5/5** non-negative vectors under `mtlrun` (the only failing vectors are the negative-input ones above). Not committed as a solution file, since it does not pass all vectors.

Recorded, not patched (issue #53 / §10.1). Admitting a primitive here would re-open the freeze.

### seal_count_set_bits  (micro, arithmetic)

**Spec.** Given an integer n, take its absolute value and return the number of 1-bits in the binary representation of that absolute value (its population count). The absolute value of 0 has zero 1-bits.

**Signature.** `f(n) -> int`

**Missing frozen-glyph capability.** No negative-integer input encoding: the grammar has only unsigned literals and `-` = `Sub`. Failing vector(s): `-5`, `-256` — these lex to `Sub`-containing forms that fault before the program runs.

**Algorithmic expressibility (evidence it is NOT a primitive gap).** Frozen-glyph candidate program:

```
:0<[0~-][]?0~[:0=][_][:2/~2%@+~][]|
```
Passes **5/5** non-negative vectors under `mtlrun` (the only failing vectors are the negative-input ones above). Not committed as a solution file, since it does not pass all vectors.

Recorded, not patched (issue #53 / §10.1). Admitting a primitive here would re-open the freeze.

### seal_alternating_sum  (tier2, fold)

**Spec.** Given a list of integers xs, return xs[0] - xs[1] + xs[2] - xs[3] + ... , alternating signs starting with a plus on the first element (index 0 is added, index 1 is subtracted, index 2 is added, and so on). For an empty list the result is 0.

**Signature.** `f(xs) -> int`

**Missing frozen-glyph capability.** No negative-integer input encoding: the grammar has only unsigned literals and `-` = `Sub`. Failing vector(s): `[-1 -2 -3]` — these lex to `Sub`-containing forms that fault before the program runs.

**Algorithmic expressibility (evidence it is NOT a primitive gap).** Frozen-glyph candidate program:

```
[>0=][0][][-]|
```
Passes **6/6** non-negative vectors under `mtlrun` (the only failing vectors are the negative-input ones above). Not committed as a solution file, since it does not pass all vectors.

Recorded, not patched (issue #53 / §10.1). Admitting a primitive here would re-open the freeze.

### seal_running_max  (tier2, map)

**Spec.** Given a list of integers xs, return a list of the same length in which element i is the maximum of xs[0..i] inclusive (the running / prefix maximum). For an empty list return an empty list.

**Signature.** `f(xs) -> list[int]`

**Missing frozen-glyph capability.** No negative-integer input encoding: the grammar has only unsigned literals and `-` = `Sub`. Failing vector(s): `[-5 -2 -8 -1]` — these lex to `Sub`-containing forms that fault before the program runs.

**Algorithmic expressibility (evidence it is NOT a primitive gap).** Frozen-glyph candidate program:

```
0~[>0=][_[]][[^^<[~_][_]?]'[:]'][;]|
```
Passes **6/6** non-negative vectors under `mtlrun` (the only failing vectors are the negative-input ones above). Not committed as a solution file, since it does not pass all vectors.

Recorded, not patched (issue #53 / §10.1). Admitting a primitive here would re-open the freeze.

### seal_max_adjacent_diff  (tier2, sequence)

**Spec.** Given a list of integers xs, return the maximum over all adjacent pairs of the absolute difference |xs[i] - xs[i-1]| for i from 1 to len(xs)-1. If the list has fewer than 2 elements, return 0.

**Signature.** `f(xs) -> int`

**Missing frozen-glyph capability.** No negative-integer input encoding: the grammar has only unsigned literals and `-` = `Sub`. Failing vector(s): `[-5 5]`, `[0 -100 100]` — these lex to `Sub`-containing forms that fault before the program runs.

**Algorithmic expressibility (evidence it is NOT a primitive gap).** Frozen-glyph candidate program:

```
[:>[~_][[]]?>[__0][1]?][_0][:>_>__-:0<[0~-][]?~>_~_][^^<[~_][_]?]|
```
Passes **5/5** non-negative vectors under `mtlrun` (the only failing vectors are the negative-input ones above). Not committed as a solution file, since it does not pass all vectors.

Recorded, not patched (issue #53 / §10.1). Admitting a primitive here would re-open the freeze.

### seal_dedup_adjacent  (tier2, sequence)

**Spec.** Given a list of integers xs, return a new list that collapses each run of consecutive equal elements into a single copy of that element, preserving order. Non-adjacent duplicates are kept. An empty list returns an empty list.

**Signature.** `f(xs) -> list[int]`

**Missing frozen-glyph capability.** No negative-integer input encoding: the grammar has only unsigned literals and `-` = `Sub`. Failing vector(s): `[-1 -1 0 0 -1]` — these lex to `Sub`-containing forms that fault before the program runs.

**Algorithmic expressibility (evidence it is NOT a primitive gap).** Frozen-glyph candidate program:

```
[][^>[_^=][0]?[_][~;]?]([][~;](
```
Passes **6/6** non-negative vectors under `mtlrun` (the only failing vectors are the negative-input ones above). Not committed as a solution file, since it does not pass all vectors.

Recorded, not patched (issue #53 / §10.1). Admitting a primitive here would re-open the freeze.

### seal_rle_flatten  (tier3, statemachine)

**Spec.** Given a list of integers xs, run-length encode consecutive equal elements and return the result flattened into a single list of integers as [value1, count1, value2, count2, ...], where each (value, count) pair describes one maximal run in order. An empty input returns an empty list.

**Signature.** `f(xs) -> list[int]`

**Missing frozen-glyph capability.** No negative-integer input encoding: the grammar has only unsigned literals and `-` = `Sub`. Failing vector(s): `[-1 -1 -1 0]` — these lex to `Sub`-containing forms that fault before the program runs.

**Algorithmic expressibility (evidence it is NOT a primitive gap).** Frozen-glyph candidate program:

```
[][~>[@:@:>__~[=]'~[~_~1+~;][~[;]'~;1~;]?][[];1~;]?]([][~;](
```
Passes **6/6** non-negative vectors under `mtlrun` (the only failing vectors are the negative-input ones above). Not committed as a solution file, since it does not pass all vectors.

Recorded, not patched (issue #53 / §10.1). Admitting a primitive here would re-open the freeze.

### seal_min_running_balance  (tier3, statemachine)

**Spec.** A balance starts at the integer value start. Process the list of integer deltas xs in order, adding each delta to the running balance. Return the minimum balance ever observed, where the initial value start counts as an observation before any delta is applied. If xs is empty the answer is start.

**Signature.** `f(start, xs) -> int`

**Missing frozen-glyph capability.** No negative-integer input encoding: the grammar has only unsigned literals and `-` = `Sub`. Failing vector(s): `0 [-1 -2 3]`, `5 [-10 20]`, `0 [-5 -5 20 -100]`, `-3 [1 1 1]` — these lex to `Sub`-containing forms that fault before the program runs.

**Algorithmic expressibility (evidence it is NOT a primitive gap).** Frozen-glyph candidate program:

```
^~[>0=][_][[+:[^^<[_][~_]?]']'][]|
```
Passes **3/3** non-negative vectors under `mtlrun` (the only failing vectors are the negative-input ones above). Not committed as a solution file, since it does not pass all vectors.

Recorded, not patched (issue #53 / §10.1). Admitting a primitive here would re-open the freeze.

