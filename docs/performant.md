# Regras e restrições de `performant`

Este documento descreve as regras conservadoras atualmente aplicadas ao bloco `performant` em Artcode.

Resumo das regras ativas (implementadas em `crates/interpreter/src/type_infer.rs`):

- `return` não é permitido dentro de `performant`.
  - Motivo: retornar diretamente pode expor referências de objetos alocados na arena para o código externo, abrindo caminho para uso após finalização.
- Declarações de função (`func`/`function`) não são permitidas dentro de `performant`.
  - Motivo: closures podem capturar valores da arena e sobreviver à arena.
- Atribuições (`let`) com inicializadores compostos (arrays, struct-initializers, enum-initializers, chamadas) emitem erro conservador.
  - Motivo: esses inicializadores normalmente criam objetos compostos alocados no heap/arena.
- Atribuir a uma variável que existe no escopo externo (shadowing/assign-to-outer) é proibido.
  - Motivo: pode promover sem querer um objeto da arena para um escopo mais amplo.
- Inicializadores que referenciam variáveis externas (captura lexical) são proibidos; detectado conservadoramente.
  - Motivo: pode ligar um objeto do escopo externo a um valor da arena.
- Advertência heurística: bindings sem prefixo `_` dentro de `performant` geram mensagens incentivando o uso de temporários `_name`.
  - Objetivo: reduzir falsos positivos quando o desenvolvedor pretende apenas usar temporários.

Escopo da análise atual

- A análise é propositalmente conservadora: rejeita muitos casos onde a captura real pode não ocorrer. O objetivo é garantir segurança de memória antes de implementarmos uma análise completa baseada em tabelas de símbolos.
- Implementação atual:
  - snapshot das variáveis conhecidas em `TypeEnv` como `outer_vars`;
  - rastreamento de declarações locais no bloco `performant` para evitar falsos positivos;
  - verificação recursiva de expressões para encontrar usos de `Variable` que apontam para `outer_vars`.

Exemplos

- Válido (não erro de análise):
  - let _tmp = 1; // inicializador não composto
  - let _arr = make_array(); // ainda pode sinalizar se `make_array()` for uma call — atualmente `Call` é conservadoramente tratado como composto

- Inválido (rejeitado):
  - performnat { let a = [1,2,3]; }
  - performant { return x; }
  - performant { func f() { ... } }
  - performant { let a = outer_var; } // captura detectada

Próximos passos recomendados

- Implementar análise lexically-scoped symbol table para distinguir entre atribuição local vs. captura real.
- Ajustar mensagens de erro para serem mais instrutivas (sugerir alternativa segura).
- Adicionar documentação de uso ao `cli/examples/` mostrando padrões seguros.

Diagnósticos e correções rápidas

- Mensagem: "Variable 'x' initialized with a composite value inside `performant` — ensure it does not escape the block"
  - Causa: inicializador cria um valor composto (array, struct, chamada) dentro do bloco.
  - Correção: mova a construção composta para fora do `performant` ou use um temporário prefixado com `_` e garanta que ele não seja atribuído ao escopo externo.

- Mensagem: "`return` is not allowed inside `performant` blocks"
  - Causa: tentativa de retornar algo que pode conter referência de arena.
  - Correção: reestruture a função para retornar antes do `performant` ou converta `performant` em função separada que não retorna arena-refs.

- Mensagem: "Assignment to outer-scope variable '...' inside `performant` is not allowed"
  - Causa: um `let` com nome já existente no escopo externo.
  - Correção: use um binding local (por exemplo `_tmp`) ou realize a atribuição fora do `performant`.

Observação sobre testes e `unwrap()`/`panic!()`

Grande parte dos 71 locais detectados pela varredura de panics estão em testes e benches — isto é aceitável. A triagem deve priorizar ocorrências em código de produção/cli/xtask. Para testes, prefira mensagens de `expect()`s claras onde for útil.


Arquivo de implementação: `crates/interpreter/src/type_infer.rs`.

