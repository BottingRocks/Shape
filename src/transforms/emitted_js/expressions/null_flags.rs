//! Eliminate a private null-test channel paired with a value channel. Each
//! normal branch must leave the flag equal to `null == value`. Values and
//! effects stay at their original stores; only inert flag stores disappear.
use super::super::index::SpanIndex;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};
use std::collections::{BTreeMap, BTreeSet};
use crate::transforms::emitted_js::support::unwrap;

fn null_test<'s, 'a>(e: &'s Expression<'a>) -> Option<(&'s Expression<'a>, bool)> {
    let Expression::BinaryExpression(b) = unwrap(e) else {
        return None;
    };
    if !matches!(
        b.operator,
        BinaryOperator::Equality | BinaryOperator::Inequality
    ) {
        return None;
    }
    let value = if matches!(unwrap(&b.left), Expression::NullLiteral(_)) {
        &b.right
    } else if matches!(unwrap(&b.right), Expression::NullLiteral(_)) {
        &b.left
    } else {
        return None;
    };
    Some((value, b.operator == BinaryOperator::Inequality))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Token {
    Symbol(SymbolId),
    Other(u32),
    Null,
    Undefined,
}
#[derive(Clone, Copy)]
enum Bool {
    Constant(bool),
    Null(Token, bool),
}
#[derive(Clone, Default)]
struct State {
    values: BTreeMap<SymbolId, Token>,
    booleans: BTreeMap<SymbolId, Bool>,
    nullness: BTreeMap<Token, bool>,
}

#[derive(Default)]
struct Facts {
    owners: SpanIndex,
    tries: Vec<Span>,
    dynamic: bool,
    pending: bool,
    shorthand: BTreeSet<Span>,
    non_values: BTreeSet<Span>,
}
impl<'a> Visit<'a> for Facts {
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == UnaryOperator::Delete {
            self.non_values.insert(unwrap(&u.argument).span());
        }
        walk::walk_unary_expression(self, u);
    }
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
    fn visit_try_statement(&mut self, s: &TryStatement<'a>) {
        self.tries.push(s.span);
        walk::walk_try_statement(self, s);
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
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        if p.shorthand {
            self.shorthand.insert(p.value.span());
        }
        walk::walk_object_property(self, p);
    }
}

pub(crate) fn eliminate_null_flags(
    source: &str,
    protected: &BTreeSet<String>,
    checked_captures: bool,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.visit_program(&program);
    facts.owners.build();
    if facts.dynamic || facts.pending && !checked_captures {
        return Ok((source.into(), 0));
    }
    let mut scan = Scan {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        facts: &facts,
        edits: vec![],
        count: 0,
    };
    scan.visit_program(&program);
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("null flag edits overlap".into());
    }
    Ok((applied.output, scan.count))
}

