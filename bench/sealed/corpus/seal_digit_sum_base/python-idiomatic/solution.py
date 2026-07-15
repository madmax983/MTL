def digit_sum_base(n, b):
    total = 0
    while n > 0:
        total += n % b
        n //= b
    return total
