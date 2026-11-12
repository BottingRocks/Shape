//! Resugaring: method calls, nullish tests and repeated tests written as source would.

use super::*;

/// Fold the VM's two-read spellings back into one-read source forms:
/// `o[k].call(o, …)` -> `o[k](…)`, `c ? c : b` -> `c || b`,
/// `c ? b : c` -> `c && b`, `c == null ? b : c` -> `c ?? b`, and
/// `c == null ? void 0 : c.p` -> `c?.p`. `c` and `o` are plain names, so
/// reading one once instead of twice sees the same value. `??` treats the
/// legacy `document.all` as present where `== null` does not.
pub(crate) fn resugar_expressions(source: &str) -> Result<TidyExpressionsResult, String> {
    let allocator = Allocator::default();
    let mut program = analysis::parse_program(&allocator, source)?;
    let mut resugar = Resugar {
        builder: AstBuilder::new(&allocator),
        source,
        folded: 0,
    };
    resugar.visit_program(&mut program);
    if resugar.folded == 0 {
        return Ok(TidyExpressionsResult {
            code: source.into(),
            unwrapped: 0,
        });
    }
    Ok(TidyExpressionsResult {
        code: print(&program),
        unwrapped: resugar.folded,
    })
}

pub(super) fn print(program: &Program<'_>) -> String {
    Codegen::new()
        .with_options(CodegenOptions {
            indent_char: IndentChar::Tab,
            indent_width: 1,
            ..CodegenOptions::default()
        })
        .build(program)
        .code
}

pub(super) struct Resugar<'a> {
    builder: AstBuilder<'a>,
    source: &'a str,
    folded: usize,
}

/// A test that evaluating twice or once is the same: names, literals, `!`,
/// `typeof`, `void`, strict (in)equality, and `== null`/`!= null`.
pub(super) fn repeatable(expression: &Expression<'_>) -> bool {
    use oxc_syntax::operator::BinaryOperator;
    match expression {
        // With no getters a property path reads the same twice.
        Expression::StaticMemberExpression(member) => !member.optional && repeatable(&member.object),
        Expression::ComputedMemberExpression(member) => {
            !member.optional
                && matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_))
                && repeatable(&member.object)
        }
        Expression::Identifier(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            matches!(unary.operator, UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void)
                && repeatable(&unary.argument)
        }
        Expression::BinaryExpression(binary) => match binary.operator {
            BinaryOperator::StrictEquality | BinaryOperator::StrictInequality => {
                repeatable(&binary.left) && repeatable(&binary.right)
            }
            BinaryOperator::Equality | BinaryOperator::Inequality => {
                (matches!(binary.left, Expression::NullLiteral(_)) || matches!(binary.right, Expression::NullLiteral(_)))
                    && repeatable(&binary.left)
                    && repeatable(&binary.right)
            }
            // Over primitives a comparison runs no code: `x > 0` twice is
            // one value (`x > 0 ? x < 1 : x > 0` is `x > 0 && x < 1`).
            BinaryOperator::LessThan | BinaryOperator::LessEqualThan | BinaryOperator::GreaterThan | BinaryOperator::GreaterEqualThan => {
                repeatable(&binary.left) && repeatable(&binary.right)
            }
            _ => false,
        },
        _ => false,
    }
}

/// `a op b` over two integer literals, when the result is a small
/// non-negative integer that prints the same.
/// `null == null` and friends: comparisons of two literal nullish values.
pub(super) fn folded_nullish(expression: &Expression<'_>) -> Option<bool> {
    let Expression::BinaryExpression(binary) = expression else {
        return None;
    };
    let nullish = |e: &Expression<'_>| {
        matches!(e, Expression::NullLiteral(_))
            || matches!(e, Expression::UnaryExpression(u) if u.operator == UnaryOperator::Void && matches!(u.argument, Expression::NumericLiteral(_)))
    };
    if !nullish(&binary.left) || !nullish(&binary.right) {
        return None;
    }
    let same = matches!(binary.left, Expression::NullLiteral(_)) == matches!(binary.right, Expression::NullLiteral(_));
    match binary.operator {
        BinaryOperator::Equality => Some(true),
        BinaryOperator::Inequality => Some(false),
        BinaryOperator::StrictEquality => Some(same),
        BinaryOperator::StrictInequality => Some(!same),
        _ => None,
    }
}

