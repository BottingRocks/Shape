use super::build::{binary_name, unary_name};
use super::*;

pub const GRAPH_SCHEMA_VERSION: &str = "shape-handler-graph/v1";
/// Version of the proved micro-op primitive vocabulary the graph may carry.
pub const PRIMITIVE_REGISTRY_VERSION: &str = "shape-handler-primitives/v1";

/// Maximum nesting depth the public fallible legacy-value projection will
/// materialize.  Legacy [`crate::operations::templates::SemanticValue`] is a recursively
/// boxed tree, so its drop/clone recursion is not safe for arbitrary external
/// depth.  Deeper caller-supplied graphs fail closed with a projection error
/// and are rejected by [`HandlerGraph::validate`]; the extractor's own trusted
/// projection is not subjected to this limit.
pub const MAX_PROJECTED_VALUE_DEPTH: u32 = 1024;
/// Maximum number of legacy value nodes the public projection will materialize
/// for one value root.  Repeated operand edges are projected by cloning shared
/// sub-trees, so a small DAG can otherwise describe an exponential tree.
pub const MAX_PROJECTED_VALUE_NODES: u64 = 1_000_000;

/// Identity of an individual value or operation occurrence within one handler.
pub type OccurrenceId = u32;
/// Identity of one entry in the ordered effect chain.
pub type EffectId = u32;

/// How precisely an occurrence's span identifies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanGranularity {
    /// The span is the originating sub-expression, and maps exactly to it.
    Expression,
    /// The span is the enclosing statement/declaration.  The current extractor
    /// does not retain a narrower sub-expression span here; this is recorded
    /// explicitly instead of fabricating a precise span.
    Statement,
}

