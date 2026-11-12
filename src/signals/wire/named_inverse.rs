//! Inverse of the named projection: named JSON → one signal's wire bytes.
//!
//! Decoding (`named.rs`) walks a source-certified grammar over a
//! [`WireValue`] and projects its leaves into named fields. Encoding walks the
//! same grammar in the opposite direction: the requested fields become a
//! partial edit of the retained original value (when the caller supplies the
//! captured bytes) or a complete value built from the fields alone. The result
//! is accepted only when decoding the produced bytes projects back to exactly
//! the requested fields; no bytes are returned otherwise.
//!
//! A retained original value supplies every fact the display cannot: the
//! per-signal random prefix, unprojected leaves (for example flag arrays with no
//! named bits), and packed/flush arm choices. Edits only replace what the named
//! fields actually describe.
use super::super::{
    counted, ControlStep, CounterChoiceProof, CounterInput, DecodedSlot, FloatByteOrder, Op,
    Outcome, PackedPhase, Program, Selection, SelectionProof, Support, WireError, WireSlot,
    WireValue,
};
use super::{
    count, extended, Field, NamedStatus, Naming, Node, Part, ProjectOptions, Runtime, Static, Step,
};
use crate::signals::extract::contracts::{GrammarNode, LoopDirection};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

type Ctx = Vec<usize>;
type Path = Vec<Step>;

/// A partial replacement of a decoded value, keyed exactly like `Runtime`.
#[derive(Clone, Default)]
struct Edit {
    /// Leaf ordinal + occurrence path → new value.
    leaves: BTreeMap<(usize, Ctx), WireValue>,
    /// Control path + occurrence path → chosen arm or repeat length.
    controls: BTreeMap<(Path, Ctx), usize>,
    /// Flag fields whose bytes are applied after the structure is resolved.
    bits: Vec<BitsField>,
}

/// One flag field: `bits` names the bit of each concatenated byte, `values`
/// carries the booleans the caller asked for.
#[derive(Clone)]
struct BitsField {
    node: Node,
    values: BTreeMap<u32, bool>,
    ctx: Ctx,
}

/// The original value the retained bytes decode to, flattened for lookups.
struct Base {
    source: Option<super::super::source_number::State>,
    leaves: BTreeMap<(usize, Ctx), WireValue>,
    controls: BTreeMap<(Path, Ctx), usize>,
}

fn base_of(grammar: &GrammarNode, program: &Program, value: &WireValue) -> Result<Base, WireError> {
    let mut runtime: Runtime<'_> = Runtime {
        fields_only: true,
        ..Runtime::default()
    };
    runtime.packing(&program.body);
    if let Op::RootChoice { cases } = &program.body {
        if !matches!(grammar, GrammarNode::Choice { .. }) {
            return Err(WireError::Evidence);
        }
        runtime
            .root_prefix_arms
            .extend(cases.iter().map(|case| case.plan.arm));
    } else if !matches!(grammar, GrammarNode::Sequence { .. }) {
        return Err(WireError::Evidence);
    }
    runtime.walk(grammar, value, vec![], 0, &[])?;
    Ok(Base {
        source: None,
        leaves: runtime
            .leaves
            .into_iter()
            .map(|(key, value)| (key, value.clone()))
            .collect(),
        controls: runtime.controls,
    })
}

fn type_error() -> WireError {
    WireError::Type
}
fn stage(stage: &str, error: WireError) -> WireError {
    match error {
        WireError::Unsupported(reason) => WireError::Unsupported(format!("{stage}: {reason}")),
        other => WireError::Unsupported(format!("{stage}: {other}")),
    }
}

/// The decoder renders finite integral numbers as JSON integers. Compare
/// requested fields the same way so an edit such as `8.0` still names 8.
pub(crate) fn canonical_fields(fields: &Map<String, Value>) -> Map<String, Value> {
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Number(number) if number.as_i64().is_none() && number.as_u64().is_none() => {
                number
                    .as_f64()
                    .map(super::finite_number)
                    .unwrap_or_else(|| value.clone())
            }
            Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(key, value)| (key.clone(), canonical(value)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
    fields
        .iter()
        .map(|(key, value)| (key.clone(), canonical(value)))
        .collect()
}
fn op_kind(op: &Op) -> &'static str {
    match op {
        Op::RootChoice { .. } => "root_choice",
        Op::Byte { .. } => "byte",
        Op::Number { .. } => "number",
        Op::String { .. } => "string",
        Op::Sequence { .. } => "sequence",
        Op::Repeat { .. } => "repeat",
        Op::CountedRepeat { .. } => "counted_repeat",
        Op::CounterChoice { .. } => "counter_choice",
        Op::DomainChoice { .. } => "domain_choice",
        Op::Choice { .. } => "choice",
    }
}
fn unsupported(reason: &str) -> WireError {
    WireError::Unsupported(reason.into())
}

fn number(value: &WireValue) -> Result<f64, WireError> {
    let WireValue::NumberBits(bits) = value else {
        return Err(WireError::Type);
    };
    Ok(f64::from_bits(*bits))
}
fn count_of(value: &WireValue) -> Result<usize, WireError> {
    let number = number(value)?;
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number > usize::MAX as f64 {
        return Err(WireError::Bounds);
    }
    Ok(number as usize)
}