struct Scan<'s, 'a> {
    source: &'s str,
    protected: &'s BTreeSet<String>,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    facts: &'s Facts,
    edits: Vec<analysis::Edit>,
    count: usize,
}
impl Scan<'_, '_> {
    fn stable(&self, s: SymbolId) -> bool {
        !self.protected.contains(self.scoping.symbol_name(s))
            && self
                .scoping
                .symbol_flags(s)
                .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
            && self.scoping.symbol_redeclarations(s).is_empty()
            && self
                .scoping
                .get_resolved_references(s)
                .all(|r| !r.is_write())
    }
    fn token(&self, e: &Expression<'_>, state: &State) -> Option<Token> {
        match unwrap(e) {
            Expression::Identifier(id) => {
                let s = analysis::identifier_symbol(id, self.scoping)?;
                state
                    .values
                    .get(&s)
                    .copied()
                    .or_else(|| self.stable(s).then_some(Token::Symbol(s)))
            }
            Expression::NullLiteral(_) => Some(Token::Null),
            Expression::UnaryExpression(u)
                if u.operator == UnaryOperator::Void
                    && super::super::purity::expression_is_pure(&u.argument) =>
            {
                Some(Token::Undefined)
            }
            _ => None,
        }
    }
    fn boolean(&self, e: &Expression<'_>, state: &State, depth: usize) -> Option<Bool> {
        if depth == 0 {
            return None;
        }
        if let Some((value, negated)) = null_test(e) {
            return Some(Bool::Null(self.token(value, state)?, negated));
        }
        match unwrap(e) {
            Expression::BooleanLiteral(b) => Some(Bool::Constant(b.value)),
            Expression::UnaryExpression(u) if u.operator == UnaryOperator::LogicalNot => {
                match self.boolean(&u.argument, state, depth - 1)? {
                    Bool::Constant(b) => Some(Bool::Constant(!b)),
                    Bool::Null(v, n) => Some(Bool::Null(v, !n)),
                }
            }
            Expression::Identifier(id) => {
                let s = analysis::identifier_symbol(id, self.scoping)?;
                if let Some(b) = state.booleans.get(&s) {
                    return Some(*b);
                }
                if !self.stable(s) {
                    return None;
                }
                let AstKind::VariableDeclarator(d) =
                    self.nodes.kind(self.scoping.symbol_declaration(s))
                else {
                    return None;
                };
                // An earlier cached predicate must not be reinterpreted
                // using a mutable channel's current value.
                self.boolean(d.init.as_ref()?, &State::default(), depth - 1)
            }
            _ => None,
        }
    }
    fn known_null(state: &State, t: Token) -> Option<bool> {
        match t {
            Token::Null | Token::Undefined => Some(true),
            _ => state.nullness.get(&t).copied(),
        }
    }
    fn relation(&self, state: &State, flag: SymbolId, value: SymbolId) -> bool {
        let Some(value) = state.values.get(&value).copied() else {
            return false;
        };
        match state.booleans.get(&flag) {
            Some(Bool::Null(t, false)) if *t == value => true,
            Some(Bool::Null(t, inverted)) => Self::known_null(state, *t)
                .zip(Self::known_null(state, value))
                .is_some_and(|(a, b)| (a ^ inverted) == b),
            Some(Bool::Constant(b)) => Self::known_null(state, value) == Some(*b),
            None => false,
        }
    }
    fn branch(
        &self,
        i: &IfStatement<'_>,
        state: State,
        flag: SymbolId,
        stores: &mut Vec<(Span, Span)>,
        budget: &mut usize,
    ) -> Option<Vec<State>> {
        let predicate = self.boolean(&i.test, &state, 24);
        let mut yes = state.clone();
        let mut no = state;
        if let Some(Bool::Null(t, n)) = predicate {
            yes.nullness.insert(t, !n);
            no.nullness.insert(t, n);
        }
        let mut paths = self.statement(&i.consequent, yes, flag, stores, budget)?;
        if let Some(alt) = &i.alternate {
            paths.extend(self.statement(alt, no, flag, stores, budget)?);
        } else {
            paths.push(no);
        }
        (paths.len() <= 64).then_some(paths)
    }
    fn statement(
        &self,
        s: &Statement<'_>,
        mut state: State,
        flag: SymbolId,
        stores: &mut Vec<(Span, Span)>,
        budget: &mut usize,
    ) -> Option<Vec<State>> {
        *budget = budget.checked_sub(1)?;
        match s {
            Statement::BlockStatement(b) => {
                let mut paths = vec![state];
                for s in &b.body {
                    let mut next = vec![];
                    for state in paths {
                        next.extend(self.statement(s, state, flag, stores, budget)?);
                    }
                    if next.len() > 64 {
                        return None;
                    }
                    paths = next;
                }
                Some(paths)
            }
            Statement::IfStatement(i) => self.branch(i, state, flag, stores, budget),
            Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => Some(vec![]),
            Statement::ExpressionStatement(s) => {
                if let Expression::AssignmentExpression(a) = unwrap(&s.expression) {
                    if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                        let symbol = analysis::identifier_symbol(id, self.scoping)?;
                        if a.operator != AssignmentOperator::Assign {
                            return None;
                        }
                        let boolean = self.boolean(&a.right, &state, 24);
                        if symbol == flag {
                            // These accepted boolean expressions only read
                            // lexical cells, compare with null, and negate.
                            boolean?;
                            stores.push((s.span, id.span));
                        }
                        let token = self
                            .token(&a.right, &state)
                            .unwrap_or(Token::Other(a.right.span().start));
                        state.values.insert(symbol, token);
                        state.booleans.remove(&symbol);
                        if let Some(b) = boolean {
                            state.booleans.insert(symbol, b);
                        }
                    }
                }
                Some(vec![state])
            }
            Statement::VariableDeclaration(d) => {
                for item in &d.declarations {
                    if let (BindingPattern::BindingIdentifier(id), Some(init)) =
                        (&item.id, &item.init)
                    {
                        let s = id.symbol_id.get()?;
                        let b = self.boolean(init, &state, 24);
                        let t = self
                            .token(init, &state)
                            .unwrap_or(Token::Other(init.span().start));
                        state.values.insert(s, t);
                        if let Some(b) = b {
                            state.booleans.insert(s, b);
                        }
                    }
                }
                Some(vec![state])
            }
            Statement::EmptyStatement(_) | Statement::FunctionDeclaration(_) => Some(vec![state]),
            _ => None,
        }
    }
    fn candidate(&mut self, i: &IfStatement<'_>, flag: SymbolId, value: SymbolId) {
        let name = self.scoping.symbol_name(flag);
        let value_name = self.scoping.symbol_ident(value);
        let owner = self.facts.owners.enclosing(i.span);
        if flag == value
            || self.protected.contains(name)
            || self.protected.contains(value_name.as_str())
            || !(name.starts_with("$SSA_") || name.starts_with("$CARRY_"))
            || self.scoping.symbol_span(flag).start >= i.span.start
            || self.scoping.symbol_span(value).start >= i.span.start
            || !self.scoping.symbol_redeclarations(flag).is_empty()
            || !self.scoping.symbol_redeclarations(value).is_empty()
            || self.facts.tries.iter().any(|t| {
                self.facts.owners.enclosing(*t) == owner
                    && t.start <= i.span.start
                    && i.span.end <= t.end
            })
        {
            return;
        }
        for symbol in [flag, value] {
            let AstKind::VariableDeclarator(d) =
                self.nodes.kind(self.scoping.symbol_declaration(symbol))
            else {
                return;
            };
            if d.init.is_some()
                || !self
                    .scoping
                    .symbol_flags(symbol)
                    .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
            {
                return;
            }
        }
        let refs: Vec<_> = self.scoping.get_resolved_references(flag).collect();
        if refs.iter().any(|r| {
            let at = self.nodes.get_node(r.node_id()).kind().span();
            self.facts.owners.enclosing(at) != owner
                || self.facts.shorthand.contains(&at)
                || self.facts.non_values.contains(&at)
                || if r.is_read() {
                    at.start < i.span.end
                        || self.scoping.find_binding(r.scope_id(), value_name) != Some(value)
                } else {
                    at.start < i.span.start || at.end > i.span.end
                }
        }) || self.scoping.get_resolved_references(value).any(|r| {
            let at = self.nodes.get_node(r.node_id()).kind().span();
            r.is_write()
                && (self.facts.owners.enclosing(at) != owner
                    || at.start < i.span.start
                    || at.end > i.span.end)
        }) {
            return;
        }
        let mut state = State::default();
        state.values.insert(value, Token::Undefined);
        let mut stores = vec![];
        let mut budget = 256;
        let Some(paths) = self.branch(i, state, flag, &mut stores, &mut budget) else {
            return;
        };
        if paths.is_empty() || !paths.iter().all(|s| self.relation(s, flag, value)) {
            return;
        }
        stores.sort_unstable();
        stores.dedup();
        if refs.iter().filter(|r| r.is_write()).any(|r| {
            !stores
                .iter()
                .any(|(_, target)| *target == self.nodes.get_node(r.node_id()).kind().span())
        }) {
            return;
        }
        // Nested assignments to the value channel are not abstractly
        // interpreted. Require each write to be a complete plain store.
        if self.scoping.get_resolved_references(value).filter(|r|r.is_write()).any(|r| {
            let at=self.nodes.get_node(r.node_id()).kind().span();
            !matches!(self.nodes.parent_kind(r.node_id()),AstKind::AssignmentExpression(a) if a.operator==AssignmentOperator::Assign && matches!(self.nodes.parent_kind(self.nodes.parent_id(r.node_id())),AstKind::ExpressionStatement(_)) && a.left.span()==at)
        }){return;}
        let mut edits: Vec<_> = stores
            .iter()
            .map(|(s, _)| analysis::Edit::delete(*s))
            .collect();
        for r in refs.iter().filter(|r| r.is_read()) {
            let at = self.nodes.get_node(r.node_id()).kind().span();
            edits.push(analysis::Edit::replace(
                at,
                format!("(null == {value_name})"),
            ));
        }
        if edits.iter().any(|e| {
            self.source[e.start as usize..e.end as usize].contains("/*")
                || self.source[e.start as usize..e.end as usize].contains("//")
                || self
                    .edits
                    .iter()
                    .any(|p| e.start < p.end && p.start < e.end)
        }) {
            return;
        }
        self.edits.extend(edits);
        self.count += 1;
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_if_statement(&mut self, i: &IfStatement<'a>) {
        struct Pairs<'s> {
            scoping: &'s Scoping,
            pairs: BTreeSet<(SymbolId, SymbolId)>,
        }
        impl<'a> Visit<'a> for Pairs<'_> {
            fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
                if let AssignmentTarget::AssignmentTargetIdentifier(flag) = &a.left {
                    if let Some((Expression::Identifier(value), false)) =
                        null_test(&a.right).map(|(v, n)| (unwrap(v), n))
                    {
                        if let (Some(f), Some(v)) = (
                            analysis::identifier_symbol(flag, self.scoping),
                            analysis::identifier_symbol(value, self.scoping),
                        ) {
                            self.pairs.insert((f, v));
                        }
                    }
                }
                walk::walk_assignment_expression(self, a);
            }
            fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
            fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
            fn visit_class(&mut self, _: &Class<'a>) {}
        }
        let mut pairs = Pairs {
            scoping: self.scoping,
            pairs: BTreeSet::new(),
        };
        pairs.visit_if_statement(i);
        for (flag, value) in pairs.pairs {
            self.candidate(i, flag, value);
        }
        walk::walk_if_statement(self, i);
    }
}
