# MTL: Minimal Token Language — Specification v0.1

**Status:** Draft for review
**North star:** Turing complete. Minimize expected LLM-tokenizer tokens per program over a benchmark task distribution.
**Verification target:** Reference semantics and interpreter verified in Verus (SPEC → PROOF → RED → GREEN → REFACTOR).

---

## 1. Objective and Non-Goals

### 1.1 The metric

Let `T` be a task distribution (§10), `tok(p)` the token count of program text `p` under a fixed tokenizer set (o200k_base, cl100k_base, Claude tokenizer), and `sol(t, L)` the shortest known correct solution to task `t` in language `L`.

> **Objective:** minimize `E[t ~ T] [ tok(sol(t, MTL)) ]`, subject to MTL being Turing complete.

**Success gate (Abrash rule):** MTL ships only if it achieves ≥3× token reduction vs. idiomatic Python on the benchmark suite, at equal or better agent success rate. Below that, it's a curiosity and we say so.

### 1.2 Non-goals

- Minimal *number of primitives*. Turing tarpits are explicitly rejected; the primitive set grows whenever a new primitive pays for itself in corpus-level token savings.
- Human ergonomics. Writers are agents; a validator and pretty-printer serve humans.
- Performance. The reference interpreter optimizes for provability, not speed.

---

## 2. Lexical Structure

The lexer is deliberately trivial — every lexing rule exists to enable BPE merges.

### 2.1 Token classes

| Class | Rule |
|---|---|
| **Symbol word** | A single ASCII punctuation character from the primitive table (§5). Self-delimiting: `:!` lexes as `:` `!`. |
| **Integer literal** | `-?[0-9]+`, value in `i64`. Delimited by any non-digit. |
| **String literal** | `"..."` with `\"` and `\\` escapes only. |
| **Named word** | `[a-z][a-z0-9]*` — used only for host-injected capabilities (§8) and user definitions (§9). Delimited by non-alphanumerics. |
| **Whitespace** | Optional between all tokens except adjacent integers / adjacent named words. Never required between symbol words. |

### 2.2 The merge principle

Because symbol words self-delimit, programs are written **without whitespace between symbols**: `:!` not `: !`. BPE tokenizers frequently merge adjacent punctuation into single tokens (`:!`, `];`, `[[` are commonly 1 token each). This means MTL's effective cost per primitive is often *below* 1 token — a property no whitespace-separated language can have.

**Consequence:** primitive glyph assignment is an empirical optimization problem (§11), not an aesthetic choice. Glyphs are assigned to maximize merge frequency of common *bigrams and trigrams* in the benchmark corpus.

---

## 3. Values and Machine State

```
Value  ::= Int(i64) | Str(string) | Quote(Program)
Program ::= sequence of Word
Word   ::= Push(Value) | Prim(PrimOp) | Call(name)
State  ::= (stack: List<Value>, cont: Program)
```

- A **program** is a finite sequence of words.
- The **machine state** is a pair: an operand stack and a continuation (the remaining program).
- **Quotations** `[ ... ]` are first-class values: unevaluated programs pushed onto the stack. They are MTL's *only* abstraction mechanism — functions, closures, control flow, and data constructors are all quotations.
- Booleans are integers: `0` is false, non-zero is true. (A dedicated bool type is a token tax with no benefit.)

There are no variables and no environments. This is a design consequence of the metric: names are pure token spend, and point-free composition eliminates them. It is also a verification gift — the state is a pair of lists, no binding structure, no substitution lemmas.

---

## 4. Small-Step Operational Semantics

The semantics is a **total step function** — this is the load-bearing decision for Verus verification. Every state maps to exactly one of three outcomes:

```
Step ::= Next(State) | Halt(stack) | Fault(Error)

Error ::= Underflow | TypeMismatch | Overflow | DivByZero
        | UnknownWord | FuelExhausted   -- FuelExhausted: driver only, not step
```

### 4.1 Step rules

Notation: stack grows rightward; `s · v` is stack `s` with `v` on top. `p` is the remaining continuation after consuming the head word.

