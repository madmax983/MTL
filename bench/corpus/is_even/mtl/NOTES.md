# is_even — MTL notes

Program: `2%0=`

Reading (stack top on the right), input `n`:

1. `2` — push 2         → `n 2`
2. `%` — modulo         → `(n%2)`
3. `0` — push 0         → `(n%2) 0`
4. `=` — equal          → `1` if `n%2 == 0` else `0`

Note: MTL yields Int 1/0; Python idiomatic yields bool — semantically
equivalent under MTL's integer-boolean convention.

STATUS: validated — parses with mtl-syntax and executes correctly on the mtl-core interpreter against 5 test vectors (see bench/validate/tests/corpus.rs).

CONFIDENCE: high — straight-line stack code, no recursion.
