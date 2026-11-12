//! Final presentation of semantically proven VM operation tables.
//!
//! This pass is deliberately not a role detector. Naming/FIND carries the
//! proven Operations binding across the post-RENAME source boundary; this
//! module reparses that source and resolves the corresponding fresh SymbolId.
//! A callable array that merely happens to have a bracket call is ignored.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::Span;
use oxc_syntax::scope::ScopeFlags;
use wasmparser::{Imports, Parser, Payload, Validator};

use crate::transforms::deob::pipeline::ProvenRoleBinding;
use crate::{
    analysis,
    diagnostics::{Diagnostic, Stage},
    roles::VmRole,
};

#[derive(Debug, Clone, Copy)]
struct Handler {
    index: usize,
    span: Span,
}

#[derive(Debug)]
struct Candidate {
    symbol: SymbolId,
    initializer: Span,
    handlers: Vec<Handler>,
    has_spread: bool,
    receiver_observable: bool,
}

/// The proven declaration's code-generation-stable identity.  Canonical spelling
/// is not identity on its own, so the document-order ordinal pins the exact
/// declaration and the initializer fingerprint confirms it was not substituted.
#[derive(Debug, Clone)]
struct Target {
    original_name: String,
    canonical_name: String,
    elements: u32,
    functions: u32,
}

struct OperationProbe<'a, 'n> {
    scoping: &'a Scoping,
    nodes: &'n AstNodes<'a>,
    targets: BTreeMap<u32, Target>,
    next_declarator: u32,
    resolved: BTreeSet<u32>,
    candidates: Vec<Candidate>,
    calls: BTreeSet<SymbolId>,
    approved_references: BTreeSet<(SymbolId, u32, u32)>,
    wasm_reference_spans: BTreeSet<(u32, u32)>,
    references: Vec<(SymbolId, Span)>,
    identity_diagnostics: Vec<Diagnostic>,
}

/// Convert only the post-RENAME binding(s) whose Operations role was proven by
/// FIND/RENAME. The fresh AST analysis below re-resolves the binding after
/// names and source spans have changed.
pub(crate) fn present(
    source: &str,
    proven_roles: &[ProvenRoleBinding],
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(String, usize), String> {
    let allocator = oxc_allocator::Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut targets: BTreeMap<u32, Target> = BTreeMap::new();
    for binding in proven_roles {
        if binding.role != VmRole::Operations {
            continue;
        }
        targets.insert(
            binding.declaration_ordinal,
            Target {
                original_name: binding.original_name.clone(),
                canonical_name: binding.canonical_name.clone(),
                elements: binding.initializer_elements,
                functions: binding.initializer_functions,
            },
        );
    }
    if targets.is_empty() {
        return Ok((source.to_owned(), 0));
    }

    let wasm_reference_spans = validated_wasm_namespace_reference_spans(&scoping, &nodes);
    let mut probe = OperationProbe {
        scoping: &scoping,
        nodes: &nodes,
        targets,
        next_declarator: 0,
        resolved: BTreeSet::new(),
        candidates: Vec::new(),
        calls: BTreeSet::new(),
        approved_references: BTreeSet::new(),
        wasm_reference_spans,
        references: Vec::new(),
        identity_diagnostics: Vec::new(),
    };
    probe.visit_program(&program);

    // Any proven ordinal that never resolved to a matching declaration is an
    // identity mismatch; decline explicitly instead of falling back to a name
    // search that could capture an unrelated same-spelled binding.
    let unresolved: Vec<(u32, String, String)> = probe
        .targets
        .iter()
        .filter(|(ordinal, _)| !probe.resolved.contains(ordinal))
        .map(|(ordinal, target)| {
            (
                *ordinal,
                target.original_name.clone(),
                target.canonical_name.clone(),
            )
        })
        .collect();
    for (ordinal, original_name, canonical_name) in unresolved {
        probe.identity_diagnostics.push(Diagnostic::warn(
            Stage::Extract,
            "OPERATION_OBJECT_IDENTITY_DECLINED",
            format!(
                "proven operation declaration #{ordinal} (`{original_name}` -> `{canonical_name}`) could not be resolved to a unique binding; retained as an array"
            ),
        ));
    }
    diagnostics.append(&mut probe.identity_diagnostics);

    let mut edits = Vec::new();
    for candidate in probe.candidates {
        if !probe.calls.contains(&candidate.symbol) {
            continue;
        }
        let unsupported = probe
            .references
            .iter()
            .filter(|(symbol, _)| *symbol == candidate.symbol)
            .map(|(_, span)| *span)
            .filter(|span| {
                !probe
                    .approved_references
                    .contains(&(candidate.symbol, span.start, span.end))
            })
            .count();
        let out_of_range = candidate
            .handlers
            .iter()
            .any(|handler| handler.index > u32::MAX as usize);
        if candidate.receiver_observable || candidate.has_spread || unsupported > 0 || out_of_range
        {
            let reason = if candidate.receiver_observable {
                "an operation handler observes its array receiver through `this`"
            } else if candidate.has_spread {
                "spread elements make operation indices dynamic"
            } else if out_of_range {
                "an operation index is outside the canonical numeric key range"
            } else {
                "the operation table has a length, iteration, array-method, assignment, or escaping reference"
            };
            diagnostics.push(
                Diagnostic::warn(
                    Stage::Extract,
                    "OPERATION_OBJECT_DECLINED",
                    format!(
                        "proven operation table at {}..{} retained as an array: {reason}",
                        candidate.initializer.start, candidate.initializer.end
                    ),
                )
                .with_span(candidate.initializer.start, candidate.initializer.end),
            );
            continue;
        }

        let replacement = render_object(source, &candidate.handlers);
        edits.push(analysis::Edit::replace(candidate.initializer, replacement));
    }

    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        diagnostics.push(Diagnostic::warn(
            Stage::Extract,
            "OPERATION_OBJECT_OVERLAP",
            analysis::describe_rejected_edits(&applied.rejected),
        ));
    }
    let changed = applied.applied;
    let output = applied.output;
    if changed == 0 {
        return Ok((output, changed));
    }
    let formatted_allocator = oxc_allocator::Allocator::default();
    let formatted = analysis::parse_program(&formatted_allocator, &output)?;
    Ok((analysis::codegen(&formatted), changed))
}

