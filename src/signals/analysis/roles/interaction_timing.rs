//! Package `p12_interaction_timing`: the interaction and timing collectors.
//!
//! See `roles/mod.rs` for the recognition rules.
//!
//! # How this package decides
//!
//! Every role here is proven from what a function *does* with a DOM event and
//! from the stable host API it uses:
//!
//! * an event listener is a function handed to `addEventListener(name, ...)`
//!   (directly, through a MAIN binding, or hoisted) and a cleanup is the
//!   function that removes the same set of names with `removeEventListener`;
//! * a sample recorder calls `put` on a ring buffer with a record built from
//!   the event; a hold/interval statistic recorder writes clock deltas into
//!   captured counters; a flush callback checks the group's dirty flag and
//!   emits the ring through `next`;
//! * the throttle factory is the function that returns a latch-guarded wrapper
//!   which schedules the latch release with `setTimeout` and forwards
//!   `fn.apply(this, arguments)`.
//!
//! Both tracer shapes read as one through [`ParameterSlots`] (`arguments[N]`
//! and the Nth formal are one slot), the `.call(receiver, ...)` invocation
//! form, and quote-preserving member names. Generated spellings, byte offsets,
//! line numbers, numeric constants and the target lists are never read.
//!
//! The orientation running-statistics helper may be emitted as an owned
//! function declaration nested in the probe-initializer arrow instead of as a
//! callback found in the collector function set. It is still the same
//! `updateOrientationRunningStats` body, so it is proven from its own bounded
//! shape plus the direct host registration: the arrow that owns the
//! `addEventListener("deviceorientation", helper, {passive:true,capture:true})`
//! call owns the helper declaration, the callback resolves to that declaration
//! by the same scoped symbol, and the owned body keeps the running means and
//! second moments of the inter-event clock delta and of `alpha`, `beta` and
//! `gamma`. Parent probe initializers and wave collectors are containers and
//! are never claimed. Two source copies are two independently proven
//! declarations, so both keep the canonical role and the naming plan decides
//! their concrete names.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{
    AssignmentOperator, BinaryOperator, UnaryOperator, UpdateOperator,
};

use crate::signals::extract::bindings::{property_key_name, static_member_name, ParameterSlots};
use crate::signals::schema::Confidence;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};
use crate::analysis;

