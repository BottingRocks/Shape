//! Whole-body proof for the signal serializer factory.
//!
//! The owned witness (`uhc-uhc-final01`, `$THREAD_1_43446`) is the factory that
//! builds the binary value-encoder configuration, registers the two serializer
//! extension points on it, declares the snapshot serializer and returns it:
//!
//! ```text
//! function factory(keysetsToOmit, reserved, seedKey) {
//!     let config;                                  // or `let config = encodeConfig;`
//!     let seed = seedKey;
//!     let encodeConfig = { keysetsToOmit: keysetsToOmit,
//!                          depthBound: 100, extensions: void 0 };
//!     let extensions = { "0": capturedSentinelExtension };
//!     let depthExtension = capturedDepthExtension;
//!     if ("<numeric extension id>" in extensions) {
//!         depthExtension = { value: depthExtension, enumerable: true,
//!                            configurable: true, writable: true };
//!         Object.defineProperty(extensions, "<numeric extension id>", depthExtension);
//!     } else {
//!         extensions[<numeric extension id>] = depthExtension;
//!     }
//!     encodeConfig.extensions = extensions;
//!     config = encodeConfig;
//!     function serialiseSnapshot(snapshot) { ... }
//!     return serialiseSnapshot;
//! }
//! ```
//!
//! Identity comes from the complete outer dataflow: one plain immutable
//! `keysetsToOmit` formal forwarded into a three-field encode configuration
//! whose `depthBound` is the fixed `100` and whose `extensions` slot starts
//! unset; a one-entry extension map whose `"0"` entry is a captured extension
//! object; a captured depth extension point that is converted into a data
//! descriptor and registered under one consistent numeric extension id with
//! `Object.defineProperty`, with the matching computed store in the alternate;
//! the map stored into the configuration's `extensions`; the configuration kept
//! in a distinct local that the returned serializer closes over; and the third
//! formal kept in a distinct immutable seed alias that the returned serializer
//! also closes over.
//!
//! The returned nested function is proved to be the signal snapshot serializer
//! by its own owned body only: its single formal must be read through the
//! `errors`, `positionalSignals`, `keyedSignals` and `signals` members at that
//! function's own call-frame level. Nested callbacks are not descended into, so
//! a decoy that moves the field reads into a child closure cannot stand in for
//! the serializer.
//!
//! The second formal is reserved and never referenced anywhere in this family,
//! so a non-empty slot refuses. Statement order is free among the independent
//! declarations, and both `let config = encodeConfig` and the declare-then-
//! assign form are accepted. Numeric-encoder (varint loops), payload finalizer
//! and snapshot-to-bytes roles are deliberately *not* claimed here: no loop
//! form, byte append, nonce, base64 step or nested-callback body participates.

use super::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

/// One scalar extension-point identifier used by the serializer configuration.
const EXTENSION_ID_MAX: f64 = 4_294_967_295.0;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}

/// Plain, unreassigned formals. Default, rest and destructuring formals refuse.
fn plain_params(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> Option<Vec<Symbol>> {
    if params.rest.is_some() {
        return None;
    }
    let mut symbols = Vec::new();
    for parameter in &params.items {
        if parameter.initializer.is_some() {
            return None;
        }
        let BindingPattern::BindingIdentifier(id) = &parameter.pattern else {
            return None;
        };
        let s = id.symbol_id.get()?;
        if model.binding_writes(s) != 0 || !model.scoping.symbol_redeclarations(s).is_empty() {
            return None;
        }
        symbols.push(s);
    }
    Some(symbols)
}

/// A plain local variable binding declared inside the factory body.
fn local_binding(
    model: &Model<'_, '_>,
    site: FunctionSite,
    pattern: &BindingPattern<'_>,
) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = pattern else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (site.span.contains_inclusive(model.scoping.symbol_span(s))
        && model.scoping.symbol_redeclarations(s).is_empty()
        && matches!(
            model.nodes.parent_kind(model.scoping.symbol_declaration(s)),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        ))
    .then_some(s)
}

/// True when the symbol is a consumer-provided variable declared before the
/// factory. Parameters, catches, loop targets, function declarations and
/// redeclarations never qualify.
fn is_captured_source(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> bool {
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || model.scoping.symbol_span(s).end >= site.span.start
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return false;
    };
    matches!(&variable.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        && matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        && !matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
}

fn single_variable<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    if !matches!(
        d.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}

fn assignment<'a>(s: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    let Expression::AssignmentExpression(a) = &s.expression else {
        return None;
    };
    (a.operator.as_str() == "=").then_some(a)
}

