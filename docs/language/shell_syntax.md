# Sintaxe Shell

Artcode agora suporta um statement shell de primeira camada com prefixo `$`.
Tambem suporta chamada estilo funcao para executaveis no PATH quando o identificador nao resolve para simbolo Artcode.

## Forma suportada

```art
$ echo "hello"
$ ls -la
$ echo hello |> tr a-z A-Z

let r = echo("hello from function")
match r {
	case .Ok(out): println(f"ok={out}")
	case .Err(err): println(f"err={err}")
}

match shell_result {
	case .Ok(out): println(f"ok={out}")
	case .Err(err): println(f"err={err}")
}
```

Regras atuais:
- O statement inicia com `$` e consome os tokens da mesma linha (ou ate `;`).
- O primeiro argumento e o programa.
- Os demais sao passados como argumentos para `std::process::Command`.
- `|>` conecta estagios de processo (stdout do estagio anterior vira stdin do proximo).
- Strings entre aspas viram um argumento unico.
- Chamadas `cmd(arg1, arg2, ...)` onde `cmd` nao existe como simbolo Artcode sao mapeadas para execucao shell de `cmd` no PATH.
- O runtime publica o retorno da ultima execucao shell em `shell_result` como `Result.Ok(stdout)` ou `Result.Err(stderr)`.
- Em `--pure`, comandos shell sao bloqueados com diagnostico de runtime.

## Exemplo

Veja [examples/35_shell_syntax.art](../examples/35_shell_syntax.art).

## Limitacoes desta fase

- Ainda nao ha expansao de variaveis/globs.

## Validacao

A cobertura desta fase inclui:
- Lexer: reconhecimento de token `$`.
- Parser: construcao de `Stmt::ShellCommand`.
- Runtime: execucao de comando externo, pipeline `|>`, retorno tipado em `shell_result` e bloqueio em modo `--pure`.
- Runtime: mapeamento de chamada estilo funcao para executaveis no PATH (`echo("ok")` etc.).
- CLI: teste de integracao com `art run`.
