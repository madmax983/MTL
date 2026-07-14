#!/usr/bin/env python3
"""Session-economics crossover harness for GitHub issue #45.

This is a DETERMINISTIC accounting harness. It does NOT re-run any LLM. It
reuses EXISTING measured token/success data from the tier-2 (`T_agent-trial`)
and tier-3 (`T_tier3-trial`) cold-agent trials and layers a session-size +
prompt-caching economic model on top.

Three arms are priced across session sizes N in {1,2,3,5,8,16}:
  (A) Python  (no cache)
  (B) MTL cold (no cache) -- quickref re-read as full input on every task
  (C) MTL cached          -- quickref written to cache once, cache-read after

It answers: N* = smallest N where mean cost-per-correct of (C) < (A).

Run from anywhere:
    python3 session_econ.py
or:
    cd bench/agent-trial/sessions && python3 session_econ.py

Regenerates:
    results/task_costs.json   -- per-task extracted cost table + provenance
    results/ledger.jsonl      -- raw per-(N,replicate,arm) token/dollar ledger
    results/curve.csv         -- per-N per-arm mean cost-per-correct (+bands)
    results/curve.png         -- crossover plot with N* annotated

Pure python3 + tokcount + matplotlib. Touches nothing under crates/.
"""

from __future__ import annotations

import csv
import glob
import json
import os
import random
import statistics
import sys

# ---------------------------------------------------------------------------
# Path plumbing (robust to CWD).
# ---------------------------------------------------------------------------
_HERE = os.path.dirname(os.path.abspath(__file__))          # .../sessions
_AGENT_TRIAL = os.path.dirname(_HERE)                        # .../agent-trial
_BENCH = os.path.dirname(_AGENT_TRIAL)                       # .../bench
_REPO = os.path.dirname(_BENCH)                              # repo root

# tokcount is a package at bench/tokcount; add bench/ to the path so
# `from tokcount.tokcount import count` resolves. (Importing from within the
# tokcount/ dir would shadow the package with tokcount.py, so we import from
# a neutral sys.path entry.)
if _BENCH not in sys.path:
    sys.path.insert(0, _BENCH)
from tokcount.tokcount import count, count_file  # noqa: E402

RESULTS_DIR = os.path.join(_HERE, "results")
QUICKREF_PATH = os.path.join(_REPO, "docs", "mtl-quickref.md")

# Fixed sweep parameters (deterministic).
SESSION_SIZES = [1, 2, 3, 5, 8, 16]
K_REPLICATES = 500
SEED = 4045
MAX_ATTEMPTS = 5  # both trials capped attempts at 5

# Canonical Python-arm host stub function names (tier-3), per tier3/PROTOCOL.md.
TIER3_PY_STUBS = [
    "read_line", "read_lines", "emit", "emit_int", "line_hit",
    "transform", "next_line", "end_p", "concat", "select",
]


def o200k(text: str) -> int:
    """o200k_base token count of a string (primary encoding for #45)."""
    return int(count(text)["o200k_base"])


def _tok_program(program: str) -> int:
    """Token count of an emitted program, stripping exactly one trailing
    newline -- mirrors tokcount.count_file and tier3/report.py's `tok`, applied
    uniformly to both arms so the output-token comparison stays fair. Tier-2's
    stored program_tokens_o200k were produced by the same convention."""
    if program.endswith("\n"):
        program = program[:-1]
    return int(count(program)["o200k_base"])


# ---------------------------------------------------------------------------
# Quickref prefix Q -- the MTL-only static cacheable prefix. MEASURED, not
# hardcoded.
# ---------------------------------------------------------------------------
def measure_quickref_tokens() -> int:
    return int(count_file(QUICKREF_PATH)["o200k_base"])


