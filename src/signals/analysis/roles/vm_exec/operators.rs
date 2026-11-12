//! Package `p02_exec_a`: the first half of the execution framework.
//!
//! Names the reactive operators (`scan`, `last`, `reduce`, `map`, unordered
//! flat-map, fork-join, callback-to-Observable), their executors, observers and
//! teardowns, the subscribe teardown normalizer, the document-ready gate, the
//! callback-registry leftovers, the singleton and `Object.assign` helpers and
//! the inner functions of the time-budgeted task queue.
//!
//! Everything hangs off one anchor proven by structure: the MAIN binding that
//! holds the result of a zero-argument factory returning a class with
//! `subscribe` and `pipe` methods. An operator is a function returning a
//! function that returns `new <that binding>(executor)`; the executor subscribes
//! the source and its observers are told apart by what they do with their own
//! slots and the subscriber slot. Observers may be positional callbacks or an
//! observer object built in steps, and may be inline arrows or hoisted
//! declarations; formals and `arguments[N]` are the same slot.
//!
//! See `roles/mod.rs` for the recognition rules. The roles `roles::purposes` already proves (the
//! registry factory, notify and register, the queue factory and its API
//! methods, the task runners, the once wrapper) are used nowhere here.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{
    AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator,
};

use crate::signals::extract::bindings::{object_members_of, property_key_name, ParameterSlots};
use crate::signals::schema::Confidence;
use crate::signals::analysis::roles::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};
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
    // Callback registry leftovers and its uncurried array helpers.
    function("callback_registry_unregister", "unregisterCallback"),
    function("callback_registry_merge", "mergeCallbacks"),
    binding("registry_invoke_method", "invokeMethod"),
    binding("registry_find_index_method", "findCallbackIndex"),
    binding("registry_append_method", "appendCallback"),
    binding("registry_concat_method", "mergeCallbackLists"),
    binding("registry_slice_method", "copyCallbackList"),
    // The subscribe teardown normalizer and the guarded signal task call.
    function("subscribe_run_executor", "innerSubscribe"),
    function("subscribe_unsubscribe_closure", "unsubscribeInner"),
    function("signal_task_guarded_invoke", "invokeSignalTaskSafely"),
    // Empty functions.
    function("unreferenced_noop", "unreferencedNoop"),
    function("held_noop", "noop"),
    // Reduce.
    function("reduce_factory", "createReduceOperator"),
    function("reduce_operator", "reduceOperatorFunction"),
    // Scan.
    function("scan_factory", "createScanOperator"),
    alias("scan_factory_alias", "scanOperatorFactory"),
    function("scan_operator", "scanOperatorFunction"),
    function("scan_executor", "scanSourceExecutor"),
    function("scan_next", "accumulateScanValue"),
    function("scan_error", "forwardScanError"),
    function("scan_complete", "completeScanSubscriber"),
    function("scan_teardown", "unsubscribeScanSource"),
    binding("scan_no_seed_sentinel", "noSeedSentinel"),
    // Last.
    function("last_factory", "createLastOperator"),
    alias("last_factory_alias", "lastOperatorFactory"),
    function("last_operator", "lastOperatorFunction"),
    function("last_executor", "lastSourceExecutor"),
    function("last_next", "storeLastValue"),
    function("last_error", "forwardLastError"),
    function("last_complete", "emitLastValue"),
    function("last_teardown", "unsubscribeLastSource"),
    binding("last_no_value_sentinel", "noValueSentinel"),
    // Map.
    function("map_factory", "createMapOperator"),
    function("map_operator", "mapOperatorFunction"),
    function("map_executor", "mapSourceExecutor"),
    function("map_next", "mapNextHandler"),
    function("map_error", "forwardMapError"),
    function("map_complete", "completeMapSubscriber"),
    function("map_teardown", "unsubscribeMapSource"),
    // Unordered flat-map.
    function("flat_map_factory", "createUnorderedFlatMapOperator"),
    function("flat_map_operator", "unorderedFlatMapOperatorFunction"),
    function("flat_map_executor", "unorderedFlatMapExecutor"),
    function("flat_map_inner_next", "forwardInnerNext"),
    function("flat_map_error", "forwardInnerError"),
    function("flat_map_complete", "completeInner"),
    function("flat_map_outer_next", "subscribeFlatMapInner"),
    function("flat_map_teardown", "unsubscribeFlatMapSources"),
    function("flat_map_teardown_each", "unsubscribeFlatMapEntry"),
    // Fork-join.
    function("fork_join_factory", "forkJoinObservables"),
    function("fork_join_executor", "forkJoinExecutor"),
    function("fork_join_subscribe_source", "subscribeForkJoinSource"),
    function("fork_join_next", "storeResultAtIndex"),
    function("fork_join_complete", "completeForkJoinSource"),
    function("fork_join_error", "failForkJoinSource"),
    function("fork_join_error_each", "unsubscribeForkJoinEntryOnError"),
    function("fork_join_teardown", "unsubscribeForkJoinSources"),
    function("fork_join_teardown_each", "unsubscribeForkJoinEntry"),
    // Callback to Observable.
    function("from_callback_factory", "observableFromCallback"),
    function("from_callback_wrapper", "invokeCallbackAsObservable"),
    function("from_callback_executor", "callbackObservableExecutor"),
    function("from_callback_success", "emitCallbackResult"),
    function("from_callback_failure", "forwardCallbackError"),
    // Document-ready gate.
    function("ready_queue_flush", "flushReadyQueue"),
    binding("ready_queue_pending", "pendingReadyCallbacks"),
    binding("ready_queue_flushed", "documentReady"),
    // Small helpers.
    function("object_assign_helper", "assignObjectProperties"),
    function("has_own_property_helper", "hasOwn"),
    binding("has_own_property_method", "hasOwnPropertyBuiltin"),
    function("singleton_factory", "createSingletonFactory"),
    function("singleton_getter", "getOrCreateSingleton"),
    function("scheduled_queue_once_factory", "createOnceContinuation"),
    // Time-budgeted task queue internals.
    function("task_queue_drain", "drainTaskQueue"),
    function("task_queue_step", "runNextQueuedTask"),
    function("task_queue_step_schedule_empty", "scheduleEmptyListenerFromStep"),
    function("task_queue_schedule_error", "scheduleErrorListener"),
    function("task_queue_deliver_error", "deliverQueueError"),
    function("task_queue_drain_schedule_empty", "scheduleEmptyListenerFromDrain"),
    function("task_queue_discard_schedule_empty", "scheduleEmptyListenerFromDiscard"),
    function("task_queue_deliver_late_error", "deliverLateQueueError"),
    binding("task_queue_default_clock_factory", "elapsedTimerFactory"),
];

// ===========================================================================
// Structural index
// ===========================================================================

const NONE: u32 = u32::MAX;

#[derive(Clone, Copy)]
enum FnNode<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
}

/// One function or arrow and the nodes it owns directly.
struct Unit<'a> {
    span: Span,
    func: FnNode<'a>,
    parent: Option<usize>,
    nested: Vec<usize>,
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    returns: Vec<&'a ReturnStatement<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    updates: Vec<&'a UpdateExpression<'a>>,
    ifs: Vec<&'a IfStatement<'a>>,
    tries: usize,
    throws: usize,
    loops: usize,
}

impl<'a> Unit<'a> {
    fn new(span: Span, func: FnNode<'a>, parent: Option<usize>) -> Self {
        Self {
            span,
            func,
            parent,
            nested: Vec::new(),
            calls: Vec::new(),
            news: Vec::new(),
            returns: Vec::new(),
            assignments: Vec::new(),
            updates: Vec::new(),
            ifs: Vec::new(),
            tries: 0,
            throws: 0,
            loops: 0,
        }
    }

    fn statements(&self) -> &'a [Statement<'a>] {
        match self.func {
            FnNode::Function(function) => {
                function.body.as_ref().map(|body| body.statements.as_slice()).unwrap_or(&[])
            }
            FnNode::Arrow(arrow) => arrow.body.statements.as_slice(),
        }
    }

    fn formals(&self) -> usize {
        match self.func {
            FnNode::Function(function) => function.params.items.len(),
            FnNode::Arrow(arrow) => arrow.params.items.len(),
        }
    }

    /// The values the function returns: every `return` argument, plus the body
    /// expression of a concise arrow.
    fn results(&self) -> Vec<&'a Expression<'a>> {
        let mut out: Vec<&'a Expression<'a>> =
            self.returns.iter().filter_map(|statement| statement.argument.as_ref()).collect();
        if let FnNode::Arrow(arrow) = self.func {
            if arrow.expression {
                if let Some(Statement::ExpressionStatement(statement)) =
                    arrow.body.statements.first()
                {
                    out.push(&statement.expression);
                }
            }
        }
        out
    }

    /// True for a body with no statement other than `return void 0`.
    fn is_empty_body(&self) -> bool {
        self.statements().iter().all(|statement| match statement {
            Statement::ReturnStatement(statement) => {
                statement.argument.as_ref().is_none_or(is_void0)
            }
            Statement::EmptyStatement(_) => true,
            _ => false,
        })
    }
}

/// What a value expression is, after following single-origin aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Val {
    Fn(usize),
    Slot(usize, u32),
    /// A binding with no single origin (several writes, or a class).
    Sym(SymbolId),
    /// The origin expression itself, by span.
    Expr(Span),
}

/// A call normalized to `receiver.name(args)`: `r.m(a)`, `r["m"](a)` and
/// `r["m"].call(r, a)` read the same.
struct MethodCall<'a> {
    receiver: &'a Expression<'a>,
    name: &'a str,
    args: Vec<&'a Expression<'a>>,
}

struct Index<'c, 'a> {
    ctx: &'c FinderCtx<'c, 'a>,
    slots: ParameterSlots<'a, 'a>,
    units: Vec<Unit<'a>>,
    owner: Vec<u32>,
    fn_of_node: Vec<u32>,
    decl_fn: BTreeMap<SymbolId, usize>,
    classes: BTreeMap<SymbolId, &'a Class<'a>>,
    plain_writes: BTreeMap<SymbolId, Vec<&'a Expression<'a>>>,
    member_writes: BTreeMap<SymbolId, Vec<(String, &'a Expression<'a>)>>,
    prototype_writes: BTreeMap<SymbolId, BTreeSet<String>>,
    /// Bindings declared with an initializer.
    initialized: Vec<SymbolId>,
    all_calls: Vec<&'a CallExpression<'a>>,
}

