FROM ubuntu:24.04
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
  build-essential ca-certificates curl git cmake pkg-config llvm-dev libclang-dev clang
RUN useradd -m builder
USER builder
WORKDIR /home/builder
