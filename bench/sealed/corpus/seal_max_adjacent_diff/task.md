# seal_max_adjacent_diff

Tier: tier2  |  Category: sequence

Given a list of integers xs, return the maximum over all adjacent pairs of the absolute difference |xs[i] - xs[i-1]| for i from 1 to len(xs)-1. If the list has fewer than 2 elements, return 0.

Signature: `f(xs) -> int`. Stack input (mtlrun prefix): `[] ...` — arguments are pushed left to right, so the last argument is on top of the stack when the program starts.
