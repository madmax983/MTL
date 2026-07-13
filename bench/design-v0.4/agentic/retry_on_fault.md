# retry_on_fault

**Intent:** call a flaky tool and retry up to a fixed budget until it succeeds
(retry-on-fault loop). Exercises a bounded loop + a per-capability call budget —
the natural home for the metering annotation in `glyphs.md`.

## I/O contract
- **Input:** none (or an opaque request handle); a retry budget `N`.
- **Output:** the successful result handle, or a failure sentinel if the budget
  is exhausted.
- **Capabilities (stack effects):**
  - `try-op : ( -- r )` — attempt the operation; returns a result handle. On a
    hard failure the host signals `HostFault` (§8.2), which the driver converts
    into the retry path (design note below).
  - `ok? : ( r -- r 0|1 )` — did the attempt succeed? leaves `r`, pushes a flag.

## Python sketch (idiomatic)
```python
def solve():
    for _ in range(3):
        r, ok = try_op()
        if ok:
            return r
    return None
```

## MTL sketch (design-stage, representative — ±band)
```
3[try-op ok?][_][][1-]|
```
Carry the remaining budget `n = 3`; `linrec`: `P = [try-op ok?]` (attempt + test),
`T = [_]` (success: drop the flag, keep `r`), `R1 = []`, `R2 = [1-]` (decrement
budget and recurse). Loops until success or `n` hits 0. **Design note:** a genuine
retry-*on-fault* needs the §8.2 `HostFault → Resume` contract so a faulting
`try-op` returns control with `ok=0` rather than aborting the VM; that host
contract is a v0.4 obligation, not yet specified. Token count is a representative
estimate (±4), not min-golfed. ✓ value-level trace for `N=3`, success on attempt 2.

## Tokens (o200k / cl100k)
| | Python | MTL |
|---|---:|---:|
| retry_on_fault | 30 / 30 | 12 / 12 |

## Needs in-core strings?
**No.** The loop is `Int` budget arithmetic + branch on a flag; the result is an
opaque handle. This task motivates **metering** (a call budget), not `Str`.
