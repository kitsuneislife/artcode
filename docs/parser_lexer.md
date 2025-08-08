# Lexer e Parser

## Lexer
Responsável por converter código fonte em tokens.

Tokens principais: identificadores, números, strings, interpolated strings, símbolos (`+ - * / == != <= >=` etc.), palavras-chave.

### InterpolatedString
Detectado ao ver `f"`. O conteúdo interno (sem aspas) é armazenado bruto para parsing posterior.

### Números
- Inteiros ou floats (presença de ponto).

### Identificadores vs Palavras-chave
Mapa de keywords decide se o lexema vira token especial ou `Identifier`.

## Parser
Recursivo descendente. Constrói `Expr` e `Stmt`.

### Precedência
Gerenciada por enum `Precedence` e laço `parse_precedence`.

### f-strings
`TokenType::InterpolatedString` -> `parse_interpolated_string`:
1. Percorre caracteres
2. Segmenta literais e expressões `{ ... }` (com contador de profundidade)
3. Re-lexera e re-parsa cada expressão interna

### Pattern Matching
Statements `Match { expr, cases }` armazenam pares `(MatchPattern, Stmt)`.

## Limitações Atuais
- Sem recuperação de erro sofisticada (usa `panic!`)
- Não há suporte a comentários de bloco
- Tipos são apenas strings nas anotações

## Próximos Passos
| Item | Prioridade |
|------|------------|
| Erros com localização | Alta |
| Árvore de tipos dedicada | Média |
| Suporte a macros | Baixa |
