# seal_alternating_sum

Tier: tier2  |  Category: fold

Given a list of integers xs, return xs[0] - xs[1] + xs[2] - xs[3] + ... , alternating signs starting with a plus on the first element (index 0 is added, index 1 is subtracted, index 2 is added, and so on). For an empty list the result is 0.

Signature: `f(xs) -> int`. Stack input (mtlrun prefix): `[] ...` — arguments are pushed left to right, so the last argument is on top of the stack when the program starts.
