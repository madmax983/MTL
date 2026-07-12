# `pick` / `roll` — corpus rewrites, hand-traces, and measurements

Companion to `README.md`. Every program string here was token-counted with
`bench/tokcount` (o200k_base + cl100k_base, tiktoken 0.8.0); both encodings agree
on every cell. Rewrites are hand-traced against the §3 semantics sketch (pick/roll
are **not** implemented, so nothing here is interpreter-validated). Provisional
glyphs: `pick → (`, `roll → )`.

Semantics recap (depth `d` popped from top; 0 = new top):
`0 pick`=dup `:`, `1 pick`=over `^`; `0 roll`=nop, `1 roll`=swap `~`, `2 roll`=rot `@`.

---

## 1. Router equivalence table (measured)

| op today | tok | pick/roll | tok | Δ |
|---|--:|---|--:|--:|
| `:` dup | 1 | `0(` | 2 | +1 |
| `^` over | 1 | `1(` | 2 | +1 |
| `~` swap | 1 | `1)` | 2 | +1 |
| `@` rot | 1 | `2)` | 2 | +1 |
| `^^` | 1 | `1(1(` | 4 | +3 |
| `~@` | 2 | `1)2)` | 4 | +2 |

## 2. Full-solution rewrites (measured, behavior-preserving)

| task | variant | program | o200k | cl100k |
|---|---|---|--:|--:|
| contains | original | `0~@[>[;0][[]1]?][__][>_[^=@~+0~<~]'][]|` | 26 | 26 |
| contains | pick/roll | `0 1)2)[>[;0][[]1]?][__][>_[1(=2)1)+0 1)<1)]'][]|` | 33 | 33 |
| count | original | `0~@[>[;0][[]1]?][__][>_[^=@~+~]'][]|` | 23 | 23 |
| count | pick/roll | `0 1)2)[>[;0][[]1]?][__][>_[1(=2)1)+1)]'][]|` | 29 | 29 |
| max_list | original | `>_[>[;0][[]1]?][_][>_[^^<[~_][_]?]'][]|` | 22 | 22 |
| max_list | pick/roll | `>_[>[;0][[]1]?][_][>_[1(1(<[1)_][_]?]'][]|` | 25 | 25 |

Every rewrite LOSES tokens. No corpus site breaks even, because all routing is at
depth ≤ 3 where a single named glyph is already 1 token.

## 3. Hand-traces (bottom → top)

### 3.1 rev3 `~@` ≡ `1)2)`, input `[a b c]`
Original: `~`(swap) `a b c`→`a c b`; `@`(rot, `x y z -- y z x`) `a c b`→`c b a`.
Rewrite: `1)`(roll d=1, move 2nd→top = swap) `a b c`→`a c b`; `2)`(roll d=2, move
3rd→top = rot) `a c b`→`c b a`. Identical result `c b a`. 2 tok → 4 tok. ✓ hand-traced.

### 3.2 max_list `^^` ≡ `1(1(`, dip context stack `acc head`
Original: `^`(over) `acc head`→`acc head acc`; `^`(over on `acc head acc`, copies
2nd item `head`)→`acc head acc head`.
Rewrite: `1(`(pick d=1 = over) `acc head`→`acc head acc`; `1(`(pick d=1 on
`acc head acc`, copies 2nd item `head`)→`acc head acc head`. Identical. `^^`=1 tok →
`1(1(`=4 tok (the BPE-merge killer). ✓ hand-traced.

### 3.3 contains setup `0~@` ≡ `0 1)2)`, input `list x`
Original: `0`→`list x 0`; `~`→`list 0 x`; `@`(rot)→`0 x list` (state `found=0, x, list`).
Rewrite: `0`→`list x 0`; `1)`(swap)→`list 0 x`; `2)`(rot)→`0 x list`. Identical.
Note the mandatory space in `0 1)`: `01` would maximal-munch to `Int(1)`, so the
literal must be separated from the depth digit — that space is itself +1 token. ✓ hand-traced.

### 3.4 contains BODY `^=@~+0~<~` ≡ `1(=2)1)+0 1)<1)`, dip context `found x head`
Original:
`^`→`found x head x`; `=`(head==x=b)→`found x b`; `@`(rot)→`x b found`;
`~`(swap)→`x found b`; `+`→`x s` (s=found+b); `0`→`x s 0`; `~`→`x 0 s`;
`<`(0<s=or)→`x or`; `~`→`or x`.
Rewrite:
`1(`(over)→`found x head x`; `=`→`found x b`; `2)`(rot)→`x b found`;
`1)`(swap)→`x found b`; `+`→`x s`; `0`→`x s 0`; `1)`(swap)→`x 0 s`;
`<`→`x or`; `1)`(swap)→`or x`. Identical result `or x`. 26 tok → 33 tok. ✓ hand-traced.

### 3.5 count BODY `^=@~+~` ≡ `1(=2)1)+1)`, dip context `found x head`
Same as 3.4 through `+` giving `x s`, then final `~`/`1)`(swap)→`s x` (keeps the
running sum, no OR-clamp). Identical. 23 tok → 29 tok. ✓ hand-traced.

## 4. Where pick/roll would WIN — depth ≥ 3 (constructed, NOT in corpus)

Non-destructive copy of the depth-`d` item to the top. `pick` is 2 tokens at any
depth; pure MTL must nest dips, and the cost climbs:

| access | pick | tok | pure-MTL dip nest | tok |
|---|---|--:|---|--:|
| copy depth-1 | `1(` | 2 | `[^]'` | 2 |
| copy depth-2 | `2(` | 2 | `[[^]']'` | 5 |
| copy depth-3 | `3(` | 2 | `[[[^]']']'` | 6 |

Beyond the token gap, the dip-nest form is acutely error-prone to emit (each layer
buries one more item), which is the writability half of the win. **No `T_v0` or
`T_v0.2` task reaches depth ≥ 2 non-destructive access** — the solved corpus was
hand-designed to keep carried values shallow (TIER2_NOTES incident log #2). So this
win is real but entirely outside the current benchmark, which is why the
recommendation is *defer to the agent trial*, not *reject forever*.

## 5. Reproduce

```
cd /home/user/MTL/bench
python3 tokcount/tokcount.py '~@'                 # 2 / 2
python3 tokcount/tokcount.py '1)2)'               # 4 / 4
python3 tokcount/tokcount.py '^^'                 # 1 / 1
python3 tokcount/tokcount.py '1(1('              # 4 / 4
printf '%s' "0~@[>[;0][[]1]?][__][>_[^=@~+0~<~]'][]|"        | python3 tokcount/tokcount.py   # 26 / 26
printf '%s' "0 1)2)[>[;0][[]1]?][__][>_[1(=2)1)+0 1)<1)]'][]|" | python3 tokcount/tokcount.py # 33 / 33
```
Design-stage only; off the `bench/validate` discovery path and out of `tasks.json`.
