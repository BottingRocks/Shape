//! Replace immutable generated SSA literals at their resolved read sites.
//!
//! A candidate is `let|const $SSA_n = <primitive literal>` where the literal is
//! numeric/string/boolean/null/bigint or unary `+`/`-` over a numeric literal.
//! The binding must have no writes, no read before its declaration completes,
//! no read from another enclosing function, shorthand-property read, or delete
//! of the binding (which observes a reference rather than its value).
//! Captures are retained because a closure may run before a `let` initializes;
//! comments in the declaration and `with`/`eval` decline. Literal source text
//! is copied at each read, so escapes and `-0` survive. This removes named
//! constants materialized by the tracer.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;
use super::super::support::whole_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralSsaResult {
    pub code: String,
    pub inlined_bindings: usize,
    pub replaced_reads: usize,
}

/// Inline a primitive literal assigned once to a generated `$SSA_` binding.
/// Reads must occur after initialization in the same function. Captures are
/// retained because the closure could run before its declaration initializes.
pub fn inline_literal_ssa(source: &str) -> Result<LiteralSsaResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan {
        this_safe: false,
        classes: Vec::new(),
        functions: Vec::new(),
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
            if references.is_empty()
                || references.iter().any(|reference| {
                    let span = nodes.get_node(reference.node_id()).kind().span();
                    reference.is_write()
                        || span.start < declaration.span.end
                        || scan.enclosing_function(span) != Some(function)
                        || scan.non_value_reads.contains(&span)
                        || (declaration.this_value
                            && scan.enclosing_class(span) != scan.enclosing_class(declaration.span))
                })
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
            for reference in references {
                let span = nodes.get_node(reference.node_id()).kind().span();
                let replacement = if declaration.bare_literal {
                    value.to_owned()
                } else {
                    format!("({value})")
                };
                edits.push(analysis::Edit::replace(span, replacement));
                reads += 1;
            }
            bindings += 1;
        }
    }
    if edits.is_empty() {
        return Ok(LiteralSsaResult {
            code: source.into(),
            inlined_bindings: 0,
            replaced_reads: 0,
        });
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() || applied.applied != bindings + reads {
        return Err("literal SSA edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("literal SSA output: {error}"))?;
    Ok(LiteralSsaResult {
        code: applied.output,
        inlined_bindings: bindings,
        replaced_reads: reads,
    })
}

struct Declaration {
    this_value: bool,
    symbol: oxc_semantic::SymbolId,
    span: Span,
    value: Span,
    bare_literal: bool,
}

struct Scan {
    classes: Vec<Span>,
    this_safe: bool,
    functions: Vec<Span>,
    declarations: Vec<Declaration>,
    non_value_reads: Vec<Span>,
    dynamic_scope: bool,
}

impl Scan {
    fn enclosing_class(&self, span: Span) -> Option<Span> {
        self.classes
            .iter()
            .copied()
            .filter(|class| class.start <= span.start && class.end >= span.end)
            .min_by_key(|class| class.end - class.start)
    }
    fn enclosing_function(&self, span: Span) -> Option<Span> {
        self.functions
            .iter()
            .copied()
            .filter(|function| function.start <= span.start && function.end >= span.end)
            .min_by_key(|function| function.end - function.start)
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
            if !(name.name.starts_with("$SSA_") || name.name.starts_with("$CARRY_"))
                || !(primitive_literal(value)
                    || (self.this_safe && matches!(value, Expression::ThisExpression(_))))
            {
                continue;
            }
            if let Some(symbol) = name.symbol_id.get() {
                self.declarations.push(Declaration {
                    this_value: matches!(value, Expression::ThisExpression(_)),
                    symbol,
                    span: declaration.span,
                    value: value.span(),
                    bare_literal: matches!(
                        value,
                        Expression::StringLiteral(_)
                            | Expression::BooleanLiteral(_)
                            | Expression::NullLiteral(_)
                    ),
                });
            }
        }
    }
}

impl<'a> Visit<'a> for Scan {
    fn visit_class(&mut self, class: &Class<'a>) {
        self.classes.push(class.span);
        walk::walk_class(self, class);
    }
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let previous = self.this_safe;
        // An ordinary declaration cannot have a derived-constructor `this`
        // TDZ. Method/constructor and lexical-arrow contexts stay conservative.
        self.this_safe = function.r#type == FunctionType::FunctionDeclaration;
        self.functions.push(function.span);
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
        self.this_safe = previous;
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        let previous = self.this_safe;
        self.this_safe = false;
        self.functions.push(arrow.span);
        self.body(&arrow.body.statements);
        walk::walk_arrow_function_expression(self, arrow);
        self.this_safe = previous;
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
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

fn primitive_literal(value: &Expression<'_>) -> bool {
    match value {
        Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            (unary.operator == UnaryOperator::Void
                && super::super::purity::expression_is_pure(&unary.argument))
                || matches!(
                    unary.operator,
                    oxc_ast::ast::UnaryOperator::UnaryNegation
                        | oxc_ast::ast::UnaryOperator::UnaryPlus
                ) && matches!(&unary.argument, Expression::NumericLiteral(_))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::inline_literal_ssa;

    #[test]
    fn keeps_delete_of_a_binding() {
        let result =
            inline_literal_ssa("function f() { let $SSA_1 = 3; return delete $SSA_1; }").unwrap();
        assert_eq!(result.inlined_bindings, 0);
    }

    #[test]
    fn replaces_multiple_resolved_reads_and_removes_declaration() {
        let source = "function f(x) { let $SSA_1 = 72; if (x) return $SSA_1; return $SSA_1 + 1; }";
        let result = inline_literal_ssa(source).unwrap();
        assert_eq!(result.inlined_bindings, 1);
        assert_eq!(result.replaced_reads, 2);
        assert!(!result.code.contains("let $SSA_1"));
        assert!(result.code.contains("return (72);"));
        assert!(result.code.contains("return (72) + 1;"));
    }

    #[test]
    fn inlines_plain_string_boolean_and_null_without_wrappers() {
        let source = "function f(input) { let $SSA_1 = \"architecture\"; let $SSA_2 = true; let $SSA_3 = null; return [input, $SSA_1, $SSA_2, $SSA_3]; }";
        let result = inline_literal_ssa(source).unwrap();
        assert!(
            result
                .code
                .contains("return [input, \"architecture\", true, null]"),
            "{}",
            result.code
        );
    }

    #[test]
    fn keeps_writes_early_reads_captures_and_shorthand() {
        for source in [
            "function f() { let $SSA_1 = 2; $SSA_1 = 3; return $SSA_1; }",
            "function f() { return $SSA_1; let $SSA_1 = 2; }",
            "function f() { let $SSA_1 = 2; return () => $SSA_1; }",
            "function f() { let $SSA_1 = 2; return { $SSA_1 }; }",
            "function f() { for (let $SSA_1 = 2; $SSA_1 < 3;) return $SSA_1; }",
            "function f() { let $SSA_1 = /a/; return $SSA_1; }",
        ] {
            let result = inline_literal_ssa(source).unwrap();
            assert_eq!(result.inlined_bindings, 0, "{source}");
            assert_eq!(result.code, source);
        }
    }

    #[test]
    fn keeps_shadowed_binding_and_dynamic_scope_safe() {
        let source = "function f() { let $SSA_1 = 4; { let $SSA_1 = f(); consume($SSA_1); } return $SSA_1; }";
        let result = inline_literal_ssa(source).unwrap();
        assert_eq!(result.inlined_bindings, 1);
        assert!(result.code.contains("consume($SSA_1)"));
        assert!(result.code.contains("return (4)"));
        let eval_source = "function f() { let $SSA_1 = 4; eval('x'); return $SSA_1; }";
        assert_eq!(inline_literal_ssa(eval_source).unwrap().code, eval_source);
    }
}
