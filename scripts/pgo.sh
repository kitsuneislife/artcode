#!/usr/bin/env bash
set -e

# PGO Automation Script for Artcode Interpreter
echo "=== Starting PGO (Profile-Guided Optimization) Build Process ==="

PGO_DATA_DIR="/tmp/artcode-pgo-data"
rm -rf "$PGO_DATA_DIR"
mkdir -p "$PGO_DATA_DIR"

echo "[1/4] Building instrumented binary..."
RUSTFLAGS="-Cprofile-generate=$PGO_DATA_DIR" cargo build -p cli --release

CLI_BIN="target/release/art"
if [ ! -f "$CLI_BIN" ]; then
    echo "Error: CLI binary not found at $CLI_BIN"
    exit 1
fi

echo "[2/4] Running benchmarks to gather profile data..."
for bench in benches/*.art; do
    echo "  Profiling $bench..."
    "$CLI_BIN" run "$bench" >/dev/null
done

echo "[3/4] Merging profile data using llvm-profdata..."
# Note: Ensure llvm-profdata is installed (rustup component add llvm-tools-preview)
# and in PATH, or leverage the rustup wrapper
PROFDATA_CMD=$(find $(rustc --print sysroot) -name llvm-profdata -executable | head -n1 || echo "llvm-profdata")

"$PROFDATA_CMD" merge -o "$PGO_DATA_DIR/merged.profdata" "$PGO_DATA_DIR"

echo "[4/4] Building PGO-optimized final binary..."
RUSTFLAGS="-Cprofile-use=$PGO_DATA_DIR/merged.profdata" cargo build -p cli --release

echo "=== PGO Build Complete ==="
echo "The highly optimized binary is ready at target/release/art"

# Optional: Output some benchmark tracking to see exactly how much PGO helped
echo "Running baseline check with newly optimized binary..."
./scripts/benchmark.sh
