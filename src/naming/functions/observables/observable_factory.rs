//! Fast-loop shared rule: an observable class factory.
//!
//! The owned witness is a zero-formal function whose whole body is exactly one
//! class declaration, one `let alias = ClassName;` binding and
//! `return alias;`. The class is declared and owned by that factory and the
//! returned identity is that same class through one immutable, single-origin
//! alias.
//!
//! The complete class API is proved from its own bodies, never from method
//! spellings alone:
//!
//! * `constructor`: a non-function formal is rejected with `new TypeError()`,
//!   otherwise it is stored as `this._subscriber` and `void 0` is returned;
//! * `static of`: copies its `arguments` into a fresh array and builds a new
//!   instance whose executor emits every element with `next` and then
//!   `complete`;
//! * `static empty`: builds a new instance whose executor only completes;
//! * `static from`: the ordered instance / Promise / array-like / iterator
//!   protocol chain, including the caller-supplied scheduler's scheduled and
//!   unscheduled loops, the done/value step protocol and the failure throw;
//!   the emission-in-test do-while alternative additionally proves its local
//!   receiver/method/value captures, numeric index update, sentinel writes and
//!   the ordered error-forwarding/sentinel-failure catch;
//! * `subscribe`: normalizes the observer form, rejects an absent subscriber,
//!   constructs the observer and subscription handles, calls `start`, stores
//!   the captured teardown result in `_cleanup`, forwards a teardown error to
//!   the observer, runs a cleanup that was not consumed by an observer, and
//!   returns the subscription;
//! * `pipe`: copies its `arguments` and reduces the operators over `this`;
//! * `map` / `unorderedFlatMap`: forward through `pipe` with a captured,
//!   independently distinct operator function.
//!
//! Class members may be written in any order because none of them has a
//! side-effect relationship with its siblings. Statement order inside every
//! member is meaningful and is proved. Captured constructors, the teardown and
//! the two operators are consumer input: this rule requires only stable,
//! immutable scoped identities and never claims a native/static identity for
//! them. Generated spellings, source offsets, vendor data, body hashes and
//! inherited labels never participate.

use super::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

type Symbol = SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn identifier(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    symbol(model, e.without_parentheses())
}

fn immutable(model: &Model<'_, '_>, s: Symbol) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}

fn captured(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> bool {
    !site.span.contains_inclusive(model.scoping.symbol_span(s))
        && model.scoping.symbol_redeclarations(s).is_empty()
}

fn plain_binding(model: &Model<'_, '_>, p: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(s)
        .is_empty()
        .then_some(s)
}

/// Plain formals only: no rest element, no default initializer, no pattern and
/// no reassignment. Callback formals are owned identities and are immutable.
fn plain_params<'a>(model: &Model<'_, '_>, function: &'a Function<'a>) -> Option<Vec<Symbol>> {
    if function.params.rest.is_some() {
        return None;
    }
    let mut params = Vec::new();
    for parameter in &function.params.items {
        if parameter.initializer.is_some() || !parameter.decorators.is_empty() {
            return None;
        }
        let s = plain_binding(model, &parameter.pattern)?;
        if model.binding_writes(s) != 0 {
            return None;
        }
        params.push(s);
    }
    Some(params)
}

fn plain_arrow_params(
    model: &Model<'_, '_>,
    arrow: &ArrowFunctionExpression<'_>,
) -> Option<Vec<Symbol>> {
    if arrow.params.rest.is_some() {
        return None;
    }
    let mut params = Vec::new();
    for parameter in &arrow.params.items {
        if parameter.initializer.is_some() || !parameter.decorators.is_empty() {
            return None;
        }
        let s = plain_binding(model, &parameter.pattern)?;
        if model.binding_writes(s) != 0 {
            return None;
        }
        params.push(s);
    }
    Some(params)
}

/// A normal, non-async, non-generator function body with no directives.
fn body_of<'a>(function: &'a Function<'a>) -> Option<&'a FunctionBody<'a>> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if body.directives.iter().any(|d| d.directive != "use strict") {
        return None;
    }
    Some(body)
}

fn expression_statement<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ExpressionStatement(s) => Some(&s.expression),
        _ => None,
    }
}

fn return_value<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ReturnStatement(s) => s.argument.as_ref(),
        _ => None,
    }
}

/// The value an arrow returns: an expression body's sole expression or a
/// block body's sole `return`.
fn arrow_value<'a>(arrow: &'a ArrowFunctionExpression<'a>) -> Option<&'a Expression<'a>> {
    let [statement] = arrow.body.statements.as_slice() else {
        return None;
    };
    if arrow.expression {
        expression_statement(statement)
    } else {
        return_value(statement)
    }
}

fn one_variable<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    if !matches!(
        d.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    Some(v)
}

fn local(model: &Model<'_, '_>, v: &VariableDeclarator<'_>) -> Option<Symbol> {
    plain_binding(model, &v.id)
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

fn plain_call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn new_expression<'a>(e: &'a Expression<'a>) -> Option<&'a NewExpression<'a>> {
    match e.without_parentheses() {
        Expression::NewExpression(n) => Some(n),
        _ => None,
    }
}

fn new_arguments<'a>(call: &'a NewExpression<'a>) -> Vec<&'a Expression<'a>> {
    call.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

fn exact_arguments<'a>(
    call: &'a CallExpression<'a>,
    count: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let arguments = call_arguments(call);
    (arguments.len() == count && call.arguments.len() == count).then_some(arguments)
}

fn exact_new_arguments<'a>(
    call: &'a NewExpression<'a>,
    count: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let arguments = new_arguments(call);
    (arguments.len() == count && call.arguments.len() == count).then_some(arguments)
}

fn receiver_of<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn method_call<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a CallExpression<'a>> {
    let call = plain_call(e)?;
    receiver_of(&call.callee, key)?;
    Some(call)
}

fn binary<'a>(
    e: &'a Expression<'a>,
    operator: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == operator).then_some((&b.left, &b.right))
}

fn logical<'a>(
    e: &'a Expression<'a>,
    operator: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(l) = e.without_parentheses() else {
        return None;
    };
    (l.operator.as_str() == operator).then_some((&l.left, &l.right))
}

fn conditional<'a>(
    e: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ConditionalExpression(c) = e.without_parentheses() else {
        return None;
    };
    Some((&c.test, &c.consequent, &c.alternate))
}

fn is_number(e: &Expression<'_>, value: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(n) if n.value == value)
}

fn is_string(e: &Expression<'_>, value: &str) -> bool {
    matches!(e.without_parentheses(), Expression::StringLiteral(s) if s.value == value)
}

fn is_null(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::NullLiteral(_))
}

fn is_void_zero(e: &Expression<'_>) -> bool {
    match e.without_parentheses() {
        Expression::UnaryExpression(u) => {
            u.operator.as_str() == "void" && is_number(&u.argument, 0.0)
        }
        _ => false,
    }
}

fn is_arguments(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::Identifier(i) if i.name == "arguments")
}

fn unbound(model: &Model<'_, '_>, e: &Expression<'_>, name: &str) -> bool {
    match e.without_parentheses() {
        Expression::Identifier(i) => {
            i.name == name && crate::analysis::identifier_symbol(i, model.scoping).is_none()
        }
        _ => false,
    }
}

fn global_member(model: &Model<'_, '_>, e: &Expression<'_>, root: &str, key: &str) -> bool {
    match e.without_parentheses() {
        Expression::StaticMemberExpression(m) => {
            !m.optional && m.property.name == key && unbound(model, &m.object, root)
        }
        _ => false,
    }
}

fn throw_expression<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ThrowStatement(t) = s else {
        return None;
    };
    Some(&t.argument)
}

fn typeof_test<'a>(
    e: &'a Expression<'a>,
    operator: &str,
    value: &str,
) -> Option<&'a Expression<'a>> {
    let (left, right) = binary(e, operator)?;
    let Expression::UnaryExpression(u) = left.without_parentheses() else {
        return None;
    };
    (u.operator.as_str() == "typeof" && is_string(right, value)).then_some(&u.argument)
}

fn member_of(model: &Model<'_, '_>, e: &Expression<'_>, object: Symbol, key: &str) -> bool {
    receiver_of(e, key).is_some_and(|receiver| identifier(model, receiver) == Some(object))
}

fn this_member(e: &Expression<'_>, key: &str) -> bool {
    receiver_of(e, key)
        .is_some_and(|object| matches!(object.without_parentheses(), Expression::ThisExpression(_)))
}

/// A stable callable identity captured by the factory. The declaration may be a
/// function, a class, a function/arrow/class expression in a variable, or a
/// plain immutable alias chain ending in one of those. No native identity is
/// claimed for it.
fn captured_callable(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<Symbol> {
    let s = identifier(model, e)?;
    if !captured(model, site, s) || !immutable(model, s) {
        return None;
    }
    callable_declaration(model, s, 8).then_some(s)
}

fn callable_declaration(model: &Model<'_, '_>, s: Symbol, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    match model.nodes.kind(model.scoping.symbol_declaration(s)) {
        AstKind::Function(_) | AstKind::Class(_) => true,
        AstKind::VariableDeclarator(v) => {
            let Some(init) = v.init.as_ref() else {
                return false;
            };
            match init.without_parentheses() {
                Expression::FunctionExpression(_)
                | Expression::ArrowFunctionExpression(_)
                | Expression::ClassExpression(_) => true,
                Expression::Identifier(i) => {
                    let Some(next) = crate::analysis::identifier_symbol(i, model.scoping) else {
                        return false;
                    };
                    next != s && callable_declaration(model, next, depth - 1)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn class_identity(model: &Model<'_, '_>, e: &Expression<'_>, class: Symbol, alias: Symbol) -> bool {
    identifier(model, e).is_some_and(|s| s == class || s == alias)
}

fn distinct(values: &[Symbol]) -> bool {
    let mut set = BTreeSet::new();
    values.iter().all(|value| set.insert(*value))
}

/// `let N = arguments.length; let A = new Array(N); let i = 0;
///  for (; i < N; i++) { A[i] = arguments[i]; }`
/// or, when the array local is hoisted first:
/// `let A; let N = arguments.length; A = new Array(N); let i = 0;
///  for (; i < N; i++) { A[i] = arguments[i]; }`
///
/// The index may instead be declared inline in the loop head, the same owned
/// copy loop with no separate statement:
/// `let N = arguments.length; let A = new Array(N);
///  for (let i = 0; i < N; i++) { A[i] = arguments[i]; }`
///
/// Returns `(length, array, index, statements_consumed)`.
pub(in crate::naming) fn copy_arguments<'a>(
    model: &Model<'_, '_>,
    records: &'a [Statement<'a>],
) -> Option<(Symbol, Symbol, Symbol, usize)> {
    let (hoisted, offset) = match records.first().and_then(|first| one_variable(first)) {
        Some(variable) if variable.init.is_none() => (Some(local(model, variable)?), 1),
        _ => (None, 0),
    };
    let length_variable = one_variable(records.get(offset)?)?;
    let length = local(model, length_variable)?;
    if !immutable(model, length)
        || !receiver_of(length_variable.init.as_ref()?, "length")
            .is_some_and(|object| is_arguments(object))
    {
        return None;
    }
    let array = match hoisted {
        Some(array) => {
            let assigned = expression_statement(records.get(offset + 1)?)?;
            let Expression::AssignmentExpression(assigned) = assigned.without_parentheses() else {
                return None;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assigned.left else {
                return None;
            };
            if assigned.operator.as_str() != "="
                || crate::analysis::identifier_symbol(target, model.scoping) != Some(array)
            {
                return None;
            }
            new_array_size(model, &assigned.right, length)?;
            array
        }
        None => {
            let array_variable = one_variable(records.get(offset + 1)?)?;
            let array = local(model, array_variable)?;
            new_array_size(model, array_variable.init.as_ref()?, length)?;
            array
        }
    };
    // The index is either a separate `let i = 0;` statement before the loop or
    // the loop's own inline head declaration. Both are the bounded, owned,
    // init-zero, single-binding copy index; neither reorders or drops a
    // statement, and the consumed count differs by that one declaration.
    let (index, loop_, consumed) = match records.get(offset + 2)? {
        Statement::ForStatement(loop_) => (
            inline_index(model, loop_.init.as_ref()?)?,
            loop_,
            offset + 3,
        ),
        record => {
            let index_variable = one_variable(record)?;
            let index = local(model, index_variable)?;
            if !is_number(index_variable.init.as_ref()?, 0.0) {
                return None;
            }
            let Statement::ForStatement(loop_) = records.get(offset + 3)? else {
                return None;
            };
            if loop_.init.is_some() {
                return None;
            }
            (index, loop_, offset + 4)
        }
    };
    let (left, right) = binary(loop_.test.as_ref()?, "<")?;
    if identifier(model, left) != Some(index) || identifier(model, right) != Some(length) {
        return None;
    }
    let Expression::UpdateExpression(update) = loop_.update.as_ref()?.without_parentheses() else {
        return None;
    };
    if update.operator.as_str() != "++"
        || !matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id)
            if crate::analysis::identifier_symbol(id, model.scoping) == Some(index))
    {
        return None;
    }
    let [write] = block(&loop_.body)? else {
        return None;
    };
    let write = expression_statement(write)?;
    let Expression::AssignmentExpression(write) = write.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::ComputedMemberExpression(target) = &write.left else {
        return None;
    };
    if write.operator.as_str() != "="
        || target.optional
        || identifier(model, &target.object) != Some(array)
        || identifier(model, &target.expression) != Some(index)
    {
        return None;
    }
    let Expression::ComputedMemberExpression(source_value) = write.right.without_parentheses()
    else {
        return None;
    };
    if source_value.optional
        || !is_arguments(&source_value.object)
        || identifier(model, &source_value.expression) != Some(index)
    {
        return None;
    }
    Some((length, array, index, consumed))
}

/// The inline copy-loop head `for (let i = 0; ...)`: exactly one plain
/// `var`/`let`/`const` declarator bound to a single-owned local and initialised
/// with the literal zero. A multi-declarator head, a destructuring binding or a
/// non-zero initializer is refused; the alternative adds only this declaration
/// and never reorders or drops any executed statement.
fn inline_index(model: &Model<'_, '_>, init: &ForStatementInit<'_>) -> Option<Symbol> {
    let ForStatementInit::VariableDeclaration(declaration) = init else {
        return None;
    };
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    if !matches!(
        declaration.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let index = local(model, declarator)?;
    is_number(declarator.init.as_ref()?, 0.0).then_some(index)
}

/// `new Array(length)` with the unshadowed global `Array` and exactly one
/// argument that is the argument-count local.
fn new_array_size(model: &Model<'_, '_>, e: &Expression<'_>, length: Symbol) -> Option<()> {
    let new = new_expression(e)?;
    if !unbound(model, &new.callee, "Array") {
        return None;
    }
    let args = exact_new_arguments(new, 1)?;
    (identifier(model, args[0]) == Some(length)).then_some(())
}

fn new_instance<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    class: Symbol,
    alias: Symbol,
) -> Option<(&'a NewExpression<'a>, &'a ArrowFunctionExpression<'a>)> {
    let new = new_expression(e)?;
    if !class_identity(model, &new.callee, class, alias) {
        return None;
    }
    let args = exact_new_arguments(new, 1)?;
    let [executor] = args.as_slice() else {
        return None;
    };
    let Expression::ArrowFunctionExpression(executor) = executor.without_parentheses() else {
        return None;
    };
    Some((new, executor))
}

fn observer_call<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    handle: Symbol,
    member: &str,
    arguments: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let call = method_call(e, member)?;
    let args = exact_arguments(call, arguments)?;
    receiver_of(&call.callee, member)
        .is_some_and(|receiver| identifier(model, receiver) == Some(handle))
        .then_some(args)
}

/// The two-element tuple of a bundled method-call lowering: either the
/// declaration `let t = [a, b];` or the reused-target assignment `t = [a, b];`.
fn bundle_pair<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
) -> Option<(Symbol, [&'a Expression<'a>; 2])> {
    let (tuple, array) = if let Some(variable) = one_variable(statement) {
        (local(model, variable)?, variable.init.as_ref()?)
    } else {
        let expression = expression_statement(statement)?;
        let Expression::AssignmentExpression(assigned) = expression.without_parentheses() else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assigned.left else {
            return None;
        };
        if assigned.operator.as_str() != "=" {
            return None;
        }
        (
            crate::analysis::identifier_symbol(target, model.scoping)?,
            &assigned.right,
        )
    };
    let Expression::ArrayExpression(literal) = array.without_parentheses() else {
        return None;
    };
    let [first, second] = literal.elements.as_slice() else {
        return None;
    };
    Some((tuple, [first.as_expression()?, second.as_expression()?]))
}

/// The trailing arguments of a `t[1].call(t[0], ...)` bundle call.
fn bundle_arguments<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    tuple: Symbol,
) -> Option<Vec<&'a Expression<'a>>> {
    let call = plain_call(expression_statement(statement)?)?;
    let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else {
        return None;
    };
    if member.optional || member.property.name != "call" {
        return None;
    }
    let Expression::ComputedMemberExpression(indexed) = member.object.without_parentheses() else {
        return None;
    };
    if indexed.optional
        || identifier(model, &indexed.object) != Some(tuple)
        || !is_number(&indexed.expression, 1.0)
    {
        return None;
    }
    let [this_argument, rest @ ..] = call.arguments.as_slice() else {
        return None;
    };
    let Expression::ComputedMemberExpression(element) =
        this_argument.as_expression()?.without_parentheses()
    else {
        return None;
    };
    if element.optional
        || identifier(model, &element.object) != Some(tuple)
        || !is_number(&element.expression, 0.0)
    {
        return None;
    }
    rest.iter()
        .map(|argument| argument.as_expression())
        .collect()
}

