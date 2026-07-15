# seal_xor_reduce

Tier: tier2  |  Category: fold

Given a list of non-negative integers xs, return the bitwise XOR of all elements, folded left to right. The XOR of an empty list is 0. (All inputs are non-negative so bit patterns are unambiguous.)

Signature: `f(xs) -> int`. Stack input (mtlrun prefix): `[] ...` — arguments are pushed left to right, so the last argument is on top of the stack when the program starts.
