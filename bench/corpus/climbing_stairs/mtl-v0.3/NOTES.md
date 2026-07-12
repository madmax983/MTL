# climbing_stairs — MTL v0.3 notes

Primitive used: none (scalar task).

Program: `1 1@[~^+]._`

Stack effect: `( n -- ways )`. SCALAR int input. Note the SPACE between the two `1` seeds.

Unchanged from v0.2 — no fold/xor applies (this is scalar iterative DP, not a list task). Carried forward verbatim so the v0.3 tier and v0.2 tier stay comparable.

STATUS: interpreter-validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against the task's I/O vectors (see bench/validate/tests/tier2_v03.rs).

CONFIDENCE: high — executed on the reference interpreter.
