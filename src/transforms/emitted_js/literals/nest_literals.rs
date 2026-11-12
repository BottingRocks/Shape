//! Build a fresh literal in one piece: fold its fills into it, then move it
//! to its only use.
//!
//! Expanded helpers and the VM's table builders leave
//!
//! ```js
//! let a = [0];
//! let b = [["POST"]];
//! a[1] = 5;
//! a[2] = 0;
//! b[1] = a;
//! t[3] = b;
//! ```
//!
//! which is `t[3] = [["POST"], [0, 5, 0]];`. Two rewrites, repeated in a
//! statement list until neither applies:
//!
//! - **Fill.** `x[n] = v;` or `x["k"] = v;` after `let x = <literal>;`,
//!   with a pure `v` and only statements that never mention `x` in between,
//!   becomes part of the literal (the next array element, a replaced element,
//!   a new key at the end, or a replaced key's value). Until `x` is mentioned
//!   no code can reach the fresh object, and a pure `v` (no names) is the
//!   same value whenever it is built. `__proto__` keys, holes past the end,
//!   spreads and accessors decline.
//! - **Nest.** `let x = <pure literal>;` read exactly once, later in the
//!   same list and not inside a nested function or loop, is replaced by the
//!   literal at that read: building a pure literal later runs nothing, and
//!   the one read is the only place its identity is seen.
//!
//! Only generated (`$`-prefixed) names are touched. Names are counted
//! across the whole program, so a second binding of the same name only makes
//! the pass decline.

use std::collections::HashMap;

use oxc_allocator::{Allocator, CloneIn};
use oxc_ast::ast::*;
use oxc_ast::AstBuilder;
use oxc_ast_visit::{walk_mut, Visit, VisitMut};
use oxc_codegen::{Codegen, CodegenOptions, IndentChar};
use oxc_span::SPAN;
use oxc_syntax::scope::ScopeFlags;

use super::super::purity::expression_is_pure;
use crate::analysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NestedLiterals {
    pub code: String,
    pub folded: usize,
}

pub(crate) fn nest_literals(source: &str) -> Result<NestedLiterals, String> {
    let mut code = source.to_string();
    let mut folded = 0;
    for _ in 0..16 {
        let allocator = Allocator::default();
        let mut program = analysis::parse_program(&allocator, &code)?;
        let mut counts = Counts::default();
        counts.visit_program(&program);
        let mut nest = Nest {
            builder: AstBuilder::new(&allocator),
            counts: counts.0,
            folded: 0,
        };
        nest.visit_program(&mut program);
        if nest.folded == 0 {
            break;
        }
        folded += nest.folded;
        code = Codegen::new()
            .with_options(CodegenOptions {
                indent_char: IndentChar::Tab,
                indent_width: 1,
                ..CodegenOptions::default()
            })
            .build(&program)
            .code;
    }
    Ok(NestedLiterals { code, folded })
}

/// References per name.
#[derive(Default)]
struct Counts(HashMap<String, usize>);

impl<'a> Visit<'a> for Counts {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        *self.0.entry(identifier.name.to_string()).or_insert(0) += 1;
    }
}

/// `statement` mentions `name` anywhere.
fn mentions(statement: &Statement<'_>, name: &str) -> bool {
    struct Find<'n>(&'n str, bool);
    impl<'a> Visit<'a> for Find<'_> {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            if identifier.name == self.0 {
                self.1 = true;
            }
        }
    }
    let mut find = Find(name, false);
    find.visit_statement(statement);
    find.1
}

/// `let x = <array or object literal>;` with a generated name.
fn literal_declaration<'s, 'a>(statement: &'s Statement<'a>) -> Option<(&'s str, &'s Expression<'a>)> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    if declaration.kind == VariableDeclarationKind::Var || declaration.declarations.len() != 1 {
        return None;
    }
    let declarator = &declaration.declarations[0];
    let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
        return None;
    };
    let init = declarator.init.as_ref()?;
    (binding.name.starts_with('$')
        && matches!(init, Expression::ArrayExpression(_) | Expression::ObjectExpression(_)))
    .then_some((binding.name.as_str(), init))
}

/// The key of a fill `x[k] = v;` on `name`: an array index or a string key.
enum Key {
    Index(usize),
    Name(String),
}

