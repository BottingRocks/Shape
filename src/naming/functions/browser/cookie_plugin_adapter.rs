//! Bounded own-body proof for the custom cookie-collector registration pair.
//!
//! A custom collector regains two unnamed declarations in the anchor seed: the
//! zero-formal **stage-registrar plugin** that a collector's `ctor` returns, and
//! the one-formal **registration adapter** that wraps that plugin with the
//! captured ctor factory. Both are proven from their own bodies only; the inner
//! cookie callback keeps the existing `emitMatchingCookieValue` name and the
//! document capture keeps `captureGlobalDocument`.
//!
//! Plugin `createCookieCollectorPlugin` (zero formals):
//!
//! ```text
//! function plugin() {
//!   function immediate(slot) {
//!     let sink = slot;
//!     document.cookie.split("; ").forEach(entry => {
//!       let parts = entry.split("=");
//!       if (parts.length > 0 && parts[0] === "<key>") {
//!         sink.next({ value: parts[1] }); sink.complete();
//!       }
//!     });
//!     sink.next({}); sink.complete();
//!   }
//!   let api = { setup: void 0, immediately: void 0 };
//!   let document;
//!   function setup(slot) { document = slot.global.document; }
//!   api.setup = setup;
//!   api.immediately = immediate;
//!   return api;
//! }
//! ```
//!
//! The plugin owns exactly two nested `FunctionDeclaration` children, returns a
//! local whose literal has only `setup`/`immediately`/`afterReady`/`lastChance`/
//! `teardown` keys with `void 0` placeholders, assigns exactly those keys to the
//! two children, and its `immediately` child independently satisfies the
//! existing cookie-emission proof (`super::super::events::cookie_emission::cookie_emission`) while its
//! `setup` child independently satisfies the shared document-capture proof
//! (`super::document_capture`). An unreferenced nested declaration, a non
//! cookie-immediate, a non document setup or a flag-valued stage object is
//! refused.
//!
//! Adapter `createCookieCollectorCtorAdapter` (one formal):
//!
//! ```text
//! function adapter(plugin) {
//!   let record = plugin;
//!   return ctorFactory((signal, parent, init, lastChance, teardown) => {
//!     let ready = !!record.needsDOMReady;
//!     let handle, chance, teardownFn, initFn, error;
//!     try {
//!       if (record.setup) { record.setup(signal); }
//!       handle = parent; chance = lastChance; teardownFn = teardown; initFn = init;
//!     } catch (e) {
//!       error = e; handle = parent; chance = lastChance; teardownFn = teardown; initFn = init;
//!     }
//!     if (record.immediately) {
//!       handle(cb => { if (error != null) { cb.error(error); } else { record.immediately(cb); } });
//!     }
//!     ...
//!   });
//! }
//! ```
//!
//! The adapter returns exactly the captured factory applied to a five-formal
//! stage-registrar arrow that reads the plugin record's `needsDOMReady` flag,
//! runs `record.setup(signal)` under a swallowing `try`/`catch`, and routes the
//! `immediately` stage behind the `error != null` short-circuit. The factory
//! callee must be a captured (outer-declared) binding, never a fresh `new` and
//! never a local: the shape, not the absence of `new`, is the proof.
//!
//! Recognition reads structure and scope-resolved symbol identity only. Generated
//! spellings, offsets, vendors, hashes, literal cookie names and inherited labels
//! never participate.

use super::*;
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

const PLUGIN_NAME: &str = "createCookieCollectorPlugin";
const ADAPTER_NAME: &str = "createCookieCollectorCtorAdapter";
const EVALUATOR: &str = "cookie_plugin_adapter";

