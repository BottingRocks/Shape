//! Bounded, owned-body proofs for the `hashStringOrBuffer` loop-node variants
//! that the older anchor classifier declines.
//!
//! The logical family is unchanged: a `typeof input === "string"` branch folds
//! UTF-16 code units in Java-style `h * 31` pairs (with a one-unit odd tail),
//! and otherwise a captured `ArrayBuffer.isView` feature flag gates a 4-byte
//! aligned `Int32Array` word fold with the same `h * 31` update.  This module
//! only widens the *source-proven* node shapes:
//!
//! * `for` and `while` string loops, with `i = i + 2` or `i += 2`,
//! * `for` and `while` word loops, with `++i`, `i++`, `i += 1` or `i = i + 1`,
//! * a positive `if (flag) { words } else { throw new TypeError() }` dispatch,
//!   or a negated `if (!flag) { throw new TypeError() }` guard followed by the
//!   word body,
//! * an accumulator coercion split into `h = h | 0` after the fold update,
//! * `let words = new Int32Array(buffer)` as well as `buffer = new
//!   Int32Array(buffer)`,
//! * plain immutable aliases of the ambient `ArrayBuffer`, `Int8Array` and
//!   `Int32Array` namespaces (`let ns = ArrayBuffer; ns.isView(...)`),
//! * binding the word loop's index and accumulator by their use - the index is
//!   the local compared with `words.length` and stepped, the accumulator is the
//!   folded and returned local - instead of by declaration order,
//! * a captured unit reader whose immutable single-assignment alias resolves to
//!   a function/arrow returning `formal.charCodeAt()` even when that value is
//!   declared after the candidate: a deferred forward invocation whose value
//!   lies lexically outside the candidate body, so the binding is initialised
//!   before the candidate can be called,
//! * the split memo fold `h = [h << 5, h]; h = h[0] - h[1] + operand | 0`, with
//!   the two unit reads optionally hoisted into `let u1`/`let u2` locals and the
//!   odd tail allowed to reuse the even bound local for its one leftover unit.
//!
//! Every identifier comparison uses the existing scope index.  The captured
//! unit reader is accepted only when its own bounded body returns
//! `formal.charCodeAt()` (the same contract the older classifier proves); no
//! native callee, generated name, vendor, body hash, span or inherited label
//! participates.  The older classifiers are not modified: this rule runs only
//! after every earlier generation has declined the function.
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;
use std::collections::{BTreeMap, BTreeSet};

type Symbol = SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn number(e: &Expression<'_>, expected: f64) -> bool {
    matches!(e, Expression::NumericLiteral(n) if n.value == expected)
}

fn is_symbol(model: &Model<'_, '_>, e: &Expression<'_>, s: Symbol) -> bool {
    symbol(model, e) == Some(s)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn call_expr<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
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

fn plain_declarator(model: &Model<'_, '_>, v: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (model.scoping.symbol_redeclarations(s).is_empty()
        && matches!(
            model.nodes.parent_kind(model.scoping.symbol_declaration(s)),
            AstKind::VariableDeclaration(d)
                if matches!(d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const)
        ))
    .then_some(s)
}

// A captured DATA slot may be read and written, but it must be a plain
// declarator in an ancestor statement container: parameters, catches,
// destructuring and loop bindings never establish an independent identity.
fn captured_data_symbol(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> bool {
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(s);
    if !matches!(
        model.nodes.kind(declaration),
        AstKind::VariableDeclarator(_)
    ) || !matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(_)
    ) || !matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(d)
            if matches!(d.kind,
                VariableDeclarationKind::Var
                    | VariableDeclarationKind::Let
                    | VariableDeclarationKind::Const)
    ) || matches!(
        model.nodes.parent_kind(model.nodes.parent_id(declaration)),
        AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
    ) {
        return false;
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return false;
    };
    matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
}

fn data_capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = symbol(model, e)?;
    captured_data_symbol(model, site, s).then_some(s)
}

// One ordered callable origin: a function declaration or a plain variable with
// exactly one initializer/assignment.  Parameters, catches, destructuring and
// loop bindings can carry runtime values but never establish a callable.
//
// `forward` carries the candidate function whose body is being proved.  A
// binding declared before the use is accepted as before; a forward reference is
// admitted only when the value lies lexically outside that candidate, so the
// read is a deferred invocation (the binding is initialised before the
// candidate can run) rather than an immediate initializer read while the
// binding is still uninitialised.  Uniqueness, no-redeclaration, the
// single-write invariant and the function/arrow value shape are unchanged.
fn ordered_function(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    use_start: u32,
    forward: Option<FunctionSite>,
    depth: usize,
) -> Option<FunctionSite> {
    if depth > 16 {
        return None;
    }
    if let Expression::Identifier(i) = e {
        let s = crate::analysis::identifier_symbol(i, model.scoping)?;
        if !model.scoping.symbol_redeclarations(s).is_empty() {
            return None;
        }
        let declaration = model.scoping.symbol_declaration(s);
        let writes = model.binding_writes(s);
        if let AstKind::Function(f) = model.nodes.kind(declaration) {
            return (writes == 0).then(|| model.site_by_span(f.span)).flatten();
        }
        let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
            return None;
        };
        if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
            || !matches!(
                model.nodes.parent_kind(declaration),
                AstKind::VariableDeclaration(d) if matches!(d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const)
            )
            || matches!(
                model.nodes.parent_kind(model.nodes.parent_id(declaration)),
                AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
            )
        {
            return None;
        }
        let value = match (writes, v.init.as_ref()) {
            (0, Some(init)) => init,
            (1, None) => model.symbol_value(s)?,
            _ => return None,
        };
        if value.span().end > use_start
            && !forward.is_some_and(|site| value.span().start >= site.span.end)
        {
            return None;
        }
        return ordered_function(model, value, value.span().start, forward, depth + 1);
    }
    if let Expression::ParenthesizedExpression(parenthesized) = e {
        return ordered_function(
            model,
            &parenthesized.expression,
            use_start,
            forward,
            depth + 1,
        );
    }
    model.function_from_expression(e)
}

