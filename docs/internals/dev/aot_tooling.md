# Ferramental AOT

Este documento cobre as ferramentas internas que operam sobre IR textual e planos
de compilação. Para o caminho de usuário — compilar um `.art` para binário nativo —
veja [`docs/guides/aot_llvm.md`](../../guides/aot_llvm.md).

Todas vivem no crate `ir`. Não existe JIT: o único caminho nativo é AOT, emitindo
LLVM IR textual e compilando com `clang`.

## Pré-requisitos

- Toolchain Rust estável.
- `clang` no `PATH`, apenas para compilar de fato o IR emitido.

Nenhuma delas depende de `inkwell` nem de uma versão específica da C-API do LLVM.

## Inspecionar um perfil e um plano

`aot_inspect` cruza um perfil de execução com um plano de compilação e normaliza o
resultado em `aot_plan.normalized.json`:

```bash
cargo run -p ir --bin aot_inspect -- profile.json aot_plan.json [caminho/para/ir_dir]
```

Passando um diretório de IR, ele calcula uma estimativa de custo por função a partir
da análise textual (`ir::analyzer`): contagem de instruções, blocos, chamadas e
alocações, cada uma com um peso.

## Simular o consumidor

`aot_consumer` lê o plano normalizado e imprime a ordem de compilação que um
agendador seguiria:

```bash
cargo run -p ir --bin aot_consumer -- aot_plan.normalized.json
```

## Calibrar os pesos do analisador

`calibrate` ajusta os pesos de `ir::analyzer` contra medições reais:

```bash
cargo run -p ir --bin calibrate -- profile.json [caminho/para/ir_dir]
```

## Via xtask

O wrapper escolhe arquivos padrão quando não informados:

```bash
cargo run -p xtask -- aot-inspect --profile profile.json --plan aot_plan.json --ir-dir ci/ir
```

## Módulos relacionados

| Módulo | Responsabilidade |
|---|---|
| `ir::analyzer` | Métrica de custo a partir de IR textual |
| `ir::loader` | Parser de IR textual para `ir::Function` |
| `ir::cache` | Cache por conteúdo (FNV-1a) de artefatos de compilação |
| `ir::trampolines` | Contrato de chamada nativa e protocolo de deopt |
| `ir::parse_ir_signature` | Extrai aridade e tipo de retorno de uma assinatura |

`ir::trampolines` não tem chamador hoje: ele descreve a ABI (`extern "C" fn(*mut i64, ...) -> i64`,
status 0 para sucesso e 1 para bailout) que qualquer execução de código gerado
precisará respeitar.
