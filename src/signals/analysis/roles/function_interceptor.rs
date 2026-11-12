//! Function-valued property descriptor interceptor proof (method interceptor).
//!
//! Shared by the signal finder roles (`request_output_carriers`) and the naming rule in
//! `naming::functions::runtime::function_hooks`.
//!
//! The whole owned parent body is proved from source, never from a generated
//! spelling, a thread id, a source position, a vendor or a fingerprint:
//!
//! ```text
//! function interceptor(object, key, optional) {
//!   optional = optional === void 0 ? false : optional;
//!   let record = <descriptorLookup>(object, key);
//!   if (record == null) return null;
//!   let containing = record.containingObj;
//!   record = record.desc;
//!   let configurable = record.configurable;
//!   let writable = record.writable;
//!   let value = record.value;
//!   if (!<hasOwnProperty>(record, "value")) return null;
//!   let originals = Object.create(null);
//!   originals.value = value;
//!   if (configurable === false && writable === false
//!       || typeof value !== "function") {
//!     return { originals: originals };
//!   } else {
//!     let cached = <cacheLookup>(containing, key);
//!     if (cached != null) { ...cached.result / cached.original... }
//!     let onBeforeInvoke = <hookFactory>();
//!     let onAfterInvoke = <hookFactory>();
//!     function wrapper() {
//!       "use strict";
//!       ...onBeforeInvoke.notify(...) / onAfterInvoke.notify(...)...
//!       ...value.apply/call... try/finally ...bypassResult...
//!     }
//!     record.value = wrapper;
//!     ...Function.prototype.toString capture / push...
//!     let names = Object.getOwnPropertyNames(value);
//!     for (let i = 0; i < names.length; i++) {        // or while
//!       if (<indexOf>(excluded, names[i]) === -1) {
//!         let own = Object.getOwnPropertyDescriptor(wrapper, names[i]);
//!         let from = Object.getOwnPropertyDescriptor(value, names[i]);
//!         ...Object.defineProperty(wrapper, names[i], from);
//!       }
//!     }
//!     ...wrapper.prototype = void 0 when the original has none...
//!     Object.defineProperty(containing, key, record);
//!     result = { onBeforeInvoke: onBeforeInvoke,
//!                onAfterInvoke: onAfterInvoke,
//!                originals: originals };
//!     ...registry[key].push({ object, result, original })...
//!     return result;
//!   }
//! }
//! ```
//!
//! Every binding comparison uses the existing scope index. The descriptor
//! lookup, the cached-original lookup and the hook factory are consumer
//! protocol inputs: their spelling and origin are not evidence, but their
//! argument slots, their result fields and the exact local identities that
//! consume them are. `Object.*`, `Array.prototype.*` and
//! `Function.prototype.toString` must be the unshadowed globals, either called
//! directly or through a call-through alias whose first argument is the
//! unshadowed global. The nested wrapper must be owned by this exact function
//! (a direct child) and must independently consume both hook identities, the
//! original value, `arguments`, `bypassResult` and a try/finally. A node-order
//! variant may move statements; a `for` loop, a `for (let ...)` loop and a
//! `while` loop are the same property-copy operation. The early return may
//! publish `{ originals }` inline or save it in a branch-local first and
//! return that same local. Local spellings are opaque.
//!
//! # Inline lookups
//!
//! The cached captures resolve the descriptor record and the
//! cached-original record through opaque two-argument helpers. The remaining
//! captures inline the same operations instead, and are proved the
//! same way from their own body:
//!
//! * the descriptor walk starts a local cursor from formal slot 0, clears the
//!   record, guards the cursor with `== null` by setting the record to `null`
//!   and breaking, reads `Object.getOwnPropertyDescriptor(cursor, key)` with
//!   the exact key formal slot 1, advances the same cursor through
//!   `Object.getPrototypeOf(cursor)` on a descriptor miss, and builds exactly
//!   `{ containingObj: cursor, desc }` on a hit (directly or through the cursor
//!   local) before breaking;
//! * the per-property registry scan guards `registry[key]` with `== null`,
//!   scans `registry[key][cursor]` within `registry[key].length`, and selects
//!   the element whose `.object` is exactly the containing object.
//!
//! Both inline forms keep the existing helper-call forms accepted and feed the
//! same early-return, hook-factory, owned-wrapper, descriptor-copy,
//! `Object.defineProperty` and registry-record invariants.
use crate::signals::extract::bindings::{
    call_arguments, expression_path, property_key_name, FunctionSite, Model,
};
use std::collections::{BTreeMap, BTreeSet};
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::SymbolId;


fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression, model.scoping)
}

fn static_member<'a>(expression: &'a Expression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn indexed<'a>(expression: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    (!member.optional).then_some((&member.object, &member.expression))
}

pub(crate) fn exact_arguments<'a>(call: &'a CallExpression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    let values = call_arguments(call);
    (values.len() == call.arguments.len()).then_some(values)
}

pub(crate) fn is_null_literal(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::NullLiteral(_))
}

pub(crate) fn plain_assignment<'a>(statement: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &expression.expression else {
        return None;
    };
    (assignment.operator.as_str() == "=").then_some(assignment)
}

pub(crate) fn is_break(statement: &Statement<'_>) -> bool {
    matches!(statement, Statement::BreakStatement(branch) if branch.label.is_none())
}

pub(crate) fn is_continue(statement: &Statement<'_>) -> bool {
    matches!(statement, Statement::ContinueStatement(branch) if branch.label.is_none())
}

pub(crate) fn call_of<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return None;
    };
    (!call.optional).then_some(call)
}

fn null_compared_symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    if !matches!(binary.operator.as_str(), "==" | "===") {
        return None;
    }
    if is_null_literal(&binary.right) {
        symbol_of(model, &binary.left)
    } else if is_null_literal(&binary.left) {
        symbol_of(model, &binary.right)
    } else {
        None
    }
}

fn not_null_compared(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    candidate: SymbolId,
) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    matches!(binary.operator.as_str(), "!=" | "!==")
        && ((symbol_of(model, &binary.left) == Some(candidate) && is_null_literal(&binary.right))
            || (symbol_of(model, &binary.right) == Some(candidate)
                && is_null_literal(&binary.left)))
}

pub(crate) fn plain_local_in_body(model: &Model<'_, '_>, site: FunctionSite, symbol: SymbolId) -> bool {
    model.formal_symbol_slot(symbol).is_none()
        && model.scoping.symbol_redeclarations(symbol).is_empty()
        && site
            .body_span
            .unwrap_or(site.span)
            .contains_inclusive(model.scoping.symbol_span(symbol))
}

