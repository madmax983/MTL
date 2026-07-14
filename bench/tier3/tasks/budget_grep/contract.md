# Tier-3 task: `budget_grep`

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

## Budget

`emit` call budget = 2 (set on the meter for this task). The fixture has exactly two
hits, so the correct program emits exactly twice and halts. A buggy program that
emits every line (e.g. dropping the `linehit` guard: `readlines 0[emit](_`) faults
`HostFaulted(BudgetExhausted)` on the 3rd `emit`, which writes nothing.

## Executable MTL

```
readlines 0[linehit[emit][_]?](_
```

## Fixture (host-owned inputs)

lines = ["ant","bee","art","cod"], predicate = starts-with 'a'

## Expected result

output == "ant\nart\n"

## MTL adjustment vs the design sketch

None. Same program as `grep_filter`; the task differs only in the `emit` budget and
fixture. `(` is Fold, `?` is If.
