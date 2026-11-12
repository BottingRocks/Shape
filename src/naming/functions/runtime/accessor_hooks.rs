//! Bounded, owned-body proof for the accessor-descriptor interceptor.
//!
//! The logical family is unchanged across the cached
//! captures:
//!
//! ```text
//! function hook(object, property) {
//!   let record = <lookup>(object, property);
//!   if (record == null) return null;
//!   let owner = record.containingObj;
//!   record = record.desc;
//!   let configurable = record.configurable;
//!   let getter = record.get;
//!   let setter = record.set;
//!   let dataProperty = <apply>(Object.hasOwnProperty, record, "value");
//!   let original = Object.create(null);
//!   if (configurable === false || dataProperty) {
//!     if (getter != null) original.get = getter;
//!     if (setter != null) original.set = setter;
//!     original = { originals: original };
//!     return original;
//!   } else {
//!     let prior = <registryLookup>(owner, property);
//!     if (prior != null) {
//!       if (prior.result != null) return prior.result;
//!       getter = prior.original.get;
//!       setter = prior.original.set;
//!     }
//!     let hooks = { onAfterGet, onBeforeGet, onAfterSet, onBeforeSet, originals };
//!     if (getter != null) { ... install wrapped getter ... }
//!     if (setter != null) { ... install wrapped setter ... }
//!     Object.defineProperty(owner, property, record);
//!     hooks.originals = original;
//!     <register { object, result: hooks, original } under property>
//!     return hooks;
//!   }
//! }
//! ```
//!
//! The getter wrapper notifies a before/after hook pair around the original
//! accessor and honours `bypassResult`; the setter wrapper additionally accepts
//! an optional rewritten `param`. The wrappers are owned nested declarations
//! and are re-proved here from source, so the outer interceptor never borrows a
//! name or a role from a nested callback. The outer interceptor is the only
//! function this rule claims.
//!
//! # Deliberately variable source shapes
//!
//! These are the variants observed in the cached family bodies:
//!
//! * every local, parameter and helper spelling is opaque; only resolved scope
//!   symbols and exact unbound global names participate,
//! * `let`, `const` and `var` plain declarators are equivalent, and adjacent
//!   declarators may be grouped into one declaration,
//! * a wrapper may alias its mutable event record (`let event = pre`) or
//!   reassign the single event record directly,
//! * the registration record may be an inline object literal, a fresh full
//!   literal assigned to the owner local, or an `{ object }` step followed by
//!   `result` / `original` field writes in either order,
//! * the getter and setter blocks, the two accessor copies and the two final
//!   field writes may appear in either order where the language leaves the
//!   result unchanged,
//! * the descriptor lookup may be the accepted two-argument helper call or an
//!   inlined prototype walk that advances one cursor receiver with
//!   `Object.getPrototypeOf` and packs exactly `{ containingObj, desc }`; one or
//!   two locals may alias the packed record,
//! * the prior lookup may be the accepted helper call or an inlined scan of
//!   `registry[property]` that bounds-checks the bucket, selects the element
//!   whose `.object` is the containing owner, and keeps the selected record and
//!   its derived result in one or two locals.
//!
//! # Required invariants
//!
//! * Two plain formals, no rest/default/async/generator and no directives.
//! * One descriptor lookup, guarded by `== null` / `return null`.
//! * `configurable` / data-property dispatch with the exact
//!   `=== false || hasOwnProperty(record, "value")` protocol.
//! * The get/set hooks object literal with exactly the four hook keys and an
//!   `originals` slot, and the exact `onBefore*` / `onAfter*` wiring to the
//!   observers created by the same nullary factory.
//! * `Object.defineProperty(owner, property, record)` on the unshadowed global
//!   `Object`, `Object.create(null)` for the original descriptor, and
//!   `Array.prototype.push` through the same opaque apply helper.
//! * Every accessor invocation goes through that one opaque apply helper and
//!   every notification through the paired observers' `.notify` method.
//!
//! No generated name, vendor, span, body hash, positional index or inherited
//! label is consulted. The older stages are untouched: this module is only
//! reached after every earlier generation has declined the function.
use super::*;
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

mod captured;

/// Identity of an opaque helper. Bound callees resolve to a scope symbol;
/// unbound helper protocols keep their exact global spelling, never a
/// substring or a fuzzy match.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Reference {
    Symbol(Symbol),
    Global(String),
}

fn reference(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Reference> {
    let Expression::Identifier(identifier) = e.without_parentheses() else {
        return None;
    };
    Some(
        match crate::analysis::identifier_symbol(identifier, model.scoping) {
            Some(symbol) => Reference::Symbol(symbol),
            None => Reference::Global(identifier.name.to_string()),
        },
    )
}

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    let Expression::Identifier(identifier) = e.without_parentheses() else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

fn member<'a>(e: &'a Expression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = e.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn receiver(model: &Model<'_, '_>, e: &Expression<'_>, name: &str) -> Option<Symbol> {
    member(e, name).and_then(|object| symbol(model, object))
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = e.without_parentheses() else {
        return None;
    };
    (!call.optional).then_some(call)
}

fn arguments<'a>(call: &'a CallExpression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    let values = call_arguments(call);
    (values.len() == call.arguments.len()).then_some(values)
}

fn expression_statement<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    Some(&statement.expression)
}

fn simple_assignment<'a>(statement: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Expression::AssignmentExpression(assignment) = expression_statement(statement)? else {
        return None;
    };
    (assignment.operator.as_str() == "=").then_some(assignment)
}

fn target_symbol(model: &Model<'_, '_>, target: &AssignmentTarget<'_>) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

fn target_member<'a>(target: &'a AssignmentTarget<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn object_fields<'a>(e: &'a Expression<'a>) -> Option<BTreeMap<String, &'a Expression<'a>>> {
    let Expression::ObjectExpression(object) = e.without_parentheses() else {
        return None;
    };
    let mut fields = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return None;
        }
        let name = property_key_name(&property.key)?;
        if fields.insert(name, &property.value).is_some() {
            return None;
        }
    }
    Some(fields)
}

fn object_of<'a>(
    e: &'a Expression<'a>,
    keys: &[&str],
) -> Option<BTreeMap<String, &'a Expression<'a>>> {
    let fields = object_fields(e)?;
    (fields.len() == keys.len() && keys.iter().all(|key| fields.contains_key(*key)))
        .then_some(fields)
}

fn field<'a>(
    fields: &BTreeMap<String, &'a Expression<'a>>,
    key: &str,
) -> Option<&'a Expression<'a>> {
    fields.get(key).copied()
}

fn is_this(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::ThisExpression(_))
}

fn is_null(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::NullLiteral(_))
}

fn is_boolean(e: &Expression<'_>, expected: bool) -> bool {
    matches!(e.without_parentheses(), Expression::BooleanLiteral(value) if value.value == expected)
}

fn number(e: &Expression<'_>, expected: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(value) if value.value == expected)
}

fn string(e: &Expression<'_>, expected: &str) -> bool {
    matches!(e.without_parentheses(), Expression::StringLiteral(value) if value.value == expected)
}

fn void_zero(e: &Expression<'_>) -> bool {
    matches!(
        e.without_parentheses(),
        Expression::UnaryExpression(expression)
            if expression.operator.as_str() == "void" && number(&expression.argument, 0.0)
    )
}

fn path_is(model: &Model<'_, '_>, e: &Expression<'_>, expected: &[&str]) -> bool {
    let path = expression_path(model, e);
    path.len() == expected.len()
        && path
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.as_str() == *expected)
}

fn strict(body: &FunctionBody<'_>) -> bool {
    body.directives.len() == 1 && body.directives[0].directive == "use strict"
}

fn plain_parameter(model: &Model<'_, '_>, pattern: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = pattern else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    (model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty())
        .then_some(symbol)
}

fn declared(model: &Model<'_, '_>, declarator: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(symbol)
        .is_empty()
        .then_some(symbol)
}

/// A body statement flattened to one logical item. A multi-declarator `let`
/// contributes each declarator separately, so declaration grouping cannot
/// decide the proof.
enum Item<'a> {
    Decl(&'a VariableDeclarator<'a>),
    Stmt(&'a Statement<'a>),
}

fn flatten<'a>(statements: &'a [Statement<'a>]) -> Vec<Item<'a>> {
    let mut items = Vec::new();
    for statement in statements {
        if let Statement::VariableDeclaration(declaration) = statement {
            for declarator in &declaration.declarations {
                items.push(Item::Decl(declarator));
            }
        } else {
            items.push(Item::Stmt(statement));
        }
    }
    items
}

fn declared_init<'a>(
    model: &Model<'_, '_>,
    item: &Item<'a>,
) -> Option<(Symbol, &'a Expression<'a>)> {
    let Item::Decl(declarator) = item else {
        return None;
    };
    let declarator = *declarator;
    Some((declared(model, declarator)?, declarator.init.as_ref()?))
}

fn declared_bare<'a>(model: &Model<'_, '_>, item: &Item<'a>) -> Option<Symbol> {
    let Item::Decl(declarator) = item else {
        return None;
    };
    let declarator = *declarator;
    let symbol = declared(model, declarator)?;
    declarator.init.is_none().then_some(symbol)
}

fn item_statement<'a>(item: &Item<'a>) -> Option<&'a Statement<'a>> {
    let Item::Stmt(statement) = item else {
        return None;
    };
    Some(*statement)
}

#[derive(Clone, Copy)]
enum Equality {
    Equal,
    NotEqual,
}

fn binary_test<'a>(
    test: &'a Expression<'a>,
    operator: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(binary) = test.without_parentheses() else {
        return None;
    };
    (binary.operator.as_str() == operator).then_some((&binary.left, &binary.right))
}

fn null_test(
    model: &Model<'_, '_>,
    test: &Expression<'_>,
    subject: Symbol,
    equality: Equality,
) -> bool {
    let operators: &[&str] = match equality {
        Equality::Equal => &["==", "==="],
        Equality::NotEqual => &["!=", "!=="],
    };
    let Some((left, right)) = operators
        .iter()
        .find_map(|operator| binary_test(test, operator))
    else {
        return false;
    };
    (symbol(model, left) == Some(subject) && is_null(right))
        || (symbol(model, right) == Some(subject) && is_null(left))
}

fn member_null_test(
    model: &Model<'_, '_>,
    test: &Expression<'_>,
    owner: Symbol,
    name: &str,
    equality: Equality,
) -> bool {
    let operators: &[&str] = match equality {
        Equality::Equal => &["==", "==="],
        Equality::NotEqual => &["!=", "!=="],
    };
    let Some((left, right)) = operators
        .iter()
        .find_map(|operator| binary_test(test, operator))
    else {
        return false;
    };
    let is_member = |e: &Expression<'_>| {
        member(e, name).and_then(|object| symbol(model, object)) == Some(owner)
    };
    (is_member(left) && is_null(right)) || (is_member(right) && is_null(left))
}

fn bypass_member(model: &Model<'_, '_>, e: &Expression<'_>, event: Symbol, field: &str) -> bool {
    member(e, field)
        .and_then(|object| member(object, "bypassResult"))
        .and_then(|object| symbol(model, object))
        == Some(event)
}

