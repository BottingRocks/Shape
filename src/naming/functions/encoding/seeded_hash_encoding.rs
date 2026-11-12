use super::*;

use crate::naming::support::seeded_flow::*;

// ---------------------------------------------------------------------------
// `emitSeededHashEncoding`: random nonce plus exact seeded hash updates.
// ---------------------------------------------------------------------------
fn seed_base(model: &Model<'_, '_>, e: &Expression<'_>, seed: Symbol) -> bool {
    if symbol(model, e) == Some(seed) {
        return true;
    }
    let Expression::BinaryExpression(or) = e else {
        return false;
    };
    or.operator.as_str() == "|" && number(&or.right, 0.0) && symbol(model, &or.left) == Some(seed)
}

fn random_number(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let base = match e {
        Expression::BinaryExpression(or) if or.operator.as_str() == "|" => {
            if !number(&or.right, 0.0) {
                return false;
            }
            &or.left
        }
        _ => e,
    };
    let Expression::BinaryExpression(mul) = base else {
        return false;
    };
    let Some(random) = call(&mul.left) else {
        return false;
    };
    mul.operator.as_str() == "*"
        && number(&mul.right, 4294967296.0)
        && is_global(model, &random.callee, &["Math", "random"])
        && arity(random, 0).is_some()
}

fn nonce_element(model: &Model<'_, '_>, e: &Expression<'_>, seed: Symbol, slot: usize) -> bool {
    match slot {
        0 => {
            let Expression::BinaryExpression(shift) = e else {
                return false;
            };
            shift.operator.as_str() == ">>>"
                && seed_base(model, &shift.left, seed)
                && number(&shift.right, 24.0)
        }
        1 | 2 => {
            let Expression::BinaryExpression(mask) = e else {
                return false;
            };
            let shift_value = if slot == 1 { 16.0 } else { 8.0 };
            let Expression::BinaryExpression(shift) = &mask.left else {
                return false;
            };
            mask.operator.as_str() == "&"
                && number(&mask.right, 255.0)
                && shift.operator.as_str() == ">>"
                && seed_base(model, &shift.left, seed)
                && number(&shift.right, shift_value)
        }
        3 => {
            let Expression::BinaryExpression(mask) = e else {
                return false;
            };
            mask.operator.as_str() == "&"
                && seed_base(model, &mask.left, seed)
                && number(&mask.right, 255.0)
        }
        _ => false,
    }
}

fn nonce_declaration(
    model: &Model<'_, '_>,
    v: &VariableDeclarator<'_>,
    seed: Symbol,
) -> Option<Symbol> {
    let bytes = plain_local(model, v)?;
    let Expression::ArrayExpression(array) = v.init.as_ref()? else {
        return None;
    };
    if array.elements.len() != 4 {
        return None;
    }
    let mut elements = Vec::new();
    for element in &array.elements {
        elements.push(element.as_expression()?);
    }
    elements
        .iter()
        .enumerate()
        .all(|(slot, e)| nonce_element(model, e, seed, slot))
        .then_some(bytes)
}

fn hash_seeded(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    seed: Symbol,
) -> Option<FunctionSite> {
    let c = call(e)?;
    let target = callable_capture(model, site, &c.callee)?;
    let args = arity(c, 1)?;
    let Expression::BinaryExpression(xor) = args[0] else {
        return None;
    };
    (xor.operator.as_str() == "^"
        && seed_base(model, &xor.left, seed)
        && matches!(&xor.right, Expression::NumericLiteral(_)))
    .then_some(target)
}

// A hash-or-mix call over the established nonce bytes. The first argument is
// the seeded hash callable VALUE; it is never treated as a host API by name.
fn mix_call<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    c: &'a CallExpression<'a>,
    hash: Symbol,
    bytes: Symbol,
) -> Option<(FunctionSite, &'a Expression<'a>, &'a Expression<'a>)> {
    let target = callable_capture(model, site, &c.callee)?;
    let args = arity(c, 4)?;
    (symbol(model, args[0]) == Some(hash) && symbol(model, args[2]) == Some(bytes))
        .then_some((target, args[1], args[3]))
}

fn formal_index(model: &Model<'_, '_>, e: &Expression<'_>, site: FunctionSite) -> Option<u32> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    if m.optional || model.site_parameter_slot(&m.object, site) != Some(0) {
        return None;
    }
    match &m.expression {
        Expression::NumericLiteral(n)
            if n.value >= 0.0 && n.value <= 4.0 && n.value.fract() == 0.0 =>
        {
            Some(n.value as u32)
        }
        _ => None,
    }
}

fn empty_array(e: &Expression<'_>) -> bool {
    matches!(e, Expression::ArrayExpression(a) if a.elements.is_empty())
}

/// The shared observer tail. `first` is `None` for the literal `[]` payload and
/// `Some(symbol)` when the owned empty payload array is a local declared by the
/// proof (the descending fixed-field variant).
fn observer_end(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    start: usize,
    bytes: Symbol,
    first: Option<Symbol>,
) -> Option<()> {
    let rest = body.statements.get(start..)?;
    let [next, complete] = rest else {
        return None;
    };
    let next = statement_call(next)?;
    let observer = data_capture(model, site, member(&next.callee, "next")?)?;
    let next_args = arity(next, 1)?;
    let Expression::ArrayExpression(payload) = next_args[0] else {
        return None;
    };
    let [first_arg, second] = payload.elements.as_slice() else {
        return None;
    };
    let (Some(first_arg), Some(second)) = (first_arg.as_expression(), second.as_expression())
    else {
        return None;
    };
    let first_ok = match first {
        None => empty_array(first_arg),
        Some(payload_local) => symbol(model, first_arg) == Some(payload_local),
    };
    let complete = statement_call(complete)?;
    if !first_ok || symbol(model, second) != Some(bytes) || arity(complete, 0).is_none() {
        return None;
    }
    (symbol(model, member(&complete.callee, "complete")?) == Some(observer)).then_some(())
}