impl<'a, 'n> Visit<'a> for OperationProbe<'a, 'n> {
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        let ordinal = self.next_declarator;
        self.next_declarator += 1;

        let Some(binding) = simple_binding(&it.id) else {
            walk::walk_variable_declarator(self, it);
            return;
        };
        // Resolve by document-order ordinal, never by spelling. The name and
        // fingerprint below are consistency checks, not the identity itself.
        let Some(target) = self.targets.get(&ordinal).cloned() else {
            walk::walk_variable_declarator(self, it);
            return;
        };
        // The proven ordinal maps to exactly one declaration; everything below
        // validates that this declaration still matches its recorded identity.
        self.resolved.insert(ordinal);
        if binding.name.as_str() != target.canonical_name {
            self.identity_diagnostics.push(
                Diagnostic::warn(
                    Stage::Extract,
                    "OPERATION_OBJECT_IDENTITY_DECLINED",
                    format!(
                        "proven operation declaration #{ordinal} (`{}` -> `{}`) is now `{}`; retained as an array",
                        target.original_name, target.canonical_name, binding.name
                    ),
                )
                .with_span(it.span.start, it.span.end),
            );
            walk::walk_variable_declarator(self, it);
            return;
        }
        let Some(symbol) = binding.symbol_id.get() else {
            walk::walk_variable_declarator(self, it);
            return;
        };
        let Some(Expression::ArrayExpression(array)) = it.init.as_ref() else {
            self.identity_diagnostics.push(
                Diagnostic::warn(
                    Stage::Extract,
                    "OPERATION_OBJECT_IDENTITY_DECLINED",
                    format!(
                        "proven operation declaration #{ordinal} (`{}` -> `{}`) lost its array initializer; retained as-is",
                        target.original_name, target.canonical_name
                    ),
                )
                .with_span(it.span.start, it.span.end),
            );
            walk::walk_variable_declarator(self, it);
            return;
        };
        let functions = array
            .elements
            .iter()
            .filter(|element| {
                matches!(
                    element,
                    ArrayExpressionElement::FunctionExpression(_)
                        | ArrayExpressionElement::ArrowFunctionExpression(_)
                )
            })
            .count();
        if array.elements.len() != target.elements as usize
            || functions != target.functions as usize
        {
            self.identity_diagnostics.push(
                Diagnostic::warn(
                    Stage::Extract,
                    "OPERATION_OBJECT_IDENTITY_DECLINED",
                    format!(
                        "proven operation declaration #{ordinal} (`{}` -> `{}`) no longer has its recorded {} element(s)/{} function(s); retained as an array",
                        target.original_name, target.canonical_name, target.elements, target.functions
                    ),
                )
                .with_span(array.span.start, array.span.end),
            );
            walk::walk_variable_declarator(self, it);
            return;
        }

        let mut handlers = Vec::new();
        let mut has_spread = false;
        let mut receiver_observable = false;
        for (index, element) in array.elements.iter().enumerate() {
            match element {
                ArrayExpressionElement::FunctionExpression(function) => {
                    let mut receiver = ReceiverUseProbe::default();
                    receiver.visit_function(function, ScopeFlags::empty());
                    receiver_observable |= receiver.observable;
                    handlers.push(Handler {
                        index,
                        span: function.span,
                    });
                }
                ArrayExpressionElement::ArrowFunctionExpression(function) => {
                    let mut receiver = ReceiverUseProbe::default();
                    receiver.visit_arrow_function_expression(function);
                    receiver_observable |= receiver.observable;
                    handlers.push(Handler {
                        index,
                        span: function.span,
                    });
                }
                ArrayExpressionElement::SpreadElement(_) => has_spread = true,
                ArrayExpressionElement::Elision(_) => {}
                _ => {
                    handlers.clear();
                    has_spread = false;
                    receiver_observable = false;
                    break;
                }
            }
        }
        if !handlers.is_empty() {
            self.candidates.push(Candidate {
                symbol,
                initializer: array.span,
                handlers,
                has_spread,
                receiver_observable,
            });
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Expression::ComputedMemberExpression(member) = &it.callee {
            if let Expression::Identifier(identifier) = &member.object {
                if computed_member_is_numeric_index(member) {
                    if let Some(symbol) = identifier_symbol_with_scoping(identifier, self.scoping) {
                        self.calls.insert(symbol);
                        self.approved_references.insert((
                            symbol,
                            identifier.span.start,
                            identifier.span.end,
                        ));
                    }
                }
            }
        }
        walk::walk_call_expression(self, it);
    }

    /// A numeric computed read preserves its meaning when the array becomes an
    /// object: `operations[v]` reads the same property either way.  The callee
    /// form above is only one such read; a handler passed to a wrapper (for
    /// example `wrapped(operations[v], context)`) is the same access with a
    /// different parent, so it must not be classified as an escaping reference.
    /// Writes are still treated as unsupported because a non-index key would
    /// gain a plain property instead of the array's own slot.
    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        if let Expression::Identifier(identifier) = &it.object {
            if computed_member_is_numeric_index(it)
                && !computed_member_is_write_target(self.nodes, it)
            {
                if let Some(symbol) = identifier_symbol_with_scoping(identifier, self.scoping) {
                    self.approved_references.insert((
                        symbol,
                        identifier.span.start,
                        identifier.span.end,
                    ));
                }
            }
        }
        walk::walk_computed_member_expression(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        if let Some(symbol) = identifier_symbol_with_scoping(it, self.scoping) {
            self.references.push((symbol, it.span));
            if self
                .wasm_reference_spans
                .contains(&(it.span.start, it.span.end))
            {
                self.approved_references
                    .insert((symbol, it.span.start, it.span.end));
            }
        }
        walk::walk_identifier_reference(self, it);
    }
}

