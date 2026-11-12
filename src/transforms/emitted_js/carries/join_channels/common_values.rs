//! Factor branch-independent private channel values. Unlike a common textual
//! suffix, channels can have different positions among other branch work.
//! Every normal path must assign the same inert value, and no code in the
//! branch, closure or handler may observe the cell before the new store.
use std::collections::BTreeMap;

use super::*;
use oxc_semantic::SymbolId;

struct Store {
    statement: Span,
    value: Span,
    input: Option<(SymbolId, Span)>,
    this: bool,
}

#[cfg(test)]
mod tests {
    #[test]
    fn common_values_cross_different_orders_and_nested_fallthrough_paths() {
        for source in [
            "function f(flag){const saved=read();let $CARRY_0;if(flag){$CARRY_0=saved;left();}else{right();$CARRY_0=saved;}consume($CARRY_0);}",
            "function f(flag,mode){const saved=read();let $CARRY_0;if(flag){if(mode){$CARRY_0=saved;left();}else{return 2;}}else{right();$CARRY_0=saved;}consume($CARRY_0);}",
            "function f(flag){let saved=read();let $CARRY_0;if(flag){$CARRY_0=saved;left();}else{right();$CARRY_0=saved;}consume($CARRY_0);saved=next();return saved;}",
            "function f(flag){let $CARRY_0;if(flag){$CARRY_0=this;left();}else{right();$CARRY_0=this;}consume($CARRY_0);}",
            "function f(flag){const a=read();const b=read();let $CARRY_0,$CARRY_1;if(flag){$CARRY_0=a;left();$CARRY_1=b;}else{$CARRY_1=b;right();$CARRY_0=a;}consume($CARRY_0,$CARRY_1);}",
        ] {
            let (code,count)=super::super::normalize_join_channels(source,&Default::default()).unwrap();
            assert!(count>0,"{code}");
            assert_eq!(code.matches("$CARRY_0=saved;").count(),0,"{code}");
            assert!(code.contains("$CARRY_0 = "),"{code}");
        }
    }

    #[test]
    fn retain_observed_unassigned_shadowed_mutable_and_abrupt_channels() {
        for source in [
            "function f(flag,mode){const saved=read();let result;if(flag){if(mode){result=saved;left();}}else{right();result=saved;}return result;}",
            "function f(flag){let saved=read();let result;if(flag){result=saved;saved=2;}else{right();result=saved;}return result;}",
            "function f(flag){let result;if(flag){result=saved;var saved=2;}else{right();result=saved;}return result;}",
            "function f(flag){const saved=read();let result;if(flag){let saved=other();result=saved;left();}else{right();result=saved;}return result;}",
            "function f(flag){const saved=read();let result;if(flag){result=saved;consume(result);}else{right();result=saved;}return result;}",
            "function f(flag){const saved=read();let result;function observe(){return result;}if(flag){result=saved;left();}else{right();result=saved;}return observe();}",
            "function f(flag){const saved=read();let result;try{if(flag){result=saved;left();}else{right();result=saved;}}catch(e){return result;}return result;}",
            "function f(flag){const saved=read();let result;label:{if(flag){result=saved;break label;}else{right();result=saved;}}return result;}",
            "function f(flag){const saved=read();let result;if(flag){result=saved;left();}else{right();result=saved;}result=other();return result;}",
            "function f(flag){const saved=read();let result;function $THREAD_1_2(){}if(flag){result=saved;left();}else{right();result=saved;}return result;}",
            "function f(flag){let result;if(flag){result=1;left();}else{right();result=2;}return result;}",
            "function f(flag){let saved=read();let result;function change(){saved=other();}if(flag){result=saved;change();}else{right();result=saved;}return result;}",
            "class C extends B{constructor(flag){let result;if(flag){result=this;left();}else{right();result=this;}return result;}}",
        ] {
            let (code,count)=super::super::normalize_join_channels(source,&Default::default()).unwrap();
            assert_eq!(count,0,"{code}");
            assert_eq!(code,source);
        }
    }