```
(s, ε)                          → Halt(s)

(s, Push(v) p)                  → Next(s · v, p)

-- Stack shuffling
(s · v,        Dup  p)          → Next(s · v · v, p)
(s · v,        Drop p)          → Next(s, p)
(s · a · b,    Swap p)          → Next(s · b · a, p)
(s · a · b · c, Rot p)          → Next(s · b · c · a, p)
(s · a · b,    Over p)          → Next(s · a · b · a, p)

-- Quotation algebra
(s · Quote(q),           Apply p) → Next(s, q ++ p)          -- "i"
(s · Quote(a) · Quote(b), Cat  p) → Next(s · Quote(a ++ b), p)
(s · v · Quote(q),        Cons p) → Next(s · Quote(Push(v) :: q), p)
(s · a · Quote(q),        Dip  p) → Next(s, q ++ (Push(a) :: p))

-- Arithmetic (checked; result outside i64 → Fault(Overflow))
(s · Int(a) · Int(b), Add p)    → Next(s · Int(a+b), p)
  ... likewise Sub, Mul

-- Division & remainder: PINNED to truncating (Rust) semantics,
-- matching i64::checked_div / checked_rem exactly:
--   trunc_div(-7, 2) = -3 (not Euclidean -4); remainder sign follows dividend
--   b = 0                  → Fault(DivByZero)         (both Div and Mod)
--   a = i64::MIN, b = -1   → Fault(Overflow)          (both Div and Mod —
--     checked_rem(MIN,-1) is None even though the math remainder is 0;
--     the spec models the exec truth)
(s · Int(a) · Int(b), Div p)    → Next(s · Int(trunc_div(a,b)), p)
(s · Int(a) · Int(b), Mod p)    → Next(s · Int(trunc_mod(a,b)), p)

-- Comparison (result Int(1) or Int(0))
(s · Int(a) · Int(b), Eq p)     → Next(s · Int(a==b), p)
(s · Int(a) · Int(b), Lt p)     → Next(s · Int(a<b), p)

-- Branch
(s · Int(c) · Quote(t) · Quote(f), If p)
    → Next(s, t ++ p)   if c ≠ 0
    → Next(s, f ++ p)   if c = 0
```

Any pattern not matched above with the required arity/types faults with `Underflow` or `TypeMismatch`. **No rule is partial; no rule panics.**

### 4.2 Key semantic property

`Apply` splices the quotation into the continuation rather than recursing into a sub-interpreter. This makes the step relation *flat* — a single small-step transition system with no nested evaluation — which is what makes the Verus refinement proof tractable (§7). It also gives proper tail calls for free: a loop written as `dup apply` consumes no stack in the continuation.

### 4.3 Determinism

For every state `σ`, exactly one rule applies. Determinism is proof obligation **P1** (§7.3).

---

## 5. Primitive Set v0 and Glyph Assignment

Glyphs below are **provisional** — final assignment comes from the measurement protocol (§11). Stack effects in Forth notation.

| Glyph | Name | Stack effect | Notes |
|---|---|---|---|
| `[` `]` | quote | `( -- [q] )` | delimiters, not words |
| `:` | dup | `( a -- a a )` | |
| `_` | drop | `( a -- )` | |
| `~` | swap | `( a b -- b a )` | |
| `@` | rot | `( a b c -- b c a )` | |
| `^` | over | `( a b -- a b a )` | |
| `!` | apply | `( [q] -- ... )` | Kerby's `i` |
| `,` | cat | `( [a] [b] -- [ab] )` | |
| `;` | cons | `( v [q] -- [v q] )` | |
| `'` | dip | `( a [q] -- ... a )` | run q under top |
| `+` `-` `*` `/` `%` | arith | `( a b -- c )` | checked |
| `=` | eq | `( a b -- 0\|1 )` | |
| `<` | lt | `( a b -- 0\|1 )` | |
| `?` | if | `( c [t] [f] -- ... )` | |

**Deliberate inclusions beyond the minimal base.** `swap`, `rot`, `over`, `dip`, native ints, `if` are all derivable from the Kerby base `{dup, drop, cat, cons, apply}` — and we include them anyway, because derived forms cost 5–20 tokens *per use site* while a primitive costs ~1. This is the anti-tarpit principle applied consistently: **the primitive set is open, and admission is decided by corpus-level token accounting**, not by minimality aesthetics.

**v0.2 candidates** (admit if benchmarks justify): `times` (bounded loop), `map`/`fold` over a list value type, `pick`/`roll` generalized stack access, a `linrec`-style recursion combinator.

---

## 6. Turing Completeness

**Theorem (TC).** MTL with primitive set §5 is Turing complete.

