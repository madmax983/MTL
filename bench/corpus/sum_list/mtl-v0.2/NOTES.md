# sum_list — MTL v0.2 notes

Program: `[>0=][0][][+]|`

Stack effect: `( [xs] -- sum )`. Primitive `|` = LinRec,
`( [P] [T] [R1] [R2] -- ... )`.

How it works: `[>0=]` is the predicate P — `>` uncons the list; on an empty
list uncons pushes only the present-flag `0`, and `0=` tests it, so the
predicate is true exactly when the list is empty. `[0]` is the base T (empty
sum = 0). `[]` R1 is a no-op on the way down (the head is already exposed by
the predicate's uncons and threaded through). `[+]` R2 adds the head to the
recursive sum on the way up.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors [1 2 3]->6, [5]->5, [10 20 30 40]->100,
[]->0, [0]->0 (see bench/validate/tests/tier2.rs test `sum_list`).

CONFIDENCE: high — executed on the reference interpreter.
