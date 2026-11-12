//! Move the statements every branch ends with out after the branches.
//!
//! The VM spells the same tail differently per arm: one arm ends
//! `$E = a; ++$C; $D = 128;`, another `++$C; $E = a; $D = $D & 63;`. Two
//! statements depend on each other when one writes what the other reads or
//! writes (a variable, or the heap), when a call can reach what the other
//! touches, or when both can throw. Statements that depend on nothing after
//! them in their arm can run last in it, so a statement every arm of an
//! `if`/`else if`/`else` chain has (same text, same bindings, declared
//! outside the chain) moves after the chain:
//!
//! ```js
//! if (d < 128) { e = a; ++c; } else { o[++c] = x; ++c; e = a; d = 9; }
//! // →
//! if (d < 128) {} else { o[++c] = x; d = 9; }
//! e = a;
//! ++c;
//! ```
//!
//! The moved statements keep the first arm's order; two that other arms
//! have the other way round must not depend on each other. A statement
//! that can throw moves past one that writes only locals, and only when no
//! `try` in the function can observe the difference. Arms that jump
//! (`break`, `return`, …) after the statement keep it. Inner chains go
//! first; the pass repeats until nothing moves.

use std::collections::{HashMap, HashSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn hoist_branch_tails(source: &str) -> Result<(String, usize), String> {
    let mut code = source.to_string();
    let mut total = 0;
    for _ in 0..8 {
        let (next, count) = round(&code)?;
        if count == 0 {
            break;
        }
        code = next;
        total += count;
    }
    Ok((code, total))
}

fn round(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    // The heads of `if`/`else if`/`else` chains, not the `else if`s.
    let ifs: Vec<(NodeId, &IfStatement<'_>)> = nodes
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::IfStatement(statement) => Some((node.id(), statement)),
            _ => None,
        })
        .collect();
    let alternates: HashSet<Span> = ifs
        .iter()
        .filter_map(|(_, statement)| match &statement.alternate {
            Some(Statement::IfStatement(alternate)) => Some(alternate.span),
            _ => None,
        })
        .collect();
    let scan = Scan { source, scoping: &scoping, nodes: &nodes, captured: HashMap::default().into() };
    let mut planned: Vec<(Span, Vec<analysis::Edit>)> = Vec::new();
    for &(node, chain) in &ifs {
        if alternates.contains(&chain.span) {
            continue;
        }
        if let Some(edits) = scan.chain(node, chain) {
            planned.push((chain.span, edits));
        }
    }
    // Innermost first: a chain holding another planned one waits a round.
    let edits: Vec<_> = planned
        .iter()
        .filter(|(outer, _)| !planned.iter().any(|(inner, _)| inner != outer && outer.start <= inner.start && inner.end <= outer.end))
        .collect();
    let count = edits.len();
    if count == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, edits.into_iter().flat_map(|(_, edits)| edits.iter().cloned()).collect(), "branch tail")?;
    Ok((output, count))
}

/// What a statement reads, writes and can do.
#[derive(Default, Debug)]
struct Effects {
    reads: HashSet<SymbolId>,
    writes: HashSet<SymbolId>,
    heap_read: bool,
    heap_write: bool,
    /// This function's `arguments` object, apart from the heap: only a
    /// store into it or handing it on changes it.
    arguments_read: bool,
    arguments_write: bool,
    calls: bool,
    throws: bool,
    /// Jumps, nested functions, `yield`/`await`: it stays where it is.
    fixed: bool,
}

struct EffectScan<'s, 'a> {
    scoping: &'s Scoping,
    effects: Effects,
    /// The identifier being visited is a member's object.
    member_object: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

fn is_arguments(expression: &Expression<'_>) -> bool {
    matches!(expression.get_inner_expression(), Expression::Identifier(identifier) if identifier.name == "arguments")
}

/// A standard builtin that only computes from primitive operands.
fn pure_builtin(callee: &Expression<'_>) -> bool {
    match callee.get_inner_expression() {
        Expression::Identifier(identifier) => {
            matches!(identifier.name.as_str(), "Number" | "String" | "Boolean" | "parseInt" | "parseFloat" | "isNaN" | "isFinite")
        }
        Expression::StaticMemberExpression(member) => matches!(&member.object, Expression::Identifier(object) if object.name == "Math"),
        Expression::ComputedMemberExpression(member) => {
            matches!(&member.object, Expression::Identifier(object) if object.name == "Math")
                || matches!((&member.object, &member.expression), (Expression::Identifier(object), Expression::StringLiteral(name)) if object.name == "String" && name.value == "fromCharCode")
        }
        _ => false,
    }
}

impl<'a> EffectScan<'_, 'a> {
    /// A member read: of the `arguments` object, or of the heap.
    fn member(&mut self, object: &Expression<'a>) {
        if is_arguments(object) {
            self.effects.arguments_read = true;
            self.member_object = true;
            self.visit_expression(object);
            self.member_object = false;
        } else {
            self.effects.heap_read = true;
            self.effects.throws = true;
            self.visit_expression(object);
        }
    }
}

