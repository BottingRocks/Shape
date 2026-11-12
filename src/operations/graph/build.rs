use super::*;

/// Builds the occurrence/effect/unknown state while the shared recipe walk runs.
///
/// The builder is deliberately the same walk used for the legacy steps: there
/// is no second parser or recognizer pipeline.
#[derive(Debug, Default)]
pub(crate) struct GraphBuilder {
    occurrences: Vec<Occurrence>,
    effects: Vec<Effect>,
    unknown_regions: Vec<UnknownRegion>,
    versions: BTreeMap<u32, OccurrenceId>,
    stack_writes: BTreeMap<LocationKey, OccurrenceId>,
    slot_writes: BTreeMap<LocationKey, OccurrenceId>,
}

impl GraphBuilder {
    fn push_value(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        op: OccurrenceOp,
    ) -> OccurrenceId {
        let id = self.occurrences.len() as OccurrenceId;
        self.occurrences.push(Occurrence {
            id,
            op,
            span: span.clone(),
            granularity,
            step_order: None,
            effect: None,
            source: None,
            provenance: Provenance {
                source_hash: String::new(),
                raw_input_hash: String::new(),
                source_length: 0,
                table_declaration_ordinal: 0,
                table_element_count: 0,
                table_function_count: 0,
                table_key: 0,
                handler_index: 0,
                span,
            },
        });
        id
    }

    fn push_effect(
        &mut self,
        occurrence: OccurrenceId,
        kind: EffectKind,
        may_throw: bool,
        observed_write: Option<OccurrenceId>,
        location: Option<LocationKey>,
    ) {
        let id = self.effects.len() as EffectId;
        self.effects.push(Effect {
            id,
            occurrence,
            kind,
            may_throw,
            observed_write,
            location,
        });
        self.occurrences[occurrence as usize].effect = Some(id);
    }

    pub(crate) fn context(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
    ) -> OccurrenceId {
        self.push_value(span, granularity, OccurrenceOp::Context)
    }

    pub(crate) fn operand(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        position: u32,
    ) -> OccurrenceId {
        self.push_value(span, granularity, OccurrenceOp::Operand { position })
    }

    pub(crate) fn literal(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        value: LiteralValue,
    ) -> OccurrenceId {
        self.push_value(span, granularity, OccurrenceOp::Literal(value))
    }

    pub(crate) fn stack_length(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
    ) -> OccurrenceId {
        let id = self.push_value(span, granularity, OccurrenceOp::StackLength);
        self.push_effect(
            id,
            EffectKind::StackLengthRead,
            false,
            None,
            Some(LocationKey::StackLength),
        );
        id
    }

    pub(crate) fn stack_read(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        index: OccurrenceId,
    ) -> OccurrenceId {
        let location = self.location_of(index);
        let observed = location
            .as_ref()
            .and_then(|key| latest_write_for(&self.stack_writes, key));
        let id = self.push_value(span, granularity, OccurrenceOp::StackRead { index });
        self.push_effect(id, EffectKind::StackRead, false, observed, location);
        id
    }

    pub(crate) fn memory_read(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        slot: OccurrenceId,
    ) -> OccurrenceId {
        let location = self.location_of(slot);
        let observed = location
            .as_ref()
            .and_then(|key| latest_write_for(&self.slot_writes, key));
        let id = self.push_value(span, granularity, OccurrenceOp::MemoryRead { slot });
        self.push_effect(id, EffectKind::MemoryRead, true, observed, location);
        id
    }

    pub(crate) fn temporary_ref(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        temporary: u32,
    ) -> OccurrenceId {
        let version = self.versions.get(&temporary).copied().unwrap_or(0);
        self.push_value(
            span,
            granularity,
            OccurrenceOp::TemporaryRef { temporary, version },
        )
    }

    pub(crate) fn binary(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        operator: PrimitiveOperator,
        left: OccurrenceId,
        right: OccurrenceId,
    ) -> OccurrenceId {
        self.push_value(
            span,
            granularity,
            OccurrenceOp::Binary {
                operator,
                left,
                right,
            },
        )
    }

