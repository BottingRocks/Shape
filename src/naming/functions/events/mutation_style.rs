//! Source proof for a `MutationRecord` added-node iterator and the inspection
//! callback it directly owns.
//!
//! The whole owned parent body is proved from scoped operations, never from a
//! generated spelling, a thread id, a source position, a vendor or a digest
//! dictionary:
//!
//! ```text
//! function iterator(records) {
//!   let record, index;
//!   let cursor = 0;
//!   while (cursor < records.length) {
//!     record = records[cursor];
//!     function inspect() {
//!       let node = record.addedNodes[index];
//!       if (node.nodeType !== 1) return ...;
//!       if (node.tagName === "DIV") {
//!         let names = Array.from(node.style);
//!         names.sort();
//!         if (<consumer>(names.join(";")) === <number>) {
//!           divFlag = true;
//!           <sink>.next({ measurementDiv: true, animationStyle: styleFlag });
//!         }
//!       } else if (node.tagName === "STYLE") {
//!         let lines = node.textContent.split("\n")...;
//!         if (<consumer>(lines.join("\n")) === <number>) {
//!           styleFlag = true;
//!           <sink>.next({ measurementDiv: divFlag, animationStyle: true });
//!         }
//!       }
//!     }
//!     for (;; index = cursor = next) {
//!       if (!(cursor < record.addedNodes.length)) break;
//!       inspect();
//!     }
//!     ++cursor;
//!   }
//! }
//! ```
//!
//! The iterator is named only when its own body reads a record from the
//! argument list through the loop cursor and bounds a nested loop on that
//! record's `addedNodes.length`, and the nested body invokes exactly one function
//! directly owned by the iterator. The inspection callback is named only when
//! its owner independently proves that iterator, its own body reads
//! `record.addedNodes[index]` from the iterator's exact record/cursor
//! identities, the same `nodeType === 1` branch contains the paired `tagName`
//! `DIV`/`STYLE` chain, and the two probes carry an inline-style signature and a
//! text-content signature respectively. Both probes flip distinct captured
//! flags, and the two probe branches hand object payloads reading those flags
//! to the same scoped call path.
//!
//! A nested decoy whose body alone mentions `addedNodes` never contributes to
//! the iterator proof, and a callback that merely sits under an unrelated
//! function never matches. `Array.from` must be the unshadowed global; the
//! style/text receiver must be the proved node identity, and the STYLE join and
//! its digest argument must both consume the one line array the node's
//! `textContent.split("\n")` dataflow produces through array-preserving
//! transforms. The `map`/`filter` callback bodies (per-line `trim` and the like)
//! are opaque and are not evidence. The digest consumer and the payload sink are
//! opaque protocol inputs: only their argument slots and the exact scoped
//! identities they consume are evidence. A `while` loop, a classic `for` loop
//! and a `for (;;)` loop with a negated break guard are the same
//! bounded-iteration operation; branch order and local spellings are opaque.
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

// ===========================================================================
// Scoped expression helpers
// ===========================================================================

fn symbol_of(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

/// A plain identifier reference to the unshadowed global named by the caller.
/// A local with the same spelling is a different binding and is refused.
fn unbound_global(model: &Model<'_, '_>, expression: &Expression<'_>, name: &str) -> bool {
    let Expression::Identifier(identifier) = expression.without_parentheses() else {
        return false;
    };
    identifier.name == name
        && crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
}

fn static_member<'a>(expression: &'a Expression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn computed_pair<'a>(
    expression: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional).then_some((&member.object, &member.expression))
}

fn plain_call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return None;
    };
    (!call.optional).then_some(call)
}

/// Exact argument list. A spread or other non-expression argument changes the
/// list length and refuses the shape.
fn exact_arguments<'a>(
    call: &'a CallExpression<'a>,
    count: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let values = call_arguments(call);
    (call.arguments.len() == count && values.len() == count).then_some(values)
}

fn member_on(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    target: SymbolId,
    name: &str,
) -> bool {
    static_member(expression, name).is_some_and(|object| symbol_of(model, object) == Some(target))
}

/// A numeric literal, including the sign a source writes in front of one.
///
/// The digest consumer and its compared value are opaque protocol data: the
/// guard only requires that the comparison tests a numeric literal, never a
/// particular magnitude, sign or vendor constant. Oxc parses `-1422589240` and
/// `+7` as unary expressions over a numeric literal rather than as a signed
/// literal node, so a literal-only check silently refused the witness form
/// while accepting the same shape written with an unsigned constant.
fn numeric_literal(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::NumericLiteral(_) => true,
        Expression::UnaryExpression(unary) if matches!(unary.operator.as_str(), "-" | "+") => {
            matches!(
                unary.argument.without_parentheses(),
                Expression::NumericLiteral(_)
            )
        }
        _ => false,
    }
}