const THROTTLE_FACTORY: &str = "interaction_throttle_factory";
const THROTTLED_WRAPPER: &str = "interaction_throttled_wrapper";
const THROTTLE_RELEASE: &str = "interaction_throttle_release";
const VISIBILITY_WHEEL_COLLECTOR: &str = "interaction_visibility_wheel_collector";
const VISIBILITY_RECORDER: &str = "interaction_visibility_sample_recorder";
const WHEEL_HANDLER: &str = "interaction_wheel_handler";
const VISIBILITY_WHEEL_LAST_CHANCE: &str = "interaction_visibility_wheel_last_chance";
const VISIBILITY_WHEEL_CLEANUP: &str = "interaction_visibility_wheel_cleanup";
const KEYSTROKE_COLLECTOR: &str = "interaction_keystroke_hold_collector";
const KEYDOWN_TIMING: &str = "interaction_keydown_timing_recorder";
const KEYUP_HOLD_STATS: &str = "interaction_keyup_hold_statistics_recorder";
const KEYSTROKE_INIT: &str = "interaction_keystroke_timing_initializer";
const DIGEST_SENDER: &str = "interaction_digest_telemetry_sender";
const DIGEST_THEN: &str = "interaction_digest_url_substitution";
const DIGEST_CATCH: &str = "interaction_digest_failure_ignorer";
const KEYSTROKE_LAST_CHANCE: &str = "interaction_keystroke_last_chance";
const KEYSTROKE_CLEANUP: &str = "interaction_keystroke_timing_cleanup";
const KEYBOARD_RECORDER: &str = "interaction_keyboard_event_sample_recorder";
const KEYBOARD_LAST_CHANCE: &str = "interaction_keyboard_sample_last_chance";
const KEYBOARD_CLEANUP: &str = "interaction_keyboard_sample_cleanup";
const KEYDOWN_SAMPLE: &str = "interaction_keydown_sample_handler";
const KEYUP_SAMPLE: &str = "interaction_keyup_sample_handler";
const KEYPRESS_SAMPLE: &str = "interaction_keypress_sample_handler";
const ORIENTATION_UPDATER: &str = "interaction_orientation_running_stats";
const ORIENTATION_CLEANUP: &str = "interaction_orientation_cleanup";
const ORIENTATION_LAST_CHANCE: &str = "interaction_orientation_last_chance";
const INPUT_RECORDER: &str = "interaction_document_input_recorder";
const INPUT_INIT: &str = "interaction_document_input_initializer";
const INPUT_CLEANUP: &str = "interaction_document_input_cleanup";
const TOUCH_START: &str = "interaction_touch_start_recorder";
const TOUCH_MOVE: &str = "interaction_touch_move_recorder";
const TOUCH_END: &str = "interaction_touch_end_recorder";
const TOUCH_CLEANUP: &str = "interaction_touch_cleanup";
const INTERVAL_LAST_CHANCE: &str = "interaction_mouse_move_interval_last_chance";
const INTERVAL_INIT: &str = "interaction_mouse_move_interval_initializer";
const INTERVAL_TICK: &str = "interaction_mouse_move_interval_tick";
const INTERVAL_CLEANUP: &str = "interaction_mouse_move_interval_cleanup";
const THROTTLED_MOUSE: &str = "interaction_throttled_mouse_move_recorder";
const MOUSE_LAST_CHANCE: &str = "interaction_mouse_sample_last_chance";
const MOUSE_PREDICATE: &str = "interaction_mouse_move_until_mouse_up_predicate";
const MOUSE_UP: &str = "interaction_mouse_up_sample_recorder";
const MOUSE_DOWN: &str = "interaction_mouse_down_sample_recorder";
const MOUSE_CLICK: &str = "interaction_click_sample_recorder";
const MOUSE_MOVE: &str = "interaction_mouse_move_sample_recorder";
const MOUSE_CLEANUP: &str = "interaction_mouse_cleanup";
const RING_ALIAS: &str = "interaction_collector_ring_buffer";
const ELAPSED_SINCE_INIT: &str = "interaction_elapsed_time_since_init";
const ELAPSED_CLOCK: &str = "interaction_elapsed_time_clock";
const SEED_COUNTER: &str = "interaction_seed_wave_counter";
const SEED_QUEUE: &str = "interaction_seed_wave_queue";
const RANDOM_SOURCE: &str = "interaction_random_source";
const CRYPTO_SOURCE: &str = "interaction_crypto_source";
const PERFORMANCE_SOURCE: &str = "interaction_performance_source";
const SEED_DATE: &str = "interaction_seed_date_millis";
const FLAGGED_KEYS: &str = "interaction_flagged_key_names";
const HAS_OWN_ALIAS: &str = "interaction_has_own_property_check";

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole {
        role,
        name,
        kind: FinderRoleKind::Function,
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
    function(THROTTLE_FACTORY, "createThrottle"),
    function(THROTTLED_WRAPPER, "runThrottledCallback"),
    function(THROTTLE_RELEASE, "releaseThrottleLock"),
    function(
        VISIBILITY_WHEEL_COLLECTOR,
        "collectVisibilityAndWheelSignals",
    ),
    function(VISIBILITY_RECORDER, "recordVisibilityChangeSample"),
    function(WHEEL_HANDLER, "handleWheelEvent"),
    function(
        VISIBILITY_WHEEL_LAST_CHANCE,
        "flushVisibilityAndWheelSamples",
    ),
    function(
        VISIBILITY_WHEEL_CLEANUP,
        "removeVisibilityAndWheelListeners",
    ),
    function(KEYSTROKE_COLLECTOR, "collectKeystrokeHoldTiming"),
    function(KEYDOWN_TIMING, "recordKeydownTiming"),
    function(KEYUP_HOLD_STATS, "recordKeyupHoldStatistics"),
    function(KEYSTROKE_INIT, "initializeKeystrokeTimingProbe"),
    function(DIGEST_SENDER, "sendDigestTelemetryRequest"),
    function(DIGEST_THEN, "substituteDigestInRequestUrl"),
    function(DIGEST_CATCH, "ignoreDigestFailure"),
    function(KEYSTROKE_LAST_CHANCE, "flushKeystrokeHoldStatistics"),
    function(KEYSTROKE_CLEANUP, "removeKeystrokeTimingListeners"),
    function(KEYBOARD_RECORDER, "recordKeyboardEventSample"),
    function(KEYBOARD_LAST_CHANCE, "flushKeyboardEventSamples"),
    function(KEYBOARD_CLEANUP, "removeKeyboardEventListeners"),
    function(KEYDOWN_SAMPLE, "handleKeydownEvent"),
    function(KEYUP_SAMPLE, "handleKeyupEvent"),
    function(KEYPRESS_SAMPLE, "handleKeypressEvent"),
    function(ORIENTATION_UPDATER, "updateOrientationRunningStats"),
    function(ORIENTATION_CLEANUP, "removeOrientationListener"),
    function(ORIENTATION_LAST_CHANCE, "flushOrientationStatistics"),
    function(INPUT_RECORDER, "recordDocumentInputEvent"),
    function(INPUT_INIT, "registerDocumentInputListener"),
    function(INPUT_CLEANUP, "removeDocumentInputListener"),
    function(TOUCH_START, "onTouchStart"),
    function(TOUCH_MOVE, "onTouchMove"),
    function(TOUCH_END, "recordTouchEndSample"),
    function(TOUCH_CLEANUP, "removeTouchListeners"),
    function(INTERVAL_LAST_CHANCE, "flushMouseMoveIntervalStatistics"),
    function(INTERVAL_INIT, "startMouseMoveIntervalTimer"),
    function(INTERVAL_TICK, "recordMouseMoveIntervalTick"),
    function(INTERVAL_CLEANUP, "stopMouseMoveIntervalTimer"),
    function(THROTTLED_MOUSE, "recordThrottledMouseMoveSample"),
    function(MOUSE_LAST_CHANCE, "flushMouseMoveSamples"),
    function(MOUSE_PREDICATE, "keepMouseMoveBeforeMouseUp"),
    function(MOUSE_UP, "recordMouseUpSample"),
    function(MOUSE_DOWN, "captureMouseDownEvent"),
    function(MOUSE_CLICK, "captureClickEvent"),
    function(MOUSE_MOVE, "captureMouseMoveEvent"),
    function(MOUSE_CLEANUP, "removeMouseListeners"),
    binding(RING_ALIAS, "collectorRingBuffer"),
    binding(ELAPSED_SINCE_INIT, "elapsedTimeSinceInit"),
    binding(ELAPSED_CLOCK, "elapsedTimeClock"),
    binding(SEED_COUNTER, "seedWaveCounter"),
    binding(SEED_QUEUE, "seedWaveQueue"),
    binding(RANDOM_SOURCE, "randomSource"),
    binding(CRYPTO_SOURCE, "cryptoSource"),
    binding(PERFORMANCE_SOURCE, "performanceSource"),
    binding(SEED_DATE, "seedDateMillis"),
    binding(FLAGGED_KEYS, "flaggedKeyNames"),
    binding(HAS_OWN_ALIAS, "hasOwnPropertyCheck"),
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
    ifs: Vec<&'a IfStatement<'a>>,
    binaries: Vec<&'a BinaryExpression<'a>>,
    unaries: Vec<&'a UnaryExpression<'a>>,
    updates: Vec<&'a UpdateExpression<'a>>,
    arrays: Vec<&'a ArrayExpression<'a>>,
    objects: Vec<&'a ObjectExpression<'a>>,
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
    assignments: Vec<&'a AssignmentExpression<'a>>,
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
    call.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
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
            assignments: Vec::new(),
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
                self.fn_of_node
                    .get(&parent)
                    .copied()
                    .or(self.owner[parent.index()])
            };
            self.owner[id.index()] = owner;
            let found = match node.kind() {
                AstKind::Function(found) => Some((found.span, FnNode::Function(found))),
                AstKind::ArrowFunctionExpression(found) => Some((found.span, FnNode::Arrow(found))),
                _ => None,
            };
            if let Some((span, func)) = found {
                let index = self.fns.len();
                self.fns.push(FnEntry {
                    node: id,
                    span,
                    func,
                    parent: owner,
                });
                self.own.push(Own::default());
                self.fn_of_node.insert(id, index);
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
        match kind {
            AstKind::AssignmentExpression(assignment) => {
                if assignment.operator == AssignmentOperator::Assign {
                    if let AssignmentTarget::AssignmentTargetIdentifier(identifier) =
                        &assignment.left
                    {
                        if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping)
                        {
                            self.assigns.entry(symbol).or_default().push(assignment);
                        }
                    }
                }
                self.assignments.push(assignment);
                if let Some(owner) = owner {
                    if let Some(name) = target_member_name(&assignment.left) {
                        self.own[owner].written_keys.insert(name.to_string());
                    }
                    self.own[owner].assignments.push(assignment);
                }
            }
            AstKind::ObjectProperty(property) if property.kind == PropertyKind::Init => {
                if let Some(name) = property_key_name(&property.key) {
                    if let Some(owner) = owner {
                        self.own[owner].written_keys.insert(name.clone());
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
                    own.returns.push(argument);
                }
            }
            AstKind::UnaryExpression(unary) => own.unaries.push(unary),
            AstKind::BinaryExpression(binary) => own.binaries.push(binary),
            AstKind::UpdateExpression(update) => own.updates.push(update),
            AstKind::IfStatement(statement) => own.ifs.push(statement),
            AstKind::ArrayExpression(array) => own.arrays.push(array),
            AstKind::ObjectExpression(object) => own.objects.push(object),
            AstKind::IdentifierReference(identifier) => {
                if let (Some(symbol), Some(reference)) = (
                    analysis::identifier_symbol(identifier, scoping),
                    identifier.reference_id.get(),
                ) {
                    if scoping.get_reference(reference).is_read() {
                        own.reads.insert(symbol);
                    }
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
            _ => {}
        }
    }

    fn span(&self, index: usize) -> Span {
        self.fns[index].span
    }

    fn parent(&self, index: usize) -> Option<usize> {
        self.fns[index].parent
    }

    fn is_arrow(&self, index: usize) -> bool {
        matches!(self.fns[index].func, FnNode::Arrow(_))
    }

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.scoping)
    }

    fn value_name(&self, expression: &Expression<'a>) -> Option<String> {
        self.value_name_depth(expression, 0)
    }

    fn value_name_depth(&self, expression: &Expression<'a>, depth: u8) -> Option<String> {
        if let Some((_, name)) = member(expression) {
            return Some(name.to_string());
        }
        if depth > 6 {
            return None;
        }
        let symbol = self.symbol(expression)?;
        let origin = self.single_origin(symbol)?;
        self.value_name_depth(origin, depth + 1)
    }

    /// The unbound-global member path a value expression denotes.
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

    fn initializer(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self.nodes.kind(self.scoping.symbol_declaration(symbol)) {
            AstKind::VariableDeclarator(declarator) => declarator.init.as_ref(),
            _ => None,
        }
    }

    fn write_count(&self, symbol: SymbolId) -> usize {
        self.scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count()
    }

    /// Resolve an identifier through single-origin `alias = target` links to
    /// the binding that holds the value.
    fn resolve_symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        self.resolve_symbol_depth(expression, 0)
    }

    fn resolve_symbol_depth(&self, expression: &Expression<'a>, depth: u8) -> Option<SymbolId> {
        let symbol = self.symbol(expression)?;
        if depth > 6 {
            return Some(symbol);
        }
        if let Some(origin) = self.single_origin(symbol) {
            if matches!(strip(origin), Expression::Identifier(_)) {
                if let Some(target) = self.resolve_symbol_depth(origin, depth + 1) {
                    return Some(target);
                }
            }
        }
        Some(symbol)
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
                && !self
                    .scoping
                    .get_resolved_references(symbol)
                    .any(|r| r.is_write());
            return immutable.then_some(index);
        }
        // `let handler = null; ... handler = fn;` is the standard hoisted
        // callback shape. Every function-valued origin must agree; non-function
        // placeholders (`null`, `void 0`) lend nothing.
        let mut found = None;
        for origin in self.origins(symbol)? {
            if let Some(index) = self.resolve_fn_depth(origin, depth - 1) {
                if found.is_some() && found != Some(index) {
                    return None;
                }
                found = Some(index);
            }
        }
        found
    }

    /// The callee and arguments of a call, seen through the target cohort's
    /// `callee.call(receiver, ...)` form.
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

    fn is_incoming(&self, expression: &Expression<'a>) -> bool {
        self.slots.incoming_slot(strip(expression)).is_some()
    }

    /// The method name of a `receiver.name(...)` call and its receiver.
    fn method<'e>(&self, call: &'e CallExpression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
        member(self.invocation(call).0)
    }

    /// The stable name of the called host/member operation, also through a
    /// local copy of the global (`let f = global.setInterval; f(...)`).
    fn call_name(&self, call: &CallExpression<'a>) -> Option<String> {
        let (callee, _) = self.invocation(call);
        if let Some((_, name)) = member(callee) {
            return Some(name.to_string());
        }
        let Expression::Identifier(identifier) = strip(callee) else {
            return None;
        };
        let Some(symbol) = self.symbol(callee) else {
            return Some(identifier.name.to_string());
        };
        self.single_origin(symbol)
            .and_then(|origin| self.value_name(origin))
    }

    /// The parameter slot of `function` an expression denotes, directly or
    /// through one local copy.
    fn slot_of(&self, expression: &Expression<'a>, function: usize) -> Option<u32> {
        if let Some(slot) = self.slots.incoming_slot(strip(expression)) {
            if slot.owner == self.fns[function].node {
                return Some(slot.index);
            }
        }
        // The emitted code copies a slot into a branch-joined local: every
        // origin of that local must be the same slot of the same function.
        let symbol = self.symbol(expression)?;
        let mut slot = None;
        for origin in self.origins(symbol)? {
            let found = self.slots.incoming_slot(strip(origin))?;
            if found.owner != self.fns[function].node {
                return None;
            }
            if slot.is_some() && slot != Some(found.index) {
                return None;
            }
            slot = Some(found.index);
        }
        slot
    }

    fn has_call_name(&self, function: usize, name: &str) -> bool {
        self.own[function]
            .calls
            .iter()
            .any(|call| self.call_name(call).as_deref() == Some(name))
    }

    fn call_named(&self, function: usize, name: &str) -> Vec<&'a CallExpression<'a>> {
        self.own[function]
            .calls
            .iter()
            .filter(|call| self.call_name(call).as_deref() == Some(name))
            .copied()
            .collect()
    }

    fn first_fn_argument(&self, call: &CallExpression<'a>) -> Option<usize> {
        args(call)
            .first()
            .and_then(|argument| self.resolve_fn(argument))
    }

    /// The ring-buffer binding a function `put`s into.
    fn put_ring(&self, function: usize) -> Option<SymbolId> {
        for call in &self.own[function].calls {
            if let Some((alias, "call")) = member(&call.callee) {
                if let Some(ring) = self.aliased_put_ring(call, alias, function) {
                    return Some(ring);
                }
            }
            if self.call_name(call).as_deref() != Some("put") {
                continue;
            }
            let (callee, _) = self.invocation(call);
            if let Some((receiver, _)) = member(callee) {
                if let Some(symbol) = self.resolve_symbol(receiver) {
                    return Some(symbol);
                }
            }
        }
        None
    }

    /// The bounded extracted-alias spelling
    /// `let method = ring.put; method.call(ring, row);`.
    ///
    /// The alias must have one local declaration and exactly one origin (the
    /// exact `.put` member of a stable ring), the `.call` must pass exactly the
    /// row argument, and the first argument must resolve to that same ring
    /// directly or through one stable same-function alias. Both definitions
    /// must precede the call, in the same function, with no conditional or loop
    /// ancestor. `invocation` is deliberately not widened: any other `.call`
    /// and any other member name stays unidentified.
    fn aliased_put_ring(
        &self,
        call: &CallExpression<'a>,
        alias: &Expression<'a>,
        function: usize,
    ) -> Option<SymbolId> {
        let arguments = args(call);
        let [receiver, _row] = arguments.as_slice() else {
            return None;
        };
        let call_node = call.node_id.get();
        let call_block = self.enclosing_block(call_node, function)?;
        if self.inside_branch(call_node, function) {
            return None;
        }
        let alias_symbol = self.symbol(alias)?;
        let origin = self.single_origin(alias_symbol)?;
        let (ring_expression, name) = member(origin)?;
        if name != "put" {
            return None;
        }
        let ring = self.symbol(ring_expression)?;
        let ring_origin = self.single_origin(ring)?;
        if !self.origin_precedes(ring, ring_origin, call.span) {
            return None;
        }
        if !self.receiver_is_ring(*receiver, ring, function, call.span, call_block) {
            return None;
        }
        self.definition_dominates(alias_symbol, origin, function, call.span, call_block)
            .then_some(ring)
    }

    /// The `.call` receiver is the ring itself or one stable same-function
    /// alias of it.
    fn receiver_is_ring(
        &self,
        expression: &Expression<'a>,
        ring: SymbolId,
        function: usize,
        call_span: Span,
        call_block: NodeId,
    ) -> bool {
        let Some(symbol) = self.symbol(expression) else {
            return false;
        };
        if symbol == ring {
            return true;
        }
        let Some(origin) = self.single_origin(symbol) else {
            return false;
        };
        if !matches!(strip(origin), Expression::Identifier(_)) || self.symbol(origin) != Some(ring)
        {
            return false;
        }
        self.definition_dominates(symbol, origin, function, call_span, call_block)
    }

    /// The alias declaration/assignment and its origin both belong to
    /// `function`, end before the call, and have no conditional or loop
    /// ancestor.
    fn definition_dominates(
        &self,
        symbol: SymbolId,
        origin: &Expression<'a>,
        function: usize,
        call_span: Span,
        call_block: NodeId,
    ) -> bool {
        let declaration = self.scoping.symbol_declaration(symbol);
        if self.owner[declaration.index()] != Some(function) {
            return false;
        }
        let Some(node) = self.origin_node(symbol, origin) else {
            return false;
        };
        if self.owner[node.index()] != Some(function) {
            return false;
        }
        if self.nodes.get_node(node).span().end > call_span.start
            || origin.span().end > call_span.start
        {
            return false;
        }
        if self.enclosing_block(node, function) != Some(call_block) {
            return false;
        }
        !self.inside_branch(node, function)
    }

    /// The nearest statement block (or function body) that owns `node`.
    fn enclosing_block(&self, node: NodeId, function: usize) -> Option<NodeId> {
        let root = self.fns[function].node;
        let mut current = node;
        loop {
            if current == root {
                return None;
            }
            match self.nodes.kind(current) {
                AstKind::BlockStatement(_) | AstKind::FunctionBody(_) => return Some(current),
                _ => {}
            }
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
        }
    }

    /// The one origin of `symbol` ends before the call.
    fn origin_precedes(
        &self,
        symbol: SymbolId,
        origin: &Expression<'a>,
        call_span: Span,
    ) -> bool {
        self.origin_node(symbol, origin)
            .is_some_and(|node| self.nodes.get_node(node).span().end <= call_span.start)
            && origin.span().end <= call_span.start
    }

    /// The AST node that establishes `origin`: the declarator initializer or
    /// the one plain assignment.
    fn origin_node(&self, symbol: SymbolId, origin: &Expression<'a>) -> Option<NodeId> {
        let declaration = self.scoping.symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) {
            if declarator
                .init
                .as_ref()
                .is_some_and(|init| std::ptr::eq(init, origin))
            {
                return Some(declaration);
            }
        }
        self.assigns.get(&symbol)?.iter().find_map(|assignment| {
            std::ptr::eq(&assignment.right, origin).then_some(assignment.node_id.get())
        })
    }

    /// Whether `node` sits under a conditional or loop inside `function`.
    fn inside_branch(&self, node: NodeId, function: usize) -> bool {
        let root = self.fns[function].node;
        let mut current = node;
        loop {
            if current == root {
                return false;
            }
            match self.nodes.kind(current) {
                AstKind::IfStatement(_)
                | AstKind::ConditionalExpression(_)
                | AstKind::LogicalExpression(_)
                | AstKind::ForStatement(_)
                | AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_)
                | AstKind::WhileStatement(_)
                | AstKind::DoWhileStatement(_)
                | AstKind::SwitchStatement(_) => return true,
                _ => {}
            }
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return false;
            }
            current = parent;
        }
    }

    fn written_symbols(&self, function: usize) -> BTreeSet<SymbolId> {
        let mut written = BTreeSet::new();
        for assignment in &self.own[function].assignments {
            if assignment.operator != AssignmentOperator::Assign {
                continue;
            }
            if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                    written.insert(symbol);
                }
            }
        }
        for update in &self.own[function].updates {
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &update.argument
            {
                if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                    written.insert(symbol);
                }
            }
        }
        written
    }

    fn false_writes(&self, function: usize) -> BTreeSet<SymbolId> {
        let mut false_written = BTreeSet::new();
        for assignment in &self.own[function].assignments {
            if assignment.operator != AssignmentOperator::Assign
                || !is_boolean(&assignment.right, false)
            {
                continue;
            }
            if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                    false_written.insert(symbol);
                }
            }
        }
        false_written
    }

    fn readers(&self, symbol: SymbolId) -> BTreeSet<usize> {
        (0..self.fns.len())
            .filter(|function| self.own[*function].reads.contains(&symbol))
            .collect()
    }

    /// True when the function is handed to one of its parent's parameter slots.
    fn passed_to_parent_slot(&self, function: usize) -> bool {
        let Some(parent) = self.parent(function) else {
            return false;
        };
        self.own[parent].calls.iter().any(|call| {
            let (callee, arguments) = self.invocation(call);
            self.slot_of(callee, parent).is_some()
                && arguments
                    .iter()
                    .any(|argument| self.resolve_fn(argument) == Some(function))
        })
    }

    /// A group coordinator takes the init/emit/last-chance/cleanup callbacks
    /// as parameter slots and calls several of them.
    fn is_group_function(&self, function: usize) -> bool {
        if self.is_arrow(function) {
            return false;
        }
        let slot_calls = self.own[function]
            .calls
            .iter()
            .filter(|call| {
                let (callee, arguments) = self.invocation(call);
                self.slot_of(callee, function).is_some()
                    && arguments
                        .iter()
                        .any(|argument| self.resolve_fn(argument).is_some())
            })
            .count();
        slot_calls >= 3
    }

    fn group_above(&self, function: usize) -> Option<usize> {
        let mut current = Some(function);
        for _ in 0..64 {
            let index = current?;
            if self.is_group_function(index) {
                return Some(index);
            }
            current = self.parent(index);
        }
        None
    }

    /// `expr` is an empty-bodied (or `return void 0`) function: a swallowed
    /// rejection handler.
    fn is_empty_body(&self, function: usize) -> bool {
        let own = &self.own[function];
        own.calls.is_empty()
            && own.news.is_empty()
            && own.assignments.is_empty()
            && own.ifs.is_empty()
            && own.binaries.is_empty()
            && own.arrays.is_empty()
            && own.objects.is_empty()
            && own.returns.iter().all(|value| {
                matches!(strip(value), Expression::UnaryExpression(unary)
                    if unary.operator == UnaryOperator::Void)
                    || matches!(strip(value), Expression::NullLiteral(_))
                    || matches!(strip(value), Expression::Identifier(identifier)
                        if identifier.name == "undefined")
            })
    }

    fn is_ring_buffer_class(&self, symbol: SymbolId) -> bool {
        match self.nodes.kind(self.scoping.symbol_declaration(symbol)) {
            AstKind::Class(class) => {
                let methods: BTreeSet<String> = class
                    .body
                    .body
                    .iter()
                    .filter_map(|element| match element {
                        ClassElement::MethodDefinition(method) => property_key_name(&method.key),
                        _ => None,
                    })
                    .collect();
                ["put", "toArray", "clear"]
                    .iter()
                    .all(|name| methods.contains(*name))
            }
            AstKind::Function(_) => {
                let mut methods = BTreeSet::new();
                for assignment in &self.assignments {
                    let Some(name) = target_member_name(&assignment.left) else {
                        continue;
                    };
                    if !matches!(name, "put" | "toArray" | "clear") {
                        continue;
                    }
                    let Some(object) = target_member_object(&assignment.left) else {
                        continue;
                    };
                    let Some((prototype, "prototype")) = member(object) else {
                        continue;
                    };
                    if self.symbol(prototype) == Some(symbol) {
                        methods.insert(name.to_string());
                    }
                }
                ["put", "toArray", "clear"]
                    .iter()
                    .all(|name| methods.contains(*name))
            }
            _ => false,
        }
    }

    /// The elapsed-clock factory: it reads `performance.now`, returns a nested
    /// closure that subtracts two clock calls, and records the baseline on the
    /// closure's `.start`.
    fn is_elapsed_factory(&self, function: usize) -> bool {
        let own = &self.own[function];
        if !(own.members.contains("performance") && own.members.contains("now")) {
            return false;
        }
        if !own.written_keys.contains("start") {
            return false;
        }
        own.returns
            .iter()
            .filter_map(|value| self.resolve_fn(value))
            .any(|returned| {
                self.parent(returned) == Some(function)
                    && self.own[returned].binaries.iter().any(|binary| {
                        binary.operator == BinaryOperator::Subtraction
                            && (matches!(strip(&binary.left), Expression::CallExpression(_))
                                || matches!(strip(&binary.right), Expression::CallExpression(_)))
                    })
            })
    }

    /// The `return hasOwnProperty.call(object, key)` helper.
    fn is_has_own_helper(&self, function: usize) -> bool {
        let own = &self.own[function];
        let [call] = own.calls.as_slice() else {
            return false;
        };
        let returned = own.returns.iter().any(|value| {
            matches!(strip(value), Expression::CallExpression(found) if found.span == call.span)
        });
        if !returned {
            return false;
        }
        let Some((receiver, "call")) = self.method(call) else {
            return false;
        };
        let Some(holder) = self.symbol(receiver) else {
            return false;
        };
        let Some((_, "hasOwnProperty")) = self.single_origin(holder).and_then(member) else {
            return false;
        };
        let arguments = args(call);
        arguments.len() == 2
            && arguments
                .iter()
                .enumerate()
                .all(|(index, argument)| self.slot_of(argument, function) == Some(index as u32))
    }

    /// A binding declared outside `function`: the captured state, clock and
    /// host receiver the owned body reads and writes. Locals introduced by the
    /// body and the function's own formals are deliberately not captured.
    fn is_outside(&self, symbol: SymbolId, function: usize) -> bool {
        let owner = self.span(function);
        let mut declared = false;
        for declaration in self.scoping.symbol_declarations(symbol) {
            declared = true;
            let span = self.nodes.kind(declaration).span();
            if span.start >= owner.start && span.end <= owner.end {
                return false;
            }
        }
        declared
    }

    fn inside(&self, outer: Span, inner: Span) -> bool {
        inner.start >= outer.start && inner.end <= outer.end
    }

    /// The handler a registration passes. The callback is read directly by the
    /// same scoped symbol; when one hoisted alias is assigned more than one
    /// probe-local function, the nearest preceding plain assignment in the
    /// owning arrow selects the function that is live at this registration
    /// (the two emitted loop/tail copies share one alias).
    fn registered_handler(&self, call: &CallExpression<'a>, owner: usize) -> Option<usize> {
        let arguments = args(call);
        let callback = arguments.get(1).copied()?;
        if let Some(handler) = self.resolve_fn(callback) {
            return Some(handler);
        }
        let Expression::Identifier(identifier) = strip(callback) else {
            return None;
        };
        let symbol = analysis::identifier_symbol(identifier, self.scoping)?;
        let mut best: Option<(u32, usize)> = None;
        for assignment in self.own[owner].assignments.iter().copied() {
            if assignment.operator != AssignmentOperator::Assign
                || assignment.span.end > call.span.start
            {
                continue;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                continue;
            };
            if analysis::identifier_symbol(target, self.scoping) != Some(symbol) {
                continue;
            }
            let Some(handler) = self.resolve_fn(&assignment.right) else {
                continue;
            };
            if best.is_none_or(|(start, _)| assignment.span.start >= start) {
                best = Some((assignment.span.start, handler));
            }
        }
        best.map(|(_, handler)| handler)
    }

    /// The `deviceorientation` registration is a direct host registration made
    /// by the same arrow that owns the helper: the receiver is a captured
    /// binding (never the arrow's or the helper's own formal), the callback
    /// resolves to the helper by the same scoped symbol, and the options are
    /// the exact `{passive:true,capture:true}` pair.
    fn registration_is_direct(
        &self,
        call: &CallExpression<'a>,
        owner: usize,
        handler: usize,
    ) -> bool {
        let Some((receiver, "addEventListener")) = self.method(call) else {
            return false;
        };
        if self.registered_handler(call, owner) != Some(handler) {
            return false;
        }
        let Some(receiver_symbol) = self.symbol(receiver) else {
            return false;
        };
        if !self.is_outside(receiver_symbol, handler) {
            return false;
        }
        if let Some(slot) = self.slots.incoming_slot(strip(receiver)) {
            if slot.owner == self.fns[owner].node || slot.owner == self.fns[handler].node {
                return false;
            }
        }
        let arguments = args(call);
        registration_options(arguments.get(2).copied())
    }

    /// The owned running-moments body: one captured counter, a first-sample
    /// seed branch, eight `(n-2)/(n-1)` / `1/(n-1)` / `(n-1)/n` / `1/n`
    /// mean-second-moment updates, one captured ready flag, and a try whose
    /// catch forwards the caught binding to a captured error sink.
    fn is_orientation_running_moments(&self, function: usize) -> bool {
        let FnNode::Function(found) = self.fns[function].func else {
            return false;
        };
        if !found.is_declaration() || found.r#async || found.generator {
            return false;
        }
        let [parameter] = found.params.items.as_slice() else {
            return false;
        };
        if found.params.rest.is_some() || parameter.initializer.is_some() {
            return false;
        }
        let BindingPattern::BindingIdentifier(event_binding) = &parameter.pattern else {
            return false;
        };
        let Some(event) = event_binding.symbol_id.get() else {
            return false;
        };

        let Some(body) = found.body.as_deref() else {
            return false;
        };
        let statements = body.statements.as_slice();
        let Some((last, preceding)) = statements.split_last() else {
            return false;
        };
        for statement in preceding {
            let Statement::VariableDeclaration(declaration) = statement else {
                return false;
            };
            if !matches!(
                declaration.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Var
            ) {
                return false;
            }
            if declaration.declarations.is_empty()
                || declaration
                    .declarations
                    .iter()
                    .any(|declarator| {
                        declarator.init.is_some()
                            || !matches!(declarator.id, BindingPattern::BindingIdentifier(_))
                    })
            {
                return false;
            }
        }
        let Statement::TryStatement(try_statement) = last else {
            return false;
        };
        if try_statement.finalizer.is_some() {
            return false;
        }
        let Some(catch_clause) = try_statement.handler.as_deref() else {
            return false;
        };
        let Some(catch_param) = catch_clause.param.as_ref() else {
            return false;
        };
        let BindingPattern::BindingIdentifier(caught_binding) = &catch_param.pattern else {
            return false;
        };
        let Some(caught) = caught_binding.symbol_id.get() else {
            return false;
        };
        if !self.catch_forwards_caught(catch_clause, caught, function) {
            return false;
        }

        let own = &self.own[function];
        let [update] = own.updates.as_slice() else {
            return false;
        };
        if update.operator != UpdateOperator::Increment {
            return false;
        }
        let SimpleAssignmentTarget::AssignmentTargetIdentifier(update_identifier) = &update.argument
        else {
            return false;
        };
        let Some(counter) = analysis::identifier_symbol(update_identifier, self.scoping) else {
            return false;
        };
        if !self.is_outside(counter, function) || !self.inside(try_statement.block.span, update.span)
        {
            return false;
        }

        let [conditional] = own.ifs.as_slice() else {
            return false;
        };
        if !self.inside(try_statement.block.span, conditional.span)
            || !self.is_first_sample_test(&conditional.test, counter)
        {
            return false;
        }

        let mut ready = 0;
        for assignment in &own.assignments {
            if assignment.operator != AssignmentOperator::Assign
                || !is_boolean(&assignment.right, true)
            {
                continue;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                continue;
            };
            let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
                continue;
            };
            if symbol != event && self.is_outside(symbol, function) {
                ready += 1;
            }
        }
        if ready != 1 {
            return false;
        }

        let mut formulas = Vec::new();
        for assignment in &own.assignments {
            if assignment.operator != AssignmentOperator::Assign
                || !self.inside(try_statement.block.span, assignment.span)
            {
                continue;
            }
            let Some(formula) =
                orientation_formula(self, &assignment.right, counter, event, function)
            else {
                continue;
            };
            formulas.push(formula);
        }
        if formulas.len() != 8 {
            return false;
        }
        let mut states = BTreeSet::new();
        let mut delta_linear = 0;
        let mut delta_square = 0;
        let mut angles: BTreeMap<(OrientationAngle, bool), usize> = BTreeMap::new();
        for formula in &formulas {
            if formula.state == counter || formula.state == event {
                return false;
            }
            if !states.insert(formula.state) {
                return false;
            }
            match formula.term {
                OrientationTerm::Delta { squared } => {
                    if formula.ratio != (2, 1) || formula.divisor != 1 {
                        return false;
                    }
                    if squared {
                        delta_square += 1;
                    } else {
                        delta_linear += 1;
                    }
                }
                OrientationTerm::Angle { angle, squared } => {
                    if formula.ratio != (1, 0) || formula.divisor != 0 {
                        return false;
                    }
                    *angles.entry((angle, squared)).or_insert(0) += 1;
                }
            }
        }
        if delta_linear != 1 || delta_square != 1 {
            return false;
        }
        if angles.values().any(|count| *count != 1) {
            return false;
        }
        [
            OrientationAngle::Alpha,
            OrientationAngle::Beta,
            OrientationAngle::Gamma,
        ]
        .iter()
        .all(|angle| {
            angles.contains_key(&(*angle, false)) && angles.contains_key(&(*angle, true))
        })
    }

    /// The catch body is exactly `capturedSink.error(caughtBinding)`.
    fn catch_forwards_caught(
        &self,
        catch_clause: &CatchClause<'a>,
        caught: SymbolId,
        function: usize,
    ) -> bool {
        let [statement] = catch_clause.body.body.as_slice() else {
            return false;
        };
        let Statement::ExpressionStatement(statement) = statement else {
            return false;
        };
        let Expression::CallExpression(call) = strip(&statement.expression) else {
            return false;
        };
        let Some((receiver, "error")) = self.method(call) else {
            return false;
        };
        let arguments = args(call);
        if arguments.len() != 1 || self.symbol(arguments[0]) != Some(caught) {
            return false;
        }
        self.symbol(receiver)
            .is_some_and(|sink| self.is_outside(sink, function))
    }

    /// The first-sample branch test `counter <= 1`.
    fn is_first_sample_test(&self, expression: &Expression<'a>, counter: SymbolId) -> bool {
        let Expression::BinaryExpression(binary) = strip(expression) else {
            return false;
        };
        binary.operator == BinaryOperator::LessEqualThan
            && self.symbol(&binary.left) == Some(counter)
            && matches!(
                strip(&binary.right),
                Expression::NumericLiteral(literal) if literal.value == 1.0
            )
    }
}

