def reverse_list(xs):
    r = []
    for x in xs:
        r = [x] + r
    return r
