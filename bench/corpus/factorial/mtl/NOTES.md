# factorial — MTL notes

Program: `1~[^0=[__][[:@*~1-]':!]?]:!`

Idiom: the `[L]:!` self-application (spec §6.2 `: !`) pushes the loop
quotation `L`, `:` dups it, `!` applies one copy — so at loop-body entry `L`
is retained ON TOP of the stack and the body may `:!` it again to recurse or
drop it to terminate. This mirrors the interpreter's `countdown` test in
`crates/mtl-core/tests/interpreter.rs`.

An accumulator carries the running product. Loop state is `[acc, n, L]`
(`L` on top). Setup `1~` turns input `[n]` into `[1, n]` (`acc = 1`), then
`[L]:!` starts the loop.

Body `^0=[__][[:@*~1-]':!]?` on `[acc, n, L]`:

- `^` over — copy `n` (the second-from-top) to the top → `[acc, n, L, n]`
- `0=` — test `n == 0`
- `[__]` base case (`n == 0`): drop `L`, drop `n`, leaving `[acc]` = `n!`
- `[[:@*~1-]':!]` recursive case: `'` dip runs `[:@*~1-]` on `[acc, n]`
  beneath `L`, producing `[acc*n, n-1]`, then restores `L` and `:!` recurses.
  The dipped quote `:@*~1-` maps `[acc, n]` → `[acc*n, n-1]`:
  `:` dup, `@` rot, `*` mul (→ `acc*n`), `~` swap, `1-` decrement `n`.
- `?` if — select base vs recursive branch

Correction note: the original corpus text `[:1<[_1][:1-'!*]?]:!` was an
unvalidated structural sketch and is INCORRECT — its body opens with `:`
(dup), which duplicates the loop-quote `L` (on top), so the following `1<`
compares a Quote against an Int and the interpreter faults `TypeMismatch`
immediately (n=3 → Fault(TypeMismatch)). The corrected program brings the
numeric argument to the top with `^` before testing, carries an explicit
accumulator, and uses `'` dip to keep `L` on top across the recursion.

STATUS: validated — parses with mtl-syntax and executes correctly on the mtl-core interpreter against 6 test vectors (n = 0,1,2,3,5,6; see bench/validate/tests/corpus.rs).

CONFIDENCE: high — executed on the reference interpreter.
