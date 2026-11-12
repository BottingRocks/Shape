//! Public layout descriptors. This allowlist deliberately excludes executable
//! programs and source evidence; it is not an input to `Decoder::load`.
use super::{
    named::{Field, Node},
    SchemaDocument, WireError, WireRoute,
};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Signal names mapped to their concise layout/encoding contract.
/// See `docs/signals/schema.md` for inheritance and ordering conventions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CompactSchema(pub BTreeMap<String, Value>);

// Explicit recursive allowlist: new compiler members cannot leak by accident.
fn descriptor(v: &Value, message: bool) -> Result<Value, WireError> {
    let src = v.as_object().ok_or(WireError::Evidence)?;
    let mut out = Map::new();
    for key in [
        "type", "order", "bits", "bit", "byte", "xor", "nullable", "optional", "length", "key",
        "position", "maximum",
    ] {
        if let Some(value) = src.get(key) {
            out.insert(key.into(), value.clone());
        }
    }
    if message {
        out.remove("type");
        // Explicit source behavior has no measurement fields, empty or null.
        if src.get("behavior").and_then(Value::as_str) == Some("completion_only")
            && src.get("payload").and_then(Value::as_str) == Some("none")
        {
            out.insert("behavior".into(), json!("completion_only"));
            out.insert("payload".into(), json!("none"));
            if let Some(effect) = src.get("effect").and_then(Value::as_object) {
                out.insert(
                    "effect".into(),
                    Value::Object(
                        ["event", "windowFlag"]
                            .into_iter()
                            .filter_map(|k| {
                                effect
                                    .get(k)
                                    .filter(|v| v.is_string())
                                    .map(|v| (k.to_owned(), v.clone()))
                            })
                            .collect(),
                    ),
                );
            }
        }
    }
    // At signal/message scope xor initializes the stream. At field scope xor
    // is static XOR, so nested stream masks retain their distinct mask name.
    if message {
        if let Some(mask) = src.get("mask").filter(|v| !v.is_null()) {
            out.insert("xor".into(), mask.clone());
        }
    } else {
        match src.get("maskMode").and_then(Value::as_str) {
            None | Some("inherit") => {}
            Some(mode) => {
                out.insert("maskMode".into(), json!(mode));
                if let Some(mask) = src.get("mask").filter(|v| !v.is_null()) {
                    out.insert("mask".into(), mask.clone());
                }
            }
        }
    }
    if let Some(fields) = src.get("fields") {
        let fields = fields.as_object().ok_or(WireError::Evidence)?;
        out.insert(
            "fields".into(),
            Value::Object(
                fields
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), descriptor(v, false)?)))
                    .collect::<Result<_, WireError>>()?,
            ),
        );
    }
    for key in ["arms", "items"] {
        if let Some(values) = src.get(key) {
            out.insert(
                if key == "arms" { "cases" } else { key }.into(),
                Value::Array(
                    values
                        .as_array()
                        .ok_or(WireError::Evidence)?
                        .iter()
                        .map(|v| descriptor(v, false))
                        .collect::<Result<_, _>>()?,
                ),
            );
        }
    }
    if let Some(count) = src.get("count") {
        out.insert("count".into(), descriptor(count, false)?);
    }
    if let Some(item) = src.get("item") {
        out.insert("item".into(), descriptor(item, false)?);
    }
    if let Some(flags) = src.get("physicalFlags") {
        out.insert("physicalFlags".into(), descriptor(&flags["schema"], false)?);
    }
    logical(src, &mut out)?;
    Ok(Value::Object(out))
}

// Paths use public field names and tuple/choice indices, never source keys.
fn writes(node: &Node, path: Vec<Value>, out: &mut Vec<(usize, Vec<Value>)>) {
    match node {
        Node::Leaf { leaf, .. } => out.push((*leaf, path)),
        Node::Record { fields } | Node::ScalarNull { fields } => field_writes(fields, path, out),
        Node::FixedArray { items } => {
            for (i, f) in items.iter().enumerate() {
                let mut p = path.clone();
                p.push(json!(i));
                writes(&f.node, p, out);
            }
        }
        Node::Array { item, .. } => {
            let mut p = path;
            p.push(json!("item"));
            writes(item, p, out);
        }
        Node::Choice { arms, .. } => {
            for (i, n) in arms.iter().enumerate() {
                let mut p = path.clone();
                p.push(json!(i));
                writes(n, p, out);
            }
        }
        Node::Empty | Node::Unknown => {}
    }
}
fn field_writes(fields: &[Field], path: Vec<Value>, out: &mut Vec<(usize, Vec<Value>)>) {
    for f in fields {
        let mut p = path.clone();
        p.push(json!(f.name));
        writes(&f.node, p, out);
    }
}

