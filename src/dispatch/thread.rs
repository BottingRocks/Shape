//! Bounded, caller-supplied thread startup and incoming-context construction.
//!
//! [`DispatchPreparation::with_entry`] binds a validated VM coordinate to a
//! proof-backed preparation. This module complements it with the missing
//! *construction* step: a caller-supplied [`ThreadDescriptor`] plus a
//! [`ThreadInputs`] capture snapshot are turned into the incoming
//! [`TraceContext`] for that coordinate, in the same order as the canonical
//! `vmCaptureThread`/`vmBuildThreadInvocation` helpers:
//!
//! 1. captured-memory snapshot (`capturedSlots` copied from the explicit
//!    incoming values),
//! 2. optional `resultSlot` self-reference capture,
//! 3. per-invocation clone of the captured memory,
//! 4. optional `argumentsSlot` aggregate write,
//! 5. `initialSlots` initialization, replacing prior cells,
//! 6. `argumentSlots` positional writes, then `undefined` for missing ones.
//!
//! Positional invocation arguments follow the canonical `Math.min` mapping:
//! only the first `argumentSlots.len()` values reach the argument slots, and
//! extra arguments within the documented invocation-argument bound are
//! accepted and ignored. The incoming argument iterator is bounded before it is
//! collected, so an oversized input never allocates or consumes more than
//! `bound + 1` values. Every `argumentSlots` target must already own a cell
//! from the capture/result/aggregate/initialization steps, because the
//! canonical `writeSlot` updates an existing cell rather than creating one; a
//! missing target is rejected instead of silently allocated.
//!
//! Provenance honesty: the descriptor and inputs are *caller-supplied
//! parameters*, not facts derived from the analyzed source. They are never
//! advertised as source-proven. The prepared dispatch proof (image, table,
//! operand descriptors, handler recipes, and completion contract) is still
//! required and is never replaced by these values.
//!
//! Scope honesty: this is an invocation snapshot, not live shared
//! closure-cell alias support. Captured values are copied into a fresh
//! context for each invocation; requests that purport to model a live shared
//! cell are rejected ([`ThreadStartupError::UnsupportedSharedCapture`]).
//! The receiver (`this`) defaults to `undefined`, as in the canonical plain
//! call `vmCaptureThread(...)()`; handlers read it as `context.receiverValue`
//! and `context.receiverObject`.

use std::{borrow::Borrow, collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    DispatchEntryError, DispatchInvocation, DispatchPreparation, DispatchStart,
    DispatchTraceReport, SymbolicValue, TraceContext, TraceContextError, TraceDebugOptions,
    TraceLimits,
};

/// Whether a captured slot is an independent invocation snapshot or a live
/// shared closure cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CapturePolicy {
    /// Copy the incoming value into an invocation snapshot. This is the only
    /// supported policy; every invocation starts from its own clone.
    #[default]
    Snapshot,
    /// Model a live, aliased closure cell shared with the caller's memory.
    /// Unsupported: this API captures values, not shared cells. Constructing a
    /// thread with this policy fails closed.
    LiveShared,
}

