//! Reusable, structure-proven purpose finders for recovered program helpers.
//!
//! The finders here recognize small, recurring program utilities by what their
//! own body does and by how their result is consumed: the callback registry and
//! its notify/register methods, the three task-queue implementations and the
//! task runners handed to them, the asynchronous signed 32-bit rolling hashes,
//! the getter/setter/method wrappers installed by the interceptors, and the
//! once-only callback wrappers.
//!
//! # Contract
//!
//! [`find_purposes`] returns one [`PurposeOutcome`] per proven function, per
//! equal-evidence tie and per role that nothing matched. A finding addresses
//! the `Function`/`ArrowFunctionExpression` node by its exact span; it does not
//! rename anything. The naming stream resolves that span to an editable binding
//! through its own fail-closed proofs. [`find_program_purposes`] is the public,
//! serializable report of the same run for tests and evaluation.
//!
//! # How a finder decides
//!
//! * Every predicate is about the function's *own* body (nested functions are
//!   separate units) or about a value that the function owns: its returned
//!   object, its own locals, its formal slots and their direct consumers.
//! * A function's Nth formal and `arguments[N]` are the same slot, proven by
//!   the shared [`super::bindings::ParameterSlots`] resolver the scanner also
//!   uses. A formal counts only when it is a plain, never-written identifier
//!   whose owner never lets its `arguments` object escape; `arguments` counts
//!   only when unresolved, owned by the nearest non-arrow function, and never
//!   written there. A local whose single origin is one of those reads is an
//!   alias of the slot.
//! * Stable public property names of an API (`notify`, `start`, `get`, ...) may
//!   link a method to its owner, but each method is still proven by behavior.
//!   Generated spellings, offsets, numeric build constants, and saved catalog
//!   proposals are never read.
//! * A finding is refused when the function's own binding is redeclared or
//!   reassigned, or when the owned object that carries the proof is.
//! * Two proven functions that would receive the same name in one scope are
//!   reported as [`PurposeOutcome::Ambiguous`] instead of picking one. Parallel
//!   copies in different scopes are legitimate and each gets its own finding.
//!
//! The existing `semantic_names` roles are reused rather than duplicated:
//! the method/accessor interceptor installers are recognized here only as
//! anchors for their installed wrappers, and the synchronous string/buffer hash
//! (`string_buffer_hash`) and the PRNG byte source are not reported again.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, ScopeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator, UpdateOperator};
use serde::{Deserialize, Serialize};

use crate::analysis;

use crate::signals::extract::bindings::{numeric_index, object_members_of, property_key_name, ParameterSlots};
use crate::signals::schema::{Confidence, SourceSpan};

// ===========================================================================
// Contract
// ===========================================================================

/// One proven purpose of one function node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PurposeFinding {
    /// Stable machine label of the role, e.g. `callback_registry_factory`.
    pub role: &'static str,
    /// The descriptive name for the function's own binding.
    pub action_name: &'static str,
    /// The `Function` or `ArrowFunctionExpression` node span.
    pub function_span: Span,
    /// The descriptive name for the function's single holder alias, when it has
    /// exactly one.
    pub holder_alias_name: Option<&'static str>,
    pub confidence: Confidence,
    /// The structural clues that established the role.
    pub evidence: Vec<String>,
}

/// The result of one finder for one role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PurposeOutcome {
    Found(PurposeFinding),
    /// Several functions matched with equal evidence, or would receive the same
    /// name in one scope. None of them is named.
    Ambiguous {
        role: &'static str,
        candidates: Vec<Span>,
        reason: String,
    },
    NotFound {
        role: &'static str,
    },
}

/// Status of one serialized report entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PurposeStatus {
    Found,
    Ambiguous,
    NotFound,
    /// The structure matched but a binding the proof depends on is redeclared
    /// or reassigned, so no name may be derived from it.
    Refused,
}

/// One serializable entry of [`find_program_purposes`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurposeReportEntry {
    pub role: String,
    pub status: PurposeStatus,
    /// The descriptive function name, for `found` entries.
    pub action_name: Option<String>,
    /// The descriptive holder-alias name, for `found` entries with one alias.
    pub holder_alias_name: Option<String>,
    /// The function node span(s): one for `found`/`refused`, every tied
    /// candidate for `ambiguous`, none for `not_found`.
    pub function_spans: Vec<SourceSpan>,
    pub confidence: Option<Confidence>,
    pub evidence: Vec<String>,
    /// Why an `ambiguous` or `refused` entry was not named.
    pub reason: Option<String>,
}

/// Run every purpose finder over exact program source.
pub fn find_program_purposes(source: &str) -> Result<Vec<PurposeReportEntry>, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let run = run_finders(&program, &scoping, &nodes);
    let mut entries: Vec<PurposeReportEntry> = run
        .outcomes
        .into_iter()
        .map(|outcome| match outcome {
            PurposeOutcome::Found(finding) => PurposeReportEntry {
                role: finding.role.to_string(),
                status: PurposeStatus::Found,
                action_name: Some(finding.action_name.to_string()),
                holder_alias_name: finding.holder_alias_name.map(str::to_string),
                function_spans: vec![SourceSpan::from(finding.function_span)],
                confidence: Some(finding.confidence),
                evidence: finding.evidence,
                reason: None,
            },
            PurposeOutcome::Ambiguous {
                role,
                candidates,
                reason,
            } => PurposeReportEntry {
                role: role.to_string(),
                status: PurposeStatus::Ambiguous,
                action_name: None,
                holder_alias_name: None,
                function_spans: candidates.into_iter().map(SourceSpan::from).collect(),
                confidence: None,
                evidence: Vec::new(),
                reason: Some(reason),
            },
            PurposeOutcome::NotFound { role } => PurposeReportEntry {
                role: role.to_string(),
                status: PurposeStatus::NotFound,
                action_name: None,
                holder_alias_name: None,
                function_spans: Vec::new(),
                confidence: None,
                evidence: Vec::new(),
                reason: None,
            },
        })
        .collect();
    for refusal in run.refusals {
        entries.push(PurposeReportEntry {
            role: refusal.role.to_string(),
            status: PurposeStatus::Refused,
            action_name: None,
            holder_alias_name: None,
            function_spans: vec![SourceSpan::from(refusal.span)],
            confidence: None,
            evidence: Vec::new(),
            reason: Some(refusal.reason),
        });
    }
    Ok(entries)
}

/// Run every purpose finder over one parsed and analyzed program.
///
/// This is the entry the unified naming plan consumes; it is not called from
/// this module.
pub(crate) fn find_purposes(
    _source: &str,
    program: &Program<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> Vec<PurposeOutcome> {
    run_finders(program, scoping, nodes).outcomes
}

// ===========================================================================
// Roles
// ===========================================================================

/// Every role this module reports with its action name and optional holder
/// alias name. The unified naming plan treats these as part of its single
/// vocabulary: `<role>` gets the action name and `<role>_alias` the alias name.
pub(crate) fn purpose_vocabulary(
) -> impl Iterator<Item = (&'static str, &'static str, Option<&'static str>)> {
    ROLES
        .iter()
        .map(|spec| (spec.role, spec.action, spec.alias))
}

struct RoleSpec {
    role: &'static str,
    action: &'static str,
    alias: Option<&'static str>,
}

const fn role(role: &'static str, action: &'static str, alias: Option<&'static str>) -> RoleSpec {
    RoleSpec {
        role,
        action,
        alias,
    }
}

const CALLBACK_REGISTRY_FACTORY: &str = "callback_registry_factory";
const CALLBACK_REGISTRY_NOTIFY: &str = "callback_registry_notify";
const CALLBACK_REGISTRY_REGISTER: &str = "callback_registry_register";
const SEQUENTIAL_QUEUE_FACTORY: &str = "sequential_task_queue_factory";
const SCHEDULED_QUEUE_FACTORY: &str = "scheduled_task_queue_factory";
const TIME_BUDGETED_QUEUE_FACTORY: &str = "time_budgeted_task_queue_factory";
const QUEUE_START: &str = "task_queue_start";
const QUEUE_ENQUEUE: &str = "task_queue_enqueue";
const QUEUE_EMPTY: &str = "task_queue_empty";
const QUEUE_ON_EMPTY: &str = "task_queue_on_empty";
const QUEUE_ON_ERROR: &str = "task_queue_on_error";
const SEQUENTIAL_QUEUE_RUNNER: &str = "sequential_task_queue_runner";
const SCHEDULED_QUEUE_RUNNER: &str = "scheduled_task_queue_runner";
const TYPED_ARRAY_HASH_ASYNC: &str = "typed_array_hash_async";
const TYPED_ARRAY_HASH_BATCH: &str = "typed_array_hash_batch_step";
const STRING_HASH_ASYNC: &str = "string_hash_async";
const STRING_HASH_BATCH: &str = "string_hash_batch_step";
const INTERCEPTED_GETTER: &str = "intercepted_getter";
const INTERCEPTED_SETTER: &str = "intercepted_setter";
const INTERCEPTED_METHOD: &str = "intercepted_method";
const ONCE_FACTORY: &str = "once_callback_factory";
const ONCE_INVOKE: &str = "once_guarded_invoke";

/// Every role this module reports, in report order. A role nothing matched is
/// reported as `NotFound`.
const ROLES: &[RoleSpec] = &[
    role(
        CALLBACK_REGISTRY_FACTORY,
        "createCallbackRegistry",
        Some("callbackRegistryFactory"),
    ),
    role(CALLBACK_REGISTRY_NOTIFY, "notifyCallbacks", None),
    role(CALLBACK_REGISTRY_REGISTER, "registerCallback", None),
    role(
        SEQUENTIAL_QUEUE_FACTORY,
        "createSequentialTaskQueue",
        Some("sequentialTaskQueueFactory"),
    ),
    role(
        SCHEDULED_QUEUE_FACTORY,
        "createScheduledTaskQueue",
        Some("scheduledTaskQueueFactory"),
    ),
    role(
        TIME_BUDGETED_QUEUE_FACTORY,
        "createTimeBudgetedTaskQueue",
        Some("timeBudgetedTaskQueueFactory"),
    ),
    role(QUEUE_START, "startTaskQueue", None),
    role(QUEUE_ENQUEUE, "enqueueTasks", None),
    role(QUEUE_EMPTY, "discardPendingTasks", None),
    role(QUEUE_ON_EMPTY, "onTaskQueueEmpty", None),
    role(QUEUE_ON_ERROR, "onTaskQueueError", None),
    role(
        SEQUENTIAL_QUEUE_RUNNER,
        "runSequentialQueueTask",
        Some("sequentialQueueTaskRunner"),
    ),
    role(
        SCHEDULED_QUEUE_RUNNER,
        "runScheduledQueueTask",
        Some("scheduledQueueTaskRunner"),
    ),
    role(
        TYPED_ARRAY_HASH_ASYNC,
        "hashArrayBufferViewAsync",
        Some("arrayBufferViewHashAsync"),
    ),
    role(TYPED_ARRAY_HASH_BATCH, "hashTypedArrayWordBatch", None),
    role(
        STRING_HASH_ASYNC,
        "hashStringAsync",
        Some("stringHashAsync"),
    ),
    role(STRING_HASH_BATCH, "hashStringCharBatch", None),
    role(INTERCEPTED_GETTER, "instrumentedPropertyGetter", None),
    role(INTERCEPTED_SETTER, "instrumentedPropertySetter", None),
    role(INTERCEPTED_METHOD, "instrumentedMethodWrapper", None),
    role(
        ONCE_FACTORY,
        "createOnceCallback",
        Some("onceCallbackFactory"),
    ),
    role(ONCE_INVOKE, "invokeOnce", None),
];

fn spec(role: &str) -> &'static RoleSpec {
    ROLES
        .iter()
        .find(|spec| spec.role == role)
        .expect("every candidate role is listed in ROLES")
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

struct FnEntry<'a> {
    node: NodeId,
    span: Span,
    func: FnNode<'a>,
    parent: Option<usize>,
}