fn string_with(expression: &Expression<'_>, needle: &str) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::StringLiteral(string) if string.value.as_str().contains(needle)
    )
}

fn boolean_true(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::BooleanLiteral(boolean) if boolean.value
    )
}

/// The signature separator/consumer split is a source operation, not a literal
/// value: the caller supplies the separator it is proving.
fn matches_join(
    model: &Model<'_, '_>,
    call: &CallExpression<'_>,
    receiver: Option<SymbolId>,
    separator: &str,
) -> bool {
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return false;
    };
    if member.optional || member.property.name != "join" {
        return false;
    }
    if let Some(receiver) = receiver {
        if symbol_of(model, &member.object) != Some(receiver) {
            return false;
        }
    }
    exact_arguments(call, 1).is_some_and(|arguments| string_with(&arguments[0], separator))
}

/// A digest consumer is opaque. Only the exact argument dataflow is evidence:
/// `consumer(signature_join) === <number>` (or the mirrored comparator).
fn digest_compare(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    receiver: Option<SymbolId>,
    separator: &str,
) -> bool {
    let is_digest = |expression: &Expression<'_>| {
        let Some(call) = plain_call(expression) else {
            return false;
        };
        let Some(arguments) = exact_arguments(call, 1) else {
            return false;
        };
        let Some(join) = plain_call(&arguments[0]) else {
            return false;
        };
        matches_join(model, join, receiver, separator)
    };
    scan.binaries.iter().copied().any(|binary| {
        if !matches!(binary.operator.as_str(), "==" | "===" | "!=" | "!==") {
            return false;
        }
        (numeric_literal(&binary.left) && is_digest(&binary.right))
            || (numeric_literal(&binary.right) && is_digest(&binary.left))
    })
}

// ===========================================================================
// Bounded owned-iteration scan (nested functions are separate entities)
// ===========================================================================

#[derive(Default)]
struct Scan<'a> {
    defs: BTreeMap<SymbolId, &'a Expression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    calls: Vec<&'a CallExpression<'a>>,
    members: Vec<&'a StaticMemberExpression<'a>>,
    binaries: Vec<&'a BinaryExpression<'a>>,
    ifs: Vec<&'a IfStatement<'a>>,
    loops: Vec<&'a Statement<'a>>,
}

fn scan_statements<'a>(statements: &'a [Statement<'a>], scan: &mut Scan<'a>) {
    for statement in statements {
        scan_statement(statement, scan);
    }
}

fn scan_statement<'a>(statement: &'a Statement<'a>, scan: &mut Scan<'a>) {
    match statement {
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if let Some(init) = &declarator.init {
                    scan_expression(init, scan);
                    if let BindingPattern::BindingIdentifier(identifier) = &declarator.id {
                        if let Some(symbol) = identifier.symbol_id.get() {
                            scan.defs.entry(symbol).or_insert(init);
                        }
                    }
                }
            }
        }
        Statement::ExpressionStatement(expression) => scan_expression(&expression.expression, scan),
        Statement::ReturnStatement(return_) => {
            if let Some(argument) = &return_.argument {
                scan_expression(argument, scan);
            }
        }
        Statement::IfStatement(branch) => {
            scan.ifs.push(branch);
            scan_expression(&branch.test, scan);
            scan_statement(&branch.consequent, scan);
            if let Some(alternate) = &branch.alternate {
                scan_statement(alternate, scan);
            }
        }
        Statement::BlockStatement(block) => scan_statements(&block.body, scan),
        Statement::ForStatement(loop_) => {
            scan.loops.push(statement);
            if let Some(init) = &loop_.init {
                if let Some(expression) = init.as_expression() {
                    scan_expression(expression, scan);
                } else if let ForStatementInit::VariableDeclaration(declaration) = init {
                    for declarator in &declaration.declarations {
                        if let Some(value) = &declarator.init {
                            scan_expression(value, scan);
                        }
                    }
                }
            }
            if let Some(test) = &loop_.test {
                scan_expression(test, scan);
            }
            if let Some(update) = &loop_.update {
                scan_expression(update, scan);
            }
            scan_statement(&loop_.body, scan);
        }
        Statement::WhileStatement(loop_) => {
            scan.loops.push(statement);
            scan_expression(&loop_.test, scan);
            scan_statement(&loop_.body, scan);
        }
        Statement::DoWhileStatement(loop_) => {
            scan.loops.push(statement);
            scan_statement(&loop_.body, scan);
            scan_expression(&loop_.test, scan);
        }
        // A nested declaration is a separate owned entity. Its body is proved
        // separately and must contribute nothing to the owner's proof.
        Statement::FunctionDeclaration(_) => {}
        _ => {}
    }
}

