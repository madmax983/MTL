#!/usr/bin/env python3
"""Pareto frontier for the ICL-preamble ablation (issue #73).

Reads results/metrics.json (from aggregate.py) and computes the Pareto frontier
in the objective space (maximize solve_rate, minimize preamble_tokens). Emits:
  - results/pareto.json  the frontier + per-variant points
  - results/pareto.png   solve_rate (y) vs preamble_tokens (x), Pareto set marked

Usage:  python3 pareto.py [--metrics <path>] [--out-json <path>] [--out-png <path>]
"""
from __future__ import annotations

import argparse
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
VARIANT_TOKENS = os.path.join(HERE, "variant_tokens.json")

# Full-quickref reference (the baseline the ablation compresses against).
REFERENCE_VARIANT = "v1_full"


def load_points(metrics_path: str) -> list[dict]:
    with open(metrics_path, encoding="utf-8") as f:
        metrics = json.load(f)
    # Preamble-token x-coordinate: mean_preamble_tokens from metrics (per-cell,
    # so v5 uses its per-task mean automatically).
    points = []
    for variant, m in metrics.items():
        points.append({
            "variant": variant,
            "preamble_tokens": m.get("mean_preamble_tokens"),
            "solve_rate": m.get("solve_rate"),
            "median_tokens_to_first_correct": m.get("median_tokens_to_first_correct"),
        })
    return points


def pareto_frontier(points: list[dict]) -> list[str]:
    """Non-dominated set: maximize solve_rate, minimize preamble_tokens.
    A point p is dominated if some q is >= on solve_rate AND <= on tokens, with
    at least one strict."""
    frontier = []
    for p in points:
        if p["preamble_tokens"] is None or p["solve_rate"] is None:
            continue
        dominated = False
        for q in points:
            if q is p or q["preamble_tokens"] is None or q["solve_rate"] is None:
                continue
            better_eq = (q["solve_rate"] >= p["solve_rate"]
                         and q["preamble_tokens"] <= p["preamble_tokens"])
            strictly = (q["solve_rate"] > p["solve_rate"]
                        or q["preamble_tokens"] < p["preamble_tokens"])
            if better_eq and strictly:
                dominated = True
                break
        if not dominated:
            frontier.append(p["variant"])
    return frontier


def render_png(points: list[dict], frontier: set[str], out_png: str) -> None:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(figsize=(8, 5.5))
    valid = [p for p in points
             if p["preamble_tokens"] is not None and p["solve_rate"] is not None]

    # Frontier line (sorted by tokens ascending).
    fr = sorted((p for p in valid if p["variant"] in frontier),
                key=lambda p: p["preamble_tokens"])
    if len(fr) >= 2:
        ax.plot([p["preamble_tokens"] for p in fr],
                [p["solve_rate"] for p in fr],
                "--", color="tab:green", zorder=1, label="Pareto frontier")

    for p in valid:
        on_front = p["variant"] in frontier
        is_ref = p["variant"] == REFERENCE_VARIANT
        ax.scatter(p["preamble_tokens"], p["solve_rate"],
                   s=170 if on_front else 90,
                   marker="*" if is_ref else ("o" if on_front else "x"),
                   color="tab:green" if on_front else "tab:red",
                   edgecolors="black" if on_front else "none",
                   zorder=3)
        ax.annotate(p["variant"],
                    (p["preamble_tokens"], p["solve_rate"]),
                    textcoords="offset points", xytext=(7, 5), fontsize=9)

    ax.set_xlabel("preamble tokens (o200k_base, mean per cell)")
    ax.set_ylabel("solve rate")
    ax.set_title("ICL-preamble ablation — solve rate vs preamble tokens (issue #73)")
    ax.grid(True, alpha=0.3)
    ax.set_ylim(-0.05, 1.05)
    ax.legend(loc="lower right")
    fig.tight_layout()
    fig.savefig(out_png, dpi=130)
    plt.close(fig)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--metrics", default=os.path.join(HERE, "results", "metrics.json"))
    ap.add_argument("--out-json", default=os.path.join(HERE, "results", "pareto.json"))
    ap.add_argument("--out-png", default=os.path.join(HERE, "results", "pareto.png"))
    args = ap.parse_args()

    if not os.path.isfile(args.metrics):
        raise SystemExit(f"no metrics file: {args.metrics} (run aggregate.py first)")

    points = load_points(args.metrics)
    frontier = pareto_frontier(points)
    result = {
        "objective": {"maximize": "solve_rate", "minimize": "preamble_tokens"},
        "reference_variant": REFERENCE_VARIANT,
        "pareto_frontier": frontier,
        "points": points,
    }
    os.makedirs(os.path.dirname(args.out_json), exist_ok=True)
    with open(args.out_json, "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2)
        f.write("\n")
    print(json.dumps(result, indent=2))

    try:
        render_png(points, set(frontier), args.out_png)
        print(f"\nwrote {args.out_json} and {args.out_png}")
    except ImportError:
        print(f"\nwrote {args.out_json}; matplotlib missing, skipped PNG "
              f"(pip3 install matplotlib)")


if __name__ == "__main__":
    main()