fn emit_fixed_hash(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [random, nonce, hash_stmt, h_decl, a, b, c, d, next, complete] = body.statements.as_slice()
    else {
        return None;
    };
    let seed = plain_local(model, one_variable(random)?)?;
    if !random_number(model, one_variable(random)?.init.as_ref()?) {
        return None;
    }
    let bytes = nonce_declaration(model, one_variable(nonce)?, seed)?;
    let (hash, hash_target) = match (hash_stmt, one_variable(hash_stmt)) {
        (Statement::ExpressionStatement(_), _) => {
            let assign = assignment(hash_stmt)?;
            if assign_symbol(model, assign) != Some(seed) {
                return None;
            }
            (seed, hash_seeded(model, site, &assign.right, seed)?)
        }
        (_, Some(v)) => {
            let hash = plain_local(model, v)?;
            (hash, hash_seeded(model, site, v.init.as_ref()?, seed)?)
        }
        _ => return None,
    };
    let h = plain_local(model, one_variable(h_decl)?)?;
    let h_call = call(one_variable(h_decl)?.init.as_ref()?)?;
    let (mix_target, second, fourth) = mix_call(model, site, h_call, hash, bytes)?;
    if mix_target.span == hash_target.span || !number(fourth, 3.0) {
        return None;
    }
    let mut indices = vec![formal_index(model, second, site)?];
    for (update, is_assignment) in [(a, true), (b, true), (c, true), (d, false)] {
        let Statement::ExpressionStatement(statement) = update else {
            return None;
        };
        let call_expr = match &statement.expression {
            Expression::AssignmentExpression(a) => {
                if !is_assignment || assign_symbol(model, a) != Some(h) {
                    return None;
                }
                call(&a.right)?
            }
            e => {
                if is_assignment {
                    return None;
                }
                call(e)?
            }
        };
        let (target, second, fourth) = mix_call(model, site, call_expr, hash, bytes)?;
        if target.span != mix_target.span || symbol(model, fourth) != Some(h) {
            return None;
        }
        indices.push(formal_index(model, second, site)?);
    }
    indices.sort_unstable();
    if indices != [0, 1, 2, 3, 4] {
        return None;
    }
    let _ = (next, complete);
    observer_end(model, site, body, 8, bytes, None)
}

/// Inner-mix fixed-field variant: the same seeded header, hash factory, nonce
/// array and four-argument mixer chain as `emit_fixed_hash`, but one update
/// folds its cursor field into the accumulator argument as a bounded nested call
/// on the SAME mixer/hash/nonce chain instead of a flat accumulator step. The
/// nested call is normalised in true inner-to-outer evaluation order, and the
/// declared formal indices must still cover exactly `{0,1,2,3,4}`.
fn emit_fixed_hash_inner_mix(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [random, nonce, hash_stmt, h_decl, a, b, c, next, complete] = body.statements.as_slice()
    else {
        return None;
    };
    let seed = plain_local(model, one_variable(random)?)?;
    if !random_number(model, one_variable(random)?.init.as_ref()?) {
        return None;
    }
    let bytes = nonce_declaration(model, one_variable(nonce)?, seed)?;
    let (hash, hash_target) = match (hash_stmt, one_variable(hash_stmt)) {
        (Statement::ExpressionStatement(_), _) => {
            let assign = assignment(hash_stmt)?;
            if assign_symbol(model, assign) != Some(seed) {
                return None;
            }
            (seed, hash_seeded(model, site, &assign.right, seed)?)
        }
        (_, Some(v)) => {
            let hash = plain_local(model, v)?;
            (hash, hash_seeded(model, site, v.init.as_ref()?, seed)?)
        }
        _ => return None,
    };
    let h = plain_local(model, one_variable(h_decl)?)?;
    let h_call = call(one_variable(h_decl)?.init.as_ref()?)?;
    let (mix_target, second, fourth) = mix_call(model, site, h_call, hash, bytes)?;
    if mix_target.span == hash_target.span || !number(fourth, 3.0) {
        return None;
    }
    let mut indices = vec![formal_index(model, second, site)?];
    for (update, is_assignment) in [(a, true), (b, true), (c, false)] {
        let Statement::ExpressionStatement(statement) = update else {
            return None;
        };
        let call_expr = match &statement.expression {
            Expression::AssignmentExpression(assign) => {
                if !is_assignment || assign_symbol(model, assign) != Some(h) {
                    return None;
                }
                call(&assign.right)?
            }
            e => {
                if is_assignment {
                    return None;
                }
                call(e)?
            }
        };
        let (target, second, fourth) = mix_call(model, site, call_expr, hash, bytes)?;
        if target.span != mix_target.span {
            return None;
        }
        if symbol(model, fourth) == Some(h) {
            indices.push(formal_index(model, second, site)?);
        } else {
            let (inner_target, inner_second, inner_fourth) =
                mix_call(model, site, call(fourth)?, hash, bytes)?;
            if inner_target.span != mix_target.span || symbol(model, inner_fourth) != Some(h) {
                return None;
            }
            indices.push(formal_index(model, inner_second, site)?);
            indices.push(formal_index(model, second, site)?);
        }
    }
    indices.sort_unstable();
    if indices != [0, 1, 2, 3, 4] {
        return None;
    }
    let _ = (next, complete);
    observer_end(model, site, body, 7, bytes, None)
}

/// Compact fixed-field variant: the initial accumulator folds formal slot 3
/// inside the slot-4 call, the next statement folds slot 0, and the final
/// discarded call folds slot 2 inside slot 1. This is intentionally a separate
/// finite arm from `emit_fixed_hash_inner_mix`: there are exactly eight
/// top-level statements, and the five formal slots must be recovered in this
/// exact order.
fn emit_fixed_hash_compact_inner_mix(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [random, nonce, hash_stmt, acc_decl, first_update, final_nested_mix, next, complete] =
        body.statements.as_slice()
    else {
        return None;
    };
    let seed = plain_local(model, one_variable(random)?)?;
    if !random_number(model, one_variable(random)?.init.as_ref()?) {
        return None;
    }
    let bytes = nonce_declaration(model, one_variable(nonce)?, seed)?;
    let (hash, hash_target) = hash_declaration(model, site, hash_stmt, seed)?;
    let acc = plain_local(model, one_variable(acc_decl)?)?;
    let initial = call(one_variable(acc_decl)?.init.as_ref()?)?;
    let (mix_target, initial_field, initial_fourth) = mix_call(model, site, initial, hash, bytes)?;
    let initial_inner = mix_call(model, site, call(initial_fourth)?, hash, bytes)?;
    if mix_target.span == hash_target.span
        || initial_inner.0.span != mix_target.span
        || !number(initial_inner.2, 3.0)
    {
        return None;
    }
    let mut indices = vec![
        formal_index(model, initial_field, site)?,
        formal_index(model, initial_inner.1, site)?,
    ];

    let first = assignment(first_update)?;
    if assign_symbol(model, first) != Some(acc) {
        return None;
    }
    let first_mix = mix_call(model, site, call(&first.right)?, hash, bytes)?;
    if first_mix.0.span != mix_target.span || symbol(model, first_mix.2) != Some(acc) {
        return None;
    }
    indices.push(formal_index(model, first_mix.1, site)?);

    let final_mix = statement_call(final_nested_mix)?;
    let final_outer = mix_call(model, site, final_mix, hash, bytes)?;
    let final_inner = mix_call(model, site, call(final_outer.2)?, hash, bytes)?;
    if final_outer.0.span != mix_target.span
        || final_inner.0.span != mix_target.span
        || symbol(model, final_inner.2) != Some(acc)
    {
        return None;
    }
    indices.push(formal_index(model, final_inner.1, site)?);
    indices.push(formal_index(model, final_outer.1, site)?);
    indices.sort_unstable();
    if indices != [0, 1, 2, 3, 4] {
        return None;
    }
    let _ = (next, complete);
    observer_end(model, site, body, 6, bytes, None)
}

