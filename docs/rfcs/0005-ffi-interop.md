# RFC 0005 — FFI Interoperability (Draft/WIP)

Status: Draft

Proponente: eng-runtime
Owner: eng-runtime

## Resumo curto
Este documento estabelece as diretrizes para a integração de código da linguagem Artcode com bibliotecas nativas C e outras bibliotecas Rust. O objetivo primário nesta fase é definir as convenções da Interface de Função Estrangeira (FFI) relativas à governança de memória, a ABI adotada e as convenções de mapeamentos de tipos básicos.

## Motivação
Possibilitar que códigos Artcode consigam aproveitar ecosistemas já existentes (como wrappers de janela C/C++, manipulação matemática rápida etc.), servindo de base preliminar para uma futura interoperabilidade com WebAssembly (WASM).

## Diretrizes e Princípios

- **Zero abstrações mágicas**: A fronteira entre o Artcode e a função nativa deve explicitar qualquer conversão de tipo necessária. Não haverão coerções automáticas escondidas da máquina virtual.
- **Ownership explícito**: Ficará determinado nos mapeamentos da FFI quem deverá alocar e quem deverá limpar (free) a memória transferida.
- **Tipos C-Compatíveis e Simples**: Estruturas de dados básicas com layout estável (ex.: `repr(C)`).
- **Tratamento Seguro de Erros**: Nunca gerar um *panic* de Rust inter-fronteiras FFI. Retornar erros através de códigos (ex.: `i32`) ou usando enums `Result`.

## Níveis Lógicos de Operação
1. **Ponte Base (C ABI)**: Funções marcadas como `extern "C"` e sem "mangling" de nomes `#[no_mangle]` para interoperar com bibliotecas puramente C ou chamadas do tipo AOT/LLVM em estágios futuros.
2. **Camadas em Rust**: Binding usando o runtime exposto para projetos Rust.
3. *(Futuro) WASM*: Adaptação sandbox das primitivas para execução web.

## Convenções de Mapeamento de Tipos Iniciais

| Categoria Artcode | Tipo C Sugerido / Representação Física | Notas |
| ----------------- | -------------------------------------- | ----- |
| `i64`             | `int64_t` (`i64`)                      | Direto |
| `f64`             | `double` (`f64`)                       | Direto |
| `bool`            | `uint8_t` (`u8`) ou `bool`             | Direto (0 = False, 1 = True) |
| `String`          | Ponteiros `*mut c_char` ou Fatias      | É necessário converter as `Arc<str>` do runtime de e para a FFI em null-terminated ou fat pointers. |
| Object references | `*mut c_void` ou Type-opaque pointers  | Os ponteiros não expõem layout de AST ou runtime internals. |

## Modelo de Memória do Artcode na Fronteira

Como a linguagem adota gerenciamento automático via contagem de referência (ARC), as pontes FFI devem controlar o incremento e o decremento explicitamente quando reterem variáveis ou objetos durante um call que atravessa a biblioteca nativa, bem como na FFI inversa:

```rust
// Exemplo canônico ideal a ser exposto no Core
#[no_mangle]
pub extern "C" fn art_value_retain(ptr: *mut c_void) { ... }

#[no_mangle]
pub extern "C" fn art_value_release(ptr: *mut c_void) { ... }
```

### Padrões Previstos (Ownership Crossing)
- **Borrowing (Empréstimo Cego)**: A função C somente lerá a informação ou vetor. A posse permanece do Artcode (`callee` não precisa acionar `retain`).
- **Transferência**: O Artcode entrega uma Array recentemente construído para o lado de C e *dropa* a referência local; do lado do C (e usando nossos free/release), torna-se o **Proprietário Único**.
- **Compartilhamento (Clone RC)**: O ambiente C necessita reter e guardar o valor na Heap. As APIs chamarão explicitamente `art_value_retain`.

## Pontos de Risco (Safety Checks e Mitigações)
1. **Ponteiros Nulos**: Deve-se conferir que valores e ponteiros vindos de C não são nulos.
2. **Buffer Overflow**: Qualquer tipo de buffer alocado não gerenciado por ARC (ex.: strings importadas provisórias) devem possuir verificação explícita de tamanho antes da construção da `Arc<str>` do sistema do Artcode.
3. Todas as pontes de conversões da API estarão em blocos `unsafe`.

## Próximos Passos
- Implementar o protótipo inicial como um módulo de features em `crates/core/src/ffi.rs`.
- Prover um exemplo integrando uma chamada ou extração FFI em `examples/ffi`.