    pub(crate) fn unary(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        operator: PrimitiveOperator,
        value: OccurrenceId,
    ) -> OccurrenceId {
        self.push_value(span, granularity, OccurrenceOp::Unary { operator, value })
    }

    pub(crate) fn unknown_value(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        reason: &str,
    ) -> OccurrenceId {
        self.push_value(
            span,
            granularity,
            OccurrenceOp::UnknownValue {
                reason: reason.into(),
            },
        )
    }

    /// Record one operation occurrence and its effect, returning its id.  The
    /// legacy step order is assigned here so the projection is exact.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn operation(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        step_order: u32,
        operation: &RecipeOperation,
        inputs: &[OccurrenceId],
        region: Option<UnknownRegionKind>,
        source: Option<String>,
    ) -> OccurrenceId {
        let op = match operation {
            RecipeOperation::Let { temporary, .. } => OccurrenceOp::Bind {
                temporary: *temporary,
                value: inputs[0],
            },
            RecipeOperation::Assign { temporary, .. } => OccurrenceOp::Assign {
                temporary: *temporary,
                value: inputs[0],
            },
            RecipeOperation::StackWrite { .. } => OccurrenceOp::StackWrite {
                index: inputs[0],
                value: inputs[1],
            },
            RecipeOperation::StackLengthSet { .. } => {
                OccurrenceOp::StackLengthSet { value: inputs[0] }
            }
            RecipeOperation::MemoryWrite { .. } => OccurrenceOp::MemoryWrite {
                slot: inputs[0],
                value: inputs[1],
            },
            RecipeOperation::MemoryInitialize { .. } => {
                OccurrenceOp::MemoryInitialize { slot: inputs[0] }
            }
            RecipeOperation::PropertyWrite { .. } => OccurrenceOp::PropertyWrite {
                object: inputs[0],
                key: inputs[1],
                value: inputs[2],
            },
            RecipeOperation::PushTryFrame { .. } => OccurrenceOp::PushTryFrame {
                byte_offset: inputs[0],
                dispatch_state: inputs[1],
            },
            RecipeOperation::PopTryFrame
            | RecipeOperation::PushFinallyRecord
            | RecipeOperation::PopErrorRecord
            | RecipeOperation::RethrowIfHandled
            | RecipeOperation::DropThrownIfHandled
            | RecipeOperation::DropThrownUnderIfHandled => OccurrenceOp::ExceptionStep {
                operation: operation.clone(),
            },
            RecipeOperation::PushCallFrame => OccurrenceOp::PushCallFrame,
            RecipeOperation::PopCallFrame => OccurrenceOp::PopCallFrame,
            RecipeOperation::SetProgramCounter { .. } => {
                OccurrenceOp::SetProgramCounter { value: inputs[0] }
            }
            RecipeOperation::SetDispatchState { .. } => {
                OccurrenceOp::SetDispatchState { value: inputs[0] }
            }
            RecipeOperation::CompleteThread { .. } => {
                OccurrenceOp::CompleteThread { value: inputs[0] }
            }
            RecipeOperation::JumpIf { .. } => OccurrenceOp::JumpIf {
                condition: inputs[0],
                byte_offset: inputs[1],
                dispatch_state: inputs[2],
            },
            RecipeOperation::Evaluate { .. } => OccurrenceOp::Evaluate { value: inputs[0] },
            RecipeOperation::Return { .. } => OccurrenceOp::Return { value: inputs[0] },
            RecipeOperation::Throw { .. } => OccurrenceOp::Throw { value: inputs[0] },
            RecipeOperation::Unknown { reason, .. } => OccurrenceOp::Unknown {
                reason: reason.clone(),
            },
        };
        let id = self.occurrences.len() as OccurrenceId;
        self.occurrences.push(Occurrence {
            id,
            op: op.clone(),
            span: span.clone(),
            granularity,
            step_order: Some(step_order),
            effect: None,
            source: source.clone(),
            provenance: Provenance {
                source_hash: String::new(),
                raw_input_hash: String::new(),
                source_length: 0,
                table_declaration_ordinal: 0,
                table_element_count: 0,
                table_function_count: 0,
                table_key: 0,
                handler_index: 0,
                span,
            },
        });
        if let Some(temporary) = op.defines() {
            self.versions.insert(temporary, id);
        }
        match &op {
            OccurrenceOp::StackWrite { index, .. } => {
                let location = self.location_of(*index);
                self.push_effect(id, EffectKind::StackWrite, false, None, location.clone());
                record_stack_write(&mut self.stack_writes, location, id);
            }
            OccurrenceOp::StackLengthSet { .. } => {
                self.push_effect(
                    id,
                    EffectKind::StackLengthWrite,
                    false,
                    None,
                    Some(LocationKey::StackLength),
                );
                self.stack_writes.clear();
            }
            OccurrenceOp::MemoryWrite { slot, .. } => {
                let location = self.location_of(*slot);
                self.push_effect(id, EffectKind::MemoryWrite, false, None, location.clone());
                record_slot_write(&mut self.slot_writes, location, id);
            }
            OccurrenceOp::MemoryInitialize { slot } => {
                let location = self.location_of(*slot);
                self.push_effect(
                    id,
                    EffectKind::MemoryInitialize,
                    false,
                    None,
                    location.clone(),
                );
                record_slot_write(&mut self.slot_writes, location, id);
            }
            OccurrenceOp::PropertyWrite { .. } => {
                // A write to a non-VM object cannot alias the VM stack or
                // memory slots, so read-from witnesses are retained.
                self.push_effect(id, EffectKind::PropertyWrite, false, None, None);
            }
            // VM control state is separate from the stack and memory slots, so
            // read-from witnesses are retained.
            OccurrenceOp::PushTryFrame { .. }
            | OccurrenceOp::ExceptionStep { .. }
            | OccurrenceOp::ExceptionRead { .. } => {
                // VM exception state is separate from the stack and memory.
                self.push_effect(id, EffectKind::ControlTransfer, false, None, None);
            }
            OccurrenceOp::PushCallFrame => {
                self.push_effect(id, EffectKind::CallFramePush, false, None, None);
            }
            OccurrenceOp::PopCallFrame => {
                self.push_effect(id, EffectKind::CallFramePop, false, None, None);
            }
            OccurrenceOp::SetProgramCounter { .. }
            | OccurrenceOp::SetDispatchState { .. }
            | OccurrenceOp::JumpIf { .. } => {
                self.push_effect(id, EffectKind::ControlTransfer, false, None, None);
            }
            OccurrenceOp::Evaluate { .. } => {
                self.push_effect(id, EffectKind::Evaluate, false, None, None);
            }
            OccurrenceOp::Return { .. } | OccurrenceOp::CompleteThread { .. } => {
                self.push_effect(id, EffectKind::Return, false, None, None);
            }
            OccurrenceOp::Throw { .. } => {
                self.push_effect(id, EffectKind::Throw, false, None, None);
            }
            OccurrenceOp::Unknown { .. } => {
                self.push_effect(id, EffectKind::Unknown, false, None, None);
                // A call or other unknown region may mutate any stack or
                // memory location.  Drop every read-from witness rather than
                // keeping a stale pre-call observation.
                self.stack_writes.clear();
                self.slot_writes.clear();
                self.unknown_regions.push(UnknownRegion {
                    occurrence: id,
                    kind: region.unwrap_or(UnknownRegionKind::Other),
                    span: self.occurrences[id as usize].span.clone(),
                    reason: match &self.occurrences[id as usize].op {
                        OccurrenceOp::Unknown { reason } => reason.clone(),
                        _ => unreachable!(),
                    },
                    source: self.occurrences[id as usize]
                        .source
                        .clone()
                        .unwrap_or_default(),
                });
            }
            _ => {}
        }
        id
    }

    pub(crate) fn popped_call_frame(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        field: CallFrameField,
    ) -> OccurrenceId {
        self.push_value(span, granularity, OccurrenceOp::PoppedCallFrame { field })
    }

    pub(crate) fn receiver(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        field: ReceiverField,
    ) -> OccurrenceId {
        self.push_value(span, granularity, OccurrenceOp::Receiver { field })
    }

    pub(crate) fn exception_field(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        value: SemanticValue,
    ) -> OccurrenceId {
        let id = self.push_value(span, granularity, OccurrenceOp::ExceptionRead { value });
        self.push_effect(id, EffectKind::ControlTransfer, false, None, None);
        id
    }

    pub(crate) fn function_literal(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        source: String,
    ) -> OccurrenceId {
        self.push_value(span, granularity, OccurrenceOp::FunctionLiteral { source })
    }

    pub(crate) fn stack_slice(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        start: OccurrenceId,
    ) -> OccurrenceId {
        let id = self.push_value(span, granularity, OccurrenceOp::StackSlice { start });
        self.push_effect(id, EffectKind::StackRangeRead, false, None, None);
        id
    }

    pub(crate) fn call(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        callee: OccurrenceId,
        this_value: OccurrenceId,
        arguments: OccurrenceId,
    ) -> OccurrenceId {
        let id = self.push_value(
            span,
            granularity,
            OccurrenceOp::Call {
                callee,
                this_value,
                arguments,
            },
        );
        self.push_effect(id, EffectKind::ProgramCall, true, None, None);
        id
    }

    pub(crate) fn construct(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        callee: OccurrenceId,
        arguments: OccurrenceId,
    ) -> OccurrenceId {
        let id = self.push_value(
            span,
            granularity,
            OccurrenceOp::Construct { callee, arguments },
        );
        self.push_effect(id, EffectKind::ProgramCall, true, None, None);
        id
    }

    pub(crate) fn delete(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        object: OccurrenceId,
        key: OccurrenceId,
    ) -> OccurrenceId {
        let id = self.push_value(span, granularity, OccurrenceOp::Delete { object, key });
        self.push_effect(id, EffectKind::ProgramCall, true, None, None);
        id
    }

    pub(crate) fn create_thread(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        descriptor: OccurrenceId,
        byte_offset: OccurrenceId,
        dispatch_state: OccurrenceId,
    ) -> OccurrenceId {
        let id = self.push_value(
            span,
            granularity,
            OccurrenceOp::CreateThread {
                descriptor,
                byte_offset,
                dispatch_state,
            },
        );
        self.push_effect(id, EffectKind::ThreadCapture, false, None, None);
        id
    }

    pub(crate) fn binding(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        name: String,
    ) -> OccurrenceId {
        self.push_value(span, granularity, OccurrenceOp::Binding { name })
    }

    pub(crate) fn property_read(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        object: OccurrenceId,
        key: OccurrenceId,
    ) -> OccurrenceId {
        self.push_value(
            span,
            granularity,
            OccurrenceOp::PropertyRead { object, key },
        )
    }

    pub(crate) fn object_literal(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        keys: Vec<String>,
        values: Vec<OccurrenceId>,
    ) -> OccurrenceId {
        self.push_value(
            span,
            granularity,
            OccurrenceOp::ObjectLiteral { keys, values },
        )
    }

    pub(crate) fn array_literal(
        &mut self,
        span: SpanRecord,
        granularity: SpanGranularity,
        elements: Vec<OccurrenceId>,
    ) -> OccurrenceId {
        self.push_value(span, granularity, OccurrenceOp::ArrayLiteral { elements })
    }

    pub(crate) fn value_version(&self, temporary: u32) -> Option<OccurrenceId> {
        self.versions.get(&temporary).copied()
    }

    pub(crate) fn next_step_order(&self) -> u32 {
        self.occurrences
            .iter()
            .filter(|occurrence| occurrence.op.is_operation())
            .count() as u32
    }

    fn location_of(&self, id: OccurrenceId) -> Option<LocationKey> {
        match &self.occurrences.get(id as usize)?.op {
            OccurrenceOp::Literal(LiteralValue::DecodedNumber { bits }) => {
                Some(LocationKey::NumberBits(*bits))
            }
            OccurrenceOp::Literal(LiteralValue::String(value)) => {
                Some(LocationKey::String(value.clone()))
            }
            OccurrenceOp::Literal(LiteralValue::Boolean(value)) => {
                Some(LocationKey::Boolean(*value))
            }
            OccurrenceOp::Literal(LiteralValue::Null) => Some(LocationKey::Null),
            OccurrenceOp::Literal(LiteralValue::Undefined) => Some(LocationKey::Undefined),
            _ => None,
        }
    }

    /// Attach provenance/identity and produce the finished handler IR.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish(
        mut self,
        source_hash: &str,
        raw_input_hash: &str,
        table_declaration_ordinal: u32,
        table_element_count: u32,
        table_function_count: u32,
        table_key: u32,
        handler_index: u32,
        handler_span: SpanRecord,
        source: &Arc<str>,
    ) -> HandlerIR {
        for occurrence in &mut self.occurrences {
            occurrence.provenance.source_hash = source_hash.into();
            occurrence.provenance.raw_input_hash = raw_input_hash.into();
            occurrence.provenance.source_length = source.len();
            occurrence.provenance.table_declaration_ordinal = table_declaration_ordinal;
            occurrence.provenance.table_element_count = table_element_count;
            occurrence.provenance.table_function_count = table_function_count;
            occurrence.provenance.table_key = table_key;
            occurrence.provenance.handler_index = handler_index;
        }
        let mut graph = HandlerIR {
            schema_version: GRAPH_SCHEMA_VERSION.into(),
            primitive_version: PRIMITIVE_REGISTRY_VERSION.into(),
            source_hash: source_hash.into(),
            raw_input_hash: raw_input_hash.into(),
            source_length: source.len(),
            source: Some(Arc::clone(source)),
            table_declaration_ordinal,
            table_element_count,
            table_function_count,
            table_key,
            handler_index,
            handler_span,
            status: TemplateStatus::Unsupported,
            occurrences: self.occurrences,
            effects: self.effects,
            unknown_regions: self.unknown_regions,
        };
        graph.status = graph.derived_status();
        graph
    }
}

