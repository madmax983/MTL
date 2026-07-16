# MTL replication kit — clean-checkout dry-run evidence (issue #71, AC #7)

This is the captured transcript of running the reviewer-runnable kit end-to-end
in a clean checkout of the repo, at branch `paper`. It is the falsifiable
evidence that the documented command sequence works — each block is labelled
with the exact command that produced it. The non-Verus pipeline
(`./kit/replicate.sh`) was executed in full; the Verus proof gates
(`./kit/proof-gates.sh`) are captured separately below.

All values match the published artifacts. Every `bench/BASELINE*.md` regenerated
**byte-identically** (the `git diff --exit-code` tolerance check passed with an
empty diff — zero drift), so the reproduced ratios equal the committed ones exactly.

---

## `./kit/replicate.sh` — full non-Verus pipeline (RESULT: PASS, 14/14)

Final summary of the actual run:

```
  PASS  tiktoken toolchain installed
  PASS  cargo test --workspace: 322 passed, 0 failed
  PASS  bench/BASELINE.md byte-identical after regeneration
  PASS  T_v0 aggregate 3.72x present (found: 3.72x)
  PASS  bench/BASELINE-TIER2.md byte-identical after regeneration
  PASS  tier-2 o200k 3.87x present (found: 3.87x)
  PASS  tier-2 cl100k 3.92x present (found: 3.92x)
  PASS  bench/BASELINE-TIER3.md byte-identical after regeneration
  PASS  tier-3 exec o200k 1.86x present (found: 1.86x)
  PASS  tier-3 exec cl100k 1.85x present (found: 1.85x)
  PASS  mtl-bench-validate: all corpus/tier2/tier2_v03/sealed vectors pass (14/14 sealed correct)
  PASS  arena oracle: 148/148 programs agree (direct + forced-compaction)
  PASS  mtl-datagen: contamination + coverage + oracle-gate + revalidation pass
  PASS  factorial demo produced 120 (HALT: 120)

  Steps passed: 14   Steps failed: 0
  RESULT: PASS — the full non-Verus MTL pipeline reproduced every headline ratio byte-identically.
```

### `pip3 install -r bench/tokcount/requirements.txt`

```
Successfully installed regex-2026.7.10 tiktoken-0.8.0
```
(tiktoken pinned at **0.8.0** — the tokenizer used for every ratio.)

### `cargo test --workspace`  — expect 0 failed

```
test result: ok. 97 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; ...
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; ...   (mtl-arena)
test result: ok.  6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; ...   (p5_minsky)
... (all crates) ...
```
Summed across the workspace: **322 passed, 0 failed**.

### `python3 bench/tokcount/report.py`  — T_v0 micro baseline (headline 3.72x)

```
[report.py] wrote /home/user/MTL/bench/BASELINE.md
```
Regenerated `bench/BASELINE.md` headline table:

```
| v0.2 (validated) | o200k_base | 93 | 25 | 3.72x | MET |
| v0.2 (validated) | cl100k_base | 93 | 25 | 3.72x | MET |
```
`git diff --exit-code bench/BASELINE.md` → empty (byte-identical).

### `python3 bench/tokcount/report_tier2.py`  — tier-2 (3.87x / 3.92x)

```
[report_tier2.py] wrote /home/user/MTL/bench/BASELINE-TIER2.md
```
Regenerated `bench/BASELINE-TIER2.md` scenario-B aggregate:

```
| v0.3 tier-2 (scenario B) | 11 | o200k_base | 352 | 91 | 3.87x |
| v0.3 tier-2 (scenario B) | 11 | cl100k_base | 349 | 89 | 3.92x |
```
`git diff --exit-code bench/BASELINE-TIER2.md` → empty (byte-identical).

### `python3 bench/tier3/report.py`  — tier-3 executable (1.86x / 1.85x)

```
[wrote /home/user/MTL/bench/BASELINE-TIER3.md]
```
Regenerated `bench/BASELINE-TIER3.md`:

```
| **TOTAL** | **256** | **255** | **136** | **136** | **138** | **138** | **1.86x** |
- **executable**: o200k **1.86x**, cl100k **1.85x**  (the lexer-safe programs actually run).
```
`git diff --exit-code bench/BASELINE-TIER3.md` → empty (byte-identical).
Note: README's older **1.90x** tier-3 figure is superseded by this artifact's
executable **1.86x/1.85x** (the exec column is what the tier3run oracle runs).

### `cargo test -p mtl-bench-validate`  — corpus + tier + sealed validation

```
tests/corpus.rs   : 10 passed; 0 failed   (affine, factorial(+v02), fib, gcd(+v02), is_even, power, rev3, sum_to)
tests/tier2.rs    : 10 passed; 0 failed
tests/tier2_v03.rs: 11 passed; 0 failed
tests/sealed.rs   :  2 passed; 0 failed   (committed_solutions_pass_all_vectors_constructed_stack -> 14/14 sealed correct;
                                           running_max_candidate_is_algorithmically_wrong -> negative control)
```

### `cargo test -p mtl-arena --test oracle`  — 148-case interp-vs-arena differential

```
running 2 tests
test differential_oracle ... ok
test differential_oracle_forced_compaction ... ok
test result: ok. 2 passed; 0 failed; ...
```
Both tests assert the corpus is exactly **148 cases** and **148/148 programs
agree** between the reference interpreter and the arena backend (the second
across a forced arena compaction).

### `cargo test -p mtl-datagen`  — contamination / anti-gaming gates

```
tests/contamination.rs : 8 passed; 0 failed   (incl. sealed_disjoint_from_dev, manifest_matches_sealed_tasks,
                                                planted_*_collision_is_caught)
tests/coverage.rs      : 3 passed; 0 failed
tests/oracle_gate.rs   : 4 passed; 0 failed
tests/revalidation.rs  : 2 passed; 0 failed
```

### `cargo run --bin mtlrun -p mtl-bench-validate -- '5[1][*]&'`  — factorial demo

```
mtlrun output: HALT: 120
```
`5[1][*]&` = PrimRec factorial of 5 → **120**, executed on the reference
interpreter end-to-end.

---

## `./kit/proof-gates.sh` — 5 Verus proof gates (source-built pinned Verus)

Expected scoreboard (each root run as bare `verus <root.rs>` with
`VERUS_Z3_PATH` set, Verus `0.2026.07.05.49b8806` / Z3 `4.12.5` / Rust `1.96.0`):

```
  PASS  mtl_core         76 verified / 0 errors  (expected 76/0)
  PASS  p5_universality  118 verified / 0 errors (expected 118/0)   [HARD GATE]
  PASS  p4_verus         101 verified / 0 errors (expected 101/0)
  PASS  checker_verus    116 verified / 0 errors (expected 116/0)
  PASS  arena_verus      145 verified / 0 errors (expected 145/0)   [HARD GATE]
  RESULT: PASS — all 5 proof roots reproduced (76 + 118 + 101 + 116 + 145 verified, 0 errors).
```

`verus --no-cheating` flags exactly **2** trusted P2 Clone `external_body` stubs
(`Word::clone`, `Value::clone`) — the declared trust boundary, report-only.

In-container proof-gate reproduction verdict (filled by the Verus-build worker):

<!-- VERUS_INCONTAINER_RESULT -->
