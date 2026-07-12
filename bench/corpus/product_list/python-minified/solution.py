product_list=lambda xs:1 if not xs else xs[0]*product_list(xs[1:])
