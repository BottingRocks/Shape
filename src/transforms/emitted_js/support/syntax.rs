//! Small syntax questions the passes share.

use oxc_ast::ast::*;
use oxc_ast::AstKind;

pub(crate) fn unwrap<'s, 'a>(mut e: &'s Expression<'a>) -> &'s Expression<'a> {
    while let Expression::ParenthesizedExpression(p) = e {
        e = &p.expression;
    }
    e
}

pub(crate) fn is_loop(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
    )
}

pub(crate) fn is_void_zero(expression: &Expression<'_>) -> bool {
    matches!(expression.get_inner_expression(), Expression::UnaryExpression(unary) if unary.operator == oxc_syntax::operator::UnaryOperator::Void && matches!(unary.argument, Expression::NumericLiteral(_)))
}
