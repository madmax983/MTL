# MTL — A Language Co-Designed for Model Tokenizers and Formal Verification

**A capstone research report.** This document is the definitive statement of what MTL
(Minimal Token Language) claims and why. Every numeric claim cites its source artifact
inline as a relative repository path. Where this paper and the top-level `README.md`
disagree, this paper cites the current merged proof and benchmark artifacts and
supersedes the README; the specific stale-README points are flagged in §5 and §11.

The sharp question this report answers, stated verbatim from the 2026-07-11 adversarial
review (`docs/reviews/2026-07-11-adversarial-review.md`, §Bottom line):

> "Can a language co-designed for model tokenizers and formal verification reduce total
> inference cost without reducing agent reliability?"

The honest one-paragraph answer, which the rest of this report substantiates:
**reliability is preserved** — 100% pass@5 agent writability on unseen held-out tasks,
with no measurable read-tax; **total inference cost is NOT reduced in the cold,
single-session regime** — the session-economics break-even point N* is structurally
unreachable within N ≤ 16 because the one-time preamble / read-tax cost dominates; but
**per-solution token economics favor MTL** — a held-out correct-solutions-per-million-tokens
(CSPM) ratio of 2.124× (`bench/BASELINE-SEALED.md`). The general-compression thesis (a
≥3× token reduction against idiomatic Python) **did not hold out of sample and has been
formally retired**; compression survives only as a niche property. What survives as
machine-checked, out-of-sample contributions are **verified capability confinement** and
**agent reliability**.

---

## 1. Abstract

MTL is a concatenative, point-free, stack-based language whose intended writers are
language-model agents, not humans. Its 25 glyphs are single ASCII characters chosen so
that BPE tokenizers merge adjacent primitives, pushing effective cost below one token per
primitive. We asked whether co-designing a language for tokenizers and for formal
verification could lower total inference cost without lowering agent reliability. The
answer, measured out of sample, is mixed and reported plainly. **Reliability is
preserved**: cold agents write correct MTL at 100% pass@5 on a blind held-out set,
matching Python, with no measurable read-tax (`bench/BASELINE-SEALED.md`,
`bench/agent-trial/readtax/round2/results/REPORT.md`). **Total cost is not reduced in the
cold single-session regime**: the break-even session length N* is structurally unreachable
within N ≤ 16 because the cold preamble dominates (`bench/agent-trial/sessions/REPORT.md`).
**Compression is a niche property, not a headline**: the original ≥3× gate held only in
sample (3.72×–3.92×, `bench/BASELINE.md`, `bench/BASELINE-TIER2.md`) and collapsed to
1.67× on the first blind held-out set (`bench/BASELINE-SEALED.md`); it was formally retired
(PR #95, commit `3b9e20a`). What survives is **per-solution economics** (held-out CSPM
2.124×) and a **machine-checked core**: five Verus proof roots totaling 76 + 118 + 101 +
116 + 145 = 556 verified obligations with 0 errors, 322 workspace tests passing, and a
trusted boundary of exactly two `Clone` stubs (`crates/mtl-arena/proof-log.txt`). The
verified contributions include Turing-completeness, interpreter refinement, parser
round-trip, static-checker soundness, an arena-backend refinement proof, and
capability confinement enforced by the runtime independent of agent behavior.

---

## 2. Introduction

### 2.1 The sharp question

