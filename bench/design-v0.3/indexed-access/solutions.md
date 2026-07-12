# Indexed access — candidate solutions, hand-traces, token measurements

- Status: **design stage.** The `nth`/`len`/`get`/`set` primitives do **not** exist,
  so these programs are **hand-traced against the semantics sketches**, not
  interpreter-validated. Program strings are real and were counted with
  `bench/tokcount` (tiktoken 0.8.0, `o200k_base` + `cl100k_base`); both encodings
  agree on every cell unless noted. Exact minimal golf of a nested point-free loop is
  itself uncertain at design stage — counts are marked **design estimates** with a band.
- Provisional glyphs (a separate worker finalizes the glyph round): from the free ASCII
  set `# $ ( ) \` `` ` `` `{ }`. Option (a): `#`=len, `$`=nth. Option (b): `{ }`=array
  literal delimiters, `#`=len, `$`=get, `\`=set.

## 0. Baselines (denominators)

Measured on the frozen corpus Python (`bench/tokcount/tokcount.py`):

| task | Python idiomatic | Python minified |
|---|---:|---:|
| two_sum | **48** | 33 |
| binary_search | **83** | 74 |

The tier-2 aggregate uses **idiomatic** Python as numerator (matching `report.py`).

## 1. The new primitives, traced concretely

The load-bearing new semantics is the extraction itself. Traced on the two-sum input
`xs = [2 7 11 15]` (an MTL quote `[2 7 11 15]` = `PushInt 2, PushInt 7, PushInt 11,
PushInt 15`).

**`len` — `( [xs] -- n )`** (option a) or `( {v} -- n )` (option b):
`[2 7 11 15] #` → `4`. Total; counts words. ✓

**`nth` (option a, flagged, uncons-shaped) — `( [xs] i -- x 1 ) | ( oob -- 0 )`:**
- `[2 7 11 15] 0 $` → walk to word 0 = `PushInt 2` → extract as value `Int 2`, push flag `1` → stack `2 1`. ✓
- `[2 7 11 15] 3 $` → word 3 = `PushInt 15` → `15 1`. ✓
- `[2 7 11 15] 4 $` → out of range → `0` (flag only). ✓
- Extraction mirrors `uncons` exactly: `PushInt(i)->Int(i)`, `PushQuote(s)->Quote(s)`,
  bare `Prim`/`Call` head → `TypeMismatch`.

**`get` (option b) — `( {v} i -- x )`, O(1):** `{2 7 11 15} 2 $` → `Int 11` in one step
(array random access; out-of-range → `IndexOutOfBounds` fault, or flagged variant).

**`set` (option b) — `( {v} i x -- {v'} )`:** functional update; `{2 7 11 15} 1 9 \` →
`{2 9 11 15}`. Copies the vector (persistent).

## 2. binary_search

Target trace: `binary_search([1 3 5 7 9], 7) → 3`. Python idiomatic keeps `lo/hi`,
probes `mid=(lo+hi)//2`, compares `xs[mid]`.

### Option (a) — bisection shape over the cons-list (nth = `$`, O(n) per probe)

Carry `xs t lo hi`; `linrec |` loops while `lo<=hi`; each iter `mid=(lo+hi)/2`,
`x = xs nth mid`, branch. Representative program (design estimate — exact routing
of the four-deep carried state is the hard part):

```
^#1-0~@[^^~<0=][@@___][@:^+2/:$@@^~<[1+~][~1-]?][']|
```

- Measured: **o200k = 37, cl100k = 37.**
- Value-level trace, `[1 3 5 7 9] 7`: `^#1-` → hi=4, `0~@` seeds lo=0, order to
  `xs t lo hi`. Probe 1: mid=(0+4)/2=2, xs[2]=5, 5<7 → lo=mid+1=3. Probe 2:
  mid=(3+4)/2=3, xs[3]=7, 7==7 → return mid=3. ✓
- **Complexity caveat:** `$` walks the cons-list from the head every probe, so this is
  O(n) per probe → **O(n·log n) overall. It is not a true binary search** — the
  algorithmic property the task is named for is not delivered.

### Option (a′) — pragmatic linear index-of (also expressible; ~33 tok)

Because `$` is O(n) anyway, the honest cheapest expressible form is a linear scan
carrying an index counter (this shape is in fact already expressible in v0.2 today with
`linrec`+`uncons`+arithmetic — see §4). Returns the correct index at O(n). ~33 tokens.

### Option (b) — true binary search (get = `$`, O(1))

Identical program shape to (a), but `$` is O(1) array access:

```
^#1-0~@[^^~<0=][@@___][@:^+2/^^~$@~<[1+~][~1-]?]['+~]|
```

- Measured: **o200k = 39, cl100k = 39.** Same trace, but each probe is O(1) → **true
  O(log n)**.

### The honest comparison

