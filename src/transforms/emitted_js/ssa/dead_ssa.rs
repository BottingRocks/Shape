//! Remove generated SSA/read scaffolding left by linked-program rewrites.
//!
//! Three independent cleanups share this module:
//! - an unread single-binding `let $SSA_n = init;` is removed when `init` is
//!   pure, or kept as `init;` when it has effects but is shorter as an
//!   expression; an identifier initialized by the immediately preceding
//!   declaration/plain assignment is also removable;
//! - an unread generated slot of `let [a, b, c] = init;` becomes a hole. The
//!   pattern keeps its length, so the iterator steps the same number of times;
//!   only when `init` calls a fresh-tuple helper (an array literal) and the
//!   code writes no native prototype (`dense_arrays::ordinary_prototypes`, so
//!   the standard Array iterator steps it) are trailing holes dropped, and a
//!   fully unread tuple becomes `init;`;
//! - a bare `$KEY_n;`/`$CARRY_n;` read is removed only after a declaration of
//!   the same resolved symbol has executed in that statement list;
//! - a discarded `(arguments[<u32>]);` read is removed only when the function
//!   never exposes `arguments` (no parameter, no other access form, no
//!   `delete`).
//!
//! All forms skip `with`/`eval`. The first two are run in a bounded loop with
//! dead-key cleanup; the dropped-read case assumes ordinary built-ins without
//! numeric prototype getters.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;

use super::super::purity::expression_is_pure;
use super::super::tuple_unpack::fresh_tuple_helper;
use super::super::support::whole_line;

#[derive(Debug)]
pub(crate) struct DeadSsaResult {
    pub code: String,
    pub removed: usize,
}

/// Drop an unread SSA declaration. If its initializer can throw or have an
/// effect, evaluate that expression in place and discard only the binding.
pub(crate) fn remove_dead_ssa(source: &str) -> Result<DeadSsaResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut helpers = FreshHelpers::default();
    // Dropping iterator steps needs the standard Array iterator: no fresh
    // helper counts when the code writes a native prototype.
    if super::super::dense_arrays::ordinary_prototypes(&program, &scoping) {
        helpers.visit_program(&program);
    }
    let mut scan = Scan {
        scoping: &scoping,
        nodes: &nodes,
        candidates: Vec::new(),
        tuples: Vec::new(),
        fresh_helpers: helpers
            .symbols
            .into_iter()
            .filter(|symbol| {
                !scoping
                    .get_resolved_references(*symbol)
                    .any(|reference| reference.is_write())
            })
            .collect(),
        dynamic_scope: false,
    };
    scan.visit_program(&program);
    if scan.dynamic_scope {
        return Ok(DeadSsaResult {
            code: source.into(),
            removed: 0,
        });
    }
    let mut edits = Vec::new();
    let mut removed = 0;
    for tuple in &scan.tuples {
        let live: Vec<bool> = tuple
            .slots
            .iter()
            .map(|slot| {
                slot.is_some_and(|(_, symbol, generated)| {
                    !generated
                        || scoping
                            .get_resolved_references(symbol)
                            .next()
                            .is_some()
                })
            })
            .collect();
        let dead = tuple
            .slots
            .iter()
            .zip(&live)
            .filter(|(slot, live)| slot.is_some() && !**live)
            .count();
        if dead == 0 {
            continue;
        }
        removed += dead;
        let Some(last_live) = live.iter().rposition(|live| *live) else {
            if tuple.fresh {
                let value = &source[tuple.initializer.start as usize..tuple.initializer.end as usize];
                edits.push(analysis::Edit::replace(tuple.statement, format!("{value};")));
            } else {
                edits.push(analysis::Edit::replace(
                    tuple.pattern,
                    pattern_text(source, &tuple.slots, &live, tuple.slots.len()),
                ));
            }
            continue;
        };
        let length = if tuple.fresh { last_live + 1 } else { tuple.slots.len() };
        edits.push(analysis::Edit::replace(
            tuple.pattern,
            pattern_text(source, &tuple.slots, &live, length),
        ));
    }
    let tuple_edits = edits.len();
    for candidate in scan.candidates {
        if scoping
            .get_resolved_references(candidate.symbol)
            .next()
            .is_some()
        {
            continue;
        }
        if candidate.pure {
            edits.push(analysis::Edit::delete(whole_line(
                source,
                candidate.statement,
            )));
        } else {
            let value =
                &source[candidate.initializer.start as usize..candidate.initializer.end as usize];
            let trimmed = value.trim_start();
            let needs_parens = trimmed.starts_with('{')
                || trimmed.starts_with("function")
                || trimmed.starts_with("class");
            let replacement = if needs_parens {
                format!("({value});")
            } else {
                format!("{value};")
            };
            if replacement.len() >= (candidate.statement.end - candidate.statement.start) as usize {
                continue;
            }
            edits.push(analysis::Edit::replace(candidate.statement, replacement));
        }
    }
    if edits.is_empty() {
        return Ok(DeadSsaResult {
            code: source.into(),
            removed: 0,
        });
    }
    removed += edits.len() - tuple_edits;
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("dead SSA edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("dead SSA output: {error}"))?;
    Ok(DeadSsaResult {
        code: applied.output,
        removed,
    })
}

