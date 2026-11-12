//! Bounded own-body proof for the Navigation-API form-submit interception.
//!
//! Two independent roles live inside the form-submit interceptor family whose
//! outer installer is already named by `request_output_carriers`
//! (`install_form_submit_interception`). Neither role below inherits evidence
//! from that installer parent, and this rule never claims the installer itself.
//!
//! ```text
//! // role 1: the "navigate" event handler
//! function handler(event) {
//!   let data = event.formData;
//!   let url  = event.destination.url;
//!   try {
//!     if (data != null) {
//!       let forms = window.document.getElementsByTagName("form");
//!       let index = 0;
//!       let matched = null;
//!       while (index < forms.length) {
//!         let form = forms[index];
//!         if (form.action === url) {
//!           if (matched == null) { ++index; matched = form; continue; }
//!           data = { form: null, formData: data };   // or with url inline
//!           data.url = url;
//!           data = registry.notify(data);
//!           if (data !== null) {
//!             if (event.canIntercept && data.cancelSubmission) {
//!               /* captured NavigateEvent.prototype.intercept.originals.value.call(event) */
//!             }
//!             if (data.updatedFormData != null) {
//!               if (window.navigation != null)
//!                 window.navigation.removeEventListener("navigate", handler);
//!               executor(url, data.updatedFormData, null);
//!             }
//!           }
//!           return void 0;
//!         } else { ++index; continue; }
//!       }
//!       data = { form: matched, formData: data };
//!       data.url = url;
//!       data = registry.notify(data);
//!       /* same response block, executor(url, data.updatedFormData, matched) */
//!     }
//!   } catch (e) {}
//!   return void 0;
//! }
//!
//! // role 2: the hidden-form executor
//! function executor(url, data, options) {
//!   let form = window.document.createElement("form");
//!   let action;
//!   if (options != null) {
//!     form.target = options.target; form.enctype = options.enctype;
//!     form.acceptCharset = options.acceptCharset; form.method = options.method;
//!     action = options.action;
//!   } else { form.method = "POST"; action = url; }
//!   form.action = action;
//!   data.forEach((value, name) => {
//!     let input = window.document.createElement("input");
//!     input.type = "hidden"; input.name = name; input.value = value;
//!     form.appendChild(input);
//!   });
//!   form.setAttribute("style", "...");
//!   (window.document.body || window.document.head).appendChild(form);
//!   /* captured HTMLFormElement.prototype.submit.originals.value.call(form) */
//!   form.parentElement.removeChild(form);
//! }
//! ```
//!
//! # Required invariants
//!
//! * Scope-resolved identity only. Every local, formal and captured helper is
//!   compared through the existing scope index (`SymbolId`); the `event` read,
//!   the submission record, the registry receiver, the captured native
//!   `intercept`/`submit` and the executor callee are all proven by the same
//!   binding, never by a spelling, position, vendor or fingerprint.
//! * Exact captured-native identity. A `captured.originals.value` holder is not
//!   evidence by itself: an arbitrary captured root with an `originals.value`
//!   member would otherwise satisfy the same `.call(event)`/`.call(form)` shape.
//!   The root is resolved through its own scoping metadata (`Model::symbol_value`
//!   on the exact `SymbolId`) and is accepted only when its initializer is a
//!   capture call whose receiver is the exact unshadowed native prototype and
//!   whose property is the exact native method: `window.NavigateEvent[?].prototype`
//!   + `"intercept"` for role 1 and `window.HTMLFormElement.prototype` +
//!   `"submit"` for role 2. The initializer must live in a bounded ancestor
//!   (parent-owned) function body, never an unrelated sibling body, and the
//!   holder must be a body-local.
//! * Role 1 requires all of: `event.formData`, `event.destination.url`,
//!   `window.document.getElementsByTagName("form")`, a strict `form.action ===
//!   url` comparison, a `{ form, formData }` (optionally `url`) submission held
//!   in a local and passed to a captured `.notify`, the exact
//!   `event.canIntercept && response.cancelSubmission` guard, the captured
//!   `NavigateEvent.prototype.intercept` `.originals.value.call(event)`,
//!   `removeEventListener("navigate", <this>)`, and a
//!   `(url, response.updatedFormData, matched|null)` call into a function that
//!   independently proves role 2. The record's `form` field is exactly the
//!   proven `matched` local or the explicit `null` fallback, every key appears
//!   at most once, and duplicate keys reject the literal.
//! * Role 2 requires all of: `createElement("form")` through the captured
//!   document, `target`/`enctype`/`acceptCharset` copied from the options
//!   formal, `method` copied from options plus the `"POST"` fallback, an
//!   `action` from `options.action` or the url formal (directly or through a
//!   body-local), a `formData.forEach` callback that builds one hidden `input`
//!   per entry and appends it to that same form, a `style` attribute on the
//!   form, a `document.body`/`document.head` append of the form, the captured
//!   `HTMLFormElement.prototype.submit` `.originals.value.call(form)`, and
//!   `form.parentElement.removeChild(form)`.
//!
//! Every named participant in one role is the very same resolved symbol in the
//! other, so a wrong receiver, a wrong property key, a wrong guard, a wrong
//! captured native, a shadowed local, an unrelated record form or a decoy that
//! drops one of the operations is refused. Generated spellings, byte offsets,
//! vendors, body hashes and inherited labels are never read. The older
//! classifier stages are untouched: this rule runs only after every earlier
//! generation has declined the function.
use super::*;
use oxc_ast::ast::ChainElement;
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator};

