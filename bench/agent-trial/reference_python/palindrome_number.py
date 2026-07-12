def solve(n):
    s = str(n)
    return 1 if s == s[::-1] else 0
