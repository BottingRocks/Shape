use super::*;

/// Options for direct Oxc emission. Every external symbolic input must appear
/// in `external_parameters`; no input is silently serialized into the output.
///
/// VM storage is intentionally absent from emitted computation, so the
/// `context_parameter` field is retained only for source compatibility and is
/// ignored. A complete trace that reaches this emitter has already transferred
/// its useful values into the graph; the generated function therefore receives
/// only its external inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitOptions {
    pub function_name: String,
    /// Retained for API compatibility; emitted functions do not receive it.
    pub context_parameter: String,
    pub external_parameters: Vec<String>,
    /// Simplify bindings while emitting: a `let` that is never reassigned
    /// and holds a constant, or copies another such binding or a free name,
    /// is written in place at every read; a pure `let` nothing reads is left
    /// out. Off by default, so the output keeps one statement per trace
    /// statement for [`crate::cleanup_emitted_javascript`] and provenance
    /// maps.
    pub simplify_bindings: bool,
    /// Emit a trace the walk did not finish (stopped, budget, unsupported)
    /// instead of refusing it. Only `trace thread --partial` sets this, for
    /// inspecting where a walk stopped; whole programs and the corpus never
    /// emit incomplete code.
    pub allow_incomplete: bool,
}

impl Default for EmitOptions {
    fn default() -> Self {
        Self {
            function_name: "trace".into(),
            context_parameter: "context".into(),
            external_parameters: Vec::new(),
            simplify_bindings: false,
            allow_incomplete: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedStatementMap {
    pub trace_statement: TraceStatementId,
    pub emitted_statement: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedJavaScript {
    pub code: String,
    pub statement_map: Vec<EmittedStatementMap>,
    pub node_map: BTreeMap<TraceNodeId, usize>,
    /// Stable caller-visible mapping for existing object identities.  Each
    /// identity is one function parameter and every reference reuses it.
    pub object_parameters: BTreeMap<u64, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    IncompleteTrace(String),
    MissingExternal(String),
    InvalidIdentifier(String),
    InvalidNode(TraceNodeId),
    InvalidArena(String),
    ExpressionCycle(TraceNodeId),
    ExpressionTooDeep { node: TraceNodeId, limit: usize },
    ExpressionTooLarge { node: TraceNodeId, limit: usize },
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteTrace(v) => write!(f, "cannot emit incomplete trace: {v}"),
            Self::MissingExternal(v) => {
                write!(f, "external input {v:?} was not supplied to emitter")
            }
            Self::InvalidIdentifier(v) => write!(f, "invalid identifier {v:?}"),
            Self::InvalidNode(v) => write!(f, "trace references missing node {v}"),
            Self::InvalidArena(v) => write!(f, "invalid trace arena: {v}"),
            Self::ExpressionCycle(v) => write!(f, "trace expression contains a cycle at node {v}"),
            Self::ExpressionTooDeep { node, limit } => {
                write!(
                    f,
                    "trace expression at node {node} exceeds depth limit {limit}"
                )
            }
            Self::ExpressionTooLarge { node, limit } => {
                write!(
                    f,
                    "trace expression at node {node} exceeds expanded-cost limit {limit}"
                )
            }
        }
    }
}
impl std::error::Error for EmitError {}

#[derive(Debug, Clone)]
pub(super) struct SlotReadPlan {
    pub(super) slot: u32,
    pub(super) value: TraceNodeId,
}
#[derive(Debug, Clone)]
pub(super) struct SlotWritePlan {
    pub(super) slot: u32,
    pub(super) value: TraceNodeId,
}
#[derive(Debug, Default)]
pub(super) struct SlotEmissionPlan {
    pub(super) before: BTreeMap<TraceStatementId, Vec<SlotReadPlan>>,
    pub(super) after: BTreeMap<TraceStatementId, Vec<SlotWritePlan>>,
    pub(super) reads: HashMap<(TraceStatementId, TraceNodeId), u32>,
    pub(super) slots: BTreeSet<u32>,
}

/// Borrowed hash key over the full [`TraceProvenance`] equality. `TraceProvenance`
/// deliberately has no `Hash`, so planning keeps its own call-local key and hashes
/// exactly the fields its `Eq` covers: handler/table identity, the semantic
/// string, step order, the monotonic trace step, and the source span. Equality is
/// the derived `==`, so hash collisions cannot merge distinct provenances.
pub(super) struct ProvenanceKey<'a>(&'a TraceProvenance);

impl PartialEq for ProvenanceKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for ProvenanceKey<'_> {}
impl std::hash::Hash for ProvenanceKey<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let provenance = self.0;
        provenance.handler_index.hash(state);
        provenance.table_key.hash(state);
        provenance.semantic_identity.hash(state);
        provenance.step_order.hash(state);
        provenance.trace_step.hash(state);
        provenance.span.start.hash(state);
        provenance.span.end.hash(state);
    }
}

/// Immutable call-local indexes over one validated arena, built once per
/// [`plan_slot_emission`]. They replace the per-event full-statement scans in
/// `unique_event_statement`, `unique_value_definition`,
/// `reusable_value_is_proven` and `constant_number`. Every bucket keeps original
/// statement order, so candidate multiplicity and the "exactly one" ambiguity
/// rule are unchanged; the maps are only ever queried by key and their vectors
/// iterated in order, so map iteration order never reaches the output.
pub(super) struct SlotIndex<'a> {
    /// Statement ids grouped by their exact provenance, in statement order.
    pub(super) by_provenance: HashMap<ProvenanceKey<'a>, Vec<TraceStatementId>>,
    /// `Let`/`Assign` targets by binding name, in statement order. Each entry is
    /// the defining statement and the value node it binds.
    pub(super) definitions: HashMap<&'a str, Vec<(TraceStatementId, TraceNodeId)>>,
    /// Canonical id of each statement's block path; statements with equal paths
    /// share an id. Replaces the `path_of(...) == path_of(...)` slice compare.
    pub(super) path_ids: Vec<u32>,
}

impl<'a> SlotIndex<'a> {
    pub(super) fn build(arena: &'a TraceArena, paths: &[Vec<(TraceStatementId, u8)>]) -> Self {
        let statements = arena.statements.len();
        let mut by_provenance: HashMap<ProvenanceKey<'a>, Vec<TraceStatementId>> =
            HashMap::with_capacity_and_hasher(statements, Default::default());
        let mut definitions: HashMap<&'a str, Vec<(TraceStatementId, TraceNodeId)>> =
            HashMap::with_capacity_and_hasher(statements, Default::default());
        for statement in &arena.statements {
            by_provenance
                .entry(ProvenanceKey(&statement.provenance))
                .or_default()
                .push(statement.id);
            if let TraceStatementKind::Let { name, value }
            | TraceStatementKind::Assign { name, value } = &statement.kind
            {
                definitions
                    .entry(name.as_str())
                    .or_default()
                    .push((statement.id, *value));
            }
        }
        let mut intern: HashMap<&[(TraceStatementId, u8)], u32> =
            HashMap::with_capacity_and_hasher(paths.len(), Default::default());
        let mut path_ids = vec![0u32; paths.len()];
        for (index, path) in paths.iter().enumerate() {
            let id = match intern.get(path.as_slice()) {
                Some(&id) => id,
                None => {
                    let id = intern.len() as u32;
                    intern.insert(path.as_slice(), id);
                    id
                }
            };
            path_ids[index] = id;
        }
        Self {
            by_provenance,
            definitions,
            path_ids,
        }
    }
}

/// Project only storage events tied to one original statement in one original
/// block. Ambiguous events stay in the existing SSA graph (writes are omitted).
pub(super) fn plan_slot_emission(report: &TraceReport) -> SlotEmissionPlan {
    let arena = &report.arena;
    let mut owned = vec![false; arena.statements.len()];
    for statement in &arena.statements {
        for child in block_children(&statement.kind) {
            owned[child as usize] = true;
        }
    }
    let roots: Vec<_> = arena
        .statements
        .iter()
        .filter(|s| !owned[s.id as usize])
        .map(|s| s.id)
        .collect();
    let mut paths = vec![Vec::new(); arena.statements.len()];
    collect_slot_paths(arena, &roots, &[], &mut paths);
    let index = SlotIndex::build(arena, &paths);
    let mut plan = SlotEmissionPlan::default();
    let mut reads: BTreeMap<TraceStatementId, Vec<(u32, TraceNodeId)>> = BTreeMap::new();
    for event in &arena.events {
        let TraceEventKind::MemoryRead { slot, value } = event.kind else {
            continue;
        };
        let Some(slot) = concrete_slot(arena, &index, slot) else {
            continue;
        };
        let Some(statement) = unique_event_statement(arena, &index, event, value) else {
            continue;
        };
        if slot_value_is_reusable(arena, value)
            && reusable_value_is_proven(&index, arena, value, statement)
        {
            reads.entry(statement).or_default().push((slot, value));
        }
    }
    for (statement, values) in reads {
        if values.len() != 1 || !path_is_safe(&paths[statement as usize], arena) {
            continue;
        }
        let (slot, value) = values[0];
        plan.before
            .entry(statement)
            .or_default()
            .push(SlotReadPlan { slot, value });
        plan.reads.insert((statement, value), slot);
        plan.slots.insert(slot);
    }
    for event in &arena.events {
        let TraceEventKind::MemoryWrite { slot, value } = event.kind else {
            continue;
        };
        let Some(slot) = concrete_slot(arena, &index, slot) else {
            continue;
        };
        if !matches!(
            arena.nodes.get(value as usize).map(|n| &n.kind),
            Some(TraceNodeKind::Identifier { .. })
        ) {
            continue;
        }
        let Some(definition) = unique_value_definition(arena, &index, value, event) else {
            continue;
        };
        if !path_is_safe(&paths[definition as usize], arena) {
            continue;
        }
        plan.after
            .entry(definition)
            .or_default()
            .push(SlotWritePlan { slot, value });
        plan.slots.insert(slot);
    }
    plan
}

