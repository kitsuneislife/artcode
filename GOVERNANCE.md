# Governança do Projeto Artcode

Este documento descreve um processo mínimo de governança para decisões de design e contribuição.

Principles:
- Transparência: decisões registradas em RFCs e `docs/decisions/`.
- Meritocracia orientada a contribuição: decisões técnicas lideradas por autores de RFCs e revisores.

Roles:
- Maintainers: revisam PRs críticos, aprovam merges em `main`.
- Authors: quem propõe RFCs/PRs.
- Reviewers: participantes com histórico demonstrado.

Decision flow:
1. Criar issue com proposta de alto nível.
2. Preencher RFC em `docs/rfcs/` usando `0000-template.md` (Draft).
3. Discutir na issue; iterar RFC até consenso mínimo (thread com +1/+discussion).
4. Quando pronto, abrir PR implementando o MVP e referenciar a RFC.
5. Pelo menos 2 approvals de maintainers ou 1 maintainer + 2 reviewers para mudanças de linguagem/semântica.
6. Se houver desacordo persistente, abrir votação por escrito entre maintainers (48h) e documentar resultado em `docs/decisions/`.

Conflict of interest:
- Membros devem declarar conflito em PR/issue onde apropriado.

Emergency changes:
- Correções de segurança/bug críticos podem ser justificadas e revertidas posteriormente via RFC retrospectiva.

Maintaining this document:
- Atualize este arquivo via PR; mudanças processuais simples exigem 1 approval de maintainer.
