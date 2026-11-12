//! Lift a carried loop counter out of a `while` body into the `for` header.
//!
//! After loop reconstruction a counted loop reads as
//!
//! ```js
//! let $CARRY_i = 0;
//! while ($CARRY_i < n) {
//! 	use($CARRY_i);
//! 	$CARRY_i = Number($CARRY_i) + 1;
//! }
//! ```
//!
//! The counter is declared outside the loop and written at its back edge, so
//! it is a loop carry. When every reference to the counter stays inside the
//! loop, in the same function, and the body ends in exactly one assignment to
//! it, the `while` becomes
//!
//! ```js
//! let $CARRY_i = 0;
//! for (; $CARRY_i < n; $CARRY_i = Number($CARRY_i) + 1) {
//! 	use($CARRY_i);
//! }
//! ```
//!
//! The binding stays where it is, so its initializer keeps its position and
//! effects; only the back-edge update moves from the body to the header, which
//! runs at the same point (`break` leaves both, and a `continue` that would
//! skip the update declines the rewrite). The update's operands must still be
//! in scope there, so a read of a body-local binding declines. This is the
//! shape the loop-naming stage already recognizes, so the counter then takes
//! an `i`/`j`/`k` name like every other `for` counter.
//!
//! A loop usually stores its other carried values right after the counter, so
//! the update need not be the last statement: it may be lifted past trailing
//! statements when they are plain stores to other bindings of a value read
//! without a call, a member read, an assignment or an update, that neither
//! write the counter nor read into the update. Reordering those past the
//! update is only invisible when the update itself cannot throw — a bound
//! identifier or literal, or, for a counter whose stores are all numeric,
//! `Number(counter)` and arithmetic over it.
//!
//! Reading a member or calling a helper is left untouched; the pass assumes
//! only standard built-ins, the same assumption the rest of the pipeline
//! makes. `with`/`eval` disables it.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, ReferenceId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, UnaryOperator};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;
use super::super::support::whole_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoopCountersResult {
    pub code: String,
    pub lifted: usize,
}