impl<'a> FnEntry<'a> {
    fn is_arrow(&self) -> bool {
        matches!(self.func, FnNode::Arrow(_))
    }

    fn params(&self) -> &'a FormalParameters<'a> {
        match self.func {
            FnNode::Function(function) => &function.params,
            FnNode::Arrow(arrow) => &arrow.params,
        }
    }

    fn statements(&self) -> &'a [Statement<'a>] {
        match self.func {
            FnNode::Function(function) => function
                .body
                .as_ref()
                .map(|body| body.statements.as_slice())
                .unwrap_or(&[]),
            FnNode::Arrow(arrow) => arrow.body.statements.as_slice(),
        }
    }

    fn formal_count(&self) -> usize {
        self.params().items.len()
    }
}

impl<'a> Own<'a> {
    /// True when the function reads its slot `slot` as a formal or through
    /// `arguments`.
    fn reads_slot(&self, entry: &FnEntry<'a>, slot: usize) -> bool {
        entry.formal_count() > slot || self.argument_reads.contains(&slot)
    }
}

/// The nodes a function owns directly: everything in its parameters and body
/// except what belongs to a nested function.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    returns: Vec<&'a ReturnStatement<'a>>,
    updates: Vec<&'a UpdateExpression<'a>>,
    declarators: Vec<&'a VariableDeclarator<'a>>,
    ifs: Vec<&'a IfStatement<'a>>,
    tries: Vec<&'a TryStatement<'a>>,
    loops: usize,
    /// `object.name` / `object["name"]` reads and writes.
    members: Vec<(&'a Expression<'a>, &'a str)>,
    /// Object-literal keys and member-assignment target names.
    written_keys: BTreeSet<String>,
    /// `arguments[N]` indices read where this (non-arrow) function owns
    /// `arguments`, including from nested arrows.
    argument_reads: BTreeSet<usize>,
    nested: Vec<usize>,
}

/// A source value of a binding: its declarator initializer or one plain
/// assignment, with the node and function that own it.
#[derive(Clone, Copy)]
struct Origin<'a> {
    value: &'a Expression<'a>,
    node: NodeId,
    owner: Option<usize>,
}

struct Refusal {
    role: &'static str,
    span: Span,
    reason: String,
}

struct Candidate {
    role: &'static str,
    function: usize,
    confidence: Confidence,
    evidence: Vec<String>,
}

#[derive(Default)]
struct Run {
    outcomes: Vec<PurposeOutcome>,
    refusals: Vec<Refusal>,
}

struct Ctx<'s, 'a> {
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    fns: Vec<FnEntry<'a>>,
    own: Vec<Own<'a>>,
    /// Owning function index of every node, by node index.
    owner: Vec<u32>,
    /// Function index of every function node, by node index.
    fn_of_node: Vec<u32>,
    /// Function-declaration symbols (and named function-expression ids).
    decl_fn: BTreeMap<SymbolId, usize>,
    /// Plain `symbol = value` writes, by target symbol.
    assignments: BTreeMap<SymbolId, Vec<&'a AssignmentExpression<'a>>>,
    /// `lhs = <identifier rhs>` origins, by right-hand symbol.
    identifier_origins: BTreeMap<SymbolId, BTreeSet<SymbolId>>,
    /// The shared parameter-slot resolver (formal N and `arguments[N]`).
    slots: ParameterSlots<'s, 'a>,
}

impl<'s, 'a> Ctx<'s, 'a> {
    fn new(program: &Program<'_>, scoping: &'s Scoping, nodes: &'s AstNodes<'a>) -> Self {
        let count = nodes.len();
        let mut ctx = Self {
            scoping,
            nodes,
            fns: Vec::new(),
            own: Vec::new(),
            owner: vec![NONE; count],
            fn_of_node: vec![NONE; count],
            decl_fn: BTreeMap::new(),
            assignments: BTreeMap::new(),
            identifier_origins: BTreeMap::new(),
            slots: ParameterSlots::new(program, scoping, nodes),
        };
        ctx.index();
        ctx
    }

