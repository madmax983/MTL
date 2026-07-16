#!/usr/bin/env bash
# kit/proof-gates.sh — reviewer-runnable Verus proof-gate confirmation (issue #71).
#
# Runs the 5 MTL proof roots through a PINNED source-built Verus and asserts each
# reaches its exact "N verified, 0 errors" count. Usable by someone who does NOT
# trust the repo's CI: it re-checks the actual .rs proof artifacts locally.
#
#   Pins: Verus 0.2026.07.05.49b8806 · Z3 4.12.5 · Rust 1.96.0
#   Invocations are the EXACT ones from .github/workflows/ci.yml and the
#   crates/*/proof-log.txt files (bare `verus <root.rs>`, no extra flags).
#
# HARD GATES (exit non-zero if they miss their count or hit any error):
#   - p5_universality.rs  (P5 Turing-completeness / Minsky construction)
#   - arena_verus.rs      (arena refinement of spec_step)
# The other three (mtl_core, checker_verus, p4_verus) are also asserted; any
# miss on any root makes the script exit non-zero, but the two above are the
# named admit-free hard gates.
#
# Usage:
#   ./kit/proof-gates.sh                 # auto-locates verus + z3
#   VERUS_BIN=/path/to/verus ./kit/proof-gates.sh
# Build the toolchain first if you don't have it:  ./kit/build-verus.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"

# --- (a) locate a usable verus binary -------------------------------------
DEFAULT_VERUS=/root/verus-src/source/target-verus/release/verus
if [ -n "${VERUS_BIN:-}" ] && [ -x "${VERUS_BIN}" ]; then
  VERUS="$VERUS_BIN"
elif command -v verus >/dev/null 2>&1; then
  VERUS="$(command -v verus)"
elif [ -x "$DEFAULT_VERUS" ]; then
  VERUS="$DEFAULT_VERUS"
else
  cat >&2 <<EOF
ERROR: no usable 'verus' binary found.
  Looked at: \$VERUS_BIN, PATH, and $DEFAULT_VERUS
  Build the pinned toolchain from source first:
      ./kit/build-verus.sh
  then re-run:
      VERUS_BIN=$DEFAULT_VERUS ./kit/proof-gates.sh
EOF
  exit 2
fi
echo "verus binary: $VERUS"

# --- (b) set VERUS_Z3_PATH -------------------------------------------------
if [ -z "${VERUS_Z3_PATH:-}" ]; then
  for cand in /root/verus-src/source/z3 /root/z3-src/build/z3; do
    if [ -x "$cand" ]; then export VERUS_Z3_PATH="$cand"; break; fi
  done
fi
if [ -z "${VERUS_Z3_PATH:-}" ]; then
  echo "ERROR: VERUS_Z3_PATH is unset and no z3 found (run ./kit/build-verus.sh)." >&2
  exit 2
fi
echo "VERUS_Z3_PATH: $VERUS_Z3_PATH"

echo "verus version:"
"$VERUS" --version 2>&1 | sed 's/^/  /' || true

# --- proof roots + expected verified counts (M must be 0 for every root) ---
# name|path|expected_verified|hardgate(1=yes)
ROOTS=(
  "mtl_core|crates/mtl-core/src/mtl_core.rs|76|0"
  "p5_universality|crates/mtl-core/src/p5_universality.rs|118|1"
  "p4_verus|crates/mtl-syntax/proofs/p4_verus.rs|101|0"
  "checker_verus|crates/mtl-core/src/checker_verus.rs|116|0"
  "arena_verus|crates/mtl-arena/proofs/arena_verus.rs|145|1"
)

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
declare -a SCORE
OVERALL=0

for entry in "${ROOTS[@]}"; do
  IFS='|' read -r NAME REL EXP HARD <<<"$entry"
  echo
  echo "==================================================================="
  echo "== GATE: $NAME  ($REL)   expect: $EXP verified, 0 errors$([ "$HARD" = 1 ] && echo '   [HARD GATE]')"
  echo "==================================================================="
  LOG="$WORK/$NAME.log"
  # (c) EXACT invocation — bare `verus <root.rs>` per ci.yml / proof-log.txt.
  set +e
  "$VERUS" "$REL" 2>&1 | tee "$LOG"
  set -e
  # (d) parse "N verified, M errors"
  LINE="$(grep -oE "[0-9]+ verified, [0-9]+ errors" "$LOG" | tail -1 || true)"
  GOTV="$(echo "$LINE" | grep -oE "^[0-9]+" || echo "?")"
  GOTE="$(echo "$LINE" | grep -oE "[0-9]+ errors" | grep -oE "^[0-9]+" || echo "?")"
  if [ "$GOTV" = "$EXP" ] && [ "$GOTE" = "0" ]; then
    echo ">> $NAME: PASS ($LINE)"
    SCORE+=("PASS  $NAME  $GOTV verified / $GOTE errors  (expected $EXP/0)")
  else
    echo ">> $NAME: FAIL (got '${LINE:-<no verification line>}', expected $EXP verified, 0 errors)"
    SCORE+=("FAIL  $NAME  got '${LINE:-none}'  (expected $EXP/0)")
    OVERALL=1
    # (e) hard gates force a non-zero exit; non-hard misses also fail the run.
  fi
done

echo
echo "==================================================================="
echo "== PROOF-GATE SCOREBOARD"
echo "==================================================================="
printf '  %s\n' "${SCORE[@]}"
echo
if [ "$OVERALL" -ne 0 ]; then
  echo "  RESULT: FAIL — at least one proof root did not reach its verified,0-errors count."
  exit 1
fi
echo "  RESULT: PASS — all 5 proof roots reproduced (76 + 118 + 101 + 116 + 145 verified, 0 errors)."
echo "  NOTE:  \`verus --no-cheating\` flags exactly 2 trusted P2 Clone external_body"
echo "         stubs (Word::clone, Value::clone) — the declared trust boundary, report-only."