impl<'a> Visit<'a> for EffectScan<'_, 'a> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        let reference = self.scoping.get_reference(identifier.reference_id());
        match reference.symbol_id() {
            Some(symbol) => {
                if reference.is_read() {
                    self.effects.reads.insert(symbol);
                }
                if reference.is_write() {
                    self.effects.writes.insert(symbol);
                }
            }
            None => {
                let name = identifier.name.as_str();
                if name == "arguments" && !self.member_object {
                    self.effects.arguments_read = true;
                    self.effects.arguments_write = true;
                }
                if reference.is_write() {
                    self.effects.heap_write = true;
                }
                if !matches!(name, "arguments" | "undefined" | "NaN" | "Infinity") {
                    self.effects.heap_read = true;
                    if !analysis::is_native_global(name) {
                        self.effects.throws = true;
                    }
                }
            }
        }
    }
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        self.member(&member.object);
        self.visit_expression(&member.expression);
    }
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.member(&member.object);
    }
    fn visit_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'a>) {
        if let Some(member) = target.as_member_expression() {
            if is_arguments(member.object()) {
                self.effects.arguments_write = true;
            } else {
                self.effects.heap_write = true;
                self.effects.throws = true;
            }
        }
        walk::walk_simple_assignment_target(self, target);
    }
    fn visit_assignment_target_pattern(&mut self, pattern: &AssignmentTargetPattern<'a>) {
        self.effects.fixed = true;
        walk::walk_assignment_target_pattern(self, pattern);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if pure_builtin(&call.callee) {
            for argument in &call.arguments {
                self.visit_argument(argument);
            }
            return;
        }
        self.effects.calls = true;
        self.effects.heap_read = true;
        self.effects.heap_write = true;
        self.effects.throws = true;
        walk::walk_call_expression(self, call);
    }
    fn visit_new_expression(&mut self, call: &NewExpression<'a>) {
        self.effects.calls = true;
        self.effects.heap_read = true;
        self.effects.heap_write = true;
        self.effects.throws = true;
        walk::walk_new_expression(self, call);
    }
    fn visit_tagged_template_expression(&mut self, tagged: &TaggedTemplateExpression<'a>) {
        self.effects.fixed = true;
        walk::walk_tagged_template_expression(self, tagged);
    }
    fn visit_binary_expression(&mut self, binary: &BinaryExpression<'a>) {
        if matches!(binary.operator, BinaryOperator::In | BinaryOperator::Instanceof) {
            self.effects.throws = true;
            self.effects.heap_read = true;
        }
        walk::walk_binary_expression(self, binary);
    }
    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::Delete {
            self.effects.heap_write = true;
            self.effects.throws = true;
        }
        walk::walk_unary_expression(self, unary);
    }
    fn visit_function(&mut self, _function: &Function<'a>, _flags: oxc_syntax::scope::ScopeFlags) {
        self.effects.fixed = true;
    }
    fn visit_arrow_function_expression(&mut self, _arrow: &ArrowFunctionExpression<'a>) {
        self.effects.fixed = true;
    }
    fn visit_class(&mut self, _class: &Class<'a>) {
        self.effects.fixed = true;
    }
    fn visit_yield_expression(&mut self, _expression: &YieldExpression<'a>) {
        self.effects.fixed = true;
    }
    fn visit_await_expression(&mut self, _expression: &AwaitExpression<'a>) {
        self.effects.fixed = true;
    }
    fn visit_break_statement(&mut self, _statement: &BreakStatement<'a>) {
        self.effects.fixed = true;
    }
    fn visit_continue_statement(&mut self, _statement: &ContinueStatement<'a>) {
        self.effects.fixed = true;
    }
    fn visit_return_statement(&mut self, _statement: &ReturnStatement<'a>) {
        self.effects.fixed = true;
    }
    fn visit_throw_statement(&mut self, _statement: &ThrowStatement<'a>) {
        self.effects.fixed = true;
    }
    fn visit_try_statement(&mut self, _statement: &TryStatement<'a>) {
        self.effects.fixed = true;
    }
    fn visit_labeled_statement(&mut self, _statement: &LabeledStatement<'a>) {
        self.effects.fixed = true;
    }
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    captured: std::cell::RefCell<HashMap<SymbolId, bool>>,
}

