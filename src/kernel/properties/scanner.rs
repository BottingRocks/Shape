use super::*;

#[derive(Clone)]
struct Binding {
    symbol: SymbolId,
    name: String,
    owner_function: Option<Span>,
    value: PropertyValue,
    decoder_callee: Option<SymbolId>,
    decoder_call_span: Option<Span>,
    alias_of: Option<SymbolId>,
    memory_constructor: Option<SymbolId>,
    object_methods: BTreeSet<String>,
    wasm_constructor: bool,
    operation_table: Option<(Vec<String>, Vec<String>, Vec<Span>)>,
}

#[derive(Clone)]
struct Assignment {
    owner: Span,
    property: String,
    value: PropertyValue,
}

#[derive(Clone)]
struct SlotCall {
    owner: Option<Span>,
    function: String,
    method: String,
    arguments: Vec<PropertyValue>,
    receiver: Option<SlotReceiver>,
}

#[derive(Clone)]
enum SlotReceiver {
    Binding(SymbolId),
    Prototype(SymbolId),
    MemoryField,
}

#[derive(Clone)]
struct DecoderCall {
    span: Span,
    owner: Option<Span>,
    callee: SymbolId,
    name: String,
    arguments: Vec<PropertyValue>,
    argument_text: Vec<String>,
}

#[derive(Clone)]
struct WasmImportBinding {
    namespace: Option<String>,
    property: Option<String>,
    binding: String,
}

#[derive(Clone)]
struct WorkerBootstrap {
    source_binding: String,
    owner: Option<Span>,
    entry_pc: PropertyValue,
    state: PropertyValue,
    descriptor_index: PropertyValue,
    initial_slots: PropertyValue,
}

#[derive(Clone)]
struct DefineProperty {
    owner: Option<Span>,
    receiver: String,
    constructor_symbol: Option<SymbolId>,
    property: String,
    value: PropertyValue,
}

struct EntryCandidate {
    function_span: Span,
    symbol: Option<SymbolId>,
    function_name: Option<String>,
    parameters: Vec<String>,
    arguments: Vec<PropertyValue>,
}

#[derive(Clone)]
struct FunctionInfo {
    name: String,
    span: Span,
    owner_function: Option<Span>,
    symbol: Option<SymbolId>,
    parameters: Vec<String>,
    /// First explicit `return` of a literal array, kept as a bounded value.
    /// Used only to read the stack constructor's literal initial array.
    return_value: Option<PropertyValue>,
}

pub(super) struct Scanner<'a> {
    source: &'a str,
    scoping: &'a Scoping,
    limits: ExtractionLimits,
    bindings: Vec<Binding>,
    by_symbol: BTreeMap<SymbolId, usize>,
    assignments: Vec<Assignment>,
    slot_calls: Vec<SlotCall>,
    decoder_calls: Vec<DecoderCall>,
    define_properties: Vec<DefineProperty>,
    prototype_aliases: BTreeMap<SymbolId, SymbolId>,
    function_stack: Vec<(String, Span)>,
    functions: Vec<FunctionInfo>,
    operation_symbols: BTreeSet<SymbolId>,
    wasm_module_symbols: BTreeSet<SymbolId>,
    wasm_import_bindings: BTreeMap<SymbolId, Vec<WasmImportBinding>>,
    wasm_module_inputs: BTreeMap<SymbolId, SymbolId>,
    references: BTreeMap<Span, BTreeSet<SymbolId>>,
    mutable_symbols: BTreeSet<SymbolId>,
    worker_bootstraps: Vec<WorkerBootstrap>,
    literal_budget: Cell<usize>,
    entry: Option<EntryCandidate>,
}

impl<'a> Scanner<'a> {
    pub(super) fn new(source: &'a str, scoping: &'a Scoping, limits: ExtractionLimits) -> Self {
        Self {
            source,
            scoping,
            limits,
            bindings: Vec::new(),
            by_symbol: BTreeMap::new(),
            assignments: Vec::new(),
            slot_calls: Vec::new(),
            decoder_calls: Vec::new(),
            define_properties: Vec::new(),
            prototype_aliases: BTreeMap::new(),
            function_stack: Vec::new(),
            functions: Vec::new(),
            operation_symbols: BTreeSet::new(),
            wasm_module_symbols: BTreeSet::new(),
            wasm_import_bindings: BTreeMap::new(),
            wasm_module_inputs: BTreeMap::new(),
            references: BTreeMap::new(),
            mutable_symbols: BTreeSet::new(),
            worker_bootstraps: Vec::new(),
            literal_budget: Cell::new(limits.max_literal_elements.saturating_mul(16)),
            entry: None,
        }
    }

    /// Finish only the source facts consumed by `KernelArtifact`.
    ///
    /// `finish` builds the full human-facing property projection, including
    /// re-proving decoders and projecting operations, strings, and slots. The
    /// artifact only needs the entry call, the entry-owned thread-descriptor
    /// table, and worker bootstrap records, so keep this path intentionally
    /// narrow.
    pub(super) fn finish_entry_facts(self) -> Result<ActualProperties, ActualPropertiesError> {
        let mut result = ActualProperties::default();
        let entry_span = self.entry.as_ref().map(|entry| entry.function_span);

        if let Some(entry) = self.entry.as_ref() {
            let arguments = entry.arguments.clone();
            if let Some(PropertyValue::Array(indices)) = arguments.get(3) {
                result.slots.initial_special_indices = indices.clone();
            }
            result.entry = Some(EntryProperties {
                function_name: entry.function_name.clone(),
                parameters: entry.parameters.clone(),
                initial_memory_slots: arguments.get(3).cloned(),
                arguments,
            });
        }

        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.owner_function == entry_span)
        {
            if is_thread_descriptors(&binding.value) {
                add_named(
                    &mut result.thread_descriptors,
                    &binding.name,
                    binding.value.clone(),
                );
            }
        }

        let owned_spans = entry_span.map(|entry| {
            let mut spans = BTreeSet::from([entry]);
            let mut changed = true;
            while changed {
                changed = false;
                for function in &self.functions {
                    if function
                        .owner_function
                        .is_some_and(|owner| spans.contains(&owner))
                        && spans.insert(function.span)
                    {
                        changed = true;
                    }
                }
            }
            spans
        });
        let owns = |owner: Option<Span>| match &owned_spans {
            Some(spans) => owner.is_some_and(|span| spans.contains(&span)),
            None => owner.is_none(),
        };
        for worker in &self.worker_bootstraps {
            if !owns(worker.owner) {
                continue;
            }
            let mut fields = BTreeMap::new();
            fields.insert("entryPc".into(), worker.entry_pc.clone());
            fields.insert("state".into(), worker.state.clone());
            fields.insert("descriptorIndex".into(), worker.descriptor_index.clone());
            fields.insert("initialSlots".into(), worker.initial_slots.clone());
            add_named(
                &mut result.additional,
                &worker.source_binding,
                PropertyValue::Object(fields),
            );
        }

