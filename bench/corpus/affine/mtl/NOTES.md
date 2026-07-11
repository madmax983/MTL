# affine — MTL notes

Program: `3*7+`

Reading (stack top on the right), input `n` already on the stack:

1. `3` — push 3         → `n 3`
2. `*` — multiply       → `(n*3)`
3. `7` — push 7         → `(n*3) 7`
4. `+` — add            → `(n*3 + 7)`

Straight-line stack code, no recursion.

STATUS: validated — parses with mtl-syntax and executes correctly on the mtl-core interpreter against 4 test vectors (see bench/validate/tests/corpus.rs).

CONFIDENCE: high — straight-line stack code, no recursion.