const PLUGIN_DETAIL: &str = "owned zero-formal stage-registrar plugin: returns a local whose literal carries only the setup/immediately/afterReady/lastChance/teardown keys with void-0 placeholders, assigns exactly those keys to its two-and-only direct nested FunctionDeclaration children, where the setup child independently proves the shared document capture and the immediately child independently proves the existing cookie read/filter/emitter callback";
const ADAPTER_DETAIL: &str = "owned one-formal ctor adapter: returns the captured (outer-declared) ctor factory applied to a five-formal stage-registrar arrow that reads the record's needsDOMReady flag, runs record.setup(signal) under a swallowing try/catch, and routes the immediately stage behind the caught-error short-circuit; the factory callee is a captured binding, not a new-expression and not a local";

/// The exact stage keys a collector plugin record may carry.
const STAGE_KEYS: [&str; 5] = [
    "setup",
    "immediately",
    "afterReady",
    "lastChance",
    "teardown",
];

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn is_symbol(model: &Model<'_, '_>, expression: &Expression<'_>, expected: Symbol) -> bool {
    symbol(model, expression) == Some(expected)
}

fn is_void_zero(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::UnaryExpression(unary)
        if unary.operator.as_str() == "void"
            && matches!(unary.argument.without_parentheses(),
                Expression::NumericLiteral(number) if number.value == 0.0))
}

fn plain_call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expression.without_parentheses() {
        Expression::CallExpression(call) if !call.optional => Some(call),
        _ => None,
    }
}

/// `object.<name>` for a non-optional static member read.
fn member_object<'a>(expression: &'a Expression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn is_member_of(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    owner: Symbol,
    name: &str,
) -> bool {
    member_object(expression, name).and_then(|object| symbol(model, object)) == Some(owner)
}

/// The property of a non-optional static member read on `owner`.
fn member_name_of(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    owner: Symbol,
) -> Option<String> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    if member.optional || symbol(model, &member.object)? != owner {
        return None;
    }
    Some(member.property.name.to_string())
}

/// A captured plain slot: declared strictly before the proven function, never a
/// formal and never a redeclaration.
fn captured(model: &Model<'_, '_>, site: FunctionSite, value: Symbol) -> bool {
    let span = model.scoping.symbol_span(value);
    !site.span.contains_inclusive(span)
        && span.end <= site.span.start
        && model.formal_symbol_slot(value).is_none()
        && model.scoping.symbol_redeclarations(value).is_empty()
}

fn resolve_function(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<FunctionSite> {
    super::stable_function(model, expression, 0)
}

/// The single plain immutable formal of a one-formal function site, as the
/// existing cookie-emission and document-capture proofs consume it.
fn immutable_formal(model: &Model<'_, '_>, site: FunctionSite) -> Option<Symbol> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if function.r#async || function.generator || function.params.rest.is_some() {
        return None;
    }
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
        return None;
    };
    let value = identifier.symbol_id.get()?;
    (model.binding_writes(value) == 0 && model.scoping.symbol_redeclarations(value).is_empty())
        .then_some(value)
}

// ===========================================================================
// The plugin stage-registrar
// ===========================================================================

