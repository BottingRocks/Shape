//! Remove Number snapshots only after proving a Number result. Unknown inputs
//! retain their first coercion, including valueOf effects and BigInt handling.
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};
use std::collections::BTreeSet;

pub(crate) fn simplify_numeric_coercions(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts {
        scoping: &scoping,
        numbers: BTreeSet::new(),
        dynamic: false,
    };
    // Definitions are emitted in evaluation order. Repeat for definitions in
    // enclosing functions that follow a nested function declaration.
    for _ in 0..16 {
        let count = facts.numbers.len();
        facts.visit_program(&program);
        if count == facts.numbers.len() {
            break;
        }
    }
    if facts.dynamic {
        return Ok((source.into(), 0));
    }
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        numbers: &facts.numbers,
        edits: Vec::new(),
    };
    scan.visit_program(&program);
    let count = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("numeric coercion edits overlap".into());
    }
    Ok((applied.output, count))
}
pub(crate) fn number_argument<'s, 'a>(
    e: &'s Expression<'a>,
    scoping: &Scoping,
) -> Option<&'s Expression<'a>> {
    let Expression::CallExpression(c) = e else {
        return None;
    };
    let Expression::Identifier(id) = &c.callee else {
        return None;
    };
    (id.name == "Number"
        && analysis::identifier_symbol(id, scoping).is_none()
        && !c.optional
        && c.arguments.len() == 1
        && !c.arguments[0].is_spread())
    .then(|| c.arguments[0].to_expression())
}

// Share the existing native-Number guard with the ordered-argument proof.
// Calling Number still performs its original coercion; only its cached
// successful result supplies a type fact.
pub(crate) fn ordinary_number(program: &Program<'_>, scoping: &Scoping) -> bool {
    let mut facts = Facts {
        scoping,
        numbers: BTreeSet::new(),
        dynamic: false,
    };
    facts.visit_program(program);
    !facts.dynamic
}
fn numeric(e: &Expression<'_>, numbers: &BTreeSet<SymbolId>, scoping: &Scoping) -> bool {
    if number_argument(e, scoping).is_some() {
        return true;
    }
    match e {
        Expression::NumericLiteral(_) => true,
        Expression::Identifier(id) => {
            analysis::identifier_symbol(id, scoping).is_some_and(|s| numbers.contains(&s))
        }
        Expression::ParenthesizedExpression(p) => numeric(&p.expression, numbers, scoping),
        Expression::UnaryExpression(u) => {
            u.operator == UnaryOperator::UnaryPlus
                || (matches!(
                    u.operator,
                    UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot
                ) && numeric(&u.argument, numbers, scoping))
        }
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
            numeric(&b.left, numbers, scoping) && numeric(&b.right, numbers, scoping)
        }
        Expression::BinaryExpression(b)
            if matches!(
                b.operator,
                BinaryOperator::Subtraction
                    | BinaryOperator::Multiplication
                    | BinaryOperator::Division
                    | BinaryOperator::Remainder
                    | BinaryOperator::Exponential
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::ShiftRightZeroFill
                    | BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseXOR
                    | BinaryOperator::BitwiseAnd
            ) =>
        {
            numeric(&b.left, numbers, scoping) || numeric(&b.right, numbers, scoping)
        }
        _ => false,
    }
}
struct Facts<'s> {
    scoping: &'s Scoping,
    numbers: BTreeSet<SymbolId>,
    dynamic: bool,
}
impl<'a> Visit<'a> for Facts<'_> {
    fn visit_assignment_target(&mut self, target: &AssignmentTarget<'a>) {
        self.dynamic |= match target {
            AssignmentTarget::StaticMemberExpression(m) => m.property.name == "Number",
            AssignmentTarget::ComputedMemberExpression(m) => {
                matches!(&m.expression,Expression::StringLiteral(k) if k.value=="Number")
            }
            _ => false,
        };
        walk::walk_assignment_target(self, target);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        // Redefinition of the global constructor invalidates the type proof.
        self.dynamic |= call
            .arguments
            .get(1)
            .and_then(|a| a.as_expression())
            .is_some_and(|e| matches!(e,Expression::StringLiteral(k) if k.value=="Number"));
        walk::walk_call_expression(self, call);
    }
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(id), Some(value)) = (&d.id, &d.init) {
            if let Some(symbol) = id.symbol_id.get() {
                if id.name.starts_with("$SSA_")
                    && !self
                        .scoping
                        .get_resolved_references(symbol)
                        .any(|r| r.is_write())
                    && numeric(value, &self.numbers, self.scoping)
                {
                    self.numbers.insert(symbol);
                }
            }
        }
        walk::walk_variable_declarator(self, d);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval"
            || (id.name == "Number"
                && analysis::identifier_symbol(id, self.scoping).is_none()
                && id
                    .reference_id
                    .get()
                    .is_some_and(|r| self.scoping.get_reference(r).is_write()));
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}
struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    numbers: &'s BTreeSet<SymbolId>,
    edits: Vec<analysis::Edit>,
}
impl<'a> Visit<'a> for Scan<'_, '_> {
    fn visit_expression(&mut self, e: &Expression<'a>) {
        if let Some(arg) = number_argument(e, self.scoping) {
            if numeric(arg, self.numbers, self.scoping) {
                self.edits.push(analysis::Edit::replace(
                    e.span(),
                    format!(
                        "({})",
                        &self.source[arg.span().start as usize..arg.span().end as usize]
                    ),
                ));
                return;
            }
        }
        walk::walk_expression(self, e);
    }
    fn visit_for_statement(&mut self, f: &ForStatement<'a>) {
        if let (
            Some(ForStatementInit::VariableDeclaration(d)),
            Some(Expression::AssignmentExpression(a)),
        ) = (&f.init, &f.update)
        {
            if d.kind == VariableDeclarationKind::Let
                && d.declarations.len() == 1
                && a.operator == AssignmentOperator::Assign
            {
                if let (
                    BindingPattern::BindingIdentifier(id),
                    Some(Expression::NumericLiteral(_)),
                    AssignmentTarget::AssignmentTargetIdentifier(target),
                    Expression::BinaryExpression(b),
                ) = (
                    &d.declarations[0].id,
                    &d.declarations[0].init,
                    &a.left,
                    &a.right,
                ) {
                    if let Some(symbol) = id.symbol_id.get() {
                        let arg = number_argument(&b.left, self.scoping);
                        if analysis::identifier_symbol(target, self.scoping) == Some(symbol)
                            && matches!(
                                b.operator,
                                BinaryOperator::Addition | BinaryOperator::Subtraction
                            )
                            && matches!(&b.right,Expression::NumericLiteral(n) if n.value==1.0)
                            && matches!(arg,Some(Expression::Identifier(arg)) if analysis::identifier_symbol(arg,self.scoping)==Some(symbol))
                            && self
                                .scoping
                                .get_resolved_references(symbol)
                                .filter(|r| r.is_write())
                                .all(|r| {
                                    self.nodes.get_node(r.node_id()).kind().span() == target.span
                                })
                        {
                            self.edits.push(analysis::Edit::replace(
                                a.span,
                                format!(
                                    "{}{}",
                                    target.name,
                                    if b.operator == BinaryOperator::Addition {
                                        "++"
                                    } else {
                                        "--"
                                    }
                                ),
                            ));
                            if let Some(init) = &f.init {
                                self.visit_for_statement_init(init);
                            }
                            if let Some(test) = &f.test {
                                self.visit_expression(test);
                            }
                            self.visit_statement(&f.body);
                            return;
                        }
                    }
                }
            }
        }
        walk::walk_for_statement(self, f);
    }
}
