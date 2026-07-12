# climbing_stairs — MTL v0.2 notes

Program: `1 1@[~^+]._`

Stack effect: `( n -- ways )`. Primitive `.` = Times, `( n [Q] -- ... )` runs
Q max(n,0) times.

How it works: this is the iterative Fibonacci recurrence (ways(n) =
ways(n-1) + ways(n-2), ways(0)=ways(1)=1). `1 1` seeds the pair `[a, b] =
[1, 1]` (NOTE the required space: `11` would lex as a single Int). `@` rot
brings n to the top; `[~^+].` runs the step n times, each mapping `[a, b]` to
`[b, a+b]` (`~` swap, `^` over, `+` add). `_` drops the top of the final pair,
leaving ways(n).

STATUS: validated — parses with mtl-syntax and executes correctly on the
mtl-core interpreter against vectors 0->1, 1->1, 2->2, 3->3, 4->5, 5->8 (see
bench/validate/tests/tier2.rs test `climbing_stairs`).

CONFIDENCE: high — executed on the reference interpreter.
