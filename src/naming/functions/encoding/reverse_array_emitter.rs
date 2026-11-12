//! Closed reverse-array emitter protocol. Only the predecrement for/Number
//! lowering is admitted; external array/native/channel purpose is unproved.
//! Captured callables are resolved and re-proved by identity, never by spelling.
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;
type Symbol = SymbolId;
fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expression.without_parentheses() {
        Expression::CallExpression(call) if !call.optional => Some(call),
        _ => None,
    }
}

/// Exact argument list. A spread or other non-expression argument changes the
/// list length and refuses the shape.
fn arguments<'a>(call: &'a CallExpression<'a>, count: usize) -> Option<Vec<&'a Expression<'a>>> {
    let values = call_arguments(call);
    (call.arguments.len() == count && values.len() == count).then_some(values)
}

fn member<'a>(expression: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member)
            if !member.optional && member.property.name == key =>
        {
            Some(&member.object)
        }
        _ => None,
    }
}

fn numeric(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal) => Some(literal.value),
        _ => None,
    }
}

fn number(expression: &Expression<'_>, expected: f64) -> bool {
    numeric(expression) == Some(expected)
}

fn empty_array(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::ArrayExpression(array) if array.elements.is_empty())
}

fn expression<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match statement {
        Statement::ExpressionStatement(statement) => Some(&statement.expression),
        _ => None,
    }
}

fn statement_call<'a>(statement: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    call(expression(statement)?)
}

fn assignment<'a>(statement: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    match expression(statement)? {
        Expression::AssignmentExpression(assignment) if assignment.operator.as_str() == "=" => {
            Some(assignment)
        }
        _ => None,
    }
}

fn assign_symbol(model: &Model<'_, '_>, assignment: &AssignmentExpression<'_>) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

fn one_variable<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(variable)
}

fn plain_local(model: &Model<'_, '_>, variable: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
        return None;
    };
    let value = identifier.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(value)
        .is_empty()
        .then_some(value)
}

fn directives(body: &FunctionBody<'_>) -> bool {
    body.directives.is_empty()
        || (body.directives.len() == 1 && body.directives[0].directive == "use strict")
}

fn unchanged(model: &Model<'_, '_>, value: Symbol) -> bool {
    model.binding_writes(value) == 0 && model.scoping.symbol_redeclarations(value).is_empty()
}

