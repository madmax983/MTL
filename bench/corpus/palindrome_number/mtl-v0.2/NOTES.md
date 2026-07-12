# palindrome_number — MTL v0.2 notes

Program: `0^[:1<][_=][:10%@10*+~10/][]|`

Stack effect: `( n -- 0|1 )`. Primitive `|` = LinRec.

How it works: `0^` seeds a reversed-digit accumulator (0) and Over-copies n so
the original survives for the final compare. The LinRec reverses n's decimal
digits: predicate `[:1<]` stops when the remaining number is < 1 (i.e. 0);
base `[_=]` drops the exhausted number and tests the rebuilt reverse against
the original with `=`, yielding 1/0. R1 `[:10%@10*+~10/]` peels the low digit
(`:10%`), folds it into the running reverse (`@10*+`), and divides the number
by 10 (`~10/`). R2 `[]` is a no-op.

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors 121->1, 123->0, 7->1, 1221->1, 10->0,
0->1 (see bench/validate/tests/tier2.rs test `palindrome_number`).

CONFIDENCE: high — executed on the reference interpreter.
