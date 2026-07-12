# MTL v0.3 candidate — `fold` (`#`): a left fold over the cons-list

- Status: **design stage**, mirrors the format of
  `docs/design/v0.2-recursion-primitives.md`. `fold` is **not yet implemented**;
  programs are **hand-traced against the semantics sketch** (§3), not
  interpreter-validated. Every token count is a real `bench/tokcount` output.
- Branch: `v03-design`. Frozen `T_v0`/`T_v0.2` solutions, `bench/BASELINE*.md`,
  `crates/`, and the frozen corpus are untouched. All work here lives under
  `bench/design-v0.3/fold/`.
- Tokenizers: `tiktoken` `o200k_base` and `cl100k_base` at 0.8.0 (the pinned
  bench set). Both encodings agree except where a cell says otherwise.
- Provisional glyph `#` (free ASCII per the scout briefing). Final glyph
  assignment is a separate worker's job; measured here with `#` in place.

---

## 1. The problem, quantified

The tier-2 probe (`bench/BASELINE-TIER2.md`) sits at **1.91× / 1.89×** — far
below the frozen `T_v0` 3.72× — because list-accumulator code pays a large
hand-rolled stack-juggling tax. Five "solved-but-ugly" tasks cost **113 / 113
tokens** re-deriving, by hand, an accumulator that the language does not give
them:

| task | v0.2 source | o200k | cl100k |
|---|---|---:|---:|
| max_list | `>_[>[;0][[]1]?][_][>_[^^<[~_][_]?]'][]\|` | 22 | 22 |
| min_list | `>_[>[;0][[]1]?][_][>_[^^<[_][~_]?]'][]\|` | 23 | 23 |
| reverse_list | `[]~[>[;0][[]1]?][_][>_[~;]'][]\|` | 19 | 19 |
| contains | `0~@[>[;0][[]1]?][__][>_[^=@~+0~<~]'][]\|` | 26 | 26 |
| count_occurrences | `0~@[>[;0][[]1]?][__][>_[^=@~+~]'][]\|` | 23 | 23 |
| **total** | | **113** | **113** |

**Why `linrec` can't express these cleanly.** The three CLEAN folds
(`sum_list [>0=][0][][+]|`, `product_list`, `length_list`) work because
`linrec`'s natural shape is a **right fold**: it runs `combine(head, fold(tail))`
with the head still on the stack from the null-test's `>`, and a *constant* init
(`0`/`1`). That shape has **no left-threaded accumulator slot** and **no place to
carry an extra scalar**. The five ugly tasks need exactly those:

- **max/min** seed the accumulator from the first element (there is no writable
  `-∞`/neutral element), so they need a value threaded left-to-right;
- **reverse** must prepend into a growing accumulator list;
- **contains/count** thread BOTH a running accumulator AND the search scalar `x`.

Lacking those slots, the v0.2 solutions **emulate a left fold by hand** over
`linrec`: a non-destructive null-test `[>[;0][[]1]?]` that *rebuilds* the list
(9 tokens, ×5 ≈ 45), forcing the recursive branch to re-`uncons` with `>_`; a
`dip` (`'`) to reach under the accumulator; and `^`/`@`/`~` routing to keep `x`
at a fixed reachable depth each step (`TIER2_NOTES.md` incident log). That
scaffolding — not the arithmetic — is the token cost, and it is the dominant LLM
failure mode (blind spatial routing; adversarial review §"Stack Juggling Tax").

**Goal:** a primitive that supplies the left-threaded accumulator natively so the
body expresses only the per-element step, admitted strictly on corpus-level token
accounting (spec §5/§11).

---

## 2. Candidate set

| candidate | shape it captures | stack effect | glyph |
|---|---|---|---|
| `fold` | left fold / accumulate over a cons-list | `( seq init [C] -- result )`, `C:( acc w -- acc' )` | `#` (provisional) |
| `foldc` / `pickN`-carry | fold that threads an extra carried value to `C` | `( seq init carry [C] -- result carry )` | **deferred — does not earn its place**, see §7 |

`fold` is a **LEFT** fold. Justification (§3, §7): (a) left fold is the natural
tail/iterative form for a stack machine — constant stack, no unwind, matching
`times` and tail-`linrec`; (b) it makes `reverse` a one-liner (`[][~;]#`) that a
right fold cannot; (c) operand order `( seq init [C] )` reads left-to-right
("fold THIS from INIT by C") and puts `seq` — usually already on the stack as
input — at the bottom, so clean folds are just `init [C] #`.

---

## 3. Small-step semantics (spec_step extension sketch)

