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
docker build -t artcode-llvm -f Dockerfile .
```

2. Run a shell and build with JIT feature:

```sh
docker run --rm -it -v "$PWD":/workspaces -w /workspaces artcode-llvm bash
# inside container
rustup default stable
cargo build -p jit --features=jit
```

Notes:
- Building with `--features=jit` requires LLVM dev headers/libraries available to the system package manager. Versions may vary across distributions; prefer an Ubuntu LTS image with matching `inkwell`/LLVM versions.
- This image is intentionally minimal; for CI pin to a stable Ubuntu runner or provide a hosted runner with LLVM preinstalled.
# Como preparar um ambiente com LLVM (dev) via Docker

Este documento descreve um container Docker leve para desenvolver e testar o backend JIT baseado em LLVM (`inkwell`). O objetivo é fornecer uma imagem reutilizável que facilite a execução de testes JIT em runners locais ou CI opt-in.

Imagem base (Ubuntu LTS)

Dockerfile mínimo (exemplo):

```dockerfile
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential curl ca-certificates pkg-config git cmake clang llvm-dev libclang-dev \
    libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

# Rust toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Opcional: instalar cargo-chef, etc. para builds otimizados
RUN cargo install cargo-chef --locked || true

# Defina diretório de trabalho
WORKDIR /work

CMD ["/bin/bash"]
```

Como usar

1. Construa a imagem:

   docker build -t artcode-dev-llvm .

2. Rode um container interativo montando o código fonte:

   docker run --rm -it -v $(pwd):/work -w /work artcode-dev-llvm

3. Dentro do container, habilite a feature `jit` ao compilar:

   cargo build -p jit --features=jit

Notas
- Dependendo da versão do LLVM/Clang disponível nas distros, pode ser necessário ajustar pacotes (por exemplo `llvm-15-dev`).
- Use esta imagem apenas para desenvolvimento JIT; a imagem padrão dos colaboradores não precisa de LLVM.
