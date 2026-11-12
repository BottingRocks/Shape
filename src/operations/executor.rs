//! Bounded execution of proved HandlerTemplate recipes.
//!
//! This module is intentionally a small interpreter for the existing semantic
//! recipe schema. It is not a JavaScript runtime or a tracer. A template must be
//! executable before a job is started; unsupported and partial templates are
//! rejected without touching the supplied job context.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{
    js_number::{
        number_to_js_string, parse_numeric_literal, string_to_number, to_int32, to_uint32,
    },
    operations::templates::{
        admit_recipe, AdmissionError, HandlerTemplate, LiteralValue, PrimitiveOperator,
        RecipeBounds, RecipeOperation, RecipeStep, SemanticValue, SpanRecord, TemplateStatus,
    },
};

/// The largest stack length accepted by the default bounded executor.
pub const DEFAULT_MAX_STACK_LENGTH: usize = 1_000_000;
/// The largest number of executed recipe steps accepted by the default executor.
pub const DEFAULT_MAX_STEPS: usize = 100_000;
/// The largest number of initialized memory slots accepted by the default executor.
pub const DEFAULT_MAX_SLOTS: usize = 1_000_000;

/// Primitive values accepted by the bounded handler executor.
///
/// Objects, symbols, and bigints are represented so that supplied operands can
/// be rejected explicitly. They are never coerced by this executor.
#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Object,
    Symbol,
    BigInt,
}

impl JsValue {
    pub fn number(value: f64) -> Self {
        Self::Number(value)
    }

    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }
}

/// A distinguishable memory-slot state. Initialized(Undefined) is not Absent.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotState {
    Absent,
    Initialized(JsValue),
}

/// Limits that keep a recipe job bounded and make resource failures explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandlerExecutionLimits {
    pub max_stack_length: usize,
    pub max_steps: usize,
    pub max_slots: usize,
}

impl Default for HandlerExecutionLimits {
    fn default() -> Self {
        Self {
            max_stack_length: DEFAULT_MAX_STACK_LENGTH,
            max_steps: DEFAULT_MAX_STEPS,
            max_slots: DEFAULT_MAX_SLOTS,
        }
    }
}

/// Per-job mutable state. It is moved into and back out of an execution, so
/// jobs cannot accidentally share stack or slot state.
#[derive(Debug, Clone, PartialEq)]
pub struct HandlerContext {
    stack: Vec<Option<JsValue>>,
    slots: BTreeMap<u32, JsValue>,
}

impl Default for HandlerContext {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerContext {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            slots: BTreeMap::new(),
        }
    }

    pub fn with_stack(stack: Vec<Option<JsValue>>) -> Self {
        Self {
            stack,
            slots: BTreeMap::new(),
        }
    }

    pub fn from_values(values: Vec<JsValue>) -> Self {
        Self::with_stack(values.into_iter().map(Some).collect())
    }

    pub fn stack(&self) -> &[Option<JsValue>] {
        &self.stack
    }

    pub fn stack_len(&self) -> usize {
        self.stack.len()
    }

    pub fn slot_state(&self, slot: u32) -> SlotState {
        self.slots
            .get(&slot)
            .cloned()
            .map_or(SlotState::Absent, SlotState::Initialized)
    }

    pub fn initialize_slot(&mut self, slot: u32) {
        self.slots.insert(slot, JsValue::Undefined);
    }

    pub fn write_slot(&mut self, slot: u32, value: JsValue) {
        self.slots.insert(slot, value);
    }
}

/// The completion produced by an explicit handler return or throw.
#[derive(Debug, Clone, PartialEq)]
pub enum HandlerCompletion {
    Return(JsValue),
    Throw(JsValue),
}

/// Closed set of bounded resources named by [`HandlerExecutionError::ResourceLimit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionResource {
    InitialStackLength,
    InitialInitializedMemorySlots,
    RecipeSteps,
    StackLength,
    InitializedMemorySlots,
}

impl ExecutionResource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InitialStackLength => "initial stack length",
            Self::InitialInitializedMemorySlots => "initial initialized memory slots",
            Self::RecipeSteps => "recipe steps",
            Self::StackLength => "stack length",
            Self::InitializedMemorySlots => "initialized memory slots",
        }
    }
}

impl fmt::Display for ExecutionResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The value or coercion named by [`HandlerExecutionError::UnsupportedValue`].
///
/// Primitive operators reuse their Display spelling, so the error text remains
/// the operator name (`+`, `unary +`, and so on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueOperation {
    ContextValue,
    Operand,
    NumericLiteral,
    Addition,
    LogicalNot,
    BitwiseNot,
    StringConversion,
    Operator(PrimitiveOperator),
}

impl ValueOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContextValue => "context value",
            Self::Operand => "operand",
            Self::NumericLiteral => "numeric literal",
            Self::Addition => "addition",
            Self::LogicalNot => "logical not",
            Self::BitwiseNot => "bitwise not",
            Self::StringConversion => "string conversion",
            Self::Operator(operator) => operator_name(operator),
        }
    }
}

