//! Bounded owned-body proof for the persistent transaction-token initializer.
//!
//! The family is the zero-argument session step that defends a transaction
//! token in persistent storage and seeds a second persistent value:
//!
//! ```text
//! function init() {
//!   let key = config.persistentId.key;        // or .resetKey
//!   let token = config.transactionToken;
//!   try {                                      // optional guard/try shape
//!     ready = false;
//!     if (storage) {
//!       if (!storage.getItem(key)) storage.setItem(key, token);
//!       ready = true;
//!     }
//!   } catch (e) {}
//!   try {
//!     if (storage) {
//!       seedKey = "<prefix>" + config.persistentId.key;
//!       ... Date.now / Math.random / performance.now / new Date /
//!           crypto.getRandomValues ...
//!       ++counter;
//!       seedValue = "" + stamp + ((mix ^ entropy ^ counter) >>> 0);
//!       if (!storage.getItem(seedKey)) storage.setItem(seedKey, seedValue);
//!     }
//!   } catch (e) {}
//! }
//! ```
//!
//! The proof is deliberately operation/dataflow based:
//!
//! * the function is a zero-formal, non-async, non-generator owned body;
//! * one captured configuration provider is read for `persistentId.key`
//!   (`resetKey` is the same role) and `transactionToken`;
//! * one captured storage provider is called through `getItem`/`setItem`
//!   pairs, each pair guarded by a presence test on that same provider;
//! * the first pair stores the transaction token under the direct persistent
//!   key only when the key is absent;
//! * the second pair stores a generated string under a prefixed persistent
//!   key only when that key is absent, and the exact stored value is a string
//!   concatenation whose terms must include the timestamp result and an
//!   unsigned `XOR >>> 0` mix;
//! * that mix is resolved as one expression/dataflow graph: `Math.random`, a
//!   second ambient entropy source (`performance.now` or
//!   `crypto.getRandomValues`) and an incremented counter must actually reach
//!   the XOR, so an unrelated timestamp/entropy call in the same guard cannot
//!   donate proof;
//! * a bare zero-argument helper call is admitted only when its immutable
//!   callable binding and the resolved function are captured outside the owned
//!   body, and that helper's direct unsigned-XOR return flow also contains a
//!   time source;
//! * aliases are followed through their initializer and every plain `=` write
//!   that belongs to the owned body, so the `let x = ...` and `x = ...`
//!   spellings are the same origin; ambient globals only count when they are
//!   unshadowed, and the storage and configuration roots must stay captured
//!   bindings (a shadowing local is refused);
//! * nested functions, arrows, methods and classes are counted and never
//!   descended into, so a nested decoy cannot supply the outer evidence.
//!
//! The configuration/storage/persistent/token spellings are protocol member
//! names, not generated names: no thread id, KEY spelling, span, vendor or
//! hash participates. The older `p14_persistence_telemetry` finder already
//! owns the encrypted telemetry record, so this rule runs only after every
//! earlier stage has declined the function and does not claim storage-event
//! listeners, cookie writers or general payload collection.

use super::*;
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{UnaryOperator, UpdateOperator};

const NAME: &str = "initializePersistentTransactionState";
const EVALUATOR: &str = "posthost_persistent_init";
const DETAIL: &str = "zero-formal owned body: one captured config provider yields persistentId.key/resetKey and transactionToken; one captured storage provider is read/written by two presence-guarded getItem/setItem pairs; the first stores the transaction token under the direct persistent key, the second stores a generated string under a prefixed persistent key whose exact value is a string concatenation of the timestamp result and either an inline unsigned XOR >>> 0 mix or a bare zero-argument call to one immutable captured helper whose direct unsigned-XOR return flow contains time, Math.random, second unshadowed performance/crypto entropy, and an incremented counter; storage/config/helper roots must stay captured, ambient globals must be unshadowed, and nested functions never supply evidence";

// ===========================================================================
// Small typed views of the source operations
// ===========================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum StorageId {
    Symbol(SymbolId),
    Global(&'static str),
}

#[derive(Clone, Copy)]
enum Root {
    Symbol(SymbolId),
    Global,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct KeyOrigin {
    config: SymbolId,
    derived: bool,
}

struct GetCall<'a> {
    call: &'a CallExpression<'a>,
    storage: StorageId,
    key: KeyOrigin,
}

struct SetCall<'a> {
    call: &'a CallExpression<'a>,
    storage: StorageId,
    key: KeyOrigin,
    value: &'a Expression<'a>,
}

// ===========================================================================
// Member and scope helpers
// ===========================================================================

