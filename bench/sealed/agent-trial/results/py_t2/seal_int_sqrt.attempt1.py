def solve(n):
    if n < 2:
        return n
    lo, hi = 0, n
    while lo <= hi:
        mid = (lo + hi) // 2
        if mid * mid <= n:
            lo = mid + 1
        else:
            hi = mid - 1
    return hi