/// The infinite traversal loop shared by the inline descriptor walk and the
/// inline registry scan. `for (;;)` and `while (true)` are the same operation.
pub(crate) fn loop_body<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    match statement {
        Statement::ForStatement(loop_)
            if loop_.init.is_none() && loop_.test.is_none() && loop_.update.is_none() =>
        {
            block(&loop_.body)
        }
        Statement::WhileStatement(loop_)
            if matches!(
                loop_.test.without_parentheses(),
                Expression::BooleanLiteral(boolean) if boolean.value
            ) =>
        {
            block(&loop_.body)
        }
        _ => None,
    }
}

/// `registry[key]` where `key` is the installer's exact second parameter.
fn keyed_bucket<'a>(
    model: &Model<'_, '_>,
    expression: &'a Expression<'a>,
    site: FunctionSite,
) -> Option<(&'a Expression<'a>, SymbolId)> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    if member.optional || model.site_parameter_slot(&member.expression, site) != Some(1) {
        return None;
    }
    let registry = symbol_of(model, &member.object)?;
    Some((&member.object, registry))
}

fn plain_parameter(model: &Model<'_, '_>, parameter: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = parameter else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    (model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty())
        .then_some(symbol)
}

pub(crate) fn block<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(block) = statement else {
        return None;
    };
    Some(&block.body)
}

/// A call-through alias keeps the unshadowed global identity of its first
/// argument: `check(Object.hasOwnProperty, target, "value")` and
/// `check(Array.prototype.indexOf, excluded, element)` are the same operations
/// as the direct calls. The alias binding itself is not role evidence.
fn call_through<'a>(
    model: &Model<'_, '_>,
    call: &'a CallExpression<'a>,
    path: &[&str],
) -> Option<Vec<&'a Expression<'a>>> {
    let arguments = exact_arguments(call)?;
    if expression_path(model, &call.callee) == path {
        return Some(arguments);
    }
    let (first, rest) = arguments.split_first()?;
    (expression_path(model, first) == path).then(|| rest.to_vec())
}

fn has_own_property<'a>(
    model: &Model<'_, '_>,
    call: &'a CallExpression<'a>,
) -> Option<Vec<&'a Expression<'a>>> {
    for path in [
        &["Object", "hasOwnProperty"][..],
        &["Object", "prototype", "hasOwnProperty"][..],
    ] {
        if let Some(arguments) = call_through(model, call, path) {
            return Some(arguments);
        }
    }
    None
}

pub(crate) fn direct_call<'a>(
    model: &Model<'_, '_>,
    call: &'a CallExpression<'a>,
    path: &[&str],
) -> Option<Vec<&'a Expression<'a>>> {
    (expression_path(model, &call.callee) == path).then(|| exact_arguments(call))?
}

#[derive(Default)]
pub(crate) struct Scan<'a> {
    defs: BTreeMap<SymbolId, &'a Expression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    calls: Vec<&'a CallExpression<'a>>,
    members: Vec<&'a StaticMemberExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    ifs: Vec<&'a IfStatement<'a>>,
    pub(crate) loops: Vec<&'a Statement<'a>>,
    tries: Vec<&'a TryStatement<'a>>,
    arguments: bool,
}

pub(crate) fn scan_statements<'a>(statements: &'a [Statement<'a>], scan: &mut Scan<'a>) {
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
                    if let Some(symbol) = plain_local_symbol(declarator) {
                        scan.defs.entry(symbol).or_insert(init);
                    }
                }
            }
        }
        Statement::ExpressionStatement(expression) => scan_expression(&expression.expression, scan),
        Statement::ReturnStatement(return_) => {
            if let Some(argument) = &return_.argument {
                scan.returns.push(argument);
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
        Statement::TryStatement(try_) => {
            scan.tries.push(try_);
            scan_statements(&try_.block.body, scan);
            if let Some(handler) = &try_.handler {
                scan_statements(&handler.body.body, scan);
            }
            if let Some(finalizer) = &try_.finalizer {
                scan_statements(&finalizer.body, scan);
            }
        }
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
        Statement::ThrowStatement(throw_) => {
            scan_expression(&throw_.argument, scan);
        }
        // A nested declaration is a separate owned entity. Its body is proved
        // separately and must contribute nothing to the installer's proof.
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
                if let Some(value) = element.as_expression() {
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
        Expression::Identifier(identifier) => {
            if identifier.name == "arguments" {
                scan.arguments = true;
            }
        }
        // Nested function/arrow bodies are separate entities.
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {}
        _ => {}
    }
}

pub(crate) fn plain_local_symbol(declarator: &VariableDeclarator<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
        return None;
    };
    identifier.symbol_id.get()
}

pub(crate) fn assignment_identifier(
    model: &Model<'_, '_>,
    assignment: &AssignmentExpression<'_>,
) -> Option<SymbolId> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

pub(crate) fn assignment_member<'a>(
    assignment: &'a AssignmentExpression<'a>,
) -> Option<(&'a Expression<'a>, &'a str)> {
    let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
        return None;
    };
    (!member.optional).then_some((&member.object, member.property.name.as_str()))
}

fn assigned_value<'a>(
    model: &Model<'_, '_>,
    scan: &'a Scan<'a>,
    symbol: SymbolId,
) -> Option<&'a Expression<'a>> {
    scan.assignments.iter().find_map(|assignment| {
        (assignment.operator.as_str() == "="
            && assignment_identifier(model, assignment) == Some(symbol))
        .then_some(&assignment.right)
    })
}

fn definition<'a>(
    model: &Model<'_, '_>,
    scan: &'a Scan<'a>,
    symbol: SymbolId,
) -> Option<&'a Expression<'a>> {
    scan.defs
        .get(&symbol)
        .copied()
        .or_else(|| assigned_value(model, scan, symbol))
}

/// Every single-origin definition in source order: declarator initializers and
/// plain `x = value` assignments. Local spelling and declaration form are
/// opaque; only the symbol identity and the value shape matter.
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
        if let Some(symbol) = assignment_identifier(model, assignment) {
            definitions.push((symbol, &assignment.right));
        }
    }
    definitions
}

pub(crate) fn is_return_null(statement: &Statement<'_>) -> bool {
    let Some(statements) = block(statement) else {
        return false;
    };
    let [Statement::ReturnStatement(return_)] = statements else {
        return false;
    };
    matches!(&return_.argument, Some(Expression::NullLiteral(_)))
}

pub(crate) fn symbol_of(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    symbol(model, expression.without_parentheses())
}

pub(crate) fn is_null_compare(model: &Model<'_, '_>, expression: &Expression<'_>, symbol: SymbolId) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    matches!(binary.operator.as_str(), "==" | "===")
        && ((symbol_of(model, &binary.left) == Some(symbol)
            && matches!(&binary.right, Expression::NullLiteral(_)))
            || (symbol_of(model, &binary.right) == Some(symbol)
                && matches!(&binary.left, Expression::NullLiteral(_))))
}

