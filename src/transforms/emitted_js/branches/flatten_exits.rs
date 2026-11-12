//! Flatten `else` after an arm that always leaves, and sink bare declarations
//! into their first store.
//!
//! The VM's branches nest every continuation inside the arm that did not
//! leave:
//!
//! ```js
//! if (t) { …; return x; } else { REST }   // -> if (t) { …; return x; } REST
//! if (t) { REST } else { …; throw e; }    // -> if (!(t)) { …; throw e; } REST
//! ```
//!
//! An arm *always leaves* when its last statement is `return`, `throw`,
//! `break` or `continue`. The continuation then runs exactly when the other
//! arm was taken, so splicing that arm after the `if` changes nothing. The
//! spliced arm's `let`/`const` bindings move to the enclosing block, so the
//! rewrite declines when one is not a generated `$` name (which could clash
//! with a later binding of the same name there) or when the arm declares a
//! function or class (hoisting differs).
//!
//! A bare block in a statement list is spliced into it under the same rule.
//!
//! A bare `let $X;` whose next reference is a plain store `$X = v;` later in
//! the same statement list becomes `let $X = v;` there: nothing observed the
//! uninitialized binding in between. Later stores keep `$X` mutable.
//!
//! A carry that every leaf of a complete `if`/`else` tree sets to the same
//! settled value (a literal, a stable global or built-in member, `this`, an
//! unwritten binding declared before the tree, or a fixed `arguments[n]`)
//! as part of the leaf's tail, followed only by more settled stores, is set
//! once after the tree instead:
//!
//! ```js
//! if (t) { g(); $J = arguments[1]; } else { $J = arguments[1]; }
//! // -> if (t) { g(); } else { }  $J = arguments[1];
//! ```
//!
//! The value cannot change and storing it cannot throw, so the store only
//! moves past settled stores to the end of the tree. It requires every write
//! of the carry to be one of those stores, no other reference inside the
//! tree or in another function, and no `try` around the tree in its function
//! (a throw after the store would otherwise reach a handler that sees it).

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;

use crate::analysis;
use super::super::support::whole_line;
use crate::transforms::emitted_js::support::indent_before;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlattenExitsResult {
    pub code: String,
    pub flattened: usize,
    pub sunk: usize,
    pub hoisted: usize,
}

pub(crate) fn flatten_exits(source: &str) -> Result<FlattenExitsResult, String> {
    // Inner rewrites covered by an outer one wait for the next round: go
    // around until nothing changes, so callers see one settled result.
    let mut result = flatten_exits_once(source)?;
    for _ in 0..64 {
        if result.code == source || result.flattened + result.sunk + result.hoisted == 0 {
            break;
        }
        let next = flatten_exits_once(&result.code)?;
        if next.flattened + next.sunk + next.hoisted == 0 {
            break;
        }
        result = FlattenExitsResult {
            code: next.code,
            flattened: result.flattened + next.flattened,
            sunk: result.sunk + next.sunk,
            hoisted: result.hoisted + next.hoisted,
        };
    }
    Ok(result)
}

