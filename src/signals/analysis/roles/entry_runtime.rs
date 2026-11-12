//! Package `p01_entry_runtime`: the bootstrap shell, the exchange handshake,
//! runtime teardown and adapter start, request-policy data, the seed-wave
//! helpers and the shared MAIN state no other package reads first.
//!
//! Everything hangs off one structural anchor, the bootstrap shell. Most
//! builds publish the result of their nested MAIN function through a
//! non-cancelable `CustomEvent` from a program-level function called once by a
//! top-level statement. A build that lowers that publisher away instead keeps
//! the same four-step chain as nested functions: a wrapper that calls only its
//! registrar, a registrar that installs the event receiver for one literal
//! event-name list, and a receiver that removes the same listeners and starts
//! the nested MAIN factory from the dispatched event's `detail`.
//!
//! Once MAIN is identified, its `{ init }` result leads to the handshake and
//! the runtime constructor; the constructor's `{ init, teardown }` result
//! leads to the rest. Data bindings are proven by the value they hold and how
//! proven functions consume them. When a build lowers the shell/handshake
//! away, the runtime's logging-only pair is still reconstructed from the
//! global-null branch that returns it. See `roles/mod.rs` for the rules.
//!
//! The public init, the runtime constructor and its alias, the instrumenter
//! aliases and the native hook list are named by the built-in roles, so this
//! package reads them as anchors and does not claim them on the publisher
//! path. A listener chain's MAIN factory is only named by the late fallback
//! ([`late_entry_claims`]) so it can never displace a built-in main role.
//!
//! The plan does not refuse two different bindings claimed for one role, so
//! [`Claims::one`] fails closed here: a role with more than one candidate
//! claims nothing and stays `unproven` in the plan.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

use crate::signals::extract::bindings::{numeric_value, property_key_name, ParameterSlots};
use crate::signals::schema::Confidence;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};
use crate::analysis;

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Function }
}

const fn alias(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::FunctionAlias }
}

const fn binding(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::MainBinding }
}

const BOOTSTRAP_SHELL: &str = "bootstrap_shell";
const INSTALL_PROGRAM_INIT_LISTENERS: &str = "install_program_init_listeners";
const INITIALIZE_PROGRAM_FROM_EVENT: &str = "initialize_program_from_event";
const CREATE_PROGRAM_RUNTIME: &str = "create_program_runtime";
const EXCHANGE_VETO_LISTENER: &str = "exchange_veto_listener";
const EXCHANGE_RECEIVER: &str = "exchange_data_receiver";
const EXCHANGE_NAMES: &str = "exchange_event_names";
const NO_GLOBAL_INIT: &str = "no_global_init";
const NO_GLOBAL_INIT_ALIAS: &str = "no_global_init_alias";
const NO_GLOBAL_TEARDOWN: &str = "no_global_teardown";
const RUNTIME_TEARDOWN: &str = "runtime_teardown";
const RUN_TEARDOWN_CALLBACK: &str = "run_teardown_callback";
const TEARDOWN_ADAPTER: &str = "teardown_adapter_callback";
const TEARDOWN_CALLBACKS: &str = "teardown_callback_list";
const INVOKE_READY: &str = "invoke_ready_dispatcher_callback";
const START_ADAPTER: &str = "start_adapter_callback";
const SUBSCRIBE_PROGRESS: &str = "subscribe_progress_relay";
const RELAY_PROGRESS: &str = "relay_progress_record";
const LOOKUP_KEYSET: &str = "lookup_keyset_entry";
const KEYSET_TABLE: &str = "keyset_table";
const KEYSET_INDEXES: &str = "keyset_index_list";
const INVOKE_HOOK_FACTORY: &str = "invoke_hook_factory_callback";
const HOOK_FACTORY_LIST: &str = "platform_hook_factory_list";
const HOOK_FACTORY_ALIAS: &str = "platform_hook_factories";
const REFILL_PRNG: &str = "refill_sin_prng_buffer";
const CODE_UNITS_TO_STRING: &str = "code_units_to_string";
const PUSH_CODE_UNIT: &str = "push_code_unit_character";
const SET_PROTOTYPE: &str = "set_prototype_with_fallback";
const SET_PROTOTYPE_FALLBACK: &str = "set_prototype_legacy_fallback";
const PROTOTYPE_SETTER: &str = "prototype_setter_memo";
const CLASS_DEFINER_FACTORY: &str = "unused_class_definer_factory";
const CLASS_DEFINER: &str = "unused_class_definer";
const CLASS_DEFINER_RESULT: &str = "unused_class_definer_binding";
const READY_QUEUE_FLUSHER: &str = "ready_queue_flusher_alias";
const EMPTY_CALLBACK: &str = "unread_empty_callback_alias";
const PERFORMANCE_API: &str = "unread_performance_api";
const UNREAD_ZERO: &str = "unread_zero_value";
const TO_STRING_FACADE: &str = "to_string_facade";
const NATIVE_TO_STRING_SOURCES: &str = "native_to_string_sources";
const UNCURRY_BINDER: &str = "uncurry_method_binder";
const ROUTE_PATTERNS: &str = "url_route_patterns";
const ROUTE_TABLE: &str = "url_route_table";
const CLOSED_DESCRIPTORS: &str = "closed_property_descriptors";
const CLOSED_DESCRIPTOR_INDEX: &str = "closed_property_descriptor_index";
const NAN_BYTES: &str = "nan_float64_bytes";
const DOM_COLLECTOR: &str = "dom_structure_collector_descriptor";
const PRODUCT_COLLECTOR: &str = "product_page_collector_descriptor";
const ANCHOR_ACCESSORS: &str = "anchor_href_accessors";
const ANCHOR_DESCRIPTOR: &str = "anchor_href_descriptor";
const ANCHOR_SETTER: &str = "anchor_href_setter";
const PROTO_DESCRIPTOR: &str = "proto_accessor_descriptor";
const EXTENSION_REGISTRY: &str = "extension_registry_class";
const FIXED_EXTENSION: &str = "fixed_serialization_extension";
const SENTINEL_EXTENSION: &str = "sentinel_extension";
const LAZY_REGISTRY_CLASS: &str = "lazy_callback_registry_class";

/// Every role this package can claim, with the one name it receives.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    function(BOOTSTRAP_SHELL, "bootstrapProgram"),
    function(INSTALL_PROGRAM_INIT_LISTENERS, "installProgramInitListeners"),
    function(INITIALIZE_PROGRAM_FROM_EVENT, "initializeProgramFromEvent"),
    function(CREATE_PROGRAM_RUNTIME, "createProgramRuntime"),
    function(EXCHANGE_VETO_LISTENER, "preventDefaultHandler"),
    function(EXCHANGE_RECEIVER, "receiveExchangeData"),
    binding(EXCHANGE_NAMES, "exchangeEventNames"),
    function(NO_GLOBAL_INIT, "initWithoutGlobal"),
    alias(NO_GLOBAL_INIT_ALIAS, "noGlobalInitializer"),
    function(NO_GLOBAL_TEARDOWN, "noopTeardown"),
    function(RUNTIME_TEARDOWN, "teardownRuntimeState"),
    function(RUN_TEARDOWN_CALLBACK, "runTeardownCallback"),
    function(TEARDOWN_ADAPTER, "teardownAdapter"),
    binding(TEARDOWN_CALLBACKS, "teardownCallbacks"),
    function(INVOKE_READY, "invokeReadyDispatcher"),
    function(START_ADAPTER, "startAdapter"),
    function(SUBSCRIBE_PROGRESS, "subscribeProgressRelay"),
    function(RELAY_PROGRESS, "relayProgressRecord"),
    function(LOOKUP_KEYSET, "lookupKeysetEntry"),
    binding(KEYSET_TABLE, "keysetTable"),
    binding(KEYSET_INDEXES, "keysetIndexList"),
    function(INVOKE_HOOK_FACTORY, "invokeHookFactory"),
    binding(HOOK_FACTORY_LIST, "platformHookFactoryList"),
    binding(HOOK_FACTORY_ALIAS, "platformHookFactories"),
    function(REFILL_PRNG, "refillSinPrngBuffer"),
    function(CODE_UNITS_TO_STRING, "codeUnitsToString"),
    function(PUSH_CODE_UNIT, "pushCodeUnitCharacter"),
    function(SET_PROTOTYPE, "setPrototypeOfWithFallback"),
    function(SET_PROTOTYPE_FALLBACK, "setPrototypeOfLegacyFallback"),
    binding(PROTOTYPE_SETTER, "prototypeSetter"),
    function(CLASS_DEFINER_FACTORY, "createUnusedClassPropertyDefiner"),
    function(CLASS_DEFINER, "defineClassProperties"),
    binding(CLASS_DEFINER_RESULT, "unusedClassPropertyDefiner"),
    binding(READY_QUEUE_FLUSHER, "readyQueueFlusher"),
    binding(EMPTY_CALLBACK, "unreadEmptyCallback"),
    binding(PERFORMANCE_API, "performanceApi"),
    binding(UNREAD_ZERO, "unreadZeroValue"),
    binding(TO_STRING_FACADE, "toStringFacade"),
    binding(NATIVE_TO_STRING_SOURCES, "nativeToStringSources"),
    binding(UNCURRY_BINDER, "uncurryMethodBinder"),
    binding(ROUTE_PATTERNS, "urlRoutePatterns"),
    binding(ROUTE_TABLE, "urlRouteTable"),
    binding(CLOSED_DESCRIPTORS, "closedPropertyDescriptors"),
    binding(CLOSED_DESCRIPTOR_INDEX, "closedPropertyDescriptorIndex"),
    binding(NAN_BYTES, "nanFloat64Bytes"),
    binding(DOM_COLLECTOR, "domStructureSignalCollector"),
    binding(PRODUCT_COLLECTOR, "productPageSignalCollector"),
    binding(ANCHOR_ACCESSORS, "anchorHrefAccessors"),
    binding(ANCHOR_DESCRIPTOR, "anchorHrefDescriptor"),
    binding(ANCHOR_SETTER, "anchorHrefSetter"),
    binding(PROTO_DESCRIPTOR, "protoAccessorDescriptor"),
    binding(EXTENSION_REGISTRY, "extensionRegistryClass"),
    binding(FIXED_EXTENSION, "fixedSerializationExtension"),
    binding(SENTINEL_EXTENSION, "sentinelExtensionDefinition"),
    binding(LAZY_REGISTRY_CLASS, "lazyCallbackRegistryClass"),
];

// ===========================================================================
// Structural index
// ===========================================================================

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

impl<'a> FnEntry<'a> {
    fn statements(&self) -> &'a [Statement<'a>] {
        match self.func {
            FnNode::Function(function) => {
                function.body.as_ref().map_or(&[], |body| body.statements.as_slice())
            }
            FnNode::Arrow(arrow) => arrow.body.statements.as_slice(),
        }
    }

    /// The statements that do something: a trailing `return void 0` (the
    /// target tracer's explicit fall-off) does not count.
    fn effective_statements(&self) -> &'a [Statement<'a>] {
        let statements = self.statements();
        match statements.split_last() {
            Some((Statement::ReturnStatement(last), rest))
                if last.argument.as_ref().is_some_and(is_void0) =>
            {
                rest
            }
            _ => statements,
        }
    }
}

