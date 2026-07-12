# MTL v0.3 — `pick` / `roll` (parameterized deep-stack access)

- Status: **design stage, RECOMMEND REJECT/DEFER for v0.3.** Candidate primitives
  below are **not implemented**; semantics are **hand-traced against the sketch**,
  never interpreter-validated. Every token count is real (`bench/tokcount`,
  o200k_base + cl100k_base, tiktoken 0.8.0); every rewrite is hand-traced and
  marked `✓ hand-traced`.
- Branch: `v03-design`. Frozen `T_v0`/`T_v0.2` solutions, `bench/BASELINE.md`,
  `bench/BASELINE-TIER2.md`, `crates/`, `bench/design-v0.2/` all untouched. All
  work here lives under `bench/design-v0.3/pick-roll/`.
- Verdict headline (don't skip): on the entire solved corpus pick/roll are a
  **strict token LOSS** and a **writability wash-to-negative**. The genuine win
  (non-destructive access at depth ≥ 3) is **real but unexercised** by any task.
  **Defer to the warm/cold agent trial**, exactly as v0.2 did — and note the
  structural gap they'd address (two_sum / binary_search) is better served by an
  indexed *sequence* primitive than by stack pick/roll.

---

## 1. The problem, quantified

The adversarial review (`docs/reviews/2026-07-11-adversarial-review.md`, Gemini §1
"Stack-Juggling Tax") argues point-free routing chains like `@ ^ ~ @` — dragging a
deeply-buried value to the top — can cost more than name-omission saves, and
**prioritizes `pick`/`roll` over `map`/`fold`** because "LLMs struggle profoundly
with blind spatial routing." The tier-2 probe (1.91× aggregate, well under 3×)
surfaced the same tax concretely: the 5 "solved-but-ugly" fold tasks burn
**113/113 tokens**, dominated by hand-rolled routing (`^^`, `@~`, `^=@~+…`).

Forth semantics generalize MTL's fixed routers to arbitrary depth:

| Forth | copies/moves | MTL equivalent today |
|---|---|---|
| `0 pick` | copy top | `:` dup |
| `1 pick` | copy 2nd | `^` over |
| `1 roll` | move 2nd to top | `~` swap |
| `2 roll` | move 3rd to top | `@` rot |

So `pick`/`roll` are the *depth-parameterized* generalization of `: ^ ~ @`. The
design question: does the depth come from the **stack** (`n pick`, an Int already
on the stack) or is it baked into the glyph? Given MTL's unsigned left-to-right
literals and self-delimiting glyphs, **`n pick` (depth on the stack) is the only
natural fit** — a per-depth glyph family would consume the entire remaining free
punctuation budget. This doc measures whether that generalization pays.

## 2. Candidate set

| candidate | shape it captures | stack effect | glyph (provisional) |
|---|---|---|---|
| `pick` | copy the depth-`n` item to top (non-destructive random read) | `( … n -- … x_n )` | `(` |
| `roll` | move the depth-`n` item to top (destructive random read) | `( … n -- … )` | `)` |

`0 pick` = dup, `1 pick` = over; `0 roll` = nop, `1 roll` = swap, `2 roll` = rot.
Depth is a runtime **Int operand consumed from the top of the stack**. Glyphs `(` `)`
are provisional (final assignment is a separate worker); they are among the only
free ASCII punctuation left (`# $ ( ) \ ` `` ` `` { }`).

## 3. Small-step semantics (spec_step extension sketches)

Written to match `spec_step_prim` in `crates/mtl-core/src/mtl_core.rs`: dispatch on
`p`, arity check → `Fault(Underflow)`, then operand type → `Fault(TypeMismatch)`,
then the semantic body. Stack is `Seq<SpecValue>`, `n = stk.len() as int`, top at
`stk[n-1]`. New `SpecPrim` variants: `Pick, Roll`.

> **Proof-surface flag — DATA-DEPENDENT ARITY (read this).** Every existing
> primitive has a *static, constant* arity checked *before any operand is
> inspected* (spec §4.4 step 1: arity → type → semantic). `pick`/`roll` **break
> that invariant**: their true arity is `d + 2` where `d` is the *runtime value*
> of the depth operand, so you must read (and type-check) the operand *before* you
> can know whether enough items are present. The precedence therefore becomes:
> (1) `Underflow` if the stack is empty (can't even read the depth); (2)
> `TypeMismatch` if the depth is not `Int`; (3) **depth-out-of-range** — a NEW
> fault mode. Two options: a fresh `Error::DepthRange` variant (touches P1
> precedence enumeration, every `match Error` exhaustiveness site, and P3
> progress) or **fold it into `Underflow`** (the stack really is too shallow for
> the requested depth; keeps the `Error` enum frozen → cheaper for the P1/P3
> proofs). **Recommendation: fold into `Underflow`.** The sketches below do that,
> and flag the range test inline. This is the single biggest proof cost of the
> pair — a genuine mark against admission.

### 3.1 `pick` — `( … n -- … x_n )`

Pop the Int depth `d`. Copy the item now at depth `d` (0 = top of the remaining
stack) onto the top; the source stays in place. Non-affine (the value is
replicated). `0 pick` = `:` dup, `1 pick` = `^` over.

```rust
SpecPrim::Pick => {
    if n < 1 { SpecStep::Fault(Error::Underflow) }        // need the depth itself
    else {
        match stk[n - 1] {
            SpecValue::Int(d) => {
                let base = stk.subrange(0, n - 1);         // stack with depth popped
                let blen = n - 1;                          // items available below it
                // DATA-DEPENDENT range check (see proof-surface flag): folds to Underflow
                if d < 0 || d >= blen { SpecStep::Fault(Error::Underflow) }
                else {
                    let x = base[blen - 1 - d];            // 0 = top of base
                    SpecStep::Next(SpecState { stack: base.push(x), cont: rest })
                }
            }
            _ => SpecStep::Fault(Error::TypeMismatch),
        }
    }
}
```

### 3.2 `roll` — `( … n -- … )`

Pop the Int depth `d`. Remove the item at depth `d` from its position and push it
on top, sliding everything above it down one — a cyclic rotation of the top
`d + 1` items. Affine (permutation, multiset-preserving). `0 roll` = nop,
`1 roll` = `~` swap, `2 roll` = `@` rot.

```rust
SpecPrim::Roll => {
    if n < 1 { SpecStep::Fault(Error::Underflow) }
    else {
        match stk[n - 1] {
            SpecValue::Int(d) => {
                let base = stk.subrange(0, n - 1);
                let blen = n - 1;
                if d < 0 || d >= blen { SpecStep::Fault(Error::Underflow) }
                else {
                    let idx = blen - 1 - d;                // position of the target
                    let x = base[idx];
                    let without = base.subrange(0, idx) + base.subrange(idx + 1, blen);
                    SpecStep::Next(SpecState { stack: without.push(x), cont: rest })
                }
            }
            _ => SpecStep::Fault(Error::TypeMismatch),
        }
    }
}
```

Fault precedence within each arm: `n < 1` (Underflow) → non-Int depth
(TypeMismatch) → range (Underflow). The range check is the only novelty vs. the
existing dispatchers.

## 4. Multiplicity / affine story

| primitive | depth arg | data reading | class |
|---|---|---|---|
| `pick` | Int, consumed (linear) | copies `base[d]` to top — the value now occurs **twice**, multiset **grows by one** | **non-affine / multiplicative**, same class as `:` dup and `^` over |
| `roll` | Int, consumed (linear) | moves `base[d]` to top, nothing duplicated or dropped — **multiset preserved exactly** (cyclic rotation of the top `d+1` items) | **affine / permutation**, same class as `~` swap and `@` rot |

Neither adds a new substructural wrinkle over the fixed routers they generalize:
`pick` is `dup`/`over` at parameterized depth (replication), `roll` is
`swap`/`rot` at parameterized depth (permutation). The depth operand itself is
always consumed once (linear).

## 5. Glyph assignment (measured)

Provisional `pick → (`, `roll → )`. Final assignment is a separate worker, but the
measurement here is glyph-agnostic in the worst way: **the cost is dominated by
the depth digit and the loss of BPE merges, not by the glyph choice.** A single
router glyph is 1 token; any `<digit><glyph>` form is 2 (see §6). No punctuation
choice recovers that, and a preceding literal forces a **space** (`0 1)` not `01)`,
since `01` maximal-munches to `Int(1)`), adding yet another token. The critical
merge fact cuts the wrong way: **`^^` is a single token; `1(1(` is four.**

## 6. Measured results — the token loss

Real `bench/tokcount`, both encodings identical on every cell.

### 6.1 Micro (one router at a time)

| op | today | tok | pick/roll form | tok | Δ |
|---|---|--:|---|--:|--:|
| dup | `:` | 1 | `0(` | 2 | +1 |
| over | `^` | 1 | `1(` | 2 | +1 |
| swap | `~` | 1 | `1)` | 2 | +1 |
| rot | `@` | 1 | `2)` | 2 | +1 |
| over·over | `^^` | **1** | `1(1(` | **4** | **+3** |
| rev3 | `~@` | 2 | `1)2)` | 4 | +2 |

Every fixed router is 1 token; its parameterized twin is 2. The `^^` row is the
killer: BPE merges the repeated glyph to one token, but `1(1(` cannot merge.

### 6.2 Full "solved-but-ugly" solutions (faithful, behavior-preserving rewrites)

| task | original | tok (o200k=cl100k) | pick/roll rewrite | tok | Δ | % |
|---|---|--:|---|--:|--:|--:|
| contains | `0~@…[^=@~+0~<~]…` | **26** | `0 1)2)…[1(=2)1)+0 1)<1)]…` | **33** | +7 | +27% |
| count_occurrences | `0~@…[^=@~+~]…` | **23** | `0 1)2)…[1(=2)1)+1)]…` | **29** | +6 | +26% |
| max_list | `…[^^<[~_][_]?]…` | **22** | `…[1(1(<[1)_][_]?]…` | **25** | +3 | +14% |

(Rewrites hand-traced in §7 and `solutions.md`; they compute the identical result,
so the delta is a pure like-for-like tax, not a worse algorithm.) Extrapolated
across all 5 fold tasks the routing tax **grows** by ~25%, dragging the tier-2
aggregate the wrong way.

### 6.3 Where a win actually lives — depth ≥ 3 (NOT in the corpus)

`pick`/`roll` take a flat **2 tokens at any depth**. Pure MTL has **no** depth-≥2
non-destructive access primitive, so it must nest dips, whose cost escalates:

| access | pick form | tok | pure-MTL (dip nest) | tok |
|---|---|--:|---|--:|
| copy depth-1 | `1(` | 2 | `[^]'` | 2 |
| copy depth-2 | `2(` | 2 | `[[^]']'` | 5 |
| copy depth-3 | `3(` | 2 | `[[[^]']']'` | 6 |

At depth ≥ 2 pick overtakes, and the pure-MTL form is also acutely error-prone.
**This is the genuine token+writability win — and no `T_v0`/`T_v0.2` task reaches
that depth.** Every solved task keeps carried values at depth ≤ 3, reachable by a
single named router. The corpus was, in fact, *hand-designed* to keep them shallow
(TIER2_NOTES incident log #2: the state layout `found x list` was chosen precisely
so `x` sits at depth 2, reachable by one `^`).

## 7. Hand-traces (design stage — NOT interpreter-validated)

Stack bottom→top. Full traces in `solutions.md`; the equivalences:

- **rev3** `~@` ≡ `1)2)` on `a b c`: `1)`(swap)→`a c b`, `2)`(rot)→`c b a`. ✓ hand-traced.
- **max_list `^^`** ≡ `1(1(` on `acc head`: `1(`→`acc head acc`, `1(`(copies `head`)→`acc head acc head`. ✓ hand-traced.
- **contains setup** `0~@` ≡ `0 1)2)`: `0`→`list x 0`, `1)`(swap)→`list 0 x`, `2)`(rot)→`0 x list`. ✓ hand-traced.
- **contains BODY** `^=@~+0~<~` ≡ `1(=2)1)+0 1)<1)` on `found x head`: ends `or x`, identical to the original. ✓ hand-traced.

Every rewrite is behavior-identical to the frozen solution — confirming §6.2 is a
clean tax, not an algorithm change.

## 8. Writability (E[tokens × attempts]) — THE HEADLINE

The review's whole case for pick/roll is writability, so measure it honestly, both
sides, from the corpus.

**FOR (Gemini's bet).** `pick`/`roll` give the model *random access by counting*
instead of *blind spatial routing*. Today, writing `contains` well required the
author to **design a state layout** (`found x list`) so the carried scalar stays at
a fixed shallow depth — TIER2_NOTES calls this out as the hard part, and the
incident log shows the `@ ^ ~` chains are exactly where the hand-written solutions
faulted (`Underflow`, `TypeMismatch`). With random access the model can grab the
value from *wherever* it is (`2(`) and skip the layout puzzle entirely. For deep
access (§6.3) there is no competition: pick is one named op vs a fragile dip nest.

**AGAINST (why it fails on THIS corpus).**
1. **It moves the counting burden, it doesn't remove it.** `over`/`swap`/`rot`/`dup`
   are *named* Forth operations with strong LLM priors — the model recognizes "over"
   as a concept. `1 pick` demands the model *compute a stack index*, and Gemini §2
   ("scratchpad deficit") is precisely that LLMs track hidden stack depth poorly.
   A blind-routing error becomes a depth-miscount error — the task's own worry.
2. **The index is position-dynamic.** After every stack op the correct depth
   shifts, so the model must **re-count at each use**. A fixed named op (`^` always
   means "2nd item") is *more* stable to emit than `k(` whose `k` drifts.
3. **The token cost is real and negative:** +26% on the ugly folds (§6.2), +1 on
   every shallow router (§6.1), never a merge. Admitting pick/roll would push the
   tier-2 aggregate *down*, opposite of v0.3's goal.
4. **The corpus never reaches the depth where FOR applies.** All 10 solved tasks
   live at depth ≤ 3. The writability win is entirely in the depth-≥3 regime that
   no task exercises — so at stage 1 the claim is **unmeasurable**, exactly as v0.2
   concluded (§8 of `docs/design/v0.2-recursion-primitives.md`).

**Net writability verdict: wash-to-negative on the current corpus.** At shallow
depth the named routers are at least as writable and cheaper; pick/roll trade a
spatial-routing error mode for a depth-miscount error mode that LLMs are *also* bad
at, while costing tokens. The FOR case is genuine but lives in a task class the
benchmark doesn't yet contain.

## 9. Recommended admission set

**REJECT / DEFER `pick` and `roll` for v0.3.** They are a strict token loss on the
solved corpus (§6) and a writability wash-to-negative (§8). This matches the v0.2
disposition (deferred pending the warm/cold agent trial) and the anti-tarpit
admission rule (spec §5, §10.2): a primitive is admitted iff it pays for itself in
corpus-level token accounting — pick/roll do the opposite here.

**If they are revisited, revisit them for the RIGHT task class.** Their real value
is non-destructive deep access (§6.3), which the corpus lacks. But the concrete
unsolved tasks that motivate "random access" — `two_sum`, `binary_search` — need
indexing into **data (a sequence)**, not into **stack positions**. A `nth`/`index`
sequence primitive addresses that structural wall directly; stack `pick`/`roll`
does not. So even the deep-access argument points at a *different* primitive.
**Defer pick/roll to the agent trial (the only instrument that can adjudicate a
writability-only claim); do not admit on intuition.**

## 10. Implementation plan (only if admitted later)

1. **spec_step** (`crates/mtl-core/src/mtl_core.rs`): add `SpecPrim::{Pick, Roll}`
   and the §3 arms. **Resolve the data-dependent-arity / depth-range fault first**
   (recommend fold into `Underflow`; if a new `Error::DepthRange` is chosen, update
   P1 precedence enumeration + every `match Error` site + P3 progress).
2. **Proofs:** the arms are total functions (P1 style). Add smoke theorems à la
   `smoke_dup_apply`: `0 pick` ≡ dup, `1 pick` ≡ over, `1 roll` ≡ swap, `2 roll` ≡
   rot (ties the generalization to the verified fixed routers); a range-fault
   theorem (`d >= blen` ⇒ `Underflow`). Extend `interp.rs` exec twins, re-run P2.
3. **Parser** (`crates/mtl-syntax/src/ast.rs`): `Prim::{Pick, Roll}` + `GLYPHS` rows
   for the final glyphs; `print.rs` `needs_separator` (self-delimiting); lexer
   vectors incl. the literal-adjacency space rule (`0 1)` vs the `01` munch trap).
4. **Interpreter/validate:** exec variants + `conv`/`conv_program` in
   `bench/validate/src/lib.rs`.
5. **Corpus/validate:** only if a depth-≥3 task is added to justify them; regenerate
   BASELINE via the appropriate report script.

## Appendix — reproduce

All counts via `cd /home/user/MTL/bench && python3 tokcount/tokcount.py '<src>'`
(pipe through stdin for programs containing the dip glyph `'`), tiktoken 0.8.0,
`o200k_base` + `cl100k_base`. The exact strings are in `solutions.md`. None of these
paths is on the `bench/validate` discovery path or in `tasks.json`, so `cargo test`,
`BASELINE.md`, and `BASELINE-TIER2.md` are unaffected.