fn text(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

impl<'a> Scan<'_, 'a> {
    fn effects(&self, statement: &Statement<'a>) -> Effects {
        let mut scan = EffectScan { scoping: self.scoping, effects: Effects::default(), member_object: false, _marker: std::marker::PhantomData };
        scan.visit_statement(statement);
        scan.effects
    }

    fn function_of(&self, mut scope: oxc_semantic::ScopeId) -> oxc_semantic::ScopeId {
        loop {
            if self.scoping.scope_flags(scope).is_function() {
                return scope;
            }
            match self.scoping.scope_parent_id(scope) {
                Some(parent) => scope = parent,
                None => return scope,
            }
        }
    }

    /// Read or written from another function than its own: a call can
    /// reach it.
    fn captured(&self, symbol: SymbolId) -> bool {
        if let Some(&known) = self.captured.borrow().get(&symbol) {
            return known;
        }
        let home = self.function_of(self.scoping.symbol_scope_id(symbol));
        let captured = self.scoping.symbol_name(symbol).starts_with("$KEY_")
            || self
                .scoping
                .get_resolved_references(symbol)
                .any(|reference| self.function_of(self.nodes.get_node(reference.node_id()).scope_id()) != home);
        self.captured.borrow_mut().insert(symbol, captured);
        captured
    }

    /// Only locals no call reaches: invisible once an exception leaves the
    /// function.
    fn local_only(&self, effects: &Effects) -> bool {
        !effects.heap_write && !effects.arguments_write && !effects.calls && effects.writes.iter().all(|&symbol| !self.captured(symbol))
    }

    fn depends(&self, a: &Effects, b: &Effects, guarded: bool) -> bool {
        if a.fixed || b.fixed {
            return true;
        }
        let touches = |e: &Effects, symbols: &HashSet<SymbolId>| e.reads.iter().chain(&e.writes).any(|s| symbols.contains(s));
        if touches(b, &a.writes) || touches(a, &b.writes) {
            return true;
        }
        if (a.heap_write && (b.heap_read || b.heap_write)) || (b.heap_write && a.heap_read) {
            return true;
        }
        if (a.arguments_write && (b.arguments_read || b.arguments_write)) || (b.arguments_write && a.arguments_read) {
            return true;
        }
        let reaches = |call: &Effects, other: &Effects| call.calls && other.reads.iter().chain(&other.writes).any(|&s| self.captured(s));
        if reaches(a, b) || reaches(b, a) {
            return true;
        }
        if a.throws || b.throws {
            if a.throws && b.throws || guarded {
                return true;
            }
            let quiet = if a.throws { b } else { a };
            if !self.local_only(quiet) {
                return true;
            }
        }
        false
    }

    /// Inside a `try` (or `catch`/`finally`) of its function: an exception
    /// can be seen there.
    fn guarded(&self, node: NodeId) -> bool {
        for ancestor in self.nodes.ancestor_ids(node) {
            match self.nodes.kind(ancestor) {
                AstKind::TryStatement(_) => return true,
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return false,
                _ => {}
            }
        }
        false
    }

    /// Every reference resolves outside `chain` (or to a global).
    fn bound_outside(&self, statement: &Statement<'a>, chain: Span) -> bool {
        let mut names = Names { references: Vec::new() };
        names.visit_statement(statement);
        names.references.iter().all(|&reference| match self.scoping.get_reference(reference).symbol_id() {
            Some(symbol) => {
                let declared = self.scoping.symbol_span(symbol);
                declared.end <= chain.start || declared.start >= chain.end
            }
            None => true,
        })
    }

    fn chain(&self, node: NodeId, head: &IfStatement<'a>) -> Option<Vec<analysis::Edit>> {
        // The arms: each `if`'s block, then the final `else` block.
        let mut arms: Vec<&[Statement<'a>]> = Vec::new();
        let mut at = head;
        loop {
            let Statement::BlockStatement(block) = &at.consequent else {
                return None;
            };
            arms.push(&block.body);
            match at.alternate.as_ref()? {
                Statement::IfStatement(next) => at = next,
                Statement::BlockStatement(block) => {
                    arms.push(&block.body);
                    break;
                }
                _ => return None,
            }
        }
        if arms.iter().any(|arm| arm.is_empty()) {
            return None;
        }
        let guarded = self.guarded(node);
        let effects: Vec<Vec<Effects>> = arms.iter().map(|arm| arm.iter().map(|s| self.effects(s)).collect()).collect();
        // Candidates: the first arm's expression statements every arm has,
        // matched as the same occurrence counted from the end.
        let rank_from_end = |arm: &[Statement<'a>], index: usize| {
            let wanted = text(self.source, arm[index].span());
            arm[index + 1..].iter().filter(|s| text(self.source, s.span()) == wanted).count()
        };
        let find = |arm: &[Statement<'a>], wanted: &str, rank: usize| {
            arm.iter()
                .enumerate()
                .rev()
                .filter(|(_, s)| text(self.source, s.span()) == wanted)
                .nth(rank)
                .map(|(index, _)| index)
        };
        let mut moved: Vec<Vec<usize>> = Vec::new();
        for (index, statement) in arms[0].iter().enumerate() {
            if !matches!(statement, Statement::ExpressionStatement(_)) || effects[0][index].fixed || !self.bound_outside(statement, head.span) {
                continue;
            }
            let wanted = text(self.source, statement.span());
            let rank = rank_from_end(arms[0], index);
            let places: Option<Vec<usize>> = arms.iter().map(|arm| find(arm, wanted, rank)).collect();
            if let Some(places) = places {
                if places.iter().zip(&arms).all(|(&place, arm)| self.bound_outside(&arm[place], head.span)) {
                    moved.push(places);
                }
            }
        }
        // Drop those that depend on something staying after them, until
        // what is left can all run last.
        loop {
            let before = moved.len();
            let kept: Vec<Vec<usize>> = moved
                .iter()
                .filter(|places| {
                    places.iter().enumerate().all(|(arm, &place)| {
                        (place + 1..arms[arm].len()).all(|later| {
                            moved.iter().any(|other| other[arm] == later) || !self.depends(&effects[arm][place], &effects[arm][later], guarded)
                        })
                    })
                })
                .cloned()
                .collect();
            // Two moved statements in another order than the first arm's
            // must not depend on each other.
            let mut kept = kept;
            'order: loop {
                for (i, first) in kept.iter().enumerate() {
                    for second in &kept[i + 1..] {
                        for arm in 1..arms.len() {
                            if first[arm] > second[arm] && self.depends(&effects[arm][first[arm]], &effects[arm][second[arm]], guarded) {
                                kept.remove(i);
                                continue 'order;
                            }
                        }
                    }
                }
                break;
            }
            moved = kept;
            if moved.len() == before {
                break;
            }
        }
        if moved.is_empty() {
            return None;
        }
        let line_start = self.source[..head.span.start as usize].rfind('\n').map_or(0, |at| at + 1);
        let indent = &self.source[line_start..head.span.start as usize];
        if !indent.trim().is_empty() {
            return None;
        }
        let mut edits = Vec::new();
        let mut tail = String::new();
        for places in &moved {
            tail.push('\n');
            tail.push_str(indent);
            tail.push_str(text(self.source, arms[0][places[0]].span()));
            for (arm, &place) in places.iter().enumerate() {
                edits.push(analysis::Edit::delete(whole_line(self.source, arms[arm][place].span())));
            }
        }
        edits.push(analysis::Edit::replace(Span::new(head.span.end, head.span.end), tail));
        Some(edits)
    }
}