/// The nodes a function owns directly, without its nested functions.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    returns: Vec<&'a ReturnStatement<'a>>,
    declarators: Vec<&'a VariableDeclarator<'a>>,
    fors: Vec<&'a ForStatement<'a>>,
    ifs: Vec<&'a IfStatement<'a>>,
    members: BTreeSet<&'a str>,
    nested: Vec<usize>,
}

struct Index<'a> {
    program: &'a Program<'a>,
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    fns: Vec<FnEntry<'a>>,
    own: Vec<Own<'a>>,
    fn_by_span: BTreeMap<(u32, u32), usize>,
    decl_fn: BTreeMap<SymbolId, usize>,
    /// Plain writes of every binding: initializers and `=` assignments.
    writes: BTreeMap<SymbolId, Vec<&'a Expression<'a>>>,
    /// `binding.key = value` / `binding[key] = value` writes, by binding.
    member_writes: BTreeMap<SymbolId, Vec<&'a AssignmentExpression<'a>>>,
    slots: ParameterSlots<'a, 'a>,
}

impl<'a> Index<'a> {
    fn new(ctx: &FinderCtx<'_, 'a>) -> Self {
        let mut index = Self {
            program: ctx.program,
            scoping: ctx.scoping,
            nodes: ctx.nodes,
            fns: Vec::new(),
            own: Vec::new(),
            fn_by_span: BTreeMap::new(),
            decl_fn: BTreeMap::new(),
            writes: BTreeMap::new(),
            member_writes: BTreeMap::new(),
            slots: ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes),
        };
        index.build();
        index
    }

    fn build(&mut self) {
        const NONE: u32 = u32::MAX;
        let nodes = self.nodes;
        let mut owner = vec![NONE; nodes.len()];
        let mut fn_of_node = vec![NONE; nodes.len()];
        for (id, node) in nodes.iter_enumerated() {
            let at = id.index();
            let parent = nodes.parent_id(id);
            let own = if parent == id || parent.index() >= at {
                NONE
            } else if fn_of_node[parent.index()] != NONE {
                fn_of_node[parent.index()]
            } else {
                owner[parent.index()]
            };
            owner[at] = own;
            let own = (own != NONE).then_some(own as usize);
            let func = match node.kind() {
                AstKind::Function(function) => Some((function.span, FnNode::Function(function))),
                AstKind::ArrowFunctionExpression(arrow) => Some((arrow.span, FnNode::Arrow(arrow))),
                _ => None,
            };
            let Some((span, func)) = func else {
                self.index_node(node.kind(), own);
                continue;
            };
            let entry = self.fns.len();
            self.fns.push(FnEntry { node: id, span, func, parent: own });
            self.own.push(Own::default());
            self.fn_by_span.insert((span.start, span.end), entry);
            fn_of_node[at] = entry as u32;
            if let Some(own) = own {
                self.own[own].nested.push(entry);
            }
            if let FnNode::Function(function) = func {
                if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                    self.decl_fn.insert(symbol, entry);
                }
            }
        }
    }

    fn index_node(&mut self, kind: AstKind<'a>, owner: Option<usize>) {
        match kind {
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign =>
            {
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                        self.writes.entry(symbol).or_default().push(&assignment.right);
                    }
                } else if let Some(symbol) = target_object(&assignment.left).and_then(|object| self.symbol(object)) {
                    self.member_writes.entry(symbol).or_default().push(assignment);
                }
                if let Some(owner) = owner {
                    self.own[owner].assignments.push(assignment);
                }
            }
            AstKind::VariableDeclarator(declarator) => {
                if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                {
                    if let Some(symbol) = binding.symbol_id.get() {
                        self.writes.entry(symbol).or_default().push(init);
                    }
                }
                if let Some(owner) = owner {
                    self.own[owner].declarators.push(declarator);
                }
            }
            _ => {}
        }
        let Some(owner) = owner else {
            return;
        };
        let own = &mut self.own[owner];
        match kind {
            AstKind::CallExpression(call) => own.calls.push(call),
            AstKind::NewExpression(new) => own.news.push(new),
            AstKind::ReturnStatement(statement) => own.returns.push(statement),
            AstKind::ForStatement(statement) => own.fors.push(statement),
            AstKind::IfStatement(statement) => own.ifs.push(statement),
            AstKind::StaticMemberExpression(member) => {
                own.members.insert(member.property.name.as_str());
            }
            AstKind::ComputedMemberExpression(member) => {
                if let Expression::StringLiteral(literal) = &member.expression {
                    own.members.insert(literal.value.as_str());
                }
            }
            _ => {}
        }
    }

    fn symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        analysis::expression_symbol(expression.without_parentheses(), self.scoping)
    }

    /// The function an expression denotes, through single-origin bindings.
    fn function_of(&self, expression: &Expression<'_>) -> Option<usize> {
        let mut symbol = match expression.without_parentheses() {
            Expression::FunctionExpression(function) => {
                return self.fn_by_span.get(&(function.span.start, function.span.end)).copied();
            }
            Expression::ArrowFunctionExpression(arrow) => {
                return self.fn_by_span.get(&(arrow.span.start, arrow.span.end)).copied();
            }
            other => self.symbol(other)?,
        };
        for _ in 0..8 {
            if let Some(function) = self.decl_fn.get(&symbol) {
                return Some(*function);
            }
            let value = self.single_value(symbol)?;
            match value.without_parentheses() {
                Expression::Identifier(_) => symbol = self.symbol(value)?,
                _ => return self.function_of(value),
            }
        }
        None
    }

    /// The one value ever written to `symbol`.
    fn single_value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self.writes.get(&symbol)?.as_slice() {
            [value] => Some(value),
            _ => None,
        }
    }

    fn reads(&self, symbol: SymbolId) -> usize {
        self.scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_read())
            .count()
    }

    /// True when `expression` is parameter slot `slot` of function `entry`.
    fn is_slot(&self, expression: &Expression<'_>, entry: usize, slot: u32) -> bool {
        self.slots
            .parameter_slot(expression)
            .is_some_and(|found| found.owner == self.fns[entry].node && found.index == slot)
    }

    /// True when `expression` reads an unbound global called `name`.
    fn is_global(&self, expression: &Expression<'_>, name: &str) -> bool {
        matches!(
            expression.without_parentheses(),
            Expression::Identifier(identifier)
                if identifier.name == name
                    && analysis::identifier_symbol(identifier, self.scoping).is_none()
        )
    }

    /// True when `expression` is `root.path[0].path[1]...` on an unbound root.
    fn is_global_path(&self, expression: &Expression<'_>, root: &str, path: &[&str]) -> bool {
        match path.split_last() {
            None => self.is_global(expression, root),
            Some((last, rest)) => member(expression).is_some_and(|(object, name)| {
                name == *last && self.is_global_path(object, root, rest)
            }),
        }
    }

    /// Every function at or below `entry`.
    fn subtree(&self, entry: usize) -> Vec<usize> {
        let mut out = vec![entry];
        let mut at = 0;
        while at < out.len() {
            out.extend(self.own[out[at]].nested.iter().copied());
            at += 1;
        }
        out
    }

    /// True when the subtree calls `*.method(literal)` with a matching literal.
    fn calls_with_literal(&self, entry: usize, method: &str, test: impl Fn(&str) -> bool) -> bool {
        self.subtree(entry).into_iter().any(|function| {
            self.own[function].calls.iter().any(|call| {
                member(&call.callee).is_some_and(|(_, name)| name == method)
                    && matches!(args(call).first(), Some(Expression::StringLiteral(literal)) if test(literal.value.as_str()))
            })
        })
    }

    /// The bindings declared directly in MAIN's function scope, excluding
    /// function declarations.
    fn main_variables(&self, main: usize) -> Vec<SymbolId> {
        let FnNode::Function(function) = self.fns[main].func else {
            return Vec::new();
        };
        let Some(scope) = function.scope_id.get() else {
            return Vec::new();
        };
        let mut symbols: Vec<SymbolId> = self
            .scoping
            .iter_bindings_in(scope)
            .filter(|symbol| !self.decl_fn.contains_key(symbol))
            .collect();
        symbols.sort();
        symbols
    }

    /// MAIN variables whose one write is an identifier denoting `target`.
    fn aliases_of(&self, variables: &[SymbolId], target: usize) -> Vec<SymbolId> {
        variables
            .iter()
            .copied()
            .filter(|symbol| {
                self.single_value(*symbol).is_some_and(|value| {
                    matches!(value.without_parentheses(), Expression::Identifier(_))
                        && self.function_of(value) == Some(target)
                })
            })
            .collect()
    }

    /// The value of a single-expression function: `return value;` or an
    /// arrow's expression body.
    fn only_result(&self, entry: usize) -> Option<&'a Expression<'a>> {
        match self.fns[entry].effective_statements() {
            [Statement::ReturnStatement(statement)] => statement.argument.as_ref(),
            [Statement::ExpressionStatement(statement)]
                if matches!(self.fns[entry].func, FnNode::Arrow(arrow) if arrow.expression) =>
            {
                Some(&statement.expression)
            }
            _ => None,
        }
    }

    /// The one call a callback makes: `x => x()`, `() => { x(); }` or
    /// `function () { return x(); }`.
    fn only_call(&self, entry: usize) -> Option<&'a CallExpression<'a>> {
        let value = match self.fns[entry].effective_statements() {
            [Statement::ExpressionStatement(statement)] => &statement.expression,
            [Statement::ReturnStatement(statement)] => statement.argument.as_ref()?,
            _ => return None,
        };
        match value.without_parentheses() {
            Expression::CallExpression(call) => Some(call),
            _ => None,
        }
    }

    /// True when both expressions read the same binding or parameter slot.
    fn same_value(&self, left: &Expression<'_>, right: &Expression<'_>) -> bool {
        match (self.slots.parameter_slot(left), self.slots.parameter_slot(right)) {
            (Some(left), Some(right)) => left == right,
            (None, None) => self.symbol(left).is_some() && self.symbol(left) == self.symbol(right),
            _ => false,
        }
    }

    /// `object.method(a, b)` or the target tracer's equivalent
    /// `object.method.call(object, a, b)`: object, method and arguments.
    fn method_call<'e>(
        &self,
        call: &'e CallExpression<'a>,
    ) -> Option<(&'e Expression<'a>, &'e str, Vec<&'e Expression<'a>>)> {
        let (object, name) = member(&call.callee)?;
        let arguments = args(call);
        if name == "call" {
            if let (Some((receiver, method)), Some((first, rest))) = (member(object), arguments.split_first()) {
                if self.same_value(first, receiver) {
                    return Some((receiver, method, rest.to_vec()));
                }
            }
        }
        Some((object, name, arguments))
    }

    /// `object.method(callback, ...)`: the object and the callback's function.
    fn method_callback<'e>(
        &self,
        call: &'e CallExpression<'a>,
        method: &str,
    ) -> Option<(&'e Expression<'a>, usize)> {
        let (object, name, arguments) = self.method_call(call)?;
        if name != method {
            return None;
        }
        Some((object, self.function_of(arguments.first()?)?))
    }

    /// `callee()` or `callee.call(void 0)`: the call takes no value argument.
    fn is_plain_invocation(&self, call: &CallExpression<'_>, callee: &Expression<'_>) -> bool {
        if call.arguments.is_empty() {
            return call.callee.span() == callee.span();
        }
        let arguments = args(call);
        matches!(arguments.as_slice(), [only] if is_void0(only))
            && member(&call.callee).is_some_and(|(object, name)| name == "call" && object.span() == callee.span())
    }

    /// The element an `array[index]` read denotes, when `array` is a binding
    /// whose only write is an array literal. The seed-3 tracer lowers a method
    /// value into such a tuple (`[receiver, receiver.method]`); the index must
    /// be a literal and the binding must have one write, so nothing is guessed.
    fn tuple_element(&self, expression: &Expression<'_>) -> Option<&'a Expression<'a>> {
        let Expression::ComputedMemberExpression(index) = expression.without_parentheses() else {
            return None;
        };
        let at = numeric_value(index.expression.without_parentheses())?;
        if at < 0.0 || at.fract() != 0.0 || at > usize::MAX as f64 {
            return None;
        }
        let symbol = self.symbol(&index.object)?;
        let items = elements(self.single_value(symbol)?)?;
        items.get(at as usize).copied()
    }

    /// The publish call in either tracer shape: a direct
    /// `event.initCustomEvent(...)`, `event.initCustomEvent.call(event, ...)`,
    /// or the lowered local tuple `[event, event.initCustomEvent]` read as
    /// `tuple[1].call(tuple[0], ...)`. Returns the receiver and the publish
    /// arguments. Every indirection resolves through one-write bindings; an
    /// optional chain has no member form and is refused.
    fn publish_call<'e>(
        &self,
        call: &'e CallExpression<'a>,
    ) -> Option<(&'e Expression<'a>, Vec<&'e Expression<'a>>)> {
        let (callee, name) = member(&call.callee)?;
        let arguments = args(call);
        if name == "initCustomEvent" {
            return Some((callee, arguments));
        }
        if name != "call" {
            return None;
        }
        let (this_argument, rest) = arguments.split_first()?;
        let (receiver, method) = match member(callee) {
            Some(found) => found,
            None => member(self.tuple_element(callee)?)?,
        };
        if method != "initCustomEvent" {
            return None;
        }
        let same_receiver = self.same_value(this_argument, receiver)
            || self
                .tuple_element(this_argument)
                .is_some_and(|value| self.same_value(value, receiver));
        same_receiver.then_some((receiver, rest.to_vec()))
    }

    /// True when `expression` reads the one binding whose only write creates
    /// the publish event: `new CustomEvent(...)` or the legacy
    /// `document.createEvent("CustomEvent")` path.
    fn is_custom_event(&self, expression: &Expression<'_>) -> bool {
        let Some(symbol) = self.symbol(expression) else {
            return false;
        };
        let Some(value) = self.single_value(symbol) else {
            return false;
        };
        match value.without_parentheses() {
            Expression::NewExpression(new) => self.is_global(&new.callee, "CustomEvent"),
            Expression::CallExpression(call) => {
                self.is_global_path(&call.callee, "document", &["createEvent"])
                    && args(call).first().is_some_and(|argument| is_string(argument, "CustomEvent"))
            }
            _ => false,
        }
    }
}

