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
