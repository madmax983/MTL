count_occurrences=lambda xs,x:0 if not xs else (1 if xs[0]==x else 0)+count_occurrences(xs[1:],x)