/// A caller-supplied VM thread descriptor.
///
/// These are the same slot-role fields carried by `vmThreadDescriptors`, but
/// this value is *not* proven from the source: the caller declares it. The
/// dispatch image, table, operand descriptors, and handler recipes still come
/// from the proof-backed [`DispatchPreparation`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ThreadDescriptor {
    /// Slots initialized to `undefined`, replacing any captured cell.
    pub initial_slots: Vec<u32>,
    /// Slots copied from the caller's explicit incoming values.
    pub captured_slots: Vec<u32>,
    /// Slots receiving the positional invocation arguments.
    ///
    /// Each slot must already have a cell from `captured_slots`,
    /// `initial_slots`, `arguments_slot`, or `result_slot`; the positional
    /// write updates an existing cell and never creates one. A missing target
    /// is rejected with [`ThreadStartupError::MissingArgumentSlot`]. Duplicate
    /// entries are allowed and are written in order, so the last write wins,
    /// exactly as the canonical `argumentSlots` loop does.
    pub argument_slots: Vec<u32>,
    /// Optional slot receiving the opaque aggregate `arguments` object. The
    /// supplied value must be [`SymbolicValue::Object`] or
    /// [`SymbolicValue::External`]; a primitive is rejected.
    pub arguments_slot: Option<u32>,
    /// Optional slot receiving the thread's opaque self/function value. The
    /// supplied value must be [`SymbolicValue::Object`] or
    /// [`SymbolicValue::External`]; a primitive is rejected.
    pub result_slot: Option<u32>,
    /// How captured values relate to the caller's incoming memory.
    pub capture_policy: CapturePolicy,
}

impl ThreadDescriptor {
    /// An empty descriptor: no captured, initial, argument, or result slots.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the slots initialized to `undefined` before arguments are written.
    #[must_use]
    pub fn initial_slots(mut self, slots: impl IntoIterator<Item = u32>) -> Self {
        self.initial_slots = slots.into_iter().collect();
        self
    }

    /// Set the slots copied from the explicit incoming capture values.
    #[must_use]
    pub fn captured_slots(mut self, slots: impl IntoIterator<Item = u32>) -> Self {
        self.captured_slots = slots.into_iter().collect();
        self
    }

    /// Set the slots receiving positional invocation arguments.
    #[must_use]
    pub fn argument_slots(mut self, slots: impl IntoIterator<Item = u32>) -> Self {
        self.argument_slots = slots.into_iter().collect();
        self
    }

    /// Select the slot receiving the opaque aggregate `arguments` object.
    #[must_use]
    pub fn arguments_slot(mut self, slot: u32) -> Self {
        self.arguments_slot = Some(slot);
        self
    }

    /// Select the slot receiving the thread's opaque self/function value.
    #[must_use]
    pub fn result_slot(mut self, slot: u32) -> Self {
        self.result_slot = Some(slot);
        self
    }

    /// Select the capture policy. [`CapturePolicy::LiveShared`] always fails.
    #[must_use]
    pub fn capture_policy(mut self, policy: CapturePolicy) -> Self {
        self.capture_policy = policy;
        self
    }
}

/// Caller-supplied incoming values for one thread capture.
///
/// `captured_memory` must supply exactly one explicit value for every slot in
/// [`ThreadDescriptor::captured_slots`]. `SymbolicValue::Undefined` is an
/// explicit value: a missing capture is an error, never a silent placeholder.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThreadInputs {
    /// The explicit incoming value for each captured slot. Values are copied
    /// into the invocation snapshot; they are never aliased.
    pub captured_memory: BTreeMap<u32, SymbolicValue>,
    /// An explicit incoming stack snapshot. Empty means the stack starts
    /// empty; `None` entries are holes, while `Some(Undefined)` is an explicit
    /// value write.
    pub incoming_stack: Vec<Option<SymbolicValue>>,
    /// The opaque aggregate `arguments` object. Required when
    /// [`ThreadDescriptor::arguments_slot`] is selected. `SymbolicValue` has no
    /// arrays, so this must be an explicitly supplied opaque object-like value
    /// ([`SymbolicValue::Object`] or [`SymbolicValue::External`]); a primitive
    /// is rejected and the object is never simulated.
    pub arguments_object: Option<SymbolicValue>,
    /// The opaque self/function value. Required when
    /// [`ThreadDescriptor::result_slot`] is selected, for the same reason as
    /// `arguments_object`. Must be [`SymbolicValue::Object`] or
    /// [`SymbolicValue::External`].
    pub self_value: Option<SymbolicValue>,
    /// The receiver (`this`) the thread is called with. `None` is a plain
    /// call: `undefined`.
    pub receiver: Option<SymbolicValue>,
    /// Bounds applied to both context construction and dispatch execution.
    pub limits: TraceLimits,
    /// Optional maximum number of positional invocation arguments.
    ///
    /// This is a resource bound on the caller's argument iterator, independent
    /// of the descriptor's `argumentSlots` arity. `None` derives the bound from
    /// [`Self::limits`]' `max_stack_length` (so the default is 4096). Context
    /// construction stops the iterator after `bound + 1` values, so an
    /// oversized or infinite iterator never runs away.
    pub max_invocation_arguments: Option<usize>,
}