fn bypass_null(
    model: &Model<'_, '_>,
    test: &Expression<'_>,
    event: Symbol,
    equality: Equality,
) -> bool {
    let operators: &[&str] = match equality {
        Equality::Equal => &["==", "==="],
        Equality::NotEqual => &["!=", "!=="],
    };
    let Some((left, right)) = operators
        .iter()
        .find_map(|operator| binary_test(test, operator))
    else {
        return false;
    };
    // The member side of the comparison is already `event.bypassResult`; adding
    // another `bypassResult` step here would require `.bypassResult.bypassResult`.
    let has_bypass = |e: &Expression<'_>| {
        member(e, "bypassResult").and_then(|object| symbol(model, object)) == Some(event)
    };
    (has_bypass(left) && is_null(right)) || (has_bypass(right) && is_null(left))
}

fn negated(model: &Model<'_, '_>, test: &Expression<'_>, subject: Symbol) -> bool {
    matches!(
        test.without_parentheses(),
        Expression::UnaryExpression(expression)
            if expression.operator.as_str() == "!"
                && symbol(model, &expression.argument) == Some(subject)
    )
}

fn logical<'a>(
    test: &'a Expression<'a>,
    operator: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(logical) = test.without_parentheses() else {
        return None;
    };
    (logical.operator.as_str() == operator).then_some((&logical.left, &logical.right))
}

fn exact_apply<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    apply: &Reference,
    count: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let call = call(e)?;
    if reference(model, &call.callee)? != *apply {
        return None;
    }
    let values = arguments(call)?;
    (values.len() == count).then_some(values)
}

fn throw_block(statements: &[Statement<'_>], model: &Model<'_, '_>, event: Symbol) -> bool {
    matches!(
        statements,
        [Statement::ThrowStatement(throw)]
            if bypass_member(model, &throw.argument, event, "value")
    )
}

fn return_block(statements: &[Statement<'_>], model: &Model<'_, '_>, event: Symbol) -> bool {
    matches!(
        statements,
        [Statement::ReturnStatement(returned)]
            if returned.argument.as_ref().is_some_and(|argument| bypass_member(model, argument, event, "value"))
    )
}

/// `if (event.bypassResult.throw) { throw event.bypassResult.value; }
///  else { return event.bypassResult.value; }`
fn throw_or_return(model: &Model<'_, '_>, branch: &IfStatement<'_>, event: Symbol) -> bool {
    let Some(Statement::BlockStatement(alternate)) = branch.alternate.as_ref() else {
        return false;
    };
    let Statement::BlockStatement(consequent) = &branch.consequent else {
        return false;
    };
    bypass_member(model, &branch.test, event, "throw")
        && throw_block(&consequent.body, model, event)
        && return_block(&alternate.body, model, event)
}

fn object_has_own(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    path_is(model, e, &["Object", "hasOwnProperty"])
}

/// `if (source != null) { buffer[field] = source; }` — an accessor copy into
/// the original-descriptor buffer.
fn accessor_copy(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    buffer: Symbol,
    source: Symbol,
    field_name: &str,
) -> bool {
    let Some(Statement::IfStatement(branch)) = item_statement(item) else {
        return false;
    };
    if branch.alternate.is_some() || !null_test(model, &branch.test, source, Equality::NotEqual) {
        return false;
    }
    let Statement::BlockStatement(consequent) = &branch.consequent else {
        return false;
    };
    let [copy] = consequent.body.as_slice() else {
        return false;
    };
    let Some(assignment) = simple_assignment(copy) else {
        return false;
    };
    target_member(&assignment.left, field_name).and_then(|object| symbol(model, object))
        == Some(buffer)
        && symbol(model, &assignment.right) == Some(source)
}

/// `let observer = <same factory>();`
fn factory_binding(model: &Model<'_, '_>, item: &Item<'_>) -> Option<(Symbol, Reference)> {
    let (symbol, init) = declared_init(model, item)?;
    let call = call(init)?;
    if !call.arguments.is_empty() {
        return None;
    }
    Some((symbol, reference(model, &call.callee)?))
}

fn nested_function<'a>(item: &Item<'a>) -> Option<(&'a Function<'a>, Symbol)> {
    let Statement::FunctionDeclaration(function) = item_statement(item)? else {
        return None;
    };
    let symbol = function.id.as_ref()?.symbol_id.get()?;
    (function.r#async == false
        && function.generator == false
        && function.params.rest.is_none()
        && function
            .params
            .items
            .iter()
            .all(|parameter| parameter.initializer.is_none()))
    .then_some((&**function, symbol))
}

/// `let string = <namespace>.toString.call(original);`
fn preserved_tostring(model: &Model<'_, '_>, item: &Item<'_>, original: Symbol) -> Option<Symbol> {
    let (string_symbol, init) = declared_init(model, item)?;
    let call = call(init)?;
    if static_member_name(&call.callee) != Some("call") {
        return None;
    }
    let tostring = member(&call.callee, "call")?;
    if static_member_name(tostring) != Some("toString") {
        return None;
    }
    let values = arguments(call)?;
    let [value] = values.as_slice() else {
        return None;
    };
    (symbol(model, value) == Some(original)).then_some(string_symbol)
}

fn push_call<'a>(item: &Item<'a>) -> Option<&'a CallExpression<'a>> {
    let expression = expression_statement(item_statement(item)?)?;
    let call = call(expression)?;
    (static_member_name(&call.callee) == Some("push")).then_some(call)
}

fn target_is_static_member_of(
    model: &Model<'_, '_>,
    target: &AssignmentTarget<'_>,
    name: &str,
    owner_path: &[&str],
) -> bool {
    target_member(target, name).is_some_and(|object| path_is(model, object, owner_path))
}

/// The `typeof original === "function"` toString preservation guard.
fn tostring_preservation(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    original: Symbol,
    record: Symbol,
    installed: &str,
) -> bool {
    let items = flatten(statements);
    let [string_item, first_push, second_push, guard] = items.as_slice() else {
        return false;
    };
    let Some(string_symbol) = preserved_tostring(model, string_item, original) else {
        return false;
    };
    // `namespace.push(<source string>)`
    let Some(first) = push_call(first_push).and_then(arguments) else {
        return false;
    };
    let [first_value] = first.as_slice() else {
        return false;
    };
    if symbol(model, first_value) != Some(string_symbol) {
        return false;
    }
    // `namespace.push(record.<installed>)`
    let Some(second) = push_call(second_push).and_then(arguments) else {
        return false;
    };
    let [second_value] = second.as_slice() else {
        return false;
    };
    if member(second_value, installed).and_then(|object| symbol(model, object)) != Some(record) {
        return false;
    }
    // `if (Function.prototype.toString !== namespace.toString) { ... = ...; }`
    let Some(statement) = item_statement(guard) else {
        return false;
    };
    let Statement::IfStatement(guard) = statement else {
        return false;
    };
    if guard.alternate.is_some() {
        return false;
    }
    let Some((left, right)) =
        binary_test(&guard.test, "!==").or_else(|| binary_test(&guard.test, "!="))
    else {
        return false;
    };
    let function_path = path_is(model, left, &["Function", "prototype", "toString"]);
    let function_value = static_member_name(right) == Some("toString");
    if !(function_path && function_value) {
        return false;
    }
    let Some(Statement::BlockStatement(consequent)) = Some(&guard.consequent) else {
        return false;
    };
    let [assign] = consequent.body.as_slice() else {
        return false;
    };
    let Some(assign) = simple_assignment(assign) else {
        return false;
    };
    // The shim is installed from the same saved toString alias object the guard
    // compares against; `Function.prototype.toString` itself is the target only.
    let shim_alias = member(right, "toString").and_then(|object| reference(model, object));
    shim_alias.is_some()
        && target_is_static_member_of(model, &assign.left, "toString", &["Function", "prototype"])
        && member(&assign.right, "toString").and_then(|object| reference(model, object))
            == shim_alias
}

fn typeof_function(model: &Model<'_, '_>, test: &Expression<'_>, original: Symbol) -> bool {
    let Some((left, right)) = binary_test(test, "===")
        .or_else(|| binary_test(test, "=="))
        .or_else(|| binary_test(test, "!=="))
        .or_else(|| binary_test(test, "!="))
    else {
        return false;
    };
    let unary = |e: &Expression<'_>| {
        matches!(
            e.without_parentheses(),
            Expression::UnaryExpression(expression)
                if expression.operator.as_str() == "typeof"
                    && symbol(model, &expression.argument) == Some(original)
        )
    };
    (unary(left) && string(right, "function")) || (unary(right) && string(left, "function"))
}

fn typeof_guard(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    original: Symbol,
    record: Symbol,
    installed: &str,
) -> bool {
    let Some(Statement::IfStatement(branch)) = item_statement(item) else {
        return false;
    };
    if branch.alternate.is_some() || !typeof_function(model, &branch.test, original) {
        return false;
    }
    let Statement::BlockStatement(consequent) = &branch.consequent else {
        return false;
    };
    let [attempt] = consequent.body.as_slice() else {
        return false;
    };
    let Statement::TryStatement(attempt) = attempt else {
        return false;
    };
    if attempt.finalizer.is_some() {
        return false;
    }
    let Some(handler) = attempt.handler.as_ref() else {
        return false;
    };
    let Some(parameter) = handler.param.as_ref() else {
        return false;
    };
    let BindingPattern::BindingIdentifier(_) = &parameter.pattern else {
        return false;
    };
    if !handler.body.body.is_empty() {
        return false;
    }
    tostring_preservation(model, &attempt.block.body, original, record, installed)
}

/// `try { if (!apply(Object.hasOwnProperty, original, "prototype")) {
///          record.<installed>.prototype = void 0;
///        } } catch (<plain>) {}`
fn prototype_guard(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    record: Symbol,
    original: Symbol,
    installed: &str,
    apply: &Reference,
) -> bool {
    let Some(Statement::TryStatement(attempt)) = item_statement(item) else {
        return false;
    };
    if attempt.finalizer.is_some() {
        return false;
    }
    let Some(handler) = attempt.handler.as_ref() else {
        return false;
    };
    let Some(parameter) = handler.param.as_ref() else {
        return false;
    };
    let BindingPattern::BindingIdentifier(_) = &parameter.pattern else {
        return false;
    };
    if !handler.body.body.is_empty() {
        return false;
    }
    let [Statement::IfStatement(branch)] = attempt.block.body.as_slice() else {
        return false;
    };
    if branch.alternate.is_some() {
        return false;
    }
    let Expression::UnaryExpression(not) = branch.test.without_parentheses() else {
        return false;
    };
    if not.operator.as_str() != "!" {
        return false;
    }
    let Some(values) = exact_apply(model, &not.argument, apply, 3) else {
        return false;
    };
    let [has_own, target, prototype] = values.as_slice() else {
        return false;
    };
    if !object_has_own(model, has_own)
        || symbol(model, target) != Some(original)
        || !string(prototype, "prototype")
    {
        return false;
    }
    let Statement::BlockStatement(consequent) = &branch.consequent else {
        return false;
    };
    let [clear] = consequent.body.as_slice() else {
        return false;
    };
    let Some(clear) = simple_assignment(clear) else {
        return false;
    };
    target_member(&clear.left, "prototype")
        .and_then(|object| member(object, installed))
        .and_then(|object| symbol(model, object))
        == Some(record)
        && void_zero(&clear.right)
}