**Proof route: simulation of 2-counter Minsky machines** (chosen over SKI translation because every step is directly checkable and the encoding uses no cleverness).

A Minsky machine is a finite list of instructions over counters `c1, c2`:
`INC(ci, next)` | `DEC_JZ(ci, next_if_zero, next_else)` | `HALT`.
2-counter Minsky machines are Turing complete (Minsky 1967).

**Encoding.**

1. **Counters** are the two integers at the bottom of the stack: state shape is `c1 c2 ⟨control⟩`.
2. **Unbounded iteration** comes from self-application. For any quotation body `B`:

   ```
   [ :[B]!' ... ] : !
   ```

   The idiom `: !` (dup, apply) applied to a quotation that re-duplicates itself yields unbounded recursion — MTL's Y combinator is two tokens. Because `!` splices into the continuation (§4.2), this loops in constant continuation space.
3. **Each instruction** becomes a quotation:
   - `INC(c1, j)`: `[ ['1+]'! Qj ]` — increment under the top, continue as `Qj`. (Concretely: bring `c1` up with stack ops, `1+`, restore, then run `Qj`.)
   - `DEC_JZ(c1, j, k)`: fetch `c1`, `:0=` , `[ restore; Qj ] [ 1-; restore; Qk ] ?`
   - `HALT`: `[]` (empty quotation → continuation empties → machine `Halt`s).
4. **Program counter** is which quotation currently occupies the continuation; the instruction table is finite, so each `Qi` is a fixed literal quotation.

**Simulation invariant.** Define `R(m, σ)`: Minsky configuration `m = (pc, c1, c2)` is represented by MTL state `σ` iff `σ.stack = [Int(c1), Int(c2)] ++ scratch` with empty scratch at instruction boundaries and `σ.cont = ⟦Q_pc⟧ ++ ε`. Then:

> **Lemma (lock-step):** if `m →_Minsky m'` then the MTL machine reaches, in a bounded number of steps, a state `σ'` with `R(m', σ')`; and the Minsky machine halts iff the MTL machine `Halt`s.

This lemma is finite case analysis over three instruction forms — each case is a fixed-length symbolic execution of the step rules in §4.1. It is proof obligation **P5** and is *mechanizable in Verus* as a spec-level theorem about the step relation, since both machines are pure spec functions. ∎

---

## 7. Verus Verification Plan

Structure follows TAVDD: the Verus spec is written and its spine proofs discharged **before** the production interpreter exists.

### 7.1 Architecture: ghost model + refinement

```
┌──────────────────────────────────────────────┐
│  spec fn spec_step(σ: SpecState) -> SpecStep │   pure math, Seq-based,
│  (transcription of §4.1, total)              │   the "math shadow"
└──────────────▲───────────────────────────────┘
               │ refinement: exec result == spec_step(view(σ))
┌──────────────┴───────────────────────────────┐
│  fn exec_step(vm: &mut Vm) -> StepResult     │   Vec-based, checked
│  ensures vm@ == spec-successor of old(vm)@   │   arithmetic, no panics
└──────────────────────────────────────────────┘
```

- Spec side: `SpecValue { Int(int), Str(Seq<char>), Quote(Seq<SpecWord>) }`, state as `(Seq<SpecValue>, Seq<SpecWord>)`. Recursive datatype with `decreases` on structural size.
- Exec side: `enum Value { Int(i64), Str(String), Quote(Vec<Word>) }` with a `View` impl mapping to spec values (i64 → int is where the overflow obligations surface).
- Driver: `fn run(vm, fuel: u64) -> Outcome` — a fuel-bounded loop. **We do not prove termination of `run`; TC forbids it.** We prove that `run` is a correct finite unrolling of `spec_step` up to `fuel`, and that `FuelExhausted` is the only outcome the spec doesn't determine.

### 7.2 Invariants ("impossible states are impossible")