/// The tracer's bundled method call, which lowers `recv.method(args...)` as
/// either `let t = [recv, recv.method]; t[1].call(t[0], args...)` or the nested
/// `let t = [recv.method, recv.method.call]; t[1].call(t[0], recv, args...)`.
/// Returns `(recv, method, args)`; receiver and argument identities are proved.
fn bundled_call<'a>(
    model: &Model<'_, '_>,
    bundle: &'a Statement<'a>,
    call: &'a Statement<'a>,
) -> Option<(Symbol, String, Vec<&'a Expression<'a>>)> {
    let (tuple, [first, second]) = bundle_pair(model, bundle)?;
    if let Some(receiver) = identifier(model, first) {
        let Expression::StaticMemberExpression(member) = second.without_parentheses() else {
            return None;
        };
        if member.optional || identifier(model, &member.object) != Some(receiver) {
            return None;
        }
        let method = member.property.name.to_string();
        return Some((receiver, method, bundle_arguments(model, call, tuple)?));
    }
    let Expression::StaticMemberExpression(inner) = first.without_parentheses() else {
        return None;
    };
    if inner.optional {
        return None;
    }
    let receiver = identifier(model, &inner.object)?;
    let method = inner.property.name.to_string();
    let Expression::StaticMemberExpression(outer) = second.without_parentheses() else {
        return None;
    };
    let Expression::StaticMemberExpression(same) = outer.object.without_parentheses() else {
        return None;
    };
    if outer.optional
        || outer.property.name != "call"
        || same.optional
        || same.property.name.as_str() != inner.property.name.as_str()
        || identifier(model, &same.object) != Some(receiver)
    {
        return None;
    }
    let mut arguments = bundle_arguments(model, call, tuple)?;
    if arguments
        .first()
        .copied()
        .and_then(|argument| identifier(model, argument))
        != Some(receiver)
    {
        return None;
    }
    arguments.remove(0);
    Some((receiver, method, arguments))
}

/// The pre-schedule index initializer: either the declaration `let i = 0;` or
/// the write `i = 0;` into a pre-declared local.
fn index_zero(model: &Model<'_, '_>, statement: &Statement<'_>) -> Option<Symbol> {
    if let Some(variable) = one_variable(statement) {
        let index = local(model, variable)?;
        return is_number(variable.init.as_ref()?, 0.0).then_some(index);
    }
    let expression = expression_statement(statement)?;
    let Expression::AssignmentExpression(assigned) = expression.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assigned.left else {
        return None;
    };
    if assigned.operator.as_str() != "=" || !is_number(&assigned.right, 0.0) {
        return None;
    }
    crate::analysis::identifier_symbol(target, model.scoping)
}

/// The executor's `[alias, maybe hoisted local, branch]` head. The alias is an
/// immutable rebuild of the subscriber slot; the optional bare local is a
/// pre-declared index or iterator that the branch assigns.
fn executor_branch<'a>(
    model: &Model<'_, '_>,
    statements: &'a [Statement<'a>],
    subscriber: Symbol,
) -> Option<(Symbol, &'a Statement<'a>)> {
    let (alias_statement, branch) = match statements {
        [alias, branch] => (alias, branch),
        [alias, declared, branch] => {
            let variable = one_variable(declared)?;
            if variable.init.is_some() {
                return None;
            }
            local(model, variable)?;
            (alias, branch)
        }
        _ => return None,
    };
    let alias_variable = one_variable(alias_statement)?;
    let handle = local(model, alias_variable)?;
    if identifier(model, alias_variable.init.as_ref()?) != Some(subscriber)
        || !immutable(model, handle)
    {
        return None;
    }
    Some((handle, branch))
}

/// `let S = source[Symbol.iterator]()` or the pre-declared
/// `S = source[Symbol.iterator]()`.
fn iterator_step<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    source: Symbol,
) -> Option<Symbol> {
    if let Some(variable) = one_variable(statement) {
        let step = local(model, variable)?;
        check_iterator_call(model, variable.init.as_ref()?, source)?;
        return Some(step);
    }
    let expression = expression_statement(statement)?;
    let Expression::AssignmentExpression(assigned) = expression.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assigned.left else {
        return None;
    };
    if assigned.operator.as_str() != "=" {
        return None;
    }
    let step = crate::analysis::identifier_symbol(target, model.scoping)?;
    check_iterator_call(model, &assigned.right, source)?;
    Some(step)
}

