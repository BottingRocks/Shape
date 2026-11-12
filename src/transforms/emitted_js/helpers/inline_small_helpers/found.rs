//! Which helpers are small enough to expand: their shape and body.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Shape {
    /// `return E;`: usable as an expression.
    Expression,
    /// Statements (and a pure final return): usable as a statement.
    Statements,
}

pub(super) struct Helper {
    pub(super) declaration: Span,
    pub(super) shape: Shape,
    /// The body pieces to emit: the return expression, or the statements.
    pub(super) pieces: Vec<Span>,
    pub(super) parameters: usize,
    /// Parameter reads in source order: (span, parameter index).
    pub(super) uses: Vec<(Span, usize)>,
    /// Body nodes that can run code, as (start, end).
    pub(super) effects: Vec<(u32, u32)>,
    /// The effects that remain once property reads and stores run no code
    /// (no getters or setters): calls, conversions, stores to names.
    pub(super) code: Vec<(u32, u32)>,
    /// Names read from outside the helper.
    pub(super) free: Vec<(String, Option<SymbolId>)>,
    /// The body's own expression for `(0, E)` when expanded as a callee.
    pub(super) member: bool,
    /// What a statement-shaped helper returns: a parameter or an empty
    /// literal (`None` is `undefined`).
    pub(super) value: Option<Span>,
    /// Parameters the body assigns: their arguments always get temporaries.
    pub(super) written: Vec<bool>,
    /// The body's own `let` bindings and their reads, as (span, local
    /// index): each call gives them fresh names.
    pub(super) locals: Vec<(Span, usize)>,
    pub(super) local_count: usize,
    /// The returned value only reads names (`[t, p]`): dropped as a statement.
    pub(super) reads_only: bool,
}

pub(super) struct Found<'s> {
    pub(super) source: &'s str,
    pub(super) scoping: &'s Scoping,
    pub(super) comments: &'s [Span],
    pub(super) helpers: BTreeMap<SymbolId, Helper>,
    pub(super) dynamic: bool,
    pub(super) exact: bool,
    /// Helper declarations nothing references (collected in exact mode).
    pub(super) unused: Vec<Span>,
}

