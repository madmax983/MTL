#!/usr/bin/env python3
"""Report generator for the MTL agent-writability trial (T_agent-trial).

Reads per-attempt record JSON files, groups them into cells (task, arm, trial),
computes the trial metrics (A-F), and writes a machine-readable metrics.json and
a human-readable REPORT.md. A compact summary is also printed to stdout.

CLI:
  python3 report.py --records-dir <dir> [--payload <payload.json>]
                    [--out <REPORT.md>] [--json-out <metrics.json>]

Defaults are resolved relative to this script's location so the tool runs from
any working directory. The report uses o200k_base token counts as primary and
reports cl100k_base as a secondary column where convenient.
"""

import argparse
import json
import os
import statistics
import sys
from collections import defaultdict


ARMS = ("mtl", "python")
MAX_ATTEMPTS = 5
EXPECTED_TRIALS = 3

# Error-type buckets called out by the review.
STACK_TRACKING_FAULTS = ("Underflow", "TypeMismatch")


def _script_dir():
    return os.path.dirname(os.path.abspath(__file__))


def _default(*parts):
    return os.path.join(_script_dir(), *parts)


def parse_args(argv=None):
    p = argparse.ArgumentParser(
        description="Generate the MTL agent-writability trial report."
    )
    p.add_argument(
        "--records-dir",
        default=_default("results", "attempts"),
        help="Directory of per-attempt record JSON files.",
    )
    p.add_argument(
        "--payload",
        default=_default("payload.json"),
        help="payload.json holding quickref token costs.",
    )
    p.add_argument(
        "--out",
        default=_default("results", "REPORT.md"),
        help="Path to write the human-readable REPORT.md.",
    )
    p.add_argument(
        "--json-out",
        default=_default("results", "metrics.json"),
        help="Path to write the machine-readable metrics.json.",
    )
    return p.parse_args(argv)


def load_records(records_dir):
    """Load every *.json record file. Returns (records, load_errors).

    Robust to a missing directory and to individual malformed/partial files:
    a bad file is skipped and noted rather than crashing the run.
    """
    records = []
    load_errors = []
    if not os.path.isdir(records_dir):
        load_errors.append("records-dir does not exist: %s" % records_dir)
        return records, load_errors
    for name in sorted(os.listdir(records_dir)):
        if not name.endswith(".json"):
            continue
        path = os.path.join(records_dir, name)
        if not os.path.isfile(path):
            continue
        try:
            with open(path, "r") as fh:
                obj = json.load(fh)
        except (ValueError, OSError) as exc:
            load_errors.append("could not read %s: %s" % (name, exc))
            continue
        if not isinstance(obj, dict):
            load_errors.append("not a JSON object: %s" % name)
            continue
        # Minimum viable fields for grouping.
        if not all(k in obj for k in ("task", "arm", "trial", "attempt")):
            load_errors.append("missing key fields: %s" % name)
            continue
        obj["_file"] = name
        records.append(obj)
    return records, load_errors


def _int(rec, key, default=0):
    val = rec.get(key, default)
    try:
        return int(val)
    except (TypeError, ValueError):
        return default


