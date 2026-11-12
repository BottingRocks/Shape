use super::*;

use crate::naming::support::media_entropy::*;

fn plus_parts<'a>(e: &'a Expression<'a>, parts: &mut Vec<&'a Expression<'a>>) {
    if let Some((left, right)) = binary(e, "+") {
        plus_parts(left, parts);
        parts.push(right);
        return;
    }
    parts.push(e);
}

/// `(a,b,c,d,e) => b.call(a, c + d + e + ")").matches` applied to
/// `(formal, formal.matchMedia, "(" + feature + ": ", candidate, unit)`.
fn media_probe(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    input: Symbol,
    feature: Symbol,
    unit: Symbol,
    candidate: Symbol,
) -> bool {
    let Some(c) = call(e) else {
        return false;
    };
    let Expression::ArrowFunctionExpression(arrow) = c.callee.without_parentheses() else {
        return false;
    };
    if arrow.r#async || !arrow.body.directives.is_empty() {
        return false;
    }
    let Some(probe_params) = plain_formals(model, &arrow.params) else {
        return false;
    };
    let [a, b, pc, pd, pe] = probe_params.as_slice() else {
        return false;
    };
    let [Statement::ReturnStatement(ret)] = arrow.body.statements.as_slice() else {
        return false;
    };
    let Some(matches_object) = ret.argument.as_ref().and_then(|r| member(r, "matches")) else {
        return false;
    };
    let Some(invoked) = call(matches_object) else {
        return false;
    };
    let Some(receiver) = member(&invoked.callee, "call") else {
        return false;
    };
    if symbol(model, receiver) != Some(*b) || invoked.arguments.len() != 2 {
        return false;
    }
    let invoked_args = call_arguments(invoked);
    if symbol(model, invoked_args[0]) != Some(*a) {
        return false;
    }
    let mut query = Vec::new();
    plus_parts(invoked_args[1], &mut query);
    if query.len() != 4
        || symbol(model, query[0]) != Some(*pc)
        || symbol(model, query[1]) != Some(*pd)
        || symbol(model, query[2]) != Some(*pe)
        || !string(query[3], ")")
    {
        return false;
    }
    if c.arguments.len() != 5 {
        return false;
    }
    let applied = call_arguments(c);
    if symbol(model, applied[0]) != Some(input)
        || !member(applied[1], "matchMedia")
            .is_some_and(|receiver| symbol(model, receiver) == Some(input))
    {
        return false;
    }
    let mut prefix = Vec::new();
    plus_parts(applied[2], &mut prefix);
    if prefix.len() != 3
        || !string(prefix[0], "(")
        || symbol(model, prefix[1]) != Some(feature)
        || !string(prefix[2], ": ")
        || symbol(model, applied[3]) != Some(candidate)
        || symbol(model, applied[4]) != Some(unit)
    {
        return false;
    }
    true
}