pub(super) fn folded_number(expression: &Expression<'_>) -> Option<f64> {
    use oxc_syntax::operator::BinaryOperator;
    let Expression::BinaryExpression(binary) = expression else {
        return None;
    };
    let (Expression::NumericLiteral(left), Expression::NumericLiteral(right)) = (&binary.left, &binary.right) else {
        return None;
    };
    let value = match binary.operator {
        BinaryOperator::Addition => left.value + right.value,
        BinaryOperator::Subtraction => left.value - right.value,
        BinaryOperator::Multiplication => left.value * right.value,
        _ => return None,
    };
    (left.value.fract() == 0.0 && right.value.fract() == 0.0 && (0.0..=9007199254740991.0).contains(&value))
        .then_some(value)
}

/// An expression statement whose value nobody reads and that does nothing:
/// inert reads, and arithmetic over them (the VM's decoys; their operands
/// are primitives).
pub(super) fn discardable(expression: &Expression<'_>) -> bool {
    use oxc_syntax::operator::BinaryOperator;
    match expression {
        Expression::ParenthesizedExpression(inner) => discardable(&inner.expression),
        Expression::BinaryExpression(binary) => {
            !matches!(binary.operator, BinaryOperator::In | BinaryOperator::Instanceof)
                && discardable(&binary.left)
                && discardable(&binary.right)
        }
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                UnaryOperator::UnaryNegation
                    | UnaryOperator::UnaryPlus
                    | UnaryOperator::BitwiseNot
                    | UnaryOperator::Typeof
                    | UnaryOperator::LogicalNot
                    | UnaryOperator::Void
            ) && discardable(&unary.argument)
                || droppable(expression)
        }
        // A property read whose value nobody uses does nothing (no getters).
        Expression::StaticMemberExpression(member) => discardable(&member.object),
        Expression::ComputedMemberExpression(member)
            if matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_)) =>
        {
            discardable(&member.object)
        }
        // `Math.random()` and the like change nothing anyone can see, nor do
        // a number's or string's own methods on such a result.
        Expression::CallExpression(call) if !call.optional && call.arguments.iter().all(|argument| {
            argument.as_expression().is_some_and(discardable)
        }) => match call.callee.get_member_expr() {
            Some(member) if !member.optional() => match member.object() {
                Expression::Identifier(object) => object.name == "Math",
                object => {
                    matches!(member.static_property_name(), Some("toString" | "slice" | "substring" | "toFixed"))
                        && matches!(object, Expression::CallExpression(_) | Expression::StringLiteral(_))
                        && discardable(object)
                }
            },
            _ => false,
        },
        // Reading a standard global or the page's own `document`/`window`
        // runs no code and never throws in the page.
        Expression::Identifier(identifier)
            if super::super::super::primitives::is_stable_global(&identifier.name)
                || matches!(identifier.name.as_str(), "document" | "window" | "self" | "navigator" | "location") =>
        {
            true
        }
        _ => droppable(expression),
    }
}

/// Always a boolean: a comparison, `!`, or a boolean literal, or `&&`, `||`
/// and `?:` over those.
pub(super) fn boolean(expression: &Expression<'_>) -> bool {
    use oxc_syntax::operator::{BinaryOperator, LogicalOperator};
    match expression.without_parentheses() {
        Expression::BooleanLiteral(_) => true,
        Expression::UnaryExpression(unary) => unary.operator == UnaryOperator::LogicalNot,
        Expression::BinaryExpression(binary) => matches!(
            binary.operator,
            BinaryOperator::LessThan
                | BinaryOperator::LessEqualThan
                | BinaryOperator::GreaterThan
                | BinaryOperator::GreaterEqualThan
                | BinaryOperator::Equality
                | BinaryOperator::Inequality
                | BinaryOperator::StrictEquality
                | BinaryOperator::StrictInequality
                | BinaryOperator::In
                | BinaryOperator::Instanceof
        ),
        Expression::LogicalExpression(logical) => {
            logical.operator != LogicalOperator::Coalesce && boolean(&logical.left) && boolean(&logical.right)
        }
        Expression::ConditionalExpression(conditional) => {
            boolean(&conditional.consequent) && boolean(&conditional.alternate)
        }
        _ => false,
    }
}

