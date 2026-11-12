//! Expose unconditional join-channel definitions without changing evaluation
//! order. Common branch suffixes remain after branch work. Private undefined
//! cells with one later unconditional store can initialize at that store.
use std::collections::BTreeSet;

use super::super::index::SpanIndex;
mod common_values;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{scope::ScopeFlags, scope::ScopeId};

pub(crate) fn normalize_join_channels(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.visit_program(&program);
    facts.owners.build();
    if facts.dynamic {
        return Ok((source.into(), 0));
    }
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        protected,
        facts: &facts,
        edits: vec![],
        changed: 0,
    };
    scan.visit_program(&program);
    let result = analysis::apply_edits(source, scan.edits);
    if !result.rejected.is_empty() {
        return Err("join channel edits overlap".into());
    }
    Ok((result.output, scan.changed))
}
#[derive(Default)]
struct Facts {
    owners: SpanIndex,
    dynamic: bool,
    strings: bool,
    list_ifs: BTreeSet<Span>,
    tries: Vec<Span>,
    pending_children: bool,
    initialized: std::collections::BTreeMap<oxc_semantic::SymbolId, Span>,
    initialized_this: BTreeSet<Span>,
}
impl Facts {
    fn declarations(&mut self, body: &[Statement<'_>], end: u32) {
        for s in body {
            if let Statement::VariableDeclaration(d) = s {
                if d.kind != VariableDeclarationKind::Var {
                    for item in &d.declarations {
                        if let BindingPattern::BindingIdentifier(id) = &item.id {
                            if let Some(symbol) = id.symbol_id.get() {
                                self.initialized.insert(symbol, Span::new(d.span.end, end));
                            }
                        }
                    }
                }
            }
        }
    }
}
impl<'a> Visit<'a> for Facts {
    fn visit_try_statement(&mut self, s: &TryStatement<'a>) {
        self.tries.push(s.span);
        walk::walk_try_statement(self, s);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.declarations(&b.statements, b.span.end);
        self.list_ifs.extend(
            b.statements
                .iter()
                .filter_map(|s| matches!(s, Statement::IfStatement(_)).then_some(s.span())),
        );
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.declarations(&b.body, b.span.end);
        self.list_ifs.extend(
            b.body
                .iter()
                .filter_map(|s| matches!(s, Statement::IfStatement(_)).then_some(s.span())),
        );
        walk::walk_block_statement(self, b);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        if f.r#type == FunctionType::FunctionDeclaration {
            self.initialized_this.insert(f.span);
        }
        self.pending_children |=
            f.id.as_ref()
                .is_some_and(|id| id.name.starts_with("$THREAD_"))
                && f.body
                    .as_ref()
                    .is_some_and(|b| b.statements.is_empty() && b.directives.is_empty());
        self.owners.push(f.span);
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
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
    fn visit_variable_declaration(&mut self, d: &VariableDeclaration<'a>) {
        self.dynamic |= !matches!(
            d.kind,
            VariableDeclarationKind::Var
                | VariableDeclarationKind::Let
                | VariableDeclarationKind::Const
        );
        walk::walk_variable_declaration(self, d);
    }
    fn visit_expression_statement(&mut self, s: &ExpressionStatement<'a>) {
        self.strings |= matches!(s.expression, Expression::StringLiteral(_));
        walk::walk_expression_statement(self, s);
    }
}
struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    protected: &'s BTreeSet<String>,
    facts: &'s Facts,
    edits: Vec<analysis::Edit>,
    changed: usize,
}
impl Scan<'_, '_> {
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }
    fn accept(&mut self, proposal: Vec<analysis::Edit>) -> bool {
        if proposal.iter().any(|e| {
            self.edits
                .iter()
                .any(|p| e.start < p.end && p.start < e.end)
        }) {
            return false;
        }
        self.edits.extend(proposal);
        self.changed += 1;
        true
    }
    fn suffix(&mut self, i: &IfStatement<'_>) {
        let (Statement::BlockStatement(yes), Some(Statement::BlockStatement(no))) =
            (&i.consequent, &i.alternate)
        else {
            return;
        };
        let Some(outside) = yes
            .scope_id
            .get()
            .and_then(|s| self.scoping.scope_parent_id(s))
        else {
            return;
        };
        let mut common = vec![];
        for (a, b) in yes.body.iter().rev().zip(no.body.iter().rev()) {
            let (Statement::ExpressionStatement(a), Statement::ExpressionStatement(b)) = (a, b)
            else {
                break;
            };
            if self.text(a.expression.span()) != self.text(b.expression.span()) {
                break;
            }
            let mut check = Outside {
                scoping: self.scoping,
                scope: outside,
                valid: true,
            };
            check.visit_expression(&a.expression);
            check.visit_expression(&b.expression);
            if !check.valid {
                break;
            }
            common.push((a.span, b.span, a.expression.span()));
        }
        let Some((a, b, _)) = common.last() else {
            return;
        };
        let yes_tail = Span::new(a.start, yes.span.end - 1);
        let no_tail = Span::new(b.start, no.span.end - 1);
        if [yes_tail, no_tail]
            .iter()
            .any(|s| self.text(*s).contains("/*") || self.text(*s).contains("//"))
        {
            return;
        }
        let suffix = common
            .iter()
            .rev()
            .map(|(_, _, e)| format!("{};", self.text(*e)))
            .collect::<Vec<_>>()
            .join("\n");
        // A label must enclose both the branch and its factored suffix, so a
        // break out of the original branch still skips its remaining work.
        let wrapped = !self.facts.list_ifs.contains(&i.span);
        let mut proposal = vec![
            analysis::Edit::delete(yes_tail),
            analysis::Edit::delete(no_tail),
            analysis::Edit::replace(
                Span::new(i.span.end, i.span.end),
                format!("\n{suffix}\n{}", if wrapped { "}" } else { "" }),
            ),
        ];
        if wrapped {
            proposal.push(analysis::Edit::replace(
                Span::new(i.span.start, i.span.start),
                "{",
            ));
        }
        self.accept(proposal);
    }
    fn definitions(&mut self, body: &[Statement<'_>], scope: Span) {
        for (index, s) in body.iter().enumerate() {
            let Statement::VariableDeclaration(d) = s else {
                continue;
            };
            if d.kind != VariableDeclarationKind::Let
                || d.declarations.iter().any(|v| {
                    v.init.is_some() || !matches!(v.id, BindingPattern::BindingIdentifier(_))
                })
                || self.text(d.span).contains("/*")
                || self.text(d.span).contains("//")
            {
                continue;
            }
            let mut assignments = vec![];
            let mut removed = BTreeSet::new();
            for declaration in &d.declarations {
                let BindingPattern::BindingIdentifier(id) = &declaration.id else {
                    continue;
                };
                let Some(symbol) = id.symbol_id.get() else {
                    continue;
                };
                if self.protected.contains(id.name.as_str())
                    || !self.scoping.symbol_redeclarations(symbol).is_empty()
                {
                    continue;
                }
                for next in body.iter().skip(index + 1) {
                    let Statement::ExpressionStatement(s) = next else {
                        continue;
                    };
                    let Expression::AssignmentExpression(a) = &s.expression else {
                        continue;
                    };
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &a.left else {
                        continue;
                    };
                    if a.operator != AssignmentOperator::Assign
                        || analysis::identifier_symbol(target, self.scoping) != Some(symbol)
                    {
                        continue;
                    }
                    let refs = self
                        .scoping
                        .get_resolved_references(symbol)
                        .collect::<Vec<_>>();
                    if !refs.iter().all(|r| {
                        let at = self.nodes.get_node(r.node_id()).kind().span();
                        self.facts.owners.enclosing(at) == self.facts.owners.enclosing(d.span)
                            && if r.is_write() {
                                at == target.span && !r.is_read()
                            } else {
                                at.start >= s.span.end && at.end <= scope.end
                            }
                    }) || refs.iter().filter(|r| r.is_write()).count() != 1
                        || self.text(s.span).contains("/*")
                        || self.text(s.span).contains("//")
                    {
                        break;
                    }
                    assignments.push(analysis::Edit::replace(
                        s.span,
                        format!(
                            "const {} = {};",
                            self.text(id.span),
                            self.text(a.right.span())
                        ),
                    ));
                    removed.insert(symbol);
                    break;
                }
            }
            if assignments.is_empty() {
                continue;
            }
            let remain = d
                .declarations
                .iter()
                .filter(|v| match &v.id {
                    BindingPattern::BindingIdentifier(id) => {
                        !removed.contains(&id.symbol_id.get().unwrap())
                    }
                    _ => true,
                })
                .map(|v| self.text(v.span))
                .collect::<Vec<_>>();
            let replacement = if remain.is_empty() {
                if self.facts.strings {
                    ";".into()
                } else {
                    String::new()
                }
            } else {
                format!("let {};", remain.join(", "))
            };
            assignments.push(analysis::Edit::replace(d.span, replacement));
            self.accept(assignments);
        }
    }
}
struct Outside<'s> {
    scoping: &'s Scoping,
    scope: ScopeId,
    valid: bool,
}
impl<'a> Visit<'a> for Outside<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.valid &= self.scoping.find_binding(self.scope, id.name)
            == analysis::identifier_symbol(id, self.scoping);
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {
        self.valid = false;
    }
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {
        self.valid = false;
    }
    fn visit_class(&mut self, _: &Class<'a>) {
        self.valid = false;
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_if_statement(&mut self, i: &IfStatement<'a>) {
        self.suffix(i);
        self.common_values(i);
        walk::walk_if_statement(self, i);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.definitions(&b.statements, b.span);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.definitions(&b.body, b.span);
        walk::walk_block_statement(self, b);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn resource_disposal_keeps_common_suffix_inside_each_branch() {
        let source = "function f(){if(flag){using r=acquire();consume();}else{consume();}}";
        let (code, count) = super::normalize_join_channels(source, &Default::default()).unwrap();
        assert_eq!(count, 0);
        assert_eq!(code, source);
    }

    #[test]
    fn externally_observed_slots_keep_their_declaration() {
        let source = "function f(){let $KEY_0;work();$KEY_0=read();return $KEY_0;}";
        let protected = ["$KEY_0".into()].into_iter().collect();
        let (code, count) = super::normalize_join_channels(source, &protected).unwrap();
        assert_eq!(count, 0);
        assert_eq!(code, source);
        let (_, count) = super::normalize_join_channels(source, &Default::default()).unwrap();
        assert_eq!(count, 1);
    }
}
