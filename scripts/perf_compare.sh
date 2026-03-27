#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

OUT_MD="${1:-perf.md}"
PGO_DATA_DIR="${TMPDIR:-/tmp}/artcode-pgo-data-$$"
WARMUP_BIN="$ROOT/target/release/art-warmup"
PGO_BIN="$ROOT/target/release/art-pgo"
WARMUP_CSV="${TMPDIR:-/tmp}/artcode-warmup-$$.csv"
PGO_CSV="${TMPDIR:-/tmp}/artcode-pgo-$$.csv"

cleanup() {
  rm -rf "$PGO_DATA_DIR" "$WARMUP_CSV" "$PGO_CSV"
}
trap cleanup EXIT

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required" >&2
  exit 1
fi

BENCHES=(benches/*.art)
if [ ! -e "${BENCHES[0]}" ]; then
  echo "No benchmark files found in benches/*.art" >&2
  exit 1
fi

timestamp() {
  date +"%Y-%m-%dT%H:%M:%S%z"
}

now_ms() {
  python3 - <<'PY'
import time
print(int(time.time() * 1000))
PY
}

measure_suite() {
  local bin="$1"
  local out_csv="$2"
  : > "$out_csv"
  for bench in "${BENCHES[@]}"; do
    local bench_name
    bench_name=$(basename "$bench")
    local start end elapsed
    start=$(now_ms)
    "$bin" run "$bench" >/dev/null
    end=$(now_ms)
    elapsed=$((end - start))
    echo "$bench_name,$elapsed" >> "$out_csv"
  done
}

echo "[1/6] Building warmup baseline binary..."
cargo build --release -p cli >/dev/null
cp target/release/art "$WARMUP_BIN"

echo "[2/6] Running warmup baseline benchmark suite..."
measure_suite "$WARMUP_BIN" "$WARMUP_CSV"

echo "[3/6] Building instrumented binary for PGO..."
rm -rf "$PGO_DATA_DIR"
mkdir -p "$PGO_DATA_DIR"
RUSTFLAGS="-Cprofile-generate=$PGO_DATA_DIR" cargo build --release -p cli >/dev/null

echo "[4/6] Collecting profile data with instrumented binary..."
for bench in "${BENCHES[@]}"; do
  target/release/art run "$bench" >/dev/null
 done

PROFDATA_CMD=$(find "$(rustc --print sysroot)" -name llvm-profdata -type f -perm -111 | head -n1 || true)
if [ -z "$PROFDATA_CMD" ]; then
  if command -v llvm-profdata >/dev/null 2>&1; then
    PROFDATA_CMD=$(command -v llvm-profdata)
  else
    echo "llvm-profdata not found. Install rust llvm tools (rustup component add llvm-tools-preview)." >&2
    exit 1
  fi
fi

"$PROFDATA_CMD" merge -o "$PGO_DATA_DIR/merged.profdata" "$PGO_DATA_DIR"

echo "[5/6] Building optimized PGO binary..."
RUSTFLAGS="-Cprofile-use=$PGO_DATA_DIR/merged.profdata" cargo build --release -p cli >/dev/null
cp target/release/art "$PGO_BIN"

echo "[6/6] Running PGO benchmark suite and generating markdown report..."
measure_suite "$PGO_BIN" "$PGO_CSV"

python3 - "$WARMUP_CSV" "$PGO_CSV" "$OUT_MD" <<'PY'
import csv
import sys
from datetime import datetime

warmup_csv, pgo_csv, out_md = sys.argv[1:4]

warm = {}
with open(warmup_csv, newline='') as f:
    for name, ms in csv.reader(f):
        warm[name] = int(ms)

pgo = {}
with open(pgo_csv, newline='') as f:
    for name, ms in csv.reader(f):
        pgo[name] = int(ms)

benches = sorted(set(warm) | set(pgo))
rows = []
wt = 0
pt = 0
for b in benches:
    w = warm.get(b, 0)
    p = pgo.get(b, 0)
    wt += w
    pt += p
    delta = ((p - w) / w * 100.0) if w else 0.0
    rows.append((b, w, p, delta))

total_delta = ((pt - wt) / wt * 100.0) if wt else 0.0

def emoji(delta):
    if delta < 0:
        return "faster"
    if delta > 0:
        return "slower"
    return "same"

lines = []
lines.append("# Performance Comparison (Warmup vs PGO)\n")
lines.append(f"Generated at: {datetime.now().isoformat()}\n")
lines.append("")
lines.append("| Benchmark | Warmup (ms) | PGO (ms) | Delta |")
lines.append("|---|---:|---:|---:|")
for b, w, p, d in rows:
    lines.append(f"| `{b}` | {w} | {p} | {d:+.2f}% ({emoji(d)}) |")
lines.append("")
lines.append("## Totals")
lines.append("")
lines.append(f"- Warmup total: **{wt} ms**")
lines.append(f"- PGO total: **{pt} ms**")
lines.append(f"- Delta: **{total_delta:+.2f}%** ({emoji(total_delta)})")
lines.append("")
lines.append("## Reproduce")
lines.append("")
lines.append("```bash")
lines.append("bash scripts/perf_compare.sh")
lines.append("```")

with open(out_md, 'w', encoding='utf-8') as f:
    f.write("\n".join(lines) + "\n")
PY

echo "Report written to $OUT_MD"
