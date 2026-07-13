#!/usr/bin/env python3
"""Token measurements for the v0.4 string tasks (reverse, RLE).

Measures, under o200k_base + cl100k_base (tiktoken 0.8.0), every program variant
that the v0.4 Str-in-core-vs-host-side question turns on:

  - Python idiomatic reference (the bare function, as the corpus counts it).
  - MTL Variant A  (Str in the core): a proposed minimal string-primitive set.
  - MTL Variant B2 (host-side): strings enter/leave as codepoint-lists
    (Quote of Int) via host capabilities; the algorithm is pure Int/Quote
    compute over EXISTING primitives (no Str value at all).

Run from bench/:   python3 design-v0.4/strings/measure.py

Every count is len(enc.encode(text)). All MTL programs are HAND-TRACED,
design-stage, NOT interpreter-validated. The RLE MTL rows are design ESTIMATES
(representative real-glyph programs, not guaranteed minimally golfed; +-4 band),
following the v0.3 two_sum precedent. reverse rows are exact hand-traces.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tokcount"))
from tokcount import count  # noqa: E402

# --- Python idiomatic references (bare function = what the corpus counts) ----
REVERSE_PY = "def reverse(s):\n    return s[::-1]"
RLE_PY = (
    'def rle(s):\n'
    '    if not s:\n'
    '        return ""\n'
    '    out = []\n'
    '    prev, n = s[0], 1\n'
    '    for c in s[1:]:\n'
    '        if c == prev:\n'
    '            n += 1\n'
    '        else:\n'
    '            out.append(prev + str(n))\n'
    '            prev, n = c, 1\n'
    '    out.append(prev + str(n))\n'
    '    return "".join(out)'
)

# --- MTL Variant A: Str in the core -----------------------------------------
# Placeholder glyphs (final glyphs come from the glyphs.md sweep):
#   '#'  str-cons    ( c s -- s' )   prepend codepoint c onto string s
#   '`'  str-uncons  ( s -- c s' 1 | "" -- 0 )   head-split, codepoint as Int
#   '}'  num-to-str  ( n -- s )      render an Int as its decimal digit string
#   ','  reused as str-concat when both operands are Str
#   '""' empty-string literal
#   fold '(' GENERALISED to iterate a Str's codepoints (no new iteration prim)
REVERSE_A_FOLD = '""[~#]('                     # fold-over-str, C prepends -> reverse
REVERSE_A_LINREC = "\"\"~[`[#0][[]1]?][_][`_[~#]'][]|"  # linrec, no fold-gen (2 new prims)
# RLE Variant A: linrec carrying  s out prev cnt  (design ESTIMATE, +-4 band).
RLE_A = "\"\"0 0[`0=][@}#,@][`@^=[_1+][@}#,~1]?]|"

# --- MTL Variant B2: host-side (codepoint-list) ------------------------------
# Host capability read-codepoints ( -- [cp...] ) marshals the input string into a
# Quote of Int; host capability render/emit takes the structured result out.
# The IN-CORE algorithm then uses ONLY existing primitives (no Str value at all).
REVERSE_B2 = '[][~;]('                  # == the existing corpus reverse_list solution
# RLE compute over a codepoint-list -> list of [codepoint count] pairs, existing
# prims only (uncons >, cons ;, eq =, fold (). Design ESTIMATE, +-4 band.
RLE_B2_CORE = '[]~[^0=[[;1];][>_@^=[1+~;;][@[;1]]?]?](~[~;]('

PROGRAMS = [
    ("reverse  Python idiomatic",                          REVERSE_PY),
    ("reverse  MTL Var A (fold-gen + str-cons)",           REVERSE_A_FOLD),
    ("reverse  MTL Var A (linrec, 2 str prims)",           REVERSE_A_LINREC),
    ("reverse  MTL Var B2 (=reverse_list, 0 new prims)",   REVERSE_B2),
    ("rle      Python idiomatic",                          RLE_PY),
    ("rle      MTL Var A (str-cons+num-to-str, est)",      RLE_A),
    ("rle      MTL Var B2 core (pairs, existing, est)",    RLE_B2_CORE),
]


def main():
    print(f"{'program':<50} {'chars':>6} {'o200k':>7} {'cl100k':>7}")
    print("-" * 74)
    for label, prog in PROGRAMS:
        c = count(prog)
        print(f"{label:<50} {len(prog):>6} {c['o200k_base']:>7} {c['cl100k_base']:>7}")


if __name__ == "__main__":
    main()