/// One semantic micro-op, reusing the existing recipe/operator vocabulary but
/// with explicit operand identities instead of owned sub-trees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OccurrenceOp {
    // ---- value-producing occurrences -------------------------------------
    Context,
    Operand {
        position: u32,
    },
    Literal(LiteralValue),
    StackLength,
    StackRead {
        index: OccurrenceId,
    },
    MemoryRead {
        slot: OccurrenceId,
    },
    /// A reference to a specific SSA version of a temporary.
    TemporaryRef {
        temporary: u32,
        version: OccurrenceId,
    },
    Binary {
        operator: PrimitiveOperator,
        left: OccurrenceId,
        right: OccurrenceId,
    },
    Unary {
        operator: PrimitiveOperator,
        value: OccurrenceId,
    },
    UnknownValue {
        reason: String,
    },
    /// A captured or global name, as an abstract object expression.
    Binding {
        name: String,
    },
    /// `object[key]` on a non-VM object expression.
    PropertyRead {
        object: OccurrenceId,
        key: OccurrenceId,
    },
    /// `{ keys[i]: values[i], ... }`, in source order.
    ObjectLiteral {
        keys: Vec<String>,
        values: Vec<OccurrenceId>,
    },
    /// `[elements...]`.
    ArrayLiteral {
        elements: Vec<OccurrenceId>,
    },
    /// A field of the call frame popped by this handler.
    PoppedCallFrame {
        field: CallFrameField,
    },
    /// `context.receiverObject` / `context.receiverValue`.
    Receiver {
        field: ReceiverField,
    },
    /// A read of the VM exception stacks: a popped try-frame or error-record
    /// field, or the popped pending exception. Holds the
    /// [`SemanticValue::PoppedTryFrame`], [`SemanticValue::PoppedErrorRecord`]
    /// or [`SemanticValue::PopThrownValue`] leaf.
    ExceptionRead {
        value: SemanticValue,
    },
    /// A closed function expression, by source.
    FunctionLiteral {
        source: String,
    },
    /// `new callee(...arguments)`.
    Construct {
        callee: OccurrenceId,
        arguments: OccurrenceId,
    },
    /// `delete object[key]`.
    Delete {
        object: OccurrenceId,
        key: OccurrenceId,
    },
    /// `context.stack.slice(start)`.
    StackSlice {
        start: OccurrenceId,
    },
    /// `Function.prototype.apply.call(callee, this_value, arguments)`.
    Call {
        callee: OccurrenceId,
        this_value: OccurrenceId,
        arguments: OccurrenceId,
    },
    /// `vmCreateThread(descriptor, byte_offset, dispatch_state, memory)`.
    CreateThread {
        descriptor: OccurrenceId,
        byte_offset: OccurrenceId,
        dispatch_state: OccurrenceId,
    },
    // ---- operation occurrences -------------------------------------------
    Bind {
        temporary: u32,
        value: OccurrenceId,
    },
    Assign {
        temporary: u32,
        value: OccurrenceId,
    },
    StackWrite {
        index: OccurrenceId,
        value: OccurrenceId,
    },
    StackLengthSet {
        value: OccurrenceId,
    },
    MemoryWrite {
        slot: OccurrenceId,
        value: OccurrenceId,
    },
    MemoryInitialize {
        slot: OccurrenceId,
    },
    /// `object[key] = value` on a non-VM object.
    PropertyWrite {
        object: OccurrenceId,
        key: OccurrenceId,
        value: OccurrenceId,
    },
    /// Enter a `try` whose handler is at this PC and state.
    PushTryFrame {
        byte_offset: OccurrenceId,
        dispatch_state: OccurrenceId,
    },
    /// An input-less step of the VM exception machinery: one of
    /// [`RecipeOperation::PopTryFrame`], `PushFinallyRecord`,
    /// `PopErrorRecord`, `RethrowIfHandled` or `DropThrownIfHandled`.
    ExceptionStep {
        operation: RecipeOperation,
    },
    /// Push a VM call frame returning after this handler.
    PushCallFrame,
    /// Pop the VM call frame read by [`Self::PoppedCallFrame`].
    PopCallFrame,
    /// Continue at this PC after the handler.
    SetProgramCounter {
        value: OccurrenceId,
    },
    /// Continue in this dispatch state after the handler.
    SetDispatchState {
        value: OccurrenceId,
    },
    /// `vmPendingResult = value`: the VM function returns `value`.
    CompleteThread {
        value: OccurrenceId,
    },
    /// Continue at this PC and state after the handler if `condition` holds.
    JumpIf {
        condition: OccurrenceId,
        byte_offset: OccurrenceId,
        dispatch_state: OccurrenceId,
    },
    Evaluate {
        value: OccurrenceId,
    },
    Return {
        value: OccurrenceId,
    },
    Throw {
        value: OccurrenceId,
    },
    Unknown {
        reason: String,
    },
}

impl OccurrenceOp {
    /// True when the occurrence produces a value that other occurrences use.
    pub fn is_value(&self) -> bool {
        matches!(
            self,
            Self::Context
                | Self::Operand { .. }
                | Self::Literal(_)
                | Self::StackLength
                | Self::StackRead { .. }
                | Self::MemoryRead { .. }
                | Self::TemporaryRef { .. }
                | Self::Binary { .. }
                | Self::Unary { .. }
                | Self::UnknownValue { .. }
                | Self::Binding { .. }
                | Self::PropertyRead { .. }
                | Self::ObjectLiteral { .. }
                | Self::ArrayLiteral { .. }
                | Self::PoppedCallFrame { .. }
                | Self::Receiver { .. }
                | Self::FunctionLiteral { .. }
                | Self::ExceptionRead { .. }
                | Self::Construct { .. }
                | Self::Delete { .. }
                | Self::StackSlice { .. }
                | Self::Call { .. }
                | Self::CreateThread { .. }
        )
    }

    /// True when the occurrence is an executable operation (a recipe step).
    pub fn is_operation(&self) -> bool {
        !self.is_value()
    }

