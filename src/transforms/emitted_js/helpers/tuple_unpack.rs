//! Collapse immediate indexed reads of a fresh helper result into one binding.
//!
//! This pass assumes the standard Array iterator, as does the program's
//! standard-builtins cleanup mode. It only accepts a local, immutable helper
//! whose linear body ends in a dense array literal return. The result must be
//! read exactly once at each index, in order, without intervening statements.
//! The run becomes one destructuring declaration. It shortens the caller but
//! deliberately leaves the helper's array allocation and call in place.

use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;

pub(crate) struct UnpackedTuples {
    pub code: String,
    pub collapsed: usize,
}

pub(crate) fn unpack_fresh_helper_tuples(source: &str) -> Result<UnpackedTuples, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        helpers: BTreeMap::new(),
        edits: Vec::new(),
        collapsed: 0,
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic || scan.edits.is_empty() {
        return Ok(UnpackedTuples {
            code: source.into(),
            collapsed: 0,
        });
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("fresh tuple unpack edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("fresh tuple unpack output: {error}"))?;
    Ok(UnpackedTuples {
        code: applied.output,
        collapsed: scan.collapsed,
    })
}

/// A generated helper whose linear body ends in a dense array literal return:
/// every call yields a fresh array of that length. Callers must still check
/// that the helper's binding is never reassigned.
pub(crate) fn fresh_tuple_helper(function: &Function<'_>) -> Option<(SymbolId, usize)> {
    let (Some(id), Some(body)) = (&function.id, &function.body) else {
        return None;
    };
    let Some(Statement::ReturnStatement(ret)) = body.statements.last() else {
        return None;
    };
    if !(id.name.starts_with("$VMHELPER_") || id.name.starts_with("vm_sub_"))
        || function.r#async
        || function.generator
        || !body.directives.is_empty()
        || !body.statements[..body.statements.len() - 1].iter().all(|stmt| {
            matches!(
                stmt,
                Statement::VariableDeclaration(_) | Statement::ExpressionStatement(_)
            )
        })
    {
        return None;
    }
    let (Some(symbol), Some(Expression::ArrayExpression(array))) =
        (id.symbol_id.get(), ret.argument.as_ref())
    else {
        return None;
    };
    array
        .elements
        .iter()
        .all(|element| element.as_expression().is_some())
        .then_some((symbol, array.elements.len()))
}

struct Scan<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'s>,
    helpers: BTreeMap<SymbolId, usize>,
    edits: Vec<analysis::Edit>,
    collapsed: usize,
    dynamic: bool,
}