// ===========================================================================
// Expression helpers
// ===========================================================================

fn is_void0(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void
    )
}

/// `object.name` / `object["name"]`.
fn member<'e, 'a>(expression: &'e Expression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

/// The object of `object.key = ...` / `object[key] = ...`.
fn target_object<'e, 'a>(target: &'e AssignmentTarget<'a>) -> Option<&'e Expression<'a>> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => Some(&member.object),
        AssignmentTarget::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

/// The literal key of `object.key = ...` / `object["key"] = ...`.
fn target_key<'e>(target: &'e AssignmentTarget<'_>) -> Option<&'e str> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn statement_call<'e, 'a>(statement: &'e Statement<'a>) -> Option<&'e CallExpression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    match statement.expression.without_parentheses() {
        Expression::CallExpression(call) => Some(call),
        _ => None,
    }
}

fn args<'e, 'a>(call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
    call.arguments.iter().filter_map(Argument::as_expression).collect()
}

/// The elements of an array literal without holes or spreads.
fn elements<'e, 'a>(expression: &'e Expression<'a>) -> Option<Vec<&'e Expression<'a>>> {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return None;
    };
    array.elements.iter().map(ArrayExpressionElement::as_expression).collect()
}

/// The `(name, value)` pairs of an object literal.
fn object_members<'e, 'a>(expression: &'e Expression<'a>) -> Option<Vec<(String, &'e Expression<'a>)>> {
    let Expression::ObjectExpression(object) = expression.without_parentheses() else {
        return None;
    };
    Some(
        object
            .properties
            .iter()
            .filter_map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    Some((property_key_name(&property.key)?, &property.value))
                }
                ObjectPropertyKind::SpreadProperty(_) => None,
            })
            .collect(),
    )
}

fn object_value<'e, 'a>(expression: &'e Expression<'a>, key: &str) -> Option<&'e Expression<'a>> {
    object_members(expression)?
        .into_iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

fn has_exact_keys(expression: &Expression<'_>, keys: &[&str]) -> bool {
    object_members(expression).is_some_and(|members| {
        members.len() == keys.len() && keys.iter().all(|key| members.iter().any(|(name, _)| name == key))
    })
}

fn is_string(expression: &Expression<'_>, value: &str) -> bool {
    matches!(expression.without_parentheses(), Expression::StringLiteral(literal) if literal.value == value)
}

fn is_bool(expression: &Expression<'_>, value: bool) -> bool {
    matches!(expression.without_parentheses(), Expression::BooleanLiteral(literal) if literal.value == value)
}

// ===========================================================================
// Claims
// ===========================================================================

struct Claims<'i, 'a> {
    index: &'i Index<'a>,
    out: Vec<FinderClaim>,
}

impl Claims<'_, '_> {
    fn function(&mut self, role: &'static str, function: usize, alias_role: Option<&'static str>, evidence: &str) {
        self.out.push(FinderClaim::Function {
            role,
            function_span: self.index.fns[function].span,
            alias_role,
            confidence: Confidence::High,
            evidence: vec![evidence.to_string()],
        });
    }

    fn binding(&mut self, role: &'static str, symbol: SymbolId, evidence: &str) {
        self.out.push(FinderClaim::MainBinding {
            role,
            identifier_span: self.index.scoping.symbol_span(symbol),
            confidence: Confidence::High,
            evidence: vec![evidence.to_string()],
        });
    }

    /// Claim the only candidate. Several candidates mean the structure does
    /// not tell them apart, so none is claimed.
    fn one(&mut self, role: &'static str, candidates: &[SymbolId], evidence: &str) {
        if let [symbol] = candidates {
            self.binding(role, *symbol, evidence);
        }
    }

    /// [`Self::one`] for functions.
    fn one_function(&mut self, role: &'static str, candidates: &[usize], alias_role: Option<&'static str>, evidence: &str) {
        if let [function] = candidates {
            self.function(role, *function, alias_role, evidence);
        }
    }
}

/// Structural claims of this package.
///
/// The publisher-shell path is claimed exactly as before. When no publisher
/// shell exists the listener-style entry chain is claimed instead: its
/// program-level wrapper, the registrar and the event receiver. MAIN itself
/// is deliberately left to the late fallback (below), so an existing built-in
/// main role is never displaced by this finder.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    collect(ctx, true)
}

/// The claims every existing consumer saw before the listener-style entry
/// chain was recognized: the publisher shell plus the global-null fallback.
///
/// The threads naming pass uses this for its early proposals, so the new
/// listener claims can never take priority over a shared, inherited or
/// collector claim. Those claims are supplied only by [`late_entry_claims`].
pub(crate) fn find_thread_early(ctx: &FinderCtx) -> Vec<FinderClaim> {
    collect(ctx, false)
}

/// The listener-style entry chain and the context claims anchored on its MAIN,
/// for a consumer that has already collected every other proposal and must not
/// displace one.
///
/// A publisher shell is not repeated: only its still-unnamed MAIN factory is
/// added. For a listener chain the four entry functions and the MAIN-anchored
/// context claims are returned, so a late caller can attach only the spans
/// that are still unproven. Building the structural index here is the whole
/// cost, independent of the number of functions.
pub(crate) fn late_entry_claims(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let index = Index::new(ctx);
    let mut claims = Claims { index: &index, out: Vec::new() };
    match find_shell(&index) {
        Some((_, main)) => {
            claims.function(
                CREATE_PROGRAM_RUNTIME,
                main,
                None,
                "the nested MAIN factory the publisher shell publishes: owns the program body and returns its `{ init }` entrypoint",
            );
        }
        None => {
            if let Some(chain) = find_listener_chain(&index) {
                claim_listener_chain(&index, &mut claims, &chain, true);
            }
        }
    }
    claims.out
}

fn collect(ctx: &FinderCtx, include_listener: bool) -> Vec<FinderClaim> {
    let index = Index::new(ctx);
    let mut claims = Claims { index: &index, out: Vec::new() };
    match find_shell(&index) {
        Some((root, main)) => claim_shell(&index, &mut claims, root, main),
        None if include_listener => {
            if let Some(chain) = find_listener_chain(&index) {
                claim_listener_chain(&index, &mut claims, &chain, false);
            }
        }
        None => {}
    }
    claim_no_global_fallback(&index, &mut claims);
    claims.out
}

/// The claims that hang off the proven publisher shell and handshake.
fn claim_shell(index: &Index<'_>, claims: &mut Claims<'_, '_>, root: usize, main: usize) {
    claims.function(
        BOOTSTRAP_SHELL,
        root,
        None,
        "program-level function called once by a top-level statement, publishing its nested function's result as the detail of a non-cancelable CustomEvent",
    );
    claim_context(index, main, claims);
}

/// The claims of the listener-style bootstrap chain. `claim_main` is set only
/// by the late fallback: the main naming pass keeps the reader's built-in main
/// role instead of claiming the same binding twice.
fn claim_listener_chain(
    index: &Index<'_>,
    claims: &mut Claims<'_, '_>,
    chain: &ListenerChain,
    claim_main: bool,
) {
    claims.function(
        BOOTSTRAP_SHELL,
        chain.root,
        None,
        "program-level function called once by a top-level statement whose whole body is the call of its only nested registrar",
    );
    claims.function(
        INSTALL_PROGRAM_INIT_LISTENERS,
        chain.registrar,
        None,
        "the nested registrar declaring one literal event-name list and registering its nested receiver for each name",
    );
    claims.function(
        INITIALIZE_PROGRAM_FROM_EVENT,
        chain.receiver,
        None,
        "the registered receiver removing the same name/handler pairs and starting the nested MAIN factory from the event's `detail`",
    );
    if claim_main {
        claims.function(
            CREATE_PROGRAM_RUNTIME,
            chain.main,
            None,
            "the nested MAIN factory the receiver calls, owning the program body and returning its `{ init }` entrypoint",
        );
        claim_context(index, chain.main, claims);
    }
}

/// The claims anchored on the identified MAIN.
fn claim_context(index: &Index<'_>, main: usize, claims: &mut Claims<'_, '_>) {
    let variables = index.main_variables(main);
    if let Some(runtime) = claim_handshake(index, main, &variables, claims) {
        claim_runtime(index, runtime, &variables, claims);
    }
    claim_library(index, main, &variables, claims);
    claim_ready_flusher(index, main, &variables, claims);
    claim_unread(index, &variables, claims);
    claim_tostring_state(index, &variables, claims);
    claim_routes(index, &variables, claims);
    claim_closed_descriptors(index, main, &variables, claims);
    claim_nan_bytes(index, &variables, claims);
    claim_collectors(index, &variables, claims);
    claim_descriptors(index, &variables, claims);
    claim_extensions(index, &variables, claims);
}

