# length_list — MTL v0.3 notes

Primitive used: Fold (`(`).

Program: `0[_1+](`

Stack effect: `( [xs] -- len )`. Primitive `(` = Fold.

How it works: `0` seeds the count. The combinator `[_1+]` drops the element (`_`) and adds 1 to the accumulator, so every element bumps the count by one. Empty list -> 0.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
