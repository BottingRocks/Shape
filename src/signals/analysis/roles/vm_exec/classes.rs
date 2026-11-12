//! Package `p03_exec_b` finders: the second half of the execution framework.
//! See `roles/mod.rs` for the recognition rules.
//!
//! The package names the elapsed-time clock, the helpers inside the sequential
//! and scheduled task queues, the reactive classes (subscription, subscription
//! observer, observable), the batch scheduler and the callback notifier class.
//!
//! # How the two tracer shapes read as one
//!
//! * A *class* is a [`ClassShape`]: either a `class` node, or a constructor
//!   function with `C.prototype.m = f` / `C.m = f` writes. Accessors are read
//!   from the class body or from the `{ key, get }` descriptor handed to
//!   `Object.defineProperty(C.prototype, ...)`.
//! * Named formals and `arguments[N]` are one slot, through
//!   [`ParameterSlots`]; a local whose single origin is a slot read is that
//!   slot.
//! * `f(a)` and `f.call(void 0, a)` are one invocation, `o.m(a)` and
//!   `o.m.call(o, a)` are one method call, `o.m` and `o["m"]` are one member.
//! * Inline callbacks and hoisted named callbacks are one function value,
//!   through [`Ix::resolve_fn`].
//!
//! # What is anchored on what
//!
//! Queue helpers anchor on the queue factories, `start`, `empty` and `onError`
//! methods `roles::purposes` proves; those functions are not claimed again. A
//! whole class is recognized from all of its methods at once, so one look-alike
//! method claims nothing. Aliases are named from how the proven class consumes
//! them. Member names used here are stable library API names, never generated
//! spellings, and every one is backed by a behavior check.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::Span;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

use crate::analysis;

use crate::signals::extract::bindings::{object_members_of, property_key_name, ParameterSlots};
use crate::signals::analysis::roles::purposes::{find_purposes, PurposeOutcome};
use crate::signals::schema::Confidence;
use crate::signals::analysis::roles::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};

const ELAPSED_FACTORY: &str = "elapsed_timer_factory";
const ELAPSED_DATE_CLOCK: &str = "elapsed_timer_date_clock";
const ELAPSED_READING: &str = "elapsed_timer_reading";
const SEQUENTIAL_ONCE: &str = "sequential_queue_once_factory";
const SEQUENTIAL_ONCE_ALIAS: &str = "sequential_queue_once_factory_alias";
const SCHEDULED_ONCE_ALIAS: &str = "scheduled_queue_once_factory_alias";
const QUEUE_ADVANCE: &str = "task_queue_advance";
const QUEUE_SCHEDULE_ERROR: &str = "task_queue_schedule_error_listener";
const QUEUE_INVOKE_ERROR: &str = "task_queue_invoke_error_listener";
const QUEUE_RUN_TASK: &str = "task_queue_run_dequeued_task";
const QUEUE_SCHEDULE_EMPTY: &str = "task_queue_schedule_empty_listener";
const QUEUE_LATCHED_ERROR: &str = "task_queue_deliver_latched_error";
const CUSTOM_EVENT: &str = "bubbling_custom_event_dispatcher";
const READY_STATE: &str = "document_ready_state_change_handler";
const RELEASED_NOOP: &str = "released_callback_noop";
const SUB_CTOR: &str = "subscription_constructor";
const SUB_CLASS: &str = "subscription_class";
const SUB_UNSUBSCRIBE: &str = "subscription_unsubscribe";
const SUB_COMPLETE: &str = "subscription_is_complete";
const SUB_CLOSED: &str = "subscription_closed_getter";
const SUB_CLASS_ALIAS: &str = "subscription_class_alias";
const OBS_CTOR: &str = "subscription_observer_constructor";
const OBS_CLASS: &str = "subscription_observer_class";
const OBS_NEXT: &str = "subscription_observer_next";
const OBS_ERROR: &str = "subscription_observer_error";
const OBS_DONE: &str = "subscription_observer_complete";
const OBS_IS_COMPLETE: &str = "subscription_observer_is_complete";
const OBS_CLOSED: &str = "subscription_observer_closed_getter";
const OBS_CLASS_ALIAS: &str = "subscription_observer_class_alias";
const OBSERVABLE_CTOR: &str = "observable_constructor";
const OBSERVABLE_OF: &str = "observable_of";
const OF_EXECUTOR: &str = "observable_of_executor";
const OF_EMIT: &str = "observable_of_emit_value";
const OBSERVABLE_EMPTY: &str = "observable_empty";
const EMPTY_EXECUTOR: &str = "observable_empty_executor";
const OBSERVABLE_FROM: &str = "observable_from";
const PROMISE_EXECUTOR: &str = "observable_from_promise_executor";
const PROMISE_VALUE: &str = "observable_from_promise_value";
const PROMISE_REJECTION: &str = "observable_from_promise_rejection";
const ARRAY_EXECUTOR: &str = "observable_from_array_executor";
const ARRAY_STEP: &str = "observable_from_array_step";
const ITERABLE_EXECUTOR: &str = "observable_from_iterable_executor";
const ITERABLE_STEP: &str = "observable_from_iterable_step";
const OBSERVABLE_SUBSCRIBE: &str = "observable_subscribe";
const OBSERVABLE_PIPE: &str = "observable_pipe";
const PIPE_STEP: &str = "observable_pipe_apply_operator";
const OBSERVABLE_MAP: &str = "observable_map";
const OBSERVABLE_FLAT_MAP: &str = "observable_unordered_flat_map";
const MAP_ALIAS: &str = "map_operator_factory_alias";
const FLAT_MAP_ALIAS: &str = "flat_map_operator_factory_alias";
const TEARDOWN_ALIAS: &str = "executor_teardown_normalizer_alias";
const BATCH_CTOR: &str = "batch_scheduler_constructor";
const BATCH_CLASS: &str = "batch_scheduler_class";
const BATCH_NOW: &str = "batch_scheduler_now";
const BATCH_SCHEDULE: &str = "batch_scheduler_schedule";
const BATCH_PROCESS: &str = "batch_scheduler_process";
const NOTIFIER_CTOR: &str = "callback_notifier_constructor";
const NOTIFIER_CLASS: &str = "callback_notifier_class";
const NOTIFIER_REGISTER: &str = "callback_notifier_register";
const NOTIFIER_UNREGISTER: &str = "callback_notifier_unregister";
const NOTIFIER_CLEAR: &str = "callback_notifier_clear";
const NOTIFIER_NOTIFY: &str = "callback_notifier_notify";

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole {
        role,
        name,
        kind: FinderRoleKind::Function,
    }
}

const fn class(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole {
        role,
        name,
        kind: FinderRoleKind::Class,
    }
}

const fn binding(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole {
        role,
        name,
        kind: FinderRoleKind::MainBinding,
    }
}