`fold` is a **native** step rule that recurses by re-emitting itself, exactly like
`primrec`/`times` (only `linrec` desugars into `If`). It does **not** desugar
into `linrec`, because `linrec` is a right fold and cannot host a left-threaded
accumulator without the very hand-juggling `fold` exists to remove.

Written to match `spec_step_prim` in `crates/mtl-core/src/mtl_core.rs` (arity →
type → semantic ordering; `n = stk.len()`; top at `stk[n-1]`; the existing
`value_to_word` helper at line 114 turns a `SpecValue` into its push-word). New
variant: `SpecPrim::Fold`.

### 3.1 `fold` — `( seq init [C] -- result )`

Prose: pop the combine quote `[C]`, the seed `init` (any value), and `seq` (must
be a `Quote`). On the empty sequence the result is `init`. On `seq = [w | tl]`,
compute `acc' = C(init, w)` — i.e. run `C` with the stack `… init w` on top — then
recurse `fold(tl, acc', C)`. Left fold, left to right. The head deconstruction is
the `uncons` reading: a head that is not itself a value (a bare `Prim`/`Call`)
faults `TypeMismatch`. `fold` is **partial** like `!`/`linrec` (a non-terminating
`C` would loop), but on any finite list with a straight-line `C` it is total and
terminates in `len(seq)` applications; the spine strictly shrinks each step.

```rust
// fold ( [w0 w1 ...] init [C] -- r ): LEFT fold. init seeds the accumulator;
// C ( acc w -- acc' ) is applied once per element, left to right. The sequence
// is deconstructed head-first — affine, like uncons (consumed once, never
// duplicated); C is replicated along the spine — multiplicative, like C in
// primrec. Native recursion via re-emitting Fold (does NOT desugar into linrec).
SpecPrim::Fold => {
    if n < 3 { SpecStep::Fault(Error::Underflow) }        // arity first
    else {
        match (stk[n - 3], stk[n - 2], stk[n - 1]) {
            // seq must be a Quote; init is ANY value; combine must be a Quote.
            (SpecValue::Quote(qs), init, SpecValue::Quote(qc)) => {
                let base = stk.subrange(0, n - 3);
                if qs.len() == 0 {
                    // empty list: the result is the seed accumulator.
                    SpecStep::Next(SpecState { stack: base.push(init), cont: rest })
                } else {
                    // head-first deconstruction (the uncons reading).
                    let tail = qs.subrange(1, qs.len() as int);
                    match qs[0] {
                        // qs[0] is already a push-word for the head value; splice
                        // it directly. Continuation, run on `base`:
                        //   [tail]  init  <push head>   C   [C] fold
                        // -> park [tail] at the bottom, put `init head` on top,
                        //    run C ( acc w -- acc' ), then recurse fold(tail,acc',C).
                        SpecWord::PushInt(_) | SpecWord::PushQuote(_) => {
                            let recur = seq![
                                SpecWord::PushQuote(tail),
                                value_to_word(init),
                                qs[0]
                            ] + qc + seq![
                                SpecWord::PushQuote(qc),
                                SpecWord::Prim(SpecPrim::Fold)
                            ];
                            SpecStep::Next(SpecState { stack: base, cont: recur + rest })
                        }
                        _ => SpecStep::Fault(Error::TypeMismatch),   // non-value head
                    }
                }
            }
            _ => SpecStep::Fault(Error::TypeMismatch),               // seq/C not quotes
        }
    }
}
```

**Operand-order justification.** `C` sees `( acc w -- acc' )` (acc below, current
element on top). The desugar parks `[tail]` at the bottom so it survives the
combine untouched, layers `init` then the head on top, runs `C` to get `acc'`
directly above `[tail]`, then re-pushes `[C]` and `Fold` — leaving exactly
`[tail] acc' [C]` = the next `( seq init [C] )`. No swap in the machine; the only
per-call routing the *author* writes is when they choose to seed init from the
list (max/min: `>_~`, 3 tokens) — everything else is `init [C] #`.

### 3.2 Fault precedence (normative, spec §4.4)