/// A static member, a computed string member, or nothing. Optional chaining is
/// never the same protocol member.
fn member_of<'a>(expression: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a str, bool)> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some((
            &member.object,
            member.property.name.as_str(),
            member.optional,
        )),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => {
                Some((&member.object, literal.value.as_str(), member.optional))
            }
            _ => None,
        },
        _ => None,
    }
}

fn call_member<'a>(call: &'a CallExpression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    if call.optional {
        return None;
    }
    let (object, property, optional) = member_of(&call.callee)?;
    (!optional && property == name).then_some(object)
}

/// The same local argument view the sibling owned-body rules use; the shared
/// parent module does not re-export `new_arguments`.
fn new_arguments<'a>(new: &'a NewExpression<'a>) -> Vec<&'a Expression<'a>> {
    new.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

/// The root of a static/computed-string member chain and its property names.
/// The walk is bounded; a root is either a lexical symbol or an unshadowed
/// ambient name.
fn member_chain(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<(Root, Vec<String>)> {
    member_chain_at(model, expression, 0)
}

fn member_chain_at(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    depth: usize,
) -> Option<(Root, Vec<String>)> {
    if depth > 16 {
        return None;
    }
    let expression = expression.without_parentheses();
    if let Expression::Identifier(identifier) = expression {
        return Some(
            match crate::analysis::identifier_symbol(identifier, model.scoping) {
                Some(symbol) => (Root::Symbol(symbol), Vec::new()),
                None => (Root::Global, vec![identifier.name.to_string()]),
            },
        );
    }
    let (object, property, optional) = member_of(expression)?;
    if optional {
        return None;
    }
    let (root, mut path) = member_chain_at(model, object, depth + 1)?;
    path.push(property.to_string());
    Some((root, path))
}

/// A plain plain-declarator binding declared outside the owned body, with no
/// redeclaration. The storage and configuration providers are captured values.
fn captured_variable(model: &Model<'_, '_>, site: FunctionSite, symbol: SymbolId) -> bool {
    if !model.scoping.symbol_redeclarations(symbol).is_empty() {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    if site
        .span
        .contains_inclusive(model.nodes.get_node(declaration).span())
    {
        return false;
    }
    matches!(
        model.nodes.kind(declaration),
        AstKind::VariableDeclarator(declarator)
            if matches!(&declarator.id, BindingPattern::BindingIdentifier(identifier)
                if identifier.symbol_id.get() == Some(symbol))
    )
}

// ===========================================================================
// Single-origin values
// ===========================================================================

/// The one value an alias can denote: a local declarator initializer, or the
/// sole plain `=` assignment to a captured/local binding inside this body.
fn single_origin<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    symbol: SymbolId,
) -> Option<&'a Expression<'a>> {
    if !model.scoping.symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(declarator) = model.nodes.kind(declaration) else {
        return None;
    };
    if site
        .span
        .contains_inclusive(model.nodes.get_node(declaration).span())
    {
        let writes = model.binding_writes(symbol);
        if writes == 0 {
            return declarator.init.as_ref();
        }
        if writes == 1 && declarator.init.is_none() {
            return assigned_value(model, site, symbol);
        }
        None
    } else {
        assigned_value(model, site, symbol)
    }
}

fn assigned_value<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    symbol: SymbolId,
) -> Option<&'a Expression<'a>> {
    let mut references = model
        .scoping
        .get_resolved_references(symbol)
        .filter(|reference| reference.is_write());
    let reference = references.next()?;
    if references.next().is_some() {
        return None;
    }
    let node = reference.node_id();
    if !site
        .span
        .contains_inclusive(model.nodes.get_node(node).span())
    {
        return None;
    }
    let AstKind::AssignmentExpression(assignment) = model.nodes.parent_kind(node) else {
        return None;
    };
    if assignment.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    if crate::analysis::identifier_symbol(identifier, model.scoping) != Some(symbol) {
        return None;
    }
    Some(&assignment.right)
}

fn resolve<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
    depth: usize,
) -> Option<&'a Expression<'a>> {
    if depth > 8 {
        return None;
    }
    let expression = expression.without_parentheses();
    let Expression::Identifier(identifier) = expression else {
        return Some(expression);
    };
    let Some(symbol) = crate::analysis::identifier_symbol(identifier, model.scoping) else {
        return Some(expression);
    };
    match single_origin(model, site, symbol) {
        Some(value) => resolve(model, site, value, depth + 1),
        None => Some(expression),
    }
}

// ===========================================================================
// Family member identities
// ===========================================================================