pub(crate) fn lift_loop_counters(source: &str) -> Result<LoopCountersResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        functions: Vec::new(),
        candidates: Vec::new(),
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok(unchanged(source));
    }
    let mut edits = Vec::new();
    let mut lifted = 0;
    for candidate in &scan.candidates {
        if !candidate.acceptable(&scoping, &nodes) {
            continue;
        }
        let header = candidate.header;
        let header_text = format!(
            "for (; {}; {} = {}) ",
            candidate.test, candidate.name, candidate.rhs,
        );
        let proposal = [
            analysis::Edit::replace(header, header_text),
            analysis::Edit::delete(whole_line(source, candidate.update_span)),
        ];
        if proposal.iter().any(|edit| {
            edits
                .iter()
                .any(|other: &analysis::Edit| edit.start < other.end && other.start < edit.end)
        }) {
            continue;
        }
        edits.extend(proposal);
        lifted += 1;
    }
    if lifted == 0 {
        return Ok(unchanged(source));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("loop counter edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("loop counter output: {error}"))?;
    Ok(LoopCountersResult {
        code: applied.output,
        lifted,
    })
}

fn unchanged(source: &str) -> LoopCountersResult {
    LoopCountersResult {
        code: source.into(),
        lifted: 0,
    }
}

/// A `while` loop whose body ends in an assignment to a carried counter.
struct Candidate {
    while_span: Span,
    /// `while (test)` up to the body's opening brace.
    header: Span,
    update_span: Span,
    update_symbol: Option<SymbolId>,
    name: String,
    test: String,
    rhs: String,
    /// A `continue` in the body would skip the update once it is in the header.
    skips_update: bool,
    /// Symbols read by the update's right-hand side, and the function they and
    /// the loop sit in.
    rhs_symbols: Vec<SymbolId>,
    function: Option<Span>,
    /// Statements after the update that it is lifted past; moving it past them
    /// must not change whether its own evaluation throws.
    reorders_past_stores: bool,
    /// Whether the update's right-hand side cannot throw on its own.
    update_total: bool,
}

impl Candidate {
    fn acceptable(&self, scoping: &Scoping, nodes: &AstNodes<'_>) -> bool {
        if self.skips_update || !self.name.starts_with('$') {
            return false;
        }
        let Some(symbol) = self.update_symbol else {
            return false;
        };
        // The counter must be a generated `let`/`var` local declared before the
        // loop, so moving its update into the header cannot change its scope.
        let declaration = scoping.symbol_declaration(symbol);
        if !matches!(nodes.kind(declaration), AstKind::VariableDeclarator(_))
            || !matches!(nodes.parent_kind(declaration), AstKind::VariableDeclaration(declaration)
                if matches!(declaration.kind, VariableDeclarationKind::Let | VariableDeclarationKind::Var))
        {
            return false;
        }
        let declared = scoping.symbol_span(symbol);
        if declared.end > self.while_span.start {
            return false;
        }
        // Every reference stays inside the loop, in the loop's own function, so
        // moving the update cannot change capture or post-loop reads. The update
        // must also be the last write to the counter, or a later write would
        // run before it once it moves into the header.
        for reference in scoping.get_resolved_references(symbol) {
            let node = reference.node_id();
            let span = nodes.kind(node).span();
            if !contains(self.while_span, span) || enclosing_function(node, nodes) != self.function {
                return false;
            }
            if reference.is_write() && span.start >= self.update_span.end {
                return false;
            }
        }
        // Reordering the update past later stores is only invisible when its
        // own evaluation cannot throw.
        if self.reorders_past_stores && !self.update_total {
            return false;
        }
        // The update's operands must still resolve in the header, so nothing it
        // reads may be declared inside the loop body.
        self.rhs_symbols
            .iter()
            .all(|read| scoping.symbol_span(*read).start < self.while_span.start)
    }
}

/// Whether evaluating `expression` cannot throw and reads nothing a trailing
/// store could change: a bound identifier that no other function writes (a
/// trailing store's `valueOf` could run such a closure), a literal, or, when
/// the counter provably holds a number, `Number(counter)` and arithmetic over
/// the counter.
fn update_is_total(
    expression: &Expression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    symbol: SymbolId,
) -> bool {
    match expression {
        Expression::Identifier(identifier) => identifier
            .reference_id
            .get()
            .and_then(|reference| scoping.get_reference(reference).symbol_id())
            .is_some_and(|read| {
                let home = enclosing_function(scoping.symbol_declaration(read), nodes);
                scoping
                    .get_resolved_references(read)
                    .filter(|reference| reference.is_write())
                    .all(|reference| enclosing_function(reference.node_id(), nodes) == home)
            }),
        Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::RegExpLiteral(_) => true,
        Expression::ParenthesizedExpression(parenthesized) => {
            update_is_total(&parenthesized.expression, scoping, nodes, symbol)
        }
        _ => counter_is_number(scoping, nodes, symbol) && total_number(expression, scoping, symbol),
    }
}

/// A counter whose only stores are numeric: a numeric-literal initializer and
/// writes of `Number(counter)`, the counter itself and arithmetic over
/// numbers. `Number` and arithmetic then cannot throw on any value it ever
/// holds, so reordering its update past side-effect-free stores is invisible.
fn counter_is_number(scoping: &Scoping, nodes: &AstNodes<'_>, symbol: SymbolId) -> bool {
    let declaration = scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) else {
        return false;
    };
    if !matches!(declarator.init, Some(Expression::NumericLiteral(_))) {
        return false;
    }
    scoping.get_resolved_references(symbol).all(|reference| {
        if !reference.is_write() {
            return true;
        }
        match nodes.parent_kind(reference.node_id()) {
            AstKind::AssignmentExpression(assignment) => {
                total_number(&assignment.right, scoping, symbol)
            }
            _ => false,
        }
    })
}

/// An expression that cannot throw when `symbol` holds a number: numeric
/// literals, the counter, `Number(x)` of one, and arithmetic over those.
fn total_number(expression: &Expression<'_>, scoping: &Scoping, symbol: SymbolId) -> bool {
    match expression {
        Expression::NumericLiteral(_) => true,
        Expression::Identifier(identifier) => {
            identifier
                .reference_id
                .get()
                .and_then(|reference| scoping.get_reference(reference).symbol_id())
                == Some(symbol)
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            total_number(&parenthesized.expression, scoping, symbol)
        }
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot
            ) && total_number(&unary.argument, scoping, symbol)
        }
        Expression::BinaryExpression(binary) => {
            arithmetic(binary.operator)
                && total_number(&binary.left, scoping, symbol)
                && total_number(&binary.right, scoping, symbol)
        }
        Expression::CallExpression(call) => {
            !call.optional
                && matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == "Number")
                && call.arguments.len() == 1
                && call.arguments[0]
                    .as_expression()
                    .is_some_and(|argument| total_number(argument, scoping, symbol))
        }
        _ => false,
    }
}