fn check_iterator_call(model: &Model<'_, '_>, e: &Expression<'_>, source: Symbol) -> Option<()> {
    let call = plain_call(e)?;
    if !exact_arguments(call, 0)?.is_empty() {
        return None;
    }
    iterator_expression(model, &call.callee, source)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Receiver {
    Symbol(Symbol),
    This,
    Direct(Symbol),
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CallFact {
    receiver: Receiver,
    method: String,
    arity: Option<usize>,
}

/// A bounded structural scan of one owned callback body. It records only
/// scoped-receiver method calls, construction sites, static member reads and
/// `object[Symbol.iterator]` indexing. It is used inside the `from` executors,
/// where the exact branch order is checked separately.
struct Scan<'m, 'a> {
    model: &'m Model<'m, 'a>,
    calls: BTreeSet<CallFact>,
    reads: BTreeSet<(Receiver, String)>,
    news: BTreeSet<(Option<Symbol>, String, Option<usize>)>,
    symbol_iterators: BTreeSet<Symbol>,
    computed_reads: BTreeSet<Symbol>,
}

impl<'m, 'a> Scan<'m, 'a> {
    fn new(model: &'m Model<'m, 'a>) -> Self {
        Self {
            model,
            calls: BTreeSet::new(),
            reads: BTreeSet::new(),
            news: BTreeSet::new(),
            symbol_iterators: BTreeSet::new(),
            computed_reads: BTreeSet::new(),
        }
    }

    fn receiver(&self, e: &Expression<'a>) -> Receiver {
        match e.without_parentheses() {
            Expression::Identifier(i) => crate::analysis::identifier_symbol(i, self.model.scoping)
                .map_or(Receiver::Other, Receiver::Symbol),
            Expression::ThisExpression(_) => Receiver::This,
            _ => Receiver::Other,
        }
    }

    fn calls(&self, receiver: Receiver, method: &str, arity: usize) -> bool {
        self.calls.contains(&CallFact {
            receiver,
            method: method.to_string(),
            arity: Some(arity),
        })
    }

    fn reads(&self, receiver: Receiver, key: &str) -> bool {
        self.reads.contains(&(receiver, key.to_string()))
    }

    /// The step result is a local created inside the callback, so its exact
    /// identity is not known here. The same scoped receiver must still expose
    /// both `done` and `value`; a method-name-only match on one of them is not
    /// enough.
    fn reads_step_result(&self) -> bool {
        let done = self.reads.iter().any(|(receiver, key)| {
            key == "done" && self.reads.contains(&(*receiver, "value".to_string()))
        });
        done
    }
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let arity = call
            .arguments
            .iter()
            .all(|argument| argument.as_expression().is_some())
            .then_some(call.arguments.len());
        let callee = call.callee.without_parentheses();
        match callee {
            Expression::StaticMemberExpression(member) if !member.optional => {
                let receiver = self.receiver(&member.object);
                self.calls.insert(CallFact {
                    receiver,
                    method: member.property.name.to_string(),
                    arity,
                });
            }
            Expression::ComputedMemberExpression(member) if !member.optional => {
                if let Expression::Identifier(receiver) = member.object.without_parentheses() {
                    if let Some(symbol) =
                        crate::analysis::identifier_symbol(receiver, self.model.scoping)
                    {
                        if global_member(self.model, &member.expression, "Symbol", "iterator") {
                            self.symbol_iterators.insert(symbol);
                        }
                    }
                }
                self.calls.insert(CallFact {
                    receiver: self.receiver(&member.object),
                    method: "[]".into(),
                    arity,
                });
            }
            Expression::Identifier(i) => {
                if let Some(symbol) = crate::analysis::identifier_symbol(i, self.model.scoping) {
                    self.calls.insert(CallFact {
                        receiver: Receiver::Direct(symbol),
                        method: String::new(),
                        arity,
                    });
                }
            }
            _ => (),
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_new_expression(&mut self, call: &NewExpression<'a>) {
        let arity = call
            .arguments
            .iter()
            .all(|argument| argument.as_expression().is_some())
            .then_some(call.arguments.len());
        match call.callee.without_parentheses() {
            Expression::Identifier(i) => {
                let symbol = crate::analysis::identifier_symbol(i, self.model.scoping);
                let name = if symbol.is_none() {
                    i.name.to_string()
                } else {
                    String::new()
                };
                self.news.insert((symbol, name, arity));
            }
            _ => {
                self.news.insert((None, String::new(), arity));
            }
        }
        walk::walk_new_expression(self, call);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        let receiver = self.receiver(&member.object);
        if !matches!(receiver, Receiver::Other) {
            self.reads
                .insert((receiver, member.property.name.to_string()));
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if let Expression::Identifier(object) = member.object.without_parentheses() {
            if let Some(symbol) = crate::analysis::identifier_symbol(object, self.model.scoping) {
                self.computed_reads.insert(symbol);
            }
        }
        walk::walk_computed_member_expression(self, member);
    }
}

fn require_calls(scan: &Scan<'_, '_>, required: &[(Receiver, &str, usize)]) -> bool {
    required
        .iter()
        .all(|(receiver, method, arity)| scan.calls(*receiver, method, *arity))
}

fn constructor_method(model: &Model<'_, '_>, function: &Function<'_>) -> Option<()> {
    let params = plain_params(model, function)?;
    let [subscriber] = params.as_slice() else {
        return None;
    };
    let body = body_of(function)?;
    let [branch] = body.statements.as_slice() else {
        return None;
    };
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    if identifier(model, typeof_test(&branch.test, "!==", "function")?) != Some(*subscriber) {
        return None;
    }
    let rejected = block(&branch.consequent)?;
    let [throw] = rejected else {
        return None;
    };
    let new = new_expression(throw_expression(throw)?)?;
    if !unbound(model, &new.callee, "TypeError") || !exact_new_arguments(new, 0)?.is_empty() {
        return None;
    }
    let accepted = block(branch.alternate.as_ref()?)?;
    let [store, finish] = accepted else {
        return None;
    };
    let store = expression_statement(store)?;
    let Expression::AssignmentExpression(store) = store.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(member) = &store.left else {
        return None;
    };
    if store.operator.as_str() != "="
        || member.optional
        || member.property.name != "_subscriber"
        || !matches!(
            member.object.without_parentheses(),
            Expression::ThisExpression(_)
        )
        || identifier(model, &store.right) != Some(*subscriber)
    {
        return None;
    }
    if !return_value(finish).is_some_and(is_void_zero) {
        return None;
    }
    Some(())
}

fn of_method(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    class: Symbol,
    alias: Symbol,
) -> Option<()> {
    if !plain_params(model, function)?.is_empty() {
        return None;
    }
    let body = body_of(function)?;
    let statements = body.statements.as_slice();
    let (_, array, _, consumed) = copy_arguments(model, statements)?;
    let [finish] = statements.get(consumed..)? else {
        return None;
    };
    let (_, executor) = new_instance(model, return_value(finish)?, class, alias)?;
    let args = plain_arrow_params(model, executor)?;
    let [subscriber] = args.as_slice() else {
        return None;
    };
    let [alias_statement, emit, complete] = executor.body.statements.as_slice() else {
        return None;
    };
    let alias_variable = one_variable(alias_statement)?;
    let handle = local(model, alias_variable)?;
    if identifier(model, alias_variable.init.as_ref()?) != Some(*subscriber)
        || !immutable(model, handle)
    {
        return None;
    }
    let emit = expression_statement(emit)?;
    let for_each = method_call(emit, "forEach")?;
    if !receiver_of(&for_each.callee, "forEach")
        .is_some_and(|receiver| identifier(model, receiver) == Some(array))
    {
        return None;
    }
    let args = exact_arguments(for_each, 1)?;
    let [callback] = args.as_slice() else {
        return None;
    };
    let Expression::ArrowFunctionExpression(callback) = callback.without_parentheses() else {
        return None;
    };
    let args = plain_arrow_params(model, callback)?;
    let [element] = args.as_slice() else {
        return None;
    };
    let forward = arrow_value(callback)?;
    let args = observer_call(model, forward, handle, "next", 1)?;
    if identifier(model, args[0]) != Some(*element) {
        return None;
    }
    observer_call(
        model,
        expression_statement(complete)?,
        handle,
        "complete",
        0,
    )?;
    Some(())
}

fn empty_method(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    class: Symbol,
    alias: Symbol,
) -> Option<()> {
    if !plain_params(model, function)?.is_empty() {
        return None;
    }
    let body = body_of(function)?;
    let [finish] = body.statements.as_slice() else {
        return None;
    };
    let (_, executor) = new_instance(model, return_value(finish)?, class, alias)?;
    let args = plain_arrow_params(model, executor)?;
    let [subscriber] = args.as_slice() else {
        return None;
    };
    let [complete] = executor.body.statements.as_slice() else {
        return None;
    };
    observer_call(
        model,
        expression_statement(complete)?,
        *subscriber,
        "complete",
        0,
    )?;
    Some(())
}

fn promise_executor(
    model: &Model<'_, '_>,
    executor: &ArrowFunctionExpression<'_>,
    source: Symbol,
) -> Option<()> {
    let args = plain_arrow_params(model, executor)?;
    let [subscriber] = args.as_slice() else {
        return None;
    };
    let [alias_statement, chain] = executor.body.statements.as_slice() else {
        return None;
    };
    let alias_variable = one_variable(alias_statement)?;
    let handle = local(model, alias_variable)?;
    if identifier(model, alias_variable.init.as_ref()?) != Some(*subscriber)
        || !immutable(model, handle)
    {
        return None;
    }
    let catch = method_call(expression_statement(chain)?, "catch")?;
    let args = exact_arguments(catch, 1)?;
    let [rejection] = args.as_slice() else {
        return None;
    };
    let then = method_call(receiver_of(&catch.callee, "catch")?, "then")?;
    if identifier(model, receiver_of(&then.callee, "then")?) != Some(source) {
        return None;
    }
    let args = exact_arguments(then, 1)?;
    let [value] = args.as_slice() else {
        return None;
    };
    let Expression::ArrowFunctionExpression(value) = value.without_parentheses() else {
        return None;
    };
    let args = plain_arrow_params(model, value)?;
    let [item] = args.as_slice() else {
        return None;
    };
    let [next, complete] = value.body.statements.as_slice() else {
        return None;
    };
    let args = observer_call(model, expression_statement(next)?, handle, "next", 1)?;
    if identifier(model, args[0]) != Some(*item) {
        return None;
    }
    observer_call(
        model,
        expression_statement(complete)?,
        handle,
        "complete",
        0,
    )?;
    let Expression::ArrowFunctionExpression(rejection) = rejection.without_parentheses() else {
        return None;
    };
    let args = plain_arrow_params(model, rejection)?;
    let [error] = args.as_slice() else {
        return None;
    };
    let [forward] = rejection.body.statements.as_slice() else {
        return None;
    };
    let args = observer_call(model, expression_statement(forward)?, handle, "error", 1)?;
    (identifier(model, args[0]) == Some(*error)).then_some(())
}

fn scheduled_if<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    scheduler: Symbol,
) -> Option<(&'a ArrowFunctionExpression<'a>, &'a Statement<'a>)> {
    let Statement::IfStatement(branch) = statement else {
        return None;
    };
    let (left, right) = match binary(&branch.test, "!=") {
        Some(value) => value,
        None => {
            return None;
        }
    };
    if identifier(model, left) != Some(scheduler) || !is_null(right) {
        return None;
    }
    let scheduled = match block(&branch.consequent) {
        Some(value) => value,
        None => {
            return None;
        }
    };
    let schedule_statement = match scheduled {
        [only] => only,
        [index_statement, schedule_statement] => {
            index_zero(model, index_statement)?;
            schedule_statement
        }
        _ => {
            return None;
        }
    };
    let schedule = match method_call(expression_statement(schedule_statement)?, "schedule") {
        Some(value) => value,
        None => {
            return None;
        }
    };
    if !receiver_of(&schedule.callee, "schedule")
        .is_some_and(|receiver| identifier(model, receiver) == Some(scheduler))
    {
        return None;
    }
    let args = exact_arguments(schedule, 1)?;
    let [callback] = args.as_slice() else {
        return None;
    };
    let Expression::ArrowFunctionExpression(callback) = callback.without_parentheses() else {
        return None;
    };
    if !plain_arrow_params(model, callback)?.is_empty() {
        return None;
    }
    Some((callback, branch.alternate.as_ref()?))
}

/// A bounded synchronous copy loop in the unscheduled branch: `while (i < n)`
/// or the lowered `for (; i < n; i++)`. The deliberate infinite
/// `for (;;)` iterator form is recognized separately.
fn has_while(statement: &Statement<'_>) -> bool {
    let Some(block) = block(statement) else {
        return false;
    };
    let [try_] = block else {
        return false;
    };
    let Statement::TryStatement(try_) = try_ else {
        return false;
    };
    try_.block.body.iter().any(|statement| match statement {
        Statement::WhileStatement(_) => true,
        Statement::ForStatement(loop_) => loop_.test.is_some(),
        _ => false,
    })
}

fn has_infinite_for(statement: &Statement<'_>) -> bool {
    block(statement).is_some_and(|statements| {
        statements.iter().any(|statement| {
            matches!(statement, Statement::ForStatement(loop_)
                if loop_.init.is_none() && loop_.test.is_none() && loop_.update.is_none())
        })
    })
}

fn array_executor(
    model: &Model<'_, '_>,
    executor: &ArrowFunctionExpression<'_>,
    scheduler: Symbol,
    source: Symbol,
) -> Option<()> {
    let args = plain_arrow_params(model, executor)?;
    let [subscriber] = args.as_slice() else {
        return None;
    };
    let (handle, branch) =
        executor_branch(model, executor.body.statements.as_slice(), *subscriber)?;
    let (_, alternate) = scheduled_if(model, branch, scheduler)?;
    if !has_while(alternate) {
        return None;
    }
    let mut scan = Scan::new(model);
    scan.visit_function_body(&executor.body);
    if !require_calls(
        &scan,
        &[
            (Receiver::Symbol(scheduler), "schedule", 1),
            (Receiver::Symbol(handle), "next", 1),
            (Receiver::Symbol(handle), "complete", 0),
            (Receiver::Symbol(handle), "error", 1),
        ],
    ) || !scan.reads(Receiver::Symbol(source), "length")
        || !scan.computed_reads.contains(&source)
    {
        return None;
    }
    Some(())
}

fn iterator_expression(model: &Model<'_, '_>, e: &Expression<'_>, source: Symbol) -> Option<()> {
    let Expression::ComputedMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    if m.optional
        || identifier(model, &m.object) != Some(source)
        || !global_member(model, &m.expression, "Symbol", "iterator")
    {
        return None;
    }
    Some(())
}

fn iterator_executor(
    model: &Model<'_, '_>,
    executor: &ArrowFunctionExpression<'_>,
    scheduler: Symbol,
    source: Symbol,
) -> Option<()> {
    let args = plain_arrow_params(model, executor)?;
    let [subscriber] = args.as_slice() else {
        return None;
    };
    let (handle, branch) =
        executor_branch(model, executor.body.statements.as_slice(), *subscriber)?;
    let Statement::TryStatement(try_) = branch else {
        return None;
    };
    if try_.finalizer.is_some() {
        return None;
    }
    let [iterator_statement, branch] = try_.block.body.as_slice() else {
        return None;
    };
    let step = iterator_step(model, iterator_statement, source)?;
    let (_, alternate) = scheduled_if(model, branch, scheduler)?;
    if !has_infinite_for(alternate) {
        return None;
    }
    let mut scan = Scan::new(model);
    scan.visit_function_body(&executor.body);
    if !require_calls(
        &scan,
        &[
            (Receiver::Symbol(scheduler), "schedule", 1),
            (Receiver::Symbol(step), "next", 0),
            (Receiver::Symbol(handle), "next", 1),
            (Receiver::Symbol(handle), "complete", 0),
            (Receiver::Symbol(handle), "error", 1),
        ],
    ) || !scan.reads_step_result()
        || !scan.symbol_iterators.contains(&source)
    {
        return None;
    }
    Some(())
}

fn completes(model: &Model<'_, '_>, statement: &Statement<'_>, handle: Symbol) -> Option<()> {
    observer_call(
        model,
        expression_statement(statement)?,
        handle,
        "complete",
        0,
    )?;
    Some(())
}

fn returns_bool(statement: &Statement<'_>, value: bool) -> Option<()> {
    matches!(return_value(statement)?.without_parentheses(), Expression::BooleanLiteral(b) if b.value == value).then_some(())
}

fn increments(model: &Model<'_, '_>, expression: &Expression<'_>, index: Symbol) -> Option<()> {
    let Expression::UpdateExpression(update) = expression.without_parentheses() else {
        return None;
    };
    (update.operator.as_str() == "++"
        && matches!(&update.argument,
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id)
        if crate::analysis::identifier_symbol(id, model.scoping) == Some(index)))
    .then_some(())
}

/// A complete error handler, with no nested evidence or extra effects.
fn forwards_error(
    model: &Model<'_, '_>,
    try_: &TryStatement<'_>,
    handle: Symbol,
    scheduled: bool,
) -> Option<()> {
    if try_.finalizer.is_some() {
        return None;
    }
    let handler = try_.handler.as_ref()?;
    let error = plain_binding(model, &handler.param.as_ref()?.pattern)?;
    if !immutable(model, error) {
        return None;
    }
    let (forward, rest) = handler.body.body.split_first()?;
    let args = observer_call(model, expression_statement(forward)?, handle, "error", 1)?;
    if identifier(model, args[0]) != Some(error) {
        return None;
    }
    match (scheduled, rest) {
        (false, []) => Some(()),
        (true, [finish]) => returns_bool(finish, false),
        _ => None,
    }
}

fn emits_index(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    handle: Symbol,
    source: Symbol,
    index: Symbol,
) -> Option<()> {
    let args = observer_call(model, expression_statement(statement)?, handle, "next", 1)?;
    let Expression::ComputedMemberExpression(member) = args[0].without_parentheses() else {
        return None;
    };
    (!member.optional
        && identifier(model, &member.object) == Some(source)
        && identifier(model, &member.expression) == Some(index))
    .then_some(())
}

/// Unlike the legacy scan, these alternatives consume each statement in its
/// actual callable. A nested arrow, donor class or extra write cannot provide
/// missing loop behavior for a newly accepted factory or a class claim.
fn strict_array_executor(
    model: &Model<'_, '_>,
    executor: &ArrowFunctionExpression<'_>,
    scheduler: Symbol,
    source: Symbol,
    additional_lowerings: bool,
) -> Option<()> {
    array_executor_variant(
        model,
        executor,
        scheduler,
        source,
        additional_lowerings,
        false,
    )
}

fn array_executor_variant(
    model: &Model<'_, '_>,
    executor: &ArrowFunctionExpression<'_>,
    scheduler: Symbol,
    source: Symbol,
    additional_lowerings: bool,
    mixed: bool,
) -> Option<()> {
    let params = plain_arrow_params(model, executor)?;
    let [subscriber] = params.as_slice() else {
        return None;
    };
    let statements = executor.body.statements.as_slice();
    let (handle, branch) = executor_branch(model, statements, *subscriber)?;
    let branch_statement = branch;
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let scheduled = block(&branch.consequent)?;
    let [init, _] = scheduled else {
        return None;
    };
    let index = index_zero(model, init)?;
    match statements {
        [_, declared, _] => {
            let variable = one_variable(declared)?;
            if local(model, variable)? != index
                || variable.init.is_some()
                || assignment_value_to_identifier(model, init, index).is_none()
            {
                return None;
            }
        }
        [_, _] => {
            one_variable(init)?;
        }
        _ => return None,
    }
    // The only writes are the unconditional zero initialization and increment.
    let expected_writes = if one_variable(init).is_some() { 1 } else { 2 };
    if model.binding_writes(index) != expected_writes {
        return None;
    }
    let (callback, alternate) = scheduled_if(model, branch_statement, scheduler)?;
    let [Statement::TryStatement(try_)] = callback.body.statements.as_slice() else {
        return None;
    };
    forwards_error(model, try_, handle, true)?;
    let [Statement::IfStatement(done)] = try_.block.body.as_slice() else {
        return None;
    };
    let (left, right) = binary(&done.test, ">=")?;
    if identifier(model, left) != Some(index) || !member_of(model, right, source, "length") {
        return None;
    }
    let [complete, finish] = block(&done.consequent)? else {
        return None;
    };
    completes(model, complete, handle)?;
    returns_bool(finish, false)?;
    let [emit, increment, finish] = block(done.alternate.as_ref()?)? else {
        return None;
    };
    emits_index(model, emit, handle, source, index)?;
    increments(model, expression_statement(increment)?, index)?;
    returns_bool(finish, true)?;
    if lowered_array_loop(model, alternate, handle, source).is_some()
        || invoked_array_loop(model, alternate, handle, source, additional_lowerings).is_some()
        || (mixed
            && mixed_from::array_loop(model, alternate, handle, source)
                .or_else(|| mixed_from::array_first_loop(model, alternate, handle, source))
                .is_some())
    {
        return Some(());
    }
    let [Statement::TryStatement(try_)] = block(alternate)? else {
        return None;
    };
    forwards_error(model, try_, handle, false)?;
    let (loop_, index, complete) = match try_.block.body.as_slice() {
        [Statement::ForStatement(loop_), complete] => {
            (loop_, inline_index(model, loop_.init.as_ref()?)?, complete)
        }
        [init, Statement::ForStatement(loop_), complete] => {
            if loop_.init.is_some() {
                return None;
            }
            (loop_, index_zero(model, init)?, complete)
        }
        [init, Statement::WhileStatement(loop_), complete] => {
            let index = index_zero(model, init)?;
            one_variable(init)?;
            let (left, right) = binary(&loop_.test, "<")?;
            if identifier(model, left) != Some(index) || !member_of(model, right, source, "length")
            {
                return None;
            }
            let [emit, increment] = block(&loop_.body)? else {
                return None;
            };
            emits_index(model, emit, handle, source, index)?;
            increments(model, expression_statement(increment)?, index)?;
            if model.binding_writes(index) != 1 {
                return None;
            }
            return completes(model, complete, handle);
        }
        _ => return None,
    };
    let (left, right) = binary(loop_.test.as_ref()?, "<")?;
    if identifier(model, left) != Some(index)
        || !member_of(model, right, source, "length")
        || model.binding_writes(index) != 1
    {
        return None;
    }
    increments(model, loop_.update.as_ref()?, index)?;
    let [emit] = block(&loop_.body)? else {
        return None;
    };
    emits_index(model, emit, handle, source, index)?;
    completes(model, complete, handle)
}

/// Consume the emission-in-test do-while alternative in execution order.
/// Sentinel writes and catch failures are part of the contract, not dead code.
/// All temporary identities originate here and every reference stays here.
fn lowered_array_loop(
    model: &Model<'_, '_>,
    alternate: &Statement<'_>,
    handle: Symbol,
    source: Symbol,
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
    let [init, Statement::VariableDeclaration(temporaries), Statement::DoWhileStatement(loop_), complete] =
        try_.block.body.as_slice()
    else {
        return None;
    };
    one_variable(init)?;
    let index = index_zero(model, init)?;
    let [array, receiver, method, value] = temporaries.declarations.as_slice() else {
        return None;
    };
    if temporaries.declarations.iter().any(|v| v.init.is_some()) {
        return None;
    }
    let array = local(model, array)?;
    let receiver = local(model, receiver)?;
    let method = local(model, method)?;
    let value = local(model, value)?;
    let locals = [
        sentinel_a, sentinel_b, index, array, receiver, method, value,
    ];
    if !distinct(&locals) || locals.contains(&handle) || locals.contains(&source) {
        return None;
    }
    for temporary in locals {
        if model.binding_writes(temporary) != 1
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
    let [mark_a, mark_b, refresh] = block(&loop_.body)? else {
        return None;
    };
    // The numeric payloads are private defined sentinels: only undefined is
    // tested in the catch, so their values carry no observable identity.
    if !matches!(
        assignment_value_to_identifier(model, mark_a, sentinel_a)?.without_parentheses(),
        Expression::NumericLiteral(_)
    ) || !matches!(
        assignment_value_to_identifier(model, mark_b, sentinel_b)?.without_parentheses(),
        Expression::NumericLiteral(_)
    ) || identifier(
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
    let [capture_receiver, capture_method, capture_value, emit, increment, truth] =
        effects.expressions.as_slice()
    else {
        return None;
    };
    if identifier(
        model,
        assigned_expression(model, capture_receiver, receiver)?,
    ) != Some(handle)
        || !member_of(
            model,
            assigned_expression(model, capture_method, method)?,
            receiver,
            "next",
        )
        || !matches!(truth.without_parentheses(), Expression::BooleanLiteral(b) if b.value)
    {
        return None;
    }
    let Expression::ComputedMemberExpression(element) =
        assigned_expression(model, capture_value, value)?.without_parentheses()
    else {
        return None;
    };
    if element.optional
        || identifier(model, &element.object) != Some(source)
        || identifier(model, &element.expression) != Some(index)
    {
        return None;
    }
    let args = observer_call(model, emit, method, "call", 2)?;
    if identifier(model, args[0]) != Some(receiver) || identifier(model, args[1]) != Some(value) {
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

/// A second bounded emission-in-test lowering. The body always executes and
/// writes both sentinels before the test, including the empty-array case.
/// The IIFE reads the element after capturing receiver/method and before the
/// numeric index update. Its four immutable formals are linked positionally
/// to those exact actuals; no sibling callback supplies emission evidence.
fn invoked_array_loop(
    model: &Model<'_, '_>,
    alternate: &Statement<'_>,
    handle: Symbol,
    source: Symbol,
    additional_lowerings: bool,
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
    let receiver = one_variable(temporary)?;
    if receiver.init.is_some() {
        return None;
    }
    let receiver = local(model, receiver)?;
    let [mark_a, mark_b] = block(&loop_.body)? else {
        return None;
    };
    // The complete extension follows executed stores, independently of the
    // neutral declaration order. Canonical witnesses retain their own domain.
    let (sentinel_a, sentinel_b) = if assignment_value_to_identifier(model, mark_a, sentinel_a)
        .is_some()
        && assignment_value_to_identifier(model, mark_b, sentinel_b).is_some()
    {
        (sentinel_a, sentinel_b)
    } else if additional_lowerings
        && assignment_value_to_identifier(model, mark_a, sentinel_b).is_some()
        && assignment_value_to_identifier(model, mark_b, sentinel_a).is_some()
    {
        (sentinel_b, sentinel_a)
    } else {
        return None;
    };
    let locals = [sentinel_a, sentinel_b, index, receiver];

    if !distinct(&locals) || locals.contains(&handle) || locals.contains(&source) {
        return None;
    }
    for temporary in locals {
        if model.binding_writes(temporary) != 1
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
    let [mark_a, mark_b] = block(&loop_.body)? else {
        return None;
    };
    if !matches!(
        assignment_value_to_identifier(model, mark_a, sentinel_a)?.without_parentheses(),
        Expression::NumericLiteral(_)
    ) || !matches!(
        assignment_value_to_identifier(model, mark_b, sentinel_b)?.without_parentheses(),
        Expression::NumericLiteral(_)
    ) {
        return None;
    }
    let (bound, effects) = logical(&loop_.test, "&&")?;
    let (left, right) = binary(bound, "<")?;
    if identifier(model, left) != Some(index) || !member_of(model, right, source, "length") {
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

fn assigned_expression<'a>(
    model: &Model<'_, '_>,
    expression: &'a Expression<'a>,
    target: Symbol,
) -> Option<&'a Expression<'a>> {
    let Expression::AssignmentExpression(assigned) = expression.without_parentheses() else {
        return None;
    };
    (assigned.operator.as_str() == "="
        && matches!(&assigned.left, AssignmentTarget::AssignmentTargetIdentifier(id)
            if crate::analysis::identifier_symbol(id, model.scoping) == Some(target)))
    .then_some(&assigned.right)
}

fn sentinel_failure(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    sentinel: Symbol,
    message: &str,
) -> Option<()> {
    let Statement::IfStatement(branch) = statement else {
        return None;
    };
    let (left, right) = binary(&branch.test, "===")?;
    if branch.alternate.is_some()
        || identifier(model, left) != Some(sentinel)
        || !is_void_zero(right)
    {
        return None;
    }
    let [failure] = block(&branch.consequent)? else {
        return None;
    };
    let new = new_expression(throw_expression(failure)?)?;
    (unbound(model, &new.callee, "TypeError")
        && is_string(exact_new_arguments(new, 1)?[0], message))
    .then_some(())
}

fn strict_iterator_executor(
    model: &Model<'_, '_>,
    executor: &ArrowFunctionExpression<'_>,
    scheduler: Symbol,
    source: Symbol,
) -> Option<()> {
    let params = plain_arrow_params(model, executor)?;
    let [subscriber] = params.as_slice() else {
        return None;
    };
    let statements = executor.body.statements.as_slice();
    let (handle, branch) = executor_branch(model, statements, *subscriber)?;
    let Statement::TryStatement(try_) = branch else {
        return None;
    };
    forwards_error(model, try_, handle, false)?;
    let [init, branch] = try_.block.body.as_slice() else {
        return None;
    };
    let iterator = iterator_step(model, init, source)?;
    match statements {
        [_, declared, _] => {
            let variable = one_variable(declared)?;
            if local(model, variable)? != iterator
                || variable.init.is_some()
                || model.binding_writes(iterator) != 1
            {
                return None;
            }
            assignment_value_to_identifier(model, init, iterator)?;
        }
        [_, _] => {
            one_variable(init)?;
            if !immutable(model, iterator) {
                return None;
            }
        }
        _ => return None,
    }
    let Statement::IfStatement(scheduled) = branch else {
        return None;
    };
    if block(&scheduled.consequent)?.len() != 1 {
        return None;
    }
    let (callback, alternate) = scheduled_if(model, branch, scheduler)?;
    let [Statement::TryStatement(try_)] = callback.body.statements.as_slice() else {
        return None;
    };
    forwards_error(model, try_, handle, true)?;
    let [step, Statement::IfStatement(done)] = try_.block.body.as_slice() else {
        return None;
    };
    let result = strict_step(model, step, iterator)?;
    if !member_of(model, &done.test, result, "done") {
        return None;
    }
    let [complete, finish] = block(&done.consequent)? else {
        return None;
    };
    completes(model, complete, handle)?;
    returns_bool(finish, false)?;
    let [emit, finish] = block(done.alternate.as_ref()?)? else {
        return None;
    };
    strict_value(model, emit, handle, result)?;
    returns_bool(finish, true)?;
    if lowered_iterator_loop(model, alternate, handle, iterator).is_some() {
        return Some(());
    }
    let [Statement::ForStatement(loop_), complete] = block(alternate)? else {
        return None;
    };
    if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() {
        return None;
    }
    let [step, Statement::IfStatement(done), emit] = block(&loop_.body)? else {
        return None;
    };
    let result = strict_step(model, step, iterator)?;
    if !member_of(model, &done.test, result, "done") || done.alternate.is_some() {
        return None;
    }
    let Statement::BreakStatement(break_) = &done.consequent else {
        return None;
    };
    if break_.label.is_some() {
        return None;
    }
    strict_value(model, emit, handle, result)?;
    completes(model, complete, handle)
}

/// Consume the exact step/test/emission sequence. The IIFE owns its method,
/// receiver, result and key formals; none can be supplied by donor code.
fn lowered_iterator_loop(
    model: &Model<'_, '_>,
    alternate: &Statement<'_>,
    handle: Symbol,
    iterator: Symbol,
) -> Option<()> {
    let [Statement::VariableDeclaration(declaration), Statement::DoWhileStatement(loop_), complete] =
        block(alternate)?
    else {
        return None;
    };
    let [result, receiver] = declaration.declarations.as_slice() else {
        return None;
    };
    if result.init.is_some() || receiver.init.is_some() {
        return None;
    }
    let result = local(model, result)?;
    let receiver = local(model, receiver)?;
    if model.binding_writes(result) != 1 || model.binding_writes(receiver) != 1 {
        return None;
    }
    let [step] = block(&loop_.body)? else {
        return None;
    };
    let step = assignment_value_to_identifier(model, step, result)?;
    observer_call(model, step, iterator, "next", 0)?;
    let (done, emit) = logical(&loop_.test, "&&")?;
    let Expression::UnaryExpression(done) = done.without_parentheses() else {
        return None;
    };
    if done.operator.as_str() != "!" || !member_of(model, &done.argument, result, "done") {
        return None;
    }
    let Expression::SequenceExpression(sequence) = emit.without_parentheses() else {
        return None;
    };
    let [capture, invoke, yes] = sequence.expressions.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(capture) = capture.without_parentheses() else {
        return None;
    };
    if capture.operator.as_str() != "="
        || !matches!(&capture.left, AssignmentTarget::AssignmentTargetIdentifier(id) if crate::analysis::identifier_symbol(id, model.scoping) == Some(receiver))
        || identifier(model, &capture.right) != Some(handle)
        || !matches!(yes, Expression::BooleanLiteral(b) if b.value)
    {
        return None;
    }
    let invoke = plain_call(invoke)?;
    let Expression::ArrowFunctionExpression(callback) = invoke.callee.without_parentheses() else {
        return None;
    };
    if callback.r#async {
        return None;
    }
    let params = plain_arrow_params(model, callback)?;
    let [recv, method, value, key] = params.as_slice() else {
        return None;
    };
    let args = exact_arguments(invoke, 4)?;
    if identifier(model, args[0]) != Some(receiver)
        || !member_of(model, args[1], receiver, "next")
        || identifier(model, args[2]) != Some(result)
        || !is_string(args[3], "value")
    {
        return None;
    }
    let [forward] = callback.body.statements.as_slice() else {
        return None;
    };
    if callback.expression {
        return None;
    }
    let forward = plain_call(expression_statement(forward)?)?;
    if !member_of(model, &forward.callee, *method, "call") {
        return None;
    }
    let args = exact_arguments(forward, 2)?;
    let Expression::ComputedMemberExpression(read) = args[1].without_parentheses() else {
        return None;
    };
    if identifier(model, args[0]) != Some(*recv)
        || read.optional
        || identifier(model, &read.object) != Some(*value)
        || identifier(model, &read.expression) != Some(*key)
    {
        return None;
    }
    completes(model, complete, handle)
}

fn strict_step(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    iterator: Symbol,
) -> Option<Symbol> {
    let variable = one_variable(statement)?;
    let result = local(model, variable)?;
    if !immutable(model, result) {
        return None;
    }
    observer_call(model, variable.init.as_ref()?, iterator, "next", 0)?;
    Some(result)
}

fn strict_value(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    handle: Symbol,
    result: Symbol,
) -> Option<()> {
    let args = observer_call(model, expression_statement(statement)?, handle, "next", 1)?;
    member_of(model, args[0], result, "value").then_some(())
}

fn guard_terms<'a>(expression: &'a Expression<'a>) -> Vec<&'a Expression<'a>> {
    if let Some((left, right)) = logical(expression, "&&") {
        let mut terms = guard_terms(left);
        terms.extend(guard_terms(right));
        terms
    } else {
        vec![expression]
    }
}

/// Recover a short-circuit conjunction, including the lowering
/// `let t; if (a) { t = b; } else { t = a; } if (t) ...`.
/// The split assignments are direct, exhaustive, ordered and single-origin.
fn protocol_branch<'a>(
    model: &Model<'_, '_>,
    statements: &'a [Statement<'a>],
    source: Symbol,
    extended: bool,
    additional_lowerings: bool,
) -> Option<(
    &'a IfStatement<'a>,
    Vec<&'a Expression<'a>>,
    &'a [Statement<'a>],
)> {
    if let [Statement::IfStatement(branch), rest @ ..] = statements {
        if extended && branch.alternate.is_some() {
            return None;
        }
        return Some((branch, guard_terms(&branch.test), rest));
    }
    if !extended {
        return None;
    }
    let [declaration, Statement::IfStatement(split), Statement::IfStatement(branch), rest @ ..] =
        statements
    else {
        return None;
    };
    let variable = one_variable(declaration)?;
    let alias = local(model, variable)?;
    if variable.init.is_some() || model.binding_writes(alias) != 2 || branch.alternate.is_some() {
        return None;
    }
    let [yes] = block(&split.consequent)? else {
        return None;
    };
    let [no] = block(split.alternate.as_ref()?)? else {
        return None;
    };
    let yes = assignment_value_to_identifier(model, yes, alias)?;
    let no = assignment_value_to_identifier(model, no, alias)?;
    // Globals repeat the same safe typeof test; the array guard uses false,
    // so a potentially effectful length getter is never evaluated twice.
    let same = ["Promise", "Symbol"].iter().any(|global| {
        typeof_test(&split.test, "!==", "undefined").is_some_and(|e| unbound(model, e, global))
            && (typeof_test(no, "!==", "undefined").is_some_and(|e| unbound(model, e, global)) || (additional_lowerings && matches!(no.without_parentheses(), Expression::BooleanLiteral(value) if !value.value)))
    }) || (typeof_test(&split.test, "===", "number")
        .is_some_and(|e| member_of(model, e, source, "length"))
        && matches!(no.without_parentheses(), Expression::BooleanLiteral(value) if !value.value));
    if !same {
        return None;
    }
    let mut terms = vec![&split.test, yes];
    if identifier(model, &branch.test) != Some(alias) {
        let (left, right) = logical(&branch.test, "&&")?;
        if identifier(model, left) != Some(alias) {
            return None;
        }
        terms.push(right);
    }
    Some((branch, terms, rest))
}

fn from_method(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    class: Symbol,
    alias: Symbol,
    strict: bool,
    additional_lowerings: bool,
) -> Option<()> {
    from_method_variant(
        model,
        function,
        class,
        alias,
        strict,
        additional_lowerings,
        false,
    )
}

fn from_method_variant(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    class: Symbol,
    alias: Symbol,
    strict: bool,
    additional_lowerings: bool,
    mixed: bool,
) -> Option<()> {
    let params = plain_params(model, function)?;
    let [input, scheduler] = params.as_slice() else {
        return None;
    };
    let body = body_of(function)?;
    let statements = body.statements.as_slice();
    let (aliases, guarded, failure) = match statements {
        [first, second, guarded, failure] => (vec![first, second], guarded, failure),
        [first, second, third, guarded, failure] => (vec![first, second, third], guarded, failure),
        _ => return None,
    };
    // The host lowering may emit input/scheduler aliases in either order. A
    // third bare array alias is the older form; the newer form creates the
    // array alias inside the array-like branch. Recover each role from its
    // scoped identity instead of declaration order.
    let mut source = None;
    let mut array = None;
    let mut schedule = None;
    for statement in aliases {
        let variable = one_variable(statement)?;
        let local = local(model, variable)?;
        match variable.init.as_ref() {
            None if array.replace(local).is_none() => {}
            Some(init)
                if identifier(model, init) == Some(*input) && source.replace(local).is_none() => {}
            Some(init)
                if identifier(model, init) == Some(*scheduler)
                    && schedule.replace(local).is_none() => {}
            _ => return None,
        }
    }
    let source = source?;
    let schedule = schedule?;
    if !distinct(&[source, schedule])
        || array.is_some_and(|array| array == source || array == schedule)
        || !immutable(model, source)
        || !immutable(model, schedule)
    {
        return None;
    }
    let Statement::IfStatement(guard) = guarded else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let (left, right) = binary(&guard.test, "!=")?;
    if identifier(model, left) != Some(*input) || !is_null(right) {
        return None;
    }
    let protocol = block(&guard.consequent)?;
    let [instance, rest @ ..] = protocol else {
        return None;
    };
    let (promise, promise_terms, rest) =
        protocol_branch(model, rest, source, strict, additional_lowerings)?;
    let (array_like, array_terms, rest) =
        protocol_branch(model, rest, source, strict, additional_lowerings)?;
    let (iterator, iterator_terms, rest) =
        protocol_branch(model, rest, source, strict, additional_lowerings)?;
    if !rest.is_empty() {
        return None;
    }
    let Statement::IfStatement(instance) = instance else {
        return None;
    };
    let (left, right) = binary(&instance.test, "instanceof")?;
    if identifier(model, left) != Some(*input) || !class_identity(model, right, class, alias) {
        return None;
    }
    let returned = block(&instance.consequent)?;
    let [returned] = returned else {
        return None;
    };
    let returned = return_value(returned)?;
    if identifier(model, returned).is_none_or(|s| s != *input && s != source) {
        return None;
    }
    let [left, right] = promise_terms.as_slice() else {
        return None;
    };
    if !unbound(model, typeof_test(left, "!==", "undefined")?, "Promise") {
        return None;
    }
    let (left, right) = binary(right, "instanceof")?;
    if identifier(model, left).is_none_or(|s| s != *input && s != source)
        || !unbound(model, right, "Promise")
    {
        return None;
    }
    let returned = block(&promise.consequent)?;
    let [returned] = returned else {
        return None;
    };
    let (_, executor) = new_instance(model, return_value(returned)?, class, alias)?;
    promise_executor(model, executor, source)?;
    let [left, right] = array_terms.as_slice() else {
        return None;
    };
    let length_test = typeof_test(left, "===", "number")?;
    if !member_of(model, length_test, source, "length")
        || typeof_test(right, "!==", "function").is_none_or(|tested| {
            identifier(model, tested).is_none_or(|s| s != source && s != *input)
        })
    {
        return None;
    }
    let returned = block(&array_like.consequent)?;
    let [store, returned] = returned else {
        return None;
    };
    let array = match array {
        Some(array) => {
            let store = expression_statement(store)?;
            let Expression::AssignmentExpression(store) = store.without_parentheses() else {
                return None;
            };
            if store.operator.as_str() != "="
                || !matches!(&store.left, AssignmentTarget::AssignmentTargetIdentifier(id)
                    if crate::analysis::identifier_symbol(id, model.scoping) == Some(array))
                || identifier(model, &store.right) != Some(source)
            {
                return None;
            }
            array
        }
        None => {
            let variable = one_variable(store)?;
            let array = local(model, variable)?;
            if !immutable(model, array)
                || identifier(model, variable.init.as_ref()?) != Some(source)
            {
                return None;
            }
            array
        }
    };
    let (_, executor) = new_instance(model, return_value(returned)?, class, alias)?;
    if strict {
        if mixed {
            array_executor_variant(model, executor, schedule, array, additional_lowerings, true)?;
        } else {
            strict_array_executor(model, executor, schedule, array, additional_lowerings)?;
        }
    } else {
        array_executor(model, executor, schedule, array)?;
    }
    let [left, right, outer_right] = iterator_terms.as_slice() else {
        return None;
    };
    if !unbound(model, typeof_test(left, "!==", "undefined")?, "Symbol")
        || !global_member(model, right, "Symbol", "iterator")
    {
        return None;
    }
    iterator_expression(model, outer_right, source)?;
    let returned = block(&iterator.consequent)?;
    let [returned] = returned else {
        return None;
    };
    let (_, executor) = new_instance(model, return_value(returned)?, class, alias)?;
    if strict {
        strict_iterator_executor(model, executor, schedule, source)?;
    } else {
        iterator_executor(model, executor, schedule, source)?;
    }
    let new = new_expression(throw_expression(failure)?)?;
    if !unbound(model, &new.callee, "Error") {
        return None;
    }
    let args = exact_new_arguments(new, 1)?;
    let [message] = args.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(tail) = message.without_parentheses() else {
        return None;
    };
    if tail.operator.as_str() != "+" || !is_string(&tail.right, " to observable") {
        return None;
    }
    let Expression::BinaryExpression(head) = tail.left.without_parentheses() else {
        return None;
    };
    (head.operator.as_str() == "+"
        && is_string(&head.left, "Could not convert ")
        && identifier(model, &head.right) == Some(source))
    .then_some(())
}

fn object_value<'a>(property: &'a ObjectPropertyKind<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let ObjectPropertyKind::ObjectProperty(p) = property else {
        return None;
    };
    if p.computed || p.method || p.kind != PropertyKind::Init {
        return None;
    }
    (property_key_name(&p.key)? == key).then_some(&p.value)
}

/// The fully resolved observer literal. `next`, `error` and `complete` are each
/// either the canonical inline conditional or a prelude slot already holding
/// that value.
fn observer_object(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    observer: Symbol,
    next_alias: Option<Symbol>,
    error_alias: Option<Symbol>,
    error_slot: Symbol,
    complete_alias: Option<Symbol>,
    complete_slot: Symbol,
) -> Option<()> {
    let Expression::ObjectExpression(object) = e.without_parentheses() else {
        return None;
    };
    let [start_property, next_property, error_property, complete_property] =
        object.properties.as_slice()
    else {
        return None;
    };
    observer_start(model, object_value(start_property, "start")?, observer)?;
    let next = object_value(next_property, "next")?;
    match next_alias {
        Some(alias) => (identifier(model, next) == Some(alias)).then_some(())?,
        None => observer_next(model, next, observer)?,
    }
    let error = object_value(error_property, "error")?;
    match error_alias {
        Some(alias) => (identifier(model, error) == Some(alias)).then_some(())?,
        None => observer_error(model, error, observer, error_slot)?,
    }
    let complete = object_value(complete_property, "complete")?;
    match complete_alias {
        Some(alias) => (identifier(model, complete) == Some(alias)).then_some(())?,
        None => observer_complete(model, complete, observer, complete_slot)?,
    }
    Some(())
}

/// The placeholder form: the literal keeps `start` but leaves `next`, `error`
/// and `complete` as `void 0`, then three member writes fill the same handle.
fn observer_placeholder(model: &Model<'_, '_>, e: &Expression<'_>, observer: Symbol) -> Option<()> {
    let Expression::ObjectExpression(object) = e.without_parentheses() else {
        return None;
    };
    let [start_property, next_property, error_property, complete_property] =
        object.properties.as_slice()
    else {
        return None;
    };
    observer_start(model, object_value(start_property, "start")?, observer)?;
    (is_void_zero(object_value(next_property, "next")?)
        && is_void_zero(object_value(error_property, "error")?)
        && is_void_zero(object_value(complete_property, "complete")?))
    .then_some(())
}

/// The partly inlined placeholder: `start` and `next` are already resolved
/// inline and only `error`/`complete` are left as `void 0` for the following
/// same-handle writes. The next value is the canonical inline conditional, so
/// the two-write lowering cannot smuggle a foreign next.
fn observer_partial_literal(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    observer: Symbol,
) -> Option<()> {
    let Expression::ObjectExpression(object) = e.without_parentheses() else {
        return None;
    };
    let [start_property, next_property, error_property, complete_property] =
        object.properties.as_slice()
    else {
        return None;
    };
    observer_start(model, object_value(start_property, "start")?, observer)?;
    observer_next(model, object_value(next_property, "next")?, observer)?;
    (is_void_zero(object_value(error_property, "error")?)
        && is_void_zero(object_value(complete_property, "complete")?))
    .then_some(())
}

fn observer_writes<'a>(
    model: &Model<'_, '_>,
    next: &'a Statement<'a>,
    error: &'a Statement<'a>,
    complete: &'a Statement<'a>,
    handle: Symbol,
    observer: Symbol,
    next_alias: Option<Symbol>,
    error_slot: Symbol,
    complete_slot: Symbol,
) -> Option<()> {
    let next_value = assignment_value(model, next, handle, "next")?;
    match next_alias {
        Some(alias) => (identifier(model, next_value) == Some(alias)).then_some(())?,
        None => observer_next(model, next_value, observer)?,
    }
    observer_error(
        model,
        assignment_value(model, error, handle, "error")?,
        observer,
        error_slot,
    )?;
    observer_complete(
        model,
        assignment_value(model, complete, handle, "complete")?,
        observer,
        complete_slot,
    )?;
    Some(())
}

/// The split normalization lowering keeps a predeclared next slot, fills it
/// in an observer/function `if`, then copies that slot into the placeholder
/// observer record.  The slot is accepted only when both assignments resolve
/// to the same local and the two source expressions are the canonical
/// observer-derived values.
fn observer_next_prelude(
    model: &Model<'_, '_>,
    declaration: &Statement<'_>,
    branch: &Statement<'_>,
    observer: Symbol,
) -> Option<Symbol> {
    let variable = one_variable(declaration)?;
    if variable.init.is_some() {
        return None;
    }
    let next_alias = local(model, variable)?;
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    if identifier(model, typeof_test(&branch.test, "===", "function")?) != Some(observer) {
        return None;
    }
    let yes = block(&branch.consequent)?;
    let [yes] = yes else {
        return None;
    };
    let yes = assignment_value_to_identifier(model, yes, next_alias)?;
    if identifier(model, yes) != Some(observer) {
        return None;
    }
    let alternate = block(branch.alternate.as_ref()?)?;
    let [no] = alternate else {
        return None;
    };
    let no = assignment_value_to_identifier(model, no, next_alias)?;
    observer_next_fallback(model, no, observer)?;
    Some(next_alias)
}

/// The two-slot split lowering's object-form prelude: a predeclared slot filled
/// from one observer member in an `if (typeof observer === "object")` and
/// otherwise from that member's argument slot. The resolved member (`error` or
/// `complete`) and its fallback slot both prove the slot's role, so the role is
/// never assumed from statement position; it mirrors `observer_next_prelude`
/// and owns its own stable local.
fn observer_slot_prelude(
    model: &Model<'_, '_>,
    declaration: &Statement<'_>,
    branch: &Statement<'_>,
    observer: Symbol,
    member: &str,
    slot: Symbol,
) -> Option<Symbol> {
    let variable = one_variable(declaration)?;
    if variable.init.is_some() {
        return None;
    }
    let alias = local(model, variable)?;
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    if identifier(model, typeof_test(&branch.test, "===", "object")?) != Some(observer) {
        return None;
    }
    let yes = block(&branch.consequent)?;
    let [yes] = yes else {
        return None;
    };
    let yes = assignment_value_to_identifier(model, yes, alias)?;
    if !member_of(model, yes, observer, member) {
        return None;
    }
    let alternate = block(branch.alternate.as_ref()?)?;
    let [no] = alternate else {
        return None;
    };
    let no = assignment_value_to_identifier(model, no, alias)?;
    if identifier(model, no) != Some(slot) {
        return None;
    }
    Some(alias)
}

fn assignment_value_to_identifier<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    target: Symbol,
) -> Option<&'a Expression<'a>> {
    let expression = expression_statement(statement)?;
    let Expression::AssignmentExpression(assigned) = expression.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assigned.left else {
        return None;
    };
    (assigned.operator.as_str() == "="
        && crate::analysis::identifier_symbol(identifier, model.scoping) == Some(target))
    .then_some(&assigned.right)
}

