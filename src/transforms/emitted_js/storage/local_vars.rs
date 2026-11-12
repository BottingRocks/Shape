//! Locals the VM's slots leave behind.
//!
//! A slot the VM used for a jump target or a constant it never reads is a
//! `var $KEY_n = 9;` nothing reads: its stores go (a value that could run
//! code stays as a statement). A `var` only its own function uses, whose
//! first use stores it (`$KEY_4 = f(); var $KEY_4; use($KEY_4);`), is a
//! `let` at that store, which forwarding can then place like any other
//! value.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolFlags};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;

use crate::analysis;
use super::super::support::whole_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalVars {
    pub code: String,
    pub changed: usize,
}

pub(crate) fn clean_local_vars(source: &str) -> Result<LocalVars, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut dynamic = Dynamic::default();
    dynamic.visit_program(&program);
    if dynamic.0 {
        return Ok(LocalVars { code: source.into(), changed: 0 });
    }
    let (scoping, nodes) = analysis::analyze(&program);
    let cleaner = Cleaner { source, scoping: &scoping, nodes: &nodes };
    let mut edits: Vec<analysis::Edit> = Vec::new();
    let mut changed = 0;
    for symbol in scoping.symbol_ids() {
        let declarations: Vec<NodeId> = scoping.symbol_declarations(symbol).collect();
        if !declarations.iter().all(|d| matches!(nodes.kind(*d), AstKind::VariableDeclarator(_))) {
            continue;
        }
        let references: Vec<_> = scoping.get_resolved_references(symbol).collect();
        let proposal = if references.iter().all(|reference| reference.is_write() && !reference.is_read()) {
            cleaner.unread(symbol, &declarations)
        } else {
            cleaner.localize(symbol, &declarations)
        };
        let Some(proposal) = proposal else {
            continue;
        };
        if proposal.iter().any(|edit| {
            edits.iter().any(|other| edit.start < other.end && other.start < edit.end || (edit.start == other.start && edit.end == other.end))
        }) {
            continue;
        }
        edits.extend(proposal);
        changed += 1;
    }
    if changed == 0 {
        return Ok(LocalVars { code: source.into(), changed: 0 });
    }
    let output = super::super::support::apply_checked(source, edits, "local var")?;
    Ok(LocalVars { code: output, changed })
}

#[derive(Default)]
struct Dynamic(bool);

impl<'a> Visit<'a> for Dynamic {
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.0 = true;
        walk::walk_with_statement(self, statement);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.0 = true;
        }
    }
}

struct Cleaner<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
}

/// Evaluating it can run no code: literals, names, property reads (no
/// getters), operators over those, and functions or literals built of them.
fn quiet(expression: &Expression<'_>, scoping: &Scoping, nodes: &AstNodes<'_>) -> bool {
    struct Loud<'s, 'n>(bool, &'s Scoping, &'s AstNodes<'n>);
    impl Loud<'_, '_> {
        /// A name only ever holding an object or array literal, or the VM's
        /// key snapshot (`vmForInKeys(o)`, a fresh array): reading a
        /// property of it cannot throw.
        fn literal_object(&self, object: &Expression<'_>) -> bool {
            let Expression::Identifier(name) = object.without_parentheses() else {
                return false;
            };
            let Some(symbol) = self.1.get_reference(name.reference_id()).symbol_id() else {
                return false;
            };
            matches!(self.2.kind(self.1.symbol_declaration(symbol)), AstKind::VariableDeclarator(declarator)
                if matches!(&declarator.init, Some(Expression::ArrayExpression(_) | Expression::ObjectExpression(_)))
                    || matches!(&declarator.init, Some(Expression::CallExpression(call))
                        if matches!(&call.callee, Expression::Identifier(callee) if callee.name == "vmForInKeys")))
                && self.1.get_resolved_references(symbol).all(|reference| !reference.is_write())
        }
    }
    impl<'a> Visit<'a> for Loud<'_, '_> {
        // A global that may not exist throws `ReferenceError`. A generated
        // `$` name free in a thread is a slot an enclosing thread declares.
        fn visit_identifier_reference(&mut self, name: &IdentifierReference<'a>) {
            if self.1.get_reference(name.reference_id()).symbol_id().is_none()
                && !name.name.starts_with('$')
                && !super::super::primitives::is_stable_global(&name.name)
                && !matches!(name.name.as_str(), "window" | "self" | "document" | "navigator")
            {
                self.0 = true;
            }
        }
        // A property read throws on `null`/`undefined` (and getters are
        // taken not to exist), unless its object is a literal's name.
        fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
            self.0 |= !self.literal_object(&member.object);
            walk::walk_static_member_expression(self, member);
        }
        fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
            self.0 |= !self.literal_object(&member.object);
            walk::walk_computed_member_expression(self, member);
        }
        fn visit_private_field_expression(&mut self, _: &PrivateFieldExpression<'a>) {
            self.0 = true;
        }
        fn visit_call_expression(&mut self, _: &CallExpression<'a>) {
            self.0 = true;
        }
        fn visit_new_expression(&mut self, _: &NewExpression<'a>) {
            self.0 = true;
        }
        fn visit_tagged_template_expression(&mut self, _: &TaggedTemplateExpression<'a>) {
            self.0 = true;
        }
        fn visit_assignment_expression(&mut self, _: &AssignmentExpression<'a>) {
            self.0 = true;
        }
        fn visit_update_expression(&mut self, _: &UpdateExpression<'a>) {
            self.0 = true;
        }
        fn visit_await_expression(&mut self, _: &AwaitExpression<'a>) {
            self.0 = true;
        }
        fn visit_yield_expression(&mut self, _: &YieldExpression<'a>) {
            self.0 = true;
        }
        fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
            if unary.operator == oxc_syntax::operator::UnaryOperator::Delete {
                self.0 = true;
            }
            walk::walk_unary_expression(self, unary);
        }
        fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
        fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    }
    let mut loud = Loud(false, scoping, nodes);
    loud.visit_expression(expression);
    !loud.0
}

