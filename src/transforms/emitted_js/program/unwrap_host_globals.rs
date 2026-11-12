//! Replace `host["name"]` with the global `name`.
//!
//! `host` is the global object the emitted program closes over
//! (`tracing::symbolic::emit::HOST`), so reading `host["name"]` reads the global
//! `name`. The pass rewrites a computed member whose object is the free name
//! `host` and whose key is an identifier-shaped string literal into that
//! identifier, in place: `host["Math"]["floor"]` becomes `Math["floor"]` and
//! `new host["TypeError"]()` becomes `new TypeError()`.
//!
//! It leaves a member that is the direct callee or tag of a call, or an
//! assignment, update, `delete`, or `for`-`in`/`of` target, because those
//! positions read the receiver or write through `host` itself and a bare name
//! would change the receiver or the target. It leaves a key that is not an
//! identifier or is a reserved word, and one whose name a binding holds where
//! it is read (`function f(document) { host["document"] }`), where the bare
//! name would be that binding. If `host` is a local binding anywhere in the
//! program the whole pass declines, because the free name is then not the
//! global object.

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping};
use oxc_span::{GetSpan, Span};
use oxc_syntax::keyword::is_reserved_keyword;

use crate::analysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostGlobalsResult {
    pub code: String,
    pub unwrapped: usize,
}

/// Rewrite every read of `host["name"]` into the global `name`.
pub fn unwrap_host_globals(source: &str) -> Result<HostGlobalsResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan {
        scoping: &scoping,
        nodes: &nodes,
        shadowed: false,
        edits: Vec::new(),
        unwrapped: 0,
    };
    scan.visit_program(&program);
    if scan.shadowed || scan.edits.is_empty() {
        return Ok(unchanged(source));
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("host-globals edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("host-globals output: {error}"))?;
    Ok(HostGlobalsResult {
        code: applied.output,
        unwrapped: scan.unwrapped,
    })
}

fn unchanged(source: &str) -> HostGlobalsResult {
    HostGlobalsResult {
        code: source.into(),
        unwrapped: 0,
    }
}

struct Scan<'s, 'a> {
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    shadowed: bool,
    edits: Vec<analysis::Edit>,
    unwrapped: usize,
}

impl Scan<'_, '_> {
    fn is_host(&self, expression: &Expression<'_>) -> bool {
        matches!(expression, Expression::Identifier(id)
            if id.name == "host" && analysis::identifier_symbol(id, self.scoping).is_none())
    }

    /// True when the member is a receiver- or target-sensitive position.
    fn sensitive(&self, member: &ComputedMemberExpression<'_>) -> bool {
        let span = member.span;
        let same = |other: Span| other == span;
        match self.nodes.parent_kind(member.node_id.get()) {
            AstKind::CallExpression(call) => same(call.callee.span()),
            AstKind::TaggedTemplateExpression(tagged) => same(tagged.tag.span()),
            AstKind::AssignmentExpression(assignment) => same(assignment.left.span()),
            AstKind::UpdateExpression(update) => same(update.argument.span()),
            AstKind::UnaryExpression(unary) => {
                unary.operator == UnaryOperator::Delete && same(unary.argument.span())
            }
            AstKind::ForInStatement(statement) => same(statement.left.span()),
            AstKind::ForOfStatement(statement) => same(statement.left.span()),
            _ => false,
        }
    }
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        if identifier.name == "host" {
            self.shadowed = true;
        }
        walk::walk_binding_identifier(self, identifier);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        walk::walk_computed_member_expression(self, member);
        if member.optional || !self.is_host(&member.object) || self.sensitive(member) {
            return;
        }
        let Expression::StringLiteral(key) = &member.expression else {
            return;
        };
        let name = key.value.as_str();
        if name.is_empty() || !is_identifier_name(name) || is_reserved_keyword(name) {
            return;
        }
        let scope = self.nodes.get_node(member.node_id.get()).scope_id();
        if self.scoping.find_binding(scope, name.into()).is_some() {
            return;
        }
        self.edits
            .push(analysis::Edit::replace(member.span, name.to_owned()));
        self.unwrapped += 1;
    }
}

/// An ASCII identifier name, usable verbatim as an identifier reference.
fn is_identifier_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

#[cfg(test)]
mod tests {
    use super::unwrap_host_globals;

    #[test]
    fn unwraps_a_global_read() {
        let result = unwrap_host_globals("let x = host[\"document\"];").unwrap();
        assert_eq!(result.unwrapped, 1);
        assert_eq!(result.code, "let x = document;");
    }

    #[test]
    fn unwraps_nested_and_constructor_positions() {
        let source = "host[\"Math\"][\"floor\"](x); y = new host[\"TypeError\"]();";
        let result = unwrap_host_globals(source).unwrap();
        assert_eq!(result.unwrapped, 2);
        assert_eq!(result.code, "Math[\"floor\"](x); y = new TypeError();");
    }

    #[test]
    fn used_escaped_key_value() {
        let result = unwrap_host_globals("host[\"\\u0041\"];").unwrap();
        assert_eq!(result.code, "A;");
    }

    #[test]
    fn declines_a_name_bound_where_it_is_read() {
        let source = "function f(document) { return host[\"document\"]; }\nfunction g() { let Math = 1; return host[\"Math\"]; }";
        let result = unwrap_host_globals(source).unwrap();
        assert_eq!(result.unwrapped, 0, "{}", result.code);
        assert_eq!(result.code, source);
    }

    #[test]
    fn declines_shadowed_host() {
        let source = "function f(host) { return host[\"x\"]; }";
        let result = unwrap_host_globals(source).unwrap();
        assert_eq!(result.unwrapped, 0);
        assert_eq!(result.code, source);
    }

    #[test]
    fn leaves_receiver_and_target_positions_alone() {
        let source = "host[\"setTimeout\"](f, 0); host[\"x\"] = 1; delete host[\"y\"]; host[\"z\"]++; for (host[\"k\"] in o) {}";
        let result = unwrap_host_globals(source).unwrap();
        assert_eq!(result.unwrapped, 0);
        assert_eq!(result.code, source);
    }

    #[test]
    fn leaves_non_identifier_and_reserved_keys_alone() {
        let source = "host[\"a-b\"]; host[\"class\"]; host[0]; host[key];";
        let result = unwrap_host_globals(source).unwrap();
        assert_eq!(result.unwrapped, 0);
        assert_eq!(result.code, source);
    }
}