fn fill<'s, 'a>(statement: &'s Statement<'a>, name: &str) -> Option<(Key, &'s Expression<'a>)> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &expression.expression else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign || !expression_is_pure(&assignment.right) {
        return None;
    }
    let (object, key) = match &assignment.left {
        AssignmentTarget::ComputedMemberExpression(member) => {
            let key = match &member.expression {
                Expression::NumericLiteral(number)
                    if number.value >= 0.0 && number.value.fract() == 0.0 && number.value < 1e6 =>
                {
                    Key::Index(number.value as usize)
                }
                Expression::StringLiteral(string) => Key::Name(string.value.to_string()),
                _ => return None,
            };
            (&member.object, key)
        }
        AssignmentTarget::StaticMemberExpression(member) => (&member.object, Key::Name(member.property.name.to_string())),
        _ => return None,
    };
    if !matches!(object, Expression::Identifier(object) if object.name == name) {
        return None;
    }
    if matches!(&key, Key::Name(key) if key == "__proto__") {
        return None;
    }
    Some((key, &assignment.right))
}

fn property_key(property: &ObjectProperty<'_>) -> Option<String> {
    match &property.key {
        PropertyKey::StaticIdentifier(identifier) if !property.computed => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(string) => Some(string.value.to_string()),
        _ => None,
    }
}

struct Nest<'a> {
    builder: AstBuilder<'a>,
    counts: HashMap<String, usize>,
    folded: usize,
}

impl<'a> Nest<'a> {
    /// Folds one fill into the literal `init`; false when it cannot.
    fn fold(&self, init: &mut Expression<'a>, key: Key, value: Expression<'a>) -> bool {
        match (init, key) {
            (Expression::ArrayExpression(array), Key::Index(index)) => {
                if array.elements.iter().any(|element| matches!(element, ArrayExpressionElement::SpreadElement(_))) {
                    return false;
                }
                let element = ArrayExpressionElement::from(value);
                if index == array.elements.len() {
                    array.elements.push(element);
                } else if index < array.elements.len() {
                    // The old element still has to run.
                    if array.elements[index].as_expression().is_some_and(|old| !expression_is_pure(old)) {
                        return false;
                    }
                    array.elements[index] = element;
                } else {
                    return false;
                }
                true
            }
            (Expression::ObjectExpression(object), Key::Name(key)) => {
                let mut found = None;
                for (at, property) in object.properties.iter().enumerate() {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return false;
                    };
                    if property.kind != PropertyKind::Init || property.method {
                        return false;
                    }
                    match property_key(property) {
                        Some(name) if name == key => found = Some(at),
                        Some(_) => {}
                        None => return false,
                    }
                }
                match found {
                    Some(at) => {
                        let ObjectPropertyKind::ObjectProperty(property) = &mut object.properties[at] else {
                            unreachable!();
                        };
                        if !expression_is_pure(&property.value) {
                            return false;
                        }
                        property.value = value;
                    }
                    None => {
                        let key = PropertyKey::StringLiteral(self.builder.alloc_string_literal(
                            SPAN,
                            self.builder.atom(&key),
                            None,
                        ));
                        let property =
                            self.builder.object_property_kind_object_property(SPAN, PropertyKind::Init, key, value, false, false, true);
                        object.properties.push(property);
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn list(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>) {
        let mut index = 0;
        while index < statements.len() {
            let Some((name, _)) = literal_declaration(&statements[index]) else {
                index += 1;
                continue;
            };
            let name = name.to_string();
            // Fill: fold the stores that follow, across statements that
            // never mention the name.
            let mut at = index + 1;
            while at < statements.len() {
                if let Some((key, value)) = fill(&statements[at], &name) {
                    let value = value.clone_in(self.builder.allocator);
                    let Statement::VariableDeclaration(declaration) = &mut statements[index] else {
                        unreachable!();
                    };
                    let init = declaration.declarations[0].init.as_mut().unwrap();
                    if !self.fold(init, key, value) {
                        break;
                    }
                    statements.remove(at);
                    if let Some(count) = self.counts.get_mut(&name) {
                        *count -= 1;
                    }
                    self.folded += 1;
                    continue;
                }
                if mentions(&statements[at], &name) {
                    break;
                }
                at += 1;
            }
            // Nest: a pure literal read once moves to that read.
            let pure = literal_declaration(&statements[index]).is_some_and(|(_, init)| expression_is_pure(init));
            if pure && self.counts.get(&name) == Some(&1) {
                if let Some(user) = (index + 1..statements.len()).find(|&at| mentions(&statements[at], &name)) {
                    let Statement::VariableDeclaration(declaration) = &statements[index] else {
                        unreachable!();
                    };
                    let init = declaration.declarations[0].init.as_ref().unwrap().clone_in(self.builder.allocator);
                    let mut place = Place {
                        name: &name,
                        value: Some(init),
                        depth: 0,
                    };
                    place.visit_statement(&mut statements[user]);
                    if place.value.is_none() {
                        statements.remove(index);
                        self.counts.remove(&name);
                        self.folded += 1;
                        continue;
                    }
                }
            }
            self.project(statements, index, &name);
            index += 1;
        }
    }

    /// `let r = [a, Math, Math["sin"]];` only read as `r[1]`, `r[2]`: a read
    /// of a fixed element (a literal, a standard global, or a named property
    /// of one) is that element.
    fn project(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>, index: usize, name: &str) {
        let Some((_, Expression::ArrayExpression(array))) = literal_declaration(&statements[index]) else {
            return;
        };
        let fixed: Vec<Option<Expression<'a>>> = array
            .elements
            .iter()
            .map(|element| element.as_expression().filter(|e| fixed(e)).map(|e| e.clone_in(self.builder.allocator)))
            .collect();
        if fixed.iter().all(Option::is_none) {
            return;
        }
        let mut scan = Reads { name, length: fixed.len(), reads: 0, other: false };
        for statement in statements.iter().skip(index + 1) {
            scan.visit_statement(statement);
        }
        if scan.other || Some(&scan.reads) != self.counts.get(name) {
            return;
        }
        let mut put = Project { allocator: self.builder.allocator, name, fixed: &fixed, replaced: 0 };
        for statement in statements.iter_mut().skip(index + 1) {
            put.visit_statement(statement);
        }
        if put.replaced > 0 {
            if let Some(count) = self.counts.get_mut(name) {
                *count -= put.replaced;
            }
            self.folded += put.replaced;
        }
    }
}

/// A value that reads the same anywhere: a primitive literal, a standard
/// global, or a named property of one.
fn fixed(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::NumericLiteral(_) | Expression::StringLiteral(_) | Expression::BooleanLiteral(_) | Expression::NullLiteral(_) => true,
        Expression::Identifier(identifier) => {
            super::super::primitives::is_stable_global(&identifier.name) && !matches!(identifier.name.as_str(), "RegExp" | "Error")
        }
        Expression::StaticMemberExpression(member) => !member.optional && fixed(&member.object) && !matches!(member.object, Expression::NumericLiteral(_) | Expression::StringLiteral(_) | Expression::BooleanLiteral(_) | Expression::NullLiteral(_)),
        Expression::ComputedMemberExpression(member) => {
            !member.optional
                && matches!(member.expression, Expression::StringLiteral(_))
                && matches!(member.object, Expression::Identifier(_) | Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_))
                && fixed(&member.object)
        }
        _ => false,
    }
}

/// Counts `name[n]` reads; any other mention of `name` sets `other`.
struct Reads<'n> {
    name: &'n str,
    length: usize,
    reads: usize,
    other: bool,
}

fn constant_index(member: &ComputedMemberExpression<'_>, name: &str, length: usize) -> Option<usize> {
    if member.optional || !matches!(&member.object, Expression::Identifier(object) if object.name == name) {
        return None;
    }
    match &member.expression {
        Expression::NumericLiteral(number) if number.value.fract() == 0.0 && number.value >= 0.0 && (number.value as usize) < length => {
            Some(number.value as usize)
        }
        _ => None,
    }
}

impl<'a> Visit<'a> for Reads<'_> {
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if constant_index(member, self.name, self.length).is_some() {
            self.reads += 1;
            return;
        }
        oxc_ast_visit::walk::walk_computed_member_expression(self, member);
    }
    fn visit_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'a>) {
        // A store through the tuple changes an element.
        if let SimpleAssignmentTarget::ComputedMemberExpression(member) = target {
            if matches!(&member.object, Expression::Identifier(object) if object.name == self.name) {
                self.other = true;
            }
        }
        oxc_ast_visit::walk::walk_simple_assignment_target(self, target);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == self.name {
            self.other = true;
        }
    }
}