/// Evaluating it runs no code and cannot throw: generated locals (declared
/// before the code reading them), `arguments[n]`, literals, and `!`,
/// `typeof`, strict and null comparisons over those.
pub(super) fn droppable(expression: &Expression<'_>) -> bool {
    use oxc_syntax::operator::BinaryOperator;
    match expression {
        Expression::Identifier(identifier) => identifier.name.starts_with('$') || identifier.name == "arguments",
        Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::ComputedMemberExpression(member) => {
            !member.optional
                && matches!(&member.object, Expression::Identifier(object) if object.name == "arguments")
                && matches!(member.expression, Expression::NumericLiteral(_))
        }
        Expression::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::Typeof => matches!(unary.argument, Expression::Identifier(_)) || droppable(&unary.argument),
            UnaryOperator::LogicalNot | UnaryOperator::Void => droppable(&unary.argument),
            _ => false,
        },
        Expression::BinaryExpression(binary) => {
            let operands = droppable(&binary.left) && droppable(&binary.right);
            match binary.operator {
                BinaryOperator::StrictEquality | BinaryOperator::StrictInequality => operands,
                BinaryOperator::Equality | BinaryOperator::Inequality => {
                    operands
                        && (matches!(binary.left, Expression::NullLiteral(_))
                            || matches!(binary.right, Expression::NullLiteral(_)))
                }
                _ => false,
            }
        }
        _ => false,
    }
}

pub(super) fn same_name(a: &Expression<'_>, b: &Expression<'_>) -> bool {
    matches!((a, b), (Expression::Identifier(a), Expression::Identifier(b)) if a.name == b.name)
}

/// The same name, `this`, or member path of one with literal keys:
/// `arguments[1]`, `this["a"].b`. Reading it twice reads the same value.
pub(super) fn same_path(a: &Expression<'_>, b: &Expression<'_>) -> bool {
    match (a.without_parentheses(), b.without_parentheses()) {
        (Expression::Identifier(a), Expression::Identifier(b)) => a.name == b.name,
        (Expression::ThisExpression(_), Expression::ThisExpression(_)) => true,
        (Expression::StaticMemberExpression(a), Expression::StaticMemberExpression(b)) => {
            !a.optional && !b.optional && a.property.name == b.property.name && same_path(&a.object, &b.object)
        }
        (Expression::ComputedMemberExpression(a), Expression::ComputedMemberExpression(b)) => {
            !a.optional
                && !b.optional
                && match (&a.expression, &b.expression) {
                    (Expression::StringLiteral(x), Expression::StringLiteral(y)) => x.value == y.value,
                    (Expression::NumericLiteral(x), Expression::NumericLiteral(y)) => x.value == y.value,
                    _ => false,
                }
                && same_path(&a.object, &b.object)
        }
        _ => false,
    }
}

/// `c == null`, `null == c` (or `!=` with `negated`): the tested name.
pub(super) fn null_test<'e, 'a>(test: &'e Expression<'a>, negated: bool) -> Option<&'e Expression<'a>> {
    let Expression::BinaryExpression(binary) = test else {
        return None;
    };
    let operator = if negated {
        oxc_syntax::operator::BinaryOperator::Inequality
    } else {
        oxc_syntax::operator::BinaryOperator::Equality
    };
    if binary.operator != operator {
        return None;
    }
    match (&binary.left, &binary.right) {
        (Expression::NullLiteral(_), name) | (name, Expression::NullLiteral(_))
            if matches!(name, Expression::Identifier(_)) || (name.is_member_expression() && repeatable(name)) =>
        {
            Some(name)
        }
        _ => None,
    }
}