/// A seeded hash declaration: either `let <local> = <factory>(...)` or the
/// reseed assignment `<seed> = <factory>(...)`. The factory is never treated as
/// a host API by name; only the exact `seed ^ const` hash shape is accepted.
fn hash_declaration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    stmt: &Statement<'_>,
    seed: Symbol,
) -> Option<(Symbol, FunctionSite)> {
    match one_variable(stmt) {
        Some(v) => {
            let hash = plain_local(model, v)?;
            Some((hash, hash_seeded(model, site, v.init.as_ref()?, seed)?))
        }
        None => {
            let assign = assignment(stmt)?;
            if assign_symbol(model, assign) != Some(seed) {
                return None;
            }
            Some((seed, hash_seeded(model, site, &assign.right, seed)?))
        }
    }
}

/// The length local declared as `let <local> = <formal>.length`: the copy of the
/// sole plain parameter's `length`. Any other initialiser is refused.
fn length_declaration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    stmt: &Statement<'_>,
) -> Option<Symbol> {
    let v = one_variable(stmt)?;
    let length = plain_local(model, v)?;
    let formal = member(v.init.as_ref()?, "length")?;
    (model.site_parameter_slot(formal, site) == Some(0)).then_some(length)
}

fn emit_loop_hash(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [random, nonce, first_decl, second_decl, i_decl, h_decl, Statement::ForStatement(loop_), next, complete] =
        body.statements.as_slice()
    else {
        return None;
    };
    let seed = plain_local(model, one_variable(random)?)?;
    if !random_number(model, one_variable(random)?.init.as_ref()?) {
        return None;
    }
    let bytes = nonce_declaration(model, one_variable(nonce)?, seed)?;
    // Exactly two typed declaration orders are accepted for the length local and
    // the seeded hash callable: hash-then-length (the original spelling) and the
    // bounded length-then-hash order. The statements are never shuffled and no
    // executed source is rewritten.
    let (hash, hash_target, length) = if let (Some((hash, target)), Some(length)) = (
        hash_declaration(model, site, first_decl, seed),
        length_declaration(model, site, second_decl),
    ) {
        (hash, target, length)
    } else {
        let length = length_declaration(model, site, first_decl)?;
        let (hash, target) = hash_declaration(model, site, second_decl, seed)?;
        (hash, target, length)
    };
    if plain_local(model, one_variable(i_decl)?)? == length
        || !number(one_variable(i_decl)?.init.as_ref()?, 0.0)
    {
        return None;
    }
    let i = plain_local(model, one_variable(i_decl)?)?;
    let h = plain_local(model, one_variable(h_decl)?)?;
    let h_call = call(one_variable(h_decl)?.init.as_ref()?)?;
    let (mix_target, second, fourth) = mix_call(model, site, h_call, hash, bytes)?;
    if mix_target.span == hash_target.span
        || symbol(model, second) != Some(length)
        || !number(fourth, 3.0)
    {
        return None;
    }
    let Some(test) = &loop_.test else {
        return None;
    };
    let Expression::BinaryExpression(less) = test else {
        return None;
    };
    if loop_.init.is_some()
        || less.operator.as_str() != "<"
        || symbol(model, &less.left) != Some(i)
        || symbol(model, &less.right) != Some(length)
        || loop_
            .update
            .as_ref()
            .and_then(|e| update_symbol(model, e, false, "++"))
            != Some(i)
    {
        return None;
    }
    let Statement::BlockStatement(block) = &loop_.body else {
        return None;
    };
    let [update] = block.body.as_slice() else {
        return None;
    };
    let update = assignment(update)?;
    if assign_symbol(model, update) != Some(h) {
        return None;
    }
    let update_call = call(&update.right)?;
    let (target, second, fourth) = mix_call(model, site, update_call, hash, bytes)?;
    let Expression::ComputedMemberExpression(element) = second else {
        return None;
    };
    if target.span != mix_target.span
        || element.optional
        || model.site_parameter_slot(&element.object, site) != Some(0)
        || symbol(model, &element.expression) != Some(i)
        || symbol(model, fourth) != Some(h)
    {
        return None;
    }
    let _ = (next, complete);
    observer_end(model, site, body, 7, bytes, None)
}

/// `Math.random() * 4294967296` without the `| 0` wrap. The descending bodies
/// keep the fractional seed and apply `(seed | 0)` only inside the nonce bytes.
fn random_fraction(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(mul) = e else {
        return false;
    };
    if mul.operator.as_str() != "*" || !number(&mul.right, 4294967296.0) {
        return false;
    }
    let Some(random) = call(&mul.left) else {
        return false;
    };
    is_global(model, &random.callee, &["Math", "random"]) && arity(random, 0).is_some()
}

/// The seed base as `(seed | 0)`, the form used by the descending bodies. The
/// already-proved fixed/loop variants accept either base; this strict form keeps
/// the new variant disjoint from the existing fixtures.
fn zeroed_seed(model: &Model<'_, '_>, e: &Expression<'_>, seed: Symbol) -> bool {
    let Expression::BinaryExpression(or) = e else {
        return false;
    };
    or.operator.as_str() == "|" && number(&or.right, 0.0) && symbol(model, &or.left) == Some(seed)
}

