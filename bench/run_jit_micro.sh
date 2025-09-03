#!/usr/bin/env bash
# Minimal microbench harness to exercise a hot loop for JIT smoke runs
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

cd "$REPO"

echo "Building with JIT feature..."
cargo build --release --features=jit

echo "Running microbenchmark (release, jit)..."
RUST_BACKTRACE=1 target/release/art run --example micro_bench || true

echo "Done"
