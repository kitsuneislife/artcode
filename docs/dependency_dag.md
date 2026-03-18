# DAG de Dependencias

Este guia documenta a ferramenta de ordenacao topologica para configuracoes de boot e planejamento de dependencias.

## API

Use o builtin:

```art
dag_topo_sort(nodes, deps)
```

Onde:

- `nodes`: array de strings com os ids dos nos.
- `deps`: array de tuplas `(node, depends_on)`.

A semantica de `(node, depends_on)` significa aresta `depends_on -> node`.

## Retorno

- Em grafo aciclico: retorna array com a ordem topologica.
- Em ciclo: retorna `None` e emite diagnostico de runtime.

## Exemplo

```art
let nodes = ["kernel", "drivers", "fs", "shell"];
let deps = [
    ("drivers", "kernel"),
    ("fs", "drivers"),
    ("shell", "fs"),
];

let order = dag_topo_sort(nodes, deps);
println(order);
```

## Caso com ciclo

```art
let nodes = ["a", "b"];
let deps = [("a", "b"), ("b", "a")];
let order = dag_topo_sort(nodes, deps); // None + diagnostico de ciclo
```