/// Remove a bare generated local read after its declaration has executed in
/// the same block. A direct read has no effect once the lexical binding is
/// initialized. In particular, this consumes the read expression left when
/// an unread SSA copy of a branch-joined carry is removed.
pub(crate) fn remove_noop_local_reads(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scan = NoopReads {
        scoping: &scoping,
        reads: Vec::new(),
        dynamic_scope: false,
    };
    scan.visit_program(&program);
    if scan.dynamic_scope || scan.reads.is_empty() {
        return Ok((source.into(), 0));
    }
    let removed = scan.reads.len();
    let edits = scan
        .reads
        .into_iter()
        .map(|span| analysis::Edit::delete(whole_line(source, span)))
        .collect();
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("no-op local read edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("no-op local read output: {error}"))?;
    Ok((applied.output, removed))
}

/// Discard an unused numeric `arguments[index]` read only when the function's
/// arguments object never appears except as a direct numeric-index receiver.
/// With ordinary built-ins, no code in that function can install an accessor
/// on the unescaped arguments object.
pub(crate) fn remove_discarded_argument_reads(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut scan = DiscardedArguments {
        eligible: Vec::new(),
        reads: Vec::new(),
        dynamic_scope: false,
    };
    scan.visit_program(&program);
    if scan.dynamic_scope || scan.reads.is_empty() {
        return Ok((source.into(), 0));
    }
    let removed = scan.reads.len();
    let edits = scan
        .reads
        .into_iter()
        .map(|span| analysis::Edit::delete(whole_line(source, span)))
        .collect();
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("discarded argument read edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("discarded argument read output: {error}"))?;
    Ok((applied.output, removed))
}

struct DiscardedArguments {
    eligible: Vec<bool>,
    reads: Vec<Span>,
    dynamic_scope: bool,
}

impl DiscardedArguments {
    fn body(&mut self, statements: &[Statement<'_>]) {
        if self.eligible.last() != Some(&true) {
            return;
        }
        for statement in statements {
            let Statement::ExpressionStatement(expr) = statement else {
                continue;
            };
            let mut value = &expr.expression;
            while let Expression::ParenthesizedExpression(parenthesized) = value {
                value = &parenthesized.expression;
            }
            let Expression::ComputedMemberExpression(member) = value else {
                continue;
            };
            if matches!(&member.object, Expression::Identifier(id) if id.name == "arguments")
                && matches!(&member.expression, Expression::NumericLiteral(number)
                    if number.value >= 0.0 && number.value.fract() == 0.0)
            {
                self.reads.push(statement.span());
            }
        }
    }
}

// Regular functions own separate arguments objects. Arrows inherit the
// surrounding one; strings, comments and child functions' uses are unrelated.
#[derive(Default)]
struct ArgumentsUsage {
    invalid: bool,
    mutated: bool,
    receivers: BTreeSet<Span>,
}
fn arguments_receiver(mut e: &Expression<'_>) -> bool {
    while let Expression::ParenthesizedExpression(p) = e {
        e = &p.expression;
    }
    let object = match e {
        Expression::ComputedMemberExpression(m) => Some(&m.object),
        Expression::ChainExpression(c) => match &c.expression {
            ChainElement::ComputedMemberExpression(m) => Some(&m.object),
            _ => None,
        },
        _ => None,
    };
    object.is_some_and(|o| matches!(o,Expression::Identifier(i) if i.name=="arguments"))
}
impl<'a> Visit<'a> for ArgumentsUsage {
    fn visit_assignment_target(&mut self, target: &AssignmentTarget<'a>) {
        if let AssignmentTarget::ComputedMemberExpression(m) = target {
            self.mutated |=
                matches!(&m.object, Expression::Identifier(id) if id.name == "arguments");
        }
        walk::walk_assignment_target(self, target);
    }
    fn visit_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'a>) {
        if let SimpleAssignmentTarget::ComputedMemberExpression(m) = target {
            self.mutated |=
                matches!(&m.object, Expression::Identifier(id) if id.name == "arguments");
        }
        walk::walk_simple_assignment_target(self, target);
    }
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        self.invalid |= arguments_receiver(&c.callee);
        walk::walk_call_expression(self, c);
    }
    fn visit_tagged_template_expression(&mut self, t: &TaggedTemplateExpression<'a>) {
        self.invalid |= arguments_receiver(&t.tag);
        walk::walk_tagged_template_expression(self, t);
    }
    fn visit_function(&mut self, f: &Function<'a>, _: ScopeFlags) {
        self.invalid |= f.id.as_ref().is_some_and(|id| id.name == "arguments");
    }
    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        self.invalid |= id.name == "arguments";
    }
    fn visit_computed_member_expression(&mut self, m: &ComputedMemberExpression<'a>) {
        if let Expression::Identifier(id) = &m.object {
            if id.name == "arguments"
                && matches!(&m.expression,Expression::NumericLiteral(n) if n.value>=0.0 && n.value.fract()==0.0)
            {
                self.receivers.insert(id.span);
            }
        }
        walk::walk_computed_member_expression(self, m);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.invalid |= id.name == "arguments" && !self.receivers.contains(&id.span);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        self.invalid |= u.operator == oxc_syntax::operator::UnaryOperator::Delete;
        walk::walk_unary_expression(self, u);
    }
}

