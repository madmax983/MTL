# MTL — task preamble

MTL is a stack-based, point-free language: words run left to right against an
operand stack (top at the **right**). `[q]` is a quote (an unevaluated program,
also used as a list like `[1 2 3]`). Ints are i64; `0` is false, nonzero true;
comparisons yield `1`/`0`. A leading `-` is always `Sub`. Inputs are already on
the stack; leave the result on the stack.

## Primitives (stack effect: top at right; `[q]` a quote)

| Glyph | Name | Stack effect | Meaning |
|---|---|---|---|
| `[` `]` | quote | `( -- [q] )` | delimiters (not a word) |
| `_` | drop | `( a -- )` | discard top |
| `~` | swap | `( a b -- b a )` | swap top two |
| `;` | cons | `( v [q] -- [v q] )` | prepend value into quote |
| `'` | dip | `( a [q] -- ... a )` | run `q` with `a` temporarily removed, then restore `a` on top |
| `?` | if | `( c [t] [f] -- ... )` | run `[t]` if `c≠0` else `[f]` |
| `\|` | linrec | `( [P][T][R1][R2] -- ... )` | linear/tail recursion |
| `>` | uncons | `( [w …] -- w [ … ] 1 )` or `( [] -- 0 )` | split a quote |

## Faults (halt, no partial result)

- **Underflow** — too few operands for the primitive.
- **TypeMismatch** — an operand has the wrong type (e.g. `!` on an Int, `+` on a Quote, `>` head is not a value).
