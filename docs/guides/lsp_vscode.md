# LSP — VS Code

## Instalação

Não há extensão publicada na Marketplace. Configure o servidor LSP manualmente via
[`vscode-langclient`](https://github.com/microsoft/vscode-languageserver-node) ou use
a extensão genérica **"LSP Client"**.

### Opção A — extensão genérica (mais rápida)

1. Instale a extensão [Generic LSP Client](https://marketplace.visualstudio.com/items?itemName=llg.generic-lsp-client) no VS Code.

2. Adicione ao `.vscode/settings.json` do projeto:

```json
{
  "genericLspClient.serverOptions": {
    "command": "art",
    "args": ["lsp"],
    "transport": "stdio"
  },
  "genericLspClient.documentSelector": [
    { "scheme": "file", "language": "artcode" }
  ]
}
```

3. Adicione a associação de linguagem no mesmo arquivo:

```json
{
  "files.associations": {
    "*.art": "artcode"
  }
}
```

### Opção B — extensão dedicada (em desenvolvimento)

Uma extensão nativa está prevista para v0.5. Enquanto isso, use a Opção A.

## Funcionalidades disponíveis

| Funcionalidade         | Status     |
|------------------------|------------|
| Diagnósticos (erros)   | Disponível |
| Hover (tipo/assinatura)| Disponível |
| Goto Definition        | Disponível |
| Completion             | Disponível |
| Rename Symbol          | Disponível |
| Semantic Highlighting  | Disponível |

## Requisitos

- `art` no `PATH` (instale com `cargo install --path cli` na raiz do repositório)
- VS Code 1.75+

## Verificação

Abra um arquivo `.art` e execute `> Developer: Show Logs > Extension Host` para confirmar
que o LSP iniciou sem erros. O primeiro hover sobre uma função exibe a assinatura.
