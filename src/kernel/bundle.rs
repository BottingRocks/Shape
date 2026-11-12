//! Concise flat tracer-input bundle for a deobfuscated VM copy.
//!
//! [`extract_actual_properties`](crate::extract_actual_properties) is the typed
//! reader; this module renders the single static data document a tracer
//! consumes. The document is a **sample data configuration**, not a closure-
//! complete copy of the JavaScript runtime. In particular it carries:
//!
//! * every operation-table handler source verbatim,
//! * the exact string table and thread descriptor arrays,
//! * parsed VM context/memory/stack field roles and descriptor flags,
//! * filtered scalar/array sample constants, and
//! * the exact base64 bytecode/module payloads plus parsed WASM import/export
//!   names and linear-memory limits.
//!
//! Alias definitions, decoder bodies, helper/function sources, runtime caches,
//! native-function tables, and external global lists are deliberately absent:
//! deobfuscation replaces aliases later, so the bundle must not freeze a lossy
//! runtime closure. Handler bodies remain byte-exact, including any aliases they
//! still reference.
//!
//! ## Flat document
//!
//! The document is flat: each component is one top-level key, and actual table
//! or payload names from the seed are retained instead of being wrapped in
//! `operations`/`strings`/`threads`/`vm`/`data`/`constants` groups.
//!
//! ```text
//! {
//!   "entry":                 { "pc"?, "state"?, "initialSlots"? },
//!   "<operation table>":     { "<key>": "<handler source>" },
//!   "<string table>":        ["..."],
//!   "<descriptor table>":    [{ ... }],
//!   "vmContext":             { "<field>": field-record },
//!   "vmMemory":              { "<field>": field-record, "methods"?: ["..."] },
//!   "vmStack":               { "initial"?: [], "methods"?: ["..."] },
//!   "bytecode":              "base64",
//!   "<module>":              "base64",
//!   "<memory>":              { "initial": 3, "maximum": 3 },
//!   "moduleImports"?:        { "<module>": ["..."] },
//!   "moduleExports"?:        { "<module>": ["..."] },
//!   "<dispatch table>":      [[...]],
//!   "<constant>":            scalar-or-array,
//!   "<worker bootstrap>":    { "entryPc", "state", ... }
//! }
//! ```
//!
//! A `field-record` is one merged record per field: an initializer role
//! `{ "argument": index }` or `{ "kind": "stack" }`, a literal initializer under
//! `"initial"`, a prototype-only literal default under `"default"`, and the
//! descriptor flags `"writable"`, `"enumerable"`, and `"configurable"` in the
//! same object. Constructor initializers win over inherited prototype defaults,
//! and a prototype `value: null` is never presented as an instance initial
//! value. An unparsed initializer keeps the field name with only the flags that
//! are actually known (possibly just `{ "writable": true }`); no initial value
//! is invented. Function-valued prototype descriptors become the `methods` name
//! list rather than copied bodies.
//!
//! Empty optional components are omitted. The entry component is derived only
//! from the invocation's known positions: the second argument is the boot PC,
//! the third is dispatch state, and the fourth is the initial slot vector. Host
//! detection, parameter names, and entry backreferences are not emitted.

use std::collections::{BTreeMap, BTreeSet};

use crate::kernel::properties::{
    is_worker_bootstrap_value, value_json, ActualProperties, PropertyValue,
};
use crate::util::json_string;

/// Render the concise flat tracer-input bundle as JSON.
///
/// `selected_operations` restricts each operation-table map to those local table
/// keys; an empty slice selects every handler. Selection only drops keys from
/// the direct map and adds no shape or count metadata.
pub fn tracer_json(properties: &ActualProperties, selected_operations: &[String]) -> String {
    object_json(&document(properties, selected_operations))
}

