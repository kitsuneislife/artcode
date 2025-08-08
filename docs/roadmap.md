# Roadmap

## Curto Prazo (0-2 meses)
- Diagnósticos detalhados (erro de parsing com linha/coluna)
- Erros estruturados em f-strings (sem panic)
- REPL melhora: exibir valor da última expressão
- Suporte a `count` e `sum` genérico com verificação de tipos

## Médio Prazo (2-6 meses)
- Sistema de tipos gradual (anotações opcionais com inferência local)
- Generics reais (monomorfização inicial) para Result, coleções
- Métodos em structs/enums (impl blocks)
- Guards em pattern matching
- Formatação em f-strings (`{expr:fmt}`)

## Longo Prazo (6-12 meses)
- JIT baseline + otimizações PGO (coleta de perfil no interpretador)
- Debugger time-travel (record/replay) protótipo
- FFI Rust/C/WASM com fronteiras de ownership explícitas
- Ferramenta de detecção de ciclos ARC offline

## Visão Estendida
- Atores e blocos `performant` com análise estática de acesso
- IR intermediário para otimizações SSA
- Perfil incremental armazenado em arquivo para builds AOT

## Métricas de Sucesso
| Métrica | Alvo |
|---------|------|
| Tempo de bootstrap | < 50ms interpretador simples |
| Cobertura de testes | > 70% núcleo (interpretador + parser) |
| Crash-free sessions | 99% (eliminar panics não tratados) |

## Riscos
| Risco | Mitigação |
|-------|-----------|
| Complexidade de generics | Implementar versão mínima iterativa |
| Crescimento de código sem docs | Enforcer CI para docs em PRs principais |
| Performance degradada | Benchmarks semanais básicos |

## Contribuição
Abrir RFC para features que alterem sintaxe, semântica ou runtime. Pequenos ajustes (refactors internos) podem ir direto com testes.