fn call_resolves_to(
    model: &Model<'_, '_>,
    c: &CallExpression<'_>,
    target: FunctionSite,
    forward: Option<FunctionSite>,
) -> bool {
    !c.optional
        && ordered_function(model, &c.callee, c.callee.span().start, forward, 0)
            .is_some_and(|site| site.span == target.span)
}

// The captured unit reader's consumer contract: its own bounded body returns
// exactly `formal.charCodeAt()`.  This is the same source proof the older
// classifier uses, re-established here for the variant rule.
fn char_code_accessor(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    forward: Option<FunctionSite>,
) -> Option<FunctionSite> {
    let target = ordered_function(model, e, e.span().start, forward, 0)?;
    let (params, body, expression) = match model.nodes.kind(target.node_id) {
        AstKind::Function(function) => {
            if function.r#async || function.generator {
                return None;
            }
            (&function.params, function.body.as_ref()?, false)
        }
        AstKind::ArrowFunctionExpression(arrow) => {
            if arrow.r#async {
                return None;
            }
            (&arrow.params, &arrow.body, arrow.expression)
        }
        _ => return None,
    };
    if params.rest.is_some() || params.items.len() != 1 || !body.directives.is_empty() {
        return None;
    }
    let parameter = &params.items[0];
    if parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
        return None;
    };
    let formal = identifier.symbol_id.get()?;
    if model.binding_writes(formal) != 0 || !model.scoping.symbol_redeclarations(formal).is_empty()
    {
        return None;
    }
    let call = if expression {
        let [Statement::ExpressionStatement(statement)] = body.statements.as_slice() else {
            return None;
        };
        let Expression::CallExpression(call) = &statement.expression else {
            return None;
        };
        call
    } else {
        let [Statement::ReturnStatement(result)] = body.statements.as_slice() else {
            return None;
        };
        let Expression::CallExpression(call) = result.argument.as_ref()? else {
            return None;
        };
        call
    };
    let object = member(&call.callee, "charCodeAt")?;
    (!call.optional && call.arguments.is_empty() && is_symbol(model, object, formal))
        .then_some(target)
}

fn char_index<'a>(
    model: &Model<'_, '_>,
    c: &'a CallExpression<'a>,
    accessor: FunctionSite,
    forward: Option<FunctionSite>,
) -> Option<(Symbol, &'a Expression<'a>)> {
    if !call_resolves_to(model, c, accessor, forward) {
        return None;
    }
    let arguments = call_arguments(c);
    let [argument] = arguments.as_slice() else {
        return None;
    };
    let Expression::ComputedMemberExpression(read) = argument else {
        return None;
    };
    if read.optional {
        return None;
    }
    Some((symbol(model, &read.object)?, &read.expression))
}

fn path_is(model: &Model<'_, '_>, e: &Expression<'_>, expected: &str) -> bool {
    let path = expression_path(model, e);
    matches!(path.as_slice(), [only] if only == expected)
}

// A plain immutable alias of an ambient constructor namespace is source-proven
// by following one bounded origin chain to the unresolved global name.  No
// callee is named by fiat: if the source does not literally bind it to the
// ambient name, the variant is refused.
fn namespace_root(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    expected: &str,
    use_start: u32,
    depth: usize,
) -> bool {
    if depth > 8 {
        return false;
    }
    if path_is(model, e, expected) {
        return true;
    }
    let Expression::Identifier(id) = e else {
        return false;
    };
    let Some(s) = crate::analysis::identifier_symbol(id, model.scoping) else {
        return false;
    };
    if !model.scoping.symbol_redeclarations(s).is_empty() || model.binding_writes(s) != 0 {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return false;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d) if matches!(d.kind,
                VariableDeclarationKind::Var
                    | VariableDeclarationKind::Let
                    | VariableDeclarationKind::Const)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return false;
    }
    let Some(init) = v.init.as_ref() else {
        return false;
    };
    if init.span().end > use_start {
        return false;
    }
    namespace_root(model, init, expected, use_start, depth + 1)
}

fn new_arguments<'a>(n: &'a NewExpression<'a>) -> Vec<&'a Expression<'a>> {
    n.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

fn expression_statement<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}

// `h[i]` with the exact numeric index `expected`.
fn memo_index(model: &Model<'_, '_>, e: &Expression<'_>, hash: Symbol, expected: f64) -> bool {
    let Expression::ComputedMemberExpression(member) = e else {
        return false;
    };
    !member.optional
        && is_symbol(model, &member.object, hash)
        && number(&member.expression, expected)
}