/// Approve only accesses that can denote an array index after the table is
/// represented as an object. Known non-index strings such as `"length"`,
/// `"toString"`, and arbitrary property names must remain conservative. An
/// identifier is retained as a supported dynamic index because proven VM
/// dispatchers receive their operation selector as a numeric value.
fn computed_member_is_numeric_index(member: &ComputedMemberExpression<'_>) -> bool {
    match &member.expression {
        Expression::NumericLiteral(number) => {
            number.value.is_finite()
                && number.value >= 0.0
                && number.value.fract() == 0.0
                && number.value < u32::MAX as f64
        }
        Expression::StringLiteral(string) => canonical_array_index(string.value.as_str()),
        Expression::Identifier(_) => true,
        Expression::ComputedMemberExpression(index) if arguments_last_index(index) => true,
        _ => false,
    }
}

fn arguments_last_index(member: &ComputedMemberExpression<'_>) -> bool {
    let Expression::Identifier(arguments) = &member.object else {
        return false;
    };
    if arguments.name != "arguments" {
        return false;
    }
    let Expression::BinaryExpression(subtract) = &member.expression else {
        return false;
    };
    if subtract.operator != BinaryOperator::Subtraction
        || !matches!(&subtract.right, Expression::NumericLiteral(value) if value.value == 1.0)
    {
        return false;
    }
    matches!(
        &subtract.left,
        Expression::StaticMemberExpression(length)
            if length.property.name == "length"
                && matches!(&length.object,
                    Expression::Identifier(identifier) if identifier.name == "arguments")
    )
}

