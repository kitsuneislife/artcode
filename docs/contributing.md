# Contribuindo

## Princípios
1. Clareza antes de abstração
2. Evolução incremental e testada
3. Determinismo e previsibilidade

## Passos para PR
1. Abra uma issue descrevendo motivação (ou RFC para mudanças de linguagem)
2. Sincronize branch `main`
3. Escreva/atualize testes (integração no crate `interpreter`)
4. Atualize documentação em `docs/` se aplicável
5. `cargo test` deve passar sem panics inesperados
6. Descreva trade-offs no corpo do PR

## Estilo de Código
- Preferir nomes explícitos (sem abreviações crípticas)
- Evitar unwrap/expect em código de produção (exceto protótipo sinalizado)
- Panics temporários devem ter TODO de conversão em erro estruturado

## Estrutura de Testes
- `crates/interpreter/tests` para cenários de linguagem
- Futuro: adicionar `crates/parser/tests` para erros sintáticos

## RFCs
Inclua: motivação, design detalhado, exemplos de código, impacto em runtime, riscos, plano de migração.

## Revisão
Critérios:
- Correção sem quebrar exemplos existentes
- Cobertura de casos de erro
- Legibilidade e comentários onde lógica é sutil
- Documentação atualizada

## Anti-Patterns
| Padrão | Alternativa |
|--------|-------------|
| Função gigante monolítica | Extrair helpers nomeados |
| Duplicação de lógica de parsing | Compartilhar utilitários | 
| Panics silenciosos | Erros estruturados + mensagens claras |

## Comunicação
Use linguagem inclusiva e objetiva. Debates técnicos com foco em fatos/benchmark.
