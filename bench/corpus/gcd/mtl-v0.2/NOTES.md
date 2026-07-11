# gcd — MTL v0.2 notes

Program: `[:0=][_][~^%][]|`

Primitive: `|` = LinRec, `( [P] [T] [R1] [R2] -- ... )`. For input `[a, b]`
(b on top), Euclid's algorithm `gcd(a, b) = a if b == 0 else gcd(b, a mod b)`:

- `[:0=]` predicate P: `:` dup b, `0=` test `b == 0` (leaves a flag).
- `[_]` base T: drop the top (`b`, which is 0), leaving `[a]` = gcd.
- `[~^%]` recurse-down R1: `~` swap -> `[b, a]`, `^` over -> `[b, a, b]`,
  `%` mod -> `[b, a mod b]`, the next Euclid pair.
- `[]` unwind R2: no-op; the result bubbles straight back up.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against 6 test vectors ((12,8),(48,36),(17,5),(0,5),
(5,0),(10,10); see bench/validate/tests/corpus.rs test `gcd_v02`).

CONFIDENCE: high — executed on the reference interpreter.
