# RFC 0003: Generics and Monomorphization

## 1. Summary
This RFC proposes the addition of generic functions to Artcode, allowing the same function logic to operate on multiple data types. The proposed approach relies on **monomorphization** at runtime (JIT/Interpreter level), generating a specific function copy for each concrete type combination used.

## 2. Motivation
Currently, Artcode functions require explicitly setting the types of parameters and return values. If a developer wants to write a `map` or `filter` function, they must duplicate the logic for `[Int]`, `[Float]`, `[String]`, etc. Generics remove this boilerplate, promoting code reuse, type safety, and better library design.

## 3. Proposed Syntax

### 3.1 Function Declarations
Generic parameters will be defined inside angle brackets `<...>` immediately following the function name:

```rust
func map<T, U>(arr: [T], f: func(T) -> U) -> [U] {
    let out = [];
    for item in arr {
        out.push(f(item));
    }
    return out;
}
```

### 3.2 Constraints (Future-proofing)
Optionally, type boundaries can be specified using a colon `:` syntax inside the type parameter list:

```rust
func add<T: Numeric>(a: T, b: T) -> T {
    return a + b;
}
```

### 3.3 Function Calls and Instantiation
In most cases, the compiler/interpreter should deduce the types `<T, U>` from the arguments passed.

```rust
let numbers = [1, 2, 3];
// T = Int, U = String (deduced from the lambda)
let strings = map(numbers, |x| x.to_string()); 
```

Explicit type arguments can be provided using the turbofish-like syntax if type deduction fails or is ambiguous:

```rust
let parsed = parse::<Int>("123");
```

## 4. Implementation Details

### 4.1 Parser and AST
- Update `crates/parser/src/lib.rs` to recognize `<T>` in `func name<T>(...)`.
- Add `type_params: Vec<String>` to `Stmt::Function`.
- Update expression parsing to recognize explicit type arguments in calls, e.g., `Expr::Call { callee, type_args: Option<Vec<Type>>, args }`.

### 4.2 Type Checking & Inferencing
- During `infer_expr` or initial binding, register the generic function in the `TypeRegistry` or `Environment` with its uninstantiated AST body.
- When a generic function is called, perform type unification between the expected parameters and the provided argument types to deduce `T`, `U`, etc.

### 4.3 Monomorphization (Interpreter Backend)
Artcode is currently interpreted (with a JIT in progress). For generics:
1. When a generic function is invoked with a new set of concrete types (e.g., `T=Int`), the interpreter checks a cache (`monomorphized_funcs`) for this combination.
2. If not found, it clones the generic AST and instantiates a new concrete function where `T` is replaced by `Int`.
3. The new monomorphized function is then executed. 
4. This ensures that field accesses and type-specific operations within the generic body are strictly validated against the concrete type.

## 5. Drawbacks and Limitations
- **Code Bloat:** Monomorphization creates multiple copies of the same function. While this is standard in Rust/C++, it could lead to higher memory usage in the Artcode interpreter.
- **Complexity:** Type deduction unification can be complex, especially with higher-order functions and nested generics.

## 6. Alternatives Considered
- **Type Erasure (Java-style):** Storing everything as a generic `Any` type at runtime and casting. Rejected because Artcode values are strongly typed, and monomorphization plays better with the eventual JIT compiler for unboxed performance.

## 7. Unresolved Questions
- Should we allow generic structs and enums in this phase, or restrict generics to functions only? (Proposed: Functions only for Phase 18, expand to Structs/Enums later).
- How strict should type deduction be when encountering `Any` vs a concrete type?