# ---------------------------------------------------------------------------
# Tier-2 extraction (10 tasks).
#   INPUT tokens:  tokcount(payload.json mtl_prompt / python_prompt)
#   OUTPUT tokens: results.jsonl program_tokens_o200k summed over attempts
#                  1..first-correct (all-attempts-to-first-correct); if a trial
#                  never solved, sum all its attempts (the tokens actually
#                  spent). Averaged over the 3 trials per (task,arm).
#   SUCCESS:       fraction of trials solved within 5 attempts.
# ---------------------------------------------------------------------------
def extract_tier2():
    payload = json.load(open(os.path.join(_AGENT_TRIAL, "payload.json")))
    prompts = {t["id"]: t for t in payload["tasks"]}

    recs = [json.loads(l) for l in open(
        os.path.join(_AGENT_TRIAL, "results", "results.jsonl"))]
    # group by (task, arm, trial)
    cells = {}
    for r in recs:
        key = (r["task"], r["arm"], int(r.get("trial", 1)))
        cells.setdefault(key, []).append(r)

    # collapse per (task, arm) across trials
    agg = {}  # (task,arm) -> list of (solved, output_tokens_spent)
    for (task, arm, trial), rs in cells.items():
        rs = sorted(rs, key=lambda r: int(r.get("attempt", 1)))
        ats = next((int(r["attempt"]) for r in rs if r.get("ok")), None)
        solved = ats is not None
        if solved:
            o = sum(int(r["program_tokens_o200k"]) for r in rs
                    if int(r["attempt"]) <= ats)
        else:
            o = sum(int(r["program_tokens_o200k"]) for r in rs)
        agg.setdefault((task, arm), []).append((solved, o))

    tasks = []
    for task_id in prompts:
        p = prompts[task_id]
        P_mtl = o200k(p["mtl_prompt"])
        P_py = o200k(p["python_prompt"])
        mtl_cells = agg[(task_id, "mtl")]
        py_cells = agg[(task_id, "python")]
        O_mtl = statistics.fmean(o for _, o in mtl_cells)
        O_py = statistics.fmean(o for _, o in py_cells)
        mtl_solved = statistics.fmean(1.0 if s else 0.0 for s, _ in mtl_cells)
        py_solved = statistics.fmean(1.0 if s else 0.0 for s, _ in py_cells)
        tasks.append({
            "task": task_id, "tier": 2,
            "P_py": P_py, "O_py": round(O_py, 4), "py_solved": round(py_solved, 4),
            "P_mtl": P_mtl, "O_mtl": round(O_mtl, 4), "mtl_solved": round(mtl_solved, 4),
            "n_trials": len(mtl_cells),
            "prov_input": "o200k tokcount of payload.json mtl_prompt/python_prompt",
            "prov_output": ("mean over trials of results.jsonl program_tokens_o200k "
                            "summed over attempts 1..first-correct (all-attempts cost)"),
            "prov_success": "fraction of trials with ok within 5 attempts",
        })
    return tasks


# ---------------------------------------------------------------------------
# Tier-3 extraction (8 tasks).
#   INPUT tokens:  each arm receives the task spec (prompt + granted +
#                  emit_budget + expected_output). Both arms get the same
#                  prompt body; the arms differ only in how the capability set
#                  is presented -- the MTL arm sees the granted MTL cap words,
#                  the Python arm sees the canonical host stub function list
#                  (per tier3/PROTOCOL.md). Rendered with the canonical
#                  templates below and tokcounted per arm. (The quickref Q is
#                  added on top of the MTL arm separately by the economic model,
#                  exactly as in tier-2 -- it is NOT part of P_mtl here.)
#   OUTPUT tokens: attempts[].program tokcounted (o200k, newline-stripped),
#                  summed over attempts 1..winning_attempt; if unsolved, all
#                  attempts. Averaged over the 2 trials per (task,arm).
#   SUCCESS:       fraction of trials solved within 5 attempts.
# ---------------------------------------------------------------------------
def _render_tier3_mtl_prompt(t) -> str:
    budget = "unlimited" if t.get("emit_budget") is None else str(t["emit_budget"])
    return (
        t["prompt"]
        + "\nGranted capabilities: " + " ".join(t["granted"])
        + "\nEmit budget: " + budget
        + "\nExpected output: " + repr(t["expected_output"])
    )


def _render_tier3_py_prompt(t) -> str:
    budget = "unlimited" if t.get("emit_budget") is None else str(t["emit_budget"])
    return (
        t["prompt"]
        + "\nAvailable host stub functions: " + ", ".join(TIER3_PY_STUBS)
        + "\nEmit budget: " + budget
        + "\nExpected output: " + repr(t["expected_output"])
    )