    /// Meaningful, stable semantic family name.  Array-free and counter-free:
    /// never `runLoop_7`-style, and ids stay in the occurrence metadata.
    pub fn semantic_name(&self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Operand { .. } => "operand",
            Self::Literal(_) => "literal",
            Self::StackLength => "context.stack.length",
            Self::StackRead { .. } => "context.stack.read",
            Self::MemoryRead { .. } => "context.memory.readSlot",
            Self::TemporaryRef { .. } => "temporary.copy",
            Self::Binary { operator, .. } => binary_name(*operator),
            Self::Unary { operator, .. } => unary_name(*operator),
            Self::UnknownValue { .. } => "unknown.value",
            Self::Binding { .. } => "binding",
            Self::PropertyRead { .. } => "property.read",
            Self::ObjectLiteral { .. } => "object.new",
            Self::ArrayLiteral { .. } => "array.new",
            Self::PropertyWrite { .. } => "property.write",
            Self::PoppedCallFrame { .. } => "context.callFrame.field",
            Self::Receiver { .. } => "context.receiver",
            Self::FunctionLiteral { .. } => "function.new",
            Self::Construct { .. } => "function.construct",
            Self::Delete { .. } => "property.delete",
            Self::StackSlice { .. } => "context.stack.slice",
            Self::Call { .. } => "function.call",
            Self::CreateThread { .. } => "vm.thread.create",
            Self::ExceptionRead { .. } => "context.exception.read",
            Self::PushTryFrame { .. } => "context.tryFrameStack.push",
            Self::ExceptionStep { .. } => "context.exception.step",
            Self::PushCallFrame => "context.callFrameStack.push",
            Self::PopCallFrame => "context.callFrameStack.pop",
            Self::SetProgramCounter { .. } => "context.byteOffset.write",
            Self::SetDispatchState { .. } => "context.dispatchState.write",
            Self::JumpIf { .. } => "context.jumpIf",
            Self::CompleteThread { .. } => "vm.thread.return",
            Self::Bind { .. } => "let",
            Self::Assign { .. } => "assign",
            Self::StackWrite { .. } => "context.stack.write",
            Self::StackLengthSet { .. } => "context.stack.length.write",
            Self::MemoryWrite { .. } => "context.memory.writeSlot",
            Self::MemoryInitialize { .. } => "context.memory.initializeSlot",
            Self::Evaluate { .. } => "evaluate",
            Self::Return { .. } => "return",
            Self::Throw { .. } => "throw",
            Self::Unknown { .. } => "unknown",
        }
    }

    /// The operands that contribute a *value* to legacy projection.  A
    /// [`Self::TemporaryRef`]'s version edge names its defining operation; it
    /// projects directly to `SemanticValue::Temporary` and must not be walked.
    pub fn value_operands(&self) -> Vec<OccurrenceId> {
        if matches!(self, Self::TemporaryRef { .. }) {
            Vec::new()
        } else {
            self.operands()
        }
    }

    /// The operand occurrence ids this micro-op consumes, in evaluation order.
    /// This is the explicit input edge set the validator checks.
    pub fn operands(&self) -> Vec<OccurrenceId> {
        match self {
            Self::Context
            | Self::Operand { .. }
            | Self::Literal(_)
            | Self::StackLength
            | Self::UnknownValue { .. }
            | Self::Binding { .. }
            | Self::PoppedCallFrame { .. }
            | Self::Receiver { .. }
            | Self::FunctionLiteral { .. }
            | Self::ExceptionRead { .. }
            | Self::ExceptionStep { .. }
            | Self::PushCallFrame
            | Self::PopCallFrame
            | Self::Unknown { .. } => Vec::new(),
            Self::SetProgramCounter { value }
            | Self::SetDispatchState { value }
            | Self::CompleteThread { value } => vec![*value],
            Self::JumpIf {
                condition,
                byte_offset,
                dispatch_state,
            } => vec![*condition, *byte_offset, *dispatch_state],
            Self::PropertyRead { object, key } => vec![*object, *key],
            Self::ObjectLiteral { values, .. } => values.clone(),
            Self::ArrayLiteral { elements } => elements.clone(),
            Self::StackSlice { start } => vec![*start],
            Self::Construct { callee, arguments } => vec![*callee, *arguments],
            Self::Delete { object, key } => vec![*object, *key],
            Self::PushTryFrame {
                byte_offset,
                dispatch_state,
            } => vec![*byte_offset, *dispatch_state],
            Self::Call {
                callee,
                this_value,
                arguments,
            } => vec![*callee, *this_value, *arguments],
            Self::CreateThread {
                descriptor,
                byte_offset,
                dispatch_state,
            } => vec![*descriptor, *byte_offset, *dispatch_state],
            Self::PropertyWrite { object, key, value } => vec![*object, *key, *value],
            Self::StackRead { index } | Self::MemoryRead { slot: index } => vec![*index],
            Self::TemporaryRef { version, .. } => vec![*version],
            Self::Binary { left, right, .. } => vec![*left, *right],
            Self::Unary { value, .. } => vec![*value],
            Self::Bind { value, .. } | Self::Assign { value, .. } => vec![*value],
            Self::StackWrite { index, value } => vec![*index, *value],
            Self::StackLengthSet { value }
            | Self::Evaluate { value }
            | Self::Return { value }
            | Self::Throw { value } => vec![*value],
            Self::MemoryWrite { slot, value } => vec![*slot, *value],
            Self::MemoryInitialize { slot } => vec![*slot],
        }
    }

    /// The temporary this occurrence defines a fresh version of, if any.
    pub fn defines(&self) -> Option<u32> {
        match self {
            Self::Bind { temporary, .. } | Self::Assign { temporary, .. } => Some(*temporary),
            _ => None,
        }
    }
}