fn assignment_value<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    handle: Symbol,
    key: &str,
) -> Option<&'a Expression<'a>> {
    let expression = expression_statement(statement)?;
    let Expression::AssignmentExpression(assigned) = expression.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(member) = &assigned.left else {
        return None;
    };
    (assigned.operator.as_str() == "="
        && !member.optional
        && member.property.name == key
        && identifier(model, &member.object) == Some(handle))
    .then_some(&assigned.right)
}

// start: typeof observer === "object" ? observer.start : void 0
fn observer_start(model: &Model<'_, '_>, e: &Expression<'_>, observer: Symbol) -> Option<()> {
    let (test, yes, no) = conditional(e)?;
    (identifier(model, typeof_test(test, "===", "object")?) == Some(observer)
        && member_of(model, yes, observer, "start")
        && is_void_zero(no))
    .then_some(())
}

// next: typeof observer === "function" ? observer : observer ? observer.next : void 0
fn observer_next(model: &Model<'_, '_>, e: &Expression<'_>, observer: Symbol) -> Option<()> {
    let (test, yes, no) = conditional(e)?;
    if identifier(model, typeof_test(test, "===", "function")?) != Some(observer)
        || identifier(model, yes) != Some(observer)
    {
        return None;
    }
    let (fallback, member, empty) = conditional(no)?;
    (identifier(model, fallback) == Some(observer)
        && member_of(model, member, observer, "next")
        && is_void_zero(empty))
    .then_some(())
}