fn arithmetic(operator: oxc_syntax::operator::BinaryOperator) -> bool {
    use oxc_syntax::operator::BinaryOperator::*;
    matches!(
        operator,
        Addition
            | Subtraction
            | Multiplication
            | Division
            | Remainder
            | Exponential
            | BitwiseAnd
            | BitwiseOR
            | BitwiseXOR
            | ShiftLeft
            | ShiftRight
            | ShiftRightZeroFill
    )
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    functions: Vec<Span>,
    candidates: Vec<Candidate>,
    dynamic: bool,
}

impl Scan<'_, '_> {
    fn loop_body(&mut self, statement: &WhileStatement<'_>) {
        let Statement::BlockStatement(body) = &statement.body else {
            return;
        };
        for (index, candidate) in body.body.iter().enumerate().rev() {
            let Statement::ExpressionStatement(expression) = candidate else {
                continue;
            };
            let Expression::AssignmentExpression(assign) = &expression.expression else {
                continue;
            };
            if assign.operator != AssignmentOperator::Assign {
                continue;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
                continue;
            };
            let Some(symbol) = target
                .reference_id
                .get()
                .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
            else {
                continue;
            };
            // Only the counter the loop tests is a loop control: a trailing
            // store is lifted by the next step of the scan instead.
            let mut tested = Reads::default();
            tested.visit_expression(&statement.test);
            if !tested.symbols.iter().any(|reference| {
                self.scoping.get_reference(*reference).symbol_id() == Some(symbol)
            }) {
                continue;
            }
            let mut reads = Reads::default();
            reads.visit_expression(&assign.right);
            let rhs_symbols: Vec<SymbolId> = reads
                .symbols
                .iter()
                .filter_map(|reference| self.scoping.get_reference(*reference).symbol_id())
                .collect();
            // The counter update is the last such statement; any carries stored
            // after it must be reorderable past it.
            let trailing = &body.body[index + 1..];
            let mut trailing_ok = true;
            for statement in trailing {
                trailing_ok &= self.trailing_store(statement, symbol, &rhs_symbols);
            }
            if !trailing_ok {
                continue;
            }
            let mut jumps = Jumps::default();
            jumps.visit_statement(&statement.body);
            self.candidates.push(Candidate {
                while_span: statement.span,
                header: Span::new(statement.span.start, body.span.start),
                update_span: candidate.span(),
                update_symbol: Some(symbol),
                name: target.name.to_string(),
                test: self.text(statement.test.span()).to_string(),
                rhs: self.text(assign.right.span()).to_string(),
                skips_update: jumps.skips_update,
                rhs_symbols,
                function: self.functions.last().copied(),
                reorders_past_stores: !trailing.is_empty(),
                update_total: update_is_total(&assign.right, self.scoping, self.nodes, symbol),
            });
            return;
        }
    }

    /// A statement after the counter update that may be reordered past it: a
    /// plain store to another binding with no call, no member read and no
    /// second write, that neither touches the counter nor reads a value the
    /// update reads.
    fn trailing_store(
        &self,
        statement: &Statement<'_>,
        counter: SymbolId,
        update_reads: &[SymbolId],
    ) -> bool {
        let Statement::ExpressionStatement(expression) = statement else {
            return false;
        };
        let Expression::AssignmentExpression(assign) = &expression.expression else {
            return false;
        };
        if assign.operator != AssignmentOperator::Assign {
            return false;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
            return false;
        };
        if !side_effect_free(&assign.right) {
            return false;
        }
        if target
            .reference_id
            .get()
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
            .is_some_and(|symbol| symbol == counter || update_reads.contains(&symbol))
        {
            return false;
        }
        let mut reads = Reads::default();
        reads.visit_expression(&assign.right);
        !reads.symbols.iter().any(|reference| {
            self.scoping.get_reference(*reference).symbol_id() == Some(counter)
        })
    }

    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }
}