fn plain_params(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> bool {
    params.rest.is_none()
        && params.items.iter().all(|parameter| {
            parameter.initializer.is_none()
                && matches!(&parameter.pattern, BindingPattern::BindingIdentifier(identifier)
                    if identifier.symbol_id.get().is_some_and(|value| unchanged(model, value)))
        })
}

fn one_plain_formal(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> bool {
    params.rest.is_none()
        && params.items.len() == 1
        && params.items[0].initializer.is_none()
        && matches!(&params.items[0].pattern, BindingPattern::BindingIdentifier(identifier)
            if identifier.symbol_id.get().is_some_and(|value| unchanged(model, value)))
}

fn captured_data(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<Symbol> {
    let value = symbol(model, expression)?;
    let span = model.scoping.symbol_span(value);
    (!site.span.contains_inclusive(span)
        && span.end <= expression.span().start
        && model.formal_symbol_slot(value).is_none()
        && model.scoping.symbol_redeclarations(value).is_empty()
        && matches!(
            model.nodes.kind(model.scoping.symbol_declaration(value)),
            AstKind::VariableDeclarator(_)
        ))
    .then_some(value)
}

fn owned_facts<'a>(model: &Model<'_, 'a>, target: FunctionSite) -> Facts<'a> {
    let nodes = model.nodes;
    let mut facts = Facts::default();
    for node in nodes.iter() {
        let Some(owner) = model.owning_site(node.id()) else {
            continue;
        };
        if owner.span.start != target.span.start {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call) => facts.calls.push(call),
            AstKind::ReturnStatement(statement) => {
                if let Some(value) = &statement.argument {
                    facts.returns.push(value);
                }
            }
            AstKind::StaticMemberExpression(member) => {
                facts.members.insert(member.property.name.to_string());
            }
            _ => {}
        }
    }
    facts
}

/// The accepted source-proven number writer used by the variant. The
/// resolved callee site is re-proved synchronously with the shared
/// `thread_posthost_numeric` evaluator and must produce that evaluator's
/// `writeNumber` claim. Name, spelling, arity alone or a foreign four-formal
/// function is never sufficient; a foreign writer fails this identity check
/// even when both numeric call sites use it.
pub(in crate::naming) fn numeric_writer(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<Symbol> {
    let value = symbol(model, expression)?;
    let target = captured_target(model, site, expression)?;
    let AstKind::Function(function) = model.nodes.kind(target.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    if function.r#async
        || function.generator
        || !directives(body)
        || !plain_params(model, &function.params)
        || function.params.items.len() != 4
    {
        return None;
    }
    let facts = owned_facts(model, target);
    let claims = super::numeric_writer::evaluate(model, target, &facts, proposals);
    claims
        .iter()
        .any(|claim| claim.name == "writeNumber" && claim.evaluator == "thread_posthost_numeric")
        .then_some(value)
}

fn global_path(model: &Model<'_, '_>, expression: &Expression<'_>) -> Vec<String> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => {
            if crate::analysis::identifier_symbol(identifier, model.scoping).is_some() {
                Vec::new()
            } else {
                vec![identifier.name.to_string()]
            }
        }
        Expression::StaticMemberExpression(member) if !member.optional => {
            let mut path = global_path(model, &member.object);
            if !path.is_empty() {
                path.push(member.property.name.to_string());
            }
            path
        }
        _ => Vec::new(),
    }
}

fn is_global(model: &Model<'_, '_>, expression: &Expression<'_>, path: &[&str]) -> bool {
    let found = global_path(model, expression);
    found.len() == path.len()
        && found
            .iter()
            .zip(path.iter())
            .all(|(actual, expected)| actual.as_str() == *expected)
}

fn random_seed(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(or) = expression.without_parentheses() else {
        return false;
    };
    if or.operator.as_str() != "|" || !number(&or.right, 0.0) {
        return false;
    }
    let Expression::BinaryExpression(product) = or.left.without_parentheses() else {
        return false;
    };
    if product.operator.as_str() != "*" || !number(&product.right, 4_294_967_296.0) {
        return false;
    }
    let Some(random) = call(&product.left) else {
        return false;
    };
    is_global(model, &random.callee, &["Math", "random"]) && random.arguments.is_empty()
}

fn seed_base(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    if symbol(model, expression) == Some(seed) {
        return true;
    }
    let Expression::BinaryExpression(or) = expression.without_parentheses() else {
        return false;
    };
    or.operator.as_str() == "|" && number(&or.right, 0.0) && symbol(model, &or.left) == Some(seed)
}

fn big_endian_header(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return false;
    };
    let mut elements = Vec::new();
    for element in &array.elements {
        let Some(value) = element.as_expression() else {
            return false;
        };
        elements.push(value);
    }
    if elements.len() != 4 {
        return false;
    }
    let Expression::BinaryExpression(shift) = elements[0].without_parentheses() else {
        return false;
    };
    if shift.operator.as_str() != ">>>"
        || !seed_base(model, &shift.left, seed)
        || !number(&shift.right, 24.0)
    {
        return false;
    }
    for (slot, shift_value) in [(1usize, 16.0f64), (2, 8.0)] {
        let Expression::BinaryExpression(mask) = elements[slot].without_parentheses() else {
            return false;
        };
        if mask.operator.as_str() != "&" || !number(&mask.right, 255.0) {
            return false;
        }
        let Expression::BinaryExpression(inner) = mask.left.without_parentheses() else {
            return false;
        };
        if inner.operator.as_str() != ">>"
            || !seed_base(model, &inner.left, seed)
            || !number(&inner.right, shift_value)
        {
            return false;
        }
    }
    let Expression::BinaryExpression(mask) = elements[3].without_parentheses() else {
        return false;
    };
    mask.operator.as_str() == "&"
        && seed_base(model, &mask.left, seed)
        && number(&mask.right, 255.0)
}

// The shared resolver includes ordered initialization at the actual reference
// edge. Do not replace it with a role/name lookup or a family member inventory.
fn captured_target(
    model: &Model<'_, '_>,
    site: FunctionSite,
    value: &Expression<'_>,
) -> Option<FunctionSite> {
    // A deferred callback may be invoked before a later outer initializer.
    // Require its captured alias to have an ordered origin before the callback
    // declaration itself, not merely before the textual reference in its body.
    let capture = symbol(model, value)?;
    match model.nodes.kind(model.scoping.symbol_declaration(capture)) {
        AstKind::VariableDeclarator(variable) => {
            if variable.init.is_some() {
                if variable.span.end > site.span.start {
                    return None;
                }
            } else {
                let write = model
                    .scoping
                    .get_resolved_references(capture)
                    .find(|r| r.is_write())?;
                let AstKind::AssignmentExpression(origin) =
                    model.nodes.parent_kind(write.node_id())
                else {
                    return None;
                };
                if origin.span.end > site.span.start {
                    return None;
                }
            }
        }
        AstKind::Function(_) => {}
        _ => return None,
    }
    let target = super::stable_function(model, value, 0).or_else(|| crate::naming::support::callable_identity::captured_callable(model, site, value))?;
    (!site.span.contains_inclusive(target.span) && target.span.end <= value.span().start)
        .then_some(target)
}

#[derive(Clone, Copy)]
struct Salt(u32);
fn salt(expression: &Expression<'_>) -> Option<Salt> {
    let n = numeric(expression)?;
    (n.is_finite() && n.fract() == 0.0 && (0.0..=4_294_967_295.0).contains(&n))
        .then_some(Salt(n as u32))
}
fn length(model: &Model<'_, '_>, expression: &Expression<'_>, input: Symbol) -> bool {
    member(expression, "length").is_some_and(|object| symbol(model, object) == Some(input))
}
fn decrement(model: &Model<'_, '_>, statement: &Statement<'_>, index: Symbol) -> bool {
    let Some(Expression::UpdateExpression(update)) = expression(statement) else {
        return false;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &update.argument else {
        return false;
    };
    update.operator.as_str() == "--"
        && update.prefix
        && crate::analysis::identifier_symbol(id, model.scoping) == Some(index)
}
fn prove(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<Salt> {
    let (empty, statements) = if body.statements.len() == 9 {
        let v = one_variable(&body.statements[0])?;
        let id = plain_local(model, v)?;
        if !unchanged(model, id) || !empty_array(v.init.as_ref()?) { return None; }
        (Some(id), &body.statements[1..])
    } else { (None, body.statements.as_slice()) };
    let [seed_s, header_s, stream_s, index_s, cursor_s, loop_s, next_s, complete_s] = statements else { return None; };
    let while_variant = matches!(loop_s, Statement::WhileStatement(_));
    let seed_v = one_variable(seed_s)?;
    let seed = plain_local(model, seed_v)?;
    if !unchanged(model, seed) || !random_seed(model, seed_v.init.as_ref()?) {
        return None;
    }
    let header_v = one_variable(header_s)?;
    let header = plain_local(model, header_v)?;
    if !unchanged(model, header) || !big_endian_header(model, header_v.init.as_ref()?, seed) {
        return None;
    }
    let stream_v = one_variable(stream_s)?;
    let stream = plain_local(model, stream_v)?;
    if !unchanged(model, stream) {
        return None;
    }
    let factory_call = call(stream_v.init.as_ref()?)?;
    captured_target(model, site, &factory_call.callee)?;
    super::curried_seeded_emitter::factory_role(
        model,
        site,
        &factory_call.callee,
        proposals,
        "nextKeystreamByte",
    )?;
    let seed_args = arguments(factory_call, 1)?;
    let Expression::BinaryExpression(xor) = seed_args[0].without_parentheses() else {
        return None;
    };
    if xor.operator.as_str() != "^" || symbol(model, &xor.left) != Some(seed) {
        return None;
    }
    let salt = salt(&xor.right)?;
    let BindingPattern::BindingIdentifier(input_id) = &match model.nodes.kind(site.node_id) {
        AstKind::Function(f) => f,
        _ => return None,
    }
    .params
    .items[0]
        .pattern
    else {
        return None;
    };
    let input = input_id.symbol_id.get()?;
    let index_v = one_variable(index_s)?;
    let index = plain_local(model, index_v)?;
    if model.binding_writes(index) != (if while_variant { 1 } else { 2 }) || !length(model, index_v.init.as_ref()?, input) {
        return None;
    }
    let cursor_v = one_variable(cursor_s)?;
    let cursor = plain_local(model, cursor_v)?;
    if model.binding_writes(cursor) != 1 {
        return None;
    }
    let first = call(cursor_v.init.as_ref()?)?;
    let writer = numeric_writer(model, site, &first.callee, proposals)?;
    let args = arguments(first, 4)?;
    if symbol(model, args[0]) != Some(stream)
        || !length(model, args[1], input)
        || symbol(model, args[2]) != Some(header)
        || !number(args[3], 3.0)
    {
        return None;
    }
    let write_s = if let Statement::WhileStatement(loop_) = loop_s {
        let Expression::BinaryExpression(test) = loop_.test.without_parentheses() else { return None; };
        let Expression::BinaryExpression(offset) = test.left.without_parentheses() else { return None; };
        if test.operator.as_str() != ">=" || !number(&test.right, 0.0)
            || offset.operator.as_str() != "-" || symbol(model, &offset.left) != Some(index) || !number(&offset.right, 1.0) { return None; }
        let Statement::BlockStatement(block) = &loop_.body else { return None; };
        let [write, step] = block.body.as_slice() else { return None; };
        if !decrement(model, step, index) { return None; }
        write
    } else {
        let Statement::ForStatement(loop_) = loop_s else {
            return None;
        };
        if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() {
            return None;
        }
        let Statement::BlockStatement(block) = &loop_.body else {
            return None;
        };
        let [decrement_s, guard_s, write_s, normalize_s] = block.body.as_slice() else {
            return None;
        };
        if !decrement(model, decrement_s, index) {
            return None;
        }
        let Statement::IfStatement(guard) = guard_s else {
            return None;
        };
        if guard.alternate.is_some() {
            return None;
        }
        let Expression::UnaryExpression(negated) = guard.test.without_parentheses() else {
            return None;
        };
        if negated.operator.as_str() != "!" {
            return None;
        }
        let Expression::BinaryExpression(test) = negated.argument.without_parentheses() else {
            return None;
        };
        if test.operator.as_str() != ">="
            || symbol(model, &test.left) != Some(index)
            || !number(&test.right, 0.0)
        {
            return None;
        }
        let Statement::BreakStatement(exit) = &guard.consequent else {
            return None;
        };
        if exit.label.is_some() {
            return None;
        }
        let normalize = assignment(normalize_s)?;
        if assign_symbol(model, normalize) != Some(index) {
            return None;
        }
        let conversion = call(&normalize.right)?;
        let convert_args = arguments(conversion, 1)?;
        if !is_global(model, &conversion.callee, &["Number"])
            || symbol(model, convert_args[0]) != Some(index)
        {
            return None;
        }
        write_s
    };
    let write = assignment(write_s)?;
    if assign_symbol(model, write) != Some(cursor) {
        return None;
    }
    let next_write = call(&write.right)?;
    if symbol(model, &next_write.callee) != Some(writer) {
        return None;
    }
    let args = arguments(next_write, 4)?;
    let Expression::ComputedMemberExpression(element) = args[1].without_parentheses() else {
        return None;
    };
    if element.optional
        || symbol(model, &element.object) != Some(input)
        || !(if while_variant {
            matches!(element.expression.without_parentheses(), Expression::BinaryExpression(offset) if offset.operator.as_str() == "-" && symbol(model, &offset.left) == Some(index) && number(&offset.right, 1.0))
        } else { symbol(model, &element.expression) == Some(index) })
        || symbol(model, args[0]) != Some(stream)
        || symbol(model, args[2]) != Some(header)
        || symbol(model, args[3]) != Some(cursor)
    {
        return None;
    }
    let next = statement_call(next_s)?;
    let observer = captured_data(model, site, member(&next.callee, "next")?)?;
    let args = arguments(next, 1)?;
    let Expression::ArrayExpression(pair) = args[0].without_parentheses() else {
        return None;
    };
    if pair.elements.len() != 2
        || !(if let Some(empty) = empty { symbol(model, pair.elements[0].as_expression()?) == Some(empty) } else { empty_array(pair.elements[0].as_expression()?) })
        || symbol(model, pair.elements[1].as_expression()?) != Some(header)
    {
        return None;
    }
    let complete = statement_call(complete_s)?;
    if symbol(model, member(&complete.callee, "complete")?) != Some(observer) {
        return None;
    }
    arguments(complete, 0)?;
    // Exact owned statements account for all local effects; additionally reject
    // references donating a header local through a nested escaping scope.
    let locals = [seed, stream, header, index, cursor, input];
    let distinct: BTreeSet<_> = locals.into_iter().collect();
    if distinct.len() != locals.len() {
        return None;
    }
    if model.scoping.get_resolved_references(header).count() != 3 {
        return None;
    }
    Some(salt)
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
    if function.r#async || function.generator || !one_plain_formal(model, &function.params) {
        return Vec::new();
    }
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if !directives(body) {
        return Vec::new();
    }
    if let Some(order) = prove_tuple(model, site, body, proposals) {
        return vec![proposal("emitSeededFiveFieldTuple", "owned_seeded_five_field_tuple", &format!("complete five-field formal tuple emission in exact index order {:?}; source-proven numeric writer and seed-to-byte factory, four-byte big-endian header, chained cursor and same captured next/complete; external tuple purpose unproved", order), f)];
    }
    let Some(salt) = prove(model, site, body, proposals) else {
        return Vec::new();
    };
    if matches!(body.statements.last().and_then(|_| body.statements.get(body.statements.len().saturating_sub(3))), Some(Statement::WhileStatement(_))) {
        return vec![proposal("emitReverseArrayEncoding", "owned_reverse_array_emitter", &format!("whole length-framed descending while(index-1>=0) body; exact u32 XOR salt {}; source-proven writer/factory, index-1 read before prefix decrement, cursor chaining, next then complete; external formal purpose unproved", salt.0), f)];
    }
    vec![proposal("emitReverseArrayEncoding", "owned_reverse_array_emitter", &format!("owned predecrement/Number reverse traversal; exact u32 XOR salt {}; source-proven numeric writer and factory-owned nextKeystreamByte with live seed-to-state dataflow; big-endian random header, length at cursor 3, returned cursor chained through each descending element, next([[], header]) then complete on same capture; external capture/channel and formal native/AudioBuffer purpose explicitly unproved", salt.0), f)]
}

/// Five-field tuple protocol: preserve the exact encountered permutation and
/// every writer edge; captures are independently re-proved, never name lookups.
fn prove_tuple(
    model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<Vec<u32>> {
    let [seed_s, header_s, stream_s, first_s, second_s, third_s, fourth_s, last_s, next_s, complete_s] = body.statements.as_slice() else { return None; };
    let seed_v = one_variable(seed_s)?;
    let seed = plain_local(model, seed_v)?;
    // Both explicit coercion placements are checked, never normalized away.
    let init = seed_v.init.as_ref()?;
    let coerced = random_seed(model, init);
    let raw = if let Expression::BinaryExpression(product) = init.without_parentheses() {
        product.operator.as_str() == "*" && number(&product.right, 4294967296.0)
            && call(&product.left).is_some_and(|r| r.arguments.is_empty() && is_global(model, &r.callee, &["Math", "random"]))
    } else { false };
    if !unchanged(model, seed) || !(coerced || raw) { return None; }
    let header_v = one_variable(header_s)?;
    let header = plain_local(model, header_v)?;
    if !unchanged(model, header) || !big_endian_header(model, header_v.init.as_ref()?, seed) { return None; }
    let stream_v = one_variable(stream_s)?;
    let stream = plain_local(model, stream_v)?;
    if !unchanged(model, stream) { return None; }
    let factory = call(stream_v.init.as_ref()?)?;
    captured_target(model, site, &factory.callee)?;
    super::curried_seeded_emitter::factory_role(model, site, &factory.callee, proposals, "nextKeystreamByte")?;
    let fa = arguments(factory, 1)?;
    let Expression::BinaryExpression(xor) = fa[0].without_parentheses() else { return None; };
    if xor.operator.as_str() != "^" || !seed_base(model, &xor.left, seed) { return None; }
    salt(&xor.right)?;
    let AstKind::Function(fun) = model.nodes.kind(site.node_id) else { return None; };
    let BindingPattern::BindingIdentifier(input_id) = &fun.params.items[0].pattern else { return None; };
    let input = input_id.symbol_id.get()?;
    let cursor_v = one_variable(first_s)?;
    let cursor = plain_local(model, cursor_v)?;
    if model.binding_writes(cursor) != 3 { return None; }
    let first = call(cursor_v.init.as_ref()?)?;
    let writer = numeric_writer(model, site, &first.callee, proposals)?;
    let mut calls = vec![first];
    for statement in [second_s, third_s, fourth_s] {
        let a = assignment(statement)?;
        if assign_symbol(model, a) != Some(cursor) { return None; }
        calls.push(call(&a.right)?);
    }
    calls.push(statement_call(last_s)?);
    let mut order = Vec::new();
    for (pos, c) in calls.into_iter().enumerate() {
        let a = arguments(c, 4)?;
        let Expression::ComputedMemberExpression(field) = a[1].without_parentheses() else { return None; };
        let index = numeric(&field.expression)?;
        if field.optional || symbol(model, &field.object) != Some(input)
            || index.fract() != 0.0 || !(0.0..=4.0).contains(&index)
            || symbol(model, &c.callee) != Some(writer)
            || symbol(model, a[0]) != Some(stream) || symbol(model, a[2]) != Some(header)
            || !(if pos == 0 { number(a[3], 3.0) } else { symbol(model, a[3]) == Some(cursor) }) { return None; }
        if order.contains(&(index as u32)) { return None; }
        order.push(index as u32);
    }
    let next = statement_call(next_s)?;
    let observer = captured_data(model, site, member(&next.callee, "next")?)?;
    let a = arguments(next, 1)?;
    let Expression::ArrayExpression(pair) = a[0].without_parentheses() else { return None; };
    if pair.elements.len() != 2 || !empty_array(pair.elements[0].as_expression()?)
        || symbol(model, pair.elements[1].as_expression()?) != Some(header) { return None; }
    let complete = statement_call(complete_s)?;
    if symbol(model, member(&complete.callee, "complete")?) != Some(observer) { return None; }
    arguments(complete, 0)?;
    let locals = [seed, header, stream, cursor, input];
    if locals.into_iter().collect::<BTreeSet<_>>().len() != locals.len() || locals.contains(&observer)
        || model.scoping.get_resolved_references(header).count() != 6 { return None; }
    Some(order)
}
