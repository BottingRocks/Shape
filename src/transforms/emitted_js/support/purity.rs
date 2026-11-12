//! Conservative expression-purity analysis for the post-emission cleanup pass.
//!
//! "Pure" here is deliberately narrower than "has no side effects": the
//! expression must also be unable to throw, because removing a declaration
//! whose initializer would have thrown changes program behaviour. Identifier
//! reads are rejected outright, so the analysis never has to reason about
//! uninitialized `let`/`const` bindings, unresolved globals, or `typeof`
//! guards, and every coercion it accepts is over literal primitives only.
//!
//! Everything else (calls, member reads, assignments, updates, spreads,
//! template substitutions, class expressions, `await`/`yield`, tagged
//! templates, `new`, `in`, `instanceof`, `delete`) is treated as effectful and
//! retained.

use oxc_ast::ast::{
    ArrayExpressionElement, BinaryOperator, Expression, ObjectPropertyKind, PropertyKey, UnaryOperator,
};

/// The primitive kind of a literal-only expression. Used to allow arithmetic
/// that cannot invoke user coercion hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrimitiveKind {
    Number,
    String,
    Boolean,
    Null,
    BigInt,
}

/// Is `expression` safe to evaluate and then discard?
pub(crate) fn expression_is_pure(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::StringLiteral(_) => true,
        // A regex literal allocates a fresh object but cannot run user code.
        Expression::RegExpLiteral(_) => true,
        Expression::TemplateLiteral(template) => template.expressions.is_empty(),
        // `this` throws before `super()` in a derived constructor. This helper
        // has no constructor context, so retain it everywhere.
        Expression::MetaProperty(_) => true,
        Expression::ArrayExpression(array) => array.elements.iter().all(|element| match element {
            ArrayExpressionElement::Elision(_) => true,
            ArrayExpressionElement::SpreadElement(_) => false,
            _ => element.as_expression().is_some_and(expression_is_pure),
        }),
        Expression::ObjectExpression(object) => {
            object.properties.iter().all(|property| match property {
                ObjectPropertyKind::SpreadProperty(_) => false,
                ObjectPropertyKind::ObjectProperty(property) => {
                    // A computed literal key converts without running code.
                    (!property.computed
                        || matches!(property.key, PropertyKey::StringLiteral(_) | PropertyKey::NumericLiteral(_)))
                        && expression_is_pure(&property.value)
                }
            })
        }
        // Creating a closure captures the environment but executes nothing.
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
        Expression::ConditionalExpression(conditional) => {
            expression_is_pure(&conditional.test)
                && expression_is_pure(&conditional.consequent)
                && expression_is_pure(&conditional.alternate)
        }
        Expression::LogicalExpression(logical) => {
            expression_is_pure(&logical.left) && expression_is_pure(&logical.right)
        }
        Expression::SequenceExpression(sequence) => {
            sequence.expressions.iter().all(expression_is_pure)
        }
        Expression::UnaryExpression(unary) => match unary.operator {
            // These operators never coerce their operand beyond ToBoolean,
            // which has no user-visible hook.
            UnaryOperator::LogicalNot | UnaryOperator::Void | UnaryOperator::Typeof => {
                expression_is_pure(&unary.argument)
            }
            // Arithmetic coercion of a literal primitive cannot call user code.
            UnaryOperator::UnaryPlus => matches!(
                primitive_literal_kind(&unary.argument),
                Some(
                    PrimitiveKind::Number
                        | PrimitiveKind::String
                        | PrimitiveKind::Boolean
                        | PrimitiveKind::Null
                )
            ),
            UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => matches!(
                primitive_literal_kind(&unary.argument),
                Some(
                    PrimitiveKind::Number
                        | PrimitiveKind::String
                        | PrimitiveKind::Boolean
                        | PrimitiveKind::Null
                        | PrimitiveKind::BigInt
                )
            ),
            UnaryOperator::Delete => false,
        },
        Expression::BinaryExpression(binary) => {
            if binary.operator.is_in() || binary.operator.is_instance_of() {
                return false;
            }
            primitive_literal_kind(expression).is_some()
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            expression_is_pure(&parenthesized.expression)
        }
        _ => false,
    }
}

/// The primitive kind of a literal-only expression, if any.
fn primitive_literal_kind(expression: &Expression<'_>) -> Option<PrimitiveKind> {
    match expression {
        Expression::BooleanLiteral(_) => Some(PrimitiveKind::Boolean),
        Expression::NullLiteral(_) => Some(PrimitiveKind::Null),
        Expression::NumericLiteral(_) => Some(PrimitiveKind::Number),
        Expression::BigIntLiteral(_) => Some(PrimitiveKind::BigInt),
        Expression::StringLiteral(_) => Some(PrimitiveKind::String),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
            Some(PrimitiveKind::String)
        }
        Expression::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::LogicalNot if expression_is_pure(&unary.argument) => {
                Some(PrimitiveKind::Boolean)
            }
            UnaryOperator::Typeof if expression_is_pure(&unary.argument) => {
                Some(PrimitiveKind::String)
            }
            UnaryOperator::UnaryPlus => match primitive_literal_kind(&unary.argument) {
                Some(
                    PrimitiveKind::Number
                    | PrimitiveKind::String
                    | PrimitiveKind::Boolean
                    | PrimitiveKind::Null,
                ) => Some(PrimitiveKind::Number),
                _ => None,
            },
            UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => {
                match primitive_literal_kind(&unary.argument) {
                    Some(PrimitiveKind::BigInt) => Some(PrimitiveKind::BigInt),
                    Some(
                        PrimitiveKind::Number
                        | PrimitiveKind::String
                        | PrimitiveKind::Boolean
                        | PrimitiveKind::Null,
                    ) => Some(PrimitiveKind::Number),
                    _ => None,
                }
            }
            _ => None,
        },
        Expression::BinaryExpression(binary) => {
            if binary.operator.is_in() || binary.operator.is_instance_of() {
                return None;
            }
            let left = primitive_literal_kind(&binary.left)?;
            let right = primitive_literal_kind(&binary.right)?;
            // BigInt/Number mixing throws; String + BigInt is also excluded
            // rather than special-cased.
            if left == PrimitiveKind::BigInt || right == PrimitiveKind::BigInt {
                return None;
            }
            Some(match binary.operator {
                BinaryOperator::Equality
                | BinaryOperator::Inequality
                | BinaryOperator::StrictEquality
                | BinaryOperator::StrictInequality
                | BinaryOperator::LessThan
                | BinaryOperator::LessEqualThan
                | BinaryOperator::GreaterThan
                | BinaryOperator::GreaterEqualThan => PrimitiveKind::Boolean,
                BinaryOperator::Addition => {
                    if left == PrimitiveKind::String || right == PrimitiveKind::String {
                        PrimitiveKind::String
                    } else {
                        PrimitiveKind::Number
                    }
                }
                _ => PrimitiveKind::Number,
            })
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            primitive_literal_kind(&parenthesized.expression)
        }
        _ => None,
    }
}