/// Stronger than the discarded-read proof: snapshots may only rematerialize
/// when no index can change, including through sloppy parameter mapping.
pub(crate) fn readonly_arguments(function: &Function<'_>) -> bool {
    function.params.items.is_empty()
        && function.params.rest.is_none()
        && !function
            .id
            .as_ref()
            .is_some_and(|id| id.name == "arguments")
        && function.body.as_ref().is_some_and(|body| {
            let mut usage = ArgumentsUsage::default();
            usage.visit_function_body(body);
            !usage.invalid && !usage.mutated
        })
}

impl<'a> Visit<'a> for DiscardedArguments {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let eligible = function.body.as_ref().is_some_and(|body| {
            let mut usage = ArgumentsUsage::default();
            usage.invalid |= function
                .id
                .as_ref()
                .is_some_and(|id| id.name == "arguments");
            usage.visit_formal_parameters(&function.params);
            usage.visit_function_body(body);
            !usage.invalid
        });
        self.eligible.push(eligible);
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
        self.eligible.pop();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.eligible.push(self.eligible.last() == Some(&true));
        if !arrow.expression {
            self.body(&arrow.body.statements);
        }
        walk::walk_arrow_function_expression(self, arrow);
        self.eligible.pop();
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}

struct NoopReads<'s> {
    scoping: &'s Scoping,
    reads: Vec<Span>,
    dynamic_scope: bool,
}

