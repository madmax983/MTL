# max_list — MTL v0.2 notes

Program: `>_[>[;0][[]1]?][_][>_[^^<[~_][_]?]'][]|`

Stack effect: `( [xs] -- max )`, non-empty lists only. Primitive `|` = LinRec.

How it works: the leading `>_` seeds the fold by unconsing the first element
as the running max and dropping the present-flag. The predicate
`[>[;0][[]1]?]` is the NON-DESTRUCTIVE null test (see TIER2_NOTES): it uses
`>` uncons then `?` If to REBUILD the list, leaving `list 0` (non-empty) or
`[] 1` (empty). Because P rebuilds the list, the recursive branch R1 must
re-uncons with `>_` before touching the head. R1 `[>_[^^<[~_][_]?]']` compares
the head against the running max with `^^<` (over, over, less-than) and keeps
the larger via `?` If, dipped over the tail with `'` (Dip) so the recursion
continues on the rest. Base `[_]` drops the sentinel; `[]` R2 is a no-op.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors [3 1 2]->3, [5]->5, [1 9 4 9 2]->9,
[10 20 5]->20 (see bench/validate/tests/tier2.rs test `max_list`).

CONFIDENCE: high — executed on the reference interpreter.