def build_cells(records):
    """Group records into cells keyed by (task, arm, trial)."""
    grouped = defaultdict(list)
    for rec in records:
        key = (str(rec["task"]), str(rec["arm"]), _int(rec, "trial"))
        grouped[key].append(rec)

    cells = {}
    for key, recs in grouped.items():
        recs = sorted(recs, key=lambda r: _int(r, "attempt"))
        solved = any(bool(r.get("ok")) for r in recs)
        attempts_to_solve = None
        for r in recs:
            if bool(r.get("ok")):
                attempts_to_solve = _int(r, "attempt")
                break

        tokens_o = [_int(r, "program_tokens_o200k") for r in recs]

        if attempts_to_solve is not None:
            # Sum program tokens over attempts 1..attempts_to_solve inclusive.
            first_correct_tokens = sum(
                _int(r, "program_tokens_o200k")
                for r in recs
                if _int(r, "attempt") <= attempts_to_solve
            )
            # The last (winning) attempt's program-only tokens.
            winning = next(r for r in recs if bool(r.get("ok")))
            program_only_tokens = _int(winning, "program_tokens_o200k")
        else:
            first_correct_tokens = None
            program_only_tokens = None

        cells[key] = {
            "task": key[0],
            "arm": key[1],
            "trial": key[2],
            "solved": solved,
            "attempts_used": len(recs),
            "attempts_to_solve": attempts_to_solve,
            "output_tokens_to_first_correct": first_correct_tokens,
            "total_output_tokens": sum(tokens_o),
            "program_only_tokens_first_correct": program_only_tokens,
            "records": recs,
        }
    return cells


def _mean(xs):
    return statistics.fmean(xs) if xs else None


def _median(xs):
    return statistics.median(xs) if xs else None


def compute_arm_metrics(cells_list):
    """Compute metrics A-F for one arm given a list of cell dicts."""
    total = len(cells_list)
    solved = [c for c in cells_list if c["solved"]]
    n_solved = len(solved)

    # A. P(correct within 5 attempts)
    p_correct = (n_solved / total) if total else None

    # B. Attempts-to-first-correct
    ats = [c["attempts_to_solve"] for c in solved]
    attempts_dist = {i: 0 for i in range(1, MAX_ATTEMPTS + 1)}
    for a in ats:
        if a in attempts_dist:
            attempts_dist[a] += 1
        else:
            attempts_dist[a] = attempts_dist.get(a, 0) + 1

    # C. Output tokens to first correct
    otfc = [c["output_tokens_to_first_correct"] for c in solved]

    # D. Correct-solutions-per-million-tokens
    total_output_all = sum(c["total_output_tokens"] for c in cells_list)
    cspm = (n_solved / total_output_all * 1e6) if total_output_all else None

    # E. program-only first-correct tokens (last winning attempt)
    program_only = [c["program_only_tokens_first_correct"] for c in solved]

    # F. Error-type distribution over ALL non-ok attempts
    error_counts = defaultdict(int)
    for c in cells_list:
        for r in c["records"]:
            if not bool(r.get("ok")):
                et = r.get("error_type")
                et = "unknown" if et is None else str(et)
                error_counts[et] += 1

    # F. Repair efficacy: cells that failed attempt 1 but eventually solved.
    failed_first = [
        c for c in cells_list
        if c["records"] and not bool(c["records"][0].get("ok"))
    ]
    failed_first_then_solved = [c for c in failed_first if c["solved"]]
    repair_success_rate = (
        len(failed_first_then_solved) / len(failed_first)
        if failed_first else None
    )
    repair_mean_attempts = _mean(
        [c["attempts_to_solve"] for c in failed_first_then_solved]
    )

    return {
        "total_cells": total,
        "solved_cells": n_solved,
        "unsolved_cells": total - n_solved,
        "p_correct_within_5": p_correct,
        "attempts_to_first_correct_mean": _mean(ats),
        "attempts_to_first_correct_median": _median(ats),
        "attempts_to_first_correct_distribution": attempts_dist,
        "output_tokens_to_first_correct_mean": _mean(otfc),
        "output_tokens_to_first_correct_median": _median(otfc),
        "total_output_tokens_all_cells": total_output_all,
        "correct_solutions_per_million_tokens": cspm,
        "program_only_first_correct_median": _median(program_only),
        "program_only_first_correct_mean": _mean(program_only),
        "error_type_distribution": dict(error_counts),
        "stack_tracking_fault_count": sum(
            error_counts.get(k, 0) for k in STACK_TRACKING_FAULTS
        ),
        "parse_fault_count": error_counts.get("parse", 0),
        "wrong_output_fault_count": error_counts.get("wrong_output", 0),
        "repair": {
            "failed_attempt1_cells": len(failed_first),
            "failed_attempt1_then_solved": len(failed_first_then_solved),
            "repair_success_rate": repair_success_rate,
            "mean_attempts_among_repaired": repair_mean_attempts,
        },
    }


