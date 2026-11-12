//! Declare each slot variable once, where its uses are.
//!
//! The emitter declares a slot `$KEY_n` with `var` at its first store in
//! emission order, or in a `let $KEY_a, $KEY_b;` list at the top of the
//! function; cleanups then move the code around it. Each such variable is
//! one binding per call of its function. One `let` in the innermost block
//! holding every use (and every initialized declaration) is too, as long
//! as that block runs at most once per call: so the block is never inside
//! a loop the uses share, which lifts the `let` before that loop.
//!
//! The `let` goes just before the block's first statement using the name,
//! and becomes that statement when it is a plain store not reading the name
//! (`let x = v;`). A use inside a nested function (a created thread) could
//! run as soon as its function exists, so then the `let` goes first in the
//! block. A declaration with a value elsewhere becomes a store.

use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn place_key_declarations(source: &str) -> Result<(String, usize), String> {
    if !source.contains("$KEY_") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut collect = Collect { source, dynamic: false, fixed: Vec::new(), by_symbol: BTreeMap::new() };
    collect.visit_program(&program);
    if collect.dynamic {
        return Ok((source.into(), 0));
    }
    let place = Place { scoping: &scoping, nodes: &nodes };
    // New declarations, by the statement they go before.
    let mut inserts: BTreeMap<u32, (Span, Vec<String>)> = BTreeMap::new();
    // Stores that become declarations.
    let mut folded: BTreeMap<u32, (Span, String)> = BTreeMap::new();
    // Declaration statements: their declarator texts (`None` once moved)
    // and the stores replacing moved values.
    let mut lists: BTreeMap<u32, (Span, VariableDeclarationKind, Vec<Option<String>>, Vec<String>)> = BTreeMap::new();
    let mut count = 0;
    for (symbol, declarators) in &collect.by_symbol {
        if collect.fixed.contains(symbol) {
            continue;
        }
        // Never used and never given a value: it goes.
        if scoping.get_resolved_reference_ids(*symbol).is_empty() && declarators.iter().all(|d| d.init.is_none()) {
            for declarator in declarators {
                lists
                    .entry(declarator.statement.start)
                    .or_insert_with(|| (declarator.statement, declarator.kind, declarator.all.iter().cloned().map(Some).collect(), Vec::new()))
                    .2[declarator.position] = None;
            }
            count += 1;
            continue;
        }
        let Some(plan) = place.plan(*symbol, declarators) else {
            continue;
        };
        count += 1;
        let name = scoping.symbol_name(*symbol);
        let text = |span: Span| &source[span.start as usize..span.end as usize];
        for declarator in declarators {
            let list = lists
                .entry(declarator.statement.start)
                .or_insert_with(|| (declarator.statement, declarator.kind, declarator.all.iter().cloned().map(Some).collect(), Vec::new()));
            list.2[declarator.position] = None;
            match (declarator.init, plan.fold) {
                // This declaration's value starts the new `let`.
                (Some(init), Fold::Declaration(at)) if at == declarator.statement => {
                    inserts.entry(at.start).or_insert((at, Vec::new())).1.push(format!("let {name} = {};", text(init)));
                }
                (Some(init), _) => list.3.push(format!("{name} = {};", text(init))),
                (None, _) => {}
            }
        }
        match plan.fold {
            Fold::Store(at, value) => {
                folded.insert(at.start, (at, format!("let {name} = {};", text(value))));
            }
            Fold::Declaration(_) => {}
            Fold::None(at) => inserts.entry(at.start).or_insert((at, Vec::new())).1.push(name.to_string()),
        }
    }
    if count == 0 {
        return Ok((source.into(), 0));
    }
    let mut edits = Vec::new();
    let mut inserted: BTreeMap<u32, (Span, String)> = BTreeMap::new();
    for (start, (span, mut declarations)) in inserts {
        let indent = indent_of(source, span);
        // Names alone share one `let`; initialized ones follow.
        declarations.sort_by_key(|text| text.starts_with("let "));
        let (bare, valued): (Vec<String>, Vec<String>) = declarations.into_iter().partition(|text| !text.starts_with("let "));
        let mut lines = Vec::new();
        if !bare.is_empty() {
            lines.push(format!("let {};", bare.join(", ")));
        }
        lines.extend(valued);
        inserted.insert(start, (span, lines.join(&format!("\n{indent}"))));
    }
    for (_, (span, text)) in folded {
        edits.push(analysis::Edit::replace(span, text));
    }
    for (_, (span, kind, items, stores)) in lists {
        let keyword = match kind {
            VariableDeclarationKind::Var => "var",
            _ => "let",
        };
        let left: Vec<String> = items.into_iter().flatten().collect();
        let mut lines = Vec::new();
        if !left.is_empty() {
            lines.push(format!("{keyword} {};", left.join(", ")));
        }
        lines.extend(stores);
        // New declarations before this statement take its place.
        if let Some((_, text)) = inserted.remove(&span.start) {
            lines.insert(0, text);
        }
        if lines.is_empty() {
            edits.push(analysis::Edit::delete(whole_line(source, span)));
        } else {
            edits.push(analysis::Edit::replace(span, lines.join(&format!("\n{}", indent_of(source, span)))));
        }
    }
    for (_, (span, text)) in inserted {
        let indent = indent_of(source, span);
        edits.push(analysis::Edit::replace(Span::new(span.start, span.start), format!("{text}\n{indent}")));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err(format!("key declaration edits overlap: {}", analysis::describe_rejected_edits(&applied.rejected)));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("key declaration output: {error}"))?;
    Ok((applied.output, count))
}

