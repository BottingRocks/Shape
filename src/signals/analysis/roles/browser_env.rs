//! Package `p10_browser_env`: browser-environment fingerprint and probe
//! collection.
//!
//! The package covers four families, each proven from its own structure:
//!
//! * native-function and prototype tamper checks — the identity test against
//!   `Function.prototype.toString`, the `Proxy` handler with
//!   `getPrototypeOf`/`has`/`get` traps, the `toString` accessor probe and the
//!   regex that recognizes native code;
//! * media-feature measurement — the `matchMedia` string builders, the value
//!   mapper, the fraction measurement with its asynchronous result/error
//!   delivery and its bracket scheduler;
//! * the deferred-result pair behind the WebRTC ICE gatherer — a success
//!   settle that replays the pending waiters and an error settle that also
//!   raises the failure flag, plus the arrays and the cached argument they
//!   schedule;
//! * the aggregate environment collectors — one group body per subsystem,
//!   recognized by the host APIs its own callbacks use, and the callbacks
//!   themselves by the registrar slot of the collector ABI they are handed to.
//!
//! # How this package decides
//!
//! Recognition reads a function's own statements and the inline callbacks it
//! hands to calls; a group body additionally owns the callbacks it registers.
//! Host API names (`matchMedia`, `getPrototypeOf`, `setPrototypeOf`,
//! `createDataChannel`, `deviceorientation`, `touchstart`, `insertFromPaste`,
//! `native code`, `(max-`) are stable across captures and are the only
//! literals used as evidence. No span, offset, generated spelling, ordinal or
//! build constant is read.
//!
//! Both tracer shapes are read: a formal and `arguments[N]` are one slot
//! ([`ParameterSlots`]), `name.method(...)` and `name["method"].call(name, …)`
//! are one call, callback arguments may be inline arrows or hoisted
//! declarations, and `obj["key"]` is the same property as `obj.key`.
//!
//! Every role names one entity. When a predicate matches more than one
//! function or binding the role is dropped whole ([`Out::unique_function`]),
//! so the plan never receives two candidates for one role; a role this package
//! cannot separate structurally is left unnamed rather than picked by source
//! order.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

use crate::signals::extract::bindings::{
    array_elements, numeric_index, object_members_of, static_member_name, ParameterSlots,
};
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

/// Every role this package can claim, with the one name it receives.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    // Native-function and prototype tamper checks.
    function("is_native_function", "isNativeFunction"),
    alias("native_function_check", "nativeFunctionCheck"),
    function("prototype_behavior_probe", "hasGenuinePrototypeBehavior"),
    alias("native_object_check", "nativeObjectCheck"),
    function("proxy_get_prototype_of_trap", "proxyGetPrototypeOfTrap"),
    function("proxy_has_trap", "proxyHasTrap"),
    function("proxy_get_trap", "proxyGetTrap"),
    function("tostring_probe_sentinel", "toStringTamperSentinel"),
    function("tostring_probe_getter", "toStringGetterStub"),
    function("non_native_function_predicate", "isNonNativeFunction"),
    function("descriptor_tamper_scan", "scanPropertyDescriptorTamper"),
    function("native_global_scan", "countNativeGlobalApi"),
    binding("native_code_pattern", "nativeCodeRegex"),
    binding("get_prototype_of", "getPrototypeOf"),
    binding("set_prototype_of", "setPrototypeOf"),
    binding("bound_function_source_text", "boundFunctionSourceText"),
    binding("descriptor_function_source", "descriptorFunctionSourceText"),
    binding("function_source_stringifier", "stringifyFunctionSource"),
    // Media-feature measurement.
    function("measure_max_media_feature", "measureMaxMediaFeatureValue"),
    function("evaluate_media_feature_values", "evaluateMediaFeatureValues"),
    function("media_feature_match_probe", "matchesMediaFeatureValue"),
    function("measure_ratio_media_feature", "measureRatioMediaFeatureFraction"),
    function("deliver_ratio_result", "deliverRatioResultAsync"),
    function("notify_ratio_error", "notifyErrorAsync"),
    function("ratio_result_callback", "invokeRatioResultCallback"),
    function("ratio_error_callback", "invokeRatioErrorCallback"),
    function("ratio_bracket_step", "narrowRatioBracket"),
    // Deferred result behind the WebRTC ICE gatherer.
    function("resolve_pending_result", "resolveDeferredResult"),
    function("reject_pending_result", "rejectDeferredResult"),
    function("dispatch_pending_waiter", "schedulePendingResultWaiter"),
    function("dispatch_error_waiter", "scheduleErrorResultWaiter"),
    binding("pending_result_callbacks", "pendingResultCallbacks"),
    binding("deferred_error_callbacks", "iceErrorWaiters"),
    binding("deferred_callback_argument", "deferredCallbackArgument"),
    binding("ice_gatherer_holder", "iceCandidateCollector"),
    // Aggregate environment collectors.
    function("location_tamper_collector", "collectLocationWebGLTamperSignals"),
    function("browser_environment_collector", "collectBrowserEnvironmentSignals"),
    function("device_orientation_collector", "collectDeviceOrientationAndCanvasFingerprint"),
    function("input_automation_collector", "collectInputAndAutomationProbeSignals"),
    function("touch_environment_collector", "collectEnvironmentAndTouchSignals"),
    function("timing_environment_collector", "timingAndEnvironmentCollector"),
    function("pointer_native_api_collector", "collectPointerAndNativeApiSignals"),
    function("environment_encoding_collector", "emitEncodedEnvironmentSignal"),
    function("location_tamper_init", "initializeLocationTamperProbe"),
    function("keyboard_probe_init", "initializeKeyboardProbes"),
    function("device_orientation_init", "initializeDeviceOrientationProbe"),
    function("input_automation_last_chance", "flushInputAutomationProbe"),
    function("touch_probe_init", "initializeTouchProbes"),
    function("native_type_error_probe", "triggerNativeTypeErrorProbe"),
    function("touch_records_last_chance", "flushTouchRecords"),
    function("timing_environment_emit", "emitTimingEnvironmentSignal"),
    function("pointer_probe_init", "initializePointerAndWebRtcProbes"),
    function("constructor_probe_table_replay", "replayHostConstructorProbeTable"),
    function("native_global_census_emit", "emitNativeGlobalApiCensus"),
    function("native_global_count_emit", "emitNativeGlobalApiCount"),
    function("client_hints_then", "encodeUserAgentClientHints"),
    function("path_walk_probe", "walkObjectPathAndRecord"),
    function("path_walk_operator", "offsetEncodedValue"),
    function("seed_stream_subscribe", "subscribeSeedStream"),
    function("seed_stream_next", "emitSeededHashEncoding"),
    function("seed_stream_error", "forwardSeedStreamError"),
    function("forward_scan_error", "forwardSortedNameError"),
    function("emit_sorted_name_hash", "emitSortedNameHash"),
    function("webauthentication_capability_bits", "emitWebAuthenticationCapabilityBits"),
    function("native_global_scan_error", "forwardNativeGlobalScanError"),
    function("seed_snapshot_last_chance", "reemitSeededSnapshot"),
    function("intersection_observer_stub", "stubIntersectionObserver"),
    function("mutation_observer_stub", "stubMutationObserver"),
    function("performance_observer_stub", "stubPerformanceObserver"),
    binding("automation_global_names", "globalAutomationProbeNames"),
    binding("automation_document_property_names", "documentAutomationProbeNames"),
    binding("automation_global_markers", "globalAutomationMarkerNames"),
    binding("automation_document_markers", "documentAutomationMarkerNames"),
    binding("automation_body_markers", "bodyAutomationMarkerNames"),
    binding("automation_navigator_markers", "navigatorAutomationMarkerNames"),
    binding("automation_crypto_markers", "cryptoAutomationMarkerNames"),
    binding("automation_external_markers", "externalAutomationMarkerNames"),
    binding("browser_global_probe_names", "browserGlobalProbeNames"),
    binding("font_probe_config", "fontProbeConfig"),
    binding("location_field_codes", "locationFieldCodeTable"),
    binding("payload_date_millis", "encodedDateMilliseconds"),
    binding("observable_factory_binder", "observableFactory"),
    binding("throttle_factory_alias", "throttleWrapper"),
    binding("own_property_checker", "ownPropertyChecker"),
    binding("property_descriptor_lookup", "propertyDescriptorLookup"),
    binding("prototype_lookup", "prototypeLookup"),
    binding("object_tag_reader", "objectTagReader"),
];

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
    params: usize,
    is_arrow: bool,
}

/// What a function owns directly, excluding nested functions.
#[allow(dead_code)]
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    reads: BTreeSet<SymbolId>,
    writes: BTreeSet<SymbolId>,
    members: BTreeSet<String>,
    strings: BTreeSet<String>,
    ifs: Vec<&'a IfStatement<'a>>,
    /// `obj.key = value` writes: (object symbol, key, value).
    member_writes: Vec<(Option<SymbolId>, String, &'a Expression<'a>)>,
    updates: usize,
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
    assigns: BTreeMap<SymbolId, Vec<&'a AssignmentExpression<'a>>>,
    /// `delete expr` sites with the function that owns them.
    deletes: Vec<(usize, &'a Expression<'a>)>,
    /// Functions that are this program's callable inspectors: a nested function
    /// that deletes its slot 0's `prototype` and tests a native-source regex.
    probe_inspectors: BTreeSet<usize>,
    members_sub: Vec<BTreeSet<String>>,
    strings_sub: Vec<BTreeSet<String>>,
}

fn strip<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    expression.without_parentheses()
}

fn member<'e, 'a>(expression: &'e Expression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
    let name = static_member_name(strip(expression))?;
    match strip(expression) {
        Expression::StaticMemberExpression(found) => Some((&found.object, name)),
        Expression::ComputedMemberExpression(found) => Some((&found.object, name)),
        _ => None,
    }
}

fn args<'e, 'a>(call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
    call.arguments.iter().filter_map(Argument::as_expression).collect()
}

fn new_args<'e, 'a>(call: &'e NewExpression<'a>) -> Vec<&'e Expression<'a>> {
    call.arguments.iter().filter_map(Argument::as_expression).collect()
}

