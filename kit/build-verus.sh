#!/usr/bin/env bash
# kit/build-verus.sh — source-build the PINNED Verus + Z3 the proof gates need.
#
# WHY SOURCE-BUILD: the pinned Verus release asset is NOT on crates.io, and the
# GitHub release-asset download + api.github.com are network-blocked in this
# environment (403). `git clone` traverses the working proxy fine, so we build
# Verus (verus-lang/verus @ 49b8806) and Z3 (Z3Prover/z3 @ z3-4.12.5) from
# source. crates.io, rustup component downloads, and static.rust-lang.org all
# work; only the release-asset zip is blocked.
#
# This transcribes the confirmed-working recipe verbatim. It produces:
#   verus  -> /root/verus-src/source/target-verus/release/verus
#   z3     -> /root/verus-src/source/z3   (4.12.5)
# and `verus --version` == 0.2026.07.05.49b8806.
#
# After it finishes:  ./kit/proof-gates.sh
#
# Pins:  Verus 0.2026.07.05.49b8806 (commit 49b8806) · Z3 4.12.5 · Rust 1.96.0
set -euo pipefail

VERUS_COMMIT="49b8806"          # release/0.2026.07.05.49b8806
Z3_TAG="z3-4.12.5"              # apt's 4.8.12 is too old — MUST be 4.12.5
RUST_TC="1.96.0-x86_64-unknown-linux-gnu"

echo "== 1/4  Clone + checkout Verus at the pinned commit ($VERUS_COMMIT)"
cd /root && rm -rf verus-src
git clone https://github.com/verus-lang/verus verus-src
cd verus-src && git checkout "$VERUS_COMMIT"

echo "== 2/4  Add the Rust components Verus needs (toolchain 1.96.0 is pinned by verus-src/rust-toolchain.toml)"
# verus-src/rust-toolchain.toml already pins channel=1.96.0, so no toolchain
# auto-download / 403 occurs; only these extra components are required.
rustup component add rustc-dev rustfmt llvm-tools --toolchain "$RUST_TC"

echo "== 3/4  Build Z3 $Z3_TAG from source (get-z3.sh downloads from GitHub releases = BLOCKED)"
cd /root && rm -rf z3-src
git clone https://github.com/Z3Prover/z3 z3-src
cd z3-src && git checkout "$Z3_TAG"
python3 scripts/mk_make.py
cd build && make -j"$(nproc)"          # binary lands at /root/z3-src/build/z3
cp /root/z3-src/build/z3 /root/verus-src/source/z3

echo "== 4/4  Build Verus (vargo build --release)"
cd /root/verus-src/source
# GOTCHA 1: this container globally exports CARGO_TARGET_DIR=/workspace/target,
# which breaks vargo's build orchestration (`which vargo` fails, builds land in
# the wrong place). It MUST be unset before any cargo/vargo step.
unset CARGO_TARGET_DIR
export VERUS_Z3_PATH=/root/verus-src/source/z3
# GOTCHA 2: do NOT pipe `source ../tools/activate` (e.g. `| tail`) — the pipe
# runs it in a subshell so its `export PATH` never reaches this shell. Source it
# bare, then belt-and-suspenders prepend vargo to PATH by hand.
source ../tools/activate
export PATH="/root/verus-src/tools/vargo/target/release:$PATH"
vargo build --release                  # builds cargo-verus + verus, verifies vstd
                                        # ends with vstd "1972 verified, 0 errors"

VERUS_BIN=/root/verus-src/source/target-verus/release/verus
echo
echo "== DONE"
echo "   verus binary : $VERUS_BIN"
echo "   z3 binary    : /root/verus-src/source/z3   (export VERUS_Z3_PATH to this)"
echo
echo "   Expected \`VERUS_Z3_PATH=$VERUS_Z3_PATH $VERUS_BIN --version\`:"
echo "     Verus"
echo "       Version: 0.2026.07.05.49b8806"
echo "       Profile: release"
echo "       Platform: linux_x86_64"
echo "       Toolchain: 1.96.0-x86_64-unknown-linux-gnu"
echo
echo "   Next: VERUS_BIN=$VERUS_BIN ./kit/proof-gates.sh"