pub(super) fn render_literal(value: &LiteralValue) -> String {
    match value {
        LiteralValue::Number(text) => format!("lit({text})"),
        LiteralValue::DecodedNumber { bits } => format!("lit({bits:#018x})"),
        LiteralValue::String(value) => format!("str({value:?})"),
        LiteralValue::Boolean(value) => format!("bool({value})"),
        LiteralValue::Null => "null".to_string(),
        LiteralValue::Undefined => "undefined".to_string(),
    }
}

/// Runtime array/memory keys are property keys, not Rust enum variants:
/// `0`, `"0"`, `false`, `+0` and `-0` can name the same location in the
/// JavaScript semantics being described.  Compare the coerced property key
/// instead of relying on `LocationKey` inequality.
pub(super) fn literal_property_key(location: &LocationKey) -> Option<String> {
    match location {
        LocationKey::NumberBits(bits) => Some(f64::from_bits(*bits).to_js_string()),
        LocationKey::String(value) => Some(value.clone()),
        LocationKey::Boolean(value) => Some(value.to_string()),
        LocationKey::Null => Some("null".into()),
        LocationKey::Undefined => Some("undefined".into()),
        LocationKey::Context
        | LocationKey::StackLength
        | LocationKey::Operand(_)
        | LocationKey::Temporary { .. } => None,
    }
}

