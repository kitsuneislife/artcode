#!/usr/bin/env bash
set -euo pipefail

# Usage: scripts/run_metrics.sh <script> <out.json>
SCRIPT=${1:-cli/examples/99_weak_unowned_demo.art}
OUT=${2:-artifacts/metrics.json}
if [ ! -f "$SCRIPT" ]; then
  echo "Error: script file not found: $SCRIPT"
  echo "Usage: $0 [script] [out.json]  (defaults to cli/examples/99_weak_unowned_demo.art)"
  exit 2
fi
mkdir -p $(dirname "$OUT")

# Run CLI with --json and write output to OUT (workspace binary location)
target/debug/art metrics --json "$SCRIPT" > "$OUT"
echo "Wrote metrics to $OUT"