fn scan_expression<'a>(expression: &'a Expression<'a>, scan: &mut Scan<'a>) {
    match expression {
        Expression::CallExpression(call) => {
            scan.calls.push(call);
            scan_expression(&call.callee, scan);
            for argument in &call.arguments {
                if let Some(value) = argument.as_expression() {
                    scan_expression(value, scan);
                }
            }
        }
        Expression::StaticMemberExpression(member) => {
            scan.members.push(member);
            scan_expression(&member.object, scan);
        }
        Expression::ComputedMemberExpression(member) => {
            scan_expression(&member.object, scan);
            scan_expression(&member.expression, scan);
        }
        Expression::BinaryExpression(binary) => {
            scan.binaries.push(binary);
            scan_expression(&binary.left, scan);
            scan_expression(&binary.right, scan);
        }
        Expression::LogicalExpression(logical) => {
            scan_expression(&logical.left, scan);
            scan_expression(&logical.right, scan);
        }
        Expression::UnaryExpression(unary) => scan_expression(&unary.argument, scan),
        Expression::ConditionalExpression(conditional) => {
            scan_expression(&conditional.test, scan);
            scan_expression(&conditional.consequent, scan);
            scan_expression(&conditional.alternate, scan);
        }
        Expression::AssignmentExpression(assignment) => {
            scan.assignments.push(assignment);
            scan_expression(&assignment.right, scan);
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        scan_expression(&property.value, scan)
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        scan_expression(&spread.argument, scan)
                    }
                }
            }
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                if let ArrayExpressionElement::SpreadElement(spread) = element {
                    scan_expression(&spread.argument, scan);
                } else if let Some(value) = element.as_expression() {
                    scan_expression(value, scan);
                }
            }
        }
        Expression::SequenceExpression(sequence) => {
            for value in &sequence.expressions {
                scan_expression(value, scan);
            }
        }
        Expression::NewExpression(new) => {
            scan_expression(&new.callee, scan);
            for argument in &new.arguments {
                if let Some(value) = argument.as_expression() {
                    scan_expression(value, scan);
                }
            }
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            scan_expression(&parenthesized.expression, scan)
        }
        // Nested function/arrow bodies are separate entities.
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {}
        _ => {}
    }
}

fn loop_body<'a>(statement: &'a Statement<'a>) -> &'a [Statement<'a>] {
    match statement {
        Statement::BlockStatement(block) => &block.body,
        other => std::slice::from_ref(other),
    }
}

fn definitions<'a>(
    model: &Model<'_, '_>,
    scan: &'a Scan<'a>,
) -> Vec<(SymbolId, &'a Expression<'a>)> {
    let mut definitions: Vec<_> = scan
        .defs
        .iter()
        .map(|(symbol, value)| (*symbol, *value))
        .collect();
    for assignment in &scan.assignments {
        if assignment.operator.as_str() != "=" {
            continue;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
            continue;
        };
        if let Some(symbol) = crate::analysis::identifier_symbol(identifier, model.scoping) {
            definitions.push((symbol, &assignment.right));
        }
    }
    definitions
}

// ===========================================================================
// Iterator proof
// ===========================================================================

/// A bounded iteration whose cursor bounds `owner.length` and whose body reads
/// `list[cursor]`.
fn length_comparison<'a>(
    model: &Model<'_, '_>,
    test: &'a Expression<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    let Expression::BinaryExpression(binary) = test.without_parentheses() else {
        return None;
    };
    let (cursor_expression, length_expression) = match binary.operator.as_str() {
        "<" => (&binary.left, &binary.right),
        ">" => (&binary.right, &binary.left),
        _ => return None,
    };
    let cursor = symbol_of(model, cursor_expression)?;
    let owner = static_member(length_expression, "length")?;
    Some((cursor, owner))
}

fn breaks(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::BreakStatement(_) => true,
        Statement::BlockStatement(block) => block.body.iter().any(breaks),
        _ => false,
    }
}

/// `for (;;)`/`while` loops can carry the bound in a negated break guard
/// instead of the loop header. Both are the same bounded-iteration operation.
fn break_guard<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    for candidate in loop_body(statement) {
        let Statement::IfStatement(branch) = candidate else {
            continue;
        };
        if !breaks(&branch.consequent) {
            continue;
        }
        let test = branch.test.without_parentheses();
        if let Expression::UnaryExpression(unary) = test {
            if unary.operator.as_str() == "!" {
                if let Some(found) = length_comparison(model, &unary.argument) {
                    return Some(found);
                }
            }
        }
        if let Expression::BinaryExpression(binary) = test {
            if binary.operator.as_str() == ">=" {
                let Some(cursor) = symbol_of(model, &binary.left) else {
                    continue;
                };
                let Some(owner) = static_member(&binary.right, "length") else {
                    continue;
                };
                return Some((cursor, owner));
            }
        }
    }
    None
}

