# MTL v0.3 — Bitwise operators (design stage, measurement-driven)

- Status: **design stage**. The `xor` primitive below is **not yet implemented** in
  the parser/interpreter; its program is **hand-traced against the semantics sketch**
  (§3), not interpreter-validated. Every token count is real (measured with
  `bench/tokcount`, tiktoken 0.8.0 in this env); the correctness claim is a hand-trace,
  explicitly marked `✓ hand-traced`.
- Branch: `v03-design`. Frozen `crates/`, `bench/BASELINE.md`, `bench/BASELINE-TIER2.md`,
  the frozen `bench/corpus/**/mtl/` and `**/mtl-v0.2/` solutions, `bench/design-v0.2/`, and
  `bench/agent-trial/` are **untouched**. All new work lives under `bench/design-v0.3/bitwise/`.
- Tokenizers: tiktoken `o200k_base` and `cl100k_base`. Counts are identical under both
  encodings for every program in this doc.

---

## 1. The problem, quantified

`single_number` (tier-2 task 11) is a **WALL** — inexpressible in MTL v0.2. Its canonical
algorithm is an XOR-reduce (`a ^ a == 0`, so all paired elements cancel and the lone
element survives), and there is **no bitwise primitive** anywhere in the 21-primitive set.
The glyphs that *look* bitwise are all taken (verified on the interpreter, per WALL.md):
`^` = Over (`5 3^` → `5 3 5`), `&` = PrimRec, `|` = LinRec. Values are `Int(i64) | Quote`
with no bit access. So the task is excluded from the tier-2 aggregate entirely (no MTL
denominator).

| task | idiomatic Python (tok, o200k/cl100k) | MTL v0.2 | status |
|---|---:|---:|:--:|
| single_number | 25 / 25 | — | **WALL (inexpressible)** |

**Goal:** admit the minimal bitwise primitive that unblocks this task (and the bit-
manipulation problem class), justified by corpus-level token accounting, and honestly
report which of `and`/`or`/`shl`/`shr` earn admission vs. are speculative.

---

## 2. Candidate set

| candidate | shape it captures | stack effect | glyph (measured, §5) | verdict |
|---|---|---|---|---|
| `xor` | XOR-fold / parity / toggle | `( a b -- a^b )` | `$` (provisional) | **admit** |
| `and` | bit mask / clear | `( a b -- a&b )` | provisional free char | defer (speculative) |
| `or`  | bit set / boolean-or | `( a b -- a\|b )` | provisional free char | defer (speculative) |
| `shl` | `<< ` (scale, pack) | `( a b -- a<<b )` | provisional free char | defer (speculative) |
| `shr` | `>>` (unpack, halve) | `( a b -- a>>b )` | provisional free char | defer (speculative) |

Only `xor` has corpus evidence (§6). `and`/`or`/`shl`/`shr` block **0** corpus tasks and
carry real costs (glyph scarcity, and for the shifts an added semantic-fault decision);
see §9.

---

## 3. Small-step semantics (spec_step extension sketches)

Written to match `spec_step_prim` in `crates/mtl-core/src/mtl_core.rs`. XOR is a **total
binary Int→Int op**, structurally identical to `Eq`/`Lt` (mtl_core.rs lines 267–294): arity
check first (`Underflow`), then operand types (`TypeMismatch`), then a total semantic body
with **no** semantic-fault arm. `n = stk.len() as int`, top at `stk[n-1]`. New variant:
`SpecPrim::Xor`.

### 3.1 `xor` — `( a b -- a^b )`

Pop two Ints, push their bitwise XOR. MTL ints are **i64** (`Value ::= Int(i64)`, spec §3),
so XOR is defined on the **64-bit two's-complement bit pattern** — exactly Rust's `i64 ^ i64`.
Unlike `Add`/`Sub`/`Mul` (which compute in `int` then check `in_i64` for `Overflow`), the XOR
of two in-range i64 values is **always** in i64 range, so there is **no `Overflow` arm and no
`DivByZero` arm** — the semantic layer is trivially total, matching `Eq`/`Lt`. Fault order is
therefore just arity → type.

```rust
// dispatch line, alongside SpecPrim::Add / Sub / Mul:
SpecPrim::Xor => {
    if n < 2 { SpecStep::Fault(Error::Underflow) }        // (1) arity  -> Underflow
    else {
        match (stk[n - 2], stk[n - 1]) {
            (SpecValue::Int(a), SpecValue::Int(b)) =>
                // (3) semantic: total. Bitwise XOR on the i64 two's-complement
                // representation. a,b are constrained in_i64 by the stack invariant;
                // (a ^ b) is always in i64 range => no Overflow arm (cf. Eq/Lt).
                SpecStep::Next(SpecState {
                    stack: stk.subrange(0, n - 2).push(
                        SpecValue::Int(i64_bitxor(a, b))),   // ((a as i64) ^ (b as i64)) as int
                    cont: rest,
                }),
            _ => SpecStep::Fault(Error::TypeMismatch),        // (2) type -> TypeMismatch
        }
    }
}
```

