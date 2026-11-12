use super::*;
use oxc_semantic::SymbolId;

use crate::naming::support::interaction::*;

// Resolve callable origins through the strict shared proof: every alias hop is
// a plain var/let/const before-use initializer or one direct assignment in the
// declaration's own FunctionBody/Program statement list. Parameters, catch
// bindings, destructuring, loops, resources, setup callbacks and conditional
// assignments never qualify, and no opaque data value is treated as callable.
fn stable_callable(
    model: &Model<'_, '_>,
    site: FunctionSite,
    c: &CallExpression<'_>,
) -> Option<FunctionSite> {
    if c.optional {
        return None;
    }
    let target = crate::naming::resolve::ordered_callable(model, &c.callee, 0)?;
    (target.span != site.span).then_some(target)
}

fn random_seed(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(or) = e else {
        return false;
    };
    if or.operator.as_str() != "|" || !number(&or.right, 0.0) {
        return false;
    }
    let Expression::BinaryExpression(mul) = &or.left else {
        return false;
    };
    if mul.operator.as_str() != "*" || !number(&mul.right, 4_294_967_296.0) {
        return false;
    }
    let Expression::CallExpression(random) = &mul.left else {
        return false;
    };
    !random.optional
        && random.arguments.is_empty()
        && expression_path(model, &random.callee) == ["Math", "random"]
}

fn header_byte(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    seed: SymbolId,
    shift: Option<f64>,
) -> bool {
    let Expression::BinaryExpression(b) = e else {
        return false;
    };
    if b.operator.as_str() != "&" || !number(&b.right, 255.0) {
        return false;
    }
    match shift {
        None => symbol(model, &b.left) == Some(seed),
        Some(shift) => matches!(binary(&b.left, ">>"), Some((left, right))
            if symbol(model, left) == Some(seed) && number(right, shift)),
    }
}

fn frame_header(model: &Model<'_, '_>, e: &Expression<'_>, seed: SymbolId) -> bool {
    let Expression::ArrayExpression(a) = e else {
        return false;
    };
    let parts: Option<Vec<_>> = a.elements.iter().map(|e| e.as_expression()).collect();
    let Some(parts) = parts else {
        return false;
    };
    parts.len() == 4
        && matches!(binary(parts[0], ">>>"), Some((left, right))
            if symbol(model, left) == Some(seed) && number(right, 24.0))
        && header_byte(model, parts[1], seed, Some(16.0))
        && header_byte(model, parts[2], seed, Some(8.0))
        && header_byte(model, parts[3], seed, None)
}

fn seed_xor(model: &Model<'_, '_>, e: &Expression<'_>, seed: SymbolId) -> bool {
    matches!(binary(e, "^"), Some((left, right)) if symbol(model, left) == Some(seed) && matches!(right, Expression::NumericLiteral(_)))
}

fn reseed_call<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &'a Expression<'a>,
    seed: SymbolId,
) -> Option<&'a CallExpression<'a>> {
    let c = call(e)?;
    stable_callable(model, site, c)?;
    let args = arguments(c)?;
    (args.len() == 1 && seed_xor(model, args[0], seed)).then_some(c)
}

#[derive(Clone, Copy)]
enum WriterArgument {
    Initial,
    Chain(SymbolId),
}

fn writer_argument(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    stream: SymbolId,
    header: SymbolId,
) -> Option<WriterArgument> {
    let c = call(e)?;
    stable_callable(model, site, c)?;
    let args = arguments(c)?;
    if args.len() != 4
        || symbol(model, args[0]) != Some(stream)
        || symbol(model, args[2]) != Some(header)
    {
        return None;
    }
    if number(args[3], 3.0) {
        Some(WriterArgument::Initial)
    } else {
        symbol(model, args[3]).map(WriterArgument::Chain)
    }
}