type Symbol = SymbolId;

const NAVIGATE_HANDLER: &str = "handleNavigationFormSubmit";
const HIDDEN_FORM_EXECUTOR: &str = "submitFormViaHiddenForm";
const EVALUATOR: &str = "thread_posthost_navigation_submit";

/// The exact captured native each role is allowed to forward.
///
/// The handler's cancellation call site is the Navigation-API `intercept`
/// method (`window.NavigateEvent?.prototype.intercept`); the discovery role
/// description labels that path "preventDefault", but the tracer captures the
/// `"intercept"` property, so identity is proved against the source it really
/// is. The executor's native is `HTMLFormElement.prototype.submit`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CapturedNative {
    NavigateIntercept,
    FormSubmit,
}

impl CapturedNative {
    /// The exact native property name the capture call must request.
    fn property(self) -> &'static str {
        match self {
            Self::NavigateIntercept => "intercept",
            Self::FormSubmit => "submit",
        }
    }

    /// The unshadowed global whose prototype the capture call must receive.
    fn prototype_global(self) -> &'static str {
        match self {
            Self::NavigateIntercept => "NavigateEvent",
            Self::FormSubmit => "HTMLFormElement",
        }
    }
}

// ---------------------------------------------------------------------------
// Small AST readers
// ---------------------------------------------------------------------------

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn is_symbol(model: &Model<'_, '_>, expression: &Expression<'_>, expected: Symbol) -> bool {
    symbol(model, expression) == Some(expected)
}

fn static_member<'e, 'a>(expression: &'e Expression<'a>, name: &str) -> Option<&'e Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

/// `expression.name` for a static member, including the member at the head of
/// an optional chain (`window.NavigateEvent?.prototype` is a `ChainExpression`
/// wrapping the optional `.prototype` member). The optional flag is irrelevant
/// here: the capture call is what proves the receiver.
fn member_named<'e, 'a>(expression: &'e Expression<'a>, name: &str) -> Option<&'e Expression<'a>> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => {
            (member.property.name == name).then_some(&member.object)
        }
        Expression::ChainExpression(chain) => match &chain.expression {
            ChainElement::StaticMemberExpression(member) if member.property.name == name => {
                Some(&member.object)
            }
            _ => None,
        },
        _ => None,
    }
}

/// `window.<name>` where `window` is the unshadowed ambient global.
fn window_global_member(model: &Model<'_, '_>, expression: &Expression<'_>, name: &str) -> bool {
    member_named(expression, name).is_some_and(|object| is_unbound_global(model, object, "window"))
}

/// `<unshadowed window>.<global>.prototype` (the `.prototype` member may be
/// optional, as in `window.NavigateEvent?.prototype`).
fn is_native_prototype(model: &Model<'_, '_>, expression: &Expression<'_>, global: &str) -> bool {
    member_named(expression, "prototype")
        .is_some_and(|object| window_global_member(model, object, global))
}

fn string_literal<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression.without_parentheses() {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn is_null(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::NullLiteral(_))
}

fn call_expression<'e, 'a>(expression: &'e Expression<'a>) -> Option<&'e CallExpression<'a>> {
    match expression.without_parentheses() {
        Expression::CallExpression(call) if !call.optional => Some(call),
        _ => None,
    }
}

fn plain_binding(model: &Model<'_, '_>, pattern: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = pattern else {
        return None;
    };
    let value = identifier.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(value)
        .is_empty()
        .then_some(value)
}

fn in_body(model: &Model<'_, '_>, site: FunctionSite, value: Symbol) -> bool {
    let body = site.body_span.unwrap_or(site.span);
    let span = model.scoping.symbol_span(value);
    span.start >= body.start && span.end <= body.end
}

fn captured(model: &Model<'_, '_>, site: FunctionSite, value: Symbol) -> bool {
    !in_body(model, site, value) && model.scoping.symbol_redeclarations(value).is_empty()
}

fn target_identifier(model: &Model<'_, '_>, target: &AssignmentTarget<'_>) -> Option<Symbol> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, model.scoping)
        }
        _ => None,
    }
}

/// `(object, property)` of a plain `object.property = value` assignment.
fn written_member<'e, 'a>(
    assignment: &'e AssignmentExpression<'a>,
) -> Option<(&'e Expression<'a>, &'e str)> {
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
        return None;
    };
    (!member.optional).then_some((&member.object, member.property.name.as_str()))
}