pub(crate) fn literal_object<'a>(
    expression: &'a Expression<'a>,
) -> Option<BTreeMap<String, &'a Expression<'a>>> {
    let Expression::ObjectExpression(object) = expression.without_parentheses() else {
        return None;
    };
    let mut members = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return None;
        }
        let key = property_key_name(&property.key)?;
        if members.insert(key, &property.value).is_some() {
            return None;
        }
    }
    Some(members)
}

pub(crate) fn member_on(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    symbol: SymbolId,
    name: &str,
) -> bool {
    static_member(expression.without_parentheses(), name)
        .is_some_and(|object| symbol_of(model, object) == Some(symbol))
}

fn only_originals<'a>(
    model: &Model<'_, '_>,
    members: &BTreeMap<String, &'a Expression<'a>>,
    originals: SymbolId,
) -> bool {
    members.len() == 1
        && members
            .get("originals")
            .is_some_and(|value| symbol_of(model, value) == Some(originals))
}

fn returns_originals(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    statement: &Statement<'_>,
    originals: SymbolId,
) -> bool {
    let Some(statements) = block(statement) else {
        return false;
    };
    let Some((return_index, argument)) =
        statements.iter().enumerate().find_map(|(index, entry)| {
            let Statement::ReturnStatement(return_) = entry else {
                return None;
            };
            Some((index, return_.argument.as_ref()?))
        })
    else {
        return false;
    };
    match argument.without_parentheses() {
        Expression::ObjectExpression(_) => literal_object(argument)
            .is_some_and(|members| only_originals(model, &members, originals)),
        Expression::Identifier(_) => {
            let Some(symbol) = symbol_of(model, argument) else {
                return false;
            };
            // The early return may publish the null-prototype `originals`
            // holder through a local that this same branch rewrites to the
            // result record (`saved = { originals: saved }; return saved;`).
            // Only a write before the returned statement in this exact branch
            // can supply the returned record.
            let mut local = None;
            for entry in &statements[..return_index] {
                let Statement::ExpressionStatement(expression) = entry else {
                    continue;
                };
                let Expression::AssignmentExpression(assignment) = &expression.expression else {
                    continue;
                };
                if assignment.operator.as_str() != "="
                    || assignment_identifier(model, assignment) != Some(symbol)
                {
                    continue;
                }
                let Some(members) = literal_object(&assignment.right) else {
                    // A branch that rewrites the returned local to anything
                    // other than a literal record is not the proven publish
                    // shape.
                    return false;
                };
                if local.replace(members).is_some() {
                    return false;
                }
            }
            if let Some(members) = &local {
                // The rewritten record must be the originals holder itself;
                // otherwise its `originals` field would capture an unrelated
                // earlier value.
                return symbol == originals && only_originals(model, members, originals);
            }
            definition(model, scan, symbol)
                .and_then(literal_object)
                .is_some_and(|members| only_originals(model, &members, originals))
        }
        _ => false,
    }
}

fn early_condition(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    configurable: SymbolId,
    writable: SymbolId,
    value: SymbolId,
) -> bool {
    let Expression::LogicalExpression(or) = expression.without_parentheses() else {
        return false;
    };
    if or.operator.as_str() != "||" {
        return false;
    }
    let Expression::LogicalExpression(and) = or.left.without_parentheses() else {
        return false;
    };
    if and.operator.as_str() != "&&" {
        return false;
    }
    let is_false = |expression: &Expression<'_>, symbol: SymbolId| {
        matches!(expression.without_parentheses(),
            Expression::BinaryExpression(binary)
                if binary.operator.as_str() == "==="
                    && symbol_of(model, &binary.left) == Some(symbol)
                    && matches!(&binary.right, Expression::BooleanLiteral(boolean) if !boolean.value))
    };
    is_false(&and.left, configurable)
        && is_false(&and.right, writable)
        && matches!(
            or.right.without_parentheses(),
            Expression::BinaryExpression(binary)
                if binary.operator.as_str() == "!=="
                    && matches!(&binary.left, Expression::UnaryExpression(unary)
                        if unary.operator.as_str() == "typeof"
                            && symbol_of(model, &unary.argument) == Some(value))
                    && matches!(&binary.right, Expression::StringLiteral(string) if string.value == "function")
        )
}

fn unique_field(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    record: SymbolId,
    name: &str,
) -> Option<SymbolId> {
    let mut found = None;
    for (symbol, init) in definitions(model, scan) {
        if member_on(model, init, record, name) {
            if found.replace(symbol).is_some() {
                return None;
            }
        }
    }
    found
}

fn has_value_guard(model: &Model<'_, '_>, scan: &Scan<'_>, record: SymbolId, name: &str) -> bool {
    scan.ifs.iter().any(|branch| {
        branch.alternate.is_none()
            && is_return_null(&branch.consequent)
            && matches!(
                branch.test.without_parentheses(),
                Expression::UnaryExpression(unary)
                    if unary.operator.as_str() == "!"
                        && matches!(&unary.argument, Expression::CallExpression(call)
                            if has_own_property(model, call).is_some_and(|arguments| {
                                arguments.len() == 2
                                    && symbol_of(model, arguments[0]) == Some(record)
                                    && matches!(&arguments[1], Expression::StringLiteral(string)
                                        if string.value == name)
                            }))
            )
    })
}

/// The already accepted helper-call cache lookup: `<cacheLookup>(owner, key)`.
/// Its spelling and origin are not evidence; the argument slots and the exact
/// result/original dataflow that consume it are.
fn helper_cache(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    site: FunctionSite,
    containing: SymbolId,
) -> Option<SymbolId> {
    let mut cached = None;
    for (symbol, init) in definitions(model, scan) {
        let Expression::CallExpression(call) = init.without_parentheses() else {
            continue;
        };
        let Some(arguments) = exact_arguments(call) else {
            continue;
        };
        if arguments.len() == 2
            && symbol_of(model, arguments[0]) == Some(containing)
            && model.site_parameter_slot(arguments[1], site) == Some(1)
        {
            if cached.replace(symbol).is_some() {
                return None;
            }
        }
    }
    cached
}

fn cache_dataflow(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    cached: SymbolId,
    value: SymbolId,
    originals: SymbolId,
) -> bool {
    let result_return = scan
        .returns
        .iter()
        .any(|returned| member_on(model, returned, cached, "result"));
    let original_rebind = scan.assignments.iter().any(|assignment| {
        assignment.operator.as_str() == "="
            && assignment_identifier(model, assignment) == Some(value)
            && member_on(model, &assignment.right, cached, "original")
    });
    let originals_rewrite = scan.assignments.iter().any(|assignment| {
        assignment.operator.as_str() == "="
            && assignment_member(assignment).is_some_and(|(object, name)| {
                symbol_of(model, object) == Some(originals) && name == "value"
            })
            && symbol_of(model, &assignment.right) == Some(value)
    });
    result_return && original_rebind && originals_rewrite
}

