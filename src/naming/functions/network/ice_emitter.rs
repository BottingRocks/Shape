//! Bounded owned-body proof for the ICE-candidate seeded field emitter.
//!
//! This is the WebRTC-candidate variant of the reviewed seeded signal-frame
//! emitter.  One plain formal is null-guarded, one `Math.random() * 2^32` value
//! (optionally `| 0`) is packed most-significant-byte first into a four-byte
//! correlation header, one stream state is reseeded through `seed ^ <number>`
//! and used to derive one writer, the candidate-list length and then every
//! candidate's `ip` and `candidate` strings are mixed through that writer with
//! the same header and a running cursor, and the emitter finally answers
//! `next([[], header])` and `complete()` on one captured observer.
//!
//! Two verified loop forms are accepted: a forward `for (; i < formal.length;
//! i++)` counter and a descending `for (;;) { --count; if (!(count >= 0))
//! break; ... count = Number(count); }` counter.  The two field names may be
//! emitted in either order.  Nothing else varies.
//!
//! Every compared value is a scoped symbol.  This rule reads no generated
//! spelling, vendor name, span or hash, does not rename anything, and runs only
//! in the last post-host stage after every earlier source-backed evaluator has
//! declined.  It has no outer-container fallback and never claims audio,
//! canvas or probe bodies.

use super::*;
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

const NAME: &str = "emitSeededHashEncoding";
const EVALUATOR: &str = "thread_posthost_ice_emitter";

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn computed<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn number(e: &Expression<'_>, expected: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(v) if v.value == expected)
}

fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn expression_statement<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}

fn statement_call<'a>(s: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    call(expression_statement(s)?)
}

fn assignment<'a>(e: &'a Expression<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Expression::AssignmentExpression(a) = e.without_parentheses() else {
        return None;
    };
    (a.operator.as_str() == "=").then_some(a)
}

fn assigned_symbol(model: &Model<'_, '_>, a: &AssignmentExpression<'_>) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
        return None;
    };
    crate::analysis::identifier_symbol(id, model.scoping)
}

fn update_symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    prefix: bool,
    operator: &str,
) -> Option<Symbol> {
    let Expression::UpdateExpression(u) = e.without_parentheses() else {
        return None;
    };
    if u.prefix != prefix || u.operator.as_str() != operator {
        return None;
    }
    match &u.argument {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
            crate::analysis::identifier_symbol(id, model.scoping)
        }
        _ => None,
    }
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

fn one_variable<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}

/// A plain `var`/`let`/`const` declarator bound to exactly this symbol.
fn plain_local(model: &Model<'_, '_>, v: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let s = id.symbol_id.get()?;
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(declared) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&declared.id, BindingPattern::BindingIdentifier(d)
        if d.symbol_id.get() == Some(s))
    {
        return None;
    }
    if !matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(d)
            if matches!(
                d.kind,
                VariableDeclarationKind::Var
                    | VariableDeclarationKind::Let
                    | VariableDeclarationKind::Const
            )
    ) {
        return None;
    }
    Some(s)
}

/// A captured data receiver: a plain ancestor declarator outside the owned
/// body, distinct from every local read by the proof.
fn captured_symbol(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> Option<Symbol> {
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
        || !matches!(&variable.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    Some(s)
}

fn method_receiver(model: &Model<'_, '_>, c: &CallExpression<'_>, method: &str) -> Option<Symbol> {
    let Expression::StaticMemberExpression(m) = c.callee.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == method).then(|| symbol(model, &m.object))?
}

fn arity<'a>(c: &'a CallExpression<'a>, count: usize) -> Option<Vec<&'a Expression<'a>>> {
    let args = call_arguments(c);
    (args.len() == count && c.arguments.len() == count).then_some(args)
}

fn empty_array(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::ArrayExpression(a) if a.elements.is_empty())
}

/// An unshadowed global namespace path: a bound root is never a host global.
fn global_path(model: &Model<'_, '_>, e: &Expression<'_>) -> Vec<String> {
    match e.without_parentheses() {
        Expression::Identifier(i) => {
            if crate::analysis::identifier_symbol(i, model.scoping).is_some() {
                Vec::new()
            } else {
                vec![i.name.to_string()]
            }
        }
        Expression::StaticMemberExpression(m) if !m.optional => {
            let mut path = global_path(model, &m.object);
            if !path.is_empty() {
                path.push(m.property.name.to_string());
            }
            path
        }
        _ => Vec::new(),
    }
}

fn is_global(model: &Model<'_, '_>, e: &Expression<'_>, path: &[&str]) -> bool {
    global_path(model, e) == path
}