pub(super) fn collect_slot_paths(
    arena: &TraceArena,
    ids: &[TraceStatementId],
    path: &[(TraceStatementId, u8)],
    paths: &mut [Vec<(TraceStatementId, u8)>],
) {
    for &id in ids {
        paths[id as usize] = path.to_vec();
    }
    for &id in ids {
        let children: Vec<(u8, &Vec<TraceStatementId>)> = match &arena.statements[id as usize].kind
        {
            TraceStatementKind::If {
                taken,
                fall_through,
                ..
            } => vec![(0, taken), (1, fall_through)],
            TraceStatementKind::Loop { body, .. } => vec![(2, body)],
            TraceStatementKind::Try {
                body,
                handler,
                finalizer,
                ..
            } => [(3, body), (4, handler)]
                .into_iter()
                .chain(finalizer.iter().map(|finalizer| (6, finalizer)))
                .collect(),
            TraceStatementKind::Function { body, .. } => vec![(5, body)],
            _ => Vec::new(),
        };
        for (arm, children) in children {
            let mut child_path = path.to_vec();
            child_path.push((id, arm));
            collect_slot_paths(arena, children, &child_path, paths);
        }
    }
}

pub(super) fn path_is_safe(path: &[(TraceStatementId, u8)], arena: &TraceArena) -> bool {
    path.iter().all(|(id, _)| {
        !matches!(
            arena.statements[*id as usize].kind,
            TraceStatementKind::Loop { .. }
                | TraceStatementKind::Try { .. }
                // A subroutine's function does not own the caller's slots.
                | TraceStatementKind::Function { .. }
        )
    })
}

pub(super) fn unique_event_statement(
    arena: &TraceArena,
    index: &SlotIndex,
    event: &TraceEvent,
    value: TraceNodeId,
) -> Option<TraceStatementId> {
    let bucket = index.by_provenance.get(&ProvenanceKey(&event.provenance))?;
    let mut candidate = None;
    for &id in bucket {
        if statement_contains_node(arena, &arena.statements[id as usize].kind, value) {
            if candidate.is_some() {
                return None;
            }
            candidate = Some(id);
        }
    }
    candidate
}

pub(super) fn statement_contains_node(
    arena: &TraceArena,
    kind: &TraceStatementKind,
    target: TraceNodeId,
) -> bool {
    let roots: Vec<_> = match kind {
        TraceStatementKind::Let { value, .. }
        | TraceStatementKind::Assign { value, .. }
        | TraceStatementKind::Evaluate { value }
        | TraceStatementKind::Return { value }
        | TraceStatementKind::Throw { value }
        | TraceStatementKind::If { test: value, .. } => vec![*value],
        TraceStatementKind::PropertyWrite { object, key, value } => vec![*object, *key, *value],
        _ => Vec::new(),
    };
    roots
        .into_iter()
        .any(|root| node_contains(arena, root, target))
}
pub(super) fn node_contains(arena: &TraceArena, node: TraceNodeId, target: TraceNodeId) -> bool {
    node == target
        || node_children(&arena.nodes[node as usize].kind)
            .into_iter()
            .any(|child| node_contains(arena, child, target))
}

pub(super) fn reusable_value_is_proven(
    index: &SlotIndex,
    arena: &TraceArena,
    value: TraceNodeId,
    use_statement: TraceStatementId,
) -> bool {
    match arena.nodes.get(value as usize).map(|n| &n.kind) {
        Some(TraceNodeKind::Identifier { name }) => {
            // `path_of(paths, use_statement)`; `validate_arena` keeps statement
            // ids in range, so the sentinel is never observed.
            let use_path = index
                .path_ids
                .get(use_statement as usize)
                .copied()
                .unwrap_or(u32::MAX);
            let mut count = 0usize;
            if let Some(definitions) = index.definitions.get(name.as_str()) {
                for &(id, _) in definitions {
                    if id < use_statement && index.path_ids[id as usize] == use_path {
                        count += 1;
                        if count > 1 {
                            return false;
                        }
                    }
                }
            }
            count == 1
        }
        Some(
            TraceNodeKind::Undefined
            | TraceNodeKind::Null
            | TraceNodeKind::Boolean(_)
            | TraceNodeKind::Number(_)
            | TraceNodeKind::String(_)
            | TraceNodeKind::Object { .. }
            | TraceNodeKind::External { .. },
        ) => true,
        _ => false,
    }
}

pub(super) fn unique_value_definition(
    arena: &TraceArena,
    index: &SlotIndex,
    value: TraceNodeId,
    event: &TraceEvent,
) -> Option<TraceStatementId> {
    let TraceNodeKind::Identifier { name } = &arena.nodes.get(value as usize)?.kind else {
        return None;
    };
    let bucket = index.by_provenance.get(&ProvenanceKey(&event.provenance))?;
    let mut candidate = None;
    for &id in bucket {
        if matches!(&arena.statements[id as usize].kind, TraceStatementKind::Let { name: n, .. } | TraceStatementKind::Assign { name: n, .. } if n == name)
        {
            if candidate.is_some() {
                return None;
            }
            candidate = Some(id);
        }
    }
    candidate
}

pub(super) fn concrete_slot(
    arena: &TraceArena,
    index: &SlotIndex,
    node: TraceNodeId,
) -> Option<u32> {
    let value = constant_number(arena, index, node, &mut HashSet::default())?;
    (value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value <= u32::MAX as f64)
        .then_some(value as u32)
}
pub(super) fn constant_number<'a>(
    arena: &'a TraceArena,
    index: &SlotIndex<'a>,
    node: TraceNodeId,
    visiting: &mut HashSet<&'a str>,
) -> Option<f64> {
    match &arena.nodes.get(node as usize)?.kind {
        TraceNodeKind::Number(raw) => raw.parse().ok(),
        TraceNodeKind::Identifier { name } => {
            // A binding in flight on this path is a cycle, not a permanent
            // unknown; the caller returns `None` and the whole resolution
            // declines, so the visiting set is only reused on success.
            let name = name.as_str();
            if !visiting.insert(name) {
                return None;
            }
            let mut values = Vec::new();
            let mut unknown = false;
            if let Some(definitions) = index.definitions.get(name) {
                for &(_, value) in definitions {
                    match constant_number(arena, index, value, visiting) {
                        Some(value) => values.push(value),
                        None => {
                            unknown = true;
                            break;
                        }
                    }
                }
            }
            visiting.remove(name);
            if unknown {
                return None;
            }
            // Every definition of the name must agree bit for bit; no definition
            // at all is not a proof.
            let first = *values.first()?;
            values
                .iter()
                .all(|v| v.to_bits() == first.to_bits())
                .then_some(first)
        }
        TraceNodeKind::Unary {
            operator: PrimitiveOperator::UnaryPlus,
            value,
        } => constant_number(arena, index, *value, visiting),
        TraceNodeKind::Unary {
            operator: PrimitiveOperator::UnaryMinus,
            value,
        } => Some(-constant_number(arena, index, *value, visiting)?),
        TraceNodeKind::Binary {
            operator,
            left,
            right,
        } => {
            let (left, right) = (
                constant_number(arena, index, *left, visiting)?,
                constant_number(arena, index, *right, visiting)?,
            );
            match operator {
                PrimitiveOperator::Subtract => Some(left - right),
                PrimitiveOperator::Multiply => Some(left * right),
                PrimitiveOperator::Divide if right != 0.0 => Some(left / right),
                _ => None,
            }
        }
        _ => None,
    }
}
pub(super) fn slot_value_is_reusable(arena: &TraceArena, node: TraceNodeId) -> bool {
    matches!(
        arena.nodes.get(node as usize).map(|n| &n.kind),
        Some(
            TraceNodeKind::Identifier { .. }
                | TraceNodeKind::Undefined
                | TraceNodeKind::Null
                | TraceNodeKind::Boolean(_)
                | TraceNodeKind::Number(_)
                | TraceNodeKind::String(_)
                | TraceNodeKind::Object { .. }
                | TraceNodeKind::External { .. }
        )
    )
}

/// Build a JavaScript function using Oxc's [`AstBuilder`], then print it with
/// Oxc codegen. No generated source is reparsed to construct the AST.
pub fn emit_javascript(
    report: &TraceReport,
    options: &EmitOptions,
) -> Result<EmittedJavaScript, EmitError> {
    emit_with(report, options, false, None).map(|(emitted, _)| emitted)
}

/// One VM function a traced function creates, as program mode emits it: the
/// empty `function <name>() {}` its code replaces, and which function it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramThread {
    pub name: String,
    pub descriptor: usize,
    pub pc: usize,
    pub state: usize,
    /// The slots it is given, each the creator's slot of the same number.
    pub cells: Vec<u32>,
}

/// What program mode tells of one emitted function besides its code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramFacts {
    /// The functions it creates, in the order their markers appear.
    pub threads: Vec<ProgramThread>,
    /// The slots it shares with its creator: it reads them as `$KEY_<slot>`
    /// without declaring them.
    pub captured: BTreeSet<u32>,
    /// How often it names itself ([`PROGRAM_SELF`]) besides its header.
    pub self_names: usize,
    /// How often it reads its receiver ([`PROGRAM_THIS`]).
    pub this_names: usize,
}

/// The name program mode gives a function in its own code; the program
/// writes the name it is nested under there.
pub const PROGRAM_SELF: &str = "$THREAD_SELF";

/// What program mode writes for the function's receiver, for the caller to
/// write `this` there once its cleanups are done: they take a `this` for
/// the global object, which a strict function's is not.
pub const PROGRAM_THIS: &str = "$THIS";

/// Emit `report` for `shape trace program`: no parameters (inputs are
/// `arguments[i]`, `arguments`, `this` and the function itself), every
/// memory slot a variable `$KEY_<slot>` (a shared cell's box unwrapped), and
/// each function it creates an empty `function $THREAD_<state>_<pc>() {}`
/// for the program to nest that function's code in.
pub fn emit_program_function(
    report: &TraceReport,
    options: &EmitOptions,
) -> Result<(EmittedJavaScript, ProgramFacts), EmitError> {
    emit_with(report, options, true, None)
}

/// Emit a linked VM function with an owner identity in every generated value
/// name, so independently traced functions never reuse a `$SSA` identifier.
pub fn emit_program_function_in_namespace(
    report: &TraceReport,
    options: &EmitOptions,
    namespace: &str,
) -> Result<(EmittedJavaScript, ProgramFacts), EmitError> {
    if namespace.is_empty()
        || !namespace
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(EmitError::InvalidIdentifier(namespace.into()));
    }
    emit_with(report, options, true, Some(namespace))
}