fn flatten_exits_once(source: &str) -> Result<FlattenExitsResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let comments: Vec<u32> = program.comments.iter().map(|comment| comment.span.start).collect();
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        comments: &comments,
        edits: Vec::new(),
        flattened: 0,
        sunk: 0,
        hoisted: 0,
        dynamic: false,
        functions: Vec::new(),
    };
    scan.visit_program(&program);
    if scan.dynamic || scan.edits.is_empty() {
        return Ok(FlattenExitsResult {
            code: source.into(),
            flattened: 0,
            sunk: 0,
            hoisted: 0,
        });
    }
    // Inner rewrites wait for the next round when an outer one covers them.
    let mut edits = scan.edits;
    edits.sort_by_key(|edit| (edit.start, std::cmp::Reverse(edit.end)));
    let mut kept: Vec<analysis::Edit> = Vec::new();
    for edit in edits {
        if kept.iter().any(|other| edit.start < other.end && other.start < edit.end) {
            continue;
        }
        kept.push(edit);
    }
    let applied = analysis::apply_edits(source, kept);
    if !applied.rejected.is_empty() {
        return Err("exit flattening edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("exit flattening output: {error}"))?;
    Ok(FlattenExitsResult {
        code: applied.output,
        flattened: scan.flattened,
        sunk: scan.sunk,
        hoisted: scan.hoisted,
    })
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    comments: &'s [u32],
    edits: Vec<analysis::Edit>,
    flattened: usize,
    sunk: usize,
    hoisted: usize,
    dynamic: bool,
    /// Enclosing functions: span, `arguments` never changes, `try` depth.
    functions: Vec<(Span, bool, u32)>,
}

impl Scan<'_, '_> {
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    fn has_comment(&self, span: Span) -> bool {
        let at = self.comments.partition_point(|start| *start < span.start);
        self.comments.get(at).is_some_and(|start| *start < span.end)
    }

    fn list(&mut self, statements: &[Statement<'_>]) {
        for (position, statement) in statements.iter().enumerate() {
            match statement {
                Statement::IfStatement(branch) => {
                    let before = self.edits.len();
                    self.flatten(branch);
                    if self.edits.len() == before {
                        self.hoist_common_tails(branch);
                    }
                    if self.edits.len() == before {
                        self.store_test_first(branch);
                    }
                }
                Statement::VariableDeclaration(declaration) => {
                    let before = self.edits.len();
                    if position > 0 {
                        if let Statement::IfStatement(branch) = &statements[position - 1] {
                            self.retarget_join(branch, statement);
                        }
                    }
                    if self.edits.len() == before {
                        self.sink(declaration, &statements[position + 1..]);
                    }
                }
                Statement::ExpressionStatement(_) if position > 0 => {
                    if let Statement::IfStatement(branch) = &statements[position - 1] {
                        self.retarget_join(branch, statement);
                    }
                }
                Statement::BlockStatement(block) => self.unwrap_block(block),
                Statement::ReturnStatement(ret) if position > 0 => {
                    if let Statement::IfStatement(branch) = &statements[position - 1] {
                        self.sink_return(branch, ret);
                    }
                }
                _ => {}
            }
        }
    }

    fn flatten(&mut self, branch: &IfStatement<'_>) {
        let (Statement::BlockStatement(yes), Some(Statement::BlockStatement(no))) =
            (&branch.consequent, &branch.alternate)
        else {
            return;
        };
        if self.has_comment(branch.span) {
            return;
        }
        let (kept, rest, test) = if leaves(&yes.body) && !leaves(&no.body) {
            (yes, no, format!("if ({}) ", self.text(branch.test.span())))
        } else if leaves(&no.body) && !leaves(&yes.body) {
            (no, yes, format!("if (!({})) ", self.text(branch.test.span())))
        } else {
            return;
        };
        if !self.splice_safe(&rest.body) {
            return;
        }
        let indent = indent_before(self.source, branch.span.start);
        let mut out = format!("{test}{}", self.text(kept.span));
        for statement in &rest.body {
            out.push('\n');
            out.push_str(&indent);
            out.push_str(self.text(statement.span()));
        }
        self.edits.push(analysis::Edit::replace(branch.span, out));
        self.flattened += 1;
    }

    /// Carries every leaf of `branch` sets to one settled value: rewritten as
    /// one replacement of the whole tree, so the rewrite stays atomic.
    fn hoist_common_tails(&mut self, branch: &IfStatement<'_>) {
        if self.has_comment(branch.span) || self.functions.last().is_some_and(|function| function.2 > 0) {
            return;
        }
        let mut leaves = Vec::new();
        if !self.tree_tails(branch, &mut leaves) || leaves.len() < 2 {
            return;
        }
        // Candidates: the first leaf's tail stores.
        let mut removed: Vec<Span> = Vec::new();
        let mut appended: Vec<String> = Vec::new();
        for (symbol, _, value) in &leaves[0] {
            let value_text = self.text(*value);
            let writes: Vec<Span> = leaves
                .iter()
                .filter_map(|tail| {
                    tail.iter()
                        .rev()
                        .find(|(other, _, _)| other == symbol)
                        .filter(|(_, _, other)| self.text(*other) == value_text)
                        .map(|(_, statement, _)| *statement)
                })
                .collect();
            if writes.len() != leaves.len() || !self.only_these_writes(*symbol, &writes, branch.span) {
                continue;
            }
            removed.extend(writes);
            let name = self.scoping.symbol_name(*symbol);
            appended.push(format!("{name} = {value_text};"));
        }
        if appended.is_empty() {
            return;
        }
        removed.sort_by_key(|span| span.start);
        let mut out = String::new();
        let mut at = branch.span.start;
        for span in removed {
            let span = whole_line(self.source, span);
            out.push_str(&self.source[at as usize..span.start as usize]);
            at = span.end;
        }
        out.push_str(&self.source[at as usize..branch.span.end as usize]);
        let indent = indent_before(self.source, branch.span.start);
        for store in &appended {
            out.push('\n');
            out.push_str(&indent);
            out.push_str(store);
        }
        self.hoisted += appended.len();
        self.edits.push(analysis::Edit::replace(branch.span, out));
    }

    /// `if (t) { B; $J = x; } else { $J = t; }` stores the test first:
    /// `$J = t; if ($J) { B; $J = x; }` (and `if (!t) { $J = t; } else { … }`
    /// alike). `t` is a name the arm cannot change before the test, `$J` is
    /// read nowhere in the `if` and by no closure, and `B` ends storing `$J`
    /// with no `break`/`continue` before it, so every path that goes on
    /// overwrites the early store. No `try` may surround it.
    fn store_test_first(&mut self, branch: &IfStatement<'_>) {
        let Some(alternate) = &branch.alternate else {
            return;
        };
        // A throw in `B` could reach a handler that sees the early store.
        if self.has_comment(branch.span) || self.functions.last().is_some_and(|function| function.2 > 0) {
            return;
        }
        let (name, negated) = match &branch.test {
            Expression::Identifier(test) => (test, false),
            Expression::UnaryExpression(unary) if unary.operator == oxc_syntax::operator::UnaryOperator::LogicalNot => {
                let Expression::Identifier(test) = &unary.argument else {
                    return;
                };
                (test, true)
            }
            _ => return,
        };
        let (store_arm, other) = if negated { (&branch.consequent, alternate) } else { (alternate, &branch.consequent) };
        let Statement::BlockStatement(store_block) = store_arm else {
            return;
        };
        let Statement::BlockStatement(other_block) = other else {
            return;
        };
        let [store] = store_block.body.as_slice() else {
            return;
        };
        let Some(test_symbol) = analysis::identifier_symbol(name, self.scoping) else {
            return;
        };
        let Some((carry, value)) = self.plain_target(store) else {
            return;
        };
        let Expression::Identifier(value) = value else {
            return;
        };
        if analysis::identifier_symbol(value, self.scoping) != Some(test_symbol)
            || self.scoping.symbol_is_mutated(test_symbol)
            || carry == test_symbol
        {
            return;
        }
        // `B` ends storing the carry and has no jump that could skip it.
        if !other_block.body.last().is_some_and(|last| self.plain_target(last).is_some_and(|(target, _)| target == carry))
            || self.text(other_block.span).contains("break")
            || self.text(other_block.span).contains("continue")
        {
            return;
        }
        let owner = self.functions.last().map(|function| function.0);
        let carry_ok = self.scoping.get_resolved_references(carry).all(|reference| {
            let span = self.nodes.kind(reference.node_id()).span();
            let inside = branch.span.start <= span.start && span.end <= branch.span.end;
            let function = self
                .nodes
                .ancestor_ids(reference.node_id())
                .find(|id| matches!(self.nodes.kind(*id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
                .map(|id| self.nodes.kind(id).span());
            function == owner && !(inside && reference.is_read())
        });
        if !carry_ok || !self.scoping.symbol_name(carry).starts_with('$') {
            return;
        }
        let carry_name = self.scoping.symbol_name(carry);
        let indent = indent_before(self.source, branch.span.start);
        self.edits.push(analysis::Edit::replace(
            branch.span,
            format!(
                "{carry_name} = {};\n{indent}if ({carry_name}) {}",
                self.text(name.span),
                self.text(other_block.span)
            ),
        ));
        self.hoisted += 1;
    }

    /// `if (…) { …; $J = a; } else { $J = b; } return $J;` returns from each
    /// leaf instead: `if (…) { …; return a; } else { return b; }`. Every
    /// leaf of the complete tree ends storing `$J` or leaves, `$J` has no
    /// other read and no closure reference, so the stored value is exactly
    /// what the `return` would have returned.
    fn sink_return(&mut self, branch: &IfStatement<'_>, ret: &ReturnStatement<'_>) {
        let Some(Expression::Identifier(returned)) = &ret.argument else {
            return;
        };
        let Some(carry) = analysis::identifier_symbol(returned, self.scoping) else {
            return;
        };
        if self.has_comment(Span::new(branch.span.start, ret.span.end)) || !returned.name.starts_with('$') {
            return;
        }
        let mut stores: Vec<(Span, Span)> = Vec::new();
        if !self.tree_stores(branch, carry, &mut stores) || stores.is_empty() {
            return;
        }
        if !self.only_read_at(carry, &stores, returned.span) {
            return;
        }
        let mut out = String::new();
        let mut at = branch.span.start;
        for (statement, value) in &stores {
            out.push_str(&self.source[at as usize..statement.start as usize]);
            out.push_str(&format!("return {};", self.text(*value)));
            at = statement.end;
        }
        out.push_str(&self.source[at as usize..branch.span.end as usize]);
        self.edits.push(analysis::Edit::replace(Span::new(branch.span.start, ret.span.end), out));
        self.flattened += 1;
    }

    /// `if (…) { …; $J = a; } else { $J = b; } K = $J;` stores into `K` in
    /// each leaf instead. The stores end their leaves, so `K` changes at
    /// the same point; `$J` has no other read and no closure reference. A
    /// `let K = $J;` becomes `let K;` before the tree, which must not name
    /// `K` (it would have met its TDZ).
    fn retarget_join(&mut self, branch: &IfStatement<'_>, copy: &Statement<'_>) {
        let (target_text, joined, declaration) = match copy {
            Statement::VariableDeclaration(declaration) => {
                let [declarator] = declaration.declarations.as_slice() else {
                    return;
                };
                let (BindingPattern::BindingIdentifier(binding), Some(Expression::Identifier(joined))) =
                    (&declarator.id, &declarator.init)
                else {
                    return;
                };
                let Some(symbol) = binding.symbol_id.get() else {
                    return;
                };
                let inside = self.scoping.get_resolved_references(symbol).any(|reference| {
                    let span = self.nodes.kind(reference.node_id()).span();
                    branch.span.start <= span.start && span.end <= branch.span.end
                });
                if inside {
                    return;
                }
                let kind = if declaration.kind == VariableDeclarationKind::Var { "var" } else { "let" };
                (self.text(binding.span).to_string(), joined, Some(kind))
            }
            Statement::ExpressionStatement(expression) => {
                let Expression::AssignmentExpression(assignment) = &expression.expression else {
                    return;
                };
                let (AssignmentTarget::AssignmentTargetIdentifier(target), Expression::Identifier(joined)) =
                    (&assignment.left, &assignment.right)
                else {
                    return;
                };
                if assignment.operator != AssignmentOperator::Assign {
                    return;
                }
                (self.text(target.span).to_string(), joined, None)
            }
            _ => return,
        };
        let Some(carry) = analysis::identifier_symbol(joined, self.scoping) else {
            return;
        };
        if !joined.name.starts_with('$') || self.has_comment(Span::new(branch.span.start, copy.span().end)) {
            return;
        }
        let mut stores: Vec<(Span, Span)> = Vec::new();
        if !self.tree_stores(branch, carry, &mut stores) || stores.is_empty() || !self.only_read_at(carry, &stores, joined.span) {
            return;
        }
        let indent = indent_before(self.source, branch.span.start);
        let mut out = declaration.map_or(String::new(), |kind| format!("{kind} {target_text};\n{indent}"));
        let mut at = branch.span.start;
        for (statement, value) in &stores {
            out.push_str(&self.source[at as usize..statement.start as usize]);
            out.push_str(&format!("{target_text} = {};", self.text(*value)));
            at = statement.end;
        }
        out.push_str(&self.source[at as usize..branch.span.end as usize]);
        self.edits.push(analysis::Edit::replace(Span::new(branch.span.start, copy.span().end), out));
        self.flattened += 1;
    }

    /// Every write of `carry` is one of `stores`, its one read is at `read`,
    /// and no closure names it.
    fn only_read_at(&self, carry: SymbolId, stores: &[(Span, Span)], read: Span) -> bool {
        let owner = self.functions.last().map(|function| function.0);
        let writes = self.scoping.get_resolved_references(carry).filter(|reference| reference.is_write()).count();
        writes == stores.len()
            && self.scoping.get_resolved_references(carry).all(|reference| {
                let span = self.nodes.kind(reference.node_id()).span();
                let function = self
                    .nodes
                    .ancestor_ids(reference.node_id())
                    .find(|id| matches!(self.nodes.kind(*id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
                    .map(|id| self.nodes.kind(id).span());
                function == owner
                    && if reference.is_write() {
                        !reference.is_read()
                            && stores.iter().any(|(statement, _)| statement.start <= span.start && span.end <= statement.end)
                    } else {
                        span == read
                    }
            })
    }

    /// Each leaf of a complete tree ends storing `carry` (collected as the
    /// statement and value) or with `return`/`throw`.
    fn tree_stores(&self, branch: &IfStatement<'_>, carry: SymbolId, stores: &mut Vec<(Span, Span)>) -> bool {
        let Some(alternate) = &branch.alternate else {
            return false;
        };
        [&branch.consequent, alternate].into_iter().all(|arm| {
            let last = match arm {
                Statement::BlockStatement(block) => block.body.last(),
                other => Some(other),
            };
            match last {
                Some(Statement::IfStatement(inner)) => self.tree_stores(inner, carry, stores),
                Some(Statement::ReturnStatement(_) | Statement::ThrowStatement(_)) => true,
                Some(statement) => match self.plain_target(statement) {
                    Some((target, value)) if target == carry => {
                        stores.push((statement.span(), value.span()));
                        true
                    }
                    _ => false,
                },
                None => false,
            }
        })
    }

    /// `$X = value;`: the target and the value.
    fn plain_target<'b, 'a>(&self, statement: &'b Statement<'a>) -> Option<(SymbolId, &'b Expression<'a>)> {
        let Statement::ExpressionStatement(expression) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &expression.expression else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            return None;
        };
        (assignment.operator == AssignmentOperator::Assign)
            .then(|| analysis::identifier_symbol(target, self.scoping))
            .flatten()
            .map(|symbol| (symbol, &assignment.right))
    }

    /// Collect each leaf's tail of settled stores `(symbol, statement, value)`
    /// in a complete `if`/`else` tree; false when an arm is missing.
    fn tree_tails(&self, branch: &IfStatement<'_>, leaves: &mut Vec<Vec<(SymbolId, Span, Span)>>) -> bool {
        let Some(alternate) = &branch.alternate else {
            return false;
        };
        self.arm_tails(&branch.consequent, leaves) && self.arm_tails(alternate, leaves)
    }

    fn arm_tails(&self, arm: &Statement<'_>, leaves: &mut Vec<Vec<(SymbolId, Span, Span)>>) -> bool {
        let statements: &[Statement<'_>] = match arm {
            Statement::BlockStatement(block) => &block.body,
            Statement::IfStatement(_) => std::slice::from_ref(arm),
            _ => return false,
        };
        let mut tail = Vec::new();
        let mut end = statements.len();
        while end > 0 {
            let Some((symbol, value)) = self.settled_store(&statements[end - 1]) else {
                break;
            };
            tail.push((symbol, statements[end - 1].span(), value));
            end -= 1;
        }
        tail.reverse();
        // A nested complete tree right before the tail: its leaves run the
        // tail after their own.
        if let Some(Statement::IfStatement(inner)) = end.checked_sub(1).map(|at| &statements[at]) {
            let first = leaves.len();
            if self.tree_tails(inner, leaves) {
                for leaf in &mut leaves[first..] {
                    leaf.extend(tail.iter().copied());
                }
                return true;
            }
            leaves.truncate(first);
        }
        leaves.push(tail);
        true
    }

    /// `$X = v;` with a settled `v`: the target and the value.
    fn settled_store(&self, statement: &Statement<'_>) -> Option<(SymbolId, Span)> {
        let Statement::ExpressionStatement(expression) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &expression.expression else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            return None;
        };
        if assignment.operator != AssignmentOperator::Assign || !target.name.starts_with('$') {
            return None;
        }
        let symbol = analysis::identifier_symbol(target, self.scoping)?;
        self.settled(&assignment.right, statement.span()).then(|| (symbol, assignment.right.span()))
    }

    /// A value no code can change, read without effect, that names only
    /// bindings declared outside `scope`.
    fn settled(&self, value: &Expression<'_>, scope: Span) -> bool {
        match value {
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::ThisExpression(_) => true,
            Expression::UnaryExpression(unary) => {
                unary.operator == oxc_syntax::operator::UnaryOperator::Void
                    && matches!(unary.argument, Expression::NumericLiteral(_))
            }
            Expression::Identifier(identifier) => match analysis::identifier_symbol(identifier, self.scoping) {
                None => super::super::primitives::is_stable_global(&identifier.name),
                Some(symbol) => {
                    !self.scoping.symbol_is_mutated(symbol)
                        && self.scoping.symbol_declarations(symbol).count() == 1
                        && matches!(
                            self.nodes.kind(self.scoping.symbol_declaration(symbol)),
                            AstKind::VariableDeclarator(declarator) if declarator.init.is_some()
                        )
                        && self.nodes.kind(self.scoping.symbol_declaration(symbol)).span().end <= scope.start
                }
            },
            Expression::ComputedMemberExpression(member) => {
                matches!(&member.object, Expression::Identifier(object)
                    if object.name == "arguments" && analysis::identifier_symbol(object, self.scoping).is_none())
                    && matches!(&member.expression, Expression::NumericLiteral(_))
                    && !member.optional
                    && self.fixed_arguments()
                    || super::super::primitives::is_builtin_member(
                        value.as_member_expression().expect("a member"),
                        self.scoping,
                    )
            }
            Expression::StaticMemberExpression(_) => {
                super::super::primitives::is_builtin_member(value.as_member_expression().expect("a member"), self.scoping)
            }
            _ => false,
        }
    }

    /// The current function never changes its `arguments`.
    fn fixed_arguments(&self) -> bool {
        self.functions.last().is_some_and(|function| function.1)
    }

    /// Every write of `symbol` is one of `writes`; it has no other reference
    /// inside `tree`, and none outside the current function's own code.
    fn only_these_writes(&self, symbol: SymbolId, writes: &[Span], tree: Span) -> bool {
        let owner = self.functions.last().map(|function| function.0);
        self.scoping.get_resolved_references(symbol).all(|reference| {
            let span = self.nodes.kind(reference.node_id()).span();
            let inside = tree.start <= span.start && span.end <= tree.end;
            let is_listed = writes.iter().any(|write| write.start <= span.start && span.end <= write.end);
            let function = self
                .nodes
                .ancestor_ids(reference.node_id())
                .find(|id| matches!(self.nodes.kind(*id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
                .map(|id| self.nodes.kind(id).span());
            function == owner && if reference.is_write() { is_listed && !reference.is_read() } else { !inside }
        }) && writes.len()
            == self.scoping.get_resolved_references(symbol).filter(|reference| reference.is_write()).count()
            && self.nodes.kind(self.scoping.symbol_declaration(symbol)).span().end <= tree.start
    }

    /// A bare `{ … }` in a statement list whose own bindings can move to the
    /// list is just its statements.
    fn unwrap_block(&mut self, block: &BlockStatement<'_>) {
        if self.has_comment(block.span) || !self.splice_safe(&block.body) {
            return;
        }
        let replacement = match (block.body.first(), block.body.last()) {
            (Some(first), Some(last)) => self.source[first.span().start as usize..last.span().end as usize].to_string(),
            _ => String::new(),
        };
        self.edits.push(analysis::Edit::replace(block.span, replacement));
        self.flattened += 1;
    }

    /// The arm's own bindings can move to the enclosing block.
    fn splice_safe(&self, statements: &[Statement<'_>]) -> bool {
        statements.iter().all(|statement| match statement {
            Statement::FunctionDeclaration(_) | Statement::ClassDeclaration(_) => false,
            Statement::VariableDeclaration(declaration) => declaration.declarations.iter().all(|declarator| {
                let mut names = Names::default();
                names.visit_binding_pattern(&declarator.id);
                names.all_generated
            }),
            _ => true,
        })
    }

    fn sink(&mut self, declaration: &VariableDeclaration<'_>, rest: &[Statement<'_>]) {
        if declaration.kind != VariableDeclarationKind::Let || self.has_comment(declaration.span) {
            return;
        }
        // Each bare `$X` of `let $X, $Y;` whose first reference is a plain
        // store statement of this list is declared at that store instead.
        let mut kept: Vec<&str> = Vec::new();
        let mut stores: Vec<(Span, String)> = Vec::new();
        for declarator in &declaration.declarations {
            let text = self.text(declarator.span);
            match (&declarator.id, &declarator.init) {
                (BindingPattern::BindingIdentifier(binding), None) if binding.name.starts_with('$') => {
                    match binding.symbol_id.get().and_then(|symbol| self.first_store(symbol, rest)) {
                        Some((store, value)) if !stores.iter().any(|(other, _)| *other == store) => {
                            stores.push((store, format!("let {} = {};", self.text(binding.span), self.text(value))));
                        }
                        _ => kept.push(text),
                    }
                }
                _ => kept.push(text),
            }
        }
        if stores.is_empty() {
            return;
        }
        if kept.is_empty() {
            self.edits.push(analysis::Edit::delete(whole_line(self.source, declaration.span)));
        } else {
            self.edits.push(analysis::Edit::replace(declaration.span, format!("let {};", kept.join(", "))));
        }
        self.sunk += stores.len();
        for (store, replacement) in stores {
            self.edits.push(analysis::Edit::replace(store, replacement));
        }
    }

    /// The store statement in `rest` holding `symbol`'s first reference, and
    /// its value, when it is a plain store.
    fn first_store(&self, symbol: SymbolId, rest: &[Statement<'_>]) -> Option<(Span, Span)> {
        // A closure could run before the store and meet the binding's TDZ.
        let function_of = |node| {
            self.nodes
                .ancestor_ids(node)
                .find(|id| matches!(self.nodes.kind(*id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
        };
        let declaring = function_of(self.scoping.symbol_declaration(symbol));
        if self.scoping.get_resolved_references(symbol).any(|reference| function_of(reference.node_id()) != declaring) {
            return None;
        }
        let first = self
            .scoping
            .get_resolved_references(symbol)
            .map(|reference| self.nodes.kind(reference.node_id()).span())
            .min_by_key(|span| span.start)?;
        let store = rest.iter().find(|statement| statement.span().start <= first.start && first.end <= statement.span().end)?;
        let value = self.plain_store(store, symbol)?;
        (!self.has_comment(store.span())).then(|| (store.span(), value))
    }

    /// `$X = value;` storing into `symbol`, with a value that does not read it.
    fn plain_store(&self, statement: &Statement<'_>, symbol: SymbolId) -> Option<Span> {
        let Statement::ExpressionStatement(expression) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &expression.expression else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            return None;
        };
        if assignment.operator != AssignmentOperator::Assign
            || analysis::identifier_symbol(target, self.scoping) != Some(symbol)
            || matches!(assignment.right, Expression::SequenceExpression(_))
        {
            return None;
        }
        let mut reads = Reads { scoping: self.scoping, symbol, found: false };
        reads.visit_expression(&assignment.right);
        (!reads.found).then(|| assignment.right.span())
    }
}

/// A statement list that always leaves its block.
fn leaves(statements: &[Statement<'_>]) -> bool {
    matches!(
        statements.last(),
        Some(
            Statement::ReturnStatement(_)
                | Statement::ThrowStatement(_)
                | Statement::BreakStatement(_)
                | Statement::ContinueStatement(_)
        )
    )
}

#[derive(Default)]
struct Names {
    all_generated: bool,
    seen: bool,
}

impl<'a> Visit<'a> for Names {
    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        if !self.seen {
            self.all_generated = true;
            self.seen = true;
        }
        self.all_generated &= identifier.name.starts_with('$');
    }
}

struct Reads<'s> {
    scoping: &'s Scoping,
    symbol: SymbolId,
    found: bool,
}

impl<'a> Visit<'a> for Reads<'_> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if analysis::identifier_symbol(identifier, self.scoping) == Some(self.symbol) {
            self.found = true;
        }
    }
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.list(&body.statements);
        walk::walk_function_body(self, body);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.list(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_switch_case(&mut self, case: &SwitchCase<'a>) {
        self.list(&case.consequent);
        walk::walk_switch_case(self, case);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.dynamic |= identifier.name == "eval";
    }
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.functions.push((function.span, super::super::dead_ssa::readonly_arguments(function), 0));
        walk::walk_function(self, function, flags);
        self.functions.pop();
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push((arrow.span, false, 0));
        walk::walk_arrow_function_expression(self, arrow);
        self.functions.pop();
    }
    fn visit_try_statement(&mut self, statement: &TryStatement<'a>) {
        if let Some(function) = self.functions.last_mut() {
            function.2 += 1;
        }
        walk::walk_try_statement(self, statement);
        if let Some(function) = self.functions.last_mut() {
            function.2 -= 1;
        }
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

#[cfg(test)]
mod tests {
    use super::flatten_exits;

    #[test]
    fn flattens_else_after_an_arm_that_leaves() {
        let source = "function f(t, x) {\n  if (t) {\n    return null;\n  } else {\n    let $SSA_1 = x.p;\n    g($SSA_1);\n  }\n  if (t) {\n    g(1);\n  } else {\n    throw x;\n  }\n}";
        let result = flatten_exits(source).unwrap();
        assert_eq!(result.flattened, 2, "{}", result.code);
        assert!(result.code.contains("if (t) {\n    return null;\n  }\n  let $SSA_1 = x.p;\n  g($SSA_1);"), "{}", result.code);
        assert!(result.code.contains("if (!(t)) {\n    throw x;\n  }\n  g(1);"), "{}", result.code);
    }

    #[test]
    fn sinks_a_bare_declaration_into_its_first_store() {
        let source = "function f(x) {\n  let $CARRY_1;\n  g();\n  $CARRY_1 = x;\n  $CARRY_1 = 2;\n  return $CARRY_1;\n}";
        let result = flatten_exits(source).unwrap();
        assert_eq!(result.sunk, 1, "{}", result.code);
        assert!(result.code.contains("g();\n  let $CARRY_1 = x;\n  $CARRY_1 = 2;"), "{}", result.code);
    }

    #[test]
    fn sets_a_carry_every_leaf_sets_alike_once_after_the_tree() {
        let source = "function f() {\n  let $JOIN_1, $JOIN_2;\n  if (arguments[0] < 0) {\n    $JOIN_1 = true;\n    $JOIN_2 = arguments[1];\n  } else {\n    if (arguments[0] === 0) {\n      g();\n      $JOIN_1 = g();\n      $JOIN_2 = arguments[1];\n    } else {\n      $JOIN_1 = false;\n      $JOIN_2 = arguments[1];\n    }\n  }\n  return [$JOIN_1, $JOIN_2];\n}";
        let result = flatten_exits(source).unwrap();
        assert_eq!(result.hoisted, 1, "{}", result.code);
        assert_eq!(result.code.matches("$JOIN_2 = arguments[1];").count(), 1, "{}", result.code);
        assert!(result.code.contains("  }\n  let $JOIN_2 = arguments[1];\n  return"), "{}", result.code);
        for declined in [
            // a missing else: one path keeps the old value
            source.replace("    } else {\n      $JOIN_1 = false;\n      $JOIN_2 = arguments[1];\n    }\n", "    }\n"),
            // a value that runs code
            source.replace("$JOIN_2 = arguments[1];", "$JOIN_2 = g();"),
            // an effect after the store in a leaf
            source.replace("      $JOIN_1 = g();\n      $JOIN_2 = arguments[1];", "      $JOIN_2 = arguments[1];\n      $JOIN_1 = g();"),
            // a parameter makes arguments[n] writable
            source.replace("function f()", "function f(a)"),
            // a try around the tree
            source.replace("  if (arguments[0] < 0) {", "  try {} finally {}\n  try {\n  if (arguments[0] < 0) {").replace("  return [", "  } catch (e) {}\n  return ["),
        ] {
            let result = flatten_exits(&declined).unwrap();
            assert_eq!(result.hoisted, 0, "{declined} -> {}", result.code);
        }
    }

    #[test]
    fn sinks_names_out_of_a_shared_declaration() {
        let source = "function f(x) {\n  let $VAR_1, $JOIN_2, $JOIN_3;\n  g();\n  $VAR_1 = this;\n  $JOIN_3 = x;\n  return [$VAR_1, $JOIN_2, $JOIN_3, () => $JOIN_2];\n}";
        let result = flatten_exits(source).unwrap();
        assert_eq!(result.sunk, 2, "{}", result.code);
        assert!(result.code.contains("let $JOIN_2;\n  g();\n  let $VAR_1 = this;\n  let $JOIN_3 = x;"), "{}", result.code);
    }

    #[test]
    fn stores_the_test_first_when_one_arm_stores_it() {
        let source = "function f(a, b, g) {\n  const $MERGE_1 = a && b;\n  let $JOIN_1;\n  if ($MERGE_1) {\n    let $SSA_1 = g();\n    $JOIN_1 = $SSA_1.x;\n  } else {\n    $JOIN_1 = $MERGE_1;\n  }\n  return g($JOIN_1);\n}";
        let result = flatten_exits(source).unwrap();
        assert!(result.code.contains("  let $JOIN_1 = $MERGE_1;\n  if ($JOIN_1) {\n    let $SSA_1 = g();\n    $JOIN_1 = $SSA_1.x;\n  }\n  return"), "{}", result.code);
        for declined in [
            // the other arm does not end storing the carry
            source.replace("    $JOIN_1 = $SSA_1.x;\n", "    $JOIN_1 = $SSA_1.x;\n    g();\n"),
            // the arm reads the carry
            source.replace("let $SSA_1 = g();", "let $SSA_1 = g($JOIN_1);"),
            // a closure reads the carry
            source.replace("  return g($JOIN_1);", "  return () => $JOIN_1;"),
        ] {
            let result = flatten_exits(&declined).unwrap();
            assert!(result.code.contains("} else {"), "{declined} -> {}", result.code);
        }
    }

    #[test]
    fn splices_a_bare_block_into_its_list() {
        let source = "function f(g) {\n  {\n    let $LOOP_1 = 0;\n    g($LOOP_1);\n  }\n  {\n    let i = 0;\n    g(i);\n  }\n  {\n    function h() {}\n  }\n}";
        let result = flatten_exits(source).unwrap();
        assert!(result.code.contains("function f(g) {\n  let $LOOP_1 = 0;\n    g($LOOP_1);\n  {\n    let i = 0;"), "{}", result.code);
        assert!(result.code.contains("{\n    function h() {}\n  }"), "{}", result.code);
    }

    #[test]
    fn returns_from_each_leaf_instead_of_a_join() {
        let source = "function f(c, d, g) {\n  let $JOIN_1;\n  if (c) {\n    g();\n    $JOIN_1 = 1;\n  } else {\n    if (d) {\n      throw g;\n    } else {\n      $JOIN_1 = g(2);\n    }\n  }\n  return $JOIN_1;\n}";
        let result = flatten_exits(source).unwrap();
        assert!(result.code.contains("    g();\n    return 1;\n  }\n  if (d) {"), "{}", result.code);
        assert!(result.code.contains("      return g(2);\n    }\n}"), "{}", result.code);
        // another read of the carry
        let kept = flatten_exits(&source.replace("    g();\n    $JOIN_1 = 1;", "    $JOIN_1 = 1;\n    g($JOIN_1);")).unwrap();
        assert!(kept.code.contains("return $JOIN_1;"), "{}", kept.code);
    }

    #[test]
    fn stores_into_the_copy_of_a_join_in_each_leaf() {
        let source = "function f(c, g) {\n  let $JOIN_1;\n  if (c) {\n    g();\n    $JOIN_1 = 1;\n  } else {\n    $JOIN_1 = g(2);\n  }\n  let $LOOP_1 = $JOIN_1;\n  while (g($LOOP_1)) $LOOP_1 = 3;\n}";
        let result = flatten_exits(source).unwrap();
        assert!(result.code.contains("  let $LOOP_1;\n  if (c) {\n    g();\n    $LOOP_1 = 1;\n  } else {\n    $LOOP_1 = g(2);\n  }\n  while"), "{}", result.code);
        let assigned = flatten_exits(&source.replace("let $LOOP_1 = $JOIN_1;", "$KEY_1 = $JOIN_1;")).unwrap();
        assert!(assigned.code.contains("    $KEY_1 = g(2);\n  }\n  while"), "{}", assigned.code);
        // the tree names the target of a `let`
        let kept = flatten_exits(&source.replace("    g();\n    $JOIN_1 = 1;", "    g(() => $LOOP_1);\n    $JOIN_1 = 1;")).unwrap();
        assert!(kept.code.contains("let $LOOP_1 = $JOIN_1;"), "{}", kept.code);
    }

    #[test]
    fn keeps_what_cannot_move() {
        for source in [
            // a non-generated binding would move to the outer block
            "function f(t) {\n  if (t) {\n    return 1;\n  } else {\n    let y = 2;\n    g(y);\n  }\n}",
            // a function declaration in the spliced arm
            "function f(t) {\n  if (t) {\n    return 1;\n  } else {\n    function h() {}\n    g(h);\n  }\n}",
            // both arms leave
            "function f(t) {\n  if (t) {\n    return 1;\n  } else {\n    return 2;\n  }\n}",
            // the first reference is a read
            "function f(x) {\n  let $CARRY_1;\n  g($CARRY_1);\n  $CARRY_1 = x;\n}",
            // the first store is nested, not a statement of the list
            "function f(t, x) {\n  let $CARRY_1;\n  if (t) {\n    $CARRY_1 = x;\n  }\n  return $CARRY_1;\n}",
        ] {
            let result = flatten_exits(source).unwrap();
            assert_eq!(result.flattened + result.sunk, 0, "{source} -> {}", result.code);
        }
    }
}
