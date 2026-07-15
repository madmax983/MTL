<!-- Provenance: frozen winner of the cold-preamble ablation (PR #88, issue #73).
     Source: bench/agent-trial/preamble/variants/v4_compressed_minimal.md
     (variant v4_compressed_minimal, 487 o200k tokens, 100% solve on the 10 pure
     tasks). The primitive reference below is byte-identical to that frozen
     variant; this header and the division-of-labor note are the only wrapper. -->

> **Division of labor.** `docs/mtl-quickref-min.md` (this file) = **pure
> computation** only. `docs/mtl-quickref.md` (full) = **pure computation + host
> capabilities**. This minimal reference deliberately omits the Host-capabilities
> section; any task that reaches the host (Tier-3 I/O, tools, grants, budgets)
> needs the full [`docs/mtl-quickref.md`](mtl-quickref.md).
>
> **Negative inputs.** Literals are unsigned (`-` is always Sub). A negative
> *constant* in program text is `0 N -`; negative or list *inputs* are supplied
> via `mtlrun --input '<spec>'` (e.g. `-24`, `[-5 -2 -8 -1]`) — a harness
> decoder, not a language change.

# MTL primitives (stack top at right; `[q]`=quote/list; 0=false; `-` is always Sub)

```
[ ]  quote ( -- [q] )
:    dup   ( a -- a a )
_    drop  ( a -- )
~    swap  ( a b -- b a )
@    rot   ( a b c -- b c a )
^    over  ( a b -- a b a )
!    apply ( [q] -- ... )
,    cat   ( [a] [b] -- [ab] )
;    cons  ( v [q] -- [v q] )
'    dip   ( a [q] -- ... a )
+    add   ( a b -- a+b )
-    sub   ( a b -- a-b )
*    mul   ( a b -- a*b )
/    div   ( a b -- a/b )   trunc toward 0; b=0 faults
%    mod   ( a b -- a%b )   trunc; sign of dividend; b=0 faults
=    eq    ( a b -- 0|1 )
<    lt    ( a b -- 0|1 )
?    if    ( c [t] [f] -- ... )   run [t] if c!=0 else [f]
&    primrec ( n [I] [C] -- r )   n<=0:I; else recurse n-1 then C sees (n r)
.    times ( n [Q] -- ... )   run [Q] max(n,0) times
|    linrec ( [P][T][R1][R2] -- ... )   P;flag!=0:T else R1,recurse,R2
>    uncons ( [w t] -- w [t] 1 ) or ( [] -- 0 )
(    fold  ( [seq] init [C] -- r )   C:( acc w -- acc' ) per element L->R
$    xor   ( a b -- a$b )   bitwise i64
```

Faults (halt, no partial result): Underflow, TypeMismatch, Overflow, DivByZero (also FuelExhausted). Precedence: arity, then types, then DivByZero/Overflow.
