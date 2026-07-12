# MTL Runtime Performance Baseline

Measured runtime-performance references for the MTL reference interpreter, produced by the `mtl-perf` crate. **Runtime performance is an explicit NON-goal of the language** (spec §1.2: *"The reference interpreter optimizes for provability, not speed."*). This baseline exists to serve the spec's **REFACTOR phase** — §12.3 open question: *"Continuation representation — Vec splice (`q ++ p`) is O(n) per apply; a cons-list or rope may be needed."* — with honest data on whether that refactor is actually needed, and where.

> **Hardware caveat.** All numbers below were captured in a shared cloud container. They are **relative references, not absolute performance figures** — use ratios and growth curves, not the raw ns/step, when reasoning across machines. Re-run locally for machine-specific numbers.

## Running the suite

```
cargo bench -p mtl-perf                              # criterion benches (statistical)
cargo run --release --example perf_report -p mtl-perf   # deterministic step/cont/ns-per-step tables (source of this doc)
```

Criterion groups: `dispatch/{flat,loop_steady}`, `recursion/{selfapp,primrec_sumto,linrec_countdown}`, `fold/{sum_ints,quote_list}`, `parser/throughput`, `corpus/{T_v0,tier2_v03}`. The `perf_report` example counts exact interpreter steps and peak continuation length, which criterion cannot, and is what the tables below come from.

## What is measured

The continuation is a `Vec<Word>` used as a queue: every step pops the head with `cont.remove(0)` (O(n) in continuation length) and re-emission splices with `prefix ++ cont` (O(n)) — see `mtl-core/src/interp.rs` `prepend` / the `Apply`, `Times`, `PrimRec`, `LinRec`, `Fold` arms. The benches exercise both costs at increasing scale, and track **peak `cont` length** so structural growth is visible.

---

## Measurements

### (a) Dispatch — flat straight-line program (`1 1 + _` × units)

Whole program sits in `cont` at once; every step pays `cont.remove(0)`.

| steps | peak cont | exec µs | ns/step | steps/sec |
|---:|---:|---:|---:|---:|
| 64 | 64 | 0.8 | 12.2 | 81.8M |
| 256 | 256 | 7.7 | 30.0 | 33.3M |
| 1024 | 1024 | 106.6 | 104.1 | 9.6M |
| 4096 | 4096 | 4889.1 | 1193.6 | 0.84M |
| 16384 | 16384 | 82858.0 | 5057.2 | 0.20M |

**steps grew 256×, ns/step grew 414× ⇒ per-step cost scales ~linearly with program length ⇒ TOTAL ~O(n²).** A flat program is the worst layout for this representation.

### (a) Dispatch — steady-state loop (`0 n [1 +] .`, `cont` stays ~constant)

| steps | peak cont | ns/step | steps/sec |
|---:|---:|---:|---:|
| 5002 | 5 | 27.4 | 36.5M |
| 50002 | 5 | 28.1 | 35.6M |
| 500002 | 5 | 28.9 | 34.6M |
| 5000002 | 5 | 28.8 | 34.7M |

**Peak dispatch throughput ≈ 35M steps/sec, ~29 ns/step, flat across 1000× scale.** This is the interpreter's true steady-state speed when the continuation does not accumulate.

### (b) `: !` self-application recursion (countdown to depth n) — the documented splice suspect

| n (depth) | steps | peak cont | ns/step | steps/sec |
|---:|---:|---:|---:|---:|
| 10 | 140 | 6 | 25.7 | 39.0M |
| 100 | 1310 | 6 | 26.6 | 37.6M |
| 1000 | 13010 | 6 | 26.7 | 37.5M |
| 10000 | 130010 | 6 | 27.6 | 36.2M |

**Peak cont constant at 6; ns/step grew 1.08× over 1000× depth ⇒ TOTAL ~O(n) (linear).** The `: !` splice that §12.3 flags as the suspect is **not** pathological: because self-application sits in *tail position*, each `Apply` splices its body onto an empty tail, so the continuation never accumulates. Deep `: !` recursion is linear.

### (c) PrimRec — `[0] [+] &` sum to n