fn emit_seeded_signal_frame(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.params.rest.is_some() || function.params.items.len() != 1 {
        return None;
    }
    plain_param(model, &function.params.items[0])?;
    let statements = body.statements.as_slice();
    let (complete_index, prefix) = statements.split_last()?;
    let (next_index, prefix) = prefix.split_last()?;
    let next = call(expression_statement(next_index)?)?;
    let complete = call(expression_statement(complete_index)?)?;
    let receiver = captured_symbol(model, site, method_receiver(model, next, "next")?)?;
    if complete.arguments.len() != 0
        || method_receiver(model, complete, "complete") != Some(receiver)
    {
        return None;
    }
    let payload = arguments(next)?;
    let [payload] = payload.as_slice() else {
        return None;
    };
    let Expression::ArrayExpression(frame) = payload else {
        return None;
    };
    if frame.elements.len() != 2 {
        return None;
    }
    let header = symbol(model, frame.elements[1].as_expression()?)?;
    let mut seed = None;
    let mut stream = None;
    let mut cursor = None;
    let mut header_seen = false;
    let mut reseed_seen = false;
    let mut writers = 0usize;
    let mut chained = 0usize;
    for statement in prefix {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                let variable = single_variable(declaration)?;
                let local = plain_var(model, variable)?;
                let init = variable.init.as_ref()?;
                if random_seed(model, init) {
                    if seed.replace(local).is_some() {
                        return None;
                    }
                    continue;
                }
                let Some(current) = seed else {
                    return None;
                };
                if local == header && frame_header(model, init, current) {
                    if header_seen {
                        return None;
                    }
                    header_seen = true;
                    continue;
                }
                if reseed_call(model, site, init, current).is_some() {
                    if reseed_seen {
                        return None;
                    }
                    reseed_seen = true;
                    stream = Some(local);
                    continue;
                }
                if stream.is_some() && cursor.is_none() {
                    if let Some(WriterArgument::Initial) =
                        writer_argument(model, site, init, stream?, header)
                    {
                        cursor = Some(local);
                        writers += 1;
                    }
                }
            }
            Statement::ExpressionStatement(statement) => {
                let expression = &statement.expression;
                if let (Some(current), Some(a)) = (seed, assignment(expression)) {
                    if target_symbol(model, &a.left) == Some(current)
                        && reseed_call(model, site, &a.right, current).is_some()
                    {
                        if reseed_seen {
                            return None;
                        }
                        reseed_seen = true;
                        stream = Some(current);
                        continue;
                    }
                }
                let (Some(current), Some(chain)) = (stream, cursor) else {
                    return None;
                };
                if let Some(a) = assignment(expression) {
                    if target_symbol(model, &a.left) == Some(chain)
                        && matches!(
                            writer_argument(model, site, &a.right, current, header),
                            Some(WriterArgument::Chain(c)) if c == chain
                        )
                    {
                        writers += 1;
                        chained += 1;
                        continue;
                    }
                    return None;
                }
                if matches!(
                    writer_argument(model, site, expression, current, header),
                    Some(WriterArgument::Chain(c)) if c == chain
                ) {
                    writers += 1;
                    chained += 1;
                    continue;
                }
                return None;
            }
            Statement::WhileStatement(loop_) => {
                let (Some(current), Some(chain)) = (stream, cursor) else {
                    return None;
                };
                let Statement::BlockStatement(block) = &loop_.body else {
                    return None;
                };
                let [Statement::ExpressionStatement(step), Statement::ExpressionStatement(update)] =
                    block.body.as_slice()
                else {
                    return None;
                };
                let Some(assignment_) = assignment(&step.expression) else {
                    return None;
                };
                if target_symbol(model, &assignment_.left) != Some(chain)
                    || !matches!(
                        writer_argument(model, site, &assignment_.right, current, header),
                        Some(WriterArgument::Chain(c)) if c == chain
                    )
                {
                    return None;
                }
                let Expression::UpdateExpression(update) = &update.expression else {
                    return None;
                };
                let SimpleAssignmentTarget::AssignmentTargetIdentifier(counter) = &update.argument
                else {
                    return None;
                };
                if update.operator.as_str() != "--"
                    || crate::analysis::identifier_symbol(counter, model.scoping).is_none()
                {
                    return None;
                }
                writers += 1;
                chained += 1;
            }
            _ => return None,
        }
    }
    let seed = seed?;
    let stream = stream?;
    let cursor = cursor?;
    if !header_seen {
        return None;
    }
    if !reseed_seen {
        return None;
    }
    if writers < 2 || chained == 0 {
        return None;
    }
    if receiver == seed || receiver == header || receiver == stream || receiver == cursor {
        return None;
    }
    Some(())
}

