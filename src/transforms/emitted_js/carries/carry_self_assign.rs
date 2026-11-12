//! Remove discarded generated self-stores and initialized local reads.
//! Comma expressions retain their Value semantics even after losing prefixes;
//! const errors, TDZ, unknown getters, coercions and effects remain observable.

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::Scoping;
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;

use super::super::adjacent_ssa::LocalReads;
use crate::analysis;
use std::collections::BTreeSet;
use crate::transforms::emitted_js::support::unwrap;

pub(crate) fn remove_carry_self_assignments(source: &str) -> Result<(String, usize), String> {
    if !source.contains("$SSA_") && !source.contains("$CARRY_") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    if nodes.iter().any(|n| {
        matches!(n.kind(), AstKind::WithStatement(_))
            || matches!(n.kind(),AstKind::IdentifierReference(id) if id.name=="eval")
    }) {
        return Ok((source.into(), 0));
    }
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, &BTreeSet::new());
    // Concise arrows store their returned expression in an ExpressionStatement
    // AST node. Its value is used, unlike an actual standalone statement.
    let value_statements = nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::ArrowFunctionExpression(a) if a.expression => {
                a.body.statements.first().map(GetSpan::span)
            }
            _ => None,
        })
        .collect();
    let mutable_declarations = nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::VariableDeclarator(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var | VariableDeclarationKind::Let
                ) =>
            {
                if let BindingPattern::BindingIdentifier(id) = &d.id {
                    id.symbol_id.get()
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();
    let mut scan = Scan {
        source,
        scoping: &scoping,
        locals: &locals,
        value_statements,
        mutable_declarations,
        edits: vec![],
    };
    scan.visit_program(&program);
    let count = scan.edits.len();
    if count == 0 {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("carry self-assignment edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("carry self-assignment output: {error}"))?;
    Ok((applied.output, count))
}

fn generated(name: &str) -> bool {
    name.starts_with("$SSA_") || name.starts_with("$CARRY_")
}
fn contains_cell(e: &Expression<'_>) -> bool {
    match unwrap(e) {
        Expression::Identifier(id) => generated(id.name.as_str()),
        Expression::AssignmentExpression(a) => {
            matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id) if generated(id.name.as_str()))
        }
        Expression::SequenceExpression(s) => s.expressions.iter().any(contains_cell),
        Expression::UnaryExpression(u) => contains_cell(&u.argument),
        Expression::LogicalExpression(l) => contains_cell(&l.left) || contains_cell(&l.right),
        Expression::ConditionalExpression(c) => {
            contains_cell(&c.test) || contains_cell(&c.consequent) || contains_cell(&c.alternate)
        }
        _ => false,
    }
}
struct Scan<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    locals: &'s LocalReads,
    value_statements: BTreeSet<Span>,
    mutable_declarations: BTreeSet<oxc_semantic::SymbolId>,
    edits: Vec<analysis::Edit>,
}
impl Scan<'_> {
    fn sequence(&mut self, s: &SequenceExpression<'_>, used: bool) -> bool {
        if self.comments(s.span) || s.expressions.len() < 2 {
            return false;
        }
        let kept: Vec<_> = s
            .expressions
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                (used && i + 1 == s.expressions.len() || !self.discarded(e)).then_some(e)
            })
            .collect();
        if kept.len() == s.expressions.len() {
            return false;
        }
        // A lone explicit zero prefix already carries the required Value
        // semantics. Do not replace it with itself on every fixed-point round.
        if used
            && kept.len() == 1
            && s.expressions.len() == 2
            && matches!(unwrap(&s.expressions[0]),Expression::NumericLiteral(n) if n.value==0.0)
        {
            return false;
        }
        let mut parts: Vec<_> = kept
            .iter()
            .map(|e| &self.source[e.span().start as usize..e.span().end as usize])
            .collect();
        // A used comma result is a Value, even for delete/typeof or an
        // indirect call/tag. Preserve that category when only one part remains.
        if used && parts.len() == 1 {
            parts.insert(0, "0");
        }
        self.edits.push(analysis::Edit::replace(
            s.span,
            if parts.is_empty() {
                "0".into()
            } else {
                format!("({})", parts.join(", "))
            },
        ));
        true
    }
    fn comments(&self, span: Span) -> bool {
        let text = &self.source[span.start as usize..span.end as usize];
        text.contains("/*") || text.contains("//")
    }
    fn discarded(&self, e: &Expression<'_>) -> bool {
        if super::super::purity::expression_is_pure(e) {
            return true;
        }
        match unwrap(e) {
            Expression::Identifier(id) if generated(id.name.as_str()) => {
                analysis::identifier_symbol(id, self.scoping)
                    .is_some_and(|s| self.locals.plain_initialized(s, e.span()))
            }
            Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
                let (
                    AssignmentTarget::AssignmentTargetIdentifier(target),
                    Expression::Identifier(value),
                ) = (&a.left, unwrap(&a.right))
                else {
                    return false;
                };
                generated(target.name.as_str())
                    && analysis::identifier_symbol(target, self.scoping).is_some_and(|s| {
                        analysis::identifier_symbol(value, self.scoping) == Some(s)
                            && self.mutable_declarations.contains(&s)
                            && !self.scoping.symbol_flags(s).is_const_variable()
                            && self.locals.initialized(s, e.span())
                    })
            }
            Expression::SequenceExpression(s) => s.expressions.iter().all(|e| self.discarded(e)),
            Expression::UnaryExpression(u)
                if matches!(
                    u.operator,
                    UnaryOperator::LogicalNot | UnaryOperator::Void | UnaryOperator::Typeof
                ) =>
            {
                self.discarded(&u.argument)
            }
            Expression::LogicalExpression(l) => self.discarded(&l.left) && self.discarded(&l.right),
            Expression::ConditionalExpression(c) => {
                self.discarded(&c.test)
                    && self.discarded(&c.consequent)
                    && self.discarded(&c.alternate)
            }
            _ => false,
        }
    }
}
impl<'a> Visit<'a> for Scan<'_> {
    fn visit_expression_statement(&mut self, s: &ExpressionStatement<'a>) {
        if !self.value_statements.contains(&s.span)
            && contains_cell(&s.expression)
            && !self.comments(s.span)
        {
            if self.discarded(&s.expression) {
                // Preserve a barrier before following non-directive strings.
                self.edits.push(analysis::Edit::replace(s.span, ";"));
                return;
            }
            if let Expression::SequenceExpression(sequence) = unwrap(&s.expression) {
                if self.sequence(sequence, false) {
                    return;
                }
            }
        }
        walk::walk_expression_statement(self, s);
    }
    fn visit_sequence_expression(&mut self, s: &SequenceExpression<'a>) {
        if self.sequence(s, true) {
            return;
        }
        walk::walk_sequence_expression(self, s);
    }
    fn visit_for_statement(&mut self, s: &ForStatement<'a>) {
        if let Some(init) = &s.init {
            if let Some(e) = init
                .as_expression()
                .filter(|e| contains_cell(e) && !self.comments(e.span()) && self.discarded(e))
            {
                self.edits.push(analysis::Edit::replace(e.span(), "0"));
            } else if let Some(Expression::SequenceExpression(sequence)) =
                init.as_expression().map(unwrap)
            {
                if !self.sequence(sequence, false) {
                    self.visit_for_statement_init(init);
                }
            } else {
                self.visit_for_statement_init(init);
            }
        }
        if let Some(test) = &s.test {
            self.visit_expression(test);
        }
        if let Some(update) = &s.update {
            if contains_cell(update) && !self.comments(update.span()) && self.discarded(update) {
                self.edits.push(analysis::Edit::replace(update.span(), "0"));
            } else if let Expression::SequenceExpression(sequence) = unwrap(update) {
                if !self.sequence(sequence, false) {
                    self.visit_expression(update);
                }
            } else {
                self.visit_expression(update);
            }
        }
        self.visit_statement(&s.body);
    }
}

