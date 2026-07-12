# Indexed / random access — proof-impact assessment (P1–P4)

- Status: **design stage.** Nothing implemented; this is a static analysis of what
  each option costs the verified core. Grounded in the actual artifact
  (`crates/mtl-core/src/mtl_core.rs`, `crates/mtl-syntax/proofs/p4_verus.rs`).
- The value model is `SpecValue ::= Int(int) | Quote(Seq<SpecWord>)` — **two
  variants**, in both the ghost model and the exec twin. This is the object every
  option is measured against.

## 0. The proof surface, measured

Static counts over `crates/mtl-core/src/mtl_core.rs` (so the "how much changes"
claims are not hand-waved):

| surface | count | what it is |
|---|---:|---|
| `SpecValue` occurrences (ghost) | 56 | ~55 in match/construct positions across every typed `spec_step_prim` arm + the view functions |
| `Value::…` occurrences (exec) | 93 | the exec twin `exec_prim` + `view_*` |
| view / deep_view spec fns | 5 | `view_word`, `view_words`, `view_value`, `view_stack`, `deep_view` — the **P2 refinement bridge** |
| `decreases` clauses on the bridge | 2 | `view_word: decreases w, 0nat` and `view_words: decreases ws, ws.len()` — the lexicographic datatype-height measure that closed the "nested-quotation termination hole" (file header, line 8) |
| `Error` variants | 5 | `Underflow, TypeMismatch, Overflow, DivByZero, UnknownWord` — the P1 fault-precedence surface |
| exec-side twin sites (`interp.rs`) | 31 | must stay byte-for-byte in lock-step |
| `bench/validate` `conv` arms | 21 | one per `Prim`; grows by one per new primitive |

The decisive structural fact: **primitives are cheap to add; a Value variant is
not.** Adding a `SpecPrim` variant touches only the two `spec_step_prim`/`exec_prim`
dispatch matches (the typed arms already carry `_ => TypeMismatch`, so totality is
preserved by construction). Adding a `SpecValue` variant touches all ~55 match sites
**and reopens the deep-view termination measure** — the single hardest closed proof
in the file. Options (a) and (b) sit on opposite sides of exactly this line.

---

## Option (a) — `nth` + `len` primitives on the cons-list

### What lands
Two new `SpecPrim` variants. **No `SpecValue` change.** Recommended shapes:

- `len ( [xs] -- n )` — total; count the words in the quote.
- `nth ( [xs] i -- x 1 ) | ( [xs] i-out-of-range -- 0 )` — **flagged, uncons-shaped.**
  On an in-range index it extracts the element *as a value* exactly the way `uncons`
  extracts a head (`PushInt(i) -> Int`, `PushQuote(s) -> Quote`, else `TypeMismatch`),
  and pushes flag `1`; on an out-of-range index it pushes flag `0` and nothing else.

