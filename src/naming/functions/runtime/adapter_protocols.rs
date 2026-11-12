//! Closed, capture-free adapter protocols. Compare the entire parsed body after
//! alpha conversion by resolved SymbolId; member names, effects, control flow,
//! receiver/argument order and return shape remain literal parts of the proof.
//! These are semantic grammars, independent of producer/vendor identifiers.
use crate::naming::*;
use oxc_span::GetSpan;
use std::sync::OnceLock;

const CONTRACTS: &[(&str, &str, &str)] = &[
    ("readHrefProperty", "browser_href_accessors", "function adapter(object) { return object.href; }"),
    ("writeHrefProperty", "browser_href_accessors", "function adapter(object, value) { object.href = value; }"),
    ("normalizeJsonOrReportInvalid", "json_roundtrip_with_invalid_error", "function adapter(input) { try { return JSON.stringify(JSON.parse(input)); } catch (error) { return \"INVALID:\" + error; } }"),
];

fn canonical(source: &str) -> Option<String> {
    let arena = Allocator::default();
    let program = crate::analysis::parse_program(&arena, source).ok()?;
    Some(
        oxc_codegen::Codegen::new()
            .with_options(oxc_codegen::CodegenOptions {
                comments: oxc_codegen::CommentOptions::disabled(),
                ..Default::default()
            })
            .build(&program)
            .code,
    )
}

/// No external symbol is accepted, including a locally shadowed `JSON`.
/// All declaration/reference occurrences use the same resolved identity. This
/// preserves wrong-capture refusals even when spelling matches a local binder.
fn normalized(model: &Model<'_, '_>, site: FunctionSite) -> Option<String> {
    let mut occurrences = BTreeMap::new();
    for node in model
        .nodes
        .iter()
        .filter(|n| site.span.contains_inclusive(n.kind().span()))
    {
        let (span, symbol) = match node.kind() {
            AstKind::BindingIdentifier(id) => (id.span, id.symbol_id.get()?),
            AstKind::IdentifierReference(id) => {
                let Some(symbol) = crate::analysis::identifier_symbol(id, model.scoping) else {
                    if id.name != "JSON" {
                        return None;
                    }
                    continue;
                };
                (id.span, symbol)
            }
            AstKind::WithStatement(_) => return None,
            _ => continue,
        };
        if !model.scoping.symbol_redeclarations(symbol).is_empty()
            || !site
                .span
                .contains_inclusive(model.scoping.symbol_span(symbol))
            || model.scoping.get_resolved_references(symbol).any(|r| {
                r.is_write()
                    && !site
                        .span
                        .contains_inclusive(model.nodes.kind(r.node_id()).span())
            })
        {
            return None;
        }
        occurrences.insert((span.start, span.end), symbol);
    }
    let mut locals = BTreeMap::new();
    let mut edits = Vec::new();
    for ((start, end), symbol) in occurrences {
        let next = locals.len();
        let index = *locals.entry(symbol).or_insert(next);
        edits.push((start, end, format!("__adapterLocal{index}")));
    }
    let mut text = model
        .source
        .get(site.span.start as usize..site.span.end as usize)?
        .to_owned();
    for (start, end, replacement) in edits.into_iter().rev() {
        text.replace_range(
            (start - site.span.start) as usize..(end - site.span.start) as usize,
            &replacement,
        );
    }
    canonical(&text)
}

fn expected() -> Vec<String> {
    CONTRACTS
        .iter()
        .map(|(_, _, contract)| {
            let source = format!("function main(){{ {contract} return adapter; }} main();");
            let arena = Allocator::default();
            let program =
                crate::analysis::parse_program(&arena, &source).expect("adapter grammar parses");
            let (scoping, nodes) = crate::analysis::analyze(&program);
            let model = Model::new(
                &source,
                &program,
                &scoping,
                &nodes,
                ProgramPropertiesLimits::default(),
            )
            .expect("adapter grammar indexes");
            let site = model
                .all_sites()
                .iter()
                .find(|s| {
                    s.binding_symbol
                        .is_some_and(|id| scoping.symbol_name(id) == "adapter")
                })
                .expect("adapter grammar site");
            normalized(&model, *site).expect("adapter grammar is capture free")
        })
        .collect()
}

