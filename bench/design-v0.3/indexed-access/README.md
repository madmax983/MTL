# MTL v0.3 — Indexed / random access into sequences (options analysis)

- Status: **design stage — options analysis for the maintainer to decide.** This
  touches the verified core's **value model**, so it is *not* a unilateral pick. Three
  options are analysed with real proof-impact and real token measurements; one is
  recommended, with the tradeoffs laid out so the maintainer can overrule.
- Branch: `v03-design`. Frozen `T_v0`/`T_v0.2` solutions, `bench/BASELINE.md`,
  `bench/BASELINE-TIER2.md`, `bench/design-v0.2/`, and `crates/` are **untouched**.
  All work lives under `bench/design-v0.3/indexed-access/`.
- Tokenizers: tiktoken `o200k_base` + `cl100k_base` at 0.8.0 (the pinned bench set).
- Companion files: **`proof-impact.md`** (the crux — P1–P4 cost per option) and
  **`solutions.md`** (candidate programs, hand-traces, token tables).

## 1. The problem, quantified

Two tier-2 tasks are walled (`bench/corpus/{two_sum,binary_search}/WALL.md`): MTL's
only sequence is a **cons-list** (a `Quote`) whose only deconstructor is sequential
`uncons` (`>`). There is no positional index / random access.

| task | idiomatic Py (tok) | MTL v0.2 | status |
|---|---:|---|---|
| two_sum | 48 | — | walled: returning **indices** needs positional info |
| binary_search | 83 | — | walled: needs **O(1) random access** to `xs[mid]` |

Both are **excluded** from the 1.91× tier-2 aggregate (no MTL denominator). The
question: what, if anything, should the core gain to address indexed access — and is
it worth the cost to the value model and the proofs?

The value model is the constraint that makes this hard:
`Value ::= Int(i64) | Quote(Program)` — two variants, in both the ghost spec
(`spec_step` in `crates/mtl-core/src/mtl_core.rs`, normative) and the exec twin.
A cons-list cannot index in O(1); attaching a position to an element has nowhere to live.

## 2. Candidate set

