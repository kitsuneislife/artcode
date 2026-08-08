//! Guards the interner against unbounded growth in a long-lived process.
//!
//! Interning used to `Box::leak` every distinct symbol into a permanent global
//! pool. That is defensible for `art run script.art`, which exits, and wrong
//! for `art lsp`, which lives for the whole editing session and re-lexes the
//! file on every keystroke — every partial identifier ever typed was leaked and
//! never reclaimed.
//!
//! The same shape is what made `Fuzz CI` time out: libFuzzer reuses one process
//! across iterations, so a pool that grows with input rather than with the
//! program's vocabulary degrades throughput until an arbitrary input trips the
//! timeout.
//!
//! These phases live in a single `#[test]` on purpose. The pool is process-wide
//! global state, so separate test functions observing it would run concurrently
//! and read each other's interning as growth.

use interpreter::interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

/// Runs one lex + parse pass, discarding the result.
fn lex_and_parse(source: String) {
    let mut lexer = Lexer::new(source);
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(_) => return,
    };
    let _ = Parser::new(tokens).parse();
}

/// Runs the full lex + parse + interpret pipeline, discarding the result.
///
/// The interpret step matters independently: `Environment::define` allocates
/// a key per binding name, so a leak there would grow with the programs
/// executed rather than with the source of any one of them.
fn run_pipeline(source: String) {
    let mut lexer = Lexer::new(source);
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(_) => return,
    };
    let (program, _diags) = Parser::new(tokens).parse();
    let mut interp = Interpreter::with_prelude();
    let _ = interp.interpret(program);
}

/// Re-lexes a source file the way an editor does: one pass per keystroke, with
/// the identifier under the cursor growing a character at a time.
fn simulate_typing(identifier: &str) {
    for len in 1..=identifier.len() {
        let partial = &identifier[..len];
        lex_and_parse(format!(
            "func demo() {{ let {} = 1; return {}; }}",
            partial, partial
        ));
    }
}

#[test]
fn interner_tracks_vocabulary_not_input() {
    // ── Phase 1: an unchanged file, re-parsed ────────────────────────────
    let source = r#"
        func fib(n) {
            if n < 2 { return n; }
            return fib(n - 1) + fib(n - 2);
        }
        let result = fib(10);
    "#;

    // Warm up so the file's own vocabulary is already interned.
    for _ in 0..8 {
        lex_and_parse(source.to_string());
    }

    let steady = core::sweep_interned();
    for _ in 0..200 {
        lex_and_parse(source.to_string());
    }

    assert_eq!(
        core::sweep_interned(),
        steady,
        "re-parsing an unchanged file must not add symbols; an editor does this \
         on every keystroke"
    );

    // ── Phase 2: throwaway identifiers, as typed ─────────────────────────
    let before = core::sweep_interned();

    // 40 identifiers of ~23 characters: roughly 900 distinct prefixes that a
    // leaking interner retains for the lifetime of the process.
    for i in 0..40 {
        simulate_typing(&format!("some_long_identifier_{:03}", i));
    }

    let growth = core::sweep_interned().saturating_sub(before);
    assert!(
        growth <= 64,
        "interner grew by {} entries while lexing throwaway identifiers. \
         The pool must track the language's vocabulary, not the input: every \
         entry is a Box::leak that outlives the process.",
        growth
    );

    // ── Phase 3: whole programs executed, as a fuzzer or REPL does ───────
    //
    // Every iteration binds differently-named variables. This is the shape
    // that degrades `Fuzz CI`: libFuzzer keeps one process alive across
    // iterations, so anything retained per-input accumulates without bound.
    let before = core::sweep_interned();

    for i in 0..120 {
        run_pipeline(format!(
            "let var_{i}_a = {i}; let var_{i}_b = var_{i}_a + 1; let var_{i}_c = var_{i}_b * 2;"
        ));
    }

    let growth = core::sweep_interned().saturating_sub(before);
    assert!(
        growth <= 64,
        "interner grew by {} entries while running 120 throwaway programs. \
         Binding names must not accumulate across independent executions.",
        growth
    );
}