fn zeroed_nonce_element(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    seed: Symbol,
    slot: usize,
) -> bool {
    match slot {
        0 => {
            let Expression::BinaryExpression(shift) = e else {
                return false;
            };
            shift.operator.as_str() == ">>>"
                && zeroed_seed(model, &shift.left, seed)
                && number(&shift.right, 24.0)
        }
        1 | 2 => {
            let Expression::BinaryExpression(mask) = e else {
                return false;
            };
            let shift_value = if slot == 1 { 16.0 } else { 8.0 };
            let Expression::BinaryExpression(shift) = &mask.left else {
                return false;
            };
            mask.operator.as_str() == "&"
                && number(&mask.right, 255.0)
                && shift.operator.as_str() == ">>"
                && zeroed_seed(model, &shift.left, seed)
                && number(&shift.right, shift_value)
        }
        3 => {
            let Expression::BinaryExpression(mask) = e else {
                return false;
            };
            mask.operator.as_str() == "&"
                && zeroed_seed(model, &mask.left, seed)
                && number(&mask.right, 255.0)
        }
        _ => false,
    }
}

fn zeroed_nonce_declaration(
    model: &Model<'_, '_>,
    v: &VariableDeclarator<'_>,
    seed: Symbol,
) -> Option<Symbol> {
    let bytes = plain_local(model, v)?;
    let Expression::ArrayExpression(array) = v.init.as_ref()? else {
        return None;
    };
    if array.elements.len() != 4 {
        return None;
    }
    let mut elements = Vec::new();
    for element in &array.elements {
        elements.push(element.as_expression()?);
    }
    elements
        .iter()
        .enumerate()
        .all(|(slot, e)| zeroed_nonce_element(model, e, seed, slot))
        .then_some(bytes)
}

/// A numeric literal selected member: `captured[<number>]`. The index value is
/// deliberately opaque; only the fact that the object is one captured binding
/// is proved.
fn numeric_index<'a>(e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && matches!(&m.expression, Expression::NumericLiteral(_))).then_some(&m.object)
}

/// An explicitly coerced random word. Keep this local to the descending grammar:
/// other emitter families retain their existing nonce conventions.
fn descending_integer_random(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(coerce) = e else {
        return false;
    };
    matches!(coerce.operator.as_str(), "|" | ">>>")
        && number(&coerce.right, 0.0)
        && random_fraction(model, &coerce.left)
}

fn descending_word(model: &Model<'_, '_>, e: &Expression<'_>, seed: Symbol) -> bool {
    if seed_base(model, e, seed) {
        return true;
    }
    matches!(e, Expression::BinaryExpression(shift)
        if shift.operator.as_str() == ">>>" && number(&shift.right, 0.0)
            && symbol(model, &shift.left) == Some(seed))
}

fn descending_integer_nonce(
    model: &Model<'_, '_>,
    v: &VariableDeclarator<'_>,
    seed: Symbol,
) -> Option<Symbol> {
    let bytes = plain_local(model, v)?;
    let Expression::ArrayExpression(array) = v.init.as_ref()? else {
        return None;
    };
    if array.elements.len() != 4 {
        return None;
    }
    for (slot, element) in array.elements.iter().enumerate() {
        let e = element.as_expression()?;
        let valid = match (slot, e) {
            (0, Expression::BinaryExpression(shift)) => {
                shift.operator.as_str() == ">>>"
                    && descending_word(model, &shift.left, seed)
                    && number(&shift.right, 24.0)
            }
            (1 | 2, Expression::BinaryExpression(mask)) => {
                let Expression::BinaryExpression(shift) = &mask.left else {
                    return None;
                };
                mask.operator.as_str() == "&"
                    && number(&mask.right, 255.0)
                    && matches!(shift.operator.as_str(), ">>" | ">>>")
                    && descending_word(model, &shift.left, seed)
                    && number(&shift.right, if slot == 1 { 16.0 } else { 8.0 })
            }
            (3, Expression::BinaryExpression(mask)) => {
                mask.operator.as_str() == "&"
                    && number(&mask.right, 255.0)
                    && descending_word(model, &mask.left, seed)
            }
            _ => false,
        };
        if !valid {
            return None;
        }
    }
    Some(bytes)
}

