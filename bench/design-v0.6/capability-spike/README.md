# MTL v0.6 — indexed access via HOST CAPABILITIES (option (d)) — measurement spike

- Status: **design-stage measurement spike.** No production/verified file is touched.
  Everything lives under `bench/design-v0.6/capability-spike/`. This crate is a
  **detached workspace** (its own `[workspace]` table) so it is NOT a member of the
  repo workspace and cannot perturb any `crates/` crate; it consumes `mtl-core`,
  `mtl-syntax`, and `mtl-host` only as read-only path dependencies.
- What (d) is: indexed access (`nth`/`len`/`slice`) is provided **not** as a core
  value type (option b) or core primitive (option a), but as **host capability
  calls** over the existing v0.4 effects seam — the same "strings pattern" MTL
  already uses. The core stays exactly `Int | Quote`; a sequence is an opaque `i64`
  **handle** on the stack (`Value::Int`), and the host owns the `Vec<i64>`.
- Tokenizers: tiktoken `o200k_base` + `cl100k_base` (`bench/tokcount`). Counts agree
  under both encodings on every row below.

## 1. The capability mechanism (how a host call is written, and its token cost)

A capability call is just a **bareword** `Call(name)` in MTL source (lexer:
`crates/mtl-syntax/src/parse.rs:256-265`, names are `[a-z][a-z0-9]*` maximal-munch).
At runtime the pure core suspends at every `Call` and yields
`Outcome::Invoke{name, stack}` (`crates/mtl-core/src/host.rs:44-56, 130-158`); the
host `HostShim::service` grant-checks the name, meters it, runs the capability
closure (which pops declared inputs and pushes declared outputs on the value stack),
and resumes (`crates/mtl-host/src/core_bridge.rs:92-143`). Capabilities are plain
data — `Capability{name, effect, faults, run}` in a `Registry`
(`crates/mtl-host/src/capability.rs:45-84`). The existing **`select` capability**
(`crates/mtl-host/src/caps/mod.rs:293-310`) is already exactly host-side `nth` over
a `Quote` of ints — precedent for (d).

**Calling convention:** inputs are consumed / outputs pushed on the ordinary MTL
value stack (bottom..top). Nothing about the name is special beyond it being an
unrecognised bareword; the host selects the capability by string name.

