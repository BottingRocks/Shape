//! Bounded owned-body proof for the `_selectMeta` accessor-probe setter.
//!
//! This is deliberately *not* the `MutationRecord` DIV/STYLE inspector and not
//! the `addedNodes` iterator callback it directly owns. It is the plain
//! one-formal body that an enclosing scope installs as the `set` of a
//! `_selectMeta` accessor descriptor:
//!
//! ```text
//! function setter(input) {
//!   storedSlot = input;                       // defuse the argument
//!   if (input == null) {                      // null guard
//!     return void 0;
//!   } else {
//!     let keys = Object.keys(input);          // own enumerable names
//!     keys.sort();                            // ordered in place
//!     // Either the direct join form below, or the bounded holder form:
//!     // keys = [keys, keys.join];
//!     // if (<consumer>(keys[1].call(keys[0], ";")) === <number>) {
//!     if (<consumer>(keys.join(";")) === <number>) {
//!       probeFlag = true;
//!       <observer>.next({
//!         selectMeta: true,
//!         animationStart: <captured>,
//!         measurementDiv: <captured>,
//!         animationStyle: <captured>
//!       });
//!     }
//!     return void 0;
//!   }
//! }
//! // ... registration that follows the function in the same scope:
//! let descriptor = { get: void 0, set: void 0 };
//! function getter() { return storedSlot; }
//! descriptor.get = getter;
//! descriptor.set = setter;                    // scope-resolved set -> this function
//! Object.defineProperty(target, "_selectMeta", descriptor);
//! ```
//!
//! The setter is named only when its own body defuses exactly one plain formal
//! into one captured slot, null-guards that same formal, builds the sorted
//! `Object.keys` array, and uses one of two bounded digest shapes: the direct
//! `keys.join(";")` form, or the explicit RHS-before-self-overwrite
//! `keys = [keys, keys.join]` followed by `keys[1].call(keys[0], ";")`. It then
//! compares that opaque consumer against a numeric literal, flips one captured
//! flag to `true` and hands exactly the four-field
//! `selectMeta`/`animationStart`/`measurementDiv`/`animationStyle` object to
//! one captured `.next` receiver whose three state fields are distinct
//! captured slots.
//!
//! The registration is re-proved from the same scope index, never from a name,
//! a span, a vendor or a digest dictionary: exactly one assignment
//! `descriptor.set = <this function>` resolves to the claimed site, the paired
//! `descriptor.get` is a function whose whole body returns the same captured
//! slot, and exactly one `Object.defineProperty(<target>, "_selectMeta",
//! descriptor)` on the unshadowed global `Object` consumes that descriptor.
//!
//! `Object.keys`, the digest consumer and the payload sink are the only opaque
//! protocol inputs; the method spellings `sort`, `join`, `call`, `next` and the
//! protocol literals `_selectMeta`, `";"` and the four payload keys are fixed
//! source operations, not evidence about a particular capture. Local spellings,
//! offsets and hashes are not evidence. Statement order is used only to prove
//! reaching registration values in one enclosing execution region. The older
//! `style_inspector` zero-formal guard and its bounded `addedNodes` iterator
//! dependency are untouched; this rule runs only in the final fallback stage,
//! after every earlier source-backed evaluator has declined the function.
use super::*;
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

const NAME: &str = "setSelectMetaProbeValue";
const EVALUATOR: &str = "thread_posthost_select_meta_setter";
const SELECT_META: &str = "_selectMeta";

// ===========================================================================
// Scoped expression and statement readers
// ===========================================================================

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn member<'a>(expression: &'a Expression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn computed<'a>(
    expression: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional).then_some((&member.object, &member.expression))
}

fn call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return None;
    };
    (!call.optional).then_some(call)
}

fn method_receiver(model: &Model<'_, '_>, call: &CallExpression<'_>, name: &str) -> Option<Symbol> {
    let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == name).then(|| symbol(model, &member.object))?
}

fn number(expression: &Expression<'_>, expected: f64) -> bool {
    matches!(expression.without_parentheses(), Expression::NumericLiteral(value) if value.value == expected)
}

/// A numeric literal, including the sign a source writes in front of one. The
/// digest consumer and its compared value are opaque protocol data: only that
/// the comparison tests a numeric literal is evidence, never a magnitude.
fn numeric_literal(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::NumericLiteral(_) => true,
        Expression::UnaryExpression(unary) if matches!(unary.operator.as_str(), "-" | "+") => {
            matches!(
                unary.argument.without_parentheses(),
                Expression::NumericLiteral(_)
            )
        }
        _ => false,
    }
}

