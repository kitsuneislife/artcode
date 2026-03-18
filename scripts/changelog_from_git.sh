#!/usr/bin/env sh
set -eu

# Gera um rascunho de changelog a partir de mensagens de commit.
# Convencao esperada (quando possivel):
# feat:, fix:, docs:, refactor:, ci:, test:, chore:

range="${1:-}"
if [ -n "$range" ]; then
  log_cmd="git log --no-merges --pretty=format:%s $range"
else
  log_cmd="git log --no-merges --pretty=format:%s"
fi

collect() {
  prefix="$1"
  title="$2"
  echo "### $title"
  # shellcheck disable=SC2086
  sh -c "$log_cmd" | grep -E "^$prefix" | sed "s/^$prefix[[:space:]]*//" | sed 's/^/- /' || true
  echo
}

echo "## [Unreleased]"
echo
collect "feat:" "Added"
collect "fix:" "Fixed"
collect "refactor:" "Changed"
collect "docs:" "Docs"
collect "ci:" "CI"
collect "test:" "Tests"
collect "chore:" "Chore"
