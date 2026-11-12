//! Package `request_output_carriers`: the request/output carriers and their transport hooks.
//!
//! The package owns four carrier families and their lifecycle pairs:
//!
//! * the form-submit interception family — the capture listener, the per-form
//!   one-shot observation, the `preventDefault` / `stopPropagation` /
//!   `stopImmediatePropagation` forwarders, the submitted-button recorder and
//!   the injected token inputs;
//! * the XHR telemetry carrier — the sender and its `onerror`, `onloadend` and
//!   `onload` handlers;
//! * the `CustomEvent` signal hooks — the factory, its init and teardown
//!   dispatchers, and the `createEvent("CustomEvent")` fallback constructors;
//! * the fetch and XHR instrumentation modules and their no-op lifecycle
//!   pairs, reached through the generic `onBeforeInvoke` hook channels.
//!
//! # How this package decides
//!
//! Every claim starts from a stable host or public API name the program cannot
//! rename — `addEventListener("submit", …, true)`, `XMLHttpRequest`,
//! `createEvent("CustomEvent")`, `initCustomEvent`, `dispatchEvent`,
//! `onBeforeInvoke`, `headers.set`, `setRequestHeader`, `open`, `send`,
//! `createElement("style")`, `formaction` / `formenctype` / `formmethod`,
//! `input[type=hidden]` — or from a binding this package has already proven.
//! A function that is only handed to a call is recognized by the object it
//! returns or the structure inside its own body; a forwarder is told apart from
//! its twin by the interception bundle it is registered on, never by source
//! order.
//!
//! Both tracer shapes are read: a named formal and `arguments[N]` are one slot
//! through [`ParameterSlots`], a `name.prop` and a `name["prop"]` are one
//! member, and a callback may be an inline arrow or a hoisted declaration.
//!
//! Randomized property-name arrays are recognized by their *use* as computed
//! keys; their literal spellings are never read.
//!
//! # Fail-closed
//!
//! The plan gives every candidate the name of its role, so two bindings of one
//! role would receive the same name. Every role here names one thing in a
//! program, so [`Out::into_unique`] drops a role that matched more than one
//! span.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator};

use crate::signals::extract::bindings::{
    array_elements, numeric_index, object_members_of, static_member_name, ParameterSlots,
};
use crate::signals::schema::Confidence;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};
use crate::analysis;

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

const INSTALL_FORM_SUBMIT_INTERCEPTION: &str = "install_form_submit_interception";
const INSTALL_FORM_SUBMIT_OBSERVATION: &str = "install_form_submit_observation";
const HANDLE_OBSERVED_FORM_SUBMIT: &str = "handle_observed_form_submit";
const FORWARD_STOP_IMMEDIATE_SUBMIT: &str = "forward_stop_immediate_propagation_submit";
const HANDLE_STOP_PROPAGATION_SUBMIT: &str = "handle_stop_propagation_submit";
const HANDLE_PREVENT_DEFAULT_SUBMIT: &str = "handle_prevent_default_submit";
const HANDLE_DOCUMENT_SUBMIT_CAPTURE: &str = "handle_document_submit_capture";
const FORM_SUBMIT_BEFORE_INVOKE_CALLBACK: &str = "form_submit_before_invoke_callback";
const STOP_IMMEDIATE_BEFORE_INVOKE_CALLBACK: &str = "stop_immediate_propagation_before_invoke_callback";
const STOP_PROPAGATION_BEFORE_INVOKE_CALLBACK: &str = "stop_propagation_before_invoke_callback";
const PREVENT_DEFAULT_BEFORE_INVOKE_CALLBACK: &str = "prevent_default_before_invoke_callback";
const CANCEL_BUBBLE_BEFORE_SET_CALLBACK: &str = "cancel_bubble_before_set_callback";

const SEND_TELEMETRY_REQUEST: &str = "send_telemetry_request";
const MARK_TELEMETRY_NETWORK_ERROR: &str = "mark_telemetry_request_network_error";
const ENCODE_TELEMETRY_RESPONSE: &str = "encode_telemetry_response";
const CAPTURE_TELEMETRY_RESPONSE_TEXT: &str = "capture_telemetry_response_text";

const CREATE_CUSTOM_EVENT_SIGNAL_HOOKS: &str = "create_custom_event_signal_hooks";
const DISPATCH_SIGNAL_CONFIG_EVENTS: &str = "dispatch_signal_config_events";
const CREATE_FALLBACK_CUSTOM_EVENT: &str = "create_fallback_custom_event";
const DISPATCH_SIGNAL_TEARDOWN_EVENTS: &str = "dispatch_signal_teardown_events";
const CREATE_FALLBACK_CUSTOM_EVENT_FOR_TEARDOWN: &str = "create_fallback_custom_event_for_teardown";
const NOOP_CUSTOM_EVENT_HOOK_INIT: &str = "noop_custom_event_hook_init";
const NOOP_CUSTOM_EVENT_HOOK_TEARDOWN: &str = "noop_custom_event_hook_teardown";

const CREATE_PLATFORM_FORM_SUBMIT_HOOK: &str = "create_platform_form_submit_hook";
const RECORD_CLICKED_SUBMITTER_CANDIDATE: &str = "record_clicked_submitter_candidate";
const CLEAR_SUBMITTER_CANDIDATE: &str = "clear_submitter_candidate";
const REMOVE_INJECTED_TOKEN_INPUTS: &str = "remove_injected_token_inputs";
const REMOVE_ONE_INJECTED_TOKEN_INPUT: &str = "remove_one_injected_token_input";
const REPORT_FORM_SUBMIT_ERROR: &str = "report_form_submit_error";
const INIT_SUBMIT_SIGNAL_HOOKS: &str = "init_submit_signal_hooks";
const INJECT_SUBMIT_OPTION_INPUTS: &str = "inject_submit_option_inputs";
const CLEAR_SUBMIT_OPTION_CACHE: &str = "clear_submit_option_cache";
const TEARDOWN_SUBMIT_INSTRUMENTATION: &str = "teardown_submit_instrumentation";

const CREATE_FETCH_INSTRUMENTATION: &str = "create_fetch_instrumentation";
const INIT_FETCH_INSTRUMENTATION: &str = "init_fetch_instrumentation";
const INJECT_FETCH_REQUEST_HEADERS: &str = "inject_fetch_request_headers";
const TEARDOWN_FETCH_INSTRUMENTATION: &str = "teardown_fetch_instrumentation";
const NOOP_FETCH_HOOK_INIT: &str = "noop_fetch_hook_init";
const NOOP_FETCH_HOOK_TEARDOWN: &str = "noop_fetch_hook_teardown";

const CREATE_XHR_INSTRUMENTATION: &str = "create_xhr_instrumentation_module";
const INIT_XHR_INSTRUMENTATION: &str = "init_xhr_instrumentation";
const CAPTURE_XHR_OPEN_METADATA: &str = "capture_xhr_open_metadata";
const INJECT_REQUEST_HEADERS_BEFORE_SEND: &str = "inject_request_headers_before_send";
const TEARDOWN_XHR_INSTRUMENTATION: &str = "teardown_xhr_instrumentation";
const NOOP_XHR_INIT: &str = "noop_xhr_init";
const NOOP_XHR_TEARDOWN: &str = "noop_xhr_teardown";
const NOOP_XHR_PROTOTYPE_INIT: &str = "noop_xhr_prototype_init";
const NOOP_XHR_PROTOTYPE_TEARDOWN: &str = "noop_xhr_prototype_teardown";
const NOOP_XHR_INSTRUMENTED_INIT: &str = "noop_xhr_instrumented_init";
const NOOP_XHR_INSTRUMENTED_TEARDOWN: &str = "noop_xhr_instrumented_teardown";

const REQUEST_MATCHER_ALIAS: &str = "request_matcher_alias";
const ENCODING_OPTIONS_FACTORY_ALIAS: &str = "encoding_options_factory_alias";
const PREFIXED_KEY_LENGTH_FORMATTER_ALIAS: &str = "prefixed_key_length_formatter_alias";
const SUBMIT_LISTENER: &str = "submit_listener";
const SUBMIT_INTERCEPTORS_INSTALLED: &str = "submit_interceptors_installed";
const ERROR_CALLBACKS: &str = "error_callbacks";
const SUBMIT_EVENT_CALLBACKS: &str = "submit_event_callbacks";
const SUBMIT_TARGET_CALLBACKS: &str = "submit_target_callbacks";
const HEADER_CHUNKER_ALIAS: &str = "header_chunker_alias";
const INSTRUMENTATION_MARKER_PROPERTY_NAMES: &str = "instrumentation_marker_property_names";
const SIGNAL_EVENT_NAMES: &str = "signal_event_names";
const INTERNAL_STATE_PROPERTY_NAMES: &str = "internal_state_property_names";

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Function }
}

const fn binding(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::MainBinding }
}

/// Every role this package can claim, with the one name it receives.
///
/// The interception records, the document reference and the instrumentation
/// window are named by the built-in roles, and the chunker, matcher, envelope
/// builder and length formatter functions are p07 targets. This package only
/// claims the carriers themselves and the four MAIN aliases p07 leaves to it.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    function(INSTALL_FORM_SUBMIT_INTERCEPTION, "installFormSubmitInterception"),
    function(INSTALL_FORM_SUBMIT_OBSERVATION, "installFormSubmitObservation"),
    function(HANDLE_OBSERVED_FORM_SUBMIT, "handleObservedFormSubmit"),
    function(FORWARD_STOP_IMMEDIATE_SUBMIT, "forwardSubmitOnStopImmediatePropagation"),
    function(HANDLE_STOP_PROPAGATION_SUBMIT, "handleSubmitViaStopPropagation"),
    function(HANDLE_PREVENT_DEFAULT_SUBMIT, "handleSubmitViaPreventDefault"),
    function(HANDLE_DOCUMENT_SUBMIT_CAPTURE, "handleDocumentSubmitCapture"),
    function(FORM_SUBMIT_BEFORE_INVOKE_CALLBACK, "forwardFormSubmitInvocation"),
    function(STOP_IMMEDIATE_BEFORE_INVOKE_CALLBACK, "forwardStopImmediatePropagationInvocation"),
    function(STOP_PROPAGATION_BEFORE_INVOKE_CALLBACK, "forwardStopPropagationInvocation"),
    function(PREVENT_DEFAULT_BEFORE_INVOKE_CALLBACK, "forwardPreventDefaultInvocation"),
    function(CANCEL_BUBBLE_BEFORE_SET_CALLBACK, "forwardCancelBubbleToStopPropagation"),
    function(SEND_TELEMETRY_REQUEST, "sendTelemetryRequest"),
    function(MARK_TELEMETRY_NETWORK_ERROR, "markTelemetryRequestNetworkError"),
    function(ENCODE_TELEMETRY_RESPONSE, "encodeTelemetryResponse"),
    function(CAPTURE_TELEMETRY_RESPONSE_TEXT, "captureTelemetryResponseText"),
    function(CREATE_CUSTOM_EVENT_SIGNAL_HOOKS, "createCustomEventSignalHooks"),
    function(DISPATCH_SIGNAL_CONFIG_EVENTS, "dispatchSignalConfigEvents"),
    function(CREATE_FALLBACK_CUSTOM_EVENT, "createFallbackCustomEvent"),
    function(DISPATCH_SIGNAL_TEARDOWN_EVENTS, "dispatchSignalTeardownEvents"),
    function(CREATE_FALLBACK_CUSTOM_EVENT_FOR_TEARDOWN, "createFallbackCustomEventForTeardown"),
    function(NOOP_CUSTOM_EVENT_HOOK_INIT, "noopCustomEventHookInit"),
    function(NOOP_CUSTOM_EVENT_HOOK_TEARDOWN, "noopCustomEventHookTeardown"),
    function(CREATE_PLATFORM_FORM_SUBMIT_HOOK, "createPlatformFormSubmitHook"),
    function(RECORD_CLICKED_SUBMITTER_CANDIDATE, "recordClickedSubmitterCandidate"),
    function(CLEAR_SUBMITTER_CANDIDATE, "clearSubmitterCandidate"),
    function(REMOVE_INJECTED_TOKEN_INPUTS, "removeInjectedTokenInputs"),
    function(REMOVE_ONE_INJECTED_TOKEN_INPUT, "removeInjectedTokenInput"),
    function(REPORT_FORM_SUBMIT_ERROR, "reportFormSubmitError"),
    function(INIT_SUBMIT_SIGNAL_HOOKS, "initSubmitSignalHooks"),
    function(INJECT_SUBMIT_OPTION_INPUTS, "injectSubmitOptionInputs"),
    function(CLEAR_SUBMIT_OPTION_CACHE, "clearSubmitOptionCache"),
    function(TEARDOWN_SUBMIT_INSTRUMENTATION, "teardownSubmitInstrumentation"),
    function(CREATE_FETCH_INSTRUMENTATION, "createFetchInstrumentation"),
    function(INIT_FETCH_INSTRUMENTATION, "initFetchInstrumentation"),
    function(INJECT_FETCH_REQUEST_HEADERS, "injectFetchRequestHeaders"),
    function(TEARDOWN_FETCH_INSTRUMENTATION, "teardownFetchInstrumentation"),
    function(NOOP_FETCH_HOOK_INIT, "noopFetchHookInit"),
    function(NOOP_FETCH_HOOK_TEARDOWN, "noopFetchHookTeardown"),
    function(CREATE_XHR_INSTRUMENTATION, "createXhrInstrumentationModule"),
    function(INIT_XHR_INSTRUMENTATION, "initXhrInstrumentation"),
    function(CAPTURE_XHR_OPEN_METADATA, "captureXhrOpenMetadata"),
    function(INJECT_REQUEST_HEADERS_BEFORE_SEND, "injectRequestHeadersBeforeSend"),
    function(TEARDOWN_XHR_INSTRUMENTATION, "teardownXhrInstrumentation"),
    function(NOOP_XHR_INIT, "noopXhrInit"),
    function(NOOP_XHR_TEARDOWN, "noopXhrTeardown"),
    function(NOOP_XHR_PROTOTYPE_INIT, "noopXhrPrototypeInit"),
    function(NOOP_XHR_PROTOTYPE_TEARDOWN, "noopXhrPrototypeTeardown"),
    function(NOOP_XHR_INSTRUMENTED_INIT, "noopXhrInstrumentedInit"),
    function(NOOP_XHR_INSTRUMENTED_TEARDOWN, "noopXhrInstrumentedTeardown"),
    binding(REQUEST_MATCHER_ALIAS, "requestMatcher"),
    binding(ENCODING_OPTIONS_FACTORY_ALIAS, "encodingOptionsFactory"),
    binding(PREFIXED_KEY_LENGTH_FORMATTER_ALIAS, "prefixedKeyLengthFormatter"),
    binding(SUBMIT_LISTENER, "submitListener"),
    binding(SUBMIT_INTERCEPTORS_INSTALLED, "submitInterceptorsInstalled"),
    binding(ERROR_CALLBACKS, "errorCallbacks"),
    binding(SUBMIT_EVENT_CALLBACKS, "submitEventCallbacks"),
    binding(SUBMIT_TARGET_CALLBACKS, "submitTargetCallbacks"),
    binding(HEADER_CHUNKER_ALIAS, "headerChunker"),
    binding(INSTRUMENTATION_MARKER_PROPERTY_NAMES, "instrumentationMarkerPropertyNames"),
    binding(SIGNAL_EVENT_NAMES, "signalEventNames"),
    binding(INTERNAL_STATE_PROPERTY_NAMES, "internalStatePropertyNames"),
];

// ---------------------------------------------------------------------------
// Structural index
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum FnNode<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
}

struct FnEntry<'a> {
    node: NodeId,
    span: Span,
    func: FnNode<'a>,
    parent: Option<usize>,
}

/// What one function owns directly, excluding nested functions.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    members: Vec<(&'a Expression<'a>, &'a str)>,
    written_keys: BTreeSet<String>,
    objects: Vec<&'a ObjectExpression<'a>>,
    nested: Vec<usize>,
}

struct Ix<'c, 'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    slots: &'c ParameterSlots<'a, 'a>,
    fns: Vec<FnEntry<'a>>,
    own: Vec<Own<'a>>,
    owner: Vec<Option<usize>>,
    fn_of_node: BTreeMap<NodeId, usize>,
    decl_fn: BTreeMap<SymbolId, usize>,
    values: BTreeMap<SymbolId, Vec<&'a Expression<'a>>>,
    assigned: BTreeMap<SymbolId, usize>,
    assigns: BTreeMap<SymbolId, Vec<&'a AssignmentExpression<'a>>>,
    /// Function or arrow whose body directly reads each symbol.
    read_owners: BTreeMap<SymbolId, BTreeSet<usize>>,
    xhr_native_methods: BTreeSet<usize>,
    xhr_envelope_builders: BTreeSet<usize>,
}

fn strip<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    expression.without_parentheses()
}

/// `(object, name)` of a static or string-keyed computed member.
fn member<'e, 'a>(expression: &'e Expression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
    let name = static_member_name(strip(expression))?;
    match strip(expression) {
        Expression::StaticMemberExpression(found) => Some((&found.object, name)),
        Expression::ComputedMemberExpression(found) => Some((&found.object, name)),
        _ => None,
    }
}

