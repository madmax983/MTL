# seal_dedup_adjacent

Tier: tier2  |  Category: sequence

Given a list of integers xs, return a new list that collapses each run of consecutive equal elements into a single copy of that element, preserving order. Non-adjacent duplicates are kept. An empty list returns an empty list.

Signature: `f(xs) -> list[int]`. Stack input (mtlrun prefix): `[] ...` — arguments are pushed left to right, so the last argument is on top of the stack when the program starts.
