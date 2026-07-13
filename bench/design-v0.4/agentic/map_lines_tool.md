# map_lines_tool

**Intent:** for each input line, call a transform tool and emit the result
(map-over-input-lines-calling-a-tool).

## I/O contract
- **Input:** N text lines as a `Quote` of line handles `[h0 h1 …]`.
- **Output:** the transformed lines emitted, in order.
- **Capabilities (stack effects):**
  - `read-lines : ( -- [h...] )` — host returns a Quote of line handles.
  - `transform : ( h -- h' )` — tool: map one line handle to a new handle.
  - `emit : ( h' -- )` — write a line. Effect `{output}`.

## Python sketch (idiomatic)
```python
def solve():
    for line in read_lines():
        emit(transform(line))
```

## MTL sketch (design-stage, hand-traced)
```
read-lines 0[transform emit](_
```
`fold` over the line list with `C = [transform emit]` : `(acc h -- acc)` —
`transform` maps the handle, `emit` writes it, acc `0` is threaded untouched;
trailing `_` drops it. The map is a per-element capability call inside the fold. ✓

## Tokens (o200k / cl100k)
| | Python | MTL |
|---|---:|---:|
| map_lines_tool | 15 / 15 | 9 / 9 |

## Needs in-core strings?
**No.** Each line is an opaque handle transformed by a capability; the core only
drives the fold. Identical structural story to `grep_filter`.
