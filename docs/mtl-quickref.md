# MTL Quick Reference (v0.4)

> **Division of labor.** `docs/mtl-quickref.md` (this file, full) = **pure
> computation + host capabilities**. For pure-computation work only, the frozen
> [`docs/mtl-quickref-min.md`](mtl-quickref-min.md) (the 487-token cold-preamble
> ablation winner, PR #88) is the cheaper default; it omits the Host-capabilities
> section below, so any task that reaches the host needs this full reference.

New in v0.4: the **Host capabilities** section at the end of this document,
covering Tier-3 capability calls, the grant model, budgets, and host faults.

MTL is a stack-based, point-free language. A program is a flat sequence of
words executed left to right against an operand stack. There are no variables,
names, or environments — all abstraction is done with quotations.

## Value model

- **Int** — a signed 64-bit integer (`i64`).
- **Quote** — a quotation `[ ... ]`: an unevaluated program pushed as a value.
  Quotations are the only abstraction (functions, control flow, and lists are
  all quotes). A list is just a quote of ints, e.g. `[1 2 3]`.
- There are **no strings** and no booleans: `0` is false, any nonzero is true.
  Comparisons yield `1` or `0`.

## Literals and lexing

- Integer literals are **unsigned**: `[0-9]+`. A leading `-` is **never** part
  of a literal — `-` is always the `Sub` primitive. Write `-7` as `0 7 -`
  (push 0, push 7, subtract).
- Symbol words are single ASCII characters and **self-delimiting**: write them
  with no spaces, e.g. `~@^`, `3*7+`, `:!`.
- **Whitespace is required only** between two adjacent integer literals
  (`12 34`) or two adjacent named words. It is never needed around symbols:
  `3:*` is `3 dup *`. `[1 2+]` is a quote pushing 1, 2, then Add.
- `.` next to a digit is Times, never a decimal point: `3.` is `Int(3) Times`.

## The 23 primitives

Stack effect notation: top of stack is at the **right**. `[q]` is a quote.

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
| `$` | xor | `( a b -- a$b )` | bitwise XOR on the i64 two's-complement; total (no Overflow, no DivByZero) |

**Truncation vs Python.** `/` and `%` truncate toward zero (remainder sign follows the dividend), unlike Python's *floored* `//`/`%`: MTL `-7 % 3` is `-1` (truncated) where Python gives `2` (floored) — a porting trap for twins.

## Recursion combinators (exact rewrites)

Continuation splicing: `!` and the combinators splice quote bodies into the
program rather than calling a sub-interpreter.

- **`!` apply**: `[q] !` → runs the words of `q` in place.
- **`'` dip**: `a [q] '` → removes `a`, runs `q`, then pushes `a` back on top.
  Use it to operate one level down.
- **`&` primrec** `n [I] [C] &`: if `n ≤ 0`, run `I`. If `n > 0`, it recurses on
  `n-1` and then runs `C` with the stack `... n r` where `n` is the current
  count and `r` is the subresult. So `C` sees both the counter and the recursive
  result. Example: factorial `[1][*]&` — base pushes 1; combine multiplies the
  counter `n` by the running product.
- **`.` times** `n [Q] .`: run `[Q]` exactly `max(n,0)` times, left to right.
  `Q` sees whatever the stack holds; no counter is supplied.
- **`|` linrec** `[P][T][R1][R2] |`: desugars to if. Runs `P` (a predicate that
  leaves a flag on top), then: if flag ≠ 0 run `T` (terminate); else run `R1`,
  recurse (the whole linrec), then run `R2`. Tail recursion is `R2 = []`.
  Shape: `[predicate][then][before-recurse][after-recurse]|`.
- **`>` uncons**: `[h t...] >` → `h [t...] 1`; `[] >` → `0`. It consumes the
  quote once (affine). A non-empty list yields head value, tail quote, and flag
  `1`; empty yields just `0`. A quote whose head is not a pushed value faults.
- **Fuel is a depth budget.** Interpreter fuel is charged against recursion
  DEPTH, so a deep-but-valid `&`/`|`/`.` can exhaust fuel before completing;
  size the fuel budget to the expected recursion depth.

### List idioms

- **Non-destructive null test.** `>` consumes the quote, so to test emptiness
  without losing the list use `>[;0][[]1]?`: uncons; if non-empty (flag 1) run
  `[;0]` = cons the head back on and push flag `0` (rebuilt list, "not empty");
  if empty (flag 0) run `[[]1]` = push empty quote and flag `1`. Result:
  `[list] not_empty_flag` on top, list preserved.
- **Re-uncons gotcha.** After a null test rebuilds the list, you still need the
  head to do work. Re-split with `>` inside the recursive step, and use `>_`
  when you want the tail but not the flag, or `'` (dip) to work under the tail.
- **Building an output list.** Start with `[]` and `;` (cons) each element to
  grow a quote; consing while consuming the input reverses order.

## Faults

Execution halts with a fault (no partial results). Kinds:

- **Underflow** — too few operands for the primitive.
- **TypeMismatch** — an operand has the wrong type (e.g. `!` on an Int, `+` on
  a Quote, `>` head is not a value).
- **Overflow** — a checked arithmetic result leaves `i64`.
- **DivByZero** — `/` or `%` with divisor 0.

**Precedence** when a primitive cannot fire: **arity first** (Underflow),
**then operand types** (TypeMismatch), **then semantic checks** (DivByZero /
Overflow). E.g. `Int(1) Add` with one operand is Underflow, not TypeMismatch.

## Worked examples

1. **Arithmetic** — `n -> 3n+7`: `3*7+`. On stack `[5]`: `5 3 *` → `15`,
   `7 +` → `22`.
2. **Times loop** — sum 1..n is not needed here, but e.g. double n times:
   `[2*].` with `n` and a seed below it runs `2*` n times.
3. **PrimRec** — `n [I] [C] &`. `[0][+]&` computes `0+1+...+n`: base pushes 0,
   combine adds the counter to the subresult.
4. **LinRec over a list** with the non-destructive null test — skeleton:
   `[ >[;0][[]1]? ][ ...terminate... ][ ...pre... ][ ...post... ]|`.
   The predicate `>[;0][[]1]?` leaves `[list] flag`; when the list is empty the
   flag is `1` and `T` runs; otherwise `R1` runs (typically re-uncons with `>`
   and process the head), then linrec recurses, then `R2` runs.
5. **Reverse / cons to build output** — begin with an empty accumulator quote
   `[]`, then for each input element `; ` (cons) it onto the accumulator. Because
   cons prepends, walking the input front-to-back yields the reversed list. Use
   `'` (dip) to run the recursion under the growing accumulator, and `>_` to
   take a tail while discarding the uncons flag.

## Host capabilities (v0.4 Tier-3)

The pure language above is total and effect-free. Tier-3 programs also reach a
**host** through *capabilities* — named words serviced outside the verified
core. This is the only source of I/O, tools, and resources.

### Named words = capability Calls

- A **named word** `[a-z][a-z0-9]*` (e.g. `readline`, `emit`) is a host
  capability `Call`, **not** a core primitive. The core yields on every named
  word; the host services it, popping/pushing per the capability's stack effect.
- **Single glyphs** (`: _ ~ + ? | …`) are always pure core prims. Only
  lowercase-alphanumeric words hit the host. The lexer reads `-` as `sub` and
  `?` as `if`, so design names like `read-line`/`done?` are spelled
  `readline`/`donep`.

### Capability table

Stack effect notation matches the core (top at right). `{output}` = writes
output bytes (metered). "faults" lists the [`HostCode`](#fault-codes) a call may
raise; all can also raise `NotGranted`/`BudgetExhausted`.

**Input** (read host-owned fixture data):

| Call | Stack effect | Faults | Note |
|---|---|---|---|
| `readline` | `( -- h )` | InputClosed | first input line handle |
| `nextline` | `( -- h )` | InputClosed | next line handle; advances |
| `readlines` | `( -- [h…] )` | — | all lines as handle list |
| `readstate` | `( -- s )` | — | initial agent state (int) |
| `readjson` | `( -- j )` | — | json document handle |
| `readinput` | `( -- q )` | — | query string handle |
| `readtext` | `( -- t )` | — | free-text handle |

**Predicates** (leave input, push a `0|1` flag — the way to cope with limits):

| Call | Stack effect | Note |
|---|---|---|
| `endp` | `( -- 0\|1 )` | 1 iff input exhausted |
| `linehit` | `( h -- h 0\|1 )` | 1 iff line matches predicate |
| `donep` | `( s -- s 0\|1 )` | 1 iff `s >= threshold` |
| `okp` | `( r -- r 0\|1 )` | 1 once flaky op warmed up |

**Transforms** (handle/state → new handle/int):

| Call | Stack effect | Faults | Note |
|---|---|---|---|
| `step` | `( s -- s' )` | — | `s + 1` |
| `getname` | `( j -- v )` | ToolError | extract json "name" field |
| `fetch` | `( q -- doc )` | ToolError | tool: query → document |
| `parse` | `( doc -- v )` | ToolError | tool: document → value |
| `tokenize` | `( t -- [w…] )` | — | split text into word handles |
| `transform` | `( h -- h' )` | — | uppercase the string |
| `tryop` | `( -- r )` | ToolError | flaky tool; retry until `okp` |

**Handles** (move/combine opaque strings without reading bytes):

| Call | Stack effect | Faults | Note |
|---|---|---|---|
| `concat` | `( h1 h2 -- h )` | ToolError | join two string handles |
| `select` | `( [h…] n -- h )` | ToolError | nth handle (0-indexed) |

**Output** (metered against the byte cap):

| Call | Stack effect | Faults | Note |
|---|---|---|---|
| `emit` | `( h -- )` `{output}` | OutputCapExceeded | write string + `\n` |
| `emitint` | `( n -- )` `{output}` | OutputCapExceeded | write decimal `n` + `\n` |

### Grant model

The **grant set** (registry) fixes which names are callable. Calling a name that
is **not granted** faults `NotGranted` and does nothing — no pop, no push, no
effect. A task may grant only a restricted subset ("you may only use `readline`,
`emit`); staying inside that set is part of the task. Assume nothing is granted
unless the task lists it.

### Budgets

Two orthogonal host meters, each charged **before** the effect, atomically (a
refused charge runs nothing, mutates nothing):

- **Per-name call budget** — a capability may be capped at `N` calls. The
  `N+1`th faults `BudgetExhausted` and does not run.
- **Total output-byte cap** — a shared budget across all `{output}` calls. An
  `emit`/`emitint` whose bytes would exceed it faults `OutputCapExceeded` and
  writes nothing.

Unbudgeted names/bytes are unlimited. Bound your calls to stay inside a stated
budget — there is no refund and no retry after the fault.

### Fault codes

`HostCode`: `InputClosed`, `OutputCapExceeded`, `BudgetExhausted`, `ToolError`,
`Timeout`, `NotGranted`.

A **host fault is terminal**: it ends the run with no partial effect, and there
is **no in-MTL catch** — `?`/`|` cannot recover from a fault that already fired.
So you **cope by predicting**: call a predicate capability, read its `0|1` flag,
and branch with `?`/`|` so you never make the call that would over-read or
over-budget. Guard input with `endp`/`linehit`, loops with `donep`, retries with
`okp`.

### String handles

The core has no string type. A string is an **opaque `i64` handle** on the
stack. The core can only **move** a handle — `:` dup, `_` drop, `~` swap, and
list ops — it cannot read, compare, or build the bytes behind it. All string
work happens host-side inside capabilities (`transform`, `concat`, `getname`, …).
`0` is never a valid handle (usable as a sentinel).

### Examples

1. **Echo one line** — `readline emit`: read the first line's handle, write it.
2. **Grep by predicate** — `readlines 0[linehit[emit][_]?](_`: fold over the
   line handles; `linehit` leaves the handle and a flag, `?` emits it or drops
   it.
3. **Drain input safely** — `[endp][][nextline emit][]|`: linrec whose predicate
   `endp` terminates on exhaustion; otherwise `nextline emit` then recurse.
   `endp` guards `nextline` so it never faults `InputClosed`.
4. **Retry a flaky tool** — `tryop[okp][_][_ tryop][]|`: seed one `tryop`, then
   loop while `okp`'s flag is `0`, each pass dropping the old result and calling
   `tryop` again; on success drop the result.
