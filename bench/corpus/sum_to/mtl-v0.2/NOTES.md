# sum_to — MTL v0.2 notes

Program: `[0][+]&`

Primitive: `&` = PrimRec, `( n [I] [C] -- r )`. For input `[n]`:
`[0]` pushes the base quotation (sum over the empty range = 0) and `[+]`
the combine quotation, then `&` runs the primitive recursion: base case
`n <= 0` yields `0`; the recursive case sees `(n, sum(n-1))` and `+` adds
them, giving `n + sum(0..n-1) = 0 + 1 + ... + n`.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against 4 test vectors (n = 0,1,3,10; see
bench/validate/tests/corpus.rs test `sum_to`).

CONFIDENCE: high — executed on the reference interpreter.
