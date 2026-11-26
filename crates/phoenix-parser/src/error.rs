use std::fmt;

use phoenix_ast::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    /// Formats the error with its line and column in `src`.
    pub fn render(&self, src: &str) -> String {
        let start = self.span.start.min(src.len());
        let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = src[start..].find('\n').map_or(src.len(), |i| start + i);
        let line_no = src[..start].matches('\n').count() + 1;
        let col = src[line_start..start].chars().count() + 1;
        let width = src[start..self.span.end.min(line_end).max(start)]
            .chars()
            .count()
            .max(1);

        format!(
            "error : {msg}\n --> {line_no}:{col}\n  |\n  | {line}\n  | {pad}{carets}\n",
            msg = self.message,
            line = &src[line_start..line_end],
            pad = " ".repeat(col - 1),
            carets = "^".repeat(width),
        )
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (bytes {}..{})",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for ParseError {}