struct Declarator {
    /// The declaration statement and this declarator's place in its list.
    statement: Span,
    kind: VariableDeclarationKind,
    position: usize,
    /// The list's declarator texts.
    all: Vec<String>,
    init: Option<Span>,
    node: NodeId,
}

/// The `$KEY_` declarators in statement lists, by variable.
struct Collect<'s> {
    source: &'s str,
    dynamic: bool,
    /// Variables some declaration of which is not in a block's list.
    fixed: Vec<SymbolId>,
    by_symbol: BTreeMap<SymbolId, Vec<Declarator>>,
}

impl Collect<'_> {
    fn list(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            if !matches!(declaration.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let) {
                continue;
            }
            let all: Vec<String> = declaration
                .declarations
                .iter()
                .map(|d| self.source[d.span.start as usize..d.span.end as usize].to_string())
                .collect();
            for (position, declarator) in declaration.declarations.iter().enumerate() {
                let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                    continue;
                };
                let Some(symbol) = binding.symbol_id.get() else {
                    continue;
                };
                if !binding.name.starts_with("$KEY_") {
                    continue;
                }
                self.by_symbol.entry(symbol).or_default().push(Declarator {
                    statement: declaration.span,
                    kind: declaration.kind,
                    position,
                    all: all.clone(),
                    init: declarator.init.as_ref().map(GetSpan::span),
                    node: binding.node_id.get(),
                });
            }
        }
    }

    fn fix(&mut self, declaration: &VariableDeclaration<'_>) {
        for declarator in &declaration.declarations {
            if let BindingPattern::BindingIdentifier(binding) = &declarator.id {
                if let Some(symbol) = binding.symbol_id.get() {
                    self.fixed.push(symbol);
                }
            }
        }
    }
}

