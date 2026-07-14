# Round 2 — read-tax battery ground truth

- **model_under_test:** claude-opus-4-8
- **interpreter (mtlrun):** `/workspace/target/debug/mtlrun` (input literal prepended, empty starting stack)
- Every MTL program was interpreter-validated through mtlrun and every Python twin was run under python3; all programs are interpreter-validated.

## Comprehension

# round2 comprehension — ground truth audit trail

Interpreter: `/workspace/target/debug/mtlrun` (input literal prepended, empty starting stack).
Every MTL program was run through mtlrun; every Python twin was run under python3 and asserted equal to the MTL integer.

## horner_sq  (tier A, 28 glyphs, sim_depth 4)
why hard: fold updates acc=acc*2+w*w via a dip under the accumulator, then a triangular primrec tail

MTL program (input already prepended):
```
[1 2 0 2 1 3 1]0[[2*]':*+]([0][+]&
```
mtlrun output line: `HALT: 30628`

Python twin:
```python
def solve(xs):
    acc = 0
    for w in xs:
        acc = acc * 2 + w * w
    return acc * (acc + 1) // 2

>>> solve([1, 2, 0, 2, 1, 3, 1])
30628
```
confirmed expected_int = **30628**  (mtl == python)

## max_fold  (tier A, 28 glyphs, sim_depth 4)
why hard: over/over/lt max idiom buried in the combine quote, then a square

MTL program (input already prepended):
```
[7 2 9 4 5 1 8 3 6]0[^^<[~_][_]?](:*
```
mtlrun output line: `HALT: 81`

Python twin:
```python
def solve(xs):
    best = 0
    for w in xs:
        if best < w:
            best = w
    return best * best

>>> solve([7, 2, 9, 4, 5, 1, 8, 3, 6])
81
```
confirmed expected_int = **81**  (mtl == python)

## xor_scan  (tier A, 27 glyphs, sim_depth 4)
why hard: xor scan with a per-element 2w+1 transform, then a trailing xor and multiply

MTL program (input already prepended):
```
[9 5 2 6 3 8 4 1 7 2]0[2*1+$](137$4*
```
mtlrun output line: `HALT: 572`

Python twin:
```python
def solve(xs):
    acc = 0
    for w in xs:
        acc ^= 2 * w + 1
    acc ^= 137
    return acc * 4

>>> solve([9, 5, 2, 6, 3, 8, 4, 1, 7, 2])
572
```
confirmed expected_int = **572**  (mtl == python)

## cube_fold  (tier A, 27 glyphs, sim_depth 4)
why hard: cube-sum fold (::**+) feeding a triangular primrec then a double

MTL program (input already prepended):
```
[2 3 1 4 2 3 5]0[::**+]([0][+]&2*
```
mtlrun output line: `HALT: 67860`

Python twin:
```python
def solve(xs):
    acc = 0
    for w in xs:
        acc += w ** 3
    acc = acc * (acc + 1) // 2
    return acc * 2

>>> solve([2, 3, 1, 4, 2, 3, 5])
67860
```
confirmed expected_int = **67860**  (mtl == python)

## factsum_sumsq  (tier B, 37 glyphs, sim_depth 6)
why hard: factorial primrec nested inside a fold, its sum driving a sum-of-squares primrec

MTL program (input already prepended):
```
[3 4 5 2 4 3 2 3]0[[1][*]&+]([0][~:*+]&1000-
```
mtlrun output line: `HALT: 2303415`

Python twin:
```python
from math import factorial

def solve(xs):
    total = 0
    for w in xs:
        total += factorial(w)
    squares = total * (total + 1) * (2 * total + 1) // 6
    return squares - 1000

>>> solve([3, 4, 5, 2, 4, 3, 2, 3])
2303415
```
confirmed expected_int = **2303415**  (mtl == python)

## prod_fold  (tier B, 42 glyphs, sim_depth 6)
why hard: fold of fold: an inner product-fold per row summed by the outer fold, then xor/sub/add

MTL program (input already prepended):
```
[[2 3][4 1][5 2 2][3 3]]0[1[*](+](137$1000-500+
```
mtlrun output line: `HALT: -326`

Python twin:
```python
def solve(rows):
    acc = 0
    for row in rows:
        product = 1
        for w in row:
            product *= w
        acc += product
    acc ^= 137
    return acc - 1000 + 500

>>> solve([[2, 3], [4, 1], [5, 2, 2], [3, 3]])
-326
```
confirmed expected_int = **-326**  (mtl == python)

## tri_fold  (tier B, 39 glyphs, sim_depth 6)
why hard: triangular primrec nested in a fold, squared, then a sub/mod/xor/add chain

MTL program (input already prepended):
```
[4 3 5 2 6 3 4]0[[0][+]&+](:*1000-29%137$500+
```
mtlrun output line: `HALT: 631`

Python twin:
```python
def solve(xs):
    acc = 0
    for w in xs:
        acc += w * (w + 1) // 2
    acc = acc * acc
    acc -= 1000
    acc %= 29
    acc ^= 137
    return acc + 500

>>> solve([4, 3, 5, 2, 6, 3, 4])
631
```
confirmed expected_int = **631**  (mtl == python)

## fact_linrec  (tier B, 36 glyphs, sim_depth 6)
why hard: linrec factorial with a NON-empty R2 (multiply on the way up), then a sum-of-squares primrec and xor

MTL program (input already prepended):
```
6[:0=][_1][:1-][*]|47-[0][~:*+]&137$
```
mtlrun output line: `HALT: 101833528`

Python twin:
```python
def solve(n):
    fact = 1
    while n > 0:
        fact *= n
        n -= 1
    fact -= 47
    squares = fact * (fact + 1) * (2 * fact + 1) // 6
    return squares ^ 137

>>> solve(6)
101833528
```
confirmed expected_int = **101833528**  (mtl == python)

## fold_of_fold  (tier C, 51 glyphs, sim_depth 8)
why hard: fold-of-fold over nested quotes (sum of row sums) plus a four-op arithmetic tail

MTL program (input already prepended):
```
[[1 2 3][4 5][6 7 8 9][2 2][5 5 5]]0[0[+](+](:*137$1000-500+
```
mtlrun output line: `HALT: 3733`

Python twin:
```python
def solve(rows):
    acc = 0
    for row in rows:
        acc += sum(row)
    acc = acc * acc
    acc ^= 137
    acc -= 1000
    return acc + 500

>>> solve([[1, 2, 3], [4, 5], [6, 7, 8, 9], [2, 2], [5, 5, 5]])
3733
```
confirmed expected_int = **3733**  (mtl == python)

## sumsq_linrec  (tier C, 52 glyphs, sim_depth 8)
why hard: linrec over a list with a non-trivial R2 that squares-and-adds each head, then triangular + xor

MTL program (input already prepended):
```
[3 1 4 2 5][>[;0][[]1]?][_0][>_[:*]'][+]|[0][+]&500+137$
```
mtlrun output line: `HALT: 1905`

Python twin:
```python
def solve(xs):
    def ssq(lst):
        if not lst:
            return 0
        head, *tail = lst
        return head * head + ssq(tail)
    acc = ssq(xs)
    acc = acc * (acc + 1) // 2
    acc += 500
    return acc ^ 137

>>> solve([3, 1, 4, 2, 5])
1905
```
confirmed expected_int = **1905**  (mtl == python)

## uncons_unpack  (tier C, 50 glyphs, sim_depth 8)
why hard: nine-deep uncons/drop unpack, then an eight-op stack reduction and a truncated-mod tail (negative dividend)

MTL program (input already prepended):
```
[9 2 5 3 1 6 4 8 2]>_>_>_>_>_>_>_>_>_ _+*-+*-+*137$1000-29%
```
mtlrun output line: `HALT: -7`

Python twin:
```python
def solve(xs):
    a, b, c, d, e, f, g, h, i = xs
    x = h + i
    x = g * x
    x = f - x
    x = e + x
    x = d * x
    x = c - x
    x = b + x
    x = a * x
    x ^= 137
    x -= 1000
    # truncated remainder: sign follows the dividend (C-style %)
    r = abs(x) % 29
    return -r if x < 0 else r

>>> solve([9, 2, 5, 3, 1, 6, 4, 8, 2])
-7
```
confirmed expected_int = **-7**  (mtl == python)

## sumsq_fold  (tier C, 51 glyphs, sim_depth 8)
why hard: sum-of-squares primrec nested in a fold, then a triangular primrec and a five-op tail

MTL program (input already prepended):
```
[3 2 4 1 3 2 3 2]0[[0][~:*+]&+]([0][+]&137$1000-29%500+13*
```
mtlrun output line: `HALT: 6773`

Python twin:
```python
def solve(xs):
    def ssq(n):
        return sum(k * k for k in range(1, n + 1))
    acc = 0
    for w in xs:
        acc += ssq(w)
    acc = acc * (acc + 1) // 2
    acc ^= 137
    acc -= 1000
    acc %= 29
    return (acc + 500) * 13

>>> solve([3, 2, 4, 1, 3, 2, 3, 2])
6773
```
confirmed expected_int = **6773**  (mtl == python)


### Tier D (escalation)

Eight items that push mental simulation past tier C: triple nesting and long interleaved combinator chains, 60-100 glyphs, sim_depth 11-13. Every program halts with one integer, mtlrun-verified, and its Python twin returns == that integer under python3.


#### d1  (tier D, 61 glyphs, sim_depth 11)
why hard: fold-of-fold-of-fold: sum of squares across three matrices' rows, then a triangular primrec and xor/sub tail

MTL program (input already prepended):
```
[[[1 2][3 4]][[5][6 7]][[2 2 2]]]0[0[0[:*+](+](+]([0][+]&137$1000-
```
mtlrun output line: `HALT: 10749`

Python twin:
```python
def solve(mats):
    acc = 0
    for mat in mats:
        for row in mat:
            for w in row:
                acc += w * w
    acc = acc * (acc + 1) // 2
    acc ^= 137
    return acc - 1000

>>> solve([[[1, 2], [3, 4]], [[5], [6, 7]], [[2, 2, 2]]])
10749
```
confirmed expected_int = **10749**  (mtl == python)

#### d2  (tier D, 64 glyphs, sim_depth 12)
why hard: linrec over a list whose R1 runs a triangular primrec per head, feeding a sum-of-squares primrec and xor

MTL program (input already prepended):
```
[3 5 2 4][>[;0][[]1]?][_0][>_[[0][+]&]'][+]|[0][~:*+]&137$1000-500+
```
mtlrun output line: `HALT: 13320`

Python twin:
```python
def solve(xs):
    def tri(n):
        r = 0
        for k in range(1, n + 1):
            r += k
        return r
    acc = 0
    for h in xs:
        acc += tri(h)
    s = 0
    for k in range(1, acc + 1):
        s += k * k
    s ^= 137
    return s - 1000 + 500

>>> solve([3, 5, 2, 4])
13320
```
confirmed expected_int = **13320**  (mtl == python)