fn target_member_name<'e>(target: &'e AssignmentTarget<'_>) -> Option<&'e str> {
    match target {
        AssignmentTarget::StaticMemberExpression(found) => Some(found.property.name.as_str()),
        AssignmentTarget::ComputedMemberExpression(found) => match &found.expression {
            Expression::StringLiteral(literal) => Some(literal.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn target_member_object<'e, 'a>(target: &'e AssignmentTarget<'a>) -> Option<&'e Expression<'a>> {
    match target {
        AssignmentTarget::StaticMemberExpression(found) => Some(&found.object),
        AssignmentTarget::ComputedMemberExpression(found) => Some(&found.object),
        _ => None,
    }
}

fn args<'e, 'a>(call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
    call.arguments.iter().filter_map(Argument::as_expression).collect()
}

fn new_args<'e, 'a>(new: &'e NewExpression<'a>) -> Vec<&'e Expression<'a>> {
    new.arguments.iter().filter_map(Argument::as_expression).collect()
}

fn string_value<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    match strip(expression) {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn is_boolean(expression: &Expression<'_>, value: bool) -> bool {
    matches!(strip(expression), Expression::BooleanLiteral(literal) if literal.value == value)
}

fn is_null(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::NullLiteral(_))
}

/// String literals of an expression tree: additions, conditionals, calls and
/// their arguments. Used for host selectors and API arguments only.
fn expression_strings<'e, 'a>(expression: &'e Expression<'a>, out: &mut Vec<&'e str>) {
    match strip(expression) {
        Expression::StringLiteral(literal) => out.push(literal.value.as_str()),
        Expression::BinaryExpression(binary) => {
            expression_strings(&binary.left, out);
            expression_strings(&binary.right, out);
        }
        Expression::LogicalExpression(logical) => {
            expression_strings(&logical.left, out);
            expression_strings(&logical.right, out);
        }
        Expression::ConditionalExpression(conditional) => {
            expression_strings(&conditional.consequent, out);
            expression_strings(&conditional.alternate, out);
        }
        Expression::CallExpression(call) => {
            for argument in args(call) {
                expression_strings(argument, out);
            }
        }
        Expression::NewExpression(new) => {
            for argument in new_args(new) {
                expression_strings(argument, out);
            }
        }
        _ => {}
    }
}

struct LifecyclePair {
    init: usize,
    teardown: usize,
    node: NodeId,
}

impl<'c, 'a> Ix<'c, 'a> {
    fn new(ctx: &FinderCtx<'_, 'a>, slots: &'c ParameterSlots<'a, 'a>) -> Self {
        let mut ix = Self {
            scoping: ctx.scoping,
            nodes: ctx.nodes,
            slots,
            fns: Vec::new(),
            own: Vec::new(),
            owner: vec![None; ctx.nodes.len()],
            fn_of_node: BTreeMap::new(),
            decl_fn: BTreeMap::new(),
            values: BTreeMap::new(),
            assigned: BTreeMap::new(),
            assigns: BTreeMap::new(),
            read_owners: BTreeMap::new(),
            xhr_native_methods: BTreeSet::new(),
            xhr_envelope_builders: BTreeSet::new(),
        };
        ix.index();
        ix
    }

    fn index(&mut self) {
        let nodes = self.nodes;
        for (id, node) in nodes.iter_enumerated() {
            let parent = nodes.parent_id(id);
            let owner = if parent == id {
                None
            } else {
                self.fn_of_node.get(&parent).copied().or(self.owner[parent.index()])
            };
            self.owner[id.index()] = owner;
            let unit = match node.kind() {
                AstKind::Function(found) => {
                    Some((found.span, FnNode::Function(found), found.id.as_ref().and_then(|id| id.symbol_id.get())))
                }
                AstKind::ArrowFunctionExpression(found) => {
                    Some((found.span, FnNode::Arrow(found), None))
                }
                _ => None,
            };
            if let Some((span, func, symbol)) = unit {
                let index = self.fns.len();
                self.fns.push(FnEntry { node: id, span, func, parent: owner });
                self.own.push(Own::default());
                self.fn_of_node.insert(id, index);
                if let Some(owner) = owner {
                    self.own[owner].nested.push(index);
                }
                if let Some(symbol) = symbol {
                    self.decl_fn.insert(symbol, index);
                }
                if let FnNode::Arrow(found) = func {
                    if found.expression {
                        if let Some(Statement::ExpressionStatement(statement)) =
                            found.body.statements.first()
                        {
                            self.own[index].returns.push(&statement.expression);
                        }
                    }
                }
                continue;
            }
            self.index_node(node.kind(), owner);
        }
    }

    fn index_node(&mut self, kind: AstKind<'a>, owner: Option<usize>) {
        match kind {
            AstKind::VariableDeclarator(declarator) => {
                if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                {
                    if let Some(symbol) = binding.symbol_id.get() {
                        self.values.entry(symbol).or_default().push(init);
                    }
                }
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign =>
            {
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                        self.values.entry(symbol).or_default().push(&assignment.right);
                        *self.assigned.entry(symbol).or_default() += 1;
                        self.assigns.entry(symbol).or_default().push(assignment);
                    }
                }
                if let Some(owner) = owner {
                    if let Some(name) = target_member_name(&assignment.left) {
                        self.own[owner].written_keys.insert(name.to_string());
                    }
                }
            }
            _ => {}
        }
        let Some(owner) = owner else {
            return;
        };
        let scoping = self.scoping;
        match kind {
            AstKind::CallExpression(call) => self.own[owner].calls.push(call),
            AstKind::NewExpression(new) => self.own[owner].news.push(new),
            AstKind::AssignmentExpression(assignment) => self.own[owner].assignments.push(assignment),
            AstKind::ReturnStatement(statement) => {
                if let Some(argument) = statement.argument.as_ref() {
                    self.own[owner].returns.push(argument);
                }
            }
            AstKind::ObjectExpression(object) => self.own[owner].objects.push(object),
            AstKind::IdentifierReference(identifier) => {
                if let (Some(symbol), Some(reference)) = (
                    analysis::identifier_symbol(identifier, scoping),
                    identifier.reference_id.get(),
                ) {
                    if scoping.get_reference(reference).is_read() {
                        self.read_owners.entry(symbol).or_default().insert(owner);
                    }
                }
            }
            AstKind::StaticMemberExpression(found) => {
                self.own[owner].members.push((&found.object, found.property.name.as_str()));
            }
            AstKind::ComputedMemberExpression(found) => {
                if let Expression::StringLiteral(literal) = &found.expression {
                    self.own[owner].members.push((&found.object, literal.value.as_str()));
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Resolution
    // -----------------------------------------------------------------------

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.scoping)
    }

    /// `Object.prototype.hasOwnProperty`-style path rooted at an unbound global.
    fn path(&self, expression: &Expression<'a>) -> Option<String> {
        match strip(expression) {
            Expression::Identifier(identifier) if self.symbol(expression).is_none() => {
                Some(identifier.name.to_string())
            }
            _ => {
                let (object, name) = member(expression)?;
                Some(format!("{}.{name}", self.path(object)?))
            }
        }
    }

    /// The one value of a variable with exactly one declaration and one plain
    /// write (initializer or assignment).
    fn single_value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        if self.scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        if !matches!(
            self.nodes.kind(self.scoping.symbol_declaration(symbol)),
            AstKind::VariableDeclarator(_)
        ) {
            return None;
        }
        let writes =
            self.scoping.get_resolved_references(symbol).filter(|r| r.is_write()).count();
        if writes != self.assigned.get(&symbol).copied().unwrap_or(0) {
            return None;
        }
        match self.values.get(&symbol)?.as_slice() {
            [value] => Some(*value),
            _ => None,
        }
    }

    fn resolve_fn(&self, expression: &Expression<'a>) -> Option<usize> {
        self.resolve_fn_depth(expression, 8)
    }

    fn resolve_fn_depth(&self, expression: &Expression<'a>, depth: u8) -> Option<usize> {
        if depth == 0 {
            return None;
        }
        match strip(expression) {
            Expression::FunctionExpression(found) => {
                return self.fn_of_node.get(&found.node_id.get()).copied();
            }
            Expression::ArrowFunctionExpression(found) => {
                return self.fn_of_node.get(&found.node_id.get()).copied();
            }
            _ => {}
        }
        let symbol = self.symbol(expression)?;
        if let Some(index) = self.decl_fn.get(&symbol).copied() {
            let immutable = self.scoping.symbol_declarations(symbol).count() == 1
                && !self.scoping.get_resolved_references(symbol).any(|r| r.is_write());
            return immutable.then_some(index);
        }
        let mut found = Vec::new();
        for value in self.values.get(&symbol)? {
            if let Some(index) = self.resolve_fn_depth(value, depth - 1) {
                found.push(index);
            }
        }
        found.sort_unstable();
        found.dedup();
        match found.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn slot_index(&self, expression: &Expression<'a>, function: usize) -> Option<u32> {
        let mut current = strip(expression);
        for _ in 0..8 {
            if let Some(slot) = self.slots.incoming_slot(current) {
                return (slot.owner == self.fns[function].node).then_some(slot.index);
            }
            let symbol = self.symbol(current)?;
            if self.decl_fn.contains_key(&symbol) {
                return None;
            }
            current = strip(self.single_value(symbol)?);
        }
        None
    }

    fn slot_member(&self, function: usize, name: &str) -> bool {
        self.own[function]
            .members
            .iter()
            .any(|(object, member)| *member == name && self.slot_index(object, function) == Some(0))
    }

    /// `(receiver, method, arguments)` of `receiver.method(...)` and of the
    /// traced `receiver["method"].call(receiver, ...)`.
    fn invocation<'e>(
        &self,
        call: &'e CallExpression<'a>,
    ) -> (&'e Expression<'a>, Vec<&'e Expression<'a>>) {
        let arguments = args(call);
        if let Some((inner, "call")) = member(&call.callee) {
            if !arguments.is_empty() && (member(inner).is_some() || self.symbol(inner).is_some()) {
                return (inner, arguments[1..].to_vec());
            }
        }
        (&call.callee, arguments)
    }

    fn method<'e>(&self, call: &'e CallExpression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
        member(self.invocation(call).0)
    }

    fn method_name(&self, call: &CallExpression<'_>) -> Option<String> {
        self.method(call).map(|(_, name)| name.to_string())
    }

    fn call_args<'e>(&self, call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
        self.invocation(call).1
    }

    /// The binding and literal index of `holder[index]`. The platform form
    /// tracer lowers a native call to `[document, document.createElement][1]
    /// .call([document, document.createElement][0], "style")`; resolving the
    /// holder through `single_value` below keeps the two tuple positions tied
    /// to the same reaching definition.
    fn array_slot(&self, expression: &Expression<'a>) -> Option<(SymbolId, u32)> {
        let Expression::ComputedMemberExpression(found) = strip(expression) else {
            return None;
        };
        Some((
            self.symbol(&found.object)?,
            numeric_index(strip(&found.expression))?,
        ))
    }

    fn same_binding(&self, left: &Expression<'a>, right: &Expression<'a>) -> bool {
        match (self.symbol(left), self.symbol(right)) {
            (Some(left), Some(right)) => left == right,
            (None, None) => self.path(left) == self.path(right),
            _ => false,
        }
    }

    /// True when `symbol` is `target` itself or reaches it through a chain of
    /// single-value aliases (`let alias = target`). The depth bound refuses a
    /// mutually recursive alias cycle instead of recursing forever.
    fn symbol_aliases_to(&self, symbol: SymbolId, target: SymbolId, depth: u8) -> bool {
        if symbol == target {
            return true;
        }
        if depth == 0 {
            return false;
        }
        self.single_value(symbol)
            .and_then(|value| self.symbol(value))
            .is_some_and(|next| self.symbol_aliases_to(next, target, depth - 1))
    }

    /// The nearest enclosing statement of an expression or declaration node.
    fn enclosing_statement(&self, node: NodeId) -> Option<NodeId> {
        let mut current = node;
        for _ in 0..512 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            if self.nodes.kind(current).is_statement() {
                return Some(current);
            }
        }
        None
    }

    /// A statement whose parent is the candidate function's own body block.
    /// That is what makes it a straight-line statement of this body: a write
    /// nested in an `if`/loop block has a `BlockStatement` parent instead.
    fn statement_is_direct_body_child(&self, statement: NodeId, body: Span) -> bool {
        matches!(
            self.nodes.kind(self.nodes.parent_id(statement)),
            AstKind::FunctionBody(found) if found.span == body
        )
    }

    fn statement_index_in_body(&self, function: usize, statement: NodeId) -> Option<usize> {
        let span = self.nodes.kind(statement).span();
        self.body_statements(function)
            .iter()
            .position(|candidate| candidate.span() == span)
    }

    /// The node id of the expression with this exact source span. Node ids are
    /// only used to walk the parent chain; the ordering proof below reads the
    /// enclosing body's statement list instead of comparing spans.
    fn expression_node(&self, expression: &Expression<'a>) -> Option<NodeId> {
        let span = expression.span();
        self.nodes.iter_enumerated().find_map(|(id, node)| {
            let kind = node.kind();
            (!kind.is_statement() && !kind.is_declaration() && kind.span() == span).then_some(id)
        })
    }

    fn call_node(&self, call: &CallExpression<'a>) -> Option<NodeId> {
        let span = call.span();
        self.nodes.iter_enumerated().find_map(|(id, node)| {
            matches!(node.kind(), AstKind::CallExpression(found) if found.span() == span)
                .then_some(id)
        })
    }

    fn function_body_span(&self, function: usize) -> Option<Span> {
        match self.fns[function].func {
            FnNode::Function(found) => found.body.as_ref().map(|body| body.span),
            FnNode::Arrow(found) => Some(found.body.span),
        }
    }

    /// The tuple's one defining write must be its own statement on the
    /// candidate's straight-line body before the actual `.call`. A write
    /// nested in a conditional or loop block is not a direct body statement,
    /// and a write in a later statement is refused: neither dominates the
    /// call, so the tuple is not a reaching definition there.
    fn tuple_definition_dominates(
        &self,
        function: usize,
        definition: &Expression<'a>,
        call: &CallExpression<'a>,
    ) -> bool {
        let Some(body) = self.function_body_span(function) else {
            return false;
        };
        let Some(definition_node) = self.expression_node(definition) else {
            return false;
        };
        let Some(call_node) = self.call_node(call) else {
            return false;
        };
        let Some(definition_statement) = self.enclosing_statement(definition_node) else {
            return false;
        };
        let Some(call_statement) = self.enclosing_statement(call_node) else {
            return false;
        };
        if !self.statement_is_direct_body_child(definition_statement, body)
            || !self.statement_is_direct_body_child(call_statement, body)
        {
            return false;
        }
        let Some(definition_index) = self.statement_index_in_body(function, definition_statement)
        else {
            return false;
        };
        let Some(call_index) = self.statement_index_in_body(function, call_statement) else {
            return false;
        };
        definition_index < call_index
    }

    fn tuple_slot_is_stable(&self, function: usize, tuple: SymbolId, index: u32) -> bool {
        !self.subtree(function).iter().any(|unit| {
            self.own[*unit].assignments.iter().any(|assignment| {
                let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
                    return false;
                };
                let Some(object) = self.symbol(&member.object) else {
                    return false;
                };
                self.symbol_aliases_to(object, tuple, 8)
                    && numeric_index(strip(&member.expression)) == Some(index)
            })
        })
    }

    /// True when a member named `name` on the same binding as `object` is
    /// written anywhere in the candidate's subtree. A shadowed or overwritten
    /// native is not the captured platform method the tuple names. Both a
    /// symbol-bound receiver (through its single-value aliases) and an unbound
    /// global path receiver are compared.
    fn member_write_on(&self, function: usize, object: &Expression<'a>, name: &str) -> bool {
        let object_symbol = self.symbol(object);
        let object_path = self.path(object);
        self.subtree(function).iter().any(|unit| {
            self.own[*unit].assignments.iter().any(|assignment| {
                if target_member_name(&assignment.left) != Some(name) {
                    return false;
                }
                let Some(target) = target_member_object(&assignment.left) else {
                    return false;
                };
                match (object_symbol, self.symbol(target)) {
                    (Some(object_symbol), Some(target_symbol)) => {
                        self.symbol_aliases_to(target_symbol, object_symbol, 8)
                    }
                    (None, None) => object_path.is_some() && object_path == self.path(target),
                    _ => false,
                }
            })
        })
    }

    fn is_platform_document(&self, expression: &Expression<'a>, function: usize) -> bool {
        self.is_platform_document_depth(expression, function, 8)
    }

    fn is_platform_document_depth(
        &self,
        expression: &Expression<'a>,
        function: usize,
        depth: u8,
    ) -> bool {
        if depth == 0 {
            return false;
        }
        let expression = strip(expression);
        if self.path(expression).is_some_and(|path| {
            matches!(
                path.as_str(),
                "document"
                    | "window.document"
                    | "globalThis.document"
                    | "self.document"
                    | "top.document"
                    | "parent.document"
            )
        }) {
            return true;
        }
        if let Some((object, "document")) = member(expression) {
            if self.is_platform_global_object(object, function, depth - 1) {
                return true;
            }
        }
        self.symbol(expression)
            .and_then(|symbol| self.single_value(symbol))
            .is_some_and(|value| self.is_platform_document_depth(value, function, depth - 1))
    }

    fn is_platform_global_object(
        &self,
        expression: &Expression<'a>,
        function: usize,
        depth: u8,
    ) -> bool {
        if depth == 0 {
            return false;
        }
        if self.path(expression).is_some_and(|path| {
            matches!(path.as_str(), "window" | "globalThis" | "self" | "top" | "parent")
        }) {
            return true;
        }
        if let Some((object, "global")) = member(expression) {
            return self.slot_index(object, function) == Some(0);
        }
        self.symbol(expression)
            .and_then(|symbol| self.single_value(symbol))
            .is_some_and(|value| self.is_platform_global_object(value, function, depth - 1))
    }

    /// Prove the captured-native style creation at its actual call site. This
    /// deliberately does not treat a donor tuple, a foreign receiver, or a
    /// later-overwritten tuple slot as equivalent evidence.
    fn captured_style_creation(&self, call: &CallExpression<'a>, function: usize) -> bool {
        let Some((tuple_method, "call")) = member(&call.callee) else {
            return false;
        };
        let Some((tuple, 1)) = self.array_slot(tuple_method) else {
            return false;
        };
        let call_arguments = args(call);
        let [tuple_receiver, style] = call_arguments.as_slice() else {
            return false;
        };
        let Some((receiver_tuple, 0)) = self.array_slot(tuple_receiver) else {
            return false;
        };
        if tuple != receiver_tuple
            || string_value(style) != Some("style")
            || !self.tuple_slot_is_stable(function, tuple, 0)
            || !self.tuple_slot_is_stable(function, tuple, 1)
        {
            return false;
        }
        let Some(value) = self.single_value(tuple) else {
            return false;
        };
        if !self.tuple_definition_dominates(function, value, call) {
            return false;
        }
        let Some(elements) = array_elements(strip(value)) else {
            return false;
        };
        let [receiver, method] = elements.as_slice() else {
            return false;
        };
        let Some((method_receiver, "createElement")) = member(method) else {
            return false;
        };
        if self.member_write_on(function, receiver, "createElement") {
            return false;
        }
        self.same_binding(receiver, method_receiver)
            && self.is_platform_document(receiver, function)
    }

    /// Preserve the existing direct-style predicate, but require any new
    /// captured-tuple evidence to be owned by the candidate itself. An
    /// unreachable nested tuple call must not lend host evidence to the
    /// factory; lifecycle and form-option evidence remains independently
    /// checked by the existing gates below.
    fn has_platform_style_creation(&self, function: usize) -> bool {
        if self.subtree_has_call_string(function, "createElement", "style") {
            return true;
        }
        self.own[function].calls.iter().any(|call| {
            self.captured_style_creation(call, function)
        })
    }

    fn is_null_prototype_object(&self, expression: &Expression<'a>) -> bool {
        let Expression::CallExpression(call) = strip(expression) else {
            return false;
        };
        let arguments = args(call);
        self.path(&call.callee).as_deref() == Some("Object.create")
            && matches!(arguments.as_slice(), [only] if is_null(only))
    }

    fn single_return(&self, function: usize) -> Option<&'a Expression<'a>> {
        match self.own[function].returns.as_slice() {
            [value] => Some(*value),
            _ => None,
        }
    }

    fn body_statements(&self, function: usize) -> &'a [Statement<'a>] {
        match self.fns[function].func {
            FnNode::Function(found) => {
                found.body.as_ref().map_or(&[][..], |body| body.statements.as_slice())
            }
            FnNode::Arrow(found) => found.body.statements.as_slice(),
        }
    }

    fn is_void(expression: &Expression<'a>) -> bool {
        match strip(expression) {
            Expression::UnaryExpression(unary) => {
                unary.operator == oxc_syntax::operator::UnaryOperator::Void
            }
            Expression::Identifier(identifier) => identifier.name == "undefined",
            _ => false,
        }
    }

    /// A lifecycle step that does nothing: no statements, or only a bare or
    /// `void` return.
    fn body_is_noop(&self, function: usize) -> bool {
        self.body_statements(function).iter().all(|statement| {
            matches!(statement, Statement::ReturnStatement(found)
                if found.argument.as_ref().is_none_or(Self::is_void))
        })
    }

    fn subtree(&self, function: usize) -> Vec<usize> {
        (0..self.fns.len())
            .filter(|candidate| {
                let mut current = Some(*candidate);
                while let Some(index) = current {
                    if index == function {
                        return true;
                    }
                    current = self.fns[index].parent;
                }
                false
            })
            .collect()
    }

    fn subtree_has_call_string(&self, function: usize, name: &str, argument: &str) -> bool {
        self.subtree(function).iter().any(|unit| {
            self.own[*unit].calls.iter().any(|call| {
                self.method_name(call).as_deref() == Some(name)
                    && self.call_args(call).iter().any(|value| string_value(value) == Some(argument))
            })
        })
    }

    fn subtree_has_member(&self, function: usize, name: &str) -> bool {
        self.subtree(function)
            .iter()
            .any(|unit| self.own[*unit].members.iter().any(|(_, member)| *member == name))
    }

    /// A call that captures a host member: `installer(target, "name")`, also
    /// behind the traced `?? instrumented.x ?? capture(target, "name")` form.
    fn captured_host_member(&self, expression: &Expression<'a>) -> Option<String> {
        match strip(expression) {
            Expression::CallExpression(call) => {
                let arguments = args(call);
                let [_, name] = arguments.as_slice() else {
                    return None;
                };
                let name = string_value(name)?;
                self.resolve_fn(&call.callee)?;
                Some(name.to_string())
            }
            Expression::LogicalExpression(logical)
                if matches!(logical.operator, LogicalOperator::Coalesce | LogicalOperator::Or) =>
            {
                self.captured_host_member(&logical.left)
                    .or_else(|| self.captured_host_member(&logical.right))
            }
            Expression::ConditionalExpression(conditional) => self
                .captured_host_member(&conditional.consequent)
                .or_else(|| self.captured_host_member(&conditional.alternate)),
            _ => None,
        }
    }

    /// The registrations `bundle.onBeforeInvoke.register(callback)` of a
    /// function, with the host member the bundle captures.
    fn hook_registrations(&self, function: usize) -> Vec<(String, usize)> {
        let mut found = Vec::new();
        for call in &self.own[function].calls {
            let (callee, _) = self.invocation(call);
            let Some((hook, name)) = member(callee) else {
                continue;
            };
            if name != "register" {
                continue;
            }
            let Some((bundle, hook_name)) = member(hook) else {
                continue;
            };
            if hook_name != "onBeforeInvoke" && hook_name != "onBeforeSet" {
                continue;
            }
            let Some(symbol) = self.symbol(bundle) else {
                continue;
            };
            let Some(host) = self.single_value(symbol).and_then(|value| self.captured_host_member(value))
            else {
                continue;
            };
            let Some(callback) =
                self.call_args(call).first().and_then(|value| self.resolve_fn(value))
            else {
                continue;
            };
            found.push((host, callback));
        }
        found
    }

    fn object_holder_symbol(&self, object: &ObjectExpression<'a>) -> Option<SymbolId> {
        let span = object.span();
        for (_, node) in self.nodes.iter_enumerated() {
            let AstKind::VariableDeclarator(declarator) = node.kind() else {
                continue;
            };
            if declarator.init.as_ref().map(GetSpan::span) != Some(span) {
                continue;
            }
            if let BindingPattern::BindingIdentifier(binding) = &declarator.id {
                return binding.symbol_id.get();
            }
        }
        None
    }

    fn object_node_id(&self, object: &ObjectExpression<'a>) -> NodeId {
        let span = object.span();
        self.nodes
            .iter_enumerated()
            .find(|(_, node)| {
                matches!(node.kind(), AstKind::ObjectExpression(found) if found.span() == span)
            })
            .map(|(id, _)| id)
            .unwrap_or(oxc_semantic::NodeId::DUMMY)
    }

    fn lifecycle_pairs(&self, function: usize) -> Vec<LifecyclePair> {
        let mut pairs: Vec<LifecyclePair> = Vec::new();
        for object in &self.own[function].objects {
            let members = object_members_of(object);
            let Some(init) =
                members.iter().find(|entry| entry.name == "init").and_then(|entry| self.resolve_fn(entry.value))
            else {
                continue;
            };
            let mut teardown = members
                .iter()
                .find(|entry| entry.name == "teardown")
                .and_then(|entry| self.resolve_fn(entry.value));
            if teardown.is_none() {
                if let Some(symbol) = self.object_holder_symbol(object) {
                    for assignment in &self.own[function].assignments {
                        if target_member_name(&assignment.left) == Some("teardown")
                            && target_member_object(&assignment.left).and_then(|value| self.symbol(value))
                                == Some(symbol)
                        {
                            teardown = self.resolve_fn(&assignment.right);
                        }
                    }
                }
            }
            let Some(teardown) = teardown else {
                continue;
            };
            if pairs.iter().any(|pair| pair.init == init && pair.teardown == teardown) {
                continue;
            }
            pairs.push(LifecyclePair {
                init,
                teardown,
                node: self.object_node_id(object),
            });
        }
        pairs
    }

    fn enclosing_if_test(&self, node: NodeId) -> Option<&'a Expression<'a>> {
        let mut current = node;
        for _ in 0..512 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            if let AstKind::IfStatement(found) = self.nodes.kind(current) {
                return Some(&found.test);
            }
        }
        None
    }

    /// The subject of a `subject == null` / `null == subject` guard.
    fn null_guard_subject<'e>(test: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
        let Expression::BinaryExpression(binary) = strip(test) else {
            return None;
        };
        if !matches!(binary.operator, BinaryOperator::Equality | BinaryOperator::StrictEquality) {
            return None;
        }
        if is_null(&binary.right) {
            Some(&binary.left)
        } else if is_null(&binary.left) {
            Some(&binary.right)
        } else {
            None
        }
    }

    fn values_holder(&self, target: usize) -> Option<SymbolId> {
        let mut holders = Vec::new();
        for (symbol, values) in &self.values {
            if values.len() != 1 {
                continue;
            }
            if self.scoping.symbol_declarations(*symbol).count() != 1 {
                continue;
            }
            let writes = self
                .scoping
                .get_resolved_references(*symbol)
                .filter(|reference| reference.is_write())
                .count();
            if writes != self.assigned.get(symbol).copied().unwrap_or(0) {
                continue;
            }
            if !matches!(strip(values[0]), Expression::Identifier(_)) {
                continue;
            }
            if self.resolve_fn(values[0]) == Some(target) {
                holders.push(*symbol);
            }
        }
        match holders.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn array_of_strings(&self, symbol: SymbolId) -> bool {
        let Some(values) = self.values.get(&symbol) else {
            return false;
        };
        let [value] = values.as_slice() else {
            return false;
        };
        let Expression::ArrayExpression(array) = strip(value) else {
            return false;
        };
        !array.elements.is_empty()
            && array.elements.iter().all(|element| {
                element
                    .as_expression()
                    .is_some_and(|value| matches!(strip(value), Expression::StringLiteral(_)))
            })
    }

    fn reads_in(&self, symbol: SymbolId, function: Option<usize>) -> bool {
        function.is_some_and(|function| {
            self.read_owners
                .get(&symbol)
                .is_some_and(|owners| owners.contains(&function))
        })
    }
}

