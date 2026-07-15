# seal_rle_flatten

Tier: tier3  |  Category: statemachine

Given a list of integers xs, run-length encode consecutive equal elements and return the result flattened into a single list of integers as [value1, count1, value2, count2, ...], where each (value, count) pair describes one maximal run in order. An empty input returns an empty list.

Signature: `f(xs) -> list[int]`. Stack input (mtlrun prefix): `[] ...` — arguments are pushed left to right, so the last argument is on top of the stack when the program starts.