fn plugin_stage_registrar(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
    {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }

    let mut returned: Option<Symbol> = None;
    // (declared symbol, member names, every member a `void 0` placeholder)
    let mut objects: Vec<(Symbol, Vec<String>, bool)> = Vec::new();
    let mut children: Vec<FunctionSite> = Vec::new();
    let mut assigned: BTreeMap<String, (Symbol, FunctionSite)> = BTreeMap::new();

    for statement in &body.statements {
        match statement {
            Statement::FunctionDeclaration(declared) => {
                children.push(model.site_by_span(declared.span)?);
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                        return None;
                    };
                    let value = identifier.symbol_id.get()?;
                    match declarator.init.as_ref() {
                        None => {}
                        Some(init) => {
                            let Expression::ObjectExpression(literal) = init.without_parentheses()
                            else {
                                return None;
                            };
                            let mut names = Vec::new();
                            let mut all_void = true;
                            for property in &literal.properties {
                                let ObjectPropertyKind::ObjectProperty(property) = property else {
                                    return None;
                                };
                                if property.computed
                                    || property.method
                                    || property.kind != PropertyKind::Init
                                {
                                    return None;
                                }
                                names.push(property_key_name(&property.key)?);
                                if !is_void_zero(&property.value) {
                                    all_void = false;
                                }
                            }
                            objects.push((value, names, all_void));
                        }
                    }
                }
            }
            Statement::ExpressionStatement(expression) => {
                let Expression::AssignmentExpression(assignment) = &expression.expression else {
                    return None;
                };
                if assignment.operator.as_str() != "=" {
                    return None;
                }
                let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
                    return None;
                };
                if member.optional {
                    return None;
                }
                let key = member.property.name.to_string();
                if !STAGE_KEYS.contains(&key.as_str()) {
                    return None;
                }
                let owner = symbol(model, &member.object)?;
                let child = resolve_function(model, &assignment.right)?;
                if assigned.insert(key, (owner, child)).is_some() {
                    return None;
                }
            }
            Statement::ReturnStatement(ret) => {
                let value = symbol(model, ret.argument.as_ref()?)?;
                if returned.replace(value).is_some() {
                    return None;
                }
            }
            _ => return None,
        }
    }

    let returned = returned?;
    let mut literal = None;
    for (value, names, all_void) in objects {
        if value == returned {
            literal = Some((names, all_void));
        }
    }
    let (names, all_void) = literal?;

    // The returned literal carries only stage keys, each a `void 0` placeholder.
    let keys: BTreeSet<String> = names.iter().cloned().collect();
    if keys.len() != names.len() || !all_void {
        return None;
    }
    if names
        .iter()
        .any(|name| !STAGE_KEYS.contains(&name.as_str()))
    {
        return None;
    }
    if !keys.contains("setup") || !keys.contains("immediately") {
        return None;
    }
    if keys != assigned.keys().cloned().collect::<BTreeSet<_>>() {
        return None;
    }
    for (owner, _) in assigned.values() {
        if *owner != returned {
            return None;
        }
    }

    // Every declared child is referenced exactly once by an assignment, and no
    // assignment names anything outside the direct child set: an unused nested
    // donor is refused.
    let child_sites: BTreeSet<(u32, u32)> = children
        .iter()
        .map(|child| (child.span.start, child.span.end))
        .collect();
    let assigned_sites: BTreeSet<(u32, u32)> = assigned
        .values()
        .map(|(_, child)| (child.span.start, child.span.end))
        .collect();
    if child_sites.len() != children.len()
        || assigned_sites.len() != assigned.len()
        || child_sites != assigned_sites
    {
        return None;
    }

    let setup = assigned.get("setup").map(|(_, child)| *child)?;
    let immediately = assigned.get("immediately").map(|(_, child)| *child)?;
    if setup.span.start == immediately.span.start {
        return None;
    }

    // The setup child is the shared document capture.
    let AstKind::Function(setup_function) = model.nodes.kind(setup.node_id) else {
        return None;
    };
    if super::document_capture(model, setup, setup_function.body.as_ref()?).is_none() {
        return None;
    }
    // The immediately child is the existing cookie read/filter/emitter.
    let AstKind::Function(immediate_function) = model.nodes.kind(immediately.node_id) else {
        return None;
    };
    let immediate_body = immediate_function.body.as_ref()?;
    let input = immutable_formal(model, immediately)?;
    if super::super::events::cookie_emission::cookie_emission(
        model,
        immediately,
        input,
        immediate_body,
    )
    .is_none()
    {
        return None;
    }
    Some(())
}

// ===========================================================================
// The ctor registration adapter
// ===========================================================================

