def solve(n):
    if n < 2:
        return n
    r = 0
    bit = 1
    while bit * 4 <= n:
        bit *= 4
    while bit != 0:
        if n >= r + bit:
            n -= r + bit
            r = (r >> 1) + bit
        else:
            r >>= 1
        bit >>= 2
    return r