// This route is deliberately separate from the established flexible route.
// It accounts for every statement and reference in the array-before-nonce,
// reused-nonce spelling; it never relaxes the shared write/callable resolver.
fn local_references(
    model: &Model<'_, '_>,
    site: FunctionSite,
    local: SymbolId,
    count: usize,
    writes: usize,
) -> bool {
    model.binding_writes(local) == writes
        && model.scoping.symbol_redeclarations(local).is_empty()
        && model.scoping.get_resolved_references(local).count() == count
        && model
            .scoping
            .get_resolved_references(local)
            .all(|reference| {
                model
                    .owning_site(reference.node_id())
                    .is_some_and(|owner| owner.span == site.span)
            })
}

// Match the shared pass's mechanical facts for exactly this resolved callee.
// In particular, child calls/returns/loops cannot supply factory evidence.
fn callee_facts<'a>(model: &Model<'_, 'a>, target: FunctionSite) -> Facts<'a> {
    let mut facts = Facts::default();
    for node in model.nodes.iter() {
        if model.owning_site(node.id()).map(|owner| owner.span) != Some(target.span) {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call) => facts.calls.push(call),
            AstKind::ReturnStatement(return_) => {
                if let Some(value) = &return_.argument {
                    facts.returns.push(value);
                }
            }
            AstKind::StaticMemberExpression(member) => {
                facts.members.insert(member.property.name.to_string());
            }
            AstKind::AssignmentExpression(assignment) => facts.assignments.push(assignment),
            AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_) => facts.loops += 1,
            AstKind::ThrowStatement(_) => facts.throws += 1,
            AstKind::NewExpression(_) => facts.news += 1,
            _ => (),
        }
    }
    if let AstKind::Function(function) = model.nodes.kind(target.node_id) {
        facts.statements = function
            .body
            .as_ref()
            .map_or(0, |body| body.statements.len());
    }
    facts
}

