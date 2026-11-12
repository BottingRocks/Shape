use super::*;

pub(super) fn properties_json(
    properties: &ActualProperties,
    selected_operations: &[String],
) -> String {
    let mut values = BTreeMap::<String, PropertyValue>::new();
    let mut grouped = BTreeSet::new();
    if let Some(entry) = &properties.entry {
        let mut fields = BTreeMap::new();
        if let Some(function) = &entry.function_name {
            fields.insert("function".into(), PropertyValue::String(function.clone()));
        }
        fields.insert(
            "parameters".into(),
            PropertyValue::Array(
                entry
                    .parameters
                    .iter()
                    .cloned()
                    .map(PropertyValue::String)
                    .collect(),
            ),
        );
        // Keep actual invocation values exactly once. The fourth value is the
        // entry's special-slot argument; descriptor `initialSlots` remain in
        // their own descriptor records and are not renamed or duplicated.
        fields.insert(
            "arguments".into(),
            PropertyValue::Array(entry.arguments.iter().cloned().map(compact_value).collect()),
        );
        values.insert("entry".into(), PropertyValue::Object(fields));
    }

    let operation_names = properties
        .operations
        .iter()
        .map(|operation| operation.name.clone())
        .collect::<BTreeSet<_>>();
    for (named_values, proven_origin) in [
        (&properties.thread_descriptors, false),
        (&properties.strings, false),
        (&properties.bytecode, true),
        (&properties.dispatch, false),
        (&properties.decoders, true),
        (&properties.constants, false),
        (&properties.wasm, true),
        (&properties.additional, false),
        (&properties.unresolved, false),
    ] {
        for named in named_values {
            let value = if proven_origin {
                compact_proven_value(named.value.clone())
            } else {
                compact_value(named.value.clone())
            };
            insert_named_value(&mut values, &mut grouped, &named.name, value);
        }
    }
    for (name, value) in &properties.properties {
        // The typed map intentionally retains operation functions and helper
        // source for internal consumers. The compact view must not duplicate
        // that source under a second key.
        if !operation_names.contains(name)
            && !values.contains_key(name)
            && !matches!(value, PropertyValue::Function(_))
        {
            insert_named_value(
                &mut values,
                &mut grouped,
                name,
                compact_value(value.clone()),
            );
        }
    }
    for operation in &properties.operations {
        let mut fields = BTreeMap::new();
        fields.insert(
            "count".into(),
            PropertyValue::Number(operation.count as f64),
        );
        let selected = operation
            .keys
            .iter()
            .zip(&operation.functions)
            .filter(|(key, _)| selected_operations.iter().any(|selected| selected == *key))
            .map(|(key, source)| {
                let mut handler = BTreeMap::new();
                handler.insert("source".into(), PropertyValue::String(source.clone()));
                (key.clone(), PropertyValue::Object(handler))
            })
            .collect::<BTreeMap<_, _>>();
        if !selected.is_empty() {
            fields.insert("selected_handlers".into(), PropertyValue::Object(selected));
        }
        values.insert(operation.name.clone(), PropertyValue::Object(fields));
    }
    if !properties.slots.context.is_empty() {
        values.insert(
            "vmContext.layout".into(),
            PropertyValue::Object(compact_map(&properties.slots.context)),
        );
    }
    if !properties.slots.memory.is_empty() {
        values.insert(
            "vmMemory.layout".into(),
            PropertyValue::Object(compact_map(&properties.slots.memory)),
        );
    }
    if !properties.slots.stack.is_empty() {
        values.insert(
            "vmStack.layout".into(),
            PropertyValue::Object(compact_map(&properties.slots.stack)),
        );
    }
    if !properties.slots.initial_assignments.is_empty() {
        values.insert(
            "entry.slot_writes".into(),
            PropertyValue::Array(
                properties
                    .slots
                    .initial_assignments
                    .iter()
                    .map(compact_slot_assignment)
                    .collect(),
            ),
        );
    }
    map_json(&values)
}

fn compact_map(values: &BTreeMap<String, PropertyValue>) -> BTreeMap<String, PropertyValue> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), compact_value(value.clone())))
        .collect()
}

fn compact_slot_assignment(assignment: &SlotAssignment) -> PropertyValue {
    let mut fields = BTreeMap::new();
    fields.insert(
        "function".into(),
        PropertyValue::String(assignment.function.clone()),
    );
    fields.insert(
        "method".into(),
        PropertyValue::String(assignment.method.clone()),
    );
    fields.insert("index".into(), compact_value(assignment.index.clone()));
    fields.insert("value".into(), compact_value(assignment.value.clone()));
    PropertyValue::Object(fields)
}

