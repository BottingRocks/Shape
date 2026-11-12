//! Owned query/read/emission protocol. The captured receiver is opaque:
//! neither native DOM identity nor setup/lastChance activation is inferred.
use crate::naming::support::interaction::*;
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

fn query_receiver(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    method: &str,
    selector: &str,
) -> Option<SymbolId> {
    let query = call(expression)?;
    let [Argument::StringLiteral(argument)] = query.arguments.as_slice() else {
        return None;
    };
    if argument.value != selector {
        return None;
    }
    method_receiver(model, query, method)
}

fn remember_checkbox_protocol(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || function.params.rest.is_some()
        || function.params.items.len() != 1
    {
        return None;
    }
    let observer = plain_param(model, &function.params.items[0])?;
    let [selected, matches, update, emit] = body.statements.as_slice() else {
        return None;
    };
    let selected = variable(selected)?;
    let value = plain_var(model, selected)?;
    let Expression::LogicalExpression(first) = selected.init.as_ref()? else {
        return None;
    };
    if first.operator.as_str() != "||" {
        return None;
    }
    let receiver = captured_symbol(
        model,
        site,
        query_receiver(model, &first.left, "getElementById", "rememberMe")?,
    )?;
    if query_receiver(
        model,
        &first.right,
        "getElementById",
        "TravelerLoginForm_rememberMe",
    ) != Some(receiver)
    {
        return None;
    }
    let matches = variable(matches)?;
    let list = plain_var(model, matches)?;
    if query_receiver(
        model,
        matches.init.as_ref()?,
        "getElementsByName",
        "shouldRememberUser",
    ) != Some(receiver)
        || observer == value
        || observer == list
        || value == list
        || value == receiver
        || list == receiver
        || model.binding_writes(value) != 1
        || model.binding_writes(list) != 0
    {
        return None;
    }
    let update = assignment(expression_statement(update)?)?;
    if target_symbol(model, &update.left) != Some(value) {
        return None;
    }
    let Expression::ConditionalExpression(checked) = &update.right else {
        return None;
    };
    let (candidate, null) = binary(&checked.test, "!=")?;
    if symbol(model, candidate) != Some(value)
        || !matches!(null, Expression::NullLiteral(_))
        || member(&checked.consequent, "checked").and_then(|e| symbol(model, e)) != Some(value)
    {
        return None;
    }
    let Expression::ConditionalExpression(fallback) = &checked.alternate else {
        return None;
    };
    let (length, zero) = binary(&fallback.test, ">")?;
    if member(length, "length").and_then(|e| symbol(model, e)) != Some(list)
        || !number(zero, 0.0)
        || !matches!(&fallback.alternate, Expression::StringLiteral(s) if s.value.is_empty())
    {
        return None;
    }
    let (index, minus_one) = binary(&fallback.consequent, ">")?;
    let Expression::UnaryExpression(minus_one) = minus_one else {
        return None;
    };
    if minus_one.operator.as_str() != "-" || !number(&minus_one.argument, 1.0) {
        return None;
    }
    let index = call(index)?;
    if !matches!(index.arguments.as_slice(), [Argument::StringLiteral(s)] if s.value == "checked") {
        return None;
    }
    let class_name = member(&index.callee, "indexOf")?;
    let Expression::ComputedMemberExpression(first_match) = member(class_name, "className")? else {
        return None;
    };
    if first_match.optional
        || symbol(model, &first_match.object) != Some(list)
        || !number(&first_match.expression, 0.0)
    {
        return None;
    }
    let emit = call(expression_statement(emit)?)?;
    if method_receiver(model, emit, "next") != Some(observer)
        || !matches!(emit.arguments.as_slice(), [arg] if arg.as_expression().and_then(|e| symbol(model, e)) == Some(value))
    {
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
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if remember_hook_factory(model, function).is_some() {
        return vec![proposal("createRememberCheckboxCollectorHooks", "events_remember_checkbox_hook_factory", "Complete own zero-formal factory: private receiver initially uninitialized, exact setup/lastChance record initially void 0, setup stores its sole formal.global.document and sample owns the ordered remember-checkbox queries and next emission; exact callback publications and returned record with no other private receiver exposure; activation timing and native DOM identity remain opaque", f)];
    }
    if remember_checkbox_protocol(model, site, function, body).is_none() {
        return Vec::new();
    }
    vec![proposal(
        "queryRememberCheckboxAndEmitState",
        "events_remember_checkbox_protocol",
        "complete owned protocol: one lexical capture receives ordered getElementById(rememberMe) || getElementById(TravelerLoginForm_rememberMe) and getElementsByName(shouldRememberUser); the selected value becomes its checked property when nonnull, otherwise the first name match className.indexOf(checked) > -1 when length > 0, otherwise the empty string; the sole unchanged formal receives next of that exact value; native DOM identity is unproved and hook activation order is opaque",
        f,
    )]
}

// Factory naming consumes its own complete callback, setup and publication
// bodies. The returned hooks describe opaque receiver operations; activation
// timing and native DOM provenance are deliberately not inferred.
fn remember_hook_factory(model: &Model<'_, '_>, function: &Function<'_>) -> Option<()> {
    if function.r#async || function.generator || function.params.rest.is_some()
        || !function.params.items.is_empty() { return None; }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() { return None; }
    let [Statement::FunctionDeclaration(sample), record, capture,
        Statement::FunctionDeclaration(setup), publish_setup, publish_sample,
        Statement::ReturnStatement(finish)] = body.statements.as_slice() else { return None; };
    let sample_site = model.site_by_span(sample.span)?;
    remember_checkbox_protocol(model, sample_site, sample, sample.body.as_ref()?)?;
    let record = variable(record)?;
    let record_symbol = plain_var(model, record)?;
    let capture = variable(capture)?;
    let receiver = plain_var(model, capture)?;
    if capture.init.is_some() || model.binding_writes(receiver) != 1
        || model.binding_writes(record_symbol) != 0
        || symbol(model, finish.argument.as_ref()?) != Some(record_symbol) { return None; }
    let Expression::ObjectExpression(initial) = record.init.as_ref()? else { return None; };
    if initial.properties.len() != 2 { return None; }
    for (property, key) in initial.properties.iter().zip(["setup", "lastChance"]) {
        let ObjectPropertyKind::ObjectProperty(p) = property else { return None; };
        if p.computed || p.method || p.shorthand || p.kind != PropertyKind::Init
            || property_key_name(&p.key).as_deref() != Some(key)
            || !crate::naming::support::media_entropy::void_zero(&p.value) { return None; }
    }
    if setup.r#async || setup.generator || setup.params.rest.is_some() { return None; }
    let [parameter] = setup.params.items.as_slice() else { return None; };
    let input = plain_param(model, parameter)?;
    let setup_body = setup.body.as_ref()?;
    if !setup_body.directives.is_empty() { return None; }
    let [store] = setup_body.statements.as_slice() else { return None; };
    let store = assignment(expression_statement(store)?)?;
    if target_symbol(model, &store.left) != Some(receiver)
        || member(&store.right, "document").and_then(|e| member(e, "global"))
            .and_then(|e| symbol(model, e)) != Some(input) { return None; }
    // Tie the sample's query receiver to this exact setup destination.
    let [selected, ..] = sample.body.as_ref()?.statements.as_slice() else { return None; };
    let Expression::LogicalExpression(query) = variable(selected)?.init.as_ref()? else { return None; };
    if query_receiver(model, &query.left, "getElementById", "rememberMe") != Some(receiver) { return None; }
    for (statement, member, function) in [(publish_setup, "setup", &**setup), (publish_sample, "lastChance", &**sample)] {
        let store = assignment(expression_statement(statement)?)?;
        let AssignmentTarget::StaticMemberExpression(target) = &store.left else { return None; };
        let callback = function.id.as_ref()?.symbol_id.get()?;
        if target.optional || target.property.name != member
            || symbol(model, &target.object) != Some(record_symbol)
            || symbol(model, &store.right) != Some(callback)
            || model.binding_writes(callback) != 0 || model.binding_references(callback) != 1 { return None; }
    }
    if model.binding_references(record_symbol) != 3 || model.binding_references(input) != 1 { return None; }
    // This is the factory's private receiver; other writers/readers would add
    // an unchecked exposure or change the installed hook relation.
    if model.scoping.get_resolved_references(receiver).any(|r| {
        let span = model.nodes.kind(r.node_id()).span();
        !sample.span.contains_inclusive(span) && !setup.span.contains_inclusive(span)
    }) { return None; }
    Some(())
}

#[cfg(test)]
mod hook_factory_tests {
    use super::*;
    const SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/submit-forwarding/checkbox-hook.js"));
    fn count(source: &str) -> usize {
        let allocator = oxc_allocator::Allocator::default();
        let parsed = crate::analysis::parse(&allocator, source);
        assert!(parsed.errors.is_empty());
        let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
        let model = Model::new(source, &parsed.program, built.semantic.scoping(), built.semantic.nodes(), ProgramPropertiesLimits::default()).unwrap();
        model.all_sites().iter().filter(|site| matches!(model.nodes.kind(site.node_id), AstKind::Function(f) if remember_hook_factory(&model, f).is_some())).count()
    }
    #[test]
    fn remember_hook_factory_complete_body_and_near_misses() {
        assert_eq!(count(SOURCE), 2);
        assert_eq!(count(&SOURCE.replace("$KEY_0", "queryReceiver")), 2);
        for (from, to) in [(".lastChance =", ".other ="), (".global.document", ".global.other"), ("getElementsByName(\"shouldRememberUser\")", "getElementsByName(\"other\")")] {
            assert!(SOURCE.contains(from));
            assert_eq!(count(&SOURCE.replace(from, to)), 0, "accepted mutation {from}");
        }
    }
}
