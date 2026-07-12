def solve(n):
    r = 1
    for k in range(2, n + 1):
        r *= k
    return r
