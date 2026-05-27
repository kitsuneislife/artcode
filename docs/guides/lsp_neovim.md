# LSP — Neovim

## Pré-requisitos

- Neovim 0.9+
- Plugin [`nvim-lspconfig`](https://github.com/neovim/nvim-lspconfig)
- `art` no `PATH` (instale com `cargo install --path cli` na raiz do repositório)

## Configuração

### Com nvim-lspconfig (recomendado)

Adicione ao seu `init.lua` (ou arquivo de configuração equivalente):

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

-- Registra o servidor "artcode" se ainda não existe
if not configs.artcode then
  configs.artcode = {
    default_config = {
      cmd = { 'art', 'lsp' },
      filetypes = { 'artcode' },
      root_dir = lspconfig.util.root_pattern('.git', '*.art'),
      settings = {},
    },
  }
end

lspconfig.artcode.setup({
  on_attach = function(client, bufnr)
    local opts = { noremap = true, silent = true, buffer = bufnr }
    vim.keymap.set('n', 'gd', vim.lsp.buf.definition, opts)
    vim.keymap.set('n', 'K',  vim.lsp.buf.hover, opts)
    vim.keymap.set('n', '<leader>rn', vim.lsp.buf.rename, opts)
    vim.keymap.set('n', '<leader>ca', vim.lsp.buf.code_action, opts)
  end,
})
```

### Associação de tipo de arquivo

Crie `~/.config/nvim/ftdetect/artcode.vim`:

```vim
au BufRead,BufNewFile *.art setfiletype artcode
```

## Funcionalidades disponíveis

| Funcionalidade          | Comando Neovim            |
|-------------------------|---------------------------|
| Goto Definition         | `gd` / `vim.lsp.buf.definition` |
| Hover (tipo/assinatura) | `K` / `vim.lsp.buf.hover`       |
| Completion              | Automático com `nvim-cmp`       |
| Rename Symbol           | `<leader>rn`                    |
| Diagnósticos            | `vim.diagnostic.open_float`     |

## Integração com nvim-cmp

Se você usa [`nvim-cmp`](https://github.com/hrsh7th/nvim-cmp), o completion do LSP
é disponibilizado automaticamente pela source `nvim_lsp` sem configuração adicional.

## Verificação

Execute `:LspInfo` num buffer `.art` para confirmar que `artcode` está ativo e conectado.
`:LspLog` exibe os logs do protocolo JSON-RPC caso haja problemas.
