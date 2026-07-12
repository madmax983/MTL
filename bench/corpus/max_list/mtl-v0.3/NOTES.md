# max_list — MTL v0.3 notes

Primitive used: Fold (`(`).

Program: `>_~[^^<[~_][_]?](`

Stack effect: `( [xs] -- max )`, non-empty only. Primitive `(` = Fold.

How it works: `>_~` unconses the first element and seeds the accumulator with it (`>` uncons, `_` drop the present-flag, `~` swap so the seed sits under the tail). The combinator `[^^<[~_][_]?]` compares the carried max (`^^` over/over to copy both, `<` compare) and keeps the larger (`?` If). `(` folds the rest.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
