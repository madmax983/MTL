# bitwise — solution sources, traces, token tables

Design-stage sources for the v0.3 `xor` candidate. Not on the `bench/validate` discovery path;
not in `tasks.json`. Token counts are real `bench/tokcount` runs (tiktoken 0.8.0, both encodings).

## Sources

### single_number (v0.3, `xor`)
```
[>0=][0][][$]|
```
linrec fold, structurally identical to the clean arithmetic folds
(`sum_list = [>0=][0][][+]|`, `product_list = [>0=][1][][*]|`). Reading of the four quotes,
`linrec ( [P] [T] [R1] [R2] -- … )`:
- `P = >0=` : uncons the list; if it was empty, uncons pushed flag `0`, then `0=` → `1` (true,
  take base); if non-empty, uncons pushed head + tail + `1`, then `0=` → `0` (false, recurse).
  The head is left on the stack for the unwind.
- `T = 0`   : base case pushes the XOR identity `0`.
- `R1 = []` : empty (no work on the way down).
- `R2 = $`  : on the way back up, XOR the returned accumulator with this level's head.

Provisional glyph for `xor` is `$` (final assignment is a separate worker's job).

### Python reference (idiomatic)
`bench/corpus/single_number/python-idiomatic/solution.py`:
```python
def single_number(xs):
    r = 0
    for x in xs:
        r ^= x
    return r
```

## Token tables (measured — real bench/tokcount output)

| program | o200k | cl100k |
|---|---:|---:|
| `[>0=][0][][$]\|` (MTL v0.3 single_number) | **9** | **9** |
| `python-idiomatic/solution.py` | **25** | **25** |
| ratio py/mtl | **2.78×** | **2.78×** |

Free-glyph sweep for the fold slot `[>0=][0][][G]|` (both encodings equal):

| `G` | tokens | | `G` | tokens |
|---|---:|---|---|---:|
| `$` | 9 | | `{` | 9 |
| `#` | 9 | | `\` | 9 |
| `(` | 9 | | `)` | 10 |
|     |   | | `}` | 10 |

Verbatim splits (measured):
```
o200k  [>0=][0][][$]|  -> ['[','>','0','=','][','0','][]','[$',']|']    = 9
o200k  [>0=][0][][+]|  -> ['[','>','0','=','][','0','][]','[','+',']|'] = 10   (arith analogue, for contrast)
```
`[$` is one token; `[+` is two — so `xor`'s scarce glyph is *cheaper* than the arith glyph here.

## 7. Hand-trace (design stage — NOT interpreter-validated)

**single_number `[>0=][0][][$]|`, input list `[4 1 2 1 2]`** (linrec §README-3.1 desugar to If).
Stack shown bottom→top; the list quote sits on top at each descent. `xor` = `$`.

Descent (each level runs `P=>0=`: uncons leaves the head on the stack, `0=` yields `0` → recurse):

```
start                     : [4 1 2 1 2]
lvl0  P: > 0 =            : 4  [1 2 1 2]  0      -> If false, drop flag -> 4  [1 2 1 2] ; recurse
lvl1  P: > 0 =            : 4 1  [2 1 2]  0      -> recurse           -> 4 1  [2 1 2]
lvl2  P: > 0 =            : 4 1 2  [1 2]  0      -> recurse           -> 4 1 2  [1 2]
lvl3  P: > 0 =            : 4 1 2 1  [2]  0      -> recurse           -> 4 1 2 1  [2]
lvl4  P: > 0 =            : 4 1 2 1 2  []  0     -> recurse           -> 4 1 2 1 2  []
base  P: > 0 =            : uncons [] -> flag 0 ; 0 ; = -> 1 (true)  -> run T=0
      T: 0                : 4 1 2 1 2 0
```

Unwind (each returning level runs `R2 = $` = XOR of top two; R1 was empty so nothing else):

```
after base                : 4 1 2 1 2 0
lvl4  R2: $  (2 ^ 0 = 2)   : 4 1 2 1 2
lvl3  R2: $  (2 ^ 1 = 3)   : 4 1 2 3          [top=2, second=1]
lvl2  R2: $  (3 ^ 2 = 1)   : 4 1 1
lvl1  R2: $  (1 ^ 1 = 0)   : 4 0
lvl0  R2: $  (0 ^ 4 = 4)   : 4
HALT                       : 4
```

Result `[4]`. Independent check: XOR is commutative/associative, so the fold computes
`4 ^ 1 ^ 2 ^ 1 ^ 2 ^ 0 = 4 ^ (1^1) ^ (2^2) ^ 0 = 4 ^ 0 ^ 0 ^ 0 = 4`. ✓ **hand-traced** (n=5,
the unique element 4 appears once; 1 and 2 each appear twice and cancel).
