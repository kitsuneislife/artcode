use criterion::{Criterion, criterion_group, criterion_main};
use lexer::Lexer;
use parser::Parser;

fn bench_lex_parse(c: &mut Criterion) {
    let src = r#"
        enum E { Ok(Int), Err(String) }
        func fib(n){ if n < 2 { return n; } return fib(n-1) + fib(n-2); }
        let x = fib(12);
        let arr = [1,2,3,4,5,6,7,8,9,10];
        let s = arr.sum() + x;
    "#;
    c.bench_function("lex+parse", |b| {
        b.iter(|| {
            let mut lx = Lexer::new(src.to_string());
            let tokens = lx.scan_tokens().unwrap();
            let mut p = Parser::new(tokens);
            let (program, diags) = p.parse();
            assert!(diags.is_empty());
            std::mem::drop(program);
        });
    });
}

criterion_group!(benches, bench_lex_parse);
criterion_main!(benches);
