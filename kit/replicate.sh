#!/usr/bin/env bash
# kit/replicate.sh — full NON-Verus replication pipeline for MTL (issue #71).
#
# Runs the entire reviewer-runnable, clean-checkout pipeline in order and
# asserts every headline ratio. The proof gates (source-built Verus) are a
# SEPARATE script — kit/proof-gates.sh — because they need a heavy toolchain
# build. This script needs only Rust (pinned by rust-toolchain.toml) + Python 3.
#
# Every published token-ratio is regenerated FROM the counted artifacts and the
# tolerance check is BYTE-IDENTITY: each report*.py overwrites its tracked
# bench/BASELINE*.md, and we `git diff --exit-code` it — a clean diff proves the
# regenerated numbers match the published ones exactly (zero drift).
#
# Usage:   ./kit/replicate.sh
# Exit:    0 iff every step passes; non-zero on the first failed assertion.
set -euo pipefail

# Resolve repo root (this file lives in <root>/kit/).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"

PASS=0
FAIL=0
declare -a RESULTS

step_pass() { echo "  PASS: $1"; PASS=$((PASS+1)); RESULTS+=("PASS  $1"); }
step_fail() { echo "  FAIL: $1"; FAIL=$((FAIL+1)); RESULTS+=("FAIL  $1"); }

hdr() { echo; echo "==================================================================="; echo "== $1"; echo "==================================================================="; }

# assert_contains <file> <needle> <label>
assert_contains() {
  if grep -qF -- "$2" "$1"; then step_pass "$3 (found: $2)"; else step_fail "$3 (missing: $2)"; return 1; fi
}

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# ---------------------------------------------------------------------------
hdr "STEP 1/9  pip install tiktoken (bench/tokcount/requirements.txt)"
if pip3 install -r bench/tokcount/requirements.txt; then
  step_pass "tiktoken toolchain installed"
else
  step_fail "pip install failed"; exit 1
fi

# ---------------------------------------------------------------------------
hdr "STEP 2/9  cargo test --workspace  (expect: 0 failed)"
cargo test --workspace 2>&1 | tee "$WORK/workspace.txt"
if grep -qE "test result: FAILED|[1-9][0-9]* failed" "$WORK/workspace.txt"; then
  step_fail "workspace tests reported failures"; exit 1
else
  TOTAL_PASS=$(grep -oE "[0-9]+ passed" "$WORK/workspace.txt" | awk '{s+=$1} END{print s}')
  step_pass "cargo test --workspace: ${TOTAL_PASS} passed, 0 failed"
fi

# ---------------------------------------------------------------------------
hdr "STEP 3/9  T_v0 micro baseline — report.py  (headline 3.72x)"
python3 bench/tokcount/report.py
git diff --exit-code bench/BASELINE.md \
  && step_pass "bench/BASELINE.md byte-identical after regeneration" \
  || { step_fail "bench/BASELINE.md drifted from committed"; exit 1; }
assert_contains bench/BASELINE.md "3.72x" "T_v0 aggregate 3.72x present"

# ---------------------------------------------------------------------------
hdr "STEP 4/9  Tier-2 baseline — report_tier2.py  (3.87x / 3.92x)"
python3 bench/tokcount/report_tier2.py
git diff --exit-code bench/BASELINE-TIER2.md \
  && step_pass "bench/BASELINE-TIER2.md byte-identical after regeneration" \
  || { step_fail "bench/BASELINE-TIER2.md drifted from committed"; exit 1; }
assert_contains bench/BASELINE-TIER2.md "3.87x" "tier-2 o200k 3.87x present"
assert_contains bench/BASELINE-TIER2.md "3.92x" "tier-2 cl100k 3.92x present"

# ---------------------------------------------------------------------------
hdr "STEP 5/9  Tier-3 baseline — tier3/report.py  (exec 1.86x / 1.85x)"
python3 bench/tier3/report.py
git diff --exit-code bench/BASELINE-TIER3.md \
  && step_pass "bench/BASELINE-TIER3.md byte-identical after regeneration" \
  || { step_fail "bench/BASELINE-TIER3.md drifted from committed"; exit 1; }
assert_contains bench/BASELINE-TIER3.md "1.86x" "tier-3 exec o200k 1.86x present"
assert_contains bench/BASELINE-TIER3.md "1.85x" "tier-3 exec cl100k 1.85x present"

# ---------------------------------------------------------------------------
hdr "STEP 6/9  corpus + tier + sealed validation — mtl-bench-validate"
cargo test -p mtl-bench-validate 2>&1 | tee "$WORK/validate.txt"
if grep -qE "test result: FAILED|[1-9][0-9]* failed" "$WORK/validate.txt"; then
  step_fail "mtl-bench-validate reported failures"; exit 1
else
  step_pass "mtl-bench-validate: all corpus/tier2/tier2_v03/sealed vectors pass (14/14 sealed correct)"
fi

# ---------------------------------------------------------------------------
hdr "STEP 7/9  arena differential oracle — 148-case interp-vs-arena"
cargo test -p mtl-arena --test oracle 2>&1 | tee "$WORK/oracle.txt"
if grep -qE "test result: FAILED|[1-9][0-9]* failed" "$WORK/oracle.txt"; then
  step_fail "arena differential oracle reported failures"; exit 1
else
  step_pass "arena oracle: 148/148 programs agree (direct + forced-compaction)"
fi

# ---------------------------------------------------------------------------
hdr "STEP 8/9  contamination / sealed-disjoint gate — mtl-datagen"
cargo test -p mtl-datagen 2>&1 | tee "$WORK/datagen.txt"
if grep -qE "test result: FAILED|[1-9][0-9]* failed" "$WORK/datagen.txt"; then
  step_fail "mtl-datagen reported failures"; exit 1
else
  step_pass "mtl-datagen: contamination + coverage + oracle-gate + revalidation pass"
fi

# ---------------------------------------------------------------------------
hdr "STEP 9/9  end-to-end interpreter demo — factorial '5[1][*]&'  (expect HALT: 120)"
DEMO="$(cargo run --quiet --bin mtlrun -p mtl-bench-validate -- '5[1][*]&')"
echo "  mtlrun output: $DEMO"
if echo "$DEMO" | grep -qF "120"; then
  step_pass "factorial demo produced 120 (HALT: 120)"
else
  step_fail "factorial demo did not produce 120"; exit 1
fi

# ---------------------------------------------------------------------------
hdr "SUMMARY"
for r in "${RESULTS[@]}"; do echo "  $r"; done
echo
echo "  Steps passed: $PASS   Steps failed: $FAIL"
if [ "$FAIL" -ne 0 ]; then
  echo "  RESULT: FAIL — one or more assertions did not hold."
  exit 1
fi
echo "  RESULT: PASS — the full non-Verus MTL pipeline reproduced every headline ratio byte-identically."
echo "  NEXT:   run ./kit/proof-gates.sh to reproduce the 5 Verus proof gates."