impl SchemaDocument {
    /// Export the small public contract without changing the executable schema.
    pub fn compact_schema(&self) -> Result<CompactSchema, WireError> {
        let mut signals = BTreeMap::new();
        for (name, signal) in &self.signals {
            let result = (|| -> Result<(), WireError> {
                let mut raw =
                    serde_json::to_value(&signal.description).map_err(|_| WireError::Json)?;
                if signal.no_data && raw["behavior"] == "completion_only" {
                    raw.as_object_mut()
                        .ok_or(WireError::Evidence)?
                        .remove("fields");
                }
                if let Some(slot) = self.wire.as_ref().and_then(|w| {
                    w.slots
                        .iter()
                        .find(|s| s.naming.as_ref().is_some_and(|n| &n.schema_key == name))
                }) {
                    let naming = slot.naming.as_ref().unwrap();
                    let original = raw.clone();
                    let mut messages = BTreeMap::new();
                    let mut differing = std::collections::BTreeSet::new();
                    for part in &naming.parts {
                        let program = match slot.programs.get(part.emission) {
                            Some(
                                super::Support::Supported { program }
                                | super::Support::RequiresInitializer { program, .. },
                            ) => program,
                            _ => continue,
                        };
                        let rules = if naming.counted_arrays {
                            super::counted::Rules::new(program, &part.fields)
                        } else {
                            super::counted::Rules::default()
                        };
                        let mut message = original
                            .get("messages")
                            .map_or_else(|| original.clone(), |m| m[part.message].clone());
                        counted_description(&part.fields, &mut message, &rules, true)?;
                        if let Some(old) = messages.insert(part.message, message.clone()) {
                            if old != message {
                                // Different source encoders can share one logical
                                // message while proving different count recipes.
                                // Keep its original explicit fields and order;
                                // no one encoder may donate a common recipe.
                                differing.insert(part.message);
                            }
                        }
                    }
                    for (i, message) in messages {
                        if differing.contains(&i) {
                            continue;
                        }
                        if raw.get("messages").is_some() {
                            raw["messages"][i] = message;
                        } else {
                            raw = message;
                        }
                    }
                }
                let mut public = descriptor(&raw, true)?;
                match &signal.routing {
                    WireRoute::Positional { positional } => {
                        public["positionalSignal"] = json!(positional)
                    }
                    WireRoute::Name { name } => public["key"] = name.clone(),
                }
                if let Some(messages) = raw.get("messages") {
                    public["messages"] = Value::Array(
                        messages
                            .as_array()
                            .ok_or(WireError::Evidence)?
                            .iter()
                            .map(|v| descriptor(v, true))
                            .collect::<Result<_, _>>()?,
                    );
                }
                if let Some(naming) = self.wire.as_ref().and_then(|w| {
                    w.slots
                        .iter()
                        .filter_map(|s| s.naming.as_ref())
                        .find(|n| &n.schema_key == name)
                }) {
                    for part in &naming.parts {
                        let message = if public.get("messages").is_some() {
                            public["messages"]
                                .get_mut(part.message)
                                .ok_or(WireError::Evidence)?
                        } else {
                            &mut public
                        };
                        let association = json!([part.emission, part.part]);
                        let map = message.as_object_mut().ok_or(WireError::Evidence)?;
                        let emissions = map
                            .entry("emissions")
                            .or_insert_with(|| json!([]))
                            .as_array_mut()
                            .ok_or(WireError::Evidence)?;
                        if !emissions.contains(&association) {
                            emissions.push(association);
                        }
                        // Grouped tuples may move writes across intervening fields.
                        if part
                            .fields
                            .iter()
                            .any(|f| matches!(f.node, Node::FixedArray { .. }))
                        {
                            let mut order = Vec::new();
                            field_writes(&part.fields, vec![], &mut order);
                            order.sort_by_key(|(leaf, _)| *leaf);
                            let order =
                                json!(order.into_iter().map(|(_, p)| p).collect::<Vec<_>>());
                            if map.get("writeOrder").is_some_and(|old| old != &order) {
                                return Err(WireError::Evidence);
                            }
                            map.insert("writeOrder".into(), order);
                        }
                    }
                }
                fn implicit(v: &mut Value) {
                    if let Some(map) = v.as_object_mut() {
                        if map.get("emissions") == Some(&json!([[0, 0]])) {
                            map.remove("emissions");
                        }
                        if let Some(messages) =
                            map.get_mut("messages").and_then(Value::as_array_mut)
                        {
                            for m in messages {
                                implicit(m);
                            }
                        }
                    }
                }
                implicit(&mut public);
                signals.insert(name.clone(), public);
                Ok(())
            })();
            result.map_err(|e| WireError::Unsupported(format!("compact signal {name}: {e}")))?;
        }
        Ok(CompactSchema(signals))
    }
}

