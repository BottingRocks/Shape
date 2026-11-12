use super::*;
use oxc_span::GetSpan;

use crate::naming::support::entropy_relay::*;

fn four_byte_tag(model: &Model<'_, '_>, tag: &Expression<'_>, seed: Symbol) -> bool {
    let Expression::ArrayExpression(array) = tag else {
        return false;
    };
    let elements: Vec<_> = array
        .elements
        .iter()
        .filter_map(ArrayExpressionElement::as_expression)
        .collect();
    let [a, b, c, d] = elements.as_slice() else {
        return false;
    };
    let unsigned = |e: &Expression<'_>, shift: f64| {
        matches!(e, Expression::BinaryExpression(b)
            if b.operator.as_str() == ">>>"
                && is_symbol(model, &b.left, seed)
                && number(&b.right, shift))
    };
    let ranged = |e: &Expression<'_>, shift: f64| {
        matches!(e, Expression::BinaryExpression(and)
            if and.operator.as_str() == "&"
                && number(&and.right, 255.0)
                && matches!(&and.left, Expression::BinaryExpression(shifted)
                    if shifted.operator.as_str() == ">>"
                        && is_symbol(model, &shifted.left, seed)
                        && number(&shifted.right, shift)))
    };
    unsigned(a, 24.0)
        && ranged(b, 16.0)
        && ranged(c, 8.0)
        && matches!(d, Expression::BinaryExpression(and)
            if and.operator.as_str() == "&"
                && is_symbol(model, &and.left, seed)
                && number(&and.right, 255.0))
}