/// Descending fixed-field variant: exact 4-byte header from a random seed,
/// one owned empty payload local, a length-local descending `while` over the
/// formal, then the shared observer tail.
fn emit_descending_hash(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    facts: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    let statements = body.statements.as_slice();
    let (complete, rest) = statements.split_last()?;
    let (next, prefix) = rest.split_last()?;
    if prefix.len() != 7 {
        return None;
    }
    let mut payload = None;
    let mut remaining = Vec::new();
    for statement in prefix {
        if let Some(variable) = one_variable(statement) {
            if variable.init.as_ref().is_some_and(empty_array) {
                if payload.replace(plain_local(model, variable)?).is_some() {
                    return None;
                }
                continue;
            }
        }
        remaining.push(statement);
    }
    if remaining.len() != 6 {
        return None;
    }
    let random = remaining[0];
    let nonce = remaining[1];
    let hash_stmt = remaining[2];
    let length_decl = remaining[3];
    let acc_decl = remaining[4];
    let Statement::WhileStatement(loop_) = remaining[5] else {
        return None;
    };
    let seed = plain_local(model, one_variable(random)?)?;
    let random_value = one_variable(random)?.init.as_ref()?;
    let integer = descending_integer_random(model, random_value);
    if !integer && !random_fraction(model, random_value) {
        return None;
    }
    let bytes = if integer {
        // The integer vector witnesses keep the original nonce word separate
        // from the seeded callable. Do not absorb the reseed-in-place neighbor.
        if !unchanged(model, seed) {
            return None;
        }
        descending_integer_nonce(model, one_variable(nonce)?, seed)?
    } else {
        zeroed_nonce_declaration(model, one_variable(nonce)?, seed)?
    };
    let (hash, hash_target) = match (hash_stmt, one_variable(hash_stmt)) {
        (Statement::ExpressionStatement(_), _) => {
            let assign = assignment(hash_stmt)?;
            if assign_symbol(model, assign) != Some(seed) {
                return None;
            }
            (seed, hash_seeded(model, site, &assign.right, seed)?)
        }
        (_, Some(v)) => {
            let hash = plain_local(model, v)?;
            let target = if integer {
                if hash == seed || !unchanged(model, hash) {
                    return None;
                }
                let factory = call(v.init.as_ref()?)?;
                let args = arity(factory, 1)?;
                let Expression::BinaryExpression(xor) = args[0] else {
                    return None;
                };
                if xor.operator.as_str() != "^"
                    || !descending_word(model, &xor.left, seed)
                    || !matches!(&xor.right, Expression::NumericLiteral(_))
                {
                    return None;
                }
                callable_capture(model, site, &factory.callee)?
            } else {
                hash_seeded(model, site, v.init.as_ref()?, seed)?
            };
            (hash, target)
        }
        _ => return None,
    };
    let length = plain_local(model, one_variable(length_decl)?)?;
    let Some(formal) = member(one_variable(length_decl)?.init.as_ref()?, "length") else {
        return None;
    };
    if model.site_parameter_slot(formal, site) != Some(0) {
        return None;
    }
    let acc = plain_local(model, one_variable(acc_decl)?)?;
    let acc_call = call(one_variable(acc_decl)?.init.as_ref()?)?;
    let (mix_target, second, fourth) = mix_call(model, site, acc_call, hash, bytes)?;
    // The initial digest field is either the length local or the same
    // `formal.length` member expression the length local was copied from.
    let length_operand = symbol(model, second) == Some(length)
        || member(second, "length")
            .is_some_and(|receiver| symbol(model, receiver) == symbol(model, formal));
    if mix_target.span == hash_target.span || !length_operand || !number(fourth, 3.0) {
        return None;
    }
    let test = &loop_.test;
    let Expression::BinaryExpression(ge) = test else {
        return None;
    };
    let Expression::BinaryExpression(distance) = &ge.left else {
        return None;
    };
    if ge.operator.as_str() != ">="
        || !number(&ge.right, 0.0)
        || distance.operator.as_str() != "-"
        || symbol(model, &distance.left) != Some(length)
        || !number(&distance.right, 1.0)
    {
        return None;
    }
    let Statement::BlockStatement(block) = &loop_.body else {
        return None;
    };
    let [step, decrement] = block.body.as_slice() else {
        return None;
    };
    let step = assignment(step)?;
    if assign_symbol(model, step) != Some(acc) {
        return None;
    }
    let (step_target, operand, step_fourth) =
        mix_call(model, site, call(&step.right)?, hash, bytes)?;
    if step_target.span != mix_target.span || symbol(model, step_fourth) != Some(acc) {
        return None;
    }
    let Expression::ComputedMemberExpression(element) = operand else {
        return None;
    };
    let Expression::BinaryExpression(index) = &element.expression else {
        return None;
    };
    if element.optional
        || model.site_parameter_slot(&element.object, site) != Some(0)
        || index.operator.as_str() != "-"
        || symbol(model, &index.left) != Some(length)
        || !number(&index.right, 1.0)
    {
        return None;
    }
    if update_symbol(model, expression(decrement)?, true, "--") != Some(length) {
        return None;
    }
    let _ = (next, complete);
    observer_end(model, site, body, 7, bytes, Some(payload?))?;
    // Only the new integer descending extension defers to an existing frame
    // proof on this exact owner. Reuse the seeded-signal-frame pure evaluator, which does not
    // call the evaluate_additions stage; no parallel shadow proof or name
    // history is needed. Fractional and other Hash families keep their prior
    // agreement/governance behavior.
    if integer
        && crate::naming::functions::observables::seeded_signal_frame::evaluate(model, site, facts, proposals)
            .iter()
            .any(|claim| {
                claim.name == "emitSeededSignalFrame"
                    && claim.evaluator == "signals_seeded_signal_frame"
            })
    {
        return None;
    }
    Some(())
}