#### d3  (tier D, 65 glyphs, sim_depth 12)
why hard: primrec-in-fold-in-linrec: per row a fold triangular-primrecs each element; linrec sums the rows; then triangular+xor

MTL program (input already prepended):
```
[[3 2][4][1 3 2]][>[;0][[]1]?][_0][>_[0[[0][+]&+](]'][+]|[0][+]&137$
```
mtlrun output line: `HALT: 314`

Python twin:
```python
def solve(rows):
    def tri(n):
        return n * (n + 1) // 2 if n > 0 else 0
    acc = 0
    for row in rows:
        rowacc = 0
        for w in row:
            rowacc += tri(w)
        acc += rowacc
    acc = tri(acc)
    return acc ^ 137

>>> solve([[3, 2], [4], [1, 3, 2]])
314
```
confirmed expected_int = **314**  (mtl == python)

#### d4  (tier D, 60 glyphs, sim_depth 11)
why hard: fold-of-fold-of-fold: product of row-sums per matrix summed across matrices, then triangular, xor and truncated mod

MTL program (input already prepended):
```
[[[1 2][3]][[2 2][1 1 1]]]0[1[0[+](*](+]([0][+]&137$1000-31%500+
```
mtlrun output line: `HALT: 478`

Python twin:
```python
def solve(mats):
    acc = 0
    for mat in mats:
        product = 1
        for row in mat:
            product *= sum(row)
        acc += product
    acc = acc * (acc + 1) // 2
    acc ^= 137
    acc -= 1000
    # truncated remainder: sign follows the dividend (C-style %)
    r = abs(acc) % 31
    acc = -r if acc < 0 else r
    return acc + 500

>>> solve([[[1, 2], [3]], [[2, 2], [1, 1, 1]]])
478
```
confirmed expected_int = **478**  (mtl == python)

#### d5  (tier D, 60 glyphs, sim_depth 13)
why hard: fold-in-primrec-in-fold: each n drives a primrec that folds a weight list; then triangular, square, xor, truncated-mod chain

MTL program (input already prepended):
```
[4 3 5 2]0[[0][~[2 3 1]0[+](*+]&+]([0][+]&:*137$1000-97%500+13*7-
```
mtlrun output line: `HALT: 6857`

Python twin:
```python
def solve(ns):
    weights = [2, 3, 1]
    s = sum(weights)
    acc = 0
    for n in ns:
        prim = 0
        for k in range(1, n + 1):
            prim += k * s
        acc += prim
    acc = acc * (acc + 1) // 2
    acc = acc * acc
    acc ^= 137
    acc -= 1000
    # truncated remainder: sign follows the dividend (C-style %)
    r = abs(acc) % 97
    acc = -r if acc < 0 else r
    acc += 500
    acc *= 13
    return acc - 7

>>> solve([4, 3, 5, 2])
6857
```
confirmed expected_int = **6857**  (mtl == python)

#### d6  (tier D, 62 glyphs, sim_depth 11)
why hard: linrec over a list of rows with non-trivial R2; R1 folds each row's sum-of-squares; then triangular and xor

MTL program (input already prepended):
```
[[1 2 3][4 5][6 7 8]][>[;0][[]1]?][_0][>_[0[:*+](]'][+]|[0][+]&137$
```
mtlrun output line: `HALT: 20775`

Python twin:
```python
def solve(rows):
    acc = 0
    for row in rows:
        s = 0
        for w in row:
            s += w * w
        acc += s
    acc = acc * (acc + 1) // 2
    return acc ^ 137

>>> solve([[1, 2, 3], [4, 5], [6, 7, 8]])
20775
```
confirmed expected_int = **20775**  (mtl == python)

#### d7  (tier D, 63 glyphs, sim_depth 12)
why hard: eight-deep uncons then three interleaved rot/dip/swap blocks tracking up to eight stack values, xor and truncated mod

MTL program (input already prepended):
```
[8 3 5 2 4 6 7 1]>_>_>_>_>_>_>_>_ _@[+]'*@[+]'*@[*]'+~-137$1000-29%500+
```
mtlrun output line: `HALT: 498`

Python twin:
```python
def solve(xs):
    a, b, c, d, e, f, g, h = xs
    x = (g + h) * f
    x = (e + x) * d
    x = c * x + b
    x = x - a
    x ^= 137
    x -= 1000
    # truncated remainder: sign follows the dividend (C-style %)
    r = abs(x) % 29
    x = -r if x < 0 else r
    return x + 500

>>> solve([8, 3, 5, 2, 4, 6, 7, 1])
498
```
confirmed expected_int = **498**  (mtl == python)

#### d8  (tier D, 62 glyphs, sim_depth 13)
why hard: twelve-deep uncons feeding an eleven-op alternating add/mul/sub reduction, xor and truncated mod (negative dividend)

MTL program (input already prepended):
```
[9 2 5 3 1 6 4 8 2 7 3 5]>_>_>_>_>_>_>_>_>_>_>_>_ _+*-+*-+*-+*137$1000-41%
```
mtlrun output line: `HALT: -28`

Python twin:
```python
def solve(xs):
    a, b, c, d, e, f, g, h, i, j, k, l = xs
    x = k + l
    x = j * x
    x = i - x
    x = h + x
    x = g * x
    x = f - x
    x = e + x
    x = d * x
    x = c - x
    x = b + x
    x = a * x
    x ^= 137
    x -= 1000
    # truncated remainder: sign follows the dividend (C-style %)
    r = abs(x) % 41
    return -r if x < 0 else r

>>> solve([9, 2, 5, 3, 1, 6, 4, 8, 2, 7, 3, 5])
-28
```
confirmed expected_int = **-28**  (mtl == python)

## Recall

# readtax round-2 recall — ground truth

HARDER verbatim-recall items. Each shows a dense program, ~200 words of neutral distractor prose, then asks for exact reproduction. The distractor embeds 1-3 near-duplicate DECOYS (single small edits of the target) framed as superseded draft transcriptions, to test whether the model recalls the EXACT original without blending.

MTL targets verified with `printf '%s' PROG | /workspace/target/debug/mtlrun`. Parsing confirmed for all (no PARSE ERROR). Python twins confirmed to compile.


## arith1  (Tier A)

**MTL target** (`31` glyphs): `3:*2*7+5%9+:*4-2*1+3+8*2-6+5*2+`
- mtlrun outcome: `HALT: 6342`
- decoys:
  - `3:*2*7+5%9+:*4-2*1+3+8*2-6+5*2*`  — final glyph + -> * (add becomes mul)

**Python twin** (`52` chars):

```python
def solve(n): return (n*n*2+7)%5*9-4+2*1-3+8*2-6+5*2
```
- decoys:
  - final * -> + before the last 2
    ```python
    def solve(n): return (n*n*2+7)%5*9-4+2*1-3+8*2-6+5+2
    ```

## arith2  (Tier A)

**MTL target** (`33` glyphs): `5:*3+2*7-:*4%9+8*2-1+6*3+7%2*5+3*`
- mtlrun outcome: `HALT: 21`
- decoys:
  - `5:*3+2*7-:*4*9+8*2-1+6*3+7%2*5+3*`  — the % after 4 -> * (mod becomes mul)

**Python twin** (`54` chars):

```python
def solve(n): return (n*n+3)*2-7%4+9+8*2-1+6*3+7%2*5+3
```
- decoys:
  - % -> * before the first 4
    ```python
    def solve(n): return (n*n+3)*2-7*4+9+8*2-1+6*3+7%2*5+3
    ```

## times1  (Tier A)

**MTL target** (`32` glyphs): `2 8[3*7+2%5+].9-:*4%1+6*3+2*7-5%`
- mtlrun outcome: `HALT: 3`
- decoys:
  - `2 8[3*7+2*5+].9-:*4%1+6*3+2*7-5%`  — % -> * inside the times-loop body

**Python twin** (`97` chars):

```python
def solve(n):
    x = 8
    for _ in range(n): x = (x*3+7) % 2 + 5
    return (x-9) % 4 + 1 + 6*3
```
- decoys:
  - + -> - inside x*3+7 in the loop
    ```python
    def solve(n):
        x = 8
        for _ in range(n): x = (x*3-7) % 2 + 5
        return (x-9) % 4 + 1 + 6*3
    ```

## primrec1  (Tier A)

**MTL target** (`31` glyphs): `4[1][*]&7+:*3%9+2*5-:2/1+6*3+2*`
- mtlrun outcome: `HALT: 15 102`
- decoys:
  - `4[1][*]&7+:*3%9+2*5-:2*1+6*3+2*`  — the / after :2 -> * (div becomes mul)

**Python twin** (`97` chars):

```python
def solve(n):
    r = 1
    for k in range(1, n+1): r *= k
    return (r+7) % 3 + 9 + 2*5 - 1 + 6
```
- decoys:
  - *= -> += in the accumulator
    ```python
    def solve(n):
        r = 1
        for k in range(1, n+1): r += k
        return (r+7) % 3 + 9 + 2*5 - 1 + 6
    ```

## pipe1  (Tier B)

**MTL target** (`51` glyphs): `7:*2%3+9*5-:*8%1+4*6-2+:*3%7+2*5+9-:*4%1+6*3-8+2*7%`
- mtlrun outcome: `HALT: 1`
- decoys:
  - `7*:2%3+9*5-:*8%1+4*6-2+:*3%7+2*5+9-:*4%1+6*3-8+2*7%`  — swapped adjacent pair: opening :* transposed to *:
  - `7:*2%3+9*5-:*8*1+4*6-2+:*3%7+2*5+9-:*4%1+6*3-8+2*7%`  — single glyph: the % after 8 -> *

**Python twin** (`74` chars):

```python
def solve(n): return (n*n*2)%3+9*5-8%1+4*6-2+(n*n)%3*7+2*5+9-4%1+6*3-8+2*7
```
- decoys:
  - transposed operands: 4*6 -> 6*4
    ```python
    def solve(n): return (n*n*2)%3+9*5-8%1+6*4-2+(n*n)%3*7+2*5+9-4%1+6*3-8+2*7
    ```
  - single char: % -> * before the 1
    ```python
    def solve(n): return (n*n*2)%3+9*5-8*1+4*6-2+(n*n)%3*7+2*5+9-4%1+6*3-8+2*7
    ```

## pipe2  (Tier B)

**MTL target** (`53` glyphs): `3:*2*7+5%9+:*4-2*1+3+8*2-6+5*2+9-:*4%1+6*3-8+2*7%5+3*`
- mtlrun outcome: `HALT: 33`
- decoys:
  - `3:*2*7+5%9+*:4-2*1+3+8*2-6+5*2+9-:*4%1+6*3-8+2*7%5+3*`  — swapped adjacent pair: :* transposed to *: before the 4-
  - `3:*2*7+5*9+:*4-2*1+3+8*2-6+5*2+9-:*4%1+6*3-8+2*7%5+3*`  — single glyph: the % after the first 5 -> *

**Python twin** (`72` chars):

