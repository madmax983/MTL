# Tier-3 task: `transform_hits`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readlines : ( -- [h...] )
    linehit   : ( h -- h 0|1 )   (hit = line starts with predicate char)
    transform : ( h -- h' )      (uppercases the string)
    emit      : ( h -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
readlines 0[linehit[transform emit][_]?](_
```

## Fixture (host-owned inputs)

lines = ["apple","banana","apricot","cherry"], predicate = starts-with 'a'

## Expected result

output == "APPLE\nAPRICOT\n"

## MTL adjustment vs the design sketch

None (new v0.4 task). This is the `grep_filter` fold idiom with `transform`
inserted into the hit branch: fold threads a dummy accumulator 0 unchanged (each
element uppercases+emits or drops, leaving acc=0), then `_` drops it. `(` is Fold,
`?` is If.