/// Map one projected leaf value to its wire value. Byte mappings own the
/// logical meaning (including null); numeric and string special shapes retain
/// the bits/units the decoder emits for non-finite numbers and lone surrogates.
fn leaf_value(
    kind: &str,
    mapping: Option<&super::super::semantic::ByteMapping>,
    value: &Value,
) -> Result<WireValue, WireError> {
    if let Some(mapping) = mapping {
        return mapping.encode(value).map(WireValue::Byte);
    }
    match kind {
        "byte" => value
            .as_u64()
            .filter(|byte| *byte <= u8::MAX as u64)
            .map(|byte| WireValue::Byte(byte as u8))
            .ok_or_else(type_error),
        "number" => {
            if let Some(bits) = value.get("numberBits").and_then(Value::as_u64) {
                return Ok(WireValue::NumberBits(bits));
            }
            let number = value.as_f64().ok_or_else(type_error)?;
            Ok(WireValue::NumberBits(number.to_bits()))
        }
        "string" => {
            if let Some(units) = value.get("utf16Units").and_then(Value::as_array) {
                let units = units
                    .iter()
                    .map(|unit| {
                        unit.as_u64()
                            .filter(|unit| *unit <= u16::MAX as u64)
                            .map(|unit| unit as u16)
                            .ok_or_else(type_error)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(WireValue::StringUnits(units));
            }
            let text = value.as_str().ok_or_else(type_error)?;
            Ok(WireValue::StringUnits(text.encode_utf16().collect()))
        }
        // A generic value leaf is only reachable for named-object emissions,
        // which this schema version does not map to named fields.
        _ => Err(unsupported("generic value leaf has no named projection")),
    }
}

#[derive(Clone)]
struct Assign<'a> {
    counted: counted::Rules,
    edit: Edit,
    base_controls: Option<&'a BTreeMap<(Path, Ctx), usize>>,
}

impl Assign<'_> {
    fn fields(
        &mut self,
        fields: &[Field],
        ctx: &Ctx,
        values: &Map<String, Value>,
        flatten: bool,
    ) -> Result<(), WireError> {
        if flatten {
            let [field] = fields else {
                return Err(WireError::Evidence);
            };
            let bits = field.bits.as_ref().ok_or(WireError::Evidence)?;
            return self.assign_bits(bits, &field.node, values, ctx);
        }
        let mut names = BTreeSet::new();
        for field in fields {
            names.insert(field.name.as_str());
            self.assign_field(field, values.get(&field.name), ctx)?;
        }
        if values.keys().any(|key| !names.contains(key.as_str())) {
            return Err(type_error());
        }
        Ok(())
    }

    fn assign_field(
        &mut self,
        field: &Field,
        value: Option<&Value>,
        ctx: &Ctx,
    ) -> Result<(), WireError> {
        if self.counted.count(&field.node).is_some() {
            // A counted array length is derived from the array, never read
            // from a displayed field.
            return if value.is_some() {
                Err(type_error())
            } else {
                Ok(())
            };
        }
        match value {
            None => self.absent(&field.node, ctx),
            Some(value) => match &field.bits {
                Some(bits) => self.assign_bits(
                    bits,
                    &field.node,
                    value.as_object().ok_or_else(type_error)?,
                    ctx,
                ),
                None => self.node(&field.node, value, ctx),
            },
        }
    }

    /// A field is absent only when the projection would omit it: an optional
    /// choice selecting its no-write arm. Records are never absent.
    fn absent(&mut self, node: &Node, ctx: &Ctx) -> Result<(), WireError> {
        match node {
            Node::Empty => Ok(()),
            Node::Choice { control, arms } => {
                let key = (control.clone(), ctx.to_vec());
                let preferred = self.base_controls.and_then(|base| base.get(&key)).copied();
                let mut order: Vec<usize> = (0..arms.len()).collect();
                if let Some(preferred) = preferred {
                    order.retain(|arm| *arm != preferred);
                    order.insert(0, preferred);
                }
                for arm in order {
                    let mut trial = self.clone();
                    if trial.absent(&arms[arm], ctx).is_ok() {
                        self.edit = trial.edit;
                        self.edit.controls.insert(key, arm);
                        return Ok(());
                    }
                }
                Err(type_error())
            }
            _ => Err(type_error()),
        }
    }

    fn assign_bits(
        &mut self,
        bits: &BTreeMap<String, u32>,
        node: &Node,
        values: &Map<String, Value>,
        ctx: &Ctx,
    ) -> Result<(), WireError> {
        if values.len() != bits.len() || bits.keys().any(|name| !values.contains_key(name)) {
            return Err(type_error());
        }
        let mut flags = BTreeMap::new();
        for (name, bit) in bits {
            let value = values
                .get(name)
                .and_then(Value::as_bool)
                .ok_or_else(type_error)?;
            flags.insert(*bit, value);
        }
        // A present flag byte is the first arm that contributes enough bytes,
        // preferring the retained arm so an unedited value keeps its preimage.
        // Arrays derive their length and flush pattern from the structure.
        if let Node::Choice { control, arms } = node {
            let needed = bits
                .values()
                .copied()
                .max()
                .map(|bit| bit as usize / 8 + 1)
                .unwrap_or(0);
            let key = (control.clone(), ctx.to_vec());
            let preferred = self.base_controls.and_then(|base| base.get(&key)).copied();
            let mut order: Vec<usize> = (0..arms.len()).collect();
            if let Some(preferred) = preferred {
                order.retain(|arm| *arm != preferred);
                order.insert(0, preferred);
            }
            let arm = order
                .into_iter()
                .find(|arm| bits_bytes_in_arm(&arms[*arm]) >= needed)
                .ok_or_else(type_error)?;
            self.edit.controls.insert(key, arm);
        }
        self.edit.bits.push(BitsField {
            node: node.clone(),
            values: flags,
            ctx: ctx.to_vec(),
        });
        Ok(())
    }

    fn node(&mut self, node: &Node, value: &Value, ctx: &Ctx) -> Result<(), WireError> {
        match node {
            Node::Leaf {
                leaf,
                kind,
                byte_mapping,
                ..
            } => {
                let value = leaf_value(kind, byte_mapping.as_ref(), value)?;
                self.edit.leaves.insert((*leaf, ctx.to_vec()), value);
                Ok(())
            }
            Node::Record { fields } => {
                if let Some(array) = self.counted.collapsed(fields) {
                    if value.is_array() {
                        return self.node(&array.node, value, ctx);
                    }
                }
                let values = value.as_object().ok_or_else(type_error)?;
                self.fields(fields, ctx, values, false)
            }
            Node::FixedArray { items } => {
                let values = value.as_array().ok_or_else(type_error)?;
                if values.len() != items.len() {
                    return Err(type_error());
                }
                for (field, value) in items.iter().zip(values) {
                    self.assign_field(field, Some(value), ctx)?;
                }
                Ok(())
            }
            Node::ScalarNull { fields } => {
                if !value.is_null() {
                    return Err(type_error());
                }
                for field in fields {
                    self.assign_field(field, Some(&Value::Null), ctx)?;
                }
                Ok(())
            }
            Node::Array { control, item } => {
                let values = value.as_array().ok_or_else(type_error)?;
                self.edit
                    .controls
                    .insert((control.clone(), ctx.to_vec()), values.len());
                for (index, value) in values.iter().enumerate() {
                    let mut occurrence = ctx.to_vec();
                    occurrence.push(index);
                    self.node(item, value, &occurrence)?;
                }
                if let Some(rule) = self.counted.array(node) {
                    self.edit.leaves.insert(
                        (rule.leaf, ctx.to_vec()),
                        WireValue::NumberBits((values.len() as f64).to_bits()),
                    );
                }
                Ok(())
            }
            Node::Choice { control, arms } => {
                let key = (control.clone(), ctx.to_vec());
                let preferred = self.base_controls.and_then(|base| base.get(&key)).copied();
                let mut order: Vec<usize> = (0..arms.len()).collect();
                if let Some(preferred) = preferred {
                    order.retain(|arm| *arm != preferred);
                    order.insert(0, preferred);
                }
                for arm in order {
                    let mut trial = self.clone();
                    if trial.node(&arms[arm], value, ctx).is_ok() {
                        self.edit = trial.edit;
                        self.edit.controls.insert(key, arm);
                        return Ok(());
                    }
                }
                Err(type_error())
            }
            Node::Empty => Err(type_error()),
            Node::Unknown => Err(unsupported("unmapped field")),
        }
    }
}