fn loop_cursor_and_length<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    match statement {
        Statement::ForStatement(loop_) => {
            if let Some(test) = &loop_.test {
                if let Some(found) = length_comparison(model, test) {
                    return Some(found);
                }
            }
            break_guard(model, &loop_.body)
        }
        Statement::WhileStatement(loop_) => {
            length_comparison(model, &loop_.test).or_else(|| break_guard(model, &loop_.body))
        }
        _ => None,
    }
}

/// A local index is the cursor itself or a single-origin alias of it. The
/// generated split form writes the loop counter into an outer local.
fn index_matches(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    expression: &Expression<'_>,
    cursor: SymbolId,
    proven_aliases: &BTreeSet<SymbolId>,
) -> bool {
    let Some(index) = symbol_of(model, expression) else {
        return false;
    };
    if index == cursor {
        return true;
    }
    // The callback is scanned as its own function, so its body cannot carry
    // the assignment made immediately before the callback call in a split
    // inner loop. The iterator proof records only aliases whose complete
    // inner-loop definition set is this cursor; use that scoped evidence
    // before falling back to the model-wide single-write lookup.
    if proven_aliases.contains(&index) {
        return true;
    }
    // A single plain write anywhere in the program is a proven alias even when
    // the callback body is scanned separately from the loop that writes it.
    if model.binding_writes(index) <= 1
        && model
            .symbol_value(index)
            .is_some_and(|value| symbol_of(model, value) == Some(cursor))
    {
        return true;
    }
    let mut values = definitions(model, scan)
        .into_iter()
        .filter(|(symbol, _)| *symbol == index)
        .map(|(_, value)| value);
    let Some(value) = values.next() else {
        return false;
    };
    if values.any(|other| symbol_of(model, other) != Some(cursor)) {
        return false;
    }
    symbol_of(model, value) == Some(cursor)
}

/// Aliases assigned in the split inner-loop body are not visible in a scan of
/// the directly-owned callback body. Keep only symbols whose every definition
/// in that loop body is the same loop cursor; this is the bounded equivalent of
/// the callback's `index = cursor` handoff and does not borrow unrelated outer
/// or nested-function assignments.
fn aliases_for_cursor(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    cursor: SymbolId,
) -> BTreeSet<SymbolId> {
    let candidates: BTreeSet<_> = definitions(model, scan)
        .into_iter()
        .map(|(symbol, _)| symbol)
        .filter(|symbol| *symbol != cursor)
        .collect();
    candidates
        .into_iter()
        .filter(|candidate| {
            let mut values = definitions(model, scan)
                .into_iter()
                .filter(|(symbol, _)| symbol == candidate)
                .map(|(_, value)| value);
            values
                .next()
                .is_some_and(|value| symbol_of(model, value) == Some(cursor))
                && values.all(|value| symbol_of(model, value) == Some(cursor))
        })
        .collect()
}

/// `record.addedNodes` or a local that only ever holds that exact member.
fn added_nodes_owner(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    expression: &Expression<'_>,
    record: SymbolId,
) -> bool {
    let expression = expression.without_parentheses();
    if is_added_nodes_direct(model, expression, record) {
        return true;
    }
    let Some(alias) = symbol_of(model, expression) else {
        return false;
    };
    let mut values = definitions(model, scan)
        .into_iter()
        .filter(|(symbol, _)| *symbol == alias)
        .map(|(_, value)| value);
    let Some(value) = values.next() else {
        return false;
    };
    if values.any(|other| !is_added_nodes_direct(model, other, record)) {
        return false;
    }
    is_added_nodes_direct(model, value, record)
}

fn is_added_nodes_direct(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    record: SymbolId,
) -> bool {
    static_member(expression, "addedNodes")
        .is_some_and(|owner| symbol_of(model, owner) == Some(record))
}

fn owned_callbacks(model: &Model<'_, '_>, outer: FunctionSite, scan: &Scan<'_>) -> BTreeSet<u32> {
    let mut callbacks = BTreeSet::new();
    for call in scan.calls.iter().copied() {
        let Some(callee) = symbol_of(model, &call.callee) else {
            continue;
        };
        let Some(target) = model.function_from_symbol(callee) else {
            continue;
        };
        if model.owning_site(target.node_id).map(|owner| owner.node_id) != Some(outer.node_id) {
            continue;
        }
        if !matches!(model.nodes.kind(target.node_id), AstKind::Function(_)) {
            continue;
        }
        callbacks.insert(target.span.start);
    }
    callbacks
}

struct Iteration {
    record: SymbolId,
    cursor: SymbolId,
    cursor_aliases: BTreeSet<SymbolId>,
    callbacks: BTreeSet<u32>,
}

