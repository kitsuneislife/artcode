#include <stdint.h>
#include <stdio.h>

// Forward declarations das funcoes exportadas pela FFI do Artcode (core)
extern void* art_create_i64(int64_t val);
extern int64_t art_extract_i64(void* ptr);
extern void art_value_retain(void* ptr);
extern void art_value_release(void* ptr);

int main() {
    printf("--- Exemplo Artcode FFI (C) ---\n");

    // O C chama a maquina virtual para criar uma variavel Artcode Heap-based
    void* meu_valor = art_create_i64(420);
    printf("1. Construido novo ArtValue nativo no heap: %p\n", meu_valor);
    
    // Podemos emular o borrowing caso outra funcao C va reutilizar
    art_value_retain(meu_valor); // Simulando outro scope

    // Extrair de volta o payload nativo de i64
    int64_t result = art_extract_i64(meu_valor);
    printf("2. Valor processado pelo Core: %lld\n", (long long)result);
    
    // Libera heap: release do retain fake e depois a criacao em si
    art_value_release(meu_valor);
    art_value_release(meu_valor);
    
    printf("3. Memoria do Core desalocada.\n");
    return 0;
}
