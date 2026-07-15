def solve(xs):
    count = 0
    i = 1
    while i < len(xs) - 1:
        if xs[i] > xs[i - 1] and xs[i] > xs[i + 1]:
            count += 1
        i += 1
    return count
