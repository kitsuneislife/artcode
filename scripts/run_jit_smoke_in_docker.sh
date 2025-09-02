#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
docker build -t artcode-llvm-ci -f $ROOT/ci/docker/llvm/Dockerfile $ROOT
docker run --rm -v $ROOT:/work -w /work artcode-llvm-ci bash -lc "cargo test -p jit --features=jit"