fn cached_guard(model: &Model<'_, '_>, scan: &Scan<'_>, cached: SymbolId) -> bool {
    scan.ifs
        .iter()
        .any(|branch| branch.alternate.is_none() && not_null_compared(model, &branch.test, cached))
}

fn bucket_null_test(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    registry: SymbolId,
    site: FunctionSite,
) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    if !matches!(binary.operator.as_str(), "==" | "===") {
        return false;
    }
    let is_bucket = |candidate: &Expression<'_>| {
        keyed_bucket(model, candidate, site).is_some_and(|(_, found)| found == registry)
    };
    (is_bucket(&binary.left) && is_null_literal(&binary.right))
        || (is_null_literal(&binary.left) && is_bucket(&binary.right))
}

pub(crate) fn increments(model: &Model<'_, '_>, statement: &Statement<'_>, cursor: SymbolId) -> bool {
    let Statement::ExpressionStatement(expression) = statement else {
        return false;
    };
    let Expression::UpdateExpression(update) = &expression.expression else {
        return false;
    };
    update.operator.as_str() == "++"
        && matches!(
            &update.argument,
            SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier)
                if crate::analysis::identifier_symbol(identifier, model.scoping) == Some(cursor)
        )
}

fn declares_zero(statement: &Statement<'_>, symbol: SymbolId) -> bool {
    let Statement::VariableDeclaration(declaration) = statement else {
        return false;
    };
    declaration.declarations.iter().any(|declarator| {
        plain_local_symbol(declarator) == Some(symbol)
            && declarator.init.as_ref().is_some_and(|init| {
                matches!(
                    init.without_parentheses(),
                    Expression::NumericLiteral(number) if number.value == 0.0
                )
            })
    })
}

/// The inline per-property registry scan:
///
/// ```text
/// if (registry[key] == null) {
///   cached = null;
/// } else {
///   let cursor = 0;
///   for (;;) {
///     if (cursor >= registry[key].length) { cached = null; break; }
///     cached = registry[key][cursor];
///     if (owner !== cached.object) { ++cursor; continue; }
///     break;
///   }
/// }
/// ```
fn registry_scan<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    site: FunctionSite,
    containing: SymbolId,
) -> Option<(SymbolId, SymbolId, SymbolId)> {
    let statements = loop_body(statement)?;
    let [bounds, select, match_branch, exit] = statements else {
        return None;
    };
    if !is_break(exit) {
        return None;
    }

    let Statement::IfStatement(bounds) = bounds else {
        return None;
    };
    if bounds.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(compare) = bounds.test.without_parentheses() else {
        return None;
    };
    if compare.operator.as_str() != ">=" {
        return None;
    }
    let cursor = symbol_of(model, &compare.left)?;
    let length_bucket = static_member(&compare.right, "length")?;
    let (_, registry) = keyed_bucket(model, length_bucket, site)?;
    let Some(bounds_body) = block(&bounds.consequent) else {
        return None;
    };
    let [clear, stop] = bounds_body else {
        return None;
    };
    let Some(clear) = plain_assignment(clear) else {
        return None;
    };
    if !is_null_literal(&clear.right) || !is_break(stop) {
        return None;
    }
    let cached = assignment_identifier(model, clear)?;

    let Some(select) = plain_assignment(select) else {
        return None;
    };
    if assignment_identifier(model, select) != Some(cached) {
        return None;
    }
    let Expression::ComputedMemberExpression(element) = select.right.without_parentheses() else {
        return None;
    };
    if element.optional || symbol_of(model, &element.expression) != Some(cursor) {
        return None;
    }
    let (_, element_registry) = keyed_bucket(model, &element.object, site)?;
    if element_registry != registry {
        return None;
    }

    let Statement::IfStatement(match_branch) = match_branch else {
        return None;
    };
    if match_branch.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(compare) = match_branch.test.without_parentheses() else {
        return None;
    };
    if !matches!(compare.operator.as_str(), "!==" | "!=") {
        return None;
    }
    let selected = |candidate: &Expression<'_>| {
        static_member(candidate, "object").and_then(|object| symbol_of(model, object))
            == Some(cached)
    };
    let owner = if selected(&compare.right) {
        &compare.left
    } else if selected(&compare.left) {
        &compare.right
    } else {
        return None;
    };
    if symbol_of(model, owner) != Some(containing) {
        return None;
    }
    let Some(matches_body) = block(&match_branch.consequent) else {
        return None;
    };
    let [increment, next] = matches_body else {
        return None;
    };
    if !is_continue(next) || !increments(model, increment, cursor) {
        return None;
    }
    Some((registry, cached, cursor))
}

fn registry_guard(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    site: FunctionSite,
    registry: SymbolId,
    containing: SymbolId,
    cached: SymbolId,
    cursor: SymbolId,
) -> bool {
    if !plain_local_in_body(model, site, cursor) {
        return false;
    }
    scan.ifs.iter().any(|branch| {
        let Some(alternate) = branch.alternate.as_ref() else {
            return false;
        };
        let Some(alternate_body) = block(alternate) else {
            return false;
        };
        // The alternate is exactly one bounded traversal loop reached through
        // a single zero-valued cursor local; nothing else may carry the scan.
        if alternate_body
            .iter()
            .filter(|entry| loop_body(*entry).is_some())
            .count()
            != 1
        {
            return false;
        }
        let Some(loop_index) = alternate_body
            .iter()
            .position(|entry| loop_body(entry).is_some())
        else {
            return false;
        };
        if !alternate_body[..loop_index]
            .iter()
            .any(|entry| declares_zero(entry, cursor))
        {
            return false;
        }
        if registry_scan(model, &alternate_body[loop_index], site, containing)
            != Some((registry, cached, cursor))
        {
            return false;
        }
        if !bucket_null_test(model, &branch.test, registry, site) {
            return false;
        }
        let Some(consequent) = block(&branch.consequent) else {
            return false;
        };
        let [assignment] = consequent else {
            return false;
        };
        let Some(assignment) = plain_assignment(assignment) else {
            return false;
        };
        assignment_identifier(model, assignment) == Some(cached)
            && is_null_literal(&assignment.right)
    })
}

fn inline_registry_cache(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    site: FunctionSite,
    containing: SymbolId,
    registry: SymbolId,
) -> Option<SymbolId> {
    let mut found = None;
    for statement in &scan.loops {
        let Some((candidate_registry, cached, cursor)) =
            registry_scan(model, *statement, site, containing)
        else {
            continue;
        };
        if candidate_registry != registry {
            continue;
        }
        if !registry_guard(model, scan, site, registry, containing, cached, cursor) {
            continue;
        }
        if found.replace(cached).is_some() {
            return None;
        }
    }
    found
}

fn containing_descriptor_record(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    cursor: SymbolId,
    desc: SymbolId,
) -> bool {
    let Some(members) = literal_object(expression) else {
        return false;
    };
    members.len() == 2
        && members
            .get("containingObj")
            .is_some_and(|value| symbol_of(model, value) == Some(cursor))
        && members
            .get("desc")
            .is_some_and(|value| symbol_of(model, value) == Some(desc))
}