impl NoopReads<'_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        let mut initialized = BTreeSet::new();
        for statement in statements {
            match statement {
                Statement::VariableDeclaration(declaration) => {
                    for binding in &declaration.declarations {
                        if let BindingPattern::BindingIdentifier(id) = &binding.id {
                            if let Some(symbol) = id.symbol_id.get() {
                                initialized.insert(symbol);
                            }
                        }
                    }
                }
                Statement::ExpressionStatement(expr) => {
                    let Expression::Identifier(id) = &expr.expression else {
                        continue;
                    };
                    if !(id.name.starts_with("$KEY_") || id.name.starts_with("$CARRY_")) {
                        continue;
                    }
                    if analysis::identifier_symbol(id, self.scoping)
                        .is_some_and(|symbol| initialized.contains(&symbol))
                    {
                        self.reads.push(statement.span());
                    }
                }
                _ => {}
            }
        }
    }
}

impl<'a> Visit<'a> for NoopReads<'_> {
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

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}

struct Candidate {
    statement: Span,
    initializer: Span,
    symbol: SymbolId,
    pure: bool,
}

/// `let [a, , b] = init;`: each slot is a name (its span, symbol, and
/// whether it is a generated `$SSA_`/`$CARRY_` name) or a hole.
struct TupleCandidate {
    statement: Span,
    pattern: Span,
    initializer: Span,
    slots: Vec<Option<(Span, SymbolId, bool)>>,
    fresh: bool,
}

/// The pattern with unread slots as holes, cut to `length` slots. A trailing
/// hole needs its own comma, or the pattern would lose a slot.
fn pattern_text(
    source: &str,
    slots: &[Option<(Span, SymbolId, bool)>],
    live: &[bool],
    length: usize,
) -> String {
    let parts: Vec<&str> = slots[..length]
        .iter()
        .zip(live)
        .map(|(slot, live)| match slot {
            Some((span, _, _)) if *live => &source[span.start as usize..span.end as usize],
            _ => "",
        })
        .collect();
    let mut text = format!("[{}", parts.join(", "));
    if parts.last().is_some_and(|part| part.is_empty()) {
        text.push(',');
    }
    text.push(']');
    text
}

/// Generated helpers that return a fresh dense array literal.
#[derive(Default)]
struct FreshHelpers {
    symbols: BTreeSet<SymbolId>,
}

impl<'a> Visit<'a> for FreshHelpers {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some((symbol, _)) = fresh_tuple_helper(function) {
            self.symbols.insert(symbol);
        }
        walk::walk_function(self, function, flags);
    }
}

struct Scan<'s> {
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'s>,
    candidates: Vec<Candidate>,
    tuples: Vec<TupleCandidate>,
    fresh_helpers: BTreeSet<SymbolId>,
    dynamic_scope: bool,
}

impl Scan<'_> {
    fn statements(&mut self, statements: &[Statement<'_>]) {
        for (index, statement) in statements.iter().enumerate() {
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
            if let (BindingPattern::ArrayPattern(pattern), Some(initializer)) =
                (&binding.id, binding.init.as_ref())
            {
                self.tuple(statement, pattern, initializer);
                continue;
            }
            let BindingPattern::BindingIdentifier(name) = &binding.id else {
                continue;
            };
            if !(name.name.starts_with("$SSA_") || name.name.starts_with("$CARRY_")) {
                continue;
            }
            let (Some(symbol), Some(initializer)) = (name.symbol_id.get(), binding.init.as_ref())
            else {
                continue;
            };
            self.candidates.push(Candidate {
                statement: statement.span(),
                initializer: initializer.span(),
                symbol,
                pure: expression_is_pure(initializer)
                    || self.fresh_tuple_read(initializer)
                    || (index > 0
                        && matches!(initializer, Expression::Identifier(id) if previous_initializes_binding(&statements[index - 1], id, self.scoping))),
            });
        }
    }
}