/// How many bytes one choice arm contributes to a flag field. Only used to
/// choose a present flag byte's arm; arrays are resolved after planning.
fn bits_bytes_in_arm(node: &Node) -> usize {
    match node {
        Node::Leaf { .. } => 1,
        Node::Empty => 0,
        Node::FixedArray { items } => items.iter().map(|item| bits_bytes_in_arm(&item.node)).sum(),
        Node::Array { .. } => usize::MAX / 2,
        Node::Choice { arms, .. } => arms.iter().map(bits_bytes_in_arm).max().unwrap_or(0),
        Node::Record { fields } => fields
            .iter()
            .map(|field| bits_bytes_in_arm(&field.node))
            .sum(),
        Node::ScalarNull { fields } => fields
            .iter()
            .map(|field| bits_bytes_in_arm(&field.node))
            .sum(),
        Node::Unknown => 0,
    }
}

fn assign_part(
    program: &Program,
    part: &Part,
    fields: &Map<String, Value>,
    base_controls: Option<&BTreeMap<(Path, Ctx), usize>>,
    flatten: bool,
    rules: counted::Rules,
) -> Result<Edit, WireError> {
    let mut assign = Assign {
        counted: rules,
        edit: Edit::default(),
        base_controls,
    };
    if let Some(control) = &part.split_choice {
        assign
            .edit
            .controls
            .insert((control.clone(), vec![]), part.part);
    }
    assign.fields(&part.fields, &Vec::new(), fields, flatten)?;
    Ok(assign.edit)
}

/// Immutable facts every planning and building step shares.
struct Env<'a> {
    facts: &'a Static,
    base: Option<&'a Base>,
    packed: BTreeSet<Path>,
    prefix: Path,
    /// The grammar the compiled body was built from: the emission grammar, or
    /// one root-case arm body.
    local: &'a GrammarNode,
}

