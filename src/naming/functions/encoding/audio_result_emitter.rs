//! Closed scalar-prefix/reverse-for and scalar-trailer/reverse-while protocols. The input's
//! getChannelData method is opaque: a native AudioBuffer origin is unproved.
//! No byte equivalence with another ordering or salt is claimed.
use crate::naming::support::interaction::*;
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

fn facts<'a>(model: &Model<'_, 'a>, target: FunctionSite) -> Facts<'a> {
    let mut f = Facts::default();
    for n in model.nodes.iter() {
        if model.owning_site(n.id()).map(|s| s.span) != Some(target.span) {
            continue;
        }
        match n.kind() {
            AstKind::CallExpression(c) => f.calls.push(c),
            AstKind::ReturnStatement(r) => {
                if let Some(e) = &r.argument {
                    f.returns.push(e);
                }
            }
            AstKind::StaticMemberExpression(m) => {
                f.members.insert(m.property.name.to_string());
            }
            AstKind::AssignmentExpression(a) => f.assignments.push(a),
            AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_) => f.loops += 1,
            AstKind::ThrowStatement(_) => f.throws += 1,
            AstKind::NewExpression(_) => f.news += 1,
            _ => (),
        }
    }
    if let AstKind::Function(fun) = model.nodes.kind(target.node_id) {
        f.statements = fun.body.as_ref().map_or(0, |b| b.statements.len());
    }
    f
}

// Data identity only: this does not assign a native role or purpose to a capture.
// A direct, unconditional declaration in its own invocation precedes the owned
// callback, has an initializer and no rebinding. Conditional/loop setup fails.
fn capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<SymbolId> {
    let s = captured_symbol(model, site, symbol(model, e)?)?;
    let d = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(d) else {
        return None;
    };
    let container = model.nodes.parent_id(model.nodes.parent_id(d));
    if !matches!(model.nodes.kind(container), AstKind::FunctionBody(_)) {
        return None;
    }
    let value = model.symbol_value(s)?;
    if value.span().end >= site.span.start {
        return None;
    }
    match (v.init.is_some(), model.binding_writes(s)) {
        (true, 0) => (),
        (false, 1) => {
            // A fresh invocation-local data slot, assigned exactly once by an
            // unconditional direct sibling statement before registration.
            let valid = model.scoping.get_resolved_references(s).any(|r| {
                let parent = model.nodes.parent_id(r.node_id());
                matches!(model.nodes.kind(parent), AstKind::AssignmentExpression(a)
                if a.operator.as_str() == "=" && a.right.span() == value.span())
                    && matches!(
                        model.nodes.parent_kind(parent),
                        AstKind::ExpressionStatement(_)
                    )
                    && model.nodes.parent_id(model.nodes.parent_id(parent)) == container
            });
            if !valid {
                return None;
            }
        }
        _ => return None,
    }
    Some(s)
}

fn random(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<()> {
    let (mul, zero) = binary(e, "|")?;
    let (draw, scale) = binary(mul, "*")?;
    let c = call(draw)?;
    (number(zero, 0.0)
        && number(scale, 4294967296.0)
        && c.arguments.is_empty()
        && member(&c.callee, "random").is_some()
        && expression_path(model, &c.callee) == ["Math", "random"])
    .then_some(())
}

fn header(model: &Model<'_, '_>, e: &Expression<'_>, seed: SymbolId) -> Option<()> {
    let Expression::ArrayExpression(a) = e else {
        return None;
    };
    let p: Vec<_> = a
        .elements
        .iter()
        .map(|e| e.as_expression())
        .collect::<Option<_>>()?;
    if p.len() != 4 {
        return None;
    }
    let (v, n) = binary(p[0], ">>>")?;
    if symbol(model, v) != Some(seed) || !number(n, 24.0) {
        return None;
    }
    for (e, shift) in [(p[1], Some(16.0)), (p[2], Some(8.0)), (p[3], None)] {
        let (v, mask) = binary(e, "&")?;
        if !number(mask, 255.0) {
            return None;
        }
        let v = if let Some(shift) = shift {
            let (v, n) = binary(v, ">>")?;
            if !number(n, shift) {
                return None;
            }
            v
        } else {
            v
        };
        if symbol(model, v) != Some(seed) {
            return None;
        }
    }
    Some(())
}

fn indexed<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}

