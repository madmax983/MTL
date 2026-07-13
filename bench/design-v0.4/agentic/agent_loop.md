# agent_loop

**Intent:** the canonical "call a tool until done" fixed-point agent loop — step
an opaque state until a host termination predicate fires.

## I/O contract
- **Input:** an initial state handle from the host.
- **Output:** the final (done) state handle left on the stack.
- **Capabilities (stack effects):**
  - `read-state : ( -- s )` — host returns the initial state handle.
  - `done? : ( s -- s 0|1 )` — host predicate; leaves the state, pushes a flag.
  - `step : ( s -- s' )` — host advances the state by one tool call.

## Python sketch (idiomatic)
```python
def solve():
    s = read_state()
    while not done(s):
        s = step(s)
    return s
```

## MTL sketch (design-stage, hand-traced)
```
read-state[done?][][step][]|
```
`linrec ( [P] [T] [R1] [R2] -- )`: `P = [done?]` (non-consuming test), `T = []`
(terminal: keep `s`), `R1 = [step]` (advance before recursing), `R2 = []`. Loops
`step` while `done?` is false, leaving the final state. **Partial / fuel-bounded**
— non-termination is possible if the host never reports done (a metering hook, see
`glyphs.md` budget annotation, is the natural guard). ✓ hand-traced for a 2-step
fixpoint.

## Tokens (o200k / cl100k)
| | Python | MTL |
|---|---:|---:|
| agent_loop | 24 / 24 | 10 / 10 |

## Needs in-core strings?
**No.** The state is an opaque handle; all inspection is via `done?`/`step`
capabilities. This is a pure control-flow win (24 → 10), MTL's strongest suit.
