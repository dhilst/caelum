//! Source spans for diagnostics.
//!
//! A [`Span`] locates a slice of the original source both by byte offset (what
//! the parser works in) and by 1-based line/column (what an editor needs to draw
//! an inline marker). Editors map by line/column rather than byte offset because
//! their positions are UTF-16 units while ours are UTF-8 bytes; the two diverge
//! on the Unicode operators Caelum accepts (`∀`, `□`, `∧`, …).

/// A located slice of source text. Line and column are 1-based; `byte_start` is
/// inclusive and `byte_end` exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Span {
    pub byte_start: usize,
    pub byte_end: usize,
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl Span {
    /// Build a span from a pest match span.
    pub fn from_pest(span: pest::Span<'_>) -> Self {
        let (start_line, start_col) = span.start_pos().line_col();
        let (end_line, end_col) = span.end_pos().line_col();
        Span {
            byte_start: span.start(),
            byte_end: span.end(),
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Build a span from a pest parse *failure*, whose position pest exposes
    /// without a successful match. A single-position error becomes a zero-width
    /// span (start == end).
    pub fn from_parse_error<R>(err: &pest::error::Error<R>) -> Self {
        use pest::error::{InputLocation, LineColLocation};

        let (byte_start, byte_end) = match err.location {
            InputLocation::Pos(pos) => (pos, pos),
            InputLocation::Span((start, end)) => (start, end),
        };
        let ((start_line, start_col), (end_line, end_col)) = match err.line_col {
            LineColLocation::Pos(pos) => (pos, pos),
            LineColLocation::Span(start, end) => (start, end),
        };
        Span {
            byte_start,
            byte_end,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }
}
