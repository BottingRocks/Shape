//! Edit spans and applying edits.

use oxc_allocator::Allocator;
use oxc_span::Span;

use crate::analysis;

/// The statement's whole line, newline included, when nothing else is on
/// it (the end of the source ends a line too); the span itself otherwise.
pub(crate) fn whole_line(source: &str, span: Span) -> Span {
    let (start, end) = (span.start as usize, span.end as usize);
    let line_start = source[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = source[end..].find('\n').map_or(source.len(), |at| end + at + 1);
    if source[line_start..start].trim().is_empty() && source[end..line_end].trim().is_empty() {
        Span::new(line_start as u32, line_end as u32)
    } else {
        span
    }
}

/// `edits` applied to `source`, which must not overlap and must leave a
/// program that parses; `what` names the pass in the error.
pub(crate) fn apply_checked(source: &str, edits: Vec<analysis::Edit>, what: &str) -> Result<String, String> {
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err(format!("{what} edits overlap"));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("{what} output: {error}"))?;
    Ok(applied.output)
}

pub(crate) fn indent_before(source: &str, start: u32) -> String {
    let start = start as usize;
    let line = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let indent = &source[line..start];
    if indent.trim().is_empty() {
        indent.to_string()
    } else {
        String::new()
    }
}

pub(crate) fn indent_at(source: &str, span: Span) -> String {
    let start = span.start as usize;
    let line = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let indent = &source[line..start];
    if indent.trim().is_empty() {
        indent.to_string()
    } else {
        String::new()
    }
}