fn string(expression: &Expression<'_>, expected: &str) -> bool {
    matches!(expression.without_parentheses(), Expression::StringLiteral(value) if value.value == expected)
}

fn boolean_true(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::BooleanLiteral(value) if value.value)
}

fn void_zero(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::UnaryExpression(unary)
            if unary.operator.as_str() == "void" && number(&unary.argument, 0.0)
    )
}

fn expression_statement<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    Some(&statement.expression)
}

fn assignment<'a>(expression: &'a Expression<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Expression::AssignmentExpression(assignment) = expression.without_parentheses() else {
        return None;
    };
    (assignment.operator.as_str() == "=").then_some(assignment)
}

fn assigned_symbol(model: &Model<'_, '_>, assignment: &AssignmentExpression<'_>) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

fn one_variable<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(variable)
}

fn block<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(block) = statement else {
        return None;
    };
    Some(block.body.as_slice())
}

/// A plain `let`/`const`/`var` declarator bound to exactly this symbol in its
/// own body. No other declaration shape is the local identity the proof reads.
fn plain_local(model: &Model<'_, '_>, variable: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
        return None;
    };
    let local = identifier.symbol_id.get()?;
    if !model.scoping.symbol_redeclarations(local).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(local);
    let AstKind::VariableDeclarator(declared) = model.nodes.kind(declaration) else {
        return None;
    };
    matches!(&declared.id, BindingPattern::BindingIdentifier(found)
        if found.symbol_id.get() == Some(local))
    .then_some(local)
}

/// A captured data slot: a plain local declared outside the owned body and
/// never redeclared, distinct from every local the proof reads inside it.
fn captured_local(model: &Model<'_, '_>, site: FunctionSite, candidate: Symbol) -> Option<Symbol> {
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(candidate))
        || !model.scoping.symbol_redeclarations(candidate).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(candidate);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    matches!(&variable.id, BindingPattern::BindingIdentifier(found)
        if found.symbol_id.get() == Some(candidate))
    .then_some(candidate)
}

fn returns_void(statement: &Statement<'_>) -> bool {
    let returned = match statement {
        Statement::ReturnStatement(returned) => returned,
        Statement::BlockStatement(block) => match block.body.as_slice() {
            [Statement::ReturnStatement(returned)] => returned,
            _ => return false,
        },
        _ => return false,
    };
    returned.argument.as_ref().is_some_and(void_zero)
}

/// An unshadowed global namespace path. A bound root is never a host global.
fn is_global(model: &Model<'_, '_>, expression: &Expression<'_>, path: &[&str]) -> bool {
    expression_path(model, expression)
        .iter()
        .map(String::as_str)
        .eq(path.iter().copied())
}

// ===========================================================================
// Owned setter body
// ===========================================================================

fn input_null(model: &Model<'_, '_>, test: &Expression<'_>, input: Symbol) -> bool {
    let Expression::BinaryExpression(binary) = test.without_parentheses() else {
        return false;
    };
    if !matches!(binary.operator.as_str(), "==" | "===") {
        return false;
    }
    let is_null = |expression: &Expression<'_>| {
        matches!(expression.without_parentheses(), Expression::NullLiteral(_))
    };
    (symbol(model, &binary.left) == Some(input) && is_null(&binary.right))
        || (symbol(model, &binary.right) == Some(input) && is_null(&binary.left))
}

/// `Object.keys(input)` on the unshadowed global `Object`.
fn object_keys(model: &Model<'_, '_>, expression: &Expression<'_>, input: Symbol) -> bool {
    let Some(call) = call(expression) else {
        return false;
    };
    if !is_global(model, &call.callee, &["Object", "keys"]) {
        return false;
    }
    let arguments = call_arguments(call);
    arguments.len() == 1 && call.arguments.len() == 1 && symbol(model, arguments[0]) == Some(input)
}

fn sorted(model: &Model<'_, '_>, expression: &Expression<'_>, keys: Symbol) -> bool {
    let Some(call) = call(expression) else {
        return false;
    };
    call.arguments.is_empty() && method_receiver(model, call, "sort") == Some(keys)
}

