use core::TokenType;
use lexer::lexer::Lexer;

#[test]
fn lexes_dollar_for_shell_statement() {
    let mut lx = Lexer::new("$ echo hello".to_string());
    let tokens = lx.scan_tokens().expect("lexer should succeed");
    assert!(matches!(tokens[0].token_type, TokenType::Dollar));
}
