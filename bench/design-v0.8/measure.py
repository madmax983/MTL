#!/usr/bin/env python3
"""v0.8 broad-distribution compression measurement.

Reads the distinct task shapes emitted by `bench/dataset/src/bin/broad.rs`
(`broad_shapes.json`), renders a fair *idiomatic* (not code-golfed) Python
reference per shape from a per-family TEMPLATE, tokenizes BOTH the MTL program
and the Python reference with the SAME o200k tokenizer used everywhere in the
bench (`bench/tokcount`), and reports per-family + overall token-SUM
compression ratios, split into TRAIN and DEV.

METHOD LABELS per number:
  * MTL side  -> "synth"    (oracle-verified datagen candidate program)
  * Py  side  -> "template" (rendered from the per-family idiomatic template)
No "hand" numbers are used in this run; the column exists for provenance.

Fairness bar mirrors `bench/design-v0.2/python/*-idiomatic.py`: a real `def`
with a signature, a natural body (builtins where a Python programmer would
reach for them, explicit loops otherwise), one statement per line. The scan
templates are byte-identical to the sealed `python-idiomatic` references.

Run from the repo root:
    python3 bench/design-v0.8/measure.py
"""

from __future__ import annotations

import json
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
sys.path.insert(0, str(REPO / "bench" / "tokcount"))
from tokcount import count  # noqa: E402  (same encoder as the whole bench)

ENC = "o200k_base"
SHAPES = HERE / "broad_shapes.json"


# --------------------------------------------------------------------------
# Per-family idiomatic-Python templates. Each takes the shape's int args and
# returns Python source (no trailing newline; the tokenizer policy strips one).
# --------------------------------------------------------------------------
def _fixed(src: str):
    return lambda args: src


