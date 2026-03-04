# RFC Template: [Nome Curto da Proposta]

- **Feature Name:** `feature_name` (snake_case abreviado)
- **Start Date:** YYYY-MM-DD
- **RFC PR:** [artcode/rfcs#0000](https://github.com/artcode/rfcs/pull/0000)
- **Status:** Proposto / Em Andamento / Aceito / Rejeitado

## Resumo (Summary)
Um parágrafo explicando a feature/mudança para um usuário médio da linguagem. O que é e por que é útil?

## Motivação (Motivation)
Qual problema isso resolve? Por que a Artcode precisa disso? Quais são os casos de uso esperados? Este é o principal gancho para convencer sobre a adoção da funcionalidade.

## Design Detalhado (Detailed design)
A maior seção do RFC. Explique o design de forma detalhada o suficiente para:
- Revisores entenderem as implicações semânticas e o modelo de memória.
- Qualquer desenvolvedor médio da LLVM/Interpretador/Compiler possa iniciar a implementação.
- Listar exemplos de código da sintaxe proposta, AST gerado e possíveis corner cases avaliados.

## Cenários de Interoperação (Interaction and Corner Cases)
O que acontece quando essa feature interage com Atores (`run_actors_round_robin`) ou Arenas de memória? Como se comporta em grafos cíclicos? Considere a compatibilidade híbrida da linguagem.

## Alternativas (Alternatives)
Quais outras soluções de design arquitetural foram consideradas? Por que não foram escolhidas em vez dessa proposta? Qual a implicação de simplesmente *não fazer isso*?

## Questões não resolvidas (Unresolved questions)
Existem partes do design que você está deixando definidamente em aberto no momento para a contribuição da comunidade ou que precisam de benchmark pós-implantação?