fn string_value<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    match strip(expression) {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn bool_value(expression: &Expression<'_>) -> Option<bool> {
    match strip(expression) {
        Expression::BooleanLiteral(literal) => Some(literal.value),
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

/// The observer constructor a probe-table row may name. No other member name
/// is role evidence, so an unrelated empty callback never receives one of the
/// observer names.
fn observer_constructor(name: &str) -> Option<&'static str> {
    match name {
        "IntersectionObserver" => Some("IntersectionObserver"),
        "MutationObserver" => Some("MutationObserver"),
        "PerformanceObserver" => Some("PerformanceObserver"),
        _ => None,
    }
}

/// The numeric member slot an `object[slot] = value` target writes.
fn target_numeric_index(target: &AssignmentTarget<'_>) -> Option<u32> {
    match target {
        AssignmentTarget::ComputedMemberExpression(found) => numeric_index(&found.expression),
        _ => None,
    }
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
            assigns: BTreeMap::new(),
            deletes: Vec::new(),
            probe_inspectors: BTreeSet::new(),
            members_sub: Vec::new(),
            strings_sub: Vec::new(),
        };
        ix.index();
        ix.probe_inspectors = ix.collect_probe_inspectors();
        ix.aggregate();
        ix
    }

    /// The functions that only a probe registrar owns: they delete the
    /// `prototype` of their slot 0 and test a native-source pattern. The parent
    /// that hands such an inspector on is p09's probe registrar, so this
    /// package leaves a merged function to it instead of claiming a collector
    /// role on the same binding (see [`collector_kind`]).
    fn collect_probe_inspectors(&self) -> BTreeSet<usize> {
        let mut inspectors = BTreeSet::new();
        for (owner, target) in &self.deletes {
            let Some((object, name)) = member(target) else {
                continue;
            };
            if name != "prototype" || self.slot_index(object, *owner) != Some(0) {
                continue;
            }
            if self.own[*owner]
                .calls
                .iter()
                .any(|call| self.method(call).is_some_and(|(_, name)| name == "test"))
            {
                inspectors.insert(*owner);
            }
        }
        inspectors
    }

    /// True when `function` owns a callable inspector anywhere in its subtree.
    fn owns_probe_inspector(&self, function: usize) -> bool {
        self.probe_inspectors.iter().any(|inspector| self.within(*inspector, function))
    }

    /// True when `function`'s WebGL proof is a canvas collection closure: the
    /// function that acquires the WebGL context also enqueues two or more
    /// canvas tasks on a queue captured from its parent. That is p13's canvas
    /// feature collector, so this package leaves the merged function to it.
    fn webgl_proof_is_canvas_collection(&self, function: usize) -> bool {
        let mut found = false;
        for index in 0..self.fns.len() {
            if !self.within(index, function) {
                continue;
            }
            let gets_webgl = self.own[index].calls.iter().any(|call| {
                self.method(call).is_some_and(|(_, name)| name == "getContext")
                    && args(call).first().and_then(|argument| string_value(argument)).is_some_and(
                        |kind| kind.starts_with("webgl") || kind.starts_with("experimental-webgl"),
                    )
            });
            if !gets_webgl {
                continue;
            }
            found = true;
            let body = self.fns[index].span;
            let enqueued = self.own[index]
                .calls
                .iter()
                .filter(|call| self.method(call).is_some_and(|(_, name)| name == "enqueue"))
                .filter(|call| {
                    self.method(call)
                        .and_then(|(receiver, _)| self.symbol(receiver))
                        .is_some_and(|receiver| {
                            let declaration =
                                self.nodes
                                    .get_node(self.scoping.symbol_declaration(receiver))
                                    .span();
                            declaration.start < body.start || declaration.end > body.end
                        })
                })
                .count();
            if enqueued < 2 {
                return false;
            }
        }
        found
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
            let func = match node.kind() {
                AstKind::Function(found) => {
                    Some((found.span, FnNode::Function(found), found.params.items.len()))
                }
                AstKind::ArrowFunctionExpression(found) => {
                    Some((found.span, FnNode::Arrow(found), found.params.items.len()))
                }
                _ => None,
            };
            if let Some((span, func, params)) = func {
                let index = self.fns.len();
                let is_arrow = matches!(func, FnNode::Arrow(_));
                self.fns.push(FnEntry { node: id, span, func, parent: owner, params, is_arrow });
                self.own.push(Own::default());
                self.fn_of_node.insert(id, index);
                if let Some(owner) = owner {
                    self.own[owner].nested.push(index);
                }
                if let FnNode::Function(found) = func {
                    if let Some(symbol) = found.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        self.decl_fn.insert(symbol, index);
                    }
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
        if let AstKind::AssignmentExpression(assignment) = kind {
            if assignment.operator == AssignmentOperator::Assign {
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                        self.assigns.entry(symbol).or_default().push(assignment);
                    }
                }
                if let Some(owner) = owner {
                    self.own[owner].assignments.push(assignment);
                    if let Some(name) = target_member_name(&assignment.left) {
                        let object = target_member_object(&assignment.left).and_then(|object| {
                            analysis::expression_symbol(strip(object), self.scoping)
                        });
                        self.own[owner].member_writes.push((
                            object,
                            name.to_string(),
                            &assignment.right,
                        ));
                    }
                }
            }
        }
        let Some(owner) = owner else {
            return;
        };
        if let AstKind::UnaryExpression(unary) = kind {
            if unary.operator == UnaryOperator::Delete {
                self.deletes.push((owner, &unary.argument));
            }
        }
        let scoping = self.scoping;
        let own = &mut self.own[owner];
        match kind {
            AstKind::CallExpression(call) => own.calls.push(call),
            AstKind::NewExpression(new) => own.news.push(new),
            AstKind::IfStatement(statement) => own.ifs.push(statement),
            AstKind::ReturnStatement(statement) => {
                if let Some(argument) = statement.argument.as_ref() {
                    own.returns.push(argument);
                }
            }
            AstKind::UpdateExpression(_) => own.updates += 1,
            AstKind::IdentifierReference(identifier) => {
                if let (Some(symbol), Some(reference)) = (
                    analysis::identifier_symbol(identifier, scoping),
                    identifier.reference_id.get(),
                ) {
                    if scoping.get_reference(reference).is_read() {
                        own.reads.insert(symbol);
                    } else if scoping.get_reference(reference).is_write() {
                        own.writes.insert(symbol);
                    }
                }
            }
            AstKind::StaticMemberExpression(found) => {
                own.members.insert(found.property.name.to_string());
                if let Expression::Identifier(object) = strip(&found.object) {
                    if found.property.name == "length" {
                        if let Some(symbol) = analysis::identifier_symbol(object, scoping) {
                            own.reads.insert(symbol);
                        }
                    }
                }
            }
            AstKind::ComputedMemberExpression(found) => {
                if let Expression::StringLiteral(literal) = &found.expression {
                    own.members.insert(literal.value.to_string());
                    if literal.value == "length" {
                        if let Expression::Identifier(object) = strip(&found.object) {
                            if let Some(symbol) = analysis::identifier_symbol(object, scoping) {
                                own.reads.insert(symbol);
                            }
                        }
                    }
                }
            }
            AstKind::StringLiteral(literal) => {
                own.strings.insert(literal.value.to_string());
            }
            _ => {}
        }
    }

    /// Union each function's own names with those of the functions nested in
    /// it. Children always have a larger node index than their parent, so a
    /// reverse sweep is enough.
    fn aggregate(&mut self) {
        self.members_sub = self.own.iter().map(|own| own.members.clone()).collect();
        self.strings_sub = self.own.iter().map(|own| own.strings.clone()).collect();
        for index in (0..self.fns.len()).rev() {
            let children: Vec<usize> = self.own[index].nested.clone();
            for child in children {
                let members = self.members_sub[child].clone();
                self.members_sub[index].extend(members);
                let strings = self.strings_sub[child].clone();
                self.strings_sub[index].extend(strings);
            }
        }
    }

    fn within(&self, inner: usize, outer: usize) -> bool {
        let mut current = Some(inner);
        while let Some(index) = current {
            if index == outer {
                return true;
            }
            current = self.fns[index].parent;
        }
        false
    }

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.scoping)
    }

    /// `Object.prototype.toString`-style path rooted at an unbound global.
    fn path(&self, expression: &Expression<'a>) -> Option<String> {
        match strip(expression) {
            Expression::Identifier(identifier) => (self.symbol(expression).is_none())
                .then(|| identifier.name.to_string()),
            _ => {
                let (object, name) = member(expression)?;
                Some(format!("{}.{name}", self.path(object)?))
            }
        }
    }

    fn slot_index(&self, expression: &Expression<'a>, function: usize) -> Option<u32> {
        let slot = self.slots.incoming_slot(strip(expression))?;
        (slot.owner == self.fns[function].node).then_some(slot.index)
    }

    fn is_incoming(&self, expression: &Expression<'a>) -> bool {
        self.slots.incoming_slot(strip(expression)).is_some()
    }

    /// The parameter slot an expression denotes when it is a local of
    /// `function` that only ever holds that slot (the emitted code copies a
    /// formal into a branch-joined local before handing it on).
    fn slot_value(&self, expression: &Expression<'a>, function: usize) -> Option<u32> {
        if let Some(index) = self.slot_index(expression, function) {
            return Some(index);
        }
        let symbol = self.symbol(expression)?;
        let declaration = self.nodes.get_node(self.scoping.symbol_declaration(symbol)).span();
        let body = self.fns[function].span;
        if declaration.start < body.start || declaration.end > body.end {
            return None;
        }
        let origins = self.origins(symbol)?;
        let mut slot = None;
        for origin in origins {
            let found = self.slot_index(origin, function)?;
            if slot.is_some_and(|slot| slot != found) {
                return None;
            }
            slot = Some(found);
        }
        slot
    }

    /// Every source value of a binding, or `None` when it has a write that is
    /// not a declarator initializer or plain assignment.
    fn origins(&self, symbol: SymbolId) -> Option<Vec<&'a Expression<'a>>> {
        if self.scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        let assignments = self.assigns.get(&symbol).map(Vec::as_slice).unwrap_or(&[]);
        let writes = self
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count();
        if writes != assignments.len() {
            return None;
        }
        let mut out = Vec::new();
        if let AstKind::VariableDeclarator(declarator) =
            self.nodes.kind(self.scoping.symbol_declaration(symbol))
        {
            if let Some(init) = declarator.init.as_ref() {
                out.push(init);
            }
        }
        out.extend(assignments.iter().map(|assignment| &assignment.right));
        Some(out)
    }

    fn single_origin(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self.origins(symbol)?.as_slice() {
            [origin] => Some(origin),
            _ => None,
        }
    }

    fn resolve_fn(&self, expression: &Expression<'a>) -> Option<usize> {
        self.resolve_fn_depth(expression, 6)
    }

    fn resolve_fn_depth(&self, expression: &Expression<'a>, depth: u8) -> Option<usize> {
        match strip(expression) {
            Expression::FunctionExpression(found) => {
                return self.fn_of_node.get(&found.node_id.get()).copied();
            }
            Expression::ArrowFunctionExpression(found) => {
                return self.fn_of_node.get(&found.node_id.get()).copied();
            }
            _ => {}
        }
        if depth == 0 {
            return None;
        }
        let symbol = self.symbol(expression)?;
        if let Some(index) = self.decl_fn.get(&symbol).copied() {
            let immutable = self.scoping.symbol_declarations(symbol).count() == 1
                && !self.scoping.get_resolved_references(symbol).any(|r| r.is_write());
            return immutable.then_some(index);
        }
        self.resolve_fn_depth(self.single_origin(symbol)?, depth - 1)
    }

    /// The functions that read `symbol` in their own body.
    fn readers(&self, symbol: SymbolId) -> Vec<usize> {
        (0..self.fns.len()).filter(|index| self.own[*index].reads.contains(&symbol)).collect()
    }

    /// True when `symbol` is declared outside the function `function`.
    fn is_outer(&self, symbol: SymbolId, function: usize) -> bool {
        let declaration = self.nodes.get_node(self.scoping.symbol_declaration(symbol)).span();
        let body = self.fns[function].span;
        declaration.start < body.start || declaration.end > body.end
    }

    /// The callee and arguments of a call, seen through the target cohort's
    /// `callee.call(receiver, ...)` form. A plain binding's `.call` stays as
    /// written: only a member or slot callee is forwarded.
    fn invocation<'e>(
        &self,
        call: &'e CallExpression<'a>,
    ) -> (&'e Expression<'a>, Vec<&'e Expression<'a>>) {
        let arguments = args(call);
        if let Some((inner, "call")) = member(&call.callee) {
            if !arguments.is_empty() && (member(inner).is_some() || self.is_incoming(inner)) {
                return (inner, arguments[1..].to_vec());
            }
        }
        (&call.callee, arguments)
    }

    /// The method name of a `receiver.name(...)` call and its receiver.
    fn method<'e>(&self, call: &'e CallExpression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
        member(self.invocation(call).0)
    }

    /// True when the call is a plain global call such as `setTimeout(...)`.
    fn is_global_call(&self, call: &CallExpression<'a>, name: &str) -> bool {
        self.path(&call.callee).as_deref() == Some(name)
    }

    fn call_arguments<'e>(&self, call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
        self.invocation(call).1
    }

    /// The method name of a call, also when the method was extracted into an
    /// array (`[receiver, receiver.onEmpty][1].call(receiver, …)`).
    fn method_name(&self, call: &CallExpression<'a>) -> Option<String> {
        let (receiver, name) = member(&call.callee)?;
        if name != "call" {
            return Some(name.to_string());
        }
        // `<array>[i].call(receiver, …)`: the element holds the extracted
        // method (`[target, target.onEmpty][1].call(target, callback)`).
        if let Expression::ComputedMemberExpression(computed) = strip(receiver) {
            let index = numeric_index(&computed.expression)?;
            let symbol = self.symbol(&computed.object)?;
            let origin = self.single_origin(symbol)?;
            let elements = array_elements(origin)?;
            let element = elements.get(index as usize)?;
            return member(element).map(|(_, found)| found.to_string());
        }
        Some(name.to_string())
    }

    /// Calls handed to a parameter of `function`, with the registrar slot and
    /// the boolean second argument when one is written.
    fn registrations(&self, function: usize) -> Vec<Registration> {
        let mut out = Vec::new();
        for call in &self.own[function].calls {
            let Some(slot) = self.slot_index(&call.callee, function) else {
                continue;
            };
            let arguments = args(call);
            let Some(first) = arguments.first() else {
                continue;
            };
            let Some(callback) = self.resolve_fn(first) else {
                continue;
            };
            out.push(Registration {
                slot,
                callback,
                flag: arguments.get(1).and_then(|flag| bool_value(flag)),
            });
        }
        out
    }

    fn calls_to(&self, function: usize) -> Vec<(NodeId, &'a CallExpression<'a>)> {
        let mut out = Vec::new();
        for index in 0..self.fns.len() {
            for call in &self.own[index].calls {
                if self.resolve_fn(self.invocation(call).0) == Some(function) {
                    out.push((call.node_id.get(), *call));
                }
            }
        }
        out
    }

    /// True when the node sits under a `catch` clause or inside a promise
    /// `.catch(...)` callback.
    fn failure_context(&self, node: NodeId) -> bool {
        let mut current = node;
        for _ in 0..64 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return false;
            }
            match self.nodes.kind(parent) {
                AstKind::CatchClause(_) => return true,
                AstKind::CallExpression(call) => {
                    if self.method(call).is_some_and(|(_, name)| name == "catch") {
                        return true;
                    }
                }
                _ => {}
            }
            current = parent;
        }
        false
    }

    /// True when the function is an argument of a call that is itself an
    /// argument of the given method (`x.pipe(make(arrow))`).
    fn passed_through_method(&self, function: usize, name: &str) -> bool {
        let span = self.fns[function].span;
        for index in 0..self.fns.len() {
            for call in &self.own[index].calls {
                if self.method(call).is_none_or(|(_, found)| found != name) {
                    continue;
                }
                for argument in self.call_arguments(call) {
                    let argument = argument.span();
                    if argument.start <= span.start && argument.end >= span.end {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// True when the function node is handed to a call of the given method.
    fn passed_to_method(&self, function: usize, name: &str) -> bool {
        let mut current = self.fns[function].node;
        for _ in 0..4 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return false;
            }
            if let AstKind::CallExpression(call) = self.nodes.kind(parent) {
                return self.method(call).is_some_and(|(_, found)| found == name);
            }
            current = parent;
        }
        false
    }

    fn is_empty_body(&self, function: usize) -> bool {
        match self.fns[function].func {
            FnNode::Function(found) => {
                found.body.as_ref().is_some_and(|body| body.statements.is_empty())
            }
            FnNode::Arrow(found) => !found.expression && found.body.statements.is_empty(),
        }
    }

    fn span(&self, index: usize) -> Span {
        self.fns[index].span
    }

    /// True when the expression or, for a conditional/logical value, one of its
    /// branches is the given host path.
    fn origin_has_path(&self, expression: &Expression<'a>, path: &str, depth: u8) -> bool {
        if depth == 0 {
            return false;
        }
        if self.path(expression).as_deref() == Some(path) {
            return true;
        }
        match strip(expression) {
            Expression::ConditionalExpression(conditional) => {
                self.origin_has_path(&conditional.consequent, path, depth - 1)
                    || self.origin_has_path(&conditional.alternate, path, depth - 1)
            }
            Expression::LogicalExpression(logical) => {
                self.origin_has_path(&logical.left, path, depth - 1)
                    || self.origin_has_path(&logical.right, path, depth - 1)
            }
            _ => false,
        }
    }

    fn all_symbols(&self) -> Vec<SymbolId> {
        self.scoping.symbol_ids().collect()
    }

    /// Every `receiver.name(...)` call in the program with the given method.
    fn calls_with_method(&self, name: &str) -> Vec<&'a CallExpression<'a>> {
        let mut out = Vec::new();
        for index in 0..self.fns.len() {
            for call in &self.own[index].calls {
                if self.method(call).is_some_and(|(_, found)| found == name) {
                    out.push(*call);
                }
            }
        }
        out
    }

    /// The set of event names a function or any nested callback registers.
    fn add_listener_events(&self, function: usize) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for index in 0..self.fns.len() {
            if !self.within(index, function) {
                continue;
            }
            for call in &self.own[index].calls {
                if self.method(call).is_some_and(|(_, name)| name == "addEventListener") {
                    if let Some(event) = self.call_arguments(call).first().and_then(|a| string_value(a))
                    {
                        out.insert(event.to_string());
                    }
                }
            }
        }
        out
    }

    /// The `forEach` call over a main array literal of a settle function.
    fn for_each_call(&self, function: usize) -> Option<&'a CallExpression<'a>> {
        self.own[function].calls.iter().copied().find(|call| {
            self.method(call).is_some_and(|(_, name)| name == "forEach")
                && self
                    .method(call)
                    .and_then(|(receiver, _)| self.symbol(receiver))
                    .and_then(|symbol| self.single_origin(symbol))
                    .is_some_and(|origin| matches!(strip(origin), Expression::ArrayExpression(_)))
        })
    }

    fn for_each_callback(&self, function: usize) -> Option<usize> {
        let call = self.for_each_call(function)?;
        self.call_arguments(call).first().and_then(|value| self.resolve_fn(value))
    }

    /// The main array a settle function iterates with `forEach`.
    fn for_each_array(&self, function: usize) -> Vec<SymbolId> {
        let Some(call) = self.for_each_call(function) else {
            return Vec::new();
        };
        self.method(call)
            .and_then(|(receiver, _)| self.symbol(receiver))
            .into_iter()
            .collect()
    }

    /// Outer symbols the function writes with a plain `=` from its second
    /// formal slot.
    fn cached_values(&self, function: usize) -> BTreeSet<SymbolId> {
        let mut out = BTreeSet::new();
        for assignment in &self.own[function].assignments {
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                continue;
            };
            let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
                continue;
            };
            if self.is_outer(symbol, function) && self.slot_index(&assignment.right, function) == Some(1)
            {
                out.insert(symbol);
            }
        }
        out
    }

    /// True when the function caches its second argument, flips at least two
    /// outer booleans and schedules every waiter it iterates with the cached
    /// value through its first slot's `setTimeout`.
    fn is_deferred_settle(&self, function: usize) -> bool {
        let cached = self.cached_values(function);
        if cached.is_empty() {
            return false;
        }
        let mut booleans = BTreeSet::new();
        for assignment in &self.own[function].assignments {
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                continue;
            };
            let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
                continue;
            };
            if self.is_outer(symbol, function) && bool_value(&assignment.right).is_some() {
                booleans.insert(symbol);
            }
        }
        if booleans.len() < 2 {
            return false;
        }
        let Some(callback) = self.for_each_callback(function) else {
            return false;
        };
        self.own[callback].calls.iter().any(|call| {
            if self.method(call).is_none_or(|(_, name)| name != "setTimeout") {
                return false;
            }
            let arguments = self.call_arguments(call);
            arguments
                .first()
                .is_some_and(|waiter| self.slot_value(waiter, callback) == Some(0))
                && arguments.iter().any(|argument| {
                    self.symbol(argument).is_some_and(|symbol| cached.contains(&symbol))
                })
        })
    }

    /// The call argument of a `setTimeout(fn, …)` call the setter makes.
    fn delivered_arrow(&self, setter: usize) -> Option<usize> {
        let call = self.own[setter]
            .calls
            .iter()
            .find(|call| self.method(call).is_some_and(|(_, name)| name == "setTimeout"))?;
        let only = self.call_arguments(call).first().copied()?;
        self.resolve_fn(only)
    }

    /// A nested unscheduled-result setter: one `setTimeout` that hands the
    /// enclosing measurement's result callback an argument.
    fn is_result_setter(&self, setter: usize) -> bool {
        let Some(parent) = self.fns[setter].parent else {
            return false;
        };
        if self.own[setter].calls.len() != 1 {
            return false;
        }
        let Some(arrow) = self.delivered_arrow(setter) else {
            return false;
        };
        self.own[arrow]
            .calls
            .iter()
            .any(|call| self.slot_value(self.invocation(call).0, parent).is_some())
    }
}