// ===========================================================================
// Bootstrap and handshake
// ===========================================================================

/// The bootstrap shell and MAIN: a program-level function called once at top
/// level that passes the result of its nested function as a CustomEvent detail.
fn find_shell(index: &Index<'_>) -> Option<(usize, usize)> {
    let mut found = Vec::new();
    for statement in &index.program.body {
        let Some(call) = statement_call(statement) else {
            continue;
        };
        let Some(symbol) = index.symbol(&call.callee) else {
            continue;
        };
        let Some(root) = index.decl_fn.get(&symbol).copied() else {
            continue;
        };
        if index.fns[root].parent.is_some() || !call.arguments.is_empty() || index.reads(symbol) != 1 {
            continue;
        }
        for publish in &index.own[root].calls {
            let Some((receiver, arguments)) = index.publish_call(publish) else {
                continue;
            };
            if !index.is_custom_event(receiver) {
                continue;
            }
            let [_, bubbles, cancelable, detail] = arguments.as_slice() else {
                continue;
            };
            if !is_bool(bubbles, false) || !is_bool(cancelable, false) {
                continue;
            }
            let Some(value) = index.symbol(detail).and_then(|symbol| index.single_value(symbol)) else {
                continue;
            };
            let Expression::CallExpression(produced) = value.without_parentheses() else {
                continue;
            };
            let Some(main) = index.function_of(&produced.callee) else {
                continue;
            };
            if index.fns[main].parent == Some(root) {
                found.push((root, main));
            }
        }
    }
    match found.as_slice() {
        [shell] => Some(*shell),
        _ => None,
    }
}

/// A listener-style bootstrap chain: the wrapper, the registrar, the event
/// receiver and the MAIN factory the receiver starts.
#[derive(Clone, Copy)]
struct ListenerChain {
    root: usize,
    registrar: usize,
    receiver: usize,
    main: usize,
}

/// The listener-style shell. A build that lowers the publisher away keeps the
/// same four-step bootstrap as nested functions: a program-level wrapper that
/// calls only its nested registrar, a registrar that declares one literal
/// event-name list and installs its nested receiver for each name, and a
/// receiver that removes the same listeners and starts the nested MAIN factory
/// from the dispatched event's `detail`.
fn find_listener_chain(index: &Index<'_>) -> Option<ListenerChain> {
    let mut found = Vec::new();
    for statement in &index.program.body {
        let Some(call) = statement_call(statement) else {
            continue;
        };
        let Some(symbol) = index.symbol(&call.callee) else {
            continue;
        };
        let Some(root) = index.decl_fn.get(&symbol).copied() else {
            continue;
        };
        if index.fns[root].parent.is_some() || !call.arguments.is_empty() || index.reads(symbol) != 1 {
            continue;
        }
        let own = &index.own[root];
        let [registrar] = own.nested.as_slice() else {
            continue;
        };
        let [invoke] = own.calls.as_slice() else {
            continue;
        };
        let registrar = *registrar;
        if !invoke.arguments.is_empty()
            || index.function_of(&invoke.callee) != Some(registrar)
            || index.fns[registrar].parent != Some(root)
        {
            continue;
        }
        let Some((names, receiver)) = listener_registrar(index, registrar) else {
            continue;
        };
        let Some(main) = listener_receiver(index, registrar, receiver, names) else {
            continue;
        };
        found.push(ListenerChain { root, registrar, receiver, main });
    }
    match found.as_slice() {
        [chain] => Some(*chain),
        _ => None,
    }
}

/// The registrar's one literal event-name list and the nested receiver it
/// registers for that list.
fn listener_registrar(index: &Index<'_>, registrar: usize) -> Option<(SymbolId, usize)> {
    let own = &index.own[registrar];
    let mut names = Vec::new();
    for declarator in &own.declarators {
        let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
            continue;
        };
        let Some(symbol) = binding.symbol_id.get() else {
            continue;
        };
        let Some(value) = index.single_value(symbol) else {
            continue;
        };
        let Some(items) = elements(value) else {
            continue;
        };
        if !items.is_empty()
            && items
                .iter()
                .all(|item| matches!(item.without_parentheses(), Expression::StringLiteral(_)))
        {
            names.push(symbol);
        }
    }
    let [names] = names.as_slice() else {
        return None;
    };
    let names = *names;
    let mut receivers = BTreeSet::new();
    for call in &own.calls {
        if !index.is_global(&call.callee, "addEventListener") {
            continue;
        }
        let arguments = args(call);
        let (name, handler) = match arguments.as_slice() {
            [name, handler] => (name, handler),
            [name, handler, capture] if is_bool(capture, false) => (name, handler),
            _ => continue,
        };
        let Expression::ComputedMemberExpression(entry) = name.without_parentheses() else {
            continue;
        };
        if index.symbol(&entry.object) != Some(names) {
            continue;
        }
        let Some(receiver) = index.function_of(handler) else {
            continue;
        };
        if index.fns[receiver].parent == Some(registrar) {
            receivers.insert(receiver);
        }
    }
    let receivers: Vec<usize> = receivers.into_iter().collect();
    let [receiver] = receivers.as_slice() else {
        return None;
    };
    Some((names, *receiver))
}

/// The receiver removes the same name/handler pairs and starts the nested MAIN
/// factory from the event's `detail`. Returns the factory.
fn listener_receiver(
    index: &Index<'_>,
    registrar: usize,
    receiver: usize,
    names: SymbolId,
) -> Option<usize> {
    let own = &index.own[receiver];
    let mut removes = false;
    for call in &own.calls {
        if !index.is_global(&call.callee, "removeEventListener") {
            continue;
        }
        let arguments = args(call);
        let (name, handler) = match arguments.as_slice() {
            [name, handler] => (name, handler),
            [name, handler, capture] if is_bool(capture, false) => (name, handler),
            _ => continue,
        };
        let Expression::ComputedMemberExpression(entry) = name.without_parentheses() else {
            continue;
        };
        if index.symbol(&entry.object) == Some(names) && index.function_of(handler) == Some(receiver) {
            removes = true;
        }
    }
    if !removes {
        return None;
    }
    listener_factory_start(index, registrar, receiver)
}

/// `MAIN(Object(this)).init.apply(Object(this), event.detail)` in the receiver:
/// the factory node, the this-like argument, the event `detail` and MAIN's own
/// `{ init }` result all have to agree.
fn listener_factory_start(index: &Index<'_>, registrar: usize, receiver: usize) -> Option<usize> {
    let mut found = BTreeSet::new();
    for call in &index.own[receiver].calls {
        let Some((init_object, operation)) = member(&call.callee) else {
            continue;
        };
        if operation != "apply" && operation != "call" {
            continue;
        }
        let Some((factory_call, method)) = member(init_object) else {
            continue;
        };
        if method != "init" {
            continue;
        }
        let Expression::CallExpression(factory_call) = factory_call.without_parentheses() else {
            continue;
        };
        let Some(main) = index.function_of(&factory_call.callee) else {
            continue;
        };
        if main == receiver || main == registrar {
            continue;
        }
        if !index.own[receiver].nested.contains(&main) && !index.own[registrar].nested.contains(&main) {
            continue;
        }
        if !factory_takes_this(index, factory_call) {
            continue;
        }
        let arguments = args(call);
        let [this_argument, detail] = arguments.as_slice() else {
            continue;
        };
        if !is_object_this(index, this_argument) || !is_event_detail(index, detail, receiver) {
            continue;
        }
        if returns_init_object(index, main) {
            found.insert(main);
        }
    }
    let found: Vec<usize> = found.into_iter().collect();
    let [main] = found.as_slice() else {
        return None;
    };
    Some(*main)
}

/// `MAIN()` or `MAIN(Object(this))` / `MAIN(this)`.
fn factory_takes_this(index: &Index<'_>, call: &CallExpression<'_>) -> bool {
    match args(call).as_slice() {
        [] => true,
        [only] => is_object_this(index, only),
        _ => false,
    }
}

/// `this` or the global `Object(this)`, never a shadowed `Object`.
fn is_object_this(index: &Index<'_>, expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::ThisExpression(_) => true,
        Expression::CallExpression(call) => {
            index.is_global(&call.callee, "Object")
                && matches!(
                    args(call).as_slice(),
                    [only] if matches!(only.without_parentheses(), Expression::ThisExpression(_))
                )
        }
        _ => false,
    }
}

/// The receiver's first parameter read as `.detail`.
fn is_event_detail(index: &Index<'_>, expression: &Expression<'_>, receiver: usize) -> bool {
    member(expression)
        .is_some_and(|(object, name)| name == "detail" && index.is_slot(object, receiver, 0))
}

/// MAIN returns its `{ init }` entrypoint object.
fn returns_init_object(index: &Index<'_>, main: usize) -> bool {
    index.own[main].returns.iter().any(|statement| {
        statement
            .argument
            .as_ref()
            .is_some_and(|argument| has_exact_keys(argument, &["init"]))
    })
}

/// The public init is MAIN's returned `{ init }`: it publishes the exchange
/// receiver in cancelable events, installs a capturing veto for the same names
/// and then constructs and starts the runtime. Returns the runtime constructor.
fn claim_handshake(index: &Index<'_>, main: usize, variables: &[SymbolId], claims: &mut Claims) -> Option<usize> {
    let public_init = index.own[main].returns.iter().find_map(|statement| {
        let argument = statement.argument.as_ref()?;
        if !has_exact_keys(argument, &["init"]) {
            return None;
        }
        index.function_of(object_value(argument, "init")?)
    })?;
    let own = &index.own[public_init];
    if !own.members.contains("initCustomEvent") {
        return None;
    }
    // The detail's `exchange` member is written in the object literal
    // (research seed 1) or after it (`{ exchange: void 0 }` then
    // `detail.exchange = receiver`, seeds 2/3). Both are one binding and one
    // function; anything else is refused.
    let receiver = own.declarators.iter().find_map(|declarator| {
        let init = declarator.init.as_ref()?;
        let direct = object_value(init, "exchange")?;
        let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
            return None;
        };
        let symbol = binding.symbol_id.get()?;
        let mut assigned = Vec::new();
        for write in index.member_writes.get(&symbol).into_iter().flatten() {
            if target_key(&write.left) == Some("exchange") {
                assigned.push(index.function_of(&write.right)?);
            }
        }
        match (index.function_of(direct), assigned.as_slice()) {
            (Some(receiver), []) => Some(receiver),
            (None, [receiver]) => Some(*receiver),
            _ => None,
        }
    })?;
    let (names, veto) = own.calls.iter().find_map(|call| {
        if !index.is_global(&call.callee, "addEventListener") {
            return None;
        }
        let arguments = args(call);
        let [name, handler, capture] = arguments.as_slice() else {
            return None;
        };
        let Expression::ComputedMemberExpression(name) = name.without_parentheses() else {
            return None;
        };
        is_bool(capture, true).then_some(())?;
        Some((index.symbol(&name.object)?, index.function_of(handler)?))
    })?;
    let prevents = index.only_call(veto).is_some_and(|call| {
        call.arguments.is_empty()
            && member(&call.callee)
                .is_some_and(|(object, name)| name == "preventDefault" && index.is_slot(object, veto, 0))
    });
    if !prevents || !is_exchange_receiver(index, receiver, public_init) {
        return None;
    }
    claims.function(EXCHANGE_VETO_LISTENER, veto, None, "capturing listener of the exchange events whose whole body is `event.preventDefault()`");
    claims.function(EXCHANGE_RECEIVER, receiver, None, "the `exchange` member of the event detail: stores its three arguments into three public-init locals");
    if variables.contains(&names) {
        claims.binding(EXCHANGE_NAMES, names, "the list the public init indexes for the veto listeners of its exchange events");
    }
    let starts = own.calls.iter().any(|call| {
        call.arguments.is_empty() && member(&call.callee).is_some_and(|(_, name)| name == "init")
    });
    let runtime = own.news.iter().find_map(|new| index.function_of(&new.callee));
    runtime.filter(|_| starts)
}