impl Env<'_> {
    /// Proof and count paths are relative to the compiled body (one RAW4
    /// prefix removed; a root arm body for a root-choice program).
    fn absolute_control(&self, path: &[ControlStep]) -> Path {
        let mut out = self.prefix.clone();
        out.extend(path.iter().map(|step| match step {
            ControlStep::Child(index) => Step::Child(*index),
            ControlStep::Arm(index) => Step::Arm(*index),
            ControlStep::Body => Step::Body,
        }));
        out
    }
    /// `CountRef::path` numbers grammar children the way the compiled
    /// program does: a loop body is child 0 and a branch is its arm index.
    fn absolute_count(&self, indices: &[usize]) -> Option<Path> {
        let mut out = self.prefix.clone();
        let mut grammar = self.local;
        for index in indices {
            match grammar {
                GrammarNode::Sequence { items } => {
                    out.push(Step::Child(*index));
                    grammar = items.get(*index)?;
                }
                GrammarNode::Repeat { body, .. } => {
                    if *index != 0 {
                        return None;
                    }
                    out.push(Step::Body);
                    grammar = body;
                }
                GrammarNode::Choice { arms, .. } => {
                    out.push(Step::Arm(*index));
                    grammar = &arms.get(*index)?.body;
                }
                _ => return None,
            }
        }
        Some(out)
    }
    fn leaf(&self, path: &Path) -> Option<usize> {
        self.facts.paths.iter().position(|fact| fact == path)
    }
    fn base_leaf(&self, leaf: usize, ctx: &Ctx) -> Option<&WireValue> {
        self.base
            .and_then(|base| base.leaves.get(&(leaf, ctx.to_vec())))
    }
    /// Any occurrence of a leaf. Used for counts that the compiled program
    /// reads from one path (a length is constant across occurrences).
    fn any_leaf(&self, leaf: usize) -> Option<&WireValue> {
        self.base.and_then(|base| {
            base.leaves
                .iter()
                .find(|((candidate, _), _)| *candidate == leaf)
                .map(|(_, value)| value)
        })
    }
}

/// Structure resolved before flag bytes are written: every reachable repeat
/// length and every choice arm, keyed like `Runtime`'s own controls.
#[derive(Default)]
struct State {
    edit: Edit,
    counts: BTreeMap<(Path, Ctx), usize>,
    arms: BTreeMap<(Path, Ctx), usize>,
    counters: BTreeMap<Path, u32>,
    ordinals: BTreeMap<Path, u32>,
}

impl State {
    fn count(&self, path: &Path, ctx: &Ctx) -> Option<usize> {
        self.counts.get(&(path.clone(), ctx.to_vec())).copied()
    }
    fn arm(&self, path: &Path, ctx: &Ctx) -> Option<usize> {
        self.arms.get(&(path.clone(), ctx.to_vec())).copied()
    }
    fn edited_leaf(&self, leaf: usize, ctx: &Ctx) -> Option<&WireValue> {
        self.edit.leaves.get(&(leaf, ctx.to_vec()))
    }
}

fn plan(
    env: &Env<'_>,
    state: &mut State,
    grammar: &GrammarNode,
    op: &Op,
    path: &Path,
    ctx: &Ctx,
) -> Result<(), WireError> {
    match grammar {
        GrammarNode::Sequence { items } => {
            let skip = usize::from(matches!(items.first(), Some(GrammarNode::RawPrefix(_))));
            let Op::Sequence { items: ops } = op else {
                return Err(WireError::Unsupported(format!(
                    "sequence op mismatch at {path:?}: {}",
                    op_kind(op)
                )));
            };
            if ops.len() + skip != items.len() {
                return Err(WireError::Unsupported(format!(
                    "sequence arity at {path:?}: ops {} skip {skip} grammar {}",
                    ops.len(),
                    items.len()
                )));
            }
            for (index, item) in items.iter().enumerate().skip(skip) {
                plan(
                    env,
                    state,
                    item,
                    &ops[index - skip],
                    &extended(path, Step::Child(index)),
                    ctx,
                )?;
            }
            Ok(())
        }
        GrammarNode::Repeat { body, .. } => {
            let length = repeat_length(env, state, op, path, ctx)?;
            state.counts.insert((path.clone(), ctx.to_vec()), length);
            let (body_op, direction) = match op {
                Op::Repeat {
                    body, direction, ..
                } => (&**body, *direction),
                Op::CountedRepeat {
                    body, direction, ..
                } => (&**body, *direction),
                _ => {
                    return Err(WireError::Unsupported(format!(
                        "repeat op mismatch at {path:?}: {}",
                        op_kind(op)
                    )))
                }
            };
            let repeat_path = path.clone();
            for index in 0..length {
                let counter = if direction == LoopDirection::Descending {
                    (length - 1 - index) as u32
                } else {
                    index as u32
                };
                state.counters.insert(repeat_path.clone(), counter);
                state.ordinals.insert(repeat_path.clone(), index as u32);
                let mut occurrence = ctx.to_vec();
                occurrence.push(index);
                plan(
                    env,
                    state,
                    body,
                    body_op,
                    &extended(path, Step::Body),
                    &occurrence,
                )?;
            }
            state.counters.remove(&repeat_path);
            state.ordinals.remove(&repeat_path);
            Ok(())
        }
        GrammarNode::Choice { arms, .. } => {
            let arm = choice_arm(env, state, op, path, ctx)?;
            let selected = arms.get(arm).ok_or(WireError::Evidence)?;
            state.arms.insert((path.clone(), ctx.to_vec()), arm);
            let arm_op = match op {
                Op::Choice { arms, .. }
                | Op::DomainChoice { arms, .. }
                | Op::CounterChoice { arms, .. } => arms.get(arm).ok_or(WireError::Evidence)?,
                _ => {
                    return Err(WireError::Unsupported(format!(
                        "choice op mismatch at {path:?}: {}",
                        op_kind(op)
                    )))
                }
            };
            plan(
                env,
                state,
                &selected.body,
                arm_op,
                &extended(path, Step::Arm(arm)),
                ctx,
            )
        }
        GrammarNode::RawPrefix(_) => Err(WireError::Evidence),
        GrammarNode::Byte(_) | GrammarNode::Number(_) | GrammarNode::String(_) => Ok(()),
        GrammarNode::Opaque { .. } | GrammarNode::Unresolved { .. } => Err(WireError::Evidence),
    }
}

