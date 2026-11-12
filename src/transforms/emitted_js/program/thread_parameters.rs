//! Give thread functions real parameters, and call function values
//! directly.
//!
//! Threads read their arguments as `arguments[n]`. A function (not an
//! arrow) that reads its own `arguments` only that way gets parameters
//! `$PARAM_0…$PARAM_n` instead, for the positional naming after it: with
//! nothing reading the `arguments` object, the sloppy-mode link between it
//! and the parameters cannot show. (Only the function's `length` changes.)
//!
//! Then `f.call(void 0, …)` with `f` a variable is `f(…)`: a plain call
//! passes `undefined` as `this` too. A member callee keeps the `.call`,
//! since `o.f(…)` would pass `o`.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};

use crate::analysis;

pub(crate) fn thread_parameters(source: &str) -> Result<(String, usize, usize), String> {
    let (code, parameterized) = parameters(source)?;
    let (code, calls) = direct_calls(&code)?;
    Ok((code, parameterized, calls))
}

fn parameters(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (_, nodes) = analysis::analyze(&program);
    let mut edits = Vec::new();
    let mut count = 0;
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        if !function.params.items.is_empty() || function.params.rest.is_some() || function.body.is_none() {
            continue;
        }
        let mut uses = Uses::default();
        uses.visit_function_body(function.body.as_ref().unwrap());
        if uses.other || uses.indexed.is_empty() {
            continue;
        }
        let last = uses.indexed.iter().map(|(_, index)| *index).max().unwrap_or(0);
        let names: Vec<String> = (0..=last).map(|index| format!("$PARAM_{index}")).collect();
        edits.push(analysis::Edit::replace(function.params.span, format!("({})", names.join(", "))));
        for (span, index) in uses.indexed {
            edits.push(analysis::Edit::replace(span, format!("$PARAM_{index}")));
        }
        count += 1;
    }
    if count == 0 {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("thread parameter edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("thread parameter output: {error}"))?;
    Ok((applied.output, count))
}

/// A function's own `arguments` uses (arrows included, nested ordinary
/// functions not).
#[derive(Default)]
struct Uses {
    depth: usize,
    indexed: Vec<(Span, usize)>,
    /// Any other use of `arguments`.
    other: bool,
}

impl<'a> Visit<'a> for Uses {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.depth += 1;
        walk::walk_function(self, function, flags);
        self.depth -= 1;
    }
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if self.depth == 0 {
            if let (Expression::Identifier(object), Expression::NumericLiteral(index)) = (&member.object, &member.expression) {
                if object.name == "arguments" && index.value.fract() == 0.0 && index.value >= 0.0 && index.value < 64.0 {
                    self.indexed.push((member.span, index.value as usize));
                    return;
                }
            }
        }
        walk::walk_computed_member_expression(self, member);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if self.depth == 0 && identifier.name == "arguments" {
            self.other = true;
        }
    }
    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        // A binding named `$PARAM_n` already would collide.
        if identifier.name.starts_with("$PARAM_") {
            self.other = true;
        }
    }
}

fn direct_calls(source: &str) -> Result<(String, usize), String> {
    if !source.contains("call(void 0") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut calls = Calls { source, edits: Vec::new() };
    calls.visit_program(&program);
    let count = calls.edits.len();
    if count == 0 {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, calls.edits);
    if !applied.rejected.is_empty() {
        // Nested `.call`s: the outer one waits for another run.
        return Ok((source.into(), 0));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("direct call output: {error}"))?;
    Ok((applied.output, count))
}

struct Calls<'s> {
    source: &'s str,
    edits: Vec<analysis::Edit>,
}

impl<'a> Visit<'a> for Calls<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        walk::walk_call_expression(self, call);
        let (optional, object) = match &call.callee {
            Expression::StaticMemberExpression(member) if member.property.name == "call" => (member.optional, &member.object),
            Expression::ComputedMemberExpression(member) if matches!(&member.expression, Expression::StringLiteral(key) if key.value == "call") => {
                (member.optional, &member.object)
            }
            _ => return,
        };
        let Expression::Identifier(callee) = object else {
            return;
        };
        if optional || call.optional {
            return;
        }
        let Some(Argument::UnaryExpression(receiver)) = call.arguments.first() else {
            return;
        };
        if receiver.operator != oxc_syntax::operator::UnaryOperator::Void || !matches!(receiver.argument, Expression::NumericLiteral(_)) {
            return;
        }
        let rest: Vec<&str> = call.arguments.iter().skip(1).map(|argument| &self.source[argument.span().start as usize..argument.span().end as usize]).collect();
        // Keep edits that do not nest; nested ones fail the whole run.
        if self.edits.iter().any(|edit| edit.start <= call.span.start && call.span.end <= edit.end || call.span.start <= edit.start && edit.end <= call.span.end) {
            return;
        }
        self.edits.push(analysis::Edit::replace(call.span, format!("{}({})", callee.name, rest.join(", "))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_replace_indexed_arguments() {
        let source = "function $THREAD_1() {\n\tlet x = arguments[1];\n\targuments[0].call(void 0, x);\n\treturn () => arguments[0];\n}\n";
        let (code, parameterized, calls) = thread_parameters(source).unwrap();
        assert_eq!((parameterized, calls), (1, 1));
        assert_eq!(code, "function $THREAD_1($PARAM_0, $PARAM_1) {\n\tlet x = $PARAM_1;\n\t$PARAM_0(x);\n\treturn () => $PARAM_0;\n}\n");
    }

    #[test]
    fn keeps_arguments_read_as_a_whole_and_member_callees() {
        let source = "function $THREAD_1() {\n\treturn arguments[\"length\"] + arguments[0];\n}\nfunction $THREAD_2() {\n\tlet a = arguments;\n\treturn a[0];\n}\nfunction g(o) {\n\treturn o[\"f\"].call(void 0, 1);\n}\n";
        let (code, parameterized, calls) = thread_parameters(source).unwrap();
        assert_eq!((parameterized, calls), (0, 0), "{code}");
    }
}
