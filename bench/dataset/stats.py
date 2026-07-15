#!/usr/bin/env python3
"""Exact tiktoken token accounting for a generated MTL SFT dataset.

Loads ``dataset.jsonl`` and computes exact tiktoken ``o200k_base`` (and
``cl100k_base``) token totals over the ``instruction`` + ``response`` fields of
every record, reusing the in-repo counter ``bench/tokcount/tokcount.py``.

## Why tiktoken o200k as a proxy for Qwen

The v0.7 design's "~2.7M SFT tokens" figure is under the Qwen2.5-Coder
tokenizer. Qwen ships no offline Python counter in this repo; tiktoken
``o200k_base`` is used here as a **deterministic offline proxy**. Both are
byte-level BPE tokenizers and the checked ``bench/tokcount/token_profile.json``
confirms every one of the 23 MTL primitive glyphs is exactly one token under
o200k — so at the merge/glyph level o200k and Qwen agree closely on MTL text.
The English instruction side is ordinary prose where BPE tokenizers track within
a few percent. Treat these totals as a tight lower-bound proxy for the Qwen
count, not an exact Qwen measurement.

Usage:
    python3 bench/dataset/stats.py [pilot_dir]

Writes ``<dir>/stats_tokens.json`` and folds the token totals back into
``<dir>/stats.json`` (adding a ``tiktoken`` block). Idempotent.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "tokcount"))

from tokcount import count, ENCODINGS  # noqa: E402


def main() -> int:
    pilot = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "pilot")
    ds_path = os.path.join(pilot, "dataset.jsonl")
    if not os.path.exists(ds_path):
        print(f"no dataset at {ds_path}", file=sys.stderr)
        return 1

    totals = {e: 0 for e in ENCODINGS}
    instr_totals = {e: 0 for e in ENCODINGS}
    resp_totals = {e: 0 for e in ENCODINGS}
    per_kind = {"gen": {e: 0 for e in ENCODINGS}, "repair": {e: 0 for e in ENCODINGS}}
    n = 0

    with open(ds_path) as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            r = json.loads(line)
            n += 1
            ci = count(r["instruction"])
            cr = count(r["response"])
            kind = r.get("kind", "gen")
            for e in ENCODINGS:
                iv = ci.get(e) or 0
                rv = cr.get(e) or 0
                instr_totals[e] += iv
                resp_totals[e] += rv
                totals[e] += iv + rv
                if kind in per_kind:
                    per_kind[kind][e] += iv + rv

    primary = "o200k_base"
    tok = {
        "records": n,
        "encodings": ENCODINGS,
        "total_tokens": totals,
        "instruction_tokens": instr_totals,
        "response_tokens": resp_totals,
        "per_kind_tokens": per_kind,
        "blended_tokens_per_example": {
            e: (totals[e] / n if n else 0) for e in ENCODINGS
        },
        "primary_encoding": primary,
        "note": (
            "tiktoken o200k_base used as an offline deterministic proxy for the "
            "Qwen2.5-Coder tokenizer; both are byte-level BPE and every MTL glyph "
            "is 1 token under o200k (see bench/tokcount/token_profile.json)."
        ),
    }

    with open(os.path.join(pilot, "stats_tokens.json"), "w") as fh:
        json.dump(tok, fh, indent=2)
        fh.write("\n")

    # Fold into stats.json.
    stats_path = os.path.join(pilot, "stats.json")
    if os.path.exists(stats_path):
        with open(stats_path) as fh:
            stats = json.load(fh)
        stats["tiktoken"] = tok
        with open(stats_path, "w") as fh:
            json.dump(stats, fh, indent=2)
            fh.write("\n")

    print(
        f"{n} records: {totals[primary]} {primary} tokens "
        f"({tok['blended_tokens_per_example'][primary]:.1f}/example)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