struct Registration {
    slot: u32,
    callback: usize,
    flag: Option<bool>,
}

#[derive(Default)]
struct Out {
    claims: Vec<FinderClaim>,
    seen: BTreeSet<(&'static str, u32, u32)>,
}

impl Out {
    fn function(&mut self, ix: &Ix<'_, '_>, role: &'static str, index: usize, evidence: &str) {
        let span = ix.span(index);
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: span,
                alias_role: None,
                confidence: Confidence::Medium,
                evidence: vec![evidence.to_string()],
            });
        }
    }

    fn function_with_alias(
        &mut self,
        ix: &Ix<'_, '_>,
        role: &'static str,
        alias_role: &'static str,
        index: usize,
        evidence: &str,
    ) {
        let span = ix.span(index);
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: span,
                alias_role: Some(alias_role),
                confidence: Confidence::Medium,
                evidence: vec![evidence.to_string()],
            });
        }
    }

    fn unique_function(
        &mut self,
        ix: &Ix<'_, '_>,
        role: &'static str,
        candidates: &[usize],
        evidence: &str,
    ) {
        if let [only] = candidates {
            self.function(ix, role, *only, evidence);
        }
    }

    fn unique_function_with_alias(
        &mut self,
        ix: &Ix<'_, '_>,
        role: &'static str,
        alias_role: &'static str,
        candidates: &[usize],
        evidence: &str,
    ) {
        if let [only] = candidates {
            self.function_with_alias(ix, role, alias_role, *only, evidence);
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

    fn unique_binding(
        &mut self,
        ix: &Ix<'_, '_>,
        role: &'static str,
        candidates: &[SymbolId],
        evidence: &str,
    ) {
        if let [only] = candidates {
            self.binding(ix, role, *only, evidence);
        }
    }
}

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let slots = ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes);
    let ix = Ix::new(ctx, &slots);
    let mut out = Out::default();
    native_tamper(&ix, &mut out);
    media_features(&ix, &mut out);
    deferred_results(&ix, &mut out);
    collectors(&ix, &mut out);
    observable_probes(&ix, &mut out);
    exchange_adapters(&ix, &mut out);
    out.claims
}

// ===========================================================================
// Native-function and prototype tamper checks
// ===========================================================================