        Ok(result)
    }

    pub(super) fn finish_operation_tables(
        self,
    ) -> Result<Vec<OperationSet>, ActualPropertiesError> {
        let entry_span = self.entry.as_ref().map(|entry| entry.function_span);
        let mut operations = Vec::new();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.owner_function == entry_span)
        {
            let Some((keys, functions, _spans)) = &binding.operation_table else {
                continue;
            };
            if operation_is_consumed(binding.symbol, &self.operation_symbols, &self.bindings) {
                operations.push(OperationSet {
                    name: binding.name.clone(),
                    count: functions.len(),
                    keys: keys.clone(),
                    functions: functions.clone(),
                });
            }
        }
        Ok(operations)
    }

    pub(super) fn finish(self) -> Result<ActualProperties, ActualPropertiesError> {
        let mut result = ActualProperties::default();
        let entry_span = self.entry.as_ref().map(|entry| entry.function_span);
        let owned_spans = entry_span.map(|entry| {
            let mut spans = BTreeSet::from([entry]);
            let mut changed = true;
            while changed {
                changed = false;
                for function in &self.functions {
                    if function
                        .owner_function
                        .is_some_and(|owner| spans.contains(&owner))
                        && spans.insert(function.span)
                    {
                        changed = true;
                    }
                }
            }
            spans
        });
        let owns = |owner: Option<Span>| match &owned_spans {
            Some(spans) => owner.is_some_and(|span| spans.contains(&span)),
            None => owner.is_none(),
        };
        let reachable_bindings = self
            .bindings
            .iter()
            .filter(|binding| {
                binding.owner_function == entry_span
                    || (entry_span.is_some()
                        && binding.owner_function.is_none()
                        && referenced_from_owned(&self.references, &owned_spans, binding.symbol))
            })
            .collect::<Vec<_>>();

        if let Some(entry) = self.entry.as_ref() {
            let arguments = entry.arguments.clone();
            if let Some(PropertyValue::Array(indices)) = arguments.get(3) {
                result.slots.initial_special_indices = indices.clone();
            }
            result.entry = Some(EntryProperties {
                function_name: entry.function_name.clone(),
                parameters: entry.parameters.clone(),
                initial_memory_slots: arguments.get(3).cloned(),
                arguments,
            });
        }

        for binding in reachable_bindings
            .iter()
            .filter(|binding| entry_span.is_some() && binding.owner_function.is_none())
        {
            result
                .properties
                .entry(binding.name.clone())
                .or_insert_with(|| binding.value.clone());
        }

        let candidate_decoders = self
            .decoder_calls
            .iter()
            .map(|call| resolve_alias(call.callee, &self.bindings, &self.mutable_symbols))
            .collect::<BTreeSet<_>>();
        let proven_decoders = self
            .functions
            .iter()
            .filter(|function| {
                function
                    .symbol
                    .is_some_and(|symbol| candidate_decoders.contains(&symbol))
            })
            .filter_map(|function| {
                function.symbol.and_then(|symbol| {
                    prove_runtime_decoder(self.source, function.span).map(|proof| {
                        (
                            resolve_alias(symbol, &self.bindings, &self.mutable_symbols),
                            proof,
                        )
                    })
                })
            })
            .collect::<BTreeMap<SymbolId, DecoderProof>>();

        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.owner_function == entry_span)
        {
            let value = binding.value.clone();
            result
                .properties
                .insert(binding.name.clone(), value.clone());
            if let Some((keys, functions, spans)) = &binding.operation_table {
                if operation_is_consumed(binding.symbol, &self.operation_symbols, &self.bindings) {
                    let _ = spans;
                    result.operations.push(OperationSet {
                        name: binding.name.clone(),
                        count: functions.len(),
                        keys: keys.clone(),
                        functions: functions.clone(),
                    });
                    continue;
                }
            }
            if is_thread_descriptors(&value) {
                add_named(&mut result.thread_descriptors, &binding.name, value);
                continue;
            }
            if is_string_array(&value) {
                add_named(&mut result.strings, &binding.name, value);
                continue;
            }
            if binding
                .decoder_callee
                .map(|callee| resolve_alias(callee, &self.bindings, &self.mutable_symbols))
                .is_some_and(|callee| proven_decoders.contains_key(&callee))
            {
                if let Some(call) = self.decoder_calls.iter().find(|call| {
                    Some(call.callee) == binding.decoder_callee
                        && call.owner == binding.owner_function
                        && Some(call.span) == binding.decoder_call_span
                }) {
                    let decoded = decoder_call_value(
                        call,
                        &self.limits,
                        self.wasm_bindings_for(binding.symbol),
                    );
                    if self.wasm_module_symbols.contains(&binding.symbol) {
                        add_named(&mut result.wasm, &binding.name, decoded);
                    } else {
                        add_named(&mut result.decoders, &binding.name, decoded);
                    }
                }
                continue;
            }
            if binding.wasm_constructor {
                add_named(&mut result.wasm, &binding.name, value);
                continue;
            }
            if is_dispatch_value(&value) {
                add_named(&mut result.dispatch, &binding.name, value);
                continue;
            }
            if is_number_array(&value)
                || contains_numeric_array(&value)
                || is_constant_value(&value)
                || is_reference_array(&value)
            {
                add_named(&mut result.constants, &binding.name, value);
                continue;
            }
            if matches!(value, PropertyValue::Expression(_)) {
                add_named(&mut result.unresolved, &binding.name, value);
            } else {
                add_named(&mut result.additional, &binding.name, value);
            }
        }

        for worker in &self.worker_bootstraps {
            if !owns(worker.owner) {
                continue;
            }
            let mut fields = BTreeMap::new();
            fields.insert("entryPc".into(), worker.entry_pc.clone());
            fields.insert("state".into(), worker.state.clone());
            fields.insert("descriptorIndex".into(), worker.descriptor_index.clone());
            fields.insert("initialSlots".into(), worker.initial_slots.clone());
            add_named(
                &mut result.additional,
                &worker.source_binding,
                PropertyValue::Object(fields),
            );
        }

        for function in &self.functions {
            if function.owner_function == entry_span
                || (function.owner_function.is_none()
                    && function.symbol.is_some_and(|symbol| {
                        referenced_from_owned(&self.references, &owned_spans, symbol)
                    }))
            {
                let source = self
                    .source
                    .get(function.span.start as usize..function.span.end as usize)
                    .unwrap_or_default()
                    .trim();
                if !source.is_empty() && !function.name.is_empty() {
                    result
                        .properties
                        .entry(function.name.clone())
                        .or_insert_with(|| PropertyValue::Function(source.into()));
                }
            }
        }

        for call in &self.decoder_calls {
            if owns(call.owner)
                && proven_decoders.contains_key(&resolve_alias(
                    call.callee,
                    &self.bindings,
                    &self.mutable_symbols,
                ))
                && call.arguments.len() > 1
            {
                add_named(
                    &mut result.bytecode,
                    &format!("{}#call", call.name),
                    decoder_call_value(call, &self.limits, None),
                );
            }
        }

        for call in &self.decoder_calls {
            let decoder_symbol = resolve_alias(call.callee, &self.bindings, &self.mutable_symbols);
            if owns(call.owner)
                && !proven_decoders.contains_key(&decoder_symbol)
                && candidate_decoders.contains(&decoder_symbol)
                && self
                    .functions
                    .iter()
                    .any(|function| function.symbol == Some(decoder_symbol))
                && looks_like_decoder_call(call)
            {
                let name = format!("{}#call", call.name);
                add_named(
                    &mut result.unproven_decoder_calls,
                    &name,
                    compact_decoder_call_record(call),
                );
                add_named(
                    &mut result.unresolved,
                    &name,
                    unresolved_decoder_call_value(call),
                );
            }
        }

        let mut memory_constructor_symbols = self
            .functions
            .iter()
            .filter(|function| function.name == "vmMemory")
            .filter_map(|function| {
                function.symbol.filter(|_| {
                    self.assignments.iter().any(|assignment| {
                        assignment.owner == function.span
                            && assignment.property == "slots"
                            && matches!(assignment.value, PropertyValue::Array(_))
                    })
                })
            })
            .collect::<BTreeSet<_>>();
        // Some kernels expose the slot memory through a small prototype
        // helper (`this.o = []`) instead of a field literally named `slots`.
        // Keep the role tied to the canonical VM memory constructor identity;
        // generic array/prototype shapes are not enough to claim VM memory.
        memory_constructor_symbols.extend(
            self.define_properties
                .iter()
                .filter_map(|property| property.constructor_symbol)
                .filter(|constructor| {
                    self.functions.iter().any(|function| {
                        function.symbol == Some(*constructor) && function.name == "vmMemory"
                    }) && self.assignments.iter().any(|assignment| {
                        self.functions.iter().any(|function| {
                            function.span == assignment.owner
                                && function.symbol == Some(*constructor)
                        }) && matches!(assignment.value, PropertyValue::Array(_))
                    })
                }),
        );
        let memory_method_names = self
            .define_properties
            .iter()
            .filter(|property| {
                property
                    .constructor_symbol
                    .is_some_and(|constructor| memory_constructor_symbols.contains(&constructor))
            })
            .map(|property| property.property.as_str())
            .collect::<BTreeSet<_>>();
        let memory_spans = self
            .functions
            .iter()
            .filter(|function| {
                function
                    .symbol
                    .is_some_and(|symbol| memory_constructor_symbols.contains(&symbol))
            })
            .map(|function| function.span)
            .collect::<BTreeSet<_>>();
        let function_name = |span: Span| {
            self.functions
                .iter()
                .find(|function| function.span == span)
                .map(|function| function.name.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("<constructor@{}>", span.start))
        };

        for assignment in &self.assignments {
            if owns(Some(assignment.owner)) {
                result
                    .slots
                    .constructor_properties
                    .entry(function_name(assignment.owner))
                    .or_default()
                    .entry(assignment.property.clone())
                    .or_insert_with(|| assignment.value.clone());
            }
        }

        let context_spans = self
            .functions
            .iter()
            .filter(|function| function.name == "vmContext")
            .map(|function| function.span)
            .collect::<BTreeSet<_>>();
        result.slots.memory_constructor = memory_spans.iter().find_map(|span| {
            let name = function_name(*span);
            (!name.starts_with('<')).then_some(name)
        });
        result.slots.context_constructor = context_spans.iter().find_map(|span| {
            let name = function_name(*span);
            (!name.starts_with('<')).then_some(name)
        });
        for assignment in &self.assignments {
            if !owns(Some(assignment.owner)) {
                continue;
            }
            let Some(target) = (if memory_spans.contains(&assignment.owner) {
                Some(&mut result.slots.memory)
            } else if context_spans.contains(&assignment.owner) {
                Some(&mut result.slots.context)
            } else {
                None
            }) else {
                continue;
            };
            target
                .entry(assignment.property.clone())
                .or_insert_with(|| assignment.value.clone());
        }

        for call in &self.slot_calls {
            if owns(call.owner)
                && call.receiver.as_ref().is_some_and(|receiver| {
                    self.is_memory_receiver(receiver, &memory_constructor_symbols)
                })
                && (matches!(call.method.as_str(), "writeSlot" | "initializeSlot")
                    || memory_method_names.contains(call.method.as_str()))
                && !call.arguments.is_empty()
            {
                let function = if call.function.is_empty() {
                    call.owner
                        .map(|span| format!("<function@{}>", span.start))
                        .unwrap_or_default()
                } else {
                    call.function.clone()
                };
                let assignment = SlotAssignment {
                    function,
                    method: call.method.clone(),
                    index: call.arguments[0].clone(),
                    value: call
                        .arguments
                        .get(1)
                        .cloned()
                        .unwrap_or(PropertyValue::Undefined),
                };
                if call.owner == entry_span {
                    result.slots.initial_assignments.push(assignment);
                } else {
                    result.slots.function_write_templates.push(assignment);
                }
            }
        }

        for property in &self.define_properties {
            if !owns(property.owner) {
                continue;
            }
            let is_memory_prototype = property
                .constructor_symbol
                .is_some_and(|constructor| memory_constructor_symbols.contains(&constructor));
            let is_context_prototype = property
                .constructor_symbol
                .and_then(|symbol| {
                    self.functions
                        .iter()
                        .find(|function| function.symbol == Some(symbol))
                        .map(|function| context_spans.contains(&function.span))
                })
                .unwrap_or(false);
            let constructor_name = property
                .constructor_symbol
                .and_then(|symbol| self.scoping.symbol_name(symbol).to_string().into())
                .unwrap_or_else(|| property.receiver.clone());
            result
                .slots
                .constructor_properties
                .entry(constructor_name)
                .or_default()
                .insert(property.property.clone(), property.value.clone());
            let is_stack = property.receiver == "Array" || property.receiver == "stack";
            if is_stack && result.slots.stack_constructor.is_none() {
                if let Some(owner) = property.owner {
                    let name = function_name(owner);
                    if !name.starts_with('<') {
                        result.slots.stack_constructor = Some(name);
                    }
                }
            }
            let target = if is_stack {
                &mut result.slots.stack
            } else if is_memory_prototype {
                &mut result.slots.memory_prototype
            } else if is_context_prototype {
                &mut result.slots.context_prototype
            } else {
                continue;
            };
            target
                .entry(property.property.clone())
                .or_insert_with(|| property.value.clone());
            if is_stack {
                let methods = result
                    .slots
                    .stack
                    .entry("methods".into())
                    .or_insert_with(|| PropertyValue::Array(Vec::new()));
                if let PropertyValue::Array(values) = methods {
                    let method = PropertyValue::String(property.property.clone());
                    if !values.contains(&method) {
                        values.push(method);
                    }
                }
            }
        }

        result.slots.stack_initial = result
            .slots
            .stack_constructor
            .as_deref()
            .and_then(|constructor| {
                self.functions
                    .iter()
                    .find(|function| function.name == constructor)
                    .and_then(|function| function.return_value.clone())
            })
            .filter(|value| matches!(value, PropertyValue::Array(_)));

        // Constructor parameter names are source data used by the concise
        // tracer bundle to map member initializers to argument roles. The
        // constructor bodies themselves are not part of that bundle.
        for (constructor, parameters) in [
            (
                result.slots.context_constructor.as_deref(),
                &mut result.slots.context_parameters,
            ),
            (
                result.slots.memory_constructor.as_deref(),
                &mut result.slots.memory_parameters,
            ),
            (
                result.slots.stack_constructor.as_deref(),
                &mut result.slots.stack_parameters,
            ),
        ] {
            let Some(constructor) = constructor else {
                continue;
            };
            if let Some(function) = self
                .functions
                .iter()
                .find(|function| function.name == constructor)
            {
                *parameters = function.parameters.clone();
            }
        }

        Ok(result)
    }

    fn is_memory_receiver(
        &self,
        receiver: &SlotReceiver,
        memory_constructor_symbols: &BTreeSet<SymbolId>,
    ) -> bool {
        match receiver {
            SlotReceiver::MemoryField => true,
            SlotReceiver::Prototype(symbol) => {
                let constructor = self
                    .prototype_aliases
                    .get(symbol)
                    .copied()
                    .unwrap_or(*symbol);
                memory_constructor_symbols.contains(&constructor)
            }
            SlotReceiver::Binding(symbol) => {
                let resolved = resolve_alias(*symbol, &self.bindings, &self.mutable_symbols);
                if self
                    .prototype_aliases
                    .get(&resolved)
                    .copied()
                    .is_some_and(|constructor| memory_constructor_symbols.contains(&constructor))
                {
                    return true;
                }
                let Some(binding) = self
                    .bindings
                    .iter()
                    .find(|binding| binding.symbol == resolved)
                else {
                    return false;
                };
                binding
                    .memory_constructor
                    .is_some_and(|constructor| memory_constructor_symbols.contains(&constructor))
                    || binding.object_methods.contains("writeSlot")
                    || binding.object_methods.contains("initializeSlot")
            }
        }
    }

    fn wasm_bindings_for(&self, symbol: SymbolId) -> Option<&Vec<WasmImportBinding>> {
        let mut current = symbol;
        let mut seen = BTreeSet::new();
        while seen.insert(current) {
            if let Some(bindings) = self.wasm_import_bindings.get(&current) {
                return Some(bindings);
            }
            let Some(input) = self.wasm_module_inputs.get(&current) else {
                break;
            };
            current = *input;
        }
        for (module, bindings) in &self.wasm_import_bindings {
            let mut current = *module;
            let mut seen = BTreeSet::new();
            while seen.insert(current) {
                if current == symbol {
                    return Some(bindings);
                }
                let Some(input) = self.wasm_module_inputs.get(&current) else {
                    break;
                };
                current = *input;
            }
        }
        None
    }

    fn slot_receiver(&self, object: &Expression<'a>) -> Option<SlotReceiver> {
        match object {
            Expression::Identifier(_) => {
                analysis::expression_symbol(object, self.scoping).map(SlotReceiver::Binding)
            }
            Expression::StaticMemberExpression(member) if member.property.name == "prototype" => {
                analysis::expression_symbol(&member.object, self.scoping)
                    .map(SlotReceiver::Prototype)
            }
            Expression::StaticMemberExpression(member) if member.property.name == "memory" => {
                Some(SlotReceiver::MemoryField)
            }
            Expression::ComputedMemberExpression(member)
                if static_member_key(&member.expression).as_deref() == Some("memory") =>
            {
                Some(SlotReceiver::MemoryField)
            }
            _ => None,
        }
    }

    fn value(
        &self,
        expression: &Expression<'a>,
        active: &mut BTreeSet<SymbolId>,
        depth: usize,
    ) -> PropertyValue {
        if depth >= self.limits.max_literal_depth {
            return PropertyValue::Expression(self.text(expression));
        }
        match expression {
            Expression::StringLiteral(value) => PropertyValue::String(value.value.to_string()),
            Expression::NumericLiteral(value) if value.value.is_nan() => PropertyValue::NaN,
            Expression::NumericLiteral(value) if value.value == f64::INFINITY => {
                PropertyValue::PositiveInfinity
            }
            Expression::NumericLiteral(value) if value.value == f64::NEG_INFINITY => {
                PropertyValue::NegativeInfinity
            }
            Expression::NumericLiteral(value) => PropertyValue::Number(value.value),
            Expression::BooleanLiteral(value) => PropertyValue::Boolean(value.value),
            Expression::NullLiteral(_) => PropertyValue::Null,
            Expression::Identifier(identifier)
                if identifier.name == "undefined" && self.identifier_is_global(identifier) =>
            {
                PropertyValue::Undefined
            }
            Expression::Identifier(identifier)
                if identifier.name == "NaN" && self.identifier_is_global(identifier) =>
            {
                PropertyValue::NaN
            }
            Expression::Identifier(identifier)
                if identifier.name == "Infinity" && self.identifier_is_global(identifier) =>
            {
                PropertyValue::PositiveInfinity
            }
            Expression::Identifier(identifier) => {
                let symbol = analysis::identifier_symbol(identifier, self.scoping);
                let Some(symbol) = symbol else {
                    return PropertyValue::Expression(self.text(expression));
                };
                if !active.insert(symbol) {
                    return PropertyValue::Expression(self.text(expression));
                }
                if self.mutable_symbols.contains(&symbol) {
                    active.remove(&symbol);
                    return PropertyValue::Expression(self.text(expression));
                }
                let value = self
                    .by_symbol
                    .get(&symbol)
                    .and_then(|index| self.bindings.get(*index))
                    .map(|binding| binding.value.clone())
                    .unwrap_or_else(|| PropertyValue::Expression(self.text(expression)));
                active.remove(&symbol);
                value
            }
            Expression::StaticMemberExpression(member) => self
                .literal_member_value(
                    &member.object,
                    member.property.name.as_ref(),
                    active,
                    depth + 1,
                )
                .unwrap_or_else(|| PropertyValue::Expression(self.text(expression))),
            Expression::ComputedMemberExpression(member) => {
                let key = match &member.expression {
                    Expression::StringLiteral(literal) => Some(literal.value.to_string()),
                    Expression::NumericLiteral(literal) => Some(literal.value.to_string()),
                    _ => None,
                };
                key.and_then(|key| {
                    self.literal_member_value(&member.object, &key, active, depth + 1)
                })
                .unwrap_or_else(|| PropertyValue::Expression(self.text(expression)))
            }
            Expression::UnaryExpression(unary) => match unary.operator {
                UnaryOperator::Void if side_effect_free_literal(&unary.argument) => {
                    PropertyValue::Undefined
                }
                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation => {
                    match self.value(&unary.argument, active, depth + 1) {
                        PropertyValue::Number(number) => PropertyValue::Number(
                            if unary.operator == UnaryOperator::UnaryNegation {
                                -number
                            } else {
                                number
                            },
                        ),
                        PropertyValue::NaN => PropertyValue::NaN,
                        PropertyValue::PositiveInfinity
                            if unary.operator == UnaryOperator::UnaryNegation =>
                        {
                            PropertyValue::NegativeInfinity
                        }
                        PropertyValue::NegativeInfinity
                            if unary.operator == UnaryOperator::UnaryPlus =>
                        {
                            PropertyValue::NegativeInfinity
                        }
                        _ => PropertyValue::Expression(self.text(expression)),
                    }
                }
                _ => PropertyValue::Expression(self.text(expression)),
            },
            Expression::ArrayExpression(array) => {
                if array.elements.len() > self.limits.max_literal_elements
                    || self.literal_budget.get() < array.elements.len()
                    || array
                        .elements
                        .iter()
                        .any(|element| matches!(element, ArrayExpressionElement::SpreadElement(_)))
                {
                    return PropertyValue::Expression(self.text(expression));
                }
                self.literal_budget.set(
                    self.literal_budget
                        .get()
                        .saturating_sub(array.elements.len()),
                );
                PropertyValue::Array(
                    array
                        .elements
                        .iter()
                        .map(|element| match element {
                            ArrayExpressionElement::Elision(_) => PropertyValue::Hole,
                            ArrayExpressionElement::SpreadElement(_) => unreachable!(),
                            other => self.value(other.to_expression(), active, depth + 1),
                        })
                        .collect(),
                )
            }
            Expression::ObjectExpression(object) => {
                if object.properties.len() > self.limits.max_literal_elements
                    || self.literal_budget.get() < object.properties.len()
                {
                    return PropertyValue::Expression(self.text(expression));
                }
                self.literal_budget.set(
                    self.literal_budget
                        .get()
                        .saturating_sub(object.properties.len()),
                );
                let mut fields = BTreeMap::new();
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return PropertyValue::Expression(self.text(expression));
                    };
                    if property.computed || property.method || property.kind != PropertyKind::Init {
                        return PropertyValue::Expression(self.text(expression));
                    }
                    let Some(key) = property_key(&property.key) else {
                        return PropertyValue::Expression(self.text(expression));
                    };
                    if fields.contains_key(&key) || key == "__proto__" {
                        return PropertyValue::Expression(self.text(expression));
                    }
                    fields.insert(key, self.value(&property.value, active, depth + 1));
                }
                PropertyValue::Object(fields)
            }
            Expression::NewExpression(new) if matches!(&new.callee, Expression::StaticMemberExpression(member) if member.property.name == "Memory" || member.property.name == "Module") =>
            {
                let mut fields = BTreeMap::new();
                if let Expression::StaticMemberExpression(member) = &new.callee {
                    fields.insert(
                        "constructor".into(),
                        PropertyValue::String(format!("WebAssembly.{}", member.property.name)),
                    );
                }
                if let Some(argument) = new.arguments.first().and_then(Argument::as_expression) {
                    // Keep a named input as a source backreference instead of
                    // inlining its resolved literal: the binding is emitted once
                    // (as a decoder payload or runtime binding) and the module
                    // record points at it.
                    let named_input = matches!(argument, Expression::Identifier(_))
                        && analysis::expression_symbol(argument, self.scoping)
                            .is_some_and(|symbol| self.by_symbol.contains_key(&symbol));
                    let value = if named_input {
                        PropertyValue::Expression(self.text(argument))
                    } else {
                        self.value(argument, active, depth + 1)
                    };
                    fields.insert("argument".into(), value);
                }
                PropertyValue::Object(fields)
            }
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
                PropertyValue::Function(self.text(expression))
            }
            _ => PropertyValue::Expression(self.text(expression)),
        }
    }

    fn record_define_property(&mut self, call: &CallExpression<'a>) {
        let Some(receiver) = call.arguments.first().and_then(Argument::as_expression) else {
            return;
        };
        let Some(key_expression) = call.arguments.get(1).and_then(Argument::as_expression) else {
            return;
        };
        let Some(property) = property_key_expression(key_expression, self) else {
            return;
        };
        let Some(descriptor) = call.arguments.get(2).and_then(Argument::as_expression) else {
            return;
        };
        self.define_properties.push(DefineProperty {
            owner: self.function_stack.last().map(|(_, span)| *span),
            receiver: self.text(receiver),
            constructor_symbol: constructor_symbol_for_receiver(
                receiver,
                &self.prototype_aliases,
                self.scoping,
            ),
            property,
            value: self.value(descriptor, &mut BTreeSet::new(), 0),
        });
    }

    /// A renamed `Object.defineProperty` alias, resolved through the extractor's
    /// own binding values (never by re-tokenizing arbitrary source text).
    fn is_define_property_alias_call(&self, callee: &Expression<'a>) -> bool {
        let Expression::Identifier(_) = callee else {
            return false;
        };
        let Some(symbol) = analysis::expression_symbol(callee, self.scoping) else {
            return false;
        };
        let Some(binding) = self
            .by_symbol
            .get(&symbol)
            .and_then(|index| self.bindings.get(*index))
        else {
            return false;
        };
        let PropertyValue::Expression(text) = &binding.value else {
            return false;
        };
        let Some((object, property)) = text.rsplit_once('.') else {
            return false;
        };
        property == "defineProperty" && self.is_object_alias(object)
    }

    fn is_object_alias(&self, name: &str) -> bool {
        let mut current = name.trim();
        for _ in 0..8 {
            if current == "Object" {
                return true;
            }
            let Some(binding) = self.bindings.iter().find(|binding| binding.name == current) else {
                return false;
            };
            let PropertyValue::Expression(text) = &binding.value else {
                return false;
            };
            let text = text.trim();
            if text.contains('.') || text.contains(' ') {
                return false;
            }
            current = text;
        }
        false
    }

    fn text(&self, expression: &Expression<'a>) -> String {
        self.source
            .get(expression.span().start as usize..expression.span().end as usize)
            .unwrap_or_default()
            .trim()
            .into()
    }

    fn literal_member_value(
        &self,
        object: &Expression<'a>,
        key: &str,
        active: &mut BTreeSet<SymbolId>,
        depth: usize,
    ) -> Option<PropertyValue> {
        let value = self.value(object, active, depth);
        match value {
            PropertyValue::Array(values) => key
                .parse::<usize>()
                .ok()
                .and_then(|index| values.get(index).cloned()),
            PropertyValue::Object(fields) => fields.get(key).cloned(),
            _ => None,
        }
    }

    /// The WASM adapter emits `vmHostBindings` with computed string keys. Keep
    /// that known adapter object structural, while retaining function values
    /// for the compact renderer to summarize without their bodies. Ordinary
    /// computed object literals remain opaque expressions in `value`.
    fn adapter_binding_value(&self, expression: &Expression<'a>) -> PropertyValue {
        let Expression::ObjectExpression(object) = expression else {
            return self.value(expression, &mut BTreeSet::new(), 0);
        };
        let mut fields = BTreeMap::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return PropertyValue::Expression("<opaque vmHostBindings object>".into());
            };
            let Some(key) = property_key(&property.key) else {
                return PropertyValue::Expression("<opaque vmHostBindings object>".into());
            };
            let value = match &property.value {
                Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
                    PropertyValue::Function(self.text(&property.value))
                }
                _ => self.value(&property.value, &mut BTreeSet::new(), 0),
            };
            fields.insert(key, value);
        }
        PropertyValue::Object(fields)
    }

    fn record_worker_bootstrap(&mut self, source_binding: &str, expression: &Expression<'a>) {
        let Some(entry_symbol) = self.entry.as_ref().and_then(|entry| entry.symbol) else {
            return;
        };
        let mut pieces = Vec::new();
        flatten_worker_concatenation(expression, &mut pieces);
        let Some(entry_position) = pieces.iter().position(|piece| {
            analysis::expression_symbol(piece, self.scoping) == Some(entry_symbol)
        }) else {
            return;
        };
        if entry_position == 0
            || string_literal_value(pieces[entry_position - 1]) != Some("(")
            || pieces
                .get(entry_position + 1)
                .and_then(|piece| string_literal_value(piece))
                != Some(")(self,")
        {
            return;
        }
        let Some(entry_pc) = pieces.get(entry_position + 2) else {
            return;
        };
        if pieces
            .get(entry_position + 3)
            .and_then(|piece| string_literal_value(piece))
            != Some(",")
        {
            return;
        }
        let Some(state) = pieces.get(entry_position + 4) else {
            return;
        };
        if pieces
            .get(entry_position + 5)
            .and_then(|piece| string_literal_value(piece))
            != Some(",")
        {
            return;
        }
        let Some(argument_expression) = pieces.get(entry_position + 6) else {
            return;
        };
        let Some(argument) = find_json_stringify_argument(argument_expression, self.scoping) else {
            return;
        };
        let initial_slots = self.value(argument, &mut BTreeSet::new(), 0);

        let Some(index_expression) = find_descriptor_index(argument) else {
            return;
        };
        let descriptor_index = self.value(index_expression, &mut BTreeSet::new(), 0);
        self.worker_bootstraps.push(WorkerBootstrap {
            source_binding: source_binding.into(),
            owner: self.function_stack.last().map(|(_, span)| *span),
            entry_pc: self.value(entry_pc, &mut BTreeSet::new(), 0),
            state: self.value(state, &mut BTreeSet::new(), 0),
            descriptor_index,
            initial_slots,
        });
    }

    fn identifier_is_global(&self, identifier: &IdentifierReference<'a>) -> bool {
        analysis::identifier_symbol(identifier, self.scoping).is_none()
    }
}

