# `empty?` / `len` — rewritten solutions and hand-traces (v0.3 design stage)

- Status: **design stage.** `empty?`/`len` are **not implemented** in parser/interpreter; the rewrites below are **hand-traced against the semantics sketch in README.md §3**, not interpreter-validated.
- Tokenizers: `tiktoken` `o200k_base` + `cl100k_base`, 0.8.0 (pinned bench set). **Both encodings agree on every cell below.**
- Provisional glyphs: `empty?` → `\`, `len` → `` ` `` (final assignment is a separate worker; these are the free ASCII punctuation from the briefing).

## The substitution

`empty?` is a **non-consuming** predicate: `( [xs] -- [xs] b )`, `b=1` iff empty. This is *exactly* what the linrec null-test `[>[;0][[]1]?]` produces (it rebuilds the list, leaving `list flag`). So in all five accumulator solutions the change is a **single local swap of the P quote**:

```
[>[;0][[]1]?]   ->   [\]
```

Everything else — the leading setup, `T`, `R1` (still `>_…`), `R2` — is **unchanged**. Because `empty?` leaves the identical stack shape (`list flag`, flag=1 empty / 0 non-empty), the rewrites are behaviourally identical to the frozen solutions by substitution; the traces below confirm it.

| task | before (frozen `mtl-v0.2/solution.mtl`) | after (`empty?` = `\`) |
|---|---|---|
| max_list | `>_[>[;0][[]1]?][_][>_[^^<[~_][_]?]'][]\|` | `>_[\][_][>_[^^<[~_][_]?]'][]\|` |
| min_list | `>_[>[;0][[]1]?][_][>_[^^<[_][~_]?]'][]\|` | `>_[\][_][>_[^^<[_][~_]?]'][]\|` |
| reverse_list | `[]~[>[;0][[]1]?][_][>_[~;]'][]\|` | `[]~[\][_][>_[~;]'][]\|` |
| contains | `0~@[>[;0][[]1]?][__][>_[^=@~+0~<~]'][]\|` | `0~@[\][__][>_[^=@~+0~<~]'][]\|` |
| count_occurrences | `0~@[>[;0][[]1]?][__][>_[^=@~+~]'][]\|` | `0~@[\][__][>_[^=@~+~]'][]\|` |

`len` (consuming, `( [xs] -- n )`) replaces the **whole** `length_list` program, which is itself a length-computing linrec:

| task | before | after (`len` = `` ` ``) |
|---|---|---|
| length_list | `[>0=][0][][~_1+]\|` | `` ` `` |

## Why `R1` still re-unconses (the load-bearing subtlety)

`empty?` is non-consuming *by design* precisely so it can serve as a linrec predicate. MTL's `linrec` runs `P` with no save/restore, so `P` must be non-destructive itself. The frozen idiom achieved that by unconsing and **rebuilding** (`>[;0]…`); `empty?` achieves it by never deconstructing at all. Either way the list is still on the stack when `R1` runs, so `R1` must `>_` (uncons + drop-flag) to reach the head — this is **unchanged** and remains the fix from the TIER2 incident log (bare `_` false-passed single-element inputs). `empty?` does **not** remove that footgun; it only compresses the predicate. A *consuming* `empty?` would break `R1` (no list to uncons) and break `T` (nothing to drop) — hence non-consuming is the only drop-in.

---

## Hand-traces (stack shown bottom→top; `[]` = empty quote)

### reverse_list — input `[1 2]` → expect `[2 1]`  ✓ hand-traced
`[]~[\][_][>_[~;]'][]|`, linrec `P=[\] T=[_] R1=[>_[~;]'] R2=[]`.

- setup `[]~`: `[1 2]` → push `[]` → `[1 2] []` → `~` → `[] [1 2]`  (acc=`[]`, list on top)
- **iter 1** `[] [1 2]`: `P=\` → `[] [1 2] 0` (non-empty). If false → `R1`: `>` → `[] 1 [2] 1`; `_` → `[] 1 [2]`; `[~;]'` dips over `[2]`: on `[] 1` run `~;` → `~`→`1 []` → `;` cons(v=1,[q]=[])→`[1]`; restore → `[1] [2]`. recurse.
- **iter 2** `[1] [2]`: `\`→`[1] [2] 0`. `R1`: `>`→`[1] 2 [] 1`; `_`→`[1] 2 []`; dip over `[]`: `~`→`2 [1]`; `;`→`[2 1]`; restore → `[2 1] []`. recurse.
- **iter 3** `[2 1] []`: `\`→`[2 1] [] 1` (empty). If true → `T=_` drops `[]` → `[2 1]`. `R2=[]` no-op.

Result `[2 1]`. ✓

### max_list — input `[3 1]` → expect `3`  ✓ hand-traced
`>_[\][_][>_[^^<[~_][_]?]'][]|`, `T=[_] R1=[>_[^^<[~_][_]?]']`.

- setup `>_`: `>` uncons `[3 1]` → `3 [1] 1`; `_` drop flag → `3 [1]` (max=3, rest=`[1]`)
- **iter 1** `3 [1]`: `\`→`3 [1] 0`. `R1`: `>`→`3 1 [] 1`; `_`→`3 1 []`; dip `[^^<[~_][_]?]` over `[]` on `3 1`:
  `^`→`3 1 3`; `^`→`3 1 3 1`; `<`→ (3<1)=0 → `3 1 0`; `[~_][_]?` flag 0 → run `[_]` (drop head) → `3`. restore → `3 []`. recurse.
- **iter 2** `3 []`: `\`→`3 [] 1` (empty). `T=_` drops `[]` → `3`.

Result `3`. ✓  (min symmetric: body quotes `[_][~_]` swapped; on `[3 1]`, flag=(3<1)=0 → run `[~_]` → keep head `1` → min=`1`. ✓ hand-traced.)

### contains — input `[1 2] 2` → expect `1` (found)  ✓ hand-traced
`0~@[\][__][>_[^=@~+0~<~]'][]|`, `T=[__] R1=[>_[^=@~+0~<~]']`.

- setup `0~@`: `[1 2] 2` → `0`→`[1 2] 2 0` → `~`→`[1 2] 0 2` → `@`rot→`0 2 [1 2]` (found=0, x=2, list on top)
- **iter 1** `0 2 [1 2]`: `\`→`… 0`. `R1`: `>_`→`0 2 1 [2]`; dip body over `[2]` on `0 2 1`:
  `^`→`0 2 1 2`; `=`(1==2)=0→`0 2 0`; `@`→`2 0 0`; `~`→`2 0 0`; `+`→`2 0`; `0~<`→(0<0)=0→`2 0`; `~`→`0 2`. restore → `0 2 [2]`. recurse (found still 0).
- **iter 2** `0 2 [2]`: `\`→`…0`. `R1`: `>_`→`0 2 2 []`; body on `0 2 2`: `^`→`0 2 2 2`; `=`(2==2)=1→`0 2 1`; `@`→`2 1 0`; `~`→`2 0 1`; `+`→`2 1`; `0~<`→(0<1)=1→`2 1`; `~`→`1 2`. restore → `1 2 []`. recurse (found=1).
- **iter 3** `1 2 []`: `\`→`1 2 [] 1` (empty). `T=__` drops `[]` and `x` → `1`.

Result `1`. ✓

### count_occurrences — input `[2 2] 2` → expect `2`  ✓ hand-traced
`0~@[\][__][>_[^=@~+~]'][]|` (body = contains body minus the `0~<` OR-clamp).

- setup `0~@`: → `0 2 [2 2]`.
- **iter 1**: `\`→`…0`; `R1` `>_`→`0 2 2 [2]`; body on `0 2 2`: `^`→`0 2 2 2`; `=`=1→`0 2 1`; `@`→`2 1 0`; `~`→`2 0 1`; `+`→`2 1`; `~`→`1 2`. restore → `1 2 [2]`. (count=1)
- **iter 2**: `\`→`…0`; `>_`→`1 2 2 []`; body on `1 2 2`: `^`→`1 2 2 2`; `=`=1→`1 2 1`; `@`→`2 1 1`; `~`→`2 1 1`; `+`(1+1)→`2 2`; `~`→`2 2`. restore → `2 2 []`. (count=2)
- **iter 3**: `\`→`2 2 [] 1` (empty). `T=__` → `2`.

Result `2`. ✓

### length_list — `len` on input `[7 7 7]` → expect `3`  ✓ hand-traced
`` ` `` (consuming). By the §3 native rule: top is `Quote(q)` with `q.len()==3` (three top-level words) → pop the quote, push `3`. Result `3`. Equivalent to the frozen linrec `[>0=][0][][~_1+]|` which counts one uncons per top-level word. ✓
