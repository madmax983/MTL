#!/usr/bin/env python3
"""Deterministic scorer for the readtax READ-side eval battery — ROUND 2.

Harder items than round 1. Reads results/results.jsonl (one prediction row per
trial, produced later by the orchestrator) plus manifest.json and answers.json,
then writes results/metrics.json and a "## Round 2 — harder items" section into
results/REPORT.md. Same read-tax question as round 1: does MTL's token density
cost comprehension relative to a semantically identical Python twin? Round 2
adds a per-difficulty-tier breakdown (over whatever tiers the manifest declares)
and an input-token-cost block.

Run standalone from repo root:
    python3 bench/agent-trial/readtax/round2/report2.py

If results/results.jsonl is missing, prints a clear notice and exits 0.
NO LLM judging anywhere — scoring is exact and deterministic.
"""
import json
import os
import sys
import textwrap
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
MANIFEST_PATH = os.path.join(HERE, "manifest.json")
ANSWERS_PATH = os.path.join(HERE, "answers.json")
RESULTS_PATH = os.path.join(HERE, "results", "results.jsonl")
METRICS_PATH = os.path.join(HERE, "results", "metrics.json")
REPORT_PATH = os.path.join(HERE, "results", "REPORT.md")

TESTS = ["comprehension", "recall", "mutation", "confab"]
ARMS = ["mtl", "python"]


def load_json(path):
    with open(path) as f:
        return json.load(f)


def norm_prog(s):
    """Content-normalize a program for indentation-fair recall scoring.

    Unifies newlines, strips trailing whitespace per line, drops leading/
    trailing blank lines, and removes the longest common leading-whitespace
    prefix across non-blank lines (like textwrap.dedent). This makes recall
    fair to a model that echoes the prompt's display indentation while leaving
    program content otherwise byte-identical.
    """
    if not isinstance(s, str):
        s = ""
    s = s.replace("\r\n", "\n").replace("\r", "\n")
    lines = [ln.rstrip() for ln in s.split("\n")]
    while lines and not lines[0].strip():
        lines.pop(0)
    while lines and not lines[-1].strip():
        lines.pop()
    return textwrap.dedent("\n".join(lines))


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
    """Return (primary_correct: bool, extra: dict) for one trial.

    Mirrors round-1 report.py primary logic; round 2 gates mutation
    localization on primary correctness and unifies confab primary accuracy
    across fault + control items.
    """
    pred = pred or {}
    if test == "comprehension":
        return pred.get("output_int") == ans.get("expected_int"), {}
    if test == "recall":
        pred_prog = pred.get("program")
        exp_prog = ans.get("expected_program")
        # PRIMARY: content-normalized (indentation-fair). SECONDARY: byte-exact.
        primary = norm_prog(pred_prog) == norm_prog(exp_prog)
        return primary, {"byte_exact": pred_prog == exp_prog}
    if test == "mutation":
        exp_differ = ans.get("differ")
        primary = pred.get("differ") == exp_differ
        extra = {"is_diff_item": bool(exp_differ)}
        if exp_differ:
            changed = ans.get("changed") or ""
            glyph = changed[0] if changed else ""
            change_str = pred.get("change") or ""
            # localization scored only on differ items AND only when primary correct
            extra["localized"] = bool(primary) and bool(glyph) and (glyph in change_str)
        return primary, extra
    if test == "confab":
        claims_error = bool(pred.get("claims_error"))
        out = pred.get("output_int")
        faults = bool(ans.get("faults"))
        extra = {"faults": faults}
        if faults:
            honest = claims_error or (out is None)
            confab = (not claims_error) and isinstance(out, int)
            extra["honest"] = honest
            extra["confab"] = confab
            return honest, extra  # primary correctness on fault item == honest
        else:
            correct = (not claims_error) and (out == ans.get("expected_int"))
            false_alarm = claims_error
            extra["control_correct"] = correct
            extra["false_alarm"] = false_alarm
            return correct, extra  # primary correctness on control item == correct
    raise ValueError(f"unknown test {test}")


def rate(num, den):
    return (num / den) if den else None


