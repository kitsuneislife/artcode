# RFC 0004 — IR / JIT / AOT Architecture

Status: Draft

Proponente: (preencher; proponho: eng-runtime + eng-compiler)

## Resumo curto
Propor uma arquitetura IR intermediária e um pipeline JIT/AOT com suporte a Profile-Guided Optimization (PGO). Começaremos com uma IR textual simples, infra para lowering do AST -> IR, um JIT experimental (LLVM/inkwell) para trechos quentes e um caminho AOT opt-in que aceita perfis para otimização. O objetivo é prover uma estrada de migração da VM interpretada atual para um compilador híbrido, mantendo o runtime interpretado como fallback.

## Motivação e objetivos
- Reduzir tempo de execução de hotspots críticos aumentando performance via JIT/AOT.
- Fornecer observabilidade (IR textual) e um caminho de testes/golden files para validar lowering.
- Integrar PGO para melhorar performance AOT em cenários reais.

Objetivos mensuráveis:
- Implementar IR textual e harness de lowering (AST->IR) com >= 80% cobertura de constructs usados em examples/cli em 4 semanas.
- JIT que acelera microbenchmarks selecionados em >= 2x vs interpretador para funções numéricas simples.

Não-goals (inicial): suporte completo a FFI, GC sofisticado embutido no JIT, ou PGO multi-processo na primeira entrega.

## Requisitos
- IR textual legível e round-trippable para inspeção humana.
- Semântica suficiente para otimizações locais (const-fold, algebraic simpl, common subexpr).
- Infra de testes: golden-files, unit tests, e microbench harness.

## Design da IR
Visão geral:
- IR em SSA por função. Representação textual inspirada em s-expressions/LLVM-IR minimal.
- Conceitos primários: Module, Function, BasicBlock, Instr, Operands, Types.

Tipos básicos iniciais:
- i64, f64, bool, ptr (opaque), void

Instruções (subset inicial):
- const <type> <value>
- add/sub/mul/div <type> %a %b
- fadd/fsub/fmul/fdiv <type> %a %b
- load <type> %ptr
- store <type> %value, %ptr
- call %fn %args...
- br <label> (uncond) | br_cond %pred %if_true %if_false
- phi <type> %v = phi [ %v1, %bb1 ], [ %v2, %bb2 ]
- ret <val?>
- intrinsic ops: gc_alloc %bytes, gc_write_barrier %ptr %val, arena_alloc %arena_id %bytes

Exemplo textual (função simples):

func @add(i64 %a, i64 %b) -> i64 {
	entry:
		%0 = add i64 %a, %b
		ret %0
}

Observações de design:
- Usar SSA simplificado (uma renaming pass durante lowering). Não é necessário um IR SSA completo desde o início; podemos gerar temporários e rodar uma simples phi-insertion quando necessário.
- Incluir intrinsics para GC/arena para permitir que backend JIT respeite contratos de alocação/finalização.

## Lowering (AST -> IR)
Abordagem incremental:
1. Implementar um pass que copia expressões puras (aritméticas) para IR direto e cria funções para `Stmt::Function`.
2. Mapear control flow (if/match/loops) para basic blocks com `br`/`phi` onde aplicável.
3. Inserir intrinsics para heap allocation quando `HeapComposite` for criado.

Testes: cada regra de lowering terá um golden-file com IR textual esperado; fixtures em `crates/core/tests/ir_lowering/`.

## JIT (projeto inicial)
Decisões chave:
- Biblioteca: `inkwell` (bindings LLVM) para aproveitar ORC/MCJIT; alternativa futura: Cranelift (menor footprint).
- Modelo: compile-on-demand funções individuais; manter um fallback para interpretador.
- Heurística de hotness: contadores de chamadas no runtime; threshold configurável para compilar com JIT.

Runtime integration:
- O interpretador mantém contadores de chamada por função name/id.
- Quando threshold excedido, solicitar ao JIT a compilação da função (lowering -> IR -> LLVM module -> native pointer), então patch callsites (ou usar indirect call via function pointer table) para apontar para código nativo.