fn repeat_length(
    env: &Env<'_>,
    state: &mut State,
    op: &Op,
    path: &Path,
    ctx: &Ctx,
) -> Result<usize, WireError> {
    let overridden = state
        .edit
        .controls
        .get(&(path.clone(), ctx.to_vec()))
        .copied();
    let (counted_leaf, fixed) = match op {
        Op::Repeat { count, .. } => (None, Some(*count)),
        Op::CountedRepeat {
            count: super::super::CountExpr::PriorNumber { reference },
            ..
        } => {
            let leaf = env
                .absolute_count(&reference.path)
                .and_then(|path| env.leaf(&path))
                .ok_or(WireError::Evidence)?;
            let value = state
                .edited_leaf(leaf, ctx)
                .cloned()
                .or_else(|| env.base_leaf(leaf, ctx).cloned());
            let length = match value {
                Some(value) => {
                    let number = number(&value)?;
                    if reference.reject_negative_zero && number == 0.0 && number.is_sign_negative()
                    {
                        return Err(WireError::Bounds);
                    }
                    let length = count_of(&value)?;
                    if length > reference.maximum {
                        return Err(WireError::Bounds);
                    }
                    Some(length)
                }
                None => None,
            };
            (Some((leaf, reference.maximum)), length)
        }
        _ => return Err(WireError::Evidence),
    };
    match (overridden, fixed) {
        (Some(overridden), Some(expected)) if overridden != expected => {
            // A displayed count field may not disagree with the array length.
            Err(WireError::Type)
        }
        (Some(overridden), _) => {
            // A derived (hidden) count leaf follows the array length.
            if let Some((leaf, maximum)) = counted_leaf {
                if overridden > maximum {
                    return Err(WireError::Bounds);
                }
                if !state.edit.leaves.contains_key(&(leaf, ctx.to_vec())) {
                    state.edit.leaves.insert(
                        (leaf, ctx.to_vec()),
                        WireValue::NumberBits((overridden as f64).to_bits()),
                    );
                }
            }
            Ok(overridden)
        }
        (None, Some(fixed)) => Ok(fixed),
        (None, None) => Err(unsupported(
            "repeat length is neither projected nor retained",
        )),
    }
}

fn choice_arm(
    env: &Env<'_>,
    state: &mut State,
    op: &Op,
    path: &Path,
    ctx: &Ctx,
) -> Result<usize, WireError> {
    let key = (path.clone(), ctx.to_vec());
    if let Some(arm) = state.edit.controls.get(&key) {
        return Ok(*arm);
    }
    if let Op::CounterChoice { proof, arms } = op {
        let counter = counter_value(env, state, proof)?;
        let arm = usize::from(!proof.predicate.test(counter));
        if arm >= arms.len() {
            return Err(WireError::Discriminant);
        }
        return Ok(arm);
    }
    if let Some(arm) = env.base.and_then(|base| base.controls.get(&key)).copied() {
        return Ok(arm);
    }
    Err(WireError::Unsupported(format!(
        "choice arm at {path:?} is neither projected nor retained"
    )))
}

fn counter_value(
    env: &Env<'_>,
    state: &mut State,
    proof: &CounterChoiceProof,
) -> Result<u32, WireError> {
    match &proof.input {
        CounterInput::Induction => state
            .counters
            .get(&env.absolute_control(&proof.counter.repeat_path))
            .copied()
            .ok_or(WireError::Evidence),
        CounterInput::Packed {
            count,
            modulus,
            phase,
            ..
        } => {
            if *modulus != 8 {
                return Err(WireError::Evidence);
            }
            match phase {
                PackedPhase::BeforeFlush => state
                    .ordinals
                    .get(&env.absolute_control(&proof.counter.repeat_path))
                    .map(|index| index % modulus + 1)
                    .ok_or(WireError::Evidence),
                PackedPhase::AfterLoop => {
                    let leaf = env
                        .absolute_count(&count.path)
                        .and_then(|path| env.leaf(&path))
                        .ok_or(WireError::Evidence)?;
                    let value = state
                        .edit
                        .leaves
                        .iter()
                        .find(|((candidate, _), _)| *candidate == leaf)
                        .map(|(_, value)| value)
                        .cloned()
                        .or_else(|| env.any_leaf(leaf).cloned())
                        .ok_or(WireError::Evidence)?;
                    Ok((count_of(&value)? as u32) % modulus)
                }
            }
        }
    }
}

/// Byte locations emitted by `Runtime::flag_bytes` for one flag field, in
/// concatenation order. Packed flush arms contribute no byte.
fn byte_locations(
    env: &Env<'_>,
    state: &State,
    node: &Node,
    ctx: &Ctx,
    out: &mut Vec<(usize, Ctx)>,
) -> Result<(), WireError> {
    match node {
        Node::Leaf { leaf, .. } => {
            out.push((*leaf, ctx.to_vec()));
            Ok(())
        }
        Node::Array { control, item } => {
            let length = state
                .count(control, ctx)
                .ok_or_else(|| unsupported("flag array length is unresolved"))?;
            for index in 0..length {
                let mut occurrence = ctx.to_vec();
                occurrence.push(index);
                byte_locations(env, state, item, &occurrence, out)?;
            }
            Ok(())
        }
        Node::Choice { control, arms } => {
            let arm = state
                .arm(control, ctx)
                .ok_or_else(|| unsupported("flag arm is unresolved"))?;
            let selected = arms.get(arm).ok_or(WireError::Evidence)?;
            if matches!(selected, Node::Empty) {
                return if env.packed.contains(control) {
                    Ok(())
                } else {
                    Err(type_error())
                };
            }
            byte_locations(env, state, selected, ctx, out)
        }
        _ => Err(unsupported("flag field is not a byte sequence")),
    }
}

