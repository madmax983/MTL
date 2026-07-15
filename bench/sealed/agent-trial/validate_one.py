#!/usr/bin/env python3
"""Deterministic single-program validator for the SEALED MTL agent-writability trial.

Validates ONE candidate program for ONE task against the I/O vectors in
tasks.json, for either the `mtl` or the `python` arm, and emits exactly one
JSON line describing the result. Always exits 0 (the JSON carries the verdict).

This is the held-out (sealed) counterpart of bench/agent-trial/validate_one.py.
Behaviour is identical; only the default tasks file differs (it loads the sealed
tasks.json alongside this script). A --tasks-file override is available.

Usage:
    python3 validate_one.py --task <id> --arm <mtl|python> --program-file <path>
    python3 validate_one.py --task <id> --arm <mtl|python> --program-stdin

MTL arm:  for each vector, runs `printf '%s' "<input_prefix><program>" | mtlrun`
          and passes iff the first output line == "HALT: <expected_halt>".
Python arm: execs the program in a restricted namespace, calls solve(*args)
          per vector, and compares == expected.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
BENCH_DIR = os.path.dirname(os.path.dirname(HERE))  # .../bench
TASKS_JSON = os.path.join(HERE, "tasks.json")
MTL_TIMEOUT_S = 10

# Make the tokcount module importable (bench/tokcount/tokcount.py).
if BENCH_DIR not in sys.path:
    sys.path.insert(0, BENCH_DIR)

# Fault-kind -> error_type. The kinds are the Debug names printed by mtlrun.
_FAULT_KINDS = {
    "Underflow",
    "TypeMismatch",
    "Overflow",
    "DivByZero",
    "UnknownWord",
    "FuelExhausted",
}

# Whitelisted builtins for the Python arm's restricted exec namespace.
_SAFE_BUILTINS = {
    name: __builtins__[name] if isinstance(__builtins__, dict)
    else getattr(__builtins__, name)
    for name in (
        "abs", "all", "any", "bool", "dict", "divmod", "enumerate", "filter",
        "float", "int", "len", "list", "map", "max", "min", "range",
        "reversed", "round", "set", "sorted", "str", "sum", "tuple", "zip",
        "True", "False", "None",
    )
}


def _token_counts(program: str) -> tuple[int | None, int | None]:
    """Token counts for a program string, stripping exactly one trailing newline."""
    text = program[:-1] if program.endswith("\n") else program
    try:
        from tokcount import tokcount  # type: ignore
        counts = tokcount.count(text)
        return counts.get("o200k_base"), counts.get("cl100k_base")
    except Exception:  # noqa: BLE001 - token counts are best-effort
        return None, None


def _find_mtlrun() -> str | None:
    candidates = [
        "/workspace/target/debug/mtlrun",
        os.path.join(os.path.dirname(BENCH_DIR), "target", "debug", "mtlrun"),
    ]
    for c in candidates:
        if os.path.isfile(c) and os.access(c, os.X_OK):
            return c
    return None


def _load_task(task_id: str, tasks_json: str) -> dict:
    with open(tasks_json, encoding="utf-8") as f:
        spec = json.load(f)
    for t in spec["tasks"]:
        if t["id"] == task_id:
            return t
    raise SystemExit(f"unknown task id: {task_id!r} (not in {tasks_json})")


# Durable per-attempt record options, set by main() before validation runs.
# When _RECORD_DIR is not None, each _emit() also writes a full record file.
_TRIAL: int | None = None
_ATTEMPT: int | None = None
_RECORD_DIR: str | None = None


def _write_record(result: dict) -> None:
    """Atomically write the full result dict to a per-attempt record file.

    Writes to a temp file in the same directory then os.replace(), so a
    concurrent reader never observes a partial file.
    """
    os.makedirs(_RECORD_DIR, exist_ok=True)
    trial = _TRIAL if _TRIAL is not None else 0
    attempt = _ATTEMPT if _ATTEMPT is not None else 0
    fname = f"{result['task']}_{result['arm']}_t{trial}_a{attempt}.json"
    path = os.path.join(_RECORD_DIR, fname)
    fd, tmp = tempfile.mkstemp(dir=_RECORD_DIR, prefix=".tmp-", suffix=".json")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(result, f, indent=2)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def _emit(result: dict) -> None:
    if _RECORD_DIR is not None:
        result = {**result, "trial": _TRIAL, "attempt": _ATTEMPT}
        _write_record(result)
    print(json.dumps(result))
    sys.exit(0)


def validate_mtl(task: dict, program: str, o200k, cl100k) -> None:
    base = {
        "task": task["id"],
        "arm": "mtl",
        "program_tokens_o200k": o200k,
        "program_tokens_cl100k": cl100k,
        "program": program,
    }
    mtlrun = _find_mtlrun()
    if mtlrun is None:
        _emit({
            **base, "ok": False, "error_type": "no_mtlrun",
            "error_detail": (
                "mtlrun binary not found; build it first: "
                "cargo build --bin mtlrun -p mtl-bench-validate "
                "(looked in /workspace/target/debug and ./target/debug)"
            ),
            "failing_vector": None,
        })

    for vec in task["mtl"]["vectors"]:
        src = vec["input_prefix"] + program
        expected = vec["expected_halt"]
        try:
            proc = subprocess.run(
                [mtlrun], input=src, capture_output=True, text=True,
                timeout=MTL_TIMEOUT_S,
            )
        except subprocess.TimeoutExpired:
            _emit({
                **base, "ok": False, "error_type": "timeout",
                "error_detail": f"mtlrun exceeded {MTL_TIMEOUT_S}s",
                "failing_vector": vec,
            })

        out = proc.stdout
        lines = out.splitlines()
        first = lines[0] if lines else ""

        if first.startswith("HALT: "):
            got = first[len("HALT: "):]
            if got == expected:
                continue
            _emit({
                **base, "ok": False, "error_type": "wrong_output",
                "error_detail": f"got {got!r}, expected {expected!r}",
                "failing_vector": vec,
            })
        elif first.startswith("FAULT: "):
            kind = first[len("FAULT: "):].strip()
            etype = kind if kind in _FAULT_KINDS else "fault"
            _emit({
                **base, "ok": False, "error_type": etype,
                "error_detail": out.strip(), "failing_vector": vec,
            })
        elif first.startswith("FUEL EXHAUSTED"):
            _emit({
                **base, "ok": False, "error_type": "FuelExhausted",
                "error_detail": out.strip(), "failing_vector": vec,
            })
        elif first.startswith("PARSE ERROR"):
            _emit({
                **base, "ok": False, "error_type": "parse",
                "error_detail": out.strip(), "failing_vector": vec,
            })
        else:
            detail = (out + proc.stderr).strip() or "no output"
            _emit({
                **base, "ok": False, "error_type": "unknown",
                "error_detail": detail, "failing_vector": vec,
            })

    _emit({
        **base, "ok": True, "error_type": None, "error_detail": None,
        "failing_vector": None,
    })


def validate_python(task: dict, program: str, o200k, cl100k) -> None:
    base = {
        "task": task["id"],
        "arm": "python",
        "program_tokens_o200k": o200k,
        "program_tokens_cl100k": cl100k,
        "program": program,
    }
    namespace: dict = {"__builtins__": dict(_SAFE_BUILTINS)}
    try:
        exec(compile(program, "<candidate>", "exec"), namespace)  # noqa: S102
    except Exception:  # noqa: BLE001
        _emit({
            **base, "ok": False, "error_type": "python_exception",
            "error_detail": traceback.format_exc().strip().splitlines()[-1],
            "failing_vector": None,
        })

    solve = namespace.get("solve")
    if not callable(solve):
        _emit({
            **base, "ok": False, "error_type": "python_exception",
            "error_detail": "program does not define a callable solve(...)",
            "failing_vector": None,
        })

    for vec in task["python"]["vectors"]:
        args = vec["args"]
        expected = vec["expected"]
        try:
            got = solve(*args)
        except Exception:  # noqa: BLE001
            _emit({
                **base, "ok": False, "error_type": "python_exception",
                "error_detail": traceback.format_exc().strip().splitlines()[-1],
                "failing_vector": vec,
            })
        if got != expected:
            _emit({
                **base, "ok": False, "error_type": "wrong_output",
                "error_detail": f"got {got!r}, expected {expected!r}",
                "failing_vector": vec,
            })

    _emit({
        **base, "ok": True, "error_type": None, "error_detail": None,
        "failing_vector": None,
    })


def main() -> None:
    ap = argparse.ArgumentParser(description="Validate one MTL/Python program.")
    ap.add_argument("--task", required=True)
    ap.add_argument("--arm", required=True, choices=["mtl", "python"])
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--program-file")
    src.add_argument("--program-stdin", action="store_true")
    ap.add_argument("--trial", type=int, default=None)
    ap.add_argument("--attempt", type=int, default=None)
    ap.add_argument("--record-dir", default=None)
    ap.add_argument("--tasks-file", default=TASKS_JSON,
                    help="path to tasks.json (defaults to the sealed trial spec)")
    args = ap.parse_args()

    global _TRIAL, _ATTEMPT, _RECORD_DIR
    _TRIAL = args.trial
    _ATTEMPT = args.attempt
    _RECORD_DIR = args.record_dir

    if args.program_stdin:
        program = sys.stdin.read()
    else:
        with open(args.program_file, encoding="utf-8") as f:
            program = f.read()

    task = _load_task(args.task, args.tasks_file)
    o200k, cl100k = _token_counts(program)

    if args.arm == "mtl":
        validate_mtl(task, program, o200k, cl100k)
    else:
        validate_python(task, program, o200k, cl100k)


if __name__ == "__main__":
    main()
