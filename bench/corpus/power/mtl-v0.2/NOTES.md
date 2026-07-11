# power — MTL v0.2 notes

Program: `1~[^*].~_`

Primitive: `.` = Times, `( n [Q] -- ... )` runs Q max(n,0) times. For input
`[b, e]` (e on top), iterative exponentiation with an accumulator:

- `1` pushes the accumulator (b^0 = 1) -> `[b, e, 1]`.
- `~` swap -> `[b, 1, e]` puts the exponent e on top for Times.
- `[^*].` runs the step quotation e times. Each step maps `[b, acc]` to
  `[b, acc*b]`: `^` over -> `[b, acc, b]`, `*` mul -> `[b, acc*b]`.
- After e steps the stack is `[b, b^e]`; `~` swap -> `[b^e, b]`, `_` drop
  -> `[b^e]`.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against 4 test vectors ((2,0),(2,3),(3,4),(5,2); see
bench/validate/tests/corpus.rs test `power`).

CONFIDENCE: high — executed on the reference interpreter.