fn native_tamper(ix: &Ix<'_, '_>, out: &mut Out) {
    // The native-code pattern: a `RegExp(...)` whose text contains the stable
    // `native code` marker.
    let identity: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_native_identity_check(*index)).collect();
    out.unique_function_with_alias(
        ix,
        "is_native_function",
        "native_function_check",
        &identity,
        "tests `typeof f === \"function\"` and `f.toString === Function.prototype.toString`, then matches the function text against the native-code pattern",
    );
    if let [test] = identity.as_slice() {
        let used: Vec<SymbolId> = ix
            .all_symbols()
            .into_iter()
            .filter(|symbol| {
                ix.native_code_pattern(*symbol) && ix.own[*test].reads.contains(symbol)
            })
            .collect();
        out.unique_binding(
            ix,
            "native_code_pattern",
            &used,
            "RegExp over the stable `native code` marker, read by the native-function identity test",
        );
    }

    // The prototype-behavior probe and its Proxy handler.
    let probes: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.proxy_handler(*index).is_some()).collect();
    let mut alias_role = None;
    if let [probe] = probes.as_slice() {
        if let Some(handler) = ix.proxy_handler(*probe) {
            for (key, role) in [
                ("getPrototypeOf", "proxy_get_prototype_of_trap"),
                ("has", "proxy_has_trap"),
                ("get", "proxy_get_trap"),
            ] {
                let traps: Vec<usize> = ix
                    .handler_values(handler, key)
                    .into_iter()
                    .filter_map(|value| ix.resolve_fn(value))
                    .collect();
                out.unique_function(
                    ix,
                    role,
                    &traps,
                    "installed under the probe Proxy handler's own key; raises the probe's captured flag",
                );
            }
        }
        let reads: Vec<SymbolId> = ix.own[*probe].reads.iter().copied().collect();
        let with_path = |path: &str| -> Vec<SymbolId> {
            reads
                .iter()
                .copied()
                .filter(|symbol| {
                    ix.origins(*symbol).is_some_and(|origins| {
                        origins.iter().any(|origin| ix.origin_has_path(origin, path, 3))
                    })
                })
                .collect()
        };
        let get_prototype = with_path("Object.getPrototypeOf");
        let set_prototype = with_path("Object.setPrototypeOf");
        out.unique_binding(
            ix,
            "get_prototype_of",
            &get_prototype,
            "holds `Object.getPrototypeOf` (or the captured fallback) and is read by the prototype-behavior probe",
        );
        out.unique_binding(
            ix,
            "set_prototype_of",
            &set_prototype,
            "holds `Object.setPrototypeOf` (or the captured fallback) and is read by the prototype-behavior probe",
        );
        if !get_prototype.is_empty() && !set_prototype.is_empty() {
            alias_role = Some("native_object_check");
        }
    }
    if let [probe] = probes.as_slice() {
        let evidence = "creates a Proxy with a getPrototypeOf/has/get handler and re-parents its argument through captured prototype accessors, restoring it in `finally`";
        match alias_role {
            Some(alias_role) => {
                out.function_with_alias(ix, "prototype_behavior_probe", alias_role, *probe, evidence)
            }
            None => out.function(ix, "prototype_behavior_probe", *probe, evidence),
        }
    }

    // `Object.defineProperty(sentinel, "toString", { get })`.
    let mut sentinels: BTreeSet<SymbolId> = BTreeSet::new();
    let mut getters: Vec<usize> = Vec::new();
    for call in ix.calls_with_method("defineProperty") {
        if let Some((sentinel, getter)) = ix.tostring_probe_call(call) {
            sentinels.insert(sentinel);
            getters.push(getter);
        }
    }
    let sentinel_fns: Vec<usize> = sentinels
        .iter()
        .filter_map(|symbol| ix.decl_fn.get(symbol).copied())
        .collect();
    out.unique_function(
        ix,
        "tostring_probe_sentinel",
        &sentinel_fns,
        "empty function whose `toString` property is replaced by an accessor, then string-coerced and passed to the native `Function.prototype.toString`",
    );
    out.unique_function(
        ix,
        "tostring_probe_getter",
        &getters,
        "getter installed as the sentinel's `toString`: returns the native `Function.prototype.toString` and counts reads",
    );

    // The filter predicate over a native API list.
    let predicates: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_non_native_predicate(*index)).collect();
    out.unique_function(
        ix,
        "non_native_function_predicate",
        &predicates,
        "filter callback that negates the native-code pattern's test on a function's recorded source text",
    );

    let binders: Vec<SymbolId> =
        ix.all_symbols().into_iter().filter(|symbol| ix.is_tostring_binder(*symbol)).collect();
    let bound: Vec<SymbolId> = binders
        .iter()
        .copied()
        .filter(|symbol| predicates.iter().any(|predicate| ix.own[*predicate].reads.contains(symbol)))
        .collect();
    out.unique_binding(
        ix,
        "bound_function_source_text",
        &bound,
        "call-bound native `Function.prototype.toString`, read by the non-native filter predicate",
    );

    let tamper_scans: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_descriptor_tamper_scan(*index)).collect();
    out.unique_function(
        ix,
        "descriptor_tamper_scan",
        &tamper_scans,
        "walks an object's own property descriptors and records the functions whose source text is not native",
    );
    let descriptor_source: Vec<SymbolId> = binders
        .iter()
        .copied()
        .filter(|symbol| tamper_scans.iter().any(|scan| ix.own[*scan].reads.contains(symbol)))
        .collect();
    out.unique_binding(
        ix,
        "descriptor_function_source",
        &descriptor_source,
        "call-bound native `Function.prototype.toString`, read by the descriptor tamper scan",
    );

    let scans: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_native_global_scan(*index)).collect();
    out.unique_function(
        ix,
        "native_global_scan",
        &scans,
        "descriptor walker that counts function-valued own properties whose source text is native",
    );
    let census_source: Vec<SymbolId> = binders
        .iter()
        .copied()
        .filter(|symbol| scans.iter().any(|scan| ix.own[*scan].reads.contains(symbol)))
        .collect();
    out.unique_binding(
        ix,
        "function_source_stringifier",
        &census_source,
        "call-bound native `Function.prototype.toString`, read by the native global API census",
    );
}

impl<'c, 'a> Ix<'c, 'a> {
    fn is_native_identity_check(&self, function: usize) -> bool {
        let own = &self.own[function];
        if own.returns.len() != 2 || !own.ifs.iter().any(|statement| self.identity_test(&statement.test))
        {
            return false;
        }
        let Some(receiver) = own.returns.iter().find_map(|value| self.exec_receiver(value)) else {
            return false;
        };
        own.returns.iter().any(|value| bool_value(value) == Some(false))
            && self.native_code_pattern(receiver)
    }

    fn identity_test(&self, test: &Expression<'a>) -> bool {
        let Expression::LogicalExpression(logical) = strip(test) else {
            return false;
        };
        if !matches!(logical.operator, oxc_syntax::operator::LogicalOperator::And) {
            return false;
        }
        self.typeof_function_test(&logical.left) && self.native_tostring_identity(&logical.right)
    }

    fn typeof_function_test(&self, expression: &Expression<'a>) -> bool {
        let Expression::BinaryExpression(binary) = strip(expression) else {
            return false;
        };
        if binary.operator != BinaryOperator::StrictEquality {
            return false;
        }
        let (value, literal) = match (strip(&binary.left), strip(&binary.right)) {
            (Expression::UnaryExpression(unary), literal)
                if unary.operator == UnaryOperator::Typeof =>
            {
                (&unary.argument, literal)
            }
            (literal, Expression::UnaryExpression(unary))
                if unary.operator == UnaryOperator::Typeof =>
            {
                (&unary.argument, literal)
            }
            _ => return false,
        };
        string_value(literal) == Some("function") && self.is_incoming(value)
    }

    fn native_tostring_identity(&self, expression: &Expression<'a>) -> bool {
        let Expression::BinaryExpression(binary) = strip(expression) else {
            return false;
        };
        if binary.operator != BinaryOperator::StrictEquality
            && binary.operator != BinaryOperator::Equality
        {
            return false;
        }
        let sides = [&binary.left, &binary.right];
        sides.iter().any(|side| member(side).is_some_and(|(_, name)| name == "toString"))
            && sides
                .iter()
                .any(|side| self.path(side).as_deref() == Some("Function.prototype.toString"))
    }

    fn exec_receiver(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        let expression = strip(expression);
        if let Expression::CallExpression(call) = expression {
            if let Some((receiver, "exec")) = self.method(call) {
                return self.symbol(receiver);
            }
        }
        match expression {
            Expression::UnaryExpression(unary) => self.exec_receiver(&unary.argument),
            _ => None,
        }
    }

    fn native_code_pattern(&self, symbol: SymbolId) -> bool {
        let Some(origin) = self.single_origin(symbol) else {
            return false;
        };
        let Expression::CallExpression(call) = strip(origin) else {
            return false;
        };
        if self.path(&call.callee).as_deref() != Some("RegExp") {
            return false;
        }
        args(call)
            .iter()
            .filter_map(|argument| string_value(argument))
            .any(|text| text.contains("native") && text.contains("code"))
    }

    fn proxy_handler(&self, function: usize) -> Option<SymbolId> {
        for new in &self.own[function].news {
            if self.path(&new.callee).as_deref() != Some("Proxy") {
                continue;
            }
            let arguments = new_args(new);
            let Some(arguments) = arguments.get(1) else {
                continue;
            };
            let Some(symbol) = self.symbol(arguments) else {
                continue;
            };
            let keys = ["getPrototypeOf", "has", "get"]
                .iter()
                .filter(|key| !self.handler_values(symbol, key).is_empty())
                .count();
            if keys >= 2 {
                return Some(symbol);
            }
        }
        None
    }

    fn handler_values(&self, handler: SymbolId, key: &str) -> Vec<&'a Expression<'a>> {
        let mut out = Vec::new();
        if let Some(origin) = self.single_origin(handler) {
            if let Expression::ObjectExpression(object) = strip(origin) {
                for entry in object_members_of(object) {
                    if entry.name == key {
                        out.push(entry.value);
                    }
                }
            }
        }
        for index in 0..self.fns.len() {
            for (object, name, value) in &self.own[index].member_writes {
                if *object == Some(handler) && name == key {
                    out.push(*value);
                }
            }
        }
        out
    }

    fn tostring_probe_call(&self, call: &CallExpression<'a>) -> Option<(SymbolId, usize)> {
        let arguments = self.call_arguments(call);
        if arguments.len() != 3 || string_value(arguments[1]) != Some("toString") {
            return None;
        }
        let (descriptor, descriptor_symbol) = match strip(arguments[2]) {
            Expression::ObjectExpression(object) => (Some(object), None),
            expression => (None, self.symbol(expression)),
        };
        let mut getter = descriptor
            .and_then(|object| {
                object_members_of(object)
                    .into_iter()
                    .find(|entry| entry.name == "get")
                    .and_then(|entry| self.resolve_fn(entry.value))
            });
        if getter.is_none() {
            if let Some(symbol) = descriptor_symbol {
                getter = self
                    .handler_values(symbol, "get")
                    .into_iter()
                    .find_map(|value| self.resolve_fn(value));
            }
        }
        let sentinel = self.symbol(arguments[0])?;
        Some((sentinel, getter?))
    }

    fn is_non_native_predicate(&self, function: usize) -> bool {
        let own = &self.own[function];
        if own.returns.len() != 1 {
            return false;
        }
        let Expression::UnaryExpression(unary) = strip(own.returns[0]) else {
            return false;
        };
        if unary.operator != UnaryOperator::LogicalNot {
            return false;
        }
        let Expression::CallExpression(test) = strip(&unary.argument) else {
            return false;
        };
        let Some((receiver, "test")) = self.method(test) else {
            return false;
        };
        let Some(pattern) = self.symbol(receiver) else {
            return false;
        };
        if !self.native_code_pattern(pattern) {
            return false;
        }
        let arguments = self.call_arguments(test);
        let Some(only) = arguments.first() else {
            return false;
        };
        let Expression::CallExpression(inner) = strip(only) else {
            return false;
        };
        let Some(stringify) = self.symbol(&inner.callee) else {
            return false;
        };
        self.is_tostring_binder(stringify) && self.passed_to_method(function, "filter")
    }

    fn is_tostring_binder(&self, symbol: SymbolId) -> bool {
        let Some(origin) = self.single_origin(symbol) else {
            return false;
        };
        let Expression::CallExpression(call) = strip(origin) else {
            return false;
        };
        let Some((target, "bind")) = self.method(call) else {
            return false;
        };
        if !self.path(target).as_deref().is_some_and(|path| path.ends_with(".call")) {
            return false;
        }
        self.call_arguments(call).last().is_some_and(|argument| {
            self.path(argument).as_deref().is_some_and(|path| path == "Function.prototype.toString")
        })
    }