The exec twin in `interp.rs` is literally `a ^ b` on `i64` (no `checked_*`, since it cannot
overflow), mirroring the `exec_cmp` shape rather than `exec_arith`.

### 3.2 (speculative) `and` / `or` — `( a b -- a&b )`, `( a b -- a|b )`

Identical shape and fault story to `xor`: total, arity → type only, no Overflow/DivByZero
arm (`a & b` and `a | b` of two in-range i64 are in range). Swap `i64_bitxor` for
`i64_bitand` / `i64_bitor`. **Not recommended for admission** (§9).

### 3.3 (speculative) `shl` / `shr` — `( a b -- a<<b )`, `( a b -- a>>b )`

Shifts are **not** trivially total and would **add a semantic-fault decision** that xor/and/or
do not: a shift amount `b < 0` or `b >= 64` is undefined for a plain i64 shift (Rust `<<`/`>>`
panic in debug and mask in release). MTL would have to pick one of:
(a) mask the count (`wrapping_shl`, count `& 63`), (b) saturate (count ≥ 64 → 0), or
(c) **fault** (a new `ShiftOutOfRange`, or reuse `Overflow`). `shr` additionally must choose
**arithmetic vs logical** shift (i64 `>>` is sign-extending). Each choice is a real spec
commitment with a proof obligation, for **zero** corpus payoff. **Strongly defer** (§9).

---

## 4. Multiplicity / affine story

| primitive | args | reading |
|---|---|---|
| `xor` | `a`, `b` both linear Int values, consumed once | plain binary op, exactly like `+`/`=`/`<`. No quote arg, no replication, no affine subtlety. Touches only the value layer's Int case. |

Bitwise ops are the simplest kind of primitive in the substructural sense: two linear Int
operands in, one Int out. They do **not** touch the value model (still `Int(i64) | Quote`),
do **not** add a value constructor, and do **not** interact with quotations — so their
proof impact is minimal (see §proof-impact below).

---

## 5. Glyph assignment (measured, spec §11)

The "obvious" bitwise glyphs are **all taken**: `^`=Over, `&`=PrimRec, `|`=LinRec. Only **8**
free ASCII punctuation chars exist across all of v0.3: `` # $ ( ) \ ` { } `` — a **real,
scarce budget** shared with the other v0.3 candidates (indexed-access, empty?/len, fold
combinator) running in parallel. Provisional glyph for `xor` is **`$`**; final assignment is
a separate worker's job (spec §11 corpus-level sweep).

Measured `xor`-fold cost across the free glyphs (program `[>0=][0][][G]|`, both encodings agree):

