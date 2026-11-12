//! What is evaluated before a read: harmless prefixes and first-evaluated reads.

use super::*;

/// A prefix may move before another evaluation only if it cannot throw or
/// execute user code and all of its binding reads are stable during it.
pub(super) fn harmless_prefix(e: &Expression<'_>, scoping: &Scoping, readable: &BTreeSet<SymbolId>) -> bool {
    if super::super::super::purity::expression_is_pure(e) {
        return true;
    }
    match e {
        Expression::Identifier(id) => {
            analysis::identifier_symbol(id, scoping).is_some_and(|s| readable.contains(&s))
        }
        Expression::ParenthesizedExpression(p) => harmless_prefix(&p.expression, scoping, readable),
        Expression::UnaryExpression(u)
            if matches!(
                u.operator,
                UnaryOperator::LogicalNot | UnaryOperator::Void | UnaryOperator::Typeof
            ) =>
        {
            harmless_prefix(&u.argument, scoping, readable)
        }
        Expression::BinaryExpression(b)
            if matches!(
                b.operator,
                BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
            ) =>
        {
            harmless_prefix(&b.left, scoping, readable)
                && harmless_prefix(&b.right, scoping, readable)
        }
        Expression::LogicalExpression(l) => {
            harmless_prefix(&l.left, scoping, readable)
                && harmless_prefix(&l.right, scoping, readable)
        }
        Expression::ConditionalExpression(c) => {
            harmless_prefix(&c.test, scoping, readable)
                && harmless_prefix(&c.consequent, scoping, readable)
                && harmless_prefix(&c.alternate, scoping, readable)
        }
        _ => false,
    }
}

/// The first operation in these expression shapes is the identifier read.
/// Moving an effectful initializer to that read preserves evaluation order.
pub(crate) fn first_evaluated_identifier<'a, 'b>(
    expression: &'b Expression<'a>,
) -> Option<&'b IdentifierReference<'a>> {
    match expression {
        Expression::ParenthesizedExpression(parenthesized) => {
            first_evaluated_identifier(&parenthesized.expression)
        }
        Expression::Identifier(identifier) => Some(identifier),
        Expression::StaticMemberExpression(member) => first_evaluated_identifier(&member.object),
        Expression::ComputedMemberExpression(member) => first_evaluated_identifier(&member.object),
        Expression::PrivateFieldExpression(member) => first_evaluated_identifier(&member.object),
        Expression::NewExpression(call) => first_evaluated_identifier(&call.callee),
        Expression::CallExpression(call) => first_evaluated_identifier(&call.callee),
        Expression::UnaryExpression(unary)
            if unary.operator != oxc_ast::ast::UnaryOperator::Delete =>
        {
            first_evaluated_identifier(&unary.argument)
        }
        Expression::BinaryExpression(binary) => first_evaluated_identifier(&binary.left),
        Expression::LogicalExpression(logical) => first_evaluated_identifier(&logical.left),
        Expression::ConditionalExpression(conditional) => {
            first_evaluated_identifier(&conditional.test)
        }
        _ => None,
    }
}

/// A generated function declaration has already been initialized before the
/// adjacent call. If no reference writes its binding, reading that callee
/// before the first argument cannot reorder an observable effect.
pub(super) fn argument_after_immutable_callee<'a, 'b>(
    expression: &'b Expression<'a>,
    initializer: &Expression<'a>,
    scoping: &Scoping,
    target: oxc_semantic::SymbolId,
) -> Option<&'b IdentifierReference<'a>> {
    match expression {
        Expression::ParenthesizedExpression(parenthesized) => {
            argument_after_immutable_callee(&parenthesized.expression, initializer, scoping, target)
        }
        Expression::CallExpression(call) => {
            let Expression::Identifier(callee) = &call.callee else {
                return None;
            };
            let symbol = analysis::identifier_symbol(callee, scoping)?;
            if !scoping.symbol_flags(symbol).is_function()
                || scoping
                    .get_resolved_references(symbol)
                    .any(|reference| reference.is_write())
            {
                return None;
            }
            for (index, argument) in call.arguments.iter().enumerate() {
                if argument.is_spread() {
                    return None;
                }
                if index == 0 {
                    if let Some(first) = first_evaluated_identifier(argument.to_expression()) {
                        if analysis::identifier_symbol(first, scoping) == Some(target) {
                            return Some(first);
                        }
                    }
                }
                if let Expression::Identifier(id) = argument.to_expression() {
                    if analysis::identifier_symbol(id, scoping) == Some(target) {
                        // Later arguments are safe only when moving a plain
                        // lexical read across previously initialized, plain
                        // lexical reads. Neither side can invoke a getter or
                        // modify a binding during the reorder.
                        let Expression::Identifier(source) = initializer else {
                            return None;
                        };
                        analysis::identifier_symbol(source, scoping)?;
                        return Some(id);
                    }
                    let symbol = analysis::identifier_symbol(id, scoping)?;
                    // Parameters and `var` bindings are initialized on
                    // function entry. A preceding `let` read may still be
                    // in the TDZ after a switch-case jump, even when its
                    // declaration appears earlier in source order.
                    if !scoping
                        .symbol_flags(symbol)
                        .is_function_scoped_declaration()
                    {
                        return None;
                    }
                } else if !matches!(
                    argument.to_expression(),
                    Expression::BooleanLiteral(_)
                        | Expression::NullLiteral(_)
                        | Expression::NumericLiteral(_)
                        | Expression::BigIntLiteral(_)
                        | Expression::StringLiteral(_)
                ) {
                    return None;
                }
            }
            None
        }
        _ => None,
    }
}

