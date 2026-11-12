//! Package `p04_collector_wiring` finders: the signal constructors, the group
//! coordinator, collection start-up, per-slot observers, the last-chance
//! notifier table, the request snapshot and the wiring tables they read.
//!
//! Recognition is top-down from stable collector ABI structure, never from
//! spellings: the four closure-storing registrars handed to a collector task,
//! the teardown that swaps two of the stored routines for a no-op, and the
//! `next`/`error`/`complete`, `ctor`/`priority`/`debugName` and
//! `signalFilter`/`notifyLastChance` property vocabularies. Every read works
//! on both tracer shapes: inline arrows and named formals, or hoisted named
//! callbacks, `arguments[N]`, `["prop"]` and `.call(void 0, ...)`.
//!
//! A function nested in a recognized owner is claimed only after the owner is
//! proven, and each nested role is proven by what that function itself does.
//! Parallel copies of one role in different scopes are legitimate, so a role
//! may be claimed for several function nodes.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

use crate::analysis;

use crate::signals::extract::bindings::{
    call_arguments, new_arguments, object_members_of, static_member_name, FunctionSite, Model,
};
use crate::signals::schema::{Confidence, ProgramPropertiesLimits};
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole {
        role,
        name,
        kind: FinderRoleKind::Function,
    }
}

const fn alias(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole {
        role,
        name,
        kind: FinderRoleKind::FunctionAlias,
    }
}

const fn main_binding(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole {
        role,
        name,
        kind: FinderRoleKind::MainBinding,
    }
}

const CTOR_FACTORY_CONTEXT: &str = "signal_ctor_factory_context_task";
const CTOR_FACTORY_OBSERVER: &str = "signal_ctor_factory_observer_task";
const CTOR_CONTEXT: &str = "signal_ctor_context_task";
const CTOR_OBSERVER: &str = "signal_ctor_observer_task";
const EXECUTOR_CONTEXT: &str = "signal_executor_context_task";
const EXECUTOR_OBSERVER: &str = "signal_executor_observer_task";
const DISPOSE: &str = "signal_subscription_dispose";
const REGISTER_INIT: &str = "signal_routine_register_init";
const REGISTER_EMIT: &str = "signal_routine_register_emit";
const REGISTER_LAST_CHANCE: &str = "signal_routine_register_last_chance";
const REGISTER_CLEANUP: &str = "signal_routine_register_cleanup";
const EMIT_IMMEDIATE: &str = "signal_emit_thunk_immediate";
const EMIT_TIMED: &str = "signal_emit_thunk_timed";
const LAST_CHANCE_UNLABELED: &str = "signal_last_chance_guard_unlabeled";
const LAST_CHANCE_LABELED: &str = "signal_last_chance_guard_labeled";

const COORDINATOR: &str = "signal_group_coordinator";
const COORDINATOR_ALIAS: &str = "signal_group_coordinator_alias";
const GROUP_SAFE_RUN: &str = "signal_group_safe_run";
const GROUP_FORWARD_ERROR: &str = "signal_group_forward_error";
const GROUP_CAPTURE: &str = "signal_group_capture_subscriber";
const GROUP_NOOP: &str = "signal_group_noop_unsubscribe";
const GROUP_LAST_CHANCE: &str = "signal_group_last_chance_thunk";
const GROUP_UNSUBSCRIBE: &str = "signal_group_unsubscribe";

const OBSERVER_NEXT: &str = "signal_observer_next";
const OBSERVER_ERROR: &str = "signal_observer_error";
const OBSERVER_COMPLETE: &str = "signal_observer_complete";

const SYNC_TASK_RUNNER: &str = "signal_sync_task_runner";
const START_DIRECT: &str = "signal_start_direct_descriptor";
const DIRECT_NOTIFIER: &str = "signal_direct_notifier_registrar";
const DIRECT_UNSUBSCRIBE: &str = "signal_direct_unsubscribe";
const START_GROUP: &str = "signal_start_group";
const GROUP_NOTIFIER: &str = "signal_group_notifier_registrar";

const STATUS_RELAY: &str = "signal_status_listener_relay";
const STATUS_IS_COLLECTED: &str = "signal_status_is_collected";
const STATUS_IS_ERRORED: &str = "signal_status_is_errored";

const ERROR_REPORT: &str = "signal_error_report";
const ERROR_REPORT_ALIAS: &str = "signal_error_report_alias";

const NOTIFY_HOOKS: &str = "signal_notify_last_chance_hooks";
const NOTIFY_ENTRY: &str = "signal_notify_last_chance_entry";
const PAYLOAD_NAMED_VALUE: &str = "signal_payload_named_value";
const PAYLOAD_VALUE: &str = "signal_payload_value";
const NOTIFIER_REGISTER: &str = "signal_notifier_register";
const NOTIFIER_REGISTER_ALIAS: &str = "signal_notifier_register_alias";

const VALUE_CELL_INIT: &str = "signal_value_cell_init";
const DEBUG_NAME_READ: &str = "signal_debug_name_read";
const STATUS_RECORD_INIT: &str = "signal_status_record_init";

const SHA_HASH: &str = "scr_chunked_sha256_hash";
const SHA_HASH_ALIAS: &str = "scr_chunked_sha256_hash_alias";
const SHA_PAIR: &str = "scr_digest_byte_pair";
const SHA_OUTER: &str = "scr_digest_concatenated_digests";
const SHA_APPEND: &str = "scr_append_digest_bytes";
const SHA_RESULT: &str = "scr_format_hash_result";
const SHA_HEX: &str = "scr_byte_to_hex_pair";
const SHA_FAILURE: &str = "scr_discard_hash_failure";

const NOOP_ROUTINE: &str = "signal_noop_routine";
const SAFE_RUNNER: &str = "signal_safe_runner_ref";
const OBJECT_MERGE: &str = "signal_context_merge_ref";
const ERROR_SINK: &str = "signal_error_record_sink_ref";
const WORK_SCHEDULER: &str = "signal_work_scheduler";
const STATUS_LISTENERS: &str = "signal_status_listeners";

