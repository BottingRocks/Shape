//! Closed, capture-free collection protocols. No native identity is inferred
//! for formal receivers: the names describe the exact owned operations.
use crate::naming::shared::{one_variable, plain_local};
use crate::naming::support::reassembly::{call, immutable, member_parts, symbol};
use crate::naming::*;

fn params(model: &Model<'_, '_>, p: &FormalParameters<'_>) -> Option<Vec<SymbolId>> {
    if p.rest.is_some() {
        return None;
    }
    p.items
        .iter()
        .map(|p| {
            if p.initializer.is_some() {
                return None;
            }
            immutable(model, &p.pattern)
        })
        .collect()
}

fn returned<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(r) = s else {
        return None;
    };
    r.argument.as_ref()
}

fn local(model: &Model<'_, '_>, s: &Statement<'_>) -> Option<SymbolId> {
    let v = one_variable(s)?;
    let id = plain_local(model, v)?;
    (model.binding_writes(id) == 0).then_some(id)
}

fn index(model: &Model<'_, '_>, e: &Expression<'_>, object: SymbolId, key: SymbolId) -> bool {
    matches!(e, Expression::ComputedMemberExpression(m) if !m.optional
        && symbol(model, &m.object) == Some(object) && symbol(model, &m.expression) == Some(key))
}

fn indexed_map(model: &Model<'_, '_>, body: &FunctionBody<'_>, p: &[SymbolId]) -> Option<bool> {
    let [names, values] = p else {
        return None;
    };
    let statements = body.statements.as_slice();
    let (alias, create, store, iterate, result) = match statements {
        [alias, create, iterate, result] => (alias, create, None, iterate, result),
        [alias, create, store, iterate, result] => (alias, create, Some(store), iterate, result),
        _ => return None,
    };
    let values_alias = local(model, alias)?;
    if symbol(model, one_variable(alias)?.init.as_ref()?) != Some(*values) {
        return None;
    }
    let output_variable = one_variable(create)?;
    let output = plain_local(model, output_variable)?;
    let initializer = match store {
        None if model.binding_writes(output) == 0 => output_variable.init.as_ref()?,
        Some(Statement::ExpressionStatement(store))
            if output_variable.init.is_none() && model.binding_writes(output) == 1 =>
        {
            let Expression::AssignmentExpression(store) = &store.expression else {
                return None;
            };
            if store.operator.as_str() != "="
                || !matches!(&store.left, AssignmentTarget::AssignmentTargetIdentifier(i)
                if crate::analysis::identifier_symbol(i, model.scoping) == Some(output))
            {
                return None;
            }
            &store.right
        }
        _ => return None,
    };
    let constructor = call(initializer)?;
    let (object, method) = member_parts(&constructor.callee)?;
    // Global Object is a spelling plus unresolved-reference check, never a
    // same-spelled parameter/local. Only its .create(null) call is claimed.
    if method != "create"
        || !matches!(object, Expression::Identifier(i)
        if i.name == "Object" && crate::analysis::identifier_symbol(i, model.scoping).is_none())
        || !matches!(constructor.arguments.as_slice(), [Argument::NullLiteral(_)])
    {
        return None;
    }
    let Statement::ExpressionStatement(iterate) = iterate else {
        return None;
    };
    let iterate = call(&iterate.expression)?;
    let (receiver, method) = member_parts(&iterate.callee)?;
    if method != "forEach" || symbol(model, receiver) != Some(*names) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = iterate.arguments.as_slice() else {
        return None;
    };
    if callback.r#async || callback.expression || !callback.body.directives.is_empty() {
        return None;
    }
    let callback_params = params(model, &callback.params)?;
    let [name, position] = callback_params.as_slice() else {
        return None;
    };
    let [Statement::ExpressionStatement(write)] = callback.body.statements.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(write) = &write.expression else {
        return None;
    };
    let AssignmentTarget::ComputedMemberExpression(target) = &write.left else {
        return None;
    };
    if write.operator.as_str() != "="
        || target.optional
        || symbol(model, &target.object) != Some(output)
        || symbol(model, &target.expression) != Some(*name)
        || !index(model, &write.right, values_alias, *position)
        || symbol(model, returned(result)?) != Some(output)
    {
        return None;
    }
    Some(store.is_some())
}