TEMPLATES = {
    # ---- tier-0 scalar arithmetic ----
    "affine": lambda a: f"def affine(n):\n    return {a[0]} * n + {a[1]}",
    "square": lambda a: f"def square(n):\n    return n * n + {a[0]}",
    "lincomb2": lambda a: f"def lincomb2(x, y):\n    return {a[0]} * y + {a[1]} * x",
    "binop_add": _fixed("def add(x, y):\n    return x + y"),
    "binop_sub": _fixed("def sub(x, y):\n    return x - y"),
    "binop_mul": _fixed("def mul(x, y):\n    return x * y"),
    "binop_div": _fixed("def div(x, y):\n    return int(x / y)"),
    "binop_mod": _fixed("def mod(x, y):\n    return x - y * int(x / y)"),
    # ---- predicates ----
    "is_zero": _fixed("def is_zero(n):\n    return 1 if n == 0 else 0"),
    "is_neg": _fixed("def is_negative(n):\n    return 1 if n < 0 else 0"),
    "is_pos": _fixed("def is_positive(n):\n    return 1 if n > 0 else 0"),
    "is_even": _fixed("def is_even(n):\n    return 1 if n % 2 == 0 else 0"),
    "eq_k": lambda a: f"def equals(n):\n    return 1 if n == {a[0]} else 0",
    "lt_k": lambda a: f"def less_than(n):\n    return 1 if n < {a[0]} else 0",
    "eq2": _fixed("def equals(x, y):\n    return 1 if x == y else 0"),
    "lt2": _fixed("def less_than(x, y):\n    return 1 if x < y else 0"),
    # ---- stack shuffles (concatenative combinators as tuple ops) ----
    "shuffle_dup": _fixed("def dup(a):\n    return a, a"),
    "shuffle_drop2": _fixed("def drop(a, b):\n    return a"),
    "shuffle_swap": _fixed("def swap(a, b):\n    return b, a"),
    "shuffle_over": _fixed("def over(a, b):\n    return a, b, a"),
    "shuffle_nip": _fixed("def nip(a, b):\n    return b"),
    "shuffle_rot3": _fixed("def rot(a, b, c):\n    return b, c, a"),
    "shuffle_rev3": _fixed("def rev(a, b, c):\n    return c, b, a"),
    "shuffle_triple": _fixed("def triple(a):\n    return a, a, a"),
    # ---- recursion / iteration ----
    "factorial": _fixed(
        "def factorial(n):\n    result = 1\n    for i in range(1, n + 1):\n"
        "        result *= i\n    return result"
    ),
    "sum_to": _fixed(
        "def sum_to(n):\n    total = 0\n    for i in range(1, n + 1):\n"
        "        total += i\n    return total"
    ),
    "fib": _fixed(
        "def fib(n):\n    a, b = 0, 1\n    for _ in range(n):\n"
        "        a, b = b, a + b\n    return a"
    ),
    "gcd": _fixed("def gcd(a, b):\n    while b:\n        a, b = b, a % b\n    return a"),
    "power": _fixed(
        "def power(b, e):\n    result = 1\n    for _ in range(e):\n"
        "        result *= b\n    return result"
    ),
    "times_mul": lambda a: (
        f"def times(n):\n    total = 0\n    for _ in range(n):\n"
        f"        total += {a[0]}\n    return total"
    ),
    # ---- fold / traversal ----
    "list_sum": _fixed("def list_sum(xs):\n    return sum(xs)"),
    "list_product": _fixed(
        "def list_product(xs):\n    result = 1\n    for x in xs:\n"
        "        result *= x\n    return result"
    ),
    "list_length": _fixed("def list_length(xs):\n    return len(xs)"),
    "list_max": _fixed("def list_max(xs):\n    return max(xs)"),
    "list_min": _fixed("def list_min(xs):\n    return min(xs)"),
    "list_xor": _fixed(
        "def list_xor(xs):\n    result = 0\n    for x in xs:\n"
        "        result ^= x\n    return result"
    ),
    "list_reverse": _fixed("def list_reverse(xs):\n    return xs[::-1]"),
    # ---- quotation glyph tasks ----
    "xor2": _fixed("def xor(a, b):\n    return a ^ b"),
    "apply_add_k": lambda a: f"def add_k(n):\n    return n + {a[0]}",
    "dip_add_k": lambda a: f"def dip_add_k(x, y):\n    return x + {a[0]}, y",
    "cons_k": lambda a: f"def cons_k(n):\n    return [n, {a[0]}]",
    "append_k": lambda a: f"def append_k(xs):\n    return xs + [{a[0]}]",
    "cat2": _fixed("def cat(xs, ys):\n    return xs + ys"),
    # ---- v0.8 bit/digit scalar ops ----
    "popcount": _fixed('def popcount(n):\n    return bin(abs(n)).count("1")'),
    "digit_sum_base": lambda a: (
        f"def digit_sum_base(n):\n    n = abs(n)\n    total = 0\n"
        f"    while n > 0:\n        total += n % {a[0]}\n        n //= {a[0]}\n"
        f"    return total"
    ),
    "digit_product_base": lambda a: (
        f"def digit_product_base(n):\n    if n == 0:\n        return 0\n"
        f"    n = abs(n)\n    product = 1\n    while n > 0:\n"
        f"        product *= n % {a[0]}\n        n //= {a[0]}\n    return product"
    ),
    # ---- v0.8 list-scan / control-flow ops (identical to sealed idiomatic) ----
    "alt_sum": _fixed(
        "def alternating_sum(xs):\n    total = 0\n    for i, x in enumerate(xs):\n"
        "        total += x if i % 2 == 0 else -x\n    return total"
    ),
    "local_maxima": _fixed(
        "def count_local_maxima(xs):\n    count = 0\n"
        "    for i in range(1, len(xs) - 1):\n"
        "        if xs[i] > xs[i - 1] and xs[i] > xs[i + 1]:\n"
        "            count += 1\n    return count"
    ),
    "max_adj_diff": _fixed(
        "def max_adjacent_diff(xs):\n    if len(xs) < 2:\n        return 0\n"
        "    return max(abs(xs[i] - xs[i - 1]) for i in range(1, len(xs)))"
    ),
    "dedup_adj": _fixed(
        "def dedup_adjacent(xs):\n    result = []\n    for x in xs:\n"
        "        if not result or result[-1] != x:\n            result.append(x)\n"
        "    return result"
    ),
    "rle_flatten": _fixed(
        "def rle_flatten(xs):\n    result = []\n    for x in xs:\n"
        "        if result and result[-2] == x:\n            result[-1] += 1\n"
        "        else:\n            result += [x, 1]\n    return result"
    ),
    "min_running_balance": _fixed(
        "def min_running_balance(start, xs):\n    balance = start\n"
        "    lowest = start\n    for delta in xs:\n        balance += delta\n"
        "        lowest = min(lowest, balance)\n    return lowest"
    ),
}


def toks(text: str) -> int:
    return count(text)[ENC]


def ratio(py: int, mtl: int) -> float:
    return py / mtl if mtl else 0.0


