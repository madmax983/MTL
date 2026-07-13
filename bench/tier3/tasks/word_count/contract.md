# Tier-3 task: `word_count`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readtext : ( -- t )
    tokenize : ( t -- [w...] )   (splits on whitespace)
    emitint  : ( n -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
readtext tokenize 0[_1+](emitint
```

Design sketch (hyphenated, from §8, for token comparison): `read-text tokenize 0[_1+](emit-int`

## Fixture (host-owned inputs)

text = "the quick brown fox"

## Expected result

output == "4\n"

## MTL adjustment vs the design sketch

None. `read-text`->`readtext`, `emit-int`->`emitint`. `[_1+](` is a fold length: each element drops the word and increments the accumulator.