Preserved: arity (`n<3 → Underflow`) is checked before any operand is inspected;
then operand types (`seq` and `[C]` must be `Quote`; `init` is unconstrained) →
`TypeMismatch`; a non-value head → `TypeMismatch`. No semantic-fault arm:
`fold` performs no arithmetic, so no `Overflow`/`DivByZero` (those arise only
inside `C`, under `C`'s own rules).

### 3.3 Interaction with `uncons`

`fold`'s deconstruction IS the `uncons` semantics inlined: it splits `qs` into
head-word `qs[0]` and `tail = qs[1..]`, requires the head to be a value
(`PushInt`/`PushQuote`), and faults `TypeMismatch` on a non-value head — the same
rule as `SpecPrim::Uncons` (mtl_core.rs line 400). Equivalently, one `fold` step
on a non-empty list = `uncons`, drop-flag, run `C`, recurse — but `fold` fuses
the null-test and the re-uncons that the v0.2 `linrec` idiom had to write by hand
(and that caused the incident-log bug).

### 3.4 Mechanical desugar check (sum `0[+]#` on `[3 1 2]`)

Confirms the step rule, not just the value-level trace. State
`[3 1 2] 0 [+]`, `Fold`: `qs=[3 1 2]`, `init=0`, `qc=[+]`, `base=[]`,
`tail=[1 2]`, `qs[0]=PushInt(3)`.
`recur = PushQuote([1 2]), PushInt(0), PushInt(3), Add, PushQuote([+]), Fold`.
Executing on `[]`:
`[1 2]` → `[1 2] 0` → `[1 2] 0 3` → `Add` → `[1 2] 3` → `[1 2] 3 [+]` →
`Fold` = fold(`[1 2]`, 3, `[+]`). Recurse: → fold(`[2]`, 4, `[+]`) →
fold(`[]`, 6, `[+]`) → empty ⇒ push init ⇒ **`6`** = sum([3,1,2]). ✓

---

## 4. Multiplicity / affine story

| primitive | count/seq arg | value args | reading |
|---|---|---|---|
| `fold` — **seq** | `seq` **consumed once, deconstructed head-first** | — | genuinely **affine/linear**, exactly like `uncons`: the spine is split, never duplicated; each `w` extracted once |
| `fold` — **init** | — | `init` threaded linearly (each `C` consumes the old acc, produces the new — no duplication) | linear |
| `fold` — **[C]** | — | `[C]` **replicated along the spine**, applied once per element | multiplicative, like `[C]` in `primrec` and the quotes in `linrec` |

`fold` is the first primitive that is **affine in one argument (the sequence) and
multiplicative in another (the combine) at once** — it *removes* the hand-written
replication (`'`, `[>_…]` re-uncons, `[__]` base cleanup) the author currently
performs, while keeping the affine single-consumption of the list that makes
`uncons` easy to reason about.

---

## 5. Glyph note (provisional)

> **Superseded (glyph reconciliation).** `#` was the *working placeholder* used
> for every hand-trace and measurement in this file. The final glyph is **`(`**,
> per the corpus-level §11 BPE sweep in `bench/design-v0.3/glyphs/README.md` §5:
> `](` merges to a single token in both encodings whereas `]#` does not, so `(`
> is **4 corpus tokens cheaper** (56/54 vs 60/58 over the 8 fold solutions). The
> semantics and hand-traces below are glyph-independent; only the symbol changes
> `# → (`. See `docs/design/v0.3-sequences.md` §5 for the authoritative
> assignment and the `(`-based token counts (56/54).

Measured with the placeholder `#`. `#` is one of the free ASCII punctuation chars
(`# $ ( ) \ ` `` ` `` { }`) left after the 21 v0.2 primitives (scout briefing
§1.4). Per spec §11 the *final* assignment is a corpus-level BPE sweep owned by a
separate worker; the `fold` win here is **~14× the size of any plausible
±1-token glyph BPE divergence**, so the admission decision is glyph-insensitive.
Note: `#` sits adjacent to `]` in every clean fold (`[+]#`, `[*]#`, `[~;]#`) — the
`]#` bigram is the merge worth probing first, mirroring the `]&`/`]|` finding for
`primrec`/`linrec`.

---

## 6. Measured results

All counts are real `bench/tokcount` outputs (both encodings; script
`bench/design-v0.3/fold/measure.py`, cross-checked against the `tokcount.py` CLI
on max_before/contains_after/reverse_after/sum_after — exact match). Per-task
traces in `solutions.md`.

| task | before o200k | after o200k | before cl100k | after cl100k | Δ o200k | Δ cl100k |
|---|---:|---:|---:|---:|---:|---:|
| max_list | 22 | 12 | 22 | 11 | -10 | -11 |
| min_list | 23 | 13 | 23 | 12 | -10 | -11 |
| reverse_list | 19 | 5 | 19 | 5 | -14 | -14 |
| contains | 26 | 10 | 26 | 10 | -16 | -16 |
| count_occurrences | 23 | 7 | 23 | 7 | -16 | -16 |
| **5-ugly subtotal** | **113** | **47** | **113** | **45** | **-66** | **-68** |
| sum_list | 10 | 4 | 10 | 4 | -6 | -6 |
| length_list | 13 | 5 | 13 | 5 | -8 | -8 |
| product_list | 9 | 4 | 9 | 4 | -5 | -5 |
| **all-8 total** | **145** | **60** | **145** | **58** | **-85** | **-87** |

**Findings.**
- The 5-ugly tax collapses **113 → 47/45**, a **-66 / -68** token saving (the
  scout's "~73-token tax" estimate, confirmed to within the seeding/closure
  residue). Average ugly task **22.6 → 9.4** tokens.
- **Bonus:** fold ALSO beats `linrec` on the 3 *already-clean* folds
  (**32 → 13**, -19/-19): `sum_list` 10→4, `product` 9→4, `length` 13→5. The
  `[>0=]…|` null-test wrapper is pure overhead a native fold elides.
- **No task regresses.** Every one of the 8 deltas is negative under both
  encodings. There is no task where `fold` fails to pay for itself vs `linrec`.
- Effect on the tier-2 aggregate: these 8 tasks drop from 145 to 60/58 MTL
  tokens. Holding the Python numerators fixed, the list-accumulator portion of
  the corpus improves by ~2.4× on its own — the single biggest token lever among
  the v0.3 candidates that block *zero* tasks structurally.

---

## 7. `pickN` / carrying-fold variant — evaluated, **does not earn its place**

The brief asked whether a carrying fold (`foldc ( seq init carry [C] -- … )`, or a
`pickN` deep-access primitive) earns admission for contains/count, which thread an
extra scalar `x`.

**Finding: no.** contains/count are handled at **10 / 7 tokens** by closing `x`
into the combine quote with the *existing* `;` cons —
`[=+0~<];0~#` / `[=+];0~#` (§ solutions). Consing a runtime value into a quote to
form a closure needs **no new primitive and no scarce glyph**; a `foldc` would add
a fourth stack slot, a new native step rule, and one of the last free ASCII
glyphs, to reach roughly the same token count. On this corpus the cons-closure
idiom strictly dominates. A `pickN`/`roll` deep-access primitive remains the
adversarial review's *writability* bet (blind routing is an LLM failure mode), but
— exactly as in the v0.2 report — that is a claim only the warm/cold agent trial
can adjudicate; it shows **no stage-1 token win** here. **Defer both.**

---

## 8. Writability (E[tokens × attempts])

`fold` is a **strong** writability win, in the `primrec`/`times` tier:

- It replaces the corpus's worst blind-routing idiom. The author writes a named,
  pattern-matchable shape — "accumulate = seed + step", `init [step] #` — and the
  step's only obligation is a shallow **2-item** local effect `( acc w -- acc' )`.
  It **eliminates** the whole v0.2 failure cluster: the rebuild-then-re-uncons
  null test (the incident-log Underflow bug), the `dip` under the accumulator, and
  the `^`/`@`/`~` routing to keep `x` reachable. That routing is precisely the
  "stack-juggling tax" / "scratchpad deficit" the adversarial review names as the
  dominant LLM failure mode.
- Residual obligations, both shallow: (a) max/min seed init from the first element
  (`>_~`, a fixed 3-token prelude); (b) contains/count close `x` into the combine
  via `;` — a single, nameable idiom ("bake the carried value into the step") that
  is far easier to emit correctly than per-step rot/over/swap.
- `#` rides no strong learned prior (unlike `.`/`&`/`|`), but the shape `init[…]#`
  is regular and self-delimiting; the generatability question is deferred to the
  agent trial, as for every glyph.

