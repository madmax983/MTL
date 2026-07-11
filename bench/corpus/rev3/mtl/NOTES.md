# rev3 — MTL notes

Program: `~@`

Reading (stack top on the right), input `a b c`:

1. `~` — swap top two   → `a c b`
2. `@` — rot            → `c b a`

Straight-line stack code, no recursion.

STATUS: unvalidated — MTL interpreter (Track B) has not landed; this solution's correctness is a best-effort structural claim, not executed. Token count is exact regardless of correctness.

CONFIDENCE: high — straight-line stack code, no recursion.