fn plain_formals(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> Option<Vec<Symbol>> {
    if params.rest.is_some() {
        return None;
    }
    let mut formals = Vec::with_capacity(params.items.len());
    for parameter in &params.items {
        if parameter.initializer.is_some() {
            return None;
        }
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        let value = identifier.symbol_id.get()?;
        if !model.scoping.symbol_redeclarations(value).is_empty()
            || model.binding_writes(value) != 0
        {
            return None;
        }
        formals.push(value);
    }
    Some(formals)
}

fn function_site_parts<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
) -> Option<(&'a FormalParameters<'a>, &'a FunctionBody<'a>)> {
    match model.nodes.kind(site.node_id) {
        AstKind::Function(function) => {
            let body = function.body.as_ref()?;
            Some((&*function.params, &**body))
        }
        AstKind::ArrowFunctionExpression(arrow) => Some((&*arrow.params, &*arrow.body)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Bounded own-body collection (nested functions are never entered)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Own<'a> {
    declarators: Vec<&'a VariableDeclarator<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    calls: Vec<&'a CallExpression<'a>>,
    members: Vec<(&'a Expression<'a>, &'a str)>,
    binary: Vec<&'a BinaryExpression<'a>>,
    logical: Vec<&'a LogicalExpression<'a>>,
    objects: Vec<&'a ObjectExpression<'a>>,
}

fn collect_own<'a>(body: &'a FunctionBody<'a>, own: &mut Own<'a>) {
    for statement in &body.statements {
        walk_statement(statement, own);
    }
}

fn walk_block<'a>(block: &'a BlockStatement<'a>, own: &mut Own<'a>) {
    for statement in &block.body {
        walk_statement(statement, own);
    }
}

fn walk_statement<'a>(statement: &'a Statement<'a>, own: &mut Own<'a>) {
    match statement {
        Statement::BlockStatement(block) => {
            for inner in &block.body {
                walk_statement(inner, own);
            }
        }
        Statement::ExpressionStatement(statement) => walk_expression(&statement.expression, own),
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                own.declarators.push(declarator);
                if let Some(init) = &declarator.init {
                    walk_expression(init, own);
                }
            }
        }
        Statement::IfStatement(branch) => {
            walk_expression(&branch.test, own);
            walk_statement(&branch.consequent, own);
            if let Some(alternate) = &branch.alternate {
                walk_statement(alternate, own);
            }
        }
        Statement::WhileStatement(loop_) => {
            walk_expression(&loop_.test, own);
            walk_statement(&loop_.body, own);
        }
        Statement::DoWhileStatement(loop_) => {
            walk_statement(&loop_.body, own);
            walk_expression(&loop_.test, own);
        }
        Statement::ForStatement(loop_) => {
            if let Some(init) = &loop_.init {
                if let ForStatementInit::VariableDeclaration(declaration) = init {
                    for declarator in &declaration.declarations {
                        own.declarators.push(declarator);
                        if let Some(value) = &declarator.init {
                            walk_expression(value, own);
                        }
                    }
                }
            }
            if let Some(test) = &loop_.test {
                walk_expression(test, own);
            }
            if let Some(update) = &loop_.update {
                walk_expression(update, own);
            }
            walk_statement(&loop_.body, own);
        }
        Statement::ForInStatement(loop_) => {
            walk_expression(&loop_.right, own);
            walk_statement(&loop_.body, own);
        }
        Statement::ForOfStatement(loop_) => {
            walk_expression(&loop_.right, own);
            walk_statement(&loop_.body, own);
        }
        Statement::ReturnStatement(statement) => {
            if let Some(argument) = &statement.argument {
                walk_expression(argument, own);
            }
        }
        Statement::ThrowStatement(statement) => walk_expression(&statement.argument, own),
        Statement::TryStatement(statement) => {
            walk_block(&statement.block, own);
            if let Some(handler) = &statement.handler {
                walk_block(&handler.body, own);
            }
            if let Some(finalizer) = &statement.finalizer {
                walk_block(finalizer, own);
            }
        }
        Statement::LabeledStatement(statement) => walk_statement(&statement.body, own),
        _ => {}
    }
}

