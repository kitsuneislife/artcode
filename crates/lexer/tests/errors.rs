use lexer::lexer::Lexer;

#[test]
fn unterminated_string() {
    let mut lx = Lexer::new("\"abc".to_string());
    let err = lx.scan_tokens().err().expect("should err");
    assert!(err.message.contains("Unterminated"));
}

#[test]
fn unexpected_char() {
    let mut lx = Lexer::new("`".to_string());
    let err = lx.scan_tokens().err().expect("should err");
    assert!(err.message.contains("Unexpected"));
}