impl ThreadInputs {
    /// Empty inputs with default [`TraceLimits`] and an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the trace bounds used for construction and execution.
    #[must_use]
    pub fn limits(mut self, limits: TraceLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Override the positional invocation-argument bound. See
    /// [`Self::max_invocation_arguments`].
    #[must_use]
    pub fn max_invocation_arguments(mut self, limit: usize) -> Self {
        self.max_invocation_arguments = Some(limit);
        self
    }

    /// Supply the explicit incoming value for one captured slot.
    #[must_use]
    pub fn capture(mut self, slot: u32, value: SymbolicValue) -> Self {
        self.captured_memory.insert(slot, value);
        self
    }

    /// Supply an explicit incoming stack snapshot.
    #[must_use]
    pub fn stack(mut self, snapshot: impl IntoIterator<Item = Option<SymbolicValue>>) -> Self {
        self.incoming_stack = snapshot.into_iter().collect();
        self
    }

    /// Supply the opaque aggregate `arguments` object explicitly.
    #[must_use]
    pub fn arguments_object(mut self, value: SymbolicValue) -> Self {
        self.arguments_object = Some(value);
        self
    }

    /// Supply the aggregate `arguments` object as a named external value. The
    /// value is opaque; no array semantics are simulated.
    #[must_use]
    pub fn external_arguments_object(mut self, name: impl Into<String>) -> Self {
        self.arguments_object = Some(SymbolicValue::External { name: name.into() });
        self
    }

    /// Supply the opaque self/function value explicitly.
    #[must_use]
    pub fn self_value(mut self, value: SymbolicValue) -> Self {
        self.self_value = Some(value);
        self
    }

    /// Supply the self/function value as a named external value. The value is
    /// opaque; no function semantics are simulated.
    #[must_use]
    pub fn external_self_value(mut self, name: impl Into<String>) -> Self {
        self.self_value = Some(SymbolicValue::External { name: name.into() });
        self
    }

    /// Supply the receiver (`this`) the thread is called with.
    #[must_use]
    pub fn receiver(mut self, value: SymbolicValue) -> Self {
        self.receiver = Some(value);
        self
    }
}

/// Why a thread startup request was rejected. Every variant is produced before
/// any trace executes, so a rejected request never commits partial state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadStartupError {
    /// The requested entry is outside the prepared image/table.
    Entry(DispatchEntryError),
    /// A live shared closure-cell capture was requested.
    UnsupportedSharedCapture,
    /// A captured slot appears more than once in the descriptor.
    DuplicateCapturedSlot { slot: u32 },
    /// A captured slot has no explicit incoming value.
    MissingCapture { slot: u32 },
    /// An incoming capture names a slot that is not captured.
    UnexpectedCapture { slot: u32 },
    /// The selected `argumentsSlot` has no explicit aggregate value.
    MissingArgumentsObject { slot: u32 },
    /// The selected `resultSlot` has no explicit self value.
    MissingSelfValue { slot: u32 },
    /// The selected `argumentsSlot` value is not an opaque object-like value.
    InvalidArgumentsObject,
    /// The selected `resultSlot` value is not an opaque object-like value.
    InvalidSelfValue,
    /// An `argumentSlots` target has no cell to write. Positional writes update
    /// existing cells only; the slot must come from capture/result/aggregate/
    /// initialization first.
    MissingArgumentSlot { slot: u32 },
    /// An aggregate value was supplied without a selected `argumentsSlot`.
    UnexpectedArgumentsObject,
    /// A self value was supplied without a selected `resultSlot`.
    UnexpectedSelfValue,
    /// The invocation argument iterator exceeded its configured resource bound.
    /// `provided` is the observed lower bound (`limit + 1`); the remaining
    /// values are never consumed.
    TooManyArguments { provided: usize, limit: usize },
    /// A descriptor slot list exceeds the configured memory bound.
    TooManySlots {
        what: &'static str,
        count: usize,
        limit: usize,
    },
    /// The incoming stack snapshot exceeds the configured stack bound.
    IncomingStackTooLong { length: usize, limit: usize },
    /// A supplied value is a `Computed` node that cannot come from this arena.
    ComputedInput { what: &'static str },
    /// A bounded context write was rejected. This is a defense-in-depth path;
    /// inputs are pre-validated before construction.
    Context(TraceContextError),
}

impl fmt::Display for ThreadStartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Entry(error) => write!(f, "thread entry rejected: {error}"),
            Self::UnsupportedSharedCapture => f.write_str(
                "live shared closure-cell capture is unsupported; captured values are snapshots",
            ),
            Self::DuplicateCapturedSlot { slot } => {
                write!(f, "captured slot {slot} is declared more than once")
            }
            Self::MissingCapture { slot } => write!(
                f,
                "captured slot {slot} has no explicit incoming value; missing captures are errors"
            ),
            Self::UnexpectedCapture { slot } => write!(
                f,
                "incoming value for slot {slot} does not name a captured slot"
            ),
            Self::MissingArgumentsObject { slot } => write!(
                f,
                "arguments slot {slot} was selected without an explicit opaque arguments value"
            ),
            Self::MissingSelfValue { slot } => write!(
                f,
                "result slot {slot} was selected without an explicit opaque self value"
            ),
            Self::InvalidArgumentsObject => f.write_str(
                "arguments object must be an opaque Object or External value, not a primitive",
            ),
            Self::InvalidSelfValue => f.write_str(
                "self value must be an opaque Object or External value, not a primitive",
            ),
            Self::MissingArgumentSlot { slot } => write!(
                f,
                "argument slot {slot} has no existing cell; positional writes only update captured, result, aggregate, or initialized slots"
            ),
            Self::UnexpectedArgumentsObject => {
                f.write_str("an arguments object was supplied but no arguments slot is selected")
            }
            Self::UnexpectedSelfValue => {
                f.write_str("a self value was supplied but no result slot is selected")
            }
            Self::TooManyArguments { provided, limit } => write!(
                f,
                "more than {limit} positional arguments were supplied (observed at least {provided}); the argument iterator is stopped at the bound"
            ),
            Self::TooManySlots { what, count, limit } => {
                write!(f, "{what} has {count} entries, exceeding the configured bound {limit}")
            }
            Self::IncomingStackTooLong { length, limit } => write!(
                f,
                "incoming stack length {length} exceeds the configured stack bound {limit}"
            ),
            Self::ComputedInput { what } => write!(
                f,
                "{what} is a Computed node; computed values must come from the trace arena"
            ),
            Self::Context(error) => write!(f, "thread context construction rejected: {error}"),
        }
    }
}