| n | steps | peak cont | ns/step | steps/sec |
|---:|---:|---:|---:|---:|
| 10 | 64 | 15 | 26.9 | 37.2M |
| 100 | 604 | 105 | 38.6 | 25.9M |
| 1000 | 6004 | 1005 | 178.3 | 5.6M |
| 10000 | 60004 | 10005 | 3722.9 | 0.27M |

**Peak cont grows *linearly with n* (10,005 at n=10k); ns/step grew 139× ⇒ effectively O(n²).** PrimRec re-emits the combinator after the recursive call (`[… PrimRec] ++ C ++ rest`), so the continuation tail lengthens by |C| every level. This is the primary pathology.

### (c) LinRec — `[: 0 =] [_] [1 -] [] |` countdown

| n | steps | peak cont | ns/step | steps/sec |
|---:|---:|---:|---:|---:|
| 10 | 142 | 7 | 30.3 | 33.0M |
| 100 | 1312 | 7 | 29.4 | 34.0M |
| 1000 | 13012 | 7 | 26.9 | 37.2M |
| 10000 | 130012 | 7 | 28.6 | 35.0M |

**Peak cont constant at 7; ns/step flat ⇒ TOTAL ~O(n) (linear).** With an empty post-recursion step (`R2 = []`) LinRec does not accumulate — linear. (A non-empty `R2` would accumulate like PrimRec; this measures the common tail-linear shape.)

### (c) Fold — `0 [+] (` over n ints  &  (d) Fold — `[] [_] (` over n quotation elements

| n | steps | peak cont | ns/step (ints) | ns/step (quotes) |
|---:|---:|---:|---:|---:|
| 10 | 63 | 6 | 20.6 | 16.5 |
| 100 | 603 | 6 | 22.9 | 18.2 |
| 1000 | 6003 | 6 | 36.0 | 31.9 |
| 10000 | 60003 | 6 | 532.7 | 532.7 |

**Peak cont constant at 6, yet ns/step grew ~26–32× ⇒ super-linear (~O(n²)).** The cost is invisible to a continuation-length metric: `Fold` re-emits `PushQuote(tail)` each element, deep-**cloning** the remaining list spine — O(n−k) at element k, summing to O(n²). Int vs quote payload is identical (532.7 ns/step at n=10k both), confirming the cost is the spine re-clone, not the element type. A 10k-element fold takes ~32 ms.

### (e) Parser throughput (`mtl-syntax::parse`)

| bytes | parse µs | MiB/s |
|---:|---:|---:|
| 2,100 | 22.8 | 88.0 |
| 21,000 | 200.6 | 99.9 |
| 210,000 | 3,991.4 | 50.2 |
| 1,050,000 | 25,528.5 | 39.2 |

Roughly linear; ~40–100 MiB/s (falling off with cache pressure at larger inputs). Parsing is not a bottleneck.

### (f) Corpus end-to-end (FUEL = 100,000) — realistic agent-authored solutions

**T_v0 (v0.1 gate set)** — correctness: 23/23 vectors halted with expected output.

| task | vectors | max steps | peak cont | total µs |
|---|---:|---:|---:|---:|
| affine | 4 | 4 | 4 | 0.53 |
| rev3 | 2 | 2 | 2 | 0.19 |
| is_even | 5 | 4 | 4 | 0.66 |
| factorial | 6 | 115 | 9 | 12.02 |
| gcd | 6 | 53 | 6 | 7.17 |

**Tier-2 v0.3 (fold/xor set)** — correctness: 48/48 vectors halted with expected output.

| task | vectors | max steps | peak cont | total µs |
|---|---:|---:|---:|---:|
| sum_list | 5 | 27 | 6 | 1.98 |
| length_list | 3 | 43 | 8 | 1.88 |
| product_list | 4 | 27 | 6 | 1.84 |
| max_list | 4 | 54 | 11 | 4.77 |
| min_list | 3 | 43 | 11 | 3.11 |
| reverse_list | 4 | 31 | 7 | 2.00 |
| contains | 5 | 38 | 11 | 4.09 |
| count_occurrences | 5 | 37 | 8 | 3.74 |
| single_number | 3 | 72 | 11 | 5.76 |
| palindrome_number | 6 | 99 | 15 | 11.70 |
| climbing_stairs | 6 | 36 | 7 | 4.04 |