fn storage_identity(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<StorageId> {
    let (root, path) = member_chain(model, expression)?;
    match root {
        Root::Symbol(symbol) => (path.is_empty() && captured_variable(model, site, symbol))
            .then_some(StorageId::Symbol(symbol)),
        Root::Global => {
            let name = match path.as_slice() {
                [name] if name.as_str() == "localStorage" || name.as_str() == "sessionStorage" => {
                    name.as_str()
                }
                [root, name]
                    if (root.as_str() == "window"
                        || root.as_str() == "globalThis"
                        || root.as_str() == "self")
                        && (name.as_str() == "localStorage"
                            || name.as_str() == "sessionStorage") =>
                {
                    name.as_str()
                }
                _ => return None,
            };
            Some(StorageId::Global(if name == "localStorage" {
                "localStorage"
            } else {
                "sessionStorage"
            }))
        }
    }
}

fn persistent_member<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> Option<(SymbolId, &'static str)> {
    let (root, path) = member_chain(model, expression)?;
    if path.len() != 2 || path[0].as_str() != "persistentId" {
        return None;
    }
    let name = match path[1].as_str() {
        "key" => "key",
        "resetKey" => "resetKey",
        _ => return None,
    };
    let Root::Symbol(symbol) = root else {
        return None;
    };
    captured_variable(model, site, symbol).then_some((symbol, name))
}

fn token_member<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> Option<SymbolId> {
    let (root, path) = member_chain(model, expression)?;
    if path.len() != 1 || path[0].as_str() != "transactionToken" {
        return None;
    }
    let Root::Symbol(symbol) = root else {
        return None;
    };
    captured_variable(model, site, symbol).then_some(symbol)
}

/// `"<nonempty prefix>" + config.persistentId.key` (either operand order).
fn derived_member<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> Option<(SymbolId, String)> {
    let expression = expression.without_parentheses();
    let Expression::BinaryExpression(binary) = expression else {
        return None;
    };
    if binary.operator.as_str() != "+" {
        return None;
    }
    let left = binary.left.without_parentheses();
    let right = binary.right.without_parentheses();
    let (prefix, member) = match (left, right) {
        (Expression::StringLiteral(literal), member) if !literal.value.as_str().is_empty() => {
            (literal.value.as_str(), member)
        }
        (member, Expression::StringLiteral(literal)) if !literal.value.as_str().is_empty() => {
            (literal.value.as_str(), member)
        }
        _ => return None,
    };
    let (config, _) = persistent_member(model, site, member)?;
    Some((config, prefix.to_string()))
}

fn key_origin<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> Option<KeyOrigin> {
    let value = resolve(model, site, expression, 0)?;
    if let Some((config, _)) = persistent_member(model, site, value) {
        return Some(KeyOrigin {
            config,
            derived: false,
        });
    }
    let (config, _) = derived_member(model, site, value)?;
    Some(KeyOrigin {
        config,
        derived: true,
    })
}

fn token_origin<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> Option<SymbolId> {
    let value = resolve(model, site, expression, 0)?;
    token_member(model, site, value)
}

// ===========================================================================
// Guards and generated-value operations
// ===========================================================================

fn is_string(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::StringLiteral(_)
    )
}

fn numeric_zero(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::NumericLiteral(number) if number.value == 0.0
    )
}

fn operand_is_call(expression: &Expression<'_>, call: &CallExpression<'_>) -> bool {
    expression.without_parentheses().span() == call.span
}

fn is_nullish(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::NullLiteral(_) => true,
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void => matches!(
            unary.argument.without_parentheses(),
            Expression::NumericLiteral(number) if number.value == 0.0
        ),
        _ => false,
    }
}

/// The `if (!storage.getItem(key))` / `if (storage.getItem(key) == null)`
/// condition whose consequent owns the write.
fn negated_call_condition<'a>(
    conditionals: &[&'a IfStatement<'a>],
    call: &CallExpression<'_>,
) -> Option<&'a IfStatement<'a>> {
    conditionals
        .iter()
        .copied()
        .find(|condition| match condition.test.without_parentheses() {
            Expression::UnaryExpression(unary) => {
                unary.operator == UnaryOperator::LogicalNot
                    && unary.argument.without_parentheses().span() == call.span
            }
            Expression::BinaryExpression(binary) => {
                let equality =
                    binary.operator.as_str() == "==" || binary.operator.as_str() == "===";
                equality
                    && ((operand_is_call(&binary.left, call) && is_nullish(&binary.right))
                        || (operand_is_call(&binary.right, call) && is_nullish(&binary.left)))
            }
            _ => false,
        })
}

