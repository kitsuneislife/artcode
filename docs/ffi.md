# Integração FFI (Foreign Function Interface)

Este guia destina-se a desenvolvedores que necessitam interoperar código C (ou linguagens parceiras) com as bibliotecas nativas e a máquina virtual do Artcode. A fundação de FFI faz parte de um conjunto exposto via `crate::core::ffi`.

## Tipos Suportados na ABI C

A linguagem Artcode faz uso de `Value`, um type opaco que pode instanciar Strings, Inteiros (`i64`), Ponto Flutuante (`f64`), ou Listas localizadas nos ambientes da Heap do Interpretador. Na fronteira com a ABI C, os ponteiros nativos operam diretamente no tipo `C` de destino:

| Artcode | Tipo C Sugerido | Operação |
| ------- | --------------- | -------- |
| `i64`   | `int64_t`       | `art_extract_i64` / `art_create_i64` |
| `String`| `char*` nulo    | `art_string_to_cstr` / `art_free_cstr` |

### Exemplos na C-ABI
A interface do `core::ffi` atual disponibiliza ponteiros baseados em `extern "C"`:

```c
#include <stdint.h>
#include <stdio.h>

// Forward declarations das funcoes exportadas do Artcode
extern void* art_create_i64(int64_t val);
extern int64_t art_extract_i64(void* ptr);
extern void art_value_retain(void* ptr);
extern void art_value_release(void* ptr);

int main() {
    // Exemplo chamativo de instanciacao:
    void* meu_valor = art_create_i64(42);
    
    // Extrai devolta:
    int64_t result = art_extract_i64(meu_valor);
    printf("Resultado extraido da VM Artcode: %lld\\n", (long long)result);
    
    // Libera heap:
    art_value_release(meu_valor);
    return 0;
}
```

## Como C Strings funcionam?

Para integrar texto oriundo do ambiente C para Artcode, ou transformar uma `String (<Arc<str>>)` da VM em um C String literal que pode ser impresso no `printf`, utilizamos `art_string_to_cstr`.

**Cuidado com vazamentos de Memória**: O Artcode é construído através de um complexo contador de referências Cíclico (ARC). Como o modelo C abdica do ARC, a ponte FFI foi desenhada num pretexto de *Memory Empréstimo*. Uma `String` retornada de `art_string_to_cstr` está "viva" debaixo dos panos. Ela deve ser explicitamente morta através do invocador (C) usando a diretiva correspondente do Artcode `art_free_cstr(sua_string)`. Caso não o faça, a representação binária nativa C string não retornará e a VM ficará com leak.