impl<'c, 'a> Index<'c, 'a> {
    fn new(ctx: &'c FinderCtx<'c, 'a>) -> Self {
        let count = ctx.nodes.len();
        let mut index = Self {
            ctx,
            slots: ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes),
            units: Vec::new(),
            owner: vec![NONE; count],
            fn_of_node: vec![NONE; count],
            decl_fn: BTreeMap::new(),
            classes: BTreeMap::new(),
            plain_writes: BTreeMap::new(),
            member_writes: BTreeMap::new(),
            prototype_writes: BTreeMap::new(),
            initialized: Vec::new(),
            all_calls: Vec::new(),
        };
        index.collect();
        index
    }

    fn collect(&mut self) {
        let nodes = self.ctx.nodes;
        for (id, node) in nodes.iter_enumerated() {
            let at = id.index();
            let parent = nodes.parent_id(id);
            let owner = if parent == id || parent.index() >= at {
                NONE
            } else if self.fn_of_node[parent.index()] != NONE {
                self.fn_of_node[parent.index()]
            } else {
                self.owner[parent.index()]
            };
            self.owner[at] = owner;
            let owner = (owner != NONE).then_some(owner as usize);
            let func = match node.kind() {
                AstKind::Function(function) => Some((function.span, FnNode::Function(function))),
                AstKind::ArrowFunctionExpression(arrow) => {
                    Some((arrow.span, FnNode::Arrow(arrow)))
                }
                _ => None,
            };
            if let Some((span, func)) = func {
                let unit = self.units.len();
                self.units.push(Unit::new(span, func, owner));
                self.fn_of_node[at] = unit as u32;
                if let Some(owner) = owner {
                    self.units[owner].nested.push(unit);
                }
                if let FnNode::Function(function) = func {
                    if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        self.decl_fn.insert(symbol, unit);
                    }
                }
                continue;
            }
            self.collect_node(node.kind(), owner);
        }
    }

    fn collect_node(&mut self, kind: AstKind<'a>, owner: Option<usize>) {
        match kind {
            AstKind::Class(class) => {
                if let Some(symbol) = class.id.as_ref().and_then(|id| id.symbol_id.get()) {
                    self.classes.insert(symbol, class);
                }
            }
            AstKind::CallExpression(call) => self.all_calls.push(call),
            AstKind::VariableDeclarator(declarator) => {
                if let (BindingPattern::BindingIdentifier(binding), Some(_)) =
                    (&declarator.id, declarator.init.as_ref())
                {
                    self.initialized.extend(binding.symbol_id.get());
                }
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign =>
            {
                self.collect_write(assignment);
            }
            _ => {}
        }
        let Some(owner) = owner else {
            return;
        };
        let unit = &mut self.units[owner];
        match kind {
            AstKind::CallExpression(call) => unit.calls.push(call),
            AstKind::NewExpression(new) => unit.news.push(new),
            AstKind::ReturnStatement(statement) => unit.returns.push(statement),
            AstKind::AssignmentExpression(assignment) => unit.assignments.push(assignment),
            AstKind::UpdateExpression(update) => unit.updates.push(update),
            AstKind::IfStatement(statement) => unit.ifs.push(statement),
            AstKind::TryStatement(_) => unit.tries += 1,
            AstKind::ThrowStatement(_) => unit.throws += 1,
            AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::ForStatement(_)
            | AstKind::ForInStatement(_) => unit.loops += 1,
            _ => {}
        }
    }

    fn collect_write(&mut self, assignment: &'a AssignmentExpression<'a>) {
        let scoping = self.ctx.scoping;
        match &assignment.left {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                if let Some(symbol) = analysis::identifier_symbol(identifier, scoping) {
                    self.plain_writes.entry(symbol).or_default().push(&assignment.right);
                }
            }
            target => {
                let Some(member) = target.as_member_expression() else {
                    return;
                };
                let Some((object, name)) = member_parts(member) else {
                    return;
                };
                if let Some(symbol) = analysis::expression_symbol(strip(object), scoping) {
                    self.member_writes
                        .entry(symbol)
                        .or_default()
                        .push((name.to_string(), &assignment.right));
                } else if let Some((root, "prototype")) = expression_member(object) {
                    if let Some(symbol) = analysis::expression_symbol(strip(root), scoping) {
                        self.prototype_writes.entry(symbol).or_default().insert(name.to_string());
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Values
    // -----------------------------------------------------------------------

    fn owner_of(&self, node: NodeId) -> Option<usize> {
        let owner = *self.owner.get(node.index())?;
        (owner != NONE).then_some(owner as usize)
    }

    /// The one non-`void 0` value a binding ever receives. `None` for several
    /// declarations, several values or a write that is not a plain assignment.
    fn origin(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        let scoping = self.ctx.scoping;
        if !scoping.symbol_redeclarations(symbol).is_empty() {
            return None;
        }
        let writes = self.plain_writes.get(&symbol).map(Vec::as_slice).unwrap_or(&[]);
        let write_count =
            scoping.get_resolved_references(symbol).filter(|reference| reference.is_write()).count();
        if write_count != writes.len() {
            return None;
        }
        let mut values: Vec<&'a Expression<'a>> = writes.to_vec();
        if let AstKind::VariableDeclarator(declarator) =
            self.ctx.nodes.kind(scoping.symbol_declaration(symbol))
        {
            values.extend(declarator.init.as_ref());
        }
        values.retain(|value| !is_void0(value));
        match values.as_slice() {
            [value] => Some(value),
            _ => None,
        }
    }

    fn slot(&self, expression: &Expression<'_>) -> Option<(usize, u32)> {
        let slot = self.slots.incoming_slot(expression)?;
        let unit = *self.fn_of_node.get(slot.owner.index())?;
        (unit != NONE).then_some((unit as usize, slot.index))
    }

    /// Follow single-origin aliases to the expression that holds the value.
    fn terminal(&self, expression: &'a Expression<'a>) -> &'a Expression<'a> {
        let mut current = strip(expression);
        for _ in 0..8 {
            if self.slot(current).is_some() {
                break;
            }
            let Some(symbol) = analysis::expression_symbol(current, self.ctx.scoping) else {
                break;
            };
            if self.decl_fn.contains_key(&symbol) {
                break;
            }
            let Some(next) = self.origin(symbol) else {
                break;
            };
            current = strip(next);
        }
        current
    }

    fn val(&self, expression: &'a Expression<'a>) -> Val {
        let terminal = self.terminal(expression);
        if let Some((unit, slot)) = self.slot(terminal) {
            return Val::Slot(unit, slot);
        }
        let node = match terminal {
            Expression::FunctionExpression(function) => Some(function.node_id.get()),
            Expression::ArrowFunctionExpression(arrow) => Some(arrow.node_id.get()),
            _ => None,
        };
        if let Some(node) = node {
            if let Some(&unit) = self.fn_of_node.get(node.index()) {
                if unit != NONE {
                    return Val::Fn(unit as usize);
                }
            }
        }
        if let Some(symbol) = analysis::expression_symbol(terminal, self.ctx.scoping) {
            if let Some(&unit) = self.decl_fn.get(&symbol) {
                return Val::Fn(unit);
            }
            return Val::Sym(symbol);
        }
        Val::Expr(terminal.span())
    }

    fn fn_val(&self, expression: &'a Expression<'a>) -> Option<usize> {
        match self.val(expression) {
            Val::Fn(unit) => Some(unit),
            _ => None,
        }
    }

    fn is_slot(&self, expression: &'a Expression<'a>, unit: usize, slot: u32) -> bool {
        self.val(expression) == Val::Slot(unit, slot)
    }

    // -----------------------------------------------------------------------
    // Calls
    // -----------------------------------------------------------------------

    fn same_text(&self, left: &Expression<'_>, right: &Expression<'_>) -> bool {
        let text = |span: Span| self.ctx.source.get(span.start as usize..span.end as usize);
        text(strip(left).span()) == text(strip(right).span())
    }

    fn method(&self, call: &'a CallExpression<'a>) -> Option<MethodCall<'a>> {
        let (object, name) = expression_member(&call.callee)?;
        let args = plain_arguments(call)?;
        if name == "call" {
            if let (Some((receiver, inner)), Some(first)) = (expression_member(object), args.first())
            {
                if self.same_text(receiver, first) {
                    return Some(MethodCall { receiver, name: inner, args: args[1..].to_vec() });
                }
            }
        }
        Some(MethodCall { receiver: object, name, args })
    }

    /// The callee and arguments of a plain call; `f.call(void 0, a)` is `f(a)`.
    fn plain(&self, call: &'a CallExpression<'a>) -> Option<(&'a Expression<'a>, Vec<&'a Expression<'a>>)> {
        let args = plain_arguments(call)?;
        if let Some((object, "call")) = expression_member(&call.callee) {
            if args.first().is_some_and(|first| is_void0(first)) {
                return Some((object, args[1..].to_vec()));
            }
        }
        Some((&call.callee, args))
    }

    fn methods(&self, unit: usize, name: &str) -> Vec<MethodCall<'a>> {
        self.units[unit]
            .calls
            .iter()
            .filter_map(|call| self.method(call))
            .filter(|method| method.name == name)
            .collect()
    }

    fn calls_sink(&self, unit: usize, sink: Val, name: &str) -> usize {
        self.methods(unit, name).iter().filter(|method| self.val(method.receiver) == sink).count()
    }

    /// `fn(v) { sink.<name>(v) }` with `slot_arg`, or `fn() { sink.<name>() }`.
    fn forwards(&self, unit: usize, sink: Val, name: &str, slot_arg: bool) -> bool {
        let facts = &self.units[unit];
        let [call] = facts.calls.as_slice() else {
            return false;
        };
        let Some(method) = self.method(call) else {
            return false;
        };
        let arguments = match method.args.as_slice() {
            [] => !slot_arg,
            [value] => slot_arg && self.is_slot(value, unit, 0),
            _ => false,
        };
        arguments
            && method.name == name
            && self.val(method.receiver) == sink
            && facts.nested.is_empty()
            && facts.tries == 0
    }

    /// `setTimeout(value, 0)` through the global, as `(value)`.
    fn timer_argument(&self, call: &'a CallExpression<'a>) -> Option<&'a Expression<'a>> {
        let (callee, args) = self.plain(call)?;
        let Expression::Identifier(identifier) = strip(callee) else {
            return None;
        };
        let global = identifier.name == "setTimeout"
            && analysis::identifier_symbol(identifier, self.ctx.scoping).is_none();
        match args.as_slice() {
            [value, delay] if global && is_zero(delay) => Some(value),
            _ => None,
        }
    }

    fn subscribe_site(&self, unit: usize) -> Option<(MethodCall<'a>, Span)> {
        let mut sites = self.units[unit].calls.iter().filter_map(|call| {
            let method = self.method(call)?;
            (method.name == "subscribe").then_some((method, call.span))
        });
        let site = sites.next()?;
        sites.next().is_none().then_some(site)
    }

    /// Members of an object held by `expression`: literal keys, then the
    /// `object.key = value` writes that overwrite them.
    fn object_members(&self, expression: &'a Expression<'a>) -> BTreeMap<String, &'a Expression<'a>> {
        let mut members = BTreeMap::new();
        let Some(symbol) = analysis::expression_symbol(strip(expression), self.ctx.scoping) else {
            return members;
        };
        let Some(Expression::ObjectExpression(object)) = self.origin(symbol).map(strip) else {
            return members;
        };
        for member in object_members_of(object) {
            members.insert(member.name, member.value);
        }
        for (name, value) in self.member_writes.get(&symbol).map(Vec::as_slice).unwrap_or(&[]) {
            members.insert(name.clone(), *value);
        }
        members
    }

    /// `[next, error, complete]` of a subscribe call: three positional
    /// callbacks or one observer object.
    fn observer(&self, args: &[&'a Expression<'a>]) -> Option<[usize; 3]> {
        match args {
            [next, error, complete] => {
                Some([self.fn_val(next)?, self.fn_val(error)?, self.fn_val(complete)?])
            }
            [object] => {
                let members = self.object_members(object);
                Some([
                    self.fn_val(members.get("next")?)?,
                    self.fn_val(members.get("error")?)?,
                    self.fn_val(members.get("complete")?)?,
                ])
            }
            _ => None,
        }
    }

    /// The nested function a unit returns as its only result.
    fn returned_fn(&self, unit: usize) -> Option<usize> {
        let [result] = self.units[unit].results()[..] else {
            return None;
        };
        let inner = self.fn_val(result)?;
        (self.units[inner].parent == Some(unit)).then_some(inner)
    }

    /// `unit` returns `recv.unsubscribe()` of the subscription made at `site`.
    fn unsubscribes(&self, unit: usize, site: Span) -> bool {
        let methods = self.methods(unit, "unsubscribe");
        self.units[unit].calls.len() == 1
            && matches!(methods.as_slice(), [method]
                if method.args.is_empty() && self.val(method.receiver) == Val::Expr(site))
    }

    /// `entry => entry.unsubscribe()`.
    fn unsubscribes_entry(&self, unit: usize) -> bool {
        let methods = self.methods(unit, "unsubscribe");
        self.units[unit].calls.len() == 1
            && matches!(methods.as_slice(), [method]
                if method.args.is_empty() && self.is_slot(method.receiver, unit, 0))
    }

    /// The per-entry callback of a nested `list.forEach(entry => entry.unsubscribe())`.
    fn unsubscribe_each(&self, unit: usize) -> Option<usize> {
        let [method] = &self.methods(unit, "forEach")[..] else {
            return None;
        };
        let [callback] = method.args.as_slice() else {
            return None;
        };
        let entry = self.fn_val(callback)?;
        (self.units[entry].parent == Some(unit) && self.unsubscribes_entry(entry)).then_some(entry)
    }

    /// `if (x <= 0) ...` after `--x`: one decrement and one exhausted test.
    fn counts_down(&self, unit: usize) -> bool {
        let facts = &self.units[unit];
        matches!(facts.updates.as_slice(), [update] if update.operator == UpdateOperator::Decrement)
            && matches!(facts.ifs.as_slice(), [statement] if matches!(strip(&statement.test),
                Expression::BinaryExpression(test) if test.operator == BinaryOperator::LessEqualThan))
    }

    /// A MAIN `{}` compared with `===`/`!==` in `unit`.
    fn compared_sentinel(&self, unit: usize, main: usize) -> Option<SymbolId> {
        for statement in &self.units[unit].ifs {
            let Expression::BinaryExpression(test) = strip(&statement.test) else {
                continue;
            };
            if !matches!(test.operator, BinaryOperator::StrictEquality | BinaryOperator::StrictInequality) {
                continue;
            }
            for side in [&test.left, &test.right] {
                let Some(symbol) = analysis::expression_symbol(strip(side), self.ctx.scoping) else {
                    continue;
                };
                if self.is_main_sentinel(symbol, main) {
                    return Some(symbol);
                }
            }
        }
        None
    }

    fn is_main_sentinel(&self, symbol: SymbolId, main: usize) -> bool {
        self.declared_in(symbol, main)
            && matches!(self.origin(symbol).map(strip),
                Some(Expression::ObjectExpression(object)) if object.properties.is_empty())
    }

    fn declared_in(&self, symbol: SymbolId, unit: usize) -> bool {
        self.owner_of(self.ctx.scoping.symbol_declaration(symbol)) == Some(unit)
    }

    fn reads_symbol(&self, value: &Expression<'_>, symbol: SymbolId) -> bool {
        analysis::expression_symbol(strip(value), self.ctx.scoping) == Some(symbol)
    }
}

// ---------------------------------------------------------------------------
// Pure readers
// ---------------------------------------------------------------------------

fn strip<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    match expression {
        Expression::ParenthesizedExpression(inner) => strip(&inner.expression),
        _ => expression,
    }
}

fn assignment_statement<'a>(statement: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = strip(&statement.expression) else {
        return None;
    };
    (assignment.operator == AssignmentOperator::Assign).then_some(assignment)
}

fn assigned_symbol(assignment: &AssignmentExpression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    analysis::identifier_symbol(identifier, scoping)
}

fn one_declarator<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(declarator)
}

fn is_void0(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::UnaryExpression(unary)
        if unary.operator == UnaryOperator::Void && matches!(unary.argument, Expression::NumericLiteral(_)))
}

fn is_zero(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::NumericLiteral(literal) if literal.value == 0.0)
}

fn member_parts<'a>(member: &'a MemberExpression<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    match member {
        MemberExpression::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        MemberExpression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

fn expression_member<'a>(expression: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
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

/// The arguments of a call without spreads.
fn plain_arguments<'a>(call: &'a CallExpression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    call.arguments.iter().map(Argument::as_expression).collect()
}

fn is_unbound(index: &Index<'_, '_>, expression: &Expression<'_>, name: &str) -> bool {
    matches!(strip(expression), Expression::Identifier(identifier)
        if identifier.name == name
            && analysis::identifier_symbol(identifier, index.ctx.scoping).is_none())
}

// ===========================================================================
// Recognizers
// ===========================================================================

