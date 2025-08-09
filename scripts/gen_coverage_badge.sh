#!/usr/bin/env bash
set -euo pipefail
LCOV_FILE=${1:-lcov.info}
OUT=${2:-target/coverage_badge.svg}
if [[ ! -f "$LCOV_FILE" ]]; then
  echo "lcov file not found: $LCOV_FILE" >&2
  exit 1
fi
# Extrai linhas cobertas e totais
LINES=$(grep -h '^DA:' "$LCOV_FILE" | awk -F',' '{print $2}' | awk -F':' '{print $2}' | awk '{covered+=$1; total+=1} END { if (total==0) {print "0 0"} else {print covered" "total}}')
COVERED=$(echo $LINES | awk '{print $1}')
TOTAL=$(echo $LINES | awk '{print $2}')
PCT=0
if [[ $TOTAL -gt 0 ]]; then
  PCT=$(awk -v c=$COVERED -v t=$TOTAL 'BEGIN { printf("%.1f", (c/t)*100) }')
fi
COLOR="#e05d44" # red
# thresholds
if awk "BEGIN {exit !($PCT >= 90)}"; then COLOR="#4c1"; elif awk "BEGIN {exit !($PCT >= 75)}"; then COLOR="#dfb317"; fi
mkdir -p "$(dirname "$OUT")"
cat > "$OUT" <<SVG
<svg xmlns="http://www.w3.org/2000/svg" width="150" height="20" role="img" aria-label="coverage: $PCT%">
  <linearGradient id="s" x2="0" y2="100%"><stop offset="0" stop-color="#bbb" stop-opacity=".1"/><stop offset="1" stop-opacity=".1"/></linearGradient>
  <rect rx="3" width="150" height="20" fill="#555"/>
  <rect rx="3" x="70" width="80" height="20" fill="$COLOR"/>
  <rect rx="3" width="150" height="20" fill="url(#s)"/>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="11">
    <text x="35" y="14">coverage</text>
    <text x="110" y="14">$PCT%</text>
  </g>
</svg>
SVG
echo "Generated badge ($PCT%) at $OUT" >&2
