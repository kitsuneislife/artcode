# Artcode

Artcode é uma linguagem de programação simples e moderna implementada em Rust. Este projeto demonstra a implementação completa de uma linguagem de programação, desde a análise léxica até a interpretação do código.

## Visão Geral do Projeto

Artcode é uma linguagem interpretada que oferece sintaxe limpa e recursos fundamentais de programação. O projeto é estruturado como um workspace Rust modular, separando claramente as responsabilidades de cada componente.

### Características da Linguagem

- **Tipagem simples**: Suporte para inteiros, números decimais, strings e booleanos
- **Variáveis**: Declaração com `let` e atribuição
- **Operações matemáticas**: Adição, subtração, multiplicação, divisão
- **Operações lógicas**: `and`, `or`, `not`
- **Estruturas de controle**: `if/else` com suporte a condições complexas
- **Blocos de escopo**: Definição de escopo com `{}`
- **Função de saída**: `println` para exibir resultados
- **REPL**: Modo interativo para execução de código

### Características em Desenvolvimento

- **Structs e Enums**: Tipos de dados personalizados (em implementação)
- **Pattern matching**: Correspondência de padrões com `match` (em implementação)
- **Funções**: Definição de funções personalizadas (em implementação)

## Estrutura do Projeto

```
artcode/
├── cli/                    # Interface de linha de comando
│   ├── src/main.rs        # Ponto de entrada do CLI
│   └── examples/          # Exemplos de código Artcode
├── crates/
│   ├── core/              # Funcionalidades básicas
│   ├── lexer/             # Análise léxica (tokenização)
│   ├── parser/            # Análise sintática
│   └── interpreter/       # Interpretador e execução
├── src/                   # Implementação alternativa
└── Cargo.toml            # Configuração do workspace
```

## Como Construir

```bash
# Clone o repositório
git clone https://github.com/kitsuneislife/artcode.git
cd artcode

# Compile o projeto
cargo build --release
```

## Como Usar

### Executar um arquivo

```bash
# Usar o binário compilado
./target/release/art run examples/hello.art

# Ou usando cargo
cargo run --bin art run examples/hello.art
```

### Modo interativo (REPL)

```bash
# Executar sem argumentos para modo interativo
./target/release/art
```

## Exemplos de Sintaxe

### Hello World
```artcode
println("Hello, Artcode!");
```

### Variáveis e Operações
```artcode
let a = 10;
let b = a * 2;
println(b + 5);  // Resultado: 25

let message = "O resultado é: ";
println(message);

// Suporte a diferentes tipos
let inteiro = 42;
let decimal = 3.14;
let texto = "Artcode!";
let booleano = true;

println(inteiro);
println(decimal);
println(texto);
println(booleano);
```

### Estruturas de Controle
```artcode
let idade = 18;

if (idade >= 18) {
    println("Maior de idade");
} else {
    println("Menor de idade");
}

// Operadores lógicos
let tem_carteira = true;
if (idade >= 18 and tem_carteira) {
    println("Pode dirigir!");
}

// Escopo de variáveis
let valor_global = "global";
{
    let valor_local = "local";
    println(valor_local);  // "local"
}
println(valor_global);     // "global"
```

### Operações Matemáticas
```artcode
println(10 + 2 * 3);    // 16
println((10 + 2) * 3);  // 36
println(10 / 4);        // 2
println(5 - 2 * 3);     // -1
```

### Recursos Avançados (Em Desenvolvimento)

As seguintes funcionalidades estão sendo implementadas:

### Structs e Enums (Planejado)
```artcode
// Exemplo do que será suportado no futuro
struct Pessoa {
    nome: String,
    idade: Int,
}

enum Status {
    Ok,
    Erro(String),
}
```

## Exemplos Inclusos

O projeto inclui vários exemplos na pasta `cli/examples/`:

### Funcionando Atualmente:
- `hello.art` - Hello World básico
- `variables.art` - Declaração e uso de variáveis  
- `math.art` - Operações matemáticas
- `control_flow.art` - Estruturas condicionais e escopo
- `numbers.art` - Diferentes tipos numéricos
- `artvalue.art` - Tipos básicos suportados

### Em Desenvolvimento:
- `struct_enum.art` - Estruturas de dados personalizadas (em implementação)
- `match.art` - Pattern matching (em implementação)
- `result.art` - Sistema de tratamento de erros (planejado)
- `result_propagation.art` - Propagação de erros (planejado)
- `arr_opt.art` - Arrays e tipos opcionais (planejado)

## Arquitetura Técnica

### Fluxo de Execução

1. **Lexer**: Converte o código fonte em tokens
2. **Parser**: Analisa os tokens e constrói uma Árvore de Sintaxe Abstrata (AST)
3. **Interpreter**: Executa a AST e produz os resultados

### Componentes

- **Lexer** (`crates/lexer`): Análise léxica e tokenização
- **Parser** (`crates/parser`): Análise sintática e construção da AST
- **Interpreter** (`crates/interpreter`): Execução do código e gerenciamento de estado
- **CLI** (`cli`): Interface de linha de comando com suporte a REPL

## Desenvolvimento

### Executar Testes
```bash
cargo test
```

### Verificar Formatação
```bash
cargo fmt
```

### Análise Estática
```bash
cargo clippy
```

## Contribuição

Este projeto demonstra conceitos fundamentais de implementação de linguagens de programação:

- Análise léxica e sintática
- Construção de AST
- Interpretação de código
- Gerenciamento de escopo
- Sistema de tipos básico

## Tecnologias Utilizadas

- **Rust**: Linguagem de implementação
- **Clap**: Interface de linha de comando
- **Workspace Cargo**: Gerenciamento modular do projeto

## Status do Projeto

O projeto está em desenvolvimento ativo e implementa as funcionalidades básicas de uma linguagem de programação interpretada:

### ✅ Implementado:
- Análise léxica completa
- Parser para expressões e declarações básicas
- Interpretador funcional
- Variáveis e tipos básicos (int, float, string, boolean)
- Operações matemáticas e lógicas
- Estruturas de controle (if/else)
- Blocos de escopo
- Interface de linha de comando com REPL

### 🚧 Em Desenvolvimento:
- Structs e enums personalizados
- Pattern matching com match
- Sistema de funções
- Arrays e coleções
- Sistema de tratamento de erros

### 📋 Planejado:
- Módulos e imports
- Closures
- Iteradores
- Melhor sistema de tipos