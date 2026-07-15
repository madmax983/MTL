# Host conformance — the Invoke-contract obligations (O1–O8)

MTL's end-to-end safety factors into two halves (design `docs/design/v0.4-effects.md`
§2.3, §3.2, §7; spec §8.3):

```
end-to-end soundness  =  (P1 ∧ P2 ∧ P3, PROVED in-core)  ∧  (H conforms, ASSUMED)
```

The pure core is formally verified and total up to fuel; every actual effect
(I/O, tools, RNG, resource metering) happens in the **host**, arbitrary Rust that
is *assumed* — not proved — to respect the host contract. The `mtl-host-conformance`
crate is the executable precursor to the (still unproved) **P9 host-conformance
theorem**: a harness generic over any `H: mtl_core::host::Host` that runs `H`
against the enumerated obligations below and reports pass/fail per obligation,
naming the violated clause. It discharges the *assumed* half of the factoring by
**testing**, not by proof.

## The TCB boundary

The `mtl_core::host::Host` seam **is** the trust boundary. Everything above it is
outside the verified core:

- The only channel between core and host is the `Invoke` value: it carries
  `(name, stack_snapshot, cont)` out of the core and a `HostResult`
  (`Resume(stack)` | `HostFault(code)`) back in (design §2.3).
- **`host_state` never enters the core.** It stays host-local; only the resumed
  value stack crosses back.
- **`cont` is opaque to the host.** The `drive` loop holds it; `Host::service`
  is never handed the continuation and so cannot observe or rewrite it.
- Fuel is a *pure in-core* step counter owned by `drive`; host wall-clock / budget
  are metered host-side and surface only as a `HostCode`. Host cost is never
  folded into core fuel (design §7, Option B).

The core proves P1/P2/P3; conformance of `H` is the residual assumption. This kit
turns that assumption into a battery of checks.

## The obligations, traced to design §3.2

Design §3.2 states the host contract as clauses 1–5 over the two-machine seam
(§2.3 the channel, §7 the fuel/metering boundary). The kit enumerates them as
O1–O8:

| Obl. | Statement | Design trace |
|------|-----------|--------------|
| **O1** | `stack'.len() == stack.len() - consumes + produces` on every `Resume`. | §3.2 clause 1 (declared stack effect) |
| **O2** | The untouched prefix below the `consumes` consumed inputs is returned **verbatim**. | §3.2 clause 1 (effect is local to the top-`consumes` region) |
| **O3** | Each produced value matches its declared shape; core strings stay opaque host handles. | §3.2 clause 1 (declared shape) + §5 (opaque handles) |
| **O4** | Any raised `HostCode` is in the capability's declared fault set. | §3.2 clause 2 (fault contract) / §3.1, §6 |
| **O5** | `service` terminates — no silent divergence. | §3.2 clause 3 (host bounds its own service time, §7) |
| **O6** | Each `Invoke` causes **at most one** observable effect (no re-entrancy / replay). | §3.2 clause 4 (at-most-once) / §2.3 |
| **O7** | The host never observes or mutates `cont` (continuation opacity). | §2.3 (the channel) / §3.2 clause 5 |
| **O8** | The host cannot fold its cost into core fuel; the fuel/metering boundary holds. | §7 (global fuel budget, Option B metering) |

O1/O2/O4/O5 are **probed** — the kit drives `service` directly with synthesised
stacks (O5 under a bounded watchdog thread). O3/O6/O7/O8 are **boundary
obligations discharged structurally**: the trait signature or the value model
makes the violation either unrepresentable or observable only through a driven
run (see the two contract observations below). The test suite exercises the
*bite* of each structural obligation with concrete adversarial hosts.

## The catch table

Each adversarial host violates exactly **one** obligation (the #32 perturbation
discipline, applied to hosts). The kit rejects each at exactly its clause.

| Obligation | Conforming case | Adversarial host | Verdict |
|------------|-----------------|------------------|---------|
| O1 arity | `ConformingHost` returns `len - consumes + produces` | `WrongArityHost` (one extra value) | rejected at O1 (probed) |
| O1 arity | — | `LeakHost` (residual undeclared values) | rejected at O1 (probed) |
| O1 arity | — | `StringsLeakHost` (extra out-of-shape `Quote`) | rejected at O1 (probed) — see obs. (a) |
| O2 prefix | prefix returned verbatim | `MutatePrefixHost` (rewrites stack bottom) | rejected at O2 (probed) |
| O4 faults | raised code ∈ declared set | `MisdeclareFaultHost` (undeclared code) | rejected at O4 (probed) |
| O5 termination | returns within `SERVICE_TIMEOUT` | `DivergeHost` (never returns) | rejected at O5 (probed, watchdog) |
| O6 at-most-once | `ConformingHost` acts once/`Invoke` | `DoubleServiceHost` (2 effects) | caught by `ObservableHost` effect-count probe (boundary test) |
| O7 cont opacity | cont after `Call` executes | — (unrepresentable) | structurally discharged — see obs. (b) |
| O8 fuel boundary | slow host, core fuel intact | `SlowHost` (burns wall-clock) | structurally discharged; boundary test shows fuel unaffected — see obs. (b) |

## Two contract observations the kit exposes

**(a) `CapabilitySig` declares only arities, not per-value shapes.** The signature
carries `consumes: usize`, `produces: usize`, and a `faults` set — but no
per-value type/shape. So §3.2 clause 1's "each pushed value matches the declared
shape" is **not machine-checkable** against the contract as it stands: O3's
declared-shape half is a **flagged gap**. The *other* half of O3 — that core
strings are opaque and cannot leak — is discharged by the value model itself:
`mtl_core::interp::Value` has only `Int | Quote` (no `Str` variant), so a
core-string leak is *unrepresentable*. The `StringsLeakHost` adversary, having no
`Str` to leak, can only push an extra out-of-shape `Quote`, which degrades to an
O1 arity violation and is caught there.

**(b) O7 and O8 have no writable adversarial host.** The seam signature
`fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult` gives the
host neither the continuation nor a fuel channel:

- A **cont-observing / cont-mutating** host cannot be written — `cont` is simply
  not a parameter. The `drive` loop owns it and resumes from it untouched.
- A **fuel-folding** host cannot be written — `service` returns a `HostResult`
  with no cost field, so a host cannot debit the core's fuel counter.

This *unrepresentability is itself the guarantee*, recorded as **structurally
discharged**. The boundary tests make it positive rather than merely absent: the
O7 test proves the continuation after a `Call` still executes (a host cannot have
tampered with it), and the O8 test drives a genuinely slow host and shows the run
still completes on unchanged core fuel (host wall-clock is not charged to fuel).

## Using the kit

```rust
use mtl_host_conformance::{check_conformance, Obligation};

let report = check_conformance(move || my_build_host(), &my_sigs());
assert!(report.passed(), "{}", report.render());
// or inspect a single clause:
assert!(report.for_obligation(Obligation::O4Faults).unwrap().passed);
```

`check_conformance` takes a `Fn() -> H` (a fresh host per probe) plus the
`CapabilitySig`s the host grants, and returns a `ConformanceReport` with one
`ObligationResult` per obligation (`passed`, `kind: Probed | Structural`, and a
human-readable `detail`). `report.render()` prints the full per-obligation table.
