FROM ubuntu:22.04

# Add official LLVM apt repository for LLVM 15 (provides libpolly and other components)
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y wget gnupg ca-certificates && \
    wget -O - https://apt.llvm.org/llvm-snapshot.gpg.key | apt-key add - && \
    echo "deb http://apt.llvm.org/jammy/ llvm-toolchain-jammy-15 main" > /etc/apt/sources.list.d/llvm.list

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
  build-essential ca-certificates curl git cmake pkg-config clang-15 libclang-15-dev llvm-15-dev llvm-15 clang-tools-15 libpolly15 libpolly-15-dev
RUN update-alternatives --install /usr/bin/clang clang /usr/bin/clang-15 100 && \
    update-alternatives --install /usr/bin/clang++ clang++ /usr/bin/clang++-15 100
RUN useradd -m builder
USER builder
WORKDIR /home/builder

# Install rustup and stable toolchain for running cargo inside the container
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/home/builder/.cargo/bin:${PATH}"
RUN rustup default stable
