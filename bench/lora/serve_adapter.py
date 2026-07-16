#!/usr/bin/env python3
"""
serve_adapter.py — serve the warm MTL LoRA adapter with vLLM (--enable-lora),
exposing an OpenAI-compatible endpoint for the agent-trial battery's WARM arm.

This is a thin, documented wrapper around `vllm serve`. It prints (and by default
execs) the exact command; use --print-only to only show it.

The WARM arm rule: the served adapter emits MTL with ZERO quickref preamble. The
agent-trial battery points its OpenAI-compatible client at this endpoint and sends
the task prompt ONLY (no docs/mtl-quickref.md bytes in the system prompt).

Usage:
  python serve_adapter.py --adapter /workspace/out/r64/adapter
  python serve_adapter.py --adapter /workspace/out/r64/adapter --print-only

Equivalent raw command it runs:
  vllm serve Qwen/Qwen2.5-Coder-7B-Instruct \
      --enable-lora \
      --lora-modules mtl-warm=/workspace/out/r64/adapter \
      --max-lora-rank 128 \
      --dtype bfloat16 \
      --max-model-len 4096 \
      --port 8000 \
      --served-model-name mtl-warm

Then point the battery at it (OpenAI-compatible):
  export OPENAI_BASE_URL=http://<pod-ip>:8000/v1
  export OPENAI_API_KEY=EMPTY
  # request model = "mtl-warm"  (the --lora-modules name, NOT the base model)
  # system prompt = short marker only ("emit MTL"); NO quickref. 0 preamble.

Quick sanity probe once it is up:
  curl http://localhost:8000/v1/chat/completions \
    -H "Content-Type: application/json" \
    -d '{"model":"mtl-warm",
         "messages":[{"role":"system","content":"emit MTL"},
                     {"role":"user","content":"Given an integer n on the stack, compute 1*n + 0."}],
         "temperature":0}'
"""
from __future__ import annotations

import argparse
import shlex
import subprocess
import sys

BASE_MODEL = "Qwen/Qwen2.5-Coder-7B-Instruct"
ADAPTER_NAME = "mtl-warm"


def build_command(adapter: str, port: int, max_lora_rank: int, max_model_len: int) -> list[str]:
    return [
        "vllm", "serve", BASE_MODEL,
        "--enable-lora",
        "--lora-modules", f"{ADAPTER_NAME}={adapter}",
        "--max-lora-rank", str(max_lora_rank),   # >= r used in training (128 covers the whole sweep)
        "--dtype", "bfloat16",
        "--max-model-len", str(max_model_len),
        "--port", str(port),
        "--served-model-name", ADAPTER_NAME,
    ]


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Serve the warm MTL LoRA adapter via vLLM.")
    ap.add_argument("--adapter", required=True, help="path to <out>/adapter")
    ap.add_argument("--port", type=int, default=8000)
    ap.add_argument("--max-lora-rank", type=int, default=128,
                    help="must be >= the training rank r (128 covers the r=32/64/128 sweep)")
    ap.add_argument("--max-model-len", type=int, default=4096)
    ap.add_argument("--print-only", action="store_true", help="print the command and exit")
    args = ap.parse_args(argv)

    cmd = build_command(args.adapter, args.port, args.max_lora_rank, args.max_model_len)
    print("# vLLM serve command:")
    print(" ".join(shlex.quote(c) for c in cmd))
    print()
    print(f"# WARM arm: request model='{ADAPTER_NAME}', system prompt = short marker only, 0 preamble.")
    print(f"# Point the battery at:  OPENAI_BASE_URL=http://<pod-ip>:{args.port}/v1  OPENAI_API_KEY=EMPTY")

    if args.print_only:
        return 0
    try:
        return subprocess.call(cmd)
    except FileNotFoundError:
        print("\nERROR: `vllm` not found. Install bench/lora/requirements.txt on the pod first.",
              file=sys.stderr)
        return 127


if __name__ == "__main__":
    raise SystemExit(main())