/// A canonical key for a mutable location (stack index / memory slot) when the
/// index expression is statically comparable.  Used to prove a read observed a
/// specific preceding write and to distinguish effect order in comparisons.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocationKey {
    NumberBits(u64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    Context,
    StackLength,
    Operand(u32),
    Temporary {
        temporary: u32,
        version: OccurrenceId,
    },
}

/// Provenance binding one occurrence to its exact source/handler identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// SHA-256 identity of the exact standardized source bytes.
    pub source_hash: String,
    /// Legacy pipeline input identity; retained for compatibility joins only.
    pub raw_input_hash: String,
    pub source_length: usize,
    pub table_declaration_ordinal: u32,
    pub table_element_count: u32,
    pub table_function_count: u32,
    /// Which handler table this occurrence belongs to.
    pub table_key: u32,
    /// Which handler slot inside that table.
    pub handler_index: u32,
    /// Byte span in the standardized source the occurrence was extracted from.
    pub span: SpanRecord,
}

/// One identified value or operation occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    /// Dense id, unique within the graph's (source, table, handler) identity.
    pub id: OccurrenceId,
    pub op: OccurrenceOp,
    pub span: SpanRecord,
    pub granularity: SpanGranularity,
    /// Position among the projected recipe steps, for operation occurrences.
    pub step_order: Option<u32>,
    /// The effect this occurrence performs, if it is observable.
    pub effect: Option<EffectId>,
    /// Original source text, retained for unknown/unsupported occurrences.
    pub source: Option<String>,
    pub provenance: Provenance,
}

impl Occurrence {
    pub fn semantic_name(&self) -> &'static str {
        self.op.semantic_name()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    StackLengthRead,
    StackRead,
    MemoryRead,
    StackWrite,
    StackLengthWrite,
    MemoryWrite,
    MemoryInitialize,
    PropertyWrite,
    CallFramePush,
    CallFramePop,
    ControlTransfer,
    /// Reads every stack value from a start index to the top.
    StackRangeRead,
    /// Calls program code, which may have any effect outside the VM.
    ProgramCall,
    /// Creates a VM closure that snapshots the handler's memory slots.
    ThreadCapture,
    Evaluate,
    Return,
    Throw,
    Unknown,
}