/// Three slots stored into three distinct locals of the enclosing function.
fn is_exchange_receiver(index: &Index<'_>, receiver: usize, public_init: usize) -> bool {
    let assignments = &index.own[receiver].assignments;
    let mut targets = BTreeSet::new();
    for (slot, assignment) in assignments.iter().enumerate() {
        if !index.is_slot(&assignment.right, receiver, slot as u32) {
            return false;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
            return false;
        };
        let Some(symbol) = analysis::identifier_symbol(identifier, index.scoping) else {
            return false;
        };
        targets.insert(symbol);
    }
    index.fns[receiver].parent == Some(public_init) && assignments.len() == 3 && targets.len() == 3
}

// ===========================================================================
// Runtime construction
// ===========================================================================

fn claim_runtime(index: &Index<'_>, runtime: usize, variables: &[SymbolId], claims: &mut Claims) {
    claim_no_global(index, runtime, claims);
    let started = index.own[runtime].returns.iter().find_map(|statement| {
        let argument = statement.argument.as_ref()?;
        let init = index.function_of(object_value(argument, "init")?)?;
        let teardown = index.function_of(object_value(argument, "teardown")?)?;
        Some((init, teardown))
    });
    if let Some((init, teardown)) = started {
        claim_teardown(index, teardown, variables, claims);
        claim_adapter_start(index, init, claims);
    }
    claim_encoder_inputs(index, runtime, variables, claims);
    claim_hook_factories(index, runtime, variables, claims);
}

/// The `{ init }` object with a later empty `.teardown`, built without a global.
fn claim_no_global(index: &Index<'_>, runtime: usize, claims: &mut Claims) {
    let mut pairs = Vec::new();
    for assignment in &index.own[runtime].assignments {
        if target_key(&assignment.left) != Some("teardown") {
            continue;
        }
        let Some(symbol) = target_object(&assignment.left).and_then(|object| index.symbol(object)) else {
            continue;
        };
        let Some(init) = index.single_value(symbol).and_then(|value| object_value(value, "init")) else {
            continue;
        };
        let (Some(init), Some(teardown)) = (index.function_of(init), index.function_of(&assignment.right)) else {
            continue;
        };
        let logs = matches!(index.fns[init].effective_statements(), [statement]
            if statement_call(statement).is_some_and(|call| index.is_global_path(&call.callee, "console", &["error"])));
        if logs && index.fns[teardown].effective_statements().is_empty() {
            pairs.push((init, teardown));
        }
    }
    if let [(init, teardown)] = pairs.as_slice() {
        claims.function(NO_GLOBAL_INIT, *init, Some(NO_GLOBAL_INIT_ALIAS), "`init` of the runtime pair whose teardown is empty; its whole body logs through console.error");
        claims.function(NO_GLOBAL_TEARDOWN, *teardown, None, "empty function assigned as `teardown` of the runtime's logging-only pair");
    }
}

/// The same logging-only pair, reached directly from the runtime's
/// `if (capturedGlobal == null)` fallback branch. Some builds lower the
/// shell/handshake differently and never reach [`claim_runtime`], so the
/// branch itself is reconstructed: the pair must be the one the branch
/// returns, its `{ init }` provider is the known `console.error` initializer,
/// and one owned `.teardown` write holds the empty function. Only this
/// package's teardown role is claimed here; the logging initializer already
/// carries the shared console-error label.
fn claim_no_global_fallback(index: &Index<'_>, claims: &mut Claims) {
    if claims.out.iter().any(|claim| claim.role() == NO_GLOBAL_TEARDOWN) {
        return;
    }
    let mut teardowns = BTreeSet::new();
    for candidate in 0..index.fns.len() {
        if let Some(teardown) = no_global_fallback_teardown(index, candidate) {
            teardowns.insert(teardown);
        }
    }
    let mut found = teardowns.into_iter();
    let Some(teardown) = found.next() else {
        return;
    };
    if found.next().is_some() {
        return;
    }
    claims.function(
        NO_GLOBAL_TEARDOWN,
        teardown,
        None,
        "empty function assigned as `teardown` of the runtime's logging-only pair returned from the global-null fallback branch",
    );
}

/// The empty teardown of the pair a runtime returns from its global-null
/// branch, when that pair is the same binding the branch returns and the
/// runtime also returns a started `{ init, teardown }` pair.
fn no_global_fallback_teardown(index: &Index<'_>, candidate: usize) -> Option<usize> {
    if !has_started_pair(index, candidate) {
        return None;
    }
    let mut found = Vec::new();
    for statement in &index.own[candidate].ifs {
        if !is_global_null_test(index, &statement.test) || statement.alternate.is_none() {
            continue;
        }
        let Some(pair) = branch_returned_symbol(index, &statement.consequent) else {
            continue;
        };
        let Some(init_value) = index.single_value(pair).and_then(|value| object_value(value, "init")) else {
            continue;
        };
        let Some(init) = index.function_of(init_value) else {
            continue;
        };
        if !logs_no_global(index, init) {
            continue;
        }
        let Some((_, teardown)) = branch_teardown_write(index, &statement.consequent, pair) else {
            continue;
        };
        if !index.fns[teardown].effective_statements().is_empty() {
            continue;
        }
        if index.fns[teardown].parent != Some(candidate) {
            continue;
        }
        if !agreeing_member_write(index, pair, "teardown", teardown, true)
            || !agreeing_member_write(index, pair, "init", init, false)
        {
            continue;
        }
        found.push(teardown);
    }
    match found.as_slice() {
        [teardown] => Some(*teardown),
        _ => None,
    }
}

/// The branch's direct statements; a non-block branch is one statement.
fn branch_statements<'e, 'a>(statement: &'e Statement<'a>) -> &'e [Statement<'a>] {
    match statement {
        Statement::BlockStatement(block) => block.body.as_slice(),
        _ => std::slice::from_ref(statement),
    }
}

/// The one binding a branch returns, when every direct return names a binding.
fn branch_returned_symbol(index: &Index<'_>, branch: &Statement<'_>) -> Option<SymbolId> {
    let mut returned = None;
    for statement in branch_statements(branch) {
        let Statement::ReturnStatement(statement) = statement else {
            continue;
        };
        let symbol = index.symbol(statement.argument.as_ref()?)?;
        if returned.replace(symbol).is_some() {
            return None;
        }
    }
    returned
}

/// The branch's one direct `pair.teardown = fn` write, with its function.
fn branch_teardown_write<'e, 'a>(
    index: &Index<'a>,
    branch: &'e Statement<'a>,
    pair: SymbolId,
) -> Option<(&'e AssignmentExpression<'a>, usize)> {
    let mut found = None;
    for statement in branch_statements(branch) {
        let Statement::ExpressionStatement(statement) = statement else {
            continue;
        };
        let Expression::AssignmentExpression(assignment) = statement.expression.without_parentheses() else {
            continue;
        };
        if assignment.operator != AssignmentOperator::Assign {
            continue;
        }
        if target_key(&assignment.left) != Some("teardown") {
            continue;
        }
        let Some(object) = target_object(&assignment.left).and_then(|object| index.symbol(object)) else {
            continue;
        };
        if object != pair {
            continue;
        }
        let Some(teardown) = index.function_of(&assignment.right) else {
            continue;
        };
        if found.replace((&**assignment, teardown)).is_some() {
            return None;
        }
    }
    found
}

/// `binding == null` / `binding === null`, including the mirrored literal-first
/// spelling.
fn is_global_null_test(index: &Index<'_>, test: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(binary) = test.without_parentheses() else {
        return false;
    };
    if !matches!(binary.operator, BinaryOperator::Equality | BinaryOperator::StrictEquality) {
        return false;
    }
    let null = |value: &Expression<'_>| matches!(value.without_parentheses(), Expression::NullLiteral(_));
    let binding = |value: &Expression<'_>| index.symbol(value).is_some();
    (null(&binary.right) && binding(&binary.left)) || (null(&binary.left) && binding(&binary.right))
}

/// The runtime also returns the started `{ init, teardown }` pair.
fn has_started_pair(index: &Index<'_>, candidate: usize) -> bool {
    index.own[candidate].returns.iter().any(|statement| {
        let Some(argument) = statement.argument.as_ref() else {
            return false;
        };
        object_value(argument, "init").is_some() && object_value(argument, "teardown").is_some()
    })
}

/// The initializer's whole body logs through the unshadowed `console.error`.
fn logs_no_global(index: &Index<'_>, init: usize) -> bool {
    matches!(index.fns[init].effective_statements(), [statement]
        if statement_call(statement).is_some_and(|call| index.is_global_path(&call.callee, "console", &["error"])))
}

/// Every `pair.key = fn` write agrees on `expected`; no write may disagree or
/// fail to resolve. `require_one` refuses an unassigned member (the teardown),
/// while the initializer may live in the literal only.
fn agreeing_member_write(index: &Index<'_>, pair: SymbolId, key: &str, expected: usize, require_one: bool) -> bool {
    let mut written = BTreeSet::new();
    for assignment in index.member_writes.get(&pair).into_iter().flatten() {
        if target_key(&assignment.left) != Some(key) {
            continue;
        }
        let Some(target) = index.function_of(&assignment.right) else {
            return false;
        };
        written.insert(target);
    }
    if written.is_empty() {
        return !require_one;
    }
    written.len() == 1 && written.contains(&expected)
}

fn claim_teardown(index: &Index<'_>, teardown: usize, variables: &[SymbolId], claims: &mut Claims) {
    let mut cleanups = Vec::new();
    let mut adapters = Vec::new();
    for call in &index.own[teardown].calls {
        let Some((list, callback)) = index.method_callback(call, "forEach") else {
            continue;
        };
        let Some(inner) = index.only_call(callback) else {
            continue;
        };
        let cleanup = member(&inner.callee).map_or(&inner.callee, |(object, _)| object);
        if index.is_slot(cleanup, callback, 0) && index.is_plain_invocation(inner, cleanup) {
            cleanups.push((callback, index.symbol(list)));
        } else if inner.arguments.is_empty()
            && member(&inner.callee)
                .is_some_and(|(object, name)| name == "teardown" && index.is_slot(object, callback, 0))
        {
            adapters.push(callback);
        }
    }
    let ([(cleanup, list)], [adapter]) = (cleanups.as_slice(), adapters.as_slice()) else {
        return;
    };
    claims.function(RUNTIME_TEARDOWN, teardown, None, "`teardown` of the runtime's started pair: runs every listed cleanup and every adapter's teardown");
    claims.function(RUN_TEARDOWN_CALLBACK, *cleanup, None, "forEach callback of the runtime teardown calling its element with no arguments");
    claims.function(TEARDOWN_ADAPTER, *adapter, None, "forEach callback of the runtime teardown calling its element's `teardown`");
    if let Some(list) = list.filter(|list| variables.contains(list)) {
        claims.binding(TEARDOWN_CALLBACKS, list, "the MAIN list the runtime teardown runs as no-argument cleanups");
    }
}

