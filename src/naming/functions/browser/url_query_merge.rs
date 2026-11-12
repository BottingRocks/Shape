//! Bounded owned-body proof for the three-formal URL query-parameter merger
//! (`mergeUrlSearchParameter`) that the older classifiers decline.
//!
//! Ten captures carry one builder that reads the original URL twice,
//! merges a single injected `(name, value)` pair into its existing query
//! string and re-serializes the parsed URL:
//!
//! ```text
//! function merge(url, name, value) {
//!   let query = "";
//!   let parsed = capturedParser(url);            // captured, stable callable
//!   let entries;
//!   if (url == null || url.trim() === "") {
//!     entries = [];
//!   } else {
//!     let anchor = document.createElement("a");
//!     anchor.href = url;                         // the same input
//!     let search = anchor.search.slice(1);
//!     if (search === "") {
//!       entries = [];
//!     } else {
//!       entries = search.split("&").map(pair => { ... });
//!     }
//!   }
//!   let encodedName  = guard(encodeURIComponent(name.toString()));
//!   let encodedValue = guard(encodeURIComponent(value.toString()));
//!   if (entries.length > 0) {
//!     let found = false;
//!     entries.forEach((entry, index) => { replace first equal name, else append });
//!     if (!found) { query = query + ("&" + encodedName + "=" + encodedValue); }
//!   } else {
//!     query = encodedName + "=" + encodedValue;
//!   }
//!   return parsed.protocol + "://" + parsed.hostname
//!        + (parsed.port.length > 0 ? ":" + parsed.port : "")
//!        + parsed.pathname
//!        + (query.length > 0 ? "?" + query : "")
//!        + (parsed.hash.length > 0 ? "#" + parsed.hash : "");
//! }
//! ```
//!
//! The proof is by `SymbolId` and control-flow ownership only.  The original
//! URL is the same formal in the captured parser call, the `anchor.href` store
//! and the `trim` guard.  The `parsed` object must come from a captured stable
//! callable whose own body returns a URL-shaped object (protocol, hostname,
//! port, pathname, hash); a bare helper, a parameter or a generic URL object
//! donor cannot stand in.  The query decode/encode must be the direct ambient
//! `decodeURIComponent` / `encodeURIComponent` (unshadowed) with the exact
//! `\+` -> space and `%20` -> `+` replacements, and the undefined/null guards
//! are proved on the same component expression.  The `map` and `forEach`
//! callbacks must be inline functions owned by this exact body, so the direct
//! owned callback linkage is preserved and a nested/hoisted donor contributes
//! nothing.
//!
//! Two source shapes of `search.slice(1)` are admitted: the direct
//! `search.split("&")` and the lowered `search = [search, search.split];
//! search[1].call(search[0], "&")`.  Two source shapes of the existing-entry
//! append are admitted: the shared `"="` and the branch-complete
//! concatenation.  The existing entry value is retained on that path; only
//! replacement and new-parameter append paths use the injected encoded value.
//! The final rebuild always needs the same parsed protocol/hostname/optional
//! port/pathname, the merged query after `?`, and the parsed hash after `#`.

use super::*;
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

pub(in crate::naming) fn symbol(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn number(expression: &Expression<'_>, expected: f64) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::NumericLiteral(literal) if literal.value == expected
    )
}

pub(in crate::naming) fn boolean(
    expression: &Expression<'_>,
    expected: bool,
) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::BooleanLiteral(literal) if literal.value == expected
    )
}

pub(in crate::naming) fn text(
    expression: &Expression<'_>,
    expected: &str,
) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::StringLiteral(literal) if literal.value == expected
    )
}

/// An unresolved ambient name (no local binding shadows it).
fn global_name(model: &Model<'_, '_>, expression: &Expression<'_>, expected: &str) -> bool {
    matches!(expression_path(model, expression).as_slice(), [only] if only == expected)
}

pub(in crate::naming) fn binary<'a>(
    expression: &'a Expression<'a>,
) -> Option<&'a BinaryExpression<'a>> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    Some(binary)
}

pub(super) fn member<'a>(expression: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == key).then_some(&member.object)
}

pub(super) fn call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return None;
    };
    (!call.optional).then_some(call)
}

fn statement_call<'a>(statement: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    call(&expression.expression)
}

pub(in crate::naming) fn block<'a>(
    statement: &'a Statement<'a>,
) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(block) = statement else {
        return None;
    };
    Some(block.body.as_slice())
}

pub(in crate::naming) fn one_variable<'a>(
    statement: &'a Statement<'a>,
) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(variable)
}

pub(in crate::naming) fn plain_binding(
    model: &Model<'_, '_>,
    variable: &VariableDeclarator<'_>,
) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(symbol)
        .is_empty()
        .then_some(symbol)
}

pub(in crate::naming) fn plain_formals(
    model: &Model<'_, '_>,
    params: &FormalParameters<'_>,
) -> Option<Vec<Symbol>> {
    if params.rest.is_some() {
        return None;
    }
    let mut symbols = Vec::new();
    for parameter in &params.items {
        if parameter.initializer.is_some() {
            return None;
        }
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        let symbol = identifier.symbol_id.get()?;
        if model.binding_writes(symbol) != 0
            || !model.scoping.symbol_redeclarations(symbol).is_empty()
        {
            return None;
        }
        symbols.push(symbol);
    }
    Some(symbols)
}

fn plain_param(model: &Model<'_, '_>, parameter: &FormalParameter<'_>) -> Option<Symbol> {
    if parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    (model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty())
        .then_some(symbol)
}

