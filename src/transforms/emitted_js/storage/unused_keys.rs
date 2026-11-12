//! Drop `$KEY_` slots nothing reads.
//!
//! The tracer declares every slot of a thread's memory and starts each at
//! `void 0`; cleanup removes the stores nothing reads, which leaves slots
//! the program never touches: the VM's scratch registers, and the few a
//! later pass consumed (a folded fingerprint's regexes, a class helper).
//! This runs on the whole program, where a child thread's reads resolve to
//! the slot its parent declares, so a slot with no read at all is unused
//! everywhere. It goes when its initializer and every store to it run no
//! code, each store being a statement of its own (`$KEY_n = value;`); a
//! store anywhere else keeps the slot. Threads the deleted stores named
//! stay: a thread nothing calls is for the tracer to explain, not for
//! cleanup to hide.

use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::Span;

use super::super::purity::expression_is_pure;
use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn remove_unused_keys(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut stores = Stores { scoping: &scoping, stores: HashMap::new() };
    super::super::support::for_each_list(&program, |list| stores.list(list));
    let mut scan = Scan { source, scoping: &scoping, stores: stores.stores, edits: Vec::new(), removed: 0 };
    super::super::support::for_each_list(&program, |list| scan.list(list));
    if scan.removed == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "unused key")?;
    Ok((output, scan.removed))
}

/// `$KEY_n = value;` statements in statement lists, by slot: (statement,
/// whether `value` runs no code).
struct Stores<'s> {
    scoping: &'s Scoping,
    stores: HashMap<SymbolId, Vec<(Span, bool)>>,
}

impl Stores<'_> {
    fn list(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let Statement::ExpressionStatement(statement) = statement else {
                continue;
            };
            let Expression::AssignmentExpression(assignment) = &statement.expression else {
                continue;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                continue;
            };
            if assignment.operator != AssignmentOperator::Assign || !target.name.starts_with("$KEY_") {
                continue;
            }
            if let Some(symbol) = self.scoping.get_reference(target.reference_id()).symbol_id() {
                let quiet = quiet(self.scoping, &assignment.right);
                self.stores.entry(symbol).or_default().push((statement.span, quiet));
            }
        }
    }
}


struct Scan<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    stores: HashMap<SymbolId, Vec<(Span, bool)>>,
    edits: Vec<analysis::Edit>,
    removed: usize,
}

fn regexp(callee: &Expression<'_>, arguments: &[Argument<'_>]) -> bool {
    matches!(callee, Expression::Identifier(name) if name.name == "RegExp")
        && !arguments.is_empty()
        && arguments.len() <= 2
        && arguments.iter().all(|argument| matches!(argument, Argument::StringLiteral(_)))
}

fn global(scoping: &Scoping, expression: &Expression<'_>, name: &str) -> bool {
    matches!(expression, Expression::Identifier(identifier)
        if identifier.name == name && scoping.get_reference(identifier.reference_id()).symbol_id().is_none())
}

/// Creating the value runs no code: a pure literal, a function, a global
/// `RegExp` of string literals (the original program built it already), a
/// variable declared before it, a random tag, or `a ?? b` of those.
fn quiet(scoping: &Scoping, init: &Expression<'_>) -> bool {
    match init.get_inner_expression() {
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
        Expression::CallExpression(call) => {
            (regexp(&call.callee, &call.arguments) && global(scoping, &call.callee, "RegExp")) || random_text(scoping, init)
        }
        Expression::NewExpression(new) => regexp(&new.callee, &new.arguments) && global(scoping, &new.callee, "RegExp"),
        Expression::Identifier(name) => scoping
            .get_reference(name.reference_id())
            .symbol_id()
            .is_some_and(|symbol| scoping.symbol_span(symbol).end <= name.span.start || scoping.symbol_flags(symbol).is_function()),
        Expression::LogicalExpression(logical) => quiet(scoping, &logical.left) && quiet(scoping, &logical.right),
        Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
            random_text(scoping, init) || expression_is_pure(init)
        }
        other => expression_is_pure(other),
    }
}

