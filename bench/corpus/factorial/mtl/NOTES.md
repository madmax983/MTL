# factorial — MTL notes

Program: `[:1<[_1][:1-'!*]?]:!`

Intended reading: the `[...]:!` Y-idiom dup-applies the body quotation,
leaving a self-copy available for re-entry. The body:

- `:` dup n
- `1<` test `n < 1`
- `[_1]` base-case quotation: drop n, push `1`
- `[:1-'!*]` recursive quotation: dup n, compute `n-1`, `'` dip the
  recursive self-apply `!` under the saved `n`, then `*` to form
  `n * fact(n-1)`
- `?` if — select base vs recursive branch

STATUS: unvalidated — MTL interpreter (Track B) has not landed; this solution's correctness is a best-effort structural claim, not executed. Token count is exact regardless of correctness.

CONFIDENCE: structural sketch — the `:!` self-application ordering is not yet interpreter-checked; treat its token count as indicative.