/// Every role this package can claim, with the one name it receives.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    function(CTOR_FACTORY_CONTEXT, "bindContextTaskToSignalCtor"),
    function(CTOR_FACTORY_OBSERVER, "bindObserverTaskToSignalCtor"),
    function(CTOR_CONTEXT, "contextTaskSignalCtor"),
    function(CTOR_OBSERVER, "observerTaskSignalCtor"),
    function(EXECUTOR_CONTEXT, "runContextTaskSignal"),
    function(EXECUTOR_OBSERVER, "runObserverTaskSignal"),
    function(DISPOSE, "disposeSignalSubscription"),
    function(REGISTER_INIT, "registerSignalInitRoutine"),
    function(REGISTER_EMIT, "registerSignalEmitRoutine"),
    function(REGISTER_LAST_CHANCE, "registerSignalLastChanceRoutine"),
    function(REGISTER_CLEANUP, "registerSignalCleanupRoutine"),
    function(EMIT_IMMEDIATE, "runSignalEmitImmediately"),
    function(EMIT_TIMED, "runSignalEmitOnTimedQueue"),
    function(LAST_CHANCE_UNLABELED, "runUnlabeledSignalLastChance"),
    function(LAST_CHANCE_LABELED, "runLabeledSignalLastChance"),
    function(COORDINATOR, "coordinateObservableSubscriptions"),
    alias(COORDINATOR_ALIAS, "observableSubscriptionCoordinator"),
    function(GROUP_SAFE_RUN, "runGroupRoutineSafely"),
    function(GROUP_FORWARD_ERROR, "forwardErrorToGroupHandle"),
    function(GROUP_CAPTURE, "captureGroupSubscriber"),
    function(GROUP_NOOP, "noopGroupUnsubscribe"),
    function(GROUP_LAST_CHANCE, "runGroupLastChanceSafely"),
    function(GROUP_UNSUBSCRIBE, "unsubscribeSignalGroup"),
    function(OBSERVER_NEXT, "recordSignalValue"),
    function(OBSERVER_ERROR, "recordSignalError"),
    function(OBSERVER_COMPLETE, "recordSignalBytesOnComplete"),
    function(SYNC_TASK_RUNNER, "invokeTaskSynchronously"),
    function(START_DIRECT, "startDirectSignalDescriptor"),
    function(DIRECT_NOTIFIER, "registerDirectSignalNotifier"),
    function(DIRECT_UNSUBSCRIBE, "unsubscribeDirectSignal"),
    function(START_GROUP, "startSignalGroup"),
    function(GROUP_NOTIFIER, "registerGroupSignalNotifier"),
    function(STATUS_RELAY, "relaySignalStatusToListener"),
    function(STATUS_IS_COLLECTED, "isSignalCollected"),
    function(STATUS_IS_ERRORED, "isSignalErrored"),
    function(ERROR_REPORT, "reportSignalError"),
    alias(ERROR_REPORT_ALIAS, "signalErrorLogger"),
    function(NOTIFY_HOOKS, "notifyLastChanceSignalHooks"),
    function(NOTIFY_ENTRY, "invokeLastChanceEntry"),
    function(PAYLOAD_NAMED_VALUE, "readNamedSignalValue"),
    function(PAYLOAD_VALUE, "readSignalValue"),
    function(NOTIFIER_REGISTER, "registerSignalNotifier"),
    alias(NOTIFIER_REGISTER_ALIAS, "signalNotifierRegistrar"),
    function(VALUE_CELL_INIT, "createEmptySignalValueCell"),
    function(DEBUG_NAME_READ, "readSignalDebugName"),
    function(STATUS_RECORD_INIT, "createSignalStatusRecord"),
    function(SHA_HASH, "computeChunkedSha256Hash"),
    alias(SHA_HASH_ALIAS, "chunkedSha256Hasher"),
    function(SHA_PAIR, "digestSignalBytePair"),
    function(SHA_OUTER, "digestConcatenatedDigests"),
    function(SHA_APPEND, "appendDigestBytes"),
    function(SHA_RESULT, "formatChunkedHashResult"),
    function(SHA_HEX, "byteToHexPair"),
    function(SHA_FAILURE, "discardHashFailure"),
    main_binding(NOOP_ROUTINE, "noopSignalRoutine"),
    main_binding(SAFE_RUNNER, "signalTaskSafeRunner"),
    main_binding(OBJECT_MERGE, "objectPropertyAssigner"),
    main_binding(ERROR_SINK, "errorRecordReporter"),
    main_binding(WORK_SCHEDULER, "signalWorkScheduler"),
    main_binding(STATUS_LISTENERS, "signalStatusListeners"),
];

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let Ok(model) = Model::new(
        ctx.source,
        ctx.program,
        ctx.scoping,
        ctx.nodes,
        ProgramPropertiesLimits::default(),
    ) else {
        return Vec::new();
    };
    let mut scan = Scan::new(&model);
    scan.run();
    scan.claims
}

// ===========================================================================
// One function's own body
// ===========================================================================

/// What one function's own body holds; nested functions are separate units.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    /// `return` arguments and arrow expression bodies; `return void 0` is out.
    returns: Vec<&'a Expression<'a>>,
    assigns: Vec<&'a AssignmentExpression<'a>>,
    declarators: Vec<&'a VariableDeclarator<'a>>,
    /// `(property, object)` of every static-name member read or write.
    members: Vec<(String, &'a Expression<'a>)>,
    nested: Vec<FunctionSite>,
    tries: usize,
}

/// A call with `.call(...)` indirection removed, so `f.call(void 0, a)` reads
/// as `f(a)` and `r.m.call(r, a)` as `r.m(a)`.
struct Call<'a> {
    callee: &'a Expression<'a>,
    method: Option<&'a str>,
    receiver: Option<&'a Expression<'a>>,
    args: Vec<&'a Expression<'a>>,
}

fn member_object<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

fn is_void(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void
    )
}

fn normal_call<'a>(call: &'a CallExpression<'a>) -> Call<'a> {
    let mut args = call_arguments(call);
    let callee = call.callee.without_parentheses();
    if static_member_name(callee) == Some("call") {
        if let Some(object) = member_object(callee) {
            let object = object.without_parentheses();
            if args.first().is_some_and(|first| is_void(first)) {
                args.remove(0);
                return plain_call(object, args);
            }
            if let (Some(method), false) = (static_member_name(object), args.is_empty()) {
                let receiver = args.remove(0);
                return Call {
                    callee: object,
                    method: Some(method),
                    receiver: Some(receiver),
                    args,
                };
            }
        }
    }
    plain_call(callee, args)
}

fn plain_call<'a>(callee: &'a Expression<'a>, args: Vec<&'a Expression<'a>>) -> Call<'a> {
    Call {
        callee,
        method: static_member_name(callee),
        receiver: member_object(callee).map(Expression::without_parentheses),
        args,
    }
}

fn is_function_kind(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
    )
}

fn target_key(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => Some(member.property.name.to_string()),
        AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        },
        _ => None,
    }
}

// ===========================================================================
// Scan
// ===========================================================================

/// The closure state one set of four registrars fills.
struct Routines {
    init_register: FunctionSite,
    emit_register: FunctionSite,
    last_chance_register: FunctionSite,
    cleanup_register: FunctionSite,
    last_chance: SymbolId,
    /// The stored emit routine and any local that copies it.
    emit: BTreeSet<SymbolId>,
    /// The flag stored with the emit routine: true selects the immediate queue.
    route_flag: SymbolId,
    /// The no-op a teardown swaps into the init and last-chance references.
    noop: SymbolId,
}

struct Scan<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    own: HashMap<NodeId, Own<'a>>,
    seen: BTreeSet<(u32, u32)>,
    /// Function spans proven as group coordinators.
    coordinators: BTreeSet<(u32, u32)>,
    claims: Vec<FinderClaim>,
}