/// Re-prove the owned getter wrapper and return its `(before, after)`
/// observer identities.
fn getter_body(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    original: Symbol,
    apply: &Reference,
    split_guard: bool,
) -> Option<(Symbol, Symbol)> {
    if function.r#async
        || function.generator
        || !function.params.items.is_empty()
        || function.params.rest.is_some()
    {
        return None;
    }
    let body = function.body.as_ref()?;
    if !strict(body) {
        return None;
    }
    let items = flatten(&body.statements);
    let (notify_item, guarded, fallback, split) = match items.as_slice() {
        [notify, guarded, fallback] if !split_guard => (notify, guarded, fallback, None),
        [notify, temporary, test, guarded, fallback] if split_guard => {
            (notify, guarded, fallback, Some((temporary, test)))
        }
        _ => return None,
    };
    let (notify, notify_init) = declared_init(model, notify_item)?;
    let notify_call = call(notify_init)?;
    let before = receiver(model, &notify_call.callee, "notify")?;
    let values = arguments(notify_call)?;
    let [argument] = values.as_slice() else {
        return None;
    };
    let fields = object_of(argument, &["thisObj"])?;
    if !field(&fields, "thisObj").is_some_and(is_this) {
        return None;
    }
    let Some(Statement::IfStatement(guard)) = item_statement(guarded) else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    if let Some((temporary, test)) = split {
        let (temporary, init) = declared_init(model, temporary)?;
        if symbol(model, init) != Some(notify) || temporary == notify {
            return None;
        }
        let Statement::IfStatement(test) = item_statement(test)? else {
            return None;
        };
        if test.alternate.is_some() || symbol(model, &test.test) != Some(temporary) {
            return None;
        }
        let Statement::BlockStatement(block) = &test.consequent else {
            return None;
        };
        let [assignment] = block.body.as_slice() else {
            return None;
        };
        let assignment = simple_assignment(assignment)?;
        if target_symbol(model, &assignment.left) != Some(temporary)
            || !bypass_null(model, &assignment.right, notify, Equality::NotEqual)
            || !negated(model, &guard.test, temporary)
        {
            return None;
        }
    } else {
        let (left, right) = logical(&guard.test, "||")?;
        if !((negated(model, left, notify) && bypass_null(model, right, notify, Equality::Equal))
            || (negated(model, right, notify) && bypass_null(model, left, notify, Equality::Equal)))
        {
            return None;
        }
    }
    let Statement::BlockStatement(consequent) = &guard.consequent else {
        return None;
    };
    let after = getter_protected(model, &consequent.body, notify, original, apply)?;
    let Some(Statement::IfStatement(fallback)) = item_statement(fallback) else {
        return None;
    };
    throw_or_return(model, fallback, notify).then_some((before, after))
}

fn getter_protected(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    notify: Symbol,
    original: Symbol,
    apply: &Reference,
) -> Option<Symbol> {
    let items = flatten(statements);
    let (event, event_init) = declared_init(model, items.first()?)?;
    if !getter_pre_event(model, event_init) {
        return None;
    }
    let try_index = items
        .iter()
        .position(|item| matches!(item_statement(item), Some(Statement::TryStatement(_))))?;
    if try_index != items.len().checked_sub(2)? {
        return None;
    }
    let mut bare = Vec::new();
    let mut alias = None;
    for item in &items[1..try_index] {
        let Item::Decl(declarator) = item else {
            return None;
        };
        let declarator = *declarator;
        let declared_symbol = declared(model, declarator)?;
        match declarator.init.as_ref() {
            None => bare.push(declared_symbol),
            Some(init) if symbol(model, init) == Some(event) => {
                if alias.replace(declared_symbol).is_some() {
                    return None;
                }
            }
            Some(_) => return None,
        }
    }
    let Statement::TryStatement(attempt) = item_statement(&items[try_index])? else {
        return None;
    };
    if attempt.handler.is_some() {
        return None;
    }
    let finalizer = attempt.finalizer.as_ref()?;
    let [invoke, completed] = attempt.block.body.as_slice() else {
        return None;
    };
    let Some(invoke) = simple_assignment(invoke) else {
        return None;
    };
    let Some(result) = target_symbol(model, &invoke.left) else {
        return None;
    };
    let values = exact_apply(model, &invoke.right, apply, 2)?;
    let [accessor, this_arg] = values.as_slice() else {
        return None;
    };
    if symbol(model, accessor) != Some(original) || !is_this(this_arg) {
        return None;
    }
    let event_target = alias.unwrap_or(event);
    let Some(completed) = simple_assignment(completed) else {
        return None;
    };
    if target_symbol(model, &completed.left) != Some(event_target)
        || !getter_post_event(model, &completed.right, result)
    {
        return None;
    }
    let final_items = flatten(&finalizer.body);
    let [notify_item, guard_item] = final_items.as_slice() else {
        return None;
    };
    let (after_event, after_init) = declared_init(model, notify_item)?;
    let after_call = call(after_init)?;
    let after = receiver(model, &after_call.callee, "notify")?;
    if after == notify {
        return None;
    }
    let values = arguments(after_call)?;
    let [event_arg] = values.as_slice() else {
        return None;
    };
    if symbol(model, event_arg) != Some(event_target) {
        return None;
    }
    let Some(Statement::IfStatement(guard)) = item_statement(guard_item) else {
        return None;
    };
    let Some((left, right)) = logical(&guard.test, "&&") else {
        return None;
    };
    if !((symbol(model, left) == Some(after_event)
        && bypass_null(model, right, after_event, Equality::NotEqual))
        || (symbol(model, right) == Some(after_event)
            && bypass_null(model, left, after_event, Equality::NotEqual)))
    {
        return None;
    }
    let Statement::BlockStatement(consequent) = &guard.consequent else {
        return None;
    };
    let [inner] = consequent.body.as_slice() else {
        return None;
    };
    let Statement::IfStatement(inner) = inner else {
        return None;
    };
    if !throw_or_return(model, inner, after_event) {
        return None;
    }
    let Some(Statement::BlockStatement(alternate)) = guard.alternate.as_ref() else {
        return None;
    };
    let [finish] = alternate.body.as_slice() else {
        return None;
    };
    let Some(finish) = simple_assignment(finish) else {
        return None;
    };
    let Some(out) = target_symbol(model, &finish.left) else {
        return None;
    };
    if symbol(model, &finish.right) != Some(result) {
        return None;
    }
    if bare.len() != 2 || result == out || !bare.contains(&result) || !bare.contains(&out) {
        return None;
    }
    let Some(Statement::ReturnStatement(returned)) = item_statement(items.last()?) else {
        return None;
    };
    if returned.argument.as_ref().and_then(|e| symbol(model, e)) != Some(out) {
        return None;
    }
    Some(after)
}

fn getter_pre_event(_model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Some(fields) = object_of(e, &["thisObj", "result", "threw"]) else {
        return false;
    };
    field(&fields, "thisObj").is_some_and(is_this)
        && field(&fields, "result").is_some_and(is_null)
        && field(&fields, "threw").is_some_and(|value| is_boolean(value, true))
}

fn getter_post_event(model: &Model<'_, '_>, e: &Expression<'_>, result: Symbol) -> bool {
    let Some(fields) = object_of(e, &["thisObj", "result", "threw"]) else {
        return false;
    };
    field(&fields, "thisObj").is_some_and(is_this)
        && field(&fields, "result").is_some_and(|value| symbol(model, value) == Some(result))
        && field(&fields, "threw").is_some_and(|value| is_boolean(value, false))
}

/// Re-prove the owned setter wrapper and return its `(before, after)`
/// observer identities.
fn setter_body(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    original: Symbol,
    apply: &Reference,
) -> Option<(Symbol, Symbol)> {
    if function.r#async || function.generator || function.params.rest.is_some() {
        return None;
    }
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let param = plain_parameter(model, &parameter.pattern)?;
    let body = function.body.as_ref()?;
    if !strict(body) {
        return None;
    }
    let items = flatten(&body.statements);
    let [notify_item, dispatch, event_item, rest @ ..] = items.as_slice() else {
        return None;
    };
    let (notify, notify_init) = declared_init(model, notify_item)?;
    let notify_call = call(notify_init)?;
    let before = receiver(model, &notify_call.callee, "notify")?;
    let values = arguments(notify_call)?;
    let [argument] = values.as_slice() else {
        return None;
    };
    let fields = object_of(argument, &["param", "thisObj"])?;
    if !field(&fields, "param").is_some_and(|value| symbol(model, value) == Some(param))
        || !field(&fields, "thisObj").is_some_and(is_this)
    {
        return None;
    }
    let Some(Statement::IfStatement(dispatch)) = item_statement(dispatch) else {
        return None;
    };
    if !setter_dispatch(model, dispatch, notify, param, apply) {
        return None;
    }
    let (event, event_init) = declared_init(model, event_item)?;
    if !setter_pre_event(model, event_init, param) {
        return None;
    }
    let try_index = rest
        .iter()
        .position(|item| matches!(item_statement(item), Some(Statement::TryStatement(_))))?;
    if try_index != rest.len().checked_sub(2)? {
        return None;
    }
    let mut bare = Vec::new();
    let mut alias = None;
    for item in &rest[..try_index] {
        let Item::Decl(declarator) = item else {
            return None;
        };
        let declarator = *declarator;
        let declared_symbol = declared(model, declarator)?;
        match declarator.init.as_ref() {
            None => bare.push(declared_symbol),
            Some(init) if symbol(model, init) == Some(event) => {
                if alias.replace(declared_symbol).is_some() {
                    return None;
                }
            }
            Some(_) => return None,
        }
    }
    let Statement::TryStatement(attempt) = item_statement(&rest[try_index])? else {
        return None;
    };
    if attempt.handler.is_some() {
        return None;
    }
    let finalizer = attempt.finalizer.as_ref()?;
    let [invoke, completed] = attempt.block.body.as_slice() else {
        return None;
    };
    let Some(invoke) = simple_assignment(invoke) else {
        return None;
    };
    let Some(result) = target_symbol(model, &invoke.left) else {
        return None;
    };
    let values = exact_apply(model, &invoke.right, apply, 3)?;
    let [accessor, this_arg, param_arg] = values.as_slice() else {
        return None;
    };
    if symbol(model, accessor) != Some(original)
        || !is_this(this_arg)
        || symbol(model, param_arg) != Some(notify)
    {
        return None;
    }
    let event_target = alias.unwrap_or(event);
    let Some(completed) = simple_assignment(completed) else {
        return None;
    };
    if target_symbol(model, &completed.left) != Some(event_target)
        || !setter_post_event(model, &completed.right, param, result)
    {
        return None;
    }
    let final_items = flatten(&finalizer.body);
    let [notify_item, guard_item] = final_items.as_slice() else {
        return None;
    };
    let (after_event, after_init) = declared_init(model, notify_item)?;
    let after_call = call(after_init)?;
    let after = receiver(model, &after_call.callee, "notify")?;
    if after == notify {
        return None;
    }
    let values = arguments(after_call)?;
    let [event_arg] = values.as_slice() else {
        return None;
    };
    if symbol(model, event_arg) != Some(event_target) {
        return None;
    }
    let Some(Statement::IfStatement(guard)) = item_statement(guard_item) else {
        return None;
    };
    let Some((left, right)) = logical(&guard.test, "&&") else {
        return None;
    };
    if !((symbol(model, left) == Some(after_event)
        && bypass_null(model, right, after_event, Equality::NotEqual))
        || (symbol(model, right) == Some(after_event)
            && bypass_null(model, left, after_event, Equality::NotEqual)))
    {
        return None;
    }
    let Statement::BlockStatement(consequent) = &guard.consequent else {
        return None;
    };
    let [inner] = consequent.body.as_slice() else {
        return None;
    };
    let Statement::IfStatement(inner) = inner else {
        return None;
    };
    if !throw_or_return(model, inner, after_event) {
        return None;
    }
    let Some(Statement::BlockStatement(alternate)) = guard.alternate.as_ref() else {
        return None;
    };
    let [finish] = alternate.body.as_slice() else {
        return None;
    };
    let Some(finish) = simple_assignment(finish) else {
        return None;
    };
    let Some(out) = target_symbol(model, &finish.left) else {
        return None;
    };
    if symbol(model, &finish.right) != Some(result) {
        return None;
    }
    if bare.len() != 2 || result == out || !bare.contains(&result) || !bare.contains(&out) {
        return None;
    }
    let Some(Statement::ReturnStatement(returned)) = item_statement(rest.last()?) else {
        return None;
    };
    if returned.argument.as_ref().and_then(|e| symbol(model, e)) != Some(out) {
        return None;
    }
    Some((before, after))
}

