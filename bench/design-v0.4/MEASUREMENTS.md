# MTL v0.4 "effects" round — Measurements summary (WORK IN PROGRESS)

Skeleton commit. Tables and readings are filled in as each artifact lands.

- Tokenizers: `tiktoken` `o200k_base` + `cl100k_base` at **0.8.0** (the pinned bench set).
- Method: identical to prior rounds — `bench/tokcount/tokcount.py`, `len(enc.encode(text))`,
  single trailing newline stripped. Run from `bench/`.
- Every number in this directory is a real `tokcount` run (command shown). MTL solutions
  are **hand-traced, design-stage, NOT interpreter-validated**.