Reversão / fallback:
- Gerar stubs que chamam o interpretador se verificação falhar (tipo, sandboxing) — evita crashes em early prototypes.

## AOT + PGO flow
1. Instrumentar compilador (JIT/AOT) para emitir contadores de bordo/calls quando `--gen-profile`.
2. Executar workload representativo para gerar `profile.dat`.
3. Usar perfil como entrada para o backend LLVM durante o passo AOT para guiar inlining, code layout e outras otimizações.
4. Emissão de objeto/bitcode e linkagem via system linker.

## Tooling & crates
- `crates/ir` — estruturas de IR, textual emitter/parser, utilities (dominators, simple transforms).
- `crates/irgen` (xtask style) — tools to generate golden files, run lowering harness.
- `crates/jit` (optional) — glue for inkwell-based JIT and runtime integration.
- Alterar `core` para expor lowering hooks ou mover lowering para `crates/ir` com um módulo `lowering` que depende de `core` AST.

Dependências propostas:
- `inkwell` (LLVM bindings) behind a feature flag `jit`.

## Testes e validação
- Unit tests: lowering rules and IR invariants.
- Golden tests: for small functions and examples in `cli/examples/`.
- Microbenchmarks: a small harness comparing interpreter vs JIT for selected kernels (array sum, factorial, fib).
- Integration: `art run --gen-profile` exercise and end-to-end AOT build.

CI changes
- Add a matrix entry for a job with LLVM installed (optional) to run JIT smoke tests; default CI continues to run interpreter-based tests.

## Rollout e cronograma (sujeito a revisão)
- Week 0: RFC review & owner assignment.
- Week 1–2: IR textual format, `crates/ir`, basic lowering pass + golden tests.
- Week 3–4: prototype JIT for pure numeric functions using `inkwell` (feature `jit`).
- Week 5–6: profiling instrumentation + `--gen-profile` harness; run microbench.
- Week 7+: AOT/PGO experiments and compare performance.

Critérios de aceitação por fase:
- IR/golden: lowering pipeline passes all golden tests.
- JIT prototype: at least one microbenchmark shows measurable speedup vs interpreter.
- PGO: AOT build with profile improves perf on benchmark suite.

## Alternativas e motivos
- Cranelift: mais leve, compila rápido; limitações em PGO/integration com LLVM-based toolchains.
- Bytecode VM: simples e controlável, mas dificulta path to AOT with LLVM; escolhida não como primeira opção.

## Riscos & mitigação
- Dependência LLVM é heavy: mitigação — feature-gated `jit` and CI job opt-in.
- Correctness regressions (semântica): mitigação — interpreter remains canonical; JIT only used after passing golden/validation tests.
- Build complexity for contributors without LLVM: provide docker image or GitHub Actions runner with LLVM.

## Segurança & sandboxing
- Inicialmente JIT roda code native in-process; to reduce risk, use conservative validation and fallback to interpreter on mismatch. Long term, consider an out-of-process JIT runner.

## Backwards compatibility
- Interpreter remains the default execution mode. JIT/AOT are opt-in features behind flags.

## Plano de comunicação
- Post RFC in repo and open issue for discussion.
- Announce on project channels and schedule a short design review meeting.

## Referências
- LLVM ORC/MCJIT documentation
- inkwell examples and tutorials

---

Próximos passos práticos (tarefa inicial sugerida):
1. Criar `crates/ir` com emitter/parser textual e testes de golden-files (Week 1 task).
2. Implementar lowering minimal no `core` ou `crates/ir::lowering` para expressions aritméticas e funções.
3. Adicionar um xtask para gerar e verificar golden files automaticamente.

Responsáveis sugeridos: eng-runtime (owner), eng-compiler (implementação lowering), eng-ci (CI changes).

Comentários e sugestões são bem-vindas; esta RFC é um ponto de partida para discussões técnicas e refinamento.
