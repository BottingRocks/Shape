//! Rematerialize numeric input reads of an unchanged, unescaped arguments object.
//! Parameterless regular functions avoid sloppy parameter mapping. Arrows and
//! nested functions are separate regions: never change which arguments object
//! a use observes. Shares the ordinary-built-in policy for missing indices.
use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use super::super::{adjacent_ssa::LocalReads, index::SpanIndex};
use crate::analysis;
use crate::transforms::emitted_js::support::unwrap;

pub(crate) fn input(e: &Expression<'_>) -> bool {
    matches!(unwrap(e), Expression::ComputedMemberExpression(m)
        if !m.optional && matches!(unwrap(&m.object), Expression::Identifier(i) if i.name == "arguments")
        && matches!(unwrap(&m.expression), Expression::NumericLiteral(n) if n.value >= 0.0 && n.value < 65536.0 && n.value.fract() == 0.0))
}
#[derive(Default)]
struct Facts {
    owners: SpanIndex,
    eligible: BTreeSet<Span>,
    bare: BTreeSet<Span>,
    nonvalues: BTreeSet<Span>,
    dynamic: bool,
}
impl<'a> Visit<'a> for Facts {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.owners.push(f.span);
        if super::super::dead_ssa::readonly_arguments(f) {
            self.eligible.insert(f.span);
        }
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.owners.push(f.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.owners.push(c.span);
        walk::walk_class(self, c);
    }
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        self.bare.insert(unwrap(&c.callee).span());
        walk::walk_call_expression(self, c);
    }
    fn visit_tagged_template_expression(&mut self, t: &TaggedTemplateExpression<'a>) {
        self.bare.insert(unwrap(&t.tag).span());
        walk::walk_tagged_template_expression(self, t);
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        if p.shorthand {
            self.nonvalues.insert(p.value.span());
        }
        walk::walk_object_property(self, p);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == UnaryOperator::Delete {
            self.nonvalues.insert(unwrap(&u.argument).span());
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

pub(crate) fn inline_argument_snapshots(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    if !source.contains("arguments") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.visit_program(&program);
    if facts.dynamic
        || facts.eligible.is_empty()
        || !super::super::dense_arrays::ordinary_prototypes(&program, &scoping)
    {
        return Ok((source.into(), 0));
    }
    facts.owners.build();
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        facts: &facts,
        locals: &locals,
        protected,
        edits: vec![],
        count: 0,
    };
    super::super::support::for_each_list(&program, |list| scan.body(list));
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("argument snapshot edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)?;
    Ok((applied.output, scan.count))
}
struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    facts: &'s Facts,
    locals: &'s LocalReads,
    protected: &'s BTreeSet<String>,
    edits: Vec<analysis::Edit>,
    count: usize,
}
impl Scan<'_, '_> {
    fn body(&mut self, body: &[Statement<'_>]) {
        // Removing a leading declaration must not promote a later string
        // expression into a directive (in particular, "use strict").
        let barrier = body.iter().any(|s| {
            matches!(s, Statement::ExpressionStatement(e) if matches!(e.expression, Expression::StringLiteral(_)))
        });
        for s in body {
            let Statement::VariableDeclaration(d) = s else {
                continue;
            };
            if !matches!(
                d.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            ) || d.declarations.len() != 1
            {
                continue;
            }
            let (BindingPattern::BindingIdentifier(id), Some(init)) =
                (&d.declarations[0].id, &d.declarations[0].init)
            else {
                continue;
            };
            if !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
                || self.protected.contains(id.name.as_str())
                || !input(init)
            {
                continue;
            }
            let Some(owner) = self.facts.owners.enclosing(d.span) else {
                continue;
            };
            if !self.facts.eligible.contains(&owner) {
                continue;
            }
            let Some(symbol) = id.symbol_id.get() else {
                continue;
            };
            if !self.scoping.symbol_redeclarations(symbol).is_empty() {
                continue;
            }
            let refs: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
            if refs.is_empty()
                || refs.iter().any(|r| {
                    let at = self.nodes.get_node(r.node_id()).kind().span();
                    r.is_write()
                        || at.start < d.span.end
                        || !self.locals.initialized(symbol, at)
                        || self.facts.owners.enclosing(at) != Some(owner)
                        || self.facts.nonvalues.contains(&at)
                        || self
                            .scoping
                            .find_binding(r.scope_id(), self.scoping.symbol_ident(symbol))
                            != Some(symbol)
                })
            {
                continue;
            }
            // The arguments object has no SymbolId; the function-wide proof
            // above excludes every local binding/escape of that name.
            let value = &self.source[init.span().start as usize..init.span().end as usize];
            let mut group = vec![analysis::Edit::replace(
                d.span,
                if barrier { ";" } else { "" },
            )];
            let mut replacement_bytes = usize::from(barrier);
            let mut original_bytes = d.span.size() as usize;
            for r in refs {
                let at = self.nodes.get_node(r.node_id()).kind().span();
                let text = if self.facts.bare.contains(&at) {
                    format!("(0, {value})")
                } else {
                    format!("({value})")
                };
                replacement_bytes += text.len();
                original_bytes += at.size() as usize;
                group.push(analysis::Edit::replace(at, text));
            }
            if replacement_bytes >= original_bytes
                || group.iter().any(|e| {
                    self.edits
                        .iter()
                        .any(|p| e.start < p.end && p.start < e.end)
                        || self.source[e.start as usize..e.end as usize].contains("/*")
                        || self.source[e.start as usize..e.end as usize].contains("//")
                })
            {
                continue;
            }
            self.edits.extend(group);
            self.count += 1;
        }
    }
}
