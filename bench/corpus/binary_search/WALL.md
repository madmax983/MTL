# binary_search — WALL (inexpressible in MTL v0.2)

Canonical solution: maintain lo/hi bounds and repeatedly probe the MIDDLE
element `xs[mid]`, halving the search interval each step.

BLOCKER: binary search is defined by O(1) random access to xs[mid]. In MTL a
quotation is a cons-list and the only deconstructor is `>` (head + tail),
which is strictly sequential — reaching the middle element costs a linear
walk, and there is no positional `index`/`nth`. A TRUE binary search
(logarithmic probes) is therefore impossible; only a linear scan is
expressible, which defeats the purpose of the task and returns the wrong
algorithmic complexity.

v0.3 PRIMITIVE THAT WOULD UNBLOCK: a random-access / indexed sequence
primitive (array-backed sequence with `index`/`nth`, or a length + positional
access pair). Shares the exact structural gap with two_sum. Unblocks 2 tasks.
