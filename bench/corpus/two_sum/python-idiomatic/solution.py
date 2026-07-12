def two_sum(xs, target):
    seen = {}
    for i, x in enumerate(xs):
        if target - x in seen:
            return [seen[target - x], i]
        seen[x] = i
    return []
