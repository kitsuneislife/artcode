#!/usr/bin/env bash
set -euo pipefail

# Dev check: clippy, tests, grep panic occurrences in src

echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check || true

echo "==> cargo clippy --all -q"
cargo clippy --all --quiet || true

echo "==> cargo test -q"
cargo test -q

echo "==> grep for panic!/unwrap in src"
if grep -R --line-number -E "panic!|unwrap\(|expect\(" crates/ src | tee /dev/stderr | wc -l | grep -q "^0$"; then
  echo "No panics/unwraps found in code paths."
else
  echo "Found potential panics (see above)." >&2
fi

echo "Done."
