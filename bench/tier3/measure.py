#!/usr/bin/env python3
"""Token measurements for the v0.4 Tier-3 agentic suite — TWO MTL columns.

For each of the 8 tasks this measures:
  (a) mtl-sketch  — the design's canonical hyphenated sketch (docs/design §8),
                    which reproduces the projected 1.96x aggregate; and
  (b) mtl-exec    — the EXECUTABLE lexer-safe `solution.mtl` actually run and
                    validated by crates/mtl-host (names mangled to [a-z][a-z0-9]*).

Python column and mtl-sketch column mirror bench/design-v0.4/agentic/measure.py
byte-for-byte, so the sketch aggregate re-derives the design's 1.96x. The exec
column is the real bytes from bench/tier3/tasks/<t>/solution.mtl (one trailing
newline stripped, matching the tokcount policy).

Run from bench/:   python3 tier3/measure.py
"""
import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent           # bench/tier3
_BENCH = _HERE.parent                             # bench
sys.path.insert(0, str(_BENCH / "tokcount"))
from tokcount import count  # noqa: E402

# name -> (python-sketch, mtl-design-sketch). Exec column read from solution.mtl.
TASKS = [
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
    ("word_count",
     "def solve():\n    return len(tokenize(read_text()))",
     "read-text tokenize 0[_1+](emit-int"),
    # ---- v0.4 expansion: 8 new tasks (multi-cap, budget-aware, fault-handling,
    #      string-handle pipelines, capability confinement). These are new tasks
    #      with no separate §8 design sketch, so the sketch column is the
    #      hyphenated form of the executable program (same shape, hyphenated
    #      capability names, matching the naming convention of the originals).
    ("transform_hits",
     "def solve():\n    for line in read_lines():\n        if line_hit(line):\n            emit(transform(line))",
     "read-lines 0[line-hit[transform emit][_]?](_"),
    ("emit_budget",
     "def solve():\n    for line in read_lines()[:2]:\n        emit(line)",
     "read-lines>@emit_>@emit__"),
    ("guarded_read",
     "def solve():\n    while not end_p():\n        emit(next_line())",
     "[end?][][next-line emit][]|"),
    ("concat_lines",
     "def solve():\n    emit(concat(next_line(), next_line()))",
     "next-line next-line concat emit"),
    ("select_line",
     "def solve():\n    emit(select(read_lines(), 2))",
     "read-lines 2 select emit"),
    ("confined_echo",
     "def solve():\n    emit(read_line())",
     "read-line emit"),
    ("confined_grep",
     "def solve():\n    for line in read_lines():\n        if line_hit(line):\n            emit(line)",
     "read-lines 0[line-hit[emit][_]?](_"),
    ("budget_grep",
     "def solve():\n    for line in read_lines():\n        if line_hit(line):\n            emit(line)",
     "read-lines 0[line-hit[emit][_]?](_"),
]


def exec_source(task: str) -> str:
    p = _HERE / "tasks" / task / "solution.mtl"
    text = p.read_text(encoding="utf-8")
    if text.endswith("\n"):
        text = text[:-1]
    return text


def main():
    hdr = (f"{'task':<20} {'py o':>5} {'py cl':>6} "
           f"{'skt o':>6} {'skt cl':>7} {'exe o':>6} {'exe cl':>7}")
    print(hdr)
    print("-" * len(hdr))
    tp_o = tp_c = ts_o = ts_c = te_o = te_c = 0
    for name, py, sketch in TASKS:
        cp = count(py)
        cs = count(sketch)
        ce = count(exec_source(name))
        tp_o += cp['o200k_base']; tp_c += cp['cl100k_base']
        ts_o += cs['o200k_base']; ts_c += cs['cl100k_base']
        te_o += ce['o200k_base']; te_c += ce['cl100k_base']
        print(f"{name:<20} {cp['o200k_base']:>5} {cp['cl100k_base']:>6} "
              f"{cs['o200k_base']:>6} {cs['cl100k_base']:>7} "
              f"{ce['o200k_base']:>6} {ce['cl100k_base']:>7}")
    print("-" * len(hdr))
    print(f"{'TOTAL':<20} {tp_o:>5} {tp_c:>6} "
          f"{ts_o:>6} {ts_c:>7} {te_o:>6} {te_c:>7}")
    print()

    def ratio(p, m):
        return f"{p/m:.2f}x" if m else "n/a"

    print("aggregate design-sketch (sum Py / sum MTL-sketch): "
          f"o200k {ratio(tp_o, ts_o)}   cl100k {ratio(tp_c, ts_c)}")
    print("aggregate executable    (sum Py / sum MTL-exec):   "
          f"o200k {ratio(tp_o, te_o)}   cl100k {ratio(tp_c, te_c)}")


if __name__ == "__main__":
    main()
