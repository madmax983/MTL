# grep_filter

**Intent:** over the input lines, emit only those a host predicate accepts
(grep-like filter driven by a tool call).

## I/O contract
- **Input:** N text lines, delivered as a `Quote` of line handles `[h0 h1 …]`.
- **Output:** the matching lines emitted, in order.
- **Capabilities (stack effects):**
  - `read-lines : ( -- [h...] )` — host returns a Quote of line handles.
  - `line-hit : ( h -- h 0|1 )` — host predicate; leaves the handle and pushes a
    match flag (the "does this line match" tool). The pattern is host-owned.
  - `emit : ( h -- )` — write a line. Effect `{output}`.

## Python sketch (idiomatic)
```python
def solve():
    for line in read_lines():
        if line_matches(line):
            emit(line)
```

## MTL sketch (design-stage, hand-traced)
```
read-lines 0[line-hit[emit][_]?](_
```
`read-lines` → `[h0 h1 …]`; seed a dummy acc `0`; `fold` over the line list with
`C = [line-hit[emit][_]?]` : `(acc h -- acc)` — `line-hit` leaves `h flag`, `?`
runs `[emit]` (consume+write) or `[_]` (drop) — acc `0` untouched. Trailing `_`
drops the acc. Filtering is control flow; the *matching* is a capability. ✓

## Tokens (o200k / cl100k)
| | Python | MTL |
|---|---:|---:|
| grep_filter | 20 / 20 | 12 / 12 |

## Needs in-core strings?
**No.** Line handles are opaque; the substring/regex match is the `line-hit`
capability. The core only routes handles and branches on an `Int` flag.
