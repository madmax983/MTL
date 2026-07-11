# rev3 — MTL notes

Program: `~@`

Reading (stack top on the right), input `a b c`:

1. `~` — swap top two   → `a c b`
2. `@` — rot            → `c b a`

Straight-line stack code, no recursion.

STATUS: validated — parses with mtl-syntax and executes correctly on the mtl-core interpreter against 2 test vectors (see bench/validate/tests/corpus.rs).

CONFIDENCE: high — straight-line stack code, no recursion.