/// A random tag such as `"x" + Math.random().toString(36).slice(2)`: a
/// number from the global `Math.random()`, written with a literal radix,
/// sliced with literal bounds, and joined to string literals. None of it
/// can throw, and the only effect is a draw nothing can observe.
fn random_text(scoping: &Scoping, expression: &Expression<'_>) -> bool {
    let literal_numbers = |arguments: &[Argument<'_>]| arguments.iter().all(|argument| matches!(argument, Argument::NumericLiteral(_)));
    match expression.get_inner_expression() {
        Expression::StringLiteral(_) => true,
        Expression::BinaryExpression(binary) => {
            binary.operator == BinaryOperator::Addition && random_text(scoping, &binary.left) && random_text(scoping, &binary.right)
        }
        Expression::CallExpression(call) if !call.optional => {
            // `o.m(…)` or, before property names are dotted, `o["m"](…)`.
            let Some(member) = call.callee.get_inner_expression().as_member_expression() else {
                return false;
            };
            if member.optional() {
                return false;
            }
            match member.static_property_name().unwrap_or_default() {
                "slice" | "substring" | "substr" => {
                    call.arguments.len() <= 2 && literal_numbers(&call.arguments) && random_text(scoping, member.object())
                }
                "toString" => {
                    let radix_ok = match call.arguments.as_slice() {
                        [] => true,
                        [Argument::NumericLiteral(radix)] => (2.0..=36.0).contains(&radix.value) && radix.value.fract() == 0.0,
                        _ => false,
                    };
                    radix_ok && random_number(scoping, member.object())
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn random_number(scoping: &Scoping, expression: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = expression.get_inner_expression() else {
        return false;
    };
    let Some(member) = call.callee.get_inner_expression().as_member_expression() else {
        return false;
    };
    !call.optional
        && !member.optional()
        && call.arguments.is_empty()
        && member.static_property_name() == Some("random")
        && global(scoping, member.object(), "Math")
}

impl Scan<'_> {
    fn unused(&self, declarator: &VariableDeclarator<'_>) -> bool {
        let BindingPattern::BindingIdentifier(id) = &declarator.id else {
            return false;
        };
        if !id.name.starts_with("$KEY_") || !declarator.init.as_ref().is_none_or(|init| quiet(self.scoping, init)) {
            return false;
        }
        let symbol = id.symbol_id();
        let mut references = 0;
        for reference in self.scoping.get_resolved_references(symbol) {
            if reference.is_read() {
                return false;
            }
            references += 1;
        }
        // Every write is a store statement this pass can delete.
        let stores = self.stores.get(&symbol).map_or(&[][..], Vec::as_slice);
        stores.len() == references && stores.iter().all(|&(_, quiet)| quiet)
    }

    fn list(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            if let Statement::VariableDeclaration(declaration) = statement {
                self.declaration(declaration);
            }
        }
    }

    fn declaration(&mut self, declaration: &VariableDeclaration<'_>) {
        let (gone, kept): (Vec<_>, Vec<_>) = declaration.declarations.iter().partition(|declarator| self.unused(declarator));
        if gone.is_empty() {
            return;
        }
        self.removed += gone.len();
        for declarator in &gone {
            let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                continue;
            };
            for &(span, _) in self.stores.get(&id.symbol_id()).into_iter().flatten() {
                self.edits.push(analysis::Edit::delete(whole_line(self.source, span)));
            }
        }
        if kept.is_empty() {
            self.edits.push(analysis::Edit::delete(whole_line(self.source, declaration.span)));
        } else {
            let kept: Vec<&str> = kept.iter().map(|declarator| &self.source[declarator.span.start as usize..declarator.span.end as usize]).collect();
            self.edits.push(analysis::Edit::replace(declaration.span, format!("{} {};", declaration.kind.as_str(), kept.join(", "))));
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_slots_nothing_touches() {
        let source = "function $THREAD_1($A) {\n\tlet $KEY_1, $KEY_2, $KEY_3;\n\tlet $KEY_7 = $A ?? \"x\";\n\tlet $KEY_4 = void 0;\n\tlet $KEY_5 = RegExp(\"\\\\s\", \"g\");\n\tlet $KEY_6 = f();\n\tfunction $THREAD_2() {\n\t\treturn $KEY_2;\n\t}\n\treturn $THREAD_2;\n}\n";
        let (code, removed) = remove_unused_keys(source).unwrap();
        assert_eq!(removed, 5);
        // $KEY_2 is read by a child; $KEY_6's initializer runs code.
        assert_eq!(code, "function $THREAD_1($A) {\n\tlet $KEY_2;\n\tlet $KEY_6 = f();\n\tfunction $THREAD_2() {\n\t\treturn $KEY_2;\n\t}\n\treturn $THREAD_2;\n}\n");
    }

    #[test]
    fn drops_slots_only_written() {
        let source = "function $THREAD_1($A) {\n\tlet $KEY_5 = \"x\" + Math[\"random\"]()[\"toString\"](36)[\"slice\"](2);\n\tlet $KEY_1, $KEY_2, $KEY_3, $KEY_4;\n\tfunction $THREAD_2() {\n\t\t$KEY_1 = [];\n\t\t$KEY_2 = $A.b;\n\t\tif ($A) $KEY_3 = 0;\n\t}\n\t$KEY_4 = $THREAD_3;\n\tfunction $THREAD_3() {\n\t}\n\t$KEY_1 = \"x\" + Math.random().toString(36).slice(2);\n\treturn $THREAD_2;\n}\n";
        let (code, removed) = remove_unused_keys(source).unwrap();
        assert_eq!(removed, 3);
        // $KEY_2's store can throw; $KEY_3's is not a statement of its own;
        // $THREAD_3 stays for whoever asks why nothing calls it.
        assert_eq!(code, "function $THREAD_1($A) {\n\tlet $KEY_2, $KEY_3;\n\tfunction $THREAD_2() {\n\t\t$KEY_2 = $A.b;\n\t\tif ($A) $KEY_3 = 0;\n\t}\n\tfunction $THREAD_3() {\n\t}\n\treturn $THREAD_2;\n}\n");
    }

    #[test]
    fn keeps_random_tags_that_can_throw() {
        // A radix of 37 throws a RangeError; a local `Math` could be anything.
        for init in ["Math.random().toString(37)", "\"x\" + f().toString(36)"] {
            let source = format!("function $THREAD_1() {{\n\tlet $KEY_1 = {init};\n}}\n");
            assert_eq!(remove_unused_keys(&source).unwrap().1, 0, "{init}");
        }
        let source = "function $THREAD_1(Math) {\n\tlet $KEY_1 = Math.random().toString(36);\n}\n";
        assert_eq!(remove_unused_keys(source).unwrap().1, 0);
    }
}
