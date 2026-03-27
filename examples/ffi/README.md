# Exemplos de FFI (Interoperabilidade Nativa)

A pasta `examples/ffi/` documenta como inicializar e interagir com o Artcode diretamente de linguagens nativas (como C e Rust) em contextos embarcados.

## 1. Exemplo em Linguagem C (`main.c`)

O exemplo em C demonstra como usar o call-gate seguro via **handles opacos** (`u64`) para compartilhar valores com o runtime sem double-free.

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
Quando exportar `String`s para o C através de `art_handle_string_to_cstr`, não use `free()` nativo no ponteiro retornado. Em vez disso, prefira retornar à VM para livrar o cache estático construído no FFI:
```c
extern void art_free_cstr_cache();
// Chame art_free_cstr_cache() no encerramento da sua aplicação caso strings tenham sido passadas.
```

## 3. Syscalls unsafe por registradores

Para wrappers de baixo nivel (Ring 3), existe o gateway:

```c
extern int64_t art_syscall_unsafe(
	int64_t num,
	uintptr_t a0,
	uintptr_t a1,
	uintptr_t a2,
	uintptr_t a3,
	uintptr_t a4,
	uintptr_t a5,
	int64_t* out_errno
);
```

Use somente em bibliotecas de infraestrutura com invariantes bem documentadas.
