//! Packed flags: write-site shapes the shared derivation in [`super`] reads
//! through these helpers (a child module, so it uses the parent's private
//! helpers).
//!
//! * Reaching definitions: a definition followed by `return` / `throw` in a
//!   block that does not hold the read never reaches it (duplicated exit
//!   paths of one producer).
//! * Masked values: `buf[k] = v ^ m` with the mask draw bound first
//!   (`m = m()`), and the constant-zero arm `buf[k] = m ^ 0`.
//! * Shifts guarded by the deobfuscator: `x << (x === void 0 ? n : m)` is
//!   `x << m` (an undefined `x` shifts to 0 either way).
//! * Packing loops: the in-loop temp `t = acc | X[j] << c` with `acc = t`,
//!   the copy `b = acc ^ m()` of the tail byte, the flush test on
//!   `t = Number(c) + 1`, a loop the deobfuscator left without a flush (a
//!   list shorter than a byte), a register reused for another loop before the
//!   read, and probed lists filled by index (`list[i] = test(L[i])`) or
//!   handed to the packer through callbacks and config objects
//!   (`collect(config, document, (flags) => pack(flags))` with
//!   `queue((item, done) => { list.push(test(item)); done() }, config.items)`).
//! * Record labels: a boolean copied out of a record element (`v = rec.k[i]`),
//!   and the record construction that fits every indexed label read of the
//!   slot (a record storing a number under a label read as a list is not the
//!   one read).
//!
//! Nothing here reads salts, slot addresses, generated spellings or build
//! constants.

use std::collections::BTreeSet;

use oxc_ast::ast::{BinaryOperator, Expression};
use oxc_ast::AstKind;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::signals::analysis::write_sites::RecordConstruction;
use crate::signals::analysis::{FinderCtx, SlotView};
use super::{integer, nearest_flags, PackedTerm, Scope, Updates, TERM_HOPS};

/// Definitions and call sites followed while resolving a probed list.
const LIST_HOPS: usize = 16;