// split subscribe prelude fallback: observer ? observer.next : void 0
fn observer_next_fallback(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    observer: Symbol,
) -> Option<()> {
    let (test, next, empty) = conditional(e)?;
    (identifier(model, test) == Some(observer)
        && member_of(model, next, observer, "next")
        && is_void_zero(empty))
    .then_some(())
}

// error: typeof observer === "object" ? observer.error : slot 1
fn observer_error(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    observer: Symbol,
    slot: Symbol,
) -> Option<()> {
    let (test, yes, no) = conditional(e)?;
    (identifier(model, typeof_test(test, "===", "object")?) == Some(observer)
        && member_of(model, yes, observer, "error")
        && identifier(model, no) == Some(slot))
    .then_some(())
}

// complete: typeof observer === "object" ? observer.complete : slot 2
fn observer_complete(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    observer: Symbol,
    slot: Symbol,
) -> Option<()> {
    let (test, yes, no) = conditional(e)?;
    (identifier(model, typeof_test(test, "===", "object")?) == Some(observer)
        && member_of(model, yes, observer, "complete")
        && identifier(model, no) == Some(slot))
    .then_some(())
}

/// The normalized observer handle: either the declaration `let handle = …;`
/// or the write `handle = …;` into an already declared slot. The boolean marks
/// the declaration form.
fn normalization<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
) -> Option<(Symbol, &'a Expression<'a>, bool)> {
    if let Some(variable) = one_variable(statement) {
        let handle = local(model, variable)?;
        return Some((handle, variable.init.as_ref()?, true));
    }
    let expression = expression_statement(statement)?;
    let Expression::AssignmentExpression(assigned) = expression.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assigned.left else {
        return None;
    };
    if assigned.operator.as_str() != "=" {
        return None;
    }
    let handle = crate::analysis::identifier_symbol(target, model.scoping)?;
    Some((handle, &assigned.right, false))
}