/// A deliberately narrow capture origin: one immutable same-owner variable
/// initialized directly from an ordinary hoisted function declaration. Its
/// body is an opaque consumer; its identity/readiness, not its purpose, matters.
fn envelope_consumer(model: &Model<'_, '_>, callee: &Expression<'_>) -> Option<()> {
    let symbol = crate::analysis::expression_symbol(callee, model.scoping)?;
    if model.binding_writes(symbol) != 0 || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(symbol))
    {
        return None;
    }
    let statement = model.nodes.parent_id(declaration);
    let owner = model.nodes.parent_id(statement);
    if !matches!(model.nodes.kind(statement), AstKind::VariableDeclaration(_))
        || !matches!(model.nodes.kind(owner), AstKind::FunctionBody(_))
    {
        return None;
    }
    let Expression::Identifier(origin) = variable.init.as_ref()? else {
        return None;
    };
    let origin_symbol = crate::analysis::identifier_symbol(origin, model.scoping)?;
    if model.binding_writes(origin_symbol) != 0
        || !model
            .scoping
            .symbol_redeclarations(origin_symbol)
            .is_empty()
    {
        return None;
    }
    let origin_declaration = model.scoping.symbol_declaration(origin_symbol);
    let AstKind::Function(function) = model.nodes.kind(origin_declaration) else {
        return None;
    };
    if function.r#async || function.generator || model.nodes.parent_id(origin_declaration) != owner
    {
        return None;
    }
    // The shared resolver checks actual exposures of a hoisted callable, not
    // just textual declaration order. Its direct-body origin restriction also
    // excludes conditional setup and nested deferred writers.
    let resolved = crate::naming::shared::stable_function(model, callee, 0)?;
    (resolved.span == function.span).then_some(())
}

fn envelope(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<()> {
    use crate::naming::support::seeded_flow::{assignment, member, plain_params, symbol, unchanged};
    if function.params.items.len() != 1 || !plain_params(model, &function.params) {
        return None;
    }
    let body = function.body.as_ref()?;
    let [declaration, event_store, target_store, Statement::ReturnStatement(ret)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let variable = crate::naming::shared::one_variable(declaration)?;
    let BindingPattern::BindingIdentifier(record) = &variable.id else {
        return None;
    };
    let record = record.symbol_id.get()?;
    if !unchanged(model, record) {
        return None;
    }
    let Expression::ObjectExpression(object) = variable.init.as_ref()? else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(event), ObjectPropertyKind::ObjectProperty(target)] =
        object.properties.as_slice()
    else {
        return None;
    };
    for (property, key) in [(event, "event"), (target, "target")] {
        if property.computed
            || property.method
            || property.shorthand
            || property.kind != PropertyKind::Init
            || property_key_name(&property.key).as_deref() != Some(key)
            || !matches!(&property.value, Expression::UnaryExpression(u) if u.operator.as_str() == "void" && matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 0.0))
        {
            return None;
        }
    }
    for (statement, key, target_value) in [
        (event_store, "event", false),
        (target_store, "target", true),
    ] {
        let store = assignment(statement)?;
        let AssignmentTarget::StaticMemberExpression(field) = &store.left else {
            return None;
        };
        if field.optional
            || field.property.name != key
            || symbol(model, &field.object) != Some(record)
        {
            return None;
        }
        let value = if target_value {
            member(&store.right, "target")?
        } else {
            &store.right
        };
        if model.site_parameter_slot(value, site) != Some(0) {
            return None;
        }
    }
    let Expression::CallExpression(invoke) = ret.argument.as_ref()? else {
        return None;
    };
    let [argument] = invoke.arguments.as_slice() else {
        return None;
    };
    if invoke.optional || symbol(model, argument.as_expression()?) != Some(record) {
        return None;
    }
    let captured = symbol(model, &invoke.callee)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(captured))
    {
        return None;
    }
    envelope_consumer(model, &invoke.callee)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    // Bounded small adapters only. This is a work bound, never positive evidence.
    if site.span.size() > 2048 {
        return vec![];
    }
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    if function.r#async || function.generator {
        return vec![];
    }
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if !body.directives.is_empty() {
        return vec![];
    }
    if envelope(model, site, function).is_some() {
        return vec![proposal("forwardEventAndTarget", "events_event_target_envelope",
            "whole owned event envelope: initialize event/target to void 0, store original event then its target, return same record passed to one immutable captured callable; same-owner direct function-declaration alias and actual exposure readiness proved; consumer body/native/form purpose explicitly unproved", facts)];
    }
    static EXPECTED: OnceLock<Vec<String>> = OnceLock::new();
    let Some(actual) = normalized(model, site) else {
        return vec![];
    };
    for ((name, evaluator, _), expected) in CONTRACTS.iter().zip(EXPECTED.get_or_init(expected)) {
        if &actual == expected {
            return vec![proposal(name, evaluator,
                "whole parsed capture-free adapter grammar: resolved local and callback identities; exact ordered operations, native member spelling, receiver/argument slots, branches and returns; no external helper/native identity inferred", facts)];
        }
    }
    vec![]
}