pub(super) fn successful_numeric_input(
    e: &Expression<'_>,
    numeric_inputs: &BTreeSet<SymbolId>,
    scoping: &Scoping,
    ordinary_number: bool,
) -> bool {
    if ordinary_number && super::super::super::numeric_coercions::number_argument(e, scoping).is_some() {
        return true;
    }
    let number = |e| successful_numeric_input(e, numeric_inputs, scoping, ordinary_number);
    match e {
        Expression::BooleanLiteral(_) | Expression::NullLiteral(_) => true,
        Expression::ParenthesizedExpression(p) => number(&p.expression),
        Expression::UnaryExpression(u)
            if matches!(
                u.operator,
                UnaryOperator::UnaryPlus
                    | UnaryOperator::LogicalNot
                    | UnaryOperator::Void
                    | UnaryOperator::Delete
            ) =>
        {
            true
        }
        Expression::BinaryExpression(b)
            if b.operator.is_compare()
                || b.operator.is_equality()
                || b.operator.is_in()
                || b.operator.is_instance_of() =>
        {
            true
        }
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::ShiftRightZeroFill => true,
        Expression::BinaryExpression(b)
            if matches!(
                b.operator,
                BinaryOperator::Subtraction
                    | BinaryOperator::Multiplication
                    | BinaryOperator::Division
                    | BinaryOperator::Remainder
                    | BinaryOperator::Exponential
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseXOR
            ) =>
        {
            number(&b.left) || number(&b.right)
        }
        Expression::ConditionalExpression(c) => number(&c.consequent) && number(&c.alternate),
        Expression::LogicalExpression(l) => number(&l.left) && number(&l.right),
        Expression::SequenceExpression(s) => s.expressions.last().is_some_and(number),
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
            number(&b.left) && number(&b.right)
        }
        _ => numeric_expression(e, numeric_inputs, scoping),
    }
}

// Number/String/Boolean/null/undefined on successful evaluation. This is a
// result-type proof only: getters, calls and coercions in the initializer
// are still effects and remain ordered by the consumer's separate proof.
pub(super) fn successful_primitive_input(
    e: &Expression<'_>,
    inputs: &BTreeSet<SymbolId>,
    scoping: &Scoping,
    ordinary_number: bool,
) -> bool {
    let primitive = |e| successful_primitive_input(e, inputs, scoping, ordinary_number);
    match e {
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => true,
        Expression::ParenthesizedExpression(p) => primitive(&p.expression),
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::Typeof => true,
        Expression::BinaryExpression(b) if !b.operator.is_in() && !b.operator.is_instance_of() => {
            b.operator.is_compare()
                || b.operator.is_equality()
                || b.operator == BinaryOperator::ShiftRightZeroFill
                || primitive(&b.left)
                || primitive(&b.right)
        }
        Expression::ConditionalExpression(c) => primitive(&c.consequent) && primitive(&c.alternate),
        Expression::LogicalExpression(l) => primitive(&l.left) && primitive(&l.right),
        Expression::SequenceExpression(s) => s.expressions.last().is_some_and(primitive),
        _ => successful_numeric_input(e, inputs, scoping, ordinary_number),
    }
}

