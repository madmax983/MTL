# `bench/design-v0.2/` — design-stage candidates for v0.2 recursion primitives

**DESIGN STAGE.** These programs use candidate glyphs (`&` primrec, `.` times, `|` linrec, `>` uncons) that are **not yet implemented** in the parser or interpreter. They are **hand-traced** against the semantics sketches in `docs/design/v0.2-recursion-primitives.md`, **not** interpreter-validated. Token counts are real (measured with `bench/tokcount`); correctness is by hand-trace.

Nothing here is on the `bench/validate` discovery path (which is hardcoded to `bench/corpus/<task>/mtl/solution.mtl` for the five `T_v0` tasks) and nothing here is referenced by `bench/tokcount/tasks.json`, so `cargo test` and the generated `bench/BASELINE.md` are unaffected by this directory.

- `candidates/*.mtl` — design-stage MTL v0.2 solutions (candidate glyphs).
- `python/*.py` — idiomatic + minified Python baselines for the new `T_v0.2-dev` tasks (fib, sum_to, power).
- `measurements.md` — the full token sweep that drove glyph selection.
- `measure.py` — reproduces every count against the pinned `o200k_base`/`cl100k_base` encoders.

Run: `python3 bench/design-v0.2/measure.py`
