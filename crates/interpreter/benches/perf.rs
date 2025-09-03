use criterion::{Criterion, criterion_group, criterion_main};
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

fn bench_parse_exec(c: &mut Criterion) {
    let src = r#"
        enum E { Ok(Int), Err(String) }
        func fib(n){ if n < 2 { return n; } return fib(n-1) + fib(n-2); }
        let x = fib(10);
        let arr = [1,2,3,4,5,6,7,8,9,10];
        let s = arr.sum() + x;
    "#;
    c.bench_function("parse+exec", |b| {
        b.iter(|| {
            let mut lx = Lexer::new(src.to_string());
            let tokens = match lx.scan_tokens() {
                Ok(t) => t,
                Err(e) => {
                    assert!(false, "lexer scan_tokens in bench perf.rs failed: {:?}", e);
                    Vec::new()
                }
            };
            let mut p = Parser::new(tokens);
            let (program, diags) = p.parse();
            assert!(diags.is_empty());
            let mut interp = Interpreter::with_prelude();
            assert!(
                interp.interpret(program).is_ok(),
                "interpret program in bench perf.rs failed"
            );
        });
    });
}

criterion_group!(benches, bench_parse_exec);
criterion_main!(benches);
