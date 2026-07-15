# MTL v0.8 — CANDIDATES: measured admission analysis for the ~1.7× out-of-sample ceiling

Turns the sink taxonomy (`scratchpad/diagnose-taxonomy.md`) into concrete,
**measured** candidates. Every MTL rewrite here is tokenized with the same o200k
(`bench/tokcount`) encoder as the harness, and the aperture rewrites are
**simulator-verified against the datagen oracle** (`verify_aperture.py`), not
merely hand-traced — the design record's repeated lesson is "no hand-traced
number survives contact with the interpreter."

Reproduce:
```
python3 bench/design-v0.8/candidates/measure_aperture.py   # token deltas + juggle counts
python3 bench/design-v0.8/candidates/verify_aperture.py    # oracle-correctness of the rewrites
python3 bench/design-v0.8/candidates/project_ratio.py      # projected TRAIN/DEV new-families ratio
```

The addressable sink is stack-juggling (≈59% of the shortfall). Of that, the
taxonomy's five scan/state tasks split into **two structurally different kinds**,
and the distinction decides everything:

- **windowed** (sliding fixed-width look at consecutive input elements):
  `count_local_maxima` (3-window), `max_adjacent_diff` (2-window). The juggle is
  the uncons-triple + re-thread that rebuilds the window every step.
- **stateful/accumulator** (history-dependent running state): `rle_flatten`
  (run value+count state machine), `min_running_balance` (twin running
  accumulators), `dedup_adjacent` (element-vs-last-emitted). The juggle threads a
  history that no fixed window supplies.

Only the **windowed** kind is addressable by an aperture combinator. This is the
load-bearing finding.

---

## Candidate 1 — Stack-aperture (windowed-fold) combinator `w`

### spec_step semantics

