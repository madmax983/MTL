def solve():
    for _ in range(3):
        r = try_op()
        if ok(r):
            return r
    return None
