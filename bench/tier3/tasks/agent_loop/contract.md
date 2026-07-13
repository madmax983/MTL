# Tier-3 task: `agent_loop`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readstate : ( -- s )
    donep     : ( s -- s 0|1 )   (done = s >= threshold)
    step      : ( s -- s' )      (s' = s + 1)
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
readstate[donep][][step][]|
```

Design sketch (hyphenated, from §8, for token comparison): `read-state[done?][][step][]|`

## Fixture (host-owned inputs)

initial_state = 0, done_threshold = 5

## Expected result

RunResult::Done with top of stack Int(5)

## MTL adjustment vs the design sketch

None. `read-state`->`readstate`, `done?`->`donep`. `|` is LinRec: P=[donep] leaves the flag, T=[] (no-op on done), R1=[step], R2=[].
