//! Streaming wheel sample protocol with two independently proved numeric writes.
use super::super::encoding::curried_seeded_emitter as seeded;
use crate::naming::support::media_entropy::*;
use crate::naming::support::ordered_numeric::{eq, n, op, s};
use super::*;
fn inc(model: &Model<'_, '_>, site: FunctionSite, st: &Statement<'_>) -> Option<Symbol> {
    let Expression::UpdateExpression(u) = expression_statement(st)?.without_parentheses() else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
        return None;
    };
    if !u.prefix || u.operator.as_str() != "++" {
        return None;
    }
    capture_symbol(
        model,
        site,
        crate::analysis::identifier_symbol(id, model.scoping)?,
    )
}
fn prove(
    model: &Model<'_, '_>,
    site: FunctionSite,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<bool> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let b = f.body.as_ref()?;
    if f.r#async || f.generator || !b.directives.is_empty() || !plain_params(model, f)?.is_empty() {
        return None;
    }
    let [Statement::TryStatement(t)] = b.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [clock, Statement::IfStatement(threshold), nonce, header, reseed, first_slot, second_slot, third_slot, write, emit] =
        t.block.body.as_slice()
    else {
        return None;
    };
    let v = one_variable(clock)?;
    let now = plain_binding(model, v, Some(0))?;
    let clock = call(v.init.as_ref()?)?;
    if !clock.arguments.is_empty()
        || !crate::naming::support::ordered_numeric::clock_origin(model, site, &clock.callee)
    {
        return None;
    }
    let [increment, save] = block(&threshold.consequent)? else {
        return None;
    };
    if threshold.alternate.is_some() {
        return None;
    }
    let slow = inc(model, site, increment)?;
    let save = assignment(save)?;
    let previous = capture_symbol(model, site, assigned_symbol(model, &save.left)?)?;
    if symbol(model, &save.right) != Some(now)
        || !eq(
            model,
            &threshold.test,
            op(">", op("-", s(now), s(previous)), n(500.0)),
        )
    {
        return None;
    }
    let v = one_variable(nonce)?;
    let seed = plain_binding(model, v, Some(1))?;
    let raw = v.init.as_ref()?;
    let (draw, integer_nonce) = if let Some((draw, zero)) = binary(raw, "|") {
        if !number(zero, 0.0) { return None; }
        (draw, true)
    } else { (raw, false) };
    let Some((random, scale)) = binary(draw, "*") else {
        return None;
    };
    let random = call(random)?;
    if !number(scale, 4294967296.0)
        || !random.arguments.is_empty()
        || expression_path(model, &random.callee) != ["Math", "random"]
    {
        return None;
    }
    let v = one_variable(header)?;
    let bytes = plain_binding(model, v, Some(0))?;
    if !seeded::big_endian_header(model, v.init.as_ref()?, seed) {
        return None;
    }
    let a = assignment(reseed)?;
    if assigned_symbol(model, &a.left) != Some(seed) {
        return None;
    }
    let factory = call(&a.right)?;
    let args = call_arguments(factory);
    if args.len() != 1 || args.len() != factory.arguments.len() {
        return None;
    }
    let (base, key) = binary(args[0], "^")?;
    if !(eq(model, base, op("|", s(seed), n(0.0))) || integer_nonce && symbol(model, base) == Some(seed))
        || !matches!(key.without_parentheses(), Expression::NumericLiteral(_))
    {
        return None;
    }
    seeded::factory_role(model, site, &factory.callee, proposals, "nextKeystreamByte")?;
    // Two closed effect orders are supported. The second reuses its initial
    // writer-alias slot as the cursor only after evaluating that exact alias.
    let first_v = one_variable(first_slot)?;
    let alias_first = symbol(model, first_v.init.as_ref()?).is_some();
    let (writer, cursor, slow, total) = if alias_first {
        let cursor = plain_binding(model, first_v, Some(1))?;
        let writer_expr = first_v.init.as_ref()?;
        let writer = seeded::numeric_writer(model, site, writer_expr, proposals)?;
        let total = inc(model, site, second_slot)?;
        let assigned = assignment(third_slot)?;
        if assigned_symbol(model, &assigned.left) != Some(cursor) { return None; }
        let first = call(&assigned.right)?;
        let args = call_arguments(first);
        if symbol(model,&first.callee) != Some(cursor) || args.len()!=4 || args.len()!=first.arguments.len()
            || symbol(model,args[0])!=Some(seed) || symbol(model,args[1])!=Some(total)
            || symbol(model,args[2])!=Some(bytes) || !number(args[3],3.0) { return None; }
        let second = call(expression_statement(write)?)?;
        let args = call_arguments(second);
        if symbol(model,&second.callee)!=Some(writer) || args.len()!=4 || args.len()!=second.arguments.len()
            || symbol(model,args[0])!=Some(seed) || symbol(model,args[1])!=Some(slow)
            || symbol(model,args[2])!=Some(bytes) || symbol(model,args[3])!=Some(cursor) { return None; }
        (writer,cursor,slow,total)
    } else {
        let cursor = plain_binding(model, first_v, Some(0))?;
        let first = call(first_v.init.as_ref()?)?;
        let writer = seeded::numeric_writer(model, site, &first.callee, proposals)?;
        let args = call_arguments(first);
        if args.len()!=4 || args.len()!=first.arguments.len()
            || symbol(model,args[0])!=Some(seed) || symbol(model,args[1])!=Some(slow)
            || symbol(model,args[2])!=Some(bytes) || !number(args[3],3.0) { return None; }
        let v = one_variable(second_slot)?;
        let alias = plain_binding(model,v,Some(0))?;
        if symbol(model,v.init.as_ref()?)!=Some(writer) { return None; }
        let total=inc(model,site,third_slot)?;
        let second=call(expression_statement(write)?)?;
        let args=call_arguments(second);
        if symbol(model,&second.callee)!=Some(alias) || args.len()!=4 || args.len()!=second.arguments.len()
            || symbol(model,args[0])!=Some(seed) || symbol(model,args[1])!=Some(total)
            || symbol(model,args[2])!=Some(bytes) || symbol(model,args[3])!=Some(cursor) { return None; }
        (writer,cursor,slow,total)
    };
    let _ = cursor;
    // Plain immutable local alias was tied above to the exact source-proved writer.
    let emit = call(expression_statement(emit)?)?;
    let sink = capture(model, site, member(&emit.callee, "next")?)?;
    let args = call_arguments(emit);
    if args.len() != 1 || args.len() != emit.arguments.len() {
        return None;
    }
    let Expression::ArrayExpression(frame) = args[0].without_parentheses() else {
        return None;
    };
    let [empty, data] = frame.elements.as_slice() else {
        return None;
    };
    if !empty_array(empty.as_expression()?) || symbol(model, data.as_expression()?) != Some(bytes) {
        return None;
    }
    let h = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(err) = &h.param.as_ref()?.pattern else {
        return None;
    };
    let [report, Statement::IfStatement(remove)] = h.body.body.as_slice() else {
        return None;
    };
    let report = call(expression_statement(report)?)?;
    let args = call_arguments(report);
    if args.len() != 1
        || args.len() != report.arguments.len()
        || symbol(model, args[0]) != err.symbol_id.get()
        || symbol(model, member(&report.callee, "error")?) != Some(sink)
        || remove.alternate.is_some()
    {
        return None;
    }
    let (handler, null) = binary(&remove.test, "!=")?;
    let handler = capture(model, site, handler)?;
    if !null_literal(null) {
        return None;
    }
    let [st] = block(&remove.consequent)? else {
        return None;
    };
    let call = call(expression_statement(st)?)?;
    let host = capture(model, site, member(&call.callee, "removeEventListener")?)?;
    let args = call_arguments(call);
    if args.len() != 3
        || args.len() != call.arguments.len()
        || !string(args[0], "wheel")
        || symbol(model, args[1]) != Some(handler)
        || !boolean(args[2], false)
    {
        return None;
    }
    let distinct: BTreeSet<_> = [slow, previous, total, sink, handler, host, writer]
        .into_iter()
        .collect();
    (distinct.len() == 7).then_some(alias_first)
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if let Some(alias_first) = prove(model, site, proposals) {
        if alias_first { return vec![proposal("emitWheelCounterFrame", "events_wheel_counter_frame_writer_cursor", "complete wheel callback with unchanged scoped clock/factory/writer proofs: update 500ms gap count and timestamp; pack exact 32-bit nonce header; derive keystream; initialize writer alias, increment total, consume that alias before replacing its slot by the first numeric-write cursor, then write gap count with that exact cursor; next empty payload/frame; catch forwards the same error then removes the same captured wheel listener with false capture flag", f)]; }
        vec![proposal("emitWheelCounterFrame","events_wheel_counter_frame","complete wheel callback: update 500ms gap count and timestamp, pack random 32-bit big-endian nonce, derive owned keystream from coerced nonce XOR literal, independently prove shared number writer and same writer alias, write gap count then incremented total into chained frame, next empty payload/frame; catch forwards error then removes captured wheel listener with false capture flag",f)]
    } else {
        Vec::new()
    }
}
