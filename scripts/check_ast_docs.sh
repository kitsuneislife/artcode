#!/usr/bin/env bash
set -euo pipefail
# Verifica se houve mudança em crates/core/src/ast.rs e se docs/overview.md foi alterado no mesmo commit (staged diff)
changed_ast=$(git diff --name-only --cached | grep -E '^crates/core/src/ast.rs' || true)
if [[ -n "$changed_ast" ]]; then
  changed_docs=$(git diff --name-only --cached | grep -E '^docs/(overview|functions|fstrings)\.md' || true)
  if [[ -z "$changed_docs" ]]; then
    echo "AST modificado sem atualizar docs (overview/functions/fstrings). Atualize a documentação." >&2
    exit 1
  fi
fi
exit 0
