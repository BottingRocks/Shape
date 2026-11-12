//! Bounded source protocol for the retained setup-array plus four-formal thunk.
//! No VM store is erased: setup runs before every bound check, the next getter
//! precedes the indexed read, Number coercion follows emission, and catch
//! forwards the error before testing the two setup sentinels in store order.
use super::*;

struct ArrayLoopProtocol {
    sentinels: [Symbol; 2],
    index: Symbol,
    array: Symbol,
    receiver: Symbol,
}

/// The original mixed001 store order stays the first, independent witness.
pub(super) fn array_loop(
    model: &Model<'_, '_>,
    alternate: &Statement<'_>,
    handle: Symbol,
    source: Symbol,
) -> Option<()> {
    prove_loop(
        model,
        alternate,
        handle,
        source,
        SetupOrder::SentinelsThenArray,
    )
}

/// An exact additional source protocol: the retained array refresh executes
/// before both sentinel stores, even for zero elements and before any getter.
pub(super) fn array_first_loop(
    model: &Model<'_, '_>,
    alternate: &Statement<'_>,
    handle: Symbol,
    source: Symbol,
) -> Option<()> {
    prove_loop(
        model,
        alternate,
        handle,
        source,
        SetupOrder::ArrayThenSentinels,
    )
}

#[derive(Clone, Copy)]
enum SetupOrder {
    SentinelsThenArray,
    ArrayThenSentinels,
}