def compute_task_metrics(cells_list):
    """Per-task x arm breakdown: P(correct), tokens, cspm."""
    by_task = defaultdict(lambda: {a: [] for a in ARMS})
    for c in cells_list:
        by_task[c["task"]][c["arm"]].append(c)

    out = {}
    for task in sorted(by_task):
        out[task] = {}
        for arm in ARMS:
            arm_cells = by_task[task][arm]
            total = len(arm_cells)
            solved = [c for c in arm_cells if c["solved"]]
            total_out = sum(c["total_output_tokens"] for c in arm_cells)
            otfc = [c["output_tokens_to_first_correct"] for c in solved]
            out[task][arm] = {
                "total_cells": total,
                "solved_cells": len(solved),
                "p_correct_within_5": (len(solved) / total) if total else None,
                "output_tokens_to_first_correct_median": _median(otfc),
                "correct_solutions_per_million_tokens": (
                    len(solved) / total_out * 1e6 if total_out else None
                ),
                "trials_present": sorted({c["trial"] for c in arm_cells}),
            }
    return out


def compute_metrics(cells, payload, load_errors):
    cells_list = list(cells.values())
    by_arm = {arm: [c for c in cells_list if c["arm"] == arm] for arm in ARMS}

    quickref_o = _int(payload, "quickref_tokens_o200k")
    quickref_c = _int(payload, "quickref_tokens_cl100k")
    payload_missing = payload.get("_missing", False)

    arm_metrics = {arm: compute_arm_metrics(by_arm[arm]) for arm in ARMS}

    # E. Total-token accounting.
    token_accounting = {}
    for arm in ARMS:
        m = arm_metrics[arm]
        gen_repair_median = m["output_tokens_to_first_correct_median"]
        program_only_median = m["program_only_first_correct_median"]
        # Cold instruction cost: MTL pays the quickref; Python ~ 0.
        cold_cost = quickref_o if arm == "mtl" else 0
        cold_total = (
            (gen_repair_median + cold_cost)
            if gen_repair_median is not None else None
        )
        amortized = (
            (gen_repair_median + cold_cost / 10.0)
            if gen_repair_median is not None else None
        )
        token_accounting[arm] = {
            "program_only_median": program_only_median,
            "generation_plus_repair_median": gen_repair_median,
            "cold_instruction_cost_o200k": cold_cost,
            "cold_total_per_solve": cold_total,
            "amortized_over_10_tasks": amortized,
        }

    # Verdict: does the static (program-length) edge survive total accounting?
    mtl_static = arm_metrics["mtl"]["program_only_first_correct_median"]
    py_static = arm_metrics["python"]["program_only_first_correct_median"]
    mtl_cspm = arm_metrics["mtl"]["correct_solutions_per_million_tokens"]
    py_cspm = arm_metrics["python"]["correct_solutions_per_million_tokens"]
    mtl_cold_total = token_accounting["mtl"]["cold_total_per_solve"]
    py_cold_total = token_accounting["python"]["cold_total_per_solve"]

    cspm_ratio = (
        (mtl_cspm / py_cspm) if (mtl_cspm and py_cspm) else None
    )

    static_edge_mtl_lower = (
        (mtl_static is not None and py_static is not None and mtl_static < py_static)
    )
    survives = None
    if mtl_cspm is not None and py_cspm is not None:
        survives = mtl_cspm >= py_cspm

    verdict = {
        "static_program_median_mtl": mtl_static,
        "static_program_median_python": py_static,
        "static_edge_favors_mtl": static_edge_mtl_lower,
        "cspm_mtl": mtl_cspm,
        "cspm_python": py_cspm,
        "cspm_ratio_mtl_over_python": cspm_ratio,
        "cold_total_per_solve_mtl": mtl_cold_total,
        "cold_total_per_solve_python": py_cold_total,
        "static_edge_survives_total_accounting": survives,
    }

    # Data-completeness diagnostics.
    present_pairs = defaultdict(set)
    for c in cells_list:
        present_pairs[(c["task"], c["arm"])].add(c["trial"])
    underfilled = []
    for (task, arm), trials in sorted(present_pairs.items()):
        if len(trials) < EXPECTED_TRIALS:
            underfilled.append(
                {
                    "task": task,
                    "arm": arm,
                    "trials_present": sorted(trials),
                    "expected": EXPECTED_TRIALS,
                }
            )
    inprogress = [
        {"task": c["task"], "arm": c["arm"], "trial": c["trial"],
         "attempts_used": c["attempts_used"]}
        for c in cells_list
        if (not c["solved"]) and c["attempts_used"] < MAX_ATTEMPTS
    ]

    total_attempts = sum(c["attempts_used"] for c in cells_list)

    return {
        "meta": {
            "arms": list(ARMS),
            "max_attempts": MAX_ATTEMPTS,
            "expected_trials_per_task_arm": EXPECTED_TRIALS,
            "quickref_tokens_o200k": quickref_o,
            "quickref_tokens_cl100k": quickref_c,
            "payload_missing": payload_missing,
            "cells_found": len(cells_list),
            "attempts_found": total_attempts,
            "load_errors": load_errors,
            "underfilled_task_arms": underfilled,
            "in_progress_cells": inprogress,
        },
        "per_arm": arm_metrics,
        "per_task": compute_task_metrics(cells_list),
        "token_accounting": token_accounting,
        "verdict": verdict,
    }


