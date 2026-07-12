max_list=lambda xs:xs[0] if not xs[1:] else (lambda m:xs[0] if xs[0]>m else m)(max_list(xs[1:]))
