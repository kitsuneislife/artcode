#!/usr/bin/env sh
set -eu

# Runs the parser/loops cargo-fuzz worker with a bounded execution window.
# Usage:
#   bash scripts/run_fuzz_ci.sh [seconds]
# Example:
#   bash scripts/run_fuzz_ci.sh 60

MAX_TOTAL_TIME="${1:-60}"

if ! command -v cargo-fuzz >/dev/null 2>&1; then
  echo "cargo-fuzz not found. Installing..."
  cargo install cargo-fuzz --locked
fi

cd fuzzing
cargo fuzz run parser_loops -- -max_total_time="$MAX_TOTAL_TIME"
