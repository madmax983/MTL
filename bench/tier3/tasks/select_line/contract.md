# Tier-3 task: `select_line`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readlines : ( -- [h...] )
    select    : ( [h...] n -- h )   (the n-th handle, 0-indexed; ToolError if out of range)
    emit      : ( h -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
readlines 2 select emit
```

## Fixture (host-owned inputs)

lines = ["a","b","c","d"]

## Expected result

output == "c\n"

## MTL adjustment vs the design sketch

None (new v0.4 task). `readlines` pushes the handle list; literal `2` is the
index; `select` pops the index and the list and pushes the 2nd (0-indexed) handle
"c"; `emit` writes it.
