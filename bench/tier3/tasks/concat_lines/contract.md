# Tier-3 task: `concat_lines`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    nextline : ( -- h )      {advances the read cursor}
    concat   : ( h1 h2 -- h )  (for stack ... h1 h2, interns resolve(h1)+resolve(h2))
    emit     : ( h -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
nextline nextline concat emit
```

## Fixture (host-owned inputs)

lines = ["foo","bar"]

## Expected result

output == "foobar\n"

## MTL adjustment vs the design sketch

None (new v0.4 task). Two `nextline` reads leave `h1 h2` (h1="foo" below,
h2="bar" on top); `concat` resolves in stack order resolve(h1)+resolve(h2) =
"foo"+"bar" = "foobar"; `emit` writes it. The read cursor makes the two reads
distinct lines (unlike `readline`, which always returns the first line).
