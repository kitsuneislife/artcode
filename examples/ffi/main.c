#include <stdint.h>
#include <stdio.h>

// Forward declarations das funcoes exportadas pela FFI segura do Artcode (core)
extern uint64_t art_handle_create_i64(int64_t val);
extern uint8_t art_handle_retain(uint64_t handle);
extern uint8_t art_handle_release(uint64_t handle);
extern int32_t art_handle_extract_i64(uint64_t handle, int64_t* out_value);

int main() {
    printf("--- Exemplo Artcode FFI seguro (C) ---\n");

    // O C cria um handle opaco no registry da VM.
    uint64_t handle = art_handle_create_i64(420);
    printf("1. Handle criado: %llu\n", (unsigned long long)handle);

    // Simula compartilhamento entre escopos nativos
    art_handle_retain(handle);

    // Extrai payload i64 com codigo de erro explicito
    int64_t result = 0;
    int32_t rc = art_handle_extract_i64(handle, &result);
    if (rc == 0) {
        printf("2. Valor processado pelo Core: %lld\n", (long long)result);
    } else {
        printf("2. Falha no extract, codigo=%d\n", rc);
    }

    // Libera refs do handle
    art_handle_release(handle);
    art_handle_release(handle);

    printf("3. Handle liberado sem double-free.\n");
    return 0;
}
