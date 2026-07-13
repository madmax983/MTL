# echo_line

**Intent:** read one line of input and emit it unchanged (the minimal I/O agent).

## I/O contract
- **Input:** one text line, delivered as an opaque host handle.
- **Output:** the same line emitted verbatim.
- **Capabilities (stack effects):**
  - `read-line : ( -- h )` — host reads a line, returns handle `h`.
  - `emit : ( h -- )` — host writes handle `h` to output. Effect `{output}`.

## Python sketch (idiomatic)
```python
def solve():
    emit(read_line())
```

## MTL sketch (design-stage, hand-traced)
```
read-line emit
```
Two `Call`s, no core compute. `read-line` suspends → `Invoke("read-line", …)` →
host `Resume([…h])`; `emit` suspends → host consumes `h`. Stack empty at halt. ✓

## Tokens (o200k / cl100k)
| | Python | MTL |
|---|---:|---:|
| echo_line | 8 / 8 | 3 / 3 |

## Needs in-core strings?
**No.** The line is an opaque handle passed between two capabilities; the core
never inspects a character.
