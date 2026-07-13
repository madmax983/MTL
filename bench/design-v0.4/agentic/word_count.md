# word_count

**Intent:** read the input text, count the words, emit the count. **This is the
pivotal task for the Str-in-core question** — the only one in the suite whose
"obvious" formulation touches character-level compute.

## I/O contract
- **Input:** a text blob, delivered as an opaque host handle.
- **Output:** a single integer — the word count — emitted.
- **Capabilities (stack effects):**
  - `read-text : ( -- t )` — host returns the text handle.
  - `tokenize : ( t -- [w...] )` — host splits `t` into a Quote of word handles
    on whitespace (the tokenizer is host-owned).
  - `emit-int : ( n -- )` — write an integer. Effect `{output}`.

## Framing 1 — tokenize is a capability (RECOMMENDED)
```python
def solve():
    return len(tokenize(read_text()))
```
```
read-text tokenize 0[_1+](emit-int
```
`tokenize` returns a Quote; the count is a pure in-core `fold` length
(`0[_1+](`), no `Str` value anywhere. **This is the whole task.**

## Framing 2 — tokenize in-core (what Str-in-core would force)
If you *refuse* a `tokenize` capability, the core must scan the raw string for
whitespace runs: `str-uncons` every codepoint, compare to `' '` (32), count
transitions. That needs the **Variant A `Str` value + `str-uncons` + `eq`**, i.e.
it drags the entire `Str` constructor and its proof surface (§E.4/E.5) into the
core to re-implement what `tokenize` already does host-side. It is strictly more
tokens *and* strictly more proof cost, for a capability the host trivially
provides.

## Tokens (o200k / cl100k)
| | Python | MTL (Framing 1) |
|---|---:|---:|
| word_count | 11 / 11 | 11 / 11 |

A rare **tie** — the MTL program is dominated by long capability *names*
(`read-text`, `tokenize`, `emit-int`), which tokenize to several BPE tokens each,
erasing MTL's glyph-density edge. The lesson: where a task is mostly capability
*calls* (not control flow), MTL does **not** compress much; the wins come from
control flow (`agent_loop`, `retry_on_fault`), not from capability names.

## Needs in-core strings?
**No — provided `tokenize` is a capability.** Only Framing 2 (refusing the
capability) would need `Str` in the core, and it loses on both tokens and proof
cost. Even the one "stringy" agentic task does not justify the `Str` constructor.
