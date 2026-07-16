#!/usr/bin/env python3
"""
train_mtl_lora.py — QLoRA fine-tune of Qwen2.5-Coder-7B-Instruct into a "warm"
MTL agent (v0.7 recipe; refs docs/design/v0.7-lora-warm-agent.md, issue #83).

The warm agent writes MTL with ZERO quickref preamble in context: MTL competence
is moved from context (the ~4051-token quickref, "cold") into weights (a LoRA
adapter, "warm"), deleting the fixed per-task tax that PR #80 proved structurally
blocks per-task economics.

Design contract (see the design doc §6 recipe):
  Model    Qwen/Qwen2.5-Coder-7B-Instruct (Apache-2.0)
  Method   QLoRA (NF4 + double-quant, bf16 compute) via Unsloth
  Vocab    NO extension in v1 (raw BPE)
  Hypers   r=64 (sweep 32-128) · alpha=r + rsLoRA · all-linear · LR 2e-4,
           warmup 5-10%, 2-3 epochs · completion-only loss
  Data     oracle-validated MTL pairs (bench/dataset), ~10-20% general replay

CRITICAL DESIGN NOTE — lazy imports:
  This file is import-safe on a machine with NO GPU and NO torch/unsloth/trl.
  Heavy / GPU deps (torch, unsloth, trl, transformers model classes, datasets,
  vllm) are imported ONLY inside the real training path. `--smoke` imports only
  the standard library + PyYAML (+ optionally a transformers *tokenizer*), so it
  runs GPU-free and torch-free on a laptop / in CI.

Usage:
  # Offline pre-flight validation on the committed pilot (no GPU, no torch):
  python train_mtl_lora.py --smoke --config config/r64.yaml

  # Real training on a RunPod GPU pod:
  python train_mtl_lora.py --config config/r64.yaml \
      --data bench/dataset/full/dataset.jsonl --out /workspace/out/r64

  # Resume after eviction (out MUST be on the network volume):
  python train_mtl_lora.py --config config/r64.yaml --out /workspace/out/r64 --resume
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
from dataclasses import dataclass, asdict
from datetime import datetime, timezone

# yaml is the ONLY third-party import allowed at module top level (smoke needs it,
# and it is pure-python / torch-free).
import yaml

# ------------------------------------------------------------------------------
# Repo-relative defaults
# ------------------------------------------------------------------------------
HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
DEFAULT_PILOT = os.path.join(REPO_ROOT, "bench", "dataset", "pilot", "dataset.jsonl")

# Verbatim success gate from docs/design/v0.7-lora-warm-agent.md §6 (do NOT edit).
# Kept byte-exact (unicode ≥ / ≤ / →) so the pre-registered gate is reproducible in
# the model card / manifest.
SUCCESS_GATE_VERBATIM = (
    "Eval gate  warm pass@1 ≥ cold, tok→correct within ±2, 0 preamble, "
    "N* ≤ 2 on ≥3/5 price configs, on the #53 sealed set"
)
# Coordinator's pre-registered warm-arm gate (verbatim):
WARM_ARM_GATE = "warm cspm > Python AND N* ≤ 2"

REQUIRED_FIELDS = {
    "instruction": str,
    "response": str,
    "tier": int,
    "family": str,
    "difficulty": int,
    "kind": str,
    "canonical_sha256": str,
    "io_sha256": str,
}
# `check` is provenance only (io vectors); it is present in every row but is NOT
# part of the chat turns and MUST be ignored by the trainer.


# ------------------------------------------------------------------------------
# Config
# ------------------------------------------------------------------------------
@dataclass
class Config:
    # model / method
    base_model: str = "Qwen/Qwen2.5-Coder-7B-Instruct"
    r: int = 64
    alpha: int | None = None          # defaults to r
    use_rslora: bool = True
    target_modules: str = "all-linear"
    lora_dropout: float = 0.0
    # optimisation
    lr: float = 2e-4
    epochs: int = 2
    warmup_ratio: float = 0.05
    weight_decay: float = 0.01
    lr_scheduler_type: str = "cosine"
    max_seq_len: int = 2048
    per_device_batch_size: int = 8
    grad_accum: int = 4
    packing: bool = True
    gradient_checkpointing: bool = True
    # replay
    replay_dataset: str = "bigcode/the-stack-smol"
    replay_text_field: str = "content"
    replay_fraction: float = 0.15
    replay_max_seq_len: int = 1024
    replay_split: str = "train"
    replay_data_dir: str | None = "data/python"   # the-stack-smol is per-language
    # eval / holdout
    eval_fraction: float = 0.05
    # provenance / determinism
    seed: int = 0
    # checkpointing
    save_steps: int = 200
    save_total_limit: int = 3
    # optional short system marker so inference matches (design §2)
    system_marker: str | None = "emit MTL"

    def resolved_alpha(self) -> int:
        return self.alpha if self.alpha is not None else self.r

    def effective_batch(self) -> int:
        return self.per_device_batch_size * self.grad_accum


def load_config(path: str) -> Config:
    with open(path, "r") as fh:
        raw = yaml.safe_load(fh) or {}
    known = {f.name for f in Config.__dataclass_fields__.values()}  # type: ignore[attr-defined]
    unknown = set(raw) - known
    if unknown:
        raise ValueError(f"unknown config keys: {sorted(unknown)}")
    return Config(**raw)


def sanity_check_config(cfg: Config) -> list[str]:
    """Return a list of human-readable problems; empty list == OK."""
    problems: list[str] = []
    if not (8 <= cfg.r <= 256):
        problems.append(f"r={cfg.r} outside [8,256]")
    if cfg.resolved_alpha() <= 0:
        problems.append(f"alpha={cfg.resolved_alpha()} must be > 0")
    if not (0.0 <= cfg.replay_fraction < 1.0):
        problems.append(f"replay_fraction={cfg.replay_fraction} must be in [0,1)")
    if cfg.epochs < 1:
        problems.append(f"epochs={cfg.epochs} must be >= 1")
    if not (0.0 <= cfg.warmup_ratio < 1.0):
        problems.append(f"warmup_ratio={cfg.warmup_ratio} must be in [0,1)")
    if not (0.0 <= cfg.lora_dropout <= 0.5):
        problems.append(f"lora_dropout={cfg.lora_dropout} must be in [0,0.5]")
    if not (0.0 <= cfg.eval_fraction < 0.5):
        problems.append(f"eval_fraction={cfg.eval_fraction} must be in [0,0.5)")
    if cfg.lr <= 0:
        problems.append(f"lr={cfg.lr} must be > 0")
    if cfg.max_seq_len < 128:
        problems.append(f"max_seq_len={cfg.max_seq_len} too small")
    if cfg.per_device_batch_size < 1 or cfg.grad_accum < 1:
        problems.append("batch/grad_accum must be >= 1")
    return problems


# ------------------------------------------------------------------------------
# Dataset loading + validation (torch-free)
# ------------------------------------------------------------------------------
def load_jsonl(path: str) -> list[dict]:
    rows: list[dict] = []
    with open(path, "r") as fh:
        for lineno, line in enumerate(fh, 1):
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError as exc:
                raise ValueError(f"{path}:{lineno}: bad JSON: {exc}") from exc
    return rows


def validate_rows(rows: list[dict]) -> tuple[list[str], dict]:
    """Validate schema. Returns (problems, stats). `check` must be present but is ignored."""
    problems: list[str] = []
    tier_hist: dict[int, int] = {}
    kind_hist: dict[str, int] = {}
    for i, r in enumerate(rows):
        for fld, typ in REQUIRED_FIELDS.items():
            if fld not in r:
                problems.append(f"row {i}: missing field '{fld}'")
                continue
            val = r[fld]
            # bool is a subclass of int; disallow it for int fields
            if typ is int and isinstance(val, bool):
                problems.append(f"row {i}: field '{fld}' is bool, want int")
            elif not isinstance(val, typ):
                problems.append(
                    f"row {i}: field '{fld}' is {type(val).__name__}, want {typ.__name__}"
                )
        if "check" not in r:
            problems.append(f"row {i}: missing provenance field 'check'")
        if r.get("instruction", "") == "":
            problems.append(f"row {i}: empty instruction")
        if r.get("response", "") == "":
            problems.append(f"row {i}: empty response")
        t = r.get("tier")
        if isinstance(t, int) and not isinstance(t, bool):
            tier_hist[t] = tier_hist.get(t, 0) + 1
        k = r.get("kind")
        if isinstance(k, str):
            kind_hist[k] = kind_hist.get(k, 0) + 1
        if len(problems) > 50:
            problems.append("... (truncated; >50 schema problems)")
            break
    stats = {"count": len(rows), "tier_hist": tier_hist, "kind_hist": kind_hist}
    return problems, stats


def sha256_of_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


# ------------------------------------------------------------------------------
# Deterministic tier-stratified eval holdout (torch-free)
# ------------------------------------------------------------------------------
def carve_eval_split(rows: list[dict], eval_fraction: float, seed: int) -> tuple[list[dict], list[dict]]:
    """Deterministically carve an eval holdout, stratified by tier when practical."""
    if eval_fraction <= 0:
        return rows, []
    by_tier: dict[int, list[int]] = {}
    for idx, r in enumerate(rows):
        by_tier.setdefault(int(r.get("tier", 0)), []).append(idx)
    rng = random.Random(seed)
    eval_idx: set[int] = set()
    for tier, idxs in sorted(by_tier.items()):
        idxs = list(idxs)
        rng.shuffle(idxs)
        k = int(round(len(idxs) * eval_fraction))
        # keep at least 1 eval example per tier that has enough rows
        if k == 0 and len(idxs) >= 20:
            k = 1
        eval_idx.update(idxs[:k])
    train = [r for i, r in enumerate(rows) if i not in eval_idx]
    ev = [r for i, r in enumerate(rows) if i in eval_idx]
    return train, ev


# ------------------------------------------------------------------------------
# Chat-template formatting (torch-free structural helper)
# ------------------------------------------------------------------------------
def build_messages(row: dict, system_marker: str | None) -> list[dict]:
    msgs: list[dict] = []
    if system_marker:
        msgs.append({"role": "system", "content": system_marker})
    msgs.append({"role": "user", "content": row["instruction"]})
    msgs.append({"role": "assistant", "content": row["response"]})
    return msgs


def structural_chatml(row: dict, system_marker: str | None) -> str:
    """Self-contained ChatML render used ONLY for the offline smoke fallback.

    The real training path uses tokenizer.apply_chat_template; this mirror exists
    so the offline smoke can assert the assistant/response span is recoverable
    without downloading a tokenizer.
    """
    parts: list[str] = []
    if system_marker:
        parts.append(f"<|im_start|>system\n{system_marker}<|im_end|>\n")
    parts.append(f"<|im_start|>user\n{row['instruction']}<|im_end|>\n")
    parts.append(f"<|im_start|>assistant\n{row['response']}<|im_end|>\n")
    return "".join(parts)


# Qwen2.5 assistant header used for completion-only masking.
ASSISTANT_HEADER = "<|im_start|>assistant\n"


# ------------------------------------------------------------------------------
# VRAM estimate (formula-based; torch-free)
# ------------------------------------------------------------------------------
def estimate_vram_gb(cfg: Config) -> dict:
    """Rough formula-based VRAM estimate for the resolved config (7B QLoRA)."""
    # 7B params, 4-bit NF4 base + double-quant ~= 3.5-4.5 GB.
    base_4bit_gb = 4.0
    # LoRA params: all-linear on a 7B has ~7 linear projections/layer, 28 layers,
    # hidden ~3584, intermediate ~18944. Approximate trainable params:
    #   per-module = r*(in+out). Sum over q,k,v,o,gate,up,down across 28 layers.
    # Use a conservative closed-form: ~ r * 28 * (sum of in+out dims).
    hidden = 3584
    inter = 18944
    # (in+out) sums, Qwen2.5-7B (GQA: kv proj smaller ~512):
    dims = [
        hidden + hidden,   # q_proj
        hidden + 512,      # k_proj (GQA)
        hidden + 512,      # v_proj (GQA)
        hidden + hidden,   # o_proj
        hidden + inter,    # gate_proj
        hidden + inter,    # up_proj
        inter + hidden,    # down_proj
    ]
    lora_params = cfg.r * 28 * sum(dims)
    # bf16 adapter weights (2B) + grads (2B) + Adam m,v (2*4B) ~= 12 bytes/param
    lora_state_gb = lora_params * 12 / 1e9
    # Activations: dominated by batch * seq * hidden * layers with checkpointing.
    tok = cfg.per_device_batch_size * cfg.max_seq_len
    act_per_tok_bytes = 2 * hidden * (2 if cfg.gradient_checkpointing else 12)
    act_gb = tok * act_per_tok_bytes / 1e9
    # CUDA context + kernels + fragmentation overhead.
    overhead_gb = 1.5
    total = base_4bit_gb + lora_state_gb + act_gb + overhead_gb
    return {
        "base_4bit_gb": round(base_4bit_gb, 2),
        "lora_trainable_params_millions": round(lora_params / 1e6, 2),
        "lora_optimizer_state_gb": round(lora_state_gb, 2),
        "activation_gb": round(act_gb, 2),
        "overhead_gb": overhead_gb,
        "total_gb": round(total, 2),
        "fits_4090_24gb": total < 24,
        "fits_a100_80gb": total < 80,
    }


# ------------------------------------------------------------------------------
# SMOKE MODE — torch-free preflight
# ------------------------------------------------------------------------------
def try_load_tokenizer(base_model: str, tokenizer_path: str | None):
    """Return a HF tokenizer if transformers + files are available, else None."""
    try:
        from transformers import AutoTokenizer  # lazy, optional
    except Exception:
        return None, "transformers not installed"
    src = tokenizer_path or base_model
    try:
        tok = AutoTokenizer.from_pretrained(src)
        return tok, None
    except Exception as exc:  # offline / gated / network
        return None, f"tokenizer load failed: {type(exc).__name__}: {exc}"


def smoke(cfg: Config, data_path: str, tokenizer_path: str | None) -> int:
    ok = True

    def mark(passed: bool, skip: bool = False) -> str:
        if skip:
            return "SKIP"
        return "✓" if passed else "FAIL"

    print("=" * 72)
    print("MTL LoRA training-script SMOKE (GPU-free, torch-free)")
    print("=" * 72)

    # --- Check 1: config parse + sanity ---
    problems = sanity_check_config(cfg)
    c1 = not problems
    print(f"\n[1/4] Config parse + sanity ......... {mark(c1)}")
    print("  Resolved config:")
    for k, v in asdict(cfg).items():
        print(f"    {k:24s} = {v}")
    print(f"    {'(alpha resolved)':24s} = {cfg.resolved_alpha()}")
    print(f"    {'(effective batch)':24s} = {cfg.effective_batch()}")
    if problems:
        for p in problems:
            print(f"  PROBLEM: {p}")
        ok = False

    # --- Check 2: JSONL load + schema ---
    print(f"\n[2/4] JSONL load + schema ........... ", end="")
    c2 = True
    stats = {}
    try:
        rows = load_jsonl(data_path)
        row_problems, stats = validate_rows(rows)
        if row_problems:
            c2 = False
            print(mark(False))
            for p in row_problems[:10]:
                print(f"  PROBLEM: {p}")
        else:
            print(mark(True))
        print(f"  data: {data_path}")
        print(f"  records: {stats.get('count')}")
        print(f"  tier histogram: {stats.get('tier_hist')}")
        print(f"  kind histogram: {stats.get('kind_hist')}")
        print("  'check' field present and IGNORED for training (provenance only)")
        # deterministic eval carve preview
        if rows:
            train, ev = carve_eval_split(rows, cfg.eval_fraction, cfg.seed)
            print(f"  eval holdout (fraction={cfg.eval_fraction}, seed={cfg.seed}): "
                  f"{len(ev)} eval / {len(train)} train")
    except Exception as exc:
        c2 = False
        print(mark(False))
        print(f"  ERROR: {type(exc).__name__}: {exc}")
        rows = []
    ok = ok and c2

    # --- Check 3: chat template + tokenizer round-trip ---
    print(f"\n[3/4] Chat-template + round-trip .... ", end="")
    c3 = True
    skipped_realtok = False
    if not rows:
        print(mark(False))
        print("  ERROR: no rows to format")
        c3 = False
    else:
        samples = rows[:3]
        tok, tok_err = try_load_tokenizer(cfg.base_model, tokenizer_path)
        if tok is not None:
            print(mark(True))
            print(f"  REAL tokenizer loaded: {cfg.base_model}")
            for i, row in enumerate(samples):
                msgs = build_messages(row, cfg.system_marker)
                text = tok.apply_chat_template(msgs, tokenize=False, add_generation_prompt=False)
                # assistant/response span must be locatable for masking
                if ASSISTANT_HEADER not in text:
                    print(f"  PROBLEM: sample {i}: assistant header not found for masking")
                    c3 = False
                if row["response"] not in text:
                    print(f"  PROBLEM: sample {i}: response span not present in rendered text")
                    c3 = False
                # decode(encode(text)) must round-trip the response substring
                ids = tok(text, add_special_tokens=False)["input_ids"]
                decoded = tok.decode(ids)
                if row["response"] not in decoded:
                    print(f"  PROBLEM: sample {i}: response did not survive encode/decode")
                    c3 = False
            if c3:
                print("  3 samples: apply_chat_template OK, assistant span locatable, "
                      "encode/decode round-trips response substring")
        else:
            skipped_realtok = True
            print("SKIP")
            print(f"  REAL-tokenizer round-trip SKIPPED (offline): {tok_err}")
            print("  Falling back to self-contained ChatML structural check ...")
            struct_ok = True
            for i, row in enumerate(samples):
                text = structural_chatml(row, cfg.system_marker)
                if ASSISTANT_HEADER not in text:
                    print(f"  PROBLEM: sample {i}: assistant header missing")
                    struct_ok = False
                # recover the response span between the assistant header and <|im_end|>
                after = text.split(ASSISTANT_HEADER, 1)[1]
                recovered = after.split("<|im_end|>", 1)[0]
                if recovered != row["response"]:
                    print(f"  PROBLEM: sample {i}: response span not recoverable")
                    struct_ok = False
            if struct_ok:
                print("  structural ChatML check PASSED: assistant/response span recoverable "
                      "for completion-only masking (3 samples)")
            else:
                c3 = False
    ok = ok and c3

    # --- Check 4: VRAM estimate ---
    print(f"\n[4/4] Expected-VRAM estimate ........ {mark(True)}")
    est = estimate_vram_gb(cfg)
    for k, v in est.items():
        print(f"    {k:32s} = {v}")
    print(f"  => estimated peak ~{est['total_gb']} GB : "
          f"4090(24GB) {'FITS' if est['fits_4090_24gb'] else 'OOM-risk'}, "
          f"A100(80GB) {'FITS' if est['fits_a100_80gb'] else 'OOM-risk'}")

    # --- Summary ---
    print("\n" + "=" * 72)
    print("Summary:")
    print(f"  [1] config sanity ............ {mark(c1)}")
    print(f"  [2] jsonl schema ............. {mark(c2)}")
    print(f"  [3] chat-template round-trip . {mark(c3, skip=skipped_realtok)}"
          f"{'  (real-tokenizer SKIPPED, structural PASSED)' if skipped_realtok else ''}")
    print(f"  [4] vram estimate ............ {mark(True)}")
    print("=" * 72)
    if ok:
        print("SMOKE PASSED")
        return 0
    print("SMOKE FAILED")
    return 1


# ------------------------------------------------------------------------------
# Provenance artifacts
# ------------------------------------------------------------------------------
def write_provenance(out_dir: str, cfg: Config, data_path: str, rows_count: int,
                     dataset_sha: str, timestamp: str) -> None:
    os.makedirs(out_dir, exist_ok=True)
    manifest = {
        "base_model": cfg.base_model,
        "config": asdict(cfg),
        "resolved_alpha": cfg.resolved_alpha(),
        "effective_batch": cfg.effective_batch(),
        "seed": cfg.seed,
        "dataset_path": data_path,
        "dataset_sha256": dataset_sha,
        "record_count": rows_count,
        "replay_dataset": cfg.replay_dataset,
        "replay_fraction": cfg.replay_fraction,
        "timestamp": timestamp,
        "success_gate": SUCCESS_GATE_VERBATIM,
        "warm_arm_gate": WARM_ARM_GATE,
    }
    with open(os.path.join(out_dir, "run_manifest.json"), "w") as fh:
        json.dump(manifest, fh, indent=2)

    card = f"""# MTL warm-agent LoRA adapter — MODEL CARD (stub)

