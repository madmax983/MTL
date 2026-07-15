def dedup_adjacent(xs):
    result = []
    for x in xs:
        if not result or result[-1] != x:
            result.append(x)
    return result