/// The exact hit record, either written directly
/// (`record = { containingObj: cursor, desc }`) or through the same receiver
/// first (`cursor = { containingObj: cursor, desc }; record = cursor;`).
fn descriptor_hit(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    record: SymbolId,
    cursor: SymbolId,
    desc: SymbolId,
) -> bool {
    for (index, entry) in statements.iter().enumerate() {
        let Some(assignment) = plain_assignment(entry) else {
            continue;
        };
        let Some(target) = assignment_identifier(model, assignment) else {
            continue;
        };
        if target == record {
            if containing_descriptor_record(model, &assignment.right, cursor, desc)
                && statements.get(index + 1).is_some_and(|next| is_break(next))
            {
                return true;
            }
            continue;
        }
        if target != cursor || !containing_descriptor_record(model, &assignment.right, cursor, desc)
        {
            continue;
        }
        let Some(next) = statements.get(index + 1) else {
            continue;
        };
        let Some(next_assignment) = plain_assignment(next) else {
            continue;
        };
        if assignment_identifier(model, next_assignment) == Some(record)
            && symbol_of(model, &next_assignment.right) == Some(cursor)
            && statements.get(index + 2).is_some_and(|last| is_break(last))
        {
            return true;
        }
    }
    false
}

fn is_prototype_advance(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    cursor: SymbolId,
) -> bool {
    let Some(call) = call_of(expression) else {
        return false;
    };
    if expression_path(model, &call.callee) != ["Object", "getPrototypeOf"] {
        return false;
    }
    exact_arguments(call).is_some_and(|arguments| {
        arguments.len() == 1 && symbol_of(model, arguments[0]) == Some(cursor)
    })
}

/// `cursor` is a local of this exact installer, its one input value is formal
/// slot 0, and every other write is either the proven prototype advance of
/// that same receiver or (for the rewrite hit shape) the hit record itself.
fn descriptor_source(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    site: FunctionSite,
    cursor: SymbolId,
    desc: SymbolId,
) -> bool {
    if !plain_local_in_body(model, site, cursor) {
        return false;
    }
    let mut from_input = 0;
    let mut writes = 0;
    for (symbol, init) in definitions(model, scan) {
        if symbol != cursor {
            continue;
        }
        writes += 1;
        if model.site_parameter_slot(init, site) == Some(0) {
            from_input += 1;
            continue;
        }
        if is_prototype_advance(model, init, cursor)
            || containing_descriptor_record(model, init, cursor, desc)
        {
            continue;
        }
        return false;
    }
    from_input == 1 && writes >= 2
}

/// The inline descriptor walk:
///
/// ```text
/// let cursor = object;                       // exact formal slot 0
/// let record;
/// for (;;) {
///   record = void 0;
///   if (cursor == null) { record = null; break; }
///   let desc = Object.getOwnPropertyDescriptor(cursor, key);   // exact slot 1
///   if (desc == null) { cursor = Object.getPrototypeOf(cursor); continue; }
///   record = { containingObj: cursor, desc };
///   break;
/// }
/// ```
fn descriptor_walk<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    scan: &Scan<'a>,
    site: FunctionSite,
) -> Option<SymbolId> {
    let statements = loop_body(statement)?;
    let mut record = None;
    let mut cursor = None;
    for entry in statements {
        let Statement::IfStatement(branch) = entry else {
            continue;
        };
        if branch.alternate.is_some() {
            continue;
        }
        let Some(candidate_cursor) = null_compared_symbol(model, &branch.test) else {
            continue;
        };
        let Some(body) = block(&branch.consequent) else {
            continue;
        };
        let [assignment, exit] = body else {
            continue;
        };
        let Some(assignment) = plain_assignment(assignment) else {
            continue;
        };
        if !is_null_literal(&assignment.right) || !is_break(exit) {
            continue;
        }
        let Some(candidate_record) = assignment_identifier(model, assignment) else {
            continue;
        };
        match record {
            None => record = Some(candidate_record),
            Some(existing) if existing != candidate_record => return None,
            _ => {}
        }
        match cursor {
            None => cursor = Some(candidate_cursor),
            Some(existing) if existing != candidate_cursor => return None,
            _ => {}
        }
    }
    let record = record?;
    let cursor = cursor?;

    let mut desc = None;
    for entry in statements {
        let (symbol, init) = match entry {
            Statement::VariableDeclaration(declaration) => {
                let [declarator] = declaration.declarations.as_slice() else {
                    continue;
                };
                let Some(symbol) = plain_local_symbol(declarator) else {
                    continue;
                };
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                (symbol, init)
            }
            _ => {
                let Some(assignment) = plain_assignment(entry) else {
                    continue;
                };
                let Some(symbol) = assignment_identifier(model, assignment) else {
                    continue;
                };
                (symbol, &assignment.right)
            }
        };
        let Some(call) = call_of(init) else {
            continue;
        };
        if expression_path(model, &call.callee) != ["Object", "getOwnPropertyDescriptor"] {
            continue;
        }
        let Some(arguments) = exact_arguments(call) else {
            continue;
        };
        let [receiver, key] = arguments.as_slice() else {
            continue;
        };
        if symbol_of(model, *receiver) != Some(cursor)
            || model.site_parameter_slot(*key, site) != Some(1)
        {
            continue;
        }
        match desc {
            None => desc = Some(symbol),
            Some(existing) if existing != symbol => return None,
            _ => {}
        }
    }
    let desc = desc?;
    if !plain_local_in_body(model, site, record) || !plain_local_in_body(model, site, desc) {
        return None;
    }

    let mut advances = 0;
    for entry in statements {
        let Statement::IfStatement(branch) = entry else {
            continue;
        };
        if branch.alternate.is_some() || null_compared_symbol(model, &branch.test) != Some(desc) {
            continue;
        }
        let Some(body) = block(&branch.consequent) else {
            continue;
        };
        let [step, exit] = body else {
            continue;
        };
        let Some(step) = plain_assignment(step) else {
            continue;
        };
        if assignment_identifier(model, step) != Some(cursor)
            || !is_continue(exit)
            || !is_prototype_advance(model, &step.right, cursor)
        {
            continue;
        }
        advances += 1;
    }
    if advances != 1 || !descriptor_hit(model, statements, record, cursor, desc) {
        return None;
    }
    if !descriptor_source(model, scan, site, cursor, desc) {
        return None;
    }
    Some(record)
}

fn inline_descriptor_record<'a>(
    model: &Model<'_, '_>,
    scan: &Scan<'a>,
    site: FunctionSite,
) -> Option<SymbolId> {
    let mut found = None;
    for statement in &scan.loops {
        let Some(record) = descriptor_walk(model, *statement, scan, site) else {
            continue;
        };
        if found.replace(record).is_some() {
            return None;
        }
    }
    found
}

