//! Collapse whitespace inside array expressions only. Literal and comment bytes
//! are protected; reparsing must reproduce the identical canonical AST print.
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;
use std::collections::BTreeSet;

pub fn compact_arrays(source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    #[derive(Default)]
    struct Scan {
        arrays: Vec<Span>,
        protected: Vec<Span>,
        brackets: BTreeSet<u32>,
    }
    impl<'a> Visit<'a> for Scan {
        fn visit_array_expression(&mut self, array: &ArrayExpression<'a>) {
            self.arrays.push(array.span);
            self.brackets.insert(array.span.start);
            self.brackets.insert(array.span.end - 1);
            walk::walk_array_expression(self, array);
        }
        fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
            self.protected.push(literal.span);
        }
        fn visit_reg_exp_literal(&mut self, literal: &RegExpLiteral<'a>) {
            self.protected.push(literal.span);
        }
        fn visit_template_element(&mut self, literal: &TemplateElement<'a>) {
            self.protected.push(literal.span);
        }
    }
    let mut scan = Scan::default();
    scan.visit_program(&program);
    if scan.arrays.is_empty() {
        return Ok(source.into());
    }
    for comment in &program.comments {
        let mut span = comment.span;
        // The newline after // terminates the comment. Keep that separator.
        if source[span.start as usize..].starts_with("//") {
            if let Some(end) = source[span.end as usize..].find('\n') {
                span.end += end as u32 + 1;
            }
        }
        scan.protected.push(span);
    }
    scan.protected.sort_unstable_by_key(|span| span.start);
    scan.arrays
        .sort_unstable_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
    let mut edits = vec![];
    let mut end = 0;
    for span in scan.arrays {
        if span.start < end {
            continue;
        }
        end = span.end;
        let mut output = Vec::new();
        let mut at = span.start as usize;
        let mut protected = scan.protected.partition_point(|s| s.end <= span.start);
        while at < span.end as usize {
            while protected < scan.protected.len() && scan.protected[protected].end as usize <= at {
                protected += 1;
            }
            if let Some(literal) = scan
                .protected
                .get(protected)
                .filter(|s| s.start as usize <= at)
            {
                let stop = (literal.end as usize).min(span.end as usize);
                output.extend_from_slice(&source.as_bytes()[at..stop]);
                at = stop;
                continue;
            }
            if source.as_bytes()[at].is_ascii_whitespace() {
                let start = at;
                while at < span.end as usize
                    && source.as_bytes()[at].is_ascii_whitespace()
                    && scan
                        .protected
                        .get(protected)
                        .is_none_or(|s| at < (s.start as usize))
                {
                    at += 1;
                }
                let after_open = scan.brackets.contains(&(start as u32 - 1))
                    && source.as_bytes()[start - 1] == b'[';
                let before_close =
                    scan.brackets.contains(&(at as u32)) && source.as_bytes()[at] == b']';
                if !after_open && !before_close {
                    output.push(b' ');
                }
            } else {
                output.push(source.as_bytes()[at]);
                at += 1;
            }
        }
        let output = String::from_utf8(output).map_err(|e| format!("array formatting: {e}"))?;
        if output != source[span.start as usize..span.end as usize] {
            edits.push(analysis::Edit::replace(span, output));
        }
    }
    if edits.is_empty() {
        return Ok(source.into());
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("array formatting edits overlap".into());
    }
    let check_allocator = Allocator::default();
    let checked = analysis::parse_program(&check_allocator, &applied.output)?;
    // This also protects ASI/return boundaries in function-valued elements.
    if analysis::codegen(&checked) != analysis::codegen(&program) {
        return Ok(source.into());
    }
    Ok(applied.output)
}
