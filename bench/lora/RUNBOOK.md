# RUNBOOK — RunPod warm-agent LoRA training (laptop → adapter)

Operator guide for training the **v0.7 warm MTL agent**: a QLoRA fine-tune of
`Qwen/Qwen2.5-Coder-7B-Instruct` that writes MTL with **zero quickref preamble in
context**. Recipe contract: [`docs/design/v0.7-lora-warm-agent.md`](../../docs/design/v0.7-lora-warm-agent.md) §6, issue #83.

## Where this lives & why

Everything is under **`bench/lora/`, not `kit/`**, because `kit/` holds only the
replication-driver scripts (replicate.sh, proof-gates.sh, build-verus.sh,
EVIDENCE.md) while every experiment lives under `bench/<name>/` (e.g.
`bench/dataset`, `bench/design-lora`, `bench/agent-trial`). This training
experiment is one more `bench/` experiment, so it belongs here.

---

## Pod choice: RTX 4090 vs A100 80GB (RunPod Secure Cloud)

The 7B QLoRA fits in ~8-10 GB (smoke estimates ~7.7 GB peak at r=64), so **both
GPUs have ample VRAM** — the choice is wall-clock vs $/hr, not capacity.

| | RTX 4090 24GB (~$0.69/hr) | A100 80GB (~$1.40/hr) |
|---|---|---|
| **Single run** (r=64, ~2-3 h) | ~2-3 h → **$1.38-$2.07** | ~2-3 h → **$2.80-$4.20** |
| **Full sweep** (r=32/64/128 + eval, ~14 h / ~6 h) | ~14 h → **$9.66** | ~6 h → **$8.40** |
| VRAM headroom | fine (r=128 is the only OOM-prone rung) | huge (multi-LoRA, bigger replay) |
| Best for | cost-optimizing a **single run** | **full sweep / time-sensitive** work |

**Recommendation:** **A100 80GB if you are running the full r=32/64/128 sweep or
are time-sensitive** — it finishes the sweep in ~6 h for ~$8.40 (cheaper *and*
2.3× faster than the 4090's ~14 h/$9.66, because the A100's higher throughput more
than offsets its higher $/hr over a long job). **RTX 4090 if you are cost-optimizing
a single r=64 run** — ~$1.38-$2.07 is the cheapest way to get one adapter, and its
24 GB is more than enough for a single 7B QLoRA. (Sweep wall-clock totals ~14 h /
~6 h are from the v0.7 platform research; measure your own throughput.)

---

## Template + volume

- **Image:** official RunPod PyTorch/CUDA image, e.g. **`runpod/pytorch:2.4.0-py3.11-cuda12.4.1-devel-ubuntu22.04`** (any `runpod/pytorch:2.x-cu12x`). torch ships in the image — do **not** reinstall it.
- **Deps:** `pip install -r bench/lora/requirements.txt` (pinned; torch comes from the image).
- **Volume:** attach a **~25 GB network volume** (≈15 GB base model + data + adapter + checkpoints) and mount it at `/workspace`. **Put `--out` on the network volume** (`/workspace/out/...`) so checkpoints survive pod eviction and `--resume` can recover.

---

## The command sequence (laptop → adapter): **8 commands**

Launch the pod from the RunPod console/CLI first (pick GPU + attach the network
volume mounted at `/workspace`, image above), then SSH in and run **8 numbered
commands**. This uses the **recommended in-pod data generation** path.