    fn index(&mut self) {
        let nodes = self.nodes;
        for (id, node) in nodes.iter_enumerated() {
            let index = id.index();
            let parent = nodes.parent_id(id);
            let owner = if parent == id || parent.index() >= index {
                NONE
            } else if self.fn_of_node[parent.index()] != NONE {
                self.fn_of_node[parent.index()]
            } else {
                self.owner[parent.index()]
            };
            self.owner[index] = owner;
            let owner = (owner != NONE).then_some(owner as usize);
            let func = match node.kind() {
                AstKind::Function(function) => Some((function.span, FnNode::Function(function))),
                AstKind::ArrowFunctionExpression(arrow) => Some((arrow.span, FnNode::Arrow(arrow))),
                _ => None,
            };
            if let Some((span, func)) = func {
                let function_index = self.fns.len();
                self.fns.push(FnEntry {
                    node: id,
                    span,
                    func,
                    parent: owner,
                });
                self.own.push(Own::default());
                self.fn_of_node[index] = function_index as u32;
                if let Some(owner) = owner {
                    self.own[owner].nested.push(function_index);
                }
                if let FnNode::Function(function) = func {
                    if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        self.decl_fn.insert(symbol, function_index);
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
                            self.assignments.entry(symbol).or_default().push(assignment);
                            if let Some(rhs) =
                                analysis::expression_symbol(strip(&assignment.right), self.scoping)
                            {
                                self.identifier_origins
                                    .entry(rhs)
                                    .or_default()
                                    .insert(symbol);
                            }
                        }
                    }
                }
                if let Some(owner) = owner {
                    if let Some(name) = assignment_target_key(&assignment.left) {
                        self.own[owner].written_keys.insert(name.to_string());
                    }
                    self.own[owner].assignments.push(assignment);
                }
            }
            AstKind::VariableDeclarator(declarator) => {
                if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                {
                    if let (Some(symbol), Some(rhs)) = (
                        binding.symbol_id.get(),
                        analysis::expression_symbol(strip(init), self.scoping),
                    ) {
                        self.identifier_origins
                            .entry(rhs)
                            .or_default()
                            .insert(symbol);
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
        if let AstKind::ComputedMemberExpression(member) = kind {
            if self.is_arguments_object(&member.object) {
                if let (Some(slot), Some(args_owner)) =
                    (numeric_index(&member.expression), self.args_owner(owner))
                {
                    self.own[args_owner].argument_reads.insert(slot as usize);
                }
            }
        }
        let own = &mut self.own[owner];
        match kind {
            AstKind::CallExpression(call) => own.calls.push(call),
            AstKind::NewExpression(new) => own.news.push(new),
            AstKind::ReturnStatement(statement) => own.returns.push(statement),
            AstKind::UpdateExpression(update) => own.updates.push(update),
            AstKind::IfStatement(statement) => own.ifs.push(statement),
            AstKind::TryStatement(statement) => own.tries.push(statement),
            AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::ForStatement(_) => own.loops += 1,
            AstKind::StaticMemberExpression(member) => {
                own.members
                    .push((&member.object, member.property.name.as_str()));
            }
            AstKind::ComputedMemberExpression(member) => {
                if let Expression::StringLiteral(literal) = &member.expression {
                    own.members.push((&member.object, literal.value.as_str()));
                }
            }
            AstKind::ObjectProperty(property) => {
                if property.kind == PropertyKind::Init {
                    if let Some(name) = property_key_name(&property.key) {
                        own.written_keys.insert(name);
                    }
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Locations
    // -----------------------------------------------------------------------

    fn owner_of(&self, node: NodeId) -> Option<usize> {
        let owner = *self.owner.get(node.index())?;
        (owner != NONE).then_some(owner as usize)
    }

    /// The nearest non-arrow function at or above `index`: the function that
    /// owns `arguments` there.
    fn args_owner(&self, mut index: usize) -> Option<usize> {
        loop {
            if !self.fns[index].is_arrow() {
                return Some(index);
            }
            index = self.fns[index].parent?;
        }
    }

    /// True when `inner` is `outer` or lexically nested inside it.
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

    fn fn_of_expression(&self, expression: &Expression<'a>) -> Option<usize> {
        let node = match strip(expression) {
            Expression::FunctionExpression(function) => function.node_id.get(),
            Expression::ArrowFunctionExpression(arrow) => arrow.node_id.get(),
            _ => return None,
        };
        let index = *self.fn_of_node.get(node.index())?;
        (index != NONE).then_some(index as usize)
    }

    fn is_arguments_object(&self, expression: &Expression<'a>) -> bool {
        matches!(strip(expression), Expression::Identifier(identifier)
            if identifier.name == "arguments"
                && analysis::identifier_symbol(identifier, self.scoping).is_none())
    }

    // -----------------------------------------------------------------------
    // Binding origins (fail closed)
    // -----------------------------------------------------------------------

    fn write_count(&self, symbol: SymbolId) -> usize {
        self.scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count()
    }

    fn declaration_count(&self, symbol: SymbolId) -> usize {
        self.scoping.symbol_declarations(symbol).count()
    }

    fn declaration_owner(&self, symbol: SymbolId) -> Option<usize> {
        self.owner_of(self.scoping.symbol_declaration(symbol))
    }

    /// Every source value of a binding. `None` when the binding has several
    /// declarations or a write the plain-assignment inventory does not model
    /// (compound, update, destructuring, `for...of`).
    fn origins(&self, symbol: SymbolId) -> Option<Vec<Origin<'a>>> {
        if self.declaration_count(symbol) != 1 {
            return None;
        }
        let assignments = self
            .assignments
            .get(&symbol)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if self.write_count(symbol) != assignments.len() {
            return None;
        }
        let mut out = Vec::new();
        let declaration = self.scoping.symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) {
            if let Some(init) = declarator.init.as_ref() {
                out.push(Origin {
                    value: init,
                    node: declaration,
                    owner: self.owner_of(declaration),
                });
            }
        }
        for assignment in assignments {
            let node = assignment.node_id.get();
            out.push(Origin {
                value: &assignment.right,
                node,
                owner: self.owner_of(node),
            });
        }
        Some(out)
    }

    fn single_origin(&self, symbol: SymbolId) -> Option<Origin<'a>> {
        match self.origins(symbol)?.as_slice() {
            [origin] => Some(*origin),
            _ => None,
        }
    }

    /// The function a value expression denotes: a function literal, or an
    /// immutable binding chain that ends at one.
    fn resolve_fn(&self, expression: &Expression<'a>) -> Option<usize> {
        self.resolve_fn_depth(expression, 6)
    }

    fn resolve_fn_depth(&self, expression: &Expression<'a>, depth: u8) -> Option<usize> {
        if let Some(index) = self.fn_of_expression(expression) {
            return Some(index);
        }
        if depth == 0 {
            return None;
        }
        let symbol = analysis::expression_symbol(strip(expression), self.scoping)?;
        if let Some(index) = self.decl_fn.get(&symbol).copied() {
            return (self.declaration_count(symbol) == 1 && self.write_count(symbol) == 0)
                .then_some(index);
        }
        let origin = self.single_origin(symbol)?;
        self.resolve_fn_depth(origin.value, depth - 1)
    }

    /// The editable binding that names function `index`, if any. A function
    /// declaration must be declared once and never written; a function value
    /// must be the single origin of the variable that holds it. Anything else
    /// is refused.
    fn binding_of(&self, index: usize) -> Result<Option<SymbolId>, String> {
        let entry = &self.fns[index];
        if let FnNode::Function(function) = entry.func {
            if function.r#type == FunctionType::FunctionDeclaration {
                let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
                    return Ok(None);
                };
                if self.declaration_count(symbol) != 1 || self.write_count(symbol) != 0 {
                    return Err(
                        "the function declaration's binding is redeclared or reassigned"
                            .to_string(),
                    );
                }
                return Ok(Some(symbol));
            }
        }
        let holder = match self.nodes.parent_kind(entry.node) {
            AstKind::VariableDeclarator(declarator) => match &declarator.id {
                BindingPattern::BindingIdentifier(binding)
                    if declarator
                        .init
                        .as_ref()
                        .is_some_and(|init| strip(init).span() == entry.span) =>
                {
                    binding.symbol_id.get()
                }
                _ => None,
            },
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign
                    && strip(&assignment.right).span() == entry.span =>
            {
                match &assignment.left {
                    AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                        analysis::identifier_symbol(identifier, self.scoping)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(holder) = holder else {
            return Ok(None);
        };
        match self.single_origin(holder) {
            Some(origin) if strip(origin.value).span() == entry.span => Ok(Some(holder)),
            _ => Err("the variable holding the function is redeclared or reassigned".to_string()),
        }
    }

    /// Bindings whose single origin is `symbol` itself (`let alias = f`).
    fn aliases_of(&self, symbol: SymbolId) -> Vec<SymbolId> {
        let Some(candidates) = self.identifier_origins.get(&symbol) else {
            return Vec::new();
        };
        candidates
            .iter()
            .copied()
            .filter(|candidate| {
                self.single_origin(*candidate).is_some_and(|origin| {
                    analysis::expression_symbol(strip(origin.value), self.scoping) == Some(symbol)
                })
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Formal slots: the Nth formal and `arguments[N]` are one slot
    // -----------------------------------------------------------------------

    /// A direct slot read of function `index` itself: its plain,
    /// never-written formal or its own `arguments[N]`, as the shared
    /// [`ParameterSlots`] resolver proves them. An `arguments[N]` read in a
    /// function that writes its `arguments` object is not a slot.
    fn direct_slot(&self, expression: &Expression<'a>, index: usize) -> Option<usize> {
        let slot = self.slots.incoming_slot(expression)?;
        (slot.owner == self.fns[index].node).then_some(slot.index as usize)
    }

    /// A slot read through a direct read or through a local of `index` whose
    /// single origin is a direct read.
    fn value_slot(&self, expression: &Expression<'a>, index: usize) -> Option<usize> {
        if let Some(slot) = self.direct_slot(expression, index) {
            return Some(slot);
        }
        let symbol = analysis::expression_symbol(strip(expression), self.scoping)?;
        self.alias_slot(symbol, index)
    }

    fn alias_slot(&self, symbol: SymbolId, index: usize) -> Option<usize> {
        if self.declaration_owner(symbol) != Some(index) {
            return None;
        }
        let origin = self.single_origin(symbol)?;
        if origin.owner != Some(index) {
            return None;
        }
        self.direct_slot(origin.value, index)
    }

    /// A local of `index` written exactly twice: once from slot N and once,
    /// under `if (slot N === void 0)`, from a default value.
    fn defaulted_slot(
        &self,
        symbol: SymbolId,
        index: usize,
    ) -> Option<(usize, &'a Expression<'a>)> {
        if self.declaration_owner(symbol) != Some(index) {
            return None;
        }
        let origins = self.origins(symbol)?;
        let [first, second] = origins.as_slice() else {
            return None;
        };
        for (slot_origin, default) in [(first, second), (second, first)] {
            if slot_origin.owner != Some(index) || default.owner != Some(index) {
                continue;
            }
            let Some(slot) = self.direct_slot(slot_origin.value, index) else {
                continue;
            };
            if self.guarded_by_undefined_slot(default.node, index, slot) {
                return Some((slot, default.value));
            }
        }
        None
    }

    /// `slot === void 0 ? fallback : slot` written in `index`.
    fn defaulted_value(
        &self,
        expression: &'a Expression<'a>,
        index: usize,
    ) -> Option<(usize, &'a Expression<'a>)> {
        let Expression::ConditionalExpression(conditional) = strip(expression) else {
            return None;
        };
        let slot = self.undefined_slot_test(&conditional.test, index)?;
        (self.direct_slot(&conditional.alternate, index) == Some(slot))
            .then_some((slot, &conditional.consequent))
    }

    /// The same `slot === void 0 ? fallback : slot` written as the leading
    /// `if (slot === void 0) { local = []; } else { local = slot; }` statement,
    /// where `symbol` is `local` and the copy read at `read_span` is `local`'s
    /// only consumer. The two arms are `symbol`'s only writes, each arm runs
    /// exactly its one assignment (no branch side effect), and the statement
    /// precedes the copy in the same top-level statement list, so the two
    /// assignments dominate the copy.
    fn statement_defaulted_value(
        &self,
        symbol: SymbolId,
        index: usize,
        read_span: Span,
    ) -> Option<(usize, &'a Expression<'a>)> {
        if self.declaration_owner(symbol) != Some(index) {
            return None;
        }
        let origins = self.origins(symbol)?;
        let [first, second] = origins.as_slice() else {
            return None;
        };
        if first.owner != Some(index) || second.owner != Some(index) {
            return None;
        }
        let written = [first.node, second.node];
        let assigns = |assignment: &AssignmentExpression<'a>| {
            matches!(
                &assignment.left,
                AssignmentTarget::AssignmentTargetIdentifier(identifier)
                    if analysis::identifier_symbol(identifier, self.scoping) == Some(symbol)
            )
        };
        let copy = self.enclosing_statement_index(read_span, index)?;
        for (position, statement) in self.fns[index].statements().iter().enumerate() {
            let Statement::IfStatement(guard) = statement else {
                continue;
            };
            let Some(alternate) = guard.alternate.as_ref() else {
                continue;
            };
            let Some(slot) = self.undefined_slot_test(&guard.test, index) else {
                continue;
            };
            let Some(empty) = self.branch_assignment(&guard.consequent) else {
                continue;
            };
            let Some(fallback) = self.branch_assignment(alternate) else {
                continue;
            };
            if !assigns(empty) || !is_empty_array(&empty.right) {
                continue;
            }
            if !assigns(fallback) || self.direct_slot(&fallback.right, index) != Some(slot) {
                continue;
            }
            if !written.contains(&empty.node_id.get()) || !written.contains(&fallback.node_id.get())
            {
                continue;
            }
            if position < copy {
                return Some((slot, &empty.right));
            }
        }
        None
    }

    /// The one plain `target = value` assignment a branch runs, when the branch
    /// is exactly that statement.
    fn branch_assignment(&self, branch: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
        let statements = block_statements(branch);
        let [statement] = statements.as_slice() else {
            return None;
        };
        let Statement::ExpressionStatement(expression) = *statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = strip(&expression.expression) else {
            return None;
        };
        (assignment.operator == AssignmentOperator::Assign).then_some(&**assignment)
    }

    /// The position of the top-level statement of `index` that contains `span`.
    fn enclosing_statement_index(&self, span: Span, index: usize) -> Option<usize> {
        self.fns[index]
            .statements()
            .iter()
            .position(|statement| contains(statement.span(), span))
    }

    /// `slot === void 0` (either side) on a direct slot of `index`.
    fn undefined_slot_test(&self, test: &Expression<'a>, index: usize) -> Option<usize> {
        let Expression::BinaryExpression(binary) = strip(test) else {
            return None;
        };
        if binary.operator != BinaryOperator::StrictEquality {
            return None;
        }
        if is_void0(&binary.right) {
            self.direct_slot(&binary.left, index)
        } else if is_void0(&binary.left) {
            self.direct_slot(&binary.right, index)
        } else {
            None
        }
    }

    fn guarded_by_undefined_slot(&self, node: NodeId, index: usize, slot: usize) -> bool {
        let mut current = node;
        for _ in 0..64 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return false;
            }
            match self.nodes.kind(parent) {
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return false,
                AstKind::IfStatement(statement) => {
                    if self.undefined_slot_test(&statement.test, index) == Some(slot)
                        && contains(statement.consequent.span(), self.nodes.kind(current).span())
                    {
                        return true;
                    }
                }
                _ => {}
            }
            current = parent;
        }
        false
    }

    // -----------------------------------------------------------------------
    // Small readers
    // -----------------------------------------------------------------------

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.scoping)
    }

    /// A symbol whose origins include one that satisfies `predicate`.
    fn has_origin(&self, symbol: SymbolId, predicate: impl Fn(&Expression<'a>) -> bool) -> bool {
        self.origins(symbol)
            .is_some_and(|origins| origins.iter().any(|origin| predicate(origin.value)))
    }

    /// `object.method(...)` called on a symbol; `object.method.call(object,
    /// ...)` is the same method call.
    fn calls_method_on(&self, call: &CallExpression<'a>, method: &str) -> Option<SymbolId> {
        let (object, name) = member(&call.callee)?;
        if name == method {
            return self.symbol(object);
        }
        if name != "call" {
            return None;
        }
        let (target, inner) = member(object)?;
        let receiver = self.symbol(target)?;
        let this_arg = call_args(call).first().and_then(|first| self.symbol(first));
        (inner == method && this_arg == Some(receiver)).then_some(receiver)
    }

    /// The statements of a function body without a trailing bare return.
    fn effective_statements(
        &self,
        index: usize,
        allow_return_this: bool,
    ) -> Vec<&'a Statement<'a>> {
        let mut statements: Vec<&'a Statement<'a>> = self.fns[index].statements().iter().collect();
        if let Some(Statement::ReturnStatement(statement)) = statements.last() {
            let bare = match statement.argument.as_ref() {
                None => true,
                Some(argument) => {
                    is_void0(argument)
                        || (allow_return_this
                            && matches!(strip(argument), Expression::ThisExpression(_)))
                }
            };
            if bare {
                statements.pop();
            }
        }
        statements
    }

    fn label(&self, index: usize) -> String {
        let span = self.fns[index].span;
        format!("function {}..{}", span.start, span.end)
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

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn is_void0(expression: &Expression<'_>) -> bool {
    match strip(expression) {
        Expression::UnaryExpression(unary) => unary.operator == UnaryOperator::Void,
        _ => false,
    }
}

fn is_bool(expression: &Expression<'_>, value: bool) -> bool {
    matches!(strip(expression), Expression::BooleanLiteral(literal) if literal.value == value)
}

fn is_number(expression: &Expression<'_>, value: f64) -> bool {
    matches!(strip(expression), Expression::NumericLiteral(literal) if literal.value == value)
}

fn is_empty_array(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::ArrayExpression(array) if array.elements.is_empty())
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

/// `this.name` / `this["name"]`.
fn this_key<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    let (object, name) = member(expression)?;
    matches!(strip(object), Expression::ThisExpression(_)).then_some(name)
}

fn assignment_target_key<'e>(target: &'e AssignmentTarget<'_>) -> Option<&'e str> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

/// `(object, key)` of a member assignment target.
fn assignment_target_member<'e, 'a>(
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

fn call_args<'e, 'a>(call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
    call.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

/// The arguments of a method call, without the receiver of the
/// `object.method.call(object, ...)` form.
fn method_args<'e, 'a>(call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
    let args = call_args(call);
    match member(&call.callee) {
        Some((object, "call")) if member(object).is_some() && !args.is_empty() => {
            args[1..].to_vec()
        }
        _ => args,
    }
}

/// A call with `.call(void 0, ...)` unwrapped: `f(a)` and `f.call(void 0, a)`
/// are the same invocation of `f`.
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

/// True when an expression mentions `this.key` anywhere.
fn mentions_this_key(expression: &Expression<'_>, key: &str) -> bool {
    struct Scan<'k> {
        key: &'k str,
        found: bool,
    }
    impl<'a> Visit<'a> for Scan<'_> {
        fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
            if matches!(member.object, Expression::ThisExpression(_))
                && member.property.name == self.key
            {
                self.found = true;
            }
            walk::walk_static_member_expression(self, member);
        }
        fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
            if matches!(member.object, Expression::ThisExpression(_))
                && matches!(&member.expression, Expression::StringLiteral(literal) if literal.value == self.key)
            {
                self.found = true;
            }
            walk::walk_computed_member_expression(self, member);
        }
    }
    let mut scan = Scan { key, found: false };
    scan.visit_expression(expression);
    scan.found
}

/// The statement as a call expression statement.
fn statement_call<'e, 'a>(statement: &'e Statement<'a>) -> Option<&'e CallExpression<'a>> {
    match statement {
        Statement::ExpressionStatement(expression) => match strip(&expression.expression) {
            Expression::CallExpression(call) => Some(call),
            _ => None,
        },
        _ => None,
    }
}

fn block_statements<'e, 'a>(statement: &'e Statement<'a>) -> Vec<&'e Statement<'a>> {
    match statement {
        Statement::BlockStatement(block) => block.body.iter().collect(),
        other => vec![other],
    }
}

fn span_text(span: Span) -> String {
    format!("{}..{}", span.start, span.end)
}

// ===========================================================================
// Finders
// ===========================================================================

fn run_finders(program: &Program<'_>, scoping: &Scoping, nodes: &AstNodes<'_>) -> Run {
    let ctx = Ctx::new(program, scoping, nodes);
    let mut finder = Finder {
        ctx: &ctx,
        candidates: Vec::new(),
        refusals: Vec::new(),
        ties: Vec::new(),
    };
    let registries = finder.callback_registries();
    let onces = finder.once_wrappers();
    finder.task_queues(&onces);
    finder.async_hashes();
    finder.interception_wrappers(&registries);
    finder.finish()
}

struct Finder<'c, 's, 'a> {
    ctx: &'c Ctx<'s, 'a>,
    candidates: Vec<Candidate>,
    refusals: Vec<Refusal>,
    /// Structural ties detected by a finder before naming.
    ties: Vec<(&'static str, Vec<usize>, String)>,
}

impl<'c, 's, 'a> Finder<'c, 's, 'a> {
    fn push(
        &mut self,
        role: &'static str,
        function: usize,
        confidence: Confidence,
        evidence: Vec<String>,
    ) {
        self.candidates.push(Candidate {
            role,
            function,
            confidence,
            evidence,
        });
    }

    fn refuse(&mut self, role: &'static str, function: usize, reason: impl Into<String>) {
        self.refusals.push(Refusal {
            role,
            span: self.ctx.fns[function].span,
            reason: reason.into(),
        });
    }

    // -----------------------------------------------------------------------
    // (a) Callback registry
    // -----------------------------------------------------------------------

    /// A registry factory builds a local object literal with exactly one
    /// empty-array slot (the callback list), attaches its own nested methods to
    /// that object, and returns it. Its notify method walks a copy of the list
    /// from the end and threads each callback's result into the next call; its
    /// register method appends the argument to the list.
    ///
    /// Not a registry: a plain object factory without a proven notify method,
    /// an emitter that notifies in forward order without threading a result,
    /// a reduce helper that does not read the owner's list.
    fn callback_registries(&mut self) -> Vec<usize> {
        let ctx = self.ctx;
        let mut proven = Vec::new();
        for factory in 0..ctx.fns.len() {
            if ctx.fns[factory].is_arrow() {
                continue;
            }
            for declarator in &ctx.own[factory].declarators {
                let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                else {
                    continue;
                };
                let Expression::ObjectExpression(object) = strip(init) else {
                    continue;
                };
                let Some(symbol) = binding.symbol_id.get() else {
                    continue;
                };
                let members = object_members_of(object);
                let lists: Vec<&str> = members
                    .iter()
                    .filter(|member| is_empty_array(member.value))
                    .map(|member| member.name.as_str())
                    .collect();
                let [list_key] = lists.as_slice() else {
                    continue;
                };
                if !self.returns_only(factory, symbol) {
                    continue;
                }
                // A method slot written twice proves nothing about either value.
                let Some(methods) = self.owned_methods(factory, symbol, &members) else {
                    continue;
                };
                let notifies: Vec<(String, usize, Vec<String>)> = methods
                    .iter()
                    .filter_map(|(key, method)| {
                        self.registry_notify(*method, list_key)
                            .map(|evidence| (key.clone(), *method, evidence))
                    })
                    .collect();
                let [(notify_key, notify, notify_evidence)] = notifies.as_slice() else {
                    if notifies.len() > 1 {
                        self.ties.push((
                            CALLBACK_REGISTRY_NOTIFY,
                            notifies.iter().map(|(_, method, _)| *method).collect(),
                            "more than one method of one registry threads results through the callback list".to_string(),
                        ));
                    }
                    continue;
                };
                let registers: Vec<(usize, bool)> = methods
                    .iter()
                    .filter(|(_, method)| method != notify)
                    .filter_map(|(key, method)| {
                        self.registry_register(*method, list_key, key)
                            .map(|helper| (*method, helper))
                    })
                    .collect();
                let [(register, opaque)] = registers.as_slice() else {
                    continue;
                };
                if ctx.write_count(symbol) != 0 {
                    self.refuse(
                        CALLBACK_REGISTRY_FACTORY,
                        factory,
                        "the registry object binding is reassigned",
                    );
                    continue;
                }
                let object_span = span_text(object.span);
                self.push(
                    CALLBACK_REGISTRY_FACTORY,
                    factory,
                    Confidence::High,
                    vec![
                        format!("returns its own object literal {object_span} whose only empty-array slot `{list_key}` is the callback list"),
                        format!("attaches its nested notify method as `{notify_key}` and its register method to that object"),
                    ],
                );
                let mut evidence = notify_evidence.clone();
                evidence.push(format!(
                    "method `{notify_key}` of the registry built at {object_span}"
                ));
                self.push(
                    CALLBACK_REGISTRY_NOTIFY,
                    *notify,
                    Confidence::High,
                    evidence,
                );
                let register_evidence = if *opaque {
                    vec![
                        format!(
                            "passes `this.{list_key}` and its slot 0 to one discarded helper call"
                        ),
                        "attached under the registry's `register` key".to_string(),
                    ]
                } else {
                    vec![format!(
                        "appends its slot 0 to `this.{list_key}` and returns nothing else"
                    )]
                };
                self.push(
                    CALLBACK_REGISTRY_REGISTER,
                    *register,
                    if *opaque {
                        Confidence::Medium
                    } else {
                        Confidence::High
                    },
                    register_evidence,
                );
                proven.push(factory);
            }
        }
        proven
    }

    /// Every own `return` of `index` returns exactly `symbol`.
    fn returns_only(&self, index: usize, symbol: SymbolId) -> bool {
        let returns = &self.ctx.own[index].returns;
        !returns.is_empty()
            && returns.iter().all(|statement| {
                statement
                    .argument
                    .as_ref()
                    .is_some_and(|argument| self.ctx.symbol(argument) == Some(symbol))
            })
    }

    /// The functions directly nested in `owner` that it attaches to its local
    /// object `symbol`, by key. `None` when one key is written twice.
    fn owned_methods(
        &self,
        owner: usize,
        symbol: SymbolId,
        literal: &[crate::signals::extract::bindings::ObjectMember<'a>],
    ) -> Option<Vec<(String, usize)>> {
        let ctx = self.ctx;
        let mut methods: Vec<(String, usize)> = Vec::new();
        let mut keys: BTreeSet<String> = BTreeSet::new();
        for member in literal {
            if let Some(method) = ctx.resolve_fn(member.value) {
                if ctx.fns[method].parent == Some(owner) {
                    keys.insert(member.name.clone());
                    methods.push((member.name.clone(), method));
                }
            }
        }
        for assignment in &ctx.own[owner].assignments {
            let Some((object, key)) = assignment_target_member(&assignment.left) else {
                continue;
            };
            if ctx.symbol(object) != Some(symbol) {
                continue;
            }
            let Some(method) = ctx.resolve_fn(&assignment.right) else {
                continue;
            };
            if ctx.fns[method].parent != Some(owner) {
                continue;
            }
            if !keys.insert(key.to_string()) {
                return None;
            }
            methods.push((key.to_string(), method));
        }
        Some(methods)
    }

    /// Reverse-order notify that threads each callback's result.
    fn registry_notify(&self, method: usize, list_key: &str) -> Option<Vec<String>> {
        let ctx = self.ctx;
        if ctx.fns[method].is_arrow() {
            return None;
        }
        let own = &ctx.own[method];
        if own.loops == 0 {
            return None;
        }
        if own.assignments.iter().any(|assignment| {
            assignment_target_member(&assignment.left).is_some_and(|(object, key)| {
                key == list_key && matches!(strip(object), Expression::ThisExpression(_))
            })
        }) {
            return None;
        }
        // The local copy of the owner's callback list.
        let copies: BTreeSet<SymbolId> = own
            .declarators
            .iter()
            .filter_map(|declarator| {
                let init = declarator.init.as_ref()?;
                let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                    return None;
                };
                mentions_this_key(init, list_key)
                    .then(|| binding.symbol_id.get())
                    .flatten()
            })
            .collect();
        if copies.is_empty() {
            return None;
        }
        let decrements = own.updates.iter().any(|update| {
            update.operator == UpdateOperator::Decrement
                && matches!(
                    &update.argument,
                    SimpleAssignmentTarget::AssignmentTargetIdentifier(_)
                )
        });
        if !decrements {
            return None;
        }
        let tries: Vec<Span> = own
            .tries
            .iter()
            .map(|statement| statement.block.span)
            .collect();
        for call in &own.calls {
            if !tries.iter().any(|block| contains(*block, call.span)) {
                continue;
            }
            let (callee, args) = invocation(call);
            let [payload, accumulator] = args.as_slice() else {
                continue;
            };
            if ctx.value_slot(payload, method) != Some(0) {
                continue;
            }
            let Some(accumulator) = ctx.symbol(accumulator) else {
                continue;
            };
            if ctx.declaration_owner(accumulator) != Some(method) {
                continue;
            }
            let from_copy = |expression: &Expression<'a>| {
                matches!(strip(expression), Expression::ComputedMemberExpression(element)
                    if ctx.symbol(&element.object).is_some_and(|symbol| copies.contains(&symbol)))
            };
            let callee_ok = from_copy(callee)
                || ctx
                    .symbol(callee)
                    .is_some_and(|temp| ctx.has_origin(temp, |value| from_copy(value)));
            if !callee_ok {
                continue;
            }
            let threaded = ctx.has_origin(accumulator, |value| {
                strip(value).span() == call.span
                    || ctx.symbol(value).is_some_and(|temp| {
                        ctx.has_origin(temp, |inner| strip(inner).span() == call.span)
                    })
            });
            let returned = own.returns.iter().any(|statement| {
                statement
                    .argument
                    .as_ref()
                    .is_some_and(|argument| ctx.symbol(argument) == Some(accumulator))
            });
            if threaded && returned {
                return Some(vec![
                    format!("walks a copy of `this.{list_key}` with a decrementing counter"),
                    format!(
                        "calls each callback inside try with (slot 0, accumulator) at {} and threads the result",
                        span_text(call.span)
                    ),
                    "returns the accumulated result".to_string(),
                ]);
            }
        }
        None
    }

    /// Append-only register. `Some(true)` when the append goes through an
    /// opaque bound helper and is only linked by the `register` key.
    fn registry_register(&self, method: usize, list_key: &str, key: &str) -> Option<bool> {
        let ctx = self.ctx;
        if ctx.fns[method].is_arrow() {
            return None;
        }
        let statements = ctx.effective_statements(method, true);
        let [statement] = statements.as_slice() else {
            return None;
        };
        let call = statement_call(statement)?;
        let (callee, args) = invocation(call);
        if let Some((object, "push")) = member(callee) {
            if this_key(object) == Some(list_key)
                && args.len() == 1
                && ctx.value_slot(args[0], method) == Some(0)
            {
                return Some(false);
            }
            return None;
        }
        let opaque = matches!(strip(callee), Expression::Identifier(_))
            && args.iter().any(|arg| this_key(arg) == Some(list_key))
            && args
                .last()
                .is_some_and(|last| ctx.value_slot(last, method) == Some(0))
            && key == "register";
        opaque.then_some(true)
    }

    // -----------------------------------------------------------------------
    // (e) Once wrappers
    // -----------------------------------------------------------------------

    /// A once factory keeps its slot-0 callback and a local latch that starts
    /// `false`, and returns its own nested function that tests the latch, sets
    /// it `true` and calls the callback. The latch has exactly those two
    /// writes.
    ///
    /// Not once: a throttle (its latch is reset to `false` by a timer), a
    /// memoizer (stores a result), a debounce (timer state, no latch).
    fn once_wrappers(&mut self) -> Vec<usize> {
        let ctx = self.ctx;
        let mut factories = Vec::new();
        for factory in 0..ctx.fns.len() {
            let returns = &ctx.own[factory].returns;
            if returns.is_empty() {
                continue;
            }
            let returned: BTreeSet<usize> = returns
                .iter()
                .filter_map(|statement| ctx.resolve_fn(statement.argument.as_ref()?))
                .collect();
            let [inner] = returned.iter().copied().collect::<Vec<_>>()[..] else {
                continue;
            };
            if returned.len() != 1
                || returns.iter().any(|statement| statement.argument.is_none())
                || ctx.fns[inner].parent != Some(factory)
            {
                continue;
            }
            let Some(evidence) = self.once_guard(factory, inner) else {
                continue;
            };
            self.push(
                ONCE_FACTORY,
                factory,
                Confidence::High,
                vec![
                    "keeps its slot-0 callback and a local latch initialized `false`".to_string(),
                    format!(
                        "returns its own guarded function {}",
                        span_text(ctx.fns[inner].span)
                    ),
                ],
            );
            self.push(ONCE_INVOKE, inner, Confidence::High, evidence);
            factories.push(factory);
        }
        factories
    }

    fn once_guard(&self, factory: usize, inner: usize) -> Option<Vec<String>> {
        let ctx = self.ctx;
        let statements = ctx.effective_statements(inner, false);
        // `if (!latch) { latch = true; callback(...) }` or
        // `if (latch) return; latch = true; callback(...)`.
        let (latch, body): (SymbolId, Vec<&'a Statement<'a>>) = match statements.as_slice() {
            [Statement::IfStatement(guard)] if guard.alternate.is_none() => {
                let Expression::UnaryExpression(not) = strip(&guard.test) else {
                    return None;
                };
                if not.operator != UnaryOperator::LogicalNot {
                    return None;
                }
                (
                    ctx.symbol(&not.argument)?,
                    block_statements(&guard.consequent),
                )
            }
            [Statement::IfStatement(guard), rest @ ..] if guard.alternate.is_none() => {
                let returns = matches!(block_statements(&guard.consequent).as_slice(),
                    [Statement::ReturnStatement(statement)]
                        if statement.argument.as_ref().is_none_or(|argument| is_void0(argument)));
                if !returns {
                    return None;
                }
                (ctx.symbol(&guard.test)?, rest.to_vec())
            }
            _ => return None,
        };
        let [set, invoke] = body.as_slice() else {
            return None;
        };
        let Statement::ExpressionStatement(set) = set else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = strip(&set.expression) else {
            return None;
        };
        let target_is_latch = matches!(&assignment.left,
            AssignmentTarget::AssignmentTargetIdentifier(identifier)
                if analysis::identifier_symbol(identifier, ctx.scoping) == Some(latch));
        if !target_is_latch || !is_bool(&assignment.right, true) {
            return None;
        }
        let call = statement_call(invoke)?;
        let (callee, _) = invocation(call);
        if ctx.value_slot(callee, factory) != Some(0) {
            return None;
        }
        if ctx.declaration_owner(latch) != Some(factory) {
            return None;
        }
        // A `void 0` declaration in the factory is the same unset latch.
        let origins: Vec<Origin<'a>> = ctx
            .origins(latch)?
            .into_iter()
            .filter(|origin| !(origin.owner == Some(factory) && is_void0(origin.value)))
            .collect();
        let [first, second] = origins.as_slice() else {
            return None;
        };
        let pair_ok = |initial: &Origin<'a>, set: &Origin<'a>| {
            initial.owner == Some(factory)
                && is_bool(initial.value, false)
                && set.owner == Some(inner)
                && is_bool(set.value, true)
        };
        if !(pair_ok(first, second) || pair_ok(second, first)) {
            return None;
        }
        Some(vec![
            "tests the factory's latch, sets it `true`, then calls the factory's slot-0 callback"
                .to_string(),
            "the latch has no other write, so later calls do nothing".to_string(),
        ])
    }

    // -----------------------------------------------------------------------
    // (b) Task queues
    // -----------------------------------------------------------------------

    /// A task-queue factory returns its own API object whose `start`,
    /// `enqueue`, `empty`, `onEmpty` and `onError` methods are each proven by
    /// behavior over the factory's own pending list (a copy of a defaulted
    /// initial-items slot) and its two listener lists. The variant comes from
    /// how the factory's worker slot is called on a dequeued item:
    ///
    /// * one argument, timed by a defaulted clock slot inside a drain loop:
    ///   time-budgeted;
    /// * `(item, wrapped continuation)` and scheduled through a caller-supplied
    ///   scheduler slot: scheduled;
    /// * `(item, wrapped continuation)` and scheduled through a slot that
    ///   defaults to the host `setTimeout`: sequential.
    ///
    /// Not a queue: a debounce or throttle (no pending list or listener API), a
    /// Promise/Deferred (no start/enqueue), a polling loop.
    fn task_queues(&mut self, onces: &[usize]) {
        let ctx = self.ctx;
        let mut continuation_queues: Vec<(usize, &'static str, usize)> = Vec::new();
        for factory in 0..ctx.fns.len() {
            if ctx.fns[factory].is_arrow() {
                continue;
            }
            let Some(queue) = self.queue_api(factory) else {
                continue;
            };
            let QueueApi {
                api,
                methods,
                pending,
                initial_slot,
                initial_default,
            } = queue;
            let Some((variant, worker_slot, mut evidence)) =
                self.queue_variant(factory, pending, onces)
            else {
                continue;
            };
            if ctx.write_count(api) != 0 {
                self.refuse(
                    variant,
                    factory,
                    "the queue API object binding is reassigned",
                );
                continue;
            }
            evidence.insert(
                0,
                format!(
                    "returns its own API object with proven start/enqueue/empty/onEmpty/onError methods over a pending copy of slot {initial_slot}"
                ),
            );
            if initial_default == InitialDefault::Statement {
                evidence.push(
                    "statement-form initial-items default: preserve the existing once-wrapper/base attribution"
                        .to_string(),
                );
            }
            self.push(variant, factory, Confidence::High, evidence);
            for (role, method, evidence) in methods {
                self.push(role, method, Confidence::High, evidence);
            }
            // The statement spelling proves the factory and its five methods;
            // it deliberately does not feed the caller-runner proof, so an
            // already-applied generic worker name keeps its binding. The
            // advance step is proven separately by the queue-helper finder.
            if variant != TIME_BUDGETED_QUEUE_FACTORY
                && initial_default == InitialDefault::Conditional
            {
                continuation_queues.push((factory, variant, worker_slot));
            }
        }
        self.queue_runners(&continuation_queues);
    }

    fn queue_api(&mut self, factory: usize) -> Option<QueueApi> {
        let ctx = self.ctx;
        for declarator in &ctx.own[factory].declarators {
            let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                (&declarator.id, declarator.init.as_ref())
            else {
                continue;
            };
            let Expression::ObjectExpression(object) = strip(init) else {
                continue;
            };
            let Some(api) = binding.symbol_id.get() else {
                continue;
            };
            if !self.returns_only(factory, api) {
                continue;
            }
            let literal = object_members_of(object);
            let methods = self.owned_methods(factory, api, &literal)?;
            let method = |key: &str| {
                methods
                    .iter()
                    .find(|(name, _)| name == key)
                    .map(|(_, index)| *index)
            };
            let (Some(start), Some(enqueue), Some(empty), Some(on_empty), Some(on_error)) = (
                method("start"),
                method("enqueue"),
                method("empty"),
                method("onEmpty"),
                method("onError"),
            ) else {
                continue;
            };
            let (pending, initial_slot, initial_default) = self.pending_list(factory)?;
            let on_empty_list = self.slot_pushed_list(on_empty, factory)?;
            let on_error_list = self.slot_pushed_list(on_error, factory)?;
            if on_empty_list == on_error_list || ctx.own[on_error].ifs.is_empty() {
                return None;
            }
            if !self.queue_empty(empty, pending, on_empty_list)
                || !self.queue_enqueue(enqueue, pending)
                || !self.queue_start(start, factory)
            {
                return None;
            }
            return Some(QueueApi {
                api,
                methods: vec![
                    (QUEUE_START, start, vec!["one-shot started latch that kicks the queue".to_string()]),
                    (QUEUE_ENQUEUE, enqueue, vec!["appends its call arguments to the pending list and kicks an idle started queue".to_string()]),
                    (QUEUE_EMPTY, empty, vec!["drops pending tasks and schedules every empty listener".to_string()]),
                    (QUEUE_ON_EMPTY, on_empty, vec!["appends its slot 0 to the empty-listener list".to_string()]),
                    (QUEUE_ON_ERROR, on_error, vec!["appends its slot 0 to the error-listener list unless the queue already failed".to_string()]),
                ],
                pending,
                initial_slot,
                initial_default,
            });
        }
        None
    }

    /// The factory's pending list: a local written from `items.slice()` where
    /// `items` defaults an initial-items slot to `[]`, either as the
    /// `slot === void 0 ? [] : slot` conditional value or as the equivalent
    /// leading `if (slot === void 0) { items = []; } else { items = slot; }`
    /// statement. The returned form keeps the two spellings distinguishable for
    /// the caller-runner proof.
    fn pending_list(&self, factory: usize) -> Option<(SymbolId, usize, InitialDefault)> {
        let ctx = self.ctx;
        let mut found: Option<(SymbolId, usize, InitialDefault)> = None;
        let mut consider = |target: SymbolId, value: &'a Expression<'a>| {
            let Expression::CallExpression(call) = strip(value) else {
                return;
            };
            let Some(items) = ctx.calls_method_on(call, "slice") else {
                return;
            };
            if !call.arguments.is_empty() {
                return;
            }
            let defaulted = ctx
                .single_origin(items)
                .and_then(|origin| ctx.defaulted_value(origin.value, factory))
                .map(|value| (value, InitialDefault::Conditional))
                .or_else(|| {
                    ctx.statement_defaulted_value(items, factory, call.span)
                        .map(|value| (value, InitialDefault::Statement))
                });
            if let Some(((slot, fallback), form)) = defaulted {
                if is_empty_array(fallback) && ctx.declaration_owner(target) == Some(factory) {
                    found = Some((target, slot, form));
                }
            }
        };
        for declarator in &ctx.own[factory].declarators {
            if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                (&declarator.id, declarator.init.as_ref())
            {
                if let Some(symbol) = binding.symbol_id.get() {
                    consider(symbol, init);
                }
            }
        }
        for assignment in &ctx.own[factory].assignments {
            if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                if let Some(symbol) = analysis::identifier_symbol(identifier, ctx.scoping) {
                    consider(symbol, &assignment.right);
                }
            }
        }
        found
    }

    /// The factory-local list a method appends its slot 0 to.
    fn slot_pushed_list(&self, method: usize, factory: usize) -> Option<SymbolId> {
        let ctx = self.ctx;
        let mut lists = BTreeSet::new();
        for call in &ctx.own[method].calls {
            let Some(list) = ctx.calls_method_on(call, "push") else {
                continue;
            };
            let args = method_args(call);
            if args.len() == 1
                && ctx.value_slot(args[0], method) == Some(0)
                && ctx.declaration_owner(list) == Some(factory)
            {
                lists.insert(list);
            }
        }
        match lists.len() {
            1 => lists.into_iter().next(),
            _ => None,
        }
    }

    fn queue_empty(&self, method: usize, pending: SymbolId, listeners: SymbolId) -> bool {
        let ctx = self.ctx;
        let own = &ctx.own[method];
        let tests_pending = own.ifs.iter().any(|statement| {
            matches!(strip(&statement.test), Expression::BinaryExpression(binary)
                if binary.operator == BinaryOperator::GreaterThan
                    && is_number(&binary.right, 0.0)
                    && member(&binary.left).is_some_and(|(object, name)| name == "length" && ctx.symbol(object) == Some(pending)))
        });
        let resets = own
            .assignments
            .iter()
            .any(|assignment| match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                    analysis::identifier_symbol(identifier, ctx.scoping) == Some(pending)
                        && is_empty_array(&assignment.right)
                }
                target => assignment_target_member(target).is_some_and(|(object, name)| {
                    name == "length"
                        && ctx.symbol(object) == Some(pending)
                        && is_number(&assignment.right, 0.0)
                }),
            });
        let notifies = own
            .calls
            .iter()
            .any(|call| ctx.calls_method_on(call, "forEach") == Some(listeners));
        tests_pending && resets && notifies
    }

    fn queue_enqueue(&self, method: usize, pending: SymbolId) -> bool {
        let ctx = self.ctx;
        ctx.own[method].calls.iter().any(|call| {
            let Some((push, "apply")) = member(&call.callee) else {
                return ctx.calls_method_on(call, "push") == Some(pending);
            };
            member(push).is_some_and(|(_, name)| name == "push")
                && call_args(call)
                    .first()
                    .is_some_and(|first| ctx.symbol(first) == Some(pending))
        })
    }

    fn queue_start(&self, method: usize, factory: usize) -> bool {
        let ctx = self.ctx;
        let own = &ctx.own[method];
        own.assignments.iter().any(|assignment| {
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                return false;
            };
            let Some(flag) = analysis::identifier_symbol(identifier, ctx.scoping) else {
                return false;
            };
            is_bool(&assignment.right, true)
                && ctx.declaration_owner(flag) == Some(factory)
                && own
                    .ifs
                    .iter()
                    .any(|statement| ctx.symbol(&statement.test) == Some(flag))
        }) && !own.calls.is_empty()
    }

    /// The queue variant, the worker slot and evidence.
    fn queue_variant(
        &self,
        factory: usize,
        pending: SymbolId,
        onces: &[usize],
    ) -> Option<(&'static str, usize, Vec<String>)> {
        let ctx = self.ctx;
        let nested: Vec<usize> = (0..ctx.fns.len())
            .filter(|index| *index != factory && ctx.within(*index, factory))
            .collect();
        let dequeued = |expression: &Expression<'a>| {
            ctx.symbol(expression).is_some_and(|item| {
                ctx.has_origin(item, |value| {
                    matches!(strip(value), Expression::CallExpression(call)
                        if ctx.calls_method_on(call, "shift") == Some(pending))
                })
            })
        };
        let mut workers: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut continuation_wrapped_by_once = false;
        for &index in &nested {
            for call in &ctx.own[index].calls {
                let (callee, args) = invocation(call);
                let Some(slot) = ctx.value_slot(callee, factory) else {
                    continue;
                };
                if !args.first().is_some_and(|first| dequeued(first)) {
                    continue;
                }
                if let Some(Expression::CallExpression(wrap)) = args.get(1).map(|arg| strip(arg)) {
                    if ctx
                        .resolve_fn(&wrap.callee)
                        .is_some_and(|wrapper| onces.contains(&wrapper))
                    {
                        continuation_wrapped_by_once = true;
                    }
                }
                workers.insert((slot, args.len()));
            }
        }
        let [(worker_slot, arity)] = workers.iter().copied().collect::<Vec<_>>()[..] else {
            return None;
        };
        let mut evidence = vec![format!(
            "calls its slot {worker_slot} worker on each item shifted off the pending list with {arity} argument(s)"
        )];
        if arity == 1 {
            let clock = nested.iter().any(|&index| {
                ctx.own[index].calls.iter().any(|call| {
                    let (callee, args) = invocation(call);
                    args.is_empty()
                        && ctx
                            .symbol(callee)
                            .is_some_and(|symbol| ctx.defaulted_slot(symbol, factory).is_some())
                })
            });
            let drains = nested.iter().any(|&index| ctx.own[index].loops > 0);
            if !(clock && drains) {
                return None;
            }
            evidence.push(
                "a nested drain loop times each task with a defaulted clock slot".to_string(),
            );
            return Some((TIME_BUDGETED_QUEUE_FACTORY, worker_slot, evidence));
        }
        if arity != 2 {
            return None;
        }
        if continuation_wrapped_by_once {
            evidence.push("the continuation is wrapped by a proven once factory".to_string());
        }
        let mut schedulers: BTreeSet<SymbolId> = BTreeSet::new();
        for &index in &nested {
            for call in &ctx.own[index].calls {
                let (callee, args) = invocation(call);
                if args.len() != 2 || !is_number(args[1], 0.0) {
                    continue;
                }
                if let Some(symbol) = ctx.symbol(callee) {
                    if ctx.declaration_owner(symbol) == Some(factory) {
                        schedulers.insert(symbol);
                    }
                }
            }
        }
        let [scheduler] = schedulers.iter().copied().collect::<Vec<_>>()[..] else {
            return None;
        };
        if let Some(slot) = ctx.alias_slot(scheduler, factory) {
            evidence.push(format!(
                "schedules every step through the caller-supplied slot {slot}"
            ));
            return Some((SCHEDULED_QUEUE_FACTORY, worker_slot, evidence));
        }
        if let Some((slot, fallback)) = ctx.defaulted_slot(scheduler, factory) {
            let host_timer = member(fallback).is_some_and(|(object, name)| {
                name == "setTimeout"
                    && matches!(strip(object), Expression::Identifier(identifier)
                        if analysis::identifier_symbol(identifier, ctx.scoping).is_none())
            });
            if host_timer {
                evidence.push(format!(
                    "schedules every step through slot {slot}, which defaults to the host setTimeout"
                ));
                return Some((SEQUENTIAL_QUEUE_FACTORY, worker_slot, evidence));
            }
        }
        None
    }

    /// The task runner passed to a proven continuation queue: invokes its
    /// slot-0 task and its slot-1 continuation, forwarding a thrown error to the
    /// continuation. A runner shape with no proven queue consumer keeps the
    /// generic role.
    ///
    /// Not a runner: a `finally` cleanup that does not receive the error, a
    /// Promise chain, a wrapper that rethrows.
    fn queue_runners(&mut self, queues: &[(usize, &'static str, usize)]) {
        let ctx = self.ctx;
        let mut consumed: BTreeMap<usize, BTreeSet<&'static str>> = BTreeMap::new();
        for own in &ctx.own {
            for call in &own.calls {
                let Some(factory) = ctx.resolve_fn(&call.callee) else {
                    continue;
                };
                for &(queue, variant, worker_slot) in queues {
                    if queue != factory {
                        continue;
                    }
                    let Some(argument) = call_args(call).get(worker_slot).copied() else {
                        continue;
                    };
                    if let Some(runner) = ctx.resolve_fn(argument) {
                        consumed.entry(runner).or_default().insert(variant);
                    }
                }
            }
        }
        for runner in 0..ctx.fns.len() {
            if !self.forwards_error_to_continuation(runner) {
                continue;
            }
            let base = "runs its slot-0 task then its slot-1 continuation and forwards a thrown error to the continuation".to_string();
            match consumed
                .get(&runner)
                .map(|variants| variants.iter().copied().collect::<Vec<_>>())
            {
                None => {}
                Some(variants) => match variants.as_slice() {
                    [variant] => {
                        let role = if *variant == SEQUENTIAL_QUEUE_FACTORY {
                            SEQUENTIAL_QUEUE_RUNNER
                        } else {
                            SCHEDULED_QUEUE_RUNNER
                        };
                        self.push(
                            role,
                            runner,
                            Confidence::High,
                            vec![
                                base,
                                "passed as the worker slot of a proven queue factory".to_string(),
                            ],
                        );
                    }
                    _ => self.ties.push((
                        SEQUENTIAL_QUEUE_RUNNER,
                        vec![runner],
                        "one runner is consumed by more than one queue variant".to_string(),
                    )),
                },
            }
        }
    }

    fn forwards_error_to_continuation(&self, index: usize) -> bool {
        let ctx = self.ctx;
        if ctx.fns[index].is_arrow() {
            return false;
        }
        let statements = ctx.effective_statements(index, false);
        let [Statement::TryStatement(statement)] = statements.as_slice() else {
            return false;
        };
        if statement.finalizer.is_some() {
            return false;
        }
        let Some(handler) = statement.handler.as_ref() else {
            return false;
        };
        let Some(error) = handler
            .param
            .as_ref()
            .and_then(|param| match &param.pattern {
                BindingPattern::BindingIdentifier(binding) => binding.symbol_id.get(),
                _ => None,
            })
        else {
            return false;
        };
        let calls_slot = |statement: &Statement<'a>, slot: usize, forwards_error: bool| {
            let Some(call) = statement_call(statement) else {
                return false;
            };
            let (callee, args) = invocation(call);
            ctx.value_slot(callee, index) == Some(slot)
                && match args.as_slice() {
                    [] => !forwards_error,
                    [argument] => forwards_error && ctx.symbol(argument) == Some(error),
                    _ => false,
                }
        };
        let block: Vec<&Statement<'a>> = statement.block.body.iter().collect();
        let catch: Vec<&Statement<'a>> = handler.body.body.iter().collect();
        matches!(block.as_slice(), [task, done] if calls_slot(task, 0, false) && calls_slot(done, 1, false))
            && matches!(catch.as_slice(), [forward] if calls_slot(forward, 1, true))
    }

    // -----------------------------------------------------------------------
    // (c) Asynchronous signed 32-bit rolling hashes
    // -----------------------------------------------------------------------

    /// An async rolling hasher hands one of its own nested, parameterless
    /// functions to a call at its own level (the batch scheduler). That step
    /// advances `h = ((h << 5) - h + x) | 0` in a loop, returns `true` to be
    /// rescheduled and `false` when done, passes the final `h` to a callback
    /// slot of the hasher and forwards a caught error to another slot. The
    /// hasher itself returns nothing. Int32 words over the slot-0 view's
    /// `buffer` make the typed-array variant; UTF-16 char pairs of the slot-0
    /// string make the string variant.
    ///
    /// Not async: the synchronous string/buffer hash (`string_buffer_hash`)
    /// advances the same recurrence in its own body and returns the value.
    /// Not a digest: no rolling recurrence.
    fn async_hashes(&mut self) {
        let ctx = self.ctx;
        for hasher in 0..ctx.fns.len() {
            let own = &ctx.own[hasher];
            if own.returns.iter().any(|statement| {
                statement
                    .argument
                    .as_ref()
                    .is_some_and(|argument| !is_void0(argument))
            }) {
                continue;
            }
            for &step in &own.nested {
                if ctx.fns[step].formal_count() != 0 {
                    continue;
                }
                let handed_off = own.calls.iter().any(|call| {
                    call_args(call)
                        .iter()
                        .any(|argument| ctx.resolve_fn(argument) == Some(step))
                });
                if !handed_off {
                    continue;
                }
                let Some((kind, step_evidence)) = self.hash_batch_step(hasher, step) else {
                    continue;
                };
                let (role, step_role, input) = match kind {
                    HashInput::Words => (
                        TYPED_ARRAY_HASH_ASYNC,
                        TYPED_ARRAY_HASH_BATCH,
                        "Int32 words of the slot-0 view's buffer",
                    ),
                    HashInput::CharPairs => (
                        STRING_HASH_ASYNC,
                        STRING_HASH_BATCH,
                        "UTF-16 char pairs of the slot-0 string",
                    ),
                };
                self.push(
                    role,
                    hasher,
                    Confidence::High,
                    vec![
                        format!("hands its nested batch step {} to a scheduler call and returns nothing", span_text(ctx.fns[step].span)),
                        format!("hashes {input} with the signed 32-bit multiplier-31 recurrence"),
                    ],
                );
                self.push(step_role, step, Confidence::High, step_evidence);
            }
        }
    }

    fn hash_batch_step(&self, hasher: usize, step: usize) -> Option<(HashInput, Vec<String>)> {
        let ctx = self.ctx;
        let own = &ctx.own[step];
        if own.loops == 0 {
            return None;
        }
        let returns_continue = own.returns.iter().any(|statement| {
            statement
                .argument
                .as_ref()
                .is_some_and(|argument| is_bool(argument, true))
        });
        let returns_done = own.returns.iter().any(|statement| {
            statement
                .argument
                .as_ref()
                .is_some_and(|argument| is_bool(argument, false))
        });
        if !(returns_continue && returns_done) {
            return None;
        }
        let mut kinds = BTreeSet::new();
        let mut accumulators = BTreeSet::new();
        for assignment in &own.assignments {
            let Some((accumulator, element)) = self.rolling_step(assignment) else {
                continue;
            };
            accumulators.insert(accumulator);
            if let Some(kind) = self.hash_input(hasher, element) {
                kinds.insert(kind);
            }
        }
        let [kind] = kinds.iter().copied().collect::<Vec<_>>()[..] else {
            return None;
        };
        let [accumulator] = accumulators.iter().copied().collect::<Vec<_>>()[..] else {
            return None;
        };
        // The final value goes to a callback slot of the hasher.
        let delivers = own.calls.iter().any(|call| {
            let (callee, args) = invocation(call);
            matches!(args.as_slice(), [value] if ctx.symbol(value) == Some(accumulator))
                && ctx.value_slot(callee, hasher).is_some()
        });
        // A caught error goes to another slot of the hasher.
        let forwards_error = own.tries.iter().any(|statement| {
            let Some(handler) = statement.handler.as_ref() else {
                return false;
            };
            handler.body.body.iter().any(|statement| {
                statement_call(statement).is_some_and(|call| {
                    let (callee, args) = invocation(call);
                    args.len() == 1 && ctx.value_slot(callee, hasher).is_some()
                })
            })
        });
        if !(delivers && forwards_error) {
            return None;
        }
        Some((
            kind,
            vec![
                "advances the rolling hash for one bounded batch in a loop".to_string(),
                "returns `true` to be rescheduled and `false` once the hash is delivered"
                    .to_string(),
                "delivers the hash to one hasher slot and a caught error to another".to_string(),
            ],
        ))
    }

    /// `h = ((h << 5) - h + x) | 0`: the accumulator and the element `x`.
    fn rolling_step<'e>(
        &self,
        assignment: &'e AssignmentExpression<'a>,
    ) -> Option<(SymbolId, &'e Expression<'a>)> {
        let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
            return None;
        };
        if assignment.operator != AssignmentOperator::Assign {
            return None;
        }
        let accumulator = analysis::identifier_symbol(identifier, self.ctx.scoping)?;
        let is_accumulator =
            |expression: &Expression<'a>| self.ctx.symbol(expression) == Some(accumulator);
        let Expression::BinaryExpression(truncate) = strip(&assignment.right) else {
            return None;
        };
        if truncate.operator != BinaryOperator::BitwiseOR || !is_number(&truncate.right, 0.0) {
            return None;
        }
        let Expression::BinaryExpression(add) = strip(&truncate.left) else {
            return None;
        };
        if add.operator != BinaryOperator::Addition {
            return None;
        }
        let Expression::BinaryExpression(subtract) = strip(&add.left) else {
            return None;
        };
        if subtract.operator != BinaryOperator::Subtraction || !is_accumulator(&subtract.right) {
            return None;
        }
        let Expression::BinaryExpression(shift) = strip(&subtract.left) else {
            return None;
        };
        if shift.operator != BinaryOperator::ShiftLeft
            || !is_number(&shift.right, 5.0)
            || !is_accumulator(&shift.left)
        {
            return None;
        }
        Some((accumulator, &add.right))
    }

    fn hash_input(&self, hasher: usize, element: &Expression<'a>) -> Option<HashInput> {
        let ctx = self.ctx;
        match strip(element) {
            // `words[i]`, where `words` is a constructed view and the hasher
            // reads its slot-0 view's `buffer`.
            Expression::ComputedMemberExpression(word) => {
                let words = ctx.symbol(&word.object)?;
                let constructed = ctx.has_origin(words, |value| {
                    matches!(strip(value), Expression::NewExpression(_))
                });
                let reads_buffer = ctx.own[hasher].members.iter().any(|(object, name)| {
                    *name == "buffer" && ctx.value_slot(object, hasher) == Some(0)
                });
                (constructed && reads_buffer).then_some(HashInput::Words)
            }
            // `code(s[i]) << 16 | code(s[i + 1])` over the slot-0 string.
            Expression::BinaryExpression(pair) if pair.operator == BinaryOperator::BitwiseOR => {
                let Expression::BinaryExpression(high) = strip(&pair.left) else {
                    return None;
                };
                if high.operator != BinaryOperator::ShiftLeft || !is_number(&high.right, 16.0) {
                    return None;
                }
                let reads_string = |expression: &Expression<'a>| {
                    let Expression::CallExpression(call) = strip(expression) else {
                        return false;
                    };
                    call_args(call).iter().any(|argument| {
                        matches!(strip(argument), Expression::ComputedMemberExpression(character)
                            if ctx.value_slot(&character.object, hasher) == Some(0))
                    })
                };
                (reads_string(&high.left) && reads_string(&pair.right))
                    .then_some(HashInput::CharPairs)
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // (d) Interception wrappers
    // -----------------------------------------------------------------------

    /// The wrappers an interceptor installer puts on a property descriptor. The
    /// installer is the existing `install_method_interceptor` /
    /// `install_accessor_interceptor` shape (its own body writes the
    /// `onBefore*`/`onAfter*` hook keys). A wrapper is a function nested
    /// directly in the installer, assigned at the installer's own level to a
    /// descriptor's `get`, `set` or `value`, whose own body notifies at least
    /// two hook channels that the installer created with a proven callback
    /// registry factory, and honours a `bypassResult`.
    ///
    /// Not a wrapper: a plain getter with no hooks, a Proxy `get` trap (an
    /// object-literal key, not a descriptor write), a DOM event handler.
    fn interception_wrappers(&mut self, registries: &[usize]) {
        let ctx = self.ctx;
        if registries.is_empty() {
            return;
        }
        for installer in 0..ctx.fns.len() {
            let keys = &ctx.own[installer].written_keys;
            let method = keys.contains("onBeforeInvoke") && keys.contains("onAfterInvoke");
            let accessor = keys.contains("onBeforeGet") && keys.contains("onBeforeSet");
            if !(method || accessor) {
                continue;
            }
            for assignment in &ctx.own[installer].assignments {
                let Some((_, key)) = assignment_target_member(&assignment.left) else {
                    continue;
                };
                let role = match (key, method, accessor) {
                    ("get", _, true) => INTERCEPTED_GETTER,
                    ("set", _, true) => INTERCEPTED_SETTER,
                    ("value", true, _) => INTERCEPTED_METHOD,
                    _ => continue,
                };
                let Some(wrapper) = ctx.resolve_fn(&assignment.right) else {
                    continue;
                };
                if ctx.fns[wrapper].parent != Some(installer) || ctx.fns[wrapper].is_arrow() {
                    continue;
                }
                let channels = self.notified_channels(wrapper, registries);
                let bypass = ctx.own[wrapper]
                    .members
                    .iter()
                    .any(|(_, name)| *name == "bypassResult");
                if channels < 2 || !bypass {
                    continue;
                }
                let entry = &ctx.fns[wrapper];
                let reads_value = ctx.own[wrapper].reads_slot(entry, 0);
                let arity_ok = match role {
                    INTERCEPTED_GETTER => !reads_value,
                    INTERCEPTED_SETTER => reads_value && entry.formal_count() <= 1,
                    _ => true,
                };
                if !arity_ok {
                    continue;
                }
                self.push(
                    role,
                    wrapper,
                    Confidence::Medium,
                    vec![
                        format!("installed as the descriptor `{key}` by an interceptor installer"),
                        format!("notifies {channels} hook channels created by a proven callback registry factory before and after the original"),
                        "honours a hook's bypassResult".to_string(),
                    ],
                );
            }
        }
    }

    /// Distinct hook channels a function calls `.notify` on. A channel is a
    /// binding whose every origin is `void 0` or a call of a proven registry
    /// factory, with at least one such call.
    fn notified_channels(&self, index: usize, registries: &[usize]) -> usize {
        let ctx = self.ctx;
        let mut channels = BTreeSet::new();
        for call in &ctx.own[index].calls {
            let Some(channel) = ctx.calls_method_on(call, "notify") else {
                continue;
            };
            let Some(origins) = ctx.origins(channel) else {
                continue;
            };
            let from_registry = |value: &Expression<'a>| {
                matches!(strip(value), Expression::CallExpression(created)
                    if ctx.resolve_fn(&created.callee).is_some_and(|factory| registries.contains(&factory)))
            };
            if origins.iter().any(|origin| from_registry(origin.value))
                && origins
                    .iter()
                    .all(|origin| is_void0(origin.value) || from_registry(origin.value))
            {
                channels.insert(channel);
            }
        }
        channels.len()
    }

    // -----------------------------------------------------------------------
    // Outcomes
    // -----------------------------------------------------------------------

    fn finish(self) -> Run {
        let ctx = self.ctx;
        let mut refusals = self.refusals;
        let mut outcomes: Vec<(usize, PurposeOutcome)> = Vec::new();
        let order = |role: &str| {
            ROLES
                .iter()
                .position(|spec| spec.role == role)
                .unwrap_or(usize::MAX)
        };

        // One function with two different roles is never named.
        let mut roles_of: BTreeMap<usize, BTreeSet<&'static str>> = BTreeMap::new();
        for candidate in &self.candidates {
            roles_of
                .entry(candidate.function)
                .or_default()
                .insert(candidate.role);
        }
        let mut tied: BTreeSet<usize> = BTreeSet::new();
        for (role, functions, reason) in &self.ties {
            tied.extend(functions.iter().copied());
            outcomes.push((
                order(role),
                PurposeOutcome::Ambiguous {
                    role,
                    candidates: functions.iter().map(|index| ctx.fns[*index].span).collect(),
                    reason: reason.clone(),
                },
            ));
        }
        for (function, roles) in &roles_of {
            if roles.len() > 1 {
                tied.insert(*function);
                let names: Vec<&str> = roles.iter().copied().collect();
                outcomes.push((
                    order(names[0]),
                    PurposeOutcome::Ambiguous {
                        role: names[0],
                        candidates: vec![ctx.fns[*function].span],
                        reason: format!("one function matched the roles {}", names.join(" and ")),
                    },
                ));
            }
        }

        // Resolve bindings; refuse mutated ones.
        struct Named {
            role: &'static str,
            function: usize,
            scope: Option<ScopeId>,
            alias_scope: Option<ScopeId>,
            finding: PurposeFinding,
        }
        let mut named: Vec<Named> = Vec::new();
        let mut seen: BTreeSet<(usize, &'static str)> = BTreeSet::new();
        for candidate in self.candidates {
            if tied.contains(&candidate.function)
                || !seen.insert((candidate.function, candidate.role))
            {
                continue;
            }
            let spec = spec(candidate.role);
            let binding = match ctx.binding_of(candidate.function) {
                Ok(binding) => binding,
                Err(reason) => {
                    refusals.push(Refusal {
                        role: candidate.role,
                        span: ctx.fns[candidate.function].span,
                        reason,
                    });
                    continue;
                }
            };
            let mut evidence = candidate.evidence;
            let mut holder_alias_name = None;
            let mut alias_scope = None;
            if let (Some(symbol), Some(alias_name)) = (binding, spec.alias) {
                match ctx.aliases_of(symbol).as_slice() {
                    [] => {}
                    [alias] => {
                        holder_alias_name = Some(alias_name);
                        alias_scope = Some(ctx.scoping.symbol_scope_id(*alias));
                        evidence.push(format!(
                            "single holder alias declared at {}",
                            span_text(ctx.scoping.symbol_span(*alias))
                        ));
                    }
                    aliases => evidence.push(format!(
                        "{} holder aliases; no alias name is proposed",
                        aliases.len()
                    )),
                }
            }
            if binding.is_none() {
                evidence
                    .push("the function has no editable binding; role metadata only".to_string());
            }
            named.push(Named {
                role: candidate.role,
                function: candidate.function,
                scope: binding.map(|symbol| ctx.scoping.symbol_scope_id(symbol)),
                alias_scope,
                finding: PurposeFinding {
                    role: candidate.role,
                    action_name: spec.action,
                    function_span: ctx.fns[candidate.function].span,
                    holder_alias_name,
                    confidence: candidate.confidence,
                    evidence,
                },
            });
        }

        // Two identical names in one scope are a tie, never a first winner.
        let mut by_name: BTreeMap<(&'static str, ScopeId), Vec<usize>> = BTreeMap::new();
        for (position, entry) in named.iter().enumerate() {
            if let Some(scope) = entry.scope {
                by_name
                    .entry((entry.finding.action_name, scope))
                    .or_default()
                    .push(position);
            }
            if let (Some(alias), Some(scope)) = (entry.finding.holder_alias_name, entry.alias_scope)
            {
                by_name.entry((alias, scope)).or_default().push(position);
            }
        }
        let mut colliding: BTreeSet<usize> = BTreeSet::new();
        let mut reported_ties: BTreeSet<Vec<usize>> = BTreeSet::new();
        for ((name, _), positions) in &by_name {
            let functions: BTreeSet<usize> = positions
                .iter()
                .map(|position| named[*position].function)
                .collect();
            if functions.len() < 2 {
                continue;
            }
            colliding.extend(positions.iter().copied());
            // A function name and its alias name usually collide together:
            // one tie per set of functions.
            if !reported_ties.insert(functions.iter().copied().collect()) {
                continue;
            }
            let first = &named[positions[0]];
            outcomes.push((
                order(first.role),
                PurposeOutcome::Ambiguous {
                    role: first.role,
                    candidates: functions.iter().map(|index| ctx.fns[*index].span).collect(),
                    reason: format!(
                        "{} proven functions would receive `{name}` in one scope: {}",
                        functions.len(),
                        functions
                            .iter()
                            .map(|index| ctx.label(*index))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                },
            ));
        }
        for (position, entry) in named.into_iter().enumerate() {
            if !colliding.contains(&position) {
                outcomes.push((order(entry.role), PurposeOutcome::Found(entry.finding)));
            }
        }

        let reported: BTreeSet<&'static str> = outcomes
            .iter()
            .filter_map(|(_, outcome)| match outcome {
                PurposeOutcome::Found(finding) => Some(finding.role),
                PurposeOutcome::Ambiguous { role, .. } => Some(*role),
                PurposeOutcome::NotFound { .. } => None,
            })
            .collect();
        for (position, spec) in ROLES.iter().enumerate() {
            if !reported.contains(spec.role) {
                outcomes.push((position, PurposeOutcome::NotFound { role: spec.role }));
            }
        }
        outcomes.sort_by_key(|(position, outcome)| {
            let start = match outcome {
                PurposeOutcome::Found(finding) => finding.function_span.start,
                PurposeOutcome::Ambiguous { candidates, .. } => {
                    candidates.first().map(|span| span.start).unwrap_or(0)
                }
                PurposeOutcome::NotFound { .. } => 0,
            };
            (*position, start)
        });
        refusals.sort_by_key(|refusal| (order(refusal.role), refusal.span.start));
        Run {
            outcomes: outcomes.into_iter().map(|(_, outcome)| outcome).collect(),
            refusals,
        }
    }
}

/// Which spelling established the pending list's defaulted initial items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InitialDefault {
    /// `slot === void 0 ? [] : slot` as an expression value.
    Conditional,
    /// `if (slot === void 0) { local = []; } else { local = slot; }`.
    Statement,
}

struct QueueApi {
    api: SymbolId,
    methods: Vec<(&'static str, usize, Vec<String>)>,
    pending: SymbolId,
    initial_slot: usize,
    initial_default: InitialDefault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum HashInput {
    Words,
    CharPairs,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_ctx(source: &str, check: impl FnOnce(&Ctx<'_, '_>)) {
        let allocator = Allocator::default();
        let program = analysis::parse_program(&allocator, source).expect("parse");
        let (scoping, nodes) = analysis::analyze(&program);
        let ctx = Ctx::new(&program, &scoping, &nodes);
        check(&ctx);
    }

    fn fn_at(ctx: &Ctx<'_, '_>, source: &str, needle: &str) -> usize {
        let start = source.find(needle).expect("needle") as u32;
        ctx.fns
            .iter()
            .position(|entry| entry.span.start == start)
            .expect("function at needle")
    }

    #[test]
    fn formal_and_arguments_are_one_slot() {
        let source = "function named(a, b) { let x = b; return x; }\n\
                      function trace() { let x; x = arguments[1]; return () => arguments[0]; }\n\
                      function written(a) { a = 1; return a; }\n\
                      function unsafe(a) { arguments[0] = 2; return a; }";
        with_ctx(source, |ctx| {
            let named = fn_at(ctx, source, "function named");
            let trace = fn_at(ctx, source, "function trace");
            let written = fn_at(ctx, source, "function written");
            let unsafe_fn = fn_at(ctx, source, "function unsafe");
            let value = |index: usize| {
                ctx.own[index]
                    .returns
                    .first()
                    .and_then(|statement| statement.argument.as_ref())
                    .expect("return value")
            };
            assert_eq!(ctx.value_slot(value(named), named), Some(1));
            let alias = ctx.own[trace]
                .assignments
                .first()
                .map(|assignment| &assignment.right)
                .expect("assignment");
            assert_eq!(ctx.value_slot(alias, trace), Some(1));
            // The arrow's `arguments[0]` is the enclosing function's slot 0.
            let arrow = ctx.own[trace].nested[0];
            let Some(Statement::ExpressionStatement(body)) = ctx.fns[arrow].statements().first()
            else {
                panic!("arrow body");
            };
            assert_eq!(ctx.value_slot(&body.expression, trace), Some(0));
            assert_eq!(ctx.value_slot(&body.expression, arrow), None);
            assert_eq!(ctx.value_slot(value(written), written), None);
            assert_eq!(ctx.value_slot(value(unsafe_fn), unsafe_fn), None);
        });
    }
}