fn walk_expression<'a>(expression: &'a Expression<'a>, own: &mut Own<'a>) {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => {
            own.members
                .push((&member.object, member.property.name.as_str()));
            walk_expression(&member.object, own);
        }
        Expression::ComputedMemberExpression(member) => {
            walk_expression(&member.object, own);
            walk_expression(&member.expression, own);
        }
        Expression::CallExpression(call) => {
            own.calls.push(call);
            walk_expression(&call.callee, own);
            for argument in &call.arguments {
                match argument {
                    Argument::SpreadElement(spread) => walk_expression(&spread.argument, own),
                    _ => {
                        if let Some(value) = argument.as_expression() {
                            walk_expression(value, own);
                        }
                    }
                }
            }
        }
        Expression::NewExpression(new) => {
            walk_expression(&new.callee, own);
            for argument in &new.arguments {
                if let Some(value) = argument.as_expression() {
                    walk_expression(value, own);
                }
            }
        }
        Expression::BinaryExpression(binary) => {
            own.binary.push(binary);
            walk_expression(&binary.left, own);
            walk_expression(&binary.right, own);
        }
        Expression::LogicalExpression(logical) => {
            own.logical.push(logical);
            walk_expression(&logical.left, own);
            walk_expression(&logical.right, own);
        }
        Expression::UnaryExpression(unary) => walk_expression(&unary.argument, own),
        Expression::AssignmentExpression(assignment) => {
            own.assignments.push(assignment);
            walk_target(&assignment.left, own);
            walk_expression(&assignment.right, own);
        }
        Expression::ConditionalExpression(conditional) => {
            walk_expression(&conditional.test, own);
            walk_expression(&conditional.consequent, own);
            walk_expression(&conditional.alternate, own);
        }
        Expression::SequenceExpression(sequence) => {
            for value in &sequence.expressions {
                walk_expression(value, own);
            }
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                match element {
                    ArrayExpressionElement::SpreadElement(spread) => {
                        walk_expression(&spread.argument, own)
                    }
                    ArrayExpressionElement::Elision(_) => {}
                    _ => {
                        if let Some(value) = element.as_expression() {
                            walk_expression(value, own);
                        }
                    }
                }
            }
        }
        Expression::ObjectExpression(object) => {
            own.objects.push(object);
            for property in &object.properties {
                if let ObjectPropertyKind::ObjectProperty(property) = property {
                    walk_expression(&property.value, own);
                }
            }
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            walk_expression(&parenthesized.expression, own)
        }
        Expression::TemplateLiteral(template) => {
            for value in &template.expressions {
                walk_expression(value, own);
            }
        }
        // Nested function/arrow bodies belong to their own site, never this one.
        Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ClassExpression(_) => {}
        _ => {}
    }
}

fn walk_target<'a>(target: &'a AssignmentTarget<'a>, own: &mut Own<'a>) {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => {
            walk_expression(&member.object, own);
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            walk_expression(&member.object, own);
            walk_expression(&member.expression, own);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Document receiver and captured-native helpers
// ---------------------------------------------------------------------------

fn is_unbound_global(model: &Model<'_, '_>, expression: &Expression<'_>, name: &str) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::Identifier(identifier)
            if identifier.name == name
                && crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
    )
}

/// `window.document`, or the unbound global `document` itself.
fn is_document(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    if static_member(expression, "document")
        .is_some_and(|object| is_unbound_global(model, object, "window"))
    {
        return true;
    }
    is_unbound_global(model, expression, "document")
}

fn is_body_or_head(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    if static_member(expression, "body").is_some_and(|object| is_document(model, object)) {
        return true;
    }
    if static_member(expression, "head").is_some_and(|object| is_document(model, object)) {
        return true;
    }
    if let Expression::LogicalExpression(logical) = expression.without_parentheses() {
        if logical.operator == LogicalOperator::Or {
            return is_body_or_head(model, &logical.left) && is_body_or_head(model, &logical.right);
        }
    }
    false
}

/// Whether `span` lives in a function body that physically encloses `site`
/// (a bounded parent-owned context). A capture initializer in an unrelated
/// sibling function is never evidence.
fn in_ancestor_body(model: &Model<'_, '_>, site: FunctionSite, span: Span) -> bool {
    model.all_sites().iter().any(|ancestor| {
        ancestor.span != site.span
            && ancestor.span.start <= site.span.start
            && ancestor.span.end >= site.span.end
            && {
                let body = ancestor.body_span.unwrap_or(ancestor.span);
                span.start >= body.start && span.end <= body.end
            }
    })
}

/// The exact captured native root: the root binding is resolved through its own
/// scoping metadata (`Model::symbol_value` on the exact `SymbolId`) to a
/// parent-owned capture call for `native`. A merely captured root that happens
/// to expose an `originals.value` member is not enough.
fn captured_native_root(
    model: &Model<'_, '_>,
    site: FunctionSite,
    root: Symbol,
    native: CapturedNative,
) -> bool {
    if !captured(model, site, root) {
        return false;
    }
    let Some(value) = model.symbol_value(root) else {
        return false;
    };
    if !in_ancestor_body(model, site, value.span()) {
        return false;
    }
    let Some(call) = call_expression(value) else {
        return false;
    };
    let Some(callee) = symbol(model, &call.callee) else {
        return false;
    };
    if !captured(model, site, callee)
        || !in_ancestor_body(model, site, model.scoping.symbol_span(callee))
    {
        return false;
    }
    let arguments = call_arguments(call);
    let [receiver, property] = arguments.as_slice() else {
        return false;
    };
    string_literal(property) == Some(native.property())
        && is_native_prototype(model, receiver, native.prototype_global())
}

/// `<exact captured native>.originals.value`, the tracer's slot for a captured
/// native call. Only the scoping-proven native root qualifies.
fn captured_originals_value(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    native: CapturedNative,
) -> Option<Symbol> {
    let value = static_member(expression, "value")?;
    let originals = static_member(value, "originals")?;
    let root = symbol(model, originals)?;
    captured_native_root(model, site, root, native).then_some(root)
}

/// A body-local assigned the exact captured native's `.originals.value`.
fn captured_call_holder<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    own: &Own<'a>,
    native: CapturedNative,
) -> Option<Symbol> {
    own.assignments.iter().copied().find_map(|assignment| {
        if assignment.operator != AssignmentOperator::Assign {
            return None;
        }
        let holder = target_identifier(model, &assignment.left)?;
        if !in_body(model, site, holder) {
            return None;
        }
        captured_originals_value(model, site, &assignment.right, native)?;
        Some(holder)
    })
}