// A pure two-formal IIFE view of the same 31-fold core.  The function value is
// resolved directly from the call callee; this is deliberately not a general
// expression inliner.  Both formals are immutable plain identifiers and the
// body is exactly one return statement (or an expression-bodied arrow).
fn iife_fold_core(model: &Model<'_, '_>, e: &Expression<'_>, hash: Symbol) -> Option<bool> {
    let call = call_expr(e.without_parentheses())?;
    if call.arguments.len() != 2 {
        return None;
    }
    let arguments = call_arguments(call);
    let [first_argument, second_argument] = arguments.as_slice() else {
        return None;
    };
    if !is_symbol(model, first_argument, hash) || !is_symbol(model, second_argument, hash) {
        return None;
    }
    let callee = call.callee.without_parentheses();
    if !matches!(
        callee,
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
    ) {
        return None;
    }
    let target = model.function_from_expression(callee)?;
    let (params, body, expression) = match model.nodes.kind(target.node_id) {
        AstKind::Function(function) => {
            if function.r#async || function.generator {
                return None;
            }
            (&function.params, function.body.as_ref()?, false)
        }
        AstKind::ArrowFunctionExpression(arrow) => {
            if arrow.r#async {
                return None;
            }
            (&arrow.params, &arrow.body, arrow.expression)
        }
        _ => return None,
    };
    if params.rest.is_some() || params.items.len() != 2 || !body.directives.is_empty() {
        return None;
    }
    let [first_parameter, second_parameter] = params.items.as_slice() else {
        return None;
    };
    if first_parameter.initializer.is_some() || second_parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(first_identifier) = &first_parameter.pattern else {
        return None;
    };
    let BindingPattern::BindingIdentifier(second_identifier) = &second_parameter.pattern else {
        return None;
    };
    let first_formal = first_identifier.symbol_id.get()?;
    let second_formal = second_identifier.symbol_id.get()?;
    if model.binding_writes(first_formal) != 0
        || model.binding_writes(second_formal) != 0
        || !model.scoping.symbol_redeclarations(first_formal).is_empty()
        || !model
            .scoping
            .symbol_redeclarations(second_formal)
            .is_empty()
    {
        return None;
    }
    let result = if expression {
        let [Statement::ExpressionStatement(statement)] = body.statements.as_slice() else {
            return None;
        };
        &statement.expression
    } else {
        let [Statement::ReturnStatement(statement)] = body.statements.as_slice() else {
            return None;
        };
        statement.argument.as_ref()?
    };
    let Expression::BinaryExpression(subtract) = result.without_parentheses() else {
        return None;
    };
    let Expression::BinaryExpression(shift) = subtract.left.without_parentheses() else {
        return None;
    };
    (subtract.operator.as_str() == "-"
        && shift.operator.as_str() == "<<"
        && number(&shift.right, 5.0)
        && is_symbol(model, &shift.left, first_formal)
        && is_symbol(model, &subtract.right, second_formal))
    .then_some(false)
}

// The shared 31-fold core: `(h << 5) - h` directly, the narrow pure IIFE
// spelling above, or the memo read `h[0] - h[1]` whose array must have been
// stored by `h = [h << 5, h]`.  The boolean reports whether the memo core was
// used.
fn fold_core(model: &Model<'_, '_>, e: &Expression<'_>, hash: Symbol) -> Option<bool> {
    if let Some(core) = iife_fold_core(model, e, hash) {
        return Some(core);
    }
    let Expression::BinaryExpression(subtract) = e else {
        return None;
    };
    if subtract.operator.as_str() != "-" {
        return None;
    }
    if let Expression::BinaryExpression(shift) = &subtract.left {
        if shift.operator.as_str() == "<<"
            && number(&shift.right, 5.0)
            && is_symbol(model, &shift.left, hash)
            && is_symbol(model, &subtract.right, hash)
        {
            return Some(false);
        }
    }
    (memo_index(model, &subtract.left, hash, 0.0) && memo_index(model, &subtract.right, hash, 1.0))
        .then_some(true)
}

// `(h << 5) - h + operand | 0` or the memo `h[0] - h[1] + operand | 0`, with the
// trailing coercion optional so the split `h = operand; h = h | 0` spelling can
// be proved separately.  The boolean reports whether the memo core was used.
fn hash_fold<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    hash: Symbol,
) -> Option<(&'a Expression<'a>, bool, bool)> {
    let (core, masked) = match e {
        Expression::BinaryExpression(or)
            if or.operator.as_str() == "|" && number(&or.right, 0.0) =>
        {
            (&or.left, true)
        }
        _ => (e, false),
    };
    let Expression::BinaryExpression(add) = core else {
        return None;
    };
    if add.operator.as_str() != "+" {
        return None;
    }
    let memo = fold_core(model, &add.left, hash)?;
    Some((&add.right, masked, memo))
}

// `h = [h << 5, h]` - the two-element memo array the split fold reads back.
fn memo_assignment(model: &Model<'_, '_>, s: &Statement<'_>, hash: Symbol) -> bool {
    let Some(Expression::AssignmentExpression(assign)) = expression_statement(s) else {
        return false;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
        return false;
    };
    if assign.operator.as_str() != "="
        || crate::analysis::identifier_symbol(target, model.scoping) != Some(hash)
    {
        return false;
    }
    let Expression::ArrayExpression(array) = &assign.right else {
        return false;
    };
    if array.elements.len() != 2 {
        return false;
    }
    let mut elements = array
        .elements
        .iter()
        .filter_map(|element| element.as_expression());
    let (Some(first), Some(second), None) = (elements.next(), elements.next(), elements.next())
    else {
        return false;
    };
    matches!(first, Expression::BinaryExpression(shift)
        if shift.operator.as_str() == "<<" && number(&shift.right, 5.0) && is_symbol(model, &shift.left, hash))
        && is_symbol(model, second, hash)
}

fn hash_assignment<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
    hash: Symbol,
) -> Option<(&'a Expression<'a>, bool, bool)> {
    let Expression::AssignmentExpression(assign) = expression_statement(s)? else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
        return None;
    };
    if assign.operator.as_str() != "="
        || crate::analysis::identifier_symbol(target, model.scoping) != Some(hash)
    {
        return None;
    }
    hash_fold(model, &assign.right, hash)
}

