#!/usr/bin/env python3
"""validate_py.py — the Python-arm oracle for the Tier-3 capability cold-agent
trial, SYMMETRIC to the `tier3run` MTL validator.

Usage:
    python3 validate_py.py <task> < program.py

Reads a Python program defining `solve()` from stdin, execs it in a restricted
namespace whose globals bind ONLY host-function stubs (plus a small set of safe
builtins), then calls `solve()` and compares captured output to the task's
`expected_output`. Prints ONE verdict line and exits 0:

    PASS
    FAIL: wrong_output got=<repr> want=<repr>
    FAIL: NotGranted <name>
    FAIL: BudgetExhausted
    FAIL: InputClosed
    FAIL: python_exception <ExcType>: <msg>

Confinement is enforced symmetrically with the MTL runtime: EVERY stub name is
bound into the namespace, but any capability NOT in the task's `granted` list is
bound to a stub that RAISES NotGranted(<name>) when called. An ungranted-call
attempt is therefore a loud, categorizable failure in both arms, so the trial
can count ungranted-call attempts on either side.
"""
from __future__ import annotations

import io
import json
import sys
from contextlib import redirect_stdout
from pathlib import Path

TASKS_PATH = Path(__file__).resolve().parent / "tasks.json"


# --- Sentinels mirroring the MTL host fault vocabulary -----------------------
class NotGranted(Exception):
    """A capability outside the task's grant set was called."""


class BudgetExhausted(Exception):
    """A metered call exceeded its per-name call budget."""


class InputClosed(Exception):
    """next_line was called past the end of input."""


# --- Per-task fixtures (host-owned inputs; the analogue of the Rust fixtures) -
# lines: the host input lines. predicate_char: the char line_hit tests for.
FIXTURES = {
    "transform_hits": {"lines": ["apple", "banana", "apricot", "cherry"], "predicate_char": "a"},
    "emit_budget":    {"lines": ["one", "two", "three", "four"],          "predicate_char": None},
    "guarded_read":   {"lines": ["x", "y", "z"],                          "predicate_char": None},
    "concat_lines":   {"lines": ["foo", "bar"],                           "predicate_char": None},
    "select_line":    {"lines": ["a", "b", "c", "d"],                     "predicate_char": None},
    "confined_echo":  {"lines": ["hello"],                                "predicate_char": None},
    "confined_grep":  {"lines": ["cat", "dog", "car", "fish"],            "predicate_char": "c"},
    "budget_grep":    {"lines": ["ant", "bee", "art", "cod"],             "predicate_char": "a"},
}

# Map every implemented stub's Python binding name -> its MTL capability name.
# Grantedness is decided against the capability name, so a task granting
# "readline"+"emit" makes read_line and emit callable and everything else raise.
STUB_TO_CAP = {
    "read_line": "readline",
    "read_lines": "readlines",
    "emit": "emit",
    "emit_int": "emitint",
    "line_hit": "linehit",
    "transform": "transform",
    "next_line": "nextline",
    "end_p": "endp",
    "concat": "concat",
    "select": "select",
}

# Safe builtins for the sandbox: enough to write idiomatic solve() bodies, but
# no __import__, open, eval, exec, etc.
SAFE_BUILTINS = {
    name: __builtins__[name] if isinstance(__builtins__, dict) else getattr(__builtins__, name)
    for name in (
        "range", "len", "enumerate", "zip", "map", "filter", "list", "dict",
        "set", "tuple", "str", "int", "bool", "float", "abs", "min", "max",
        "sum", "sorted", "reversed", "all", "any", "True", "False", "None",
        "print", "isinstance", "repr", "iter", "next", "ValueError",
        "IndexError", "TypeError", "KeyError", "StopIteration", "Exception",
    )
}


def load_task(task_name):
    spec = json.loads(TASKS_PATH.read_text(encoding="utf-8"))
    for t in spec["tasks"]:
        if t["name"] == task_name:
            return t
    return None


def build_namespace(task, fixture, out_buf):
    """Bind stub globals for `task`. Granted stubs get the real implementation;
    ungranted ones get a NotGranted-raising stub. All names are always bound."""
    granted = set(task["granted"])
    lines = list(fixture["lines"])
    predicate_char = fixture["predicate_char"]

    # Mutable host state shared by the closures.
    state = {"cursor": 0, "emitted": 0}
    emit_budget = task["emit_budget"]  # int or None

    def read_line():
        if not lines:
            raise InputClosed()
        return lines[0]

    def read_lines():
        return list(lines)

    def emit(s):
        if emit_budget is not None and state["emitted"] >= emit_budget:
            raise BudgetExhausted()
        state["emitted"] += 1
        out_buf.append(str(s) + "\n")

    def emit_int(n):
        out_buf.append(str(int(n)) + "\n")

    def line_hit(s):
        if predicate_char is None:
            return False
        return str(s).startswith(predicate_char)

    def transform(s):
        return str(s).upper()

    def next_line():
        if state["cursor"] >= len(lines):
            raise InputClosed()
        line = lines[state["cursor"]]
        state["cursor"] += 1
        return line

    def end_p():
        return state["cursor"] >= len(lines)

    def concat(a, b):
        return str(a) + str(b)

    def select(lst, n):
        return lst[n]

    real_impls = {
        "read_line": read_line,
        "read_lines": read_lines,
        "emit": emit,
        "emit_int": emit_int,
        "line_hit": line_hit,
        "transform": transform,
        "next_line": next_line,
        "end_p": end_p,
        "concat": concat,
        "select": select,
    }

    def make_denied(cap_name):
        def denied(*_args, **_kwargs):
            raise NotGranted(cap_name)
        return denied

    ns = {"__builtins__": SAFE_BUILTINS}
    for py_name, cap_name in STUB_TO_CAP.items():
        if cap_name in granted:
            ns[py_name] = real_impls[py_name]
        else:
            # Bind the NAME but make any call a loud, categorizable failure.
            ns[py_name] = make_denied(cap_name)
    return ns


def verdict(task_name, program):
    task = load_task(task_name)
    if task is None:
        print(f"unknown task: {task_name}", file=sys.stderr)
        return 1
    fixture = FIXTURES.get(task_name)
    if fixture is None:
        print(f"no fixture for task: {task_name}", file=sys.stderr)
        return 1

    out_buf = []
    ns = build_namespace(task, fixture, out_buf)

    try:
        # Compile+exec the program body (defines solve), then call solve().
        # stdout from the program itself is discarded; capture is via out_buf.
        code = compile(program, "<program>", "exec")
        sink = io.StringIO()
        with redirect_stdout(sink):
            exec(code, ns)
            solve = ns.get("solve")
            if not callable(solve):
                print("FAIL: python_exception NameError: solve() is not defined")
                return 0
            solve()
    except NotGranted as e:
        print(f"FAIL: NotGranted {e.args[0]}")
        return 0
    except BudgetExhausted:
        print("FAIL: BudgetExhausted")
        return 0
    except InputClosed:
        print("FAIL: InputClosed")
        return 0
    except Exception as e:  # noqa: BLE001 - any other error is a categorized failure
        msg = str(e).splitlines()[0] if str(e) else ""
        print(f"FAIL: python_exception {type(e).__name__}: {msg}")
        return 0

    got = "".join(out_buf)
    want = task["expected_output"]
    if got == want:
        print("PASS")
    else:
        print(f"FAIL: wrong_output got={got!r} want={want!r}")
    return 0


def main(argv):
    if len(argv) < 2:
        print("usage: validate_py.py <task> < program.py", file=sys.stderr)
        return 1
    program = sys.stdin.read()
    return verdict(argv[1], program)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
