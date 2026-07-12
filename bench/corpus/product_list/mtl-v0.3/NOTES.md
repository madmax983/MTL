# product_list — MTL v0.3 notes

Primitive used: Fold (`(`).

Program: `1[*](`

Stack effect: `( [xs] -- product )`. Primitive `(` = Fold.

How it works: `1` seeds the accumulator (multiplicative identity), `[*]` multiplies each element into the accumulator left-to-right. Empty list -> 1.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