/// A plain local declared in this function's own body. Parameters, catches,
/// loop targets, nested function declarations and redeclarations are refused.
pub(in crate::naming) fn body_local(
    model: &Model<'_, '_>,
    site: FunctionSite,
    symbol: Symbol,
) -> bool {
    if model.formal_symbol_slot(symbol).is_some()
        || !site
            .span
            .contains_inclusive(model.scoping.symbol_span(symbol))
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    matches!(
        model.nodes.kind(declaration),
        AstKind::VariableDeclarator(_)
    ) && matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(_)
    ) && !matches!(
        model.nodes.parent_kind(model.nodes.parent_id(declaration)),
        AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
    )
}

/// A consumer-provided binding declared outside this function.
fn is_captured(model: &Model<'_, '_>, site: FunctionSite, symbol: Symbol) -> bool {
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(symbol))
        || model.formal_symbol_slot(symbol).is_some()
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    matches!(
        model.nodes.kind(model.scoping.symbol_declaration(symbol)),
        AstKind::VariableDeclarator(_) | AstKind::Function(_) | AstKind::Class(_)
    )
}

pub(super) fn array_items<'a>(expression: &'a Expression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return None;
    };
    array
        .elements
        .iter()
        .map(|element| element.as_expression())
        .collect()
}

pub(in crate::naming) fn assignment_statement<'a>(
    statement: &'a Statement<'a>,
) -> Option<&'a AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = expression.expression.without_parentheses()
    else {
        return None;
    };
    Some(assignment)
}

pub(in crate::naming) fn target_symbol(
    model: &Model<'_, '_>,
    assignment: &AssignmentExpression<'_>,
) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

// ===========================================================================
// Component values, the guarded decode/encode forms and the merge strings
// ===========================================================================

/// The component whose decode/encode is proved: a formal, or `parts[index]` of
/// the callback's own split.
#[derive(Clone, Copy)]
pub(in crate::naming) enum ValueRef {
    Param(Symbol),
    Index { owner: Symbol, index: u32 },
}

pub(super) fn is_value(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    value: ValueRef,
) -> bool {
    match value {
        ValueRef::Param(param_symbol) => symbol(model, expression) == Some(param_symbol),
        ValueRef::Index { owner, index } => {
            let Expression::ComputedMemberExpression(member) = expression.without_parentheses()
            else {
                return false;
            };
            !member.optional
                && number(&member.expression, index as f64)
                && symbol(model, &member.object) == Some(owner)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::naming) enum Direction {
    Decode,
    Encode,
}

impl Direction {
    fn callee(self) -> &'static str {
        match self {
            Direction::Decode => "decodeURIComponent",
            Direction::Encode => "encodeURIComponent",
        }
    }

    fn pattern(self) -> &'static str {
        match self {
            Direction::Decode => "\\+",
            Direction::Encode => "%20",
        }
    }

    fn replacement(self) -> &'static str {
        match self {
            Direction::Decode => " ",
            Direction::Encode => "+",
        }
    }
}

