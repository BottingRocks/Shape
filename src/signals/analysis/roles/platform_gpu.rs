//! Package `p11_env_platform_gpu`: platform and GPU environment probes.
//!
//! The anchors are host API and protocol names the program cannot rename:
//! `RTCPeerConnection` / `onicecandidate` / `createOffer` / `setLocalDescription`,
//! `Promise` / `vendor` / `userAgent` / `indexedDB` / `onupgradeneeded` /
//! `createObjectStore` / `queryUsageAndQuota` / `webkitRequestFileSystem`,
//! `measureUserAgentSpecificMemory` / `breakdown` / `attribution` / `scope`,
//! `matchMedia` / `setTimeout`, and `getContext("webgl")` / `getParameter` /
//! `shaderPrecisions` / `supportedExtensions` / `onmessage` / `onerror`.
//! Everything else is read from the function's own body: the calls it makes,
//! the members it reads and writes, its formal slots, its returned value and
//! the function values it hands to host APIs. Nested functions are separate
//! units; a callback is claimed in its own right.
//!
//! Both tracer shapes are read. A named formal and `arguments[N]` are the same
//! slot ([`ParameterSlots`]); `a.b` and `a["b"]` are the same member; a call
//! argument can be an inline arrow or a hoisted declaration. Argument
//! *positions* of a host API call (`queryUsageAndQuota(success, error)`,
//! `webkitRequestFileSystem(size, flags, granted, denied)`) are read as such;
//! the only numeric index read here is a RegExp match index (`m[0]` is the
//! whole match, `m[1]` its first capture), which is part of the match shape,
//! not a build constant.
//!
//! # Ownership shared with `p10_browser_env`
//!
//! Four functions this package's targets alias — `resolveDeferredResult`,
//! `rejectDeferredResult`, `measureMaxMediaFeatureValue` and
//! `evaluateMediaFeatureValues` — are on `p10_browser_env`'s target list and are
//! left to it. This package claims only their *single MAIN alias*
//! (`resolveDeferredResultAlias`, `rejectDeferredResultAlias`,
//! `measureMaxMediaFeatureValueAlias`, `evaluateMediaFeatureValuesAlias`),
//! which p11's list owns. Each alias role is named after the aliased function's
//! proven p10 role, not after a host API the alias happens to serve. Should p10
//! also emit an `alias_role` for one of those bindings, the plan records a role
//! conflict and leaves the alias unnamed rather than renaming it twice.
//!
//! # Fail closed
//!
//! A role names one entity. If two different spans prove the same role, both
//! claims are dropped ([`Out::finish`]) instead of handing two bindings the same
//! name. Two roles on one binding are refused by the plan itself. Equal
//! candidates for one role are therefore never resolved by source order.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator};

use crate::signals::extract::bindings::{
    binding_pattern_symbol, new_arguments, numeric_index, property_key_name, static_member_name,
    ParameterSlots,
};
use crate::signals::schema::Confidence;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};
use crate::analysis;

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Function }
}

const fn binding(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::MainBinding }
}

/// Every role this package can claim, with the one name it receives.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    // WebRTC local-IP collection.
    function("webrtc_ice_candidate_collector", "collectLocalIpsViaWebRtc"),
    function("webrtc_ice_candidate_handler", "handleIceCandidate"),
    function("ice_candidate_ip_match_predicate", "matchesStoredIceCandidateIp"),
    function("ice_offer_local_description_applier", "applyLocalDescriptionFromOffer"),
    function("ice_offer_rejection_reporter", "reportIceOfferFailure"),
    function("ice_offer_legacy_description_applier", "applyLocalDescriptionFromLegacyOffer"),
    function("ice_offer_legacy_rejection_reporter", "reportLegacyIceOfferFailure"),
    binding("webrtc_results_ready", "resultsReady"),
    binding("webrtc_enumeration_started", "enumerationStarted"),
    binding("ice_gathering_failed", "iceGatheringFailed"),
    binding("ice_candidate_ip_pattern", "ipAddressPattern"),
    binding("ice_candidate_type_pattern", "candidateTypePattern"),
    // MAIN aliases of `p10_browser_env`'s shared deferred settles. The roles
    // follow the aliased functions' proven p10 roles, not the ICE gatherer that
    // happens to call them.
    binding("resolve_deferred_result_alias", "resolveDeferredResultAlias"),
    binding("reject_deferred_result_alias", "rejectDeferredResultAlias"),
    // Memory-attribution formatting and measurement.
    function("memory_attribution_formatter", "formatMemoryAttributionBreakdown"),
    function("memory_breakdown_entry_mapper", "mapMemoryBreakdownEntry"),
    function("memory_attribution_mapper", "mapMemoryAttribution"),
    binding("memory_breakdown_formatter", "memoryBreakdownFormatter"),
    function("memory_attribution_report_emitter", "emitMemoryAttributionReport"),
    function("memory_breakdown_attribution_filter", "keepCurrentDocumentBreakdownEntry"),
    function("memory_attribution_current_document_test", "isCurrentDocumentAttribution"),
    function("memory_breakdown_attribution_scanner", "scanBreakdownEntryAttributions"),
    function("memory_breakdown_entry_index_writer", "rememberCurrentDocumentEntryIndex"),
    function("memory_measure_error_forwarder", "forwardMemoryMeasureError"),
    // Browser/platform detection.
    function("platform_detection_prober", "detectBrowserPlatform"),
    function("platform_detection_executor", "probeBrowserPlatform"),
    function("indexed_db_blob_support_probe", "probeIndexedDbBlobStorageSupport"),
    function("indexed_db_upgrade_blob_put", "onTempDatabaseUpgradePutBlob"),
    function("quota_usage_comparator", "compareQuotaToHeapLimit"),
    function("quota_usage_rejection_reporter", "rejectWithQuotaError"),
    function("file_system_granted_callback", "handleFileSystemGranted"),
    function("file_system_denied_callback", "handleFileSystemDenied"),
    function("platform_result_emitter", "emitEncodedPlatformResult"),
    function("platform_result_error_forwarder", "forwardPlatformProbeError"),
    // MAIN aliases of p10's media-feature helpers.
    binding("measure_max_media_feature_value_alias", "measureMaxMediaFeatureValueAlias"),
    binding("evaluate_media_feature_values_alias", "evaluateMediaFeatureValuesAlias"),
    // Media-feature range probes handed to the shared prober.
    function("media_feature_range_prober", "probeMediaFeatureRanges"),
    function("media_feature_range_serializer", "serializeMediaFeatureRangeProbe"),
    function("media_feature_probe_completion", "completeMediaFeatureRangeProbe"),
    // WebGL report collection and the worker protocol.
    function("webgl_parameter_map_builder", "buildWebglParameterMap"),
    function("webgl_parameter_map_writer", "assignParameterValueByName"),
    function("webgl_report_collector", "collectWebglReport"),
    function("webgl_dimension_values_joiner", "joinWebglDimensions"),
    function("webgl_precision_range_joiner", "formatShaderPrecisionRange"),
    function("webgl_worker_message_handler", "handleWebglWorkerMessage"),
    function("webgl_worker_dimension_joiner", "joinWorkerMessageDimensions"),
    function("webgl_worker_error_handler", "handleWebglWorkerError"),
    function("webgl_worker_cleanup", "stopWebglWorkerAndCompleteStream"),
];

// ---------------------------------------------------------------------------
// Structural index
// ---------------------------------------------------------------------------

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

