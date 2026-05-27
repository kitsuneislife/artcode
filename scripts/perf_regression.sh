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

if [ ! -f "$ART" ]; then
  echo "ERROR: art binary not found at $ART" >&2
  exit 1
fi
if [ ! -f "$BASELINE" ]; then
  echo "ERROR: baseline file not found at $BASELINE" >&2
  exit 1
fi

EXPECTED_STMTS=$(python3 -c "import json; d=json.load(open('$BASELINE')); print(d['expected_stmts'])")
MIN_RATE=$(python3 -c "import json; d=json.load(open('$BASELINE')); print(d['min_stmts_per_sec'])")

echo "Benchmark: fib(20)"
echo "Expected statements: $EXPECTED_STMTS"
echo "Min stmts/s threshold: $MIN_RATE"
echo ""

# Capture metrics JSON (stdout is mixed with program output, separate with temp file)
TMPJSON=$(mktemp /tmp/artcode_perf_XXXXXX.json)
trap "rm -f $TMPJSON" EXIT

START_NS=$(date +%s%N)
"$ART" metrics --json "$SCRIPT" 2>/dev/null | tail -1 > "$TMPJSON"
END_NS=$(date +%s%N)

ELAPSED_S=$(awk -v s=$START_NS -v e=$END_NS 'BEGIN { printf("%.3f", (e-s)/1e9) }')
STMTS=$(python3 -c "import json; d=json.load(open('$TMPJSON')); print(d['executed_statements'])")
RATE=$(awk -v s=$STMTS -v t=$ELAPSED_S 'BEGIN { printf("%d", s/t) }')

echo "Results:"
echo "  executed_statements : $STMTS"
echo "  elapsed             : ${ELAPSED_S}s"
echo "  stmts/s             : $RATE"
echo ""

FAIL=0

# Check 1: statement count must match exactly (proves correctness)
if [ "$STMTS" -ne "$EXPECTED_STMTS" ]; then
  echo "FAIL: executed_statements=$STMTS, expected=$EXPECTED_STMTS (function semantics changed)" >&2
  FAIL=1
fi

# Check 2: stmts/s must meet the minimum floor
if [ "$RATE" -lt "$MIN_RATE" ]; then
  echo "FAIL: stmts/s=$RATE < threshold=$MIN_RATE (performance regression detected)" >&2
  FAIL=1
fi

if [ "$FAIL" -eq 0 ]; then
  echo "PASS: performance within acceptable range"
fi

exit $FAIL
