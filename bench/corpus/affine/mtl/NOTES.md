# affine — MTL notes

Program: `3*7+`

Reading (stack top on the right), input `n` already on the stack:

1. `3` — push 3         → `n 3`
2. `*` — multiply       → `(n*3)`
3. `7` — push 7         → `(n*3) 7`
4. `+` — add            → `(n*3 + 7)`

Straight-line stack code, no recursion.

STATUS: unvalidated — MTL interpreter (Track B) has not landed; this solution's correctness is a best-effort structural claim, not executed. Token count is exact regardless of correctness.

CONFIDENCE: high — straight-line stack code, no recursion.
