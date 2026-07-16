# MTL: Minimal Token Language — Specification v0.4-draft

**Status:** Draft for review — revised in response to the 2026-07-11 adversarial review (`docs/reviews/2026-07-11-adversarial-review.md`). v0.2-draft added the four recursion primitives (`primrec`, `times`, `linrec`, `uncons`) admitted by `docs/design/v0.2-recursion-primitives.md` (merged PR #8). v0.3-draft adds the two sequence primitives (`fold`, `xor`) admitted by `docs/design/v0.3-sequences.md` (merged PR #13) and implemented in the core (`spec_step_prim` + `crate::interp`). v0.4-draft adds the effects boundary admitted by `docs/design/v0.4-effects.md`: `Invoke`, a fourth `SpecStep` outcome, so every `Call` suspends the pure core for an unverified host runner (§8), implemented in the core and the `mtl-core::host` seam; and discharges **P5**: Turing completeness is now a machine-checked **theorem** (Layer B), not a conjecture — see §6, proof artifact `crates/mtl-core/src/p5_universality.rs` (118 verified, 0 errors; re-verifies the frozen core alongside, still 76/0).
**North star:** Turing complete (proved — theorem, see §6). Minimize expected total LLM inference tokens to a correct solution over a benchmark task distribution.
**Verification target:** Reference semantics and interpreter verified in Verus (SPEC → PROOF → RED → GREEN → REFACTOR). The normative artifact is `crates/mtl-core/src/mtl_core.rs`: where this prose and `spec_step` disagree, `spec_step` is authoritative and the prose is the defect.

---

## 1. Objective and Non-Goals

### 1.1 The metric

Let `T` be a task distribution (§10), `tok(p)` the token count of program text `p` under a fixed, pinned tokenizer set (o200k_base, cl100k_base, and a pinned Claude tokenizer implementation — §11), and `sol(t, L)` the shortest known correct solution to task `t` in language `L`.

> **Program-length objective:** minimize `E[t ~ T] [ tok(sol(t, MTL)) ]`, subject to MTL being Turing complete.

Program length is necessary but not sufficient. The **operational headline metric** is *correct solutions per million inference tokens* on a sealed evaluation set (§10.6), which folds in language-acquisition cost, validator errors, and repair attempts — a token-cheap but unwritable language loses. Raw `tok(sol)` is one input to that, not the target.

**Success gate (Abrash rule):** MTL ships only if it beats a baseline panel — in particular idiomatic Python — on correct-solutions-per-million-inference-tokens at equal or better agent success rate, on the sealed set (§10.6). Below that, it's a curiosity and we say so.

**Gate verdict (v0.8, concluded):** the ≥3× *compression* sub-gate FAILED out-of-sample (1.67× held-out vs 3.7–3.9× in-sample; the dev corpora co-evolved with the primitives). MTL ships on the per-solution-economics form of the gate — CSPM 2.124× held-out at 100% pass@5 — plus verified confinement. Compression is retained as a niche property (2–4× on loop/fold shapes, ≤1× on scans/builtins), not a headline. Post-mortem: §10.6, `bench/BASELINE-SEALED.md`, `docs/design/v0.8-generalization.md`.

### 1.2 Non-goals

- Minimal *number of primitives*. Turing tarpits are explicitly rejected; the primitive set grows whenever a new primitive pays for itself in corpus-level token savings.
- Human ergonomics. Writers are agents; a validator and pretty-printer serve humans.
- Performance. The reference interpreter optimizes for provability, not speed.

---

## 2. Lexical Structure

The lexer is deliberately trivial — every lexing rule exists to enable BPE merges — but it is now specified as a deterministic algorithm with test vectors (§2.3).

### 2.1 Token classes

| Class | Rule |
|---|---|
| **Symbol word** | A single ASCII punctuation character from the primitive table (§5). Self-delimiting: `:!` lexes as `:` `!`. |
| **Integer literal** | `[0-9]+`, value in `0 ..= i64::MAX`. **Unsigned** — a leading `-` is never part of a literal; it is always the `Sub` primitive (§2.3). Delimited by any non-digit. |
| **String literal** | `"..."` with `\"` and `\\` escapes only. **Reserved, not part of the v0.1 core** — the v0.1 parser rejects string literals (`StringUnsupported`); see §3. |
| **Named word** | `[a-z][a-z0-9]*` — reserved for host-injected capabilities (§8). Delimited by non-alphanumerics. |
| **Whitespace** | Optional between all tokens except adjacent integers / adjacent named words. Never required between symbol words. |

### 2.2 The merge principle

Because symbol words self-delimit, programs are written **without whitespace between symbols**: `:!` not `: !`. BPE tokenizers *often* merge adjacent punctuation into single tokens (`:!`, `];`, `[[` are 1 token under some tokenizers/revisions). Whether a given pair merges — and at what cost — differs across tokenizers and revisions and must be **measured against pinned tokenizer snapshots** (§11), not assumed. Where merges occur, MTL's effective cost per primitive can fall *below* 1 token — a property whitespace-separated languages structurally cannot achieve for their delimiters, which must spend a separator token. The stronger v0.1 phrasings ("frequently merge", "effective cost often below one token", "no whitespace-separated language can have this property") are downgraded to hypotheses to be demonstrated on a published corpus + tokenizer snapshot, not established facts.

**Consequence:** primitive glyph assignment is an empirical optimization problem (§11), not an aesthetic choice. Glyphs are assigned to maximize merge frequency of common *bigrams and trigrams* in the benchmark corpus.

### 2.3 Tokenization algorithm (deterministic)

The v0.1 lexer is a deterministic maximal-munch scanner. Given the rejection of signed literals (below), there is no `1-2` ambiguity.

**Integer-literal decision (Option A, normative).** Integer literals are **unsigned**: `IntegerLiteral ::= [0-9]+`, value in `0 ..= i64::MAX`. A `-` is **always** the `Sub` primitive (§5), never part of a literal. Negative constants are produced operationally: `-7` is written `0 7 -` (push 0, push 7, subtract). This supersedes the v0.1 `-?[0-9]+` grammar.

Rationale: this eliminates the `1-2` ambiguity (is it `Int(1) Int(-2)` or `Int(1) Sub Int(2)`?) and the closely related LLM footgun where `1 -2` and `1 - 2` would otherwise tokenize differently. Writability — expected `tokens × attempts` to a correct program — dominates the rare saving on negative literals: a model that must reason about literal-vs-operator sign boundaries fails more often, and each failure costs far more than the one token a signed literal would occasionally save.

**Algorithm.** Scan left to right; at each position, skip optional whitespace, then match the longest token by class:

1. Next char is an ASCII digit `[0-9]`: consume the maximal run of digits → `Int(value)`. No preceding `-` is ever folded into the literal.
2. Else next char is `[a-z]`: consume the maximal run of `[a-z0-9]` → `Name(word)`.
3. Else next char is `"`: consume a string literal with `\"`/`\\` escapes → **reserved**; the v0.1 parser rejects it (`StringUnsupported`, §3).
4. Else next char is a primitive symbol from §5 (`[ ] : _ ~ @ ^ ! , ; ' + - * / % = < ? `): consume exactly that one char → the corresponding symbol word. Symbol words are always single-character and self-delimiting, so maximal munch never merges two symbols into one lexical token (BPE merging happens later, in the tokenizer, and is orthogonal to lexing).
5. Else: lexical error (unknown character).

Whitespace is required only to separate two adjacent integer literals or two adjacent named words (rules 1–2 are the only greedy classes); it is never required around symbol words and never changes the tokenization of a symbol run.

**Test vectors.**

| Source | Tokens |
|---|---|
| `1-2` | `Int(1) Sub Int(2)` |
| `1 - 2` | `Int(1) Sub Int(2)` |
| `12 34` | `Int(12) Int(34)` |
| `1234` | `Int(1234)` |
| `0 7 -` | `Int(0) Int(7) Sub` (the canonical `-7`) |
| `:!` | `Dup Apply` |
| `:[` | `Dup LQuote` |
| `[1 2+]` | `LQuote Int(1) Int(2) Add RQuote` |
| `~@^` | `Swap Rot Over` |
| `3:*` | `Int(3) Dup Mul` |

---

## 3. Values and Machine State

```
Value   ::= Int(i64) | Quote(Program)          -- v0.1 core (matches mtl_core.rs)
Program ::= sequence of Word
Word    ::= Push(Value) | Prim(PrimOp) | Call(name)
State   ::= (stack: List<Value>, cont: Program)
```

- `Str(string)` is **reserved, not a v0.1 core value.** `mtl_core.rs` defines `SpecValue` and `Value` with only `Int` and `Quote`; the parser rejects string literals (`StringUnsupported`). This resolves the review's observation that strings were "semantically present but unusable": v0.1 excludes them from the core, and they return as a v0.2 value once string primitives and/or host capabilities justify them (§10.2, §14).
- A **program** is a finite sequence of words.
- The **machine state** is a pair: an operand stack and a continuation (the remaining program).
- **Quotations** `[ ... ]` are first-class values: unevaluated programs pushed onto the stack. They are MTL's *only* abstraction mechanism — functions, closures, control flow, and data constructors are all quotations.
- Booleans are integers: `0` is false, non-zero is true. (A dedicated bool type is a token tax with no benefit.)

There are no variables and no environments. This is a design consequence of the metric: names are pure token spend, and point-free composition eliminates them. It is also a verification gift — the state is a pair of lists, no binding structure, no substitution lemmas.

---

## 4. Small-Step Operational Semantics

The semantics is a **total step function** — this is the load-bearing decision for Verus verification. Every state maps to exactly one of four outcomes:

```
Step  ::= Next(State) | Halt(stack) | Fault(Error)
        | Invoke(name, stack, cont)   -- host suspension (v0.4); every Call yields this

Error ::= Underflow | TypeMismatch | Overflow | DivByZero
        | UnknownWord | FuelExhausted   -- FuelExhausted: driver only, not step
```

`Invoke(name, stack, cont)` — the fourth, host-suspension outcome — is the **v0.4 effects boundary** (§8): as of v0.4 every `Call(name)` yields `Invoke`, suspending the pure core for the host runner (this is exactly what `spec_step` does). `UnknownWord` remains in the `Error` enum but is **no longer reachable from the `Call` arm** — grant/deny is a host-side decision (§8.3).

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

-- Bounded recursion & iteration (v0.2). Total and terminating: the count
-- strictly decreases toward 0. Checked i64 arithmetic; because k>0 ⇒
-- 0 ≤ k-1 < k, the decrement is always in range — no Overflow rule is needed.
(s · Int(k) · Quote(i) · Quote(c), PrimRec p)        -- ( n [I] [C] -- r )
    → Next(s, i ++ p)                                             if k ≤ 0
    → Next(s, [k, k-1, [i], [c], PrimRec] ++ c ++ p)             if k > 0
(s · Int(k) · Quote(q), Times p)                     -- ( n [Q] -- … )
    → Next(s, p)                                                 if k ≤ 0
    → Next(s, q ++ [k-1, [q], Times] ++ p)                       if k > 0

-- Linear recursion (v0.2). DESUGARS into If — no new control operator; it
-- inherits If's branch semantics. Partial, like Apply: bounded by fuel.
(s · Quote(P) · Quote(T) · Quote(R1) · Quote(R2), LinRec p)  -- ( [P][T][R1][R2] -- … )
    → Next(s, P ++ [[T], [E], If] ++ p)
      where E = R1 ++ [[P], [T], [R1], [R2], LinRec] ++ R2      -- else-branch quote

-- Quotation deconstruction (v0.2). Structural, affine: the input quote is
-- consumed once and split. A head word that is not a value (a bare Prim/Call,
-- not Push) faults TypeMismatch.
(s · Quote([]),        Uncons p) → Next(s · Int(0), p)               -- empty
(s · Quote(Push(v)::t), Uncons p) → Next(s · v · Quote(t) · Int(1), p) -- non-empty

-- Sequence fold (v0.3). Native LEFT fold; recurses by re-emitting Fold (does
-- NOT desugar into LinRec). Total and terminating on a finite list: the spine
-- strictly shrinks each step (tail is one shorter than the sequence), the same
-- well-founded measure primrec/times use — distinct from LinRec, which is
-- partial. `init` is ANY value; a non-value sequence head (bare Prim/Call, not
-- Push) faults TypeMismatch, exactly as Uncons. No arithmetic ⇒ no Overflow arm
-- (Overflow/DivByZero arise only inside C, under C's own rules).
(s · Quote([]) · init · Quote(c), Fold p)        -- ( [seq] init [C] -- r )
    → Next(s · init, p)                                          -- empty ⇒ seed
(s · Quote(Push(h)::t) · init · Quote(c), Fold p)
    → Next(s, [[t], Push(init), Push(h)] ++ c ++ [[c], Fold] ++ p)  -- run C(init,h), recurse

-- Bitwise XOR (v0.3). Total, arity → type only (like Eq/Lt): the XOR of two
-- 64-bit two's-complement patterns is always a valid i64, so — unlike Add/Mul —
-- there is NO Overflow rule and NO DivByZero rule. `a^b` is Rust's `i64 ^ i64`.
(s · Int(a) · Int(b), Xor p)    → Next(s · Int(a^b), p)
```

Any pattern not matched above with the required arity/types faults with `Underflow` or `TypeMismatch` per the precedence in §4.4. **No rule is partial; no rule panics.**

### 4.2 Key semantic property

`Apply` splices the quotation into the continuation rather than recursing into a sub-interpreter. This makes the step relation *flat* — a single small-step transition system with no nested evaluation — which is what makes the Verus refinement proof tractable (§7).

**Bounded-space tail execution (conditional).** Flat continuation splicing permits *bounded-space* tail execution for quotations in a **loop normal form** in which the recursive self-application occurs in *tail position* — no work is scheduled in the continuation after the recursive call. It does **not** give "proper tail calls for free" unconditionally: `Apply` sets `cont := q ++ rest`, so a body that schedules work after its recursive `!` grows the continuation by `len(q)` each iteration. The bound is proof obligation **P6**, which must first define tail position precisely, then state *which* space it bounds. Four distinct quantities must not be conflated:

- **semantic continuation size** — `len(cont)` in the spec machine (what P6 targets);
- **temporary allocation** during `Vec` concatenation in the exec machine;
- **physical call-stack usage** of the interpreter;
- **heap retention** from shared quotation values.

P6 bounds the first, for the loop normal form only.

### 4.3 Determinism

For every state `σ`, exactly one rule applies. Determinism is proof obligation **P1** (§7.3).

### 4.4 Fault classification and precedence (normative)

When a primitive cannot fire, `spec_step` faults, and the *order* in which `spec_step_prim` checks determines *which* error is reported. The v0.1 draft left "`Underflow` or `TypeMismatch`" undetermined; the normative precedence, read directly off `spec_step_prim` in `mtl_core.rs`, is:

1. **Arity** — if the stack holds fewer values than the primitive's input count, `Fault(Underflow)`. Checked first, before any operand is inspected.
2. **Operand types** — with arity satisfied, if any consumed operand has the wrong type, `Fault(TypeMismatch)` (first mismatch under the arm's match).
3. **Semantic checks** — with arity and types satisfied, value-level checks fire: `Fault(DivByZero)` for `/ %` with divisor `0`; `Fault(Overflow)` for arithmetic whose true result leaves `i64` (including `i64::MIN / -1` and `i64::MIN % -1`).

This ordering is normative because `spec_step` **is** the specification: P2 refines the exec interpreter against exactly this function, and P1/P3 are stated over it. The document does not get to override it; if the prose above ever disagrees with `spec_step_prim`, the code wins.

**Worked examples.** Core value types are `Int` and `Quote` only (§3); `Str` is not a v0.1 core value, so the review's `[Str] …` cases are rejected earlier by the parser (`StringUnsupported`) and never reach `spec_step`. Using `Quote` as the concrete non-`Int` value:

| Stack (top at right) | Word | Outcome | Why |
|---|---|---|---|
| `[Int(1)]` | `Add` | `Fault(Underflow)` | arity: `Add` needs 2, stack has 1 — checked before types |
| `[Quote(q), Int(1)]` | `Add` | `Fault(TypeMismatch)` | arity ok (2); operand `Quote` is not `Int` |
| `[Int(1), Quote(q)]` | `Add` | `Fault(TypeMismatch)` | arity ok; top operand `Quote` is not `Int` |
| `[Int(5), Int(0)]` | `Div` | `Fault(DivByZero)` | arity ok, both `Int`; divisor 0 |
| `[Int(i64::MIN), Int(-1)]` | `Div` | `Fault(Overflow)` | arity ok, both `Int`, divisor ≠ 0; result leaves `i64` |
| `[Int(3)]` | `Apply` | `Fault(TypeMismatch)` | arity ok (1); `Apply` requires `Quote`, got `Int` |
| `[Quote(q), Quote(q)]` | `PrimRec` | `Fault(Underflow)` | arity: `primrec` needs 3, stack has 2 — checked before types |
| `[Quote(q), Quote(q), Quote(q)]` | `PrimRec` | `Fault(TypeMismatch)` | arity ok (3); count slot is `Quote`, not `Int` |
| `[Int(3), Int(1)]` | `Times` | `Fault(TypeMismatch)` | arity ok (2); `times` requires `Quote` on top, got `Int` |
| `[Int(9)]` | `Uncons` | `Fault(TypeMismatch)` | arity ok (1); `uncons` requires `Quote`, got `Int` |
| `[Quote([Add])]` | `Uncons` | `Fault(TypeMismatch)` | arity ok, is `Quote`; head word `Add` is not a value (`Push…`) |
| `[Int(1), Int(2)]` | `Fold` | `Fault(Underflow)` | arity: `fold` needs 3, stack has 2 — checked before types |
| `[Int(9), Int(0), Quote(c)]` | `Fold` | `Fault(TypeMismatch)` | arity ok (3); sequence slot is `Int`, not `Quote` |
| `[Quote([Add]), Int(0), Quote(c)]` | `Fold` | `Fault(TypeMismatch)` | arity ok, both quotes; sequence head `Add` is not a value (`Push…`) |
| `[Int(5)]` | `Xor` | `Fault(Underflow)` | arity: `xor` needs 2, stack has 1 — checked before types |
| `[Quote(q), Int(1)]` | `Xor` | `Fault(TypeMismatch)` | arity ok (2); operand `Quote` is not `Int`. `xor` is total — no semantic-fault case |

The review's illustrative pair maps as: `[Str] Add` → single non-matching operand, arity 2 unsatisfied → `Underflow` (arity checked first); `[Str, Int] Add` → arity satisfied, operand wrong → `TypeMismatch`. In the v0.1 core these are moot because `Str` never reaches the stack.

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
| `+` `-` `*` `/` `%` | arith | `( a b -- c )` | checked; `-` is always Sub (§2.3) |
| `=` | eq | `( a b -- 0\|1 )` | |
| `<` | lt | `( a b -- 0\|1 )` | |
| `?` | if | `( c [t] [f] -- ... )` | |

**v0.2 recursion primitives** (admitted by `docs/design/v0.2-recursion-primitives.md`, merged PR #8; implemented in `spec_step_prim` and `crate::interp`). Glyphs are the measured assignment from the design doc (§5 there); the lexer wiring for these glyphs lands with the v0.2 parser.

| Glyph | Name | Stack effect | Notes |
|---|---|---|---|
| `&` | primrec | `( n [I] [C] -- r )` | bounded primitive recursion; **total, terminating** (count ↓ to 0). `n≤0` runs `I`; `n>0` folds `C` over the `(n-1)` subresult, keeping `n` available to `C`. Checked i64; no Overflow arm (`k-1` provably in range) |
| `.` | times | `( n [Q] -- ... )` | run `Q` exactly `max(n,0)` times, left to right; **total, terminating**. `.` is unambiguous next to digits — integer literals have no decimal point (§2.3), so `3.` is `Int(3) Times` |
| `\|` | linrec | `( [P] [T] [R1] [R2] -- ... )` | general linear/tail recursion; **desugars into `?` (if)** — inherits its verified branch semantics, adds no control operator. **Partial**, like `!`; bounded by fuel. Tail recursion is `R2 = []` |
| `>` | uncons | `( [w …] -- w [ … ] 1 )` \| `( [] -- 0 )` | deconstruct a quotation: non-empty → head value, tail quote, flag `1`; empty → flag `0`. Structural and **affine** (consumed once, split). A non-value head (bare `Prim`/`Call`) faults `TypeMismatch`. The direct enabler of the honest TC proof (§6.2) |

`primrec`/`times`/`linrec` are admitted on token grounds (frozen `T_v0` reaches 3.72× *in-sample* with `primrec`+`linrec` — a dev-corpus figure that does not hold out-of-sample; see the §1.1/§10.6 gate verdict); `uncons` is admitted on the TC-proof / list-enablement rationale. Multiplicity (spec §14.4): `primrec`/`times`/`linrec` **replicate** their quote arguments along the recursion spine (like `:!`), while `uncons` is genuinely **affine** — it splits its one quote without duplication.

**v0.3 sequence primitives** (admitted by `docs/design/v0.3-sequences.md`, merged PR #13; implemented in `spec_step_prim` and `crate::interp`). Glyphs are the measured assignment from the design doc (§5 there); the lexer wiring for these glyphs lands with the v0.3 parser.

| Glyph | Name | Stack effect | Notes |
|---|---|---|---|
| `(` | fold | `( [seq] init [C] -- r )`, `C:( acc w -- acc' )` | native **LEFT** fold; `init` seeds the accumulator, `C` runs once per element left-to-right. On `[]` the result is `init`. Recurses by **re-emitting itself** (does **not** desugar into `linrec`); **total, terminating** — the sequence spine strictly shrinks each step (the same well-founded measure `primrec`/`times` use). **Affine** in `seq` (consumed once, split head-first like `uncons`), **multiplicative** in `[C]` (replicated along the spine like `primrec`). A non-value sequence head faults `TypeMismatch`. No Overflow arm (any Overflow arises inside `C`) |
| `$` | xor | `( a b -- a^b )` | bitwise XOR on the i64 two's-complement representation (Rust's `i64 ^ i64`). **Total**, arity → type only: unlike `+`/`*`, the XOR of two in-range i64 values is always in i64 range, so there is **no Overflow arm and no DivByZero arm**. The two "obvious" bitwise glyphs `^`/`&`/`\|` are all taken (over/primrec/linrec), and `[$` merges to one token in both pinned tokenizers |

`fold` is admitted on **token grounds** — it collapses the tier-2 list-traversal solutions 145 → 56/54 tokens (design §6), internalising the left-fold "stack-juggling tax" that is the dominant LLM failure mode, with no task regressing. `xor` is admitted because it **clears the `single_number` wall and the whole bit-manipulation class** at trivial proof cost (total; one lock-step P2 pair). Together they take the tier-2 aggregate from 1.91× to **3.87× / 3.92×** *in-sample*, on par with the frozen `T_v0` headline (these are dev-corpus figures; out-of-sample the same measurement is ~1.7× — see the §1.1/§10.6 gate verdict). Multiplicity (spec §14.4): `fold` is the first primitive that is **affine in one argument (`seq`) and multiplicative in another (`[C]`) at once**; `xor` is a plain binary value op (two linear `Int`s in, one out), like `+`/`=`.

**Deliberate inclusions beyond the minimal base.** `swap`, `rot`, `over`, `dip`, native ints, `if` are all derivable from the Kerby base `{dup, drop, cat, cons, apply}` — and we include them anyway, because derived forms cost 5–20 tokens *per use site* while a primitive costs ~1. This is the anti-tarpit principle applied consistently: **the primitive set is open, and admission is decided by corpus-level token accounting**, not by minimality aesthetics.

**v0.2 candidates** (admit if benchmarks justify, per §10.2's admission corpus): `uncons` (quotation deconstructor — the direct enabler of the honest Turing-completeness proof, §6.2); `times` (bounded loop); `map`/`fold` over a list value type; `pick`/`roll` generalized stack access; a `linrec`-style recursion combinator; string primitives (§10.2). The Gemini review recommends prioritizing `pick`/`roll` over `map`/`fold`, on the grounds that LLMs handle explicit indexed stack access better than blind spatial routing (§10 notes the stack-juggling tax) — measurement decides.

---

## 6. Turing Completeness

**Status: theorem (Layer-B, spec-level), machine-checked.** The v0.1 draft asserted a "Theorem (TC)" via a two-counter Minsky simulation using two `i64` integers as counters. That argument was **invalid** and was withdrawn to a conjecture (v0.1.1). As of v0.4 the conjecture is **discharged**: an admit-free Verus proof (`crates/mtl-core/src/p5_universality.rs`, 118 verified, 0 errors) establishes universality using the quotation-encoded unbounded storage route of §6.2. §6.1 records why the bounded-counter argument fails and thus what the proof had to do instead.

> **Theorem (TC).** MTL's `spec_step` semantics faithfully simulate an arbitrary **two-counter Minsky machine** with unbounded `nat` counters, via the **unary-quotation counter encoding** of §6.2 (counter value `n` ↔ a quotation of `n` marker words; increment = `Cons`; decrement-and-zero-test = `Uncons`), compiled into a single self-applying `: !` dispatch-loop with a bounded-`Int` program counter (§6.4). Bounded-stutter (multi-step) simulation is proved (`p5_stutter_step`, `p5_simulation`) — one Minsky transition maps to a bounded run of `spec_step`s (`K ≈ 6·pc + loop-entry + handler`, linear in the PC), a stuttering simulation rather than a literal one-to-one lock-step; and halting correspondence is proved in **both directions with explicit fuel quantification**: the Minsky machine halts with output ⟺ ∃ a fuel bound at which the MTL run halts with the encoded output (`p5_halt_forward`, `p5_halt_forward_monotone`); the Minsky machine diverges ⟺ ∀ fuel the run has not halted (`p5_diverge`, `p5_halt_reverse`). Hence MTL is Turing complete.
>
> **Scope (honest).** The theorem is about the **spec-level `spec_step` semantics**, where counters live in unbounded `Seq` length — the `i64` bound limits only integer *values* and the program counter (a finite program), never the simulation itself. The executable `run` is fuel- and memory-bounded like any physical machine; termination of `run` is intentionally **not** claimed (a Turing-complete language must permit non-termination — which is exactly why the halting directions are fuel-quantified). **No new primitives** were added: the proof uses `? : ! uncons` plus `Cons` from the frozen core, which `p5_universality.rs` re-verifies alongside (still 76 verified, 0 errors, byte-for-byte unchanged).

### 6.1 Why the bounded-counter argument fails

The natural encoding — each Minsky counter as one `Int` at the bottom of the stack — does **not** yield universality. A two-counter Minsky machine is Turing complete precisely because its counters are *unbounded* nonnegative integers. With two `i64` counters, a finite instruction set, and no other unbounded storage participating in the simulation, the reachable simulated state space is **finite**, and a finite-state machine is not Turing complete. The implication

> "MTL has two `i64` counters + branching + iteration, therefore MTL simulates an arbitrary two-counter machine"

is invalid as written, so the draft's "Theorem (TC)" was withdrawn (v0.1.1) and rebuilt on the unbounded representation below.

This never showed MTL is *not* Turing complete. MTL quotations grow without bound through `Cons` (`;`), `Cat` (`,`), and continuation splicing under `Apply` (`!`). That is a genuine source of unbounded storage — the v0.1 proof simply failed to use it, and the discharged P5 proof (§6.5) uses exactly it.

### 6.2 The proof's unbounded storage: quotation-encoded counters

The discharged Minsky proof represents each counter `n` as a **unary quotation** holding `n` marker words:

- **Increment** — `Cons` a marker onto the quotation (`;`).
- **Zero test** — distinguish the empty quotation from a non-empty one.
- **Decrement** — inspect the quotation and remove one marker.

The obstruction in the v0.1 primitive set was: `Cons` (`;`) *constructs* quotations, but **nothing deconstructs them**. Both zero-testing and decrement need to observe a quotation's head/emptiness, which requires a deconstructor:

```
uncons : [v q]  ->  v [q] Int(1)     -- non-empty: head v, tail [q], flag 1
       : []      ->  Int(0)           -- empty: flag 0 only
```

`uncons` (`>`) was admitted in v0.2 (`docs/design/v0.2-recursion-primitives.md`, merged PR #8) and is part of the frozen core. It supplies both the zero test and the decrement in one step: an empty counter quotation yields flag `0`, a non-empty one yields the removed marker, the tail, and flag `1`. With `uncons` plus `Cons`, the unary-quotation Minsky encoding gives a direct, step-checkable universality proof — this is what P5 (§6.5) discharges, using no primitives beyond `? : ! uncons` and `Cons` already in `mtl_core.rs`.

### 6.3 Self-application and the `: !` kernel

`: !` (`dup`, `apply`) is a **two-glyph self-application kernel**, not — by itself — a Y combinator. The v0.1 draft's "MTL's Y combinator is two tokens" overclaims: `: !` self-applies whatever quotation sits on top of the stack, but a fixed point results only when that quotation has the right shape. Three notions must be kept distinct:

1. **Self-application kernel** — the fixed two glyphs `: !`. Given stack `[Quote(q)]`, `: !` steps (in two spec steps) to stack `[Quote(q)]` with `q`'s body spliced into the continuation — i.e. it runs `q` while handing `q` a fresh copy of itself. Verified: the `smoke_dup_apply` theorem in `mtl_core.rs`.
2. **Recursive quotation normal form** — the shape a body `q` must have for `: !` to loop rather than crash. A quotation is in *recursive normal form* when, along every control path, it (a) consumes the self-copy `: !` leaves for it, (b) preserves the stack-effect signature it was entered with, and (c) re-establishes `[Quote(q)]` on top before re-invoking `: !` (typically by threading the retained self-copy back to the top). A body lacking this shape produces stack debris or `Underflow`/`TypeMismatch` when driven by `: !`.
3. **Fixed-point construction** — a (future) theorem transforming an arbitrary suitable body into a recursive program in normal form. This is a P6-adjacent obligation, not yet discharged.

Readers who test an *arbitrary* quotation with `: !` and observe underflow are seeing (2) violated, not a defect in (1).

### 6.4 Instruction encoding (as compiled by the proof)

Under the quotation-encoded storage of §6.2, each Minsky instruction becomes a fixed literal quotation, and the whole program compiles into a single self-applying `: !` dispatch-loop indexed by a bounded-`Int` program counter:

- `INC(ci, j)`: `Cons` a marker onto counter `ci`'s quotation, then run `Qj`.
- `DEC_JZ(ci, j, k)`: `uncons` counter `ci`; on flag `0` (empty) run `Qj`; on flag `1` discard the removed marker and run `Qk`.
- `HALT`: `[]` (empty quotation → continuation empties → machine `Halt`s).

The **program counter** is which quotation currently occupies the continuation; the instruction table is finite, so each `Qi` is a fixed literal quotation. Unbounded *iteration* comes from `: !` (§6.3); unbounded *storage* from the unary quotations (§6.2).

**Out-of-range-PC model (chosen and stated).** The construction is a **finite-code Minsky machine with implicit halt outside the code domain**: a program counter `pc ≥ len(code)` — including a `DEC_JZ`/`INC` jump target past the last instruction — is a legal way to **halt**, not a fault. The dispatch cascade's fall-through arm is `[Drop, Drop]` (drain `U` and the PC → `Halt`), exactly mirroring the ghost `minsky_step`, which returns `None` for any out-of-range `pc`. The Verus proof (`disp`, `lemma_reach_halt`), the executable compiler, and the reference interpreter (`tests/p5_minsky.rs`) all adopt this one model by name. (A fault-producing fall-through was considered and rejected: it would break `lemma_reach_halt`, which maps *every* `minsky_step is None` — including out-of-range PC — to `Halt(halt_stack)`.) The `i64` bound on the PC constrains only the finite instruction-**address** space (made explicit executable-side by `validate_prog`), never counter magnitude; at the **spec** level the PC and jump targets are ghost `nat → int` casts (`PushInt(j as int)`) into MTL's *unbounded mathematical* integer, which do not truncate, so the spec-level theorem carries **no** `len(prog) ≤ i64::MAX` precondition.

### 6.5 The bounded-stutter (multi-step) lemma (P5) — discharged

P5 is **proved** in `crates/mtl-core/src/p5_universality.rs` (118 verified, 0 errors; the file re-verifies the frozen `mtl_core.rs` alongside, still 76/0). The proof carries a simulation invariant `R(m, σ)` between a Minsky configuration `m = (pc, c1, c2)` and an MTL `SpecState` `σ`, with counters encoded as **unary quotations** (not bottom-of-stack `Int`s), and consists of six theorems:

- **`p5_stutter_step`** (formerly `p5_lockstep`) — one Minsky step `m →_Minsky m'` is matched, `R`-preservingly, by a **bounded** number of `spec_step`s (`K ≈ 6·pc + loop-entry + handler`, linear in the PC; a *stutter*, not a literal 1:1 step). Finite case analysis over the three instruction forms `INC` / `DEC_JZ` / `HALT`.
- **`p5_simulation`** — bounded-stutter (multi-step) simulation for `t` Minsky steps, by induction on `t` composing `p5_stutter_step`.
- **`p5_halt_forward`** and **`p5_halt_forward_monotone`** — if the Minsky machine halts with an output, then there is a fuel bound at which the MTL run halts with the *encoded* output, and it stays halted for all larger fuel (monotonicity).
- **`p5_diverge`** — if the Minsky machine diverges, then for every fuel bound the MTL run has not yet halted.
- **`p5_halt_reverse`** — the converse halting direction: if the MTL run halts for some fuel, the Minsky machine halts (the contrapositive of `p5_diverge`).

Both machines are pure spec functions, so this is a spec-level theorem. Together the six give the biconditional halting correspondence of the Theorem in §6, with explicit fuel quantification. **TC is now a theorem** — universality is its own milestone, discharged (§7.5, Layer B).

**Halting-correspondence decomposition (the four requirements a halting proof needs, mapped onto the six theorems).**

| Requirement | Discharged by | What it rules out |
|---|---|---|
| **Forward simulation** — Minsky runs `t` steps ⇒ MTL reaches the encoded `t`-th config | `p5_stutter_step` (one step) + `p5_simulation` (`t` steps) | MTL failing to follow a transition |
| **Boundary preservation** — `R` holds at *every* `[Dup, Apply]` loop boundary | `p5_stutter_step` postcondition (`rep(prog, m2, s2)`), carried inductively by `p5_simulation` | representation drift mid-run |
| **Halt preservation** — Minsky halts ⇒ MTL halts with the encoded output | `p5_halt_forward`, `p5_halt_forward_monotone` | MTL running past a Minsky halt |
| **Reflection / no-spurious-halt** — MTL halts (for some fuel) ⇒ the Minsky machine is (eventually) halted | `p5_halt_reverse`, via `p5_diverge` | MTL *silently terminating early* at a non-halt Minsky state |

The fourth is the load-bearing one flagged by the external review: without it, the proof would show MTL can *follow* transitions but not that it cannot *silently stop* early. It is genuinely proved (not asserted by symmetry): `p5_halt_reverse` (`∃ fuel. spec_run(s0,·) is Halt ⇒ ∃ t. minsky_run(prog,m0,t) is None`) is the contrapositive of `p5_diverge` (Minsky diverges ⇒ *every* fuel `spec_run` is `FuelExhausted`, never `Halt`), which itself rests on `p5_reach_steps` (t Minsky steps cost `k ≥ t` all-`Next` MTL steps) and `lemma_stepn_next_run`. The chain bounds the pre-halt run length from below, so it is non-vacuous. There is **no open halting gap**.

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

- Spec side: `SpecValue { Int(int), Quote(Seq<SpecWord>) }`, state as `(Seq<SpecValue>, Seq<SpecWord>)`. Recursive datatype with `decreases` on structural size. (No `Str` variant — the core carries only `Int` and `Quote`, matching `mtl_core.rs`; the v0.1 draft's `Str(Seq<char>)` is dropped from the core, §3.)
- Exec side: `enum Value { Int(i64), Quote(Vec<Word>) }` with a deep-view mapping to spec values (i64 → int is where the overflow obligations surface).
- Driver: `fn run(vm, fuel: u64) -> Outcome` — a fuel-bounded loop. **We do not prove termination of `run`; TC forbids it.** We prove that `run` is a correct finite unrolling of `spec_step` up to `fuel`, and that `FuelExhausted` is the only outcome the spec doesn't determine. Fuel exhaustion is semantically load-bearing for linear resources (§14.2).

### 7.2 Invariants ("impossible states are impossible")

- **I1 — Totality of step:** `spec_step` is a total function on all states (Verus enforces this by construction; no `arbitrary()`, no partial match).
- **I2 — No panics:** exec interpreter has no `unwrap`, no indexing without proof, no unchecked arithmetic. All Vec pops are guarded by length preconditions discharged from the match structure.
- **I3 — Value well-formedness:** every `Quote` contains a well-formed program (structural, by construction of the parser's postcondition; well-formed programs have every `PushInt` ≥ 0, matching the unsigned-literal lexer §2.3).
- **I4 — Fault stability:** `Fault` states are terminal — no rule resumes from a fault.

### 7.3 Proof obligations (spine proofs first)

| ID | Statement | Kind |
|---|---|---|
| **P1** | Determinism: `spec_step` is a function (free — it's a `spec fn`); additionally, the §4.1 rules are proven non-overlapping and the fault precedence (§4.4) is faithful to the published rules | spec |
| **P2** | Refinement: `exec_step` faults exactly when `spec_step` faults, else `vm'@ == next state of spec_step(vm@)`; overflow in exec ↔ result outside i64 in spec | refinement |
| **P3** | Progress: every state is `Next`, `Halt`, or `Fault` — no stuck states | spec |
| **P4** | Parser round-trip over well-formed programs (all `PushInt` ≥ 0, §2.3): `parse(print(p)) == Ok(p)` **and** `print(parse(src)) == canonicalize(src)`; `parse` postcondition establishes I3. The second direction catches normalization surprises that matter to token accounting | exec |
| **P5** | TC bounded-stutter (multi-step) lemma (§6.5) over the spec step relation, with counters as unary quotations — **proved** in `crates/mtl-core/src/p5_universality.rs` (six theorems; two-way fuel-quantified halting correspondence, incl. no-spurious-halt) | spec, hard, ✅ done |
| **P6** | Tail-call space bound: for programs in the defined loop normal form (recursive `: !` in tail position, §4.2), *semantic continuation length* is bounded across iterations | spec, v0.2 |
| **P7** | Heap acyclicity: the core value heap is a DAG in every reachable state (§14.3) | spec, future |
| **P8a/b/c** | Exact reference counts / prompt reclamation / normal-termination resource closure (§14.3) — replaces the single "no leaks" claim | spec, future |
| **P9 (split)** | Checker soundness, split into static soundness / guard-insertion soundness / host conformance / normal-exit resource theorem (§14.5) | spec, future |

If P2 gets hard, that's the design-smell signal: refine the representation (e.g., continuation as a persistent list vs. Vec splice) before fighting the prover.

**Verification status (as stated in `crates/mtl-core/src/mtl_core.rs`):** P3 (progress); P1 (by construction — total, non-overlapping match, no wildcard arm); truncating div/mod semantics via concrete witnesses *and* the general correctness lemma (`a = q·b + r`, `|r| < |b|`, remainder sign follows dividend) discharged with `nonlinear_arith`; deep-view termination through nested quotations (lexicographic datatype-height measure); and the `smoke_dup_apply` theorem that `: !` self-applies in exactly two spec steps, retaining the quotation while splicing its body into the continuation; and **P5** (Turing completeness — the two-counter Minsky simulation over `spec_step`, §6.5), discharged admit-free in `crates/mtl-core/src/p5_universality.rs` (118 verified, 0 errors), which re-verifies the frozen core alongside (still 76/0). Open holes: P2 (needs the GREEN interpreter), P6–P9 (scheduled per §7.5).

> **Reproducibility (per review §7).** "N queries, 0 errors" is evidence about a *particular artifact*. Claims must be accompanied by a **pinned Verus commit** (not a date-shaped version string), the exact invocation command, solver versions, and a checked-in proof log. This is Go/No-Go gate **G7** (§15); until it is met, the verification status above is provisional.

> **Current verification status (2026-07-11) — honest caveat.** The pinned Verus toolchain is **`0.2026.07.05.49b8806`**. verus-lang publishes release assets with a trailing build hash, so the date-shaped `0.2026.07.05` names no downloadable asset and returned HTTP 403; the full build id is the real, fetchable pin (corrected in CI, #5), and it also satisfies review §7's "pin a precise commit, not a date-shaped version." With the correct pin, `verus` runs in CI (the `verus verify` check). The spec-level obligations above (P1, P3, div/mod, deep-view termination, `smoke_dup_apply`) are written as genuine proofs in the artifact; the GREEN-phase obligations (P2 refinement, P4 round-trip) are additionally evidenced by **differential-oracle property tests** (exec vs. a naive reference interpreter, §7.4) with their Verus proof contracts currently **stubbed via `admit()`** pending the GREEN interpreter. Until the `verus verify` check is green with a checked-in proof log (gate **G7**), treat "machine-checked" here as *contract-stated, CI-and-proptest-evidenced* rather than fully SMT-discharged.

### 7.4 Test layer (RED/GREEN — complements, not replaces, proofs)

Proofs cover what we modeled; property tests poke at what we forgot to model:

- **Happy path:** golden programs (factorial, Fibonacci, the Minsky simulator itself) with expected final stacks.
- **Boundary:** empty program, deeply nested quotations, `i64::MIN / -1`, `i64::MIN % -1`, quotation catenation at size limits.
- **Property (proptest):** (a) fuzz arbitrary well-formed programs — interpreter never panics, always returns in ≤ fuel steps (re-checks I2/P3 against the *actual binary*, catching spec/exec transcription gaps); (b) differential testing exec vs. a naive unverified oracle interpreter (currently the primary evidence for P2/P4 while `verus` is unavailable, §7.3); (c) `parse ∘ print` round-trip on arbitrary ASTs (re-checks P4, both directions).
- **Regression:** every bug found by Havoc-style fuzzing lands as a named test before the fix commits.

Coverage gate 85–90%; criterion benches on the step loop.

### 7.5 Layered proof hierarchy (six milestones, not one cliff)

Per review §20, the proof program is layered so that universality, typing, heap, and effects are *separate publishable milestones* rather than one P9-shaped obligation:

- **Layer A — pure core:** `Int(i64) | Quote(Program)`; no strings, definitions, host calls, heap identities, or resources. Total step, explicit fault precedence (§4.4), exec/spec refinement (P2), parser/printer properties (P4), no interpreter panic. *This is v0.1's scope.*
- **Layer B — universality:** ✅ **done.** The unbounded representation of §6.2 (unary-quotation counters); representation invariant, instruction simulation, and two-way fuel-quantified halting correspondence (P5), all proved in `crates/mtl-core/src/p5_universality.rs`. TC is a theorem (§6).
- **Layer C — static stack typing:** literal quotations only; preservation, progress excluding arithmetic faults, branch-stack compatibility.
- **Layer D — dynamic quotation composition:** effect-carrying quotation values or runtime guards; gradual guarantee.
- **Layer E — heap implementation:** allocation identities, explicit refcount semantics, edge-age acyclicity (P7), exact counts, no unreachable nodes after reclamation (P8a/b).
- **Layer F — host effects and linear resources:** host contracts and cancellation semantics; at-most-once use, no live resources on normal halt (P8c), host-conformance preservation.

---

## 8. Effects: Host-Injected Capabilities

The core is pure — `spec_step` closes over nothing. As of **v0.4** every `Call(name)` is a **suspension**, not a fault: `spec_step`'s `Call` arm yields the fourth outcome `Invoke(name, stack, cont)` (§8.2). The core does **not** decide whether a name is bound — there is no in-core dictionary — so `Error::UnknownWord` (still present in the `Error` enum) is **no longer reachable from the `Call` arm**; grant/deny is a host-side decision returned as a `HostFault` (§8.3). This is the actual behavior of `spec_step` in `mtl_core.rs` and is the normative core semantics; the older sketch in which `Call` faulted with `UnknownWord`, and in which a separate `CoreStep`/`HostResult`-threading-`host_state` machine was proposed, is superseded by what follows.

### 8.1 The pure core and the trust boundary

`SpecState = { stack: Seq<SpecValue>, cont: Seq<SpecWord> }` carries no word dictionary, host state, effect trace, or capability signature. The pure-core theorems (P1 determinism, P2 refinement, P3 progress) hold **independently of any host** — they are unconditional statements *about the core*. The v0.1 draft's phrase "the verified core's theorems are unconditional" is made precise as:

> The pure-core theorems are independent of host behavior. End-to-end guarantees about a *running* MTL program additionally require assumptions about the host contract (§8.3): a host word may panic, diverge, return a malformed value, violate its declared stack effect, leak a resource, or mutate ambient state. Trusted or external components remain part of the trusted computing base; they are not discharged by the core proofs.

The **only** channel between the verified core and the untrusted host is the `Invoke` value: it carries `(name, stack_snapshot, cont)` *out* of the core, and the host returns `Resume(result_stack)` or `HostFault(code)` back *in*. Host state **never enters the core**; `cont` is opaque to the host, which must hand it back untouched at resume. This is the load-bearing design choice (design `docs/design/v0.4-effects.md` §2.4): the core closes over nothing, so P1/P2/P3 remain intact.

### 8.2 `Invoke`: the fourth `SpecStep` outcome

`Invoke` is the **fourth constructor of `SpecStep`** (not a parallel `CoreStep` enum — the earlier §8.2 sketch is superseded). `SpecState` (stack + cont, closing over nothing) *is* the core state; a parallel enum would duplicate the whole `spec_step → exec_step → run` spine for no gain. Its exact shape in `mtl_core.rs`:

```
SpecStep ::= Next(SpecState)
           | Halt(Seq<SpecValue>)
           | Fault(Error)
           | Invoke(Seq<char>, Seq<SpecValue>, Seq<SpecWord>)   -- (name, stack snapshot, continuation)

SpecWord::Call(name) => SpecStep::Invoke(name, s.stack, rest)   -- rest == cont after the consumed Call
```

Three properties define the outcome:

- **Every `Call` yields.** The `Call` arm stopped faulting; it now suspends unconditionally. The core does not distinguish bound from unbound names (no in-core dictionary). `Invoke` carries an immutable **snapshot** of the whole stack (a `Seq<SpecValue>`, not a delta) and the continuation `rest` — the tail *after* the `Call` word, so the `Call` is already consumed and is never re-executed on resume.
- **Terminal-within-a-run.** `Invoke` is a **base case** of `spec_run` (it terminates the run like `Halt`/`Fault`), so `spec_run`'s `decreases fuel` measure is **byte-untouched** — the core never threads `host_state` and never loops on host results. The mirrored exec-side enums (`SpecOutcome`, `StepResult`, `Outcome`) each gain the matching `Invoke` arm, and `spec_run`/`run` suspend (return, not recurse).
- **Resumption is a fresh run.** Continuing after a host call is a **new** `spec_run`/`run` seeded with `SpecState { stack: result_stack, cont }` — never a re-entry into the suspended step. This is what makes at-most-once hold in-core (§8.5).

The verified core re-verifies at **76 verified, 0 errors** with this arm added: P3 stays exhaustive (`… || is Invoke`), P1 stays deterministic (the new arm is non-overlapping), and P2 reuses the existing `view_stack`/`view_words` homomorphism lemmas to image the `Invoke` snapshot and `cont` under `deep_view`. `Invoke` introduces no new `Value` or `Word` constructor, so the deep-view termination measure and both `Clone` stubs are untouched (design §4).

### 8.3 The host seam and the drive loop

The impure host runner lives *above* the verified core, in the TCB (`crates/mtl-core/src/host.rs`). The seam, exactly as implemented — note `HostResult` does **not** thread `host_state` (it stays host-local):

```rust
pub enum HostCode {                    // host-side fault codes (never raised by the core)
    InputClosed, OutputCapExceeded, BudgetExhausted, ToolError, Timeout, NotGranted,
}
pub enum HostResult { Resume(Vec<Value>), HostFault(HostCode) }   // host_state stays host-local
pub trait Host { fn service(&mut self, name: &str, stack: Vec<Value>) -> HostResult; }
pub struct CapabilitySig {             // a capability signature as data (declarations live host-side)
    pub name: String, pub consumes: usize, pub produces: usize, pub faults: Vec<HostCode>,
}
pub enum RunResult { Done(Vec<Value>), Faulted(Fault), Cancelled, HostFaulted(HostCode) }

pub fn drive(mut vm: Vm, fuel: u64, host: &mut dyn Host) -> RunResult {
    let mut remaining = fuel;                         // ONE global budget for the whole run
    loop {
        if remaining == 0 { return RunResult::Cancelled; }   // clean, BETWEEN steps
        match exec_step(&mut vm) {
            Step::Next          => remaining -= 1,     // charge one unit per in-core step
            Step::Halt          => return RunResult::Done(vm.stack),
            Step::Fault(fault)  => return RunResult::Faulted(fault),
            Step::Invoke(name) => {                    // Call yielded; costs NO fuel
                let snapshot = core::mem::take(&mut vm.stack);   // vm.cont already past the Call
                match host.service(&name, snapshot) {
                    HostResult::Resume(result_stack) => vm.stack = result_stack,  // resume in place
                    HostResult::HostFault(code)      => return RunResult::HostFaulted(code),
                }
            }
        }
    }
}
```

The loop: step the pure core one small step at a time, charging **one** unit of a single `remaining` budget per in-core step → on `Invoke(name)` call `host.service(name, stack)` → `Resume(stack')` resumes the core **in place** from the already-advanced `cont` (an `Invoke` costs no fuel and does **not** reset `remaining`), `HostFault(code)` terminates the drive. The `NotGranted` code is how a host rejects an ungranted capability host-side, since the core yields on *every* `Call`. `HostFault` performs **no** effect and leaves the drive terminated (`HostFaulted`). Because `drive` owns the step loop over the verified `exec_step`, the pure core's `run`/`Outcome` and the entire Verus surface are byte-untouched.

**Host contract.** For end-to-end theorems, each capability declares a stack effect (`consumes`/`produces`) and a fault contract (`faults`), and the host runner is *assumed* to conform: preserve the untouched stack prefix, return the declared arity/shapes, raise only faults in its contract, not leak linear resources, service each yielded `Invoke` **at most once**, resume with exactly the carried `cont`, and either terminate or signal `HostFault`. These assumptions (design §3.2, the P9 host-conformance sub-judgment / Layer F) are the price of the trust boundary; the core proofs do not establish them for arbitrary Rust hosts. `CapabilitySig` is the minimal data shape a sibling host crate builds its registry from; the checker may later carry these as effect rows (review §19: `emit : Str -> Unit ! {output}`).

### 8.4 Capability-name grammar (lexer-safe NAME tokens)

Capability names are **bare `Call` name tokens** — there is no invoke sigil (design §9: a sigil spends a scarce glyph to make programs *longer*). A capability name is therefore exactly a NAME token as produced by the `mtl-syntax` lexer, whose token class is:

> **NAME** = `[a-z][a-z0-9]*` — a leading ASCII lowercase letter (`a`–`z`) followed by zero or more ASCII lowercase letters or ASCII digits.

This is the maximal-munch rule at `crates/mtl-syntax/src/parse.rs:186–200` (arm `'a'..='z'`, continuation predicate `is_ascii_lowercase() || is_ascii_digit()`, producing `Word::Call(name)`). No uppercase, no underscore, no hyphen, no `?`, no other punctuation is part of a NAME.

**Consequence — the design doc's illustrative capability names do not lex.** The sketches `read-line`, `read-state`, `done?` in `docs/design/v0.4-effects.md` (§3.1, §8) are *illustrative only*: the lexer reads each self-delimiting glyph separately, so `-` lexes as `Prim::Sub` and `?` lexes as `Prim::If` (`crates/mtl-syntax/src/ast.rs`: `('-', Prim::Sub)`, `('?', Prim::If)`, `('_', Prim::Drop)`). Thus `read-line` parses as `Call("read") · Sub · Call("line")` — three words, not one capability call — and `done?` as `Call("done") · If`. **Real capability names must be lexer-safe single NAME tokens**: e.g. `readline`, `readstate`, `emit`, `done`, `step`, `tokenize`, `emitint`. Future tasks must not copy the hyphen/`?` sketch names into program text.

### 8.5 Fuel, host metering, and cancellation

**Fuel is a pure in-core step counter, cumulative across resumptions** (design §6, Option B). `spec_run(s, fuel)` still `decreases fuel` per segment, but `drive` threads a **single `remaining` budget over the ENTIRE driven run**: the total number of in-core steps summed across every inter-`Invoke` segment is bounded by `fuel`. An `Invoke` yield is a clean boundary between steps and costs **no** fuel; servicing it does **not** reset `remaining`. This is what makes metering total. A program that yields a capability inside a non-terminating loop (a tier-3 `agent_loop` whose `done` never trips) reaches `Invoke` before any single segment exhausts fuel on *every* iteration; were `fuel` re-supplied per segment, exhaustion could never fire and `drive` would spin forever — defeating the global-budget guarantee. With one cumulative budget the summed in-core steps run out and the loop is cancelled. Host cost is **never** folded into `fuel`: host work — per-capability call budgets, output-byte caps, service time — is bounded by a **separate host-side meter**, debited only at `Invoke` yield points, surfacing as a `HostCode` (`BudgetExhausted`, `OutputCapExceeded`, `Timeout`) via `HostResult::HostFault`. In-core instructions and host effects are orthogonal budgets; a single scalar cannot price both.

**Cancellation is clean between steps.** The budget is checked *between* steps (`remaining == 0 => Cancelled` before `exec_step`), so exhaustion occurs only at a step boundary — before the core emits an `Invoke`, or after it has fully re-entered from a prior `Resume`. It can **never** occur mid-capability, because the core is suspended while the host acts (they are separate machines). Cancellation therefore cancels with **zero partial in-core effect**: `drive` returns `Cancelled` at a step boundary with no half-executed step. When the budget expires exactly at a pending `Call`, that `Invoke` is simply never emitted, so no `(name, stack)` is handed to the host at the cancellation point and at-most-once is preserved. (An endless-capability loop is serviced a *bounded* number of times before cancellation — each service is followed by ≥ 1 fuel-charged step, so the service count can never exceed `fuel`.) A host that bounds its *own* service time uses a host timeout resolving to `HostFault(Timeout)`; the core's fuel accounting is untouched.

**Host contract assumptions remain the TCB.** As in §8.1, end-to-end guarantees still rest on the conforming-host assumption; the core proves only P1/P2/P3 about itself.

---

## 9. Definitions — deferred out of v0.1

> **Status: deferred entirely. Not part of v0.1.**

The v0.1 draft sketched `#f[...]` to bind a quotation to a single-letter name. On review this is **under-specified**: `#` is not in the lexical classes (§2); it is unclear whether definitions are a lexical macro, an AST desugaring, or a runtime dictionary lookup; and scope, shadowing of host words, behavior inside quotations, forward references, whether `f` parses as `Call("f")`, printer behavior, whether the declaration counts toward token benchmarking, and whether expansion duplicates bodies are all unresolved. Rather than ship an ambiguous feature, **definitions are removed from v0.1 entirely.** When reintroduced, the spec will pick exactly one mechanism — lexical macro expansion before parsing, AST desugaring before execution, or runtime dictionary lookup — and specify it fully, including its interaction with the token metric and the checker.

Recursion does **not** depend on definitions — it is `: !` (§6.3) — so their removal does not affect the semantics or the proofs.

---

## 10. Benchmark Suite (define before optimizing)

The benchmark is the project's oracle, and it can be gamed several ways; the design below is built to resist that.

### 10.1 Corpus splits (anti-overfitting)

The task corpus is partitioned into four disjoint sets, and glyphs/primitives are optimized on *different* data than they are evaluated on:

- **Glyph-training corpus** — used only to measure bigram/trigram frequencies for glyph assignment (§11).
- **Primitive-admission corpus** — used only to decide whether a candidate primitive pays for itself.
- **Development set** — used during implementation for iteration and debugging.
- **Sealed evaluation set** — held out; touched only for the final headline numbers. Never used to admit a glyph or a primitive.

Optimizing glyphs on the training split and admitting primitives on the admission split — then reporting on the sealed split — is what separates *general* token compression from benchmark-fitting (introducing a primitive in response to a task and then evaluating it on that same task).

### 10.2 Task sets are versioned against the primitive set

Because the primitive set is open (§5), each task set is tagged with the primitive set it assumes:

- **T_v0** — primitive set §5, no strings, no host capabilities.
- **T_v0.2** — adds whatever v0.2 admits: string primitives, list values, `uncons`, host capabilities.

String tasks (string reverse, run-length encoding) and capability-driven agentic tasks are **T_v0.2**: they are *impossible* in T_v0, which has no `Str` value at all (§3) and no host words. Reporting a T_v0.2 result as a v0.1 number would be a category error.

### 10.3 Comparison baselines (per task)

`sol(t, L)` is measured for a *panel* of languages per task, not MTL vs. one strawman:

- **Idiomatic Python**, **minified Python**, and **Python generated by the evaluated model** (controls for researcher golfing effort);
- **Forth** and **Joy** (concatenative controls);
- **jq** and a compact S-expression DSL where suitable;
- **MTL without corpus-optimized glyphs** (mnemonic / un-optimized) and **MTL with optimized glyphs** (isolates the glyph-assignment contribution).

"Shortest known correct solution" alone measures researcher search effort as much as language merit; the panel controls for that.

### 10.4 Total-token accounting

Per task, per language, per tokenizer, record **total inference tokens to a correct solution**, not just program length:

```
total_tokens = amortized_language_instruction_tokens   -- teaching the language
             + generated_program_tokens
             + validator_error_tokens                  -- checker/parse errors seen
             + repair_attempt_tokens                   -- failed attempts (full cost)
             + execution/tool_tokens
```

`tokens × attempts` (the v0.1 draft's metric) is **insufficient**: an "attempt" hides its size — a failed attempt may be a five-token patch or a 2,000-token explanation. Measure total consumption directly.

### 10.5 Warm vs. cold agent protocol

Each task is run under two conditions:

- **Warm agent** — the language lives in the model's system context or fine-tuning (amortized instruction cost near zero).
- **Cold agent** — the language reference is supplied in the prompt (instruction cost paid per task).

Report both; they answer different questions (steady-state cost vs. acquisition cost).

### 10.6 Metrics

- **Headline: correct solutions per million inference tokens** (on the sealed set).
- Supporting: `P(correct within budget)`; median total model-output tokens to first correct; mean censored at budget; execution success after validator acceptance; semantic diversity of failures.

**Success gate (Abrash rule), restated:** MTL ships only if it beats the panel — in particular idiomatic Python — on *correct solutions per million inference tokens at equal-or-better correctness*, on the sealed set. Token-cheap but unwritable loses; the objective is to minimize expected total inference tokens to correct execution subject to a fixed correctness target, **not** raw program length (see §11 on why the most compressible alphabet may not be the most generatable one).

**Gate verdict (v0.8, concluded — the surviving ship criterion is this CSPM form, not ≥3× compression):** the ≥3× *static-compression* sub-gate FAILED out-of-sample — held-out static compression is **1.67×** (vs 3.7–3.9× in-sample), broad generated distribution ~1.7×, and ~1.03× against fair terse Python; the dev corpora co-evolved with the primitives, so the in-sample figure was a benchmark-fitting artifact. What holds out-of-sample and defines the ship criterion: **CSPM 2.124× held-out** (correct solutions per million tokens, MTL/Python, widening from dev's 1.274×) at **100% pass@5**, plus verified capability confinement. Static compression is retained as a **niche** property — 2–4× on loop/fold/recursion shapes, ≤1× on scans and builtin-heavy code. Full post-mortem: `bench/BASELINE-SEALED.md` §6 and `docs/design/v0.8-generalization.md`.

A cross-cutting writability concern (Gemini review): point-free code imposes a **stack-juggling tax** — bringing a deeply buried value to the top costs routing tokens (`@ ^ ~ @`) that can exceed the savings of omitting a name, and LLMs track an implicit stack poorly across long generations. Two consequences fold into the harness: (1) validator errors must report the **exact typed stack state at the fault**, not just "TypeMismatch at word 14", so the agent can repair its mental model; (2) `pick`/`roll` are prioritized v0.2 candidates (§5).

## 11. Glyph Assignment Protocol (Abrash-style measurement)

1. Write the benchmark solutions using placeholder primitive names.
2. Enumerate candidate glyph assignments (single ASCII punctuation, plus short names for anything that misses).
3. For each assignment, render the full solution corpus and count tokens under each tokenizer — **corpus-level, not per-glyph**, because BPE merging is context-dependent (`:!` may be 1 token; `:?` may be 2).
4. Frequency-weighted optimization: assign the most merge-friendly bigrams to the most frequent primitive *pairs* in the corpus (measure pair frequencies first).
5. Freeze assignment; re-run whenever the primitive set changes. The script lives in-repo; assignments never change without a measurement diff in the ADR.
6. **Pinned tokenizers.** All counts are reported against pinned tokenizer implementations and versions: `tiktoken` `o200k_base` and `cl100k_base` at a recorded release, and a **pinned Claude tokenizer implementation** — a web tokenizer UI is not a reproducible gate. Merge behavior is tokenizer- and revision-dependent, so the snapshot is part of the result.
7. Glyph assignment is measured on the **glyph-training corpus** only and frozen before the sealed evaluation set is touched (§10.1).
8. **Generatability ablation.** The most *compressible* alphabet may not be the most *generatable* one: tokenizer-optimal punctuation can sit on weak learned priors, while verbose Python rides a polished statistical highway. Token count is not information content from the model's perspective. Run ablations over alphabets — mnemonic names, arbitrary punctuation, tokenizer-optimized punctuation, and model-optimized punctuation discovered by generation experiments — and select for *generatability at fixed correctness*, not raw compressibility.

## 12. Open Questions

1. **List/record values in core vs. quotation-encoded** — quotation encoding is elegant and proof-cheap but token-expensive per access; measurement decides (v0.2).
2. **Static arity/type checking** — subsumed by the linearity checker (§14): the multiplicity-typed stack-effect checker is a strictly stronger validator, converting more runtime faults into pre-execution errors at zero token cost.
3. **Continuation representation** — Vec splice (`q ++ p`) is O(n) per apply; a cons-list or rope may be needed. Must not disturb P2; do representation change spec-first per TAVDD.
4. **String primitives** — none in v0; `Str` is not even a v0.1 core value (§3). Admit via benchmark pressure only (T_v0.2, §10.2).
5. **Whether `'` (dip) merges badly** — apostrophe adjacency to `]` is tokenizer-hostile in early checks; may swap glyph with a lower-frequency primitive.

## 13. Roadmap

| Phase | Deliverable | Gate |
|---|---|---|
| SPEC | This document + `mtl_core.rs` Verus spec skeleton | review |
| PROOF | P1–P4 verified; **P5 proved** (`p5_universality.rs`, TC theorem, §6.5) | `verus` green |
| RED | Golden + boundary + proptest suites (failing) | tests exist, fail |
| GREEN | Exec interpreter passing tests, P2 discharged | tests + proofs green |
| REFACTOR | Continuation representation tuning under green lights | benches |
| CHECK | §14 multiplicity checker + P7–P9 (future work) | `verus` green on split P9 |
| MEASURE | §10 suite vs. panel; §11 glyph freeze | **CONCLUDED** — ≥3× compression gate FAILED out-of-sample (held-out 1.67×); CSPM/reliability/confinement thesis holds; post-mortem in §10.6 / `docs/design/v0.8-generalization.md` |

## 14. Linearity and Memory Model (v0.2+ exploratory — future work)

> **Status: exploratory future work (v0.2+).** Nothing in §14 is part of the v0.1 verified core. `mtl_core.rs` implements no heap model, no refcount instrumentation, no multiplicity checker, and no linear resources. The claims here are a research direction, deliberately stated conservatively below; the strong forms in the v0.1 draft (a literal Perceus equivalence, an unconditional no-leak theorem, `dip`-as-borrow, "exactly once" resources) overreached what any current artifact proves. P7–P9 are **not** scheduled for v0.1; they are Layers E–F (§7.5).

### 14.1 The structural observation

Rust needs a borrow checker because **names create aliases**: multiple variables can reach one value, so ownership must be tracked through binding structure. MTL has no binders. The only way a value is aliased is `:` (dup) / `^` (over); the only ways it dies are `_` (drop) or consumption by a word. So **every MTL program already contains its ownership operations in the program text** — the most interesting structural insight of this section, and directionally right.

The safer statement of the Perceus connection: MTL exposes the structural events (dup = contraction, drop = weakening) that a Perceus-like precise reference-counting system ordinarily *infers*. In MTL that inference is closer to the identity. But Perceus is a precise reference-counting *and reuse* system over a particular linear functional core — **not** merely "programs contain dup and drop", so this **resembles** Perceus rather than **being** it. Honest claim: *MTL exposes the structural events that a Perceus-like system tracks, potentially simplifying exact reference-count accounting.*

**Design consequence:** memory safety would be enforced by a *static checker over unmodified programs*, not by syntax. The default path costs **zero additional tokens** — the multiplicity information lives in primitive signatures and the checker, never in program text.

### 14.2 Multiplicity discipline

Every stack type carries a multiplicity:

| Multiplicity | May `:` / `^` | May `_` | Consumption |
|---|---|---|---|
| **unrestricted** (`Int`) | yes | yes | no requirement |
| **affine** (`Quote`; `Str` when it arrives in v0.2) | yes (refcounted) | yes | no requirement |
| **linear** (host resources, v0.2) | **no** — check-time error | **no** — implicit drop is an error | **at most once in all executions; exactly once on every normal terminating path** (§14.2a) |

- Words are linear function signatures: inputs are consumed, outputs are produced. This is already how §4.1 is written — no rule copies a value implicitly.
- Linear resources get Rust move semantics *as a restriction rather than an annotation*: `:` on a file handle is rejected before execution. Zero tokens, because prohibitions are free.
- The linear tier is what would make agent-generated Kairos-style skills safe to run unattended: "leaked forty handles" becomes "validator rejected the skill."

**14.2a "Exactly once" vs. nontermination.** A Turing-complete program may diverge while holding a linear resource (`Q : !` can preserve `Q` — and a resource it closes over — through infinitely many iterations), so "consumed exactly once" cannot hold unconditionally. The precise discipline is: **used at most once in every execution**; **consumed on every *normal* terminating path**; and **no linear resource left on the final stack**. Cancellation or fuel exhaustion while a resource is live invokes a **host-defined cleanup protocol** — return ownership to the host, close it, preserve the suspended VM, or (worst case) leak — a host-contract decision (§8.2). This is why fuel exhaustion is semantically load-bearing, not merely an implementation cap.

### 14.3 Heap model: acyclic, refcounted, deterministic — not GC

v0 values are immutable, and the only constructors (`;` cons, `,` cat, `[...]` literal) build new values from existing ones. Back-edges are unconstructible, so:

> **P7 (acyclicity — future work).** In every reachable state the *core* value heap is a DAG. Because the only constructors build new values from existing ones, every heap edge `u → v` satisfies `birth(v) < birth(u)` — an age-ordering that entails acyclicity by construction. This is a **constructor-level invariant, not a reachability theorem**, and it holds only for core values: host-injected objects may conceal arbitrary graphs and are excluded unless opaque.

Given exact counts, age-ordered heap edges, and recursive zero-count reclamation, unreachable *core* values are reclaimed deterministically and promptly — the pathological case for reference counting (cycles) is *provably impossible* for core values, so this is no more "garbage collection" than Rust's own `Rc`. The v0.1 draft's flat "acyclic + refcounted = exact deterministic destruction" is the conditional statement above, not an unconditional one.

> **P8 (split — future work).** The single "no leaks" claim is false-or-vacuous as written (for a diverging program "eventually consumed" may never hold) and is replaced by three:
> - **P8a (exact reference counts):** for each heap value `v`, `rc(v)` equals its incoming heap edges plus root (stack/continuation) references.
> - **P8b (prompt reclamation):** after each transition and its reclamation step, every allocated node is reachable from a root.
> - **P8c (normal-termination resource closure):** if a statically checked program halts *normally*, no linear resources remain unconsumed.

These are spec-level invariants over an instrumented heap model in Verus, refined by the exec interpreter via P2 (Layer E, §7.5).

### 14.4 Mutation and non-access intervals without syntax (v0.2+)

- **Uniqueness typing for in-place mutation.** A mutation word requires its target to be statically unique. But `rc(v) = 1` is **not by itself sufficient** for safe in-place mutation: every possible alias must be represented by that count — host aliases, continuation literals, nested quotation values, temporary interpreter references, capability-owned references, suspended effect calls. The actual obligation is `rc(v) = 1 ∧ unique_root(v) ⇒ no other observable path reaches v`. Under that condition, in-place mutation is unobservable, echoing Clean's uniqueness typing and Perceus-style reuse.
- **`'` (dip) is a non-access interval, not a borrow.** `dip` temporarily removes one stack occurrence, runs the quotation, and restores it (the restore is compiled into the continuation — see `spec_step_prim`'s `Dip` arm, which appends `value_to_word(a)` after `q`). This means the quotation cannot access *that stack occurrence* — it does **not** imply the absence of aliases elsewhere, no host mutation, no global handle, or a Rust-sense unique borrow. Precisely: `dip` creates a stack-local, checked interval in which one occurrence is inaccessible.
- **`^` (over) is a duplicate, not a shared borrow.** `over` produces an actual second reference (a refcount increment), not a borrow.
- **Copy-on-write fallback:** a mutation word on a non-unique affine value either faults (strict mode) or clones-then-mutates (COW mode) — a per-word decision made by token accounting.

### 14.5 The checker

A **linear stack-effect checker**: abstract interpretation over stacks of multiplicity-annotated types, run pre-execution by the validator.

- **Literal quotations** (the overwhelmingly common case): fully static — the checker recurses into `[...]` bodies, joins branches at `?`, and verifies loop bodies in `: !` normal form preserve their stack-type signature.
- **Runtime-composed quotations** (`,` / `;` on non-literal operands): the known-hard problem in typing concatenative languages (cf. Cat, Kleffner; row-like stack typing; quotation typing). MTL's answer is **gradual**: at truly dynamic composition points, either a checker-visible effect annotation or a deferred runtime multiplicity check. The escape hatch costs tokens *only where dynamism is actually used*.
- **Branch-dependent stack shapes at `uncons` (acknowledged for the future checker).** `uncons` (`>`) is a **sum-typed deconstructor**: on a non-empty quote it leaves `head [tail] Int(1)` (three cells), on an empty quote it leaves `Int(0)` (one cell), and on a malformed head (a bare `Prim`/`Call`, not a value word) it faults `TypeMismatch`. The two success branches therefore have **different stack heights and shapes**, discriminated by the top flag `0`/`1` and reconciled at the following `?`. The P5 universality construction (§6.2, §6.4) relies on exactly this effect. A linear stack-effect checker must model `uncons` as producing a **tagged sum of stack shapes** (joined at the matching `?`), not a single fixed effect — the checker's branch-join at `?` is where the two `uncons` outcomes are unified. This is recorded here as a concrete requirement the checker design must satisfy.

> **P9 (split — future work).** The single "headline" checker-soundness theorem is too broad for a gradual checker over higher-order quotations. It is replaced by separate judgments and theorems:
> - `check_static(p) = Static(effect)` — **static soundness:** statically checked programs never fault with `Underflow` or `TypeMismatch`.
> - `check_guarded(p) = Guarded(effect, obligations)` — **guard-insertion soundness:** the inserted runtime multiplicity guards discharge the residual obligations.
> - **host conformance:** end-to-end soundness holds *given* the host contract (§8.2).
> - **normal-exit resource theorem:** a statically checked program that halts normally leaves no linear resource unconsumed (this is P8c).
>
> Bundling these into one theorem would be a proof obligation "shaped like a small moon." Each is its own milestone (§7.5, Layers C/D/F).

P9-family soundness is also the metric's best friend: it converts runtime faults into pre-execution validator errors, directly raising agent success rate and protecting the headline metric (§10.6) — provided validator errors report the exact typed stack state at the fault (§10.6).

### 14.6 Token accounting summary

| Feature | Token cost |
|---|---|
| Ownership / moves / drops | 0 — already in program text (`:`, `_`, consumption) |
| Lifetimes, `&`, `mut`, annotations | 0 — do not exist |
| Linear resource discipline | 0 net — explicit `close` words you'd write anyway |
| Non-access intervals / duplicates | 0 — `'` and `^` already exist |
| Dynamic-composition escape hatch | >0, rare, self-punishing — paid only where used |

**Scope honesty:** interpreter memory safety was already guaranteed (verified Rust). §14 is the *aspiration* to make *MTL programs themselves* memory-safe as a language property — free correctness in pure v0, load-bearing the moment resources and mutation arrive in v0.2. None of it is proven in v0.1; it is future work (Layers E–F, §7.5).

---

## 15. Go/No-Go Gates (v0.1 → v0.2)

Before expanding the language or publishing quantitative claims, these gates must be green (adapted from the adversarial review §22; see `docs/reviews/2026-07-11-adversarial-review.md`):

- **G1** Deterministic lexer specification with test vectors (§2.3). — ✅ specified in this revision.
- **G2** Complete step semantics including fault precedence (§4.4). — ✅ specified in this revision.
- **G3** Separated pure-core / host-call boundary (§8). — ✅ implemented in v0.4: `spec_step`'s `Call` arm yields `Invoke` (the fourth `SpecStep` outcome), and the unverified host seam (`mtl-core::host`) drives it. Core re-verifies at 76 verified, 0 errors.
- **G4** Corrected TC theorem, or explicit withdrawal of the claim (§6). — ✅ **corrected TC theorem proved** (P5, `crates/mtl-core/src/p5_universality.rs`, 118/0); the invalid `i64`-counter argument was withdrawn (v0.1.1) and rebuilt on unbounded unary-quotation counters (§6).
- **G5** Five real programs written entirely in the stated v0 primitive set (T_v0). — ☐ open.
- **G6** Reproducible tokenizer measurements for those programs against pinned tokenizers (§11). — ☐ open.
- **G7** Pinned Verus commit and checked-in proof log (not a date-shaped version). — ◑ pin corrected to the full build id **`0.2026.07.05.49b8806`** (the date-shaped `0.2026.07.05` named no asset → HTTP 403; fixed in CI #5); the checked-in proof log remains open, so this gate is not yet fully met (§7.3).
- **G8** P2 discharged for at least the pure arithmetic/quotation core. — ☐ open (needs GREEN interpreter; currently proptest-evidenced with `admit()` stubs, §7.3).
- **G9** §14 reduced to claims supported by an actual heap semantics. — ◑ §14 frozen as future work; claims softened.
- **G10** Benchmark split preventing glyph and primitive overfitting (§10.1). — ✅ specified in this revision.

## 16. Changelog

- **Spec unchanged — claims restatement (Abrash gate concluded)** (2026-07-15) — Documentation/claims reconciliation only; **no primitive, glyph, `spec_step`, `Outcome`, or semantic change** and **no spec-version bump**. The ≥3× **Abrash compression gate is concluded FAILED out-of-sample**: held-out static compression **1.67×** (`bench/BASELINE-SEALED.md`), broad generated distribution **~1.7×** (`docs/design/v0.8-generalization.md`, 1145 shapes), **~1.03×** against fair terse Python — versus the in-sample **3.7–3.9×** (T_v0 3.72×, tier-2 3.87×/3.92×), which was a benchmark-fitting artifact of dev corpora that co-evolved with primitive admission. The **restated shipping thesis** is (1) agent **reliability** — 100% pass@5 on unseen tasks, equal to Python; (2) **per-solution economics** — CSPM **2.124×** held-out (correct solutions per million tokens, MTL/Python, widened from dev's 1.274×); (3) **verified confinement** — the machine-checked core plus capability confinement. Static **compression is retained as a niche property** — 2–4× on loop/fold/recursion shapes, ≤1× on scans and builtin-heavy code — not the headline. README and spec §1.1 / §10.6 / §13 roadmap MEASURE row reconciled to this framing. `cargo test --workspace` unaffected (no code touched).
- **Spec unchanged — arena backend promoted to DEFAULT engine** (2026-07-15) — The arena execution backend (`crates/mtl-arena`) is now the **default execution path** across the user-facing entry points (`mtlrun`, `tier3run`, the `mtl-host` driver `driver::drive`, and the corpus validation gate), selectable via an `Engine` seam (`--engine=arena|interp`, default `arena`). This is a **backend/default-selection change only — the specification, primitive set, `spec_step`, and `Outcome` set are byte-untouched**; this entry advances **no** spec version. The promotion rests on the now-**discharged** arena refinement obligation (issue #47): the machine-checked Verus proof `crates/mtl-arena/proofs/arena_verus.rs` (145 verified / 0 errors, unconditional, admit/assume-free, with fault parity) establishes that the arena refines `spec_step`, so the "never default without proof" precondition is met and the owner has ratified arena-as-default. The `mtl-core` interpreter (`crate::interp`) is **not retired** — it remains the reference twin and **differential anchor**, reachable behind an explicit `--engine=interp` / `driver::drive_interp` selection. Both engines share the same host seam (`mtl_arena::host::arena_drive` ≡ `mtl_core::host::drive`, outcome-for-outcome, same global-fuel/cancellation), and the differential oracle continues to run **BOTH** engines and compare them (`tests/oracle.rs`, 148 cases; fault-corpus `FaultInfo` parity; host-driver parity in `tests/host_parity.rs` + `mtl-host/tests/arena_backend.rs`) — the twin runs are what make the default flip safe. `cargo test --workspace` green.
- **Spec unchanged — optional arena backend added** (2026-07-13) — An **optional, opt-in** arena execution backend (`crates/mtl-arena`, admitted by `docs/design/v0.5-refactor.md`) was added alongside the reference interpreter. It is **semantics-frozen**: no new primitives, no new `SpecStep`/`Outcome` constructor, no change to `spec_step` or to this specification — this entry advances **no** spec version, and the `mtl-core` interpreter (`crate::interp`) remains the reference twin and normative oracle. The arena is selected explicitly (`mtl_arena::run_arena` for pure execution, `mtl_arena::host::arena_drive` for the host-driven seam) and is never silently substituted; it is **differentially validated** bit-identical against the interpreter (47-case differential oracle, fault-corpus `FaultInfo` parity, and host-driver parity against `mtl-core::host::drive` with the same global-fuel/cancellation semantics). It ships this round **without** a machine-checked refinement proof — the P2-style obligation that the arena refines `spec_step` is a stated, deliberately deferred open item (design §5), leaving the arena in the same validated-not-proved status as the production interpreter twin.
- **v0.4-draft** (2026-07-13) — Added the v0.4 effects boundary admitted by `docs/design/v0.4-effects.md`: **`Invoke`, a fourth `SpecStep` constructor** in the verified ghost model (mirrored on `SpecOutcome`/`StepResult`/`Outcome`), with the exact shape `Invoke(Seq<char>, Seq<SpecValue>, Seq<SpecWord>)` = (name, stack snapshot, continuation after the consumed `Call`). **Every `Call(name)` now yields `Invoke(name, stack, cont)`** instead of `Fault(UnknownWord)`: the pure core suspends at the call site with an immutable stack snapshot, holds no in-core dictionary, and never threads host state — so `Error::UnknownWord` remains in the enum but is no longer reachable from the `Call` arm (grant/deny is a host-side `HostFault`). `Invoke` is **terminal-within-a-run** (a base case of `spec_run` like `Halt`/`Fault`), leaving `spec_run`'s `decreases fuel` byte-untouched; resumption is a **fresh** `spec_run`/`run` seeded with `SpecState { stack: result_stack, cont }`. Added the unverified **host seam** `mtl-core::host` (`crates/mtl-core/src/host.rs`, in the TCB): `HostResult { Resume(Vec<Value>) | HostFault(HostCode) }` (host state stays host-local — no `host_state` threaded back), `HostCode { InputClosed, OutputCapExceeded, BudgetExhausted, ToolError, Timeout, NotGranted }`, the `Host` service trait, `CapabilitySig` (name/consumes/produces/faults declaration data), `RunResult { Done | Faulted | Cancelled | HostFaulted }`, and the impure `drive` loop (step the core over `exec_step` → on `Invoke` call `host.service` → `Resume` resumes the core in place from `cont`, `HostFault` terminates). Fuel stays a **pure in-core step counter**, threaded as a **single cumulative budget across all resumptions** (the total in-core steps over the whole driven run are bounded by `fuel`, so an endless capability loop is `Cancelled` rather than hanging — design §6 Option B / §8.5); host cost is metered separately host-side, never folded into fuel. §8 rewritten to the implemented semantics (superseding the older `CoreStep`/`host_state`-threading sketch), with an explicit capability-name grammar paragraph (§8.4): capability names are bare `Call` NAME tokens `[a-z][a-z0-9]*` (`crates/mtl-syntax/src/parse.rs`), so the design doc's illustrative `read-line`/`read-state`/`done?` do **not** lex (`-`→`Sub`, `?`→`If`) — real names must be lexer-safe (`readline`, `emit`, `done`, `step`, `tokenize`). §4 outcome set updated to four outcomes; G3 gate (§15) marked implemented. Verified core re-verifies at **76 verified, 0 errors** (`Invoke` adds no `Value`/`Word` constructor, so deep-view termination and both `Clone` stubs are untouched); `cargo test --workspace` green. **11 new tests**: `crates/mtl-core/tests/interpreter.rs` gains `call_yields_invoke_with_snapshot` and `call_invoke_carries_continuation`; new `crates/mtl-core/tests/invoke_host.rs` adds 9 drive-loop tests (bound-name yields `Invoke`, resume-continues, `HostFault` surfaces with no partial effect, multiple `Invoke`s reseed, fault precedence unchanged). Also **P5 discharged: Turing completeness is now a theorem** (Layer B, §6, §7.5). Admit-free Verus proof in `crates/mtl-core/src/p5_universality.rs` (118 verified, 0 errors) that MTL's `spec_step` semantics faithfully simulate an arbitrary two-counter Minsky machine with unbounded `nat` counters, via the unary-quotation counter encoding (§6.2: value `n` ↔ quotation of `n` markers; increment = `Cons`; decrement-and-zero-test = `Uncons`), compiled into a single self-applying `: !` dispatch-loop with a bounded-`Int` program counter (§6.4). Six theorems: `p5_stutter_step`, `p5_simulation` (bounded-stutter, i.e. multi-step, simulation); `p5_halt_forward`, `p5_halt_forward_monotone`, `p5_diverge`, `p5_halt_reverse` (two-way, fuel-quantified halting correspondence, including reflection / no-spurious-halt). Honest scope: the theorem is spec-level (counters live in unbounded `Seq` length; the `i64` bound limits only integer values and the finite program counter, not the simulation); the executable `run` stays fuel-/memory-bounded and its termination is intentionally not claimed. **No new primitives** — uses `? : ! uncons` + `Cons` from the frozen core; the proof re-verifies `mtl_core.rs` alongside (still 76/0, byte-for-byte unchanged). Executably corroborated by `crates/mtl-core/tests/p5_minsky.rs` (four Minsky machines through the real interpreter, plus a boundary-at-a-time differential layer and proptest-generated coverage cross-checked against a reference Minsky evaluator). Conjecture framing removed from §6 and the north-star line; §6.1–§6.5, §7.3 (P5 row + status), §7.5 Layer B, §13 roadmap, and gate G4 updated accordingly. **External P5 review pass** (`docs/reviews/2026-07-13-p5-review-gpt55.md`): renamed the one-step lemma `p5_lockstep → p5_stutter_step` (a bounded stutter `K ≈ 6·pc + loop-entry + handler`, linear in the PC, not literal lock-step); pinned `uncons`'s exact branch-dependent stack effect (§14.5); named the out-of-range-PC model (finite-code Minsky machine with implicit halt outside the code domain, §6.4); replaced unchecked `usize as i64` casts in the executable compiler with checked `pc_int`/`validate_prog`; converted the executable `disp` cascade to an iterative reverse fold; and documented the halting-correspondence decomposition (§6.5) showing no-spurious-halt is genuinely proved via `p5_diverge`.
- **v0.3-draft** (2026-07-12) — Added the two v0.3 sequence primitives admitted by `docs/design/v0.3-sequences.md` (merged PR #13) and implemented in the verified ghost model (`SpecPrim::{Fold, Xor}` + `spec_step_prim` arms), the exec twin, and the cargo interpreter `crate::interp`: **`fold` (`(`)** native **LEFT** fold `( [seq] init [C] -- r )` — total and terminating on a finite list (the sequence spine strictly shrinks each step, the same well-founded measure `primrec`/`times` use; it recurses by re-emitting itself and does **not** desugar into `linrec`), affine in `seq` and multiplicative in `[C]`, with a non-value sequence head faulting `TypeMismatch` (as `uncons`) and no Overflow arm (Overflow arises only inside `C`); and **`xor` (`$`)** bitwise XOR `( a b -- a^b )` on the i64 two's-complement representation — **total**, arity → type only, with **no Overflow or DivByZero arm** because the XOR of two in-range i64 values is always in i64 range (contrast `+`/`*`). Step rules added to §4.1, primitive table and multiplicity notes to §5, fault-precedence worked examples to §4.4 (each new arm checks arity → types; `fold` has no semantic-fault arm, `xor` is total). Golden (max/min/reverse/contains/count via `fold`, `single_number` via `xor`), boundary (empty-sequence `fold` returns `init`, i64 XOR edges incl. `MIN^MAX`, `x^x==0`, `x^0==x`, `MIN^-1`), fault-precedence, and differential-proptest-oracle coverage added in `crates/mtl-core/tests/interpreter.rs`. Smoke theorems for the new arms (`smoke_fold_base`, `smoke_fold_step`, `smoke_xor`) and the `i64_bitxor` spec helper stated in `mtl_core.rs` for the Verus CI job. Note (deviation from the design doc's §10.2 framing): `fold` is documented and proven as **terminating like `primrec`/`times`** (spine-decreases measure), not as "partial, no termination claim" — the small-step semantics are byte-identical to the design sketch; only the termination framing is strengthened.
- **v0.2-draft** (2026-07-11) — Added the four v0.2 recursion primitives admitted by `docs/design/v0.2-recursion-primitives.md` (merged PR #8) and implemented in the verified ghost model (`SpecPrim::{PrimRec, Times, LinRec, Uncons}` + `spec_step_prim` arms), the exec twin, and the cargo interpreter `crate::interp`: **`primrec` (`&`)** bounded primitive recursion `( n [I] [C] -- r )`, **`times` (`.`)** bounded iteration `( n [Q] -- … )` — both total and terminating (count strictly decreases; checked i64, no Overflow arm since `k-1` is provably in range); **`linrec` (`\|`)** linear recursion `( [P] [T] [R1] [R2] -- … )` that **desugars into `if`** (inherits its verified branch semantics, adds no control operator; partial, fuel-bounded); and **`uncons` (`>`)** quotation deconstructor `( [w …] -- w [ … ] 1 ) | ( [] -- 0 )` — structural, affine, and the TC-proof enabler (§6.2). Step rules added to §4.1, primitive table and multiplicity notes to §5, fault-precedence worked examples to §4.4 (each new arm checks arity → types → semantics). Golden (incl. factorial-via-primrec, gcd-via-linrec, fib-via-times, sum_to, power), boundary (i64 edges, non-positive/`i64::MIN` counts, empty-quotation uncons), fault-precedence, and differential-proptest-oracle coverage added in `crates/mtl-core/tests/interpreter.rs`. Uncons open decision (non-value head) resolved to `TypeMismatch` per the design's faithful reading. Smoke theorems for the new arms (`smoke_primrec_*`, `smoke_times_*`, `smoke_linrec_desugar`, `smoke_uncons_*`) stated in `mtl_core.rs` for the Verus CI job.
- **v0.1.1** (2026-07-11) — Revision in response to the adversarial review (`docs/reviews/2026-07-11-adversarial-review.md`). TC "theorem" withdrawn to a conjecture with a quotation-encoded repair route and a v0.2 `uncons` candidate (§6). Deterministic lexer with unsigned integer literals (Option A) and test vectors; `-` is always `Sub` (§2.3). Normative fault precedence arity → types → semantics with worked examples (§4.4). `Call → UnknownWord` documented as v0.1 core behavior; two-machine `Invoke`/host-runner split specified for v0.2; "unconditional theorems" reworded (§8). `: !` reframed as a self-application kernel with a recursive-normal-form definition; "proper tail calls for free" made conditional on a loop normal form (P6) (§4.2, §6.3). §14 frozen as v0.2+ exploratory future work with claim-by-claim softening (Perceus "resembles"; `dip` = non-access interval; `over` = duplicate; P8 → P8a/b/c; P9 split into static / guarded / host-conformance / normal-exit; linear = at-most-once + exactly-once-on-normal-halt). Definitions `#f[...]` deferred out of v0.1 (§9). Benchmark redesigned with corpus splits, versioned task sets (T_v0 / T_v0.2), a baseline panel, warm/cold protocol, total-token accounting, and the headline metric *correct solutions per million inference tokens* (§10–§11). Layered proof hierarchy A–F (§7.5) and Go/No-Go gates (§15) added. String literals / `Str` clarified as not part of the v0.1 core (parser rejects; §2–§3, §7.1). Verification-status caveat added and the Verus pin corrected to the full build id `0.2026.07.05.49b8806` (the date-shaped `0.2026.07.05` named no downloadable asset → HTTP 403; fixed in CI #5), satisfying the review's "pin a precise commit, not a date-shaped version"; GREEN-phase proofs remain contract-stated and differential-proptest-evidenced pending the interpreter (§7.3, G7).
- **v0.1** — Initial draft.
