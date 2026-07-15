def solve(n):
    if n < 2:
        return n
    r = int(n ** 0.5)
    while r * r > n:
        r -= 1
    while (r + 1) * (r + 1) <= n:
        r += 1
    return r
