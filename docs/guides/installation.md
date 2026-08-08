# Dependências e instruções de instalação

Resumo
- Este documento lista as dependências necessárias para compilar e executar o workspace `artcode` e fornece passos de instalação para Ubuntu, Arch Linux e Windows.

Dependências Rust (crates)
- Crates internos (workspace): `core`, `lexer`, `parser`, `interpreter`, `diagnostics`, `cli`, `xtask` (referenciados via path nos Cargo.toml).
- Crates externas declaradas nos manifests do projeto:
  - `clap` = 4.5.4 (features: derive) — usado pelo `cli`.
  - `clap` = 4 (usado pelo `xtask`).
  - `serde` = "1.0" (features: derive).
  - `serde_json` = "1.0".
  - `regex` = "1" (usado por `xtask`).
  - `criterion` = "0.5" (dev-dependency para benches, features: html_reports).

Componentes do Rust toolchain (mínimos)
- Rust (stable) e Cargo — instalar via rustup.
- Componentes recomendados:
  - `rustfmt` (cargo fmt) — usado por `cargo run -p xtask -- devcheck`.
  - `clippy` (cargo clippy) — usado por `cargo run -p xtask -- devcheck`.

Dependências de sistema / utilitários (recomendados)
- build-essential / base-devel (compilador C/C++ e ferramentas de build)
- pkg-config
- curl, git, bash, python3
- clang / llvm (opcional — necessário para cobertura com `cargo-llvm-cov`)
- cargo-llvm-cov (opcional, instalar via `cargo install cargo-llvm-cov`)
- GitHub CLI `gh` (opcional — facilita criar releases e subir assets)

Comandos úteis do projeto
- Compilar e rodar testes:
```bash
cargo test --all
```
- Rodar todos os exemplos:
```bash
cargo run -p xtask -- run-examples
```
- Rodar CLI em um exemplo:
```bash
cargo run --bin art -- run examples/00_hello.art
```

Instalação: Ubuntu 24.04 (passos rápidos)
1) Pacotes do sistema:
```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl git python3 python3-venv ca-certificates
```
2) Instalar Rust (rustup) e componentes:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env
rustup default stable
rustup component add rustfmt clippy
```
3) Ferramentas opcionais (cobertura / uploads):
```bash
sudo apt install -y clang llvm
cargo install cargo-llvm-cov
sudo apt install -y gh   # opcional
```

Instalação: Arch Linux (passos rápidos)
1) Pacotes do sistema:
```bash
sudo pacman -Syu --noconfirm base-devel pkgconf curl git python
```
2) Instalar Rust (pode usar o pacote ou rustup diretamente):
```bash
# opcional via pacman
sudo pacman -S --noconfirm rustup
rustup default stable
rustup component add rustfmt clippy
```
3) Ferramentas opcionais:
```bash
sudo pacman -S --noconfirm clang llvm
cargo install cargo-llvm-cov
```

Instalação: Windows (passos recomendados)
- Opção recomendada: usar WSL2 (Ubuntu) e seguir instruções de Ubuntu acima — evita muitas diferenças de toolchain no Windows.

Se for instalar nativamente no Windows:
1) Instalar Rust via rustup (PowerShell administrador):
```powershell
iwr https://sh.rustup.rs -UseBasicParsing | iex
# ou baixar e executar https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe
```
2) Toolchain para compilação MSVC (recomendado): instale "Build Tools for Visual Studio" com workload C++ (Visual C++ build tools) — fornece o compilador MSVC.
3) No PowerShell (após instalar rustup):
```powershell
rustup default stable
rustup component add rustfmt clippy
```
4) Ferramentas opcionais:
 - Instalar LLVM/Clang para Windows se precisar de cobertura; configuração extra pode ser necessária.
 - Alternativa: usar MSYS2 / pacman para instalar dependências nativas.

### Windows: repositório dentro de OneDrive

Se o clone estiver numa pasta sincronizada pelo OneDrive, mande a saída de build
para fora dela. Binários escritos em pasta sincronizada recebem metadados de
nuvem, e o Smart App Control os trata como conteúdo baixado — executáveis recém
compilados passam a ser bloqueados:

```
error: ... Uma política de Controle de Aplicativo bloqueou este arquivo. (os error 4551)
```

O sintoma é intermitente e confuso: o bloqueio é por hash, então cada `cargo`
com unificação de features diferente gera um binário novo que pode ou não
passar. Isso atinge principalmente os testes que executam o binário `art`.

Crie `.cargo/config.toml` na raiz do repositório (o arquivo é gitignorado, pois
o caminho é específico da máquina):

```toml
[build]
target-dir = "C:/dev/artcode-target"
```

Além de resolver o bloqueio, isso evita que gigabytes de artefatos de build
sincronizem para a nuvem. Não é necessário desligar o Smart App Control — e não
convém: desligá-lo é irreversível sem reinstalar o Windows.

Um caso relacionado: componentes do rustup recém-lançados podem não ter
reputação ainda e serem bloqueados mesmo fora do OneDrive. Reinstalar o
componente resolve, porque baixa um artefato já conhecido:

```powershell
rustup component remove clippy; rustup component add clippy
```

### Verificar o ambiente

`scripts/check_system.sh` percorre as dependências acima e relata o que falta,
separando o que é crítico do que é opcional. Requer bash (no Windows, Git Bash
ou WSL).

```bash
bash scripts/check_system.sh
```

## Comandos de teste

Esta seção reúne os comandos de build/test/validação usados no desenvolvimento e CI do projeto. Todos os comandos abaixo devem ser executados a partir da raiz do workspace (`/workspaces/artcode`).

1) Build e testes básicos

```bash
# Compilar todo o workspace
cargo build --all