fn pattern_and_flags(args: &[&Expression<'_>], pattern: &str) -> bool {
    let [literal, flags] = args else {
        return false;
    };
    text(literal, pattern) && text(flags, "g")
}

/// `RegExp(pattern, "g")`, `new RegExp(pattern, "g")` or
/// `new RegExp(RegExp(pattern, "g"))`: the global replacement pattern.
fn regex(model: &Model<'_, '_>, expression: &Expression<'_>, pattern: &str) -> bool {
    match expression.without_parentheses() {
        Expression::CallExpression(call) => {
            global_name(model, &call.callee, "RegExp")
                && pattern_and_flags(&call_arguments(call), pattern)
        }
        Expression::NewExpression(new) => {
            if !global_name(model, &new.callee, "RegExp") {
                return false;
            }
            let args: Vec<&Expression<'_>> = new
                .arguments
                .iter()
                .filter_map(Argument::as_expression)
                .collect();
            match args.as_slice() {
                [inner] => regex(model, inner, pattern),
                _ => pattern_and_flags(&args, pattern),
            }
        }
        _ => false,
    }
}

/// `decodeURIComponent(component.toString()).replace(RegExp(...), " ")` or
/// the mirror `encodeURIComponent(component.toString()).replace(..., "+")`.
fn transform(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    value: ValueRef,
    direction: Direction,
) -> bool {
    let Some(replace) = call(expression) else {
        return false;
    };
    let Some(encoded) = member(&replace.callee, "replace") else {
        return false;
    };
    let replace_arguments = call_arguments(replace);
    let [pattern, replacement] = replace_arguments.as_slice() else {
        return false;
    };
    if !text(replacement, direction.replacement()) || !regex(model, pattern, direction.pattern()) {
        return false;
    }
    let Some(encoded) = call(encoded) else {
        return false;
    };
    if !global_name(model, &encoded.callee, direction.callee()) {
        return false;
    }
    let encoded_arguments = call_arguments(encoded);
    let [to_string] = encoded_arguments.as_slice() else {
        return false;
    };
    let Some(to_string) = call(to_string) else {
        return false;
    };
    let Some(component) = member(&to_string.callee, "toString") else {
        return false;
    };
    is_value(model, component, value)
        && to_string.arguments.is_empty()
        && encoded.arguments.len() == 1
        && replace.arguments.len() == 2
}

pub(in crate::naming) fn is_typeof_undefined(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    value: ValueRef,
) -> bool {
    let Some(test) = binary(expression) else {
        return false;
    };
    if test.operator.as_str() != "===" {
        return false;
    }
    let Expression::UnaryExpression(unary) = test.left.without_parentheses() else {
        return false;
    };
    unary.operator.as_str() == "typeof"
        && is_value(model, &unary.argument, value)
        && text(&test.right, "undefined")
}

pub(in crate::naming) fn is_null_test(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    value: ValueRef,
) -> bool {
    let Some(test) = binary(expression) else {
        return false;
    };
    test.operator.as_str() == "==="
        && is_value(model, &test.left, value)
        && matches!(test.right.without_parentheses(), Expression::NullLiteral(_))
}

/// The guarded component value: `typeof x === "undefined" ? "undefined" :
/// x === null ? "null" : decode(x)`, or the null-only tail used inside an
/// `if (typeof x === "undefined") ... else ...` node shape.
pub(in crate::naming) fn guarded_expr(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    value: ValueRef,
    direction: Direction,
) -> bool {
    match expression.without_parentheses() {
        Expression::ConditionalExpression(conditional) => {
            if is_typeof_undefined(model, &conditional.test, value) {
                text(&conditional.consequent, "undefined")
                    && guarded_null(model, &conditional.alternate, value, direction)
            } else if is_null_test(model, &conditional.test, value) {
                text(&conditional.consequent, "null")
                    && transform(model, &conditional.alternate, value, direction)
            } else {
                false
            }
        }
        _ => transform(model, expression, value, direction),
    }
}

fn guarded_null(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    value: ValueRef,
    direction: Direction,
) -> bool {
    match expression.without_parentheses() {
        Expression::ConditionalExpression(conditional)
            if is_null_test(model, &conditional.test, value) =>
        {
            text(&conditional.consequent, "null")
                && transform(model, &conditional.alternate, value, direction)
        }
        _ => transform(model, expression, value, direction),
    }
}

/// `if (typeof x === "undefined") { out = ""; } else { out = guarded; }`
fn guarded_if(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    out: Symbol,
    value: ValueRef,
    direction: Direction,
) -> bool {
    let Statement::IfStatement(branch) = statement else {
        return false;
    };
    if !is_typeof_undefined(model, &branch.test, value) {
        return false;
    }
    let Some(consequent) = block(&branch.consequent) else {
        return false;
    };
    let [default_statement] = consequent else {
        return false;
    };
    let Some(default) = assignment_statement(default_statement) else {
        return false;
    };
    if default.operator.as_str() != "="
        || target_symbol(model, default) != Some(out)
        || !(text(&default.right, "") || text(&default.right, "undefined"))
    {
        return false;
    }
    let Some(alternate) = branch.alternate.as_ref().and_then(block) else {
        return false;
    };
    let [guarded_statement] = alternate else {
        return false;
    };
    let Some(guarded) = assignment_statement(guarded_statement) else {
        return false;
    };
    guarded.operator.as_str() == "="
        && target_symbol(model, guarded) == Some(out)
        && guarded_expr(model, &guarded.right, value, direction)
}

/// A component value bound either by one guarded initializer or by a declared
/// local plus its `if` guard. Returns the cursor after the binding and the
/// bound symbol.
fn guarded_local(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    cursor: usize,
    value: ValueRef,
    direction: Direction,
) -> Option<(usize, Symbol)> {
    let variable = one_variable(statements.get(cursor)?)?;
    let local = plain_binding(model, variable)?;
    if let Some(initializer) = variable.init.as_ref() {
        if guarded_expr(model, initializer, value, direction) {
            return Some((cursor + 1, local));
        }
        return None;
    }
    if !guarded_if(model, statements.get(cursor + 1)?, local, value, direction) {
        return None;
    }
    Some((cursor + 2, local))
}

/// A concatenation leaf: a string literal, a symbol, or the `index === 0`
/// separator choice (`zero` for the first entry, `other` afterwards).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::naming) enum Part {
    Text(String),
    Sym(Symbol),
    Choice { zero: Vec<Part>, other: Vec<Part> },
}

fn sym(symbol: Symbol) -> Part {
    Part::Sym(symbol)
}

fn txt(value: &str) -> Part {
    Part::Text(value.to_string())
}

fn zero_test(model: &Model<'_, '_>, expression: &Expression<'_>, index: Symbol) -> bool {
    let Some(test) = binary(expression) else {
        return false;
    };
    test.operator.as_str() == "==="
        && symbol(model, &test.left) == Some(index)
        && number(&test.right, 0.0)
}

pub(in crate::naming) fn parts(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    index: Symbol,
    depth: usize,
) -> Option<Vec<Part>> {
    if depth > 64 {
        return None;
    }
    match expression.without_parentheses() {
        Expression::StringLiteral(literal) => Some(vec![Part::Text(literal.value.to_string())]),
        Expression::Identifier(identifier) => Some(vec![Part::Sym(
            crate::analysis::identifier_symbol(identifier, model.scoping)?,
        )]),
        Expression::BinaryExpression(binary) if binary.operator.as_str() == "+" => {
            let mut left = parts(model, &binary.left, index, depth + 1)?;
            left.extend(parts(model, &binary.right, index, depth + 1)?);
            Some(left)
        }
        Expression::ConditionalExpression(conditional) => {
            if !zero_test(model, &conditional.test, index) {
                return None;
            }
            Some(vec![Part::Choice {
                zero: parts(model, &conditional.consequent, index, depth + 1)?,
                other: parts(model, &conditional.alternate, index, depth + 1)?,
            }])
        }
        _ => None,
    }
}

// ===========================================================================
// Captured parser and the URL-shaped object it yields
// ===========================================================================

fn url_object(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let object = match expression.without_parentheses() {
        Expression::ObjectExpression(object) => object,
        Expression::Identifier(identifier) => {
            let Some(symbol) = crate::analysis::identifier_symbol(identifier, model.scoping) else {
                return false;
            };
            let Some(value) = model.symbol_value(symbol) else {
                return false;
            };
            let Expression::ObjectExpression(object) = value.without_parentheses() else {
                return false;
            };
            object
        }
        _ => return false,
    };
    let mut keys = BTreeSet::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            continue;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            continue;
        }
        if let Some(key) = property_key_name(&property.key) {
            keys.insert(key);
        }
    }
    ["protocol", "hostname", "port", "pathname", "hash"]
        .iter()
        .all(|key| keys.contains(*key))
}