def extract_tier3():
    spec = json.load(open(os.path.join(_AGENT_TRIAL, "tier3", "tasks.json")))
    tlist = spec if isinstance(spec, list) else spec.get("tasks", spec)
    by_name = {t["name"]: t for t in tlist}

    cells = {}  # (task,arm) -> list of cell dicts
    for f in sorted(glob.glob(os.path.join(
            _AGENT_TRIAL, "tier3", "results", "attempts", "*.json"))):
        r = json.load(open(f))
        cells.setdefault((r["task"], r["arm"]), []).append(r)

    tasks = []
    for name, t in by_name.items():
        P_mtl = _tok_prompt(_render_tier3_mtl_prompt(t))
        P_py = _tok_prompt(_render_tier3_py_prompt(t))
        rec = {}
        for arm in ("mtl", "python"):
            outs, solves = [], []
            for c in cells[(name, arm)]:
                wa = c.get("winning_attempt")
                atts = c["attempts"]
                if c["solved"] and wa:
                    o = sum(_tok_program(a["program"]) for a in atts if a["n"] <= wa)
                else:
                    o = sum(_tok_program(a["program"]) for a in atts)
                outs.append(o)
                solves.append(1.0 if c["solved"] else 0.0)
            rec[arm] = (statistics.fmean(outs), statistics.fmean(solves), len(outs))
        tasks.append({
            "task": name, "tier": 3,
            "P_py": P_py, "O_py": round(rec["python"][0], 4),
            "py_solved": round(rec["python"][1], 4),
            "P_mtl": P_mtl, "O_mtl": round(rec["mtl"][0], 4),
            "mtl_solved": round(rec["mtl"][1], 4),
            "n_trials": rec["mtl"][2],
            "prov_input": ("o200k tokcount of the canonically-rendered per-arm task "
                           "prompt (prompt + granted/stub list + emit_budget + "
                           "expected_output); quickref Q added separately by the model"),
            "prov_output": ("mean over trials of attempts[].program o200k tokcount "
                            "(newline-stripped) summed over attempts 1..winning_attempt"),
            "prov_success": "fraction of trials with solved within 5 attempts",
        })
    return tasks


def _tok_prompt(text: str) -> int:
    """o200k tokcount of a rendered prompt string (no newline stripping -- a
    prompt is not a program)."""
    return int(count(text)["o200k_base"])


# ---------------------------------------------------------------------------
# The token/cost model (issue #45 section 3). Q = quickref prefix tokens.
# ---------------------------------------------------------------------------
def arm_buckets(sampled, Q, arm):
    """Return the four token buckets (normal_input, output, cache_write,
    cache_read) for one arm over an ordered list of sampled task dicts.

    Buckets are priced by the caller. This function is pricing-independent so
    the raw token ledger is comparable across price configs."""
    N = len(sampled)
    if arm == "python":
        ni = sum(t["P_py"] for t in sampled)
        out = sum(t["O_py"] for t in sampled)
        return {"normal_input": ni, "output": out, "cache_write": 0.0, "cache_read": 0.0}
    if arm == "mtl_cold":
        # quickref re-read as full input on EVERY task (status quo)
        ni = sum(Q + t["P_mtl"] for t in sampled)
        out = sum(t["O_mtl"] for t in sampled)
        return {"normal_input": ni, "output": out, "cache_write": 0.0, "cache_read": 0.0}
    if arm == "mtl_cached":
        # quickref written to cache once (task 1), cache-read on tasks 2..N
        ni = sum(t["P_mtl"] for t in sampled)
        out = sum(t["O_mtl"] for t in sampled)
        cw = float(Q)
        cr = float(Q) * (N - 1)
        return {"normal_input": ni, "output": out, "cache_write": cw, "cache_read": cr}
    raise ValueError(arm)


def price_buckets(b, cfg):
    """Dollar cost of a token-bucket dict under a price config."""
    p_in = cfg["p_in"] / 1e6
    p_out = cfg["p_out"] / 1e6
    pw = cfg["cache_write_mult"] * cfg["p_in"] / 1e6
    pr = cfg["cache_read_mult"] * cfg["p_in"] / 1e6
    return (b["normal_input"] * p_in + b["output"] * p_out
            + b["cache_write"] * pw + b["cache_read"] * pr)


