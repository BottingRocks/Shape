//! Drop `(0, e)` comma wrappers that mean nothing and reprint with only the
//! parentheses precedence needs.
//!
//! The emitter writes `(0, e)` so that a member read used as a callee runs
//! with a bare-call `this`. After cleanup most of those values are no longer
//! callees, and text-splicing passes leave defensive parentheses behind.
//! The final tidy (stage 13b of each function) parses once, replaces `(0, e)` by `e` everywhere except
//! where the comma is observable — a tag callee (`this`; a plain name is
//! called without one either way, absent `with`), a `delete`
//! operand (a reference versus a value), and `typeof` over an identifier (an
//! unresolved name throws only through the comma). A member callee
//! `(0, o.m)(x)` is written the way the emitter writes an unbound call,
//! `o.m.call(void 0, x)`, assuming the standard `Function.prototype.call`
//! like the tuple passes assume the standard Array iterator. It prints the program
//! with Oxc's codegen, which emits the minimal parentheses. It indents with
//! tabs like the linked program, so reprinting does not grow the file. Bare
//! `;` statements that deleted statements leave in a list are dropped (a
//! loop or `if` body `;` is not in a list and stays), as are empty `else`
//! arms; an empty `if` arm before a non-empty `else` negates the test.
//! A counter step `x = Number(x) + 1` (or `- 1`, or `x = x - 1`) is `++x` /
//! `--x`.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_allocator::CloneIn;
use oxc_ast::AstBuilder;
use oxc_span::{ContentEq, GetSpan, SPAN};
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_codegen::{Codegen, CodegenOptions, IndentChar};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator, UpdateOperator};

use crate::analysis;

mod loops;
mod chains;
mod unwrap;
mod resugar;
#[cfg(test)]
mod tests;

pub(crate) use resugar::resugar_expressions;
use loops::*;
use resugar::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TidyExpressionsResult {
    pub code: String,
    pub unwrapped: usize,
}

pub(crate) fn tidy_expressions(source: &str) -> Result<TidyExpressionsResult, String> {
    tidy(source, false)
}

/// The last tidy also writes counter steps as `++`/`--`, which earlier
/// passes would read as an opaque update.
pub(crate) fn tidy_final_expressions(source: &str) -> Result<TidyExpressionsResult, String> {
    tidy(source, true)
}

fn tidy(source: &str, counter_steps: bool) -> Result<TidyExpressionsResult, String> {
    let allocator = Allocator::default();
    let mut program = analysis::parse_program(&allocator, source)?;
    let mut with = WithStatements::default();
    with.visit_program(&program);
    let (own_names, read_counts) = if counter_steps { own_names(&program) } else { Default::default() };
    let mut unwrap = Unwrap {
        builder: AstBuilder::new(&allocator),
        keep: false,
        plain_callees: !with.found,
        counter_steps,
        own_names,
        read_counts,
        unwrapped: 0,
    };
    unwrap.visit_program(&mut program);
    Ok(TidyExpressionsResult {
        code: Codegen::new()
            .with_options(CodegenOptions {
                indent_char: IndentChar::Tab,
                indent_width: 1,
                ..CodegenOptions::default()
            })
            .build(&program)
            .code,
        unwrapped: unwrap.unwrapped,
    })
}

#[derive(Default)]
struct WithStatements {
    found: bool,
}

impl<'a> Visit<'a> for WithStatements {
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.found = true;
        walk::walk_with_statement(self, statement);
    }
}

struct Unwrap<'a> {
    builder: AstBuilder<'a>,
    /// The next expression visited keeps its own comma wrapper.
    keep: bool,
    /// `(0, name)()` may become `name()`: no `with` can bind the name.
    plain_callees: bool,
    counter_steps: bool,
    /// Names whose every binding is referenced only by its own function: no
    /// call can read them.
    own_names: std::collections::HashSet<String>,
    /// References per name.
    read_counts: std::collections::HashMap<String, usize>,
    unwrapped: usize,
}