/// The consequent span of the nearest `if (storage)` guard that contains
/// `inner`.
fn guarded_span(
    model: &Model<'_, '_>,
    site: FunctionSite,
    conditionals: &[&IfStatement<'_>],
    storage: StorageId,
    inner: Span,
) -> Option<Span> {
    conditionals.iter().copied().find_map(|condition| {
        if storage_identity(model, site, &condition.test) != Some(storage) {
            return None;
        }
        let span = condition.consequent.span();
        span.contains_inclusive(inner).then_some(span)
    })
}

/// Bounds for the value dataflow walk. Every hop stays inside the owned body
/// and never enters a nested function, so a fixed budget decides the family
/// deterministically.
const MAX_FLOW_DEPTH: usize = 16;
const MAX_FLOW_SYMBOLS: usize = 64;

/// The calls, constructor calls and incremented bindings actually reachable
/// from one stored value through body-local aliases. Ambient globals are
/// resolved by [`expression_path`], which only yields a path for an unshadowed
/// binding, so a shadowing local can never donate entropy.
#[derive(Default)]
struct ValueFlow<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    incremented: Vec<SymbolId>,
}

fn path_matches(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.as_str() == *expected)
}

fn flow_has_path(model: &Model<'_, '_>, flow: &ValueFlow<'_>, path: &[&str]) -> bool {
    flow.calls
        .iter()
        .any(|call| path_matches(&expression_path(model, &call.callee), path))
}

fn flow_has_timestamp(model: &Model<'_, '_>, flow: &ValueFlow<'_>) -> bool {
    flow_has_path(model, flow, &["Date", "now"])
        || flow
            .news
            .iter()
            .any(|new| path_matches(&expression_path(model, &new.callee), &["Date"]))
}

fn flow_has_random(model: &Model<'_, '_>, flow: &ValueFlow<'_>) -> bool {
    flow_has_path(model, flow, &["Math", "random"])
}

fn flow_has_second_entropy(model: &Model<'_, '_>, flow: &ValueFlow<'_>) -> bool {
    flow_has_path(model, flow, &["performance", "now"])
        || flow_has_path(model, flow, &["crypto", "getRandomValues"])
}

/// A callable binding is immutable and captured when its declaration is a
/// plain function or variable declaration outside the owned body, with no
/// redeclaration or resolved write. This is checked for both the call-site
/// alias and the function resolved through that alias.
fn captured_immutable_callable(
    model: &Model<'_, '_>,
    site: FunctionSite,
    symbol: SymbolId,
) -> bool {
    if !model.scoping.symbol_redeclarations(symbol).is_empty() || model.binding_writes(symbol) != 0
    {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    if site
        .span
        .contains_inclusive(model.nodes.get_node(declaration).span())
    {
        return false;
    }
    matches!(
        model.nodes.kind(declaration),
        AstKind::Function(_) | AstKind::VariableDeclarator(_)
    )
}

/// Prove only a helper's direct return expression and follow that expression
/// with the same bounded flow rules as an inline mix. The helper must have a
/// zero-formal plain function body and its returned mix must carry time,
/// random, second entropy and an incremented counter.
fn helper_returns_generated_mix(model: &Model<'_, '_>, helper: FunctionSite) -> bool {
    let AstKind::Function(function) = model.nodes.kind(helper.node_id) else {
        return false;
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
    {
        return false;
    }
    let Some(body) = function.body.as_ref() else {
        return false;
    };
    let mut returns = body
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::ReturnStatement(return_) => return_.argument.as_ref(),
            _ => None,
        });
    let Some(value) = returns.next() else {
        return false;
    };
    if returns.next().is_some() {
        return false;
    }
    let Some(mix) = unsigned_xor_mix(value) else {
        return false;
    };
    let mut flow = ValueFlow::default();
    let mut seen = Vec::new();
    collect_flow(model, helper, mix, &mut flow, &mut seen, 0);
    flow_has_random(model, &flow)
        && flow_has_second_entropy(model, &flow)
        && flow_has_timestamp(model, &flow)
        && !flow.incremented.is_empty()
}