fn setter_dispatch(
    model: &Model<'_, '_>,
    dispatch: &IfStatement<'_>,
    notify: Symbol,
    param: Symbol,
    apply: &Reference,
) -> bool {
    if symbol(model, &dispatch.test) != Some(notify) {
        return false;
    }
    let Statement::BlockStatement(consequent) = &dispatch.consequent else {
        return false;
    };
    let [inner] = consequent.body.as_slice() else {
        return false;
    };
    let Statement::IfStatement(inner) = inner else {
        return false;
    };
    if !bypass_null(model, &inner.test, notify, Equality::NotEqual) {
        return false;
    }
    let Statement::BlockStatement(inner_consequent) = &inner.consequent else {
        return false;
    };
    let [throw_or_return_item] = inner_consequent.body.as_slice() else {
        return false;
    };
    let Statement::IfStatement(throw_or_return_item) = throw_or_return_item else {
        return false;
    };
    if !throw_or_return(model, throw_or_return_item, notify) {
        return false;
    }
    let has_param = |test: &Expression<'_>| {
        exact_apply(model, test, apply, 3).is_some_and(|values| {
            matches!(values.as_slice(), [has_own, target, key]
                if object_has_own(model, has_own) && symbol(model, target) == Some(notify) && string(key, "param"))
        })
    };
    let assigns = |statement: &Statement<'_>, expected_param: bool| {
        simple_assignment(statement).is_some_and(|a| {
            target_symbol(model, &a.left) == Some(notify)
                && if expected_param {
                    member(&a.right, "param").and_then(|e| symbol(model, e)) == Some(notify)
                } else {
                    symbol(model, &a.right) == Some(param)
                }
        })
    };
    match inner.alternate.as_ref() {
        Some(Statement::BlockStatement(alternate)) => {
            let [rewrite] = alternate.body.as_slice() else {
                return false;
            };
            let Some(rewrite) = simple_assignment(rewrite) else {
                return false;
            };
            let Expression::ConditionalExpression(c) = rewrite.right.without_parentheses() else {
                return false;
            };
            if target_symbol(model, &rewrite.left) != Some(notify)
                || !has_param(&c.test)
                || member(&c.consequent, "param").and_then(|e| symbol(model, e)) != Some(notify)
                || symbol(model, &c.alternate) != Some(param)
            {
                return false;
            }
        }
        Some(Statement::IfStatement(rewrite)) => {
            if !has_param(&rewrite.test) {
                return false;
            }
            let Some(yes) = block(&rewrite.consequent) else {
                return false;
            };
            let Some(no) = rewrite.alternate.as_ref().and_then(block) else {
                return false;
            };
            if !matches!(yes.body.as_slice(), [s] if assigns(s, true))
                || !matches!(no.body.as_slice(), [s] if assigns(s, false))
            {
                return false;
            }
        }
        _ => return false,
    }
    let Some(Statement::BlockStatement(alternate)) = dispatch.alternate.as_ref() else {
        return false;
    };
    let [fallback] = alternate.body.as_slice() else {
        return false;
    };
    let Some(fallback) = simple_assignment(fallback) else {
        return false;
    };
    target_symbol(model, &fallback.left) == Some(notify)
        && symbol(model, &fallback.right) == Some(param)
}

fn setter_pre_event(model: &Model<'_, '_>, e: &Expression<'_>, param: Symbol) -> bool {
    let Some(fields) = object_of(e, &["param", "thisObj", "result", "threw"]) else {
        return false;
    };
    field(&fields, "param").is_some_and(|value| symbol(model, value) == Some(param))
        && field(&fields, "thisObj").is_some_and(is_this)
        && field(&fields, "result").is_some_and(is_null)
        && field(&fields, "threw").is_some_and(|value| is_boolean(value, true))
}

fn setter_post_event(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    param: Symbol,
    result: Symbol,
) -> bool {
    let Some(fields) = object_of(e, &["param", "thisObj", "result", "threw"]) else {
        return false;
    };
    field(&fields, "param").is_some_and(|value| symbol(model, value) == Some(param))
        && field(&fields, "thisObj").is_some_and(is_this)
        && field(&fields, "result").is_some_and(|value| symbol(model, value) == Some(result))
        && field(&fields, "threw").is_some_and(|value| is_boolean(value, false))
}

fn accessor_guard(model: &Model<'_, '_>, item: &Item<'_>, source: Symbol) -> bool {
    let Some(Statement::IfStatement(branch)) = item_statement(item) else {
        return false;
    };
    branch.alternate.is_none() && null_test(model, &branch.test, source, Equality::NotEqual)
}

fn get_block(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    buffer: Symbol,
    original: Symbol,
    descriptor: Symbol,
    hooks: Symbol,
    apply: &Reference,
    split_guard: bool,
) -> Option<Reference> {
    let Some(Statement::IfStatement(branch)) = item_statement(item) else {
        return None;
    };
    if branch.alternate.is_some() || !null_test(model, &branch.test, original, Equality::NotEqual) {
        return None;
    }
    let Statement::BlockStatement(consequent) = &branch.consequent else {
        return None;
    };
    let items = flatten(&consequent.body);
    let [install, before_item, after_item, getter_item, assign, tostring, prototype, hook_one, hook_two] =
        items.as_slice()
    else {
        return None;
    };
    // `buffer.get = original`
    let Some(install) = simple_assignment(item_statement(install)?) else {
        return None;
    };
    if target_member(&install.left, "get").and_then(|object| symbol(model, object)) != Some(buffer)
        || symbol(model, &install.right) != Some(original)
    {
        return None;
    }
    // `let before = factory(); let after = factory();`
    let (before, before_factory) = factory_binding(model, before_item)?;
    let (after, after_factory) = factory_binding(model, after_item)?;
    if before == after || before_factory != after_factory {
        return None;
    }
    // `function getter() { ... }`
    let (function, getter) = nested_function(getter_item)?;
    if model.binding_writes(getter) != 0 {
        return None;
    }
    let (proven_before, proven_after) = getter_body(model, function, original, apply, split_guard)?;
    if !((before == proven_before && after == proven_after)
        || (before == proven_after && after == proven_before))
    {
        return None;
    }
    // `descriptor.get = getter`
    let Some(assign) = simple_assignment(item_statement(assign)?) else {
        return None;
    };
    if target_member(&assign.left, "get").and_then(|object| symbol(model, object))
        != Some(descriptor)
        || symbol(model, &assign.right) != Some(getter)
    {
        return None;
    }
    if !typeof_guard(model, tostring, original, descriptor, "get") {
        return None;
    }
    if !prototype_guard(model, prototype, descriptor, original, "get", apply) {
        return None;
    }
    let hook = |item: &Item<'_>, name: &str, value: Symbol| {
        item_statement(item)
            .and_then(simple_assignment)
            .is_some_and(|assignment| {
                target_member(&assignment.left, name).and_then(|object| symbol(model, object))
                    == Some(hooks)
                    && symbol(model, &assignment.right) == Some(value)
            })
    };
    let direct =
        hook(hook_one, "onBeforeGet", proven_before) && hook(hook_two, "onAfterGet", proven_after);
    let swapped =
        hook(hook_one, "onAfterGet", proven_after) && hook(hook_two, "onBeforeGet", proven_before);
    (direct || swapped).then_some(before_factory)
}

fn set_block(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    buffer: Symbol,
    original: Symbol,
    descriptor: Symbol,
    hooks: Symbol,
    apply: &Reference,
) -> Option<Reference> {
    let Some(Statement::IfStatement(branch)) = item_statement(item) else {
        return None;
    };
    if branch.alternate.is_some() || !null_test(model, &branch.test, original, Equality::NotEqual) {
        return None;
    }
    let Statement::BlockStatement(consequent) = &branch.consequent else {
        return None;
    };
    let items = flatten(&consequent.body);
    let [install, before_item, after_item, setter_item, assign, tostring, prototype, hook_one, hook_two] =
        items.as_slice()
    else {
        return None;
    };
    let Some(install) = simple_assignment(item_statement(install)?) else {
        return None;
    };
    if target_member(&install.left, "set").and_then(|object| symbol(model, object)) != Some(buffer)
        || symbol(model, &install.right) != Some(original)
    {
        return None;
    }
    let (before, before_factory) = factory_binding(model, before_item)?;
    let (after, after_factory) = factory_binding(model, after_item)?;
    if before == after || before_factory != after_factory {
        return None;
    }
    let (function, setter) = nested_function(setter_item)?;
    if model.binding_writes(setter) != 0 {
        return None;
    }
    let (proven_before, proven_after) = setter_body(model, function, original, apply)?;
    if !((before == proven_before && after == proven_after)
        || (before == proven_after && after == proven_before))
    {
        return None;
    }
    let Some(assign) = simple_assignment(item_statement(assign)?) else {
        return None;
    };
    if target_member(&assign.left, "set").and_then(|object| symbol(model, object))
        != Some(descriptor)
        || symbol(model, &assign.right) != Some(setter)
    {
        return None;
    }
    if !typeof_guard(model, tostring, original, descriptor, "set") {
        return None;
    }
    if !prototype_guard(model, prototype, descriptor, original, "set", apply) {
        return None;
    }
    let hook = |item: &Item<'_>, name: &str, value: Symbol| {
        item_statement(item)
            .and_then(simple_assignment)
            .is_some_and(|assignment| {
                target_member(&assignment.left, name).and_then(|object| symbol(model, object))
                    == Some(hooks)
                    && symbol(model, &assignment.right) == Some(value)
            })
    };
    let direct =
        hook(hook_one, "onBeforeSet", proven_before) && hook(hook_two, "onAfterSet", proven_after);
    let swapped =
        hook(hook_one, "onAfterSet", proven_after) && hook(hook_two, "onBeforeSet", proven_before);
    (direct || swapped).then_some(before_factory)
}