/// The explicit RHS-before-self-overwrite `keys = [keys, keys.join]`. The right
/// side is read before the assignment replaces the binding, so the first slot
/// is the pre-sort array symbol itself and the second is that same symbol's own
/// `join` method. This is one bounded two-slot array, never a general
/// destructuring or spread framework.
fn keys_pair_overwrite(model: &Model<'_, '_>, expression: &Expression<'_>, keys: Symbol) -> bool {
    let Some(assignment) = assignment(expression) else {
        return false;
    };
    if assigned_symbol(model, assignment) != Some(keys) {
        return false;
    }
    let Expression::ArrayExpression(array) = assignment.right.without_parentheses() else {
        return false;
    };
    let mut elements = Vec::new();
    for element in &array.elements {
        let Some(value) = element.as_expression() else {
            return false;
        };
        elements.push(value);
    }
    let [first, second] = elements.as_slice() else {
        return false;
    };
    symbol(model, first) == Some(keys)
        && member(second, "join").and_then(|object| symbol(model, object)) == Some(keys)
}

/// `keys[index]` for the proved keys array symbol and an exact numeric index.
fn computed_index(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    keys: Symbol,
    index: f64,
) -> bool {
    let Some((object, position)) = computed(expression) else {
        return false;
    };
    symbol(model, object) == Some(keys) && number(position, index)
}

/// `keys[1].call(keys[0], ";")`: the sorted array's own `join` invoked through
/// `.call` with the array as `this` and the fixed `;` separator.
fn keys_join_call(model: &Model<'_, '_>, expression: &Expression<'_>, keys: Symbol) -> bool {
    let Some(call) = call(expression) else {
        return false;
    };
    let Some(receiver) = member(&call.callee, "call") else {
        return false;
    };
    if !computed_index(model, receiver, keys, 1.0) {
        return false;
    }
    let arguments = call_arguments(call);
    arguments.len() == 2
        && call.arguments.len() == 2
        && computed_index(model, arguments[0], keys, 0.0)
        && string(arguments[1], ";")
}

/// The direct variant: `keys.join(";")` while `keys` is still the sorted
/// array. It is intentionally separate from the holder/call protocol above;
/// no arbitrary member-call or return normalization is admitted.
fn direct_keys_join(model: &Model<'_, '_>, expression: &Expression<'_>, keys: Symbol) -> bool {
    let Some(call) = call(expression) else {
        return false;
    };
    if method_receiver(model, call, "join") != Some(keys) {
        return false;
    }
    let arguments = call_arguments(call);
    arguments.len() == 1 && call.arguments.len() == 1 && string(arguments[0], ";")
}

/// An opaque digest consumer over the one join call. Only the exact argument
/// dataflow is evidence: `consumer(keys[1].call(keys[0], ";"))`.
fn digest_call(model: &Model<'_, '_>, expression: &Expression<'_>, keys: Symbol) -> bool {
    let Some(consumer) = call(expression) else {
        return false;
    };
    let arguments = call_arguments(consumer);
    arguments.len() == 1
        && consumer.arguments.len() == 1
        && keys_join_call(model, arguments[0], keys)
}

/// An opaque digest consumer over the direct join form. The consumer remains
/// opaque; only its one argument and that argument's receiver/separator are
/// source-backed.
fn direct_digest_call(model: &Model<'_, '_>, expression: &Expression<'_>, keys: Symbol) -> bool {
    let Some(consumer) = call(expression) else {
        return false;
    };
    let arguments = call_arguments(consumer);
    arguments.len() == 1
        && consumer.arguments.len() == 1
        && direct_keys_join(model, arguments[0], keys)
}

fn digest_test(model: &Model<'_, '_>, test: &Expression<'_>, keys: Symbol, direct: bool) -> bool {
    let Expression::BinaryExpression(binary) = test.without_parentheses() else {
        return false;
    };
    if !matches!(binary.operator.as_str(), "==" | "===" | "!=" | "!==") {
        return false;
    }
    let digest = |expression: &Expression<'_>| {
        if direct {
            direct_digest_call(model, expression, keys)
        } else {
            digest_call(model, expression, keys)
        }
    };
    (numeric_literal(&binary.left) && digest(&binary.right))
        || (numeric_literal(&binary.right) && digest(&binary.left))
}

