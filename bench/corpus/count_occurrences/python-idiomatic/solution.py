def count_occurrences(xs, x):
    c = 0
    for e in xs:
        if e == x:
            c += 1
    return c