/// The three angle identities, in `deviceorientation` property order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OrientationAngle {
    Alpha,
    Beta,
    Gamma,
}

fn orientation_angle(name: &str) -> Option<OrientationAngle> {
    match name {
        "alpha" => Some(OrientationAngle::Alpha),
        "beta" => Some(OrientationAngle::Beta),
        "gamma" => Some(OrientationAngle::Gamma),
        _ => None,
    }
}

/// One term of a running-moment update: the inter-event clock delta or an
/// event angle, optionally squared.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OrientationTerm {
    Delta { squared: bool },
    Angle {
        angle: OrientationAngle,
        squared: bool,
    },
}

struct OrientationFormula {
    state: SymbolId,
    ratio: (i64, i64),
    term: OrientationTerm,
    divisor: i64,
}

/// `event.<angle>` read through the single event formal.
fn event_member<'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &Expression<'a>,
    event: SymbolId,
) -> Option<OrientationAngle> {
    let (object, name) = member(expression)?;
    (ix.symbol(object) == Some(event))
        .then(|| orientation_angle(name))
        .flatten()
}

/// A zero-argument call to a captured callback: the helper's clock.
fn is_clock_call<'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &Expression<'a>,
    function: usize,
) -> bool {
    let Expression::CallExpression(call) = strip(expression) else {
        return false;
    };
    let (callee, arguments) = ix.invocation(call);
    if !arguments.is_empty() {
        return false;
    }
    let Some(symbol) = ix.symbol(callee) else {
        return false;
    };
    ix.is_outside(symbol, function)
}