/// The member an assignment target names, with its object.
fn target_member<'e, 'a>(target: &'e AssignmentTarget<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
    match target {
        AssignmentTarget::StaticMemberExpression(found) => {
            Some((&found.object, found.property.name.as_str()))
        }
        AssignmentTarget::ComputedMemberExpression(found) => match &found.expression {
            Expression::StringLiteral(literal) => Some((&found.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

fn target_identifier<'e, 'a>(target: &'e AssignmentTarget<'a>) -> Option<&'e IdentifierReference<'a>> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => Some(identifier),
        _ => None,
    }
}

fn string_value<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    match strip(expression) {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn is_void(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::UnaryExpression(unary)
        if unary.operator == UnaryOperator::Void)
}

fn is_true(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::BooleanLiteral(literal) if literal.value)
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

#[derive(Clone, Copy)]
enum FnNode<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
}

struct FnEntry<'a> {
    span: Span,
    func: FnNode<'a>,
    parent: Option<usize>,
}

/// What a function owns directly, excluding nested functions.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    reads: BTreeSet<SymbolId>,
    members: BTreeSet<String>,
    written_keys: BTreeSet<String>,
    /// Member names tested with `typeof <x>.name === "function"`.
    typeof_members: BTreeSet<String>,
    ifs: Vec<&'a IfStatement<'a>>,
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
    member_assignments: Vec<&'a AssignmentExpression<'a>>,
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
            member_assignments: Vec::new(),
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
            let func = match node.kind() {
                AstKind::Function(found) => Some((found.span, FnNode::Function(found))),
                AstKind::ArrowFunctionExpression(found) => {
                    Some((found.span, FnNode::Arrow(found)))
                }
                _ => None,
            };
            if let Some((span, func)) = func {
                let index = self.fns.len();
                self.fns.push(FnEntry { span, func, parent: owner });
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
                            let value = &statement.expression;
                            if !is_void(value) {
                                self.own[index].returns.push(value);
                            }
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
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign =>
            {
                if let Some(identifier) = target_identifier(&assignment.left) {
                    if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                        self.assigns.entry(symbol).or_default().push(assignment);
                    }
                }
                if target_member(&assignment.left).is_some() {
                    self.member_assignments.push(assignment);
                }
                if let Some(owner) = owner {
                    if let Some((_, name)) = target_member(&assignment.left) {
                        self.own[owner].written_keys.insert(name.to_string());
                    }
                    self.own[owner].assignments.push(assignment);
                }
            }
            AstKind::ObjectProperty(property) if property.kind == PropertyKind::Init => {
                if let Some(name) = property_key_name(&property.key) {
                    if let Some(owner) = owner {
                        self.own[owner].written_keys.insert(name);
                    }
                }
            }
            _ => {}
        }
        let Some(owner) = owner else {
            return;
        };
        let scoping = self.scoping;
        let own = &mut self.own[owner];
        match kind {
            AstKind::CallExpression(call) => own.calls.push(call),
            AstKind::NewExpression(new) => own.news.push(new),
            AstKind::ReturnStatement(statement) => {
                if let Some(argument) = statement.argument.as_ref() {
                    if !is_void(argument) {
                        own.returns.push(argument);
                    }
                }
            }
            AstKind::IfStatement(statement) => own.ifs.push(statement),
            AstKind::IdentifierReference(identifier) => {
                if let Some(symbol) = analysis::identifier_symbol(identifier, scoping) {
                    own.reads.insert(symbol);
                }
            }
            AstKind::StaticMemberExpression(found) => {
                own.members.insert(found.property.name.to_string());
            }
            AstKind::ComputedMemberExpression(found) => {
                if let Expression::StringLiteral(literal) = &found.expression {
                    own.members.insert(literal.value.to_string());
                }
            }
            AstKind::BinaryExpression(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::StrictEquality | BinaryOperator::Equality
                ) =>
            {
                if let Some(name) = typeof_member_operand(binary) {
                    own.typeof_members.insert(name.to_string());
                }
            }
            _ => {}
        }
    }

    fn span(&self, index: usize) -> Span {
        self.fns[index].span
    }

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.scoping)
    }

    /// `Object.prototype.toString`-style path rooted at an unbound global.
    fn path(&self, expression: &Expression<'a>) -> Option<String> {
        match strip(expression) {
            Expression::Identifier(identifier) => {
                (self.symbol(expression).is_none()).then(|| identifier.name.to_string())
            }
            _ => {
                let (object, name) = member(expression)?;
                Some(format!("{}.{name}", self.path(object)?))
            }
        }
    }

    fn slot_index(&self, expression: &Expression<'a>, function: usize) -> Option<u32> {
        let slot = self.slots.incoming_slot(strip(expression))?;
        (slot.owner == self.fn_node(function)).then_some(slot.index as u32)
    }

    /// A slot of `function`, directly or through one local copy of the incoming
    /// argument (`var success = arguments[2]`).
    fn slot_value(&self, expression: &Expression<'a>, function: usize) -> Option<u32> {
        if let Some(index) = self.slot_index(expression, function) {
            return Some(index);
        }
        let symbol = self.symbol(expression)?;
        self.slot_index(self.single_origin(symbol)?, function)
    }

    fn fn_node(&self, index: usize) -> NodeId {
        match self.fns[index].func {
            FnNode::Function(found) => found.node_id.get(),
            FnNode::Arrow(found) => found.node_id.get(),
        }
    }

    /// Every source value of a binding, or `None` when it has a write that is
    /// not a declarator initializer or a plain assignment.
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

    fn is_create_null(&self, expression: &Expression<'a>) -> bool {
        let Expression::CallExpression(call) = strip(expression) else {
            return false;
        };
        if self.path(&call.callee).as_deref() != Some("Object.create") {
            return false;
        }
        matches!(args(call).as_slice(), [only] if matches!(strip(only), Expression::NullLiteral(_)))
    }

    fn is_regexp_value(&self, expression: &Expression<'a>) -> bool {
        match strip(expression) {
            Expression::RegExpLiteral(_) => true,
            Expression::CallExpression(call) => {
                self.path(&call.callee).as_deref() == Some("RegExp")
                    || member(&call.callee).is_some_and(|(_, name)| name == "RegExp")
            }
            _ => false,
        }
    }

    fn method<'e>(
        &self,
        call: &'e CallExpression<'a>,
    ) -> Option<(&'e Expression<'a>, &'e str)> {
        member(self.invocation(call).0)
    }

    fn invoked_name<'e>(&self, call: &'e CallExpression<'a>) -> Option<&'e str> {
        static_member_name(self.invocation(call).0)
    }

    /// The method a call names, also through the capture's lowered
    /// `[receiver, receiver.method][1].call(receiver, ...)` form and through
    /// `.call` / `.apply` forwarding.
    fn call_name(&self, call: &CallExpression<'a>) -> Option<String> {
        if let Some((inner, "call")) = member(&call.callee) {
            return self.callee_name(inner);
        }
        self.callee_name(&call.callee)
    }

    fn callee_name<'e>(&self, callee: &'e Expression<'a>) -> Option<String> {
        let callee = strip(callee);
        if let Some(name) = static_member_name(callee) {
            return Some(name.to_string());
        }
        if let Expression::ComputedMemberExpression(found) = callee {
            if let Some(index) = numeric_index(&found.expression) {
                if let Some(element) = self.array_element(&found.object, index) {
                    return self.callee_name(element);
                }
            }
        }
        let symbol = self.symbol(callee)?;
        self.callee_name(self.single_origin(symbol)?)
    }

    /// The array element `index` of an array literal, directly or through a
    /// single-origin binding. The capture lowers `receiver.method.call(...)`
    /// to `[receiver, receiver.method][1].call(receiver, ...)`.
    fn array_element<'e>(
        &self,
        expression: &'e Expression<'a>,
        index: u32,
    ) -> Option<&'e Expression<'a>> {
        match strip(expression) {
            Expression::ArrayExpression(array) => array
                .elements
                .get(index as usize)
                .and_then(|element| element.as_expression()),
            Expression::Identifier(_) => {
                let symbol = self.symbol(expression)?;
                self.array_element(self.single_origin(symbol)?, index)
            }
            _ => None,
        }
    }

    /// The single main-scope alias of `function`, if it has exactly one.
    fn main_alias(&self, function: usize) -> Option<SymbolId> {
        let mut found = Vec::new();
        for (_, node) in self.nodes.iter_enumerated() {
            let AstKind::VariableDeclarator(declarator) = node.kind() else {
                continue;
            };
            let Some(init) = declarator.init.as_ref() else {
                continue;
            };
            if self.resolve_fn(init) != Some(function) {
                continue;
            }
            let Some(symbol) = binding_pattern_symbol(&declarator.id) else {
                continue;
            };
            if self.declared_outside(symbol, function) {
                found.push(symbol);
            }
        }
        match found.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    /// The callee and arguments of a call, seen through the target cohort's
    /// `callee.call(receiver, ...)` form.
    fn invocation<'e>(
        &self,
        call: &'e CallExpression<'a>,
    ) -> (&'e Expression<'a>, Vec<&'e Expression<'a>>) {
        let arguments = args(call);
        if let Some((inner, "call")) = member(&call.callee) {
            if !arguments.is_empty()
                && (member(inner).is_some() || self.slots.incoming_slot(strip(inner)).is_some())
            {
                return (inner, arguments[1..].to_vec());
            }
        }
        (&call.callee, arguments)
    }

    /// Function values handed to a call.
    fn callbacks(&self, call: &CallExpression<'a>) -> Vec<usize> {
        self.invocation(call)
            .1
            .into_iter()
            .filter_map(|argument| self.resolve_fn(argument))
            .collect()
    }

    /// Nearest enclosing `if` statement of a node.
    fn enclosing_if(&self, node: NodeId) -> Option<&'a IfStatement<'a>> {
        let mut current = node;
        for _ in 0..64 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            if let AstKind::IfStatement(statement) = self.nodes.kind(current) {
                return Some(statement);
            }
        }
        None
    }

    /// The binding at `symbol` is declared outside `function`.
    fn declared_outside(&self, symbol: SymbolId, function: usize) -> bool {
        let declaration = self.scoping.symbol_declaration(symbol);
        self.owner[declaration.index()] != Some(function)
    }

    /// Exactly one non-`void` return value, in either body form.
    fn single_return(&self, index: usize) -> Option<&'a Expression<'a>> {
        match self.own[index].returns.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }

    /// The local that `call` initializes (`let m = re.exec(s)`).
    fn initialized_from(&self, function: usize, call: &CallExpression<'a>) -> Option<SymbolId> {
        let span = call.span;
        for (id, node) in self.nodes.iter_enumerated() {
            if self.owner[id.index()] != Some(function) {
                continue;
            }
            let AstKind::VariableDeclarator(declarator) = node.kind() else {
                continue;
            };
            if declarator.init.as_ref().map(GetSpan::span) != Some(span) {
                continue;
            }
            if let Some(symbol) = binding_pattern_symbol(&declarator.id) {
                return Some(symbol);
            }
        }
        None
    }

    /// `[index]` reads of `symbol` inside `function`'s own body.
    fn numeric_reads(&self, function: usize, symbol: SymbolId) -> Vec<u32> {
        let mut out = Vec::new();
        for (id, node) in self.nodes.iter_enumerated() {
            if self.owner[id.index()] != Some(function) {
                continue;
            }
            let AstKind::ComputedMemberExpression(found) = node.kind() else {
                continue;
            };
            if self.symbol(&found.object) != Some(symbol) {
                continue;
            }
            if let Some(index) = numeric_index(&found.expression) {
                out.push(index);
            }
        }
        out
    }
}

