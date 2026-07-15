# seal_count_local_maxima

Tier: tier2  |  Category: sequence

Given a list of integers xs, count the number of interior indices i (with 0 < i < len(xs)-1) that are strict local maxima, meaning xs[i] > xs[i-1] AND xs[i] > xs[i+1]. First and last elements are never counted. Lists of length 0, 1, or 2 always yield 0.

Signature: `f(xs) -> int`. Stack input (mtlrun prefix): `[] ...` — arguments are pushed left to right, so the last argument is on top of the stack when the program starts.