MTL sits at the intersection of two bets. The first is a tokenizer bet: BPE tokenizers
merge runs of adjacent ASCII punctuation into single tokens, so a whitespace-free program
written entirely in single-character glyphs can cost *fewer tokens than it has
primitives*. The second is a verification bet: a small-step, flat operational semantics
whose entire state is `(stack, continuation)` is tractable to machine-check in
[Verus](https://github.com/verus-lang/verus), so the language can carry a formal ground
truth from birth. The research question that unifies both bets is the one the adversarial
review named as "the sharp paper" (`docs/reviews/2026-07-11-adversarial-review.md`):

> "Can a language co-designed for model tokenizers and formal verification reduce total
> inference cost without reducing agent reliability?"

### 2.2 The honest arc, in miniature

This report's spine is a negative result reported as plainly as the positive ones. In
sample, MTL cleared its own ≥3× compression gate: T_v0 reached 3.72× and tier-2 reached
3.87×/3.92× against idiomatic Python (`bench/BASELINE.md`, `bench/BASELINE-TIER2.md`). The
first out-of-sample measurement — a blind sealed set the language authors never saw —
collapsed that number to 1.67× (o200k) / 1.72× (cl100k) over 14 algorithmically-correct
tasks (`bench/BASELINE-SEALED.md`), below the gate and below the in-sample figure. A broad
generated distribution of 1,145 shapes reproduced the collapse at ~1.70× train / ~1.72× dev
(`docs/design/v0.8-generalization.md`). Against fair terse Python the ratio is ~1.03×
(`docs/design/v0.8-generalization.md`). The ≥3× "Abrash gate" was formally retired (PR #95,
commit `3b9e20a`).

What did not collapse: agent writability held at 100% pass@5 on the held-out set, equal to
Python; the marginal per-solution CSPM ratio actually *widened* out of sample from 1.274
(dev) to 2.124 (sealed) (`bench/BASELINE-SEALED.md`); and the five machine-checked proof
roots are unaffected by any benchmark outcome (`crates/mtl-arena/proof-log.txt`). The
restated thesis is therefore "MTL is a machine-checked, agent-writable, capability-confined
language that is marginally tighter per solution," not "MTL compresses ≥3× on unseen tasks."

### 2.3 Contributions

The negatives and the methodological finding below are first-class contributions, not
caveats.

1. **A verified core (positive).** Five Verus proof roots — determinism, interpreter
   refinement, progress, parser round-trip, Turing-completeness, static-checker soundness,
   and arena refinement — machine-checked at a pinned toolchain, with an explicit two-stub
   trusted boundary (§5).
2. **Verified capability confinement (positive).** The effect boundary is the trust
   boundary; confinement is enforced by the runtime and proven by test, independent of
   whether the agent tries to escape it (§6.8).
3. **Agent reliability (positive, out of sample).** 100% pass@5 writability held out, no
   measurable read-tax, and a widening per-solution CSPM edge (§6.4, §6.5, §6.7).
4. **The general-compression thesis failed out of sample (negative).** The ≥3× gate was an
   in-sample artifact and is retired; compression is a niche property (§6.1–§6.3).
5. **The co-evolution finding (methodological).** The in-sample 3.7–3.9× was inflated
   because the benchmark corpora and the language primitives were tuned together. This is a
   citable, general warning for anyone benchmarking LLM-targeted DSLs: measure on a sealed
   set the language authors never saw (§3.4, §6.3).
6. **Total cold-session cost is not reduced (negative).** N* is structurally unreachable
   within N ≤ 16; the warm/fine-tuned arm that would reach N* → 1 is specified but not yet
   run (§6.6).

---

## 3. Design method

MTL was built by a measurement-driven, review-absorbing process. The process discipline is
itself part of the evidence, so it is documented here rather than assumed.

### 3.1 Measurement-driven primitive admission

The standing admission rule is an anti-tarpit rule: a primitive is admitted only if
token accounting on the admission corpus shows it pays for itself in corpus-level token
savings (`docs/mtl-spec.md` §1.2). This is how the recursion primitives (`primrec`,
`times`, `linrec`, `uncons`) and the sequence primitives (`fold`, `xor`) entered — each
under demonstrated token pressure. It is also why indexed access, strings-in-core, and
`#f[...]` definitions were declined: they did not pay (§7). The rule is honest about its own
hazard — a benchmark tuned alongside the primitives it admits will over-reward those
primitives — and that hazard is exactly what the co-evolution finding (§3.4, §6.3) later
made concrete.

### 3.2 TAVDD and the reference-first workflow

Development followed a test-and-verify-driven discipline (SPEC → PROOF → RED → GREEN →
REFACTOR): the reference semantics `spec_step` is authoritative, and where prose and
`spec_step` disagree the prose is the defect (`docs/mtl-spec.md` preamble). Every corpus
solution is executed on the reference interpreter, not hand-traced; the design record's
repeated lesson, stated across `bench/BASELINE-TIER2.md` and
`docs/design/v0.6-indexed-access.md`, is "no hand-traced number survives contact with the
interpreter." The v0.6 indexed-access spike is the sharpest example: v0.3 design estimates
were ~1.9–2.4× optimistic, and the program that produced the optimistic binary_search figure
did not even solve the task (`docs/design/v0.6-indexed-access.md` §2.2).

### 3.3 The six-way primitive mirror and single-source-of-truth codegen

The architecture review named semantic drift — one declaration of the primitive set
silently disagreeing with another — as the primary remaining engineering risk
(`docs/reviews/2026-07-13-architecture-review-gpt55.md`). The response is a single canonical
table, the `for_each_primitive!` x-macro in `crates/mtl-syntax/src/manifest.rs`, holding the
23 rows `(index, Name, glyph, arity, stack_effect)`
(`docs/design/primitive-mirror-codegen.md`). Of the six named mirror surfaces, 4 of 6 are
generated from the manifest (parser `Prim` enum, `GLYPHS` + glyph conversions, and the two
`conv` opcode maps), meeting the ≥4/6 target. The interpreter `Prim` enum and the Verus
`SpecPrim` enum are deliberately kept hand-written and comparison-policed — the interpreter
so the differential oracle "has something to disagree with," and the Verus core so it gains
no codegen/proc-macro dependency. The arena `Prim` mirror is a policed seventh, kept
independent for the same differential-oracle reason
(`docs/design/primitive-mirror-codegen.md`, `docs/design/v0.5-refactor.md` §5).

### 3.4 Adversarial-review absorption as process

The 2026-07-11 adversarial review (`docs/reviews/2026-07-11-adversarial-review.md`) is the
decision record's backbone. It invalidated the original Turing-completeness argument (which
represented each Minsky counter as one bounded `i64`, giving a finite state space) and drove
the repair to unary-quotation-encoded counters, which required admitting `uncons` as a
quotation destructor (§5.3). It forced unsigned integer literals to remove lexer ambiguity
(`1-2` had two readings), pinned fault precedence (arity → type → semantic), adopted the
two-machine pure-core/host split, split the §14 memory claims into P8a/b/c + P9, and drove
the corpus-splitting and CSPM-as-headline discipline that the sealed evaluation later
executed. The review is where the co-evolution warning originates: "primitives get
introduced in response to tasks and evaluated on the same tasks — benchmark-fitting
masquerading as general compression" (§14), and "split the suite: glyph-training corpus,
primitive-admission corpus, development set, sealed evaluation set" (§16). The v0.8
generalization round (§6.3) is that warning, measured.

Two follow-up reviews closed the loop. The P5 review upgraded the Turing-completeness
construction from "credible executable sketch" to a discharged theorem
(`docs/reviews/2026-07-13-p5-review-gpt55.md`), and the architecture review confirmed the
three-layer correspondence story (surface AST → Verus exec model → ghost semantics → Minsky
machine) and named drift as the primary residual risk
(`docs/reviews/2026-07-13-architecture-review-gpt55.md`).

---

## 4. The language

MTL is a concatenative, point-free stack language with no variables and no environments;
computation is a sequence of self-delimiting single-character words applied to an implicit
stack (`docs/mtl-spec.md` §2, §5). The value domain is minimal: `Value = Int(i64) |
Quote(Program)`, with booleans encoded as integers (0 is false). Integer literals are
**unsigned** — a leading `-` is always the `Sub` primitive, resolving the lexer ambiguity
the adversarial review flagged (`docs/mtl-spec.md` §2.1). Host-side effects live entirely
outside the pure core (§6.8).