impl Cleaner<'_, '_> {
    fn function_of(&self, node: NodeId) -> Option<NodeId> {
        super::super::support::function_of(self.nodes, node)
    }

    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    /// Removing one declarator: the statement, or the name and a comma.
    fn declarator_removal(&self, declarator: NodeId, keep: Option<String>) -> Option<analysis::Edit> {
        let AstKind::VariableDeclarator(this) = self.nodes.kind(declarator) else {
            return None;
        };
        let AstKind::VariableDeclaration(declaration) = self.nodes.parent_kind(declarator) else {
            return None;
        };
        let parent = self.nodes.parent_kind(self.nodes.parent_id(declarator));
        let in_list = matches!(parent, AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_));
        if declaration.declarations.len() == 1 {
            if !in_list {
                return None;
            }
            return Some(match keep {
                Some(text) => analysis::Edit::replace(declaration.span, format!("{text};")),
                None => analysis::Edit::delete(whole_line(self.source, declaration.span)),
            });
        }
        if keep.is_some() {
            return None;
        }
        let at = declaration.declarations.iter().position(|other| other.span == this.span)?;
        Some(match declaration.declarations.get(at + 1) {
            Some(next) => analysis::Edit::delete(Span::new(this.span.start, next.span.start)),
            None => analysis::Edit::delete(Span::new(declaration.declarations[at - 1].span.end, this.span.end)),
        })
    }

    /// Nothing reads the name: drop its stores.
    fn unread(&self, symbol: oxc_semantic::SymbolId, declarations: &[NodeId]) -> Option<Vec<analysis::Edit>> {
        let mut edits = Vec::new();
        for declaration in declarations {
            let AstKind::VariableDeclarator(declarator) = self.nodes.kind(*declaration) else {
                return None;
            };
            if !matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
                return None;
            }
            let keep = match &declarator.init {
                Some(init) if !quiet(init, self.scoping, self.nodes) => Some(self.text(init.span()).to_string()),
                _ => None,
            };
            edits.push(self.declarator_removal(*declaration, keep)?);
        }
        for reference in self.scoping.get_resolved_references(symbol) {
            let assignment = self.nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(store) = self.nodes.kind(assignment) else {
                return None;
            };
            let AstKind::ExpressionStatement(statement) = self.nodes.parent_kind(assignment) else {
                return None;
            };
            if store.operator != AssignmentOperator::Assign
                || !matches!(
                    self.nodes.parent_kind(self.nodes.parent_id(assignment)),
                    AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_)
                )
            {
                return None;
            }
            edits.push(if quiet(&store.right, self.scoping, self.nodes) {
                analysis::Edit::delete(whole_line(self.source, statement.span))
            } else {
                analysis::Edit::replace(statement.span, format!("{};", self.text(store.right.span())))
            });
        }
        Some(edits)
    }

    /// `X = E; … var X; …` in one function, first used by that store at the
    /// top of a block holding every use: `let X = E;`.
    fn localize(&self, symbol: oxc_semantic::SymbolId, declarations: &[NodeId]) -> Option<Vec<analysis::Edit>> {
        let flags = self.scoping.symbol_flags(symbol);
        if !flags.contains(SymbolFlags::FunctionScopedVariable) || declarations.len() != 1 {
            return None;
        }
        let declaration = declarations[0];
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) else {
            return None;
        };
        if declarator.init.is_some() {
            return None;
        }
        let home = self.function_of(declaration);
        home?;
        let mut references: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
        if references.iter().any(|reference| self.function_of(reference.node_id()) != home) {
            return None;
        }
        references.sort_by_key(|reference| self.nodes.kind(reference.node_id()).span().start);
        let first = references.first()?;
        if !first.is_write() || first.is_read() {
            return None;
        }
        let assignment = self.nodes.parent_id(first.node_id());
        let AstKind::AssignmentExpression(store) = self.nodes.kind(assignment) else {
            return None;
        };
        let statement = self.nodes.parent_id(assignment);
        let AstKind::ExpressionStatement(store_statement) = self.nodes.kind(statement) else {
            return None;
        };
        if store.operator != AssignmentOperator::Assign {
            return None;
        }
        // The block holding the store holds every use.
        let block = self.nodes.parent_id(statement);
        let block_span = match self.nodes.kind(block) {
            AstKind::BlockStatement(block) => block.span,
            AstKind::FunctionBody(body) => body.span,
            _ => return None,
        };
        if !references.iter().all(|reference| {
            let span = self.nodes.kind(reference.node_id()).span();
            block_span.start <= span.start && span.end <= block_span.end
        }) {
            return None;
        }
        // The value cannot read the name itself.
        let value = store.right.span();
        if references.iter().any(|reference| {
            let span = self.nodes.kind(reference.node_id()).span();
            value.start <= span.start && span.end <= value.end
        }) {
            return None;
        }
        let mut edits = vec![self.declarator_removal(declaration, None)?];
        edits.push(analysis::Edit::replace(Span::new(store_statement.span.start, store_statement.span.start), "let "));
        // Later stores stay stores of the new name.
        Some(edits)
    }
}

