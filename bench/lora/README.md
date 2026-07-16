# bench/lora — warm-agent LoRA training kit (v0.7)

Trains the **warm MTL agent**: a QLoRA fine-tune of
`Qwen/Qwen2.5-Coder-7B-Instruct` that emits MTL with **zero quickref preamble in
context**, deleting the ~4051-token quickref tax that PR #80 proved structurally
blocks per-task economics. Recipe contract: `docs/design/v0.7-lora-warm-agent.md`
§6, issue #83.

> Lives under `bench/` (not `kit/`) because `kit/` holds only replication-driver
> scripts; every experiment lives under `bench/<name>/`.

## Start here → [`RUNBOOK.md`](RUNBOOK.md)

Operator guide, laptop → adapter in **8 commands**: pod choice (4090 vs A100),
image + network volume, in-pod datagen, the mandatory contamination gate (#53),
train, serve, and the pre-registered warm-arm eval gate.

## Files

| File | Purpose |
|---|---|
| [`RUNBOOK.md`](RUNBOOK.md) | Operator document: pod choice, command sequence, cost table, OOM playbook, eval wiring. |
| [`train_mtl_lora.py`](train_mtl_lora.py) | Config-driven QLoRA training script. Completion-only loss, general-code replay, eval holdout, adapter + model-card + manifest export, `--resume`, and a GPU-free **`--smoke`** preflight. |
| [`serve_adapter.py`](serve_adapter.py) | vLLM `--enable-lora` serve wrapper → OpenAI-compatible endpoint for the WARM arm. |
| [`config/r64.yaml`](config/r64.yaml) | Primary config (r=64). |
| [`config/r32.yaml`](config/r32.yaml) | Sweep low end (r=32). |
| [`config/r128.yaml`](config/r128.yaml) | Sweep high end (r=128) + OOM-fallback block. |
| [`requirements.txt`](requirements.txt) | Pinned deps (torch comes from the RunPod base image). |

## Quick preflight (no GPU, no torch)

```bash
python bench/lora/train_mtl_lora.py --smoke --config bench/lora/config/r64.yaml
```

Validates config, JSONL schema, chat-template + tokenizer round-trip (real Qwen
tokenizer if available, self-contained structural check offline), and prints a
VRAM estimate. Runs on the committed pilot (`bench/dataset/pilot/dataset.jsonl`)
by default. Exits 0 on success.