def num_correct(sampled, arm):
    """Expected number of correct solutions in the session for an arm.
    Success flags are per-task solved fractions from the trials; the session
    total is their sum (an expected count that gracefully handles partial
    success). Python uses py_solved for arms A; MTL arms use mtl_solved."""
    key = "py_solved" if arm == "python" else "mtl_solved"
    return sum(t[key] for t in sampled)


# ---------------------------------------------------------------------------
# Sweep.
# ---------------------------------------------------------------------------
def run_sweep(tasks, Q, cfg, ledger_fp=None, cfg_name="default"):
    """Return per-N summary dict for one price config. Optionally write raw
    per-(N,replicate,arm) lines to ledger_fp (only done for the default cfg)."""
    rng = random.Random(SEED)
    arms = [("python", "python"), ("mtl_cold", "mtl_cold"), ("mtl_cached", "mtl_cached")]
    # arm -> success key
    succ_arm = {"python": "python", "mtl_cold": "mtl", "mtl_cached": "mtl"}

    summary = {}
    for N in SESSION_SIZES:
        per_arm_cpc = {a: [] for a, _ in arms}          # cost-per-correct samples
        per_arm_cost = {a: [] for a, _ in arms}         # absolute dollar cost
        c_beats_a = 0                                   # (C) cpc < (A) cpc count
        b_beats_a = 0                                   # (B) cpc < (A) cpc count
        mtl_succ_rate, py_succ_rate = [], []
        for rep in range(K_REPLICATES):
            sampled = rng.sample(tasks, N)              # WITHOUT replacement, paired
            row_cpc = {}
            for arm, _ in arms:
                b = arm_buckets(sampled, Q, arm)
                cost = price_buckets(b, cfg)
                # number correct depends on the arm's success flags
                nc = num_correct(sampled, "python" if arm == "python" else "mtl")
                cpc = cost / nc if nc > 0 else float("inf")
                per_arm_cost[arm].append(cost)
                per_arm_cpc[arm].append(cpc)
                row_cpc[arm] = cpc
                if ledger_fp is not None:
                    ledger_fp.write(json.dumps({
                        "config": cfg_name, "N": N, "replicate": rep, "arm": arm,
                        "task_ids": [t["task"] for t in sampled],
                        "normal_input_tokens": round(b["normal_input"], 4),
                        "output_tokens": round(b["output"], 4),
                        "cache_write_tokens": round(b["cache_write"], 4),
                        "cache_read_tokens": round(b["cache_read"], 4),
                        "dollar_cost": cost,
                        "num_correct": round(nc, 6),
                        "cost_per_correct": cpc,
                    }) + "\n")
            if row_cpc["mtl_cached"] < row_cpc["python"]:
                c_beats_a += 1
            if row_cpc["mtl_cold"] < row_cpc["python"]:
                b_beats_a += 1
            # success parity (paired sample)
            mtl_succ_rate.append(statistics.fmean(t["mtl_solved"] for t in sampled))
            py_succ_rate.append(statistics.fmean(t["py_solved"] for t in sampled))

        def band(xs):
            s = sorted(xs)
            return (statistics.fmean(xs),
                    s[int(0.10 * (len(s) - 1))],
                    s[int(0.90 * (len(s) - 1))])

        summary[N] = {
            "python": band(per_arm_cpc["python"]),
            "mtl_cold": band(per_arm_cpc["mtl_cold"]),
            "mtl_cached": band(per_arm_cpc["mtl_cached"]),
            "python_cost_mean": statistics.fmean(per_arm_cost["python"]),
            "mtl_cold_cost_mean": statistics.fmean(per_arm_cost["mtl_cold"]),
            "mtl_cached_cost_mean": statistics.fmean(per_arm_cost["mtl_cached"]),
            "frac_C_beats_A": c_beats_a / K_REPLICATES,
            "frac_B_beats_A": b_beats_a / K_REPLICATES,
            "mtl_success_mean": statistics.fmean(mtl_succ_rate),
            "py_success_mean": statistics.fmean(py_succ_rate),
            # AC4: flag any N where MTL success is strictly below Python.
            "mtl_success_deficit_flag": (
                statistics.fmean(mtl_succ_rate) < statistics.fmean(py_succ_rate)
            ),
        }
    return summary