fn prove_loop(
    model: &Model<'_, '_>,
    alternate: &Statement<'_>,
    handle: Symbol,
    source: Symbol,
    order: SetupOrder,
) -> Option<()> {
    let [Statement::VariableDeclaration(sentinels), Statement::TryStatement(try_)] =
        block(alternate)?
    else {
        return None;
    };
    let [first, second] = sentinels.declarations.as_slice() else {
        return None;
    };
    if first.init.is_some() || second.init.is_some() || try_.finalizer.is_some() {
        return None;
    }
    let sentinel_a = local(model, first)?;
    let sentinel_b = local(model, second)?;
    let [init, temporary, Statement::DoWhileStatement(loop_), complete] =
        try_.block.body.as_slice()
    else {
        return None;
    };
    one_variable(init)?;
    let index = index_zero(model, init)?;
    let Statement::VariableDeclaration(temporaries) = temporary else {
        return None;
    };
    let [array, receiver] = temporaries.declarations.as_slice() else {
        return None;
    };
    if array.init.is_some() || receiver.init.is_some() {
        return None;
    }
    let a = local(model, array)?;
    let b = local(model, receiver)?;
    let [first, second, third] = block(&loop_.body)? else {
        return None;
    };
    // The enum records the executed source order. No node is dropped or
    // reordered: both alternatives consume precisely these three statements.
    let (mark_a, mark_b, refresh) = match order {
        SetupOrder::SentinelsThenArray => (first, second, third),
        SetupOrder::ArrayThenSentinels => (second, third, first),
    };

    let (array, receiver) = if assignment_value_to_identifier(model, refresh, a)
        .is_some_and(|value| identifier(model, value) == Some(source))
    {
        (a, b)
    } else if assignment_value_to_identifier(model, refresh, b)
        .is_some_and(|value| identifier(model, value) == Some(source))
    {
        (b, a)
    } else {
        return None;
    };
    // The complete extension follows executed stores, independently of the
    // neutral declaration order. Canonical witnesses retain their own domain.
    let (sentinel_a, sentinel_b) = if assignment_value_to_identifier(model, mark_a, sentinel_a)
        .is_some()
        && assignment_value_to_identifier(model, mark_b, sentinel_b).is_some()
    {
        (sentinel_a, sentinel_b)
    } else if assignment_value_to_identifier(model, mark_a, sentinel_b).is_some()
        && assignment_value_to_identifier(model, mark_b, sentinel_a).is_some()
    {
        (sentinel_b, sentinel_a)
    } else {
        return None;
    };
    let protocol = ArrayLoopProtocol {
        sentinels: [sentinel_a, sentinel_b],
        index,
        array,
        receiver,
    };
    let ArrayLoopProtocol {
        sentinels: [sentinel_a, sentinel_b],
        index,
        array,
        receiver,
    } = protocol;
    let locals = [sentinel_a, sentinel_b, index, array, receiver];

    if !distinct(&locals) || locals.contains(&handle) || locals.contains(&source) {
        return None;
    }
    for (temporary, references) in locals.into_iter().zip([2, 2, 4, 2, 3]) {
        if model.scoping.get_resolved_references(temporary).count() != references
            || model.binding_writes(temporary) != 1
            || !alternate
                .span()
                .contains_inclusive(model.scoping.symbol_span(temporary))
            || !model
                .scoping
                .get_resolved_references(temporary)
                .all(|reference| {
                    alternate
                        .span()
                        .contains_inclusive(model.nodes.get_node(reference.node_id()).span())
                })
        {
            return None;
        }
    }
    if !matches!(
        assignment_value_to_identifier(model, mark_a, sentinel_a)?.without_parentheses(),
        Expression::NumericLiteral(_)
    ) || !matches!(
        assignment_value_to_identifier(model, mark_b, sentinel_b)?.without_parentheses(),
        Expression::NumericLiteral(_)
    ) {
        return None;
    }
    if identifier(
        model,
        assignment_value_to_identifier(model, refresh, array)?,
    ) != Some(source)
    {
        return None;
    }
    let (bound, effects) = logical(&loop_.test, "&&")?;
    let (left, right) = binary(bound, "<")?;
    if identifier(model, left) != Some(index) || !member_of(model, right, array, "length") {
        return None;
    }
    let Expression::SequenceExpression(effects) = effects.without_parentheses() else {
        return None;
    };
    let [capture_receiver, invoke, increment, truth] = effects.expressions.as_slice() else {
        return None;
    };
    if identifier(
        model,
        assigned_expression(model, capture_receiver, receiver)?,
    ) != Some(handle)
        || !matches!(truth.without_parentheses(), Expression::BooleanLiteral(b) if b.value)
    {
        return None;
    }
    let invocation = plain_call(invoke)?;
    let Expression::ArrowFunctionExpression(arrow) = invocation.callee.without_parentheses() else {
        return None;
    };
    if arrow.r#async || arrow.expression || !arrow.body.directives.is_empty() {
        return None;
    }
    let params = plain_arrow_params(model, arrow)?;
    let [receiver_param, method_param, source_param, index_param] = params.as_slice() else {
        return None;
    };
    if !distinct(&params)
        || params.iter().any(|p| {
            locals.contains(p)
                || *p == source
                || *p == handle
                || model.scoping.get_resolved_references(*p).count() != 1
        })
    {
        return None;
    }
    let args = exact_arguments(invocation, 4)?;
    if identifier(model, args[0]) != Some(receiver)
        || !member_of(model, args[1], receiver, "next")
        || identifier(model, args[2]) != Some(source)
        || identifier(model, args[3]) != Some(index)
    {
        return None;
    }
    let [emit] = arrow.body.statements.as_slice() else {
        return None;
    };
    let args = observer_call(model, expression_statement(emit)?, *method_param, "call", 2)?;
    let Expression::ComputedMemberExpression(element) = args[1].without_parentheses() else {
        return None;
    };
    if identifier(model, args[0]) != Some(*receiver_param)
        || element.optional
        || identifier(model, &element.object) != Some(*source_param)
        || identifier(model, &element.expression) != Some(*index_param)
    {
        return None;
    }
    let (number, one) = binary(assigned_expression(model, increment, index)?, "+")?;
    let number = plain_call(number)?;
    if !is_number(one, 1.0)
        || !unbound(model, &number.callee, "Number")
        || identifier(model, exact_arguments(number, 1)?[0]) != Some(index)
    {
        return None;
    }
    completes(model, complete, handle)?;
    let handler = try_.handler.as_ref()?;
    let error = plain_binding(model, &handler.param.as_ref()?.pattern)?;
    if !immutable(model, error) {
        return None;
    }
    let [forward, check_a, check_b] = handler.body.body.as_slice() else {
        return None;
    };
    let args = observer_call(model, expression_statement(forward)?, handle, "error", 1)?;
    if identifier(model, args[0]) != Some(error) {
        return None;
    }
    sentinel_failure(
        model,
        check_a,
        sentinel_a,
        "Cannot read properties of undefined (reading '0')",
    )?;
    sentinel_failure(
        model,
        check_b,
        sentinel_b,
        "Cannot read properties of undefined (reading 'undefined')",
    )
}

#[cfg(test)]
#[path = "observable_factory_mixed_tests.rs"]
mod tests;