pub(super) struct Emitter<'a, 'b> {
    pub(super) report: &'a TraceReport,
    pub(super) builder: AstBuilder<'b>,
    pub(super) external: BTreeMap<String, String>,
    pub(super) object_parameters: BTreeMap<u64, String>,
    pub(super) local_names: HashMap<&'a str, String>,
    /// The top-level statement each node was emitted in, or `usize::MAX`.
    pub(super) node_map: Vec<usize>,
    /// The top-level statement being emitted; nested statements map to it.
    pub(super) current_statement: usize,
    pub(super) statement_map: Vec<EmittedStatementMap>,
    /// Unread pure `let`s, left out.
    pub(super) dead: Vec<bool>,
    /// The `throw`s that end a `finally` traced after a throw, rethrowing
    /// the exception: the emitted `finally` does that itself.
    pub(super) rethrows: BTreeSet<TraceStatementId>,
    /// Bindings written in place at every read; see [`inline_bindings`].
    pub(super) inline: HashMap<&'a str, TraceNodeId>,
    pub(super) slot_plan: SlotEmissionPlan,
    pub(super) slot_names: BTreeMap<u32, String>,
    pub(super) declared_slots: BTreeSet<String>,
    pub(super) current_trace_statement: TraceStatementId,
    pub(super) suppress_slot_reads: bool,
    /// Set in program mode (see [`emit_program_function`]).
    pub(super) program: Option<ProgramEmission>,
}

/// Program mode's state while one function is emitted.
pub(super) struct ProgramEmission {
    pub(super) cells: ProgramCells,
    pub(super) facts: ProgramFacts,
    /// Whether the function reads `this`.
    pub(super) strict: bool,
    /// How many function statements the emission is inside: their `this`
    /// and `arguments` are not the thread's.
    pub(super) function_depth: usize,
}