impl FinderCtx<'_, '_, '_> {
    // =======================================================================
    // Reaching definitions
    // =======================================================================

    /// `node` is followed by a `return` / `throw` in a block (or statement
    /// list) that does not contain `read`: control leaves the function before
    /// reaching the read.
    pub(crate) fn exits_before(&self, node: NodeId, read: NodeId) -> bool {
        let nodes = self.parse.nodes;
        let read_span = nodes.kind(read).span();
        let mut child = node;
        loop {
            let parent = nodes.parent_id(child);
            if parent == child {
                return false;
            }
            let span = nodes.kind(parent).span();
            if span.start <= read_span.start && read_span.end <= span.end {
                return false;
            }
            let after = nodes.kind(child).span().end;
            let exits = |statements: &oxc_allocator::Vec<'_, oxc_ast::ast::Statement<'_>>| {
                statements.iter().any(|statement| {
                    statement.span().start >= after
                        && matches!(
                            statement,
                            oxc_ast::ast::Statement::ReturnStatement(_) | oxc_ast::ast::Statement::ThrowStatement(_)
                        )
                })
            };
            match nodes.kind(parent) {
                AstKind::BlockStatement(block) if exits(&block.body) => return true,
                AstKind::FunctionBody(_) | AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_) => {
                    return false
                }
                _ => {}
            }
            child = parent;
        }
    }

    /// When the nearest definition `definition` of `symbol` cannot reach
    /// `read` ([`Self::exits_before`]), the latest plain definition before
    /// the read in the same function that does.
    pub(crate) fn reaching_definition(&self, symbol: SymbolId, definition: NodeId, read: NodeId) -> Option<NodeId> {
        if !self.exits_before(definition, read) {
            return None;
        }
        let nodes = self.parse.nodes;
        let read_at = nodes.kind(read).span().start;
        let function = self.enclosing_function(read);
        let mut candidates: Vec<NodeId> = self
            .parse
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .filter_map(|reference| {
                let parent = nodes.parent_id(reference.node_id());
                match nodes.kind(parent) {
                    AstKind::AssignmentExpression(assignment) if assignment.operator.is_assign() => Some(parent),
                    _ => None,
                }
            })
            .collect();
        let declaration = self.parse.scoping.symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) {
            if declarator.init.is_some() {
                candidates.push(declaration);
            }
        }
        candidates
            .into_iter()
            .filter(|candidate| nodes.kind(*candidate).span().end <= read_at)
            .filter(|candidate| self.enclosing_function(*candidate) == function)
            .filter(|candidate| !self.exits_before(*candidate, read))
            .max_by_key(|candidate| nodes.kind(*candidate).span().end)
    }

    // =======================================================================
    // Masked values and guarded shifts
    // =======================================================================

    /// The binding `node` reads is last defined as a zero-argument call (a
    /// mask draw `m = m()`).
    fn is_draw(&self, node: NodeId) -> bool {
        if !matches!(self.parse.nodes.kind(node), AstKind::IdentifierReference(_)) {
            return false;
        }
        match self.identifier_value(node) {
            Some((value, Updates::Or(ors))) if ors.is_empty() => {
                matches!(self.parse.nodes.kind(value), AstKind::CallExpression(call) if call.arguments.is_empty())
            }
            _ => false,
        }
    }

    /// The logical value of a written byte value: `v ^ m` with `m` a bound
    /// mask draw is `v`; a bound draw xor-ed with a literal (`m ^ 0`, the
    /// constant arm) is `None` (a literal byte, no bits).
    pub(super) fn site_value(&self, value: NodeId) -> Option<NodeId> {
        let nodes = self.parse.nodes;
        if let AstKind::BinaryExpression(binary) = nodes.kind(value) {
            if binary.operator == BinaryOperator::BitwiseXOR {
                // `v ^ k.call(void 0)` (the tracer's draw, `k` possibly an
                // argument-tuple slot): `v`.
                if super::mask_call(&binary.right) && !matches!(binary.right.without_parentheses(), Expression::CallExpression(call) if call.arguments.is_empty()) {
                    return self.expression_node(&binary.left).or(Some(value));
                }
                let (Some(left), Some(right)) = (self.expression_node(&binary.left), self.expression_node(&binary.right)) else {
                    return Some(value);
                };
                return match (self.is_draw(left), self.is_draw(right)) {
                    (false, true) => Some(left),
                    (true, false) => Some(right),
                    (true, true) => self.merged_xor(left, right).unwrap_or(Some(value)),
                    _ => Some(value),
                };
            }
        }
        if self.is_draw(value) {
            if let AstKind::BinaryExpression(binary) = nodes.kind(nodes.parent_id(value)) {
                let literal = |expression: &Expression<'_>| matches!(expression.without_parentheses(), Expression::NumericLiteral(_));
                if binary.operator == BinaryOperator::BitwiseXOR && (literal(&binary.left) || literal(&binary.right)) {
                    return None;
                }
            }
        }
        Some(value)
    }

    /// `buf[k] = a ^ b` after `if (…) { a = v; b = m() } else { a = m(); b = 0 }`
    /// (the two arms of one write merged): the packed arm's value `v`, or
    /// `None` when both arms write constants. `None` (outer): not this shape.
    fn merged_xor(&self, left: NodeId, right: NodeId) -> Option<Option<NodeId>> {
        let nodes = self.parse.nodes;
        let read_at = nodes.kind(left).span().start;
        let function = self.enclosing_function(left);
        let symbols = [self.reference_symbol(left)?, self.reference_symbol(right)?];
        // Plain writes before the read, in the read's function.
        let writes = |symbol: SymbolId| -> Vec<NodeId> {
            self.parse
                .scoping
                .get_resolved_references(symbol)
                .filter(|reference| reference.is_write())
                .filter_map(|reference| {
                    let parent = nodes.parent_id(reference.node_id());
                    match nodes.kind(parent) {
                        AstKind::AssignmentExpression(assignment) if assignment.operator.is_assign() => Some(parent),
                        _ => None,
                    }
                })
                .filter(|write| nodes.kind(*write).span().end <= read_at && self.enclosing_function(*write) == function)
                .collect()
        };
        let latest = writes(symbols[1]).into_iter().max_by_key(|write| nodes.kind(*write).span().end)?;
        // The `if` whose arm holds that write and not the read.
        let mut child = latest;
        let statement = loop {
            let parent = nodes.parent_id(child);
            if parent == child || Some(parent) == function {
                return None;
            }
            if let AstKind::IfStatement(statement) = nodes.kind(parent) {
                let span = statement.span;
                if span.end <= read_at {
                    break statement;
                }
            }
            child = parent;
        };
        let arms = [statement.consequent.span(), statement.alternate.as_ref()?.span()];
        // Per arm: the value of each side's latest write in it.
        let mut packed = None;
        let mut constant_arms = 0;
        for arm in arms {
            let mut values = Vec::new();
            for symbol in symbols {
                let write = writes(symbol)
                    .into_iter()
                    .filter(|write| {
                        let span = nodes.kind(*write).span();
                        arm.start <= span.start && span.end <= arm.end
                    })
                    .max_by_key(|write| nodes.kind(*write).span().end)?;
                values.push(self.definition_value(write)?);
            }
            let draw = |value: NodeId| matches!(nodes.kind(value), AstKind::CallExpression(call) if call.arguments.is_empty());
            let literal = |value: NodeId| matches!(nodes.kind(value), AstKind::NumericLiteral(_));
            match (draw(values[0]), draw(values[1])) {
                (true, false) if literal(values[1]) => constant_arms += 1,
                (false, true) if literal(values[0]) => constant_arms += 1,
                (false, true) if packed.is_none() => packed = Some(values[0]),
                (true, false) if packed.is_none() => packed = Some(values[1]),
                _ => return None,
            }
        }
        match (packed, constant_arms) {
            (Some(value), 1) => Some(Some(value)),
            (None, 2) => Some(None),
            _ => None,
        }
    }

    /// `x << (x === void 0 ? n : m)`: the shift of a defined `x` (`m`); an
    /// undefined `x` shifts to 0 whatever the amount.
    pub(super) fn guarded_shift(&self, binary: &oxc_ast::ast::BinaryExpression<'_>) -> Option<u32> {
        let Expression::ConditionalExpression(conditional) = binary.right.without_parentheses() else { return None };
        let shifted = self.identifier_name_symbol(&binary.left)?;
        let Expression::BinaryExpression(test) = conditional.test.without_parentheses() else { return None };
        let undefined = |expression: &Expression<'_>| match expression.without_parentheses() {
            Expression::UnaryExpression(unary) => unary.operator == oxc_ast::ast::UnaryOperator::Void,
            Expression::Identifier(identifier) => identifier.name == "undefined",
            Expression::NullLiteral(_) => true,
            _ => false,
        };
        let tested = (self.identifier_name_symbol(&test.left) == Some(shifted) && undefined(&test.right))
            || (self.identifier_name_symbol(&test.right) == Some(shifted) && undefined(&test.left));
        if !tested {
            return None;
        }
        let null_test = matches!(test.left.without_parentheses(), Expression::NullLiteral(_))
            || matches!(test.right.without_parentheses(), Expression::NullLiteral(_));
        let defined_arm = match test.operator {
            BinaryOperator::StrictEquality => &conditional.alternate,
            BinaryOperator::Equality if null_test => &conditional.alternate,
            BinaryOperator::StrictInequality => &conditional.consequent,
            BinaryOperator::Inequality if null_test => &conditional.consequent,
            _ => return None,
        };
        integer(defined_arm)
    }

    // =======================================================================
    // Packing loops
    // =======================================================================

    /// The accumulator read of a packing-loop byte: an in-loop temp
    /// `t = acc | X[j] << c` is that expression; a copy `b = acc ^ m()` is
    /// `acc`.
    pub(super) fn loop_value(&self, value: NodeId) -> NodeId {
        let nodes = self.parse.nodes;
        if !matches!(nodes.kind(value), AstKind::IdentifierReference(_)) {
            return value;
        }
        let Some((definition, Updates::Or(ors))) = self.identifier_value(value) else { return value };
        if !ors.is_empty() {
            return value;
        }
        let AstKind::BinaryExpression(binary) = nodes.kind(definition) else { return value };
        match binary.operator {
            BinaryOperator::BitwiseOR => {
                let right = self.expression_node(&binary.right);
                if right.is_some_and(|right| self.shift_term(right).is_some())
                    && (matches!(binary.left.without_parentheses(), Expression::Identifier(_)) || self.tuple_slot(&binary.left).is_some())
                {
                    return definition;
                }
                value
            }
            BinaryOperator::BitwiseXOR => {
                let call = |expression: &Expression<'_>| {
                    matches!(expression.without_parentheses(), Expression::CallExpression(call) if call.arguments.is_empty())
                };
                match (&binary.left.without_parentheses(), call(&binary.right)) {
                    (Expression::Identifier(_), true) => self.expression_node(&binary.left).unwrap_or(value),
                    _ => value,
                }
            }
            _ => value,
        }
    }

    /// `acc = t` where `t` is defined as `acc | X[j] << c`: the shift term.
    pub(super) fn temp_update(&self, right: &Expression<'_>, accumulator: SymbolId) -> Option<NodeId> {
        let Expression::Identifier(_) = right.without_parentheses() else { return None };
        let node = self.expression_node(right)?;
        let (definition, Updates::Or(ors)) = self.identifier_value(node)? else { return None };
        if !ors.is_empty() {
            return None;
        }
        let AstKind::BinaryExpression(binary) = self.parse.nodes.kind(definition) else { return None };
        if binary.operator != BinaryOperator::BitwiseOR
            || self.operand_symbol(&binary.left).map(|left| self.copy_root(left)) != Some(accumulator)
        {
            return None;
        }
        let shifted = self.expression_node(&binary.right)?;
        self.shift_term(shifted).map(|_| shifted)
    }

    /// The accumulator is written between the end of the packing loop and a
    /// read after it (the register holds another value by then).
    pub(super) fn reassigned_after(&self, accumulator: SymbolId, looped: Span, read: NodeId) -> bool {
        let nodes = self.parse.nodes;
        let read_at = nodes.kind(read).span().start;
        if looped.start <= read_at && read_at < looped.end {
            return false;
        }
        self.parse
            .scoping
            .get_resolved_references(accumulator)
            .filter(|reference| reference.is_write())
            .any(|reference| {
                let span = nodes.kind(reference.node_id()).span();
                // `acc = [buf, i, acc]` packs the read itself into a tuple:
                // the store holding the read is not a later value.
                let store = nodes.kind(nodes.parent_id(reference.node_id())).span();
                let holds_read = store.start <= read_at && read_at < store.end;
                span.start >= looped.end && span.end <= read_at && !holds_read
            })
    }

    /// Flush sizes the shared test does not read: `t === n` where `t` is
    /// defined as `Number(c) + 1` (or `c + 1`) in the loop; `Some(8)` for a
    /// loop the deobfuscator left without any flush (no equality test on
    /// the counter, no store in the loop): every item goes to the tail byte
    /// (checked against the list length by [`Self::unflushed_fits`]).
    pub(super) fn flush_size_more(&self, looped: Span, counter: SymbolId) -> Option<u32> {
        let nodes = self.parse.nodes;
        let stepped = |expression: &Expression<'_>| -> bool {
            let Expression::BinaryExpression(sum) = expression.without_parentheses() else { return false };
            if sum.operator != BinaryOperator::Addition || integer(&sum.right) != Some(1) {
                return false;
            }
            match sum.left.without_parentheses() {
                Expression::CallExpression(call) => {
                    matches!(call.callee.without_parentheses(), Expression::Identifier(callee) if callee.name == "Number")
                        && call.arguments.len() == 1
                        && call.arguments[0]
                            .as_expression()
                            .is_some_and(|argument| self.identifier_name_symbol(argument) == Some(counter))
                }
                other => self.identifier_name_symbol(other) == Some(counter),
            }
        };
        // Bindings defined as the stepped counter inside the loop.
        let mut steps: BTreeSet<SymbolId> = BTreeSet::new();
        for id in self.nodes_within(looped) {
            match nodes.kind(id) {
                AstKind::VariableDeclarator(declarator) => {
                    if declarator.init.as_ref().is_some_and(|init| stepped(init)) {
                        if let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &declarator.id {
                            if let Some(symbol) = identifier.symbol_id.get() {
                                steps.insert(symbol);
                            }
                        }
                    }
                }
                AstKind::AssignmentExpression(assignment) if assignment.operator.is_assign() && stepped(&assignment.right) => {
                    if let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                        if let Some(symbol) = self.identifier_symbol(identifier) {
                            steps.insert(symbol);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut found = None;
        let mut any_test = false;
        let mut stores = false;
        for id in self.nodes_within(looped) {
            match nodes.kind(id) {
                AstKind::BinaryExpression(test)
                    if matches!(test.operator, BinaryOperator::StrictEquality | BinaryOperator::Equality) =>
                {
                    let side = |expression: &Expression<'_>| {
                        self.identifier_name_symbol(expression)
                            .is_some_and(|symbol| symbol == counter || steps.contains(&symbol))
                    };
                    if side(&test.left) || side(&test.right) {
                        any_test = true;
                    }
                    let symbol_of = |expression: &Expression<'_>| self.identifier_name_symbol(expression);
                    if symbol_of(&test.left).is_some_and(|symbol| steps.contains(&symbol)) {
                        found = found.or(integer(&test.right));
                    } else if symbol_of(&test.right).is_some_and(|symbol| steps.contains(&symbol)) {
                        found = found.or(integer(&test.left));
                    }
                }
                AstKind::AssignmentExpression(assignment) => {
                    if matches!(
                        assignment.left,
                        oxc_ast::ast::AssignmentTarget::ComputedMemberExpression(_)
                            | oxc_ast::ast::AssignmentTarget::StaticMemberExpression(_)
                    ) {
                        stores = true;
                    }
                }
                _ => {}
            }
        }
        match found {
            Some(size) => Some(size),
            None if !any_test && !stores => Some(8),
            None => None,
        }
    }

    /// A byte write inside the packing loop is a flush byte unless it is
    /// the tail written on the loop's exit arm (`for (;;) { if (j >= 0) {
    /// …; continue; } if (c !== 0) write(acc); break; }`): a flush is under
    /// an equality test (`c === 8`), the tail under `c !== 0`.
    pub(super) fn flush_site(&self, site: Span, looped: Span) -> bool {
        if !(looped.start <= site.start && site.end <= looped.end) {
            return false;
        }
        let nodes = self.parse.nodes;
        let Some(mut child) = self.nodes_within(site).into_iter().next() else { return true };
        let mut tail = false;
        loop {
            let parent = nodes.parent_id(child);
            if parent == child || nodes.kind(parent).span() == looped || nodes.kind(parent).span().start < looped.start {
                break;
            }
            if let AstKind::IfStatement(statement) = nodes.kind(parent) {
                let in_test = statement.test.span().start <= nodes.kind(child).span().start
                    && nodes.kind(child).span().end <= statement.test.span().end;
                let in_consequent = statement.consequent.span().start <= nodes.kind(child).span().start
                    && nodes.kind(child).span().end <= statement.consequent.span().end;
                if !in_test && in_consequent {
                    if let Expression::BinaryExpression(test) = statement.test.without_parentheses() {
                        match test.operator {
                            BinaryOperator::StrictEquality | BinaryOperator::Equality => return true,
                            BinaryOperator::StrictInequality | BinaryOperator::Inequality
                                if integer(&test.right) == Some(0) || integer(&test.left) == Some(0) =>
                            {
                                tail = true
                            }
                            _ => {}
                        }
                    }
                }
            }
            child = parent;
        }
        !tail
    }

    /// A loop without a flush ([`Self::flush_size_more`]) packs every item
    /// into the tail byte only when the list is shorter than a byte.
    pub(super) fn unflushed_fits(&self, looped: Span, counter: SymbolId, items: usize) -> Result<(), String> {
        let nodes = self.parse.nodes;
        let tested = self.nodes_within(looped).into_iter().any(|id| match nodes.kind(id) {
            AstKind::BinaryExpression(test) => matches!(test.operator, BinaryOperator::StrictEquality | BinaryOperator::Equality),
            _ => false,
        });
        let _ = counter;
        if !tested && items >= 8 {
            return Err(format!("no flush test for {items} items"));
        }
        Ok(())
    }

    /// The probed strings of a boolean list the shared reader does not
    /// find: filled by index in a loop over a list, or a callback parameter
    /// fed with a list filled by a probe queue.
    pub(super) fn more_list_items(&self, list: SymbolId) -> Option<Vec<String>> {
        self.indexed_fill_items(list).or_else(|| self.parameter_list_items(list, LIST_HOPS))
    }

    /// `list = [false, …]; for (…; i++) { k = i; list[k] = test(…L[i]…) }`
    /// read at `read` with the register `list` reused for other values
    /// elsewhere: only the element stores between the reaching literal (of
    /// one boolean per item) and the read, each indexed by the ascending loop
    /// index (or a register copied from it in the loop).
    pub(super) fn windowed_fill_items(&self, list: SymbolId, object: NodeId, read: NodeId) -> Option<Vec<String>> {
        let nodes = self.parse.nodes;
        let (value, _) = self.identifier_value(object)?;
        let AstKind::ArrayExpression(array) = nodes.kind(value) else { return None };
        let booleans = array.elements.iter().all(|element| {
            matches!(element.as_expression().map(|e| e.without_parentheses()), Some(Expression::BooleanLiteral(_)))
        });
        if !booleans || array.elements.is_empty() {
            return None;
        }
        let function = self.enclosing_function(read);
        if self.enclosing_function(value) != function {
            return None;
        }
        let (from, to) = (nodes.kind(value).span().end, nodes.kind(read).span().start);
        let mut found: Option<Vec<String>> = None;
        for reference in self.parse.scoping.get_resolved_references(list) {
            let node = reference.node_id();
            let span = nodes.kind(node).span();
            if span.start < from || span.end > to {
                continue;
            }
            if reference.is_write() {
                return None;
            }
            let parent = nodes.parent_id(node);
            let AstKind::ComputedMemberExpression(element) = nodes.kind(parent) else { continue };
            if element.object.span() != span {
                continue;
            }
            let store = nodes.parent_id(parent);
            let AstKind::AssignmentExpression(assignment) = nodes.kind(store) else { continue };
            if assignment.left.span() != element.span {
                continue;
            }
            if self.enclosing_function(store) != function {
                return None;
            }
            let looped = self.enclosing_loop(store)?;
            let loop_span = nodes.kind(looped).span();
            let mut index = self.identifier_name_symbol(&element.expression)?;
            if self.step_of(index, loop_span) != Some(true) {
                // `k = i` in the loop: the loop index.
                let read = self.expression_node(&element.expression)?;
                let (copied, _) = self.identifier_value(read)?;
                if !matches!(nodes.kind(copied), AstKind::IdentifierReference(_)) {
                    return None;
                }
                let source = self.reference_symbol(copied)?;
                let copied_at = nodes.kind(copied).span();
                if copied_at.start < loop_span.start || copied_at.end > loop_span.end || self.step_of(source, loop_span) != Some(true) {
                    return None;
                }
                index = source;
            }
            let mut lists: Vec<Vec<String>> = Vec::new();
            for id in self.nodes_within(loop_span) {
                let AstKind::ComputedMemberExpression(probed) = nodes.kind(id) else { continue };
                if self.identifier_name_symbol(&probed.expression) != Some(index) || probed.span == element.span {
                    continue;
                }
                let Some(probed_object) = self.expression_node(&probed.object) else { continue };
                if let Some(items) = self.resolve_value(probed_object, LIST_HOPS).and_then(|value| self.string_list(value)) {
                    if !lists.contains(&items) {
                        lists.push(items);
                    }
                }
            }
            let [items] = lists.as_slice() else { return None };
            match &found {
                None => found = Some(items.clone()),
                Some(previous) if previous == items => {}
                Some(_) => return None,
            }
        }
        // One boolean per probed item.
        found.filter(|items| items.len() == array.elements.len())
    }

    /// `list[i] = test(…L[i]…)` in an ascending loop over `i` (every store
    /// to `list` is in that loop, the loop reads `L[i]` of one probed list
    /// `L`): the items of `L`.
    fn indexed_fill_items(&self, list: SymbolId) -> Option<Vec<String>> {
        let nodes = self.parse.nodes;
        let mut found: Option<Vec<String>> = None;
        let mut any = false;
        for reference in self.parse.scoping.get_resolved_references(list) {
            let node = reference.node_id();
            let parent = nodes.parent_id(node);
            let AstKind::ComputedMemberExpression(element) = nodes.kind(parent) else { continue };
            if element.object.span() != nodes.kind(node).span() {
                continue;
            }
            let store = nodes.parent_id(parent);
            let AstKind::AssignmentExpression(assignment) = nodes.kind(store) else { continue };
            if assignment.left.span() != element.span {
                continue;
            }
            let index = self.identifier_name_symbol(&element.expression)?;
            let looped = self.enclosing_loop(store)?;
            let loop_span = nodes.kind(looped).span();
            if self.step_of(index, loop_span) != Some(true) {
                return None;
            }
            // The probed list read at the same index in the loop.
            let mut lists: Vec<Vec<String>> = Vec::new();
            for id in self.nodes_within(loop_span) {
                let AstKind::ComputedMemberExpression(read) = nodes.kind(id) else { continue };
                if self.identifier_name_symbol(&read.expression) != Some(index) || read.span == element.span {
                    continue;
                }
                let Some(object) = self.expression_node(&read.object) else { continue };
                if let Some(items) = self.resolve_value(object, LIST_HOPS).and_then(|value| self.string_list(value)) {
                    if !lists.contains(&items) {
                        lists.push(items);
                    }
                }
            }
            let [items] = lists.as_slice() else { return None };
            match &found {
                None => found = Some(items.clone()),
                Some(previous) if previous == items => {}
                Some(_) => return None,
            }
            any = true;
        }
        if !any {
            return None;
        }
        found
    }

    /// The list a callback parameter receives: `list` is parameter `k` of a
    /// function passed as argument `a` of a call to `H`; `H` invokes its
    /// parameter `a` with a list filled by [`Self::pushed_items`].
    fn parameter_list_items(&self, list: SymbolId, hops: usize) -> Option<Vec<String>> {
        let (function, position) = self.parameter_of(list)?;
        let mut found: Option<Vec<String>> = None;
        let mut any = false;
        let calls = self.invocations(function, hops);
        for call in calls {
            let AstKind::CallExpression(call) = self.parse.nodes.kind(call) else { return None };
            let argument = self.expression_node(call.arguments.get(position)?.as_expression()?)?;
            let items = self.pushed_items(argument)?;
            match &found {
                None => found = Some(items),
                Some(previous) if *previous == items => {}
                Some(_) => return None,
            }
            any = true;
        }
        if !any {
            return None;
        }
        found
    }

    /// A local list `L` (read by `node`) filled only by `alias.push(v)` in
    /// one callback `C`, `C` passed with a probed list to an in-order queue
    /// `Q(C, items)` that calls `C` with each item (its first parameter,
    /// read by `C`): the items in order.
    fn pushed_items(&self, node: NodeId) -> Option<Vec<String>> {
        let nodes = self.parse.nodes;
        let list = self.reference_symbol(node)?;
        // The list and its plain aliases (`let a = list`).
        let mut symbols = vec![list];
        for reference in self.parse.scoping.get_resolved_references(list) {
            let parent = nodes.parent_id(reference.node_id());
            if let AstKind::VariableDeclarator(declarator) = nodes.kind(parent) {
                if let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &declarator.id {
                    if let Some(alias) = identifier.symbol_id.get() {
                        if self.parse.scoping.get_resolved_references(alias).all(|reference| !reference.is_write()) {
                            symbols.push(alias);
                        }
                    }
                }
            }
        }
        // `list = []` (declared or assigned) once, no other writes.
        let writes: Vec<NodeId> = self
            .parse
            .scoping
            .get_resolved_references(list)
            .filter(|reference| reference.is_write())
            .map(|reference| nodes.parent_id(reference.node_id()))
            .collect();
        let empty = |write: &NodeId| match nodes.kind(*write) {
            AstKind::AssignmentExpression(assignment) => {
                assignment.operator.is_assign()
                    && matches!(assignment.right.without_parentheses(), Expression::ArrayExpression(array) if array.elements.is_empty())
            }
            _ => false,
        };
        if writes.len() > 1 || !writes.iter().all(empty) {
            return None;
        }
        let mut callback: Option<NodeId> = None;
        let mut pushes = 0;
        for symbol in &symbols {
            for reference in self.parse.scoping.get_resolved_references(*symbol) {
                let parent = nodes.parent_id(reference.node_id());
                let AstKind::StaticMemberExpression(member) = nodes.kind(parent) else { continue };
                if member.object.span() != nodes.kind(reference.node_id()).span() {
                    continue;
                }
                match member.property.name.as_str() {
                    "push" => {}
                    "length" | "slice" | "map" | "forEach" | "join" => continue,
                    _ => return None,
                }
                for call in self.push_calls(parent)? {
                    let function = self.enclosing_function(call)?;
                    match callback {
                        None => callback = Some(function),
                        Some(previous) if previous == function => {}
                        Some(_) => return None,
                    }
                    pushes += 1;
                }
            }
        }
        let callback = callback?;
        if pushes != 1 || self.enclosing_loop(callback).is_some() {
            return None;
        }
        // The callback reads its item parameter.
        let item = self.parameter_symbol(callback, 0)?;
        if self.parse.scoping.get_resolved_references(item).next().is_none() {
            return None;
        }
        // `Q(C, items)`: the call the callback is a direct argument of.
        let call = nodes.parent_id(callback);
        let AstKind::CallExpression(queue) = nodes.kind(call) else { return None };
        let callback_span = nodes.kind(callback).span();
        if queue.arguments.first().map(|argument| argument.span()) != Some(callback_span) {
            return None;
        }
        let mut lists = Vec::new();
        for argument in queue.arguments.iter().skip(1) {
            let Some(argument) = argument.as_expression().and_then(|argument| self.expression_node(argument)) else { continue };
            if let Some(items) = self.resolve_value(argument, LIST_HOPS).and_then(|value| self.string_list(value)) {
                lists.push(items);
            }
        }
        let [items] = lists.as_slice() else { return None };
        let helper = self.expression_node(&queue.callee).and_then(|callee| self.resolve_value(callee, LIST_HOPS))?;
        self.in_order_queue(helper).then(|| items.clone())
    }

    /// The one-argument push calls of a `L.push` member (`member`):
    /// `L.push(v)`, or the method read once into a binding and called on
    /// the list: `let f = L.push; f.call(L, v)`, `let x = [L, L.push];
    /// x[1].call(x[0], v)`. `None` when the method escapes otherwise.
    fn push_calls(&self, member: NodeId) -> Option<Vec<NodeId>> {
        let nodes = self.parse.nodes;
        let member_span = nodes.kind(member).span();
        let parent = nodes.parent_id(member);
        match nodes.kind(parent) {
            AstKind::CallExpression(push) if push.callee.span() == member_span => {
                (push.arguments.len() == 1).then(|| vec![parent])
            }
            AstKind::VariableDeclarator(declarator) => {
                let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &declarator.id else { return None };
                let method = identifier.symbol_id.get()?;
                let mut calls = Vec::new();
                for reference in self.parse.scoping.get_resolved_references(method) {
                    if reference.is_write() {
                        return None;
                    }
                    let read = nodes.parent_id(reference.node_id());
                    let AstKind::StaticMemberExpression(call_member) = nodes.kind(read) else { return None };
                    let call = nodes.parent_id(read);
                    match nodes.kind(call) {
                        AstKind::CallExpression(invoke)
                            if call_member.property.name == "call" && invoke.callee.span() == call_member.span && invoke.arguments.len() == 2 =>
                        {
                            calls.push(call)
                        }
                        _ => return None,
                    }
                }
                Some(calls)
            }
            AstKind::ArrayExpression(pair) => {
                let [Some(first), Some(second)] = [pair.elements.first(), pair.elements.get(1)] else { return None };
                if pair.elements.len() != 2 || second.span() != member_span {
                    return None;
                }
                let _ = first;
                let declarator = nodes.parent_id(parent);
                let AstKind::VariableDeclarator(declarator) = nodes.kind(declarator) else { return None };
                let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &declarator.id else { return None };
                let pair_symbol = identifier.symbol_id.get()?;
                let mut calls = Vec::new();
                for reference in self.parse.scoping.get_resolved_references(pair_symbol) {
                    if reference.is_write() {
                        return None;
                    }
                    let element = nodes.parent_id(reference.node_id());
                    let AstKind::ComputedMemberExpression(element_member) = nodes.kind(element) else { return None };
                    match element_member.expression.without_parentheses() {
                        Expression::NumericLiteral(index) if index.value == 0.0 => continue,
                        Expression::NumericLiteral(index) if index.value == 1.0 => {}
                        _ => return None,
                    }
                    let read = nodes.parent_id(element);
                    let AstKind::StaticMemberExpression(call_member) = nodes.kind(read) else { return None };
                    let call = nodes.parent_id(read);
                    match nodes.kind(call) {
                        AstKind::CallExpression(invoke)
                            if call_member.property.name == "call" && invoke.callee.span() == call_member.span && invoke.arguments.len() == 2 =>
                        {
                            calls.push(call)
                        }
                        _ => return None,
                    }
                }
                Some(calls)
            }
            _ => None,
        }
    }

    /// A queue function that takes items from the front of its list
    /// (`.shift()`) and never reorders it.
    fn in_order_queue(&self, function: NodeId) -> bool {
        let nodes = self.parse.nodes;
        if !matches!(nodes.kind(function), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) {
            return false;
        }
        let mut shift = false;
        for id in self.nodes_within(nodes.kind(function).span()) {
            if let AstKind::StaticMemberExpression(member) = nodes.kind(id) {
                match member.property.name.as_str() {
                    "shift" => shift = true,
                    "pop" | "reverse" | "sort" | "unshift" => return false,
                    _ => {}
                }
            }
        }
        shift
    }

    /// The function and position of a formal parameter binding.
    fn parameter_of(&self, symbol: SymbolId) -> Option<(NodeId, usize)> {
        let nodes = self.parse.nodes;
        let mut node = self.parse.scoping.symbol_declaration(symbol);
        for _ in 0..4 {
            if let AstKind::FormalParameter(_) = nodes.kind(node) {
                let parameters = nodes.parent_id(node);
                let AstKind::FormalParameters(list) = nodes.kind(parameters) else { return None };
                let span = nodes.kind(node).span();
                let position = list.items.iter().position(|item| item.span == span)?;
                return Some((nodes.parent_id(parameters), position));
            }
            let parent = nodes.parent_id(node);
            if parent == node {
                return None;
            }
            node = parent;
        }
        None
    }

    /// Parameter `position` of a function literal.
    fn parameter_symbol(&self, function: NodeId, position: usize) -> Option<SymbolId> {
        let params = match self.parse.nodes.kind(function) {
            AstKind::Function(function) => &function.params,
            AstKind::ArrowFunctionExpression(arrow) => &arrow.params,
            _ => return None,
        };
        let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &params.items.get(position)?.pattern else {
            return None;
        };
        identifier.symbol_id.get()
    }

    /// A binding and its read-only aliases (`let a = b`).
    fn with_aliases(&self, symbol: SymbolId) -> Vec<SymbolId> {
        let nodes = self.parse.nodes;
        let mut out = vec![symbol];
        let mut index = 0;
        while index < out.len() && out.len() < 8 {
            let current = out[index];
            index += 1;
            for reference in self.parse.scoping.get_resolved_references(current) {
                if !reference.is_read() {
                    continue;
                }
                let span = nodes.kind(reference.node_id()).span();
                let parent = nodes.parent_id(reference.node_id());
                let alias = match nodes.kind(parent) {
                    // `let a = b`, never reassigned.
                    AstKind::VariableDeclarator(declarator) => {
                        let Some(init) = &declarator.init else { continue };
                        if init.without_parentheses().span() != span {
                            continue;
                        }
                        let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &declarator.id else { continue };
                        let Some(alias) = identifier.symbol_id.get() else { continue };
                        if self.parse.scoping.get_resolved_references(alias).any(|reference| reference.is_write()) {
                            continue;
                        }
                        alias
                    }
                    // `a = b`, the only write of a binding declared without a value.
                    AstKind::AssignmentExpression(assignment) if assignment.operator.is_assign() => {
                        if assignment.right.without_parentheses().span() != span {
                            continue;
                        }
                        let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else { continue };
                        let Some(alias) = self.identifier_symbol(identifier) else { continue };
                        let declaration = self.parse.scoping.symbol_declaration(alias);
                        let declared_empty =
                            matches!(nodes.kind(declaration), AstKind::VariableDeclarator(declarator) if declarator.init.is_none());
                        let writes = self.parse.scoping.get_resolved_references(alias).filter(|reference| reference.is_write()).count();
                        if !declared_empty || writes != 1 {
                            continue;
                        }
                        alias
                    }
                    _ => continue,
                };
                if !out.contains(&alias) {
                    out.push(alias);
                }
            }
        }
        out
    }

    /// Calls whose callee is `symbol` or one of its aliases.
    fn calls_through(&self, symbol: SymbolId) -> Vec<NodeId> {
        let nodes = self.parse.nodes;
        let mut out = Vec::new();
        for alias in self.with_aliases(symbol) {
            for reference in self.parse.scoping.get_resolved_references(alias) {
                if !reference.is_read() {
                    continue;
                }
                let parent = nodes.parent_id(reference.node_id());
                if let AstKind::CallExpression(call) = nodes.kind(parent) {
                    if call.callee.without_parentheses().span() == nodes.kind(reference.node_id()).span() {
                        out.push(parent);
                    }
                }
                // `r = f` into a register written more than once: the
                // calls of `r` this store reaches (`r = f; … r(…)`).
                if let AstKind::AssignmentExpression(assignment) = nodes.kind(parent) {
                    if !assignment.operator.is_assign() || assignment.right.without_parentheses().span() != nodes.kind(reference.node_id()).span() {
                        continue;
                    }
                    let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else { continue };
                    let Some(register) = self.identifier_symbol(identifier) else { continue };
                    for read in self.parse.scoping.get_resolved_references(register) {
                        if !read.is_read() {
                            continue;
                        }
                        let call = nodes.parent_id(read.node_id());
                        let AstKind::CallExpression(invoke) = nodes.kind(call) else { continue };
                        if invoke.callee.without_parentheses().span() != nodes.kind(read.node_id()).span() {
                            continue;
                        }
                        match self.identifier_value(read.node_id()) {
                            Some((value, Updates::Or(ors))) if ors.is_empty() && value == reference.node_id() => {
                                if !out.contains(&call) {
                                    out.push(call);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        out
    }

    /// The binding a function literal is declared or initialised as.
    fn function_symbol(&self, function: NodeId) -> Option<SymbolId> {
        let nodes = self.parse.nodes;
        if let AstKind::Function(literal) = nodes.kind(function) {
            if let Some(symbol) = literal.id.as_ref().and_then(|id| id.symbol_id.get()) {
                return Some(symbol);
            }
        }
        match nodes.kind(nodes.parent_id(function)) {
            AstKind::VariableDeclarator(declarator) => match &declarator.id {
                oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
                _ => None,
            },
            _ => None,
        }
    }

    /// Where the function literal `function` is invoked: by its name (or an
    /// alias), or, when it is argument `a` of a call to a function `H`, where
    /// `H` invokes its parameter `a` (or an alias of it).
    fn invocations(&self, function: NodeId, hops: usize) -> Vec<NodeId> {
        let nodes = self.parse.nodes;
        if hops == 0 {
            return Vec::new();
        }
        if let Some(symbol) = self.function_symbol(function) {
            return self.calls_through(symbol);
        }
        let call = nodes.parent_id(function);
        let AstKind::CallExpression(outer) = nodes.kind(call) else { return Vec::new() };
        let span = nodes.kind(function).span();
        let Some(position) = outer.arguments.iter().position(|argument| argument.span() == span) else { return Vec::new() };
        let Some(helper) = self.expression_node(&outer.callee).and_then(|callee| self.resolve_value(callee, hops - 1)) else {
            return Vec::new();
        };
        match self.parameter_symbol(helper, position) {
            Some(parameter) => self.calls_through(parameter),
            None => Vec::new(),
        }
    }

    /// The source expression of a value through bindings (a single
    /// definition, else the nearest), parameters (the argument of the one
    /// call site), object properties and array elements of literals; a
    /// binding of a function declaration is the function.
    pub(super) fn resolve_value(&self, node: NodeId, hops: usize) -> Option<NodeId> {
        if hops == 0 {
            return None;
        }
        let nodes = self.parse.nodes;
        match nodes.kind(node) {
            AstKind::ParenthesizedExpression(inner) => self.resolve_value(self.expression_node(&inner.expression)?, hops - 1),
            AstKind::IdentifierReference(_) => {
                let symbol = self.reference_symbol(node)?;
                if let Some((function, position)) = self.parameter_of(symbol) {
                    // The argument of the one call site, or the one value
                    // every call site passes (a helper called by several
                    // copies of a collector with the same table).
                    let calls = self.invocations(function, hops - 1);
                    let mut resolved: Option<NodeId> = None;
                    for call in &calls {
                        let AstKind::CallExpression(call) = nodes.kind(*call) else { return None };
                        let argument = self.expression_node(call.arguments.get(position)?.as_expression()?)?;
                        let value = self.resolve_value(argument, hops - 1)?;
                        match resolved {
                            None => resolved = Some(value),
                            Some(previous) if previous == value => {}
                            Some(_) => return None,
                        }
                    }
                    return resolved;
                }
                let declaration = self.parse.scoping.symbol_declaration(symbol);
                match nodes.kind(declaration) {
                    AstKind::Function(_) => return Some(declaration),
                    AstKind::VariableDeclarator(declarator) => {
                        let writes: Vec<NodeId> = self
                            .parse
                            .scoping
                            .get_resolved_references(symbol)
                            .filter(|reference| reference.is_write())
                            .map(|reference| reference.node_id())
                            .collect();
                        match (&declarator.init, writes.as_slice()) {
                            (Some(init), []) => return self.resolve_value(self.expression_node(init)?, hops - 1),
                            (None, [write]) => {
                                if let AstKind::AssignmentExpression(assignment) = nodes.kind(nodes.parent_id(*write)) {
                                    if assignment.operator.is_assign() {
                                        return self.resolve_value(self.expression_node(&assignment.right)?, hops - 1);
                                    }
                                }
                                return None;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                match self.identifier_value(node)? {
                    (value, Updates::Or(ors)) if ors.is_empty() => self.resolve_value(value, hops - 1),
                    _ => None,
                }
            }
            AstKind::StaticMemberExpression(member) => {
                let object = self.resolve_value(self.expression_node(&member.object)?, hops - 1)?;
                let AstKind::ObjectExpression(literal) = nodes.kind(object) else { return None };
                let value = literal.properties.iter().find_map(|property| match property {
                    oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property)
                        if property.key.static_name().as_deref() == Some(member.property.name.as_str()) =>
                    {
                        Some(&property.value)
                    }
                    _ => None,
                });
                match value {
                    Some(value) => self.resolve_value(self.expression_node(value)?, hops - 1),
                    // `o = {…}; o.k = v`: the store joins the record.
                    None => {
                        let record = self.record_at(object)?;
                        self.resolve_value(*record.values.get(member.property.name.as_str())?, hops - 1)
                    }
                }
            }
            AstKind::ComputedMemberExpression(member) => {
                let index = integer(&member.expression)?;
                let object = self.resolve_value(self.expression_node(&member.object)?, hops - 1)?;
                let AstKind::ArrayExpression(array) = nodes.kind(object) else { return None };
                let element = array.elements.get(index as usize)?.as_expression()?;
                self.resolve_value(self.expression_node(element)?, hops - 1)
            }
            _ => Some(node),
        }
    }

    // =======================================================================
    // Record labels
    // =======================================================================

    /// The element or member read a boolean binding copies (`v = rec.k[i]`,
    /// `v = list[0]`), through plain copies.
    pub(super) fn copy_source(&self, base: NodeId) -> Option<NodeId> {
        let nodes = self.parse.nodes;
        let mut node = base;
        for _ in 0..TERM_HOPS {
            match nodes.kind(node) {
                AstKind::IdentifierReference(_) => match self.identifier_value(node)? {
                    (value, Updates::Or(ors)) if ors.is_empty() => node = value,
                    _ => return None,
                },
                AstKind::ComputedMemberExpression(_) | AstKind::StaticMemberExpression(_) if node != base => return Some(node),
                _ => return None,
            }
        }
        None
    }

    /// A stored record value that is never a list: a primitive literal, an
    /// operator result, `Math.*` of something, a conditional or logical
    /// whose arms all are, or a copy of the same label from another record
    /// (a derived record, not the construction).
    fn never_list(&self, value: NodeId, label: &str, hops: usize) -> bool {
        if hops == 0 {
            return false;
        }
        let nodes = self.parse.nodes;
        let primitive = |expression: &Expression<'_>| -> Option<NodeId> { self.expression_node(expression) };
        match nodes.kind(value) {
            AstKind::NumericLiteral(_)
            | AstKind::StringLiteral(_)
            | AstKind::BooleanLiteral(_)
            | AstKind::NullLiteral(_)
            | AstKind::TemplateLiteral(_)
            | AstKind::UnaryExpression(_)
            | AstKind::BinaryExpression(_)
            | AstKind::UpdateExpression(_) => true,
            AstKind::ParenthesizedExpression(inner) => primitive(&inner.expression).is_some_and(|inner| self.never_list(inner, label, hops - 1)),
            AstKind::ConditionalExpression(conditional) => {
                [&conditional.consequent, &conditional.alternate]
                    .into_iter()
                    .all(|arm| primitive(arm).is_some_and(|arm| self.never_list(arm, label, hops - 1)))
            }
            AstKind::LogicalExpression(logical) => [&logical.left, &logical.right]
                .into_iter()
                .all(|side| primitive(side).is_some_and(|side| self.never_list(side, label, hops - 1))),
            AstKind::CallExpression(call) => match call.callee.without_parentheses() {
                Expression::StaticMemberExpression(member) => {
                    matches!(member.object.without_parentheses(), Expression::Identifier(object) if object.name == "Math")
                }
                _ => false,
            },
            AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_) => {
                self.read_key(value).as_deref() == Some(label)
            }
            AstKind::IdentifierReference(_) => match self.identifier_value(value) {
                Some((next, Updates::Or(ors))) if ors.is_empty() => self.never_list(next, label, hops - 1),
                _ => false,
            },
            _ => false,
        }
    }

    /// The record construction can be the one the scope's booleans read:
    /// no label the slot reads as a list (`rec.k[i]`) holds a value that is
    /// never a list there.
    pub(super) fn record_fits(&self, record: &RecordConstruction, scope: &Scope) -> bool {
        scope.indexed.iter().all(|label| match record.values.get(label) {
            Some(value) => !self.never_list(*value, label, TERM_HOPS),
            None => true,
        })
    }
}

/// The slot-wide naming scope: the record labels every term of every write
/// site of the slot's packed fields reads (directly or through a copied
/// element), and the labels read as lists.
pub(super) fn slot_scope<'a>(ctx: &FinderCtx, view: &SlotView, terms: impl Iterator<Item = &'a PackedTerm>) -> Scope {
    let mut labels = BTreeSet::new();
    let mut indexed = BTreeSet::new();
    for term in terms {
        let Some(base) = term.base else { continue };
        for node in [Some(base), ctx.copy_source(base)].into_iter().flatten() {
            if let Some((label, index)) = ctx.record_label(node) {
                if index.is_some() {
                    indexed.insert(label.clone());
                }
                labels.insert(label);
            }
        }
    }
    Scope {
        spans: view.spans(),
        labels,
        indexed,
    }
}

/// A second naming attempt for a boolean no card flag was read for: the
/// element or member it copies, named in the slot-wide scope.
pub(super) fn retry_names(
    ctx: &FinderCtx,
    base: NodeId,
    slot: &Scope,
    probes: &[(String, Vec<String>)],
) -> Vec<String> {
    let source = ctx.copy_source(base).unwrap_or(base);
    nearest_flags(ctx, source, slot, probes)
}

/// Loop items written as numbers named by a card list of code points
/// (`97` -> `U+0061`); other items unchanged.
pub(super) fn card_item(item: &str, flags: &[String]) -> String {
    if flags.iter().any(|flag| flag == item) {
        return item.to_string();
    }
    match item.parse::<u32>() {
        Ok(value) => {
            let point = format!("U+{value:04X}");
            if flags.contains(&point) {
                point
            } else {
                item.to_string()
            }
        }
        Err(_) => item.to_string(),
    }
}