/// The clock value, directly or through one captured local that only ever
/// holds that call (the emitted delta local).
fn clock_operand<'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &Expression<'a>,
    function: usize,
) -> bool {
    if is_clock_call(ix, expression, function) {
        return true;
    }
    let Some(symbol) = ix.symbol(expression) else {
        return false;
    };
    ix.origins(symbol)
        .is_some_and(|origins| origins.iter().any(|origin| is_clock_call(ix, origin, function)))
}

/// `clock - lastClock` where one side is the captured clock and the other is a
/// captured stored timestamp.
fn clock_delta<'c, 'a>(ix: &Ix<'c, 'a>, expression: &Expression<'a>, function: usize) -> bool {
    let Expression::BinaryExpression(binary) = strip(expression) else {
        return false;
    };
    if binary.operator != BinaryOperator::Subtraction {
        return false;
    }
    let outside = |value: &Expression<'a>| {
        ix.symbol(value)
            .is_some_and(|symbol| ix.is_outside(symbol, function))
    };
    (clock_operand(ix, &binary.left, function) && outside(&binary.right))
        || (clock_operand(ix, &binary.right, function) && outside(&binary.left))
}

/// Classify the increment term of a running update.
fn orientation_term<'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &Expression<'a>,
    event: SymbolId,
    function: usize,
    depth: u8,
) -> Option<OrientationTerm> {
    if depth > 4 {
        return None;
    }
    let expression = strip(expression);
    if let Some(angle) = event_member(ix, expression, event) {
        return Some(OrientationTerm::Angle {
            angle,
            squared: false,
        });
    }
    if let Expression::BinaryExpression(binary) = expression {
        if binary.operator == BinaryOperator::Multiplication {
            let left = orientation_term(ix, &binary.left, event, function, depth + 1);
            let right = orientation_term(ix, &binary.right, event, function, depth + 1);
            match (left, right) {
                (
                    Some(OrientationTerm::Angle {
                        angle,
                        squared: false,
                    }),
                    Some(OrientationTerm::Angle {
                        angle: other,
                        squared: false,
                    }),
                ) if angle == other => {
                    return Some(OrientationTerm::Angle {
                        angle,
                        squared: true,
                    });
                }
                (
                    Some(OrientationTerm::Delta { squared: false }),
                    Some(OrientationTerm::Delta { squared: false }),
                ) => {
                    return Some(OrientationTerm::Delta { squared: true });
                }
                _ => {}
            }
        }
    }
    if clock_delta(ix, expression, function) {
        return Some(OrientationTerm::Delta { squared: false });
    }
    let symbol = ix.symbol(expression)?;
    let origin = ix.single_origin(symbol)?;
    orientation_term(ix, origin, event, function, depth + 1)
}

/// Resolve one body-local alias step, so the emitted `let ratio = (n-1)/n`
/// and `let inverse = 1/(n-1)` spellings read as the same formula.
fn alias_origin<'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &Expression<'a>,
) -> Option<&'a Expression<'a>> {
    ix.symbol(strip(expression))
        .and_then(|symbol| ix.single_origin(symbol))
}

/// The counter offset of `counter`, `counter - 1` or `counter - 2`.
fn counter_offset<'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &Expression<'a>,
    counter: SymbolId,
    depth: u8,
) -> Option<i64> {
    if depth > 3 {
        return None;
    }
    let expression = strip(expression);
    if ix.symbol(expression) == Some(counter) {
        return Some(0);
    }
    if let Expression::BinaryExpression(binary) = expression {
        if binary.operator == BinaryOperator::Subtraction
            && ix.symbol(&binary.left) == Some(counter)
        {
            if let Expression::NumericLiteral(literal) = strip(&binary.right) {
                let value = literal.value;
                if value.fract() == 0.0 && (0.0..=2.0).contains(&value) {
                    return Some(value as i64);
                }
            }
        }
    }
    let origin = alias_origin(ix, expression)?;
    counter_offset(ix, origin, counter, depth + 1)
}

/// `(counter - a) / (counter - b)` with the numerator ahead of the denominator.
fn counter_ratio<'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &Expression<'a>,
    counter: SymbolId,
    depth: u8,
) -> Option<(i64, i64)> {
    if depth > 3 {
        return None;
    }
    if let Expression::BinaryExpression(binary) = strip(expression) {
        if binary.operator == BinaryOperator::Division {
            let numerator = counter_offset(ix, &binary.left, counter, depth + 1)?;
            let denominator = counter_offset(ix, &binary.right, counter, depth + 1)?;
            if numerator > denominator {
                return Some((numerator, denominator));
            }
        }
    }
    let origin = alias_origin(ix, expression)?;
    counter_ratio(ix, origin, counter, depth + 1)
}

/// `1 / (counter - offset)`.
fn inverse_counter<'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &Expression<'a>,
    counter: SymbolId,
    depth: u8,
) -> Option<i64> {
    if depth > 3 {
        return None;
    }
    if let Expression::BinaryExpression(binary) = strip(expression) {
        if binary.operator == BinaryOperator::Division {
            if let Expression::NumericLiteral(literal) = strip(&binary.left) {
                if literal.value == 1.0 {
                    if let Some(offset) = counter_offset(ix, &binary.right, counter, depth + 1) {
                        return Some(offset);
                    }
                }
            }
        }
    }
    let origin = alias_origin(ix, expression)?;
    inverse_counter(ix, origin, counter, depth + 1)
}

/// `state * ratio` where `state` is captured running state.
fn state_and_ratio<'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &Expression<'a>,
    counter: SymbolId,
    function: usize,
) -> Option<(SymbolId, (i64, i64))> {
    let Expression::BinaryExpression(binary) = strip(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::Multiplication {
        return None;
    }
    for (state, ratio) in [
        (&binary.left, &binary.right),
        (&binary.right, &binary.left),
    ] {
        let (Some(symbol), Some(ratio_value)) =
            (ix.symbol(state), counter_ratio(ix, ratio, counter, 0))
        else {
            continue;
        };
        if symbol != counter && ix.is_outside(symbol, function) {
            return Some((symbol, ratio_value));
        }
    }
    None
}

/// `term * (1 / denominator)`.
fn factor_and_increment<'e, 'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &'e Expression<'a>,
    counter: SymbolId,
) -> Option<(&'e Expression<'a>, i64)> {
    let Expression::BinaryExpression(binary) = strip(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::Multiplication {
        return None;
    }
    for (term, increment) in [
        (&binary.left, &binary.right),
        (&binary.right, &binary.left),
    ] {
        if let Some(offset) = inverse_counter(ix, increment, counter, 0) {
            return Some((term, offset));
        }
    }
    None
}

/// `state * ((n - a) / (n - b)) + term * (1 / (n - d))`.
fn orientation_formula<'e, 'c, 'a>(
    ix: &Ix<'c, 'a>,
    expression: &'e Expression<'a>,
    counter: SymbolId,
    event: SymbolId,
    function: usize,
) -> Option<OrientationFormula> {
    let Expression::BinaryExpression(binary) = strip(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::Addition {
        return None;
    }
    for (state_side, term_side) in [
        (&binary.left, &binary.right),
        (&binary.right, &binary.left),
    ] {
        let Some((state, ratio)) = state_and_ratio(ix, state_side, counter, function) else {
            continue;
        };
        let Some((term, divisor)) = factor_and_increment(ix, term_side, counter) else {
            continue;
        };
        let Some(term) = orientation_term(ix, term, event, function, 0) else {
            continue;
        };
        return Some(OrientationFormula {
            state,
            ratio,
            term,
            divisor,
        });
    }
    None
}

/// The exact host-listener option object `{passive:true,capture:true}`.
fn registration_options(options: Option<&Expression<'_>>) -> bool {
    let Some(Expression::ObjectExpression(object)) = options.map(|value| strip(value)) else {
        return false;
    };
    let mut passive = false;
    let mut capture = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        if property.computed
            || property.method
            || property.kind != PropertyKind::Init
            || !is_boolean(&property.value, true)
        {
            return false;
        }
        match property_key_name(&property.key).as_deref() {
            Some("passive") => passive = true,
            Some("capture") => capture = true,
            _ => {}
        }
    }
    passive && capture
}