/// The two placeholder-write lowerings that fill a partly resolved normalized
/// observer record. `Triple` leaves `next`, `error` and `complete` as `void 0`
/// and writes all three same-handle; `ErrorComplete` resolves `next` inline in
/// the literal and writes only `error` and `complete`.
enum ObserverWrites<'a> {
    Triple(&'a Statement<'a>, &'a Statement<'a>, &'a Statement<'a>),
    ErrorComplete(&'a Statement<'a>, &'a Statement<'a>),
}

/// The one-slot lowering's single object-form prelude: exactly one observer
/// member is resolved into a predeclared slot, so the member name and its
/// fallback slot prove the slot's role. Returns the role alias plus the slot the
/// resolved literal must be assigned back into.
fn single_slot_aliases(
    model: &Model<'_, '_>,
    declaration: &Statement<'_>,
    branch: &Statement<'_>,
    observer: Symbol,
    error_slot: Symbol,
    complete_slot: Symbol,
) -> Option<(Option<Symbol>, Option<Symbol>, Symbol)> {
    if let Some(alias) =
        observer_slot_prelude(model, declaration, branch, observer, "error", error_slot)
    {
        return Some((Some(alias), None, alias));
    }
    let alias = observer_slot_prelude(
        model,
        declaration,
        branch,
        observer,
        "complete",
        complete_slot,
    )?;
    Some((None, Some(alias), alias))
}

/// Ordered lowering of the observer record. Every prelude is a complete
/// same-body branch, every placeholder is filled exactly once before use.
fn extended_normalization<'a>(
    model: &Model<'_, '_>,
    function: &'a Function<'a>,
    observer: Symbol,
    error_slot: Symbol,
    complete_slot: Symbol,
) -> Option<(Symbol, &'a Statement<'a>)> {
    let statements = body_of(function)?.statements.as_slice();
    let (branch, mut prefix) = statements.split_last()?;
    let mut aliases = [None; 3];
    let mut writes_per_alias = [2; 3];
    while let [declaration, branch, rest @ ..] = prefix {
        if !one_variable(declaration).is_some_and(|v| v.init.is_none()) {
            break;
        }
        let mut found = None;
        for role in 0..3 {
            let alias = match role {
                0 => observer_next_prelude(model, declaration, branch, observer)
                    .or_else(|| expanded_next_prelude(model, declaration, branch, observer)),
                1 => {
                    observer_slot_prelude(model, declaration, branch, observer, "error", error_slot)
                }
                _ => observer_slot_prelude(
                    model,
                    declaration,
                    branch,
                    observer,
                    "complete",
                    complete_slot,
                ),
            };
            if let Some(alias) = alias {
                found = Some((role, alias));
                break;
            }
        }
        let (role, alias) = found?;
        if role == 0 && expanded_next_prelude(model, declaration, branch, observer).is_some() {
            writes_per_alias[role] = 3;
        }
        if aliases[role].replace(alias).is_some() {
            return None;
        }
        prefix = rest;
    }
    let (normalize, writes) = prefix.split_first()?;
    let (handle, value, declared) = normalization(model, normalize)?;
    if !declared && !aliases.contains(&Some(handle)) {
        return None;
    }
    for (role, alias) in aliases
        .iter()
        .enumerate()
        .filter_map(|(role, alias)| alias.map(|alias| (role, alias)))
    {
        // Exactly the proved branch writes and, only for the reused slot,
        // the record replacement. Nested/dead/late writes are refused.
        if model.binding_writes(alias)
            != writes_per_alias[role] + usize::from(!declared && alias == handle)
        {
            return None;
        }
    }
    let Expression::ObjectExpression(object) = value.without_parentheses() else {
        return None;
    };
    let [start, next, error, complete] = object.properties.as_slice() else {
        return None;
    };
    observer_start(model, object_value(start, "start")?, observer)?;
    let mut writes = writes.iter();
    for (role, property, key) in [
        (0, next, "next"),
        (1, error, "error"),
        (2, complete, "complete"),
    ] {
        let mut value = object_value(property, key)?;
        if is_void_zero(value) {
            value = assignment_value(model, writes.next()?, handle, key)?;
        }
        if let Some(alias) = aliases[role] {
            if identifier(model, value) != Some(alias) {
                return None;
            }
        } else {
            match role {
                0 => observer_next(model, value, observer)?,
                1 => observer_error(model, value, observer, error_slot)?,
                _ => observer_complete(model, value, observer, complete_slot)?,
            }
        }
    }
    if writes.next().is_some() {
        return None;
    }
    Some((handle, branch))
}

fn expanded_next_prelude(
    model: &Model<'_, '_>,
    declaration: &Statement<'_>,
    statement: &Statement<'_>,
    observer: Symbol,
) -> Option<Symbol> {
    let variable = one_variable(declaration)?;
    if variable.init.is_some() {
        return None;
    }
    let alias = local(model, variable)?;
    let Statement::IfStatement(branch) = statement else {
        return None;
    };
    if identifier(model, typeof_test(&branch.test, "===", "function")?) != Some(observer) {
        return None;
    }
    let [yes] = block(&branch.consequent)? else {
        return None;
    };
    if identifier(model, assignment_value_to_identifier(model, yes, alias)?) != Some(observer) {
        return None;
    }
    let Statement::IfStatement(fallback) = branch.alternate.as_ref()? else {
        return None;
    };
    if identifier(model, &fallback.test) != Some(observer) {
        return None;
    }
    let [yes] = block(&fallback.consequent)? else {
        return None;
    };
    let [no] = block(fallback.alternate.as_ref()?)? else {
        return None;
    };
    if !member_of(
        model,
        assignment_value_to_identifier(model, yes, alias)?,
        observer,
        "next",
    ) || !is_void_zero(assignment_value_to_identifier(model, no, alias)?)
    {
        return None;
    }
    Some(alias)
}

fn legacy_normalization<'a>(
    model: &Model<'_, '_>,
    function: &'a Function<'a>,
    observer: Symbol,
    next_slot: Symbol,
    complete_slot: Symbol,
) -> Option<(Symbol, &'a Statement<'a>)> {
    let (normalize, writes, branch, next_alias, error_alias, complete_alias, resolved_slot) =
        match body_of(function)?.statements.as_slice() {
            [normalize, branch] => (normalize, None, branch, None, None, None, None),
            // A single object-form prelude resolves exactly one observer member
            // into a predeclared slot and the fully resolved literal is then
            // assigned back into that same slot. The guard keeps this lowering
            // distinct from the placeholder-write arm below.
            [slot_declaration, slot_branch, normalize, branch]
                if one_variable(slot_declaration)
                    .is_some_and(|variable| variable.init.is_none()) =>
            {
                let (error_alias, complete_alias, resolved) = single_slot_aliases(
                    model,
                    slot_declaration,
                    slot_branch,
                    observer,
                    next_slot,
                    complete_slot,
                )?;
                (
                    normalize,
                    None,
                    branch,
                    None,
                    error_alias,
                    complete_alias,
                    Some(resolved),
                )
            }
            [normalize, error, complete, branch] => (
                normalize,
                Some(ObserverWrites::ErrorComplete(error, complete)),
                branch,
                None,
                None,
                None,
                None,
            ),
            [normalize, next, error, complete, branch] => (
                normalize,
                Some(ObserverWrites::Triple(next, error, complete)),
                branch,
                None,
                None,
                None,
                None,
            ),
            [next_declaration, next_branch, normalize, next, error, complete, branch] => {
                let next_alias =
                    observer_next_prelude(model, next_declaration, next_branch, observer)?;
                (
                    normalize,
                    Some(ObserverWrites::Triple(next, error, complete)),
                    branch,
                    Some(next_alias),
                    None,
                    None,
                    Some(next_alias),
                )
            }
            [next_declaration, next_branch, slot_declaration, slot_branch, normalize, branch] => {
                let next_alias =
                    observer_next_prelude(model, next_declaration, next_branch, observer)?;
                // The object-form prelude resolves exactly one observer member.
                // Its member name and fallback slot prove the role: `error`
                // falls back to the error argument and `complete` to the
                // complete argument; neither is inferred from statement order.
                let (error_alias, complete_alias) = match observer_slot_prelude(
                    model,
                    slot_declaration,
                    slot_branch,
                    observer,
                    "error",
                    next_slot,
                ) {
                    Some(alias) => (Some(alias), None),
                    None => (
                        None,
                        Some(observer_slot_prelude(
                            model,
                            slot_declaration,
                            slot_branch,
                            observer,
                            "complete",
                            complete_slot,
                        )?),
                    ),
                };
                (
                    normalize,
                    None,
                    branch,
                    Some(next_alias),
                    error_alias,
                    complete_alias,
                    Some(next_alias),
                )
            }
            _ => return None,
        };
    let (handle, normalized, declared) = normalization(model, normalize)?;
    // The two-slot form assigns the resolved literal back into the next prelude
    // slot itself and the single-slot form back into its resolved slot; the
    // other forms declare a fresh handle. The expected slot comes from the
    // prelude role proof, never from statement position.
    if !declared && resolved_slot != Some(handle) {
        return None;
    }
    match writes {
        None => {
            observer_object(
                model,
                normalized,
                observer,
                next_alias,
                error_alias,
                next_slot,
                complete_alias,
                complete_slot,
            )?;
        }
        Some(ObserverWrites::Triple(next, error, complete)) => {
            observer_placeholder(model, normalized, observer)?;
            observer_writes(
                model,
                next,
                error,
                complete,
                handle,
                observer,
                next_alias,
                next_slot,
                complete_slot,
            )?;
        }
        Some(ObserverWrites::ErrorComplete(error, complete)) => {
            observer_partial_literal(model, normalized, observer)?;
            observer_error(
                model,
                assignment_value(model, error, handle, "error")?,
                observer,
                next_slot,
            )?;
            observer_complete(
                model,
                assignment_value(model, complete, handle, "complete")?,
                observer,
                complete_slot,
            )?;
        }
    }
    Some((handle, branch))
}

fn subscribe_method(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    class: Symbol,
    alias: Symbol,
    extended: bool,
) -> Option<()> {
    let params = plain_params(model, function)?;
    let [observer, next_slot, complete_slot] = params.as_slice() else {
        return None;
    };
    let (handle, branch) = legacy_normalization(
        model,
        function,
        *observer,
        *next_slot,
        *complete_slot,
    )
    .or_else(|| {
        extended
            .then(|| extended_normalization(model, function, *observer, *next_slot, *complete_slot))
            .flatten()
    })?;
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let (left, right) = binary(&branch.test, "==")?;
    if !this_member(left, "_subscriber") || !is_null(right) {
        return None;
    }
    let rejected = block(&branch.consequent)?;
    let [throw] = rejected else {
        return None;
    };
    let new = new_expression(throw_expression(throw)?)?;
    if !unbound(model, &new.callee, "TypeError") || !exact_new_arguments(new, 0)?.is_empty() {
        return None;
    }
    let accepted = block(branch.alternate.as_ref()?)?;
    let [observer_new, start_branch, wrapper_new, cleanup_try, release_branch, finish] = accepted
    else {
        return None;
    };
    // new ObserverCtor(normalized)
    let observer_new = one_variable(observer_new)?;
    let subscription = local(model, observer_new)?;
    let new = new_expression(observer_new.init.as_ref()?)?;
    let observer_ctor = captured_callable(model, site, &new.callee)?;
    if identifier(model, exact_new_arguments(new, 1)?[0]) != Some(handle) {
        return None;
    }
    // if (typeof normalized.start === "function") normalized.start(subscription);
    let Statement::IfStatement(start) = start_branch else {
        return None;
    };
    if start.alternate.is_some()
        || !member_of(
            model,
            typeof_test(&start.test, "===", "function")?,
            handle,
            "start",
        )
    {
        return None;
    }
    let started = block(&start.consequent)?;
    match started {
        [start_call] => {
            let start_call = method_call(expression_statement(start_call)?, "start")?;
            if !receiver_of(&start_call.callee, "start")
                .is_some_and(|receiver| identifier(model, receiver) == Some(handle))
                || identifier(model, exact_arguments(start_call, 1)?[0]) != Some(subscription)
            {
                return None;
            }
        }
        [bundle, call] => {
            let (receiver, method, arguments) = bundled_call(model, bundle, call)?;
            if receiver != handle
                || method != "start"
                || arguments.len() != 1
                || identifier(model, arguments[0]) != Some(subscription)
            {
                return None;
            }
        }
        _ => return None,
    }
    // new WrapperCtor(subscription)
    let wrapper_new = one_variable(wrapper_new)?;
    let wrapper = local(model, wrapper_new)?;
    let new = new_expression(wrapper_new.init.as_ref()?)?;
    let wrapper_ctor = captured_callable(model, site, &new.callee)?;
    if identifier(model, exact_new_arguments(new, 1)?[0]) != Some(subscription) {
        return None;
    }
    // try { subscription._cleanup = Teardown(this._subscriber, wrapper); }
    // catch (e) { wrapper.error(e); }
    let Statement::TryStatement(cleanup_try) = cleanup_try else {
        return None;
    };
    if cleanup_try.finalizer.is_some() {
        return None;
    }
    let [cleanup] = cleanup_try.block.body.as_slice() else {
        return None;
    };
    let cleanup = expression_statement(cleanup)?;
    let Expression::AssignmentExpression(cleanup) = cleanup.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(target) = &cleanup.left else {
        return None;
    };
    if cleanup.operator.as_str() != "="
        || target.optional
        || target.property.name != "_cleanup"
        || identifier(model, &target.object) != Some(subscription)
    {
        return None;
    }
    let teardown_call = plain_call(&cleanup.right)?;
    let teardown = captured_callable(model, site, &teardown_call.callee)?;
    let args = exact_arguments(teardown_call, 2)?;
    let [feed, wrapped] = args.as_slice() else {
        return None;
    };
    if !this_member(feed, "_subscriber") || identifier(model, wrapped) != Some(wrapper) {
        return None;
    }
    let handler = cleanup_try.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    match handler.body.body.as_slice() {
        [report] => {
            let args = observer_call(model, expression_statement(report)?, wrapper, "error", 1)?;
            if identifier(model, args[0]) != Some(error) {
                return None;
            }
        }
        [bundle, call] => {
            let (receiver, method, arguments) = bundled_call(model, bundle, call)?;
            if receiver != wrapper
                || method != "error"
                || arguments.len() != 1
                || identifier(model, arguments[0]) != Some(error)
            {
                return None;
            }
        }
        _ => return None,
    }
    // if (subscription._observer === void 0) { ... }
    let Statement::IfStatement(release) = release_branch else {
        return None;
    };
    if release.alternate.is_some() {
        return None;
    }
    let (left, right) = binary(&release.test, "===")?;
    if !member_of(model, left, subscription, "_observer") || !is_void_zero(right) {
        return None;
    }
    let released = block(&release.consequent)?;
    let [cleanup_value, inner] = released else {
        return None;
    };
    let cleanup_variable = one_variable(cleanup_value)?;
    let cleanup_symbol = local(model, cleanup_variable)?;
    if !member_of(
        model,
        cleanup_variable.init.as_ref()?,
        subscription,
        "_cleanup",
    ) || !immutable(model, cleanup_symbol)
    {
        return None;
    }
    let Statement::IfStatement(inner) = inner else {
        return None;
    };
    if inner.alternate.is_some() {
        return None;
    }
    let (left, right) = binary(&inner.test, "!==")?;
    if identifier(model, left) != Some(cleanup_symbol) || !is_void_zero(right) {
        return None;
    }
    let body = block(&inner.consequent)?;
    let [clear, run] = body else {
        return None;
    };
    let clear = expression_statement(clear)?;
    let Expression::AssignmentExpression(clear) = clear.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(clear_target) = &clear.left else {
        return None;
    };
    if clear.operator.as_str() != "="
        || clear_target.optional
        || clear_target.property.name != "_cleanup"
        || identifier(model, &clear_target.object) != Some(subscription)
        || !is_void_zero(&clear.right)
    {
        return None;
    }
    let Statement::TryStatement(run) = run else {
        return None;
    };
    if run.finalizer.is_some() {
        return None;
    }
    let [run_call] = run.block.body.as_slice() else {
        return None;
    };
    let run_call = plain_call(expression_statement(run_call)?)?;
    if identifier(model, &run_call.callee) != Some(cleanup_symbol)
        || !exact_arguments(run_call, 0)?.is_empty()
    {
        return None;
    }
    let run_handler = run.handler.as_ref()?;
    if !run_handler.body.body.is_empty() {
        return None;
    }
    // return subscription;
    if identifier(model, return_value(finish)?) != Some(subscription) {
        return None;
    }
    if !distinct(&[observer_ctor, wrapper_ctor, teardown, subscription])
        || [observer_ctor, wrapper_ctor, teardown]
            .iter()
            .any(|identity| *identity == class || *identity == alias)
    {
        return None;
    }
    Some(())
}

