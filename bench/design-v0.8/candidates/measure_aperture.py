#!/usr/bin/env python3
"""v0.8 CANDIDATES — aperture-combinator token-delta measurement.

Hand-rewrites the juggle-heavy scan solutions using a proposed windowed-fold
("aperture") combinator and measures the o200k/cl100k token delta vs the
current juggle-heavy programs. Also counts JUGGLE glyphs before/after to test
the #41 hypothesis (does the aperture REMOVE juggle glyphs or RELOCATE them?).

These MTL rewrites are HAND-TRACED, not interpreter-verified (the aperture
primitive is not implemented in the verified core; this round measures token
deltas by hand-rewriting, per the decision-record protocol). Each rewrite ships
a hand-trace comment justifying correctness on a representative window.

Run from repo root:  python3 bench/design-v0.8/candidates/measure_aperture.py
"""
import sys
from pathlib import Path
REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "bench" / "tokcount"))
from tokcount import count

JUGGLE = set(":~^_@'")  # dup swap over drop rot dip


def toks(s):
    c = count(s)
    return c["o200k_base"], c["cl100k_base"]


def jug(s):
    return sum(1 for ch in s if ch in JUGGLE)


# -- current juggle-heavy programs (verbatim from families.rs) --------------
BASE = {
    "alt_sum": "[>0=][0][][-]|",
    "local_maxima": "[:>[~_][[]]?>[~_][[]]?>[__0][1]?][_0][:>_>_>__^<@@<*~>_~_][+]|",
    "max_adj_diff": "[:>[~_][[]]?>[__0][1]?][_0][:>_>__-:0<[0~-][]?~>_~_][^^<[~_][_]?]|",
    "dedup_adj": "[][^>[_^=][0]?[_][~;]?]([][~;](",
    "rle_flatten": "[][~>[@:@:>__~[=]'~[~_~1+~;][~[;]'~;1~;]?][[];1~;]?]([][~;](",
    "min_running_balance": "^~[>0=][_][[+:[^^<[_][~_]?]']']|",
}

# -- aperture rewrites (parameterized: [xs] acc0 k [C] w) --------------------
# The window combinator hands C the k consecutive elements of the sliding
# window (advance-by-1) plus the accumulator below them; C: [acc]+[e0..e_{k-1}]
# -> [acc]. Short lists (< k) yield acc0 untouched.
APERTURE_PARAM = {
    # local_maxima, k=3: C takes acc a b c -> acc + (b>a && b>c).
    #   ^  over(b over a)? trace: acc a b c
    #   ^  -> acc a b c b      < -> acc a b (c<b)=(b>c)=p1
    #   @  -> acc b p1 a       @ -> acc p1 a b   < -> acc p1 (a<b)=(b>a)=p2
    #   *  -> acc (p1*p2)      + -> acc'
    "local_maxima": "0 3[^<@@<*+]w",
    # max_adj_diff, k=2: C takes acc a b -> max(acc,|a-b|). acc0=0 valid
    #   (all diffs >=0; short list -> 0).  - : abs(:0<[0~-][]?) : max(^^<[~_][_]?)
    "max_adj_diff": "0 2[-:0<[0~-][]?^^<[~_][_]?]w",
    # dedup_adj, k=2 window emitting b when b!=a; needs first-elem seed + reverse.
    #   This is element-vs-accumulator, NOT a clean fixed window; the seed of the
    #   first element and the final reverse re-introduce the fold machinery, so
    #   the aperture barely helps. Best honest attempt (seed = uncons first elem):
    #   see note in report — measured as a fold-with-window, marginal.
    "dedup_adj": "[>~]0 2[^=[_][~;]?]w",  # HAND-APPROX, see report caveat
}

# -- aperture rewrites (fixed-width single glyph, no literal-k) --------------
# Two glyphs: '#' = window-3 fold, '`' = window-2 fold.  No literal-k operand.
APERTURE_FIXED = {
    "local_maxima": "0[^<@@<*+]#",
    "max_adj_diff": "0[-:0<[0~-][]?^^<[~_][_]?]`",
}


def report():
    print("=== BASELINE juggle-heavy scan programs ===")
    print(f"{'task':22} {'o200k':>6} {'cl100k':>7} {'juggle':>7} {'chars':>6}")
    btot_o = btot_c = bjug = 0
    for k, v in BASE.items():
        o, c = toks(v)
        j = jug(v)
        btot_o += o
        btot_c += c
        bjug += j
        print(f"{k:22} {o:6} {c:7} {j:7} {len(v):6}")
    print(f"{'TOTAL':22} {btot_o:6} {btot_c:7} {bjug:7}")

    print("\n=== APERTURE (parameterized: [xs] acc0 k [C] w) ===")
    print(f"{'task':22} {'prog':38} {'o200k':>6} {'cl100k':>7} {'jug':>4}")
    for k, v in APERTURE_PARAM.items():
        o, c = toks(v)
        bo, bc = toks(BASE[k])
        print(f"{k:22} {v:38} {o:6} {c:7} {jug(v):4}   "
              f"(base {bo}/{bc}, delta {o-bo:+d}/{c-bc:+d}, jug {jug(BASE[k])}->{jug(v)})")

    print("\n=== APERTURE (fixed-width single glyph, no literal-k) ===")
    for k, v in APERTURE_FIXED.items():
        o, c = toks(v)
        bo, bc = toks(BASE[k])
        print(f"{k:22} {v:38} {o:6} {c:7} {jug(v):4}   "
              f"(base {bo}/{bc}, delta {o-bo:+d}/{c-bc:+d})")

    # Projected scan-family totals (o200k) under fixed-width aperture, using the
    # aperture rewrite where it applies and the baseline elsewhere.
    print("\n=== PROJECTED scan-family o200k totals ===")
    apply_fixed = {"local_maxima": APERTURE_FIXED["local_maxima"],
                   "max_adj_diff": APERTURE_FIXED["max_adj_diff"]}
    apply_param = {"local_maxima": APERTURE_PARAM["local_maxima"],
                   "max_adj_diff": APERTURE_PARAM["max_adj_diff"]}
    for label, mp in [("baseline", {}), ("fixed-glyph aperture", apply_fixed),
                      ("param-k aperture", apply_param)]:
        tot = 0
        for k, v in BASE.items():
            prog = mp.get(k, v)
            tot += toks(prog)[0]
        print(f"  {label:24} scan-family o200k SUM = {tot}")


if __name__ == "__main__":
    report()