/// Names only their declaring function references, and reference counts.
fn own_names(program: &Program<'_>) -> (std::collections::HashSet<String>, std::collections::HashMap<String, usize>) {
    use oxc_ast::AstKind;
    let (scoping, nodes) = analysis::analyze(program);
    let function_of = |node: oxc_semantic::NodeId| {
        nodes
            .ancestor_ids(node)
            .find(|ancestor| matches!(nodes.kind(*ancestor), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
    };
    let mut own = std::collections::HashSet::new();
    let mut shared = std::collections::HashSet::new();
    let mut counts = std::collections::HashMap::new();
    for symbol in scoping.symbol_ids() {
        let name = scoping.symbol_name(symbol).to_string();
        let home = function_of(scoping.symbol_declaration(symbol));
        let mut local = true;
        for reference in scoping.get_resolved_references(symbol) {
            *counts.entry(name.clone()).or_insert(0) += 1;
            if function_of(reference.node_id()) != home {
                local = false;
            }
        }
        if local {
            own.insert(name);
        } else {
            shared.insert(name);
        }
    }
    for name in scoping.root_unresolved_references().keys() {
        shared.insert(name.to_string());
    }
    own.retain(|name| !shared.contains(name));
    // A name with one binding in the whole program (and no global use)
    // can move to an outer block without meeting another.
    let mut bindings: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for symbol in scoping.symbol_ids() {
        *bindings.entry(scoping.symbol_name(symbol).to_string()).or_insert(0) += 1;
    }
    for name in scoping.root_unresolved_references().keys() {
        *bindings.entry(name.to_string()).or_insert(0) += 2;
    }
    for (name, count) in bindings {
        if count == 1 {
            counts.entry(format!("\u{0}unique {name}")).or_insert(0);
        }
    }
    // A block-scoped binding can move to an enclosing block of its function
    // when no other binding of the name lives in that function's own scopes
    // and every use of the name in the function is this binding.
    let mut by_name: std::collections::HashMap<&str, Vec<oxc_semantic::SymbolId>> = std::collections::HashMap::new();
    for symbol in scoping.symbol_ids() {
        by_name.entry(scoping.symbol_name(symbol)).or_default().push(symbol);
    }
    for symbol in scoping.symbol_ids() {
        let name = scoping.symbol_name(symbol);
        let home = function_of(scoping.symbol_declaration(symbol));
        let home_span = home.map(|home| nodes.kind(home).span());
        let inside = |node: oxc_semantic::NodeId| {
            let span = nodes.kind(node).span();
            home_span.is_none_or(|home| home.start <= span.start && span.end <= home.end)
        };
        let clash = by_name[name].iter().any(|other| {
            *other != symbol && {
                let declaration = scoping.symbol_declaration(*other);
                // Another binding in the function's own scopes, or an outer
                // one used inside the function.
                function_of(declaration) == home
                    || (!inside(declaration)
                        && scoping.get_resolved_references(*other).any(|reference| inside(reference.node_id())))
            }
        }) || scoping
            .root_unresolved_references()
            .get(name)
            .is_some_and(|references| references.iter().any(|reference| inside(scoping.get_reference(*reference).node_id())));
        if !clash {
            counts.entry(format!("\u{0}movable {}", symbol.index())).or_insert(0);
        }
    }
    (own, counts)
}

/// `expression` mentions `name`.
fn mentions(expression: &Expression<'_>, name: &str) -> bool {
    struct Find<'n> {
        name: &'n str,
        found: bool,
    }
    impl<'a> Visit<'a> for Find<'_> {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            if identifier.name == self.name {
                self.found = true;
            }
        }
    }
    let mut find = Find { name, found: false };
    find.visit_expression(expression);
    find.found
}

/// The literal text of `"a" + "b"`, `"a" + 2`, or `(e + "a") + "b"`.
fn joined_strings(expression: &Expression<'_>) -> Option<String> {
    let Expression::BinaryExpression(binary) = expression else {
        return None;
    };
    if binary.operator != BinaryOperator::Addition {
        return None;
    }
    let text = |literal: &Expression<'_>| match literal {
        Expression::StringLiteral(string) if !string.value.contains('\u{fffd}') => Some(string.value.to_string()),
        Expression::NumericLiteral(number)
            if number.value.fract() == 0.0 && number.value.abs() <= 9007199254740991.0 =>
        {
            Some(format!("{}", number.value as i64))
        }
        _ => None,
    };
    let right = text(&binary.right)?;
    match &binary.left {
        Expression::StringLiteral(left) => Some(format!("{}{right}", left.value)),
        Expression::NumericLiteral(_) if matches!(binary.right, Expression::StringLiteral(_)) => {
            Some(format!("{}{right}", text(&binary.left)?))
        }
        Expression::BinaryExpression(inner) if inner.operator == BinaryOperator::Addition => match &inner.right {
            Expression::StringLiteral(left) => Some(format!("{}{right}", left.value)),
            _ => None,
        },
        _ => None,
    }
}

