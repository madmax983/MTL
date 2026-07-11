# fib — MTL v0.2 notes

Program: `0 1@[~^+]._`

Primitive: `.` = Times, `( n [Q] -- ... )` runs Q max(n,0) times. For input
`[n]` this is the iterative Fibonacci with a (a, b) accumulator pair:

- `0 1` seeds the pair `[a, b] = [0, 1]` on top of `n`. NOTE the SPACE
  between `0` and `1`: without it, `01` lexes as a single Int(1) and the
  `0` seed is lost. The spaced form is the corrected seed.
- `@` rot brings `n` to the top -> `[a, b, n]`.
- `[~^+].` runs the step quotation n times. Each step maps `[a, b]` to
  `[b, a+b]`: `~` swap -> `[b, a]`, `^` over -> `[b, a, b]`, `+` add ->
  `[b, a+b]`.
- After n steps the pair holds `[fib(n), fib(n+1)]`; `_` drops the top,
  leaving `[fib(n)]`.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against 6 test vectors (n = 0,1,2,3,5,10; see
bench/validate/tests/corpus.rs test `fib`).

CONFIDENCE: high — executed on the reference interpreter.
