//! Turn repeated left-associated +1 index snapshots into a private counter.
//! Each increment occupies the original index evaluation position. Numeric
//! category, complete reference coverage and capture proofs preserve both
//! coercion order and rounding, without reassociating floating-point sums.
use super::super::adjacent_ssa::LocalReads;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;
use std::collections::BTreeSet;
use crate::transforms::emitted_js::support::unwrap;

fn offset(e: &Expression<'_>, symbol: SymbolId, scoping: &Scoping) -> Option<(usize, Span)> {
    let mut e = unwrap(e);
    let mut count = 0;
    while let Expression::BinaryExpression(b) = e {
        if count >= 16
            || b.operator != BinaryOperator::Addition
            || !matches!(unwrap(&b.right), Expression::NumericLiteral(n) if n.value == 1.0)
        {
            return None;
        }
        count += 1;
        e = unwrap(&b.left);
    }
    match e {
        Expression::Identifier(id) if analysis::identifier_symbol(id, scoping) == Some(symbol) => {
            Some((count, id.span))
        }
        _ => None,
    }
}

#[derive(Default)]
struct Facts {
    dynamic: bool,
    pending: bool,
}
impl<'a> Visit<'a> for Facts {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.pending |=
            f.id.as_ref()
                .is_some_and(|id| id.name.starts_with("$THREAD_"))
                && f.body
                    .as_ref()
                    .is_some_and(|b| b.statements.is_empty() && b.directives.is_empty());
        walk::walk_function(self, f, flags);
    }
}

pub(crate) fn recover_index_counters(
    source: &str,
    protected: &BTreeSet<String>,
    checked_captures: bool,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut facts = Facts::default();
    facts.visit_program(&program);
    if facts.dynamic || facts.pending && !checked_captures {
        return Ok((source.into(), 0));
    }
    let (scoping, nodes) = analysis::analyze(&program);
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let mut scan = Scan {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        locals: &locals,
        edits: vec![],
        count: 0,
    };
    super::super::support::for_each_list(&program, |list| scan.body(list));
    let result = analysis::apply_edits(source, scan.edits);
    if !result.rejected.is_empty() {
        return Err("index counter edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &result.output)?;
    Ok((result.output, scan.count))
}
struct Scan<'s, 'a> {
    source: &'s str,
    protected: &'s BTreeSet<String>,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    locals: &'s LocalReads,
    edits: Vec<analysis::Edit>,
    count: usize,
}
impl Scan<'_, '_> {
    fn body(&mut self, body: &[Statement<'_>]) {
        for (position, s) in body.iter().enumerate() {
            let Statement::VariableDeclaration(d) = s else {
                continue;
            };
            if d.kind != VariableDeclarationKind::Let || d.declarations.len() != 1 {
                continue;
            }
            let BindingPattern::BindingIdentifier(id) = &d.declarations[0].id else {
                continue;
            };
            let Some(symbol) = id.symbol_id.get() else {
                continue;
            };
            if !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
                || self.protected.contains(id.name.as_str())
                || d.declarations[0].init.is_none()
                || !self.scoping.symbol_redeclarations(symbol).is_empty()
                || !self
                    .locals
                    .numeric_cell(symbol, Span::new(d.span.end, d.span.end))
            {
                continue;
            }
            let refs: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
            if refs.iter().any(|r| r.is_write()) {
                continue;
            }
            let mut expected = BTreeSet::new();
            let mut proposal = vec![];
            let mut writes = 0;
            for next in body.iter().skip(position + 1) {
                let index = match next {
                    Statement::ExpressionStatement(s) => match &s.expression {
                        Expression::AssignmentExpression(a)
                            if a.operator == AssignmentOperator::Assign =>
                        {
                            match &a.left {
                                AssignmentTarget::ComputedMemberExpression(m) => {
                                    Some(&m.expression)
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(index) = index {
                    if let Some((step, read)) = offset(index, symbol, self.scoping) {
                        if step != writes {
                            break;
                        }
                        expected.insert(read);
                        if step != 0 {
                            proposal.push(analysis::Edit::replace(
                                index.span(),
                                format!("(++{})", id.name),
                            ));
                        }
                        writes += 1;
                        continue;
                    }
                }
                let terminal = match next {
                    Statement::ReturnStatement(s) => s.argument.as_ref(),
                    Statement::ExpressionStatement(s) => match &s.expression {
                        Expression::AssignmentExpression(a)
                            if a.operator == AssignmentOperator::Assign
                                && matches!(
                                    a.left,
                                    AssignmentTarget::AssignmentTargetIdentifier(_)
                                ) =>
                        {
                            Some(&a.right)
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if writes >= 2 {
                    if let Some((value, (step, read))) =
                        terminal.and_then(|e| offset(e, symbol, self.scoping).map(|o| (e, o)))
                    {
                        if step == writes - 1 || step == writes {
                            expected.insert(read);
                            proposal.push(analysis::Edit::replace(
                                value.span(),
                                if step == writes {
                                    format!("(++{})", id.name)
                                } else {
                                    id.name.to_string()
                                },
                            ));
                        }
                    }
                }
                break;
            }
            if writes < 2
                || expected.len() != refs.len()
                || refs.iter().any(|r| {
                    let at = self.nodes.get_node(r.node_id()).kind().span();
                    !r.is_read()
                        || !expected.contains(&at)
                        || !self.locals.initialized(symbol, at)
                        || !self.locals.same_iteration(d.span, at)
                })
            {
                continue;
            }
            if proposal.iter().any(|e| {
                let old = &self.source[e.start as usize..e.end as usize];
                old.contains("/*")
                    || old.contains("//")
                    || self
                        .edits
                        .iter()
                        .any(|p| e.start < p.end && p.start < e.end)
            }) {
                continue;
            }
            self.edits.extend(proposal);
            self.count += 1;
        }
    }
}