There are 25 glyphs: 23 single-character word-primitives plus the `[` and `]` quotation
delimiters (`docs/mtl-quickref-min.md`). Grouped by role:

- **Stack:** `:` dup, `_` drop, `~` swap, `@` rot, `^` over
- **Control / apply:** `!` apply, `'` dip, `?` if
- **Data:** `,` cat, `;` cons, `>` uncons
- **Arithmetic:** `+ - * / %` (division and mod truncate toward zero; `b=0` faults)
- **Comparison / bitwise:** `=` eq, `<` lt, `$` xor
- **Recursion:** `&` primrec, `.` times, `|` linrec, `(` fold

The glyphs are deliberately chosen so BPE tokenizers merge adjacent primitives; the
self-application idiom `:!` lexes as `:` `!` but frequently tokenizes to one token. Faults
are typed and halt with no partial result — Underflow, TypeMismatch, Overflow, DivByZero,
plus FuelExhausted — with a pinned precedence of arity, then types, then
DivByZero/Overflow (`docs/mtl-quickref-min.md`). The full table with stack effects is in the
specification (`docs/mtl-spec.md` §5); it is not reproduced here.

---

## 5. Verification

All verification is machine-checked in Verus at the pinned release
`0.2026.07.05.49b8806` with Z3 4.12.5 (`crates/mtl-core/proof-log.txt`,
`crates/mtl-arena/proof-log.txt`). The authoritative counts below are the re-verification
block in `crates/mtl-arena/proof-log.txt`, which re-runs every root in one session. These
are the current merged numbers; where the README or the 0.4.0 release notes cite smaller,
older counts, this paper supersedes them (§5.6).

### 5.1 The five proof roots

| Root | Artifact | Result | Proves |
|---|---|---|---|
| Core | `crates/mtl-core/src/mtl_core.rs` | **76 verified, 0 errors** | P1 determinism, P3 progress, P2 refinement via the exec twin |
| Universality | `crates/mtl-core/src/p5_universality.rs` | **118 verified, 0 errors** | P5 Turing-completeness (two-counter Minsky, unary-quotation counters) |
| Round-trip | `crates/mtl-syntax/proofs/p4_verus.rs` | **101 verified, 0 errors** | P4 parser/printer round-trip |
| Checker | `crates/mtl-core/src/checker_verus.rs` | **116 verified, 0 errors** | Layer-C static-checker soundness (T-Static, T-Progress, T-Branch) |
| Arena | `crates/mtl-arena/proofs/arena_verus.rs` | **145 verified, 0 errors** | Arena refinement α(arena_step) = spec_step(α) |

Alongside the proofs, `cargo test --workspace` reports **322 passed, 0 failed** in the current
in-container reproduction (the checked-in `crates/mtl-arena/proof-log.txt` records an earlier
275-test run; more tests have since landed on main, none failing), and the P5 executable
validation suite `crates/mtl-core/tests/p5_minsky.rs` reports **6 passed**
(`crates/mtl-core/tests/p5_minsky.rs`).

### 5.2 The zero-cheat statement and the two-stub trusted boundary

The only trusted boundary in the entire proof stack is exactly two `Clone` `external_body`
stubs — `Word::clone` and `Value::clone` — which exist because Verus rejects a derived
`Clone` on the recursive quote type. The audit command `verus --no-cheating` flags exactly
these two and nothing else (`README.md` §Honest boundaries, corrected count in
`crates/mtl-arena/proof-log.txt`). P2 refinement, which was formerly `admit()`-stubbed with
seven `external_body` exec functions, is now fully discharged: the exec twin's
`exec_step`/`exec_prim`/`exec_arith`/`exec_divmod`/`exec_cmp`/`run`/`value_to_exec_word`
functions are verified, and the interpreter provably faults exactly when the spec faults and
otherwise reaches the same next state (`docs/RELEASE-NOTES-0.4.0.md`; the core count moved
from 22 to 76). The arena proof carries zero cheats — no admit/assume/external_body on any
load-bearing lemma (`crates/mtl-arena/proof-log.txt`).

**Note on a stale in-repo artifact.** `crates/mtl-core/proof-log.txt` still describes P2 as
an `admit()`-stubbed open hole with seven `external_body` exec functions. That file is stale;
the release notes, the re-verification block in `crates/mtl-arena/proof-log.txt`, and the
76/0 count all record P2 as discharged. This paper uses the discharged status (§5.6).

### 5.3 P5 — the Minsky construction

P5 establishes Turing-completeness by lock-step (more precisely, bounded-stutter)
simulation of a two-counter Minsky machine, with each counter encoded as a unary quotation
whose *length* carries the counter's magnitude — genuinely unbounded storage in the abstract
`Seq`-based semantics, not the finite `i64` the original withdrawn proof used
(`crates/mtl-core/src/p5_universality.rs`, `docs/RELEASE-NOTES-0.4.0.md`). The proof has six
theorems including a two-way fuel-quantified halting correspondence: forward simulation,
representation preservation, divergence preservation, and reverse halting derived through the
divergence theorem so there is no spurious early halt (`docs/reviews/2026-07-13-p5-review-gpt55.md`).
The external P5 review confirmed the route is valid and admit-free, correcting only wording
("executable validation," not "empirical"; "bounded-stutter," not "lock-step"). One simulated
Minsky step costs O(program length) MTL steps.

### 5.4 Layer-C checker soundness

The static multiplicity / stack-effect checker is a total function returning one of three
judgments — Static, Guarded, or Reject — over the literal-quotation fragment
(`docs/design/v0.6-checker.md` §1). Three theorems are machine-checked in
`crates/mtl-core/src/checker_verus.rs` (116 verified, 0 errors): T-Static (no reachable
Underflow/TypeMismatch, and the halt stack satisfies the inferred effect), T-Progress
(preservation + progress), and T-Branch (branch-stack compatibility at every `?`). The
mechanized fragment covers straight-line code plus If, Times, PrimRec, homogeneous-Int Fold,
and deterministic literal Uncons, lifted to the full row-polymorphic effect with non-empty
`pre`.

