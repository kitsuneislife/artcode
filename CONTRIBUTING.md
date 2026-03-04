# Contribuição ao Artcode (Contributing)

Bem-vindo(a) ao repositório do projeto da linguagem **Artcode**! Estamos muito felizes pelo seu interesse. Como um projeto jovem focado na complexidade progressiva das premissas de Atores e Arena GC, todo o auxílio no Parser, Engine Interpretador, CLI, e futura compilação IR/AOT é valioso!

### Como posso contribuir?

#### 1. Bug Reports e Funcionalidades Rápidas
Se você encontrar bugs ou panics no compilador, problemas de performance, leaks de memória (`cargo test`), ou tiver sugestões curtas de funcionalidade (como otimização de cache/strings ou Quick Wins de LSP):
- Procure se a issue já existe no repositório GitHub.
- Sinta-se à vontade para abrir uma nova _Issue_.
- Discutiremos, faremos a triagem dela no Milestone correto, e você poderá abrir um **Pull Request**.

#### 2. Processo RFC (Request for Comments)
O Artcode segue um modelo de Governança para decisões arquiteturais baseada no modelo original de Especificações e RFC do Ecossistema Rust!

Antes de propôr qualquer mudança profunda (novas features de Typings (Fase 18), mudança da lógica cooperativa dos Atores (Fase 9), ou de Memory Lifetime (Fase 8), você **DEVE** trilhar o caminho do processo RFC!

1. Copie o template localizado em [`docs/rfcs/0000-template.md`](docs/rfcs/0000-template.md).
2. Escreva sua proposta em uma [Issue/Draft PR] detalhando todo o seu design até as alternativas de Interoperação.
3. Debatemos até a aceitação ou recusa. Só após aprovado que inicia-se o Pull Request com a implementação.

Para checar os detalhes de liderança e escopo de Decisão do projeto, recomendamos que consulte fortemente o documento de [Governança (`GOVERNANCE.md`)](GOVERNANCE.md) na raiz do repositório!

#### Preparando seu Ambiente
A compilação e testes contínuos ocorrem inteiramente em Cargo Mvp base.
```bash
cargo check
cargo test --all
```

Ao rodar os testes garanta sempre de validar as fases anteriores de `fuzzing`/`stress` que construímos em `.kit/checklist.md`! Bons commits :)