/// One spelling for every comparison and test, so the same code reads the
/// same across seeds (shape/'s flip-binaries and negative-charge passes):
/// a literal goes right of `===`, `!==`, `==`, `!=`, `|`, `&`, `^` and `*`
/// (evaluating a literal first or last does nothing); `!(a !== b)` is
/// `a === b`; and `!t ? a : b` is `t ? b : a`.
fn normalize<'a>(expression: &mut Expression<'a>, builder: AstBuilder<'a>) {
    fn literal(expression: &Expression<'_>) -> bool {
        match expression {
            Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => true,
            Expression::UnaryExpression(unary) => {
                matches!(unary.operator, UnaryOperator::Void | UnaryOperator::UnaryNegation)
                    && matches!(unary.argument, Expression::NumericLiteral(_))
            }
            _ => false,
        }
    }
    match expression {
        Expression::BinaryExpression(binary) => {
            if matches!(
                binary.operator,
                BinaryOperator::StrictEquality
                    | BinaryOperator::StrictInequality
                    | BinaryOperator::Equality
                    | BinaryOperator::Inequality
                    | BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseXOR
                    | BinaryOperator::Multiplication
            ) && literal(&binary.left)
                && !literal(&binary.right)
            {
                let binary = &mut **binary;
                std::mem::swap(&mut binary.left, &mut binary.right);
            }
        }
        Expression::UnaryExpression(unary)
            if unary.operator == UnaryOperator::LogicalNot
                && matches!(unary.argument.without_parentheses(), Expression::LogicalExpression(inner)
                    if inner.operator == oxc_syntax::operator::LogicalOperator::Or
                        && matches!(&inner.left, Expression::UnaryExpression(l) if l.operator == UnaryOperator::LogicalNot)
                        && matches!(&inner.right, Expression::UnaryExpression(r) if r.operator == UnaryOperator::LogicalNot && boolean(&r.argument))) =>
        {
            // !(!a || !b) is !!a && b for a boolean b
            let argument = std::mem::replace(&mut unary.argument, builder.expression_null_literal(SPAN));
            let Expression::LogicalExpression(inner) = argument.into_inner_expression() else { unreachable!() };
            let inner = inner.unbox();
            let Expression::UnaryExpression(left) = inner.left else { unreachable!() };
            let Expression::UnaryExpression(right) = inner.right else { unreachable!() };
            let left = left.unbox().argument;
            let left = if boolean(&left) {
                left
            } else {
                let once = builder.expression_unary(SPAN, UnaryOperator::LogicalNot, left);
                builder.expression_unary(SPAN, UnaryOperator::LogicalNot, once)
            };
            *expression = builder.expression_logical(SPAN, left, oxc_syntax::operator::LogicalOperator::And, right.unbox().argument);
        }
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
            if let Expression::BinaryExpression(inner) = unary.argument.without_parentheses() {
                if matches!(
                    inner.operator,
                    BinaryOperator::StrictEquality | BinaryOperator::StrictInequality | BinaryOperator::Equality | BinaryOperator::Inequality
                ) {
                    let argument = std::mem::replace(&mut unary.argument, builder.expression_null_literal(SPAN));
                    let Expression::BinaryExpression(mut inner) = argument.into_inner_expression() else {
                        unreachable!();
                    };
                    inner.operator = match inner.operator {
                        BinaryOperator::StrictEquality => BinaryOperator::StrictInequality,
                        BinaryOperator::StrictInequality => BinaryOperator::StrictEquality,
                        BinaryOperator::Equality => BinaryOperator::Inequality,
                        _ => BinaryOperator::Equality,
                    };
                    *expression = Expression::BinaryExpression(inner);
                    normalize(expression, builder);
                }
            }
        }
        Expression::ConditionalExpression(choice)
            if int32(&choice.alternate)
                && matches!(choice.test.without_parentheses(), Expression::BinaryExpression(test)
                    if test.operator == BinaryOperator::StrictEquality && test.left.content_eq(&test.right)) =>
        {
            // `x === x ? v ^ x : v` with a 32-bit `v`: a NaN `x` gives `v ^ 0`,
            // which is `v`.
            drop_nan_guard(expression, builder);
        }
        Expression::ConditionalExpression(choice) => {
            if let Expression::UnaryExpression(unary) = &mut choice.test {
                if unary.operator == UnaryOperator::LogicalNot {
                    let test = std::mem::replace(&mut unary.argument, builder.expression_null_literal(SPAN));
                    let choice = &mut **choice;
                    choice.test = test;
                    std::mem::swap(&mut choice.consequent, &mut choice.alternate);
                }
            }
        }
        _ => {}
    }
}

