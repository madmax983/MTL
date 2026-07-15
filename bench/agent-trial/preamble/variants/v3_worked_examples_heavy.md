# MTL Quick Reference (v0.4) — worked-examples variant

MTL is a stack-based, point-free language: a flat sequence of words run left to
right against an operand stack. Learn it from the primitive table plus the
worked examples below. Top of stack is at the **right**; `[q]` is a quote (an
unevaluated program, also used as a list, e.g. `[1 2 3]`). `0` is false, nonzero
is true; comparisons yield `1`/`0`. A leading `-` is always `Sub`, never a sign.

## The 23 primitives

| Glyph | Name | Stack effect | Meaning |
|---|---|---|---|
| `[` `]` | quote | `( -- [q] )` | delimiters (not a word) |
| `:` | dup | `( a -- a a )` | duplicate top |
| `_` | drop | `( a -- )` | discard top |
| `~` | swap | `( a b -- b a )` | swap top two |
| `@` | rot | `( a b c -- b c a )` | rotate third to top |
| `^` | over | `( a b -- a b a )` | copy second over top |
| `!` | apply | `( [q] -- ... )` | run quote (splice into program) |
| `,` | cat | `( [a] [b] -- [ab] )` | concatenate quotes |
| `;` | cons | `( v [q] -- [v q] )` | prepend value into quote |
| `'` | dip | `( a [q] -- ... a )` | run `q` with `a` temporarily removed, then restore `a` on top |
| `+` | add | `( a b -- a+b )` | checked |
| `-` | sub | `( a b -- a-b )` | checked; `-` is always Sub |
| `*` | mul | `( a b -- a*b )` | checked |
| `/` | div | `( a b -- a/b )` | truncating; `b=0` faults |
| `%` | mod | `( a b -- a%b )` | truncating remainder (sign follows dividend); `b=0` faults |
| `=` | eq | `( a b -- 0\|1 )` | equality |
| `<` | lt | `( a b -- 0\|1 )` | less-than |
| `?` | if | `( c [t] [f] -- ... )` | run `[t]` if `c≠0` else `[f]` |
| `&` | primrec | `( n [I] [C] -- r )` | bounded primitive recursion |
| `.` | times | `( n [Q] -- ... )` | run `[Q]` n times |
| `\|` | linrec | `( [P][T][R1][R2] -- ... )` | linear/tail recursion |
| `>` | uncons | `( [w …] -- w [ … ] 1 )` or `( [] -- 0 )` | split a quote |
| `(` | fold | `( [seq] init [C] -- r )` | native left fold; `C:( acc w -- acc' )` runs once per element left-to-right; `[]` gives `init` |
| `$` | xor | `( a b -- a$b )` | bitwise XOR on the i64 two's-complement; total |

**Truncation vs Python.** `/` and `%` truncate toward zero (remainder sign
follows the dividend), unlike Python's floored `//`/`%`.

## Worked examples

1. **Arithmetic** — `n -> 3n+7`: `3*7+`. On stack `[5]`: `5 3 *` → `15`,
   `7 +` → `22`.
2. **Times loop** — `[2*].` with `n` and a seed below it runs `2*` n times.
   `.` supplies no counter; `[Q]` sees whatever the stack holds.
3. **PrimRec** — `n [I] [C] &`. `[0][+]&` computes `0+1+...+n`: base pushes 0,
   combine adds the counter to the subresult. Factorial is `[1][*]&`: base
   pushes 1, combine multiplies the counter `n` by the running product (`C`
   sees the stack `... n r`, current counter `n` and subresult `r`).
4. **LinRec over a list** with the non-destructive null test — skeleton:
   `[ >[;0][[]1]? ][ ...terminate... ][ ...pre... ][ ...post... ]|`.
   `|` runs predicate `P`; if its flag ≠ 0 run `T` (terminate); else run `R1`,
   recurse (the whole linrec), then run `R2`. The predicate `>[;0][[]1]?`
   leaves `[list] flag`, preserving the list: uncons; if non-empty (flag 1)
   cons the head back and push `0`; if empty push `[] 1`.
5. **Reverse / cons to build output** — begin with an empty accumulator quote
   `[]`, then for each input element `;` (cons) it onto the accumulator. Because
   cons prepends, walking the input front-to-back yields the reversed list. Use
   `'` (dip) to run the recursion under the growing accumulator, and `>_` to
   take a tail while discarding the uncons flag.
6. **Sum a list with native fold** — `[seq] 0 [+] (`. Fold seeds the
   accumulator with `0` and runs `[+]` once per element (`acc w -- acc'`). On
   `[1 2 3]`: `0[+](` → `6`; on `[]` it leaves the seed `0`.
7. **gcd via linrec (Euclid)** — with `a b` on the stack (`b` on top):
   `[:0=][_][~^%][]|`. Predicate `:0=` tests `b==0` (dup b, compare 0);
   terminate `_` drops the `0` leaving `a`; pre-step `~^%` computes
   `a b -- b (a%b)` to recurse on `(b, a%b)`; post `[]` is empty (tail form).
   On `12 8` → `4`; on `5 0` → `5`.
8. **reverse_list via linrec** — with a list quotation on the stack:
   `[]~[>[;0][[]1]?][_][>_[~;]'][]|`. Seed an empty accumulator `[]` under the
   list (`~`), null-test each step, and in the pre-step `>_[~;]'` take the head
   and cons it onto the accumulator held one level down with `'` (dip). On
   `[1 2 3]` → `[3 2 1]`; on `[]` → `[]`.

## Faults

Execution halts with a fault (no partial results): **Underflow** (too few
operands), **TypeMismatch** (wrong operand type — e.g. `!` on an Int, `+` on a
Quote, `>` head not a value), **Overflow** (checked arithmetic leaves `i64`),
**DivByZero** (`/` or `%` by 0). Precedence: arity first, then types, then
DivByZero/Overflow.
