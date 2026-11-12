//! Declare a run of value-less `let`s in one statement.
//!
//! `let a;\nlet b;\nlet c;` becomes `let a, b, c;`: adjacent declarations
//! of one scope run in order either way. Only declarations without a value
//! join, so `let x = v;` keeps its own line; a comment between two breaks
//! the run.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_span::{GetSpan, Span};

use crate::analysis;

pub(crate) fn join_let_runs(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let comments: Vec<Span> = program.comments.iter().map(|comment| comment.span).collect();
    let mut scan = Scan { source, comments, edits: Vec::new() };
    super::super::support::for_each_list(&program, |list| scan.list(list));
    if scan.edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let joined = scan.edits.len();
    let output = super::super::support::apply_checked(source, scan.edits, "let run")?;
    Ok((output, joined))
}

struct Scan<'s> {
    source: &'s str,
    comments: Vec<Span>,
    edits: Vec<analysis::Edit>,
}

/// `let a, b;` with no value: its names.
fn bare<'d>(statement: &'d Statement<'_>) -> Option<Vec<&'d str>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    if declaration.kind != VariableDeclarationKind::Let || declaration.declare {
        return None;
    }
    declaration
        .declarations
        .iter()
        .map(|declarator| match (&declarator.id, &declarator.init) {
            (BindingPattern::BindingIdentifier(id), None) => Some(id.name.as_str()),
            _ => None,
        })
        .collect()
}

impl Scan<'_> {
    fn list(&mut self, statements: &[Statement<'_>]) {
        let mut index = 0;
        while index < statements.len() {
            let Some(mut names) = bare(&statements[index]) else {
                index += 1;
                continue;
            };
            let start = statements[index].span().start;
            let mut end = statements[index].span().end;
            let mut last = index;
            while let Some(next) = statements.get(last + 1) {
                let Some(more) = bare(next) else { break };
                let gap = Span::new(end, next.span().start);
                if !self.source[gap.start as usize..gap.end as usize].trim().is_empty()
                    || self.comments.iter().any(|comment| gap.start <= comment.start && comment.end <= gap.end)
                {
                    break;
                }
                names.extend(more);
                end = next.span().end;
                last += 1;
            }
            if last > index {
                self.edits.push(analysis::Edit::replace(Span::new(start, end), format!("let {};", names.join(", "))));
            }
            index = last + 1;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_adjacent_value_less_lets() {
        let source = "function f(a) {\n\tlet k1 = a;\n\tlet k2;\n\tlet k3, k4;\n\tlet k5;\n\tg();\n\tlet k6;\n\t// note\n\tlet k7;\n\tconst k8 = 1;\n\tlet k9;\n}\n";
        assert_eq!(
            join_let_runs(source).unwrap().0,
            "function f(a) {\n\tlet k1 = a;\n\tlet k2, k3, k4, k5;\n\tg();\n\tlet k6;\n\t// note\n\tlet k7;\n\tconst k8 = 1;\n\tlet k9;\n}\n"
        );
    }
}