def crossover_N(summary, arm):
    """Smallest N where mean cost-per-correct of `arm` < mean of python."""
    for N in SESSION_SIZES:
        if summary[N][arm][0] < summary[N]["python"][0]:
            return N
    return None


# ---------------------------------------------------------------------------
# Break-even task-size analysis (issue #45 -- makes the negative result
# ACTIONABLE). Deterministic; derived from the SAME cost formulas as the sweep
# (§3), not a new model.
#
# Asymptotically (N large, the one-time cache-write amortized to zero) the
# MTL-cached arm beats Python at the per-task margin iff:
#
#     Q*cr_mult*p_in + P_mtl*p_in + O_mtl*p_out  <  P_py*p_in + O_py*p_out
#
# Solving for the per-task output-token savings dO = O_py - O_mtl required to
# clear the cached quickref, at a fixed price ratio:
#
#   * to cover the cache-read tax alone (ignoring the input penalty):
#         dO_breakeven_tax  = cr_mult * Q * (p_in / p_out)
#   * to cover tax AND MTL's extra input tokens (the honest full threshold):
#         dO_breakeven_full = (cr_mult*Q + (P_mtl - P_py)) * (p_in / p_out)
#
# A crossover exists iff the MEASURED mean savings dO_measured exceeds the
# threshold. Here dO_measured ~= 7 vs a threshold ~= 81, so none does.
# ---------------------------------------------------------------------------
def break_even_analysis(tasks, Q, pricing):
    P_py = statistics.fmean(t["P_py"] for t in tasks)
    P_mtl = statistics.fmean(t["P_mtl"] for t in tasks)
    O_py = statistics.fmean(t["O_py"] for t in tasks)
    O_mtl = statistics.fmean(t["O_mtl"] for t in tasks)
    dO_measured = O_py - O_mtl
    input_penalty = P_mtl - P_py  # extra input tokens MTL pays per task
    out = {
        "mean_P_py": round(P_py, 4),
        "mean_P_mtl": round(P_mtl, 4),
        "mean_O_py": round(O_py, 4),
        "mean_O_mtl": round(O_mtl, 4),
        "measured_output_savings_per_task": round(dO_measured, 4),
        "measured_input_penalty_per_task": round(input_penalty, 4),
        "quickref_tokens": Q,
        "per_config": {},
    }
    for name, cfg in pricing.items():
        ratio = cfg["p_in"] / cfg["p_out"]          # p_in/p_out (default 1/5)
        cr = cfg["cache_read_mult"]
        be_tax = cr * Q * ratio
        be_full = (cr * Q + input_penalty) * ratio
        out["per_config"][name] = {
            "label": cfg["label"],
            "p_out_over_p_in": round(cfg["p_out"] / cfg["p_in"], 4),
            "cache_read_mult": cr,
            # output-token savings/task needed to break even
            "dO_breakeven_tax_only": round(be_tax, 4),
            "dO_breakeven_incl_input_penalty": round(be_full, 4),
            # how far short the measured battery falls (multiplicative)
            "shortfall_factor_vs_measured": (
                round(be_full / dO_measured, 2) if dO_measured > 0 else None
            ),
            "crossover_reachable_at_measured_savings": dO_measured >= be_full,
        }
    return out


# ---------------------------------------------------------------------------
# Outputs.
# ---------------------------------------------------------------------------
def write_curve_csv(summary, path):
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow([
            "N",
            "python_cpc_mean", "python_cpc_p10", "python_cpc_p90",
            "mtl_cold_cpc_mean", "mtl_cold_cpc_p10", "mtl_cold_cpc_p90",
            "mtl_cached_cpc_mean", "mtl_cached_cpc_p10", "mtl_cached_cpc_p90",
            "frac_C_beats_A", "frac_B_beats_A",
            "mtl_success_mean", "py_success_mean", "mtl_success_deficit_flag",
        ])
        for N in SESSION_SIZES:
            s = summary[N]
            w.writerow([
                N,
                s["python"][0], s["python"][1], s["python"][2],
                s["mtl_cold"][0], s["mtl_cold"][1], s["mtl_cold"][2],
                s["mtl_cached"][0], s["mtl_cached"][1], s["mtl_cached"][2],
                s["frac_C_beats_A"], s["frac_B_beats_A"],
                s["mtl_success_mean"], s["py_success_mean"],
                s["mtl_success_deficit_flag"],
            ])


