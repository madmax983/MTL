# length_list — MTL v0.2 notes

Program: `[>0=][0][][~_1+]|`

Stack effect: `( [xs] -- n )`. Primitive `|` = LinRec.

How it works: predicate `[>0=]` is the same empty-list test as sum_list
(uncons, test the present-flag). Base `[0]` (empty length = 0). R1 `[]` is a
no-op descending. R2 `[~_1+]` drops the head value and adds 1 to the recursive
count: `~` swap, `_` drop the head, `1+` increment the tally returned from
below.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors [1 2 3]->3, []->0, [7 7 7 7 7]->5 (see
bench/validate/tests/tier2.rs test `length_list`).

CONFIDENCE: high — executed on the reference interpreter.