impl<'a> Visit<'a> for Collect<'_> {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.list(&body.statements);
        walk::walk_function_body(self, body);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.list(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_switch_case(&mut self, case: &SwitchCase<'a>) {
        for statement in &case.consequent {
            if let Statement::VariableDeclaration(declaration) = statement {
                self.fix(declaration);
            }
        }
        walk::walk_switch_case(self, case);
    }
    fn visit_for_statement_init(&mut self, init: &ForStatementInit<'a>) {
        if let ForStatementInit::VariableDeclaration(declaration) = init {
            self.fix(declaration);
        }
        walk::walk_for_statement_init(self, init);
    }
    fn visit_for_statement_left(&mut self, left: &ForStatementLeft<'a>) {
        if let ForStatementLeft::VariableDeclaration(declaration) = left {
            self.fix(declaration);
        }
        walk::walk_for_statement_left(self, left);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.dynamic |= identifier.name == "eval";
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

/// One step of a use's path: a statement list's owner (a function body or
/// block) and the statement in it holding the use.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Step {
    container: NodeId,
    statement: Span,
    /// A loop lies between this list and the one outside it.
    in_loop: bool,
}

#[derive(Clone, Copy)]
enum Fold {
    /// `let name;` goes before this statement.
    None(Span),
    /// This store `name = value;` becomes `let name = value;`.
    Store(Span, Span),
    /// This declaration of the name alone keeps its value, as a `let`.
    Declaration(Span),
}

struct Plan {
    fold: Fold,
}

struct Place<'s, 'a> {
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
}

impl Place<'_, '_> {
    /// The path from the variable's function body down to `node`, and the
    /// outermost function it passes through on the way.
    fn path(&self, node: NodeId, home: NodeId) -> Option<(Vec<Step>, Option<NodeId>)> {
        let mut steps = Vec::new();
        let mut child = node;
        let mut nested = None;
        let mut in_loop = false;
        for ancestor in self.nodes.ancestor_ids(node) {
            match self.nodes.kind(ancestor) {
                AstKind::FunctionBody(_) | AstKind::BlockStatement(_) => {
                    steps.push(Step { container: ancestor, statement: self.nodes.kind(child).span(), in_loop });
                    in_loop = false;
                    if ancestor == home {
                        steps.reverse();
                        // `in_loop` belongs to the list inside the loop.
                        return Some((steps, nested));
                    }
                }
                AstKind::ForStatement(_)
                | AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_)
                | AstKind::WhileStatement(_)
                | AstKind::DoWhileStatement(_) => in_loop = true,
                // The variable outlives each call: its declaration stays
                // outside, so the path starts at the statement holding the
                // function.
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Class(_) => {
                    nested = Some(ancestor);
                    steps.clear();
                    in_loop = false;
                }
                AstKind::Program(_) => return None,
                _ => {}
            }
            child = ancestor;
        }
        None
    }

    fn plan(&self, symbol: SymbolId, declarators: &[Declarator]) -> Option<Plan> {
        // Where the variable lives: a `var` in its function's body, a `let`
        // in its own block (inside a loop, one per pass: it stays there).
        let is_var = declarators.iter().all(|d| d.kind == VariableDeclarationKind::Var);
        if !is_var && declarators.len() != 1 {
            return None;
        }
        let home = self.nodes.ancestor_ids(declarators[0].node).find(|ancestor| match self.nodes.kind(*ancestor) {
            AstKind::FunctionBody(_) => true,
            AstKind::BlockStatement(_) => !is_var,
            _ => false,
        })?;
        let mut paths = Vec::new();
        let mut nested = Vec::new();
        for reference in self.scoping.get_resolved_references(symbol) {
            let (path, inside) = self.path(reference.node_id(), home)?;
            nested.extend(inside);
            paths.push(path);
        }
        for declarator in declarators {
            let (path, inside) = self.path(declarator.node, home)?;
            if inside.is_some() {
                return None;
            }
            if declarator.init.is_some() {
                paths.push(path);
            }
        }
        if paths.is_empty() {
            return None;
        }
        // The innermost list all uses share, not past a loop they share.
        let mut depth = 0;
        while paths.iter().all(|path| path.len() > depth && path[depth].container == paths[0][depth].container) {
            depth += 1;
            if paths.iter().any(|path| path[depth - 1].in_loop) {
                break;
            }
        }
        if depth == 0 {
            return None;
        }
        let level = depth - 1;
        let first = paths.iter().map(|path| path[level].statement).min_by_key(|span| span.start)?;
        // A nested use runs once its function can be called: unless that
        // is only after the first statement, the `let` goes first.
        if !nested.iter().all(|function| self.reachable_after(*function, first, home)) {
            let at = self.first_statement(paths[0][level].container)?;
            return Some(Plan { fold: Fold::None(at) });
        }
        let reads_inside = |span: Span| {
            self.scoping.get_resolved_references(symbol).any(|reference| {
                let at = self.nodes.get_node(reference.node_id()).span();
                reference.is_read() && span.start <= at.start && at.end <= span.end
            })
        };
        if declarators.iter().any(|d| d.statement == first && d.init.is_some() && d.all.len() == 1) && !reads_inside(first) {
            return Some(Plan { fold: Fold::Declaration(first) });
        }
        if let Some(value) = self.store_of(first, symbol) {
            if !reads_inside(value) {
                return Some(Plan { fold: Fold::Store(first, value) });
            }
        }
        Some(Plan { fold: Fold::None(first) })
    }

    /// Whether the function `node` (nested in `home`) can only be called
    /// after `statement` ran: an expression evaluated after it, or a
    /// declaration whose name is first read after it (outside any other
    /// function, whose calls could come earlier).
    fn reachable_after(&self, node: NodeId, statement: Span, home: NodeId) -> bool {
        let after = |span: Span| span.start >= statement.end;
        match self.nodes.kind(node) {
            AstKind::Function(function) if function.is_declaration() => {
                let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
                    return false;
                };
                self.scoping.get_resolved_references(symbol).all(|reference| {
                    after(self.nodes.get_node(reference.node_id()).span())
                        && self.path(reference.node_id(), home).is_some_and(|(_, inside)| inside.is_none())
                })
            }
            kind => after(kind.span()),
        }
    }

    fn first_statement(&self, container: NodeId) -> Option<Span> {
        match self.nodes.kind(container) {
            AstKind::FunctionBody(body) => body.statements.first().map(GetSpan::span),
            AstKind::BlockStatement(block) => block.body.first().map(GetSpan::span),
            _ => None,
        }
    }

    /// `name = value;` as the statement `span`.
    fn store_of(&self, span: Span, symbol: SymbolId) -> Option<Span> {
        let mut found = None;
        for reference in self.scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let node = reference.node_id();
            let parent = self.nodes.parent_id(node);
            let AstKind::AssignmentExpression(assignment) = self.nodes.kind(parent) else {
                continue;
            };
            let statement = self.nodes.parent_id(parent);
            if assignment.operator == AssignmentOperator::Assign
                && matches!(self.nodes.kind(statement), AstKind::ExpressionStatement(s) if s.span == span)
            {
                found = Some(assignment.right.span());
            }
        }
        found
    }
}

