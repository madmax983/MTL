# `empty?` / `len` — v0.3 candidate measurement (design stage)

- Status: **design stage.** Not implemented in parser/interpreter. Programs are **hand-traced against the semantics sketch (§3)**, not interpreter-validated. Every token count is **real** (`bench/tokcount`, both encodings); every correctness claim is a hand-trace in `solutions.md`, explicitly marked `✓ hand-traced`.
- Branch: `v03-design`. Frozen `T_v0`/`T_v0.2` corpus, `bench/BASELINE*.md`, `crates/`, `bench/design-v0.2/` all untouched. This work lives only under `bench/design-v0.3/len-empty/`.
- Tokenizers: `tiktoken` `o200k_base` + `cl100k_base`, 0.8.0 (pinned bench set). **Both encodings agree on every cell in this doc.**
- Provisional glyphs: `empty?` → `\`, `len` → `` ` `` (final assignment is a separate worker; both are free ASCII punctuation per briefing §1.4).

## 1. The problem, quantified

Five of the ten solved tier-2 tasks (the "solved-but-ugly" accumulators) each embed the **non-destructive null test** `[>[;0][[]1]?]` as their linrec predicate `P`. This is an inline `empty?` that also **rebuilds the list** so the recursive branch can keep traversing. It is re-derived verbatim in all five solutions.

`len` appears once: the entire `length_list` solution `[>0=][0][][~_1+]|` *is* a hand-rolled length.

Goal: measure whether folding these into primitives pays for itself corpus-level (spec §5/§11), and separate the `empty?` saving from the `len` saving.

## 2. Candidate set

| candidate | shape it captures | stack effect | glyph (provisional) |
|---|---|---|---|
| `empty?` | is-a-sequence-empty predicate, **non-destructive** | `( [xs] -- [xs] b )`, `b=1` iff empty | `\` |
| `len` | length of a cons-list sequence, consuming | `( [xs] -- n )` | `` ` `` |

**Why `empty?` is non-consuming.** MTL's `linrec` runs `P` with no save/restore, so `P` must leave the list intact for the base (`T`) and recursive (`R1`) branches. The frozen idiom did this by unconsing then **rebuilding** (`>[;0]…`); a non-consuming `empty?` does it by never deconstructing. A *consuming* `empty?` (`( [xs] -- b )`) cannot be a drop-in: it would leave `R1` with no list to `>_` and `T` with nothing to drop. So `empty?` = the frozen `P` with the rebuild elided. It leaves the **identical** stack shape `list flag` (flag 1 empty / 0 non-empty), which is why the rewrite is a pure local substitution (`solutions.md`).

**Why `len` is consuming.** The only corpus consumer is `length_list`, which discards the list. No corpus site needs a retained-list `( [xs] -- [xs] n )`, so consuming is chosen; it matches the frozen behaviour.

## 3. Small-step semantics (spec_step extension sketches)

Written to match `spec_step_prim` in `crates/mtl-core/src/mtl_core.rs`: dispatch on `p`, arity check first (`Underflow`), then operand type (`TypeMismatch`) — the normative fault precedence (spec §4.4). New `SpecPrim` variants `Empty, Len`. `n = stk.len() as int`, top at `stk[n-1]`.

### 3.1 `empty?` — `( [xs] -- [xs] b )`  (non-consuming peek)

Inspect the top quotation's length **without deconstructing it**; leave it in place and push `1` if empty else `0`. Non-`Quote` top → `TypeMismatch`. No semantic-check arm (no `DivByZero`/`Overflow` reachable).

```rust
SpecPrim::Empty => {
    if n < 1 { SpecStep::Fault(Error::Underflow) }
    else {
        match stk[n - 1] {
            SpecValue::Quote(q) => {
                // NON-consuming: quote stays at stk[n-1]; push the flag above it.
                let flag = if q.len() == 0 { 1int } else { 0int };
                SpecStep::Next(SpecState {
                    stack: stk.push(SpecValue::Int(flag)),
                    cont: rest,
                })
            }
            _ => SpecStep::Fault(Error::TypeMismatch),
        }
    }
}
```

**Desugaring equivalence (adds no expressive power).** `empty?` ≡ the existing word sequence
`Uncons ; PushQuote([Cons, PushInt(0)]) ; PushQuote([PushQuote([]), PushInt(1)]) ; If`
— i.e. the frozen idiom `>[;0][[]1]?` run inline. On `…[xs]`: non-empty → `…[xs_rebuilt] 0`; empty → `…[] 1`, matching the native rule cell-for-cell. So admitting `empty?` is **pure token compression of an existing macro**, not a new capability — the cleanest possible admission argument. The native rule is preferred only because it skips the rebuild allocation.

### 3.2 `len` — `( [xs] -- n )`  (consuming)

Pop the top quotation, push its top-level word count. Nested quotes count as one element each (a cons-list length), consistent with `length_list`'s one-uncons-per-word count.

```rust
SpecPrim::Len => {
    if n < 1 { SpecStep::Fault(Error::Underflow) }
    else {
        match stk[n - 1] {
            SpecValue::Quote(q) => {
                let base = stk.subrange(0, n - 1);
                SpecStep::Next(SpecState {
                    stack: base.push(SpecValue::Int(q.len() as int)),
                    cont: rest,
                })
            }
            _ => SpecStep::Fault(Error::TypeMismatch),
        }
    }
}
```

`len` also desugars (≡ the `length_list` linrec `[>0=][0][][~_1+]|`), so likewise adds no expressive power — pure compression.

