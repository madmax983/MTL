contains=lambda xs,x:0 if not xs else (1 if xs[0]==x else contains(xs[1:],x))
