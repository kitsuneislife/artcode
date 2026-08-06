#!/usr/bin/env bash
# Performance regression detector.
# Runs the fib(20) benchmark, checks that:
#   1. executed_statements matches the expected count (correctness)
#   2. executed_statements/s >= baseline min_stmts_per_sec (performance)
#
# Usage: scripts/perf_regression.sh [art-binary]
# Exit 0 on pass, 1 on failure.

set -euo pipefail

ART=${1:-target/debug/art}
BASELINE="baseline/perf_fib20.json"
SCRIPT="baseline/bench_fib20.art"
# Shared CI runners are noisy: a single sample can be several times slower than
# the machine's real throughput. Take the best of N runs so an unlucky sample
# cannot fail the build on its own.
RUNS=${PERF_RUNS:-5}

if [ ! -f "$ART" ]; then
  echo "ERROR: art binary not found at $ART" >&2
  exit 1
fi
if [ ! -f "$BASELINE" ]; then
  echo "ERROR: baseline file not found at $BASELINE" >&2
  exit 1
fi
if [ ! -f "$SCRIPT" ]; then
  echo "ERROR: benchmark script not found at $SCRIPT" >&2
  exit 1
fi

# Reads one integer field from a flat JSON object without needing python3/jq,
# so the check has no dependency beyond coreutils and the shell.
json_int() {
  local file=$1 key=$2 value
  value=$(tr -d ' \n\r' < "$file" | grep -o "\"$key\":[0-9]*" | head -1 | cut -d: -f2)
  if [ -z "$value" ]; then
    echo "ERROR: key '$key' not found in $file" >&2
    echo "--- file contents ---" >&2
    cat "$file" >&2
    exit 1
  fi
  printf '%s' "$value"
}

EXPECTED_STMTS=$(json_int "$BASELINE" expected_stmts)
MIN_RATE=$(json_int "$BASELINE" min_stmts_per_sec)

echo "Benchmark: fib(20)"
echo "Expected statements: $EXPECTED_STMTS"
echo "Min stmts/s threshold: $MIN_RATE"
echo "Runs (best-of): $RUNS"
echo ""

TMPDIR_RUN=$(mktemp -d)
trap 'rm -rf "$TMPDIR_RUN"' EXIT
TMPJSON="$TMPDIR_RUN/metrics.json"
TMPERR="$TMPDIR_RUN/stderr.txt"

run_once() {
  # stderr is kept rather than discarded so a crash is reported instead of
  # surfacing later as an unhelpful "key not found" parse failure.
  if ! "$ART" metrics --json "$SCRIPT" 2>"$TMPERR" > "$TMPDIR_RUN/raw.txt"; then
    echo "FAIL: '$ART metrics --json $SCRIPT' exited non-zero" >&2
    echo "--- stderr ---" >&2
    cat "$TMPERR" >&2
    exit 1
  fi
  tail -1 "$TMPDIR_RUN/raw.txt" > "$TMPJSON"
}

# Warm-up: the first run pays page-cache and dynamic-loader costs that are not
# part of what we want to measure.
run_once

BEST_RATE=0
STMTS=0
for _ in $(seq "$RUNS"); do
  START_NS=$(date +%s%N)
  run_once
  END_NS=$(date +%s%N)

  STMTS=$(json_int "$TMPJSON" executed_statements)
  RATE=$(awk -v s="$STMTS" -v a="$START_NS" -v b="$END_NS" \
    'BEGIN { t=(b-a)/1e9; if (t <= 0) t=1e-9; printf("%d", s/t) }')
  if [ "$RATE" -gt "$BEST_RATE" ]; then
    BEST_RATE=$RATE
  fi
done

echo "Results:"
echo "  executed_statements : $STMTS"
echo "  best stmts/s        : $BEST_RATE"
echo ""

FAIL=0

# Check 1: statement count must match exactly (proves correctness)
if [ "$STMTS" -ne "$EXPECTED_STMTS" ]; then
  echo "FAIL: executed_statements=$STMTS, expected=$EXPECTED_STMTS (function semantics changed)" >&2
  echo "      If this change is intentional, update expected_stmts in $BASELINE." >&2
  FAIL=1
fi

# Check 2: stmts/s must meet the minimum floor
if [ "$BEST_RATE" -lt "$MIN_RATE" ]; then
  echo "FAIL: stmts/s=$BEST_RATE < threshold=$MIN_RATE (performance regression detected)" >&2
  FAIL=1
fi

if [ "$FAIL" -eq 0 ]; then
  echo "PASS: performance within acceptable range"
fi

exit $FAIL
