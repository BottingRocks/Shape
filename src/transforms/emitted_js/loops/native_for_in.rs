//! Turn the VM's lowered `for…in` back into the native loop.
//!
//! The VM enumerates with a key snapshot and skips keys gone by the time
//! they come up:
//!
//! ```js
//! let K = vmForInKeys(O);
//! for (let i = 0; i < K["length"]; i++) {
//!     if (K[i] in O) BODY
//! }
//! ```
//!
//! A native `for (const i in O) BODY` (with `K[i]` read as `i`) visits the
//! same keys in the same order and skips the same deleted ones. It holds when
//! `K` is read only as `K["length"]` in the test and `K[i]` in the body, `i`
//! only inside `K[i]` (besides the counter itself), and `O` reads names the
//! loop never assigns, so the check sees the object the snapshot came from.
//! A `vmForInKeys` matching the emitter's own text with no calls left goes.

use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator, UpdateOperator};

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn native_for_in_loops(source: &str) -> Result<(String, usize), String> {
    // A loop nested in another rewritten one goes in the next round.
    let mut code = source.to_string();
    let mut total = 0;
    loop {
        let (next, count) = native_for_in_once(&code)?;
        if count == 0 {
            return Ok((next, total));
        }
        code = next;
        total += count;
    }
}

fn native_for_in_once(source: &str) -> Result<(String, usize), String> {
    if !source.contains("function vmForInKeys") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let canonical = crate::tracing::symbolic::HELPERS
        .iter()
        .find(|(name, _)| *name == "vmForInKeys")
        .map(|(_, text)| analysis::parse_program(&allocator, text).map(|parsed| analysis::codegen(&parsed)))
        .transpose()?
        .ok_or("no canonical vmForInKeys")?;
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, canonical, allocator: &allocator, helpers: Vec::new(), loops: Vec::new() };
    scan.visit_program(&program);
    let mut edits = Vec::new();
    let mut calls = BTreeMap::<SymbolId, usize>::new();
    scan.loops.sort_by_key(|found| (found.span.start, std::cmp::Reverse(found.span.end)));
    let mut end = 0;
    for found in &scan.loops {
        if !scan.helpers.iter().any(|(symbol, _)| *symbol == found.helper) || found.span.start < end {
            continue;
        }
        end = found.span.end;
        *calls.entry(found.helper).or_default() += 1;
        edits.push(analysis::Edit::replace(found.span, found.text.clone()));
    }
    let count = edits.len();
    for (symbol, span) in &scan.helpers {
        let references = scoping.get_resolved_reference_ids(*symbol).len();
        if references == calls.get(symbol).copied().unwrap_or(0) {
            edits.push(analysis::Edit::delete(whole_line(source, *span)));
        }
    }
    if edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, edits, "native for-in")?;
    Ok((output, count))
}

struct Found {
    /// The `let K = …;` declaration through the loop's end.
    span: Span,
    helper: SymbolId,
    text: String,
}

/// The loop's counter: `for (let i = 0; i < K["length"]; i++)`, or a
/// `while (i < K["length"])` stepping it in the body.
struct Counter<'s, 'a> {
    body: &'s Statement<'a>,
    /// The `for` update, `i++`.
    update: Option<&'s Expression<'a>>,
    /// `let i = 0`, in the loop head or before it.
    init: &'s VariableDeclaration<'a>,
    index: &'s BindingIdentifier<'a>,
    symbol: SymbolId,
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    canonical: String,
    allocator: &'a Allocator,
    /// `vmForInKeys` declarations matching the emitter's text.
    helpers: Vec<(SymbolId, Span)>,
    loops: Vec<Found>,
}

fn identifier<'s, 'a>(expression: &'s Expression<'a>) -> Option<&'s IdentifierReference<'a>> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier),
        _ => None,
    }
}

/// `K[i]`, `K["length"]` or `K.length`: the object and the key.
fn member_of<'s, 'a>(expression: &'s Expression<'a>) -> Option<(&'s IdentifierReference<'a>, &'s Expression<'a>)> {
    match expression {
        Expression::ComputedMemberExpression(member) if !member.optional => {
            Some((identifier(&member.object)?, &member.expression))
        }
        _ => None,
    }
}

