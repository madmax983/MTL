# Sealed evaluation set (`T_sealed-v1`, issue #53)

This directory holds the **held-out sealed evaluation set** for the MTL
benchmark: 15 tasks used to report contamination-free held-out results. The set
is the source of truth for the contamination gate in the `mtl-datagen` crate
(`bench/dataset/`).

## No-peek protocol

The sealed set follows a **public-tasks / withheld-solutions** discipline:

- **Tasks and I/O vectors are public and runnable.** `tasks.json` ships the full
  prompt (`arm_common_desc`), the signature, and every input→output vector for
  both the MTL arm (`mtl.vectors`, as `input_prefix` / `expected_halt` strings)
  and a language-agnostic Python reference arm (`python.vectors`, as
  `args` / `expected`). Anyone can run a candidate solution against these
  vectors.
- **Reference MTL solutions are withheld** until the post-freeze *unseal*. No
  `*.mtl` solution file and no canonical program hash for these tasks is
  committed before the freeze. This is what keeps the number honest: a model
  cannot be trained on the reference programs because they do not exist in-repo
  yet.
- **The manifest of salted content-hashes is the tamper-evidence.** Because the
  tasks are public, the guarantee we need is that the *published* tasks are
  exactly the ones scored, and that no training row reproduces a sealed task's
  behavior. `bench/dataset/sealed/sealed.manifest.json` provides both: a salted
  fingerprint of each task spec (tamper-evidence) and an I/O-behavior hash the
  live data-generation gate excludes against.

Authorship was **blind to the MTL glyph/primitive set** — tasks were designed
from language-agnostic computational semantics alone, with no knowledge of which
operators exist or are "cheap". See [`AUTHORSHIP.md`](./AUTHORSHIP.md) for the
full information-barrier statement and ground-truth verification notes.

## The manifest (`mtl-sealed-manifest/v2`)

`bench/dataset/sealed/sealed.manifest.json` is an object:

```json
{
  "schema": "mtl-sealed-manifest/v2",
  "salt": "mtl-sealed-v1:issue-53",
  "freeze_commit": "",
  "generated_from": "bench/sealed/tasks.json",
  "entries": [ { "task_id": ..., "tier": ..., "content_sha256": ...,
                 "io_hash": ..., "canonical_sha256": "" }, ... ]
}
```

- **`salt`** — the FIXED, documented salt string `mtl-sealed-v1:issue-53`.
  Derivation is fully deterministic (no randomness, no clock), so the manifest
  is reproducible byte-for-byte.
- **`freeze_commit`** — empty until the freeze step, when it is filled with the
  git commit that freezes the sealed set.
- **`tier`** — the task's `tier_num` (0 micro / 2 tier2 / 3 tier3).

### Per-entry hash semantics + exact formulas

- **`content_sha256`** — the **salted tamper-evidence fingerprint** of the task
  spec. A third party recomputes it from `tasks.json` alone:

  ```text
  content_sha256 = sha256_hex( salt.as_bytes() ++ [0x00] ++ canonical_spec_bytes )
  canonical_spec_bytes = serde_json::to_vec(&{
      task_id:   <id>,
      tier:      <tier_num>,
      prompt:    <arm_common_desc>,
      signature: <signature>,
      vectors:   <lexicographically-sorted Vec<String> of
                  "{input_prefix}=>{expected_halt}" over the MTL vectors>,
  })
  ```

  If any published field (prompt, signature, tier, or a vector) is altered, this
  hash changes — that is the tamper evidence.

- **`io_hash`** — the **I/O-behavior-vector hash** (`canon::io_hash`), computed
  over `IoVector`s built from the task's `python.vectors` (each `args` entry → an
  input value; `expected` → a single `Halt` output value; integers become int
  values, arrays become MTL list values). This is the **same** hash the dataset
  generator computes for every training row, so a training row that reproduces a
  sealed task's exact I/O behavior collides and fails the gate — even if the
  program's surface form differs.

- **`canonical_sha256`** — the reference MTL solution's canonical
  (`mtl_syntax::print`) SHA-256. **Empty pre-freeze**: reference solutions are
  withheld until the unseal, so there is no canonical key yet. The gate treats an
  empty `canonical_sha256` (or empty `io_hash`) as "no key" and never registers a
  collision on it, so an empty-string training row cannot false-match.

## Recompute / verify

Regenerate the manifest from the task set:

```
cargo run -p mtl-datagen --bin mkseal
```

This reads `bench/sealed/tasks.json` and writes
`bench/dataset/sealed/sealed.manifest.json` (pretty JSON + trailing newline),
deterministically. Verify + prove the properties:

```
cargo test -p mtl-datagen
```

The `tests/contamination.rs` suite proves, among others:

- `manifest_matches_sealed_tasks` — every `content_sha256` and `io_hash` in the
  committed manifest recomputes exactly from `tasks.json` (reproducible
  tamper-evidence; 15 entries; salt `mtl-sealed-v1:issue-53`).
- `sealed_disjoint_from_dev` — the 15 sealed tasks are I/O-hash-disjoint AND
  id-disjoint from the 10 `bench/agent-trial` dev tasks.
- `planted_dev_task_collision_is_caught` — the gate bites when a training row
  reproduces a held-out dev task's exact I/O.

## Post-freeze unseal

At the freeze step, `freeze_commit` is filled and the withheld reference MTL
solutions (plus their `canonical_sha256` values) are published. The gate and CI
wiring work unchanged: populated `canonical_sha256` fields simply add a second
collision key.