fn pipe_method(model: &Model<'_, '_>, function: &Function<'_>) -> Option<()> {
    if !plain_params(model, function)?.is_empty() {
        return None;
    }
    let body = body_of(function)?;
    let statements = body.statements.as_slice();
    let (_, operators, _, consumed) = copy_arguments(model, statements)?;
    let [finish] = statements.get(consumed..)? else {
        return None;
    };
    let reduce = method_call(return_value(finish)?, "reduce")?;
    if !receiver_of(&reduce.callee, "reduce")
        .is_some_and(|receiver| identifier(model, receiver) == Some(operators))
    {
        return None;
    }
    let args = exact_arguments(reduce, 2)?;
    let [step, initial] = args.as_slice() else {
        return None;
    };
    if !matches!(initial.without_parentheses(), Expression::ThisExpression(_)) {
        return None;
    }
    let Expression::ArrowFunctionExpression(step) = step.without_parentheses() else {
        return None;
    };
    let args = plain_arrow_params(model, step)?;
    let [accumulated, operator] = args.as_slice() else {
        return None;
    };
    let apply = plain_call(arrow_value(step)?)?;
    if identifier(model, &apply.callee) != Some(*operator) {
        return None;
    }
    let args = exact_arguments(apply, 1)?;
    if identifier(model, args[0]) != Some(*accumulated) {
        return None;
    }
    Some(())
}

fn operator_method(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
) -> Option<()> {
    let params = plain_params(model, function)?;
    let [selector] = params.as_slice() else {
        return None;
    };
    let body = body_of(function)?;
    let [finish] = body.statements.as_slice() else {
        return None;
    };
    let pipe = method_call(return_value(finish)?, "pipe")?;
    if !receiver_of(&pipe.callee, "pipe").is_some_and(|receiver| {
        matches!(
            receiver.without_parentheses(),
            Expression::ThisExpression(_)
        )
    }) {
        return None;
    }
    let args = exact_arguments(pipe, 1)?;
    let [operator_call] = args.as_slice() else {
        return None;
    };
    let operator_call = plain_call(operator_call)?;
    captured_callable(model, site, &operator_call.callee)?;
    let args = exact_arguments(operator_call, 1)?;
    let [argument] = args.as_slice() else {
        return None;
    };
    (identifier(model, argument) == Some(*selector)).then_some(())
}

struct Api<'a> {
    constructor: &'a Function<'a>,
    of: &'a Function<'a>,
    empty: &'a Function<'a>,
    from: &'a Function<'a>,
    subscribe: &'a Function<'a>,
    pipe: &'a Function<'a>,
    map: &'a Function<'a>,
    unordered_flat_map: &'a Function<'a>,
}

/// Consume the entire eager class-definition surface before using any method
/// body as API evidence. All supported classes have ordinary noncomputed
/// methods only: no decorators (including parameter decorators), superclass,
/// fields, accessors masquerading as methods, static blocks or default inputs.
/// Quoted noncomputed literal keys keep their existing property-key semantics.
fn closed_class_definition(class: &Class<'_>) -> Option<()> {
    if class.super_class.is_some() || !class.decorators.is_empty() {
        return None;
    }
    for element in &class.body.body {
        let ClassElement::MethodDefinition(method) = element else { return None; };
        if method.computed || !method.decorators.is_empty()
            || !matches!(method.kind, MethodDefinitionKind::Constructor | MethodDefinitionKind::Method)
            || method.value.params.items.iter().any(|p| p.initializer.is_some() || !p.decorators.is_empty())
        {
            return None;
        }
    }
    Some(())
}

fn class_api<'a>(class: &'a Class<'a>) -> Option<Api<'a>> {
    closed_class_definition(class)?;
    let mut constructor = None;
    let mut of = None;
    let mut empty = None;
    let mut from = None;
    let mut subscribe = None;
    let mut pipe = None;
    let mut map = None;
    let mut unordered_flat_map = None;
    let mut seen = BTreeSet::new();
    for element in &class.body.body {
        let ClassElement::MethodDefinition(method) = element else {
            return None;
        };
        if method.computed {
            return None;
        }
        let key = property_key_name(&method.key)?;
        if !seen.insert(key.clone()) {
            return None;
        }
        let value = &method.value;
        if value.body.is_none() || value.r#async || value.generator {
            return None;
        }
        match (key.as_str(), method.kind, method.r#static) {
            ("constructor", MethodDefinitionKind::Constructor, false) => {
                constructor = Some(value);
            }
            ("of", MethodDefinitionKind::Method, true) => of = Some(value),
            ("empty", MethodDefinitionKind::Method, true) => empty = Some(value),
            ("from", MethodDefinitionKind::Method, true) => from = Some(value),
            ("subscribe", MethodDefinitionKind::Method, false) => subscribe = Some(value),
            ("pipe", MethodDefinitionKind::Method, false) => pipe = Some(value),
            ("map", MethodDefinitionKind::Method, false) => map = Some(value),
            ("unorderedFlatMap", MethodDefinitionKind::Method, false) => {
                unordered_flat_map = Some(value);
            }
            _ => return None,
        }
    }
    Some(Api {
        constructor: constructor?,
        of: of?,
        empty: empty?,
        from: from?,
        subscribe: subscribe?,
        pipe: pipe?,
        map: map?,
        unordered_flat_map: unordered_flat_map?,
    })
}

/// Async callbacks change completion/error and scheduler return behavior. Keep
/// this guard on the new proof only, preserving the established evaluator path.
struct SynchronousCallbacks(bool);

impl<'a> Visit<'a> for SynchronousCallbacks {
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.0 &= !arrow.r#async;
        walk::walk_arrow_function_expression(self, arrow);
    }
}

fn owned_factory(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    extended: bool,
    additional_lowerings: bool,
) -> Option<(Span, Symbol)> {
    if !function.params.items.is_empty() || function.params.rest.is_some() {
        return None;
    }
    let body = body_of(function)?;
    let [Statement::ClassDeclaration(class), Statement::VariableDeclaration(alias_declaration), Statement::ReturnStatement(returned)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let class_symbol = class.id.as_ref()?.symbol_id.get()?;
    if class.super_class.is_some()
        || !site.span.contains_inclusive(class.span)
        || !site
            .span
            .contains_inclusive(model.scoping.symbol_span(class_symbol))
        || !immutable(model, class_symbol)
    {
        return None;
    }
    let [alias] = alias_declaration.declarations.as_slice() else {
        return None;
    };
    if !matches!(
        alias_declaration.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let alias_symbol = local(model, alias)?;
    if alias_symbol == class_symbol
        || !site
            .span
            .contains_inclusive(model.scoping.symbol_span(alias_symbol))
        || !immutable(model, alias_symbol)
        || identifier(model, alias.init.as_ref()?) != Some(class_symbol)
        || identifier(model, returned.argument.as_ref()?) != Some(alias_symbol)
    {
        return None;
    }
    if extended {
        let mut callbacks = SynchronousCallbacks(true);
        callbacks.visit_function_body(body);
        if !callbacks.0 {
            return None;
        }
    }
    let api = class_api(class)?;
    constructor_method(model, api.constructor)?;
    of_method(model, api.of, class_symbol, alias_symbol)?;
    empty_method(model, api.empty, class_symbol, alias_symbol)?;
    from_method(
        model,
        api.from,
        class_symbol,
        alias_symbol,
        extended,
        additional_lowerings,
    )?;
    subscribe_method(
        model,
        site,
        api.subscribe,
        class_symbol,
        alias_symbol,
        extended,
    )?;
    pipe_method(model, api.pipe)?;
    operator_method(model, site, api.map)?;
    operator_method(model, site, api.unordered_flat_map)?;
    Some((class.span, class_symbol))
}

// This family accepts only declaration/initializer origins. All factory
// exposures must follow each captured initializer and every alias link must
// itself read an already available, immutable origin. Hoisted functions need
// no initialization; class declarations do.
fn ordered_captured_origin(
    model: &Model<'_, '_>,
    s: Symbol,
    use_node: NodeId,
    depth: usize,
) -> bool {
    if depth == 0 || !immutable(model, s) {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let end = match model.nodes.kind(declaration) {
        AstKind::Function(_) => return true,
        AstKind::Class(c) => c.span.end,
        AstKind::VariableDeclarator(v) => {
            let Some(value) = v.init.as_ref() else {
                return false;
            };
            if let Expression::Identifier(id) = value.without_parentheses() {
                let Some(next) = crate::analysis::identifier_symbol(id, model.scoping) else {
                    return false;
                };
                let Some(reference) = id.reference_id.get() else {
                    return false;
                };
                if !ordered_captured_origin(
                    model,
                    next,
                    model.scoping.get_reference(reference).node_id(),
                    depth - 1,
                ) {
                    return false;
                }
            } else if !matches!(
                value.without_parentheses(),
                Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::ClassExpression(_)
            ) {
                return false;
            }
            v.span.end
        }
        _ => return false,
    };
    let mut body = model.nodes.parent_id(declaration);
    if matches!(model.nodes.kind(body), AstKind::VariableDeclaration(_)) {
        body = model.nodes.parent_id(body);
    }
    matches!(
        model.nodes.kind(body),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) && crate::naming::shared::callable_origin_precedes(model, body, end, use_node, None, 0, true)
}

struct CapturedOrigins<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    site: FunctionSite,
    uses: Vec<NodeId>,
    valid: bool,
}
impl<'a> Visit<'a> for CapturedOrigins<'_, '_, 'a> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let Some(s) = crate::analysis::identifier_symbol(id, self.model.scoping) else {
            return;
        };
        if captured(self.model, self.site, s) && callable_declaration(self.model, s, 8) {
            self.valid &= self
                .uses
                .iter()
                .all(|use_node| ordered_captured_origin(self.model, s, *use_node, 8));
        }
    }
}

/// An alternative placement of the same complete API: constructor/of/empty
/// stay in the class; each remaining member is declared and immediately
/// installed on that exact immutable class alias, before the sole return.
/// This consumes the entire factory in execution order, not a member bag.
fn installed_factory(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    additional_lowerings: bool,
) -> Option<(Span, Symbol)> {
    installed_factory_variant(model, site, function, additional_lowerings, false)
}

fn installed_factory_variant(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    additional_lowerings: bool,
    mixed: bool,
) -> Option<(Span, Symbol)> {
    if !plain_params(model, function)?.is_empty() {
        return None;
    }
    let body = body_of(function)?;
    let [Statement::ClassDeclaration(class), alias_statement, tail @ ..] =
        body.statements.as_slice()
    else {
        return None;
    };
    closed_class_definition(class)?;
    let class_symbol = class.id.as_ref()?.symbol_id.get()?;
    if !immutable(model, class_symbol) {
        return None;
    }
    let variable = one_variable(alias_statement)?;
    let alias = local(model, variable)?;
    if !immutable(model, alias) || identifier(model, variable.init.as_ref()?) != Some(class_symbol)
    {
        return None;
    }
    let mut constructor = None;
    let mut of = None;
    let mut empty = None;
    for element in &class.body.body {
        let ClassElement::MethodDefinition(method) = element else {
            return None;
        };
        if method.computed {
            return None;
        }
        let slot = match (
            property_key_name(&method.key)?.as_str(),
            method.kind,
            method.r#static,
        ) {
            ("constructor", MethodDefinitionKind::Constructor, false) => &mut constructor,
            ("of", MethodDefinitionKind::Method, true) => &mut of,
            ("empty", MethodDefinitionKind::Method, true) => &mut empty,
            _ => return None,
        };
        if slot.replace(&*method.value).is_some() {
            return None;
        }
    }
    constructor_method(model, constructor?)?;
    of_method(model, of?, class_symbol, alias)?;
    empty_method(model, empty?, class_symbol, alias)?;
    let mut operator_inputs = BTreeSet::new();
    let mut remaining = tail;
    for (key, prototype) in [
        ("from", false),
        ("subscribe", true),
        ("pipe", true),
        ("map", true),
        ("unorderedFlatMap", true),
    ] {
        let [Statement::FunctionDeclaration(member), install, rest @ ..] = remaining else {
            return None;
        };
        let member_symbol = member.id.as_ref()?.symbol_id.get()?;
        if !immutable(model, member_symbol) {
            return None;
        }
        let Expression::AssignmentExpression(assignment) =
            expression_statement(install)?.without_parentheses()
        else {
            return None;
        };
        let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
            return None;
        };
        if assignment.operator.as_str() != "="
            || target.optional
            || target.property.name != key
            || identifier(model, &assignment.right) != Some(member_symbol)
        {
            return None;
        }
        let receiver = if prototype {
            receiver_of(&target.object, "prototype")?
        } else {
            &target.object
        };
        if identifier(model, receiver) != Some(alias) {
            return None;
        }
        // No extra read/exposure can invoke this declaration before setup or
        // turn an unrelated nested/sibling function into installed evidence.
        let references: Vec<_> = model
            .scoping
            .get_resolved_references(member_symbol)
            .collect();
        if references.len() != 1
            || model.nodes.kind(references[0].node_id()).span() != assignment.right.span()
        {
            return None;
        }
        if mixed && prototype {
            let [directive] = member.body.as_ref()?.directives.as_slice() else {
                return None;
            };
            if directive.directive != "use strict" {
                return None;
            }
        }
        if mixed && matches!(key, "map" | "unorderedFlatMap") {
            let [finish] = body_of(member)?.statements.as_slice() else {
                return None;
            };
            let pipe = method_call(return_value(finish)?, "pipe")?;
            let call = plain_call(exact_arguments(pipe, 1)?[0])?;
            if !operator_inputs.insert(captured_callable(model, site, &call.callee)?) {
                return None;
            }
        }
        match key {
            "from" => from_method_variant(
                model,
                member,
                class_symbol,
                alias,
                true,
                additional_lowerings,
                mixed,
            )?,
            "subscribe" => subscribe_method(model, site, member, class_symbol, alias, true)?,
            "pipe" => pipe_method(model, member)?,
            _ => operator_method(model, site, member)?,
        }
        remaining = rest;
    }
    let [finish] = remaining else {
        return None;
    };
    if identifier(model, return_value(finish)?) != Some(alias) {
        return None;
    }
    let mut callbacks = SynchronousCallbacks(true);
    callbacks.visit_function_body(body);
    let factory_symbol = function.id.as_ref()?.symbol_id.get()?;
    if !immutable(model, factory_symbol) {
        return None;
    }
    let uses: Vec<_> = model
        .scoping
        .get_resolved_references(factory_symbol)
        .map(|r| r.node_id())
        .collect();
    if uses.is_empty() {
        return None;
    }
    let mut origins = CapturedOrigins {
        model,
        site,
        uses,
        valid: true,
    };
    origins.visit_function_body(body);
    (callbacks.0 && origins.valid).then_some((class.span, class_symbol))
}

