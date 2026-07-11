#!/usr/bin/env python3
"""Reproduce the v0.2 recursion-primitive token counts (design stage).

Imports the SAME tiktoken encoders the bench harness uses
(`bench/tokcount/tokcount.py`: the module-level `count(text)` helper and the
`ENCODINGS` list) so these counts are directly comparable to `bench/BASELINE.md`.

Run from the repo root:  python3 bench/design-v0.2/measure.py

Counts:
  (a) each design-stage candidate .mtl program string, and
  (b) each python/*.py baseline (stripping one trailing newline, matching the
      harness's count_file policy),
then prints a `label | o200k | cl100k` table and the three aggregate ratios
from docs/design/v0.2-recursion-primitives.md §6.3.

DESIGN STAGE: the .mtl programs use candidate glyphs not yet implemented in the
parser/interpreter; correctness is by hand-trace, not interpreter-validated.
Only the token counts are real.
"""

from __future__ import annotations

from pathlib import Path
import sys

# --- locate the repo and the tokcount module -------------------------------
HERE = Path(__file__).resolve().parent          # bench/design-v0.2
REPO = HERE.parent.parent                        # repo root
TOKCOUNT_DIR = REPO / "bench" / "tokcount"
sys.path.insert(0, str(TOKCOUNT_DIR))

from tokcount import count, ENCODINGS  # noqa: E402  (same encoders as the harness)

CANDIDATES = HERE / "candidates"
PYTHON = HERE / "python"


def count_str(text: str) -> dict[str, int | None]:
    return count(text)


def count_pyfile(path: Path) -> dict[str, int | None]:
    """Match tokcount.count_file: strip exactly one trailing newline."""
    text = path.read_text(encoding="utf-8")
    if text.endswith("\n"):
        text = text[:-1]
    return count(text)


def fmt(counts: dict[str, int | None]) -> tuple[str, str]:
    def cell(name: str) -> str:
        v = counts[name]
        return "UNAVAIL" if v is None else str(v)
    return cell("o200k_base"), cell("cl100k_base")


def main() -> int:
    # (a) candidates (design-stage MTL v0.2 programs) -----------------------
    candidate_order = ["factorial", "sum_to", "gcd", "fib", "power"]
    mtl_counts: dict[str, dict[str, int | None]] = {}
    rows: list[tuple[str, str, str]] = []
    for name in candidate_order:
        text = (CANDIDATES / f"{name}.mtl").read_text(encoding="utf-8")
        # candidate files carry no trailing newline, but be defensive:
        if text.endswith("\n"):
            text = text[:-1]
        c = count_str(text)
        mtl_counts[name] = c
        o, cl = fmt(c)
        rows.append((f"mtl/{name}  `{text}`", o, cl))

    # (b) python baselines (this dir only carries the dev tasks) ------------
    py_order = [
        "fib-idiomatic", "fib-minified",
        "sum_to-idiomatic", "sum_to-minified",
        "power-idiomatic", "power-minified",
    ]
    py_counts: dict[str, dict[str, int | None]] = {}
    for name in py_order:
        c = count_pyfile(PYTHON / f"{name}.py")
        py_counts[name] = c
        o, cl = fmt(c)
        rows.append((f"py/{name}", o, cl))

    # --- print the measurement table --------------------------------------
    print("# v0.2 candidate token counts (design stage)")
    print(f"# encoders: {', '.join(ENCODINGS)} (via bench/tokcount)")
    print()
    w = max(len(r[0]) for r in rows)
    print(f"| {'label'.ljust(w)} | o200k | cl100k |")
    print(f"| {'-' * w} | -----:| ------:|")
    for label, o, cl in rows:
        print(f"| {label.ljust(w)} | {o:>5} | {cl:>6} |")
    print()

    # --- aggregate ratios (§6.3) ------------------------------------------
    # Use o200k as canonical; both encodings agree on every cell here.
    ENC = "o200k_base"

    def mtl(name: str) -> int:
        return int(mtl_counts[name][ENC])

    def py(name: str) -> int:
        return int(py_counts[name][ENC])

    # Straight-line T_v0 tasks are not recursion candidates; their counts are
    # fixed constants from bench/BASELINE.md (idiomatic Python) and the doc
    # (MTL v0.1 == v0.2, unchanged). factorial/gcd idiomatic-Py are likewise
    # taken from BASELINE.md; the MTL v0.2 factorial/gcd are MEASURED above.
    PY_IDIOM = {
        "affine": 13, "rev3": 16, "is_even": 14, "factorial": 26, "gcd": 24,
    }
    MTL_STRAIGHT = {"affine": 4, "rev3": 2, "is_even": 4}

    # frozen T_v0 (5)
    frozen_py = sum(PY_IDIOM.values())
    frozen_mtl = sum(MTL_STRAIGHT.values()) + mtl("factorial") + mtl("gcd")

    # dev recursion tasks (idiomatic Python measured from files above)
    dev_py = py("fib-idiomatic") + py("sum_to-idiomatic") + py("power-idiomatic")
    dev_mtl = mtl("fib") + mtl("sum_to") + mtl("power")

    # combined
    all8_py = frozen_py + dev_py
    all8_mtl = frozen_mtl + dev_mtl

    # recursion-only (factorial, gcd, fib, sum_to, power)
    rec_py = (PY_IDIOM["factorial"] + PY_IDIOM["gcd"]
              + py("fib-idiomatic") + py("sum_to-idiomatic") + py("power-idiomatic"))
    rec_mtl = (mtl("factorial") + mtl("gcd")
               + mtl("fib") + mtl("sum_to") + mtl("power"))

    print("# aggregate ratios (§6.3)")
    print()
    print("| corpus | idiomatic Py | MTL v0.2 | aggregate |")
    print("|---|---:|---:|---:|")
    print(f"| frozen T_v0 (5) | {frozen_py} | {frozen_mtl} | {frozen_py / frozen_mtl:.2f}x |")
    print(f"| T_v0 + dev (8) | {all8_py} | {all8_mtl} | {all8_py / all8_mtl:.2f}x |")
    print(f"| recursion-only (5) | {rec_py} | {rec_mtl} | {rec_py / rec_mtl:.2f}x |")
    print()

    # --- self-check against the doc's headline numbers --------------------
    checks = {
        "factorial == 5": mtl("factorial") == 5,
        "gcd == 10": mtl("gcd") == 10,
        "fib == 6": mtl("fib") == 6,
        "frozen aggregate ~ 3.72x": abs(frozen_py / frozen_mtl - 3.72) < 0.01,
        "8-task aggregate ~ 4.37x": abs(all8_py / all8_mtl - 4.37) < 0.01,
        "recursion-only ~ 4.39x": abs(rec_py / rec_mtl - 4.39) < 0.01,
    }
    print("# self-check (vs doc)")
    ok = True
    for label, passed in checks.items():
        print(f"  [{'PASS' if passed else 'FAIL'}] {label}")
        ok = ok and passed
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