fn media_bisection_base(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let params = plain_params(model, function)?;
    let [input, feature, start, fallback] = params.as_slice() else {
        return None;
    };
    let [unit_init, low_init, attempts_init, result_init, high_init, Statement::ForStatement(grow), limit_init, Statement::WhileStatement(narrow), end] =
        body.statements.as_slice()
    else {
        return None;
    };
    let unit_variable = one_variable(unit_init)?;
    let unit = plain_binding(model, unit_variable, Some(0))?;
    let Expression::ConditionalExpression(unit_default) =
        unit_variable.init.as_ref()?.without_parentheses()
    else {
        return None;
    };
    let Some((unit_test, unit_alternate)) = binary(&unit_default.test, "===") else {
        return None;
    };
    if symbol(model, unit_test) != Some(*fallback)
        || !void_zero(unit_alternate)
        || !string(&unit_default.consequent, "")
        || symbol(model, &unit_default.alternate) != Some(*fallback)
    {
        return None;
    }
    let low_variable = one_variable(low_init)?;
    let low = plain_binding(model, low_variable, Some(1))?;
    if symbol(model, low_variable.init.as_ref()?) != Some(*start) {
        return None;
    }
    let attempts_variable = one_variable(attempts_init)?;
    let attempts = plain_binding(model, attempts_variable, Some(1))?;
    if !number(attempts_variable.init.as_ref()?, 50.0) {
        return None;
    }
    let result_variable = one_variable(result_init)?;
    let result = plain_binding(model, result_variable, Some(2))?;
    if !number(result_variable.init.as_ref()?, 0.0) {
        return None;
    }
    let high_variable = one_variable(high_init)?;
    let high = plain_binding(model, high_variable, Some(2))?;
    if high_variable.init.is_some() {
        return None;
    }
    if grow.init.is_some() || grow.test.is_some() || grow.update.is_some() {
        return None;
    }
    let grow_body = block(&grow.body)?;
    let [Statement::ExpressionStatement(seed), Statement::IfStatement(stop), attempts_step, Statement::ExpressionStatement(double), Statement::ExpressionStatement(previous)] =
        grow_body
    else {
        return None;
    };
    let seed = assignment_in(seed)?;
    if assigned_symbol(model, &seed.left) != Some(high) || symbol(model, &seed.right) != Some(low) {
        return None;
    }
    let Some((probe, attempts_left)) = logical(&stop.test, "||") else {
        return None;
    };
    if stop.alternate.is_some() || !media_probe(model, probe, *input, *feature, unit, high) {
        return None;
    }
    match &stop.consequent {
        Statement::BreakStatement(b) if b.label.is_none() => {}
        Statement::BlockStatement(b)
            if matches!(b.body.as_slice(),
                [Statement::BreakStatement(inner)] if inner.label.is_none()) => {}
        _ => return None,
    }
    let Some((attempts_expr, zero)) = binary(attempts_left, "<=") else {
        return None;
    };
    if symbol(model, attempts_expr) != Some(attempts) || !number(zero, 0.0) {
        return None;
    }
    if !update_target(model, attempts_step, attempts, "--") {
        return None;
    }
    let double = assignment_in(double)?;
    let Some((scaled, factor)) = binary(&double.right, "*") else {
        return None;
    };
    if assigned_symbol(model, &double.left) != Some(low)
        || symbol(model, scaled) != Some(high)
        || !number(factor, 2.0)
    {
        return None;
    }
    let previous = assignment_in(previous)?;
    if assigned_symbol(model, &previous.left) != Some(result)
        || symbol(model, &previous.right) != Some(high)
    {
        return None;
    }
    let limit_variable = one_variable(limit_init)?;
    let limit = plain_binding(model, limit_variable, Some(1))?;
    if !number(limit_variable.init.as_ref()?, 50.0) {
        return None;
    }
    let Some((bracket, budget)) = logical(&narrow.test, "&&") else {
        return None;
    };
    let Some((bracket_left, bracket_right)) = binary(bracket, "<") else {
        return None;
    };
    if symbol(model, bracket_left) != Some(result)
        || symbol(model, bracket_right) != Some(high)
        || !matches!(binary(budget, ">"), Some((left, right))
            if symbol(model, left) == Some(limit) && number(right, 0.0))
    {
        return None;
    }
    let narrow_body = block(&narrow.body)?;
    let [limit_step, middle_init, Statement::IfStatement(branch)] = narrow_body else {
        return None;
    };
    if !update_target(model, limit_step, limit, "--") {
        return None;
    }
    let middle_variable = one_variable(middle_init)?;
    let middle = plain_binding(model, middle_variable, Some(0))?;
    let Some(floor) = call(middle_variable.init.as_ref()?) else {
        return None;
    };
    if expression_path(model, &floor.callee) != ["Math", "floor"] || floor.arguments.len() != 1 {
        return None;
    }
    let floor_args = call_arguments(floor);
    let Some((sum, divisor)) = binary(floor_args[0], "/") else {
        return None;
    };
    if !number(divisor, 2.0) {
        return None;
    }
    let Some((left, right)) = binary(sum, "+") else {
        return None;
    };
    if symbol(model, left) != Some(result) || symbol(model, right) != Some(high) {
        return None;
    }
    if !media_probe(model, &branch.test, *input, *feature, unit, middle)
        || branch.alternate.is_none()
    {
        return None;
    }
    let yes = block(&branch.consequent)?;
    let [Statement::ExpressionStatement(set_high), Statement::ContinueStatement(again)] = yes
    else {
        return None;
    };
    let set_high = assignment_in(set_high)?;
    if assigned_symbol(model, &set_high.left) != Some(high)
        || symbol(model, &set_high.right) != Some(middle)
        || again.label.is_some()
    {
        return None;
    }
    let no = block(branch.alternate.as_ref()?)?;
    let [Statement::ExpressionStatement(set_low), Statement::ContinueStatement(again)] = no else {
        return None;
    };
    let set_low = assignment_in(set_low)?;
    let Some((next, one)) = binary(&set_low.right, "+") else {
        return None;
    };
    if assigned_symbol(model, &set_low.left) != Some(result)
        || symbol(model, next) != Some(middle)
        || !number(one, 1.0)
        || again.label.is_some()
        || symbol(model, returned(end)?) != Some(result)
    {
        return None;
    }
    Some(())
}