// ---------------------------------------------------------------------------
// Claims
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Out {
    claims: Vec<FinderClaim>,
    seen: BTreeSet<(&'static str, u32, u32)>,
    functions: BTreeMap<&'static str, usize>,
}

impl Out {
    /// Keep the claims of every role that matched exactly one span. Two
    /// candidates for one role cannot be told apart, so neither is named.
    fn into_unique(self) -> Vec<FinderClaim> {
        let mut spans: BTreeMap<&'static str, usize> = BTreeMap::new();
        for (role, _, _) in &self.seen {
            *spans.entry(*role).or_default() += 1;
        }
        let mut claims = self.claims;
        claims.retain(|claim| spans.get(claim.role()) == Some(&1));
        claims
    }

    fn function(&mut self, ix: &Ix<'_, '_>, role: &'static str, index: usize, evidence: &str) {
        let span = ix.fns[index].span;
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: span,
                alias_role: None,
                confidence: Confidence::Medium,
                evidence: vec![evidence.to_string()],
            });
            self.functions.entry(role).or_insert(index);
        }
    }

    fn binding(&mut self, ix: &Ix<'_, '_>, role: &'static str, symbol: SymbolId, evidence: &str) {
        let span = ix.scoping.symbol_span(symbol);
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::MainBinding {
                role,
                identifier_span: span,
                confidence: Confidence::Medium,
                evidence: vec![evidence.to_string()],
            });
        }
    }

    fn role_fn(&self, role: &'static str) -> Option<usize> {
        self.functions.get(role).copied()
    }
}

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let slots = ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes);
    let mut ix = Ix::new(ctx, &slots);
    ix.index_xhr_source_proofs(ctx);
    let mut out = Out::default();
    ix.claim_platform_family(&mut out);
    ix.claim_custom_event_family(&mut out);
    ix.claim_fetch_family(&mut out);
    ix.claim_xhr_family(&mut out);
    ix.claim_telemetry_family(&mut out);
    ix.claim_form_family(&mut out);
    ix.claim_carrier_aliases(&mut out);
    ix.claim_marker_arrays(&mut out);
    out.into_unique()
}

impl<'c, 'a> Ix<'c, 'a> {
    // -----------------------------------------------------------------------
    // Form-submit interception
    // -----------------------------------------------------------------------

    fn is_form_interceptor_installer(&self, function: usize) -> bool {
        let adds_capture = self.own[function].calls.iter().any(|call| {
            self.method_name(call).as_deref() == Some("addEventListener")
                && matches!(
                    self.call_args(call).as_slice(),
                    [event, _, capture] if string_value(event) == Some("submit") && is_boolean(capture, true)
                )
        });
        if !adds_capture {
            return false;
        }
        self.hook_registrations(function).len() >= 4
    }

    fn is_observation_installer(&self, function: usize) -> bool {
        let adds = self.own[function].calls.iter().any(|call| {
            self.method_name(call).as_deref() == Some("addEventListener")
                && matches!(self.call_args(call).as_slice(), [event, _] if string_value(event) == Some("submit"))
        });
        let removes = self.own[function].calls.iter().any(|call| {
            self.method_name(call).as_deref() == Some("removeEventListener")
                && matches!(self.call_args(call).as_slice(), [event, _] if string_value(event) == Some("submit"))
        });
        adds && removes && self.slot_member(function, "event") && self.slot_member(function, "target")
    }

    fn is_observed_form_handler(&self, function: usize) -> bool {
        let own = &self.own[function];
        let removes_itself = own.calls.iter().any(|call| {
            self.method_name(call).as_deref() == Some("removeEventListener")
                && matches!(
                    self.call_args(call).as_slice(),
                    [event, listener]
                        if string_value(event) == Some("submit")
                            && self.resolve_fn(listener) == Some(function)
                )
        });
        let has_form = own.objects.iter().any(|object| {
            object_members_of(object).iter().any(|entry| entry.name == "form")
        });
        let has_event = own.written_keys.contains("event")
            || own.objects.iter().any(|object| {
                object_members_of(object).iter().any(|entry| entry.name == "event")
            });
        let descriptor = has_form && has_event;
        let notifies = own
            .calls
            .iter()
            .any(|call| self.method_name(call).as_deref() == Some("notify"));
        removes_itself && descriptor && notifies
    }

    fn forwarded_callee(&self, callback: usize) -> Option<usize> {
        self.own[callback].calls.iter().find_map(|call| {
            if member(&call.callee).is_some() {
                return None;
            }
            self.resolve_fn(&call.callee)
        })
    }