/// Whether `element` is a body-local that only ever holds `document.body` /
/// `document.head`.
fn holds_document_body_head<'a>(
    model: &Model<'_, '_>,
    own: &Own<'a>,
    element: &Expression<'_>,
) -> bool {
    let Some(local) = symbol(model, element) else {
        return false;
    };
    if own.declarators.iter().copied().any(|declarator| {
        plain_binding(model, &declarator.id) == Some(local)
            && declarator
                .init
                .as_ref()
                .is_some_and(|init| is_body_or_head(model, init))
    }) {
        return true;
    }
    own.assignments.iter().copied().any(|assignment| {
        target_identifier(model, &assignment.left) == Some(local)
            && is_body_or_head(model, &assignment.right)
    })
}

// ---------------------------------------------------------------------------
// Role 1: the navigate handler
// ---------------------------------------------------------------------------

/// `{ form, formData }` (with an optional `url`) whose `form` is exactly the
/// proven `matched` local or the explicit `null` fallback and whose `formData`
/// is the exact `event.formData` binding. Duplicate keys are refused. Returns
/// whether `url` was inlined.
fn submission_literal(
    model: &Model<'_, '_>,
    object: &ObjectExpression<'_>,
    form_data: Symbol,
    url: Symbol,
    matched: Symbol,
) -> Option<bool> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut form_seen = false;
    let mut data_seen = false;
    let mut url_seen = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed
            || property.method
            || property.shorthand
            || property.kind != PropertyKind::Init
        {
            return None;
        }
        let key = property_key_name(&property.key)?;
        if !seen.insert(key.clone()) {
            // A repeated key silently overwrites the earlier value; it cannot
            // prove the record the downstream role consumes.
            return None;
        }
        match key.as_str() {
            "form" => {
                // Only the exact matched-form local or the explicit null
                // fallback is the record's form. Any other non-null symbol
                // (registry, tag list, unrelated local) is a disconnected form.
                if !(is_null(&property.value) || is_symbol(model, &property.value, matched)) {
                    return None;
                }
                form_seen = true;
            }
            "formData" => {
                if !is_symbol(model, &property.value, form_data) {
                    return None;
                }
                data_seen = true;
            }
            "url" => {
                if !is_symbol(model, &property.value, url) {
                    return None;
                }
                url_seen = true;
            }
            _ => return None,
        }
    }
    (form_seen && data_seen).then_some(url_seen)
}