// ``random 32-bit value -> 4-byte entropy tag -> per-value keystream state ->
// repeated value encoder over a numeric sequence in reverse``; the encoded
// buffer is then emitted through a captured observer. The captured encoder and
// keystream functions are resolved through ordered single-origin callables.
fn encode_numeric_array_with_entropy_tag(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> bool {
    let [seed, tag, stream, length, offset, Statement::ForStatement(loop_), emit, complete] =
        body.statements.as_slice()
    else {
        return false;
    };
    let Some(seed_value) = one_variable(seed) else {
        return false;
    };
    let Some(seed_symbol) = plain_declarator(model, seed_value) else {
        return false;
    };
    let Some(Expression::BinaryExpression(seed_or)) = seed_value.init.as_ref() else {
        return false;
    };
    let Expression::BinaryExpression(product) = &seed_or.left else {
        return false;
    };
    let Expression::CallExpression(random) = &product.left else {
        return false;
    };
    if seed_or.operator.as_str() != "|"
        || !number(&seed_or.right, 0.0)
        || product.operator.as_str() != "*"
        || !number(&product.right, 4_294_967_296.0)
        || !global_math_call(model, random, "random")
        || !random.arguments.is_empty()
    {
        return false;
    }
    let Some(tag_value) = one_variable(tag) else {
        return false;
    };
    let Some(tag_symbol) = plain_declarator(model, tag_value) else {
        return false;
    };
    let Some(tag_init) = tag_value.init.as_ref() else {
        return false;
    };
    if tag_symbol == seed_symbol || !four_byte_tag(model, tag_init, seed_symbol) {
        return false;
    }
    let Some(stream_value) = one_variable(stream) else {
        return false;
    };
    let Some(stream_symbol) = plain_declarator(model, stream_value) else {
        return false;
    };
    let Some(Expression::CallExpression(keyed)) = stream_value.init.as_ref() else {
        return false;
    };
    let Some(keyed_site) = ordered_function(model, &keyed.callee, keyed.callee.span().start, 0)
    else {
        return false;
    };
    let keyed_arguments = call_arguments(keyed);
    let [seed_xor] = keyed_arguments.as_slice() else {
        return false;
    };
    let Expression::BinaryExpression(seed_xor) = seed_xor else {
        return false;
    };
    if keyed.optional
        || keyed_site.span == site.span
        || seed_xor.operator.as_str() != "^"
        || !is_symbol(model, &seed_xor.left, seed_symbol)
        || !matches!(&seed_xor.right, Expression::NumericLiteral(n) if n.value.is_finite())
    {
        return false;
    }
    let Some(length_value) = one_variable(length) else {
        return false;
    };
    let Some(length_symbol) = plain_declarator(model, length_value) else {
        return false;
    };
    let Some(length_init) = length_value.init.as_ref() else {
        return false;
    };
    let Some(length_read) = member(length_init, "length") else {
        return false;
    };
    if slot_of(model, site, length_read) != Some(0) {
        return false;
    }
    let Some(offset_value) = one_variable(offset) else {
        return false;
    };
    let Some(offset_symbol) = plain_declarator(model, offset_value) else {
        return false;
    };
    let Some(Expression::CallExpression(first)) = offset_value.init.as_ref() else {
        return false;
    };
    let Some(encoder) = ordered_function(model, &first.callee, first.callee.span().start, 0) else {
        return false;
    };
    let first_arguments = call_arguments(first);
    let [first_stream, first_length, first_tag, first_offset] = first_arguments.as_slice() else {
        return false;
    };
    if encoder.span == site.span
        || encoder.span == keyed_site.span
        || !is_symbol(model, first_stream, stream_symbol)
        || !is_symbol(model, first_length, length_symbol)
        || !is_symbol(model, first_tag, tag_symbol)
        || !number(first_offset, 3.0)
    {
        return false;
    }
    let [Statement::ExpressionStatement(step), Statement::IfStatement(stop), Statement::ExpressionStatement(update_step), Statement::ExpressionStatement(restore)] =
        block(&loop_.body).unwrap_or_default()
    else {
        return false;
    };
    let Expression::UpdateExpression(step) = &step.expression else {
        return false;
    };
    if loop_.init.is_some()
        || loop_.test.is_some()
        || loop_.update.is_some()
        || step.operator.as_str() != "--"
        || !step.prefix
        || !matches!(&step.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(i)
            if crate::analysis::identifier_symbol(i, model.scoping) == Some(length_symbol))
    {
        return false;
    }
    let Expression::UnaryExpression(negate) = &stop.test else {
        return false;
    };
    let Expression::BinaryExpression(limit) = &negate.argument else {
        return false;
    };
    let valid_break = match &stop.consequent {
        Statement::BreakStatement(break_) => break_.label.is_none(),
        _ => {
            matches!(block(&stop.consequent), Some([Statement::BreakStatement(break_)]) if break_.label.is_none())
        }
    };
    if stop.alternate.is_some()
        || negate.operator.as_str() != "!"
        || limit.operator.as_str() != ">="
        || !is_symbol(model, &limit.left, length_symbol)
        || !number(&limit.right, 0.0)
        || !valid_break
    {
        return false;
    }
    let Expression::AssignmentExpression(update) = &update_step.expression else {
        return false;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(update_target) = &update.left else {
        return false;
    };
    let Expression::CallExpression(next) = &update.right else {
        return false;
    };
    let next_arguments = call_arguments(next);
    let [next_stream, next_index, next_tag, next_offset] = next_arguments.as_slice() else {
        return false;
    };
    let Expression::ComputedMemberExpression(indexed) = next_index else {
        return false;
    };
    if crate::analysis::identifier_symbol(update_target, model.scoping) != Some(offset_symbol)
        || !ordered_function(model, &next.callee, next.callee.span().start, 0)
            .is_some_and(|target| target.span == encoder.span)
        || !is_symbol(model, next_stream, stream_symbol)
        || indexed.optional
        || slot_of(model, site, &indexed.object) != Some(0)
        || !is_symbol(model, &indexed.expression, length_symbol)
        || !is_symbol(model, next_tag, tag_symbol)
        || !is_symbol(model, next_offset, offset_symbol)
    {
        return false;
    }
    let Expression::AssignmentExpression(restore) = &restore.expression else {
        return false;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(restore_target) = &restore.left else {
        return false;
    };
    let Expression::CallExpression(number_call) = &restore.right else {
        return false;
    };
    let number_arguments = call_arguments(number_call);
    let [number_value] = number_arguments.as_slice() else {
        return false;
    };
    if crate::analysis::identifier_symbol(restore_target, model.scoping) != Some(length_symbol)
        || !matches!(&number_call.callee, Expression::Identifier(i) if i.name == "Number")
        || !is_symbol(model, number_value, length_symbol)
    {
        return false;
    }
    let Some(observer_next) = statement_call(emit) else {
        return false;
    };
    let Some(observer) =
        member(&observer_next.callee, "next").and_then(|e| data_capture(model, site, e))
    else {
        return false;
    };
    let Some(envelope) = single_argument(observer_next) else {
        return false;
    };
    let Expression::ArrayExpression(envelope) = envelope else {
        return false;
    };
    let envelope_elements: Vec<_> = envelope
        .elements
        .iter()
        .filter_map(ArrayExpressionElement::as_expression)
        .collect();
    let [empty, emitted_tag] = envelope_elements.as_slice() else {
        return false;
    };
    let Expression::ArrayExpression(empty) = empty else {
        return false;
    };
    if !empty.elements.is_empty() || !is_symbol(model, emitted_tag, tag_symbol) {
        return false;
    }
    let Some(complete_call) = statement_call(complete) else {
        return false;
    };
    let Some(complete_receiver) =
        member(&complete_call.callee, "complete").and_then(|e| data_capture(model, site, e))
    else {
        return false;
    };
    let _ = complete_call;
    observer == complete_receiver
        && complete_call.arguments.is_empty()
        && observer != seed_symbol
        && observer != tag_symbol
        && observer != stream_symbol
        && observer != length_symbol
        && observer != offset_symbol
        && tag_symbol != seed_symbol
        && stream_symbol != seed_symbol
        && stream_symbol != tag_symbol
        && length_symbol != seed_symbol
        && length_symbol != tag_symbol
        && length_symbol != stream_symbol
        && offset_symbol != seed_symbol
        && offset_symbol != tag_symbol
        && offset_symbol != stream_symbol
        && offset_symbol != length_symbol
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
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return Vec::new();
    }
    let plain_params = function.params.items.iter().all(|p| {
        p.initializer.is_none()
            && matches!(&p.pattern, BindingPattern::BindingIdentifier(id)
                if id.symbol_id.get().is_some_and(|s| unchanged(model, s)))
    });
    if !plain_params || function.params.items.len() != 1 || f.calls.is_empty() {
        return Vec::new();
    }
    let input = function.params.items.first().and_then(|parameter| {
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        identifier.symbol_id.get()
    });
    let _ = input;
    let mut claims = Vec::new();
    if encode_numeric_array_with_entropy_tag(model, site, body) {
        claims.push(proposal(
            "encodeNumericArrayWithEntropyTag",
            "encoding_entropy_tagged_numeric_array",
            "derives a 4-byte entropy tag from Math.random, derives a per-value stream state from that word and a literal XOR seed, then runs the captured single-origin value encoder over the sequence length and every formal0 element in reverse before emitting the encoded buffer through one captured observer",
            f,
        ));
    }
    claims
}
