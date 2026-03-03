# Exemplos de FFI (Interoperabilidade Nativa)

A pasta `examples/ffi/` documenta como inicializar e interagir com o Artcode diretamente de linguagens nativas (como C e Rust) em contextos embarcados.

## 1. Exemplo em Linguagem C (`main.c`)

O exemplo em C demonstra como exportar primitivos (neste caso, `i64`) diretamente para o Heap do interpretador e resgatar o valor de volta.

Para compilar e testar manualmente via `gcc` e `cargo`:
```bash
# 1. Construir a biblioteca estática ou dinâmica do core
cd ../../crates/core
cargo build --release

# 2. Compilar o arquivo C chamando a biblioteca gerada
# CUIDADO: Este comando base precisará lincar a libcore gerada. 
# ex: gcc main.c -L../../target/release/ -lcore -o ffi_test
```

## 2. Ponto de Cuidado (Strings)
Quando exportar `String`s para o C através de `art_string_to_cstr`, não use `free()` nativo no ponteiro retornado. Em vez disso, prefira retornar à VM para livrar o cache estático construído no FFI:
```c
extern void art_free_cstr_cache();
// Chame art_free_cstr_cache() no encerramento da sua aplicação caso strings tenham sido passadas.
```
