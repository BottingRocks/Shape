//! Move a generated one-use `$ARRAY_`/`$OBJECT_` literal to its only use.
//!
//! The tracer materializes literals as named temporaries even when they are
//! read exactly once. This pass deletes such a declaration and substitutes the
//! literal at the single resolved reference, so the value is built where it is
//! used. It is deliberately narrow:
//!
//! - the binding is a generated `$ARRAY_`/`$OBJECT_` `let`/`const` with one
//!   simple declarator and exactly one resolved reference;
//! - the reference is a read (not `delete`, a shorthand property, or a direct
//!   assignment), occurs after the declaration, and stays in the same function;
//! - the initializer is a movable literal: an array/object literal with no
//!   spreads, pure elements/values, and static or primitive-literal computed
//!   keys. Moving a pure allocation is unobservable because the value is read
//!   once, but a reference inside a loop the declaration is not in could build
//!   it more than once, so that case declines; so does one in a class body
//!   (a field initializer runs per instance, a static block per class).
//!
//! Comments in the declaration and `with`/`eval` decline. Literal source text
//! is copied verbatim, so escapes and `-0` survive.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;
use super::super::support::whole_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleUseLiteralResult {
    pub code: String,
    pub inlined_bindings: usize,
    pub replaced_reads: usize,
}

/// Inline a generated `$ARRAY_`/`$OBJECT_` literal at its only resolved read.
pub fn inline_single_use_literals(source: &str) -> Result<SingleUseLiteralResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan {
        functions: Vec::new(),
        loops: Vec::new(),
        declarations: Vec::new(),
        non_value_reads: Vec::new(),
        dynamic_scope: false,
    };
    scan.visit_program(&program);
    let mut edits = Vec::new();
    let mut bindings = 0;
    let mut reads = 0;
    if !scan.dynamic_scope {
        for declaration in &scan.declarations {
            let Some(function) = scan.enclosing_function(declaration.span) else {
                continue;
            };
            let references: Vec<_> = scoping
                .get_resolved_references(declaration.symbol)
                .collect();
            // Exactly one syntactic reference. A value read once is safe to
            // move; a value read twice could have its identity compared.
            let [reference] = references.as_slice() else {
                continue;
            };
            let span = nodes.get_node(reference.node_id()).kind().span();
            if reference.is_write()
                || span.start < declaration.span.end
                || scan.enclosing_function(span) != Some(function)
                || scan.non_value_reads.contains(&span)
                || scan.repeats_without(span, declaration.span)
            {
                continue;
            }
            let declaration_text =
                &source[declaration.span.start as usize..declaration.span.end as usize];
            if declaration_text.contains("//") || declaration_text.contains("/*") {
                continue;
            }
            let value = &source[declaration.value.start as usize..declaration.value.end as usize];
            edits.push(analysis::Edit::delete(whole_line(
                source,
                declaration.span,
            )));
            edits.push(analysis::Edit::replace(span, format!("({value})")));
            reads += 1;
            bindings += 1;
        }
    }
    if edits.is_empty() {
        return Ok(SingleUseLiteralResult {
            code: source.into(),
            inlined_bindings: 0,
            replaced_reads: 0,
        });
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() || applied.applied != bindings + reads {
        return Err("single-use literal edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("single-use literal output: {error}"))?;
    Ok(SingleUseLiteralResult {
        code: applied.output,
        inlined_bindings: bindings,
        replaced_reads: reads,
    })
}

struct Declaration {
    symbol: oxc_semantic::SymbolId,
    span: Span,
    value: Span,
}

struct Scan {
    functions: Vec<Span>,
    loops: Vec<Span>,
    declarations: Vec<Declaration>,
    non_value_reads: Vec<Span>,
    dynamic_scope: bool,
}

impl Scan {
    fn enclosing_function(&self, span: Span) -> Option<Span> {
        self.functions
            .iter()
            .copied()
            .filter(|function| function.start <= span.start && function.end >= span.end)
            .min_by_key(|function| function.end - function.start)
    }

    /// The reference sits in a loop the declaration is not in: the literal
    /// would be built once per iteration instead of once per declaration.
    fn repeats_without(&self, reference: Span, declaration: Span) -> bool {
        self.loops.iter().any(|span| {
            span.start <= reference.start
                && span.end >= reference.end
                && !(span.start <= declaration.start && span.end >= declaration.end)
        })
    }

    fn body(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            if !matches!(
                declaration.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            ) || declaration.declarations.len() != 1
            {
                continue;
            }
            let binding = &declaration.declarations[0];
            let (BindingPattern::BindingIdentifier(name), Some(value)) =
                (&binding.id, &binding.init)
            else {
                continue;
            };
            if !(name.name.starts_with("$ARRAY_") || name.name.starts_with("$OBJECT_"))
                || !movable_literal(value)
            {
                continue;
            }
            if let Some(symbol) = name.symbol_id.get() {
                self.declarations.push(Declaration {
                    symbol,
                    span: declaration.span,
                    value: value.span(),
                });
            }
        }
    }
}

impl<'a> Visit<'a> for Scan {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(function.span);
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push(arrow.span);
        self.body(&arrow.body.statements);
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }

    // A class body runs a field initializer once per instance: like a loop,
    // a literal moved into it is built more than once.
    fn visit_class_body(&mut self, body: &ClassBody<'a>) {
        self.loops.push(body.span);
        walk::walk_class_body(self, body);
    }

    fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_while_statement(self, statement);
    }

    fn visit_do_while_statement(&mut self, statement: &DoWhileStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_do_while_statement(self, statement);
    }

    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_for_statement(self, statement);
    }

    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_for_in_statement(self, statement);
    }

    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_for_of_statement(self, statement);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.shorthand {
            if let Expression::Identifier(identifier) = &property.value {
                self.non_value_reads.push(identifier.span);
            }
        }
        walk::walk_object_property(self, property);
    }

    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::Delete {
            if let Expression::Identifier(id) = &unary.argument {
                self.non_value_reads.push(id.span);
            }
        }
        walk::walk_unary_expression(self, unary);
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}