fn zero_argument_calls<'a>(
    model: &Model<'_, '_>,
    scan: &'a Scan<'a>,
) -> Vec<(SymbolId, &'a CallExpression<'a>)> {
    let mut calls = Vec::new();
    for (symbol, init) in &scan.defs {
        if let Expression::CallExpression(call) = init.without_parentheses() {
            if call.arguments.is_empty() {
                calls.push((*symbol, &**call));
            }
        }
    }
    for assignment in &scan.assignments {
        let Some(symbol) = assignment_identifier(model, assignment) else {
            continue;
        };
        if assignment.operator.as_str() != "=" {
            continue;
        }
        if let Expression::CallExpression(call) = assignment.right.without_parentheses() {
            if call.arguments.is_empty() {
                calls.push((symbol, call));
            }
        }
    }
    calls
}

fn wrapper_proof(
    model: &Model<'_, '_>,
    wrapper: FunctionSite,
    outer: FunctionSite,
    hooks: &[SymbolId],
) -> Option<()> {
    if model
        .enclosing_function(wrapper.node_id)
        .map(|site| site.span)
        != Some(outer.span)
    {
        return None;
    }
    let AstKind::Function(function) = model.nodes.kind(wrapper.node_id) else {
        return None;
    };
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    let strict = body
        .directives
        .iter()
        .any(|directive| directive.directive == "use strict");
    if !strict {
        return None;
    }
    let mut scan = Scan::default();
    scan_statements(&body.statements, &mut scan);
    let mut notified = BTreeSet::new();
    for call in &scan.calls {
        let Some(receiver) = static_member(&call.callee, "notify") else {
            continue;
        };
        if let Some(symbol) = symbol_of(model, receiver) {
            if hooks.contains(&symbol) {
                notified.insert(symbol);
            }
        }
    }
    if !hooks.iter().all(|hook| notified.contains(hook)) {
        return None;
    }
    if !scan.tries.iter().any(|try_| try_.finalizer.is_some()) {
        return None;
    }
    let bypass = scan
        .members
        .iter()
        .filter(|member| member.property.name == "bypassResult")
        .count();
    if bypass < 2 {
        return None;
    }
    scan.arguments.then_some(())
}

fn wrapper_invokes_original(model: &Model<'_, '_>, wrapper: FunctionSite, value: SymbolId) -> bool {
    let AstKind::Function(function) = model.nodes.kind(wrapper.node_id) else {
        return false;
    };
    let Some(body) = function.body.as_ref() else {
        return false;
    };
    let mut scan = Scan::default();
    scan_statements(&body.statements, &mut scan);
    scan.calls.iter().any(|call| {
        exact_arguments(call).is_some_and(|arguments| {
            arguments
                .iter()
                .any(|argument| symbol_of(model, argument) == Some(value))
        })
    })
}

fn copy_loop(
    model: &Model<'_, '_>,
    loop_statement: &Statement<'_>,
    names: SymbolId,
    installed: SymbolId,
    value: SymbolId,
) -> bool {
    let (test, body) = match loop_statement {
        Statement::ForStatement(loop_) => (loop_.test.as_ref(), &loop_.body),
        Statement::WhileStatement(loop_) => (Some(&loop_.test), &loop_.body),
        _ => return false,
    };
    let Some(test) = test else {
        return false;
    };
    let Expression::BinaryExpression(compare) = test.without_parentheses() else {
        return false;
    };
    if compare.operator.as_str() != "<" {
        return false;
    }
    let Some(cursor) = symbol_of(model, &compare.left) else {
        return false;
    };
    if !static_member(&compare.right, "length")
        .is_some_and(|object| symbol_of(model, object) == Some(names))
    {
        return false;
    }
    let Some(statements) = block(body) else {
        return false;
    };
    let Some(guard) = statements.iter().find_map(|statement| {
        let Statement::IfStatement(branch) = statement else {
            return None;
        };
        let Expression::BinaryExpression(compare) = branch.test.without_parentheses() else {
            return None;
        };
        if !matches!(compare.operator.as_str(), "==" | "===") {
            return None;
        }
        let Expression::CallExpression(index_of) = compare.left.without_parentheses() else {
            return None;
        };
        let arguments = call_through(model, index_of, &["Array", "prototype", "indexOf"])?;
        if arguments.len() != 2 {
            return None;
        }
        let (object, index) = indexed(arguments[1].without_parentheses())?;
        if symbol_of(model, object) != Some(names) || symbol_of(model, index) != Some(cursor) {
            return None;
        }
        Some(branch)
    }) else {
        return false;
    };
    let Some(inner) = block(&guard.consequent) else {
        return false;
    };
    let mut scan = Scan::default();
    scan_statements(inner, &mut scan);
    let indexed_cursor = |expression: &Expression<'_>| {
        indexed(expression.without_parentheses()).is_some_and(|(object, index)| {
            symbol_of(model, object) == Some(names) && symbol_of(model, index) == Some(cursor)
        })
    };
    let has = |path: &[&str], receiver: SymbolId| {
        scan.calls.iter().any(|call| {
            direct_call(model, call, path).is_some_and(|arguments| {
                arguments.len() >= 2
                    && symbol_of(model, arguments[0]) == Some(receiver)
                    && indexed_cursor(arguments[1])
            })
        })
    };
    has(&["Object", "getOwnPropertyDescriptor"], installed)
        && has(&["Object", "getOwnPropertyDescriptor"], value)
        && scan.calls.iter().any(|call| {
            direct_call(model, call, &["Object", "defineProperty"]).is_some_and(|arguments| {
                arguments.len() == 3
                    && symbol_of(model, arguments[0]) == Some(installed)
                    && indexed_cursor(arguments[1])
            })
        })
}

fn prototype_guard(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    installed: SymbolId,
    value: SymbolId,
) -> bool {
    scan.ifs.iter().any(|branch| {
        branch.alternate.is_none()
            && matches!(
                branch.test.without_parentheses(),
                Expression::UnaryExpression(unary)
                    if unary.operator.as_str() == "!"
                        && matches!(&unary.argument, Expression::CallExpression(call)
                            if has_own_property(model, call).is_some_and(|arguments| {
                                arguments.len() == 2
                                    && symbol_of(model, arguments[0]) == Some(value)
                                    && matches!(&arguments[1], Expression::StringLiteral(string)
                                        if string.value == "prototype")
                            }))
            )
            && block(&branch.consequent).is_some_and(|statements| {
                statements.iter().any(|statement| {
                    let Statement::ExpressionStatement(expression) = statement else {
                        return false;
                    };
                    let Expression::AssignmentExpression(assignment) = &expression.expression
                    else {
                        return false;
                    };
                    assignment.operator.as_str() == "="
                        && assignment_member(assignment).is_some_and(|(object, name)| {
                            symbol_of(model, object) == Some(installed) && name == "prototype"
                        })
                        && matches!(
                            assignment.right.without_parentheses(),
                            Expression::UnaryExpression(unary)
                                if unary.operator.as_str() == "void"
                                    && matches!(&unary.argument, Expression::NumericLiteral(number)
                                        if number.value == 0.0)
                        )
                })
            })
    })
}