| option | mechanism | access | value-model change | glyphs (provisional) |
|---|---|---|---|---|
| (a) | `nth`/`len` primitives walking the cons-list | **O(n)** | **none** | `$`=nth, `#`=len |
| (b) | new `Vec(Seq<Value>)` value + `get`/`set`/`len` + `{ }` literals | **O(1)** | **new variant** | `{ }` delim, `$`=get, `#`=len, `\`=set |
| (c) | do nothing; document the niche | — | none | — |

## 3. Small-step semantics (spec_step extension sketches)

Written to match `spec_step_prim` in `crates/mtl-core/src/mtl_core.rs`: dispatch on `p`,
arity check first (`Fault(Underflow)`), then operand types (`Fault(TypeMismatch)`), then
semantic body (spec §4.4). `n = stk.len() as int`; top at `stk[n-1]`.

### 3.a.1 `len` (option a) — `( [xs] -- n )`

```rust
SpecPrim::Len => {
    if n < 1 { SpecStep::Fault(Error::Underflow) }
    else {
        match stk[n - 1] {
            SpecValue::Quote(q) => SpecStep::Next(SpecState {
                stack: stk.subrange(0, n - 1).push(SpecValue::Int(q.len() as int)),
                cont: rest,
            }),
            _ => SpecStep::Fault(Error::TypeMismatch),
        }
    }
}
```

### 3.a.2 `nth` (option a, flagged, uncons-shaped) — `( [xs] i -- x 1 ) | ( oob -- 0 )`

Extract element `i` **as a value** the way `uncons` extracts a head; on an
out-of-range index push flag `0` (data, not a fault — no new `Error` variant). A
non-value head (bare `Prim`/`Call`) faults `TypeMismatch`, exactly as `uncons`.

```rust
SpecPrim::Nth => {
    if n < 2 { SpecStep::Fault(Error::Underflow) }
    else {
        match (stk[n - 2], stk[n - 1]) {
            (SpecValue::Quote(q), SpecValue::Int(i)) => {
                let base = stk.subrange(0, n - 2);
                if i < 0 || i >= q.len() {
                    SpecStep::Next(SpecState { stack: base.push(SpecValue::Int(0int)), cont: rest })
                } else {
                    match q[i] {
                        SpecWord::PushInt(v)   => SpecStep::Next(SpecState {
                            stack: base.push(SpecValue::Int(v)).push(SpecValue::Int(1int)), cont: rest }),
                        SpecWord::PushQuote(s) => SpecStep::Next(SpecState {
                            stack: base.push(SpecValue::Quote(s)).push(SpecValue::Int(1int)), cont: rest }),
                        _ => SpecStep::Fault(Error::TypeMismatch),
                    }
                }
            }
            _ => SpecStep::Fault(Error::TypeMismatch),
        }
    }
}
```

### 3.b `get`/`len`/`set` (option b) — requires `SpecValue::Vec(Seq<SpecValue>)`

```rust
// get ( {v} i -- x ) : O(1). Out-of-range faults (or use a flagged variant).
SpecPrim::Get => {
    if n < 2 { SpecStep::Fault(Error::Underflow) }
    else {
        match (stk[n - 2], stk[n - 1]) {
            (SpecValue::Vec(v), SpecValue::Int(i)) => {
                if i < 0 || i >= v.len() { SpecStep::Fault(Error::IndexOutOfBounds) }  // NEW Error variant
                else { SpecStep::Next(SpecState {
                    stack: stk.subrange(0, n - 2).push(v[i]), cont: rest }) }
            }
            _ => SpecStep::Fault(Error::TypeMismatch),
        }
    }
}
// set ( {v} i x -- {v'} ) : functional update, fresh vector.
// len ( {v} -- n ) : analogous to 3.a.1 on the Vec variant.
```

The `SpecValue::Vec` variant, the `IndexOutOfBounds` `Error` variant, and the array
literal `{ … }` (new delimiter glyphs) are the model change — see `proof-impact.md`.

## 4. Proof-impact assessment on P1–P4 (summary — full detail in `proof-impact.md`)

Measured proof surface: `SpecValue` is matched/constructed at ~55 sites; the
deep-view bridge (`view_value`/`view_words`/`view_stack`/`deep_view`) carries the
**hard-won lexicographic termination measure** that closed the nested-quotation hole;
`Error` has 5 variants (the P1 precedence surface).

- **(a)** adds two `SpecPrim` variants only. The typed arms already carry
  `_ => TypeMismatch`, so totality (P1/P3) is preserved by construction. The
  **deep-view measure is untouched** (a list is still a `Quote`). No new `Error`
  variant (flagged `nth`). No new literal syntax for P4 (two self-delimiting glyph
  rows). **No new P2/P4 holes** — this is exactly the machinery that already shipped for
  `uncons`. **Uncons-sized; proof-safe even with P2 open.**
- **(b)** adds a `SpecValue::Vec` variant → ~55 audit sites; **reopens the deep-view
  termination measure** (a third mutual-recursion spine, value→Seq<value>→value); adds
  an `IndexOutOfBounds` `Error` variant (P1 precedence grows); adds a recursive Vec
  well-formedness invariant (I3′); **materially enlarges P4** (new `{ }` delimiters +
  a new literal case in the round-trip proof); and **breaks the affine story** (`get`
  copies, `set` persists — not the clean "consumed once" reading `uncons` enjoys).
  Building this on top of the still-`admit()`-stubbed P2 maximizes rework. **Weeks;
  should land after P2.**
- **(c)** zero proof cost.

## 5. Glyph assignment (provisional, measured probes)

From the free ASCII set `# $ ( ) \` `` ` `` `{ }`. Merge probes (tiktoken 0.8.0):

