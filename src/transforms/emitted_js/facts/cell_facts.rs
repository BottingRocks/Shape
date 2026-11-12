//! Forward facts about successful lexical reads and exact stored values.
//! These facts justify forwarding; they never remove an evaluation themselves.
//! Unknown effects forget mutable values, but cannot undo lexical initialization.
use super::super::adjacent_ssa::LocalReads;
use crate::analysis;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};
use std::collections::{BTreeMap, BTreeSet};
use crate::transforms::emitted_js::support::unwrap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Token {
    Literal(String),
    Undefined,
    Argument(Span, u32),
    Immutable(SymbolId),
}
#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct Value {
    pub token: Option<Token>,
    pub primitive: bool,
    // Number/String/Boolean/null/undefined: numeric coercion cannot throw or
    // invoke hooks. BigInt and Symbol deliberately do not belong here.
    pub scalar: bool,
}
impl Value {
    fn scalar() -> Self {
        Self {
            token: None,
            primitive: true,
            scalar: true,
        }
    }
    fn primitive() -> Self {
        Self {
            token: None,
            primitive: true,
            scalar: false,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Cell {
    Local(SymbolId),
    Incoming(u32),
}
#[derive(Clone)]
struct State {
    initialized: BTreeSet<Cell>,
    values: BTreeMap<Cell, Value>,
    live: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            initialized: BTreeSet::new(),
            values: BTreeMap::new(),
            live: true,
        }
    }
}
impl State {
    fn merge(a: &Self, b: &Self) -> Self {
        if !a.live {
            return b.clone();
        }
        if !b.live {
            return a.clone();
        }
        Self {
            initialized: a
                .initialized
                .intersection(&b.initialized)
                .copied()
                .collect(),
            values: a
                .values
                .iter()
                .filter_map(|(k, v)| {
                    let other = b.values.get(k)?;
                    let value = Value {
                        token: v.token.clone().filter(|t| Some(t) == other.token.as_ref()),
                        primitive: v.primitive && other.primitive,
                        scalar: v.scalar && other.scalar,
                    };
                    (value.token.is_some() || value.primitive).then_some((*k, value))
                })
                .collect(),
            live: true,
        }
    }
}
#[derive(Default)]
pub(crate) struct Facts {
    pub ready: BTreeSet<Span>,
    pub values: BTreeMap<Span, Value>,
}
pub(crate) fn collect(
    source: &str,
    program: &Program<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    locals: &LocalReads,
    protected: &BTreeSet<String>,
    ordinary: bool,
) -> Facts {
    let names: BTreeSet<_> = nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::Function(f) => f.id.as_ref().map(|id| id.name.to_string()),
            _ => None,
        })
        .collect();
    let private: BTreeSet<_> = scoping
        .symbol_ids()
        .filter(|&symbol| {
            !protected.contains(scoping.symbol_name(symbol))
                && locals.private_cell(symbol)
                && !(scoping
                    .symbol_flags(symbol)
                    .is_function_scoped_declaration()
                    && names.contains(scoping.symbol_name(symbol)))
        })
        .collect();
    let mut scan = Scan {
        source,
        scoping,
        nodes,
        locals,
        protected,
        ordinary,
        private,
        ordinary_number: super::super::numeric_coercions::ordinary_number(program, scoping),
        state: State::default(),
        arguments: None,
        facts: Facts::default(),
    };
    scan.visit_program(program);
    scan.facts
}
struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    locals: &'s LocalReads,
    protected: &'s BTreeSet<String>,
    ordinary: bool,
    private: BTreeSet<SymbolId>,
    ordinary_number: bool,
    state: State,
    arguments: Option<Span>,
    facts: Facts,
}
impl<'a> Scan<'_, 'a> {
    fn cell(&self, id: &IdentifierReference<'_>) -> Option<Cell> {
        analysis::identifier_symbol(id, self.scoping)
            .map(Cell::Local)
            .or_else(|| {
                if !self.protected.contains(id.name.as_str()) {
                    return None;
                }
                id.name
                    .strip_prefix("$KEY_")?
                    .parse()
                    .ok()
                    .map(Cell::Incoming)
            })
    }
    fn value(&self, e: &Expression<'_>) -> Value {
        self.facts
            .values
            .get(&e.span())
            .cloned()
            .unwrap_or_default()
    }
    // An unknown effect can write shared or captured cells. It cannot assign
    // an uncaptured lexical cell; do not discard its stored-value/type facts.
    // Control-flow boundaries and unknown implicit stores still clear all.
    fn effect(&mut self) {
        self.state
            .values
            .retain(|cell, _| matches!(cell,Cell::Local(s)if self.private.contains(s)));
    }
    // Retain only private cells with no explicit or implicit store in a loop.
    // A syntactically earlier var initializer inside the loop is a backedge store.
    fn forget_loop(&mut self, region: Span) {
        self.effect();
        self.state.values.retain(|cell, _| {
            let Cell::Local(symbol) = cell else {
                return false;
            };
            let declaration = self.scoping.symbol_span(*symbol);
            !(region.start <= declaration.start && declaration.end <= region.end)
                && self.scoping.get_resolved_references(*symbol).all(|r| {
                    let at = self.nodes.get_node(r.node_id()).kind().span();
                    !(r.is_write() && region.start <= at.start && at.end <= region.end)
                })
        });
    }
    fn forget(&mut self) {
        self.state.values.clear();
    }
    fn statements(&mut self, body: &[Statement<'a>]) {
        for s in body {
            if !self.state.live {
                break;
            }
            self.visit_statement(s);
        }
    }
    fn immutable(&self, s: SymbolId, at: Span) -> bool {
        self.locals.initialized(s, at)
            && self.scoping.symbol_redeclarations(s).is_empty()
            && self
                .scoping
                .get_resolved_references(s)
                .all(|r| !r.is_write())
            && matches!(self.nodes.kind(self.scoping.symbol_declaration(s)),
                AstKind::VariableDeclarator(d) if d.init.is_some() && matches!(d.id, BindingPattern::BindingIdentifier(_)))
            && matches!(self.nodes.parent_kind(self.scoping.symbol_declaration(s)),
                AstKind::VariableDeclaration(d) if matches!(d.kind, VariableDeclarationKind::Let | VariableDeclarationKind::Const))
    }
    fn refine(&mut self, expression: &Expression<'_>, yes: bool) {
        let Expression::BinaryExpression(b) = unwrap(expression) else {
            return;
        };
        let equal = match b.operator {
            BinaryOperator::StrictEquality | BinaryOperator::Equality => yes,
            BinaryOperator::StrictInequality | BinaryOperator::Inequality => !yes,
            _ => return,
        };
        if !equal {
            return;
        }
        for (value, literal) in [(&b.left, &b.right), (&b.right, &b.left)] {
            if let (Expression::UnaryExpression(u), Expression::StringLiteral(kind)) =
                (unwrap(value), unwrap(literal))
            {
                if u.operator != UnaryOperator::Typeof {
                    continue;
                }
                let Expression::Identifier(id) = unwrap(&u.argument) else {
                    continue;
                };
                if !matches!(
                    kind.value.as_str(),
                    "number" | "string" | "boolean" | "bigint" | "symbol"
                ) {
                    continue;
                }
                if let Some(cell) = self.cell(id) {
                    self.state.initialized.insert(cell);
                    self.state.values.insert(
                        cell,
                        if matches!(kind.value.as_str(), "number" | "string" | "boolean") {
                            Value::scalar()
                        } else {
                            Value::primitive()
                        },
                    );
                }
            } else if let Expression::Identifier(id) = unwrap(value) {
                let Some(cell) = self.cell(id) else {
                    continue;
                };
                if matches!(
                    b.operator,
                    BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
                ) && matches!(
                    unwrap(literal),
                    Expression::NumericLiteral(_)
                        | Expression::StringLiteral(_)
                        | Expression::BooleanLiteral(_)
                        | Expression::NullLiteral(_)
                        | Expression::BigIntLiteral(_)
                ) {
                    // Strict equality identifies +0 and -0. It cannot prove
                    // their exact stored value (division/Object.is can tell).
                    let value = if matches!(unwrap(literal),Expression::NumericLiteral(n)if n.value==0.0)
                    {
                        Value::scalar()
                    } else {
                        self.value(literal)
                    };
                    self.state.values.insert(cell, value);
                }
            }
        }
    }
    fn result(&self, e: &Expression<'_>) -> Value {
        match e {
            Expression::ParenthesizedExpression(p) => self.value(&p.expression),
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_) => Value {
                token: Some(Token::Literal(
                    self.source[e.span().start as usize..e.span().end as usize].into(),
                )),
                primitive: true,
                scalar: !matches!(e, Expression::BigIntLiteral(_)),
            },
            Expression::Identifier(id) => {
                // Identifier facts were saved at GetValue, before later child effects.
                self.facts.values.get(&id.span).cloned().unwrap_or_default()
            }
            Expression::UnaryExpression(u) => Value {
                token: if u.operator == UnaryOperator::Void {
                    Some(Token::Undefined)
                } else if matches!(
                    u.operator,
                    UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation
                ) && matches!(unwrap(&u.argument), Expression::NumericLiteral(_))
                {
                    Some(Token::Literal(
                        self.source[e.span().start as usize..e.span().end as usize].into(),
                    ))
                } else {
                    None
                },
                primitive: true,
                scalar: matches!(
                    u.operator,
                    UnaryOperator::UnaryPlus
                        | UnaryOperator::LogicalNot
                        | UnaryOperator::Typeof
                        | UnaryOperator::Void
                        | UnaryOperator::Delete
                ) || self.value(&u.argument).scalar,
            },
            Expression::BinaryExpression(b) => Value {
                token: None,
                primitive: true,
                scalar: b.operator.is_compare()
                    || b.operator.is_equality()
                    || b.operator.is_in()
                    || b.operator.is_instance_of()
                    || b.operator == BinaryOperator::ShiftRightZeroFill
                    || self.value(&b.left).scalar
                    || self.value(&b.right).scalar,
            },
            Expression::UpdateExpression(u) => Value {
                token: None,
                primitive: true,
                scalar: self
                    .facts
                    .values
                    .get(&u.argument.span())
                    .is_some_and(|v| v.scalar),
            },
            Expression::TemplateLiteral(_) => Value::scalar(),
            Expression::CallExpression(_)
                if self.ordinary_number
                    && super::super::numeric_coercions::number_argument(e, self.scoping).is_some() =>
            {
                Value::scalar()
            }
            Expression::LogicalExpression(l) => {
                let a = self.value(&l.left);
                let b = self.value(&l.right);
                Value {
                    token: a.token.clone().filter(|t| Some(t) == b.token.as_ref()),
                    primitive: a.primitive && b.primitive,
                    scalar: a.scalar && b.scalar,
                }
            }
            Expression::ConditionalExpression(c) => {
                let a = self.value(&c.consequent);
                let b = self.value(&c.alternate);
                Value {
                    token: a.token.clone().filter(|t| Some(t) == b.token.as_ref()),
                    primitive: a.primitive && b.primitive,
                    scalar: a.scalar && b.scalar,
                }
            }
            Expression::SequenceExpression(s) => s
                .expressions
                .last()
                .map(|e| self.value(e))
                .unwrap_or_default(),
            Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
                self.value(&a.right)
            }
            Expression::ComputedMemberExpression(m)
                if self.arguments.is_some()
                    && self.ordinary
                    && super::super::argument_snapshots::input(e)
                    && matches!(unwrap(&m.object), Expression::Identifier(id) if analysis::identifier_symbol(id,self.scoping).is_none()) =>
            {
                let Expression::NumericLiteral(n) = unwrap(&m.expression) else {
                    return Value::default();
                };
                Value {
                    token: Some(Token::Argument(self.arguments.unwrap(), n.value as u32)),
                    primitive: false,
                    scalar: false,
                }
            }
            _ => Value::default(),
        }
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_expression(&mut self, e: &Expression<'a>) {
        walk::walk_expression(self, e);
        let value = self.result(e);
        self.facts.values.insert(e.span(), value);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let Some(cell) = self.cell(id) else {
            // Unknown global lookup can invoke an accessor.
            if analysis::identifier_symbol(id, self.scoping).is_none()
                && !(self.arguments.is_some() && id.name == "arguments")
            {
                self.effect();
            }
            return;
        };
        if !id
            .reference_id
            .get()
            .is_some_and(|r| self.scoping.get_reference(r).is_read())
        {
            return;
        }
        let mut value = self.state.values.get(&cell).cloned().unwrap_or_default();
        if let Cell::Local(s) = cell {
            if self.immutable(s, id.span)
                && !matches!(value.token, Some(Token::Literal(_) | Token::Undefined))
            {
                value.token = Some(Token::Immutable(s));
            }
            value.primitive |= self.locals.primitive_cell(s, id.span);
            value.scalar |= self.locals.primitive_cell(s, id.span);
        }
        self.facts.values.insert(id.span, value);
        self.state.initialized.insert(cell);
    }
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if let Some(Expression::Identifier(id)) = d.init.as_ref().map(unwrap) {
            if self
                .cell(id)
                .is_some_and(|c| self.state.initialized.contains(&c))
            {
                self.facts.ready.insert(d.span);
            }
        }
        if !matches!(d.id, BindingPattern::BindingIdentifier(_)) {
            if let Some(init) = &d.init {
                self.visit_expression(init);
            }
            self.forget();
            return;
        }
        walk::walk_variable_declarator(self, d);
        if let BindingPattern::BindingIdentifier(id) = &d.id {
            if let Some(s) = id.symbol_id.get() {
                // No initializer on var is not an execution-time store.
                if let Some(init) = &d.init {
                    self.state.values.insert(Cell::Local(s), self.value(init));
                    self.state.initialized.insert(Cell::Local(s));
                } else {
                    let declaration = self.nodes.parent_id(self.scoping.symbol_declaration(s));
                    if matches!(self.nodes.kind(declaration),AstKind::VariableDeclaration(v)if v.kind==VariableDeclarationKind::Let)
                        && matches!(
                            self.nodes.parent_kind(declaration),
                            AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
                        )
                    {
                        self.state.values.insert(
                            Cell::Local(s),
                            Value {
                                token: Some(Token::Undefined),
                                primitive: true,
                                scalar: true,
                            },
                        );
                        self.state.initialized.insert(Cell::Local(s));
                    }
                }
            }
        } else {
            self.forget();
        }
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        if a.operator != AssignmentOperator::Assign && !a.operator.is_logical() {
            // Get the old value before evaluating RHS. Only the final successful
            // result supplies a new type; the coercion itself remains ordered.
            self.visit_assignment_target(&a.left);
            let old = self
                .facts
                .values
                .get(&a.left.span())
                .cloned()
                .unwrap_or_default();
            self.visit_expression(&a.right);
            let right = self.value(&a.right);
            self.effect();
            if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                if let Some(cell) = self.cell(id) {
                    self.state.values.insert(
                        cell,
                        Value {
                            token: None,
                            primitive: true,
                            scalar: old.scalar
                                || right.scalar
                                || a.operator == AssignmentOperator::ShiftRightZeroFill,
                        },
                    );
                    self.state.initialized.insert(cell);
                }
            }
            return;
        }
        if a.operator != AssignmentOperator::Assign {
            // Logical assignments remain conditional stores.
            self.visit_assignment_target(&a.left);
            let before = self.state.clone();
            self.visit_expression(&a.right);
            if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                if let Some(cell) = self.cell(id) {
                    self.state.values.insert(cell, self.value(&a.right));
                    self.state.initialized.insert(cell);
                } else {
                    self.forget();
                }
            } else {
                self.effect();
            }
            self.state = State::merge(&before, &self.state);
            return;
        }
        if matches!(
            a.left,
            AssignmentTarget::ArrayAssignmentTarget(_)
                | AssignmentTarget::ObjectAssignmentTarget(_)
        ) {
            self.visit_expression(&a.right);
            self.forget();
            return;
        }
        self.visit_assignment_target(&a.left);
        self.visit_expression(&a.right);
        if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
            if let Some(cell) = self.cell(id) {
                self.state.values.insert(cell, self.value(&a.right));
                self.state.initialized.insert(cell);
            } else {
                self.forget();
            }
        } else {
            self.effect();
        }
    }
    fn visit_update_expression(&mut self, u: &UpdateExpression<'a>) {
        walk::walk_update_expression(self, u);
        let scalar = self
            .facts
            .values
            .get(&u.argument.span())
            .is_some_and(|v| v.scalar);
        self.effect();
        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument {
            if let Some(cell) = self.cell(id) {
                self.state.values.insert(
                    cell,
                    Value {
                        token: None,
                        primitive: true,
                        scalar,
                    },
                );
            }
        }
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        // typeof can suppress an unresolved read; delete never performs GetValue.
        if matches!(u.operator, UnaryOperator::Typeof | UnaryOperator::Delete)
            && matches!(unwrap(&u.argument), Expression::Identifier(_))
        {
            if let Expression::Identifier(id) = unwrap(&u.argument) {
                match analysis::identifier_symbol(id, self.scoping) {
                    Some(s)
                        if u.operator == UnaryOperator::Typeof
                            && self.locals.initialized(s, id.span) =>
                    {
                        self.visit_expression(&u.argument)
                    }
                    None if !(self.arguments.is_some() && id.name == "arguments") => {
                        // typeof can invoke a global getter; deleting a
                        // global reference can execute environment traps.
                        self.effect();
                    }
                    _ => {}
                }
            }
            return;
        }
        self.visit_expression(&u.argument);
        if !matches!(
            u.operator,
            UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
        ) && !self.value(&u.argument).primitive
        {
            self.effect();
        }
    }
    fn visit_binary_expression(&mut self, b: &BinaryExpression<'a>) {
        walk::walk_binary_expression(self, b);
        if !(matches!(
            b.operator,
            BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
        ) || matches!(
            b.operator,
            BinaryOperator::Equality | BinaryOperator::Inequality
        ) && (matches!(unwrap(&b.left), Expression::NullLiteral(_))
            || matches!(unwrap(&b.right), Expression::NullLiteral(_)))
            || !b.operator.is_in()
                && !b.operator.is_instance_of()
                && self.value(&b.left).primitive
                && self.value(&b.right).primitive)
        {
            self.effect();
        }
    }
    fn visit_if_statement(&mut self, i: &IfStatement<'a>) {
        self.visit_expression(&i.test);
        let before = self.state.clone();
        self.refine(&i.test, true);
        self.visit_statement(&i.consequent);
        let yes = self.state.clone();
        self.state = before;
        self.refine(&i.test, false);
        if let Some(no) = &i.alternate {
            self.visit_statement(no);
        }
        self.state = State::merge(&yes, &self.state);
    }
    fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'a>) {
        self.visit_expression(&c.test);
        let before = self.state.clone();
        self.refine(&c.test, true);
        self.visit_expression(&c.consequent);
        let yes = self.state.clone();
        self.state = before;
        self.refine(&c.test, false);
        self.visit_expression(&c.alternate);
        self.state = State::merge(&yes, &self.state);
    }
    fn visit_logical_expression(&mut self, l: &LogicalExpression<'a>) {
        self.visit_expression(&l.left);
        let before = self.state.clone();
        match l.operator {
            LogicalOperator::And => self.refine(&l.left, true),
            LogicalOperator::Or => self.refine(&l.left, false),
            _ => {}
        }
        self.visit_expression(&l.right);
        self.state = State::merge(&before, &self.state);
    }
    // Defaults/destructuring are conditional, and must not establish facts for
    // later parameters or the body. Emitted threads have no formal parameters.
    fn visit_formal_parameters(&mut self, _: &FormalParameters<'a>) {}
    fn visit_catch_clause(&mut self, c: &CatchClause<'a>) {
        self.forget();
        self.visit_block_statement(&c.body);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.statements(&b.statements);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.statements(&b.body);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        let state = std::mem::take(&mut self.state);
        let args = self.arguments;
        self.arguments = super::super::dead_ssa::readonly_arguments(f).then_some(f.span);
        walk::walk_function(self, f, flags);
        self.state = state;
        self.arguments = args;
        if f.r#type == FunctionType::FunctionDeclaration {
            if let Some(id) = &f.id {
                // Annex-B block declarations can write an outer var without
                // an IdentifierReference store. Forget every matching cell.
                let incoming = id
                    .name
                    .strip_prefix("$KEY_")
                    .and_then(|n| n.parse::<u32>().ok());
                self.state.values.retain(|cell, _| match cell {
                    Cell::Local(s) => self.scoping.symbol_name(*s) != id.name,
                    Cell::Incoming(n) => incoming != Some(*n),
                });
            }
        }
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        let state = std::mem::take(&mut self.state);
        let args = self.arguments;
        self.arguments = None;
        walk::walk_arrow_function_expression(self, f);
        self.state = state;
        self.arguments = args;
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        // Class definition can run computed keys, heritage getters and static code.
        // Its nested executions do not inherit forward facts from the creator.
        let state = std::mem::take(&mut self.state);
        let args = self.arguments;
        self.arguments = None;
        walk::walk_class(self, c);
        self.state = state;
        self.arguments = args;
        self.effect();
    }
    fn visit_return_statement(&mut self, r: &ReturnStatement<'a>) {
        if let Some(e) = &r.argument {
            self.visit_expression(e);
        }
        self.state.live = false;
    }
    fn visit_throw_statement(&mut self, r: &ThrowStatement<'a>) {
        self.visit_expression(&r.argument);
        self.state.live = false;
    }
    fn visit_break_statement(&mut self, _: &BreakStatement<'a>) {
        self.state.live = false;
    }
    fn visit_continue_statement(&mut self, _: &ContinueStatement<'a>) {
        self.state.live = false;
    }
    fn visit_while_statement(&mut self, s: &WhileStatement<'a>) {
        self.forget_loop(s.span);
        self.visit_expression(&s.test);
        let exit = self.state.clone();
        self.visit_statement(&s.body);
        self.state = exit;
        self.forget_loop(s.span);
    }
    fn visit_for_statement(&mut self, s: &ForStatement<'a>) {
        if let Some(init) = &s.init {
            self.visit_for_statement_init(init);
        }
        self.forget_loop(s.span);
        if let Some(test) = &s.test {
            self.visit_expression(test);
        }
        let exit = self.state.clone();
        self.visit_statement(&s.body);
        // The update can also be reached through continues; no body facts survive.
        self.state = exit.clone();
        self.forget_loop(s.span);
        if let Some(update) = &s.update {
            self.visit_expression(update);
        }
        self.state = exit;
        self.forget_loop(s.span);
    }
    fn visit_do_while_statement(&mut self, s: &DoWhileStatement<'a>) {
        let before = self.state.clone();
        self.forget_loop(s.span);
        self.visit_statement(&s.body);
        self.state = before.clone();
        self.forget_loop(s.span);
        self.visit_expression(&s.test);
        self.state = before;
        self.forget_loop(s.span);
    }
    fn visit_for_in_statement(&mut self, s: &ForInStatement<'a>) {
        self.visit_expression(&s.right);
        self.forget_loop(s.span);
        let before = self.state.clone();
        self.visit_statement(&s.body);
        self.state = before;
        self.forget_loop(s.span);
    }
    fn visit_for_of_statement(&mut self, s: &ForOfStatement<'a>) {
        self.visit_expression(&s.right);
        self.forget_loop(s.span);
        let before = self.state.clone();
        self.visit_for_statement_left(&s.left);
        self.visit_statement(&s.body);
        self.state = before;
        self.forget_loop(s.span);
    }
    fn visit_try_statement(&mut self, s: &TryStatement<'a>) {
        let before = self.state.clone();
        self.visit_block_statement(&s.block);
        let normal = self.state.clone();
        if let Some(handler) = &s.handler {
            // A catch can begin at any throwing prefix. Never import the
            // normal end-of-try values or newly initialized cells into it.
            self.state = before.clone();
            self.forget();
            self.visit_catch_clause(handler);
            self.state = State::merge(&normal, &self.state);
        }
        if let Some(finalizer) = &s.finalizer {
            let normal = self.state.clone();
            // Finalizers also execute for pending returns/throws. Collect
            // point facts from this conservative entry, then compute only
            // normal continuation state without exporting narrower facts
            // back into the finalizer's source positions.
            self.state = before;
            self.forget();
            self.visit_block_statement(finalizer);
            let conservative_facts = std::mem::take(&mut self.facts);
            self.state = normal;
            self.visit_block_statement(finalizer);
            self.facts = conservative_facts;
        }
    }
    fn visit_switch_statement(&mut self, s: &SwitchStatement<'a>) {
        self.visit_expression(&s.discriminant);
        let before = self.state.clone();
        for case in &s.cases {
            self.state = before.clone();
            self.forget();
            if let Some(test) = &case.test {
                self.visit_expression(test);
            }
            // Fallthrough bypasses the case test. Its reads cannot dominate a case body.
            self.state = before.clone();
            self.forget();
            self.statements(&case.consequent);
        }
        self.state = before;
        self.forget();
    }
    fn visit_labeled_statement(&mut self, s: &LabeledStatement<'a>) {
        let before = self.state.clone();
        self.forget();
        self.visit_statement(&s.body);
        self.state = before;
        self.forget();
    }
    fn visit_chain_expression(&mut self, c: &ChainExpression<'a>) {
        // Conditional operands cannot establish definite facts after the chain.
        let before = self.state.clone();
        self.forget();
        walk::walk_chain_expression(self, c);
        self.state = before;
        self.forget();
    }
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        walk::walk_call_expression(self, c);
        self.effect();
    }
    fn visit_new_expression(&mut self, c: &NewExpression<'a>) {
        walk::walk_new_expression(self, c);
        self.effect();
    }
    fn visit_static_member_expression(&mut self, m: &StaticMemberExpression<'a>) {
        walk::walk_static_member_expression(self, m);
        self.effect();
    }
    fn visit_computed_member_expression(&mut self, m: &ComputedMemberExpression<'a>) {
        walk::walk_computed_member_expression(self, m);
        self.effect();
    }
    fn visit_tagged_template_expression(&mut self, t: &TaggedTemplateExpression<'a>) {
        walk::walk_tagged_template_expression(self, t);
        self.effect();
    }
    fn visit_spread_element(&mut self, e: &SpreadElement<'a>) {
        walk::walk_spread_element(self, e);
        self.effect();
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        self.visit_property_key(&p.key);
        if p.computed {
            self.effect();
        }
        self.visit_expression(&p.value);
    }
    fn visit_await_expression(&mut self, e: &AwaitExpression<'a>) {
        walk::walk_await_expression(self, e);
        self.effect();
    }
    fn visit_yield_expression(&mut self, e: &YieldExpression<'a>) {
        walk::walk_yield_expression(self, e);
        self.effect();
    }
    fn visit_template_literal(&mut self, t: &TemplateLiteral<'a>) {
        for e in &t.expressions {
            self.visit_expression(e);
            if !self.value(e).primitive {
                self.effect();
            }
        }
    }
}
