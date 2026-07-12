# min_list — MTL v0.2 notes

Program: `>_[>[;0][[]1]?][_][>_[^^<[_][~_]?]'][]|`

Stack effect: `( [xs] -- min )`, non-empty lists only. Primitive `|` = LinRec.

How it works: identical structure to max_list, with the If branches of the
comparison swapped so the SMALLER element is kept. `>_` seeds the running min
from the first element; the non-destructive null test `[>[;0][[]1]?]` rebuilds
the list, so R1 re-uncons with `>_`; `^^<` compares head vs running min and
`[_][~_]?` keeps the min, dipped over the tail with `'`.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors [3 1 2]->1, [5]->5, [9 4 9 2]->2 (see
bench/validate/tests/tier2.rs test `min_list`).

CONFIDENCE: high — executed on the reference interpreter.