fn captures_function_source(model: &Model<'_, '_>, scan: &Scan<'_>, value: SymbolId) -> bool {
    scan.calls.iter().any(|call| {
        let Expression::StaticMemberExpression(call_member) = &call.callee else {
            return false;
        };
        call_member.property.name == "call"
            && !call_member.optional
            && static_member(&call_member.object, "toString").is_some()
            && exact_arguments(call).is_some_and(|arguments| {
                arguments.len() == 1 && symbol_of(model, arguments[0]) == Some(value)
            })
    })
}

fn registration(model: &Model<'_, '_>, scan: &Scan<'_>, site: FunctionSite) -> Option<SymbolId> {
    let (bucket, registry) = scan.defs.iter().find_map(|(symbol, init)| {
        let Expression::ComputedMemberExpression(member) = init.without_parentheses() else {
            return None;
        };
        if member.optional || model.site_parameter_slot(&member.expression, site) != Some(1) {
            return None;
        }
        Some((*symbol, symbol_of(model, &member.object)?))
    })?;
    let null_branch = scan.ifs.iter().any(|branch| {
        is_null_compare(model, &branch.test, bucket)
            && block(&branch.consequent).is_some_and(|statements| {
                let mut inner = Scan::default();
                scan_statements(statements, &mut inner);
                let allocates = inner.assignments.iter().any(|assignment| {
                    assignment.operator.as_str() == "="
                        && assignment_identifier(model, assignment) == Some(bucket)
                        && matches!(
                            assignment.right.without_parentheses(),
                            Expression::ArrayExpression(_)
                        )
                });
                let stores = inner.assignments.iter().any(|assignment| {
                    assignment.operator.as_str() == "="
                        && matches!(&assignment.left, AssignmentTarget::ComputedMemberExpression(member)
                            if !member.optional
                                && symbol_of(model, &member.object) == Some(registry)
                                && model.site_parameter_slot(&member.expression, site) == Some(1))
                        && symbol_of(model, &assignment.right) == Some(bucket)
                });
                allocates && stores
            })
    });
    if !null_branch {
        return None;
    }
    scan.calls
        .iter()
        .any(|call| {
            call_through(model, call, &["Array", "prototype", "push"]).is_some_and(|arguments| {
                arguments.len() == 2 && symbol_of(model, arguments[0]) == Some(bucket)
            })
        })
        .then_some(registry)
}

fn result_record(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    before: SymbolId,
    after: SymbolId,
    originals: SymbolId,
) -> Option<SymbolId> {
    let mut found = None;
    for (symbol, init) in &scan.defs {
        if let Some(members) = literal_object(init) {
            if record_matches(model, &members, before, after, originals) {
                if found.replace(*symbol).is_some() {
                    return None;
                }
            }
        }
    }
    for assignment in &scan.assignments {
        let Some(symbol) = assignment_identifier(model, assignment) else {
            continue;
        };
        if assignment.operator.as_str() != "=" {
            continue;
        }
        if let Some(members) = literal_object(&assignment.right) {
            if record_matches(model, &members, before, after, originals) {
                if found.replace(symbol).is_some() {
                    return None;
                }
            }
        }
    }
    found
}

fn record_matches(
    model: &Model<'_, '_>,
    members: &BTreeMap<String, &Expression<'_>>,
    before: SymbolId,
    after: SymbolId,
    originals: SymbolId,
) -> bool {
    members.len() == 3
        && members
            .get("onBeforeInvoke")
            .is_some_and(|value| symbol_of(model, value) == Some(before))
        && members
            .get("onAfterInvoke")
            .is_some_and(|value| symbol_of(model, value) == Some(after))
        && members
            .get("originals")
            .is_some_and(|value| symbol_of(model, value) == Some(originals))
}

