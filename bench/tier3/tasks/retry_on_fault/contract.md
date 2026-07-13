# Tier-3 task: `retry_on_fault`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    tryop : ( -- r )        (flaky: succeeds once the warm-up count is reached)
    okp   : ( r -- r 0|1 )  (ok = tryop-call-count >= success threshold)
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
3[tryop okp][~_][_1-][]|
```

Design sketch (hyphenated, from §8, for token comparison): `3[try-op ok?][_][][1-]|`

## Fixture (host-owned inputs)

flaky_success_at = 3, budget = 3

## Expected result

RunResult::Done with the success result Int(3) on the stack; tryop and okp each called exactly 3 times

## MTL adjustment vs the design sketch

ADJUSTED from the design sketch `3[try-op ok?][_][][1-]|`. The sketch's branch bodies are incorrect on the real LinRec desugaring: (i) T=[_] drops the fresh result instead of the budget, so success returns the budget not r; (ii) R1=[] never drops the failed result, so failed r-values accumulate on the stack; (iii) R2=[1-] runs POST-recursion, so the budget-decrement corrupts the returned result. The faithful, minimal fix keeps the same approach (bounded LinRec on an Int budget) with correct bodies: T=[~_] (drop the budget, keep r), R1=[_1-] (drop the failed r and decrement the budget PRE-recursion), R2=[]. Stack invariant per level is [budget]; on ok, `~_` yields [r].