impl<'a, 'b> Emitter<'a, 'b> {
    /// Whether program mode leaves `statement` out: a shared cell's
    /// `$CELL_<slot> = captured<slot>`, or a `Declare` of only shared cells.
    pub(super) fn program_skips(&self, statement: &TraceStatement) -> bool {
        let Some(emission) = &self.program else {
            return false;
        };
        match &statement.kind {
            TraceStatementKind::Assign { .. } => emission.cells.aliases.contains(&statement.id),
            TraceStatementKind::Declare { names } => names
                .iter()
                .all(|name| emission.cells.captured.contains_key(name.as_str())),
            _ => false,
        }
    }
    /// The cell slot of `node` when it reads a cell variable `$CELL_<slot>`.
    pub(super) fn cell_variable(&self, node: TraceNodeId) -> Option<u32> {
        self.program.as_ref()?;
        match &self.report.arena.nodes.get(node as usize)?.kind {
            TraceNodeKind::Identifier { name } if !self.inline.contains_key(name.as_str()) => {
                cell_slot(name)
            }
            _ => None,
        }
    }
    /// Whether `key` is the string `"value"`, a cell's one property.
    pub(super) fn is_cell_value_key(&self, key: TraceNodeId) -> bool {
        matches!(
            self.report.arena.nodes.get(key as usize).map(|node| &node.kind),
            Some(TraceNodeKind::String(key)) if key == "value"
        )
    }
    /// Program mode's variable of cell slot `slot`: the function's own
    /// `$KEY_<slot>` or the creator's.
    pub(super) fn cell_key(&self, slot: u32) -> Result<String, EmitError> {
        let name = format!("$CELL_{slot}");
        let emission = self.program.as_ref().expect("program mode");
        if emission.cells.captured.contains_key(&name) {
            return Ok(program_key(slot));
        }
        Ok(self.local_name(&name)?.to_string())
    }
    /// `name = value`.
    pub(super) fn assignment(
        &mut self,
        name: &str,
        value: TraceNodeId,
    ) -> Result<Statement<'b>, EmitError> {
        let b = self.builder;
        let value = self.expression(value)?;
        Ok(Statement::ExpressionStatement(
            b.alloc_expression_statement(
                SPAN,
                b.expression_assignment(
                    SPAN,
                    oxc_syntax::operator::AssignmentOperator::Assign,
                    b.simple_assignment_target_assignment_target_identifier(SPAN, b.ident(name))
                        .into(),
                    value,
                ),
            ),
        ))
    }
    /// Program mode's form of `statement`, when it has one: a cell's box
    /// unwrapped into its variable, and a creation as the empty function the
    /// program nests the created function's code in.
    pub(super) fn program_statement(
        &mut self,
        statement: &TraceStatement,
    ) -> Result<Option<Statement<'b>>, EmitError> {
        if self.program.is_none() {
            return Ok(None);
        }
        let arena = &self.report.arena;
        let kind = |node: TraceNodeId| arena.nodes.get(node as usize).map(|node| &node.kind);
        let invalid = |what: String| EmitError::InvalidArena(format!("program mode: {what}"));
        Ok(Some(match &statement.kind {
            TraceStatementKind::Assign { name, value } if cell_slot(name).is_some() => {
                // A new cell holding a value, or the function itself (see
                // [`program_cells`]).
                let held = match kind(*value) {
                    Some(TraceNodeKind::ObjectLiteral { values, .. }) => values[0],
                    Some(TraceNodeKind::External { name: input }) if input == "callee" => *value,
                    _ => return Err(invalid(format!("{name} is given what is not a cell"))),
                };
                let key = self.cell_key(cell_slot(name).unwrap_or_default())?;
                self.assignment(&key, held)?
            }
            TraceStatementKind::PropertyWrite { object, key, value } => {
                let Some(slot) = self.cell_variable(*object) else {
                    return Ok(None);
                };
                if !self.is_cell_value_key(*key) {
                    return Err(invalid(format!(
                        "a write to cell {slot} is not of its value"
                    )));
                }
                let key = self.cell_key(slot)?;
                self.assignment(&key, *value)?
            }
            TraceStatementKind::Let { name, value } => {
                let Some(TraceNodeKind::VmFunction {
                    descriptor,
                    byte_offset,
                    dispatch_state,
                    memory,
                }) = kind(*value)
                else {
                    return Ok(None);
                };
                // [`program_cells`] checked each is slot K as slot K.
                let cells = match memory.and_then(kind) {
                    Some(TraceNodeKind::ObjectLiteral { keys, .. }) => {
                        keys.iter().filter_map(|key| key.parse().ok()).collect()
                    }
                    _ => Vec::new(),
                };
                if self
                    .program
                    .as_ref()
                    .is_some_and(|emission| emission.function_depth > 0)
                {
                    return Err(invalid("a function statement creates a function".into()));
                }
                let name = self.local_name(name)?.to_string();
                if let Some(slot) = self.node_map.get_mut(*value as usize) {
                    *slot = self.current_statement;
                }
                let b = self.builder;
                let params = b.alloc_formal_parameters(
                    SPAN,
                    FormalParameterKind::FormalParameter,
                    b.vec(),
                    NONE,
                );
                let marker = Statement::FunctionDeclaration(b.alloc_function(
                    SPAN,
                    FunctionType::FunctionDeclaration,
                    Some(b.binding_identifier(SPAN, b.ident(&name))),
                    false,
                    false,
                    false,
                    NONE,
                    NONE,
                    params,
                    NONE,
                    Some(b.alloc_function_body(SPAN, b.vec(), b.vec())),
                ));
                let emission = self.program.as_mut().expect("program mode");
                emission.facts.threads.push(ProgramThread {
                    name,
                    descriptor: *descriptor,
                    pc: *byte_offset,
                    state: *dispatch_state,
                    cells,
                });
                marker
            }
            TraceStatementKind::Declare { names } => {
                let b = self.builder;
                let mut declarators = b.vec_with_capacity(names.len());
                for name in names {
                    if self.program_captures(name) {
                        continue;
                    }
                    declarators.push(b.variable_declarator(
                        SPAN,
                        VariableDeclarationKind::Let,
                        b.binding_pattern_binding_identifier(SPAN, b.ident(self.local_name(name)?)),
                        NONE,
                        None,
                        false,
                    ));
                }
                Statement::VariableDeclaration(b.alloc_variable_declaration(
                    SPAN,
                    VariableDeclarationKind::Let,
                    declarators,
                    false,
                ))
            }
            _ => return Ok(None),
        }))
    }
    pub(super) fn program_captures(&self, name: &str) -> bool {
        self.program
            .as_ref()
            .is_some_and(|emission| emission.cells.captured.contains_key(name))
    }
    /// A thread input in program mode, read in place.
    pub(super) fn program_input(&mut self, raw: &str) -> Result<Expression<'b>, EmitError> {
        let b = self.builder;
        let emission = self.program.as_mut().expect("program mode");
        let input = program_input(raw).ok_or_else(|| EmitError::MissingExternal(raw.into()))?;
        if emission.function_depth > 0 && input != ProgramInput::Callee {
            return Err(EmitError::InvalidArena(format!(
                "program mode: a function statement reads the thread's {raw}"
            )));
        }
        Ok(match input {
            ProgramInput::Argument(index) => Expression::from(b.member_expression_computed(
                SPAN,
                b.expression_identifier(SPAN, b.ident("arguments")),
                b.expression_numeric_literal(SPAN, f64::from(index), None, NumberBase::Decimal),
                false,
            )),
            ProgramInput::Arguments => b.expression_identifier(SPAN, b.ident("arguments")),
            ProgramInput::Receiver => {
                emission.strict = true;
                emission.facts.this_names += 1;
                b.expression_identifier(SPAN, b.ident(PROGRAM_THIS))
            }
            ProgramInput::Callee => {
                emission.facts.self_names += 1;
                b.expression_identifier(SPAN, b.ident(PROGRAM_SELF))
            }
            ProgramInput::Captured(slot) => {
                return Err(EmitError::InvalidArena(format!(
                    "program mode: the cell of slot {slot} is read other than as its variable"
                )))
            }
        })
    }
    pub(super) fn planned_before(
        &mut self,
        id: TraceStatementId,
        body: &mut oxc_allocator::Vec<'b, Statement<'b>>,
    ) -> Result<(), EmitError> {
        for plan in self.slot_plan.before.get(&id).cloned().unwrap_or_default() {
            body.push(self.slot_assignment(plan.slot, plan.value)?);
        }
        Ok(())
    }
    pub(super) fn planned_after(
        &mut self,
        id: TraceStatementId,
        body: &mut oxc_allocator::Vec<'b, Statement<'b>>,
    ) -> Result<(), EmitError> {
        for plan in self.slot_plan.after.get(&id).cloned().unwrap_or_default() {
            body.push(self.slot_assignment(plan.slot, plan.value)?);
        }
        Ok(())
    }
    pub(super) fn slot_assignment(
        &mut self,
        slot: u32,
        value: TraceNodeId,
    ) -> Result<Statement<'b>, EmitError> {
        let name = self
            .slot_names
            .get(&slot)
            .ok_or_else(|| EmitError::InvalidArena(format!("unplanned slot {slot}")))?
            .clone();
        let old = self.suppress_slot_reads;
        self.suppress_slot_reads = true;
        let expression = self.expression(value)?;
        self.suppress_slot_reads = old;
        let b = self.builder;
        if self.declared_slots.insert(name.clone()) {
            Ok(Statement::VariableDeclaration(
                b.alloc_variable_declaration(
                    SPAN,
                    VariableDeclarationKind::Var,
                    b.vec1(b.variable_declarator(
                        SPAN,
                        VariableDeclarationKind::Var,
                        b.binding_pattern_binding_identifier(SPAN, b.ident(&name)),
                        NONE,
                        Some(expression),
                        false,
                    )),
                    false,
                ),
            ))
        } else {
            Ok(Statement::ExpressionStatement(
                b.alloc_expression_statement(
                    SPAN,
                    b.expression_assignment(
                        SPAN,
                        oxc_syntax::operator::AssignmentOperator::Assign,
                        b.simple_assignment_target_assignment_target_identifier(
                            SPAN,
                            b.ident(&name),
                        )
                        .into(),
                        expression,
                    ),
                ),
            ))
        }
    }
    pub(super) fn statement(
        &mut self,
        statement: &TraceStatement,
    ) -> Result<Statement<'b>, EmitError> {
        self.current_trace_statement = statement.id;
        let b = self.builder;
        let span = SPAN;
        self.statement_map.push(EmittedStatementMap {
            trace_statement: statement.id,
            emitted_statement: self.current_statement,
        });
        if let Some(emitted) = self.program_statement(statement)? {
            return Ok(emitted);
        }
        Ok(match &statement.kind {
            TraceStatementKind::Let { name, value } => {
                Statement::VariableDeclaration(b.alloc_variable_declaration(
                    span,
                    VariableDeclarationKind::Let,
                    b.vec1(b.variable_declarator(
                        span,
                        VariableDeclarationKind::Let,
                        b.binding_pattern_binding_identifier(span, b.ident(self.local_name(name)?)),
                        NONE,
                        Some(self.expression(*value)?),
                        false,
                    )),
                    false,
                ))
            }
            TraceStatementKind::Assign { name, value } => Statement::ExpressionStatement(
                b.alloc_expression_statement(
                    span,
                    b.expression_assignment(
                        span,
                        oxc_syntax::operator::AssignmentOperator::Assign,
                        b.simple_assignment_target_assignment_target_identifier(
                            span,
                            b.ident(self.local_name(name)?),
                        )
                        .into(),
                        self.expression(*value)?,
                    ),
                ),
            ),
            TraceStatementKind::Evaluate { value } => Statement::ExpressionStatement(
                b.alloc_expression_statement(span, self.expression(*value)?),
            ),
            TraceStatementKind::Return { value } => Statement::ReturnStatement(
                b.alloc_return_statement(span, Some(self.expression(*value)?)),
            ),
            TraceStatementKind::Throw { value } => {
                Statement::ThrowStatement(b.alloc_throw_statement(span, self.expression(*value)?))
            }
            TraceStatementKind::PropertyWrite { object, key, value } => {
                let object = self.expression(*object)?;
                let key = self.expression(*key)?;
                let target = b.member_expression_computed(span, object, key, false);
                Statement::ExpressionStatement(b.alloc_expression_statement(
                    span,
                    b.expression_assignment(
                        span,
                        oxc_syntax::operator::AssignmentOperator::Assign,
                        SimpleAssignmentTarget::from(target).into(),
                        self.expression(*value)?,
                    ),
                ))
            }
            TraceStatementKind::Declare { names } => {
                let mut declarators = b.vec_with_capacity(names.len());
                for name in names {
                    declarators.push(b.variable_declarator(
                        span,
                        VariableDeclarationKind::Let,
                        b.binding_pattern_binding_identifier(span, b.ident(self.local_name(name)?)),
                        NONE,
                        None,
                        false,
                    ));
                }
                Statement::VariableDeclaration(b.alloc_variable_declaration(
                    span,
                    VariableDeclarationKind::Let,
                    declarators,
                    false,
                ))
            }
            TraceStatementKind::If {
                test,
                taken,
                fall_through,
            } => {
                // `if (!x) goto` skips an `if` body: print the body first.
                let (test, consequent, alternate) = match self.negated(*test) {
                    Some(inner) if !fall_through.is_empty() => {
                        (self.expression(inner)?, fall_through, taken)
                    }
                    _ if taken.is_empty() && !fall_through.is_empty() => {
                        let test = self.expression(*test)?;
                        (
                            b.expression_unary(span, UnaryOperator::LogicalNot, test),
                            fall_through,
                            taken,
                        )
                    }
                    _ => (self.expression(*test)?, taken, fall_through),
                };
                let consequent = self.arm(consequent)?;
                let alternate = if alternate.is_empty() {
                    None
                } else {
                    Some(self.arm(alternate)?)
                };
                b.statement_if(span, test, consequent, alternate)
            }
            TraceStatementKind::Loop { label, body } => {
                let body = self.arm(body)?;
                b.statement_labeled(
                    span,
                    b.label_identifier(span, b.ident(label)),
                    b.statement_for(span, None, None, None, body),
                )
            }
            TraceStatementKind::Continue { label } => {
                b.statement_continue(span, Some(b.label_identifier(span, b.ident(label))))
            }
            TraceStatementKind::Break { label } => {
                b.statement_break(span, Some(b.label_identifier(span, b.ident(label))))
            }
            TraceStatementKind::Function { name, params, body } => {
                if let Some(emission) = &mut self.program {
                    emission.function_depth += 1;
                }
                let body = self.block(body);
                if let Some(emission) = &mut self.program {
                    emission.function_depth -= 1;
                }
                let body = body?.unbox().body;
                let mut formal = b.vec_with_capacity(params.len());
                for param in params {
                    formal.push(b.formal_parameter(
                        SPAN,
                        b.vec(),
                        b.binding_pattern_binding_identifier(
                            SPAN,
                            b.ident(self.local_name(param)?),
                        ),
                        NONE,
                        NONE,
                        false,
                        None,
                        false,
                        false,
                    ));
                }
                let formal = b.alloc_formal_parameters(
                    SPAN,
                    FormalParameterKind::FormalParameter,
                    formal,
                    NONE,
                );
                Statement::FunctionDeclaration(b.alloc_function(
                    SPAN,
                    FunctionType::FunctionDeclaration,
                    Some(b.binding_identifier(SPAN, b.ident(self.local_name(name)?))),
                    false,
                    false,
                    false,
                    NONE,
                    NONE,
                    formal,
                    NONE,
                    Some(b.alloc_function_body(SPAN, b.vec(), body)),
                ))
            }
            TraceStatementKind::Try {
                param,
                body,
                handler,
                catches,
                finalizer,
            } => {
                // The `finally`: traced once from the body and a throw, or
                // only after a throw, as the handler (see `TraceStatementKind::Try`).
                let finally = match (catches, finalizer) {
                    (_, Some(finalizer)) => Some(finalizer),
                    (false, None) => Some(handler),
                    (true, None) => None,
                };
                // `try { try { … } catch (e) { … } } finally { … }` is
                // `try { … } catch (e) { … } finally { … }`: the VM pushes
                // the `finally` frame first, then the `catch` one.
                let inner =
                    self.catch_in_finally(statement.id).and_then(|(_, inner)| {
                        match &self.report.arena.statements[inner as usize].kind {
                            TraceStatementKind::Try {
                                param,
                                body,
                                handler,
                                ..
                            } => Some((param, body, handler)),
                            _ => None,
                        }
                    });
                let (body, caught) = match inner {
                    Some((param, body, handler)) => (body, Some((param, handler))),
                    None => (body, catches.then_some((param, handler))),
                };
                let body = self.block(body)?;
                let catch = match caught {
                    Some((param, handler)) => {
                        let param = b.catch_parameter(
                            span,
                            b.binding_pattern_binding_identifier(
                                span,
                                b.ident(self.local_name(param)?),
                            ),
                            NONE,
                        );
                        let handler = self.block(handler)?;
                        Some(b.alloc_catch_clause(span, Some(param), handler))
                    }
                    None => None,
                };
                let finally = match finally {
                    Some(finally) => Some(self.block(finally)?),
                    None => None,
                };
                b.statement_try(span, body, catch, finally)
            }
        })
    }

    /// The statements of one block.
    pub(super) fn block(
        &mut self,
        ids: &[TraceStatementId],
    ) -> Result<oxc_allocator::Box<'b, BlockStatement<'b>>, EmitError> {
        let Statement::BlockStatement(block) = self.arm(ids)? else {
            unreachable!("arm builds a block statement");
        };
        Ok(block)
    }

    /// The statements of one `if` arm, as a block.
    pub(super) fn arm(&mut self, ids: &[TraceStatementId]) -> Result<Statement<'b>, EmitError> {
        let mut body = self.builder.vec_with_capacity(ids.len());
        for id in ids {
            let statement = self
                .report
                .arena
                .statements
                .get(*id as usize)
                .ok_or_else(|| EmitError::InvalidArena(format!("missing statement {id}")))?;
            if !is_empty_declare(statement)
                && !self.dead[*id as usize]
                && !self.program_skips(statement)
            {
                if self.rethrows.contains(id) {
                    continue;
                }
                if let Some((before, _)) = self.catch_in_finally(*id) {
                    for id in before {
                        let statement = &self.report.arena.statements[id as usize];
                        self.planned_before(id, &mut body)?;
                        body.push(self.statement(statement)?);
                        self.planned_after(id, &mut body)?;
                    }
                }
                self.planned_before(*id, &mut body)?;
                body.push(self.statement(statement)?);
                self.planned_after(*id, &mut body)?;
            }
        }
        Ok(self.builder.statement_block(SPAN, body))
    }

    /// For `try { lets; try { … } catch (e) { … } lets } finally { … }`,
    /// where each `let` binds a pure value: the `let`s and the inner `try`.
    /// The VM pushes the `finally` frame, then the `catch` one: this is one
    /// `try { … } catch (e) { … } finally { … }`, after the `let`s, which
    /// cannot throw and nothing after the `try` reads.
    pub(super) fn catch_in_finally(
        &self,
        id: TraceStatementId,
    ) -> Option<(Vec<TraceStatementId>, TraceStatementId)> {
        let arena = &self.report.arena;
        let TraceStatementKind::Try {
            body,
            catches: false,
            ..
        } = &arena.statements.get(id as usize)?.kind
        else {
            return None;
        };
        let live: Vec<TraceStatementId> = body
            .iter()
            .copied()
            .filter(|id| {
                let statement = &arena.statements[*id as usize];
                !is_empty_declare(statement)
                    && !self.dead[*id as usize]
                    && !self.program_skips(statement)
            })
            .collect();
        let is_catch = |id: &TraceStatementId| {
            matches!(
                arena.statements[*id as usize].kind,
                TraceStatementKind::Try {
                    catches: true,
                    finalizer: None,
                    ..
                }
            )
        };
        let inner = *live.iter().find(|id| is_catch(id))?;
        let lets: Vec<TraceStatementId> = live.iter().copied().filter(|id| *id != inner).collect();
        let pure = lets.iter().all(|id| {
            matches!(arena.statements[*id as usize].kind,
                TraceStatementKind::Let { value, .. } if is_pure_value(arena, value))
        });
        pure.then_some((lets, inner))
    }

    /// `x` when node `id` is `!x`.
    pub(super) fn negated(&self, id: TraceNodeId) -> Option<TraceNodeId> {
        match self.report.arena.nodes.get(id as usize)?.kind {
            TraceNodeKind::Unary {
                operator: PrimitiveOperator::LogicalNot,
                value,
            } => Some(value),
            _ => None,
        }
    }
    pub(super) fn expression(&mut self, id: TraceNodeId) -> Result<Expression<'b>, EmitError> {
        let node = self
            .report
            .arena
            .nodes
            .get(id as usize)
            .ok_or(EmitError::InvalidNode(id))?;
        if !self.suppress_slot_reads {
            if let Some(slot) = self.node_map.get_mut(id as usize) {
                *slot = self.current_statement;
            }
        }
        if !self.suppress_slot_reads {
            if let Some(slot) = self
                .slot_plan
                .reads
                .get(&(self.current_trace_statement, id))
            {
                let name = self
                    .slot_names
                    .get(slot)
                    .ok_or_else(|| EmitError::InvalidArena(format!("unplanned slot {slot}")))?;
                return Ok(self
                    .builder
                    .expression_identifier(SPAN, self.builder.ident(name)));
            }
        }
        let b = self.builder;
        Ok(match &node.kind {
            TraceNodeKind::Undefined => b.void_0(SPAN),
            TraceNodeKind::Null => b.expression_null_literal(SPAN),
            TraceNodeKind::Boolean(v) => b.expression_boolean_literal(SPAN, *v),
            TraceNodeKind::Number(raw) => {
                if raw == "NaN" {
                    b.expression_binary(
                        SPAN,
                        b.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal),
                        BinaryOperator::Division,
                        b.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal),
                    )
                } else if raw == "Infinity" {
                    b.expression_binary(
                        SPAN,
                        b.expression_numeric_literal(SPAN, 1.0, None, NumberBase::Decimal),
                        BinaryOperator::Division,
                        b.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal),
                    )
                } else if raw == "-Infinity" {
                    b.expression_binary(
                        SPAN,
                        b.expression_unary(
                            SPAN,
                            UnaryOperator::UnaryNegation,
                            b.expression_numeric_literal(SPAN, 1.0, None, NumberBase::Decimal),
                        ),
                        BinaryOperator::Division,
                        b.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal),
                    )
                } else if raw == "-0" {
                    b.expression_unary(
                        SPAN,
                        UnaryOperator::UnaryNegation,
                        b.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal),
                    )
                } else {
                    b.expression_numeric_literal(
                        SPAN,
                        raw.parse().unwrap_or(0.0),
                        None,
                        NumberBase::Decimal,
                    )
                }
            }
            TraceNodeKind::String(v) => b.expression_string_literal(SPAN, b.atom(v), None),
            TraceNodeKind::Object { identity } => {
                let name = self.object_parameters.get(identity).ok_or_else(|| {
                    EmitError::InvalidArena(format!("missing object identity {identity}"))
                })?;
                b.expression_identifier(SPAN, b.ident(name))
            }
            TraceNodeKind::External { name } if self.program.is_some() => {
                self.program_input(name)?
            }
            TraceNodeKind::PropertyRead { object, key }
                if self.cell_variable(*object).is_some() =>
            {
                let slot = self.cell_variable(*object).unwrap_or_default();
                if !self.is_cell_value_key(*key) {
                    return Err(EmitError::InvalidArena(format!(
                        "program mode: a read of cell {slot} is not of its value"
                    )));
                }
                b.expression_identifier(SPAN, b.ident(&self.cell_key(slot)?))
            }
            TraceNodeKind::Identifier { name }
                if self.program.is_some()
                    && cell_slot(name).is_some()
                    && !self.inline.contains_key(name.as_str()) =>
            {
                return Err(EmitError::InvalidArena(format!(
                    "program mode: {name} is used other than through its value"
                )));
            }
            TraceNodeKind::VmFunction { .. } if self.program.is_some() => {
                return Err(EmitError::InvalidArena(
                    "program mode: a function is created other than into a binding".into(),
                ));
            }
            TraceNodeKind::External { name } => {
                let name = self
                    .external
                    .get(name)
                    .ok_or_else(|| EmitError::MissingExternal(name.clone()))?;
                b.expression_identifier(SPAN, b.ident(name))
            }
            // A constant or a copy of a constant binding is written in place.
            TraceNodeKind::Identifier { name } if self.inline.contains_key(name.as_str()) => {
                let value = self.inline[name.as_str()];
                return self.expression(value);
            }
            TraceNodeKind::Identifier { name } => {
                b.expression_identifier(SPAN, b.ident(self.local_name(name)?))
            }
            TraceNodeKind::StackLength
            | TraceNodeKind::StackRead { .. }
            | TraceNodeKind::MemoryRead { .. } => return Err(EmitError::InvalidNode(id)),
            TraceNodeKind::Binary {
                operator,
                left,
                right,
            } => b.expression_binary(
                SPAN,
                self.expression(*left)?,
                binary_operator(*operator).ok_or_else(|| {
                    EmitError::InvalidArena(format!("invalid binary operator at node {id}"))
                })?,
                self.expression(*right)?,
            ),
            TraceNodeKind::Unary { operator, value } => b.expression_unary(
                SPAN,
                unary_operator(*operator).ok_or_else(|| {
                    EmitError::InvalidArena(format!("invalid unary operator at node {id}"))
                })?,
                self.expression(*value)?,
            ),
            TraceNodeKind::Global { name } => b.expression_identifier(SPAN, b.ident(name)),
            TraceNodeKind::PropertyRead { object, key } => {
                let object = self.expression(*object)?;
                let key = self.expression(*key)?;
                Expression::from(b.member_expression_computed(SPAN, object, key, false))
            }
            TraceNodeKind::ObjectLiteral { keys, values } => {
                let mut properties = b.vec_with_capacity(keys.len());
                for (key, value) in keys.iter().zip(values) {
                    let key = if is_identifier_name(key) {
                        b.property_key_static_identifier(SPAN, b.atom(key))
                    } else {
                        PropertyKey::StringLiteral(b.alloc_string_literal(SPAN, b.atom(key), None))
                    };
                    properties.push(b.object_property_kind_object_property(
                        SPAN,
                        PropertyKind::Init,
                        key,
                        self.expression(*value)?,
                        false,
                        false,
                        false,
                    ));
                }
                b.expression_object(SPAN, properties)
            }
            TraceNodeKind::ArrayLiteral { elements } => {
                let mut items = b.vec_with_capacity(elements.len());
                for element in elements {
                    items.push(ArrayExpressionElement::from(self.expression(*element)?));
                }
                b.expression_array(SPAN, items)
            }
            TraceNodeKind::Call {
                callee,
                this_value,
                arguments,
            } => self.call(*callee, *this_value, arguments)?,
            TraceNodeKind::New { callee, arguments } => {
                let callee = self.expression(*callee)?;
                let mut items = b.vec_with_capacity(arguments.len());
                for argument in arguments {
                    items.push(Argument::from(self.expression(*argument)?));
                }
                b.expression_new(SPAN, callee, NONE, items)
            }
            TraceNodeKind::Delete { object, key } => {
                let object = self.expression(*object)?;
                let key = self.expression(*key)?;
                let member = b.member_expression_computed(SPAN, object, key, false);
                b.expression_unary(SPAN, UnaryOperator::Delete, Expression::from(member))
            }
            TraceNodeKind::Function { source } => self.function(source)?,
            TraceNodeKind::VmFunction {
                descriptor,
                byte_offset,
                dispatch_state,
                memory,
            } => {
                let mut items = b.vec_with_capacity(4);
                for value in [descriptor, byte_offset, dispatch_state] {
                    items.push(Argument::from(b.expression_numeric_literal(
                        SPAN,
                        *value as f64,
                        None,
                        NumberBase::Decimal,
                    )));
                }
                if let Some(memory) = memory {
                    items.push(Argument::from(self.expression(*memory)?));
                }
                b.expression_call(
                    SPAN,
                    b.expression_identifier(SPAN, b.ident(VM_CREATE_THREAD)),
                    NONE,
                    items,
                    false,
                )
            }
        })
    }

    /// A closed function expression, parsed from its source into the output.
    pub(super) fn function(&self, source: &str) -> Result<Expression<'b>, EmitError> {
        let invalid =
            || EmitError::InvalidArena(format!("function source {source:?} is not one expression"));
        let allocator = self.builder.allocator;
        let text = allocator.alloc_str(&format!("({source})"));
        let parsed = oxc_parser::Parser::new(allocator, text, SourceType::script()).parse();
        if parsed.panicked || !parsed.errors.is_empty() {
            return Err(invalid());
        }
        let mut body = parsed.program.body.into_iter();
        let (Some(Statement::ExpressionStatement(statement)), None) = (body.next(), body.next())
        else {
            return Err(invalid());
        };
        match statement.unbox().expression {
            Expression::ParenthesizedExpression(inner) => match inner.unbox().expression {
                function @ Expression::FunctionExpression(_) => Ok(function),
                _ => Err(invalid()),
            },
            _ => Err(invalid()),
        }
    }

    /// `callee(arguments...)` when the receiver is `undefined` and the callee
    /// is a plain name (a plain call passes `undefined` too),
    /// `object[key](arguments...)` when the callee is a property of the
    /// receiver, and otherwise `callee.call(this_value, arguments...)`.
    pub(super) fn call(
        &mut self,
        callee: TraceNodeId,
        this_value: TraceNodeId,
        arguments: &[TraceNodeId],
    ) -> Result<Expression<'b>, EmitError> {
        let b = self.builder;
        let kind = |id: TraceNodeId| {
            self.report
                .arena
                .nodes
                .get(id as usize)
                .map(|node| &node.kind)
                .ok_or(EmitError::InvalidNode(id))
        };
        let plain = matches!(kind(this_value)?, TraceNodeKind::Undefined)
            && matches!(
                kind(callee)?,
                TraceNodeKind::Identifier { .. }
                    | TraceNodeKind::Global { .. }
                    | TraceNodeKind::External { .. }
                    | TraceNodeKind::Object { .. }
            );
        // A method call `object[key](...)` passes `object` as the receiver.
        let method = match kind(callee)? {
            TraceNodeKind::PropertyRead { object, .. } => {
                *object == this_value
                    || matches!(
                        (kind(*object)?, kind(this_value)?),
                        (TraceNodeKind::Global { name: a }, TraceNodeKind::Global { name: b })
                            if a == b
                    )
            }
            _ => false,
        };
        let callee = self.expression(callee)?;
        let mut items = b.vec_with_capacity(arguments.len() + 1);
        let target = if plain || method {
            callee
        } else {
            items.push(Argument::from(self.expression(this_value)?));
            Expression::from(b.member_expression_static(
                SPAN,
                callee,
                b.identifier_name(SPAN, "call"),
                false,
            ))
        };
        for argument in arguments {
            items.push(Argument::from(self.expression(*argument)?));
        }
        Ok(b.expression_call(SPAN, target, NONE, items, false))
    }

    pub(super) fn local_name(&self, name: &str) -> Result<&str, EmitError> {
        self.local_names
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| EmitError::InvalidIdentifier(name.into()))
    }
}