    fn is_descriptor_tamper_scan(&self, function: usize) -> bool {
        let own = &self.own[function];
        if !self.fns[function].is_arrow || self.fns[function].params != 1 {
            return false;
        }
        own.members.contains("getOwnPropertyDescriptor")
            && own.members.contains("test")
            && own.updates == 0
            && own.calls.iter().any(|call| self.method(call).is_some_and(|(_, name)| name == "push"))
    }

    fn is_native_global_scan(&self, function: usize) -> bool {
        let own = &self.own[function];
        if !self.fns[function].is_arrow || self.fns[function].params != 1 {
            return false;
        }
        own.members.contains("getOwnPropertyDescriptor")
            && own.members.contains("test")
            && own.updates > 0
    }
}

// ===========================================================================
// Media-feature measurement
// ===========================================================================

fn media_features(ix: &Ix<'_, '_>, out: &mut Out) {
    let max_measures: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_max_feature_measure(*index)).collect();
    out.unique_function(
        ix,
        "measure_max_media_feature",
        &max_measures,
        "builds `(<feature>: <value>)` media queries, doubles until one stops matching, binary-searches the smallest matching integer and returns it",
    );

    let evaluators: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_feature_value_mapper(*index)).collect();
    out.unique_function(
        ix,
        "evaluate_media_feature_values",
        &evaluators,
        "maps each candidate value to `matchMedia(\"(<feature>: <value>)\").matches`",
    );
    if let [evaluator] = evaluators.as_slice() {
        let mappers: Vec<usize> = ix
            .own[*evaluator]
            .calls
            .iter()
            .filter(|call| ix.method(call).is_some_and(|(_, name)| name == "map"))
            .flat_map(|call| ix.call_arguments(call).into_iter().filter_map(|value| ix.resolve_fn(value)))
            .collect();
        out.unique_function(
            ix,
            "media_feature_match_probe",
            &mappers,
            "map callback returning `matchMedia(...).matches` for one candidate value",
        );
    }

    let ratio_measures: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_ratio_feature_measure(*index)).collect();
    out.unique_function(
        ix,
        "measure_ratio_media_feature",
        &ratio_measures,
        "searches `(max-<feature>: <n>/<d>)` queries for the resolution boundary and hands the fraction to asynchronous result/error setters",
    );
    if let [ratio] = ratio_measures.as_slice() {
        let steps: Vec<usize> = ix
            .own[*ratio]
            .calls
            .iter()
            .filter(|call| ix.method(call).is_some_and(|(_, name)| name == "schedule"))
            .flat_map(|call| ix.call_arguments(call).into_iter().filter_map(|value| ix.resolve_fn(value)))
            .collect();
        out.unique_function(
            ix,
            "ratio_bracket_step",
            &steps,
            "scheduler callback that narrows the fraction bracket and re-probes `matchMedia`",
        );

        let setters: Vec<usize> = ix.own[*ratio]
            .nested
            .iter()
            .copied()
            .filter(|setter| ix.is_result_setter(*setter))
            .collect();
        let mut results = Vec::new();
        let mut errors = Vec::new();
        for setter in setters {
            let fails = ix
                .calls_to(setter)
                .iter()
                .any(|(node, _)| ix.failure_context(*node));
            if fails {
                errors.push(setter);
            } else {
                results.push(setter);
            }
        }
        out.unique_function(
            ix,
            "deliver_ratio_result",
            &results,
            "schedules the measurement's result callback with the settled fraction",
        );
        out.unique_function(
            ix,
            "notify_ratio_error",
            &errors,
            "schedules the measurement's error callback; every call site hands it the value of a `catch`",
        );
        if let [result] = results.as_slice() {
            if let Some(arrow) = ix.delivered_arrow(*result) {
                out.function(
                    ix,
                    "ratio_result_callback",
                    arrow,
                    "timer callback that invokes the enclosing measurement's result callback",
                );
            }
        }
        if let [error] = errors.as_slice() {
            if let Some(arrow) = ix.delivered_arrow(*error) {
                out.function(
                    ix,
                    "ratio_error_callback",
                    arrow,
                    "timer callback that invokes the enclosing measurement's error callback",
                );
            }
        }
    }
}

impl<'c, 'a> Ix<'c, 'a> {
    fn is_max_feature_measure(&self, function: usize) -> bool {
        let own = &self.own[function];
        own.members.contains("matchMedia")
            && own.members.contains("matches")
            && own.calls.iter().any(|call| self.method(call).is_some_and(|(_, name)| name == "floor"))
            && !own.returns.is_empty()
    }

    fn is_feature_value_mapper(&self, function: usize) -> bool {
        let own = &self.own[function];
        if own.returns.len() != 1 {
            return false;
        }
        let Some(call) = own.calls.iter().find(|call| {
            self.method(call).is_some_and(|(_, name)| name == "map")
        }) else {
            return false;
        };
        let callbacks: Vec<usize> = self
            .call_arguments(call)
            .into_iter()
            .filter_map(|value| self.resolve_fn(value))
            .collect();
        !callbacks.is_empty()
            && callbacks.iter().all(|callback| {
                self.own[*callback].members.contains("matchMedia")
                    && self.own[*callback].members.contains("matches")
            })
    }

    fn is_ratio_feature_measure(&self, function: usize) -> bool {
        let own = &self.own[function];
        own.members.contains("matchMedia")
            && self.strings_sub[function].iter().any(|text| text.starts_with("(max-"))
            && own.calls.iter().any(|call| self.method(call).is_some_and(|(_, name)| name == "schedule"))
    }
}

// ===========================================================================
// Deferred result behind the WebRTC ICE gatherer
// ===========================================================================

fn deferred_results(ix: &Ix<'_, '_>, out: &mut Out) {
    let settles: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_deferred_settle(*index)).collect();
    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for settle in settles {
        let fails = ix
            .calls_to(settle)
            .iter()
            .any(|(node, _)| ix.failure_context(*node));
        if fails {
            failures.push(settle);
        } else {
            successes.push(settle);
        }
    }
    out.unique_function(
        ix,
        "resolve_pending_result",
        &successes,
        "caches its value, marks the result ready, clears the in-flight flag and schedules every pending waiter with the cached value",
    );
    out.unique_function(
        ix,
        "reject_pending_result",
        &failures,
        "the same settle plus a failure flag; every call site hands it the value of a `catch` or a promise rejection",
    );
    if let [success] = successes.as_slice() {
        let arrays = ix.for_each_array(*success);
        out.unique_binding(
            ix,
            "pending_result_callbacks",
            &arrays,
            "array whose `forEach` the success settle replays through `setTimeout`",
        );
        if let Some(callback) = ix.for_each_callback(*success) {
            out.function(
                ix,
                "dispatch_pending_waiter",
                callback,
                "forEach callback that schedules one retained waiter with the cached value",
            );
        }
    }
    if let [failure] = failures.as_slice() {
        let arrays = ix.for_each_array(*failure);
        out.unique_binding(
            ix,
            "deferred_error_callbacks",
            &arrays,
            "array whose `forEach` the error settle replays through `setTimeout`",
        );
        if let Some(callback) = ix.for_each_callback(*failure) {
            out.function(
                ix,
                "dispatch_error_waiter",
                callback,
                "forEach callback that schedules one retained error waiter with the cached value",
            );
        }
    }
    if let ([success], [failure]) = (successes.as_slice(), failures.as_slice()) {
        let cached = ix.cached_values(*success);
        let read_by_scheduler = |symbol: SymbolId, settle: usize| {
            let Some(callback) = ix.for_each_callback(settle) else {
                return false;
            };
            (0..ix.fns.len())
                .any(|index| ix.within(index, callback) && ix.own[index].reads.contains(&symbol))
        };
        let shared: Vec<SymbolId> = cached
            .into_iter()
            .filter(|symbol| {
                ix.cached_values(*failure).contains(symbol)
                    && read_by_scheduler(*symbol, *success)
                    && read_by_scheduler(*symbol, *failure)
            })
            .collect();
        out.unique_binding(
            ix,
            "deferred_callback_argument",
            &shared,
            "mutable main state both settles write from their value slot and read back when they schedule the waiters",
        );
    }

    // The function that owns the RTCPeerConnection gatherer and exposes it
    // through a single main binding.
    let ice_gatherers: Vec<SymbolId> = ix
        .all_symbols()
        .into_iter()
        .filter(|symbol| {
            ix.origins(*symbol).is_some_and(|origins| {
                origins.iter().any(|origin| {
                    ix.resolve_fn(origin).is_some_and(|function| {
                        ix.own[function].members.contains("createDataChannel")
                            && ix.own[function].members.contains("RTCPeerConnection")
                    })
                })
            })
        })
        .collect();
    out.unique_binding(
        ix,
        "ice_gatherer_holder",
        &ice_gatherers,
        "holds the function that builds an `RTCPeerConnection`, creates a data channel and feeds the deferred-result settles",
    );
}

// ===========================================================================
// Aggregate environment collectors
// ===========================================================================

const COLLECTOR_EVIDENCE: &[(&str, &str)] = &[
    (
        "location_tamper_collector",
        "collector group body whose callbacks enumerate `location`, probe `getContext(\"webgl2\")` and check the `toString` accessor sentinel",
    ),
    (
        "browser_environment_collector",
        "collector group body whose init callback installs keyboard listeners and whose emit callback measures canvas text",
    ),
    (
        "device_orientation_collector",
        "collector group body whose init callback installs a `deviceorientation` listener and emits `history`/`referrer` state",
    ),
    (
        "input_automation_collector",
        "collector group body that probes automation marker arrays and whose init callback watches `insertFromPaste` input",
    ),
    (
        "touch_environment_collector",
        "collector group body whose init callback installs `touchstart`/`touchmove`/`touchend` listeners",
    ),
    (
        "timing_environment_collector",
        "collector group body whose emit callback measures interval timing and reads `Intl` locale data",
    ),
    (
        "pointer_native_api_collector",
        "collector group body whose init callback installs pointer listeners, probes observer tables and runs the `RTCPeerConnection` gatherer",
    ),
    (
        "environment_encoding_collector",
        "collector group body that shifts a seed from the main pool and whose emit callback encodes the host environment",
    ),
];

fn collector_kind(ix: &Ix<'_, '_>, function: usize) -> Option<&'static str> {
    // The collector ABI hands init/emit/last-chance/cleanup callbacks to the
    // third parameter and later, whether the body names its formals or reads
    // `arguments[N]`.
    let high_registrations = ix
        .registrations(function)
        .into_iter()
        .filter(|registration| registration.slot >= 2)
        .count();
    if high_registrations < 2 {
        return None;
    }
    // A capture may fuse the probe registrar with an environment collector. The
    // callable inspector is p09's own proof, so leave the merged function to it
    // rather than claim a collector role on the same binding.
    if ix.owns_probe_inspector(function) {
        return None;
    }
    let strings = &ix.strings_sub[function];
    let members = &ix.members_sub[function];
    if strings.contains("webgl2") || strings.contains("experimental-webgl2") {
        // A capture may fuse the WebGL probe with p13's canvas feature
        // collection closure. When the WebGL context is acquired by a closure
        // that enqueues canvas tasks on a captured queue, the merged function
        // is that canvas collector, not this package's location/WebGL group.
        if ix.webgl_proof_is_canvas_collection(function) {
            return None;
        }
        return Some("location_tamper_collector");
    }
    if members.contains("MozAppearance") {
        return Some("environment_encoding_collector");
    }
    let events = ix.add_listener_events(function);
    if events.contains("touchstart") {
        return Some("touch_environment_collector");
    }
    if events.contains("deviceorientation") {
        return Some("device_orientation_collector");
    }
    if events.contains("keypress") {
        return Some("browser_environment_collector");
    }
    if strings.contains("insertFromPaste") {
        return Some("input_automation_collector");
    }
    if members.contains("Collator") {
        return Some("timing_environment_collector");
    }
    if events.contains("mousemove") && members.contains("IntersectionObserver") {
        return Some("pointer_native_api_collector");
    }
    None
}

