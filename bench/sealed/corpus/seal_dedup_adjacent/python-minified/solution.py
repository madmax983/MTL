dedup_adjacent=lambda xs:[x for i,x in enumerate(xs)if i==0 or xs[i-1]!=x]
