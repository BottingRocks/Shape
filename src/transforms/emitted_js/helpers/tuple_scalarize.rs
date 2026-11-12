//! Replace a fresh, non-escaping literal tuple and its immediate indexed reads
//! with the scalar bindings it carried.
//!
//! The tuple has 2–16 elements, followed by exactly one
//! `let $SSA_i = $SSA_t[i];` per element in order, with a later statement
//! present. The tuple must have no other references or writes, and every read
//! must use the exact tuple node. Each value still evaluates once in order.
//! Projection bindings must not be captured or observed during construction,
//! since scalar initialization happens earlier. The run becomes scalar
//! `let`s using verbatim value text, preserving `-0`, escapes, and lone
//! surrogates. `eval`/`with` declines. This removes the array and its index
//! reads and feeds scalar values to later SSA inlining.

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use super::super::index::SpanIndex;
use crate::analysis;

pub(crate) struct ScalarizedTuples {
    pub code: String,
    pub removed: usize,
}

pub(crate) fn scalarize_literal_tuples(source: &str) -> Result<ScalarizedTuples, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    #[derive(Default)]
    struct Regions(SpanIndex);
    impl<'a> Visit<'a> for Regions {
        fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
            self.0.push(f.span);
            walk::walk_function(self, f, flags);
        }
        fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
            self.0.push(f.span);
            walk::walk_arrow_function_expression(self, f);
        }
        fn visit_class(&mut self, c: &Class<'a>) {
            self.0.push(c.span);
            walk::walk_class(self, c);
        }
    }
    let mut regions = Regions::default();
    regions.visit_program(&program);
    regions.0.build();
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        regions: &regions.0,
        edits: Vec::new(),
        removed: 0,
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic || scan.edits.is_empty() {
        return Ok(ScalarizedTuples {
            code: source.into(),
            removed: 0,
        });
    }
    scan.edits
        .sort_by_key(|e| (e.start, std::cmp::Reverse(e.end)));
    let mut end = 0;
    scan.edits.retain(|e| {
        if e.start < end {
            return false;
        }
        end = e.end;
        true
    });
    scan.removed = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("literal tuple scalarization edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("literal tuple scalarization: {error}"))?;
    Ok(ScalarizedTuples {
        code: applied.output,
        removed: scan.removed,
    })
}

struct Scan<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'s>,
    regions: &'s SpanIndex,
    edits: Vec<analysis::Edit>,
    removed: usize,
    dynamic: bool,
}

/// The scalar bindings a run of tuple projections declares, in index order.
struct Projections {
    bindings: Vec<String>,
    binding_symbols: Vec<Option<SymbolId>>,
    object_spans: Vec<Span>,
}

/// A single `let $SSA_t = [...]` declaration: the declaration, the tuple's
/// symbol, and its (unparenthesized) array literal.
fn literal_tuple<'b, 'a>(
    statement: &'b Statement<'a>,
) -> Option<(&'b VariableDeclaration<'a>, SymbolId, &'b ArrayExpression<'a>)> {
    let Statement::VariableDeclaration(tuple_decl) = statement else {
        return None;
    };
    if tuple_decl.kind != VariableDeclarationKind::Let || tuple_decl.declarations.len() != 1
    {
        return None;
    }
    let tuple = &tuple_decl.declarations[0];
    let BindingPattern::BindingIdentifier(tuple_id) = &tuple.id else {
        return None;
    };
    if !tuple_id.name.starts_with("$SSA_") {
        return None;
    }
    let (Some(symbol), Some(initializer)) = (tuple_id.symbol_id.get(), tuple.init.as_ref())
    else {
        return None;
    };
    let mut initializer = initializer;
    while let Expression::ParenthesizedExpression(paren) = initializer {
        initializer = &paren.expression;
    }
    let Expression::ArrayExpression(array) = initializer else {
        return None;
    };
    Some((tuple_decl, symbol, array))
}