fn statement_call<'a>(s: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    let Expression::CallExpression(c) = s.expression.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

fn exact_arguments<'a>(c: &'a CallExpression<'a>, count: usize) -> Option<Vec<&'a Expression<'a>>> {
    if c.arguments.len() != count {
        return None;
    }
    let values = call_arguments(c);
    (values.len() == count).then_some(values)
}

fn number(e: &Expression<'_>, value: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(n) if n.value == value)
}

fn void_zero(e: &Expression<'_>) -> bool {
    matches!(
        e.without_parentheses(),
        Expression::UnaryExpression(u)
            if u.operator.as_str() == "void"
                && matches!(u.argument.without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.0)
    )
}

fn boolean(e: &Expression<'_>, value: bool) -> bool {
    matches!(e.without_parentheses(), Expression::BooleanLiteral(b) if b.value == value)
}

/// A literal object with unique, non-computed, non-method `init` members.
fn object_members<'a>(
    object: &'a ObjectExpression<'a>,
) -> Option<BTreeMap<String, &'a Expression<'a>>> {
    let mut members = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        if p.computed || p.method || p.kind != PropertyKind::Init {
            return None;
        }
        let key = property_key_name(&p.key)?;
        if members.insert(key, &p.value).is_some() {
            return None;
        }
    }
    Some(members)
}

/// `{ keysetsToOmit: slot0, depthBound: 100, extensions: void 0 }`.
fn encode_configuration(
    model: &Model<'_, '_>,
    object: &ObjectExpression<'_>,
    keysets_to_omit: Symbol,
) -> Option<()> {
    let members = object_members(object)?;
    if members.len() != 3 {
        return None;
    }
    if symbol(model, members.get("keysetsToOmit")?)? != keysets_to_omit {
        return None;
    }
    if !number(members.get("depthBound")?, 100.0) {
        return None;
    }
    if !void_zero(members.get("extensions")?) {
        return None;
    }
    Some(())
}

/// `{ "0": capturedExtension }` with a distinct captured extension object.
fn extension_map(
    model: &Model<'_, '_>,
    site: FunctionSite,
    object: &ObjectExpression<'_>,
) -> Option<Symbol> {
    let members = object_members(object)?;
    if members.len() != 1 {
        return None;
    }
    let sentinel = symbol(model, members.get("0")?)?;
    is_captured_source(model, site, sentinel).then_some(sentinel)
}

/// The one extension point that is converted into a data descriptor and stored
/// under one consistent integer id on the extension map.
fn extension_registration(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    extension_map: Symbol,
    depth_extension: Symbol,
) -> Option<()> {
    let Statement::IfStatement(branch) = statement else {
        return None;
    };
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if test.operator.as_str() != "in" {
        return None;
    }
    let Expression::StringLiteral(id) = test.left.without_parentheses() else {
        return None;
    };
    let id = id.value.as_str();
    let extension_id: f64 = id.parse::<u32>().ok()? as f64;
    if !(extension_id <= EXTENSION_ID_MAX) || symbol(model, &test.right) != Some(extension_map) {
        return None;
    }
    let [descriptor_statement, define_statement] = block(&branch.consequent)? else {
        return None;
    };
    let descriptor = assignment(descriptor_statement)?;
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &descriptor.left else {
        return None;
    };
    if crate::analysis::identifier_symbol(target, model.scoping) != Some(depth_extension) {
        return None;
    }
    let Expression::ObjectExpression(descriptor_object) = descriptor.right.without_parentheses()
    else {
        return None;
    };
    let members = object_members(descriptor_object)?;
    if members.len() != 4
        || symbol(model, members.get("value")?)? != depth_extension
        || !boolean(members.get("enumerable")?, true)
        || !boolean(members.get("configurable")?, true)
        || !boolean(members.get("writable")?, true)
    {
        return None;
    }
    let define = statement_call(define_statement)?;
    if expression_path(model, &define.callee) != ["Object", "defineProperty"] {
        return None;
    }
    let arguments = exact_arguments(define, 3)?;
    if symbol(model, arguments[0]) != Some(extension_map)
        || !matches!(arguments[1].without_parentheses(), Expression::StringLiteral(value) if value.value == id)
        || symbol(model, arguments[2]) != Some(depth_extension)
    {
        return None;
    }
    let [store_statement] = block(branch.alternate.as_ref()?)? else {
        return None;
    };
    let store = assignment(store_statement)?;
    let AssignmentTarget::ComputedMemberExpression(target) = &store.left else {
        return None;
    };
    if target.optional
        || symbol(model, &target.object) != Some(extension_map)
        || !matches!(target.expression.without_parentheses(), Expression::NumericLiteral(n) if n.value == extension_id)
        || symbol(model, &store.right) != Some(depth_extension)
    {
        return None;
    }
    Some(())
}