fn is_hash_coercion(model: &Model<'_, '_>, s: &Statement<'_>, hash: Symbol) -> bool {
    let Some(Expression::AssignmentExpression(assign)) = expression_statement(s) else {
        return false;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
        return false;
    };
    let Expression::BinaryExpression(or) = &assign.right else {
        return false;
    };
    assign.operator.as_str() == "="
        && crate::analysis::identifier_symbol(target, model.scoping) == Some(hash)
        && or.operator.as_str() == "|"
        && is_symbol(model, &or.left, hash)
        && number(&or.right, 0.0)
}

// `++i` / `i++` / `i += delta` / `i = i + delta`; the increment direction and
// stride are part of the family proof.
fn expression_step(model: &Model<'_, '_>, e: &Expression<'_>, index: Symbol, delta: f64) -> bool {
    match e {
        Expression::UpdateExpression(update) => {
            delta == 1.0
                && update.operator.as_str() == "++"
                && matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id)
                    if crate::analysis::identifier_symbol(id, model.scoping) == Some(index))
        }
        Expression::AssignmentExpression(assign) => {
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
                return false;
            };
            if crate::analysis::identifier_symbol(target, model.scoping) != Some(index) {
                return false;
            }
            match assign.operator.as_str() {
                "+=" => number(&assign.right, delta),
                "=" => {
                    let Expression::BinaryExpression(add) = &assign.right else {
                        return false;
                    };
                    add.operator.as_str() == "+"
                        && is_symbol(model, &add.left, index)
                        && number(&add.right, delta)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn statement_step(model: &Model<'_, '_>, s: &Statement<'_>, index: Symbol, delta: f64) -> bool {
    expression_statement(s).is_some_and(|e| expression_step(model, e, index, delta))
}

// Body of a fold loop: exactly one accumulator update, required 32-bit
// coercion (inline `| 0` or a following `h = h | 0`), an optional memo array
// `h = [h << 5, h]` before the update, and - for `while` loops - exactly one
// index step that follows the update.  `skip` names the hoisted unit-local
// declarations proved by the caller, which must precede the fold.
fn fold_body<'a>(
    model: &Model<'_, '_>,
    body: &'a [Statement<'a>],
    hash: Symbol,
    step: Option<(Symbol, f64)>,
    skip: &BTreeSet<u32>,
) -> Option<&'a Expression<'a>> {
    let mut operand = None;
    let mut masked = false;
    let mut stepped = false;
    let mut memo = false;
    for statement in body {
        if skip.contains(&statement.span().start) {
            if operand.is_some() {
                return None;
            }
            continue;
        }
        if memo_assignment(model, statement, hash) {
            if operand.is_some() || memo {
                return None;
            }
            memo = true;
        } else if let Some((right, inline_mask, fold_memo)) =
            hash_assignment(model, statement, hash)
        {
            if operand.is_some() || (step.is_some() && stepped) || fold_memo != memo {
                return None;
            }
            operand = Some(right);
            masked = inline_mask;
        } else if is_hash_coercion(model, statement, hash) {
            if operand.is_none() {
                return None;
            }
            masked = true;
        } else if let Some((index, delta)) = step {
            if operand.is_none() || stepped || !statement_step(model, statement, index, delta) {
                return None;
            }
            stepped = true;
        } else {
            return None;
        }
    }
    if operand.is_none() || !masked || step.is_some() && !stepped {
        return None;
    }
    operand
}

// A single unit read `accessor(input[at])` written inline.
fn unit_call<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    forward: Option<FunctionSite>,
) -> Option<(FunctionSite, Symbol, &'a Expression<'a>)> {
    let call = call_expr(e)?;
    let accessor = char_code_accessor(model, &call.callee, forward)?;
    let (object, at) = char_index(model, call, accessor, forward)?;
    Some((accessor, object, at))
}

// A unit read either inline or through a same-body immutable hoist local
// (`let u1 = accessor(input[i]);`).
fn unit_read<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    units: &[(Symbol, &'a Expression<'a>)],
    forward: Option<FunctionSite>,
) -> Option<(FunctionSite, Symbol, &'a Expression<'a>)> {
    if let Expression::Identifier(identifier) = e {
        let symbol = crate::analysis::identifier_symbol(identifier, model.scoping)?;
        let mut hoisted = None;
        for (unit, init) in units {
            if *unit == symbol {
                hoisted = Some(*init);
            }
        }
        return unit_call(model, hoisted?, forward);
    }
    unit_call(model, e, forward)
}

// Hoisted unit locals of one loop body: `let u = <call>` statements whose local
// is never written again.  The pair matcher still validates that every call
// really reads `accessor(input[i])` or `accessor(input[i + 1])`, so the skip set
// cannot smuggle an unrelated statement past the fold proof.
fn unit_locals<'a>(
    model: &Model<'_, '_>,
    body: &'a [Statement<'a>],
) -> (BTreeSet<u32>, Vec<(Symbol, &'a Expression<'a>)>) {
    let mut skip = BTreeSet::new();
    let mut units = Vec::new();
    for statement in body {
        let Some(value) = one_variable(statement) else {
            continue;
        };
        let Some(symbol) = plain_declarator(model, value) else {
            continue;
        };
        let Some(init) = value.init.as_ref() else {
            continue;
        };
        if call_expr(init).is_none() || model.binding_writes(symbol) != 0 {
            continue;
        }
        skip.insert(statement.span().start);
        units.push((symbol, init));
    }
    (skip, units)
}

