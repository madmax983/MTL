# `fold` (`#`) — before/after solutions with hand-traces

- Status: **design stage**, NOT interpreter-validated. `fold` is not yet in the
  parser/interpreter. Every program below is hand-traced against the semantics
  sketch in `README.md` §3. Every token count is a real `bench/tokcount` output
  (pinned `tiktoken` `o200k_base` + `cl100k_base`, 0.8.0) — see `README.md` §4.
- Provisional glyph `#` (a free ASCII char per the scout briefing; final glyph
  assignment is a separate worker's job). All counts below use `#` in place.
- Input contract is UNCHANGED from the frozen v0.2 solutions: the input list
  (and, for contains/count, the scalar `x`) is already on the stack when the
  program runs, exactly as `bench/validate/tests/tier2.rs` prepends it. So the
  fold rewrite is a drop-in replacement measured on the same contract.

`fold` stack effect: **`( seq init [C] -- result )`, a LEFT fold.** `C` has
effect `( acc w -- acc' )` and is applied once per element, left to right. See
`README.md` §3 for the small-step rule and §3.4 for the mechanical desugar check.

Stack notation below is **bottom → top**.

---

## The 3 clean folds (were already clean under `linrec`; fold still wins)

### sum_list
```
before  [>0=][0][][+]|          o200k 10  cl100k 10
after   0[+]#                    o200k  4  cl100k  4     Δ -6 / -6
```
Trace, list `[3 1 2]` on stack (`✓ hand-traced`):
- `[3 1 2]` → `0` → `[3 1 2] 0` → `[+]` → `[3 1 2] 0 [+]` → `#`
- fold: acc0=0; C(0,3)=0+3=3; C(3,1)=4; C(4,2)=6 → **6** = sum. ✓
  (mechanically re-derived through the desugar in README §3.4.)

### product_list
```
before  [>0=][1][][*]|          o200k  9  cl100k  9
after   1[*]#                    o200k  4  cl100k  4     Δ -5 / -5
```
Trace `[3 1 2]`: acc0=1; 1*3=3; 3*1=3; 3*2=6 → **6** = product. ✓

### length_list
```
before  [>0=][0][][~_1+]|       o200k 13  cl100k 13
after   0[_1+]#                  o200k  5  cl100k  5     Δ -8 / -8
```
C = `[_1+]` : `(acc w -- acc+1)` — drop element `w` (`_`), push `1`, add.
Trace `[3 1 2]`: acc0=0; C(0,3): `0 3 _ 1 +` → `0 1 +` = 1; C(1,1)=2; C(2,2)=3 → **3** = length. ✓

---

## The 5 accumulator/"solved-but-ugly" tasks (the ~73-token juggling tax)

### max_list
```
before  >_[>[;0][[]1]?][_][>_[^^<[~_][_]?]'][]|     o200k 22  cl100k 22
after   >_~[^^<[~_][_]?]#                            o200k 12  cl100k 11   Δ -10 / -11
```
- `>_~` seeds the accumulator from the FIRST element and leaves `[tail] head`
  for `fold ( seq init [C] )`:
  `[3 1 2]` → `>` → `3 [1 2] 1` → `_` (drop flag) → `3 [1 2]` → `~` → `[1 2] 3`.
- C = `[^^<[~_][_]?]` : `(acc w -- max acc w)`. `^^` = over over → `acc w acc w`;
  `<` → `acc w (acc<w)`; if true `[~_]` (swap,drop → keeps `w`), else `[_]` (drop
  `w` → keeps `acc`). (This is the SAME max-combine the v0.2 solution used, now
  reused verbatim as the fold body.)
Trace `[3 1 2]` (`✓ hand-traced`):
- seed acc=3, fold over `[1 2]`.
- C(3,1): `3 1 ^^< → 3 1 (3<1=0)`; false → `[_]` → `3`. acc=3.
- C(3,2): `3 2 (3<2=0)`; false → `[_]` → `3`. acc=3. → **3** = max. ✓
- (empty list ⇒ `>_~` faults Underflow: max of empty is undefined — acceptable,
  same behaviour class as the v0.2 solution.)

### min_list
```
before  >_[>[;0][[]1]?][_][>_[^^<[_][~_]?]'][]|     o200k 23  cl100k 23
after   >_~[^^<[_][~_]?]#                            o200k 13  cl100k 12   Δ -10 / -11
```
Identical shape, branches swapped: true `[_]` (keep acc), false `[~_]` (keep w).
Trace `[3 1 2]`: seed 3.
- C(3,1): `3 1 (3<1=0)`; false → `[~_]` (swap,drop) → `1`. acc=1.
- C(1,2): `1 2 (1<2=1)`; true → `[_]` → `1`. acc=1. → **1** = min. ✓

### reverse_list
```
before  []~[>[;0][[]1]?][_][>_[~;]'][]|             o200k 19  cl100k 19
after   [][~;]#                                      o200k  5  cl100k  5   Δ -14 / -14
```
init = `[]` (empty accumulator list). C = `[~;]` : `(acc w -- w;acc)` — swap so
value is under quote, then `;` cons prepends `w` onto the built list. A LEFT fold
that prepends each element yields the reversal (this is exactly why fold is
defined LEFT — a RIGHT fold with cons would reproduce the input order).
Trace `[3 1 2]` (`✓ hand-traced`):
- seq=`[3 1 2]`, init=`[]`.
- C(`[]`,3): `[] 3 ~ → 3 []`; `;` → `[3]`. acc=`[3]`.
- C(`[3]`,1): `[3] 1 ~ → 1 [3]`; `;` → `[1 3]`. acc=`[1 3]`.
- C(`[1 3]`,2): `[1 3] 2 ~ → 2 [1 3]`; `;` → `[2 1 3]`. → **`[2 1 3]`** = reverse. ✓

### contains
```
before  0~@[>[;0][[]1]?][__][>_[^=@~+0~<~]'][]|     o200k 26  cl100k 26
after   [=+0~<];0~#                                  o200k 10  cl100k 10   Δ -16 / -16
```
Input `list x` (list below, x on top). The carried scalar `x` is CLOSED OVER by
consing it into the combine quote — no new primitive, just the existing `;`:
- `[=+0~<]` push → `list x [=+0~<]`.
- `;` cons (`v [q] -- [v q]`) → `list [x = + 0 ~ <]`   (x baked in as first word).
- `0` `~` → `list 0 [x=+0~<]`   ( = `seq init [C]`, init=found=0 ).
- `#` fold. C = `[x =+0~<]` : `(acc w -- acc OR (w==x))`. Body on `acc w`:
  `x` → `acc w x`; `=` → `acc (w==x)`; `+` → `acc+eq`; `0~<` → `(0 < acc+eq)` = OR
  (valid because both operands are non-negative flags — the v0.2 OR idiom).
Trace `[3 1 2]`, x=1 (`✓ hand-traced`):
- built C = `[1 = + 0 ~ <]`; seq=`[3 1 2]`, init=0.
- C(0,3): `0 3 1 = → 0 0`; `+ → 0`; `0~< → (0<0)=0`. acc=0.
- C(0,1): `0 1 1 = → 0 1`; `+ → 1`; `0~< → (0<1)=1`. acc=1.
- C(1,2): `1 2 1 = → 1 0`; `+ → 1`; `0~< → (0<1)=1`. acc=1. → **1** = contains. ✓
- x=5 (absent): every `eq`=0, acc stays 0 → **0**. ✓

### count_occurrences
```
before  0~@[>[;0][[]1]?][__][>_[^=@~+~]'][]|        o200k 23  cl100k 23
after   [=+];0~#                                     o200k  7  cl100k  7   Δ -16 / -16
```
Same closure trick; combine keeps the running sum instead of OR-clamping.
- `[=+]` `;` → `list [x = +]`; `0~` → `list 0 [x=+]`; `#`.
- C = `[x =+]` : `(acc w -- acc + (w==x))`.
Trace `[3 1 2]`, x=1 (`✓ hand-traced`):
- C(0,3): `0 3 1 = + → 0 0 + = 0`. acc=0.
- C(0,1): `0 1 1 = + → 0 1 + = 1`. acc=1.
- C(1,2): `1 2 1 = + → 1 0 + = 1`. acc=1. → **1** = count of 1. ✓
- On `[1 1 2]`, x=1: 0→1→2→2 → **2**. ✓ (spine-count correct.)

---

## Token summary (measured, both encodings)

| task | before o200k | after o200k | before cl100k | after cl100k | Δ o200k | Δ cl100k |
|---|---:|---:|---:|---:|---:|---:|
| max_list | 22 | 12 | 22 | 11 | -10 | -11 |
| min_list | 23 | 13 | 23 | 12 | -10 | -11 |
| reverse_list | 19 | 5 | 19 | 5 | -14 | -14 |
| contains | 26 | 10 | 26 | 10 | -16 | -16 |
| count_occurrences | 23 | 7 | 23 | 7 | -16 | -16 |
| **5-ugly subtotal** | **113** | **47** | **113** | **45** | **-66** | **-68** |
| sum_list | 10 | 4 | 10 | 4 | -6 | -6 |
| length_list | 13 | 5 | 13 | 5 | -8 | -8 |
| product_list | 9 | 4 | 9 | 4 | -5 | -5 |
| **all-8 total** | **145** | **60** | **145** | **58** | **-85** | **-87** |

Both encodings agree on every BEFORE cell and differ by ≤1 on two AFTER cells
(max/min, where cl100k merges one bigram o200k does not). No task regresses:
fold beats `linrec` on all 8, including the 3 that were already "clean".