impl EffectKind {
    pub fn semantic_name(&self) -> &'static str {
        match self {
            Self::StackLengthRead => "effect.stack.length.read",
            Self::StackRead => "effect.stack.read",
            Self::MemoryRead => "effect.memory.read",
            Self::StackWrite => "effect.stack.write",
            Self::StackLengthWrite => "effect.stack.length.write",
            Self::MemoryWrite => "effect.memory.write",
            Self::MemoryInitialize => "effect.memory.initialize",
            Self::PropertyWrite => "effect.property.write",
            Self::CallFramePush => "effect.call_frame.push",
            Self::CallFramePop => "effect.call_frame.pop",
            Self::ControlTransfer => "effect.control.transfer",
            Self::StackRangeRead => "effect.stack.range.read",
            Self::ProgramCall => "effect.program.call",
            Self::ThreadCapture => "effect.thread.capture",
            Self::Evaluate => "effect.evaluate",
            Self::Return => "effect.return",
            Self::Throw => "effect.throw",
            Self::Unknown => "effect.unknown",
        }
    }
}

/// One entry in the ordered, conservative effect chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    pub id: EffectId,
    pub occurrence: OccurrenceId,
    pub kind: EffectKind,
    pub may_throw: bool,
    /// For reads: the write occurrence this read is proved to observe, if the
    /// location key is statically comparable.  `None` means no preceding write
    /// was proved for this location.
    pub observed_write: Option<OccurrenceId>,
    /// The statically-known location this effect touches, when comparable.
    pub location: Option<LocationKey>,
}

/// Coarse classification of an unsupported region, kept honest and explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownRegionKind {
    ControlFlow,
    Call,
    Property,
    Assignment,
    Expression,
    Local,
    Other,
}

impl UnknownRegionKind {
    pub fn semantic_name(&self) -> &'static str {
        match self {
            Self::ControlFlow => "unsupported.control_flow",
            Self::Call => "unsupported.call",
            Self::Property => "unsupported.property",
            Self::Assignment => "unsupported.assignment",
            Self::Expression => "unsupported.expression",
            Self::Local => "unsupported.local",
            Self::Other => "unsupported.other",
        }
    }
}

/// A retained, non-executable region: source, span and reason are preserved so
/// a later phase (helper extraction) can consume the identity after proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRegion {
    pub occurrence: OccurrenceId,
    pub kind: UnknownRegionKind,
    pub span: SpanRecord,
    pub reason: String,
    pub source: String,
}

/// The canonical per-handler graph.
///
/// Prefer the public alias [`HandlerIR`] in new code.  This is the recovered
/// static program for one selected handler; it does not identify a runtime
/// instruction occurrence or carry a dispatch PC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerGraph {
    pub schema_version: String,
    pub primitive_version: String,
    /// SHA-256 identity of the standardized source (`PipelineReport::output`).
    pub source_hash: String,
    /// Legacy pipeline input identity; it is not source-proof authority.
    pub raw_input_hash: String,
    pub source_length: usize,
    /// Present for graphs extracted from a report.  The same immutable source
    /// allocation is shared by the report and all of its graphs, so per-handler
    /// extraction does not duplicate the standardized source.  Without it,
    /// validation is structural only and cannot prove source ownership.
    pub source: Option<Arc<str>>,
    pub table_declaration_ordinal: u32,
    pub table_element_count: u32,
    pub table_function_count: u32,
    pub table_key: u32,
    pub handler_index: u32,
    pub handler_span: SpanRecord,
    pub status: TemplateStatus,
    pub occurrences: Vec<Occurrence>,
    pub effects: Vec<Effect>,
    pub unknown_regions: Vec<UnknownRegion>,
}

/// Preferred public name for one handler's recovered micro-op program.
///
/// `HandlerIR` is the static result of extraction for a selected handler: an
/// ordered legacy step projection backed by identified micro-op occurrences,
/// explicit value/effect edges, and retained unsupported regions.  The same
/// value is reused across every instruction occurrence that selects this
/// handler; it is never a decoded instruction and carries no dispatch PC or
/// operand values.
///
/// A `HandlerIR` is not runtime authority.  Unsupported regions stay explicit,
/// and execution requires the legacy projection to pass
/// [`crate::operations::templates::admit_recipe`].  This is an alias of [`HandlerGraph`],
/// so type identity, `Debug`, hashing, and the existing report/schema pins are
/// unchanged.
pub type HandlerIR = HandlerGraph;