`[xs] acc0 k [C] w`  (k a **literal** per issue #50's gate; C a literal quote):

```
w: pop [C]; pop k (literal Int); pop acc0; pop [xs] (quote of ints).
   acc := acc0
   for i in 0 ..= len(xs) - k:            // sliding width-k window, step 1
       acc := run C on stack [acc, xs[i], xs[i+1], ..., xs[i+k-1]]   // C: [acc]+[e0..e_{k-1}] -> [acc]
   push acc
   // lists shorter than k contribute no window: result is acc0.
```

This is `fold`'s rule generalized from a 1-element step to a k-element sliding
window. Two forms measured:

- **parameterized** (`w`, one glyph, literal-k operand — #50-compliant): `0 3[C]w`
- **fixed-width** (`#`=window-3, `` ` ``=window-2, two glyphs, no operand): `0[C]#`

### Measured token delta on TRAIN + DEV (o200k / cl100k), simulator-verified

| task | split | baseline | aperture (param) | aperture (fixed) | juggle glyphs |
|---|---|--:|--:|--:|--:|
| `count_local_maxima` | **TRAIN** | 43 / 38 | `0 3[^<@@<*+]w` = **10 / 9** | `0[^<@@<*+]#` = **8 / 7** | **20 → 3** |
| `max_adjacent_diff` | **DEV** | 42 / 39 | `0 2[-:0<[0~-][]?^^<[~_][_]?]w` = **19 / 19** | `` 0[…]` `` = **17 / 17** | **21 → 7** |

Both rewrites **PASS the oracle grid** (`verify_aperture.py`: empty, singleton,
runs, negatives, alternations — every `scan_lists()` vector). The deltas are
**−33/−35 tok** (local_maxima) and **−23/−25 tok** (max_adj_diff).

### The #41 hypothesis — DISPROVEN for windowed scans (does the aperture remove or relocate juggle?)

Juggle glyph counts (`: ~ ^ _ @ '`) fall **20 → 3** and **21 → 7**. The aperture
**removes** the windowing juggle; it does not relocate it. The residual 3 / 7
glyphs are genuine *intra-window comparison* (`b>a && b>c`, `max(acc,|a−b|)`),
which Python also pays for.

This is the crucial divergence from #41 ("juggling-bound not access-bound"): #41
concerned **random access into a buried 4-value carried state** (two_sum /
binary_search), where deep pick-chains dominate regardless of the access
primitive — so no mechanism helped. **Sequential windowed scans are structurally
different**: the combinator *supplies* the consecutive elements directly,
eliminating the uncons-triple + re-thread. The #41 wall does **not** transfer to
sequential windowed scans. It **does** still bind the *stateful* tasks (below).

### What the aperture does NOT address (honest scope)

- `rle_flatten` (45 tok, 20 juggle): history-dependent run state, **not** a fixed
  window. No aperture supplies `(value, count)` run state — this hits a genuine
  #41-style wall. **Unaddressed.**
- `min_running_balance` (22 tok, 11 juggle): twin running accumulators; a
  width-1 window is just `fold`, which already exists. **Unaddressed.**
- `dedup_adjacent` (20 tok, 6 juggle): element-vs-last-emitted; a 2-window form
  exists but needs a first-element seed + list acc that re-introduce the fold
  machinery — measured marginal (~−4 tok) and **not oracle-verified here**, so
  **not claimed**.
- `alt_sum` (9 tok, **0 juggle**): already optimal (a plain linrec). Untouched.

So the aperture cleanly addresses **2 of the 5** scan tasks — the windowed
sub-shape — which is ~50 of the 89.6-token stack-juggling excess (**~35% of the
total 141-token shortfall**), and **0%** of the state-machine / accumulator /
missing-idiom sinks.

### Checker typeability (proven-fragment impact)

Per `docs/design/v0.6-checker.md`, `w` types **exactly like `fold`**: with a
**literal k** the window arity is statically known, and `C : [acc:Int] ++
[Int;k] -> [acc:Int]` is checked for height/type stability. Over an **opaque
input list** (runtime length) the verdict is **Guarded** (runtime-length
obligation) — the *same class* as the existing fold-based / linrec-based scan
programs. **No proven-fragment shrink**: Guarded → Guarded. The cost is
implementation, not regression: a new Fold-analogous effect rule threaded through
the `AbsVal`/`Kind` lattice, plus `spec_step_prim`/`exec_prim` arms, the 7 mirror
surfaces (#32), manifest count 23→24, P2 refinement lemmas, and a smoke theorem
— the full primitive-mirror fan-out #41/#50 costed. (`: !` dup-apply is *not*
involved, so no checkable-fragment loss there.)

### Projected broad-distribution ratio (new families = scan + bitdigit, o200k)

| variant | TRAIN | DEV |
|---|--:|--:|
| baseline (current language) | 380/223 = **1.70×** | 375/218 = **1.72×** |
| param-k aperture | 380/190 = **2.00×** | 375/195 = **1.92×** |
| fixed-glyph aperture | 380/188 = **2.02×** | 375/193 = **1.94×** |

Scan-family only: TRAIN 1.62×→**2.65–2.76×**, DEV 1.42×→**1.86–1.92×**.

**Held-out validation holds**: the aperture was designed/verified on
`local_maxima` (**TRAIN**) and transfers to `max_adjacent_diff` (**DEV**, held
out) — a real, if thin (n=1 held-out windowed shape), out-of-sample signal. It is
a genuine combinator (any sliding-window computation: moving stats, adjacent
comparisons, run detection), more general than the `$`-xor overfit, but **narrower
than `fold`/`linrec`** (which fire on the whole list family). It still lands at
**~2×, not 3×** — the Abrash gate is not recovered.

### Quickref cold-cost delta + break-even

New-glyph quickref line: **~39 tok** (terse) to **~97 tok** (full-quickref style
with a worked example). Against the 4051-tok full cold preamble (break-even ~154
tasks, ≈26 net tok/task), a ~40–97-tok line pushes break-even up **~1.5–3.7
tasks**. Small in isolation — **but the amortization problem is decisive**: on a
broad distribution the windowed shapes are **2 of 1145 (0.17%)**, saving ~60 tok
total, so the ~40–97 cold tokens **plus** the full implementation/proof fan-out
are **not amortized** unless the target distribution is scan/window-heavy. The
parameterized `w` (1 glyph, 1 quickref line) is cold-preferable to fixed-width
(2 glyphs, 2 lines) despite costing +2 tok per use.

---

## Candidate 2 — Idiom / doc fix (zero language cost)

Documenting the `abs` (`:0<[0~-][]?`), accumulator, and scan idioms in the
quickref. **Measured effect on authored compression: exactly 0.** The corpus
solutions already *use* the optimal idioms (they are oracle-golfed), so no
solution's token count changes. Documenting an idiom does not shorten any
program — it changes **writability** (a live model may reach the optimum faster,
fewer failed attempts), **not compression**. And it **grows the cold quickref**
(+lines), so its measurable cold-economics effect is *slightly negative*.

Verdict: **worth doing for writability / generation-reliability, contributes 0 to
the compression thesis.** Cannot be validated as a compression win without a model
in the loop (this round measures static compression, where it is provably 0).

---

## Candidate 3 — A scalar builtin (`popcount`)

`popcount` currently costs **22 tok** (`:0<[0~-][]?0~[:0=][_][:2/~2%@+~][]|`) vs
Python's `bin(abs(n)).count("1")` = **15 tok** — the one task where MTL **loses**
(0.68×). A `popcount` glyph (incl. abs) = **1 tok** → 15× on that shape.

But: it fires on **exactly one shape** (base-2). `digit_sum_base` for bases
3–16 and `digit_product_base` are **unaffected** (the loop is base-parameterized;
popcount is only the base-2 special case). Broad-distribution impact: replaces
22→1 on **1 of 1145 shapes**, moving the bitdigit family by one shape —
**negligible on the aggregate** — while spending a scarce glyph, a ~19-tok
quickref line, a checker arm, and the mirror/proof fan-out.

Verdict: **textbook benchmark-fit / anti-niche.** It wins its one motivating
shape and nothing else — the same pattern the sealed post-mortem named for `$`.
**Reject.**

---

## Recommended admission set

**Primary recommendation: ADMIT NOTHING — restate the thesis.**

On the broad, generator-based out-of-sample distribution the round is measured
against, the compression ceiling is **X = ~1.7×** (new families TRAIN 1.70× /
DEV 1.72×; full-distribution ~3.2× is an arithmetic-domination artifact, not
generalization). The **≥3× Abrash gate does not generalize** and was a
co-evolution/baseline artifact.

The single addressable lever — the windowed aperture `w` — is real and verified
(cuts windowed-scan juggle 60–80%, oracle-correct, checker-typeable, held-out DEV
transfer), but:
1. it lifts only **2 of 1145** shapes, to **~2×, still short of 3×**;
2. it addresses **~35%** of the juggling sink and **0%** of the state-machine /
   accumulator / missing-idiom sinks (`rle_flatten`, `min_running_balance`,
   `count_set_bits`, `digit_product` are structurally untouched);
3. on a broad distribution its **cold-quickref + full primitive-mirror/proof cost
   is not amortized** by 0.17%-frequency windowed shapes;
4. doc-fixes contribute **0** compression; `popcount` is **benchmark-fit** with
   **~0** broad impact.

So the honest out-of-sample ceiling is **~1.7×** and should be stated plainly as
the v0.8 result. MTL's defensible edge stays its fold/linrec niche (2–4× on
loop/recursion shapes); the broad-mix ceiling is ~1.7×.

**Conditional recommendation (only if the maintainer re-scopes the target to a
window/scan-heavy niche):** admit the **parameterized windowed-fold `w`**
(literal-k, #50-gated) — the one candidate with a verified, generalizing,
checker-typeable (Guarded, fold-class, no proven-fragment shrink) token win. It
projects the scan family to **TRAIN ~2.65× / DEV ~1.9×** and the new-families
aggregate to **~2.0× out-of-sample**. Prefer param-k (1 glyph) over fixed-width
(2 glyphs) for cold economics. **Even then the result is ~2×, not 3×.**

### Quantified ceiling

| scope | current | with aperture `w` (if admitted) |
|---|--:|--:|
| new families (scan+bitdigit), out-of-sample | **1.70× / 1.72×** | **2.00× / 1.92×** |
| scan family only | 1.62× / 1.42× | 2.65× / 1.86× |
| broad full-distribution (arithmetic-padded, not a generalization number) | ~3.2× | ~3.2× |

**X = ~1.7× admitting nothing; ~2.0× admitting the aperture into a scan-heavy
niche. The 3× gate is not recoverable on a blind broad mix of this composition.**