/// Defaulted unit local: `unit = fallback === void 0 ? "" : fallback`.
fn media_unit(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    fallback: Symbol,
) -> Option<Symbol> {
    let variable = one_variable(statement)?;
    let unit = plain_binding(model, variable, Some(0))?;
    let Expression::ConditionalExpression(default) = variable.init.as_ref()?.without_parentheses()
    else {
        return None;
    };
    let (test, missing) = binary(&default.test, "===")?;
    if symbol(model, test) != Some(fallback)
        || !void_zero(missing)
        || !string(&default.consequent, "")
        || symbol(model, &default.alternate) != Some(fallback)
    {
        return None;
    }
    Some(unit)
}

/// `low = start` seed for the smallest matching media-feature value.
fn media_low(model: &Model<'_, '_>, statement: &Statement<'_>, start: Symbol) -> Option<Symbol> {
    let variable = one_variable(statement)?;
    let low = plain_binding(model, variable, Some(1))?;
    (symbol(model, variable.init.as_ref()?) == Some(start)).then_some(low)
}

fn media_numeric_binding(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    value: f64,
    writes: usize,
) -> Option<Symbol> {
    let variable = one_variable(statement)?;
    if !number(variable.init.as_ref()?, value) {
        return None;
    }
    plain_binding(model, variable, Some(writes))
}

/// The `result = 0` and `attempts = 50` seeds, whose declaration order is a
/// lowering choice and not an identity.
fn media_result_attempts(
    model: &Model<'_, '_>,
    first: &Statement<'_>,
    second: &Statement<'_>,
) -> Option<(Symbol, Symbol)> {
    if let Some(result) = media_numeric_binding(model, first, 0.0, 2) {
        let attempts = media_numeric_binding(model, second, 50.0, 1)?;
        return Some((result, attempts));
    }
    let attempts = media_numeric_binding(model, first, 50.0, 1)?;
    let result = media_numeric_binding(model, second, 0.0, 2)?;
    Some((result, attempts))
}

/// A concatenation of string literals and the three captured query values.
#[derive(Clone, PartialEq, Eq)]
enum QueryPart {
    Text(String),
    Value(Symbol),
}

/// Resolve a query expression to its literal/feature/candidate/unit sequence.
/// `bindings` supplies the already-resolved applied value for every probe or
/// accessory-builder parameter; `outer` is the member's own
/// `(feature, candidate, unit)` identity. Any other symbol is refused, and an
/// accessory builder arrow is accepted only as a pure `+` concatenation of its
/// own parameters.
fn flatten_query(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    bindings: &BTreeMap<Symbol, Vec<QueryPart>>,
    outer: (Symbol, Symbol, Symbol),
) -> Option<Vec<QueryPart>> {
    if let Some((left, right)) = binary(e, "+") {
        let mut parts = flatten_query(model, left, bindings, outer)?;
        parts.extend(flatten_query(model, right, bindings, outer)?);
        return Some(parts);
    }
    if let Expression::StringLiteral(text) = e.without_parentheses() {
        return Some(vec![QueryPart::Text(text.value.as_str().to_owned())]);
    }
    if let Some(sym) = symbol(model, e) {
        if let Some(parts) = bindings.get(&sym) {
            return Some(parts.clone());
        }
        if sym == outer.0 || sym == outer.1 || sym == outer.2 {
            return Some(vec![QueryPart::Value(sym)]);
        }
        return None;
    }
    let invocation = call(e)?;
    let Expression::ArrowFunctionExpression(arrow) = invocation.callee.without_parentheses() else {
        return None;
    };
    if arrow.r#async || !arrow.body.directives.is_empty() {
        return None;
    }
    let params = plain_formals(model, &arrow.params)?;
    let [Statement::ReturnStatement(ret)] = arrow.body.statements.as_slice() else {
        return None;
    };
    let args = call_arguments(invocation);
    if args.len() != params.len() {
        return None;
    }
    let mut inner = BTreeMap::new();
    for (param, arg) in params.iter().zip(args) {
        inner.insert(*param, flatten_query(model, arg, bindings, outer)?);
    }
    flatten_query(model, ret.argument.as_ref()?, &inner, outer)
}

