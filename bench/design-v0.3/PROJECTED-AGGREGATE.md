# MTL v0.3 — Projected tier-2 aggregate (authoritative, measured)

- **Status:** design stage. `fold`/`xor`/`nth`/`len` are not yet in the
  parser/interpreter; the MTL v0.3 programs are hand-traced against the semantics
  sketches in the sibling `fold/`, `bitwise/`, `indexed-access/` dirs. **Every token
  count in this file is a real `bench/tokcount` (raw tiktoken) run** — see the
  Appendix for raw outputs and the reproduce script. Nothing frozen, in `crates/`, or
  on the `bench/validate` discovery path is touched.
- **Tokenizers (pinned):** tiktoken `o200k_base` + `cl100k_base`, tiktoken 0.8.0.
- **Metric (identical to `report.py` / BASELINE-TIER2):** static program-source
  tokens; per-task ratio = tokens(python-idiomatic) / tokens(mtl); **aggregate =
  sum(python-idiomatic) / sum(mtl)** over the tasks in scope (token-SUM aggregation).
- **Glyphs used:** `fold = (`, `xor = $` (token-optimal — see `glyphs/README.md`).
  A `fold = #` variant is reported alongside as the generatability hedge.

Python-idiomatic and mtl-v0.2 denominators were re-tokcounted from the corpus sources
directly and **match BASELINE-TIER2.md exactly** (py 327/324 o200k/cl100k, mtl-v0.2
171/171 over the 10 solved tasks).

---

## Per-task table (all real tokcounts)

| task | py o200k | mtl-v0.2 o200k | mtl-v0.3 o200k | py cl100k | mtl-v0.2 cl100k | mtl-v0.3 cl100k | v0.3 program | primitive |
|---|---:|---:|---:|---:|---:|---:|---|---|
| sum_list | 25 | 10 | **4** | 25 | 10 | **4** | `0[+](` | fold |
| length_list | 26 | 13 | **5** | 26 | 13 | **5** | `0[_1+](` | fold |
| product_list | 25 | 9 | **3** | 25 | 9 | **3** | `1[*](` | fold |
| max_list | 35 | 22 | **11** | 34 | 22 | **10** | `>_~[^^<[~_][_]?](` | fold |
| min_list | 35 | 23 | **12** | 34 | 23 | **11** | `>_~[^^<[_][~_]?](` | fold |
| reverse_list | 27 | 19 | **4** | 27 | 19 | **4** | `[][~;](` | fold |
| palindrome_number | 54 | 18 | 18 | 53 | 18 | 18 | `0^[:1<][_=][:10%@10*+~10/][]|` | (unchanged) |
| climbing_stairs | 38 | 8 | 8 | 38 | 8 | 8 | `1 1@[~^+]._` | (unchanged) |
| contains | 27 | 26 | **10** | 27 | 26 | **10** | `[=+0~<];0~(` | fold |
| count_occurrences | 35 | 23 | **7** | 35 | 23 | **7** | `[=+];0~(` | fold |
| single_number | 25 | — | **9** | 25 | — | **9** | `[>0=][0][][$]|` | xor |
| two_sum | 48 | — | ~34 est | 48 | — | ~34 est | (nth/len, opt-a) | indexed(a) |
| binary_search | 83 | — | 37 | 83 | — | 37 | `^#1-0~@[^^~<0=][@@___][@:^+2/:\@@^~<[1+~][~1-]?][+]|` | indexed(a) |

`two_sum` under indexed-access (a) is the one non-measurable cell: a *correct*
brute-force point-free program is a design estimate of **~34 tokens (band 30–38)**
(the schematic core measures 22, but is not first-match-guarded / result-built). It is
carried as an estimate and its sensitivity is shown in scenario C. `binary_search` (a)
is a real 37/37 tokcount, re-measured here with `len=#`, `nth=\` so it does not collide
with `xor=$` (unchanged from the indexed worker's `$`/`#` version — free single-char
glyphs all cost ~1 token).

---

## Scenario A — v0.2 baseline (10 tasks), pipeline reproduction

| encoding | sum(py idiomatic) | sum(mtl-v0.2) | aggregate |
|---|---:|---:|---:|
| o200k_base | 327 | 171 | **1.91×** |
| cl100k_base | 324 | 171 | **1.89×** |

Reproduces BASELINE-TIER2.md (1.91×/1.89×) to the digit → the pipeline is faithful.

---

## Scenario B — **{fold + xor}, 11 tasks** — THE HEADLINE

10 solved tier-2 tasks (8 fold-rewritten + palindrome/climbing unchanged) + `single_number` via xor.

| encoding | sum(py idiomatic) | sum(mtl-v0.3) | **aggregate** | vs A |
|---|---:|---:|---:|---:|
| o200k_base | 352 | 91 | **3.87×** | +1.96× |
| cl100k_base | 349 | 89 | **3.92×** | +2.03× |

