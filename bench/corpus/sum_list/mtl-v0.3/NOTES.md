# sum_list — MTL v0.3 notes

Primitive used: Fold (`(`).

Program: `0[+](`

Stack effect: `( [xs] -- sum )`. Primitive `(` = Fold (LEFT fold).

How it works: `0` seeds the accumulator, `[+]` is the combinator `C:(acc w -- acc')` run per element left-to-right, and `(` folds the list. Empty list folds to the seed `0`.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