fn index_plus_one(model: &Model<'_, '_>, at: &Expression<'_>, index: Symbol) -> bool {
    matches!(at, Expression::BinaryExpression(add)
        if add.operator.as_str() == "+" && is_symbol(model, &add.left, index) && number(&add.right, 1.0))
}

// `accessor(input[want])` inline or through a hoisted local.
fn unit_at<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    accessor: FunctionSite,
    input: Symbol,
    want: Symbol,
    units: &[(Symbol, &'a Expression<'a>)],
    forward: Option<FunctionSite>,
) -> bool {
    unit_read(model, e, units, forward).is_some_and(|(site, object, at)| {
        site.span == accessor.span && object == input && is_symbol(model, at, want)
    })
}

// Every collected hoist must really be one of the pair's two unit reads.
fn units_resolve<'a>(
    model: &Model<'_, '_>,
    units: &[(Symbol, &'a Expression<'a>)],
    accessor: FunctionSite,
    input: Symbol,
    index: Symbol,
    forward: Option<FunctionSite>,
) -> bool {
    units.iter().all(|entry| {
        unit_call(model, entry.1, forward).is_some_and(|(site, object, at)| {
            site.span == accessor.span
                && object == input
                && (is_symbol(model, at, index) || index_plus_one(model, at, index))
        })
    })
}

// `(unit(i) << 16 | unit(i + 1))` with both units read by the same proved
// accessor, each directly or through a hoisted local.
fn string_pair<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    input: Symbol,
    index: Symbol,
    units: &[(Symbol, &'a Expression<'a>)],
    forward: Option<FunctionSite>,
) -> Option<FunctionSite> {
    let Expression::BinaryExpression(or) = e else {
        return None;
    };
    if or.operator.as_str() != "|" {
        return None;
    }
    let Expression::BinaryExpression(shift) = &or.left else {
        return None;
    };
    if shift.operator.as_str() != "<<" || !number(&shift.right, 16.0) {
        return None;
    }
    let (first, first_object, first_at) = unit_read(model, &shift.left, units, forward)?;
    if first_object != input || !is_symbol(model, first_at, index) {
        return None;
    }
    let (second, second_object, second_at) = unit_read(model, &or.right, units, forward)?;
    if second.span != first.span || second_object != input {
        return None;
    }
    index_plus_one(model, second_at, index).then_some(first)
}

// `(unit << 16)` for the single leftover unit of the odd tail.
fn string_single<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    accessor: FunctionSite,
    input: Symbol,
    index: Symbol,
    units: &[(Symbol, &'a Expression<'a>)],
    forward: Option<FunctionSite>,
) -> bool {
    let Expression::BinaryExpression(shift) = e else {
        return false;
    };
    if shift.operator.as_str() != "<<" || !number(&shift.right, 16.0) {
        return false;
    }
    unit_at(model, &shift.left, accessor, input, index, units, forward)
}

fn less_than_length(
    model: &Model<'_, '_>,
    test: &Expression<'_>,
    index: Symbol,
    words: Symbol,
) -> bool {
    let Expression::BinaryExpression(compare) = test else {
        return false;
    };
    compare.operator.as_str() == "<"
        && is_symbol(model, &compare.left, index)
        && member(&compare.right, "length").is_some_and(|e| is_symbol(model, e, words))
}

// The index local of a fold loop, read from the loop's own test: the symbol
// compared against `words.length`.  This is how the word half binds its locals
// by use instead of by declaration order.
fn loop_index_test(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    words: Symbol,
) -> Option<Symbol> {
    let test = match statement {
        Statement::ForStatement(loop_) => loop_.test.as_ref()?,
        Statement::WhileStatement(loop_) => &loop_.test,
        _ => return None,
    };
    let Expression::BinaryExpression(compare) = test else {
        return None;
    };
    if compare.operator.as_str() != "<"
        || !member(&compare.right, "length").is_some_and(|e| is_symbol(model, e, words))
    {
        return None;
    }
    symbol(model, &compare.left)
}