fn reused_nonce_frame(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    if function.params.rest.is_some() || function.params.items.len() != 1 {
        return None;
    }
    let input = plain_param(model, &function.params.items[0])?;
    let [empty, nonce, header, reseed, length, initial, loop_, next, complete] =
        body.statements.as_slice()
    else {
        return None;
    };
    let empty = variable(empty)?;
    let payload = plain_var(model, empty)?;
    if !matches!(empty.init.as_ref()?, Expression::ArrayExpression(a) if a.elements.is_empty()) {
        return None;
    }
    let nonce = variable(nonce)?;
    let seed = plain_var(model, nonce)?;
    if !random_seed(model, nonce.init.as_ref()?) {
        return None;
    }
    let header = variable(header)?;
    let bytes = plain_var(model, header)?;
    if !frame_header(model, header.init.as_ref()?, seed) {
        return None;
    }
    let reseed = assignment(expression_statement(reseed)?)?;
    if target_symbol(model, &reseed.left) != Some(seed) {
        return None;
    }
    let factory = reseed_call(model, site, &reseed.right, seed)?;
    captured_symbol(model, site, symbol(model, &factory.callee)?)?;
    // The shared pass evaluates factories and emitters against one immutable
    // proposal snapshot, then publishes all claims. A factory role produced in
    // that pass is not available through role() here. Re-run its source proof
    // synchronously using ONLY the resolved factory's own mechanical facts.
    let factory_site = stable_callable(model, site, factory)?;
    let factory_facts = callee_facts(model, factory_site);
    let factory_claim =
        crate::naming::shared::evaluate_shared(model, factory_site, &factory_facts, proposals)?;
    if factory_claim.name != "createKeystreamByteSource"
        || factory_claim.evaluator != "shared_scoped_role"
    {
        return None;
    }
    // Integrator API glue: expose this existing proof to the threads subtree.
    // It proves owned returned-helper provenance AND formal-to-byte-state flow;
    // a donated helper or a factory that ignores/overwrites its seed fails.
    super::super::encoding::curried_seeded_emitter::factory_role(
        model,
        site,
        &factory.callee,
        proposals,
        "nextKeystreamByte",
    )?;

    let length = variable(length)?;
    let count = plain_var(model, length)?;
    if symbol(model, member(length.init.as_ref()?, "length")?) != Some(input) {
        return None;
    }
    let initial = variable(initial)?;
    let cursor = plain_var(model, initial)?;
    let first = call(initial.init.as_ref()?)?;
    if !matches!(
        writer_argument(model, site, initial.init.as_ref()?, seed, bytes),
        Some(WriterArgument::Initial)
    ) {
        return None;
    }
    let first_args = arguments(first)?;
    if symbol(model, first_args[1]) != Some(count) {
        return None;
    }
    captured_symbol(model, site, symbol(model, &first.callee)?)?;
    let writer = stable_callable(model, site, first)?;
    // Re-prove the resolved writer body, rather than accepting an arbitrary
    // four-argument function or an inherited/name-only proposal.
    let facts = callee_facts(model, writer);
    if !super::super::encoding::numeric_writer::evaluate(model, writer, &facts, proposals)
        .iter()
        .any(|claim| claim.name == "writeNumber" && claim.evaluator == "thread_posthost_numeric")
    {
        return None;
    }
    let Statement::WhileStatement(loop_) = loop_ else {
        return None;
    };
    let (remaining, zero) = binary(&loop_.test, ">=")?;
    let (counter, one) = binary(remaining, "-")?;
    if symbol(model, counter) != Some(count) || !number(one, 1.0) || !number(zero, 0.0) {
        return None;
    }
    let Statement::BlockStatement(block) = &loop_.body else {
        return None;
    };
    let [write, decrement] = block.body.as_slice() else {
        return None;
    };
    let write = assignment(expression_statement(write)?)?;
    if target_symbol(model, &write.left) != Some(cursor)
        || !matches!(writer_argument(model, site, &write.right, seed, bytes),
            Some(WriterArgument::Chain(chain)) if chain == cursor)
    {
        return None;
    }
    let repeated = call(&write.right)?;
    if symbol(model, &repeated.callee) != symbol(model, &first.callee)
        || stable_callable(model, site, repeated)?.span != writer.span
    {
        return None;
    }
    let repeated_args = arguments(repeated)?;
    let Expression::ComputedMemberExpression(item) = repeated_args[1] else {
        return None;
    };
    let (counter, one) = binary(&item.expression, "-")?;
    if item.optional
        || symbol(model, &item.object) != Some(input)
        || symbol(model, counter) != Some(count)
        || !number(one, 1.0)
    {
        return None;
    }
    let Expression::UpdateExpression(decrement) = expression_statement(decrement)? else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(counter) = &decrement.argument else {
        return None;
    };
    if !decrement.prefix
        || decrement.operator.as_str() != "--"
        || crate::analysis::identifier_symbol(counter, model.scoping) != Some(count)
    {
        return None;
    }
    let next = call(expression_statement(next)?)?;
    let complete = call(expression_statement(complete)?)?;
    let receiver = captured_symbol(model, site, method_receiver(model, next, "next")?)?;
    if !complete.arguments.is_empty()
        || method_receiver(model, complete, "complete") != Some(receiver)
    {
        return None;
    }
    let next_args = arguments(next)?;
    let [frame] = next_args.as_slice() else {
        return None;
    };
    let Expression::ArrayExpression(frame) = frame else {
        return None;
    };
    if frame.elements.len() != 2
        || symbol(model, frame.elements[0].as_expression()?) != Some(payload)
        || symbol(model, frame.elements[1].as_expression()?) != Some(bytes)
    {
        return None;
    }
    // The complete syntax above fixes each read/write site. Counting ALL scoped
    // references (including nested captures and outside uses) closes the liveness
    // proof: four header reads, one RHS numeric read, one callable write, then
    // only the two writer arguments. No later numeric read or hidden capture.
    let locals = [
        (input, 2, 0),
        (payload, 1, 0),
        (seed, 8, 1),
        (bytes, 3, 0),
        (count, 4, 1),
        (cursor, 2, 1),
    ];
    let distinct: BTreeSet<_> = locals.iter().map(|(local, _, _)| *local).collect();
    if distinct.len() != locals.len()
        || distinct.contains(&receiver)
        || !locals
            .into_iter()
            .all(|(local, count, writes)| local_references(model, site, local, count, writes))
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
    if emit_seeded_signal_frame(model, site, function, body).is_some()
        || reused_nonce_frame(model, site, function, body, proposals).is_some()
    {
        claims.push(proposal(
            "emitSeededSignalFrame",
            "signals_seeded_signal_frame",
            "computes a random 32-bit seed, packs it most-significant-byte first into a four-byte frame header, derives one stream state from seed XOR an exact numeric literal through an independently ordered plain var/let/const callable, forwards each field through the same independently ordered plain var/let/const writer with that header and a running cursor, then responds next([payload, header]) and complete(); no other statement kinds or writes",
            f,
        ));
    }
    if claims.is_empty() && captured_scalar_frame(model, site, function, body, proposals).is_some() {
        let mut claim = proposal("emitSeededCapturedScalarFrame", "signals_seeded_captured_scalar_frame", "Complete zero-formal body: keep Math.random()*2^32 as a Number, perform each source int32 coercion separately while packing the big-endian header, derive a factory-owned source from the same nonce XOR its exact literal, write the sole opaque captured scalar once at cursor3 through the independently source-proved numeric writer, discard its return, then next([[], sameHeader]) and complete on one captured emitter; all local references/writes and callback captures are closed; no native property-count purpose claimed", f);
        claim.evidence = vec![site.span.into()];
        claims.push(claim);
    }
    claims
}