```python
def solve(n): return (n*n*2+7)%5+9-4*2+1-3+8*2-6+5*2+9-4%1+6*3-8+2*7%5+3
```
- decoys:
  - transposed operands: 4*2 -> 2*4
    ```python
    def solve(n): return (n*n*2+7)%5+9-2*4+1-3+8*2-6+5*2+9-4%1+6*3-8+2*7%5+3
    ```
  - single char: % -> * before the final 5
    ```python
    def solve(n): return (n*n*2+7)%5+9-4*2+1-3+8*2-6+5*2+9-4%1+6*3-8+2*7*5+3
    ```

## loop1  (Tier B)

**MTL target** (`48` glyphs): `2 8[3*7+2%5+].9-:*4%1+6*3+2*7-5%9+:*4%1+6*3-8+2*`
- mtlrun outcome: `HALT: 22`
- decoys:
  - `2 8[3*7+2%5+].9-*:4%1+6*3+2*7-5%9+:*4%1+6*3-8+2*`  — swapped adjacent pair: :* transposed to *: after 9-
  - `2 8[3*7+2*5+].9-:*4%1+6*3+2*7-5%9+:*4%1+6*3-8+2*`  — single glyph: % -> * inside the loop body

**Python twin** (`111` chars):

```python
def solve(n):
    x = 8
    for _ in range(n): x = (x*3+7) % 2 + 5
    return (x-9) % 4 + 1 + 6*3+2 - 7 % 5 + 9
```
- decoys:
  - transposed operands: 6*3 -> 3*6
    ```python
    def solve(n):
        x = 8
        for _ in range(n): x = (x*3+7) % 2 + 5
        return (x-9) % 4 + 1 + 3*6+2 - 7 % 5 + 9
    ```
  - single char: + -> * inside x*3+7
    ```python
    def solve(n):
        x = 8
        for _ in range(n): x = (x*3*7) % 2 + 5
        return (x-9) % 4 + 1 + 6*3+2 - 7 % 5 + 9
    ```

## loop2  (Tier B)

**MTL target** (`52` glyphs): `5 6[2*3+7%1+].8-:*4%9+2*5-:*3%7+1+6*3-8+2*9-:*4%1+6*`
- mtlrun outcome: `HALT: 12`
- decoys:
  - `5 6[2*3+7%1+].8-*:4%9+2*5-:*3%7+1+6*3-8+2*9-:*4%1+6*`  — swapped adjacent pair: :* transposed to *: after 8-
  - `5 6[2*3+7*1+].8-:*4%9+2*5-:*3%7+1+6*3-8+2*9-:*4%1+6*`  — single glyph: % -> * inside the loop body

**Python twin** (`119` chars):

```python
def solve(n):
    x = 6
    for _ in range(n): x = (x*2+3) % 7 + 1
    return (x-8) % 4 + 9 + 2*5 - 3 % 7 + 1 + 6*3 - 8
```
- decoys:
  - transposed operands: 2*5 -> 5*2
    ```python
    def solve(n):
        x = 6
        for _ in range(n): x = (x*2+3) % 7 + 1
        return (x-8) % 4 + 9 + 5*2 - 3 % 7 + 1 + 6*3 - 8
    ```
  - single char: + -> * inside x*2+3
    ```python
    def solve(n):
        x = 6
        for _ in range(n): x = (x*2*3) % 7 + 1
        return (x-8) % 4 + 9 + 2*5 - 3 % 7 + 1 + 6*3 - 8
    ```

## deep1  (Tier C)

**MTL target** (`62` glyphs): `3:*2*7+5%9+13*4-2*1+3+8*2-6+5*2+9-:*47%1+6*3-8+2*7%5+3*2*9-11%`
- mtlrun outcome: `HALT: 2`
- decoys:
  - `3:*2*7+5%9+13*4-2*1+3+8*2-6+5*2+9-*:47%1+6*3-8+2*7%5+3*2*9-11%`  — swapped adjacent pair: :* transposed to *: before 47
  - `3:*2*7+5*9+13*4-2*1+3+8*2-6+5*2+9-:*47%1+6*3-8+2*7%5+3*2*9-11%`  — single glyph: the % after the first 5 -> *
  - `3:*2*7+5%9+13*4-2*1+3+8*2-6+5*2+9-:*41%1+6*3-8+2*7%5+3*2*9-11%`  — VERY CLOSE: deep literal 47 -> 41 (one digit)

**Python twin** (`87` chars):

```python
def solve(n): return (n*n*2+7)%5+13*4-2*1+3+8*2-6+5*2+9-(n*n)%47+1+6*3-8+2*7%5+3*2*9-11
```
- decoys:
  - transposed operands: 13*4 -> 4*13
    ```python
    def solve(n): return (n*n*2+7)%5+4*13-2*1+3+8*2-6+5*2+9-(n*n)%47+1+6*3-8+2*7%5+3*2*9-11
    ```
  - single char: % -> * after the parenthesis
    ```python
    def solve(n): return (n*n*2+7)*5+13*4-2*1+3+8*2-6+5*2+9-(n*n)%47+1+6*3-8+2*7%5+3*2*9-11
    ```
  - VERY CLOSE: deep literal 47 -> 41
    ```python
    def solve(n): return (n*n*2+7)%5+13*4-2*1+3+8*2-6+5*2+9-(n*n)%41+1+6*3-8+2*7%5+3*2*9-11
    ```

## deep2  (Tier C)

**MTL target** (`64` glyphs): `7:*2%13+9*5-:*8%1+4*6-2+:*3%17+2*5+9-:*4%1+6*3-8+2*7%5+3*2%9+11*`
- mtlrun outcome: `HALT: 99`
- decoys:
  - `7:*2%13+9*5-*:8%1+4*6-2+:*3%17+2*5+9-:*4%1+6*3-8+2*7%5+3*2%9+11*`  — swapped adjacent pair: :* transposed to *: after 5-
  - `7:*2*13+9*5-:*8%1+4*6-2+:*3%17+2*5+9-:*4%1+6*3-8+2*7%5+3*2%9+11*`  — single glyph: the % after the first 2 -> *
  - `7:*2%13+9*5-:*8%1+4*6-2+:*3%13+2*5+9-:*4%1+6*3-8+2*7%5+3*2%9+11*`  — VERY CLOSE: deep literal 17 -> 13 (one digit)

**Python twin** (`93` chars):

```python
def solve(n): return (n*n)%2+13+9*5-(n*n)%8+1+4*6-2+(n*n)%3+17+2*5+9-4%1+6*3-8+2*7%5+3*2%9+11
```
- decoys:
  - transposed operands: 9*5 -> 5*9
    ```python
    def solve(n): return (n*n)%2+13+5*9-(n*n)%8+1+4*6-2+(n*n)%3+17+2*5+9-4%1+6*3-8+2*7%5+3*2%9+11
    ```
  - single char: % -> * before the 8
    ```python
    def solve(n): return (n*n)%2+13+9*5-(n*n)*8+1+4*6-2+(n*n)%3+17+2*5+9-4%1+6*3-8+2*7%5+3*2%9+11
    ```
  - VERY CLOSE: deep literal 17 -> 13
    ```python
    def solve(n): return (n*n)%2+13+9*5-(n*n)%8+1+4*6-2+(n*n)%3+13+2*5+9-4%1+6*3-8+2*7%5+3*2%9+11
    ```

## linrec1  (Tier C)

**MTL target** (`62` glyphs): `[>[;0][[]1]?][_0][>_:2*3+7%5+][~;4*2-9+6%]|3+5*2-7%9+:*4%1+6*3`
- mtlrun outcome: `FAULT: Underflow (run on empty stack; parses cleanly; a linrec recursion skeleton meant to be applied to a list argument)`
- decoys:
  - `[>[;0][[]1]?][_0][>_:2*3+7%5+][;~4*2-9+6%]|3+5*2-7%9+:*4%1+6*3`  — swapped adjacent pair: ~; transposed to ;~ in the after-recurse quote
  - `[>[;0][[]1]?][_0][>_:2*3+7%5+][~;4*2-9+6%]|3+5*2-7%9+:*4*1+6*3`  — single glyph: % -> * in the trailing pipeline
  - `[>[;0][[]1]?][_0][>_:2*3+9%5+][~;4*2-9+6%]|3+5*2-7%9+:*4%1+6*3`  — VERY CLOSE: deep digit 7 -> 9 inside the pre-recurse quote

**Python twin** (`107` chars):

```python
def solve(xs):
    if not xs: return 0
    h, *t = xs
    return (h*2+3) % 7 + 5 + solve(t) * 4 - 2 + 9 % 6
```
- decoys:
  - transposed operands: h*2 -> 2*h
    ```python
    def solve(xs):
        if not xs: return 0
        h, *t = xs
        return (2*h+3) % 7 + 5 + solve(t) * 4 - 2 + 9 % 6
    ```
  - single char: % -> * after the parenthesis
    ```python
    def solve(xs):
        if not xs: return 0
        h, *t = xs
        return (h*2+3) * 7 + 5 + solve(t) * 4 - 2 + 9 % 6
    ```
  - VERY CLOSE: deep digit 3 -> 9 inside the parenthesis
    ```python
    def solve(xs):
        if not xs: return 0
        h, *t = xs
        return (h*2+9) % 7 + 5 + solve(t) * 4 - 2 + 9 % 6
    ```

## deep3  (Tier C)

**MTL target** (`62` glyphs): `9:*2%13+7*5-:*8%1+4*6-2+:*3%17+2*5+9-:*4%1+6*3-8+2*7%5+3*2%11+`
- mtlrun outcome: `HALT: 11`
- decoys:
  - `9:*2%13+7*5-*:8%1+4*6-2+:*3%17+2*5+9-:*4%1+6*3-8+2*7%5+3*2%11+`  — swapped adjacent pair: :* transposed to *: after 5-
  - `9:*2*13+7*5-:*8%1+4*6-2+:*3%17+2*5+9-:*4%1+6*3-8+2*7%5+3*2%11+`  — single glyph: the % after the first 2 -> *
  - `9:*2%13+7*5-:*8%1+4*6-2+:*3%14+2*5+9-:*4%1+6*3-8+2*7%5+3*2%11+`  — VERY CLOSE: deep literal 17 -> 14 (one digit)

**Python twin** (`91` chars):

```python
def solve(n): return (n*n)%2+13+7*5-(n*n)%8+1+4*6-2+(n*n)%3+17+2*5+9-4%1+6*3-8+2*7%5+3*2%11
```
- decoys:
  - transposed operands: 7*5 -> 5*7
    ```python
    def solve(n): return (n*n)%2+13+5*7-(n*n)%8+1+4*6-2+(n*n)%3+17+2*5+9-4%1+6*3-8+2*7%5+3*2%11
    ```
  - single char: % -> * before the 8
    ```python
    def solve(n): return (n*n)%2+13+7*5-(n*n)*8+1+4*6-2+(n*n)%3+17+2*5+9-4%1+6*3-8+2*7%5+3*2%11
    ```
  - VERY CLOSE: deep literal 17 -> 14
    ```python
    def solve(n): return (n*n)%2+13+7*5-(n*n)%8+1+4*6-2+(n*n)%3+14+2*5+9-4%1+6*3-8+2*7%5+3*2%11
    ```