/// Human-facing rendering of the same values. The heading matches the
/// established `kernel show --format markdown` contract and the fenced JSON is
/// byte-identical to [`tracer_json`].
pub fn tracer_markdown(properties: &ActualProperties, selected_operations: &[String]) -> String {
    let mut out = String::from("# VM properties\n\n");
    if let Some(entry) = &properties.entry {
        out.push_str("- Entry invocation:");
        if let Some(pc) = entry.arguments.get(1) {
            out.push_str(&format!(" pc `{}`", expression_or_value(pc)));
        }
        if let Some(state) = entry.arguments.get(2) {
            out.push_str(&format!(", state `{}`", expression_or_value(state)));
        }
        if let Some(slots) = entry.arguments.get(3) {
            out.push_str(&format!(", initial slots `{}`", expression_or_value(slots)));
        }
        out.push('\n');
    }
    let handlers = properties
        .operations
        .iter()
        .map(|operation| operation.count)
        .sum::<usize>();
    out.push_str(&format!(
        "- Operations: {handlers} handler(s) across {} table(s)\n",
        properties.operations.len()
    ));
    out.push_str(&format!(
        "- String tables: {}; thread descriptors: {}; base64 payloads: {}\n",
        properties.strings.len(),
        properties.thread_descriptors.len(),
        base64_payload_count(properties),
    ));
    out.push('\n');
    out.push_str("Handler sources are verbatim and still contain their original aliases; alias definitions, decoder bodies, helper sources, string caches, native-function tables, and runtime helpers are intentionally not part of this data bundle.\n");
    out.push_str("\n```json\n");
    out.push_str(&tracer_json(properties, selected_operations));
    out.push_str("\n```\n");
    out
}

fn document(
    properties: &ActualProperties,
    selected_operations: &[String],
) -> Vec<(String, PropertyValue)> {
    let mut document = Vec::new();
    let mut used = BTreeSet::new();

    if let Some(entry) = entry_value(properties) {
        push_component(&mut document, &mut used, "entry", entry);
    }

    for operation in &properties.operations {
        let mut handlers = BTreeMap::new();
        for (key, source) in operation.keys.iter().zip(&operation.functions) {
            if !selected_operations.is_empty()
                && !selected_operations.iter().any(|selected| selected == key)
            {
                continue;
            }
            handlers.insert(key.clone(), PropertyValue::String(source.clone()));
        }
        push_component(
            &mut document,
            &mut used,
            &operation.name,
            PropertyValue::Object(handlers),
        );
    }

    for named in &properties.strings {
        push_component(&mut document, &mut used, &named.name, named.value.clone());
    }
    for named in &properties.thread_descriptors {
        push_component(&mut document, &mut used, &named.name, named.value.clone());
    }

    if let Some(context) = role_value(
        &properties.slots.context,
        &properties.slots.context_prototype,
        &properties.slots.context_parameters,
        properties.slots.stack_constructor.as_deref(),
    ) {
        push_component(&mut document, &mut used, "vmContext", context);
    }
    if let Some(memory) = role_value(
        &properties.slots.memory,
        &properties.slots.memory_prototype,
        &properties.slots.memory_parameters,
        properties.slots.stack_constructor.as_deref(),
    ) {
        push_component(&mut document, &mut used, "vmMemory", memory);
    }
    if let Some(stack) = stack_value(properties) {
        push_component(&mut document, &mut used, "vmStack", stack);
    }

    if let Some(bytecode) = bytecode_value(properties) {
        push_component(&mut document, &mut used, "bytecode", bytecode);
    }

    let mut imports = BTreeMap::new();
    let mut exports = BTreeMap::new();
    for named in &properties.wasm {
        let PropertyValue::Object(fields) = &named.value else {
            continue;
        };
        let Some(PropertyValue::String(constructor)) = fields.get("constructor") else {
            continue;
        };
        match constructor.as_str() {
            "WebAssembly.Module" => {
                let Some(encoded) = module_bytes(fields.get("argument"), properties) else {
                    continue;
                };
                push_component(
                    &mut document,
                    &mut used,
                    &named.name,
                    PropertyValue::String(encoded.clone()),
                );
                if let Some((module_imports, module_exports)) = module_summary(&encoded) {
                    if !module_imports.is_empty() {
                        imports.insert(named.name.clone(), strings_value(&module_imports));
                    }
                    if !module_exports.is_empty() {
                        exports.insert(named.name.clone(), strings_value(&module_exports));
                    }
                }
            }
            "WebAssembly.Memory" => {
                let Some(PropertyValue::Object(memory)) = fields.get("argument") else {
                    continue;
                };
                let mut limits = BTreeMap::new();
                for key in ["initial", "maximum"] {
                    if let Some(value) = memory.get(key) {
                        limits.insert(key.into(), value.clone());
                    }
                }
                if !limits.is_empty() {
                    push_component(
                        &mut document,
                        &mut used,
                        &named.name,
                        PropertyValue::Object(limits),
                    );
                }
            }
            _ => {}
        }
    }
    if !imports.is_empty() {
        push_component(
            &mut document,
            &mut used,
            "moduleImports",
            PropertyValue::Object(imports),
        );
    }
    if !exports.is_empty() {
        push_component(
            &mut document,
            &mut used,
            "moduleExports",
            PropertyValue::Object(exports),
        );
    }

    for named in &properties.dispatch {
        push_component(&mut document, &mut used, &named.name, named.value.clone());
    }
    for named in &properties.constants {
        if is_meaningful_constant(&named.value) {
            push_component(&mut document, &mut used, &named.name, named.value.clone());
        }
    }
    for named in &properties.additional {
        if is_worker_bootstrap_value(&named.value) {
            push_component(&mut document, &mut used, &named.name, named.value.clone());
        }
    }

    document
}