fn is_length(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::ComputedMemberExpression(member) => {
            matches!(&member.expression, Expression::StringLiteral(key) if key.value == "length")
        }
        Expression::StaticMemberExpression(member) => member.property.name == "length",
        _ => false,
    }
}

/// Names and member reads only: evaluating it again gives the same value
/// while none of its names is assigned.
fn names_of(expression: &Expression<'_>, names: &mut Vec<SymbolId>, scoping: &Scoping) -> bool {
    match expression {
        Expression::Identifier(identifier) => {
            if let Some(symbol) = analysis::identifier_symbol(identifier, scoping) {
                names.push(symbol);
                true
            } else {
                // A global could be stored by any call; `arguments` is fixed.
                identifier.name == "arguments"
            }
        }
        Expression::ComputedMemberExpression(member) if !member.optional => {
            names_of(&member.object, names, scoping) && names_of(&member.expression, names, scoping)
        }
        Expression::StaticMemberExpression(member) if !member.optional => names_of(&member.object, names, scoping),
        Expression::NumericLiteral(_) | Expression::StringLiteral(_) => true,
        _ => false,
    }
}

impl Scan<'_, '_> {
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    fn list(&mut self, statements: &[Statement<'_>]) {
        for (at, first) in statements.iter().enumerate() {
            // Statements between the snapshot and the loop: the counter's
            // declaration and stores of literals, which run nothing.
            let mut end = at + 1;
            while statements.get(end).is_some_and(|statement| self.inert(statement)) {
                end += 1;
            }
            let Some(loop_) = statements.get(end) else {
                continue;
            };
            if let Some(found) = self.lowered(first, &statements[at + 1..end], loop_) {
                self.loops.push(found);
            }
        }
    }

    /// `let x = 0;`, `let x;`, `x = "";`: no code runs, no key is read.
    fn inert(&self, statement: &Statement<'_>) -> bool {
        let literal = |expression: &Expression<'_>| {
            matches!(
                expression,
                Expression::NumericLiteral(_) | Expression::StringLiteral(_) | Expression::BooleanLiteral(_) | Expression::NullLiteral(_)
            ) || matches!(expression, Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void && matches!(unary.argument, Expression::NumericLiteral(_)))
        };
        match statement {
            Statement::VariableDeclaration(declaration) => {
                declaration.kind == VariableDeclarationKind::Let
                    && declaration.declarations.iter().all(|declarator| {
                        matches!(declarator.id, BindingPattern::BindingIdentifier(_)) && declarator.init.as_ref().is_none_or(literal)
                    })
            }
            Statement::ExpressionStatement(expression) => matches!(&expression.expression, Expression::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign
                    && matches!(assignment.left, AssignmentTarget::AssignmentTargetIdentifier(_))
                    && literal(&assignment.right)),
            _ => false,
        }
    }

    fn increments(&self, statement: &Statement<'_>, index: SymbolId) -> bool {
        matches!(statement, Statement::ExpressionStatement(expression) if self.increment(&expression.expression, index))
    }