    fn claim_form_family(&self, out: &mut Out) {
        let mut installers: Vec<usize> =
            (0..self.fns.len()).filter(|index| self.is_form_interceptor_installer(*index)).collect();
        if installers.len() != 1 {
            return;
        }
        let installer = installers.pop().expect("one installer");
        out.function(
            self,
            INSTALL_FORM_SUBMIT_INTERCEPTION,
            installer,
            "idempotent installer: adds a capture-phase document submit listener and registers the generic before-invoke/preventDefault/cancelBubble callbacks",
        );

        let capture_wrapper = self.own[installer].calls.iter().find_map(|call| {
            if self.method_name(call).as_deref() != Some("addEventListener") {
                return None;
            }
            match self.call_args(call).as_slice() {
                [event, wrapper, capture]
                    if string_value(event) == Some("submit") && is_boolean(capture, true) =>
                {
                    self.resolve_fn(wrapper)
                }
                _ => None,
            }
        });
        let Some(capture_wrapper) = capture_wrapper else {
            return;
        };
        out.function(
            self,
            HANDLE_DOCUMENT_SUBMIT_CAPTURE,
            capture_wrapper,
            "document submit capture listener; wraps {event, target} and hands it to the form observer",
        );

        if let Some(observer) =
            self.own[capture_wrapper].calls.iter().find_map(|call| self.resolve_fn(&call.callee))
        {
            if self.is_observation_installer(observer) {
                out.function(
                    self,
                    INSTALL_FORM_SUBMIT_OBSERVATION,
                    observer,
                    "per-form observer: adds a one-shot submit listener, or runs it immediately for a programmatic submit",
                );
                for call in &self.own[observer].calls {
                    if self.method_name(call).as_deref() != Some("addEventListener") {
                        continue;
                    }
                    let Some(handler) =
                        self.call_args(call).get(1).and_then(|value| self.resolve_fn(value))
                    else {
                        continue;
                    };
                    if self.is_observed_form_handler(handler) {
                        out.function(
                            self,
                            HANDLE_OBSERVED_FORM_SUBMIT,
                            handler,
                            "one-shot submit listener: removes itself and notifies the submit registry with {form, event}",
                        );
                    }
                }
            }
        }

        if let Some(listener) = self.values_holder(capture_wrapper) {
            out.binding(
                self,
                SUBMIT_LISTENER,
                listener,
                "single main-scope alias of the document submit capture wrapper; passed to addEventListener(\"submit\", …, true) and removed in teardown",
            );
        }

        for (host, callback) in self.hook_registrations(installer) {
            let role = match host.as_str() {
                "submit" => FORM_SUBMIT_BEFORE_INVOKE_CALLBACK,
                "stopImmediatePropagation" => STOP_IMMEDIATE_BEFORE_INVOKE_CALLBACK,
                "stopPropagation" => STOP_PROPAGATION_BEFORE_INVOKE_CALLBACK,
                "preventDefault" => PREVENT_DEFAULT_BEFORE_INVOKE_CALLBACK,
                "cancelBubble" => CANCEL_BUBBLE_BEFORE_SET_CALLBACK,
                _ => continue,
            };
            let evidence = match host.as_str() {
                "submit" => "generic before-invoke callback of the intercepted HTMLFormElement.submit bundle; preserves the bypass result and notifies the form observer",
                "stopImmediatePropagation" => "generic before-invoke callback of the intercepted Event.stopImmediatePropagation bundle; preserves the bypass result",
                "stopPropagation" => "generic before-invoke callback of the intercepted Event.stopPropagation bundle; preserves the bypass result",
                "preventDefault" => "generic before-invoke callback of the intercepted Event.preventDefault bundle; preserves the bypass result",
                _ => "generic before-set callback of the intercepted Event.cancelBubble accessor; forwards to the live stopPropagation",
            };
            out.function(self, role, callback, evidence);

            let forwarded = match host.as_str() {
                "stopImmediatePropagation" => {
                    Some((FORWARD_STOP_IMMEDIATE_SUBMIT, "submit forwarder invoked by the stopImmediatePropagation before-hook; clears itself after forwarding"))
                }
                "stopPropagation" => {
                    Some((HANDLE_STOP_PROPAGATION_SUBMIT, "submit forwarder invoked by the stopPropagation before-hook; clears itself after forwarding"))
                }
                "preventDefault" => {
                    Some((HANDLE_PREVENT_DEFAULT_SUBMIT, "preventDefault before-hook: removes the one-shot listener and notifies the submit-target registry with the form"))
                }
                _ => None,
            };
            if let Some((forward_role, forwarded_evidence)) = forwarded {
                if let Some(callee) = self.forwarded_callee(callback) {
                    if !self.body_is_noop(callee) {
                        out.function(self, forward_role, callee, forwarded_evidence);
                    }
                }
            }
        }

        // The idempotence flag: a boolean that the installer sets true and the
        // teardown resets to false.
        if let Some(teardown) = out.role_fn(TEARDOWN_SUBMIT_INSTRUMENTATION) {
            let mut flags = Vec::new();
            for (symbol, values) in &self.values {
                if !values.iter().any(|value| is_boolean(value, false)) {
                    continue;
                }
                let written = self.assigns.get(symbol).map(Vec::as_slice).unwrap_or(&[]);
                let sets_true = written.iter().any(|assignment| {
                    is_boolean(&assignment.right, true)
                        && self.owner[assignment.node_id.get().index()] == Some(installer)
                });
                let resets = written.iter().any(|assignment| {
                    is_boolean(&assignment.right, false)
                        && self.owner[assignment.node_id.get().index()] == Some(teardown)
                });
                if sets_true && resets {
                    flags.push(*symbol);
                }
            }
            if let [flag] = flags.as_slice() {
                out.binding(
                    self,
                    SUBMIT_INTERCEPTORS_INSTALLED,
                    *flag,
                    "boolean install flag: initialized false, set true by the submit interceptor installer and reset false by its teardown",
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Platform form-submit hook
    // -----------------------------------------------------------------------

    fn platform_form_submit_factory(&self) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.fns.len())
            .filter(|index| {
                self.has_platform_style_creation(*index)
                    && {
                        let mut strings = Vec::new();
                        for unit in self.subtree(*index) {
                            for call in &self.own[unit].calls {
                                for argument in self.call_args(call) {
                                    expression_strings(argument, &mut strings);
                                }
                            }
                        }
                        ["formaction", "formenctype", "formmethod"]
                            .iter()
                            .all(|name| strings.contains(name))
                    }
                    && self
                        .lifecycle_pairs(*index)
                        .iter()
                        .any(|pair| !self.body_is_noop(pair.init) && !self.body_is_noop(pair.teardown))
            })
            .collect();
        match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn claim_platform_family(&self, out: &mut Out) {
        let Some(factory) = self.platform_form_submit_factory() else {
            return;
        };
        out.function(
            self,
            CREATE_PLATFORM_FORM_SUBMIT_HOOK,
            factory,
            "form-submit factory: creates a detached style node, records the clicked submitter and returns a real {init, teardown} lifecycle",
        );
        let captured_style_creation = self.own[factory]
            .calls
            .iter()
            .any(|call| self.captured_style_creation(call, factory));
        let pairs = self.lifecycle_pairs(factory);
        let Some(pair) = pairs
            .iter()
            .find(|pair| !self.body_is_noop(pair.init) && !self.body_is_noop(pair.teardown))
        else {
            return;
        };
        out.function(
            self,
            INIT_SUBMIT_SIGNAL_HOOKS,
            pair.init,
            "form lifecycle init: registers the submit, token-input and error callbacks and conditionally adds the click listener",
        );
        out.function(
            self,
            TEARDOWN_SUBMIT_INSTRUMENTATION,
            pair.teardown,
            "form lifecycle teardown: removes the click and submit listeners, clears the three registries and removes the style node",
        );

        // Clicked-submitter recorder, registered as the click listener.
        for call in &self.own[pair.init].calls {
            if self.method_name(call).as_deref() != Some("addEventListener") {
                continue;
            }
            let Some(listener) = self.call_args(call).get(1).and_then(|value| self.resolve_fn(value))
            else {
                continue;
            };
            if !self.is_click_recorder(listener) {
                continue;
            }
            if !captured_style_creation {
                out.function(
                    self,
                    RECORD_CLICKED_SUBMITTER_CANDIDATE,
                    listener,
                    "click listener: stores the click target as the submitter candidate and schedules its clearing",
                );
            }
            if let Some(clear) = self.scheduled_callback(listener) {
                out.function(
                    self,
                    CLEAR_SUBMITTER_CANDIDATE,
                    clear,
                    "zero-delay timer callback that clears the recorded submitter candidate",
                );
            }
        }

        // The three callbacks registered by the init, classified by structure.
        for call in &self.own[pair.init].calls {
            let Some((receiver, "register")) = self.method(call) else {
                continue;
            };
            let Some(registry) = self.symbol(receiver) else {
                continue;
            };
            let Some(callback) =
                self.call_args(call).first().and_then(|value| self.resolve_fn(value))
            else {
                continue;
            };
            if self.is_error_reporter(callback) {
                if !captured_style_creation {
                    out.function(
                        self,
                        REPORT_FORM_SUBMIT_ERROR,
                        callback,
                        "error reporter: wraps its message in an object and hands it to the main error sink",
                    );
                }
                out.binding(
                    self,
                    ERROR_CALLBACKS,
                    registry,
                    "callback registry the form lifecycle registers its error reporter on; handlers notify it with failures",
                );
            } else if self.is_token_input_remover(callback) {
                out.function(
                    self,
                    REMOVE_INJECTED_TOKEN_INPUTS,
                    callback,
                    "submit-target cleanup: removes the hidden inputs of the cached options from the form or the document body",
                );
                out.binding(
                    self,
                    SUBMIT_TARGET_CALLBACKS,
                    registry,
                    "submit-target callback registry the form lifecycle registers its token-input cleanup on; the preventDefault hook notifies it with the form",
                );
                if let Some(callback_fn) = self.foreach_callback(callback) {
                    out.function(
                        self,
                        REMOVE_ONE_INJECTED_TOKEN_INPUT,
                        callback_fn,
                        "forEach callback of the token-input cleanup: one per key, it removes that one hidden input",
                    );
                }
            } else if self.is_submit_option_injector(callback) {
                out.function(
                    self,
                    INJECT_SUBMIT_OPTION_INPUTS,
                    callback,
                    "submit callback: caches the encoded options and injects one hidden input per option key into the form",
                );
                out.binding(
                    self,
                    SUBMIT_EVENT_CALLBACKS,
                    registry,
                    "submit-event callback registry the form lifecycle registers its option injector on; the one-shot submit handler notifies it with {form, event}",
                );
                if let Some(clear) = self.scheduled_callback(callback) {
                    out.function(
                        self,
                        CLEAR_SUBMIT_OPTION_CACHE,
                        clear,
                        "zero-delay timer callback that clears the cached encoded submit options",
                    );
                }
            }
        }
    }

    fn is_click_recorder(&self, function: usize) -> bool {
        let records_outer = self.own[function]
            .assignments
            .iter()
            .any(|assignment| matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(_)));
        records_outer && self.scheduled_callback(function).is_some()
    }

    /// The callback of the first `setTimeout(callback, …)` call.
    fn scheduled_callback(&self, function: usize) -> Option<usize> {
        self.own[function].calls.iter().find_map(|call| {
            if self.method_name(call).as_deref() == Some("setTimeout")
                || self.path(&call.callee).as_deref() == Some("setTimeout")
            {
                return self.call_args(call).first().and_then(|value| self.resolve_fn(value));
            }
            None
        })
    }

    fn is_error_reporter(&self, function: usize) -> bool {
        let message_object = self.own[function].objects.iter().any(|object| {
            object_members_of(object).iter().any(|entry| entry.name == "message")
        });
        let calls_sink = self.own[function].calls.iter().any(|call| {
            member(&call.callee).is_none() && self.resolve_fn(&call.callee).is_some()
        });
        message_object && calls_sink
    }

    fn is_token_input_remover(&self, function: usize) -> bool {
        let removes = self.subtree(function).iter().any(|unit| {
            self.own[*unit]
                .calls
                .iter()
                .any(|call| self.method_name(call).as_deref() == Some("removeChild"))
        });
        removes && self.has_selector_string(function, "input[type=hidden]")
    }

    fn is_submit_option_injector(&self, function: usize) -> bool {
        self.subtree_has_call_string(function, "createElement", "input")
            || self.subtree_has_call_string(function, "setAttribute", "hidden")
    }

    fn has_selector_string(&self, function: usize, needle: &str) -> bool {
        self.subtree(function).iter().any(|unit| {
            self.own[*unit].calls.iter().any(|call| {
                self.method_name(call).as_deref() == Some("querySelector")
                    && self.call_args(call).iter().any(|argument| {
                        let mut strings = Vec::new();
                        expression_strings(argument, &mut strings);
                        strings.iter().any(|value| value.contains(needle))
                    })
            })
        })
    }

    fn foreach_callback(&self, function: usize) -> Option<usize> {
        self.subtree(function).iter().find_map(|unit| {
            self.own[*unit].calls.iter().find_map(|call| {
                if self.method_name(call).as_deref() != Some("forEach") {
                    return None;
                }
                self.call_args(call).first().and_then(|value| self.resolve_fn(value))
            })
        })
    }

    // -----------------------------------------------------------------------
    // CustomEvent signal hooks
    // -----------------------------------------------------------------------

    fn custom_event_factory(&self) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.fns.len())
            .filter(|index| {
                self.slot_member(*index, "platformIntegratedHooksCalled")
                    && self.subtree_has_call_string(*index, "createEvent", "CustomEvent")
                    && self.subtree_has_member(*index, "initCustomEvent")
                    && self.subtree_has_member(*index, "dispatchEvent")
                    && !self.lifecycle_pairs(*index).is_empty()
            })
            .collect();
        match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    /// `Some(true)` for the init dispatcher (its event detail is an object),
    /// `Some(false)` for the teardown dispatcher (its detail is null).
    fn dispatcher_is_init(&self, function: usize) -> Option<bool> {
        for new in &self.own[function].news {
            let arguments = new_args(new);
            let Some(detail) = arguments.get(1) else {
                continue;
            };
            let Expression::ObjectExpression(object) = strip(detail) else {
                continue;
            };
            let Some(entry) = object_members_of(object).into_iter().find(|entry| entry.name == "detail")
            else {
                continue;
            };
            return match strip(entry.value) {
                Expression::ObjectExpression(_) => Some(true),
                other if is_null(other) => Some(false),
                _ => None,
            };
        }
        None
    }

    fn fallback_event_constructor(&self, dispatcher: usize) -> Option<usize> {
        let fallbacks: Vec<usize> = self.own[dispatcher]
            .nested
            .iter()
            .copied()
            .filter(|nested| {
                self.subtree_has_call_string(*nested, "createEvent", "CustomEvent")
                    && self.subtree_has_member(*nested, "initCustomEvent")
            })
            .collect();
        match fallbacks.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn claim_custom_event_family(&self, out: &mut Out) {
        let Some(factory) = self.custom_event_factory() else {
            return;
        };
        out.function(
            self,
            CREATE_CUSTOM_EVENT_SIGNAL_HOOKS,
            factory,
            "factory of the CustomEvent signal hooks: resolves the native constructor, returns a {init, teardown} dispatcher pair or a no-op pair when the platform hooks ran",
        );
        let pairs = self.lifecycle_pairs(factory);
        for pair in &pairs {
            let noop = self.body_is_noop(pair.init) && self.body_is_noop(pair.teardown);
            if noop {
                out.function(
                    self,
                    NOOP_CUSTOM_EVENT_HOOK_INIT,
                    pair.init,
                    "empty init of the no-op CustomEvent lifecycle pair returned when the platform hooks already ran",
                );
                out.function(
                    self,
                    NOOP_CUSTOM_EVENT_HOOK_TEARDOWN,
                    pair.teardown,
                    "empty teardown of the no-op CustomEvent lifecycle pair returned when the platform hooks already ran",
                );
                continue;
            }
            if self.body_is_noop(pair.init) || self.body_is_noop(pair.teardown) {
                continue;
            }
            let Some(init_kind) = self.dispatcher_is_init(pair.init) else {
                continue;
            };
            let Some(teardown_kind) = self.dispatcher_is_init(pair.teardown) else {
                continue;
            };
            if !init_kind || teardown_kind {
                continue;
            }
            out.function(
                self,
                DISPATCH_SIGNAL_CONFIG_EVENTS,
                pair.init,
                "CustomEvent init hook: dispatches one configured event per name with the helper API in its detail",
            );
            out.function(
                self,
                DISPATCH_SIGNAL_TEARDOWN_EVENTS,
                pair.teardown,
                "CustomEvent teardown hook: dispatches one configured event per name with a null detail",
            );
            if let Some(fallback) = self.fallback_event_constructor(pair.init) {
                out.function(
                    self,
                    CREATE_FALLBACK_CUSTOM_EVENT,
                    fallback,
                    "fallback CustomEvent constructor inside the signal init hook: createEvent(\"CustomEvent\") + initCustomEvent, prototype set to Event.prototype",
                );
            }
            if let Some(fallback) = self.fallback_event_constructor(pair.teardown) {
                out.function(
                    self,
                    CREATE_FALLBACK_CUSTOM_EVENT_FOR_TEARDOWN,
                    fallback,
                    "fallback CustomEvent constructor inside the signal teardown hook: createEvent(\"CustomEvent\") + initCustomEvent, prototype set to Event.prototype",
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Fetch instrumentation
    // -----------------------------------------------------------------------

    fn fetch_factory(&self) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.fns.len())
            .filter(|index| {
                self.subtree_has_member(*index, "fetch")
                    && self.subtree_has_member(*index, "onBeforeInvoke")
                    && self.subtree_has_member(*index, "defineProperty")
                    && !self.lifecycle_pairs(*index).is_empty()
            })
            .collect();
        match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn claim_fetch_family(&self, out: &mut Out) {
        let Some(factory) = self.fetch_factory() else {
            return;
        };
        out.function(
            self,
            CREATE_FETCH_INSTRUMENTATION,
            factory,
            "fetch instrumentation factory: captures the original fetch and Request, installs the generic wrapper and returns a lifecycle pair (or a no-op pair when a marker is already present)",
        );
        let pairs = self.lifecycle_pairs(factory);
        let real = pairs
            .iter()
            .find(|pair| !self.body_is_noop(pair.init) && !self.body_is_noop(pair.teardown));
        if let Some(pair) = real {
            out.function(
                self,
                INIT_FETCH_INSTRUMENTATION,
                pair.init,
                "fetch lifecycle init: registers the before-invoke callback and marks the original fetch as instrumented",
            );
            out.function(
                self,
                TEARDOWN_FETCH_INSTRUMENTATION,
                pair.teardown,
                "fetch lifecycle teardown: deletes the marker properties and unregisters the before-invoke callback",
            );
        }
        for pair in &pairs {
            if self.body_is_noop(pair.init) && self.body_is_noop(pair.teardown) {
                out.function(
                    self,
                    NOOP_FETCH_HOOK_INIT,
                    pair.init,
                    "empty init of the no-op fetch instrumentation lifecycle pair",
                );
                out.function(
                    self,
                    NOOP_FETCH_HOOK_TEARDOWN,
                    pair.teardown,
                    "empty teardown of the no-op fetch instrumentation lifecycle pair",
                );
            }
        }
        for (host, callback) in self
            .subtree(factory)
            .iter()
            .flat_map(|unit| self.hook_registrations(*unit))
        {
            if host != "fetch" {
                continue;
            }
            if self.is_fetch_header_injector(callback) {
                out.function(
                    self,
                    INJECT_FETCH_REQUEST_HEADERS,
                    callback,
                    "fetch before-invoke callback: rebuilds the Request, injects the encoded header chunks with headers.set and returns {args: [request]}",
                );
            }
        }
    }

    fn is_fetch_header_injector(&self, function: usize) -> bool {
        let returns_args = self.own[function].returns.iter().any(|value| {
            match strip(value) {
                Expression::ObjectExpression(object) => object_members_of(object)
                    .iter()
                    .any(|entry| entry.name == "args"),
                _ => false,
            }
        });
        let sets_headers = self.subtree(function).iter().any(|unit| {
            self.own[*unit].calls.iter().any(|call| {
                self.method(call).is_some_and(|(receiver, name)| {
                    name == "set"
                        && member(receiver).is_some_and(|(_, field)| field == "headers")
                })
            })
        });
        returns_args && sets_headers
    }

    // -----------------------------------------------------------------------
    // XHR instrumentation
    // -----------------------------------------------------------------------

    fn xhr_factory(&self) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.fns.len())
            .filter(|index| {
                self.subtree_has_member(*index, "XMLHttpRequest")
                    && self.subtree_has_member(*index, "onBeforeInvoke")
                    && self.subtree_has_member(*index, "setRequestHeader")
                    && self.lifecycle_pairs(*index).len() >= 2
            })
            .collect();
        match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn claim_xhr_family(&self, out: &mut Out) {
        let Some(factory) = self.xhr_factory() else {
            return;
        };
        out.function(
            self,
            CREATE_XHR_INSTRUMENTATION,
            factory,
            "XHR instrumentation factory: captures open/send/setRequestHeader, installs the generic wrappers and returns a lifecycle pair, or a no-op pair when the constructor, prototype or marker is missing",
        );
        let pairs = self.lifecycle_pairs(factory);
        let real = pairs
            .iter()
            .find(|pair| !self.body_is_noop(pair.init) && !self.body_is_noop(pair.teardown));
        if let Some(pair) = real {
            out.function(
                self,
                INIT_XHR_INSTRUMENTATION,
                pair.init,
                "XHR lifecycle init: registers the open and send before-invoke callbacks and marks the prototype as instrumented",
            );
            out.function(
                self,
                TEARDOWN_XHR_INSTRUMENTATION,
                pair.teardown,
                "XHR lifecycle teardown: deletes the marker properties and unregisters the open and send callbacks",
            );
        }
        for pair in &pairs {
            if !(self.body_is_noop(pair.init) && self.body_is_noop(pair.teardown)) {
                continue;
            }
            match self.noop_guard_kind(pair) {
                Some(NoopGuard::MissingConstructor) => {
                    out.function(self, NOOP_XHR_INIT, pair.init, "empty init of the no-op XHR lifecycle pair returned when XMLHttpRequest is absent");
                    out.function(self, NOOP_XHR_TEARDOWN, pair.teardown, "empty teardown of the no-op XHR lifecycle pair returned when XMLHttpRequest is absent");
                }
                Some(NoopGuard::MissingPrototype) => {
                    out.function(self, NOOP_XHR_PROTOTYPE_INIT, pair.init, "empty init of the no-op XHR lifecycle pair returned when XMLHttpRequest.prototype is absent");
                    out.function(self, NOOP_XHR_PROTOTYPE_TEARDOWN, pair.teardown, "empty teardown of the no-op XHR lifecycle pair returned when XMLHttpRequest.prototype is absent");
                }
                Some(NoopGuard::AlreadyInstrumented) => {
                    out.function(self, NOOP_XHR_INSTRUMENTED_INIT, pair.init, "empty init of the no-op XHR lifecycle pair returned when the instrumentation marker is already present");
                    out.function(self, NOOP_XHR_INSTRUMENTED_TEARDOWN, pair.teardown, "empty teardown of the no-op XHR lifecycle pair returned when the instrumentation marker is already present");
                }
                None => {}
            }
        }
        for (host, callback) in self
            .subtree(factory)
            .iter()
            .flat_map(|unit| self.hook_registrations(*unit))
        {
            match host.as_str() {
                "open" => out.function(
                    self,
                    CAPTURE_XHR_OPEN_METADATA,
                    callback,
                    "open before-invoke callback: records the lowercased method and url as non-enumerable per-request metadata on the receiver",
                ),
                "send" if self.stable_xhr_send_registration(factory, callback) => out.function(
                    self,
                    INJECT_REQUEST_HEADERS_BEFORE_SEND,
                    callback,
                    "send before-invoke callback: reads the request metadata, chunks the encoded options and calls the original setRequestHeader for each chunk",
                ),
                _ => {}
            }
        }
        // Keep the existing capture route and its evidence independent.
        if let Some(pair) = real {
            if let Some(callback) = self.split_xhr_send_registration(factory, pair.init, pair.teardown) {
                out.function(
                    self,
                    INJECT_REQUEST_HEADERS_BEFORE_SEND,
                    callback,
                    "send before-invoke callback: reads the request metadata, chunks the encoded options and calls the original setRequestHeader for each chunk",
                );
            }
        }
    }

    /// The independent single-value send route still needs a reaching
    /// definition in the factory body. A sole write stranded in one split
    /// arm is not a substitute for the guarded two-arm proof below.
    fn stable_xhr_send_registration(&self, factory: usize, callback: usize) -> bool {
        self.subtree(factory).iter().any(|unit| self.own[*unit].calls.iter().any(|call| {
            let (callee, _) = self.invocation(call);
            let Some((hook, "register")) = member(callee) else { return false };
            let Some((bundle, "onBeforeInvoke")) = member(hook) else { return false };
            if self.call_args(call).first().and_then(|arg| self.resolve_fn(arg)) != Some(callback) {
                return false;
            }
            let Some(value) = self.symbol(bundle).and_then(|symbol| self.single_value(symbol)) else { return false };
            let Some(statement) = self.expression_node(value).and_then(|node| self.enclosing_statement(node)) else { return false };
            self.function_body_span(factory).is_some_and(|body|
                self.statement_is_direct_body_child(statement, body))
        }))
    }

    /// Source-backed interceptor identities, never a semantic-name lookup.
    fn index_xhr_source_proofs(&mut self, ctx: &FinderCtx<'_, 'a>) {
        for claim in crate::signals::analysis::roles::request_envelope::find(ctx) {
            if let FinderClaim::Function { role: "request_envelope_builder", function_span, .. } = claim {
                if let Some(index) = self.fns.iter().position(|function| function.span == function_span) {
                    self.xhr_envelope_builders.insert(index);
                }
            }
        }
        let Ok(model) = crate::signals::extract::bindings::Model::new(
            ctx.source, ctx.program, ctx.scoping, ctx.nodes,
            crate::signals::schema::ProgramPropertiesLimits::default(),
        ) else { return };
        for site in model.all_sites() {
            let Some(index) = self.fn_of_node.get(&site.node_id).copied() else { continue };
            let FnNode::Function(function) = self.fns[index].func else { continue };
            let Some(body) = function.body.as_ref() else { continue };
            if super::function_interceptor::proves_method_interceptor(&model, *site, function, body) {
                self.xhr_native_methods.insert(index);
            }
        }
    }

    /// Finite immutable callable aliases. Every read must remain a direct
    /// callee (or another immutable alias); argument/member escapes refuse.
    fn xhr_stable_callable(&self, expression: &Expression<'a>, depth: u8) -> Option<usize> {
        if depth == 0 { return None; }
        let symbol = self.symbol(expression)?;
        for reference in self.scoping.get_resolved_references(symbol) {
            if reference.is_write() { continue; }
            let node = reference.node_id();
            match self.nodes.kind(self.nodes.parent_id(node)) {
                AstKind::CallExpression(call) if self.symbol(&call.callee) == Some(symbol) => {}
                AstKind::VariableDeclarator(alias) if alias.init.as_ref().is_some_and(|value| self.symbol(value) == Some(symbol)) => {
                    let BindingPattern::BindingIdentifier(alias) = &alias.id else { return None };
                    let alias_symbol = alias.symbol_id.get()?;
                    self.single_value(alias_symbol)?;
                    // The next alias must itself have no mutable/escaping uses.
                    for use_ in self.scoping.get_resolved_references(alias_symbol) {
                        if use_.is_write() { continue; }
                        if !matches!(self.nodes.kind(self.nodes.parent_id(use_.node_id())),
                            AstKind::CallExpression(call) if self.symbol(&call.callee) == Some(alias_symbol))
                        { return None; }
                    }
                }
                AstKind::AssignmentExpression(alias) if alias.operator == AssignmentOperator::Assign
                    && self.symbol(&alias.right) == Some(symbol) => {
                    let AssignmentTarget::AssignmentTargetIdentifier(alias) = &alias.left else { return None };
                    let alias_symbol = analysis::identifier_symbol(alias, self.scoping)?;
                    self.single_value(alias_symbol)?;
                    for use_ in self.scoping.get_resolved_references(alias_symbol) {
                        if use_.is_write() { continue; }
                        if !matches!(self.nodes.kind(self.nodes.parent_id(use_.node_id())),
                            AstKind::CallExpression(call) if self.symbol(&call.callee) == Some(alias_symbol))
                        { return None; }
                    }
                }
                AstKind::ObjectProperty(_) => return self.xhr_gate(None, "callable.read.object_property_publication"),
                _ => return self.xhr_gate(None, "callable.read.other_escape"),
            }
        }
        if let Some(function) = self.decl_fn.get(&symbol).copied() {
            return (!self.scoping.get_resolved_references(symbol).any(|r| r.is_write())).then_some(function);
        }
        self.xhr_stable_callable(self.single_value(symbol)?, depth - 1)
    }

    /// Native send capture only: recursively account for immutable local
    /// aliases of the source-proven installer. This does not allow value
    /// publication, argument escapes, property callees or mutable bindings.
    fn xhr_native_capture_callable(&self, expression: &Expression<'a>) -> Option<usize> {
        let helper = self.xhr_gate(self.resolve_fn(expression), "native.origin.resolve_actual_callee")?;
        self.xhr_gate(self.xhr_native_methods.contains(&helper).then_some(()),
            "native.origin.indexed_interceptor_protocol")?;
        let mut current = expression;
        let mut origins = BTreeSet::new();
        let root = loop {
            let symbol = self.xhr_gate(self.symbol(current), "native.origin.identifier")?;
            if !origins.insert(symbol) || origins.len() > 8 {
                return self.xhr_gate(None, "native.origin.alias_cycle_or_budget");
            }
            if self.decl_fn.get(&symbol).copied() == Some(helper) { break symbol; }
            current = self.xhr_gate(self.single_value(symbol), "native.origin.unique_value_and_writes")?;
        };
        let mut seen = BTreeSet::new();
        self.xhr_native_alias_reads(root, &mut seen)?;
        Some(helper)
    }

    fn xhr_native_alias_reads(&self, symbol: SymbolId, seen: &mut BTreeSet<SymbolId>) -> Option<()> {
        if seen.contains(&symbol) { return Some(()); }
        if seen.len() >= 16 { return self.xhr_gate(None, "native.alias.graph_budget"); }
        seen.insert(symbol);
        let declarations = self.scoping.symbol_declarations(symbol).count();
        let writes = self.scoping.get_resolved_references(symbol).filter(|r| r.is_write()).count();
        #[cfg(test)]
        eprintln!("[native header origin] alias facts declarations={declarations} writes={writes} declaration_function={}", self.decl_fn.contains_key(&symbol));
        if declarations != 1 { return self.xhr_gate(None, "native.alias.unique_declaration"); }
        if self.decl_fn.contains_key(&symbol) {
            if writes != 0 { return self.xhr_gate(None, "native.alias.function_binding_write"); }
        } else {
            self.xhr_gate(self.single_value(symbol), "native.alias.unique_value_and_writes")?;
        }
        for reference in self.scoping.get_resolved_references(symbol) {
            if reference.is_write() { continue; } // immutable origin already proved
            let node = reference.node_id();
            let parent = self.nodes.parent_id(node);
            let alias = match self.nodes.kind(parent) {
                AstKind::CallExpression(call) if matches!(strip(&call.callee), Expression::Identifier(id)
                    if id.reference_id.get().is_some_and(|reference|
                        self.scoping.get_reference(reference).node_id() == node))
                    && !call.optional && call.arguments.iter().all(|arg| arg.as_expression().is_some()) => None,
                AstKind::VariableDeclarator(declaration)
                    if declaration.init.as_ref().is_some_and(|value| self.symbol(value) == Some(symbol)) => {
                    let BindingPattern::BindingIdentifier(id) = &declaration.id else {
                        return self.xhr_gate(None, "native.alias.plain_binding_pattern");
                    };
                    Some(self.xhr_gate(id.symbol_id.get(), "native.alias.local_binding_symbol")?)
                }
                AstKind::ObjectProperty(_) => return self.xhr_gate(None, "native.alias.publication_refused"),
                _ => return self.xhr_gate(None, "native.alias.noncallee_or_mutating_use"),
            };
            if !self.decl_fn.contains_key(&symbol) {
                self.xhr_native_alias_precedes(symbol, node)?;
            }
            if let Some(alias) = alias { self.xhr_native_alias_reads(alias, seen)?; }
        }
        Some(())
    }

    fn xhr_native_alias_precedes(&self, symbol: SymbolId, use_node: NodeId) -> Option<()> {
        let value = self.xhr_gate(self.single_value(symbol), "native.alias.order.single_value")?;
        let definition = self.xhr_gate(self.expression_node(value).and_then(|node| self.enclosing_statement(node)),
            "native.alias.order.definition_statement")?;
        self.xhr_gate(self.owner[self.scoping.symbol_declaration(symbol).index()],
            "native.alias.order.declaration_owner")?;
        self.xhr_gate(self.owner[use_node.index()], "native.alias.order.use_owner")?;
        let container = self.nodes.parent_id(definition);
        if !matches!(self.nodes.kind(container), AstKind::FunctionBody(_) | AstKind::BlockStatement(_)) {
            return self.xhr_gate(None, "native.alias.order.definition_container");
        }
        self.xhr_native_use_after(definition, use_node, &mut BTreeSet::new(), &mut 128)
    }

    /// Prove the first possible activation/exposure after initialization.
    /// A hoisted declaration's position says nothing about when its body runs:
    /// recursively check every reference to its binding instead. For inline
    /// closures, creation after initialization is a conservative lower bound.
    /// Cycles, redeclarations, writes and unbounded graphs refuse.
    fn xhr_native_use_after(&self, definition: NodeId, use_node: NodeId,
        active: &mut BTreeSet<usize>, budget: &mut u16) -> Option<()> {
        if *budget == 0 { return self.xhr_gate(None, "native.alias.order.activation_budget"); }
        *budget -= 1;
        let container = self.nodes.parent_id(definition);
        let mut boundary = use_node;
        for _ in 0..512 {
            if let Some(function) = self.fn_of_node.get(&boundary).copied() {
                if let FnNode::Function(found) = self.fns[function].func {
                    if found.r#type == FunctionType::FunctionDeclaration {
                        let symbol = found.id.as_ref()?.symbol_id.get()?;
                        if self.scoping.symbol_declarations(symbol).count() != 1
                            || !active.insert(function)
                        { return self.xhr_gate(None, "native.alias.order.hoisted_cycle_or_redeclaration"); }
                        for reference in self.scoping.get_resolved_references(symbol) {
                            if reference.is_write() {
                                return self.xhr_gate(None, "native.alias.order.hoisted_binding_write");
                            }
                            self.xhr_native_use_after(definition, reference.node_id(), active, budget)?;
                        }
                        active.remove(&function);
                        return Some(());
                    }
                }
            }
            let parent = self.nodes.parent_id(boundary);
            if parent == container {
                let statement = self.nodes.kind(boundary);
                let before = self.nodes.kind(definition).span().end <= statement.span().start;
                return self.xhr_gate(
                    ((statement.is_statement() || statement.is_declaration()) && before).then_some(()),
                    "native.alias.order.reaching_definition",
                );
            }
            if parent == boundary { break; }
            boundary = parent;
        }
        self.xhr_gate(None, "native.alias.order.use_outside_definition_container")
    }

    /// The chunker is a closed function called through immutable identifiers.
    /// Publishing its value in the helper API copies that value; it does not
    /// expose the lexical binding used by the header writer. Capture/header
    /// callables continue to use the stricter nonescaping resolver.
    fn xhr_published_chunker(&self, expression: &Expression<'a>, callback: usize) -> Option<usize> {
        let mut stage = "xhr_published_chunker.entry";
        let result = (|| {
            self.xhr_probe_stage(&mut stage, "chunker.origin.resolve_actual_callee.006L2153");
            let helper = self.resolve_fn(expression)?;
            self.xhr_probe_stage(&mut stage, "chunker.origin.owned_protocol.006L2154");
            self.xhr_chunker_helper(helper)?;
            self.xhr_probe_stage(&mut stage, "chunker.origin.parent_scope.006L2155");
            let root = self.fns[helper].parent?;
            // The chunker grammar uses only its parameters/locals and unshadowed
            // Object.create. An exported closure over mutable data is not enough.
            self.xhr_probe_stage(&mut stage, "chunker.origin.closed_lexical_reads.006L2158");
            #[cfg(test)]
            for (symbol, owners) in &self.read_owners {
                if owners.contains(&helper) && self.owner[self.scoping.symbol_declaration(*symbol).index()] != Some(helper) {
                    eprintln!("[split header origin] closed_read foreign_declaration_kind={}",
                        match self.nodes.kind(self.scoping.symbol_declaration(*symbol)) {
                            AstKind::Function(_) => "Function",
                            AstKind::FormalParameter(_) => "FormalParameter",
                            AstKind::VariableDeclarator(_) => "VariableDeclarator",
                            _ => "Other",
                        });
                }
            }
            if self.read_owners.iter().any(|(symbol, owners)| owners.contains(&helper)
                && self.owner[self.scoping.symbol_declaration(*symbol).index()] != Some(helper))
            { return None; }
            let mut chain = BTreeSet::new();
            let mut value = expression;
            for _ in 0..8 {
                self.xhr_probe_stage(&mut stage, "chunker.alias.resolve_identifier.006L2164");
                let symbol = self.symbol(value)?;
                self.xhr_probe_stage(&mut stage, "chunker.alias.unique_declaration_and_cycle.006L2165");
                if !chain.insert(symbol) || self.scoping.symbol_declarations(symbol).count() != 1 { return None; }
                self.xhr_probe_stage(&mut stage, "chunker.alias.declaration_owner.006L2166");
                if self.owner[self.scoping.symbol_declaration(symbol).index()] != Some(root) { return None; }
                self.xhr_probe_stage(&mut stage, "chunker.alias.function_declaration.006L2167");
                if self.decl_fn.get(&symbol).copied() == Some(helper) {
                    self.xhr_probe_stage(&mut stage, "chunker.alias.function_binding_writes.006L2168");
                    if self.scoping.get_resolved_references(symbol).any(|reference| reference.is_write()) { return None; }
                    break;
                }
                self.xhr_probe_stage(&mut stage, "chunker.alias.single_value_and_all_writes.006L2171");
                value = self.single_value(symbol)?;
                // Alias initialization must precede the actual enclosing factory
                // in the same statement container. Conditional/late origins fail.
                self.xhr_probe_stage(&mut stage, "chunker.alias.enclosing_definition_statement.006L2174");
                let definition = self.enclosing_statement(self.expression_node(value)?)?;
                self.xhr_probe_stage(&mut stage, "chunker.alias.caller_factory_ancestry.006L2175");
                let mut scope = callback;
                while self.fns[scope].parent != Some(root) { scope = self.fns[scope].parent?; }
                self.xhr_probe_stage(&mut stage, "chunker.alias.same_container_and_definition_order.006L2177");
                #[cfg(test)]
                eprintln!("[split header origin] alias_definition kind={} same_container={} before_factory={}",
                    match self.nodes.kind(definition) {
                        AstKind::VariableDeclaration(_) => "VariableDeclaration",
                        AstKind::BlockStatement(_) => "BlockStatement",
                        AstKind::ExpressionStatement(_) => "ExpressionStatement",
                        _ => "Other",
                    },
                    self.nodes.parent_id(definition) == self.nodes.parent_id(self.fns[scope].node),
                    self.nodes.kind(definition).span().end <= self.fns[scope].span.start);
                if self.nodes.parent_id(definition) != self.nodes.parent_id(self.fns[scope].node)
                    || self.nodes.kind(definition).span().end > self.fns[scope].span.start
                { return None; }
            }
            self.xhr_probe_stage(&mut stage, "chunker.alias.terminal_exact_helper.006L2181");
            if !matches!(self.symbol(value).and_then(|symbol| self.decl_fn.get(&symbol)), Some(index) if *index == helper) { return None; }
            let mut publications = BTreeSet::new();
            for symbol in &chain {
                for reference in self.scoping.get_resolved_references(*symbol) {
                    if reference.is_write() { continue; } // single_value/decl checks accounted for all writes
                    let node = reference.node_id();
                    self.xhr_probe_stage(&mut stage, "chunker.alias.reference_use.006L2187");
                    match self.nodes.kind(self.nodes.parent_id(node)) {
                        AstKind::CallExpression(call) if self.symbol(&call.callee) == Some(*symbol)
                            && !call.optional && call.arguments.iter().all(|arg| arg.as_expression().is_some()) => {}
                        AstKind::VariableDeclarator(alias) if alias.init.as_ref().is_some_and(|value| self.symbol(value) == Some(*symbol)) => {
                            let BindingPattern::BindingIdentifier(alias) = &alias.id else { return None };
                            if !alias.symbol_id.get().is_some_and(|alias| chain.contains(&alias)) { return None; }
                        }
                        AstKind::AssignmentExpression(alias) if alias.operator == AssignmentOperator::Assign
                            && self.symbol(&alias.right) == Some(*symbol) => {
                            let AssignmentTarget::AssignmentTargetIdentifier(alias) = &alias.left else { return None };
                            if !analysis::identifier_symbol(alias, self.scoping).is_some_and(|alias| chain.contains(&alias)) { return None; }
                        }
                        AstKind::ObjectProperty(property) => {
                            self.xhr_probe_stage(&mut stage, "chunker.api.publication_protocol.006L2200");
                            publications.insert(self.xhr_chunker_publication(property, *symbol, root)?);
                        }
                        _ => return None,
                    }
                }
            }
            // This route describes the actual paired API publication, rather than
            // granting a general exception for arbitrary escaping callable values.
            self.xhr_probe_stage(&mut stage, "chunker.api.unique_publication_record.006L2208");
            if publications.len() != 1 { return None; }
            Some(helper)
        })();
        self.xhr_gate(result, stage)
    }

    fn xhr_chunker_publication(&self, property: &ObjectProperty<'a>, symbol: SymbolId,
        root: usize) -> Option<NodeId> {
        let mut stage = "xhr_chunker_publication.entry";
        let result = (|| {
            self.xhr_probe_stage(&mut stage, "chunker.api.ordinary_value_property.006L2214");
            if property.computed || property.method || property.kind != PropertyKind::Init
                || self.symbol(&property.value) != Some(symbol) { return None; }
            self.xhr_probe_stage(&mut stage, "chunker.api.property_parent.006L2216");
            let api_node = self.nodes.parent_id(property.node_id.get());
            self.xhr_probe_stage(&mut stage, "chunker.api.fresh_literal.006L2217");
            let AstKind::ObjectExpression(api) = self.nodes.kind(api_node) else { return None };
            let mut copies = 0;
            let mut keys = BTreeSet::new();
            for field in &api.properties {
                self.xhr_probe_stage(&mut stage, "chunker.api.no_spread.006L2221");
                let ObjectPropertyKind::ObjectProperty(field) = field else { return None };
                self.xhr_probe_stage(&mut stage, "chunker.api.ordinary_fields.006L2222");
                if field.computed || field.method || field.kind != PropertyKind::Init { return None; }
                self.xhr_probe_stage(&mut stage, "chunker.api.unique_static_keys.006L2223");
                if !keys.insert(field.key.static_name()?.to_string()) { return None; }
                if self.symbol(&field.value) == Some(symbol) { copies += 1; }
            }
            // Both public aliases copy the same value; no property key spelling
            // supplies identity. A forged or cross-scope second alias is refused.
            self.xhr_probe_stage(&mut stage, "chunker.api.paired_same_binding_values.006L2228");
            if copies != 2 { return None; }
            self.xhr_probe_stage(&mut stage, "chunker.api.detail_parent.006L2229");
            let detail_node = self.nodes.parent_id(api_node);
            self.xhr_probe_stage(&mut stage, "chunker.api.detail_property.006L2230");
            let AstKind::ObjectProperty(detail) = self.nodes.kind(detail_node) else { return None };
            self.xhr_probe_stage(&mut stage, "chunker.api.detail_key_and_kind.006L2231");
            if detail.computed || detail.method || detail.kind != PropertyKind::Init
                || detail.key.static_name().as_deref() != Some("detail") { return None; }
            self.xhr_probe_stage(&mut stage, "chunker.api.options_parent.006L2233");
            let options_node = self.nodes.parent_id(detail_node);
            self.xhr_probe_stage(&mut stage, "chunker.api.inline_event_options.006L2234");
            let AstKind::ObjectExpression(options) = self.nodes.kind(options_node) else { return None };
            self.xhr_probe_stage(&mut stage, "chunker.api.only_detail_option.006L2235");
            if options.properties.len() != 1 { return None; }
            self.xhr_probe_stage(&mut stage, "chunker.api.event_parent.006L2236");
            let new_node = self.nodes.parent_id(options_node);
            self.xhr_probe_stage(&mut stage, "chunker.api.event_construction.006L2237");
            let AstKind::NewExpression(event) = self.nodes.kind(new_node) else { return None };
            let arguments = new_args(event);
            self.xhr_probe_stage(&mut stage, "chunker.api.exact_event_options_argument.006L2239");
            if event.arguments.len() != 2 || !matches!(arguments.as_slice(), [_, Expression::ObjectExpression(found)]
                if found.node_id.get() == options_node) { return None; }
            self.xhr_probe_stage(&mut stage, "chunker.api.immediate_dispatch_parent.006L2241");
            let AstKind::CallExpression(dispatch) = self.nodes.kind(self.nodes.parent_id(new_node)) else { return None };
            self.xhr_probe_stage(&mut stage, "chunker.api.dispatch_method.006L2242");
            let (global, "dispatchEvent") = member(&dispatch.callee)? else { return None };
            self.xhr_probe_stage(&mut stage, "chunker.api.exact_dispatch_argument.006L2243");
            if dispatch.optional || dispatch.arguments.len() != 1
                || !matches!(args(dispatch).as_slice(), [Expression::NewExpression(found)] if found.node_id.get() == new_node)
            { return None; }
            self.xhr_probe_stage(&mut stage, "chunker.api.owning_init.006L2246");
            let init = self.owner[new_node.index()]?;
            self.xhr_probe_stage(&mut stage, "chunker.api.exporter_factory.006L2247");
            let factory = self.fns[init].parent?;
            self.xhr_probe_stage(&mut stage, "chunker.api.scope_lifecycle_and_platform_global.006L2248");
            if self.fns[factory].parent != Some(root) || self.custom_event_factory() != Some(factory)
                || !self.lifecycle_pairs(factory).iter().any(|pair| pair.init == init && !self.body_is_noop(pair.init))
                || !self.is_platform_global_object(global, factory, 8)
            { return None; }
            Some(api_node)
        })();
        self.xhr_gate(result, stage)
    }

    #[inline]
    fn xhr_probe_stage(&self, stage: &mut &'static str, next: &'static str) {
        *stage = next;
        #[cfg(test)]
        eprintln!("[split header origin] ENTER {next}");
    }

    /// Diagnostic instrumentation only: identity in production builds. The
    /// library diagnostic pins the first failed gate without weakening it.
    #[inline]
    fn xhr_gate<T>(&self, value: Option<T>, _predicate: &'static str) -> Option<T> {
        #[cfg(test)]
        {
            eprintln!("[header origin] {}: {}", _predicate, if value.is_some() { "PASS" } else { "REFUSE" });
            if value.is_none() {
                HEADER_ORIGIN_FIRST_REFUSAL.with(|first| {
                    let mut first = first.borrow_mut();
                    if first.is_none() { *first = Some(_predicate); }
                });
            }
        }
        value
    }

    fn xhr_decl<'e>(&self, statement: &'e Statement<'a>) -> Option<(SymbolId, Option<&'e Expression<'a>>)> {
        let Statement::VariableDeclaration(declaration) = statement else { return None };
        let [declaration] = declaration.declarations.as_slice() else { return None };
        let BindingPattern::BindingIdentifier(id) = &declaration.id else { return None };
        Some((id.symbol_id.get()?, declaration.init.as_ref()))
    }

    fn xhr_returns_symbol(&self, statement: &Statement<'a>, symbol: SymbolId) -> bool {
        matches!(statement, Statement::ReturnStatement(ret)
            if ret.argument.as_ref().and_then(|value| self.symbol(value)) == Some(symbol))
    }

    fn xhr_returns_null(statement: &Statement<'a>) -> bool {
        matches!(statement, Statement::ReturnStatement(ret) if ret.argument.as_ref().is_some_and(is_null))
    }

    fn xhr_descriptor_helper(&self, helper: usize) -> Option<()> {
        if !self.xhr_sync_parameters(helper, 2) { return None; }
        // Same descriptor-walk contract as threads::shared::descriptor_search,
        // tied here to the actual called helper rather than its name/role.
        let [initial, Statement::WhileStatement(walk), end] = self.body_statements(helper) else { return None };
        let (cursor, initial) = self.xhr_decl(initial)?;
        if self.slot_index(initial?, helper) != Some(0) || !Self::xhr_returns_null(end)
            || self.symbol(Self::xhr_nonnull(&walk.test)?) != Some(cursor)
        { return None; }
        let [descriptor, Statement::IfStatement(found)] = Self::xhr_block(&walk.body)? else { return None };
        let (descriptor, initial) = self.xhr_decl(descriptor)?;
        let Expression::CallExpression(read) = strip(initial?) else { return None };
        let read_args = args(read);
        let [object, key] = read_args.as_slice() else { return None };
        if self.path(&read.callee).as_deref() != Some("Object.getOwnPropertyDescriptor")
            || self.symbol(object) != Some(cursor) || self.slot_index(key, helper) != Some(1)
            || self.symbol(Self::xhr_nonnull(&found.test)?) != Some(descriptor)
        { return None; }
        let [pack, ret] = Self::xhr_block(&found.consequent)? else { return None };
        let Expression::ObjectExpression(record) = strip(self.xhr_plain_write(pack, cursor)?) else { return None };
        let fields = object_members_of(record);
        if record.properties.len() != 2 || fields.len() != 2 || !fields.iter().all(|field| match field.name.as_str() {
            "containingObj" => self.symbol(field.value) == Some(cursor),
            "desc" => self.symbol(field.value) == Some(descriptor),
            _ => false,
        }) || !self.xhr_returns_symbol(ret, cursor) { return None; }
        let [step, Statement::ContinueStatement(next)] = Self::xhr_block(found.alternate.as_ref()?)? else { return None };
        let Expression::CallExpression(parent) = strip(self.xhr_plain_write(step, cursor)?) else { return None };
        if self.path(&parent.callee).as_deref() != Some("Object.getPrototypeOf")
            || !matches!(args(parent).as_slice(), [object] if self.symbol(object) == Some(cursor))
            || next.label.is_some() || self.assigns.get(&cursor)?.len() != 2
        { return None; }
        self.single_value(descriptor)?;
        Some(())
    }

    fn xhr_registry_item<'e>(&self, expression: &'e Expression<'a>, helper: usize,
        index: Option<SymbolId>) -> Option<SymbolId> {
        let mut expression = strip(expression);
        if let Some(index) = index {
            let Expression::ComputedMemberExpression(item) = expression else { return None };
            if self.symbol(&item.expression) != Some(index) { return None; }
            expression = strip(&item.object);
        }
        let Expression::ComputedMemberExpression(entries) = expression else { return None };
        (self.slot_index(&entries.expression, helper) == Some(1)).then(|| self.symbol(&entries.object)).flatten()
    }

    fn xhr_registry_helper(&self, helper: usize) -> Option<SymbolId> {
        if !self.xhr_sync_parameters(helper, 2) { return None; }
        let [Statement::IfStatement(absent), initial, Statement::WhileStatement(scan), end] =
            self.body_statements(helper) else { return None };
        let registry = self.xhr_registry_item(Self::xhr_nullish_absent(&absent.test)?, helper, None)?;
        let [null] = Self::xhr_block(&absent.consequent)? else { return None };
        if absent.alternate.is_some() || !Self::xhr_returns_null(null) || !Self::xhr_returns_null(end) { return None; }
        let (index, initial) = self.xhr_decl(initial)?;
        if numeric_index(initial?) != Some(0) { return None; }
        let Expression::BinaryExpression(limit) = strip(&scan.test) else { return None };
        let (entries, "length") = member(&limit.right)? else { return None };
        if limit.operator != BinaryOperator::LessThan || self.symbol(&limit.left) != Some(index)
            || self.xhr_registry_item(entries, helper, None) != Some(registry) { return None; }
        let [Statement::IfStatement(found)] = Self::xhr_block(&scan.body)? else { return None };
        let Expression::BinaryExpression(test) = strip(&found.test) else { return None };
        let (item, "object") = member(&test.right)? else { return None };
        if test.operator != BinaryOperator::StrictEquality || self.slot_index(&test.left, helper) != Some(0)
            || self.xhr_registry_item(item, helper, Some(index)) != Some(registry) { return None; }
        let [Statement::ReturnStatement(ret)] = Self::xhr_block(&found.consequent)? else { return None };
        if self.xhr_registry_item(ret.argument.as_ref()?, helper, Some(index)) != Some(registry) { return None; }
        let [Statement::ExpressionStatement(increment), Statement::ContinueStatement(next)] =
            Self::xhr_block(found.alternate.as_ref()?)? else { return None };
        let Expression::UpdateExpression(increment) = strip(&increment.expression) else { return None };
        let SimpleAssignmentTarget::AssignmentTargetIdentifier(counter) = &increment.argument else { return None };
        if increment.operator != oxc_syntax::operator::UpdateOperator::Increment || next.label.is_some()
            || analysis::identifier_symbol(counter, self.scoping) != Some(index) { return None; }
        Some(registry)
    }

    fn xhr_member_symbol(&self, expression: &Expression<'a>, field: &str, symbol: SymbolId) -> bool {
        member(expression).is_some_and(|(object, name)| name == field && self.symbol(object) == Some(symbol))
    }

    fn xhr_inside(&self, symbol: SymbolId, root: NodeId) -> bool {
        self.scoping.get_resolved_references(symbol).all(|reference| {
            let mut node = reference.node_id();
            loop {
                if node == root { return true; }
                let parent = self.nodes.parent_id(node);
                if parent == node { return false; }
                node = parent;
            }
        })
    }

    fn xhr_header_value_settings(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        // Inspect the outer optional member without flattening or evaluating
        // either getter read. Calls and dynamic computed keys are not origins.
        let (originals, name) = match strip(expression) {
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::StaticMemberExpression(found) =>
                    (&found.object, found.property.name.as_str()),
                ChainElement::ComputedMemberExpression(found) => {
                    let Expression::StringLiteral(name) = strip(&found.expression) else { return None };
                    (&found.object, name.value.as_str())
                }
                _ => return None,
            },
            _ => member(expression)?,
        };
        if name != "value" { return None; }
        let (bundle, "originals") = member(originals)? else { return None };
        let (instrumented, "setRequestHeader") = member(bundle)? else { return None };
        let (settings, "instrumented") = member(instrumented)? else { return None };
        self.symbol(settings)
    }

