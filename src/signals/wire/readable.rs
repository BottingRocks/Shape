//! Shared readable projection rules. Physical fields remain intact inside tuples.
use super::{
    named::{Field, Naming, Node},
    WireError,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub(crate) struct Group {
    pub signal: &'static str,
    pub prefix: &'static str,
    pub parent: &'static str,
    pub count: usize,
    pub alphabetic: bool,
}
pub(crate) const GROUPS: &[Group] = &[
    Group {
        signal: "FingerprintApiIntegrity",
        prefix: "integrityFlags",
        parent: "integrityChecks",
        count: 11,
        alphabetic: false,
    },
    Group {
        signal: "NativeApiIntegrityFlags",
        prefix: "integrityFlags",
        parent: "integrityChecks",
        count: 3,
        alphabetic: false,
    },
    Group {
        signal: "BrowserApiProxyFlags",
        prefix: "proxyFlags",
        parent: "proxyChecks",
        count: 3,
        alphabetic: false,
    },
    Group {
        signal: "GlobalPropertyChecks",
        prefix: "presenceBits",
        parent: "presenceBits",
        count: 9,
        alphabetic: false,
    },
    Group {
        signal: "AnchorIdnaUnicodeVersion",
        prefix: "resultBits",
        parent: "resultBits",
        count: 2,
        alphabetic: false,
    },
    Group {
        signal: "GrecaptchaAccessorHookFlags",
        prefix: "hookFlags",
        parent: "hookChecks",
        count: 2,
        alphabetic: true,
    },
];
impl Group {
    pub fn old(&self, index: usize) -> String {
        if self.alphabetic {
            format!("{}{}", self.prefix, (b'A' + index as u8) as char)
        } else {
            format!("{}{}", self.prefix, index + 1)
        }
    }
}
fn error() -> WireError {
    WireError::Evidence
}
pub(crate) fn flatten(fields: &[Field]) -> bool {
    matches!(fields, [Field { bits: Some(bits), node: Node::Leaf { kind, .. }, .. }] if !bits.is_empty() && kind == "byte" && bits.values().all(|b| *b < 8))
}
// A false result refuses the Boolean tuple while retaining the proven bytes.
// Analysis deliberately leaves bits absent when flag naming is not proven.
fn group_fields(fields: &mut Vec<Field>, rule: &Group) -> Result<bool, WireError> {
    let names: Vec<_> = (0..rule.count).map(|i| rule.old(i)).collect();
    if !fields.iter().any(|f| names.contains(&f.name)) {
        return Ok(false);
    }
    if fields.iter().any(|f| f.name == rule.parent) {
        return Err(error());
    }
    let mut items = Vec::new();
    let mut keys = BTreeSet::new();
    let mut leaves = BTreeSet::new();
    for name in &names {
        let found: Vec<_> = fields.iter().filter(|f| &f.name == name).collect();
        let [field] = found.as_slice() else {
            return Err(error());
        };
        if !matches!(&field.node, Node::Leaf { kind, .. } if kind == "byte")
            || !field
                .bits
                .as_ref()
                .is_none_or(|b| !b.is_empty() && b.values().all(|b| *b < 8))
        {
            return Err(error());
        }
        let Node::Leaf { leaf, .. } = &field.node else {
            unreachable!()
        };
        if !keys.insert(&field.field_key) || !leaves.insert(*leaf) {
            return Err(error());
        }
        items.push((*field).clone());
    }
    // Check every member before refusing: a missing bit map must not hide a
    // duplicate/missing field, wrong kind, collision, or invalid present map.
    if items.iter().any(|field| field.bits.is_none()) {
        return Ok(false);
    }
    let first = fields
        .iter()
        .position(|f| names.contains(&f.name))
        .ok_or_else(error)?;
    let field_key = items[0].field_key.clone();
    fields.retain(|f| !names.contains(&f.name));
    fields.insert(
        first,
        Field {
            field_key,
            name: rule.parent.into(),
            bits: None,
            node: Node::FixedArray { items },
        },
    );
    Ok(true)
}
pub(crate) fn group(n: &mut Naming) -> Result<(), WireError> {
    if let Some(rule) = GROUPS.iter().find(|r| r.signal == n.canonical_name) {
        for part in &mut n.parts {
            group_fields(&mut part.fields, rule)?;
        }
    }
    Ok(())
}
/// Validate tuple membership and order even without source analysis. Original
/// field names, keys and leaf paths survive; tuples cannot add or alias writes.
pub(crate) fn validate(n: &Naming) -> Result<(), WireError> {
    if ![1, 2].contains(&n.version) {
        return Err(WireError::Version);
    }
    fn walk(fields: &[Field], rule: Option<&Group>, version: u32) -> Result<(), WireError> {
        if version == 2 {
            if let Some(rule) = rule {
                // Ungrouped family fields are canonical only when complete
                // byte evidence exists but a Boolean tuple cannot be proven.
                let mut grouped = fields.to_vec();
                group_fields(&mut grouped, rule)?;
                if grouped != fields {
                    return Err(error());
                }
            }
        }
        let mut names = BTreeSet::new();
        for f in fields {
            if !names.insert(&f.name) {
                return Err(error());
            }
            match &f.node {
                Node::FixedArray { items } => {
                    let r = rule.ok_or_else(error)?;
                    if version != 2
                        || f.name != r.parent
                        || f.bits.is_some()
                        || items.len() != r.count
                        || items
                            .first()
                            .is_none_or(|item| item.field_key != f.field_key)
                    {
                        return Err(error());
                    }
                    let mut keys = BTreeSet::new();
                    let mut leaves = BTreeSet::new();
                    for (i, item) in items.iter().enumerate() {
                        let Node::Leaf { leaf, kind, .. } = &item.node else {
                            return Err(error());
                        };
                        if item.name != r.old(i)
                            || kind != "byte"
                            || !keys.insert(&item.field_key)
                            || !leaves.insert(leaf)
                            || !item
                                .bits
                                .as_ref()
                                .is_some_and(|b| !b.is_empty() && b.values().all(|b| *b < 8))
                        {
                            return Err(error());
                        }
                    }
                }
                Node::Record { fields } | Node::ScalarNull { fields } => {
                    walk(fields, None, version)?
                }
                Node::Array { item, .. } => walk_node(item, version)?,
                Node::Choice { arms, .. } => {
                    for arm in arms {
                        walk_node(arm, version)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn walk_node(n: &Node, version: u32) -> Result<(), WireError> {
        match n {
            Node::Record { fields } | Node::ScalarNull { fields } => walk(fields, None, version),
            Node::Array { item, .. } => walk_node(item, version),
            Node::Choice { arms, .. } => {
                for a in arms {
                    walk_node(a, version)?;
                }
                Ok(())
            }
            Node::FixedArray { .. } => Err(error()),
            _ => Ok(()),
        }
    }
    let rule = GROUPS.iter().find(|r| r.signal == n.canonical_name);
    for p in &n.parts {
        walk(&p.fields, rule, n.version)?;
    }
    Ok(())
}
/// Convert descriptive fields with the very same tuple membership/order rule.
/// `order` is logical order; each item retains its original physical field key.
pub(crate) fn describe(n: &Naming, v: &mut Value) {
    fn message(n: &Naming, part: &super::named::Part, v: &mut Value) {
        for f in &part.fields {
            if let Node::FixedArray { items } = &f.node {
                if v["fields"].get(&f.name).is_some() {
                    continue;
                }
                let mut schemas = Vec::new();
                for item in items {
                    let mut schema = v["fields"]
                        .as_object_mut()
                        .unwrap()
                        .remove(&item.name)
                        .unwrap();
                    schema["fieldKey"] = json!(item.field_key);
                    schema["sourceName"] = json!(item.name);
                    schemas.push(schema);
                }
                let names: BTreeSet<_> = items.iter().map(|i| i.name.as_str()).collect();
                if let Some(order) = v["order"].as_array_mut() {
                    let first = order
                        .iter()
                        .position(|s| s.as_str().is_some_and(|s| names.contains(s)))
                        .unwrap();
                    order.retain(|s| !s.as_str().is_some_and(|s| names.contains(s)));
                    order.insert(first, json!(f.name));
                }
                v["fields"][&f.name] =
                    json!({"type":"fixed_array","length":items.len(),"items":schemas});
            }
        }
        if n.version >= 2 && flatten(&part.fields) {
            let f = &part.fields[0];
            if let Some(packed) = v["fields"].get(&f.name).cloned() {
                v["physicalFlags"] =
                    json!({"fieldKey":f.field_key,"sourceName":f.name,"schema":packed});
                let bits = f.bits.as_ref().unwrap();
                v["fields"] = Value::Object(
                    bits.iter()
                        .map(|(name, bit)| {
                            (
                                name.clone(),
                                json!({"type":"boolean","bit":bit,"fieldKey":f.field_key}),
                            )
                        })
                        .collect(),
                );
                v["order"] = json!(bits.keys().collect::<Vec<_>>());
            }
        }
    }
    let mut seen = BTreeSet::new();
    for part in &n.parts {
        if !seen.insert(part.message) {
            continue;
        }
        if v.get("messages").is_some() {
            if let Some(m) = v["messages"].get_mut(part.message) {
                message(n, part, m);
            }
        } else {
            message(n, part, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn packed(index: usize, rule: &Group) -> Field {
        Field {
            field_key: format!("f{}", index + 1),
            name: rule.old(index),
            bits: Some(BTreeMap::from([(format!("check{index}"), 0)])),
            node: Node::Leaf {
                leaf: index,
                grammar_path: vec![super::super::named::Step::Child(index)],
                kind: "byte".into(),
                mask: json!({"mode":"none"}),
                static_xor: None,
                byte_mapping: None,
            },
        }
    }

    #[test]
    fn tuple_grouping_refuses_incomplete_or_ambiguous_evidence_without_mutation() {
        for rule in GROUPS {
            let original: Vec<_> = (0..rule.count).map(|i| packed(i, rule)).collect();
            let mut cases = Vec::new();
            let mut empty_bits = original.clone();
            empty_bits[0].bits = Some(BTreeMap::new());
            cases.push(empty_bits);
            let mut invalid_bits = original.clone();
            invalid_bits[0]
                .bits
                .as_mut()
                .unwrap()
                .insert("outside".into(), 8);
            cases.push(invalid_bits);
            let mut duplicate = original.clone();
            duplicate.push(original[0].clone());
            cases.push(duplicate);
            let mut missing = original.clone();
            missing.pop();
            cases.push(missing);
            let mut parent = original.clone();
            let mut collision = packed(rule.count, rule);
            collision.name = rule.parent.into();
            parent.push(collision);
            cases.push(parent);
            let mut wrong_kind = original.clone();
            if let Node::Leaf { kind, .. } = &mut wrong_kind[0].node {
                *kind = "number".into();
            }
            cases.push(wrong_kind);
            for mut fields in cases {
                let before = fields.clone();
                assert!(group_fields(&mut fields, rule).is_err());
                assert_eq!(fields, before);
            }
            let mut valid = original.clone();
            valid.reverse();
            assert!(group_fields(&mut valid, rule).unwrap());
            let [Field {
                node: Node::FixedArray { items },
                ..
            }] = valid.as_slice()
            else {
                panic!("expected one source-preserving tuple")
            };
            assert_eq!(items, &original);
        }
    }

    #[test]
    fn missing_bit_names_preserve_byte_projection_and_refuse_only_the_tuple() {
        use super::super::named::Part;
        for rule in GROUPS {
            let fields: Vec<_> = (0..rule.count).map(|i| packed(i, rule)).collect();
            let mut naming = Naming {
                version: 2,
                counted_arrays: true,
                schema_key: rule.signal.into(),
                canonical_name: rule.signal.into(),
                parts: (0..2)
                    .map(|emission| Part {
                        emission,
                        part: 0,
                        message: 0,
                        stream_salt: None,
                        part_leaves: (0..rule.count).collect(),
                        split_choice: None,
                        fields: fields.clone(),
                    })
                    .collect(),
                mapping_error: None,
            };
            // Complete bit evidence must still be grouped in version 2.
            assert!(validate(&naming).is_err());
            for part in &mut naming.parts {
                part.fields[0].bits = None;
            }
            let original = naming.clone();
            group(&mut naming).unwrap();
            assert_eq!(naming, original);
            validate(&naming).unwrap();
            let saved = serde_json::to_vec(&naming).unwrap();
            let loaded: Naming = serde_json::from_slice(&saved).unwrap();
            validate(&loaded).unwrap();
            assert_eq!(loaded, original);

            // Fallback cannot hide other malformed members.
            for mutation in 0..5 {
                let mut invalid = original.clone();
                let fields = &mut invalid.parts[0].fields;
                match mutation {
                    0 => {
                        fields.pop();
                    }
                    1 => {
                        fields.push(fields[0].clone());
                    }
                    2 => {
                        fields[1].bits.as_mut().unwrap().insert("invalid".into(), 8);
                    }
                    3 => {
                        fields[1].field_key = fields[0].field_key.clone();
                    }
                    _ => {
                        fields[1].node = fields[0].node.clone();
                    }
                }
                assert!(group(&mut invalid).is_err());
                assert!(validate(&invalid).is_err());
            }
        }
    }
}