#[derive(Clone)]
struct Reg {
    event: String,
    handler: Option<usize>,
    remove: bool,
    /// The function whose own body contains the registration call.
    owner: Option<usize>,
    /// The registration call's index in `Ix::own[owner].calls`.
    call: Option<usize>,
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

    /// A role names one thing. Candidates this module cannot tell apart would
    /// all receive the same name, so every claim of such a role is dropped.
    /// Independently proven source declarations (the nested running-moments
    /// helpers) deliberately do not go through this ambiguity drop: each one is
    /// its own direct `deviceorientation` registration, so the naming plan
    /// decides their concrete names in their own scopes.
    fn into_unique(self) -> Vec<FinderClaim> {
        let mut spans: BTreeMap<&'static str, usize> = BTreeMap::new();
        for (role, _, _) in &self.seen {
            *spans.entry(*role).or_default() += 1;
        }
        self.claims
            .into_iter()
            .filter(|claim| spans.get(claim.role()) == Some(&1))
            .collect()
    }
}

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let slots = ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes);
    let ix = Ix::new(ctx, &slots);
    let mut run = Run::default();
    run.run(&ix);
    let mut claims = run.out.into_unique();
    let mut seen: BTreeSet<(&'static str, u32, u32)> = claims
        .iter()
        .filter_map(|claim| match claim {
            FinderClaim::Function {
                role,
                function_span,
                ..
            } => Some((*role, function_span.start, function_span.end)),
            _ => None,
        })
        .collect();
    for index in &run.nested_orientation {
        let span = ix.span(*index);
        if !seen.insert((ORIENTATION_UPDATER, span.start, span.end)) {
            continue;
        }
        claims.push(FinderClaim::Function {
            role: ORIENTATION_UPDATER,
            function_span: span,
            alias_role: None,
            confidence: Confidence::Medium,
            evidence: vec![
                "nested function declaration owned by the arrow that directly registers it for \
                 deviceorientation with the passive/capture option pair; the owned body keeps the \
                 running mean and second moment of the inter-event clock delta and of the alpha, \
                 beta and gamma angles"
                    .into(),
            ],
        });
    }
    claims
}

#[derive(Default)]
struct Run {
    out: Out,
    regs: Vec<Reg>,
    claimed: BTreeSet<usize>,
    visibility: Option<usize>,
    wheel: Option<usize>,
    hold_down: Option<usize>,
    hold_up: Option<usize>,
    sample_down: Option<usize>,
    sample_up: Option<usize>,
    sample_press: Option<usize>,
    input: Option<usize>,
    orientation: Option<usize>,
    touch_start: Option<usize>,
    touch_move: Option<usize>,
    touch_end: Option<usize>,
    mouse_up: Option<usize>,
    mouse_down: Option<usize>,
    mouse_click: Option<usize>,
    mouse_move: Option<usize>,
    keyboard_recorder: Option<usize>,
    interval_tick: Option<usize>,
    interval_init: Option<usize>,
    digest_sender: Option<usize>,
    digest_then: Option<usize>,
    digest_catch: Option<usize>,
    throttle_factory: Option<usize>,
    throttle_wrapper: Option<usize>,
    throttle_release: Option<usize>,
    throttled_mouse: Option<usize>,
    visibility_group: Option<usize>,
    keystroke_group: Option<usize>,
    visibility_flush: Option<usize>,
    keyboard_flush: Option<usize>,
    keystroke_flush: Option<usize>,
    orientation_flush: Option<usize>,
    interval_flush: Option<usize>,
    mouse_flush: Option<usize>,
    mouse_predicate: Option<usize>,
    /// Source declarations proven as the nested running-moments helper by
    /// their own direct registration. They are independent of the
    /// collector-set `orientation` candidate and are emitted after the
    /// ambiguity drop.
    nested_orientation: Vec<usize>,
}

fn one<T: Copy>(candidates: &[T]) -> Option<T> {
    match candidates {
        [only] => Some(*only),
        _ => None,
    }
}

impl Run {
    fn run(&mut self, ix: &Ix<'_, '_>) {
        self.registrations(ix);
        self.classify_handlers(ix);
        self.keyboard_recorder(ix);
        self.groups(ix);
        self.interval(ix);
        self.digest(ix);
        self.throttle(ix);
        self.throttled_mouse_recorder(ix);
        self.flushes(ix);
        self.mouse_predicate(ix);
        self.emit_functions(ix);
        self.cleanups(ix);
        self.initializers(ix);
        self.bindings(ix);
    }

    fn registrations(&mut self, ix: &Ix<'_, '_>) {
        for function in 0..ix.fns.len() {
            for (call_index, call) in ix.own[function].calls.iter().enumerate() {
                let Some(name) = ix.call_name(call) else {
                    continue;
                };
                let remove = match name.as_str() {
                    "addEventListener" => false,
                    "removeEventListener" => true,
                    _ => continue,
                };
                let (_, arguments) = ix.invocation(call);
                let Some(event) = arguments.first().and_then(|value| string_value(value)) else {
                    continue;
                };
                let handler = arguments.get(1).and_then(|value| ix.resolve_fn(value));
                self.regs.push(Reg {
                    event: event.to_string(),
                    handler,
                    remove,
                    owner: Some(function),
                    call: Some(call_index),
                });
            }
        }
    }

    /// The orientation running-moments body emitted as an owned helper nested
    /// in the probe-initializer arrow. Each direct `deviceorientation`
    /// registration of such a helper is proven on its own: the registration
    /// must be owned by the helper's parent arrow, resolve its callback to the
    /// helper by the same scoped symbol, use a captured host receiver and the
    /// exact passive/capture options, and the helper body must keep the clock
    /// delta and angle means and second moments. The enclosing probe
    /// initializer and wave collector are containers and never candidates.
    fn nested_orientation_helpers(&self, ix: &Ix<'_, '_>) -> Vec<usize> {
        let mut found = Vec::new();
        for reg in &self.regs {
            if reg.remove || reg.event != "deviceorientation" {
                continue;
            }
            let (Some(owner), Some(call_index)) = (reg.owner, reg.call) else {
                continue;
            };
            let Some(call) = ix.own[owner].calls.get(call_index).copied() else {
                continue;
            };
            let Some(handler) = ix.registered_handler(call, owner) else {
                continue;
            };
            if !ix.is_arrow(owner) || ix.parent(handler) != Some(owner) {
                continue;
            }
            if !matches!(ix.fns[handler].func, FnNode::Function(_)) {
                continue;
            }
            if !ix.registration_is_direct(call, owner, handler) {
                continue;
            }
            if !ix.is_orientation_running_moments(handler) {
                continue;
            }
            if !found.contains(&handler) {
                found.push(handler);
            }
        }
        found
    }

    fn classify_handlers(&mut self, ix: &Ix<'_, '_>) {
        let mut visibility = Vec::new();
        let mut wheel = Vec::new();
        let mut hold_down = Vec::new();
        let mut hold_up = Vec::new();
        let mut sample_down = Vec::new();
        let mut sample_up = Vec::new();
        let mut sample_press = Vec::new();
        let mut input = Vec::new();
        let mut orientation = Vec::new();
        let mut touch_start = Vec::new();
        let mut touch_move = Vec::new();
        let mut touch_end = Vec::new();
        let mut mouse_up = Vec::new();
        let mut mouse_down = Vec::new();
        let mut mouse_click = Vec::new();
        let mut mouse_move = Vec::new();
        for reg in self.regs.clone() {
            if reg.remove {
                continue;
            }
            let Some(handler) = reg.handler else {
                continue;
            };
            let own = &ix.own[handler];
            let delegates = own
                .calls
                .iter()
                .any(|call| delegation(ix, handler, call).is_some());
            match reg.event.as_str() {
                "visibilitychange" if ix.put_ring(handler).is_some() => visibility.push(handler),
                "wheel" if ix.has_call_name(handler, "next") => wheel.push(handler),
                "keydown" if own.members.contains("keyCode") => hold_down.push(handler),
                "keyup" if own.members.contains("keyCode") => hold_up.push(handler),
                "keydown" if delegates => sample_down.push(handler),
                "keyup" if delegates => sample_up.push(handler),
                "keypress" if delegates => sample_press.push(handler),
                "input" if own.members.contains("tagName") && own.members.contains("inputType") => {
                    input.push(handler)
                }
                "deviceorientation"
                    if own.members.contains("alpha")
                        && own.members.contains("beta")
                        && own.members.contains("gamma") =>
                {
                    orientation.push(handler)
                }
                "touchstart" if ix.put_ring(handler).is_some() => touch_start.push(handler),
                "touchmove" if ix.put_ring(handler).is_some() => touch_move.push(handler),
                "touchend" if ix.put_ring(handler).is_some() => touch_end.push(handler),
                "mouseup" if ix.put_ring(handler).is_some() => mouse_up.push(handler),
                "mousedown" if ix.put_ring(handler).is_some() => mouse_down.push(handler),
                "click" if ix.put_ring(handler).is_some() => mouse_click.push(handler),
                "mousemove" if ix.put_ring(handler).is_some() => mouse_move.push(handler),
                _ => {}
            }
        }
        self.visibility = one(&visibility);
        self.wheel = one(&wheel);
        self.hold_down = one(&hold_down);
        let hold_up_recorder = one(&hold_up).or_else(|| self.repeated_keyup_recorder(ix));
        self.hold_up = hold_up_recorder;
        self.sample_down = one(&sample_down);
        self.sample_up = one(&sample_up);
        self.sample_press = one(&sample_press);
        self.input = one(&input);
        self.orientation = one(&orientation);
        self.touch_start = one(&touch_start);
        self.touch_move = one(&touch_move);
        self.touch_end = one(&touch_end);
        self.mouse_up = one(&mouse_up);
        self.mouse_down = one(&mouse_down);
        self.mouse_click = one(&mouse_click);
        self.mouse_move = one(&mouse_move);
        let nested = self.nested_orientation_helpers(ix);
        self.nested_orientation = nested;
    }

    /// The emitted loop/tail copies register the same `keyup` hold-statistics
    /// recorder more than once through the same single-origin listener alias on
    /// the same captured host, which `one` refuses as an ambiguous candidate.
    /// This bounded fallback collapses only those identical repeats: every
    /// keyup registration whose handler reads a `keyCode` index must be a plain
    /// `host.addEventListener("keyup", alias)` with no third argument, whose
    /// host is a captured binding outside the recorder, whose alias is a
    /// single-origin identifier resolving to the same recorder, and whose
    /// registering function is the same task. A second recorder, a foreign host
    /// or listener, a capture option, an overwritten alias or any other shape
    /// returns none, so no unrelated keyup handler is claimed.
    fn repeated_keyup_recorder(&self, ix: &Ix<'_, '_>) -> Option<usize> {
        let mut found: Option<(usize, SymbolId, SymbolId, usize)> = None;
        for reg in &self.regs {
            if reg.remove || reg.event != "keyup" {
                continue;
            }
            let Some(handler) = reg.handler else {
                continue;
            };
            if !ix.own[handler].members.contains("keyCode") {
                continue;
            }
            let (Some(owner), Some(call_index)) = (reg.owner, reg.call) else {
                return None;
            };
            let call = ix.own[owner].calls.get(call_index).copied()?;
            let Some((receiver, "addEventListener")) = ix.method(call) else {
                return None;
            };
            let arguments = args(call);
            if arguments.len() > 2 {
                return None;
            }
            let Some(host) = ix.symbol(receiver) else {
                return None;
            };
            if !ix.is_outside(host, handler) {
                return None;
            }
            let Some(listener) = arguments.get(1).and_then(|value| ix.symbol(value)) else {
                return None;
            };
            let Some(origin) = ix.single_origin(listener) else {
                return None;
            };
            if !matches!(strip(origin), Expression::Identifier(_))
                || ix.resolve_fn(origin) != Some(handler)
            {
                return None;
            }
            match found {
                Some((repeated, known_host, known_listener, known_owner))
                    if repeated == handler
                        && known_host == host
                        && known_listener == listener
                        && known_owner == owner => {}
                Some(_) => return None,
                None => found = Some((handler, host, listener, owner)),
            }
        }
        found.map(|(handler, _, _, _)| handler)
    }