/// A 32-bit integer: an integer literal in range, or a bitwise result.
fn int32(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::NumericLiteral(number) => {
            number.value.fract() == 0.0 && number.value >= -2147483648.0 && number.value <= 2147483647.0
        }
        Expression::BinaryExpression(binary) => matches!(
            binary.operator,
            BinaryOperator::BitwiseOR
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseXOR
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
        ),
        Expression::UnaryExpression(unary) => unary.operator == UnaryOperator::BitwiseNot,
        Expression::ConditionalExpression(choice) => int32(&choice.consequent) && int32(&choice.alternate),
        _ => false,
    }
}

/// In a bitwise operand, `x === x ? v ^ x : v` is `v ^ x`: a NaN `x`
/// makes `v ^ x` the 32-bit `v`, which is what the outer operator reads
/// from `v` anyway (also for `|`).
fn drop_nan_guard<'a>(expression: &mut Expression<'a>, builder: AstBuilder<'a>) {
    let target = match expression {
        Expression::ParenthesizedExpression(inner) => &mut inner.expression,
        other => other,
    };
    let Expression::ConditionalExpression(choice) = target else {
        return;
    };
    let guarded = match choice.test.without_parentheses() {
        Expression::BinaryExpression(test)
            if test.operator == BinaryOperator::StrictEquality
                && matches!((&test.left, &test.right), (Expression::Identifier(a), Expression::Identifier(b)) if a.name == b.name) =>
        {
            let Expression::Identifier(name) = &test.left else { unreachable!() };
            name.name
        }
        _ => return,
    };
    let fits = matches!(choice.consequent.without_parentheses(), Expression::BinaryExpression(step)
        if matches!(step.operator, BinaryOperator::BitwiseXOR | BinaryOperator::BitwiseOR)
            && ((matches!(&step.right, Expression::Identifier(x) if x.name == guarded) && step.left.content_eq(&choice.alternate))
                || (matches!(&step.left, Expression::Identifier(x) if x.name == guarded) && step.right.content_eq(&choice.alternate))));
    if fits {
        let consequent = std::mem::replace(&mut choice.consequent, builder.expression_null_literal(SPAN));
        *target = consequent;
    }
}

/// `!!x && …` as `x && …` (the leftmost operand of an `&&` chain).
fn strip_double_not<'a>(expression: &mut Expression<'a>, builder: AstBuilder<'a>) {
    match expression {
        Expression::ParenthesizedExpression(inner) => strip_double_not(&mut inner.expression, builder),
        Expression::LogicalExpression(logical) if logical.operator == oxc_syntax::operator::LogicalOperator::And => {
            strip_double_not(&mut logical.left, builder);
        }
        Expression::UnaryExpression(unary)
            if unary.operator == UnaryOperator::LogicalNot
                && matches!(&unary.argument, Expression::UnaryExpression(inner) if inner.operator == UnaryOperator::LogicalNot) =>
        {
            let Expression::UnaryExpression(inner) =
                std::mem::replace(&mut unary.argument, builder.expression_null_literal(SPAN))
            else {
                unreachable!();
            };
            *expression = inner.unbox().argument;
        }
        _ => {}
    }
}

/// `(0, name)`: a comma around a plain identifier.
fn wraps_identifier(expression: &Expression<'_>) -> bool {
    matches!(expression, Expression::SequenceExpression(sequence)
        if sequence.expressions.len() == 2
            && matches!(sequence.expressions[0], Expression::NumericLiteral(_))
            && matches!(sequence.expressions[1], Expression::Identifier(_)))
}

/// `(0, o.m)`: a comma around a member read.
fn wraps_member(expression: &Expression<'_>) -> bool {
    matches!(expression, Expression::SequenceExpression(sequence)
        if sequence.expressions.len() == 2
            && matches!(sequence.expressions[0], Expression::NumericLiteral(_))
            && sequence.expressions[1].is_member_expression())
}