---

## 9. Recommendation

**Admit `fold` (`#`, provisional) on token grounds.** It is the largest
token+writability lever among the v0.3 candidates that block no task structurally:
**-66/-68** on the five ugly accumulators, **-85/-87** across all eight list
folds, **no regressions**, and it removes the corpus's dominant LLM failure mode.

**Defer** the carrying `foldc`/`pickN` variant: the existing `;` cons-closure
already handles the carried-scalar tasks with no new primitive or glyph (§7); a
deep-access `pick`/`roll` stays a writability-only bet for the agent trial.

---

## Appendix — reproduce

```
python3 bench/design-v0.3/fold/measure.py
```
re-counts every before/after program against the pinned `o200k_base`/`cl100k_base`
encoders. This path is OFF the `bench/validate` discovery path (hardcoded to
`bench/corpus/<task>/mtl*/solution.mtl`) and absent from `tasks.json`, so
`cargo test` and `bench/BASELINE*.md` are unaffected. Spot-check any single
program directly, e.g. `cd bench && python3 tokcount/tokcount.py '0[+]#'`; for
sources containing the dip `'` use stdin:
`printf '%s' ">_[>[;0][[]1]?][_][>_[^^<[~_][_]?]'][]|" | python3 bench/tokcount/tokcount.py`.
```