/// A generated seed may use one direct, bare helper call as its mix term.
/// Resolve the actual call chain, reject receivers/arguments, and require both
/// the call binding and resolved function binding to be immutable captures
/// outside the owned initializer. In particular, a nested local or an
/// unrelated call elsewhere in the value cannot donate helper evidence.
fn direct_helper_mix(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return false;
    };
    if call.optional || !call.arguments.is_empty() {
        return false;
    }
    let Expression::Identifier(identifier) = call.callee.without_parentheses() else {
        return false;
    };
    let Some(symbol) = crate::analysis::identifier_symbol(identifier, model.scoping) else {
        return false;
    };
    if !captured_immutable_callable(model, site, symbol) {
        return false;
    }
    let Some(helper) = model.callee_function(&call.callee) else {
        return false;
    };
    if site.span.contains_inclusive(helper.span) {
        return false;
    }
    let Some(helper_symbol) = helper.binding_symbol else {
        return false;
    };
    captured_immutable_callable(model, site, helper_symbol)
        && helper_returns_generated_mix(model, helper)
}

fn is_one(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::NumericLiteral(number) if number.value == 1.0
    )
}

/// A node belongs to this owned body only when the innermost function or arrow
/// around it is the evaluated site itself. A write or initializer inside a
/// nested callback is therefore never a source of outer evidence.
fn owned_in_site(model: &Model<'_, '_>, site: FunctionSite, node_id: oxc_semantic::NodeId) -> bool {
    model
        .owning_site(node_id)
        .is_some_and(|owner| owner.node_id == site.node_id)
}

/// Follow one body-local binding to its initializer and to every plain `=`
/// write inside the owned body. A captured binding is still inspected for
/// in-body writes (the counter is usually declared by the enclosing scope), but
/// its out-of-body initializer is never required. Updates only mark a binding
/// as incremented; they never become a source by themselves.
fn follow_symbol<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    symbol: SymbolId,
    flow: &mut ValueFlow<'a>,
    seen: &mut Vec<SymbolId>,
    depth: usize,
) {
    let declaration = model.scoping.symbol_declaration(symbol);
    if owned_in_site(model, site, declaration) {
        if let AstKind::VariableDeclarator(declarator) = model.nodes.kind(declaration) {
            if let Some(init) = declarator.init.as_ref() {
                collect_flow(model, site, init, flow, seen, depth + 1);
            }
        }
    }
    for reference in model.scoping.get_resolved_references(symbol) {
        if !reference.is_write() {
            continue;
        }
        let node = reference.node_id();
        if !owned_in_site(model, site, node) {
            continue;
        }
        match model.nodes.parent_kind(node) {
            AstKind::AssignmentExpression(assignment) => match assignment.operator.as_str() {
                "=" => {
                    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                    else {
                        continue;
                    };
                    if crate::analysis::identifier_symbol(identifier, model.scoping) != Some(symbol)
                    {
                        continue;
                    }
                    collect_flow(model, site, &assignment.right, flow, seen, depth + 1);
                }
                "+=" if is_one(&assignment.right) => flow.incremented.push(symbol),
                _ => {}
            },
            AstKind::UpdateExpression(update)
                if update.operator == UpdateOperator::Increment
                    || update.operator == UpdateOperator::Decrement =>
            {
                flow.incremented.push(symbol);
            }
            _ => {}
        }
    }
}

