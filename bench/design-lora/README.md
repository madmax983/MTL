# bench/design-lora — per-tokenizer MTL compression study

The centerpiece experiment for the LoRA fine-tuning report: **does MTL's
token-compression edge survive under the tokenizers of candidate open-weights
base models?**

MTL's economic thesis is that BPE tokenizers *merge* adjacent ASCII-punctuation
glyphs, so a whitespace-free MTL program can cost **< 1 token per primitive
glyph** and beat idiomatic (whitespace-separated) Python. That was only ever
measured on OpenAI tiktoken (`o200k_base` / `cl100k_base`). A LoRA fine-tune,
however, trains on top of a specific open-weights base model, and *that* model's
tokenizer decides whether the compression holds. A base model whose BPE shreds
ASCII punctuation changes the economics — so we measure it directly.

## What it does

`tokcompress.py`:

1. Loads a paired **MTL <-> idiomatic-Python** corpus using the same pairing
   `bench/tokcount/tasks.json` encodes (so ratios are comparable to the repo's
   published 3.72x/3.87x numbers), plus the `bench/tier3/tasks` capability
   programs and their python twins. For each corpus task the newest MTL variant
   present is used (v0.3 > v0.2 > v0.1).
2. For **every tokenizer** computes: total MTL tokens, total Python tokens,
   compression ratio (`Python / MTL`), mean tokens/MTL-program, tokens per
   primitive-glyph (is it `< 1`?), bytes-per-token on MTL, and the share of MTL
   programs whose token count exceeds their glyph count (fragmentation).
3. Tokenizes the cold-preamble `docs/mtl-quickref.md` under each tokenizer.
4. Writes a sorted markdown table + readout to [`RESULTS.md`](./RESULTS.md).

Tokenizers: tiktoken `o200k_base` and `cl100k_base` (baselines / sanity anchor),
and HuggingFace `AutoTokenizer` for the open-weights candidates
(Qwen2.5-Coder-7B, DeepSeek-Coder-V2-Lite, StarCoder2-7B, Codestral-22B,
Mistral-7B-v0.3, Llama-3.1-8B, CodeGemma-7B). Gated repos (`meta-llama/*`,
`google/*`) fall back to an ungated mirror carrying the identical tokenizer,
recorded as a substitution in the provenance table.

## Reproduce

```bash
python3 -m venv .venv && source .venv/bin/activate
pip install -r bench/design-lora/requirements.txt
python3 bench/design-lora/tokcompress.py    # writes bench/design-lora/RESULTS.md
```

Only **tokenizer files** are downloaded (a few MB each), never model weights;
`torch` is not required. Network egress to the HuggingFace Hub is needed on the
first run (tokenizers are then cached; set `HF_HOME` to control the cache
location). Do **not** commit the venv or the HF cache.

## Files

- `tokcompress.py` — the self-contained harness.
- `requirements.txt` — pinned `tiktoken==0.8.0` plus the HF tokenizer stack.
- `RESULTS.md` — generated table + readout (checked in).
