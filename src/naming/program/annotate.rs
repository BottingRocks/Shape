//! Signal comment insertion at statement boundaries.
//!
//! Comments are inserted as whole-line `//` comments immediately before an
//! existing statement. Because a line comment never merges tokens and the
//! insertion always ends with a newline followed by the statement's own
//! indentation, it cannot change ASI or the meaning of a single-statement
//! branch. Any comment text is escaped so it cannot terminate the comment or
//! open a new token, and a comment already present before the statement is
//! left alone so the pass is idempotent.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::{Program, Statement};
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};

use crate::analysis;

use super::{
    AnnotationOutcome, AnnotationSkipReason, DiagnosticCode, ProgramTransformDiagnostic,
    SignalAnnotation,
};

pub(super) struct AnnotationPlan {
    pub edits: Vec<analysis::Edit>,
    pub outcomes: Vec<AnnotationOutcome>,
    pub diagnostics: Vec<ProgramTransformDiagnostic>,
}

pub(super) fn plan_annotations(
    source: &str,
    program: &Program,
    annotations: &[SignalAnnotation],
    enabled: bool,
) -> AnnotationPlan {
    let mut outcomes: Vec<AnnotationOutcome> = annotations
        .iter()
        .map(|annotation| AnnotationOutcome {
            statement_start: annotation.statement_start,
            text: annotation.text.clone(),
            applied: false,
            skip_reason: None,
        })
        .collect();
    let mut plan = AnnotationPlan {
        edits: Vec::new(),
        outcomes: Vec::new(),
        diagnostics: Vec::new(),
    };
    if annotations.is_empty() {
        return plan;
    }

    let starts = statement_starts(program);
    let mut pending: Vec<usize> = Vec::new();

    for index in 0..annotations.len() {
        let annotation = &annotations[index];
        if !enabled {
            outcomes[index].skip_reason = Some(AnnotationSkipReason::Disabled);
            continue;
        }
        let text = sanitize(&annotation.text);
        outcomes[index].text = text.clone();
        if text.is_empty() {
            outcomes[index].skip_reason = Some(AnnotationSkipReason::EmptyText);
            plan.diagnostics.push(diagnostic(
                DiagnosticCode::EmptyAnnotation,
                format!(
                    "annotation text at byte {} is empty after escaping",
                    annotation.statement_start
                ),
                Some((annotation.statement_start, annotation.statement_start)),
            ));
            continue;
        }
        if !starts.contains(&annotation.statement_start) {
            outcomes[index].skip_reason = Some(AnnotationSkipReason::NotAStatementBoundary);
            plan.diagnostics.push(diagnostic(
                DiagnosticCode::NotAStatementBoundary,
                format!(
                    "byte {} is not the start of a parsed statement",
                    annotation.statement_start
                ),
                Some((annotation.statement_start, annotation.statement_start)),
            ));
            continue;
        }
        let line = format!("// {text}");
        if comment_line_present(source, annotation.statement_start as usize, &line) {
            outcomes[index].skip_reason = Some(AnnotationSkipReason::AlreadyPresent);
            continue;
        }
        pending.push(index);
    }

    let mut by_offset: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for &index in &pending {
        by_offset
            .entry(annotations[index].statement_start)
            .or_default()
            .push(index);
    }

    for (offset, indices) in by_offset {
        let indent = indent_at(source, offset as usize);
        let mut content = String::new();
        let mut emitted: Vec<&str> = Vec::new();
        for &index in &indices {
            let text = outcomes[index].text.as_str();
            if !emitted.contains(&text) {
                content.push_str("// ");
                content.push_str(text);
                content.push('\n');
                content.push_str(indent);
                emitted.push(text);
            }
        }
        plan.edits
            .push(analysis::Edit::replace(Span::new(offset, offset), content));
        for &index in &indices {
            outcomes[index].applied = true;
        }
    }

    plan.outcomes = outcomes;
    plan
}

/// All statement start offsets in the program.
fn statement_starts(program: &Program) -> BTreeSet<u32> {
    struct Collect {
        starts: BTreeSet<u32>,
    }
    impl<'a> Visit<'a> for Collect {
        fn visit_statement(&mut self, statement: &Statement<'a>) {
            self.starts.insert(statement.span().start);
            walk::walk_statement(self, statement);
        }
    }
    let mut collect = Collect {
        starts: BTreeSet::new(),
    };
    collect.visit_program(program);
    collect.starts
}

/// The whitespace prefix of the line that contains `offset`, or an empty
/// string when the statement does not begin a line.
fn indent_at(source: &str, offset: usize) -> &str {
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &source[line_start..offset];
    if prefix
        .chars()
        .all(|character| character == ' ' || character == '\t')
    {
        prefix
    } else {
        ""
    }
}

/// Is `line` already the last comment line before `offset`?
fn comment_line_present(source: &str, offset: usize, line: &str) -> bool {
    let Some(before) = source.get(..offset) else {
        return false;
    };
    let mut started = false;
    for candidate in before.lines().rev() {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            if started {
                break;
            }
            continue;
        }
        if trimmed.starts_with("//") {
            started = true;
            if trimmed == line {
                return true;
            }
        } else {
            break;
        }
    }
    false
}

/// Escape comment text into a single safe comment body: line separators and
/// control characters become spaces so the text cannot end the comment early.
fn sanitize(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\n' | '\r' | '\u{2028}' | '\u{2029}' => escaped.push(' '),
            character if character.is_control() => escaped.push(' '),
            character => escaped.push(character),
        }
    }
    escaped.trim().to_string()
}

fn diagnostic(
    code: DiagnosticCode,
    message: String,
    span: Option<(u32, u32)>,
) -> ProgramTransformDiagnostic {
    ProgramTransformDiagnostic {
        code,
        message,
        span,
    }
}