impl fmt::Display for ValueOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The indexable target named by [`HandlerExecutionError::UnsupportedIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexTarget {
    Stack,
    MemorySlot,
}

impl IndexTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stack => "stack",
            Self::MemorySlot => "memory slot",
        }
    }
}

impl fmt::Display for IndexTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Structured failures. A thrown JavaScript payload is a
/// HandlerCompletion::Throw, not an executor error or an observation tracker
/// entry.
#[derive(Debug, Clone, PartialEq)]
pub enum HandlerExecutionError {
    TemplateUnavailable {
        status: TemplateStatus,
        semantic_identity: String,
        source_span: SpanRecord,
        reason: String,
    },
    InvalidRecipe {
        step: Option<u32>,
        reason: String,
    },
    MissingOperand {
        position: u32,
    },
    UndefinedTemporary {
        id: u32,
    },
    ReferenceError {
        slot: u32,
    },
    UnsupportedValue {
        operation: ValueOperation,
        reason: String,
    },
    UnsupportedIndex {
        target: IndexTarget,
        value: JsValue,
        reason: String,
    },
    MissingMemorySlotWrite {
        slot: u32,
    },
    InvalidStackLength {
        value: JsValue,
        reason: String,
    },
    ResourceLimit {
        resource: ExecutionResource,
        limit: usize,
    },
}

impl fmt::Display for HandlerExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TemplateUnavailable {
                status,
                semantic_identity,
                reason,
                ..
            } => write!(
                f,
                "handler template {semantic_identity} is {status}: {reason}"
            ),
            Self::InvalidRecipe { reason, .. } => write!(f, "invalid handler recipe: {reason}"),
            Self::MissingOperand { position } => write!(f, "missing handler operand {position}"),
            Self::UndefinedTemporary { id } => write!(f, "temporary {id} is not initialized"),
            Self::ReferenceError { slot } => {
                write!(f, "ReferenceError: memory slot {slot} is absent")
            }
            Self::UnsupportedValue { operation, reason } => {
                write!(f, "unsupported value for {operation}: {reason}")
            }
            Self::UnsupportedIndex {
                target,
                value,
                reason,
            } => write!(f, "unsupported {target} index {value:?}: {reason}"),
            Self::MissingMemorySlotWrite { slot } => {
                write!(
                    f,
                    "memory slot {slot} is missing; initialize it before writing"
                )
            }
            Self::InvalidStackLength { value, reason } => {
                write!(f, "invalid stack length {value:?}: {reason}")
            }
            Self::ResourceLimit { resource, limit } => {
                write!(f, "{resource} exceeds bounded limit {limit}")
            }
        }
    }
}

impl std::error::Error for HandlerExecutionError {}

/// One ordered stack mutation captured by an observation.
#[derive(Debug, Clone, PartialEq)]
pub struct StackChange {
    pub index: usize,
    pub before: Option<JsValue>,
    pub after: Option<JsValue>,
}

/// One ordered memory-slot mutation captured by an observation.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotChange {
    pub slot: u32,
    pub before: SlotState,
    pub after: SlotState,
}

/// Lightweight evidence for one recipe step. This is intentionally an event
/// record, not another semantic IR.
#[derive(Debug, Clone, PartialEq)]
pub struct HandlerObservation {
    pub handler_key: u32,
    pub semantic_identity: String,
    pub source_span: SpanRecord,
    pub step_order: Option<u32>,
    pub operands: Vec<JsValue>,
    pub result: Option<JsValue>,
    pub stack_changes: Vec<StackChange>,
    pub slot_changes: Vec<SlotChange>,
    pub completion: Option<HandlerCompletion>,
    pub error: Option<HandlerExecutionError>,
}

/// The context and observations are returned even when execution fails after
/// earlier ordered effects. Preflight failures return the original context
/// unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum HandlerExecution {
    Completed {
        completion: HandlerCompletion,
        context: HandlerContext,
        observations: Vec<HandlerObservation>,
    },
    Failed {
        error: HandlerExecutionError,
        context: HandlerContext,
        observations: Vec<HandlerObservation>,
    },
}

impl HandlerExecution {
    pub fn context(&self) -> &HandlerContext {
        match self {
            Self::Completed { context, .. } | Self::Failed { context, .. } => context,
        }
    }

    pub fn observations(&self) -> &[HandlerObservation] {
        match self {
            Self::Completed { observations, .. } | Self::Failed { observations, .. } => {
                observations
            }
        }
    }

    pub fn completion(&self) -> Option<&HandlerCompletion> {
        match self {
            Self::Completed { completion, .. } => Some(completion),
            Self::Failed { .. } => None,
        }
    }

    pub fn error(&self) -> Option<&HandlerExecutionError> {
        match self {
            Self::Completed { .. } => None,
            Self::Failed { error, .. } => Some(error),
        }
    }
}

/// Executes executable handler templates with explicit resource bounds.
#[derive(Debug, Clone, Copy, Default)]
pub struct HandlerExecutor {
    limits: HandlerExecutionLimits,
}

impl HandlerExecutor {
    pub fn new(limits: HandlerExecutionLimits) -> Self {
        Self { limits }
    }

    pub fn limits(&self) -> HandlerExecutionLimits {
        self.limits
    }

    pub fn execute(
        &self,
        template: &HandlerTemplate,
        context: HandlerContext,
        operands: &[JsValue],
    ) -> HandlerExecution {
        if let Err(error) = preflight(template, self.limits) {
            return HandlerExecution::Failed {
                error,
                context,
                observations: Vec::new(),
            };
        }
        if context.stack.len() > self.limits.max_stack_length {
            return HandlerExecution::Failed {
                error: HandlerExecutionError::ResourceLimit {
                    resource: ExecutionResource::InitialStackLength,
                    limit: self.limits.max_stack_length,
                },
                context,
                observations: Vec::new(),
            };
        }
        if context.slots.len() > self.limits.max_slots {
            return HandlerExecution::Failed {
                error: HandlerExecutionError::ResourceLimit {
                    resource: ExecutionResource::InitialInitializedMemorySlots,
                    limit: self.limits.max_slots,
                },
                context,
                observations: Vec::new(),
            };
        }

        let nan_slots = context
            .slots
            .iter()
            .filter_map(|(&slot, value)| is_nan_value(value).then_some(slot))
            .collect();
        let mut runtime = Runtime {
            context,
            operands,
            temporaries: BTreeMap::new(),
            limits: self.limits,
            nan_slots,
        };
        let mut observations = Vec::with_capacity(template.steps.len());

        for step in &template.steps {
            let mut mutations = MutationJournal::default();
            let mut used_operands = Vec::new();
            let operation = execute_operation(
                &mut runtime,
                &step.operation,
                &mut used_operands,
                &mut mutations,
            );
            let (result, completion, error) = match operation {
                Ok(effect) => (effect.result, effect.completion, None),
                Err(error) => (None, None, Some(error)),
            };
            let observation = make_observation(
                template,
                step,
                ObservationInput {
                    operands: used_operands,
                    result: result.clone(),
                    completion: completion.clone(),
                    error: error.clone(),
                    mutations: &mutations,
                    nan_slots: &runtime.nan_slots,
                    context: &runtime.context,
                },
            );
            observations.push(observation);

            if let Some(error) = error {
                return HandlerExecution::Failed {
                    error,
                    context: runtime.context,
                    observations,
                };
            }
            if let Some(completion) = completion {
                return HandlerExecution::Completed {
                    completion,
                    context: runtime.context,
                    observations,
                };
            }
        }

        let completion = HandlerCompletion::Return(JsValue::Undefined);
        observations.push(HandlerObservation {
            handler_key: template.table_key,
            semantic_identity: template.semantic_identity.to_string(),
            source_span: template.source_span.clone(),
            step_order: None,
            operands: Vec::new(),
            result: Some(JsValue::Undefined),
            stack_changes: Vec::new(),
            slot_changes: Vec::new(),
            completion: Some(completion.clone()),
            error: None,
        });
        HandlerExecution::Completed {
            completion,
            context: runtime.context,
            observations,
        }
    }
}

/// Convenience entry point using the default bounds.
pub fn execute_handler(
    template: &HandlerTemplate,
    context: HandlerContext,
    operands: &[JsValue],
) -> HandlerExecution {
    HandlerExecutor::default().execute(template, context, operands)
}

struct Runtime<'a> {
    context: HandlerContext,
    operands: &'a [JsValue],
    temporaries: BTreeMap<u32, JsValue>,
    limits: HandlerExecutionLimits,
    nan_slots: BTreeSet<u32>,
}

struct OperationEffect {
    result: Option<JsValue>,
    completion: Option<HandlerCompletion>,
}

#[derive(Default)]
struct MutationJournal {
    stack_before: BTreeMap<usize, Option<JsValue>>,
    slots_before: BTreeMap<u32, SlotState>,
}

impl MutationJournal {
    fn record_stack_before(&mut self, index: usize, before: Option<JsValue>) {
        self.stack_before.entry(index).or_insert(before);
    }

    fn record_slot_before(&mut self, slot: u32, before: SlotState) {
        self.slots_before.entry(slot).or_insert(before);
    }
}

fn preflight(
    template: &HandlerTemplate,
    limits: HandlerExecutionLimits,
) -> Result<(), HandlerExecutionError> {
    admit_recipe(
        template,
        RecipeBounds {
            max_steps: limits.max_steps,
            ..RecipeBounds::default()
        },
    )
    .map_err(|error| admission_error(template, error))?;
    // This interpreter models JavaScript primitives only. Object-model nodes
    // (bindings, property access, object/array construction) and VM control
    // nodes (PC, dispatch state, call frames) are executed by the symbolic
    // tracer and dispatch drivers instead, and are rejected here before any
    // effect.
    for step in &template.steps {
        if let Some(kind) = object_model_node(&step.operation) {
            return Err(HandlerExecutionError::InvalidRecipe {
                step: Some(step.order),
                reason: format!("{kind} is outside the concrete primitive executor"),
            });
        }
    }
    Ok(())
}

fn object_model_node(operation: &RecipeOperation) -> Option<&'static str> {
    fn value_kind(value: &SemanticValue) -> Option<&'static str> {
        let mut pending = vec![value];
        while let Some(value) = pending.pop() {
            match value {
                SemanticValue::Binding { .. } => return Some("binding"),
                SemanticValue::PropertyRead { .. } => return Some("property read"),
                SemanticValue::ObjectLiteral { .. } => return Some("object literal"),
                SemanticValue::ArrayLiteral { .. } => return Some("array literal"),
                SemanticValue::PoppedCallFrame { .. } => return Some("call frame read"),
                SemanticValue::StackSlice { .. } => return Some("stack slice"),
                SemanticValue::Call { .. } => return Some("program function call"),
                SemanticValue::CreateThread { .. } => return Some("VM closure creation"),
                SemanticValue::Receiver { .. } => return Some("VM function receiver"),
                SemanticValue::FunctionLiteral { .. } | SemanticValue::ClosurePlan { .. } => {
                    return Some("function expression")
                }
                SemanticValue::PoppedTryFrame { .. }
                | SemanticValue::PoppedErrorRecord { .. }
                | SemanticValue::PopThrownValue => return Some("VM exception state"),
                SemanticValue::Construct { .. } => return Some("constructor call"),
                SemanticValue::Delete { .. } => return Some("property delete"),
                SemanticValue::Binary { operator, .. } | SemanticValue::Unary { operator, .. }
                    if !operator.is_concrete() =>
                {
                    return Some(operator_name(*operator))
                }
                other => pending.extend(other.children()),
            }
        }
        None
    }
    match operation {
        RecipeOperation::PropertyWrite { .. } => Some("property write"),
        RecipeOperation::PushCallFrame => Some("call frame push"),
        RecipeOperation::PopCallFrame => Some("call frame pop"),
        RecipeOperation::SetProgramCounter { .. } => Some("program counter write"),
        RecipeOperation::SetDispatchState { .. } => Some("dispatch state write"),
        RecipeOperation::JumpIf { .. } => Some("conditional jump"),
        RecipeOperation::PushTryFrame { .. }
        | RecipeOperation::PopTryFrame
        | RecipeOperation::PushFinallyRecord
        | RecipeOperation::PopErrorRecord
        | RecipeOperation::RethrowIfHandled
        | RecipeOperation::DropThrownIfHandled
        | RecipeOperation::DropThrownUnderIfHandled => Some("VM exception handling"),
        RecipeOperation::CompleteThread { .. } => Some("VM function return"),
        other => other.values().into_iter().find_map(value_kind),
    }
}

fn admission_error(template: &HandlerTemplate, error: AdmissionError) -> HandlerExecutionError {
    match error {
        AdmissionError::TemplateStatus { status } => HandlerExecutionError::TemplateUnavailable {
            status,
            semantic_identity: template.semantic_identity.to_string(),
            source_span: template.source_span.clone(),
            reason: "only fully proved templates may execute".into(),
        },
        AdmissionError::UnsupportedFragment { reason, source } => {
            HandlerExecutionError::InvalidRecipe {
                step: None,
                reason: format!("unsupported handler fragment: {reason}: {source}"),
            }
        }
        AdmissionError::StepLimit { limit, .. } => HandlerExecutionError::ResourceLimit {
            resource: ExecutionResource::RecipeSteps,
            limit,
        },
        AdmissionError::StepOrder { step, .. } => HandlerExecutionError::InvalidRecipe {
            step: Some(step),
            reason: "recipe steps are not strictly source ordered".into(),
        },
        AdmissionError::UnknownOperation { step, reason } => HandlerExecutionError::InvalidRecipe {
            step: Some(step),
            reason: format!("unknown recipe operation: {reason}"),
        },
        AdmissionError::UnknownValue { step, reason } => HandlerExecutionError::InvalidRecipe {
            step: Some(step),
            reason: format!("unknown semantic value: {reason}"),
        },
        AdmissionError::ContextValue { step } => HandlerExecutionError::InvalidRecipe {
            step: Some(step),
            reason: "materializing the execution context as a JS value is unsupported".into(),
        },
        AdmissionError::TemporaryReadBeforeLet { step, id } => {
            HandlerExecutionError::InvalidRecipe {
                step: Some(step),
                reason: format!("temporary {id} is read before its let step"),
            }
        }
        AdmissionError::TemporaryAssignedBeforeLet { step, id } => {
            HandlerExecutionError::InvalidRecipe {
                step: Some(step),
                reason: format!("temporary {id} is assigned before its let step"),
            }
        }
        AdmissionError::DuplicateTemporary { step, id } => HandlerExecutionError::InvalidRecipe {
            step: Some(step),
            reason: format!("temporary {id} is defined more than once"),
        },
        AdmissionError::ExpressionWork { step, limit, .. } => {
            HandlerExecutionError::InvalidRecipe {
                step: Some(step),
                reason: format!("recipe expression work exceeds limit {limit}"),
            }
        }
        AdmissionError::ExpressionDepth { step, limit, .. } => {
            HandlerExecutionError::InvalidRecipe {
                step: Some(step),
                reason: format!("recipe expression depth exceeds configured limit {limit}"),
            }
        }
    }
}

fn execute_operation(
    runtime: &mut Runtime<'_>,
    operation: &RecipeOperation,
    used_operands: &mut Vec<JsValue>,
    mutations: &mut MutationJournal,
) -> Result<OperationEffect, HandlerExecutionError> {
    // `HandlerExecutor::execute` admits the complete template before this
    // interpreter starts, so structural rejection cases cannot reach runtime.
    let result = match operation {
        RecipeOperation::Let { temporary, value } => {
            let result = evaluate(runtime, value, used_operands)?;
            runtime.temporaries.insert(*temporary, result.clone());
            result
        }
        RecipeOperation::Assign { temporary, value } => {
            let result = evaluate(runtime, value, used_operands)?;
            if !runtime.temporaries.contains_key(temporary) {
                unreachable!("temporary assignment rejected by preflight");
            }
            runtime.temporaries.insert(*temporary, result.clone());
            result
        }
        RecipeOperation::StackWrite { index, value } => {
            let index = evaluate_index(runtime, index, used_operands, IndexTarget::Stack)?;
            let value = evaluate(runtime, value, used_operands)?;
            ensure_stack_capacity(runtime, index)?;
            let before = runtime.context.stack.get(index).cloned().unwrap_or(None);
            mutations.record_stack_before(index, before);
            if index >= runtime.context.stack.len() {
                runtime.context.stack.resize(index + 1, None);
            }
            runtime.context.stack[index] = Some(value.clone());
            value
        }
        RecipeOperation::StackLengthSet { value } => {
            let value = evaluate(runtime, value, used_operands)?;
            let length = stack_length(&value, runtime.limits.max_stack_length)?;
            if length < runtime.context.stack.len() {
                for (index, before) in runtime.context.stack.iter().enumerate().skip(length) {
                    if before.is_some() {
                        mutations.record_stack_before(index, before.clone());
                    }
                }
            }
            runtime.context.stack.resize(length, None);
            value
        }
        RecipeOperation::MemoryWrite { slot, value } => {
            let slot = evaluate_slot(runtime, slot, used_operands)?;
            let value = evaluate(runtime, value, used_operands)?;
            if !runtime.context.slots.contains_key(&slot) {
                return Err(HandlerExecutionError::MissingMemorySlotWrite { slot });
            }
            mutations.record_slot_before(
                slot,
                SlotState::Initialized(
                    runtime
                        .context
                        .slots
                        .get(&slot)
                        .expect("checked memory slot presence")
                        .clone(),
                ),
            );
            if is_nan_value(&value) {
                runtime.nan_slots.insert(slot);
            } else {
                runtime.nan_slots.remove(&slot);
            }
            runtime.context.slots.insert(slot, value.clone());
            value
        }
        RecipeOperation::MemoryInitialize { slot } => {
            let slot = evaluate_slot(runtime, slot, used_operands)?;
            let is_new = !runtime.context.slots.contains_key(&slot);
            if is_new && runtime.context.slots.len() >= runtime.limits.max_slots {
                return Err(HandlerExecutionError::ResourceLimit {
                    resource: ExecutionResource::InitializedMemorySlots,
                    limit: runtime.limits.max_slots,
                });
            }
            let before = runtime
                .context
                .slots
                .get(&slot)
                .cloned()
                .map_or(SlotState::Absent, SlotState::Initialized);
            mutations.record_slot_before(slot, before);
            runtime.nan_slots.remove(&slot);
            runtime.context.slots.insert(slot, JsValue::Undefined);
            JsValue::Undefined
        }
        RecipeOperation::Evaluate { value }
        | RecipeOperation::Return { value }
        | RecipeOperation::Throw { value } => evaluate(runtime, value, used_operands)?,
        RecipeOperation::Unknown { .. } => unreachable!("unknown operation rejected by preflight"),
        RecipeOperation::PropertyWrite { .. }
        | RecipeOperation::PushCallFrame
        | RecipeOperation::PopCallFrame
        | RecipeOperation::SetProgramCounter { .. }
        | RecipeOperation::SetDispatchState { .. }
        | RecipeOperation::JumpIf { .. }
        | RecipeOperation::CompleteThread { .. }
        | RecipeOperation::PushTryFrame { .. }
        | RecipeOperation::PopTryFrame
        | RecipeOperation::PushFinallyRecord
        | RecipeOperation::PopErrorRecord
        | RecipeOperation::RethrowIfHandled
        | RecipeOperation::DropThrownIfHandled
        | RecipeOperation::DropThrownUnderIfHandled => {
            unreachable!("object-model or VM control operation rejected by preflight")
        }
    };

    let completion = match operation {
        RecipeOperation::Return { .. } => Some(HandlerCompletion::Return(result.clone())),
        RecipeOperation::Throw { .. } => Some(HandlerCompletion::Throw(result.clone())),
        _ => None,
    };
    Ok(OperationEffect {
        result: Some(result),
        completion,
    })
}

fn evaluate(
    runtime: &Runtime<'_>,
    value: &SemanticValue,
    used_operands: &mut Vec<JsValue>,
) -> Result<JsValue, HandlerExecutionError> {
    match value {
        SemanticValue::Context => unreachable!("context value rejected by preflight"),
        SemanticValue::Operand { position } => {
            let Some(operand_position) = position.checked_sub(1) else {
                return Err(HandlerExecutionError::UnsupportedValue {
                    operation: ValueOperation::Operand,
                    reason: "recipe operand position 0 refers to the execution context".into(),
                });
            };
            let operand = runtime
                .operands
                .get(operand_position as usize)
                .ok_or(HandlerExecutionError::MissingOperand {
                    position: *position,
                })?
                .clone();
            used_operands.push(operand.clone());
            Ok(operand)
        }
        SemanticValue::Temporary { id } => Ok(runtime
            .temporaries
            .get(id)
            .cloned()
            .unwrap_or_else(|| unreachable!("temporary read rejected by preflight: {id}"))),
        SemanticValue::Literal(literal) => literal_value(literal),
        SemanticValue::StackLength => Ok(JsValue::Number(runtime.context.stack.len() as f64)),
        SemanticValue::StackRead { index } => {
            let index = evaluate_index(runtime, index, used_operands, IndexTarget::Stack)?;
            Ok(runtime
                .context
                .stack
                .get(index)
                .and_then(Clone::clone)
                .unwrap_or(JsValue::Undefined))
        }
        SemanticValue::MemoryRead { slot } => {
            let slot = evaluate_slot(runtime, slot, used_operands)?;
            runtime
                .context
                .slots
                .get(&slot)
                .cloned()
                .ok_or(HandlerExecutionError::ReferenceError { slot })
        }
        SemanticValue::Binary {
            operator,
            left,
            right,
        } => {
            let left = evaluate(runtime, left, used_operands)?;
            let right = evaluate(runtime, right, used_operands)?;
            apply_binary(*operator, left, right)
        }
        SemanticValue::Unary { operator, value } => {
            let value = evaluate(runtime, value, used_operands)?;
            apply_unary(*operator, value)
        }
        SemanticValue::Unknown { .. } => {
            unreachable!("unknown semantic value rejected by preflight")
        }
        SemanticValue::Binding { .. }
        | SemanticValue::PropertyRead { .. }
        | SemanticValue::ObjectLiteral { .. }
        | SemanticValue::ArrayLiteral { .. }
        | SemanticValue::PoppedCallFrame { .. }
        | SemanticValue::StackSlice { .. }
        | SemanticValue::Call { .. }
        | SemanticValue::CreateThread { .. }
        | SemanticValue::Receiver { .. }
        | SemanticValue::FunctionLiteral { .. }
        | SemanticValue::ClosurePlan { .. }
        | SemanticValue::Construct { .. }
        | SemanticValue::Delete { .. }
        | SemanticValue::PoppedTryFrame { .. }
        | SemanticValue::PoppedErrorRecord { .. }
        | SemanticValue::PopThrownValue => {
            unreachable!("object-model or VM control value rejected by preflight")
        }
    }
}

fn literal_value(value: &LiteralValue) -> Result<JsValue, HandlerExecutionError> {
    match value {
        LiteralValue::Number(raw) => parse_number(raw),
        LiteralValue::DecodedNumber { bits } => Ok(JsValue::Number(f64::from_bits(*bits))),
        LiteralValue::String(value) => Ok(JsValue::String(value.clone())),
        LiteralValue::Boolean(value) => Ok(JsValue::Boolean(*value)),
        LiteralValue::Null => Ok(JsValue::Null),
        LiteralValue::Undefined => Ok(JsValue::Undefined),
    }
}

fn parse_number(raw: &str) -> Result<JsValue, HandlerExecutionError> {
    parse_numeric_literal(raw)
        .map(JsValue::Number)
        .ok_or_else(|| HandlerExecutionError::UnsupportedValue {
            operation: ValueOperation::NumericLiteral,
            reason: format!("unsupported JavaScript number literal {raw:?}"),
        })
}

fn evaluate_index(
    runtime: &Runtime<'_>,
    value: &SemanticValue,
    used_operands: &mut Vec<JsValue>,
    target: IndexTarget,
) -> Result<usize, HandlerExecutionError> {
    let value = evaluate(runtime, value, used_operands)?;
    let JsValue::Number(number) = value.clone() else {
        return Err(HandlerExecutionError::UnsupportedIndex {
            target,
            value,
            reason: "only finite non-negative integer Number indices are supported".into(),
        });
    };
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 {
        return Err(HandlerExecutionError::UnsupportedIndex {
            target,
            value,
            reason: "only finite non-negative integer Number indices are supported".into(),
        });
    }
    let index = number as u128;
    if index > usize::MAX as u128 || index > u32::MAX as u128 - 1 {
        return Err(HandlerExecutionError::UnsupportedIndex {
            target,
            value,
            reason: "index exceeds the JavaScript array-index range".into(),
        });
    }
    Ok(index as usize)
}

fn evaluate_slot(
    runtime: &Runtime<'_>,
    value: &SemanticValue,
    used_operands: &mut Vec<JsValue>,
) -> Result<u32, HandlerExecutionError> {
    let value = evaluate(runtime, value, used_operands)?;
    let JsValue::Number(number) = value.clone() else {
        return Err(HandlerExecutionError::UnsupportedIndex {
            target: IndexTarget::MemorySlot,
            value,
            reason: "only finite non-negative integer Number slots are supported".into(),
        });
    };
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number > u32::MAX as f64 {
        return Err(HandlerExecutionError::UnsupportedIndex {
            target: IndexTarget::MemorySlot,
            value,
            reason: "only finite non-negative integer Number slots are supported".into(),
        });
    }
    Ok(number as u32)
}

fn ensure_stack_capacity(runtime: &Runtime<'_>, index: usize) -> Result<(), HandlerExecutionError> {
    if index >= runtime.limits.max_stack_length {
        return Err(HandlerExecutionError::ResourceLimit {
            resource: ExecutionResource::StackLength,
            limit: runtime.limits.max_stack_length,
        });
    }
    Ok(())
}

fn stack_length(value: &JsValue, limit: usize) -> Result<usize, HandlerExecutionError> {
    let JsValue::Number(number) = value else {
        return Err(HandlerExecutionError::InvalidStackLength {
            value: value.clone(),
            reason: "stack length requires a finite integer Number".into(),
        });
    };
    if !number.is_finite() || *number < 0.0 || number.fract() != 0.0 {
        return Err(HandlerExecutionError::InvalidStackLength {
            value: value.clone(),
            reason: "stack length requires a finite non-negative integer Number".into(),
        });
    }
    let length = *number as u128;
    if length > limit as u128 {
        return Err(HandlerExecutionError::ResourceLimit {
            resource: ExecutionResource::StackLength,
            limit,
        });
    }
    Ok(length as usize)
}

fn apply_binary(
    operator: PrimitiveOperator,
    left: JsValue,
    right: JsValue,
) -> Result<JsValue, HandlerExecutionError> {
    if operator == PrimitiveOperator::JsAdd {
        return js_add(left, right);
    }
    let left = to_number(left, ValueOperation::Operator(operator))?;
    let right = to_number(right, ValueOperation::Operator(operator))?;
    let result = match operator {
        PrimitiveOperator::Subtract => left - right,
        PrimitiveOperator::Multiply => left * right,
        PrimitiveOperator::Divide => left / right,
        PrimitiveOperator::Remainder => left % right,
        PrimitiveOperator::ShiftLeft => {
            f64::from((to_int32(left) as i64).wrapping_shl(to_uint32(right) & 31) as i32)
        }
        PrimitiveOperator::ShiftRight => f64::from(to_int32(left) >> (to_uint32(right) & 31)),
        PrimitiveOperator::ShiftRightZeroFill => {
            f64::from(to_uint32(left) >> (to_uint32(right) & 31))
        }
        PrimitiveOperator::BitwiseOr => f64::from(to_int32(left) | to_int32(right)),
        PrimitiveOperator::BitwiseXor => f64::from(to_int32(left) ^ to_int32(right)),
        PrimitiveOperator::BitwiseAnd => f64::from(to_int32(left) & to_int32(right)),
        PrimitiveOperator::JsAdd
        | PrimitiveOperator::UnaryPlus
        | PrimitiveOperator::UnaryMinus
        | PrimitiveOperator::BitwiseNot
        | PrimitiveOperator::LogicalNot
        | PrimitiveOperator::Void
        | PrimitiveOperator::Typeof => {
            return Err(HandlerExecutionError::UnsupportedValue {
                operation: ValueOperation::Operator(operator),
                reason: "operator is not binary".into(),
            })
        }
        PrimitiveOperator::Exponent
        | PrimitiveOperator::Equal
        | PrimitiveOperator::NotEqual
        | PrimitiveOperator::StrictEqual
        | PrimitiveOperator::StrictNotEqual
        | PrimitiveOperator::LessThan
        | PrimitiveOperator::LessThanOrEqual
        | PrimitiveOperator::GreaterThan
        | PrimitiveOperator::GreaterThanOrEqual
        | PrimitiveOperator::In
        | PrimitiveOperator::InstanceOf => {
            unreachable!("symbolic-only operator rejected by preflight")
        }
    };
    Ok(JsValue::Number(result))
}

fn apply_unary(
    operator: PrimitiveOperator,
    value: JsValue,
) -> Result<JsValue, HandlerExecutionError> {
    match operator {
        PrimitiveOperator::LogicalNot => match value {
            JsValue::Undefined | JsValue::Null => Ok(JsValue::Boolean(true)),
            JsValue::Boolean(value) => Ok(JsValue::Boolean(!value)),
            JsValue::Number(value) => Ok(JsValue::Boolean(value == 0.0 || value.is_nan())),
            JsValue::String(value) => Ok(JsValue::Boolean(value.is_empty())),
            other => Err(unsupported_coercion(ValueOperation::LogicalNot, other)),
        },
        PrimitiveOperator::UnaryPlus => Ok(JsValue::Number(to_number(
            value,
            ValueOperation::Operator(PrimitiveOperator::UnaryPlus),
        )?)),
        PrimitiveOperator::UnaryMinus => Ok(JsValue::Number(-to_number(
            value,
            ValueOperation::Operator(PrimitiveOperator::UnaryMinus),
        )?)),
        PrimitiveOperator::BitwiseNot => Ok(JsValue::Number(f64::from(!to_int32(to_number(
            value,
            ValueOperation::BitwiseNot,
        )?)))),
        PrimitiveOperator::Void => Ok(JsValue::Undefined),
        _ => Err(HandlerExecutionError::UnsupportedValue {
            operation: ValueOperation::Operator(operator),
            reason: "operator is not unary".into(),
        }),
    }
}

fn js_add(left: JsValue, right: JsValue) -> Result<JsValue, HandlerExecutionError> {
    let left = to_primitive(left, ValueOperation::Addition)?;
    let right = to_primitive(right, ValueOperation::Addition)?;
    if matches!(left, JsValue::String(_)) || matches!(right, JsValue::String(_)) {
        return Ok(JsValue::String(format!(
            "{}{}",
            to_js_string(&left)?,
            to_js_string(&right)?
        )));
    }
    Ok(JsValue::Number(
        to_number(left, ValueOperation::Addition)? + to_number(right, ValueOperation::Addition)?,
    ))
}

fn to_primitive(
    value: JsValue,
    operation: ValueOperation,
) -> Result<JsValue, HandlerExecutionError> {
    match value {
        JsValue::Object | JsValue::Symbol | JsValue::BigInt => {
            Err(unsupported_coercion(operation, value))
        }
        value => Ok(value),
    }
}

fn to_number(value: JsValue, operation: ValueOperation) -> Result<f64, HandlerExecutionError> {
    match value {
        JsValue::Undefined => Ok(f64::NAN),
        JsValue::Null => Ok(0.0),
        JsValue::Boolean(value) => Ok(if value { 1.0 } else { 0.0 }),
        JsValue::Number(value) => Ok(value),
        JsValue::String(value) => Ok(string_to_number(&value)),
        other => Err(unsupported_coercion(operation, other)),
    }
}

fn to_js_string(value: &JsValue) -> Result<String, HandlerExecutionError> {
    match value {
        JsValue::Undefined => Ok("undefined".into()),
        JsValue::Null => Ok("null".into()),
        JsValue::Boolean(value) => Ok(value.to_string()),
        JsValue::Number(value) => Ok(number_to_js_string(*value)),
        JsValue::String(value) => Ok(value.clone()),
        other => Err(unsupported_coercion(
            ValueOperation::StringConversion,
            other.clone(),
        )),
    }
}

fn unsupported_coercion(operation: ValueOperation, value: JsValue) -> HandlerExecutionError {
    HandlerExecutionError::UnsupportedValue {
        operation,
        reason: format!("coercion of {value:?} is outside the proved primitive subset"),
    }
}

const fn operator_name(operator: PrimitiveOperator) -> &'static str {
    match operator {
        PrimitiveOperator::JsAdd => "+",
        PrimitiveOperator::Subtract => "-",
        PrimitiveOperator::Multiply => "*",
        PrimitiveOperator::Divide => "/",
        PrimitiveOperator::Remainder => "%",
        PrimitiveOperator::ShiftLeft => "<<",
        PrimitiveOperator::ShiftRight => ">>",
        PrimitiveOperator::ShiftRightZeroFill => ">>>",
        PrimitiveOperator::BitwiseOr => "|",
        PrimitiveOperator::BitwiseXor => "^",
        PrimitiveOperator::BitwiseAnd => "&",
        PrimitiveOperator::UnaryPlus => "unary +",
        PrimitiveOperator::UnaryMinus => "unary -",
        PrimitiveOperator::BitwiseNot => "~",
        PrimitiveOperator::LogicalNot => "!",
        PrimitiveOperator::Void => "void",
        PrimitiveOperator::Exponent => "**",
        PrimitiveOperator::Equal => "==",
        PrimitiveOperator::NotEqual => "!=",
        PrimitiveOperator::StrictEqual => "===",
        PrimitiveOperator::StrictNotEqual => "!==",
        PrimitiveOperator::LessThan => "<",
        PrimitiveOperator::LessThanOrEqual => "<=",
        PrimitiveOperator::GreaterThan => ">",
        PrimitiveOperator::GreaterThanOrEqual => ">=",
        PrimitiveOperator::In => "in",
        PrimitiveOperator::InstanceOf => "instanceof",
        PrimitiveOperator::Typeof => "typeof",
    }
}

struct ObservationInput<'a> {
    operands: Vec<JsValue>,
    result: Option<JsValue>,
    completion: Option<HandlerCompletion>,
    error: Option<HandlerExecutionError>,
    mutations: &'a MutationJournal,
    nan_slots: &'a BTreeSet<u32>,
    context: &'a HandlerContext,
}

fn make_observation(
    template: &HandlerTemplate,
    step: &RecipeStep,
    input: ObservationInput<'_>,
) -> HandlerObservation {
    let stack_changes = input
        .mutations
        .stack_before
        .iter()
        .filter_map(|(&index, before)| {
            let after = input.context.stack.get(index).cloned().unwrap_or(None);
            (!same_value_option(before, &after)).then_some(StackChange {
                index,
                before: before.clone(),
                after,
            })
        })
        .collect();
    let mut observed_slots = input.nan_slots.clone();
    observed_slots.extend(input.mutations.slots_before.keys().copied());
    let slot_changes = observed_slots
        .into_iter()
        .filter_map(|slot| {
            let after = input
                .context
                .slots
                .get(&slot)
                .cloned()
                .map_or(SlotState::Absent, SlotState::Initialized);
            let before = input
                .mutations
                .slots_before
                .get(&slot)
                .cloned()
                .unwrap_or_else(|| after.clone());
            (before != after).then_some(SlotChange {
                slot,
                before,
                after,
            })
        })
        .collect();
    HandlerObservation {
        handler_key: template.table_key,
        semantic_identity: template.semantic_identity.to_string(),
        source_span: step.span.clone(),
        step_order: Some(step.order),
        operands: input.operands,
        result: input.result,
        stack_changes,
        slot_changes,
        completion: input.completion,
        error: input.error,
    }
}

fn same_value_option(left: &Option<JsValue>, right: &Option<JsValue>) -> bool {
    match (left, right) {
        (Some(JsValue::Number(left)), Some(JsValue::Number(right))) => {
            left.to_bits() == right.to_bits()
        }
        _ => left == right,
    }
}

fn is_nan_value(value: &JsValue) -> bool {
    matches!(value, JsValue::Number(value) if value.is_nan())
}