/// The iterator's own body: records from the argument list bounded by
/// `records.length`, then an inner loop bounded by `record.addedNodes.length`
/// that calls exactly one function the iterator directly owns.
fn outer_iteration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<Iteration> {
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return None;
    }
    let [records_parameter] = function.params.items.as_slice() else {
        return None;
    };
    if records_parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(records_identifier) = &records_parameter.pattern else {
        return None;
    };
    let records = records_identifier.symbol_id.get()?;
    if model.binding_writes(records) != 0
        || !model.scoping.symbol_redeclarations(records).is_empty()
    {
        return None;
    }

    let mut body_scan = Scan::default();
    scan_statements(&body.statements, &mut body_scan);

    let mut found = Vec::new();
    let no_aliases = BTreeSet::new();
    for statement in body_scan.loops.iter().copied() {
        let Some((cursor, length_owner)) = loop_cursor_and_length(model, statement) else {
            continue;
        };
        if symbol_of(model, length_owner) != Some(records) {
            continue;
        }
        let mut loop_scan = Scan::default();
        scan_statements(loop_body(statement), &mut loop_scan);

        // (1) Each iteration reads one mutation record from the list.
        let mut record = None;
        for (candidate, value) in definitions(model, &loop_scan) {
            let Some((object, index)) = computed_pair(value) else {
                continue;
            };
            if symbol_of(model, object) != Some(records) {
                continue;
            }
            if !index_matches(model, &loop_scan, index, cursor, &no_aliases) {
                continue;
            }
            match record {
                None => record = Some(candidate),
                Some(existing) if existing == candidate => {}
                Some(_) => {
                    record = None;
                    break;
                }
            }
        }
        let Some(record) = record else {
            continue;
        };
        if record == records || record == cursor {
            continue;
        }

        // (2) A nested loop is bounded by exactly that record's added nodes and
        // invokes an owned callback.
        let mut inner = Vec::new();
        for inner_loop in loop_scan.loops.iter().copied() {
            let Some((inner_cursor, length_owner)) = loop_cursor_and_length(model, inner_loop)
            else {
                continue;
            };
            if !added_nodes_owner(model, &loop_scan, length_owner, record) {
                continue;
            }
            let mut inner_scan = Scan::default();
            scan_statements(loop_body(inner_loop), &mut inner_scan);
            let cursor_aliases = aliases_for_cursor(model, &inner_scan, inner_cursor);
            let callbacks = owned_callbacks(model, site, &inner_scan);
            // The claimed shape has exactly one directly-owned callback. A
            // second owned callback invocation is a different body and must not
            // borrow the iterator proof.
            if callbacks.len() != 1 {
                continue;
            }
            inner.push((inner_cursor, cursor_aliases, callbacks));
        }
        if inner.len() != 1 {
            continue;
        }
        let (cursor, cursor_aliases, callbacks) = inner.pop()?;
        found.push(Iteration {
            record,
            cursor,
            cursor_aliases,
            callbacks,
        });
    }
    if found.len() != 1 {
        return None;
    }
    found.pop()
}

// ===========================================================================
// Owned inspection-callback proof
// ===========================================================================

fn captured(model: &Model<'_, '_>, site: FunctionSite, target: SymbolId) -> bool {
    !site
        .span
        .contains_inclusive(model.scoping.symbol_span(target))
        && model.scoping.symbol_redeclarations(target).is_empty()
}

fn numeric_one(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::NumericLiteral(number) if number.value == 1.0
    )
}

/// The `nodeType === 1` guard must be the same `if` that owns the paired
/// DIV/STYLE chain: the chain sits in the consequent of an equality test or in
/// the alternate of an inequality test. A node-type comparison somewhere else in
/// the callback donates nothing.
fn node_type_guards(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    node: SymbolId,
    div: &IfStatement<'_>,
    style: &IfStatement<'_>,
) -> bool {
    scan.ifs.iter().copied().any(|branch| {
        let Expression::BinaryExpression(binary) = branch.test.without_parentheses() else {
            return false;
        };
        let positive = match binary.operator.as_str() {
            "==" | "===" => true,
            "!=" | "!==" => false,
            _ => return false,
        };
        let mentions_node_type = (member_on(model, &binary.left, node, "nodeType")
            && numeric_one(&binary.right))
            || (member_on(model, &binary.right, node, "nodeType") && numeric_one(&binary.left));
        if !mentions_node_type {
            return false;
        }
        let region = if positive {
            &branch.consequent
        } else {
            let Some(alternate) = &branch.alternate else {
                return false;
            };
            alternate
        };
        contains_if(region, div) && contains_if(region, style)
    })
}

fn contains_if(region: &Statement<'_>, branch: &IfStatement<'_>) -> bool {
    match region {
        Statement::IfStatement(candidate) => {
            std::ptr::eq(&**candidate, branch)
                || contains_if(&candidate.consequent, branch)
                || candidate
                    .alternate
                    .as_ref()
                    .is_some_and(|alternate| contains_if(alternate, branch))
        }
        Statement::BlockStatement(block) => block.body.iter().any(|item| contains_if(item, branch)),
        _ => false,
    }
}

