def rle_flatten(xs):
    result = []
    for x in xs:
        if result and result[-2] == x:
            result[-1] += 1
        else:
            result += [x, 1]
    return result
