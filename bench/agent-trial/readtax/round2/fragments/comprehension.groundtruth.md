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
