# reverse_list — MTL v0.3 notes

Primitive used: Fold (`(`).

Program: `[][~;](`

Stack effect: `( [xs] -- [reversed] )`; the OUTPUT is a quotation. Primitive `(` = Fold.

How it works: `[]` seeds an empty accumulator list. The combinator `[~;]` conses each element onto the FRONT of the accumulator (`~` swap acc/elem, `;` cons), which reverses the order left-to-right. Empty list -> empty list.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