/// The member name of a `typeof x.name === "function"` test.
fn typeof_member_operand<'e, 'a>(binary: &'e BinaryExpression<'a>) -> Option<&'e str> {
    let operand = |expression: &'e Expression<'a>| -> Option<&'e str> {
        let Expression::UnaryExpression(unary) = strip(expression) else {
            return None;
        };
        if unary.operator != UnaryOperator::Typeof {
            return None;
        }
        member(&unary.argument).map(|(_, name)| name)
    };
    let test = |expression: &'e Expression<'a>| -> bool {
        matches!(strip(expression), Expression::StringLiteral(literal) if literal.value == "function")
    };
    if test(&binary.right) {
        return operand(&binary.left);
    }
    if test(&binary.left) {
        return operand(&binary.right);
    }
    None
}

// ---------------------------------------------------------------------------
// Claims
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Out {
    claims: Vec<FinderClaim>,
    seen: BTreeSet<(&'static str, u32, u32)>,
}

impl Out {
    fn function(&mut self, ix: &Ix<'_, '_>, role: &'static str, index: usize, evidence: &str) {
        let span = ix.span(index);
        if !self.seen.insert((role, span.start, span.end)) {
            return;
        }
        self.claims.push(FinderClaim::Function {
            role,
            function_span: span,
            alias_role: None,
            confidence: Confidence::Medium,
            evidence: vec![evidence.to_string()],
        });
    }

    fn binding(&mut self, ix: &Ix<'_, '_>, role: &'static str, symbol: SymbolId, evidence: &str) {
        let span = ix.scoping.symbol_span(symbol);
        if !self.seen.insert((role, span.start, span.end)) {
            return;
        }
        self.claims.push(FinderClaim::MainBinding {
            role,
            identifier_span: span,
            confidence: Confidence::Medium,
            evidence: vec![evidence.to_string()],
        });
    }

    /// A role names one entity: keep it only when exactly one span proved it.
    fn finish(self) -> Vec<FinderClaim> {
        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        for claim in &self.claims {
            *counts.entry(claim.role()).or_default() += 1;
        }
        self.claims
            .into_iter()
            .filter(|claim| counts.get(claim.role()) == Some(&1))
            .collect()
    }
}

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let slots = ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes);
    let ix = Ix::new(ctx, &slots);
    let mut out = Out::default();
    webrtc(&ix, &mut out);
    memory_format(&ix, &mut out);
    platform(&ix, &mut out);
    memory_measure(&ix, &mut out);
    media_features(&ix, &mut out);
    webgl(&ix, &mut out);
    out.finish()
}

// ---------------------------------------------------------------------------
// WebRTC local-IP collection
// ---------------------------------------------------------------------------

const RTC_CONSTRUCTORS: &[&str] = &[
    "RTCPeerConnection",
    "mozRTCPeerConnection",
    "webkitRTCPeerConnection",
];

fn is_rtc_constructor(ix: &Ix<'_, '_>, expression: &Expression<'_>) -> bool {
    is_rtc_constructor_depth(ix, expression, 4)
}

fn is_rtc_constructor_depth(ix: &Ix<'_, '_>, expression: &Expression<'_>, depth: u8) -> bool {
    match strip(expression) {
        Expression::LogicalExpression(logical)
            if matches!(logical.operator, LogicalOperator::Or | LogicalOperator::Coalesce) =>
        {
            is_rtc_constructor_depth(ix, &logical.left, depth)
                || is_rtc_constructor_depth(ix, &logical.right, depth)
        }
        other => {
            if static_member_name(other).is_some_and(|name| RTC_CONSTRUCTORS.contains(&name))
                || ix.path(other).is_some_and(|path| RTC_CONSTRUCTORS.contains(&path.as_str()))
            {
                return true;
            }
            if depth == 0 {
                return false;
            }
            ix.symbol(other)
                .and_then(|symbol| ix.single_origin(symbol))
                .is_some_and(|origin| is_rtc_constructor_depth(ix, origin, depth - 1))
        }
    }
}

/// The host object a fallback constructor expression reads the RTC constructor
/// from. Every constructor in the expression must be a member of the one same
/// bound host: the lowered fallback never mixes hosts.
fn rtc_constructor_host(ix: &Ix<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    rtc_constructor_host_depth(ix, expression, 4)
}