pub(super) fn binary_operator(operator: PrimitiveOperator) -> Option<BinaryOperator> {
    Some(match operator {
        PrimitiveOperator::JsAdd => BinaryOperator::Addition,
        PrimitiveOperator::Subtract => BinaryOperator::Subtraction,
        PrimitiveOperator::Multiply => BinaryOperator::Multiplication,
        PrimitiveOperator::Divide => BinaryOperator::Division,
        PrimitiveOperator::Remainder => BinaryOperator::Remainder,
        PrimitiveOperator::ShiftLeft => BinaryOperator::ShiftLeft,
        PrimitiveOperator::ShiftRight => BinaryOperator::ShiftRight,
        PrimitiveOperator::ShiftRightZeroFill => BinaryOperator::ShiftRightZeroFill,
        PrimitiveOperator::BitwiseOr => BinaryOperator::BitwiseOR,
        PrimitiveOperator::BitwiseXor => BinaryOperator::BitwiseXOR,
        PrimitiveOperator::BitwiseAnd => BinaryOperator::BitwiseAnd,
        PrimitiveOperator::Exponent => BinaryOperator::Exponential,
        PrimitiveOperator::Equal => BinaryOperator::Equality,
        PrimitiveOperator::NotEqual => BinaryOperator::Inequality,
        PrimitiveOperator::StrictEqual => BinaryOperator::StrictEquality,
        PrimitiveOperator::StrictNotEqual => BinaryOperator::StrictInequality,
        PrimitiveOperator::LessThan => BinaryOperator::LessThan,
        PrimitiveOperator::LessThanOrEqual => BinaryOperator::LessEqualThan,
        PrimitiveOperator::GreaterThan => BinaryOperator::GreaterThan,
        PrimitiveOperator::GreaterThanOrEqual => BinaryOperator::GreaterEqualThan,
        PrimitiveOperator::In => BinaryOperator::In,
        PrimitiveOperator::InstanceOf => BinaryOperator::Instanceof,
        _ => return None,
    })
}
pub(super) fn unary_operator(operator: PrimitiveOperator) -> Option<UnaryOperator> {
    Some(match operator {
        PrimitiveOperator::UnaryPlus => UnaryOperator::UnaryPlus,
        PrimitiveOperator::UnaryMinus => UnaryOperator::UnaryNegation,
        PrimitiveOperator::BitwiseNot => UnaryOperator::BitwiseNot,
        PrimitiveOperator::LogicalNot => UnaryOperator::LogicalNot,
        PrimitiveOperator::Void => UnaryOperator::Void,
        PrimitiveOperator::Typeof => UnaryOperator::Typeof,
        _ => return None,
    })
}