/// `seed` or `(seed | 0)`.
fn seed_base(model: &Model<'_, '_>, e: &Expression<'_>, seed: Symbol) -> bool {
    if symbol(model, e) == Some(seed) {
        return true;
    }
    matches!(binary(e, "|"), Some((left, right))
        if number(right, 0.0) && symbol(model, left) == Some(seed))
}

/// `Math.random() * 4294967296`, optionally `| 0`.
fn random_seed(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let base = match binary(e, "|") {
        Some((left, right)) if number(right, 0.0) => left,
        _ => e.without_parentheses(),
    };
    let Some((rand, scale)) = binary(base, "*") else {
        return false;
    };
    if !number(scale, 4_294_967_296.0) {
        return false;
    }
    call(rand)
        .is_some_and(|c| c.arguments.is_empty() && is_global(model, &c.callee, &["Math", "random"]))
}

fn header_slot(model: &Model<'_, '_>, e: &Expression<'_>, seed: Symbol, slot: usize) -> bool {
    match slot {
        0 => matches!(binary(e, ">>>"), Some((left, right))
            if seed_base(model, left, seed) && number(right, 24.0)),
        1 | 2 => {
            let shift = if slot == 1 { 16.0 } else { 8.0 };
            matches!(binary(e, "&"), Some((left, right))
                if number(right, 255.0)
                    && matches!(binary(left, ">>"), Some((shifted, by))
                        if seed_base(model, shifted, seed) && number(by, shift)))
        }
        3 => matches!(binary(e, "&"), Some((left, right))
            if number(right, 255.0) && seed_base(model, left, seed)),
        _ => false,
    }
}

/// The exact four-byte most-significant-byte-first header of `seed`.
fn header_bytes(model: &Model<'_, '_>, e: &Expression<'_>, seed: Symbol) -> bool {
    let Expression::ArrayExpression(array) = e.without_parentheses() else {
        return false;
    };
    let mut slots = Vec::new();
    for element in &array.elements {
        let Some(expression) = element.as_expression() else {
            return false;
        };
        slots.push(expression);
    }
    slots.len() == 4
        && slots
            .iter()
            .enumerate()
            .all(|(slot, expression)| header_slot(model, expression, seed, slot))
}

/// `mix(seed[|0] ^ <number>)` through one independently ordered callable.
fn reseeded(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    seed: Symbol,
) -> Option<()> {
    let c = call(e)?;
    crate::naming::resolve::ordered_callable(model, &c.callee, 0)?;
    let args = arity(c, 1)?;
    let (left, right) = binary(args[0], "^")?;
    (seed_base(model, left, seed)
        && matches!(right.without_parentheses(), Expression::NumericLiteral(_)))
    .then_some(())
}

/// A writer factory call over the established stream state.
fn writer_derivation(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    stream: Symbol,
) -> Option<()> {
    let c = call(e)?;
    crate::naming::resolve::ordered_callable(model, &c.callee, 0)?;
    let args = arity(c, 1)?;
    (symbol(model, args[0]) == Some(stream)).then_some(())
}

/// The four-argument length/header mix; returns the length operand.
fn length_mix<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    c: &'a CallExpression<'a>,
    stream: Symbol,
    header: Symbol,
) -> Option<&'a Expression<'a>> {
    crate::naming::resolve::ordered_callable(model, &c.callee, 0)?;
    let args = arity(c, 4)?;
    (symbol(model, args[0]) == Some(stream)
        && symbol(model, args[2]) == Some(header)
        && number(args[3], 3.0))
    .then_some(args[1])
}

/// One `stream = writer(formal[index].ip|.candidate, header, stream)` step.
fn field_step<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    writer: Symbol,
    stream: Symbol,
    header: Symbol,
    input: Symbol,
    index: Symbol,
) -> Option<&'a str> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    // `ExpressionStatement` owns its inner expression; the assignment proof is
    // still over that exact node, never over a generated spelling.
    let assignment_ = assignment(&expression.expression)?;
    if assigned_symbol(model, assignment_) != Some(stream) {
        return None;
    }
    let c = call(&assignment_.right)?;
    if symbol(model, &c.callee) != Some(writer) {
        return None;
    }
    let args = arity(c, 3)?;
    let Expression::StaticMemberExpression(field) = args[0].without_parentheses() else {
        return None;
    };
    if field.optional {
        return None;
    }
    let name = field.property.name.as_str();
    if name != "ip" && name != "candidate" {
        return None;
    }
    let (object, slot) = computed(&field.object)?;
    if symbol(model, object) != Some(input) || symbol(model, slot) != Some(index) {
        return None;
    }
    (symbol(model, args[1]) == Some(header) && symbol(model, args[2]) == Some(stream))
        .then_some(name)
}