fn apply_bits(env: &Env<'_>, state: &mut State) -> Result<(), WireError> {
    let bits = std::mem::take(&mut state.edit.bits);
    for field in bits {
        let needed = field
            .values
            .keys()
            .max()
            .map(|bit| *bit as usize / 8 + 1)
            .unwrap_or(0);
        let mut locations = Vec::new();
        byte_locations(env, state, &field.node, &field.ctx, &mut locations)?;
        if locations.len() < needed {
            return Err(WireError::Type);
        }
        let mut bytes = Vec::with_capacity(locations.len());
        for (leaf, ctx) in &locations {
            let retained = match env.base_leaf(*leaf, ctx) {
                Some(WireValue::Byte(byte)) => *byte,
                Some(_) => return Err(WireError::Evidence),
                None => 0,
            };
            bytes.push(retained);
        }
        for (bit, set) in &field.values {
            let byte = &mut bytes[(bit / 8) as usize];
            let mask = 1u8 << (bit % 8);
            if *set {
                *byte |= mask;
            } else {
                *byte &= !mask;
            }
        }
        for ((leaf, ctx), byte) in locations.into_iter().zip(bytes) {
            state.edit.leaves.insert((leaf, ctx), WireValue::Byte(byte));
        }
    }
    Ok(())
}

fn build(
    env: &Env<'_>,
    state: &State,
    grammar: &GrammarNode,
    op: &Op,
    path: &Path,
    ctx: &Ctx,
    base: usize,
) -> Result<WireValue, WireError> {
    Ok(match (grammar, op) {
        (GrammarNode::Sequence { items }, Op::Sequence { items: ops }) => {
            let skip = usize::from(matches!(items.first(), Some(GrammarNode::RawPrefix(_))));
            if ops.len() + skip != items.len() {
                return Err(WireError::Evidence);
            }
            let mut values = Vec::with_capacity(ops.len());
            let mut offset = base;
            for (index, item) in items.iter().enumerate().skip(skip) {
                values.push(build(
                    env,
                    state,
                    item,
                    &ops[index - skip],
                    &extended(path, Step::Child(index)),
                    ctx,
                    offset,
                )?);
                offset += count(item);
            }
            WireValue::Sequence(values)
        }
        (GrammarNode::Repeat { body, .. }, Op::Repeat { body: op, .. })
        | (GrammarNode::Repeat { body, .. }, Op::CountedRepeat { body: op, .. }) => {
            let length = state
                .count(path, ctx)
                .ok_or_else(|| unsupported("repeat length was not resolved"))?;
            let mut values = Vec::with_capacity(length);
            for index in 0..length {
                let mut occurrence = ctx.to_vec();
                occurrence.push(index);
                values.push(build(
                    env,
                    state,
                    body,
                    op,
                    &extended(path, Step::Body),
                    &occurrence,
                    base,
                )?);
            }
            WireValue::Repeat(values)
        }
        (GrammarNode::Choice { arms, .. }, op) => {
            let arm = state
                .arm(path, ctx)
                .ok_or_else(|| unsupported("choice arm was not resolved"))?;
            let selected = arms.get(arm).ok_or(WireError::Evidence)?;
            let arm_op = match op {
                Op::Choice { arms, .. }
                | Op::DomainChoice { arms, .. }
                | Op::CounterChoice { arms, .. } => arms.get(arm).ok_or(WireError::Evidence)?,
                _ => return Err(WireError::Evidence),
            };
            let offset = base
                + arms[..arm]
                    .iter()
                    .map(|other| count(&other.body))
                    .sum::<usize>();
            let value = build(
                env,
                state,
                &selected.body,
                arm_op,
                &extended(path, Step::Arm(arm)),
                ctx,
                offset,
            )?;
            WireValue::Choice {
                arm,
                value: Box::new(value),
            }
        }
        (GrammarNode::Byte(_), Op::Byte { .. })
        | (GrammarNode::Number(_), Op::Number { .. })
        | (GrammarNode::String(_), Op::String { .. }) => {
            if let Some(value) = state.edit.leaves.get(&(base, ctx.to_vec())) {
                return Ok(value.clone());
            }
            if let Some(value) = env.base_leaf(base, ctx) {
                return Ok(value.clone());
            }
            return Err(unsupported(
                "leaf has neither a projected value nor retained bytes",
            ));
        }
        _ => return Err(WireError::Evidence),
    })
}