/// Full native identity proof; naming and hygiene remain the caller's responsibility.
pub(in crate::naming) fn proven_canonical_native_class(
    model: &Model<'_, '_>,
    site: FunctionSite,
) -> Option<(Span, Symbol)> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    owned_factory(model, site, function, true, false)
        .or_else(|| installed_factory(model, site, function, false))
}

/// Complete constructor witness including the two additional whole-body
/// lowerings. Canonical map naming deliberately uses the narrower witness above.
pub(in crate::naming) fn proven_native_class(
    model: &Model<'_, '_>,
    site: FunctionSite,
) -> Option<(Span, Symbol)> {
    proven_canonical_native_class(model, site)
        .or_else(|| {
            let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
                return None;
            };
            owned_factory(model, site, function, true, true)
                .or_else(|| installed_factory(model, site, function, true))
        })
        .or_else(|| {
            let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
                return None;
            };
            installed_factory_variant(model, site, function, true, true)
        })
        .or_else(|| source_class::prove(model, site))
}

/// Deferred invocation evidence belongs only to the full proved API's bodies.
/// Parameter/decorator/key expressions are deliberately outside these spans.
/// The native witness closes all class definition surfaces and, for installed
/// APIs, consumes each direct declaration's sole exact installation on this
/// owned class alias. No arbitrary nested/sibling declaration contributes.
pub(in crate::naming) fn proven_deferred_api_bodies(
    model: &Model<'_, '_>, site: FunctionSite,
) -> Option<Vec<Span>> {
    let (class_span, class_symbol) = proven_native_class(model, site)?;
    let AstKind::Function(factory) = model.nodes.kind(site.node_id) else { return None; };
    let [Statement::ClassDeclaration(class), alias_statement, tail @ ..] = body_of(factory)?.statements.as_slice() else { return None; };
    if class.span != class_span || class.id.as_ref()?.symbol_id.get()? != class_symbol { return None; }
    closed_class_definition(class)?;
    let mut spans = Vec::new();
    for element in &class.body.body {
        let ClassElement::MethodDefinition(method) = element else { return None; };
        spans.push(body_of(&method.value)?.span);
    }
    // Inline and source-class witnesses end with the alias return. Installed
    // witnesses have exactly the five already proved declaration/install pairs.
    if tail.len() != 1 {
        let alias = local(model, one_variable(alias_statement)?)?;
        let mut remaining = tail;
        for (key, prototype) in [("from", false), ("subscribe", true), ("pipe", true), ("map", true), ("unorderedFlatMap", true)] {
            let [Statement::FunctionDeclaration(member), install, rest @ ..] = remaining else { return None; };
            let Expression::AssignmentExpression(a) = expression_statement(install)?.without_parentheses() else { return None; };
            let AssignmentTarget::StaticMemberExpression(target) = &a.left else { return None; };
            let receiver = if prototype { receiver_of(&target.object, "prototype")? } else { &target.object };
            if a.operator.as_str() != "=" || target.optional || target.property.name != key
                || identifier(model, receiver) != Some(alias)
                || identifier(model, &a.right) != member.id.as_ref()?.symbol_id.get()
            { return None; }
            spans.push(body_of(member)?.span);
            remaining = rest;
        }
        if remaining.len() != 1 { return None; }
    }
    Some(spans)
}

/// Recover a role only from the exact declaration installed by a completely
/// proved factory. The parent proof consumes every member body, installation,
/// return and captured origin; a standalone method or sibling donor cannot
/// supply any part of this witness. Operator names describe their installed
/// slots, not the behavior of the consumer-supplied captured callables.
fn installed_member_name(model: &Model<'_, '_>, site: FunctionSite) -> Option<&'static str> {
    let parent = model.enclosing_function(site.node_id)?;
    let AstKind::Function(factory) = model.nodes.kind(parent.node_id) else {
        return None;
    };
    installed_factory(model, parent, factory, false)
        .or_else(|| installed_factory(model, parent, factory, true))
        .or_else(|| installed_factory_variant(model, parent, factory, true, true))?;
    for pair in body_of(factory)?.statements.windows(2) {
        let Statement::FunctionDeclaration(member) = &pair[0] else {
            continue;
        };
        if member.span != site.span || member.id.as_ref()?.symbol_id.get() != site.binding_symbol {
            continue;
        }
        let Expression::AssignmentExpression(assignment) =
            expression_statement(&pair[1])?.without_parentheses()
        else {
            return None;
        };
        let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
            return None;
        };
        return match target.property.name.as_str() {
            "from" => Some("convertToObservable"),
            "subscribe" => Some("subscribeToObservable"),
            "pipe" => Some("pipeObservable"),
            "map" => Some("pipeObservableMapOperator"),
            "unorderedFlatMap" => Some("pipeObservableFlatMapOperator"),
            _ => None,
        };
    }
    None
}

/// An ordinary strict constructor is callable without `new`, unlike a native
/// class. Prove its complete installed API separately and never feed this
/// witness into `proven_native_class` or the native class claim resolver.
fn ordinary_factory(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
) -> Option<Vec<(Span, &'static str)>> {
    if !plain_params(model, function)?.is_empty() {
        return None;
    }
    let body = body_of(function)?;
    let [Statement::FunctionDeclaration(from), Statement::FunctionDeclaration(constructor), alias_statement,
        Statement::FunctionDeclaration(of), install_of,
        Statement::FunctionDeclaration(empty), install_empty, install_from,
        Statement::FunctionDeclaration(subscribe), install_subscribe,
        Statement::FunctionDeclaration(pipe), install_pipe,
        Statement::FunctionDeclaration(map), install_map,
        Statement::FunctionDeclaration(flat_map), install_flat_map, finish] = body.statements.as_slice()
    else {
        return None;
    };
    let constructor_symbol = constructor.id.as_ref()?.symbol_id.get()?;
    let alias_variable = one_variable(alias_statement)?;
    let alias = local(model, alias_variable)?;
    if !distinct(&[constructor_symbol, alias])
        || !immutable(model, constructor_symbol)
        || !immutable(model, alias)
        || identifier(model, alias_variable.init.as_ref()?) != Some(constructor_symbol)
        || identifier(model, return_value(finish)?) != Some(alias)
    {
        return None;
    }
    // Strict receiver semantics are part of this ordinary-constructor contract.
    for member in [constructor, subscribe, pipe, map, flat_map] {
        let [directive] = member.body.as_ref()?.directives.as_slice() else {
            return None;
        };
        if directive.directive != "use strict" {
            return None;
        }
    }
    constructor_method(model, constructor)?;
    of_method(model, of, constructor_symbol, alias)?;
    empty_method(model, empty, constructor_symbol, alias)?;
    from_method(model, from, constructor_symbol, alias, true, true)?;
    subscribe_method(model, site, subscribe, constructor_symbol, alias, true)?;
    pipe_method(model, pipe)?;
    operator_method(model, site, map)?;
    operator_method(model, site, flat_map)?;
    let operator_capture = |member: &Function<'_>| {
        let [finish] = body_of(member)?.statements.as_slice() else { return None; };
        let pipe = method_call(return_value(finish)?, "pipe")?;
        let call = plain_call(exact_arguments(pipe, 1)?[0])?;
        captured_callable(model, site, &call.callee)
    };
    if operator_capture(map)? == operator_capture(flat_map)? {
        return None;
    }

    let mut names = Vec::new();
    for (member, install, receiver, key, prototype, name) in [
        (&**of, install_of, constructor_symbol, "of", false, "createSubscriberStreamOf"),
        (&**empty, install_empty, alias, "empty", false, "createEmptySubscriberStream"),
        (&**from, install_from, alias, "from", false, "convertToSubscriberStream"),
        (&**subscribe, install_subscribe, alias, "subscribe", true, "subscribeToSubscriberStream"),
        (&**pipe, install_pipe, alias, "pipe", true, "pipeSubscriberStream"),
        (&**map, install_map, alias, "map", true, "pipeSubscriberStreamMapSlot"),
        (&**flat_map, install_flat_map, alias, "unorderedFlatMap", true, "pipeSubscriberStreamFlatMapSlot"),
    ] {
        let member_symbol = member.id.as_ref()?.symbol_id.get()?;
        if !immutable(model, member_symbol) {
            return None;
        }
        let Expression::AssignmentExpression(assignment) = expression_statement(install)?.without_parentheses() else {
            return None;
        };
        let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
            return None;
        };
        let object = if prototype { receiver_of(&target.object, "prototype")? } else { &target.object };
        if assignment.operator.as_str() != "=" || target.optional
            || target.property.name != key || identifier(model, object) != Some(receiver)
            || identifier(model, &assignment.right) != Some(member_symbol)
        {
            return None;
        }
        let references: Vec<_> = model.scoping.get_resolved_references(member_symbol).collect();
        if references.len() != 1 || model.nodes.kind(references[0].node_id()).span() != assignment.right.span() {
            return None;
        }
        names.push((member.span, name));
    }
    // The constructor itself has only its alias initializer and static `of`
    // installation as reads. No call or extra publication can precede setup.
    let constructor_reads: Vec<_> = model.scoping.get_resolved_references(constructor_symbol).collect();
    if constructor_reads.len() != 2 || constructor_reads.iter().any(|reference| {
        let span = model.nodes.kind(reference.node_id()).span();
        span != alias_variable.init.as_ref().unwrap().span() && !install_of.span().contains_inclusive(span)
    }) {
        return None;
    }
    let factory_symbol = function.id.as_ref()?.symbol_id.get()?;
    if !immutable(model, factory_symbol) {
        return None;
    }
    let uses: Vec<_> = model.scoping.get_resolved_references(factory_symbol).map(|r| r.node_id()).collect();
    if uses.is_empty() {
        return None;
    }
    let mut origins = CapturedOrigins { model, site, uses, valid: true };
    origins.visit_function_body(body);
    let mut callbacks = SynchronousCallbacks(true);
    callbacks.visit_function_body(body);
    if !origins.valid || !callbacks.0 {
        return None;
    }
    names.push((constructor.span, "initializeSubscriberStream"));
    Some(names)
}

/// Complete ordinary strict constructor API, deliberately separate from native class identity.
pub(in crate::naming) fn proven_ordinary_constructor(
    model: &Model<'_, '_>, site: FunctionSite,
) -> Option<()> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else { return None; };
    ordinary_factory(model, site, function).map(|_| ())
}

fn ordinary_name(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<&'static str> {
    if ordinary_factory(model, site, function).is_some() {
        return Some("createSubscriberStreamConstructor");
    }
    let parent = model.enclosing_function(site.node_id)?;
    let AstKind::Function(factory) = model.nodes.kind(parent.node_id) else { return None; };
    ordinary_factory(model, parent, factory)?.into_iter().find_map(|(span, name)| (span == site.span).then_some(name))
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
    if let Some(name) = installed_member_name(model, site) {
        return vec![proposal(
            name,
            "shared_fast_observable_installed_member",
            "complete own method protocol and exact declaration installed on the proved factory's class alias; whole factory, ordered installations, sole declaration reference and captured initialization at every factory exposure proved; captured operators retain consumer-defined identity",
            f,
        )];
    }
    if let Some(name) = ordinary_name(model, site, function) {
        return vec![proposal(
            name,
            "shared_observable_ordinary_constructor_api",
            "whole ordinary strict constructor factory with exact argument-copy emission, conversion, subscription, teardown and pipe protocols; ordered constructor/alias API installations and sole member publications; immutable SymbolId captures initialized at every factory exposure; map and flat-map names describe installed slots of distinct consumer-supplied operators; no native class identity claimed",
            f,
        )];
    }
    if owned_factory(model, site, function, false, false)
        .or_else(|| owned_factory(model, site, function, true, false))
        .or_else(|| installed_factory(model, site, function, false))
        .or_else(|| proven_native_class(model, site))
        .is_none()
    {
        return Vec::new();
    }
    vec![proposal(
        "createObservableClass",
        "shared_fast_observable_factory",
        "whole factory: one owned class declaration returned through one immutable single-origin alias; constructor rejects a non-function and stores _subscriber; static of/empty/from implement the argument-copy, emit-then-complete, complete-only, instance/promise/array-like/iterator protocols with scheduled and unscheduled loops and the conversion failure; subscribe normalizes the observer, builds observer/subscription handles, calls start, stores the captured teardown in _cleanup, forwards teardown errors and runs an unconsumed cleanup; pipe reduces the copied operators over this; map/unorderedFlatMap forward through pipe with captured independent operator functions",
        f,
    )]
}

#[cfg(test)]
#[path = "observable_factory_subscription_operator_tests.rs"]
mod subscription_operator_tests;

#[cfg(test)]
#[path = "observable_factory_layout_tests.rs"]
mod observable_factory_layout_tests;

#[path = "observable_factory_source_class.rs"]
mod source_class;

#[path = "observable_factory_mixed_from.rs"]
mod mixed_from;

#[cfg(test)]
#[path = "observable_factory_setup_order_tests.rs"]
mod setup_order_tests;