/// A literal whose allocation and evaluation can be moved to its single read:
/// no spread, no user code in an element/value, and no dynamic key.
fn movable_literal(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::ArrayExpression(array) => array.elements.iter().all(|element| match element {
            ArrayExpressionElement::Elision(_) => true,
            ArrayExpressionElement::SpreadElement(_) => false,
            _ => element
                .as_expression()
                .is_some_and(super::super::purity::expression_is_pure),
        }),
        Expression::ObjectExpression(object) => {
            object.properties.iter().all(|property| match property {
                ObjectPropertyKind::SpreadProperty(_) => false,
                ObjectPropertyKind::ObjectProperty(property) => {
                    let key_is_static = !property.computed
                        || matches!(
                            &property.key,
                            PropertyKey::StringLiteral(_) | PropertyKey::NumericLiteral(_)
                        );
                    key_is_static && super::super::purity::expression_is_pure(&property.value)
                }
            })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::inline_single_use_literals;

    #[test]
    fn keeps_a_literal_a_class_field_reads() {
        let source = "function f() { let $ARRAY_1_2 = []; class C { x = $ARRAY_1_2; } return new C().x === new C().x; }";
        let result = inline_single_use_literals(source).unwrap();
        assert_eq!(result.inlined_bindings, 0, "{}", result.code);
    }

    #[test]
    fn inlines_an_empty_array_at_its_only_read() {
        let source = "function f() { let $ARRAY_1_2 = []; return $ARRAY_1_2; }";
        let result = inline_single_use_literals(source).unwrap();
        assert_eq!(result.inlined_bindings, 1);
        assert_eq!(result.replaced_reads, 1);
        assert!(!result.code.contains("let $ARRAY_1_2"));
        assert!(result.code.contains("return ([]);"), "{}", result.code);
    }

    #[test]
    fn inlines_an_object_with_void_zero_fields() {
        let source =
            "function f() { let $OBJECT_1_2 = {[\"writable\"]: void 0}; return [$OBJECT_1_2]; }";
        let result = inline_single_use_literals(source).unwrap();
        assert_eq!(result.inlined_bindings, 1);
        assert!(
            result.code.contains("return [({[\"writable\"]: void 0})];"),
            "{}",
            result.code
        );
    }

    #[test]
    fn keeps_multiple_reads_writes_deletes_and_shorthand() {
        for source in [
            "function f() { let $ARRAY_1 = []; return [$ARRAY_1, $ARRAY_1]; }",
            "function f() { let $ARRAY_1 = []; $ARRAY_1 = 1; return $ARRAY_1; }",
            "function f() { let $ARRAY_1 = []; return delete $ARRAY_1; }",
            "function f() { let $ARRAY_1 = []; return { $ARRAY_1 }; }",
        ] {
            let result = inline_single_use_literals(source).unwrap();
            assert_eq!(result.inlined_bindings, 0, "{source}");
            assert_eq!(result.code, source);
        }
    }

    #[test]
    fn keeps_non_movable_initializers_and_captures() {
        for source in [
            "function f(x) { let $ARRAY_1 = [x]; return $ARRAY_1; }",
            "function f() { let $ARRAY_1 = [g()]; return $ARRAY_1; }",
            "function f() { let $ARRAY_1 = []; return () => $ARRAY_1; }",
            "function f() { let $OBJECT_1 = {...{}}; return $OBJECT_1; }",
            "function f() { let $ARRAY_1 = [1, ...[]]; return $ARRAY_1; }",
            "function f() { let $ARRAY_1 = []; eval('x'); return $ARRAY_1; }",
        ] {
            let result = inline_single_use_literals(source).unwrap();
            assert_eq!(result.inlined_bindings, 0, "{source}");
        }
    }

    #[test]
    fn keeps_a_read_inside_a_loop_the_declaration_is_not_in() {
        let source =
            "function f(n) { let $ARRAY_1 = []; for (let i = 0; i < n; i++) consume($ARRAY_1); }";
        let result = inline_single_use_literals(source).unwrap();
        assert_eq!(result.inlined_bindings, 0);
    }

    #[test]
    fn inlines_a_read_in_the_same_loop_and_through_a_conditional() {
        let same_loop = "function f(n) { for (let i = 0; i < n; i++) { let $ARRAY_1 = []; consume($ARRAY_1); } }";
        assert_eq!(
            inline_single_use_literals(same_loop)
                .unwrap()
                .inlined_bindings,
            1
        );
        let conditional = "function f(x) { let $ARRAY_1 = []; if (x) consume($ARRAY_1); }";
        assert_eq!(
            inline_single_use_literals(conditional)
                .unwrap()
                .inlined_bindings,
            1
        );
    }

    #[test]
    fn keeps_a_comment_in_the_declaration() {
        let source = "function f() { let $ARRAY_1 = [ /* keep */ ]; return $ARRAY_1; }";
        let result = inline_single_use_literals(source).unwrap();
        assert_eq!(result.inlined_bindings, 0);
    }
}
