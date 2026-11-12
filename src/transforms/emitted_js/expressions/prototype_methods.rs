//! Name a built-in method by its prototype, not through a throwaway value.
//!
//! `[].push.apply(a, b)` reads `push` off a fresh array: that is
//! `Array.prototype.push`, as `{}.hasOwnProperty` is
//! `Object.prototype.hasOwnProperty` and `"".trim` is
//! `String.prototype.trim` (built-ins are taken not to change). Only the
//! throwaway value is rewritten, so the method keeps its receiver: the
//! receiver need not be an array (`arguments`, a `NodeList`, a string) and
//! `{}.hasOwnProperty.call(o, k)` stays a check that does not ask `o`.
//! The constructor's name must not be shadowed where it is written.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping};
use oxc_span::GetSpan;

use crate::analysis;

pub(crate) fn name_prototype_methods(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, edits: Vec::new() };
    scan.visit_program(&program);
    if scan.edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let named = scan.edits.len();
    let output = super::super::support::apply_checked(source, scan.edits, "prototype method")?;
    Ok((output, named))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.name(&member.object, member.span);
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if matches!(member.expression, Expression::StringLiteral(_)) {
            self.name(&member.object, member.span);
        }
        walk::walk_computed_member_expression(self, member);
    }
}

impl Scan<'_, '_> {
    fn name(&mut self, object: &Expression<'_>, member: oxc_span::Span) {
        let constructor = match object.without_parentheses() {
            Expression::ArrayExpression(array) if array.elements.is_empty() => "Array",
            Expression::ObjectExpression(object) if object.properties.is_empty() => "Object",
            Expression::StringLiteral(string) if string.value.is_empty() => "String",
            _ => return,
        };
        // The scope the member is written in: the nearest node with this span.
        let Some(node) = self.nodes.iter().find(|node| node.kind().span() == member) else {
            return;
        };
        if self.scoping.find_binding(node.scope_id(), constructor.into()).is_some() {
            return;
        }
        // `({}).x`: the parentheses go with the literal.
        let mut span = object.span();
        let (before, after) = (&self.source[..span.start as usize], &self.source[span.end as usize..]);
        if before.trim_end().ends_with('(') && after.trim_start().starts_with(')') && before.trim_end().len() >= member.start as usize {
            span = oxc_span::Span::new(before.trim_end().len() as u32 - 1, (self.source.len() - after.trim_start().len()) as u32 + 1);
        }
        self.edits.push(analysis::Edit::replace(span, format!("{constructor}.prototype")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_prototype_and_keeps_the_receiver() {
        let source = "[].push.apply(a, b.slice(0, 9));\nx = [][\"slice\"].call(arguments, 0);\nif ({}.hasOwnProperty.call(o, k)) f(({}).toString.call(d), \"\".trim.call(s));\n";
        assert_eq!(
            name_prototype_methods(source).unwrap().0,
            "Array.prototype.push.apply(a, b.slice(0, 9));\nx = Array.prototype[\"slice\"].call(arguments, 0);\nif (Object.prototype.hasOwnProperty.call(o, k)) f(Object.prototype.toString.call(d), String.prototype.trim.call(s));\n"
        );
    }

    #[test]
    fn keeps_literals_with_contents_or_a_shadowed_constructor() {
        let source = "[1].push.apply(a, b);\n({ a: 1 }).toString();\nfunction f(Array) {\n\treturn [].slice.call(Array);\n}\n";
        assert_eq!(name_prototype_methods(source).unwrap().1, 0);
    }
}
