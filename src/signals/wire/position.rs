//! Proves contiguous physical writes, separately from source evaluation order.
//! Unmodelled cursor expressions are refused rather than treated as lengths.
use crate::signals::extract::contracts::{EmissionContract, GrammarNode, WriteNode};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator, UpdateOperator};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
enum Address {
    Constant(u32),
    Read(SymbolId, Span),
    Increment(SymbolId, Span),
    PostIncrement(SymbolId, Span),
    Returned(Span),
    Advance(Box<Address>),
    Offset(Box<Address>, u32),
    Conditional(Span),
    Tail(Box<Address>),
    Unknown,
}
fn symbol(id: &IdentifierReference<'_>, scopes: &Scoping) -> Option<SymbolId> {
    id.reference_id
        .get()
        .and_then(|r| scopes.get_reference(r).symbol_id())
}
fn address(e: &Expression<'_>, scopes: &Scoping, depth: usize) -> Address {
    if depth > 16 {
        return Address::Unknown;
    }
    match e.without_parentheses() {
        Expression::NumericLiteral(n)
            if n.value.is_finite()
                && n.value.fract() == 0.
                && n.value >= 0.
                && n.value <= u32::MAX as f64 =>
        {
            Address::Constant(n.value as u32)
        }
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::UnaryPlus => {
            address(&u.argument, scopes, depth + 1)
        }
        Expression::Identifier(id) => symbol(id, scopes)
            .map(|s| Address::Read(s, id.span))
            .unwrap_or(Address::Unknown),
        Expression::ConditionalExpression(c) => Address::Conditional(c.span),
        Expression::SequenceExpression(s) => s
            .expressions
            .last()
            .map(|e| Address::Tail(Box::new(address(e, scopes, depth + 1))))
            .unwrap_or(Address::Unknown),
        Expression::UpdateExpression(u) if u.operator == UpdateOperator::Increment => {
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument {
                symbol(id, scopes)
                    .map(|s| {
                        if u.prefix {
                            Address::Increment(s, id.span)
                        } else {
                            Address::PostIncrement(s, id.span)
                        }
                    })
                    .unwrap_or(Address::Unknown)
            } else {
                Address::Unknown
            }
        }
        Expression::BinaryExpression(b)
            if b.operator == BinaryOperator::Addition
                && matches!(b.right.without_parentheses(),Expression::NumericLiteral(n) if n.value.is_finite() && n.value >= 0. && n.value <= u32::MAX as f64 && n.value.fract() == 0.) =>
        {
            let Expression::NumericLiteral(n) = b.right.without_parentheses() else {
                unreachable!()
            };
            let base = Box::new(address(&b.left, scopes, depth + 1));
            if n.value == 1. {
                Address::Advance(base)
            } else {
                Address::Offset(base, n.value as u32)
            }
        }
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
            match address(&b.right, scopes, depth + 1) {
                Address::Constant(n) => {
                    Address::Offset(Box::new(address(&b.left, scopes, depth + 1)), n)
                }
                _ => Address::Unknown,
            }
        }
        Expression::CallExpression(c) => {
            if matches!(c.callee.without_parentheses(),Expression::Identifier(id) if id.name=="Number" && symbol(id,scopes).is_none())
                && c.arguments.len() == 1
            {
                return c.arguments[0]
                    .as_expression()
                    .map(|e| address(e, scopes, depth + 1))
                    .unwrap_or(Address::Unknown);
            }
            Address::Returned(c.span)
        }
        _ => Address::Unknown,
    }
}
struct Definition {
    span: Span,
    symbol: SymbolId,
    target: Option<Span>,
    value: Address,
}
struct Call {
    span: Span,
    args: Vec<Address>,
    arg_spans: Vec<Option<Span>>,
    receiverless: bool,
}
struct Index {
    span: Span,
    address: Address,
}
struct Context<'s> {
    scopes: &'s Scoping,
    definitions: Vec<Definition>,
    calls: Vec<Call>,
    indices: Vec<Index>,
    functions: Vec<Span>,
    blocks: Vec<Span>,
    guarded_regions: Vec<Span>,
    conditionals: Vec<(Span, Address, Address)>,
    branches: Vec<(Span, Vec<Span>)>,
}
impl<'a> Visit<'a> for Context<'_> {
    fn visit_update_expression(&mut self, node: &UpdateExpression<'a>) {
        if node.operator == UpdateOperator::Increment {
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &node.argument {
                if let Some(symbol) = symbol(id, self.scopes) {
                    self.definitions.push(Definition {
                        span: node.span,
                        symbol,
                        target: Some(id.span),
                        value: Address::Advance(Box::new(Address::Read(symbol, id.span))),
                    });
                }
            }
        }
        walk::walk_update_expression(self, node);
    }
    fn visit_if_statement(&mut self, node: &IfStatement<'a>) {
        self.branches.push((
            node.span,
            std::iter::once(node.consequent.span())
                .chain(node.alternate.as_ref().map(GetSpan::span))
                .collect(),
        ));
        self.guarded_regions.push(node.consequent.span());
        self.guarded_regions
            .extend(node.alternate.as_ref().map(GetSpan::span));
        walk::walk_if_statement(self, node);
    }
    fn visit_conditional_expression(&mut self, node: &ConditionalExpression<'a>) {
        self.branches.push((
            node.span,
            vec![node.consequent.span(), node.alternate.span()],
        ));
        self.conditionals.push((
            node.span,
            address(&node.consequent, self.scopes, 0),
            address(&node.alternate, self.scopes, 0),
        ));
        self.guarded_regions
            .extend([node.consequent.span(), node.alternate.span()]);
        walk::walk_conditional_expression(self, node);
    }
    fn visit_logical_expression(&mut self, node: &LogicalExpression<'a>) {
        self.guarded_regions.push(node.right.span());
        walk::walk_logical_expression(self, node);
    }
    fn visit_for_statement(&mut self, node: &ForStatement<'a>) {
        self.guarded_regions.push(node.body.span());
        walk::walk_for_statement(self, node);
    }
    fn visit_for_in_statement(&mut self, node: &ForInStatement<'a>) {
        self.guarded_regions.push(node.body.span());
        walk::walk_for_in_statement(self, node);
    }
    fn visit_for_of_statement(&mut self, node: &ForOfStatement<'a>) {
        self.guarded_regions.push(node.body.span());
        walk::walk_for_of_statement(self, node);
    }
    fn visit_while_statement(&mut self, node: &WhileStatement<'a>) {
        self.guarded_regions.push(node.body.span());
        walk::walk_while_statement(self, node);
    }
    fn visit_do_while_statement(&mut self, node: &DoWhileStatement<'a>) {
        self.guarded_regions.push(node.body.span());
        walk::walk_do_while_statement(self, node);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.functions.push(f.span);
        if let Some(b) = &f.body {
            self.blocks.push(b.span);
        }
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.functions.push(f.span);
        self.blocks.push(f.body.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.blocks.push(b.span);
        walk::walk_block_statement(self, b);
    }
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(id), Some(e)) = (&d.id, &d.init) {
            if let Some(s) = id.symbol_id.get() {
                self.definitions.push(Definition {
                    span: e.span(),
                    symbol: s,
                    target: None,
                    value: address(e, self.scopes, 0),
                });
            }
        }
        walk::walk_variable_declarator(self, d);
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        if a.operator != AssignmentOperator::Assign {
            if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                if let Some(s) = symbol(id, self.scopes) {
                    let value = if a.operator == AssignmentOperator::Addition {
                        match address(&a.right, self.scopes, 0) {
                            Address::Constant(1) => {
                                Address::Advance(Box::new(Address::Read(s, id.span)))
                            }
                            Address::Constant(n) => {
                                Address::Offset(Box::new(Address::Read(s, id.span)), n)
                            }
                            _ => Address::Unknown,
                        }
                    } else {
                        Address::Unknown
                    };
                    self.definitions.push(Definition {
                        span: a.span,
                        symbol: s,
                        target: Some(id.span),
                        value,
                    });
                }
            }
        }
        if a.operator == AssignmentOperator::Assign {
            if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                if let Some(s) = symbol(id, self.scopes) {
                    self.definitions.push(Definition {
                        span: a.right.span(),
                        symbol: s,
                        target: Some(id.span),
                        value: address(&a.right, self.scopes, 0),
                    });
                }
            }
            if let AssignmentTarget::ComputedMemberExpression(m) = &a.left {
                self.indices.push(Index {
                    span: a.span,
                    address: address(&m.expression, self.scopes, 0),
                });
            }
        }
        walk::walk_assignment_expression(self, a);
    }
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        self.calls.push(Call{span:c.span,receiverless:matches!(c.callee.without_parentheses(),Expression::StaticMemberExpression(m) if m.property.name=="call"),args:c.arguments.iter().map(|a|a.as_expression().map(|e|address(e,self.scopes,0)).unwrap_or(Address::Unknown)).collect(),arg_spans:c.arguments.iter().map(|argument|argument.as_expression().map(GetSpan::span)).collect()});
        walk::walk_call_expression(self, c);
    }
}
fn contains(a: Span, b: Span) -> bool {
    a.start <= b.start && b.end <= a.end
}
fn result_expression(address: &Address, span: Span) -> bool {
    match address {
        Address::Returned(at) | Address::Conditional(at) => *at == span,
        Address::Advance(inner) | Address::Offset(inner, _) => result_expression(inner, span),
        _ => false,
    }
}
fn address_symbols(address: &Address, symbols: &mut BTreeSet<SymbolId>) {
    match address {
        Address::Read(s, _) | Address::Increment(s, _) | Address::PostIncrement(s, _) => {
            symbols.insert(*s);
        }
        Address::Advance(a) | Address::Offset(a, _) | Address::Tail(a) => {
            address_symbols(a, symbols)
        }
        _ => {}
    }
}
fn smallest(spans: &[Span], s: Span) -> Option<Span> {
    spans
        .iter()
        .copied()
        .filter(|a| contains(*a, s))
        .min_by_key(|a| a.size())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Constant(u32),
    Current(u64),
    Next(u64),
    Offset(u64, i64),
    Unknown,
}
fn relative(epoch: u64, offset: i64) -> Value {
    match offset {
        0 => Value::Current(epoch),
        1 => Value::Next(epoch),
        n => Value::Offset(epoch, n),
    }
}
#[derive(Clone)]
struct State {
    epoch: u64,
    known: Option<u32>,
    values: BTreeMap<SymbolId, Value>,
    last_span: Option<Span>,
    seen_defs: BTreeSet<(u32, u32)>,
    results: BTreeMap<(u32, u32), Value>,
    origins: BTreeMap<SymbolId, u32>,
}
impl State {
    fn at(epoch: u64, offset: i64) -> Value {
        match offset {
            0 => Value::Current(epoch),
            1 => Value::Next(epoch),
            _ => Value::Offset(epoch, offset),
        }
    }
    fn displacement(&self, value: Value) -> Option<i64> {
        match value {
            Value::Current(epoch) if epoch == self.epoch => Some(0),
            Value::Next(epoch) if epoch == self.epoch => Some(1),
            Value::Offset(epoch, offset) if epoch == self.epoch => Some(offset),
            _ => None,
        }
    }
    fn role(&self, value: Value) -> Value {
        if self.current(value) {
            Value::Current(0)
        } else if self.next(value) {
            Value::Next(0)
        } else if let Value::Offset(epoch, offset) = value {
            if epoch == self.epoch {
                Value::Offset(0, offset)
            } else {
                Value::Unknown
            }
        } else if matches!(value, Value::Current(_) | Value::Next(_)) {
            Value::Unknown
        } else {
            value
        }
    }
    fn new() -> Self {
        Self {
            epoch: 0,
            known: Some(3),
            values: BTreeMap::new(),
            last_span: None,
            seen_defs: BTreeSet::new(),
            results: BTreeMap::new(),
            origins: BTreeMap::new(),
        }
    }
    fn join(&mut self, results: Vec<Self>) {
        let epoch = results.iter().map(|s| s.epoch).max().unwrap() + 1;
        let normalize = |value: Value, s: &State| {
            if s.current(value) {
                Value::Current(epoch)
            } else if s.next(value) {
                Value::Next(epoch)
            } else if let Value::Offset(old, offset) = value {
                if old == s.epoch {
                    Value::Offset(epoch, offset)
                } else {
                    Value::Unknown
                }
            } else {
                value
            }
        };
        let keys: BTreeSet<_> = results
            .iter()
            .flat_map(|s| s.values.keys().copied())
            .collect();
        self.values = keys
            .into_iter()
            .map(|key| {
                let first = normalize(
                    results[0]
                        .values
                        .get(&key)
                        .copied()
                        .unwrap_or(Value::Unknown),
                    &results[0],
                );
                let same = results.iter().all(|s| {
                    normalize(s.values.get(&key).copied().unwrap_or(Value::Unknown), s) == first
                });
                (key, if same { first } else { Value::Unknown })
            })
            .collect();
        self.epoch = epoch;
        self.results = results[0]
            .results
            .iter()
            .filter_map(|(key, value)| {
                let value = normalize(*value, &results[0]);
                results
                    .iter()
                    .all(|s| {
                        s.results
                            .get(key)
                            .is_some_and(|v| normalize(*v, s) == value)
                    })
                    .then_some((*key, value))
            })
            .collect();
        self.origins = results.iter().flat_map(|s| s.origins.iter()).fold(
            BTreeMap::new(),
            |mut m, (key, start)| {
                m.entry(*key)
                    .and_modify(|v: &mut u32| *v = (*v).min(*start))
                    .or_insert(*start);
                m
            },
        );
        self.known = results[0]
            .known
            .filter(|n| results.iter().all(|s| s.known == Some(*n)));
        self.last_span = results
            .iter()
            .filter_map(|s| s.last_span)
            .max_by_key(|s| s.end);
        self.seen_defs = results.into_iter().flat_map(|s| s.seen_defs).collect();
    }
    fn current(&self, v: Value) -> bool {
        v == Value::Current(self.epoch) || matches!(v,Value::Constant(n) if Some(n)==self.known)
    }
    fn next(&self, v: Value) -> bool {
        v == Value::Next(self.epoch)
            || matches!(v,Value::Constant(n) if self.known.and_then(|n|n.checked_add(1))==Some(n))
    }
    fn advance(&mut self, byte: bool) {
        let old = self.epoch;
        self.epoch += 1;
        if byte {
            self.known = self.known.and_then(|n| n.checked_add(1));
            for value in self.values.values_mut().chain(self.results.values_mut()) {
                *value = match *value {
                    Value::Current(e) if e == old => relative(self.epoch, -1),
                    Value::Next(e) if e == old => relative(self.epoch, 0),
                    Value::Offset(e, n) if e == old => n
                        .checked_sub(1)
                        .map(|n| relative(self.epoch, n))
                        .unwrap_or(Value::Unknown),
                    v => v,
                };
            }
        } else {
            self.known = None;
        }
    }
}
struct Proof<'c, 's> {
    ctx: &'c Context<'s>,
    root: &'c GrammarNode,
    plan: Option<&'c super::SourcePlan>,
    used: BTreeSet<SymbolId>,
    allowed: BTreeSet<(u32, u32)>,
    allowed_definitions: BTreeSet<(u32, u32)>,
    steps: usize,
    live_ranges: Vec<(SymbolId, u32, u32)>,
    relevant: BTreeSet<SymbolId>,
}
impl Proof<'_, '_> {
    fn offset_value(
        &mut self,
        e: &Address,
        offset: u32,
        state: &mut State,
        active_loop: Option<Span>,
        depth: usize,
    ) -> Value {
        let value = self.value(e, state, active_loop, depth + 1);
        match value {
            Value::Constant(n) => n
                .checked_add(offset)
                .map(Value::Constant)
                .unwrap_or(Value::Unknown),
            _ => state
                .displacement(value)
                .and_then(|n| n.checked_add(i64::from(offset)))
                .map(|n| State::at(state.epoch, n))
                .unwrap_or(Value::Unknown),
        }
    }
    fn value(
        &mut self,
        e: &Address,
        state: &mut State,
        active_loop: Option<Span>,
        depth: usize,
    ) -> Value {
        if depth > 16 {
            return Value::Unknown;
        }
        match e {
            Address::Constant(n) => Value::Constant(*n),
            Address::Returned(span) | Address::Conditional(span) => state
                .results
                .get(&(span.start, span.end))
                .copied()
                .unwrap_or(Value::Unknown),
            Address::Tail(e) => self.value(e, state, active_loop, depth + 1),
            Address::Advance(e) => self.offset_value(e, 1, state, active_loop, depth),
            Address::Offset(e, offset) => self.offset_value(e, *offset, state, active_loop, depth),
            Address::Read(s, at) => {
                self.used.insert(*s);
                let def = self
                    .ctx
                    .definitions
                    .iter()
                    .filter(|d| {
                        d.symbol == *s
                            && d.span.end <= at.start
                            && smallest(&self.ctx.functions, d.span)
                                == smallest(&self.ctx.functions, *at)
                            && smallest(&self.ctx.blocks, d.span).is_some_and(|b| contains(b, *at))
                            && !self.ctx.guarded_regions.iter().any(|region| {
                                contains(*region, d.span)
                                    && !contains(*region, *at)
                                    && smallest(&self.ctx.functions, *region)
                                        == smallest(&self.ctx.functions, d.span)
                            })
                    })
                    .max_by_key(|d| d.span.end);
                if let Some(d) = def {
                    // bind_result caches each verified RHS value at its exact commit.
                    let fresh = state.last_span.is_none_or(|last| d.span.start >= last.end)
                        && active_loop.is_none_or(|l| contains(l, d.span));
                    let cold =
                        !state.values.contains_key(s) && matches!(d.value, Address::Constant(_));
                    if (fresh || cold) && !state.seen_defs.contains(&(d.span.start, d.span.end)) {
                        state.seen_defs.insert((d.span.start, d.span.end));
                        self.allowed_definitions.insert((d.span.start, d.span.end));
                        let value = self.value(&d.value, state, active_loop, depth + 1);
                        if let Some(target) = d.target {
                            self.allowed.insert((target.start, target.end));
                        }
                        state.values.insert(*s, value);
                        state.origins.insert(*s, d.span.start);
                    }
                }
                let value = state.values.get(s).copied().unwrap_or(Value::Unknown);
                let start = state.origins.get(s).copied().unwrap_or(at.start);
                self.live_ranges.push((*s, start, at.end));
                if let Some(loop_span) = active_loop {
                    if start < loop_span.start
                        && (state.current(value) || state.displacement(value).is_some())
                    {
                        // A carried cursor remains live across the backedge,
                        // even after its last lexical read in the loop body.
                        self.live_ranges.push((*s, start, loop_span.end));
                    }
                }
                value
            }
            _ => Value::Unknown,
        }
    }
    fn walk(&mut self, n: &GrammarNode, state: &mut State, active_loop: Option<Span>) -> bool {
        if self.steps == 0 {
            return false;
        }
        self.steps -= 1;
        match n {
            GrammarNode::RawPrefix(_) => true,
            GrammarNode::Sequence { items } => {
                items.iter().all(|n| self.walk(n, state, active_loop))
            }
            GrammarNode::Repeat {
                span,
                count: Some(count),
                body,
                ..
            } if *count <= 65536 => {
                for _ in 0..*count {
                    state.last_span = None;
                    state
                        .seen_defs
                        .retain(|(s, e)| !contains(*span, Span::new(*s, *e)));
                    if !self.walk(body, state, Some(*span)) {
                        return false;
                    }
                }
                true
            }
            GrammarNode::Repeat {
                span,
                count: None,
                body,
                ..
            } => {
                let path = super::control::grammar_path(self.root, n);
                if !self.plan.is_some_and(|plan| {
                    plan.counts.iter().any(|reference| {
                        Some(&reference.repeat_path) == path.as_ref()
                            && reference.loop_span.start == span.start
                            && reference.loop_span.end == span.end
                    })
                }) {
                    return false;
                }
                let mut entry = state.clone();
                entry.known = None;
                for value in entry.values.values_mut() {
                    if state.current(*value) {
                        *value = Value::Current(entry.epoch);
                    }
                }
                let mut iteration = entry.clone();
                iteration.last_span = None;
                iteration
                    .seen_defs
                    .retain(|(start, end)| !contains(*span, Span::new(*start, *end)));
                if !self.walk(body, &mut iteration, Some(*span)) {
                    return false;
                }
                // A symbolic entry cursor makes every body write prove the
                // same invariant for ANY iteration, including no-byte arms.
                // Joining zero iterations retains possibly uninitialized locals.
                state.join(vec![entry, iteration]);
                let invariant = state.clone();
                let mut next = invariant.clone();
                next.last_span = None;
                next.seen_defs
                    .retain(|(start, end)| !contains(*span, Span::new(*start, *end)));
                if !self.walk(body, &mut next, Some(*span)) {
                    return false;
                }
                let mut closed = invariant.clone();
                closed.join(vec![invariant.clone(), next]);
                let keys: BTreeSet<_> = invariant
                    .values
                    .keys()
                    .chain(closed.values.keys())
                    .copied()
                    .collect();
                if keys.iter().any(|key| {
                    invariant.role(invariant.values.get(key).copied().unwrap_or(Value::Unknown))
                        != closed.role(closed.values.get(key).copied().unwrap_or(Value::Unknown))
                }) {
                    return false;
                }
                *state = closed;
                state.last_span = Some(*span);
                true
            }
            GrammarNode::Choice { arms, span, .. } => {
                if arms.is_empty() {
                    return false;
                }
                let mut results = Vec::new();
                for (index, arm) in arms.iter().enumerate() {
                    let mut branch = state.clone();
                    if !self.walk(&arm.body, &mut branch, active_loop) {
                        return false;
                    }
                    if let Some(region) = self
                        .ctx
                        .branches
                        .iter()
                        .find(|(at, _)| at == span)
                        .and_then(|(_, arms)| arms.get(index))
                    {
                        let mut definitions: Vec<_> = self
                            .ctx
                            .definitions
                            .iter()
                            .filter(|d| {
                                self.relevant.contains(&d.symbol)
                                    && contains(*region, d.span)
                                    && smallest(&self.ctx.functions, d.span)
                                        == smallest(&self.ctx.functions, *span)
                                    && !branch.seen_defs.contains(&(d.span.start, d.span.end))
                                    && branch.last_span.is_none_or(|last| d.span.start >= last.end)
                            })
                            .collect();
                        definitions.sort_by_key(|d| d.span.end);
                        for d in definitions {
                            // Nested guards are handled only by their own
                            // represented choice, never replayed lexically.
                            if self.ctx.guarded_regions.iter().any(|guard| {
                                contains(*region, *guard)
                                    && *guard != *region
                                    && contains(*guard, d.span)
                            }) {
                                continue;
                            }
                            let value = self.value(&d.value, &mut branch, active_loop, 0);
                            branch.values.insert(d.symbol, value);
                            branch.origins.insert(d.symbol, d.span.start);
                            branch.seen_defs.insert((d.span.start, d.span.end));
                            self.allowed_definitions.insert((d.span.start, d.span.end));
                            if let Some(target) = d.target {
                                self.allowed.insert((target.start, target.end));
                            }
                        }
                    }
                    if let Some((_, yes, no)) =
                        self.ctx.conditionals.iter().find(|(at, _, _)| at == span)
                    {
                        let result = self.value(
                            if index == 0 { yes } else { no },
                            &mut branch,
                            active_loop,
                            0,
                        );
                        branch.results.insert((span.start, span.end), result);
                        self.bind_result(*span, &mut branch, active_loop);
                    }
                    results.push(branch);
                }
                state.join(results);
                true
            }
            GrammarNode::Number(w) | GrammarNode::String(w) => {
                if w.via_helper.is_some() {
                    return false;
                }
                let Some(call) = self.ctx.calls.iter().find(|c| c.span == w.span) else {
                    return false;
                };
                let position = if matches!(n, GrammarNode::Number(_)) {
                    3
                } else {
                    2
                } + usize::from(call.receiverless);
                let Some(e) = call.args.get(position) else {
                    return false;
                };
                let value = self.value(e, state, active_loop, 0);
                if !state.current(value) {
                    return false;
                }
                state.advance(false);
                state
                    .results
                    .insert((w.span.start, w.span.end), Value::Current(state.epoch));
                state.last_span = Some(w.span);
                for d in self.ctx.definitions.iter().filter(|d| d.span == w.span) {
                    self.allowed_definitions.insert((d.span.start, d.span.end));
                    self.used.insert(d.symbol);
                    if let Some(target) = d.target {
                        self.allowed.insert((target.start, target.end));
                    }
                    state.values.insert(d.symbol, Value::Current(state.epoch));
                    state.origins.insert(d.symbol, d.span.start);
                }
                self.bind_result(w.span, state, active_loop);
                true
            }
            GrammarNode::Byte(w) => self.byte(w, state, active_loop),
            _ => false,
        }
    }
    // A caller assignment completes after its RHS writer/conditional. Cache
    // that value before another physical write invalidates the return cursor.
    fn bind_result(&mut self, span: Span, state: &mut State, active_loop: Option<Span>) {
        for d in &self.ctx.definitions {
            if !contains(d.span, span) || state.seen_defs.contains(&(d.span.start, d.span.end)) {
                continue;
            }
            if !result_expression(&d.value, span) {
                continue;
            }
            let value = self.value(&d.value, state, active_loop, 0);
            if value == Value::Unknown {
                continue;
            }
            state.seen_defs.insert((d.span.start, d.span.end));
            self.allowed_definitions.insert((d.span.start, d.span.end));
            if let Some(target) = d.target {
                self.allowed.insert((target.start, target.end));
            }
            state.values.insert(d.symbol, value);
            state.origins.insert(d.symbol, d.span.start);
        }
    }
    fn byte(&mut self, w: &WriteNode, state: &mut State, active_loop: Option<Span>) -> bool {
        let literal = w
            .position
            .as_deref()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .map(Address::Constant);
        let Some(e) = self
            .ctx
            .indices
            .iter()
            .find(|i| i.span == w.span)
            .map(|i| &i.address)
            .or_else(|| {
                // The extractor proved the inline helper's single store and
                // retained its exact CALLER index argument, including effects.
                let span = w.position_span?;
                let call = self.ctx.calls.iter().find(|call| call.span == w.span)?;
                let argument = call
                    .arg_spans
                    .iter()
                    .position(|argument| *argument == Some(Span::new(span.start, span.end)))?;
                call.args.get(argument)
            })
            .or(literal.as_ref())
        else {
            return false;
        };
        let value = if let Address::Increment(symbol, at) | Address::PostIncrement(symbol, at) = e {
            let value = self.value(&Address::Read(*symbol, *at), state, active_loop, 0);
            let post = matches!(e, Address::PostIncrement(..));
            if !(if post {
                state.next(value)
            } else {
                state.current(value)
            }) {
                return false;
            }
            self.allowed.insert((at.start, at.end));
            for definition in self.ctx.definitions.iter().filter(|definition| {
                definition.target == Some(*at) && matches!(definition.value, Address::Advance(_))
            }) {
                self.allowed_definitions
                    .insert((definition.span.start, definition.span.end));
            }
            state.values.insert(
                *symbol,
                if post {
                    Value::Offset(state.epoch, 2)
                } else {
                    Value::Next(state.epoch)
                },
            );
            Value::Next(state.epoch)
        } else {
            self.value(e, state, active_loop, 0)
        };
        if !state.next(value) {
            return false;
        }
        state.advance(true);
        state.last_span = Some(w.span);
        true
    }
}
pub(super) fn validate(source: &str, emission: &EmissionContract) -> bool {
    validate_in_scope(source, emission, None, None)
}
pub(super) fn validate_in_scope(
    source: &str,
    emission: &EmissionContract,
    root_scope: Option<&super::control::RootScope>,
    plan: Option<&super::SourcePlan>,
) -> bool {
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return false;
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut ctx = Context {
        scopes: semantic.scoping(),
        definitions: vec![],
        calls: vec![],
        indices: vec![],
        functions: vec![],
        blocks: vec![],
        guarded_regions: vec![],
        conditionals: vec![],
        branches: vec![],
    };
    ctx.visit_program(&parsed.program);
    if let Some(scope) = root_scope {
        ctx.definitions
            .retain(|definition| scope.includes(definition.span));
    }
    let mut relevant = BTreeSet::new();
    emission.grammar.walk(&mut |node| match node {
        GrammarNode::Byte(w) => {
            if let Some(i) = ctx.indices.iter().find(|i| i.span == w.span) {
                address_symbols(&i.address, &mut relevant);
            }
        }
        GrammarNode::Number(w) | GrammarNode::String(w) => {
            if let Some(c) = ctx.calls.iter().find(|c| c.span == w.span) {
                let index = if matches!(node, GrammarNode::Number(_)) {
                    3
                } else {
                    2
                } + usize::from(c.receiverless);
                if let Some(a) = c.args.get(index) {
                    address_symbols(a, &mut relevant);
                }
            }
        }
        _ => {}
    });
    loop {
        let size = relevant.len();
        for d in &ctx.definitions {
            if relevant.contains(&d.symbol) {
                address_symbols(&d.value, &mut relevant);
            }
        }
        if relevant.len() == size {
            break;
        }
    }
    let mut proof = Proof {
        ctx: &ctx,
        root: &emission.grammar,
        plan,
        used: BTreeSet::new(),
        allowed: BTreeSet::new(),
        allowed_definitions: BTreeSet::new(),
        steps: 65536,
        live_ranges: vec![],
        relevant,
    };
    let mut state = State::new();
    if !proof.walk(&emission.grammar, &mut state, None) {
        return false;
    }
    let mut prefix = None;
    emission.grammar.walk(&mut |n| {
        if let GrammarNode::RawPrefix(p) = n {
            if prefix.is_none() {
                prefix = Some(p.span);
            }
        }
    });
    let Some(_) = prefix else {
        return false;
    };
    let site = Span::new(emission.site.start, emission.site.end);
    let owner = smallest(&ctx.functions, site);
    let mut dead = Vec::new();
    emission.grammar.walk(&mut |n| {
        if let GrammarNode::Repeat {
            span,
            count: Some(0),
            ..
        } = n
        {
            dead.push(*span);
        }
    });
    proof.used.iter().all(|symbol| {
        if smallest(&ctx.functions, ctx.scopes.symbol_span(*symbol)).is_none() {
            return false;
        }
        if ctx.scopes.symbol_flags(*symbol).is_const_variable()
            && ctx
                .scopes
                .get_resolved_references(*symbol)
                .any(|r| r.is_write())
        {
            return false;
        }
        if ctx.definitions.iter().any(|d| {
            d.symbol == *symbol
                && proof.live_ranges.iter().any(|(s, start, end)| {
                    s == symbol && d.span.end > *start && d.span.start < *end
                })
                && !dead.iter().any(|s| contains(*s, d.span))
                && !proof
                    .allowed_definitions
                    .contains(&(d.span.start, d.span.end))
        }) {
            return false;
        }
        ctx.scopes
            .get_resolved_references(*symbol)
            .filter(|r| {
                r.is_write()
                    && root_scope.is_none_or(|scope| {
                        scope.includes(semantic.nodes().get_node(r.node_id()).span())
                    })
            })
            .all(|r| {
                let span = semantic.nodes().get_node(r.node_id()).span();
                if dead.iter().any(|s| contains(*s, span)) {
                    return true;
                }
                smallest(&ctx.functions, span) == owner
                    && (!proof.live_ranges.iter().any(|(s, start, end)| {
                        s == symbol && span.end > *start && span.start < *end
                    }) || proof.allowed.contains(&(span.start, span.end)))
            })
    })
}