    /// An own, straight-line initializer still reaching this source point.
    /// Later phase writes are allowed; earlier writes and nested captures are
    /// not. This does not relax the shared whole-lifetime single_value proof.
    fn xhr_initial_local_before(&self, symbol: SymbolId, function: usize,
        before: u32) -> Option<&'a Expression<'a>> {
        if self.scoping.symbol_declarations(symbol).count() != 1 { return None; }
        let declaration_node = self.scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(declaration) = self.nodes.kind(declaration_node) else { return None };
        let initial = declaration.init.as_ref()?;
        let statement = self.enclosing_statement(declaration_node)?;
        if self.owner[declaration_node.index()] != Some(function)
            || !self.statement_is_direct_body_child(statement, self.function_body_span(function)?)
            || self.nodes.kind(statement).span().end > before
            || self.scoping.get_resolved_references(symbol).any(|reference| {
                self.owner[reference.node_id().index()] != Some(function)
                    || (reference.is_write() && self.nodes.kind(reference.node_id()).span().start < before)
            })
        { return None; }
        Some(initial)
    }

    fn xhr_cache_receiver_from_descriptor(&self, receiver: &Expression<'a>, call: &CallExpression<'a>,
        interceptor: usize, descriptor_record: SymbolId, descriptor_helper: usize) -> bool {
        let Some(symbol) = self.symbol(receiver) else { return false };
        let Some(initial) = self.xhr_initial_local_before(symbol, interceptor, call.span.start) else { return false };
        if !self.xhr_member_symbol(initial, "containingObj", descriptor_record)
            // The object cannot be passed out or mutated before this lookup.
            || self.scoping.get_resolved_references(symbol).any(|reference|
                self.nodes.kind(reference.node_id()).span().start < call.span.start)
        { return false; }
        let Some(record) = self.xhr_initial_local_before(descriptor_record, interceptor, initial.span().start) else { return false };
        let Expression::CallExpression(read) = strip(record) else { return false };
        if self.resolve_fn(&read.callee) != Some(descriptor_helper)
            || !matches!(args(read).as_slice(), [object, key]
                if self.slot_index(object, interceptor) == Some(0) && self.slot_index(key, interceptor) == Some(1))
        { return false; }
        // Before extracting containingObj, only the descriptor's null guard
        // may read the fresh record. No property mutation or donor can replace it.
        self.scoping.get_resolved_references(descriptor_record).all(|reference| {
            if self.nodes.kind(reference.node_id()).span().start >= initial.span().start { return true; }
            matches!(self.nodes.kind(self.nodes.parent_id(reference.node_id())), AstKind::BinaryExpression(test)
                if test.operator == BinaryOperator::Equality && is_null(&test.right)
                    && self.symbol(&test.left) == Some(descriptor_record))
        })
    }

    fn xhr_original_header(&self, factory: usize, init: usize, callback: usize,
        header: &Expression<'a>, settings: SymbolId, prototype: SymbolId, interceptor: usize) -> Option<()> {
        let header_symbol = self.symbol(header)?;
        let mut branches = self.body_statements(factory).iter().filter_map(|statement| {
            let Statement::IfStatement(branch) = statement else { return None };
            (self.xhr_header_value_settings(Self::xhr_nonnull(&branch.test)?) == Some(settings)).then_some(branch)
        });
        let branch = self.xhr_gate(branches.next(), "header.original.settings_guard")?;
        if branches.next().is_some() { return None; }
        let [selected] = Self::xhr_block(&branch.consequent)? else { return None };
        if self.xhr_header_value_settings(self.xhr_plain_write(selected, header_symbol)?) != Some(settings)
            || self.values.get(&header_symbol)?.len() != 2 || self.assigns.get(&header_symbol)?.len() != 2
            || self.scoping.get_resolved_references(header_symbol).filter(|r| r.is_write()).count() != 2
        { return None; }
        let [descriptor, temporary, Statement::IfStatement(absent), publish] =
            Self::xhr_block(branch.alternate.as_ref()?)? else { return None };
        let (descriptor, descriptor_init) = self.xhr_decl(descriptor)?;
        let (temporary, temporary_init) = self.xhr_decl(temporary)?;
        if temporary_init.is_some() || self.symbol(self.xhr_plain_write(publish, header_symbol)?) != Some(temporary)
            || self.symbol(Self::xhr_nullish_absent(&absent.test)?) != Some(descriptor)
            || !self.xhr_inside(temporary, branch.node_id.get())
            || !self.xhr_inside(descriptor, branch.node_id.get())
            || self.values.get(&temporary)?.len() != 4 || self.assigns.get(&temporary)?.len() != 4
            || self.scoping.get_resolved_references(temporary).filter(|r| r.is_write()).count() != 4
        { return None; }
        let Expression::CallExpression(descriptor_call) = strip(descriptor_init?) else { return None };
        let descriptor_helper = self.xhr_gate(self.xhr_stable_callable(&descriptor_call.callee, 8),
            "header.original.descriptor_callable")?;
        self.xhr_gate(self.xhr_descriptor_helper(descriptor_helper), "header.original.descriptor_protocol")?;
        let descriptor_args = args(descriptor_call);
        if !matches!(descriptor_args.as_slice(), [receiver, name]
            if self.symbol(receiver) == Some(prototype) && string_value(name) == Some("setRequestHeader"))
        { return None; }
        // Reuse the exact descriptor input of the source-proven interceptor.
        let descriptor_record = self.values.iter().find_map(|(symbol, values)| values.iter().any(|value| {
            let Expression::CallExpression(call) = strip(value) else { return false };
            self.owner[call.node_id.get().index()] == Some(interceptor)
                && self.resolve_fn(&call.callee) == Some(descriptor_helper)
                && matches!(args(call).as_slice(), [object, key]
                    if self.slot_index(object, interceptor) == Some(0) && self.slot_index(key, interceptor) == Some(1))
        }).then_some(*symbol))?;
        let [empty] = Self::xhr_block(&absent.consequent)? else { return None };
        if !Self::is_void(self.xhr_plain_write(empty, temporary)?) { return None; }
        let [lookup, Statement::IfStatement(cached)] = Self::xhr_block(absent.alternate.as_ref()?)? else { return None };
        let Expression::CallExpression(cache_call) = strip(self.xhr_plain_write(lookup, temporary)?) else { return None };
        let cache_helper = self.xhr_gate(self.xhr_stable_callable(&cache_call.callee, 8),
            "header.original.cache_callable")?;
        let registry = self.xhr_gate(self.xhr_registry_helper(cache_helper), "header.original.cache_protocol")?;
        if !matches!(args(cache_call).as_slice(), [object, key]
            if self.xhr_member_symbol(object, "containingObj", descriptor) && string_value(key) == Some("setRequestHeader"))
            || self.symbol(Self::xhr_nonnull(&cached.test)?) != Some(temporary)
        { return None; }
        if !self.own[interceptor].calls.iter().any(|call| {
            if self.resolve_fn(&call.callee) != Some(cache_helper) { return false; }
            let arguments = args(call);
            let [object, key] = arguments.as_slice() else { return false };
            self.slot_index(key, interceptor) == Some(1)
                && self.xhr_cache_receiver_from_descriptor(object, call, interceptor, descriptor_record, descriptor_helper)
        }) { return None; }
        if !self.own[interceptor].assignments.iter().any(|write| {
            let AssignmentTarget::ComputedMemberExpression(target) = &write.left else { return false };
            write.operator == AssignmentOperator::Assign && self.symbol(&target.object) == Some(registry)
                && self.slot_index(&target.expression, interceptor) == Some(1)
        }) { return None; }
        let [original] = Self::xhr_block(&cached.consequent)? else { return None };
        if !self.xhr_member_symbol(self.xhr_plain_write(original, temporary)?, "original", temporary) { return None; }
        let [original, record, entries, Statement::IfStatement(no_entries), append] =
            Self::xhr_block(cached.alternate.as_ref()?)? else { return None };
        let (desc, "value") = member(self.xhr_plain_write(original, temporary)?)? else { return None };
        if !self.xhr_member_symbol(desc, "desc", descriptor) { return None; }
        let (record, record_init) = self.xhr_decl(record)?;
        let Expression::ObjectExpression(record_value) = strip(record_init?) else { return None };
        let fields = object_members_of(record_value);
        if record_value.properties.len() != 2 || fields.len() != 2 || !fields.iter().all(|field| match field.name.as_str() {
            "object" => self.xhr_member_symbol(field.value, "containingObj", descriptor),
            "original" => self.symbol(field.value) == Some(temporary),
            _ => false,
        }) { return None; }
        let (entries, entries_init) = self.xhr_decl(entries)?;
        if !self.xhr_member_symbol(entries_init?, "setRequestHeader", registry)
            || self.symbol(Self::xhr_nullish_absent(&no_entries.test)?) != Some(entries)
            || no_entries.alternate.is_some() { return None; }
        let [create, Statement::ExpressionStatement(store)] = Self::xhr_block(&no_entries.consequent)? else { return None };
        let Expression::ArrayExpression(array) = strip(self.xhr_plain_write(create, entries)?) else { return None };
        let Expression::AssignmentExpression(store) = strip(&store.expression) else { return None };
        if !array.elements.is_empty() || store.operator != AssignmentOperator::Assign
            || target_member_name(&store.left) != Some("setRequestHeader")
            || target_member_object(&store.left).and_then(|object| self.symbol(object)) != Some(registry)
            || self.symbol(&store.right) != Some(entries) { return None; }
        let Statement::ExpressionStatement(append) = append else { return None };
        let Expression::CallExpression(append) = strip(&append.expression) else { return None };
        let append_args = args(append);
        if !matches!(append_args.as_slice(), [push, receiver, value]
            if self.path(push).as_deref() == Some("Array.prototype.push")
                && self.symbol(receiver) == Some(entries) && self.symbol(value) == Some(record))
            || !self.xhr_inside(record, branch.node_id.get()) || !self.xhr_inside(entries, branch.node_id.get())
        { return None; }
        // The native interceptor's registry publication uses the same call
        // adapter with the unshadowed Array.prototype.push operation.
        let adapter = self.xhr_gate(self.xhr_native_call_adapter(&append.callee),
            "header.original.native_call_adapter")?;
        if !self.own[interceptor].calls.iter().any(|call| self.symbol(&call.callee) == Some(adapter)
            && args(call).first().is_some_and(|arg| self.path(arg).as_deref() == Some("Array.prototype.push")))
        { return None; }
        // Account for all uses of the callable that crosses the join. Factory
        // writes are the two recognized alternatives; reads can only guard
        // init or be the exact receiver-preserving calls already flow-proven.
        for reference in self.scoping.get_resolved_references(header_symbol) {
            if reference.is_write() { continue; }
            let node = reference.node_id();
            match self.nodes.kind(self.nodes.parent_id(node)) {
                AstKind::BinaryExpression(test) if self.owner[node.index()] == Some(init)
                    && test.operator == BinaryOperator::Equality && is_null(&test.right) => {}
                AstKind::StaticMemberExpression(method) if self.owner[node.index()] == Some(callback)
                    && method.property.name == "call" => {}
                _ => return None,
            }
        }
        Some(())
    }

