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