/// The expression/dataflow graph rooted at one stored value. Nested functions,
/// arrows and classes are never entered, so an inner callback cannot donate
/// evidence to the outer body.
fn collect_flow<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
    flow: &mut ValueFlow<'a>,
    seen: &mut Vec<SymbolId>,
    depth: usize,
) {
    if depth > MAX_FLOW_DEPTH || seen.len() > MAX_FLOW_SYMBOLS {
        return;
    }
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => {
            let Some(symbol) = crate::analysis::identifier_symbol(identifier, model.scoping) else {
                return;
            };
            if seen.contains(&symbol) {
                return;
            }
            seen.push(symbol);
            follow_symbol(model, site, symbol, flow, seen, depth);
        }
        Expression::CallExpression(call) => {
            flow.calls.push(call);
            collect_flow(model, site, &call.callee, flow, seen, depth + 1);
            for argument in call_arguments(call) {
                collect_flow(model, site, argument, flow, seen, depth + 1);
            }
        }
        Expression::NewExpression(new) => {
            flow.news.push(new);
            collect_flow(model, site, &new.callee, flow, seen, depth + 1);
            for argument in new_arguments(new) {
                collect_flow(model, site, argument, flow, seen, depth + 1);
            }
        }
        Expression::BinaryExpression(binary) => {
            collect_flow(model, site, &binary.left, flow, seen, depth + 1);
            collect_flow(model, site, &binary.right, flow, seen, depth + 1);
        }
        Expression::LogicalExpression(logical) => {
            collect_flow(model, site, &logical.left, flow, seen, depth + 1);
            collect_flow(model, site, &logical.right, flow, seen, depth + 1);
        }
        Expression::ConditionalExpression(conditional) => {
            collect_flow(model, site, &conditional.test, flow, seen, depth + 1);
            collect_flow(model, site, &conditional.consequent, flow, seen, depth + 1);
            collect_flow(model, site, &conditional.alternate, flow, seen, depth + 1);
        }
        Expression::UnaryExpression(unary) => {
            collect_flow(model, site, &unary.argument, flow, seen, depth + 1);
        }
        Expression::AssignmentExpression(assignment) => {
            collect_flow(model, site, &assignment.right, flow, seen, depth + 1);
        }
        Expression::StaticMemberExpression(member) => {
            collect_flow(model, site, &member.object, flow, seen, depth + 1);
        }
        Expression::ComputedMemberExpression(member) => {
            collect_flow(model, site, &member.object, flow, seen, depth + 1);
            collect_flow(model, site, &member.expression, flow, seen, depth + 1);
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                if let Some(expression) = element.as_expression() {
                    collect_flow(model, site, expression, flow, seen, depth + 1);
                }
            }
        }
        Expression::SequenceExpression(sequence) => {
            for expression in &sequence.expressions {
                collect_flow(model, site, expression, flow, seen, depth + 1);
            }
        }
        Expression::TemplateLiteral(template) => {
            for expression in &template.expressions {
                collect_flow(model, site, expression, flow, seen, depth + 1);
            }
        }
        Expression::TaggedTemplateExpression(tagged) => {
            collect_flow(model, site, &tagged.tag, flow, seen, depth + 1);
            for expression in &tagged.quasi.expressions {
                collect_flow(model, site, expression, flow, seen, depth + 1);
            }
        }
        Expression::AwaitExpression(await_) => {
            collect_flow(model, site, &await_.argument, flow, seen, depth + 1);
        }
        Expression::YieldExpression(yield_) => {
            if let Some(argument) = &yield_.argument {
                collect_flow(model, site, argument, flow, seen, depth + 1);
            }
        }
        Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ClassExpression(_) => {}
        _ => {}
    }
}

/// Every `+` operand of a concatenation chain, left to right.
fn concat_terms<'a>(
    expression: &'a Expression<'a>,
    terms: &mut Vec<&'a Expression<'a>>,
    depth: usize,
) {
    if depth > MAX_FLOW_DEPTH {
        return;
    }
    let expression = expression.without_parentheses();
    if let Expression::BinaryExpression(binary) = expression {
        if binary.operator.as_str() == "+" {
            concat_terms(&binary.left, terms, depth + 1);
            concat_terms(&binary.right, terms, depth + 1);
            return;
        }
    }
    terms.push(expression);
}

/// `(x ^ y ^ ...) >>> 0`: the unsigned XOR mix term of a stored value.
fn unsigned_xor_mix<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::BinaryExpression(shift) = expression.without_parentheses() else {
        return None;
    };
    if shift.operator.as_str() != ">>>" || !numeric_zero(&shift.right) {
        return None;
    }
    let Expression::BinaryExpression(xor) = shift.left.without_parentheses() else {
        return None;
    };
    (xor.operator.as_str() == "^").then_some(&shift.left)
}

/// The generated persistent value is one string concatenation whose terms
/// include the timestamp result and an unsigned XOR mix. The mix must actually
/// consume `Math.random`, a second entropy source and an incremented counter
/// through the same alias graph, so a disconnected call in the same guard does
/// not donate proof.
fn generated_seed<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> bool {
    let Some(value) = resolve(model, site, expression, 0) else {
        return false;
    };
    let Expression::BinaryExpression(root) = value.without_parentheses() else {
        return false;
    };
    if root.operator.as_str() != "+" {
        return false;
    }
    let mut terms = Vec::new();
    concat_terms(value, &mut terms, 0);
    if !terms.iter().any(|term| is_string(term)) {
        return false;
    }
    let mut timestamp = false;
    let mut mix = false;
    for term in terms {
        if let Some(root) = unsigned_xor_mix(term) {
            let mut flow = ValueFlow::default();
            let mut seen = Vec::new();
            collect_flow(model, site, root, &mut flow, &mut seen, 0);
            if flow_has_random(model, &flow)
                && flow_has_second_entropy(model, &flow)
                && !flow.incremented.is_empty()
            {
                mix = true;
                continue;
            }
        }
        if direct_helper_mix(model, site, term) {
            mix = true;
            continue;
        }
        let mut flow = ValueFlow::default();
        let mut seen = Vec::new();
        collect_flow(model, site, term, &mut flow, &mut seen, 0);
        if flow_has_timestamp(model, &flow) {
            timestamp = true;
        }
    }
    timestamp && mix
}