# --------------------------------------------------------------------------
# Formatting helpers
# --------------------------------------------------------------------------

def fnum(x, nd=1):
    if x is None:
        return "n/a"
    if isinstance(x, float):
        return ("%.*f" % (nd, x)).rstrip("0").rstrip(".") if nd else "%d" % round(x)
    return str(x)


def fpct(x):
    return "n/a" if x is None else "%.1f%%" % (100.0 * x)


def render_report(metrics):
    meta = metrics["meta"]
    pa = metrics["per_arm"]
    ta = metrics["token_accounting"]
    v = metrics["verdict"]
    lines = []
    w = lines.append

    w("# MTL agent-writability trial — REPORT")
    w("")
    w("Primary token counts are **o200k_base**; cl100k is secondary.")
    w("")
    if meta["payload_missing"]:
        w("> NOTE: payload.json was missing; quickref token cost treated as 0.")
        w("")
    w("Cells found: **%d** | attempts found: **%d** | max attempts/cell: %d | "
      "expected trials per (task,arm): %d"
      % (meta["cells_found"], meta["attempts_found"], meta["max_attempts"],
         meta["expected_trials_per_task_arm"]))
    if meta["load_errors"]:
        w("")
        w("> Load warnings (%d): %s" % (len(meta["load_errors"]),
                                        "; ".join(meta["load_errors"][:10])))
    w("")

    # Headline
    w("## Headline (MTL vs Python)")
    w("")
    w("| Metric | MTL | Python |")
    w("|---|---|---|")
    w("| P(correct ≤ 5 attempts) | %s | %s |"
      % (fpct(pa["mtl"]["p_correct_within_5"]),
         fpct(pa["python"]["p_correct_within_5"])))
    w("| Median output tokens to first correct | %s | %s |"
      % (fnum(pa["mtl"]["output_tokens_to_first_correct_median"]),
         fnum(pa["python"]["output_tokens_to_first_correct_median"])))
    w("| Correct-solutions per million tokens | %s | %s |"
      % (fnum(pa["mtl"]["correct_solutions_per_million_tokens"], 2),
         fnum(pa["python"]["correct_solutions_per_million_tokens"], 2)))
    w("")
    if v["cspm_ratio_mtl_over_python"] is not None:
        w("**Correct-solutions-per-million-tokens ratio (MTL / Python): %s** "
          "(>1 favors MTL)." % fnum(v["cspm_ratio_mtl_over_python"], 3))
        w("")

    # A + B detail
    w("## A/B. Correctness and attempts-to-first-correct")
    w("")
    w("| Metric | MTL | Python |")
    w("|---|---|---|")
    w("| Total cells | %d | %d |"
      % (pa["mtl"]["total_cells"], pa["python"]["total_cells"]))
    w("| Solved cells | %d | %d |"
      % (pa["mtl"]["solved_cells"], pa["python"]["solved_cells"]))
    w("| P(correct ≤ 5) | %s | %s |"
      % (fpct(pa["mtl"]["p_correct_within_5"]),
         fpct(pa["python"]["p_correct_within_5"])))
    w("| Mean attempts to first correct | %s | %s |"
      % (fnum(pa["mtl"]["attempts_to_first_correct_mean"], 2),
         fnum(pa["python"]["attempts_to_first_correct_mean"], 2)))
    w("| Median attempts to first correct | %s | %s |"
      % (fnum(pa["mtl"]["attempts_to_first_correct_median"]),
         fnum(pa["python"]["attempts_to_first_correct_median"])))
    w("")
    w("Attempts-to-first-correct distribution (solved cells, by attempt index):")
    w("")
    w("| Attempt | MTL | Python |")
    w("|---|---|---|")
    keys = sorted(set(pa["mtl"]["attempts_to_first_correct_distribution"])
                  | set(pa["python"]["attempts_to_first_correct_distribution"]))
    for k in keys:
        w("| %s | %d | %d |"
          % (k,
             pa["mtl"]["attempts_to_first_correct_distribution"].get(k, 0),
             pa["python"]["attempts_to_first_correct_distribution"].get(k, 0)))
    w("")

    # C
    w("## C. Output tokens to first correct (§17 headline component)")
    w("")
    w("| Metric | MTL | Python |")
    w("|---|---|---|")
    w("| Median | %s | %s |"
      % (fnum(pa["mtl"]["output_tokens_to_first_correct_median"]),
         fnum(pa["python"]["output_tokens_to_first_correct_median"])))
    w("| Mean | %s | %s |"
      % (fnum(pa["mtl"]["output_tokens_to_first_correct_mean"], 2),
         fnum(pa["python"]["output_tokens_to_first_correct_mean"], 2)))
    w("")

    # D
    w("## D. Correct-solutions-per-million-tokens (headline, §10.6)")
    w("")
    w("Charges the full cost of failed repair attempts: "
      "`solved_cells / sum(total_output_tokens over ALL cells) * 1e6`.")
    w("")
    w("| Metric | MTL | Python |")
    w("|---|---|---|")
    w("| Solved cells | %d | %d |"
      % (pa["mtl"]["solved_cells"], pa["python"]["solved_cells"]))
    w("| Total output tokens (all cells) | %d | %d |"
      % (pa["mtl"]["total_output_tokens_all_cells"],
         pa["python"]["total_output_tokens_all_cells"]))
    w("| Correct-solutions per 1e6 tokens | %s | %s |"
      % (fnum(pa["mtl"]["correct_solutions_per_million_tokens"], 2),
         fnum(pa["python"]["correct_solutions_per_million_tokens"], 2)))
    w("| MTL / Python ratio | %s | |"
      % fnum(v["cspm_ratio_mtl_over_python"], 3))
    w("")

    # Per-task
    w("## Per-task × arm breakdown")
    w("")
    w("| Task | Arm | Cells | Solved | P(≤5) | Med tok→correct | Correct/1e6 tok | Trials |")
    w("|---|---|---|---|---|---|---|---|")
    for task in sorted(metrics["per_task"]):
        for arm in ARMS:
            t = metrics["per_task"][task][arm]
            w("| %s | %s | %d | %d | %s | %s | %s | %s |"
              % (task, arm, t["total_cells"], t["solved_cells"],
                 fpct(t["p_correct_within_5"]),
                 fnum(t["output_tokens_to_first_correct_median"]),
                 fnum(t["correct_solutions_per_million_tokens"], 2),
                 ",".join(str(x) for x in t["trials_present"]) or "-"))
    w("")

    # E
    w("## E. Total-token accounting (§10.4) and the verdict")
    w("")
    w("| Quantity | MTL (cold) | Python (cold) |")
    w("|---|---|---|")
    w("| Program-only median (winning attempt) | %s | %s |"
      % (fnum(ta["mtl"]["program_only_median"]),
         fnum(ta["python"]["program_only_median"])))
    w("| Generation+repair median | %s | %s |"
      % (fnum(ta["mtl"]["generation_plus_repair_median"]),
         fnum(ta["python"]["generation_plus_repair_median"])))
    w("| Cold instruction cost (quickref, o200k) | %s | %s |"
      % (fnum(ta["mtl"]["cold_instruction_cost_o200k"]),
         fnum(ta["python"]["cold_instruction_cost_o200k"])))
    w("| Cold total per solve | %s | %s |"
      % (fnum(ta["mtl"]["cold_total_per_solve"]),
         fnum(ta["python"]["cold_total_per_solve"])))
    w("| Amortized over 10 tasks | %s | %s |"
      % (fnum(ta["mtl"]["amortized_over_10_tasks"], 1),
         fnum(ta["python"]["amortized_over_10_tasks"], 1)))
    w("")
    w("Python's cold instruction cost is ~0: the model already knows Python and "
      "receives no language reference, whereas MTL pays the full quickref cost "
      "(%d o200k tokens) once per task cold. This is the asymmetry the review "
      "predicted." % meta["quickref_tokens_o200k"])
    w("")
    w("### Does MTL's static (program-length) token edge survive total-token accounting?")
    w("")
    if v["static_program_median_mtl"] is None or v["static_program_median_python"] is None:
        w("- Insufficient solved cells in one or both arms to judge the static edge.")
    else:
        edge_dir = ("MTL programs are shorter" if v["static_edge_favors_mtl"]
                    else "MTL programs are NOT shorter")
        w("- **Static edge:** median first-correct program tokens — MTL %s vs "
          "Python %s. %s."
          % (fnum(v["static_program_median_mtl"]),
             fnum(v["static_program_median_python"]), edge_dir))
    if v["cspm_mtl"] is not None and v["cspm_python"] is not None:
        w("- **Efficiency (charges failed repairs):** correct-solutions per 1e6 "
          "tokens — MTL %s vs Python %s (ratio %s)."
          % (fnum(v["cspm_mtl"], 2), fnum(v["cspm_python"], 2),
             fnum(v["cspm_ratio_mtl_over_python"], 3)))
    if v["cold_total_per_solve_mtl"] is not None and v["cold_total_per_solve_python"] is not None:
        w("- **Cold total per solve:** MTL %s vs Python %s."
          % (fnum(v["cold_total_per_solve_mtl"]),
             fnum(v["cold_total_per_solve_python"])))
    w("")
    if v["static_edge_survives_total_accounting"] is None:
        w("**Verdict: INDETERMINATE** — not enough data to decide.")
    elif v["static_edge_survives_total_accounting"]:
        w("**Verdict: the static edge SURVIVES.** On the efficiency metric that "
          "charges failed repairs, MTL is at least as good as Python "
          "(correct-solutions-per-million-tokens MTL ≥ Python).")
    else:
        w("**Verdict: the static edge does NOT survive total-token accounting.** "
          "Even where MTL programs are statically shorter, once wasted repair "
          "attempts (and, cold, the quickref instruction cost) are charged, "
          "Python yields more correct solutions per million tokens.")
    w("")

    # F
    w("## F. Error-type distribution and repair efficacy")
    w("")
    all_ets = sorted(set(pa["mtl"]["error_type_distribution"])
                     | set(pa["python"]["error_type_distribution"]))
    w("| error_type | MTL | Python |")
    w("|---|---|---|")
    for et in all_ets:
        w("| %s | %d | %d |"
          % (et,
             pa["mtl"]["error_type_distribution"].get(et, 0),
             pa["python"]["error_type_distribution"].get(et, 0)))
    w("")
    w("Stack-tracking faults (Underflow + TypeMismatch) — MTL %d, Python %d. "
      "parse — MTL %d, Python %d. wrong_output — MTL %d, Python %d. "
      "The review predicted stack-tracking failures dominate the MTL arm."
      % (pa["mtl"]["stack_tracking_fault_count"],
         pa["python"]["stack_tracking_fault_count"],
         pa["mtl"]["parse_fault_count"], pa["python"]["parse_fault_count"],
         pa["mtl"]["wrong_output_fault_count"],
         pa["python"]["wrong_output_fault_count"]))
    w("")
    w("### Repair efficacy (does stack-state-in-errors feedback fix failures?)")
    w("")
    w("| Metric | MTL | Python |")
    w("|---|---|---|")
    w("| Cells that failed attempt 1 | %d | %d |"
      % (pa["mtl"]["repair"]["failed_attempt1_cells"],
         pa["python"]["repair"]["failed_attempt1_cells"]))
    w("| ...that eventually solved | %d | %d |"
      % (pa["mtl"]["repair"]["failed_attempt1_then_solved"],
         pa["python"]["repair"]["failed_attempt1_then_solved"]))
    w("| Repair success rate | %s | %s |"
      % (fpct(pa["mtl"]["repair"]["repair_success_rate"]),
         fpct(pa["python"]["repair"]["repair_success_rate"])))
    w("| Mean attempts among repaired | %s | %s |"
      % (fnum(pa["mtl"]["repair"]["mean_attempts_among_repaired"], 2),
         fnum(pa["python"]["repair"]["mean_attempts_among_repaired"], 2)))
    w("")

    # Data completeness
    if meta["underfilled_task_arms"] or meta["in_progress_cells"] or meta["load_errors"]:
        w("## Data completeness")
        w("")
        if meta["underfilled_task_arms"]:
            w("Task/arms with fewer than %d trials:" % EXPECTED_TRIALS)
            for u in meta["underfilled_task_arms"]:
                w("- %s / %s: trials present %s (expected %d)"
                  % (u["task"], u["arm"], u["trials_present"], u["expected"]))
            w("")
        if meta["in_progress_cells"]:
            w("In-progress/unsolved cells with < %d attempts (counted, not "
              "solved): %d." % (MAX_ATTEMPTS, len(meta["in_progress_cells"])))
            w("")

    # Caveats
    w("## Caveats")
    w("")
    w("- **Cold-only, no warm arm.** The model-under-test is the session's "
      "Claude model. Only cold agents were tested — no fine-tuning or warmed-up "
      "(agent already fluent in MTL) arm was available. A warm MTL arm would pay "
      "no quickref cost and would likely raise pass@1, so the cold numbers are a "
      "lower bound for MTL.")
    w("- **Token-count proxy.** The output-token metric counts only the visible "
      "program the model emits, not hidden reasoning tokens. Both arms are "
      "measured identically so the comparison is fair, but absolute totals "
      "understate true generation cost.")
    w("- **Both arms treated identically** for token counting (o200k_base "
      "primary, cl100k_base secondary) and for the 5-attempt repair budget.")
    w("- TODO: add any further cold-only / warm-agent and tokcount-proxy "
      "caveats the reviewer wants to expand here.")
    w("")

    return "\n".join(lines) + "\n"