/// The probe encodings in the family: the existing five-parameter
/// `(input, matchMedia, prefix, candidate, unit)` arrow, a three-parameter
/// arrow whose third parameter is the already-built query, and the accessory
/// five/six-parameter arrows that carry the prefix pieces as separate arguments.
/// In every case the resolved concatenation must be exactly
/// `"(" + feature + ": " + candidate + unit + ")"` and the applied media
/// receiver must be the member's own `input.matchMedia`.
fn media_probe_variant(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    input: Symbol,
    feature: Symbol,
    unit: Symbol,
    candidate: Symbol,
) -> bool {
    let Some(c) = call(e) else {
        return false;
    };
    let Expression::ArrowFunctionExpression(arrow) = c.callee.without_parentheses() else {
        return false;
    };
    if arrow.r#async || !arrow.body.directives.is_empty() {
        return false;
    }
    let Some(probe_params) = plain_formals(model, &arrow.params) else {
        return false;
    };
    if !matches!(probe_params.len(), 3 | 5 | 6) {
        return false;
    }
    let [Statement::ReturnStatement(ret)] = arrow.body.statements.as_slice() else {
        return false;
    };
    let Some(matches_object) = ret.argument.as_ref().and_then(|r| member(r, "matches")) else {
        return false;
    };
    let Some(invoked) = call(matches_object) else {
        return false;
    };
    let Some(receiver) = member(&invoked.callee, "call") else {
        return false;
    };
    if symbol(model, receiver) != Some(probe_params[1]) {
        return false;
    }
    let invoked_args = call_arguments(invoked);
    if invoked.arguments.len() != 2
        || invoked_args.len() != 2
        || symbol(model, invoked_args[0]) != Some(probe_params[0])
    {
        return false;
    }
    let applied = call_arguments(c);
    if c.arguments.len() != probe_params.len() || applied.len() != probe_params.len() {
        return false;
    }
    if symbol(model, applied[0]) != Some(input)
        || !member(applied[1], "matchMedia")
            .is_some_and(|receiver| symbol(model, receiver) == Some(input))
    {
        return false;
    }
    let outer = (feature, candidate, unit);
    let empty = BTreeMap::new();
    let mut bindings = BTreeMap::new();
    for index in 2..probe_params.len() {
        let parts = match flatten_query(model, applied[index], &empty, outer) {
            Some(parts) => parts,
            None => return false,
        };
        bindings.insert(probe_params[index], parts);
    }
    let expected = vec![
        QueryPart::Text("(".to_owned()),
        QueryPart::Value(feature),
        QueryPart::Text(": ".to_owned()),
        QueryPart::Value(candidate),
        QueryPart::Value(unit),
        QueryPart::Text(")".to_owned()),
    ];
    flatten_query(model, invoked_args[1], &bindings, outer) == Some(expected)
}

fn media_double_update(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    low: Symbol,
    high: Symbol,
) -> bool {
    matches!(assignment(statement), Some(a)
        if assigned_symbol(model, &a.left) == Some(low)
            && matches!(binary(&a.right, "*"), Some((scaled, factor))
                if symbol(model, scaled) == Some(high) && number(factor, 2.0)))
}

fn media_result_update(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    result: Symbol,
    high: Symbol,
) -> bool {
    matches!(assignment(statement), Some(a)
        if assigned_symbol(model, &a.left) == Some(result)
            && symbol(model, &a.right) == Some(high))
}

/// `while (result < high && limit > 0) { --limit; let middle =
/// Math.floor((result + high) / 2); if (probe(middle)) high = middle; else
/// result = middle + 1; } return result;`
fn media_search_loop(
    model: &Model<'_, '_>,
    narrow: &WhileStatement<'_>,
    input: Symbol,
    feature: Symbol,
    unit: Symbol,
    result: Symbol,
    high: Symbol,
    limit: Symbol,
    end: &Statement<'_>,
) -> Option<()> {
    let (bracket, budget) = logical(&narrow.test, "&&")?;
    let (bracket_left, bracket_right) = binary(bracket, "<")?;
    if symbol(model, bracket_left) != Some(result)
        || symbol(model, bracket_right) != Some(high)
        || !matches!(binary(budget, ">"), Some((left, right))
            if symbol(model, left) == Some(limit) && number(right, 0.0))
    {
        return None;
    }
    let narrow_body = block(&narrow.body)?;
    let [limit_step, middle_init, Statement::IfStatement(branch)] = narrow_body else {
        return None;
    };
    if !update_target(model, limit_step, limit, "--") {
        return None;
    }
    let middle_variable = one_variable(middle_init)?;
    let middle = plain_binding(model, middle_variable, Some(0))?;
    let floor = call(middle_variable.init.as_ref()?)?;
    if expression_path(model, &floor.callee) != ["Math", "floor"] || floor.arguments.len() != 1 {
        return None;
    }
    let floor_args = call_arguments(floor);
    if floor_args.len() != 1 {
        return None;
    }
    let (sum, divisor) = binary(floor_args[0], "/")?;
    if !number(divisor, 2.0) {
        return None;
    }
    let (left, right) = binary(sum, "+")?;
    if symbol(model, left) != Some(result) || symbol(model, right) != Some(high) {
        return None;
    }
    if !media_probe_variant(model, &branch.test, input, feature, unit, middle)
        || branch.alternate.is_none()
    {
        return None;
    }
    let yes = block(&branch.consequent)?;
    let [Statement::ExpressionStatement(set_high), Statement::ContinueStatement(again)] = yes
    else {
        return None;
    };
    let set_high = assignment_in(set_high)?;
    if assigned_symbol(model, &set_high.left) != Some(high)
        || symbol(model, &set_high.right) != Some(middle)
        || again.label.is_some()
    {
        return None;
    }
    let no = block(branch.alternate.as_ref()?)?;
    let [Statement::ExpressionStatement(set_low), Statement::ContinueStatement(again)] = no else {
        return None;
    };
    let set_low = assignment_in(set_low)?;
    let (next, one) = binary(&set_low.right, "+")?;
    if assigned_symbol(model, &set_low.left) != Some(result)
        || symbol(model, next) != Some(middle)
        || !number(one, 1.0)
        || again.label.is_some()
        || symbol(model, returned(end)?) != Some(result)
    {
        return None;
    }
    Some(())
}