/// The caught-error short-circuit: `cb => { if (error != null) { ... } else {
/// record.<stage>(cb); ... } }`.
fn error_gate(
    model: &Model<'_, '_>,
    call: &CallExpression<'_>,
    record: Symbol,
    stage: &str,
    error: Symbol,
) -> Option<()> {
    let arguments = call_arguments(call);
    let [callback] = arguments.as_slice() else {
        return None;
    };
    let Expression::ArrowFunctionExpression(callback) = callback.without_parentheses() else {
        return None;
    };
    if callback.r#async || callback.params.rest.is_some() || callback.expression {
        return None;
    }
    let [parameter] = callback.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(callback_param) = &parameter.pattern else {
        return None;
    };
    let callback_param = callback_param.symbol_id.get()?;
    let [Statement::IfStatement(branch)] = callback.body.statements.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if test.operator.as_str() != "!="
        || !matches!(test.right.without_parentheses(), Expression::NullLiteral(_))
        || !is_symbol(model, &test.left, error)
    {
        return None;
    }
    let Statement::BlockStatement(alternate) = branch.alternate.as_ref()? else {
        return None;
    };
    let [Statement::ExpressionStatement(invoke), ..] = alternate.body.as_slice() else {
        return None;
    };
    let invoke = plain_call(&invoke.expression)?;
    if !is_member_of(model, &invoke.callee, record, stage) {
        return None;
    }
    let invoke_arguments = call_arguments(invoke);
    if invoke_arguments.len() != 1 || !is_symbol(model, invoke_arguments[0], callback_param) {
        return None;
    }
    Some(())
}

/// The five-formal stage-registrar arrow: reads the record's `needsDOMReady`
/// flag, runs `record.setup(signal)` under a swallowing try/catch, and routes
/// the record's `immediately` stage behind the caught-error short-circuit.
fn stage_registrar(model: &Model<'_, '_>, site: FunctionSite, record: Symbol) -> Option<()> {
    let AstKind::ArrowFunctionExpression(arrow) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if arrow.r#async || arrow.expression || arrow.params.rest.is_some() {
        return None;
    }
    if arrow.params.items.len() != 5 {
        return None;
    }
    for parameter in &arrow.params.items {
        if parameter.initializer.is_some()
            || !matches!(&parameter.pattern, BindingPattern::BindingIdentifier(_))
        {
            return None;
        }
    }
    if !arrow.body.directives.is_empty() {
        return None;
    }

    let mut flag = false;
    let mut setup_seen = false;
    let mut error: Option<Symbol> = None;
    let mut tries = 0usize;

    for statement in &arrow.body.statements {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    let Some(init) = declarator.init.as_ref() else {
                        continue;
                    };
                    // `let ready = !!record.needsDOMReady;`
                    if let Expression::UnaryExpression(outer) = init.without_parentheses() {
                        if outer.operator.as_str() == "!" {
                            if let Expression::UnaryExpression(inner) =
                                outer.argument.without_parentheses()
                            {
                                if inner.operator.as_str() == "!"
                                    && is_member_of(model, &inner.argument, record, "needsDOMReady")
                                {
                                    flag = true;
                                }
                            }
                        }
                    }
                }
            }
            Statement::TryStatement(attempt) => {
                tries += 1;
                if attempt.finalizer.is_some() {
                    return None;
                }
                let handler = attempt.handler.as_ref()?;
                let [Statement::IfStatement(guard), rest @ ..] = attempt.block.body.as_slice()
                else {
                    return None;
                };
                if !is_member_of(model, &guard.test, record, "setup") {
                    return None;
                }
                let Statement::BlockStatement(guard_body) = &guard.consequent else {
                    return None;
                };
                let [Statement::ExpressionStatement(guard_call)] = guard_body.body.as_slice()
                else {
                    return None;
                };
                let setup_call = plain_call(&guard_call.expression)?;
                if !is_member_of(model, &setup_call.callee, record, "setup") {
                    return None;
                }
                let setup_arguments = call_arguments(setup_call);
                if setup_arguments.len() != 1
                    || model.site_parameter_slot(setup_arguments[0], site) != Some(0)
                {
                    return None;
                }
                setup_seen = true;
                for statement in rest {
                    let Statement::ExpressionStatement(expression) = statement else {
                        return None;
                    };
                    if !matches!(
                        &expression.expression,
                        Expression::AssignmentExpression(assignment)
                            if assignment.operator.as_str() == "="
                    ) {
                        return None;
                    }
                }
                let parameter = handler.param.as_ref()?;
                let BindingPattern::BindingIdentifier(thrown) = &parameter.pattern else {
                    return None;
                };
                let thrown = thrown.symbol_id.get()?;
                let [Statement::ExpressionStatement(caught), ..] = handler.body.body.as_slice()
                else {
                    return None;
                };
                let Expression::AssignmentExpression(assignment) = &caught.expression else {
                    return None;
                };
                if assignment.operator.as_str() != "=" {
                    return None;
                }
                let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                    return None;
                };
                error = Some(crate::analysis::identifier_symbol(target, model.scoping)?);
                if !is_symbol(model, &assignment.right, thrown) {
                    return None;
                }
            }
            Statement::IfStatement(_) => {}
            _ => return None,
        }
    }

    let error = error?;
    if !flag || !setup_seen || tries != 1 {
        return None;
    }

    let mut stages = BTreeSet::new();
    for statement in &arrow.body.statements {
        let Statement::IfStatement(branch) = statement else {
            continue;
        };
        let stage = member_name_of(model, &branch.test, record)?;
        if !STAGE_KEYS.contains(&stage.as_str()) {
            return None;
        }
        let Statement::BlockStatement(block) = &branch.consequent else {
            return None;
        };
        let [Statement::ExpressionStatement(call_statement)] = block.body.as_slice() else {
            return None;
        };
        let call = plain_call(&call_statement.expression)?;
        symbol(model, &call.callee)?;
        if stage == "immediately" && error_gate(model, call, record, "immediately", error).is_none()
        {
            return None;
        }
        stages.insert(stage);
    }
    stages.contains("immediately").then_some(())
}