impl<'a> Visit<'a> for Scanner<'a> {
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(identifier) = &it.id {
            if let Some(symbol) = identifier.symbol_id.get() {
                if let Some(Expression::StaticMemberExpression(member)) = it.init.as_ref() {
                    if member.property.name == "prototype" {
                        if let Some(constructor) =
                            analysis::expression_symbol(&member.object, self.scoping)
                        {
                            self.prototype_aliases.insert(symbol, constructor);
                        }
                    }
                }
                let value = it
                    .init
                    .as_ref()
                    .map(|init| {
                        if identifier.name == "vmHostBindings" {
                            self.adapter_binding_value(init)
                        } else {
                            self.value(init, &mut BTreeSet::new(), 0)
                        }
                    })
                    .unwrap_or_else(|| PropertyValue::Expression("<uninitialized>".into()));
                if let Some(init) = it.init.as_ref() {
                    self.record_worker_bootstrap(identifier.name.as_ref(), init);
                }
                let wasm_constructor = it.init.as_ref().is_some_and(is_wasm_constructor);
                if let Some(Expression::NewExpression(new)) = it.init.as_ref() {
                    if matches!(
                        &new.callee,
                        Expression::StaticMemberExpression(member) if member.property.name == "Module"
                    ) {
                        if let Some(input) = new
                            .arguments
                            .first()
                            .and_then(Argument::as_expression)
                            .and_then(|argument| {
                                analysis::expression_symbol(argument, self.scoping)
                            })
                        {
                            self.wasm_module_inputs.insert(symbol, input);
                        }
                    }
                }
                let operation_table = it
                    .init
                    .as_ref()
                    .and_then(|init| operation_table(init, self.source));
                let alias_of = it
                    .init
                    .as_ref()
                    .and_then(|init| analysis::expression_symbol(init, self.scoping));
                let decoder_callee = it.init.as_ref().and_then(|init| match init {
                    Expression::CallExpression(call) => {
                        analysis::expression_symbol(&call.callee, self.scoping)
                    }
                    _ => None,
                });
                let memory_constructor = it.init.as_ref().and_then(|init| match init {
                    Expression::NewExpression(new) => {
                        analysis::expression_symbol(&new.callee, self.scoping)
                    }
                    _ => None,
                });
                let object_methods = match &value {
                    PropertyValue::Object(fields) => fields.keys().cloned().collect(),
                    _ => BTreeSet::new(),
                };
                let owner_function = self.function_stack.last().map(|(_, span)| *span);
                let index = self.bindings.len();
                self.by_symbol.insert(symbol, index);
                self.bindings.push(Binding {
                    symbol,
                    name: identifier.name.to_string(),
                    owner_function,
                    value,
                    decoder_callee,
                    decoder_call_span: it.init.as_ref().and_then(|init| match init {
                        Expression::CallExpression(call) => Some(call.span),
                        _ => None,
                    }),
                    alias_of,
                    memory_constructor,
                    object_methods,
                    wasm_constructor,
                    operation_table,
                });
            }
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        let name = it
            .id
            .as_ref()
            .map(|id| id.name.to_string())
            .unwrap_or_default();
        self.functions.push(FunctionInfo {
            name: name.clone(),
            span: it.span,
            owner_function: self.function_stack.last().map(|(_, span)| *span),
            symbol: it.id.as_ref().and_then(|id| id.symbol_id.get()),
            parameters: it
                .params
                .items
                .iter()
                .filter_map(|item| simple_pattern_name(&item.pattern))
                .collect(),
            return_value: None,
        });
        self.function_stack.push((name, it.span));
        walk::walk_function(self, it, flags);
        self.function_stack.pop();
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.functions.push(FunctionInfo {
            name: String::new(),
            span: it.span,
            owner_function: self.function_stack.last().map(|(_, span)| *span),
            symbol: None,
            parameters: it
                .params
                .items
                .iter()
                .filter_map(|item| simple_pattern_name(&item.pattern))
                .collect(),
            return_value: None,
        });
        self.function_stack.push((String::new(), it.span));
        walk::walk_arrow_function_expression(self, it);
        self.function_stack.pop();
    }

    fn visit_return_statement(&mut self, it: &ReturnStatement<'a>) {
        let Some(span) = self.function_stack.last().map(|(_, span)| *span) else {
            return;
        };
        let Some(argument) = it.argument.as_ref() else {
            return;
        };
        let value = self.value(argument, &mut BTreeSet::new(), 0);
        if !matches!(value, PropertyValue::Array(_)) {
            walk::walk_return_statement(self, it);
            return;
        }
        if let Some(function) = self
            .functions
            .iter_mut()
            .find(|function| function.span == span)
        {
            if function.return_value.is_none() {
                function.return_value = Some(value);
            }
        }
        walk::walk_return_statement(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if self.entry.is_none() {
            if let Some((function_span, symbol, function_name, parameters)) =
                entry_function(&it.callee)
            {
                if it.arguments.len() >= 4
                    && it
                        .arguments
                        .get(3)
                        .and_then(Argument::as_expression)
                        .is_some_and(|expression| {
                            matches!(expression, Expression::ArrayExpression(_))
                        })
                    && contains_host_name(it.arguments.first().and_then(Argument::as_expression))
                {
                    self.entry = Some(EntryCandidate {
                        function_span,
                        symbol,
                        function_name,
                        parameters,
                        arguments: it
                            .arguments
                            .iter()
                            .filter_map(Argument::as_expression)
                            .map(|argument| self.value(argument, &mut BTreeSet::new(), 0))
                            .collect(),
                    });
                }
            }
        }
        if let Expression::ComputedMemberExpression(member) = &it.callee {
            if let Some(symbol) = analysis::expression_symbol(&member.object, self.scoping) {
                self.operation_symbols.insert(symbol);
            }
        }
        if let Some((object, method)) = slot_call_member(&it.callee) {
            if !(matches!(object, Expression::Identifier(identifier) if identifier.name == "Object")
                && method == "defineProperty")
            {
                self.slot_calls.push(SlotCall {
                    owner: self.function_stack.last().map(|(_, span)| *span),
                    function: self
                        .function_stack
                        .last()
                        .map(|(name, _)| name.clone())
                        .unwrap_or_default(),
                    method: method.clone(),
                    receiver: self.slot_receiver(object),
                    arguments: it
                        .arguments
                        .iter()
                        .filter_map(Argument::as_expression)
                        .map(|argument| self.value(argument, &mut BTreeSet::new(), 0))
                        .collect(),
                });
            }
            if matches!(object, Expression::Identifier(identifier) if identifier.name == "Object")
                && method == "defineProperty"
            {
                self.record_define_property(it);
            }
        } else if self.is_define_property_alias_call(&it.callee) {
            // A renamed `DR = Object.defineProperty` alias still describes the
            // prototype/descriptor semantics; record it without pretending the
            // alias was executed.
            self.record_define_property(it);
        }
        if let Some(callee) = analysis::expression_symbol(&it.callee, self.scoping) {
            self.decoder_calls.push(DecoderCall {
                span: it.span,
                owner: self.function_stack.last().map(|(_, span)| *span),
                callee,
                name: self
                    .source
                    .get(it.callee.span().start as usize..it.callee.span().end as usize)
                    .unwrap_or_default()
                    .into(),
                arguments: it
                    .arguments
                    .iter()
                    .filter_map(Argument::as_expression)
                    .map(|argument| self.value(argument, &mut BTreeSet::new(), 0))
                    .collect(),
                argument_text: it
                    .arguments
                    .iter()
                    .filter_map(Argument::as_expression)
                    .map(|argument| self.text(argument))
                    .collect(),
            });
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        walk::walk_computed_member_expression(self, it);
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        let member_object = match &it.left {
            AssignmentTarget::StaticMemberExpression(member) => Some(&member.object),
            AssignmentTarget::ComputedMemberExpression(member) => Some(&member.object),
            _ => None,
        };
        if let Some(symbol) =
            member_object.and_then(|object| analysis::expression_symbol(object, self.scoping))
        {
            self.mutable_symbols.insert(symbol);
        }
        match &it.left {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                    self.mutable_symbols.insert(symbol);
                }
            }
            AssignmentTarget::StaticMemberExpression(member)
                if matches!(&member.object, Expression::ThisExpression(_)) =>
            {
                self.assignments.push(Assignment {
                    owner: self
                        .function_stack
                        .last()
                        .map(|(_, span)| *span)
                        .unwrap_or(Span::new(0, 0)),
                    property: member.property.name.to_string(),
                    value: self.value(&it.right, &mut BTreeSet::new(), 0),
                });
            }
            AssignmentTarget::ComputedMemberExpression(member)
                if matches!(&member.object, Expression::ThisExpression(_)) =>
            {
                let property = property_key_expression(&member.expression, self)
                    .unwrap_or_else(|| self.text(&member.expression));
                self.assignments.push(Assignment {
                    owner: self
                        .function_stack
                        .last()
                        .map(|(_, span)| *span)
                        .unwrap_or(Span::new(0, 0)),
                    property,
                    value: self.value(&it.right, &mut BTreeSet::new(), 0),
                });
            }
            _ => {}
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        if let Some((_, owner)) = self.function_stack.last() {
            if let Some(symbol) = analysis::identifier_symbol(it, self.scoping) {
                self.references.entry(*owner).or_default().insert(symbol);
            }
        }
    }

    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        if let Expression::StaticMemberExpression(member) = &it.callee {
            if member.property.name == "Module" {
                if let Some(symbol) = it
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|argument| analysis::expression_symbol(argument, self.scoping))
                {
                    self.wasm_module_symbols.insert(symbol);
                }
            } else if member.property.name == "Instance" {
                if let Some(module) = it
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|argument| analysis::expression_symbol(argument, self.scoping))
                {
                    if let Some(imports) = it.arguments.get(1).and_then(Argument::as_expression) {
                        self.wasm_import_bindings
                            .entry(module)
                            .or_default()
                            .extend(wasm_import_bindings(imports, self.source));
                    }
                }
            }
        }
        walk::walk_new_expression(self, it);
    }
}