### Tier D (escalation)

Extreme-length dense programs (MTL 90-140 glyphs) with FOUR aggressive near-duplicate decoys apiece: a single-glyph change, an adjacent transpose, a deep multi-digit-literal flip, and a glyph changed inside a nested quote/paren -- every decoy within +/-2 chars of the target length. Presentation differs from tiers A-C: the program to reproduce is shown between `>>>BEGIN PROGRAM<<<` and `>>>END PROGRAM<<<` marker lines with NO leading indentation (fixing the round-2 4-space indentation artifact), and the instruction asks for the exact text between the markers. The four decoys are woven into the ~200-word neutral distractor as struck-through 'superseded drafts'.

## d1  (Tier D)

**MTL target** (`91` glyphs): `2 9[3*7+2%5+].8-:*4%13+6*3+2*7-5%9+:*4%1+6*3-8+2*7%5+3*2%11+7*4-:*6%1+3*8-2%9+5*2-:*7%1+4*3`
- mtlrun outcome: `HALT: 8 3`
- decoys:
  - `2 9[3*7+2%5+].8-:*4%13+6*3+2*7-5*9+:*4%1+6*3-8+2*7%5+3*2%11+7*4-:*6%1+3*8-2%9+5*2-:*7%1+4*3`  -- single-glyph (the % after -5 -> *)
  - `2 9[3*7+2%5+].8-*:4%13+6*3+2*7-5%9+:*4%1+6*3-8+2*7%5+3*2%11+7*4-:*6%1+3*8-2%9+5*2-:*7%1+4*3`  -- adjacent transpose (:* -> *: before 4%13)
  - `2 9[3*7+2%5+].8-:*4%17+6*3+2*7-5%9+:*4%1+6*3-8+2*7%5+3*2%11+7*4-:*6%1+3*8-2%9+5*2-:*7%1+4*3`  -- deep multi-digit-literal flip (13 -> 17 after :*4%)
  - `2 9[3*7+2*5+].8-:*4%13+6*3+2*7-5%9+:*4%1+6*3-8+2*7%5+3*2%11+7*4-:*6%1+3*8-2%9+5*2-:*7%1+4*3`  -- glyph inside nested quote ([3*7+2%5+] -> % becomes *)

**Python twin** (`93` chars):

```python
def solve(n): return ((n*n)*2+7)%5+13*4-2*1+3+8*2-6+5*2+9-(n*n)%11+6*3-8+2*7%5+3*2%9+11-4*6+7
```
- decoys:
  - single char (the % before the final 5 in 2*7%5 -> *)
    ```python
    def solve(n): return ((n*n)*2+7)%5+13*4-2*1+3+8*2-6+5*2+9-(n*n)%11+6*3-8+2*7*5+3*2%9+11-4*6+7
    ```
  - transposed operands (13*4 -> 4*13)
    ```python
    def solve(n): return ((n*n)*2+7)%5+4*13-2*1+3+8*2-6+5*2+9-(n*n)%11+6*3-8+2*7%5+3*2%9+11-4*6+7
    ```
  - deep literal flip (11 -> 17 after (n*n)%)
    ```python
    def solve(n): return ((n*n)*2+7)%5+13*4-2*1+3+8*2-6+5*2+9-(n*n)%17+6*3-8+2*7%5+3*2%9+11-4*6+7
    ```
  - glyph inside nested paren (((n*n)*2+7) -> +7 becomes +9)
    ```python
    def solve(n): return ((n*n)*2+9)%5+13*4-2*1+3+8*2-6+5*2+9-(n*n)%11+6*3-8+2*7%5+3*2%9+11-4*6+7
    ```

## d2  (Tier D)

**MTL target** (`93` glyphs): `4[1][*]&7+:*3%13+2*5-:2/1+6*3+2*7%5+9-:*4%1+6*3-8+2*7%5+3*2%9+11+:*4%1+6*3-8+2*9-:*5%1+7*3+2%`
- mtlrun outcome: `HALT: 23 0`
- decoys:
  - `4[1][*]&7+:*3%13+2*5-:2*1+6*3+2*7%5+9-:*4%1+6*3-8+2*7%5+3*2%9+11+:*4%1+6*3-8+2*9-:*5%1+7*3+2%`  -- single-glyph (the / after :2 -> *)
  - `4[1][*]&7+*:3%13+2*5-:2/1+6*3+2*7%5+9-:*4%1+6*3-8+2*7%5+3*2%9+11+:*4%1+6*3-8+2*9-:*5%1+7*3+2%`  -- adjacent transpose (:* -> *: before 3%13)
  - `4[1][*]&7+:*3%13+2*5-:2/1+6*3+2*7%5+9-:*4%1+6*3-8+2*7%5+3*2%9+17+:*4%1+6*3-8+2*9-:*5%1+7*3+2%`  -- deep multi-digit-literal flip (11 -> 17 after %9+)
  - `4[1][+]&7+:*3%13+2*5-:2/1+6*3+2*7%5+9-:*4%1+6*3-8+2*7%5+3*2%9+11+:*4%1+6*3-8+2*9-:*5%1+7*3+2%`  -- glyph inside nested quote ([*] -> [+])

**Python twin** (`92` chars):

```python
def solve(n): return ((n+3)*2-7)%4+13+2*5-9%1+6*3+2*7%5+9-4%1+6*3-8+2*7%5+3*2%9+11+4*6-2+7*3
```
- decoys:
  - single char (the % after -9 -> *)
    ```python
    def solve(n): return ((n+3)*2-7)%4+13+2*5-9*1+6*3+2*7%5+9-4%1+6*3-8+2*7%5+3*2%9+11+4*6-2+7*3
    ```
  - transposed operands (4*6 -> 6*4)
    ```python
    def solve(n): return ((n+3)*2-7)%4+13+2*5-9%1+6*3+2*7%5+9-4%1+6*3-8+2*7%5+3*2%9+11+6*4-2+7*3
    ```
  - deep literal flip (13 -> 17 after %4+)
    ```python
    def solve(n): return ((n+3)*2-7)%4+17+2*5-9%1+6*3+2*7%5+9-4%1+6*3-8+2*7%5+3*2%9+11+4*6-2+7*3
    ```
  - glyph inside nested paren (((n+3)*2-7) -> +3 becomes +7)
    ```python
    def solve(n): return ((n+7)*2-7)%4+13+2*5-9%1+6*3+2*7%5+9-4%1+6*3-8+2*7%5+3*2%9+11+4*6-2+7*3
    ```

## d3  (Tier D)

**MTL target** (`109` glyphs): `7:*2%13+9*5-:*8%1+4*6-2+3 5[2*7+3%4+].9-:*4%17+6*3-8+2*7%5+3*2%9+11*4-:*6%1+3*8-2+5*7-:*9%1+2*3-6+4%7+:*8%1+3`
- mtlrun outcome: `HALT: 4 1 3`
- decoys:
  - `7:*2%13+9*5-:*8*1+4*6-2+3 5[2*7+3%4+].9-:*4%17+6*3-8+2*7%5+3*2%9+11*4-:*6%1+3*8-2+5*7-:*9%1+2*3-6+4%7+:*8%1+3`  -- single-glyph (the % after :*8 -> *)
  - `7*:2%13+9*5-:*8%1+4*6-2+3 5[2*7+3%4+].9-:*4%17+6*3-8+2*7%5+3*2%9+11*4-:*6%1+3*8-2+5*7-:*9%1+2*3-6+4%7+:*8%1+3`  -- adjacent transpose (:* -> *: at head, before 2%13)
  - `7:*2%13+9*5-:*8%1+4*6-2+3 5[2*7+3%4+].9-:*4%13+6*3-8+2*7%5+3*2%9+11*4-:*6%1+3*8-2+5*7-:*9%1+2*3-6+4%7+:*8%1+3`  -- deep multi-digit-literal flip (17 -> 13 after :*4%)
  - `7:*2%13+9*5-:*8%1+4*6-2+3 5[2*7+3*4+].9-:*4%17+6*3-8+2*7%5+3*2%9+11*4-:*6%1+3*8-2+5*7-:*9%1+2*3-6+4%7+:*8%1+3`  -- glyph inside nested quote ([2*7+3%4+] -> % becomes *)

**Python twin** (`99` chars):

```python
def solve(n): return ((n*n)%2+13)+9*5-(n*n)%8+1+4*6-2+(n*n)%17+2*5+9-4%1+6*3-8+2*7%5+3*2%9+11-4*6+7
```
- decoys:
  - single char (the % before the 8 -> *)
    ```python
    def solve(n): return ((n*n)%2+13)+9*5-(n*n)*8+1+4*6-2+(n*n)%17+2*5+9-4%1+6*3-8+2*7%5+3*2%9+11-4*6+7
    ```
  - transposed operands (9*5 -> 5*9)
    ```python
    def solve(n): return ((n*n)%2+13)+5*9-(n*n)%8+1+4*6-2+(n*n)%17+2*5+9-4%1+6*3-8+2*7%5+3*2%9+11-4*6+7
    ```
  - deep literal flip (17 -> 13 after (n*n)%)
    ```python
    def solve(n): return ((n*n)%2+13)+9*5-(n*n)%8+1+4*6-2+(n*n)%13+2*5+9-4%1+6*3-8+2*7%5+3*2%9+11-4*6+7
    ```
  - glyph inside nested paren (((n*n)%2+13) -> %2 becomes *2)
    ```python
    def solve(n): return ((n*n)*2+13)+9*5-(n*n)%8+1+4*6-2+(n*n)%17+2*5+9-4%1+6*3-8+2*7%5+3*2%9+11-4*6+7
    ```

## Mutation

# round2 mutation-detection — ground truth (audit trail)

HARDER round-2 mutation items. 12 items x 2 arms (MTL + Python twin).
8 differ by exactly ONE glyph/token; 4 are byte-identical controls.
Every MTL program A and B was run through `/workspace/target/debug/mtlrun`
(empty starting stack). Every Python snippet was run under `python3`.
Glyph counts are non-whitespace characters of program A.

## Swap-kind coverage
Covered (8, one per differ item): `^`<->`~`, `/`<->`%`, `@`<->`^`, `'`<->`!`, `,`<->`;`, deep-literal digit, `:`<->`^`, `<`<->`=`.
NOT covered (only 8 differ slots for 10 requested kinds): `&`<->`|` (primrec/linrec) and `>` add/remove. Flagged as follow-ups.

## Summary table

