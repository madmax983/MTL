# contains — MTL v0.2 notes

Program: `0~@[>[;0][[]1]?][__][>_[^=@~+0~<~]'][]|`

Stack effect: `( [xs] x -- 0|1 )` (x on top). Primitive `|` = LinRec.

How it works: `0~@` reshapes the input `list x` into the loop state
`found x list` (push found=0, swap, rot). The non-destructive null test
`[>[;0][[]1]?]` walks the list; base T `[__]` drops the empty list and x,
leaving found. R1 `[>_[^=@~+0~<~]']` re-uncons the rebuilt list, then the body
`^=@~+0~<~` compares the head to the carried x (`^=`, using Over to reach x at
fixed depth), ORs the boolean into found (`@~+0~<` synthesizes OR as
`0 < found+eq`, valid because flags are non-negative), and restores x on top
for the next step; `'` Dip runs the body over the tail. R2 `[]` no-op.

This is a "solved-but-ugly" exemplar: 39 glyphs, deep stack juggling
(rot+over+swap to thread the carried scalar past the accumulator every step).

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors ([1 2 3],2)->1, ([1 2 3],5)->0,
([],5)->0, ([7],7)->1, ([4 4 4],4)->1 (see bench/validate/tests/tier2.rs test
`contains`).

CONFIDENCE: high — executed on the reference interpreter.