fn static_member_key(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::NumericLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn slot_call_member<'a>(callee: &'a Expression<'a>) -> Option<(&'a Expression<'a>, String)> {
    match callee {
        Expression::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.to_string()))
        }
        Expression::ComputedMemberExpression(member) => {
            Some((&member.object, static_member_key(&member.expression)?))
        }
        _ => None,
    }
}

fn constructor_symbol_for_receiver(
    expression: &Expression<'_>,
    prototype_aliases: &BTreeMap<SymbolId, SymbolId>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match expression {
        Expression::Identifier(_) => analysis::expression_symbol(expression, scoping)
            .and_then(|symbol| prototype_aliases.get(&symbol).copied().or(Some(symbol))),
        Expression::StaticMemberExpression(member) if member.property.name == "prototype" => {
            analysis::expression_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn resolve_alias(
    symbol: SymbolId,
    bindings: &[Binding],
    mutable_symbols: &BTreeSet<SymbolId>,
) -> SymbolId {
    let mut current = symbol;
    let mut seen = BTreeSet::new();
    while seen.insert(current) {
        if mutable_symbols.contains(&current) {
            break;
        }
        let Some(binding) = bindings.iter().find(|binding| binding.symbol == current) else {
            break;
        };
        let Some(alias) = binding.alias_of else { break };
        current = alias;
    }
    current
}

type EntryFunction = (Span, Option<SymbolId>, Option<String>, Vec<String>);

fn entry_function(expression: &Expression<'_>) -> Option<EntryFunction> {
    let function = match expression {
        Expression::FunctionExpression(function) => function,
        Expression::ArrowFunctionExpression(function) => {
            return Some((
                function.span,
                None,
                None,
                function
                    .params
                    .items
                    .iter()
                    .filter_map(|item| simple_pattern_name(&item.pattern))
                    .collect(),
            ))
        }
        _ => return None,
    };
    Some((
        function.span,
        function.id.as_ref().and_then(|id| id.symbol_id.get()),
        function.id.as_ref().map(|id| id.name.to_string()),
        function
            .params
            .items
            .iter()
            .filter_map(|item| simple_pattern_name(&item.pattern))
            .collect(),
    ))
}

fn simple_pattern_name(pattern: &BindingPattern<'_>) -> Option<String> {
    if let BindingPattern::BindingIdentifier(id) = pattern {
        Some(id.name.to_string())
    } else {
        None
    }
}

fn contains_host_name(expression: Option<&Expression<'_>>) -> bool {
    struct HostProbe(bool);
    impl<'a> Visit<'a> for HostProbe {
        fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
            self.0 |= matches!(it.name.as_str(), "window" | "global" | "self");
        }
    }
    let mut probe = HostProbe(false);
    if let Some(expression) = expression {
        probe.visit_expression(expression);
    }
    probe.0
}

fn operation_table(
    expression: &Expression<'_>,
    source: &str,
) -> Option<(Vec<String>, Vec<String>, Vec<Span>)> {
    let mut keys = Vec::new();
    let mut functions = Vec::new();
    let mut spans = Vec::new();
    match expression {
        Expression::ArrayExpression(array) => {
            for (index, element) in array.elements.iter().enumerate() {
                let function = match element {
                    ArrayExpressionElement::FunctionExpression(function) => Some(function.span),
                    ArrayExpressionElement::ArrowFunctionExpression(function) => {
                        Some(function.span)
                    }
                    _ => None,
                }?;
                keys.push(index.to_string());
                functions.push(
                    source
                        .get(function.start as usize..function.end as usize)
                        .unwrap_or_default()
                        .into(),
                );
                spans.push(function);
            }
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                let function = match &property.value {
                    Expression::FunctionExpression(function) => function.span,
                    Expression::ArrowFunctionExpression(function) => function.span,
                    _ => return None,
                };
                keys.push(property_key(&property.key)?);
                functions.push(
                    source
                        .get(function.start as usize..function.end as usize)
                        .unwrap_or_default()
                        .into(),
                );
                spans.push(function);
            }
        }
        _ => return None,
    }
    (!functions.is_empty()).then_some((keys, functions, spans))
}

fn property_key(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(value) => Some(value.value.to_string()),
        PropertyKey::NumericLiteral(value) => Some(value.value.to_string()),
        _ => None,
    }
}