fn indent_of(source: &str, span: Span) -> &str {
    let start = span.start as usize;
    let line = source[..start].rfind('\n').map_or(0, |at| at + 1);
    let indent = &source[line..start];
    if indent.trim().is_empty() { indent } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(source: &str) -> String {
        place_key_declarations(source).unwrap().0
    }

    #[test]
    fn declares_a_var_at_its_first_store_in_the_block_holding_its_uses() {
        let code = place("function f(a) {\n\tif (a) {\n\t\ta();\n\t\t$KEY_8 = 1;\n\t\tvar $KEY_8;\n\t\ta($KEY_8);\n\t}\n}\n");
        assert_eq!(code, "function f(a) {\n\tif (a) {\n\t\ta();\n\t\tlet $KEY_8 = 1;\n\t\ta($KEY_8);\n\t}\n}\n");
    }

    #[test]
    fn keeps_a_variable_the_loop_carries_outside_the_loop() {
        let code = place("function f(a) {\n\tlet $KEY_1, $KEY_2;\n\ta();\n\tfor (;;) {\n\t\t$KEY_1 = a($KEY_1);\n\t}\n}\n");
        assert_eq!(code, "function f(a) {\n\ta();\n\tlet $KEY_1;\n\tfor (;;) {\n\t\t$KEY_1 = a($KEY_1);\n\t}\n}\n");
    }

    #[test]
    fn declares_first_in_the_block_when_a_function_can_read_it_earlier() {
        // `g` is hoisted and handed out before the store.
        let code = place("function f(a) {\n\tlet $KEY_3;\n\ta(g);\n\t$KEY_3 = 1;\n\tfunction g() {\n\t\treturn $KEY_3;\n\t}\n}\n");
        assert!(code.starts_with("function f(a) {\n\tlet $KEY_3;\n\ta(g);\n\t$KEY_3 = 1;"), "{code}");
        // Handed out only after the store: the store declares it.
        let code = place("function f(a) {\n\tlet $KEY_3;\n\t$KEY_3 = 1;\n\ta(g);\n\tfunction g() {\n\t\treturn $KEY_3;\n\t}\n}\n");
        assert!(code.starts_with("function f(a) {\n\tlet $KEY_3 = 1;\n\ta(g);"), "{code}");
    }

    #[test]
    fn leaves_a_let_inside_a_loop_body_in_place() {
        let source = "function f(a) {\n\tfor (;;) {\n\t\tlet $KEY_4 = a();\n\t\ta(() => $KEY_4);\n\t}\n}\n";
        assert_eq!(place(source), source);
    }
    #[test]
    fn never_moves_a_declaration_into_the_function_using_it() {
        let source = "function f() {\n\tlet $KEY_8;\n\tfunction g() {\n\t\t$KEY_8 = h($KEY_8);\n\t}\n\treturn g;\n}\n";
        assert_eq!(place(source), "function f() {\n\tlet $KEY_8;\n\tfunction g() {\n\t\t$KEY_8 = h($KEY_8);\n\t}\n\treturn g;\n}\n");
        let source = "function f() {\n\tlet $KEY_8;\n\tif (a) {\n\t\tb();\n\t\tlet g = () => $KEY_8;\n\t\tg();\n\t}\n}\n";
        assert_eq!(place(source), "function f() {\n\tif (a) {\n\t\tlet $KEY_8;\n\t\tb();\n\t\tlet g = () => $KEY_8;\n\t\tg();\n\t}\n}\n");
    }
}