def plot_curve(summary, nstar, path):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    xs = SESSION_SIZES
    colors = {"python": "#DD8452", "mtl_cold": "#55A868", "mtl_cached": "#4C72B0"}
    labels = {
        "python": "(A) Python (no cache)",
        "mtl_cold": "(B) MTL cold (no cache)",
        "mtl_cached": "(C) MTL cached (quickref cached)",
    }
    fig, ax = plt.subplots(figsize=(9, 5.5))
    for arm in ("python", "mtl_cold", "mtl_cached"):
        means = [summary[N][arm][0] * 100 for N in xs]   # cents
        p10 = [summary[N][arm][1] * 100 for N in xs]
        p90 = [summary[N][arm][2] * 100 for N in xs]
        ax.plot(xs, means, marker="o", color=colors[arm], label=labels[arm], lw=2)
        ax.fill_between(xs, p10, p90, color=colors[arm], alpha=0.15)
    if nstar is not None:
        ax.axvline(nstar, color="#333333", ls="--", lw=1.3)
        ymax = max(summary[1][a][2] for a in ("python", "mtl_cold", "mtl_cached")) * 100
        ax.annotate(f"N* = {nstar}\n(C) cost/correct < (A)",
                    xy=(nstar, ymax * 0.72),
                    xytext=(nstar + 0.4, ymax * 0.72),
                    fontsize=10, color="#333333",
                    va="center")
    ax.set_xlabel("Session size N (tasks per session)")
    ax.set_ylabel("Cost per correct solution (US cents)")
    ax.set_title("MTL vs Python cost-per-correct across session sizes\n"
                 "(mean lines, 10-90 percentile bands over 500 paired replicates)")
    ax.set_xticks(xs)
    ax.legend(loc="upper right")
    ax.grid(True, alpha=0.25)
    fig.tight_layout()
    fig.savefig(path, dpi=130)
    plt.close(fig)