The flagged form is the whole trick for keeping the proof cost near zero: it reuses
`uncons`'s exact fault/extraction pattern and **introduces no new `Error` variant**
(out-of-range is a data flag `0`, not a fault — consistent with MTL booleans-as-ints
and with `uncons`'s empty case `( [] -- 0 )`).

### P1 — Determinism / fault precedence
- Two new non-overlapping match arms. Totality preserved by construction (same as the
  four v0.2 primitives). Fault precedence within each arm is the normative
  arity → type → semantic order: `n < ARITY -> Underflow`; non-quote / non-value-head
  → `TypeMismatch`; out-of-range → **flag, not fault** (so precedence is unchanged and
  the `Error` enum is **untouched**).
- Cost: re-audit non-overlap for 2 arms. Trivial; identical to admitting `uncons`.

### P2 — Refinement (currently OPEN, `admit()`-stubbed)
- **The deep-view bridge is untouched.** `view_value`/`view_words`/`view_stack`/
  `deep_view` and their `decreases` measures do not change, because no value shape
  changes — a "list" is still a `Quote`. This is the key win: option (a) does **not**
  go anywhere near the hard termination lemma.
- Two new arms to mirror in the exec twin (`exec_prim`) and two in the differential
  proptest oracle (`tests/interpreter.rs`). P2 stays exactly as open as it is today —
  **no new proof holes**, just two more arms whose refinement will be discharged with
  the rest when the GREEN interpreter closes P2.

### P3 — Progress
- Free: `spec_step` remains total (every arm returns `Next`/`Halt`/`Fault`). Restated
  `p3_progress` still holds by construction.

### P4 — Parser round-trip
- Two `GLYPHS` rows (`('#', Len), ('$', Nth)`). Both are single self-delimiting ASCII
  punctuation glyphs — **no new literal syntax, no new delimiter, no lexer
  maximal-munch change.** `print.rs::needs_separator` treats them as self-delimiting
  (they are). The P4 obligation and its skeleton (`p4_verus.rs`) are unaffected in
  structure; `wf_words` is unchanged (lists are quotes, already covered).

### New smoke theorems
Two, both near-copies of existing ones: `smoke_len` (like `smoke_uncons_empty`) and
`smoke_nth_hit` / `smoke_nth_oob` (like `smoke_uncons_head_int` / `smoke_uncons_empty`).

### Verdict for (a)
**Uncons-sized.** Same admission machinery that already shipped for the four v0.2
primitives, on a proven-safe pattern. No model change, no `Error` growth, no
deep-view reopening, no new P2/P4 holes. Days, not weeks. **Proof-safe even with P2
still open**, because it adds nothing to the refinement surface that isn't already
there for `uncons`.

---

## Option (b) — a real `Vec` value type

### What lands
A new value variant in **both** models:

```
SpecValue ::= Int(int) | Quote(Seq<SpecWord>) | Vec(Seq<SpecValue>)   // ghost
     Value ::= Int(i64) | Quote(Vec<Word>)    | Vec(Vec<Value>)       // exec
```

plus `get ( {v} i -- x )` (O(1)), `len ( {v} -- n )`, and (for real array use)
`set ( {v} i x -- {v'} )` (functional update), and **array-literal surface syntax**
`{ … }`.

This is a **model change**, and the file's own design note is explicit about where the
pain is (header, line 8): the "nested-quotation termination hole is closed" by a
lexicographic datatype-height measure. Option (b) reopens it.

### P1 — Determinism / fault precedence
- All ~55 `SpecValue` match sites must be re-audited. The typed arms
  (`apply`, `cat`, `cons`, `dip`, `eq`, `lt`, `if`, arith, divmod, `primrec`, `times`,
  `linrec`, `uncons`) keep their `_ => TypeMismatch` wildcard, so **totality survives
  by construction** — but each arm now has a third variant flowing through it, and the
  determinism/precedence audit surface grows by one variant across ~14 arms. Mechanical,
  but real, and easy to get subtly wrong (e.g. should `cons`/`cat` accept a `Vec`? the
  intended answer is "no, `TypeMismatch`", but that is now a *decision* to state and
  prove, not a non-issue).
- **New `Error` variant** `IndexOutOfBounds` for `get`/`set` out of range (unless both
  are flagged — but in-range `set` is a genuinely partial operation, so a fault is the
  honest model). `Error` 5 → 6 touches the P1 non-overlap/precedence proof and every
  arm's precedence audit.

### P2 — Refinement (the expensive part)
- **`view_value` gains a `Vec` arm**, which introduces a *third mutually-recursive
  spine*: `value → Seq<value> → value`, alongside the existing `word → Seq<word> → word`
  spine. The closed termination measure (`decreases w, 0nat` / `decreases ws, ws.len()`)
  must be **reopened and re-proven** to cover a value that structurally contains a
  `Seq<SpecValue>`. Verus datatype-height reasoning does support this (a `Vec<Value>`
  field sits strictly below its container), but this is genuine new proof work on the
  **single most delicate existing lemma** — the one the header calls out as hard-won.
- `view_stack`/`deep_view` and the exec twin (`exec_prim`, 31 `Value` sites) all grow a
  `Vec` case; the proptest oracle needs a `Vec` generator + `get`/`set`/`len`
  differential arms.
- Doing this **while P2 is still `admit()`-stubbed** means growing an unproven
  refinement surface: you would be adding a value variant to a refinement proof that is
  not yet closed, maximizing rework when P2 is finally discharged. This is the
  strongest single reason to **sequence (b) after P2**, not before.

### P3 — Progress
- Preserved by totality, as for (a) — but only after the arm-by-arm audit above.

### P4 — Parser round-trip (grows a whole syntactic form)
- Array literals need **new delimiter glyphs** distinct from `[ ]` (which mean
  quotation). This is a real glyph-budget hit: `{ }` (two of the seven free ASCII
  punctuation glyphs) consumed *just for the literal*, before any operator glyph.
- Lexer maximal-munch gains `{`/`}`; `print.rs` gains boundary/separator rules for them;
  and the P4 proof (`parse(print(p)) == Ok(p)`, `p4_verus.rs`) gains **a new case in the
  round-trip induction** plus an extension of `lemma_tokens_separated` and
  `wf_words`/`wf_word` (a `Vec` literal is well-formed iff every element is). P4 is
  currently an admitted skeleton; (b) enlarges it materially.

### New well-formedness invariant
- **I3′:** a `Vec` value is well-formed iff every element is well-formed (recursive).
  Threads through the parser postcondition (P4) and any I3-dependent lemma, and — when
  Layers E/F arrive — through the heap-DAG (P7) and refcount (P8) story.

### Multiplicity / affine story — breaks cleanly
- `get` **copies** an element out of the vector → replication, **not affine**. `set`
  is a **functional update** producing a fresh vector (persistent), with internal
  sharing. `uncons`'s clean "consumed once, split" affine reading (spec §14.4) does
  **not** extend to random-access `get`/`set`. This is a second aggregate constructor
  with internal sharing, which the (future) P7 acyclicity and P8 refcount obligations
  must both account for.

### Verdict for (b)
**Weeks, and it reopens the hardest closed proof.** New `Value` variant → ~55 audit
sites, a third recursion spine in the deep-view termination measure, a new `Error`
variant, a new well-formedness invariant, a materially larger P4 (new literal +
delimiter glyphs), and a broken affine story. It should **not** land before P2 is
discharged.

---

## Side-by-side

| dimension | (a) nth+len on cons-list | (b) Vec value type |
|---|---|---|
| new `SpecValue` variant | **none** | yes → ~55 audit sites |
| deep-view termination measure | **untouched** | **reopened** (third spine) |
| new `Error` variant | **none** (flagged) | `IndexOutOfBounds` (5→6) |
| P1 (determinism/precedence) | +2 arms, trivial | +1 variant across ~14 arms + Error |
| P2 (refinement, OPEN) | +2 arms; no new hole | grows the unproven bridge; sequence after P2 |
| P3 (progress) | free | free after audit |
| P4 (round-trip) | +2 glyph rows, no literal | new `{ }` delimiters + new literal case in the proof |
| new well-formedness inv. | none | I3′ (recursive Vec wf) |
| affine story | clean (uncons-shaped) | breaks (get copies, set persists) |
| glyph budget (of 7 free) | 2 op glyphs (`# $`) | 2 delimiters (`{ }`) + 2–3 op glyphs |
| smoke theorems | 2 (near-copies) | 3+ (new shapes) |
| effort | uncons-sized (days) | model change (weeks) |
| access complexity delivered | **O(n)** (walks the list) | **O(1)** (true random access) |

The last row is the honest punchline: **(a) is far cheaper to verify but does not
deliver logarithmic access** — `nth` still walks the cons-list, so binary_search under
(a) is O(n·log n), not O(log n). Only (b) buys the algorithmic property binary_search
actually needs, and it buys it by reopening the file's hardest proof.