| glyph `G` | o200k | cl100k | note |
|---|---:|---:|---|
| `$` | **9** | **9** | merges: `[$` is one token |
| `#` | 9 | 9 | ties `$` |
| `(` | 9 | 9 | ties (but likely wanted by another candidate) |
| `{` | 9 | 9 | ties |
| `\` | 9 | 9 | ties (shell-hostile in sources) |
| `)` | 10 | 10 | no merge |
| `}` | 10 | 10 | no merge |

**Key merge fact:** with `$`, the tokenizer splits `…[$]|` as `['[$', ']|']` — the `[$`
bigram is a **single token** in both encodings (common in shell/template/regex corpora), so
the whole fold is **9 tokens, one LESS than the arithmetic-analogue `+`-fold (10)**. The
`+`-fold splits as `['[', '+', ']|']` because `[+` does not merge. So the scarce glyph `$`
is, in this context, *more* merge-friendly than a "premium" arith glyph — a rare case where
glyph scarcity does **not** cost tokens.

Verbatim token splits (measured):
```
o200k  [>0=][0][][$]|  -> ['[','>','0','=','][','0','][]','[$',']|']   = 9
o200k  [>0=][0][][+]|  -> ['[','>','0','=','][','0','][]','[','+',']|'] = 10
```
(cl100k identical.)

**The glyph-scarcity cost is real for the *speculative* ops, not for xor.** Admitting
`and`/`or`/`shl`/`shr` too would burn 1 scarce free char *each* out of a budget of 8 shared
across all v0.3 candidates. A bitwise op forced onto a 2-char BPE fallback name (once single
chars run out) would cost extra tokens per use. This is the honest cost that keeps the
recommendation to **xor only** (§9).

---

## 6. Measured results and projected aggregate

All MTL rows are **hand-traced (design stage)**, glyph-measured. Trace in `solutions.md` §7.
Both encodings agree on every cell.

### 6.1 single_number: WALL → expressible

| task | idiomatic Py (o200k/cl100k) | MTL v0.3 (`xor`) | primitive | ratio |
|---|---:|---:|---|---:|
| single_number `[>0=][0][][$]\|` | 25 / 25 | **9 / 9** | `xor` `$` | **2.78×** |

`ratio = tokens(py-idiomatic) / tokens(mtl) = 25 / 9 = 2.78×` (both encodings). The Python
reference is `bench/corpus/single_number/python-idiomatic/solution.py` (the `r ^= x` loop),
measured at 25 tokens — matching BASELINE-TIER2.md row 33.

### 6.2 Effect on the tier-2 aggregate

`xor` moves single_number from **inexpressible** (excluded from the 1.91×/1.89× aggregate)
to **expressible**, adding 25 py tokens to the numerator and 9 mtl tokens to the denominator
in each encoding. The tier-2 aggregate is a token-SUM ratio over the solvable set
(report_tier2.py):

| encoding | before (10 tasks) | after (11 tasks, +single_number) | Δ |
|---|---|---|---|
| o200k_base | 327 / 171 = **1.91×** | (327+25) / (171+9) = 352 / 180 = **1.96×** | +0.05× |
| cl100k_base | 324 / 171 = **1.89×** | (324+25) / (171+9) = 349 / 180 = **1.94×** | +0.05× |

The lift is modest because single_number's ratio (2.78×) is only slightly above the running
aggregate — but crossing from "inexpressible" to "expressible" is the point: it removes a WALL
and unblocks the entire bit-manipulation class, of which single_number is one instance.

---

## 7. Recommended admission set

**Admit on token grounds: `xor` (`$`, provisional glyph).** Unblocks the single_number WALL
(and the bit-manipulation class), collapses a previously-inexpressible task to a 9-token
clean fold (2.78×), and — because `[$` merges — costs *fewer* tokens than the arith-analogue.
Trivially total (arity→type only), zero value-model impact, minimal proof burden.

**Defer `and` / `or`:** block 0 corpus tasks. The only tangential use is boolean-and/or
synthesis (tier-2 candidate 5, a *minor* tax that blocks nothing; OR is currently synthesized
as `+0~<`, 4 tokens) — not worth burning a scarce free glyph on speculation. Revisit only if
a future corpus task needs true bit masking.

**Strongly defer `shl` / `shr`:** block 0 corpus tasks **and** carry an added semantic cost
xor/and/or don't — a shift-amount out-of-range decision (mask/saturate/fault) plus the
arithmetic-vs-logical choice for `shr`, each a real spec+proof commitment. Zero corpus payoff.

**Honest bottom line: only `xor` pays.** The rest are speculative and would each consume 1 of
just 8 scarce free ASCII chars shared across all v0.3 candidates.

---

## 8. Proof impact (P1–P4)

Minimal. `xor` is a total binary op on the existing `Int` case of the value model — it adds
**no** value constructor, touches **no** quotation/heap machinery, and interacts with **no**
recursion primitive.

- **P1 (determinism / non-overlapping rules / fault precedence):** preserved by construction.
  The arm is a total function; fault precedence is the same arity→type ordering as `Eq`/`Lt`,
  with no semantic-fault arm to order.
- **P2 (refinement exec↔spec):** one new lock-step pair — spec `i64_bitxor` vs. exec `a ^ b`
  on i64. These agree definitionally (both are the 64-bit two's-complement XOR); the simplest
  possible refinement case, no `checked_*`/Overflow reasoning.
- **P3 (progress):** the arm returns `Next`/`Fault` for every input (never stuck); trivially
  preserved.
- **P4 (parser round-trip):** one new `GLYPHS` row (`$` ↔ `Xor`), self-delimiting like every
  other symbol glyph; `needs_separator` unaffected.

No impact on P5–P9 (TC/heap/resource proofs) — `xor` does not touch quotations or the heap.

---

## Appendix — reproduce

All counts re-derivable with the pinned encoders:
```
cd /home/user/MTL/bench
python3 tokcount/tokcount.py corpus/single_number/python-idiomatic/solution.py   # 25 / 25
printf '%s' '[>0=][0][][$]|' | python3 tokcount/tokcount.py                        # 9 / 9
```
This path (`bench/design-v0.3/bitwise/`) is OFF the `bench/validate` discovery path and not in
`tasks.json`, so `cargo test`, `bench/BASELINE.md`, and `bench/BASELINE-TIER2.md` are unaffected.