fn is_thread_descriptors(value: &PropertyValue) -> bool {
    let PropertyValue::Array(rows) = value else {
        return false;
    };
    !rows.is_empty()
        && rows.iter().all(|row| {
            matches!(row, PropertyValue::Object(fields) if fields.values().filter(|value| matches!(value, PropertyValue::Array(_))).count() >= 2)
        })
}

fn operation_is_consumed(
    symbol: SymbolId,
    consumed: &BTreeSet<SymbolId>,
    bindings: &[Binding],
) -> bool {
    let mut current = symbol;
    let mut seen = BTreeSet::new();
    loop {
        if consumed.contains(&current) {
            return true;
        }
        if !seen.insert(current) {
            return false;
        }
        let Some(binding) = bindings.iter().find(|binding| binding.symbol == current) else {
            return false;
        };
        let Some(alias) = binding.alias_of else {
            return false;
        };
        current = alias;
    }
}

fn is_string_array(value: &PropertyValue) -> bool {
    let PropertyValue::Array(values) = value else {
        return false;
    };
    !values.is_empty()
        && values
            .iter()
            .all(|value| matches!(value, PropertyValue::String(_)))
}

fn is_number_array(value: &PropertyValue) -> bool {
    matches!(value, PropertyValue::Array(values) if !values.is_empty() && values.iter().all(|value| matches!(value, PropertyValue::Number(_) | PropertyValue::NaN | PropertyValue::PositiveInfinity | PropertyValue::NegativeInfinity)))
}

