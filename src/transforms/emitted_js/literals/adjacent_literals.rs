//! Build a literal where it is used, when that is the next statement.
//!
//! ```js
//! let $AJ = { message: "IM10", stack: e + "" };
//! $KEY_367($AJ);
//! ```
//!
//! becomes `$KEY_367({ message: "IM10", stack: e + "" });`. The literal need
//! not be pure: it still runs right after the code before it, because
//! nothing with an effect runs between the old declaration and its use. So
//! the binding is a `let`/`const` of one object or array literal, read once,
//! in the statement right after it, outside any function that statement
//! creates; and in that statement only names, literals, `this` and property
//! paths off them are evaluated before the read: a callee and the arguments
//! before it, an assignment target, or nothing (`return x`, `let y = x`).

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_semantic::Scoping;
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn inline_adjacent_literals(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, edits: Vec::new(), moved: 0, comments: program.comments.iter().map(|c| c.span).collect() };
    super::super::support::for_each_list(&program, |list| scan.list(list));
    if scan.moved == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "adjacent literal")?;
    Ok((output, scan.moved))
}

struct Scan<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    edits: Vec<analysis::Edit>,
    moved: usize,
    comments: Vec<Span>,
}

/// Evaluating it runs no code (no getters, as everywhere in cleanup).
fn inert(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::Identifier(_)
        | Expression::ThisExpression(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::StaticMemberExpression(member) => !member.optional && inert(&member.object),
        Expression::ComputedMemberExpression(member) => {
            !member.optional
                && matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_))
                && inert(&member.object)
        }
        _ => false,
    }
}

/// `read` is evaluated first in `expression`, after inert parts only.
fn first(expression: &Expression<'_>, read: Span) -> bool {
    let expression = expression.without_parentheses();
    if expression.span() == read {
        return matches!(expression, Expression::Identifier(_));
    }
    match expression {
        Expression::CallExpression(call) => !call.optional && inert(&call.callee) && in_arguments(&call.arguments, read),
        Expression::NewExpression(new) => inert(&new.callee) && in_arguments(&new.arguments, read),
        Expression::AssignmentExpression(assignment) => {
            let target_inert = match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(_) => true,
                AssignmentTarget::StaticMemberExpression(member) => !member.optional && inert(&member.object),
                _ => false,
            };
            assignment.operator == AssignmentOperator::Assign && target_inert && first(&assignment.right, read)
        }
        _ => false,
    }
}

fn in_arguments(arguments: &[Argument<'_>], read: Span) -> bool {
    for argument in arguments {
        let Some(argument) = argument.as_expression() else {
            return false;
        };
        let span = argument.span();
        if span.start <= read.start && read.end <= span.end {
            return first(argument, read);
        }
        if !inert(argument) {
            return false;
        }
    }
    false
}

impl Scan<'_> {
    fn list(&mut self, statements: &[Statement<'_>]) {
        let mut taken = 0;
        for (index, statement) in statements.iter().enumerate() {
            // A statement that just received a literal keeps its old text in
            // the edits: it cannot also give one away.
            if index < taken {
                continue;
            }
            let Some(next) = statements.get(index + 1) else {
                continue;
            };
            if self.move_into(statement, next) {
                taken = index + 2;
            }
        }
    }

    fn move_into(&mut self, statement: &Statement<'_>, next: &Statement<'_>) -> bool {
        let Statement::VariableDeclaration(declaration) = statement else {
            return false;
        };
        let [declarator] = declaration.declarations.as_slice() else {
            return false;
        };
        let (BindingPattern::BindingIdentifier(id), Some(literal)) = (&declarator.id, &declarator.init) else {
            return false;
        };
        if !matches!(literal, Expression::ObjectExpression(_) | Expression::ArrayExpression(_)) || declaration.kind == VariableDeclarationKind::Var {
            return false;
        }
        let between = &self.source[statement.span().end as usize..next.span().start as usize];
        if !between.trim().is_empty() || self.comments.iter().any(|c| statement.span().start <= c.start && c.end <= statement.span().end) {
            return false;
        }
        let mut references = self.scoping.get_resolved_references(id.symbol_id());
        let (Some(reference), None) = (references.next(), references.next()) else {
            return false;
        };
        if !reference.is_read() || reference.is_write() {
            return false;
        }
        let Some(read) = self.read_span(next, id.name.as_str(), id.symbol_id()) else {
            return false;
        };
        let leads = match next {
            Statement::ExpressionStatement(expression) => first(&expression.expression, read),
            Statement::ReturnStatement(ret) => ret.argument.as_ref().is_some_and(|argument| first(argument, read)),
            Statement::ThrowStatement(throw) => first(&throw.argument, read),
            Statement::VariableDeclaration(declaration) => match declaration.declarations.as_slice() {
                [declarator] => declarator.init.as_ref().is_some_and(|init| first(init, read)),
                _ => false,
            },
            _ => false,
        };
        if !leads {
            return false;
        }
        let text = &self.source[literal.span().start as usize..literal.span().end as usize];
        self.edits.push(analysis::Edit::delete(whole_line(self.source, statement.span())));
        self.edits.push(analysis::Edit::replace(read, text.to_string()));
        self.moved += 1;
        true
    }

    /// The span of the one read of `symbol` in `statement`, outside the
    /// functions it creates.
    fn read_span(&self, statement: &Statement<'_>, name: &str, symbol: oxc_semantic::SymbolId) -> Option<Span> {
        struct Find<'s> {
            scoping: &'s Scoping,
            name: &'s str,
            symbol: oxc_semantic::SymbolId,
            found: Option<Span>,
        }
        impl<'a> Visit<'a> for Find<'_> {
            fn visit_identifier_reference(&mut self, reference: &IdentifierReference<'a>) {
                if reference.name == self.name && self.scoping.get_reference(reference.reference_id()).symbol_id() == Some(self.symbol) {
                    self.found = Some(reference.span);
                }
            }
            fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
            fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
            fn visit_class(&mut self, _: &Class<'a>) {}
        }
        let mut find = Find { scoping: self.scoping, name, symbol, found: None };
        find.visit_statement(statement);
        find.found
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn inline(source: &str) -> String {
        inline_adjacent_literals(source).unwrap().0
    }

    #[test]
    fn builds_an_error_object_in_its_call() {
        let source = "function f(e) {\n\tlet a = {\n\t\tmessage: \"IM10\",\n\t\tstack: e + \"\"\n\t};\n\treport(a);\n\tlet b = [e];\n\treturn o.p.notify(1, b);\n}\n";
        assert_eq!(inline(source), "function f(e) {\n\treport({\n\t\tmessage: \"IM10\",\n\t\tstack: e + \"\"\n\t});\n\treturn o.p.notify(1, [e]);\n}\n");
    }

    #[test]
    fn keeps_a_literal_that_would_run_after_other_code() {
        // `g()` runs before the read; `b` is read twice; `c` in a closure;
        // `d` is not used by the next statement.
        let source = "function f(e) {\n\tlet a = { s: e + \"\" };\n\treport(g(), a);\n\tlet b = [];\n\tb.push(b);\n\tlet c = {};\n\th(() => c);\n\tlet d = {};\n\th();\n\treturn d;\n}\n";
        assert_eq!(inline(source), source);
    }
}