/// S1: bounded `do { high = low; cond = !probe(high); } while (cond &&
/// attempts > 0 && (tmp = Number(attempts) - 1, low = high * 2,
/// result = high, attempts = tmp, true))`, where `high`, `cond` and `tmp` are
/// declared together without initializers. The dependency-free `low`/`result`
/// writes may appear in either order, but the write that computes `tmp` must
/// precede the `attempts = tmp` read of that temporary.
fn media_bisection_do_while(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    input: Symbol,
    feature: Symbol,
    start: Symbol,
    fallback: Symbol,
) -> Option<()> {
    let [unit_init, low_init, first, second, temporaries_init, Statement::DoWhileStatement(grow), limit_init, Statement::WhileStatement(narrow), end] =
        body.statements.as_slice()
    else {
        return None;
    };
    let unit = media_unit(model, unit_init, fallback)?;
    let low = media_low(model, low_init, start)?;
    let (result, attempts) = media_result_attempts(model, first, second)?;
    let grow_body = block(&grow.body)?;
    let [Statement::ExpressionStatement(seed), Statement::ExpressionStatement(condition)] =
        grow_body
    else {
        return None;
    };
    let seed = assignment_in(seed)?;
    let high = assigned_symbol(model, &seed.left)?;
    if symbol(model, &seed.right) != Some(low) {
        return None;
    }
    let condition = assignment_in(condition)?;
    let cond = assigned_symbol(model, &condition.left)?;
    let Expression::UnaryExpression(negated) = condition.right.without_parentheses() else {
        return None;
    };
    if negated.operator.as_str() != "!"
        || !media_probe_variant(model, &negated.argument, input, feature, unit, high)
    {
        return None;
    }
    let Statement::VariableDeclaration(declaration) = temporaries_init else {
        return None;
    };
    if declaration.declarations.len() != 3 {
        return None;
    }
    let mut declared = Vec::with_capacity(3);
    for variable in &declaration.declarations {
        if variable.init.is_some() {
            return None;
        }
        declared.push(plain_binding(model, variable, None)?);
    }
    if !declared.contains(&high)
        || !declared.contains(&cond)
        || model.binding_writes(high) != 2
        || model.binding_writes(cond) != 1
    {
        return None;
    }
    let (bounds, sequence) = logical(&grow.test, "&&")?;
    let (cond_read, attempt_bound) = logical(bounds, "&&")?;
    let (attempts_read, zero) = binary(attempt_bound, ">")?;
    if symbol(model, cond_read) != Some(cond)
        || symbol(model, attempts_read) != Some(attempts)
        || !number(zero, 0.0)
    {
        return None;
    }
    let Expression::SequenceExpression(sequence) = sequence.without_parentheses() else {
        return None;
    };
    if sequence.expressions.len() != 5 || !boolean(&sequence.expressions[4], true) {
        return None;
    }
    let mut double = false;
    let mut result_step = false;
    let mut attempts_step = false;
    let mut attempts_value = None;
    let mut attempts_at = None;
    let mut temporary = None;
    let mut temporary_at = None;
    for (index, expression) in sequence.expressions[..4].iter().enumerate() {
        let Expression::AssignmentExpression(assignment) = expression.without_parentheses() else {
            return None;
        };
        if assignment.operator.as_str() != "=" {
            return None;
        }
        let target = assigned_symbol(model, &assignment.left)?;
        if target == low {
            if double {
                return None;
            }
            double = true;
            let Some((scaled, factor)) = binary(&assignment.right, "*") else {
                return None;
            };
            if symbol(model, scaled) != Some(high) || !number(factor, 2.0) {
                return None;
            }
        } else if target == result {
            if result_step {
                return None;
            }
            result_step = true;
            if symbol(model, &assignment.right) != Some(high) {
                return None;
            }
        } else if target == attempts {
            if attempts_step || attempts_value.is_some() {
                return None;
            }
            attempts_step = true;
            attempts_value = symbol(model, &assignment.right);
            attempts_at = Some(index);
        } else {
            if temporary.is_some() {
                return None;
            }
            temporary = Some(target);
            temporary_at = Some(index);
            if !number_minus_one(model, &assignment.right, attempts) {
                return None;
            }
        }
    }
    let Some(temporary) = temporary else {
        return None;
    };
    let (Some(temporary_at), Some(attempts_at)) = (temporary_at, attempts_at) else {
        return None;
    };
    if !double
        || !result_step
        || !attempts_step
        || attempts_value != Some(temporary)
        || temporary_at >= attempts_at
        || !declared.contains(&temporary)
        || model.binding_writes(temporary) != 1
    {
        return None;
    }
    let limit_variable = one_variable(limit_init)?;
    let limit = plain_binding(model, limit_variable, Some(1))?;
    if !number(limit_variable.init.as_ref()?, 50.0) {
        return None;
    }
    media_search_loop(
        model,
        narrow.as_ref(),
        input,
        feature,
        unit,
        result,
        high,
        limit,
        end,
    )
}