    fn xhr_native_call_adapter(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        let symbol = self.symbol(expression)?;
        let Expression::CallExpression(capture) = strip(self.single_value(symbol)?) else { return None };
        if !matches!(args(capture).as_slice(), [native] if self.path(native).as_deref() == Some("Object.call")) { return None; }
        let binder = self.symbol(&capture.callee)?;
        let Expression::CallExpression(bind) = strip(self.single_value(binder)?) else { return None };
        // Existing p09 uncurry contract, restricted to this actual unshadowed
        // native path rather than accepting any x.call.bind property bag.
        if self.path(&bind.callee).as_deref() != Some("Object.call.bind")
            || !matches!(args(bind).as_slice(), [binding, call]
                if self.path(binding).as_deref() == Some("Object.bind")
                    && self.path(call).as_deref() == Some("Object.call")) { return None; }
        Some(symbol)
    }

    fn xhr_map_write<'e>(&self, statement: &'e Statement<'a>, output: SymbolId)
        -> Option<(&'e Expression<'a>, &'e Expression<'a>)> {
        let Statement::ExpressionStatement(statement) = statement else { return None };
        let Expression::AssignmentExpression(write) = strip(&statement.expression) else { return None };
        let AssignmentTarget::ComputedMemberExpression(target) = &write.left else { return None };
        (write.operator == AssignmentOperator::Assign && self.symbol(&target.object) == Some(output))
            .then_some((&target.expression, &write.right))
    }

    fn xhr_chunk_slice(&self, value: &Expression<'a>, helper: usize,
        start: Option<SymbolId>, end: Option<SymbolId>) -> bool {
        let Expression::CallExpression(slice) = strip(value) else { return false };
        let Some((input, "slice")) = member(&slice.callee) else { return false };
        let arguments = args(slice);
        let [from, to] = arguments.as_slice() else { return false };
        if self.slot_index(input, helper) != Some(1) { return false; }
        match (start, end) {
            (None, None) => numeric_index(from) == Some(0) && self.slot_index(to, helper) == Some(2),
            (Some(cursor), Some(size_owner)) => self.symbol(from) == Some(cursor)
                && matches!(strip(to), Expression::BinaryExpression(sum)
                    if sum.operator == BinaryOperator::Addition && self.symbol(&sum.left) == Some(cursor)
                        && self.symbol(&sum.right) == Some(size_owner)),
            _ => false,
        }
    }

    fn xhr_chunker_helper(&self, helper: usize) -> Option<()> {
        if !self.xhr_sync_parameters(helper, 3) { return None; }
        let [initial, Statement::IfStatement(split), ret] = self.body_statements(helper) else { return None };
        let (output, initial) = self.xhr_decl(initial)?;
        if !self.is_null_prototype_object(initial?) || !self.xhr_returns_symbol(ret, output) { return None; }
        let Expression::LogicalExpression(guard) = strip(&split.test) else { return None };
        let Expression::BinaryExpression(length) = strip(&guard.right) else { return None };
        let (value, "length") = member(&length.left)? else { return None };
        if guard.operator != LogicalOperator::And || self.slot_index(Self::xhr_nonnull(&guard.left)?, helper) != Some(1)
            || length.operator != BinaryOperator::GreaterThan || self.slot_index(value, helper) != Some(1)
            || self.slot_index(&length.right, helper) != Some(2) { return None; }
        let size = self.symbol(&length.right)?;
        let [first, index, cursor, Statement::WhileStatement(rest)] = Self::xhr_block(&split.consequent)? else { return None };
        let (key, value) = self.xhr_map_write(first, output)?;
        if self.slot_index(key, helper) != Some(0) || !self.xhr_chunk_slice(value, helper, None, None) { return None; }
        let (index, index_init) = self.xhr_decl(index)?;
        let (cursor, cursor_init) = self.xhr_decl(cursor)?;
        if numeric_index(index_init?) != Some(0) || self.slot_index(cursor_init?, helper) != Some(2) { return None; }
        let Expression::BinaryExpression(limit) = strip(&rest.test) else { return None };
        let (input, "length") = member(&limit.right)? else { return None };
        if limit.operator != BinaryOperator::LessThan || self.symbol(&limit.left) != Some(cursor)
            || self.slot_index(input, helper) != Some(1) { return None; }
        let [write, Statement::ExpressionStatement(increment), advance] = Self::xhr_block(&rest.body)? else { return None };
        let (key, value) = self.xhr_map_write(write, output)?;
        let Expression::BinaryExpression(key) = strip(key) else { return None };
        if key.operator != BinaryOperator::Addition || self.slot_index(&key.left, helper) != Some(0)
            || self.symbol(&key.right) != Some(index) || !self.xhr_chunk_slice(value, helper, Some(cursor), Some(size)) { return None; }
        let Expression::UpdateExpression(increment) = strip(&increment.expression) else { return None };
        let SimpleAssignmentTarget::AssignmentTargetIdentifier(index_id) = &increment.argument else { return None };
        let Expression::BinaryExpression(sum) = strip(self.xhr_plain_write(advance, cursor)?) else { return None };
        if increment.operator != oxc_syntax::operator::UpdateOperator::Increment
            || analysis::identifier_symbol(index_id, self.scoping) != Some(index)
            || sum.operator != BinaryOperator::Addition || self.symbol(&sum.left) != Some(cursor)
            || self.symbol(&sum.right) != Some(size) { return None; }
        let [whole] = Self::xhr_block(split.alternate.as_ref()?)? else { return None };
        let (key, value) = self.xhr_map_write(whole, output)?;
        let Expression::BinaryExpression(text) = strip(value) else { return None };
        if self.slot_index(key, helper) != Some(0) || text.operator != BinaryOperator::Addition
            || string_value(&text.left) != Some("") || self.slot_index(&text.right, helper) != Some(1) { return None; }
        Some(())
    }

    fn xhr_options_helper(&self, helper: usize) -> Option<()> {
        // Reuse the existing source-backed packet/collector/sextet contract,
        // then require the real helper's executable writer sequence. The six
        // fields cannot be donated by a nested function or dormant branch.
        if !self.xhr_sync_parameters(helper, 1) || !self.xhr_envelope_builders.contains(&helper) { return None; }
        let [prefix @ .., f, b, c, d, z, a, ret] = self.body_statements(helper) else { return None };
        let Statement::ReturnStatement(ret) = ret else { return None };
        let output = self.symbol(ret.argument.as_ref()?)?;
        for (statement, name) in [(f,"f"),(b,"b"),(c,"c"),(d,"d"),(z,"z"),(a,"a")] {
            let Statement::ExpressionStatement(statement) = statement else { return None };
            let Expression::AssignmentExpression(write) = strip(&statement.expression) else { return None };
            if write.operator != AssignmentOperator::Assign || target_member_name(&write.left) != Some(name)
                || target_member_object(&write.left).and_then(|value| self.symbol(value)) != Some(output) { return None; }
        }
        let [preparation @ .., Statement::WhileStatement(groups), Statement::IfStatement(tail)] = prefix else { return None };
        if !preparation.iter().all(|statement| matches!(statement,
            Statement::VariableDeclaration(_) | Statement::ExpressionStatement(_)))
            || !self.xhr_encoding_falls_through(&groups.body) || !self.xhr_encoding_falls_through(&tail.consequent)
            || !tail.alternate.as_ref().is_some_and(|statement| self.xhr_encoding_falls_through(statement))
        { return None; }
        Some(())
    }

    fn xhr_encoding_falls_through(&self, statement: &Statement<'a>) -> bool {
        match statement {
            Statement::VariableDeclaration(_) | Statement::ExpressionStatement(_) => true,
            Statement::BlockStatement(block) => block.body.iter().all(|s| self.xhr_encoding_falls_through(s)),
            Statement::IfStatement(branch) => self.xhr_encoding_falls_through(&branch.consequent)
                && branch.alternate.as_ref().is_none_or(|s| self.xhr_encoding_falls_through(s)),
            _ => false,
        }
    }

    fn xhr_boxed_from(&self, statement: &Statement<'a>, source: SymbolId) -> Option<SymbolId> {
        let (boxed, initial) = self.xhr_decl(statement)?;
        let Expression::CallExpression(object) = strip(initial?) else { return None };
        if self.path(&object.callee).as_deref() != Some("Object")
            || !matches!(args(object).as_slice(), [value] if self.symbol(value) == Some(source))
        { return None; }
        self.single_value(boxed)?;
        Some(boxed)
    }

    fn xhr_key(&self, loop_: &ForInStatement<'a>, boxed: SymbolId) -> Option<SymbolId> {
        let ForStatementLeft::VariableDeclaration(declaration) = &loop_.left else { return None };
        let [key] = declaration.declarations.as_slice() else { return None };
        let BindingPattern::BindingIdentifier(key_id) = &key.id else { return None };
        if key.init.is_some() || self.symbol(&loop_.right) != Some(boxed) { return None; }
        key_id.symbol_id.get()
    }

    fn xhr_own_guard(&self, expression: &Expression<'a>, object: SymbolId, key: SymbolId) -> bool {
        let mut test = strip(expression);
        for _ in 0..2 {
            let Expression::UnaryExpression(not) = test else { return false };
            if not.operator != oxc_syntax::operator::UnaryOperator::LogicalNot { return false; }
            test = strip(&not.argument);
        }
        let Expression::CallExpression(call) = test else { return false };
        self.path(&call.callee).as_deref() == Some("Object.prototype.hasOwnProperty.call")
            && matches!(args(call).as_slice(), [source, name]
                if self.symbol(source) == Some(object) && self.symbol(name) == Some(key))
    }

    fn xhr_options_pipeline(&self, options: &Statement<'a>, output_init: &Statement<'a>,
        merge: &Statement<'a>, emission: &Statement<'a>, callback: usize, header: &Expression<'a>) -> Option<(usize, usize)> {
        let mut stage = "xhr_options_pipeline.entry";
        let result = (|| {
            self.xhr_probe_stage(&mut stage, "pipeline.output.emission_identity.006L2657");
            let output = self.symbol(self.xhr_gate(self.xhr_emission(emission, callback, header), "pipeline.emission")?)?;
            self.xhr_probe_stage(&mut stage, "pipeline.output.initialization.006L2658");
            let output_value = match output_init {
                Statement::VariableDeclaration(_) => {
                    let (symbol, initial) = self.xhr_decl(output_init)?;
                    if symbol != output { return None; }
                    initial?
                }
                _ => self.xhr_plain_write(output_init, output)?,
            };
            self.xhr_probe_stage(&mut stage, "pipeline.output.null_prototype.006L2666");
            if !self.is_null_prototype_object(output_value) { return None; }
            self.xhr_probe_stage(&mut stage, "pipeline.input.options_declaration.006L2667");
            let (options, options_init) = self.xhr_decl(options)?;
            self.xhr_probe_stage(&mut stage, "pipeline.input.options_immutable.006L2668");
            self.single_value(options)?;
            self.xhr_probe_stage(&mut stage, "pipeline.input.actual_options_call.006L2669");
            let Expression::CallExpression(build) = strip(options_init?) else { return None };
            self.xhr_probe_stage(&mut stage, "pipeline.input.options_callable.006L2670");
            let builder = self.xhr_gate(self.xhr_stable_callable(&build.callee, 8), "pipeline.options_callable")?;
            self.xhr_probe_stage(&mut stage, "pipeline.input.options_argument.006L2671");
            if !matches!(args(build).as_slice(), [value] if Self::is_void(value)) { return None; }
            self.xhr_probe_stage(&mut stage, "pipeline.input.options_source_order.006L2672");
            self.xhr_gate(self.xhr_options_helper(builder), "pipeline.options_helper.source_and_order")?;
            self.xhr_probe_stage(&mut stage, "pipeline.input.merge_guard.006L2673");
            let Statement::IfStatement(present) = merge else { return None };
            self.xhr_probe_stage(&mut stage, "pipeline.input.merge_guard_identity.006L2674");
            if present.alternate.is_some() || self.symbol(self.xhr_present(&present.test)?) != Some(options) { return None; }
            let statements = Self::xhr_block(&present.consequent)?;
            self.xhr_probe_stage(&mut stage, "pipeline.input.outer_loop_shape.006L2676");
            let (boxed, alias, outer) = match statements {
                [boxed, Statement::ForInStatement(outer)] => (boxed, None, outer),
                [boxed, alias, Statement::ForInStatement(outer)] => {
                    let (alias, initial) = self.xhr_decl(alias)?;
                    if initial.is_some_and(|value| !Self::is_void(value)) { return None; }
                    (boxed, Some(alias), outer)
                }
                _ => return None,
            };
            self.xhr_probe_stage(&mut stage, "pipeline.input.boxed_options.006L2685");
            let boxed = self.xhr_boxed_from(boxed, options)?;
            self.xhr_probe_stage(&mut stage, "pipeline.input.options_loop_key.006L2686");
            let key = self.xhr_key(outer, boxed)?;
            self.xhr_probe_stage(&mut stage, "pipeline.input.own_guard_shape.006L2687");
            let [Statement::IfStatement(own)] = Self::xhr_block(&outer.body)? else { return None };
            self.xhr_probe_stage(&mut stage, "pipeline.input.own_guard_identity.006L2688");
            if own.alternate.is_some() || !self.xhr_own_guard(&own.test, options, key) { return None; }
            self.xhr_probe_stage(&mut stage, "pipeline.chunk.call_then_guard.006L2689");
            let [chunks, Statement::IfStatement(chunks_present)] = Self::xhr_block(&own.consequent)? else { return None };
            let (chunks, chunks_init) = self.xhr_decl(chunks)?;
            self.xhr_probe_stage(&mut stage, "pipeline.chunk.immutable_result.006L2691");
            self.single_value(chunks)?;
            let Expression::CallExpression(chunk) = strip(chunks_init?) else { return None };
            self.xhr_probe_stage(&mut stage, "pipeline.chunk.corrected_callable_origin.006L2693");
            let chunker = self.xhr_gate(self.xhr_published_chunker(&chunk.callee, callback), "pipeline.chunker_closed_origin_and_api_publication")?;
            self.xhr_probe_stage(&mut stage, "pipeline.chunk.owned_protocol.006L2694");
            self.xhr_gate(self.xhr_chunker_helper(chunker), "pipeline.chunker.owned_protocol")?;
            self.xhr_probe_stage(&mut stage, "pipeline.chunk.arguments.006L2695");
            let chunk_args = args(chunk);
            let [name, value, limit] = chunk_args.as_slice() else { return None };
            let Expression::BinaryExpression(name) = strip(name) else { return None };
            let Expression::ComputedMemberExpression(value) = strip(value) else { return None };
            self.xhr_probe_stage(&mut stage, "pipeline.chunk.name_value_size_and_guard.006L2699");
            if name.operator != BinaryOperator::Addition || !string_value(&name.left).is_some_and(|prefix| !prefix.is_empty())
                || self.symbol(&name.right) != Some(key) || self.symbol(&value.object) != Some(options)
                || self.symbol(&value.expression) != Some(key) || !numeric_index(limit).is_some_and(|limit| limit > 0)
                || chunks_present.alternate.is_some() || self.symbol(self.xhr_present(&chunks_present.test)?) != Some(chunks)
            { return None; }
            self.xhr_probe_stage(&mut stage, "pipeline.writer.inner_loop_shape.006L2704");
            let [boxed, Statement::ForInStatement(inner)] = Self::xhr_block(&chunks_present.consequent)? else { return None };
            self.xhr_probe_stage(&mut stage, "pipeline.writer.boxed_chunks.006L2705");
            let boxed = self.xhr_boxed_from(boxed, chunks)?;
            self.xhr_probe_stage(&mut stage, "pipeline.writer.chunk_key.006L2706");
            let inner_key = self.xhr_key(inner, boxed)?;
            self.xhr_probe_stage(&mut stage, "pipeline.writer.key_alias.006L2707");
            let (key, copy) = match (Self::xhr_block(&inner.body)?, alias) {
                ([Statement::IfStatement(copy)], None) => (inner_key, copy),
                ([assign, Statement::IfStatement(copy)], Some(alias)) => {
                    if self.symbol(self.xhr_plain_write(assign, alias)?) != Some(inner_key)
                        || self.assigns.get(&alias)?.len() != 1
                    { return None; }
                    (alias, copy)
                }
                _ => return None,
            };
            self.xhr_probe_stage(&mut stage, "pipeline.writer.own_guard.006L2717");
            if copy.alternate.is_some() || !self.xhr_own_guard(&copy.test, chunks, key) { return None; }
            self.xhr_probe_stage(&mut stage, "pipeline.writer.single_copy_effect.006L2718");
            let [write] = Self::xhr_block(&copy.consequent)? else { return None };
            let (destination, value) = self.xhr_map_write(write, output)?;
            let Expression::ComputedMemberExpression(value) = strip(value) else { return None };
            self.xhr_probe_stage(&mut stage, "pipeline.writer.destination_and_value.006L2721");
            if self.symbol(destination) != Some(key) || self.symbol(&value.object) != Some(chunks)
                || self.symbol(&value.expression) != Some(key) { return None; }
            Some((builder, chunker))
        })();
        self.xhr_gate(result, stage)
    }

    fn xhr_sync_parameters(&self, helper: usize, count: usize) -> bool {
        let FnNode::Function(function) = self.fns[helper].func else { return false };
        self.own[helper].calls.iter().all(|call| !call.optional
            && call.arguments.iter().all(|argument| argument.as_expression().is_some()))
            && !function.r#async && !function.generator && function.params.rest.is_none()
            && function.params.items.len() == count
            && function.params.items.iter().all(|parameter| parameter.initializer.is_none()
                && matches!(&parameter.pattern, BindingPattern::BindingIdentifier(_)))
    }

