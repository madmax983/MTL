def int_sqrt(n):
    r = 0
    while (r + 1) * (r + 1) <= n:
        r += 1
    return r