fn tag_branch(model: &Model<'_, '_>, test: &Expression<'_>, node: SymbolId, tag: &str) -> bool {
    let Expression::BinaryExpression(binary) = test.without_parentheses() else {
        return false;
    };
    if !matches!(binary.operator.as_str(), "==" | "===") {
        return false;
    }
    (member_on(model, &binary.left, node, "tagName")
        && matches!(
            binary.right.without_parentheses(),
            Expression::StringLiteral(string) if string.value == tag
        ))
        || (member_on(model, &binary.right, node, "tagName")
            && matches!(
                binary.left.without_parentheses(),
                Expression::StringLiteral(string) if string.value == tag
            ))
}

fn is_else_if(outer: &IfStatement<'_>, inner: &IfStatement<'_>) -> bool {
    matches!(
        &outer.alternate,
        Some(Statement::IfStatement(next)) if next.test.span() == inner.test.span()
    )
}

fn branch_scan<'a>(branch: &'a IfStatement<'a>) -> Scan<'a> {
    let mut scan = Scan::default();
    scan_statements(loop_body(&branch.consequent), &mut scan);
    scan
}

/// `Array.from(node.style)` or `[...node.style]`; the provider must be the
/// unshadowed global `Array` and the receiver the proved node identity.
fn style_names(model: &Model<'_, '_>, expression: &Expression<'_>, node: SymbolId) -> bool {
    let expression = expression.without_parentheses();
    if let Expression::ArrayExpression(array) = expression {
        return array.elements.iter().any(|element| match element {
            ArrayExpressionElement::SpreadElement(spread) => {
                member_on(model, &spread.argument, node, "style")
            }
            _ => false,
        });
    }
    let Some(call) = plain_call(expression) else {
        return false;
    };
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return false;
    };
    if member.optional
        || member.property.name != "from"
        || !unbound_global(model, &member.object, "Array")
    {
        return false;
    }
    exact_arguments(call, 1).is_some_and(|arguments| member_on(model, &arguments[0], node, "style"))
}

fn call_on(
    model: &Model<'_, '_>,
    call: &CallExpression<'_>,
    receiver: SymbolId,
    name: &str,
    count: usize,
) -> bool {
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return false;
    };
    !member.optional
        && member.property.name == name
        && symbol_of(model, &member.object) == Some(receiver)
        && exact_arguments(call, count).is_some()
}

fn branch_flag(model: &Model<'_, '_>, scan: &Scan<'_>, excluded: &[SymbolId]) -> Option<SymbolId> {
    let mut flag = None;
    for assignment in scan.assignments.iter().copied() {
        if assignment.operator.as_str() != "=" {
            continue;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
            continue;
        };
        let Some(target) = crate::analysis::identifier_symbol(identifier, model.scoping) else {
            continue;
        };
        if excluded.contains(&target) || !boolean_true(&assignment.right) {
            continue;
        }
        match flag {
            None => flag = Some(target),
            Some(existing) if existing == target => {}
            Some(_) => return None,
        }
    }
    flag
}

/// The call path an object literal is handed to. The payload sink is opaque, so
/// the same scoped callee identity is the only source-level link between the
/// two probe branches.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CallKey {
    Symbol(SymbolId),
    Member(SymbolId, String),
}

fn call_key(model: &Model<'_, '_>, call: &CallExpression<'_>) -> Option<CallKey> {
    match call.callee.without_parentheses() {
        Expression::Identifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, model.scoping).map(CallKey::Symbol)
        }
        Expression::StaticMemberExpression(member) if !member.optional => {
            symbol_of(model, &member.object)
                .map(|symbol| CallKey::Member(symbol, member.property.name.to_string()))
        }
        _ => None,
    }
}

/// Probe evidence carried by object literals that the branch passes to a call:
/// the captured flag symbols they read and whether they also report the
/// branch's own just-assigned state as the literal `true`. Keyed by the scoped
/// call path so a free-floating object or a decoy call cannot borrow the sink.
#[derive(Default)]
struct Payload {
    flags: BTreeSet<SymbolId>,
    true_literal: bool,
}

fn payload_flags(model: &Model<'_, '_>, scan: &Scan<'_>) -> BTreeMap<CallKey, Payload> {
    let mut payloads: BTreeMap<CallKey, Payload> = BTreeMap::new();
    for call in scan.calls.iter().copied() {
        let Some(key) = call_key(model, call) else {
            continue;
        };
        for argument in &call.arguments {
            let Some(value) = argument.as_expression() else {
                continue;
            };
            let Expression::ObjectExpression(object) = value.without_parentheses() else {
                continue;
            };
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    continue;
                };
                if property.computed || property.method || property.kind != PropertyKind::Init {
                    continue;
                }
                let payload = payloads.entry(key.clone()).or_default();
                if let Some(symbol) = symbol_of(model, &property.value) {
                    payload.flags.insert(symbol);
                }
                if boolean_true(&property.value) {
                    payload.true_literal = true;
                }
            }
        }
    }
    payloads
}

