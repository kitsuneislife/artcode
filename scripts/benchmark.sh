#!/usr/bin/env bash
set -e

# Setup history file
HISTORY_FILE=".kit/perf_history.csv"

if [ ! -f "$HISTORY_FILE" ]; then
    mkdir -p .kit
    echo "timestamp,commit,benchmark,duration_ms" > "$HISTORY_FILE"
fi

# Ensure bin is latest release
echo "Building CLI release..."
cargo build --release -p cli >/dev/null 2>&1

CLI_BIN="target/release/art"
if [ ! -f "$CLI_BIN" ]; then
    echo "Error: CLI binary not found at $CLI_BIN"
    exit 1
fi

COMMIT_HASH=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
TIMESTAMP=$(date +"%Y-%m-%dT%H:%M:%S")

# Extract the previous run's total time
PREV_TOTAL=$(awk -F',' 'NR>1 {sum[$2]+=$4} END {for (i in sum) print sum[i]}' "$HISTORY_FILE" | tail -n1 || str "0")
if [ -z "$PREV_TOTAL" ]; then
    PREV_TOTAL="0"
fi

total_current=0

echo "Running benchmarks..."
for bench in benches/*.art; do
    bench_name=$(basename "$bench")
    echo "  Running $bench_name..."
    
    start=$(python3 -c 'import time; print(int(time.time() * 1000))')
    "$CLI_BIN" run "$bench" >/dev/null
    end=$(python3 -c 'import time; print(int(time.time() * 1000))')
    
    duration=$((end - start))
    total_current=$((total_current + duration))
    
    echo "    Finished $bench_name: ${duration}ms"
    echo "$TIMESTAMP,$COMMIT_HASH,$bench_name,$duration" >> "$HISTORY_FILE"
done

echo "Total suite time: ${total_current}ms"

# Regression detector check (if previous total > 0)
if [ "$PREV_TOTAL" -gt 0 ]; then
    # Calculate difference
    threshold=$(( PREV_TOTAL * 110 / 100 )) # 10% tolerance 

    echo "Previous baseline: ${PREV_TOTAL}ms | Degradation threshold: ${threshold}ms"
    
    if [ "$total_current" -gt "$threshold" ]; then
        echo "❌ REGRESSION DETECTED! Current time (${total_current}ms) exceeds the 10% threshold of baseline (${PREV_TOTAL}ms)."
        exit 1
    else
        echo "✅ Performance within acceptable bounds."
    fi
else
    echo "✅ First baseline captured. No regression check performed."
fi