fn contains_numeric_array(value: &PropertyValue) -> bool {
    match value {
        PropertyValue::Array(values) => {
            is_number_array(value) || values.iter().any(contains_numeric_array)
        }
        PropertyValue::Object(fields) => fields.values().any(contains_numeric_array),
        _ => false,
    }
}

fn is_reference_array(value: &PropertyValue) -> bool {
    matches!(value, PropertyValue::Array(values) if !values.is_empty() && values.iter().all(|value| matches!(value, PropertyValue::Expression(_) | PropertyValue::Function(_) | PropertyValue::Undefined)))
}

fn is_constant_value(value: &PropertyValue) -> bool {
    matches!(value, PropertyValue::Object(fields) if !fields.is_empty())
}

fn add_named(values: &mut Vec<NamedValue>, name: &str, value: PropertyValue) {
    values.push(NamedValue {
        name: name.into(),
        value,
    });
}

fn decoder_call_value(
    call: &DecoderCall,
    limits: &ExtractionLimits,
    import_bindings: Option<&Vec<WasmImportBinding>>,
) -> PropertyValue {
    let mut fields = decoder_call_fields(call);
    if let Some(PropertyValue::String(encoded)) = call.arguments.first() {
        match wasm_module::decode_base64(encoded, Base64Policy::UrlSafe) {
            Ok(bytes) if bytes.len() <= limits.max_decoded_bytes => {
                if let Some((imports, exports)) = wasm_summary(&bytes) {
                    fields.insert(
                        "imports".into(),
                        PropertyValue::Array(
                            imports.iter().cloned().map(PropertyValue::String).collect(),
                        ),
                    );
                    fields.insert(
                        "exports".into(),
                        PropertyValue::Array(
                            exports.into_iter().map(PropertyValue::String).collect(),
                        ),
                    );
                    if let Some(import_bindings) = import_bindings {
                        let mut bindings = Vec::new();
                        for import in &imports {
                            let Some((namespace, property)) = import.split_once("::") else {
                                continue;
                            };
                            let binding = import_bindings
                                .iter()
                                .filter(|candidate| {
                                    candidate.namespace.as_deref() == Some(namespace)
                                        && candidate.property.as_deref() == Some(property)
                                })
                                .chain(import_bindings.iter().filter(|candidate| {
                                    candidate.namespace.as_deref() == Some(namespace)
                                        && candidate.property.is_none()
                                }))
                                .chain(import_bindings.iter().filter(|candidate| {
                                    candidate.namespace.is_none() && candidate.property.is_none()
                                }))
                                .next()
                                .map(|candidate| candidate.binding.clone());
                            let mut fields_for_import = BTreeMap::new();
                            fields_for_import.insert(
                                "namespace".into(),
                                PropertyValue::String(namespace.into()),
                            );
                            fields_for_import
                                .insert("property".into(), PropertyValue::String(property.into()));
                            if let Some(binding) = binding {
                                fields_for_import
                                    .insert("binding".into(), PropertyValue::String(binding));
                            }
                            bindings.push(PropertyValue::Object(fields_for_import));
                        }
                        if !bindings.is_empty() {
                            fields.insert("import_bindings".into(), PropertyValue::Array(bindings));
                        }
                    }
                }
                fields.insert(
                    "decoded".into(),
                    PropertyValue::Array(
                        bytes
                            .into_iter()
                            .map(|byte| PropertyValue::Number(f64::from(byte)))
                            .collect(),
                    ),
                );
            }
            Ok(_) => {
                fields.insert(
                    "decoded".into(),
                    PropertyValue::Expression("<decoded bytes exceed limit>".into()),
                );
            }
            Err(_) => {
                fields.insert(
                    "decoded".into(),
                    PropertyValue::Expression("<unresolved decoder result>".into()),
                );
            }
        }
    }
    PropertyValue::Object(fields)
}