/// The shape every pipeable operator shares.
struct Operator<'a> {
    operator: usize,
    executor: usize,
    site: MethodCall<'a>,
    site_span: Span,
    next: usize,
    error: usize,
    complete: usize,
    teardown: usize,
}

struct Run<'r, 'c, 'a> {
    index: &'r Index<'c, 'a>,
    claims: Vec<FinderClaim>,
    /// The expression span of the `factory()` call MAIN stores the class in.
    observable: Option<Span>,
    main: Option<usize>,
}

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let index = Index::new(ctx);
    let mut run = Run { index: &index, claims: Vec::new(), observable: None, main: None };
    run.observable_constructor();
    run.registry();
    run.subscribe_normalizer();
    run.signal_task_invoke();
    run.once_continuation();
    run.ready_queue();
    run.object_assign();
    run.has_own_property();
    run.singleton();
    run.task_queue();
    if run.main.is_some() {
        run.empty_functions();
        let scan = run.scan();
        let last = run.last();
        run.reduce(&scan, &last);
        run.map();
        run.flat_map();
        run.fork_join();
        run.callback_observable();
    }
    one_function_per_role(run.claims)
}

/// A role names one function. Candidates this module cannot tell apart would
/// all receive the same name, so every claim of such a role is dropped.
fn one_function_per_role(claims: Vec<FinderClaim>) -> Vec<FinderClaim> {
    let mut spans: BTreeMap<&'static str, BTreeSet<Span>> = BTreeMap::new();
    for claim in &claims {
        let span = match claim {
            FinderClaim::Function { function_span, .. } => *function_span,
            FinderClaim::MainBinding { identifier_span, .. } => *identifier_span,
            FinderClaim::ClassBinding { class_span, .. } => *class_span,
        };
        spans.entry(claim.role()).or_default().insert(span);
    }
    claims.into_iter().filter(|claim| spans[claim.role()].len() == 1).collect()
}

impl<'a> Run<'_, '_, 'a> {
    fn function(&mut self, role: &'static str, unit: usize, evidence: &str) {
        self.claim(role, unit, None, evidence);
    }

    fn claim(&mut self, role: &'static str, unit: usize, alias_role: Option<&'static str>, evidence: &str) {
        self.claims.push(FinderClaim::Function {
            role,
            function_span: self.index.units[unit].span,
            alias_role,
            confidence: Confidence::High,
            evidence: vec![evidence.to_string()],
        });
    }

    fn binding(&mut self, role: &'static str, symbol: SymbolId, evidence: &str) {
        self.claims.push(FinderClaim::MainBinding {
            role,
            identifier_span: self.index.ctx.scoping.symbol_span(symbol),
            confidence: Confidence::High,
            evidence: vec![evidence.to_string()],
        });
    }

    fn units(&self) -> std::ops::Range<usize> {
        0..self.index.units.len()
    }

    // -----------------------------------------------------------------------
    // Observable anchor
    // -----------------------------------------------------------------------

    /// True when `unit` takes no slot and returns a class with `subscribe` and
    /// `pipe` methods, declared in class or prototype form.
    fn returns_observable_class(&self, unit: usize) -> bool {
        let index = self.index;
        let facts = &index.units[unit];
        if facts.formals() != 0 {
            return false;
        }
        let [result] = facts.results()[..] else {
            return false;
        };
        let names: BTreeSet<String> = match index.val(result) {
            Val::Sym(symbol) => match index.classes.get(&symbol) {
                Some(class) => class
                    .body
                    .body
                    .iter()
                    .filter_map(|element| match element {
                        ClassElement::MethodDefinition(method) => property_key_name(&method.key),
                        _ => None,
                    })
                    .collect(),
                None => index.prototype_writes.get(&symbol).cloned().unwrap_or_default(),
            },
            Val::Fn(class) => {
                let FnNode::Function(function) = index.units[class].func else {
                    return false;
                };
                function
                    .id
                    .as_ref()
                    .and_then(|id| id.symbol_id.get())
                    .and_then(|symbol| index.prototype_writes.get(&symbol).cloned())
                    .unwrap_or_default()
            }
            _ => return false,
        };
        names.contains("subscribe") && names.contains("pipe")
    }

    /// The anchor: the one binding holding the Observable class. The plan
    /// already names it, so it is read here and never claimed.
    fn observable_constructor(&mut self) {
        let index = self.index;
        let mut found: Vec<(Span, usize)> = Vec::new();
        let symbols: BTreeSet<SymbolId> =
            index.plain_writes.keys().chain(&index.initialized).copied().collect();
        for symbol in symbols {
            let Some(Expression::CallExpression(call)) = index.origin(symbol).map(strip) else {
                continue;
            };
            let Some(factory) = index.fn_val(&call.callee) else {
                continue;
            };
            if !call.arguments.is_empty() || !self.returns_observable_class(factory) {
                continue;
            }
            let Some(main) = index.owner_of(index.ctx.scoping.symbol_declaration(symbol)) else {
                continue;
            };
            found.push((call.span, main));
        }
        if let [(span, main)] = found[..] {
            self.observable = Some(span);
            self.main = Some(main);
        }
    }

    /// The executor of `new <Observable>(executor)` when `unit` returns it.
    fn returned_executor(&self, unit: usize) -> Option<usize> {
        let index = self.index;
        let [result] = index.units[unit].results()[..] else {
            return None;
        };
        let Expression::NewExpression(new) = index.terminal(result) else {
            return None;
        };
        if index.val(&new.callee) != Val::Expr(self.observable?) {
            return None;
        }
        let [argument] = new.arguments.as_slice() else {
            return None;
        };
        let executor = index.fn_val(argument.as_expression()?)?;
        (index.units[executor].parent == Some(unit)).then_some(executor)
    }

