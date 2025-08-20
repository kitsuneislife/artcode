#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, col: usize) -> Self {
        Self {
            start,
            end,
            line,
            col,
        }
    }
    pub fn dummy() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 0,
            col: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    Lex,
    Parse,
    Type,
    Runtime,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub span: Span,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn new(kind: DiagnosticKind, message: impl Into<String>, span: Span) -> Self {
        Self {
            kind,
            message: message.into(),
            span,
            notes: vec![],
        }
    }
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
    pub fn simple(kind: DiagnosticKind, span: Span, message: impl Into<String>) -> Self {
        Self::new(kind, message, span)
    }
}

pub type DiagResult<T> = Result<T, Diagnostic>;

pub fn format_diagnostic(source: &str, d: &Diagnostic) -> String {
    let mut out = String::new();
    let line_idx = if d.span.line > 0 { d.span.line - 1 } else { 0 };
    let line_opt = source.lines().nth(line_idx);
    use std::fmt::Write;
    let kind_str = match d.kind {
        DiagnosticKind::Lex => "lex",
        DiagnosticKind::Parse => "parse",
        DiagnosticKind::Type => "type",
        DiagnosticKind::Runtime => "runtime",
        DiagnosticKind::Internal => "internal",
    };
    let _ = writeln!(
        out,
        "{} error ({}:{}): {}",
        kind_str, d.span.line, d.span.col, d.message
    );
    if let Some(line_text) = line_opt {
        let _ = writeln!(out, "{}", line_text);
        let col = if d.span.col == 0 { 1 } else { d.span.col };
        let underline_len = std::cmp::max(1, d.span.end.saturating_sub(d.span.start));
        let caret_line: String = (1..col).map(|_| ' ').collect::<String>()
            + "^"
            + &"~".repeat(underline_len.saturating_sub(1));
        let _ = writeln!(out, "{}", caret_line);
    }
    for note in &d.notes {
        let _ = writeln!(out, "note: {}", note);
    }
    out
}

#[inline]
pub fn error(kind: DiagnosticKind, span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(kind, message, span)
}
