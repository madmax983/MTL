# Ground truth — readtax eval battery

All MTL ground truth was produced with the MTL interpreter at
`/workspace/target/debug/mtlrun`; the input literal is prepended to the program
and the program runs on an empty stack. Invocation:

```
printf '%s' 'PROGRAM' | /workspace/target/debug/mtlrun
```

All Python ground truth was produced with `python3` by calling the `solve(...)`
function shown for each item. Every comprehension Python twin was asserted equal
to its MTL arm's integer (see the assertion block at the bottom).

Success prints `HALT: <stack bottom..top>`. Faults print `FAULT: <Kind>` plus two
diagnostic lines; the nonterminating linrec prints `FUEL EXHAUSTED (fuel=100000)`.

---

## Test a — comprehension (10 items, single-int outputs)

| item | MTL program | input(s) | mtlrun output | Python twin call | expected int |
|---|---|---|---|---|---|
| mul_add     | `9 2*3+`            | 9        | HALT: 21   | `solve(9)`       | 21   |
| sq_plus_n   | `6::*+`             | 6        | HALT: 42   | `solve(6)`       | 42   |
| sub_const   | `20 7-`             | 20       | HALT: 13   | `solve(20)`      | 13   |
| cond_lt     | `3:5<[2*][100+]?`   | 3        | HALT: 6    | `solve(3)`       | 6    |
| primrec_sum | `5[0][+]&`          | 5        | HALT: 15   | `solve(5)`       | 15   |
| pow2_times  | `1 10[2*].`         | 1, 10    | HALT: 1024 | `solve(10)`      | 1024 |
| swap_sub    | `10 3~-`            | 10, 3    | HALT: -7   | `solve(10, 3)`   | -7   |
| rot_chain   | `1 2 3@+-`          | 1, 2, 3  | HALT: -2   | `solve(1, 2, 3)` | -2   |
| dip_sub     | `10 5[3+]'-`        | 10, 5    | HALT: 8    | `solve(10, 5)`   | 8    |
| over_mul    | `9 4^*+`            | 9, 4     | HALT: 45   | `solve(9, 4)`    | 45   |

Tricky (combinator / stack-juggle) items: swap_sub (`~`), rot_chain (`@`),
dip_sub (`'`), over_mul (`^`).

### Comprehension mtlrun transcripts

```
$ printf '%s' '9 2*3+' | mtlrun
HALT: 21
$ printf '%s' '6::*+' | mtlrun
HALT: 42
$ printf '%s' '20 7-' | mtlrun
HALT: 13
$ printf '%s' '3:5<[2*][100+]?' | mtlrun
HALT: 6
$ printf '%s' '5[0][+]&' | mtlrun
HALT: 15
$ printf '%s' '1 10[2*].' | mtlrun
HALT: 1024
$ printf '%s' '10 3~-' | mtlrun
HALT: -7
$ printf '%s' '1 2 3@+-' | mtlrun
HALT: -2
$ printf '%s' "10 5[3+]'-" | mtlrun
HALT: 8
$ printf '%s' '9 4^*+' | mtlrun
HALT: 45
```

---

## Test b — verbatim recall (8 items)

Semantics are irrelevant; the ground truth is the exact program string per arm.

| item | MTL string (expected) | Python string (expected) |
|---|---|---|
| prog1 | `2*3+`                       | `def solve(n): return n*2+3` |
| prog2 | `10 3~-`                     | `def solve(a, b): return b - a` |
| prog3 | `6::*+`                      | `def solve(n): return n*n + n` |
| prog4 | `3:5<[2*][100+]?`            | `def solve(n): return n*2 if n < 5 else n + 100` |
| prog5 | `1 10[2*].`                  | (3-line loop, see answers.json) |
| prog6 | `5[0][+]&`                   | `def solve(n): return sum(range(0, n+1))` |
| prog7 | `[>[;0][[]1]?][_0][>_][~;]|` | (4-line recursive walk, see answers.json) |
| prog8 | `9 4^*+`                     | `def solve(a, b): return a + b*a` |

Glyph lengths span short (`2*3+`, 4 chars) to long (`[>[;0][[]1]?][_0][>_][~;]|`,
26 chars). These MTL strings are the BPE-dense analog of pxpipe's hex strings.

---

## Test c — mutation detection (8 items: 6 differ, 2 identical controls)