// ===========================================================================
// Read-only owned-body census
// ===========================================================================

/// The operations of this exact function body. Nested functions, arrows,
/// methods and classes are counted and never descended into, so a decoy
/// callback cannot supply the outer evidence.
#[derive(Default)]
struct Ops<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    conditionals: Vec<&'a IfStatement<'a>>,
    nested: usize,
}

impl<'a> Ops<'a> {
    fn body(&mut self, body: &'a FunctionBody<'a>) {
        for statement in &body.statements {
            self.statement(statement);
        }
    }

    fn statements(&mut self, statements: &'a [Statement<'a>]) {
        for statement in statements {
            self.statement(statement);
        }
    }

    fn variable_declaration(&mut self, declaration: &'a VariableDeclaration<'a>) {
        for declarator in &declaration.declarations {
            if let Some(init) = &declarator.init {
                self.expression(init);
            }
        }
    }

    fn statement(&mut self, statement: &'a Statement<'a>) {
        match statement {
            Statement::VariableDeclaration(declaration) => self.variable_declaration(declaration),
            Statement::ExpressionStatement(statement) => self.expression(&statement.expression),
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.expression(argument);
                }
            }
            Statement::BlockStatement(block) => self.statements(&block.body),
            Statement::IfStatement(branch) => {
                self.conditionals.push(branch);
                self.expression(&branch.test);
                self.statement(&branch.consequent);
                if let Some(alternate) = &branch.alternate {
                    self.statement(alternate);
                }
            }
            Statement::TryStatement(attempt) => {
                self.statements(&attempt.block.body);
                if let Some(handler) = &attempt.handler {
                    self.statements(&handler.body.body);
                }
                if let Some(finalizer) = &attempt.finalizer {
                    self.statements(&finalizer.body);
                }
            }
            Statement::ForStatement(loop_) => {
                if let Some(ForStatementInit::VariableDeclaration(declaration)) = &loop_.init {
                    self.variable_declaration(declaration);
                }
                if let Some(test) = &loop_.test {
                    self.expression(test);
                }
                if let Some(update) = &loop_.update {
                    self.expression(update);
                }
                self.statement(&loop_.body);
            }
            Statement::ForInStatement(loop_) => {
                self.expression(&loop_.right);
                self.statement(&loop_.body);
            }
            Statement::ForOfStatement(loop_) => {
                self.expression(&loop_.right);
                self.statement(&loop_.body);
            }
            Statement::WhileStatement(loop_) => {
                self.expression(&loop_.test);
                self.statement(&loop_.body);
            }
            Statement::DoWhileStatement(loop_) => {
                self.statement(&loop_.body);
                self.expression(&loop_.test);
            }
            Statement::SwitchStatement(switch) => {
                self.expression(&switch.discriminant);
                for case in &switch.cases {
                    if let Some(test) = &case.test {
                        self.expression(test);
                    }
                    self.statements(&case.consequent);
                }
            }
            Statement::ThrowStatement(thrown) => self.expression(&thrown.argument),
            Statement::LabeledStatement(labeled) => self.statement(&labeled.body),
            Statement::WithStatement(with) => {
                self.expression(&with.object);
                self.statement(&with.body);
            }
            Statement::FunctionDeclaration(_) | Statement::ClassDeclaration(_) => self.nested += 1,
            _ => {}
        }
    }

    fn expression(&mut self, expression: &'a Expression<'a>) {
        match expression.without_parentheses() {
            Expression::CallExpression(call) => {
                self.calls.push(call);
                self.expression(&call.callee);
                for argument in call_arguments(call) {
                    self.expression(argument);
                }
            }
            Expression::NewExpression(new) => {
                self.expression(&new.callee);
                for argument in new_arguments(new) {
                    self.expression(argument);
                }
            }
            Expression::BinaryExpression(binary) => {
                self.expression(&binary.left);
                self.expression(&binary.right);
            }
            Expression::LogicalExpression(logical) => {
                self.expression(&logical.left);
                self.expression(&logical.right);
            }
            Expression::UnaryExpression(unary) => self.expression(&unary.argument),
            Expression::AssignmentExpression(assignment) => {
                self.expression(&assignment.right);
            }
            Expression::ConditionalExpression(conditional) => {
                self.expression(&conditional.test);
                self.expression(&conditional.consequent);
                self.expression(&conditional.alternate);
            }
            Expression::StaticMemberExpression(member) => self.expression(&member.object),
            Expression::ComputedMemberExpression(member) => {
                self.expression(&member.object);
                self.expression(&member.expression);
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    if let Some(expression) = element.as_expression() {
                        self.expression(expression);
                    }
                }
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.expression(expression);
                }
            }
            Expression::TemplateLiteral(template) => {
                for expression in &template.expressions {
                    self.expression(expression);
                }
            }
            Expression::TaggedTemplateExpression(tagged) => {
                self.expression(&tagged.tag);
                for expression in &tagged.quasi.expressions {
                    self.expression(expression);
                }
            }
            Expression::AwaitExpression(await_) => self.expression(&await_.argument),
            Expression::YieldExpression(yield_) => {
                if let Some(argument) = &yield_.argument {
                    self.expression(argument);
                }
            }
            Expression::ArrowFunctionExpression(_)
            | Expression::FunctionExpression(_)
            | Expression::ClassExpression(_) => self.nested += 1,
            _ => {}
        }
    }
}