/// Preferred public name for one extraction's per-handler `HandlerIR` values.
///
/// The report is produced by static extraction from a [`crate::PipelineReport`], not
/// by executing JavaScript or WASM.  Its `source_hash` identifies the exact
/// standardized source, while `raw_input_hash` is the legacy pipeline input
/// identity retained for joins.  Those per-seed identities only compare within
/// the same extraction; they do not establish AST authenticity. Unsupported
/// regions remain explicit and may prevent execution admission.
pub type HandlerIRReport = HandlerGraphReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerGraphReport {
    pub schema_version: String,
    pub source_hash: String,
    pub raw_input_hash: String,
    pub source_length: usize,
    /// The one immutable standardized source shared by every graph in this
    /// report.  It lets [`Self::validate`] verify source ownership once instead
    /// of rehashing the whole source for each handler.
    pub source: Option<Arc<str>>,
    pub table: ProvenOperationTable,
    pub graphs: Vec<HandlerIR>,
}

impl HandlerGraphReport {
    /// Validate the report as one boundary: prove source ownership/hash once,
    /// then validate each graph's structural invariants and shared source
    /// identity without rehashing the full source per handler.
    pub fn validate(&self) -> Result<(), GraphValidationError> {
        if let Some(source) = &self.source {
            if source.len() != self.source_length {
                return Err(GraphValidationError::SourceLengthMismatch);
            }
            if semantic_source_hash(source.as_bytes()) != self.source_hash {
                return Err(GraphValidationError::SourceHashMismatch);
            }
        }
        for graph in &self.graphs {
            if let Some(source) = &self.source {
                let same_allocation = graph
                    .source
                    .as_ref()
                    .is_some_and(|graph_source| Arc::ptr_eq(graph_source, source));
                if !same_allocation && graph.source.as_deref() != Some(source.as_ref()) {
                    return Err(GraphValidationError::SourceHashMismatch);
                }
                if graph.source_length != source.len() || graph.source_hash != self.source_hash {
                    return Err(GraphValidationError::SourceHashMismatch);
                }
            }
            graph.validate_inner(self.source.is_some())?;
        }
        Ok(())
    }
}

/// Graph-level invariant failure.  Any variant means the graph is not a
/// trustworthy description of the handler and must not be used as authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphValidationError {
    NonDenseId {
        index: usize,
        id: OccurrenceId,
    },
    DanglingInput {
        occurrence: OccurrenceId,
        input: OccurrenceId,
    },
    UseBeforeDefinition {
        occurrence: OccurrenceId,
        input: OccurrenceId,
    },
    InvalidTemporaryVersion {
        occurrence: OccurrenceId,
        temporary: u32,
        version: OccurrenceId,
    },
    ForeignProvenance {
        occurrence: OccurrenceId,
    },
    SourceHashMismatch,
    SourceLengthMismatch,
    SpanOutOfBounds {
        occurrence: Option<OccurrenceId>,
        span: SpanRecord,
    },
    InvalidUtf8Boundary {
        occurrence: Option<OccurrenceId>,
        span: SpanRecord,
    },
    HandlerSpanInvalid,
    HandlerSpanOutOfBounds,
    StepOrderMismatch {
        occurrence: OccurrenceId,
        expected: u32,
        actual: Option<u32>,
    },
    WrongOperandKind {
        occurrence: OccurrenceId,
        input: OccurrenceId,
    },
    MissingEffect {
        occurrence: OccurrenceId,
    },
    UnexpectedEffect {
        occurrence: OccurrenceId,
        effect: EffectId,
    },
    IncorrectEffect {
        effect: EffectId,
    },
    UnknownRegionMismatch {
        region: usize,
    },
    UnknownRegionMissing {
        occurrence: OccurrenceId,
    },
    StatusMismatch {
        expected: TemplateStatus,
        actual: TemplateStatus,
    },
    InvalidGraphForProjection,
    InvertedSpan {
        occurrence: OccurrenceId,
        span: SpanRecord,
    },
    EffectLinkMismatch {
        occurrence: OccurrenceId,
        effect: EffectId,
    },
    NonDenseEffectId {
        index: usize,
        id: EffectId,
    },
    DanglingEffect {
        effect: EffectId,
        occurrence: OccurrenceId,
    },
    NonMonotonicEffect {
        effect: EffectId,
        previous: OccurrenceId,
        occurrence: OccurrenceId,
    },
    UnsafeEffectReorder {
        effect: EffectId,
        read: OccurrenceId,
        observed_write: OccurrenceId,
    },
    StaleEffectWitness {
        effect: EffectId,
        expected: Option<OccurrenceId>,
        actual: Option<OccurrenceId>,
    },
    DanglingUnknownRegion {
        region: usize,
        occurrence: OccurrenceId,
    },
    OrphanUnknownValue {
        occurrence: OccurrenceId,
    },
    ValueProjectionTooDeep {
        occurrence: OccurrenceId,
        depth: u32,
    },
    ValueProjectionTooComplex {
        occurrence: OccurrenceId,
        nodes: u64,
    },
}