fn insert_named_value(
    values: &mut BTreeMap<String, PropertyValue>,
    grouped: &mut BTreeSet<String>,
    name: &str,
    value: PropertyValue,
) {
    let Some(existing) = values.get_mut(name) else {
        values.insert(name.into(), value);
        return;
    };
    if grouped.contains(name) {
        if let PropertyValue::Array(items) = existing {
            if !items.contains(&value) {
                items.push(value);
            }
        }
    } else if *existing != value {
        let first = std::mem::replace(existing, PropertyValue::Array(Vec::new()));
        *existing = PropertyValue::Array(vec![first, value]);
        grouped.insert(name.into());
    }
}

/// Compact a value without interpreting object keys as metadata. In
/// particular, a literal object containing fields named `decoded`,
/// `expression`, or `constructor` remains that same object.
fn compact_value(value: PropertyValue) -> PropertyValue {
    match value {
        PropertyValue::Function(_) => PropertyValue::Expression("<function body omitted>".into()),
        PropertyValue::Array(values) => {
            PropertyValue::Array(values.into_iter().map(compact_value).collect())
        }
        PropertyValue::Object(fields) => PropertyValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, compact_value(value)))
                .collect(),
        ),
        other => other,
    }
}

/// Apply payload/constructor shortening only to values already classified by
/// the extractor as decoder, bytecode, or WASM output. This keeps exact
/// literal objects exact while avoiding duplicate encoded/decoded bookkeeping
/// in proven call-site records.
fn compact_proven_value(value: PropertyValue) -> PropertyValue {
    let PropertyValue::Object(fields) = value else {
        return compact_value(value);
    };

    if matches!(
        fields.get("constructor"),
        Some(PropertyValue::String(constructor)) if constructor == "WebAssembly.Module"
    ) {
        let mut module = BTreeMap::new();
        for (key, value) in fields {
            if key != "argument" {
                module.insert(key, compact_value(value));
            }
        }
        return PropertyValue::Object(module);
    }

    let Some(decoded) = fields.get("decoded") else {
        return compact_value(PropertyValue::Object(fields));
    };
    let mut payload = BTreeMap::new();
    payload.insert("bytes".into(), compact_value(decoded.clone()));
    for (key, value) in fields {
        match key.as_str() {
            // These are alternate representations of the same proven call;
            // destination is retained because it identifies the write sink.
            "decoded" | "expression" | "arguments" | "encoded" => {}
            _ => {
                payload.insert(key, compact_value(value));
            }
        }
    }
    if payload.len() == 1 {
        payload
            .remove("bytes")
            .expect("proven payload always contains bytes")
    } else {
        PropertyValue::Object(payload)
    }
}

fn json_joined<I, F>(open: &str, close: &str, values: I, encode: F) -> String
where
    I: IntoIterator,
    F: FnMut(I::Item) -> String,
{
    format!(
        "{open}{}{close}",
        values.into_iter().map(encode).collect::<Vec<_>>().join(",")
    )
}

fn map_json(values: &BTreeMap<String, PropertyValue>) -> String {
    json_joined("{", "}", values, |(key, value)| {
        format!("{}:{}", json_string(key), value_json(value))
    })
}
fn values_json(values: &[PropertyValue]) -> String {
    json_joined("[", "]", values, value_json)
}
pub(crate) fn value_json(value: &PropertyValue) -> String {
    match value {
        PropertyValue::String(value) => json_string(value),
        PropertyValue::Number(value) if !value.is_finite() && value.is_sign_positive() => {
            "{\"$number\":\"Infinity\"}".into()
        }
        PropertyValue::Number(value) if !value.is_finite() => "{\"$number\":\"-Infinity\"}".into(),
        PropertyValue::Number(value) if *value == 0.0 && value.is_sign_negative() => "-0".into(),
        PropertyValue::Number(value) if value.is_finite() => value.to_string(),
        PropertyValue::Number(_) => "{\"$number\":\"non-finite\"}".into(),
        PropertyValue::Boolean(value) => value.to_string(),
        PropertyValue::Null => "null".into(),
        PropertyValue::Undefined => "{\"$undefined\":true}".into(),
        PropertyValue::NaN => "{\"$number\":\"NaN\"}".into(),
        PropertyValue::PositiveInfinity => "{\"$number\":\"Infinity\"}".into(),
        PropertyValue::NegativeInfinity => "{\"$number\":\"-Infinity\"}".into(),
        PropertyValue::Hole => "{\"$hole\":true}".into(),
        PropertyValue::Array(values) => values_json(values),
        PropertyValue::Object(values) => map_json(values),
        PropertyValue::Function(source) => format!("{{\"$function\":{}}}", json_string(source)),
        PropertyValue::Expression(source) => format!("{{\"$expression\":{}}}", json_string(source)),
    }
}