| item | tier | glyphs | swap | differ | MTL A->B behavior | Python A->B |
|---|---|---|---|---|---|---|
| a1 | A | 27 | `^->~` | yes | HALT: 7 -> FAULT: Underflow | 21 -> -9 |
| a2 | A | 26 | `/->%` | yes | HALT: 8 -> HALT: 6 | 10 -> 15 |
| a3 | A | 27 | `none (control)` | NO | HALT: 2 (identical) | -1 (identical) |
| a4 | A | 26 | `none (control)` | NO | HALT: 8 (identical) | 24 (identical) |
| b1 | B | 37 | `@->^` | yes | HALT: 3 21 -> HALT: 8 3 12 | 21 -> 8 |
| b2 | B | 37 | `'->!` | yes | HALT: 6 20 -1 -> HALT: 6 4 1 | 5 -> 1 |
| b3 | B | 38 | `,->;` | yes | HALT: 2 6 3 -> FAULT: TypeMismatch | -2 -> 18 |
| b4 | B | 39 | `none (control)` | NO | HALT: 7 7 (identical) | 30 (identical) |
| c1 | C | 48 | `0->8 (digit in literal 1000->1080)` | yes | HALT: 317279 -> HALT: 340319 | 6039 -> 6519 |
| c2 | C | 49 | `:->^` | yes | HALT: 2 32 -> HALT: 2 38 | 18 -> 15 |
| c3 | C | 51 | `<->=` | yes | HALT: 29152 -> HALT: 31672 | 13 -> 25 |
| c4 | C | 53 | `none (control)` | NO | HALT: 21 (identical) | 8 (identical) |

## Per-item detail

### a1 (tier A, DIFFER)

MTL:
```
A: 9 2+3*1-4%5*6+2 4+^-+7*9-6%4+
B: 9 2+3*1-4%5*6+2 4+~-+7*9-6%4+
```
- glyphs (non-space, A): 27
- mtlrun A: HALT: 7
- mtlrun B: FAULT: Underflow
- swap: `^->~` — changed: ~ (was ^)
- position: the stack-op glyph immediately after '2 4+' (between the '+' and the following '-')
- why quiet: over(^) leaves an extra copy so the tail reduces to a single value; swap(~) leaves one fewer value, so a later '+' underflows. Halt->fault flip from a one-keystroke glyph.

Python twin:
```python
# A
def f(a, b, c):
    t = (a - b) * c
    return t + a - c
print(f(9, 4, 3))
# B
def f(a, b, c):
    t = (b - a) * c
    return t + a - c
print(f(9, 4, 3))
```
- python3 A: 21
- python3 B: -9
- mutation: `a<->b operands` — changed: b (was a)
- position: first operand of the subtraction: '(b - a)' was '(a - b)'
- why quiet: operands of the non-commutative subtraction swapped; both compile and run.

### a2 (tier A, DIFFER)

MTL:
```
A: 90 7*5+40%3*8-11/2+6*9-4%5+
B: 90 7*5+40%3*8-11%2+6*9-4%5+
```
- glyphs (non-space, A): 26
- mtlrun A: HALT: 8
- mtlrun B: HALT: 6
- swap: `/->%` — changed: % (was /)
- position: the binary operator right after the literal '11', mid-program
- why quiet: div and mod look almost identical and only diverge for this specific dividend/divisor pair; the rest of the chain preserves the small delta to the final value.

Python twin:
```python
# A
def h(n):
    total = 0
    for i in range(10):
        if i < n:
            total += i
    return total
print(h(5))
# B
def h(n):
    total = 0
    for i in range(10):
        if i <= n:
            total += i
    return total
print(h(5))
```
- python3 A: 10
- python3 B: 15
- mutation: `<-><=` — changed: <= (was <)
- position: the loop comparison 'if i <= n' was 'if i < n'
- why quiet: the boundary case i==n flips whether it is counted; one extra term.

### a3 (tier A, CONTROL)

MTL:
```
A: 4 7*2+9-3*5%8+6*1-2%7+3*4-9%
B: 4 7*2+9-3*5%8+6*1-2%7+3*4-9%
```
- glyphs (non-space, A): 27
- mtlrun A: HALT: 2
- mtlrun B: HALT: 2
- byte-identical control (A == B)
- why quiet: identical control: a dense mixed +-*/% chain with no combinators, so 'no diff' must be confirmed glyph by glyph.

Python twin:
```python
# A
def calc(x):
    return (x * 7 + 2) % 9 - x
print(calc(4))
# B
def calc(x):
    return (x * 7 + 2) % 9 - x
print(calc(4))
```
- python3 A: -1
- python3 B: -1
- byte-identical control (A == B)
- why quiet: identical control.

### a4 (tier A, CONTROL)

MTL:
```
A: 6 3+[2*1+]!4+5*9%7-2*8+3%6+
B: 6 3+[2*1+]!4+5*9%7-2*8+3%6+
```
- glyphs (non-space, A): 26
- mtlrun A: HALT: 8
- mtlrun B: HALT: 8
- byte-identical control (A == B)
- why quiet: identical control containing a quote+apply, so a reader may wrongly suspect the quote hides a change.

Python twin:
```python
# A
def apply_twice(x):
    def step(v):
        return v * 2 + 1
    return step(step(x)) - 3
print(apply_twice(6))
# B
def apply_twice(x):
    def step(v):
        return v * 2 + 1
    return step(step(x)) - 3
print(apply_twice(6))
```
- python3 A: 24
- python3 B: 24
- byte-identical control (A == B)
- why quiet: identical control with a nested helper function.

### b1 (tier B, DIFFER)

MTL:
```
A: 8 3 5[2+[@*]!7-]!4+9%6*2-3%1+7*2-9%4+3*
B: 8 3 5[2+[^*]!7-]!4+9%6*2-3%1+7*2-9%4+3*
```
- glyphs (non-space, A): 37
- mtlrun A: HALT: 3 21
- mtlrun B: HALT: 8 3 12
- swap: `@->^` — changed: ^ (was @)
- position: inside the inner quote: '[^*]' was '[@*]' (the stack-op just before the '*')
- why quiet: rot(@) consumes three values and reduces to one; over(^) copies instead of rotating, leaving an extra value that shifts the whole tail. Buried two quotes deep.

Python twin:
```python
# A
def f(a, b, c):
    d = a * b
    e = b + c
    return d - e + a * c
print(f(4, 5, 2))
# B
def f(a, b, c):
    d = a * b
    e = b + c
    return d - d + a * c
print(f(4, 5, 2))
```
- python3 A: 21
- python3 B: 8
- mutation: `rename e->d` — changed: d (was e)
- position: the subtrahend in the return: 'd - d' was 'd - e'
- why quiet: the single use of the variable 'e' is renamed to 'd', silently reusing the wrong binding.

### b2 (tier B, DIFFER)