def render_stdout_summary(metrics):
    pa = metrics["per_arm"]
    v = metrics["verdict"]
    meta = metrics["meta"]
    lines = []
    w = lines.append
    w("MTL agent-writability trial — summary")
    w("cells=%d attempts=%d (payload_missing=%s)"
      % (meta["cells_found"], meta["attempts_found"], meta["payload_missing"]))
    w("")
    header = "%-28s %12s %12s" % ("metric", "MTL", "Python")
    w(header)
    w("-" * len(header))
    rows = [
        ("P(correct<=5)",
         fpct(pa["mtl"]["p_correct_within_5"]),
         fpct(pa["python"]["p_correct_within_5"])),
        ("median tok->first correct",
         fnum(pa["mtl"]["output_tokens_to_first_correct_median"]),
         fnum(pa["python"]["output_tokens_to_first_correct_median"])),
        ("mean tok->first correct",
         fnum(pa["mtl"]["output_tokens_to_first_correct_mean"], 1),
         fnum(pa["python"]["output_tokens_to_first_correct_mean"], 1)),
        ("correct-sol/1e6 tok",
         fnum(pa["mtl"]["correct_solutions_per_million_tokens"], 2),
         fnum(pa["python"]["correct_solutions_per_million_tokens"], 2)),
        ("program-only median",
         fnum(pa["mtl"]["program_only_first_correct_median"]),
         fnum(pa["python"]["program_only_first_correct_median"])),
        ("cold total per solve",
         fnum(metrics["token_accounting"]["mtl"]["cold_total_per_solve"]),
         fnum(metrics["token_accounting"]["python"]["cold_total_per_solve"])),
        ("stack-tracking faults",
         str(pa["mtl"]["stack_tracking_fault_count"]),
         str(pa["python"]["stack_tracking_fault_count"])),
        ("repair success rate",
         fpct(pa["mtl"]["repair"]["repair_success_rate"]),
         fpct(pa["python"]["repair"]["repair_success_rate"])),
    ]
    for name, m, p in rows:
        w("%-28s %12s %12s" % (name, m, p))
    w("")
    w("cspm ratio MTL/Python = %s" % fnum(v["cspm_ratio_mtl_over_python"], 3))
    if v["static_edge_survives_total_accounting"] is None:
        w("static edge survives total accounting: INDETERMINATE")
    else:
        w("static edge survives total accounting: %s"
          % ("YES" if v["static_edge_survives_total_accounting"] else "NO"))
    if meta["underfilled_task_arms"]:
        w("WARNING: %d (task,arm) have < %d trials"
          % (len(meta["underfilled_task_arms"]), EXPECTED_TRIALS))
    return "\n".join(lines) + "\n"


