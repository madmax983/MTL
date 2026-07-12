# min_list — MTL v0.3 notes

Primitive used: Fold (`(`).

Program: `>_~[^^<[_][~_]?](`

Stack effect: `( [xs] -- min )`, non-empty only. Primitive `(` = Fold.

How it works: same shape as max_list but the If branches are swapped so the smaller element is kept. `>_~` seeds the accumulator with the head; `[^^<[_][~_]?]` keeps the minimum; `(` folds the rest.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