pub(super) fn is_undefined(expression: &Expression<'_>) -> bool {
    matches!(expression, Expression::UnaryExpression(unary)
        if unary.operator == UnaryOperator::Void && matches!(unary.argument, Expression::NumericLiteral(_)))
}

impl<'a> Resugar<'a> {
    fn take(&self, expression: &mut Expression<'a>) -> Expression<'a> {
        std::mem::replace(expression, self.builder.expression_null_literal(SPAN))
    }

    fn logical(&self, left: Expression<'a>, operator: oxc_syntax::operator::LogicalOperator, right: Expression<'a>) -> Expression<'a> {
        self.builder.expression_logical(SPAN, left, operator, right)
    }

    /// `c?.p` / `c?.[k]` for a member read of `c`.
    fn optional_member(&self, name: &Expression<'a>, member: &mut Expression<'a>) -> Option<Expression<'a>> {
        // A longer chain off a repeated path: `c.p.q` with `c` a path marks
        // the link right after `c` optional.
        if !matches!(name, Expression::Identifier(_)) {
            fn mark<'b>(expression: &mut Expression<'b>, name: &Expression<'b>) -> bool {
                match expression {
                    Expression::StaticMemberExpression(m) => {
                        if m.object.content_eq(name) {
                            m.optional = true;
                            true
                        } else {
                            mark(&mut m.object, name)
                        }
                    }
                    Expression::ComputedMemberExpression(m) => {
                        if m.object.content_eq(name) {
                            m.optional = true;
                            true
                        } else {
                            mark(&mut m.object, name)
                        }
                    }
                    _ => false,
                }
            }
            if !mark(member, name) {
                return None;
            }
            let taken = self.take(member);
            let chain = match taken {
                Expression::StaticMemberExpression(m) => ChainElement::StaticMemberExpression(m),
                Expression::ComputedMemberExpression(m) => ChainElement::ComputedMemberExpression(m),
                _ => return None,
            };
            return Some(self.builder.expression_chain(SPAN, chain));
        }
        let b = self.builder;
        let object = match member {
            Expression::StaticMemberExpression(m) if same_name(&m.object, name) => {
                let property = m.property.clone_in(b.allocator);
                let object = self.take(&mut m.object);
                Expression::from(b.member_expression_static(SPAN, object, property, true))
            }
            Expression::ComputedMemberExpression(m) if same_name(&m.object, name) => {
                let key = self.take(&mut m.expression);
                let object = self.take(&mut m.object);
                Expression::from(b.member_expression_computed(SPAN, object, key, true))
            }
            _ => return None,
        };
        let chain = match object {
            Expression::StaticMemberExpression(m) => ChainElement::StaticMemberExpression(m),
            Expression::ComputedMemberExpression(m) => ChainElement::ComputedMemberExpression(m),
            _ => return None,
        };
        Some(b.expression_chain(SPAN, chain))
    }

    /// `!t` for a boolean `t`, spelled the short way: `!a` is `a` when `a`
    /// is boolean (`!!a` otherwise), `!(a || b)` is `!a && !b`, and
    /// `!(a === b)` is `a !== b`.
    fn negate(&self, test: Expression<'a>) -> Expression<'a> {
        use oxc_syntax::operator::{BinaryOperator, LogicalOperator};
        match test {
            Expression::ParenthesizedExpression(inner) => self.negate(inner.unbox().expression),
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
                let argument = unary.unbox().argument;
                if boolean(&argument) {
                    argument
                } else {
                    let once = self.builder.expression_unary(SPAN, UnaryOperator::LogicalNot, argument);
                    self.builder.expression_unary(SPAN, UnaryOperator::LogicalNot, once)
                }
            }
            Expression::LogicalExpression(logical)
                if logical.operator == LogicalOperator::Or && boolean(&logical.left) && boolean(&logical.right) =>
            {
                let logical = logical.unbox();
                let left = self.negate(logical.left);
                let right = self.negate(logical.right);
                self.logical(left, LogicalOperator::And, right)
            }
            Expression::BinaryExpression(mut binary)
                if matches!(binary.operator, BinaryOperator::StrictEquality | BinaryOperator::StrictInequality) =>
            {
                binary.operator = if binary.operator == BinaryOperator::StrictEquality {
                    BinaryOperator::StrictInequality
                } else {
                    BinaryOperator::StrictEquality
                };
                Expression::BinaryExpression(binary)
            }
            test => self.builder.expression_unary(SPAN, UnaryOperator::LogicalNot, test),
        }
    }

    fn fold_conditional(&mut self, expression: &mut Expression<'a>) {
        use oxc_syntax::operator::LogicalOperator;
        let Expression::ConditionalExpression(conditional) = expression else {
            return;
        };
        let same_text = |a: &Expression<'_>, b: &Expression<'_>| {
            let (a, b) = (a.span(), b.span());
            // Nodes built by this pass have an empty span and no text.
            a.end > a.start
                && b.end > b.start
                && self.source[a.start as usize..a.end as usize] == self.source[b.start as usize..b.end as usize]
        };
        // A test that cannot throw or run code has no effect to keep.
        let local_test = droppable(&conditional.test);
        let folded = if local_test && conditional.consequent.content_eq(&conditional.alternate) {
            // c ? a : a
            Some(self.take(&mut conditional.consequent))
        } else if !matches!(conditional.test, Expression::Identifier(_))
            && repeatable(&conditional.test)
            && same_text(&conditional.test, &conditional.consequent)
        {
            // t ? t : b with a repeatable test t
            let test = self.take(&mut conditional.test);
            let alternate = self.take(&mut conditional.alternate);
            Some(self.logical(test, LogicalOperator::Or, alternate))
        } else if !matches!(conditional.test, Expression::Identifier(_))
            && repeatable(&conditional.test)
            && same_text(&conditional.test, &conditional.alternate)
        {
            // t ? b : t
            let test = self.take(&mut conditional.test);
            let consequent = self.take(&mut conditional.consequent);
            Some(self.logical(test, LogicalOperator::And, consequent))
        } else if matches!(conditional.test, Expression::Identifier(_))
            && same_name(&conditional.test, &conditional.consequent)
        {
            // c ? c : b
            let test = self.take(&mut conditional.test);
            let alternate = self.take(&mut conditional.alternate);
            Some(self.logical(test, LogicalOperator::Or, alternate))
        } else if matches!(conditional.test, Expression::Identifier(_))
            && same_name(&conditional.test, &conditional.alternate)
        {
            // c ? b : c
            let test = self.take(&mut conditional.test);
            let consequent = self.take(&mut conditional.consequent);
            Some(self.logical(test, LogicalOperator::And, consequent))
        } else if repeatable(&conditional.test)
            && matches!(&conditional.alternate, Expression::UnaryExpression(not)
                if not.operator == UnaryOperator::LogicalNot && not.argument.content_eq(&conditional.test))
        {
            // t ? a : !t is !t || a
            let alternate = self.take(&mut conditional.alternate);
            let consequent = self.take(&mut conditional.consequent);
            Some(self.logical(alternate, LogicalOperator::Or, consequent))
        } else if boolean(&conditional.test)
            && matches!(conditional.alternate, Expression::BooleanLiteral(ref value) if !value.value)
        {
            // t ? b : false, with a boolean t
            let test = self.take(&mut conditional.test);
            let consequent = self.take(&mut conditional.consequent);
            Some(self.logical(test, LogicalOperator::And, consequent))
        } else if boolean(&conditional.test)
            && matches!(conditional.consequent, Expression::BooleanLiteral(ref value) if !value.value)
        {
            // t ? false : b is !t && b, with a boolean t
            let test = self.take(&mut conditional.test);
            let test = self.negate(test);
            let alternate = self.take(&mut conditional.alternate);
            Some(self.logical(test, LogicalOperator::And, alternate))
        } else if boolean(&conditional.test)
            && matches!(conditional.consequent, Expression::BooleanLiteral(ref value) if value.value)
        {
            // t ? true : b, with a boolean t
            let test = self.take(&mut conditional.test);
            let alternate = self.take(&mut conditional.alternate);
            Some(self.logical(test, LogicalOperator::Or, alternate))
        } else if let Some(name) = null_test(&conditional.test, false) {
            let name_copy = name.clone_in(self.builder.allocator);
            if same_name(&name_copy, &conditional.alternate) {
                // c == null ? b : c
                let consequent = self.take(&mut conditional.consequent);
                Some(self.logical(name_copy, LogicalOperator::Coalesce, consequent))
            } else if is_undefined(&conditional.consequent) {
                // c == null ? void 0 : c.p
                self.optional_member(&name_copy, &mut conditional.alternate)
            } else {
                None
            }
        } else if let Some(name) = null_test(&conditional.test, true) {
            let name_copy = name.clone_in(self.builder.allocator);
            if same_name(&name_copy, &conditional.consequent) {
                // c != null ? c : b
                let alternate = self.take(&mut conditional.alternate);
                Some(self.logical(name_copy, LogicalOperator::Coalesce, alternate))
            } else if is_undefined(&conditional.alternate) {
                // c != null ? c.p : void 0
                self.optional_member(&name_copy, &mut conditional.consequent)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(folded) = folded {
            *expression = folded;
            self.folded += 1;
        }
    }

    /// `o[k].call(o, …)` -> `o[k](…)`, also through a `(0, o[k])` wrapper,
    /// for `o` a name, `this` or a literal-keyed member path of one, and
    /// `.call` or `["call"]`.
    fn fold_method_call(&mut self, call: &mut CallExpression<'a>) {
        let (optional, callee_object) = match &call.callee {
            Expression::StaticMemberExpression(callee) if callee.property.name == "call" => (callee.optional, &callee.object),
            Expression::ComputedMemberExpression(callee)
                if matches!(&callee.expression, Expression::StringLiteral(key) if key.value == "call") =>
            {
                (callee.optional, &callee.object)
            }
            _ => return,
        };
        if optional || call.optional {
            return;
        }
        let object = match callee_object {
            Expression::StaticMemberExpression(method) if !method.optional => &method.object,
            Expression::ComputedMemberExpression(method) if !method.optional => &method.object,
            // The emitter writes an unbound member read as `(0, o.m)`;
            // `.call(o, …)` then rebinds the receiver, so the pair is
            // `o.m(…)`. The discarded comma head is a number literal.
            Expression::SequenceExpression(sequence)
                if sequence.expressions.len() == 2
                    && matches!(sequence.expressions[0], Expression::NumericLiteral(_)) =>
            {
                match &sequence.expressions[1] {
                    Expression::StaticMemberExpression(method) if !method.optional => &method.object,
                    Expression::ComputedMemberExpression(method) if !method.optional => &method.object,
                    _ => return,
                }
            }
            _ => return,
        };
        let Some(receiver) = call.arguments.first().and_then(Argument::as_expression) else {
            return;
        };
        if !same_path(object, receiver) {
            return;
        }
        let method = match self.take(&mut call.callee) {
            Expression::StaticMemberExpression(callee) => callee.unbox().object,
            Expression::ComputedMemberExpression(callee) => callee.unbox().object,
            _ => unreachable!("matched above"),
        };
        let method = match method {
            Expression::SequenceExpression(mut sequence) => sequence
                .expressions
                .pop()
                .unwrap_or_else(|| unreachable!("matched above")),
            method => method,
        };
        call.callee = method;
        call.arguments.remove(0);
        self.folded += 1;
    }
}

impl<'a> VisitMut<'a> for Resugar<'a> {
    fn visit_expression(&mut self, expression: &mut Expression<'a>) {
        walk_mut::walk_expression(self, expression);
        if let Expression::CallExpression(call) = expression {
            self.fold_method_call(call);
        }
        self.fold_conditional(expression);
    }
}
