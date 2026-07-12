# reverse_list — MTL v0.2 notes

Program: `[]~[>[;0][[]1]?][_][>_[~;]'][]|`

Stack effect: `( [xs] -- [reversed] )` — the OUTPUT is a quotation (list).
Primitive `|` = LinRec.

How it works: `[]~` seeds an empty accumulator list beneath the input. The
non-destructive null test `[>[;0][[]1]?]` rebuilds the list each step, so R1
re-uncons with `>_`. R1 `[>_[~;]']` conses the current head onto the front of
the accumulator (`~` swap, `;` cons), dipped over the tail with `'` so the
recursion walks the rest; because each head is prepended to the accumulator,
the result comes out reversed. Base `[_]` drops the emptied input; `[]` R2 is
a no-op.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors [1 2 3]->[3 2 1], []->[], [7]->[7],
[1 2 3 4]->[4 3 2 1] (see bench/validate/tests/tier2.rs test `reverse_list`).

CONFIDENCE: high — executed on the reference interpreter.