impl Scan<'_> {
    /// `t[k]` of a never-written `t` that a fresh-tuple helper call
    /// initialized: an own index of a fresh dense array (or, past its
    /// length, `undefined` through the ordinary prototypes), so reading it
    /// runs no code and cannot throw.
    fn fresh_tuple_read(&self, initializer: &Expression<'_>) -> bool {
        let Expression::ComputedMemberExpression(member) = initializer else {
            return false;
        };
        let (Expression::Identifier(object), Expression::NumericLiteral(index)) =
            (&member.object, &member.expression)
        else {
            return false;
        };
        if member.optional || index.value < 0.0 || index.value.fract() != 0.0 {
            return false;
        }
        let Some(tuple) = self.scoping.get_reference(object.reference_id()).symbol_id() else {
            return false;
        };
        if self.scoping.symbol_is_mutated(tuple) {
            return false;
        }
        let declaration = self.scoping.symbol_declaration(tuple);
        let oxc_ast::AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) else {
            return false;
        };
        if declarator.kind == VariableDeclarationKind::Var {
            return false;
        }
        let Some(Expression::CallExpression(call)) = &declarator.init else {
            return false;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return false;
        };
        self.scoping
            .get_reference(callee.reference_id())
            .symbol_id()
            .is_some_and(|helper| self.fresh_helpers.contains(&helper))
    }

    fn tuple(
        &mut self,
        statement: &Statement<'_>,
        pattern: &ArrayPattern<'_>,
        initializer: &Expression<'_>,
    ) {
        if pattern.rest.is_some() {
            return;
        }
        let mut slots = Vec::new();
        for element in &pattern.elements {
            match element {
                None => slots.push(None),
                Some(BindingPattern::BindingIdentifier(id)) => {
                    let Some(symbol) = id.symbol_id.get() else {
                        return;
                    };
                    let generated = id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_");
                    slots.push(Some((id.span, symbol, generated)));
                }
                Some(_) => return,
            }
        }
        let fresh = matches!(initializer, Expression::CallExpression(call)
            if matches!(&call.callee, Expression::Identifier(callee)
                if analysis::identifier_symbol(callee, self.scoping)
                    .is_some_and(|symbol| self.fresh_helpers.contains(&symbol))));
        self.tuples.push(TupleCandidate {
            statement: statement.span(),
            pattern: pattern.span,
            initializer: initializer.span(),
            slots,
            fresh,
        });
    }
}