    fn increment(&self, expression: &Expression<'_>, index: SymbolId) -> bool {
        matches!(expression, Expression::UpdateExpression(update)
            if update.operator == UpdateOperator::Increment
                && matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(name)
                    if analysis::identifier_symbol(name, self.scoping) == Some(index)))
    }

    fn lowered<'s, 'a>(&self, first: &'s Statement<'a>, between: &'s [Statement<'a>], second: &'s Statement<'a>) -> Option<Found> {
        let (keys, helper, object) = self.snapshot(first)?;
        let Counter { body, update, init, index, symbol: index_symbol } = self.counter(between, second, keys)?;
        let (check, increments, inner_step) = self.presence_check(body, update, index_symbol)?;
        if !self.checks_presence(check, keys, index_symbol, object) {
            return None;
        }
        if !self.object_fixed(object, between, second, &check.consequent) {
            return None;
        }
        let steps = if update.is_some() { 1 } else { increments.len() };
        let slots = self.key_slots(check, keys, index_symbol, inner_step, steps)?;
        let name = index.name.as_str();
        let body = self.native_body(check.consequent.span(), &slots, name, inner_step);
        // The inert statements stay, the counter's declaration goes.
        let indent = {
            let start = first.span().start as usize;
            let line = self.source[..start].rfind('\n').map_or(0, |at| at + 1);
            &self.source[line..start]
        };
        let mut text = String::new();
        for statement in between {
            if matches!(statement, Statement::VariableDeclaration(declaration) if std::ptr::eq(&**declaration, init)) {
                continue;
            }
            text.push_str(self.text(statement.span()));
            text.push('\n');
            text.push_str(indent);
        }
        text.push_str(&format!("for (const {name} in {}) {body}", self.text(object.span())));
        Some(Found { span: Span::new(first.span().start, second.span().end), helper, text })
    }

    /// `let K = vmForInKeys(O);`: the snapshot, the helper, and `O`.
    fn snapshot<'s, 'a>(&self, first: &'s Statement<'a>) -> Option<(SymbolId, SymbolId, &'s Expression<'a>)> {
        let Statement::VariableDeclaration(declaration) = first else {
            return None;
        };
        let [declarator] = declaration.declarations.as_slice() else {
            return None;
        };
        let BindingPattern::BindingIdentifier(keys) = &declarator.id else {
            return None;
        };
        let keys = keys.symbol_id.get()?;
        let Some(Expression::CallExpression(call)) = &declarator.init else {
            return None;
        };
        let callee = identifier(&call.callee)?;
        let helper = analysis::identifier_symbol(callee, self.scoping)?;
        let [object] = call.arguments.as_slice() else {
            return None;
        };
        let object = object.as_expression()?;
        Some((keys, helper, object))
    }

    /// Whether `expression` is a bare read of `symbol`.
    fn reads(&self, expression: &Expression<'_>, symbol: SymbolId) -> bool {
        identifier(expression).and_then(|name| analysis::identifier_symbol(name, self.scoping)) == Some(symbol)
    }

    fn counter<'s, 'a>(&self, between: &'s [Statement<'a>], second: &'s Statement<'a>, keys: SymbolId) -> Option<Counter<'s, 'a>> {
        // `for (let i = 0; i < K["length"]; i++)`, or `while (i < K["length"])`
        // counting with `++i` itself; the counter may be declared before.
        let (test, body, update, init) = match second {
            Statement::ForStatement(loop_) => {
                let init = match &loop_.init {
                    Some(ForStatementInit::VariableDeclaration(init)) => Some(&**init),
                    None => None,
                    _ => return None,
                };
                (loop_.test.as_ref()?, &loop_.body, Some(loop_.update.as_ref()?), init)
            }
            Statement::WhileStatement(loop_) => (&loop_.test, &loop_.body, None, None),
            _ => return None,
        };
        let Expression::BinaryExpression(test) = test else {
            return None;
        };
        let index_symbol = identifier(&test.left).and_then(|name| analysis::identifier_symbol(name, self.scoping))?;
        // `let i = 0`, in the loop head or in its own declaration before it.
        let declares_index = |declaration: &VariableDeclaration<'_>| {
            matches!(declaration.declarations.as_slice(), [declarator]
                if matches!(&declarator.id, BindingPattern::BindingIdentifier(binding) if binding.symbol_id.get() == Some(index_symbol)))
        };
        let declared_before = || {
            between.iter().find_map(|statement| match statement {
                Statement::VariableDeclaration(declaration) if declares_index(declaration) => Some(&**declaration),
                _ => None,
            })
        };
        let init = match init {
            Some(init) => init,
            None => declared_before()?,
        };
        let [counter] = init.declarations.as_slice() else {
            return None;
        };
        let BindingPattern::BindingIdentifier(index) = &counter.id else {
            return None;
        };
        if index.symbol_id.get() != Some(index_symbol) {
            return None;
        }
        if init.kind != VariableDeclarationKind::Let
            || !matches!(&counter.init, Some(Expression::NumericLiteral(zero)) if zero.value == 0.0)
        {
            return None;
        }
        let length_of_keys = match &test.right {
            Expression::ComputedMemberExpression(member) => self.reads(&member.object, keys),
            Expression::StaticMemberExpression(member) => self.reads(&member.object, keys),
            _ => false,
        };
        if test.operator != BinaryOperator::LessThan || !is_length(&test.right) || !length_of_keys {
            return None;
        }
        if update.is_some_and(|update| !self.increment(update, index_symbol)) {
            return None;
        }
        Some(Counter { body, update, init, index, symbol: index_symbol })
    }

    /// The `if (K[i] in O)` the body runs under, the counter steps a `while`
    /// takes, and the step inside the check's arm if any.
    fn presence_check<'s, 'a>(
        &self,
        body: &'s Statement<'a>,
        update: Option<&'s Expression<'a>>,
        index_symbol: SymbolId,
    ) -> Option<(&'s IfStatement<'a>, Vec<Span>, Option<Span>)> {
        // `{ if (K[i] in O) BODY }` for a `for`; a `while` counts in the
        // body: `{ if (K[i] in O) BODY ++i; }` or
        // `{ if (K[i] in O) { A ++i; B } else { ++i; } }`.
        let statements: &[Statement<'_>] = match body {
            Statement::BlockStatement(block) => &block.body,
            other => std::slice::from_ref(other),
        };
        let mut increments: Vec<Span> = Vec::new();
        let check = match (update.is_some(), statements) {
            (true, [only]) => only,
            (false, [only @ Statement::IfStatement(_), last]) if self.increments(last, index_symbol) => {
                increments.push(last.span());
                only
            }
            (false, [only @ Statement::IfStatement(_)]) => only,
            _ => return None,
        };
        let Statement::IfStatement(check) = check else {
            return None;
        };
        // The counter step inside the arms of a `while`.
        let mut inner_step = None;
        match (&check.alternate, update.is_some() || !increments.is_empty()) {
            (None, true) => {}
            (Some(alternate), false) => {
                let alternate: &[Statement<'_>] = match alternate {
                    Statement::BlockStatement(block) => &block.body,
                    other => std::slice::from_ref(other),
                };
                let [step] = alternate else {
                    return None;
                };
                let Statement::BlockStatement(consequent) = &check.consequent else {
                    return None;
                };
                let steps: Vec<&Statement<'_>> =
                    consequent.body.iter().filter(|statement| self.increments(statement, index_symbol)).collect();
                let ([own], true) = (steps.as_slice(), self.increments(step, index_symbol)) else {
                    return None;
                };
                increments.push(step.span());
                increments.push(own.span());
                inner_step = Some(own.span());
            }
            _ => return None,
        }
        // Without a `for` update, a `continue` would skip the step.
        if update.is_none() {
            let mut continues = Continues::default();
            continues.visit_statement(&check.consequent);
            if continues.found {
                return None;
            }
        }
        Some((check, increments, inner_step))
    }

    /// The check's test is `K[i] in O`, with `O` spelled as in the snapshot.
    fn checks_presence(&self, check: &IfStatement<'_>, keys: SymbolId, index_symbol: SymbolId, object: &Expression<'_>) -> bool {
        let Expression::BinaryExpression(present) = &check.test else {
            return false;
        };
        let Some((checked_keys, checked_index)) = member_of(&present.left) else {
            return false;
        };
        !(present.operator != BinaryOperator::In
            || analysis::identifier_symbol(checked_keys, self.scoping) != Some(keys)
            || !self.reads(checked_index, index_symbol)
            || self.text(present.right.span()) != self.text(object.span()))
    }

    /// `O` is the same object at every check: it reads only names neither
    /// the loop nor the statements before it assign, and the body does not
    /// change a property on the way to it.
    fn object_fixed(&self, object: &Expression<'_>, between: &[Statement<'_>], second: &Statement<'_>, consequent: &Statement<'_>) -> bool {
        // O reads names the loop never assigns.
        let mut names = Vec::new();
        if !names_of(object, &mut names, self.scoping) {
            return false;
        }
        let assigned_inside = |symbol: SymbolId| {
            self.scoping.get_resolved_references(symbol).any(|reference| {
                let at = self.span_of_reference(reference);
                reference.is_write() && second.span().start <= at.start && at.end <= second.span().end
            })
        };
        // Nor do the stores the snapshot now follows.
        let assigned_between = between.iter().any(|statement| {
            matches!(statement, Statement::ExpressionStatement(expression) if matches!(&expression.expression,
                Expression::AssignmentExpression(assignment) if matches!(&assignment.left,
                    AssignmentTarget::AssignmentTargetIdentifier(name) if analysis::identifier_symbol(name, self.scoping).is_some_and(|symbol| names.contains(&symbol)))))
        });
        if assigned_between || names.iter().any(|symbol| assigned_inside(*symbol)) {
            return false;
        }
        // A property of a name the body stores through or hands to a call
        // could change: then `O` must be that name itself.
        if !matches!(object, Expression::Identifier(_)) {
            let mut handed = Handed { source: self.source, object: self.text(object.span()), names: &names, scoping: self.scoping, found: false };
            handed.visit_statement(consequent);
            if handed.found {
                return false;
            }
        }
        true
    }

    /// The `K[i]` slots in the body, when they and the counting are the only
    /// uses of `K` and `i`.
    fn key_slots(&self, check: &IfStatement<'_>, keys: SymbolId, index_symbol: SymbolId, inner_step: Option<Span>, steps: usize) -> Option<Vec<Span>> {
        // K only in the test and as `K[i]`; i only there, in the check and
        // in its steps. Past an inner step `K[i]` is the next key.
        let mut uses = KeyUses { keys, index: index_symbol, scoping: self.scoping, slots: Vec::new(), other: false, skip: inner_step };
        uses.visit_statement(&check.consequent);
        if uses.other || inner_step.is_some_and(|step| uses.slots.iter().any(|slot| slot.start > step.start)) {
            return None;
        }
        let allowed_key_refs = uses.slots.len() + 2; // the test and the check
        let allowed_index_refs = uses.slots.len() + 2 + steps; // test, check, steps
        if self.scoping.get_resolved_reference_ids(keys).len() != allowed_key_refs
            || self.scoping.get_resolved_reference_ids(index_symbol).len() != allowed_index_refs
        {
            return None;
        }
        Some(uses.slots)
    }

    /// The check's consequent as the native loop's body: each `K[i]` read as
    /// `i`, the inner step's line gone, one level shallower.
    fn native_body(&self, body_span: Span, slots: &[Span], name: &str, inner_step: Option<Span>) -> String {
        let mut cuts: Vec<(Span, &str)> = slots.iter().map(|slot| (*slot, name)).collect();
        if let Some(step) = inner_step {
            cuts.push((whole_line(self.source, step), ""));
        }
        cuts.sort_by_key(|(span, _)| span.start);
        let mut body = String::new();
        let mut at = body_span.start;
        for (cut, text) in &cuts {
            body.push_str(&self.source[at as usize..cut.start as usize]);
            body.push_str(text);
            at = cut.end;
        }
        body.push_str(&self.source[at as usize..body_span.end as usize]);
        dedent(&body)
    }

    fn span_of_reference(&self, reference: &oxc_semantic::Reference) -> Span {
        self.nodes.get_node(reference.node_id()).span()
    }
}

