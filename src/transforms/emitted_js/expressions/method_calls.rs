//! Recover method syntax from emitted native `.call`/`.apply` operations.
//!
//! As with the trace-program property cleanup, Function.prototype.call/apply
//! have their ordinary behavior. A receiver must resolve to the same local
//! binding on both sides, with no captured/implicit argument writes. Member
//! lookup still happens once at the original location; saved method snapshots
//! are not re-read across effects. Explicit call/apply mutation disables this
//! pass. Dynamic scope and optional/spread/unknown array-like forms decline.
use super::super::index::SpanIndex;
use crate::analysis;

pub(crate) fn ordinary_call_lookups(program: &Program<'_>) -> bool {
    let mut facts = Facts::default();
    facts.visit_program(program);
    !facts.dynamic && !facts.custom_call
}
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::Scoping;
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;
use std::collections::BTreeSet;

pub(crate) fn simplify_method_calls(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.visit_program(&program);
    facts.functions.build();
    facts.parameters.build();
    if facts.dynamic || facts.custom_call {
        return Ok((source.into(), 0));
    }
    let mut scan = Scan {
        protected,
        source,
        scoping: &scoping,
        nodes: &nodes,
        facts: &facts,
        edits: Vec::new(),
        selected: BTreeSet::new(),
    };
    scan.visit_program(&program);
    if scan.edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let count = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("method-call edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)?;
    Ok((applied.output, count))
}
#[derive(Default)]
struct Facts {
    functions: SpanIndex,
    parameters: SpanIndex,
    dynamic: bool,
    custom_call: bool,
}
fn special_member<'a, 'b>(expression: &'b Expression<'a>) -> Option<(&'b Expression<'a>, &'b str)> {
    match expression {
        Expression::StaticMemberExpression(m) if !m.optional => {
            Some((&m.object, m.property.name.as_str()))
        }
        Expression::ComputedMemberExpression(m) if !m.optional => {
            if let Expression::StringLiteral(s) = &m.expression {
                Some((&m.object, s.value.as_str()))
            } else {
                None
            }
        }
        _ => None,
    }
}
impl<'a> Visit<'a> for Facts {
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        if special_member(&c.callee).is_some_and(|(_, name)| name == "defineProperty")
            && c.arguments.len() > 1
        {
            self.custom_call |= matches!(c.arguments[1].to_expression(),Expression::StringLiteral(s) if s.value=="call" || s.value=="apply");
        }
        walk::walk_call_expression(self, c);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(f.span);
        self.parameters.push(f.params.span);
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.functions.push(f.span);
        self.parameters.push(f.params.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        self.custom_call |= match &a.left {
            AssignmentTarget::StaticMemberExpression(m) => {
                matches!(m.property.name.as_str(), "call" | "apply")
            }
            AssignmentTarget::ComputedMemberExpression(m) => {
                matches!(&m.expression,Expression::StringLiteral(s) if s.value=="call" || s.value=="apply")
            }
            _ => false,
        };
        walk::walk_assignment_expression(self, a);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == UnaryOperator::Delete {
            self.custom_call |= special_member(&u.argument)
                .is_some_and(|(_, name)| matches!(name, "call" | "apply"));
        }
        walk::walk_unary_expression(self, u);
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        self.custom_call |= p
            .key
            .static_name()
            .is_some_and(|s| s == "call" || s == "apply");
        walk::walk_object_property(self, p);
    }
}
struct Scan<'s, 'a> {
    protected: &'s BTreeSet<String>,
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    facts: &'s Facts,
    edits: Vec<analysis::Edit>,
    selected: BTreeSet<Span>,
}
impl Scan<'_, '_> {
    fn receiver_matches(&self, a: &Expression<'_>, b: &Expression<'_>) -> bool {
        match (a, b) {
            (Expression::Identifier(a), Expression::Identifier(b)) => {
                let Some(symbol) = analysis::identifier_symbol(a, self.scoping) else {
                    return false;
                };
                if self.protected.contains(self.scoping.symbol_name(symbol)) {
                    return false;
                }
                if analysis::identifier_symbol(b, self.scoping) != Some(symbol)
                    || self
                        .facts
                        .parameters
                        .enclosing(self.scoping.symbol_span(symbol))
                        .is_some()
                {
                    return false;
                }
                let owner = self.facts.functions.enclosing(a.span);
                self.scoping.get_resolved_references(symbol).all(|r| {
                    !r.is_write()
                        || self
                            .facts
                            .functions
                            .enclosing(self.nodes.get_node(r.node_id()).kind().span())
                            == owner
                })
            }
            (Expression::ThisExpression(_), Expression::ThisExpression(_)) => true,
            _ => false,
        }
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        walk::walk_call_expression(self, c);
        if c.optional
            || c.arguments.is_empty()
            || c.arguments.iter().any(Argument::is_spread)
            || self
                .selected
                .iter()
                .any(|s| c.span.start < s.end && s.start < c.span.end)
        {
            return;
        }
        let Some((method, kind)) = special_member(&c.callee) else {
            return;
        };
        if !matches!(kind, "call" | "apply") {
            return;
        }
        let Some((receiver, _)) = special_member(method) else {
            return;
        };
        if !self.receiver_matches(receiver, c.arguments[0].to_expression()) {
            return;
        }
        let text = |span: Span| &self.source[span.start as usize..span.end as usize];
        let args = if kind == "call" {
            c.arguments
                .iter()
                .skip(1)
                .map(|a| format!("({})", text(a.span())))
                .collect::<Vec<_>>()
        } else {
            if c.arguments.len() != 2 {
                return;
            }
            let Expression::ArrayExpression(array) = c.arguments[1].to_expression() else {
                return;
            };
            if array.elements.iter().any(|e| {
                matches!(
                    e,
                    ArrayExpressionElement::Elision(_) | ArrayExpressionElement::SpreadElement(_)
                )
            }) {
                return;
            }
            array
                .elements
                .iter()
                .map(|e| format!("({})", text(e.span())))
                .collect::<Vec<_>>()
        };
        self.edits.push(analysis::Edit::replace(
            c.span,
            format!("{}({})", text(method.span()), args.join(", ")),
        ));
        self.selected.insert(c.span);
    }
}
