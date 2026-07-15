def solve(n, b):
    if n == 0:
        return 0
    total = 0
    while n > 0:
        total += n % b
        n = n // b
    return total
