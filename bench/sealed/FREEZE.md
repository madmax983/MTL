# Measurement Freeze — Sealed Eval Set (issue #53)

- **Freeze declared:** 2026-07-15, issue #53.
- **Seal commit (sealed set fixed):** `456292bb3a0b930f51c49fa50e37c339ae4eaf59`

## Statement

As of the seal commit above, the MTL glyph set and primitive set are FROZEN
for the purpose of this held-out measurement. The sealed set is unsealed
exactly once, after this freeze, to author reference solutions and run the
held-out batteries. No primitive or glyph may be admitted in response to a
sealed task; any gap a sealed task surfaces is recorded in
bench/sealed/GAPS.md, not patched (per §10.1 / §11.7 of docs/mtl-spec.md and
issue #53).

## How a third party verifies the set was fixed before the freeze and unchanged after

- `cargo test -p mtl-datagen manifest_matches_sealed_tasks` recomputes every
  content/io hash from `bench/sealed/tasks.json` and asserts an exact match to
  the committed manifest.
- `git log -- bench/sealed/tasks.json bench/dataset/sealed/sealed.manifest.json`
  shows they were fixed at the seal commit and (aside from filling
  `freeze_commit`) unchanged after.