pub(super) fn emit_with(
    report: &TraceReport,
    options: &EmitOptions,
    program: bool,
    namespace: Option<&str>,
) -> Result<(EmittedJavaScript, ProgramFacts), EmitError> {
    let mut facts = ProgramFacts::default();
    if !options.allow_incomplete
        && matches!(
            report.completion,
            TraceCompletion::PartialUnsupported
                | TraceCompletion::BudgetStopped
                | TraceCompletion::PrefixStopped(_)
        )
    {
        return Err(EmitError::IncompleteTrace(format!(
            "{:?}",
            report.completion
        )));
    }
    validate_arena(report)?;
    let slot_plan = plan_slot_emission(report);
    let mut names = NameMap::new(options);
    // Free names (for example `host`) are emitted verbatim, so they are
    // reserved first and every generated name avoids them.
    for node in &report.arena.nodes {
        match &node.kind {
            TraceNodeKind::Global { name } => {
                if !is_identifier_name(name) || is_reserved(name) {
                    return Err(EmitError::InvalidIdentifier(name.clone()));
                }
                names.used.insert(name.clone());
            }
            TraceNodeKind::VmFunction { .. } => {
                names.used.insert(VM_CREATE_THREAD.into());
            }
            _ => {}
        }
    }
    let function_name = names.reserve(&options.function_name)?;
    let cells = if program {
        program_cells(report)?
    } else {
        ProgramCells::default()
    };
    let mut external = BTreeMap::new();
    for raw in &report.external_inputs {
        if program {
            // Read in place (see [`Emitter::program_input`]); none is a
            // parameter.
            if program_input(raw).is_none() {
                return Err(EmitError::MissingExternal(raw.clone()));
            }
            continue;
        }
        if !options
            .external_parameters
            .iter()
            .any(|candidate| candidate == raw)
        {
            return Err(EmitError::MissingExternal(raw.clone()));
        }
        external.insert(raw.clone(), names.reserve(raw)?);
    }
    let object_ids = report
        .arena
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            TraceNodeKind::Object { identity } => Some(*identity),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if program && !object_ids.is_empty() {
        return Err(EmitError::InvalidArena(
            "program mode: the function reads an object it was not given".into(),
        ));
    }
    if program {
        names.used.insert(PROGRAM_THIS.into());
    }
    // A shared slot is the creator's `$KEY_<slot>`: nothing here takes it.
    for slot in cells.captured.values() {
        names.used.insert(program_key(*slot));
    }
    let mut object_parameters = BTreeMap::new();
    for identity in object_ids {
        let raw = format!("object_{identity}");
        object_parameters.insert(identity, names.reserve(&raw)?);
    }
    let mut slot_names = BTreeMap::new();
    for slot in &slot_plan.slots {
        let name = if program {
            program_name(&mut names, &program_key(*slot))?
        } else {
            names.reserve(&memory_slot_name(*slot))?
        };
        slot_names.insert(*slot, name);
    }
    let arena = &report.arena;
    let inline = if options.simplify_bindings {
        inline_bindings(arena)
    } else {
        HashMap::default()
    };
    let mut dead = if options.simplify_bindings {
        dead_bindings(arena, &inline)
    } else {
        vec![false; arena.statements.len()]
    };
    for statement in slot_plan.before.keys() {
        if let Some(dead) = dead.get_mut(*statement as usize) {
            *dead = false;
        }
    }
    let mut preferred_names: HashMap<&str, String> =
        HashMap::with_capacity_and_hasher(arena.statements.len(), Default::default());
    // A trace join, loop, or try variable can be written more than once. Keep
    // `$SSA` for bindings with one definition; give mutable carried values a
    // separate name in JavaScript lowering.
    let assigned = assigned_names(arena);
    for statement in arena.statements.iter() {
        let (name, value) = match &statement.kind {
            TraceStatementKind::Let { name, value }
            | TraceStatementKind::Assign { name, value } => (name.as_str(), *value),
            _ => continue,
        };
        // The first statement with a preferred name gives it.
        if let std::collections::hash_map::Entry::Vacant(entry) = preferred_names.entry(name) {
            let preferred = match &arena.nodes[value as usize].kind {
                TraceNodeKind::VmFunction {
                    byte_offset,
                    dispatch_state,
                    ..
                } if program => Some(format!("$THREAD_{dispatch_state}_{byte_offset}")),
                _ => {
                    preferred_generated_name(name, value, arena, namespace, assigned.contains(name))
                }
            };
            if let Some(preferred) = preferred {
                entry.insert(preferred);
            }
        }
    }
    // Every emitted reference names a binding an emitted statement declares;
    // reserve those, in statement order so the names are deterministic.
    let mut local_names: HashMap<&str, String> =
        HashMap::with_capacity_and_hasher(arena.statements.len(), Default::default());
    for (index, statement) in arena.statements.iter().enumerate() {
        let declared: Vec<&String> = match &statement.kind {
            TraceStatementKind::Let { name, .. } if !dead[index] => vec![name],
            TraceStatementKind::Assign { name, .. } => vec![name],
            TraceStatementKind::Declare { names } => names.iter().collect(),
            TraceStatementKind::Try { param, .. } => vec![param],
            TraceStatementKind::Function { name, params, .. } => {
                std::iter::once(name).chain(params).collect()
            }
            _ => Vec::new(),
        };
        for name in declared {
            if program && cell_slot(name).is_some() {
                // A shared cell is the creator's variable; an own one is
                // exactly `$KEY_<slot>`.
                if !cells.captured.contains_key(name.as_str())
                    && !local_names.contains_key(name.as_str())
                {
                    let key = program_key(cell_slot(name).unwrap_or_default());
                    local_names.insert(name, program_name(&mut names, &key)?);
                }
                continue;
            }
            if !local_names.contains_key(name.as_str()) {
                let preferred = preferred_names
                    .get(name.as_str())
                    .cloned()
                    .or_else(|| {
                        matches!(&statement.kind, TraceStatementKind::Try { param, .. } if param == name)
                            .then(|| {
                                preferred_generated_binding_name(
                                    name,
                                    namespace,
                                    assigned.contains(name.as_str()),
                                )
                            })
                            .flatten()
                    })
                    .unwrap_or_else(|| name.to_string());
                local_names.insert(name, names.reserve(&preferred)?);
            }
        }
    }
    let allocator = Allocator::default();
    let builder = AstBuilder::new(&allocator);
    let mut ast = Emitter {
        report,
        builder,
        external: external.clone(),
        object_parameters: object_parameters.clone(),
        local_names,
        node_map: vec![usize::MAX; arena.nodes.len()],
        current_statement: usize::MAX,
        statement_map: Vec::with_capacity(arena.statements.len()),
        dead,
        rethrows: finally_rethrows(arena),
        inline,
        slot_plan,
        slot_names,
        declared_slots: BTreeSet::new(),
        current_trace_statement: u32::MAX,
        suppress_slot_reads: false,
        program: program.then(|| ProgramEmission {
            cells,
            facts: ProgramFacts::default(),
            strict: false,
            function_depth: 0,
        }),
    };
    // Statements owned by an `if` arm or a loop body are emitted inside it.
    let mut owned = vec![false; arena.statements.len()];
    for statement in &arena.statements {
        for child in block_children(&statement.kind) {
            owned[child as usize] = true;
        }
    }
    let mut statements = builder.vec_with_capacity(report.arena.statements.len());
    // Helpers the code calls by name, defined first.
    for (name, source) in HELPERS {
        let used = arena.nodes.iter().any(
            |node| matches!(&node.kind, TraceNodeKind::Global { name: global } if global == name),
        );
        if used {
            statements.push(helper_declaration(builder, source)?);
        }
    }
    for statement in &report.arena.statements {
        if owned[statement.id as usize]
            || is_empty_declare(statement)
            || ast.dead[statement.id as usize]
            || ast.program_skips(statement)
        {
            continue;
        }
        // A `try`/`catch`/`finally` goes after the `let`s its outer `try`
        // body had around the inner one (see `catch_in_finally`).
        if let Some((before, _)) = ast.catch_in_finally(statement.id) {
            for id in before {
                let statement = &report.arena.statements[id as usize];
                ast.planned_before(id, &mut statements)?;
                ast.current_statement = statements.len();
                statements.push(ast.statement(statement)?);
                ast.planned_after(id, &mut statements)?;
            }
        }
        ast.planned_before(statement.id, &mut statements)?;
        ast.current_statement = statements.len();
        statements.push(ast.statement(statement)?);
        ast.planned_after(statement.id, &mut statements)?;
    }
    let statement_map = std::mem::take(&mut ast.statement_map);
    let mut directives = builder.vec();
    if let Some(emission) = &mut ast.program {
        if emission.strict {
            // `this` is the caller's receiver only in strict code.
            directives.push(builder.directive(
                SPAN,
                builder.string_literal(SPAN, builder.atom("use strict"), None),
                builder.atom("use strict"),
            ));
        }
        facts = std::mem::take(&mut emission.facts);
        facts.captured = emission.cells.captured.values().copied().collect();
    }
    let body = builder.alloc_function_body(SPAN, directives, statements);
    // Parameters follow the order the caller listed them in.
    let mut ordered = external.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(raw, _)| {
        options
            .external_parameters
            .iter()
            .position(|candidate| candidate == *raw)
    });
    let mut params = builder.vec_with_capacity(external.len());
    for name in ordered
        .into_iter()
        .map(|(_, name)| name)
        .chain(object_parameters.values())
    {
        params.push(builder.formal_parameter(
            SPAN,
            builder.vec(),
            builder.binding_pattern_binding_identifier(SPAN, builder.ident(name)),
            NONE,
            NONE,
            false,
            None,
            false,
            false,
        ));
    }
    let formal =
        builder.alloc_formal_parameters(SPAN, FormalParameterKind::FormalParameter, params, NONE);
    let mut program_body = builder.vec();
    program_body.push(Statement::FunctionDeclaration(builder.alloc_function(
        SPAN,
        FunctionType::FunctionDeclaration,
        Some(builder.binding_identifier(SPAN, builder.ident(&function_name))),
        false,
        false,
        false,
        NONE,
        NONE,
        formal,
        NONE,
        Some(body),
    )));
    let program = builder.program(
        SPAN,
        SourceType::script(),
        "",
        builder.vec(),
        None,
        builder.vec(),
        program_body,
    );
    let code = Codegen::new().build(&program).code;
    Ok((
        EmittedJavaScript {
            code,
            statement_map,
            node_map: ast
                .node_map
                .iter()
                .enumerate()
                .filter(|(_, statement)| **statement != usize::MAX)
                .map(|(node, statement)| (node as TraceNodeId, *statement))
                .collect(),
            object_parameters,
        },
        std::mem::take(&mut facts),
    ))
}

