#!/usr/bin/env python3
"""Token measurements for the v0.4 Tier-3 agentic suite (read-input / emit-output
/ tool-call shape). o200k_base + cl100k_base, tiktoken 0.8.0.

All MTL programs are HAND-TRACED, design-stage, REPRESENTATIVE sketches (real
glyphs; capability names are `Call` words). Token counts are real; exact
minimal golf is not guaranteed (design estimates, per the v0.3 two_sum
precedent). Python sketches are the bare idiomatic `solve()` bodies.

Run from bench/:   python3 design-v0.4/agentic/measure.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tokcount"))
from tokcount import count  # noqa: E402

TASKS = [
    # name, python-sketch, mtl-sketch
    ("echo_line",
     "def solve():\n    emit(read_line())",
     "read-line emit"),
    ("grep_filter",
     "def solve():\n    for line in read_lines():\n        if line_matches(line):\n            emit(line)",
     "read-lines 0[line-hit[emit][_]?](_"),
    ("agent_loop",
     "def solve():\n    s = read_state()\n    while not done(s):\n        s = step(s)\n    return s",
     "read-state[done?][][step][]|"),
    ("json_field",
     'def solve():\n    return get_field(read_json(), "name")',
     "read-json get-name emit"),
    ("two_tool_pipeline",
     "def solve():\n    return parse(fetch(read_input()))",
     "read-input fetch parse emit"),
    ("retry_on_fault",
     "def solve():\n    for _ in range(3):\n        r, ok = try_op()\n        if ok:\n            return r\n    return None",
     "3[try-op ok?][_][][1-]|"),
    ("map_lines_tool",
     "def solve():\n    for line in read_lines():\n        emit(transform(line))",
     "read-lines 0[transform emit](_"),
    ("word_count (tokenize cap)",
     "def solve():\n    return len(tokenize(read_text()))",
     "read-text tokenize 0[_1+](emit-int"),
]


def main():
    print(f"{'task':<28} {'py o200k':>9} {'py cl100k':>10} {'mtl o200k':>10} {'mtl cl100k':>11}")
    print("-" * 72)
    tp_o = tp_c = tm_o = tm_c = 0
    for name, py, mtl in TASKS:
        cp = count(py)
        cm = count(mtl)
        tp_o += cp['o200k_base']; tp_c += cp['cl100k_base']
        tm_o += cm['o200k_base']; tm_c += cm['cl100k_base']
        print(f"{name:<28} {cp['o200k_base']:>9} {cp['cl100k_base']:>10} "
              f"{cm['o200k_base']:>10} {cm['cl100k_base']:>11}")
    print("-" * 72)
    print(f"{'TOTAL':<28} {tp_o:>9} {tp_c:>10} {tm_o:>10} {tm_c:>11}")
    print(f"aggregate (sum Py / sum MTL):  o200k {tp_o/tm_o:.2f}x   cl100k {tp_c/tm_c:.2f}x")


if __name__ == "__main__":
    main()
