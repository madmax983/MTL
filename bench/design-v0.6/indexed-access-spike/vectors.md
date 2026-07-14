# I/O vectors used to validate the option-(a) programs (interpreter-checked).
# Stack input order: list pushed first, target on top.  Result = top of final stack.

## binary_search  (xs sorted, returns index or -1)
[1 3 5 7 9] 1 -> 0
[1 3 5 7 9] 3 -> 1
[1 3 5 7 9] 5 -> 2
[1 3 5 7 9] 7 -> 3
[1 3 5 7 9] 9 -> 4
[1 3 5 7 9] 4 -> -1     # absent (interior)
[1 3 5 7 9] 0 -> -1     # absent (below)
[1 3 5 7 9] 10 -> -1    # absent (above)
[5] 5 -> 0              # singleton hit
[5] 3 -> -1             # singleton miss
[2 4 6 8] 2 -> 0        # even length
[2 4 6 8] 8 -> 3
[2 4 6 8] 5 -> -1

## two_sum  (returns [i j], i<j, first pair in row-major k order)
[2 7 11 15] 9 -> [0 1]
[3 2 4] 6 -> [1 2]
[1 3 5 7 9] 12 -> [1 4]   # (1,4) precedes (2,3) in scan order
[3 3] 6 -> [0 1]          # duplicate values
[0 4 3 0] 0 -> [0 3]      # zeros
[5 75 25] 100 -> [1 2]