fn claim_adapter_start(index: &Index<'_>, init: usize, claims: &mut Claims) {
    let own = &index.own[init];
    let mut ready = Vec::new();
    let mut adapters = Vec::new();
    for call in &own.calls {
        if let Some((_, callback)) = index.method_callback(call, "forEach") {
            let members = &index.own[callback].members;
            if members.contains("afterReady") && members.contains("init") {
                adapters.push(callback);
            }
        }
        let arguments = args(call);
        let [argument] = arguments.as_slice() else {
            continue;
        };
        let Some(callback) = index.function_of(argument) else {
            continue;
        };
        let Some(inner) = index.only_call(callback).filter(|inner| inner.arguments.is_empty()) else {
            continue;
        };
        let Some(callee) = index.symbol(&inner.callee) else {
            continue;
        };
        if own.ifs.iter().any(|statement| is_typeof_function(index, &statement.test, callee)) {
            ready.push(callback);
        }
    }
    claims.one_function(INVOKE_READY, &ready, None, "callback handed to a registrar in runtime init, calling the typeof-function guarded MAIN binding");
    let [adapter] = adapters.as_slice() else {
        return;
    };
    claims.function(START_ADAPTER, *adapter, None, "forEach callback of runtime init calling each element's `init` and scheduling its `afterReady`");
    let subscribers: Vec<usize> = index.own[*adapter]
        .news
        .iter()
        .filter_map(|new| index.function_of(new.arguments.first()?.as_expression()?))
        .collect();
    let [subscriber] = subscribers.as_slice() else {
        return;
    };
    let relays: Vec<usize> = index.own[*subscriber]
        .calls
        .iter()
        .filter_map(|call| index.method_callback(call, "push").map(|(_, relay)| relay))
        .filter(|relay| {
            index.only_call(*relay).is_some_and(|inner| {
                index.method_call(inner).is_some_and(|(_, name, arguments)| {
                    name == "next" && arguments.first().is_some_and(|value| index.is_slot(value, *relay, 0))
                })
            })
        })
        .collect();
    if let [relay] = relays.as_slice() {
        claims.function(SUBSCRIBE_PROGRESS, *subscriber, None, "observable subscriber given to each adapter: pushes a relay to the subscriber's `next` onto a shared list");
        claims.function(RELAY_PROGRESS, *relay, None, "relay forwarding its argument to the subscriber's `next`");
    }
}

/// `typeof symbol === "function"`.
fn is_typeof_function(index: &Index<'_>, test: &Expression<'_>, symbol: SymbolId) -> bool {
    let Expression::BinaryExpression(binary) = test.without_parentheses() else {
        return false;
    };
    binary.operator == BinaryOperator::StrictEquality
        && is_string(&binary.right, "function")
        && matches!(
            binary.left.without_parentheses(),
            Expression::UnaryExpression(unary)
                if unary.operator == UnaryOperator::Typeof && index.symbol(&unary.argument) == Some(symbol)
        )
}

/// `list.map(i => table[i])` passed to the runtime's encoder.
fn claim_encoder_inputs(index: &Index<'_>, runtime: usize, variables: &[SymbolId], claims: &mut Claims) {
    let mut found = Vec::new();
    for call in &index.own[runtime].calls {
        let Some((list, callback)) = index.method_callback(call, "map") else {
            continue;
        };
        let Some(Expression::ComputedMemberExpression(lookup)) =
            index.only_result(callback).map(Expression::without_parentheses)
        else {
            continue;
        };
        if !index.is_slot(&lookup.expression, callback, 0) {
            continue;
        }
        let (Some(list), Some(table)) = (index.symbol(list), index.symbol(&lookup.object)) else {
            continue;
        };
        if !variables.contains(&list) || !variables.contains(&table) {
            continue;
        }
        found.push((callback, list, table));
    }
    let [(callback, list, table)] = found.as_slice() else {
        return;
    };
    claims.function(LOOKUP_KEYSET, *callback, None, "map callback of the runtime constructor reading a MAIN table at its element");
    claims.binding(KEYSET_INDEXES, *list, "the MAIN list mapped through the table for the runtime's encoder");
    claims.binding(KEYSET_TABLE, *table, "the MAIN table indexed by the runtime's encoder key-set map");
}

/// `[].map.call(factories, f => f(...))` in the runtime constructor.
fn claim_hook_factories(index: &Index<'_>, runtime: usize, variables: &[SymbolId], claims: &mut Claims) {
    let mut found = Vec::new();
    for call in &index.own[runtime].calls {
        let maps_array = member(&call.callee).is_some_and(|(object, name)| {
            name == "call"
                && member(object).is_some_and(|(array, name)| {
                    name == "map" && elements(array).is_some_and(|items| items.is_empty())
                })
        });
        let arguments = args(call);
        let ([list, callback], true) = (arguments.as_slice(), maps_array) else {
            continue;
        };
        let (Some(callback), Some(list)) = (index.function_of(callback), index.symbol(list)) else {
            continue;
        };
        let invokes = index.only_call(callback).is_some_and(|inner| {
            index.is_slot(&inner.callee, callback, 0)
                || member(&inner.callee)
                    .is_some_and(|(object, name)| name == "call" && index.is_slot(object, callback, 0))
        });
        if invokes && variables.contains(&list) {
            found.push((callback, list));
        }
    }
    let [(callback, list)] = found.as_slice() else {
        return;
    };
    claims.function(INVOKE_HOOK_FACTORY, *callback, None, "callback of the runtime's array-map over a MAIN list, calling its element");
    claims.binding(HOOK_FACTORY_ALIAS, *list, "the MAIN list of factories the runtime constructor maps");
    if let Some(origin) = index.single_value(*list).and_then(|value| index.symbol(value)) {
        if variables.contains(&origin) {
            claims.binding(HOOK_FACTORY_LIST, origin, "the array of factory functions behind the runtime's mapped list");
        }
    }
}

// ===========================================================================
// Shared library helpers
// ===========================================================================

fn claim_library(index: &Index<'_>, main: usize, variables: &[SymbolId], claims: &mut Claims) {
    let scope = index.subtree(main);
    let refills: Vec<usize> = scope.iter().copied().filter(|entry| is_prng_refill(index, *entry)).collect();
    claims.one_function(REFILL_PRNG, &refills, None, "pushes Math.sin/Math.fround derived fractions onto its first slot and bumps a counter element of it");

    let decoders: Vec<(usize, usize)> = scope.iter().filter_map(|entry| code_units_decoder(index, *entry)).collect();
    if let [(decoder, push)] = decoders.as_slice() {
        claims.function(CODE_UNITS_TO_STRING, *decoder, None, "maps its first slot through String.fromCharCode into an array and joins it");
        claims.function(PUSH_CODE_UNIT, *push, None, "forEach callback pushing String.fromCharCode of its element");
    }

    let setters: Vec<(usize, usize, SymbolId)> = scope.iter().filter_map(|entry| prototype_setter(index, *entry)).collect();
    if let [(setter, fallback, memo)] = setters.as_slice() {
        claims.function(SET_PROTOTYPE, *setter, None, "picks Object.setPrototypeOf or a __proto__ fallback, memoizes it into an outer binding and tail-calls it");
        claims.function(SET_PROTOTYPE_FALLBACK, *fallback, None, "assigns slot 1 to slot 0's __proto__");
        if variables.contains(memo) {
            claims.binding(PROTOTYPE_SETTER, *memo, "outer binding the choosing function overwrites with the chosen prototype setter");
        }
    }

    let mut unused = Vec::new();
    for symbol in variables {
        let Some(Expression::CallExpression(call)) = index.single_value(*symbol).map(Expression::without_parentheses) else {
            continue;
        };
        if index.reads(*symbol) != 0 || !call.arguments.is_empty() {
            continue;
        }
        let Some(factory) = index.function_of(&call.callee) else {
            continue;
        };
        if let Some(definer) = class_definer(index, factory) {
            unused.push((*symbol, factory, definer));
        }
    }
    if let [(symbol, factory, definer)] = unused.as_slice() {
        claims.function(CLASS_DEFINER_FACTORY, *factory, None, "returns a nested class-property definer; the binding of its result is never read");
        claims.function(CLASS_DEFINER, *definer, None, "defines descriptors from slots 1 and 2 on slot 0's prototype and on slot 0");
        claims.binding(CLASS_DEFINER_RESULT, *symbol, "single write of a class-property definer factory's result, never read");
    }
}

fn is_prng_refill(index: &Index<'_>, entry: usize) -> bool {
    let own = &index.own[entry];
    let uses = |name: &str| own.calls.iter().any(|call| index.is_global_path(&call.callee, "Math", &[name]));
    let pushes = own.calls.iter().any(|call| {
        index.method_call(call).is_some_and(|(object, name, _)| name == "push" && index.is_slot(object, entry, 0))
    });
    let bumps = own.assignments.iter().any(|assignment| {
        target_object(&assignment.left).is_some_and(|object| index.is_slot(object, entry, 0))
            && matches!(
                assignment.right.without_parentheses(),
                Expression::BinaryExpression(binary)
                    if binary.operator == BinaryOperator::Addition
                        && matches!(
                            binary.left.without_parentheses(),
                            Expression::CallExpression(call) if index.is_global(&call.callee, "Number")
                        )
            )
    });
    uses("sin") && uses("fround") && pushes && bumps
}

/// `slot0.forEach(c => out.push(String.fromCharCode(c))); return out.join("")`.
fn code_units_decoder(index: &Index<'_>, entry: usize) -> Option<(usize, usize)> {
    let own = &index.own[entry];
    let joined = own.returns.iter().find_map(|statement| {
        let Expression::CallExpression(call) = statement.argument.as_ref()?.without_parentheses() else {
            return None;
        };
        let (object, name, arguments) = index.method_call(call)?;
        (name == "join" && arguments.first().is_some_and(|value| is_string(value, ""))).then(|| index.symbol(object))?
    })?;
    own.calls.iter().find_map(|call| {
        let (list, callback) = index.method_callback(call, "forEach")?;
        let inner = index.only_call(callback)?;
        let (target, name, arguments) = index.method_call(inner)?;
        let Expression::CallExpression(convert) = arguments.first()?.without_parentheses() else {
            return None;
        };
        let converts = index.is_global_path(&convert.callee, "String", &["fromCharCode"])
            && args(convert).first().is_some_and(|value| index.is_slot(value, callback, 0));
        (converts && name == "push" && index.is_slot(list, entry, 0) && index.symbol(target) == Some(joined))
            .then_some((entry, callback))
    })
}

/// Chooses a setter, stores it in an outer binding and tail-calls it.
fn prototype_setter(index: &Index<'_>, entry: usize) -> Option<(usize, usize, SymbolId)> {
    let own = &index.own[entry];
    if !own.members.contains("setPrototypeOf") {
        return None;
    }
    let fallback = own.nested.iter().copied().find(|nested| {
        index.own[*nested].assignments.iter().any(|assignment| {
            target_key(&assignment.left) == Some("__proto__")
                && target_object(&assignment.left).is_some_and(|object| index.is_slot(object, *nested, 0))
                && index.is_slot(&assignment.right, *nested, 1)
        })
    })?;
    let chosen = own.returns.iter().find_map(|statement| {
        let Expression::CallExpression(call) = statement.argument.as_ref()?.without_parentheses() else {
            return None;
        };
        let arguments = args(call);
        let [first, second] = arguments.as_slice() else {
            return None;
        };
        (index.is_slot(first, entry, 0) && index.is_slot(second, entry, 1)).then(|| index.symbol(&call.callee))?
    })?;
    let memo = own.assignments.iter().find_map(|assignment| {
        let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
            return None;
        };
        let target = analysis::identifier_symbol(identifier, index.scoping)?;
        (target != chosen && index.symbol(&assignment.right) == Some(chosen)).then_some(target)
    })?;
    Some((entry, fallback, memo))
}

