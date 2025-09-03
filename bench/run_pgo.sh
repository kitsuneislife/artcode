#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

PROFILE=profile.json
AOT_PLAN=aot_plan.json

echo "Running microkernel example to generate profile: cli/examples/20_microkernel.art"
# generate profile using CLI
cargo run -p cli -- run cli/examples/20_microkernel.art --gen-profile "$PROFILE"

if [ -f "$PROFILE" ]; then
  echo "Profile generated: $PROFILE"
else
  echo "Profile not generated"
  exit 1
fi

# Generate AOT plan from profile
echo "Generating AOT plan from profile -> $AOT_PLAN"
cargo run -p cli -- build -- --with-profile "$PROFILE" --out "$AOT_PLAN"

if [ -f "$AOT_PLAN" ]; then
  echo "AOT plan written to $AOT_PLAN"
else
  echo "Failed to write AOT plan"
  exit 1
fi

echo "Done"
