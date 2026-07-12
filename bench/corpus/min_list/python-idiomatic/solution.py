def min_list(xs):
    m = xs[0]
    for x in xs[1:]:
        if x < m:
            m = x
    return m