impl std::error::Error for ThreadStartupError {}

impl From<TraceContextError> for ThreadStartupError {
    fn from(error: TraceContextError) -> Self {
        Self::Context(error)
    }
}

impl From<DispatchEntryError> for ThreadStartupError {
    fn from(error: DispatchEntryError) -> Self {
        Self::Entry(error)
    }
}

/// A validated, reusable thread invocation: a proof-backed entry plus a
/// caller-supplied descriptor and capture snapshot.
///
/// Each call to [`Self::invoke`] clones the captured memory and applies the
/// invocation arguments, so invocations are independent and never share live
/// cells.
#[derive(Debug, Clone)]
pub struct ThreadInvocation<'a> {
    invocation: DispatchInvocation<'a>,
    descriptor: ThreadDescriptor,
    captured_memory: BTreeMap<u32, SymbolicValue>,
    arguments_object: Option<SymbolicValue>,
    receiver: SymbolicValue,
    incoming_stack: Vec<Option<SymbolicValue>>,
    trace_limits: TraceLimits,
    max_invocation_arguments: usize,
}

impl<'a> ThreadInvocation<'a> {
    /// The validated entry used by this invocation.
    #[must_use]
    pub fn entry(&self) -> DispatchStart {
        self.invocation.start()
    }

    /// The caller-supplied descriptor. It is not source-proven.
    #[must_use]
    pub fn descriptor(&self) -> &ThreadDescriptor {
        &self.descriptor
    }