/// A factory returning its nested class-property definer.
fn class_definer(index: &Index<'_>, factory: usize) -> Option<usize> {
    let definer = index.function_of(index.own[factory].returns.first()?.argument.as_ref()?)?;
    let own = &index.own[definer];
    let defines: Vec<_> = own
        .calls
        .iter()
        .filter(|call| index.is_global_path(&call.callee, "Object", &["defineProperty"]))
        .collect();
    let on_prototype = defines.iter().any(|call| {
        args(call).first().is_some_and(|target| {
            member(target).is_some_and(|(object, name)| name == "prototype" && index.is_slot(object, definer, 0))
        })
    });
    (defines.len() == 2 && on_prototype && own.members.contains("enumerable") && own.members.contains("configurable"))
        .then_some(definer)
}

// ===========================================================================
// MAIN state
// ===========================================================================

fn claim_ready_flusher(index: &Index<'_>, main: usize, variables: &[SymbolId], claims: &mut Claims) {
    let mut flushers = Vec::new();
    for call in &index.own[main].calls {
        let arguments = args(call);
        if let (true, [event, handler]) = (
            index.is_global_path(&call.callee, "document", &["addEventListener"]),
            arguments.as_slice(),
        ) {
            if let (true, Some(function)) = (is_string(event, "DOMContentLoaded"), index.function_of(handler)) {
                flushers.extend(index.aliases_of(variables, function));
            }
        }
    }
    claims.one(READY_QUEUE_FLUSHER, &flushers, "MAIN alias of the DOMContentLoaded listener");
}

/// Bindings no code reads: the name states only what the single write holds.
/// The unread empty arrays and random tokens have several equal candidates,
/// and nothing but their position tells those apart, so they stay unnamed.
fn claim_unread(index: &Index<'_>, variables: &[SymbolId], claims: &mut Claims) {
    let mut empty = Vec::new();
    let mut performance = Vec::new();
    let mut zero = Vec::new();
    for symbol in variables {
        let Some(value) = index.single_value(*symbol).filter(|_| index.reads(*symbol) == 0) else {
            continue;
        };
        if matches!(value.without_parentheses(), Expression::Identifier(_)) {
            if index.function_of(value).is_some_and(|function| index.fns[function].effective_statements().is_empty()) {
                empty.push(*symbol);
            }
        } else if member(value).is_some_and(|(_, name)| name == "performance") {
            performance.push(*symbol);
        } else if numeric_value(value.without_parentheses()) == Some(0.0) {
            zero.push(*symbol);
        }
    }
    claims.one(EMPTY_CALLBACK, &empty, "never-read MAIN alias of an empty function");
    claims.one(PERFORMANCE_API, &performance, "never-read MAIN binding holding the host `performance` object");
    claims.one(UNREAD_ZERO, &zero, "never-read MAIN binding whose only write is zero");
}

fn claim_tostring_state(index: &Index<'_>, variables: &[SymbolId], claims: &mut Claims) {
    let mut facades = Vec::new();
    let mut sources = Vec::new();
    let mut binders = Vec::new();
    for symbol in variables {
        let Some(value) = index.single_value(*symbol) else {
            continue;
        };
        let facade = has_exact_keys(value, &["toString"])
            && index.member_writes.get(symbol).is_some_and(|writes| {
                writes.iter().any(|write| {
                    target_key(&write.left) == Some("toString")
                        && index.function_of(&write.right).is_some_and(|function| index.own[function].members.contains("lastIndexOf"))
                })
            });
        if facade {
            facades.push(*symbol);
        }
        let holds_source = elements(value).is_some_and(|items| {
            matches!(items.as_slice(), [Expression::CallExpression(call)]
                if args(call).first().is_some_and(|argument| index.is_global_path(argument, "Function", &["prototype", "toString"])))
        });
        if holds_source {
            sources.push(*symbol);
        }
        if let Expression::CallExpression(call) = value.without_parentheses() {
            let arguments = args(call);
            let uncurries = index.is_global_path(&call.callee, "Object", &["call", "bind"])
                && matches!(arguments.as_slice(), [first, second]
                    if index.is_global_path(first, "Object", &["bind"]) && index.is_global_path(second, "Object", &["call"]));
            if uncurries {
                binders.push(*symbol);
            }
        }
    }
    claims.one(TO_STRING_FACADE, &facades, "one-key `toString` object whose method looks its receiver up with lastIndexOf");
    claims.one(NATIVE_TO_STRING_SOURCES, &sources, "array seeded with the source text of Function.prototype.toString");
    claims.one(UNCURRY_BINDER, &binders, "holds Object.call.bind(Object.bind, Object.call), the uncurry-this binder");
}

fn claim_routes(index: &Index<'_>, variables: &[SymbolId], claims: &mut Claims) {
    let mut patterns = Vec::new();
    let mut tables = Vec::new();
    for symbol in variables {
        let Some(items) = index.single_value(*symbol).and_then(elements) else {
            continue;
        };
        let Some((first, rest)) = items.split_first() else {
            continue;
        };
        if rest.is_empty() || !rest.iter().all(|entry| is_route_entry(entry)) {
            continue;
        }
        if let Some(list) = index.symbol(first) {
            // A separate pattern binding: the pair is proven together.
            if holds_route_patterns(index, list) {
                patterns.push(list);
                tables.push(*symbol);
            }
        } else if holds_inline_route_patterns(index, first) && !read_as_call_argument(index, *symbol) {
            // The capture inlines the pattern list as the table's element 0.
            // The table is proven through it; there is no pattern binding, so
            // no binding is claimed for `url_route_patterns`.
            tables.push(*symbol);
        }
    }
    claims.one(ROUTE_PATTERNS, &patterns, "array of RegExp values, element 0 of the route table");
    claims.one(ROUTE_TABLE, &tables, "pattern list (a binding or an inline RegExp list) followed by [[HTTP methods], [pattern indexes]] entries");
}

/// A `RegExp(...)` call with a string pattern.
fn is_regexp_call(index: &Index<'_>, value: &Expression<'_>) -> bool {
    matches!(
        value.without_parentheses(),
        Expression::CallExpression(call)
            if index.is_global(&call.callee, "RegExp")
                && args(call).first().is_some_and(|pattern| matches!(pattern.without_parentheses(), Expression::StringLiteral(_)))
    )
}

/// An array of RegExp values with optional `void 0` holes, the shape a capture
/// inlines in place of the pattern binding.
fn are_route_patterns(index: &Index<'_>, items: &[&Expression<'_>]) -> bool {
    items.first().is_some_and(|first| is_regexp_call(index, first))
        && items.iter().all(|item| is_regexp_call(index, item) || is_void0(item))
}

fn holds_inline_route_patterns(index: &Index<'_>, expression: &Expression<'_>) -> bool {
    elements(expression).is_some_and(|items| are_route_patterns(index, &items))
}

/// True when `symbol` is read as an argument of a call. A table handed
/// straight to another proven consumer is that consumer's slot: the receiving
/// package owns the slot role, so this package does not claim the same binding
/// for its own data role. In R1 the table data is a binding of its own and the
/// consumer reads an alias binding instead; a capture that inlines the
/// patterns and passes the table itself has no separate data binding to name.
fn read_as_call_argument(index: &Index<'_>, symbol: SymbolId) -> bool {
    index.scoping.get_resolved_references(symbol).any(|reference| {
        !reference.is_write()
            && matches!(
                index.nodes.kind(index.nodes.parent_id(reference.node_id())),
                AstKind::CallExpression(_)
            )
    })
}

fn holds_route_patterns(index: &Index<'_>, symbol: SymbolId) -> bool {
    let literal = index
        .single_value(symbol)
        .and_then(elements)
        .is_some_and(|items| are_route_patterns(index, &items));
    literal && index.member_writes.get(&symbol).is_some_and(|writes| writes.iter().all(|write| is_regexp_call(index, &write.right)))
}

/// A `[[methods...], [indexes...]]` pair, or a list of them.
fn is_route_entry(expression: &Expression<'_>) -> bool {
    let Some(items) = elements(expression) else {
        return false;
    };
    if let [methods, indexes] = items.as_slice() {
        if let (Some(methods), Some(indexes)) = (elements(methods), elements(indexes)) {
            let method = |value: &&Expression<'_>| {
                matches!(value.without_parentheses(), Expression::StringLiteral(literal)
                    if matches!(literal.value.as_str(), "ANY" | "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS"))
            };
            if !methods.is_empty()
                && methods.iter().all(method)
                && !indexes.is_empty()
                && indexes.iter().all(|value| numeric_value(value.without_parentheses()).is_some())
            {
                return true;
            }
        }
    }
    !items.is_empty() && items.iter().all(|item| is_route_entry(item))
}

/// The `{ key: "closed" }` getter descriptor re-bound to a list and defined on
/// a class prototype by a loop in MAIN's own body.
///
/// Two classes can each define a `closed` property this way. The role this
/// package names is the descriptor of the class that wraps another
/// subscription: its getter delegates one member deeper
/// (`return this.<subscription>.<observer> === void 0`). The sibling class's
/// getter reads a direct `this.<observer>`, so the getter's member-chain depth
/// separates the two sites without reading any generated name.
fn claim_closed_descriptors(index: &Index<'_>, main: usize, variables: &[SymbolId], claims: &mut Claims) {
    let mut found = Vec::new();
    for statement in &index.own[main].fors {
        let Some(Expression::BinaryExpression(test)) = statement.test.as_ref().map(Expression::without_parentheses) else {
            continue;
        };
        let (Some(counter), Some((list, "length"))) = (index.symbol(&test.left), member(&test.right)) else {
            continue;
        };
        let Some(list) = index.symbol(list) else {
            continue;
        };
        let describes = index.writes.get(&list).is_some_and(|writes| {
            writes.iter().any(|value| object_value(value, "key").is_some_and(|key| is_string(key, "closed")))
        });
        let defines = index.own[main].calls.iter().any(|call| {
            call.span.start >= statement.span.start
                && call.span.end <= statement.span.end
                && index.is_global_path(&call.callee, "Object", &["defineProperty"])
                && args(call).last().is_some_and(|value| {
                    matches!(value.without_parentheses(), Expression::ComputedMemberExpression(entry)
                        if index.symbol(&entry.object) == Some(list))
                })
        });
        if describes && defines && variables.contains(&counter) && variables.contains(&list) {
            found.push((list, counter));
        }
    }
    let delegating: Vec<(SymbolId, SymbolId)> = found
        .into_iter()
        .filter(|(list, _)| closed_getter_delegates(index, *list))
        .collect();
    if let [(list, counter)] = delegating.as_slice() {
        claims.binding(CLOSED_DESCRIPTORS, *list, "`closed` property descriptor whose getter delegates through the inner subscription; re-bound to a one-element list and defined in a loop on a class prototype");
        claims.binding(CLOSED_DESCRIPTOR_INDEX, *counter, "the counter of the delegating `closed` property definition loop");
    }
}

