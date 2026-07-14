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
