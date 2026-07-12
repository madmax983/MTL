# product_list — MTL v0.2 notes

Program: `[>0=][1][][*]|`

Stack effect: `( [xs] -- product )`. Primitive `|` = LinRec.

How it works: identical shape to sum_list but with the multiplicative
identity and operator. Predicate `[>0=]` (empty test), base `[1]` (empty
product = 1), R1 `[]` no-op, R2 `[*]` multiplies the head into the recursive
product on the way up.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors [1 2 3 4]->24, []->1, [5]->5,
[2 3 0 4]->0 (see bench/validate/tests/tier2.rs test `product_list`).

CONFIDENCE: high — executed on the reference interpreter.
