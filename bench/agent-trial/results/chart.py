#!/usr/bin/env python3
"""Charts for the MTL agent-writability trial.

Reads metrics.json (produced by report.py) and renders two PNGs into the same
directory:

  (a) chart_tokens_to_first_correct.png
      Median output-tokens-to-first-correct per task, MTL vs Python (grouped bars).
  (b) chart_cspm.png
      Correct-solutions-per-million-tokens, MTL vs Python (bar).

Usage:
  python3 chart.py [--metrics <metrics.json>] [--out-dir <dir>]

Defaults resolve relative to this script so it runs from any working directory.
Pure-python + matplotlib; not in the Rust build graph or CI.
"""

import argparse
import json
import os

import matplotlib
matplotlib.use("Agg")  # headless
import matplotlib.pyplot as plt
import numpy as np

ARMS = ("mtl", "python")
COLORS = {"mtl": "#4C72B0", "python": "#DD8452"}


def _here(*parts):
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), *parts)


def parse_args():
    p = argparse.ArgumentParser(description="Render MTL-trial charts.")
    p.add_argument("--metrics", default=_here("metrics.json"))
    p.add_argument("--out-dir", default=_here())
    return p.parse_args()


def chart_tokens_per_task(metrics, out_path):
    per_task = metrics["per_task"]
    tasks = sorted(per_task)
    mtl_vals = [per_task[t]["mtl"]["output_tokens_to_first_correct_median"] for t in tasks]
    py_vals = [per_task[t]["python"]["output_tokens_to_first_correct_median"] for t in tasks]
    # None -> 0 for plotting (unsolved arm shows no bar).
    mtl_plot = [v if v is not None else 0 for v in mtl_vals]
    py_plot = [v if v is not None else 0 for v in py_vals]

    x = np.arange(len(tasks))
    width = 0.38
    fig, ax = plt.subplots(figsize=(12, 6))
    b1 = ax.bar(x - width / 2, mtl_plot, width, label="MTL", color=COLORS["mtl"])
    b2 = ax.bar(x + width / 2, py_plot, width, label="Python", color=COLORS["python"])
    ax.bar_label(b1, padding=2, fontsize=8)
    ax.bar_label(b2, padding=2, fontsize=8)
    ax.set_ylabel("Median output tokens to first correct (o200k_base)")
    ax.set_title("Median output tokens to first correct, per task — MTL vs Python\n(lower is better)")
    ax.set_xticks(x)
    ax.set_xticklabels(tasks, rotation=30, ha="right")
    ax.legend()
    ax.grid(axis="y", linestyle=":", alpha=0.5)
    fig.tight_layout()
    fig.savefig(out_path, dpi=130)
    plt.close(fig)
    return out_path


def chart_cspm(metrics, out_path):
    pa = metrics["per_arm"]
    vals = [pa[a]["correct_solutions_per_million_tokens"] for a in ARMS]
    labels = ["MTL", "Python"]
    fig, ax = plt.subplots(figsize=(6, 6))
    bars = ax.bar(labels, vals, color=[COLORS["mtl"], COLORS["python"]], width=0.55)
    ax.bar_label(bars, fmt="%.0f", padding=3)
    ax.set_ylabel("Correct solutions per 1,000,000 output tokens")
    ax.set_title("Efficiency: correct-solutions-per-million-tokens\n(charges failed repair attempts; higher is better)")
    ax.grid(axis="y", linestyle=":", alpha=0.5)
    ratio = vals[0] / vals[1] if vals[1] else float("nan")
    ax.text(0.5, 0.95, "MTL / Python ratio = %.2f" % ratio,
            transform=ax.transAxes, ha="center", va="top", fontsize=10)
    fig.tight_layout()
    fig.savefig(out_path, dpi=130)
    plt.close(fig)
    return out_path


def main():
    args = parse_args()
    with open(args.metrics) as fh:
        metrics = json.load(fh)
    os.makedirs(args.out_dir, exist_ok=True)
    p1 = chart_tokens_per_task(metrics, os.path.join(args.out_dir, "chart_tokens_to_first_correct.png"))
    p2 = chart_cspm(metrics, os.path.join(args.out_dir, "chart_cspm.png"))
    print("Wrote:")
    print(" ", p1)
    print(" ", p2)


if __name__ == "__main__":
    main()
