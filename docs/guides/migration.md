# Guia de Migracao (v0.1.x -> v0.2.x)

Este guia cobre migracoes de sintaxe e APIs publicas quando houver breaking changes.

## Fluxo Recomendado
1. Executar checagem automatica:

```bash
art upgrade --from 0.1.x --to 0.2.x --check caminho/do/script.art
```

2. Aplicar sugestoes de renome e ajustes semanticos.
3. Reexecutar testes e linter.
4. Validar docs e exemplos relacionados.

## Mapeamentos de Compatibilidade Conhecidos
- `__weak(...)` -> `weak(...)`
- `__weak_get(...)` -> `weak_get(...)`
- `__unowned(...)` -> `unowned(...)`
- `__unowned_get(...)` -> `unowned_get(...)`
- `__on_finalize(...)` -> `on_finalize(...)`

## Politica de Breaking Changes
- Toda quebra planejada exige RFC aprovada.
- Mudancas arquiteturais relevantes devem gerar ADR.
- O changelog deve conter orientacoes de migracao por release.

Consulte tambem:
- `docs/versioning.md`
- `CHANGELOG.md`
