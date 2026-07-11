# gcd — MTL notes

Program: `[^0=[__][[~^%]':!]?]:!`

Idiom: the `[L]:!` self-application (spec §6.2 `: !`) pushes the loop
quotation `L`, `:` dups it, `!` applies one copy — so at loop-body entry `L`
is retained ON TOP of the stack and the body may `:!` it again to recurse or
drop it to terminate. This mirrors the interpreter's `countdown` test in
`crates/mtl-core/tests/interpreter.rs`.

Euclid: `gcd(a, b) = a` if `b == 0`, else `gcd(b, a mod b)`. Loop state is
`[a, b, L]` (`L` on top); input `[a, b]` (`b` on top) needs no setup, so the
program is just `[L]:!`.

Body `^0=[__][[~^%]':!]?` on `[a, b, L]`:

- `^` over — copy `b` (the second-from-top) to the top → `[a, b, L, b]`
- `0=` — test `b == 0`
- `[__]` base case (`b == 0`): drop `L`, drop `b`, leaving `[a]` = `gcd`
- `[[~^%]':!]` recursive case: `'` dip runs `[~^%]` on `[a, b]` beneath `L`,
  producing `[b, a mod b]`, then restores `L` and `:!` recurses.
  The dipped quote `~^%` maps `[a, b]` → `[b, a mod b]`:
  `~` swap → `[b, a]`, `^` over → `[b, a, b]`, `%` mod → `[b, a mod b]`.
- `?` if — select base vs recursive branch

Correction note: the original corpus text `[:0=[_][~^%'!]?]:!` was an
unvalidated structural sketch and is INCORRECT — its body opens with `:`
(dup), which duplicates the loop-quote `L` (on top), so the following `0=`
compares a Quote against an Int and the interpreter faults `TypeMismatch`
immediately (gcd(12,8) → Fault(TypeMismatch)). It also had only a single `_`
in the base case (dropping just `L`, leaving `[a, b]` rather than `[a]`), and
performed the recursion inside the dipped quote rather than restoring `L`
first. The corrected program brings `b` to the top with `^` before testing,
computes the `[b, a mod b]` pair under `L` via `'` dip, and recurses with
`:!` on the restored `L`.

STATUS: validated — parses with mtl-syntax and executes correctly on the mtl-core interpreter against 6 test vectors ((12,8),(48,36),(17,5),(0,5),(5,0),(10,10); see bench/validate/tests/corpus.rs).

CONFIDENCE: high — executed on the reference interpreter.