/// Every role this package can claim, with the one name it receives.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    function(ELAPSED_FACTORY, "createElapsedTimer"),
    function(ELAPSED_DATE_CLOCK, "readDateClockMillis"),
    function(ELAPSED_READING, "readElapsedTime"),
    function(SEQUENTIAL_ONCE, "createSequentialQueueOnceCallback"),
    FinderRole {
        role: SEQUENTIAL_ONCE_ALIAS,
        name: "sequentialQueueOnceFactory",
        kind: FinderRoleKind::FunctionAlias,
    },
    binding(SCHEDULED_ONCE_ALIAS, "scheduledQueueOnceFactory"),
    function(QUEUE_ADVANCE, "advanceTaskQueue"),
    function(QUEUE_SCHEDULE_ERROR, "scheduleQueueErrorListener"),
    function(QUEUE_INVOKE_ERROR, "invokeQueueErrorListener"),
    function(QUEUE_RUN_TASK, "runDequeuedQueueTask"),
    function(QUEUE_SCHEDULE_EMPTY, "scheduleQueueEmptyListener"),
    function(QUEUE_LATCHED_ERROR, "deliverLatchedQueueError"),
    function(CUSTOM_EVENT, "dispatchBubblingCustomEvent"),
    function(READY_STATE, "handleDocumentReadyStateChange"),
    function(RELEASED_NOOP, "ignoreReleasedCallback"),
    function(SUB_CTOR, "SubscriptionConstructor"),
    class(SUB_CLASS, "Subscription"),
    function(SUB_UNSUBSCRIBE, "unsubscribeSubscription"),
    function(SUB_COMPLETE, "isSubscriptionComplete"),
    function(SUB_CLOSED, "getSubscriptionClosed"),
    binding(SUB_CLASS_ALIAS, "SubscriptionClass"),
    function(OBS_CTOR, "SubscriptionObserverConstructor"),
    class(OBS_CLASS, "SubscriptionObserver"),
    function(OBS_NEXT, "emitSubscriberNext"),
    function(OBS_ERROR, "emitSubscriberError"),
    function(OBS_DONE, "emitSubscriberComplete"),
    function(OBS_IS_COMPLETE, "isSubscriberComplete"),
    function(OBS_CLOSED, "getSubscriberClosed"),
    binding(OBS_CLASS_ALIAS, "SubscriptionObserverClass"),
    function(OBSERVABLE_CTOR, "ObservableConstructor"),
    function(OBSERVABLE_OF, "observableOf"),
    function(OF_EXECUTOR, "emitOfValuesThenComplete"),
    function(OF_EMIT, "emitOfValue"),
    function(OBSERVABLE_EMPTY, "observableEmpty"),
    function(EMPTY_EXECUTOR, "completeEmptyObservable"),
    function(OBSERVABLE_FROM, "observableFrom"),
    function(PROMISE_EXECUTOR, "emitPromiseSettlement"),
    function(PROMISE_VALUE, "emitPromiseValue"),
    function(PROMISE_REJECTION, "emitPromiseRejection"),
    function(ARRAY_EXECUTOR, "emitArrayLikeValues"),
    function(ARRAY_STEP, "emitNextArrayLikeValue"),
    function(ITERABLE_EXECUTOR, "emitIterableValues"),
    function(ITERABLE_STEP, "emitNextIterableValue"),
    function(OBSERVABLE_SUBSCRIBE, "subscribeObservable"),
    function(OBSERVABLE_PIPE, "pipeObservable"),
    function(PIPE_STEP, "applyPipeOperator"),
    function(OBSERVABLE_MAP, "mapObservable"),
    function(OBSERVABLE_FLAT_MAP, "flatMapObservableUnordered"),
    binding(MAP_ALIAS, "mapOperatorFactory"),
    binding(FLAT_MAP_ALIAS, "flatMapOperatorFactory"),
    binding(TEARDOWN_ALIAS, "executorTeardownNormalizer"),
    function(BATCH_CTOR, "BatchSchedulerConstructor"),
    class(BATCH_CLASS, "BatchScheduler"),
    function(BATCH_NOW, "readBatchSchedulerClock"),
    function(BATCH_SCHEDULE, "scheduleBatchedWork"),
    function(BATCH_PROCESS, "processWorkBatch"),
    function(NOTIFIER_CTOR, "CallbackNotifierConstructor"),
    class(NOTIFIER_CLASS, "CallbackNotifier"),
    function(NOTIFIER_REGISTER, "registerNotifierCallback"),
    function(NOTIFIER_UNREGISTER, "unregisterNotifierCallback"),
    function(NOTIFIER_CLEAR, "clearNotifierCallbacks"),
    function(NOTIFIER_NOTIFY, "runNotifierCallbacks"),
];

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let purposes = find_purposes(ctx.source, ctx.program, ctx.scoping, ctx.nodes);
    let ix = Ix::new(ctx);
    let classes = ix.classes();
    let mut run = Run {
        ix: &ix,
        claims: Vec::new(),
        seen: BTreeSet::new(),
        purpose_named: purpose_spans(&purposes),
    };
    let timers = run.elapsed_timers();
    run.queue_helpers(&purposes);
    run.custom_event_dispatchers();
    run.ready_state_handlers();
    run.released_callback_noops();
    for class in &classes {
        run.subscription_class(class);
        run.subscription_observer_class(class);
        run.observable_class(class);
        run.batch_scheduler_class(class, &timers);
        run.callback_notifier_class(class);
    }
    run.claims
}

/// Every function `roles::purposes` already names.
fn purpose_spans(purposes: &[PurposeOutcome]) -> BTreeSet<(u32, u32)> {
    purposes
        .iter()
        .filter_map(|outcome| match outcome {
            PurposeOutcome::Found(finding) => {
                Some((finding.function_span.start, finding.function_span.end))
            }
            _ => None,
        })
        .collect()
}

// ===========================================================================
// Structural index
// ===========================================================================

const NONE: u32 = u32::MAX;

#[derive(Clone, Copy)]
enum FnNode<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
}

/// One function and the nodes it owns directly (nested functions excluded).
struct Fx<'a> {
    node: NodeId,
    span: Span,
    func: FnNode<'a>,
    parent: Option<usize>,
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    assigns: Vec<&'a AssignmentExpression<'a>>,
    ifs: Vec<&'a IfStatement<'a>>,
    tries: usize,
    loops: usize,
    throws: usize,
    members: BTreeSet<&'a str>,
    object_keys: BTreeSet<String>,
    nested: Vec<usize>,
}

impl<'a> Fx<'a> {
    fn new(node: NodeId, span: Span, func: FnNode<'a>, parent: Option<usize>) -> Self {
        Self {
            node,
            span,
            func,
            parent,
            calls: Vec::new(),
            news: Vec::new(),
            assigns: Vec::new(),
            ifs: Vec::new(),
            tries: 0,
            loops: 0,
            throws: 0,
            members: BTreeSet::new(),
            object_keys: BTreeSet::new(),
            nested: Vec::new(),
        }
    }

    fn formal_count(&self) -> usize {
        match self.func {
            FnNode::Function(function) => function.params.items.len(),
            FnNode::Arrow(arrow) => arrow.params.items.len(),
        }
    }

    fn statements(&self) -> &'a [Statement<'a>] {
        match self.func {
            FnNode::Function(function) => function
                .body
                .as_ref()
                .map_or(&[], |body| body.statements.as_slice()),
            FnNode::Arrow(arrow) => arrow.body.statements.as_slice(),
        }
    }

    /// The statements without a trailing bare return.
    fn effective(&self) -> &'a [Statement<'a>] {
        let statements = self.statements();
        match statements.split_last() {
            Some((Statement::ReturnStatement(last), rest))
                if last.argument.as_ref().is_none_or(is_void0) =>
            {
                rest
            }
            _ => statements,
        }
    }

    /// The value of the last statement, when it returns one. An expression
    /// arrow's body counts.
    fn last_value(&self) -> Option<&'a Expression<'a>> {
        match self.statements().last()? {
            Statement::ReturnStatement(statement) => statement.argument.as_ref(),
            Statement::ExpressionStatement(statement) => match self.func {
                FnNode::Arrow(arrow) if arrow.expression => Some(&statement.expression),
                _ => None,
            },
            _ => None,
        }
    }

    /// The value of a body that is nothing but one `return value`.
    fn only_value(&self) -> Option<&'a Expression<'a>> {
        (self.statements().len() == 1)
            .then(|| self.last_value())
            .flatten()
    }
}

/// A class, in either tracer shape.
#[derive(Default)]
struct ClassShape {
    symbol: Option<SymbolId>,
    /// The class node span, for a `class` declaration shape. `None` for the
    /// constructor-function-with-prototype shape, whose binding is a function.
    class_span: Option<Span>,
    ctor: Option<usize>,
    methods: BTreeMap<String, usize>,
    statics: BTreeMap<String, usize>,
    getters: BTreeMap<String, usize>,
}

impl ClassShape {
    fn method(&self, name: &str) -> Option<usize> {
        self.methods.get(name).copied()
    }

    fn static_method(&self, name: &str) -> Option<usize> {
        self.statics.get(name).copied()
    }
}

struct Ix<'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    slots: ParameterSlots<'a, 'a>,
    fns: Vec<Fx<'a>>,
    fn_of_node: Vec<u32>,
    owner: Vec<u32>,
    decl_fn: BTreeMap<SymbolId, usize>,
    /// Plain `symbol = value` writes.
    assignments: BTreeMap<SymbolId, Vec<&'a AssignmentExpression<'a>>>,
    /// Plain `object.key = value` writes anywhere in the program.
    member_writes: Vec<&'a AssignmentExpression<'a>>,
    all_calls: Vec<&'a CallExpression<'a>>,
    class_nodes: Vec<&'a Class<'a>>,
    /// Bindings whose declarator or plain write reads the key symbol.
    readers: BTreeMap<SymbolId, BTreeSet<SymbolId>>,
}

