def power(b, e):
    result = 1
    for _ in range(e):
        result *= b
    return result
