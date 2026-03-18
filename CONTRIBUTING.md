# Contribuição ao Artcode (Contributing)

Bem-vindo(a) ao repositório do projeto da linguagem **Artcode**! Estamos muito felizes pelo seu interesse. Como um projeto jovem focado na complexidade progressiva das premissas de Atores e Arena GC, todo o auxílio no Parser, Engine Interpretador, CLI, e futura compilação IR/AOT é valioso!

### Como posso contribuir?

#### 1. Bug Reports e Funcionalidades Rápidas
Se você encontrar bugs ou panics no compilador, problemas de performance, leaks de memória (`cargo test`), ou tiver sugestões curtas de funcionalidade (como otimização de cache/strings ou Quick Wins de LSP):
- Procure se a issue já existe no repositório GitHub.
- Sinta-se à vontade para abrir uma nova _Issue_.
- Discutiremos, faremos a triagem dela no Milestone correto, e você poderá abrir um **Pull Request**.

As issues abertas recebem triagem automática via GitHub Actions com categorias:
- `lang-design`: linguagem, parser/lexer, sintaxe, semântica, tipagem, RFC.
- `runtime`: interpreter/VM, memória (ARC), performance, JIT/AOT, FFI.
- `tooling`: CLI, LSP, formatter/linter, CI e documentação.

#### 2. Processo RFC (Request for Comments)
O Artcode segue um processo formal de RFC para mudanças estruturais (linguagem, runtime, arquitetura de compilação e contratos públicos).

Antes de propor qualquer mudança profunda (tipagem, atores, memória, FFI, JIT/AOT), você **DEVE** seguir este fluxo:

1. Copie o template canônico em [`docs/rfcs/0000-template.md`](docs/rfcs/0000-template.md).
2. Crie uma proposta em Issue ou Draft PR, incluindo motivação, design detalhado, alternativas e riscos.
3. Aguarde revisão/consenso. Implementação só começa após aceitação do RFC.
4. Se houver decisão arquitetural relevante, registre também um ADR em [`docs/decisions/`](docs/decisions/).

Para papéis e escopo de decisão do projeto, consulte [GOVERNANCE.md](GOVERNANCE.md).

#### Preparando seu Ambiente
A compilação e testes contínuos ocorrem inteiramente em Cargo Mvp base.
```bash
cargo check
cargo test --all
```

Ao rodar os testes, valide também o estado do roadmap operacional em `.kit/checklist-v0.2.0.md`. Bons commits :)