impl<'a> Ix<'a> {
    fn new(ctx: &FinderCtx<'_, 'a>) -> Self {
        let count = ctx.nodes.len();
        let mut ix = Self {
            scoping: ctx.scoping,
            nodes: ctx.nodes,
            slots: ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes),
            fns: Vec::new(),
            fn_of_node: vec![NONE; count],
            owner: vec![NONE; count],
            decl_fn: BTreeMap::new(),
            assignments: BTreeMap::new(),
            member_writes: Vec::new(),
            all_calls: Vec::new(),
            class_nodes: Vec::new(),
            readers: BTreeMap::new(),
        };
        let nodes = ctx.nodes;
        for (id, node) in nodes.iter_enumerated() {
            let index = id.index();
            let parent = nodes.parent_id(id);
            let owner = if parent == id || parent.index() >= index {
                NONE
            } else if ix.fn_of_node[parent.index()] != NONE {
                ix.fn_of_node[parent.index()]
            } else {
                ix.owner[parent.index()]
            };
            ix.owner[index] = owner;
            let owner = (owner != NONE).then_some(owner as usize);
            let func = match node.kind() {
                AstKind::Function(function) => Some((function.span, FnNode::Function(function))),
                AstKind::ArrowFunctionExpression(arrow) => Some((arrow.span, FnNode::Arrow(arrow))),
                _ => None,
            };
            if let Some((span, func)) = func {
                let function_index = ix.fns.len();
                ix.fns.push(Fx::new(id, span, func, owner));
                ix.fn_of_node[index] = function_index as u32;
                if let Some(owner) = owner {
                    ix.fns[owner].nested.push(function_index);
                }
                if let FnNode::Function(function) = func {
                    if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        ix.decl_fn.insert(symbol, function_index);
                    }
                }
                continue;
            }
            ix.index_node(node.kind(), owner);
        }
        ix
    }

    fn index_node(&mut self, kind: AstKind<'a>, owner: Option<usize>) {
        match kind {
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign =>
            {
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                        self.assignments.entry(symbol).or_default().push(assignment);
                        if let Some(read) =
                            analysis::expression_symbol(strip(&assignment.right), self.scoping)
                        {
                            self.readers.entry(read).or_default().insert(symbol);
                        }
                    }
                } else if target_member(&assignment.left).is_some() {
                    self.member_writes.push(assignment);
                }
            }
            AstKind::VariableDeclarator(declarator) => {
                if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                {
                    if let (Some(symbol), Some(read)) = (
                        binding.symbol_id.get(),
                        analysis::expression_symbol(strip(init), self.scoping),
                    ) {
                        self.readers.entry(read).or_default().insert(symbol);
                    }
                }
            }
            AstKind::CallExpression(call) => self.all_calls.push(call),
            AstKind::Class(class) => self.class_nodes.push(class),
            _ => {}
        }
        let Some(owner) = owner else {
            return;
        };
        let own = &mut self.fns[owner];
        match kind {
            AstKind::CallExpression(call) => own.calls.push(call),
            AstKind::NewExpression(new) => own.news.push(new),
            AstKind::AssignmentExpression(assignment) => own.assigns.push(assignment),
            AstKind::IfStatement(statement) => own.ifs.push(statement),
            AstKind::TryStatement(_) => own.tries += 1,
            AstKind::ThrowStatement(_) => own.throws += 1,
            AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::ForStatement(_) => {
                own.loops += 1;
            }
            AstKind::StaticMemberExpression(member) => {
                own.members.insert(member.property.name.as_str());
            }
            AstKind::ComputedMemberExpression(member) => {
                if let Expression::StringLiteral(literal) = &member.expression {
                    own.members.insert(literal.value.as_str());
                }
            }
            AstKind::ObjectProperty(property) => {
                if let Some(name) = property_key_name(&property.key) {
                    own.object_keys.insert(name);
                }
            }
            _ => {}
        }
    }

    // ---- bindings ---------------------------------------------------------

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.scoping)
    }

    fn is_global(&self, expression: &Expression<'a>, name: &str) -> bool {
        matches!(strip(expression), Expression::Identifier(identifier)
            if identifier.name == name
                && analysis::identifier_symbol(identifier, self.scoping).is_none())
    }

    fn owner_of(&self, node: NodeId) -> Option<usize> {
        let owner = *self.owner.get(node.index())?;
        (owner != NONE).then_some(owner as usize)
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

    /// Every source value of a binding. `None` when the binding has several
    /// declarations or a write that is not a plain assignment.
    fn origins(&self, symbol: SymbolId) -> Option<Vec<&'a Expression<'a>>> {
        if self.scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        let assignments = self.assignments.get(&symbol).map_or(&[][..], Vec::as_slice);
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

    /// The one value of a binding; `void 0` placeholder writes do not count.
    fn value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        let origins = self.origins(symbol)?;
        let mut values = origins.into_iter().filter(|origin| !is_void0(origin));
        let value = values.next()?;
        values.next().is_none().then_some(value)
    }

    /// The function a value denotes: a literal, a once-declared never-written
    /// function binding, or a binding chain that ends at one.
    fn resolve_fn(&self, expression: &Expression<'a>) -> Option<usize> {
        let mut current = strip(expression);
        for _ in 0..6 {
            let node = match current {
                Expression::FunctionExpression(function) => Some(function.node_id.get()),
                Expression::ArrowFunctionExpression(arrow) => Some(arrow.node_id.get()),
                _ => None,
            };
            if let Some(node) = node {
                let index = *self.fn_of_node.get(node.index())?;
                return (index != NONE).then_some(index as usize);
            }
            let symbol = self.symbol(current)?;
            if let Some(index) = self.decl_fn.get(&symbol).copied() {
                return self.fixed_binding(symbol).then_some(index);
            }
            current = strip(self.value(symbol)?);
        }
        None
    }

    fn fixed_binding(&self, symbol: SymbolId) -> bool {
        self.scoping.symbol_declarations(symbol).count() == 1
            && !self
                .scoping
                .get_resolved_references(symbol)
                .any(|reference| reference.is_write())
    }

    /// True when `expression` is `target` or a binding chain that ends at it.
    fn denotes(&self, expression: &Expression<'a>, target: SymbolId) -> bool {
        let mut current = strip(expression);
        for _ in 0..6 {
            let Some(symbol) = self.symbol(current) else {
                return false;
            };
            if symbol == target {
                return true;
            }
            match self.value(symbol) {
                Some(value) => current = strip(value),
                None => return false,
            }
        }
        false
    }

    /// Bindings whose single origin is exactly `target`.
    fn aliases(&self, target: SymbolId) -> Vec<SymbolId> {
        self.readers.get(&target).map_or_else(Vec::new, |readers| {
            readers
                .iter()
                .copied()
                .filter(|reader| {
                    self.origins(*reader).is_some_and(|origins| {
                        matches!(origins.as_slice(), [only] if self.symbol(only) == Some(target))
                    })
                })
                .collect()
        })
    }

    /// True when every read of `symbol` sits inside function `outer`, and
    /// there is at least one.
    fn read_only_inside(&self, symbol: SymbolId, outer: usize) -> bool {
        let mut reads = 0;
        for reference in self.scoping.get_resolved_references(symbol) {
            if reference.is_read() {
                reads += 1;
                let inside = self
                    .owner_of(reference.node_id())
                    .is_some_and(|owner| self.within(owner, outer));
                if !inside {
                    return false;
                }
            }
        }
        reads > 0
    }

    // ---- slots ------------------------------------------------------------

    fn direct_slot(&self, expression: &Expression<'a>, index: usize) -> Option<usize> {
        let slot = self.slots.incoming_slot(strip(expression))?;
        (slot.owner == self.fns[index].node).then_some(slot.index as usize)
    }

    /// A slot read directly, or through a local whose one value is a slot
    /// read. A closure read of the local counts, so `index` is the function
    /// that received the argument.
    fn slot(&self, expression: &Expression<'a>, index: usize) -> Option<usize> {
        if let Some(slot) = self.direct_slot(expression, index) {
            return Some(slot);
        }
        let value = self.value(self.symbol(expression)?)?;
        self.direct_slot(value, index)
    }

    // ---- predicates -------------------------------------------------------

    /// `object.name(args)` on any receiver, in either call form.
    fn method_calls(
        &self,
        index: usize,
        name: &str,
    ) -> Vec<(&'a Expression<'a>, Vec<&'a Expression<'a>>)> {
        self.fns[index]
            .calls
            .iter()
            .filter_map(|call| method_call(call))
            .filter(|(_, method, _)| *method == name)
            .map(|(receiver, _, args)| (receiver, args))
            .collect()
    }

    /// The first function argument of a `.name(fn)` call in `index`.
    fn method_callback(&self, index: usize, name: &str) -> Option<usize> {
        self.method_calls(index, name)
            .iter()
            .find_map(|(_, args)| self.resolve_fn(args.first()?))
    }

    fn this_writes(&self, index: usize) -> BTreeMap<&'a str, &'a Expression<'a>> {
        self.fns[index]
            .assigns
            .iter()
            .filter_map(|assignment| {
                let (object, key) = target_member(&assignment.left)?;
                matches!(strip(object), Expression::ThisExpression(_))
                    .then_some((key, &assignment.right))
            })
            .collect()
    }

    /// `this.key`, or a local whose one value is `this.key`.
    fn holds_this_key(&self, expression: &Expression<'a>, key: &str) -> bool {
        if this_key(expression) == Some(key) {
            return true;
        }
        self.symbol(expression)
            .and_then(|symbol| self.value(symbol))
            .is_some_and(|value| this_key(value) == Some(key))
    }

    /// `T(fn, 0)`: a plain timer call. The first argument is returned.
    fn timer_argument(&self, call: &'a CallExpression<'a>) -> Option<&'a Expression<'a>> {
        let (callee, args) = invocation(call);
        if member(callee).is_some() {
            return None;
        }
        match args.as_slice() {
            [first, second] if is_number(second, 0.0) => Some(*first),
            _ => None,
        }
    }

    /// A function whose only effect is to hand its first slot to a timer.
    fn schedules_slot(&self, index: usize) -> bool {
        match self.fns[index].calls.as_slice() {
            [call] => self
                .timer_argument(call)
                .is_some_and(|argument| self.slot(argument, index) == Some(0)),
            _ => false,
        }
    }

    /// The function a forEach callback schedules for one listener held in its
    /// own first slot, when it does nothing else.
    fn scheduled_listener_thunk(&self, index: usize) -> Option<usize> {
        let [call] = self.fns[index].calls.as_slice() else {
            return None;
        };
        let thunk = self.resolve_fn(self.timer_argument(call)?)?;
        let [inner] = self.fns[thunk].calls.as_slice() else {
            return None;
        };
        let (callee, args) = invocation(inner);
        (args.len() == 1 && self.slot(callee, index) == Some(0)).then_some(thunk)
    }

    /// Every function `index` hands to a timer as `T(thunk, 0)`.
    fn timer_thunks(&self, index: usize) -> Vec<usize> {
        self.fns[index]
            .calls
            .iter()
            .filter_map(|call| self.resolve_fn(self.timer_argument(call)?))
            .collect()
    }

    /// `this.path... === void 0` as the whole body.
    fn returns_unset(&self, index: usize, path: &[&str]) -> bool {
        let Some(Expression::BinaryExpression(binary)) = self.fns[index].only_value().map(strip)
        else {
            return false;
        };
        binary.operator == BinaryOperator::StrictEquality
            && is_void0(&binary.right)
            && this_path(&binary.left, path)
    }

    /// The `new C(fn)` executors of `index` where `C` denotes the class.
    fn executors(&self, index: usize, class: &ClassShape) -> Vec<usize> {
        let Some(symbol) = class.symbol else {
            return Vec::new();
        };
        self.fns[index]
            .news
            .iter()
            .filter(|new| self.denotes(&new.callee, symbol))
            .filter_map(|new| self.resolve_fn(new.arguments.first()?.as_expression()?))
            .collect()
    }

    // ---- classes ----------------------------------------------------------

    fn classes(&self) -> Vec<ClassShape> {
        let mut classes: Vec<ClassShape> = Vec::new();
        let mut by_symbol: BTreeMap<SymbolId, usize> = BTreeMap::new();
        for class in &self.class_nodes {
            let mut shape = ClassShape {
                symbol: class.id.as_ref().and_then(|id| id.symbol_id.get()),
                class_span: Some(class.span),
                ..ClassShape::default()
            };
            for element in &class.body.body {
                let ClassElement::MethodDefinition(method) = element else {
                    continue;
                };
                let Some(index) = self.fn_of_node.get(method.value.node_id.get().index()) else {
                    continue;
                };
                if *index == NONE {
                    continue;
                }
                let index = *index as usize;
                let Some(name) = property_key_name(&method.key) else {
                    continue;
                };
                match (method.kind, method.r#static) {
                    (MethodDefinitionKind::Constructor, _) => shape.ctor = Some(index),
                    (MethodDefinitionKind::Get, _) => {
                        shape.getters.insert(name, index);
                    }
                    (MethodDefinitionKind::Method, true) => {
                        shape.statics.insert(name, index);
                    }
                    (MethodDefinitionKind::Method, false) => {
                        shape.methods.insert(name, index);
                    }
                    _ => {}
                }
            }
            if let Some(symbol) = shape.symbol {
                by_symbol.insert(symbol, classes.len());
            }
            classes.push(shape);
        }
        // Constructor functions with prototype methods. A key written twice
        // proves neither value.
        let mut proto: BTreeMap<SymbolId, BTreeMap<&'a str, Vec<usize>>> = BTreeMap::new();
        let mut statics: Vec<(SymbolId, &'a str, usize)> = Vec::new();
        for assignment in &self.member_writes {
            let Some((object, key)) = target_member(&assignment.left) else {
                continue;
            };
            let Some(value) = self.resolve_fn(&assignment.right) else {
                continue;
            };
            if let Some((owner, "prototype")) = member(object) {
                if let Some(symbol) = self.symbol(owner) {
                    proto
                        .entry(symbol)
                        .or_default()
                        .entry(key)
                        .or_default()
                        .push(value);
                }
            } else if let Some(symbol) = self.symbol(object) {
                statics.push((symbol, key, value));
            }
        }
        for (symbol, methods) in proto {
            let Some(ctor) = self.decl_fn.get(&symbol).copied() else {
                continue;
            };
            if by_symbol.contains_key(&symbol) || !self.fixed_binding(symbol) {
                continue;
            }
            let mut shape = ClassShape {
                symbol: Some(symbol),
                ctor: Some(ctor),
                ..ClassShape::default()
            };
            for (key, values) in methods {
                if let [value] = values.as_slice() {
                    shape.methods.insert(key.to_string(), *value);
                }
            }
            by_symbol.insert(symbol, classes.len());
            classes.push(shape);
        }
        let mut static_counts: BTreeMap<(SymbolId, &'a str), usize> = BTreeMap::new();
        for (symbol, key, _) in &statics {
            *static_counts.entry((*symbol, *key)).or_default() += 1;
        }
        for (symbol, key, value) in statics {
            if static_counts[&(symbol, key)] != 1 {
                continue;
            }
            if let Some(index) = by_symbol.get(&symbol) {
                classes[*index].statics.insert(key.to_string(), value);
            }
        }
        self.descriptor_getters(&mut classes, &by_symbol);
        classes
    }

    /// `Object.defineProperty(C.prototype, name, D[i])` with a `{ key, get }`
    /// descriptor held in `D`.
    fn descriptor_getters(
        &self,
        classes: &mut [ClassShape],
        by_symbol: &BTreeMap<SymbolId, usize>,
    ) {
        for call in &self.all_calls {
            let Some((receiver, "defineProperty", args)) = method_call(call) else {
                continue;
            };
            if !self.is_global(receiver, "Object") {
                continue;
            }
            let [target, _, descriptor] = args.as_slice() else {
                continue;
            };
            let Some((owner, "prototype")) = member(target) else {
                continue;
            };
            let Some(class) = self.symbol(owner).and_then(|symbol| by_symbol.get(&symbol)) else {
                continue;
            };
            let Expression::ComputedMemberExpression(element) = strip(descriptor) else {
                continue;
            };
            let Some(holder) = self.symbol(&element.object) else {
                continue;
            };
            if let Some((name, getter)) = self.descriptor(holder) {
                classes[*class].getters.insert(name, getter);
            }
        }
    }

    fn descriptor(&self, holder: SymbolId) -> Option<(String, usize)> {
        let origins = self.origins(holder)?;
        let mut key = None;
        let mut getters = Vec::new();
        for origin in origins {
            let Expression::ObjectExpression(object) = strip(origin) else {
                continue;
            };
            for property in object_members_of(object) {
                match (property.name.as_str(), strip(property.value)) {
                    ("key", Expression::StringLiteral(literal)) => {
                        key = Some(literal.value.to_string());
                    }
                    ("get", value) => getters.push(self.resolve_fn(value)),
                    _ => {}
                }
            }
        }
        for assignment in &self.member_writes {
            if let Some((object, "get")) = target_member(&assignment.left) {
                if self.symbol(object) == Some(holder) {
                    getters.push(self.resolve_fn(&assignment.right));
                }
            }
        }
        // A descriptor with two getters proves neither.
        match getters.as_slice() {
            [getter] => Some((key?, (*getter)?)),
            _ => None,
        }
    }
}

// ===========================================================================
// Finders
// ===========================================================================

struct Run<'i, 'a> {
    ix: &'i Ix<'a>,
    claims: Vec<FinderClaim>,
    seen: BTreeSet<(&'static str, u32)>,
    purpose_named: BTreeSet<(u32, u32)>,
}

impl Run<'_, '_> {
    fn claim(&mut self, role: &'static str, index: usize, evidence: &str) {
        self.claim_with_alias(role, index, None, evidence);
    }

    fn claim_with_alias(
        &mut self,
        role: &'static str,
        index: usize,
        alias_role: Option<&'static str>,
        evidence: &str,
    ) {
        let span = self.ix.fns[index].span;
        if self.purpose_named.contains(&(span.start, span.end))
            || !self.seen.insert((role, span.start))
        {
            return;
        }
        self.claims.push(FinderClaim::Function {
            role,
            function_span: span,
            alias_role,
            confidence: Confidence::High,
            evidence: vec![evidence.to_string()],
        });
    }

    fn claim_binding(&mut self, role: &'static str, symbol: SymbolId, evidence: &str) {
        let span = self.ix.scoping.symbol_span(symbol);
        if !self.seen.insert((role, span.start)) {
            return;
        }
        self.claims.push(FinderClaim::MainBinding {
            role,
            identifier_span: span,
            confidence: Confidence::High,
            evidence: vec![evidence.to_string()],
        });
    }

    fn claim_class(
        &mut self,
        role: &'static str,
        symbol: SymbolId,
        class_span: Span,
        evidence: &str,
    ) {
        let identifier = self.ix.scoping.symbol_span(symbol);
        if !self.seen.insert((role, identifier.start)) {
            return;
        }
        self.claims.push(FinderClaim::ClassBinding {
            role,
            class_span,
            confidence: Confidence::High,
            evidence: vec![evidence.to_string()],
        });
    }

    // ---- elapsed timer ----------------------------------------------------

    /// `createElapsedTimer`: reads `performance.now`, falls back to a nested
    /// `Number(new Date())` clock, and returns a nested `clock() - baseline`
    /// closure with the baseline on its `.start`.
    fn elapsed_timers(&mut self) -> BTreeSet<usize> {
        let ix = self.ix;
        let mut timers = BTreeSet::new();
        for index in 0..ix.fns.len() {
            let fx = &ix.fns[index];
            if !["performance", "now", "bind"]
                .iter()
                .all(|name| fx.members.contains(name))
            {
                continue;
            }
            let Some(reading) = fx.last_value().and_then(|value| ix.resolve_fn(value)) else {
                continue;
            };
            if ix.fns[reading].parent != Some(index) || !self.is_elapsed_reading(reading) {
                continue;
            }
            let baseline_written = fx.assigns.iter().any(|assignment| {
                target_member(&assignment.left).is_some_and(|(object, key)| {
                    key == "start" && ix.resolve_fn(object) == Some(reading)
                })
            });
            let date_clock = fx
                .nested
                .iter()
                .copied()
                .find(|nested| self.is_date_clock(*nested));
            let (true, Some(date_clock)) = (baseline_written, date_clock) else {
                continue;
            };
            timers.insert(index);
            self.claim(
                ELAPSED_FACTORY,
                index,
                "reads performance.now with a nested Date fallback and returns a clock-minus-baseline closure carrying .start",
            );
            self.claim(
                ELAPSED_DATE_CLOCK,
                date_clock,
                "nested fallback clock returning Number(new Date())",
            );
            self.claim(
                ELAPSED_READING,
                reading,
                "returned closure: captured clock call minus captured baseline",
            );
        }
        timers
    }

    fn is_elapsed_reading(&self, index: usize) -> bool {
        let Some(Expression::BinaryExpression(binary)) = self.ix.fns[index].only_value().map(strip)
        else {
            return false;
        };
        binary.operator == BinaryOperator::Subtraction
            && matches!(strip(&binary.left), Expression::CallExpression(_))
    }

    fn is_date_clock(&self, index: usize) -> bool {
        let ix = self.ix;
        let Some(Expression::CallExpression(call)) = ix.fns[index].only_value().map(strip) else {
            return false;
        };
        if !ix.is_global(&call.callee, "Number") {
            return false;
        }
        matches!(
            call.arguments.first().and_then(Argument::as_expression).map(strip),
            Some(Expression::NewExpression(new)) if ix.is_global(&new.callee, "Date") && new.arguments.is_empty()
        )
    }

    // ---- queues -----------------------------------------------------------

    fn queue_helpers(&mut self, purposes: &[PurposeOutcome]) {
        let ix = self.ix;
        let found = |role: &str| -> Vec<usize> {
            purposes
                .iter()
                .filter_map(|outcome| match outcome {
                    PurposeOutcome::Found(finding) if finding.role == role => ix
                        .fns
                        .iter()
                        .position(|fx| fx.span == finding.function_span),
                    _ => None,
                })
                .collect()
        };
        let sequential = found("sequential_task_queue_factory");
        // A statement-form initial-items default is enough to prove the queue
        // itself, but the older once-wrapper/base attribution must remain the
        // winner for its continuation helper. The purpose finder leaves an
        // explicit structural receipt in the queue evidence for this narrow
        // cross-finder exclusion.
        let sequential_once: Vec<usize> = purposes
            .iter()
            .filter_map(|outcome| match outcome {
                PurposeOutcome::Found(finding)
                    if finding.role == "sequential_task_queue_factory"
                        && !finding.evidence.iter().any(|e| {
                            e == "statement-form initial-items default: preserve the existing once-wrapper/base attribution"
                        }) => ix
                        .fns
                        .iter()
                        .position(|fx| fx.span == finding.function_span),
                _ => None,
            })
            .collect();
        let scheduled = found("scheduled_task_queue_factory");
        let starts = found("task_queue_start");
        let empties = found("task_queue_empty");
        let on_errors = found("task_queue_on_error");
        for factory in sequential.iter().chain(&scheduled).copied() {
            for start in starts
                .iter()
                .copied()
                .filter(|start| ix.fns[*start].parent == Some(factory))
            {
                self.queue_advance(factory, start);
            }
            for empty in empties
                .iter()
                .copied()
                .filter(|empty| ix.fns[*empty].parent == Some(factory))
            {
                if let Some(callback) = ix.method_callback(empty, "forEach") {
                    if ix.schedules_slot(callback) {
                        self.claim(QUEUE_SCHEDULE_EMPTY, callback, "forEach callback of the proven queue empty method handing one listener to the queue timer");
                    }
                }
            }
            for on_error in on_errors
                .iter()
                .copied()
                .filter(|method| ix.fns[*method].parent == Some(factory))
            {
                for thunk in ix.timer_thunks(on_error) {
                    if let [call] = ix.fns[thunk].calls.as_slice() {
                        let (callee, args) = invocation(call);
                        if args.len() == 1 && ix.slot(callee, on_error) == Some(0) {
                            self.claim(QUEUE_LATCHED_ERROR, thunk, "timer thunk of the proven queue onError method calling the new listener with the latched error");
                        }
                    }
                }
            }
        }
        for outcome in purposes {
            if let PurposeOutcome::Ambiguous {
                role: "once_callback_factory",
                candidates,
                ..
            } = outcome
            {
                for span in candidates {
                    if let Some(index) = ix.fns.iter().position(|fx| fx.span == *span) {
                        self.once_factory(index, &sequential_once, &scheduled);
                    }
                }
            }
        }
    }

    /// The function the proven `start` method calls and that shifts tasks is
    /// the queue's advance step.
    fn queue_advance(&mut self, factory: usize, start: usize) {
        let ix = self.ix;
        let advance = ix.fns[start].calls.iter().find_map(|call| {
            let (callee, _) = invocation(call);
            ix.resolve_fn(callee)
                .filter(|target| ix.fns[*target].parent == Some(factory))
        });
        let Some(advance) = advance else {
            return;
        };
        if ix.method_calls(advance, "shift").is_empty()
            || ix.method_calls(advance, "forEach").is_empty()
        {
            return;
        }
        self.claim(QUEUE_ADVANCE, advance, "local of the proven queue factory called by its start method; shifts one task or notifies listeners");
        for (_, args) in ix.method_calls(advance, "forEach") {
            let Some(callback) = args.first().and_then(|argument| ix.resolve_fn(argument)) else {
                continue;
            };
            if ix.schedules_slot(callback) {
                self.claim(QUEUE_SCHEDULE_EMPTY, callback, "forEach callback of the queue advance step handing one listener to the queue timer");
            } else if let Some(thunk) = ix.scheduled_listener_thunk(callback) {
                self.claim(QUEUE_SCHEDULE_ERROR, callback, "forEach callback of the queue advance step scheduling a thunk for one listener");
                self.claim(
                    QUEUE_INVOKE_ERROR,
                    thunk,
                    "timer thunk calling one listener with the error held by the advance step",
                );
            }
        }
        for thunk in ix.timer_thunks(advance) {
            let runs = ix.fns[thunk].calls.iter().any(|call| {
                let (_, args) = invocation(call);
                let [_, Expression::CallExpression(wrap)] = args
                    .iter()
                    .map(|argument| strip(argument))
                    .collect::<Vec<_>>()[..]
                else {
                    return false;
                };
                let (_, wrapped) = invocation(wrap);
                matches!(wrapped.as_slice(), [only] if ix.resolve_fn(only) == Some(advance))
            });
            if runs {
                self.claim(QUEUE_RUN_TASK, thunk, "timer thunk calling the queue runner with the shifted task and a wrapped advance step");
            }
        }
    }

    /// Two structurally equal once factories are told apart by the one queue
    /// factory that consumes each.
    fn once_factory(&mut self, index: usize, sequential: &[usize], scheduled: &[usize]) {
        let ix = self.ix;
        let FnNode::Function(function) = ix.fns[index].func else {
            return;
        };
        let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
            return;
        };
        let aliases = ix.aliases(symbol);
        let [alias] = aliases.as_slice() else {
            return;
        };
        let consumed_by = |factories: &[usize]| {
            factories
                .iter()
                .any(|factory| ix.read_only_inside(*alias, *factory))
        };
        if consumed_by(sequential) {
            self.claim_with_alias(
                SEQUENTIAL_ONCE,
                index,
                Some(SEQUENTIAL_ONCE_ALIAS),
                "proven once wrapper factory read only by the sequential task queue",
            );
        } else if consumed_by(scheduled) {
            self.claim_binding(
                SCHEDULED_ONCE_ALIAS,
                *alias,
                "alias of a proven once wrapper factory read only by the scheduled task queue",
            );
        }
    }

    // ---- small host helpers -----------------------------------------------

    /// `e = document.createEvent("CustomEvent"); e.initCustomEvent(<text>,
    /// true, true, {}); dispatchEvent(e)`, and nothing else.
    fn custom_event_dispatchers(&mut self) {
        let ix = self.ix;
        for index in 0..ix.fns.len() {
            let fx = &ix.fns[index];
            if fx.formal_count() != 0 || fx.effective().len() != 3 || fx.calls.len() != 3 {
                continue;
            }
            let created = ix.method_calls(index, "createEvent");
            let [(document, create_args)] = created.as_slice() else {
                continue;
            };
            let is_custom = matches!(create_args.as_slice(), [Expression::StringLiteral(kind)] if kind.value == "CustomEvent");
            if !ix.is_global(document, "document") || !is_custom {
                continue;
            }
            let init = ix.method_calls(index, "initCustomEvent");
            let bubbling = matches!(init.as_slice(), [(_, args)] if matches!(
                args.as_slice(),
                [Expression::StringLiteral(_), bubbles, cancelable, Expression::ObjectExpression(detail)]
                    if is_true(bubbles) && is_true(cancelable) && detail.properties.is_empty()
            ));
            let dispatched = fx
                .calls
                .iter()
                .any(|call| ix.is_global(&call.callee, "dispatchEvent"));
            if bubbling && dispatched {
                self.claim(CUSTOM_EVENT, index, "creates a CustomEvent, initializes it bubbling and cancelable with empty detail, and dispatches it globally");
            }
        }
    }

    /// The function installed as `document.onreadystatechange` that tests
    /// `document.readyState === "interactive"` before one call.
    fn ready_state_handlers(&mut self) {
        let ix = self.ix;
        for assignment in &ix.member_writes {
            let Some((object, "onreadystatechange")) = target_member(&assignment.left) else {
                continue;
            };
            if !ix.is_global(object, "document") {
                continue;
            }
            let Some(index) = ix.resolve_fn(&assignment.right) else {
                continue;
            };
            let tests_ready = matches!(ix.fns[index].ifs.as_slice(), [statement] if matches!(
                strip(&statement.test),
                Expression::BinaryExpression(binary)
                    if binary.operator == BinaryOperator::StrictEquality
                        && matches!(member(&binary.left), Some((target, "readyState")) if ix.is_global(target, "document"))
                        && matches!(strip(&binary.right), Expression::StringLiteral(state) if state.value == "interactive")
            ));
            if tests_ready && ix.fns[index].calls.len() == 1 {
                self.claim(READY_STATE, index, "installed as document.onreadystatechange; calls one function when readyState is interactive");
            }
        }
    }

    /// An empty function whose alias is only read to overwrite local callback
    /// holders. The sibling empty function with an unread alias does not match.
    fn released_callback_noops(&mut self) {
        let ix = self.ix;
        for (symbol, index) in ix.decl_fn.iter().map(|(symbol, index)| (*symbol, *index)) {
            let fx = &ix.fns[index];
            if fx.formal_count() != 0 || !fx.effective().is_empty() || !ix.fixed_binding(symbol) {
                continue;
            }
            let aliases = ix.aliases(symbol);
            let [alias] = aliases.as_slice() else {
                continue;
            };
            let mut reads = 0;
            let overwrites_only = ix.scoping.get_resolved_references(*alias).all(|reference| {
                if !reference.is_read() {
                    return true;
                }
                reads += 1;
                let AstKind::AssignmentExpression(assignment) =
                    ix.nodes.parent_kind(reference.node_id())
                else {
                    return false;
                };
                matches!(
                    assignment.left,
                    AssignmentTarget::AssignmentTargetIdentifier(_)
                ) && ix
                    .owner_of(reference.node_id())
                    .is_some_and(|owner| ix.fns[owner].parent.is_some())
            });
            if overwrites_only && reads > 0 {
                self.claim(RELEASED_NOOP, index, "empty function whose alias is only read to overwrite callback holders inside nested functions");
            }
        }
    }

    // ---- reactive classes -------------------------------------------------

    /// Constructor `_cleanup`/`_observer`, a guarded idempotent `unsubscribe`
    /// and an `isComplete` over `_observer`.
    fn subscription_class(&mut self, class: &ClassShape) -> Option<()> {
        let ix = self.ix;
        let ctor = class.ctor?;
        let writes = ix.this_writes(ctor);
        let shaped = writes.len() == 2
            && is_void0(writes.get("_cleanup")?)
            && ix.direct_slot(writes.get("_observer")?, ctor) == Some(0);
        let complete = class.method("isComplete")?;
        let unsubscribe = class.method("unsubscribe")?;
        let closed = class.getters.get("closed").copied();
        if !shaped || !ix.returns_unset(complete, &["_observer"]) {
            return None;
        }
        let unsub_writes = ix.this_writes(unsubscribe);
        let runs_cleanup = ix.fns[unsubscribe].calls.iter().any(|call| {
            let (callee, _) = invocation(call);
            ix.holds_this_key(callee, "_cleanup")
        });
        let guarded = ix
            .method_calls(unsubscribe, "isComplete")
            .iter()
            .any(|(receiver, _)| matches!(strip(receiver), Expression::ThisExpression(_)));
        let clears_state =
            unsub_writes.contains_key("_observer") && unsub_writes.contains_key("_cleanup");
        if !(guarded && runs_cleanup && clears_state && ix.fns[unsubscribe].tries > 0) {
            return None;
        }
        self.claim(
            SUB_CTOR,
            ctor,
            "constructor storing slot 0 as _observer with an unset _cleanup",
        );
        self.claim(
            SUB_UNSUBSCRIBE,
            unsubscribe,
            "guarded by isComplete; clears _observer and _cleanup and runs the teardown in a try",
        );
        self.claim(SUB_COMPLETE, complete, "returns this._observer === void 0");
        if let Some(closed) = closed.filter(|closed| ix.returns_unset(*closed, &["_observer"])) {
            self.claim(
                SUB_CLOSED,
                closed,
                "closed accessor returning this._observer === void 0",
            );
        }
        if let (Some(symbol), Some(span)) = (class.symbol, class.class_span) {
            self.claim_class(
                SUB_CLASS,
                symbol,
                span,
                "class declaration whose constructor stores _observer and whose unsubscribe/isComplete/closed shape is proven",
            );
        }
        for alias in ix.aliases(class.symbol?) {
            self.claim_binding(
                SUB_CLASS_ALIAS,
                alias,
                "single-origin alias of the proven subscription class",
            );
        }
        Some(())
    }

    /// Constructor `_subscription`; `next`, `error` and `complete` that read
    /// the subscription's `_observer`, and an `isComplete` delegate.
    fn subscription_observer_class(&mut self, class: &ClassShape) -> Option<()> {
        let ix = self.ix;
        let ctor = class.ctor?;
        let writes = ix.this_writes(ctor);
        if writes.len() != 1 || ix.direct_slot(writes.get("_subscription")?, ctor) != Some(0) {
            return None;
        }
        let next = class.method("next")?;
        let error = class.method("error")?;
        let complete = class.method("complete")?;
        let is_complete = class.method("isComplete")?;
        let forwards_next = ix
            .method_calls(next, "next")
            .iter()
            .any(|(receiver, args)| {
                this_path(receiver, &["_subscription", "_observer"])
                    && args
                        .first()
                        .is_some_and(|argument| ix.slot(argument, next) == Some(0))
            });
        let delegates = matches!(ix.fns[is_complete].only_value().map(strip), Some(Expression::CallExpression(call))
            if matches!(method_call(call), Some((receiver, "isComplete", args))
                if args.is_empty() && this_key(receiver) == Some("_subscription")));
        if !forwards_next
            || !delegates
            || !self.releases_observer(error, "error", "complete")
            || !self.releases_observer(complete, "complete", "error")
        {
            return None;
        }
        self.claim(
            OBS_CTOR,
            ctor,
            "constructor storing slot 0 as _subscription",
        );
        self.claim(
            OBS_NEXT,
            next,
            "forwards slot 0 to this._subscription._observer.next while it is set",
        );
        self.claim(OBS_ERROR, error, "clears the subscription's _observer and _cleanup, runs the teardown and forwards to observer.error");
        self.claim(
            OBS_DONE,
            complete,
            "clears the subscription's _observer, calls observer.complete and runs the teardown",
        );
        self.claim(
            OBS_IS_COMPLETE,
            is_complete,
            "delegates to this._subscription.isComplete()",
        );
        if let Some(closed) = class.getters.get("closed").copied() {
            if ix.returns_unset(closed, &["_subscription", "_observer"]) {
                self.claim(
                    OBS_CLOSED,
                    closed,
                    "closed accessor returning this._subscription._observer === void 0",
                );
            }
        }
        if let (Some(symbol), Some(span)) = (class.symbol, class.class_span) {
            self.claim_class(
                OBS_CLASS,
                symbol,
                span,
                "class declaration whose constructor stores _subscription and whose next/error/complete/isComplete shape is proven",
            );
        }
        for alias in ix.aliases(class.symbol?) {
            self.claim_binding(
                OBS_CLASS_ALIAS,
                alias,
                "single-origin alias of the proven subscription observer class",
            );
        }
        Some(())
    }

    /// Clears `_observer` and `_cleanup` on the handle read from
    /// `this._subscription` and reads the observer's `member` callback.
    fn releases_observer(&self, index: usize, member_name: &str, other: &str) -> bool {
        let ix = self.ix;
        let fx = &ix.fns[index];
        let clears = |key: &str| {
            fx.assigns.iter().any(|assignment| {
                target_member(&assignment.left).is_some_and(|(object, written)| {
                    written == key
                        && is_void0(&assignment.right)
                        && ix.holds_this_key(object, "_subscription")
                })
            })
        };
        clears("_observer")
            && clears("_cleanup")
            && fx.tries > 0
            && fx.members.contains(member_name)
            && !fx.members.contains(other)
    }

    fn observable_class(&mut self, class: &ClassShape) -> Option<()> {
        let ix = self.ix;
        let ctor = class.ctor?;
        let writes = ix.this_writes(ctor);
        let rejects = matches!(ix.fns[ctor].ifs.as_slice(), [statement] if is_typeof_not_function(&statement.test));
        if writes.len() != 1
            || ix.direct_slot(writes.get("_subscriber")?, ctor) != Some(0)
            || !rejects
            || ix.fns[ctor].throws == 0
        {
            return None;
        }
        let subscribe = class.method("subscribe")?;
        let pipe = class.method("pipe")?;
        let of = class.static_method("of")?;
        let empty = class.static_method("empty")?;
        let from = class.static_method("from")?;
        let of_executor = *ix.executors(of, class).first()?;
        let empty_executor = *ix.executors(empty, class).first()?;
        let of_emit = ix.method_callback(of_executor, "forEach")?;
        let pipe_step = ix
            .method_calls(pipe, "reduce")
            .iter()
            .find_map(|(_, args)| match args.as_slice() {
                [step, Expression::ThisExpression(_)] => ix.resolve_fn(step),
                _ => None,
            })?;
        let teardown = self.subscribe_teardown(subscribe)?;
        let applies = matches!(ix.fns[pipe_step].only_value().map(strip), Some(Expression::CallExpression(call)) if {
            let (callee, args) = invocation(call);
            ix.direct_slot(callee, pipe_step) == Some(1)
                && matches!(args.as_slice(), [only] if ix.direct_slot(only, pipe_step) == Some(0))
        });
        let completes_only = matches!(ix.fns[empty_executor].calls.as_slice(), [_])
            && !ix.method_calls(empty_executor, "complete").is_empty();
        let emits = !ix.method_calls(of_emit, "next").is_empty()
            && !ix.method_calls(of_executor, "complete").is_empty();
        if !applies || !completes_only || !emits {
            return None;
        }
        self.claim(
            OBSERVABLE_CTOR,
            ctor,
            "constructor rejecting a non-function and storing it as _subscriber",
        );
        self.claim(OBSERVABLE_SUBSCRIBE, subscribe, "normalizes the observer, builds subscription and observer handles and stores the executor teardown in _cleanup");
        self.claim(
            OBSERVABLE_PIPE,
            pipe,
            "reduces the collected operators over this",
        );
        self.claim(
            PIPE_STEP,
            pipe_step,
            "reduce step applying one operator to the accumulated observable",
        );
        self.claim(
            OBSERVABLE_OF,
            of,
            "static building this class with an executor over its collected arguments",
        );
        self.claim(
            OF_EXECUTOR,
            of_executor,
            "executor emitting each captured value with next, then complete",
        );
        self.claim(
            OF_EMIT,
            of_emit,
            "forEach callback forwarding one value to next",
        );
        self.claim(
            OBSERVABLE_EMPTY,
            empty,
            "static building this class with a complete-only executor",
        );
        self.claim(
            EMPTY_EXECUTOR,
            empty_executor,
            "executor that only calls complete",
        );
        self.claim_binding(TEARDOWN_ALIAS, teardown, "called by subscribe with _subscriber and the observer handle; its result fills _cleanup and it normalizes a function or .unsubscribe result");
        self.observable_from(class, from);
        self.observable_operator_method(class, "map", OBSERVABLE_MAP, MAP_ALIAS);
        self.observable_operator_method(
            class,
            "unorderedFlatMap",
            OBSERVABLE_FLAT_MAP,
            FLAT_MAP_ALIAS,
        );
        Some(())
    }

    /// `handle._cleanup = K(this._subscriber, observerHandle)` in `subscribe`:
    /// the teardown normalizer `K`, proven by its own body.
    fn subscribe_teardown(&self, subscribe: usize) -> Option<SymbolId> {
        let ix = self.ix;
        let fx = &ix.fns[subscribe];
        if fx.news.len() < 2
            || !["start", "next", "error", "complete"]
                .iter()
                .all(|key| fx.object_keys.contains(*key))
        {
            return None;
        }
        let call = fx.assigns.iter().find_map(|assignment| {
            match (target_member(&assignment.left), strip(&assignment.right)) {
                (Some((_, "_cleanup")), Expression::CallExpression(call)) => Some(&**call),
                _ => None,
            }
        });
        let (callee, args) = invocation(call?);
        if !matches!(args.first(), Some(first) if this_key(first) == Some("_subscriber")) {
            return None;
        }
        let symbol = ix.symbol(callee)?;
        let normalizer = ix.resolve_fn(callee)?;
        let runs_executor = ix.fns[normalizer].calls.iter().any(|call| {
            let (callee, args) = invocation(call);
            ix.direct_slot(callee, normalizer) == Some(0)
                && matches!(args.as_slice(), [only] if ix.direct_slot(only, normalizer) == Some(1))
        });
        let tests_function = ix.fns[normalizer]
            .ifs
            .iter()
            .any(|statement| is_typeof_function(&statement.test));
        (runs_executor && tests_function && ix.fns[normalizer].members.contains("unsubscribe"))
            .then_some(symbol)
    }

    fn observable_from(&mut self, class: &ClassShape, from: usize) {
        let ix = self.ix;
        let (mut promise, mut array, mut iterable) = (None, None, None);
        for executor in ix.executors(from, class) {
            let schedules = !ix.method_calls(executor, "schedule").is_empty();
            if !ix.method_calls(executor, "then").is_empty()
                && !ix.method_calls(executor, "catch").is_empty()
            {
                promise = Some(executor);
            } else if schedules && ix.fns[executor].members.contains("iterator") {
                iterable = Some(executor);
            } else if schedules {
                array = Some(executor);
            }
        }
        let (Some(promise), Some(array), Some(iterable)) = (promise, array, iterable) else {
            return;
        };
        let (Some(on_value), Some(on_rejection)) = (
            ix.method_callback(promise, "then"),
            ix.method_callback(promise, "catch"),
        ) else {
            return;
        };
        let (Some(array_step), Some(iterable_step)) = (
            ix.method_callback(array, "schedule"),
            ix.method_callback(iterable, "schedule"),
        ) else {
            return;
        };
        let emits = |index: usize| {
            ["next", "complete", "error"]
                .iter()
                .all(|name| !ix.method_calls(index, name).is_empty())
        };
        let value_ok = !ix.method_calls(on_value, "next").is_empty()
            && !ix.method_calls(on_value, "complete").is_empty();
        if !value_ok
            || ix.method_calls(on_rejection, "error").is_empty()
            || !emits(array_step)
            || !ix.fns[iterable_step].members.contains("done")
            || ix.fns[array_step].members.contains("done")
        {
            return;
        }
        self.claim(
            OBSERVABLE_FROM,
            from,
            "static converting promise, array-like and iterable inputs with one executor each",
        );
        self.claim(
            PROMISE_EXECUTOR,
            promise,
            "executor chaining then and catch on the captured promise",
        );
        self.claim(
            PROMISE_VALUE,
            on_value,
            "fulfillment handler: next then complete",
        );
        self.claim(PROMISE_REJECTION, on_rejection, "rejection handler: error");
        self.claim(
            ARRAY_EXECUTOR,
            array,
            "executor scheduling a step or looping over an array-like",
        );
        self.claim(
            ARRAY_STEP,
            array_step,
            "scheduled step emitting one indexed element",
        );
        self.claim(
            ITERABLE_EXECUTOR,
            iterable,
            "executor reading Symbol.iterator and scheduling a step or draining it",
        );
        self.claim(
            ITERABLE_STEP,
            iterable_step,
            "scheduled step pulling one iterator result",
        );
    }

    /// `name(f) { return this.pipe(K(f)); }` with `K` an operator factory:
    /// it returns a function that returns `new C(executor)`, and the executor
    /// subscribes to the source.
    fn observable_operator_method(
        &mut self,
        class: &ClassShape,
        name: &str,
        method_role: &'static str,
        alias_role: &'static str,
    ) -> Option<()> {
        let ix = self.ix;
        let method = class.method(name)?;
        let Expression::CallExpression(call) = strip(ix.fns[method].only_value()?) else {
            return None;
        };
        let (receiver, "pipe", args) = method_call(call)? else {
            return None;
        };
        if !matches!(strip(receiver), Expression::ThisExpression(_)) {
            return None;
        }
        let [Expression::CallExpression(operator)] = args
            .iter()
            .map(|argument| strip(argument))
            .collect::<Vec<_>>()[..]
        else {
            return None;
        };
        let (factory, factory_args) = invocation(operator);
        let [project] = factory_args.as_slice() else {
            return None;
        };
        if ix.direct_slot(project, method) != Some(0) {
            return None;
        }
        let symbol = ix.symbol(factory)?;
        let target = ix.resolve_fn(factory)?;
        let operation = ix.resolve_fn(ix.fns[target].last_value()?)?;
        let Expression::NewExpression(new) = strip(ix.fns[operation].last_value()?) else {
            return None;
        };
        let executor = ix.resolve_fn(new.arguments.first()?.as_expression()?)?;
        if ix.method_calls(executor, "subscribe").is_empty() {
            return None;
        }
        self.claim(method_role, method, "returns this.pipe(operator(slot 0))");
        self.claim_binding(alias_role, symbol, "the operator factory the proven observable method pipes; it returns an operation building a new observable that subscribes to its source");
        Some(())
    }

    // ---- batch scheduler and notifier -------------------------------------

    /// `setTimeout(this._process.bind(this, slot), 0)` in `index`.
    fn reschedules(&self, index: usize) -> bool {
        let ix = self.ix;
        ix.fns[index].calls.iter().any(|call| {
            let (callee, args) = invocation(call);
            if !ix.is_global(callee, "setTimeout") || args.len() != 2 || !is_number(args[1], 0.0) {
                return false;
            }
            matches!(strip(args[0]), Expression::CallExpression(bind)
                if matches!(method_call(bind), Some((target, "bind", bound))
                    if this_key(target) == Some("_process")
                        && matches!(bound.as_slice(), [receiver, slot]
                            if matches!(strip(receiver), Expression::ThisExpression(_))
                                && ix.direct_slot(slot, index) == Some(0))))
        })
    }

    fn batch_scheduler_class(
        &mut self,
        class: &ClassShape,
        timers: &BTreeSet<usize>,
    ) -> Option<()> {
        let ix = self.ix;
        let ctor = class.ctor?;
        let writes = ix.this_writes(ctor);
        let Expression::CallExpression(clock) = strip(writes.get("_internalNow")?) else {
            return None;
        };
        let clock_ok = ix
            .resolve_fn(&clock.callee)
            .is_some_and(|factory| timers.contains(&factory));
        if writes.len() != 2
            || ix.direct_slot(writes.get("targetBatchTimeMs")?, ctor) != Some(0)
            || !clock_ok
        {
            return None;
        }
        let now = class.method("now")?;
        let schedule = class.method("schedule")?;
        let process = class.method("_process")?;
        let reads_clock = matches!(ix.fns[now].only_value().map(strip), Some(Expression::CallExpression(call))
            if this_key(&call.callee) == Some("_internalNow") && call.arguments.is_empty());
        if !reads_clock
            || !self.reschedules(schedule)
            || !self.reschedules(process)
            || ix.fns[process].loops == 0
            || ix.method_calls(process, "now").len() < 2
        {
            return None;
        }
        self.claim(
            BATCH_CTOR,
            ctor,
            "constructor storing a time budget and a clock from the proven elapsed timer factory",
        );
        self.claim(BATCH_NOW, now, "returns this._internalNow()");
        self.claim(
            BATCH_SCHEDULE,
            schedule,
            "arms a zero timer for this._process bound to the work function",
        );
        self.claim(
            BATCH_PROCESS,
            process,
            "loops the work function under the time budget and re-arms itself while work remains",
        );
        if let (Some(symbol), Some(span)) = (class.symbol, class.class_span) {
            self.claim_class(
                BATCH_CLASS,
                symbol,
                span,
                "class declaration whose constructor stores a proven elapsed clock and whose now/schedule/_process shape is proven",
            );
        }
        Some(())
    }

    fn callback_notifier_class(&mut self, class: &ClassShape) -> Option<()> {
        let ix = self.ix;
        let ctor = class.ctor?;
        let writes = ix.this_writes(ctor);
        if writes.len() != 1 || !is_empty_array(writes.get("callbacks")?) {
            return None;
        }
        let register = class.method("register")?;
        let unregister = class.method("unregister")?;
        let clear = class.method("clear")?;
        let notify = class.method("notify")?;
        let chains = |index: usize| {
            matches!(
                ix.fns[index].last_value().map(strip),
                Some(Expression::ThisExpression(_))
            )
        };
        let appends = ix
            .method_calls(register, "push")
            .iter()
            .any(|(receiver, args)| {
                this_key(receiver) == Some("callbacks")
                    && matches!(args.as_slice(), [only] if ix.slot(only, register) == Some(0))
            });
        let blanks = ix.fns[unregister].assigns.iter().any(|assignment| {
            is_void0(&assignment.right)
                && matches!(&assignment.left, AssignmentTarget::ComputedMemberExpression(element)
                    if this_key(&element.object) == Some("callbacks"))
        });
        let resets = ix
            .this_writes(clear)
            .get("callbacks")
            .is_some_and(|value| is_empty_array(value));
        let walks = ix.fns[notify].loops > 0
            && ix.fns[notify].tries > 0
            && ix.fns[notify].members.contains("slice")
            && ix.fns[notify].calls.iter().any(|call| {
                let (_, args) = invocation(call);
                matches!(args.as_slice(), [first, _] if ix.slot(first, notify) == Some(0))
            });
        if !(appends
            && blanks
            && resets
            && walks
            && chains(register)
            && chains(unregister)
            && chains(clear))
        {
            return None;
        }
        self.claim(
            NOTIFIER_CTOR,
            ctor,
            "constructor initializing this.callbacks to an empty list",
        );
        self.claim(
            NOTIFIER_REGISTER,
            register,
            "appends slot 0 to this.callbacks and returns this",
        );
        self.claim(
            NOTIFIER_UNREGISTER,
            unregister,
            "blanks matching entries of this.callbacks in a loop and returns this",
        );
        self.claim(
            NOTIFIER_CLEAR,
            clear,
            "replaces this.callbacks with an empty list and returns this",
        );
        self.claim(NOTIFIER_NOTIFY, notify, "walks a copy of this.callbacks in a loop, calling each with the payload and the accumulator in a try");
        if let (Some(symbol), Some(span)) = (class.symbol, class.class_span) {
            self.claim_class(
                NOTIFIER_CLASS,
                symbol,
                span,
                "class declaration whose constructor initializes this.callbacks and whose register/unregister/clear/notify shape is proven",
            );
        }
        Some(())
    }
}

// ===========================================================================
// Expression helpers
// ===========================================================================

fn strip<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    match expression {
        Expression::ParenthesizedExpression(inner) => strip(&inner.expression),
        _ => expression,
    }
}

