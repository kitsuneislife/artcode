#!/usr/bin/env bash
set -uo pipefail
# Não usar 'set -e' para coletar todos os problemas e reportar no final

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

COLOR_GREEN="\e[32m"
COLOR_YELLOW="\e[33m"
COLOR_RED="\e[31m"
COLOR_RESET="\e[0m"

missing_critical=()
missing_optional=()
warnings=()

check_cmd() {
  if command -v "$1" >/dev/null 2>&1; then
    return 0
  else
    return 1
  fi
}

note() { printf "%b\n" "$1"; }
ok() { printf "%b[OK]%b %s\n" "$COLOR_GREEN" "$COLOR_RESET" "$1"; }
fail() { printf "%b[FAIL]%b %s\n" "$COLOR_RED" "$COLOR_RESET" "$1"; }
warn() { printf "%b[WARN]%b %s\n" "$COLOR_YELLOW" "$COLOR_RESET" "$1"; }

check_rust_toolchain() {
  if check_cmd rustc && check_cmd cargo; then
    ok "rustc: $(rustc --version 2>/dev/null | tr -d '\n')"
    ok "cargo: $(cargo --version 2>/dev/null | tr -d '\n')"
  else
    missing_critical+=("rust/toolchain (rustc/cargo)")
    fail "rustc or cargo not found. Instale via rustup: https://rustup.rs"
  fi
}

check_rust_components() {
  # Prefer rustup check, fallback to checking binaries
  if check_cmd rustup; then
    installed=$(rustup component list --installed 2>/dev/null || true)
    if printf "%s" "$installed" | grep -q "rustfmt"; then
      ok "rustfmt instalado"
    else
      missing_optional+=("rustfmt")
      warn "rustfmt não encontrado via rustup; 'cargo fmt' pode falhar"
    fi
    if printf "%s" "$installed" | grep -q "clippy"; then
      ok "clippy instalado"
    else
      missing_optional+=("clippy")
      warn "clippy não encontrado via rustup; 'cargo clippy' pode falhar"
    fi
  else
    # Fallback: procura binários
    if check_cmd rustfmt; then
      ok "rustfmt disponível"
    else
      missing_optional+=("rustfmt")
      warn "rustfmt não encontrado"
    fi
    # clippy não tem binário único; checar cargo-clippy pode falhar
  fi
}

check_c_compiler() {
  if check_cmd gcc || check_cmd clang; then
    if check_cmd gcc; then ok "gcc: $(gcc --version | head -n1)"; fi
    if check_cmd clang; then ok "clang: $(clang --version | head -n1)"; fi
  else
    missing_critical+=("C compiler (gcc or clang)")
    fail "Nenhum compilador C encontrado (gcc/clang). Instale build-essential / base-devel"
  fi
}

check_git_python() {
  if check_cmd git; then ok "git: $(git --version | tr -d '\n')"; else missing_critical+=("git"); fail "git não encontrado"; fi
  if check_cmd python3; then ok "python3: $(python3 --version 2>/dev/null | tr -d '\n')"; else missing_critical+=("python3"); fail "python3 não encontrado"; fi
}

check_optional_tools() {
  if check_cmd cargo-llvm-cov || (check_cmd cargo && cargo llvm-cov --version >/dev/null 2>&1); then
    ok "cargo-llvm-cov disponível"
  else
    missing_optional+=("cargo-llvm-cov (opcional for coverage)")
    warn "cargo-llvm-cov não encontrado — cobertura com llvm-cov não estará disponível"
  fi

  if check_cmd gh; then ok "gh (GitHub CLI) disponível"; else missing_optional+=("gh (GitHub CLI)"); warn "gh não encontrado — uploads/release via CLI não estarão disponíveis"; fi
  if check_cmd docker; then ok "docker disponível"; else missing_optional+=("docker"); warn "docker não encontrado — utile em CI locais/containers"; fi
}

check_workspace_size() {
  # espaço livre no filesystem que contém o workspace
  avail_mb=$(df --output=avail -m "$ROOT_DIR" | tail -n1 | tr -d ' ')
  ok "Espaço livre em $(df -h --output=target,pcent "$ROOT_DIR" | sed -n '2p' | awk '{print $1}') — ${avail_mb}MB disponível"
  min_mb=5000
  rec_mb=20000
  if [ "$avail_mb" -lt "$min_mb" ]; then
    missing_critical+=("Espaço em disco (< ${min_mb}MB)")
    fail "Espaço livre muito baixo: ${avail_mb}MB (mínimo recomendado: ${min_mb}MB)"
  elif [ "$avail_mb" -lt "$rec_mb" ]; then
    warnings+=("Espaço em disco abaixo do recomendado (${rec_mb}MB)")
    warn "Espaço livre menor que recomendado (${rec_mb}MB): ${avail_mb}MB"
  else
    ok "Espaço em disco adequado (${avail_mb}MB)"
  fi
}

