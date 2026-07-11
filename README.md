# MTL — Minimal Token Language

MTL is a concatenative, stack-based programming language designed to minimize
the expected number of LLM-tokenizer tokens per program while remaining Turing
complete. Its writers are agents, not humans: programs are point-free sequences
of self-delimiting symbol words chosen so that BPE tokenizers frequently merge
adjacent primitives into single tokens, driving the effective cost per primitive
*below* one token. The language's reference semantics and interpreter are
formally verified in [Verus](https://github.com/verus-lang/verus).

See the full language specification in [`docs/mtl-spec.md`](docs/mtl-spec.md).

## North-star metric

Let `T` be a task distribution, `tok(p)` the token count of program text `p`
under a fixed tokenizer set (o200k_base, cl100k_base, Claude tokenizer), and
`sol(t, L)` the shortest known correct solution to task `t` in language `L`.

> **Objective:** minimize `E[t ~ T] [ tok(sol(t, MTL)) ]`, subject to MTL being
> Turing complete.

The real headline metric is `E[tokens × attempts]` — a token-cheap language that
agents cannot reliably write to loses.

## The Abrash rule (≥3× gate)

MTL is pursued only if it clears a hard, measured bar:

> **Success gate (Abrash rule):** MTL ships only if it achieves ≥3× token
> reduction vs. idiomatic Python on the benchmark suite, at equal or better agent
> success rate. Below that, it's a curiosity and we say so.

This gate decides whether MTL is worth building at all. The measurement protocol
(spec §10–§11) defines the benchmark suite and glyph-assignment procedure that
feed the gate; the `MEASURE` phase below either clears ≥3× or produces the
post-mortem.

## Roadmap

Phases follow the TAVDD flow from spec §13 (SPEC → PROOF → RED → GREEN →
REFACTOR → CHECK → MEASURE):

| Phase | Status | Description |
|---|---|---|
| SPEC | Done | This document + `mtl_core.rs` Verus spec skeleton (gate: review). |
| PROOF | In progress | P1–P4 verified; P5 stated with lock-step lemma skeleton (gate: `verus` green). This bootstrap lands the machine-checked artifact with P1 (determinism) and P3 (progress) proved, plus truncating div/mod semantics and the `:!` Y-idiom smoke theorem. |
| RED | Planned | Golden + boundary + proptest suites, initially failing (gate: tests exist, fail). |
| GREEN | Planned | Exec interpreter passing tests, P2 (refinement) discharged (gate: tests + proofs green). |
| REFACTOR | Planned | Continuation representation tuning under green lights (gate: benches). |
| CHECK | Planned | §14 multiplicity checker + P7–P9 (gate: `verus` green on P9). |
| MEASURE | Planned | §10 benchmark suite vs. Python; §11 glyph freeze (gate: ≥3× or write the post-mortem). |

## Repository layout

```
MTL/
├── Cargo.toml                       # workspace root
├── rust-toolchain.toml              # stable Rust for the cargo crate
├── .gitignore
├── README.md
├── LICENSE
├── docs/
│   └── mtl-spec.md                  # MTL language specification v0.1
├── crates/
│   └── mtl-core/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs               # stable-Rust stub that cargo compiles
│           └── mtl_core.rs          # Verus artifact (verified by `verus`, not cargo)
└── .github/
    └── workflows/
        └── ci.yml                   # cargo check/test (blocking) + verus verify (non-blocking)
```

`crates/mtl-core/src/mtl_core.rs` is a self-contained Verus artifact: it uses the
`verus!` macro and depends on `vstd`, so it does **not** compile under plain
stable `rustc`/`cargo`. It deliberately lives under `src/` but is **not** wired
into the cargo build graph (there is no `mod mtl_core;` in `lib.rs`), so
`cargo check` and `cargo test` ignore it. The `verus` tool targets the file
directly.

## Running Verus

The proofs are pinned to **Verus 0.2026.07.05**. Verus is not on crates.io;
install it from the [verus-lang/verus releases](https://github.com/verus-lang/verus/releases).
Then verify the core artifact:

```sh
verus crates/mtl-core/src/mtl_core.rs
```

As of the spec revision, this checks with 10 queries and 0 errors: P3 (progress),
P1 (determinism, by construction), the truncating div/mod semantics, deep-view
termination through nested quotations, and the two-token `:!` Y-idiom smoke
theorem.

## Building

The cargo crate builds on stable Rust:

```sh
cargo check --workspace
cargo test --workspace
```