/// The consequent sat one level deeper than the new loop's body.
fn dedent(body: &str) -> String {
    body.split('\n').enumerate().map(|(index, line)| if index == 0 { line } else { line.strip_prefix('\t').unwrap_or(line) }).collect::<Vec<_>>().join("\n")
}

/// A `continue` that reaches the loop: unlabeled outside inner loops, or
/// any labeled one.
#[derive(Default)]
struct Continues {
    loops: usize,
    found: bool,
}

impl<'a> Visit<'a> for Continues {
    fn visit_statement(&mut self, statement: &Statement<'a>) {
        let nests = matches!(
            statement,
            Statement::ForStatement(_)
                | Statement::WhileStatement(_)
                | Statement::DoWhileStatement(_)
                | Statement::ForInStatement(_)
                | Statement::ForOfStatement(_)
        );
        self.loops += usize::from(nests);
        walk::walk_statement(self, statement);
        self.loops -= usize::from(nests);
    }
    fn visit_continue_statement(&mut self, statement: &ContinueStatement<'a>) {
        self.found |= statement.label.is_some() || self.loops == 0;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}

/// A store through one of `names` (`n[…] = …`) or a call handed one.
struct Handed<'s> {
    source: &'s str,
    /// The text of `O`.
    object: &'s str,
    names: &'s [SymbolId],
    scoping: &'s Scoping,
    found: bool,
}