fn looks_like_decoder_call(call: &DecoderCall) -> bool {
    call.arguments.len() > 1 && matches!(call.arguments.first(), Some(PropertyValue::String(_)))
}

fn unresolved_decoder_call_value(call: &DecoderCall) -> PropertyValue {
    PropertyValue::Object(decoder_call_fields(call))
}

/// One compact, non-executed record for an unproven decoder call: the callee
/// name, the exact encoded literal, the destination binding, and only any
/// arguments beyond the encoded input/destination pair.
fn compact_decoder_call_record(call: &DecoderCall) -> PropertyValue {
    let mut fields = BTreeMap::new();
    fields.insert("callee".into(), PropertyValue::String(call.name.clone()));
    if let Some(PropertyValue::String(encoded)) = call.arguments.first() {
        fields.insert("encoded".into(), PropertyValue::String(encoded.clone()));
    }
    if let Some(destination) = call.arguments.get(1) {
        fields.insert("destination".into(), destination.clone());
    }
    if call.arguments.len() > 2 {
        fields.insert(
            "extra_arguments".into(),
            PropertyValue::Array(call.arguments[2..].to_vec()),
        );
    }
    fields.insert("proven".into(), PropertyValue::Boolean(false));
    PropertyValue::Object(fields)
}

pub(crate) fn is_worker_bootstrap_value(value: &PropertyValue) -> bool {
    matches!(value, PropertyValue::Object(fields)
        if fields.contains_key("entryPc")
            && fields.contains_key("state")
            && fields.contains_key("descriptorIndex")
            && fields.contains_key("initialSlots"))
}