pub(super) fn numeric_expression(
    e: &Expression<'_>,
    numeric_inputs: &BTreeSet<SymbolId>,
    scoping: &Scoping,
) -> bool {
    match e {
        Expression::NumericLiteral(_) => true,
        Expression::Identifier(id) => analysis::identifier_symbol(id, scoping)
            .is_some_and(|symbol| numeric_inputs.contains(&symbol)),
        Expression::ParenthesizedExpression(p) => {
            numeric_expression(&p.expression, numeric_inputs, scoping)
        }
        Expression::UnaryExpression(u)
            if matches!(
                u.operator,
                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot
            ) =>
        {
            numeric_expression(&u.argument, numeric_inputs, scoping)
        }
        Expression::BinaryExpression(b)
            if matches!(
                b.operator,
                BinaryOperator::Addition
                    | BinaryOperator::Subtraction
                    | BinaryOperator::Multiplication
                    | BinaryOperator::Division
                    | BinaryOperator::Remainder
                    | BinaryOperator::Exponential
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::ShiftRightZeroFill
                    | BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseXOR
            ) =>
        {
            numeric_expression(&b.left, numeric_inputs, scoping)
                && numeric_expression(&b.right, numeric_inputs, scoping)
        }
        _ => false,
    }
}

// Evaluate only already-stored primitive inputs. Unlike the successful-result
// proof above, every operand must be inert; an object operand could otherwise
// run a coercion hook even when the final result is always a string/number.
pub(super) fn inert_primitive_expression(
    e: &Expression<'_>,
    inputs: &BTreeSet<SymbolId>,
    scoping: &Scoping,
    cells: Option<&super::super::super::cell_facts::Facts>,
) -> bool {
    let inert = |e| inert_primitive_expression(e, inputs, scoping, cells);
    match e {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::Identifier(id) => analysis::identifier_symbol(id, scoping).is_some_and(|s| {
            inputs.contains(&s)
                || cells.is_some_and(|f| f.values.get(&id.span).is_some_and(|v| v.scalar))
        }),
        Expression::ParenthesizedExpression(p) => inert(&p.expression),
        Expression::UnaryExpression(u) => u.operator != UnaryOperator::Delete && inert(&u.argument),
        Expression::BinaryExpression(b) => {
            !b.operator.is_in() && !b.operator.is_instance_of() && inert(&b.left) && inert(&b.right)
        }
        Expression::ConditionalExpression(c) => {
            inert(&c.test) && inert(&c.consequent) && inert(&c.alternate)
        }
        Expression::LogicalExpression(l) => inert(&l.left) && inert(&l.right),
        _ => false,
    }
}

// Used when an incoming lexical slot might still be in its TDZ. Its
// original read may move only to the first unconditional, effect-free read.
pub(crate) fn begins_with_read(
    expression: &Expression<'_>,
    target: SymbolId,
    scoping: &Scoping,
    locals: &LocalReads,
) -> bool {
    let mut initialized = BTreeSet::new();
    locals.plain_reads(expression.span(), &mut initialized);
    matches!(
        prefix_read(
            expression,
            target,
            scoping,
            &initialized,
            false,
            &BTreeSet::new(),
            ReadPolicy {
                commuting_initialized_read: false,
                this_initialized: locals.classes.enclosing(expression.span()).is_none(),
                primitive_inputs: &locals.primitive_inputs,
                cells: None,
                ordinary_number: false,
            }
        ),
        Ok(Some(_))
    )
}

/// Locate an unconditionally evaluated read before any observable operation.
/// Immutable, initialized local reads may precede it: the initializer cannot
/// change those bindings, and reading them cannot throw or call user code.
/// `Err` means evaluation hit a barrier before reaching the desired read.
#[derive(Clone, Copy)]
pub(super) struct ReadPolicy<'s> {
    pub(super) commuting_initialized_read: bool,
    pub(super) this_initialized: bool,
    pub(super) primitive_inputs: &'s BTreeSet<SymbolId>,
    pub(super) cells: Option<&'s super::super::super::cell_facts::Facts>,
    pub(super) ordinary_number: bool,
}
pub(super) fn prefix_read<'a, 'b>(
    expression: &'b Expression<'a>,
    target: oxc_semantic::SymbolId,
    scoping: &Scoping,
    initialized: &BTreeSet<oxc_semantic::SymbolId>,
    native_calls: bool,
    lexical_slots: &BTreeSet<String>,
    policy: ReadPolicy<'_>,
) -> Result<Option<&'b IdentifierReference<'a>>, ()> {
    PrefixScan {
        target,
        scoping,
        initialized,
        native_calls,
        lexical_slots,
        policy,
    }
    .read(expression)
}

/// The fixed inputs of one `prefix_read` walk.
struct PrefixScan<'s> {
    target: SymbolId,
    scoping: &'s Scoping,
    initialized: &'s BTreeSet<SymbolId>,
    native_calls: bool,
    lexical_slots: &'s BTreeSet<String>,
    policy: ReadPolicy<'s>,
}

