# MTL Tier-3 agentic suite — token + security-posture baseline (T_tier3-agentic)

- Report date: 2026-07-13
- Metric: STATIC program-source tokens under `o200k_base` + `cl100k_base` (tiktoken), one trailing newline stripped per source; ratio = tokens(python-idiomatic) / tokens(mtl), same encoding (higher = better for MTL).
- Two MTL columns: **sketch** = the design's canonical hyphenated program (`docs/design/v0.4-effects.md` §8); **exec** = the executable, lexer-safe `solution.mtl` actually run and validated by `crates/mtl-host`.

> Tokenizer availability: tiktoken **0.13.0**. 
> Both `o200k_base` and `cl100k_base` loaded; all cells populated.

## Per-task token counts

| task | py o200k | py cl100k | sketch o200k | sketch cl100k | exec o200k | exec cl100k | ratio (exec, o200k) |
|---|---:|---:|---:|---:|---:|---:|---:|
| `echo_line` | 8 | 8 | 3 | 3 | 3 | 3 | 2.67x |
| `grep_filter` | 20 | 20 | 12 | 12 | 12 | 12 | 1.67x |
| `agent_loop` | 24 | 24 | 10 | 10 | 10 | 10 | 2.40x |
| `json_field` | 13 | 13 | 5 | 5 | 5 | 5 | 2.60x |
| `two_tool_pipeline` | 10 | 10 | 5 | 5 | 5 | 5 | 2.00x |
| `retry_on_fault` | 30 | 30 | 12 | 12 | 14 | 14 | 2.14x |
| `map_lines_tool` | 15 | 15 | 9 | 9 | 9 | 9 | 1.67x |
| `word_count` | 11 | 11 | 11 | 11 | 11 | 11 | 1.00x |
| **TOTAL** | **131** | **131** | **67** | **67** | **69** | **69** | **1.90x** |

## Aggregate ratios (token-sum)

- **design-sketch**: o200k **1.96x**, cl100k **1.96x**  (reproduces the design's projected 1.96x).
- **executable**: o200k **1.90x**, cl100k **1.90x**  (the lexer-safe programs actually run).

The small gap between sketch and exec is `retry_on_fault` (12 → 14 tokens): the executable corrects the sketch's LinRec branch bodies so the success result is left on the stack (see its `contract.md`). All other tasks are token-identical up to the hyphen/`?` → lexer-safe renames.

## Security posture — capability confinement (the Tier-3 headline)

Tier-3's case for MTL is **capability confinement / safety**, not compression (design §8: 1.96x is modest — agentic glue is where MTL compression is thinnest). The `mtl-host` crate proves these guarantees:

- **The effect boundary is the trust boundary.** The pure core suspends at every capability `Call` and yields an `Invoke`; all effects happen in the unverified host runner, behind a single narrow channel.
- **Capabilities are a grant set.** Only registered names are reachable; the program text cannot perform an effect the host did not grant.
- **Metering is atomic and host-side.** Per-capability call budgets and a total output-byte cap are charged BEFORE the effect; a refused charge spends nothing and performs no effect (clean cancel).
- **Cancellation leaves no partial effect.** Fuel/budget exhaustion happens only between steps — the core is never running while the host acts, so at-most-once holds trivially and a cancel is torn-free.
- **Strings are opaque host-side handles.** No `Value::Str` in the core; the core shuffles `i64` handles it can neither inspect nor forge.

### Proven-by-test claims (`crates/mtl-host/tests/security_posture.rs`)

| test (reads as a claim) | what it demonstrates |
|---|---|
| `a_capability_not_granted_is_unreachable` | A `Call` to a capability absent from the registry returns `Refused` and performs no effect (no output, no state change). Grants are a whitelist. |
| `an_unknown_capability_is_refused_not_executed` | A never-registered name is `Refused`, categorically distinct from a pure-core `Fault` — an ungranted effect is unreachable, not a crash. |
| `budget_exhaustion_cancels_with_no_partial_effect` | With a per-capability call budget of N, the (N+1)-th call returns `HostFaulted(BudgetExhausted)` and is never serviced — exactly N effects occur, the over-budget call emits nothing. |
| `output_byte_cap_is_never_exceeded` | An emit that would exceed the total output-byte cap is refused wholesale (`HostFaulted(OutputCapExceeded)`); it writes zero bytes and the cap is never exceeded (charge-before-effect is atomic). |
| `granted_capability_is_reachable` | Positive control: a granted capability is reachable and its effect occurs. |
| `fuel_exhaustion_between_steps_cancels_cleanly` | A non-terminating program under a fuel bound returns `Cancelled` at a step boundary with no torn effect — the core is never suspended mid-capability. |
| `each_capability_invocation_consumes_the_call_exactly_once` | A capability called N times in the program is serviced exactly N times (at-most-once per yield; no double-service on resume). |

## Caveats

- **Executable names differ from design sketches.** The `mtl-syntax` lexer reads `-` as `sub` and `?` as `if`, so `read-line`/`done?` are mangled to `readline`/`donep`. Long `Call` names cost several BPE tokens each, which is why capability-name-dominated tasks (e.g. `word_count`) barely move.
- **The token case is secondary.** Per design §8, compression here is control-flow-driven; the loop tasks (`agent_loop`, `retry_on_fault`) win via `linrec`/`fold`, while name-heavy pipelines tie. The real deliverable is the confinement/safety posture above.
- **Adapter seam.** The host sources `Invoke` events by peeking the core's continuation (`core_bridge.rs`) until `mtl-core` lands `SpecStep::Invoke`; reconciliation is a one-file change.
- **tiktoken version**: measured under 0.13.0 (the design pinned 0.8.0; the o200k/cl100k vocabularies are stable across these versions — the design-sketch aggregate reproduces 1.96x exactly).