pub(crate) fn function_interceptor(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return None;
    }
    let [object_parameter, key_parameter, optional_parameter] = function.params.items.as_slice()
    else {
        return None;
    };
    if object_parameter.initializer.is_some()
        || key_parameter.initializer.is_some()
        || optional_parameter.initializer.is_some()
    {
        return None;
    }
    let object = plain_parameter(model, &object_parameter.pattern)?;
    let key = plain_parameter(model, &key_parameter.pattern)?;
    let optional = plain_parameter(model, &optional_parameter.pattern)?;
    if object == key || object == optional || key == optional {
        return None;
    }

    let mut scan = Scan::default();
    scan_statements(&body.statements, &mut scan);

    // (1) descriptor record source, then null short-circuit. The cached family
    // shape resolves it through a two-argument `lookup(object, key)` helper;
    // the inline shape walks the prototype chain itself. Both prove the same
    // record. Helper calls keep priority so every previously accepted body is
    // unchanged.
    let mut helper_records = Vec::new();
    for (symbol, init) in definitions(model, &scan) {
        let Expression::CallExpression(call) = init.without_parentheses() else {
            continue;
        };
        let Some(arguments) = exact_arguments(call) else {
            continue;
        };
        if arguments.len() == 2
            && model.site_parameter_slot(arguments[0], site) == Some(0)
            && model.site_parameter_slot(arguments[1], site) == Some(1)
        {
            helper_records.push(symbol);
        }
    }
    let record = match helper_records.as_slice() {
        [record] => *record,
        [] => inline_descriptor_record(model, &scan, site)?,
        _ => return None,
    };
    if !scan.ifs.iter().any(|branch| {
        branch.alternate.is_none()
            && is_null_compare(model, &branch.test, record)
            && is_return_null(&branch.consequent)
    }) {
        return None;
    }

    // (2) containing object, descriptor object and the three live fields.
    let containing = unique_field(model, &scan, record, "containingObj")?;
    let desc_rebound = scan.assignments.iter().any(|assignment| {
        assignment.operator.as_str() == "="
            && assignment_identifier(model, assignment) == Some(record)
            && member_on(model, &assignment.right, record, "desc")
    });
    if !desc_rebound {
        return None;
    }
    let configurable = unique_field(model, &scan, record, "configurable")?;
    let writable = unique_field(model, &scan, record, "writable")?;

    // (3) the own-"value" guard is a call-through of Object.hasOwnProperty.
    if !has_value_guard(model, &scan, record, "value") {
        return None;
    }

    // (4) null-prototype originals record, then originals.value = value.
    let Some(originals) = definitions(model, &scan)
        .into_iter()
        .find_map(|(symbol, init)| {
            let Expression::CallExpression(call) = init.without_parentheses() else {
                return None;
            };
            direct_call(model, call, &["Object", "create"])
                .is_some_and(|arguments| {
                    arguments.len() == 1 && matches!(&arguments[0], Expression::NullLiteral(_))
                })
                .then_some(symbol)
        })
    else {
        return None;
    };
    let mut value = None;
    for assignment in &scan.assignments {
        if assignment.operator.as_str() != "=" {
            continue;
        }
        let Some((receiver, name)) = assignment_member(assignment) else {
            continue;
        };
        if name != "value" || symbol_of(model, receiver) != Some(originals) {
            continue;
        }
        let Some(symbol) = symbol_of(model, &assignment.right) else {
            continue;
        };
        match value {
            None => value = Some(symbol),
            Some(existing) if existing != symbol => return None,
            _ => {}
        }
    }
    let value = value?;
    if value == record
        || value == configurable
        || value == writable
        || value == containing
        || value == originals
    {
        return None;
    }
    // The originals value must be the live `record.value` read; a different
    // receiver or property is not the same function value.
    if !definition(model, &scan, value).is_some_and(|init| member_on(model, init, record, "value"))
    {
        return None;
    }

    // (5) early non-wrapped return under the exact boolean/typeof predicate.
    let Some(early) = scan.ifs.iter().find(|branch| {
        branch
            .alternate
            .as_ref()
            .is_some_and(|alternate| block(alternate).is_some())
            && early_condition(model, &branch.test, configurable, writable, value)
            && returns_originals(model, &scan, &branch.consequent, originals)
    }) else {
        return None;
    };
    let Some(alternate) = early.alternate.as_ref() else {
        return None;
    };
    if block(alternate).is_none() {
        return None;
    }

    // (6) cached-original lookup and the exact result/original dataflow. The
    // accepted helper-call form is tried first; the inline per-property bucket
    // scan is the missing equivalent. Both must select the exact owner and key.
    let registry = registration(model, &scan, site)?;
    // The helper-call form is unchanged. The inline form additionally proves
    // that the selected record is guarded before its result/original fields are
    // consumed, so a disconnected scan cannot be read as the cache.
    let cached = if let Some(cached) = helper_cache(model, &scan, site, containing) {
        cached
    } else {
        let cached = inline_registry_cache(model, &scan, site, containing, registry)?;
        if !cached_guard(model, &scan, cached) {
            return None;
        }
        cached
    };
    if !cache_dataflow(model, &scan, cached, value, originals) {
        return None;
    }

    // (7) hook factories, owned wrapper, and the wrapper's own proof.
    let hook_candidates: BTreeMap<SymbolId, &CallExpression<'_>> =
        zero_argument_calls(model, &scan).into_iter().collect();
    let Some(wrapper) = scan.assignments.iter().find_map(|assignment| {
        if assignment.operator.as_str() != "=" {
            return None;
        }
        let (receiver, name) = assignment_member(assignment)?;
        if name != "value" || symbol_of(model, receiver) != Some(record) {
            return None;
        }
        let wrapper = model.function_from_expression(&assignment.right)?;
        (wrapper.span != site.span).then_some(wrapper)
    }) else {
        return None;
    };
    let Some(wrapper_symbol) = wrapper.binding_symbol else {
        return None;
    };
    if model.binding_writes(wrapper_symbol) != 0
        || !model
            .scoping
            .symbol_redeclarations(wrapper_symbol)
            .is_empty()
    {
        return None;
    }
    // Every hook is a zero-argument factory call that the owned wrapper
    // notifies. Requiring both to be distinct avoids accepting a single hook
    // used twice.
    let notified: BTreeSet<SymbolId> = {
        let AstKind::Function(wrapper_function) = model.nodes.kind(wrapper.node_id) else {
            return None;
        };
        let wrapper_body = wrapper_function.body.as_ref()?;
        let mut wrapper_scan = Scan::default();
        scan_statements(&wrapper_body.statements, &mut wrapper_scan);
        wrapper_scan
            .calls
            .iter()
            .filter_map(|call| static_member(&call.callee, "notify"))
            .filter_map(|receiver| symbol_of(model, receiver))
            .filter(|symbol| hook_candidates.contains_key(symbol))
            .collect()
    };
    if notified.len() != 2 {
        return None;
    }
    let hooks: Vec<SymbolId> = notified.into_iter().collect();
    wrapper_proof(model, wrapper, site, &hooks)?;
    if !wrapper_invokes_original(model, wrapper, value) {
        return None;
    }

    // (8) read back the installed wrapper; capture/push the original source.
    let Some(installed) = definitions(model, &scan)
        .into_iter()
        .find_map(|(symbol, init)| {
            (symbol != value && member_on(model, init, record, "value")).then_some(symbol)
        })
    else {
        return None;
    };
    if !captures_function_source(model, &scan, value) {
        return None;
    }

    // (9) copy own property descriptors, including the protected names.
    let Some(names) = definitions(model, &scan)
        .into_iter()
        .find_map(|(symbol, init)| {
            let Expression::CallExpression(call) = init.without_parentheses() else {
                return None;
            };
            direct_call(model, call, &["Object", "getOwnPropertyNames"])
                .is_some_and(|arguments| {
                    arguments.len() == 1 && symbol_of(model, arguments[0]) == Some(value)
                })
                .then_some(symbol)
        })
    else {
        return None;
    };
    if !scan
        .loops
        .iter()
        .any(|loop_| copy_loop(model, loop_, names, installed, value))
    {
        return None;
    }
    if !prototype_guard(model, &scan, installed, value) {
        return None;
    }

    // (10) re-install the descriptor and publish the hook record.
    let reinstalled = scan.calls.iter().any(|call| {
        direct_call(model, call, &["Object", "defineProperty"]).is_some_and(|arguments| {
            arguments.len() == 3
                && symbol_of(model, arguments[0]) == Some(containing)
                && model.site_parameter_slot(arguments[1], site) == Some(1)
                && symbol_of(model, arguments[2]) == Some(record)
        })
    });
    if !reinstalled {
        return None;
    }
    // The two hook identities are order-free: which zero-argument factory is
    // published as `onBeforeInvoke` versus `onAfterInvoke` follows the wrapper
    // dataflow, not the local declaration order that numbers the symbols. Both
    // proven pairings name the same two consumed hooks, so try either.
    let Some(result) = result_record(model, &scan, hooks[0], hooks[1], originals)
        .or_else(|| result_record(model, &scan, hooks[1], hooks[0], originals))
    else {
        return None;
    };
    if !scan
        .returns
        .iter()
        .any(|returned| symbol_of(model, returned) == Some(result))
    {
        return None;
    }
    let _ = (key, optional);
    Some(())
}

/// Whether `function` is the owned method-interceptor installer.
pub(crate) fn proves_method_interceptor(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> bool {
    function_interceptor(model, site, function, body).is_some()
}