- **I1 — Totality of step:** `spec_step` is a total function on all states (Verus enforces this by construction; no `arbitrary()`, no partial match).
- **I2 — No panics:** exec interpreter has no `unwrap`, no indexing without proof, no unchecked arithmetic. All Vec pops are guarded by length preconditions discharged from the match structure.
- **I3 — Value well-formedness:** every `Quote` contains a well-formed program (structural, by construction of the parser's postcondition).
- **I4 — Fault stability:** `Fault` states are terminal — no rule resumes from a fault.

### 7.3 Proof obligations (spine proofs first)

| ID | Statement | Kind |
|---|---|---|
| **P1** | Determinism: `spec_step` is a function (free — it's a `spec fn`, but we additionally prove the §4.1 rules are non-overlapping as documentation-grade lemma) | spec |
| **P2** | Refinement: `exec_step` faults exactly when `spec_step` faults, else `vm'@ == next state of spec_step(vm@)`; overflow in exec ↔ result outside i64 in spec | refinement |
| **P3** | Progress: every state is `Next`, `Halt`, or `Fault` — no stuck states | spec |
| **P4** | Parser round-trip: `parse(print(p)) == Ok(p)` and `parse` postcondition establishes I3 | exec |
| **P5** | TC lock-step lemma (§6) over the spec step relation | spec, hard |
| **P6** | Tail-call space bound: for programs in loop normal form `Q = [... :!]`, continuation length is bounded across iterations | spec, v0.2 |
| **P7** | Heap acyclicity: the value heap is a DAG in every reachable state (§14.3) | spec |
| **P8** | No leaks: every value pushed is eventually consumed, dropped, or in the final stack; the refcount model is exact (§14.3) | spec |
| **P9** | Checker soundness: statically checked programs never fault with `Underflow`, `TypeMismatch`, or resource misuse, and never leak (§14.5) — progress + preservation over multiplicity-typed stacks | spec, headline |

If P2 gets hard, that's the design-smell signal: refine the representation (e.g., continuation as a persistent list vs. Vec splice) before fighting the prover.

**Verification status (Verus 0.2026.07.05, `mtl_core.rs`, 10 queries, 0 errors):**
machine-checked as of this revision — P3 (progress); P1 (by construction, total non-overlapping match, no wildcard arm); truncating div/mod semantics via concrete witnesses *and* the general correctness lemma (`a = q·b + r`, `|r| < |b|`, remainder sign follows dividend) discharged with `nonlinear_arith`; deep-view termination through nested quotations (lexicographic datatype-height measure); and a smoke theorem that the two-token Y idiom `:!` self-applies in exactly two spec steps, retaining the quotation while splicing its body into the continuation. Open holes: P2 (needs the GREEN interpreter), P5 (needs the Minsky spec machine), P6–P9 (scheduled).

### 7.4 Test layer (RED/GREEN — complements, not replaces, proofs)

Proofs cover what we modeled; property tests poke at what we forgot to model:

- **Happy path:** golden programs (factorial, Fibonacci, the Minsky simulator itself) with expected final stacks.
- **Boundary:** empty program, deeply nested quotations, `i64::MIN / -1`, `i64::MIN % -1`, quotation catenation at size limits.
- **Property (proptest):** (a) fuzz arbitrary programs — interpreter never panics, always returns in ≤ fuel steps (this re-checks I2/P3 against the *actual binary*, catching spec/exec transcription gaps); (b) differential testing exec vs. a naive unverified oracle interpreter; (c) `parse ∘ print` round-trip on arbitrary ASTs (re-checks P4).
- **Regression:** every bug found by Havoc-style fuzzing lands as a named test before the fix commits.

Coverage gate 85–90%; criterion benches on the step loop.

---

## 8. Effects: Host-Injected Capabilities

The core is pure — `spec_step` closes over nothing. Effects enter only as **named words bound by the host** at VM construction:

```
Vm::new(program).with_word("emit", host_fn)  // ( v -- )
```

- Unknown named word → `Fault(UnknownWord)`. The verified core's theorems are unconditional; host words are trusted boundary, documented as such.
- Capability style: an MTL program can only affect what its host explicitly granted. This makes MTL programs safe to run when *generated by agents* — which is the point.

---

## 9. Definitions (v0.1, restricted)

```
#f[...]        -- bind quotation to single-letter name f
```

- Names are **single lowercase letters** in v0: a multi-char name costs ≥2 tokens per use site and its glyph budget is better spent on primitives.
- A definition is sugar: `f` ≡ `Push(Quote(body)) Apply`. No recursion through the name table needed — recursion is `: !` (§6.2) — so definitions never complicate the semantics or the proofs.

---

## 10. Benchmark Suite (define before optimizing)

Task distribution `T`, three tiers, each task specified as (input stack, expected output stack or effect trace):

1. **Micro (20 tasks):** arithmetic pipelines, stack shuffles, predicates, min/max, gcd, factorial, fib.
2. **Algorithmic (15 tasks):** list fold/map (via quotation encoding or v0.2 list values), sorting a small list, prime sieve, string reverse, run-length encoding, the Minsky simulator.
3. **Agentic (10 tasks):** capability-driven — "read value via host word, transform, emit"; small state-machine policies; the shape of Kairos-style skill bodies.

Recorded per task, per language (MTL, Python, jq where applicable, Forth as a concatenative control):
`tokens(solution)` under each tokenizer × `agent success rate` (N attempts, fixed model, fixed prompt scaffold) × `attempts-to-first-correct`.

**The real headline metric is `E[tokens × attempts]`** — token-cheap but unwritable loses.

## 11. Glyph Assignment Protocol (Abrash-style measurement)

1. Write the benchmark solutions using placeholder primitive names.
2. Enumerate candidate glyph assignments (single ASCII punctuation, plus short names for anything that misses).
3. For each assignment, render the full solution corpus and count tokens under each tokenizer — **corpus-level, not per-glyph**, because BPE merging is context-dependent (`:!` may be 1 token; `:?` may be 2).
4. Frequency-weighted optimization: assign the most merge-friendly bigrams to the most frequent primitive *pairs* in the corpus (measure pair frequencies first).
5. Freeze assignment; re-run whenever the primitive set changes. The script lives in-repo; assignments never change without a measurement diff in the ADR.

---

## 12. Open Questions

1. **List/record values in core vs. quotation-encoded** — quotation encoding is elegant and proof-cheap but token-expensive per access; measurement decides (v0.2).
2. **Static arity/type checking** — subsumed by the linearity checker (§14): the multiplicity-typed stack-effect checker is a strictly stronger validator, converting more runtime faults into pre-execution errors at zero token cost.
3. **Continuation representation** — Vec splice (`q ++ p`) is O(n) per apply; a cons-list or rope may be needed. Must not disturb P2; do representation change spec-first per TAVDD.
4. **String primitives** — none in v0; admit via benchmark pressure only.
5. **Whether `'` (dip) merges badly** — apostrophe adjacency to `]` is tokenizer-hostile in early checks; may swap glyph with a lower-frequency primitive.

---

## 13. Roadmap

| Phase | Deliverable | Gate |
|---|---|---|
| SPEC | This document + `mtl_core.rs` Verus spec skeleton | review |
| PROOF | P1–P4 verified; P5 stated with lock-step lemma skeleton | `verus` green |
| RED | Golden + boundary + proptest suites (failing) | tests exist, fail |
| GREEN | Exec interpreter passing tests, P2 discharged | tests + proofs green |
| REFACTOR | Continuation representation tuning under green lights | benches |
| CHECK | §14 multiplicity checker + P7–P9 | `verus` green on P9 |
| MEASURE | §10 suite vs. Python; §11 glyph freeze | ≥3× or write the post-mortem |

---

## 14. Linearity and Memory Model — Rust-Grade Safety, GC-Free, Zero Token Cost

### 14.1 The structural observation

Rust needs a borrow checker because **names create aliases**: multiple variables can reach one value, so ownership must be tracked through binding structure. MTL has no binders. The only way a value is aliased is `:` (dup) / `^` (over); the only ways it dies are `_` (drop) or consumption by a word. Therefore **every MTL program already contains its ownership operations in the program text**.

This is precisely the structure Koka's *Perceus* algorithm infers — precise compile-time dup/drop insertion yielding GC-free reference counting. In MTL, the inference step is the identity: the program *is* its ownership trace. Point-free concatenative code is accidentally a linear language with explicit structural rules (dup = contraction, drop = weakening).

**Design consequence:** memory safety is enforced by a *static checker over unmodified programs*, not by syntax. The default path costs **zero additional tokens** — the multiplicity information lives in primitive signatures and the checker, never in program text.

### 14.2 Multiplicity discipline

Every stack type carries a multiplicity:

| Multiplicity | May `:` / `^` | May `_` | Must be consumed |
|---|---|---|---|
| **unrestricted** (`Int`) | yes | yes | no |
| **affine** (`Quote`, `Str`) | yes (refcounted) | yes | no |
| **linear** (host resources, v0.2) | **no** — check-time error | **no** — implicit drop is an error | **yes, exactly once** (e.g. an explicit `close`-style capability word) |

- Words are linear function signatures: inputs are consumed, outputs are produced. This is already how §4.1 is written — no rule copies a value implicitly.
- Linear resources get Rust move semantics *as a restriction rather than an annotation*: `:` on a file handle is rejected before execution. Zero tokens, because prohibitions are free.
- The linear tier is what makes agent-generated Kairos-style skills safe to run unattended: "leaked forty handles" becomes "validator rejected the skill."

### 14.3 Heap model: acyclic, refcounted, deterministic — not GC

v0 values are immutable, and the only constructors (`;` cons, `,` cat, `[...]` literal) build new values from existing ones. Back-edges are unconstructible, so:

> **P7 (acyclicity):** in every reachable state, the value heap is a DAG.

Acyclic + refcounted = exact, deterministic destruction at last drop/consume — the pathological case for reference counting (cycles) is *provably impossible*, so this is no more "garbage collection" than Rust's own `Rc`. Freeing is deterministic and prompt, preserving the predictability story (no pauses, no tracing).

> **P8 (no leaks):** every value pushed is eventually consumed, dropped, or present in the final stack; the spec-level refcount model is exact (count = number of stack/continuation/heap references).

Both are spec-level invariants over an instrumented heap model in Verus — the exec interpreter then refines them via P2, giving a *verified* claim that the runtime neither leaks nor double-frees.

### 14.4 Mutation and borrows without syntax (v0.2)

- **Uniqueness typing does `&mut`'s job.** A mutation word (e.g. buffer write) requires its target to be *statically unique* — refcount provably 1 — which the multiplicity checker establishes. Unique ⇒ in-place mutation is unobservable ⇒ safe. This is Clean's uniqueness typing and Perceus's reuse optimization; no borrow syntax exists because exclusive access is a stack position, not a name.
- **`'` (dip) is a scoped borrow.** The value leaves the stack, the quotation runs, the value returns; the checker verifies the quotation cannot have captured it (no escape ⇒ the "borrow" ends by construction). `^` (over) is a shared borrow that the duplicable tiers pay for with a refcount increment.
- **Copy-on-write fallback:** a mutation word applied to a non-unique affine value either faults (strict mode) or clones then mutates (COW mode) — a per-word decision made by token accounting, as usual.

### 14.5 The checker

A **linear stack-effect checker**: abstract interpretation over stacks of multiplicity-annotated types, run pre-execution by the validator.

- **Literal quotations** (the overwhelmingly common case in benchmark and agent-generated code): fully static — the checker recurses into `[...]` bodies, joins branches at `?`, and verifies loop bodies in `:!` normal form preserve their stack-type signature.
- **Runtime-composed quotations** (`,` / `;` on non-literal operands): the known hard problem in typing concatenative languages (cf. Cat, Kleffner). MTL's answer is **gradual**: at truly dynamic composition points, either a checker-visible effect annotation or a deferred runtime multiplicity check. The escape hatch costs tokens *only where dynamism is actually used* — the metric and the type system have aligned incentives.

> **P9 (checker soundness) — headline theorem:** if `check(p) = Ok`, then no state reachable from `(ε, p)` faults with `Underflow`, `TypeMismatch`, or resource misuse, and P8's no-leak property holds unconditionally. Proved as progress + preservation over the multiplicity-typed stack relation; tractable in Verus because the checker and `spec_step` are both pure spec functions over the same datatypes.

P9 is also the metric's best friend: it converts runtime faults into pre-execution validator errors, directly raising agent success rate and protecting `E[tokens × attempts]` (§10).

### 14.6 Token accounting summary

| Feature | Token cost |
|---|---|
| Ownership / moves / drops | 0 — already in program text (`:`, `_`, consumption) |
| Lifetimes, `&`, `mut`, annotations | 0 — do not exist |
| Linear resource discipline | 0 net — explicit `close` words you'd write anyway |
| Scoped borrows | 0 — `'` and `^` already exist |
| Dynamic-composition escape hatch | >0, rare, self-punishing — paid only where used |

**Scope honesty:** interpreter memory safety was already guaranteed (verified Rust). §14 makes *MTL programs themselves* memory-safe as a language property — free correctness in pure v0, load-bearing the moment resources and mutation arrive in v0.2.
