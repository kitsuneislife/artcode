use core::TokenType;
use lexer::lexer::Lexer;

#[test]
fn lexes_yield_keyword() {
    let mut lx = Lexer::new("yield 1;".to_string());
    let tokens = lx.scan_tokens().expect("lexer should succeed");
    assert!(matches!(tokens[0].token_type, TokenType::Yield));
}