**Token cost of a host call = the tokenization of its NAME word + any forced
whitespace.** Two measured facts (crucial for (d)'s ergonomics):

| fragment | o200k | note |
|---|---:|---|
| `nth`, `len`, `slice` (alone) | 1 | short lowercase word = 1 token, same as a glyph |
| `:nth`  | **1** | a name abutting a *glyph* MERGES into 1 token |
| `^len 1-` vs `^#1-` | 5 vs 4 | a name before a *digit* forces a space → +1 token |
| `nth len` | 2 | two adjacent names force a space |
| `:nthc` | 2 | a 4-letter / unusual name tokenizes WORSE than `nth` |

So: **name length and spelling drive the cost.** Pick short, common-substring names
(`nth`, `len`) — they cost 1 token and often merge with a preceding glyph to 0 extra.
The only real overhead vs a single-char glyph is a **forced space when a name abuts a
digit or another name** (~+1 token per such site). Names never consume a scarce ASCII
glyph (a real plus: `$` and `(` are already taken by `xor`/`fold` in v0.3).

## 2. Is (d) a natural fit for the existing capability model?

Yes — mechanically it is a small, in-pattern host addition, **not** core proof work:

- **Reuses:** the `Invoke` seam, `HostShim` servicing, `Registry`, metering, and the
  opaque-`i64`-handle idea (`handle.rs`) verbatim. `select` already demonstrates
  host-side indexing.
- **New host-runtime work (option (d)'s real cost — all UNVERIFIED host code):**
  1. a host-side sequence store (a `Vec<Vec<i64>>` / handle table for sequences,
     mirroring `HandleTable`);
  2. three capability registrations: `nth`, `len`, `slice` (each ~15 lines, see
     `src/main.rs`);
  3. an input capability to hand a sequence handle to the program (a `readseq`, like
     `readlines`), plus declared stack-effects / fault contracts / meter budgets.
- **Core / verification impact: ZERO.** No new `SpecValue` variant, no new `SpecPrim`
  arm, no new `Error`, no P1–P4 obligation, no glyph. The affine/verification story is
  unchanged: the core still only copies/drops/shuffles `Int` handles.

## 3. VALIDATED mechanism (real `mtl_core::host::drive`)

`cargo run --release -- <program> <csv-seq> <target>`. Capabilities registered in
`src/main.rs`: `nth (s i -- s x)`, `len (s -- s n)`, `slice (s lo hi -- s')` and the
consuming variants `nthc (s i -- x)`, `lenc (s -- n)`; each indexes the host `Vec<i64>`
in **O(1)**. Initial stack is `[s_handle, target]`.

| program | seq | result | checks |
|---|---|---|---|
| `_3nth`          | 10,20,30,40,50 | `[0 40]` | nth s[3]=40, handle preserved |
| `_lenc`          | 10,20,30,40,50 | `[5]`    | length |
| `_2nthc`         | 10,20,30,40,50 | `[30]`  | consuming nth s[2]=30 |
| `_1 3slice lenc` | 10,20,30,40,50 | `[2]`   | slice[1,3)=[20,30], len 2 |
| `~0:[nth]'`      | 10,20,30       | `[99 0 10 0]` | probe idiom `t s i → t s x i` |
| `1 3^^+2/`       | 10,20,30,40,50 | `[0 99 1 3 2]` | mid=(lo+hi)/2 idiom |

All **HALT** with correct output → the invoke_host convention services host-side
sequences end-to-end. O(1) host access means a bisection is a **TRUE O(log n)** binary
search (unlike option (a)'s O(n)-per-probe cons-list walk → O(n·log n)).

## 4. Candidate programs + token table

`nth`/`len` are host calls; the list is a handle. **Programs share option (a)'s exact
control structure** — only `#`→`len`, `$`→`nth` (word for glyph) — so the token delta
is purely name/space overhead.

**binary_search (option d), 38 tok** (o200k=cl100k):
```
^len 1-0~@[^^~<0=][@@___][@:^+2/:nth@@^~<[1+~][~1-]?][+]|
```
*Parses; parity transliteration of option (a)'s reference program. NOTE: option (a)'s
own reference string does not actually compute the right index either — running its
exact structure in this harness returns 0 for `bsearch([1,3,5,7,9],7)` (should be 3).
So binary_search is an ESTIMATE on ALL of options a/b/d; the honest, robust claim is
the +1-token structural delta, not an interpreter-validated run.*

**two_sum (option d), short names, ~24 tok schematic / ~36 tok corrected estimate**:
```
^len 0[[^^nth^@nth+@=][ji][_1+]?].[_1+].
```
*Schematic (mirrors option (a)'s 22-tok schematic; `[ji]` = build `[i j]`). Hand-traced
at value level, NOT interpreter-validated (the corrected `[i j]`-building version is a
design estimate, exactly as in option (a)).*

| task | py (o200k/cl100k) | opt (a) | opt (d) | ratio (d vs py) | true O(log n)? | validated |
|---|---:|---:|---:|---:|:--:|:--:|
| two_sum        | 48 / 48 | ~34 est | **~36 est** (schem. 24) | ~1.33× | n/a (O(n²) either way) | hand-traced |
| binary_search  | 83 / 83 | 37 (O(n·log n)) | **38** (O(log n)) | 2.18× | **yes** (host O(1)) | hand-traced; mechanism validated |

**Name-overhead measurement (the crux):** binary_search opt(a) 37 → opt(d) 38 = **+1
token** (one forced space in `len 1`). two_sum opt(a) schem. 22 → opt(d) 22+2 = 24 with
3-letter names (`nth`/`len`); using 4-letter `nthc`/`lenc` it is 27 (+5) — so name
choice matters. Well-chosen short names make (d) ~1–2 tokens dearer than (a), NOT more.

## 5. Verdict

- **Tokens:** (d) is within **+1–2 tokens** of (a) when names are short (`nth`/`len`).
  Capability names do NOT sink this option — a well-chosen 3-letter name costs the same
  1 token as a glyph and even merges with an adjacent glyph. Multi-token names only bite
  if you pick long/unusual spellings.
- **What (d) uniquely buys:** true **O(1)** random access (→ true **O(log n)**
  binary_search, which (a) cannot deliver), **zero core change**, **zero proof work**,
  **zero glyph** consumed. Cost is moved entirely to the **unverified host runtime**
  (a sequence store + 3–4 capability registrations).
- **Real contender vs (a)?** Yes — for essentially the same token price it delivers the
  complexity property (a) fails to, without touching the verified core or the scarce
  glyph budget. Its price is host-side TCB surface, not proof risk. If the corpus goal
  is *coverage + correct algorithmic complexity with a stable verified core*, (d)
  dominates (a). If the goal is a self-contained pure-core language, (a)/(b) keep
  indexing inside the proved artifact where (d) pushes it into the host.
