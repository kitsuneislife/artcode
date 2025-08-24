"""
RFC 0002 — Sistema de Módulos & Pacotes (esqueleto / proposta inicial)

Status: Draft
Author: (timeboxed) proposta inicial gerada pelo time de produto
Date: 2025-08-23

Resumo
---
Esta RFC propõe um primeiro design prático para o sistema de módulos e pacotes do Artcode. O objetivo é entregar um MVP útil que permita organizar código em módulos locais, publicar/instalar pacotes locais (primeiro) e preparar a infraestrutura para resolver dependências locais/registries no futuro.

Motivação
---
- Facilitar reutilização e composição de código entre exemplos e projetos.
- Permitir builds reproducíveis e gestão de dependências mínima (Art.toml).
- Melhorar DX: imports explícitos, prelude bem definido e capacidade de empacotar libs.

Objetivos (MVP)
---
- Sintaxe de importação: `import foo.bar` + parsing e AST mínima.
- Resolução local: mapear import -> arquivo relativo/absolute no workspace (ex.: `foo/bar.art`).
- Manifesto `Art.toml`: metadados simples (name, version, dependencies).
- CLI básico: `art add <path-or-git>` que instala num cache local (`~/.artcode/cache`) e atualiza um lockfile simples.
- Documentação mínima `docs/modules.md` e exemplos em `cli/examples/modules/`.

Não objetivos (por agora)
---
- Registrar/host público de pacotes (registry) — pode ser adicionado depois.
- Resolução avançada de versões semânticas; no MVP suportar ref por caminho e ref git/tag simples.
- Sandboxing avançado/ACLs.

Design proposto (alto nível)
---
1. Sintaxe
   - `import foo.bar` resolve para um módulo `foo/bar.art` no workspace ou no cache.
   - Import relativo: `import ./util` ou `import ../pkg/helper`.

2. Manifesto `Art.toml`
   - Exemplo mínimo:
     ```toml
     name = "my-lib"
     version = "0.1.0"
     dependencies = { other = { path = "../other" } }
     ```

3. Resolução e cache
   - Resolver primeiro local (workspace), depois em cache `~/.artcode/cache/<name>-<version>`.
   - `art add <source>` copia/instala pacote no cache e registra no lockfile `.art-lock` no projeto.

4. Namespaces e prelude
   - Prelude mínimo definido pelo CLI/Interpreter (o que já existe hoje).
   - Imports não importam o prelude automaticamente (escopo explícito).

5. CLI
   - `art add <path|git>` — instala no cache e atualiza `.art-lock`.
   - `art build` / `art run` usa resolved modules via lockfile + cache.

Critérios de aceitação (MVP)
---
- Parser aceita `import` e gera AST válida.
- Exemplo de multi-file build: `examples/modules/demo` com 2 módulos e um `art run` que roda corretamente.
- `Art.toml` parse/validate e `art add` instala no cache.

Plano de milestones (curto prazo)
---
1. RFC finalizado + testes de aceitação (1 semana)
2. Parser + resolver local (2 semanas)
3. Manifesto e `art add` básico (2 semanas)
4. Docs e exemplos + CI smoke (1 semana)

Riscos
---
- Escopo grande: limitar o MVP a resolver arquivos locais e cache simples.
- Interação com o sistema de build futuro (JIT/AOT) requer alinhamento — documentar contratos entre componentes.

Notas finais
---
Esta RFC é um esqueleto. Implementação detalhada, API e testes devem ser preenchidos em PRs vinculados a esta RFC.
 
Implementação (status atual)
---
- Parser: `import` syntax implemented and AST node `Stmt::Import` added.
- Resolver: local resolution and cache-aware resolver implemented in `cli/src/resolver.rs`.
- CLI: `art add` implemented for local paths, `file://` and git URLs; writes `.art-lock` with name/version/path/commit when available.
- Manifesto: `Art.toml` parsing implemented via `toml` crate and supports `name`, `version` and optional `main` field.

Limitations e próximos passos
---
- Semver/constraints, registries and dependency graph resolution are out-of-scope for this MVP.
- `art add` uses system `git` CLI; consider `git2` if we want to avoid external dependency.
"""