| | tokens (o200k) | ratio vs py 83 | access | true binary search? |
|---|---:|---:|---|:--:|
| (a) bisection-shape | 37 | 2.24× | O(n)/probe | **no** (O(n·log n)) |
| (a′) linear index-of | ~33 | ~2.5× | O(n) | no (linear) |
| (b) vector get | 39 | 2.13× | O(1)/probe | **yes** |

**(a) is 2 tokens *cheaper* than (b) yet algorithmically worse.** The token cost is a
wash; (b)'s entire value is the complexity property, paid for with the model-change
proof cost in `proof-impact.md`.

## 3. two_sum

Target trace: `two_sum([2 7 11 15], 9) → [0 1]` (indices of `2+7=9`).

### Options (a) and (b) — brute-force O(n²), nth/get to read `xs[i]`, `xs[j]`

Carry `xs t n i`; outer `times`/`primrec` over i, inner over j>i, compare
`xs[i]+xs[j]==t`, on hit build the result quote `[i j]` with `;`. Schematic
representative (the `[…]` result-build is the part that inflates a correct version):

```
^#0[[^^$^@$+@=][ji][_1+]?].[_1+].
```

- Measured (schematic core): o200k = 22. A **correct** version that (i) builds the
  `[i j]` pair with `;`, (ii) guards first-match, and (iii) cleans the stack lands at a
  **design estimate of ~34 tokens (band 30–38)**, both encodings.
- Value-level trace, `[2 7 11 15] 9`: n=4. i=0 (val 2): j=1 → 2+7=9 == target → emit
  `[0 1]`. ✓ Result `[0 1]`.
- (a) and (b) cost the **same tokens** here; the only difference is `$` being O(n)
  (a) vs O(1) (b). Brute-force two_sum is O(n²) either way, so (b)'s O(1) access does
  **not** improve two_sum's asymptotic class — it only matters for binary_search.

| | tokens (o200k) | ratio vs py 48 |
|---|---:|---:|
| (a) nth brute-force | ~34 (est.) | ~1.41× |
| (b) get brute-force | ~34 (est.) | ~1.41× |

two_sum lands **below** the current 1.91× aggregate under both options.

## 4. Are these even blocked today? (honest caveat, feeds option c)

- **binary_search** is I/O-solvable in **v0.2 today** with no new primitive: a
  `linrec`+`uncons` linear index-of carrying a counter returns the correct index — just
  at O(n), not O(log n). The WALL is really an **algorithmic-complexity** wall, not an
  I/O-expressibility wall.
- **two_sum**'s index return is likewise reachable by carrying a counter during an
  `uncons` walk (a counter *is* an attached index); a nested double-walk returning
  `[i j]` is Turing-expressible (MTL is ~TC). The WALL's "inexpressible" overstates
  strict impossibility — it is *impractical* point-free, not impossible.

So `nth`/`len` (option a) mostly buy **token/writability** (avoid re-deriving `len` via
a fold; avoid hand-threading an index counter), not the impossible. By the standing
admission rule ("pays for itself in corpus-level token accounting"), that case is
**weak** — see the aggregate below.

## 5. Aggregate effect if admitted (the quantitative crux)

Current tier-2 aggregate (10 solvable tasks): **327 py / 171 mtl = 1.91×** (o200k).
Admitting both blocked tasks under option (a) representative counts:

| | py (idiomatic) | mtl (est.) |
|---|---:|---:|
| current 10 tasks | 327 | 171 |
| + two_sum | 48 | 34 |
| + binary_search | 83 | 37 |
| **new total (12 tasks)** | **458** | **242** |
| **new aggregate** | | **1.89×** |

**Admitting indexed access is compression-neutral (1.91× → 1.89×).** binary_search
alone helps (2.24× > aggregate); two_sum drags (1.41×); net flat-to-slightly-down.
Coverage rises 10/13 → 12/13, but the headline metric does not move. This is the
decisive number: **indexed access earns admission on coverage/writability, not on
tokens** — exactly the footing on which `uncons` was admitted (TC/list rationale, not
corpus tokens).

## Appendix — reproduce

```
cd /home/user/MTL/bench
python3 tokcount/tokcount.py corpus/two_sum/python-idiomatic/solution.py
python3 tokcount/tokcount.py corpus/binary_search/python-idiomatic/solution.py
printf '%s' '^#1-0~@[^^~<0=][@@___][@:^+2/:$@@^~<[1+~][~1-]?][+]|' | python3 tokcount/tokcount.py   # binary_search (a)
printf '%s' '^#1-0~@[^^~<0=][@@___][@:^+2/^^~$@~<[1+~][~1-]?][+~]|' | python3 tokcount/tokcount.py  # binary_search (b)
```

None of these paths is on the `bench/validate` discovery path, so `cargo test` and the
frozen baselines are unaffected.
