#!/usr/bin/env python3
"""Project the v0.8 new-families (scan+bitdigit) out-of-sample ratio IF the
aperture combinator were admitted. Reuses measure.py's templates + tokenizer,
substitutes the two aperture-addressable programs, recomputes TRAIN/DEV.
"""
import json
import sys
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "bench" / "tokcount"))
sys.path.insert(0, str(REPO / "bench" / "design-v0.8"))
from tokcount import count
from measure import TEMPLATES  # reuse identical idiomatic-Python templates

ENC = "o200k_base"
shapes = json.loads((REPO / "bench/design-v0.8/broad_shapes.json").read_text())["shapes"]


def toks(s):
    return count(s)[ENC]


# aperture substitutions (simulator-verified in verify_aperture.py)
SUBST_FIXED = {  # fixed-width single glyph, no literal-k
    "[:>[~_][[]]?>[~_][[]]?>[__0][1]?][_0][:>_>_>__^<@@<*~>_~_][+]|": "0[^<@@<*+]#",
    "[:>[~_][[]]?>[__0][1]?][_0][:>_>__-:0<[0~-][]?~>_~_][^^<[~_][_]?]|":
        "0[-:0<[0~-][]?^^<[~_][_]?]`",
}
SUBST_PARAM = {  # parameterized width: [xs] acc0 k [C] w
    "[:>[~_][[]]?>[~_][[]]?>[__0][1]?][_0][:>_>_>__^<@@<*~>_~_][+]|": "0 3[^<@@<*+]w",
    "[:>[~_][[]]?>[__0][1]?][_0][:>_>__-:0<[0~-][]?~>_~_][^^<[~_][_]?]|":
        "0 2[-:0<[0~-][]?^^<[~_][_]?]w",
}


def aggregate(subst):
    agg = defaultdict(lambda: {"train": [0, 0], "dev": [0, 0]})  # [py,mtl]
    for s in shapes:
        if s["family"] not in ("scan", "bitdigit"):
            continue
        py = toks(TEMPLATES[s["template_key"]](s["args"]))
        prog = subst.get(s["program"], s["program"])
        mtl = toks(prog)
        b = agg[s["family"]][s["split"]]
        b[0] += py
        b[1] += mtl
    return agg


def combined(agg):
    out = {}
    for sp in ("train", "dev"):
        py = sum(agg[f][sp][0] for f in agg)
        mtl = sum(agg[f][sp][1] for f in agg)
        out[sp] = (py, mtl, py / mtl if mtl else 0)
    return out


for label, subst in [("BASELINE (current lang)", {}),
                     ("FIXED-glyph aperture", SUBST_FIXED),
                     ("PARAM-k aperture", SUBST_PARAM)]:
    c = combined(aggregate(subst))
    print(f"{label:26}  TRAIN {c['train'][0]}/{c['train'][1]} = {c['train'][2]:.2f}x"
          f"   DEV {c['dev'][0]}/{c['dev'][1]} = {c['dev'][2]:.2f}x")

# scan-only (the family the aperture targets)
print()
for label, subst in [("BASELINE scan-only", {}),
                     ("FIXED aperture scan-only", SUBST_FIXED),
                     ("PARAM aperture scan-only", SUBST_PARAM)]:
    agg = aggregate(subst)
    for sp in ("train", "dev"):
        py, mtl = agg["scan"][sp]
        print(f"{label:26} {sp:5}  {py}/{mtl} = {py/mtl:.2f}x")