impl<'m, 's, 'a> Scan<'m, 's, 'a> {
    fn new(model: &'m Model<'s, 'a>) -> Self {
        let nodes = model.nodes();
        let mut owners: HashMap<NodeId, NodeId> = HashMap::new();
        let mut own: HashMap<NodeId, Own<'a>> = HashMap::new();
        // The semantic pass numbers a parent before its children.
        for (id, node) in nodes.iter_enumerated() {
            let parent = nodes.parent_id(id);
            let owner = if parent == id {
                None
            } else if is_function_kind(nodes.kind(parent)) {
                Some(parent)
            } else {
                owners.get(&parent).copied()
            };
            if let Some(owner) = owner {
                owners.insert(id, owner);
            }
            let kind = node.kind();
            if let AstKind::ArrowFunctionExpression(arrow) = kind {
                if arrow.expression {
                    if let Some(Statement::ExpressionStatement(statement)) =
                        arrow.body.statements.first()
                    {
                        own.entry(id).or_default().returns.push(&statement.expression);
                    }
                }
            }
            let Some(owner) = owner else {
                continue;
            };
            let entry = own.entry(owner).or_default();
            match kind {
                AstKind::CallExpression(call) => entry.calls.push(call),
                AstKind::NewExpression(call) => entry.news.push(call),
                AstKind::ReturnStatement(statement) => {
                    if let Some(argument) = statement.argument.as_ref() {
                        if !is_void(argument) {
                            entry.returns.push(argument);
                        }
                    }
                }
                AstKind::AssignmentExpression(assignment) => entry.assigns.push(assignment),
                AstKind::VariableDeclarator(declarator) => entry.declarators.push(declarator),
                AstKind::StaticMemberExpression(member) => {
                    entry.members.push((member.property.name.to_string(), &member.object));
                }
                AstKind::ComputedMemberExpression(member) => {
                    if let Expression::StringLiteral(literal) = &member.expression {
                        entry.members.push((literal.value.to_string(), &member.object));
                    }
                }
                AstKind::TryStatement(_) => entry.tries += 1,
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {
                    if let Some(site) = model.site_by_span(kind.span()) {
                        entry.nested.push(site);
                    }
                }
                _ => {}
            }
        }
        Self {
            model,
            own,
            seen: BTreeSet::new(),
            coordinators: BTreeSet::new(),
            claims: Vec::new(),
        }
    }

    fn run(&mut self) {
        let observer = self.find_observer();
        self.find_ctors();
        self.find_coordinators();
        let start = self.find_start();
        self.find_status_relay();
        self.find_error_report();
        self.find_notifiers();
        self.find_payload_readers();
        if let (Some(start), Some(observer)) = (&start, &observer) {
            self.find_store_initializers(start.descriptors, observer);
        }
        self.find_chunked_hash();
    }

    // -----------------------------------------------------------------------
    // Readers
    // -----------------------------------------------------------------------