impl Handed<'_> {
    /// Handing a value to a call can change its properties: a name, or a
    /// member short of `O` itself, whose slot leads to `O`.
    fn handed(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::Identifier(_) => self.root(expression),
            Expression::ComputedMemberExpression(_) | Expression::StaticMemberExpression(_) => {
                let text = &self.source[expression.span().start as usize..expression.span().end as usize];
                self.root(expression) && self.object.starts_with(text) && self.object != text
            }
            _ => false,
        }
    }

    fn root(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::Identifier(name) => {
                analysis::identifier_symbol(name, self.scoping).is_some_and(|symbol| self.names.contains(&symbol))
            }
            Expression::ComputedMemberExpression(member) => self.root(&member.object),
            Expression::StaticMemberExpression(member) => self.root(&member.object),
            _ => false,
        }
    }
}

impl<'a> Visit<'a> for Handed<'_> {
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if let Some(member) = assignment.left.as_member_expression() {
            self.found |= self.handed(member.object());
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        self.found |= call.arguments.iter().any(|argument| argument.as_expression().is_some_and(|a| self.handed(a)));
        if let Some(member) = call.callee.as_member_expression() {
            self.found |= self.handed(member.object());
        }
        walk::walk_call_expression(self, call);
    }
}

/// The `K[i]` reads in the loop body, and whether `K` or `i` appear any
/// other way.
struct KeyUses<'s> {
    keys: SymbolId,
    index: SymbolId,
    scoping: &'s Scoping,
    slots: Vec<Span>,
    other: bool,
    /// The counter's own step, which reads it.
    skip: Option<Span>,
}