/// Inline-style signature probe: enumerate the node's own style property names,
/// sort them, join the `name;name` signature (or `name:value;...`), digest it
/// with the opaque consumer and flip one captured probe flag.
fn div_signature(
    model: &Model<'_, '_>,
    branch: &IfStatement<'_>,
    node: SymbolId,
) -> Option<SymbolId> {
    let scan = branch_scan(branch);
    // A style-name candidate is the array identity only when the same symbol is
    // sorted and its own `join(";")` is the digest argument. Any other same-shape
    // array in the branch (a derived text array, for example) donates nothing.
    let mut props = None;
    for (candidate, value) in definitions(model, &scan) {
        if !style_names(model, value, node) {
            continue;
        }
        let sorted = scan
            .calls
            .iter()
            .copied()
            .any(|call| call_on(model, call, candidate, "sort", 0));
        if !sorted || !digest_compare(model, &scan, Some(candidate), ";") {
            continue;
        }
        if props.is_some_and(|existing| existing != candidate) {
            return None;
        }
        props = Some(candidate);
    }
    let props = props?;
    branch_flag(model, &scan, &[node, props])
}

/// Array operations that carry the split line list forward without changing the
/// scoped array identity: `map`, `filter`, `slice`, `concat`, the sort family
/// and the other array-returning transforms. Their callback bodies (per-line
/// `trim`/`filter`, for example) are opaque and contribute no evidence.
fn line_transform(name: &str) -> bool {
    matches!(
        name,
        "map"
            | "filter"
            | "slice"
            | "sort"
            | "reverse"
            | "concat"
            | "flat"
            | "flatMap"
            | "toSorted"
            | "toReversed"
    )
}

/// The exact `node.textContent.split("\n")` call: the proved node identity is
/// the `textContent` receiver and the separator is the line split.
fn text_lines_split(model: &Model<'_, '_>, expression: &Expression<'_>, node: SymbolId) -> bool {
    let Some(call) = plain_call(expression) else {
        return false;
    };
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return false;
    };
    !member.optional
        && member.property.name == "split"
        && member_on(model, &member.object, node, "textContent")
        && exact_arguments(call, 1).is_some_and(|arguments| string_with(&arguments[0], "\n"))
}

/// Provenance of the one line array: the node's text split, carried through
/// line-preserving array transforms and saved intermediates. The callback
/// bodies of `map`/`filter` are opaque; only the array receiver and its saved
/// symbol identity are evidence. The digest may only consume a `join` on a
/// symbol this proves, so an unrelated array/join/digest trio in the same branch
/// donates nothing.
fn line_array(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    expression: &Expression<'_>,
    node: SymbolId,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    let expression = expression.without_parentheses();
    if text_lines_split(model, expression, node) {
        return true;
    }
    if let Some(call) = plain_call(expression) {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return false;
        };
        if member.optional || !line_transform(member.property.name.as_str()) {
            return false;
        }
        return line_array(model, scan, &member.object, node, depth - 1);
    }
    let Some(symbol) = symbol_of(model, expression) else {
        return false;
    };
    let mut values = definitions(model, scan)
        .into_iter()
        .filter(|(candidate, _)| *candidate == symbol)
        .map(|(_, value)| value);
    let Some(value) = values.next() else {
        return false;
    };
    line_array(model, scan, value, node, depth - 1)
        && values.all(|other| line_array(model, scan, other, node, depth - 1))
}

/// Text-content style signature probe: split the node's text content into a
/// line array, carry that exact array through line-preserving transforms to the
/// symbol whose `join("\n")` is digested by the opaque consumer, and flip a
/// second captured probe flag. Per-line callback bodies are opaque.
fn style_signature(
    model: &Model<'_, '_>,
    branch: &IfStatement<'_>,
    node: SymbolId,
) -> Option<SymbolId> {
    let scan = branch_scan(branch);
    if !scan.members.iter().copied().any(|member| {
        member.property.name == "textContent" && symbol_of(model, &member.object) == Some(node)
    }) {
        return None;
    }
    // The split result is one scoped array identity: every join/digest that can
    // donate the signature must consume a symbol whose every definition is
    // `node.textContent.split("\n")` carried through line-preserving transforms.
    let mut line_arrays = BTreeSet::new();
    let mut mixed = BTreeSet::new();
    for (candidate, value) in definitions(model, &scan) {
        if candidate == node {
            continue;
        }
        if line_array(model, &scan, value, node, 6) {
            line_arrays.insert(candidate);
        } else {
            mixed.insert(candidate);
        }
    }
    line_arrays.retain(|symbol| !mixed.contains(symbol));
    let joined = line_arrays
        .iter()
        .copied()
        .any(|lines| digest_compare(model, &scan, Some(lines), "\n"));
    if !joined {
        return None;
    }
    branch_flag(model, &scan, &[node])
}

