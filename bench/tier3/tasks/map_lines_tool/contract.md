# Tier-3 task: `map_lines_tool`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readlines : ( -- [h...] )
    transform : ( h -- h' )   (uppercases the line)
    emit      : ( h' -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
readlines 0[transform emit](_
```

Design sketch (hyphenated, from §8, for token comparison): `read-lines 0[transform emit](_`

## Fixture (host-owned inputs)

lines = ["a","b","c"], transform = uppercase

## Expected result

output == "A\nB\nC\n"

## MTL adjustment vs the design sketch

None. Rename only. `(` folds with dummy acc 0 threaded unchanged; `_` drops it.