/// Exactly the four protocol fields, each mapped as the witness maps them:
/// `selectMeta` is the literal `true` that the branch just set, and the other
/// three read distinct captured slots. A four-field object with any other key,
/// value shape, duplicate key or spread donates nothing.
fn payload(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    excluded: &[Symbol],
) -> bool {
    let Expression::ObjectExpression(object) = expression.without_parentheses() else {
        return false;
    };
    if object.properties.len() != 4 {
        return false;
    }
    let mut seen = BTreeSet::new();
    let mut select_meta = false;
    let mut slots = BTreeSet::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return false;
        }
        let Some(name) = property_key_name(&property.key) else {
            return false;
        };
        if !seen.insert(name.clone()) {
            return false;
        }
        match name.as_str() {
            "selectMeta" => {
                if !boolean_true(&property.value) {
                    return false;
                }
                select_meta = true;
            }
            "animationStart" | "measurementDiv" | "animationStyle" => {
                let Some(slot) = symbol(model, &property.value) else {
                    return false;
                };
                let Some(slot) = captured_local(model, site, slot) else {
                    return false;
                };
                if excluded.contains(&slot) || !slots.insert(slot) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    select_meta && slots.len() == 3 && seen.len() == 4
}

/// The whole owned setter body. Returns the captured slot the argument is
/// defused into so the caller can prove the paired getter returns it.
fn select_meta_setter(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    body: &FunctionBody<'_>,
) -> Option<Symbol> {
    let [Statement::ExpressionStatement(store), Statement::IfStatement(guard)] =
        body.statements.as_slice()
    else {
        return None;
    };
    // The one formal is defused into one captured slot unchanged.
    let store = assignment(&store.expression)?;
    let stored = assigned_symbol(model, store)?;
    if symbol(model, &store.right) != Some(input) || stored == input {
        return None;
    }
    let stored = captured_local(model, site, stored)?;

    if guard.alternate.is_none() || !input_null(model, &guard.test, input) {
        return None;
    }
    if !returns_void(&guard.consequent) {
        return None;
    }
    let Some(Statement::BlockStatement(rest)) = guard.alternate.as_ref() else {
        return None;
    };
    let [keys_statement, sort_statement, remainder @ ..] = rest.body.as_slice() else {
        return None;
    };

    let keys_variable = one_variable(keys_statement)?;
    let keys = plain_local(model, keys_variable)?;
    if keys == input || keys == stored {
        return None;
    }
    if !object_keys(model, keys_variable.init.as_ref()?, input) {
        return None;
    }
    if !expression_statement(sort_statement)
        .is_some_and(|expression| sorted(model, expression, keys))
    {
        return None;
    }
    // The accepted001 holder shape has three statements after the sort. The
    // direct variant has exactly two: its probe consumes `keys.join(";")`
    // directly. Keeping the shapes disjoint prevents extra statements,
    // arbitrary reordering and return stripping from becoming normalization.
    let (probe, tail, direct) = match remainder {
        [overwrite_statement, probe, tail]
            if expression_statement(overwrite_statement)
                .is_some_and(|expression| keys_pair_overwrite(model, expression, keys)) =>
        {
            (probe, tail, false)
        }
        [probe, tail] => (probe, tail, true),
        _ => return None,
    };
    if !returns_void(tail) {
        return None;
    }

    let Statement::IfStatement(probe) = probe else {
        return None;
    };
    if probe.alternate.is_some() || !digest_test(model, &probe.test, keys, direct) {
        return None;
    }
    let Some(probe_body) = block(&probe.consequent) else {
        return None;
    };
    let [Statement::ExpressionStatement(flag_statement), Statement::ExpressionStatement(next_statement)] =
        probe_body
    else {
        return None;
    };

    let Some(flag_assignment) = assignment(&flag_statement.expression) else {
        return None;
    };
    let Some(flag) = assigned_symbol(model, flag_assignment) else {
        return None;
    };
    if !boolean_true(&flag_assignment.right) {
        return None;
    }
    let flag = captured_local(model, site, flag)?;
    if flag == input || flag == stored || flag == keys {
        return None;
    }

    let Some(next_call) = call(&next_statement.expression) else {
        return None;
    };
    let observer = method_receiver(model, next_call, "next")?;
    let observer = captured_local(model, site, observer)?;
    let arguments = call_arguments(next_call);
    if arguments.len() != 1 || next_call.arguments.len() != 1 {
        return None;
    }
    if !payload(
        model,
        site,
        arguments[0],
        &[input, stored, keys, flag, observer],
    ) {
        return None;
    }
    Some(stored)
}

// ===========================================================================
// Descriptor registration
// ===========================================================================

/// One direct statement list is one supported installation region. Merely
/// sharing a function owner would also admit sibling branches and dead donors.
fn installation_region<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
) -> Option<&'a [Statement<'a>]> {
    match model.nodes.parent_kind(site.node_id) {
        AstKind::FunctionBody(body) => Some(body.statements.as_slice()),
        AstKind::BlockStatement(block) => Some(block.body.as_slice()),
        AstKind::Program(program) => Some(program.body.as_slice()),
        _ => None,
    }
}