**Note on an internal denominator inconsistency.** The v0.6 prototype corpus-acceptance
table reports 27 Static / 39 Guarded / 13 Reject over a **79-program** corpus
(`docs/design/v0.6-checker.md` §4), while the T-Static mechanization note in the same
document reports "27 of the 87 corpus programs mechanized-Static" (§3). The numerator is 27
in both; the denominator differs (79 vs 87), consistent with the corpus having grown between
the prototype run and the mechanization. This paper cites the mechanized figure — **27 of 87
corpus programs mechanized-Static** — as the current proof-backed number, and flags the §4
prototype's 27/79 as the earlier, smaller-corpus count (§5.6).

### 5.5 Arena refinement and the proof-to-production boundary

The arena backend refines the frozen `spec_step` semantics through an abstraction function
α: `α(arena_step(s)) = spec_step(α(s))` as an unconditional one-step theorem for all 23
primitives plus the non-prim cases, with full fault parity, the u32-capacity → Fault::Overflow
characterization, and a multi-step driver corollary proved by induction on fuel
(`crates/mtl-arena/proofs/arena_verus.rs`, 145 verified, 0 errors,
`crates/mtl-arena/proof-log.txt`). The arena proof pulls `mtl_core.rs` in unmodified and
re-verifies it alongside, so it is pinned to the same discharged `spec_step`.

The proof-to-production boundary must be stated precisely, because it is **not extraction.**
The verified core is a Verus *reference semantics*. The production interpreter, printer, and
parser are shipped as byte-identical, oracle-pinned *twins* of the proven code: they are held
to the reference by a continuously-run differential oracle (interp-vs-arena, 148 cases across
both engines), fault-corpus parity, and host-driver parity (`README.md` §Arena backend,
`docs/design/v0.5-refactor.md` §7). This is the "mirrored twin" discipline (P4c): the proven
model is one artifact and the production code is a separate artifact pinned to it by
differential testing, not by in-tool extraction. The architecture review characterizes the
accurate trust diagram as "ghost semantics ↑(formally proved) Verus exec twin ≈(differential
tests) production Rust interpreter" — a real but narrower gap than "the interpreter is
unverified" (`docs/reviews/2026-07-13-architecture-review-gpt55.md`). The same review notes
that the production interpreter's "never panics" claim is stronger than the stable Rust source
alone establishes (it contains guarded `expect`/`unreachable!` arms), a residual point the
paper records rather than smooths.

### 5.6 Where this paper supersedes the README and release notes

Three in-repo records are internally inconsistent or stale; this paper cites the current
merged value and states which and why:

- **P4 count.** `README.md` and `docs/RELEASE-NOTES-0.4.0.md` cite `p4_verus.rs` at **42
  verified, 0 errors** and P4 "at model level (b)." The current merged root is **101 verified,
  0 errors** with P4 upgraded to full round-trip (level (c)) via the mirrored twin
  (`crates/mtl-arena/proof-log.txt`, `docs/design/v0.6-indexed-access.md` §1). This paper uses
  101/0.
- **P2 status.** `crates/mtl-core/proof-log.txt` describes P2 as `admit()`-stubbed; the
  release notes and the arena re-verification block record it discharged at core 76/0. This
  paper uses discharged.
- **Checker corpus denominator.** `docs/design/v0.6-checker.md` reports both 27/79 (prototype
  table) and 27/87 (mechanization note). This paper uses 27/87.

---

## 6. Results — the honest arc

Each subsection leads with the number and its artifact. The negatives (6.2, 6.3, 6.6) are
presented as prominently as the positives.

### 6.1 In-sample compression (positive, but in-sample)

On the frozen T_v0 micro corpus, aggregate static compression rose from **2.11× (v0.1) to
3.72× (v0.2)** against idiomatic Python under both o200k and cl100k (o200k_base = cl100k_base
on these strings) (`bench/BASELINE.md`). On the tier-2 dev corpus, the sequence primitives
`(` (fold) and `$` (xor) lifted the aggregate from ~1.9× to **3.87× (o200k) / 3.92× (cl100k)**
over 11 tasks (`bench/BASELINE-TIER2.md`). Both cleared the ≥3× gate. Both are in-sample: the
corpora co-evolved with the primitives.

### 6.2 The held-out collapse (negative — the results spine)

The first out-of-sample measurement is a 15-task sealed set authored blind (task semantics
only, no spec/quickref/corpus access), frozen before any solution existed, and unsealed
exactly once (`bench/BASELINE-SEALED.md` §2). On the 14 algorithmically-correct tasks, static
compression is **1.67× (o200k) / 1.72× (cl100k)** — below the ≥3× gate and below the in-sample
3.72–3.92× (`bench/BASELINE-SEALED.md` §1, §3). Every tier fell: micro 1.76×, tier-2 1.76×,
tier-3 1.44× (o200k). Two sealed micro tasks are where MTL is at or below parity —
`seal_digit_product` at 1.04× and `seal_count_set_bits` at **0.73×, where Python wins outright**
via `bin(n).count("1")` (`bench/BASELINE-SEALED.md` §3, §6). The blind set also surfaced one
genuine algorithmic bug the text harness had hidden (`seal_running_max` seeded at 0, wrong on
all-negative input), so 14 of 15 tasks are algorithmically correct (`bench/BASELINE-SEALED.md`
§7). Per the issue's MEASURE gate, an honestly-measured miss with a post-mortem is the
successful deliverable, and nothing in the frozen language was changed to chase the number
(`bench/BASELINE-SEALED.md` §1).