    /// The captured snapshot written into every invocation's memory.
    #[must_use]
    pub fn captured_memory(&self) -> &BTreeMap<u32, SymbolicValue> {
        &self.captured_memory
    }

    /// The bounds applied to construction and execution.
    #[must_use]
    pub fn trace_limits(&self) -> TraceLimits {
        self.trace_limits
    }

    /// The positional invocation-argument resource bound. See
    /// [`ThreadInputs::max_invocation_arguments`].
    #[must_use]
    pub fn max_invocation_arguments(&self) -> usize {
        self.max_invocation_arguments
    }

    /// The immutable preparation that owns the source proof.
    #[must_use]
    pub fn preparation(&self) -> &'a DispatchPreparation {
        self.invocation.preparation()
    }

    /// Build a fresh incoming context for one invocation in canonical order:
    /// the captured snapshot, the optional aggregate write, `initialSlots`,
    /// then the positional arguments (and `undefined` for missing ones). The
    /// incoming stack snapshot is applied first; the stack is empty when none
    /// was supplied. The context is validated and fully built before it is
    /// returned, so a rejected request leaves no partial state.
    ///
    /// `args` may be any iterator of owned values or references. It is stopped
    /// at [`ThreadInvocation::max_invocation_arguments`] `+ 1`, so an oversized
    /// or infinite iterator is never fully collected; exceeding the bound is a
    /// typed error. Extra arguments within the bound but beyond the descriptor's
    /// `argumentSlots` arity are accepted and ignored, matching the canonical
    /// `Math.min(arguments.length, argumentSlotCount)` mapping.
    pub fn build_context<I, V>(&self, args: I) -> Result<TraceContext, ThreadStartupError>
    where
        I: IntoIterator<Item = V>,
        V: Borrow<SymbolicValue>,
    {
        let provided = collect_invocation_arguments(args, self.max_invocation_arguments)?;
        build_context_from_spec(
            &ThreadContextSpec {
                descriptor: &self.descriptor,
                captured_memory: &self.captured_memory,
                arguments_object: self.arguments_object.as_ref(),
                receiver: &self.receiver,
                incoming_stack: &self.incoming_stack,
                trace_limits: self.trace_limits,
            },
            &provided,
        )
    }

    /// Execute this invocation with the canonical context order. Dispatch
    /// halts are reported in the [`DispatchTraceReport`]; only a rejected
    /// input or a rejected entry returns `Err`, and that happens before any
    /// handler runs.
    pub fn invoke<I, V>(&self, args: I) -> Result<DispatchTraceReport, ThreadStartupError>
    where
        I: IntoIterator<Item = V>,
        V: Borrow<SymbolicValue>,
    {
        self.invoke_with_debug(args, TraceDebugOptions::disabled())
    }

    /// Execute this invocation with live per-operation debug output.
    pub fn invoke_with_debug<I, V>(
        &self,
        args: I,
        debug: TraceDebugOptions,
    ) -> Result<DispatchTraceReport, ThreadStartupError>
    where
        I: IntoIterator<Item = V>,
        V: Borrow<SymbolicValue>,
    {
        let context = self.build_context(args)?;
        Ok(self
            .invocation
            .execute_with_debug(context, self.trace_limits, debug))
    }
}