/// Encode one candidate emission/part and return the bytes the program writes.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn encode_candidate(
    grammar: &GrammarNode,
    program: &Program,
    part: &Part,
    fields: &Map<String, Value>,
    base: Option<&Base>,
    seed: u32,
    token: i32,
    format: FloatByteOrder,
    flatten: bool,
    rules: counted::Rules,
    policy: Option<&super::super::source_number::Policy>,
) -> Result<Vec<u8>, WireError> {
    let mut facts = Static::default();
    facts.walk(grammar, vec![]);
    let edit = assign_part(
        program,
        part,
        fields,
        base.map(|base| &base.controls),
        flatten,
        rules,
    )
    .map_err(|error| stage("assign", error))?;
    let mut packed = BTreeSet::new();
    {
        let mut runtime: Runtime<'_> = Runtime {
            fields_only: true,
            ..Runtime::default()
        };
        runtime.packing(&program.body);
        packed = runtime.packed_choices;
    }
    let root_arm = match &program.body {
        Op::RootChoice { cases } => {
            let arm = edit
                .controls
                .get(&(vec![], vec![]))
                .copied()
                .or_else(|| base.and_then(|base| base.controls.get(&(vec![], vec![])).copied()))
                .ok_or_else(|| unsupported("root case is not selected"))?;
            if !cases.iter().any(|case| case.plan.arm == arm) {
                return Err(WireError::Discriminant);
            }
            Some(arm)
        }
        _ => None,
    };
    let prefix = root_arm.map_or_else(Vec::new, |arm| vec![Step::Arm(arm)]);
    let local = match (root_arm, grammar) {
        (Some(arm), GrammarNode::Choice { arms, .. }) => {
            &arms.get(arm).ok_or(WireError::Evidence)?.body
        }
        (None, grammar) => grammar,
        (Some(_), _) => return Err(WireError::Evidence),
    };
    let env = Env {
        facts: &facts,
        base,
        packed,
        prefix: prefix.clone(),
        local,
    };
    let mut state = State {
        edit,
        ..State::default()
    };
    let value = match (&program.body, grammar) {
        (Op::RootChoice { cases }, GrammarNode::Choice { arms, .. }) => {
            let arm = root_arm.ok_or(WireError::Evidence)?;
            state.arms.insert((vec![], vec![]), arm);
            let case = cases
                .iter()
                .find(|case| case.plan.arm == arm)
                .ok_or(WireError::Discriminant)?;
            let Support::Supported {
                program: case_program,
            } = case.support.as_ref()
            else {
                return Err(unsupported("selected root case is not executable"));
            };
            let body = arms.get(arm).ok_or(WireError::Evidence)?;
            plan(
                &env,
                &mut state,
                &body.body,
                &case_program.body,
                &prefix,
                &Vec::new(),
            )
            .map_err(|error| stage("plan-root", error))?;
            apply_bits(&env, &mut state).map_err(|error| stage("bits-root", error))?;
            let offset = arms[..arm]
                .iter()
                .map(|other| count(&other.body))
                .sum::<usize>();
            let inner = build(
                &env,
                &state,
                &body.body,
                &case_program.body,
                &prefix,
                &Vec::new(),
                offset,
            )
            .map_err(|error| stage("build-root", error))?;
            WireValue::Choice {
                arm,
                value: Box::new(inner),
            }
        }
        (body_op, grammar) => {
            plan(&env, &mut state, grammar, body_op, &Vec::new(), &Vec::new())
                .map_err(|error| stage("plan", error))?;
            apply_bits(&env, &mut state).map_err(|error| stage("bits", error))?;
            build(&env, &state, grammar, body_op, &Vec::new(), &Vec::new(), 0)
                .map_err(|error| stage("build", error))?
        }
    };
    let forms = base
        .and_then(|b| b.source.as_ref())
        .map(|s| super::super::source_number::reconcile(&s.forms, &s.value, &value));
    super::super::source_number::encode(
        program,
        &value,
        seed,
        token,
        format,
        policy,
        forms.as_ref(),
    )
    .map_err(|error| stage("encode", error))
}

/// The projection must reproduce the requested fields exactly.
#[allow(clippy::too_many_arguments)]
fn verify(
    slot: &WireSlot,
    program: &Program,
    emission: usize,
    bytes: &[u8],
    fields: &Map<String, Value>,
    seed: u32,
    token: i32,
    format: FloatByteOrder,
    policy: Option<&super::super::source_number::Policy>,
) -> Result<WireValue, WireError> {
    let source = super::super::source_number::decode(program, bytes, token, format, policy)?;
    let value = source.value;
    let decoded = DecodedSlot {
        positional: 0,
        descriptor: slot.contract.descriptor,
        address: slot.address.clone(),
        captured_seed: Some(seed),
        raw: Some(bytes.to_vec()),
        outcome: Outcome::Decoded {
            value: value.clone(),
            exact_reencode: false,
        },
        selection: Some(Selection {
            emissions: vec![emission],
            proof: SelectionProof::IdenticalPrograms,
            root_case: None,
        }),
    };
    let named = super::project_source_with_options(
        slot,
        &decoded,
        token,
        &Ok(format),
        Some((emission, program)),
        ProjectOptions {
            metadata: false,
            verify: false,
            constants: None,
        },
        policy.map(|p| (p, &source.forms)),
    );
    if named.status != NamedStatus::Decoded {
        return Err(unsupported("decoded candidate did not project"));
    }
    if named.fields.as_ref() != Some(&canonical_fields(fields)) {
        return Err(WireError::Reencode);
    }
    Ok(value)
}

fn seed_of(raw: &[u8]) -> Result<u32, WireError> {
    let bytes: [u8; 4] = raw
        .get(..4)
        .ok_or(WireError::Truncated)?
        .try_into()
        .map_err(|_| WireError::Truncated)?;
    Ok(u32::from_be_bytes(bytes))
}

