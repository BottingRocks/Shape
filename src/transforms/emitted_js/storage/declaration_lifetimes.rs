//! Put private uninitialized declarations and inert fresh literals at first use.
//! No read or captured observer may see the extended TDZ. Single direct stores
//! become initializers; branch channels become adjacent to their producing join.
use super::super::index::SpanIndex;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};
use std::collections::BTreeSet;

#[derive(Default)]
struct Facts {
    owners: SpanIndex,
    dynamic: bool,
    pending: bool,
}
impl<'a> Visit<'a> for Facts {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.owners.push(f.span);
        self.pending |=
            f.id.as_ref()
                .is_some_and(|id| id.name.starts_with("$THREAD_"))
                && f.body
                    .as_ref()
                    .is_some_and(|b| b.statements.is_empty() && b.directives.is_empty());
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
            VariableDeclarationKind::Let
                | VariableDeclarationKind::Const
                | VariableDeclarationKind::Var
        );
        walk::walk_variable_declaration(self, d);
    }
}
pub(crate) fn tighten(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
) -> Result<(String, usize), String> {
    tighten_with_policy(source, protected, checked, false)
}
pub(crate) fn tighten_allocations(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
) -> Result<(String, usize), String> {
    tighten_with_policy(source, protected, checked, true)
}
fn tighten_with_policy(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
    allocations_only: bool,
) -> Result<(String, usize), String> {
    let alloc = Allocator::default();
    let program = analysis::parse_program(&alloc, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.visit_program(&program);
    facts.owners.build();
    if facts.dynamic || facts.pending && !checked {
        return Ok((source.into(), 0));
    }
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        facts: &facts,
        protected,
        edits: vec![],
        count: 0,
        allocations_only,
    };
    super::super::support::for_each_list(&program, |list| scan.body(list));
    let result = analysis::apply_edits(source, scan.edits);
    if !result.rejected.is_empty() {
        return Err("declaration lifetime edits overlap".into());
    }
    Ok((result.output, scan.count))
}
struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    facts: &'s Facts,
    protected: &'s BTreeSet<String>,
    edits: Vec<analysis::Edit>,
    count: usize,
    allocations_only: bool,
}
impl Scan<'_, '_> {
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }
    fn body(&mut self, body: &[Statement<'_>]) {
        for (position, statement) in body.iter().enumerate() {
            let Statement::VariableDeclaration(decl) = statement else {
                continue;
            };
            if !matches!(
                decl.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            ) {
                continue;
            }
            for item in &decl.declarations {
                let BindingPattern::BindingIdentifier(id) = &item.id else {
                    continue;
                };
                let fresh = item.init.as_ref().is_some_and(|init| {
                    matches!(
                        init,
                        Expression::ArrayExpression(_) | Expression::ObjectExpression(_)
                    ) && super::super::purity::expression_is_pure(init)
                });
                if fresh {
                    if !self.allocations_only || decl.declarations.len() != 1 {
                        continue;
                    }
                } else if self.allocations_only
                    || item.init.is_some()
                    || decl.kind != VariableDeclarationKind::Let
                {
                    continue;
                }
                if !["$SSA_", "$CARRY_", "$ARRAY_", "$OBJECT_"]
                    .iter()
                    .any(|p| id.name.starts_with(p))
                    || self.protected.contains(id.name.as_str())
                {
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
                        self.facts.owners.enclosing(at) != self.facts.owners.enclosing(decl.span)
                            || at.start < decl.span.end
                            || fresh && r.is_write()
                    })
                {
                    continue;
                }
                let Some(first) = body.iter().enumerate().skip(position + 1).find(|(_, s)| {
                    refs.iter().any(|r| {
                        let at = self.nodes.get_node(r.node_id()).kind().span();
                        s.span().start <= at.start && at.end <= s.span().end
                    })
                }) else {
                    continue;
                };
                let (index, use_statement) = first;
                let materialize = if fresh {
                    None
                } else {
                    match use_statement {
                        Statement::ExpressionStatement(s) => match &s.expression {
                            Expression::AssignmentExpression(a)
                                if a.operator == AssignmentOperator::Assign
                                    && matches!(&a.left,AssignmentTarget::AssignmentTargetIdentifier(to) if analysis::identifier_symbol(to,self.scoping)==Some(symbol))
                                    && refs.iter().filter(|r| r.is_read()).all(|r| {
                                        self.nodes.get_node(r.node_id()).kind().span().start
                                            >= s.span.end
                                    }) =>
                            {
                                Some(a.right.span())
                            }
                            _ => None,
                        },
                        _ => None,
                    }
                };
                // Do not repeatedly swap independent uninitialized declarations.
                // Movement must cross real work, or turn a store into an initializer.
                if materialize.is_none() && !body[position+1..index].iter().any(|s|{
                    !matches!(s,Statement::EmptyStatement(_))
                        && !matches!(s,Statement::VariableDeclaration(d) if matches!(d.kind,VariableDeclarationKind::Let|VariableDeclarationKind::Const) && d.declarations.iter().all(|d|
                            d.init.is_none() || fresh && d.init.as_ref().is_some_and(|e|matches!(e,Expression::ArrayExpression(_)|Expression::ObjectExpression(_))&&super::super::purity::expression_is_pure(e))))
                }){continue;}
                let retained = decl
                    .declarations
                    .iter()
                    .filter(|d| d.span != item.span)
                    .map(|d| self.text(d.span))
                    .collect::<Vec<_>>();
                let mut group = vec![analysis::Edit::replace(
                    decl.span,
                    if retained.is_empty() {
                        ";".into()
                    } else {
                        format!("let {};", retained.join(", "))
                    },
                )];
                if let Some(rhs) = materialize {
                    group.push(analysis::Edit::replace(
                        use_statement.span(),
                        format!("let {} = ({});", id.name, self.text(rhs)),
                    ));
                } else {
                    group.push(analysis::Edit::replace(
                        Span::new(use_statement.span().start, use_statement.span().start),
                        if fresh {
                            format!("{} {};\n", decl.kind.as_str(), self.text(item.span))
                        } else {
                            format!("let {};\n", id.name)
                        },
                    ));
                }
                if group.iter().any(|e| {
                    self.text(Span::new(e.start, e.end)).contains("/*")
                        || self.text(Span::new(e.start, e.end)).contains("//")
                        || self
                            .edits
                            .iter()
                            .any(|p| e.start < p.end && p.start < e.end || e.start == p.start)
                }) {
                    continue;
                }
                self.edits.extend(group);
                self.count += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn single_stores_materialize_and_join_declarations_follow_unrelated_work() {
        for source in [
            "function f(){let $SSA_0;work();$SSA_0=make();return $SSA_0;}",
            "function f(){let $SSA_0;work();$SSA_0=make();consume($SSA_0);$SSA_0=other();return $SSA_0;}",
            "function f(flag){let $CARRY_0;work();if(flag)$CARRY_0=left();else $CARRY_0=right();return $CARRY_0;}",
        ] {
            let (code,count)=super::tighten(source,&Default::default(),false).unwrap();
            assert_eq!(count,1,"{code}");
            assert!(code.find("work()").unwrap()<code.find("let $SSA_0").or_else(||code.find("let $CARRY_0")).unwrap(),"{code}");
        }
        for source in [
            "function f(){let $SSA_0;consume($SSA_0);$SSA_0=make();return $SSA_0;}",
            "function f(){let $SSA_0;function observe(){return $SSA_0;}work();$SSA_0=make();return observe();}",
            "function f(){let $SSA_0;let spare;if(flag)$SSA_0=1;else $SSA_0=2;return $SSA_0;}",
        ] {
            let (code,count)=super::tighten(source,&Default::default(),false).unwrap();
            assert_eq!(count,0,"{code}");
        }
    }
    #[test]
    fn a_materialized_initializer_cannot_observe_the_old_undefined_cell() {
        let source = "function f(){let $SSA_0;$SSA_0=$SSA_0+1;return $SSA_0;}";
        assert_eq!(
            super::tighten(source, &Default::default(), false)
                .unwrap()
                .1,
            0
        );
    }
}

#[cfg(test)]
mod allocation_tests {
    #[test]
    fn unobserved_inert_allocations_move_to_their_first_construction_use() {
        for source in [
            "function f(){let $SSA_0=[];work();$SSA_0[0]=left();$SSA_0[1]=right();return $SSA_0;}",
            "function f(){const $ARRAY_0=[0,0];work();$ARRAY_0[0]=left();$ARRAY_0[1]=right();return $ARRAY_0;}",
            "function f(){let $SSA_0={};work();$SSA_0.x=left();return $SSA_0;}",
        ] {
            let (code,count)=super::tighten_allocations(source,&Default::default(),false).unwrap();
            assert_eq!(count,1,"{code}");
            assert!(code.find("work()").unwrap()<code.find("let $SSA_0").or_else(||code.find("const $ARRAY_0")).unwrap(),"{code}");
        }
        for source in [
            "function f(){let $SSA_0=[read()];work();return $SSA_0;}",
            "function f(){let $SSA_0={x:object.x};work();return $SSA_0;}",
            "function f(){let $SSA_0=[];function observe(){return $SSA_0;}work();return observe();}",
            "function f(){let $SSA_0=[];let $SSA_1=[];$SSA_0[0]=$SSA_1;return $SSA_0;}",
            "function f(){let $SSA_0=[];work();$SSA_0=other();return $SSA_0;}",
        ] {
            let (code,count)=super::tighten_allocations(source,&Default::default(),false).unwrap();
            assert_eq!(count,0,"{code}");
        }
    }
}