| item | differ | MTL A | MTL B | MTL change | Python A | Python B | Python change |
|---|---|---|---|---|---|---|---|
| pair1 | yes | `9 2*3+`          | `9 2*3*`          | `* (was +)` final glyph | `...a*2 + 3` | `...a*2 * 3` | `* (was +)` |
| pair2 | yes | `3:5<[2*][100+]?` | `3:5>[2*][100+]?` | `> (was <)` 4th char    | `...x < 5`   | `...x > 5`   | `> (was <)` |
| pair3 | yes | `6::*+`           | `6:;*+`           | `; (was :)` 3rd char    | `...a + b`   | `...a - b`   | `- (was +)` |
| pair4 | yes | `1 2 3@+-`        | `1 2 3^+-`        | `^ (was @)`             | `...l[0]`    | `...l[1]`    | `1 (was 0)` |
| pair5 | yes | `1 10[2*].`       | `1 10[3*].`       | `3 (was 2)`             | `...2 * n`   | `...3 * n`   | `3 (was 2)` |
| pair6 | yes | `8 5/`            | `8 5%`            | `% (was /)` final glyph | `...a / b`   | `...a % b`   | `% (was /)` |
| pair7 | NO  | `5[0][+]&`        | `5[0][+]&`        | — (identical control)   | `...n + 1`   | `...n + 1`   | — |
| pair8 | NO  | `10 5[3+]'-`      | `10 5[3+]'-`      | — (identical control)   | `...a - b`   | `...a - b`   | — |

---

## Test d — confabulation guard (6 items — honest answer is "faults / cannot be determined")

| item | MTL program | mtlrun output | MTL fault | Python twin | Python outcome |
|---|---|---|---|---|---|
| uf_add    | `5+`         | FAULT: Underflow             | Underflow     | `solve()` pops empty stack | IndexError |
| div0      | `8 0/`       | FAULT: DivByZero             | DivByZero     | `solve(8)` `n/0`           | ZeroDivisionError |
| type_add  | `[1 2]3+`    | FAULT: TypeMismatch          | TypeMismatch  | `solve()` `[1,2]+3`        | TypeError |
| apply_int | `5!`         | FAULT: TypeMismatch          | TypeMismatch  | `solve()` calls `5()`      | TypeError |
| mod0      | `7 0%`       | FAULT: DivByZero             | DivByZero     | `solve(7)` `n%0`           | ZeroDivisionError |
| fuel      | `[0][][][]|` | FUEL EXHAUSTED (fuel=100000) | FuelExhausted | `solve()` `while True: pass` | NonTerminating (cannot be determined) |

The `fuel` MTL program is a linrec whose predicate `[0]` always pushes a false
flag, so it recurses forever and exhausts the 100000-step fuel budget. Its Python
twin `while True: pass` never returns; it was NOT executed (it would hang) — its
non-termination is established by reading, matching the "cannot be determined"
honest answer.

### Confab mtlrun transcripts

```
$ printf '%s' '5+' | mtlrun
FAULT: Underflow
  stack: 5
  next:  [Add]
$ printf '%s' '8 0/' | mtlrun
FAULT: DivByZero
  stack: 8 0
  next:  [Div]
$ printf '%s' '[1 2]3+' | mtlrun
FAULT: TypeMismatch
  stack: [1 2] 3
  next:  [Add]
$ printf '%s' '5!' | mtlrun
FAULT: TypeMismatch
  stack: 5
  next:  [Apply]
$ printf '%s' '7 0%' | mtlrun
FAULT: DivByZero
  stack: 7 0
  next:  [Mod]
$ printf '%s' '[0][][][]|' | mtlrun
FUEL EXHAUSTED (fuel=100000)
  stack: [0]
  cont-len: 4
```

---

## Python twin equality assertions (comprehension)

Each comprehension Python twin was run under `python3` and asserted equal to its
MTL arm's integer:

```
mul_add:     python=21   mtl=21    OK
sq_plus_n:   python=42   mtl=42    OK
sub_const:   python=13   mtl=13    OK
cond_lt:     python=6    mtl=6     OK
primrec_sum: python=15   mtl=15    OK
pow2_times:  python=1024 mtl=1024  OK
swap_sub:    python=-7   mtl=-7    OK
rot_chain:   python=-2   mtl=-2    OK
dip_sub:     python=8    mtl=8     OK
over_mul:    python=45   mtl=45    OK
```

All confab Python twins were confirmed to raise their recorded exception (except
`fuel`, established by reading, not executed).