```bash
# 1. Clone the repo (into the network volume so it persists across eviction)
git clone https://github.com/<owner>/MTL /workspace/MTL && cd /workspace/MTL

# 2. Install the Rust toolchain (needed only for in-pod datagen; ~1 min)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && . "$HOME/.cargo/env"

# 3. Install pinned Python deps (torch already in the base image)
pip install -r bench/lora/requirements.txt

# 4. Generate the training set in-pod (deterministic from --seed 0; a few CPU minutes)
cargo run --release -p mtl-datagen --bin gen -- --count 30000 --out bench/dataset/full --seed 0

# 5. Contamination + provenance gate (MANDATORY — the sealed-manifest gate, #53)
cargo test -p mtl-datagen

# 6. Train (auto-exports adapter + MODEL_CARD.md + run_manifest.json to <out>)
python bench/lora/train_mtl_lora.py --config bench/lora/config/r64.yaml \
    --data bench/dataset/full/dataset.jsonl --out /workspace/out/r64

# 7. Push the adapter (edit the destination repo)
huggingface-cli upload <your-hf-repo> /workspace/out/r64/adapter

# 8. Kill the pod to stop billing
runpodctl stop pod "$RUNPOD_POD_ID"
```

That is **8 commands** laptop-to-adapter (fewer than 10). Optional pre-flight
before #6: `python bench/lora/train_mtl_lora.py --smoke --config bench/lora/config/r64.yaml`
(GPU-free; also runs fine on your laptop before you ever rent a pod).

For the **full sweep**, repeat #6 with `config/r32.yaml`, `config/r64.yaml`,
`config/r128.yaml` (each to its own `--out`), then upload each adapter.

---

## Data: generate in-pod vs upload — **RECOMMEND generate in-pod**

