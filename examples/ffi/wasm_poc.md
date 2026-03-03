# Prova de Conceito (PoC): WebAssembly (WASM)

A fase 11 do manifesto do Artcode inclui a compilação do ecossistema interpretado para WebAssembly.

Como o `core` isolou as primitivas de FFI usando a macro `art_extern!`, funções puras numéricas agora não sofrem com problemas comportamentais da arquitetura base do sistema operacional.

## Compilando o Core para WASM
Para gerar um módulo `.wasm` (que pode ser rodado posteriormente num Runtime de JS ou WASMTim), utilizamos a target `wasm32-unknown-unknown`.

```bash
rustup target add wasm32-unknown-unknown
cd crates/core
cargo build --release --target wasm32-unknown-unknown
```

Isto ira gerar o artefato `core.wasm` no diretório alvo do `cargo`.

As funções do FFI que mapeamos como `art_create_i64` ou `art_extract_i64` serão exportadas e disponíveis sem *name-mangling* para quem integrar esse WebAssembly, permitindo uso idêntico ao exemplo nativo em C.