/// Whether an expression is a read of already-held values: no call, no member
/// access (which may run a getter), no assignment or update, no allocation of
/// a spread. Used to move a statement past another without changing order.
fn side_effect_free(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Identifier(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::RegExpLiteral(_) => true,
        Expression::TemplateLiteral(template) => {
            template.expressions.iter().all(side_effect_free)
        }
        Expression::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::Delete => false,
            _ => side_effect_free(&unary.argument),
        },
        Expression::BinaryExpression(binary) => {
            !binary.operator.is_in()
                && !binary.operator.is_instance_of()
                && side_effect_free(&binary.left)
                && side_effect_free(&binary.right)
        }
        Expression::LogicalExpression(logical) => {
            side_effect_free(&logical.left) && side_effect_free(&logical.right)
        }
        Expression::ConditionalExpression(conditional) => {
            side_effect_free(&conditional.test)
                && side_effect_free(&conditional.consequent)
                && side_effect_free(&conditional.alternate)
        }
        Expression::SequenceExpression(sequence) => {
            sequence.expressions.iter().all(side_effect_free)
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            side_effect_free(&parenthesized.expression)
        }
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
        _ => false,
    }
}

/// Record whether a `continue` in a loop body targets the loop itself.
#[derive(Default)]
struct Jumps {
    loop_depth: usize,
    skips_update: bool,
}

impl<'a> Visit<'a> for Jumps {
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_continue_statement(&mut self, statement: &ContinueStatement<'a>) {
        if statement.label.is_none() && self.loop_depth == 0 {
            self.skips_update = true;
        }
    }
    fn visit_for_statement(&mut self, loop_: &ForStatement<'a>) {
        self.loop_depth += 1;
        walk::walk_for_statement(self, loop_);
        self.loop_depth -= 1;
    }
    fn visit_for_in_statement(&mut self, loop_: &ForInStatement<'a>) {
        self.loop_depth += 1;
        walk::walk_for_in_statement(self, loop_);
        self.loop_depth -= 1;
    }
    fn visit_for_of_statement(&mut self, loop_: &ForOfStatement<'a>) {
        self.loop_depth += 1;
        walk::walk_for_of_statement(self, loop_);
        self.loop_depth -= 1;
    }
    fn visit_while_statement(&mut self, loop_: &WhileStatement<'a>) {
        self.loop_depth += 1;
        walk::walk_while_statement(self, loop_);
        self.loop_depth -= 1;
    }
    fn visit_do_while_statement(&mut self, loop_: &DoWhileStatement<'a>) {
        self.loop_depth += 1;
        walk::walk_do_while_statement(self, loop_);
        self.loop_depth -= 1;
    }
}

/// The symbols an update expression reads; a nested function is not an operand
/// of the update and its reads are not moved.
#[derive(Default)]
struct Reads {
    symbols: Vec<ReferenceId>,
}

impl<'a> Visit<'a> for Reads {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if let Some(reference) = identifier.reference_id.get() {
            self.symbols.push(reference);
        }
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(function.span);
        walk::walk_function(self, function, flags);
        self.functions.pop();
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push(arrow.span);
        walk::walk_arrow_function_expression(self, arrow);
        self.functions.pop();
    }
    fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
        self.loop_body(statement);
        walk::walk_while_statement(self, statement);
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic = true;
        }
    }
}