struct Project<'n, 'f, 'a> {
    allocator: &'a Allocator,
    name: &'n str,
    fixed: &'f [Option<Expression<'a>>],
    replaced: usize,
}

impl<'a> VisitMut<'a> for Project<'_, '_, 'a> {
    fn visit_expression(&mut self, expression: &mut Expression<'a>) {
        if let Expression::ComputedMemberExpression(member) = expression {
            if let Some(at) = constant_index(member, self.name, self.fixed.len()) {
                if let Some(value) = &self.fixed[at] {
                    *expression = value.clone_in(self.allocator);
                    self.replaced += 1;
                    return;
                }
            }
        }
        walk_mut::walk_expression(self, expression);
    }
}

/// Puts `value` at the one read of `name`, unless that read is inside a
/// nested function or loop, or is the object of a member expression.
struct Place<'n, 'a> {
    name: &'n str,
    value: Option<Expression<'a>>,
    depth: usize,
}

impl<'a> VisitMut<'a> for Place<'_, 'a> {
    fn visit_expression(&mut self, expression: &mut Expression<'a>) {
        if self.depth == 0 && matches!(expression, Expression::Identifier(identifier) if identifier.name == self.name) {
            if let Some(value) = self.value.take() {
                *expression = value;
            }
            return;
        }
        walk_mut::walk_expression(self, expression);
    }
    fn visit_static_member_expression(&mut self, member: &mut StaticMemberExpression<'a>) {
        if !matches!(&member.object, Expression::Identifier(identifier) if identifier.name == self.name) {
            walk_mut::walk_static_member_expression(self, member);
        }
    }
    fn visit_computed_member_expression(&mut self, member: &mut ComputedMemberExpression<'a>) {
        if matches!(&member.object, Expression::Identifier(identifier) if identifier.name == self.name) {
            self.visit_expression(&mut member.expression);
        } else {
            walk_mut::walk_computed_member_expression(self, member);
        }
    }
    fn visit_function(&mut self, function: &mut Function<'a>, flags: ScopeFlags) {
        self.depth += 1;
        walk_mut::walk_function(self, function, flags);
        self.depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, arrow: &mut ArrowFunctionExpression<'a>) {
        self.depth += 1;
        walk_mut::walk_arrow_function_expression(self, arrow);
        self.depth -= 1;
    }
    fn visit_for_statement(&mut self, statement: &mut ForStatement<'a>) {
        self.depth += 1;
        walk_mut::walk_for_statement(self, statement);
        self.depth -= 1;
    }
    fn visit_for_in_statement(&mut self, statement: &mut ForInStatement<'a>) {
        self.depth += 1;
        walk_mut::walk_for_in_statement(self, statement);
        self.depth -= 1;
    }
    fn visit_for_of_statement(&mut self, statement: &mut ForOfStatement<'a>) {
        self.depth += 1;
        walk_mut::walk_for_of_statement(self, statement);
        self.depth -= 1;
    }
    fn visit_while_statement(&mut self, statement: &mut WhileStatement<'a>) {
        self.depth += 1;
        walk_mut::walk_while_statement(self, statement);
        self.depth -= 1;
    }
    fn visit_do_while_statement(&mut self, statement: &mut DoWhileStatement<'a>) {
        self.depth += 1;
        walk_mut::walk_do_while_statement(self, statement);
        self.depth -= 1;
    }
}

