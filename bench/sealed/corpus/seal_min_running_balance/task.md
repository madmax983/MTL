# seal_min_running_balance

Tier: tier3  |  Category: statemachine

A balance starts at the integer value start. Process the list of integer deltas xs in order, adding each delta to the running balance. Return the minimum balance ever observed, where the initial value start counts as an observation before any delta is applied. If xs is empty the answer is start.

Signature: `f(start, xs) -> int`. Stack input (mtlrun prefix): `0 [] ...` — arguments are pushed left to right, so the last argument is on top of the stack when the program starts.
