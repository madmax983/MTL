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