/// Reaching the next statement proves this lexical read cannot be in the TDZ.
/// The previous statement writes the very same resolved binding, and no
/// branch, call, or loop can intervene between the two statements.
fn previous_initializes_binding(
    previous: &Statement<'_>,
    read: &IdentifierReference<'_>,
    scoping: &Scoping,
) -> bool {
    let Some(read_symbol) = analysis::identifier_symbol(read, scoping) else {
        return false;
    };
    match previous {
        Statement::VariableDeclaration(declaration) if declaration.declarations.len() == 1 => {
            matches!(&declaration.declarations[0].id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(read_symbol))
        }
        Statement::ExpressionStatement(statement) => {
            let Expression::AssignmentExpression(assignment) = &statement.expression else {
                return false;
            };
            if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
                return false;
            }
            matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(id) if analysis::identifier_symbol(id, scoping) == Some(read_symbol))
        }
        _ => false,
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_program(&mut self, program: &Program<'a>) {
        self.statements(&program.body);
        walk::walk_program(self, program);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &function.body {
            self.statements(&body.statements);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.statements(&block.body);
        walk::walk_block_statement(self, block);
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{remove_dead_ssa, remove_discarded_argument_reads, remove_noop_local_reads};

    #[test]
    fn removes_unread_literal_but_preserves_call_and_throwing_read() {
        let source = "function f(x) {\n  let $SSA_1 = 42;\n  let $SSA_2 = effect();\n  let $SSA_3 = x.value;\n  return 1;\n}";
        let result = remove_dead_ssa(source).unwrap();
        assert_eq!(result.removed, 3);
        assert!(!result.code.contains("$SSA_1"));
        assert!(result.code.contains("effect();"));
        assert!(result.code.contains("x.value;"));
    }

    #[test]
    fn holes_unread_tuple_slots_and_trims_fresh_helper_tuples() {
        let helper = "function $VMHELPER_1(a, b) {\n  return [b, a, a];\n}\n";
        let source = format!(
            "{helper}function f(x) {{\n  let [$SSA_1, $SSA_2, $SSA_3] = $VMHELPER_1(x, 1);\n  let [$SSA_4, $SSA_5] = $VMHELPER_1(x, 2);\n  let [$SSA_6, $SSA_7, $SSA_8] = $VMHELPER_1(x, 3);\n  return $SSA_2 + $SSA_7;\n}}"
        );
        let result = remove_dead_ssa(&source).unwrap();
        assert_eq!(result.removed, 6, "{}", result.code);
        assert!(result.code.contains("let [, $SSA_2] = $VMHELPER_1(x, 1);"), "{}", result.code);
        assert!(result.code.contains("\n  $VMHELPER_1(x, 2);\n"), "{}", result.code);
        assert!(result.code.contains("let [, $SSA_7] = $VMHELPER_1(x, 3);"), "{}", result.code);
    }

    #[test]
    fn removes_an_unread_read_of_a_fresh_tuple_slot() {
        let helper = "function $VMHELPER_1(a, b) {\n  return [b, a, a()];\n}\n";
        let source = format!(
            "{helper}function f(x, k) {{\n  let $SSA_1 = $VMHELPER_1(k, x);\n  let $SSA_2 = $SSA_1[3];\n  let $SSA_3 = $SSA_1[0];\n  let $SSA_4 = x[3];\n  return $SSA_3;\n}}"
        );
        let result = remove_dead_ssa(&source).unwrap();
        assert!(!result.code.contains("$SSA_1[3]"), "{}", result.code);
        // Another object's slot may throw: kept as a read.
        assert!(result.code.contains("\n  x[3];\n"), "{}", result.code);
    }

    #[test]
    fn keeps_slot_count_for_other_tuple_sources() {
        let source = "function f(x) {\n  let [$SSA_1, $SSA_2, $SSA_3] = x;\n  let [$SSA_4, $SSA_5] = x;\n  let [y, $SSA_6] = x;\n  return $SSA_1 + y;\n}";
        let result = remove_dead_ssa(source).unwrap();
        assert_eq!(result.removed, 5, "{}", result.code);
        assert!(result.code.contains("let [$SSA_1, , ,] = x;"), "{}", result.code);
        assert!(result.code.contains("let [, ,] = x;"), "{}", result.code);
        assert!(result.code.contains("let [y, ,] = x;"), "{}", result.code);
    }

    #[test]
    fn keeps_iterator_steps_when_the_code_replaces_the_array_iterator() {
        let source = "function $VMHELPER_1() {\n  return [1, 2, 3];\n}\nfunction f(g) {\n  Array.prototype[Symbol.iterator] = g;\n  let [$SSA_1, $SSA_2, $SSA_3] = $VMHELPER_1();\n  return $SSA_1;\n}";
        let result = remove_dead_ssa(source).unwrap();
        assert!(result.code.contains("let [$SSA_1, , ,] = $VMHELPER_1();"), "{}", result.code);
    }

    #[test]
    fn keeps_tuples_from_reassigned_helpers_or_with_patterns() {
        let source = "function $VMHELPER_1() {\n  return [1, 2];\n}\nfunction f(x) {\n  $VMHELPER_1 = g;\n  let [$SSA_1, $SSA_2] = $VMHELPER_1();\n  let [$SSA_3 = 1, $SSA_4] = x;\n  let [$SSA_5, ...$SSA_6] = x;\n  return $SSA_1;\n}";
        let result = remove_dead_ssa(source).unwrap();
        assert_eq!(result.removed, 1, "{}", result.code);
        assert!(result.code.contains("let [$SSA_1, ,] = $VMHELPER_1();"), "{}", result.code);
        assert!(result.code.contains("let [$SSA_3 = 1, $SSA_4] = x;"));
        assert!(result.code.contains("let [$SSA_5, ...$SSA_6] = x;"));
    }

    #[test]
    fn retains_live_writes_and_dynamic_scope() {
        let source = "function f() { let $SSA_1 = effect(); $SSA_1 = 2; return $SSA_1; }";
        assert_eq!(remove_dead_ssa(source).unwrap().removed, 0);
        let dynamic = "function f() { let $SSA_1 = effect(); return eval('$SSA_1'); }";
        assert_eq!(remove_dead_ssa(dynamic).unwrap().removed, 0);
    }

    #[test]
    fn removes_unread_slot_read_after_its_direct_write() {
        let source =
            "function f() { let $KEY_1; $KEY_1 = effect(); let $SSA_1 = $KEY_1; return 1; }";
        let result = remove_dead_ssa(source).unwrap();
        assert_eq!(result.removed, 1, "{}", result.code);
        assert_eq!(result.code.matches("$KEY_1;").count(), 1, "{}", result.code);
        assert!(result.code.contains("$KEY_1 = effect();"));
    }

    #[test]
    fn retains_read_that_may_be_in_tdz_or_unresolved() {
        let tdz = "function f() { if (ready()) $KEY_1 = 1; let $SSA_1 = $KEY_1; let $KEY_1; }";
        assert_eq!(
            remove_dead_ssa(tdz)
                .unwrap()
                .code
                .matches("$KEY_1;")
                .count(),
            2
        );
        let free = "function f() { globalValue = 1; let $SSA_1 = globalValue; }";
        assert!(remove_dead_ssa(free).unwrap().code.contains("globalValue;"));
    }

    #[test]
    fn removes_bare_carry_read_after_a_branch_join() {
        let source = "function f(test) { let $CARRY_1; if (test) { $CARRY_1 = 1; } else { $CARRY_1 = 2; } $CARRY_1; return $CARRY_1; }";
        let (code, removed) = remove_noop_local_reads(source).unwrap();
        assert_eq!(removed, 1, "{code}");
        assert!(!code.contains("$CARRY_1; return"), "{code}");
        assert!(code.contains("return $CARRY_1"));
    }

    #[test]
    fn retains_bare_read_before_local_initialization() {
        let source = "function f() { $CARRY_1; let $CARRY_1; }";
        assert_eq!(remove_noop_local_reads(source).unwrap().1, 0);
    }

    #[test]
    fn removes_discarded_indexed_argument_reads_when_object_does_not_escape() {
        let source = "function f() { let x = arguments[0]; (arguments[1]); return x; }";
        let (code, removed) = remove_discarded_argument_reads(source).unwrap();
        assert_eq!(removed, 1, "{code}");
        assert!(!code.contains("(arguments[1]);"));
    }

    #[test]
    fn retains_argument_read_when_object_escapes_or_can_gain_accessor() {
        let escape = "function f() { use(arguments); (arguments[1]); }";
        assert_eq!(remove_discarded_argument_reads(escape).unwrap().1, 0);
        let mutate = "function f() { Object.defineProperty(arguments, '1', {get: effect}); (arguments[1]); }";
        assert_eq!(remove_discarded_argument_reads(mutate).unwrap().1, 0);
        let param = "function f(arguments) { (arguments[1]); }";
        assert_eq!(remove_discarded_argument_reads(param).unwrap().1, 0);
    }
}