impl fmt::Display for GraphValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonDenseId { index, id } => {
                write!(f, "occurrence slot {index} has id {id}, expected {index}")
            }
            Self::DanglingInput { occurrence, input } => {
                write!(f, "occurrence {occurrence} references dangling input {input}")
            }
            Self::UseBeforeDefinition { occurrence, input } => {
                write!(
                    f,
                    "occurrence {occurrence} uses input {input} before it is defined"
                )
            }
            Self::InvalidTemporaryVersion {
                occurrence,
                temporary,
                version,
            } => write!(
                f,
                "occurrence {occurrence} references version {version} that does not define temporary {temporary}"
            ),
            Self::ForeignProvenance { occurrence } => write!(
                f,
                "occurrence {occurrence} carries provenance that does not belong to this handler"
            ),
            Self::SourceHashMismatch => {
                f.write_str("graph source hash does not match the provided standardized source")
            }
            Self::SourceLengthMismatch => {
                f.write_str("graph source length does not match the provided standardized source")
            }
            Self::SpanOutOfBounds { occurrence, span } => write!(
                f,
                "{}span {}..{} is outside the source coordinate space",
                occurrence.map_or(String::new(), |id| format!("occurrence {id} ")),
                span.start,
                span.end
            ),
            Self::InvalidUtf8Boundary { occurrence, span } => write!(
                f,
                "{}span {}..{} is not a UTF-8 boundary",
                occurrence.map_or(String::new(), |id| format!("occurrence {id} ")),
                span.start,
                span.end
            ),
            Self::HandlerSpanInvalid => f.write_str("handler span is inverted"),
            Self::HandlerSpanOutOfBounds => {
                f.write_str("handler span is outside the source coordinate space")
            }
            Self::StepOrderMismatch {
                occurrence,
                expected,
                actual,
            } => write!(
                f,
                "operation occurrence {occurrence} has step order {actual:?}, expected {expected}"
            ),
            Self::WrongOperandKind { occurrence, input } => write!(
                f,
                "occurrence {occurrence} consumes non-value occurrence {input}"
            ),
            Self::MissingEffect { occurrence } => {
                write!(f, "effectful occurrence {occurrence} has no effect entry")
            }
            Self::UnexpectedEffect { occurrence, effect } => write!(
                f,
                "value occurrence {occurrence} has unexpected effect {effect}"
            ),
            Self::IncorrectEffect { effect } => {
                write!(f, "effect {effect} does not match its typed occurrence")
            }
            Self::UnknownRegionMismatch { region } => write!(
                f,
                "unknown region {region} does not match its unknown occurrence"
            ),
            Self::UnknownRegionMissing { occurrence } => {
                write!(f, "unknown occurrence {occurrence} has no unknown region")
            }
            Self::StatusMismatch { expected, actual } => write!(
                f,
                "graph status {actual} does not match derived status {expected}"
            ),
            Self::InvalidGraphForProjection => f.write_str("graph is not valid for projection"),
            Self::InvertedSpan { occurrence, span } => write!(
                f,
                "occurrence {occurrence} has an inverted span {}..{}",
                span.start, span.end
            ),
            Self::EffectLinkMismatch { occurrence, effect } => write!(
                f,
                "occurrence {occurrence} links to effect {effect} that names a different occurrence"
            ),
            Self::NonDenseEffectId { index, id } => {
                write!(f, "effect slot {index} has id {id}, expected {index}")
            }
            Self::DanglingEffect {
                effect,
                occurrence,
            } => write!(
                f,
                "effect {effect} references dangling occurrence {occurrence}"
            ),
            Self::NonMonotonicEffect {
                effect,
                previous,
                occurrence,
            } => write!(
                f,
                "effect {effect} breaks effect order: occurrence {occurrence} precedes {previous}"
            ),
            Self::UnsafeEffectReorder {
                effect,
                read,
                observed_write,
            } => write!(
                f,
                "effect {effect} reorders read {read} before the write {observed_write} it observes"
            ),
            Self::StaleEffectWitness {
                effect,
                expected,
                actual,
            } => write!(
                f,
                "effect {effect} records read-from witness {actual:?}, but the latest potentially aliasing write is {expected:?}"
            ),
            Self::DanglingUnknownRegion { region, occurrence } => write!(
                f,
                "unknown region {region} references dangling occurrence {occurrence}"
            ),
            Self::OrphanUnknownValue { occurrence } => write!(
                f,
                "unknown value occurrence {occurrence} has no retained unknown region; \
                 complete/executable status cannot be claimed"
            ),
            Self::ValueProjectionTooDeep { occurrence, depth } => write!(
                f,
                "value occurrence {occurrence} projects to depth {depth}, above the safe limit {MAX_PROJECTED_VALUE_DEPTH}"
            ),
            Self::ValueProjectionTooComplex { occurrence, nodes } => write!(
                f,
                "value occurrence {occurrence} projects to {nodes} nodes, above the safe limit {MAX_PROJECTED_VALUE_NODES}"
            ),
        }
    }
}

