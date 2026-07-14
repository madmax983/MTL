# Tier-3 task: `confined_grep`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readlines : ( -- [h...] )
    linehit   : ( h -- h 0|1 )   (hit = line starts with predicate char)
    emit      : ( h -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Grant (capability confinement)

Driven against a RESTRICTED registry granting ONLY `{readlines, linehit, emit}`.
Any `Call` to an ungranted name (e.g. `transform`) is unreachable and faults
`HostFaulted(NotGranted)` with no effect and no output — grants are a whitelist.

## Executable MTL

```
readlines 0[linehit[emit][_]?](_
```

## Fixture (host-owned inputs)

lines = ["cat","dog","car","fish"], predicate = starts-with 'c'

## Expected result

output == "cat\ncar\n". A solution reaching for an ungranted cap, e.g.
`readlines 0[transform emit](_`, yields `HostFaulted(NotGranted)` naming `transform`.

## MTL adjustment vs the design sketch

None. Same program as `grep_filter`; the task differs only in its CONFINED grant set.
`(` is Fold, `?` is If; seed 0 is a dummy accumulator threaded unchanged, then `_`
drops it.