struct Names {
    references: Vec<oxc_semantic::ReferenceId>,
}

impl<'a> Visit<'a> for Names {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.references.push(identifier.reference_id());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hoist(source: &str) -> (String, usize) {
        hoist_branch_tails(source).unwrap()
    }

    #[test]
    fn moves_a_shared_tail_in_any_order_after_the_chain() {
        let source = "function f(a, o, x) {\n\tlet c = 0, d = 1, e;\n\tif (d === 0) {\n\t\to[++c] = x;\n\t\te = a;\n\t\t++c;\n\t\td = 128;\n\t} else if (d < 128) {\n\t\te = a;\n\t\t++c;\n\t} else {\n\t\to[++c] = x;\n\t\t++c;\n\t\te = a;\n\t\td = d & 63;\n\t}\n\te[c] = d;\n}\n";
        let (code, count) = hoist(source);
        assert_eq!(count, 1);
        assert_eq!(code, "function f(a, o, x) {\n\tlet c = 0, d = 1, e;\n\tif (d === 0) {\n\t\to[++c] = x;\n\t\td = 128;\n\t} else if (d < 128) {\n\t} else {\n\t\to[++c] = x;\n\t\td = d & 63;\n\t}\n\te = a;\n\t++c;\n\te[c] = d;\n}\n");
    }

