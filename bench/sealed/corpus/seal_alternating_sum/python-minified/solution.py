alternating_sum=lambda xs:sum(x*(1-2*(i%2))for i,x in enumerate(xs))