// Small expression grammar for the owned batch loop. Every variable is supplied
// by its exact formal/declarator SymbolId. Property names and operators are
// protocol syntax; unresolved globals are checked separately from locals.
enum Term {
    Var(SymbolId),
    Number(f64),
    This,
    Global(&'static str),
    Member(Box<Term>, &'static str),
    Call(Box<Term>, Vec<Term>),
    Binary(Box<Term>, &'static str, Box<Term>),
    Not(Box<Term>),
    Conditional(Box<Term>, Box<Term>, Box<Term>),
}
fn v(s: SymbolId) -> Term {
    Term::Var(s)
}
fn n(x: f64) -> Term {
    Term::Number(x)
}
fn m(o: Term, k: &'static str) -> Term {
    Term::Member(Box::new(o), k)
}
fn c(f: Term, args: Vec<Term>) -> Term {
    Term::Call(Box::new(f), args)
}
fn b(l: Term, op: &'static str, r: Term) -> Term {
    Term::Binary(Box::new(l), op, Box::new(r))
}
fn this_call(k: &'static str) -> Term {
    c(m(Term::This, k), vec![])
}
fn term(model: &Model<'_, '_>, e: &Expression<'_>, p: &Term) -> bool {
    let e = e.without_parentheses();
    match p {
        Term::Var(s) => symbol(model, e) == Some(*s),
        Term::Number(x) => matches!(e, Expression::NumericLiteral(l) if l.value==*x),
        Term::This => matches!(e, Expression::ThisExpression(_)),
        Term::Global(name) => {
            matches!(e, Expression::Identifier(i) if i.name==*name && crate::analysis::identifier_symbol(i,model.scoping).is_none())
        }
        Term::Member(o, k) => {
            member_parts(e).is_some_and(|(x, key)| key == *k && term(model, x, o))
        }
        Term::Call(f, args) => call(e).is_some_and(|call| {
            call.arguments.len() == args.len()
                && term(model, &call.callee, f)
                && call
                    .arguments
                    .iter()
                    .zip(args)
                    .all(|(a, p)| a.as_expression().is_some_and(|a| term(model, a, p)))
        }),
        Term::Binary(l, op, r) => {
            matches!(e, Expression::BinaryExpression(x) if x.operator.as_str()==*op && term(model,&x.left,l) && term(model,&x.right,r))
        }
        Term::Not(x) => {
            matches!(e, Expression::UnaryExpression(u) if u.operator.as_str()=="!" && term(model,&u.argument,x))
        }
        Term::Conditional(t, a, b) => {
            matches!(e, Expression::ConditionalExpression(x) if term(model,&x.test,t) && term(model,&x.consequent,a) && term(model,&x.alternate,b))
        }
    }
}
fn expr<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}
fn assign(model: &Model<'_, '_>, e: &Expression<'_>, id: SymbolId, value: Term) -> bool {
    matches!(e, Expression::AssignmentExpression(a) if a.operator.as_str()=="="
        && matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i,model.scoping)==Some(id))
        && term(model,&a.right,&value))
}
fn decl(model: &Model<'_, '_>, s: &Statement<'_>, value: Term) -> Option<SymbolId> {
    let d = one_variable(s)?;
    let id = plain_local(model, d)?;
    term(model, d.init.as_ref()?, &value).then_some(id)
}
fn schedule(task: SymbolId) -> Term {
    c(
        Term::Global("setTimeout"),
        vec![
            c(
                m(m(Term::This, "_process"), "bind"),
                vec![Term::This, v(task)],
            ),
            n(0.0),
        ],
    )
}
fn process_batch(model: &Model<'_, '_>, body: &FunctionBody<'_>, task: SymbolId) -> Option<()> {
    let [budget, start, average, index, slots, Statement::ForStatement(loop_), Statement::IfStatement(rearm)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let budget = decl(model, budget, m(Term::This, "targetBatchTimeMs"))?;
    let start = decl(model, start, this_call("now"))?;
    let average = decl(model, average, n(0.0))?;
    let index = decl(model, index, n(0.0))?;
    let Statement::VariableDeclaration(slots) = slots else {
        return None;
    };
    let [result, next] = slots.declarations.as_slice() else {
        return None;
    };
    if result.init.is_some() || next.init.is_some() {
        return None;
    }
    let result = plain_local(model, result)?;
    let next = plain_local(model, next)?;
    for (s, writes) in [
        (budget, 0),
        (start, 0),
        (average, 1),
        (index, 1),
        (result, 1),
        (next, 1),
    ] {
        if model.binding_writes(s) != writes {
            return None;
        }
    }
    if loop_.init.is_some()
        || loop_.test.is_some()
        || !assign(model, loop_.update.as_ref()?, index, v(next))
    {
        return None;
    }
    let Statement::BlockStatement(loop_body) = &loop_.body else {
        return None;
    };
    let [begin, work, end, avg, advance, Statement::IfStatement(stop)] = loop_body.body.as_slice()
    else {
        return None;
    };
    let begin = decl(model, begin, this_call("now"))?;
    let end = decl(model, end, this_call("now"))?;
    if model.binding_writes(begin) != 0
        || model.binding_writes(end) != 0
        || !assign(model, expr(work)?, result, c(v(task), vec![]))
        || !assign(
            model,
            expr(avg)?,
            average,
            b(
                b(b(v(average), "*", v(index)), "+", b(v(end), "-", v(begin))),
                "/",
                b(v(index), "+", n(1.0)),
            ),
        )
        || !assign(
            model,
            expr(advance)?,
            next,
            b(c(Term::Global("Number"), vec![v(index)]), "+", n(1.0)),
        )
        || stop.alternate.is_some()
    {
        return None;
    }
    let within = || b(b(v(end), "+", v(average)), "<", b(v(start), "+", v(budget)));
    let stop_test = Term::Not(Box::new(Term::Conditional(
        Box::new(within()),
        Box::new(v(result)),
        Box::new(within()),
    )));
    if !term(model, &stop.test, &stop_test)
        || !matches!(&stop.consequent,Statement::BreakStatement(b) if b.label.is_none())
        || rearm.alternate.is_some()
        || symbol(model, &rearm.test) != Some(result)
    {
        return None;
    }
    let Statement::BlockStatement(rearm) = &rearm.consequent else {
        return None;
    };
    let [rearm] = rearm.body.as_slice() else {
        return None;
    };
    term(model, expr(rearm)?, &schedule(task)).then_some(())
}

fn batch_method(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    p: &[SymbolId],
) -> Option<(&'static str, &'static str)> {
    // The recovered methods execute in strict mode. No extra directives are
    // accepted, and every statement (including the loop's break target) is owned.
    let [directive] = body.directives.as_slice() else {
        return None;
    };
    if directive.directive != "use strict" {
        return None;
    }
    if p.is_empty() {
        let [s] = body.statements.as_slice() else {
            return None;
        };
        return term(model, returned(s)?, &this_call("_internalNow"))
            .then_some(("readInternalNow", "batch_scheduler_owned_now"));
    }
    let [task] = p else {
        return None;
    };
    if let [s] = body.statements.as_slice() {
        if term(model, expr(s)?, &schedule(*task)) {
            return Some(("scheduleProcessWork", "batch_scheduler_owned_schedule"));
        }
    }
    process_batch(model, body, *task).map(|_| ("processWorkBatch", "batch_scheduler_owned_process"))
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if function.r#async || function.generator {
        return vec![];
    }
    let Some(p) = params(model, &function.params) else {
        return vec![];
    };
    // Generated grammar is eligibility only. A stable, live ordinary empty
    // declaration has a proved own behavior independent of caller purpose.
    if p.is_empty()
        && body.statements.is_empty()
        && body.directives.is_empty()
        && function.r#type == FunctionType::FunctionDeclaration
    {
        if let Some(binding) = site.binding_symbol {
            if thread_id(model.scoping.symbol_name(binding)).is_some()
                && model.binding_writes(binding) == 0
                && model.binding_references(binding) > 0
                && model.scoping.symbol_redeclarations(binding).is_empty()
            {
                let mut p = proposal("noOp", "referenced_ordinary_noop",
                    "stable referenced ordinary zero-arity declaration; exactly empty own body and no directives, async/generator, defaults, rest or destructuring; no own reads, writes, calls, throws or parameter effects; implicit undefined on ordinary call. Caller, constructor and native purpose are explicitly unproved; live declaration and reference identity are retained", facts);
                p.confidence = Confidence::High;
                p.evidence = vec![site.span.into()];
                return vec![p];
            }
        }
    }
    let matched = body.directives.is_empty().then(|| indexed_map(model, body, &p)).flatten().map(|split_store| ("mapNamesToValues", "indexed_null_prototype_map",
        if split_store { "entire capture-free body: immutable values alias, uninitialized output declaration immediately followed by sole direct global Object.create(null) store, formal names.forEach with exact output[name]=values[index] callback, then return output; external formal method implementation is unproved" } else { "entire capture-free body: immutable values alias, global Object.create(null), formal names.forEach with exact output[name]=values[index] callback, then return output; external formal method implementation is unproved" }))
        .or_else(||batch_method(model,body,&p).map(|(name,evaluator)|(name,evaluator,
        "entire capture-free strict method: exact dynamic this receiver and formal identity; internalNow call, zero-delay bound process scheduling, or exact moving-average budget loop with clock/work order, Number increment, unlabeled break, and work-dependent rearm; external clock and method implementations explicitly unproved")));
    matched
        .map(|(name, evaluator, proof)| vec![proposal(name, evaluator, proof, facts)])
        .unwrap_or_default()
}