/// `Number(attempts) - 1` as the lowered form of the decrement in a comma
/// sequence.
fn number_minus_one(model: &Model<'_, '_>, e: &Expression<'_>, attempts: Symbol) -> bool {
    let Some((left, right)) = binary(e, "-") else {
        return false;
    };
    if !number(right, 1.0) {
        return false;
    }
    let Some(c) = call(left) else {
        return false;
    };
    expression_path(model, &c.callee) == ["Number"]
        && c.arguments.len() == 1
        && call_arguments(c)
            .first()
            .is_some_and(|argument| symbol(model, *argument) == Some(attempts))
}

/// S2/S3: headerless `for(;;)` growth whose three updates may be lowered in
/// any order, with a standalone uninitialized `high` and a five/six-parameter
/// prefix-carrying probe.
fn media_bisection_for_variant(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    input: Symbol,
    feature: Symbol,
    start: Symbol,
    fallback: Symbol,
) -> Option<()> {
    let [unit_init, low_init, first, second, high_init, Statement::ForStatement(grow), limit_init, Statement::WhileStatement(narrow), end] =
        body.statements.as_slice()
    else {
        return None;
    };
    let unit = media_unit(model, unit_init, fallback)?;
    let low = media_low(model, low_init, start)?;
    let (result, attempts) = media_result_attempts(model, first, second)?;
    let high_variable = one_variable(high_init)?;
    let high = plain_binding(model, high_variable, Some(2))?;
    if high_variable.init.is_some() {
        return None;
    }
    if grow.init.is_some() || grow.test.is_some() || grow.update.is_some() {
        return None;
    }
    let grow_body = block(&grow.body)?;
    let [Statement::ExpressionStatement(seed), Statement::IfStatement(stop), first_step, second_step, third_step] =
        grow_body
    else {
        return None;
    };
    let seed = assignment_in(seed)?;
    if assigned_symbol(model, &seed.left) != Some(high) || symbol(model, &seed.right) != Some(low) {
        return None;
    }
    let (probe, attempts_left) = logical(&stop.test, "||")?;
    if stop.alternate.is_some() || !media_probe_variant(model, probe, input, feature, unit, high) {
        return None;
    }
    match &stop.consequent {
        Statement::BreakStatement(b) if b.label.is_none() => {}
        Statement::BlockStatement(b)
            if matches!(b.body.as_slice(),
                [Statement::BreakStatement(inner)] if inner.label.is_none()) => {}
        _ => return None,
    }
    let (attempts_expr, zero) = binary(attempts_left, "<=")?;
    if symbol(model, attempts_expr) != Some(attempts) || !number(zero, 0.0) {
        return None;
    }
    let mut attempts_steps = 0;
    let mut doubles = 0;
    let mut results = 0;
    for step in [first_step, second_step, third_step] {
        if update_target(model, step, attempts, "--") {
            attempts_steps += 1;
        } else if media_double_update(model, step, low, high) {
            doubles += 1;
        } else if media_result_update(model, step, result, high) {
            results += 1;
        } else {
            return None;
        }
    }
    if attempts_steps != 1 || doubles != 1 || results != 1 {
        return None;
    }
    let limit_variable = one_variable(limit_init)?;
    let limit = plain_binding(model, limit_variable, Some(1))?;
    if !number(limit_variable.init.as_ref()?, 50.0) {
        return None;
    }
    media_search_loop(
        model,
        narrow.as_ref(),
        input,
        feature,
        unit,
        result,
        high,
        limit,
        end,
    )
}

