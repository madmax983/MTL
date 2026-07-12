# single_number — MTL v0.3 notes

Primitive used: Xor (`$`) + LinRec (`|`).

Program: `[>0=][0][][$]|`

Stack effect: `( [xs] -- x )` where every element appears twice except one. Primitives: `|` = LinRec, `$` = Xor.

How it works: `[>0=]` is the empty-list predicate, `[0]` the base (XOR identity), `[]` the descend no-op, and `[$]` XORs each head into the accumulated result on the way up. Pairs cancel under XOR, leaving the unique element.

WALL CLEARED: single_number had no MTL solution in v0.2 (no bitwise primitive). The v0.3 xor primitive `$` resolves the wall.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
