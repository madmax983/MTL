# Tier-3 task: `json_field`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readjson : ( -- j )
    getname  : ( j -- v )   (extracts the "name" field)
    emit     : ( v -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
readjson getname emit
```

Design sketch (hyphenated, from §8, for token comparison): `read-json get-name emit`

## Fixture (host-owned inputs)

json = {"name":"neo","age":30}

## Expected result

output == "neo\n"

## MTL adjustment vs the design sketch

None. `read-json`->`readjson`, `get-name`->`getname`.