# Rodar todos os testes (unit + integration) do workspace
cargo test --all

# Rodar um único teste (ex.: nome_do_teste)
cargo test -p interpreter --test runtime -- test_name
```

2) Validar exemplos

```bash
# Compila o binário se necessário e executa todos os exemplos de examples/
cargo run -p xtask -- run-examples

# Executar um único exemplo via binário (válido se target/debug/art existir)
cargo run --bin art -- run examples/00_hello.art
```

3) Portão de qualidade completo

`devcheck` é o mesmo conjunto de verificações que o CI executa: formatação,
clippy com `-D warnings`, testes, todos os exemplos e varredura de panics.
Rodar isto antes de commitar é o que evita descobrir uma regressão só no CI.

```bash
cargo run -p xtask -- devcheck

# Com relatório de cobertura (requer cargo-llvm-cov)
cargo run -p xtask -- devcheck --coverage
```

Para rodar um passo isolado:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p xtask -- scan            # só a varredura de panics
```

5) Benchmarks

```bash
# Executa benchmarks configurados (criterion)
cargo bench

# Executar bench específico (se definido em bench/cases/)
cargo bench -p interpreter
```

6) Cobertura (llvm-cov) — opcional e requer LLVM/cargo-llvm-cov

```bash
# Exemplo de uso (requer cargo-llvm-cov instalado e clang/llvm compatível)
# Gera relatório HTML para o workspace
cargo llvm-cov --workspace --html --open

# Para CI sem abrir, apenas gerar artefato
cargo llvm-cov --workspace --output-path target/llvm-cov
```

7) Métricas e execução com saída JSON

```bash
# Executa o CLI para coletar métricas (o script escreve em artifacts/metrics.json por padrão)
scripts/run_metrics.sh

# Ou diretamente (exemplo):
target/debug/art metrics --json examples/99_weak_unowned_demo.art > artifacts/metrics.json
```

8) Execução incremental / debug rápidos

```bash
# Rodar o binário em modo debug
cargo run --bin art -- run examples/00_hello.art

# Rodar testes com saída mais verbosa
cargo test --all -- --nocapture
```

9) Atualização de versão (CLI)

```bash
# verificar se há release nova
art update --check

# atualizar automaticamente usando o instalador oficial
art update --self
```

Notas:
- O comando de update usa cache local de checagem para evitar consultas frequentes.
- Em terminal interativo, o CLI pode exibir aviso de nova release disponível.
- Para desativar o aviso automático: ART_DISABLE_UPDATE_CHECK=1

Notas rápidas
- Muitos testes dependem do workspace estar compilado (`cargo build --all`) e do binário `art` estar disponível em `target/debug/art`.
- Em CI e localmente, o portão único é `cargo run -p xtask -- devcheck`, que cobre formatação, clippy, testes, exemplos e varredura de panics. Cobertura fica opcional via `--coverage`.

## Requisitos do sistema (comparativo)

A tabela abaixo resume requisitos de sistema para diferentes cenários (mínimo para funcionalidade, recomendado para desenvolvimento confortável e alto/ideal para build/CI/bench de grande escala). Use isso como guia para escolher máquinas locais, runners CI ou servidores de build.