/// Function declarations are hoisted within this same region. An initialized
/// function expression is supported only before the property write. Neither
/// route follows historical assignments, aliases, or another scope's callable.
fn regional_callable(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    callable: Symbol,
    use_index: usize,
    expected: FunctionSite,
) -> bool {
    if model.binding_writes(callable) != 0
        || !model.scoping.symbol_redeclarations(callable).is_empty()
    {
        return false;
    }
    statements.iter().enumerate().any(|(index, statement)| {
        match statement {
            Statement::FunctionDeclaration(function) => {
                function.span == expected.span
                    && function.id.as_ref().and_then(|id| id.symbol_id.get()) == Some(callable)
            }
            _ if index < use_index => one_variable(statement).is_some_and(|variable| {
                plain_local(model, variable) == Some(callable)
                    && variable.init.as_ref().is_some_and(|init| {
                        matches!(init.without_parentheses(),
                            Expression::FunctionExpression(function) if function.span == expected.span)
                    })
            }),
            _ => false,
        }
    })
}

/// Prove an unescaped fresh descriptor followed by its two property writes and
/// installation, with only inert function declarations interleaved. Exactly
/// three references to the descriptor excludes aliases, computed writes,
/// deletes and nested mutations even when their scopes differ. The corridor
/// excludes intervening calls/control flow that could invalidate the values.
fn ordered_installation(
    model: &Model<'_, '_>,
    site: FunctionSite,
    descriptor: Symbol,
    getter: Symbol,
    getter_site: FunctionSite,
) -> bool {
    let Some(statements) = installation_region(model, site) else {
        return false;
    };
    if model.binding_writes(descriptor) != 0 || model.binding_references(descriptor) != 3 {
        return false;
    }
    let Some(start) = statements.iter().position(|statement| {
        one_variable(statement).is_some_and(|variable| {
            plain_local(model, variable) == Some(descriptor)
                && variable.init.as_ref().is_some_and(|init| {
                    let Expression::ObjectExpression(object) = init.without_parentheses() else {
                        return false;
                    };
                    let mut fields = BTreeSet::new();
                    object.properties.iter().all(|property| {
                        let ObjectPropertyKind::ObjectProperty(property) = property else {
                            return false;
                        };
                        !property.computed
                            && !property.method
                            && property.kind == PropertyKind::Init
                            && property_key_name(&property.key).is_some_and(|name| {
                                matches!(name.as_str(), "get" | "set") && fields.insert(name)
                            })
                            && void_zero(&property.value)
                    })
                })
        })
    }) else {
        return false;
    };
    let mut got_get = false;
    let mut got_set = false;
    for (index, statement) in statements.iter().enumerate().skip(start + 1) {
        if matches!(statement, Statement::FunctionDeclaration(_)) {
            continue;
        }
        let Some(expression) = expression_statement(statement) else {
            return false;
        };
        if let Some(write) = assignment(expression) {
            let AssignmentTarget::StaticMemberExpression(target) = &write.left else {
                return false;
            };
            if target.optional || symbol(model, &target.object) != Some(descriptor) {
                return false;
            }
            match target.property.name.as_str() {
                "get" if !got_get && symbol(model, &write.right) == Some(getter) => {
                    if !regional_callable(model, statements, getter, index, getter_site) {
                        return false;
                    }
                    got_get = true;
                }
                "set" if !got_set && symbol(model, &write.right) == site.binding_symbol => {
                    let Some(setter) = site.binding_symbol else {
                        return false;
                    };
                    if !regional_callable(model, statements, setter, index, site) {
                        return false;
                    }
                    got_set = true;
                }
                _ => return false,
            }
            continue;
        }
        let Some(install) = call(expression) else {
            return false;
        };
        let arguments = call_arguments(install);
        return got_get
            && got_set
            && is_global(model, &install.callee, &["Object", "defineProperty"])
            && arguments.len() == 3
            && install.arguments.len() == 3
            && string(arguments[1], SELECT_META)
            && symbol(model, arguments[2]) == Some(descriptor);
    }
    false
}

