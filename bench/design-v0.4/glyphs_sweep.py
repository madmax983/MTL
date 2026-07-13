#!/usr/bin/env python3
"""In-context glyph BPE sweep for the v0.4 candidate primitives (spec §11).

Per the assignment protocol, each candidate glyph is measured INSIDE a small
representative program (not as a bare char) under o200k_base + cl100k_base
(tiktoken 0.8.0). The free ASCII set after the v0.3 assignment (fold '(' + xor
'$' spent) is:  #  )  \\  `  {  }   (six chars; '(' and '$' now taken).

Run from bench/:   python3 design-v0.4/glyphs_sweep.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tokcount"))
from tokcount import count  # noqa: E402

FREE = ["#", ")", "\\", "`", "{", "}"]

# Each entry: (primitive, template with {G}, load-bearing bigram note).
# The template is a representative real program in which the glyph actually
# appears in the position it would occupy in a solution.
CASES = [
    # str-cons: appears after ']' in the reverse fold  ""[~#](  -> bigram  #(  and  ~#
    ("str-cons  (reverse fold)", '""[~{G}](', "~G and G("),
    # str-uncons: appears after '[' as a predicate head in  ""~[`0=]...  -> bigram [G
    ("str-uncons (linrec pred)", '""~[{G}0=][_][{G}_[~#]\'][]|', "[G"),
    # num-to-str: appears after '@' / before '#' in RLE flush  @}#  -> bigram @G and G#
    ("num-to-str (RLE flush)", '@{G}#,@', "@G and G#"),
    # capability/invoke sigil: prefixes a Call name, e.g.  {G}emit  in a pipeline
    ("invoke sigil (pipeline)", 'read-input fetch parse {G}emit', "Gemit"),
    # metering/budget annotation: wraps a fuel budget around a loop, e.g.  {G}100 [ ... ]
    ("budget annotation (loop)", '{G}100 read-state[done?][][step][]|', "G100"),
]


def main():
    for prim, tmpl, note in CASES:
        print(f"\n### {prim}   (load-bearing bigram: {note})")
        print(f"    template: {tmpl.replace('{G}', 'G')}")
        print(f"    {'glyph':<6} {'o200k':>7} {'cl100k':>7}")
        best = None
        rows = []
        for g in FREE:
            prog = tmpl.replace("{G}", g)
            c = count(prog)
            rows.append((g, c['o200k_base'], c['cl100k_base']))
            if best is None or c['o200k_base'] < best:
                best = c['o200k_base']
        for g, o, cl in rows:
            mark = "  <- min" if o == best else ""
            gg = g if g != "\\" else "\\ (backslash)"
            print(f"    {gg:<6} {o:>7} {cl:>7}{mark}")


if __name__ == "__main__":
    main()