fn record_literal(_model: &Model<'_, '_>, record: &Expression<'_>) -> bool {
    let Some(fields) = object_of(
        record,
        &[
            "onAfterGet",
            "onBeforeGet",
            "onAfterSet",
            "onBeforeSet",
            "originals",
        ],
    ) else {
        return false;
    };
    if ["onAfterGet", "onBeforeGet", "onAfterSet", "onBeforeSet"]
        .iter()
        .any(|key| !field(&fields, key).is_some_and(void_zero))
    {
        return false;
    }
    let Some(originals) = field(&fields, "originals") else {
        return false;
    };
    matches!(originals.without_parentheses(), Expression::ObjectExpression(object) if object.properties.is_empty())
}

fn block<'a>(statement: &'a Statement<'a>) -> Option<&'a BlockStatement<'a>> {
    let Statement::BlockStatement(block) = statement else {
        return None;
    };
    Some(block)
}

/// `for (;;) { .. }` and `while (true) { .. }` are the same unbounded scan.
fn infinite_loop<'a>(statement: &'a Statement<'a>) -> Option<&'a BlockStatement<'a>> {
    match statement {
        Statement::ForStatement(loop_)
            if loop_.init.is_none() && loop_.test.is_none() && loop_.update.is_none() =>
        {
            block(&loop_.body)
        }
        Statement::WhileStatement(loop_) if is_boolean(&loop_.test, true) => block(&loop_.body),
        _ => None,
    }
}

fn get_prototype_of(model: &Model<'_, '_>, e: &Expression<'_>, argument: Symbol) -> bool {
    let Some(call) = call(e) else {
        return false;
    };
    if !path_is(model, &call.callee, &["Object", "getPrototypeOf"]) {
        return false;
    }
    arguments(call).is_some_and(
        |values| matches!(values.as_slice(), [value] if symbol(model, value) == Some(argument)),
    )
}

fn record_object(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    cursor: Symbol,
    descriptor: Symbol,
) -> bool {
    let Some(fields) = object_of(e, &["containingObj", "desc"]) else {
        return false;
    };
    field(&fields, "containingObj").is_some_and(|value| symbol(model, value) == Some(cursor))
        && field(&fields, "desc").is_some_and(|value| symbol(model, value) == Some(descriptor))
}

/// The inline prototype walk: `cursor = object`, then an unbounded scan whose
/// null guard clears the record locals, whose descriptor miss advances the same
/// cursor through `Object.getPrototypeOf`, and whose hit packs exactly
/// `{ containingObj: cursor, desc }`. The returned set is every local proved to
/// alias that packed record.
fn inline_descriptor_lookup(
    model: &Model<'_, '_>,
    items: &[Item<'_>],
    object: Symbol,
    property: Symbol,
) -> Option<(BTreeSet<Symbol>, usize)> {
    let (cursor, init) = declared_init(model, items.first()?)?;
    if cursor == object || symbol(model, init) != Some(object) {
        return None;
    }
    let mut records = Vec::new();
    let mut index = 1;
    while let Some(item) = items.get(index) {
        let Some(record) = declared_bare(model, item) else {
            break;
        };
        if records.len() >= 2 || record == cursor || records.contains(&record) {
            return None;
        }
        records.push(record);
        index += 1;
    }
    if records.is_empty() {
        return None;
    }
    let body = infinite_loop(item_statement(items.get(index)?)?)?;
    let loop_items = flatten(&body.body);
    let mut position = 0;
    // Each record local starts every iteration as `void 0`.
    while let Some(item) = loop_items.get(position) {
        let Some(assignment) = item_statement(item).and_then(simple_assignment) else {
            break;
        };
        let Some(target) = target_symbol(model, &assignment.left) else {
            break;
        };
        if !records.contains(&target) || !void_zero(&assignment.right) {
            break;
        }
        position += 1;
    }
    // `if (cursor == null) { record = null; break; }`
    let Statement::IfStatement(guard) = item_statement(loop_items.get(position)?)? else {
        return None;
    };
    if guard.alternate.is_some() || !null_test(model, &guard.test, cursor, Equality::Equal) {
        return None;
    }
    let Some(guard_body) = block(&guard.consequent) else {
        return None;
    };
    let mut cleared = Vec::new();
    let mut saw_break = false;
    for statement in &guard_body.body {
        if let Some(assignment) = simple_assignment(statement) {
            let Some(target) = target_symbol(model, &assignment.left) else {
                return None;
            };
            if records.contains(&target) && is_null(&assignment.right) {
                if !cleared.contains(&target) {
                    cleared.push(target);
                }
                continue;
            }
            return None;
        }
        if let Statement::BreakStatement(branch) = statement {
            if branch.label.is_none() && !saw_break {
                saw_break = true;
                continue;
            }
        }
        return None;
    }
    if !saw_break || records.iter().any(|record| !cleared.contains(record)) {
        return None;
    }
    position += 1;
    // `let descriptor = Object.getOwnPropertyDescriptor(cursor, property);`
    let (descriptor, descriptor_init) = declared_init(model, loop_items.get(position)?)?;
    let Some(descriptor_call) = call(descriptor_init) else {
        return None;
    };
    if !path_is(
        model,
        &descriptor_call.callee,
        &["Object", "getOwnPropertyDescriptor"],
    ) {
        return None;
    }
    let Some(values) = arguments(descriptor_call) else {
        return None;
    };
    let [target, key] = values.as_slice() else {
        return None;
    };
    if symbol(model, *target) != Some(cursor) || symbol(model, *key) != Some(property) {
        return None;
    }
    position += 1;
    // `if (descriptor == null) { cursor = Object.getPrototypeOf(cursor); continue; }`
    let Statement::IfStatement(miss) = item_statement(loop_items.get(position)?)? else {
        return None;
    };
    if miss.alternate.is_some() || !null_test(model, &miss.test, descriptor, Equality::Equal) {
        return None;
    }
    let Some(miss_body) = block(&miss.consequent) else {
        return None;
    };
    let [step, next] = miss_body.body.as_slice() else {
        return None;
    };
    let Some(step) = simple_assignment(step) else {
        return None;
    };
    if target_symbol(model, &step.left) != Some(cursor)
        || !get_prototype_of(model, &step.right, cursor)
    {
        return None;
    }
    let Statement::ContinueStatement(next) = next else {
        return None;
    };
    if next.label.is_some() {
        return None;
    }
    position += 1;
    // The hit path packs the record and copies it into every record local.
    let mut literal = false;
    let mut aliases = BTreeSet::new();
    let mut saw_hit_break = false;
    while let Some(item) = loop_items.get(position) {
        if matches!(item_statement(item), Some(Statement::BreakStatement(branch)) if branch.label.is_none())
        {
            saw_hit_break = true;
            position += 1;
            break;
        }
        let Some(assignment) = item_statement(item).and_then(simple_assignment) else {
            return None;
        };
        let Some(target) = target_symbol(model, &assignment.left) else {
            return None;
        };
        // Packet construction and copies stay in the proved local frontier;
        // a foreign target would expose the descriptor as an extra effect.
        if target != cursor && !records.contains(&target) {
            return None;
        }
        if record_object(model, &assignment.right, cursor, descriptor) {
            if literal {
                return None;
            }
            literal = true;
        } else {
            let Some(source) = symbol(model, &assignment.right) else {
                return None;
            };
            if !aliases.contains(&source) {
                return None;
            }
        }
        aliases.insert(target);
        position += 1;
    }
    if !saw_hit_break || position != loop_items.len() || !literal {
        return None;
    }
    if records.iter().any(|record| !aliases.contains(record)) {
        return None;
    }
    Some((aliases, index + 1))
}

enum DescriptorLookup {
    Helper(Symbol),
    Inline(BTreeSet<Symbol>),
}

impl DescriptorLookup {
    fn guard_matches(&self, model: &Model<'_, '_>, test: &Expression<'_>) -> bool {
        match self {
            DescriptorLookup::Helper(prior) => null_test(model, test, *prior, Equality::Equal),
            DescriptorLookup::Inline(aliases) => aliases
                .iter()
                .any(|symbol_id| null_test(model, test, *symbol_id, Equality::Equal)),
        }
    }

    fn view(
        &self,
        model: &Model<'_, '_>,
        expression: &Expression<'_>,
        name: &str,
    ) -> Option<Symbol> {
        let value = member(expression, name).and_then(|object| symbol(model, object))?;
        match self {
            DescriptorLookup::Helper(prior) if value == *prior => Some(value),
            DescriptorLookup::Inline(aliases) if aliases.contains(&value) => Some(value),
            _ => None,
        }
    }
}

fn descriptor_lookup(
    model: &Model<'_, '_>,
    items: &[Item<'_>],
    object: Symbol,
    property: Symbol,
) -> Option<(DescriptorLookup, usize)> {
    if let Some(record) = lookup_record(model, items.first()?, object, property) {
        return Some((DescriptorLookup::Helper(record), 1));
    }
    let (aliases, consumed) = inline_descriptor_lookup(model, items, object, property)?;
    Some((DescriptorLookup::Inline(aliases), consumed))
}

fn simple_target_symbol(
    model: &Model<'_, '_>,
    target: &SimpleAssignmentTarget<'_>,
) -> Option<Symbol> {
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

fn bucket_access(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    property: Symbol,
) -> Option<Reference> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    if member.optional || symbol(model, &member.expression) != Some(property) {
        return None;
    }
    reference(model, &member.object)
}

fn bucket_null_test(
    model: &Model<'_, '_>,
    test: &Expression<'_>,
    property: Symbol,
) -> Option<Reference> {
    let (left, right) = binary_test(test, "==").or_else(|| binary_test(test, "==="))?;
    if is_null(right) {
        return bucket_access(model, left, property);
    }
    if is_null(left) {
        return bucket_access(model, right, property);
    }
    None
}

fn bucket_length(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    property: Symbol,
    registry: &Reference,
) -> bool {
    member(expression, "length")
        .and_then(|object| bucket_access(model, object, property))
        .is_some_and(|value| value == *registry)
}

fn bucket_bound(
    model: &Model<'_, '_>,
    test: &Expression<'_>,
    index: Symbol,
    property: Symbol,
    registry: &Reference,
) -> bool {
    if let Some((left, right)) = binary_test(test, ">=") {
        return symbol(model, left) == Some(index)
            && bucket_length(model, right, property, registry);
    }
    let Some((left, right)) = binary_test(test, "<=") else {
        return false;
    };
    symbol(model, right) == Some(index) && bucket_length(model, left, property, registry)
}

fn bucket_element(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    property: Symbol,
    registry: &Reference,
    index: Symbol,
) -> bool {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return false;
    };
    !member.optional
        && symbol(model, &member.expression) == Some(index)
        && bucket_access(model, &member.object, property).is_some_and(|value| value == *registry)
}

fn owner_object_mismatch(
    model: &Model<'_, '_>,
    test: &Expression<'_>,
    owner: Symbol,
    selected: Symbol,
) -> bool {
    let Some((left, right)) = binary_test(test, "!==").or_else(|| binary_test(test, "!=")) else {
        return false;
    };
    let selected_object = |expression: &Expression<'_>| {
        member(expression, "object").and_then(|object| symbol(model, object)) == Some(selected)
    };
    (symbol(model, left) == Some(owner) && selected_object(right))
        || (symbol(model, right) == Some(owner) && selected_object(left))
}