impl DispatchPreparation {
    /// Bind a validated entry and construct a reusable thread invocation from
    /// a caller-supplied descriptor and capture snapshot.
    ///
    /// The entry is validated first with [`Self::with_entry`]; the descriptor
    /// and inputs are then validated against the supplied [`TraceLimits`].
    /// The positional invocation-argument bound is
    /// [`ThreadInputs::max_invocation_arguments`] when set, otherwise the
    /// supplied `max_stack_length`. Any error aborts before a context is built,
    /// so the preparation is never partially mutated. Passing [`Self::start`]
    /// as `entry` traces the root coordinate through exactly this operation;
    /// there is no separate boot path.
    pub fn thread(
        &self,
        entry: DispatchStart,
        descriptor: ThreadDescriptor,
        inputs: ThreadInputs,
    ) -> Result<ThreadInvocation<'_>, ThreadStartupError> {
        let invocation = self.with_entry(entry)?;
        validate_descriptor(&descriptor, &inputs)?;
        let captured_memory = build_captured_memory(&descriptor, &inputs)?;
        let max_invocation_arguments = inputs
            .max_invocation_arguments
            .unwrap_or(inputs.limits.max_stack_length);
        Ok(ThreadInvocation {
            invocation,
            descriptor,
            captured_memory,
            arguments_object: inputs.arguments_object,
            receiver: inputs.receiver.unwrap_or(SymbolicValue::Undefined),
            incoming_stack: inputs.incoming_stack,
            trace_limits: inputs.limits,
            max_invocation_arguments,
        })
    }
}

/// Inputs shared by every canonical thread-context construction. The prefix
/// driver reuses exactly this builder so both dispatch paths initialize a
/// context in the same order.
pub(super) struct ThreadContextSpec<'a> {
    pub descriptor: &'a ThreadDescriptor,
    pub captured_memory: &'a BTreeMap<u32, SymbolicValue>,
    pub arguments_object: Option<&'a SymbolicValue>,
    pub receiver: &'a SymbolicValue,
    pub incoming_stack: &'a [Option<SymbolicValue>],
    pub trace_limits: TraceLimits,
}

/// Collect positional invocation arguments under the resource bound, rejecting
/// a foreign `Computed` node before any write. The iterator is stopped at
/// `limit + 1`, so an oversized or infinite iterator never runs away.
pub(super) fn collect_invocation_arguments<I, V>(
    args: I,
    limit: usize,
) -> Result<Vec<SymbolicValue>, ThreadStartupError>
where
    I: IntoIterator<Item = V>,
    V: Borrow<SymbolicValue>,
{
    let mut provided = Vec::new();
    for value in args.into_iter().take(limit.saturating_add(1)) {
        provided.push(value.borrow().clone());
    }
    if provided.len() > limit {
        return Err(ThreadStartupError::TooManyArguments {
            provided: provided.len(),
            limit,
        });
    }
    // Validate every provided argument before any write, including extras and
    // values a later duplicate slot would overwrite. A caller-supplied
    // `Computed` node can only belong to a foreign arena at this point.
    if provided
        .iter()
        .any(|value| matches!(value, SymbolicValue::Computed { .. }))
    {
        return Err(ThreadStartupError::ComputedInput {
            what: "positional argument",
        });
    }
    Ok(provided)
}

