#!/usr/bin/env python3
"""Validate the hand-written aperture rewrites against the datagen oracle.

A minimal concatenative stack machine implementing exactly the glyphs used in
the aperture rewrites (: ~ ^ _ @ < = * - + [ ] ?) PLUS a proposed windowed-fold
combinator `w` ([xs] acc0 k [C] w) with advance-by-1 sliding-window semantics.
We run each rewrite over the SAME scan_lists grid + reference functions as
families.rs and assert equality. If a rewrite is wrong, this prints a MISMATCH
(the whole point: catch a bad hand-trace before quoting its token count).
"""

# ---- oracle reference functions (verbatim logic from families.rs) ----------
def ref_local_maxima(l):
    c = 0
    if len(l) >= 3:
        for i in range(1, len(l) - 1):
            if l[i] > l[i - 1] and l[i] > l[i + 1]:
                c += 1
    return c


def ref_max_adj_diff(l):
    if len(l) < 2:
        return 0
    return max(abs(l[i] - l[i - 1]) for i in range(1, len(l)))


def ref_dedup(l):
    r = []
    for x in l:
        if not r or r[-1] != x:
            r.append(x)
    return r


SCAN_LISTS = [
    [], [5], [3, 8], [1, 2, 3, 4], [4, 3, 2, 1], [3, 1, 4, 1, 5, 9, 2],
    [-5, -2, -8, -1], [7, 7, 7], [2, 2, 3, 3, 3, 1], [0, 0, 0],
    [-3, 5, -3, 5], [10, -10, 10, -10, 10],
]


# ---- tokenizer for the tiny MTL fragment -----------------------------------
class Quote(list):
    pass


def tokenize(src):
    toks, i = [], 0
    while i < len(src):
        ch = src[i]
        if ch.isspace():
            i += 1
            continue
        if ch == '[':
            depth, j = 1, i + 1
            while j < len(src) and depth:
                if src[j] == '[':
                    depth += 1
                elif src[j] == ']':
                    depth -= 1
                j += 1
            toks.append(Quote(tokenize(src[i + 1:j - 1])))
            i = j
            continue
        if ch.isdigit():
            j = i
            while j < len(src) and src[j].isdigit():
                j += 1
            toks.append(int(src[i:j]))
            i = j
            continue
        toks.append(ch)
        i += 1
    return toks


# ---- interpreter -----------------------------------------------------------
def run(prog, stack):
    for t in prog:
        if isinstance(t, Quote) or isinstance(t, int):
            stack.append(t)
        elif t == ':':
            stack.append(stack[-1])
        elif t == '_':
            stack.pop()
        elif t == '~':
            a, b = stack[-2], stack[-1]
            stack[-2], stack[-1] = b, a
        elif t == '^':
            stack.append(stack[-2])
        elif t == '@':               # a b c -> b c a
            c = stack.pop(); b = stack.pop(); a = stack.pop()
            stack += [b, c, a]
        elif t == '-':
            b = stack.pop(); a = stack.pop(); stack.append(a - b)
        elif t == '*':
            b = stack.pop(); a = stack.pop(); stack.append(a * b)
        elif t == '+':
            b = stack.pop(); a = stack.pop(); stack.append(a + b)
        elif t == '<':
            b = stack.pop(); a = stack.pop(); stack.append(1 if a < b else 0)
        elif t == '=':
            b = stack.pop(); a = stack.pop(); stack.append(1 if a == b else 0)
        elif t == '?':
            f = stack.pop(); tt = stack.pop(); c = stack.pop()
            run(tt if c else f, stack)
        elif t == ';':               # cons: q x -> q'  (append x to quote)
            x = stack.pop(); q = stack.pop(); stack.append(Quote(q + [x]))
        else:
            raise ValueError(f"unhandled token {t!r}")
    return stack


def aperture(xs, acc0, k, C):
    """[xs] acc0 k [C] w — sliding window width k, advance 1."""
    acc = acc0
    for i in range(0, len(xs) - k + 1):
        window = xs[i:i + k]
        st = [acc] + list(window)
        run(C, st)
        assert len(st) == 1, f"C must leave 1 cell, left {st}"
        acc = st[0]
    return acc


# ---- the rewrites under test ----------------------------------------------
LM_C = tokenize("^<@@<*+")
MAD_C = tokenize("-:0<[0~-][]?^^<[~_][_]?]".replace(']', '', 1))  # body only
# safer: parse bodies directly
LM_C = tokenize("^<@@<*+")
MAD_C = tokenize("-:0<[0~-][]?^^<[~_][_]?")


def rw_local_maxima(l):
    return aperture(l, 0, 3, LM_C)


def rw_max_adj_diff(l):
    return aperture(l, 0, 2, MAD_C)


def check(name, ref, rw):
    ok = True
    for l in SCAN_LISTS:
        want = ref(l)
        try:
            got = rw(l)
        except Exception as e:
            got = f"ERROR:{e}"
        if got != want:
            ok = False
            print(f"  MISMATCH {name} on {l}: want {want} got {got}")
    print(f"{name:16} {'PASS' if ok else 'FAIL'}")
    return ok


if __name__ == "__main__":
    a = check("local_maxima", ref_local_maxima, rw_local_maxima)
    b = check("max_adj_diff", ref_max_adj_diff, rw_max_adj_diff)
    print("\nALL PASS" if (a and b) else "\nSOME FAILED")