    #[test]
    fn inner_chains_first() {
        let source = "function f(a, b) {\n\tlet c = 0, e;\n\tif (a) {\n\t\te = b;\n\t\t++c;\n\t} else {\n\t\tif (b) {\n\t\t\t++c;\n\t\t\te = b;\n\t\t} else {\n\t\t\te = b;\n\t\t\t++c;\n\t\t}\n\t}\n\treturn e[c];\n}\n";
        let (code, count) = hoist(source);
        assert_eq!(count, 2);
        assert_eq!(code, "function f(a, b) {\n\tlet c = 0, e;\n\tif (a) {\n\t} else {\n\t\tif (b) {\n\t\t} else {\n\t\t}\n\t}\n\te = b;\n\t++c;\n\treturn e[c];\n}\n");
    }

    #[test]
    fn keeps_what_later_statements_depend_on() {
        for source in [
            // c is read after the increment in one arm
            "function f(o) {\n\tlet c = 0;\n\tif (o) {\n\t\t++c;\n\t\to[c] = 1;\n\t} else {\n\t\t++c;\n\t}\n}\n",
            // no else: the statement does not always run
            "function f(o) {\n\tlet c = 0;\n\tif (o) {\n\t\t++c;\n\t}\n}\n",
            // a call may read the captured variable
            "function f(o) {\n\tlet c = 0;\n\tlet g = () => c;\n\tif (o) {\n\t\t++c;\n\t\tg();\n\t} else {\n\t\t++c;\n\t}\n}\n",
            // a jump after it
            "function f(o) {\n\tlet c = 0;\n\tfor (;;) {\n\t\tif (o) {\n\t\t\t++c;\n\t\t\tbreak;\n\t\t} else {\n\t\t\t++c;\n\t\t}\n\t}\n}\n",
            // two throwing statements
            "function f(o, p) {\n\tif (o) {\n\t\to.x = 1;\n\t\tp.y = 2;\n\t} else {\n\t\to.x = 1;\n\t}\n}\n",
            // a branch-local binding
            "function f(o) {\n\tif (o) {\n\t\tlet c = 1;\n\t\to[c] = 1;\n\t} else {\n\t\tlet c = 1;\n\t\to[c] = 1;\n\t}\n}\n",
        ] {
            let (_, count) = hoist(source);
            assert_eq!(count, 0, "{source}");
        }
    }

    #[test]
    fn a_throwing_statement_passes_local_writes_outside_try_only() {
        let source = "function f(o, d) {\n\tif (d) {\n\t\to.x = 1;\n\t\td = 2;\n\t} else {\n\t\to.x = 1;\n\t}\n\treturn d;\n}\n";
        let (code, count) = hoist(source);
        assert_eq!(count, 1);
        assert_eq!(code, "function f(o, d) {\n\tif (d) {\n\t\td = 2;\n\t} else {\n\t}\n\to.x = 1;\n\treturn d;\n}\n");
        let guarded = "function f(o, d) {\n\ttry {\n\t\tif (d) {\n\t\t\to.x = 1;\n\t\t\td = 2;\n\t\t} else {\n\t\t\to.x = 1;\n\t\t}\n\t} catch (e) {}\n\treturn d;\n}\n";
        assert_eq!(hoist(guarded).1, 0);
    }
}