check_memory_swap_cores() {
  # Memória total em MB
  if [ -r /proc/meminfo ]; then
    mem_kb=$(awk '/MemTotal:/ {print $2}' /proc/meminfo)
    mem_mb=$((mem_kb/1024))
    swap_kb=$(awk '/SwapTotal:/ {print $2}' /proc/meminfo || echo 0)
    swap_mb=$((swap_kb/1024))
  else
    mem_mb=0
    swap_mb=0
  fi
  cores=$(nproc --all 2>/dev/null || echo 1)

  ok "Memória total: ${mem_mb}MB"
  ok "Swap total: ${swap_mb}MB"
  ok "CPUs (hardware threads): ${cores}"

  min_mem=2048
  rec_mem=8192
  ideal_mem=32768
  if [ "$mem_mb" -lt "$min_mem" ]; then
    missing_critical+=("RAM (< ${min_mem}MB)")
    fail "Memória RAM insuficiente: ${mem_mb}MB (mínimo: ${min_mem}MB)"
  elif [ "$mem_mb" -lt "$rec_mem" ]; then
    warnings+=("RAM menor que recomendado (${rec_mem}MB)")
    warn "Memória RAM abaixo do recomendado (${rec_mem}MB): ${mem_mb}MB"
  else
    ok "Memória RAM adequada (${mem_mb}MB)"
  fi

  min_cores=2
  rec_cores=4
  if [ "$cores" -lt "$min_cores" ]; then
    missing_critical+=("CPU cores (< ${min_cores})")
    fail "Poucos núcleos CPU: ${cores} (mínimo: ${min_cores})"
  elif [ "$cores" -lt "$rec_cores" ]; then
    warnings+=("Cores abaixo do recomendado (${rec_cores})")
    warn "Cores abaixo do recomendado (${rec_cores}): ${cores}"
  else
    ok "Cores adequados: ${cores}"
  fi
}

check_llvm_clang() {
  if check_cmd clang; then
    ok "clang: $(clang --version | head -n1)"
  else
    warn "clang não encontrado — cobertura via llvm-cov pode não funcionar"
    missing_optional+=("clang/llvm")
  fi
}

check_build_binary() {
  # Verifica se o binário CLI existe (opcional)
  BIN="$ROOT_DIR/target/debug/art"
  if [ -x "$BIN" ]; then
    ok "Binário CLI encontrado: $BIN"
  else
    warn "Binário CLI não encontrado em $BIN — execute 'cargo build --bin art' para compilar"
    missing_optional+=("target/debug/art (binário não compilado)")
  fi
}

print_summary() {
  echo
  printf "%bRELAÇÃO FINAL:%b\n" "$COLOR_GREEN" "$COLOR_RESET"
  if [ ${#missing_critical[@]} -ne 0 ]; then
    printf "%bItens críticos faltando (%d):%b\n" "$COLOR_RED" "${#missing_critical[@]}" "$COLOR_RESET"
    for i in "${missing_critical[@]}"; do
      printf "  - %s\n" "$i"
    done
  else
    printf "  - Nenhum item crítico faltando\n"
  fi

  if [ ${#missing_optional[@]} -ne 0 ]; then
    printf "%bItens opcionais/rec. ausentes (%d):%b\n" "$COLOR_YELLOW" "${#missing_optional[@]}" "$COLOR_RESET"
    for i in "${missing_optional[@]}"; do
      printf "  - %s\n" "$i"
    done
  else
    printf "  - Nenhum item opcional ausente\n"
  fi

  if [ ${#warnings[@]} -ne 0 ]; then
    printf "%bAvisos (%d):%b\n" "$COLOR_YELLOW" "${#warnings[@]}" "$COLOR_RESET"
    for i in "${warnings[@]}"; do
      printf "  - %s\n" "$i"
    done
  fi

  if [ ${#missing_critical[@]} -ne 0 ]; then
    printf "%bSTATUS: Ambiente NÃO pronto (itens críticos faltando)%b\n" "$COLOR_RED" "$COLOR_RESET"
    exit 2
  else
    printf "%bSTATUS: Ambiente pronto para desenvolvimento básico.%b\n" "$COLOR_GREEN" "$COLOR_RESET"
    exit 0
  fi
}

main() {
  echo "==> Verificação do ambiente para o projeto Artcode"
  echo "Diretório do workspace: $ROOT_DIR"
  echo

  check_rust_toolchain
  check_rust_components
  check_c_compiler
  check_git_python
  check_optional_tools
  check_workspace_size
  check_memory_swap_cores
  check_llvm_clang
  check_build_binary

  print_summary
}

main "$@"