/// Add one top-level component unless an earlier component already claimed the
/// exact seed-provided name. The first owner wins; names are never renamed or
/// invented to force a collision away.
fn push_component(
    document: &mut Vec<(String, PropertyValue)>,
    used: &mut BTreeSet<String>,
    name: &str,
    value: PropertyValue,
) {
    if used.insert(name.to_string()) {
        document.push((name.to_string(), value));
    }
}

/// Minimal startup invocation values. Only the known argument positions are
/// used; unknown/expression values are omitted rather than replaced with a
/// fabricated boot state.
fn entry_value(properties: &ActualProperties) -> Option<PropertyValue> {
    let entry = properties.entry.as_ref()?;
    let mut fields = BTreeMap::new();
    if let Some(pc) = entry
        .arguments
        .get(1)
        .filter(|value| is_known_scalar(value))
    {
        fields.insert("pc".into(), pc.clone());
    }
    if let Some(state) = entry
        .arguments
        .get(2)
        .filter(|value| is_known_scalar(value))
    {
        fields.insert("state".into(), state.clone());
    }
    if let Some(slots) = entry
        .arguments
        .get(3)
        .filter(|value| is_known_slot_vector(value))
    {
        fields.insert("initialSlots".into(), slots.clone());
    }
    (!fields.is_empty()).then_some(PropertyValue::Object(fields))
}

fn is_known_scalar(value: &PropertyValue) -> bool {
    matches!(
        value,
        PropertyValue::String(_)
            | PropertyValue::Number(_)
            | PropertyValue::Boolean(_)
            | PropertyValue::Null
    )
}

fn is_known_slot_vector(value: &PropertyValue) -> bool {
    match value {
        PropertyValue::Array(values) => values.iter().all(is_known_scalar),
        _ => false,
    }
}