#[cfg(test)]
mod tests {
    use super::remove_carry_self_assignments;

    #[test]
    fn removes_discarded_self_values_in_ten_expression_contexts() {
        for tail in [
            "$SSA_1=$SSA_1;return 1;",
            "($SSA_1=$SSA_1,run());return 1;",
            "return ($SSA_1=$SSA_1,run());",
            "if(($SSA_1=$SSA_1,flag))run();",
            "while(($SSA_1=$SSA_1,flag--))run();",
            "for(($SSA_1=$SSA_1);flag;$SSA_1=$SSA_1){run();}",
            "throw ($SSA_1=$SSA_1,run());",
            "return delete ($SSA_1=$SSA_1,object.x);",
            "return typeof ($SSA_1=$SSA_1,missing);",
            "let g=()=> (1,$SSA_1);return g();",
            "void ($SSA_1=$SSA_1);return 1;",
            "return run(($SSA_1=$SSA_1,$SSA_1));",
        ] {
            let source = format!("function f(flag){{let $SSA_1=read();{tail}}}");
            let (code, count) = remove_carry_self_assignments(&source).unwrap();
            assert!(count > 0, "{source}\n{code}");
            let allocator = oxc_allocator::Allocator::default();
            crate::analysis::parse_program(&allocator, &code).unwrap();
            assert_eq!(remove_carry_self_assignments(&code).unwrap().1, 0, "{code}");
        }
    }