    fn own(&self, site: FunctionSite) -> Option<&Own<'a>> {
        self.own.get(&site.node_id)
    }

    fn calls(&self, site: FunctionSite) -> Vec<Call<'a>> {
        self.own(site)
            .map(|own| own.calls.iter().map(|call| normal_call(call)).collect())
            .unwrap_or_default()
    }

    fn nested(&self, site: FunctionSite) -> Vec<FunctionSite> {
        self.own(site).map(|own| own.nested.clone()).unwrap_or_default()
    }

    fn returns(&self, site: FunctionSite) -> Vec<&'a Expression<'a>> {
        self.own(site).map(|own| own.returns.clone()).unwrap_or_default()
    }

    fn reads_member(&self, site: FunctionSite, name: &str) -> bool {
        self.own(site)
            .is_some_and(|own| own.members.iter().any(|(member, _)| member == name))
    }

    fn symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        let Expression::Identifier(identifier) = expression.without_parentheses() else {
            return None;
        };
        analysis::identifier_symbol(identifier, self.model.scoping())
    }

    /// The value a local stands for, when it has exactly one write.
    fn origin(&self, expression: &'a Expression<'a>) -> &'a Expression<'a> {
        let mut current = expression.without_parentheses();
        for _ in 0..4 {
            let Some(symbol) = self.symbol(current) else {
                break;
            };
            if self.model.main_symbols.contains(&symbol) {
                break;
            }
            let writes = usize::from(self.model.initializer(symbol).is_some())
                + self.model.binding_writes(symbol);
            if writes != 1 {
                break;
            }
            let Some(value) = self.model.symbol_value(symbol) else {
                break;
            };
            current = value.without_parentheses();
        }
        current
    }

    fn is_slot(&self, expression: &Expression<'_>, site: FunctionSite, slot: u32) -> bool {
        self.model.site_slot_value(expression, site) == Some(slot)
    }

    fn function_of(&self, expression: &Expression<'_>) -> Option<FunctionSite> {
        self.model.function_from_expression(expression)
    }

    /// The closure variables a registrar stores its slots in, in slot order.
    /// A registrar does nothing else.
    fn registrar_writes(&self, site: FunctionSite) -> Option<Vec<SymbolId>> {
        let own = self.own(site)?;
        if !own.calls.is_empty()
            || !own.news.is_empty()
            || !own.nested.is_empty()
            || !own.returns.is_empty()
            || !own.declarators.is_empty()
            || own.assigns.is_empty()
        {
            return None;
        }
        let mut writes = BTreeMap::new();
        for assignment in &own.assigns {
            if assignment.operator != AssignmentOperator::Assign {
                return None;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                return None;
            };
            let symbol = analysis::identifier_symbol(target, self.model.scoping())?;
            let slot = self.model.site_parameter_slot(&assignment.right, site)?;
            writes.insert(slot, symbol);
        }
        let in_order = writes.keys().copied().eq(0..writes.len() as u32);
        in_order.then(|| writes.into_values().collect())
    }

    /// Whether any reference to `symbol` lies in a function other than `exec`
    /// and the listed ones.
    fn read_elsewhere(
        &self,
        symbol: SymbolId,
        exec: FunctionSite,
        excluded: &[FunctionSite],
    ) -> bool {
        self.model.scoping().get_resolved_references(symbol).any(|reference| {
            self.model.owning_site(reference.node_id()).is_some_and(|site| {
                site.span != exec.span && excluded.iter().all(|other| other.span != site.span)
            })
        })
    }

    /// Whether `node` lies in the consequent of an `if (symbol)` inside its
    /// owner chain.
    fn under_test(&self, node: NodeId, symbol: SymbolId) -> bool {
        let nodes = self.model.nodes();
        let span = nodes.kind(node).span();
        let mut current = node;
        for _ in 0..64 {
            let parent = nodes.parent_id(current);
            if parent == current {
                return false;
            }
            current = parent;
            if let AstKind::IfStatement(statement) = nodes.kind(current) {
                let inside = statement.consequent.span();
                if self.symbol(&statement.test) == Some(symbol)
                    && inside.start <= span.start
                    && span.end <= inside.end
                {
                    return true;
                }
            }
        }
        false
    }

    /// Which side of `if (slot == null)` holds `node`, for a slot of `ctor`:
    /// true for the consequent.
    fn label_branch(&self, node: NodeId, ctor: FunctionSite) -> Option<bool> {
        let nodes = self.model.nodes();
        let span = nodes.kind(node).span();
        let mut current = node;
        for _ in 0..64 {
            let parent = nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            let AstKind::IfStatement(statement) = nodes.kind(current) else {
                continue;
            };
            let Expression::BinaryExpression(test) = statement.test.without_parentheses() else {
                continue;
            };
            if test.operator != BinaryOperator::Equality
                || !test.right.is_null()
                || self.model.site_slot_value(&test.left, ctor).is_none()
            {
                continue;
            }
            let inside = statement.consequent.span();
            return Some(inside.start <= span.start && span.end <= inside.end);
        }
        None
    }

    /// The object a function returns: its literal members plus the
    /// `local.key = value` writes to the returned local.
    fn returned_object(&self, site: FunctionSite) -> Option<BTreeMap<String, &'a Expression<'a>>> {
        let own = self.own(site)?;
        let [returned] = own.returns.as_slice() else {
            return None;
        };
        let returned = returned.without_parentheses();
        let mut members = BTreeMap::new();
        if let Expression::ObjectExpression(object) = returned {
            for member in object_members_of(object) {
                members.insert(member.name, member.value);
            }
            return Some(members);
        }
        let symbol = self.symbol(returned)?;
        let Expression::ObjectExpression(object) =
            self.model.symbol_value(symbol)?.without_parentheses()
        else {
            return None;
        };
        for member in object_members_of(object) {
            members.insert(member.name, member.value);
        }
        for assignment in &own.assigns {
            let Some(key) = target_key(&assignment.left) else {
                continue;
            };
            let root = match &assignment.left {
                AssignmentTarget::StaticMemberExpression(member) => &member.object,
                AssignmentTarget::ComputedMemberExpression(member) => &member.object,
                _ => continue,
            };
            if self.symbol(root) == Some(symbol) {
                members.insert(key, &assignment.right);
            }
        }
        Some(members)
    }

    /// `slot0.name` is what this callback returns.
    fn returns_slot_member(&self, site: FunctionSite, name: &str) -> bool {
        let returns = self.returns(site);
        let [returned] = returns.as_slice() else {
            return false;
        };
        static_member_name(returned.without_parentheses()) == Some(name)
            && member_object(returned.without_parentheses())
                .is_some_and(|object| self.is_slot(object, site, 0))
    }

    // -----------------------------------------------------------------------
    // Claims
    // -----------------------------------------------------------------------

    fn claim_function(
        &mut self,
        role: &'static str,
        site: FunctionSite,
        alias_role: Option<&'static str>,
        evidence: &str,
    ) {
        if self.seen.insert((site.span.start, site.span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: site.span,
                alias_role,
                confidence: Confidence::High,
                evidence: vec![evidence.to_string()],
            });
        }
    }

    fn claim_main(&mut self, role: &'static str, symbol: SymbolId, evidence: &str) {
        if !self.model.main_symbols.contains(&symbol) {
            return;
        }
        let identifier = self.model.scoping().symbol_span(symbol);
        if self.seen.insert((identifier.start, identifier.end)) {
            self.claims.push(FinderClaim::MainBinding {
                role,
                identifier_span: identifier,
                confidence: Confidence::High,
                evidence: vec![evidence.to_string()],
            });
        }
    }

    // -----------------------------------------------------------------------
    // The registrar and teardown contract shared by constructors and groups
    // -----------------------------------------------------------------------

    /// Prove the four registrars of one task call against the teardown that
    /// `exec` returns. The stored emit routine is the only two-slot registrar;
    /// the teardown calls the cleanup and blanks init and last-chance; of
    /// those two only the last-chance routine is read from a deferred thunk.
    fn routines(
        &self,
        exec: FunctionSite,
        registrars: &[&'a Expression<'a>],
        teardown: FunctionSite,
    ) -> Option<Routines> {
        let [first, second, third, fourth] = registrars else {
            return None;
        };
        let sites = [
            self.function_of(first)?,
            self.function_of(second)?,
            self.function_of(third)?,
            self.function_of(fourth)?,
        ];
        let writes: Vec<Vec<SymbolId>> = sites
            .iter()
            .map(|site| self.registrar_writes(*site))
            .collect::<Option<_>>()?;

        let teardown_own = self.own(teardown)?;
        let mut blanked = BTreeSet::new();
        let mut noop = BTreeSet::new();
        for assignment in &teardown_own.assigns {
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                return None;
            };
            blanked.insert(analysis::identifier_symbol(target, self.model.scoping())?);
            noop.insert(self.symbol(&assignment.right)?);
        }
        let mut noops = noop.into_iter();
        let (Some(noop), None, [cleanup_call]) =
            (noops.next(), noops.next(), teardown_own.calls.as_slice())
        else {
            return None;
        };
        if blanked.len() != 2 {
            return None;
        }
        let cleanup = self.symbol(normal_call(cleanup_call).callee)?;

        let mut init_register = None;
        let mut emit_register = None;
        let mut last_chance_register = None;
        let mut cleanup_register = None;
        let mut last_chance = None;
        let mut emit = BTreeSet::new();
        let mut route_flag = None;
        for (site, stored) in sites.iter().zip(&writes) {
            match stored.as_slice() {
                [routine, flag] => {
                    if emit_register.replace(*site).is_some() {
                        return None;
                    }
                    emit.insert(*routine);
                    route_flag = Some(*flag);
                }
                [routine] if *routine == cleanup => cleanup_register = Some(*site),
                [routine] if blanked.contains(routine) => {
                    if self.read_elsewhere(*routine, exec, &[*site, teardown]) {
                        if last_chance_register.replace(*site).is_some() {
                            return None;
                        }
                        last_chance = Some(*routine);
                    } else if init_register.replace(*site).is_some() {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        let own = self.own(exec)?;
        for routine in emit.clone() {
            for assignment in &own.assigns {
                if let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left {
                    if self.symbol(&assignment.right) == Some(routine) {
                        emit.extend(analysis::identifier_symbol(target, self.model.scoping()));
                    }
                }
            }
            for declarator in &own.declarators {
                if declarator.init.as_ref().and_then(|init| self.symbol(init)) == Some(routine) {
                    if let BindingPattern::BindingIdentifier(identifier) = &declarator.id {
                        emit.extend(identifier.symbol_id.get());
                    }
                }
            }
        }
        Some(Routines {
            init_register: init_register?,
            emit_register: emit_register?,
            last_chance_register: last_chance_register?,
            cleanup_register: cleanup_register?,
            last_chance: last_chance?,
            emit,
            route_flag: route_flag?,
            noop,
        })
    }

    fn claim_registrars(&mut self, routines: &Routines) {
        let evidence = "registrar handed to a collector task: stores its slots in the closure and does nothing else";
        self.claim_function(REGISTER_INIT, routines.init_register, None, evidence);
        self.claim_function(REGISTER_EMIT, routines.emit_register, None, evidence);
        self.claim_function(REGISTER_LAST_CHANCE, routines.last_chance_register, None, evidence);
        self.claim_function(REGISTER_CLEANUP, routines.cleanup_register, None, evidence);
        self.claim_main(
            NOOP_ROUTINE,
            routines.noop,
            "every collector teardown swaps the stored init and last-chance routines for this value",
        );
    }

    /// The emit thunk `site` is, when its single call passes the stored emit
    /// routine on. It runs immediately only under the stored route flag.
    fn claim_emit_thunk(&mut self, site: FunctionSite, routines: &Routines) -> bool {
        let calls = self.calls(site);
        let [call] = calls.as_slice() else {
            return false;
        };
        let routine = call
            .args
            .iter()
            .any(|arg| self.symbol(arg).is_some_and(|symbol| routines.emit.contains(&symbol)));
        if !routine {
            return false;
        }
        if self.under_test(site.node_id, routines.route_flag) {
            self.claim_function(
                EMIT_IMMEDIATE,
                site,
                None,
                "thunk running the stored emit routine, queued only under the stored route flag",
            );
        } else {
            self.claim_function(
                EMIT_TIMED,
                site,
                None,
                "thunk running the stored emit routine, queued when immediate routing is off",
            );
        }
        true
    }

    // -----------------------------------------------------------------------
    // Direct-slot signal constructors
    // -----------------------------------------------------------------------

    fn find_ctors(&mut self) {
        for factory in self.model.all_sites().to_vec() {
            let returns = self.returns(factory);
            let [returned] = returns.as_slice() else {
                continue;
            };
            let Some(ctor) = self.function_of(returned) else {
                continue;
            };
            if !self.nested(factory).iter().any(|site| site.span == ctor.span) {
                continue;
            }
            let ctor_returns = self.returns(ctor);
            let [Expression::NewExpression(built)] = ctor_returns.as_slice() else {
                continue;
            };
            let arguments = new_arguments(built);
            let [executor] = arguments.as_slice() else {
                continue;
            };
            let Some(exec) = self.function_of(executor) else {
                continue;
            };
            self.claim_ctor(factory, ctor, exec);
        }
    }

    fn claim_ctor(&mut self, factory: FunctionSite, ctor: FunctionSite, exec: FunctionSite) {
        let task_call = self.calls(exec).into_iter().find(|call| {
            self.model.site_slot_value(call.callee, factory).is_some() && call.args.len() >= 5
        });
        let Some(task_call) = task_call else {
            return;
        };
        let (leading, registrars) = task_call.args.split_at(task_call.args.len() - 4);
        let (roles, what) = match leading {
            [context] if self.model.site_slot_value(context, ctor).is_some() => (
                (CTOR_FACTORY_CONTEXT, CTOR_CONTEXT, EXECUTOR_CONTEXT),
                "the task receives the context and the stored routines receive the subscriber",
            ),
            [subscriber, context]
                if self.is_slot(subscriber, exec, 0)
                    && self.model.site_slot_value(context, ctor).is_some() =>
            {
                (
                    (CTOR_FACTORY_OBSERVER, CTOR_OBSERVER, EXECUTOR_OBSERVER),
                    "the task receives the subscriber and the context",
                )
            }
            _ => return,
        };
        let returns = self.returns(exec);
        let [returned] = returns.as_slice() else {
            return;
        };
        let Some(teardown) = self.function_of(returned) else {
            return;
        };
        let Some(routines) = self.routines(exec, registrars, teardown) else {
            return;
        };
        let evidence = format!("Observable-based signal constructor over a task: {what}");
        self.claim_function(roles.0, factory, None, &evidence);
        self.claim_function(roles.1, ctor, None, &evidence);
        self.claim_function(roles.2, exec, None, &evidence);
        self.claim_function(
            DISPOSE,
            teardown,
            None,
            "returned by the executor: blanks init and last-chance, then runs the stored cleanup",
        );
        self.claim_registrars(&routines);
        let excluded = [
            routines.init_register,
            routines.emit_register,
            routines.last_chance_register,
            routines.cleanup_register,
            teardown,
        ];
        for site in self.nested(exec) {
            if excluded.iter().any(|other| other.span == site.span) {
                continue;
            }
            let guarded = self.own(site).is_some_and(|own| own.tries == 1)
                && self.reads_member(site, "isComplete")
                && self.calls(site).iter().any(|call| {
                    self.symbol(call.callee) == Some(routines.last_chance)
                });
            if guarded {
                // The executor registers one copy on each side of the label
                // test. Hoisted copies share a scope, so each side has a role.
                let role = match self.label_branch(site.node_id, ctor) {
                    Some(true) => LAST_CHANCE_UNLABELED,
                    Some(false) => LAST_CHANCE_LABELED,
                    None => continue,
                };
                self.claim_function(
                    role,
                    site,
                    None,
                    "registered wrapper: runs the stored last-chance routine unless complete, routing throws to the subscriber",
                );
            } else {
                self.claim_emit_thunk(site, &routines);
            }
        }
        self.claim_exec_references(exec);
    }

    /// The MAIN helpers an executor's emit thunks call: the safe runner is the
    /// callee receiving the stored routine and the subscriber.
    fn claim_exec_references(&mut self, exec: FunctionSite) {
        for site in self.nested(exec) {
            for call in self.calls(site) {
                let [routine, subscriber] = call.args.as_slice() else {
                    continue;
                };
                if self.symbol(routine).is_none() || !self.is_slot(subscriber, exec, 0) {
                    continue;
                }
                let Some(runner) = self.symbol(call.callee) else {
                    continue;
                };
                let safe = self.function_of(call.callee).is_some_and(|target| {
                    self.reads_member(target, "isComplete") && self.reads_member(target, "error")
                });
                if safe {
                    self.claim_main(
                        SAFE_RUNNER,
                        runner,
                        "runs a stored routine with the subscriber unless complete, routing throws to it",
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Group coordinator
    // -----------------------------------------------------------------------

    fn find_coordinators(&mut self) {
        for coordinator in self.model.all_sites().to_vec() {
            self.claim_coordinator(coordinator);
        }
    }

    fn claim_coordinator(&mut self, coordinator: FunctionSite) {
        let task_call = self.calls(coordinator).into_iter().find(|call| {
            self.model.site_slot_value(call.callee, coordinator).is_some() && call.args.len() == 6
        });
        let Some(task_call) = task_call else {
            return;
        };
        let (leading, registrars) = task_call.args.split_at(2);
        let Some(handles) = self.symbol(leading[0]) else {
            return;
        };
        if self.model.site_slot_value(leading[1], coordinator).is_none() {
            return;
        }
        let nested = self.nested(coordinator);
        let capture = nested.iter().copied().find(|site| {
            let calls = self.calls(*site);
            matches!(calls.as_slice(), [call]
                if call.method == Some("push")
                    && call.receiver.and_then(|receiver| self.symbol(receiver)) == Some(handles)
                    && matches!(call.args.as_slice(), [arg] if self.is_slot(arg, *site, 0)))
        });
        let Some(capture) = capture else {
            return;
        };
        let captured = self.own(coordinator).is_some_and(|own| {
            own.news.iter().any(|built| {
                matches!(new_arguments(built).as_slice(),
                    [executor] if self.function_of(executor).is_some_and(|site| site.span == capture.span))
            })
        }) && self.calls(coordinator).iter().any(|call| call.method == Some("subscribe"));
        if !captured {
            return;
        }
        let mut teardown = None;
        let mut noop = None;
        for returned in self.returns(coordinator) {
            let Some(site) = self.function_of(returned) else {
                return;
            };
            let empty = self.own(site).is_none_or(|own| {
                own.calls.is_empty() && own.assigns.is_empty() && own.returns.is_empty()
            });
            let slot = if empty { &mut noop } else { &mut teardown };
            if slot.replace(site).is_some() {
                return;
            }
        }
        let Some(teardown) = teardown else {
            return;
        };
        let Some(routines) = self.routines(coordinator, registrars, teardown) else {
            return;
        };

        self.coordinators.insert((coordinator.span.start, coordinator.span.end));
        self.claim_function(
            COORDINATOR,
            coordinator,
            Some(COORDINATOR_ALIAS),
            "subscribes one capturing Observable per observer, hands the facades to the group task and returns its unsubscribe",
        );
        self.claim_function(GROUP_CAPTURE, capture, None, "Observable executor pushing the subscriber facade into the group's handles");
        self.claim_function(
            GROUP_UNSUBSCRIBE,
            teardown,
            None,
            "returned group unsubscribe: blanks init and last-chance, then runs the stored cleanup",
        );
        if let Some(noop) = noop {
            self.claim_function(GROUP_NOOP, noop, None, "empty unsubscribe returned when the group task throws");
        }
        self.claim_registrars(&routines);

        let mut errors = Vec::new();
        let excluded = [
            routines.init_register,
            routines.emit_register,
            routines.last_chance_register,
            routines.cleanup_register,
            teardown,
            capture,
        ];
        for site in nested {
            if excluded.iter().any(|other| other.span == site.span)
                || noop.is_some_and(|other| other.span == site.span)
            {
                continue;
            }
            errors.push(site);
            let is_safe_run = self.own(site).is_some_and(|own| own.tries == 1)
                && self.calls(site).iter().any(|call| {
                    self.is_slot(call.callee, site, 0) && call.args.is_empty()
                });
            if is_safe_run {
                errors.extend(self.nested(site));
                self.claim_function(
                    GROUP_SAFE_RUN,
                    site,
                    None,
                    "runs a routine and forwards a throw to every group handle",
                );
                continue;
            }
            if self.claim_emit_thunk(site, &routines) {
                continue;
            }
            let last_chance = self.calls(site).iter().any(|call| {
                call.args
                    .iter()
                    .any(|arg| self.symbol(arg) == Some(routines.last_chance))
            });
            if last_chance {
                self.claim_function(
                    GROUP_LAST_CHANCE,
                    site,
                    None,
                    "registered thunk running the stored last-chance routine through the group's safe runner",
                );
            }
        }
        for site in errors {
            let calls = self.calls(site);
            let forwards = matches!(calls.as_slice(), [call]
                if call.method == Some("error")
                    && call.receiver.is_some_and(|receiver| self.is_slot(receiver, site, 0)));
            if forwards {
                self.claim_function(
                    GROUP_FORWARD_ERROR,
                    site,
                    None,
                    "forEach callback forwarding a caught error to one group handle",
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Per-slot observers
    // -----------------------------------------------------------------------

    fn find_observer(&mut self) -> Option<ObserverStores> {
        let mut found = None;
        for factory in self.model.all_sites().to_vec() {
            let Some(members) = self.returned_object(factory) else {
                continue;
            };
            let (Some(next), Some(error), Some(complete)) = (
                members.get("next").and_then(|value| self.function_of(value)),
                members.get("error").and_then(|value| self.function_of(value)),
                members.get("complete").and_then(|value| self.function_of(value)),
            ) else {
                continue;
            };
            let Some(stores) = self.observer_stores(next, error, complete) else {
                continue;
            };
            self.claim_function(
                OBSERVER_NEXT,
                next,
                None,
                "observer next: stores the value cell, sets hasCollected and broadcasts a collect event",
            );
            self.claim_function(
                OBSERVER_ERROR,
                error,
                None,
                "observer error: reports the error, sets hasErrored and broadcasts an error event",
            );
            self.claim_function(
                OBSERVER_COMPLETE,
                complete,
                None,
                "observer complete: appends the slot and byte prefixes to scrBytes",
            );
            found = Some(stores);
        }
        found
    }

    fn observer_stores(
        &self,
        next: FunctionSite,
        error: FunctionSite,
        complete: FunctionSite,
    ) -> Option<ObserverStores> {
        let next_own = self.own(next)?;
        let mut values = None;
        let mut statuses = None;
        for assignment in &next_own.assigns {
            match &assignment.left {
                AssignmentTarget::ComputedMemberExpression(member)
                    if self.is_slot(&assignment.right, next, 0) =>
                {
                    values = self.symbol(&member.object);
                }
                target if target_key(target).as_deref() == Some("hasCollected") => {
                    let object = match target {
                        AssignmentTarget::StaticMemberExpression(member) => &member.object,
                        AssignmentTarget::ComputedMemberExpression(member) => &member.object,
                        _ => continue,
                    };
                    statuses = member_object(object).and_then(|root| self.symbol(root));
                }
                _ => {}
            }
        }
        let error_marks = self.reads_member(error, "hasErrored")
            || self.own(error)?.assigns.iter().any(|assignment| {
                target_key(&assignment.left).as_deref() == Some("hasErrored")
            });
        let scr = self.reads_member(complete, "scrBytes");
        if !error_marks || !scr {
            return None;
        }
        Some(ObserverStores {
            values: values?,
            statuses: statuses?,
        })
    }

    // -----------------------------------------------------------------------
    // startSignalCollection
    // -----------------------------------------------------------------------

    fn find_start(&mut self) -> Option<StartFacts> {
        let mut found = None;
        for start in self.model.all_sites().to_vec() {
            let Some(direct) = self.nested(start).into_iter().find(|site| {
                self.reads_member(*site, "ctor")
                    && self.reads_member(*site, "priority")
                    && self.reads_member(*site, "debugName")
                    && self.calls(*site).iter().any(|call| call.method == Some("subscribe"))
            }) else {
                continue;
            };
            let foreach = self.calls(start).into_iter().find(|call| {
                call.method == Some("forEach")
                    && matches!(call.args.as_slice(), [arg]
                        if self.function_of(arg).is_some_and(|site| site.span == direct.span))
            });
            let Some(descriptors) = foreach
                .and_then(|call| call.receiver)
                .and_then(|receiver| self.symbol(receiver))
            else {
                continue;
            };
            self.claim_start(start, direct);
            found = Some(StartFacts { descriptors });
        }
        found
    }

    fn claim_start(&mut self, start: FunctionSite, direct: FunctionSite) {
        self.claim_function(
            START_DIRECT,
            direct,
            None,
            "forEach callback over descriptors: builds a ctor from ctor, priority and debugName and subscribes the slot observer",
        );
        self.claim_context_merge(direct);
        for site in self.nested(direct) {
            let calls = self.calls(site);
            match calls.as_slice() {
                [call] if call.method == Some("unsubscribe") => {
                    self.claim_function(DIRECT_UNSUBSCRIBE, site, None, "teardown thunk unsubscribing one descriptor subscription");
                }
                [call] if self.forwards_callback(call, site) => {
                    self.claim_function(DIRECT_NOTIFIER, site, None, "registrar handed to the ctor: stores the callback in the notifier table under the slot");
                }
                _ => {}
            }
        }
        for site in self.nested(start) {
            if site.span == direct.span {
                continue;
            }
            let calls = self.calls(site);
            if let [call] = calls.as_slice() {
                if self.is_slot(call.callee, site, 0) && call.args.is_empty() {
                    self.claim_function(
                        SYNC_TASK_RUNNER,
                        site,
                        None,
                        "task runner of the local queue: calls the queued thunk on the current stack",
                    );
                    continue;
                }
            }
            let starts_group = calls.iter().any(|call| {
                self.function_of(call.callee).is_some_and(|target| {
                    self.coordinators.contains(&(target.span.start, target.span.end))
                })
            });
            if starts_group {
                self.claim_start_group(site);
            }
        }
        self.claim_scheduler(start);
    }

    /// The `Object.assign` helper merging an `{errorLogger}` record into the
    /// runtime context for one collector.
    fn claim_context_merge(&mut self, site: FunctionSite) {
        for call in self.calls(site) {
            let [record, _] = call.args.as_slice() else {
                continue;
            };
            let Expression::ObjectExpression(object) = self.origin(record) else {
                continue;
            };
            let carries_logger = object_members_of(object)
                .iter()
                .any(|member| member.name == "errorLogger");
            let delegates = self
                .function_of(call.callee)
                .is_some_and(|target| self.reads_member(target, "assign"));
            if let (true, true, Some(helper)) = (carries_logger, delegates, self.symbol(call.callee)) {
                self.claim_main(
                    OBJECT_MERGE,
                    helper,
                    "Object.assign helper merging the errorLogger record into the collector context",
                );
            }
        }
    }

    /// `(cb) => store(cb, slot)`: a one-call arrow forwarding its callback.
    fn forwards_callback(&self, call: &Call<'a>, site: FunctionSite) -> bool {
        call.method.is_none()
            && matches!(call.args.as_slice(), [callback, _] if self.is_slot(callback, site, 0))
    }

    fn claim_start_group(&mut self, site: FunctionSite) {
        self.claim_function(
            START_GROUP,
            site,
            None,
            "forEach callback over group bodies: maps the row to observers and hands them to the coordinator",
        );
        self.claim_context_merge(site);
        let mut notifier = None;
        for nested in self.nested(site) {
            let calls = self.calls(nested);
            if let [call] = calls.as_slice() {
                if self.forwards_callback(call, nested) {
                    notifier = Some(nested);
                }
            }
        }
        if let Some(notifier) = notifier {
            self.claim_function(
                GROUP_NOTIFIER,
                notifier,
                None,
                "registrar handed to the coordinator: stores the wrapper under the group's first slot",
            );
        }
    }

    /// `queue = factory(runner, timer)` written by the start function, whose
    /// return is that queue's `onEmpty`.
    fn claim_scheduler(&mut self, start: FunctionSite) {
        let returns = self.returns(start);
        let [returned] = returns.as_slice() else {
            return;
        };
        if static_member_name(returned.without_parentheses()) != Some("onEmpty") {
            return;
        }
        let Some(scheduler) =
            member_object(returned.without_parentheses()).and_then(|object| self.symbol(object))
        else {
            return;
        };
        self.claim_main(
            WORK_SCHEDULER,
            scheduler,
            "the timed queue the start function creates and whose onEmpty it returns",
        );
    }

    // -----------------------------------------------------------------------
    // Status broadcast
    // -----------------------------------------------------------------------

    fn find_status_relay(&mut self) {
        for relay in self.model.all_sites().to_vec() {
            let keys: BTreeSet<String> = self
                .own(relay)
                .map(|own| own.assigns.iter().filter_map(|a| target_key(&a.left)).collect())
                .unwrap_or_default();
            if !keys.contains("collectedSignals") || !keys.contains("erroredSignals") {
                continue;
            }
            self.claim_function(
                STATUS_RELAY,
                relay,
                None,
                "forEach callback building an event with collected and errored counts and calling the listener",
            );
            for site in self.nested(relay) {
                if self.returns_slot_member(site, "hasCollected") {
                    self.claim_function(STATUS_IS_COLLECTED, site, None, "filter predicate over status records: hasCollected");
                } else if self.returns_slot_member(site, "hasErrored") {
                    self.claim_function(STATUS_IS_ERRORED, site, None, "filter predicate over status records: hasErrored");
                }
            }
            let Some(owner) = self.model.owning_site(relay.node_id) else {
                continue;
            };
            let listeners = self.calls(owner).into_iter().find_map(|call| {
                let passes = call.method == Some("forEach")
                    && matches!(call.args.as_slice(), [arg]
                        if self.function_of(arg).is_some_and(|site| site.span == relay.span));
                passes.then(|| call.receiver.and_then(|receiver| self.symbol(receiver))).flatten()
            });
            if let Some(listeners) = listeners {
                self.claim_main(
                    STATUS_LISTENERS,
                    listeners,
                    "the dispatcher walks this list with the enriched status relay",
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Error report
    // -----------------------------------------------------------------------

    fn find_error_report(&mut self) {
        for report in self.model.all_sites().to_vec() {
            let Some(own) = self.own(report) else {
                continue;
            };
            let position = own.assigns.iter().any(|assignment| {
                target_key(&assignment.left).as_deref() == Some("signalPosition")
                    && self.is_slot(&assignment.right, report, 0)
            });
            let flagged = own
                .assigns
                .iter()
                .any(|assignment| target_key(&assignment.left).as_deref() == Some("il"));
            if !position
                || !flagged
                || !self.reads_member(report, "stack")
                || !self.reads_member(report, "stacktrace")
            {
                continue;
            }
            self.claim_function(
                ERROR_REPORT,
                report,
                Some(ERROR_REPORT_ALIAS),
                "builds a record with signalPosition, the il flag and scrubbed error fields and hands it to the error sink",
            );
            let sink = self
                .calls(report)
                .into_iter()
                .find(|call| call.method.is_none() && call.args.len() == 1)
                .and_then(|call| self.symbol(call.callee));
            if let Some(sink) = sink {
                self.claim_main(ERROR_SINK, sink, "receives the finished signal error record");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Last-chance notifier table and request snapshot
    // -----------------------------------------------------------------------

    /// The table `site` reads, when `site` is `(slot) => { if (T[slot] != null)
    /// T[slot](); }`.
    fn notifier_table(&self, site: FunctionSite) -> Option<SymbolId> {
        let calls = self.calls(site);
        let [call] = calls.as_slice() else {
            return None;
        };
        let Expression::ComputedMemberExpression(member) = call.callee else {
            return None;
        };
        (call.args.is_empty() && self.is_slot(&member.expression, site, 0))
            .then(|| self.symbol(&member.object))
            .flatten()
    }

    fn find_notifiers(&mut self) {
        let mut tables = BTreeSet::new();
        for hooks in self.model.all_sites().to_vec() {
            let nested = self.nested(hooks);
            for entry in nested {
                let Some(table) = self.notifier_table(entry) else {
                    continue;
                };
                let snapshot = self.is_snapshot(hooks);
                let walks_slots = self.calls(hooks).iter().any(|call| {
                    call.method == Some("forEach")
                        && call.receiver.is_some_and(|receiver| self.is_slot(receiver, hooks, 0))
                });
                if !snapshot && !walks_slots {
                    continue;
                }
                tables.insert(table);
                self.claim_function(NOTIFY_ENTRY, entry, None, "forEach callback calling the notifier-table entry of one slot");
                if walks_slots {
                    self.claim_function(NOTIFY_HOOKS, hooks, None, "walks the given slots and calls each notifier-table entry");
                }
            }
        }
        if tables.is_empty() {
            return;
        }
        for register in self.model.all_sites().to_vec() {
            let Some(own) = self.own(register) else {
                continue;
            };
            let [assignment] = own.assigns.as_slice() else {
                continue;
            };
            let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
                continue;
            };
            let is_store = own.calls.is_empty()
                && self.symbol(&member.object).is_some_and(|table| tables.contains(&table))
                && self.is_slot(&assignment.right, register, 0)
                && self.is_slot(&member.expression, register, 1);
            if is_store {
                self.claim_function(
                    NOTIFIER_REGISTER,
                    register,
                    Some(NOTIFIER_REGISTER_ALIAS),
                    "stores a callback in the notifier table the snapshot flushes, keyed by slot",
                );
            }
        }
    }

    fn is_snapshot(&self, site: FunctionSite) -> bool {
        self.reads_member(site, "signalFilter")
            && self.reads_member(site, "notifyLastChance")
            && self.reads_member(site, "includeDebugNames")
    }

    fn find_payload_readers(&mut self) {
        for snapshot in self.model.all_sites().to_vec() {
            if !self.is_snapshot(snapshot) {
                continue;
            }
            for site in self.nested(snapshot) {
                if let Some(members) = self.returned_object(site) {
                    if members.contains_key("value") && members.contains_key("signal") {
                        self.claim_function(PAYLOAD_NAMED_VALUE, site, None, "snapshot map callback: {value, signal} for one slot");
                    }
                    continue;
                }
                let returns = self.returns(site);
                if let [Expression::ComputedMemberExpression(member)] = returns.as_slice() {
                    if self.is_slot(&member.expression, site, 0) {
                        self.claim_function(PAYLOAD_VALUE, site, None, "snapshot map callback: the value cell of one slot");
                    }
                }
            }
        }
    }

    /// The `.map` callbacks that build the runtime's parallel per-descriptor
    /// stores over the descriptor table.
    fn find_store_initializers(&mut self, descriptors: SymbolId, observer: &ObserverStores) {
        for owner in self.model.all_sites().to_vec() {
            let assigns = self.own(owner).map(|own| own.assigns.clone()).unwrap_or_default();
            for assignment in assigns {
                let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                    continue;
                };
                let Some(store) = analysis::identifier_symbol(target, self.model.scoping()) else {
                    continue;
                };
                let Expression::CallExpression(call) = assignment.right.without_parentheses() else {
                    continue;
                };
                let call = normal_call(call);
                if call.method != Some("map")
                    || call.receiver.and_then(|receiver| self.symbol(receiver)) != Some(descriptors)
                {
                    continue;
                }
                let [callback] = call.args.as_slice() else {
                    continue;
                };
                let Some(callback) = self.function_of(callback) else {
                    continue;
                };
                if store == observer.values {
                    self.claim_function(VALUE_CELL_INIT, callback, None, "fills the observer's value store with one sentinel per descriptor");
                } else if store == observer.statuses {
                    let record = self.returned_object(callback).is_some_and(|members| {
                        ["debugName", "hasCollected", "hasErrored"]
                            .iter()
                            .all(|key| members.contains_key(*key))
                    });
                    if record {
                        self.claim_function(STATUS_RECORD_INIT, callback, None, "builds one status record per descriptor for the observer's status store");
                    }
                } else if self.returns_slot_member(callback, "debugName") {
                    self.claim_function(DEBUG_NAME_READ, callback, None, "reads debugName of one descriptor");
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // scrBytes chunked hash
    // -----------------------------------------------------------------------

    fn has_sha_digest(&self, site: FunctionSite) -> bool {
        self.calls(site).iter().any(|call| {
            call.method == Some("digest")
                && matches!(call.args.first(), Some(Expression::StringLiteral(literal))
                    if literal.value == "SHA-256")
        })
    }

    /// `Promise.all(data.map(pair)).then(outer).then(result).catch(failure)`.
    fn find_chunked_hash(&mut self) {
        for hash in self.model.all_sites().to_vec() {
            if !self.reads_member(hash, "sidx") || !self.reads_member(hash, "data") {
                continue;
            }
            for call in self.calls(hash) {
                if call.method != Some("catch") {
                    continue;
                }
                if let Some(chain) = self.hash_chain(&call) {
                    self.claim_chain(hash, chain);
                }
            }
        }
    }

    fn hash_chain(&self, last: &Call<'a>) -> Option<HashChain> {
        let [failure] = last.args.as_slice() else {
            return None;
        };
        let (result, outer_call) = self.then_step(last.receiver?)?;
        let (outer, first_then) = self.then_step(outer_call.receiver?)?;
        let Expression::CallExpression(all) = first_then.receiver?.without_parentheses() else {
            return None;
        };
        let all = normal_call(all);
        if all.method != Some("all") {
            return None;
        }
        let [mapped] = all.args.as_slice() else {
            return None;
        };
        let Expression::CallExpression(mapped) = self.origin(mapped) else {
            return None;
        };
        // `data.map(pair)`, or `tuple[1].call(tuple[0], pair)` over `[data, data.map]`.
        let mapped = normal_call(mapped);
        let chain = HashChain {
            pair: self.function_of(mapped.args.last()?)?,
            outer,
            result,
            failure: self.function_of(failure)?,
        };
        let keys = self.returned_object(chain.result)?;
        (self.has_sha_digest(chain.pair)
            && self.has_sha_digest(chain.outer)
            && keys.contains_key("sidx")
            && keys.contains_key("hash"))
        .then_some(chain)
    }

    /// The fulfilled handler of a `.then(handler)` step and that step's call.
    fn then_step(&self, expression: &'a Expression<'a>) -> Option<(FunctionSite, Call<'a>)> {
        let Expression::CallExpression(call) = expression.without_parentheses() else {
            return None;
        };
        let call = normal_call(call);
        if call.method != Some("then") {
            return None;
        }
        let [handler] = call.args.as_slice() else {
            return None;
        };
        Some((self.function_of(handler)?, call))
    }

    fn claim_chain(&mut self, hash: FunctionSite, chain: HashChain) {
        self.claim_function(
            SHA_HASH,
            hash,
            Some(SHA_HASH_ALIAS),
            "digests each data pair with SHA-256, digests the concatenation and resolves to {sidx, hash}",
        );
        self.claim_function(SHA_PAIR, chain.pair, None, "data.map callback: SHA-256 of one concatenated byte pair");
        self.claim_function(SHA_OUTER, chain.outer, None, "first then handler: SHA-256 of the concatenated digests");
        self.claim_function(SHA_RESULT, chain.result, None, "second then handler: hex-encodes the digest and returns {sidx, hash}");
        self.claim_function(SHA_FAILURE, chain.failure, None, "catch handler of the hash chain");
        for site in self.nested(chain.outer) {
            if self.calls(site).iter().any(|call| call.method == Some("concat")) {
                self.claim_function(SHA_APPEND, site, None, "forEach callback appending one digest's bytes to the accumulator");
            }
        }
        for site in self.nested(chain.result) {
            let hex = self.calls(site).iter().any(|call| {
                call.method == Some("toString")
                    && call.receiver.is_some_and(|receiver| self.is_slot(receiver, site, 0))
            });
            if hex {
                self.claim_function(SHA_HEX, site, None, "map callback: one byte to two lowercase hex digits");
            }
        }
    }
}

struct ObserverStores {
    values: SymbolId,
    statuses: SymbolId,
}

struct StartFacts {
    descriptors: SymbolId,
}

struct HashChain {
    pair: FunctionSite,
    outer: FunctionSite,
    result: FunctionSite,
    failure: FunctionSite,
}
