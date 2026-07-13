# Tier-3 task: `grep_filter`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readlines : ( -- [h...] )
    linehit  : ( h -- h 0|1 )   (hit = line starts with predicate char)
    emit     : ( h -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
readlines 0[linehit[emit][_]?](_
```

Design sketch (hyphenated, from §8, for token comparison): `read-lines 0[line-hit[emit][_]?](_`

## Fixture (host-owned inputs)

lines = ["apple","banana","apricot","cherry"], predicate = starts-with 'a'

## Expected result

output == "apple\napricot\n"

## MTL adjustment vs the design sketch

None. Renames only (`read-lines`->`readlines`, `line-hit`->`linehit`). `(` is Fold, `?` is If; the seed 0 is a dummy accumulator that fold threads unchanged (each element emits or drops, leaving acc=0), then `_` drops it.