    #[test]
    fn unused_self_values_keep_errors_unknown_reads_and_effectful_prefixes() {
        for source in [
            "function f(){const $SSA_1=1;return ($SSA_1=$SSA_1,run());}",
            "function f(){return ($SSA_1=$SSA_1,run());let $SSA_1=1;}",
            "function f(){g();let $SSA_1=1;function g(){return ($SSA_1=$SSA_1,run());}}",
            "function f(){return ($SSA_1=$SSA_1,run());}",
            "function f(){let $SSA_1=read();return ($SSA_1.x=$SSA_1.x,run());}",
            "function f(){let $SSA_1=read();return ($SSA_1++,run());}",
            "function f(){let $SSA_1=read();return ($SSA_1+1,run());}",
            "function f(){let $SSA_1=read();return ($SSA_1(),run());}",
            "function f(){let $SSA_1=read();return ($SSA_1=$SSA_1 /*keep*/,run());}",
            "function f(){let $SSA_1=read();return (typeof missing,run($SSA_1));}",
            "function f(){let $SSA_1=1;eval('');return ($SSA_1=$SSA_1,run());}",
            "function f(){let $SSA_1=1;return $SSA_1=$SSA_1;}",
            "let f=function $SSA_1(){'use strict';return ($SSA_1=$SSA_1,3);};",
            "function f(){using $SSA_1=resource;return ($SSA_1=$SSA_1,3);}",
        ] {
            assert_eq!(
                remove_carry_self_assignments(source).unwrap().1,
                0,
                "{source}"
            );
        }
    }

    #[test]
    fn comma_values_keep_a_zero_prefix_and_concise_arrow_results() {
        for tail in [
            "return delete ($SSA_1=$SSA_1,object.x);",
            "return typeof ($SSA_1=$SSA_1,missing);",
            "return ($SSA_1=$SSA_1,object.method)();",
            "return ($SSA_1=$SSA_1,object.tag)`text`;",
            "return ($SSA_1=$SSA_1,object.method)?.();",
        ] {
            let source = format!("function f(){{let $SSA_1=read();{tail}}}");
            let (code, count) = remove_carry_self_assignments(&source).unwrap();
            assert_eq!(count, 1, "{code}");
            assert!(code.contains("0, "), "{code}");
        }
        let source = "function f(){let $SSA_1=read();let g=()=> (1,$SSA_1);return g();}";
        let (code, count) = remove_carry_self_assignments(source).unwrap();
        assert_eq!(count, 1);
        assert!(code.contains("=> ((0, $SSA_1))"), "{code}");
    }

    #[test]
    fn removes_local_self_store_in_branch() {
        let source =
            "function f(p) { let $CARRY_1 = 1; if (p) { $CARRY_1 = $CARRY_1; } return $CARRY_1; }";
        let (code, count) = remove_carry_self_assignments(source).unwrap();
        assert_eq!(count, 1);
        assert!(!code.contains("$CARRY_1 = $CARRY_1"));
    }

    #[test]
    fn retains_unresolved_tdz_and_dynamic_scope() {
        let unresolved = "$CARRY_1 = $CARRY_1;";
        assert_eq!(remove_carry_self_assignments(unresolved).unwrap().1, 0);
        let tdz = "{ $CARRY_1 = $CARRY_1; let $CARRY_1 = 1; }";
        assert_eq!(remove_carry_self_assignments(tdz).unwrap().1, 0);
        let dynamic = "function f() { let $CARRY_1 = 1; eval(''); $CARRY_1 = $CARRY_1; }";
        assert_eq!(remove_carry_self_assignments(dynamic).unwrap().1, 0);
    }

    #[test]
    fn keeps_const_errors_early_closures_and_directive_boundaries() {
        for source in [
            "function f(){const $CARRY_1=1;$CARRY_1=$CARRY_1;}",
            "function f(){g();let $CARRY_1=1;function g(){$CARRY_1=$CARRY_1;}}",
            "function f(){let $CARRY_1=g();function g(){$CARRY_1=$CARRY_1;}}",
            "let $CARRY_1=g();function g(){$CARRY_1=$CARRY_1;}",
            "function f(){let $CARRY_1=1;return function(){$CARRY_1=$CARRY_1;};}",
        ] {
            assert_eq!(
                remove_carry_self_assignments(source).unwrap().1,
                0,
                "{source}"
            );
        }
        let source = "function f(){$CARRY_1=$CARRY_1;'use strict';var $CARRY_1;return this;}";
        let (code, count) = remove_carry_self_assignments(source).unwrap();
        assert_eq!(count, 1);
        assert!(code.contains("{;'use strict'"), "{code}");
    }
}
