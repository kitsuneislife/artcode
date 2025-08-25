# RFC 0004 — IR / JIT / AOT Architecture (Draft)

Status: Draft

Proponente: (preencher)

## Contexto e Motivação
A fase 10 do roadmap visa introduzir um pipeline de compilação híbrido JIT/AOT com suporte a Profile-Guided Optimization (PGO). Para chegar lá precisamos definir uma IR intermediária, regras de lowering do AST e um plano de integração com LLVM (ou alternativa leve) que permita:

- Execução JIT de trechos quentes para acelerar iteração de desenvolvimento.
- Geração AOT otimizada com perfil PGO para builds de produção.
- Exportar um formato IR textual para inspeção e debugging.

## Proposta
1. Definir uma IR textual leve (s-expression inspired) com representação para: funções, basic blocks, operações aritméticas, chamadas, loads/stores, phi, e intrinsics para GC/arena abstractions.
2. Implementar lowering do AST -> IR no crate `core` (novo submodulo `ir`), com teste de golden-files para inspeção textual.
3. Implementar um pequeno back-end JIT usando `inkwell` (binding LLVM para Rust) inicialmente somente para funções puras numéricas e hot paths identificados via heurística simples (call frequency counters no runtime).
4. AOT: usar LLVM via `inkwell` para emitir objeto/bitcode e invocar `clang`/linker para produzir binários; integrar passo de PGO lendo profile.dat gerado por execução instrumentada.
5. Começar com suporte limitado: sem threads no JIT, sem FFI complexa; priorizar correctness e observability.

## Fases de rollout
- Fase 0: RFC, revisão, alinhamento com equipe.
- Fase 1: IR textual e test harness de lowering (AST->IR) + golden files.
- Fase 2: Implementar `inkwell`-based JIT para small numeric functions.
- Fase 3: Instrumentação de perfil + profile collection tool (`art run --gen-profile`).
- Fase 4: AOT using PGO and compare perf.

## Alternativas consideradas
- Usar cranelift ao invés de LLVM para JIT. Trade-off: cranelift é mais leve e gera código rápido, mas tem menos maturidade para PGO/optimizations complexas.
- Implementar um bytecode VM custom: mais simples e rápido de validar, porém limitaria a evolução para AOT com PGO.

## Impacto técnico
- New crates: `xtask/irgen` (tools), `crates/ir` (IR structures), plus `crates/jit` optional; modify `core` lowering pipeline.
- Dependências: `inkwell` (LLVM bindings). Adds CI matrix entry for systems with LLVM dev libs.

## Plano de testes
- Unit tests for lowering, golden files for IR text.
- Microbenchmarks for JIT vs interpreter for selected kernels.
- Integration tests for `--gen-profile` and AOT compilation roundtrip.

## Cronograma
(estimativas a discutir)

## Backwards compatibility
Initial steps non-invasive: interpreter remains default runtime. JIT/AOT opt-in.

## Referências
- LLVM ORC docs
- inkwell crate examples


---

Notes: this is a draft to start discussion. Next steps: assign owners, refine IR design, and prepare a prototype for AST->IR lowering tests.
