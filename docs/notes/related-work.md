# Related work: pxpipe and the read-tax discipline

This note records what MTL borrows from [pxpipe](https://github.com/teamchong/pxpipe)
and, more importantly, where MTL's structure diverges from it. It distinguishes
throughout between what pxpipe *reports* (cited) and what MTL *claims*.

## What pxpipe is

[pxpipe](https://github.com/teamchong/pxpipe) is a context-as-images compressor:
a local proxy that intercepts Claude Code API traffic and renders token-dense
context (system prompts, tool docs, older history) into PNG images before
transmission, betting that vision tokens are cheaper than text tokens because
"an image is billed by pixel area, not character count." On real traffic it
reports roughly **68% input-token savings** on dense content (856k → 277k
tokens), and a density improvement of about **4.6×** over the plain-text baseline
([README](https://github.com/teamchong/pxpipe/blob/main/README.md)).

## The finding we care about

pxpipe's value to us is its **eval discipline**, not its mechanism. Its
needle-in-haystack testing on verbatim retrieval (extracting random 12-char hex
strings) reports a text baseline of 15/15 but **0/15 on the PNG path (Opus 4.8)**
— and, critically, the failures were **silent confabulations**: the model
returned plausible-but-wrong values (e.g. `30000→60000`, `32→1`) rather than
signalling uncertainty, because vision acts as lossy feature summarization, not
OCR ([FINDINGS.md](https://github.com/teamchong/pxpipe/blob/main/FINDINGS.md)).

The lesson pxpipe draws, and that we adopt: **token savings that make the model
misread are a net loss, because the misreads are silent.** pxpipe reframes
itself as a *lossy gist-compressor* — safe for semantic recall, unsafe for
byte-exact data. It also reports a model-generation effect: re-testing across a
model bump left verbatim unchanged at 0/15 while "semantic" recall drifted
(4/15 → 6/15), but a two-proportion test across the bump was statistically
indistinguishable from chance (**z ≈ 0.76, p ≈ 0.45**), with hits clustering on
round numbers that language priors would predict anyway
([FINDINGS.md](https://github.com/teamchong/pxpipe/blob/main/FINDINGS.md)). The
takeaway we keep: any density claim has to be re-validated per model generation,
because the retrieval "knee" moves.

## What MTL structurally fixes

MTL is also a density play, but its structure is designed so pxpipe's fatal
failure mode cannot occur silently:

- **Discrete whole tokens, not lossy mush.** MTL programs are exact sequences of
  single-character glyphs ([mtl-quickref.md](../mtl-quickref.md)). There is no
  image raster and no BPE-averaged approximation to misread — the bytes are the
  program.
- **LOUD typed faults, not silent wrong answers.** A misread does not produce a
  plausible wrong result; it produces a typed fault — `Underflow`,
  `TypeMismatch`, `Overflow`, or `DivByZero` — and execution halts with no
  partial result ([mtl-quickref.md](../mtl-quickref.md#faults)). pxpipe's core
  risk is that misreads are *silent*; in MTL they surface.
- **A verified interpreter as ground truth.** MTL's reference interpreter carries
  a Verus specification (`crates/mtl-core`), so a prediction about what a program
  does is *checkable* against a formal ground truth rather than trusted.
- **Escape-lane doctrine.** MTL is a dense *lane*, not a replacement. It is
  opt-in and never forced: you can always fall back to prose or Python, and the
  agent-writability trial charges MTL against exactly that Python baseline
  ([bench/agent-trial/README.md](../../bench/agent-trial/README.md)).

## What we adopted from pxpipe's methodology

- **A read-tax eval battery** (`bench/agent-trial/readtax/`): comprehension,
  verbatim-recall, mutation-detection, and confabulation-guard tests, modelled on
  pxpipe's insistence on measuring silent-misread risk rather than only average
  quality.
- **A tokenizer-drift guard** (`bench/tokcount/drift.py`, wired as **non-blocking
  CI**): because pxpipe found the retrieval knee shifts across model
  generations, we track tokenizer/density drift over time rather than trusting a
  one-time number. It is non-blocking by design — a drift signal is a prompt to
  re-measure, not a gate.
- **The escape-lane doctrine** itself: pxpipe's post-mortem is that a compressor
  presented as transparent, but actually lossy, is dangerous. MTL keeps the dense
  lane strictly opt-in.

---

The MTL spec ([docs/mtl-spec.md](../mtl-spec.md)) can link this note in a later
round; it is intentionally left unedited here.