def load_payload(path):
    if not os.path.isfile(path):
        return {"_missing": True}
    try:
        with open(path, "r") as fh:
            data = json.load(fh)
        if not isinstance(data, dict):
            return {"_missing": True}
        data["_missing"] = False
        return data
    except (ValueError, OSError):
        return {"_missing": True}


def strip_records(metrics):
    # metrics.json should not embed raw records; they are not included anyway.
    return metrics


def main(argv=None):
    args = parse_args(argv)

    records, load_errors = load_records(args.records_dir)
    payload = load_payload(args.payload)
    cells = build_cells(records)
    metrics = compute_metrics(cells, payload, load_errors)

    report_md = render_report(metrics)
    summary = render_stdout_summary(metrics)

    # Ensure output dirs exist.
    for target in (args.out, args.json_out):
        d = os.path.dirname(os.path.abspath(target))
        if d and not os.path.isdir(d):
            os.makedirs(d, exist_ok=True)

    with open(args.json_out, "w") as fh:
        json.dump(strip_records(metrics), fh, indent=2, sort_keys=True)
        fh.write("\n")
    with open(args.out, "w") as fh:
        fh.write(report_md)

    sys.stdout.write(summary)
    sys.stdout.write("\nWrote %s and %s\n" % (args.out, args.json_out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
