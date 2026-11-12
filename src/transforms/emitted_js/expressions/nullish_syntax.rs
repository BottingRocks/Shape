//! Recover optional property reads and nullish defaults from equivalent
//! guards. Loose == null checks need a value proved not to be HTMLDDA;
//! otherwise retain their wider browser semantics.
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use std::collections::BTreeSet;
use crate::transforms::emitted_js::support::unwrap;

fn undefined(e: &Expression<'_>) -> bool {
    matches!(unwrap(e),Expression::UnaryExpression(u) if u.operator==UnaryOperator::Void&&super::super::purity::expression_is_pure(&u.argument))
}
fn id(e: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match unwrap(e) {
        Expression::Identifier(i) => analysis::identifier_symbol(i, scoping),
        _ => None,
    }
}

#[derive(Default)]
struct Contexts {
    bare: BTreeSet<Span>,
    delete: BTreeSet<Span>,
    dynamic: bool,
}
impl<'a> Visit<'a> for Contexts {
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        self.bare.insert(unwrap(&c.callee).span());
        walk::walk_call_expression(self, c);
    }
    fn visit_tagged_template_expression(&mut self, t: &TaggedTemplateExpression<'a>) {
        self.bare.insert(unwrap(&t.tag).span());
        walk::walk_tagged_template_expression(self, t);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == UnaryOperator::Delete {
            self.delete.insert(unwrap(&u.argument).span());
        }
        walk::walk_unary_expression(self, u);
    }
    fn visit_identifier_reference(&mut self, i: &IdentifierReference<'a>) {
        self.dynamic |= i.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}
