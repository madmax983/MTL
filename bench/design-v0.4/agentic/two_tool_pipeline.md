# two_tool_pipeline

**Intent:** chain two tools — feed the input through tool A, then tool B, then
emit (the minimal multi-tool pipeline).

## I/O contract
- **Input:** a query handle from the host.
- **Output:** the parsed result of fetching then parsing, emitted.
- **Capabilities (stack effects):**
  - `read-input : ( -- q )` — host returns the query handle.
  - `fetch : ( q -- doc )` — tool A: retrieve a document handle for `q`.
  - `parse : ( doc -- v )` — tool B: parse the document into a value handle.
  - `emit : ( v -- )` — write the result. Effect `{output}`.

## Python sketch (idiomatic)
```python
def solve():
    return parse(fetch(read_input()))
```

## MTL sketch (design-stage, hand-traced)
```
read-input fetch parse emit
```
Straight-line handle threading: `q → doc → v → ⌀`. Point-free composition is the
identity case where MTL and a nested Python call are closest in shape; MTL still
wins by dropping `def solve(): return …(…())` scaffolding. ✓

## Tokens (o200k / cl100k)
| | Python | MTL |
|---|---:|---:|
| two_tool_pipeline | 10 / 10 | 5 / 5 |

## Needs in-core strings?
**No.** Every stage is a capability; the core passes opaque handles down the pipe.
