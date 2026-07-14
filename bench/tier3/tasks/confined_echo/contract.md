# Tier-3 task: `confined_echo`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readline : ( -- h )
    emit     : ( h -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Grant (capability confinement)

This task is driven against a RESTRICTED registry granting ONLY `{readline, emit}`.
Every other capability is removed from the grant set, so any `Call` to an ungranted
name (e.g. `transform`) is unreachable and faults `HostFaulted(NotGranted)` with no
effect and no output. The grant set is a whitelist: the program text cannot perform
an effect the host did not grant.

## Executable MTL

```
readline emit
```

## Fixture (host-owned inputs)

lines = ["hello"]

## Expected result

output == "hello\n". A solution reaching for an ungranted cap, e.g.
`readline transform emit`, yields `HostFaulted(NotGranted)` naming `transform`.

## MTL adjustment vs the design sketch

None. Same program as `echo_line`; the task differs only in its CONFINED grant set.
