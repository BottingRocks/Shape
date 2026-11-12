//! Simplify generated VM subroutines before call-site inlining.
//!
//! Only local, linear bodies are rewritten. Array construction is delegated to
//! the existing fresh-array proof; the other two shapes keep their single
//! initializer at the same first observable evaluation point.
//!
//! One shape moves a literal array into a helper call when its binding is read
//! once; another substitutes a single linear initializer at its unique read.
//! Bindings must have one write-free reference, bodies must have the expected
//! statement count and an empty directive prologue, and comments in a rewritten
//! declaration decline. The pass removes helper temporaries and exposes
//! tuple/call shapes for later inlining.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::Scoping;
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;

use super::super::simplify_array_constructions;
use super::super::support::whole_line;

pub(crate) struct SimplifiedVmSubs {
    pub code: String,
    // Read by the tests; the program no longer reports it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub simplified: usize,
}

pub(crate) fn simplify_vm_sub_bodies(source: &str) -> Result<SimplifiedVmSubs, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut spans = HelperSpans::default();
    spans.visit_program(&program);
    if spans.dynamic {
        return Ok(SimplifiedVmSubs {
            code: source.into(),
            simplified: 0,
        });
    }
    let mut array_edits = Vec::new();
    for span in spans.spans {
        let original = &source[span.start as usize..span.end as usize];
        let outcome = simplify_array_constructions(original).map_err(|error| error.to_string())?;
        if outcome.compacted_arrays > 0 {
            array_edits.push(analysis::Edit::replace(span, outcome.code));
        }
    }
    let array_simplified = array_edits.len();
    let mut code = if array_edits.is_empty() {
        source.to_owned()
    } else {
        let applied = analysis::apply_edits(source, array_edits);
        if !applied.rejected.is_empty() {
            return Err("VM helper array edits overlap".into());
        }
        applied.output
    };
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, &code)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scan = LinearHelpers {
        source: &code,
        scoping: &scoping,
        edits: Vec::new(),
        simplified: 0,
        dynamic: false,
    };
    scan.visit_program(&program);
    let simplified = array_simplified + scan.simplified;
    if !scan.dynamic && !scan.edits.is_empty() {
        let applied = analysis::apply_edits(&code, scan.edits);
        if !applied.rejected.is_empty() {
            return Err("VM helper linear edits overlap".into());
        }
        code = applied.output;
    }
    let check = Allocator::default();
    analysis::check_program(&check, &code)
        .map_err(|error| format!("simplified VM helpers: {error}"))?;
    Ok(SimplifiedVmSubs { simplified, code })
}

#[derive(Default)]
struct HelperSpans {
    spans: Vec<Span>,
    dynamic: bool,
    inside_helper: bool,
}

impl<'a> Visit<'a> for HelperSpans {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let helper = function
            .id
            .as_ref()
            .is_some_and(|id| id.name.starts_with("vm_sub_"));
        if helper && !self.inside_helper {
            self.spans.push(function.span);
        }
        let prior = self.inside_helper;
        self.inside_helper |= helper;
        walk::walk_function(self, function, flags);
        self.inside_helper = prior;
    }

    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "eval" {
            self.dynamic = true;
        }
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

struct LinearHelpers<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    edits: Vec<analysis::Edit>,
    simplified: usize,
    dynamic: bool,
}

