# v5 task-adaptive — per-task primitive selection


Each per-task preamble contains a shared 4-line grammar/value-model header, the
glyph-table rows for ONLY the primitives its reference solution uses, and the
fault kinds reachable by those primitives. Rows are taken verbatim from
`docs/mtl-quickref.md` (the language is held fixed).

Source of truth: `bench/agent-trial/reference_mtl/<task>.mtl`.

| Task | Reference solution | Primitives selected | Faults |
|---|---|---|---|
| `affine` | `3*7+` | add, mul | Underflow, TypeMismatch, Overflow |
| `rev3` | `~@` | swap, rot | Underflow, TypeMismatch |
| `is_even` | `2%0=` | mod, eq | Underflow, TypeMismatch, DivByZero |
| `factorial` | `[1][*]&` | quote, mul, primrec | Underflow, TypeMismatch, Overflow |
| `gcd` | `[:0=][_][~^%][]\|` | quote, dup, drop, swap, over, mod, eq, linrec | Underflow, TypeMismatch, DivByZero |
| `sum_list` | `[>0=][0][][+]\|` | quote, add, eq, linrec, uncons | Underflow, TypeMismatch, Overflow |
| `reverse_list` | `[]~[>[;0][[]1]?][_][>_[~;]'][]\|` | quote, drop, swap, cons, dip, if, linrec, uncons | Underflow, TypeMismatch |
| `palindrome_number` | `0^[:1<][_=][:10%@10*+~10/][]\|` | quote, dup, drop, swap, rot, over, add, mul, div, mod, eq, lt, linrec | Underflow, TypeMismatch, DivByZero, Overflow |
| `contains` | `0~@[>[;0][[]1]?][__][>_[^=@~+0~<~]'][]\|` | quote, drop, swap, rot, over, cons, dip, add, eq, lt, if, linrec, uncons | Underflow, TypeMismatch, Overflow |
| `climbing_stairs` | `1 1@[~^+]._` | quote, drop, swap, rot, over, add, times | Underflow, TypeMismatch, Overflow |

Note: the `[` `]` quote row is included whenever the reference solution uses a
quotation. Literals (digits) need no glyph row. Fault selection: Underflow and
TypeMismatch are included for every task (ubiquitous to stack ops); DivByZero
only where `/`/`%` appear; Overflow only where checked arithmetic (`+ - *`) appears.