/// The innermost function or arrow containing `node`.
fn enclosing_function(node: oxc_semantic::NodeId, nodes: &AstNodes<'_>) -> Option<Span> {
    nodes.ancestor_ids(node).find_map(|ancestor| match nodes.kind(ancestor) {
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => Some(nodes.kind(ancestor).span()),
        _ => None,
    })
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

#[cfg(test)]
mod tests {
    use super::lift_loop_counters;

    /// Evaluate `f()` of a source in Node and return its printed result.
    fn run(source: &str) -> String {
        let output = std::process::Command::new("node")
            .args(["-e", &format!("{source};console.log(JSON.stringify(f()));")])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{source}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    /// Every lifted loop must run identically to its source.
    fn equivalent(source: &str, lifted: usize) -> String {
        let result = lift_loop_counters(source).unwrap();
        assert_eq!(result.lifted, lifted, "{}", result.code);
        assert_eq!(run(source), run(&result.code), "{}", result.code);
        result.code
    }

    #[test]
    fn lifts_a_counted_while_into_a_for_header() {
        let source = "function f(a) { let $CARRY_1 = 0; while ($CARRY_1 < a.length) { use(a[$CARRY_1]); $CARRY_1 = Number($CARRY_1) + 1; } }";
        let result = lift_loop_counters(source).unwrap();
        assert_eq!(result.lifted, 1, "{}", result.code);
        assert!(
            result
                .code
                .contains("for (; $CARRY_1 < a.length; $CARRY_1 = Number($CARRY_1) + 1)"),
            "{}",
            result.code
        );
        assert!(!result.code.contains("while"), "{}", result.code);
    }

    #[test]
    fn lifts_an_index_guard_call() {
        let source = "function f(x) { let $CARRY_2 = 0; while ($VMHELPER_274($CARRY_2, x)) { use($CARRY_2); $CARRY_2 = Number($CARRY_2) + 1; } }";
        let result = lift_loop_counters(source).unwrap();
        assert_eq!(result.lifted, 1, "{}", result.code);
        assert!(
            result.code.contains(
                "for (; $VMHELPER_274($CARRY_2, x); $CARRY_2 = Number($CARRY_2) + 1)"
            ),
            "{}",
            result.code
        );
    }

    #[test]
    fn declines_when_the_counter_is_read_after_the_loop() {
        let source = "function f(a) { let $CARRY_1 = 0; while ($CARRY_1 < 3) { $CARRY_1 = $CARRY_1 + 1; } return $CARRY_1; }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn declines_when_the_counter_is_captured() {
        let source = "function f(a) { let $CARRY_1 = 0; while ($CARRY_1 < 3) { let g = () => $CARRY_1; use(g); $CARRY_1 = $CARRY_1 + 1; } }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn declines_a_continue_that_would_skip_the_update() {
        let source = "function f(a) { let $CARRY_1 = 0; while ($CARRY_1 < 3) { if (a) continue; $CARRY_1 = $CARRY_1 + 1; } }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn declines_an_update_reading_a_body_local() {
        let source = "function f(a) { let $CARRY_1 = 0; while ($CARRY_1 < 3) { let step = 1; $CARRY_1 = $CARRY_1 + step; } }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn declines_a_non_generated_or_undeclared_counter() {
        for source in [
            "function f(a) { let i = 0; while (i < 3) { i = i + 1; } }",
            "function f(a) { var x; x = 1; $SSA_9 = 1; while ($SSA_9 < 3) { $SSA_9 = $SSA_9 + 1; } }",
            "function f(a) { const $CARRY_1 = 0; while ($CARRY_1 < 3) { use($CARRY_1); } }",
        ] {
            assert_eq!(lift_loop_counters(source).unwrap().lifted, 0, "{source}");
        }
    }

    #[test]
    fn declines_when_the_body_does_not_end_in_the_update() {
        let source = "function f(a) { let $CARRY_1 = 0; while ($CARRY_1 < 3) { $CARRY_1 = $CARRY_1 + 1; use($CARRY_1); } }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn lifts_past_a_trailing_carry_store() {
        let source = "function f(a) { let $CARRY_1 = 0; let $CARRY_2 = void 0; while ($CARRY_1 < a.length) { use(a[$CARRY_1]); $CARRY_1 = Number($CARRY_1) + 1; $CARRY_2 = 43847; } }";
        let result = lift_loop_counters(source).unwrap();
        assert_eq!(result.lifted, 1, "{}", result.code);
        assert!(
            result.code.contains("for (; $CARRY_1 < a.length; $CARRY_1 = Number($CARRY_1) + 1)"),
            "{}",
            result.code
        );
        assert!(result.code.contains("$CARRY_2 = 43847;"), "{}", result.code);
    }

    #[test]
    fn declines_an_identifier_update_a_closure_can_write() {
        // `$CARRY_3 = o + 1` may call `o.valueOf`, which can run `g`; once the
        // update moves after it, it would read the new `$KEY_1`.
        let source = "function f(a, o) { var $KEY_1 = 0; let g = () => { $KEY_1 = 5; }; let $CARRY_1 = 0; let $CARRY_3 = 0; while ($CARRY_1 < a) { use(g); $CARRY_1 = $KEY_1; $CARRY_3 = o + 1; } }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn declines_when_a_trailing_store_reads_the_counter() {
        let source = "function f(a) { let $CARRY_1 = 0; let $CARRY_2 = void 0; while ($CARRY_1 < a.length) { $CARRY_1 = Number($CARRY_1) + 1; $CARRY_2 = $CARRY_1; } }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn declines_when_a_later_nested_store_writes_the_counter() {
        let source = "function f(a) { let $CARRY_1 = 0; while ($CARRY_1 < a.length) { $CARRY_1 = Number($CARRY_1) + 1; $CARRY_2 = 1; { $CARRY_1 = 4; } } }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn declines_to_reorder_an_update_that_could_throw() {
        let source = "function f(a) { let $CARRY_1 = 0; let $CARRY_2 = void 0; while ($CARRY_1 < a.length) { $CARRY_1 = $KEY_90($CARRY_1); $CARRY_2 = 1; } }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn lifts_a_numbered_counter_past_a_trailing_store() {
        let source = "function f(a) { let $CARRY_1 = 1; let $CARRY_2 = 0; while ($CARRY_1 < a.length) { use($CARRY_1); $CARRY_1 = Number($CARRY_1) + 1; $CARRY_2 = 1; } }";
        let result = lift_loop_counters(source).unwrap();
        assert_eq!(result.lifted, 1, "{}", result.code);
        assert!(
            result.code.contains("for (; $CARRY_1 < a.length; $CARRY_1 = Number($CARRY_1) + 1)"),
            "{}",
            result.code
        );
    }

    #[test]
    fn declines_a_use_after_the_loop_in_a_sibling_closure() {
        let source = "function f(a) { let $CARRY_1 = 0; while ($CARRY_1 < 3) { $CARRY_1 = $CARRY_1 + 1; } let g = function () { return $CARRY_1; }; use(g); }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn declines_dynamic_scope() {
        let source = "function f(a) { let $CARRY_1 = 0; while ($CARRY_1 < 3) { $CARRY_1 = $CARRY_1 + 1; } eval('0'); }";
        assert_eq!(lift_loop_counters(source).unwrap().lifted, 0);
    }

    #[test]
    fn lifted_loops_run_like_their_source() {
        // A plain index loop.
        equivalent(
            "function f() { const a = [4, 5, 6]; let $CARRY_1 = 0; const out = []; while ($CARRY_1 < a.length) { out.push(a[$CARRY_1]); $CARRY_1 = Number($CARRY_1) + 1; } return out; }",
            1,
        );
        // A guard helper and a counter that starts at one.
        equivalent(
            "function $HELP(i, a) { return i < a.length; } function f() { const a = [4, 5, 6]; let $CARRY_1 = 1; const out = []; while ($HELP($CARRY_1, a)) { out.push(a[$CARRY_1]); $CARRY_1 = Number($CARRY_1) + 1; } return out; }",
            1,
        );
        // Other carries stored after the update.
        equivalent(
            "function f() { const a = [4, 5, 6]; let $CARRY_1 = 0; let $CARRY_2 = 0; const out = []; while ($CARRY_1 < a.length) { out.push(a[$CARRY_1] + $CARRY_2); $CARRY_1 = Number($CARRY_1) + 1; $CARRY_2 = $CARRY_2 + 2; } return out; }",
            1,
        );
        // A countdown loop with a side-effecting body.
        equivalent(
            "function f() { let log = []; let $CARRY_1 = 3; while ($CARRY_1 > 0) { log.push($CARRY_1); $CARRY_1 = Number($CARRY_1) - 1; } return log; }",
            1,
        );
        // The loop's other carried value is still read at exit.
        equivalent(
            "function f() { const a = [4, 5, 6]; let $CARRY_1 = 0; let $CARRY_2 = 0; while ($CARRY_1 < a.length) { $CARRY_2 = a[$CARRY_1]; $CARRY_1 = Number($CARRY_1) + 1; } return $CARRY_2; }",
            1,
        );
    }
}
