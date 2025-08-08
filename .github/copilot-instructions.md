# Instruções para Agentes Copilot no Projeto Artcode

Este documento orienta agentes de IA (Copilot) para serem produtivos e alinhados com a filosofia e arquitetura do projeto Artcode v2.0. Siga rigorosamente as diretrizes abaixo para garantir contribuições relevantes e sustentáveis.

## Visão Geral e Arquitetura
- O Artcode é uma linguagem de programação moderna, focada em "Complexidade Progressiva e Adaptativa": oferece sintaxe simples para iniciantes e ferramentas avançadas para especialistas.
- O projeto é dividido em múltiplos crates Rust: `core`, `interpreter`, `lexer`, `parser`, além do CLI. Cada crate tem responsabilidades claras e separadas.
- O compilador é híbrido JIT/AOT, com suporte a Profile-Guided Optimization (PGO). O fluxo de trabalho envolve rodar em modo JIT para coletar perfis e compilar AOT usando esses dados.
- A gestão de memória utiliza ARC (Automatic Reference Counting) com referências weak/unowned explícitas, inspirada no Swift. Ciclos são diagnosticados via ferramenta de teste, não por GC em produção.
- Concorrência segue modelo híbrido: padrão é Atores (seguro), mas blocos `performant` permitem memória compartilhada e primitivas como Mutex/Atomic, com análise de lifetime rigorosa.
- Debugging é "Time-Travel" na VM de desenvolvimento, baseado em record-and-replay, não reversível puro.
- Interoperabilidade (FFI) é estratégica: integração detalhada com C, Rust (custo zero, via LLVM), e WASM. Marque explicitamente transferências de posse de dados.
- Governança segue modelo de fundação independente e RFC aberto, visando sustentabilidade e comunidade.

## Convenções e Padrões Específicos
- Sempre empodere o desenvolvedor: forneça mecanismos explícitos, nunca abstrações mágicas ou opacas.
- Ao propor novas features, avalie o impacto na complexidade e mantenha o foco em clareza, controle e determinismo.
- Use a escada de abstração na memória: ARC simples por padrão, weak/unowned para ciclos, arenas e lifetimes para performance.
- Em concorrência, delimite claramente blocos `performant` e garanta que fora deles o código seja seguro por construção.
- Para debugging, integre com a VM JIT e priorize ferramentas que permitam inspeção determinística do estado passado.
- Na FFI, priorize integração com Rust e C, documentando claramente as fronteiras de memória e tipos.
- Siga o processo de RFC para mudanças estruturais e documente decisões de design.

## Fluxos de Trabalho
- Para builds: utilize `cargo run -- run <arquivo.art>` para executar exemplos; siga o fluxo de PGO para otimizações.
- Para testes: utilize ferramentas de diagnóstico de ciclos (`art test --detect-cycles`) e garanta que código de produção não dependa de GC.
- Para debugging: utilize `art debug` para sessões de time-travel.
- Para integração: documente e teste FFI, especialmente com Rust e WASM.

## Exemplos de Arquivos Importantes
- `crates/core/src/ast.rs`, `environment.rs`, `token.rs`: núcleo da linguagem.
- `crates/interpreter/src/interpreter.rs`, `type_registry.rs`, `values.rs`: execução e tipagem dinâmica.
- `crates/lexer/src/lexer.rs`, `keywords.rs`: análise léxica.
- `crates/parser/src/parser.rs`, `expressions.rs`, `statements.rs`: parsing e AST.
- `cli/examples/`: exemplos de código Artcode.

## Estratégia de Evolução
- Priorize soluções pragmáticas e comprovadas pela indústria.
- Documente claramente trade-offs e decisões de design.
- Mantenha o alinhamento com a filosofia de "Complexidade Progressiva".
- Garanta que toda contribuição seja auditável, sustentável e alinhada ao roadmap do projeto.

---

> Siga estas instruções à risca. Se encontrar ambiguidades ou lacunas, peça feedback ao mantenedor antes de propor mudanças estruturais.
