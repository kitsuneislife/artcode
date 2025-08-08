# Funções e Escopos

## Declaração
```
func soma(a: Int, b: Int) -> Int {
    return a + b
}
```
- Tipos de parâmetros opcionais (ausentes => não checados por agora).
- `return` opcional; ausência implica `none`.

## Closures (Modelo Atual)
Cada função captura o ambiente no momento da definição (Rc<RefCell<Environment>>). Ao chamar, novo ambiente filho é criado apontando para a closure.

## Ordem de Avaliação
Argumentos são avaliados antes da troca de ambiente (corrige bug onde variáveis externas sumiam).

## Exemplo de Captura
```
let x = 5
func inc(n) { return n + x }
println(inc(10)) // 15
```

## Fallback de Field Access
`arr.sum()` é parseado como `Call(FieldAccess(arr, sum), [])`. Se o resultado de `FieldAccess` não for chamável e não houver argumentos, o interpretador retorna o valor direto (permitindo pseudo-métodos  sem implementar sistema de métodos ainda).

## Roadmap
| Item | Descrição |
|------|-----------|
| Métodos reais | Sintaxe `impl Tipo { func metodo(...) { } }` |
| Inline caching | Otimizar chamadas frequentes |
| Tail-call otimization | Futuro em modo AOT |
| Anotações performáticas | Blocos `performant { ... }` futuros |
