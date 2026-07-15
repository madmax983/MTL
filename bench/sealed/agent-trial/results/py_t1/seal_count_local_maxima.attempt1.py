def solve(xs):
    count = 0
    for i in range(1, len(xs) - 1):
        if xs[i] > xs[i - 1] and xs[i] > xs[i + 1]:
            count += 1
    return count
