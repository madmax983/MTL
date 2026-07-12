two_sum=lambda xs,t:next(([j,i] for i,x in enumerate(xs) for j in range(i) if xs[j]+x==t),[])