Every real corpus solution runs in **microseconds**, with a max of **115 steps** and peak continuation of **15 words** — three to four orders of magnitude below where the O(n²) effects begin to bite.

---

## Analysis

### The continuation-splice growth curve (spec §12.3)

The open question asks whether the O(n) `Vec` splice needs a cons-list/rope. The data refines the question:

- **The named suspect — the `: !` apply splice — is fine.** Deep self-application is **O(n) linear** (§b) because it is tail-recursive in the continuation: each splice lands on an empty tail, so `cont` stays constant (6 words) to depth 10,000. LinRec with empty post-step is likewise linear (§c).
- **The real O(n²) comes from two distinct mechanisms**, both rooted in "`cont` is a `Vec` and the head is the front":
  1. **`cont.remove(0)` front-pop** — makes any program whose continuation is *long* quadratic. A flat 16k-step program runs at 0.2M steps/sec vs 35M steady-state — a **175× slowdown** purely from front-pop shifting (§a).
  2. **Continuation / spine accumulation in the recursion prims** — **PrimRec** grows `cont` linearly with depth (re-emitting `C` after the recursive call), and **Fold** deep-clones the shrinking list tail on every element. Both are O(n²) in input size, though PrimRec's growth is visible in the cont-length metric and Fold's is hidden inside a single `PushQuote(tail)` word.

A cons-list / persistent-sequence continuation (O(1) head pop, O(1) prepend, structural sharing of the tail) would collapse all three: front-pop becomes O(1), PrimRec's re-emit stops copying, and Fold's tail becomes a shared suffix instead of a fresh clone.

### Worst pathological case (quantified)

**PrimRec over large n.** `sum_to(10000)` = **223 ms** for only 60,004 steps (3,723 ns/step — ~130× the 29 ns steady-state). Because it is O(n²), extrapolation is steep: `sum_to(100000)` ≈ **~22 s**. Any primitive-recursive program over 10k+ iterations is effectively unusable. Fold is milder but also O(n²): a 10k-element fold is ~32 ms and a 100k-element fold would be ~3 s.

### Fuel interaction

Fuel is a pure **step counter** (`run(vm, fuel)` increments one unit per `exec_step`). Nothing surprising happens to the *count* at scale — but the important observation is that **fuel does not reflect true cost under this representation**: a single "step" can perform O(n) work (`remove(0)`, splice, or `PushQuote(tail)` clone), so wall-time-per-step is not constant. PrimRec `sum_to(10000)` costs 60k fuel but 223 ms; an equal-fuel steady loop costs ~1.7 ms. **Fuel budgets therefore under-price quadratic programs** — a fuel limit that is generous for a flat program can still admit a program that is 100× slower in wall time. If the continuation is refactored to O(1) head/prepend, fuel becomes a much more faithful cost proxy. All corpus solutions sit at ≤115 steps, so the default 100k fuel has ~1000× headroom for realistic workloads.

### Verdict — is the current representation fine for realistic agent workloads?

**Yes, for the workloads the corpus represents — and the refactor is not urgent for them.** Every T_v0 and tier-2 v0.3 solution runs in microseconds at ≤115 steps and ≤15 continuation words, with three orders of magnitude of fuel headroom; the O(n²) behaviour never engages because real agent-authored solutions operate on small inputs and shallow recursion. The steady-state interpreter (~35M steps/sec) is more than adequate here.

**The REFACTOR becomes necessary only if MTL targets large-data workloads** — folds, primitive recursion, or flat programs over ~1k–10k+ elements — where the `Vec` continuation's O(n) front-pop, PrimRec re-emission, and Fold tail-clone compound into O(n²) and make 10k-element inputs sluggish (32 ms–223 ms) and 100k inputs unusable (seconds). Crucially, the data **redirects** the §12.3 hypothesis: the `: !` apply splice singled out in the spec is *not* the culprit (it is tail-linear); the actionable targets are `cont.remove(0)` and the PrimRec/Fold re-emission/clone. A single change — a persistent sequence with O(1) head-pop and prepend plus structural tail sharing — addresses all three. That is a spec-first, correctness-preserving change per TAVDD, scoped to when a large-data use case is real; until then, this baseline stands as the regression reference.