fn collectors(ix: &Ix<'_, '_>, out: &mut Out) {
    let mut groups: BTreeMap<&'static str, Vec<usize>> = BTreeMap::new();
    for function in 0..ix.fns.len() {
        if let Some(kind) = collector_kind(ix, function) {
            groups.entry(kind).or_default().push(function);
        }
    }
    for (role, candidates) in &groups {
        let evidence = COLLECTOR_EVIDENCE
            .iter()
            .find(|(name, _)| name == role)
            .map(|(_, evidence)| *evidence)
            .unwrap_or("collector group body");
        out.unique_function(ix, role, candidates, evidence);
    }
    let body = |role: &str| -> Option<usize> {
        groups.get(role).and_then(|candidates| match candidates.as_slice() {
            [only] => Some(*only),
            _ => None,
        })
    };

    // One callback per role, addressed by the registrar slot of the collector
    // ABI: 2 initializes, 3 emits, 4 runs at last chance, 5 cleans up.
    let registered = |role: &str, slot: u32, flag: Option<bool>| -> Vec<usize> {
        let Some(body) = body(role) else {
            return Vec::new();
        };
        ix.registrations(body)
            .into_iter()
            .filter(|registration| {
                registration.slot == slot
                    && flag.is_none_or(|flag| registration.flag == Some(flag))
            })
            .map(|registration| registration.callback)
            .collect()
    };
    for (role, group, slot, flag, evidence) in [
        (
            "location_tamper_init",
            "location_tamper_collector",
            2,
            None,
            "init callback registered by the location/WebGL tamper collector",
        ),
        (
            "keyboard_probe_init",
            "browser_environment_collector",
            2,
            None,
            "init callback that installs the keyboard listeners of the environment collector",
        ),
        (
            "device_orientation_init",
            "device_orientation_collector",
            2,
            None,
            "init callback that installs the `deviceorientation` listener",
        ),
        (
            "input_automation_last_chance",
            "input_automation_collector",
            4,
            None,
            "last-chance callback that emits the retained paste count and focus flag",
        ),
        (
            "touch_probe_init",
            "touch_environment_collector",
            2,
            None,
            "init callback that installs the passive touch listeners",
        ),
        (
            "touch_records_last_chance",
            "touch_environment_collector",
            4,
            None,
            "last-chance callback that flushes the retained touch records",
        ),
        (
            "timing_environment_emit",
            "timing_environment_collector",
            3,
            Some(false),
            "emit callback that writes the timing/locale state",
        ),
        (
            "pointer_probe_init",
            "pointer_native_api_collector",
            2,
            None,
            "init callback that installs the pointer listeners and runs the WebRTC probe",
        ),
        (
            "native_global_census_emit",
            "environment_encoding_collector",
            3,
            Some(false),
            "emit callback that enumerates the host object's own properties and hashes the native ones",
        ),
        (
            "seed_snapshot_last_chance",
            "environment_encoding_collector",
            4,
            None,
            "last-chance callback that re-encodes the seed snapshot before the buffer closes",
        ),
    ] {
        let candidates = registered(group, slot, flag);
        out.unique_function(ix, role, &candidates, evidence);
    }

    // The host-constructor probe table is the pointer collector's emit
    // callback that constructs or applies each table row; the other emit
    // callback paints the canvas.
    if let Some(body) = body("pointer_native_api_collector") {
        let emits: Vec<usize> = ix
            .registrations(body)
            .into_iter()
            .filter(|registration| registration.slot == 3)
            .map(|registration| registration.callback)
            .filter(|callback| {
                let own = &ix.own[*callback];
                own.members.contains("apply")
                    && !own.members.contains("getContext")
                    && !own.members.contains("arc")
            })
            .collect();
        out.unique_function(
            ix,
            "constructor_probe_table_replay",
            &emits,
            "emit callback that replays the host-constructor probe table, constructing or applying each row and encoding the outcome",
        );
    }

    // The native global census emit hands two callbacks to the path-walker's
    // `onEmpty` / `onError`.
    let census = body("environment_encoding_collector")
        .and_then(|body| {
            ix.registrations(body)
                .into_iter()
                .find(|registration| registration.slot == 3)
                .map(|registration| registration.callback)
        })
        .filter(|callback| ix.own[*callback].members.contains("getOwnPropertyNames"));
    if let Some(census) = census {
        out.function(
            ix,
            "native_global_census_emit",
            census,
            "emit callback that collects the host environment, then runs the native global API census",
        );
        let on_empty: Vec<usize> = ix
            .own[census]
            .calls
            .iter()
            .filter(|call| ix.method_name(call).as_deref() == Some("onEmpty"))
            .flat_map(|call| {
                ix.call_arguments(call).into_iter().filter_map(|value| ix.resolve_fn(value))
            })
            .collect();
        out.unique_function(
            ix,
            "native_global_count_emit",
            &on_empty,
            "`onEmpty` callback that emits the counted native globals",
        );
        let on_error: Vec<usize> = ix
            .own[census]
            .calls
            .iter()
            .filter(|call| ix.method_name(call).as_deref() == Some("onError"))
            .flat_map(|call| {
                ix.call_arguments(call).into_iter().filter_map(|value| ix.resolve_fn(value))
            })
            .collect();
        out.unique_function(
            ix,
            "native_global_scan_error",
            &on_error,
            "`onError` callback that forwards a failed native global scan to the error slot",
        );
    }

    // The client-hints promise callback of the timing collector's emit.
    if let Some(body) = body("timing_environment_collector") {
        let emits: Vec<usize> = ix
            .registrations(body)
            .into_iter()
            .filter(|registration| registration.slot == 3 && registration.flag == Some(false))
            .map(|registration| registration.callback)
            .collect();
        if let [emit] = emits.as_slice() {
            let thens: Vec<usize> = ix.own[*emit]
                .calls
                .iter()
                .filter(|call| ix.method_name(call).as_deref() == Some("then"))
                .flat_map(|call| {
                    ix.call_arguments(call).into_iter().filter_map(|value| ix.resolve_fn(value))
                })
                .collect();
            out.unique_function(
                ix,
                "client_hints_then",
                &thens,
                "`then` callback of the settled client-hints promise; encodes every user-agent field it received",
            );
        }
    }

    // The forced native TypeError: an immediately invoked function that reads a
    // computed member of `null`.
    let probes: Vec<usize> = (0..ix.fns.len())
        .filter(|function| ix.is_native_type_error_probe(*function))
        .collect();
    out.unique_function(
        ix,
        "native_type_error_probe",
        &probes,
        "immediately invoked function that reads a computed member of `null` to force the engine's own TypeError",
    );

    // Constructor stubs handed to the observer probe table.
    for (name, role) in [
        ("IntersectionObserver", "intersection_observer_stub"),
        ("MutationObserver", "mutation_observer_stub"),
        ("PerformanceObserver", "performance_observer_stub"),
    ] {
        let stubs: Vec<usize> = (0..ix.fns.len())
            .filter(|function| ix.observer_stub(*function) == Some(name))
            .collect();
        out.unique_function(
            ix,
            role,
            &stubs,
            "empty function used as the sole constructor argument of the observer probe-table row",
        );
    }
}

impl<'c, 'a> Ix<'c, 'a> {
    fn collector_bodies(&self) -> Vec<usize> {
        (0..self.fns.len()).filter(|function| collector_kind(self, *function).is_some()).collect()
    }

    fn is_native_type_error_probe(&self, function: usize) -> bool {
        let own = &self.own[function];
        let reads_null_member = own.calls.iter().any(|call| {
            matches!(
                strip(&call.callee),
                Expression::ComputedMemberExpression(member)
                    if matches!(strip(&member.object), Expression::NullLiteral(_))
            )
        });
        reads_null_member && self.immediately_invoked(function)
    }

    fn immediately_invoked(&self, function: usize) -> bool {
        let node = self.fns[function].node;
        let parent = self.nodes.parent_id(node);
        matches!(self.nodes.kind(parent), AstKind::CallExpression(call) if call.callee.span() == self.fns[function].span)
    }

