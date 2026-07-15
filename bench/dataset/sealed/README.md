# Sealed-eval contamination placeholder (feeds #53)

The real sealed-eval split is **reserved-empty in-repo** today: every task in
`bench/tokcount/tasks.json` carries `"split": "dev"`, and `bench/BASELINE.md` /
`bench/README.md` state that `train` and `sealed-eval` are reserved-empty. There
is no committed sealed manifest anywhere in the tree.

Authoring the actual sealed tasks is the job of **issue #53** ("Populate the
sealed eval set and report held-out results"), which is explicitly *out of scope*
for the data-factory crate. What the factory owns is the **hash-disjoint
machinery**: the two-key mechanical dedup gate that #83 requires so a warm number
on a contaminated task is void.

## What `sealed.manifest.json` is

`sealed.manifest.json` is the visible no-peek artifact in the shape #53 mandates
— a JSON array of `{task_id, tier, canonical_sha256, io_hash}`. It is a
**placeholder** populated with a handful of *reserved canonical forms* — programs
the generators are constructed never to emit (out-of-range affine constants, an
n^4 form, a literal-quote concatenation, …). Each row carries the program's real
`mtl_syntax::print` canonical SHA-256 and its io-behavior-vector hash.

Regenerate it with:

```
cargo run -p mtl-datagen --bin mkseal
```

## What the gate does

`gen` loads this manifest and asserts **no dataset item collides** with any
sealed entry by canonical-SHA-256 **or** io-hash (`bench/dataset/src/contamination.rs`),
emitting `pilot/contamination_report.json`. The build FAILS on any collision. The
`tests/contamination.rs` suite proves both directions: a clean dataset passes and
a planted canonical/io collision is caught.

When #53 authors the real sealed tasks, replace this placeholder manifest with
the real `{task_id, tier, canonical_sha256, io_hash}` rows (plus the withheld
`sealed.jsonl` reference solutions, encrypted/held-out per the no-peek protocol);
the gate and CI wiring here work unchanged against them.