// `for` (empty init) or `while`: the paired-unit loop that updates the even
// bound in the header or in the body.  Both spellings fold code units, directly
// or through the split memo array.
fn string_branch(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    statement: &Statement<'_>,
) -> Option<FunctionSite> {
    let Statement::IfStatement(branch) = statement else {
        return None;
    };
    if branch.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(kind) = &branch.test else {
        return None;
    };
    let Expression::UnaryExpression(typeof_) = &kind.left else {
        return None;
    };
    if kind.operator.as_str() != "==="
        || typeof_.operator.as_str() != "typeof"
        || symbol(model, &typeof_.argument) != Some(input)
        || !matches!(&kind.right, Expression::StringLiteral(s) if s.value == "string")
    {
        return None;
    }
    let [even_statement, index_statement, hash_statement, loop_statement, Statement::IfStatement(check)] =
        block(&branch.consequent)?
    else {
        return None;
    };
    let even_value = one_variable(even_statement)?;
    let even = plain_declarator(model, even_value)?;
    // The even bound is stable, except that the odd tail may reuse it once as
    // the hoisted unit local; the tail proof below must account for that write.
    if model.binding_writes(even) > 1 {
        return None;
    }
    let Expression::BinaryExpression(outer) = even_value.init.as_ref()? else {
        return None;
    };
    let Expression::BinaryExpression(inner) = &outer.left else {
        return None;
    };
    let length = member(&inner.left, "length")?;
    if outer.operator.as_str() != "<<"
        || !number(&outer.right, 1.0)
        || inner.operator.as_str() != ">>"
        || !number(&inner.right, 1.0)
        || !is_symbol(model, length, input)
    {
        return None;
    }
    let index_value = one_variable(index_statement)?;
    let index = plain_declarator(model, index_value)?;
    if !matches!(index_value.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0) {
        return None;
    }
    let hash_value = one_variable(hash_statement)?;
    let hash = plain_declarator(model, hash_value)?;
    if !matches!(hash_value.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0) {
        return None;
    }
    if index == hash || index == even || hash == even {
        return None;
    }
    let (operand, units) = match loop_statement {
        Statement::ForStatement(loop_) => {
            if loop_.init.is_some() {
                return None;
            }
            let Expression::BinaryExpression(compare) = loop_.test.as_ref()? else {
                return None;
            };
            if compare.operator.as_str() != "<"
                || !is_symbol(model, &compare.left, index)
                || !is_symbol(model, &compare.right, even)
            {
                return None;
            }
            let update = loop_.update.as_ref()?;
            if !expression_step(model, update, index, 2.0) {
                return None;
            }
            let body = block(&loop_.body)?;
            let (skip, units) = unit_locals(model, body);
            (fold_body(model, body, hash, None, &skip)?, units)
        }
        Statement::WhileStatement(loop_) => {
            let Expression::BinaryExpression(compare) = &loop_.test else {
                return None;
            };
            if compare.operator.as_str() != "<"
                || !is_symbol(model, &compare.left, index)
                || !is_symbol(model, &compare.right, even)
            {
                return None;
            }
            let body = block(&loop_.body)?;
            let (skip, units) = unit_locals(model, body);
            (
                fold_body(model, body, hash, Some((index, 2.0)), &skip)?,
                units,
            )
        }
        _ => return None,
    };
    let forward = Some(site);
    let accessor = string_pair(model, operand, input, index, &units, forward)?;
    if accessor.span == site.span || !units_resolve(model, &units, accessor, input, index, forward)
    {
        return None;
    }
    let Expression::BinaryExpression(check_test) = &check.test else {
        return None;
    };
    let check_length = member(&check_test.right, "length")?;
    let [Statement::ReturnStatement(exact)] = block(check.alternate.as_ref()?)? else {
        return None;
    };
    if !matches!(check_test.operator.as_str(), "!==" | "<")
        || !is_symbol(model, &check_test.left, even)
        || !is_symbol(model, check_length, input)
        || !is_symbol(model, exact.argument.as_ref()?, hash)
    {
        return None;
    }
    string_tail(
        model,
        block(&check.consequent)?,
        accessor,
        input,
        even,
        hash,
        forward,
    )?;
    Some(accessor)
}

// Odd tail: an optional `even = accessor(input[even])` hoist, an optional memo
// `h = [h << 5, h]`, then `return <fold of (unit << 16)>` over the one leftover
// unit.  At most one even write is allowed and it must be exactly this hoist.
fn string_tail<'a>(
    model: &Model<'_, '_>,
    statements: &'a [Statement<'a>],
    accessor: FunctionSite,
    input: Symbol,
    even: Symbol,
    hash: Symbol,
    forward: Option<FunctionSite>,
) -> Option<()> {
    let mut units: Vec<(Symbol, &'a Expression<'a>)> = Vec::new();
    let mut memo = false;
    let mut even_written = false;
    let mut result = None;
    for statement in statements {
        if result.is_some() {
            return None;
        }
        if let Some(expression) = expression_statement(statement) {
            let Expression::AssignmentExpression(assign) = expression else {
                return None;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
                return None;
            };
            let target = crate::analysis::identifier_symbol(target, model.scoping)?;
            if assign.operator.as_str() != "=" {
                return None;
            }
            if target == hash {
                if memo || !memo_assignment(model, statement, hash) {
                    return None;
                }
                memo = true;
            } else if target == even {
                if even_written
                    || !unit_at(model, &assign.right, accessor, input, even, &[], forward)
                {
                    return None;
                }
                even_written = true;
                units.push((even, &assign.right));
            } else {
                return None;
            }
            continue;
        }
        let Statement::ReturnStatement(returned) = statement else {
            return None;
        };
        let (tail, masked, fold_memo) = hash_fold(model, returned.argument.as_ref()?, hash)?;
        if !masked || fold_memo != memo {
            return None;
        }
        if !string_single(model, tail, accessor, input, even, &units, forward) {
            return None;
        }
        result = Some(());
    }
    if even_written != (model.binding_writes(even) == 1) {
        return None;
    }
    result
}

// Alias namespace receiver: either the ambient name or a bounded immutable
// alias of it.  The use site fixes the latest admissible declaration start.
fn ambient(model: &Model<'_, '_>, e: &Expression<'_>, expected: &str, use_start: u32) -> bool {
    namespace_root(model, e, expected, use_start, 0)
}