    /// The common recorder the three keyboard sample handlers delegate to.
    fn keyboard_recorder(&mut self, ix: &Ix<'_, '_>) {
        let mut recorders = Vec::new();
        for handler in [self.sample_down, self.sample_up, self.sample_press]
            .into_iter()
            .flatten()
        {
            let mut found = None;
            for call in &ix.own[handler].calls {
                if let Some(target) = delegation(ix, handler, call) {
                    if ix.put_ring(target).is_some() {
                        found = Some(target);
                        break;
                    }
                }
            }
            let Some(found) = found else {
                return;
            };
            recorders.push(found);
        }
        if recorders.len() < 2 {
            return;
        }
        let first = recorders[0];
        if recorders.iter().all(|recorder| *recorder == first) {
            self.keyboard_recorder = Some(first);
        }
    }

    fn groups(&mut self, ix: &Ix<'_, '_>) {
        if let (Some(visibility), Some(wheel)) = (self.visibility, self.wheel) {
            let left = ix.group_above(visibility);
            let right = ix.group_above(wheel);
            if left.is_some() && left == right {
                self.visibility_group = left;
            }
        }
        if let (Some(down), Some(up)) = (self.hold_down, self.hold_up) {
            let left = ix.group_above(down);
            let right = ix.group_above(up);
            if left.is_some() && left == right {
                self.keystroke_group = left;
            }
        }
    }

    fn interval(&mut self, ix: &Ix<'_, '_>) {
        let mut ticks = Vec::new();
        let mut inits = Vec::new();
        for function in 0..ix.fns.len() {
            for call in &ix.own[function].calls {
                if ix.call_name(call).as_deref() != Some("setInterval") {
                    continue;
                }
                let Some(tick) = ix.first_fn_argument(call) else {
                    continue;
                };
                if ix.put_ring(tick).is_none() {
                    continue;
                }
                let subtracts = ix.own[tick]
                    .binaries
                    .iter()
                    .any(|binary| binary.operator == BinaryOperator::Subtraction);
                if !subtracts {
                    continue;
                }
                ticks.push(tick);
                inits.push(function);
            }
        }
        self.interval_tick = one(&ticks);
        self.interval_init = one(&inits);
    }

    fn digest(&mut self, ix: &Ix<'_, '_>) {
        let mut senders = Vec::new();
        for function in 0..ix.fns.len() {
            for call in &ix.own[function].calls {
                if ix.call_name(call).as_deref() != Some("setTimeout") {
                    continue;
                }
                let Some(callback) = ix.first_fn_argument(call) else {
                    continue;
                };
                let own = &ix.own[callback];
                // `…crypto.subtle.digest` is the stable host path; the root may
                // be the global itself or a captured global object.
                let mentions = own.members.contains("subtle") && own.members.contains("digest");
                let calls_digest = own
                    .calls
                    .iter()
                    .any(|call| ix.call_name(call).as_deref() == Some("digest"));
                let typeof_digest = own.unaries.iter().any(|unary| {
                    unary.operator == UnaryOperator::Typeof
                        && member(&unary.argument).is_some_and(|(_, name)| name == "digest")
                });
                if !mentions || calls_digest {
                    continue;
                }
                if typeof_digest {
                    senders.push(callback);
                }
            }
        }
        self.digest_sender = one(&senders);
        let Some(sender) = self.digest_sender else {
            return;
        };
        let thens: Vec<usize> = ix
            .call_named(sender, "then")
            .iter()
            .filter_map(|call| ix.first_fn_argument(call))
            .filter(|callback| ix.own[*callback].members.contains("replace"))
            .collect();
        let catches: Vec<usize> = ix
            .call_named(sender, "catch")
            .iter()
            .filter_map(|call| ix.first_fn_argument(call))
            .filter(|callback| ix.is_empty_body(*callback))
            .collect();
        self.digest_then = one(&thens);
        self.digest_catch = one(&catches);
    }

