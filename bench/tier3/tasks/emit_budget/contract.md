# Tier-3 task: `emit_budget`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    readlines : ( -- [h...] )
    emit      : ( h -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
readlines>@emit_>@emit__
```

## Fixture (host-owned inputs)

lines = ["one","two","three","four"]

## Budget

`emit` call budget = 2 (set on the meter for this task). A solution that emits a
third line faults `HostFaulted(BudgetExhausted)` on the 3rd `emit`, which writes
nothing (charge-before-effect is atomic) — exactly two effects occur.

## Expected result

output == "one\ntwo\n" (RunResult::Done, empty stack)

## MTL adjustment vs the design sketch

ADJUSTED from the sketch `readlines >'emit >'emit _`. The sketch is not runnable:
`>` (uncons) leaves `h [tail] 1` with the Int `1` on top, so a following `'` (dip)
faults TypeMismatch (dip needs a quote on top, not an Int), and `'emit` has no quote
literal to dip under. The faithful fix keeps the same "uncons the list, emit the
head, recurse on the tail" shape with explicit shuffles: per step `>@emit_` =
uncons (`h [tail] 1`), rot (`[tail] 1 h`), emit (`[tail] 1`), drop the `1`
(`[tail]`). Two steps emit the first two lines; a trailing `_` drops the leftover
tail list. Halts cleanly BEFORE any third `emit`, so it does not depend on the
budget to stop.