/// Direct logical values, with concrete tags scoped to their emission tables.
fn logical(src: &Map<String, Value>, out: &mut Map<String, Value>) -> Result<(), WireError> {
    if src.get("logicalType").and_then(Value::as_str) == Some("null") && src.get("fields").is_some()
    {
        let order = src["order"].as_array().ok_or(WireError::Evidence)?;
        let markers = order
            .iter()
            .map(|key| {
                let field = &src["fields"][key.as_str().ok_or(WireError::Evidence)?];
                let mut marker = descriptor(field, false)?;
                // A scalar-null constituent is itself a certified marker.
                if let Some(byte) = field.get("byte") {
                    marker = json!({"byte":byte});
                    for k in ["xor", "maskMode", "mask"] {
                        if let Some(v) = descriptor(field, false)?.get(k) {
                            marker[k] = v.clone();
                        }
                    }
                }
                Ok(marker)
            })
            .collect::<Result<Vec<_>, WireError>>()?;
        out.remove("fields");
        out.remove("order");
        out.insert("type".into(), json!("null"));
        out.insert("values".into(), json!({"null":markers}));
        return Ok(());
    }
    if let Some(mappings) = src.get("valueMappings") {
        let mut groups: Vec<(Value, Vec<Value>)> = vec![];
        let mut ambiguous = false;
        for entry in mappings.as_array().ok_or(WireError::Evidence)? {
            let cases: Vec<super::semantic::ByteCase> =
                serde_json::from_value(entry["mapping"]["cases"].clone())
                    .map_err(|_| WireError::Evidence)?;
            let mut values = Map::new();
            for c in cases {
                let key = match c.value {
                    super::semantic::LogicalValue::Boolean(b) => b.to_string(),
                    super::semantic::LogicalValue::Null => "null".into(),
                };
                let mut tag = json!(c.byte);
                if let Some(arms) = src.get("arms").and_then(Value::as_array) {
                    let matches: Vec<_> = arms
                        .iter()
                        .filter(|a| a["type"] == "byte" && a["byte"] == c.byte)
                        .collect();
                    if let [arm] = matches.as_slice() {
                        let encoding = descriptor(arm, false)?;
                        let mut physical = Map::from_iter([("byte".into(), json!(c.byte))]);
                        for k in ["xor", "maskMode", "mask"] {
                            if let Some(v) = encoding.get(k) {
                                physical.insert(k.into(), v.clone());
                            }
                        }
                        if physical.len() > 1 {
                            tag = Value::Object(physical);
                        }
                    } else if matches.len() > 1 {
                        // The same logical tag can occur in layouts with
                        // different XOR/masks. Keep those layouts as cases.
                        ambiguous = true;
                    }
                }
                if values.insert(key, tag).is_some() {
                    return Err(WireError::Evidence);
                }
            }
            let values = Value::Object(values);
            let emission = entry
                .get("emission")
                .filter(|v| v.is_u64())
                .ok_or(WireError::Evidence)?
                .clone();
            if let Some((_, emissions)) = groups.iter_mut().find(|(v, _)| v == &values) {
                if !emissions.contains(&emission) {
                    emissions.push(emission);
                }
            } else {
                groups.push((values, vec![emission]));
            }
        }
        // Only replace a choice when all its alternatives are scalar tags.
        let simple = !ambiguous
            && src.get("arms").is_none_or(|a| {
                a.as_array().is_some_and(|a| {
                    a.iter()
                        .all(|a| a["type"] == "byte" && a.get("byte").is_some())
                })
            });
        if !groups.is_empty() {
            if simple {
                out.remove("cases");
                out.remove("byte");
                out.insert(
                    "type".into(),
                    src.get("logicalType").cloned().ok_or(WireError::Evidence)?,
                );
            }
            if groups.len() == 1 {
                out.insert("values".into(), groups.remove(0).0);
            } else {
                out.insert(
                    "variants".into(),
                    json!(groups
                        .into_iter()
                        .map(|(values, emissions)| json!({"emissions":emissions,"values":values}))
                        .collect::<Vec<_>>()),
                );
            }
        }
    }
    // Fold identical nested alternatives; actual alternative layouts remain cases.
    if out.get("type").is_some_and(|v| v == "choice") {
        if let Some(cases) = out.get("cases").and_then(Value::as_array).cloned() {
            let optional = cases.iter().any(|v| v["type"] == "empty");
            let mut distinct = vec![];
            for c in cases.into_iter().filter(|v| v["type"] != "empty") {
                if !distinct.contains(&c) {
                    distinct.push(c);
                }
            }
            if distinct.len() == 1 {
                let child = distinct.remove(0);
                out.remove("cases");
                out.remove("type");
                for (k, v) in child.as_object().ok_or(WireError::Evidence)? {
                    out.insert(k.clone(), v.clone());
                }
                if optional {
                    out.insert("optional".into(), json!(true));
                }
            }
        }
    }
    Ok(())
}

