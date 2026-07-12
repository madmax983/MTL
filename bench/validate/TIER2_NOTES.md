# Tier-2 corpus validation notes (branch tier2-corpus)

Interpreter is ground truth. Every solution below was validated by running all
its I/O vectors through `mtlrun` (mtl-syntax parse -> conv_program -> mtl-core
interp::run, FUEL=100_000).

## Runner

`bench/validate/src/bin/mtlrun.rs`. Build + invoke:

    cargo build --bin mtlrun -p mtl-bench-validate
    <target>/debug/mtlrun '<program string>'      # argv, or
    printf '%s' '<program string>' | <target>/debug/mtlrun   # stdin (use for the dip glyph ')

Input is PREPENDED to the solution as literals, exactly like tests/corpus.rs,
e.g. factorial(5): `5[1][*]&` -> HALT: 120.

### Rendering (for wiring the gate test)
- Int: decimal (`120`).
- Quotation: `[a b c]`, nested recursively (`[1 [2 3] 4]`), empty `[]`.
- Full final stack printed bottom..top, space-separated. Empty stack: `<empty>`.
- `uncons [1 2 3]` -> `1 [2 3] 1` (head, tail-quote, present-flag).
- Faults print `FAULT: <kind>` + stack + next continuation; fuel prints state.

## The linrec null-test idiom (shared by tasks 4,5,6,contains,count)
P = `>[;0][[]1]?` is a NON-destructive null test that rebuilds the list:
- non-empty: `>` -> `w [tail] 1`, If(true) runs `;0` -> cons w back onto tail,
  push 0  =>  leaves `list 0` (rebuilt list + false).
- empty: `>` -> `0`, If(false) runs `[]1` -> push empty quote, push 1
  =>  leaves `[] 1` (empty list + true).
Because P rebuilds the list, the R1 (recursive) branch must RE-uncons it with
`>_` before working on the head. That was the bug in the max/min/reverse
candidates (see incident log).

## Validated solutions (all vectors PASS)

1. sum_list           `[>0=][0][][+]|`                            (14)  UNCHANGED
2. length_list        `[>0=][0][][~_1+]|`                         (17)  UNCHANGED
3. product_list       `[>0=][1][][*]|`                            (14)  UNCHANGED
4. max_list           `>_[>[;0][[]1]?][_][>_[^^<[~_][_]?]'][]|`   (39)  FIXED (R1 `_`->`>_`)
5. min_list           `>_[>[;0][[]1]?][_][>_[^^<[_][~_]?]'][]|`   (39)  FIXED (R1 `_`->`>_`)
6. reverse_list       `[]~[>[;0][[]1]?][_][>_[~;]'][]|`           (31)  FIXED (R1 `_`->`>_`)
7. palindrome_number  `0^[:1<][_=][:10%@10*+~10/][]|`             (29)  UNCHANGED
8. climbing_stairs    `1 1@[~^+]._`                               (11)  UNCHANGED
9. contains           `0~@[>[;0][[]1]?][__][>_[^=@~+0~<~]'][]|`   (39)  NEW (designed here)
10. count_occurrences `0~@[>[;0][[]1]?][__][>_[^=@~+~]'][]|`      (36)  NEW (designed here)

Char lengths (incl. the required space in climbing_stairs `1 1`) in parens.

### contains / count_occurrences design (state `found x list`, bottom->top)
- setup `0~@`: input `list x` -> push 0 -> `list x 0` -> `~` -> `list 0 x` ->
  `@`(rot) -> `0 x list`  == `found=0, x, list`.
- P = null test (above), leaves `found x list bool`; linrec If consumes bool.
- T (null) = `__`: drop the empty list, drop x, leaving `found`.
- R1 (non-null) = `>_[BODY]'`: uncons list (`>_` drops flag) -> `found x head [tail]`,
  dip BODY over `[tail]` operating on `found x head`, leaving `found' x`; dip
  restores tail -> `found' x tail` for the recursion. R2 empty.
  - BODY contains: `^=@~+0~<~`
    `^` copy x -> `found x head x`; `=` head==x=b -> `found x b`;
    `@`(rot) -> `x b found`; `~` -> `x found b`; `+` -> `x found+b`;
    `0~<` -> `x (0<sum)` (OR, keeps 0/1); `~` -> `or x`.
  - BODY count: `^=@~+~` (same but keep the running sum instead of OR-clamping).
These are the "solved-but-ugly" exemplars: high glyph count, deep juggling
(rot+over+swap to thread the carried scalar x past the accumulator each step).
Both validated first try, no interpreter iterations needed once state layout was
chosen. Key enabler: x sits at fixed depth 2 below the list top, reachable by a
single `^` (over) after uncons because the loop restores that shape every step.

## Confirmed WALLS (structural, from the 21-prim set — GLYPHS table + interp.rs)
- single_number: XOR-reduce is canonical, but there is NO bitwise primitive.
  Glyphs `^`=Over, `&`=PrimRec, `|`=LinRec (verified: `5 3^` -> `5 3 5`, i.e.
  Over, not xor). No AND/OR/XOR/shift anywhere in the 21 prims. Inexpressible
  canonically -> missing bitwise ops.
- two_sum: returning indices needs enumerate / random access / an associative
  map. Values are only Int|Quote; the only list op is sequential `>` uncons.
  No positional index, no zip-with-index -> inexpressible.
- binary_search: quotation is a cons-list; the only deconstructor is `>` (head +
  tail), strictly sequential, no O(1) indexing -> real binary search is
  impossible; only a linear scan is expressible.

## Stack-juggling incident log
1. max/min/reverse candidates R1 began with `_` (Drop). Symptom: multi-element
   inputs FAULT Underflow with `next: [Swap Cons ...]` (single-element cases
   spuriously passed because they never entered the recursive branch). Cause: P
   rebuilds the list, so at R1 entry the stack is `...acc list`; the candidate's
   `_` dropped the whole list, then `[..]'` dipped over the now-exposed acc and
   ran the head-processing on an empty/short stack. Fix: replace leading `_`
   with `>_` (uncons the rebuilt list, drop the present-flag) so the head is
   actually exposed. One-glyph fix, identical across all three tasks.
2. contains/count setup: naive `~0~` / `list x 0 @` both yield `x 0 list`
   (accumulator in the MIDDLE), which makes the in-loop compare need to reach x
   past found. Chose state `found x list` instead so that after `>_` the head
   lands directly ABOVE x (adjacent), compare = single `^=`. Setup reversal of
   the 3-deep stack: `0~@` (push 0, swap, rot) -> `found x list`. No fault
   incident — caught at design time by reasoning about which layout keeps x at a
   fixed reachable depth.
3. OR without `>`: needed `a OR b` for 0/1 flags with no greater-than prim. Used
   `+0~<` = (0 < a+b). Works because flags are non-negative so `0<sum` == `sum!=0`.
