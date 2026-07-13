#!/usr/bin/env python3
"""Deterministic scorer for the readtax READ-side eval battery.

Reads results/results.jsonl (produced later by the orchestrator Workflow) and
answers.json, then writes results/metrics.json and results/REPORT.md with a
"read-tax verdict": does MTL's token density cost comprehension relative to a
semantically identical Python twin?

Run standalone:
    python3 bench/agent-trial/readtax/report.py

If results/results.jsonl is missing, prints a clear message and exits 0.
"""
import json
import os
import sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
ANSWERS_PATH = os.path.join(HERE, "answers.json")
RESULTS_PATH = os.path.join(HERE, "results", "results.jsonl")
METRICS_PATH = os.path.join(HERE, "results", "metrics.json")
REPORT_PATH = os.path.join(HERE, "results", "REPORT.md")

TESTS = ["comprehension", "recall", "mutation", "confab"]
ARMS = ["mtl", "python"]


def load_answers():
    with open(ANSWERS_PATH) as f:
        return json.load(f)


def load_results():
    rows = []
    with open(RESULTS_PATH) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    return rows


def score_row(test, ans, pred):
    """Return (primary_correct: bool, extra: dict) for one trial."""
    pred = pred or {}
    if test == "comprehension":
        return pred.get("output_int") == ans.get("expected_int"), {}
    if test == "recall":
        return pred.get("program") == ans.get("expected_program"), {}
    if test == "mutation":
        exp_differ = ans.get("differ")
        primary = pred.get("differ") == exp_differ
        extra = {}
        if exp_differ:
            # localization: expected changed glyph char appears in `change`
            changed = ans.get("changed") or ""
            glyph = changed[0] if changed else ""
            change_str = pred.get("change") or ""
            extra["localized"] = bool(glyph) and (glyph in change_str)
            extra["is_diff_item"] = True
        else:
            extra["is_diff_item"] = False
        return primary, extra
    if test == "confab":
        claims_error = bool(pred.get("claims_error"))
        out = pred.get("output_int")
        honest = claims_error or (out is None)
        confab = (not claims_error) and isinstance(out, int)
        return honest, {"confab": confab}
    raise ValueError(f"unknown test {test}")