fn scalar_integer_seed(model: &Model<'_, '_>, e: &Expression<'_>, seed: SymbolId) -> bool {
    matches!(binary(e.without_parentheses(), "|"), Some((left,right)) if symbol(model,left)==Some(seed) && number(right,0.0))
}

// This variant keeps the floating random sample intact and coerces each read.
// It never normalizes the sample to one early int32 store.
fn scalar_frame_header(model: &Model<'_, '_>, e: &Expression<'_>, seed: SymbolId) -> bool {
    let Expression::ArrayExpression(a)=e else{return false;};
    let Some(parts)=a.elements.iter().map(|e|e.as_expression()).collect::<Option<Vec<_>>>() else{return false;};
    if parts.len()!=4{return false;}
    let Some((first,shift))=binary(parts[0],">>>") else{return false;};
    if !scalar_integer_seed(model,first,seed)||!number(shift,24.0){return false;}
    for (part,shift) in [(parts[1],Some(16.0)),(parts[2],Some(8.0)),(parts[3],None)] {
        let Some((left,mask))=binary(part,"&") else{return false;};
        if !number(mask,255.0){return false;}
        if let Some(shift)=shift {
            let Some((left,right))=binary(left,">>") else{return false;};
            if !scalar_integer_seed(model,left,seed)||!number(right,shift){return false;}
        } else if !scalar_integer_seed(model,left,seed){return false;}
    }
    true
}