    #[test]
    fn shared_inputs_and_outputs_retain_their_channel_stores() {
        let source="function f(flag){const saved=read();let result;if(flag){result=saved;left();}else{right();result=saved;}consume(result);}";
        for name in ["saved", "result"] {
            let protected = [name.into()].into_iter().collect();
            let (code, count) = super::super::normalize_join_channels(source, &protected).unwrap();
            assert_eq!(count, 0, "{code}");
            assert_eq!(code, source);
        }
    }
}
struct Collect<'s> {
    scoping: &'s Scoping,
    stores: BTreeMap<SymbolId, Vec<Store>>,
}
impl<'a> Visit<'a> for Collect<'_> {
    fn visit_expression_statement(&mut self, s: &ExpressionStatement<'a>) {
        if let Expression::AssignmentExpression(a) = &s.expression {
            if a.operator == AssignmentOperator::Assign {
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    let mut value = &a.right;
                    while let Expression::ParenthesizedExpression(p) = value {
                        value = &p.expression;
                    }
                    let input = match value {
                        Expression::Identifier(id) => analysis::identifier_symbol(id, self.scoping)
                            .map(|s| Some((s, id.span))),
                        Expression::NumericLiteral(_)
                        | Expression::StringLiteral(_)
                        | Expression::BooleanLiteral(_)
                        | Expression::NullLiteral(_)
                        | Expression::BigIntLiteral(_) => Some(None),
                        Expression::ThisExpression(_) => Some(None),
                        Expression::UnaryExpression(_) | Expression::BinaryExpression(_)
                            if super::super::super::purity::expression_is_pure(value) =>
                        {
                            Some(None)
                        }
                        _ => None,
                    };
                    if let (Some(symbol), Some(input)) =
                        (analysis::identifier_symbol(id, self.scoping), input)
                    {
                        self.stores.entry(symbol).or_default().push(Store {
                            statement: s.span,
                            value: a.right.span(),
                            input,
                            this: matches!(value, Expression::ThisExpression(_)),
                        });
                    }
                }
            }
        }
        // Expressions cannot contain another statement in this execution
        // environment. Their nested functions/classes are deliberately opaque.
    }
    fn visit_variable_declaration(&mut self, _: &VariableDeclaration<'a>) {}
    fn visit_return_statement(&mut self, _: &ReturnStatement<'a>) {}
    fn visit_throw_statement(&mut self, _: &ThrowStatement<'a>) {}
    fn visit_if_statement(&mut self, i: &IfStatement<'a>) {
        self.visit_statement(&i.consequent);
        if let Some(alt) = &i.alternate {
            self.visit_statement(alt);
        }
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {}
}

fn reaches_if(i: &IfStatement<'_>, stores: &[Span], assigned: bool) -> Option<Option<bool>> {
    let yes = reaches(&i.consequent, stores, assigned)?;
    let no = match &i.alternate {
        Some(s) => reaches(s, stores, assigned)?,
        None => Some(assigned),
    };
    Some(match (yes, no) {
        (Some(a), Some(b)) => Some(a && b),
        (a, None) | (None, a) => a,
    })
}

// None: unsupported control flow. Some(None): no normal completion.
// Some(Some(b)): whether all normal paths have stored a value.
fn reaches(s: &Statement<'_>, stores: &[Span], mut assigned: bool) -> Option<Option<bool>> {
    if stores.contains(&s.span()) {
        return Some(Some(true));
    }
    match s {
        Statement::BlockStatement(b) => {
            for s in &b.body {
                match reaches(s, stores, assigned)? {
                    None => return Some(None),
                    Some(next) => assigned = next,
                }
            }
            Some(Some(assigned))
        }
        Statement::IfStatement(i) => reaches_if(i, stores, assigned),
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => Some(None),
        Statement::ExpressionStatement(_)
        | Statement::VariableDeclaration(_)
        | Statement::FunctionDeclaration(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_) => Some(Some(assigned)),
        _ => None,
    }
}

impl super::Scan<'_, '_> {
    fn channel_initialized(&self, symbol: SymbolId, at: Span) -> bool {
        self.facts.initialized.get(&symbol).is_some_and(|region| {
            region.start <= at.start
                && at.end <= region.end
                && self.facts.owners.enclosing(at)
                    == self
                        .facts
                        .owners
                        .enclosing(self.scoping.symbol_span(symbol))
        })
    }
    pub(crate) fn common_values(&mut self, i: &IfStatement<'_>) {
        if self.facts.pending_children {
            return;
        }
        let Statement::BlockStatement(yes) = &i.consequent else {
            return;
        };
        let Some(scope) = yes
            .scope_id
            .get()
            .and_then(|s| self.scoping.scope_parent_id(s))
        else {
            return;
        };
        let Some(owner) = self.facts.owners.enclosing(i.span) else {
            return;
        };
        if self
            .facts
            .tries
            .iter()
            .any(|s| self.facts.owners.enclosing(*s) == Some(owner))
        {
            return;
        }
        let mut found = Collect {
            scoping: self.scoping,
            stores: BTreeMap::new(),
        };
        found.visit_if_statement(i);
        let mut proposal = vec![];
        let mut suffix = String::new();
        for (symbol, stores) in found.stores {
            if stores.len() < 2
                || self.protected.contains(self.scoping.symbol_name(symbol))
                || !self
                    .scoping
                    .symbol_flags(symbol)
                    .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
                || !self.scoping.symbol_redeclarations(symbol).is_empty()
                || self.scoping.symbol_flags(symbol).is_const_variable()
                || self.scoping.symbol_span(symbol).start >= i.span.start
                || !self.channel_initialized(symbol, i.span)
            {
                continue;
            }
            let oxc_ast::AstKind::VariableDeclarator(d) =
                self.nodes.kind(self.scoping.symbol_declaration(symbol))
            else {
                continue;
            };
            if d.init.is_some() || !matches!(d.id, BindingPattern::BindingIdentifier(_)) {
                continue;
            }
            let refs = self
                .scoping
                .get_resolved_references(symbol)
                .collect::<Vec<_>>();
            if refs.iter().any(|r| {
                let at = self.nodes.get_node(r.node_id()).kind().span();
                self.facts.owners.enclosing(at) != Some(owner)
                    || (r.is_read() && at.start < i.span.end)
                    || (r.is_write()
                        && !stores
                            .iter()
                            .any(|s| s.statement.start <= at.start && at.end <= s.statement.end))
            }) {
                continue;
            }
            let rhs = stores[0].value;
            if stores.iter().any(|s| self.text(s.value) != self.text(rhs)) {
                continue;
            }
            if stores.iter().any(|s| s.this) && !self.facts.initialized_this.contains(&owner) {
                continue;
            }
            let after = Span::new(i.span.end, i.span.end);
            if stores.iter().any(|store| {
                store.input.is_some_and(|(input, at)| {
                    self.protected.contains(self.scoping.symbol_name(input))
                        || !self
                            .scoping
                            .symbol_flags(input)
                            .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
                        || !self.scoping.symbol_redeclarations(input).is_empty()
                        || self.scoping.symbol_span(input).start >= i.span.start
                        || !self.channel_initialized(input, at)
                        || !self.channel_initialized(input, after)
                        || self.scoping.get_resolved_references(input).any(|r| {
                            let at = self.nodes.get_node(r.node_id()).kind().span();
                            r.is_write()
                                && (self.facts.owners.enclosing(at) != Some(owner)
                                    || (i.span.start <= at.start && at.end <= i.span.end))
                        })
                        || self
                            .scoping
                            .find_binding(scope, self.scoping.symbol_ident(input))
                            != Some(input)
                })
            }) {
                continue;
            }
            let spans = stores.iter().map(|s| s.statement).collect::<Vec<_>>();
            if reaches_if(i, &spans, false) != Some(Some(true)) {
                continue;
            }
            if stores.iter().any(|s| {
                self.text(s.statement).contains("/*") || self.text(s.statement).contains("//")
            }) {
                continue;
            }
            proposal.extend(stores.iter().map(|s| analysis::Edit::delete(s.statement)));
            suffix.push_str(&format!(
                "\n{} = {};",
                self.scoping.symbol_name(symbol),
                self.text(rhs)
            ));
        }
        if proposal.is_empty() {
            return;
        }
        if self.facts.list_ifs.contains(&i.span) {
            proposal.push(analysis::Edit::replace(
                Span::new(i.span.end, i.span.end),
                suffix,
            ));
        } else {
            proposal.push(analysis::Edit::replace(
                Span::new(i.span.start, i.span.start),
                "{",
            ));
            proposal.push(analysis::Edit::replace(
                Span::new(i.span.end, i.span.end),
                format!("{suffix}\n}}"),
            ));
        }
        self.accept(proposal);
    }
}