def main():
    os.makedirs(RESULTS_DIR, exist_ok=True)
    pricing = json.load(open(os.path.join(_HERE, "pricing.json")))["configs"]

    Q = measure_quickref_tokens()

    tier2 = extract_tier2()
    tier3 = extract_tier3()
    tasks = tier2 + tier3
    assert len(tasks) == 18, f"expected 18 tasks, got {len(tasks)}"

    # task_costs.json (transparency table).
    json.dump({
        "quickref_tokens_o200k": Q,
        "quickref_source": os.path.relpath(QUICKREF_PATH, _REPO),
        "n_tasks": len(tasks),
        "tasks": tasks,
        "notes": {
            "encoding": "o200k_base (tiktoken 0.8.0) as public proxy for the Claude tokenizer",
            "tier2_output_source": "measured cold-run program_tokens_o200k (results.jsonl), all-attempts-to-first-correct",
            "tier3_output_source": "measured cold-run programs (attempts[].program) tokcounted o200k, all-attempts-to-first-correct",
            "success": "all 18 tasks solved within 5 attempts in both arms across all trials (see per-task *_solved)",
        },
    }, open(os.path.join(RESULTS_DIR, "task_costs.json"), "w"), indent=2)

    # Full sweep for every price config; ledger only for the default.
    default_summary = None
    sensitivity = {}
    ledger_path = os.path.join(RESULTS_DIR, "ledger.jsonl")
    with open(ledger_path, "w") as ledger_fp:
        for name, cfg in pricing.items():
            fp = ledger_fp if name == "default" else None
            summ = run_sweep(tasks, Q, cfg, ledger_fp=fp, cfg_name=name)
            nstar_c = crossover_N(summ, "mtl_cached")
            nstar_b = crossover_N(summ, "mtl_cold")
            sensitivity[name] = {
                "label": cfg["label"],
                "nstar_cached": nstar_c,
                "nstar_cold": nstar_b,
            }
            if name == "default":
                default_summary = summ

    write_curve_csv(default_summary, os.path.join(RESULTS_DIR, "curve.csv"))
    nstar = crossover_N(default_summary, "mtl_cached")
    plot_curve(default_summary, nstar, os.path.join(RESULTS_DIR, "curve.png"))

    # Console summary.
    print(f"Q (quickref o200k tokens) = {Q}")
    print(f"tasks: {len(tier2)} tier-2 + {len(tier3)} tier-3 = {len(tasks)}")
    print(f"\nDEFAULT config N* (cached) = {nstar}   "
          f"uncached crossover = {crossover_N(default_summary, 'mtl_cold')}")
    print("\nPer-N cost-per-correct (US cents), default config:")
    print(f"{'N':>3} {'A_py':>10} {'B_cold':>10} {'C_cached':>10} "
          f"{'C<A frac':>9} {'B<A frac':>9} {'mtlSucc':>8} {'pySucc':>8}")
    for N in SESSION_SIZES:
        s = default_summary[N]
        print(f"{N:>3} {s['python'][0]*100:>10.4f} {s['mtl_cold'][0]*100:>10.4f} "
              f"{s['mtl_cached'][0]*100:>10.4f} {s['frac_C_beats_A']:>9.3f} "
              f"{s['frac_B_beats_A']:>9.3f} {s['mtl_success_mean']:>8.3f} "
              f"{s['py_success_mean']:>8.3f}")
    print("\nSensitivity (config -> N*):")
    for name, d in sensitivity.items():
        print(f"  {name:22s} cached N*={d['nstar_cached']}  "
              f"cold N*={d['nstar_cold']}  ({d['label']})")

    # Break-even task-size analysis: how much per-task output compression WOULD
    # a crossover require? Converts "no N*" into a concrete adoption condition.
    be = break_even_analysis(tasks, Q, pricing)
    print("\nBreak-even task-size analysis (output-token savings/task needed):")
    print(f"  measured mean output savings dO = O_py - O_mtl = "
          f"{be['measured_output_savings_per_task']} tokens/task")
    print(f"  measured mean input penalty  = P_mtl - P_py     = "
          f"{be['measured_input_penalty_per_task']} tokens/task")
    print(f"  {'config':22s} {'dO_be(tax)':>11s} {'dO_be(full)':>12s} "
          f"{'shortfall':>10s} {'reachable?':>10s}")
    for name, d in be["per_config"].items():
        print(f"  {name:22s} {d['dO_breakeven_tax_only']:>11.2f} "
              f"{d['dO_breakeven_incl_input_penalty']:>12.2f} "
              f"{str(d['shortfall_factor_vs_measured'])+'x':>10s} "
              f"{str(d['crossover_reachable_at_measured_savings']):>10s}")

    # Persist the sensitivity + headline into a small json for the report.
    json.dump({
        "quickref_tokens_o200k": Q,
        "default_nstar_cached": nstar,
        "default_nstar_cold": crossover_N(default_summary, "mtl_cold"),
        "break_even": be,
        "sensitivity": sensitivity,
        "per_N_default": {
            str(N): {
                "python_cpc_mean": default_summary[N]["python"][0],
                "mtl_cold_cpc_mean": default_summary[N]["mtl_cold"][0],
                "mtl_cached_cpc_mean": default_summary[N]["mtl_cached"][0],
                "frac_C_beats_A": default_summary[N]["frac_C_beats_A"],
                "frac_B_beats_A": default_summary[N]["frac_B_beats_A"],
                "mtl_success_mean": default_summary[N]["mtl_success_mean"],
                "py_success_mean": default_summary[N]["py_success_mean"],
                "mtl_success_deficit_flag": default_summary[N]["mtl_success_deficit_flag"],
            } for N in SESSION_SIZES
        },
    }, open(os.path.join(RESULTS_DIR, "summary.json"), "w"), indent=2)


if __name__ == "__main__":
    main()
