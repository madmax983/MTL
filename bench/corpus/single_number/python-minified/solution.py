single_number=lambda xs:0 if not xs else xs[0]^single_number(xs[1:])
