def alternating_sum(xs):
    total = 0
    for i, x in enumerate(xs):
        total += x if i % 2 == 0 else -x
    return total