    /// The observer constructor name when the function is an empty stub stored
    /// in a probe-table row whose sibling slot names that constructor, either
    /// inside the row's literal template or in a later `row[1]` write.
    fn observer_stub(&self, function: usize) -> Option<&'static str> {
        if !self.is_empty_body(function) {
            return None;
        }
        let symbol = self
            .decl_fn
            .iter()
            .find(|(_, index)| **index == function)
            .map(|(symbol, _)| *symbol);
        let symbol = symbol?;
        for reference in self.scoping.get_resolved_references(symbol) {
            let mut current = reference.node_id();
            let mut in_slot_array = false;
            for _ in 0..6 {
                let parent = self.nodes.parent_id(current);
                if parent == current {
                    break;
                }
                match self.nodes.kind(parent) {
                    AstKind::ArrayExpression(_) => in_slot_array = true,
                    AstKind::AssignmentExpression(assignment)
                        if in_slot_array && assignment.operator == AssignmentOperator::Assign =>
                    {
                        let Some(object) = target_member_object(&assignment.left) else {
                            continue;
                        };
                        let Some(table) = self.symbol(object) else {
                            continue;
                        };
                        let Some(origin) = self.single_origin(table) else {
                            continue;
                        };
                        let Some(elements) = array_elements(origin) else {
                            continue;
                        };
                        for element in elements {
                            let Some((_, name)) = member(element) else {
                                continue;
                            };
                            match name {
                                "IntersectionObserver" => return Some("IntersectionObserver"),
                                "MutationObserver" => return Some("MutationObserver"),
                                "PerformanceObserver" => return Some("PerformanceObserver"),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
                current = parent;
            }
        }
        self.assembled_observer_stub(function, symbol)
    }

    /// The same observer stub when the capture writes the row's constructor and
    /// callback slots after the row's literal template instead of placing the
    /// constructor inside it: `let row = [true]; function stub() {}; row[1] =
    /// host.IntersectionObserver; row[2] = [stub]; table[n] = row;`.
    ///
    /// The row is rebuilt from its own writes and no other node donates
    /// evidence: every reference of the stub must be the sole element of one
    /// `row[2]` write, the row must carry exactly one `row[1]` constructor
    /// write and no other slot writes, the constructor must be a member of the
    /// captured host the table's own rows use, and one table consumer
    /// `table[n] = row` must place the row in an array-template table. The
    /// stub declaration, the row writes and the consumer must all be owned by
    /// one function (so a nested or shadowing donor never supplies evidence),
    /// and the stub must stay referenced and callable.
    fn assembled_observer_stub(&self, function: usize, symbol: SymbolId) -> Option<&'static str> {
        let owner = self.fns[function].parent?;
        let mut agreed: Option<(SymbolId, NodeId, &'static str)> = None;
        let mut referenced = false;
        for reference in self.scoping.get_resolved_references(symbol) {
            if !reference.is_read() {
                return None;
            }
            referenced = true;
            let candidate = self.assembled_observer_row(owner, reference.node_id())?;
            match agreed {
                Some((row, arguments, name))
                    if row != candidate.0 || arguments != candidate.1 || name != candidate.2 =>
                {
                    return None;
                }
                Some(_) => {}
                None => agreed = Some(candidate),
            }
        }
        if !referenced {
            return None;
        }
        agreed.map(|(_, _, name)| name)
    }

    /// One `row[2] = [stub]` write, proven up through the same row's `row[1]`
    /// constructor slot and its single table consumer. Returns the row symbol,
    /// the callback-array write node and the constructor name.
    fn assembled_observer_row(
        &self,
        owner: usize,
        reference: NodeId,
    ) -> Option<(SymbolId, NodeId, &'static str)> {
        // The reference must reach its callback array through parentheses
        // only: anything else (a call, a logical value or a second array) is
        // not the observer's sole first constructor argument.
        let mut current = reference;
        let mut slot_array = None;
        let mut arguments_assignment = None;
        let mut arguments_node = None;
        for _ in 0..6 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            match self.nodes.kind(parent) {
                AstKind::ParenthesizedExpression(_) => {}
                AstKind::ArrayExpression(array) if slot_array.is_none() => {
                    slot_array = Some(array);
                }
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator == AssignmentOperator::Assign =>
                {
                    let slot = slot_array?;
                    let Expression::ArrayExpression(array) = strip(&assignment.right) else {
                        return None;
                    };
                    // The AST matcher yields an allocator Box here while the
                    // parent walk yields the dereferenced node; compare the
                    // underlying node addresses without changing the proof.
                    if !std::ptr::eq(array.as_ref(), slot) || array.elements.len() != 1 {
                        return None;
                    }
                    let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left
                    else {
                        return None;
                    };
                    if numeric_index(&target.expression)? != 2 {
                        return None;
                    }
                    arguments_assignment = Some(assignment);
                    arguments_node = Some(parent);
                    break;
                }
                _ => return None,
            }
            current = parent;
        }
        let arguments_assignment = arguments_assignment?;
        let arguments_node = arguments_node?;
        if self.owner[arguments_node.index()] != Some(owner) {
            return None;
        }
        let row = self.symbol(target_member_object(&arguments_assignment.left)?)?;
        if self.owner[self.scoping.symbol_declaration(row).index()] != Some(owner) {
            return None;
        }

        // The row is the emitted `[true]` template with no variable write; the
        // stub and the row must be owned by the same function.
        let template = self.single_origin(row)?;
        let elements = array_elements(template)?;
        let [flag] = elements.as_slice() else {
            return None;
        };
        bool_value(flag)?;

        // The row's own writes are exactly the constructor slot and the
        // callback array, each written once. Any other slot, a second write,
        // or a callback array other than the one holding this stub refuses.
        let mut constructor_write = None;
        let mut callback_write = false;
        for write in &self.own[owner].assignments {
            let Some(index) = target_numeric_index(&write.left) else {
                continue;
            };
            let Some(object) = target_member_object(&write.left) else {
                continue;
            };
            if self.symbol(object) != Some(row) {
                continue;
            }
            match index {
                1 => {
                    if constructor_write.replace(*write).is_some() {
                        return None;
                    }
                }
                2 => {
                    if !std::ptr::eq(*write, arguments_assignment) || callback_write {
                        return None;
                    }
                    callback_write = true;
                }
                _ => return None,
            }
        }
        if !callback_write {
            return None;
        }
        let constructor_write = constructor_write?;
        let (host, constructor) = member(&constructor_write.right)?;
        let name = observer_constructor(constructor)?;
        let host = self.host_key(host)?;

        // The row reaches the probe table through exactly one `table[n] = row`
        // write whose table has an array template naming the same captured
        // host on another row.
        let mut table = None;
        for write in &self.own[owner].assignments {
            if self.symbol(&write.right) != Some(row) {
                continue;
            }
            let Some(object) = target_member_object(&write.left) else {
                continue;
            };
            if target_numeric_index(&write.left).is_none() {
                continue;
            }
            let Some(candidate) = self.symbol(object) else {
                continue;
            };
            if candidate == row || table.replace(candidate).is_some() {
                return None;
            }
        }
        let table = table?;
        let origins = self.origins(table)?;
        if !origins
            .iter()
            .any(|origin| self.array_mentions_host(*origin, &host, 3))
        {
            return None;
        }
        Some((row, arguments_node, name))
    }

    /// A captured host identity: the root binding plus the static member path
    /// read from it. A shadowed or unbound root has a different (or no)
    /// identity, so only a constructor read from the table's own host agrees.
    fn host_key<'e>(&self, expression: &'e Expression<'a>) -> Option<(SymbolId, Vec<String>)> {
        if matches!(strip(expression), Expression::Identifier(_)) {
            return Some((self.symbol(expression)?, Vec::new()));
        }
        let (object, name) = member(expression)?;
        let (root, mut path) = self.host_key(object)?;
        path.push(name.to_string());
        Some((root, path))
    }

    /// True when an array template or one of its (bounded) row arrays reads a
    /// member of the given captured host.
    fn array_mentions_host(
        &self,
        expression: &'a Expression<'a>,
        host: &(SymbolId, Vec<String>),
        depth: u8,
    ) -> bool {
        if depth == 0 {
            return false;
        }
        let Some(elements) = array_elements(expression) else {
            return false;
        };
        for element in elements {
            if let Some((object, _)) = member(element) {
                if self.host_key(object).as_ref() == Some(host) {
                    return true;
                }
            }
            if self.array_mentions_host(element, host, depth - 1) {
                return true;
            }
        }
        false
    }
}

// ===========================================================================
// Observable probe adapters inside the fingerprint collectors
// ===========================================================================

fn observable_probes(ix: &Ix<'_, '_>, out: &mut Out) {
    let walkers: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_path_walker(*index)).collect();
    out.unique_function(
        ix,
        "path_walk_probe",
        &walkers,
        "walks a property path into a host object, collects the enumerated keys and pushes the encodings into the observable",
    );

    let operators: Vec<usize> =
        (0..ix.fns.len()).filter(|index| ix.is_pipe_operator(*index)).collect();
    out.unique_function(
        ix,
        "path_walk_operator",
        &operators,
        "operator piped onto the path walker: adds a fixed offset to the accumulated value",
    );

    let on_empty: Vec<usize> = ix
        .calls_with_method("onEmpty")
        .iter()
        .filter_map(|call| ix.call_arguments(call).first().copied())
        .filter_map(|value| ix.resolve_fn(value))
        .collect();
    let subscribers: Vec<usize> =
        on_empty.iter().copied().filter(|index| ix.own[*index].members.contains("subscribe")).collect();
    out.unique_function(
        ix,
        "seed_stream_subscribe",
        &subscribers,
        "`onEmpty` callback of the seed stream that subscribes a next/error pair",
    );
    if let [subscriber] = subscribers.as_slice() {
        for call in &ix.own[*subscriber].calls {
            if ix.method(call).is_none_or(|(_, name)| name != "subscribe") {
                continue;
            }
            let Some(object) = ix.call_arguments(call).first().copied() else {
                continue;
            };
            let Some(symbol) = ix.symbol(object) else {
                continue;
            };
            for (key, role, evidence) in [
                (
                    "next",
                    "seed_stream_next",
                    "`next` handler of the seed stream subscription: encodes each retained field through the seeded mixer and emits",
                ),
                (
                    "error",
                    "seed_stream_error",
                    "`error` handler of the seed stream subscription: forwards the failure to the collector's error slot",
                ),
            ] {
                let handlers: Vec<usize> = ix
                    .handler_values(symbol, key)
                    .into_iter()
                    .filter_map(|value| ix.resolve_fn(value))
                    .collect();
                out.unique_function(ix, role, &handlers, evidence);
            }
        }
    }
    let hashers: Vec<usize> = on_empty
        .iter()
        .copied()
        .filter(|index| {
            let own = &ix.own[*index];
            own.members.contains("sort") && own.members.contains("join") && own.members.contains("next")
        })
        .collect();
    out.unique_function(
        ix,
        "emit_sorted_name_hash",
        &hashers,
        "`onEmpty` callback that sorts the collected property names, joins them and emits the hashed total",
    );

    // The error handler belongs to the same observable as the sorted-name
    // emitter: both are called on the same receiver.
    if let [hasher] = hashers.as_slice() {
        let receiver = ix
            .calls_with_method("onEmpty")
            .iter()
            .find(|call| {
                ix.call_arguments(call)
                    .first()
                    .and_then(|value| ix.resolve_fn(value))
                    == Some(*hasher)
            })
            .and_then(|call| ix.method(call))
            .and_then(|(receiver, _)| ix.symbol(receiver));
        if let Some(receiver) = receiver {
            let forwarders: Vec<usize> = ix
                .calls_with_method("onError")
                .iter()
                .filter(|call| {
                    ix.method(call)
                        .and_then(|(found, _)| ix.symbol(found))
                        .is_some_and(|found| found == receiver)
                })
                .filter_map(|call| ix.call_arguments(call).first().copied())
                .filter_map(|value| ix.resolve_fn(value))
                .filter(|index| {
                    ix.fns[*index].params == 1
                        && ix.own[*index].calls.iter().any(|call| {
                            ix.method(call).is_some_and(|(_, name)| name == "error")
                                && ix.call_arguments(call).len() == 1
                        })
                })
                .collect();
            out.unique_function(
                ix,
                "forward_scan_error",
                &forwarders,
                "`onError` callback of the sorted-name observable: forwards the scan failure to the collector's error slot",
            );
        }
    }

    let webauthn: Vec<usize> = (0..ix.fns.len())
        .filter(|index| {
            let members = &ix.own[*index].members;
            members.contains("AuthenticatorAttestationResponse")
                && members.contains("PublicKeyCredential")
        })
        .collect();
    out.unique_function(
        ix,
        "webauthentication_capability_bits",
        &webauthn,
        "builds a capability bit mask from `typeof` tests on the Web Authentication host constructors",
    );
}

// ===========================================================================
// Aggregate environment bindings and exchange adapters
// ===========================================================================