fn style_inspector(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if function.r#async
        || function.generator
        || !function.params.items.is_empty()
        || function.params.rest.is_some()
    {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let owner = model.owning_site(site.node_id)?;
    if owner.node_id == site.node_id {
        return None;
    }
    let AstKind::Function(owner_function) = model.nodes.kind(owner.node_id) else {
        return None;
    };
    let owner_body = owner_function.body.as_ref()?;
    let iteration = outer_iteration(model, owner, owner_function, owner_body)?;
    if !iteration.callbacks.contains(&site.span.start) {
        return None;
    }

    let mut scan = Scan::default();
    scan_statements(&body.statements, &mut scan);

    // (1) The added node is read from the iterator's exact record and cursor.
    let mut nodes = BTreeSet::new();
    for (candidate, value) in definitions(model, &scan) {
        let Some((object, index)) = computed_pair(value) else {
            continue;
        };
        if !added_nodes_owner(model, &scan, object, iteration.record) {
            continue;
        }
        if !index_matches(
            model,
            &scan,
            index,
            iteration.cursor,
            &iteration.cursor_aliases,
        ) {
            continue;
        }
        nodes.insert(candidate);
    }
    if nodes.len() != 1 {
        return None;
    }
    let node = *nodes.iter().next()?;
    if node == iteration.record || node == iteration.cursor {
        return None;
    }

    // (2) A paired DIV/STYLE probe chain, either order, guarded by the same
    // `nodeType === 1` branch. A disconnected node-type check donates nothing.
    let div = scan
        .ifs
        .iter()
        .copied()
        .find(|branch| tag_branch(model, &branch.test, node, "DIV"))?;
    let style = scan
        .ifs
        .iter()
        .copied()
        .find(|branch| tag_branch(model, &branch.test, node, "STYLE"))?;
    if !is_else_if(div, style) && !is_else_if(style, div) {
        return None;
    }
    if !node_type_guards(model, &scan, node, div, style) {
        return None;
    }

    // (3) Two independent signatures and their distinct captured flags.
    let div_flag = div_signature(model, div, node)?;
    let style_flag = style_signature(model, style, node)?;
    if div_flag == style_flag {
        return None;
    }
    if !captured(model, site, div_flag) || !captured(model, site, style_flag) {
        return None;
    }

    // (4) The flags are read by objects that both probe branches pass to the
    // same scoped payload-call path. A free-floating or differently-called
    // object literal donates nothing.
    let div_payloads = payload_flags(model, &branch_scan(div));
    let style_payloads = payload_flags(model, &branch_scan(style));
    let linked = div_payloads.iter().any(|(key, div)| {
        let Some(style) = style_payloads.get(key) else {
            return false;
        };
        // Each probe branch must report the *other* probe's captured flag and
        // its own just-assigned state (the literal `true`, or the flag symbol
        // itself). A branch that reports only its own state is a different
        // body; free-floating or differently-called objects donate nothing.
        let div_ok =
            div.flags.contains(&style_flag) && (div.flags.contains(&div_flag) || div.true_literal);
        let style_ok = style.flags.contains(&div_flag)
            && (style.flags.contains(&style_flag) || style.true_literal);
        div_ok && style_ok
    });
    if !linked {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if style_inspector(model, site).is_some() {
        return vec![proposal(
            "inspectMutationStyleSignature",
            "posthost_mutation_style_inspector",
            "whole owned callback body: added node read from the owning iterator's exact record.addedNodes[cursor], the same nodeType===1 branch contains the paired tagName DIV/STYLE else-if probes, Array.from(node.style)+sort+join(;)-digest DIV signature, node.textContent split(\\n) line array carried through array-preserving transforms to the join(\\n) digest STYLE signature, and two distinct captured probe flags read by branch payload objects handed to the same scoped call path; digest consumer and payload sink are opaque protocol inputs, receiver identities are the proved node, the split-derived line array and the style array, and map/filter callback bodies are opaque",
            f,
        )];
    }

    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if outer_iteration(model, site, function, body).is_some() {
        return vec![proposal(
            "iterateMutationAddedNodes",
            "posthost_mutation_style_iterator",
            "whole owned iterator body: records argument iterated by a while/for cursor bounded by records.length, each iteration reads records[cursor] as the mutation record, then a nested while/for/for(;;) cursor bounded by exactly record.addedNodes.length invokes the single function this iterator directly owns; the function's own body never contributes to this proof, cursor aliases are single-origin, and local spellings, source order, vendor data, spans and digest dictionaries are not evidence",
            f,
        )];
    }
    Vec::new()
}