impl Scan<'_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        let mut cursor = 0;
        while cursor < statements.len() {
            match self.scalarize_at(statements, cursor) {
                Some(count) => cursor += count + 1,
                None => {
                    self.forward_at(&statements[cursor]);
                    cursor += 1;
                }
            }
        }
    }

    /// A tuple of plain values (literals, or `$SSA_`/`$PARAM` bindings never
    /// assigned), never written and read only as `t[i]` values in its own
    /// function: each read becomes the element it reads, and the tuple is
    /// left unread for dead-SSA removal. A read that was folded into a later
    /// expression (`(t[1]) !== "x"`) keeps the run form above from applying.
    fn forward_at(&mut self, statement: &Statement<'_>) {
        let Some((tuple_decl, symbol, array)) = literal_tuple(statement) else {
            return;
        };
        let count = array.elements.len();
        if !(2..=16).contains(&count) {
            return;
        }
        let mut values = Vec::new();
        for element in &array.elements {
            let Some(expression) = element.as_expression() else {
                return;
            };
            let plain = match expression {
                Expression::StringLiteral(_)
                | Expression::NumericLiteral(_)
                | Expression::BooleanLiteral(_)
                | Expression::NullLiteral(_) => Some(None),
                Expression::Identifier(id)
                    if id.name.starts_with("$SSA_") || id.name.starts_with("$PARAM") =>
                {
                    analysis::identifier_symbol(id, self.scoping)
                        .filter(|&element| {
                            !self
                                .scoping
                                .get_resolved_references(element)
                                .any(|reference| reference.is_write())
                        })
                        .map(|element| Some((element, id.name.to_string())))
                }
                _ => None,
            };
            let Some(plain) = plain else {
                return;
            };
            values.push((expression.span(), plain));
        }
        let region = self.regions.enclosing(tuple_decl.span);
        let mut edits = Vec::new();
        for reference in self.scoping.get_resolved_references(symbol) {
            if reference.is_write() {
                return;
            }
            let id = reference.node_id();
            let span = self.nodes.get_node(id).kind().span();
            if span.start < tuple_decl.span.end || self.regions.enclosing(span) != region {
                return;
            }
            let member_id = self.nodes.parent_id(id);
            let AstKind::ComputedMemberExpression(member) = self.nodes.kind(member_id) else {
                return;
            };
            let Expression::NumericLiteral(index) = &member.expression else {
                return;
            };
            if member.optional
                || member.object.span() != span
                || index.value.fract() != 0.0
                || index.value < 0.0
                || index.value >= count as f64
                || !self.value_use(member_id, member.span)
            {
                return;
            }
            let (value, element) = &values[index.value as usize];
            // The element's binding must be the one the read site sees.
            if let Some((element, name)) = element {
                let scope = self.nodes.get_node(id).scope_id();
                if self.scoping.find_binding(scope, name.as_str().into()) != Some(*element) {
                    return;
                }
            }
            let text = &self.source[value.start as usize..value.end as usize];
            edits.push(analysis::Edit::replace(member.span, format!("({text})")));
        }
        if edits.is_empty() {
            return;
        }
        self.removed += 1;
        self.edits.extend(edits);
    }

    /// Whether the member read at `member_id` is used only as a value: not
    /// written, deleted, updated, nor called with the tuple as its receiver.
    fn value_use(&self, member_id: oxc_semantic::NodeId, member: Span) -> bool {
        let mut node = member_id;
        let mut span = member;
        loop {
            let parent = self.nodes.parent_id(node);
            match self.nodes.kind(parent) {
                AstKind::ParenthesizedExpression(paren) => {
                    node = parent;
                    span = paren.span;
                }
                AstKind::CallExpression(call) => return call.callee.span() != span,
                AstKind::TaggedTemplateExpression(_)
                | AstKind::UpdateExpression(_)
                | AstKind::ChainExpression(_) => return false,
                AstKind::UnaryExpression(unary) => {
                    return unary.operator != UnaryOperator::Delete
                }
                AstKind::AssignmentExpression(assignment) => {
                    return assignment.right.span() == span
                }
                AstKind::BinaryExpression(_)
                | AstKind::LogicalExpression(_)
                | AstKind::ConditionalExpression(_)
                | AstKind::VariableDeclarator(_)
                | AstKind::ExpressionStatement(_)
                | AstKind::ReturnStatement(_)
                | AstKind::ArrayExpression(_)
                | AstKind::NewExpression(_) => return true,
                _ => return false,
            }
        }
    }

    /// Replace the tuple declared at `statements[cursor]` and the projections
    /// that follow it; the tuple's element count when it is replaced.
    fn scalarize_at(&mut self, statements: &[Statement<'_>], cursor: usize) -> Option<usize> {
        let (tuple_decl, symbol, array) = literal_tuple(&statements[cursor])?;
        let count = array.elements.len();
        if !(2..=16).contains(&count) || cursor + count >= statements.len() {
            return None;
        }
        let values = array
            .elements
            .iter()
            .map(|element| element.as_expression().map(GetSpan::span))
            .collect::<Option<Vec<_>>>()?;

        let Projections {
            bindings,
            binding_symbols,
            object_spans,
        } = self.projections(statements, cursor, count, symbol);
        if bindings.len() != count {
            return None;
        }
        if self.projection_observed(binding_symbols, array.span, tuple_decl.span) {
            return None;
        }
        if !self.only_projection_reads(symbol, count, &object_spans) {
            return None;
        }
        let replacement = self.replacement(tuple_decl.span.start, &bindings, &values);
        let end = statements[cursor + count].span().end;
        if self.comment_outside_values(tuple_decl.span.start, end, &values) {
            return None;
        }
        self.edits.push(analysis::Edit::replace(
            Span::new(tuple_decl.span.start, end),
            replacement,
        ));
        self.removed += 1;
        Some(count)
    }

    /// The leading `let $SSA_i = $SSA_t[i];` run after the tuple, stopping at
    /// the first statement that is not the next projection.
    fn projections(
        &self,
        statements: &[Statement<'_>],
        cursor: usize,
        count: usize,
        symbol: SymbolId,
    ) -> Projections {
        let mut bindings = Vec::new();
        let mut binding_symbols = Vec::new();
        let mut object_spans = Vec::new();
        for index in 0..count {
            let Statement::VariableDeclaration(declaration) = &statements[cursor + index + 1]
            else {
                break;
            };
            if declaration.kind != VariableDeclarationKind::Let
                || declaration.declarations.len() != 1
            {
                break;
            }
            let binding = &declaration.declarations[0];
            let BindingPattern::BindingIdentifier(id) = &binding.id else {
                break;
            };
            if !id.name.starts_with("$SSA_") {
                break;
            }
            let Some(Expression::ComputedMemberExpression(member)) = binding.init.as_ref()
            else {
                break;
            };
            let Expression::Identifier(object) = &member.object else {
                break;
            };
            let Expression::NumericLiteral(property) = &member.expression else {
                break;
            };
            if analysis::identifier_symbol(object, self.scoping) != Some(symbol)
                || property.value != index as f64
            {
                break;
            }
            bindings.push(id.name.to_string());
            binding_symbols.push(id.symbol_id.get());
            object_spans.push(object.span);
        }
        Projections {
            bindings,
            binding_symbols,
            object_spans,
        }
    }

    /// Whether a projection binding is read inside the tuple's values or from
    /// another function or class region.
    fn projection_observed(
        &self,
        binding_symbols: Vec<Option<SymbolId>>,
        array: Span,
        tuple_decl: Span,
    ) -> bool {
        // Scalar declarations initialize earlier than the original tuple
        // projections. A value reading one of those bindings must retain
        // its original TDZ behavior.
        binding_symbols.into_iter().flatten().any(|symbol| {
            self.scoping.get_resolved_references(symbol).any(|r| {
                let span = self.nodes.get_node(r.node_id()).kind().span();
                (array.start <= span.start && span.end <= array.end)
                    || self.regions.enclosing(span) != self.regions.enclosing(tuple_decl)
            })
        })
    }

    /// Whether the tuple is never written and read only by the projections'
    /// objects at `object_spans`.
    fn only_projection_reads(&self, symbol: SymbolId, count: usize, object_spans: &[Span]) -> bool {
        let refs = self
            .scoping
            .get_resolved_references(symbol)
            .collect::<Vec<_>>();
        if refs.len() != count || refs.iter().any(|reference| reference.is_write()) {
            return false;
        }
        // The tuple's only references must be exactly these indexed reads.
        let mut reference_spans = refs
            .iter()
            .map(|reference| self.nodes.get_node(reference.node_id()).kind().span())
            .collect::<Vec<_>>();
        reference_spans.sort_by_key(|span| span.start);
        reference_spans == object_spans
    }

    /// One `let name = (value);` per binding, at the tuple's indentation.
    fn replacement(&self, start: u32, bindings: &[String], values: &[Span]) -> String {
        let start = start as usize;
        let line_start = self.source[..start].rfind('\n').map_or(0, |p| p + 1);
        let indent = if self.source[line_start..start].trim().is_empty() {
            &self.source[line_start..start]
        } else {
            ""
        };
        bindings
            .iter()
            .zip(values.iter())
            .map(|(name, span)| {
                let value = &self.source[span.start as usize..span.end as usize];
                format!("let {name} = ({value});")
            })
            .collect::<Vec<_>>()
            .join(&format!("\n{indent}"))
    }

    /// Whether `start..end` holds a comment outside the element values.
    fn comment_outside_values(&self, start: u32, end: u32, values: &[Span]) -> bool {
        // Only the element values survive: a comment anywhere else in
        // the replaced statements would be deleted.
        let mut outside = String::new();
        let mut at = start as usize;
        let mut kept: Vec<Span> = values.to_vec();
        kept.sort_by_key(|span| span.start);
        for span in &kept {
            outside.push_str(&self.source[at..span.start as usize]);
            at = span.end as usize;
        }
        outside.push_str(&self.source[at..end as usize]);
        outside.contains("/*") || outside.contains("//")
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
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
    use super::scalarize_literal_tuples;
    use crate::emitted_cleanup::inline_vm_sub_wrappers;

    #[test]
    fn removes_fresh_tuple_and_index_reads() {
        let source = "function f($PARAMA) { let $SSA_1 = [$PARAMA, 'name']; let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; return [$SSA_2, $SSA_3]; }";
        let result = scalarize_literal_tuples(source).unwrap();
        assert_eq!(result.removed, 1, "{}", result.code);
        assert!(
            result.code.contains("let $SSA_2 = ($PARAMA);"),
            "{}",
            result.code
        );
        assert!(
            result.code.contains("let $SSA_3 = ('name');"),
            "{}",
            result.code
        );
        assert!(!result.code.contains("let $SSA_1"), "{}", result.code);
    }

    #[test]
    fn forwards_reads_of_a_plain_tuple_folded_into_later_expressions() {
        let source = "function f($PARAMA) { let $SSA_1 = [$PARAMA, $PARAMA]; let $SSA_2 = $SSA_1[0]; let $SSA_3 = ($SSA_1[1]) !== 'x'; return [$SSA_2, $SSA_3]; }";
        let output = scalarize_literal_tuples(source).unwrap();
        assert!(output.code.contains("let $SSA_2 = ($PARAMA);"), "{}", output.code);
        assert!(output.code.contains("(($PARAMA)) !== 'x'"), "{}", output.code);
        assert!(!output.code.contains("$SSA_1["), "{}", output.code);
    }

    #[test]
    fn keeps_reads_of_a_tuple_that_is_a_receiver_written_or_impure() {
        // `t[0]()` calls with `t` as `this`.
        let call = "function f($PARAMA) { let $SSA_1 = [$PARAMA, 1]; let $SSA_2 = $SSA_1[0](); return $SSA_2; }";
        assert!(scalarize_literal_tuples(call).unwrap().code.contains("$SSA_1[0]()"));
        let write = "function f($PARAMA) { let $SSA_1 = [$PARAMA, 1]; $SSA_1[0] = 2; return $SSA_1[0] + $SSA_1[1]; }";
        assert!(scalarize_literal_tuples(write).unwrap().code.contains("$SSA_1[0] + $SSA_1[1]"));
        let reassigned = "function f($PARAMA) { let $SSA_9 = 1; let $SSA_1 = [$SSA_9, 1]; $SSA_9 = 2; return $SSA_1[0] + $SSA_1[1]; }";
        assert!(scalarize_literal_tuples(reassigned).unwrap().code.contains("$SSA_1[0] + $SSA_1[1]"));
        let impure = "function f($PARAMA) { let $SSA_1 = [g(), 1]; return $SSA_1[0] + $SSA_1[1]; }";
        assert!(scalarize_literal_tuples(impure).unwrap().code.contains("$SSA_1[0] + $SSA_1[1]"));
        let escapes = "function f($PARAMA) { let $SSA_1 = [$PARAMA, 1]; return [$SSA_1, $SSA_1[0]]; }";
        assert!(scalarize_literal_tuples(escapes).unwrap().code.contains("[$SSA_1, $SSA_1[0]]"));
    }

    #[test]
    fn retains_a_tuple_with_a_comment_outside_its_values() {
        let source = "function f(a) { let $SSA_1 = [1, /* keep */ 2]; let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; return [$SSA_2, $SSA_3]; }";
        let result = scalarize_literal_tuples(source).unwrap();
        assert!(result.code.contains("/* keep */"), "{}", result.code);
    }

    #[test]
    fn retains_escaping_tuple_and_scalarizes_effectful_values_in_order() {
        let escape = "function f(a) { let $SSA_1 = [a, 2]; let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; return [$SSA_1, $SSA_2, $SSA_3]; }";
        assert_eq!(scalarize_literal_tuples(escape).unwrap().removed, 0);
        let effect = "function f() { let $SSA_1 = [run(), 2]; let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; return [$SSA_2, $SSA_3]; }";
        assert_eq!(scalarize_literal_tuples(effect).unwrap().removed, 1);
    }

    #[test]
    fn scalarizes_first_property_read_before_pure_second_element() {
        let source = "function f($PARAMA) { let $SSA_1 = [$PARAMA.length, $PARAMA]; let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; return [$SSA_2, $SSA_3]; }";
        let output = scalarize_literal_tuples(source).unwrap();
        assert_eq!(output.removed, 1, "{}", output.code);
        assert!(
            output.code.contains("let $SSA_2 = ($PARAMA.length);"),
            "{}",
            output.code
        );
        assert!(
            output.code.contains("let $SSA_3 = ($PARAMA);"),
            "{}",
            output.code
        );
    }

    #[test]
    fn scalarizes_a_traced_tuple_wrapper_after_inlining() {
        let source = "function outer($SSA_arg) { function vm_sub_1_2($PARAMA) { return [$PARAMA, 'name']; } let $SSA_1 = vm_sub_1_2($SSA_arg); let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; return $SSA_3; }";
        let inlined = inline_vm_sub_wrappers(source).unwrap();
        let output = scalarize_literal_tuples(&inlined.code).unwrap();
        assert_eq!(output.removed, 1, "{}", output.code);
        assert!(
            output.code.contains("let $SSA_2 = ($SSA_arg);"),
            "{}",
            output.code
        );
        assert!(
            output.code.contains("let $SSA_3 = ('name');"),
            "{}",
            output.code
        );
    }
}