def main() -> int:
    data = json.loads(SHAPES.read_text())
    shapes = data["shapes"]

    missing = sorted({s["template_key"] for s in shapes} - set(TEMPLATES))
    if missing:
        print(f"FATAL: no Python template for keys: {missing}", file=sys.stderr)
        return 1

    # accumulate py/mtl token sums per (family, split) and per-key.
    fam = defaultdict(lambda: {"train": [0, 0, 0], "dev": [0, 0, 0]})  # [py,mtl,n]
    overall = {"train": [0, 0, 0], "dev": [0, 0, 0]}
    per_key = defaultdict(lambda: {"train": [0, 0, 0], "dev": [0, 0, 0]})

    for s in shapes:
        py_src = TEMPLATES[s["template_key"]](s["args"])
        p, m = toks(py_src), toks(s["program"])
        sp = s["split"]
        for bucket in (fam[s["family"]][sp], overall[sp], per_key[s["template_key"]][sp]):
            bucket[0] += p
            bucket[1] += m
            bucket[2] += 1

    def fam_ratio(f, sp):
        py, mtl, n = fam[f][sp]
        return py, mtl, n, ratio(py, mtl)

    print("# v0.8 BROAD-DISTRIBUTION compression (o200k, token-SUM)")
    print(f"# {data['distinct_shapes']} distinct shapes; MTL=synth, Py=template")
    print()
    print("## Per-family token-SUM ratio (py_template / mtl_synth)")
    print()
    print("| family | n(tr) | py(tr) | mtl(tr) | ratio(tr) | n(dv) | py(dv) | mtl(dv) | ratio(dv) |")
    print("|---|--:|--:|--:|--:|--:|--:|--:|--:|")
    for f in sorted(fam):
        pt, mt, nt, rt = fam_ratio(f, "train")
        pd, md, nd, rd = fam_ratio(f, "dev")
        print(f"| {f} | {nt} | {pt} | {mt} | {rt:.2f}x | {nd} | {pd} | {md} | {rd:.2f}x |")

    # macro average (mean of per-family ratios; families weighted equally)
    def macro(sp):
        rs = [fam_ratio(f, sp)[3] for f in fam if fam[f][sp][2] > 0]
        return sum(rs) / len(rs) if rs else 0.0

    # capped micro: cap each family at CAP shapes so no single family (arithmetic
    # emits ~968 near-identical affine one-liners) dominates the token-SUM. Shapes
    # are taken in the deterministic sha order they appear in the file.
    CAP = 20
    capped = {"train": [0, 0, 0], "dev": [0, 0, 0]}
    seen_fam = defaultdict(lambda: {"train": 0, "dev": 0})
    for s in shapes:
        sp = s["split"]
        if seen_fam[s["family"]][sp] >= CAP:
            continue
        seen_fam[s["family"]][sp] += 1
        py_src = TEMPLATES[s["template_key"]](s["args"])
        p, m = toks(py_src), toks(s["program"])
        capped[sp][0] += p
        capped[sp][1] += m
        capped[sp][2] += 1

    print()
    print("## Overall aggregates")
    print()
    print("| scope | split | n | py | mtl | ratio |")
    print("|---|---|--:|--:|--:|--:|")
    for sp in ("train", "dev"):
        py, mtl, n = overall[sp]
        print(f"| micro token-SUM (all shapes) | {sp} | {n} | {py} | {mtl} | {ratio(py,mtl):.2f}x |")
    for sp in ("train", "dev"):
        print(f"| macro (mean of per-family ratios) | {sp} | {len(fam)} fams | | | {macro(sp):.2f}x |")
    for sp in ("train", "dev"):
        py, mtl, n = capped[sp]
        print(f"| capped micro (<=20/family) | {sp} | {n} | {py} | {mtl} | {ratio(py,mtl):.2f}x |")

    # v0.8 target families only (the newly-added uncovered shapes)
    print()
    print("## v0.8 new families only (scan + bitdigit)")
    print()
    print("| split | n | py | mtl | ratio |")
    print("|---|--:|--:|--:|--:|")
    for sp in ("train", "dev"):
        py = mtl = n = 0
        for f in ("scan", "bitdigit"):
            py += fam[f][sp][0]
            mtl += fam[f][sp][1]
            n += fam[f][sp][2]
        print(f"| {sp} | {n} | {py} | {mtl} | {ratio(py,mtl):.2f}x |")

    # per-key table for the new families (transparency)
    print()
    print("## Per-shape (new families), method labels")
    print()
    print("| template_key | split | py(template) | mtl(synth) | ratio |")
    print("|---|---|--:|--:|--:|")
    newkeys = sorted(
        k for k in per_key
        if any(s["template_key"] == k and s["family"] in ("scan", "bitdigit") for s in shapes)
    )
    for k in newkeys:
        for sp in ("train", "dev"):
            py, mtl, n = per_key[k][sp]
            if n:
                print(f"| {k} | {sp} | {py} | {mtl} | {ratio(py,mtl):.2f}x |")

    # emit machine-readable results
    out = {
        "encoder": ENC,
        "distinct_shapes": data["distinct_shapes"],
        "overall": {sp: {"py": overall[sp][0], "mtl": overall[sp][1],
                         "n": overall[sp][2], "ratio": ratio(*overall[sp][:2])}
                    for sp in ("train", "dev")},
        "macro": {sp: macro(sp) for sp in ("train", "dev")},
        "capped_micro": {sp: {"py": capped[sp][0], "mtl": capped[sp][1],
                              "n": capped[sp][2], "ratio": ratio(*capped[sp][:2])}
                         for sp in ("train", "dev")},
        "per_family": {
            f: {sp: {"py": fam[f][sp][0], "mtl": fam[f][sp][1], "n": fam[f][sp][2],
                     "ratio": ratio(fam[f][sp][0], fam[f][sp][1])}
                for sp in ("train", "dev")}
            for f in sorted(fam)
        },
    }
    (HERE / "broad_results.json").write_text(json.dumps(out, indent=2) + "\n")
    print()
    print(f"# wrote {HERE / 'broad_results.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