fn canonical_array_index(value: &str) -> bool {
    let Ok(index) = value.parse::<u32>() else {
        return false;
    };
    index < u32::MAX && index.to_string() == value
}

/// Whether the computed member is the target of a write.  Only the immediate
/// parent is inspected; a read nested inside a larger expression is approved.
fn computed_member_is_write_target(
    nodes: &AstNodes<'_>,
    member: &ComputedMemberExpression<'_>,
) -> bool {
    let id = member.node_id.get();
    match nodes.parent_kind(id) {
        AstKind::AssignmentExpression(assignment) => matches!(
            &assignment.left,
            AssignmentTarget::ComputedMemberExpression(target) if target.node_id.get() == id
        ),
        AstKind::UpdateExpression(update) => matches!(
            &update.argument,
            SimpleAssignmentTarget::ComputedMemberExpression(target) if target.node_id.get() == id
        ),
        AstKind::UnaryExpression(unary) => {
            unary.operator == UnaryOperator::Delete
                && matches!(
                    &unary.argument,
                    Expression::ComputedMemberExpression(target) if target.node_id.get() == id
                )
        }
        AstKind::ForInStatement(statement) => matches!(
            &statement.left,
            ForStatementLeft::ComputedMemberExpression(target) if target.node_id.get() == id
        ),
        AstKind::ForOfStatement(statement) => matches!(
            &statement.left,
            ForStatementLeft::ComputedMemberExpression(target) if target.node_id.get() == id
        ),
        _ => false,
    }
}

#[derive(Default)]
struct ReceiverUseProbe {
    observable: bool,
}

impl<'a> Visit<'a> for ReceiverUseProbe {
    fn visit_this_expression(&mut self, _it: &ThisExpression) {
        self.observable = true;
    }
}

type DeclarationMap<'a> = BTreeMap<SymbolId, &'a Expression<'a>>;
type WasmImports = BTreeMap<String, BTreeSet<String>>;

