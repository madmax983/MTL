# palindrome_number — MTL v0.3 notes

Primitive used: none (scalar task).

Program: `0^[:1<][_=][:10%@10*+~10/][]|`

Stack effect: `( n -- 0|1 )`. SCALAR int input.

Unchanged from v0.2 — no fold/xor applies (this is a numeric digit-reversal task on a scalar, not a list task). Carried forward verbatim so the v0.3 tier and v0.2 tier stay comparable.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