/// True when the descriptor's `get` getter is
/// `return [void 0 ===] this.<a>.<b> [=== void 0]`, a two-member chain rooted
/// at `this`.
fn closed_getter_delegates(index: &Index<'_>, list: SymbolId) -> bool {
    let Some(writes) = index.member_writes.get(&list) else {
        return false;
    };
    let mut getters = writes
        .iter()
        .filter(|write| target_key(&write.left) == Some("get"))
        .filter_map(|write| index.function_of(&write.right));
    let (Some(getter), None) = (getters.next(), getters.next()) else {
        return false;
    };
    let mut returns = Vec::new();
    for statement in index.fns[getter].effective_statements() {
        match statement {
            Statement::ExpressionStatement(statement)
                if matches!(statement.expression.without_parentheses(), Expression::StringLiteral(_)) => {}
            Statement::ReturnStatement(statement) => returns.push(statement),
            _ => return false,
        }
    }
    let [statement] = returns.as_slice() else {
        return false;
    };
    let Some(argument) = statement.argument.as_ref().map(Expression::without_parentheses) else {
        return false;
    };
    let Expression::BinaryExpression(binary) = argument else {
        return false;
    };
    if binary.operator != BinaryOperator::StrictEquality {
        return false;
    }
    let chain = match (is_void0(&binary.left), is_void0(&binary.right)) {
        (true, false) => binary.right.without_parentheses(),
        (false, true) => binary.left.without_parentheses(),
        _ => return false,
    };
    this_member_depth(chain) == Some(2)
}

/// Member reads from `this`: `this` is 0, `this.a` is 1, `this.a.b` is 2.
fn this_member_depth(expression: &Expression<'_>) -> Option<usize> {
    let mut depth = 0;
    let mut current = expression.without_parentheses();
    loop {
        if matches!(current, Expression::ThisExpression(_)) {
            return Some(depth);
        }
        let (object, _) = member(current)?;
        depth += 1;
        current = object.without_parentheses();
    }
}

/// Null, or a byte list whose eight little-endian bytes decode to a NaN. The
/// literal bytes are decoded rather than compared, so no constant is read.
fn claim_nan_bytes(index: &Index<'_>, variables: &[SymbolId], claims: &mut Claims) {
    let is_null = |value: &Expression<'_>| matches!(value.without_parentheses(), Expression::NullLiteral(_));
    let mut found = Vec::new();
    for symbol in variables {
        let Some([first, second]) = index.writes.get(symbol).map(Vec::as_slice) else {
            continue;
        };
        let array = match (is_null(first), is_null(second)) {
            (true, false) => second,
            (false, true) => first,
            _ => continue,
        };
        let Some(items) = elements(array) else {
            continue;
        };
        let mut bytes: BTreeMap<usize, f64> = BTreeMap::new();
        for (at, item) in items.iter().enumerate() {
            if let Some(value) = numeric_value(item.without_parentheses()) {
                bytes.insert(at, value);
            }
        }
        for write in index.member_writes.get(symbol).into_iter().flatten() {
            let AssignmentTarget::ComputedMemberExpression(target) = &write.left else {
                continue;
            };
            if let (Some(at), Some(value)) = (
                numeric_value(target.expression.without_parentheses()),
                numeric_value(write.right.without_parentheses()),
            ) {
                bytes.insert(at as usize, value);
            }
        }
        if !bytes.keys().copied().eq(0..8) {
            continue;
        }
        let mut raw = [0u8; 8];
        let mut in_range = true;
        for (at, value) in &bytes {
            in_range &= value.fract() == 0.0 && (0.0..=255.0).contains(value);
            raw[*at] = *value as u8;
        }
        if in_range && f64::from_le_bytes(raw).is_nan() {
            found.push(*symbol);
        }
    }
    claims.one(NAN_BYTES, &found, "null, or eight literal little-endian bytes that decode to a float64 NaN");
}

/// Collector bindings re-bound to a `{ ctor: self, ... }` descriptor.
fn claim_collectors(index: &Index<'_>, variables: &[SymbolId], claims: &mut Claims) {
    let mut dom = Vec::new();
    let mut product = Vec::new();
    for symbol in variables {
        let Some([first, second]) = index.writes.get(symbol).map(Vec::as_slice) else {
            continue;
        };
        let wraps_self = object_value(second, "ctor").is_some_and(|value| index.symbol(value) == Some(*symbol));
        let Expression::CallExpression(call) = first.without_parentheses() else {
            continue;
        };
        let Some(callback) = args(call).first().and_then(|value| index.function_of(value)).filter(|_| wraps_self) else {
            continue;
        };
        if index.calls_with_literal(callback, "querySelectorAll", |selector| selector == "*") {
            dom.push(*symbol);
        }
        if index.calls_with_literal(callback, "querySelector", |selector| selector == "title")
            && index.calls_with_literal(callback, "querySelector", |selector| selector.contains("addToCart"))
        {
            product.push(*symbol);
        }
    }
    claims.one(DOM_COLLECTOR, &dom, "collector walking querySelectorAll(\"*\"), re-bound to its { ctor } descriptor");
    claims.one(PRODUCT_COLLECTOR, &product, "collector reading the page title and add-to-cart button, re-bound to its { ctor } descriptor");
}

/// Descriptor reads, and the anchor accessors bound from them.
fn claim_descriptors(index: &Index<'_>, variables: &[SymbolId], claims: &mut Claims) {
    let mut anchors = Vec::new();
    let mut protos = Vec::new();
    for symbol in variables {
        let Some(Expression::CallExpression(call)) = index.single_value(*symbol).map(Expression::without_parentheses) else {
            continue;
        };
        let arguments = args(call);
        let ([target, key], true) = (
            arguments.as_slice(),
            index.is_global_path(&call.callee, "Object", &["getOwnPropertyDescriptor"]),
        ) else {
            continue;
        };
        if is_string(key, "__proto__") && index.is_global_path(target, "Object", &["prototype"]) {
            protos.push(*symbol);
        }
        let of_anchor = member(target).is_some_and(|(object, name)| {
            name == "prototype" && member(object).is_some_and(|(_, name)| name == "HTMLAnchorElement")
        });
        if of_anchor && is_string(key, "href") {
            anchors.push(*symbol);
        }
    }
    claims.one(PROTO_DESCRIPTOR, &protos, "holds Object.getOwnPropertyDescriptor(Object.prototype, \"__proto__\")");
    claims.one(ANCHOR_DESCRIPTOR, &anchors, "holds the HTMLAnchorElement.prototype `href` descriptor");
    let [descriptor] = anchors.as_slice() else {
        return;
    };
    let binds = |value: &Expression<'_>, field: &str| {
        matches!(value.without_parentheses(), Expression::CallExpression(call)
            if member(&call.callee).is_some_and(|(_, name)| name == "bind")
                && args(call).first().is_some_and(|argument| {
                    member(argument).is_some_and(|(object, name)| name == field && index.symbol(object) == Some(*descriptor))
                }))
    };
    let setters: Vec<SymbolId> = variables
        .iter()
        .copied()
        .filter(|symbol| index.writes.get(symbol).is_some_and(|writes| writes.iter().any(|value| binds(value, "set"))))
        .collect();
    claims.one(ANCHOR_SETTER, &setters, "bound from the anchor `href` descriptor's `set`, or a plain href-writing fallback");
    let accessors: Vec<SymbolId> = variables
        .iter()
        .copied()
        .filter(|symbol| index.member_writes.get(symbol).is_some_and(|writes| writes.iter().any(|write| binds(&write.right, "get"))))
        .collect();
    claims.one(ANCHOR_ACCESSORS, &accessors, "object receiving the bound `get` of the anchor `href` descriptor");
}

/// Serializer extension objects and the classes behind them.
fn claim_extensions(index: &Index<'_>, variables: &[SymbolId], claims: &mut Claims) {
    let mut registries = Vec::new();
    let mut fixed = Vec::new();
    let mut sentinel = Vec::new();
    let mut lazy = Vec::new();
    for symbol in variables {
        let Some(value) = index.single_value(*symbol) else {
            continue;
        };
        if let Expression::CallExpression(call) = value.without_parentheses() {
            if let Some(factory) = index.function_of(&call.callee) {
                if is_extension_registry_factory(index, factory) {
                    registries.push(*symbol);
                }
                if is_lazy_registry_factory(index, factory) {
                    lazy.push(*symbol);
                }
            }
            continue;
        }
        if !has_exact_keys(value, &["detector", "serialiser"]) {
            continue;
        }
        let deserialiser = index
            .member_writes
            .get(symbol)
            .and_then(|writes| writes.iter().find(|write| target_key(&write.left) == Some("deserialiser")));
        let (Some(detector), Some(serialiser), Some(deserialiser)) = (
            object_value(value, "detector").and_then(|value| index.function_of(value)),
            object_value(value, "serialiser").and_then(|value| index.function_of(value)),
            deserialiser.and_then(|write| index.function_of(&write.right)),
        ) else {
            continue;
        };
        let never_matches = index.only_result(detector).is_some_and(|value| is_bool(value, false));
        let throws = matches!(index.fns[serialiser].effective_statements(), [Statement::ThrowStatement(_)]);
        let matched = index.only_result(detector).and_then(|value| match value.without_parentheses() {
            Expression::BinaryExpression(binary)
                if binary.operator == BinaryOperator::StrictEquality && index.is_slot(&binary.left, detector, 0) =>
            {
                index.symbol(&binary.right)
            }
            _ => None,
        });
        if never_matches && throws {
            fixed.push(*symbol);
        } else if matched.is_some() && matched == index.only_result(deserialiser).and_then(|value| index.symbol(value)) {
            sentinel.push(*symbol);
        }
    }
    claims.one(EXTENSION_REGISTRY, &registries, "result of a factory building a class that stores detector/serialiser/deserialiser records");
    claims.one(FIXED_EXTENSION, &fixed, "extension whose detector is always false and whose serialiser throws");
    claims.one(SENTINEL_EXTENSION, &sentinel, "extension detecting one value by identity and deserialising back to it");
    claims.one(LAZY_REGISTRY_CLASS, &lazy, "subclass whose `register` first calls a no-argument function, then the base register");
}

fn is_extension_registry_factory(index: &Index<'_>, factory: usize) -> bool {
    index.subtree(factory).into_iter().any(|entry| {
        index.own[entry].assignments.iter().any(|assignment| {
            matches!(&assignment.left, AssignmentTarget::ComputedMemberExpression(target)
                if member(&target.object).is_some_and(|(_, name)| name == "extensions"))
                && has_exact_keys(&assignment.right, &["detector", "serialiser", "deserialiser"])
        })
    })
}

/// A class factory whose `register` calls a function, then the base `register`
/// (`super.register(...)`, or `base.prototype.register.call(this, ...)`).
fn is_lazy_registry_factory(index: &Index<'_>, factory: usize) -> bool {
    index.own[factory].nested.iter().any(|method| {
        let [first, second, ..] = index.fns[*method].statements() else {
            return false;
        };
        let installs = statement_call(first).is_some_and(|call| call.arguments.is_empty() && index.symbol(&call.callee).is_some());
        let forwards = statement_call(second).is_some_and(|call| {
            member(&call.callee).is_some_and(|(object, name)| match name {
                "register" => matches!(object, Expression::Super(_)),
                "call" => member(object).is_some_and(|(prototype, name)| {
                    name == "register" && member(prototype).is_some_and(|(_, name)| name == "prototype")
                }),
                _ => false,
            })
        });
        installs && forwards
    })
}