pub(super) struct NameMap {
    pub(super) used: BTreeSet<String>,
}
impl NameMap {
    pub(super) fn new(options: &EmitOptions) -> Self {
        let _ = options;
        Self {
            used: BTreeSet::new(),
        }
    }
    pub(super) fn reserve(&mut self, raw: &str) -> Result<String, EmitError> {
        let base = sanitize_identifier(raw)?;
        let mut candidate = base.clone();
        let mut suffix = 2;
        while !self.used.insert(candidate.clone()) {
            candidate = format!("{base}_{suffix}");
            suffix += 1;
        }
        Ok(candidate)
    }
}
/// Program mode's variable of memory slot `slot`.
pub(super) fn program_key(slot: u32) -> String {
    format!("$KEY_{slot}")
}
/// `name`, reserved exactly: program mode names a slot one way only.
pub(super) fn program_name(names: &mut NameMap, name: &str) -> Result<String, EmitError> {
    let reserved = names.reserve(name)?;
    if reserved != name {
        return Err(EmitError::InvalidArena(format!(
            "program mode: {name} names two variables"
        )));
    }
    Ok(reserved)
}
/// What a thread input reads in program mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProgramInput {
    /// `arguments[i]`.
    Argument(u32),
    /// `arguments`.
    Arguments,
    /// `this`.
    Receiver,
    /// The function itself, [`PROGRAM_SELF`].
    Callee,
    /// The creator's cell of the slot, read through scope.
    Captured(u32),
}
pub(super) fn program_input(raw: &str) -> Option<ProgramInput> {
    Some(match raw {
        "args" => ProgramInput::Arguments,
        "receiver" => ProgramInput::Receiver,
        "callee" => ProgramInput::Callee,
        _ => {
            if let Some(slot) = raw.strip_prefix("captured") {
                ProgramInput::Captured(slot.parse().ok()?)
            } else {
                ProgramInput::Argument(raw.strip_prefix("arg")?.parse().ok()?)
            }
        }
    })
}
pub(super) fn memory_slot_name(slot: u32) -> String {
    format!("$SLOT_{slot}")
}
/// The `throw`s of the caught exception that end each path of a `finally`
/// traced after a throw only (see [`TraceStatementKind::Try`]).
pub(super) fn finally_rethrows(arena: &TraceArena) -> BTreeSet<TraceStatementId> {
    fn ends(
        arena: &TraceArena,
        block: &[TraceStatementId],
        param: &str,
        rethrows: &mut BTreeSet<TraceStatementId>,
    ) {
        let Some(&last) = block.last() else {
            return;
        };
        match &arena.statements[last as usize].kind {
            TraceStatementKind::Throw { value } => {
                let thrown = arena.nodes.get(*value as usize).map(|node| &node.kind);
                if matches!(thrown, Some(TraceNodeKind::Identifier { name }) if name == param) {
                    rethrows.insert(last);
                }
            }
            TraceStatementKind::If {
                taken,
                fall_through,
                ..
            } => {
                ends(arena, taken, param, rethrows);
                ends(arena, fall_through, param, rethrows);
            }
            _ => {}
        }
    }
    let mut rethrows = BTreeSet::new();
    for statement in &arena.statements {
        if let TraceStatementKind::Try {
            param,
            handler,
            catches: false,
            finalizer: None,
            ..
        } = &statement.kind
        {
            ends(arena, handler, param, &mut rethrows);
        }
    }
    rethrows
}

pub(super) fn generated_binding_serial(name: &str) -> Option<usize> {
    name.strip_prefix("trace_value_")?.parse().ok()
}
pub(super) fn preferred_generated_name(
    name: &str,
    value: TraceNodeId,
    arena: &TraceArena,
    namespace: Option<&str>,
    reassigned: bool,
) -> Option<String> {
    let serial = generated_binding_serial(name)?;
    let prefix = if reassigned {
        "$CARRY_"
    } else {
        match arena.nodes.get(value as usize)?.kind {
            TraceNodeKind::ArrayLiteral { .. } => "$ARRAY_",
            TraceNodeKind::ObjectLiteral { .. } => "$OBJECT_",
            TraceNodeKind::VmFunction { .. } => "$THREAD_",
            _ => "$SSA_",
        }
    };
    Some(generated_value_name(prefix, namespace, serial))
}
pub(super) fn preferred_generated_binding_name(
    name: &str,
    namespace: Option<&str>,
    reassigned: bool,
) -> Option<String> {
    Some(generated_value_name(
        if reassigned { "$CARRY_" } else { "$SSA_" },
        namespace,
        generated_binding_serial(name)?,
    ))
}