impl<'a> Visit<'a> for KeyUses<'_> {
    fn visit_statement(&mut self, statement: &Statement<'a>) {
        if Some(statement.span()) != self.skip {
            walk::walk_statement(self, statement);
        }
    }
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if let Some((keys, index)) = member_of(expression) {
            let index_is_counter = identifier(index)
                .is_some_and(|name| analysis::identifier_symbol(name, self.scoping) == Some(self.index));
            if analysis::identifier_symbol(keys, self.scoping) == Some(self.keys) && index_is_counter {
                self.slots.push(expression.span());
                return;
            }
        }
        walk::walk_expression(self, expression);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        let symbol = analysis::identifier_symbol(identifier, self.scoping);
        self.other |= symbol == Some(self.keys) || symbol == Some(self.index);
    }
}

impl<'a> Visit<'a> for Scan<'_, '_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(id) = &function.id {
            if id.name == "vmForInKeys" {
                if let Some(symbol) = id.symbol_id.get() {
                    let text = self.text(function.span);
                    let same = analysis::parse_program(self.allocator, text)
                        .is_ok_and(|parsed| analysis::codegen(&parsed) == self.canonical);
                    if same && !self.scoping.symbol_is_mutated(symbol) {
                        self.helpers.push((symbol, function.span));
                    }
                }
            }
        }
        walk::walk_function(self, function, flags);
    }
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.list(&body.statements);
        walk::walk_function_body(self, body);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.list(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_program(&mut self, program: &Program<'a>) {
        self.list(&program.body);
        walk::walk_program(self, program);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELPER: &str = "\tfunction vmForInKeys(object) {\n\t\tvar keys = [];\n\t\tfor (var key in object) {\n\t\t\tkeys[keys.length] = key;\n\t\t}\n\t\treturn keys;\n\t}\n";

    #[test]
    fn restores_the_native_loop_and_drops_the_helper() {
        let source = format!("function f(o, out) {{\n{HELPER}\tlet K = vmForInKeys(o);\n\tfor (let i = 0; i < K[\"length\"]; i++) {{\n\t\tif (K[i] in o) {{\n\t\t\tout.push(K[i]);\n\t\t}}\n\t}}\n}}\n");
        let (code, count) = native_for_in_loops(&source).unwrap();
        assert_eq!(count, 1);
        assert_eq!(code, "function f(o, out) {\n\tfor (const i in o) {\n\t\tout.push(i);\n\t}\n}\n");
    }

    #[test]
    fn restores_nested_loops_and_a_counter_declared_before() {
        let source = format!("function f(a, j, out) {{\n{HELPER}\tlet K = vmForInKeys(a[j]);\n\tlet i = 0;\n\tfor (; i < K[\"length\"]; i++) {{\n\t\tif (K[i] in a[j]) {{\n\t\t\tlet L = vmForInKeys(out);\n\t\t\tfor (let k = 0; k < L.length; k++) {{\n\t\t\t\tif (L[k] in out) {{\n\t\t\t\t\tout[L[k]] = a[j][K[i]];\n\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n");
        let (code, _) = native_for_in_loops(&source).unwrap();
        assert!(!code.contains("vmForInKeys"), "{code}");
        assert!(code.contains("for (const i in a[j]) {"), "{code}");
        assert!(code.contains("for (const k in out) {\n\t\t\tout[k] = a[j][i];"), "{code}");
    }

    #[test]
    fn restores_a_while_loop_that_steps_in_both_arms() {
        let source = format!("function f(o, out) {{\n{HELPER}\tlet K = vmForInKeys(o);\n\tlet n = 0;\n\tlet c = 0;\n\ts = \"\";\n\twhile (n < K[\"length\"]) {{\n\t\tif (K[n] in o) {{\n\t\t\tlet v = out[K[n]];\n\t\t\t++c;\n\t\t\t++n;\n\t\t\ts = s + v;\n\t\t}} else {{\n\t\t\t++n;\n\t\t}}\n\t}}\n}}\n");
        let (code, count) = native_for_in_loops(&source).unwrap();
        assert_eq!(count, 1, "{code}");
        assert_eq!(
            code,
            "function f(o, out) {\n\tlet c = 0;\n\ts = \"\";\n\tfor (const n in o) {\n\t\tlet v = out[n];\n\t\t++c;\n\t\ts = s + v;\n\t}\n}\n"
        );
    }

    #[test]
    fn keeps_a_while_loop_whose_continue_skips_the_step() {
        let source = format!("function f(o, out) {{\n{HELPER}\tlet K = vmForInKeys(o);\n\tlet n = 0;\n\twhile (n < K.length) {{\n\t\tif (K[n] in o) {{\n\t\t\tif (out(K[n])) continue;\n\t\t}}\n\t\t++n;\n\t}}\n}}\n");
        let (_, count) = native_for_in_loops(&source).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn keeps_loops_that_read_the_snapshot_otherwise() {
        for body in [
            // The counter read on its own.
            "\t\tif (K[i] in o) {\n\t\t\tout.push(i);\n\t\t}\n",
            // No presence check.
            "\t\tout.push(K[i]);\n",
            // The object changes inside the loop.
            "\t\tif (K[i] in o) {\n\t\t\to = out;\n\t\t}\n",
        ] {
            let source = format!("function f(o, out) {{\n{HELPER}\tlet K = vmForInKeys(o);\n\tfor (let i = 0; i < K[\"length\"]; i++) {{\n{body}\t}}\n}}\n");
            let (code, count) = native_for_in_loops(&source).unwrap();
            assert_eq!(count, 0, "{code}");
            assert!(code.contains("function vmForInKeys"), "{code}");
        }
    }

    #[test]
    fn keeps_a_member_object_whose_slot_the_body_can_change() {
        let source = format!("function f(a, j, out) {{\n{HELPER}\tlet K = vmForInKeys(a[j]);\n\tfor (let i = 0; i < K.length; i++) {{\n\t\tif (K[i] in a[j]) {{\n\t\t\tout(a);\n\t\t}}\n\t}}\n}}\n");
        let (_, count) = native_for_in_loops(&source).unwrap();
        assert_eq!(count, 0);
    }
}
