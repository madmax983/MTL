# single_number — WALL (was inexpressible through MTL v0.2)

## RESOLVED in v0.3

This task is **no longer a wall.** MTL v0.3 adds a `$` xor primitive, which
collapses the canonical XOR-reduce into a single fold. The interpreter-validated
solution is `[>0=][0][][$]|` (**9 tokens**), validated by
`bench/validate/tests/tier2_v03.rs`. Everything below is retained as historical
context describing why the task was a wall through v0.2.

---

Canonical solution: XOR-reduce the list (a ^ a == 0, so all paired elements
cancel and the lone element remains).

BLOCKER: there is NO bitwise primitive in the 21-primitive v0.2 set. The
glyphs that look bitwise are taken by other operations — verified on the
interpreter: `^` = Over (`5 3^` -> `5 3 5`), `&` = PrimRec, `|` = LinRec.
There is no AND / OR / XOR / shift anywhere in the primitive table, and Values
are only Int | Quote (no bit access). The canonical algorithm is therefore
inexpressible. (A non-canonical count-based workaround would need an
associative map, which is also absent — see two_sum.)

v0.3 PRIMITIVE THAT WOULD UNBLOCK: a bitwise XOR primitive (and ideally the
AND / OR / shift family). With `xor` the whole task collapses to a single
PrimRec/LinRec fold `[0][xor]|` — mirroring the ~5-glyph arithmetic folds.
Unblocks 1 task directly but an entire problem class (bit manipulation).