fn alignment(
    model: &Model<'_, '_>,
    align: &IfStatement<'_>,
    input: Symbol,
    buffer: Symbol,
) -> Option<()> {
    let Expression::BinaryExpression(aligned) = &align.test else {
        return None;
    };
    let Expression::BinaryExpression(remainder) = &aligned.left else {
        return None;
    };
    let byte_length = member(&remainder.left, "byteLength")?;
    if aligned.operator.as_str() != "==="
        || remainder.operator.as_str() != "%"
        || !number(&remainder.right, 4.0)
        || !number(&aligned.right, 0.0)
        || !is_symbol(model, byte_length, input)
    {
        return None;
    }
    let [Statement::ExpressionStatement(exact)] = block(&align.consequent)? else {
        return None;
    };
    let Expression::AssignmentExpression(exact) = &exact.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(exact_target) = &exact.left else {
        return None;
    };
    let exact_read = member(&exact.right, "buffer")?;
    if exact.operator.as_str() != "="
        || crate::analysis::identifier_symbol(exact_target, model.scoping) != Some(buffer)
        || !is_symbol(model, exact_read, input)
    {
        return None;
    }
    let [Statement::ExpressionStatement(copy), Statement::ExpressionStatement(fill)] =
        block(align.alternate.as_ref()?)?
    else {
        return None;
    };
    let Expression::AssignmentExpression(copy) = &copy.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(copy_target) = &copy.left else {
        return None;
    };
    let Expression::NewExpression(copy_new) = &copy.right else {
        return None;
    };
    let copy_arguments = new_arguments(copy_new);
    let [copy_size] = copy_arguments.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(copy_size) = copy_size else {
        return None;
    };
    let Expression::BinaryExpression(copy_tail) = &copy_size.right else {
        return None;
    };
    let Expression::BinaryExpression(copy_remainder) = &copy_tail.right else {
        return None;
    };
    if copy.operator.as_str() != "="
        || crate::analysis::identifier_symbol(copy_target, model.scoping) != Some(buffer)
        || !ambient(model, &copy_new.callee, "ArrayBuffer", copy_new.span.start)
        || copy_size.operator.as_str() != "+"
        || !member(&copy_size.left, "byteLength").is_some_and(|e| is_symbol(model, e, input))
        || copy_tail.operator.as_str() != "-"
        || !number(&copy_tail.left, 4.0)
        || copy_remainder.operator.as_str() != "%"
        || !member(&copy_remainder.left, "byteLength").is_some_and(|e| is_symbol(model, e, input))
        || !number(&copy_remainder.right, 4.0)
    {
        return None;
    }
    let Expression::CallExpression(fill) = &fill.expression else {
        return None;
    };
    let set_receiver = member(&fill.callee, "set")?;
    let Expression::NewExpression(set_receiver) = set_receiver else {
        return None;
    };
    let set_arguments = new_arguments(set_receiver);
    let [set_buffer] = set_arguments.as_slice() else {
        return None;
    };
    let fill_arguments = call_arguments(fill);
    let [fill_source] = fill_arguments.as_slice() else {
        return None;
    };
    let Expression::NewExpression(fill_source) = fill_source else {
        return None;
    };
    let fill_arguments = new_arguments(fill_source);
    let [fill_buffer] = fill_arguments.as_slice() else {
        return None;
    };
    let fill_buffer = member(fill_buffer, "buffer")?;
    if fill.optional
        || !ambient(
            model,
            &set_receiver.callee,
            "Int8Array",
            set_receiver.span.start,
        )
        || !is_symbol(model, set_buffer, buffer)
        || !ambient(
            model,
            &fill_source.callee,
            "Int8Array",
            fill_source.span.start,
        )
        || !is_symbol(model, fill_buffer, input)
    {
        return None;
    }
    Some(())
}

