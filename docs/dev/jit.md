# JIT / AOT Developer Guide

This document explains the dev workflow for the JIT/AOT tooling in this repo.

Prerequisites
- Rust toolchain (stable)
- Optional: LLVM dev libraries and `inkwell` if you plan to enable the `jit` feature.
- There is a Docker image under `ci/docker/llvm/` that provides a compatible LLVM development environment.

Common tasks

- Inspect a generated profile + plan:

```bash
# run the aot_inspect tool (will write aot_plan.normalized.json)
cargo run -p jit --bin aot_inspect -- profile.json aot_plan.json [path/to/ir_dir]
```

- Simulate a consumer/scheduler (prints compile order):

```bash
cargo run -p jit --bin aot_consumer -- aot_plan.normalized.json
```

- Run via xtask

```bash
# xtask wrapper will pick default files if not provided
cargo run -p xtask -- aot-inspect --profile profile.json --plan aot_plan.json --ir-dir ci/ir
```

Enabling the JIT (experimental)

1. Install LLVM development packages on your system (or use the Docker image under `ci/docker/llvm`).
2. Build with the `jit` feature:

```bash
cargo build --workspace --features=jit
```

Notes
- The current JIT crate is a scaffold. The `aot_inspect` and `aot_consumer` tools are small helpers for validation and scheduling.
- `aot_inspect` includes a lightweight cost estimate based on textual IR file size if you provide `--ir-dir`.

