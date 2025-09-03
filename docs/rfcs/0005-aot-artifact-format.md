RFC 0005 — AOT artifact format
===============================

Estado: proposta

Contexto
--------
Fase 10 exige que o CLI `art build --with-profile` produza um artefato AOT consumível por instalações e pipelines posteriores. O repositório já gera um plano (JSON) e um artifact minimal JSON para identificação; precisamos padronizar formato que equilibre praticidade, reprodutibilidade e compatibilidade com ambientes sem LLVM.

Opções consideradas
--------------------

1) Manifest JSON + tarball contendo bitcode/object files
   - Forma: `artifact.json` (manifest) + `package.tar.gz` com `*.bc` (LLVM bitcode) e/ou `*.o` objetos.
   - Vantagens: separa metadados do payload binário; permite múltiplos targets (bitcode + objects) no mesmo pacote; fácil inspeção via manifest.
   - Desvantagens: requer toolchain para gerar bitcode/objects (inkwell/clang/llc) — mais infra.

2) Single JSON artifact with embedded base64 blobs
   - Forma: um `artifact.json` com campos base64 para cada arquivo binário.
   - Vantagens: simples para transmissão, único arquivo; sem tar necessário.
   - Desvantagens: menos amigável para ferramentas nativas; aumenta o arquivo JSON e torna diffs pesados; extra transformação para consumir no disk.

3) Directory layout (manifest + files in folder)
   - Forma: `artifact/manifest.json` + `artifact/bitcode/*.bc` + `artifact/objects/*.o`
   - Vantagens: simples localmente, fácil inspeção e edição incremental.
   - Desvantagens: menos conveniente para distribuição como único artefato; precisa de empacotamento para transporte.

Recomendação
------------
Adotar a opção (1): um `artifact.json` manifest e um tarball `artifact.tar.gz` contendo os arquivos binários (bitcode/objects). Rationale:

- Separação clara entre metadados e payload. O manifest descreve versão do formato, targets suportados, checksums e provenance (profile hash, toolchain versions).
- Facilita distribuição (um manifest + um pacote) e compatibilidade com CI/infra (checkout + tar extraction).
- Mantém o CLI leve: inicialmente podemos gerar apenas the manifest (já feito) and include placeholder tarball; emission real de bitcode/objects é trabalho incremental.

Manifest (exemplo)
```
{
  "schema_version": "0.1",
  "package": "mycrate-0.1",
  "profile_hash": "...",
  "entries": [
    { "path": "lib.bc", "type": "bitcode", "sha256": "..." },
    { "path": "lib.o", "type": "object/x86_64", "sha256": "..." }
  ]
}
```

Rollout
-------
- Phase 1 (current): `art build --with-profile` writes `artifact.json` manifest only (already implemented).
- Phase 2: toolchain to emit LLVM bitcode using inkwell behind `--features=jit` or using `clang`/`llc` in CI image; artifact tarball created and checksums recorded.
- Phase 3: optional signing and reproducible builds.

Notas de implementação
---------------------
- Add helper in `cli::aot` to compose and sign an artifact tarball.
- Extend `xtask` to produce sample AOT packages for CI testing.
- Document artifact consumption API in `crates/jit` and any AOT consumers.

Conclusão
---------
Manifest+tarball dá o melhor equilíbrio para o projeto: fácil de evoluir, compatível com o roadmap e evita forçar LLVM em todos os contributors. Vou proceder com essa decisão para as próximas implementações.
