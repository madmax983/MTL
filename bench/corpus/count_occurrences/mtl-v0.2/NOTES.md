# count_occurrences — MTL v0.2 notes

Program: `0~@[>[;0][[]1]?][__][>_[^=@~+~]'][]|`

Stack effect: `( [xs] x -- count )` (x on top). Primitive `|` = LinRec.

How it works: identical state layout to `contains` (`0~@` -> `count x list`),
but the in-loop body `^=@~+~` KEEPS the running sum instead of OR-clamping it:
`^=` compares head to the carried x, `@~+` adds the 0/1 result into the
running count, `~` restores x on top. The non-destructive null test walks the
list; base `[__]` drops the empty list and x, leaving the count.

This is a "solved-but-ugly" exemplar: 36 glyphs, same carried-scalar juggling
as contains.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors ([1 2 2 3],2)->2, ([1 2 3],5)->0,
([],5)->0, ([4 4 4],4)->3, ([5 5 5 5],5)->4 (see
bench/validate/tests/tier2.rs test `count_occurrences`).

CONFIDENCE: high — executed on the reference interpreter.