// ===========================================================================
// The proof
// ===========================================================================

fn prove<'a>(model: &Model<'_, 'a>, site: FunctionSite, ops: &Ops<'a>) -> Option<()> {
    // Nested functions are counted for the audit trail, never entered; the
    // count is deliberately not a recognition gate.
    let _ = ops.nested;
    let mut gets: Vec<GetCall<'a>> = Vec::new();
    let mut sets: Vec<SetCall<'a>> = Vec::new();
    for call in &ops.calls {
        let call = *call;
        if let Some(receiver) = call_member(call, "getItem") {
            let arguments = call_arguments(call);
            if arguments.len() != call.arguments.len() {
                continue;
            }
            let [key] = arguments.as_slice() else {
                continue;
            };
            let Some(storage) = storage_identity(model, site, receiver) else {
                continue;
            };
            let Some(key) = key_origin(model, site, *key) else {
                continue;
            };
            gets.push(GetCall { call, storage, key });
        } else if let Some(receiver) = call_member(call, "setItem") {
            let arguments = call_arguments(call);
            if arguments.len() != call.arguments.len() {
                continue;
            }
            let [key, value] = arguments.as_slice() else {
                continue;
            };
            let Some(storage) = storage_identity(model, site, receiver) else {
                continue;
            };
            let Some(key) = key_origin(model, site, *key) else {
                continue;
            };
            sets.push(SetCall {
                call,
                storage,
                key,
                value: *value,
            });
        }
    }

    // Pair A: the direct persistent key receives the transaction token.
    let mut pair_a: Vec<(StorageId, SymbolId)> = Vec::new();
    for get in &gets {
        if get.key.derived {
            continue;
        }
        let Some(condition) = negated_call_condition(&ops.conditionals, get.call) else {
            continue;
        };
        if guarded_span(model, site, &ops.conditionals, get.storage, get.call.span).is_none() {
            continue;
        }
        for set in &sets {
            if set.storage != get.storage || set.key != get.key {
                continue;
            }
            if !condition
                .consequent
                .span()
                .contains_inclusive(set.call.span)
            {
                continue;
            }
            if token_origin(model, site, set.value) != Some(get.key.config) {
                continue;
            }
            let candidate = (get.storage, get.key.config);
            if !pair_a.contains(&candidate) {
                pair_a.push(candidate);
            }
        }
    }
    let (storage, config) = match pair_a.as_slice() {
        [pair] => *pair,
        _ => return None,
    };

    // Pair B: the prefixed key receives a generated timestamp/entropy string.
    let mut pair_b: Vec<(StorageId, SymbolId)> = Vec::new();
    for get in &gets {
        if !get.key.derived || get.key.config != config || get.storage != storage {
            continue;
        }
        let Some(condition) = negated_call_condition(&ops.conditionals, get.call) else {
            continue;
        };
        if guarded_span(model, site, &ops.conditionals, get.storage, get.call.span).is_none() {
            continue;
        }
        for set in &sets {
            if set.storage != get.storage || set.key != get.key {
                continue;
            }
            if !condition
                .consequent
                .span()
                .contains_inclusive(set.call.span)
            {
                continue;
            }
            if !generated_seed(model, site, set.value) {
                continue;
            }
            let candidate = (get.storage, get.key.config);
            if !pair_b.contains(&candidate) {
                pair_b.push(candidate);
            }
        }
    }
    (pair_b.len() == 1).then_some(())
}

pub(in crate::naming) fn evaluate<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
    {
        return Vec::new();
    }
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    let mut ops = Ops::default();
    ops.body(body);
    if prove(model, site, &ops).is_none() {
        return Vec::new();
    }
    vec![proposal(NAME, EVALUATOR, DETAIL, f)]
}
