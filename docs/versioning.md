# Politica de Versionamento Publico

Este documento define as promessas de compatibilidade do Artcode para releases publicas.
A politica vale para CLI, sintaxe da linguagem, comportamento de runtime e stdlib publicada.

## Esquema de Versao
O Artcode segue SemVer com o formato `MAJOR.MINOR.PATCH`.

- `MAJOR`: quebra de compatibilidade publica.
- `MINOR`: novos recursos compativeis com versoes anteriores da mesma major.
- `PATCH`: correcao de bug sem mudanca de contrato publico.

## Promessas de Compatibilidade
Para versoes `0.y.z`, a evolucao ainda e rapida, entao a compatibilidade e definida por trilhas:

- `0.2.x`:
  - Sem quebras intencionais na CLI em comandos existentes (`run`, `fmt`, `lint`, `metrics`, `doc`, `lsp`).
  - Sem remocao de sintaxe ja marcada como estavel na documentacao.
  - Builtins existentes nao devem mudar assinatura sem RFC e nota de migracao.
- Quebras planejadas dentro da serie `0.2.x` so podem ocorrer em casos criticos e devem seguir:
  - RFC aprovada com justificativa tecnica.
  - ADR registrada quando houver impacto arquitetural.
  - guia de migracao documentado antes do merge final.

## O que conta como Breaking Change
- Alterar comportamento de parser que invalide codigo valido anteriormente.
- Remover comando CLI, flag ou formato de saida consumido publicamente.
- Mudar contrato de builtin (nome, aridade, tipo de retorno) sem compat layer.
- Alterar semantica de runtime observavel (ex: pure mode, diagnostics) sem migracao.

## Fluxo Obrigatorio para Breaking Change
1. Abrir RFC em `docs/rfcs/` com impacto e plano de migracao.
2. Definir janela alvo de release no roadmap operacional.
3. Publicar nota de migracao em docs e changelog.
4. Atualizar exemplos e site antes de release.

## Politica de Deprecacao
- Marcar API/comportamento como "deprecated" por no minimo 1 MINOR antes de remover.
- Informar alternativa recomendada.
- Incluir aviso no changelog e, quando possivel, no linter/diagnostics.

## Matriz de Garantias
| Area | Garantia em `0.2.x` |
|------|----------------------|
| Sintaxe estavel | Nao quebra sem RFC + migracao |
| CLI publica | Nao remove comandos sem deprecacao |
| Stdlib builtin | Sem quebra de assinatura sem RFC |
| Formatos de docs | Evolutivos, com redirecionamento quando possivel |

## Release Checklist (Versionamento)
Antes de publicar:
- confirmar semver correto para escopo da mudanca.
- atualizar changelog com tipo de mudanca (breaking/feature/fix).
- atualizar docs, README, examples e website.
- validar testes e scripts de CI relevantes.

Para padronizacao continua do changelog, consulte `CHANGELOG.md` e o utilitario `scripts/changelog_from_git.sh`.