| Componente / Recurso | Mínimo (funcional) | Recomendado (desenvolvedor) | Alto / Ideal (CI / builds pesados) | Notas/Explicações |
|---|---:|---:|---:|---|
| CPU (arquitetura) | x86_64 ou aarch64 (dual-core) @1.6 GHz | x86_64 ou aarch64 quad-core físico @2.4+ GHz | x86_64 / aarch64 8+ cores físicos @2.8+ GHz (c/smt desativado preferível) | Rust/Cargo e compilação de crates nativos beneficiam-se de clocks e núcleos; SSE/AVX não obrigatórios. |
| Cores / Threads | 2 cores | 4 cores / 8 threads | 8+ cores / 16+ threads | Paralelismo do cargo (-j) e builds paralelos reduzem tempo; CI com runners paralelos acelera testes. |
| Memória RAM | 2 GB | 8 GB | 32 GB+ | Testes grandes e compilação incremental e benchs (criterion) exigem mais memória; 2 GB pode ser lento e provocar OOM. |
| Swap | 1 GB | 4 GB | 8+ GB | Swap evita OOM em picos; preferir SSD para swap. |
| Armazenamento livre (disco) | 5 GB | 20 GB | 100 GB+ | `target/` e caches (cargo, LLVM) ocupam espaço; builds com cobertura/llvm-cov geram artefatos grandes. |
| Tipo de disco | HDD aceitável | SSD (NVMe preferível) | NVMe SSD | SSD reduz tempo de linking e IO durante testes; NVMe melhor para CI heavy. |
| Sistema Operacional | Linux (Ubuntu 20.04+) ou macOS / WSL2 | Ubuntu 22.04 / Arch / macOS 12+ | Ubuntu 24.04 LTS ou imagem Linux custom com LLVM pré-instalado | Windows nativo é suportado, mas WSL2 é recomendado para ferramentas/scripts bash. |
| Toolchain Rust | Rust stable (qualquer) | Rust stable (atualizado nas últimas 6 semanas) | Rust stable (atualizado) + toolchains auxiliares instalados | Alguns crates usam `edition = "2024"` — mantenha toolchain atualizado; use rustup. |
| LLVM / Clang | Não obrigatório | Recomendado para cobertura: clang/llvm (12+) | LLVM compatível com `cargo-llvm-cov` (versão recomendada: 14/15/16 conforme tool) | Para gerar `cargo llvm-cov` e reports; CI costuma ter LLVM pré-instalado. |
| Rede / Banda | Conexão básica para git/cargo | Conexão estável (pouca latência) | Conexão estável e rápida para baixar dependências e artifacts | Builds iniciais baixam crates; CI caches mitigam pulls repetidos. |
| Ferramentas de build | gcc/clang (qualquer) | build-essential / base-devel, pkg-config | build-essential, pkg-config, clang, lld | Para crates que linkam código nativo: tenha toolchain C/C++ instalado. |
| Ferramentas opcionais | curl, git, python3 | gh (GitHub CLI), cargo-llvm-cov | runners com docker, caches, runners multi-core | Facilita fluxos de CI e publicação de releases. |
| GPU | Não necessário | Não necessário | Não necessário | Projeto não usa aceleração por GPU. |
| Recursos humanos (opcional) | 1 dev | Equipa pequena com CI | Infraestrutura de CI dedicada (autoscaling) | Para manutenção e runs pesados. |

Cenários exemplares
- Máquina low-end (teste rápido / experimentos locais): 4 cores (2 físicos + HT), 8 GB RAM, 50 GB SSD, Ubuntu 22.04, rustup com stable.
- Estação de desenvolvimento (recomendado): 6–8 cores, 16 GB RAM, NVMe 250GB+, clang/llvm instalado, rustfmt/clippy, docker (opcional).
- Runner CI para builds e cobertura: 8–16 cores, 32–64 GB RAM, NVMe 500GB+, LLVM instalado (versões compatíveis), cache de dependências configurado.

Boas práticas relacionadas a requisitos
- Reserve espaço e memória para `target/` e artefatos de cobertura; os relatórios HTML e o `llvm-cov` podem consumir dezenas de MBs a centenas de MBs por build.
- Use caches de dependências no CI (cargo registry, cargo git) para reduzir downloads repetidos.
- Para runs de benchmark/criterion, prefira hosts sem outros processos concorrentes para ter medições estáveis.

Contato rápido
- Se quiser, eu adiciono este arquivo ao repositório (já criado) e posso tentar rodar um build/tests aqui se autorizar instalações no container.
