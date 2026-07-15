max_adjacent_diff=lambda xs:max((abs(xs[i]-xs[i-1])for i in range(1,len(xs))),default=0)
