# Changelog

Todas as mudancas relevantes deste projeto serao documentadas neste arquivo.

O formato segue Keep a Changelog e SemVer adaptado para a trilha 0.2.x.

## [Unreleased]

### Added
- Politica publica de versionamento em docs/versioning.md.
- Exemplo 29 sobre metadados de release e compatibilidade (examples/29_versioning_policy.art).

### Changed
- Documentacao de contribuicao e roadmap atualizada para referenciar versao e compatibilidade da serie 0.2.x.

### Docs
- Estrutura de governanca, RFC/ADR e versionamento sincronizada entre docs, README e website.

## [0.2.0] - 2026-03-18

### Added
- Loop statements nativos (while/for), tuplas e destructuring.
- Blocos explicitos de try/catch no parser e interpretador.
- Modo de execucao puro via run flag --pure.
- Builtin dag_topo_sort para ordenacao topologica de dependencias.
- Workflow de triagem automatica de issues com labels lang-design, runtime e tooling.
- Autodoc de stdlib via comando art doc std.
- Politica de versionamento publico com promessas de compatibilidade para 0.2.x.

### Changed
- GOVERNANCE.md formalizado com fluxo RFC -> ADR -> implementacao.
- CONTRIBUTING.md atualizado com processo RFC, ADR e triagem automatica.
- docs/decisions ganhou template ADR canonicamente referenciado.

### Fixed
- Ajustes de parser/lexer/runtime para loops, tuples e semantica de try/catch.
- Correcoes de compatibilidade do linter para mudancas recentes de AST.

### Docs
- Novos guias: loops_tuples, error_handling, pure_mode, dependency_dag e versioning.
- README, docs e website sincronizados com recursos entregues da trilha 0.2.

## Convencao de Atualizacao

- Atualizar [Unreleased] a cada PR mergeado.
- Em release, mover [Unreleased] para uma secao versionada datada.
- Classificar entradas em Added, Changed, Deprecated, Removed, Fixed e Docs.

## Geracao Semiautomatica

Use scripts/changelog_from_git.sh para obter um rascunho por categorias semanticas a partir do git log.
Revise manualmente antes de publicar release.