fn exchange_adapters(ix: &Ix<'_, '_>, out: &mut Out) {
    // The automation marker arrays: string arrays probed against a host object.
    let mut lists: Vec<(SymbolId, BTreeSet<String>)> = Vec::new();
    for symbol in ix.all_symbols() {
        let Some(origin) = ix.single_origin(symbol) else {
            continue;
        };
        let Some(elements) = array_elements(origin) else {
            continue;
        };
        let Some(texts) = elements
            .iter()
            .map(|element| string_value(element))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        if texts.is_empty() {
            continue;
        }
        let set: BTreeSet<String> = texts.iter().map(|text| (*text).to_string()).collect();
        lists.push((symbol, set));
    }
    for (role, evidence, markers) in [
        (
            "automation_global_names",
            "host global property names probed with `in` to detect automation environments",
            vec!["ActiveXObject", "attachEvent", "callPhantom"],
        ),
        (
            "automation_document_property_names",
            "document property names probed with `in`",
            vec!["compatMode", "documentMode", "_Selenium_IDE_Recorder"],
        ),
        (
            "automation_global_markers",
            "the two global automation marker names",
            vec!["__fxdriver_unwrapped", "_Selenium_IDE_Recorder"],
        ),
        (
            "automation_document_markers",
            "document automation marker names",
            vec!["__webdriver_script_fn", "__fxdriver_unwrapped"],
        ),
        (
            "automation_body_markers",
            "body property names probed with `in`",
            vec!["contextMenu", "innerText", "mozRequestFullScreen"],
        ),
        (
            "automation_navigator_markers",
            "navigator property names probed with `in`",
            vec!["vibrate", "webdriver", "credentials"],
        ),
        (
            "automation_crypto_markers",
            "the crypto property name probed with `in`",
            vec!["subtle"],
        ),
        (
            "automation_external_markers",
            "the host `external` property name probed with `in`",
            vec!["Sequentum"],
        ),
    ] {
        let candidates: Vec<SymbolId> = lists
            .iter()
            .filter(|(_, set)| markers.iter().all(|needle| set.contains(*needle)))
            .map(|(symbol, _)| *symbol)
            .collect();
        out.unique_binding(ix, role, &candidates, evidence);
    }

    // The browser-global constructor list the environment collector probes
    // with `hasOwnProperty`.
    let global_lists: Vec<SymbolId> = lists
        .iter()
        .filter(|(symbol, set)| {
            set.contains("structuredClone")
                && set.contains("SharedArrayBuffer")
                && ix.readers(*symbol).iter().any(|reader| {
                    ix.own[*reader].members.contains("hasOwnProperty")
                })
        })
        .map(|(symbol, _)| *symbol)
        .collect();
    out.unique_binding(
        ix,
        "browser_global_probe_names",
        &global_lists,
        "browser global constructor names probed with `hasOwnProperty` by the environment collector",
    );

    // The font probe config: a numeric sample count followed by the font
    // family list measured through canvas text metrics.
    let font_configs: Vec<SymbolId> = ix
        .all_symbols()
        .into_iter()
        .filter(|symbol| ix.is_font_probe_config(*symbol))
        .collect();
    out.unique_binding(
        ix,
        "font_probe_config",
        &font_configs,
        "font-family list (with its sample count) measured through canvas text metrics",
    );

    // The location key -> single-letter code table.
    let location_tables: Vec<SymbolId> = ix
        .all_symbols()
        .into_iter()
        .filter(|symbol| ix.is_location_code_table(*symbol))
        .collect();
    out.unique_binding(
        ix,
        "location_field_codes",
        &location_tables,
        "null-prototype table mapping each `location` key to a one-character code",
    );

    // A fixed millisecond value consumed only as `new Date(value).toString()`.
    let dates: Vec<SymbolId> =
        ix.all_symbols().into_iter().filter(|symbol| ix.is_encoded_date(*symbol)).collect();
    out.unique_binding(
        ix,
        "payload_date_millis",
        &dates,
        "numeric value passed only to `new Date(...)`, whose text the collector then encodes",
    );

    // The callback-to-observable adapter: a factory call whose result is
    // called and whose return value is piped.
    let observables: Vec<SymbolId> = ix
        .all_symbols()
        .into_iter()
        .filter(|symbol| ix.is_observable_factory(*symbol))
        .collect();
    out.unique_binding(
        ix,
        "observable_factory_binder",
        &observables,
        "holds the adapter that turns a callback into an observable; another collector calls it and pipes its result",
    );

    // The throttle wrapper alias.
    let throttles: Vec<SymbolId> =
        ix.all_symbols().into_iter().filter(|symbol| ix.is_throttle_alias(*symbol)).collect();
    out.unique_binding(
        ix,
        "throttle_factory_alias",
        &throttles,
        "single main alias of the factory that wraps a callback in a `setTimeout`-gated passthrough",
    );

    // Host inspectors captured by the environment collectors.
    let bodies = ix.collector_bodies();
    let in_collector = |reader: usize| bodies.iter().any(|body| ix.within(reader, *body));
    let mut host_readers: Vec<(SymbolId, &'static str)> = Vec::new();
    for symbol in ix.all_symbols() {
        if !ix.readers(symbol).into_iter().any(in_collector) {
            continue;
        }
        let Some(origin) = ix.single_origin(symbol) else {
            continue;
        };
        let role = if ix.origin_has_path(origin, "Object.getOwnPropertyDescriptor", 2) {
            Some("property_descriptor_lookup")
        } else if ix.origin_has_path(origin, "Object.getPrototypeOf", 2) {
            Some("prototype_lookup")
        } else if ix.origin_has_path(origin, "Object.prototype.toString", 2) {
            Some("object_tag_reader")
        } else if ix.is_call_bound(origin, "Object.prototype.hasOwnProperty")
            || ix.path(origin).as_deref() == Some("Object.prototype.hasOwnProperty")
        {
            Some("own_property_checker")
        } else {
            None
        };
        if let Some(role) = role {
            host_readers.push((symbol, role));
        }
    }
    for (role, evidence) in [
        (
            "own_property_checker",
            "call-bound `Object.prototype.hasOwnProperty` read by an environment collector",
        ),
        (
            "property_descriptor_lookup",
            "`Object.getOwnPropertyDescriptor` read by an environment collector",
        ),
        ("prototype_lookup", "`Object.getPrototypeOf` read by an environment collector"),
        ("object_tag_reader", "`Object.prototype.toString` read by an environment collector"),
    ] {
        let candidates: Vec<SymbolId> = host_readers
            .iter()
            .filter(|(_, found)| *found == role)
            .map(|(symbol, _)| *symbol)
            .collect();
        out.unique_binding(ix, role, &candidates, evidence);
    }
}

impl<'c, 'a> Ix<'c, 'a> {
    fn is_path_walker(&self, function: usize) -> bool {
        let own = &self.own[function];
        self.fns[function].is_arrow
            && self.fns[function].params == 1
            && own.members.contains("join")
            && own.members.contains("pipe")
            && own.calls.iter().any(|call| self.method(call).is_some_and(|(_, name)| name == "push"))
    }

    fn is_pipe_operator(&self, function: usize) -> bool {
        let own = &self.own[function];
        if !self.fns[function].is_arrow || own.returns.len() != 1 {
            return false;
        }
        let Expression::BinaryExpression(binary) = strip(own.returns[0]) else {
            return false;
        };
        if binary.operator != BinaryOperator::Addition {
            return false;
        }
        let numeric = |value: &Expression<'a>| match strip(value) {
            Expression::NumericLiteral(_) => true,
            Expression::UnaryExpression(unary) => {
                unary.operator == UnaryOperator::UnaryNegation
                    && matches!(strip(&unary.argument), Expression::NumericLiteral(_))
            }
            _ => false,
        };
        let slot = |value: &Expression<'a>| self.slot_index(value, function) == Some(0);
        ((numeric(&binary.left) && slot(&binary.right))
            || (numeric(&binary.right) && slot(&binary.left)))
            && (self.passed_to_method(function, "pipe")
                || self.passed_through_method(function, "pipe"))
    }

    /// A binding whose origin is `target.call.bind(path)`.
    fn is_call_bound(&self, origin: &Expression<'a>, path: &str) -> bool {
        let Expression::CallExpression(call) = strip(origin) else {
            return false;
        };
        self.method(call).is_some_and(|(target, name)| {
            name == "bind" && self.path(target).as_deref().is_some_and(|found| found.ends_with(".call"))
        }) && self.call_arguments(call).last().is_some_and(|argument| {
            self.path(argument).as_deref() == Some(path)
        })
    }

    fn is_font_probe_config(&self, symbol: SymbolId) -> bool {
        let Some(origin) = self.single_origin(symbol) else {
            return false;
        };
        let Some(elements) = array_elements(origin) else {
            return false;
        };
        let [first, second] = elements.as_slice() else {
            return false;
        };
        if !matches!(strip(first), Expression::NumericLiteral(_)) {
            return false;
        }
        let Some(fonts) = array_elements(second) else {
            return false;
        };
        let Some(names) = fonts
            .iter()
            .map(|element| string_value(element))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        names.len() >= 10 && names.contains(&"Verdana") && names.contains(&"Courier New")
    }

    fn is_location_code_table(&self, symbol: SymbolId) -> bool {
        let Some(origin) = self.single_origin(symbol) else {
            return false;
        };
        let Some(call) = as_call(origin) else {
            return false;
        };
        if self.path(&call.callee).as_deref() != Some("Object.create")
            || !matches!(args(call).as_slice(), [only] if matches!(strip(only), Expression::NullLiteral(_)))
        {
            return false;
        }
        let mut codes = 0;
        for index in 0..self.fns.len() {
            for (object, _, value) in &self.own[index].member_writes {
                if *object == Some(symbol)
                    && string_value(value).is_some_and(|text| text.chars().count() == 1)
                {
                    codes += 1;
                }
            }
        }
        codes >= 5
    }

    fn is_encoded_date(&self, symbol: SymbolId) -> bool {
        let Some(origin) = self.single_origin(symbol) else {
            return false;
        };
        let numeric = match strip(origin) {
            Expression::NumericLiteral(_) => true,
            Expression::UnaryExpression(unary) => {
                unary.operator == UnaryOperator::UnaryNegation
                    && matches!(strip(&unary.argument), Expression::NumericLiteral(_))
            }
            _ => false,
        };
        if !numeric {
            return false;
        }
        let mut reads = 0;
        for reference in self.scoping.get_resolved_references(symbol) {
            if !reference.is_read() {
                continue;
            }
            reads += 1;
            let mut current = reference.node_id();
            let mut date = false;
            let mut text = false;
            let mut consumed = false;
            let mut text_call = false;
            for _ in 0..6 {
                let parent = self.nodes.parent_id(current);
                if parent == current {
                    break;
                }
                match self.nodes.kind(parent) {
                    AstKind::NewExpression(new) => {
                        date = self.path(&new.callee).as_deref() == Some("Date");
                    }
                    AstKind::StaticMemberExpression(found) => {
                        text = found.property.name == "toString";
                    }
                    AstKind::ComputedMemberExpression(found) => {
                        text = matches!(&found.expression, Expression::StringLiteral(literal) if literal.value == "toString");
                    }
                    AstKind::CallExpression(call) => {
                        if text && !text_call {
                            text_call = true;
                        } else {
                            // The consumer must be a plain call, not a member
                            // method such as `Array.from`.
                            consumed = self.method(call).is_none();
                            break;
                        }
                    }
                    _ => {}
                }
                current = parent;
            }
            if !(date && text && text_call && consumed) {
                return false;
            }
        }
        reads > 0
    }

    fn is_observable_factory(&self, symbol: SymbolId) -> bool {
        let Some(origin) = self.single_origin(symbol) else {
            return false;
        };
        let Some(call) = as_call(origin) else {
            return false;
        };
        let Some(factory) = self.resolve_fn(&call.callee) else {
            return false;
        };
        let returns_nested = self.own[factory].returns.iter().any(|value| {
            self.resolve_fn(value).is_some_and(|nested| !self.own[nested].news.is_empty())
        });
        if !returns_nested {
            return false;
        }
        let mut piped = false;
        for reader in 0..self.fns.len() {
            for call in &self.own[reader].calls {
                if self.symbol(&call.callee) != Some(symbol) {
                    continue;
                }
                let node = call.node_id.get();
                let parent = self.nodes.parent_id(node);
                if let AstKind::StaticMemberExpression(found) = self.nodes.kind(parent) {
                    if found.property.name == "pipe" {
                        piped = true;
                    }
                }
                if let AstKind::ComputedMemberExpression(found) = self.nodes.kind(parent) {
                    if matches!(&found.expression, Expression::StringLiteral(literal) if literal.value == "pipe")
                    {
                        piped = true;
                    }
                }
            }
        }
        piped
    }

    fn is_throttle_alias(&self, symbol: SymbolId) -> bool {
        let Some(origin) = self.single_origin(symbol) else {
            return false;
        };
        let Expression::Identifier(_) = strip(origin) else {
            return false;
        };
        let Some(target) = self.resolve_fn(origin) else {
            return false;
        };
        let applies_this = (0..self.fns.len()).filter(|index| self.within(*index, target)).any(|index| {
            self.own[index].calls.iter().any(|call| {
                self.method(call).is_some_and(|(_, name)| name == "apply")
                    && matches!(
                        self.call_arguments(call).as_slice(),
                        [first, second]
                            if matches!(strip(first), Expression::ThisExpression(_))
                                && matches!(strip(second), Expression::Identifier(identifier) if identifier.name == "arguments")
                    )
            })
        });
        let timer = (0..self.fns.len()).filter(|index| self.within(*index, target)).any(|index| {
            self.own[index].calls.iter().any(|call| {
                self.method(call).is_some_and(|(_, name)| name == "setTimeout")
                    || self.is_global_call(call, "setTimeout")
            })
        });
        applies_this
            && timer
            && self.own[target].returns.iter().any(|value| self.resolve_fn(value).is_some())
    }
}

fn as_call<'e, 'a>(expression: &'e Expression<'a>) -> Option<&'e CallExpression<'a>> {
    match strip(expression) {
        Expression::CallExpression(call) => Some(call),
        _ => None,
    }
}