fn navigation_handler<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    params: &'a FormalParameters<'a>,
    body: &'a FunctionBody<'a>,
) -> Option<()> {
    let formals = plain_formals(model, params)?;
    let [event] = formals.as_slice() else {
        return None;
    };
    let event = *event;

    let mut own = Own::default();
    collect_own(body, &mut own);

    // event.formData
    let form_data = own.declarators.iter().copied().find_map(|declarator| {
        let init = declarator.init.as_ref()?;
        let value = static_member(init, "formData")?;
        is_symbol(model, value, event).then(|| plain_binding(model, &declarator.id))?
    })?;
    if form_data == event {
        return None;
    }

    // event.destination.url
    let url = own.declarators.iter().copied().find_map(|declarator| {
        let init = declarator.init.as_ref()?;
        let value = static_member(init, "url")?;
        let destination = static_member(value, "destination")?;
        is_symbol(model, destination, event).then(|| plain_binding(model, &declarator.id))?
    })?;
    if url == event || url == form_data {
        return None;
    }

    // window.document.getElementsByTagName("form")
    let forms = own.declarators.iter().copied().find_map(|declarator| {
        let call = call_expression(declarator.init.as_ref()?)?;
        let receiver = static_member(&call.callee, "getElementsByTagName")?;
        if !is_document(model, receiver) {
            return None;
        }
        let arguments = call_arguments(call);
        let [argument] = arguments.as_slice() else {
            return None;
        };
        if string_literal(argument) != Some("form") {
            return None;
        }
        plain_binding(model, &declarator.id)
    })?;
    if forms == event || forms == form_data || forms == url {
        return None;
    }

    // The loop form local, read out of the tag list: `let form = forms[index];`
    let form = own.declarators.iter().copied().find_map(|declarator| {
        let Expression::ComputedMemberExpression(member) =
            declarator.init.as_ref()?.without_parentheses()
        else {
            return None;
        };
        if !is_symbol(model, &member.object, forms) {
            return None;
        }
        let local = plain_binding(model, &declarator.id)?;
        (local != forms && local != form_data && local != url && local != event).then_some(local)
    })?;

    // `form.action === url`
    let compares_action = own.binary.iter().copied().any(|binary| {
        if binary.operator != BinaryOperator::StrictEquality {
            return false;
        }
        let left = static_member(&binary.left, "action")
            .is_some_and(|object| is_symbol(model, object, form))
            && is_symbol(model, &binary.right, url);
        let right = static_member(&binary.right, "action")
            .is_some_and(|object| is_symbol(model, object, form))
            && is_symbol(model, &binary.left, url);
        left || right
    });
    if !compares_action {
        return None;
    }

    // `matched = form` and `matched == null`
    let matched = own.assignments.iter().copied().find_map(|assignment| {
        if assignment.operator != AssignmentOperator::Assign {
            return None;
        }
        let target = target_identifier(model, &assignment.left)?;
        is_symbol(model, &assignment.right, form).then_some(target)
    })?;
    if matched == form
        || matched == forms
        || matched == form_data
        || matched == url
        || matched == event
    {
        return None;
    }
    let guards_matched = own.binary.iter().copied().any(|binary| {
        if !matches!(
            binary.operator,
            BinaryOperator::Equality | BinaryOperator::StrictEquality
        ) {
            return false;
        }
        (is_symbol(model, &binary.left, matched) && is_null(&binary.right))
            || (is_null(&binary.left) && is_symbol(model, &binary.right, matched))
    });
    if !guards_matched {
        return None;
    }

    // The submission record `{ form, formData }` held in one local.
    let mut submission: Option<(Symbol, bool)> = None;
    for object in &own.objects {
        let Some(inline_url) = submission_literal(model, object, form_data, url, matched) else {
            continue;
        };
        for assignment in own.assignments.iter().copied() {
            if assignment.operator != AssignmentOperator::Assign
                || assignment.right.span() != object.span()
            {
                continue;
            }
            if let Some(holder) = target_identifier(model, &assignment.left) {
                submission = Some((holder, inline_url));
            }
        }
    }
    let (record, inline_url) = submission?;
    if !inline_url {
        let sets_url = own.assignments.iter().copied().any(|assignment| {
            assignment.operator == AssignmentOperator::Assign
                && matches!(
                    &assignment.left,
                    AssignmentTarget::StaticMemberExpression(member)
                        if !member.optional
                            && member.property.name == "url"
                            && is_symbol(model, &member.object, record)
                )
                && is_symbol(model, &assignment.right, url)
        });
        if !sets_url {
            return None;
        }
    }

    // `registry.notify(record)` through a captured registry.
    let notifies = own.calls.iter().copied().any(|call| {
        let receiver = static_member(&call.callee, "notify");
        receiver.is_some_and(|receiver| {
            symbol(model, receiver).is_some_and(|value| captured(model, site, value))
        }) && call_arguments(call)
            .first()
            .is_some_and(|argument| is_symbol(model, argument, record))
    });
    if !notifies {
        return None;
    }

    // `event.canIntercept && record.cancelSubmission`
    let guard = own.logical.iter().copied().any(|logical| {
        logical.operator == LogicalOperator::And
            && static_member(&logical.left, "canIntercept")
                .is_some_and(|object| is_symbol(model, object, event))
            && static_member(&logical.right, "cancelSubmission")
                .is_some_and(|object| is_symbol(model, object, record))
    });
    if !guard {
        return None;
    }

    // Captured NavigateEvent.prototype.intercept forwarded onto the exact event.
    let holder = captured_call_holder(model, site, &own, CapturedNative::NavigateIntercept)?;
    let cancels = own.calls.iter().copied().any(|call| {
        static_member(&call.callee, "call").is_some_and(|object| is_symbol(model, object, holder))
            && call_arguments(call)
                .iter()
                .any(|argument| is_symbol(model, argument, event))
    });
    if !cancels {
        return None;
    }

    // `record.updatedFormData != null`
    let reads_updated = own.binary.iter().copied().any(|binary| {
        if !matches!(
            binary.operator,
            BinaryOperator::Inequality | BinaryOperator::StrictInequality
        ) {
            return false;
        }
        (static_member(&binary.left, "updatedFormData")
            .is_some_and(|object| is_symbol(model, object, record))
            && is_null(&binary.right))
            || (is_null(&binary.left)
                && static_member(&binary.right, "updatedFormData")
                    .is_some_and(|object| is_symbol(model, object, record)))
    });
    if !reads_updated {
        return None;
    }

    // Removes this very handler from the "navigate" listener.
    let removes_itself = own.calls.iter().copied().any(|call| {
        static_member(&call.callee, "removeEventListener").is_some() && {
            let arguments = call_arguments(call);
            arguments.len() == 2
                && string_literal(arguments[0]) == Some("navigate")
                && model
                    .function_from_expression(arguments[1])
                    .is_some_and(|target| target.span == site.span)
        }
    });
    if !removes_itself {
        return None;
    }

    // `executor(url, record.updatedFormData, matched | null)` into the exact executor.
    let routes = own.calls.iter().copied().any(|call| {
        let arguments = call_arguments(call);
        let [first, second, third] = arguments.as_slice() else {
            return false;
        };
        let Some(target) = model.function_from_expression(&call.callee) else {
            return false;
        };
        if target.span == site.span {
            return false;
        }
        if !is_symbol(model, first, url) {
            return false;
        }
        if !static_member(second, "updatedFormData")
            .is_some_and(|object| is_symbol(model, object, record))
        {
            return false;
        }
        if !(is_null(third) || is_symbol(model, third, matched)) {
            return false;
        }
        let Some((target_params, target_body)) = function_site_parts(model, target) else {
            return false;
        };
        hidden_form_executor(model, target, target_params, target_body).is_some()
    });
    if !routes {
        return None;
    }

    Some(())
}

