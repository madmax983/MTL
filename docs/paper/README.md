# MTL capstone paper

This directory holds the capstone research report for MTL (Minimal Token Language) — the
definitive, citation-backed statement of what MTL claims and why (issue #71 writeup deliverable).

- **Master document:** [`mtl.md`](mtl.md) — a single comprehensive report. Every numeric claim
  cites its source artifact inline as a relative repository path.
- **Reproduction kit:** [`../../REPRODUCE.md`](../../REPRODUCE.md) — the claim→command map for
  re-running each proof root, token baseline, and sealed/contamination gate. Start there to verify.

## The one-paragraph answer

The sharp question: *can a language co-designed for model tokenizers and formal verification
reduce total inference cost without reducing agent reliability?* The measured answer: reliability
is **preserved** (100% pass@5 held-out, no measurable read-tax); total inference cost is **not
reduced** in the cold single-session regime (break-even N* is structurally unreachable within
N ≤ 16); general compression **failed out of sample** and the ≥3× gate was retired; what survives
is **per-solution economics** (held-out CSPM 2.124×), a **five-root machine-checked core**, and
**verified capability confinement**. The honest-negative framing is the spine of the paper, not a
footnote.

## Section map

1. **Abstract** — the sharp question, the honest answer, the headline verified artifact.
2. **Introduction** — why tokenizers + verification; the honest arc in miniature; the contributions
   list (negatives and the co-evolution finding are first-class).
3. **Design method** — measurement-driven primitive admission; TAVDD; the six-way primitive mirror
   and `for_each_primitive!` codegen; adversarial-review absorption as process.
4. **The language** — concatenative point-free core, 25 glyphs, `Value = Int | Quote`, host-side
   effects.
5. **Verification** — the five proof roots with exact counts, the two-stub trusted boundary, the
   P5 Minsky construction, Layer-C checker soundness, arena refinement, and the oracle-pinned-twin
   (not extraction) story. Includes where this paper supersedes the stale README/release-notes.
6. **Results — the honest arc** — in-sample compression; the held-out collapse to 1.67×; the
   co-evolution finding; writability 100% pass@5; no measurable read-tax; session economics +
   preamble ablation + warm/LoRA hypothesis; per-solution CSPM 2.124×; capability confinement;
   performance.
7. **Decision record** — indexed-access declined, strings host-side, arena-as-default,
   tokcount-gated admission, `#f[...]` deferred, fork rejected, host-side metering.
8. **Related work** — pxpipe and the confabulation contrast; positioning vs Python-in-a-sandbox,
   WASM, jq-style DSLs.
9. **Limitations** — all of them, plainly.
10. **Reproducibility** — the reproduction kit is the credibility path (CI never gated merges);
    what is and is not push-button reproducible.
11. **Conclusion** — what MTL demonstrably is, and the biggest open lever (the warm/fine-tuned arm,
    N* → 1).
- **Appendix A** — proof scoreboard (the five-root table).
- **Appendix B** — claim → artifact index.

## Ground-truth sources

The report is built entirely from checked-in artifacts: the proof logs
(`crates/mtl-*/proof-log.txt`), the benchmark baselines (`bench/BASELINE*.md`), the agent-trial
reports (`bench/agent-trial/**/REPORT.md`), the design decision records (`docs/design/*.md`), the
three external reviews (`docs/reviews/*.md`), and the specification (`docs/mtl-spec.md`). Where the
in-repo record was internally contradictory, the paper cites the current merged value and flags the
discrepancy rather than smoothing it (see §5.6 of `mtl.md`).