fn rtc_constructor_host_depth(
    ix: &Ix<'_, '_>,
    expression: &Expression<'_>,
    depth: u8,
) -> Option<SymbolId> {
    match strip(expression) {
        Expression::LogicalExpression(logical)
            if matches!(logical.operator, LogicalOperator::Or | LogicalOperator::Coalesce) =>
        {
            let left = rtc_constructor_host_depth(ix, &logical.left, depth)?;
            let right = rtc_constructor_host_depth(ix, &logical.right, depth)?;
            (left == right).then_some(left)
        }
        other => {
            if let Some((object, name)) = member(other) {
                if RTC_CONSTRUCTORS.contains(&name) {
                    return ix.symbol(object);
                }
            }
            if depth == 0 {
                return None;
            }
            let symbol = ix.symbol(other)?;
            rtc_constructor_host_depth(ix, ix.single_origin(symbol)?, depth - 1)
        }
    }
}

/// The explicit `if/else` constructor fallback the capture lowers instead of a
/// single `a || b || c` expression. The `new` constructs a collector-owned local
/// whose only writes are owned branches that dominate it, and every branch value
/// reads an RTC constructor from one shared host. The base `||`/`??` proof above
/// is unchanged; this is an additive arm.
fn is_rtc_constructor_branch_fallback(
    ix: &Ix<'_, '_>,
    collector: usize,
    new: &NewExpression<'_>,
) -> bool {
    if !matches!(strip(&new.callee), Expression::Identifier(_)) {
        return false;
    }
    let Some(symbol) = ix.symbol(&new.callee) else {
        return false;
    };
    if ix.declared_outside(symbol, collector) {
        return false;
    }
    let Some(origins) = ix.origins(symbol) else {
        return false;
    };
    if origins.len() < 2 {
        return false;
    }
    let assignments = ix.assigns.get(&symbol).map(Vec::as_slice).unwrap_or(&[]);
    if assignments.len() != origins.len() {
        return false;
    }
    let mut host = None;
    for &origin in &origins {
        let Some(found) = rtc_constructor_host(ix, origin) else {
            return false;
        };
        match host {
            None => host = Some(found),
            Some(previous) if previous != found => return false,
            Some(_) => {}
        }
    }
    for assignment in assignments {
        if ix.owner[assignment.node_id.get().index()] != Some(collector) {
            return false;
        }
        let Some(statement) = ix.enclosing_if(assignment.node_id.get()) else {
            return false;
        };
        if statement.span.end > new.span.start {
            return false;
        }
    }
    true
}

fn webrtc(ix: &Ix<'_, '_>, out: &mut Out) {
    let collector = (0..ix.fns.len()).find(|index| {
        let own = &ix.own[*index];
        let creates_peer = own.news.iter().any(|new| {
            is_rtc_constructor(ix, &new.callee)
                || is_rtc_constructor_branch_fallback(ix, *index, *new)
        });
        let opens_channel = own
            .calls
            .iter()
            .any(|call| ix.method(call).is_some_and(|(_, name)| name == "createDataChannel"));
        let starts_offer = own
            .calls
            .iter()
            .any(|call| ix.method(call).is_some_and(|(_, name)| name == "createOffer"));
        creates_peer
            && opens_channel
            && starts_offer
            && own.written_keys.contains("iceServers")
            && own.written_keys.contains("onicecandidate")
    });
    let Some(collector) = collector else {
        return;
    };
    out.function(
        ix,
        "webrtc_ice_candidate_collector",
        collector,
        "creates an RTCPeerConnection with iceServers, installs onicecandidate, opens a data channel and starts an offer",
    );

    candidate_handler(ix, out, collector);
    offer_callbacks(ix, out);
    let failed = collector_guards(ix, out, collector);
    webrtc_resolvers(ix, out, failed);
}

/// The two MAIN aliases of `p10_browser_env`'s deferred settles. The one that
/// records the gathering failure is the rejection settle; the other is the
/// resolution settle. Both are named through their single MAIN alias, because
/// the functions themselves belong to `p10_browser_env`; the alias roles follow
/// the aliased functions' proven p10 roles.
fn webrtc_resolvers(ix: &Ix<'_, '_>, out: &mut Out, failed: Option<SymbolId>) {
    for function in 0..ix.fns.len() {
        if !is_waiter_resolver(ix, function) {
            continue;
        }
        let Some(alias) = ix.main_alias(function) else {
            continue;
        };
        let records_failure = failed.is_some_and(|symbol| {
            ix.own[function].assignments.iter().any(|assignment| {
                target_identifier(&assignment.left)
                    .and_then(|identifier| analysis::identifier_symbol(identifier, ix.scoping))
                    == Some(symbol)
                    && is_true(&assignment.right)
            })
        });
        if records_failure {
            out.binding(
                ix,
                "reject_deferred_result_alias",
                alias,
                "MAIN alias of `rejectDeferredResult` (p10 role `reject_pending_result`): the aliased function writes the gathering-failure flag and replays its queued callbacks through `forEach` + `setTimeout`",
            );
        } else {
            out.binding(
                ix,
                "resolve_deferred_result_alias",
                alias,
                "MAIN alias of `resolveDeferredResult` (p10 role `resolve_pending_result`): the aliased function replays its queued callbacks through `forEach` + `setTimeout` and does not write the gathering-failure flag",
            );
        }
    }
}

fn is_waiter_resolver(ix: &Ix<'_, '_>, function: usize) -> bool {
    let own = &ix.own[function];
    if own.assignments.is_empty() {
        return false;
    }
    own.calls.iter().any(|call| {
        if ix.method(call).map(|(_, name)| name) != Some("forEach") {
            return false;
        }
        ix.callbacks(call).into_iter().any(|callback| {
            ix.own[callback].calls.iter().any(|call| {
                ix.method(call).is_some_and(|(_, name)| name == "setTimeout") && args(call).len() >= 2
            })
        })
    })
}

fn candidate_handler(ix: &Ix<'_, '_>, out: &mut Out, collector: usize) {
    for assignment in &ix.member_assignments {
        if target_member(&assignment.left).map(|(_, name)| name) != Some("onicecandidate") {
            continue;
        }
        if ix.owner[assignment.node_id.get().index()] != Some(collector) {
            continue;
        }
        let Some(handler) = ix.resolve_fn(&assignment.right) else {
            continue;
        };
        let own = &ix.own[handler];
        let parses_candidate = own.members.contains("candidate")
            && own.calls
                .iter()
                .any(|call| ix.method(call).is_some_and(|(_, name)| name == "exec"))
            && own.written_keys.contains("ip")
            && own.written_keys.contains("candidate");
        if !parses_candidate {
            continue;
        }
        out.function(
            ix,
            "webrtc_ice_candidate_handler",
            handler,
            "installed as onicecandidate by the collector; matches candidate text, stores a deduplicated {ip, candidate} entry and closes the peer when gathering ends",
        );
        for call in &own.calls {
            if ix.method(call).is_none_or(|(_, name)| name != "some") {
                continue;
            }
            for callback in ix.callbacks(call) {
                if is_ip_match_predicate(ix, callback) {
                    out.function(
                        ix,
                        "ice_candidate_ip_match_predicate",
                        callback,
                        "the some() predicate that compares a stored entry's `ip` with the parsed address",
                    );
                }
            }
        }
        candidate_patterns(ix, out, handler);
    }
}

fn is_ip_match_predicate(ix: &Ix<'_, '_>, callback: usize) -> bool {
    let Some(value) = ix.single_return(callback) else {
        return false;
    };
    let Expression::BinaryExpression(binary) = strip(value) else {
        return false;
    };
    if binary.operator != BinaryOperator::StrictEquality {
        return false;
    }
    let ip_of_slot = |expression: &Expression<'_>| {
        member(expression).is_some_and(|(object, name)| {
            name == "ip" && ix.slot_value(object, callback) == Some(0)
        })
    };
    ip_of_slot(&binary.left) || ip_of_slot(&binary.right)
}