impl Scan<'_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        let mut index = 0;
        while index < statements.len() {
            let Statement::VariableDeclaration(decl) = &statements[index] else {
                index += 1;
                continue;
            };
            if decl.kind != VariableDeclarationKind::Let || decl.declarations.len() != 1 {
                index += 1;
                continue;
            }
            let binding = &decl.declarations[0];
            let (BindingPattern::BindingIdentifier(tuple), Some(Expression::CallExpression(call))) =
                (&binding.id, binding.init.as_ref())
            else {
                index += 1;
                continue;
            };
            if !tuple.name.starts_with("$SSA_") {
                index += 1;
                continue;
            }
            let (Some(tuple_symbol), Expression::Identifier(callee)) =
                (tuple.symbol_id.get(), &call.callee)
            else {
                index += 1;
                continue;
            };
            let Some(helper_symbol) = analysis::identifier_symbol(callee, self.scoping) else {
                index += 1;
                continue;
            };
            let Some(&count) = self.helpers.get(&helper_symbol) else {
                index += 1;
                continue;
            };
            if !(2..=7).contains(&count)
                || index + count >= statements.len()
                || self
                    .scoping
                    .get_resolved_references(helper_symbol)
                    .any(|reference| reference.is_write())
            {
                index += 1;
                continue;
            }
            let mut names = Vec::new();
            let mut object_spans = Vec::new();
            for offset in 0..count {
                let Statement::VariableDeclaration(read_decl) = &statements[index + offset + 1]
                else {
                    break;
                };
                if read_decl.kind != VariableDeclarationKind::Let
                    || read_decl.declarations.len() != 1
                {
                    break;
                }
                let read = &read_decl.declarations[0];
                let (
                    BindingPattern::BindingIdentifier(id),
                    Some(Expression::ComputedMemberExpression(member)),
                ) = (&read.id, read.init.as_ref())
                else {
                    break;
                };
                let (Expression::Identifier(object), Expression::NumericLiteral(key)) =
                    (&member.object, &member.expression)
                else {
                    break;
                };
                if !id.name.starts_with("$SSA_")
                    || analysis::identifier_symbol(object, self.scoping) != Some(tuple_symbol)
                    || key.value != offset as f64
                {
                    break;
                }
                names.push(id.name.to_string());
                object_spans.push(object.span);
            }
            if names.len() != count {
                index += 1;
                continue;
            }
            let refs = self
                .scoping
                .get_resolved_references(tuple_symbol)
                .collect::<Vec<_>>();
            if refs.len() != count || refs.iter().any(|reference| reference.is_write()) {
                index += 1;
                continue;
            }
            let mut ref_spans = refs
                .iter()
                .map(|reference| self.nodes.get_node(reference.node_id()).kind().span())
                .collect::<Vec<_>>();
            ref_spans.sort_by_key(|span| span.start);
            if ref_spans != object_spans {
                index += 1;
                continue;
            }
            let replaced_span = Span::new(decl.span.start, statements[index + count].span().end);
            let replaced_text =
                &self.source[replaced_span.start as usize..replaced_span.end as usize];
            if replaced_text.contains("//") || replaced_text.contains("/*") {
                index += 1;
                continue;
            }
            let call_text = &self.source[call.span.start as usize..call.span.end as usize];
            let replacement = format!("let [{}] = {call_text};", names.join(", "));
            self.edits
                .push(analysis::Edit::replace(replaced_span, replacement));
            self.collapsed += 1;
            index += count + 1;
        }
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some((symbol, length)) = fresh_tuple_helper(function) {
            self.helpers.insert(symbol, length);
        }
        walk::walk_function(self, function, flags);
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        walk::walk_block_statement(self, block);
        self.body(&block.body);
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
    use super::unpack_fresh_helper_tuples;

    #[test]
    fn collapses_dense_fresh_helper_tuple() {
        let source = "function f(x) { function $VMHELPER_1(a) { let v = a + 1; return [v, v]; } let $SSA_1 = $VMHELPER_1(x); let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; return $SSA_2 + $SSA_3; }";
        let output = unpack_fresh_helper_tuples(source).unwrap();
        assert_eq!(output.collapsed, 1, "{}", output.code);
        assert!(output
            .code
            .contains("let [$SSA_2, $SSA_3] = $VMHELPER_1(x);"));
    }

    #[test]
    fn retains_escaping_or_mutable_helper_result() {
        let escape = "function f(x) { function $VMHELPER_1(a) { return [a, a]; } let $SSA_1 = $VMHELPER_1(x); let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; use($SSA_1); }";
        assert_eq!(unpack_fresh_helper_tuples(escape).unwrap().collapsed, 0);
        let reassigned = "function f(x) { function $VMHELPER_1(a) { return [a, a]; } $VMHELPER_1 = other; let $SSA_1 = $VMHELPER_1(x); let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; }";
        assert_eq!(unpack_fresh_helper_tuples(reassigned).unwrap().collapsed, 0);
    }

    #[test]
    fn retains_comments_and_non_dense_helper_returns() {
        let commented = "function f(x) { function $VMHELPER_1(a) { return [a, a]; } let $SSA_1 = $VMHELPER_1(x); /* explain */ let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; }";
        assert_eq!(unpack_fresh_helper_tuples(commented).unwrap().collapsed, 0);
        let sparse = "function f(x) { function $VMHELPER_1(a) { return [a, , a]; } let $SSA_1 = $VMHELPER_1(x); let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; let $SSA_4 = $SSA_1[2]; }";
        assert_eq!(unpack_fresh_helper_tuples(sparse).unwrap().collapsed, 0);
    }
}