// ---------------------------------------------------------------------------
// Role 2: the hidden-form executor
// ---------------------------------------------------------------------------

/// The `formData.forEach` callback that builds one hidden input per entry and
/// appends every input to the same form.
fn hidden_inputs<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    params: &'a FormalParameters<'a>,
    body: &'a FunctionBody<'a>,
    form: Symbol,
) -> bool {
    let Some(formals) = plain_formals(model, params) else {
        return false;
    };
    let [value_formal, name_formal] = formals.as_slice() else {
        return false;
    };
    let mut own = Own::default();
    collect_own(body, &mut own);

    let Some(input) = own.declarators.iter().copied().find_map(|declarator| {
        let call = call_expression(declarator.init.as_ref()?)?;
        let receiver = static_member(&call.callee, "createElement")?;
        if !is_document(model, receiver) {
            return None;
        }
        let arguments = call_arguments(call);
        let [argument] = arguments.as_slice() else {
            return None;
        };
        if string_literal(argument) != Some("input") {
            return None;
        }
        plain_binding(model, &declarator.id)
    }) else {
        return false;
    };
    if input == form {
        return false;
    }

    let mut writes: BTreeMap<&str, Vec<&Expression<'_>>> = BTreeMap::new();
    for assignment in own.assignments.iter().copied() {
        if let Some((object, property)) = written_member(assignment) {
            if is_symbol(model, object, input) {
                writes.entry(property).or_default().push(&assignment.right);
            }
        }
    }
    let writes_hidden = writes.get("type").is_some_and(|values| {
        values
            .iter()
            .copied()
            .any(|value| string_literal(value) == Some("hidden"))
    });
    let writes_name = writes.get("name").is_some_and(|values| {
        values
            .iter()
            .copied()
            .any(|value| is_symbol(model, value, *name_formal))
    });
    let writes_value = writes.get("value").is_some_and(|values| {
        values
            .iter()
            .copied()
            .any(|value| is_symbol(model, value, *value_formal))
    });
    let appends_input = own.calls.iter().copied().any(|call| {
        static_member(&call.callee, "appendChild")
            .is_some_and(|object| is_symbol(model, object, form))
            && call_arguments(call)
                .iter()
                .any(|argument| is_symbol(model, argument, input))
    });
    writes_hidden && writes_name && writes_value && appends_input
}

