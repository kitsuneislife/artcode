# AOT nativo via LLVM

O Artcode compila funções numéricas puras para binários nativos usando LLVM.
O backend emite **LLVM IR textual** (`.ll`) e o compila com `clang` — sem depender
da biblioteca `inkwell` nem de uma versão específica da C-API do LLVM. Qualquer
`clang` no `PATH` funciona.

## Pré-requisitos

- `clang` instalado e acessível no `PATH` (verifique com `clang --version`).
- Opcional: `llvm-as` para validar o IR emitido (`llvm-as arquivo.ll -o /dev/null`).

## Comandos

### Compilar para binário nativo

```sh
art build-aot programa.art --llvm --out programa
./programa
```

O backend:
1. resolve e tipa o programa;
2. baixa cada função suportada para o IR interno;
3. emite LLVM IR textual;
4. invoca `clang -O2` para gerar o binário nativo.

Binários são cacheados por hash do IR em `$TMPDIR/.artcache/` — recompilações
de um programa inalterado são instantâneas (`CACHE HIT`).

### Inspecionar o LLVM IR

```sh
art build-aot programa.art --emit-llvm-ir --out programa.ll
cat programa.ll
llvm-as programa.ll -o /dev/null   # valida o IR
```

## Exemplo

```art
func main() {
    if true { return 10 } else { return 20 }
}
```

```sh
$ art build-aot exemplo.art --llvm --out exemplo
[AOT] Compilando LLVM IR com clang -O2...
[AOT] Binário nativo exportado para: exemplo
$ ./exemplo
10
```

O valor retornado por `main` é impresso na saída padrão (via `printf`), espelhando
o comportamento do backend C (`art build-aot` sem `--llvm`).

## Cobertura atual

O lowering para IR suporta, por enquanto:

- aritmética inteira (`+`, `-`, `*`, `/`);
- `if`/`else` com retorno em cada ramo (gera `BrCond` + `phi`);
- chamadas de função (incluindo entre funções definidas no mesmo programa).

Laços (`while`/`for`), variáveis locais mutáveis e recursão atravessando
condicionais ainda não são lowered — funções não suportadas são ignoradas no AOT
com um aviso, e o restante do programa continua a compilar.

## Backends de AOT

| Comando                              | Backend         | Saída              |
|--------------------------------------|-----------------|--------------------|
| `art build-aot p.art`                | C (`gcc -O3`)   | binário nativo     |
| `art build-aot p.art --llvm`         | LLVM (`clang`)  | binário nativo     |
| `art build-aot p.art --emit-llvm-ir` | LLVM            | `.ll` textual      |
| `art build-aot p.art --wasm`         | C (`emcc`)      | WebAssembly + HTML |

Veja [RFC 0005 — AOT Artifact Format](../rfcs/0005-aot-artifact-format.md) e
[IR interno](../internals/ir.md) para detalhes de arquitetura.
