#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="$ROOT/target/debug/art"
if [ ! -x "$BIN" ]; then
  echo "[build] compilando binário art..." >&2
  cargo build -q --bin art
fi
FAILED=0
OUT_BASE="$ROOT/cli/examples/_outputs"
STDOUT_DIR="$OUT_BASE/stdout"; STDERR_DIR="$OUT_BASE/stderr"
mkdir -p "$STDOUT_DIR" "$STDERR_DIR"
for f in $(ls -1 "$ROOT/cli/examples"/[0-9][0-9]_*.art | sort); do
  name=$(basename "$f")
  echo "[run] $name"
  if ! "$BIN" run "$f" >"$STDOUT_DIR/$name.out" 2>"$STDERR_DIR/$name.err"; then
    echo "[error] execução falhou: $f" >&2
    FAILED=1
  fi
  # Regras simples de validação: garantir que não houve panic ou unwrap
  if grep -qi "panic" "$STDERR_DIR/$name.err"; then
    echo "[fail] panic detectado em $f" >&2
    FAILED=1
  fi
  if grep -qi "thread '" "$STDERR_DIR/$name.err"; then
    echo "[fail] thread crash em $f" >&2
    FAILED=1
  fi
  echo "--- stdout ($name) ---"; sed 's/^/    /' "$STDOUT_DIR/$name.out" || true
  echo "--- stderr ($name) ---"; sed 's/^/    /' "$STDERR_DIR/$name.err" || true
  echo
done
if [ $FAILED -ne 0 ]; then
  echo "Alguns exemplos falharam" >&2
  exit 1
fi
echo "Todos os exemplos executados com sucesso"