fn candidate_patterns(ix: &Ix<'_, '_>, out: &mut Out, handler: usize) {
    for call in &ix.own[handler].calls {
        let Some((receiver, "exec")) = ix.method(call) else {
            continue;
        };
        let Some(regex) = ix.symbol(receiver) else {
            continue;
        };
        if ix.declared_outside(regex, handler)
            && ix.origins(regex).is_some_and(|origins| {
                origins.len() == 1 && ix.is_regexp_value(origins[0])
            })
        {
            let Some(result) = ix.initialized_from(handler, call) else {
                continue;
            };
            for index in ix.numeric_reads(handler, result) {
                match index {
                    1 => out.binding(
                        ix,
                        "ice_candidate_ip_pattern",
                        regex,
                        "first capture group of the candidate-text match is stored as the entry's `ip`",
                    ),
                    0 => out.binding(
                        ix,
                        "ice_candidate_type_pattern",
                        regex,
                        "whole match of the candidate-text match is stored as the entry's `candidate`",
                    ),
                    _ => {}
                }
            }
        }
    }
}

fn offer_callbacks(ix: &Ix<'_, '_>, out: &mut Out) {
    for function in 0..ix.fns.len() {
        for call in &ix.own[function].calls {
            let Some((receiver, name)) = ix.method(call) else {
                continue;
            };
            let arguments = args(call);
            if name == "then" {
                let Expression::CallExpression(offer) = strip(receiver) else {
                    continue;
                };
                let promise_form = ix.method(offer).is_some_and(|(_, inner)| inner == "createOffer")
                    && args(offer).is_empty();
                if !promise_form {
                    continue;
                }
                if let Some(callback) = arguments.first().and_then(|value| ix.resolve_fn(value)) {
                    claim_description_applier(
                        ix,
                        out,
                        callback,
                        "ice_offer_local_description_applier",
                        "createOffer promise fulfilment that applies the offer as the local description",
                    );
                }
                if let Some(callback) = arguments.get(1).and_then(|value| ix.resolve_fn(value)) {
                    claim_rejection_reporter(
                        ix,
                        out,
                        callback,
                        "ice_offer_rejection_reporter",
                        "createOffer promise rejection that forwards the reason to the ICE error resolver",
                    );
                }
                continue;
            }
            // The legacy callback form passes the two callbacks to createOffer.
            if name == "createOffer" && arguments.len() == 2 {
                if let Some(callback) = arguments.first().and_then(|value| ix.resolve_fn(value)) {
                    claim_description_applier(
                        ix,
                        out,
                        callback,
                        "ice_offer_legacy_description_applier",
                        "legacy createOffer success callback that applies the offer as the local description",
                    );
                }
                if let Some(callback) = arguments.get(1).and_then(|value| ix.resolve_fn(value)) {
                    claim_rejection_reporter(
                        ix,
                        out,
                        callback,
                        "ice_offer_legacy_rejection_reporter",
                        "legacy createOffer failure callback that forwards the reason to the ICE error resolver",
                    );
                }
            }
        }
    }
}

fn claim_description_applier(
    ix: &Ix<'_, '_>,
    out: &mut Out,
    callback: usize,
    role: &'static str,
    evidence: &str,
) {
    let applies = ix.own[callback]
        .calls
        .iter()
        .any(|call| ix.method(call).is_some_and(|(_, name)| name == "setLocalDescription"));
    if applies {
        out.function(ix, role, callback, evidence);
    }
}

fn claim_rejection_reporter(
    ix: &Ix<'_, '_>,
    out: &mut Out,
    callback: usize,
    role: &'static str,
    evidence: &str,
) {
    if forwards_slot(ix, callback) {
        out.function(ix, role, callback, evidence);
    }
}

/// Calls a captured function with the callback's own first slot.
fn forwards_slot(ix: &Ix<'_, '_>, callback: usize) -> bool {
    ix.own[callback].calls.iter().any(|call| {
        args(call)
            .iter()
            .any(|argument| ix.slot_value(argument, callback) == Some(0))
    })
}

fn collector_guards(ix: &Ix<'_, '_>, out: &mut Out, collector: usize) -> Option<SymbolId> {
    for statement in &ix.own[collector].ifs {
        let Some(symbol) = ix.symbol(&statement.test) else {
            continue;
        };
        if !ix.declared_outside(symbol, collector) {
            continue;
        }
        let consequent = statement.consequent.span();
        let schedules = ix.own[collector].calls.iter().any(|call| {
            ix.method(call).is_some_and(|(_, name)| name == "setTimeout")
                && contains(consequent, call.span)
        });
        if schedules {
            out.binding(
                ix,
                "webrtc_results_ready",
                symbol,
                "the collector replays a queued success or error callback and returns once this flag is set",
            );
            continue;
        }
        let single_return = match &statement.consequent {
            Statement::BlockStatement(block) => {
                matches!(block.body.as_slice(), [Statement::ReturnStatement(_)])
            }
            _ => false,
        };
        let sets_true = ix.own[collector].assignments.iter().any(|assignment| {
            target_identifier(&assignment.left)
                .and_then(|identifier| analysis::identifier_symbol(identifier, ix.scoping))
                == Some(symbol)
                && is_true(&assignment.right)
        });
        if single_return && sets_true {
            out.binding(
                ix,
                "webrtc_enumeration_started",
                symbol,
                "the collector returns early when this flag is set and sets it before it starts gathering",
            );
        }
    }
    gathering_failed(ix, out, collector)
}