/// Both ICE field names appear exactly once, in either order.
fn both_fields(
    model: &Model<'_, '_>,
    first: &Statement<'_>,
    second: &Statement<'_>,
    writer: Symbol,
    stream: Symbol,
    header: Symbol,
    input: Symbol,
    index: Symbol,
) -> Option<()> {
    let first = field_step(model, first, writer, stream, header, input, index)?;
    let second = field_step(model, second, writer, stream, header, input, index)?;
    ((first == "ip" && second == "candidate") || (first == "candidate" && second == "ip"))
        .then_some(())
}

/// `for (; i < formal.length; i++) { ... }` over the ICE fields.
fn forward_loop(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    writer: Symbol,
    stream: Symbol,
    header: Symbol,
    input: Symbol,
    index: Symbol,
) -> Option<()> {
    let Statement::ForStatement(loop_) = statement else {
        return None;
    };
    if loop_.init.is_some() {
        return None;
    }
    let (bound, length) = binary(loop_.test.as_ref()?, "<")?;
    let receiver = member(length, "length")?;
    if symbol(model, bound) != Some(index) || symbol(model, receiver) != Some(input) {
        return None;
    }
    if loop_
        .update
        .as_ref()
        .and_then(|e| update_symbol(model, e, false, "++"))
        != Some(index)
    {
        return None;
    }
    let [first, second] = block(&loop_.body)? else {
        return None;
    };
    both_fields(model, first, second, writer, stream, header, input, index)
}

/// `for (;;) { --count; if (!(count >= 0)) break; ...; count = Number(count); }`.
fn reverse_loop(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    writer: Symbol,
    stream: Symbol,
    header: Symbol,
    input: Symbol,
    index: Symbol,
) -> Option<()> {
    let Statement::ForStatement(loop_) = statement else {
        return None;
    };
    if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() {
        return None;
    }
    let [decrement, exit, first, second, renumber] = block(&loop_.body)? else {
        return None;
    };
    let Statement::ExpressionStatement(decrement) = decrement else {
        return None;
    };
    if update_symbol(model, &decrement.expression, true, "--") != Some(index) {
        return None;
    }
    let Statement::IfStatement(exit) = exit else {
        return None;
    };
    if exit.alternate.is_some() {
        return None;
    }
    let Expression::UnaryExpression(not) = exit.test.without_parentheses() else {
        return None;
    };
    if not.operator.as_str() != "!" {
        return None;
    }
    let (bound, zero) = binary(&not.argument, ">=")?;
    if symbol(model, bound) != Some(index) || !number(zero, 0.0) {
        return None;
    }
    // The cached bodies write the exit as an unbraced
    // `if (!(count >= 0)) break;`, while the equivalent braced spelling has one
    // `break` statement in a block. Accept exactly those two forms and refuse a
    // labeled break or any other consequent.
    let stop = match &exit.consequent {
        Statement::BreakStatement(stop) => stop,
        _ => match block(&exit.consequent)? {
            [Statement::BreakStatement(stop)] => stop,
            _ => return None,
        },
    };
    if stop.label.is_some() {
        return None;
    }
    both_fields(model, first, second, writer, stream, header, input, index)?;
    let Statement::ExpressionStatement(renumber) = renumber else {
        return None;
    };
    let renumber = assignment(&renumber.expression)?;
    if assigned_symbol(model, renumber) != Some(index) {
        return None;
    }
    let c = call(&renumber.right)?;
    if !is_global(model, &c.callee, &["Number"]) {
        return None;
    }
    let args = arity(c, 1)?;
    (symbol(model, args[0]) == Some(index)).then_some(())
}