pub(super) fn generated_value_name(prefix: &str, namespace: Option<&str>, serial: usize) -> String {
    match namespace {
        Some(namespace) => format!("{prefix}{namespace}_{serial}"),
        None => format!("{prefix}{serial}"),
    }
}
pub(super) fn sanitize_identifier(raw: &str) -> Result<String, EmitError> {
    if raw.is_empty() {
        return Err(EmitError::InvalidIdentifier(raw.into()));
    }
    let mut out = String::with_capacity(raw.len() + 1);
    for (i, ch) in raw.chars().enumerate() {
        if (ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
            && !(i == 0 && ch.is_ascii_digit())
        {
            out.push(ch);
        } else if i == 0 && ch.is_ascii_digit() {
            out.push('_');
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if is_reserved(&out) {
        out.push('_');
    }
    Ok(out)
}
/// An ASCII identifier name, usable verbatim as a property key or global.
pub(super) fn is_identifier_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

pub(super) fn is_reserved(value: &str) -> bool {
    matches!(
        value,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "let"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
            | "enum"
            | "await"
            | "implements"
            | "interface"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "static"
    )
}

pub(super) const MAX_EMIT_EXPRESSION_DEPTH: usize = 256;
/// The free name a not-yet-traced VM closure is emitted through.
pub(super) const VM_CREATE_THREAD: &str = "vmCreateThread";
/// The global object the VM program closes over; a sloppy-mode `this` of
/// `null`/`undefined` is this object.
pub(super) const HOST: &str = crate::operations::templates::HOST;
pub(super) const MAX_EMIT_EXPANDED_NODES: usize = 100_000;

pub(super) fn validate_arena(report: &TraceReport) -> Result<(), EmitError> {
    let nodes = &report.arena.nodes;
    for (index, node) in nodes.iter().enumerate() {
        if node.id as usize != index {
            return Err(EmitError::InvalidArena(format!(
                "node at index {index} has id {}",
                node.id
            )));
        }
        let children = node_children(&node.kind);
        for child in children {
            if child as usize >= nodes.len() {
                return Err(EmitError::InvalidNode(child));
            }
        }
        match &node.kind {
            TraceNodeKind::Binary { operator, .. } if binary_operator(*operator).is_none() => {
                return Err(EmitError::InvalidArena(format!(
                    "node {} uses a non-binary operator",
                    node.id
                )))
            }
            TraceNodeKind::Unary { operator, .. } if unary_operator(*operator).is_none() => {
                return Err(EmitError::InvalidArena(format!(
                    "node {} uses a non-unary operator",
                    node.id
                )))
            }
            TraceNodeKind::StackLength
            | TraceNodeKind::StackRead { .. }
            | TraceNodeKind::MemoryRead { .. } => {
                return Err(EmitError::InvalidArena(format!(
                    "storage node {} cannot be emitted as an expression",
                    node.id
                )))
            }
            _ => {}
        }
    }
    for (index, statement) in report.arena.statements.iter().enumerate() {
        if statement.id as usize != index {
            return Err(EmitError::InvalidArena(format!(
                "statement at index {index} has id {}",
                statement.id
            )));
        }
        let values = match statement.kind {
            TraceStatementKind::Let { value, .. }
            | TraceStatementKind::Assign { value, .. }
            | TraceStatementKind::Evaluate { value }
            | TraceStatementKind::Return { value }
            | TraceStatementKind::Throw { value } => vec![value],
            TraceStatementKind::PropertyWrite { object, key, value } => vec![object, key, value],
            TraceStatementKind::Declare { .. }
            | TraceStatementKind::Loop { .. }
            | TraceStatementKind::Continue { .. }
            | TraceStatementKind::Break { .. }
            | TraceStatementKind::Function { .. }
            | TraceStatementKind::Try { .. } => Vec::new(),
            TraceStatementKind::If { test, .. } => vec![test],
        };
        for value in values {
            if value as usize >= nodes.len() {
                return Err(EmitError::InvalidNode(value));
            }
        }
    }
    // Each block owns later statements, and no statement has two owners, so
    // the nested blocks form a tree in statement order.
    let mut owned = vec![false; report.arena.statements.len()];
    for statement in &report.arena.statements {
        for child in block_children(&statement.kind) {
            let index = child as usize;
            if child <= statement.id || index >= owned.len() || owned[index] {
                return Err(EmitError::InvalidArena(format!(
                    "statement {} has an invalid block statement {child}",
                    statement.id
                )));
            }
            owned[index] = true;
        }
    }
    for (index, event) in report.arena.events.iter().enumerate() {
        if event.id as usize != index {
            return Err(EmitError::InvalidArena(format!(
                "event at index {index} has id {}",
                event.id
            )));
        }
        let values = match event.kind {
            TraceEventKind::StackRead { index, value }
            | TraceEventKind::StackWrite { index, value }
            | TraceEventKind::MemoryRead { slot: index, value }
            | TraceEventKind::MemoryWrite { slot: index, value } => [index, value],
            TraceEventKind::StackLengthRead { value }
            | TraceEventKind::StackLengthSet { value }
            | TraceEventKind::MemoryInitialize { slot: value }
            | TraceEventKind::BranchDecided { test: value, .. } => [value, value],
            TraceEventKind::Undone => continue,
        };
        for value in values {
            if value as usize >= nodes.len() {
                return Err(EmitError::InvalidNode(value));
            }
        }
    }

    // Validate the graph with an explicit DFS.  A memoized expanded cost
    // rejects shared DAGs whose textual tree would otherwise grow
    // exponentially, while colors catch cycles without recursive calls.
    let mut colors = vec![0u8; nodes.len()];
    let mut depths = vec![0usize; nodes.len()];
    let mut costs = vec![0usize; nodes.len()];
    for root in 0..nodes.len() {
        if colors[root] == 2 {
            continue;
        }
        colors[root] = 1;
        let mut stack = vec![ValidationFrame {
            id: root as TraceNodeId,
            next_child: 0,
            children: node_children(&nodes[root].kind),
        }];
        while let Some(frame) = stack.last_mut() {
            if frame.next_child < frame.children.len() {
                let child = frame.children[frame.next_child];
                frame.next_child += 1;
                let child_index = child as usize;
                match colors[child_index] {
                    1 => return Err(EmitError::ExpressionCycle(child)),
                    2 => {}
                    _ => {
                        colors[child_index] = 1;
                        stack.push(ValidationFrame {
                            id: child,
                            next_child: 0,
                            children: node_children(&nodes[child_index].kind),
                        });
                    }
                }
                continue;
            }
            let id = frame.id as usize;
            let mut depth = 1usize;
            let mut cost = 1usize;
            for child in &frame.children {
                depth = depth.max(depths[*child as usize].saturating_add(1));
                cost = cost.saturating_add(costs[*child as usize]);
            }
            if depth > MAX_EMIT_EXPRESSION_DEPTH {
                return Err(EmitError::ExpressionTooDeep {
                    node: frame.id,
                    limit: MAX_EMIT_EXPRESSION_DEPTH,
                });
            }
            if cost > MAX_EMIT_EXPANDED_NODES {
                return Err(EmitError::ExpressionTooLarge {
                    node: frame.id,
                    limit: MAX_EMIT_EXPANDED_NODES,
                });
            }
            depths[id] = depth;
            costs[id] = cost;
            colors[id] = 2;
            stack.pop();
        }
    }
    Ok(())
}

/// Functions the tracer calls by name for handler loops it does not trace
/// step by step, with their definitions.
pub(crate) const HELPERS: &[(&str, &str)] = &[
    (
        crate::operations::templates::FOR_IN_KEYS,
        "function vmForInKeys(object) { var keys = []; for (var key in object) { keys[keys.length] = key; } return keys; }",
    ),
    (
        crate::operations::templates::INHERIT_OR_OBJECT,
        r#"function vmInheritOrObject(value) { return typeof value === "object" || typeof value === "function" ? Object.create(value) : {}; }"#,
    ),
    (
        crate::operations::templates::POST_TO_WORKER,
        "function vmPostToWorker(first, second, third, operand, callback) { if (!vmInitializeWorker()) { return false; } vmWorkerCallbacks.push(callback); vmWorker.postMessage([first, second, third, operand]); return true; }",
    ),
];

/// A helper's `function` declaration, parsed from its source.
pub(super) fn helper_declaration<'b>(
    builder: AstBuilder<'b>,
    source: &str,
) -> Result<Statement<'b>, EmitError> {
    let invalid = || EmitError::InvalidArena(format!("helper {source:?} is not one declaration"));
    let text = builder.allocator.alloc_str(source);
    let parsed = oxc_parser::Parser::new(builder.allocator, text, SourceType::script()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(invalid());
    }
    let mut body = parsed.program.body.into_iter();
    match (body.next(), body.next()) {
        (Some(declaration @ Statement::FunctionDeclaration(_)), None) => Ok(declaration),
        _ => Err(invalid()),
    }
}

pub(super) struct ValidationFrame {
    pub(super) id: TraceNodeId,
    pub(super) next_child: usize,
    pub(super) children: Vec<TraceNodeId>,
}

pub(super) fn node_children(kind: &TraceNodeKind) -> Vec<TraceNodeId> {
    match kind {
        TraceNodeKind::Binary { left, right, .. } => vec![*left, *right],
        TraceNodeKind::Unary { value, .. } => vec![*value],
        TraceNodeKind::PropertyRead { object, key } => vec![*object, *key],
        TraceNodeKind::ObjectLiteral { values, .. } => values.clone(),
        TraceNodeKind::ArrayLiteral { elements } => elements.clone(),
        TraceNodeKind::Call {
            callee,
            this_value,
            arguments,
        } => [*callee, *this_value]
            .into_iter()
            .chain(arguments.iter().copied())
            .collect(),
        TraceNodeKind::Delete { object, key } => vec![*object, *key],
        TraceNodeKind::New { callee, arguments } => std::iter::once(*callee)
            .chain(arguments.iter().copied())
            .collect(),
        TraceNodeKind::VmFunction {
            memory: Some(memory),
            ..
        } => vec![*memory],
        _ => Vec::new(),
    }
}