fn gathering_failed(ix: &Ix<'_, '_>, out: &mut Out, collector: usize) -> Option<SymbolId> {
    for call in &ix.own[collector].calls {
        if ix.method(call).is_none_or(|(_, name)| name != "setTimeout") {
            continue;
        }
        let Some(callback) = args(call).first().and_then(|value| ix.symbol(value)) else {
            continue;
        };
        let Some(writes) = ix.assigns.get(&callback) else {
            continue;
        };
        if writes.len() != 2 {
            continue;
        }
        let slots: BTreeSet<Option<u32>> = writes
            .iter()
            .map(|assignment| ix.slot_value(&assignment.right, collector))
            .collect();
        if slots.len() != 2 || slots.contains(&None) {
            continue;
        }
        let mut condition: Option<SymbolId> = None;
        for assignment in writes {
            let Some(statement) = ix.enclosing_if(assignment.node_id.get()) else {
                condition = None;
                break;
            };
            let Some(symbol) = ix.symbol(&statement.test) else {
                condition = None;
                break;
            };
            if !ix.declared_outside(symbol, collector) {
                condition = None;
                break;
            }
            match condition {
                None => condition = Some(symbol),
                Some(previous) if previous != symbol => {
                    condition = None;
                    break;
                }
                Some(_) => {}
            }
        }
        if let Some(symbol) = condition {
            out.binding(
                ix,
                "ice_gathering_failed",
                symbol,
                "selects the error callback instead of the success callback when a new caller arrives after ICE gathering failed",
            );
            return Some(symbol);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Memory-attribution formatting
// ---------------------------------------------------------------------------

fn memory_format(ix: &Ix<'_, '_>, out: &mut Out) {
    for function in 0..ix.fns.len() {
        let Some(returned) = ix.single_return(function) else {
            continue;
        };
        let Expression::CallExpression(mapping) = strip(returned) else {
            continue;
        };
        if ix.method(mapping).is_none_or(|(_, name)| name != "map") {
            continue;
        }
        for entry_mapper in ix.callbacks(mapping) {
            let own = &ix.own[entry_mapper];
            if !(own.members.contains("bytes")
                && own.members.contains("attribution")
                && own.members.contains("types"))
            {
                continue;
            }
            let Some(inner) = own
                .calls
                .iter()
                .find(|call| ix.method(call).is_some_and(|(_, name)| name == "map"))
            else {
                continue;
            };
            out.function(
                ix,
                "memory_attribution_formatter",
                function,
                "maps a memory breakdown to {bytes, attribution entries, types}",
            );
            if let Some(alias) = ix.main_alias(function) {
                out.binding(
                    ix,
                    "memory_breakdown_formatter",
                    alias,
                    "main alias of the memory breakdown formatter",
                );
            }
            out.function(
                ix,
                "memory_breakdown_entry_mapper",
                entry_mapper,
                "map callback that maps one breakdown entry's attribution list",
            );
            for attribute_mapper in ix.callbacks(inner) {
                let inner_own = &ix.own[attribute_mapper];
                if inner_own.members.contains("url")
                    && inner_own.members.contains("scope")
                    && inner_own.members.contains("container")
                {
                    out.function(
                        ix,
                        "memory_attribution_mapper",
                        attribute_mapper,
                        "map callback that reduces one attribution to url text, scope and container",
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Browser/platform detection
// ---------------------------------------------------------------------------

fn platform(ix: &Ix<'_, '_>, out: &mut Out) {
    let mut found = None;
    for function in 0..ix.fns.len() {
        for returned in &ix.own[function].returns {
            let Expression::NewExpression(new) = strip(returned) else {
                continue;
            };
            if !matches!(&new.callee, Expression::Identifier(identifier) if identifier.name == "Promise")
            {
                continue;
            }
            for executor in new_arguments(new)
                .into_iter()
                .filter_map(|argument| ix.resolve_fn(argument))
            {
                let own = &ix.own[executor];
                if own.members.contains("vendor")
                    && own.members.contains("userAgent")
                    && own.members.contains("maxTouchPoints")
                {
                    found = Some((function, executor));
                }
            }
        }
    }
    let Some((prober, executor)) = found else {
        return;
    };
    out.function(
        ix,
        "platform_detection_prober",
        prober,
        "returns a Promise whose executor reads navigator.vendor, userAgent and maxTouchPoints",
    );
    out.function(
        ix,
        "platform_detection_executor",
        executor,
        "Promise executor that branches on the navigator fingerprint and resolves a platform code",
    );
    indexed_db_probe(ix, out, executor);
    quota_probes(ix, out, executor);
    platform_result_handlers(ix, out, prober);
}

fn indexed_db_probe(ix: &Ix<'_, '_>, out: &mut Out, executor: usize) {
    for function in 0..ix.fns.len() {
        if ix.fns[function].parent != Some(executor) {
            continue;
        }
        let own = &ix.own[function];
        let opens_database = own
            .calls
            .iter()
            .any(|call| ix.call_name(call).as_deref() == Some("open"));
        if !opens_database || !own.written_keys.contains("onupgradeneeded") {
            continue;
        }
        out.function(
            ix,
            "indexed_db_blob_support_probe",
            function,
            "opens a temporary IndexedDB database, installs onupgradeneeded and deletes the database again",
        );
        for assignment in &ix.member_assignments {
            if target_member(&assignment.left).map(|(_, name)| name) != Some("onupgradeneeded") {
                continue;
            }
            if ix.owner[assignment.node_id.get().index()] != Some(function) {
                continue;
            }
            let Some(upgrade) = ix.resolve_fn(&assignment.right) else {
                continue;
            };
            let upgrade_own = &ix.own[upgrade];
            let puts_blob = upgrade_own
                .calls
                .iter()
                .any(|call| ix.method(call).is_some_and(|(_, name)| name == "createObjectStore"))
                && upgrade_own
                    .calls
                    .iter()
                    .any(|call| ix.method(call).is_some_and(|(_, name)| name == "put"))
                && upgrade_own
                    .news
                    .iter()
                    .any(|new| matches!(&new.callee, Expression::Identifier(id) if id.name == "Blob"));
            if puts_blob {
                out.function(
                    ix,
                    "indexed_db_upgrade_blob_put",
                    upgrade,
                    "onupgradeneeded handler that creates an object store and puts a new Blob into it",
                );
            }
        }
    }
}

fn quota_probes(ix: &Ix<'_, '_>, out: &mut Out, executor: usize) {
    for call in &ix.own[executor].calls {
        if ix.method(call).is_some_and(|(_, name)| name == "queryUsageAndQuota") {
            let arguments = ix.invocation(call).1;
            if let Some(callback) = arguments.first().and_then(|value| ix.resolve_fn(value)) {
                if ix.own[callback].members.contains("jsHeapSizeLimit") {
                    out.function(
                        ix,
                        "quota_usage_comparator",
                        callback,
                        "queryUsageAndQuota success callback that compares the quota with performance.memory.jsHeapSizeLimit",
                    );
                }
            }
            if let Some(callback) = arguments.get(1).and_then(|value| ix.resolve_fn(value)) {
                let reports = ix.own[callback]
                    .news
                    .iter()
                    .any(|new| matches!(&new.callee, Expression::Identifier(id) if id.name == "Error"));
                if reports {
                    out.function(
                        ix,
                        "quota_usage_rejection_reporter",
                        callback,
                        "queryUsageAndQuota error callback that rejects the probe with a new Error",
                    );
                }
            }
        }
        if ix.invoked_name(call) == Some("webkitRequestFileSystem") {
            let arguments = ix.invocation(call).1;
            if let Some(callback) = arguments.get(2).and_then(|value| ix.resolve_fn(value)) {
                if resolves_status_object(ix, callback) {
                    out.function(
                        ix,
                        "file_system_granted_callback",
                        callback,
                        "webkitRequestFileSystem granted callback that resolves the probe result",
                    );
                }
            }
            if let Some(callback) = arguments.get(3).and_then(|value| ix.resolve_fn(value)) {
                if resolves_status_object(ix, callback) {
                    out.function(
                        ix,
                        "file_system_denied_callback",
                        callback,
                        "webkitRequestFileSystem denied callback that resolves the probe result",
                    );
                }
            }
        }
    }
}

fn resolves_status_object(ix: &Ix<'_, '_>, callback: usize) -> bool {
    ix.own[callback].written_keys.contains("a") && !ix.own[callback].calls.is_empty()
}

fn platform_result_handlers(ix: &Ix<'_, '_>, out: &mut Out, prober: usize) {
    for function in 0..ix.fns.len() {
        for call in &ix.own[function].calls {
            let Some((receiver, name)) = ix.method(call) else {
                continue;
            };
            let arguments = args(call);
            if name == "catch" {
                let Expression::CallExpression(chained) = strip(receiver) else {
                    continue;
                };
                let Some((chained_receiver, "then")) = ix.method(chained) else {
                    continue;
                };
                if !calls_prober(ix, chained_receiver, prober) {
                    continue;
                }
                if let Some(callback) = arguments.first().and_then(|value| ix.resolve_fn(value)) {
                    let forwards = ix.own[callback]
                        .calls
                        .iter()
                        .any(|call| ix.method(call).is_some_and(|(_, name)| name == "error"));
                    if forwards {
                        out.function(
                            ix,
                            "platform_result_error_forwarder",
                            callback,
                            "catch handler of the detected-platform promise chain that forwards the reason to the platform result channel",
                        );
                    }
                }
                continue;
            }
            if name == "then" && calls_prober(ix, receiver, prober) {
                if let Some(callback) = arguments.first().and_then(|value| ix.resolve_fn(value)) {
                    let emits = ix.own[callback]
                        .calls
                        .iter()
                        .any(|call| ix.method(call).is_some_and(|(_, name)| name == "next"));
                    if emits {
                        out.function(
                            ix,
                            "platform_result_emitter",
                            callback,
                            "fulfilment of the detected-platform promise that encodes the platform code and marker into the platform result channel",
                        );
                    }
                }
            }
        }
    }
}

fn calls_prober(ix: &Ix<'_, '_>, expression: &Expression<'_>, prober: usize) -> bool {
    match strip(expression) {
        Expression::CallExpression(call) => ix.resolve_fn(&call.callee) == Some(prober),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Memory measurement
// ---------------------------------------------------------------------------

fn memory_measure(ix: &Ix<'_, '_>, out: &mut Out) {
    for function in 0..ix.fns.len() {
        for call in &ix.own[function].calls {
            let Some((receiver, "then")) = ix.method(call) else {
                continue;
            };
            if !is_measure_result(ix, receiver) {
                continue;
            }
            let arguments = args(call);
            let Some(emitter) = arguments.first().and_then(|value| ix.resolve_fn(value)) else {
                continue;
            };
            if !emits_memory_report(ix, emitter) {
                continue;
            }
            out.function(
                ix,
                "memory_attribution_report_emitter",
                emitter,
                "then handler of measureUserAgentSpecificMemory that moves the current document's entry first and encodes the breakdown",
            );
            memory_report_helpers(ix, out, emitter);
            for chained in &ix.own[function].calls {
                let Some((chained_receiver, "catch")) = ix.method(chained) else {
                    continue;
                };
                if strip(chained_receiver).span() != call.span {
                    continue;
                }
                let Some(callback) = args(chained).first().and_then(|value| ix.resolve_fn(value))
                else {
                    continue;
                };
                let forwards = ix.own[callback]
                    .calls
                    .iter()
                    .any(|call| ix.method(call).is_some_and(|(_, name)| name == "error"));
                if forwards {
                    out.function(
                        ix,
                        "memory_measure_error_forwarder",
                        callback,
                        "catch handler of the measureUserAgentSpecificMemory chain that forwards the reason to the memory channel",
                    );
                }
            }
        }
    }
}

fn is_measure_result(ix: &Ix<'_, '_>, expression: &Expression<'_>) -> bool {
    match strip(expression) {
        Expression::CallExpression(call) => ix
            .method(call)
            .is_some_and(|(_, name)| name == "measureUserAgentSpecificMemory"),
        Expression::Identifier(_) => ix
            .symbol(expression)
            .and_then(|symbol| ix.single_origin(symbol))
            .is_some_and(|origin| is_measure_result(ix, origin)),
        _ => false,
    }
}

fn emits_memory_report(ix: &Ix<'_, '_>, emitter: usize) -> bool {
    let own = &ix.own[emitter];
    let has = |wanted: &str| own.calls.iter().any(|call| ix.method(call).is_some_and(|(_, name)| name == wanted));
    has("next") && has("complete") && has("filter") && has("unshift") && has("splice")
}

fn memory_report_helpers(ix: &Ix<'_, '_>, out: &mut Out, emitter: usize) {
    let own = &ix.own[emitter];
    for call in &own.calls {
        let Some((_, name)) = ix.method(call) else {
            continue;
        };
        match name {
            "filter" => {
                for filter in ix.callbacks(call) {
                    let Some(returned) = ix.single_return(filter) else {
                        continue;
                    };
                    let Expression::CallExpression(some) = strip(returned) else {
                        continue;
                    };
                    if ix.method(some).is_none_or(|(receiver, name)| {
                        name != "some"
                            || member(receiver).is_none_or(|(_, member_name)| member_name != "attribution")
                    }) {
                        continue;
                    }
                    out.function(
                        ix,
                        "memory_breakdown_attribution_filter",
                        filter,
                        "breakdown filter that keeps entries with an attribution from the current document or a Window/Worker scope",
                    );
                    for predicate in ix.callbacks(some) {
                        let predicate_own = &ix.own[predicate];
                        if predicate_own.members.contains("url")
                            && predicate_own.members.contains("scope")
                        {
                            out.function(
                                ix,
                                "memory_attribution_current_document_test",
                                predicate,
                                "some() predicate that compares the attribution url with the current location and falls back to the attribution scope",
                            );
                        }
                    }
                }
            }
            "forEach" => {
                for scanner in ix.callbacks(call) {
                    let inner_own = &ix.own[scanner];
                    let Some(inner) = inner_own
                        .calls
                        .iter()
                        .find(|call| ix.method(call).is_some_and(|(_, name)| name == "forEach"))
                    else {
                        continue;
                    };
                    out.function(
                        ix,
                        "memory_breakdown_attribution_scanner",
                        scanner,
                        "outer forEach that scans one breakdown entry's attribution list",
                    );
                    for writer in ix.callbacks(inner) {
                        let writes_index = ix.own[writer].assignments.iter().any(|assignment| {
                            target_identifier(&assignment.left)
                                .and_then(|identifier| {
                                    analysis::identifier_symbol(identifier, ix.scoping)
                                })
                                .is_some_and(|symbol| ix.declared_outside(symbol, writer))
                        });
                        if writes_index {
                            out.function(
                                ix,
                                "memory_breakdown_entry_index_writer",
                                writer,
                                "inner forEach that remembers the index of the entry whose attribution url is the current location",
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Media-feature range probes
// ---------------------------------------------------------------------------

fn media_features(ix: &Ix<'_, '_>, out: &mut Out) {
    media_aliases(ix, out);
    for function in 0..ix.fns.len() {
        let own = &ix.own[function];
        let match_media: Vec<&CallExpression<'_>> = own
            .calls
            .iter()
            .copied()
            .filter(|call| ix.method(call).is_some_and(|(_, name)| name == "matchMedia"))
            .collect();
        if match_media.len() != 1 {
            continue;
        }
        let Some(serializer) = own
            .calls
            .iter()
            .filter(|call| ix.method(call).is_some_and(|(_, name)| name == "setTimeout"))
            .flat_map(|call| ix.callbacks(call))
            .find(|callback| {
                let inner = &ix.own[*callback];
                let has = |wanted: &str| {
                    inner
                        .calls
                        .iter()
                        .any(|call| ix.method(call).is_some_and(|(_, name)| name == wanted))
                };
                has("next") && has("complete")
            })
        else {
            continue;
        };
        let completes = own
            .calls
            .iter()
            .any(|call| ix.method(call).is_some_and(|(_, name)| name == "complete"));
        if !completes {
            continue;
        }
        out.function(
            ix,
            "media_feature_range_prober",
            function,
            "probes media-feature ranges through matchMedia and schedules the serializer",
        );
        out.function(
            ix,
            "media_feature_range_serializer",
            serializer,
            "setTimeout callback of the media-feature prober that encodes the probed feature values and completes the channel",
        );
        probe_completion(ix, out, function);
    }
}

/// The MAIN aliases of the two media-feature helpers owned by
/// `p10_browser_env`: `measureMaxMediaFeatureValue` (bisection) and
/// `evaluateMediaFeatureValues` (a `map` over `matchMedia`). Only their aliases
/// are claimed here; the roles follow the aliased functions' proven p10 roles.
fn media_aliases(ix: &Ix<'_, '_>, out: &mut Out) {
    for function in 0..ix.fns.len() {
        let own = &ix.own[function];
        let bisects = own
            .calls
            .iter()
            .any(|call| ix.method(call).is_some_and(|(_, name)| name == "floor"));
        let probes = own
            .calls
            .iter()
            .any(|call| ix.method(call).is_some_and(|(_, name)| name == "matchMedia"));
        if probes && bisects {
            if let Some(alias) = ix.main_alias(function) {
                out.binding(
                    ix,
                    "measure_max_media_feature_value_alias",
                    alias,
                    "MAIN alias of `measureMaxMediaFeatureValue` (p10 role `measure_max_media_feature`): the aliased function calls `matchMedia` and narrows the range with `Math.floor`",
                );
            }
        }
        let Some(returned) = ix.single_return(function) else {
            continue;
        };
        let Expression::CallExpression(map) = strip(returned) else {
            continue;
        };
        let tests_feature = ix
            .callbacks(map)
            .into_iter()
            .any(|callback| ix.own[callback].members.contains("matchMedia"));
        if ix.method(map).is_some_and(|(_, name)| name == "map") && tests_feature {
            if let Some(alias) = ix.main_alias(function) {
                out.binding(
                    ix,
                    "evaluate_media_feature_values_alias",
                    alias,
                    "MAIN alias of `evaluateMediaFeatureValues` (p10 role `evaluate_media_feature_values`): the aliased function returns a `map` call whose callback reads `matchMedia`",
                );
            }
        }
    }
}

fn probe_completion(ix: &Ix<'_, '_>, out: &mut Out, prober: usize) {
    let Some(parent) = ix.fns[prober].parent else {
        return;
    };
    for call in &ix.own[parent].calls {
        let arguments = ix.invocation(call).1;
        let position = arguments
            .iter()
            .position(|argument| ix.resolve_fn(argument) == Some(prober));
        if position != Some(4) {
            continue;
        }
        let Some(callback) = arguments.get(5).and_then(|value| ix.resolve_fn(value)) else {
            continue;
        };
        let own = &ix.own[callback];
        if own.calls.len() == 1
            && ix
                .method(own.calls[0])
                .is_some_and(|(_, name)| name == "complete")
            && own.returns.is_empty()
        {
            out.function(
                ix,
                "media_feature_probe_completion",
                callback,
                "sixth argument of the media-feature prober call: completes the channel without a value",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// WebGL report collection and worker protocol
// ---------------------------------------------------------------------------

fn webgl(ix: &Ix<'_, '_>, out: &mut Out) {
    webgl_parameter_map(ix, out);
    let collector = webgl_collector(ix, out);
    webgl_worker(ix, out);
    if let Some(collector) = collector {
        for call in &ix.own[collector].calls {
            if ix.method(call).is_none_or(|(_, name)| name != "map") {
                continue;
            }
            for callback in ix.callbacks(call) {
                let own = &ix.own[callback];
                if own.typeof_members.contains("join") {
                    out.function(
                        ix,
                        "webgl_dimension_values_joiner",
                        callback,
                        "map callback that joins each canvas dimension pair, else returns an empty string",
                    );
                }
                if own.members.contains("precision")
                    && own.members.contains("rangeMin")
                    && own.members.contains("rangeMax")
                {
                    out.function(
                        ix,
                        "webgl_precision_range_joiner",
                        callback,
                        "map callback that joins precision, rangeMin and rangeMax of a shader precision format",
                    );
                }
            }
        }
    }
}

fn webgl_parameter_map(ix: &Ix<'_, '_>, out: &mut Out) {
    for function in 0..ix.fns.len() {
        let Some(returned) = ix.single_return(function) else {
            continue;
        };
        let Some(accumulator) = ix.symbol(returned) else {
            continue;
        };
        let created = ix
            .origins(accumulator)
            .is_some_and(|origins| origins.iter().any(|origin| ix.is_create_null(origin)));
        if !created {
            continue;
        }
        for call in &ix.own[function].calls {
            if ix.method(call).is_none_or(|(_, name)| name != "forEach") {
                continue;
            }
            for writer in ix.callbacks(call) {
                if copies_by_name(ix, writer, accumulator) {
                    out.function(
                        ix,
                        "webgl_parameter_map_builder",
                        function,
                        "builds a null-prototype map of parameter name to parameter value",
                    );
                    out.function(
                        ix,
                        "webgl_parameter_map_writer",
                        writer,
                        "forEach callback that writes values[index] under names[index]",
                    );
                }
            }
        }
    }
}

fn copies_by_name(ix: &Ix<'_, '_>, writer: usize, accumulator: SymbolId) -> bool {
    let own = &ix.own[writer];
    let [assignment] = own.assignments.as_slice() else {
        return false;
    };
    let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
        return false;
    };
    if ix.symbol(&target.object) != Some(accumulator)
        || ix.slot_value(&target.expression, writer) != Some(0)
    {
        return false;
    }
    let Expression::ComputedMemberExpression(source) = strip(&assignment.right) else {
        return false;
    };
    ix.symbol(&source.object) != Some(accumulator)
        && ix.slot_value(&source.expression, writer) == Some(1)
}

fn webgl_collector(ix: &Ix<'_, '_>, out: &mut Out) -> Option<usize> {
    let collector = (0..ix.fns.len()).find(|function| {
        let own = &ix.own[*function];
        let context = own.calls.iter().any(|call| {
            ix.call_name(call).as_deref() == Some("getContext")
                && args(call).iter().any(|argument| {
                    matches!(string_value(argument), Some("webgl") | Some("experimental-webgl"))
                })
        });
        let parameters = own
            .calls
            .iter()
            .any(|call| ix.method(call).is_some_and(|(_, name)| name == "getParameter"));
        let emits = own
            .calls
            .iter()
            .any(|call| ix.method(call).is_some_and(|(_, name)| name == "next"))
            && own
                .calls
                .iter()
                .any(|call| ix.method(call).is_some_and(|(_, name)| name == "complete"));
        context
            && parameters
            && emits
            && own.written_keys.contains("shaderPrecisions")
            && own.written_keys.contains("supportedExtensions")
    });
    if let Some(collector) = collector {
        out.function(
            ix,
            "webgl_report_collector",
            collector,
            "creates a canvas and a webgl context, reads getParameter values, emits the encoded report and completes the channel",
        );
    }
    collector
}

fn webgl_worker(ix: &Ix<'_, '_>, out: &mut Out) {
    let mut message_objects: BTreeSet<SymbolId> = BTreeSet::new();
    for assignment in &ix.member_assignments {
        let Some((object, "onmessage")) = target_member(&assignment.left) else {
            continue;
        };
        let Some(handler) = ix.resolve_fn(&assignment.right) else {
            continue;
        };
        let own = &ix.own[handler];
        let has = |wanted: &str| {
            own.calls
                .iter()
                .any(|call| ix.method(call).is_some_and(|(_, name)| name == wanted))
        };
        let webgl_protocol = own.members.contains("parameters")
            && own.written_keys.contains("shaderPrecisions")
            && own.members.contains("segment")
            && has("next")
            && has("terminate");
        if !webgl_protocol {
            continue;
        }
        out.function(
            ix,
            "webgl_worker_message_handler",
            handler,
            "onmessage handler that merges worker segments into the WebGL report, emits it and terminates the worker after the last segment",
        );
        if let Some(symbol) = ix.symbol(object) {
            message_objects.insert(symbol);
        }
        for call in &own.calls {
            if ix.method(call).is_none_or(|(_, name)| name != "map") {
                continue;
            }
            for callback in ix.callbacks(call) {
                if ix.own[callback].typeof_members.contains("join") {
                    out.function(
                        ix,
                        "webgl_worker_dimension_joiner",
                        callback,
                        "map callback in the worker message handler that joins each dimension pair, else returns an empty string",
                    );
                }
            }
        }
    }
    for assignment in &ix.member_assignments {
        let Some((object, "onerror")) = target_member(&assignment.left) else {
            continue;
        };
        let Some(symbol) = ix.symbol(object) else {
            continue;
        };
        if !message_objects.contains(&symbol) {
            continue;
        }
        let Some(handler) = ix.resolve_fn(&assignment.right) else {
            continue;
        };
        if ix.own[handler].calls.len() == 1 {
            out.function(
                ix,
                "webgl_worker_error_handler",
                handler,
                "onerror handler of the WebGL worker that falls back to the synchronous collection",
            );
        }
    }
    webgl_cleanup(ix, out);
}

fn webgl_cleanup(ix: &Ix<'_, '_>, out: &mut Out) {
    for function in 0..ix.fns.len() {
        let own = &ix.own[function];
        if own.calls.len() != 2 || !own.returns.is_empty() {
            continue;
        }
        let mut terminates = None;
        let mut completes = None;
        for call in &own.calls {
            match ix.method(call).map(|(_, name)| name) {
                Some("terminate") => terminates = Some(call),
                Some("complete") => completes = Some(call),
                _ => {}
            }
        }
        let (Some(terminate), Some(_complete)) = (terminates, completes) else {
            continue;
        };
        let Some(statement) = ix.enclosing_if(terminate.node_id.get()) else {
            continue;
        };
        let Expression::BinaryExpression(test) = strip(&statement.test) else {
            continue;
        };
        if test.operator != BinaryOperator::Inequality || !test.right.is_null() {
            continue;
        }
        out.function(
            ix,
            "webgl_worker_cleanup",
            function,
            "terminates the WebGL worker when it exists and completes the stream channel",
        );
    }
}