Generated by `bench/lora/train_mtl_lora.py`. Refs docs/design/v0.7-lora-warm-agent.md, #83.

## Provenance
- Base model: `{cfg.base_model}` (Apache-2.0)
- Method: QLoRA (NF4 + double-quant, bf16 compute), Unsloth
- Rank r = {cfg.r}, alpha = {cfg.resolved_alpha()}, use_rslora = {cfg.use_rslora}
- Target modules: {cfg.target_modules}
- LR {cfg.lr}, epochs {cfg.epochs}, warmup_ratio {cfg.warmup_ratio}, weight_decay {cfg.weight_decay}
- max_seq_len {cfg.max_seq_len}, effective batch {cfg.effective_batch()}, packing {cfg.packing}
- Seed: {cfg.seed}
- Dataset path: `{data_path}`
- Dataset sha256 (dataset.jsonl): `{dataset_sha}`
- Record count: {rows_count}
- Replay: `{cfg.replay_dataset}` @ fraction {cfg.replay_fraction}
- Vocab extension: NONE (raw BPE, v1)
- Trained: {timestamp}

## Full resolved config
```json
{json.dumps(asdict(cfg), indent=2)}
```

## Success gate (pre-registered, verbatim from design doc §6)
```
{SUCCESS_GATE_VERBATIM}
```
Warm-arm pre-registered gate: `{WARM_ARM_GATE}`

