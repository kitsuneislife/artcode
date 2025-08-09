# RFC 0001: Referências `weak` e `unowned` na Artcode

Status: Draft (Fase 8)
Autor: (preencher)
Data: 2025-08-09

## Objetivo
Definir semântica de referências não-fortes (weak/unowned) para quebrar ciclos sob ARC mantendo determinismo e clareza, alinhado à filosofia de Complexidade Progressiva.

## Motivação
1. Evitar ciclos de retenção em grafos de objetos (ex: padrões observador, parent/child, grafos).
2. Fornecer escolhas explícitas ao desenvolvedor em vez de GC oculto.
3. Permitir diagnósticos de ciclos em modo teste sem impacto em produção.

## Escopo
- Inclusão de dois tipos referenciais: `Weak<T>` e `Unowned<T>`.
- Modelo de leitura / queda (downgrade) e efeitos no runtime/interpreter.
- Interação com Optional / pattern matching.
- Base para ferramenta `--detect-cycles` (não detalha algoritmo, apenas interface e eventos necessários).

## Filosofia
| Tipo | Garantia | Custo | Risco quando alvo cai | Uso típico |
|------|----------|-------|-----------------------|-----------|
| Forte (default) | Mantém vivo | 1 inc/dec | Nenhum | Dono/co-dono lógico |
| Weak | Não mantém vivo | 1 inc weak | Retorna `None` | Referências ascendentes ou caches |
| Unowned | Não mantém vivo | 0 (apenas ptr) | UB em release (erro em debug) | Relações parent->child onde parent>child lifetime garantido |

## Semântica Proposta
### Criação
```
let w = weak expr;        // infere Weak<T> a partir de expr: T
let u = unowned expr;     // infere Unowned<T>
```
Desaçúcar interno para chamadas builtin `__weak(expr)` / `__unowned(expr)` durante protótipo.

### Leitura / Upgrade
```
let v = w?;        // sugar: tenta upgrade Weak -> forte; se alvo morto => None
let v = u!;        // sugar: lê Unowned; em debug checa validade; em release assume
```
Desaçúcar inicial: `weak_get(w)` e `unowned_get(u)`.

### Igualdade
Weak/Unowned comparam pelo endereço do alvo (se ambos vivos); se um morto e outro vivo => false; ambos mortos => sempre false (evita tratar dois dangling como iguais).

### Pattern Matching
`match w?` devolve `Optional<T>` então integra com já existente `Some/None`.

### Inferência de Tipos (Futuro)
Anotação explícita opcional: `let p: Weak<Node> = weak parent;`

### Display / Debug
`Weak<T>`: `<weak T alive>` ou `<weak T dropped>`.
`Unowned<T>`: `<unowned T>` (debug pode sinalizar dangling ao acesso, não no Display).

### Erros & Diagnósticos
| Cenário | Ação | Mensagem |
|---------|------|----------|
| `unowned_get` alvo morto (debug) | Erro runtime | `dangling unowned reference` |
| `weak_get` alvo morto | Retorna None | - |

### Ciclos & Detector
Detector em modo teste varre objetos fortes (roots conhecidos) construindo grafo de strong refs; componentes sem caminho a root e contendo >=1 strong edge são reportados com sugestão de aresta candidata a weak/unowned.

### Regras
1. `Unowned` exige justificativa eventual (comentário) até termos análise.
2. Proibido converter `Weak<T>` em `Unowned<T>` diretamente.
3. Forte -> Weak/Unowned permitido via açúcar ou builtins.
4. Sem promoção implícita inversa.

## Implementação Faseada
Fase 8 (mínimo):
1. Variantes em `ArtValue`: `WeakRef(id)`, `UnownedRef(id)` (ou ponteiro opaco).
2. Registro global: `HeapRegistry` (ID -> Rc<RefCell<dyn Any>>).
3. Builtins: `__weak`, `__weak_get`, `__unowned`, `__unowned_get`.
4. Parser (posterior): tokens `weak`, `unowned`, operadores pós-fixo `?` e `!`.
5. Métricas: `weak_upgrades_total`, `weak_dangling_total`, `unowned_dangling_panics`, `cycle_leaks_detected`.

## Alternativas
- GC de ciclos (rejeitado: custo e opacidade).
- Apenas Weak (menos expressivo para parent->child de alta frequência).

## Questões em Aberto
1. ID global vs wrapper homogêneo (impacta time-travel trace).
2. Representação em tracing para debugger.

## Critérios de Aceitação
- Builtins funcionam (criação/upgrade/dangling) com testes.
- Zero regressão em exemplos atuais.
- `memory.md` atualizado com semântica aprovada.

---
Feedback antes de implementar variantes em `ArtValue` é encorajado.