/// Merge constructor initializers and prototype descriptors into one record per
/// field. A field appears once even when it has both an initializer role and
/// descriptor flags.
fn role_value(
    initializers: &BTreeMap<String, PropertyValue>,
    prototypes: &BTreeMap<String, PropertyValue>,
    parameters: &[String],
    stack_constructor: Option<&str>,
) -> Option<PropertyValue> {
    let mut role = BTreeMap::new();
    let mut methods = Vec::new();
    let mut names = initializers
        .keys()
        .chain(prototypes.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    names.remove("methods");

    for name in &names {
        let mut record = BTreeMap::new();
        let mut include_field = initializers.contains_key(name);

        if let Some(value) = initializers.get(name) {
            match value {
                PropertyValue::Expression(source)
                    if stack_constructor
                        .is_some_and(|constructor| source.trim() == format!("{constructor}()")) =>
                {
                    record.insert("kind".into(), PropertyValue::String("stack".into()));
                }
                PropertyValue::Expression(source) => {
                    if let Some(index) = parameters
                        .iter()
                        .position(|parameter| parameter == source.trim())
                    {
                        record.insert("argument".into(), PropertyValue::Number(index as f64));
                    }
                }
                value if is_data_value(value) => {
                    record.insert("initial".into(), value.clone());
                }
                _ => {}
            }
        }

        if let Some(PropertyValue::Object(fields)) = prototypes.get(name) {
            if is_descriptor_only(fields) {
                if let Some(value) = fields.get("value") {
                    if matches!(
                        value,
                        PropertyValue::Function(_) | PropertyValue::Expression(_)
                    ) {
                        include_field &= initializers.contains_key(name);
                        push_method(&mut methods, name);
                    } else if is_data_value(value)
                        && !matches!(value, PropertyValue::Null)
                        && !record.contains_key("initial")
                    {
                        // A prototype-only literal is a default, never an
                        // instance initial value. A prototype `value: null`
                        // is explicitly not an initial value and is omitted.
                        record.insert("default".into(), value.clone());
                        include_field = true;
                    }
                }
                for key in ["writable", "enumerable", "configurable"] {
                    if let Some(value) = fields.get(key) {
                        if is_data_value(value) {
                            record.insert(key.into(), value.clone());
                            include_field = true;
                        }
                    }
                }
            }
        }

        if include_field {
            role.insert(name.clone(), PropertyValue::Object(record));
        }
    }

    if !methods.is_empty() {
        role.insert(
            "methods".into(),
            PropertyValue::Array(methods.into_iter().map(PropertyValue::String).collect()),
        );
    }
    (!role.is_empty()).then_some(PropertyValue::Object(role))
}

/// The stack role is a literal array with known prototype methods; it has no
/// constructor parameter roles. `initial` comes from the proven stack
/// constructor's returned literal, not from a prototype `value: null`.
fn stack_value(properties: &ActualProperties) -> Option<PropertyValue> {
    let mut stack = BTreeMap::new();
    if let Some(initial) = &properties.slots.stack_initial {
        if is_data_value(initial) {
            stack.insert("initial".into(), initial.clone());
        }
    }
    if let Some(PropertyValue::Array(values)) = properties.slots.stack.get("methods") {
        let methods = values
            .iter()
            .filter_map(|value| match value {
                PropertyValue::String(method) => Some(method.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !methods.is_empty() {
            stack.insert(
                "methods".into(),
                PropertyValue::Array(methods.into_iter().map(PropertyValue::String).collect()),
            );
        }
    }
    (!stack.is_empty()).then_some(PropertyValue::Object(stack))
}

fn push_method(methods: &mut Vec<String>, method: &str) {
    if !methods.iter().any(|existing| existing == method) {
        methods.push(method.to_string());
    }
}

/// Keep only static data constants. Native-function arrays, caches, alias
/// tables, and redundant descriptor objects are runtime plumbing, not sample
/// data; numeric bytecode/dispatch tables and string/number scalars remain.
fn is_meaningful_constant(value: &PropertyValue) -> bool {
    match value {
        PropertyValue::Function(_) | PropertyValue::Expression(_) => false,
        PropertyValue::Array(values) => values.iter().all(is_data_value),
        PropertyValue::Object(fields) => {
            !fields.is_empty() && !is_descriptor_only(fields) && fields.values().all(is_data_value)
        }
        _ => true,
    }
}

fn is_data_value(value: &PropertyValue) -> bool {
    match value {
        PropertyValue::Function(_) | PropertyValue::Expression(_) => false,
        PropertyValue::Array(values) => values.iter().all(is_data_value),
        PropertyValue::Object(fields) => {
            !fields.is_empty() && !is_descriptor_only(fields) && fields.values().all(is_data_value)
        }
        _ => true,
    }
}

fn is_descriptor_only(fields: &BTreeMap<String, PropertyValue>) -> bool {
    !fields.is_empty()
        && fields.keys().all(|key| {
            matches!(
                key.as_str(),
                "value" | "writable" | "enumerable" | "configurable"
            )
        })
}

fn bytecode_value(properties: &ActualProperties) -> Option<PropertyValue> {
    let mut bytecodes = Vec::new();
    for named in properties
        .unproven_decoder_calls
        .iter()
        .chain(&properties.bytecode)
        .chain(&properties.decoders)
    {
        if let Some(encoded) = payload_string(&named.value) {
            if !bytecodes.contains(&encoded) {
                bytecodes.push(encoded);
            }
        }
    }
    match bytecodes.len() {
        0 => None,
        1 => Some(PropertyValue::String(bytecodes.remove(0))),
        _ => Some(PropertyValue::Array(
            bytecodes.into_iter().map(PropertyValue::String).collect(),
        )),
    }
}

fn payload_string(value: &PropertyValue) -> Option<String> {
    match value {
        PropertyValue::String(encoded) => Some(encoded.clone()),
        PropertyValue::Object(fields) => match fields.get("encoded") {
            Some(PropertyValue::String(encoded)) => Some(encoded.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Resolve a module argument to its exact base64 literal. Named inputs are
/// followed statically through the extractor's typed binding map; no call is
/// executed and no decoder body is emitted.
fn module_bytes(argument: Option<&PropertyValue>, properties: &ActualProperties) -> Option<String> {
    match argument? {
        PropertyValue::String(encoded) => Some(encoded.clone()),
        PropertyValue::Expression(source) => resolve_expression_bytes(source, properties),
        PropertyValue::Object(fields) => match fields.get("encoded") {
            Some(PropertyValue::String(encoded)) => Some(encoded.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn resolve_expression_bytes(source: &str, properties: &ActualProperties) -> Option<String> {
    let source = source.trim();
    if let Some((_callee, encoded)) = strict_decoder_literal(source) {
        return Some(encoded);
    }
    if let Some(encoded) = string_literal(source) {
        return Some(encoded);
    }
    if !is_identifier(source) {
        return None;
    }
    match properties.properties.get(source)? {
        PropertyValue::String(encoded) => Some(encoded.clone()),
        PropertyValue::Expression(expression) => {
            strict_decoder_literal(expression).map(|(_, encoded)| encoded)
        }
        PropertyValue::Object(fields) => match fields.get("encoded") {
            Some(PropertyValue::String(encoded)) => Some(encoded.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn module_summary(encoded: &str) -> Option<(Vec<String>, Vec<String>)> {
    let bytes = crate::decode_base64(encoded).ok()?;
    crate::kernel::properties::wasm_summary(&bytes)
}

fn base64_payload_count(properties: &ActualProperties) -> usize {
    let mut count = properties
        .unproven_decoder_calls
        .iter()
        .chain(&properties.bytecode)
        .chain(&properties.decoders)
        .filter(|named| payload_string(&named.value).is_some())
        .count();
    for named in &properties.wasm {
        if let PropertyValue::Object(fields) = &named.value {
            if matches!(fields.get("constructor"), Some(PropertyValue::String(constructor)) if constructor == "WebAssembly.Module")
                && module_bytes(fields.get("argument"), properties).is_some()
            {
                count += 1;
            }
        }
    }
    count
}

fn expression_or_value(value: &PropertyValue) -> String {
    match value {
        PropertyValue::String(value) => json_string(value),
        PropertyValue::Number(value) => value.to_string(),
        PropertyValue::Boolean(value) => value.to_string(),
        PropertyValue::Null => "null".into(),
        PropertyValue::Undefined => "undefined".into(),
        PropertyValue::Expression(value) | PropertyValue::Function(value) => value.clone(),
        other => value_json(other),
    }
}

/// A `decoder("literal")` expression, matched strictly: one identifier callee
/// and one double-quoted literal whose contents cannot end the string early.
/// Arbitrary expressions are never split on their first/last quote.
fn strict_decoder_literal(source: &str) -> Option<(String, String)> {
    let source = source.trim();
    let open = source.find('(')?;
    let callee = source.get(..open)?;
    if !is_identifier(callee) || !source.ends_with(')') {
        return None;
    }
    let argument = source.get(open + 1..source.len() - 1)?.trim();
    string_literal(argument).map(|literal| (callee.to_string(), literal))
}

fn string_literal(source: &str) -> Option<String> {
    let literal = source.trim().strip_prefix('"')?.strip_suffix('"')?;
    if literal.contains('"') || literal.contains('\\') {
        return None;
    }
    literal
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '='))
        .then(|| literal.to_string())
}

fn is_identifier(source: &str) -> bool {
    let mut characters = source.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '$'
        })
}

fn strings_value(values: &[String]) -> PropertyValue {
    PropertyValue::Array(values.iter().cloned().map(PropertyValue::String).collect())
}

fn object_json(entries: &[(String, PropertyValue)]) -> String {
    let mut out = String::from("{");
    for (index, (key, value)) in entries.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&json_string(key));
        out.push(':');
        out.push_str(&value_json(value));
    }
    out.push('}');
    out
}