fn is_void0(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void)
}

fn is_true(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::BooleanLiteral(literal) if literal.value)
}

fn is_number(expression: &Expression<'_>, value: f64) -> bool {
    matches!(strip(expression), Expression::NumericLiteral(literal) if literal.value == value)
}

fn is_empty_array(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::ArrayExpression(array) if array.elements.is_empty())
}

/// `typeof x === "function"` with the operator chosen by the caller.
fn typeof_function_test(expression: &Expression<'_>, operator: BinaryOperator) -> bool {
    let Expression::BinaryExpression(binary) = strip(expression) else {
        return false;
    };
    binary.operator == operator
        && matches!(strip(&binary.left), Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Typeof)
        && matches!(strip(&binary.right), Expression::StringLiteral(literal) if literal.value == "function")
}

fn is_typeof_function(expression: &Expression<'_>) -> bool {
    typeof_function_test(expression, BinaryOperator::StrictEquality)
}

fn is_typeof_not_function(expression: &Expression<'_>) -> bool {
    typeof_function_test(expression, BinaryOperator::StrictInequality)
}

/// `object.name` or `object["name"]`.
fn member<'e, 'a>(expression: &'e Expression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
    match strip(expression) {
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

/// `(object, key)` of a member assignment target.
fn target_member<'e, 'a>(
    target: &'e AssignmentTarget<'a>,
) -> Option<(&'e Expression<'a>, &'e str)> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

/// `this.name` / `this["name"]`.
fn this_key<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    let (object, name) = member(expression)?;
    matches!(strip(object), Expression::ThisExpression(_)).then_some(name)
}

/// `this.a.b...` for the whole of `path`.
fn this_path(expression: &Expression<'_>, path: &[&str]) -> bool {
    match path.split_last() {
        None => matches!(strip(expression), Expression::ThisExpression(_)),
        Some((last, rest)) => {
            matches!(member(expression), Some((object, name)) if name == *last && this_path(object, rest))
        }
    }
}

fn call_args<'e, 'a>(call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
    call.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

/// A call with `.call(void 0, ...)` unwrapped.
fn invocation<'e, 'a>(
    call: &'e CallExpression<'a>,
) -> (&'e Expression<'a>, Vec<&'e Expression<'a>>) {
    let args = call_args(call);
    if let Some((object, "call")) = member(&call.callee) {
        if args.first().is_some_and(|first| is_void0(first)) {
            return (object, args[1..].to_vec());
        }
    }
    (&call.callee, args)
}

/// `(receiver, method, args)` of `o.m(a)` or `o.m.call(o, a)`.
fn method_call<'e, 'a>(
    call: &'e CallExpression<'a>,
) -> Option<(&'e Expression<'a>, &'e str, Vec<&'e Expression<'a>>)> {
    let (object, name) = member(&call.callee)?;
    let args = call_args(call);
    if name == "call" && !args.is_empty() {
        if let Some((receiver, method)) = member(object) {
            return Some((receiver, method, args[1..].to_vec()));
        }
    }
    Some((object, name, args))
}