### 6.3 The co-evolution finding (methodological, first-class)

The in-sample 3.7–3.9× was inflated because the dev and admission corpora co-evolved with the
primitives: the benchmark and the language were tuned together, so the cheap dev wins lean on
`(` and `$`, admitted under dev-corpus token pressure (`bench/BASELINE-SEALED.md` §6,
`docs/design/v0.8-generalization.md` §0). This is a citable warning for anyone benchmarking
LLM-targeted DSLs: **measure on a sealed set the language authors never saw.** The v0.8 sink
taxonomy apportions the vanished 3× excess (the 141.3-token gap between the 319-token MTL
aggregate and the 177.7-token 3× budget): **≈59% stack-juggling** (sequence/state scans a
concatenative stack must rebuild each step), **≈23% missing-idiom / anti-niche** (terse Python
builtins MTL cannot beat), **≈10% accumulator-shape**, and **≈8% residual** near-3× loop tasks
(`docs/design/v0.8-generalization.md` §1). A broad generated distribution of 1,145 deduped,
oracle-verified shapes reproduces the collapse: **1.70× train / 1.72× dev** on the newly-covered
scan/bitdigit shapes, matching the sealed 1.67× across harnesses. The full-distribution 3.25× is
an artifact — **968 of 1,145 shapes are affine one-liners** that swamp the token sum
(`docs/design/v0.8-generalization.md` §2). Against fair terse Python (lambda one-liners using
stock builtins), the same 14 solutions give **~1.03×** (`docs/design/v0.8-generalization.md` §1).
Stack-juggling on scan/state tasks is intrinsic to the concatenative paradigm, not a missing
primitive; the one addressable sub-shape (windowed scans, via an aperture combinator) reaches
only ~2×, still short of 3×, and its cold-quickref + proof cost is not amortized by
0.17%-frequency windowed shapes, so v0.8 admitted nothing
(`docs/design/v0.8-generalization.md` §3, §4). The ≥3× Abrash gate was formally retired
(PR #95, commit `3b9e20a`).

### 6.4 Writability — 100% pass@5 (positive, out of sample)

Cold agents write correct MTL reliably. On the dev battery, both arms solved 30/30 cells within
the 5-attempt repair budget — **100% pass@5 (MTL) = 100% pass@5 (Python)**
(`bench/agent-trial/results/REPORT.md`). On the blind held-out set, the 7 text-feedable sealed
tasks again reached **100% pass@5 (21/21) in both arms** (`bench/BASELINE-SEALED.md` §4). MTL's
attempt-1 failures were all repaired within budget (3 dev cells, 100% repair rate; 3 sealed
cells, 100% repair rate). Writability is the property that held out of sample where compression
did not.

### 6.5 No measurable read-tax (positive, with a ceiling caveat)

Agents comprehend MTL as readily as they write it. On the round-2 (harder) read-tax battery,
the comprehension accuracy delta is **+0.0 points** through every tier A–D, and comprehension,
recall, mutation, and confabulation are all near 100% in both arms
(`bench/agent-trial/readtax/round2/results/REPORT.md`). Critically, the confabulation guard
shows **0.0% confab-rate on fault items in both arms** — a misread produces a loud typed fault,
not a plausible wrong answer. This is the structural contrast with pxpipe, whose image-compression
path produced **0/15 on verbatim retrieval as silent confabulations** (returning plausible-but-wrong
values rather than signalling uncertainty) (`docs/notes/related-work.md`). The honest caveat: the
read-tax accuracy metric is at a **ceiling** (both arms ~100%), so it cannot discriminate finer
than "no measurable tax," and the input-token cost of reading MTL is *higher* — the comprehension
prompt costs 11.5× the Python prompt in tokens (`bench/agent-trial/readtax/round2/results/REPORT.md`),
because the cold reader carries the quickref (§6.6).

### 6.6 Session economics, preamble ablation, and the warm/LoRA hypothesis (negative, with a path)

This is the negative that answers the "total inference cost" half of the sharp question. The
session-economics harness sweeps N ∈ {1, 2, 3, 5, 8, 16} and finds **N* = none within range for
both MTL arms** — the crossover where MTL's cost-per-correct drops below Python's is not merely
outside the grid, it is *structurally unreachable* (`bench/agent-trial/sessions/REPORT.md`). The
cause is one term: the 4,051-token cached quickref levies a per-task cache-read tax of ~0.61
US cents/task, which alone exceeds Python's entire ~0.28-cent cost-per-correct. MTL saves ~7
output tokens/task but must save ~81 just to offset the cached quickref, while simultaneously
paying ~8 extra input tokens/task — a >10× gap, invariant across all five price configurations
(`bench/agent-trial/sessions/REPORT.md` §Break-even).

The preamble ablation attacks exactly that dominant term. The winning minimal preamble,
`v4_compressed_minimal`, is **487 tokens (12% of the 4,051-token full quickref) with no
solve-rate loss** on the 10 pure tasks (`docs/mtl-quickref-min.md`,
`bench/agent-trial/preamble/REPORT.md`). With it, the cold input-token gap collapses from ~11.9×
to ~1.64× of the required break-even, turning a structurally unreachable ~12× shortfall into a
~1.3–1.6× near-miss — but still a miss on this saturated pure-task battery
(`bench/agent-trial/preamble/REPORT.md` §80 tie-in). Shrinking the preamble does not by itself
flip the verdict.

The path to N* → 1 is the warm/LoRA arm: fine-tune MTL competence into weights so the agent pays
zero quickref tax. This is presented as a **hypothesis with a recipe, not a result.** The recipe
selects Qwen2.5-Coder-7B (which preserves MTL's sub-1-token-per-glyph edge at 0.856 tok/glyph),
specifies a QLoRA data pipeline over the verified oracle, and shows the arithmetic under which
warm MTL clears Python from task #1 (`docs/design/v0.7-lora-warm-agent.md`). No training was run —
the environment has no GPU — but the data-factory (`bench/dataset/`) and the eval-harness reuse
are in-repo, and the fine-tune itself is tracked as issue #83
(`docs/design/v0.7-lora-warm-agent.md` §6).

### 6.7 Per-solution economics — CSPM 2.124× (positive, out of sample)

The marginal per-solution token edge is the positive economic result, and it *widened* out of
sample. The correct-solutions-per-million-tokens ratio (MTL/Python, charging failed repairs,
excluding the one-time quickref) rose from **1.274 (dev) to 2.124 (held-out sealed)**
(`bench/agent-trial/results/REPORT.md`, `bench/BASELINE-SEALED.md` §4, §5). On the held-out set,
MTL's marginal median is **19 output tokens/solution versus Python's 57** (3.0× tighter on the
median, 2.12× on the mean) (`bench/BASELINE-SEALED.md` §4). The static aggregate falls out of
sample while the marginal per-solution economics rise, because the static aggregate is dragged
down by short scalar-arithmetic tasks that are not in the text-feedable trial subset. The precise
scope: this edge is defined on CSPM (marginal, quickref excluded), where MTL ≥ Python; on the cold
*total* view (quickref included) Python wins until roughly 154 held-out solves amortize the
quickref (`bench/BASELINE-SEALED.md` §4).

### 6.8 Capability confinement (positive, machine-checked)

Tier-3's case for MTL is capability confinement and safety, explicitly **not** compression, and
must not be read against the retired compression gate. The tier-3 agentic suite's executable
static aggregate is **~1.86× (o200k) / 1.85× (cl100k)** over 16 tasks
(`bench/BASELINE-TIER3.md`). (The README cites "1.90× exec density"; the benchmark artifact is
1.86×, and this paper uses the artifact — §11.)

The confinement itself is machine-checked and enforced by the runtime independent of agent
behavior. The pure core suspends at every capability call and yields a fourth outcome,
`Invoke(name, stack, cont)`, instead of faulting; the unverified host runner services it behind a
grant whitelist, with metering charged before the effect and clean between-step cancellation, so
no partial effect is possible (`README.md` §Effects, `docs/design/v0.6-checker.md` §Call). The
core threads no host state and closes over nothing, which is what keeps P1/P2/P3 intact across the
addition. Seven security-posture tests pin the guarantees:
`crates/mtl-host/tests/security_posture.rs` proves that an ungranted capability is unreachable
(returns `Refused`/`NotGranted`, categorically distinct from a fault), that budget exhaustion
cancels with no partial effect, that the output-byte cap is never exceeded, and that each
invocation consumes its budget exactly once (`bench/BASELINE-TIER3.md` §Security posture). In the
cold tier-3 agent trial, **0 ungranted-capability-call attempts occurred across all confined cells
in both arms**, and the report notes this holds *regardless of agent behavior*: an ungranted call
is a loud `NotGranted` failure that could never slip through as a PASS even if an agent tried one
(`bench/agent-trial/tier3/REPORT.md` §G).

### 6.9 Performance (a non-goal, reported)

Performance is an explicit non-goal (`docs/mtl-spec.md` §1.2), but it is measured. The arena
backend is now the default execution engine (issue #47 / PR #81), an interned, persistent,
O(1)-fork continuation engine that kills the reference interpreter's O(n²) hotspots (flat
front-pop, primrec re-emission, fold tails) (`README.md` §Arena backend,
`docs/design/v0.5-refactor.md`). The reference interpreter runs at ~35M interpreter steps/sec
(`crates/mtl-perf/PERF-BASELINE.md`). The arena's measured wins on the spike include 144× on the
primary primrec pathology and O(1) fork confirmed flat at ~1.09 ns across stack depths 1–10,000
(`docs/design/v0.5-refactor.md` §3). The arena adds no new primitives and no new semantics; both
engines are kept bit-identical by the 148-case differential oracle, and the arena is the default
only because its refinement is proved (§5.5).

---

## 7. Decision record

The standing declined and deferred choices, each with its rationale and evidence:

- **Indexed access — DECLINED.** Adding `two_sum` and `binary_search` halves the tier-2
  aggregate from 3.87×/3.92× to **2.00×**, and `two_sum` lands at **0.59×** — a task where MTL is
  strictly dearer than Python (`docs/design/v0.6-indexed-access.md` §2, §4). The load-bearing
  finding is that these tasks are **juggling-bound, not access-bound**: the token cost is
  dominated by point-free shuffling of a four-deep carried state, so swapping the access mechanism
  (O(n) primitive, `Value::Vec`, or host capability) barely moves the number. If coverage is ever
  wanted, the route is host capabilities, never a core primitive or a new value type.
- **Strings stay host-side / opaque — DECLINED in core.** There is no `Value::Str`; strings are
  opaque host-side `i64` handles the core can neither inspect nor forge (`README.md` §Effects). No
  corpus task required in-core strings, and the proof cost of a recursive string value (reopening
  the deep-view termination measure and P4) decides it against inclusion
  (`docs/design/v0.6-indexed-access.md` §3.b).
- **Arena backend — DEFERRED then promoted to DEFAULT.** Admitted opt-in as a
  differentially-validated backend, then made the default engine only once its refinement was
  discharged as a Verus proof (`docs/design/v0.5-refactor.md` §6, §7).
- **Primitive admission — GATED on the tokcount rule.** A primitive is admitted only if it pays
  for itself in corpus tokens (`docs/mtl-spec.md` §1.2); this is why v0.8 admitted nothing despite
  a verified windowed-fold candidate (`docs/design/v0.8-generalization.md` §4).
- **`#f[...]` definitions — DEFERRED behind an evidence gate.** Not part of the core; deferred
  pending evidence they pay (`README.md` §Roadmap).
- **fork-as-primitive — REJECTED.** A `fork` word introduces a choice point inside the evaluation
  relation and breaks P1 determinism; speculation is a host-layer driver over cloned `VmState`s
  that never reaches into the core (`docs/design/v0.5-refactor.md` §4).
- **Metering — host-side bytes/budget meter, not step-weighted fuel.** Metering is charged before
  the effect as a host-side budget; fuel is a single cumulative global budget across resumptions
  (`README.md` §Effects, `docs/design/v0.5-refactor.md` §4.3).

---

## 8. Related work

MTL's read-tax discipline is borrowed from **pxpipe**, a context-as-images compressor that renders
token-dense context into PNGs (`docs/notes/related-work.md`). pxpipe's value to MTL is its eval
discipline, not its mechanism: its needle-in-haystack test reported a text baseline of 15/15 but
**0/15 on the image path, and the failures were silent confabulations** — plausible-but-wrong
values returned rather than uncertainty signalled. MTL structurally cannot fail that way silently:
programs are exact discrete glyph sequences, misreads produce loud typed faults, and a verified
interpreter is the ground truth (§6.5). MTL adopts pxpipe's read-tax battery
(comprehension/recall/mutation/confabulation) and its per-model-generation re-validation
discipline.

Against the broader field, MTL positions differently from Python-in-a-sandbox, WASM demos, and
jq-style DSLs on one axis: those typically ship a headline number with no pinned-toolchain
reproduction path, whereas MTL ships the reproduction kit (§10). The concatenative lineage is Forth
and Joy; the base `{dup, drop, cat, cons, apply}` follows Kerby's minimal concatenative core, and
the reference-counting direction draws on Perceus/Koka (`README.md` §Related work,
`docs/reviews/2026-07-11-adversarial-review.md` §8).

---

## 9. Limitations

Stated plainly and in full:

- **Niche compression.** Compression is 2–4× on loop/fold/recursion shapes and ≤1× on scans and
  builtin-heavy code; it is not a general property (`docs/design/v0.8-generalization.md` §4).
- **Terse-Python parity.** Against fair terse Python the ratio is ~1.03× — essentially break-even
  (`docs/design/v0.8-generalization.md` §1).
- **Structural juggling tax.** ~59% of the lost 3× excess is stack-juggling intrinsic to a
  concatenative stack, which has no sliding window and no named running state
  (`docs/design/v0.8-generalization.md` §1).
- **Spent sealed set.** The one-shot held-out eval is now consumed because reference solutions were
  committed post-freeze; a fresh re-seal protocol is specified but not yet run
  (`bench/BASELINE-SEALED.md` §9a, `docs/design/v0.8-generalization.md` §5).
- **Cold-only trials.** No warm/fine-tuned MTL arm has been measured; every trial is cold, which is
  a lower bound for MTL (`bench/agent-trial/results/REPORT.md` §Caveats).
- **Single model under test.** All agent trials use one model, `claude-opus-4-8`, run cold
  (`bench/agent-trial/tier3/REPORT.md` §Integrity).
- **Tokenizer proxies.** Token counts use tiktoken (o200k_base / cl100k_base) as a public proxy;
  the target model's tokenizer has no pinned public implementation, so absolute costs will differ
  (`bench/agent-trial/sessions/REPORT.md` §Tokenizer caveat).
- **Read-tax ceiling effect.** The read-tax accuracy metric saturates at ~100% in both arms, so it
  establishes "no measurable tax," not a finer bound (§6.5).
- **Production-code panic-freedom.** The production interpreter's "never panics" claim is not
  literally established by the stable Rust source alone (guarded `expect`/`unreachable!` arms
  remain); the Verus twin proves its impossible arms unreachable, but that proof does not
  mechanically transfer to the cargo build (`docs/reviews/2026-07-13-architecture-review-gpt55.md`).

---

## 10. Reproducibility

The reproduction path is the credibility path, precisely because the proofs were never gated by
hosted CI. GitHub's hosted runners were stalled by GitHub-side rate-limiting through the
v0.2–v0.4 work, so **CI did not gate these merges** (issue #12); all verification evidence is
local, from source-built Verus at the pinned commit (`README.md` §CI status,
`crates/mtl-core/proof-log.txt`). That is exactly why an independent, reviewer-run kit — not a
green badge — is the credibility argument.

The top-level `REPRODUCE.md` (a separate deliverable) carries the claim→command map: how to build
Verus at the pin and re-run each proof root, how to regenerate the token baselines with tiktoken,
and how to re-run the contamination gate and sealed validation. Readers should start there and
follow the claim→command map; Appendix B of this paper maps each headline number to its artifact
file. As of this writeup, all five proof gates (`mtl_core` 76, `p5_universality` 118, `p4_verus`
101, `checker_verus` 116, `arena_verus` 145 — 0 errors each) and the full non-Verus ratio pipeline
were independently reproduced from a clean container checkout (`REPRODUCE.md`, `kit/EVIDENCE.md`):
the Verus roots against a source-built prover at the pin, and the token/test pipeline byte-identically
(14/14) from a fresh `git clone`.

What is **not** push-button reproducible: the live-model agent trials. The kit reproduces the
deterministic scorer, the ground-truth I/O vectors, and the protocol — but not the raw model
outputs, which depend on a live model call. The per-attempt records are committed, so the
aggregation is reproducible even though the generation is not
(`bench/agent-trial/results/`, `bench/agent-trial/sessions/REPORT.md` §Reproducing).

---

## 11. Conclusion

MTL is demonstrably a machine-checked, agent-writable, capability-confined concatenative language.
Its determinism, interpreter refinement, progress, parser round-trip, Turing-completeness,
static-checker soundness, and arena refinement are all proved in Verus, with a trusted boundary of
exactly two `Clone` stubs (`crates/mtl-arena/proof-log.txt`). Cold agents write it at 100% pass@5
on unseen tasks, read it with no measurable tax and zero silent confabulation, and never escape a
capability grant (`bench/BASELINE-SEALED.md`, `bench/agent-trial/readtax/round2/results/REPORT.md`,
`bench/agent-trial/tier3/REPORT.md`).

The general-compression thesis — a ≥3× token reduction against idiomatic Python — honestly failed
out of sample. It held only on dev corpora that co-evolved with the primitives (3.72–3.92×), and
collapsed to 1.67× on the first blind held-out set and to ~1.03× against fair terse Python
(`bench/BASELINE-SEALED.md`, `docs/design/v0.8-generalization.md`). The gate was formally retired
(PR #95, commit `3b9e20a`). The claim was restated around what survived out of sample: agent
reliability, per-solution economics (held-out CSPM 2.124×), and verified confinement.

Answering the sharp question directly: a language co-designed for tokenizers and verification
**preserves agent reliability**, and **improves per-solution token economics**, but **does not
reduce total inference cost in the cold single-session regime**, because the fixed cost of
teaching the language in-context dominates. The biggest open lever is to move that competence out
of context and into weights: the warm/fine-tuned arm, whose arithmetic predicts N* → 1 and whose
data pipeline is already in-repo, is the specified next experiment
(`docs/design/v0.7-lora-warm-agent.md`, issue #83).

---

## Appendix A — Proof scoreboard

All roots machine-checked in Verus, pin `0.2026.07.05.49b8806`, Z3 4.12.5, admit-free except the
two documented `Clone` stubs (`crates/mtl-arena/proof-log.txt`).

| Property | Root artifact | Verified / errors | What it establishes |
|---|---|---|---|
| P1 determinism, P2 refinement, P3 progress | `crates/mtl-core/src/mtl_core.rs` | 76 / 0 | `spec_step` total; exec twin faults exactly when spec does, else same next state; every state Next/Halt/Fault |
| P5 universality | `crates/mtl-core/src/p5_universality.rs` | 118 / 0 | Two-counter Minsky simulation, unary-quotation counters, 6 theorems, two-way fuel-quantified halting |
| P4 round-trip | `crates/mtl-syntax/proofs/p4_verus.rs` | 101 / 0 | `parse(print(p)) = p` and `print(parse(s)) = canonicalize(s)`, upgraded to full (c) via mirrored twin |
| Layer-C checker soundness | `crates/mtl-core/src/checker_verus.rs` | 116 / 0 | T-Static, T-Progress, T-Branch over the literal-quotation fragment |
| Arena refinement | `crates/mtl-arena/proofs/arena_verus.rs` | 145 / 0 | α(arena_step) = spec_step(α), all 23 primitives + fault parity + u32-capacity + multi-step driver |
| — | `cargo test --workspace` | 322 passed / 0 failed (proof-log records earlier 275) | Workspace test suite |
| — | `crates/mtl-core/tests/p5_minsky.rs` | 6 passed | P5 executable validation |

**Trusted boundary:** exactly two `Clone` `external_body` stubs (`Word::clone`, `Value::clone`);
`verus --no-cheating` flags these two and nothing else.

---

## Appendix B — Claim → artifact index

| Headline claim | Value | Artifact |
|---|---|---|
| Held-out static compression | 1.67× o200k / 1.72× cl100k (14 tasks) | `bench/BASELINE-SEALED.md` |
| In-sample micro compression | 2.11× → 3.72× | `bench/BASELINE.md` |
| In-sample tier-2 compression | 3.87× / 3.92× | `bench/BASELINE-TIER2.md` |
| Broad distribution (1,145 shapes) | 1.70× train / 1.72× dev; 3.25× full is artifact | `docs/design/v0.8-generalization.md` |
| Terse-Python parity | ~1.03× | `docs/design/v0.8-generalization.md` |
| 9-tokenizer robustness | 3.83×–4.75× core | `bench/design-lora/RESULTS.md` |
| Sink taxonomy | 59% / 23% / 10% / 8% | `docs/design/v0.8-generalization.md` |
| Writability, dev | 100% pass@5 both arms | `bench/agent-trial/results/REPORT.md` |
| Writability, held-out | 100% pass@5 both arms | `bench/BASELINE-SEALED.md` |
| CSPM ratio, dev / held-out | 1.274 / 2.124 | `bench/agent-trial/results/REPORT.md`, `bench/BASELINE-SEALED.md` |
| Marginal tokens/solution, held-out | 19 (MTL) vs 57 (Python) median | `bench/BASELINE-SEALED.md` |
| Read-tax comprehension delta | +0.0 pts through tier D | `bench/agent-trial/readtax/round2/results/REPORT.md` |
| Confabulation rate | 0% both arms (vs pxpipe 0/15 silent) | `bench/agent-trial/readtax/round2/results/REPORT.md`, `docs/notes/related-work.md` |
| Session break-even N* | none within N ≤ 16, structural | `bench/agent-trial/sessions/REPORT.md` |
| Full quickref cold cost | 4,051 tokens (o200k) | `bench/agent-trial/sessions/REPORT.md` |
| Minimal preamble | 487 tokens (12%), no solve-rate loss | `docs/mtl-quickref-min.md`, `bench/agent-trial/preamble/REPORT.md` |
| Tier-3 exec compression | ~1.86× (README cites 1.90×) | `bench/BASELINE-TIER3.md` |
| Confinement tests | 7 tests, 0 ungranted attempts | `crates/mtl-host/tests/security_posture.rs`, `bench/agent-trial/tier3/REPORT.md` |
| Runtime | ~35M steps/sec, O(1) fork | `crates/mtl-perf/PERF-BASELINE.md`, `docs/design/v0.5-refactor.md` |
| Proof stack | 76 + 118 + 101 + 116 + 145 verified, 0 errors | `crates/mtl-arena/proof-log.txt` |

The claim→command map (how to re-run each of these) lives in the top-level `REPRODUCE.md`.