    fn throttle(&mut self, ix: &Ix<'_, '_>) {
        let mut candidates = Vec::new();
        for function in 0..ix.fns.len() {
            for returned in &ix.own[function].returns {
                let Some(wrapper) = ix.resolve_fn(returned) else {
                    continue;
                };
                if ix.parent(wrapper) != Some(function) {
                    continue;
                }
                let own = &ix.own[wrapper];
                if own.ifs.is_empty() {
                    continue;
                }
                let Some(timer) = own
                    .calls
                    .iter()
                    .filter(|call| ix.call_name(call).as_deref() == Some("setTimeout"))
                    .find_map(|call| ix.first_fn_argument(call))
                else {
                    continue;
                };
                if ix.parent(timer) != Some(wrapper) {
                    continue;
                }
                let latch = ix.false_writes(timer);
                if latch.is_empty() {
                    continue;
                }
                let sets_latch = own.assignments.iter().any(|assignment| {
                    assignment.operator == AssignmentOperator::Assign
                        && is_boolean(&assignment.right, true)
                        && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(identifier)
                            if analysis::identifier_symbol(identifier, ix.scoping)
                                .is_some_and(|symbol| latch.contains(&symbol)))
                });
                if !sets_latch {
                    continue;
                }
                let applies = own.returns.iter().any(|value| {
                    matches!(strip(value), Expression::CallExpression(call)
                    if ix.call_name(call).as_deref() == Some("apply")
                        && ix.method(call).is_some_and(|(receiver, _)| {
                            ix.slot_of(receiver, function) == Some(0)
                        }))
                });
                if !applies {
                    continue;
                }
                candidates.push((function, wrapper, timer));
            }
        }
        if let [(factory, wrapper, timer)] = candidates.as_slice() {
            self.throttle_factory = Some(*factory);
            self.throttle_wrapper = Some(*wrapper);
            self.throttle_release = Some(*timer);
        }
    }

    fn throttled_mouse_recorder(&mut self, ix: &Ix<'_, '_>) {
        let Some(factory) = self.throttle_factory else {
            return;
        };
        let mut recorders = Vec::new();
        for function in 0..ix.fns.len() {
            for call in &ix.own[function].calls {
                if ix.resolve_fn(&call.callee) != Some(factory) {
                    continue;
                }
                let Some(callback) = ix.first_fn_argument(call) else {
                    continue;
                };
                if ix.put_ring(callback).is_some() {
                    recorders.push(callback);
                }
            }
        }
        self.throttled_mouse = one(&recorders);
    }

    fn flushes(&mut self, ix: &Ix<'_, '_>) {
        let mut visibility = BTreeSet::new();
        if let Some(recorder) = self.visibility {
            if let Some(ring) = ix.put_ring(recorder) {
                visibility.insert(ring);
            }
        }
        let mut keyboard = BTreeSet::new();
        if let Some(recorder) = self.keyboard_recorder {
            if let Some(ring) = ix.put_ring(recorder) {
                keyboard.insert(ring);
            }
        }
        let mut mouse = BTreeSet::new();
        for recorder in [
            self.mouse_up,
            self.mouse_down,
            self.mouse_click,
            self.mouse_move,
            self.throttled_mouse,
        ]
        .into_iter()
        .flatten()
        {
            if let Some(ring) = ix.put_ring(recorder) {
                mouse.insert(ring);
            }
        }
        let mut interval = BTreeSet::new();
        if let Some(tick) = self.interval_tick {
            if let Some(ring) = ix.put_ring(tick) {
                interval.insert(ring);
            }
        }
        let mut orientation = BTreeSet::new();
        if let Some(updater) = self.orientation {
            orientation.extend(ix.written_symbols(updater));
        }
        let mut hold = BTreeSet::new();
        for recorder in [self.hold_down, self.hold_up].into_iter().flatten() {
            hold.extend(ix.written_symbols(recorder));
        }
        let kinds: [(&'static str, &BTreeSet<SymbolId>); 6] = [
            (VISIBILITY_WHEEL_LAST_CHANCE, &visibility),
            (KEYBOARD_LAST_CHANCE, &keyboard),
            (MOUSE_LAST_CHANCE, &mouse),
            (INTERVAL_LAST_CHANCE, &interval),
            (ORIENTATION_LAST_CHANCE, &orientation),
            (KEYSTROKE_LAST_CHANCE, &hold),
        ];
        for function in 0..ix.fns.len() {
            if !ix.passed_to_parent_slot(function) {
                continue;
            }
            let own = &ix.own[function];
            let false_written = ix.false_writes(function);
            if false_written.is_empty() || own.ifs.is_empty() {
                continue;
            }
            if !false_written
                .iter()
                .any(|symbol| own.reads.contains(symbol))
            {
                continue;
            }
            if !ix.has_call_name(function, "next") {
                continue;
            }
            let mut matched = Vec::new();
            for (role, symbols) in &kinds {
                if own.reads.iter().any(|symbol| symbols.contains(symbol)) {
                    matched.push(*role);
                }
            }
            let Some(role) = one(&matched) else {
                continue;
            };
            match role {
                VISIBILITY_WHEEL_LAST_CHANCE => self.visibility_flush = Some(function),
                KEYBOARD_LAST_CHANCE => self.keyboard_flush = Some(function),
                MOUSE_LAST_CHANCE => self.mouse_flush = Some(function),
                INTERVAL_LAST_CHANCE => self.interval_flush = Some(function),
                ORIENTATION_LAST_CHANCE => self.orientation_flush = Some(function),
                KEYSTROKE_LAST_CHANCE => self.keystroke_flush = Some(function),
                _ => {}
            }
        }
    }

    fn mouse_predicate(&mut self, ix: &Ix<'_, '_>) {
        let Some(flush) = self.mouse_flush else {
            return;
        };
        let mut predicates = Vec::new();
        for call in ix.call_named(flush, "filter") {
            if let Some(predicate) = ix.first_fn_argument(call) {
                let returns_comparison = ix.own[predicate].returns.iter().any(|value| {
                    matches!(strip(value), Expression::BinaryExpression(binary)
                        if binary.operator == BinaryOperator::LessThan)
                });
                if returns_comparison {
                    predicates.push(predicate);
                }
            }
        }
        self.mouse_predicate = one(&predicates);
    }

    fn emit_functions(&mut self, ix: &Ix<'_, '_>) {
        let claim = |run: &mut Run, role: &'static str, index: usize, evidence: &str| {
            run.out.function(ix, role, index, evidence);
            run.claimed.insert(index);
        };
        if let Some(index) = self.throttle_factory {
            claim(
                self,
                THROTTLE_FACTORY,
                index,
                "returns a latch-guarded wrapper that schedules the latch release with setTimeout and forwards fn.apply(this, arguments)",
            );
        }
        if let Some(index) = self.throttle_wrapper {
            claim(
                self,
                THROTTLED_WRAPPER,
                index,
                "returned wrapper: locked calls return undefined; otherwise locks, schedules the unlock and forwards receiver and arguments",
            );
        }
        if let Some(index) = self.throttle_release {
            claim(
                self,
                THROTTLE_RELEASE,
                index,
                "setTimeout body that clears the throttle latch",
            );
        }
        if let Some(index) = self.visibility_group {
            claim(
                self,
                VISIBILITY_WHEEL_COLLECTOR,
                index,
                "group coordinator whose nested init registers visibilitychange and wheel listeners and whose callbacks flush them",
            );
        }
        if let Some(index) = self.visibility {
            claim(
                self,
                VISIBILITY_RECORDER,
                index,
                "visibilitychange listener that puts {visible,timestamp} into the group ring",
            );
        }
        if let Some(index) = self.wheel {
            claim(
                self,
                WHEEL_HANDLER,
                index,
                "wheel listener that counts bursts and emits an encoded count record",
            );
        }
        if let Some(index) = self.visibility_flush {
            claim(
                self,
                VISIBILITY_WHEEL_LAST_CHANCE,
                index,
                "last-chance callback that reads the visibility ring and emits it when the group is dirty",
            );
        }
        if let Some(index) = self.keystroke_group {
            claim(
                self,
                KEYSTROKE_COLLECTOR,
                index,
                "group coordinator whose listeners keep per-key hold-time statistics and whose callbacks flush them",
            );
        }
        if let Some(index) = self.hold_down {
            claim(
                self,
                KEYDOWN_TIMING,
                index,
                "keydown listener that stores Date.now per keyCode and accumulates the inter-key gap",
            );
        }
        if let Some(index) = self.hold_up {
            claim(
                self,
                KEYUP_HOLD_STATS,
                index,
                "keyup listener that folds the hold duration into a running mean/second moment",
            );
        }
        if let Some(index) = self.keystroke_flush {
            claim(
                self,
                KEYSTROKE_LAST_CHANCE,
                index,
                "last-chance callback that emits the keystroke hold statistics when the group is dirty",
            );
        }
        if let Some(index) = self.keyboard_recorder {
            claim(
                self,
                KEYBOARD_RECORDER,
                index,
                "two-slot recorder that builds a phase/modifier record from the event and puts it into the ring",
            );
        }
        if let Some(index) = self.keyboard_flush {
            claim(
                self,
                KEYBOARD_LAST_CHANCE,
                index,
                "last-chance callback that deltas and emits the keyboard sample ring when the group is dirty",
            );
        }
        if let Some(index) = self.sample_down {
            claim(
                self,
                KEYDOWN_SAMPLE,
                index,
                "document keydown listener that forwards the event to the sample recorder as phase 1",
            );
        }
        if let Some(index) = self.sample_up {
            claim(
                self,
                KEYUP_SAMPLE,
                index,
                "document keyup listener that forwards the event to the sample recorder as phase 2",
            );
        }
        if let Some(index) = self.sample_press {
            claim(
                self,
                KEYPRESS_SAMPLE,
                index,
                "document keypress listener that forwards the event to the sample recorder as phase 3",
            );
        }
        if let Some(index) = self.digest_sender {
            claim(
                self,
                DIGEST_SENDER,
                index,
                "setTimeout callback that checks crypto.subtle.digest support, digests scrBytes and sends the telemetry request",
            );
        }
        if let Some(index) = self.digest_then {
            claim(
                self,
                DIGEST_THEN,
                index,
                "then handler that replaces the &c= query part with the digest hash and sends the request",
            );
        }
        if let Some(index) = self.digest_catch {
            claim(
                self,
                DIGEST_CATCH,
                index,
                "empty catch handler of the digest chain that swallows the rejection",
            );
        }
        if let Some(index) = self.orientation {
            claim(
                self,
                ORIENTATION_UPDATER,
                index,
                "deviceorientation listener that keeps running means of the angles and of the inter-event delta",
            );
        }
        if let Some(index) = self.orientation_flush {
            claim(
                self,
                ORIENTATION_LAST_CHANCE,
                index,
                "last-chance callback that emits the orientation running statistics",
            );
        }
        if let Some(index) = self.input {
            claim(
                self,
                INPUT_RECORDER,
                index,
                "document input listener that counts paste targets and off-active-element edits",
            );
        }
        for (index, role, evidence) in [
            (
                self.touch_start,
                TOUCH_START,
                "touchstart listener that puts {[true,false], elapsed, touches, UIEvent, trusted} into the ring",
            ),
            (
                self.touch_move,
                TOUCH_MOVE,
                "touchmove listener that puts {[false,true], elapsed, touches, UIEvent, trusted} into the ring",
            ),
            (
                self.touch_end,
                TOUCH_END,
                "touchend listener that puts {[false,false], elapsed, touches, UIEvent, trusted} into the ring",
            ),
        ] {
            if let Some(index) = index {
                claim(self, role, index, evidence);
            }
        }
        if let Some(index) = self.interval_init {
            claim(
                self,
                INTERVAL_INIT,
                index,
                "init callback that starts the setInterval tick through the captured host timer",
            );
        }
        if let Some(index) = self.interval_tick {
            claim(
                self,
                INTERVAL_TICK,
                index,
                "setInterval callback that puts the clock delta into the interval ring and updates min/max/count",
            );
        }
        if let Some(index) = self.interval_flush {
            claim(
                self,
                INTERVAL_LAST_CHANCE,
                index,
                "last-chance callback that emits min/mean/max/count of the interval ring",
            );
        }
        if let Some(index) = self.throttled_mouse {
            claim(
                self,
                THROTTLED_MOUSE,
                index,
                "callback wrapped by the throttle factory: puts the throttled mousemove sample into the ring",
            );
        }
        if let Some(index) = self.mouse_flush {
            claim(
                self,
                MOUSE_LAST_CHANCE,
                index,
                "last-chance callback that deltas and emits the mouse sample rings when the group is dirty",
            );
        }
        if let Some(index) = self.mouse_predicate {
            claim(
                self,
                MOUSE_PREDICATE,
                index,
                "filter callback of the mouse flush that keeps the moves before the first mouseup",
            );
        }
        for (index, role, evidence) in [
            (
                self.mouse_up,
                MOUSE_UP,
                "mouseup listener that puts {[true,false], ...} into the button ring",
            ),
            (
                self.mouse_down,
                MOUSE_DOWN,
                "mousedown listener that puts {[false,true], ...} into the button ring",
            ),
            (
                self.mouse_click,
                MOUSE_CLICK,
                "click listener that puts {[false,false], ...} into the button ring",
            ),
            (
                self.mouse_move,
                MOUSE_MOVE,
                "mousemove listener that puts the move record into the ring and forwards it to the throttled recorder",
            ),
        ] {
            if let Some(index) = index {
                claim(self, role, index, evidence);
            }
        }
    }

    fn cleanups(&mut self, ix: &Ix<'_, '_>) {
        const SETS: &[(&[&str], &str, &str)] = &[
            (
                &["visibilitychange", "wheel"],
                VISIBILITY_WHEEL_CLEANUP,
                "removes the visibilitychange and wheel listeners it registered",
            ),
            (
                &["keydown", "keyup"],
                KEYSTROKE_CLEANUP,
                "removes the keystroke keydown and keyup listeners without capture options",
            ),
            (
                &["keydown", "keyup", "keypress"],
                KEYBOARD_CLEANUP,
                "removes the keyboard sample keydown, keyup and keypress listeners with capture=true",
            ),
            (
                &["deviceorientation"],
                ORIENTATION_CLEANUP,
                "removes the deviceorientation listener",
            ),
            (&["input"], INPUT_CLEANUP, "removes the document input listener"),
            (
                &["touchstart", "touchmove", "touchend"],
                TOUCH_CLEANUP,
                "removes the three touch listeners with the same passive/capture options",
            ),
            (
                &["mousedown", "mouseup", "click", "mousemove"],
                MOUSE_CLEANUP,
                "removes the four mouse listeners with the same passive/capture options",
            ),
        ];
        for function in 0..ix.fns.len() {
            let mut events = BTreeSet::new();
            for call in &ix.own[function].calls {
                if ix.call_name(call).as_deref() != Some("removeEventListener") {
                    continue;
                }
                let (_, arguments) = ix.invocation(call);
                if let Some(event) = arguments.first().and_then(|value| string_value(value)) {
                    events.insert(event.to_string());
                }
            }
            if !events.is_empty() {
                for (set, role, evidence) in SETS {
                    let expected: BTreeSet<String> =
                        set.iter().map(|event| event.to_string()).collect();
                    if events == expected {
                        self.out.function(ix, role, function, evidence);
                        self.claimed.insert(function);
                    }
                }
            }
            if ix.has_call_name(function, "clearInterval") {
                let nulls = ix.own[function].assignments.iter().any(|assignment| {
                    assignment.operator == AssignmentOperator::Assign
                        && matches!(strip(&assignment.right), Expression::NullLiteral(_))
                });
                if nulls {
                    self.out.function(
                        ix,
                        INTERVAL_CLEANUP,
                        function,
                        "cleanup callback that clears the captured interval handle and nulls it",
                    );
                    self.claimed.insert(function);
                }
            }
        }
    }

    fn initializers(&mut self, ix: &Ix<'_, '_>) {
        let mut keystroke = Vec::new();
        let mut input = Vec::new();
        for function in 0..ix.fns.len() {
            let mut adds_keydown = false;
            let mut adds_keyup = false;
            let mut adds_input = false;
            for call in &ix.own[function].calls {
                if ix.call_name(call).as_deref() != Some("addEventListener") {
                    continue;
                }
                let (_, arguments) = ix.invocation(call);
                let Some(event) = arguments.first().and_then(|value| string_value(value)) else {
                    continue;
                };
                let handler = arguments.get(1).and_then(|value| ix.resolve_fn(value));
                match event {
                    "keydown" if handler.is_some() && handler == self.hold_down => {
                        adds_keydown = true
                    }
                    "keyup" if handler.is_some() && handler == self.hold_up => adds_keyup = true,
                    "input" if handler.is_some() && handler == self.input => adds_input = true,
                    _ => {}
                }
            }
            if adds_keydown && adds_keyup {
                keystroke.push(function);
            }
            if adds_input {
                input.push(function);
            }
        }
        if let Some(function) = one(&keystroke) {
            self.out.function(
                ix,
                KEYSTROKE_INIT,
                function,
                "init callback that installs the hold-statistics keydown and keyup listeners",
            );
            self.claimed.insert(function);
        }
        if let Some(function) = one(&input) {
            self.out.function(
                ix,
                INPUT_INIT,
                function,
                "init callback that stores the input handler and installs the document input listener",
            );
            self.claimed.insert(function);
        }
    }

    fn readers_claimed(&self, ix: &Ix<'_, '_>, symbol: SymbolId) -> bool {
        ix.readers(symbol)
            .iter()
            .any(|function| self.claimed.contains(function))
    }

    fn bindings(&mut self, ix: &Ix<'_, '_>) {
        self.bind_ring_alias(ix);
        self.bind_has_own_alias(ix);
        self.bind_elapsed(ix);
        self.bind_seed_wave(ix);
        self.bind_seed_date(ix);
        self.bind_flagged_keys(ix);
    }

    fn bind_ring_alias(&mut self, ix: &Ix<'_, '_>) {
        for symbol in ix.scoping.symbol_ids().collect::<Vec<_>>() {
            let AstKind::VariableDeclarator(declarator) =
                ix.nodes.kind(ix.scoping.symbol_declaration(symbol))
            else {
                continue;
            };
            let Some(init) = declarator.init.as_ref() else {
                continue;
            };
            if ix.single_origin(symbol).is_none() {
                continue;
            }
            let Expression::Identifier(identifier) = strip(init) else {
                continue;
            };
            let Some(target) = analysis::identifier_symbol(identifier, ix.scoping) else {
                continue;
            };
            if !ix.is_ring_buffer_class(target) || !self.readers_claimed(ix, symbol) {
                continue;
            }
            self.out.binding(
                ix,
                RING_ALIAS,
                symbol,
                "main alias of the ring buffer whose put/toArray/clear methods the collectors use",
            );
        }
    }

    fn bind_has_own_alias(&mut self, ix: &Ix<'_, '_>) {
        for symbol in ix.scoping.symbol_ids().collect::<Vec<_>>() {
            let AstKind::VariableDeclarator(declarator) =
                ix.nodes.kind(ix.scoping.symbol_declaration(symbol))
            else {
                continue;
            };
            let Some(init) = declarator.init.as_ref() else {
                continue;
            };
            if ix.single_origin(symbol).is_none() {
                continue;
            }
            let Expression::Identifier(identifier) = strip(init) else {
                continue;
            };
            let Some(target) = analysis::identifier_symbol(identifier, ix.scoping) else {
                continue;
            };
            let Some(target_function) = ix.decl_fn.get(&target).copied() else {
                continue;
            };
            if !ix.is_has_own_helper(target_function) || !self.readers_claimed(ix, symbol) {
                continue;
            }
            self.out.binding(
                ix,
                HAS_OWN_ALIAS,
                symbol,
                "main alias of the hasOwnProperty.call helper the collectors use to test record fields",
            );
        }
    }

    fn bind_elapsed(&mut self, ix: &Ix<'_, '_>) {
        let wheel = self.wheel;
        let orientation = self.orientation;
        for symbol in ix.scoping.symbol_ids().collect::<Vec<_>>() {
            let AstKind::VariableDeclarator(declarator) =
                ix.nodes.kind(ix.scoping.symbol_declaration(symbol))
            else {
                continue;
            };
            let Some(init) = declarator.init.as_ref() else {
                continue;
            };
            let Expression::CallExpression(call) = strip(init) else {
                continue;
            };
            if ix.write_count(symbol) != 0 {
                continue;
            }
            let Some(factory) = ix.resolve_fn(&call.callee) else {
                continue;
            };
            if !ix.is_elapsed_factory(factory) {
                continue;
            }
            let readers = ix.readers(symbol);
            if readers.len() != 1 {
                continue;
            }
            let reader = *readers.iter().next().unwrap();
            if Some(reader) == wheel {
                self.out.binding(
                    ix,
                    ELAPSED_SINCE_INIT,
                    symbol,
                    "instance of the proven elapsed-clock factory read only by the wheel handler",
                );
            } else if Some(reader) == orientation {
                self.out.binding(
                    ix,
                    ELAPSED_CLOCK,
                    symbol,
                    "instance of the proven elapsed-clock factory read only by the orientation running-stats updater",
                );
            }
        }
    }

    fn bind_seed_wave(&mut self, ix: &Ix<'_, '_>) {
        // The random source: the binding that holds `Math.random`.
        let mut random = Vec::new();
        for symbol in ix.scoping.symbol_ids().collect::<Vec<_>>() {
            let Some(init) = ix.initializer(symbol) else {
                continue;
            };
            if ix.path(init).as_deref() != Some("Math.random") || ix.write_count(symbol) != 0 {
                continue;
            }
            if self.readers_claimed(ix, symbol) {
                random.push(symbol);
            }
        }
        let Some(random) = one(&random) else {
            return;
        };

        // The counter: a numeric initializer, incremented, added to the
        // random draw in a claimed collector.
        let mut counters = Vec::new();
        for symbol in ix.scoping.symbol_ids().collect::<Vec<_>>() {
            let Some(init) = ix.initializer(symbol) else {
                continue;
            };
            if !matches!(strip(init), Expression::NumericLiteral(_)) || ix.write_count(symbol) == 0
            {
                continue;
            }
            if !self.readers_claimed(ix, symbol) {
                continue;
            }
            let incremented = ix.own.iter().any(|own| {
                own.updates.iter().any(|update| {
                    matches!(&update.argument,
                        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier)
                            if analysis::identifier_symbol(identifier, ix.scoping) == Some(symbol))
                })
            });
            if !incremented {
                continue;
            }
            let added_to_random = ix.fns.iter().enumerate().any(|(function, _)| {
                self.claimed.contains(&function)
                    && ix.own[function].binaries.iter().any(|binary| {
                        binary.operator == BinaryOperator::Addition
                            && ((ix.symbol(&binary.left) == Some(symbol)
                                && random_draw(ix, &binary.right) == Some(random))
                                || (ix.symbol(&binary.right) == Some(symbol)
                                    && random_draw(ix, &binary.left) == Some(random)))
                    })
            });
            if added_to_random {
                counters.push(symbol);
            }
        }
        let Some(counter) = one(&counters) else {
            return;
        };

        // The queue: an empty array literal the collector pushes to and
        // shifts from alongside the counter.
        let mut queues = Vec::new();
        for symbol in ix.scoping.symbol_ids().collect::<Vec<_>>() {
            let Some(init) = ix.initializer(symbol) else {
                continue;
            };
            let Expression::ArrayExpression(array) = strip(init) else {
                continue;
            };
            if !array.elements.is_empty() || !self.readers_claimed(ix, symbol) {
                continue;
            }
            let used_as_queue = ix.fns.iter().enumerate().any(|(function, _)| {
                self.claimed.contains(&function)
                    && ix.own[function].reads.contains(&counter)
                    && ix.own[function].calls.iter().any(|call| {
                        ix.call_name(call).as_deref() == Some("push")
                            && ix.method(call).is_some_and(|(receiver, _)| {
                                ix.resolve_symbol(receiver) == Some(symbol)
                            })
                    })
                    && ix.own[function].calls.iter().any(|call| {
                        ix.call_name(call).as_deref() == Some("shift")
                            && ix.method(call).is_some_and(|(receiver, _)| {
                                ix.resolve_symbol(receiver) == Some(symbol)
                            })
                    })
            });
            if used_as_queue {
                queues.push(symbol);
            }
        }
        let Some(queue) = one(&queues) else {
            return;
        };

        self.out.binding(
            ix,
            RANDOM_SOURCE,
            random,
            "holds Math.random and is called inside the seed-wave refill of the collectors",
        );
        self.out.binding(
            ix,
            SEED_COUNTER,
            counter,
            "incremented numeric seed-wave counter added to the random draw before it is queued",
        );
        self.out.binding(
            ix,
            SEED_QUEUE,
            queue,
            "empty array seed-wave queue pushed to and shifted by the collectors",
        );

        // The optional crypto/performance entropy sources guard a host global
        // with `typeof`.
        for (name, role, evidence) in [
            (
                "crypto",
                CRYPTO_SOURCE,
                "holds crypto behind a typeof guard and reads getRandomValues in the seed-wave refill",
            ),
            (
                "performance",
                PERFORMANCE_SOURCE,
                "holds performance behind a typeof guard and reads now in the seed-wave refill",
            ),
        ] {
            let mut candidates = Vec::new();
            for symbol in ix.scoping.symbol_ids().collect::<Vec<_>>() {
                let Some(init) = ix.initializer(symbol) else {
                    continue;
                };
                if ix.write_count(symbol) != 0 || !is_global_guard(init, name) {
                    continue;
                }
                if self.readers_claimed(ix, symbol) {
                    candidates.push(symbol);
                }
            }
            if let Some(symbol) = one(&candidates) {
                self.out.binding(ix, role, symbol, evidence);
            }
        }
    }

    fn bind_seed_date(&mut self, ix: &Ix<'_, '_>) {
        // Identifier spans that are the argument of `new Date(...)`.
        let mut date_arguments: BTreeMap<SymbolId, BTreeSet<(u32, u32)>> = BTreeMap::new();
        for node in ix.nodes.iter() {
            let AstKind::NewExpression(new) = node.kind() else {
                continue;
            };
            if ix.path(&new.callee).as_deref() != Some("Date") {
                continue;
            }
            let arguments = new
                .arguments
                .iter()
                .filter_map(Argument::as_expression)
                .collect::<Vec<_>>();
            let [only] = arguments.as_slice() else {
                continue;
            };
            let Expression::Identifier(identifier) = strip(only) else {
                continue;
            };
            let Some(symbol) = analysis::identifier_symbol(identifier, ix.scoping) else {
                continue;
            };
            date_arguments
                .entry(symbol)
                .or_default()
                .insert((identifier.span.start, identifier.span.end));
        }
        for (symbol, allowed) in date_arguments {
            let Some(init) = ix.initializer(symbol) else {
                continue;
            };
            let numeric = matches!(strip(init), Expression::NumericLiteral(_))
                || matches!(strip(init), Expression::UnaryExpression(unary)
                    if unary.operator == UnaryOperator::UnaryNegation
                        && matches!(strip(&unary.argument), Expression::NumericLiteral(_)));
            if !numeric || ix.write_count(symbol) != 0 {
                continue;
            }
            if !self.readers_claimed(ix, symbol) {
                continue;
            }
            let all_date = ix.scoping.get_resolved_references(symbol).all(|reference| {
                let span = ix.nodes.get_node(reference.node_id()).span();
                reference.is_read() && allowed.contains(&(span.start, span.end))
            });
            if !all_date {
                continue;
            }
            self.out.binding(
                ix,
                SEED_DATE,
                symbol,
                "numeric timestamp read only as the argument of the seeded Date in the keystroke probe",
            );
        }
    }

    fn bind_flagged_keys(&mut self, ix: &Ix<'_, '_>) {
        for symbol in ix.scoping.symbol_ids().collect::<Vec<_>>() {
            let AstKind::VariableDeclarator(declarator) =
                ix.nodes.kind(ix.scoping.symbol_declaration(symbol))
            else {
                continue;
            };
            if declarator.init.is_some() || ix.write_count(symbol) != 1 {
                continue;
            }
            let Some(assignments) = ix.assigns.get(&symbol) else {
                continue;
            };
            let [assignment] = assignments.as_slice() else {
                continue;
            };
            let Expression::ArrayExpression(array) = strip(&assignment.right) else {
                continue;
            };
            if array.elements.is_empty()
                || !array.elements.iter().all(|element| {
                    element
                        .as_expression()
                        .is_some_and(|value| matches!(strip(value), Expression::StringLiteral(_)))
                })
            {
                continue;
            }
            if !self.readers_claimed(ix, symbol) {
                continue;
            }
            let indexed = ix.fns.iter().enumerate().any(|(function, _)| {
                self.claimed.contains(&function)
                    && ix.own[function].calls.iter().any(|call| {
                        ix.call_name(call).as_deref() == Some("indexOf")
                            && ix.method(call).is_some_and(|(receiver, _)| {
                                ix.resolve_symbol(receiver) == Some(symbol)
                            })
                    })
            });
            if !indexed {
                continue;
            }
            self.out.binding(
                ix,
                FLAGGED_KEYS,
                symbol,
                "string array assigned once and looked up by indexOf against the event key name",
            );
        }
    }
}