#[cfg(test)]
mod tests {
    use super::clean_local_vars;

    #[test]
    fn drops_unread_slots_and_localizes_stored_first_vars() {
        let source = "function f(g, k) {\n  var $KEY_7 = 9;\n  var $KEY_1 = g();\n  $KEY_4 = k(1);\n  var $KEY_4;\n  g($KEY_4);\n  return 1;\n}";
        let result = clean_local_vars(source).unwrap();
        assert!(!result.code.contains("$KEY_7"), "{}", result.code);
        assert!(result.code.contains("g();") && !result.code.contains("$KEY_1"), "{}", result.code);
        assert!(result.code.contains("let $KEY_4 = k(1);") && !result.code.contains("var $KEY_4"), "{}", result.code);
    }

    #[test]
    fn keeps_a_read_that_could_throw_as_a_statement() {
        // A missing global throws; a property read of a maybe-null value
        // throws. A parent thread's slot and a literal's element do not.
        let source = "function f(o) {\n  var $KEY_1 = missingGlobal;\n  var $KEY_2 = o.p;\n  var $KEY_3 = $KEY_9;\n  let $A = [1, 2];\n  var $KEY_4 = $A[1];\n  let $B = vmForInKeys(o);\n  var $KEY_5 = $B[0];\n  return 1;\n}";
        let result = clean_local_vars(source).unwrap();
        assert!(result.code.contains("missingGlobal;") && result.code.contains("o.p;"), "{}", result.code);
        assert!(!result.code.contains("$KEY_9") && !result.code.contains("$A[1]") && !result.code.contains("$B[0]"), "{}", result.code);
    }

    #[test]
    fn keeps_a_var_a_closure_or_an_earlier_read_uses() {
        for source in [
            "function f(k) {\n  $KEY_4 = k(1);\n  var $KEY_4;\n  return () => $KEY_4;\n}",
            "function f(k) {\n  k($KEY_4);\n  $KEY_4 = k(1);\n  var $KEY_4;\n  return $KEY_4;\n}",
        ] {
            let result = clean_local_vars(source).unwrap();
            assert!(result.code.contains("var $KEY_4"), "{}", result.code);
        }
    }
}
