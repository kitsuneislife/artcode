# Loops e Tuplas

Este documento cobre os recursos de iteracao e modelagem por tuplas adicionados no ciclo v0.2.

## Loops nativos

Artcode suporta dois loops basicos:

- `while` para repeticao baseada em condicao.
- `for` para iteracao sobre arrays (primeira fase do modelo iteravel).

Exemplo:

```art
var i = 0;
while i < 3 {
    println(i);
    i = i + 1;
}

let nums = [10, 20, 30];
for n in nums {
    println(n);
}
```

## Tuplas literais

Tuplas sao valores compostos de tamanho fixo.

```art
let p = (10, 20);
let one = (42,);
let empty = ();
```

## Destructuring em let

Bindings podem receber patterns de tupla diretamente.

```art
let pair = (7, 9);
let (x, y) = pair;
println(x, y);
```

Tambem funciona para estruturas aninhadas:

```art
let nested = ((1, 2), 3);
let ((a, b), c) = nested;
println(a, b, c);
```

## Estado atual

- `for` atualmente itera sobre arrays e agora também aceita iteradores customizados via protocolo `next()` (isto inclui funções que retornam `Option` para construir generators).
- `break` e `continue` ainda nao fazem parte da sintaxe.
- O sistema de tipos infere `Tuple(...)` e propaga tipos em patterns de `let`.

## Iteradores customizados (protocolo `next()`)

`for` pode iterar sobre qualquer função que retorna um `Option` (internamente representado como `Optional`/`Option.Some` / `Option.None`). A cada iteração, a função é chamada até que ela retorne `Option.None`.

Tambem existe o acucar `yield expr`, que eh desugar para `return Option.Some(expr)`.

Exemplo (veja `examples/38_custom_iterators.art`):

```art
let state = map_new();
map_set(state, "i", 0);

func gen() {
    map_set(state, "i", map_get(state, "i").unwrap_or(0) + 1);
    if map_get(state, "i").unwrap_or(0) <= 3 {
        yield map_get(state, "i").unwrap_or(0);
    }
    return Option.None;
}

let acc = map_new();
map_set(acc, "sum", 0);
for x in gen {
    map_set(acc, "sum", map_get(acc, "sum").unwrap_or(0) + x);
}
```
## Referencias

- `crates/parser/src/statements.rs`
- `crates/parser/src/expressions.rs`
- `crates/interpreter/src/interpreter.rs`
- `crates/interpreter/src/type_infer.rs`
