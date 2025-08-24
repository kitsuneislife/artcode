# Sistema de Módulos & Pacotes — MVP

Este documento descreve o comportamento e as regras mínimas do MVP do sistema de módulos.

Resolução (ordem):
- Imports relativos (começando pelo diretório do arquivo importador): `import ./util` ou `import utils.foo` -> `utils/foo.art` ou `utils/foo/mod.art`.
- Imports não-relativos: tratam-se como caminhos relativos ao workspace ou ao cache `~/.artcode/cache`.
- Tentativas de arquivo: `X`, `X.art`, `X/mod.art`.

Manifesto `Art.toml` (esqueleto):
```toml
name = "my-lib"
version = "0.1.0"
dependencies = { other = { path = "../other" } }
```

CLI MVP `art add <path-or-git>`: copia diretório/arquivo para `~/.artcode/cache/<name>-<version>` se `Art.toml` existir, caso contrário copia usando o nome do diretório.

Notas de segurança: MVP resolve apenas arquivos locais; resolução de rede/git é opcional futura.