impl PrefixScan<'_> {
    fn read<'a, 'b>(
        &self,
        expression: &'b Expression<'a>,
    ) -> Result<Option<&'b IdentifierReference<'a>>, ()> {
        let scoping = self.scoping;
        let ReadPolicy {
            this_initialized,
            primitive_inputs,
            cells,
            ..
        } = self.policy;
        match expression {
            Expression::ChainExpression(chain) => self.chain(chain),
            Expression::Identifier(id) => self.identifier(id),
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_) => Ok(None),
            Expression::ThisExpression(_) if this_initialized => Ok(None),
            Expression::ParenthesizedExpression(p) => self.read(&p.expression),
            Expression::UnaryExpression(u) if u.operator != UnaryOperator::Delete => {
                let found = self.read(&u.argument)?;
                if found.is_some()
                    || matches!(
                        u.operator,
                        UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
                    )
                    || inert_primitive_expression(&u.argument, primitive_inputs, scoping, cells)
                {
                    Ok(found)
                } else {
                    Err(())
                }
            }
            Expression::BinaryExpression(b) => self.binary(b),
            Expression::LogicalExpression(l) => self.read(&l.left)?.map(Some).ok_or(()),
            Expression::ConditionalExpression(c) => self.read(&c.test)?.map(Some).ok_or(()),
            Expression::StaticMemberExpression(m) => {
                let found = self.read(&m.object)?;
                if found.is_some()
                    || (self.native_calls && matches!(m.property.name.as_str(), "call" | "apply"))
                {
                    Ok(found)
                } else {
                    Err(())
                }
            }
            Expression::ComputedMemberExpression(m) => self.computed_member(m),
            Expression::TaggedTemplateExpression(t) => {
                if let Some(id) = self.read(&t.tag)? {
                    return Ok(Some(id));
                }
                for expression in &t.quasi.expressions {
                    if let Some(id) = self.read(expression)? {
                        return Ok(Some(id));
                    }
                }
                Err(())
            }
            Expression::NewExpression(c) => self.call(&c.callee, &c.arguments),
            Expression::CallExpression(c) if !c.optional => self.call(&c.callee, &c.arguments),
            Expression::SequenceExpression(s) => {
                for expression in &s.expressions {
                    if let Some(id) = self.read(expression)? {
                        return Ok(Some(id));
                    }
                }
                Ok(None)
            }
            Expression::ArrayExpression(a) => {
                for element in &a.elements {
                    if matches!(element, ArrayExpressionElement::Elision(_)) {
                        continue;
                    }
                    if matches!(element, ArrayExpressionElement::SpreadElement(_)) {
                        return Err(());
                    }
                    if let Some(id) = self.read(element.to_expression())? {
                        return Ok(Some(id));
                    }
                }
                Ok(None)
            }
            Expression::ObjectExpression(o) => {
                for property in &o.properties {
                    let ObjectPropertyKind::ObjectProperty(p) = property else {
                        return Err(());
                    };
                    if p.computed || p.shorthand {
                        return Err(());
                    }
                    if let Some(id) = self.read(&p.value)? {
                        return Ok(Some(id));
                    }
                }
                Ok(None)
            }
            Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
                self.assignment(a)
            }
            _ => Err(()),
        }
    }

    /// The base of an optional chain.
    fn chain<'a, 'b>(
        &self,
        chain: &'b ChainExpression<'a>,
    ) -> Result<Option<&'b IdentifierReference<'a>>, ()> {
        // Only the base of an optional chain is unconditional. Keys and
        // arguments after ?. may be skipped, so never sink a producer
        // into those conditional positions.
        let base = match &chain.expression {
            ChainElement::ComputedMemberExpression(m) => &m.object,
            ChainElement::StaticMemberExpression(m) => &m.object,
            ChainElement::CallExpression(c) => &c.callee,
            _ => return Err(()),
        };
        self.read(base)?.map(Some).ok_or(())
    }

    /// The target itself, or a read that cannot throw or call user code.
    fn identifier<'a, 'b>(
        &self,
        id: &'b IdentifierReference<'a>,
    ) -> Result<Option<&'b IdentifierReference<'a>>, ()> {
        let scoping = self.scoping;
        let native_calls = self.native_calls;
        // The emitted Number conversion uses the same guarded native
        // constructor contract as numeric_coercions. Its lookup cannot
        // invoke a getter; the conversion itself remains an effect
        // barrier after its arguments have been evaluated.
        if native_calls
            && self.policy.ordinary_number
            && id.name == "Number"
            && analysis::identifier_symbol(id, scoping).is_none()
        {
            return Ok(None);
        }
        if self.policy.commuting_initialized_read
            && native_calls
            && self.lexical_slots.contains(id.name.as_str())
            && analysis::identifier_symbol(id, scoping).is_none()
        {
            return Ok(None);
        }
        let symbol = analysis::identifier_symbol(id, scoping).ok_or(())?;
        if symbol == self.target {
            return Ok(Some(id));
        }
        if self.initialized.contains(&symbol)
            || (scoping.symbol_flags(symbol).is_function()
                && scoping.symbol_redeclarations(symbol).is_empty()
                && scoping
                    .get_resolved_references(symbol)
                    .all(|r| !r.is_write()))
        {
            Ok(None)
        } else {
            Err(())
        }
    }

    /// Both operands, then an operator that cannot coerce through user code.
    fn binary<'a, 'b>(
        &self,
        b: &'b BinaryExpression<'a>,
    ) -> Result<Option<&'b IdentifierReference<'a>>, ()> {
        let ReadPolicy {
            primitive_inputs,
            cells,
            ..
        } = self.policy;
        if let Some(id) = self.read(&b.left)? {
            return Ok(Some(id));
        }
        if let Some(id) = self.read(&b.right)? {
            return Ok(Some(id));
        }
        if matches!(
            b.operator,
            BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
        ) || (!b.operator.is_in()
            && !b.operator.is_instance_of()
            && inert_primitive_expression(&b.left, primitive_inputs, self.scoping, cells)
            && inert_primitive_expression(&b.right, primitive_inputs, self.scoping, cells))
        {
            Ok(None)
        } else {
            Err(())
        }
    }

    /// The object and key of a computed member read; the read itself is a
    /// barrier except for proven `arguments` inputs and `call`/`apply`.
    fn computed_member<'a, 'b>(
        &self,
        m: &'b ComputedMemberExpression<'a>,
    ) -> Result<Option<&'b IdentifierReference<'a>>, ()> {
        let native_calls = self.native_calls;
        // A proven readonly, unescaped parameterless arguments object
        // has inert numeric input reads under the ordinary-prototype
        // contract. Arbitrary members/getters remain barriers.
        if native_calls
            && self.policy.cells.is_some_and(|f| {
                f.values.get(&m.span).is_some_and(|v| {
                    matches!(v.token, Some(super::super::super::cell_facts::Token::Argument(_, _)))
                })
            })
        {
            return Ok(None);
        }
        if let Some(id) = self.read(&m.object)? {
            return Ok(Some(id));
        }
        if m.optional {
            return Err(());
        }
        if native_calls
            && matches!(&m.expression, Expression::StringLiteral(s) if s.value=="call" || s.value=="apply")
        {
            return Ok(None);
        }
        self.read(&m.expression)?.map(Some).ok_or(())
    }

    /// The callee and spread-free arguments of a call or `new`; the call
    /// itself is a barrier.
    fn call<'a, 'b>(
        &self,
        callee: &'b Expression<'a>,
        arguments: &'b [Argument<'a>],
    ) -> Result<Option<&'b IdentifierReference<'a>>, ()> {
        if let Some(id) = self.read(callee)? {
            return Ok(Some(id));
        }
        for argument in arguments {
            if argument.is_spread() {
                return Err(());
            }
            if let Some(id) = self.read(argument.to_expression())? {
                return Ok(Some(id));
            }
        }
        Err(())
    }

    /// The target's reference parts, then the right-hand side.
    fn assignment<'a, 'b>(
        &self,
        a: &'b AssignmentExpression<'a>,
    ) -> Result<Option<&'b IdentifierReference<'a>>, ()> {
        match &a.left {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                // A lexical assignment creates a reference, without reading
                // its value. Const/TDZ checks happen at PutValue, after the
                // RHS, so even a throwing store preserves RHS evaluation.
                // Incoming VM cells are known lexical bindings from tracer
                // facts despite being unresolved in this standalone AST.
                if analysis::identifier_symbol(id, self.scoping).is_none()
                    && !self.lexical_slots.contains(id.name.as_str())
                {
                    return Err(());
                }
            }
            AssignmentTarget::StaticMemberExpression(m) => {
                if let Some(id) = self.read(&m.object)? {
                    return Ok(Some(id));
                }
            }
            AssignmentTarget::ComputedMemberExpression(m) => {
                if let Some(id) = self.read(&m.object)? {
                    return Ok(Some(id));
                }
                if let Some(id) = self.read(&m.expression)? {
                    return Ok(Some(id));
                }
            }
            _ => return Err(()),
        }
        self.read(&a.right)?.map(Some).ok_or(())
    }
}