impl std::error::Error for GraphValidationError {}

/// Failure returned by the fallible graph projection APIs. Projection is
/// intentionally separate from execution admission: a structurally valid
/// graph may still require runtime operands, context, or resource limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphProjectionError {
    InvalidId(OccurrenceId),
    NotValue(OccurrenceId),
    Cycle(OccurrenceId),
    ValueTooDeep {
        occurrence: OccurrenceId,
        depth: u32,
    },
    ValueTooComplex {
        occurrence: OccurrenceId,
        nodes: u64,
    },
    InvalidGraph(GraphValidationError),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectionMode {
    /// Public, caller-supplied graphs: enforce the documented safe bounds.
    Bounded,
    /// Extractor-owned builder graphs: preserve legacy extraction without an
    /// artificial source-depth/complexity cap.
    Trusted,
}

/// Consume one operand edge of a projected value.  Shared edges are cloned so
/// a repeated operand is projected twice rather than silently becoming
/// `SemanticValue::Unknown` because its map entry was already removed.
pub(super) fn take_projected_value(
    values: &mut BTreeMap<OccurrenceId, crate::operations::templates::SemanticValue>,
    remaining: &mut BTreeMap<OccurrenceId, u32>,
    operand: OccurrenceId,
) -> Result<crate::operations::templates::SemanticValue, GraphProjectionError> {
    let Some(count) = remaining.get_mut(&operand) else {
        return Err(GraphProjectionError::InvalidGraph(
            GraphValidationError::InvalidGraphForProjection,
        ));
    };
    if *count == 0 {
        return Err(GraphProjectionError::InvalidGraph(
            GraphValidationError::InvalidGraphForProjection,
        ));
    }
    *count -= 1;
    if *count == 0 {
        values
            .remove(&operand)
            .ok_or(GraphProjectionError::InvalidGraph(
                GraphValidationError::InvalidGraphForProjection,
            ))
    } else {
        values
            .get(&operand)
            .cloned()
            .ok_or(GraphProjectionError::InvalidGraph(
                GraphValidationError::InvalidGraphForProjection,
            ))
    }
}