pub(super) fn locations_may_alias(left: &LocationKey, right: &LocationKey) -> bool {
    if left == right {
        return true;
    }
    if matches!(left, LocationKey::StackLength) || matches!(right, LocationKey::StackLength) {
        // `array.length` is another way to spell the `"length"` property key;
        // a write to it can grow or truncate the stack.
        let other = if matches!(left, LocationKey::StackLength) {
            right
        } else {
            left
        };
        return literal_property_key(other).as_deref() == Some("length");
    }
    match (literal_property_key(left), literal_property_key(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

pub(super) fn latest_write_for(
    writes: &BTreeMap<LocationKey, OccurrenceId>,
    location: &LocationKey,
) -> Option<OccurrenceId> {
    writes
        .iter()
        .find_map(|(key, occurrence)| locations_may_alias(key, location).then_some(*occurrence))
}

pub(super) fn locations_match(left: Option<LocationKey>, right: Option<LocationKey>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => locations_may_alias(&left, &right),
        (None, None) => true,
        _ => false,
    }
}

/// Keep only writes that a later read may still safely observe.  Dynamic index
/// expressions (`None`) and writes to `"length"` clear the whole stack
/// namespace; writes to a known key clear every key that may alias it.
pub(super) fn record_stack_write(
    writes: &mut BTreeMap<LocationKey, OccurrenceId>,
    location: Option<LocationKey>,
    occurrence: OccurrenceId,
) {
    match location {
        Some(location) if literal_property_key(&location).as_deref() == Some("length") => {
            writes.clear();
        }
        Some(location) => {
            writes.retain(|existing, _| !locations_may_alias(existing, &location));
            writes.insert(location, occurrence);
        }
        None => writes.clear(),
    }
}

/// Memory slots are keyed by number at the runtime boundary, but the same
/// conservative property-key aliasing applies to a non-executable graph.
pub(super) fn record_slot_write(
    writes: &mut BTreeMap<LocationKey, OccurrenceId>,
    location: Option<LocationKey>,
    occurrence: OccurrenceId,
) {
    match location {
        Some(location) => {
            writes.retain(|existing, _| !locations_may_alias(existing, &location));
            writes.insert(location, occurrence);
        }
        None => writes.clear(),
    }
}

pub(super) fn render_location(location: &LocationKey) -> String {
    match location {
        LocationKey::NumberBits(bits) => format!("{bits:#018x}"),
        LocationKey::String(value) => format!("{value:?}"),
        LocationKey::Boolean(value) => value.to_string(),
        LocationKey::Null => "null".to_string(),
        LocationKey::Undefined => "undefined".to_string(),
        LocationKey::Context => "context".to_string(),
        LocationKey::StackLength => "stack.length".to_string(),
        LocationKey::Operand(position) => format!("operand#{position}"),
        LocationKey::Temporary { temporary, version } => format!("temp#{temporary}@{version}"),
    }
}

pub(super) fn operation_signature(
    op: &OccurrenceOp,
    temporary_names: &mut BTreeMap<u32, u32>,
    next_temporary: &mut u32,
) -> String {
    let temporary = |id: u32, names: &mut BTreeMap<u32, u32>, next: &mut u32| -> u32 {
        *names.entry(id).or_insert_with(|| {
            let alias = *next;
            *next += 1;
            alias
        })
    };
    match op {
        OccurrenceOp::Context => "context".into(),
        OccurrenceOp::Operand { position } => format!("operand#{position}"),
        OccurrenceOp::Literal(value) => format!("literal:{}", render_literal(value)),
        OccurrenceOp::StackLength => "stack.length".into(),
        OccurrenceOp::StackRead { .. } => "stack.read".into(),
        OccurrenceOp::MemoryRead { .. } => "memory.readSlot".into(),
        OccurrenceOp::TemporaryRef { temporary: id, .. } => {
            format!(
                "temporary#{}",
                temporary(*id, temporary_names, next_temporary)
            )
        }
        OccurrenceOp::Binary { operator, .. } => format!("binary:{operator:?}"),
        OccurrenceOp::Unary { operator, .. } => format!("unary:{operator:?}"),
        OccurrenceOp::UnknownValue { reason } => format!("unknown-value:{reason}"),
        OccurrenceOp::Binding { name } => format!("binding:{name}"),
        OccurrenceOp::PropertyRead { .. } => "property.read".into(),
        OccurrenceOp::ObjectLiteral { keys, .. } => format!("object.new:{}", keys.join(",")),
        OccurrenceOp::ArrayLiteral { elements } => format!("array.new:{}", elements.len()),
        OccurrenceOp::PropertyWrite { .. } => "property.write".into(),
        OccurrenceOp::PoppedCallFrame { field } => format!("call_frame.{}", field.as_str()),
        OccurrenceOp::Receiver { field } => format!("receiver.{}", field.as_str()),
        OccurrenceOp::FunctionLiteral { .. } => "function.new".into(),
        OccurrenceOp::Construct { .. } => "function.construct".into(),
        OccurrenceOp::Delete { .. } => "property.delete".into(),
        OccurrenceOp::StackSlice { .. } => "stack.slice".into(),
        OccurrenceOp::Call { .. } => "function.call".into(),
        OccurrenceOp::CreateThread { .. } => "thread.create".into(),
        OccurrenceOp::ExceptionRead { value } => format!("exception.read:{value:?}"),
        OccurrenceOp::PushTryFrame { .. } => "try_frame.push".into(),
        OccurrenceOp::ExceptionStep { operation } => format!("exception.step:{operation:?}"),
        OccurrenceOp::PushCallFrame => "call_frame.push".into(),
        OccurrenceOp::PopCallFrame => "call_frame.pop".into(),
        OccurrenceOp::SetProgramCounter { .. } => "byte_offset.write".into(),
        OccurrenceOp::SetDispatchState { .. } => "dispatch_state.write".into(),
        OccurrenceOp::JumpIf { .. } => "jump_if".into(),
        OccurrenceOp::CompleteThread { .. } => "thread.return".into(),
        OccurrenceOp::Bind { temporary: id, .. } => format!(
            "let:temporary#{}",
            temporary(*id, temporary_names, next_temporary)
        ),
        OccurrenceOp::Assign { temporary: id, .. } => format!(
            "assign:temporary#{}",
            temporary(*id, temporary_names, next_temporary)
        ),
        OccurrenceOp::StackWrite { .. } => "stack.write".into(),
        OccurrenceOp::StackLengthSet { .. } => "stack.length.write".into(),
        OccurrenceOp::MemoryWrite { .. } => "memory.writeSlot".into(),
        OccurrenceOp::MemoryInitialize { .. } => "memory.initializeSlot".into(),
        OccurrenceOp::Evaluate { .. } => "evaluate".into(),
        OccurrenceOp::Return { .. } => "return".into(),
        OccurrenceOp::Throw { .. } => "throw".into(),
        OccurrenceOp::Unknown { reason } => format!("unknown:{reason}"),
    }
}

/// SHA-256 identity for standardized source bytes. This is intentionally
/// distinct from the pipeline's legacy raw-input hash.
pub(crate) fn semantic_source_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn binary_name(operator: PrimitiveOperator) -> &'static str {
    match operator {
        PrimitiveOperator::JsAdd => "js.add",
        PrimitiveOperator::Subtract => "js.subtract",
        PrimitiveOperator::Multiply => "js.multiply",
        PrimitiveOperator::Divide => "js.divide",
        PrimitiveOperator::Remainder => "js.remainder",
        PrimitiveOperator::ShiftLeft => "js.shift_left",
        PrimitiveOperator::ShiftRight => "js.shift_right",
        PrimitiveOperator::ShiftRightZeroFill => "js.shift_right_zero_fill",
        PrimitiveOperator::BitwiseOr => "js.bitwise_or",
        PrimitiveOperator::BitwiseXor => "js.bitwise_xor",
        PrimitiveOperator::BitwiseAnd => "js.bitwise_and",
        PrimitiveOperator::Exponent => "js.exponent",
        PrimitiveOperator::Equal => "js.equal",
        PrimitiveOperator::NotEqual => "js.not_equal",
        PrimitiveOperator::StrictEqual => "js.strict_equal",
        PrimitiveOperator::StrictNotEqual => "js.strict_not_equal",
        PrimitiveOperator::LessThan => "js.less_than",
        PrimitiveOperator::LessThanOrEqual => "js.less_than_or_equal",
        PrimitiveOperator::GreaterThan => "js.greater_than",
        PrimitiveOperator::GreaterThanOrEqual => "js.greater_than_or_equal",
        PrimitiveOperator::In => "js.in",
        PrimitiveOperator::InstanceOf => "js.instanceof",
        PrimitiveOperator::UnaryPlus
        | PrimitiveOperator::UnaryMinus
        | PrimitiveOperator::BitwiseNot
        | PrimitiveOperator::LogicalNot
        | PrimitiveOperator::Void
        | PrimitiveOperator::Typeof => "js.binary",
    }
}

pub(super) fn unary_name(operator: PrimitiveOperator) -> &'static str {
    match operator {
        PrimitiveOperator::UnaryPlus => "js.unary_plus",
        PrimitiveOperator::UnaryMinus => "js.unary_minus",
        PrimitiveOperator::BitwiseNot => "js.bitwise_not",
        PrimitiveOperator::LogicalNot => "js.logical_not",
        PrimitiveOperator::Void => "js.void",
        PrimitiveOperator::Typeof => "js.typeof",
        _ => "js.unary",
    }
}