/// The delegated recorder of a keyboard sample handler: a two-argument call
/// whose second argument is the handler's event slot.
fn delegation(ix: &Ix<'_, '_>, handler: usize, call: &CallExpression<'_>) -> Option<usize> {
    let (callee, arguments) = ix.invocation(call);
    if arguments.len() < 2 {
        return None;
    }
    let target = ix.resolve_fn(callee)?;
    let event_passed = arguments
        .iter()
        .any(|argument| ix.slot_of(argument, handler).is_some());
    event_passed.then_some(target)
}

/// `x()` where `x` is the random-source binding.
fn random_draw(ix: &Ix<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    let Expression::CallExpression(call) = strip(expression) else {
        return None;
    };
    let Expression::Identifier(identifier) = strip(&call.callee) else {
        return None;
    };
    analysis::identifier_symbol(identifier, ix.scoping)
}

fn is_global_guard(expression: &Expression<'_>, name: &str) -> bool {
    let Expression::ConditionalExpression(conditional) = strip(expression) else {
        return false;
    };
    let Expression::BinaryExpression(test) = strip(&conditional.test) else {
        return false;
    };
    if test.operator != BinaryOperator::StrictInequality
        && test.operator != BinaryOperator::Inequality
    {
        return false;
    }
    let typeof_name = |value: &Expression<'_>| {
        matches!(strip(value), Expression::UnaryExpression(unary)
            if unary.operator == UnaryOperator::Typeof
                && matches!(strip(&unary.argument), Expression::Identifier(identifier) if identifier.name == name))
    };
    let undefined = |value: &Expression<'_>| matches!(strip(value), Expression::StringLiteral(literal) if literal.value == "undefined");
    let (left, right) = (&test.left, &test.right);
    let guarded =
        (typeof_name(left) && undefined(right)) || (typeof_name(right) && undefined(left));
    guarded
        && matches!(strip(&conditional.consequent), Expression::Identifier(identifier) if identifier.name == name)
        && matches!(strip(&conditional.alternate), Expression::NullLiteral(_))
}
