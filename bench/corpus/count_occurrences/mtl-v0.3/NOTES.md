# count_occurrences — MTL v0.3 notes

Primitive used: Fold (`(`).

Program: `[=+];0~(`

Stack effect: `( [xs] x -- count )` (x on top). Primitive `(` = Fold.

How it works: `[=+]` compares each element to the carried target and adds the boolean (1 on match, 0 otherwise) to the running count. `;0~` seeds the count at 0 carried with x, `(` folds. Result is the number of occurrences.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
