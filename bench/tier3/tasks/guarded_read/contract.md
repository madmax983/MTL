# Tier-3 task: `guarded_read`

Part of the MTL v0.4 Tier-3 agentic suite (design: `docs/design/v0.4-effects.md` §8).
The executable program is `solution.mtl`; it is parsed by `mtl-syntax`, converted to
`mtl-core` words, and driven by the `mtl-host` runner in `crates/mtl-host/tests/tier3.rs`.

## Capabilities (declared stack effects)

```
    endp     : ( -- 0|1 )   (1 iff the read cursor is at end-of-input; never faults)
    nextline : ( -- h )      {advances the read cursor}  (faults InputClosed at EOF)
    emit     : ( h -- )   {output}
```

Capability names are lexer-safe (`[a-z][a-z0-9]*`): the `mtl-syntax` lexer reads `-`
as the `sub` prim and `?` as the `if` prim, so the design's `read-line`/`done?` are
mangled to `readline`/`donep` here.

## Executable MTL

```
[endp][][nextline emit][]|
```

## Fixture (host-owned inputs)

lines = ["x","y","z"]

## Expected result

output == "x\ny\nz\n"

## MTL adjustment vs the design sketch

None (new v0.4 task). Fault-handling by control flow: `|` is LinRec
`[P][T][R1][R2]|` — run P leaving a flag; if flag != 0 run T (terminate); else run
R1, recurse, run R2. Here P = `endp` (1 when the input is exhausted => terminate),
T = `[]`, R1 = `[nextline emit]` (read+emit one line PRE-recursion), R2 = `[]`.
Because `endp` guards every `nextline`, the faulting `nextline` is NEVER reached at
EOF. Contrast the naive unguarded over-read `4[nextline emit].` (one `nextline`
past the 3 available lines), which faults `HostFaulted(InputClosed)` — guarding is
the point of this task.
