# factorial — MTL v0.2 notes

Program: `[1][*]&`

Primitive: `&` = PrimRec, `( n [I] [C] -- r )`. For input `[n]`:
`[1]` pushes the base quotation (0! = 1) and `[*]` the combine quotation,
then `&` runs the primitive recursion: base case `n <= 0` yields `1`; the
recursive case sees `(n, fact(n-1))` on the stack and `*` multiplies them,
giving `n * fact(n-1)`.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against 6 test vectors (n = 0,1,2,3,5,6; see
bench/validate/tests/corpus.rs test `factorial_v02`).

CONFIDENCE: high — executed on the reference interpreter.