fn audio_payload(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<Symbol> {
    let variable = one_variable(statement)?;
    let payload = plain_local(model, variable)?;
    let channel = call(variable.init.as_ref()?)?;
    let receiver = member(&channel.callee, "getChannelData")?;
    if model.site_parameter_slot(receiver, site) != Some(0) {
        return None;
    }
    let args = arity(channel, 1)?;
    number(args[0], 0.0).then_some(payload)
}

fn array_from_payload(
    model: &Model<'_, '_>,
    payload: Symbol,
    statement: &Statement<'_>,
) -> Option<()> {
    let assignment = assignment(statement)?;
    if assign_symbol(model, assignment) != Some(payload) {
        return None;
    }
    let from = call(&assignment.right)?;
    if !is_global(model, &from.callee, &["Array", "from"]) {
        return None;
    }
    let args = arity(from, 1)?;
    (symbol(model, args[0]) == Some(payload)).then_some(())
}

fn reverse_audio_loop(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    payload: Symbol,
    length: Symbol,
    acc: Symbol,
    hash: Symbol,
    bytes: Symbol,
    mix_target: FunctionSite,
) -> Option<()> {
    let Statement::WhileStatement(loop_) = statement else {
        return None;
    };
    let Expression::BinaryExpression(ge) = &loop_.test else {
        return None;
    };
    let Expression::BinaryExpression(distance) = &ge.left else {
        return None;
    };
    if ge.operator.as_str() != ">="
        || !number(&ge.right, 0.0)
        || distance.operator.as_str() != "-"
        || symbol(model, &distance.left) != Some(length)
        || !number(&distance.right, 1.0)
    {
        return None;
    }
    let Statement::BlockStatement(block) = &loop_.body else {
        return None;
    };
    let [step, decrement] = block.body.as_slice() else {
        return None;
    };
    let step = assignment(step)?;
    if assign_symbol(model, step) != Some(acc) {
        return None;
    }
    let (step_target, operand, step_fourth) =
        mix_call(model, site, call(&step.right)?, hash, bytes)?;
    let Expression::ComputedMemberExpression(element) = operand else {
        return None;
    };
    let Expression::BinaryExpression(index) = &element.expression else {
        return None;
    };
    if step_target.span != mix_target.span
        || element.optional
        || symbol(model, &element.object) != Some(payload)
        || index.operator.as_str() != "-"
        || symbol(model, &index.left) != Some(length)
        || !number(&index.right, 1.0)
        || symbol(model, step_fourth) != Some(acc)
    {
        return None;
    }
    (update_symbol(model, expression(decrement)?, true, "--") == Some(length)).then_some(())
}

/// Audio-channel reverse-while variant with the captured numeric field in the
/// initial accumulator and an owned empty payload local in the observer tail.
fn emit_audio_reverse_hash(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [payload_stmt, random, nonce, payload_decl, hash_stmt, acc_decl, convert, length_decl, length_mix, loop_stmt, next, complete] =
        body.statements.as_slice()
    else {
        return None;
    };
    let payload = audio_payload(model, site, payload_stmt)?;
    let seed = plain_local(model, one_variable(random)?)?;
    if !random_number(model, one_variable(random)?.init.as_ref()?) {
        return None;
    }
    let bytes = nonce_declaration(model, one_variable(nonce)?, seed)?;
    let payload_local = plain_local(model, one_variable(payload_decl)?)?;
    if !empty_array(one_variable(payload_decl)?.init.as_ref()?) {
        return None;
    }
    let (hash, hash_target) = hash_declaration(model, site, hash_stmt, seed)?;
    let acc = plain_local(model, one_variable(acc_decl)?)?;
    let initial = call(one_variable(acc_decl)?.init.as_ref()?)?;
    let (mix_target, initial_field, initial_fourth) = mix_call(model, site, initial, hash, bytes)?;
    let captured_field = data_capture(model, site, numeric_index(initial_field)?)?;
    if captured_field == payload
        || captured_field == hash
        || captured_field == seed
        || mix_target.span == hash_target.span
        || !number(initial_fourth, 3.0)
    {
        return None;
    }
    array_from_payload(model, payload, convert)?;
    let length = plain_local(model, one_variable(length_decl)?)?;
    if !member(one_variable(length_decl)?.init.as_ref()?, "length")
        .is_some_and(|receiver| symbol(model, receiver) == Some(payload))
    {
        return None;
    }
    let length_mix = assignment(length_mix)?;
    if assign_symbol(model, length_mix) != Some(acc) {
        return None;
    }
    let length_call = mix_call(model, site, call(&length_mix.right)?, hash, bytes)?;
    if length_call.0.span != mix_target.span
        || symbol(model, length_call.1) != Some(length)
        || symbol(model, length_call.2) != Some(acc)
    {
        return None;
    }
    reverse_audio_loop(
        model, site, loop_stmt, payload, length, acc, hash, bytes, mix_target,
    )?;
    let _ = (next, complete);
    observer_end(model, site, body, 10, bytes, Some(payload_local))
}

/// Reverse audio prefix with the owned empty output declared before the nonce.
/// Unlike the older nonce-first form, the fractional word may be reseeded in
/// place. Every write is accounted for; no nested callable donates operations.
fn emit_audio_reverse_prefix_hash(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [payload_stmt, random, output, nonce, hash_stmt, acc_decl, convert, length_decl, length_mix, loop_stmt, _, _] =
        body.statements.as_slice()
    else {
        return None;
    };
    let payload = audio_payload(model, site, payload_stmt)?;
    let seed = plain_local(model, one_variable(random)?)?;
    let random_value = one_variable(random)?.init.as_ref()?;
    let bytes = if random_fraction(model, random_value) {
        zeroed_nonce_declaration(model, one_variable(nonce)?, seed)?
    } else if random_number(model, random_value) {
        nonce_declaration(model, one_variable(nonce)?, seed)?
    } else {
        return None;
    };
    let output = one_variable(output)?;
    let output_symbol = plain_local(model, output)?;
    if !empty_array(output.init.as_ref()?) {
        return None;
    }
    let (hash, hash_target) = hash_declaration(model, site, hash_stmt, seed)?;
    let acc = plain_local(model, one_variable(acc_decl)?)?;
    let (mix_target, prefix, cursor) = mix_call(
        model,
        site,
        call(one_variable(acc_decl)?.init.as_ref()?)?,
        hash,
        bytes,
    )?;
    let captured_field = data_capture(model, site, numeric_index(prefix)?)?;
    if mix_target.span == hash_target.span || !number(cursor, 3.0) {
        return None;
    }
    array_from_payload(model, payload, convert)?;
    let length = plain_local(model, one_variable(length_decl)?)?;
    if !member(one_variable(length_decl)?.init.as_ref()?, "length")
        .is_some_and(|receiver| symbol(model, receiver) == Some(payload))
    {
        return None;
    }
    let length_mix = assignment(length_mix)?;
    if assign_symbol(model, length_mix) != Some(acc) {
        return None;
    }
    let (length_target, length_operand, length_cursor) =
        mix_call(model, site, call(&length_mix.right)?, hash, bytes)?;
    if length_target.span != mix_target.span
        || symbol(model, length_operand) != Some(length)
        || symbol(model, length_cursor) != Some(acc)
    {
        return None;
    }
    reverse_audio_loop(
        model, site, loop_stmt, payload, length, acc, hash, bytes, mix_target,
    )?;
    // These symbols are distinct declarations (apart from the allowed seed
    // reassignment). Count writes globally to catch effects hidden in helpers.
    for (binding, writes) in [
        (payload, 1),
        (seed, usize::from(hash == seed)),
        (bytes, 0),
        (output_symbol, 0),
        (acc, 2),
        (length, 1),
    ] {
        if model.binding_writes(binding) != writes || captured_field == binding {
            return None;
        }
    }
    if hash != seed && model.binding_writes(hash) != 0 {
        return None;
    }
    observer_end(model, site, body, 10, bytes, Some(output_symbol))
}

/// Audio-channel reverse-while trailer variant. It has no captured prefix
/// field: the accumulator starts from the owned payload length, and one
/// captured numeric field is mixed by a final discarded call after the loop.
fn emit_audio_reverse_hash_trailer(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [payload_stmt, random, third, fourth, hash_stmt, convert, length_decl, acc_decl, loop_stmt, trailer, next, complete] =
        body.statements.as_slice()
    else {
        return None;
    };
    let payload = audio_payload(model, site, payload_stmt)?;
    let seed = plain_local(model, one_variable(random)?)?;
    // The output array and nonce have independent initializers. Select their
    // roles by the empty-array initializer, then prove both exact declarations.
    let (nonce, payload_decl) = if one_variable(third)?.init.as_ref().is_some_and(empty_array) {
        (fourth, third)
    } else {
        (third, fourth)
    };
    let random_value = one_variable(random)?.init.as_ref()?;
    let bytes = if random_fraction(model, random_value) {
        zeroed_nonce_declaration(model, one_variable(nonce)?, seed)?
    } else if random_number(model, random_value) {
        nonce_declaration(model, one_variable(nonce)?, seed)?
    } else {
        return None;
    };
    let payload_local = plain_local(model, one_variable(payload_decl)?)?;
    if !empty_array(one_variable(payload_decl)?.init.as_ref()?) {
        return None;
    }
    let (hash, hash_target) = hash_declaration(model, site, hash_stmt, seed)?;
    array_from_payload(model, payload, convert)?;
    let length = plain_local(model, one_variable(length_decl)?)?;
    if !member(one_variable(length_decl)?.init.as_ref()?, "length")
        .is_some_and(|receiver| symbol(model, receiver) == Some(payload))
    {
        return None;
    }
    let acc = plain_local(model, one_variable(acc_decl)?)?;
    let initial = call(one_variable(acc_decl)?.init.as_ref()?)?;
    let (mix_target, initial_field, initial_fourth) = mix_call(model, site, initial, hash, bytes)?;
    if mix_target.span == hash_target.span
        || symbol(model, initial_field) != Some(length)
        || !number(initial_fourth, 3.0)
    {
        return None;
    }
    reverse_audio_loop(
        model, site, loop_stmt, payload, length, acc, hash, bytes, mix_target,
    )?;
    let trailer = statement_call(trailer)?;
    let (trailer_target, trailer_field, trailer_fourth) =
        mix_call(model, site, trailer, hash, bytes)?;
    let captured_field = data_capture(model, site, numeric_index(trailer_field)?)?;
    if trailer_target.span != mix_target.span
        || symbol(model, trailer_fourth) != Some(acc)
        || captured_field == payload
        || captured_field == hash
        || captured_field == seed
    {
        return None;
    }
    let _ = (next, complete);
    observer_end(model, site, body, 10, bytes, Some(payload_local))
}

/// Audio-channel variant: the payload array starts as `formal.getChannelData(0)`
/// and is converted in place with `Array.from`, then a `for` scan over that
/// owned array feeds the same mixer chain. The initial mixer field is one
/// captured numeric member; the captured index is opaque payload input.
fn emit_audio_hash(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [payload_stmt, random, nonce, hash_stmt, acc_decl, convert, length_decl, i_decl, length_mix, Statement::ForStatement(loop_), next, complete] =
        body.statements.as_slice()
    else {
        return None;
    };
    let payload_variable = one_variable(payload_stmt)?;
    let payload = plain_local(model, payload_variable)?;
    let channel = call(payload_variable.init.as_ref()?)?;
    let Some(receiver) = member(&channel.callee, "getChannelData") else {
        return None;
    };
    if model.site_parameter_slot(receiver, site) != Some(0) {
        return None;
    }
    let channel_args = arity(channel, 1)?;
    if !matches!(channel_args[0], Expression::NumericLiteral(_)) {
        return None;
    }
    let seed = plain_local(model, one_variable(random)?)?;
    if !random_number(model, one_variable(random)?.init.as_ref()?) {
        return None;
    }
    let bytes = nonce_declaration(model, one_variable(nonce)?, seed)?;
    let (hash, hash_target) = match one_variable(hash_stmt) {
        Some(v) => {
            let hash = plain_local(model, v)?;
            (hash, hash_seeded(model, site, v.init.as_ref()?, seed)?)
        }
        None => {
            let assign = assignment(hash_stmt)?;
            if assign_symbol(model, assign) != Some(seed) {
                return None;
            }
            (seed, hash_seeded(model, site, &assign.right, seed)?)
        }
    };
    let acc = plain_local(model, one_variable(acc_decl)?)?;
    let acc_call = call(one_variable(acc_decl)?.init.as_ref()?)?;
    let (mix_target, second, fourth) = mix_call(model, site, acc_call, hash, bytes)?;
    if mix_target.span == hash_target.span || !number(fourth, 3.0) {
        return None;
    }
    let captured_field = data_capture(model, site, numeric_index(second)?)?;
    if captured_field == payload || captured_field == hash || captured_field == seed {
        return None;
    }
    let convert = assignment(convert)?;
    if assign_symbol(model, convert) != Some(payload) {
        return None;
    }
    let Some(from) = call(&convert.right) else {
        return None;
    };
    if !is_global(model, &from.callee, &["Array", "from"]) {
        return None;
    }
    let from_args = arity(from, 1)?;
    if symbol(model, from_args[0]) != Some(payload) {
        return None;
    }
    let length = plain_local(model, one_variable(length_decl)?)?;
    if !member(one_variable(length_decl)?.init.as_ref()?, "length")
        .is_some_and(|receiver| symbol(model, receiver) == Some(payload))
    {
        return None;
    }
    let i = plain_local(model, one_variable(i_decl)?)?;
    if i == length || !number(one_variable(i_decl)?.init.as_ref()?, 0.0) {
        return None;
    }
    let length_update = assignment(length_mix)?;
    if assign_symbol(model, length_update) != Some(acc) {
        return None;
    }
    let (length_target, length_second, length_fourth) =
        mix_call(model, site, call(&length_update.right)?, hash, bytes)?;
    if length_target.span != mix_target.span
        || symbol(model, length_second) != Some(length)
        || symbol(model, length_fourth) != Some(acc)
    {
        return None;
    }
    if loop_.init.is_some() {
        return None;
    }
    let test = loop_.test.as_ref()?;
    let Expression::BinaryExpression(less) = test else {
        return None;
    };
    if less.operator.as_str() != "<"
        || symbol(model, &less.left) != Some(i)
        || symbol(model, &less.right) != Some(length)
        || loop_
            .update
            .as_ref()
            .and_then(|e| update_symbol(model, e, false, "++"))
            != Some(i)
    {
        return None;
    }
    let Statement::BlockStatement(block) = &loop_.body else {
        return None;
    };
    let [update] = block.body.as_slice() else {
        return None;
    };
    let update = assignment(update)?;
    if assign_symbol(model, update) != Some(acc) {
        return None;
    }
    let (update_target, operand, update_fourth) =
        mix_call(model, site, call(&update.right)?, hash, bytes)?;
    let Expression::ComputedMemberExpression(element) = operand else {
        return None;
    };
    if update_target.span != mix_target.span
        || element.optional
        || symbol(model, &element.object) != Some(payload)
        || symbol(model, &element.expression) != Some(i)
        || symbol(model, update_fourth) != Some(acc)
    {
        return None;
    }
    let _ = (next, complete);
    observer_end(model, site, body, 10, bytes, None)
}

/// Audio-channel trailer variant: same twelve statements, same owned
/// `getChannelData`/`Array.from` payload, same header/nonce/loop/observer, but
/// the captured numeric field moves from the initial accumulator operand to a
/// trailing ignored mixer update whose return is discarded. The initial operand
/// is the loop length local and the captured field is re-proved with the same
/// `data_capture` and mixer identities as the audio proof.
fn emit_audio_hash_trailer(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [payload_stmt, random, nonce, hash_stmt, convert, length_decl, i_decl, acc_decl, Statement::ForStatement(loop_), trailer, next, complete] =
        body.statements.as_slice()
    else {
        return None;
    };
    let payload_variable = one_variable(payload_stmt)?;
    let payload = plain_local(model, payload_variable)?;
    let channel = call(payload_variable.init.as_ref()?)?;
    let Some(receiver) = member(&channel.callee, "getChannelData") else {
        return None;
    };
    if model.site_parameter_slot(receiver, site) != Some(0) {
        return None;
    }
    let channel_args = arity(channel, 1)?;
    if !matches!(channel_args[0], Expression::NumericLiteral(_)) {
        return None;
    }
    let seed = plain_local(model, one_variable(random)?)?;
    if !random_number(model, one_variable(random)?.init.as_ref()?) {
        return None;
    }
    let bytes = nonce_declaration(model, one_variable(nonce)?, seed)?;
    let (hash, hash_target) = match one_variable(hash_stmt) {
        Some(v) => {
            let hash = plain_local(model, v)?;
            (hash, hash_seeded(model, site, v.init.as_ref()?, seed)?)
        }
        None => {
            let assign = assignment(hash_stmt)?;
            if assign_symbol(model, assign) != Some(seed) {
                return None;
            }
            (seed, hash_seeded(model, site, &assign.right, seed)?)
        }
    };
    let convert = assignment(convert)?;
    if assign_symbol(model, convert) != Some(payload) {
        return None;
    }
    let Some(from) = call(&convert.right) else {
        return None;
    };
    if !is_global(model, &from.callee, &["Array", "from"]) {
        return None;
    }
    let from_args = arity(from, 1)?;
    if symbol(model, from_args[0]) != Some(payload) {
        return None;
    }
    let length = plain_local(model, one_variable(length_decl)?)?;
    if !member(one_variable(length_decl)?.init.as_ref()?, "length")
        .is_some_and(|receiver| symbol(model, receiver) == Some(payload))
    {
        return None;
    }
    let i = plain_local(model, one_variable(i_decl)?)?;
    if i == length || !number(one_variable(i_decl)?.init.as_ref()?, 0.0) {
        return None;
    }
    let acc = plain_local(model, one_variable(acc_decl)?)?;
    let acc_call = call(one_variable(acc_decl)?.init.as_ref()?)?;
    let (mix_target, acc_second, acc_fourth) = mix_call(model, site, acc_call, hash, bytes)?;
    if mix_target.span == hash_target.span
        || symbol(model, acc_second) != Some(length)
        || !number(acc_fourth, 3.0)
    {
        return None;
    }
    if loop_.init.is_some() {
        return None;
    }
    let test = loop_.test.as_ref()?;
    let Expression::BinaryExpression(less) = test else {
        return None;
    };
    if less.operator.as_str() != "<"
        || symbol(model, &less.left) != Some(i)
        || symbol(model, &less.right) != Some(length)
        || loop_
            .update
            .as_ref()
            .and_then(|e| update_symbol(model, e, false, "++"))
            != Some(i)
    {
        return None;
    }
    let Statement::BlockStatement(block) = &loop_.body else {
        return None;
    };
    let [update] = block.body.as_slice() else {
        return None;
    };
    let update = assignment(update)?;
    if assign_symbol(model, update) != Some(acc) {
        return None;
    }
    let (update_target, operand, update_fourth) =
        mix_call(model, site, call(&update.right)?, hash, bytes)?;
    let Expression::ComputedMemberExpression(element) = operand else {
        return None;
    };
    if update_target.span != mix_target.span
        || element.optional
        || symbol(model, &element.object) != Some(payload)
        || symbol(model, &element.expression) != Some(i)
        || symbol(model, update_fourth) != Some(acc)
    {
        return None;
    }
    let (trailer_target, trailer_second, trailer_fourth) =
        mix_call(model, site, statement_call(trailer)?, hash, bytes)?;
    if trailer_target.span != mix_target.span || symbol(model, trailer_fourth) != Some(acc) {
        return None;
    }
    let captured_field = data_capture(model, site, numeric_index(trailer_second)?)?;
    if captured_field == payload || captured_field == hash || captured_field == seed {
        return None;
    }
    let _ = (next, complete);
    observer_end(model, site, body, 10, bytes, None)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || !directives(body)
        || !plain_params(model, &function.params)
    {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if function.params.items.len() == 1
        && (emit_fixed_hash(model, site, body).is_some()
            || emit_fixed_hash_inner_mix(model, site, body).is_some()
            || emit_fixed_hash_compact_inner_mix(model, site, body).is_some()
            || emit_loop_hash(model, site, body).is_some()
            || emit_descending_hash(model, site, body, f, proposals).is_some()
            || emit_audio_reverse_hash(model, site, body).is_some()
            || emit_audio_reverse_prefix_hash(model, site, body).is_some()
            || emit_audio_reverse_hash_trailer(model, site, body).is_some()
            || emit_audio_hash(model, site, body).is_some()
            || emit_audio_hash_trailer(model, site, body).is_some())
    {
        claims.push(proposal(
            "emitSeededHashEncoding",
            "encoding_seeded_hash_encoding",
            "exact random nonce array, seeded hash callable, four-argument mixing updates over the formal/length plus the same captured nonce array, then next/complete on one captured observer; fixed indexed, compact inner-mix fixed fields, length-loop (hash or length declared first), descending fixed-field, owned audio-channel Array.from forward/reverse while and audio-trailer payload variants only",
            f,
        ));
    }
    claims
}
