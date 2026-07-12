# two_sum — WALL (inexpressible in MTL v0.2)

Canonical solution: one pass with a hash map from value -> index, returning
the pair of INDICES whose values sum to the target.

BLOCKER: returning indices requires positional information the language cannot
produce. A quotation is a cons-list whose only deconstructor is `>` (uncons =
head + tail), strictly sequential — there is no `enumerate`, no zip-with-index,
no positional `index`/`nth`, and no associative map. Values are only Int |
Quote, so a running index cannot be attached to elements without a
random-access or key/value structure. The value-level "does any pair sum to
t?" predicate is expressible (nested scan), but recovering the two INDICES is
not.

v0.3 PRIMITIVE THAT WOULD UNBLOCK: a random-access / indexed sequence
primitive (`index`/`nth`, or an enumerate combinator that pairs each element
with its position). This is the biggest STRUCTURAL gap: a cons-list simply
cannot index in O(1). Unblocks 2 tasks (two_sum, binary_search).