MTL:
```
A: 6 4 2[3+[5*]'8-]!9%2*7+4-6*3+8%5*2-4+2%
B: 6 4 2[3+[5*]!8-]!9%2*7+4-6*3+8%5*2-4+2%
```
- glyphs (non-space, A): 37
- mtlrun A: HALT: 6 20 -1
- mtlrun B: HALT: 6 4 1
- swap: `'->!` — changed: ! (was ')
- position: the combinator right after the inner quote '[5*]', inside the outer quote
- why quiet: dip(') shields the value beneath the quote; apply(!) runs the quote directly on it. ' and ! differ by one keystroke and both are legal here, so no fault to tip the reader off.

Python twin:
```python
# A
def check(x, y):
    if x > 0 and y > 0:
        return x + y
    return x - y
print(check(3, -2))
# B
def check(x, y):
    if x > 0 or y > 0:
        return x + y
    return x - y
print(check(3, -2))
```
- python3 A: 5
- python3 B: 1
- mutation: `and->or` — changed: or (was and)
- position: the guard 'if x > 0 or y > 0' was 'if x > 0 and y > 0'
- why quiet: and/or differ by two letters and flip which branch runs for mixed-sign inputs.

### b3 (tier B, DIFFER)

MTL:
```
A: 2 6[[3 4+][5*],!8+]!5%3*7-4+9%2*6+3-4+8%
B: 2 6[[3 4+][5*];!8+]!5%3*7-4+9%2*6+3-4+8%
```
- glyphs (non-space, A): 38
- mtlrun A: HALT: 2 6 3
- mtlrun B: FAULT: TypeMismatch
- swap: `,->;` — changed: ; (was ,)
- position: between the two inner quotes '[3 4+]' and '[5*]', inside the outer quote
- why quiet: cat(,) concatenates the two quotes into one runnable body; cons(;) nests the first quote as an element, so applying it later multiplies a quote by an int and faults. Halt->fault from ',' vs ';'.

Python twin:
```python
# A
def g(a, b, c):
    return (a - b) * (b - c) + a
print(g(8, 3, 5))
# B
def g(a, b, c):
    return (b - a) * (b - c) + a
print(g(8, 3, 5))
```
- python3 A: -2
- python3 B: 18
- mutation: `a<->b operands` — changed: b (was a)
- position: first factor of the product: '(b - a) * (b - c)' was '(a - b) * (b - c)'
- why quiet: operands of one subtraction swapped, buried inside a compound expression.

### b4 (tier B, CONTROL)

MTL:
```
A: 7 2[4+[3*9%]!5-]!6*8%2+[1+]!7-3*9%4+2*5+
B: 7 2[4+[3*9%]!5-]!6*8%2+[1+]!7-3*9%4+2*5+
```
- glyphs (non-space, A): 39
- mtlrun A: HALT: 7 7
- mtlrun B: HALT: 7 7
- byte-identical control (A == B)
- why quiet: identical control with two levels of nested quotes and two apply sites, maximizing the chance of a false 'they differ'.

Python twin:
```python
# A
def transform(a, b):
    total = 0
    for k in range(a):
        total += (k * 3) % 9
    return total - b + a * 2
print(transform(7, 2))
# B
def transform(a, b):
    total = 0
    for k in range(a):
        total += (k * 3) % 9
    return total - b + a * 2
print(transform(7, 2))
```
- python3 A: 30
- python3 B: 30
- byte-identical control (A == B)
- why quiet: identical control with a loop and modulus.

### c1 (tier C, DIFFER)

MTL:
```
A: 7 3*2+5*4-1000+9-2*3+8-4+6*2-5+3*7-2+4*3-9+2*5-8+
B: 7 3*2+5*4-1080+9-2*3+8-4+6*2-5+3*7-2+4*3-9+2*5-8+
```
- glyphs (non-space, A): 48
- mtlrun A: HALT: 317279
- mtlrun B: HALT: 340319
- swap: `0->8 (digit in literal 1000->1080)` — changed: 8 (was 0)
- position: the third digit of the 4-digit literal: '1080' was '1000'
- why quiet: a single interior digit of a long literal changes; the additive/multiplicative tail (no mod) preserves the +80 offset to the final value, but the digit is easy to skim past.

Python twin:
```python
# A
def compute(x):
    base = 1000
    scale = base * x
    return scale + x * 7 - 3
print(compute(6))
# B
def compute(x):
    base = 1080
    scale = base * x
    return scale + x * 7 - 3
print(compute(6))
```
- python3 A: 6039
- python3 B: 6519
- mutation: `digit in literal 1000->1080` — changed: 8 (was 0)
- position: the interior digit of 'base = 1080' was 'base = 1000'
- why quiet: one interior digit of a 4-digit literal changed deep in the body.

### c2 (tier C, DIFFER)

MTL:
```
A: 6 3/9 6-12 4/[:*].5+2*9%4-3*7+6%2*3+5*2-7%4+8+3*6-2+
B: 6 3/9 6-12 4/[^*].5+2*9%4-3*7+6%2*3+5*2-7%4+8+3*6-2+
```
- glyphs (non-space, A): 49
- mtlrun A: HALT: 2 32
- mtlrun B: HALT: 2 38
- swap: `:->^` — changed: ^ (was :)
- position: inside the times-loop body quote: '[^*]' was '[:*]' (the stack-op before '*')
- why quiet: dup(:) squares the top each iteration; over(^) multiplies it by the constant beneath instead. Both keep the stack depth stable so the loop runs identically-shaped; only the numbers differ. Swap sits inside a loop body.

Python twin:
```python
# A
def process(a, b, c, d):
    p = a * b + c
    q = b * d - a
    return p - q + c - d
print(process(6, 3, 9, 6))
# B
def process(a, b, c, d):
    p = a * b + c
    q = b * d - a
    return p - q + a - d
print(process(6, 3, 9, 6))
```
- python3 A: 18
- python3 B: 15
- mutation: `rename c->a` — changed: a (was c)
- position: the third term of the return: 'p - q + a - d' was 'p - q + c - d'
- why quiet: the single use of parameter 'c' in the return is renamed to 'a'.

### c3 (tier C, DIFFER)

MTL:
```
A: 5 2*4+3 8<[6*2+][6*9+]?3-2*5+7-3*2+9-4*2-7+3*6-2+5*3-
B: 5 2*4+3 8=[6*2+][6*9+]?3-2*5+7-3*2+9-4*2-7+3*6-2+5*3-
```
- glyphs (non-space, A): 51
- mtlrun A: HALT: 29152
- mtlrun B: HALT: 31672
- swap: `<->=` — changed: = (was <)
- position: the comparison glyph right after '3 8', selecting the '?' branch
- why quiet: '<' (3<8 -> 1, true branch) vs '=' (3=8 -> 0, false branch) pick different quotes; the long additive tail carries the branch delta to the end. '<' and '=' are a single glyph apart and both yield a valid flag.

Python twin:
```python
# A
def score(n):
    total = 0
    for i in range(20):
        if i * 2 < n:
            total += i * 3
    return total - 5
print(score(8))
# B
def score(n):
    total = 0
    for i in range(20):
        if i * 2 <= n:
            total += i * 3
    return total - 5
print(score(8))
```
- python3 A: 13
- python3 B: 25
- mutation: `<-><=` — changed: <= (was <)
- position: the loop test 'if i * 2 <= n' was 'if i * 2 < n'
- why quiet: the boundary i*2==n case flips inclusion, adding one term deep in the loop.

### c4 (tier C, CONTROL)

MTL:
```
A: 7[0][+]&3*[2*1+]!9%4+6*2-8%5+3*7-2+4%9+2*6-3+2*7%4+3*
B: 7[0][+]&3*[2*1+]!9%4+6*2-8%5+3*7-2+4%9+2*6-3+2*7%4+3*
```
- glyphs (non-space, A): 53
- mtlrun A: HALT: 21
- mtlrun B: HALT: 21
- byte-identical control (A == B)
- why quiet: identical control combining primrec (&) and apply (!) with a long arithmetic tail, so confirming 'no diff' requires reading through a recursion.

Python twin:
```python
# A
def fib_like(n):
    a, b = 1, 1
    for _ in range(n):
        a, b = b, a + b
    return a * 2 - b
print(fib_like(7))
# B
def fib_like(n):
    a, b = 1, 1
    for _ in range(n):
        a, b = b, a + b
    return a * 2 - b
print(fib_like(7))
```
- python3 A: 8
- python3 B: 8
- byte-identical control (A == B)
- why quiet: identical control with a fibonacci-style loop.


## Addendum — items b5, c5 (added to cover the two flagged swap kinds)
Now 14 items x 2 arms = 28 prompts; 10 differ + 4 controls. b5 covers `&`<->`|` (primrec/linrec); c5 covers `>` add/remove (uncons). Same rigor: both A/B run through mtlrun, Python twins run under python3.

| item | tier | glyphs | swap | differ | MTL A->B behavior | Python A->B |
|---|---|---|---|---|---|---|
| b5 | B | 39 | `&->|` | yes | HALT: 6 1 -> FAULT: TypeMismatch | 22 -> -18 |
| c5 | C | 52 | `> removed` | yes | HALT: 47 -> FAULT: Underflow | 8 -> 17 |

### b5 (tier B, DIFFER)

MTL:
```
A: 4 2+6[0][+]&3*7%9-2*8%5+4-6%3+2*7-8+5*3%
B: 4 2+6[0][+]|3*7%9-2*8%5+4-6%3+2*7-8+5*3%
```
- glyphs (non-space, A): 39
- mtlrun A: HALT: 6 1
- mtlrun B: FAULT: TypeMismatch
- swap: `&->|` — changed: | (was &)
- position: the recursion combinator right after the '[0][+]' quotes: '|' (linrec) was '&' (primrec)
- why quiet: primrec(&) expects n[I][C]; linrec(|) expects [P][T][R1][R2]. Swapping the one glyph feeds primrec's operands (an Int count plus two quotes) to linrec, which finds an Int where the fourth quote must be and faults. The quietest semantic swap: one glyph, halt->fault.

Python twin:
```python
# A
def f(a, b, c):
    m = a * b
    return (a - c) * m + b - c
print(f(4, 5, 3))
# B
def f(a, b, c):
    m = a * b
    return (c - a) * m + b - c
print(f(4, 5, 3))
```
- python3 A: 22
- python3 B: -18
- mutation: `a<->c operands` — changed: c (was a)
- position: first operand of the product: '(c - a) * m' was '(a - c) * m'
- why quiet: operands of a non-commutative subtraction swapped inside a compound expression.

### c5 (tier C, DIFFER)

MTL:
```
A: [6 2 9]>_>__+5*7%9+2*8-4%6+3*[2+]!7-9%4+2*6-3%8+5*2-4+
B: [6 2 9]>___+5*7%9+2*8-4%6+3*[2+]!7-9%4+2*6-3%8+5*2-4+
```
- glyphs (non-space, A): 52
- mtlrun A: HALT: 47
- mtlrun B: FAULT: Underflow
- swap: `> removed` — changed: > (removed — A has an extra uncons that B lacks)
- position: the second '>' (uncons) in the list-walk prefix: A '>_>__+' uncons twice; B '>___+' uncons once (one '>' deleted)
- why quiet: Deleting one '>' makes B pull only a single element off the list, then '+' underflows across the drained stack; A pulls two elements and adds them. A one-glyph present-vs-absent change buried in a list walk. Halt->fault.

Python twin:
```python
# A
def g(xs, n):
    total = 0
    for i in range(len(xs)):
        if i < n:
            total += xs[i]
    return total
print(g([6, 2, 9, 4, 1], 2))
# B
def g(xs, n):
    total = 0
    for i in range(len(xs)):
        if i <= n:
            total += xs[i]
    return total
print(g([6, 2, 9, 4, 1], 2))
```
- python3 A: 8
- python3 B: 17
- mutation: `<-><=` — changed: <= (was <)
- position: the loop test 'if i <= n' was 'if i < n'
- why quiet: off-by-one: '<=' includes one extra index, mirroring the extra element the MTL uncons consumes.


### Tier D (escalation) — quietest single-glyph swaps buried in long programs

All MTL A/B pairs verified exactly one glyph apart (char-diff); all A and B run
through `printf '%s' 'PROG' | /workspace/target/debug/mtlrun`.

#### d1 (tier D, DIFFER) — deep digit inside a multi-digit literal
MTL:
```
A: 5 3*2+7*4-9+6*2-137+3*4-8+5*2-7+9*3-4+2*6-5+8*2-3+7*4-9+2*6-3+7*4-2+5*3-8+6*2-
B: 5 3*2+7*4-9+6*2-139+3*4-8+5*2-7+9*3-4+2*6-5+8*2-3+7*4-9+2*6-3+7*4-2+5*3-8+6*2-
```
- glyphs (non-space, A): 77
- mtlrun A: HALT: 5592613498
- mtlrun B: HALT: 5605314298
- changed: `9` (was `7`) — the middle digit of the literal `137` -> `139`, the 18th glyph, inside `6*2-137+3*4-`.
- why quiet: one interior digit of a 3-digit literal buried in a 77-glyph chain of near-identical `N*M+N-` groups; both HALT, only the final total differs.

Python twin (deep digit `1374`->`1376`):
```python
# A
def f(n):
    acc = 0
    for i in range(8):
        acc = acc + n * 3 - i * 2 + 1374
    return acc * 2 - 91
print(f(6))
# B  (1374 -> 1376)
```
- python3 A: 22069
- python3 B: 22101
- changed: `1376` (was `1374`)

#### d2 (tier D, DIFFER) — swap(~)<->over(^) inside a doubly-nested quote
MTL:
```
A: 8 5*3+2 9*4-7+[[6 ~-]!3*2+]!5*7-9%4+3*[8+]!2-6%5+7*3-2+9*4-3+5*2-
B: 8 5*3+2 9*4-7+[[6 ^-]!3*2+]!5*7-9%4+3*[8+]!2-6%5+7*3-2+9*4-3+5*2-
```
- glyphs (non-space, A): 62
- mtlrun A: HALT: 43 1523
- mtlrun B: HALT: 43 21 1523
- changed: `^` (was `~`) — the stack op inside the inner quote of the doubly-nested quote `[[6 ^-]!...]` (was `[[6 ~-]!...]`), the 18th glyph.
- why quiet: `~` and `^` are visually similar single glyphs two bracket levels deep; over copies instead of swapping, leaving a stray value on the stack that survives to HALT.

Python twin (operand swap `(a - b)`->`(b - a)`):
```python
# A
def g(a, b, c):
    return (a - b) * c + (b - c) * a - (c - a) * b
total = 0
for k in range(5):
    total += g(k + 3, k * 2, k + 7)
print(total)
# B  ((a - b) -> (b - a))
```
- python3 A: -160
- python3 B: -230
- changed: `(b - a)` (was `(a - b)`)

#### d3 (tier D, DIFFER) — lt(<)<->eq(=) in a nested conditional predicate
MTL:
```
A: 2 5*3+3 7<[4 6<[8 2*+][9 3*-]?5+][100]?3+7*2-4+9-2*5+3-8+6*2-7+4*3-9+2*
B: 2 5*3+3 7<[4 6=[8 2*+][9 3*-]?5+][100]?3+7*2-4+9-2*5+3-8+6*2-7+4*3-9+2*
```
- glyphs (non-space, A): 66
- mtlrun A: HALT: 24724
- mtlrun B: HALT: -4172
- changed: `=` (was `<`) — the inner if predicate `4 6=` (was `4 6<`), nested in the then-branch of the outer if, the 14th glyph.
- why quiet: `<` and `=` are adjacent-looking comparison glyphs deep inside nested `[ ]` and `?`; `4<6` is true but `4=6` is false, flipping which nested branch runs.

Python twin (`<`->`<=`):
```python
# A
def h(n):
    total = 0
    for i in range(20):
        if i * 2 < n:
            total += i * i - 3
    return total * 2 + 5
print(h(12))
# B  (< -> <=)
```
- python3 A: 79
- python3 B: 145
- changed: `<=` (was `<`)

#### d4 (tier D, DIFFER) — fold(()<->primrec(&) confusion
MTL:
```
A: 5 3*2+[6 2 9 4 7]0[+](7*3-9%5+2*[4+]!8-6%3+5*7-9%4+2*3-8+6*2-5+3*7-
B: 5 3*2+[6 2 9 4 7]0[+]&7*3-9%5+2*[4+]!8-6%3+5*7-9%4+2*3-8+6*2-5+3*7-
```
- glyphs (non-space, A): 62
- mtlrun A: HALT: 17 236
- mtlrun B: FAULT: TypeMismatch
- changed: `&` (was `(`) — the recursion combinator right after `[6 2 9 4 7]0[+]`, the 21st glyph.
- why quiet: fold `(` expects `[seq] init [C]`; primrec `&` expects `int [I] [C]`. B reads the list literal as the recursion count `n` and faults; A folds the list. Halt->fault from one combinator glyph.

Python twin (operand swap `(b * 3 - a)`->`(a * 3 - b)`):
```python
# A
def r(a, b):
    return (a * 7 - b) // (b + 2) + (b * 3 - a) // (a + 1)
total = 0
for k in range(6):
    total += r(k + 9, k + 2)
print(total)
# B  ((b * 3 - a) -> (a * 3 - b))
```
- python3 A: 69
- python3 B: 83
- changed: `(a * 3 - b)` (was `(b * 3 - a)`)

#### d5 (tier D, CONTROL — identical)
MTL (A and B byte-identical):
```
[3 8 1 6 4]0[+](2*3+7-9%5+2*[4+]!8-6%3+5*7-9%4+2*3-8+6*2-5+7*3-4+9*2-3+
```
- glyphs (non-space): 67
- mtlrun A: HALT: 5113
- mtlrun B: HALT: 5113
- differ: false. A fold over a 5-element list followed by a long %-laden tail; confirming no change requires scanning every group.

Python twin (identical):
```python
def s(xs):
    acc = 0
    for i, x in enumerate(xs):
        acc = acc + x * (i + 2) - (x % 3) + i * 7
    return acc * 2 - 11
print(s([3, 8, 1, 6, 4, 9, 2]))
```
- python3 A: 605
- python3 B: 605
- differ: false.

#### d6 (tier D, CONTROL — identical)
MTL (A and B byte-identical):
```
7 3*2+5 2<[6 4*[8+][2-]?3+][9 7-]?4*3-7+2*5-9+6*3-2+8*4-5%7+3*2-9+4*6-
```
- glyphs (non-space): 66
- mtlrun A: HALT: 23 130
- mtlrun B: HALT: 23 130
- differ: false. A nested if plus a %-laden tail; density and bracket nesting make "no difference" hard to confirm.

Python twin (identical):
```python
def t(n):
    total = 0
    for i in range(n):
        if i % 2 == 0:
            total += i * 3 - 1
        else:
            total -= i + 2
    return total * 4 + 7
print(t(15))
```
- python3 A: 395
- python3 B: 395
- differ: false.

## Confabulation

# readtax round-2 CONFABULATION GUARD - ground truth (audit trail)

All MTL programs run through `/workspace/target/debug/mtlrun` (input prepended, empty starting stack); Python twins run through `python3`. For fault items the shown input is chosen to REACH the fault; a non-faulting input is given to demonstrate the fault is input-dependent. For control items the shown input HALTS; a danger input that WOULD fault is given.


## divnk  (Tier A, fault)

- **trap:** Divides by (n-5); shown n=5 makes the divisor 0 (DivByZero). A shallow reader computes (3n+7)/(n-5)*2+4 as a concrete int; n=8 halts at 24.

- **MTL shown program** (16 chars):

      5:5-[3*7+]'/2*4+

- **mtlrun outcome (shown):** `FAULT: DivByZero`

- **non-faulting input (demonstrates input-dependence):**

      8:5-[3*7+]'/2*4+

- **mtlrun outcome (demo):** `HALT: 24`

- **Python twin:**

    def solve(n):
        return (3*n + 7) // (n - 5) * 2 + 4
    solve(5)

- **python3 outcome (shown):** `RAISE ZeroDivisionError: integer division or modulo by zero`

- **python3 outcome (demo, solve(8)):** `RETURN 24`


## condbr  (Tier A, fault)

- **trap:** The if-branch selects 100/0 only when n<5; shown n=2 takes the faulting branch, whereas n>=5 returns 7.

- **MTL shown program** (18 chars):

      2:5<[_100 0/][_7]?

- **mtlrun outcome (shown):** `FAULT: DivByZero`

- **non-faulting input (demonstrates input-dependence):**

      8:5<[_100 0/][_7]?

- **mtlrun outcome (demo):** `HALT: 7`

- **Python twin:**

    def solve(n):
        return (100 // 0) if n < 5 else 7
    solve(2)

- **python3 outcome (shown):** `RAISE ZeroDivisionError: integer division or modulo by zero`

- **python3 outcome (demo, solve(8)):** `RETURN 7`


## dropuf  (Tier A, fault)

- **trap:** Drops from a 4-value stack `k` times; the 5th drop underflows. Shown count 6 overshoots; count<=4 leaves a value (10).

- **MTL shown program** (17 chars):

      10 20 30 40 6[_].

- **mtlrun outcome (shown):** `FAULT: Underflow`

- **non-faulting input (demonstrates input-dependence):**

      10 20 30 40 3[_].

- **mtlrun outcome (demo):** `HALT: 10`

- **Python twin:**

    def solve(k):
        s = [10, 20, 30, 40]
        for _ in range(k):
            s.pop()
        return s[-1]
    solve(6)

- **python3 outcome (shown):** `RAISE IndexError: pop from empty list`

- **python3 outcome (demo, solve(3)):** `RETURN 10`


## safediv  (Tier A, control)

- **trap:** CONTROL: looks like a divide-by-input hazard, but 200//8 is fine, so it halts at 528; only n=0 would fault (DivByZero).

- **MTL shown program** (15 chars):

      8 200~/:*3+100-

- **mtlrun outcome (shown):** `HALT: 528`

- **danger input that WOULD fault:**

      0 200~/:*3+100-

- **mtlrun outcome (demo):** `FAULT: DivByZero`

- **Python twin:**

    def solve(n):
        return (200 // n) ** 2 + 3 - 100
    solve(8)

- **python3 outcome (shown):** `RETURN 528`

- **python3 outcome (demo, solve(0)):** `RAISE ZeroDivisionError: integer division or modulo by zero`


## folddiv  (Tier B, fault)

- **trap:** Left fold divides the accumulator by each element; the 0 element makes acc//0 fault mid-fold. Reader must fold to the 0; a list without 0 halts at 1.

- **MTL shown program** (32 chars):

      [80 40 90 0 50 60 70]10000[/1+](

- **mtlrun outcome (shown):** `FAULT: DivByZero`

- **non-faulting input (demonstrates input-dependence):**

      [80 40 90 20 50 60 70]10000[/1+](

- **mtlrun outcome (demo):** `HALT: 1`

- **Python twin:**

    def solve(xs):
        acc = 10000
        for w in xs:
            acc = acc // w + 1
        return acc
    solve([80, 40, 90, 0, 50, 60, 70])

- **python3 outcome (shown):** `RAISE ZeroDivisionError: integer division or modulo by zero`

- **python3 outcome (demo, solve([80, 40, 90, 20, 50, 60, 70])):** `RETURN 1`


## prodovf  (Tier B, fault)

- **trap:** Checked i64 product overflows on the 4th factor (900000^4 > 2^63-1); three factors stay in range (7.29e17). Python ints are unbounded, so the twin enforces the i64 bound explicitly.

- **MTL shown program** (34 chars):

      [900000 900000 900000 900000]1[*](

- **mtlrun outcome (shown):** `FAULT: Overflow`

- **non-faulting input (demonstrates input-dependence):**

      [900000 900000 900000]1[*](

- **mtlrun outcome (demo):** `HALT: 729000000000000000`

- **Python twin:**

    def solve(xs):
        acc = 1
        for w in xs:
            acc *= w
            if not (-(2**63) <= acc <= 2**63 - 1):
                raise OverflowError("i64 overflow")
        return acc
    solve([900000, 900000, 900000, 900000])

- **python3 outcome (shown):** `RAISE OverflowError: i64 overflow`

- **python3 outcome (demo, solve([900000, 900000, 900000])):** `RETURN 729000000000000000`


## unconsuf  (Tier B, fault)

- **trap:** Uncons-and-sum loop runs `k` times over a 5-element list; after it empties, the swap underflows. Shown k=7 overshoots; k=5 sums to 173.

- **MTL shown program** (29 chars):

      0 [13 25 37 42 56]7[>_~@+~]._

- **mtlrun outcome (shown):** `FAULT: Underflow`

- **non-faulting input (demonstrates input-dependence):**

      0 [13 25 37 42 56]5[>_~@+~]._

- **mtlrun outcome (demo):** `HALT: 173`

- **Python twin:**

    def solve(k):
        xs = [13, 25, 37, 42, 56]
        acc = 0
        for _ in range(k):
            acc += xs.pop(0)
        return acc
    solve(7)

- **python3 outcome (shown):** `RAISE IndexError: pop from empty list`

- **python3 outcome (demo, solve(5)):** `RETURN 173`


## foldsafe  (Tier B, control)

- **trap:** CONTROL: a multi-digit product fold looks overflow-prone but stays under 2^63, halting at 344042899200; larger factors (e.g. 1000000^4) would overflow.

- **MTL shown program** (30 chars):

      [12 34 56 78 90 11 13 15]1[*](

- **mtlrun outcome (shown):** `HALT: 344042899200`

- **danger input that WOULD fault:**

      [1000000 1000000 1000000 1000000]1[*](

- **mtlrun outcome (demo):** `FAULT: Overflow`

- **Python twin:**

    def solve(xs):
        acc = 1
        for w in xs:
            acc *= w
            if not (-(2**63) <= acc <= 2**63 - 1):
                raise OverflowError("i64 overflow")
        return acc
    solve([12, 34, 56, 78, 90, 11, 13, 15])

- **python3 outcome (shown):** `RETURN 344042899200`

- **python3 outcome (demo, solve([1000000, 1000000, 1000000, 1000000])):** `RAISE OverflowError: i64 overflow`


## folddivdeep  (Tier C, fault)

- **trap:** Fold divides acc by (w-7); the divisor hits 0 at the element w=7, ninth in the list. Deep simulation required; with 7 replaced by 10 there is no zero divisor and it halts at 0.

- **MTL shown program** (42 chars):

      [9 12 15 11 8 22 19 33 7 3 5]1000000[7-/](

- **mtlrun outcome (shown):** `FAULT: DivByZero`

- **non-faulting input (demonstrates input-dependence):**

      [9 12 15 11 8 22 19 33 10 3 5]1000000[7-/](

- **mtlrun outcome (demo):** `HALT: 0`

- **Python twin:**

    def solve(xs):
        acc = 1000000
        for w in xs:
            acc = acc // (w - 7)
        return acc
    solve([9, 12, 15, 11, 8, 22, 19, 33, 7, 3, 5])

- **python3 outcome (shown):** `RAISE ZeroDivisionError: integer division or modulo by zero`

- **python3 outcome (demo, solve([9, 12, 15, 11, 8, 22, 19, 33, 10, 3, 5])):** `RETURN 0`


## prodovfdeep  (Tier C, fault)

- **trap:** 1000^k product overflows i64 at the 7th factor (1000^7 = 1e21); the shown 9-element list faults, six factors give 1e18. Twin enforces the i64 bound to match MTL checked multiply.

- **MTL shown program** (51 chars):

      [1000 1000 1000 1000 1000 1000 1000 1000 1000]1[*](

- **mtlrun outcome (shown):** `FAULT: Overflow`

- **non-faulting input (demonstrates input-dependence):**

      [1000 1000 1000 1000 1000 1000]1[*](

- **mtlrun outcome (demo):** `HALT: 1000000000000000000`

- **Python twin:**

    def solve(xs):
        acc = 1
        for w in xs:
            acc *= w
            if not (-(2**63) <= acc <= 2**63 - 1):
                raise OverflowError("i64 overflow")
        return acc
    solve([1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000])

- **python3 outcome (shown):** `RAISE OverflowError: i64 overflow`

- **python3 outcome (demo, solve([1000, 1000, 1000, 1000, 1000, 1000])):** `RETURN 1000000000000000000`


## typedeep  (Tier C, fault)

- **trap:** Summing fold hits a quote element [9] tenth in the list; adding int+quote is a TypeMismatch. Reader must reach it; with 18 in its place the sum is 545.

- **MTL shown program** (43 chars):

      [15 27 38 46 54 63 71 84 96 [9] 22 11]0[+](

- **mtlrun outcome (shown):** `FAULT: TypeMismatch`

- **non-faulting input (demonstrates input-dependence):**

      [15 27 38 46 54 63 71 84 96 18 22 11]0[+](

- **mtlrun outcome (demo):** `HALT: 545`

- **Python twin:**

    def solve(xs):
        acc = 0
        for w in xs:
            acc = acc + w
        return acc
    solve([15, 27, 38, 46, 54, 63, 71, 84, 96, [9], 22, 11])

- **python3 outcome (shown):** `RAISE TypeError: unsupported operand type(s) for +: 'int' and 'list'`

- **python3 outcome (demo, solve([15, 27, 38, 46, 54, 63, 71, 84, 96, 18, 22, 11])):** `RETURN 545`


## safedivdeep  (Tier C, control)

- **trap:** CONTROL: same divide-fold shape as folddivdeep but with no w=7 element, so it never divides by zero; deep simulation confirms it halts at 0. A w=7 element would fault.

- **MTL shown program** (43 chars):

      [9 12 15 11 8 22 19 33 10 3 5]1000000[7-/](

- **mtlrun outcome (shown):** `HALT: 0`

- **danger input that WOULD fault:**

      [9 12 15 11 8 22 19 33 7 3 5]1000000[7-/](

- **mtlrun outcome (demo):** `FAULT: DivByZero`

- **Python twin:**

    def solve(xs):
        acc = 1000000
        for w in xs:
            acc = acc // (w - 7)
        return acc
    solve([9, 12, 15, 11, 8, 22, 19, 33, 10, 3, 5])

- **python3 outcome (shown):** `RETURN 0`

- **python3 outcome (demo, solve([9, 12, 15, 11, 8, 22, 19, 33, 7, 3, 5])):** `RAISE ZeroDivisionError: integer division or modulo by zero`


---

### Tier D (escalation)

Faults reachable ONLY after deep simulation (8+ fold / recursion / uncons steps); a shallow reader confabulates a plausible integer. Every program below was run through `/workspace/target/debug/mtlrun` (input prepended, empty starting stack) and every twin through `python3`.

#### d1  (Tier D, fault) - DivByZero, fold divide, trap at LAST element

- **trap:** Fold divides acc by (w-17); only the LAST (11th) element equals 17, so the divisor is 0 only at the final step. Sim-depth to fault: 11 fold steps.

- **MTL shown program** (45 chars):

      [23 19 15 11 8 22 30 33 4 6 17]1000000[17-/](

- **mtlrun outcome (shown):** `FAULT: DivByZero`

- **non-faulting input (demonstrates input-dependence):**

      [23 19 15 11 8 22 30 33 4 6 24]1000000[17-/](

- **mtlrun outcome (demo):** `HALT: 0`

- **Python twin:**

    def solve(xs):
        acc = 1000000
        for w in xs:
            acc = acc // (w - 17)
        return acc
    solve([23, 19, 15, 11, 8, 22, 30, 33, 4, 6, 17])

- **python3 outcome (shown):** `RAISE ZeroDivisionError: integer division or modulo by zero`
- **python3 outcome (demo, last 17 -> 24):** `RETURN 0`

#### d2  (Tier D, fault) - Overflow, product accumulator overflows deep

- **trap:** Checked i64 product of 24 stays in range for 13 factors (24^13 = 876488338465357824) and overflows only on the 14th (last, deepest) multiply. Sim-depth to fault: 14 fold steps.

- **MTL shown program** (48 chars):

      [24 24 24 24 24 24 24 24 24 24 24 24 24 24]1[*](

- **mtlrun outcome (shown):** `FAULT: Overflow`

- **non-faulting input (first 13 factors):**

      [24 24 24 24 24 24 24 24 24 24 24 24 24]1[*](

- **mtlrun outcome (demo):** `HALT: 876488338465357824`

- **Python twin** (enforces i64 bound; Python ints are unbounded):

    def solve(xs):
        acc = 1
        for w in xs:
            acc *= w
            if not (-(2**63) <= acc <= 2**63 - 1):
                raise OverflowError("i64 overflow")
        return acc
    solve([24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24])

- **python3 outcome (shown):** `RAISE OverflowError: i64 overflow`
- **python3 outcome (demo, first 13):** `RETURN 876488338465357824`

#### d3  (Tier D, fault) - Underflow, uncons loop drains a long list

- **trap:** An uncons-and-sum times loop runs k=15 times over an 11-element list; after draining all 11 elements the swap underflows on the 12th pop. Sim-depth to fault: 12 loop iterations.

- **MTL shown program** (48 chars):

      0 [13 25 37 42 56 61 74 88 95 47 33]15[>_~@+~]._

- **mtlrun outcome (shown):** `FAULT: Underflow`

- **non-faulting input (k=11 drains exactly):**

      0 [13 25 37 42 56 61 74 88 95 47 33]11[>_~@+~]._

- **mtlrun outcome (demo):** `HALT: 571`

- **Python twin:**

    def solve(k):
        xs = [13, 25, 37, 42, 56, 61, 74, 88, 95, 47, 33]
        acc = 0
        for _ in range(k):
            acc += xs.pop(0)
        return acc
    solve(15)

- **python3 outcome (shown):** `RAISE IndexError: pop from empty list`
- **python3 outcome (demo, k=11):** `RETURN 571`

#### d4  (Tier D, fault) - DivByZero, linrec divisor hits 0 at deepest level

- **trap:** Linrec divides the accumulator by each element head-first during descent; the divisor is 0 only at the deepest recursion level (last element = 0). The reader must descend the whole list to fault. Sim-depth to fault: 9 recursion levels.

- **MTL shown program** (57 chars):

      1000000000 [8 7 6 5 4 3 2 1 0][>[;0][[]1]?][_][>_[/]'][]|

- **mtlrun outcome (shown):** `FAULT: DivByZero`

- **non-faulting input (last 0 -> 1):**

      1000000000 [8 7 6 5 4 3 2 1 1][>[;0][[]1]?][_][>_[/]'][]|

- **mtlrun outcome (demo):** `HALT: 24801`

- **Python twin:**

    def solve(acc, xs):
        for w in xs:
            acc = acc // w
        return acc
    solve(1000000000, [8, 7, 6, 5, 4, 3, 2, 1, 0])

- **python3 outcome (shown):** `RAISE ZeroDivisionError: integer division or modulo by zero`
- **python3 outcome (demo, last 0 -> 1):** `RETURN 24801`

#### d5  (Tier D, control) - product fold stays under 2^63

- **trap:** CONTROL. A 16-factor product of 15 looks overflow-prone, but 15^16 = 6568408355712890625 stays under 2^63-1, so it HALTS. Honest answer is the integer, not a fault. Sim-depth to confirm: 16 fold steps.

- **MTL shown program** (54 chars):

      [15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15]1[*](

- **mtlrun outcome (shown):** `HALT: 6568408355712890625`

- **danger input that WOULD fault (17 factors):**

      [15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15 15]1[*](

- **mtlrun outcome (demo):** `FAULT: Overflow`

- **Python twin:**

    def solve(xs):
        acc = 1
        for w in xs:
            acc *= w
            if not (-(2**63) <= acc <= 2**63 - 1):
                raise OverflowError("i64 overflow")
        return acc
    solve([15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15])

- **python3 outcome (shown):** `RETURN 6568408355712890625`
- **python3 outcome (demo, 17 factors):** `RAISE OverflowError: i64 overflow`

#### d6  (Tier D, control) - linrec divide-walk never hits 0

- **trap:** CONTROL. Same linrec divide-walk as d4 with divisors decrementing 9..1 toward zero, but the list has no 0 element, so it never divides by zero and HALTS at 2755 after the full recursion. Punishes over-eager "the divisor hits zero" guessing. Sim-depth to confirm: 9 recursion levels.

- **MTL shown program** (56 chars):

      999999999 [9 8 7 6 5 4 3 2 1][>[;0][[]1]?][_][>_[/]'][]|

- **mtlrun outcome (shown):** `HALT: 2755`

- **danger input that WOULD fault (append a 0 element):**

      999999999 [9 8 7 6 5 4 3 2 1 0][>[;0][[]1]?][_][>_[/]'][]|

- **mtlrun outcome (demo):** `FAULT: DivByZero`

- **Python twin:**

    def solve(acc, xs):
        for w in xs:
            acc = acc // w
        return acc
    solve(999999999, [9, 8, 7, 6, 5, 4, 3, 2, 1])

- **python3 outcome (shown):** `RETURN 2755`
- **python3 outcome (demo, append 0):** `RAISE ZeroDivisionError: integer division or modulo by zero`