    fn xhr_nullish_absent<'e>(expression: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
        let Expression::BinaryExpression(binary) = strip(expression) else { return None };
        (binary.operator == BinaryOperator::Equality && is_null(&binary.right)).then_some(&binary.left)
    }

    fn xhr_block<'e>(statement: &'e Statement<'a>) -> Option<&'e [Statement<'a>]> {
        let Statement::BlockStatement(block) = statement else { return None };
        Some(block.body.as_slice())
    }

    fn xhr_result_return(&self, statement: &Statement<'a>, callback: usize) -> bool {
        matches!(statement, Statement::ReturnStatement(ret)
            if ret.argument.as_ref().is_some_and(|value| self.slot_index(value, callback) == Some(1)))
    }

    fn xhr_nonnull<'e>(expression: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
        let Expression::BinaryExpression(binary) = strip(expression) else { return None };
        (binary.operator == BinaryOperator::Inequality && is_null(&binary.right))
            .then_some(&binary.left)
    }

    fn xhr_present<'e>(&self, expression: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
        let Expression::LogicalExpression(and) = strip(expression) else { return None };
        let Expression::BinaryExpression(null) = strip(&and.left) else { return None };
        let Expression::BinaryExpression(undefined) = strip(&and.right) else { return None };
        (and.operator == LogicalOperator::And
            && null.operator == BinaryOperator::StrictInequality && is_null(&null.right)
            && undefined.operator == BinaryOperator::StrictInequality && Self::is_void(&undefined.right)
            && self.same_binding(&null.left, &undefined.left)).then_some(&null.left)
    }

    /// Recognize the actual emission edge, not a header call somewhere in a
    /// subtree. The output's null guard directly owns Object(output), for-in,
    /// its own-property guard, and the single native-header invocation.
    fn xhr_emission<'e>(&self, statement: &'e Statement<'a>, callback: usize,
        header: &Expression<'a>) -> Option<&'e Expression<'a>> {
        let Statement::IfStatement(present) = statement else { return None };
        if present.alternate.is_some() { return None; }
        let output = self.xhr_present(&present.test)?;
        let [Statement::VariableDeclaration(boxed), Statement::ForInStatement(keys)] =
            Self::xhr_block(&present.consequent)? else { return None };
        let [boxed] = boxed.declarations.as_slice() else { return None };
        let BindingPattern::BindingIdentifier(boxed_id) = &boxed.id else { return None };
        let Expression::CallExpression(object) = strip(boxed.init.as_ref()?) else { return None };
        let object_args = args(object);
        let [source] = object_args.as_slice() else { return None };
        if self.path(&object.callee).as_deref() != Some("Object")
            || !self.same_binding(source, output)
            || self.symbol(&keys.right) != boxed_id.symbol_id.get()
        { return None; }
        let ForStatementLeft::VariableDeclaration(declaration) = &keys.left else { return None };
        let [key] = declaration.declarations.as_slice() else { return None };
        let BindingPattern::BindingIdentifier(key) = &key.id else { return None };
        let key_symbol = key.symbol_id.get()?;
        let [Statement::IfStatement(own)] = Self::xhr_block(&keys.body)? else { return None };
        if own.alternate.is_some() { return None; }
        let mut test = strip(&own.test);
        // The tracer's boolean coercion is two logical negations, not !own.
        for _ in 0..2 {
            let Expression::UnaryExpression(not) = test else { return None };
            if not.operator != oxc_syntax::operator::UnaryOperator::LogicalNot { return None; }
            test = strip(&not.argument);
        }
        let Expression::CallExpression(check) = test else { return None };
        let check_args = args(check);
        let [checked, checked_key] = check_args.as_slice() else { return None };
        if self.path(&check.callee).as_deref() != Some("Object.prototype.hasOwnProperty.call")
            || !self.same_binding(checked, output) || self.symbol(checked_key) != Some(key_symbol)
        { return None; }
        let [Statement::ExpressionStatement(effect)] = Self::xhr_block(&own.consequent)? else { return None };
        let Expression::CallExpression(call) = strip(&effect.expression) else { return None };
        let (original, "call") = member(&call.callee)? else { return None };
        let call_args = args(call);
        let [receiver, name, value] = call_args.as_slice() else { return None };
        let (invocation, "thisObj") = member(receiver)? else { return None };
        let Expression::ComputedMemberExpression(value) = strip(value) else { return None };
        if !self.same_binding(original, header) || self.slot_index(invocation, callback) != Some(0)
            || self.symbol(name) != Some(key_symbol) || self.symbol(&value.expression) != Some(key_symbol)
            || !self.same_binding(&value.object, output)
        { return None; }
        Some(output)
    }

    fn xhr_plain_write<'e>(&self, statement: &'e Statement<'a>, symbol: SymbolId)
        -> Option<&'e Expression<'a>> {
        let Statement::ExpressionStatement(statement) = statement else { return None };
        let Expression::AssignmentExpression(write) = strip(&statement.expression) else { return None };
        let AssignmentTarget::AssignmentTargetIdentifier(id) = &write.left else { return None };
        (write.operator == AssignmentOperator::Assign
            && analysis::identifier_symbol(id, self.scoping) == Some(symbol)).then_some(&write.right)
    }

    fn xhr_lookup(&self, index: &VariableDeclaration<'a>, metadata: &VariableDeclaration<'a>,
        lookup: &ForStatement<'a>, tuple: &Statement<'a>, callback: usize, tuple_symbol: SymbolId) -> Option<()> {
        let [index] = index.declarations.as_slice() else { return None };
        let [metadata] = metadata.declarations.as_slice() else { return None };
        let BindingPattern::BindingIdentifier(index_id) = &index.id else { return None };
        let BindingPattern::BindingIdentifier(metadata_id) = &metadata.id else { return None };
        let index_symbol = index_id.symbol_id.get()?;
        let metadata_symbol = metadata_id.symbol_id.get()?;
        if numeric_index(index.init.as_ref()?) != Some(0) || metadata.init.is_some()
            || metadata_symbol != tuple_symbol { return None; }
        let [reset, Statement::IfStatement(bound), Statement::IfStatement(missing), selected,
            Statement::BreakStatement(done)] = Self::xhr_block(&lookup.body)? else { return None };
        if !Self::is_void(self.xhr_plain_write(reset, metadata_symbol)?) || done.label.is_some()
            || bound.alternate.is_some() || missing.alternate.is_some() { return None; }
        let Expression::BinaryExpression(limit) = strip(&bound.test) else { return None };
        let (keys, "length") = member(&limit.right)? else { return None };
        let keys_symbol = self.symbol(keys)?;
        if limit.operator != BinaryOperator::GreaterEqualThan || self.symbol(&limit.left) != Some(index_symbol) { return None; }
        let [empty, Statement::BreakStatement(stop)] = Self::xhr_block(&bound.consequent)? else { return None };
        if !is_null(self.xhr_plain_write(empty, metadata_symbol)?) || stop.label.is_some() { return None; }
        let Expression::UnaryExpression(not) = strip(&missing.test) else { return None };
        let Expression::BinaryExpression(contains) = strip(&not.argument) else { return None };
        let Expression::ComputedMemberExpression(key) = strip(&contains.left) else { return None };
        let (receiver, "thisObj") = member(&contains.right)? else { return None };
        if not.operator != oxc_syntax::operator::UnaryOperator::LogicalNot
            || contains.operator != BinaryOperator::In || self.symbol(&key.object) != Some(keys_symbol)
            || self.symbol(&key.expression) != Some(index_symbol) || self.slot_index(receiver, callback) != Some(0)
        { return None; }
        let [Statement::ExpressionStatement(increment), Statement::ContinueStatement(next)] =
            Self::xhr_block(&missing.consequent)? else { return None };
        let Expression::UpdateExpression(increment) = strip(&increment.expression) else { return None };
        let SimpleAssignmentTarget::AssignmentTargetIdentifier(counter) = &increment.argument else { return None };
        if increment.operator != oxc_syntax::operator::UpdateOperator::Increment
            || analysis::identifier_symbol(counter, self.scoping) != Some(index_symbol) || next.label.is_some()
        { return None; }
        let Expression::ComputedMemberExpression(selected) = strip(self.xhr_plain_write(selected, metadata_symbol)?) else { return None };
        let (selected_receiver, "thisObj") = member(&selected.object)? else { return None };
        let Expression::ComputedMemberExpression(selected_key) = strip(&selected.expression) else { return None };
        if self.slot_index(selected_receiver, callback) != Some(0)
            || self.symbol(&selected_key.object) != Some(keys_symbol)
            || self.symbol(&selected_key.expression) != Some(index_symbol) { return None; }
        let Expression::ArrayExpression(values) = strip(self.xhr_plain_write(tuple, metadata_symbol)?) else { return None };
        let values = &values.elements;
        if values.len() != 3 || !values.iter().all(|value| value.as_expression().and_then(|value| self.symbol(value)) == Some(metadata_symbol)) { return None; }
        Some(())
    }

    /// A merge may conditionally skip entries but cannot exit the callback
    /// before the subsequent emission edge. Refuse opaque control flow and
    /// constant/dormant donor guards within this bounded region.
    fn xhr_merge_falls_through(&self, statement: &Statement<'a>) -> bool {
        match statement {
            Statement::VariableDeclaration(_) | Statement::ExpressionStatement(_) => true,
            Statement::BlockStatement(block) => block.body.iter().all(|s| self.xhr_merge_falls_through(s)),
            Statement::ForInStatement(keys) => self.symbol(&keys.right).is_some()
                && self.xhr_merge_falls_through(&keys.body),
            Statement::IfStatement(guard) => {
                let present = self.xhr_present(&guard.test).is_some();
                let mut test = strip(&guard.test);
                let mut coercions = 0;
                while let Expression::UnaryExpression(not) = test {
                    if not.operator != oxc_syntax::operator::UnaryOperator::LogicalNot { return false; }
                    coercions += 1;
                    test = strip(&not.argument);
                }
                let owns = coercions == 2 && matches!(test, Expression::CallExpression(call)
                    if self.path(&call.callee).as_deref() == Some("Object.prototype.hasOwnProperty.call")
                        && args(call).len() == 2);
                (present || owns) && guard.alternate.is_none()
                    && self.xhr_merge_falls_through(&guard.consequent)
            }
            _ => false,
        }
    }

    /// A bounded control-flow grammar for the split variant. All edges used
    /// as evidence are direct children in this grammar; no arbitrary wrapper,
    /// nested function, earlier return, or later donor can supply an edge.
    fn ordered_split_xhr_callback(&self, callback: usize, header: &Expression<'a>) -> Option<()> {
        if !self.xhr_sync_parameters(callback, 2) { return None; }
        let [Statement::TryStatement(protected)] = self.body_statements(callback) else { return None };
        if protected.handler.is_none() || protected.finalizer.is_some() { return None; }
        let [Statement::IfStatement(bypass)] = protected.block.body.as_slice() else { return None };
        let Expression::LogicalExpression(test) = strip(&bypass.test) else { return None };
        let prior = Self::xhr_nonnull(&test.left)?;
        let (prior_object, "bypassResult") = member(Self::xhr_nonnull(&test.right)?)? else { return None };
        if test.operator != LogicalOperator::And || self.slot_index(prior, callback) != Some(1)
            || self.slot_index(prior_object, callback) != Some(1)
        { return None; }
        let [ret] = Self::xhr_block(&bypass.consequent)? else { return None };
        if !self.xhr_result_return(ret, callback) { return None; }
        let [Statement::IfStatement(receiver), ret] = Self::xhr_block(bypass.alternate.as_ref()?)? else { return None };
        let (invocation, "thisObj") = member(Self::xhr_nonnull(&receiver.test)?)? else { return None };
        if self.slot_index(invocation, callback) != Some(0) || receiver.alternate.is_some()
            || !self.xhr_result_return(ret, callback)
        { return None; }
        // Lookup and tuple packaging precede the metadata predicate. In
        // particular a return inserted before either header path is refused.
        let [Statement::VariableDeclaration(index), Statement::VariableDeclaration(meta),
            Statement::ForStatement(lookup), tuple,
            Statement::IfStatement(metadata)] = Self::xhr_block(&receiver.consequent)? else { return None };
        if lookup.init.is_some() || lookup.test.is_some() || lookup.update.is_some()
            || metadata.alternate.is_some()
        { return None; }
        let Expression::LogicalExpression(metadata_test) = strip(&metadata.test) else { return None };
        let null_test = Self::xhr_nonnull(&metadata_test.left)?;
        let Expression::CallExpression(predicate) = strip(&metadata_test.right) else { return None };
        let predicate_args = args(predicate);
        let [argument] = predicate_args.as_slice() else { return None };
        let (null_tuple, 0) = self.array_slot(null_test)? else { return None };
        let (arg_tuple, 1) = self.array_slot(argument)? else { return None };
        if metadata_test.operator != LogicalOperator::And || null_tuple != arg_tuple { return None; }
        self.xhr_lookup(index, meta, lookup, tuple, callback, null_tuple)?;
        let [Statement::IfStatement(mode)] = Self::xhr_block(&metadata.consequent)? else { return None };
        // Preserve the body-presence guard after the method/flag predicates.
        let Expression::LogicalExpression(body_guard) = strip(&mode.test) else { return None };
        let Expression::LogicalExpression(length_guard) = strip(&body_guard.left) else { return None };
        let body = Self::xhr_nonnull(&body_guard.right)?;
        let Expression::ComputedMemberExpression(body) = strip(body) else { return None };
        let (args_object, "args") = member(&body.object)? else { return None };
        if body_guard.operator != LogicalOperator::And || length_guard.operator != LogicalOperator::And
            || self.slot_index(args_object, callback) != Some(0) || numeric_index(&body.expression) != Some(0)
        { return None; }
        let Expression::LogicalExpression(method_guard) = strip(&length_guard.left) else { return None };
        let Expression::BinaryExpression(length) = strip(&length_guard.right) else { return None };
        let Expression::CallExpression(includes) = strip(&method_guard.right) else { return None };
        let (methods, "includes") = member(&includes.callee)? else { return None };
        let include_args = args(includes);
        let [upper] = include_args.as_slice() else { return None };
        let Expression::CallExpression(upper) = strip(upper) else { return None };
        let (method, "toUpperCase") = member(&upper.callee)? else { return None };
        let (length_args, "length") = member(&length.left)? else { return None };
        let (length_invocation, "args") = member(length_args)? else { return None };
        if method_guard.operator != LogicalOperator::And
            || self.symbol(&method_guard.left).is_none() || self.symbol(methods).is_none()
            || self.symbol(method).is_none() || !args(upper).is_empty()
            || length.operator != BinaryOperator::GreaterThan || numeric_index(&length.right) != Some(0)
            || self.slot_index(length_invocation, callback) != Some(0)
        { return None; }
        let false_path = Self::xhr_block(mode.alternate.as_ref()?)?;
        let [options @ Statement::VariableDeclaration(_), output @ Statement::VariableDeclaration(_),
            merge @ Statement::IfStatement(_), emission, ret] = false_path else { return None };
        if !self.xhr_result_return(ret, callback) || !self.xhr_merge_falls_through(merge) { return None; }
        let false_pipeline = self.xhr_gate(self.xhr_options_pipeline(options, output, merge, emission, callback, header), "callback.false_options_pipeline")?;
        let true_path = Self::xhr_block(&mode.consequent)?;
        let [prefix @ .., Statement::IfStatement(types), ret] = true_path else { return None };
        // Before dispatch, allow only straight-line declarations/effects.
        // A return/throw/extra conditional cannot bypass the dispatch edge.
        if !self.xhr_result_return(ret, callback) || !prefix.iter().all(|statement|
            matches!(statement, Statement::VariableDeclaration(_) | Statement::ExpressionStatement(_)))
        { return None; }
        let Expression::BinaryExpression(array_buffer) = strip(&types.test) else { return None };
        if array_buffer.operator != BinaryOperator::Instanceof
            || self.path(&array_buffer.right).as_deref() != Some("ArrayBuffer")
        { return None; }
        let Statement::IfStatement(view) = types.alternate.as_ref()? else { return None };
        let Expression::CallExpression(view_test) = strip(&view.test) else { return None };
        if self.path(&view_test.callee).as_deref() != Some("ArrayBuffer.isView") { return None; }
        let Statement::IfStatement(blob) = view.alternate.as_ref()? else { return None };
        let Expression::BinaryExpression(blob_test) = strip(&blob.test) else { return None };
        if blob_test.operator != BinaryOperator::Instanceof || self.path(&blob_test.right).as_deref() != Some("Blob")
        { return None; }
        let [options @ Statement::VariableDeclaration(_), output @ Statement::ExpressionStatement(_),
            merge @ Statement::IfStatement(_), emission] = Self::xhr_block(blob.alternate.as_ref()?)? else { return None };
        if !self.xhr_merge_falls_through(merge) { return None; }
        let true_pipeline = self.xhr_gate(self.xhr_options_pipeline(options, output, merge, emission, callback, header), "callback.true_options_pipeline")?;
        if true_pipeline != false_pipeline { return None; }
        Some(())
    }

    /// Role-local transport proof for the lowered, two-read xhrSend protocol.
    /// Do not merge its values in single_value: the guard and the selected
    /// property read are separate observable operations (including getters).
    fn split_xhr_send_registration(
        &self,
        factory: usize,
        init: usize,
        teardown: usize,
    ) -> Option<usize> {
        for call in &self.own[init].calls {
            if let Some(callback) = self.split_xhr_send_call(factory, init, teardown, call) {
                return Some(callback);
            }
        }
        None
    }

    fn split_xhr_send_call(
        &self,
        factory: usize,
        init: usize,
        teardown: usize,
        registration: &CallExpression<'a>,
    ) -> Option<usize> {
        let (hook, "register") = member(&registration.callee)? else { return None };
        let (bundle, "onBeforeInvoke") = member(hook)? else { return None };
        let bundle = self.symbol(bundle)?;
        let registration_args = args(registration);
        let [callback] = registration_args.as_slice() else { return None };
        let callback = self.resolve_fn(callback)?;
        if !self.xhr_sync_parameters(factory, 1) || !self.xhr_sync_parameters(init, 0)
            || !self.xhr_sync_parameters(teardown, 0)
            || self.fns[init].parent != Some(factory)
            || self.fns[teardown].parent != Some(factory)
            || self.fns[callback].parent != Some(init)
            || self.scoping.symbol_declarations(bundle).count() != 1
        {
            return None;
        }
        // Registration is the direct statement of this bundle's own non-null
        // hook arm, not a dormant nested donor or a different hook phase.
        let statement = self.enclosing_statement(self.call_node(registration)?)?;
        let block = self.nodes.parent_id(statement);
        let AstKind::BlockStatement(_) = self.nodes.kind(block) else { return None };
        let guard_node = self.nodes.parent_id(block);
        let AstKind::IfStatement(hook_guard) = self.nodes.kind(guard_node) else { return None };
        if hook_guard.consequent.span() != self.nodes.kind(block).span() {
            return None;
        }
        let Expression::BinaryExpression(test) = strip(&hook_guard.test) else { return None };
        let (guard_bundle, "onBeforeInvoke") = member(&test.left)? else { return None };
        if test.operator != BinaryOperator::Inequality || !is_null(&test.right)
            || self.symbol(guard_bundle) != Some(bundle)
        {
            return None;
        }
        // The recovered init puts the two registration arms in the alternate
        // of its early null-header exit. Require that exact enclosing shape;
        // do not traverse arbitrary conditionals looking for donor evidence.
        let init_block = self.nodes.parent_id(guard_node);
        let AstKind::BlockStatement(_) = self.nodes.kind(init_block) else { return None };
        let exit_node = self.nodes.parent_id(init_block);
        let AstKind::IfStatement(exit) = self.nodes.kind(exit_node) else { return None };
        if exit.alternate.as_ref()?.span() != self.nodes.kind(init_block).span()
            || !self.statement_is_direct_body_child(exit_node, self.function_body_span(init)?)
        {
            return None;
        }
        let header = Self::null_guard_subject(&exit.test)?;
        let Statement::BlockStatement(exit_body) = &exit.consequent else { return None };
        let [Statement::ReturnStatement(ret)] = exit_body.body.as_slice() else { return None };
        if !ret.argument.as_ref().is_some_and(Self::is_void) {
            return None;
        }
        self.xhr_gate(self.ordered_split_xhr_callback(callback, header), "callback.ordered_pipeline")?;
        // A local role check, using only the actual callback's owned body.
        // The captured header call retains the invocation receiver, and every
        // normal/catch return preserves incoming hook result slot one.
        let header_calls: Vec<_> = self.own[callback].calls.iter().filter(|call|
            member(&call.callee).is_some_and(|(callee, _)| self.same_binding(callee, header))
        ).collect();
        if !self.slot_member(callback, "thisObj") || !self.slot_member(callback, "args")
            || self.own[callback].returns.is_empty()
            || !self.own[callback].returns.iter().all(|value| self.slot_index(value, callback) == Some(1))
            || !self.own[callback].members.iter().any(|(receiver, name)|
                *name == "bypassResult" && self.slot_index(receiver, callback) == Some(1))
            || header_calls.is_empty()
            || !header_calls.iter().all(|call| {
                let Some((callee, "call")) = member(&call.callee) else { return false };
                let arguments = args(call);
                matches!(arguments.as_slice(), [receiver, _, _]
                    if self.same_binding(callee, header)
                    && member(receiver).is_some_and(|(slot, name)| name == "thisObj"
                        && self.slot_index(slot, callback) == Some(0)))
            })
        {
            return None;
        }
        let declaration = self.scoping.symbol_declaration(bundle);
        if self.owner[declaration.index()] != Some(factory)
            || !matches!(self.nodes.kind(declaration), AstKind::VariableDeclarator(d) if d.init.is_none())
        {
            return None;
        }
        let writes = self.assigns.get(&bundle)?;
        if writes.len() != 2
            || self.values.get(&bundle)?.len() != 2
            || self.scoping.get_resolved_references(bundle).filter(|r| r.is_write()).count() != 2
        {
            return None;
        }
        // Both arms must be one plain assignment: no return, throw, extra
        // effect, conditional write, or missing branch can reach the join.
        let arm = |statement: &'a Statement<'a>| -> Option<&'a AssignmentExpression<'a>> {
            let Statement::BlockStatement(block) = statement else { return None };
            let [Statement::ExpressionStatement(statement)] = block.body.as_slice() else { return None };
            let Expression::AssignmentExpression(write) = strip(&statement.expression) else { return None };
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &write.left else { return None };
            (write.operator == AssignmentOperator::Assign
                && analysis::identifier_symbol(id, self.scoping) == Some(bundle)).then_some(write)
        };
        let (split, selected, captured) = self.body_statements(factory).iter().find_map(|statement| {
            let Statement::IfStatement(split) = statement else { return None };
            Some((split, arm(&split.consequent)?, arm(split.alternate.as_ref()?)?))
        })?;
        if !writes.iter().all(|w| w.span == selected.span || w.span == captured.span) {
            return None;
        }
        let Expression::BinaryExpression(guard) = strip(&split.test) else { return None };
        if guard.operator != BinaryOperator::Inequality || !is_null(&guard.right) {
            return None;
        }
        // Match both reads by binding and member path, without evaluating or
        // substituting either read. Only settings slot zero of this factory
        // can supply the instrumented alternative.
        let settings_of = |value: &Expression<'a>| -> Option<SymbolId> {
            let (instrumented, "xhrSend") = member(value)? else { return None };
            let (settings, "instrumented") = member(instrumented)? else { return None };
            (self.slot_index(settings, factory) == Some(0)).then(|| self.symbol(settings)).flatten()
        };
        let settings = settings_of(&guard.left)?;
        if self.scoping.get_resolved_references(settings).any(|reference| reference.is_write()) {
            return None;
        }
        if settings_of(&selected.right) != Some(settings) {
            return None;
        }
        let Expression::CallExpression(capture) = strip(&captured.right) else { return None };
        let interceptor = self.xhr_gate(self.xhr_native_capture_callable(&capture.callee), "send.native_callable")?;
        if !self.xhr_native_methods.contains(&interceptor) { return None; }
        let capture_args = args(capture);
        let [prototype, method] = capture_args.as_slice() else { return None };
        if string_value(method) != Some("send") {
            return None;
        }
        let prototype_symbol = self.symbol(prototype)?;
        let (constructor, "prototype") = member(self.single_value(prototype_symbol)?)? else { return None };
        let constructor_symbol = self.symbol(constructor)?;
        let (global, "XMLHttpRequest") = member(self.single_value(constructor_symbol)?)? else { return None };
        let (native_settings, "global") = member(global)? else { return None };
        if self.symbol(native_settings) != Some(settings) {
            return None;
        }
        self.xhr_gate(self.xhr_original_header(factory, init, callback, header, settings, prototype_symbol, interceptor), "header.original_provenance")?;
        // Capture receiver definitions must precede the split in this body.
        for symbol in [prototype_symbol, constructor_symbol] {
            let value = self.single_value(symbol)?;
            let statement = self.enclosing_statement(self.expression_node(value)?)?;
            let body = self.function_body_span(factory)?;
            if !self.statement_is_direct_body_child(statement, body)
                || self.nodes.kind(statement).span().end > split.span.start
            {
                return None;
            }
        }
        // The actual init/teardown pair is returned after the join. Neither
        // lifecycle function may also be called/escaped before that return.
        for function in [init, teardown] {
            let FnNode::Function(func) = self.fns[function].func else { return None };
            let symbol = func.id.as_ref()?.symbol_id.get()?;
            for reference in self.scoping.get_resolved_references(symbol) {
                let node = reference.node_id();
                if reference.is_write() || self.owner[node.index()] != Some(factory) {
                    return None;
                }
                let AstKind::ObjectProperty(_) = self.nodes.kind(self.nodes.parent_id(node)) else { return None };
                let object = self.nodes.parent_id(self.nodes.parent_id(node));
                let AstKind::ObjectExpression(_) = self.nodes.kind(object) else { return None };
                let parent = self.nodes.parent_id(object);
                if !matches!(self.nodes.kind(parent), AstKind::ReturnStatement(_))
                    || self.nodes.kind(parent).span().start <= split.span.end
                {
                    return None;
                }
            }
        }
        // Account for every bundle use. No argument escape, alias, computed
        // member, foreign function read, or member write is transport proof.
        for reference in self.scoping.get_resolved_references(bundle) {
            if reference.is_write() { continue; }
            let node = reference.node_id();
            let owner = self.owner[node.index()]?;
            let parent = self.nodes.parent_id(node);
            match self.nodes.kind(parent) {
                AstKind::BinaryExpression(binary)
                    if [factory, init, teardown].contains(&owner)
                        && binary.operator == BinaryOperator::Equality
                        && is_null(&binary.right) => {}
                AstKind::StaticMemberExpression(found)
                    if [init, teardown].contains(&owner)
                        && found.property.name == "onBeforeInvoke" => {
                    let use_node = self.nodes.parent_id(parent);
                    match self.nodes.kind(use_node) {
                        AstKind::BinaryExpression(binary)
                            if binary.operator == BinaryOperator::Inequality && is_null(&binary.right) => {}
                        AstKind::StaticMemberExpression(method)
                            if owner == init && method.property.name == "register"
                                && method.span == registration.callee.span() => {}
                        AstKind::AssignmentExpression(write)
                            if owner == teardown && write.right.span() == found.span => {}
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
        Some(callback)
    }

    fn noop_guard_kind(&self, pair: &LifecyclePair) -> Option<NoopGuard> {
        let test = self.enclosing_if_test(pair.node)?;
        if let Some(subject) = Self::null_guard_subject(test) {
            if let Some((_, name)) = member(subject) {
                return match name {
                    "XMLHttpRequest" => Some(NoopGuard::MissingConstructor),
                    "prototype" => Some(NoopGuard::MissingPrototype),
                    _ => None,
                };
            }
            let symbol = self.symbol(subject)?;
            let value = self.single_value(symbol)?;
            let (_, name) = member(value)?;
            return match name {
                "XMLHttpRequest" => Some(NoopGuard::MissingConstructor),
                "prototype" => Some(NoopGuard::MissingPrototype),
                _ => None,
            };
        }
        matches!(strip(test), Expression::Identifier(_))
            .then_some(NoopGuard::AlreadyInstrumented)
    }

    // -----------------------------------------------------------------------
    // Telemetry XHR carrier
    // -----------------------------------------------------------------------

    fn telemetry_sender(&self) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.fns.len())
            .filter(|index| {
                let own = &self.own[*index];
                let creates_xhr = own.news.iter().any(|new| {
                    member(&new.callee).is_some_and(|(_, name)| name == "XMLHttpRequest")
                });
                let opens_get = own.calls.iter().any(|call| {
                    self.method_name(call).as_deref() == Some("open")
                        && matches!(self.call_args(call).as_slice(), [method, _, asynchronous]
                            if string_value(method) == Some("GET") && is_boolean(asynchronous, true))
                });
                let sends = own
                    .calls
                    .iter()
                    .any(|call| self.method_name(call).as_deref() == Some("send"));
                let handlers = ["onerror", "onloadend", "onload"].iter().all(|name| {
                    own.assignments.iter().any(|assignment| {
                        target_member_name(&assignment.left) == Some(*name)
                            && self.resolve_fn(&assignment.right).is_some()
                    })
                });
                creates_xhr && opens_get && sends && handlers
            })
            .collect();
        match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn claim_telemetry_family(&self, out: &mut Out) {
        let Some(sender) = self.telemetry_sender() else {
            return;
        };
        out.function(
            self,
            SEND_TELEMETRY_REQUEST,
            sender,
            "opens and sends an XHR GET, installs the onerror/onloadend/onload handlers and emits the post-send signal bundle",
        );
        for assignment in &self.own[sender].assignments {
            let Some(name) = target_member_name(&assignment.left) else {
                continue;
            };
            let Some(handler) = self.resolve_fn(&assignment.right) else {
                continue;
            };
            match name {
                "onerror" if self.is_network_error_handler(handler) => out.function(
                    self,
                    MARK_TELEMETRY_NETWORK_ERROR,
                    handler,
                    "XHR onerror handler: records the status and load-end time and sets hasNetworkError",
                ),
                "onloadend" if self.is_response_encoder(handler) => out.function(
                    self,
                    ENCODE_TELEMETRY_RESPONSE,
                    handler,
                    "XHR onloadend handler: builds the random-prefix body, hashes the request fields and completes the emitted signal bundle",
                ),
                "onload" if self.is_response_text_capture(handler) => out.function(
                    self,
                    CAPTURE_TELEMETRY_RESPONSE_TEXT,
                    handler,
                    "XHR onload handler: copies responseText into the request record",
                ),
                _ => {}
            }
        }
    }

    fn is_network_error_handler(&self, function: usize) -> bool {
        self.own[function].written_keys.contains("hasNetworkError")
    }

    fn is_response_encoder(&self, function: usize) -> bool {
        let own = &self.own[function];
        own.calls.iter().any(|call| self.method_name(call).as_deref() == Some("complete"))
            && own.calls.iter().any(|call| self.method_name(call).as_deref() == Some("next"))
    }

    fn is_response_text_capture(&self, function: usize) -> bool {
        self.own[function].written_keys.contains("xhrResponse")
            && self.own[function].members.iter().any(|(_, name)| *name == "responseText")
    }

    // -----------------------------------------------------------------------
    // MAIN carrier aliases and marker arrays
    // -----------------------------------------------------------------------

    fn request_matcher_wrapper(&self) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.fns.len())
            .filter(|index| {
                let Some(value) = self.single_return(*index) else {
                    return false;
                };
                let Expression::CallExpression(call) = strip(value) else {
                    return false;
                };
                let arguments = args(call);
                let [request, _, mode, _] = arguments.as_slice() else {
                    return false;
                };
                let Some(matcher) = self.resolve_fn(&call.callee) else {
                    return false;
                };
                self.slot_index(request, *index) == Some(0)
                    && matches!(strip(mode), Expression::BooleanLiteral(_))
                    && self.subtree_has_member(matcher, "url")
                    && self.subtree_has_member(matcher, "method")
                    && self.subtree_has_member(matcher, "toLowerCase")
                    && self.subtree_has_member(matcher, "hostname")
            })
            .collect();
        match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn envelope_builder(&self) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.fns.len())
            .filter(|index| {
                let own = &self.own[*index];
                let reads = |name: &str| {
                    own.members.iter().any(|(_, member)| *member == name)
                };
                ["signalFilter", "encode", "notifyLastChance", "descriptor"]
                    .iter()
                    .all(|name| reads(name))
                    && own.returns.iter().any(|value| {
                        self.symbol(value)
                            .and_then(|symbol| self.single_value(symbol))
                            .is_some_and(|value| self.is_null_prototype_object(value))
                    })
            })
            .collect();
        match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn prefixed_key_length_formatter(&self) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.fns.len())
            .filter(|index| {
                let Some(value) = self.single_return(*index) else {
                    return false;
                };
                let Expression::CallExpression(join) = strip(value) else {
                    return false;
                };
                let Some((mapped, "join")) = self.method(join) else {
                    return false;
                };
                let Expression::CallExpression(map) = strip(mapped) else {
                    return false;
                };
                let Some((keys, "map")) = self.method(map) else {
                    return false;
                };
                let Expression::CallExpression(keys_call) = strip(keys) else {
                    return false;
                };
                let reads_keys = self.path(&keys_call.callee).as_deref() == Some("Object.keys")
                    && self.call_args(keys_call)
                        .first()
                        .is_some_and(|first| self.slot_index(first, *index) == Some(0));
                if !reads_keys {
                    return false;
                }
                let Some(formatter) =
                    self.call_args(map).first().and_then(|first| self.resolve_fn(first))
                else {
                    return false;
                };
                let Some(formatted) = self.single_return(formatter) else {
                    return false;
                };
                let mut parts = Vec::new();
                additions(formatted, &mut parts);
                parts.iter().any(|part| self.slot_index(part, formatter) == Some(0))
                    && parts.iter().any(|part| {
                        member(part).is_some_and(|(object, name)| {
                            name == "length"
                                && matches!(strip(object), Expression::ComputedMemberExpression(element)
                                    if self.slot_index(&element.object, *index) == Some(0)
                                        && self.slot_index(&element.expression, formatter) == Some(0))
                        })
                    })
            })
            .collect();
        match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn header_chunker(&self) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.fns.len())
            .filter(|index| {
                let own = &self.own[*index];
                let returns = own
                    .returns
                    .iter()
                    .find_map(|value| self.symbol(value))
                    .filter(|symbol| {
                        self.single_value(*symbol)
                            .is_some_and(|value| self.is_null_prototype_object(value))
                    });
                let Some(result) = returns else {
                    return false;
                };
                let (mut whole, mut first, mut rest) = (false, false, false);
                for assignment in &own.assignments {
                    let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
                        continue;
                    };
                    if self.symbol(&target.object) != Some(result) {
                        continue;
                    }
                    let sliced = matches!(strip(&assignment.right), Expression::CallExpression(call)
                        if self.method(call).is_some_and(|(receiver, name)| {
                            name == "slice"
                                && self.slot_index(receiver, *index) == Some(1)
                                && self.call_args(call).len() == 2
                        }));
                    if self.slot_index(&target.expression, *index) == Some(0) {
                        let mut parts = Vec::new();
                        additions(&assignment.right, &mut parts);
                        whole |= matches!(parts.as_slice(), [text, value]
                            if string_value(text) == Some("") && self.slot_index(value, *index) == Some(1));
                        first |= sliced;
                    } else if let Expression::BinaryExpression(key) = strip(&target.expression) {
                        rest |= sliced
                            && key.operator == BinaryOperator::Addition
                            && self.slot_index(&key.left, *index) == Some(0);
                    }
                }
                whole && first && rest && self.has_header_writing_caller(*index)
            })
            .collect();
        match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    fn has_header_writing_caller(&self, function: usize) -> bool {
        self.fns.iter().enumerate().any(|(index, _)| {
            let calls_function =
                self.own[index].calls.iter().any(|call| self.resolve_fn(&call.callee) == Some(function));
            calls_function
                && self.own[index].calls.iter().any(|call| {
                    self.method(call).is_some_and(|(receiver, name)| {
                        name == "set"
                            && member(receiver).is_some_and(|(_, field)| field == "headers")
                    })
                })
        })
    }

    fn claim_carrier_aliases(&self, out: &mut Out) {
        let aliases = [
            (
                self.request_matcher_wrapper(),
                REQUEST_MATCHER_ALIAS,
                "single main-scope alias of the request-matcher wrapper (returns the proven matcher applied to its request slot)",
            ),
            (
                self.envelope_builder(),
                ENCODING_OPTIONS_FACTORY_ALIAS,
                "single main-scope alias of the envelope builder (reads signalFilter, encode, notifyLastChance and descriptor and returns a null-prototype envelope)",
            ),
            (
                self.prefixed_key_length_formatter(),
                PREFIXED_KEY_LENGTH_FORMATTER_ALIAS,
                "single main-scope alias of the prefixed-key length formatter (space-joins one key/length entry per key)",
            ),
            (
                self.header_chunker(),
                HEADER_CHUNKER_ALIAS,
                "single main-scope alias of the header-value chunker (name/name0/name1 slice map written with headers.set by a caller)",
            ),
        ];
        for (target, role, evidence) in aliases {
            let Some(target) = target else {
                continue;
            };
            if let Some(holder) = self.values_holder(target) {
                out.binding(self, role, holder, evidence);
}
        }
    }

    fn claim_marker_arrays(&self, out: &mut Out) {
        let fetch_init = out
            .role_fn(INIT_FETCH_INSTRUMENTATION);
        let fetch_teardown = out.role_fn(TEARDOWN_FETCH_INSTRUMENTATION);
        let xhr_init = out.role_fn(INIT_XHR_INSTRUMENTATION);
        let xhr_teardown = out.role_fn(TEARDOWN_XHR_INSTRUMENTATION);
        let xhr_open = out.role_fn(CAPTURE_XHR_OPEN_METADATA);
        let xhr_send = out.role_fn(INJECT_REQUEST_HEADERS_BEFORE_SEND);
        let signal_init = out.role_fn(DISPATCH_SIGNAL_CONFIG_EVENTS);
        let signal_teardown = out.role_fn(DISPATCH_SIGNAL_TEARDOWN_EVENTS);

        let mut markers = Vec::new();
        let mut events = Vec::new();
        let mut state = Vec::new();
        for symbol in self.values.keys() {
            if !self.array_of_strings(*symbol) {
                continue;
            }
            let markers_written = self.reads_in(*symbol, fetch_init) || self.reads_in(*symbol, xhr_init);
            let markers_deleted =
                self.reads_in(*symbol, fetch_teardown) || self.reads_in(*symbol, xhr_teardown);
            let dispatched =
                self.reads_in(*symbol, signal_init) || self.reads_in(*symbol, signal_teardown);
            let request_state = self.reads_in(*symbol, xhr_open) || self.reads_in(*symbol, xhr_send);
            if markers_written && markers_deleted && !dispatched {
                markers.push(*symbol);
            } else if dispatched && !markers_written {
                events.push(*symbol);
            } else if request_state && !markers_written && !dispatched {
                state.push(*symbol);
            }
        }
        for (role, candidates, evidence) in [
            (
                INSTRUMENTATION_MARKER_PROPERTY_NAMES,
                markers,
                "array of randomized property-name strings; the instrumentation init writes its first element as the non-enumerable marker and the teardown deletes the marker list",
            ),
            (
                SIGNAL_EVENT_NAMES,
                events,
                "array of randomized event-name strings; the CustomEvent signal hooks dispatch one event per element",
            ),
            (
                INTERNAL_STATE_PROPERTY_NAMES,
                state,
                "array of randomized property-name strings; the XHR hooks define and then look up per-request state under it",
            ),
        ] {
            if let [only] = candidates.as_slice() {
                out.binding(self, role, *only, evidence);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum NoopGuard {
    MissingConstructor,
    MissingPrototype,
    AlreadyInstrumented,
}

fn additions<'e, 'a>(expression: &'e Expression<'a>, out: &mut Vec<&'e Expression<'a>>) {
    match strip(expression) {
        Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
            additions(&binary.left, out);
            additions(&binary.right, out);
        }
        other => out.push(other),
    }
}

#[cfg(test)]
std::thread_local! {
    static HEADER_ORIGIN_FIRST_REFUSAL: std::cell::RefCell<Option<&'static str>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
mod header_origin_diagnostic {
    use super::*;

    // The full witness is shared with the integration target and remains sealed
    // independently. No generated name or source offset selects the callback.
    const SOURCE: &str = include_str!("../../../../tests/fixtures/inject_request_headers_variants/branch-assigned-inline-carrier.js");

    #[test]
    fn published_chunker_strict_callable_refusal() {
        let allocator = oxc_allocator::Allocator::default();
        let program = crate::analysis::parse_program(&allocator, SOURCE).expect("full witness parses");
        let (scoping, nodes) = crate::analysis::analyze(&program);
        let model = crate::signals::extract::bindings::Model::new(
            SOURCE, &program, &scoping, &nodes,
            crate::signals::schema::ProgramPropertiesLimits::default(),
        ).expect("full witness builds valid Model");
        assert!(!model.all_sites().is_empty());
        let ctx = FinderCtx { source: SOURCE, program: &program, scoping: &scoping, nodes: &nodes };
        let slots = ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes);
        let mut ix = Ix::new(&ctx, &slots);
        ix.index_xhr_source_proofs(&ctx);
        let factory = ix.xhr_factory().expect("unique actual XHR factory");
        let pairs = ix.lifecycle_pairs(factory);
        let pair = pairs.iter().find(|pair| !ix.body_is_noop(pair.init) && !ix.body_is_noop(pair.teardown))
            .expect("actual non-noop lifecycle pair");
        let registrations: Vec<_> = ix.own[pair.init].calls.iter().copied().filter(|call| {
            let Some((hook, "register")) = member(&call.callee) else { return false };
            let Some((bundle, "onBeforeInvoke")) = member(hook) else { return false };
            ix.symbol(bundle).and_then(|symbol| ix.assigns.get(&symbol)).is_some_and(|writes| writes.len() == 2)
        }).collect();
        let [registration] = registrations.as_slice() else { panic!("unique split registration required") };
        let registration_args = args(registration);
        let [callback] = registration_args.as_slice() else { panic!("one actual callback") };
        let callback = ix.resolve_fn(callback).expect("actual callback binding");
        let chunk_calls: Vec<_> = ix.own[callback].calls.iter().copied().filter(|call|
            ix.resolve_fn(&call.callee).is_some_and(|helper| ix.xhr_chunker_helper(helper).is_some())
        ).collect();
        assert!(!chunk_calls.is_empty(), "actual owned chunk calls");
        let helpers: BTreeSet<_> = chunk_calls.iter().filter_map(|call| ix.resolve_fn(&call.callee)).collect();
        assert_eq!(helpers.len(), 1, "both paths use the same actual helper");
        HEADER_ORIGIN_FIRST_REFUSAL.with(|first| *first.borrow_mut() = None);
        // Preserve the publication diagnostic on its strict
        // resolver. The new published-chunker route does not weaken it.
        let result = ix.xhr_gate(ix.xhr_stable_callable(&chunk_calls[0].callee, 8),
            "published_chunker.strict_callable");
        let first = HEADER_ORIGIN_FIRST_REFUSAL.with(|first| *first.borrow());
        eprintln!("[published chunker] STRICT_FIRST_REJECTION={first:?}; strict_callable={}", result.is_some());
        // This strict-resolver check remains independent of
        // the retained actual positive integration assertions.
        assert_eq!(first, Some("callable.read.object_property_publication"),
            "source hypothesis differed; use the FIRST_REJECTION receipt, do not relax a gate speculatively");
        assert!(result.is_none(), "strict native/header resolver must continue to refuse publication");
    }
    #[test]
    fn split_registration_first_refusal() {
        let allocator = oxc_allocator::Allocator::default();
        let program = crate::analysis::parse_program(&allocator, SOURCE).expect("full witness parses");
        let (scoping, nodes) = crate::analysis::analyze(&program);
        let model = crate::signals::extract::bindings::Model::new(
            SOURCE, &program, &scoping, &nodes,
            crate::signals::schema::ProgramPropertiesLimits::default(),
        ).expect("full witness builds valid Model");
        assert!(!model.all_sites().is_empty());
        let ctx = FinderCtx { source: SOURCE, program: &program, scoping: &scoping, nodes: &nodes };
        let slots = ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes);
        let mut ix = Ix::new(&ctx, &slots);
        ix.index_xhr_source_proofs(&ctx);
        let factory = ix.xhr_factory().expect("unique actual XHR factory");
        let pairs = ix.lifecycle_pairs(factory);
        let pair = pairs.iter().find(|pair| !ix.body_is_noop(pair.init) && !ix.body_is_noop(pair.teardown))
            .expect("actual non-noop lifecycle pair");
        let registrations: Vec<_> = ix.own[pair.init].calls.iter().copied().filter(|call| {
            let Some((hook, "register")) = member(&call.callee) else { return false };
            let Some((bundle, "onBeforeInvoke")) = member(hook) else { return false };
            ix.symbol(bundle).and_then(|symbol| ix.assigns.get(&symbol)).is_some_and(|writes| writes.len() == 2)
        }).collect();
        let [registration] = registrations.as_slice() else { panic!("unique split registration required") };
        HEADER_ORIGIN_FIRST_REFUSAL.with(|first| *first.borrow_mut() = None);
        let result = ix.xhr_gate(
            ix.split_xhr_send_call(factory, pair.init, pair.teardown, registration),
            "corrected_split.uninstrumented_outer_predicate",
        );
        let first = HEADER_ORIGIN_FIRST_REFUSAL.with(|first| *first.borrow());
        eprintln!("[split header origin] FIRST_REJECTION={first:?}; split_claim={}", result.is_some());
        assert!(result.is_some(), "production split proof refused at {first:?}; inspect the split-origin receipt");
        assert!(first.is_none(), "no earlier rejected own predicate may supply the positive");
    }

    #[test]
    fn native_capture_first_refusal() {
        let allocator = oxc_allocator::Allocator::default();
        let program = crate::analysis::parse_program(&allocator, SOURCE).expect("full witness parses");
        let (scoping, nodes) = crate::analysis::analyze(&program);
        let model = crate::signals::extract::bindings::Model::new(
            SOURCE, &program, &scoping, &nodes,
            crate::signals::schema::ProgramPropertiesLimits::default(),
        ).expect("full witness builds valid Model");
        assert!(!model.all_sites().is_empty());
        let ctx = FinderCtx { source: SOURCE, program: &program, scoping: &scoping, nodes: &nodes };
        let slots = ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes);
        let mut ix = Ix::new(&ctx, &slots);
        ix.index_xhr_source_proofs(&ctx);
        let factory = ix.xhr_factory().expect("unique actual XHR factory");
        let pairs = ix.lifecycle_pairs(factory);
        let pair = pairs.iter().find(|pair| !ix.body_is_noop(pair.init) && !ix.body_is_noop(pair.teardown))
            .expect("actual non-noop lifecycle pair");
        let registrations: Vec<_> = ix.own[pair.init].calls.iter().copied().filter(|call| {
            let Some((hook, "register")) = member(&call.callee) else { return false };
            let Some((bundle, "onBeforeInvoke")) = member(hook) else { return false };
            ix.symbol(bundle).and_then(|symbol| ix.assigns.get(&symbol)).is_some_and(|writes| writes.len() == 2)
        }).collect();
        let [registration] = registrations.as_slice() else { panic!("unique split registration required") };
        let (hook, "register") = member(&registration.callee).expect("actual register") else { panic!("register method") };
        let (bundle, "onBeforeInvoke") = member(hook).expect("actual hook") else { panic!("before-invoke phase") };
        let bundle = ix.symbol(bundle).expect("actual bundle binding");
        let captures: Vec<_> = ix.assigns[&bundle].iter().filter_map(|write| {
            let Expression::CallExpression(call) = strip(&write.right) else { return None };
            matches!(args(call).as_slice(), [_, method] if string_value(method) == Some("send")).then_some(call)
        }).collect();
        let [capture] = captures.as_slice() else { panic!("one actual native-send fallback call") };
        let symbol = ix.symbol(&capture.callee).expect("actual capture identifier");
        let helper = ix.resolve_fn(&capture.callee).expect("actual source function");
        let local_aliases: Vec<_> = scoping.get_resolved_references(symbol).filter_map(|reference| {
            let AstKind::VariableDeclarator(alias) = nodes.kind(nodes.parent_id(reference.node_id())) else { return None };
            let BindingPattern::BindingIdentifier(alias) = &alias.id else { return None };
            alias.symbol_id.get()
        }).collect();
        eprintln!("[native header origin] capture: declarations={} writes={} local_value_aliases={} indexed_native_protocol={}",
            scoping.symbol_declarations(symbol).count(),
            scoping.get_resolved_references(symbol).filter(|reference| reference.is_write()).count(),
            local_aliases.len(), ix.xhr_native_methods.contains(&helper));
        assert!(!local_aliases.is_empty(), "full source retains actual local installer alias");
        // The old one-hop checker rejects the main alias when encountered
        // again through the function declaration's initializer reference.
        let strict = ix.xhr_stable_callable(&capture.callee, 8);
        eprintln!("[native header origin] one_hop_strict_resolves={}", strict.is_some());
        assert!(strict.is_none(), "do not weaken global strict resolution to obtain support");
        HEADER_ORIGIN_FIRST_REFUSAL.with(|first| *first.borrow_mut() = None);
        let result = ix.xhr_gate(ix.xhr_native_capture_callable(&capture.callee), "native.corrected_capture_origin");
        let first = HEADER_ORIGIN_FIRST_REFUSAL.with(|first| *first.borrow());
        eprintln!("[native header origin] FIRST_REJECTION={first:?}; actual_native_origin={}", result.is_some());
        assert_eq!(result, Some(helper), "native origin refused at {first:?}; inspect this origin receipt");
        assert!(first.is_none(), "no rejected subpredicate may donate native identity");

        // Use-side ancestry must not widen the original private-scope guard.
        // A program-global var remains externally writable even when its one
        // textual assignment and call are ordered in the same nested block.
        let global_source = "function main(){let local=0;} main(); function installer(){} var exported; { exported=installer; exported(); }";
        let allocator = oxc_allocator::Allocator::default();
        let program = crate::analysis::parse_program(&allocator, global_source).expect("global negative parses");
        let (scoping, nodes) = crate::analysis::analyze(&program);
        let _model = crate::signals::extract::bindings::Model::new(
            global_source, &program, &scoping, &nodes,
            crate::signals::schema::ProgramPropertiesLimits::default(),
        ).expect("global negative builds valid Model");
        let ctx = FinderCtx { source: global_source, program: &program, scoping: &scoping, nodes: &nodes };
        let slots = ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes);
        let ix = Ix::new(&ctx, &slots);
        let symbol = nodes.iter().find_map(|node| {
            let AstKind::VariableDeclarator(declaration) = node.kind() else { return None };
            let BindingPattern::BindingIdentifier(id) = &declaration.id else { return None };
            (id.name.as_str() == "exported").then(|| id.symbol_id.get()).flatten()
        }).expect("global alias binding retained");
        let use_node = scoping.get_resolved_references(symbol).find_map(|reference|
            matches!(nodes.kind(nodes.parent_id(reference.node_id())), AstKind::CallExpression(call)
                if ix.symbol(&call.callee) == Some(symbol)).then_some(reference.node_id())
        ).expect("global alias call retained");
        assert!(ix.owner[scoping.symbol_declaration(symbol).index()].is_none());
        assert!(ix.xhr_native_alias_precedes(symbol, use_node).is_none());
    }

}