## Contamination gate
contamination gate: PENDING  (operator: run `cargo test -p mtl-datagen` on the
FINAL training set and confirm PASS before uploading this adapter; then change
this line to `contamination gate: PASS`). This is the sealed-manifest gate
tracked under issue #53 (salt `mtl-sealed-v1:issue-53`).

## Intended use
Warm MTL agent: emits MTL programs with ZERO quickref preamble. Serve via vLLM
`--enable-lora` (see bench/lora/serve_adapter.py) as the WARM arm of the
agent-trial battery.
"""
    with open(os.path.join(out_dir, "MODEL_CARD.md"), "w") as fh:
        fh.write(card)


# ------------------------------------------------------------------------------
# REAL TRAINING PATH — every heavy import is lazy and lives here.
# ------------------------------------------------------------------------------
def train(cfg: Config, data_path: str, out_dir: str, resume: bool, timestamp: str) -> int:
    # Lazy heavy imports (GPU / torch): NEVER import these at module top level.
    import torch  # noqa: F401
    from datasets import Dataset
    from unsloth import FastLanguageModel
    from unsloth.chat_templates import train_on_responses_only
    from trl import SFTConfig, SFTTrainer

    print(f"[train] base={cfg.base_model} r={cfg.r} alpha={cfg.resolved_alpha()} "
          f"rslora={cfg.use_rslora} effbatch={cfg.effective_batch()}")

    # ---- data ----
    rows = load_jsonl(data_path)
    problems, stats = validate_rows(rows)
    if problems:
        print("FATAL: dataset schema problems:", *problems[:10], sep="\n  ")
        return 1
    dataset_sha = sha256_of_file(data_path)
    print(f"[train] {stats['count']} records  tiers={stats['tier_hist']}  sha256={dataset_sha[:16]}...")

    train_rows, eval_rows = carve_eval_split(rows, cfg.eval_fraction, cfg.seed)
    print(f"[train] split: {len(train_rows)} train / {len(eval_rows)} eval "
          f"(fraction={cfg.eval_fraction}, seed={cfg.seed})")

    # ---- model + tokenizer (QLoRA / NF4) ----
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=cfg.base_model,
        max_seq_length=cfg.max_seq_len,
        dtype=None,               # auto bf16
        load_in_4bit=True,        # NF4 4-bit
    )
    model = FastLanguageModel.get_peft_model(
        model,
        r=cfg.r,
        lora_alpha=cfg.resolved_alpha(),
        lora_dropout=cfg.lora_dropout,
        target_modules="all-linear" if cfg.target_modules == "all-linear" else cfg.target_modules,
        use_rslora=cfg.use_rslora,
        use_gradient_checkpointing="unsloth" if cfg.gradient_checkpointing else False,
        random_state=cfg.seed,
        bias="none",
    )

    def render(row: dict) -> str:
        msgs = build_messages(row, cfg.system_marker)
        return tokenizer.apply_chat_template(msgs, tokenize=False, add_generation_prompt=False)

    mtl_texts = [render(r) for r in train_rows]

    # ---- general-code replay (full-sequence causal LM loss) ----
    replay_texts: list[str] = []
    if cfg.replay_fraction > 0:
        mtl_tok_budget = sum(len(tokenizer(t, add_special_tokens=False)["input_ids"]) for t in mtl_texts)
        target_replay_toks = int(mtl_tok_budget * cfg.replay_fraction)
        print(f"[train] MTL token budget ~{mtl_tok_budget}; replay target ~{target_replay_toks} toks "
              f"from {cfg.replay_dataset}")
        try:
            from datasets import load_dataset
            kwargs = {"split": cfg.replay_split, "streaming": True}
            if cfg.replay_data_dir:
                kwargs["data_dir"] = cfg.replay_data_dir
            replay_ds = load_dataset(cfg.replay_dataset, **kwargs)
            acc = 0
            for ex in replay_ds:
                text = ex.get(cfg.replay_text_field)
                if not text:
                    continue
                ids = tokenizer(text, add_special_tokens=False)["input_ids"][: cfg.replay_max_seq_len]
                text = tokenizer.decode(ids)
                replay_texts.append(text)
                acc += len(ids)
                if acc >= target_replay_toks:
                    break
            print(f"[train] mixed in {len(replay_texts)} replay docs (~{acc} toks, "
                  f"code-continuation / full-sequence loss)")
        except Exception as exc:
            print(f"[train] WARNING: replay load failed ({type(exc).__name__}: {exc}); "
                  f"proceeding WITHOUT replay")
            replay_texts = []
    else:
        print("[train] replay_fraction=0 -> skipping replay")

    # Build the combined train dataset. MTL rows are masked to completion-only;
    # replay rows train on the full sequence (standard for replay). We tag each
    # row so we could split collators, but train_on_responses_only masks by the
    # assistant header, which is ABSENT from replay docs -> replay is naturally
    # full-sequence (no assistant header => the collator leaves it unmasked only
    # if present; we therefore append replay as its own field-compatible text and
    # rely on the header anchor for MTL rows).
    combined = [{"text": t, "_replay": False} for t in mtl_texts] + \
               [{"text": t, "_replay": True} for t in replay_texts]
    rng = random.Random(cfg.seed)
    rng.shuffle(combined)
    train_ds = Dataset.from_list([{"text": c["text"]} for c in combined])

    eval_ds = None
    if eval_rows:
        eval_ds = Dataset.from_list([{"text": render(r)} for r in eval_rows])

    # ---- trainer ----
    sft_cfg = SFTConfig(
        output_dir=out_dir,
        per_device_train_batch_size=cfg.per_device_batch_size,
        gradient_accumulation_steps=cfg.grad_accum,
        num_train_epochs=cfg.epochs,
        learning_rate=cfg.lr,
        warmup_ratio=cfg.warmup_ratio,
        weight_decay=cfg.weight_decay,
        lr_scheduler_type=cfg.lr_scheduler_type,
        max_length=cfg.max_seq_len,
        packing=cfg.packing,
        bf16=True,
        logging_steps=10,
        save_steps=cfg.save_steps,
        save_total_limit=cfg.save_total_limit,
        seed=cfg.seed,
        report_to="none",
        eval_strategy="epoch" if eval_ds is not None else "no",
        dataset_text_field="text",
    )
    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=train_ds,
        eval_dataset=eval_ds,
        args=sft_cfg,
    )
    # Completion-only loss: mask everything up to and including the assistant
    # header; loss only on the assistant/response span. Replay docs have no
    # assistant header, so they are trained as full-sequence causal LM.
    trainer = train_on_responses_only(
        trainer,
        instruction_part="<|im_start|>user\n",
        response_part="<|im_start|>assistant\n",
    )

    resume_ckpt = None
    if resume:
        # find last checkpoint in out_dir
        cks = [d for d in (os.listdir(out_dir) if os.path.isdir(out_dir) else [])
               if d.startswith("checkpoint-")]
        if cks:
            resume_ckpt = os.path.join(out_dir, sorted(cks, key=lambda x: int(x.split("-")[1]))[-1])
            print(f"[train] resuming from {resume_ckpt}")

    result = trainer.train(resume_from_checkpoint=resume_ckpt)
    print(f"[train] done. train metrics: {result.metrics}")
    if eval_ds is not None:
        eval_metrics = trainer.evaluate()
        print(f"[train] eval metrics: {eval_metrics}")

    # ---- save adapter + tokenizer + config bundle ----
    adapter_dir = os.path.join(out_dir, "adapter")
    model.save_pretrained(adapter_dir)
    tokenizer.save_pretrained(adapter_dir)
    with open(os.path.join(adapter_dir, "train_config.json"), "w") as fh:
        json.dump(asdict(cfg), fh, indent=2)

    write_provenance(out_dir, cfg, data_path, stats["count"], dataset_sha, timestamp)
    print(f"[train] adapter -> {adapter_dir}")
    print(f"[train] model card + run_manifest -> {out_dir}")
    print("[train] REMEMBER: run `cargo test -p mtl-datagen` (contamination gate #53) "
          "on the final training set and set the model card line to PASS before upload.")
    return 0


# ------------------------------------------------------------------------------
# CLI
# ------------------------------------------------------------------------------
def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="QLoRA fine-tune for the warm MTL agent (v0.7).")
    ap.add_argument("--config", required=True, help="path to YAML config")
    ap.add_argument("--data", default=None, help="MTL dataset.jsonl (default: pilot in smoke)")
    ap.add_argument("--out", default=None, help="output dir (adapter/checkpoints); MUST be on the network volume")
    ap.add_argument("--smoke", action="store_true", help="GPU-free / torch-free preflight validation")
    ap.add_argument("--tokenizer-path", default=None, help="local tokenizer path for the smoke round-trip")
    ap.add_argument("--resume", action="store_true", help="resume from last checkpoint in --out")
    # CLI overrides
    ap.add_argument("--r", type=int, default=None)
    ap.add_argument("--epochs", type=int, default=None)
    ap.add_argument("--timestamp", default=None, help="provenance timestamp (default: now, UTC)")
    args = ap.parse_args(argv)

    cfg = load_config(args.config)
    if args.r is not None:
        cfg.r = args.r
    if args.epochs is not None:
        cfg.epochs = args.epochs

    if args.smoke:
        data_path = args.data or DEFAULT_PILOT
        return smoke(cfg, data_path, args.tokenizer_path)

    # real training
    if not args.data:
        print("FATAL: --data is required for real training (only --smoke defaults to the pilot).")
        return 2
    if not args.out:
        print("FATAL: --out is required for real training (put it on the RunPod network volume).")
        return 2
    timestamp = args.timestamp or datetime.now(timezone.utc).isoformat()
    return train(cfg, args.data, args.out, args.resume, timestamp)


if __name__ == "__main__":
    raise SystemExit(main())