/// Build a fresh incoming context in the canonical order: stack snapshot,
/// captured values, optional aggregate/self writes, `initialSlots`, then the
/// positional arguments. A rejected request writes nothing observable because
/// the context is returned only when it is fully built.
pub(super) fn build_context_from_spec(
    spec: &ThreadContextSpec<'_>,
    provided: &[SymbolicValue],
) -> Result<TraceContext, ThreadStartupError> {
    let limits = spec.trace_limits;
    let mut context = TraceContext::new();
    context.set_receiver(spec.receiver.clone())?;

    // Explicit incoming stack snapshot. An absent snapshot leaves the
    // stack empty; `Some(Undefined)` is an explicit value, not a hole.
    if !spec.incoming_stack.is_empty() {
        context.set_stack_length_bounded(spec.incoming_stack.len(), limits.max_stack_length)?;
        for (index, cell) in spec.incoming_stack.iter().enumerate() {
            if let Some(value) = cell {
                context.set_stack_value_bounded(index, value.clone(), limits.max_stack_length)?;
            }
        }
    }

    // Captured-memory snapshot (the per-invocation clone of step 3).
    for (slot, value) in spec.captured_memory {
        context.set_memory_value_bounded(*slot, value.clone(), limits.max_memory_slots)?;
    }

    // Optional aggregate `arguments` object write.
    if let Some(slot) = spec.descriptor.arguments_slot {
        let aggregate = spec
            .arguments_object
            .cloned()
            .expect("arguments object validated when the thread was constructed");
        context.initialize_memory_slot_bounded(slot, limits.max_memory_slots)?;
        context.set_memory_value_bounded(slot, aggregate, limits.max_memory_slots)?;
    }

    // Initialize local slots, replacing any captured/aggregate cell.
    for slot in &spec.descriptor.initial_slots {
        context.initialize_memory_slot_bounded(*slot, limits.max_memory_slots)?;
    }

    // Positional arguments, then `undefined` for every missing slot. Every
    // target must already own a cell; `writeSlot` updates an existing cell and
    // never allocates one.
    for (index, slot) in spec.descriptor.argument_slots.iter().enumerate() {
        if !context.contains_memory_slot(*slot) {
            return Err(ThreadStartupError::MissingArgumentSlot { slot: *slot });
        }
        let value = match provided.get(index) {
            Some(value) => value.clone(),
            None => SymbolicValue::Undefined,
        };
        context.set_memory_value_bounded(*slot, value, limits.max_memory_slots)?;
    }

    Ok(context)
}

