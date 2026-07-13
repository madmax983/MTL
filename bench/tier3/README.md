# Tier-3 — the v0.4 agentic / capability-confinement suite

Tier-3 is the **effects** tier: the read-input / emit-output / tool-call shape
that MTL 0.4.0 is measured on once the host boundary exists
(`docs/design/v0.4-effects.md` §8). Where Tier-1/Tier-2 measure pure-compute
compression, Tier-3's case is **capability confinement / safety** — token
compression here is thin and secondary.

## The capability model (two-machine split)

The pure, verified core (`crates/mtl-core`) is a closed step relation over
`(stack, cont)`. It cannot do I/O. In v0.4 it **suspends at every capability
`Call(name)`** and yields an `Invoke` event to an **unverified host runner**
(`crates/mtl-host`), which:

1. checks `name` against a **grant set** (a `Registry`) — ungranted ⇒ `Refused`;
2. charges a host-side **meter** (per-capability call budget + output-byte cap)
   BEFORE any effect — a refused charge spends nothing;
3. runs the capability (pops inputs, does host work, pushes outputs);
4. resumes the core from the continuation with the returned stack.

Effects live entirely on the host side of this single narrow channel. Strings
never enter the core: they are opaque `i64` **handles** the core shuffles but
cannot inspect (design §5, no `Value::Str`).

> Adapter seam: until `mtl-core` lands `SpecStep::Invoke`, the host synthesizes
> `Invoke` events by peeking the core's continuation (`core_bridge.rs`). This is
> isolated to one file; reconciliation with the core is a one-file change.

## Tasks

Eight tasks, each a directory under `tasks/<t>/`:

| task | intent |
|---|---|
| `echo_line` | read a line, emit it |
| `grep_filter` | emit lines a predicate accepts (`fold`) |
| `agent_loop` | call `step` until `donep` (`linrec` fixpoint) |
| `json_field` | extract a JSON field, emit it |
| `two_tool_pipeline` | `fetch` → `parse` → `emit` |
| `retry_on_fault` | retry a flaky tool within a budget (bounded `linrec`) |
| `map_lines_tool` | transform each line via a tool (`fold`) |
| `word_count` | count words, emit the count (`fold` length) |

Each task dir holds:

- `solution.mtl` — the **executable** program (lexer-safe capability names,
  `[a-z][a-z0-9]*`; the design's `read-line`/`done?` become `readline`/`donep`).
- `solution.py` — the idiomatic Python `solve()` body it mirrors.
- `contract.md` — capability signatures, fixture inputs, expected output, and any
  MTL adjustment vs the design sketch (only `retry_on_fault` is adjusted).

## Running

**Rust tests (the gate).** Every `solution.mtl` is parsed by `mtl-syntax`,
converted to `mtl-core` words, and driven by the `mtl-host` runner:

```
cargo test -p mtl-host                 # tier3 (8) + security_posture (7) + units
cargo test --workspace                 # everything
```

- `crates/mtl-host/tests/tier3.rs` — one test per task, asserting the emitted
  output / terminal stack.
- `crates/mtl-host/tests/security_posture.rs` — seven tests whose names read as
  capability-confinement claims (grant refusal, budget/byte caps, clean cancel,
  at-most-once service).

**Token report.** Requires `tiktoken`
(`pip3 install -r bench/tokcount/requirements.txt`):

```
cd bench
python3 tier3/measure.py               # py | mtl-sketch | mtl-exec token table
python3 tier3/report.py                # markdown report -> stdout, writes BASELINE-TIER3.md
```

`measure.py` prints two aggregate ratios: the **design-sketch** aggregate
(reproduces the design's 1.96x) and the **executable** aggregate (1.90x — the
only difference is `retry_on_fault`, whose executable corrects the sketch's loop
bodies). If `tiktoken` is not installed the scripts still run but show `—` cells;
the Rust tests are the real gate and do not need Python.

The generated baseline is `bench/BASELINE-TIER3.md`. This harness never touches
`bench/tokcount/tasks.json` or the frozen `bench/BASELINE*.md` files.