fn adapter_registration(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if function.r#async || function.generator || function.params.rest.is_some() {
        return None;
    }
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
        return None;
    };
    let formal = identifier.symbol_id.get()?;
    if model.binding_writes(formal) != 0 || !model.scoping.symbol_redeclarations(formal).is_empty()
    {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }

    // Body: an optional `let record = formal;` then `return factory(arrow);`.
    let (last, prefix) = body.statements.split_last()?;
    let Statement::ReturnStatement(ret) = last else {
        return None;
    };
    let mut record = formal;
    match prefix {
        [] => {}
        [Statement::VariableDeclaration(declaration)] => {
            let [declarator] = declaration.declarations.as_slice() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                return None;
            };
            let copy = identifier.symbol_id.get()?;
            if model.binding_writes(copy) != 0
                || !model.scoping.symbol_redeclarations(copy).is_empty()
                || !is_symbol(model, declarator.init.as_ref()?, formal)
            {
                return None;
            }
            record = copy;
        }
        _ => return None,
    }

    let call = plain_call(ret.argument.as_ref()?)?;
    let factory = symbol(model, &call.callee)?;
    if !captured(model, site, factory) {
        return None;
    }
    let arguments = call_arguments(call);
    if call.arguments.len() != 1 || arguments.len() != 1 {
        return None;
    }
    let Expression::ArrowFunctionExpression(arrow) = arguments[0].without_parentheses() else {
        return None;
    };
    let arrow_site = model.site_by_span(arrow.span)?;
    stage_registrar(model, arrow_site, record)?;
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if plugin_stage_registrar(model, site).is_some() {
        return vec![proposal(PLUGIN_NAME, EVALUATOR, PLUGIN_DETAIL, f)];
    }
    if adapter_registration(model, site).is_some() {
        return vec![proposal(ADAPTER_NAME, EVALUATOR, ADAPTER_DETAIL, f)];
    }
    Vec::new()
}