Generate the dataset **on the pod** (command #4). Why:
- **Trivial provenance:** the corpus is deterministic from `--seed 0`, so "the data
  is exactly what `gen --seed 0` produces" is self-evident — no upload to audit.
- **CPU-bound, fast:** a few minutes; GPU sits idle at ~$0.05 of wasted GPU-time.
- **One reproducible place:** the whole pipeline (generate → gate → train) runs in
  one environment, and the **contamination gate (#53) runs right there** on the
  exact bytes you train on, before training starts.
- **One extra cost:** installing the Rust toolchain on the pod (command #2,
  `curl … rustup … -y`, ~1 min).

**Upload alternative (one line)** — for operators who pre-generate in CI and want
to skip the Rust toolchain: generate `dataset.jsonl` elsewhere, then get it onto
the pod with any of:
```bash
runpodctl send bench/dataset/full/dataset.jsonl      # -> receive on the pod
# or:  scp dataset.jsonl root@<pod-ip>:/workspace/MTL/bench/dataset/full/
# or:  huggingface-cli download <your-hf-dataset-repo> --local-dir bench/dataset/full
```
If you upload, still run the contamination gate (#53) against the uploaded file
before training, and record its sha256 (the model card does this automatically).

---

## Cost table (RunPod Secure Cloud)

| Job | RTX 4090 @ $0.69/hr | A100 80GB @ $1.40/hr |
|---|---|---|
| Single run (r=64), ~2-3 h | $1.38 – $2.07 | $2.80 – $4.20 |
| Full sweep (r=32/64/128 + eval), ~14 h / ~6 h | ~$9.66 (14 h) | ~$8.40 (6 h) |

Add ~$0.05 GPU-idle for in-pod datagen. A full research cycle of dozens of runs is
~$100-$300 (design §5).

---

## Failure / resume notes

- **Checkpoint cadence:** `save_steps=200`, `save_total_limit=3` (keeps the last 3).
  Configurable per YAML.
- **Resume:** `python bench/lora/train_mtl_lora.py --config … --out /workspace/out/r64 --resume`
  picks up the highest-numbered `checkpoint-N` in `--out`.
- **Network-volume requirement:** `--out` MUST be on the mounted network volume
  (`/workspace/...`). On pod eviction, local disk is lost; the network volume (and
  therefore your checkpoints) survives, so `--resume` works.

### OOM playbook (mainly r=128)

MTL completions are tiny (~90 tok), so **OOM comes from replay / packed sequence
length, not from MTL**. In order:
1. **Drop `per_device_batch_size` to 1 and raise `grad_accum` to hold the effective
   batch constant** (e.g. 1×32 instead of 8×4 — same effective batch of 32, same
   optimization dynamics, far less activation memory). The r128.yaml OOM-fallback
   block has these values commented in.
2. **Ensure `gradient_checkpointing: true`** (on by default in all three configs).
3. **Cap `replay_max_seq_len`** (e.g. 1024 → 512) and/or trim `max_seq_len`.
4. If still tight, lower `replay_fraction` or move to the A100.

---

## Contamination + provenance gate (MANDATORY before upload)

Before pushing any adapter, the operator **MUST** run the sealed-manifest
contamination gate on the FINAL training set and confirm **PASS**:

```bash
cargo test -p mtl-datagen      # runs bench/dataset/tests/contamination.rs (8 tests,
                               # incl. sealed_disjoint_from_dev, committed_pilot_report_is_clean)
```

This is the sealed-manifest gate tracked under **issue #53** (salt string
`mtl-sealed-v1:issue-53`). The training set must be hash-disjoint from the sealed
eval set; a warm number on a task whose solution appears (even normalized) in
training is void. The generated `MODEL_CARD.md` records the dataset **sha256**,
full resolved config, and seed, and carries a `contamination gate: PENDING` line
the operator flips to `PASS` after this test is green.

> **#87 vs #53:** the coordinator referred to a "#87 gate", but **#87 does not
> exist in this repo**. The real, checked-in gate is **#53** (`cargo test -p
> mtl-datagen`). Use #53.

---

## Warm-arm evaluation

Once trained, serve the adapter and run the existing agent-trial batteries against
it as the **WARM arm with ZERO preamble**.

1. **Serve** (see [`serve_adapter.py`](serve_adapter.py)):
   ```bash
   python bench/lora/serve_adapter.py --adapter /workspace/out/r64/adapter
   # vllm serve Qwen/Qwen2.5-Coder-7B-Instruct --enable-lora \
   #   --lora-modules mtl-warm=/workspace/out/r64/adapter --max-lora-rank 128 \
   #   --dtype bfloat16 --max-model-len 4096 --port 8000 --served-model-name mtl-warm
   ```
   This exposes an OpenAI-compatible endpoint at `http://<pod-ip>:8000/v1`.

2. **Point the batteries at it.** The existing write-side batteries under
   `bench/agent-trial/` (tier-2: 10 tasks × 3 trials, oracle `mtlrun`; tier-3:
   8 tasks × 2 trials, oracle `tier3run`) run as the WARM arm by setting
   `OPENAI_BASE_URL=http://<pod-ip>:8000/v1`, `OPENAI_API_KEY=EMPTY`, model
   `mtl-warm`, and a **system prompt containing a short marker only ("emit MTL")
   with ZERO quickref bytes**. The only thing that changes vs the cold arm is the
   inference call: no `docs/mtl-quickref.md` in context.

3. **Run the N\*→1 hypothesis test.** Re-run `bench/agent-trial/sessions/session_econ.py`
   over the 5 price configs. The #80/#88 break-even math: the cold arm pays a
   **4051-token quickref tax = 4051 · 0.10 · $15/M ≈ 0.6077 ¢/task** (#80's cached
   read tax) every task; the **warm arm removes it (Q=0)**, so the #80 negative
   result is predicted to invert and N\* to collapse toward 1.

### Pre-registered success gate (VERBATIM — do not paraphrase)

From `docs/design/v0.7-lora-warm-agent.md` §6:

```
Eval gate  warm pass@1 ≥ cold, tok→correct within ±2, 0 preamble, N* ≤ 2 on ≥3/5 price configs, on the #53 sealed set
```

AND the warm-arm pre-registered gate:

```
warm cspm > Python AND N* ≤ 2
```

Headline numbers (pass@1, cspm, N\* inversion) **must land on the #53 sealed set**,
with the dev−sealed delta reported for every metric (a large positive delta is the
signature of the LoRA having memorized dev tasks).