impl Found<'_> {
    pub(super) fn helper(&self, function: &Function<'_>) -> Option<(SymbolId, Helper)> {
        let id = function.id.as_ref()?;
        let symbol = id.symbol_id.get()?;
        if !(id.name.starts_with("$VMHELPER_") || id.name.starts_with("vm_sub_"))
            || function.r#async
            || function.generator
            || function.params.rest.is_some()
            || self.scoping.symbol_is_mutated(symbol)
            || self.comments.iter().any(|c| function.span.start <= c.start && c.end <= function.span.end)
        {
            return None;
        }
        let body = function.body.as_ref()?;
        if !body.directives.is_empty() {
            return None;
        }
        let mut parameters = Vec::new();
        let mut written = Vec::new();
        for parameter in &function.params.items {
            let BindingPattern::BindingIdentifier(binding) = &parameter.pattern else {
                return None;
            };
            let parameter_symbol = binding.symbol_id.get()?;
            let mutated = self.scoping.symbol_is_mutated(parameter_symbol);
            if mutated && !self.exact {
                return None;
            }
            written.push(mutated);
            parameters.push(parameter_symbol);
        }
        let max_statements = if self.exact { usize::MAX - 1 } else { MAX_STATEMENTS };
        let mut value = None;
        let (shape, pieces, member) = match body.statements.as_slice() {
            // An empty helper does nothing where it is a statement.
            [] => (Shape::Statements, Vec::new(), false),
            [Statement::ReturnStatement(ret)] => {
                let argument = ret.argument.as_ref()?;
                // Tuple returns belong to the tuple expansion; an array of
                // literals (or none) is a plain factory.
                if !self.exact && matches!(argument, Expression::ArrayExpression(array) if !array.elements.iter().all(|element| {
                    matches!(
                        element,
                        ArrayExpressionElement::NumericLiteral(_)
                            | ArrayExpressionElement::StringLiteral(_)
                            | ArrayExpressionElement::BooleanLiteral(_)
                            | ArrayExpressionElement::NullLiteral(_)
                    )
                })) {
                    return None;
                }
                (Shape::Expression, vec![argument.span()], argument.is_member_expression())
            }
            statements if statements.len() <= max_statements + 1 => {
                let (last, rest) = statements.split_last()?;
                let mut pieces = Vec::new();
                for statement in rest {
                    match statement {
                        Statement::ExpressionStatement(expression) => pieces.push(expression.span),
                        Statement::VariableDeclaration(declaration)
                            if self.exact
                                && matches!(declaration.kind, VariableDeclarationKind::Let | VariableDeclarationKind::Const)
                                && declaration.declarations.iter().all(|declarator| {
                                    matches!(declarator.id, BindingPattern::BindingIdentifier(_)) && declarator.init.is_some()
                                }) =>
                        {
                            pieces.push(declaration.span)
                        }
                        _ => return None,
                    }
                }
                match last {
                    Statement::ExpressionStatement(expression) => pieces.push(expression.span),
                    Statement::ReturnStatement(ret)
                        if self.exact || ret.argument.as_ref().is_none_or(|a| {
                            matches!(a, Expression::Identifier(_))
                                || matches!(a, Expression::ArrayExpression(array) if array.elements.is_empty())
                                || matches!(a, Expression::ObjectExpression(object) if object.properties.is_empty())
                        }) =>
                    {
                        value = ret.argument.as_ref().map(GetSpan::span);
                    }
                    _ => return None,
                }
                if pieces.len() > max_statements {
                    return None;
                }
                (Shape::Statements, pieces, false)
            }
            _ => return None,
        };
        let mut scan = BodyScan { bindings: self.exact.then(Vec::new), ..BodyScan::default() };
        scan.visit_function_body(body);
        if !scan.closed {
            return None;
        }
        let bindings = scan.bindings.unwrap_or_default();
        let mut locals: Vec<(Span, usize)> =
            bindings.iter().enumerate().map(|(index, (span, _))| (*span, index)).collect();
        let mut uses = Vec::new();
        let mut free = Vec::new();
        for (span, reference) in scan.references {
            let target = self.scoping.get_reference(reference).symbol_id();
            if let Some(index) = target.and_then(|t| bindings.iter().position(|(_, b)| *b == t)) {
                locals.push((span, index));
                continue;
            }
            match target.and_then(|t| parameters.iter().position(|p| *p == t)) {
                Some(index) => uses.push((span, index)),
                None => free.push((self.source[span.start as usize..span.end as usize].to_string(), target)),
            }
        }
        uses.sort_by_key(|(span, _)| span.start);
        locals.sort_by_key(|(span, _)| span.start);
        let reads_only = value.is_some_and(|span| {
            body.statements.last().is_some_and(|last| {
                matches!(last, Statement::ReturnStatement(ret) if ret.argument.as_ref().is_some_and(|a| a.span() == span
                    && matches!(a, Expression::ArrayExpression(array) if array.elements.iter().all(|element| matches!(element, ArrayExpressionElement::Identifier(_))))))
            })
        });
        let mut effects = scan.effects;
        effects.sort_unstable();
        let mut code = scan.code;
        code.sort_unstable();
        Some((
            symbol,
            Helper {
                declaration: function.span,
                shape,
                pieces,
                parameters: parameters.len(),
                uses,
                effects,
                code,
                free,
                member,
                value,
                written,
                local_count: bindings.len(),
                locals,
                reads_only,
            },
        ))
    }
}

impl<'a> Visit<'a> for Found<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some((symbol, helper)) = self.helper(function) {
            self.helpers.insert(symbol, helper);
        }
        if self.exact && function.is_declaration() {
            if let Some(id) = &function.id {
                if (id.name.starts_with("$VMHELPER_") || id.name.starts_with("vm_sub_"))
                    && id
                        .symbol_id
                        .get()
                        .is_some_and(|symbol| self.scoping.get_resolved_reference_ids(symbol).is_empty())
                {
                    self.unused.push(function.span);
                }
            }
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic = true;
        }
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

/// A helper body's references and effect nodes; rejects what cannot move.
pub(super) struct BodyScan {
    pub(super) closed: bool,
    /// The body's own bindings, when they are allowed (renamed per call).
    pub(super) bindings: Option<Vec<(Span, SymbolId)>>,
    pub(super) references: Vec<(Span, oxc_semantic::ReferenceId)>,
    pub(super) effects: Vec<(u32, u32)>,
    pub(super) code: Vec<(u32, u32)>,
}

impl Default for BodyScan {
    fn default() -> Self {
        Self {
            closed: true,
            bindings: None,
            references: Vec::new(),
            effects: Vec::new(),
            code: Vec::new(),
        }
    }
}