    fn operator(&self, factory: usize) -> Option<Operator<'a>> {
        let index = self.index;
        let operator = index.returned_fn(factory)?;
        let executor = self.returned_executor(operator)?;
        let (site, site_span) = index.subscribe_site(executor)?;
        if !index.is_slot(site.receiver, operator, 0) {
            return None;
        }
        let [next, error, complete] = index.observer(&site.args)?;
        let teardown = index.returned_fn(executor)?;
        Some(Operator { operator, executor, site, site_span, next, error, complete, teardown })
    }

    // -----------------------------------------------------------------------
    // Scan, last, reduce
    // -----------------------------------------------------------------------

    fn scan(&mut self) -> Vec<usize> {
        let index = self.index;
        let main = self.main.expect("checked by find");
        let mut factories = Vec::new();
        for factory in self.units() {
            let Some(op) = self.operator(factory) else {
                continue;
            };
            let sink = Val::Slot(op.executor, 0);
            let Some(sentinel) = index.compared_sentinel(op.next, main) else {
                continue;
            };
            let next = &index.units[op.next];
            let projects = next.calls.iter().any(|call| {
                index.plain(call).is_some_and(|(callee, args)| {
                    args.len() == 2 && index.is_slot(callee, factory, 0)
                })
            });
            let defaults_seed = index.units[factory]
                .assignments
                .iter()
                .any(|assignment| index.reads_symbol(&assignment.right, sentinel));
            if !projects
                || !defaults_seed
                || next.tries != 1
                || index.calls_sink(op.next, sink, "next") != 1
                || index.calls_sink(op.next, sink, "error") != 1
                || !index.forwards(op.error, sink, "error", true)
                || !index.forwards(op.complete, sink, "complete", false)
                || !index.unsubscribes(op.teardown, op.site_span)
            {
                continue;
            }
            self.claim(
                "scan_factory",
                factory,
                Some("scan_factory_alias"),
                "operator factory whose next folds slot 0 over an accumulator that starts at a no-seed sentinel",
            );
            self.function("scan_operator", op.operator, "operator function of the scan factory: returns a new Observable over its source");
            self.function("scan_executor", op.executor, "executor subscribing the scan source and returning its teardown");
            self.function("scan_next", op.next, "adopts the first value on the sentinel, then emits slot-0 projections and turns a throw into error");
            self.function("scan_error", op.error, "forwards its slot 0 to the subscriber error");
            self.function("scan_complete", op.complete, "completes the subscriber");
            self.function("scan_teardown", op.teardown, "unsubscribes the source subscription");
            self.binding("scan_no_seed_sentinel", sentinel, "MAIN empty object compared by identity as the missing-seed marker of scan");
            factories.push(factory);
        }
        factories
    }

    fn last(&mut self) -> Vec<usize> {
        let index = self.index;
        let main = self.main.expect("checked by find");
        let mut factories = Vec::new();
        for factory in self.units() {
            let Some(op) = self.operator(factory) else {
                continue;
            };
            let sink = Val::Slot(op.executor, 0);
            let Some(sentinel) = index.compared_sentinel(op.complete, main) else {
                continue;
            };
            let stores_filtered = index.units[op.next].calls.iter().any(|call| {
                index.plain(call).is_some_and(|(callee, args)| {
                    args.len() == 1 && index.is_slot(callee, factory, 0)
                })
            });
            let emits_default = index.methods(op.complete, "next").iter().any(|method| {
                matches!(method.args.as_slice(), [value] if index.is_slot(value, factory, 1))
            });
            if !stores_filtered
                || !emits_default
                || index.units[op.next].tries != 1
                || index.calls_sink(op.next, sink, "next") != 0
                || index.calls_sink(op.complete, sink, "next") != 2
                || index.calls_sink(op.complete, sink, "complete") != 2
                || index.calls_sink(op.complete, sink, "error") != 1
                || !index.forwards(op.error, sink, "error", true)
                || !index.unsubscribes(op.teardown, op.site_span)
            {
                continue;
            }
            self.claim(
                "last_factory",
                factory,
                Some("last_factory_alias"),
                "operator factory that stores accepted values and emits the final one or its slot-1 default on completion",
            );
            self.function("last_operator", op.operator, "operator function of the last factory: returns a new Observable over its source");
            self.function("last_executor", op.executor, "executor subscribing the last source and returning its teardown");
            self.function("last_next", op.next, "stores the value when slot 0 of the factory accepts it, or always without a predicate");
            self.function("last_error", op.error, "forwards its slot 0 to the subscriber error");
            self.function("last_complete", op.complete, "emits the stored value or the default, or errors on an empty sequence");
            self.function("last_teardown", op.teardown, "unsubscribes the source subscription");
            self.binding("last_no_value_sentinel", sentinel, "MAIN empty object compared by identity as the nothing-stored marker of last");
            factories.push(factory);
        }
        factories
    }

    fn reduce(&mut self, scan: &[usize], last: &[usize]) {
        let index = self.index;
        for factory in self.units() {
            let Some(operator) = index.returned_fn(factory) else {
                continue;
            };
            let [result] = index.units[operator].results()[..] else {
                continue;
            };
            let Expression::CallExpression(call) = strip(result) else {
                continue;
            };
            let Some(pipe) = index.method(call) else {
                continue;
            };
            let [first, second] = pipe.args.as_slice() else {
                continue;
            };
            if pipe.name != "pipe" || !index.is_slot(pipe.receiver, operator, 0) {
                continue;
            }
            let stage = |value: &'a Expression<'a>| {
                let Expression::CallExpression(call) = strip(value) else {
                    return None;
                };
                let (callee, args) = index.plain(call)?;
                Some((index.fn_val(callee)?, args))
            };
            let (Some((scanner, scan_args)), Some((taker, take_args))) = (stage(first), stage(second)) else {
                continue;
            };
            let matches_stages = scan.contains(&scanner)
                && last.contains(&taker)
                && matches!(scan_args.as_slice(), [project, seed]
                    if index.is_slot(project, factory, 0) && index.is_slot(seed, factory, 1))
                && matches!(take_args.as_slice(), [filter, seed]
                    if is_void0(filter) && index.is_slot(seed, factory, 1));
            if matches_stages {
                self.function("reduce_factory", factory, "operator factory piping its source through the proven scan and last with one seed");
                self.function("reduce_operator", operator, "pipes its source through scan(project, seed) then last(undefined, seed)");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Map, unordered flat-map, fork-join, callback to Observable
    // -----------------------------------------------------------------------

    fn map(&mut self) {
        let index = self.index;
        for factory in self.units() {
            let Some(op) = self.operator(factory) else {
                continue;
            };
            let sink = Val::Slot(op.executor, 0);
            let projected = index.methods(op.next, "next").iter().any(|method| {
                index.val(method.receiver) == sink
                    && matches!(method.args.as_slice(), [value] if matches!(strip(value),
                        Expression::CallExpression(call) if index.plain(call).is_some_and(|(callee, args)| {
                            index.is_slot(callee, factory, 0)
                                && matches!(args.as_slice(), [value] if index.is_slot(value, op.next, 0))
                        })))
            });
            if !projected
                || !matches!(op.site.args.as_slice(), [_])
                || index.units[op.next].tries != 1
                || index.calls_sink(op.next, sink, "error") != 1
                || !index.forwards(op.error, sink, "error", true)
                || !index.forwards(op.complete, sink, "complete", false)
                || !index.unsubscribes(op.teardown, op.site_span)
            {
                continue;
            }
            self.function("map_factory", factory, "operator factory emitting slot 0 of each value and turning a throw into error");
            self.function("map_operator", op.operator, "operator function of the map factory: returns a new Observable over its source");
            self.function("map_executor", op.executor, "executor building the observer object, subscribing the source and returning its teardown");
            self.function("map_next", op.next, "emits the factory's slot-0 projection of its value; a throw becomes error");
            self.function("map_error", op.error, "forwards its slot 0 to the subscriber error");
            self.function("map_complete", op.complete, "completes the subscriber");
            self.function("map_teardown", op.teardown, "unsubscribes the source subscription");
        }
    }

    fn flat_map(&mut self) {
        let index = self.index;
        for factory in self.units() {
            let Some(op) = self.operator(factory) else {
                continue;
            };
            let sink = Val::Slot(op.executor, 0);
            let Some((inner, _)) = index.subscribe_site(op.next) else {
                continue;
            };
            let projected = matches!(strip(index.terminal(inner.receiver)),
                Expression::CallExpression(call) if index.plain(call).is_some_and(|(callee, args)| {
                    index.is_slot(callee, factory, 0)
                        && matches!(args.as_slice(), [value] if index.is_slot(value, op.next, 0))
                }));
            let Some([inner_next, inner_error, inner_complete]) = index.observer(&inner.args) else {
                continue;
            };
            let Some(each) = index.unsubscribe_each(op.teardown) else {
                continue;
            };
            let outer = &index.units[op.next];
            let shared = inner_error == op.error && inner_complete == op.complete;
            let counts_up = matches!(outer.updates.as_slice(), [update] if update.operator == UpdateOperator::Increment);
            if !projected
                || !shared
                || !counts_up
                || index.methods(op.next, "push").len() != 1
                || !index.forwards(inner_next, sink, "next", true)
                || !index.forwards(op.error, sink, "error", true)
                || !index.counts_down(op.complete)
                || index.calls_sink(op.complete, sink, "complete") != 1
            {
                continue;
            }
            self.function("flat_map_factory", factory, "operator factory subscribing slot 0 of each value and forwarding every inner stream unordered");
            self.function("flat_map_operator", op.operator, "operator function of the flat-map factory: returns a new Observable over its source");
            self.function("flat_map_executor", op.executor, "executor tracking active streams and every subscription, subscribing the outer source");
            self.function("flat_map_inner_next", inner_next, "forwards an inner value to the subscriber");
            self.function("flat_map_error", op.error, "forwards an outer or inner error to the subscriber");
            self.function("flat_map_complete", op.complete, "shared by outer and inner completion: counts down and completes at zero");
            self.function("flat_map_outer_next", op.next, "counts up, subscribes the projected inner stream and records its subscription");
            self.function("flat_map_teardown", op.teardown, "unsubscribes every recorded subscription");
            self.function("flat_map_teardown_each", each, "unsubscribes one recorded subscription");
        }
    }

    fn fork_join(&mut self) {
        let index = self.index;
        for factory in self.units() {
            let Some(executor) = self.returned_executor(factory) else {
                continue;
            };
            let sink = Val::Slot(executor, 0);
            let [mapper] = &index.methods(executor, "map")[..] else {
                continue;
            };
            let [callback] = mapper.args.as_slice() else {
                continue;
            };
            let Some(source) = index.fn_val(callback) else {
                continue;
            };
            let Some((site, _)) = index.subscribe_site(source) else {
                continue;
            };
            let Some([next, error, complete]) = index.observer(&site.args) else {
                continue;
            };
            let Some(teardown) = index.returned_fn(executor) else {
                continue;
            };
            let (Some(teardown_each), Some(error_each)) =
                (index.unsubscribe_each(teardown), index.unsubscribe_each(error))
            else {
                continue;
            };
            let stores_by_index = index.units[next].assignments.iter().any(|assignment| {
                let Some(member) = assignment.left.as_member_expression() else {
                    return false;
                };
                matches!(member, MemberExpression::ComputedMemberExpression(target)
                    if index.is_slot(&target.expression, source, 1))
                    && index.is_slot(&assignment.right, next, 0)
            });
            if !index.is_slot(mapper.receiver, factory, 0)
                || !index.is_slot(site.receiver, source, 0)
                || !stores_by_index
                || !index.counts_down(complete)
                || index.calls_sink(complete, sink, "next") != 1
                || index.calls_sink(complete, sink, "complete") != 1
                || index.calls_sink(error, sink, "error") != 1
                || index.calls_sink(executor, sink, "next") != 1
                || index.calls_sink(executor, sink, "complete") != 1
            {
                continue;
            }
            self.function("fork_join_factory", factory, "returns a new Observable joining every source of its slot-0 list");
            self.function("fork_join_executor", executor, "emits an empty list at once, or subscribes every source and returns the teardown");
            self.function("fork_join_subscribe_source", source, "builds an index-bound observer and subscribes one source");
            self.function("fork_join_next", next, "stores the value at the source index");
            self.function("fork_join_complete", complete, "counts down; at zero emits the results and completes");
            self.function("fork_join_error", error, "errors the subscriber, then unsubscribes every source");
            self.function("fork_join_error_each", error_each, "unsubscribes one source after an error");
            self.function("fork_join_teardown", teardown, "unsubscribes every source");
            self.function("fork_join_teardown_each", teardown_each, "unsubscribes one source");
        }
    }

    fn callback_observable(&mut self) {
        let index = self.index;
        for factory in self.units() {
            let Some(wrapper) = index.returned_fn(factory) else {
                continue;
            };
            let Some(executor) = self.returned_executor(wrapper) else {
                continue;
            };
            let sink = Val::Slot(executor, 0);
            let [apply] = &index.methods(executor, "apply")[..] else {
                continue;
            };
            let [_, arguments] = apply.args.as_slice() else {
                continue;
            };
            let Expression::CallExpression(concat) = strip(index.terminal(arguments)) else {
                continue;
            };
            let Some(concat) = index.method(concat) else {
                continue;
            };
            let [success, failure] = concat.args.as_slice() else {
                continue;
            };
            let (Some(success), Some(failure)) = (index.fn_val(success), index.fn_val(failure)) else {
                continue;
            };
            if concat.name != "concat"
                || !index.is_slot(apply.receiver, factory, 0)
                || index.units[wrapper].loops != 1
                || index.calls_sink(executor, sink, "error") != 1
                || index.calls_sink(success, sink, "next") != 1
                || index.calls_sink(success, sink, "complete") != 1
                || !index.forwards(failure, sink, "error", true)
            {
                continue;
            }
            self.function("from_callback_factory", factory, "wraps slot 0 in a function returning a cold Observable over it");
            self.function("from_callback_wrapper", wrapper, "copies its arguments and returns a new Observable");
            self.function("from_callback_executor", executor, "applies the wrapped function with its arguments and two appended callbacks");
            self.function("from_callback_success", success, "emits its value and completes");
            self.function("from_callback_failure", failure, "errors the subscriber");
        }
    }

    // -----------------------------------------------------------------------
    // Subscribe helpers and small utilities
    // -----------------------------------------------------------------------

    fn subscribe_normalizer(&mut self) {
        let index = self.index;
        for unit in self.units() {
            let facts = &index.units[unit];
            let ([call], [closure]) = (facts.calls.as_slice(), facts.nested.as_slice()) else {
                continue;
            };
            let Some((callee, args)) = index.plain(call) else {
                continue;
            };
            let runs = index.is_slot(callee, unit, 0)
                && matches!(args.as_slice(), [value] if index.is_slot(value, unit, 1));
            let closes = index.unsubscribes(*closure, call.span);
            let returns_closure = facts.results().iter().any(|result| index.fn_val(result) == Some(*closure));
            let returns_value =
                facts.results().iter().any(|result| index.val(result) == Val::Expr(call.span));
            if runs && closes && returns_closure && returns_value && facts.throws == 1 {
                self.function("subscribe_run_executor", unit, "runs slot 0 on slot 1 and normalizes its result to a teardown function or undefined");
                self.function("subscribe_unsubscribe_closure", *closure, "teardown calling unsubscribe on the executor's returned subscription");
            }
        }
    }

    fn signal_task_invoke(&mut self) {
        let index = self.index;
        for unit in self.units() {
            let facts = &index.units[unit];
            if facts.tries != 1 || facts.calls.len() != 3 {
                continue;
            }
            let handle = Val::Slot(unit, 1);
            let runs = facts.calls.iter().any(|call| {
                index.plain(call).is_some_and(|(callee, args)| {
                    index.is_slot(callee, unit, 0)
                        && matches!(args.as_slice(), [value] if index.val(value) == handle)
                })
            });
            if runs
                && index.calls_sink(unit, handle, "isComplete") == 1
                && index.calls_sink(unit, handle, "error") == 1
            {
                self.function("signal_task_guarded_invoke", unit, "calls slot 0 on the slot-1 handle unless it is complete and turns a throw into handle.error");
            }
        }
    }

    /// True for `f => { let done = false; return v => { if (!done) { done = true; f(v); } }; }`.
    fn is_once_factory(&self, factory: usize) -> bool {
        let index = self.index;
        let Some(wrapper) = index.returned_fn(factory) else {
            return false;
        };
        let facts = &index.units[wrapper];
        let ([statement], [assignment], [call]) =
            (facts.ifs.as_slice(), facts.assignments.as_slice(), facts.calls.as_slice())
        else {
            return false;
        };
        let Expression::UnaryExpression(test) = strip(&statement.test) else {
            return false;
        };
        let Some(latch) = analysis::expression_symbol(strip(&test.argument), index.ctx.scoping) else {
            return false;
        };
        test.operator == UnaryOperator::LogicalNot
            && index.declared_in(latch, factory)
            && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(identifier)
                if analysis::identifier_symbol(identifier, index.ctx.scoping) == Some(latch))
            && matches!(strip(&assignment.right), Expression::BooleanLiteral(value) if value.value)
            && index.plain(call).is_some_and(|(callee, args)| {
                index.is_slot(callee, factory, 0)
                    && matches!(args.as_slice(), [value] if index.is_slot(value, wrapper, 0))
            })
    }

    /// The once factory of the scheduled queue: `timer(() => runner(task,
    /// once(process)), 0)` where runner and timer are both plain slots of the
    /// queue. The sequential queue has the same once body and thunk, but its
    /// timer defaults to a host value, so it is not one plain slot.
    fn once_continuation(&mut self) {
        let index = self.index;
        let mut factories = BTreeSet::new();
        for call in &index.all_calls {
            let Some((callee, args)) = index.plain(call) else {
                continue;
            };
            let [_, continuation] = args.as_slice() else {
                continue;
            };
            let Expression::CallExpression(once) = strip(continuation) else {
                continue;
            };
            let Some((once_callee, once_args)) = index.plain(once) else {
                continue;
            };
            let Val::Slot(queue, 0) = index.val(callee) else {
                continue;
            };
            let Some(factory) = index.fn_val(once_callee) else {
                continue;
            };
            let Some(thunk) = index.owner_of(call.node_id.get()) else {
                continue;
            };
            let scheduled = index.all_calls.iter().any(|timer| {
                index.plain(timer).is_some_and(|(timer_callee, timer_args)| {
                    index.is_slot(timer_callee, queue, 1)
                        && timer_args.first().is_some_and(|value| index.fn_val(value) == Some(thunk))
                })
            });
            if scheduled && once_args.len() == 1 && self.is_once_factory(factory) {
                factories.insert(factory);
            }
        }
        for factory in factories {
            self.function("scheduled_queue_once_factory", factory, "once wrapper factory whose result the scheduled queue passes as the continuation of its slot-0 runner");
        }
    }

    fn empty_functions(&mut self) {
        let index = self.index;
        let main = self.main.expect("checked by find");
        for &unit in &index.units[main].nested {
            let facts = &index.units[unit];
            let FnNode::Function(function) = facts.func else {
                continue;
            };
            if !facts.is_empty_body() || facts.formals() != 0 {
                continue;
            }
            let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
                continue;
            };
            let references: Vec<_> = index.ctx.scoping.get_resolved_references(symbol).collect();
            match references.as_slice() {
                [] => self.function("unreferenced_noop", unit, "empty MAIN function nothing references"),
                [reference] => {
                    let parent = index.ctx.nodes.parent_id(reference.node_id());
                    let holder = match index.ctx.nodes.kind(parent) {
                        AstKind::AssignmentExpression(assignment) => match &assignment.left {
                            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                                analysis::identifier_symbol(identifier, index.ctx.scoping)
                            }
                            _ => None,
                        },
                        AstKind::VariableDeclarator(declarator) => match &declarator.id {
                            BindingPattern::BindingIdentifier(binding) => binding.symbol_id.get(),
                            _ => None,
                        },
                        _ => None,
                    };
                    let unread = holder.is_some_and(|holder| {
                        index.ctx.scoping.get_resolved_references(holder).all(|read| !read.is_read())
                    });
                    if unread {
                        self.function("held_noop", unit, "empty MAIN function stored once in a binding nothing reads");
                    }
                }
                _ => {}
            }
        }
    }

    fn ready_queue(&mut self) {
        let index = self.index;
        let scoping = index.ctx.scoping;
        for unit in self.units() {
            let facts = &index.units[unit];
            if let Some((flag, queue)) = self.split_ready_queue_guard(unit) {
                let registered = index.all_calls.iter().any(|call| {
                    index.method(call).is_some_and(|method| {
                        method.name == "addEventListener"
                            && matches!(method.args.as_slice(), [event, listener]
                                if matches!(strip(event), Expression::StringLiteral(event) if event.value == "DOMContentLoaded")
                                    && index.fn_val(listener) == Some(unit))
                    })
                });
                if registered {
                    self.function("ready_queue_flush", unit, "DOMContentLoaded listener running every queued callback once");
                    self.binding("ready_queue_pending", queue, "queue the ready listener runs and then clears to null");
                    self.binding("ready_queue_flushed", flag, "gate the ready listener tests and sets before running the queue");
                    continue;
                }
            }
            let [statement] = facts.ifs.as_slice() else {
                continue;
            };
            let Expression::LogicalExpression(test) = strip(&statement.test) else {
                continue;
            };
            let Expression::UnaryExpression(negated) = strip(&test.left) else {
                continue;
            };
            let (Some(flag), Some(queue)) = (
                analysis::expression_symbol(strip(&negated.argument), scoping),
                analysis::expression_symbol(strip(&test.right), scoping),
            ) else {
                continue;
            };
            let target = |assignment: &&&AssignmentExpression<'a>, symbol: SymbolId| {
                matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(identifier)
                    if analysis::identifier_symbol(identifier, scoping) == Some(symbol))
            };
            let sets_flag = facts.assignments.iter().any(|assignment| {
                target(&assignment, flag) && matches!(strip(&assignment.right), Expression::BooleanLiteral(value) if value.value)
            });
            let clears_queue = facts.assignments.iter().any(|assignment| {
                target(&assignment, queue) && matches!(strip(&assignment.right), Expression::NullLiteral(_))
            });
            let runs_entries = facts.calls.iter().any(|call| {
                matches!(strip(&call.callee), Expression::ComputedMemberExpression(member)
                    if index.reads_symbol(&member.object, queue))
            });
            let registered = index.all_calls.iter().any(|call| {
                index.method(call).is_some_and(|method| {
                    method.name == "addEventListener"
                        && matches!(method.args.as_slice(), [event, listener]
                            if matches!(strip(event), Expression::StringLiteral(event) if event.value == "DOMContentLoaded")
                                && index.fn_val(listener) == Some(unit))
                })
            });
            if test.operator == LogicalOperator::And
                && negated.operator == UnaryOperator::LogicalNot
                && facts.loops == 1
                && sets_flag
                && clears_queue
                && runs_entries
                && registered
            {
                self.function("ready_queue_flush", unit, "DOMContentLoaded listener running every queued callback once");
                self.binding("ready_queue_pending", queue, "queue the ready listener runs and then clears to null");
                self.binding("ready_queue_flushed", flag, "gate the ready listener tests and sets before running the queue");
            }
        }
    }

    /// The split guard emitted by the PGE ready listener:
    /// `let guard; if (flag) guard = false; else guard = queue; if (guard) ...`.
    /// Keep this lexical normalization deliberately exact so it cannot turn a
    /// foreign list/flag protocol into the existing ready-queue role.
    fn split_ready_queue_guard(&self, unit: usize) -> Option<(SymbolId, SymbolId)> {
        let index = self.index;
        let scoping = index.ctx.scoping;
        let facts = &index.units[unit];
        if facts.formals() != 0
            || facts.nested.len() != 0
            || facts.calls.len() != 1
            || facts.assignments.len() != 4
            || facts.updates.len() != 1
            || facts.ifs.len() != 2
            || facts.loops != 1
            || !facts.returns.is_empty()
            || !facts.news.is_empty()
            || facts.tries != 0
            || facts.throws != 0
        {
            return None;
        }
        let [Statement::VariableDeclaration(guard_decl), Statement::IfStatement(guard_if), Statement::IfStatement(drain_if)] =
            facts.statements()
        else {
            return None;
        };
        let [guard_binding] = guard_decl.declarations.as_slice() else {
            return None;
        };
        if guard_decl.kind != VariableDeclarationKind::Let || guard_binding.init.is_some() {
            return None;
        }
        let BindingPattern::BindingIdentifier(guard_identifier) = &guard_binding.id else {
            return None;
        };
        let guard = guard_identifier.symbol_id.get()?;
        if !index.declared_in(guard, unit) {
            return None;
        }
        let flag = analysis::expression_symbol(strip(&guard_if.test), scoping)?;
        let guard_block = match &guard_if.consequent {
            Statement::BlockStatement(block) => block,
            _ => return None,
        };
        let else_block = match guard_if.alternate.as_ref()? {
            Statement::BlockStatement(block) => block,
            _ => return None,
        };
        let [set_false] = guard_block.body.as_slice() else {
            return None;
        };
        let [set_queue] = else_block.body.as_slice() else {
            return None;
        };
        let set_false = assignment_statement(set_false)?;
        let set_queue = assignment_statement(set_queue)?;
        if assigned_symbol(set_false, scoping) != Some(guard)
            || !matches!(strip(&set_false.right), Expression::BooleanLiteral(value) if !value.value)
            || assigned_symbol(set_queue, scoping) != Some(guard)
        {
            return None;
        }
        let queue = analysis::expression_symbol(strip(&set_queue.right), scoping)?;
        let has_flag_setup = scoping.get_resolved_references(flag).any(|reference| {
            if !reference.is_write() || index.owner_of(reference.node_id()) == Some(unit) {
                return false;
            }
            let parent = index.ctx.nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(assignment) = index.ctx.nodes.kind(parent) else {
                return false;
            };
            assigned_symbol(assignment, scoping) == Some(flag)
                && matches!(strip(&assignment.right), Expression::BooleanLiteral(value) if !value.value)
        });
        let has_queue_setup = scoping.get_resolved_references(queue).any(|reference| {
            if !reference.is_write() || index.owner_of(reference.node_id()) == Some(unit) {
                return false;
            }
            let parent = index.ctx.nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(assignment) = index.ctx.nodes.kind(parent) else {
                return false;
            };
            assigned_symbol(assignment, scoping) == Some(queue)
                && matches!(strip(&assignment.right), Expression::ArrayExpression(array) if array.elements.is_empty())
        });
        if flag == guard
            || queue == guard
            || flag == queue
            || index.declared_in(flag, unit)
            || index.declared_in(queue, unit)
            || !scoping.symbol_redeclarations(flag).is_empty()
            || !scoping.symbol_redeclarations(queue).is_empty()
            || !has_flag_setup
            || !has_queue_setup
        {
            return None;
        }
        if analysis::expression_symbol(strip(&drain_if.test), scoping) != Some(guard)
            || drain_if.alternate.is_some()
        {
            return None;
        }
        let Statement::BlockStatement(drain_block) = &drain_if.consequent else {
            return None;
        };
        let [set_true, index_decl, Statement::ForStatement(loop_), clear_queue] =
            drain_block.body.as_slice()
        else {
            return None;
        };
        let set_true = assignment_statement(set_true)?;
        if assigned_symbol(set_true, scoping) != Some(flag)
            || !matches!(strip(&set_true.right), Expression::BooleanLiteral(value) if value.value)
            || assigned_symbol(assignment_statement(clear_queue)?, scoping) != Some(queue)
            || !matches!(strip(&assignment_statement(clear_queue)?.right), Expression::NullLiteral(_))
        {
            return None;
        }
        let index_declarator = one_declarator(index_decl)?;
        let BindingPattern::BindingIdentifier(index_identifier) = &index_declarator.id else {
            return None;
        };
        let cursor = index_identifier.symbol_id.get()?;
        if !index.declared_in(cursor, unit)
            || !matches!(index_declarator.init.as_ref().map(strip), Some(Expression::NumericLiteral(value)) if value.value == 0.0)
            || loop_.init.is_some()
            || loop_.test.is_none()
            || loop_.update.is_none()
        {
            return None;
        }
        let Some(Expression::BinaryExpression(test)) = loop_.test.as_ref().map(strip) else {
            return None;
        };
        let Expression::StaticMemberExpression(length) = strip(&test.right) else {
            return None;
        };
        if length.optional
            || length.property.name != "length"
            || test.operator != BinaryOperator::LessThan
            || !index.reads_symbol(&test.left, cursor)
            || !index.reads_symbol(&length.object, queue)
        {
            return None;
        }
        let Some(Expression::UpdateExpression(update)) = loop_.update.as_ref().map(strip) else {
            return None;
        };
        if update.operator != UpdateOperator::Increment
            || update.prefix
            || !matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier)
                if analysis::identifier_symbol(identifier, scoping) == Some(cursor))
        {
            return None;
        }
        let Statement::BlockStatement(loop_body) = &loop_.body else {
            return None;
        };
        let [callback] = loop_body.body.as_slice() else {
            return None;
        };
        let Statement::ExpressionStatement(callback) = callback else {
            return None;
        };
        let Expression::CallExpression(callback) = strip(&callback.expression) else {
            return None;
        };
        let Expression::ComputedMemberExpression(member) = strip(&callback.callee) else {
            return None;
        };
        if member.optional
            || !callback.arguments.is_empty()
            || !index.reads_symbol(&member.object, queue)
            || !index.reads_symbol(&member.expression, cursor)
        {
            return None;
        }
        let clear_queue = assignment_statement(clear_queue)?;
        if assigned_symbol(clear_queue, scoping) != Some(queue)
            || !matches!(strip(&clear_queue.right), Expression::NullLiteral(_))
        {
            return None;
        }
        Some((flag, queue))
    }

    fn object_assign(&mut self) {
        let index = self.index;
        for unit in self.units() {
            let facts = &index.units[unit];
            let delegates = index.methods(unit, "apply").iter().any(|method| {
                matches!(expression_member(method.receiver), Some((object, "assign")) if is_unbound(index, object, "Object"))
                    && matches!(method.args.as_slice(), [receiver, arguments]
                        if is_unbound(index, receiver, "Object") && is_unbound(index, arguments, "arguments"))
            });
            let own_check = index.methods(unit, "call").iter().any(|method| {
                matches!(expression_member(method.receiver), Some((object, "hasOwnProperty"))
                    if matches!(strip(object), Expression::ObjectExpression(_)))
            });
            let copies = facts.assignments.iter().any(|assignment| {
                assignment.left.as_member_expression().is_some_and(|member| {
                    matches!(member, MemberExpression::ComputedMemberExpression(_))
                }) && matches!(strip(&assignment.right), Expression::ComputedMemberExpression(_))
            });
            let boxed_error = facts.news.iter().any(|new| is_unbound(index, &new.callee, "TypeError"));
            if delegates && own_check && copies && boxed_error && facts.loops >= 2 {
                self.function("object_assign_helper", unit, "delegates to Object.assign when it exists, otherwise copies own enumerable keys into the boxed target");
            }
        }
    }

    fn has_own_property(&mut self) {
        let index = self.index;
        for unit in self.units() {
            let facts = &index.units[unit];
            let ([call], [result]) = (facts.calls.as_slice(), &facts.results()[..]) else {
                continue;
            };
            let Expression::CallExpression(returned) = strip(result) else {
                continue;
            };
            if returned.span != call.span {
                continue;
            }
            let Some(method) = index.method(call) else {
                continue;
            };
            let Some(holder) = analysis::expression_symbol(strip(method.receiver), index.ctx.scoping) else {
                continue;
            };
            let Some((object, "hasOwnProperty")) = index.origin(holder).and_then(expression_member) else {
                continue;
            };
            if method.name == "call"
                && matches!(strip(object), Expression::ObjectExpression(_))
                && matches!(method.args.as_slice(), [target, key]
                    if index.is_slot(target, unit, 0) && index.is_slot(key, unit, 1))
            {
                self.function("has_own_property_helper", unit, "calls the held hasOwnProperty on slot 0 with key slot 1");
                self.binding("has_own_property_method", holder, "holds {}.hasOwnProperty and is called with an explicit receiver");
            }
        }
    }

    fn singleton(&mut self) {
        let index = self.index;
        let scoping = index.ctx.scoping;
        for factory in self.units() {
            let Some(getter) = index.returned_fn(factory) else {
                continue;
            };
            let facts = &index.units[getter];
            let ([result], [statement]) = (&facts.results()[..], facts.ifs.as_slice()) else {
                continue;
            };
            let Some(cache) = analysis::expression_symbol(strip(result), scoping) else {
                continue;
            };
            let constructs = facts.assignments.iter().any(|assignment| {
                matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(identifier)
                    if analysis::identifier_symbol(identifier, scoping) == Some(cache))
                    && matches!(strip(&assignment.right), Expression::NewExpression(new)
                        if new.arguments.is_empty() && index.is_slot(&new.callee, getter, 0))
            });
            let tests_null = matches!(strip(&statement.test), Expression::BinaryExpression(test)
                if test.operator == BinaryOperator::Equality
                    && index.reads_symbol(&test.left, cache)
                    && matches!(strip(&test.right), Expression::NullLiteral(_)));
            if constructs && tests_null && facts.tries == 1 && index.declared_in(cache, factory) {
                self.function("singleton_factory", factory, "owns a null cache and returns its constructing getter");
                self.function("singleton_getter", getter, "constructs slot 0 once into the cache, swallows a throw and returns the cache");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Callback registry
    // -----------------------------------------------------------------------

    /// The `(holder, method-holder)` of `invoke(method, this.list, ...)`.
    fn registry_call(&self, call: &'a CallExpression<'a>) -> Option<(SymbolId, SymbolId, Vec<&'a Expression<'a>>)> {
        let index = self.index;
        let (callee, args) = index.plain(call)?;
        let invoke = analysis::expression_symbol(strip(callee), index.ctx.scoping)?;
        let (method, list) = (args.first()?, args.get(1)?);
        let (object, _) = expression_member(list)?;
        if !matches!(strip(object), Expression::ThisExpression(_)) {
            return None;
        }
        let holder = analysis::expression_symbol(strip(method), index.ctx.scoping)?;
        Some((invoke, holder, args))
    }

    fn array_method_holder(&self, symbol: SymbolId, name: &str) -> bool {
        let index = self.index;
        let Some((object, method)) = index.origin(symbol).and_then(expression_member) else {
            return false;
        };
        method == name
            && matches!(expression_member(object), Some((root, "prototype")) if is_unbound(index, root, "Array"))
    }

    /// `Object.call.bind(Object.bind, Object.call)(Object.call)`: the uncurried
    /// `Function.prototype.call`.
    fn is_uncurried_call(&self, symbol: SymbolId) -> bool {
        let index = self.index;
        let Some(Expression::CallExpression(outer)) = index.origin(symbol).map(strip) else {
            return false;
        };
        let Expression::CallExpression(inner) = strip(&outer.callee) else {
            return false;
        };
        let Some(bind) = index.method(inner) else {
            return false;
        };
        bind.name == "bind"
            && bind.args.len() == 2
            && matches!(expression_member(bind.receiver), Some((_, "call")))
    }

    fn registry(&mut self) {
        let index = self.index;
        for factory in self.units() {
            let [result] = index.units[factory].results()[..] else {
                continue;
            };
            let members = index.object_members(result);
            let method = |name: &str| members.get(name).and_then(|value| index.fn_val(value));
            let (Some(notify), Some(register), Some(unregister), Some(merge)) =
                (method("notify"), method("register"), method("unregister"), method("__merge"))
            else {
                continue;
            };
            let only_call = |unit: usize| match index.units[unit].calls.as_slice() {
                [call] => self.registry_call(call),
                _ => None,
            };
            let notify_call = index.units[notify].calls.iter().find_map(|call| self.registry_call(call));
            let (Some((invoke, slice, _)), Some((_, push, _)), Some((_, find, find_args)), Some((_, concat, concat_args))) =
                (notify_call, only_call(register), only_call(unregister), only_call(merge))
            else {
                continue;
            };
            let clears = index.units[unregister].assignments.iter().any(|assignment| {
                matches!(strip(&assignment.right), Expression::NullLiteral(_))
                    && assignment.left.as_member_expression().is_some_and(|member| {
                        matches!(member, MemberExpression::ComputedMemberExpression(_))
                    })
            });
            let finds_slot = matches!(find_args.as_slice(), [_, _, value] if index.is_slot(value, unregister, 0));
            let merges_other = matches!(concat_args.as_slice(), [_, _, value]
                if matches!(expression_member(value), Some((object, _)) if index.is_slot(object, merge, 0)));
            if clears && finds_slot {
                self.function("callback_registry_unregister", unregister, "finds its slot 0 in the registry list and nulls that entry");
            }
            if merges_other && index.units[merge].assignments.len() == 1 {
                self.function("callback_registry_merge", merge, "replaces the registry list with its concatenation with slot 0's list");
            }
            if self.is_uncurried_call(invoke) {
                self.binding("registry_invoke_method", invoke, "uncurried Function.prototype.call, called by every registry method");
            }
            for (role, symbol, name, evidence) in [
                ("registry_find_index_method", find, "indexOf", "Array.prototype.indexOf held for unregister"),
                ("registry_append_method", push, "push", "Array.prototype.push held for register"),
                ("registry_concat_method", concat, "concat", "Array.prototype.concat held for __merge"),
                ("registry_slice_method", slice, "slice", "Array.prototype.slice held for notify"),
            ] {
                if self.array_method_holder(symbol, name) {
                    self.binding(role, symbol, evidence);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Time-budgeted task queue
    // -----------------------------------------------------------------------

    /// The nested `listener => setTimeout(listener, 0)` of a unit's `forEach`.
    fn schedule_callback(&self, unit: usize) -> Option<usize> {
        let index = self.index;
        index.methods(unit, "forEach").iter().find_map(|method| {
            let callback = index.fn_val(method.args.first()?)?;
            let [call] = index.units[callback].calls.as_slice() else {
                return None;
            };
            index.is_slot(index.timer_argument(call)?, callback, 0).then_some(callback)
        })
    }

    /// The nested `(listener) => setTimeout(() => listener(error), 0)` of a
    /// unit's `forEach`, with the timer body.
    fn deferred_error_callback(&self, unit: usize) -> Option<(usize, usize)> {
        let index = self.index;
        index.methods(unit, "forEach").iter().find_map(|method| {
            let callback = index.fn_val(method.args.first()?)?;
            let [call] = index.units[callback].calls.as_slice() else {
                return None;
            };
            let body = index.fn_val(index.timer_argument(call)?)?;
            self.calls_slot_with_value(body, callback, 0).then_some((callback, body))
        })
    }

    /// `unit` only calls slot `slot` of `owner` with one value.
    fn calls_slot_with_value(&self, unit: usize, owner: usize, slot: u32) -> bool {
        let index = self.index;
        let [call] = index.units[unit].calls.as_slice() else {
            return false;
        };
        index.plain(call).is_some_and(|(callee, args)| {
            args.len() == 1 && index.is_slot(callee, owner, slot)
        })
    }

    fn task_queue(&mut self) {
        let index = self.index;
        for factory in self.units() {
            let [result] = index.units[factory].results()[..] else {
                continue;
            };
            let members = index.object_members(result);
            let method = |name: &str| members.get(name).and_then(|value| index.fn_val(value));
            let (Some(start), Some(_), Some(empty), Some(_), Some(on_error)) = (
                method("start"),
                method("enqueue"),
                method("empty"),
                method("onEmpty"),
                method("onError"),
            ) else {
                continue;
            };
            let drain = index.units[start].calls.iter().find_map(|call| {
                let unit = index.fn_val(index.timer_argument(call)?)?;
                (index.units[unit].parent == Some(factory)).then_some(unit)
            });
            let Some(drain) = drain else {
                continue;
            };
            if inline_task_queue::prove(index, factory, drain) {
                self.function("task_queue_drain", drain, "owns the exact queue shift/worker/clock loop, rolling budget guard, clear-before-deferred-error path and self/empty notification tails; captures have scoped parent origins");
            }
            let step = index.units[drain].calls.iter().find_map(|call| {
                let (callee, args) = index.plain(call)?;
                let unit = index.fn_val(callee)?;
                (args.is_empty() && index.units[unit].parent == Some(drain)).then_some(unit)
            });
            let Some(step) = step else {
                continue;
            };
            let runs_worker = index.units[step].calls.iter().any(|call| {
                index.plain(call).is_some_and(|(callee, args)| {
                    args.len() == 1 && index.is_slot(callee, factory, 0)
                })
            });
            if index.units[drain].loops != 1
                || index.units[step].tries != 1
                || index.methods(step, "shift").len() != 1
                || !runs_worker
            {
                continue;
            }
            self.function("task_queue_drain", drain, "runs the step while the rolling average cost fits the slot-1 budget and reschedules itself");
            self.function("task_queue_step", step, "shifts one task and runs slot 0 on it, timing it; returns 0 on empty or a thrown task");
            if let Some(callback) = self.schedule_callback(step) {
                self.function("task_queue_step_schedule_empty", callback, "schedules one empty listener when the step finds no task");
            }
            if let Some((callback, body)) = self.deferred_error_callback(step) {
                self.function("task_queue_schedule_error", callback, "schedules one error listener with the caught error");
                self.function("task_queue_deliver_error", body, "timer body calling the error listener with the caught error");
            }
            if let Some(callback) = self.schedule_callback(drain) {
                self.function("task_queue_drain_schedule_empty", callback, "schedules one empty listener from the drain tail");
            }
            if let Some(callback) = self.schedule_callback(empty) {
                self.function("task_queue_discard_schedule_empty", callback, "schedules one empty listener when pending tasks are discarded");
            }
            let late = index.units[on_error].calls.iter().find_map(|call| {
                let body = index.fn_val(index.timer_argument(call)?)?;
                self.calls_slot_with_value(body, on_error, 0).then_some(body)
            });
            if let Some(body) = late {
                self.function("task_queue_deliver_late_error", body, "timer body calling a late-registered listener with the stored error");
            }
            self.default_clock(factory);
        }
    }

    /// The MAIN alias the queue calls with `window` for its default clock, when
    /// the function it holds reads `performance.now`.
    fn default_clock(&mut self, factory: usize) {
        let index = self.index;
        for call in &index.units[factory].calls {
            let Some((callee, args)) = index.plain(call) else {
                continue;
            };
            if !matches!(args.as_slice(), [value] if is_unbound(index, value, "window")) {
                continue;
            }
            let Some(holder) = analysis::expression_symbol(strip(callee), index.ctx.scoping) else {
                continue;
            };
            let Some(clock) = index.fn_val(callee) else {
                continue;
            };
            let reads_clock = index.methods(clock, "bind").iter().any(|method| {
                matches!(expression_member(method.receiver), Some((_, "now")))
            });
            if reads_clock && !index.decl_fn.contains_key(&holder) {
                self.binding("task_queue_default_clock_factory", holder, "alias the queue calls with window for its default clock; the held function binds performance.now");
            }
        }
    }
}

/// An independent, deliberately bounded proof for a drain that owns its step.
/// Every expression below is rendered from AST operators and scoped symbols;
/// source spellings, previous labels and nested/sibling step bodies are unused.
mod inline_task_queue {
    use super::*;

    struct Proof<'i, 'c, 'a> {
        index: &'i Index<'c, 'a>,
        factory: usize,
        drain: usize,
        roles: BTreeMap<SymbolId, &'static str>,
    }

    impl<'i, 'c, 'a> Proof<'i, 'c, 'a> {
        fn bind(&mut self, e: &Expression<'_>, role: &'static str) -> Option<SymbolId> {
            let s = analysis::expression_symbol(strip(e), self.index.ctx.scoping)?;
            self.bind_symbol(s, role)
        }

        fn bind_symbol(&mut self, s: SymbolId, role: &'static str) -> Option<SymbolId> {
            if !self.index.ctx.scoping.symbol_redeclarations(s).is_empty()
                || self
                    .roles
                    .iter()
                    .any(|(other, r)| *r == role && *other != s)
                || self.roles.get(&s).is_some_and(|r| *r != role)
            {
                return None;
            }
            self.roles.insert(s, role);
            Some(s)
        }

        // A tiny expression language: unknown expressions fail closed. This is
        // an exact tree comparison, including all operators and call arguments.
        fn expr(&self, e: &Expression<'_>) -> Option<String> {
            Some(match strip(e) {
                Expression::Identifier(id) => {
                    if let Some(s) = analysis::identifier_symbol(id, self.index.ctx.scoping) {
                        self.roles.get(&s)?.to_string()
                    } else if matches!(id.name.as_str(), "setTimeout" | "Number") {
                        id.name.to_string()
                    } else {
                        return None;
                    }
                }
                Expression::NumericLiteral(n) if n.value == 0.0 => "0".into(),
                Expression::NumericLiteral(n) if n.value == 1.0 => "1".into(),
                Expression::BooleanLiteral(b) => b.value.to_string(),
                Expression::StaticMemberExpression(m) if !m.optional => {
                    format!("{}.{}", self.expr(&m.object)?, m.property.name)
                }
                Expression::CallExpression(c) if !c.optional => {
                    let args = c
                        .arguments
                        .iter()
                        .map(|a| self.expr(a.as_expression()?))
                        .collect::<Option<Vec<_>>>()?;
                    format!("{}({})", self.expr(&c.callee)?, args.join(","))
                }
                Expression::BinaryExpression(b) => format!(
                    "({}{}{})",
                    self.expr(&b.left)?,
                    b.operator.as_str(),
                    self.expr(&b.right)?
                ),
                Expression::UnaryExpression(u) if u.operator == UnaryOperator::Void => {
                    format!("void {}", self.expr(&u.argument)?)
                }
                Expression::UpdateExpression(u) if u.prefix => {
                    let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
                        return None;
                    };
                    let s = analysis::identifier_symbol(id, self.index.ctx.scoping)?;
                    format!("{}{}", u.operator.as_str(), self.roles.get(&s)?)
                }
                Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
                    let left = match &a.left {
                        AssignmentTarget::AssignmentTargetIdentifier(id) => {
                            let s = analysis::identifier_symbol(id, self.index.ctx.scoping)?;
                            self.roles.get(&s)?.to_string()
                        }
                        AssignmentTarget::StaticMemberExpression(m) if !m.optional => {
                            format!("{}.{}", self.expr(&m.object)?, m.property.name)
                        }
                        _ => return None,
                    };
                    format!("{left}={}", self.expr(&a.right)?)
                }
                _ => return None,
            })
        }

        fn is(&self, e: &Expression<'_>, expected: &str) -> bool {
            self.expr(e).as_deref() == Some(expected)
        }

        fn statement(&self, s: &Statement<'_>, expected: &str) -> Option<()> {
            let Statement::ExpressionStatement(s) = s else {
                return None;
            };
            self.is(&s.expression, expected).then_some(())
        }

        fn declaration(
            &mut self,
            s: &'a Statement<'a>,
            role: &'static str,
            init: Option<&str>,
        ) -> Option<()> {
            let Statement::VariableDeclaration(d) = s else {
                return None;
            };
            if !matches!(
                d.kind,
                VariableDeclarationKind::Let
                    | VariableDeclarationKind::Const
                    | VariableDeclarationKind::Var
            ) {
                return None;
            }
            let [v] = d.declarations.as_slice() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(id) = &v.id else {
                return None;
            };
            self.bind_symbol(id.symbol_id.get()?, role)?;
            match (v.init.as_ref(), init) {
                (None, None) => Some(()),
                (Some(e), Some(expected)) if self.is(e, expected) => Some(()),
                _ => None,
            }
        }

        fn block(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
            let Statement::BlockStatement(b) = s else {
                return None;
            };
            Some(&b.body)
        }

        fn done(s: &Statement<'_>) -> Option<()> {
            matches!(s, Statement::ReturnStatement(r) if r.argument.as_ref().is_some_and(is_void0))
                .then_some(())
        }

        fn simple_function(f: &Function<'_>, arity: usize) -> bool {
            !f.r#async
                && !f.generator
                && f.params.rest.is_none()
                && f.params.items.len() == arity
                && f.params.items.iter().all(|p| {
                    p.initializer.is_none()
                        && matches!(p.pattern, BindingPattern::BindingIdentifier(_))
                })
                && f.body.as_ref().is_some_and(|b| b.directives.is_empty())
        }

        // Local function declarations must be used by this exact forEach, and
        // their callback and timer arguments must be the exact scoped values.
        fn notify(
            &mut self,
            declaration: &'a Statement<'a>,
            each: &'a Statement<'a>,
            error: bool,
        ) -> Option<SymbolId> {
            let Statement::FunctionDeclaration(f) = declaration else {
                return None;
            };
            if !Self::simple_function(f, 1) {
                return None;
            }
            let fn_symbol = f.id.as_ref()?.symbol_id.get()?;
            if !self
                .index
                .ctx
                .scoping
                .symbol_redeclarations(fn_symbol)
                .is_empty()
                || self
                    .index
                    .ctx
                    .scoping
                    .get_resolved_references(fn_symbol)
                    .any(|r| r.is_write())
            {
                return None;
            }
            let BindingPattern::BindingIdentifier(param) = &f.params.items[0].pattern else {
                return None;
            };
            let param = param.symbol_id.get()?;
            if self
                .index
                .ctx
                .scoping
                .get_resolved_references(param)
                .any(|r| r.is_write())
            {
                return None;
            }
            let Statement::ExpressionStatement(each) = each else {
                return None;
            };
            let Expression::CallExpression(c) = strip(&each.expression) else {
                return None;
            };
            let Expression::StaticMemberExpression(m) = strip(&c.callee) else {
                return None;
            };
            if c.optional || m.optional || m.property.name != "forEach" {
                return None;
            }
            let args = plain_arguments(c)?;
            let [arg] = args.as_slice() else {
                return None;
            };
            if analysis::expression_symbol(strip(arg), self.index.ctx.scoping) != Some(fn_symbol) {
                return None;
            }
            let list = self.bind(&m.object, if error { "errors" } else { "listeners" })?;
            let body = &f.body.as_ref()?.statements;
            if error {
                let [alias, timer] = body.as_slice() else {
                    return None;
                };
                let Statement::VariableDeclaration(d) = alias else {
                    return None;
                };
                if !matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                ) {
                    return None;
                }
                let [v] = d.declarations.as_slice() else {
                    return None;
                };
                let BindingPattern::BindingIdentifier(id) = &v.id else {
                    return None;
                };
                let alias = id.symbol_id.get()?;
                if !self
                    .index
                    .ctx
                    .scoping
                    .symbol_redeclarations(alias)
                    .is_empty()
                    || self
                        .index
                        .ctx
                        .scoping
                        .get_resolved_references(alias)
                        .any(|r| r.is_write())
                    || analysis::expression_symbol(strip(v.init.as_ref()?), self.index.ctx.scoping)
                        != Some(param)
                {
                    return None;
                }
                let Statement::ExpressionStatement(timer) = timer else {
                    return None;
                };
                let Expression::CallExpression(timer) = strip(&timer.expression) else {
                    return None;
                };
                let args = plain_arguments(timer)?;
                if timer.optional
                    || args.len() != 2
                    || !is_unbound(self.index, &timer.callee, "setTimeout")
                    || !matches!(strip(args[1]), Expression::NumericLiteral(n) if n.value == 0.0)
                {
                    return None;
                }
                let Expression::ArrowFunctionExpression(deliver) = strip(args[0]) else {
                    return None;
                };
                if deliver.r#async
                    || !deliver.expression
                    || deliver.params.rest.is_some()
                    || !deliver.params.items.is_empty()
                {
                    return None;
                }
                let [Statement::ExpressionStatement(invoke)] = deliver.body.statements.as_slice()
                else {
                    return None;
                };
                let Expression::CallExpression(invoke) = strip(&invoke.expression) else {
                    return None;
                };
                let args = plain_arguments(invoke)?;
                if invoke.optional
                    || args.len() != 1
                    || analysis::expression_symbol(strip(&invoke.callee), self.index.ctx.scoping)
                        != Some(alias)
                    || !self.is(args[0], "caught")
                {
                    return None;
                }
            } else {
                let [Statement::ExpressionStatement(timer)] = body.as_slice() else {
                    return None;
                };
                let Expression::CallExpression(timer) = strip(&timer.expression) else {
                    return None;
                };
                let args = plain_arguments(timer)?;
                if timer.optional
                    || args.len() != 2
                    || !is_unbound(self.index, &timer.callee, "setTimeout")
                    || analysis::expression_symbol(strip(args[0]), self.index.ctx.scoping)
                        != Some(param)
                    || !matches!(strip(args[1]), Expression::NumericLiteral(n) if n.value == 0.0)
                {
                    return None;
                }
            }
            Some(list)
        }

        fn tail(&mut self, s: &'a Statement<'a>) -> Option<()> {
            let Statement::IfStatement(branch) = s else {
                return None;
            };
            if !self.is(&branch.test, "(queue.length>0)") {
                return None;
            }
            let [schedule] = Self::block(&branch.consequent)? else {
                return None;
            };
            let Statement::ExpressionStatement(expression) = schedule else {
                return None;
            };
            let Expression::CallExpression(c) = strip(&expression.expression) else {
                return None;
            };
            let args = plain_arguments(c)?;
            if c.optional || args.len() != 2 {
                return None;
            }
            let self_symbol = self.bind(args[0], "self")?;
            self.stable_callable(self_symbol, self.drain, 0)?;
            self.statement(schedule, "setTimeout(self,0)")?;
            let Statement::IfStatement(empty) = branch.alternate.as_ref()? else {
                return None;
            };
            if !self.is(&empty.test, "running") || empty.alternate.is_some() {
                return None;
            }
            let [stop, callback, each] = Self::block(&empty.consequent)? else {
                return None;
            };
            self.statement(stop, "running=false")?;
            self.notify(callback, each, false)?;
            Some(())
        }

        // Every hop must be a single origin, a declaration in this closure,
        // and established before the drain can escape through the API.
        fn stable_callable(&self, mut s: SymbolId, target: usize, mut depth: usize) -> Option<()> {
            let mut later = u32::MAX;
            loop {
                if depth >= 8 || !self.index.ctx.scoping.symbol_redeclarations(s).is_empty() {
                    return None;
                }
                if self.index.decl_fn.get(&s) == Some(&target) {
                    let FnNode::Function(f) = self.index.units[target].func else {
                        return None;
                    };
                    if !matches!(
                        self.index.ctx.nodes.parent_kind(f.node_id.get()),
                        AstKind::FunctionBody(_)
                    ) {
                        return None;
                    }
                    return (!self
                        .index
                        .ctx
                        .scoping
                        .get_resolved_references(s)
                        .any(|r| r.is_write()))
                    .then_some(());
                }
                let e = self.index.origin(s)?;
                if e.span().end >= later {
                    return None;
                }
                later = e.span().start;
                if !self.index.units[self.factory]
                    .span
                    .contains_inclusive(self.index.ctx.scoping.symbol_span(s))
                {
                    return None;
                }
                // Require the assignment/initializer itself to be unconditional
                // in the immediate factory body, before its return.
                if !self.direct_values(s).iter().any(|v| v.span() == e.span()) {
                    return None;
                }
                s = analysis::expression_symbol(strip(e), self.index.ctx.scoping)?;
                depth += 1;
            }
        }

        fn direct_values(&self, symbol: SymbolId) -> Vec<&'a Expression<'a>> {
            let mut values = Vec::new();
            for s in self.index.units[self.factory].statements() {
                if matches!(s, Statement::ReturnStatement(_)) {
                    break;
                }
                match s {
                    Statement::VariableDeclaration(d) => {
                        for v in &d.declarations {
                            if matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(symbol))
                            {
                                values.extend(v.init.as_ref());
                            }
                        }
                    }
                    _ => {
                        if let Some(a) = assignment_statement(s) {
                            if matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id) if analysis::identifier_symbol(id, self.index.ctx.scoping) == Some(symbol))
                            {
                                values.push(&a.right);
                            }
                        }
                    }
                }
            }
            values
        }

        // Begin at the actual drain read. Each alias initializer must finish
        // before that read, then becomes the read bound for its source hop.
        // A unique later `var` initializer is not an established value.
        fn parameter_origin(&self, mut s: SymbolId, mut before: u32) -> Option<SymbolId> {
            for _ in 0..8 {
                let scoping = self.index.ctx.scoping;
                if !scoping.symbol_redeclarations(s).is_empty() {
                    return None;
                }
                let FnNode::Function(factory) = self.index.units[self.factory].func else {
                    return None;
                };
                if factory.params.items.iter().any(|p| matches!(&p.pattern, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))) {
                    if scoping.get_resolved_references(s).any(|r| r.is_write()) { return None; }
                    return Some(s);
                }
                let e = self.index.origin(s)?;
                if e.span().end >= before
                    || e.span().end >= self.index.units[self.drain].span.start
                    || !self.direct_values(s).iter().any(|v| v.span() == e.span())
                {
                    return None;
                }
                before = e.span().start;
                s = analysis::expression_symbol(strip(e), scoping)?;
            }
            None
        }

        fn role(&self, role: &str) -> Option<SymbolId> {
            self.roles
                .iter()
                .find_map(|(s, r)| (*r == role).then_some(*s))
        }

        fn origins(&self) -> Option<()> {
            let origin_at_read = |role| {
                let s = self.role(role)?;
                let read = self
                    .index
                    .ctx
                    .scoping
                    .get_resolved_references(s)
                    .filter(|r| r.is_read())
                    .map(|r| self.index.ctx.nodes.kind(r.node_id()).span())
                    .filter(|span| self.index.units[self.drain].span.contains_inclusive(*span))
                    .map(|span| span.start)
                    .min()?;
                self.parameter_origin(s, read)
            };
            let worker = origin_at_read("worker")?;
            let budget = origin_at_read("budget")?;
            if worker == budget {
                return None;
            }
            let FnNode::Function(factory) = self.index.units[self.factory].func else {
                return None;
            };
            if !Self::simple_function(factory, factory.params.items.len()) {
                return None;
            }
            let formals: BTreeSet<_> = factory
                .params
                .items
                .iter()
                .filter_map(|p| {
                    let BindingPattern::BindingIdentifier(id) = &p.pattern else {
                        return None;
                    };
                    id.symbol_id.get()
                })
                .collect();
            if !formals.contains(&worker) || !formals.contains(&budget) {
                return None;
            }
            // Queue/listener/error storage must be allocated directly in the
            // parent, never a borrowed property bag or another queue's list.
            for role in ["listeners", "errors"] {
                let s = self.role(role)?;
                let origin = self.index.origin(s)?;
                let values = self.direct_values(s);
                let [value] = values.as_slice() else {
                    return None;
                };
                if value.span() != origin.span()
                    || value.span().end >= self.index.units[self.drain].span.start
                    || !matches!(strip(value), Expression::ArrayExpression(a) if a.elements.is_empty())
                {
                    return None;
                }
            }
            let queue = self.role("queue")?;
            let origin = self.index.origin(queue)?;
            let values = self.direct_values(queue);
            let [value] = values.as_slice() else {
                return None;
            };
            if value.span() != origin.span()
                || value.span().end >= self.index.units[self.drain].span.start
            {
                return None;
            }
            let Expression::CallExpression(slice) = strip(value) else {
                return None;
            };
            let Expression::StaticMemberExpression(m) = strip(&slice.callee) else {
                return None;
            };
            if slice.optional
                || m.optional
                || m.property.name != "slice"
                || !slice.arguments.is_empty()
            {
                return None;
            }
            let initial = analysis::expression_symbol(strip(&m.object), self.index.ctx.scoping)?;
            let source = self.index.origin(initial)?;
            if !self
                .direct_values(initial)
                .iter()
                .any(|v| v.span() == source.span())
                || source.span().end >= value.span().start
            {
                return None;
            }
            let Expression::ConditionalExpression(default) = strip(source) else {
                return None;
            };
            let Expression::BinaryExpression(test) = strip(&default.test) else {
                return None;
            };
            if test.operator != BinaryOperator::StrictEquality
                || !is_void0(&test.right)
                || !matches!(strip(&default.consequent), Expression::ArrayExpression(a) if a.elements.is_empty())
            {
                return None;
            }
            let input = analysis::expression_symbol(strip(&test.left), self.index.ctx.scoping)?;
            if !formals.contains(&input)
                || input == worker
                || input == budget
                || analysis::expression_symbol(strip(&default.alternate), self.index.ctx.scoping)
                    != Some(input)
                || self
                    .index
                    .ctx
                    .scoping
                    .get_resolved_references(input)
                    .any(|r| r.is_write())
            {
                return None;
            }
            // The sampled clock is a parent parameter transported through a
            // local alias, with exactly one optional undefined-default branch.
            // This proves the sampled value's transport, not monotonicity or
            // the external default clock factory's implementation contract.
            let clock = self.role("clock")?;
            let values = self.direct_values(clock);
            let [value] = values.as_slice() else {
                return None;
            };
            if value.span().end >= self.index.units[self.drain].span.start {
                return None;
            }
            let clock_param = analysis::expression_symbol(strip(value), self.index.ctx.scoping)?;
            if !formals.contains(&clock_param)
                || [worker, budget, input].contains(&clock_param)
                || self
                    .index
                    .ctx
                    .scoping
                    .get_resolved_references(clock_param)
                    .any(|r| r.is_write())
            {
                return None;
            }
            let mut defaults = 0;
            for s in self.index.units[self.factory].statements() {
                let Statement::IfStatement(branch) = s else {
                    continue;
                };
                let Some([set]) = Self::block(&branch.consequent) else {
                    continue;
                };
                let Some(a) = assignment_statement(set) else {
                    continue;
                };
                if !matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id) if analysis::identifier_symbol(id, self.index.ctx.scoping) == Some(clock))
                {
                    continue;
                }
                let Expression::BinaryExpression(test) = strip(&branch.test) else {
                    return None;
                };
                if branch.alternate.is_some()
                    || test.operator != BinaryOperator::StrictEquality
                    || analysis::expression_symbol(strip(&test.left), self.index.ctx.scoping)
                        != Some(clock_param)
                    || !is_void0(&test.right)
                {
                    return None;
                }
                let Expression::CallExpression(c) = strip(&a.right) else {
                    return None;
                };
                let args = plain_arguments(c)?;
                if c.optional || args.len() != 1 || !is_unbound(self.index, args[0], "window") {
                    return None;
                }
                let target = self.index.fn_val(&c.callee)?;
                let origin = analysis::expression_symbol(strip(&c.callee), self.index.ctx.scoping)?;
                // The existing ordered factory transport may live outside
                // this queue; all alias hops still need single origins.
                let mut next = origin;
                let mut later = u32::MAX;
                for depth in 0..8 {
                    if !self
                        .index
                        .ctx
                        .scoping
                        .symbol_redeclarations(next)
                        .is_empty()
                    {
                        return None;
                    }
                    if self.index.decl_fn.get(&next) == Some(&target) {
                        let FnNode::Function(f) = self.index.units[target].func else {
                            return None;
                        };
                        if !matches!(
                            self.index.ctx.nodes.parent_kind(f.node_id.get()),
                            AstKind::FunctionBody(_)
                        ) {
                            return None;
                        }
                        if self
                            .index
                            .ctx
                            .scoping
                            .get_resolved_references(next)
                            .any(|r| r.is_write())
                        {
                            return None;
                        }
                        break;
                    }
                    let e = self.index.origin(next)?;
                    if e.span().end >= later || !self.unconditional_origin(e) {
                        return None;
                    }
                    later = e.span().start;
                    next = analysis::expression_symbol(strip(e), self.index.ctx.scoping)?;
                    if depth == 7 {
                        return None;
                    }
                }
                defaults += 1;
            }
            let writes = self
                .index
                .ctx
                .scoping
                .get_resolved_references(clock)
                .filter(|r| r.is_write())
                .count();
            if defaults != 1 || writes != 1 {
                return None;
            }
            self.capture_uses()?;
            Some(())
        }

        fn unconditional_origin(&self, e: &Expression<'_>) -> bool {
            let nodes = self.index.ctx.nodes;
            nodes.iter().any(|node| match node.kind() {
                AstKind::VariableDeclarator(v) if v.init.as_ref().is_some_and(|i| i.span() == e.span()) => {
                    matches!(nodes.parent_kind(node.id()), AstKind::VariableDeclaration(d)
                        if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const))
                        && matches!(nodes.parent_kind(nodes.parent_id(node.id())), AstKind::FunctionBody(_))
                }
                AstKind::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign && a.right.span() == e.span() => {
                    matches!(nodes.parent_kind(node.id()), AstKind::ExpressionStatement(_))
                        && matches!(nodes.parent_kind(nodes.parent_id(node.id())), AstKind::FunctionBody(_))
                }
                _ => false,
            })
        }

        fn capture_uses(&self) -> Option<()> {
            let nodes = self.index.ctx.nodes;
            let scoping = self.index.ctx.scoping;
            let factory = self.index.units[self.factory].span;
            for role in [
                "queue",
                "listeners",
                "errors",
                "clock",
                "worker",
                "budget",
                "running",
                "stored",
                "failed",
                "self",
            ] {
                let s = self.role(role)?;
                let declaration = scoping.symbol_declaration(s);
                if !matches!(nodes.kind(declaration), AstKind::VariableDeclarator(v)
                    if matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s)))
                    || !matches!(nodes.parent_kind(declaration), AstKind::VariableDeclaration(d)
                        if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const))
                    || self.index.owner_of(declaration) != Some(self.factory)
                    || scoping.symbol_span(s).end >= self.index.units[self.drain].span.start
                {
                    return None;
                }
                for reference in scoping.get_resolved_references(s) {
                    let node = reference.node_id();
                    if !factory.contains_inclusive(nodes.kind(node).span()) {
                        return None;
                    }
                    if reference.is_write() {
                        continue;
                    }
                    if matches!(role, "worker" | "clock") {
                        if !matches!(nodes.parent_kind(node), AstKind::CallExpression(c)
                            if !c.optional && analysis::expression_symbol(strip(&c.callee), scoping) == Some(s)
                                && c.arguments.len() == if role == "clock" { 0 } else { 1 }
                                && self.index.units[self.drain].span.contains_inclusive(c.span))
                        {
                            return None;
                        }
                    }
                    if role == "budget"
                        && !self.index.units[self.drain]
                            .span
                            .contains_inclusive(nodes.kind(node).span())
                    {
                        return None;
                    }
                    // Lists may never be aliased, returned, or passed to an
                    // arbitrary call. Only their actual queue operations are
                    // supported; enqueue's intrinsic push.apply is explicit.
                    if matches!(role, "queue" | "listeners" | "errors") {
                        match nodes.parent_kind(node) {
                            AstKind::StaticMemberExpression(m) if !m.optional => {
                                let name = m.property.name.as_str();
                                if !(match role {
                                    "queue" => matches!(name, "length" | "shift"),
                                    _ => matches!(name, "push" | "forEach"),
                                }) {
                                    return None;
                                }
                                if name != "length" {
                                    let AstKind::CallExpression(c) =
                                        nodes.parent_kind(nodes.parent_id(node))
                                    else {
                                        return None;
                                    };
                                    if c.optional
                                        || strip(&c.callee).span() != m.span
                                        || plain_arguments(c)?.len()
                                            != if name == "shift" { 0 } else { 1 }
                                    {
                                        return None;
                                    }
                                }
                                if role == "queue"
                                    && name == "shift"
                                    && self.index.owner_of(node) != Some(self.drain)
                                {
                                    return None;
                                }
                                if name == "length" {
                                    if let AstKind::AssignmentExpression(a) =
                                        nodes.parent_kind(nodes.parent_id(node))
                                    {
                                        if a.operator != AssignmentOperator::Assign
                                            || a.left.span() != m.span
                                            || !is_zero(&a.right)
                                        {
                                            return None;
                                        }
                                    }
                                    if matches!(
                                        nodes.parent_kind(nodes.parent_id(node)),
                                        AstKind::UpdateExpression(_)
                                    ) || matches!(nodes.parent_kind(nodes.parent_id(node)), AstKind::UnaryExpression(u) if u.operator == UnaryOperator::Delete)
                                    {
                                        return None;
                                    }
                                }
                            }
                            AstKind::CallExpression(c) if role == "queue" => {
                                let args = plain_arguments(c)?;
                                if c.optional
                                    || args.len() != 2
                                    || analysis::expression_symbol(strip(args[0]), scoping)
                                        != Some(s)
                                    || !matches!(strip(&c.callee), Expression::StaticMemberExpression(apply)
                                        if !apply.optional && apply.property.name == "apply"
                                        && matches!(strip(&apply.object), Expression::StaticMemberExpression(push)
                                            if !push.optional && push.property.name == "push"
                                            && matches!(strip(&push.object), Expression::StaticMemberExpression(proto)
                                                if !proto.optional && proto.property.name == "prototype"
                                                && is_unbound(self.index, &proto.object, "Array"))))
                                {
                                    return None;
                                }
                            }
                            _ => return None,
                        }
                    }
                }
            }
            // Captured status starts false; only booleans may ever be written.
            for role in ["running", "failed"] {
                let s = self.role(role)?;
                let values = self.direct_values(s);
                let [value] = values.as_slice() else {
                    return None;
                };
                if !matches!(strip(value), Expression::BooleanLiteral(b) if !b.value) {
                    return None;
                }
                if self
                    .index
                    .plain_writes
                    .get(&s)
                    .into_iter()
                    .flatten()
                    .any(|v| !matches!(strip(v), Expression::BooleanLiteral(_)))
                {
                    return None;
                }
            }
            for role in ["stored", "failed"] {
                let s = self.role(role)?;
                for reference in scoping.get_resolved_references(s).filter(|r| r.is_write()) {
                    if self.index.owner_of(reference.node_id()) != Some(self.drain)
                        && self.index.owner_of(reference.node_id()) != Some(self.factory)
                    {
                        return None;
                    }
                }
            }
            Some(())
        }

        fn run(&mut self, guard_commit: bool) -> Option<()> {
            let FnNode::Function(f) = self.index.units[self.drain].func else {
                return None;
            };
            if !Self::simple_function(f, 0) {
                return None;
            }
            let [scratch, start, zero_a, zero_b, Statement::ForStatement(loop_)] =
                self.index.units[self.drain].statements()
            else {
                return None;
            };
            if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() {
                return None;
            }
            let Statement::VariableDeclaration(d) = start else {
                return None;
            };
            let [v] = d.declarations.as_slice() else {
                return None;
            };
            let Expression::CallExpression(clock) = strip(v.init.as_ref()?) else {
                return None;
            };
            if clock.optional || !clock.arguments.is_empty() {
                return None;
            }
            self.bind(&clock.callee, "clock")?;
            self.declaration(scratch, "caught", None)?;
            self.declaration(start, "start", Some("clock()"))?;
            self.declaration(
                zero_a,
                if guard_commit { "mean" } else { "count" },
                Some("0"),
            )?;
            self.declaration(
                zero_b,
                if guard_commit { "count" } else { "mean" },
                Some("0"),
            )?;
            let [Statement::IfStatement(branch), tail, done] = Self::block(&loop_.body)? else {
                return None;
            };
            let Expression::BinaryExpression(test) = strip(&branch.test) else {
                return None;
            };
            let Expression::StaticMemberExpression(m) = strip(&test.left) else {
                return None;
            };
            self.bind(&m.object, "queue")?;
            if !self.is(&branch.test, "(queue.length<=0)") {
                return None;
            }
            let [stop, callback, each] = Self::block(&branch.consequent)? else {
                return None;
            };
            let a = assignment_statement(stop)?;
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                return None;
            };
            self.bind_symbol(
                analysis::identifier_symbol(id, self.index.ctx.scoping)?,
                "running",
            )?;
            self.statement(stop, "running=false")?;
            self.notify(callback, each, false)?;
            let work = Self::block(branch.alternate.as_ref()?)?;
            if work.len() != if guard_commit { 7 } else { 8 } {
                return None;
            }
            self.statement(&work[0], "running=true")?;
            self.declaration(&work[1], "task", Some("queue.shift()"))?;
            self.declaration(&work[2], "before", Some("clock()"))?;
            let Statement::TryStatement(t) = &work[3] else {
                return None;
            };
            if t.finalizer.is_some() {
                return None;
            }
            let [Statement::ExpressionStatement(invoke)] = t.block.body.as_slice() else {
                return None;
            };
            let Expression::CallExpression(invoke) = strip(&invoke.expression) else {
                return None;
            };
            self.bind(&invoke.callee, "worker")?;
            if !self.is(work_expression(&work[3])?, "worker(task)") {
                return None;
            }
            let handler = t.handler.as_ref()?;
            let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
                return None;
            };
            self.bind_symbol(error.symbol_id.get()?, "error")?;
            let [caught, stored, failed, stop, clear, callback, each, error_tail, error_done] =
                handler.body.body.as_slice()
            else {
                return None;
            };
            self.statement(caught, "caught=error")?;
            for (s, role) in [(stored, "stored"), (failed, "failed")] {
                let a = assignment_statement(s)?;
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                    return None;
                };
                self.bind_symbol(
                    analysis::identifier_symbol(id, self.index.ctx.scoping)?,
                    role,
                )?;
            }
            self.statement(stored, "stored=error")?;
            self.statement(failed, "failed=true")?;
            self.statement(stop, "running=false")?;
            self.statement(clear, "queue.length=0")?;
            self.notify(callback, each, true)?;
            self.tail(error_tail)?;
            Self::done(error_done)?;
            self.declaration(&work[4], "after", Some("clock()"))?;
            const MEAN: &str = "(((mean*count)+(after-before))/(count+1))";
            let guard = if guard_commit {
                self.declaration(&work[5], "next", Some("(Number(count)+1)"))?;
                &work[6]
            } else {
                self.statement(&work[5], &format!("mean={MEAN}"))?;
                self.statement(&work[6], "++count")?;
                &work[7]
            };
            let Statement::IfStatement(guard) = guard else {
                return None;
            };
            let Expression::BinaryExpression(compare) = strip(&guard.test) else {
                return None;
            };
            let Expression::BinaryExpression(deadline) = strip(&compare.right) else {
                return None;
            };
            self.bind(&deadline.right, "budget")?;
            let cost = if guard_commit { MEAN } else { "mean" };
            if guard.alternate.is_some()
                || !self.is(&guard.test, &format!("((after+{cost})<(start+budget))"))
            {
                return None;
            }
            let continuation = Self::block(&guard.consequent)?;
            let last = if guard_commit {
                let [mean, count, last] = continuation else {
                    return None;
                };
                self.statement(mean, &format!("mean={MEAN}"))?;
                self.statement(count, "count=next")?;
                last
            } else {
                let [last] = continuation else {
                    return None;
                };
                last
            };
            if !matches!(last, Statement::ContinueStatement(c) if c.label.is_none()) {
                return None;
            }
            self.tail(tail)?;
            Self::done(done)?;
            self.origins()
        }
    }

    fn work_expression<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
        let Statement::TryStatement(t) = s else {
            return None;
        };
        let [Statement::ExpressionStatement(e)] = t.block.body.as_slice() else {
            return None;
        };
        Some(&e.expression)
    }

    pub(super) fn prove(index: &Index<'_, '_>, factory: usize, drain: usize) -> bool {
        [false, true].into_iter().any(|guard_commit| {
            Proof {
                index,
                factory,
                drain,
                roles: BTreeMap::new(),
            }
            .run(guard_commit)
            .is_some()
        })
    }
}