fn local(model: &Model<'_, '_>, s: &Statement<'_>) -> Option<SymbolId> {
    plain_var(model, variable(s)?)
}
fn init<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    variable(s)?.init.as_ref()
}

fn references(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: SymbolId,
    count: usize,
    writes: usize,
) -> bool {
    model.binding_writes(s) == writes
        && model.scoping.symbol_redeclarations(s).is_empty()
        && model.scoping.get_resolved_references(s).count() == count
        && model
            .scoping
            .get_resolved_references(s)
            .all(|r| model.owning_site(r.node_id()).map(|s| s.span) == Some(site.span))
}

fn proof(
    model: &Model<'_, '_>,
    site: FunctionSite,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    let AstKind::Function(fun) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = fun.body.as_ref()?;
    if fun.r#async
        || fun.generator
        || !body.directives.is_empty()
        || fun.params.rest.is_some()
        || fun.params.items.len() != 1
    {
        return None;
    }
    // Refuse hoisted registration/use before this initialized lexical context.
    let binding = site.binding_symbol?;
    if model.binding_writes(binding) != 0
        || model
            .scoping
            .get_resolved_references(binding)
            .any(|r| model.nodes.kind(r.node_id()).span().start < site.span.end)
    {
        return None;
    }
    let input = plain_param(model, &fun.params.items[0])?;
    let statements = body.statements.as_slice();
    if statements.len() != 11 {
        return None;
    }
    let trailer = matches!(statements[3], Statement::ExpressionStatement(_));
    let (channel, nonce, bytes, stream, cursor, convert, index, length, loop_, next, complete) =
        if trailer {
            (
                &statements[0],
                &statements[1],
                &statements[2],
                &statements[3],
                &statements[6],
                &statements[4],
                &statements[5],
                &statements[6],
                &statements[7],
                &statements[9],
                &statements[10],
            )
        } else {
            (
                &statements[0],
                &statements[1],
                &statements[2],
                &statements[3],
                &statements[4],
                &statements[5],
                &statements[6],
                &statements[7],
                &statements[8],
                &statements[9],
                &statements[10],
            )
        };
    let samples = local(model, channel)?;
    let channel_call = call(init(channel)?)?;
    let args = arguments(channel_call)?;
    if method_receiver(model, channel_call, "getChannelData") != Some(input)
        || args.len() != 1
        || !number(args[0], 0.0)
    {
        return None;
    }
    let seed = local(model, nonce)?;
    random(model, init(nonce)?)?;
    let output = local(model, bytes)?;
    header(model, init(bytes)?, seed)?;
    let (stream_symbol, factory) = if trailer {
        let a = assignment(expression_statement(stream)?)?;
        if target_symbol(model, &a.left) != Some(seed) {
            return None;
        }
        (seed, call(&a.right)?)
    } else {
        (local(model, stream)?, call(init(stream)?)?)
    };
    let args = arguments(factory)?;
    if args.len() != 1 {
        return None;
    }
    let (seed_arg, salt) = binary(args[0], "^")?;
    if symbol(model, seed_arg) != Some(seed) || !matches!(salt, Expression::NumericLiteral(_)) {
        return None;
    }
    let factory_site = crate::naming::resolve::ordered_callable(model, &factory.callee, 0)?;
    let claim = crate::naming::shared::evaluate_shared(
        model,
        factory_site,
        &facts(model, factory_site),
        proposals,
    )?;
    if claim.name != "createKeystreamByteSource" || claim.evaluator != "shared_scoped_role" {
        return None;
    }
    // Retain the existing owned returned-helper and live seed-to-byte proof.
    super::curried_seeded_emitter::factory_role(
        model,
        site,
        &factory.callee,
        proposals,
        "nextKeystreamByte",
    )?;
    let cursor_symbol = local(model, cursor)?;
    let first = call(init(cursor)?)?;
    let a = arguments(first)?;
    if a.len() != 4
        || symbol(model, a[0]) != Some(stream_symbol)
        || symbol(model, a[2]) != Some(output)
        || !number(a[3], 3.0)
    {
        return None;
    }
    let scalar_call = if trailer {
        call(expression_statement(&statements[8])?)?
    } else {
        first
    };
    let scalar_args = arguments(scalar_call)?;
    if scalar_args.len() != 4 {
        return None;
    }
    let (scalar, zero) = indexed(scalar_args[1])?;
    if !number(zero, 0.0) {
        return None;
    }
    let scalar = capture(model, site, scalar)?;
    let writer = crate::naming::resolve::ordered_callable(model, &first.callee, 0).or_else(|| crate::naming::support::callable_identity::captured_callable(model, site, &first.callee))?;
    if !super::numeric_writer::evaluate(model, writer, &facts(model, writer), proposals)
        .iter()
        .any(|p| p.name == "writeNumber" && p.evaluator == "thread_posthost_numeric")
    {
        return None;
    }
    let convert = assignment(expression_statement(convert)?)?;
    if target_symbol(model, &convert.left) != Some(samples) {
        return None;
    }
    let convert = call(&convert.right)?;
    let a = arguments(convert)?;
    if member(&convert.callee, "from").is_none()
        || expression_path(model, &convert.callee) != ["Array", "from"]
        || a.len() != 1
        || symbol(model, a[0]) != Some(samples)
    {
        return None;
    }
    let counter = local(model, index)?;
    // A countdown may begin at length and decrement before its guard. This
    // consumes a different closed loop grammar; no expression is rewritten.
    let predecrement = member(init(index)?, "length").is_some();
    let len = if predecrement { init(index)? } else {
        let (len, one) = binary(init(index)?, "-")?;
        if !number(one, 1.0) { return None; }
        len
    };
    if symbol(model, member(len, "length")?) != Some(samples) {
        return None;
    }
    let length_call = if trailer {
        first
    } else {
        let length_write = assignment(expression_statement(length)?)?;
        if target_symbol(model, &length_write.left) != Some(cursor_symbol) {
            return None;
        }
        call(&length_write.right)?
    };
    let a = arguments(length_call)?;
    if a.len() != 4 || !(if predecrement {
        symbol(model, a[1]) == Some(counter)
    } else { symbol(model, member(a[1], "length")?) == Some(samples) }) {
        return None;
    }
    let (test, update, step) = if predecrement {
        let Statement::ForStatement(loop_) = loop_ else { return None; };
        if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() { return None; }
        let Statement::BlockStatement(block) = &loop_.body else { return None; };
        let [decrement, Statement::IfStatement(guard), step, coercion] = block.body.as_slice() else { return None; };
        if guard.alternate.is_some() || !matches!(&guard.consequent, Statement::BreakStatement(b) if b.label.is_none()) { return None; }
        let Expression::UnaryExpression(negation) = &guard.test else { return None; };
        if negation.operator.as_str() != "!" { return None; }
        let coerce = assignment(expression_statement(coercion)?)?;
        let numeric = call(&coerce.right)?;
        let args = arguments(numeric)?;
        if target_symbol(model, &coerce.left) != Some(counter)
            || expression_path(model, &numeric.callee) != ["Number"]
            || args.len() != 1 || symbol(model, args[0]) != Some(counter) { return None; }
        (&negation.argument, expression_statement(decrement)?, step)
    } else if trailer {
        let Statement::WhileStatement(loop_) = loop_ else {
            return None;
        };
        let Statement::BlockStatement(block) = &loop_.body else {
            return None;
        };
        let [step, decrement] = block.body.as_slice() else {
            return None;
        };
        (&loop_.test, expression_statement(decrement)?, step)
    } else {
        let Statement::ForStatement(loop_) = loop_ else {
            return None;
        };
        if loop_.init.is_some() {
            return None;
        }
        let Statement::BlockStatement(block) = &loop_.body else {
            return None;
        };
        let [step] = block.body.as_slice() else {
            return None;
        };
        (loop_.test.as_ref()?, loop_.update.as_ref()?, step)
    };
    let (test, zero) = binary(test, ">=")?;
    if symbol(model, test) != Some(counter) || !number(zero, 0.0) {
        return None;
    }
    let Expression::UpdateExpression(update) = update else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &update.argument else {
        return None;
    };
    if update.operator.as_str() != "--"
        || crate::analysis::identifier_symbol(id, model.scoping) != Some(counter)
    {
        return None;
    }
    let step = assignment(expression_statement(step)?)?;
    if target_symbol(model, &step.left) != Some(cursor_symbol) {
        return None;
    }
    let step_call = call(&step.right)?;
    let a = arguments(step_call)?;
    if a.len() != 4 {
        return None;
    }
    let (array, offset) = indexed(a[1])?;
    if symbol(model, array) != Some(samples) || symbol(model, offset) != Some(counter) {
        return None;
    }
    for c in [length_call, step_call, scalar_call] {
        let a = arguments(c)?;
        if a.len() != 4
            || symbol(model, &c.callee) != symbol(model, &first.callee)
            || crate::naming::resolve::ordered_callable(model, &c.callee, 0).or_else(|| crate::naming::support::callable_identity::captured_callable(model, site, &c.callee))?.span != writer.span
            || symbol(model, a[0]) != Some(stream_symbol)
            || symbol(model, a[2]) != Some(output)
            || !(if c.span == first.span {
                number(a[3], 3.0)
            } else {
                symbol(model, a[3]) == Some(cursor_symbol)
            })
        {
            return None;
        }
    }
    let next = call(expression_statement(next)?)?;
    let observer_expr = member(&next.callee, "next")?;
    let observer = capture(model, site, observer_expr)?;
    let a = arguments(next)?;
    if a.len() != 1 {
        return None;
    }
    let Expression::ArrayExpression(frame) = a[0] else {
        return None;
    };
    if frame.elements.len() != 2 {
        return None;
    }
    if !matches!(frame.elements[0].as_expression()?, Expression::ArrayExpression(a) if a.elements.is_empty())
        || symbol(model, frame.elements[1].as_expression()?) != Some(output)
    {
        return None;
    }
    let complete = call(expression_statement(complete)?)?;
    if !complete.arguments.is_empty()
        || method_receiver(model, complete, "complete") != Some(observer)
    {
        return None;
    }
    let locals = if trailer {
        vec![
            (input, 1, 0),
            (samples, if predecrement { 4 } else { 5 }, 1),
            (seed, 9, 1),
            (output, 4, 0),
            (cursor_symbol, 3, 1),
            (counter, if predecrement { 6 } else { 3 }, if predecrement { 2 } else { 1 }),
        ]
    } else {
        vec![
            (input, 1, 0),
            (samples, if predecrement { 4 } else { 5 }, 1),
            (seed, 5, 0),
            (output, 4, 0),
            (stream_symbol, 3, 0),
            (cursor_symbol, 4, 2),
            (counter, if predecrement { 6 } else { 3 }, if predecrement { 2 } else { 1 }),
        ]
    };
    let distinct: BTreeSet<_> = locals.iter().map(|(s, _, _)| *s).collect();
    if distinct.len() != locals.len()
        || distinct.contains(&observer)
        || distinct.contains(&scalar)
        || !locals
            .into_iter()
            .all(|(s, n, w)| references(model, site, s, n, w))
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if proof(model, site, proposals).is_none() {
        return Vec::new();
    }
    if let AstKind::Function(fun) = model.nodes.kind(site.node_id) {
        if let Some(body) = fun.body.as_ref() {
            if body.statements.iter().any(|st| matches!(st, Statement::ForStatement(loop_) if loop_.init.is_none() && loop_.test.is_none() && loop_.update.is_none())) {
                let order = if matches!(body.statements[3], Statement::ExpressionStatement(_)) { "length, reverse channel samples, scalar[0]" } else { "scalar[0], length, reverse channel samples" };
                return vec![proposal("emitSeededChannelResult", "encoding_audio_result_emitter", &format!("whole channel-zero/Array.from result emission in exact order {order}; fresh big-endian random header and source-proven numeric writer/live seed factory; descending for-loop decrements before the guard and retains its explicit Number(index) store; exact cursor chaining, then next([[],bytes]) and complete on the same initialized capture; native AudioBuffer identity and external purpose explicitly unproved"), f)];
            }
        }
    }
    vec![proposal("emitSeededChannelResult", "encoding_audio_result_emitter",
        "owned channel-zero read and unshadowed Array.from of that result; exact fresh Math.random four-byte header; source-proven writer and factory-owned live seed-to-byte stream; closed scalar[0]/length/reverse-for and length/reverse-while/scalar[0] orders through one cursor, then next([[],bytes]) and complete on the same initialized capture; native AudioBuffer origin and external purpose explicitly unproved; no wire equivalence claimed", f)]
}