def main():
    answers = load_answers()

    if not os.path.exists(RESULTS_PATH):
        print("[readtax] No results yet: expected results file not found at")
        print(f"          {RESULTS_PATH}")
        print("          The orchestrator Workflow produces results.jsonl by")
        print("          running the model-under-test on every prompt; then")
        print("          re-run this scorer. Nothing to score. Exiting 0.")
        return 0

    rows = load_results()
    if not rows:
        print("[readtax] results.jsonl is present but empty. Nothing to score. Exiting 0.")
        return 0

    # accumulate: (test, arm) -> counters
    agg = defaultdict(lambda: {
        "n": 0, "correct": 0,
        "loc_n": 0, "loc_correct": 0,   # mutation localization (diff items only)
        "confab_n": 0, "confab_count": 0,  # confab confabulations
    })

    for r in rows:
        test = r.get("test")
        arm = r.get("arm")
        rid = r.get("id")
        if rid not in answers:
            # tolerate stray rows; skip
            continue
        ans = answers[rid]
        primary, extra = score_row(test, ans, r.get("prediction"))
        a = agg[(test, arm)]
        a["n"] += 1
        if primary:
            a["correct"] += 1
        if test == "mutation" and extra.get("is_diff_item"):
            a["loc_n"] += 1
            if extra.get("localized"):
                a["loc_correct"] += 1
        if test == "confab":
            a["confab_n"] += 1
            if extra.get("confab"):
                a["confab_count"] += 1

    def rate(num, den):
        return (num / den) if den else None

    # build metrics
    metrics = {"tests": {}, "totals": {}}
    for test in TESTS:
        entry = {}
        for arm in ARMS:
            a = agg.get((test, arm))
            if not a or a["n"] == 0:
                entry[arm] = None
                continue
            d = {
                "n": a["n"],
                "primary_correct": a["correct"],
                "primary_rate": rate(a["correct"], a["n"]),
            }
            if test == "mutation":
                d["localization_n"] = a["loc_n"]
                d["localization_rate"] = rate(a["loc_correct"], a["loc_n"])
            if test == "confab":
                d["confabulations"] = a["confab_count"]
                d["honesty_rate"] = rate(a["correct"], a["n"])
                d["confabulation_rate"] = rate(a["confab_count"], a["confab_n"])
            entry[arm] = d
        # delta (mtl - python) on the primary rate when both present
        mtl = entry.get("mtl")
        py = entry.get("python")
        if mtl and py and mtl["primary_rate"] is not None and py["primary_rate"] is not None:
            entry["mtl_minus_python"] = mtl["primary_rate"] - py["primary_rate"]
        else:
            entry["mtl_minus_python"] = None
        metrics["tests"][test] = entry

    os.makedirs(os.path.dirname(METRICS_PATH), exist_ok=True)
    with open(METRICS_PATH, "w") as f:
        json.dump(metrics, f, indent=2)
        f.write("\n")

    # ---- REPORT.md ----
    def pct(x):
        return "n/a" if x is None else f"{100*x:.1f}%"

    def delta(x):
        return "n/a" if x is None else f"{100*x:+.1f} pts"

    lines = []
    lines.append("# READ-tax verdict\n")
    lines.append(
        "Both arms answer the *same* item: an MTL program (with the quickref "
        "embedded) vs. a semantically identical Python twin (no quickref). "
        "`mtl - python` is the read-tax delta; a large negative delta means "
        "MTL's token density costs comprehension.\n")

    def get(test, arm, key, default=None):
        e = metrics["tests"].get(test, {})
        d = e.get(arm)
        if not d:
            return default
        return d.get(key, default)

    # headline
    comp = metrics["tests"].get("comprehension", {})
    rec = metrics["tests"].get("recall", {})
    conf = metrics["tests"].get("confab", {})
    lines.append("## Headline\n")
    lines.append(
        f"- **Comprehension accuracy** — MTL {pct(get('comprehension','mtl','primary_rate'))} "
        f"vs Python {pct(get('comprehension','python','primary_rate'))} "
        f"(delta {delta(comp.get('mtl_minus_python'))}).")
    lines.append(
        f"- **Verbatim recall exact-match** — MTL {pct(get('recall','mtl','primary_rate'))} "
        f"vs Python {pct(get('recall','python','primary_rate'))} "
        f"(delta {delta(rec.get('mtl_minus_python'))}). "
        "This is the BPE-dense stress case.")
    lines.append(
        f"- **Confabulation rate on faulting items** — MTL "
        f"{pct(get('confab','mtl','confabulation_rate'))} "
        f"vs Python {pct(get('confab','python','confabulation_rate'))} "
        "(lower is better; a confident concrete answer on a program that "
        "faults is a confabulation).")
    lines.append("")

    lines.append("## Per-test, per-arm\n")
    lines.append("| test | arm | n | primary | extra |")
    lines.append("|---|---|---|---|---|")
    for test in TESTS:
        for arm in ARMS:
            d = metrics["tests"][test].get(arm)
            if not d:
                lines.append(f"| {test} | {arm} | 0 | n/a | (no rows) |")
                continue
            extra = ""
            if test == "mutation":
                extra = f"localization {pct(d.get('localization_rate'))} (n={d.get('localization_n')})"
            elif test == "confab":
                extra = (f"honesty {pct(d.get('honesty_rate'))}, "
                         f"confab {pct(d.get('confabulation_rate'))} "
                         f"({d.get('confabulations')} of {d['n']})")
            lines.append(
                f"| {test} | {arm} | {d['n']} | {pct(d['primary_rate'])} | {extra} |")
    lines.append("")

    lines.append("## Read-tax delta (MTL - Python), primary metric\n")
    lines.append("| test | mtl | python | delta |")
    lines.append("|---|---|---|---|")
    for test in TESTS:
        e = metrics["tests"][test]
        lines.append(
            f"| {test} | {pct(get(test,'mtl','primary_rate'))} | "
            f"{pct(get(test,'python','primary_rate'))} | "
            f"{delta(e.get('mtl_minus_python'))} |")
    lines.append("")

    # verdict sentence
    cd = comp.get("mtl_minus_python")
    if cd is None:
        verdict = "Insufficient data to judge the read-tax."
    elif cd <= -0.15:
        verdict = ("MTL density DOES cost comprehension: the model misreads MTL "
                   "materially more often than the identical Python twin.")
    elif cd >= 0.05:
        verdict = ("No read-tax on comprehension: MTL matched or beat its Python twin.")
    else:
        verdict = ("Modest/negligible read-tax on comprehension; check recall and "
                   "confabulation rows for the density penalty.")
    lines.append("## Verdict\n")
    lines.append(verdict + "\n")

    with open(REPORT_PATH, "w") as f:
        f.write("\n".join(lines))

    print(f"[readtax] scored {len(rows)} trial rows.")
    print(f"[readtax] wrote {METRICS_PATH}")
    print(f"[readtax] wrote {REPORT_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
