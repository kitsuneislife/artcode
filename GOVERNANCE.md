# Governanca do Projeto Artcode

Este documento define o processo oficial de governanca do Artcode para decisoes tecnicas,
evolucao da linguagem e contribuicoes estruturais.

## Objetivos
- Garantir decisoes auditaveis e rastreaveis.
- Preservar alinhamento com a filosofia de Complexidade Progressiva.
- Equilibrar velocidade de entrega com qualidade tecnica e estabilidade.

## Principios
- Transparencia: decisoes e trade-offs devem ser publicos e registrados.
- Determinismo: evitar mudancas opacas, implicitas ou de comportamento magico.
- Empoderamento do desenvolvedor: mecanismos explicitos, com contratos claros.
- Sustentabilidade: toda decisao deve considerar custo de manutencao e evolucao.

## Papeis
- Maintainers:
	- Revisam e aprovam PRs para `main`.
	- Definem prioridade de roadmap e resolvem impasses finais.
	- Respondem por estabilidade de API e direcao tecnica.
- Authors:
	- Propoem RFCs, implementam mudancas e documentam trade-offs.
	- Mantem consistencia entre codigo, docs e testes da feature proposta.
- Reviewers:
	- Revisam RFCs/PRs, questionam riscos e validam coerencia tecnica.
	- Podem aprovar mudancas dentro do escopo de sua experiencia.

## Escopo de Decisoes
Mudancas abaixo exigem RFC formal antes de merge:
- Semantica da linguagem, tipagem e parser/lexer.
- Contratos publicos de stdlib e comportamento de runtime.
- Arquitetura JIT/AOT, IR e interfaces de FFI.
- Mudancas que alterem ergonomia central da linguagem.

Mudancas pequenas (fixes locais de bug, refactor sem alteracao de comportamento,
ajustes de docs) podem seguir via PR direta.

## Fluxo Oficial (RFC -> ADR -> Implementacao)
1. Abrir issue de contexto com problema, impacto e objetivo.
2. Criar RFC em `docs/rfcs/` a partir de `0000-template.md`.
3. Discutir alternativas, riscos e plano de rollout ate consenso.
4. Aprovar RFC e abrir PR de implementacao referenciando RFC.
5. Para decisoes arquiteturais relevantes, registrar ADR em `docs/decisions/`.
6. Atualizar docs, exemplos e checklist operacional da iteracao.

## Regras de Aprovacao
- Mudancas estruturais (linguagem/runtime/contrato publico):
	- minimo de 2 approvals de maintainers, ou
	- 1 maintainer + 2 reviewers experientes.
- Mudancas nao estruturais:
	- 1 maintainer.
- PR com feedback de bloqueio deve responder cada ponto antes de merge.

## Resolucao de Impasses
Quando houver desacordo persistente:
1. Registrar os pontos de discordancia na issue/RFC.
2. Abrir votacao escrita entre maintainers por 48h.
3. Registrar resultado e racional em ADR ou comentario final da RFC.

## Conflito de Interesse
- Qualquer participante deve declarar conflito de interesse em issue/PR.
- Em caso de conflito direto, a aprovacao final deve vir de maintainers sem conflito.

## Mudancas Emergenciais
- Correcoes criticas de seguranca ou estabilidade podem ser aplicadas sem RFC previa.
- Toda mudanca emergencial deve gerar RFC retrospectiva em ate 7 dias.

## Versionamento e Compatibilidade
- Mudancas breaking devem informar impacto e plano de migracao na RFC.
- A politica de compatibilidade publica deve ser seguida conforme roadmap da v0.2.0.

## Manutencao deste Documento
- Alteracoes neste arquivo devem vir por PR e descricao objetiva do motivo.
- Mudancas processuais simples: 1 approval de maintainer.
- Mudancas de regra de aprovacao/quorum: 2 approvals de maintainers.
