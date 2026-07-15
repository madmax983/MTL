#!/usr/bin/env bash
# Run a bounded cargo-fuzz smoke and gate PRECISELY (issue #58).
#
# Usage: fuzz-smoke.sh <target> <max_total_time_seconds>
#
# libFuzzer distinguishes the artifacts it saves by prefix:
#   * crash-*   — a Rust panic or (in the differential target) an ENGINE
#                 DIVERGENCE. This is a real correctness bug and MUST fail the
#                 job (AC: "fails on any new panic or oracle/round-trip
#                 disagreement").
#   * timeout-* — a unit that ran longer than -timeout. For the executing
#                 targets this is an adversarial quote-doubling loop whose
#                 structure grows exponentially per step (step-fuel bounds
#                 steps, not memory). A resource-exhaustion / proof-to-production
#                 concern (#19), reported ADVISORY here — it does not flake the
#                 gate.
#   * oom-*     — out-of-memory: same resource class, advisory.
#
# So we run the target allowing a non-zero exit, then FAIL only if a `crash-*`
# artifact exists, while surfacing any timeout/oom to the step summary.
set -uo pipefail

target="${1:?target name required}"
budget="${2:?max_total_time seconds required}"
artdir="fuzz/artifacts/${target}"

# Clear stale artifacts from a previous step so we only judge THIS run.
rm -f "${artdir}"/crash-* "${artdir}"/timeout-* "${artdir}"/oom-* 2>/dev/null || true

set -x
# Force the nightly toolchain explicitly. cargo-fuzz needs nightly for the
# sanitizer/coverage instrumentation (`-Zsanitizer=address`), but the repo-root
# `rust-toolchain.toml` pins `channel = "stable"` for the rest of the workspace.
# That toolchain file is a directory-hierarchy override that also applies here,
# so a bare `cargo fuzz run` resolves to STABLE and fails with
# "error: the option `Z` is only accepted on the nightly compiler". `+nightly`
# bypasses the override (matching the documented local `cargo +nightly fuzz`).
cargo +nightly fuzz run "${target}" -- \
  -max_total_time="${budget}" \
  -rss_limit_mb=2048 \
  -timeout=25 \
  -print_final_stats=1
fuzz_rc=$?
set +x

crashes=$(ls "${artdir}"/crash-* 2>/dev/null || true)
timeouts=$(ls "${artdir}"/timeout-* "${artdir}"/oom-* 2>/dev/null || true)

if [ -n "${crashes}" ]; then
  {
    echo "### FUZZ FINDING (${target}): panic or engine divergence — GATE FAILED"
    echo "libFuzzer saved crash artifact(s):"
    echo '```'
    echo "${crashes}"
    echo '```'
    echo "Reproduce: \`cargo fuzz run ${target} <artifact>\`"
  } >> "${GITHUB_STEP_SUMMARY:-/dev/stdout}"
  echo "::error::fuzz ${target}: crash artifact present (panic/divergence)"
  exit 1
fi

if [ -n "${timeouts}" ]; then
  {
    echo "### Fuzz advisory (${target}): resource-exhaustion input (timeout/oom), NOT a divergence"
    echo "A quote-doubling loop grows structure exponentially per step; step-fuel"
    echo "bounds steps, not memory (see docs/ci-reliability.md §3). Reported, not gated."
    echo '```'
    echo "${timeouts}"
    echo '```'
  } >> "${GITHUB_STEP_SUMMARY:-/dev/stdout}"
  echo "::warning::fuzz ${target}: resource-exhaustion input (timeout/oom), advisory"
  exit 0
fi

# No artifacts at all: clean run (fuzz_rc may still be non-zero only if libFuzzer
# itself errored without saving an artifact — treat that as a real failure).
if [ "${fuzz_rc}" -ne 0 ]; then
  echo "::error::fuzz ${target}: libFuzzer exited ${fuzz_rc} with no artifact"
  exit "${fuzz_rc}"
fi

echo "### Fuzz smoke (${target}): CLEAN — no panic, no divergence" >> "${GITHUB_STEP_SUMMARY:-/dev/stdout}"
