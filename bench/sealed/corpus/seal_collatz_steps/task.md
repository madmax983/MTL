# seal_collatz_steps

Tier: micro  |  Category: recursion

Given an integer n >= 1, repeatedly apply the Collatz map: if the current value is even, divide it by 2; if it is odd, replace it with 3*value + 1. Count how many such steps are required to first reach the value 1. If n is already 1, the answer is 0.

Signature: `f(n) -> int`. Stack input (mtlrun prefix): `1 ...` — arguments are pushed left to right, so the last argument is on top of the stack when the program starts.