fn ice_field_emitter(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::IfStatement(guard), complete_statement] = body.statements.as_slice() else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let (guarded, null) = binary(&guard.test, "!=").or_else(|| binary(&guard.test, "!=="))?;
    if symbol(model, guarded) != Some(input)
        || !matches!(null.without_parentheses(), Expression::NullLiteral(_))
    {
        return None;
    }
    let [seed_statement, header_statement, stream_statement, writer_statement, index_statement, mix_statement, loop_statement, next_statement] =
        block(&guard.consequent)?
    else {
        return None;
    };

    let seed_variable = one_variable(seed_statement)?;
    let seed = plain_local(model, seed_variable)?;
    if !random_seed(model, seed_variable.init.as_ref()?) {
        return None;
    }

    let header_variable = one_variable(header_statement)?;
    let header = plain_local(model, header_variable)?;
    if model.binding_writes(header) != 0
        || !header_bytes(model, header_variable.init.as_ref()?, seed)
    {
        return None;
    }

    let stream = match stream_statement {
        Statement::VariableDeclaration(_) => {
            let variable = one_variable(stream_statement)?;
            let stream = plain_local(model, variable)?;
            reseeded(model, site, variable.init.as_ref()?, seed)?;
            if stream == seed || stream == header {
                return None;
            }
            stream
        }
        Statement::ExpressionStatement(expression) => {
            let assignment_ = assignment(&expression.expression)?;
            if assigned_symbol(model, assignment_) != Some(seed) {
                return None;
            }
            reseeded(model, site, &assignment_.right, seed)?;
            seed
        }
        _ => return None,
    };

    let writer_variable = one_variable(writer_statement)?;
    let writer = plain_local(model, writer_variable)?;
    if model.binding_writes(writer) != 0 || writer == seed || writer == header || writer == stream {
        return None;
    }
    // The writer factory is derived from the running stream state: the fresh
    // normalized local, or the seed itself when the reseed is in place.
    writer_derivation(model, site, writer_variable.init.as_ref()?, stream)?;

    let index_variable = one_variable(index_statement)?;
    let index = plain_local(model, index_variable)?;
    if index == seed || index == header || index == stream || index == writer {
        return None;
    }
    let length_init = index_variable.init.as_ref()?;
    let (forward, length) = if number(length_init, 0.0) {
        (true, None)
    } else {
        let receiver = member(length_init, "length")?;
        if symbol(model, receiver) != Some(input) {
            return None;
        }
        (false, Some(index))
    };

    let mix = assignment(expression_statement(mix_statement)?)?;
    if assigned_symbol(model, mix) != Some(stream) {
        return None;
    }
    let operand = length_mix(model, site, call(&mix.right)?, stream, header)?;
    match length {
        None => {
            if symbol(model, member(operand, "length")?) != Some(input) {
                return None;
            }
        }
        Some(length) => {
            // The mix may forward either the descending cursor local or the
            // same formal length it was initialized from; both name the exact
            // proven bound and neither is a generated spelling.
            let cursor = symbol(model, operand) == Some(length);
            let formal_length = member(operand, "length")
                .is_some_and(|receiver| symbol(model, receiver) == Some(input));
            if !cursor && !formal_length {
                return None;
            }
        }
    }

    if forward {
        forward_loop(model, loop_statement, writer, stream, header, input, index)?;
    } else {
        reverse_loop(model, loop_statement, writer, stream, header, input, index)?;
    }

    let next = statement_call(next_statement)?;
    let receiver = captured_symbol(model, site, method_receiver(model, next, "next")?)?;
    let args = arity(next, 1)?;
    let Expression::ArrayExpression(pair) = args[0].without_parentheses() else {
        return None;
    };
    let [first, second] = pair.elements.as_slice() else {
        return None;
    };
    if !empty_array(first.as_expression()?)
        || symbol(model, second.as_expression()?) != Some(header)
    {
        return None;
    }

    let complete = statement_call(complete_statement)?;
    if !complete.arguments.is_empty()
        || method_receiver(model, complete, "complete") != Some(receiver)
    {
        return None;
    }
    if [seed, header, stream, writer, index].contains(&receiver) {
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
    let Some(body) = &function.body else {
        return Vec::new();
    };
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let [parameter] = function.params.items.as_slice() else {
        return Vec::new();
    };
    if function.params.rest.is_some() || parameter.initializer.is_some() {
        return Vec::new();
    }
    let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
        return Vec::new();
    };
    let Some(input) = identifier.symbol_id.get() else {
        return Vec::new();
    };
    if model.binding_writes(input) != 0 || !model.scoping.symbol_redeclarations(input).is_empty() {
        return Vec::new();
    }
    if ice_field_emitter(model, site, input, body).is_none() {
        return Vec::new();
    }
    vec![proposal(
        NAME,
        EVALUATOR,
        "WebRTC-candidate variant of the seeded field emitter: one null-guarded \
         formal, one Math.random()*2^32 seed packed most-significant-byte first \
         into a four-byte header, one seed^number reseed and one derived writer, \
         then the candidate-list length and every candidate's ip/candidate \
         strings mixed through that writer with the same header and a running \
         cursor, and finally next([[], header]) plus complete() on one captured \
         observer",
        f,
    )]
}
