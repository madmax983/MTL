# Tier-3 task: `two_tool_pipeline`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readinput : ( -- q )
    fetch     : ( q -- doc )    (doc = "doc:" + query)
    parse     : ( doc -- v )    (v = "parsed:" + <query part of doc>)
    emit      : ( v -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
readinput fetch parse emit
```

Design sketch (hyphenated, from §8, for token comparison): `read-input fetch parse emit`

## Fixture (host-owned inputs)

query = "q1"

## Expected result

output == "parsed:q1\n"  (fetch -> "doc:q1"; parse strips "doc:" -> "parsed:q1")

## MTL adjustment vs the design sketch

None. Renames only. The exact emitted string follows this crate's deterministic fetch/parse impl (documented above).
