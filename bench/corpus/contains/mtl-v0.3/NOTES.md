# contains — MTL v0.3 notes

Primitive used: Fold (`(`).

Program: `[=+0~<];0~(`

Stack effect: `( [xs] x -- 0|1 )` (x on top). Primitive `(` = Fold.

How it works: `[=+0~<]` is the combinator: for each element it tests equality with the carried target and ORs the boolean into the found-flag (`=+0~<` is the non-negative OR idiom). `;0~` builds the fold's initial state (seed found-flag 0 carried with x) and `(` folds. Result is 1 if x occurs, else 0.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
