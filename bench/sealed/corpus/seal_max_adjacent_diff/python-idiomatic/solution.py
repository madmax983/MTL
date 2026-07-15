def max_adjacent_diff(xs):
    if len(xs) < 2:
        return 0
    return max(abs(xs[i] - xs[i - 1]) for i in range(1, len(xs)))
