//! Names are a source-validated projection, never executable wire input.
use super::{DecodedSlot, Outcome, WireError, WireRoute, WireValue};
use crate::signals::{
    analysis::SlotSummary,
    extract::contracts::{GrammarNode, SignalContract},
    payload::Report,
    schema::{FieldBinding, LayoutField, LayoutNode, LeafKind, LeafMask},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

type Path = Vec<Step>;
/// Reserve canonical spellings before allocating duplicate-instance suffixes.
pub(crate) fn emitted_key(
    canonical: &str,
    count: &mut usize,
    reserved: &BTreeSet<&str>,
    used: &Map<String, Value>,
) -> String {
    *count += 1;
    let mut key = canonical.to_string();
    if *count > 1 {
        key = format!("{canonical}_{count}");
        while reserved.contains(key.as_str()) || used.contains_key(&key) {
            *count += 1;
            key = format!("{canonical}_{count}");
        }
    }
    key
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Child(usize),
    Arm(usize),
    Body,
}

/// Versioned source-local mapping. Leaf ordinals count static writes, including
/// every choice arm and one repeat template, independently of runtime counts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Naming {
    pub version: u32,
    /// Shared source-proof presentation rule; absent on older artifacts.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub counted_arrays: bool,
    pub schema_key: String,
    pub canonical_name: String,
    pub parts: Vec<Part>,
    pub mapping_error: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Part {
    pub emission: usize,
    pub part: usize,
    pub message: usize,
    pub stream_salt: Option<i64>,
    pub part_leaves: Vec<usize>,
    pub split_choice: Option<Path>,
    pub fields: Vec<Field>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Field {
    pub field_key: String,
    pub name: String,
    pub bits: Option<BTreeMap<String, u32>>,
    pub node: Node,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case", deny_unknown_fields)]
pub enum Node {
    Leaf {
        leaf: usize,
        grammar_path: Path,
        kind: String,
        mask: Value,
        #[serde(default, deserialize_with = "optional_f64")]
        static_xor: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        byte_mapping: Option<super::semantic::ByteMapping>,
    },
    Record {
        fields: Vec<Field>,
    },
    /// Synthetic heterogeneous tuple, independent of physical write order.
    FixedArray {
        items: Vec<Field>,
    },
    /// Certified scalar null whose physical representation has several leaves.
    ScalarNull {
        fields: Vec<Field>,
    },
    Array {
        control: Path,
        item: Box<Node>,
    },
    Choice {
        control: Path,
        arms: Vec<Node>,
    },
    Empty,
    Unknown,
}

pub(super) fn contract_address(contract: &SignalContract) -> String {
    use crate::signals::extract::contracts::ProducerKind;
    match contract.producer.kind {
        ProducerKind::Grouped {
            group, position, ..
        } => format!("wave:{group}:{position}"),
        ProducerKind::Direct { .. } => format!("direct:{}", contract.descriptor),
        ProducerKind::Unrouted => format!("unrouted:{}", contract.descriptor),
    }
}

struct Control {
    path: Path,
    repeat: bool,
    leaves: Vec<usize>,
}
#[derive(Default)]
struct Static {
    paths: Vec<Path>,
    kinds: Vec<LeafKind>,
    controls: Vec<Control>,
}
impl Static {
    fn walk(&mut self, node: &GrammarNode, path: Path) {
        let start = self.paths.len();
        match node {
            GrammarNode::Byte(_) | GrammarNode::Number(_) | GrammarNode::String(_) => {
                self.kinds.push(match node {
                    GrammarNode::Byte(_) => LeafKind::Byte,
                    GrammarNode::Number(_) => LeafKind::Number,
                    GrammarNode::String(_) => LeafKind::String,
                    _ => unreachable!(),
                });
                self.paths.push(path)
            }
            GrammarNode::Sequence { items } => {
                for (i, n) in items.iter().enumerate() {
                    self.walk(n, extended(&path, Step::Child(i)));
                }
            }
            GrammarNode::Repeat { body, .. } => {
                self.walk(body, extended(&path, Step::Body));
                self.controls.push(Control {
                    path,
                    repeat: true,
                    leaves: (start..self.paths.len()).collect(),
                });
            }
            GrammarNode::Choice { arms, .. } => {
                for (i, arm) in arms.iter().enumerate() {
                    self.walk(&arm.body, extended(&path, Step::Arm(i)));
                }
                self.controls.push(Control {
                    path,
                    repeat: false,
                    leaves: (start..self.paths.len()).collect(),
                });
            }
            _ => {}
        }
    }
}
fn extended(path: &Path, step: Step) -> Path {
    let mut p = path.clone();
    p.push(step);
    p
}
fn mapping() -> WireError {
    WireError::Unsupported("source projection cannot map this physical tree".into())
}

/// Normalize readable finite numbers only. The upper bounds are strict because
/// MAX as f64 rounds up to 2^63 / 2^64; inclusive bounds would saturate casts.
fn finite_number(n: f64) -> Value {
    if n.fract() == 0.0 {
        if n >= i64::MIN as f64 && n < i64::MAX as f64 {
            return json!(n as i64);
        }
        if n >= 0.0 && n < u64::MAX as f64 {
            return json!(n as u64);
        }
    }
    json!(n)
}
fn optional_f64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    let v = Option::<Value>::deserialize(d)?;
    v.map(|v| {
        v.as_f64()
            .ok_or_else(|| serde::de::Error::custom("expected finite numeric operand"))
    })
    .transpose()
}

pub(crate) fn validate_leaf_paths(
    naming: &Naming,
    contract: &SignalContract,
) -> Result<(), WireError> {
    fn byte_field(field: &Field, fields: &[LayoutField], part: &Part) -> Result<(), WireError> {
        let (original, offset) = crate::signals::schema::find_key(fields, &field.field_key)
            .ok_or(WireError::Evidence)?;
        let Node::Leaf { leaf, .. } = &field.node else {
            return Err(WireError::Evidence);
        };
        if !matches!(original,
            LayoutNode::Leaf(l) | LayoutNode::ByteSelect { leaf: l, .. }
                if l.kind == LeafKind::Byte)
            || part.part_leaves.get(offset) != Some(leaf)
        {
            return Err(WireError::Evidence);
        }
        Ok(())
    }
    fn walk(n: &Node, facts: &Static, ids: &mut BTreeSet<usize>) -> Result<(), WireError> {
        match n {
            Node::Leaf {
                leaf,
                grammar_path,
                kind,
                ..
            } => {
                if facts.paths.get(*leaf) != Some(grammar_path)
                    || !ids.insert(*leaf)
                    || !matches!(
                        (kind.as_str(), facts.kinds.get(*leaf)),
                        ("value", _)
                            | ("byte", Some(LeafKind::Byte))
                            | ("number", Some(LeafKind::Number))
                            | ("string", Some(LeafKind::String))
                    )
                {
                    return Err(WireError::Evidence);
                }
            }
            Node::Record { fields }
            | Node::ScalarNull { fields }
            | Node::FixedArray { items: fields } => {
                for f in fields {
                    walk(&f.node, facts, ids)?;
                }
            }
            Node::Array { item, .. } => walk(item, facts, ids)?,
            Node::Choice { arms, .. } => {
                for arm in arms {
                    walk(arm, facts, ids)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let layout = crate::signals::schema::project_contract(contract);
    let rule = super::readable::GROUPS
        .iter()
        .find(|r| r.signal == naming.canonical_name);
    for part in &naming.parts {
        let mut facts = Static::default();
        facts.walk(
            &contract
                .emissions
                .get(part.emission)
                .ok_or(WireError::Evidence)?
                .grammar,
            vec![],
        );
        let mut ids = BTreeSet::new();
        for f in &part.fields {
            walk(&f.node, &facts, &mut ids)?;
            if let Node::FixedArray { items } = &f.node {
                let fields = &layout
                    .messages
                    .get(part.message)
                    .ok_or(WireError::Evidence)?
                    .fields;
                for item in items {
                    byte_field(item, fields, part)?;
                }
            } else if rule.is_some_and(|r| (0..r.count).any(|i| f.name == r.old(i))) {
                // Ungrouped family bytes need the same physical association as
                // tuple members, independently of optional flag-name maps.
                let fields = &layout
                    .messages
                    .get(part.message)
                    .ok_or(WireError::Evidence)?
                    .fields;
                byte_field(f, fields, part)?;
            }
        }
        if ids.len() != part.part_leaves.len() || ids != part.part_leaves.iter().copied().collect()
        {
            return Err(WireError::Evidence);
        }
    }
    Ok(())
}
fn leaves(node: &LayoutNode) -> usize {
    match node {
        LayoutNode::Leaf(_) | LayoutNode::ByteSelect { .. } => 1,
        LayoutNode::Array { item } => leaves(item),
        LayoutNode::Record { fields } => fields.iter().map(|f| leaves(&f.node)).sum(),
        LayoutNode::Choice { arms } => arms.iter().map(|a| leaves(&a.node)).sum(),
        _ => 0,
    }
}
struct Projection<'a> {
    ids: &'a [usize],
    at: usize,
    facts: &'a Static,
    binding: &'a FieldBinding,
    message: usize,
}
impl Projection<'_> {
    fn fields(
        &mut self,
        fields: &[LayoutField],
        parents: &[Path],
    ) -> Result<Vec<Field>, WireError> {
        let mut seen = BTreeSet::new();
        fields
            .iter()
            .map(|field| {
                let name = self
                    .binding
                    .get(self.message, &field.key)
                    .ok_or_else(mapping)?
                    .to_string();
                if !seen.insert(name.clone()) {
                    return Err(mapping());
                }
                Ok(Field {
                    field_key: field.key.clone(),
                    name,
                    bits: self.binding.flags(self.message, &field.key).cloned(),
                    node: self.node(&field.node, parents)?,
                })
            })
            .collect()
    }
    fn node(&mut self, node: &LayoutNode, parents: &[Path]) -> Result<Node, WireError> {
        Ok(match node {
            LayoutNode::Leaf(leaf) | LayoutNode::ByteSelect { leaf, .. } => {
                let id = *self.ids.get(self.at).ok_or_else(mapping)?;
                self.at += 1;
                if leaf.kind != LeafKind::Value && self.facts.kinds.get(id) != Some(&leaf.kind) {
                    return Err(WireError::Evidence);
                }
                let mask = match leaf.mask {
                    LeafMask::Inherit => json!({"mode":"inherit"}),
                    LeafMask::None => json!({"mode":"none"}),
                    LeafMask::Nested(salt) => json!({"mode":"nested","salt":salt}),
                    LeafMask::Unresolved => json!({"mode":"unresolved"}),
                };
                Node::Leaf {
                    leaf: id,
                    grammar_path: self.facts.paths.get(id).ok_or_else(mapping)?.clone(),
                    kind: match leaf.kind {
                        LeafKind::Byte => "byte",
                        LeafKind::Number => "number",
                        LeafKind::String => "string",
                        LeafKind::Value => "value",
                    }
                    .into(),
                    mask,
                    static_xor: leaf.xor,
                    byte_mapping: None,
                }
            }
            LayoutNode::Record { fields } => Node::Record {
                fields: self.fields(fields, parents)?,
            },
            LayoutNode::Array { .. } | LayoutNode::Choice { .. } => {
                let ids = self
                    .ids
                    .get(self.at..self.at + leaves(node))
                    .ok_or_else(mapping)?;
                let repeat = matches!(node, LayoutNode::Array { .. });
                let mut candidates: Vec<_> = self
                    .facts
                    .controls
                    .iter()
                    .filter(|c| {
                        c.repeat == repeat
                            && c.leaves == ids
                            && !parents.contains(&c.path)
                            && parents.last().is_none_or(|p| c.path.starts_with(p))
                    })
                    .collect();
                candidates.sort_by_key(|c| c.path.len());
                let control = candidates.first().ok_or_else(mapping)?.path.clone();
                if candidates
                    .get(1)
                    .is_some_and(|c| c.path.len() == control.len())
                {
                    return Err(mapping());
                }
                let mut parents = parents.to_vec();
                parents.push(control.clone());
                match node {
                    LayoutNode::Array { item } => Node::Array {
                        control,
                        item: Box::new(self.node(item, &parents)?),
                    },
                    LayoutNode::Choice { arms } => Node::Choice {
                        control,
                        arms: arms
                            .iter()
                            .map(|a| self.node(&a.node, &parents))
                            .collect::<Result<_, _>>()?,
                    },
                    _ => unreachable!(),
                }
            }
            LayoutNode::Empty => Node::Empty,
            LayoutNode::Unknown { .. } => Node::Unknown,
        })
    }
}
pub(super) fn from_slot(
    slot: &SlotSummary,
    contract: &SignalContract,
) -> Result<Option<Naming>, WireError> {
    let (Some(key), Some(binding), [canonical]) =
        (&slot.emitted_as, &slot.binding, slot.candidates.as_slice())
    else {
        return Ok(None);
    };
    let result = build_parts(slot, contract, binding);
    let (parts, mapping_error) = match result {
        Ok(parts) => (parts, None),
        Err(WireError::Unsupported(reason)) => (vec![], Some(reason)),
        Err(error) => return Err(error),
    };
    let mut naming = Naming {
        version: 2,
        counted_arrays: true,
        schema_key: key.clone(),
        canonical_name: canonical.to_string(),
        parts,
        mapping_error,
    };
    super::semantic::bind(&mut naming, contract);
    super::readable::group(&mut naming)?;
    Ok(Some(naming))
}
fn build_parts(
    slot: &SlotSummary,
    contract: &SignalContract,
    binding: &FieldBinding,
) -> Result<Vec<Part>, WireError> {
    let mut parts = Vec::new();
    for emission in &slot.layout.emissions {
        let grammar = &contract
            .emissions
            .get(emission.emission)
            .ok_or(WireError::Evidence)?
            .grammar;
        let mut facts = Static::default();
        facts.walk(grammar, vec![]);
        if !emission.covers(facts.paths.len()) {
            return Err(WireError::Evidence);
        }
        for (part, (_, layout)) in emission.parts.iter().enumerate() {
            let message = slot
                .layout
                .part_message(emission.emission, part)
                .ok_or(WireError::Evidence)?;
            let ids = emission.part_leaves(part);
            let mut projection = Projection {
                ids,
                at: 0,
                facts: &facts,
                binding,
                message,
            };
            let fields = projection.fields(&layout.fields, &[])?;
            if projection.at != ids.len() {
                return Err(WireError::Evidence);
            }
            let split_choice = if emission.parts.len() > 1 {
                // project_contract only splits its first top-level choice.
                // Follow that exact structural rule, never a label or salt.
                let (head, path) = match grammar {
                    GrammarNode::Sequence { items } => (
                        items.first().ok_or(WireError::Evidence)?,
                        vec![Step::Child(0)],
                    ),
                    other => (other, vec![]),
                };
                let GrammarNode::Choice { arms, .. } = head else {
                    return Err(WireError::Evidence);
                };
                if arms.len() != emission.parts.len() {
                    return Err(WireError::Evidence);
                }
                Some(path)
            } else {
                None
            };
            parts.push(Part {
                emission: emission.emission,
                part,
                message,
                stream_salt: layout.mask,
                part_leaves: ids.to_vec(),
                split_choice,
                fields,
            });
        }
    }
    Ok(parts)
}

/// Strict parsing rejects duplicates at every object depth before typed serde
/// can discard them (including unknown descriptive and metadata members).
pub(crate) fn strict_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, WireError> {
    use serde::de::{self, MapAccess, SeqAccess, Visitor};
    struct Check;
    impl<'de> Deserialize<'de> for Check {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            d.deserialize_any(Self)
        }
    }
    impl<'de> Visitor<'de> for Check {
        type Value = Self;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("JSON without duplicate keys")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self, A::Error> {
            let mut seen = BTreeSet::new();
            while let Some(key) = a.next_key::<String>()? {
                if !seen.insert(key) {
                    return Err(de::Error::custom("duplicate JSON member"));
                }
                a.next_value::<Check>()?;
            }
            Ok(Self)
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self, A::Error> {
            while a.next_element::<Check>()?.is_some() {}
            Ok(Self)
        }
        fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self, E> {
            Ok(Self)
        }
        fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self, E> {
            Ok(Self)
        }
        fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self, E> {
            Ok(Self)
        }
        fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self, E> {
            Ok(Self)
        }
        fn visit_str<E: de::Error>(self, _: &str) -> Result<Self, E> {
            Ok(Self)
        }
        fn visit_unit<E: de::Error>(self) -> Result<Self, E> {
            Ok(Self)
        }
    }
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(WireError::Bounds);
    }
    serde_json::from_slice::<Check>(bytes).map_err(|_| WireError::Json)?;
    serde_json::from_slice(bytes).map_err(|_| WireError::Json)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamedStatus {
    Decoded,
    Missing,
    Unsupported,
    Invalid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedSignal {
    pub fields: Option<Map<String, Value>>,
    pub status: NamedStatus,
    pub metadata: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedDocument {
    pub format_version: u32,
    pub signals: BTreeMap<String, NamedSignal>,
    pub unresolved_slots: Vec<NamedSignal>,
    pub metadata: RoundTrip,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoundTrip {
    #[serde(default)]
    pub decoder_options: super::TypedDecoderOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization: Option<super::InitializationCensus>,
    pub original_report: Report,
}
impl NamedDocument {
    pub fn from_json(bytes: &[u8]) -> Result<Self, WireError> {
        let doc: Self = strict_json(bytes)?;
        if doc.format_version != 1 {
            return Err(WireError::Version);
        }
        Ok(doc)
    }
    /// Restores retained bytes/channels only. Display-field edits deliberately
    /// do not participate; use Decoder::encode_slot for physical value edits.
    pub fn restore_original_report(&self) -> Report {
        self.metadata.original_report.clone()
    }
}

#[derive(Default)]
struct Runtime<'a> {
    counted: super::counted::Rules,
    root_prefix_arms: BTreeSet<usize>,
    packed_choices: BTreeSet<Path>,
    leaves: BTreeMap<(usize, Vec<usize>), &'a WireValue>,
    controls: BTreeMap<(Path, Vec<usize>), usize>,
    order: Vec<(usize, Vec<usize>)>,
    physical: BTreeMap<(usize, Vec<usize>), Value>,
    /// Skip per-leaf provenance details (fields-only projection). The default
    /// records them.
    fields_only: bool,
}
fn count(node: &GrammarNode) -> usize {
    let mut n = 0;
    node.walk(&mut |n0| {
        if matches!(
            n0,
            GrammarNode::Byte(_) | GrammarNode::Number(_) | GrammarNode::String(_)
        ) {
            n += 1
        }
    });
    n
}
impl<'a> Runtime<'a> {
    fn packing(&mut self, op: &super::Op) {
        use super::{
            control::{ControlStep, CounterInput},
            Op,
        };
        match op {
            Op::CounterChoice { proof, arms } => {
                if matches!(proof.input, CounterInput::Packed { .. })
                    && matches!(arms.as_slice(), [Op::Byte { .. }, Op::Sequence { items }] if items.is_empty())
                {
                    self.packed_choices.insert(
                        proof
                            .path
                            .iter()
                            .map(|step| match step {
                                ControlStep::Child(i) => Step::Child(*i),
                                ControlStep::Arm(i) => Step::Arm(*i),
                                ControlStep::Body => Step::Body,
                            })
                            .collect(),
                    );
                }
                for arm in arms {
                    self.packing(arm);
                }
            }
            Op::Sequence { items } => {
                for item in items {
                    self.packing(item);
                }
            }
            Op::Repeat { body, .. } | Op::CountedRepeat { body, .. } => self.packing(body),
            Op::Choice { arms, .. } | Op::DomainChoice { arms, .. } => {
                for arm in arms {
                    self.packing(arm);
                }
            }
            Op::RootChoice { cases } => {
                for case in cases {
                    if let super::Support::Supported { program } = &*case.support {
                        self.packing(&program.body);
                    }
                }
            }
            _ => {}
        }
    }

    /// A packed loop retains every physical iteration. Only the compiler's
    /// certified flush/empty choices may contribute no byte to a flag group.
    fn flag_bytes(
        &self,
        node: &Node,
        ctx: &[usize],
        details: &mut Vec<Value>,
    ) -> Result<Vec<u8>, WireError> {
        match node {
            Node::Array { control, item } => {
                let n = *self
                    .controls
                    .get(&(control.clone(), ctx.to_vec()))
                    .ok_or_else(mapping)?;
                let mut bytes = Vec::new();
                for i in 0..n {
                    let mut occurrence = ctx.to_vec();
                    occurrence.push(i);
                    bytes.extend(self.flag_bytes(item, &occurrence, details)?);
                }
                Ok(bytes)
            }
            Node::Choice { control, arms } if self.packed_choices.contains(control) => {
                let arm = *self
                    .controls
                    .get(&(control.clone(), ctx.to_vec()))
                    .ok_or_else(mapping)?;
                if !self.fields_only {
                    details.push(
                        json!({"choice":control,"arm":arm,"occurrences":ctx,"packedFlush":true}),
                    );
                }
                match arms.get(arm).ok_or_else(mapping)? {
                    Node::Empty if arm == 1 => Ok(vec![]),
                    node => self.flag_bytes(node, ctx, details),
                }
            }
            _ => {
                let value = self.node(node, ctx, details)?;
                Ok(vec![value
                    .as_u64()
                    .and_then(|n| u8::try_from(n).ok())
                    .ok_or_else(mapping)?])
            }
        }
    }
    fn walk(
        &mut self,
        g: &GrammarNode,
        v: &'a WireValue,
        path: Path,
        base: usize,
        ctx: &[usize],
    ) -> Result<(), WireError> {
        match (g, v) {
            (GrammarNode::Byte(_), WireValue::Byte(_))
            | (GrammarNode::Number(_), WireValue::NumberBits(_))
            | (GrammarNode::String(_), WireValue::StringUnits(_)) => {
                if self.leaves.insert((base, ctx.to_vec()), v).is_some() {
                    return Err(mapping());
                }
                self.order.push((base, ctx.to_vec()));
            }
            (GrammarNode::Sequence { items }, WireValue::Sequence(values)) => {
                // Root-case prefixes may be skipped only with the compiler's
                // explicit certificate for that exact root arm.
                let skip = usize::from(
                    (path.is_empty()
                        || matches!(path.as_slice(), [Step::Arm(arm)] if self.root_prefix_arms.contains(arm)))
                        && matches!(items.first(), Some(GrammarNode::RawPrefix(_))),
                );
                if items.len() - skip != values.len() {
                    return Err(mapping());
                }
                let mut offset = base;
                for (i, (n, value)) in items[skip..].iter().zip(values).enumerate() {
                    self.walk(
                        n,
                        value,
                        extended(&path, Step::Child(i + skip)),
                        offset,
                        ctx,
                    )?;
                    offset += count(n);
                }
            }
            (GrammarNode::Repeat { body, .. }, WireValue::Repeat(values)) => {
                self.controls
                    .insert((path.clone(), ctx.to_vec()), values.len());
                for (i, value) in values.iter().enumerate() {
                    let mut c = ctx.to_vec();
                    c.push(i);
                    self.walk(body, value, extended(&path, Step::Body), base, &c)?;
                }
            }
            (GrammarNode::Choice { arms, .. }, WireValue::Choice { arm, value }) => {
                let selected = arms.get(*arm).ok_or_else(mapping)?;
                self.controls.insert((path.clone(), ctx.to_vec()), *arm);
                self.walk(
                    &selected.body,
                    value,
                    extended(&path, Step::Arm(*arm)),
                    base + arms[..*arm].iter().map(|a| count(&a.body)).sum::<usize>(),
                    ctx,
                )?;
            }
            _ => return Err(mapping()),
        }
        Ok(())
    }
    fn fields(
        &self,
        fields: &[Field],
        ctx: &[usize],
        details: &mut Vec<Value>,
    ) -> Result<Map<String, Value>, WireError> {
        let mut out = Map::new();
        for field in fields {
            if self.counted.count(&field.node).is_some() {
                // Evaluate the original leaf too: its exact bits and bytes remain
                // evidence, while the inverse below replaces it from array length.
                self.node(&field.node, ctx, details)?;
                continue;
            }
            if self.absent(&field.node, ctx, details)? {
                if !self.fields_only {
                    details
                        .push(json!({"fieldKey":field.field_key,"occurrences":ctx,"absent":true}));
                }
                continue;
            }
            let value;
            if let Some(bits) = &field.bits {
                let bytes = self.flag_bytes(&field.node, ctx, details)?;
                let mut flags = Map::new();
                for (name, bit) in bits {
                    let b = bytes.get((*bit / 8) as usize).ok_or_else(mapping)?;
                    flags.insert(name.clone(), json!(b & (1 << (*bit % 8)) != 0));
                }
                if !self.fields_only {
                    details.push(json!({"fieldKey":field.field_key,"occurrences":ctx,"logicalBytes":bytes,"bits":bits}));
                }
                value = Value::Object(flags);
            } else {
                value = self.node(&field.node, ctx, details)?;
            }
            if out.insert(field.name.clone(), value).is_some() {
                return Err(mapping());
            }
        }
        Ok(out)
    }
    fn absent(
        &self,
        node: &Node,
        ctx: &[usize],
        details: &mut Vec<Value>,
    ) -> Result<bool, WireError> {
        match node {
            Node::Empty => Ok(true),
            Node::Choice { control, arms } => {
                let arm = self
                    .controls
                    .get(&(control.clone(), ctx.to_vec()))
                    .ok_or_else(mapping)?;
                let absent = self.absent(arms.get(*arm).ok_or_else(mapping)?, ctx, details)?;
                if absent && !self.fields_only {
                    details.push(json!({"choice":control,"arm":arm,"occurrences":ctx}));
                }
                Ok(absent)
            }
            _ => Ok(false),
        }
    }
    fn node(
        &self,
        node: &Node,
        ctx: &[usize],
        details: &mut Vec<Value>,
    ) -> Result<Value, WireError> {
        Ok(match node {
            Node::Leaf {
                leaf,
                grammar_path,
                mask,
                static_xor,
                byte_mapping,
                ..
            } => {
                let value = *self
                    .leaves
                    .get(&(*leaf, ctx.to_vec()))
                    .ok_or_else(mapping)?;
                let record = !self.fields_only;
                let mut detail = if record {
                    let mut detail = json!({"leaf":leaf,"grammarPath":grammar_path,"occurrences":ctx,"mask":mask,"staticXor":static_xor});
                    if let Some(physical) = self.physical.get(&(*leaf, ctx.to_vec())) {
                        detail["wireBytes"] = physical.clone();
                    }
                    detail
                } else {
                    Value::Null
                };
                let readable = match value {
                    WireValue::Byte(b) => {
                        if let Some(mapping) = byte_mapping {
                            let logical = mapping.decode(*b)?;
                            if record {
                                detail["tagByte"] = json!(b);
                                detail["byteMapping"] = json!(mapping);
                                detail["logicalValue"] = logical.clone();
                            }
                            logical
                        } else {
                            json!(b)
                        }
                    }
                    WireValue::NumberBits(bits) => {
                        if record {
                            detail["numberBits"] = json!(bits);
                        }
                        let n = f64::from_bits(*bits);
                        if n.is_finite() && !(*bits == (-0.0f64).to_bits()) {
                            finite_number(n)
                        } else {
                            json!({"numberBits":bits,"special":if n.is_nan(){"nan"}else if n==0.0{"negative_zero"}else if n.is_sign_negative(){"negative_infinity"}else{"positive_infinity"}})
                        }
                    }
                    WireValue::StringUnits(units) => {
                        if record {
                            detail["utf16Units"] = json!(units);
                        }
                        match String::from_utf16(units) {
                            Ok(s) => json!(s),
                            Err(_) => json!({"utf16Units":units}),
                        }
                    }
                    _ => return Err(mapping()),
                };
                if record {
                    details.push(detail);
                }
                readable
            }
            Node::Record { fields } => {
                let mut values = self.fields(fields, ctx, details)?;
                if let Some(array) = self.counted.collapsed(fields) {
                    values.remove(&array.name).ok_or_else(mapping)?
                } else {
                    Value::Object(values)
                }
            }
            Node::FixedArray { items } => Value::Array(
                items
                    .iter()
                    .map(|f| {
                        let mut value = self.fields(std::slice::from_ref(f), ctx, details)?;
                        value.remove(&f.name).ok_or_else(mapping)
                    })
                    .collect::<Result<_, WireError>>()?,
            ),
            Node::ScalarNull { fields } => {
                let values = self.fields(fields, ctx, details)?;
                if values.len() != fields.len() || values.values().any(|v| !v.is_null()) {
                    return Err(mapping());
                }
                Value::Null
            }
            Node::Array { control, item } => {
                let n = *self
                    .controls
                    .get(&(control.clone(), ctx.to_vec()))
                    .ok_or_else(mapping)?;
                let values: Vec<Value> = (0..n)
                    .map(|i| {
                        let mut c = ctx.to_vec();
                        c.push(i);
                        self.node(item, &c, details)
                    })
                    .collect::<Result<_, _>>()?;
                let length = values.len();
                if let Some(rule) = self.counted.array(node) {
                    let WireValue::NumberBits(bits) = self
                        .leaves
                        .get(&(rule.leaf, ctx.to_vec()))
                        .ok_or_else(mapping)?
                    else {
                        return Err(mapping());
                    };
                    if length > rule.maximum || *bits != (length as f64).to_bits() {
                        return Err(WireError::Bounds);
                    }
                    if !self.fields_only {
                        details.push(json!({"countLeaf":rule.leaf,"occurrences":ctx,"arrayLength":length,"control":control}));
                    }
                }
                Value::Array(values)
            }
            Node::Choice { control, arms } => {
                let arm = *self
                    .controls
                    .get(&(control.clone(), ctx.to_vec()))
                    .ok_or_else(mapping)?;
                if !self.fields_only {
                    details.push(json!({"choice":control,"arm":arm,"occurrences":ctx}));
                }
                self.node(arms.get(arm).ok_or_else(mapping)?, ctx, details)?
            }
            // Named fields omit absence before evaluation. An anonymous
            // no-write node cannot claim scalar null or an empty record.
            Node::Empty => return Err(mapping()),
            Node::Unknown => return Err(mapping()),
        })
    }
}

/// Rebuild mapped bytes from the logical projection, retaining physical values
/// for all other leaves (including IEEE/UTF-16 details and choice provenance).
fn rebuild_tags(
    value: &WireValue,
    order: &[(usize, Vec<usize>)],
    details: &[Value],
) -> Result<WireValue, WireError> {
    let mut replacements = BTreeMap::new();
    for detail in details {
        if let Some(byte_mapping) = detail.get("byteMapping") {
            let byte_mapping: super::semantic::ByteMapping =
                serde_json::from_value(byte_mapping.clone()).map_err(|_| WireError::Evidence)?;
            let leaf = detail["leaf"].as_u64().ok_or_else(mapping)? as usize;
            let occurrences: Vec<usize> = serde_json::from_value(detail["occurrences"].clone())
                .map_err(|_| WireError::Evidence)?;
            let byte = byte_mapping.encode(detail.get("logicalValue").ok_or_else(mapping)?)?;
            if replacements
                .insert((leaf, occurrences), WireValue::Byte(byte))
                .is_some()
            {
                return Err(WireError::Evidence);
            }
        }
        if let Some(leaf) = detail.get("countLeaf").and_then(Value::as_u64) {
            let occurrences: Vec<usize> = serde_json::from_value(detail["occurrences"].clone())
                .map_err(|_| WireError::Evidence)?;
            let length = detail["arrayLength"].as_u64().ok_or_else(mapping)?;
            if replacements
                .insert(
                    (leaf as usize, occurrences),
                    WireValue::NumberBits((length as f64).to_bits()),
                )
                .is_some()
            {
                return Err(WireError::Evidence);
            }
        }
    }
    fn walk(
        value: &mut WireValue,
        order: &mut impl Iterator<Item = (usize, Vec<usize>)>,
        replacements: &mut BTreeMap<(usize, Vec<usize>), WireValue>,
    ) -> Result<(), WireError> {
        match value {
            WireValue::Sequence(values) | WireValue::Repeat(values) => {
                for value in values {
                    walk(value, order, replacements)?;
                }
            }
            WireValue::Choice { value, .. } => walk(value, order, replacements)?,
            leaf => {
                let key = order.next().ok_or_else(mapping)?;
                if let Some(replacement) = replacements.remove(&key) {
                    if !matches!(
                        (&*leaf, &replacement),
                        (WireValue::Byte(_), WireValue::Byte(_))
                            | (WireValue::NumberBits(_), WireValue::NumberBits(_))
                    ) {
                        return Err(WireError::Type);
                    }
                    *leaf = replacement;
                }
            }
        }
        Ok(())
    }
    let mut rebuilt = value.clone();
    let mut order = order.iter().cloned();
    walk(&mut rebuilt, &mut order, &mut replacements)?;
    if order.next().is_some() || !replacements.is_empty() {
        return Err(WireError::Evidence);
    }
    Ok(rebuilt)
}

/// Replay a separate reader to record byte ranges without touching the
/// authoritative decoder's cursor. The stream below only records the bytes
/// actually applied by that proven program, independently of static XOR.
#[cfg(test)]
fn physical(
    program: &super::Program,
    value: &WireValue,
    bytes: &[u8],
    token: i32,
    format: &Result<super::FloatByteOrder, WireError>,
) -> Result<Vec<Value>, WireError> {
    physical_source(program, value, bytes, token, format, None)
}

fn physical_source(
    program: &super::Program,
    value: &WireValue,
    bytes: &[u8],
    token: i32,
    format: &Result<super::FloatByteOrder, WireError>,
    source: Option<&super::source_number::Physical>,
) -> Result<Vec<Value>, WireError> {
    use super::{Op, WireMask};
    use crate::signals::codec::{Keystream, SignalReader};
    if let Op::RootChoice { cases } = &program.body {
        let WireValue::Choice { arm, value } = value else {
            return Err(mapping());
        };
        let case = cases
            .iter()
            .find(|c| c.plan.arm == *arm)
            .ok_or_else(mapping)?;
        let super::Support::Supported { program } = case.support.as_ref() else {
            return Err(mapping());
        };
        return physical_source(program, value, bytes, token, format, source);
    }
    fn walk(
        op: &Op,
        value: &WireValue,
        r: &mut SignalReader<'_>,
        stream: &mut Keystream,
        bytes: &[u8],
        out: &mut Vec<Value>,
        format: &Result<super::FloatByteOrder, WireError>,
        source: &mut Option<super::source_number::Physical>,
    ) -> Result<(), WireError> {
        match (op, value) {
            (Op::Sequence { items }, WireValue::Sequence(values)) => {
                if items.len() != values.len() {
                    return Err(mapping());
                }
                for (o, v) in items.iter().zip(values) {
                    walk(o, v, r, stream, bytes, out, format, source)?;
                }
            }
            (Op::Repeat { body, count, .. }, WireValue::Repeat(values)) => {
                if *count != values.len() {
                    return Err(mapping());
                }
                for v in values {
                    walk(body, v, r, stream, bytes, out, format, source)?;
                }
            }
            (Op::CountedRepeat { body, .. }, WireValue::Repeat(values)) => {
                for v in values {
                    walk(body, v, r, stream, bytes, out, format, source)?;
                }
            }
            (
                Op::Choice { arms, .. }
                | Op::DomainChoice { arms, .. }
                | Op::CounterChoice { arms, .. },
                WireValue::Choice { arm, value },
            ) => walk(
                arms.get(*arm).ok_or_else(mapping)?,
                value,
                r,
                stream,
                bytes,
                out,
                format,
                source,
            )?,
            _ => {
                let mask = match op {
                    Op::Byte { mask, .. } | Op::Number { mask, .. } | Op::String { mask, .. } => {
                        mask
                    }
                    _ => return Err(mapping()),
                };
                let enabled = match mask {
                    WireMask::Inherit => true,
                    WireMask::None => false,
                    _ => return Err(mapping()),
                };
                r.masking(enabled);
                let start = bytes.len() - r.remaining();
                match (op, value) {
                    (Op::Byte { xor, .. }, WireValue::Byte(b)) => {
                        if r.byte().map_err(WireError::codec)? ^ xor != *b {
                            return Err(mapping());
                        }
                    }
                    (Op::Number { .. }, WireValue::NumberBits(bits)) => {
                        let n = match source {
                            Some(source) => source.read(r, *bits, format)?,
                            None => super::float::read(r, format)?,
                        };
                        if n.to_bits() != *bits {
                            return Err(mapping());
                        }
                    }
                    (Op::String { .. }, WireValue::StringUnits(units)) => {
                        if r.string_units().map_err(WireError::codec)? != *units {
                            return Err(mapping());
                        }
                    }
                    _ => return Err(mapping()),
                }
                let end = bytes.len() - r.remaining();
                let keystream = if enabled {
                    (start..end).map(|_| stream.next_byte()).collect::<Vec<_>>()
                } else {
                    vec![]
                };
                let unmasked = bytes[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, b)| b ^ keystream.get(i).copied().unwrap_or(0))
                    .collect::<Vec<_>>();
                out.push(json!({"offset":start,"bytes":&bytes[start..end],"staticXorAppliedBytes":unmasked,"keystream":keystream}));
            }
        }
        Ok(())
    }
    let mut reader = SignalReader::new(bytes, token, program.salt).map_err(WireError::codec)?;
    let mut stream = Keystream::new(token, reader.seed() ^ program.salt);
    let mut out = Vec::new();
    let mut source = source.cloned();
    walk(
        &program.body,
        value,
        &mut reader,
        &mut stream,
        bytes,
        &mut out,
        format,
        &mut source,
    )?;
    if let Some(source) = source {
        source.finish()?;
    }
    if !reader.at_end() {
        return Err(mapping());
    }
    Ok(out)
}

pub(super) fn project(
    slot: &super::WireSlot,
    decoded: &DecodedSlot,
    token: i32,
    format: &Result<super::FloatByteOrder, WireError>,
) -> NamedSignal {
    project_with_program(slot, decoded, token, format, None)
}

/// How much a projection builds besides the named fields.
#[derive(Clone, Copy, Debug)]
pub struct ProjectOptions<'c> {
    /// Per-leaf details and slot metadata. Off: fields only, `metadata` is null.
    pub metadata: bool,
    /// Run the mapped-tag/count re-encode check even without metadata.
    pub verify: bool,
    /// The slot's constant metadata from [`slot_constants`], reused instead of
    /// serializing the slot again.
    pub constants: Option<&'c Map<String, Value>>,
}
impl Default for ProjectOptions<'_> {
    fn default() -> Self {
        Self {
            metadata: true,
            verify: true,
            constants: None,
        }
    }
}

/// Metadata members that depend only on the slot, never on a capture.
pub fn slot_constants(slot: &super::WireSlot) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("routing".into(), json!(slot.routing));
    out.insert("sourcePlan".into(), json!(slot.source_plan));
    if let Some(n) = &slot.naming {
        out.insert("schemaKey".into(), json!(n.schema_key));
        out.insert("canonicalName".into(), json!(n.canonical_name));
        out.insert("projection".into(), json!(n));
    }
    out
}

/// Borrow one explicit-session candidate recipe without cloning the source contract.
/// All other emission recipes retain their original support status and provenance.
pub(super) fn project_with_program(
    slot: &super::WireSlot,
    decoded: &DecodedSlot,
    token: i32,
    format: &Result<super::FloatByteOrder, WireError>,
    selected: Option<(usize, &super::Program)>,
) -> NamedSignal {
    project_with_options(
        slot,
        decoded,
        token,
        format,
        selected,
        ProjectOptions::default(),
    )
}

pub(super) fn project_with_options(
    slot: &super::WireSlot,
    decoded: &DecodedSlot,
    token: i32,
    format: &Result<super::FloatByteOrder, WireError>,
    selected: Option<(usize, &super::Program)>,
    options: ProjectOptions<'_>,
) -> NamedSignal {
    project_source_with_options(slot, decoded, token, format, selected, options, None)
}

pub(crate) fn project_source_with_options(
    slot: &super::WireSlot,
    decoded: &DecodedSlot,
    token: i32,
    format: &Result<super::FloatByteOrder, WireError>,
    selected: Option<(usize, &super::Program)>,
    options: ProjectOptions<'_>,
    source: Option<(&super::source_number::Policy, &super::source_number::Forms)>,
) -> NamedSignal {
    // Details are needed for metadata and for the mapped-tag re-encode check.
    let record = options.metadata || options.verify;
    let program_at = |emission: usize| {
        if let Some((index, program)) = selected {
            if emission == index {
                return Some(program);
            }
        }
        match slot.programs.get(emission) {
            Some(super::Support::Supported { program }) => Some(program),
            _ => None,
        }
    };
    let mut metadata = if options.metadata {
        let mut m = match options.constants {
            Some(constants) => constants.clone(),
            None => slot_constants(slot),
        };
        m.insert("address".into(), json!(decoded.address));
        m.insert("position".into(), json!(decoded.positional));
        m.insert("descriptor".into(), json!(decoded.descriptor));
        m.insert("capturedSeed".into(), json!(decoded.captured_seed));
        m.insert("bytes".into(), json!(decoded.raw));
        m.insert("selection".into(), json!(decoded.selection));
        Value::Object(m)
    } else {
        Value::Null
    };
    let (mut status, mut fields) = match &decoded.outcome {
        Outcome::Missing => (NamedStatus::Missing, None),
        Outcome::Invalid { reason } => {
            metadata["reason"] = json!(reason);
            (NamedStatus::Invalid, None)
        }
        Outcome::Unsupported { reason } => {
            metadata["reason"] = json!(reason);
            (NamedStatus::Unsupported, None)
        }
        Outcome::Decoded { .. } => (NamedStatus::Unsupported, None),
    };
    if let Outcome::Decoded { value, .. } = &decoded.outcome {
        let result = (|| {
            let names = slot.naming.as_ref().ok_or_else(mapping)?;
            if names.mapping_error.is_some() {
                return Err(mapping());
            }
            let mut events = Vec::new();
            // Use the consumer selection, retaining every identical candidate
            // provenance without inventing an event order.
            for (emission, e) in slot.contract.emissions.iter().enumerate() {
                if decoded
                    .selection
                    .as_ref()
                    .is_some_and(|s| !s.emissions.contains(&emission))
                {
                    continue;
                }
                let mut runtime = Runtime {
                    fields_only: !record,
                    ..Runtime::default()
                };
                let program = program_at(emission).ok_or_else(mapping)?;
                runtime.packing(&program.body);
                if let super::Op::RootChoice { cases } = &program.body {
                    if !matches!(e.grammar, GrammarNode::Choice { .. }) {
                        return Err(mapping());
                    }
                    runtime
                        .root_prefix_arms
                        .extend(cases.iter().map(|c| c.plan.arm));
                } else if !matches!(e.grammar, GrammarNode::Sequence { .. }) {
                    return Err(mapping());
                }
                runtime.walk(&e.grammar, value, vec![], 0, &[])?;
                if record {
                    let physical = physical_source(
                        program,
                        value,
                        decoded.raw.as_deref().ok_or_else(mapping)?,
                        token,
                        format,
                        source.map(|(_, forms)| forms.physical(value)).as_ref(),
                    )?;
                    if physical.len() != runtime.order.len() {
                        return Err(mapping());
                    }
                    runtime.physical = runtime.order.iter().cloned().zip(physical).collect();
                }
                for part in names.parts.iter().filter(|p| p.emission == emission) {
                    runtime.counted = if names.counted_arrays {
                        super::counted::Rules::new(program, &part.fields)
                    } else {
                        super::counted::Rules::default()
                    };
                    if let Some(control) = &part.split_choice {
                        if runtime.controls.get(&(control.clone(), vec![])) != Some(&part.part) {
                            continue;
                        }
                    }
                    let mut details = Vec::new();
                    let mut fields = runtime.fields(&part.fields, &[], &mut details)?;
                    if names.version >= 2 && super::readable::flatten(&part.fields) {
                        fields = fields
                            .remove(&part.fields[0].name)
                            .and_then(|v| v.as_object().cloned())
                            .ok_or_else(mapping)?;
                    }
                    if details.iter().any(|detail| {
                        detail.get("byteMapping").is_some() || detail.get("countLeaf").is_some()
                    }) {
                        let rebuilt = rebuild_tags(value, &runtime.order, &details)?;
                        let encoded = if let Some((policy, forms)) = source {
                            let forms = super::source_number::reconcile(forms, value, &rebuilt);
                            super::source_number::encode(
                                program,
                                &rebuilt,
                                decoded.captured_seed.ok_or_else(mapping)?,
                                token,
                                *format.as_ref().map_err(Clone::clone)?,
                                Some(policy),
                                Some(&forms),
                            )?
                        } else {
                            program.encode_with_float(
                                &rebuilt,
                                decoded.captured_seed.ok_or_else(mapping)?,
                                token,
                                format,
                            )?
                        };
                        if Some(encoded.as_slice()) != decoded.raw.as_deref() {
                            return Err(WireError::Reencode);
                        }
                    }
                    // Move `details` in: `json!` would deep-copy an existing Value through serde.
                    let event = if options.metadata {
                        let mut event =
                            json!({"emission":emission,"part":part.part,"message":part.message});
                        event["fields"] = Value::Array(details);
                        event
                    } else {
                        Value::Null
                    };
                    events.push((fields, event));
                }
            }
            let first = events.first().ok_or_else(mapping)?.0.clone();
            if events.iter().any(|(f, _)| *f != first) {
                return Err(mapping());
            }
            if !options.metadata {
                return Ok(first);
            }
            metadata["projectionCandidates"] =
                Value::Array(events.into_iter().map(|(_, m)| m).collect());
            metadata["streamSalts"] = json!(slot
                .programs
                .iter()
                .enumerate()
                .map(|(emission, _)| match program_at(emission) {
                    Some(program) => match &program.body {
                        super::Op::RootChoice { cases } =>
                            json!(cases.iter().map(|c| c.plan.salt).collect::<Vec<_>>()),
                        _ => json!(program.salt),
                    },
                    _ => Value::Null,
                })
                .collect::<Vec<_>>());
            Ok(first)
        })();
        match result {
            Ok(f) => {
                status = NamedStatus::Decoded;
                fields = Some(f)
            }
            Err(e) => metadata["reason"] = json!(e),
        }
    }
    NamedSignal {
        fields,
        status,
        metadata,
    }
}

pub(super) fn document(
    schema: &super::WireSchema,
    decoded: &[DecodedSlot],
    report: &Report,
    token: i32,
    options: super::TypedDecoderOptions,
    format: &Result<super::FloatByteOrder, WireError>,
) -> Result<NamedDocument, WireError> {
    let mut signals = BTreeMap::new();
    let mut unresolved_slots = Vec::new();
    for d in decoded {
        let slot = schema
            .slots
            .iter()
            .find(|s| {
                s.routing
                    == WireRoute::Positional {
                        positional: d.positional,
                    }
            })
            .ok_or(WireError::Routing)?;
        let mut value = project(slot, d, token, format);
        if let Some(census) = &schema.initialization {
            value.metadata["initialization"] = json!(census
                .emissions
                .iter()
                .filter(|entry| entry.descriptor == slot.contract.descriptor)
                .collect::<Vec<_>>());
        }

        if let Some(names) = &slot.naming {
            if signals.insert(names.schema_key.clone(), value).is_some() {
                return Err(WireError::Evidence);
            }
        } else {
            unresolved_slots.push(value);
        }
    }
    for slot in &schema.slots {
        if matches!(slot.routing, WireRoute::Name { .. }) {
            let signal = NamedSignal {
                fields: None,
                status: NamedStatus::Unsupported,
                metadata: json!({"descriptor":slot.contract.descriptor,"address":slot.address,"routing":slot.routing,"reason":"named channel execution is unavailable; original channel retained"}),
            };
            if let Some(n) = &slot.naming {
                if signals.insert(n.schema_key.clone(), signal).is_some() {
                    return Err(WireError::Evidence);
                }
            } else {
                unresolved_slots.push(signal);
            }
        }
    }
    Ok(NamedDocument {
        format_version: 1,
        signals,
        unresolved_slots,
        metadata: RoundTrip {
            decoder_options: options,
            initialization: schema.initialization.clone(),
            original_report: report.clone(),
        },
    })
}

#[path = "named_inverse.rs"]
mod inverse;
pub(crate) use inverse::{canonical_fields, encode_signal_source};

#[cfg(test)]
#[path = "named_tests.rs"]
mod tests;