fn counted_description(
    fields: &[Field],
    v: &mut Value,
    rules: &super::counted::Rules,
    root: bool,
) -> Result<(), WireError> {
    // Recursion follows typed naming nodes, including choice scope. No JSON name
    // matching establishes the association; names only address the proven node.
    fn node(n: &Node, v: &mut Value, rules: &super::counted::Rules) -> Result<(), WireError> {
        match n {
            Node::Record { fields } => counted_description(fields, v, rules, false)?,
            Node::Array { item, .. } => node(item, &mut v["item"], rules)?,
            Node::Choice { arms, .. } => {
                for (i, a) in arms.iter().enumerate() {
                    node(a, &mut v["arms"][i], rules)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    for f in fields {
        if let Some(value) = v.get_mut("fields").and_then(|v| v.get_mut(&f.name)) {
            node(&f.node, value, rules)?;
        }
    }
    let physical_order = v.get("order").cloned().unwrap_or(Value::Null);
    for array in fields {
        let Some(rule) = rules.array(&array.node) else {
            continue;
        };
        let count = fields
            .iter()
            .find(|f| rules.count(&f.node).is_some_and(|r| r.leaf == rule.leaf))
            .ok_or(WireError::Evidence)?;
        let position = physical_order
            .as_array()
            .and_then(|a| a.iter().position(|n| n == &count.name))
            .ok_or(WireError::Evidence)?;
        let mut encoding = v["fields"][&count.name].clone();
        encoding["position"] = json!(position);
        encoding["maximum"] = json!(rule.maximum);
        v["fields"][&array.name]["count"] = encoding;
        v["fields"]
            .as_object_mut()
            .ok_or(WireError::Evidence)?
            .remove(&count.name);
        v["order"]
            .as_array_mut()
            .ok_or(WireError::Evidence)?
            .retain(|s| s != &count.name);
    }
    if !root {
        if let Some(array) = rules.collapsed(fields) {
            *v = v["fields"][&array.name].clone();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{
        counted::{Rule, Rules},
        named::Step,
    };
    use super::*;
    fn leaf(id: usize, kind: &str) -> Node {
        Node::Leaf {
            leaf: id,
            grammar_path: vec![Step::Child(id + 1)],
            kind: kind.into(),
            mask: json!({"mode":"inherit"}),
            static_xor: None,
            byte_mapping: None,
        }
    }
    fn field(name: &str, node: Node) -> Field {
        Field {
            field_key: name.into(),
            name: name.into(),
            bits: None,
            node,
        }
    }
    #[test]
    fn counted_schema_keeps_original_number_encoding_position_scope_and_item_type() {
        let fields = vec![
            field("statistic", leaf(0, "number")),
            field("size", leaf(1, "number")),
            field(
                "strings",
                Node::Array {
                    control: vec![Step::Child(3)],
                    item: Box::new(leaf(2, "string")),
                },
            ),
        ];
        let rules = Rules(vec![Rule {
            leaf: 1,
            control: vec![Step::Child(3)],
            maximum: 65536,
        }]);
        let mut raw = json!({"order":["statistic","size","strings"],"fields":{"statistic":{"type":"number"},"size":{"type":"number","maskMode":"none","xor":17,"compilerSecret":"no"},"strings":{"type":"array","item":{"type":"string","maskMode":"nested","mask":99}}}});
        counted_description(&fields, &mut raw, &rules, true).unwrap();
        let public = descriptor(&raw, true).unwrap();
        assert_eq!(
            public,
            json!({"order":["statistic","strings"],"fields":{"statistic":{"type":"number"},"strings":{"type":"array","count":{"type":"number","maskMode":"none","xor":17,"position":1,"maximum":65536},"item":{"type":"string","maskMode":"nested","mask":99}}}})
        );
        let mut nested = json!({"type":"record","order":["size","strings"],"fields":{"size":{"type":"number","maskMode":"none"},"strings":{"type":"array","item":{"type":"string"}}}});
        counted_description(&fields[1..], &mut nested, &rules, false).unwrap();
        assert_eq!(
            descriptor(&nested, false).unwrap(),
            json!({"type":"array","count":{"type":"number","maskMode":"none","position":0,"maximum":65536},"item":{"type":"string"}})
        );
    }
}
