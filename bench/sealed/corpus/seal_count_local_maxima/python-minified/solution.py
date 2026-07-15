count_local_maxima=lambda xs:sum(xs[i]>xs[i-1]and xs[i]>xs[i+1] for i in range(1,len(xs)-1))
