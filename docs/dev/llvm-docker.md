# Imagem Docker com LLVM (dev / CI opt-in)

Este documento descreve um Dockerfile mínimo para criar um runner com LLVM/clang e `llvm-tools` instalados, usado pelo job opt-in `jit-smoke` que testa `--features=jit`.

Resumo rápido
- Base: `ubuntu:24.04`
- Instala: `build-essential`, `clang`, `llvm`, `cmake`, `pkg-config`, `libssl-dev`, `git`, `curl`
- Instala `rustup` e toolchain estável; instala `inkwell` dependência no job se necessário.

Uso (local)

1. Build: `docker build -t artcode-llvm -f ci/docker/llvm/Dockerfile .`
2. Run (interactive): `docker run --rm -it artcode-llvm /bin/bash`

CI
- Adicione uma matrix job que usa essa imagem para executar `cargo test -p jit --features=jit` e um pequeno `jit-smoke` que compila um microkernel com `--features=jit`.

Observação: o repositório já documenta a intenção de um job opt-in; este arquivo serve como referência para replicar localmente.

Reprodução rápida (script)
---------------------------------
O repositório inclui um script útil que constrói a imagem e executa os testes JIT dentro do container:

    scripts/run_jit_smoke_in_docker.sh

Use este script para reproduzir localmente o job `jit-smoke` antes de abrir PRs que toquem na implementação JIT.
# LLVM Dev Docker image

Este documento descreve como criar uma imagem Docker com LLVM dev libs necessárias
para compilar e testar a feature `jit` localmente ou em CI runners.

Exemplo de Dockerfile mínimo (Ubuntu):

```dockerfile
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y build-essential ca-certificates curl \
    llvm-dev libclang-dev clang pkg-config cmake git ca-certificates
RUN useradd -m builder
USER builder
WORKDIR /home/builder
```

Build e use:

```bash
docker build -t artcode-llvm:latest .
docker run --rm -it -v $(pwd):/work -w /work artcode-llvm:latest bash
# dentro do container
cargo test -p jit --features=jit
```

Notas:
- Ajuste a versão do LLVM conforme necessário para compatibilidade com `inkwell`.
- Em runners CI prefira imagens oficiais ou cache de pacotes para reduzir tempo de setup.
# LLVM development Docker image

This document shows a small Dockerfile and usage to create a reproducible image with LLVM dev libs and Rust toolchain for building the `jit` feature locally or in CI.

Dockerfile (example):

```dockerfile
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl git cmake pkg-config llvm-dev clang && \
    rm -rf /var/lib/apt/lists/*

# Install Rust toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspaces

CMD ["/bin/bash"]
```

Usage:

1. Build the image:

```sh
# docs/dev/llvm-docker.md

Este documento descreve um Dockerfile mínimo e passos para criar um ambiente de desenvolvimento
com LLVM (útil para implementar e testar a feature `jit` que depende de `inkwell`).

Resumo
- Base sugerida: `ubuntu:24.04`
- Pacotes típicos: `build-essential`, `clang`, `llvm-dev`, `libclang-dev`, `cmake`, `pkg-config`, `git`, `curl`
- Instalar `rustup` dentro do container para ter toolchain Rust.

Exemplo mínimo de Dockerfile

```dockerfile
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential clang llvm-dev libclang-dev cmake pkg-config git curl ca-certificates \
    libssl-dev && rm -rf /var/lib/apt/lists/*

# Instala Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /work
CMD ["/bin/bash"]
```

Como usar
1. Build:

```bash
docker build -t artcode-llvm -f Dockerfile .
```

2. Run com o código montado:

```bash
docker run --rm -it -v "$(pwd)":/work -w /work artcode-llvm bash
# dentro do container
rustup default stable
cargo build -p jit --features=jit
```

Notas e troubleshooting
- Se `inkwell` não encontrar `libclang`, instale o pacote de desenvolvimento correto (`libclang-dev`) e ajuste `LD_LIBRARY_PATH` se necessário.
- Para garantir reproduzibilidade em CI, prefira pinagem de versão (ex.: `llvm-16-dev`) ou uma imagem pré-construída no registry da organização.

Próximo passo sugerido
- Adicionar um job opcional `jit-smoke` na CI que usa esta imagem para compilar e executar um microbenchmark com `--features=jit`.
