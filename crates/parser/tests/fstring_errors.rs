use lexer::lexer::Lexer;
use parser::parser::Parser;

fn parse(src: &str) -> Vec<diagnostics::Diagnostic> {
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().unwrap();
    let mut p = Parser::new(tokens);
    let (_program, diags) = p.parse();
    diags
}

#[test]
fn unmatched_right_brace_in_fstring() {
    let diags = parse("println(f\"a}\");");
    assert!(diags.iter().any(|d| d.message.contains("Unmatched '}'")));
}

#[test]
fn unterminated_expr_in_fstring() {
    let diags = parse("println(f\"a={1 + {2}\");");
    assert!(diags.iter().any(|d| d.message.contains("Unterminated")));
}

#[test]
fn fstring_escaped_braces() {
    let diags = parse("println(f\"x={{1}}\");");
    assert!(diags.is_empty());
}

#[test]
fn fstring_deep_nesting() {
    // nested three levels
    let diags = parse("println(f\"v={ { {1} } }\");");
    assert!(diags.is_empty());
}
