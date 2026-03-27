#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="$ROOT/target/debug/art"
if [ ! -x "$BIN" ]; then
  echo "[build] compilando binário art..." >&2
  cargo build -q --bin art
fi

TRACE="$ROOT/examples/44_ttd_keyframes.artlog"
EXAMPLE="$ROOT/examples/44_ttd_keyframes.art"

rm -f "$TRACE"

echo "[smoke] rodando record"
"$BIN" run --record "$TRACE" "$EXAMPLE"

if [ ! -f "$TRACE" ]; then
  echo "[erro] Trace não gerado: $TRACE" >&2
  exit 1
fi

# Re-executa em modo replay com fluxo de passos automáticos (Enter em branco)
# O script usa atalho para enviar linhas vazias continuamente.

echo "[smoke] rodando replay (debug)"
set +e
yes "" | head -n 30 | "$BIN" debug --replay "$TRACE" "$EXAMPLE" > "$ROOT/examples/_outputs/44_ttd_keyframes.replay.out" 2> "$ROOT/examples/_outputs/44_ttd_keyframes.replay.err"
RET=$?
set -e
if [ $RET -ne 0 ] && [ $RET -ne 141 ]; then
  echo "[erro] debug falhou com codigo $RET" >&2
  cat "$ROOT/examples/_outputs/44_ttd_keyframes.replay.err" >&2
  exit 1
fi

# Verifica que não há panic no stderr
if grep -qi "panic" "$ROOT/examples/_outputs/44_ttd_keyframes.replay.err"; then
  echo "[erro] panic detectado no replay" >&2
  cat "$ROOT/examples/_outputs/44_ttd_keyframes.replay.err" >&2
  exit 1
fi

echo "[smoke] OK: record/replay concluído sem crash"