## 4. Multiplicity / affine story

| primitive | quote arg | reading |
|---|---|---|
| `empty?` | **inspected, not consumed, not split** | Pure observer. Reads the length tag and leaves the quote in place. |
| `len` | **consumed, not split** | Linear consumption (pops the quote) but no split. |

The subtle point flagged in the briefing: a non-consuming peek "uses the sequence twice" (test it, keep it). **Is that sound?** Yes — and it sits *outside* the affine discipline that governs `uncons`. `uncons` (`>`) is affine because it **splits** a quotation into head+tail; duplicating a split would double-spend structure. `empty?` never splits: it reads the immutable length of a value and pushes an `Int`. Quotations are **copyable values** (the language already has `:` dup, and `Quote(Program)` is an immutable `Seq`), so observing one without moving it is a read, not a resource duplication. There is no refcount/linearity obligation to discharge. This is the same reason `:`,`^` (dup/over) are sound on quotes.

Consequence for the substructural story: `empty?` is a **non-affine, non-splitting observer** — a genuinely new category next to the affine `uncons` and the replicated-along-spine combinators (`primrec`/`times`/`linrec`). `len` is affine-consuming (like a fold that discards its structure). Neither introduces a duplication hazard.

## 5. Measured results

All counts real (`bench/tokcount`), file-based to avoid shell-quoting of `\ ' `` ` ``. **o200k_base == cl100k_base on every row.**

### 5.1 `empty?` — the five accumulator tasks

| task | before (tok) | after `[\]` (tok) | Δ |
|---|---:|---:|---:|
| max_list | 22 | 16 | 6 |
| min_list | 23 | 17 | 6 |
| reverse_list | 19 | 12 | 7 |
| contains | 26 | 20 | 6 |
| count_occurrences | 23 | 17 | 6 |
| **sum** | **113** | **82** | **31** |

Before-sum 113 matches BASELINE-TIER2 / briefing §5 exactly (harness faithful).

> **Honest correction to the briefing's ~45 estimate.** The briefing put the `empty?` tax at 9 tok × 5 = **45**, counting `[>[;0][[]1]?]` in isolation (measured 9/9 tok standalone → 2/2 for `[\]`, Δ7). **In context the real saving is 31 tok** (Δ6 per task, Δ7 for reverse_list), because BPE merges the idiom's characters with their neighbours in the full program, so the predicate never actually costs a clean 9 tokens where it sits. 31, not 45, is the number to bank.

### 5.2 `len` — one task

| task | before (tok) | after `` ` `` (tok) | Δ |
|---|---:|---:|---:|
| length_list | 13 | 1 | 12 |

`len` collapses the whole `length_list` program to one glyph. But it appears **exactly once** in the corpus, so its corpus-level payoff is a single 12-tok task — much narrower than `empty?`'s five-site 31.

### 5.3 Separated savings (the requested split)

- **`empty?` saving: 31 tok** (o200k = cl100k), across 5 tasks.
- **`len` saving: 12 tok** (o200k = cl100k), across 1 task.
- Combined (both admitted, distinct tasks): **43 tok**.

## 6. Fold interaction — the decisive caveat

Another worker owns the `fold`/carrying-combinator candidate (briefing §4.3 item 3, the ~73-tok tax). **The five tasks that give `empty?` its 31-tok saving, and the one task that gives `len` its 12, are exactly the tasks a carrying `fold` would replace wholesale.** A fold internalises the traversal *and* the empty-check *and* the length count. So:

- If `fold` is admitted, its combinator eats the null-test and the length linrec internally. The 31 + 12 = **43 tok are captured by `fold`, not additive to it.** `empty?`/`len`'s marginal value on this corpus **drops to ~0**.
- `empty?`/`len` retain independent value only where you must branch on emptiness / take a length **without** doing a full fold — and **the current corpus has no such site.**

Therefore the standalone case for `empty?`/`len` is weak *conditional on `fold` landing*. Their genuine strengths are (a) they are trivially cheap (1-glyph, pure-compression, no new expressive power — §3 desugarings), (b) `empty?` removes the rebuild-then-re-uncons footgun's *surface area* (though not the `>_` obligation itself — §2), and (c) writability: `\` is a named, pattern-matchable predicate an LLM emits far more reliably than the 9-glyph `>[;0][[]1]?` juggling. If `fold` is deferred, `empty?` is the single best cheap win here (31 tok, all five ugly tasks) and should be admitted; `len` is marginal (one task) and can wait.

## 7. Recommended admission

- **`empty?` (`\`): admit if `fold` is deferred; otherwise fold-subsumed.** 31-tok corpus saving, pure compression, real writability gain, sound non-affine observer. The strongest of the two.
- **`len` (`` ` ``): defer.** One-task (12-tok) payoff, fully fold-subsumed. Cheap to add later if a length-without-traversal task appears.
- Neither adds expressive power (both desugar into existing prims, §3), so neither changes any proof obligation beyond a new total `spec_step` arm + exec twin + GLYPHS row.

## Appendix — reproduce

Program sources are in `bench/design-v0.3/len-empty/tok/` (`*_before.mtl` / `*_after.mtl`); each was counted with:

```
cd /home/user/MTL/bench && python3 tokcount/tokcount.py <file>
```

`\`, the dip glyph `'`, and the backtick `` ` `` are shell-hostile, so programs were written to files (`printf '%s' … > f.mtl`) and counted by path. None of these paths is on the `bench/validate` discovery path or in `tasks.json`, so `cargo test` and the frozen BASELINEs are unaffected.