/// The signal fields a serializer call frame reads directly from its snapshot
/// formal. Nested functions are deliberately not descended into: a decoy that
/// only reads them from a child callback cannot prove this call frame.
struct SignalReads<'m, 'a> {
    model: &'m Model<'m, 'a>,
    snapshot: Symbol,
    fields: BTreeSet<String>,
}

impl<'a> Visit<'a> for SignalReads<'_, 'a> {
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if matches!(
            member.property.name.as_str(),
            "errors" | "positionalSignals" | "keyedSignals" | "signals"
        ) && crate::analysis::expression_symbol(&member.object, self.model.scoping)
            == Some(self.snapshot)
        {
            self.fields.insert(member.property.name.to_string());
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_function(&mut self, _function: &Function<'a>, _flags: oxc_semantic::ScopeFlags) {}

    fn visit_arrow_function_expression(&mut self, _arrow: &ArrowFunctionExpression<'a>) {}
}

/// The nested serializer: one plain formal, no directives, at least the four
/// signal snapshot members read from that formal in its own call frame.
fn snapshot_serializer(model: &Model<'_, '_>, function: &Function<'_>) -> Option<Symbol> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let params = plain_params(model, &function.params)?;
    let [snapshot] = params.as_slice() else {
        return None;
    };
    let mut reads = SignalReads {
        model,
        snapshot: *snapshot,
        fields: BTreeSet::new(),
    };
    reads.visit_function_body(body);
    ["errors", "positionalSignals", "keyedSignals", "signals"]
        .iter()
        .all(|field| reads.fields.contains(*field))
        .then_some(*snapshot)
}

fn referenced_within(model: &Model<'_, '_>, symbol: Symbol, site: FunctionSite) -> bool {
    model
        .scoping
        .get_resolved_references(symbol)
        .any(|reference| model.node_within(reference.node_id(), site))
}

/// Bounded whole-body proof of the serializer factory and its extension
/// configuration.
fn create_signal_serializer(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
) -> Option<()> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let params = plain_params(model, &function.params)?;
    let [keysets_to_omit, reserved, seed_key] = params.as_slice() else {
        return None;
    };
    // This family's signature carries an unused reserved slot; a referenced
    // slot is a different factory shape and refuses.
    if model.binding_references(*reserved) != 0 {
        return None;
    }

    let mut config_object = None;
    let mut extension = None;
    let mut depth = None;
    let mut config_placeholder = None;
    let mut config_declarator = None;
    let mut seed_alias = None;
    let mut inner = None;
    let mut registration = None;
    let mut extensions_store = None;
    let mut config_store = None;
    let mut returned = None;

    for (index, statement) in body.statements.iter().enumerate() {
        match statement {
            Statement::FunctionDeclaration(declared) => {
                if inner.is_some() {
                    return None;
                }
                inner = Some((declared.id.as_ref()?.symbol_id.get()?, index));
            }
            Statement::ReturnStatement(_) => {
                if index + 1 != body.statements.len() || returned.is_some() {
                    return None;
                }
                let Statement::ReturnStatement(end) = statement else {
                    return None;
                };
                returned = Some((index, symbol(model, end.argument.as_ref()?)?));
            }
            Statement::VariableDeclaration(_) => {
                let variable = single_variable(statement)?;
                let binding = local_binding(model, site, &variable.id)?;
                let Some(init) = variable.init.as_ref() else {
                    if config_placeholder.replace(binding).is_some() {
                        return None;
                    }
                    continue;
                };
                match init.without_parentheses() {
                    Expression::ObjectExpression(object) => {
                        if encode_configuration(model, object, *keysets_to_omit).is_some() {
                            if config_object.replace(binding).is_some() {
                                return None;
                            }
                        } else if let Some(sentinel) = extension_map(model, site, object) {
                            if extension.replace((binding, sentinel)).is_some() {
                                return None;
                            }
                        } else {
                            return None;
                        }
                    }
                    Expression::Identifier(_) => {
                        let source = symbol(model, init)?;
                        if source == *seed_key {
                            if seed_alias.replace(binding).is_some() {
                                return None;
                            }
                        } else if is_captured_source(model, site, source) {
                            if depth.replace((binding, source)).is_some() {
                                return None;
                            }
                        } else if config_declarator.replace(binding).is_some() {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
            Statement::ExpressionStatement(_) => {
                let store = assignment(statement)?;
                match &store.left {
                    AssignmentTarget::StaticMemberExpression(target) => {
                        if target.optional || target.property.name != "extensions" {
                            return None;
                        }
                        let object = symbol(model, &target.object)?;
                        let value = symbol(model, &store.right)?;
                        if extensions_store.replace((index, object, value)).is_some() {
                            return None;
                        }
                    }
                    AssignmentTarget::AssignmentTargetIdentifier(target) => {
                        let object = crate::analysis::identifier_symbol(target, model.scoping)?;
                        let value = symbol(model, &store.right)?;
                        if config_store.replace((index, object, value)).is_some() {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
            Statement::IfStatement(_) => {
                if registration.replace(index).is_some() {
                    return None;
                }
            }
            _ => return None,
        }
    }

    let config_object = config_object?;
    let (extension_map, sentinel) = extension?;
    let (depth_extension, depth_source) = depth?;
    let (inner_symbol, inner_index) = inner?;
    let (return_index, returned_symbol) = returned?;
    let registration = registration?;
    let (_, extensions_object, extensions_value) = extensions_store?;

    if returned_symbol != inner_symbol
        || extensions_object != config_object
        || extensions_value != extension_map
    {
        return None;
    }
    let Statement::FunctionDeclaration(inner_function) = &body.statements[inner_index] else {
        return None;
    };
    let inner_site = model.site_by_span(inner_function.span)?;
    let Statement::ReturnStatement(end) = &body.statements[return_index] else {
        return None;
    };
    if model
        .function_from_expression(end.argument.as_ref()?)
        .map(|target| target.span)
        != Some(inner_function.span)
        || model
            .enclosing_function(inner_site.node_id)
            .map(|parent| parent.span)
            != Some(site.span)
    {
        return None;
    }
    let snapshot = snapshot_serializer(model, inner_function)?;

    // The configuration is captured by one distinct local: either an immutable
    // `let config = encodeConfig` declarator or exactly one later direct store
    // into a placeholder declared without an initializer.
    let config = match (config_declarator, config_placeholder, config_store) {
        (Some(alias), None, None) => {
            let AstKind::VariableDeclarator(variable) =
                model.nodes.kind(model.scoping.symbol_declaration(alias))
            else {
                return None;
            };
            if symbol(model, variable.init.as_ref()?)? != config_object
                || model.binding_writes(alias) != 0
            {
                return None;
            }
            alias
        }
        (None, Some(placeholder), Some((_, target, value))) => {
            if target != placeholder
                || value != config_object
                || model.binding_writes(placeholder) != 1
            {
                return None;
            }
            placeholder
        }
        _ => return None,
    };

    // The third formal is captured by one immutable alias, or used directly.
    let seed = match seed_alias {
        Some(alias) => {
            let AstKind::VariableDeclarator(variable) =
                model.nodes.kind(model.scoping.symbol_declaration(alias))
            else {
                return None;
            };
            if symbol(model, variable.init.as_ref()?)? != *seed_key
                || model.binding_writes(alias) != 0
            {
                return None;
            }
            alias
        }
        None => *seed_key,
    };

    extension_registration(
        model,
        &body.statements[registration],
        extension_map,
        depth_extension,
    )?;

    if !referenced_within(model, config, inner_site) || !referenced_within(model, seed, inner_site)
    {
        return None;
    }

    // Every proved identity is a distinct scoped symbol.
    let mut distinct = BTreeSet::new();
    for identity in [
        *keysets_to_omit,
        config_object,
        extension_map,
        depth_extension,
        depth_source,
        sentinel,
        config,
        seed,
        snapshot,
        inner_symbol,
    ] {
        if !distinct.insert(identity) {
            return None;
        }
    }
    if seed != *seed_key && !distinct.insert(*seed_key) {
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
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    if function.body.is_none() || create_signal_serializer(model, site, function).is_none() {
        return Vec::new();
    }
    vec![proposal(
        "createSignalSerializer",
        "posthost_signal_serializer_factory",
        "complete signal serializer factory body: a fixed keysetsToOmit/depthBound-100/unset-extensions encode configuration; a one-entry extensions map whose \"0\" slot is a captured extension object; a captured depth extension point converted to a data descriptor and registered under one consistent numeric id with Object.defineProperty and the matching computed store; the map stored into the configuration; the configuration and the used third formal kept in distinct locals captured by the returned nested serializer; the reserved second formal unused; the returned nested function is proved by its own call frame to read errors/positionalSignals/keyedSignals/signals from its single snapshot formal, with no nested-callback ownership; numeric-encoder, payload-finalizer and byte-assembly roles are not claimed",
        f,
    )]
}