impl<'a> VisitMut<'a> for Nest<'a> {
    fn visit_statements(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>) {
        walk_mut::walk_statements(self, statements);
        self.list(statements);
    }
}

#[cfg(test)]
mod tests {
    use super::nest_literals;

    #[test]
    fn nests_filled_literals_into_their_one_use() {
        let source = "function f(t, g) {\n\tlet $a = [0];\n\tlet $b = [[\"POST\"]];\n\t$a[1] = 5;\n\tg();\n\t$a[2] = 0;\n\t$b[1] = $a;\n\tt[3] = $b;\n\tlet $o = {};\n\t$o[\"k\"] = 1;\n\t$o.k = 2;\n\treturn $o;\n}\n";
        let code = nest_literals(source).unwrap().code;
        assert!(code.contains("g();\n\tt[3] = [[\"POST\"], [\n\t\t0,\n\t\t5,\n\t\t0\n\t]];") || code.contains("t[3] = [[\"POST\"], [0, 5, 0]];"), "{code}");
        assert!(code.contains("[\"k\"]: 2"), "{code}");
        assert!(!code.contains("$a") && !code.contains("$b") && !code.contains("$o"), "{code}");
        let source = "function f(t, x) {\n\tlet $r = [x + 1, Math, Math[\"sin\"]];\n\tt($r[0], $r[2].call($r[1], 0));\n}\n";
        let code = nest_literals(source).unwrap().code;
        assert!(code.contains("t($r[0], Math[\"sin\"].call(Math, 0));"), "{code}");
    }

    #[test]
    fn keeps_overwritten_values_that_run_code() {
        for source in [
            "function f(g) {\n\tlet $a = [g()];\n\t$a[0] = 1;\n\treturn $a;\n}\n",
            "function f(g) {\n\tlet $a = { k: g() };\n\t$a.k = 1;\n\treturn $a;\n}\n",
        ] {
            assert!(nest_literals(source).unwrap().code.contains("g()"), "{source}");
        }
    }

    #[test]
    fn keeps_literals_read_in_loops_or_twice() {
        let source = "function f(t, g) {\n\tlet $a = [0];\n\tfor (;;) {\n\t\tt.push($a);\n\t}\n\tlet $b = [1];\n\tt[0] = $b;\n\tt[1] = $b;\n\tlet $c = [g()];\n\tt[2] = $c;\n}\n";
        let code = nest_literals(source).unwrap().code;
        assert!(code.contains("let $a = [0];") && code.contains("let $b = [1];") && code.contains("let $c = [g()];"), "{code}");
    }
}
