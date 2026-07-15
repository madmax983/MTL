def num_divisors(n):
    count = 0
    for d in range(1, n + 1):
        if n % d == 0:
            count += 1
    return count