/// The enclosing registration that makes the claimed body the `set` of a
/// `_selectMeta` accessor descriptor. Every comparison is a scope-resolved
/// symbol, so a same-spelled binding elsewhere donates nothing.
fn descriptor_registration(model: &Model<'_, '_>, site: FunctionSite, stored: Symbol) -> bool {
    let Some(this) = site.binding_symbol else {
        return false;
    };

    // Exactly one `<descriptor>.set = <this function>`.
    let mut descriptor = None;
    for node in model.nodes.iter() {
        let AstKind::AssignmentExpression(assignment) = node.kind() else {
            continue;
        };
        if assignment.operator.as_str() != "=" {
            continue;
        }
        let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
            continue;
        };
        if target.optional || target.property.name != "set" {
            continue;
        }
        if symbol(model, &assignment.right) != Some(this) {
            continue;
        }
        let Some(owner) = symbol(model, &target.object) else {
            return false;
        };
        if owner == this || descriptor.is_some() {
            return false;
        }
        descriptor = Some(owner);
    }
    let Some(descriptor) = descriptor else {
        return false;
    };
    if captured_local(model, site, descriptor).is_none() {
        return false;
    }

    // Exactly one paired `<descriptor>.get = <getter>`.
    let mut getter = None;
    for node in model.nodes.iter() {
        let AstKind::AssignmentExpression(assignment) = node.kind() else {
            continue;
        };
        if assignment.operator.as_str() != "=" {
            continue;
        }
        let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
            continue;
        };
        if target.optional || target.property.name != "get" {
            continue;
        }
        if symbol(model, &target.object) != Some(descriptor) {
            continue;
        }
        let Some(owner) = symbol(model, &assignment.right) else {
            return false;
        };
        if getter.is_some() {
            return false;
        }
        getter = Some(owner);
    }
    let Some(getter) = getter else {
        return false;
    };
    if getter == this || getter == descriptor {
        return false;
    }

    // The getter's whole body returns the same slot the setter defused into.
    let Some(getter_site) = model.function_from_symbol(getter) else {
        return false;
    };
    let AstKind::Function(function) = model.nodes.kind(getter_site.node_id) else {
        return false;
    };
    if function.r#async
        || function.generator
        || !function.params.items.is_empty()
        || function.params.rest.is_some()
    {
        return false;
    }
    let Some(body) = &function.body else {
        return false;
    };
    let [Statement::ReturnStatement(returned)] = body.statements.as_slice() else {
        return false;
    };
    if returned
        .argument
        .as_ref()
        .and_then(|value| symbol(model, value))
        != Some(stored)
    {
        return false;
    }

    // Exactly one `Object.defineProperty(<target>, "_selectMeta", descriptor)`.
    let mut registrations = 0usize;
    for node in model.nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !is_global(model, &call.callee, &["Object", "defineProperty"]) {
            continue;
        }
        let arguments = call_arguments(call);
        if arguments.len() != 3 || call.arguments.len() != 3 {
            continue;
        }
        if !string(arguments[1], SELECT_META) || symbol(model, arguments[2]) != Some(descriptor) {
            continue;
        }
        registrations += 1;
    }
    registrations == 1 && ordered_installation(model, site, descriptor, getter, getter_site)
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
    let Some(stored) = select_meta_setter(model, site, input, body) else {
        return Vec::new();
    };
    if !descriptor_registration(model, site, stored) {
        return Vec::new();
    }
    vec![proposal(
        NAME,
        EVALUATOR,
        "one-formal `_selectMeta` accessor setter: the formal is defused unchanged \
         into one captured slot and null-guarded, the sorted `Object.keys` array is \
         read through its explicit RHS-before-self-overwrite `[keys, keys.join]`, the \
         opaque consumer digests `keys[1].call(keys[0], \";\")` against a numeric \
         literal, one captured flag is set true and the four-field \
         selectMeta/animationStart/measurementDiv/animationStyle object is handed to \
         one captured `.next` receiver; the enclosing scope registers this exact \
         function as `descriptor.set` and its paired `descriptor.get` returns the same \
         captured slot, under one `Object.defineProperty(target, \"_selectMeta\", \
         descriptor)`",
        f,
    )]
}