pub(crate) fn recover_nullish_syntax(source: &str) -> Result<(String, usize), String> {
    let alloc = Allocator::default();
    let program = analysis::parse_program(&alloc, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut contexts = Contexts::default();
    contexts.visit_program(&program);
    if contexts.dynamic {
        return Ok((source.into(), 0));
    }
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        contexts: &contexts,
        edits: vec![],
    };
    scan.visit_program(&program);
    scan.edits
        .sort_by_key(|e| (e.start, std::cmp::Reverse(e.end)));
    let mut end = 0;
    scan.edits.retain(|e| {
        if e.start < end {
            false
        } else {
            end = e.end;
            true
        }
    });
    let count = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("nullish syntax edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)?;
    Ok((applied.output, count))
}
struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    contexts: &'s Contexts,
    edits: Vec<analysis::Edit>,
}
impl Scan<'_, '_> {
    fn text(&self, e: &Expression<'_>) -> &str {
        &self.source[e.span().start as usize..e.span().end as usize]
    }
    fn non_html_dda(&self, e: &Expression<'_>, depth: usize) -> bool {
        if depth == 0 {
            return false;
        }
        match unwrap(e){
            Expression::Identifier(i)=>analysis::identifier_symbol(i,self.scoping).is_some_and(|s|{
                self.scoping.symbol_redeclarations(s).is_empty()&&self.scoping.get_resolved_references(s).all(|r|!r.is_write())
                    && matches!(self.nodes.kind(self.scoping.symbol_declaration(s)),AstKind::VariableDeclarator(d) if d.init.as_ref().is_some_and(|e|self.non_html_dda(e,depth-1)))
                    && self.scoping.symbol_flags(s).contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
            }),
            Expression::NullLiteral(_)|Expression::NumericLiteral(_)|Expression::StringLiteral(_)|Expression::BooleanLiteral(_)|Expression::BigIntLiteral(_)|Expression::TemplateLiteral(_)|Expression::ArrayExpression(_)|Expression::ObjectExpression(_)|Expression::RegExpLiteral(_)|Expression::FunctionExpression(_)|Expression::ArrowFunctionExpression(_)|Expression::ClassExpression(_)|Expression::UnaryExpression(_)|Expression::BinaryExpression(_)=>true,
            Expression::ConditionalExpression(c)=>self.non_html_dda(&c.consequent,depth-1)&&self.non_html_dda(&c.alternate,depth-1),
            Expression::LogicalExpression(l)=>self.non_html_dda(&l.left,depth-1)&&self.non_html_dda(&l.right,depth-1),
            _=>false,
        }
    }
    // (value, null branch is consequent). Strict guards must cover both
    // null and undefined, with the same resolved lexical cell in each test.
    fn guard<'s, 'a>(&self, e: &'s Expression<'a>) -> Option<(&'s Expression<'a>, bool)> {
        if let Expression::UnaryExpression(u) = unwrap(e) {
            if u.operator == UnaryOperator::LogicalNot {
                let (v, n) = self.guard(&u.argument)?;
                return Some((v, !n));
            }
        }
        let compare = |e: &'s Expression<'a>| -> Option<(&'s Expression<'a>, bool, bool)> {
            let Expression::BinaryExpression(b) = unwrap(e) else {
                return None;
            };
            let equal = matches!(
                b.operator,
                BinaryOperator::StrictEquality | BinaryOperator::Equality
            );
            if !equal
                && !matches!(
                    b.operator,
                    BinaryOperator::StrictInequality | BinaryOperator::Inequality
                )
            {
                return None;
            }
            let (value, null) = if matches!(unwrap(&b.left), Expression::NullLiteral(_)) {
                (&b.right, true)
            } else if matches!(unwrap(&b.right), Expression::NullLiteral(_)) {
                (&b.left, true)
            } else if undefined(&b.left) {
                (&b.right, false)
            } else if undefined(&b.right) {
                (&b.left, false)
            } else {
                return None;
            };
            id(value, self.scoping)?;
            Some((value, equal, null))
        };
        if let Expression::BinaryExpression(b) = unwrap(e) {
            if matches!(
                b.operator,
                BinaryOperator::Equality | BinaryOperator::Inequality
            ) {
                let (v, equal, null) = compare(e)?;
                if null && self.non_html_dda(v, 24) {
                    return Some((v, equal));
                }
            }
        }
        let Expression::LogicalExpression(l) = unwrap(e) else {
            return None;
        };
        let (a, eq_a, null_a) = compare(&l.left)?;
        let (b, eq_b, null_b) = compare(&l.right)?;
        let strict = |e: &Expression<'_>| matches!(unwrap(e),Expression::BinaryExpression(b) if matches!(b.operator,BinaryOperator::StrictEquality|BinaryOperator::StrictInequality));
        if id(a, self.scoping) != id(b, self.scoping)
            || eq_a != eq_b
            || null_a == null_b
            || !strict(&l.left)
            || !strict(&l.right)
        {
            return None;
        }
        if eq_a && l.operator == LogicalOperator::Or || !eq_a && l.operator == LogicalOperator::And
        {
            Some((a, eq_a))
        } else {
            None
        }
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'a>) {
        let proposal = (|| {
            if self.contexts.delete.contains(&c.span) {
                return None;
            }
            let (value, null_yes) = self.guard(&c.test)?;
            let (nullish, present) = if null_yes {
                (&c.consequent, &c.alternate)
            } else {
                (&c.alternate, &c.consequent)
            };
            let mut text = if id(present, self.scoping) == id(value, self.scoping) {
                format!("(({}) ?? ({}))", self.text(value), self.text(nullish))
            } else if undefined(nullish) {
                let member = match unwrap(present) {
                    Expression::ComputedMemberExpression(m)
                        if !m.optional
                            && id(&m.object, self.scoping) == id(value, self.scoping) =>
                    {
                        format!("({})?.[{}]", self.text(value), self.text(&m.expression))
                    }
                    Expression::StaticMemberExpression(m)
                        if !m.optional
                            && id(&m.object, self.scoping) == id(value, self.scoping) =>
                    {
                        format!("({})?.{}", self.text(value), m.property.name)
                    }
                    _ => return None,
                };
                if self.contexts.bare.contains(&c.span) {
                    format!("(0, {member})")
                } else {
                    format!("({member})")
                }
            } else {
                return None;
            };
            if self.source[c.span.start as usize..c.span.end as usize].contains("/*")
                || self.source[c.span.start as usize..c.span.end as usize].contains("//")
            {
                return None;
            }
            if text.is_empty() {
                return None;
            }
            Some(analysis::Edit::replace(c.span, std::mem::take(&mut text)))
        })();
        if let Some(p) = proposal {
            self.edits.push(p);
        }
        walk::walk_conditional_expression(self, c);
    }
}