/// One named signal's wire bytes.
///
/// With retained bytes, every fact the display does not describe is preserved
/// (random prefix, unprojected leaves, flush arms). Without them, a complete
/// value is built from the fields, using the compiled count and counter
/// proofs.
pub(crate) fn encode_signal_source(
    slot: &WireSlot,
    fields: &Map<String, Value>,
    original: Option<&[u8]>,
    token: i32,
    format: FloatByteOrder,
    policies: Option<&super::super::source_number::Policies>,
) -> Result<Vec<u8>, WireError> {
    let naming = slot
        .naming
        .as_ref()
        .ok_or_else(|| unsupported("signal has no named projection"))?;
    if naming.mapping_error.is_some() {
        return Err(unsupported("signal projection is incomplete"));
    }
    let mut last = unsupported("no emission can encode this signal");
    // Retained bytes first: they preserve every fact the display cannot name
    // (random prefix, unprojected leaves, packed flush arms).
    if let Some(raw) = original {
        let seed = seed_of(raw)?;
        let usable = slot
            .programs
            .iter()
            .filter(|support| !matches!(support, Support::Unsupported { .. }))
            .count();
        let require_exact = usable > 1;
        for (emission, support) in slot.programs.iter().enumerate() {
            let program = match support {
                Support::Supported { program } | Support::RequiresInitializer { program, .. } => {
                    program
                }
                Support::Unsupported { .. } => continue,
            };
            let grammar = match slot.contract.emissions.get(emission) {
                Some(emission) => &emission.grammar,
                None => continue,
            };
            let policy = policies.and_then(|p| p.get(slot.contract.descriptor, emission));
            let source =
                match super::super::source_number::decode(program, raw, token, format, policy) {
                    Ok(source) => source,
                    Err(error) => {
                        last = error;
                        continue;
                    }
                };
            let value = source.value.clone();
            if require_exact
                && super::super::source_number::encode(
                    program,
                    &value,
                    seed,
                    token,
                    format,
                    policy,
                    policy.map(|_| &source.forms),
                )
                .map(|bytes| bytes == raw)
                    != Ok(true)
            {
                continue;
            }
            let Ok(mut base) = base_of(grammar, program, &value) else {
                continue;
            };
            if policy.is_some() {
                base.source = Some(source);
            }
            for part in naming.parts.iter().filter(|part| part.emission == emission) {
                // A split choice whose decoded arm is another part would
                // rewrite the message shape, not edit its values.
                if let Some(control) = &part.split_choice {
                    let key = (control.clone(), vec![]);
                    if base.controls.get(&key).is_some_and(|arm| *arm != part.part) {
                        continue;
                    }
                }
                let (flatten, rules) = part_encoding(naming, program, part);
                match encode_candidate(
                    grammar,
                    program,
                    part,
                    fields,
                    Some(&base),
                    seed,
                    token,
                    format,
                    flatten,
                    rules,
                    policy,
                )
                .and_then(|bytes| {
                    verify(
                        slot, program, emission, &bytes, fields, seed, token, format, policy,
                    )
                    .map(|_| bytes)
                }) {
                    Ok(bytes) => return Ok(bytes),
                    Err(error) => last = error,
                }
            }
        }
        // Only this slot's authorized candidates can carry retained number
        // forms. An empty map (including legacy artifacts), or another slot's
        // policy, must not change the legacy canonical reconstruction path.
        if policies.is_some_and(|policies| {
            slot.programs
                .iter()
                .enumerate()
                .any(|(emission, _)| policies.get(slot.contract.descriptor, emission).is_some())
        }) {
            return Err(last);
        }
    }
    // No retained bytes (or none decode under this session): build the signal
    // from the named fields alone with a fresh prefix.
    for (emission, support) in slot.programs.iter().enumerate() {
        let program = match support {
            Support::Supported { program } | Support::RequiresInitializer { program, .. } => {
                program
            }
            Support::Unsupported { .. } => continue,
        };
        let grammar = match slot.contract.emissions.get(emission) {
            Some(emission) => &emission.grammar,
            None => continue,
        };
        let policy = policies.and_then(|p| p.get(slot.contract.descriptor, emission));
        let seed = random_seed();
        for part in naming.parts.iter().filter(|part| part.emission == emission) {
            let (flatten, rules) = part_encoding(naming, program, part);
            match encode_candidate(
                grammar, program, part, fields, None, seed, token, format, flatten, rules, policy,
            )
            .and_then(|bytes| {
                verify(
                    slot, program, emission, &bytes, fields, seed, token, format, policy,
                )
                .map(|_| bytes)
            }) {
                Ok(bytes) => return Ok(bytes),
                Err(error) => last = error,
            }
        }
    }
    Err(last)
}

/// The projection rules that depend on the naming version: older artifacts
/// were decoded without tuple flattening or counted-array derivation.
fn part_encoding(naming: &Naming, program: &Program, part: &Part) -> (bool, counted::Rules) {
    let flatten = naming.version >= 2 && crate::signals::wire::readable::flatten(&part.fields);
    let rules = if naming.counted_arrays {
        counted::Rules::new(program, &part.fields)
    } else {
        counted::Rules::default()
    };
    (flatten, rules)
}

/// A fresh per-signal prefix. The prefix is random in the source; any value
/// round-trips as long as the program's masks do not depend on the original.
fn random_seed() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos() as u64)
            .unwrap_or(0),
    );
    hasher.finish() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_are_u32s() {
        let _ = random_seed();
    }
}
