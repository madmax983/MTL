# json_field

**Intent:** extract a named field from a JSON object and emit it (field access as
a capability).

## I/O contract
- **Input:** a JSON document, delivered as an opaque host handle `j`.
- **Output:** the value at key `"name"`, emitted.
- **Capabilities (stack effects):**
  - `read-json : ( -- j )` — host parses input into a JSON handle.
  - `get-name : ( j -- v )` — host field accessor for key `name`
    (a capability specialised per key; `get-field:<key>` family).
  - `emit : ( v -- )` — write the value. Effect `{output}`.

## Python sketch (idiomatic)
```python
def solve():
    return get_field(read_json(), "name")
```

## MTL sketch (design-stage, hand-traced)
```
read-json get-name emit
```
Three `Call`s. `read-json` → `j`; `get-name` → `v`; `emit` consumes `v`. The
JSON parse *and* the field lookup are host-side; the core threads one handle. ✓

## Tokens (o200k / cl100k)
| | Python | MTL |
|---|---:|---:|
| json_field | 13 / 13 | 5 / 5 |

## Needs in-core strings?
**No.** Both parsing and key lookup are capabilities. If instead field access had
to be done in-core over a raw string, this task would need full `Str` scanning —
which is exactly the argument for keeping structured access host-side.