fn validated_wasm_namespace_reference_spans(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> BTreeSet<(u32, u32)> {
    let declarations = declarations(nodes);
    let decoders = proven_decoder_symbols(nodes);
    let modules = validated_modules(&declarations, scoping, &decoders);
    let mut approved = BTreeSet::new();
    for node in nodes.iter() {
        let AstKind::NewExpression(instance) = node.kind() else {
            continue;
        };
        if !is_webassembly_instance(&instance.callee, scoping) {
            continue;
        }
        let Some(Argument::Identifier(module)) = instance.arguments.first() else {
            continue;
        };
        let Some(module_symbol) = identifier_symbol_with_scoping(module, scoping) else {
            continue;
        };
        let Some(imports) = modules.get(&module_symbol) else {
            continue;
        };
        let Some(imports_expression) = instance.arguments.get(1).and_then(Argument::as_expression)
        else {
            continue;
        };
        let imports_object = match imports_expression {
            Expression::ObjectExpression(object) => Some(&**object),
            Expression::Identifier(identifier) => {
                identifier_symbol_with_scoping(identifier, scoping)
                    .and_then(|symbol| declarations.get(&symbol).copied())
                    .and_then(|expression| match expression {
                        Expression::ObjectExpression(object) => Some(&**object),
                        _ => None,
                    })
            }
            _ => None,
        };
        let Some(imports_object) = imports_object else {
            continue;
        };
        if imports_object.properties.iter().any(|property| {
            matches!(property, ObjectPropertyKind::SpreadProperty(_))
                || matches!(property, ObjectPropertyKind::ObjectProperty(property) if property.computed)
        }) {
            continue;
        }
        for (namespace, names) in imports {
            if names.is_empty() || !names.iter().all(|name| canonical_index(name).is_some()) {
                continue;
            }
            let Some(property) = imports_object.properties.iter().find_map(|property| {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                (property
                    .key
                    .name()
                    .is_some_and(|name| name.as_ref() == namespace))
                .then_some(property)
            }) else {
                continue;
            };
            let Expression::Identifier(identifier) = &property.value else {
                continue;
            };
            approved.insert((identifier.span.start, identifier.span.end));
        }
    }
    approved
}

fn declarations<'a>(nodes: &AstNodes<'a>) -> DeclarationMap<'a> {
    nodes
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                let binding = simple_binding(&declarator.id)?;
                Some((binding.symbol_id.get()?, declarator.init.as_ref()?))
            }
            _ => None,
        })
        .collect()
}

fn validated_modules(
    declarations: &DeclarationMap<'_>,
    scoping: &Scoping,
    decoders: &BTreeSet<SymbolId>,
) -> BTreeMap<SymbolId, WasmImports> {
    let mut out = BTreeMap::new();
    for (&symbol, init) in declarations {
        let Expression::NewExpression(module) = *init else {
            continue;
        };
        if !is_webassembly_module(&module.callee, scoping) {
            continue;
        }
        let Some(argument) = module.arguments.first().and_then(Argument::as_expression) else {
            continue;
        };
        let Some(bytes) = static_module_bytes(argument, declarations, scoping, decoders) else {
            continue;
        };
        let Some(imports) = parse_validated_imports(&bytes) else {
            continue;
        };
        out.insert(symbol, imports);
    }
    out
}

/// Resolve the `WebAssembly.Module` argument to concrete bytes.  The byte
/// literal can be behind one or more value bindings (`var bytes = decode(...);
/// new WebAssembly.Module(bytes)`), so identifiers are followed through the
/// declaration map.  A decoder must be *proven by its body* (base64 alphabet
/// plus a decode loop) rather than accepted because of its spelling: the
/// spelling is a post-RENAME canonical name and is not identity.
fn static_module_bytes(
    expression: &Expression<'_>,
    declarations: &DeclarationMap<'_>,
    scoping: &Scoping,
    decoders: &BTreeSet<SymbolId>,
) -> Option<Vec<u8>> {
    match expression {
        Expression::StringLiteral(string) => crate::decode_base64(string.value.as_str()).ok(),
        Expression::Identifier(identifier) => {
            let symbol = identifier_symbol_with_scoping(identifier, scoping)?;
            static_module_bytes(declarations.get(&symbol)?, declarations, scoping, decoders)
        }
        Expression::CallExpression(call) => {
            let Expression::Identifier(callee) = &call.callee else {
                return None;
            };
            // Identity, never spelling: only a decoder whose body is the base64
            // alphabet decode loop is accepted.  An unproven callee (including a
            // global named `atob`) declines rather than guessing.
            let proven = identifier_symbol_with_scoping(callee, scoping)
                .is_some_and(|symbol| decoders.contains(&symbol));
            if !proven {
                return None;
            }
            let Expression::StringLiteral(string) = call.arguments.first()?.as_expression()? else {
                return None;
            };
            crate::decode_base64(string.value.as_str()).ok()
        }
        _ => None,
    }
}

