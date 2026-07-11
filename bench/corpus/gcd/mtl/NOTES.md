# gcd — MTL notes

Program: `[:0=[_][~^%'!]?]:!`

Intended reading: the `[...]:!` Y-idiom dup-applies the body quotation,
leaving a self-copy available for re-entry. The body:

- `:` dup b
- `0=` test `b == 0`
- `[_]` base-case quotation: drop b, leaving result `a`
- `[~^%'!]` recursive quotation: rearrange to `(b, a%b)` via swap/over/mod,
  then `'` dip / `!` self-apply to recurse
- `?` if — select base vs recursive branch

STATUS: unvalidated — MTL interpreter (Track B) has not landed; this solution's correctness is a best-effort structural claim, not executed. Token count is exact regardless of correctness.

CONFIDENCE: structural sketch — the `:!` self-application ordering is not yet interpreter-checked; treat its token count as indicative.
