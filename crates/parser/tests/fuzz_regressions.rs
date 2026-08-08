//! Inputs found by `Fuzz CI`, kept as permanent regressions.
//!
//! Each case is the exact artifact libFuzzer wrote when it stopped, replayed
//! through the same path the fuzz target takes: `String::from_utf8_lossy`, then
//! lex, then parse. Bytes are embedded rather than read from `fuzz/corpus` so
//! the coverage does not depend on the corpus surviving a checkout.

use lexer::Lexer;
use parser::Parser;

/// Runs the harness pipeline and returns how many statements came out.
///
/// The assertion is simply that this returns at all: every input here used to
/// abort or hang.
fn lex_and_parse(data: &[u8]) -> usize {
    let input = String::from_utf8_lossy(data).to_string();
    let mut lexer = Lexer::new(input);
    let Ok(tokens) = lexer.scan_tokens() else {
        return 0;
    };
    let (program, _diags) = Parser::new(tokens).parse();
    program.len()
}

/// `Parser::previous` computed `self.current - 1` without a guard, so any rule
/// reaching it before advancing underflowed: a panic in debug, an out-of-bounds
/// index in release. Artifact
/// `crash-dc07dc22a0c49e50d3731b78e432bcad858803f6`, from `interpreter_valid`.
#[test]
fn previous_token_at_position_zero_does_not_underflow() {
    const CRASH: &[u8] = &[
        0x5b, 0x66, 0x22, 0x00, 0x00, 0x00, 0x30, 0x00, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x5b,
        0x01, 0x69, 0x69, 0x69, 0x69, 0x69, 0x69, 0x69, 0x69, 0x69, 0xff, 0xff, 0xff, 0x23, 0xff,
        0x70, 0x63, 0x7b, 0x7d, 0x98, 0x8f, 0x9c, 0x84, 0x8b, 0x9c, 0x84, 0x82, 0x68, 0x70, 0x63,
        0x74, 0x63, 0x7b, 0x7d, 0x68, 0x70, 0x63, 0x7b, 0x3d, 0x00, 0x00, 0x00, 0x6e, 0x6e, 0x6e,
        0x6e, 0x6e, 0x6e, 0x6e, 0xff, 0xff, 0xff, 0xff, 0xff, 0x25, 0x3c, 0x5b, 0xa1, 0xd7, 0x2e,
        0x2e, 0xd3, 0xc3, 0x2e, 0x74, 0x7d, 0xb1, 0x5b, 0x22, 0x30, 0x7b, 0x28, 0x41,
    ];

    lex_and_parse(CRASH);
}

/// Artifact `timeout-ccbf9148929201e8ab4bcce4f187e2e4867a1a4f`, from
/// `parser_loops`: a `while` with no condition, no body and a trailing `=`.
#[test]
fn malformed_while_header_terminates() {
    lex_and_parse(b"while! !!!=");
}

/// An empty token stream reaches `peek` before anything else. `Parser::new`
/// appends the terminator so that first look-ahead has something to read.
#[test]
fn empty_input_parses_without_panicking() {
    assert_eq!(lex_and_parse(b""), 0);
}