/// S4: one headerless `for(;;)` whose body first grows the bracket and then
/// owns the binary-search `for(;;)`. The inner loop's three-way lowered
/// `$AX/$AY` temporaries are accepted only as the exact inert phi pair that
/// feeds the `result < high && limit > 0` test; the bracket, branch direction,
/// `middle + 1` step and returned lower bound stay identical.
fn media_bisection_nested(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    input: Symbol,
    feature: Symbol,
    start: Symbol,
    fallback: Symbol,
) -> Option<()> {
    let [unit_init, low_init, first, second, Statement::ForStatement(grow)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let unit = media_unit(model, unit_init, fallback)?;
    let low = media_low(model, low_init, start)?;
    let (result, attempts) = media_result_attempts(model, first, second)?;
    if grow.init.is_some() || grow.test.is_some() || grow.update.is_some() {
        return None;
    }
    let outer = block(&grow.body)?;
    let [high_init, Statement::IfStatement(probe_if), limit_init, temporary_init, Statement::ForStatement(search), end] =
        outer
    else {
        return None;
    };
    let high_variable = one_variable(high_init)?;
    let high = plain_binding(model, high_variable, Some(1))?;
    if symbol(model, high_variable.init.as_ref()?) != Some(low) {
        return None;
    }
    let (not_probe, attempts_left) = logical(&probe_if.test, "&&")?;
    let Expression::UnaryExpression(negated) = not_probe.without_parentheses() else {
        return None;
    };
    if negated.operator.as_str() != "!"
        || !media_probe_variant(model, &negated.argument, input, feature, unit, high)
    {
        return None;
    }
    let (attempts_read, zero) = binary(attempts_left, ">")?;
    if symbol(model, attempts_read) != Some(attempts) || !number(zero, 0.0) {
        return None;
    }
    if probe_if.alternate.is_some() {
        return None;
    }
    let grow_body = block(&probe_if.consequent)?;
    let [attempts_step, double, previous, Statement::ContinueStatement(again)] = grow_body else {
        return None;
    };
    if !update_target(model, attempts_step, attempts, "--")
        || !media_double_update(model, double, low, high)
        || !media_result_update(model, previous, result, high)
        || again.label.is_some()
    {
        return None;
    }
    let limit_variable = one_variable(limit_init)?;
    let limit = plain_binding(model, limit_variable, Some(1))?;
    if !number(limit_variable.init.as_ref()?, 50.0) {
        return None;
    }
    let temporary_variable = one_variable(temporary_init)?;
    if temporary_variable.init.is_some() {
        return None;
    }
    let temporary = plain_binding(model, temporary_variable, Some(2))?;
    if search.init.is_some() || search.test.is_some() || search.update.is_some() {
        return None;
    }
    let search_body = block(&search.body)?;
    let [condition_init, Statement::IfStatement(condition_if), Statement::IfStatement(break_if), limit_step, middle_init, Statement::IfStatement(branch)] =
        search_body
    else {
        return None;
    };
    let Statement::VariableDeclaration(condition_declaration) = condition_init else {
        return None;
    };
    if condition_declaration.declarations.len() != 2 {
        return None;
    }
    let mut conditions = Vec::with_capacity(2);
    for variable in &condition_declaration.declarations {
        if variable.init.is_some() {
            return None;
        }
        conditions.push(plain_binding(model, variable, None)?);
    }
    let (bracket_left, bracket_right) = binary(&condition_if.test, "<")?;
    if symbol(model, bracket_left) != Some(result) || symbol(model, bracket_right) != Some(high) {
        return None;
    }
    let condition_yes = block(&condition_if.consequent)?;
    let [Statement::ExpressionStatement(cond_yes), Statement::ExpressionStatement(temp_yes)] =
        condition_yes
    else {
        return None;
    };
    let cond_yes = assignment_in(cond_yes)?;
    let cond = assigned_symbol(model, &cond_yes.left)?;
    let (budget_left, budget_right) = binary(&cond_yes.right, ">")?;
    if symbol(model, budget_left) != Some(limit) || !number(budget_right, 0.0) {
        return None;
    }
    let temp_yes = assignment_in(temp_yes)?;
    let temporary_value = assigned_symbol(model, &temp_yes.left)?;
    if !matches!(
        temp_yes.right.without_parentheses(),
        Expression::NumericLiteral(_)
    ) {
        return None;
    }
    let condition_no = block(condition_if.alternate.as_ref()?)?;
    let [Statement::ExpressionStatement(cond_no), Statement::ExpressionStatement(temp_no)] =
        condition_no
    else {
        return None;
    };
    let cond_no = assignment_in(cond_no)?;
    if assigned_symbol(model, &cond_no.left) != Some(cond) || !boolean(&cond_no.right, false) {
        return None;
    }
    let temp_no = assignment_in(temp_no)?;
    if assigned_symbol(model, &temp_no.left) != Some(temporary_value)
        || symbol(model, &temp_no.right) != Some(temporary)
        || !conditions.contains(&cond)
        || !conditions.contains(&temporary_value)
        || model.binding_writes(cond) != 2
        || model.binding_writes(temporary_value) != 2
    {
        return None;
    }
    if break_if.alternate.is_some() {
        return None;
    }
    let Expression::UnaryExpression(not_cond) = break_if.test.without_parentheses() else {
        return None;
    };
    if not_cond.operator.as_str() != "!" || symbol(model, &not_cond.argument) != Some(cond) {
        return None;
    }
    match &break_if.consequent {
        Statement::BreakStatement(b) if b.label.is_none() => {}
        Statement::BlockStatement(b)
            if matches!(b.body.as_slice(),
                [Statement::BreakStatement(inner)] if inner.label.is_none()) => {}
        _ => return None,
    }
    if !update_target(model, limit_step, limit, "--") {
        return None;
    }
    let middle_variable = one_variable(middle_init)?;
    let middle = plain_binding(model, middle_variable, Some(0))?;
    let floor = call(middle_variable.init.as_ref()?)?;
    if expression_path(model, &floor.callee) != ["Math", "floor"] || floor.arguments.len() != 1 {
        return None;
    }
    let floor_args = call_arguments(floor);
    if floor_args.len() != 1 {
        return None;
    }
    let (sum, divisor) = binary(floor_args[0], "/")?;
    if !number(divisor, 2.0) {
        return None;
    }
    let (left, right) = binary(sum, "+")?;
    if symbol(model, left) != Some(result) || symbol(model, right) != Some(high) {
        return None;
    }
    if !media_probe_variant(model, &branch.test, input, feature, unit, middle)
        || branch.alternate.is_none()
    {
        return None;
    }
    let yes = block(&branch.consequent)?;
    let [Statement::ExpressionStatement(set_high), Statement::ExpressionStatement(restore_yes), Statement::ContinueStatement(again)] =
        yes
    else {
        return None;
    };
    let set_high = assignment_in(set_high)?;
    if assigned_symbol(model, &set_high.left) != Some(high)
        || symbol(model, &set_high.right) != Some(middle)
    {
        return None;
    }
    let restore_yes = assignment_in(restore_yes)?;
    if assigned_symbol(model, &restore_yes.left) != Some(temporary)
        || symbol(model, &restore_yes.right) != Some(temporary_value)
        || again.label.is_some()
    {
        return None;
    }
    let no = block(branch.alternate.as_ref()?)?;
    let [Statement::ExpressionStatement(set_low), Statement::ExpressionStatement(restore_no), Statement::ContinueStatement(again)] =
        no
    else {
        return None;
    };
    let set_low = assignment_in(set_low)?;
    let (next, one) = binary(&set_low.right, "+")?;
    if assigned_symbol(model, &set_low.left) != Some(result)
        || symbol(model, next) != Some(middle)
        || !number(one, 1.0)
    {
        return None;
    }
    let restore_no = assignment_in(restore_no)?;
    if assigned_symbol(model, &restore_no.left) != Some(temporary)
        || symbol(model, &restore_no.right) != Some(temporary_value)
        || again.label.is_some()
        || symbol(model, returned(end)?) != Some(result)
    {
        return None;
    }
    Some(())
}

fn media_bisection(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let params = plain_params(model, function)?;
    let [input, feature, start, fallback] = params.as_slice() else {
        return None;
    };
    if media_bisection_base(model, function, body).is_some() {
        return Some(());
    }
    if media_bisection_do_while(model, body, *input, *feature, *start, *fallback).is_some() {
        return Some(());
    }
    if media_bisection_for_variant(model, body, *input, *feature, *start, *fallback).is_some() {
        return Some(());
    }
    if media_bisection_nested(model, body, *input, *feature, *start, *fallback).is_some() {
        return Some(());
    }
    None
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
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if function.params.items.len() == 4 && media_bisection(model, function, body).is_some() {
        claims.push(proposal(
            "measureMaxMediaFeatureValue",
            "runtime_media_feature_bisection",
            "doubles a candidate until a matchMedia probe with the exact \"(<feature>: <value><unit>)\" builder stops matching, binary-searches the smallest matching integer with Math.floor, and returns it; all loop counters and formal identities are proven",
            f,
        ));
    }
    claims
}