fn increment_index(model: &Model<'_, '_>, statement: &Statement<'_>, index: Symbol) -> bool {
    let Some(expression) = expression_statement(statement) else {
        return false;
    };
    let Expression::UpdateExpression(update) = expression.without_parentheses() else {
        return false;
    };
    update.operator.as_str() == "++" && simple_target_symbol(model, &update.argument) == Some(index)
}

fn derived_truth(model: &Model<'_, '_>, expression: &Expression<'_>, selected: Symbol) -> bool {
    null_test(model, expression, selected, Equality::NotEqual)
}

#[derive(Clone, Copy)]
enum PriorAlias {
    Copy,
    Truth,
}

struct InlinePrior {
    aliases: BTreeSet<Symbol>,
    derived: Option<(Symbol, PriorAlias)>,
}

/// The inline registry scan: `registry[property]` null selects an empty pair,
/// otherwise a bounded loop picks the bucket element whose `.object` is the
/// containing owner and keeps the selected record plus its derived result.
fn inline_registry_lookup(
    model: &Model<'_, '_>,
    items: &[Item<'_>],
    owner: Symbol,
    property: Symbol,
) -> Option<(InlinePrior, Reference, usize)> {
    let mut locals = Vec::new();
    let mut index = 0;
    while let Some(item) = items.get(index) {
        let Some(local) = declared_bare(model, item) else {
            break;
        };
        if locals.len() >= 2 || locals.contains(&local) {
            return None;
        }
        locals.push(local);
        index += 1;
    }
    if locals.is_empty() {
        return None;
    }
    let Statement::IfStatement(branch) = item_statement(items.get(index)?)? else {
        return None;
    };
    let registry = bucket_null_test(model, &branch.test, property)?;
    let Some(cleared) = block(&branch.consequent) else {
        return None;
    };
    let mut null_branch: BTreeMap<Symbol, bool> = BTreeMap::new();
    for statement in &cleared.body {
        let Some(assignment) = simple_assignment(statement) else {
            return None;
        };
        let Some(target) = target_symbol(model, &assignment.left) else {
            return None;
        };
        if !locals.contains(&target) {
            return None;
        }
        let is_null_value = is_null(&assignment.right);
        if !is_null_value && !is_boolean(&assignment.right, false) {
            return None;
        }
        if null_branch.insert(target, is_null_value).is_some() {
            return None;
        }
    }
    if null_branch.is_empty() {
        return None;
    }
    let Some(scan) = block(branch.alternate.as_ref()?) else {
        return None;
    };
    let scan_items = flatten(&scan.body);
    let (counter, counter_init) = declared_init(model, scan_items.first()?)?;
    if !number(counter_init, 0.0) {
        return None;
    }
    let body = infinite_loop(item_statement(scan_items.get(1)?)?)?;
    let loop_items = flatten(&body.body);
    let mut position = 0;
    while let Some(item) = loop_items.get(position) {
        let Some(assignment) = item_statement(item).and_then(simple_assignment) else {
            break;
        };
        let Some(target) = target_symbol(model, &assignment.left) else {
            break;
        };
        if !locals.contains(&target) || !void_zero(&assignment.right) {
            break;
        }
        position += 1;
    }
    // `if (index >= registry[property].length) { selected = null; break; }`
    let Statement::IfStatement(bounds) = item_statement(loop_items.get(position)?)? else {
        return None;
    };
    if bounds.alternate.is_some()
        || !bucket_bound(model, &bounds.test, counter, property, &registry)
    {
        return None;
    }
    let Some(bounds_body) = block(&bounds.consequent) else {
        return None;
    };
    // Select the record by the subsequent bucket load, rather than assuming
    // that its null clear is the first assignment. A distinct derived flag
    // may be cleared to false before the record on the bounds exit.
    let (break_item, clears) = bounds_body.body.split_last()?;
    if clears.is_empty() || clears.len() > 2 {
        return None;
    }
    let element = item_statement(loop_items.get(position + 1)?).and_then(simple_assignment)?;
    let selected = target_symbol(model, &element.left)?;
    if !locals.contains(&selected) {
        return None;
    }
    let mut record_cleared = false;
    let mut clear_alias = None;
    for statement in clears {
        let assignment = simple_assignment(statement)?;
        let target = target_symbol(model, &assignment.left)?;
        if target == selected {
            if record_cleared || !is_null(&assignment.right) {
                return None;
            }
            record_cleared = true;
        } else {
            if !locals.contains(&target) || clear_alias.is_some() {
                return None;
            }
            let kind = if is_null(&assignment.right) {
                PriorAlias::Copy
            } else if is_boolean(&assignment.right, false) {
                PriorAlias::Truth
            } else {
                return None;
            };
            clear_alias = Some((target, kind));
        }
    }
    if !record_cleared {
        return None;
    }
    let Statement::BreakStatement(break_item) = break_item else {
        return None;
    };
    if break_item.label.is_some() {
        return None;
    }
    position += 1;
    // `selected = registry[property][index];`
    let Some(element) = item_statement(loop_items.get(position)?).and_then(simple_assignment)
    else {
        return None;
    };
    if target_symbol(model, &element.left) != Some(selected)
        || !bucket_element(model, &element.right, property, &registry, counter)
    {
        return None;
    }
    position += 1;
    // `if (owner !== selected.object) { ++index; continue; }`
    let Statement::IfStatement(mismatch) = item_statement(loop_items.get(position)?)? else {
        return None;
    };
    if mismatch.alternate.is_some()
        || !owner_object_mismatch(model, &mismatch.test, owner, selected)
    {
        return None;
    }
    let Some(mismatch_body) = block(&mismatch.consequent) else {
        return None;
    };
    let [advance, continue_item] = mismatch_body.body.as_slice() else {
        return None;
    };
    if !increment_index(model, advance, counter) {
        return None;
    }
    let Statement::ContinueStatement(continue_item) = continue_item else {
        return None;
    };
    if continue_item.label.is_some() {
        return None;
    }
    position += 1;
    let hit_alias = if let Some(assignment) = loop_items
        .get(position)
        .and_then(item_statement)
        .and_then(simple_assignment)
    {
        let alias = target_symbol(model, &assignment.left)?;
        let (cleared, kind) = clear_alias?;
        if alias != cleared
            || !match kind {
                PriorAlias::Copy => symbol(model, &assignment.right) == Some(selected),
                PriorAlias::Truth => derived_truth(model, &assignment.right, selected),
            }
        {
            return None;
        }
        position += 1;
        Some((alias, kind))
    } else {
        if clear_alias.is_some() {
            return None;
        }
        None
    };
    let Statement::BreakStatement(hit) = item_statement(loop_items.get(position)?)? else {
        return None;
    };
    if hit.label.is_some() {
        return None;
    }
    position += 1;
    if position != loop_items.len() {
        return None;
    }
    let mut aliases = BTreeSet::new();
    aliases.insert(selected);
    let mut derived = hit_alias;
    if let Some((alias, PriorAlias::Copy)) = hit_alias {
        aliases.insert(alias);
    }
    match scan_items.get(2) {
        None => {
            if scan_items.len() != 2 || locals.len() != if hit_alias.is_some() { 2 } else { 1 } {
                return None;
            }
        }
        Some(item) => {
            if hit_alias.is_some() || scan_items.len() != 3 || locals.len() != 2 {
                return None;
            }
            let Some(assignment) = item_statement(item).and_then(simple_assignment) else {
                return None;
            };
            let Some(alias) = target_symbol(model, &assignment.left) else {
                return None;
            };
            if alias == selected || !locals.contains(&alias) {
                return None;
            }
            if symbol(model, &assignment.right) == Some(selected) {
                aliases.insert(alias);
                derived = Some((alias, PriorAlias::Copy));
            } else if derived_truth(model, &assignment.right, selected) {
                derived = Some((alias, PriorAlias::Truth));
            } else {
                return None;
            }
        }
    }
    if null_branch.len() != locals.len() || null_branch.get(&selected) != Some(&true) {
        return None;
    }
    match derived {
        None => {}
        Some((alias, PriorAlias::Copy)) => {
            if null_branch.get(&alias) != Some(&true) {
                return None;
            }
        }
        Some((alias, PriorAlias::Truth)) => {
            if null_branch.get(&alias) != Some(&false) {
                return None;
            }
        }
    }
    Some((InlinePrior { aliases, derived }, registry, index + 1))
}

enum PriorLookup {
    Helper {
        prior: Symbol,
    },
    Inline {
        prior: InlinePrior,
        registry: Reference,
    },
}

impl PriorLookup {
    fn record_symbols(&self) -> BTreeSet<Symbol> {
        match self {
            PriorLookup::Helper { prior } => {
                let mut symbols = BTreeSet::new();
                symbols.insert(*prior);
                symbols
            }
            PriorLookup::Inline { prior, .. } => prior.aliases.clone(),
        }
    }
}

fn prior_lookup(
    model: &Model<'_, '_>,
    items: &[Item<'_>],
    owner: Symbol,
    property: Symbol,
) -> Option<(PriorLookup, usize)> {
    if let Some(prior) = registry_lookup(model, items.first()?, owner, property) {
        return Some((PriorLookup::Helper { prior }, 1));
    }
    let (prior, registry, consumed) = inline_registry_lookup(model, items, owner, property)?;
    Some((PriorLookup::Inline { prior, registry }, consumed))
}

fn prior_guard_test(model: &Model<'_, '_>, test: &Expression<'_>, prior: &PriorLookup) -> bool {
    match prior {
        PriorLookup::Helper { prior } => null_test(model, test, *prior, Equality::NotEqual),
        PriorLookup::Inline { prior, .. } => {
            if let Some((derived, PriorAlias::Truth)) = &prior.derived {
                if symbol(model, test) == Some(*derived) {
                    return true;
                }
            }
            prior
                .aliases
                .iter()
                .any(|symbol_id| null_test(model, test, *symbol_id, Equality::NotEqual))
        }
    }
}

fn registry_lookup(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    owner: Symbol,
    property: Symbol,
) -> Option<Symbol> {
    let (prior, init) = declared_init(model, item)?;
    let call = call(init)?;
    if !matches!(call.callee.without_parentheses(), Expression::Identifier(_)) {
        return None;
    }
    let values = arguments(call)?;
    let [owner_arg, property_arg] = values.as_slice() else {
        return None;
    };
    (symbol(model, owner_arg) == Some(owner) && symbol(model, property_arg) == Some(property))
        .then_some(prior)
}

fn prior_guard(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    prior: &PriorLookup,
    getter: Symbol,
    setter: Symbol,
) -> bool {
    let Some(Statement::IfStatement(branch)) = item_statement(item) else {
        return false;
    };
    if branch.alternate.is_some() || !prior_guard_test(model, &branch.test, prior) {
        return false;
    }
    let aliases = prior.record_symbols();
    let Statement::BlockStatement(consequent) = &branch.consequent else {
        return false;
    };
    let items = flatten(&consequent.body);
    let [exists, first, second] = items.as_slice() else {
        return false;
    };
    // `if (record.result != null) { return record.result; }`
    let Some(Statement::IfStatement(exists)) = item_statement(exists) else {
        return false;
    };
    if exists.alternate.is_some() {
        return false;
    }
    let Some(result_record) = aliases.iter().copied().find(|record| {
        member_null_test(model, &exists.test, *record, "result", Equality::NotEqual)
    }) else {
        return false;
    };
    let Statement::BlockStatement(exists_consequent) = &exists.consequent else {
        return false;
    };
    let [returned] = exists_consequent.body.as_slice() else {
        return false;
    };
    let Statement::ReturnStatement(returned) = returned else {
        return false;
    };
    if returned
        .argument
        .as_ref()
        .and_then(|argument| member(argument, "result"))
        .and_then(|object| symbol(model, object))
        != Some(result_record)
    {
        return false;
    }
    // `getter = record.original.get; setter = record.original.set;` in either order.
    let copy = |item: &Item<'_>, name: &str, target: Symbol| {
        item_statement(item)
            .and_then(simple_assignment)
            .is_some_and(|assignment| {
                target_symbol(model, &assignment.left) == Some(target)
                    && member(&assignment.right, name)
                        .and_then(|object| member(object, "original"))
                        .and_then(|object| symbol(model, object))
                        .is_some_and(|record| aliases.contains(&record))
            })
    };
    (copy(first, "get", getter) && copy(second, "set", setter))
        || (copy(first, "set", setter) && copy(second, "get", getter))
}

fn defines_property(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    owner: Symbol,
    property: Symbol,
    record: Symbol,
) -> bool {
    let Some(expression) = item_statement(item).and_then(expression_statement) else {
        return false;
    };
    let Some(call) = call(expression) else {
        return false;
    };
    if !path_is(model, &call.callee, &["Object", "defineProperty"]) {
        return false;
    }
    let Some(values) = arguments(call) else {
        return false;
    };
    let [object, key, descriptor] = values.as_slice() else {
        return false;
    };
    symbol(model, object) == Some(owner)
        && symbol(model, key) == Some(property)
        && symbol(model, descriptor) == Some(record)
}

fn sets_originals(model: &Model<'_, '_>, item: &Item<'_>, record: Symbol, buffer: Symbol) -> bool {
    item_statement(item)
        .and_then(simple_assignment)
        .is_some_and(|assignment| {
            target_member(&assignment.left, "originals").and_then(|object| symbol(model, object))
                == Some(record)
                && symbol(model, &assignment.right) == Some(buffer)
        })
}

fn registration_object(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    owner: Symbol,
    record: Symbol,
    buffer: Symbol,
) -> bool {
    let Some(fields) = object_of(e, &["object", "result", "original"]) else {
        return false;
    };
    field(&fields, "object").is_some_and(|value| symbol(model, value) == Some(owner))
        && field(&fields, "result").is_some_and(|value| symbol(model, value) == Some(record))
        && field(&fields, "original").is_some_and(|value| symbol(model, value) == Some(buffer))
}

fn registry_array(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    property: Symbol,
) -> Option<(Symbol, Reference)> {
    let (array, init) = declared_init(model, item)?;
    let Expression::ComputedMemberExpression(member) = init.without_parentheses() else {
        return None;
    };
    if member.optional || symbol(model, &member.expression) != Some(property) {
        return None;
    }
    Some((array, reference(model, &member.object)?))
}

fn registry_array_init(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    array: Symbol,
    registry: &Reference,
    property: Symbol,
) -> bool {
    let Some(Statement::IfStatement(branch)) = item_statement(item) else {
        return false;
    };
    if branch.alternate.is_some() || !null_test(model, &branch.test, array, Equality::Equal) {
        return false;
    }
    let Statement::BlockStatement(consequent) = &branch.consequent else {
        return false;
    };
    let [reset, store] = consequent.body.as_slice() else {
        return false;
    };
    let Some(reset) = simple_assignment(reset) else {
        return false;
    };
    if target_symbol(model, &reset.left) != Some(array) {
        return false;
    }
    let Expression::ArrayExpression(empty) = reset.right.without_parentheses() else {
        return false;
    };
    if !empty.elements.is_empty() {
        return false;
    }
    let Some(store) = simple_assignment(store) else {
        return false;
    };
    let AssignmentTarget::ComputedMemberExpression(member) = &store.left else {
        return false;
    };
    !member.optional
        && symbol(model, &member.expression) == Some(property)
        && reference(model, &member.object).is_some_and(|value| value == *registry)
        && symbol(model, &store.right) == Some(array)
}

fn registration_tail(
    model: &Model<'_, '_>,
    tail: &[Item<'_>],
    owner: Symbol,
    record: Symbol,
    buffer: Symbol,
    apply: &Reference,
    property: Symbol,
) -> Option<Reference> {
    let length = tail.len();
    if length < 3 {
        return None;
    }
    let (head, job) = tail.split_at(length - 3);
    let [array_item, array_guard, push_item] = job else {
        return None;
    };
    let (array, registry) = registry_array(model, array_item, property)?;
    if !registry_array_init(model, array_guard, array, &registry, property) {
        return None;
    }
    let expression = item_statement(push_item).and_then(expression_statement)?;
    let values = exact_apply(model, expression, apply, 3)?;
    let [push_target, pushed_array, pushed_record] = values.as_slice() else {
        return None;
    };
    if !path_is(model, push_target, &["Array", "prototype", "push"])
        || symbol(model, pushed_array) != Some(array)
    {
        return None;
    }
    match head {
        [] => registration_object(model, pushed_record, owner, record, buffer).then_some(registry),
        [full] => {
            if symbol(model, pushed_record) != Some(owner) {
                return None;
            }
            let assignment = simple_assignment(item_statement(full)?)?;
            if target_symbol(model, &assignment.left) != Some(owner) {
                return None;
            }
            registration_object(model, &assignment.right, owner, record, buffer).then_some(registry)
        }
        [object_only, first, second] => {
            if symbol(model, pushed_record) != Some(owner) {
                return None;
            }
            let assignment = simple_assignment(item_statement(object_only)?)?;
            if target_symbol(model, &assignment.left) != Some(owner) {
                return None;
            }
            let fields = object_of(&assignment.right, &["object"])?;
            if !field(&fields, "object").is_some_and(|value| symbol(model, value) == Some(owner)) {
                return None;
            }
            let writes = |item: &Item<'_>, name: &str, value: Symbol| {
                item_statement(item)
                    .and_then(simple_assignment)
                    .is_some_and(|assignment| {
                        target_member(&assignment.left, name)
                            .and_then(|object| symbol(model, object))
                            == Some(owner)
                            && symbol(model, &assignment.right) == Some(value)
                    })
            };
            let direct = writes(first, "result", record) && writes(second, "original", buffer);
            let swapped = writes(first, "original", buffer) && writes(second, "result", record);
            (direct || swapped).then_some(registry)
        }
        _ => None,
    }
}

fn early_branch(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    buffer: Symbol,
    getter: Symbol,
    setter: Symbol,
) -> Option<()> {
    let items = flatten(statements);
    let length = items.len();
    let Some(Statement::ReturnStatement(returned)) = item_statement(items.last()?) else {
        return None;
    };
    if returned.argument.as_ref().and_then(|e| symbol(model, e)) != Some(buffer) {
        return None;
    }
    let assign_item = items.get(length.checked_sub(2)?)?;
    let assignment = simple_assignment(item_statement(assign_item)?)?;
    if target_symbol(model, &assignment.left) != Some(buffer) {
        return None;
    }
    let fields = object_of(&assignment.right, &["originals"])?;
    if !field(&fields, "originals").is_some_and(|value| symbol(model, value) == Some(buffer)) {
        return None;
    }
    let copies = &items[..length.checked_sub(2)?];
    if copies.len() != 2 {
        return None;
    }
    let get_copy = |item: &Item<'_>| accessor_copy(model, item, buffer, getter, "get");
    let set_copy = |item: &Item<'_>| accessor_copy(model, item, buffer, setter, "set");
    ((get_copy(&copies[0]) && set_copy(&copies[1]))
        || (set_copy(&copies[0]) && get_copy(&copies[1])))
    .then_some(())
}

fn disabled(model: &Model<'_, '_>, test: &Expression<'_>, configurable: Symbol) -> bool {
    let Some((left, right)) = binary_test(test, "===").or_else(|| binary_test(test, "==")) else {
        return false;
    };
    (symbol(model, left) == Some(configurable) && is_boolean(right, false))
        || (symbol(model, right) == Some(configurable) && is_boolean(left, false))
}

fn interception_condition(
    model: &Model<'_, '_>,
    test: &Expression<'_>,
    configurable: Symbol,
    data_property: Symbol,
) -> bool {
    let Some((left, right)) = logical(test, "||") else {
        return false;
    };
    (disabled(model, left, configurable) && symbol(model, right) == Some(data_property))
        || (disabled(model, right, configurable) && symbol(model, left) == Some(data_property))
}

fn late_branch(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    owner: Symbol,
    property: Symbol,
    descriptor: Symbol,
    getter: Symbol,
    setter: Symbol,
    buffer: Symbol,
    apply: &Reference,
    split_guard: bool,
    outer: Symbol,
) -> Option<bool> {
    let items = flatten(statements);
    if items.len() < 8 {
        return None;
    }
    let (prior, consumed) = prior_lookup(model, &items, owner, property)?;
    // The additive variant proves its own keyed scan; an opaque helper call
    // cannot establish that the cached record belongs to this containing owner.
    let captured_registry = if split_guard && matches!(prior, PriorLookup::Helper { .. }) {
        Some(captured::registry(model, items.first()?, outer)?)
    } else {
        None
    };
    if !prior_guard(model, items.get(consumed)?, &prior, getter, setter) {
        return None;
    }
    let hooks_index = consumed + 1;
    let (hooks, hooks_init) = declared_init(model, items.get(hooks_index)?)?;
    if hooks == descriptor || hooks == buffer || !record_literal(model, hooks_init) {
        return None;
    }
    let get_index = (hooks_index + 1..=hooks_index + 2).find(|index| {
        items
            .get(*index)
            .is_some_and(|item| accessor_guard(model, item, getter))
    })?;
    let set_index = (hooks_index + 1..=hooks_index + 2).find(|index| {
        items
            .get(*index)
            .is_some_and(|item| accessor_guard(model, item, setter))
    })?;
    if get_index == set_index {
        return None;
    }
    if split_guard {
        let get_bookkeeping = closed_bookkeeping(model, items.get(get_index)?)?;
        let set_bookkeeping = closed_bookkeeping(model, items.get(set_index)?)?;
        if get_bookkeeping != set_bookkeeping {
            return None;
        }
    }
    let get_factory = get_block(
        model,
        items.get(get_index)?,
        buffer,
        getter,
        descriptor,
        hooks,
        apply,
        split_guard,
    )?;
    let set_factory = set_block(
        model,
        items.get(set_index)?,
        buffer,
        setter,
        descriptor,
        hooks,
        apply,
    )?;
    if get_factory != set_factory {
        return None;
    }
    let define_index = hooks_index + 3;
    let originals_index = hooks_index + 4;
    if !defines_property(model, items.get(define_index)?, owner, property, descriptor) {
        return None;
    }
    if !sets_originals(model, items.get(originals_index)?, hooks, buffer) {
        return None;
    }
    let last_index = items.len().checked_sub(1)?;
    let Some(Statement::ReturnStatement(returned)) = item_statement(items.get(last_index)?) else {
        return None;
    };
    if returned.argument.as_ref().and_then(|e| symbol(model, e)) != Some(hooks) {
        return None;
    }
    let registered = registration_tail(
        model,
        items.get(originals_index + 1..last_index)?,
        owner,
        hooks,
        buffer,
        apply,
        property,
    )?;
    if let PriorLookup::Inline { registry, .. } = &prior {
        if registered != *registry {
            return None;
        }
    }
    if captured_registry
        .as_ref()
        .is_some_and(|registry| registry != &registered)
    {
        return None;
    }
    Some(captured_registry.is_some())
}

/// For the new closed variant only, both accessors must save source strings
/// and published wrappers to the same distinct buffers, using the exact saved
/// toString receiver installed by the shim guard. This proves local identity,
/// not the native origin or external purpose of any captured value.
fn closed_bookkeeping(
    model: &Model<'_, '_>,
    item: &Item<'_>,
) -> Option<(Reference, Reference, Reference)> {
    let Statement::IfStatement(accessor) = item_statement(item)? else {
        return None;
    };
    let Statement::BlockStatement(accessor) = &accessor.consequent else {
        return None;
    };
    let items = flatten(&accessor.body);
    let Statement::IfStatement(guard) = item_statement(items.get(5)?)? else {
        return None;
    };
    binary_test(&guard.test, "===")?;
    let Statement::BlockStatement(guard) = &guard.consequent else {
        return None;
    };
    let [Statement::TryStatement(attempt)] = guard.body.as_slice() else {
        return None;
    };
    let saved = flatten(&attempt.block.body);
    let (_, init) = declared_init(model, saved.first()?)?;
    let source = member(member(&call(init)?.callee, "call")?, "toString")?;
    let source = reference(model, source)?;
    let strings = reference(model, member(&push_call(saved.get(1)?)?.callee, "push")?)?;
    let wrappers = reference(model, member(&push_call(saved.get(2)?)?.callee, "push")?)?;
    if strings == wrappers {
        return None;
    }
    let Statement::IfStatement(shim) = item_statement(saved.get(3)?)? else {
        return None;
    };
    let (_, shim_value) = binary_test(&shim.test, "!==")?;
    if reference(model, member(shim_value, "toString")?)? != source {
        return None;
    }
    Some((source, strings, wrappers))
}

fn lookup_record(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    object: Symbol,
    property: Symbol,
) -> Option<Symbol> {
    let (record, init) = declared_init(model, item)?;
    let call = call(init)?;
    if !matches!(call.callee.without_parentheses(), Expression::Identifier(_)) {
        return None;
    }
    let values = arguments(call)?;
    let [object_arg, property_arg] = values.as_slice() else {
        return None;
    };
    (symbol(model, object_arg) == Some(object) && symbol(model, property_arg) == Some(property))
        .then_some(record)
}

fn returns_null(item: &Item<'_>) -> bool {
    let Some(Statement::IfStatement(branch)) = item_statement(item) else {
        return false;
    };
    branch.alternate.is_none()
        && matches!(
            &branch.consequent,
            Statement::BlockStatement(block)
                if matches!(
                    block.body.as_slice(),
                    [Statement::ReturnStatement(returned)]
                        if returned.argument.as_ref().is_some_and(is_null)
                )
        )
}

fn desc_reassignment(model: &Model<'_, '_>, item: &Item<'_>, record: Symbol) -> bool {
    item_statement(item)
        .and_then(simple_assignment)
        .is_some_and(|assignment| {
            target_symbol(model, &assignment.left) == Some(record)
                && member(&assignment.right, "desc").and_then(|object| symbol(model, object))
                    == Some(record)
        })
}

fn member_init(
    model: &Model<'_, '_>,
    item: &Item<'_>,
    record: Symbol,
    name: &str,
) -> Option<Symbol> {
    let (declared_symbol, init) = declared_init(model, item)?;
    (member(init, name).and_then(|object| symbol(model, object)) == Some(record))
        .then_some(declared_symbol)
}

/// The apply helper: `apply(Object.hasOwnProperty, record, "value")`.
fn apply_helper(model: &Model<'_, '_>, init: &Expression<'_>, record: Symbol) -> Option<Reference> {
    let call = call(init)?;
    let apply = reference(model, &call.callee)?;
    let values = arguments(call)?;
    let [has_own, target, key] = values.as_slice() else {
        return None;
    };
    if !object_has_own(model, has_own)
        || symbol(model, target) != Some(record)
        || !string(key, "value")
    {
        return None;
    }
    Some(apply)
}

fn object_create_null(model: &Model<'_, '_>, init: &Expression<'_>) -> bool {
    let Some(call) = call(init) else {
        return false;
    };
    if !path_is(model, &call.callee, &["Object", "create"]) {
        return false;
    }
    let Some(values) = arguments(call) else {
        return false;
    };
    matches!(values.as_slice(), [value] if is_null(value))
}

fn accessor_interceptor(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
    split_guard: bool,
) -> Option<bool> {
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return None;
    }
    let [object_parameter, property_parameter] = function.params.items.as_slice() else {
        return None;
    };
    if object_parameter.initializer.is_some() || property_parameter.initializer.is_some() {
        return None;
    }
    let object = plain_parameter(model, &object_parameter.pattern)?;
    let property = plain_parameter(model, &property_parameter.pattern)?;
    if object == property {
        return None;
    }
    let items = flatten(&body.statements);
    let (lookup, consumed) = descriptor_lookup(model, &items, object, property)?;
    if split_guard && lookup_record(model, items.first()?, object, property).is_some() {
        captured::descriptor(
            model,
            items.first()?,
            function.id.as_ref()?.symbol_id.get()?,
        )?;
    }
    let rest = items.get(consumed..)?;
    let [guard, owner_item, desc, configurable_item, getter_item, setter_item, data_item, buffer_item, branch_item] =
        rest
    else {
        return None;
    };
    // `if (record == null) return null;`
    if !returns_null(guard) || !lookup.guard_matches(model, branch_test(guard)?) {
        return None;
    }
    // `let owner = record.containingObj;`
    let (owner, owner_init) = declared_init(model, owner_item)?;
    let record = lookup.view(model, owner_init, "containingObj")?;
    // `record = record.desc;`
    if !desc_reassignment(model, desc, record) {
        return None;
    }
    // descriptor views
    let configurable = member_init(model, configurable_item, record, "configurable")?;
    let getter = member_init(model, getter_item, record, "get")?;
    let setter = member_init(model, setter_item, record, "set")?;
    if getter == setter {
        return None;
    }
    let (data_property, data_init) = declared_init(model, data_item)?;
    let apply = apply_helper(model, data_init, record)?;
    let (buffer, buffer_init) = declared_init(model, buffer_item)?;
    if !object_create_null(model, buffer_init) {
        return None;
    }
    // dispatch
    let Some(Statement::IfStatement(branch)) = item_statement(branch_item) else {
        return None;
    };
    if !interception_condition(model, &branch.test, configurable, data_property) {
        return None;
    }
    let Some(alternate) = branch.alternate.as_ref() else {
        return None;
    };
    let Statement::BlockStatement(early) = &branch.consequent else {
        return None;
    };
    let Statement::BlockStatement(late) = alternate else {
        return None;
    };
    early_branch(model, &early.body, buffer, getter, setter)?;
    let captured_registry = late_branch(
        model,
        &late.body,
        owner,
        property,
        record,
        getter,
        setter,
        buffer,
        &apply,
        split_guard,
        function.id.as_ref()?.symbol_id.get()?,
    )?;
    Some(captured_registry || matches!(lookup, DescriptorLookup::Helper(_)))
}

