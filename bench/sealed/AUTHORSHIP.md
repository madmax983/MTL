# Authorship Provenance — Sealed Task Set

## Information barrier followed
I authored these 15 sealed tasks from task semantics alone. I did NOT read, open, grep,
or otherwise inspect the MTL specification (`docs/mtl-spec.md`), the quick reference,
any glyph/primitive/operator documentation, the benchmark corpus
(`bench/corpus/**`), the agent-trial solutions (`bench/agent-trial/**`), or any `*.mtl`
file. I have no knowledge of which operators exist in the target language or which are
considered "cheap," and I did not tailor any task toward a language feature. The only
metadata I consulted was the provided list of existing dev task *names* (for
de-duplication), never the files behind them.

## Semantic categories drawn from
Tasks were designed purely from standard, language-agnostic computational primitives:
digit-based arithmetic (digit product, digit-sum in an arbitrary base), integer number
theory (integer square root, divisor counting, triangular numbers), bit-level counting
(population count), bounded iterative recursion (Collatz stopping time), sequence folds
and reductions (alternating sum, XOR reduce), prefix/running aggregates (running max,
minimum running balance), sequence predicates and scanning (local maxima count, max
adjacent difference), and stateful sequence transformations (adjacent dedup, run-length
encoding flattened to an int list).

## Structural novelty vs the avoid-list
Each task was checked against the avoid-list (affine, rev3, is_even, factorial, gcd,
sum_list, reverse_list, palindrome_number, contains, climbing_stairs, fib, sum_to,
power, two_sum, binary_search, single_number, max_of_list). None is a rename or minor
mutation of those. Where a superficial neighbor existed (e.g. XOR reduce vs
single_number), I chose a general fold with distinct semantics and edge behavior rather
than the special-case problem. Tasks are also mutually distinct in shape across the
three tiers.

## Ground-truth verification
I recomputed every `expected` value by hand under each task's stated prompt, giving
particular attention to edge cases: empty lists, single-element lists, zero, negatives,
absolute-value handling, and base conversions. All vectors were confirmed consistent
with their specifications.
