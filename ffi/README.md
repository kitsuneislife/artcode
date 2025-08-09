# FFI (Foreign Function Interface) - Draft

Objetivo: estabelecer diretrizes para integração de código Art com bibliotecas C, Rust e futuramente WASM, respeitando filosofia de controle explícito de memória e ownership.

## Princípios
- Zero abstrações mágicas: toda fronteira deve declarar conversões.
- Ownership explícito: quem aloca, quem libera, e se a posse é transferida.
- Tipos estáveis: layout previsível para structs simples.
- Erros retornados via enum Result ou códigos explícitos.
- Sem panics cruzando a fronteira.

## Níveis de Integração
1. C ABI mínimo: funções exportadas com `extern "C"` (futuro backend AOT/LLVM).
2. Binding Rust direto: reuso interno sem cópia (zero-cost) expondo ponteiros ref contados.
3. WASM: sandbox para ambiente web (planejado após estabilização do core).

## Tipos Suportados (Roadmap)
| Categoria | Estado | Notas |
|----------|--------|-------|
| Inteiros / Float | Inicial | Mapear para `i64` / `f64` | 
| Bool | Inicial | `u8` / `bool` conforme alvo |
| String | Planejado | Passagem como fat pointer (ptr,len) | 
| Array | Planejado | Cabeçalho com len + ptr | 
| Struct Plain | Planejado | Layout repr(C) restrito |
| Enum (tagged) | Planejado | Tag + union simplificado |

## Memory Model
Usar ARC internamente; na fronteira FFI expor contadores explicitamente ou funções de retain/release:
```
art_value_retain(ptr)
art_value_release(ptr)
```
Ciclos não são coletados automaticamente; ferramentas de debug podem detectar.

## Ownership Patterns
| Padrão | Descrição | Exemplo |
|--------|-----------|---------|
| Borrow | Chamador mantém posse; callee não retém | `len` sobre slice |
| Transfer | Callee assume e chamador não usa mais | criação de array | 
| Clone RC | Incrementa contador para uso compartilhado | cache de string |

## Erros
- Funções podem retornar struct `{ code: i32, payload: *mut ArtValue }` ou usar enum Result quando chamado internamente.
- Sem unwind: camadas convertem para código retornado.

## Segurança
- Validar ponteiros não nulos.
- Tamanho máximo aceitável para buffers (limite configurável).
- Marcar funções `unsafe` quando invariantes do runtime forem exigidas.

## Próximos Passos
- Definir módulo `ffi` no crate core com tipos de ponte.
- Especificar representação binária de `ArtValue` mínima exportável.
- Prototipar função `art_len(value)` exportada.
- Documentar macro de ajuda para declarar builtins FFI.

## Estado
Draft inicial; sujeito a RFC antes de implementação completa.