**{fold + xor} clears the tier-2 ≥3× gate decisively: 3.87× (o200k) / 3.92×
(cl100k)** — it more than *doubles* the v0.2 tier-2 aggregate (1.91×→3.87×) and lands
right alongside the frozen T_v0 headline of 3.72×. Almost the entire lift is `fold`:
it collapses the 8 traversal solutions from 145→60 tokens (o200k) while `xor` adds one
high-ratio task (single_number, 25/9 = 2.78×) and clears a WALL.

Generatability-hedge variant (**`fold = #`**, +4 mtl tokens): o200k 352/95 = **3.71×**,
cl100k 349/93 = **3.75×** — still clears 3× comfortably. The glyph choice moves the
headline by ~0.16×; it does **not** change the verdict.

---

## Scenario C — {fold + xor + indexed-access (a) nth/len}, 13 tasks

Adds `two_sum` and `binary_search` (option (a): `nth`/`len` on the cons-list, O(n) access).

| encoding | sum(py idiomatic) | sum(mtl-v0.3) | **aggregate** | vs B |
|---|---:|---:|---:|---:|
| o200k_base | 483 | 162 | **2.98×** | **−0.89×** |
| cl100k_base | 480 | 160 | **3.00×** | **−0.92×** |

two_sum-estimate sensitivity (band 30–38 tok): o200k **3.06× … 2.91×**;
cl100k **3.08× … 2.93×**. The dilution conclusion is robust across the whole band.

**Adding the two indexed tasks DILUTES the headline from ~3.9× down to ~3.0×** — a
~23% relative drop, landing *at* the 3× gate (below it on o200k for two_sum ≥ ~34).
The cause is purely that both indexed tasks sit **below** the fold-driven B average:

| task | py o200k | mtl-v0.3 o200k | ratio | vs B avg (3.87×) |
|---|---:|---:|---:|:--:|
| two_sum | 48 | ~34 | **~1.41×** | far below |
| binary_search | 83 | 37 | **2.24×** | below |

Each new task whose individual ratio is under the running aggregate pulls the sum-ratio
down. This is the **coverage-vs-compression tradeoff quantified**: scenario C raises
coverage from 11/13 → 13/13 solved but *cuts the compression headline by ~0.9×*.

---

## Verdict

1. **Does {fold + xor} clear tier-2 ≥3×?** **Yes — comfortably. 3.87× (o200k) / 3.92×
   (cl100k)**, more than double the v0.2 tier-2 baseline (1.91×/1.89×) and on par with
   the frozen T_v0 3.72×. Even the generatability-safe `fold=#` variant clears at
   3.71×/3.75×. `fold` is the entire lever; `xor` is a cheap, WALL-clearing bonus.
2. **Is chasing two_sum / binary_search the right goal?** **No — not for the headline.**
   Under the standing anti-tarpit rule (admit iff it pays for itself in corpus tokens),
   indexed-access (a) is compression-*negative* at the aggregate level: it drags the
   headline from ~3.9× to ~3.0×. Its case is coverage/writability (unblock two
   array-shaped tasks), exactly the non-token footing on which `uncons` was admitted —
   **not** the compression case that carries `fold`. If the design doc wants a single
   headline number, it is scenario **B (3.87×/3.92×)**; scenario C should be reported
   separately as a *coverage* figure with the explicit note that it dilutes
   compression. Recommendation: ship {fold + xor} for the v0.3 headline; treat
   indexed-access (a) as an optional coverage add-on decided on writability grounds,
   and document binary_search honestly as an O(n·log n) bisection scan (option (a) is
   not a true O(log n) binary search — see `indexed-access/`).

---

## Appendix — reproduce

```
python3 /tmp/.../scratchpad/agg.py          # full A/B/C recomputation (this file)
python3 /tmp/.../scratchpad/glyph_sweep.py  # glyph BPE sweep (glyphs/README.md)
# spot checks:
cd /home/user/MTL/bench
python3 tokcount/tokcount.py corpus/single_number/python-idiomatic/solution.py   # 25/25
printf '%s' '[>0=][0][][$]|' | python3 tokcount/tokcount.py                        # 9/9  (xor single_number)
printf '%s' '[][~;](' | python3 tokcount/tokcount.py                              # 4/4  (fold reverse_list)
printf '%s' '^#1-0~@[^^~<0=][@@___][@:^+2/:\@@^~<[1+~][~1-]?][+]|' | python3 tokcount/tokcount.py  # 37/37 binary_search(a)
```

Raw scenario tokcounts (o200k / cl100k):

```
A  py 327/324   mtl-v0.2 171/171   -> 1.9123 / 1.8947
B  py 352/349   mtl-v0.3  91/ 89   -> 3.8681 / 3.9213     (fold=(, xor=$)
B# py 352/349   mtl-v0.3  95/ 93   -> 3.7053 / 3.7527     (fold=#, hedge)
C  py 483/480   mtl-v0.3 162/160   -> 2.9815 / 3.0000     (two_sum=34 est, bsearch=37 meas)
   two_sum band 30..38: o200k 3.0570..2.9096 ; cl100k 3.0769..2.9268
```

Off the `bench/validate` discovery path and out of `tasks.json`; `cargo test` and the
frozen baselines are unaffected.