/// The `if` test of the null guard. Kept separate so the outer proof reads in
/// source order.
fn branch_test<'a>(item: &Item<'a>) -> Option<&'a Expression<'a>> {
    let Statement::IfStatement(branch) = item_statement(item)? else {
        return None;
    };
    Some(&branch.test)
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
    if accessor_interceptor(model, function, body, false).is_none() {
        if let Some(captured) = accessor_interceptor(model, function, body, true) {
            if captured {
                return vec![proposal(
                    "installAccessorHookProtocol",
                    "owned_captured_accessor_hook_protocol",
                    "whole owned accessor call protocol with independently source-proved captured descriptor walk and/or keyed same-owner registry scan, scoped single-origin helper bindings, initialization and exposure checks; exact getter/setter identity, this and parameter forwarding, before/after notification wiring, finally/bypass paths, source/prototype bookkeeping and registration to the same registry. Apply/factory/native origins remain explicitly unproved; the neutral name describes the owned call protocol only",
                    f,
                )];
            }
            return vec![proposal(
                "installAccessorHookProtocol",
                "owned_split_accessor_hook_protocol",
                "whole owned inline descriptor walk and same-owner keyed registry scan; exact descriptor refusal, separate getter/setter publication and originals registration; split getter bypass guard, paired before/after notifications, exact this/argument forwarding and finally/bypass throw/value paths; saved source and prototype bookkeeping. External apply/factory/toString/array capture purposes and native origins are explicitly unproved; this neutral name describes only the owned call protocol",
                f,
            )];
        }
        return Vec::new();
    }
    vec![proposal(
        "installAccessorInterceptor",
        "posthost_accessor_hooks",
        "whole owned body: descriptor lookup with a null guard, containingObj/desc views, configurable-or-data dispatch, an Object.create(null) original descriptor, the four onBefore/onAfter get/set hook slots wired to paired factory observers, independently re-proved strict getter/setter wrappers that apply the original accessor through one opaque helper and honour bypassResult, Object.defineProperty on the unshadowed global Object, and registration through Array.prototype.push; local, helper and hook-property spellings are not role evidence, and the owned getter/setter callbacks are named by their own family",
        f,
    )]
}