| glyph | role | o200k | cl100k | note |
|---|---|---:|---:|---|
| `$` | nth/get | 1 | 1 | `]$` = 2 (o200k) / **1 (cl100k)** |
| `#` | len | 1 | 1 | `]#` = 2/2 |
| `\` | set (opt b) | 1 | 1 | `]\` = 1/1 |
| `{` `}` | array literal (opt b) | 1 | 1 | `{}` merges to **1** token |

Final glyph selection is a separate worker's job (spec §11, corpus-level). Flag: option
(b) spends **two** of the seven free glyphs on literal delimiters (`{ }`) *before* any
operator glyph — a real glyph-budget cost that (a) avoids entirely.

## 6. Measured results and projected aggregate

Design-stage, hand-traced (full traces + program strings in `solutions.md`). o200k:

| task | idiomatic Py | (a) nth/len | (b) get | (a) ratio | (b) ratio |
|---|---:|---:|---:|---:|---:|
| two_sum | 48 | ~34 (est.) | ~34 (est.) | 1.41× | 1.41× |
| binary_search | 83 | 37 | 39 | 2.24× | 2.13× |

**Aggregate effect (option a, representative):**

| | py (idiomatic) | mtl |
|---|---:|---:|
| current 10 tier-2 tasks | 327 | 171 |
| + two_sum + binary_search | +131 | +71 |
| **12-task aggregate** | **458** | **242 → 1.89×** |

**Admitting indexed access is compression-neutral (1.91× → 1.89×).** Coverage rises
10/13 → 12/13; the headline metric does not move. two_sum drags (1.41×), binary_search
helps (2.24×), net flat. And note (§ `solutions.md` §2): **(a) is 2 tokens cheaper than
(b) on binary_search yet delivers O(n·log n), not O(log n)** — the token cost of the two
options is a wash; (b)'s only advantage is the complexity property.

## 7. Writability

- **(a) nth/len** — modest, real win. `#` removes re-deriving `len` via a fold; `$`
  removes hand-threading an index counter through an `uncons` walk. But nested
  point-free loops with 3–4 carried values (two_sum's double loop, binary_search's
  `lo/hi/mid`) still demand deep blind stack routing (`@ ^ ~ '` gymnastics) — the exact
  failure mode the adversarial review flags ("blind spatial routing," "LLM scratchpad
  deficit," §3 of the briefing). `nth`/`len` shrink the tax; they do not remove it.
  Golfing a correct two_sum by hand at design stage was itself error-prone — a
  writability data point against these tasks regardless of the primitive.
- **(b) get/set/{ }** — same routing tax (the carried-state depth is identical), plus
  the model must learn a *second* aggregate syntax (`{ }` vs `[ ]`) and when to use
  which. Net writability ≈ (a); the win is runtime complexity, not authoring ease.
- **(c)** — no new surface for the model to get wrong.

## 8. Recommended admission set

**Primary recommendation: (c)-framed strategy with a minimal (a) hedge — phased.**

1. **Adopt option (c)'s strategic framing now, in docs.** Indexed random access is
   where imperative languages structurally win; MTL's cons-list cannot do O(1) access
   without a model change, and the token evidence says unblocking these two tasks is
   **compression-neutral (1.91× → 1.89×)**. By the standing anti-tarpit rule (a
   primitive is admitted iff it pays for itself in corpus tokens), indexed access does
   **not** clear the bar the way `primrec` did (2.11×→3.10× single-handed). So do **not**
   chase these tasks for the headline metric. This is the honest tier-2 story: MTL's
   edge is recursion/control (3.72× on `T_v0`), not indexed iteration (1.91×).

2. **If benchmark *coverage* is wanted, ship option (a) — and only (a) — now.** Two
   primitives (`nth` flagged/uncons-shaped, `len`), admitted on a **coverage/writability
   rationale exactly as `uncons` was admitted on a non-token rationale**, not on corpus
   tokens. It is proof-safe (uncons-sized, no model change, no deep-view reopening, no
   new `Error`/P2/P4 holes) and unblocks two_sum cleanly. **Label binary_search honestly
   as a linear/bisection scan over a cons-list (O(n·log n)), not a true binary search** —
   the benchmark passes it by I/O, not by logarithmic probing.

3. **Hold option (b) (the `Vec` model) firmly until P2 is discharged**, and admit it
   later *only* on measured justification from a future array-heavy corpus. It is the
   only option that delivers true O(1)/O(log n) access, but it reopens the file's
   hardest closed proof (deep-view termination), grows `Error`, I3, and P4, breaks the
   affine story, and — built on today's `admit()`-stubbed P2 — maximizes rework. The
   token evidence does **not** currently justify that cost (a wash with (a)).

**Maintainer's call to make:** whether benchmark *coverage* of these two array-shaped
tasks is worth two primitives that are compression-neutral. If yes → (a). If the
benchmark is content to document them as out-of-niche walls → (c) alone. **(b) is
premature under any reading until P2 lands.**

## 9. Implementation plan (if (a) is chosen)

Mirrors the v0.2 doc §10, at ~half the size (two primitives, no model change):
1. **spec_step** (`crates/mtl-core/src/mtl_core.rs`): add `SpecPrim::{Len, Nth}` and the
   §3.a arms. No `Overflow`/`Error` growth; `nth` OOB is a flag.
2. **Proofs:** smoke theorems `smoke_len`, `smoke_nth_hit`, `smoke_nth_oob` (near-copies
   of `smoke_uncons_*`). No change to the deep-view measure or the P2/P4 obligations.
3. **Parser** (`crates/mtl-syntax/src/ast.rs`): `GLYPHS` rows `('#', Len), ('$', Nth)`;
   confirm `print.rs::needs_separator` (self-delimiting); add lexer test vectors.
4. **Interpreter:** exec twins in `crates/mtl-core/src/interp.rs`; two `conv` arms in
   `bench/validate/src/lib.rs`; proptest-oracle arms in `tests/interpreter.rs`.
5. **Corpus/validate:** promote `two_sum` (and `binary_search` as linear/bisection scan)
   from WALL to `mtl-v0.3/solution.mtl` with I/O vectors; update `tasks.json`
   (`expressible:true`); regenerate the tier-2 report. Only then are §6 numbers
   interpreter-validated rather than hand-traced.

## Appendix — reproduce

See `solutions.md` §Appendix. All paths here are off the `bench/validate` discovery
path and out of `tasks.json`, so `cargo test` and the frozen baselines are unaffected.