impl LinearHelpers<'_> {
    fn consider(&mut self, function: &Function<'_>) {
        let Some(body) = function.body.as_ref() else {
            return;
        };
        if self.inline_array_argument(body) {
            return;
        }
        if body.statements.len() != 2 || !body.directives.is_empty() {
            return;
        }
        let Statement::VariableDeclaration(declaration) = &body.statements[0] else {
            return;
        };
        if declaration.kind != VariableDeclarationKind::Let || declaration.declarations.len() != 1 {
            return;
        }
        let binding = &declaration.declarations[0];
        let (BindingPattern::BindingIdentifier(id), Some(initializer)) =
            (&binding.id, binding.init.as_ref())
        else {
            return;
        };
        let Some(symbol) = id.symbol_id.get() else {
            return;
        };
        let Statement::ReturnStatement(ret) = &body.statements[1] else {
            return;
        };
        let Some(returned) = ret.argument.as_ref() else {
            return;
        };
        let replacement = match returned {
            Expression::Identifier(read)
                if analysis::identifier_symbol(read, self.scoping) == Some(symbol)
                    && matches!(initializer, Expression::ArrayExpression(_)) =>
            {
                Some(read.span)
            }
            Expression::ArrayExpression(array) => array.elements.first().and_then(|first| {
                let Expression::Identifier(read) = first.as_expression()? else {
                    return None;
                };
                (analysis::identifier_symbol(read, self.scoping) == Some(symbol))
                    .then_some(read.span)
            }),
            _ => None,
        };
        let Some(read) = replacement else {
            return;
        };
        let refs = self
            .scoping
            .get_resolved_references(symbol)
            .collect::<Vec<_>>();
        if refs.len() != 1 || refs[0].is_write() {
            return;
        }
        let rhs = &self.source[initializer.span().start as usize..initializer.span().end as usize];
        let declaration_span = whole_line(self.source, body.statements[0].span());
        let original = &self.source[declaration_span.start as usize..declaration_span.end as usize];
        if original.contains("//") || original.contains("/*") {
            return;
        }
        self.edits.push(analysis::Edit::delete(declaration_span));
        self.edits
            .push(analysis::Edit::replace(read, rhs.to_owned()));
        self.simplified += 1;
    }

    fn inline_array_argument(&mut self, body: &FunctionBody<'_>) -> bool {
        if body.statements.len() != 3 || !body.directives.is_empty() {
            return false;
        }
        let Statement::VariableDeclaration(array_decl) = &body.statements[0] else {
            return false;
        };
        let Statement::VariableDeclaration(value_decl) = &body.statements[1] else {
            return false;
        };
        let Statement::ReturnStatement(ret) = &body.statements[2] else {
            return false;
        };
        if array_decl.kind != VariableDeclarationKind::Let
            || array_decl.declarations.len() != 1
            || value_decl.kind != VariableDeclarationKind::Let
            || value_decl.declarations.len() != 1
        {
            return false;
        }
        let array = &array_decl.declarations[0];
        let value = &value_decl.declarations[0];
        let (
            BindingPattern::BindingIdentifier(array_id),
            Some(Expression::ArrayExpression(literal)),
        ) = (&array.id, array.init.as_ref())
        else {
            return false;
        };
        let (BindingPattern::BindingIdentifier(value_id), Some(Expression::CallExpression(call))) =
            (&value.id, value.init.as_ref())
        else {
            return false;
        };
        let (Some(array_symbol), Some(value_symbol)) =
            (array_id.symbol_id.get(), value_id.symbol_id.get())
        else {
            return false;
        };
        let Some(Expression::ArrayExpression(returned)) = ret.argument.as_ref() else {
            return false;
        };
        if returned.elements.len() != 2 || !returned.elements.iter().all(|element| {
            matches!(element.as_expression(), Some(Expression::Identifier(id)) if analysis::identifier_symbol(id, self.scoping) == Some(value_symbol))
        }) { return false; }
        if self.scoping.get_resolved_references(array_symbol).count() != 1
            || self.scoping.get_resolved_references(value_symbol).count() != 2
        {
            return false;
        }
        if !literal.elements.iter().all(|element| {
            matches!(
                element.as_expression(),
                Some(
                    Expression::StringLiteral(_)
                        | Expression::NumericLiteral(_)
                        | Expression::BooleanLiteral(_)
                        | Expression::NullLiteral(_)
                )
            )
        }) {
            return false;
        }
        let Some(last) = call.arguments.last() else {
            return false;
        };
        if last.is_spread()
            || call.arguments[..call.arguments.len() - 1]
                .iter()
                .any(|arg| arg.is_spread())
        {
            return false;
        }
        let Expression::Identifier(read) = last.to_expression() else {
            return false;
        };
        if analysis::identifier_symbol(read, self.scoping) != Some(array_symbol) {
            return false;
        }
        if !call.arguments[..call.arguments.len() - 1]
            .iter()
            .all(|arg| {
                matches!(
                    arg.to_expression(),
                    Expression::Identifier(_)
                        | Expression::StringLiteral(_)
                        | Expression::NumericLiteral(_)
                        | Expression::BooleanLiteral(_)
                        | Expression::NullLiteral(_)
                )
            })
        {
            return false;
        }
        let original = &self.source
            [body.statements[0].span().start as usize..body.statements[0].span().end as usize];
        if original.contains("//") || original.contains("/*") {
            return false;
        }
        let literal_text = &self.source[literal.span.start as usize..literal.span.end as usize];
        self.edits.push(analysis::Edit::delete(whole_line(
            self.source,
            body.statements[0].span(),
        )));
        self.edits
            .push(analysis::Edit::replace(read.span, literal_text.to_owned()));
        self.simplified += 1;
        true
    }
}

impl<'a> Visit<'a> for LinearHelpers<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if function
            .id
            .as_ref()
            .is_some_and(|id| id.name.starts_with("vm_sub_"))
            && !function.r#async
            && !function.generator
        {
            self.consider(function);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "eval" {
            self.dynamic = true;
        }
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

#[cfg(test)]
mod tests {
    use super::simplify_vm_sub_bodies;

    #[test]
    fn simplifies_local_length_before_tuple_return() {
        let source = "function f() { function vm_sub_1_2($PARAMA, $PARAMB) { let $SSA_1 = $PARAMA.length; return [$SSA_1, $PARAMB]; } }";
        let output = simplify_vm_sub_bodies(source).unwrap();
        assert_eq!(output.simplified, 1, "{}", output.code);
        assert!(output.code.contains("return [$PARAMA.length, $PARAMB];"));
    }

    #[test]
    fn folds_fresh_array_and_exposes_literal_return() {
        let source = "function f() { function vm_sub_1_2() { let $ARRAY_1 = [void 0,]; $ARRAY_1[0] = 'x'; return $ARRAY_1; } let x = vm_sub_1_2(); }";
        let output = simplify_vm_sub_bodies(source).unwrap();
        assert!(output.code.contains("return ["), "{}", output.code);
        assert!(!output.code.contains("$ARRAY_1[0]"));
    }

    #[test]
    fn places_fresh_array_literal_in_last_call_argument() {
        let source = "function f() { function vm_sub_1_2($PARAMA, $PARAMB, $PARAMC) { let $ARRAY_1 = [void 0, void 0]; $ARRAY_1[0] = 'no-preference'; $ARRAY_1[1] = 'reduce'; let $SSA_1 = (0, $PARAMA)($PARAMB, $PARAMC, $ARRAY_1); return [$SSA_1, $SSA_1]; } }";
        let output = simplify_vm_sub_bodies(source).unwrap();
        assert!(
            output
                .code
                .contains("(0, $PARAMA)($PARAMB, $PARAMC, ['no-preference','reduce'])")
                || output
                    .code
                    .contains("(0, $PARAMA)($PARAMB, $PARAMC, ['no-preference', 'reduce'])"),
            "{}",
            output.code
        );
    }
}