// The word half shares the family's 31-fold update over aligned Int32 words.
// The two zero-initialised locals are bound by use: the index is the one the
// loop test compares with `words.length`, and the accumulator is the other one,
// which the fold and the return then pin.  Declaration order is not consulted.
fn buffer_branch(model: &Model<'_, '_>, input: Symbol, statements: &[Statement<'_>]) -> Option<()> {
    let [buffer_statement, Statement::IfStatement(align), convert, first_statement, second_statement, loop_statement, Statement::ReturnStatement(result)] =
        statements
    else {
        return None;
    };
    let buffer_value = one_variable(buffer_statement)?;
    let buffer = plain_declarator(model, buffer_value)?;
    if buffer_value.init.is_some() {
        return None;
    }
    alignment(model, align, input, buffer)?;
    let words = match convert {
        Statement::ExpressionStatement(convert) => {
            let Expression::AssignmentExpression(convert) = &convert.expression else {
                return None;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(convert_target) = &convert.left else {
                return None;
            };
            let Expression::NewExpression(convert_new) = &convert.right else {
                return None;
            };
            let convert_arguments = new_arguments(convert_new);
            let [convert_buffer] = convert_arguments.as_slice() else {
                return None;
            };
            if convert.operator.as_str() != "="
                || crate::analysis::identifier_symbol(convert_target, model.scoping) != Some(buffer)
                || !ambient(
                    model,
                    &convert_new.callee,
                    "Int32Array",
                    convert_new.span.start,
                )
                || !is_symbol(model, convert_buffer, buffer)
            {
                return None;
            }
            buffer
        }
        Statement::VariableDeclaration(_) => {
            let convert_value = one_variable(convert)?;
            let words = plain_declarator(model, convert_value)?;
            if model.binding_writes(words) != 0 {
                return None;
            }
            let Expression::NewExpression(convert_new) = convert_value.init.as_ref()? else {
                return None;
            };
            let convert_arguments = new_arguments(convert_new);
            let [convert_buffer] = convert_arguments.as_slice() else {
                return None;
            };
            if !ambient(
                model,
                &convert_new.callee,
                "Int32Array",
                convert_new.span.start,
            ) || !is_symbol(model, convert_buffer, buffer)
            {
                return None;
            }
            words
        }
        _ => return None,
    };
    let first_value = one_variable(first_statement)?;
    let first = plain_declarator(model, first_value)?;
    if !matches!(first_value.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0) {
        return None;
    }
    let second_value = one_variable(second_statement)?;
    let second = plain_declarator(model, second_value)?;
    if !matches!(second_value.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0) {
        return None;
    }
    if first == second {
        return None;
    }
    let index = loop_index_test(model, loop_statement, words)?;
    let hash = if index == first {
        second
    } else if index == second {
        first
    } else {
        return None;
    };
    if hash == words || index == words {
        return None;
    }
    let operand = match loop_statement {
        Statement::ForStatement(loop_) => {
            if loop_.init.is_some() {
                return None;
            }
            let test = loop_.test.as_ref()?;
            if !less_than_length(model, test, index, words) {
                return None;
            }
            let update = loop_.update.as_ref()?;
            if !expression_step(model, update, index, 1.0) {
                return None;
            }
            fold_body(model, block(&loop_.body)?, hash, None, &BTreeSet::new())?
        }
        Statement::WhileStatement(loop_) => {
            if !less_than_length(model, &loop_.test, index, words) {
                return None;
            }
            fold_body(
                model,
                block(&loop_.body)?,
                hash,
                Some((index, 1.0)),
                &BTreeSet::new(),
            )?
        }
        _ => return None,
    };
    let Expression::ComputedMemberExpression(read) = operand else {
        return None;
    };
    if read.optional
        || !is_symbol(model, &read.object, words)
        || !is_symbol(model, &read.expression, index)
        || !is_symbol(model, result.argument.as_ref()?, hash)
    {
        return None;
    }
    Some(())
}

fn dispatch(
    model: &Model<'_, '_>,
    input: Symbol,
    flag: Symbol,
    rest: &[Statement<'_>],
) -> Option<()> {
    let [Statement::IfStatement(dispatch), tail @ ..] = rest else {
        return None;
    };
    if dispatch.alternate.is_none() {
        // Negated guard: `if (!flag) { throw new TypeError(); }` then the word
        // body follows at the same statement level.  This is the cached
        // representative's exact spelling.
        let Expression::UnaryExpression(not) = &dispatch.test else {
            return None;
        };
        if not.operator.as_str() != "!" || !is_symbol(model, &not.argument, flag) {
            return None;
        }
        let [Statement::ThrowStatement(failure)] = block(&dispatch.consequent)? else {
            return None;
        };
        let Expression::NewExpression(error) = &failure.argument else {
            return None;
        };
        if !path_is(model, &error.callee, "TypeError")
            || !error.arguments.is_empty()
            || tail.is_empty()
        {
            return None;
        }
        return buffer_branch(model, input, tail);
    }
    if !is_symbol(model, &dispatch.test, flag) {
        return None;
    }
    buffer_branch(model, input, block(&dispatch.consequent)?)?;
    let Some([Statement::ThrowStatement(failure)]) = block(dispatch.alternate.as_ref()?) else {
        return None;
    };
    let Expression::NewExpression(error) = &failure.argument else {
        return None;
    };
    (path_is(model, &error.callee, "TypeError") && error.arguments.is_empty()).then_some(())
}

fn is_view_test(
    model: &Model<'_, '_>,
    set: &AssignmentExpression<'_>,
    flag: Symbol,
    input: Symbol,
) -> bool {
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &set.left else {
        return false;
    };
    let Expression::CallExpression(view_test) = &set.right else {
        return false;
    };
    let Some(array_buffer) = member(&view_test.callee, "isView") else {
        return false;
    };
    set.operator.as_str() == "="
        && crate::analysis::identifier_symbol(target, model.scoping) == Some(flag)
        && !view_test.optional
        && ambient(model, array_buffer, "ArrayBuffer", view_test.span.start)
        && view_test.arguments.len() == 1
        && call_arguments(view_test)
            .first()
            .and_then(|e| symbol(model, e))
            == Some(input)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
        || function.params.items.len() != 1
    {
        return vec![];
    }
    let Some(input) = (match &function.params.items[0].pattern {
        BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
        _ => None,
    }) else {
        return vec![];
    };
    if function.params.items[0].initializer.is_some()
        || model.binding_writes(input) != 0
        || !model.scoping.symbol_redeclarations(input).is_empty()
    {
        return vec![];
    }
    let [string_statement, flag_statement, flag_check, rest @ ..] = body.statements.as_slice()
    else {
        return vec![];
    };
    let Some(accessor) = string_branch(model, site, input, string_statement) else {
        return vec![];
    };
    let Some(flag_value) = one_variable(flag_statement) else {
        return vec![];
    };
    let Some(flag) = plain_declarator(model, flag_value) else {
        return vec![];
    };
    let Some(flag_init) = flag_value.init.as_ref() else {
        return vec![];
    };
    if accessor.span == site.span || data_capture(model, site, flag_init).is_none() || flag == input
    {
        return vec![];
    }
    let Statement::IfStatement(flag_check) = flag_check else {
        return vec![];
    };
    if flag_check.alternate.is_some() || !is_symbol(model, &flag_check.test, flag) {
        return vec![];
    }
    let Some([Statement::ExpressionStatement(set)]) = block(&flag_check.consequent) else {
        return vec![];
    };
    let Expression::AssignmentExpression(set) = &set.expression else {
        return vec![];
    };
    if !is_view_test(model, set, flag, input) {
        return vec![];
    }
    if dispatch(model, input, flag, rest).is_none() {
        return vec![];
    }
    vec![proposal(
        "hashStringOrBuffer",
        "thread_fast_hash_variants",
        "variant loop-node proof of the established hashStringOrBuffer family: \
         a typeof-string branch folds UTF-16 reader pairs with the exact 31-fold \
         update and a one-unit odd tail, directly or through a memo array, and \
         otherwise a captured ArrayBuffer.isView feature flag gates either a \
         positive dispatch with a TypeError alternate or a negated TypeError \
         guard followed by a 4-byte aligned Int32Array word fold whose hash and \
         index locals are bound by use; every captured unit reader is itself \
         proved to return formal.charCodeAt(), even when declared after the caller",
        f,
    )]
}