impl<'a> Visit<'a> for BodyScan {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "arguments" {
            self.closed = false;
        }
        if let Some(reference) = identifier.reference_id.get() {
            self.references.push((identifier.span, reference));
        }
    }
    fn visit_this_expression(&mut self, _: &ThisExpression) {
        self.closed = false;
    }
    fn visit_super(&mut self, _: &Super) {
        self.closed = false;
    }
    fn visit_meta_property(&mut self, _: &MetaProperty<'a>) {
        self.closed = false;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {
        self.closed = false;
    }
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {
        self.closed = false;
    }
    fn visit_class(&mut self, _: &Class<'a>) {
        self.closed = false;
    }
    fn visit_await_expression(&mut self, _: &AwaitExpression<'a>) {
        self.closed = false;
    }
    fn visit_yield_expression(&mut self, _: &YieldExpression<'a>) {
        self.closed = false;
    }
    fn visit_binding_identifier(&mut self, binding: &BindingIdentifier<'a>) {
        // A local binding needs a fresh name at each call.
        match (&mut self.bindings, binding.symbol_id.get()) {
            (Some(bindings), Some(symbol)) => bindings.push((binding.span, symbol)),
            _ => self.closed = false,
        }
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        self.effects.push((call.span.start, call.span.end));
        self.code.push((call.span.start, call.span.end));
        // `f.call(…)`: reading the standard method runs nothing.
        if let Expression::StaticMemberExpression(member) = &call.callee {
            if !member.optional && matches!(member.property.name.as_str(), "call" | "apply") {
                self.visit_expression(&member.object);
                for argument in &call.arguments {
                    self.visit_argument(argument);
                }
                return;
            }
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_new_expression(&mut self, call: &NewExpression<'a>) {
        self.effects.push((call.span.start, call.span.end));
        self.code.push((call.span.start, call.span.end));
        walk::walk_new_expression(self, call);
    }
    fn visit_member_expression(&mut self, member: &MemberExpression<'a>) {
        self.effects.push((member.span().start, member.span().end));
        walk::walk_member_expression(self, member);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        self.effects.push((assignment.span.start, assignment.span.end));
        if assignment.left.as_member_expression().is_none()
            || assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign
        {
            self.code.push((assignment.span.start, assignment.span.end));
        }
        // `o[k] = v` evaluates `o` and `k` without reading the property.
        if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign {
            if let Some(member) = assignment.left.as_member_expression() {
                match member {
                    MemberExpression::ComputedMemberExpression(computed) => {
                        self.visit_expression(&computed.object);
                        self.visit_expression(&computed.expression);
                    }
                    MemberExpression::StaticMemberExpression(member) => self.visit_expression(&member.object),
                    MemberExpression::PrivateFieldExpression(member) => self.visit_expression(&member.object),
                }
                self.visit_expression(&assignment.right);
                return;
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        self.effects.push((update.span.start, update.span.end));
        self.code.push((update.span.start, update.span.end));
        walk::walk_update_expression(self, update);
    }
    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if !matches!(unary.operator, UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void) {
            self.effects.push((unary.span.start, unary.span.end));
            self.code.push((unary.span.start, unary.span.end));
        }
        walk::walk_unary_expression(self, unary);
    }
    fn visit_binary_expression(&mut self, binary: &BinaryExpression<'a>) {
        if !matches!(binary.operator, BinaryOperator::StrictEquality | BinaryOperator::StrictInequality) {
            self.effects.push((binary.span.start, binary.span.end));
            self.code.push((binary.span.start, binary.span.end));
        }
        walk::walk_binary_expression(self, binary);
    }
    fn visit_template_literal(&mut self, template: &TemplateLiteral<'a>) {
        if !template.expressions.is_empty() {
            self.effects.push((template.span.start, template.span.end));
            self.code.push((template.span.start, template.span.end));
        }
        walk::walk_template_literal(self, template);
    }
}

/// Reads names and properties only: no call, store, update, `delete` or
/// other code (with no getters, a property read runs nothing).
pub(super) fn read_only(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Identifier(_)
        | Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::ThisExpression(_) => true,
        Expression::ParenthesizedExpression(inner) => read_only(&inner.expression),
        Expression::StaticMemberExpression(member) => read_only(&member.object),
        Expression::ComputedMemberExpression(member) => read_only(&member.object) && read_only(&member.expression),
        Expression::UnaryExpression(unary) => {
            unary.operator != UnaryOperator::Delete && read_only(&unary.argument)
        }
        Expression::BinaryExpression(binary) => read_only(&binary.left) && read_only(&binary.right),
        Expression::LogicalExpression(logical) => read_only(&logical.left) && read_only(&logical.right),
        Expression::ConditionalExpression(choice) => {
            read_only(&choice.test) && read_only(&choice.consequent) && read_only(&choice.alternate)
        }
        _ => false,
    }
}
