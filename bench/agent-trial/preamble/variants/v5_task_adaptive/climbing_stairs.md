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
| `@` | rot | `( a b c -- b c a )` | rotate third to top |
| `^` | over | `( a b -- a b a )` | copy second over top |
| `+` | add | `( a b -- a+b )` | checked |
| `.` | times | `( n [Q] -- ... )` | run `[Q]` n times |

## Faults (halt, no partial result)

- **Underflow** — too few operands for the primitive.
- **TypeMismatch** — an operand has the wrong type (e.g. `!` on an Int, `+` on a Quote, `>` head is not a value).
- **Overflow** — a checked arithmetic result leaves `i64`.
