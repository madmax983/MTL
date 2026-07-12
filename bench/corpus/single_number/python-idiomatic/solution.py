def single_number(xs):
    r = 0
    for x in xs:
        r ^= x
    return r