/// Symbols of functions whose body implements the base64 alphabet decode loop.
/// This is the same structural proof used to name the decoder during RENAME, so
/// the presentation pass accepts exactly the decoder that earlier passes proved.
fn proven_decoder_symbols(nodes: &AstNodes<'_>) -> BTreeSet<SymbolId> {
    let mut symbols = BTreeSet::new();
    for node in nodes.iter() {
        match node.kind() {
            AstKind::Function(function) => {
                if function_has_base64_body(function) {
                    if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        symbols.insert(symbol);
                    }
                }
            }
            AstKind::VariableDeclarator(declarator) => {
                let Some(binding) = simple_binding(&declarator.id) else {
                    continue;
                };
                let Some(symbol) = binding.symbol_id.get() else {
                    continue;
                };
                let decoder = match declarator.init.as_ref() {
                    Some(Expression::FunctionExpression(function)) => {
                        function_has_base64_body(function)
                    }
                    Some(Expression::ArrowFunctionExpression(function)) => {
                        let mut probe = Base64BodyProbe::default();
                        probe.visit_arrow_function_expression(function);
                        probe.proves_decoder()
                    }
                    _ => false,
                };
                if decoder {
                    symbols.insert(symbol);
                }
            }
            _ => {}
        }
    }
    symbols
}

fn function_has_base64_body(function: &Function<'_>) -> bool {
    let mut probe = Base64BodyProbe::default();
    probe.visit_function(function, ScopeFlags::empty());
    probe.proves_decoder()
}

#[derive(Default)]
struct Base64BodyProbe {
    alphabet: bool,
    loops: usize,
}

impl Base64BodyProbe {
    fn proves_decoder(&self) -> bool {
        self.alphabet && self.loops > 0
    }
}

impl<'a> Visit<'a> for Base64BodyProbe {
    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        if it.value == crate::runtime_decoder::STANDARD_ALPHABET
            || it.value == crate::runtime_decoder::URL_SAFE_ALPHABET
        {
            self.alphabet = true;
        }
    }

    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        self.loops += 1;
        walk::walk_for_statement(self, it);
    }
}

fn parse_validated_imports(bytes: &[u8]) -> Option<WasmImports> {
    Validator::new().validate_all(bytes).ok()?;
    let mut imports = BTreeMap::<String, BTreeSet<String>>::new();
    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::ImportSection(reader) = payload.ok()? else {
            continue;
        };
        for group in reader {
            match group.ok()? {
                Imports::Single(_, import) => {
                    imports
                        .entry(import.module.to_owned())
                        .or_default()
                        .insert(import.name.to_owned());
                }
                Imports::Compact1 { module, items } => {
                    for item in items {
                        let item = item.ok()?;
                        imports
                            .entry(module.to_owned())
                            .or_default()
                            .insert(item.name.to_owned());
                    }
                }
                Imports::Compact2 { module, names, .. } => {
                    for name in names {
                        imports
                            .entry(module.to_owned())
                            .or_default()
                            .insert(name.ok()?.to_owned());
                    }
                }
            }
        }
    }
    Some(imports)
}

fn canonical_index(value: &str) -> Option<u32> {
    let index = value.parse::<u32>().ok()?;
    (index < u32::MAX && index.to_string() == value).then_some(index)
}

fn render_object(source: &str, handlers: &[Handler]) -> String {
    let properties = handlers
        .iter()
        .map(|handler| {
            format!(
                "{}: {}",
                handler.index,
                &source[handler.span.start as usize..handler.span.end as usize]
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {properties} }}")
}

fn simple_binding<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a BindingIdentifier<'a>> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier),
        _ => None,
    }
}

fn identifier_symbol_with_scoping(
    identifier: &IdentifierReference<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    crate::analysis::identifier_symbol(identifier, scoping)
}

fn is_webassembly_instance(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(expression, Expression::StaticMemberExpression(member)
        if matches!(&member.object, Expression::Identifier(identifier)
            if identifier.name == "WebAssembly"
                && identifier.reference_id.get().is_some()
                && crate::analysis::identifier_symbol(identifier, scoping).is_none())
            && member.property.name == "Instance")
}

fn is_webassembly_module(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(expression, Expression::StaticMemberExpression(member)
        if matches!(&member.object, Expression::Identifier(identifier)
            if identifier.name == "WebAssembly"
                && identifier.reference_id.get().is_some()
                && crate::analysis::identifier_symbol(identifier, scoping).is_none())
            && member.property.name == "Module")
}