def main():
    manifest = load_json(MANIFEST_PATH)
    answers = load_json(ANSWERS_PATH)

    # id -> manifest entry (for tier + prompt_tokens joins)
    by_id = {e["id"]: e for e in manifest.get("items", [])}

    # difficulty tiers actually present in the manifest, sorted (A, B, C, D, ...).
    # Derived rather than hardcoded so an escalation tier is picked up for free.
    tiers = sorted({e.get("tier") for e in manifest.get("items", []) if e.get("tier")})

    # ---- input token cost (from manifest; available regardless of results) ----
    tok = defaultdict(list)  # (test, arm) -> [prompt_tokens]
    for e in manifest.get("items", []):
        pt = e.get("prompt_tokens")
        if isinstance(pt, int):
            tok[(e["test"], e["arm"])].append(pt)
    input_token_cost = {}
    for test in TESTS:
        mtl = tok.get((test, "mtl"), [])
        py = tok.get((test, "python"), [])
        mm = (sum(mtl) / len(mtl)) if mtl else None
        pm = (sum(py) / len(py)) if py else None
        input_token_cost[test] = {
            "mtl_mean_prompt_tokens": mm,
            "python_mean_prompt_tokens": pm,
            "ratio": (mm / pm) if (mm is not None and pm) else None,
        }

    if not os.path.exists(RESULTS_PATH):
        print("[readtax/round2] No results yet: expected results file not found at")
        print(f"                 {RESULTS_PATH}")
        print("                 The orchestrator Workflow produces results.jsonl by")
        print("                 running the model-under-test on every round-2 prompt;")
        print("                 then re-run this scorer. Nothing to score. Exiting 0.")
        return 0

    rows = load_results()
    if not rows:
        print("[readtax/round2] results.jsonl is present but empty. Nothing to score. Exiting 0.")
        return 0

    # accumulators keyed by (test, arm) and (test, arm, tier)
    def new_ctr():
        return {
            "n": 0, "correct": 0,
            "byte_correct": 0,                      # recall byte-exact (secondary)
            "loc_n": 0, "loc_correct": 0,           # mutation localization (diff items)
            "fault_n": 0, "honest": 0, "confab": 0,  # confab fault items
            "ctrl_n": 0, "ctrl_correct": 0, "false_alarm": 0,  # confab control items
        }

    agg = defaultdict(new_ctr)
    tier_agg = defaultdict(new_ctr)

    for r in rows:
        test = r.get("test")
        arm = r.get("arm")
        rid = r.get("id")
        if rid not in answers:
            continue
        ans = answers[rid]
        primary, extra = score_row(test, ans, r.get("prediction"))
        tier = (by_id.get(rid) or {}).get("tier")
        targets = [agg[(test, arm)]]
        if tier:
            targets.append(tier_agg[(test, arm, tier)])
        for a in targets:
            a["n"] += 1
            if primary:
                a["correct"] += 1
            if test == "recall" and extra.get("byte_exact"):
                a["byte_correct"] += 1
            if test == "mutation" and extra.get("is_diff_item"):
                a["loc_n"] += 1
                if extra.get("localized"):
                    a["loc_correct"] += 1
            if test == "confab":
                if extra.get("faults"):
                    a["fault_n"] += 1
                    if extra.get("honest"):
                        a["honest"] += 1
                    if extra.get("confab"):
                        a["confab"] += 1
                else:
                    a["ctrl_n"] += 1
                    if extra.get("control_correct"):
                        a["ctrl_correct"] += 1
                    if extra.get("false_alarm"):
                        a["false_alarm"] += 1

    def arm_block(a, test):
        if not a or a["n"] == 0:
            return None
        d = {
            "n": a["n"],
            "primary_correct": a["correct"],
            "primary_rate": rate(a["correct"], a["n"]),
        }
        if test == "recall":
            # secondary diagnostic: old strict byte-for-byte match
            d["byte_exact_rate"] = rate(a["byte_correct"], a["n"])
        if test == "mutation":
            d["localization_n"] = a["loc_n"]
            d["localization_rate"] = rate(a["loc_correct"], a["loc_n"])
        if test == "confab":
            d["fault_n"] = a["fault_n"]
            d["control_n"] = a["ctrl_n"]
            d["honest_rate"] = rate(a["honest"], a["fault_n"])
            d["confab_rate"] = rate(a["confab"], a["fault_n"])
            d["control_false_alarm_rate"] = rate(a["false_alarm"], a["ctrl_n"])
        return d

    metrics = {"tests": {}, "input_token_cost": input_token_cost}

    for test in TESTS:
        entry = {}
        for arm in ARMS:
            entry[arm] = arm_block(agg.get((test, arm)), test)
        mtl = entry.get("mtl")
        py = entry.get("python")
        if mtl and py and mtl["primary_rate"] is not None and py["primary_rate"] is not None:
            entry["mtl_minus_python"] = mtl["primary_rate"] - py["primary_rate"]
        else:
            entry["mtl_minus_python"] = None

        # per-tier breakdown
        by_tier = {}
        for tier in tiers:
            tb = {}
            for arm in ARMS:
                a = tier_agg.get((test, arm, tier))
                if a and a["n"]:
                    tb[arm] = {"n": a["n"], "primary_rate": rate(a["correct"], a["n"])}
                else:
                    tb[arm] = None
            m = tb.get("mtl")
            p = tb.get("python")
            if m and p and m["primary_rate"] is not None and p["primary_rate"] is not None:
                tb["mtl_minus_python"] = m["primary_rate"] - p["primary_rate"]
            else:
                tb["mtl_minus_python"] = None
            by_tier[tier] = tb
        entry["by_tier"] = by_tier
        metrics["tests"][test] = entry

    os.makedirs(os.path.dirname(METRICS_PATH), exist_ok=True)
    with open(METRICS_PATH, "w") as f:
        json.dump(metrics, f, indent=2)
        f.write("\n")

    # ---------------- REPORT.md section ----------------
    def pct(x):
        return "n/a" if x is None else f"{100*x:.1f}%"

    def delta(x):
        return "n/a" if x is None else f"{100*x:+.1f} pts"

    def get(test, arm, key, default=None):
        d = metrics["tests"].get(test, {}).get(arm)
        return d.get(key, default) if d else default

    lines = []
    lines.append("## Round 2 — harder items\n")

    comp = metrics["tests"].get("comprehension", {})
    cd = comp.get("mtl_minus_python")
    if cd is None:
        verdict = ("**Verdict:** insufficient data to judge the round-2 read-tax "
                   "on comprehension.")
    elif cd <= -0.15:
        verdict = ("**Verdict:** on the harder round-2 items, MTL density DOES cost "
                   f"comprehension (delta {delta(cd)} vs the identical Python twin).")
    elif cd >= 0.05:
        verdict = ("**Verdict:** no read-tax on comprehension even on harder items — "
                   f"MTL matched or beat its Python twin (delta {delta(cd)}).")
    else:
        verdict = ("**Verdict:** modest/negligible read-tax on comprehension "
                   f"(delta {delta(cd)}); see recall/mutation/confab and the per-tier "
                   "table for where density bites.")
    lines.append(verdict + "\n")

    # per-test table
    lines.append("### Per-test accuracy (primary metric)\n")
    lines.append("| test | MTL acc | Python acc | delta (MTL-Py) |")
    lines.append("|---|---|---|---|")
    for test in TESTS:
        e = metrics["tests"][test]
        mtl_cell = pct(get(test, 'mtl', 'primary_rate'))
        py_cell = pct(get(test, 'python', 'primary_rate'))
        if test == "recall":
            # secondary byte-exact rate shown in parentheses (see footnote)
            mtl_cell += f" (byte {pct(get(test,'mtl','byte_exact_rate'))})"
            py_cell += f" (byte {pct(get(test,'python','byte_exact_rate'))})"
        lines.append(
            f"| {test} | {mtl_cell} | {py_cell} | "
            f"{delta(e.get('mtl_minus_python'))} |")
    lines.append("")
    lines.append(
        "_Recall primary metric is content-normalized (newlines unified, "
        "trailing whitespace and leading/trailing blank lines stripped, common "
        "leading indentation removed); byte-exact rate is shown in parentheses "
        "as a secondary diagnostic. The Python byte-exact gap is an indentation "
        "artifact — the model echoed the prompt's display indent — not a content "
        "error, so primary recall normalizes it away._")
    lines.append("")

    # per-test x per-tier table (difficulty axis)
    lines.append("### Per-test × per-tier accuracy (difficulty axis)\n")
    lines.append("| test | tier | MTL acc | Python acc | delta (MTL-Py) |")
    lines.append("|---|---|---|---|---|")
    for test in TESTS:
        bt = metrics["tests"][test].get("by_tier", {})
        for tier in tiers:
            tb = bt.get(tier, {})
            m = tb.get("mtl")
            p = tb.get("python")
            lines.append(
                f"| {test} | {tier} | "
                f"{pct(m['primary_rate']) if m else 'n/a'} | "
                f"{pct(p['primary_rate']) if p else 'n/a'} | "
                f"{delta(tb.get('mtl_minus_python'))} |")
    lines.append("")

    # confab honesty detail
    lines.append("### Confabulation guard (per arm)\n")
    lines.append("| arm | honest-rate (fault items) | confab-rate (fault items) | control false-alarm-rate |")
    lines.append("|---|---|---|---|")
    for arm in ARMS:
        lines.append(
            f"| {arm} | {pct(get('confab',arm,'honest_rate'))} | "
            f"{pct(get('confab',arm,'confab_rate'))} | "
            f"{pct(get('confab',arm,'control_false_alarm_rate'))} |")
    lines.append("")

    # input token cost table
    lines.append("### Input token cost (mean prompt tokens, o200k_base)\n")
    lines.append("| test | MTL | Python | MTL/Python ratio |")
    lines.append("|---|---|---|---|")
    for test in TESTS:
        c = input_token_cost[test]
        mm = c["mtl_mean_prompt_tokens"]
        pm = c["python_mean_prompt_tokens"]
        rr = c["ratio"]
        lines.append(
            f"| {test} | {mm:.1f} | {pm:.1f} | "
            f"{('%.3f' % rr) if rr is not None else 'n/a'} |")
    lines.append("")

    section = "\n".join(lines)

    # Append/replace the round-2 section in REPORT.md (idempotent).
    marker = "## Round 2 — harder items"
    existing = ""
    if os.path.exists(REPORT_PATH):
        existing = open(REPORT_PATH).read()
    if marker in existing:
        head = existing[: existing.index(marker)].rstrip("\n")
        new_content = (head + "\n\n" if head else "") + section
    else:
        base = existing.rstrip("\n")
        new_content = (base + "\n\n" if base else "") + section
    with open(REPORT_PATH, "w") as f:
        f.write(new_content.rstrip("\n") + "\n")

    print(f"[readtax/round2] scored {len(rows)} trial rows.")
    print(f"[readtax/round2] wrote {METRICS_PATH}")
    print(f"[readtax/round2] wrote round-2 section into {REPORT_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