pub(super) fn validate_descriptor(
    descriptor: &ThreadDescriptor,
    inputs: &ThreadInputs,
) -> Result<(), ThreadStartupError> {
    if descriptor.capture_policy != CapturePolicy::Snapshot {
        return Err(ThreadStartupError::UnsupportedSharedCapture);
    }

    let limits = inputs.limits;
    for (what, count) in [
        ("initial slot list", descriptor.initial_slots.len()),
        ("captured slot list", descriptor.captured_slots.len()),
        ("argument slot list", descriptor.argument_slots.len()),
        ("incoming capture map", inputs.captured_memory.len()),
    ] {
        if count > limits.max_memory_slots {
            return Err(ThreadStartupError::TooManySlots {
                what,
                count,
                limit: limits.max_memory_slots,
            });
        }
    }
    if inputs.incoming_stack.len() > limits.max_stack_length {
        return Err(ThreadStartupError::IncomingStackTooLong {
            length: inputs.incoming_stack.len(),
            limit: limits.max_stack_length,
        });
    }

    match descriptor.arguments_slot {
        Some(slot) if inputs.arguments_object.is_none() => {
            return Err(ThreadStartupError::MissingArgumentsObject { slot })
        }
        None if inputs.arguments_object.is_some() => {
            return Err(ThreadStartupError::UnexpectedArgumentsObject)
        }
        _ => {}
    }
    match descriptor.result_slot {
        Some(slot) if inputs.self_value.is_none() => {
            return Err(ThreadStartupError::MissingSelfValue { slot })
        }
        None if inputs.self_value.is_some() => return Err(ThreadStartupError::UnexpectedSelfValue),
        _ => {}
    }

    // Exactly one explicit incoming value for each captured slot; no extras.
    let mut declared = BTreeMap::new();
    for slot in &descriptor.captured_slots {
        if declared.insert(*slot, ()).is_some() {
            return Err(ThreadStartupError::DuplicateCapturedSlot { slot: *slot });
        }
    }
    for slot in &descriptor.captured_slots {
        if !inputs.captured_memory.contains_key(slot) {
            return Err(ThreadStartupError::MissingCapture { slot: *slot });
        }
    }
    for slot in inputs.captured_memory.keys() {
        if !declared.contains_key(slot) {
            return Err(ThreadStartupError::UnexpectedCapture { slot: *slot });
        }
    }

    // Cells that exist before the positional writes: captured, result,
    // aggregate arguments, and initialized slots. `writeSlot` updates an
    // existing cell, so every positional target must be one of these.
    let mut existing = std::collections::BTreeSet::new();
    existing.extend(descriptor.captured_slots.iter().copied());
    existing.extend(descriptor.initial_slots.iter().copied());
    existing.extend(descriptor.arguments_slot);
    existing.extend(descriptor.result_slot);
    for slot in &descriptor.argument_slots {
        if !existing.contains(slot) {
            return Err(ThreadStartupError::MissingArgumentSlot { slot: *slot });
        }
    }
    if existing.len() > limits.max_memory_slots {
        return Err(ThreadStartupError::TooManySlots {
            what: "distinct thread memory slots",
            count: existing.len(),
            limit: limits.max_memory_slots,
        });
    }

    for (what, value) in inputs
        .captured_memory
        .values()
        .map(|value| ("captured memory value", value))
        .chain(
            inputs
                .arguments_object
                .iter()
                .map(|value| ("arguments object", value)),
        )
        .chain(inputs.self_value.iter().map(|value| ("self value", value)))
        .chain(inputs.receiver.iter().map(|value| ("receiver", value)))
        .chain(
            inputs
                .incoming_stack
                .iter()
                .filter_map(|cell| cell.as_ref())
                .map(|value| ("incoming stack value", value)),
        )
    {
        if matches!(value, SymbolicValue::Computed { .. }) {
            return Err(ThreadStartupError::ComputedInput { what });
        }
    }

    // The aggregate `arguments` object and self/function value are opaque
    // object-like inputs, never primitives pretending to be one.
    if let Some(value) = &inputs.arguments_object {
        if !is_opaque_object(value) {
            return Err(ThreadStartupError::InvalidArgumentsObject);
        }
    }
    if let Some(value) = &inputs.self_value {
        if !is_opaque_object(value) {
            return Err(ThreadStartupError::InvalidSelfValue);
        }
    }

    Ok(())
}

fn is_opaque_object(value: &SymbolicValue) -> bool {
    matches!(
        value,
        SymbolicValue::Object { .. } | SymbolicValue::External { .. }
    )
}

pub(super) fn build_captured_memory(
    descriptor: &ThreadDescriptor,
    inputs: &ThreadInputs,
) -> Result<BTreeMap<u32, SymbolicValue>, ThreadStartupError> {
    let mut memory = BTreeMap::new();
    for slot in &descriptor.captured_slots {
        // `validate_descriptor` proved presence and uniqueness.
        let value = inputs
            .captured_memory
            .get(slot)
            .cloned()
            .ok_or(ThreadStartupError::MissingCapture { slot: *slot })?;
        memory.insert(*slot, value);
    }
    if let Some(slot) = descriptor.result_slot {
        let value = inputs
            .self_value
            .clone()
            .ok_or(ThreadStartupError::MissingSelfValue { slot })?;
        // `initializeSlot(resultSlot); writeSlot(resultSlot, threadInvocation)`.
        memory.insert(slot, value);
    }
    Ok(memory)
}
