rle_flatten=lambda xs:[y for k,g in __import__('itertools').groupby(xs)for y in(k,len(list(g)))]