fn hidden_form_executor<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    params: &'a FormalParameters<'a>,
    body: &'a FunctionBody<'a>,
) -> Option<()> {
    let formals = plain_formals(model, params)?;
    let [url, form_data, options] = formals.as_slice() else {
        return None;
    };
    let (url, form_data, options) = (*url, *form_data, *options);

    let mut own = Own::default();
    collect_own(body, &mut own);

    // `let form = window.document.createElement("form");`
    let form = own.declarators.iter().copied().find_map(|declarator| {
        let call = call_expression(declarator.init.as_ref()?)?;
        let receiver = static_member(&call.callee, "createElement")?;
        if !is_document(model, receiver) {
            return None;
        }
        let arguments = call_arguments(call);
        let [argument] = arguments.as_slice() else {
            return None;
        };
        if string_literal(argument) != Some("form") {
            return None;
        }
        plain_binding(model, &declarator.id)
    })?;
    if form == url || form == form_data || form == options {
        return None;
    }

    // Member copies from the options formal and the action fallback.
    let mut writes: BTreeMap<&str, Vec<&Expression<'_>>> = BTreeMap::new();
    for assignment in own.assignments.iter().copied() {
        if let Some((object, property)) = written_member(assignment) {
            if is_symbol(model, object, form) {
                writes.entry(property).or_default().push(&assignment.right);
            }
        }
    }
    let copies = |property: &str| {
        writes.get(property).is_some_and(|values| {
            values.iter().copied().any(|value| {
                static_member(value, property)
                    .is_some_and(|object| is_symbol(model, object, options))
            })
        })
    };
    if !copies("target") || !copies("enctype") || !copies("acceptCharset") {
        return None;
    }
    let copies_method = writes.get("method").is_some_and(|values| {
        values.iter().copied().any(|value| {
            static_member(value, "method").is_some_and(|object| is_symbol(model, object, options))
        }) && values
            .iter()
            .copied()
            .any(|value| string_literal(value) == Some("POST"))
    });
    if !copies_method {
        return None;
    }
    let action_from_options = |value: &Expression<'_>| {
        static_member(value, "action").is_some_and(|object| is_symbol(model, object, options))
    };
    let action_from_url = |value: &Expression<'_>| is_symbol(model, value, url);
    let action_from_local = |value: &Expression<'_>| {
        let Some(local) = symbol(model, value) else {
            return false;
        };
        if local == form || local == form_data || local == options || local == url {
            return false;
        }
        let declares = own.declarators.iter().copied().any(|declarator| {
            plain_binding(model, &declarator.id) == Some(local)
                && declarator
                    .init
                    .as_ref()
                    .is_some_and(|init| action_from_options(init) || action_from_url(init))
        });
        let assigns = own.assignments.iter().copied().any(|assignment| {
            target_identifier(model, &assignment.left) == Some(local)
                && (action_from_options(&assignment.right) || action_from_url(&assignment.right))
        });
        declares || assigns
    };
    let writes_action = writes.get("action").is_some_and(|values| {
        values.iter().copied().any(|value| {
            action_from_options(value) || action_from_url(value) || action_from_local(value)
        })
    });
    if !writes_action {
        return None;
    }

    // `formData.forEach((value, name) => { ... })`
    let callback = own.calls.iter().copied().find_map(|call| {
        let receiver = static_member(&call.callee, "forEach")?;
        if !is_symbol(model, receiver, form_data) {
            return None;
        }
        let arguments = call_arguments(call);
        let [callback] = arguments.as_slice() else {
            return None;
        };
        Some(*callback)
    })?;
    let callback_site = model.function_from_expression(callback)?;
    if callback_site.span == site.span {
        return None;
    }
    let (callback_params, callback_body) = function_site_parts(model, callback_site)?;
    if !hidden_inputs(model, callback_site, callback_params, callback_body, form) {
        return None;
    }

    // `form.setAttribute("style", <string>)`
    let sets_style = own.calls.iter().copied().any(|call| {
        static_member(&call.callee, "setAttribute")
            .is_some_and(|object| is_symbol(model, object, form))
            && {
                let arguments = call_arguments(call);
                arguments.len() == 2
                    && string_literal(arguments[0]) == Some("style")
                    && string_literal(arguments[1]).is_some()
            }
    });
    if !sets_style {
        return None;
    }

    // The form is appended under `document.body || document.head`.
    let has_body = own
        .members
        .iter()
        .any(|(object, property)| *property == "body" && is_document(model, object));
    let has_head = own
        .members
        .iter()
        .any(|(object, property)| *property == "head" && is_document(model, object));
    if !has_body || !has_head {
        return None;
    }
    let appends_form = own.calls.iter().copied().any(|call| {
        let Some(receiver) = static_member(&call.callee, "appendChild") else {
            return false;
        };
        let receiver_ok =
            is_body_or_head(model, receiver) || holds_document_body_head(model, &own, receiver);
        receiver_ok
            && call_arguments(call)
                .iter()
                .any(|argument| is_symbol(model, argument, form))
    });
    if !appends_form {
        return None;
    }

    // Captured HTMLFormElement.prototype.submit forwarded onto the exact form.
    let holder = captured_call_holder(model, site, &own, CapturedNative::FormSubmit)?;
    let submits = own.calls.iter().copied().any(|call| {
        static_member(&call.callee, "call").is_some_and(|object| is_symbol(model, object, holder))
            && call_arguments(call)
                .iter()
                .any(|argument| is_symbol(model, argument, form))
    });
    if !submits {
        return None;
    }

    // `form.parentElement.removeChild(form)`
    let removes_form = own.calls.iter().copied().any(|call| {
        static_member(&call.callee, "removeChild").is_some_and(|object| {
            static_member(object, "parentElement")
                .is_some_and(|parent| is_symbol(model, parent, form))
        }) && call_arguments(call)
            .iter()
            .any(|argument| is_symbol(model, argument, form))
    });
    if !removes_form {
        return None;
    }

    Some(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    if function.r#async || function.generator || function.params.rest.is_some() {
        return vec![];
    }
    if !matches!(function.params.items.len(), 1 | 3) {
        return vec![];
    }
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if !body.directives.is_empty() {
        return vec![];
    }
    let params = &*function.params;

    if navigation_handler(model, site, params, body).is_some() {
        return vec![proposal(
            NAVIGATE_HANDLER,
            EVALUATOR,
            "owned navigation submit handler: reads the navigate event's \
             formData and destination.url, scans the document's form tag list \
             for the first form whose action equals that url, sends the \
             {form, formData, url} record to the captured submit registry, \
             cancels through the captured native only behind \
             canIntercept && cancelSubmission, and re-submits \
             (url, updatedFormData, form) through the proved hidden-form \
             executor after removing its own navigate listener",
            f,
        )];
    }

    if hidden_form_executor(model, site, params, body).is_some() {
        return vec![proposal(
            HIDDEN_FORM_EXECUTOR,
            EVALUATOR,
            "owned hidden-form submit executor: creates a form through the \
             captured document, copies target/enctype/acceptCharset/method/\
             action from the options formal (method falls back to POST and \
             action to the url formal), appends one hidden input per formData \
             entry through formData.forEach, sets a style attribute, appends \
             the form under document.body or document.head, invokes the \
             captured native submit on it, and removes it again",
            f,
        )];
    }

    vec![]
}