fn returns_url_object_in(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    budget: &mut usize,
) -> bool {
    for statement in statements {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        match statement {
            Statement::ReturnStatement(returned) => {
                if returned
                    .argument
                    .as_ref()
                    .is_some_and(|argument| url_object(model, argument))
                {
                    return true;
                }
            }
            Statement::BlockStatement(block) => {
                if returns_url_object_in(model, block.body.as_slice(), budget) {
                    return true;
                }
            }
            Statement::IfStatement(branch) => {
                if returns_url_object_in(model, std::slice::from_ref(&branch.consequent), budget) {
                    return true;
                }
                if let Some(alternate) = branch.alternate.as_ref() {
                    if returns_url_object_in(model, std::slice::from_ref(alternate), budget) {
                        return true;
                    }
                }
            }
            Statement::TryStatement(tried) => {
                if returns_url_object_in(model, tried.block.body.as_slice(), budget) {
                    return true;
                }
                if let Some(handler) = tried.handler.as_ref() {
                    if returns_url_object_in(model, handler.body.body.as_slice(), budget) {
                        return true;
                    }
                }
                if let Some(finalizer) = tried.finalizer.as_ref() {
                    if returns_url_object_in(model, finalizer.body.as_slice(), budget) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn parser_returns_url_object(model: &Model<'_, '_>, target: FunctionSite) -> bool {
    let body = match model.nodes.kind(target.node_id) {
        AstKind::Function(function) => match function.body.as_deref() {
            Some(body) => body,
            None => return false,
        },
        AstKind::ArrowFunctionExpression(arrow) => &*arrow.body,
        _ => return false,
    };
    let mut budget = 128usize;
    returns_url_object_in(model, body.statements.as_slice(), &mut budget)
}

/// A captured, stable single-origin callable that returns a URL-shaped object.
fn captured_callable(
    model: &Model<'_, '_>,
    site: FunctionSite,
    callee: &Expression<'_>,
) -> Option<FunctionSite> {
    let Expression::Identifier(identifier) = callee.without_parentheses() else {
        return None;
    };
    let symbol = crate::analysis::identifier_symbol(identifier, model.scoping)?;
    if !is_captured(model, site, symbol) {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let writes = model.binding_writes(symbol);
    let stable = match model.nodes.kind(declaration) {
        AstKind::Function(_) => writes == 0,
        AstKind::VariableDeclarator(variable) => {
            (writes == 0 && variable.init.is_some()) || (writes == 1 && variable.init.is_none())
        }
        _ => false,
    };
    if !stable {
        return None;
    }
    let target = model.function_from_expression(callee)?;
    if target.span == site.span || site.span.contains_inclusive(target.span) {
        return None;
    }
    if !parser_returns_url_object(model, target) {
        return None;
    }
    Some(target)
}

// ===========================================================================
// The own query parse and the two split node shapes
// ===========================================================================

fn created_anchor(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Some(create_element) = call(expression) else {
        return false;
    };
    let Some(document) = member(&create_element.callee, "createElement") else {
        return false;
    };
    if !global_name(model, document, "document") {
        return false;
    }
    let arguments = call_arguments(create_element);
    let [kind] = arguments.as_slice() else {
        return false;
    };
    text(kind, "a") && create_element.arguments.len() == 1
}

pub(in crate::naming) fn entries_empty_assignment(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    entries: Symbol,
) -> bool {
    let Some(assignment) = assignment_statement(statement) else {
        return false;
    };
    assignment.operator.as_str() == "="
        && target_symbol(model, assignment) == Some(entries)
        && array_items(&assignment.right).is_some_and(|items| items.is_empty())
}

fn split_entries(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    search: Symbol,
    lowered: bool,
) -> bool {
    let Some(split) = call(expression) else {
        return false;
    };
    if lowered {
        let Some(one) = member(&split.callee, "call") else {
            return false;
        };
        let Expression::ComputedMemberExpression(one) = one.without_parentheses() else {
            return false;
        };
        if one.optional
            || !number(&one.expression, 1.0)
            || symbol(model, &one.object) != Some(search)
        {
            return false;
        }
        let arguments = call_arguments(split);
        let [self_argument, separator] = arguments.as_slice() else {
            return false;
        };
        let Expression::ComputedMemberExpression(zero) = self_argument.without_parentheses() else {
            return false;
        };
        !zero.optional
            && number(&zero.expression, 0.0)
            && symbol(model, &zero.object) == Some(search)
            && text(separator, "&")
            && split.arguments.len() == 2
    } else {
        let Some(object) = member(&split.callee, "split") else {
            return false;
        };
        let arguments = call_arguments(split);
        let [separator] = arguments.as_slice() else {
            return false;
        };
        symbol(model, object) == Some(search) && text(separator, "&") && split.arguments.len() == 1
    }
}

fn parse_alternate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    entries: Symbol,
    search: Symbol,
    statements: &[Statement<'_>],
) -> Option<()> {
    match statements {
        [first, second] => {
            if model.binding_writes(search) != 1 {
                return None;
            }
            let assignment = assignment_statement(first)?;
            if assignment.operator.as_str() != "="
                || target_symbol(model, assignment) != Some(search)
            {
                return None;
            }
            let items = array_items(&assignment.right)?;
            let [value, split] = items.as_slice() else {
                return None;
            };
            if symbol(model, value) != Some(search) {
                return None;
            }
            let Some(split) = member(split, "split") else {
                return None;
            };
            if symbol(model, split) != Some(search) {
                return None;
            }
            entries_map_assignment(model, site, second, entries, search, true)
        }
        [only] => {
            if model.binding_writes(search) != 0 {
                return None;
            }
            entries_map_assignment(model, site, only, entries, search, false)
        }
        _ => None,
    }
}

/// The own `document.createElement("a")` + `anchor.href = input` +
/// `anchor.search.slice(1)` parse, then either `split("&")` node shape.
pub(in crate::naming) fn query_parse(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    entries: Symbol,
    statements: &[Statement<'_>],
) -> Option<()> {
    let [anchor_statement, href_statement, search_statement, inner_if] = statements else {
        return None;
    };
    let anchor_variable = one_variable(anchor_statement)?;
    let anchor = plain_binding(model, anchor_variable)?;
    if !body_local(model, site, anchor) || !created_anchor(model, anchor_variable.init.as_ref()?) {
        return None;
    }
    let href = assignment_statement(href_statement)?;
    let AssignmentTarget::StaticMemberExpression(href_target) = &href.left else {
        return None;
    };
    if href.operator.as_str() != "="
        || href_target.optional
        || href_target.property.name != "href"
        || symbol(model, &href_target.object) != Some(anchor)
        || symbol(model, &href.right) != Some(input)
    {
        return None;
    }
    let search_variable = one_variable(search_statement)?;
    let search = plain_binding(model, search_variable)?;
    if !body_local(model, site, search) {
        return None;
    }
    let Some(slice) = call(search_variable.init.as_ref()?) else {
        return None;
    };
    let Some(search_member) = member(&slice.callee, "slice") else {
        return None;
    };
    let Some(anchor_object) = member(search_member, "search") else {
        return None;
    };
    if symbol(model, anchor_object) != Some(anchor) {
        return None;
    }
    let arguments = call_arguments(slice);
    let [one] = arguments.as_slice() else {
        return None;
    };
    if !number(one, 1.0) || slice.arguments.len() != 1 {
        return None;
    }
    let Statement::IfStatement(branch) = inner_if else {
        return None;
    };
    let Some(test) = binary(&branch.test) else {
        return None;
    };
    if test.operator.as_str() != "==="
        || symbol(model, &test.left) != Some(search)
        || !text(&test.right, "")
    {
        return None;
    }
    let Some(consequent) = block(&branch.consequent) else {
        return None;
    };
    let [empty] = consequent else {
        return None;
    };
    if !entries_empty_assignment(model, empty, entries) {
        return None;
    }
    let alternate = branch.alternate.as_ref().and_then(block)?;
    parse_alternate(model, site, entries, search, alternate)
}

// ===========================================================================
// Direct owned decode/encode callbacks and the merge bookkeeping
// ===========================================================================

struct InlineCallback<'a> {
    params: &'a FormalParameters<'a>,
    body: &'a FunctionBody<'a>,
    site: FunctionSite,
}

fn inline_callback<'a>(
    model: &Model<'_, '_>,
    expression: &'a Expression<'a>,
) -> Option<InlineCallback<'a>> {
    match expression.without_parentheses() {
        Expression::ArrowFunctionExpression(arrow) => {
            if arrow.r#async || arrow.expression || !arrow.body.directives.is_empty() {
                return None;
            }
            let site = model.site_by_span(arrow.span)?;
            Some(InlineCallback {
                params: arrow.params.as_ref(),
                body: arrow.body.as_ref(),
                site,
            })
        }
        Expression::FunctionExpression(function) => {
            if function.r#async || function.generator {
                return None;
            }
            let body = function.body.as_deref()?;
            if !body.directives.is_empty() {
                return None;
            }
            let site = model.site_by_span(function.span)?;
            Some(InlineCallback {
                params: function.params.as_ref(),
                body,
                site,
            })
        }
        _ => None,
    }
}

pub(super) fn owned_by(model: &Model<'_, '_>, callback: FunctionSite, site: FunctionSite) -> bool {
    model
        .enclosing_function(callback.node_id)
        .map(|parent| parent.span)
        == Some(site.span)
}

fn entries_map_assignment(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    entries: Symbol,
    search: Symbol,
    lowered: bool,
) -> Option<()> {
    let assignment = assignment_statement(statement)?;
    if assignment.operator.as_str() != "=" || target_symbol(model, assignment) != Some(entries) {
        return None;
    }
    let map = call(&assignment.right)?;
    let receiver = member(&map.callee, "map")?;
    if !split_entries(model, receiver, search, lowered) || map.arguments.len() != 1 {
        return None;
    }
    let arguments = call_arguments(map);
    let [callback] = arguments.as_slice() else {
        return None;
    };
    map_callback(model, site, callback, entries)
}

/// The inline map callback that decodes one `key=value` pair into the
/// `[decodedKey, decodedValue]` entry. Returns after proving the pair split,
/// both guarded decodes, the output array and its return.
fn map_callback(
    model: &Model<'_, '_>,
    site: FunctionSite,
    callback: &Expression<'_>,
    _entries: Symbol,
) -> Option<()> {
    let callback = inline_callback(model, callback)?;
    if !owned_by(model, callback.site, site) || callback.params.rest.is_some() {
        return None;
    }
    let [pair_parameter] = callback.params.items.as_slice() else {
        return None;
    };
    let pair = plain_param(model, pair_parameter)?;
    let statements = callback.body.statements.as_slice();

    let parts_variable = one_variable(statements.first()?)?;
    let parts = plain_binding(model, parts_variable)?;
    let Some(split) = call(parts_variable.init.as_ref()?) else {
        return None;
    };
    let Some(object) = member(&split.callee, "split") else {
        return None;
    };
    if symbol(model, object) != Some(pair) || split.arguments.len() != 1 {
        return None;
    }
    let split_arguments = call_arguments(split);
    let [separator] = split_arguments.as_slice() else {
        return None;
    };
    if !text(separator, "=") {
        return None;
    }

    let (cursor, key) = guarded_local(
        model,
        statements,
        1,
        ValueRef::Index {
            owner: parts,
            index: 0,
        },
        Direction::Decode,
    )?;
    let (cursor, value) = guarded_local(
        model,
        statements,
        cursor,
        ValueRef::Index {
            owner: parts,
            index: 1,
        },
        Direction::Decode,
    )?;
    if key == value || key == parts || value == parts {
        return None;
    }

    let [output_statement, Statement::ReturnStatement(returned)] = &statements[cursor..] else {
        return None;
    };
    let Some(output) = assignment_statement(output_statement) else {
        return None;
    };
    if output.operator.as_str() != "=" {
        return None;
    }
    let out = target_symbol(model, output)?;
    if out != key && out != value {
        return None;
    }
    let items = array_items(&output.right)?;
    let [first, second] = items.as_slice() else {
        return None;
    };
    if symbol(model, first) != Some(key) || symbol(model, second) != Some(value) {
        return None;
    }
    (symbol(model, returned.argument.as_ref()?) == Some(out)).then_some(())
}

/// The inline forEach callback that replaces the first equal encoded name and
/// appends every other entry. Returns the callback index symbol so the outer
/// `if (!found)` append can prove the same `index === 0` separator.
pub(in crate::naming) fn for_each_callback(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    entries: Symbol,
    query: Symbol,
    found: Symbol,
    encoded_name: Symbol,
    encoded_value: Symbol,
) -> Option<Symbol> {
    let for_each = statement_call(statement)?;
    if for_each.arguments.len() != 1 {
        return None;
    }
    let Some(object) = member(&for_each.callee, "forEach") else {
        return None;
    };
    if symbol(model, object) != Some(entries) {
        return None;
    }
    let arguments = call_arguments(for_each);
    let [callback] = arguments.as_slice() else {
        return None;
    };
    let callback = inline_callback(model, callback)?;
    if !owned_by(model, callback.site, site) || callback.params.rest.is_some() {
        return None;
    }
    let [entry_parameter, index_parameter] = callback.params.items.as_slice() else {
        return None;
    };
    let entry = plain_param(model, entry_parameter)?;
    let index = plain_param(model, index_parameter)?;
    if entry == index {
        return None;
    }
    let statements = callback.body.statements.as_slice();
    let (cursor, key) = guarded_local(
        model,
        statements,
        0,
        ValueRef::Index {
            owner: entry,
            index: 0,
        },
        Direction::Encode,
    )?;
    let (cursor, value) = guarded_local(
        model,
        statements,
        cursor,
        ValueRef::Index {
            owner: entry,
            index: 1,
        },
        Direction::Encode,
    )?;
    if key == value {
        return None;
    }

    let [Statement::IfStatement(branch)] = &statements[cursor..] else {
        return None;
    };
    let Expression::LogicalExpression(and) = branch.test.without_parentheses() else {
        return None;
    };
    if and.operator.as_str() != "&&" {
        return None;
    }
    let Some(equal) = binary(&and.left) else {
        return None;
    };
    if equal.operator.as_str() != "==="
        || symbol(model, &equal.left) != Some(key)
        || symbol(model, &equal.right) != Some(encoded_name)
    {
        return None;
    }
    let Expression::UnaryExpression(not) = and.right.without_parentheses() else {
        return None;
    };
    if not.operator.as_str() != "!" || symbol(model, &not.argument) != Some(found) {
        return None;
    }

    let Some(consequent) = block(&branch.consequent) else {
        return None;
    };
    let [replace_statement, mark_statement] = consequent else {
        return None;
    };
    let Some(replace) = assignment_statement(replace_statement) else {
        return None;
    };
    if replace.operator.as_str() != "=" || target_symbol(model, replace) != Some(query) {
        return None;
    }
    let expected_replace = vec![
        sym(query),
        Part::Choice {
            zero: vec![sym(encoded_name)],
            other: vec![txt("&"), sym(encoded_name)],
        },
        txt("="),
        sym(encoded_value),
    ];
    if parts(model, &replace.right, index, 0)? != expected_replace {
        return None;
    }
    let Some(mark) = assignment_statement(mark_statement) else {
        return None;
    };
    if mark.operator.as_str() != "="
        || target_symbol(model, mark) != Some(found)
        || !boolean(&mark.right, true)
    {
        return None;
    }

    let Some(Statement::IfStatement(else_if)) = branch.alternate.as_ref() else {
        return None;
    };
    if else_if.alternate.is_some() {
        return None;
    }
    let Some(not_equal) = binary(&else_if.test) else {
        return None;
    };
    if not_equal.operator.as_str() != "!=="
        || symbol(model, &not_equal.left) != Some(key)
        || symbol(model, &not_equal.right) != Some(encoded_name)
    {
        return None;
    }
    let Some(append) = block(&else_if.consequent) else {
        return None;
    };
    let [append_statement] = append else {
        return None;
    };
    let Some(append) = assignment_statement(append_statement) else {
        return None;
    };
    if append.operator.as_str() != "=" || target_symbol(model, append) != Some(query) {
        return None;
    }
    let actual = parts(model, &append.right, index, 0)?;
    // A nonmatching existing entry must retain its own encoded value. The
    // injected value is reserved for the equal-name replacement and the
    // outer not-found/empty-entry append paths below.
    let shared_separator = vec![
        sym(query),
        Part::Choice {
            zero: vec![sym(key), txt("=")],
            other: vec![txt("&"), sym(key), txt("=")],
        },
        sym(value),
    ];
    let branch_complete = vec![
        sym(query),
        Part::Choice {
            zero: vec![sym(key), txt("="), sym(value)],
            other: vec![txt("&"), sym(key), txt("="), sym(value)],
        },
    ];
    if actual != shared_separator && actual != branch_complete {
        return None;
    }
    Some(index)
}

fn merge_branch(
    model: &Model<'_, '_>,
    site: FunctionSite,
    branch: &IfStatement<'_>,
    entries: Symbol,
    query: Symbol,
    encoded_name: Symbol,
    encoded_value: Symbol,
) -> Option<()> {
    let Some(length) = binary(&branch.test) else {
        return None;
    };
    if length.operator.as_str() != ">" || !number(&length.right, 0.0) {
        return None;
    }
    let Some(entries_length) = member(&length.left, "length") else {
        return None;
    };
    if symbol(model, entries_length) != Some(entries) {
        return None;
    }

    let consequent = block(&branch.consequent)?;
    let [found_statement, for_each_statement, append_if] = consequent else {
        return None;
    };
    let found_variable = one_variable(found_statement)?;
    let found = plain_binding(model, found_variable)?;
    if !boolean(found_variable.init.as_ref()?, false) || model.binding_writes(found) != 1 {
        return None;
    }
    let index = for_each_callback(
        model,
        site,
        for_each_statement,
        entries,
        query,
        found,
        encoded_name,
        encoded_value,
    )?;

    let Statement::IfStatement(append_if) = append_if else {
        return None;
    };
    let Expression::UnaryExpression(not) = append_if.test.without_parentheses() else {
        return None;
    };
    if not.operator.as_str() != "!" || symbol(model, &not.argument) != Some(found) {
        return None;
    }
    if append_if.alternate.is_some() {
        return None;
    }
    let Some(append) = block(&append_if.consequent) else {
        return None;
    };
    let [append_statement] = append else {
        return None;
    };
    let Some(append) = assignment_statement(append_statement) else {
        return None;
    };
    if append.operator.as_str() != "=" || target_symbol(model, append) != Some(query) {
        return None;
    }
    let expected_append = vec![
        sym(query),
        txt("&"),
        sym(encoded_name),
        txt("="),
        sym(encoded_value),
    ];
    if parts(model, &append.right, index, 0)? != expected_append {
        return None;
    }

    let alternate = block(branch.alternate.as_ref()?)?;
    let [empty_statement] = alternate else {
        return None;
    };
    let Some(empty) = assignment_statement(empty_statement) else {
        return None;
    };
    if empty.operator.as_str() != "=" || target_symbol(model, empty) != Some(query) {
        return None;
    }
    let expected_empty = vec![sym(encoded_name), txt("="), sym(encoded_value)];
    (parts(model, &empty.right, index, 0)? == expected_empty).then_some(())
}

fn encode_formal(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    formal: Symbol,
) -> Option<Symbol> {
    let variable = one_variable(statement)?;
    let local = plain_binding(model, variable)?;
    guarded_expr(
        model,
        variable.init.as_ref()?,
        ValueRef::Param(formal),
        Direction::Encode,
    )
    .then_some(local)
}

pub(in crate::naming) fn input_guard(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    input: Symbol,
) -> bool {
    let Expression::LogicalExpression(logical) = expression.without_parentheses() else {
        return false;
    };
    if logical.operator.as_str() != "||" {
        return false;
    }
    let Some(null_test) = binary(&logical.left) else {
        return false;
    };
    if !matches!(null_test.operator.as_str(), "==" | "===")
        || symbol(model, &null_test.left) != Some(input)
        || !matches!(
            null_test.right.without_parentheses(),
            Expression::NullLiteral(_)
        )
    {
        return false;
    }
    let Some(trim) = binary(&logical.right) else {
        return false;
    };
    if trim.operator.as_str() != "===" {
        return false;
    }
    let Some(trim_call) = call(&trim.left) else {
        return false;
    };
    member(&trim_call.callee, "trim").and_then(|object| symbol(model, object)) == Some(input)
        && trim_call.arguments.is_empty()
        && text(&trim.right, "")
}

// ===========================================================================
// Final URL rebuild
// ===========================================================================

fn url_field(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    parsed: Symbol,
    key: &str,
) -> bool {
    member(expression, key).and_then(|object| symbol(model, object)) == Some(parsed)
}

fn length_positive(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    parsed: Symbol,
    key: &str,
) -> bool {
    let Some(test) = binary(expression) else {
        return false;
    };
    if test.operator.as_str() != ">" || !number(&test.right, 0.0) {
        return false;
    }
    let Some(length) = member(&test.left, "length") else {
        return false;
    };
    url_field(model, length, parsed, key)
}

fn plus_terms<'a>(
    expression: &'a Expression<'a>,
    terms: &mut Vec<&'a Expression<'a>>,
    depth: usize,
) {
    if depth > 64 {
        return;
    }
    if let Expression::BinaryExpression(binary) = expression.without_parentheses() {
        if binary.operator.as_str() == "+" {
            plus_terms(&binary.left, terms, depth + 1);
            plus_terms(&binary.right, terms, depth + 1);
            return;
        }
    }
    terms.push(expression.without_parentheses());
}

fn port_condition(model: &Model<'_, '_>, expression: &Expression<'_>, parsed: Symbol) -> bool {
    let Expression::ConditionalExpression(conditional) = expression.without_parentheses() else {
        return false;
    };
    if !length_positive(model, &conditional.test, parsed, "port")
        || !text(&conditional.alternate, "")
    {
        return false;
    }
    let mut terms = Vec::new();
    plus_terms(&conditional.consequent, &mut terms, 0);
    let [colon, port] = terms.as_slice() else {
        return false;
    };
    text(colon, ":") && url_field(model, port, parsed, "port")
}

fn query_condition(model: &Model<'_, '_>, expression: &Expression<'_>, query: Symbol) -> bool {
    let Expression::ConditionalExpression(conditional) = expression.without_parentheses() else {
        return false;
    };
    let Some(test) = binary(&conditional.test) else {
        return false;
    };
    if test.operator.as_str() != ">" || !number(&test.right, 0.0) {
        return false;
    }
    let Some(length) = member(&test.left, "length") else {
        return false;
    };
    if symbol(model, length) != Some(query) || !text(&conditional.alternate, "") {
        return false;
    }
    let mut terms = Vec::new();
    plus_terms(&conditional.consequent, &mut terms, 0);
    let [question, value] = terms.as_slice() else {
        return false;
    };
    text(question, "?") && symbol(model, value) == Some(query)
}

fn hash_condition(model: &Model<'_, '_>, expression: &Expression<'_>, parsed: Symbol) -> bool {
    let Expression::ConditionalExpression(conditional) = expression.without_parentheses() else {
        return false;
    };
    if !length_positive(model, &conditional.test, parsed, "hash")
        || !text(&conditional.alternate, "")
    {
        return false;
    }
    let mut terms = Vec::new();
    plus_terms(&conditional.consequent, &mut terms, 0);
    let [hash, value] = terms.as_slice() else {
        return false;
    };
    text(hash, "#") && url_field(model, value, parsed, "hash")
}

fn return_merge(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    parsed: Symbol,
    query: Symbol,
) -> Option<()> {
    let mut terms = Vec::new();
    plus_terms(expression, &mut terms, 0);
    let mut terms = terms.into_iter();
    let protocol = terms.next()?;
    let colon = terms.next()?;
    let hostname = terms.next()?;
    let port = terms.next()?;
    let pathname = terms.next()?;
    let query_part = terms.next()?;
    let hash = terms.next()?;
    if terms.next().is_some() {
        return None;
    }
    if !url_field(model, protocol, parsed, "protocol")
        || !text(colon, "://")
        || !url_field(model, hostname, parsed, "hostname")
        || !port_condition(model, port, parsed)
        || !url_field(model, pathname, parsed, "pathname")
        || !query_condition(model, query_part, query)
        || !hash_condition(model, hash, parsed)
    {
        return None;
    }
    Some(())
}

// ===========================================================================
// Entry point
// ===========================================================================

fn merge(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<()> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_deref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let formals = plain_formals(model, &function.params)?;
    let [input, name_formal, value_formal] = formals.as_slice() else {
        return None;
    };
    let (input, name_formal, value_formal) = (*input, *name_formal, *value_formal);
    if input == name_formal || input == value_formal || name_formal == value_formal {
        return None;
    }
    let [query_statement, parsed_statement, entries_statement, parse_if, name_statement, value_statement, merge_if, return_statement] =
        body.statements.as_slice()
    else {
        return None;
    };

    let query_variable = one_variable(query_statement)?;
    let query = plain_binding(model, query_variable)?;
    if !body_local(model, site, query)
        || !text(query_variable.init.as_ref()?, "")
        || model.binding_writes(query) == 0
    {
        return None;
    }

    let parsed_variable = one_variable(parsed_statement)?;
    let parsed = plain_binding(model, parsed_variable)?;
    if !body_local(model, site, parsed) || model.binding_writes(parsed) != 0 {
        return None;
    }
    let Some(parser_call) = call(parsed_variable.init.as_ref()?) else {
        return None;
    };
    let parser_arguments = call_arguments(parser_call);
    let [parser_input] = parser_arguments.as_slice() else {
        return None;
    };
    if symbol(model, parser_input) != Some(input) || parser_call.arguments.len() != 1 {
        return None;
    }
    captured_callable(model, site, &parser_call.callee)?;

    let entries_variable = one_variable(entries_statement)?;
    let entries = plain_binding(model, entries_variable)?;
    if !body_local(model, site, entries) || entries_variable.init.is_some() {
        return None;
    }

    let Statement::IfStatement(parse_if) = parse_if else {
        return None;
    };
    if !input_guard(model, &parse_if.test, input) {
        return None;
    }
    let Some(consequent) = block(&parse_if.consequent) else {
        return None;
    };
    let [empty] = consequent else {
        return None;
    };
    if !entries_empty_assignment(model, empty, entries) {
        return None;
    }
    let alternate = parse_if.alternate.as_ref().and_then(block)?;
    query_parse(model, site, input, entries, alternate)?;

    let encoded_name = encode_formal(model, name_statement, name_formal)?;
    let encoded_value = encode_formal(model, value_statement, value_formal)?;
    if !body_local(model, site, encoded_name) || !body_local(model, site, encoded_value) {
        return None;
    }

    let Statement::IfStatement(merge_if) = merge_if else {
        return None;
    };
    merge_branch(
        model,
        site,
        merge_if,
        entries,
        query,
        encoded_name,
        encoded_value,
    )?;

    let Statement::ReturnStatement(returned) = return_statement else {
        return None;
    };
    return_merge(model, returned.argument.as_ref()?, parsed, query)?;

    let mut distinct = BTreeSet::new();
    for identity in [
        input,
        name_formal,
        value_formal,
        query,
        parsed,
        entries,
        encoded_name,
        encoded_value,
    ] {
        if !distinct.insert(identity) {
            return None;
        }
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    if function.body.is_none() || merge(model, site, function).is_none() {
        return super::url_query_reader::evaluate(model, site, function, facts);
    }
    vec![proposal(
        "mergeUrlSearchParameter",
        "thread_url_query_merge",
        "whole owned body: a captured stable callable parses the original URL formal \
         into a URL-shaped object; the same input is parsed again through this body's \
         own document.createElement(\"a\") + anchor.href + anchor.search.slice(1); the \
         query is split on \"&\" (direct or lowered split.call) and each pair is mapped \
         through an inline owned callback that splits on \"=\", decodes both components \
         with the exact typeof-undefined/null guards and the direct \
         decodeURIComponent(...).replace(/\\+/g, \" \") mirror, and returns \
         [decodedKey, decodedValue]; the injected formal name and value are encoded by \
         the direct encodeURIComponent(...).replace(/%20/g, \"+\"); a forEach over the \
         same entries replaces the first equal encoded name with the injected value while \
         preserving each nonmatching entry's encoded value under the index-zero separator, \
         or the empty/not-found paths write the injected name=value; and the final \
         return rebuilds the same parsed protocol, hostname, optional port, pathname, \
         merged query and hash",
        facts,
    )]
}
