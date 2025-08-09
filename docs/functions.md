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

## Métodos em Structs e Enums
Sintaxe atual direta (até introdução de blocos `impl` futura):
```
struct Pessoa { nome: String, }
func Pessoa.greet(self) { println(f"Olá, {self.nome}!"); }
let p = Pessoa { nome: "Ada" };
p.greet(); // `self` é injetado automaticamente
```

Enum:
```
enum Status { Ok, Err(String) }
func Status.is_ok(self) { /* corpo */ }
let s = Status.Ok;
s.is_ok();
```

Regras:
- Primeiro parâmetro chamado exatamente `self` é removido da lista de parâmetros públicos e ligado implicitamente.
- Métodos são registrados por tipo em `TypeRegistry` no momento da definição.
- Chamada: `inst.metodo(args)` => FieldAccess produz função bound com `self` predefinido.
- Suporte tanto para variants sem payload (`Tipo.Variant`) quanto shorthand (`.Variant`).

Introspecção em enums dentro do método:
- Identificadores especiais injetados: `variant` (String) e `values` (Array dos payloads)

Limitações atuais:
- Sem agrupamento `impl {}` ainda.
- Sem sobrecarga; resolução é por nome simples.
- Checagens de campo de struct mais profundas acontecem em runtime.

Próximos passos planejados:
- Bloco `impl Tipo { }` agrupando métodos.
- Inline caching e otimizações para chamadas quentes.
- Melhor diagnóstico para redefinição de método.

## Roadmap
| Item | Descrição |
|------|-----------|
| Métodos reais | Sintaxe `impl Tipo { func metodo(...) { } }` |
| Auto-binding `self` | Implementado (forma sintaxe Tipo.metodo) |
| Inline caching | Otimizar chamadas frequentes |
| Tail-call otimization | Futuro em modo AOT |
| Anotações performáticas | Blocos `performant { ... }` futuros |
