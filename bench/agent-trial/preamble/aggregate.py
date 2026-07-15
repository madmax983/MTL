#!/usr/bin/env python3
"""Aggregate the ICL-preamble ablation (issue #73).

Reads results/results.jsonl (one cell per line; schema in run_cell.md) and
computes, per variant:
  - solve_rate            fraction of cells with solved=true
  - median_tokens_to_first_correct
        median over SOLVED cells of
        (preamble_tokens + sum of o200k(program) for attempts up to and
         including the first correct attempt)

Writes results/metrics.json and prints a table. Program token counts use
bench/tokcount (o200k_base), matching validate_one.py's counter.

Usage:  python3 aggregate.py [--results <path>] [--out <path>]
"""
from __future__ import annotations

import argparse
import json
import os
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BENCH_DIR = os.path.dirname(os.path.dirname(HERE))  # .../bench
if BENCH_DIR not in sys.path:
    sys.path.insert(0, BENCH_DIR)


def _o200k(program: str) -> int:
    """o200k token count of a program, stripping exactly one trailing newline
    (identical normalization to validate_one.py / tokcount.count_file)."""
    from tokcount import tokcount  # type: ignore
    text = program[:-1] if program.endswith("\n") else program
    n = tokcount.count(text).get("o200k_base")
    if n is None:
        raise RuntimeError("o200k_base encoder unavailable (install tiktoken)")
    return n


def _tokens_to_first_correct(cell: dict) -> int:
    """preamble_tokens + sum o200k(program) over attempts up to & incl. first
    correct. Assumes cell is solved; uses first_correct_attempt (1-based)."""
    k = cell["first_correct_attempt"]
    progs = cell["programs"][:k]
    return int(cell["preamble_tokens"]) + sum(_o200k(p) for p in progs)


def load_cells(path: str) -> list[dict]:
    cells = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                cells.append(json.loads(line))
    return cells


def aggregate(cells: list[dict]) -> dict:
    by_variant: dict[str, list[dict]] = {}
    for c in cells:
        by_variant.setdefault(c["variant"], []).append(c)

    metrics: dict[str, dict] = {}
    for variant, group in by_variant.items():
        n_cells = len(group)
        solved = [c for c in group if c.get("solved")]
        ttfc = [_tokens_to_first_correct(c) for c in solved]
        preamble_tokens = [int(c["preamble_tokens"]) for c in group]
        # Secondary discriminators (solve-rate is saturated at 1.0):
        #   - attempts-to-first-correct: first_correct_attempt over solved cells
        #   - first-attempt success rate: fraction of cells solved on attempt 1
        atfc = [int(c["first_correct_attempt"]) for c in solved
                if c.get("first_correct_attempt") is not None]
        first_try = [c for c in group
                     if c.get("first_correct_attempt") == 1]
        metrics[variant] = {
            "n_cells": n_cells,
            "n_solved": len(solved),
            "solve_rate": (len(solved) / n_cells) if n_cells else 0.0,
            "median_tokens_to_first_correct": (
                statistics.median(ttfc) if ttfc else None
            ),
            "mean_attempts_to_first_correct": (
                statistics.mean(atfc) if atfc else None
            ),
            "first_attempt_success_rate": (
                (len(first_try) / n_cells) if n_cells else 0.0
            ),
            "mean_preamble_tokens": (
                statistics.mean(preamble_tokens) if preamble_tokens else None
            ),
        }
    return metrics


def print_table(metrics: dict) -> None:
    print(f"{'variant':<26} {'cells':>6} {'solved':>7} {'solve_rate':>11} "
          f"{'med_tok2correct':>16} {'mean_preamble':>14}")
    print("-" * 84)
    for variant in sorted(metrics):
        m = metrics[variant]
        med = m["median_tokens_to_first_correct"]
        med_s = f"{med:.1f}" if med is not None else "n/a"
        pre = m["mean_preamble_tokens"]
        pre_s = f"{pre:.1f}" if pre is not None else "n/a"
        print(f"{variant:<26} {m['n_cells']:>6} {m['n_solved']:>7} "
              f"{m['solve_rate']:>11.3f} {med_s:>16} {pre_s:>14}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", default=os.path.join(HERE, "results", "results.jsonl"))
    ap.add_argument("--out", default=os.path.join(HERE, "results", "metrics.json"))
    args = ap.parse_args()

    if not os.path.isfile(args.results):
        raise SystemExit(f"no results file: {args.results} (run the trials first)")

    cells = load_cells(args.results)
    if not cells:
        raise SystemExit(f"empty results file: {args.results}")
    metrics = aggregate(cells)

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(metrics, f, indent=2)
        f.write("\n")
    print_table(metrics)
    print(f"\nwrote {args.out}")


if __name__ == "__main__":
    main()