fn decoder_call_fields(call: &DecoderCall) -> BTreeMap<String, PropertyValue> {
    let mut fields = BTreeMap::new();
    fields.insert(
        "expression".into(),
        PropertyValue::Expression(format!("{}({})", call.name, call.argument_text.join(", "))),
    );
    fields.insert(
        "arguments".into(),
        PropertyValue::Array(call.arguments.clone()),
    );
    if let Some(PropertyValue::String(encoded)) = call.arguments.first() {
        fields.insert("encoded".into(), PropertyValue::String(encoded.clone()));
    }
    if let Some(destination) = call.argument_text.get(1) {
        fields.insert(
            "destination".into(),
            PropertyValue::Expression(destination.clone()),
        );
    }
    fields
}

fn referenced_from_owned(
    references: &BTreeMap<Span, BTreeSet<SymbolId>>,
    owned_spans: &Option<BTreeSet<Span>>,
    symbol: SymbolId,
) -> bool {
    references.iter().any(|(span, symbols)| {
        owned_spans
            .as_ref()
            .is_none_or(|owned| owned.contains(span))
            && symbols.contains(&symbol)
    })
}

fn property_key_expression(expression: &Expression<'_>, scanner: &Scanner<'_>) -> Option<String> {
    match scanner.value(expression, &mut BTreeSet::new(), 0) {
        PropertyValue::String(value) => Some(value),
        PropertyValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn side_effect_free_literal(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::ArrayExpression(array) => array.elements.iter().all(|element| {
            !matches!(element, ArrayExpressionElement::SpreadElement(_))
                && side_effect_free_literal(element.to_expression())
        }),
        Expression::ObjectExpression(object) => object.properties.iter().all(|property| {
            matches!(property, ObjectPropertyKind::ObjectProperty(property) if !property.computed && !property.method && property.kind == PropertyKind::Init && side_effect_free_literal(&property.value))
        }),
        _ => false,
    }
}

fn find_json_stringify_argument<'a>(
    expression: &'a Expression<'a>,
    scoping: &Scoping,
) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
            find_json_stringify_argument(&binary.left, scoping)
                .or_else(|| find_json_stringify_argument(&binary.right, scoping))
        }
        Expression::CallExpression(call)
            if matches!(
                &call.callee,
                Expression::StaticMemberExpression(member)
                    if member.property.name == "stringify"
                        && matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "JSON")
            ) =>
        {
            let Expression::StaticMemberExpression(member) = &call.callee else {
                return None;
            };
            let Expression::Identifier(identifier) = &member.object else {
                return None;
            };
            if analysis::identifier_symbol(identifier, scoping).is_some() {
                return None;
            }
            call.arguments.first().and_then(Argument::as_expression)
        }
        _ => None,
    }
}

fn string_literal_value<'a>(expression: &Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn contains_string_literal(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::StringLiteral(_) => true,
        Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
            contains_string_literal(&binary.left) || contains_string_literal(&binary.right)
        }
        _ => false,
    }
}

fn flatten_worker_concatenation<'a>(
    expression: &'a Expression<'a>,
    pieces: &mut Vec<&'a Expression<'a>>,
) {
    if let Expression::BinaryExpression(binary) = expression {
        if binary.operator == BinaryOperator::Addition
            && (contains_string_literal(&binary.left) || contains_string_literal(&binary.right))
        {
            flatten_worker_concatenation(&binary.left, pieces);
            flatten_worker_concatenation(&binary.right, pieces);
            return;
        }
    }
    pieces.push(expression);
}

fn find_descriptor_index<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::StaticMemberExpression(member)
            if matches!(&member.object, Expression::ComputedMemberExpression(_)) =>
        {
            match &member.object {
                Expression::ComputedMemberExpression(member) => Some(&member.expression),
                _ => None,
            }
        }
        Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
            find_descriptor_index(&binary.left).or_else(|| find_descriptor_index(&binary.right))
        }
        Expression::CallExpression(call) => call
            .arguments
            .iter()
            .filter_map(Argument::as_expression)
            .find_map(find_descriptor_index),
        _ => None,
    }
}

// A decoder call is decoded only after prove_runtime_decoder succeeds.

fn is_wasm_constructor(expression: &Expression<'_>) -> bool {
    matches!(expression, Expression::NewExpression(new) if matches!(&new.callee, Expression::StaticMemberExpression(member) if member.property.name == "Memory" || member.property.name == "Module"))
}

fn wasm_import_bindings(expression: &Expression<'_>, source: &str) -> Vec<WasmImportBinding> {
    let mut bindings = Vec::new();
    let Expression::ObjectExpression(object) = expression else {
        bindings.push(WasmImportBinding {
            namespace: None,
            property: None,
            binding: expression_text(expression, source),
        });
        return bindings;
    };
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            continue;
        };
        let Some(namespace) = property_key(&property.key) else {
            continue;
        };
        if let Expression::ObjectExpression(properties) = &property.value {
            for nested in &properties.properties {
                let ObjectPropertyKind::ObjectProperty(nested) = nested else {
                    continue;
                };
                let Some(name) = property_key(&nested.key) else {
                    continue;
                };
                bindings.push(WasmImportBinding {
                    namespace: Some(namespace.clone()),
                    property: Some(name),
                    binding: expression_text(&nested.value, source),
                });
            }
        } else {
            bindings.push(WasmImportBinding {
                namespace: Some(namespace),
                property: None,
                binding: expression_text(&property.value, source),
            });
        }
    }
    bindings
}

fn expression_text(expression: &Expression<'_>, source: &str) -> String {
    source
        .get(expression.span().start as usize..expression.span().end as usize)
        .unwrap_or_default()
        .trim()
        .into()
}

pub(crate) fn wasm_summary(bytes: &[u8]) -> Option<(Vec<String>, Vec<String>)> {
    if bytes.len() < 8 || &bytes[..4] != b"\0asm" {
        return None;
    }
    // Actual-properties deliberately ignored validation and every section it
    // did not read.  Project only the neutral ABI; a detailed parse error in a
    // type/table/element/body section is not this caller's failure.
    let summary = wasm_module::inspect_module(bytes, ModuleLimits::for_input(bytes)).ok()?;
    if summary.abi.first_error.is_some() {
        return None;
    }
    Some((
        summary
            .abi
            .imports
            .iter()
            .map(|import| format!("{}::{}", import.namespace, import.name))
            .collect(),
        summary
            .abi
            .exports
            .iter()
            .map(|export| export.name.clone())
            .collect(),
    ))
}

fn is_dispatch_value(value: &PropertyValue) -> bool {
    let PropertyValue::Array(rows) = value else {
        return false;
    };
    let numeric_pairs = !rows.is_empty()
        && rows.iter().all(|row| {
            matches!(row, PropertyValue::Array(values) if values.len() == 2 && values.iter().all(|value| matches!(value, PropertyValue::Number(_))))
        });
    numeric_pairs
}