fn captured_scalar_frame(
    model:&Model<'_, '_>,site:FunctionSite,function:&Function<'_>,body:&FunctionBody<'_>,
    proposals:&BTreeMap<u32,Vec<Proposal>>,
)->Option<()> {
    if function.params.rest.is_some()||!function.params.items.is_empty(){return None;}
    let [nonce,header,write,next,complete]=body.statements.as_slice() else{return None;};
    let nonce=variable(nonce)?;let seed=plain_var(model,nonce)?;
    let (random,scale)=binary(nonce.init.as_ref()?,"*")?;
    let random=call(random)?;
    if !number(scale,4294967296.0)||!random.arguments.is_empty()||expression_path(model,&random.callee)!=["Math","random"]{return None;}
    let header=variable(header)?;let bytes=plain_var(model,header)?;
    if !scalar_frame_header(model,header.init.as_ref()?,seed){return None;}
    let write=call(expression_statement(write)?)?;let a=arguments(write)?;
    let [factory,data,target,cursor]=a.as_slice() else{return None;};
    if symbol(model,target)!=Some(bytes)||!number(cursor,3.0){return None;}
    let data=captured_symbol(model,site,symbol(model,data)?)?;
    let factory=call(factory)?;let factory_args=arguments(factory)?;
    let [xor]=factory_args.as_slice() else{return None;};
    let (nonce,literal)=binary(xor,"^")?;
    if !scalar_integer_seed(model,nonce,seed)||!matches!(literal,Expression::NumericLiteral(n) if n.value.is_finite() && n.value.fract()==0.0 && n.value>=0.0 && n.value<=4294967295.0){return None;}
    captured_symbol(model,site,symbol(model,&factory.callee)?)?;
    let factory_site=stable_callable(model,site,factory)?;
    let factory_facts=callee_facts(model,factory_site);
    let factory_claim=crate::naming::shared::evaluate_shared(model,factory_site,&factory_facts,proposals)?;
    if factory_claim.name!="createKeystreamByteSource"||factory_claim.evaluator!="shared_scoped_role"{return None;}
    super::super::encoding::curried_seeded_emitter::factory_role(model,site,&factory.callee,proposals,"nextKeystreamByte")?;
    captured_symbol(model,site,symbol(model,&write.callee)?)?;
    let writer=stable_callable(model,site,write)?;let writer_facts=callee_facts(model,writer);
    if !super::super::encoding::numeric_writer::evaluate(model,writer,&writer_facts,proposals).iter().any(|claim|claim.name=="writeNumber"&&claim.evaluator=="thread_posthost_numeric"){return None;}
    if writer.span==factory_site.span{return None;}
    let next=call(expression_statement(next)?)?;let complete=call(expression_statement(complete)?)?;
    let receiver=captured_symbol(model,site,method_receiver(model,next,"next")?)?;
    if !complete.arguments.is_empty()||method_receiver(model,complete,"complete")!=Some(receiver){return None;}
    let a=arguments(next)?;let [frame]=a.as_slice() else{return None;};
    let Expression::ArrayExpression(frame)=frame else{return None;};
    let [payload,header]=frame.elements.as_slice() else{return None;};
    if !matches!(payload.as_expression()?,Expression::ArrayExpression(a) if a.elements.is_empty())||symbol(model,header.as_expression()?)!=Some(bytes){return None;}
    if [seed,bytes,data,receiver].into_iter().collect::<BTreeSet<_>>().len()!=4
        ||!local_references(model,site,seed,5,0)||!local_references(model,site,bytes,2,0){return None;}
    Some(())
}

#[cfg(test)]
mod scalar_frame_tests {
    use super::*;
    const SOURCE:&str=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/fixtures/program/observable-setup-order/ordinary-parent.js"));
    fn check(source:&str,expected:bool){
        let result=crate::naming::name_threads(source,true).unwrap();
        let outcome=result.report.outcomes.iter().find(|o|o.binding.name=="$THREAD_1_121688").unwrap();
        assert_eq!(outcome.evaluator.as_deref()==Some("signals_seeded_captured_scalar_frame"),expected,"{:?}",outcome.reason);
    }
    #[test]
    fn captured_scalar_frame_full_dataflow_and_mutations(){
        check(SOURCE,true);check(&SOURCE.replace("$KEY_22","capturedScalar"),true);
        let begin=SOURCE.find("function $THREAD_1_121688()").unwrap();let finish=begin+SOURCE[begin..].find("$KEY_0.complete();").unwrap()+"$KEY_0.complete();".len();let own=&SOURCE[begin..finish];
        for (from,to) in [("($DA | 0) >>> 24","($DA | 1) >>> 24"),("$KEY_1, 3)","$KEY_1, 4)"),("$KEY_0.next([[], $KEY_1]);","$KEY_0.next([[], []]);"),("$KEY_0.complete();","extra(); $KEY_0.complete();")] {
            assert!(own.contains(from));let changed=format!("{}{}{}",&SOURCE[..begin],own.replace(from,to),&SOURCE[finish..]);check(&changed,false);
        }
    }
}
