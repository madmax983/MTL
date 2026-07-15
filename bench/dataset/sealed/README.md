# Sealed-eval contamination manifest (issue #53)

`sealed.manifest.json` is the **real** sealed-set contamination manifest,
derived from the sealed task set at [`bench/sealed/tasks.json`](../../sealed/tasks.json)
(15 tasks authored blind to the MTL glyph/primitive set — see
[`bench/sealed/README.md`](../../sealed/README.md) and
[`bench/sealed/AUTHORSHIP.md`](../../sealed/AUTHORSHIP.md)). It replaces the
former *placeholder* of reserved canonical forms.

## Shape (`mtl-sealed-manifest/v2`)

An OBJECT, not an array:

```json
{
  "schema": "mtl-sealed-manifest/v2",
  "salt": "mtl-sealed-v1:issue-53",
  "freeze_commit": "",
  "generated_from": "bench/sealed/tasks.json",
  "entries": [
    { "task_id": "seal_collatz_steps", "tier": 0,
      "content_sha256": "<salted spec fingerprint>",
      "io_hash": "<canon::io_hash over the task's I/O vectors>",
      "canonical_sha256": "" },
    ...
  ]
}
```

- `content_sha256` — salted tamper-evidence fingerprint of the task spec
  (`sha256_hex(salt ++ 0x00 ++ serde_json(canonical_spec))`).
- `io_hash` — the same io-behavior hash the dataset generator computes per row,
  so a training row reproducing a sealed task's exact I/O collides.
- `canonical_sha256` — the reference MTL solution's canonical SHA-256, **empty
  until the post-freeze unseal** (reference solutions are withheld). The gate
  treats an empty key as "no key".

The exact `content_sha256` formula, the io-hash construction, the salt, and the
no-peek protocol are documented in
[`bench/sealed/README.md`](../../sealed/README.md).

## Regenerate

```
cargo run -p mtl-datagen --bin mkseal
```

Reads `bench/sealed/tasks.json`, writes this manifest deterministically (pretty
JSON + trailing newline). No randomness — reproducible byte-for-byte.

## What the gate does

`gen` loads this manifest (`load_sealed` → `contamination::parse_manifest`,
which returns `.entries`) and asserts **no dataset item collides** with any
sealed entry by `canonical_sha256` **or** `io_hash`
(`bench/dataset/src/contamination.rs`), emitting `pilot/contamination_report.json`.
The build FAILS on any collision. Empty `canonical_sha256`/`io_hash` keys never
match, so the pre-freeze empty canonical column cannot false-trigger.

`tests/contamination.rs` proves the mechanism in both directions plus the
issue-#53 disjointness / held-out proofs (`sealed_disjoint_from_dev`,
`planted_dev_task_collision_is_caught`, `manifest_matches_sealed_tasks`).
