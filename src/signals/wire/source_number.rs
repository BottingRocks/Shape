//! Private source-format number policy. Numeric bits and observed wire form
//! are separate facts; none of this state changes the primitive public API.
use super::*;
use crate::signals::{
    codec::{SignalReader, SignalWriter},
    extract::contracts::{GrammarNode, WriteNode},
    schema::{SourceIdentity, SourceSpan},
};
use oxc_allocator::Allocator;
use oxc_ast::ast::Expression;
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const KEY: &str = "sourceNumberRepresentation";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    source: SourceIdentity,
    native: NumberFloatMetadata,
    entries: Vec<Entry>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    slot: usize,
    descriptor: u32,
    emission: usize,
    root: Option<usize>,
    grammar: ControlPath,
    program: Vec<usize>,
    call: SourceSpan,
    value: SourceSpan,
    writer: AlgorithmProof,
    contract_digest: String,
    program_digest: String,
    mask: WireMask,
    domain: Domain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Domain {
    Canonical,
    NativeInput,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct Policies(BTreeMap<(u32, usize), Policy>);
#[derive(Clone, Debug, Default)]
pub(crate) struct Policy {
    paths: BTreeSet<Vec<usize>>,
    program_digest: String,
}
impl Policies {
    pub(crate) fn get(&self, descriptor: u32, emission: usize) -> Option<&Policy> {
        self.0.get(&(descriptor, emission))
    }
}
fn digest<T: Serialize>(v: &T) -> Result<String, WireError> {
    Ok(algorithms::digest(
        &serde_json::to_vec(v).map_err(|_| WireError::Evidence)?,
    ))
}
fn native_ok(n: &NumberFloatMetadata, a: &AlgorithmReport) -> bool {
    let evidence = |p: &AlgorithmProof| {
        p.span.is_some_and(|s| s.start < s.end)
            && p.sha256
                .as_ref()
                .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
    };
    n.marker == 128
        && n.width == 8
        && n.writer.codec.as_deref() == Some("ieee64-number-native-conditional/2")
        && n.native_dependencies.codec.as_deref()
            == Some("native-f64-single-origin-backing-bytes/2")
        && a.integer.codec.as_deref() == Some("sign-magnitude-5-7/1")
        && n.writer.span == a.integer.span
        && n.writer.sha256 == a.integer.sha256
        && evidence(&n.writer)
        && evidence(&n.native_dependencies)
        && evidence(&a.integer)
}
fn primitive(text: &str) -> bool {
    use oxc_syntax::operator::{BinaryOperator as B, UnaryOperator as U};
    fn numeric(e: &Expression<'_>) -> bool {
        match e.without_parentheses() {
            Expression::NumericLiteral(_) | Expression::UpdateExpression(_) => true,
            Expression::UnaryExpression(u) => {
                matches!(u.operator, U::UnaryPlus | U::UnaryNegation | U::BitwiseNot)
            }
            Expression::BinaryExpression(b) => match b.operator {
                B::Addition => numeric(&b.left) && numeric(&b.right),
                B::Subtraction
                | B::Multiplication
                | B::Division
                | B::Remainder
                | B::Exponential
                | B::ShiftLeft
                | B::ShiftRight
                | B::ShiftRightZeroFill
                | B::BitwiseOR
                | B::BitwiseXOR
                | B::BitwiseAnd => true,
                _ => false,
            },
            _ => false,
        }
    }
    let arena = Allocator::default();
    let Ok(e) = Parser::new(&arena, text, SourceType::mjs()).parse_expression() else {
        return false;
    };
    numeric(&e)
}
fn domain(w: &WriteNode) -> Domain {
    if primitive(&w.value) {
        Domain::Canonical
    } else {
        Domain::NativeInput
    }
}
struct Leaf<'a> {
    w: &'a WriteNode,
    op: &'a Op,
    grammar: ControlPath,
    program: Vec<usize>,
    root: Option<usize>,
}
fn leaves<'a>(
    g: &'a GrammarNode,
    op: &'a Op,
    gp: ControlPath,
    pp: Vec<usize>,
    root: Option<usize>,
    out: &mut Vec<Leaf<'a>>,
) -> Result<(), WireError> {
    match (g, op) {
        (GrammarNode::Sequence { items }, Op::Sequence { items: ops }) => {
            let skip = usize::from(matches!(items.first(), Some(GrammarNode::RawPrefix(_))));
            if items.len() != ops.len() + skip {
                return Err(WireError::Evidence);
            }
            for (i, (g, op)) in items.iter().skip(skip).zip(ops).enumerate() {
                let mut a = gp.clone();
                a.push(ControlStep::Child(i + skip));
                let mut b = pp.clone();
                b.push(i);
                leaves(g, op, a, b, root, out)?;
            }
        }
        (
            GrammarNode::Repeat { body, .. },
            Op::Repeat { body: op, .. } | Op::CountedRepeat { body: op, .. },
        ) => {
            let mut a = gp;
            a.push(ControlStep::Body);
            let mut b = pp;
            b.push(0);
            leaves(body, op, a, b, root, out)?;
        }
        (
            GrammarNode::Choice { arms, .. },
            Op::Choice { arms: ops, .. }
            | Op::DomainChoice { arms: ops, .. }
            | Op::CounterChoice { arms: ops, .. },
        ) => {
            if arms.len() != ops.len() {
                return Err(WireError::Evidence);
            }
            for (i, (g, op)) in arms.iter().zip(ops).enumerate() {
                let mut a = gp.clone();
                a.push(ControlStep::Arm(i));
                let mut b = pp.clone();
                b.push(i);
                leaves(&g.body, op, a, b, root, out)?;
            }
        }
        (GrammarNode::Choice { arms, .. }, Op::RootChoice { cases }) => {
            for c in cases {
                if let Support::Supported { program } = c.support.as_ref() {
                    let arm = c.plan.arm;
                    let g = arms.get(arm).ok_or(WireError::Evidence)?;
                    let mut a = gp.clone();
                    a.push(ControlStep::Arm(arm));
                    let mut b = pp.clone();
                    b.push(arm);
                    leaves(&g.body, &program.body, a, b, Some(arm), out)?;
                }
            }
        }
        (GrammarNode::Number(w), Op::Number { .. }) => out.push(Leaf {
            w,
            op,
            grammar: gp,
            program: pp,
            root,
        }),
        (GrammarNode::Byte(_), Op::Byte { .. }) | (GrammarNode::String(_), Op::String { .. }) => {}
        _ => return Err(WireError::Evidence),
    }
    Ok(())
}
fn program(s: &Support) -> Option<&Program> {
    match s {
        Support::Supported { program } | Support::RequiresInitializer { program, .. } => {
            Some(program)
        }
        _ => None,
    }
}
fn entries(wire: &WireSchema) -> Result<Vec<Entry>, WireError> {
    let mut out = Vec::new();
    for (slot, s) in wire.slots.iter().enumerate() {
        for (emission, (e, support)) in s.contract.emissions.iter().zip(&s.programs).enumerate() {
            let Some(p) = program(support) else { continue };
            if !matches!(
                p.stream_scope.as_str(),
                "emission_shared_cursor" | "source_root_case"
            ) {
                continue;
            }
            let mut found = Vec::new();
            // A fallback grammar may deliberately have no source-bound structure.
            if leaves(&e.grammar, &p.body, vec![], vec![], None, &mut found).is_err() {
                continue;
            }
            for l in found {
                let Op::Number { mask, literal, .. } = l.op else {
                    unreachable!()
                };
                if l.w.writer_factory.is_some()
                    && !l
                        .w
                        .writer_factory
                        .zip(wire.algorithms.integer.span)
                        .is_some_and(|(b, s)| s.start <= b.start && b.end <= s.end)
                {
                    continue;
                }
                out.push(Entry {
                    slot,
                    descriptor: s.contract.descriptor,
                    emission,
                    root: l.root,
                    grammar: l.grammar,
                    program: l.program,
                    call: l.w.span.into(),
                    value: l.w.value_span.into(),
                    writer: wire.algorithms.integer.clone(),
                    contract_digest: digest(&s.contract)?,
                    program_digest: digest(p)?,
                    mask: mask.clone(),
                    domain: if literal.is_some()
                        || s.source_plan.get(emission).is_some_and(|p| {
                            p.counts
                                .iter()
                                .any(|c| c.write == SourceSpan::from(l.w.span))
                        }) {
                        Domain::Canonical
                    } else {
                        domain(l.w)
                    },
                });
            }
        }
    }
    Ok(out)
}
/// Generate only after the normal source compiler has finished preserving or
/// replacing recipes. Core compilation supplies accounting/call/cursor proof;
/// source-format fallback recipes cannot gain authorization by resemblance.
pub(crate) fn compile(schema: &mut SchemaDocument, source: &str) -> Result<(), WireError> {
    let Some(wire) = &schema.wire else {
        return Ok(());
    };
    let Some(native) = wire
        .number_float
        .as_ref()
        .filter(|n| native_ok(n, &wire.algorithms))
    else {
        return Ok(());
    };
    if super::float::certify(source, &wire.algorithms.integer) != *native {
        return Err(WireError::Evidence);
    }
    let mut all = entries(wire)?;
    let mut proven = BTreeSet::new();
    for (i, s) in wire.slots.iter().enumerate() {
        let core = super::programs_for(&s.contract, &wire.algorithms, source);
        for (e, (a, b)) in core.iter().zip(&s.programs).enumerate() {
            if program(a).is_some() && program(a) == program(b) {
                proven.insert((i, e));
            }
        }
    }
    all.retain(|e| proven.contains(&(e.slot, e.emission)));
    // Check concrete call origins even when the binding is a lifted factory.
    all.retain(|e| {
        let s = &wire.slots[e.slot];
        let p = program(&s.programs[e.emission]).unwrap();
        let mut ls = Vec::new();
        let _ = leaves(
            &s.contract.emissions[e.emission].grammar,
            &p.body,
            vec![],
            vec![],
            None,
            &mut ls,
        );
        ls.iter().find(|l| l.program == e.program).is_some_and(|l| {
            let call = algorithms::number_at_call(source, l.w);
            call.codec == wire.algorithms.integer.codec
                && call.span == wire.algorithms.integer.span
                && call.sha256 == wire.algorithms.integer.sha256
        })
    });
    if all.is_empty() {
        return Ok(());
    }
    let doc = Document {
        version: 1,
        source: wire.source.clone(),
        native: native.clone(),
        entries: all,
    };
    schema
        .diagnostics
        .as_object_mut()
        .ok_or(WireError::Evidence)?
        .insert(
            KEY.into(),
            serde_json::to_value(doc).map_err(|_| WireError::Evidence)?,
        );
    validate(schema)?;
    Ok(())
}
pub(crate) fn validate(schema: &SchemaDocument) -> Result<Policies, WireError> {
    let Some(value) = schema.diagnostics.get(KEY) else {
        return Ok(Policies::default());
    };
    let doc: Document = serde_json::from_value(value.clone()).map_err(|_| WireError::Evidence)?;
    let wire = schema.wire.as_ref().ok_or(WireError::Evidence)?;
    if doc.version != 1
        || doc.source != wire.source
        || wire.number_float.as_ref() != Some(&doc.native)
        || !native_ok(&doc.native, &wire.algorithms)
    {
        return Err(WireError::Evidence);
    }
    for proof in [
        &doc.native.writer,
        &doc.native.native_dependencies,
        &wire.algorithms.integer,
    ] {
        if proof
            .span
            .is_none_or(|s| s.end as usize > wire.source.bytes)
        {
            return Err(WireError::Evidence);
        }
    }
    let expected = entries(wire)?;
    let mut seen = BTreeSet::new();
    let mut policies = Policies::default();
    for e in doc.entries {
        if !seen.insert((e.slot, e.emission, e.program.clone())) || !expected.contains(&e) {
            return Err(WireError::Evidence);
        }
        if e.domain == Domain::NativeInput {
            let p = policies.0.entry((e.descriptor, e.emission)).or_default();
            if !p.program_digest.is_empty() && p.program_digest != e.program_digest {
                return Err(WireError::Evidence);
            }
            p.program_digest = e.program_digest;
            p.paths.insert(e.program);
        }
    }
    Ok(policies)
}
pub(crate) fn regenerate(schema: &SchemaDocument, source: &str) -> Result<(), WireError> {
    if schema.diagnostics.get(KEY).is_none() {
        return Ok(());
    }
    let mut rebuilt = schema.clone();
    rebuilt
        .diagnostics
        .as_object_mut()
        .ok_or(WireError::Evidence)?
        .remove(KEY);
    compile(&mut rebuilt, source)?;
    if rebuilt.diagnostics.get(KEY) != schema.diagnostics.get(KEY) {
        return Err(WireError::Evidence);
    }
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Location {
    path: Vec<usize>,
    occurrences: Vec<usize>,
}
#[derive(Clone, Debug)]
struct Form {
    bits: u64,
    ieee: bool,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct Forms {
    program: String,
    values: BTreeMap<Location, Form>,
}
#[derive(Clone, Debug)]
pub(crate) struct State {
    pub(crate) value: WireValue,
    pub(crate) forms: Forms,
}
#[derive(Clone)]
pub(super) struct Execution {
    policy: Policy,
    forms: Forms,
    used: BTreeSet<Location>,
    pub(super) occurrences: Vec<usize>,
}
impl Execution {
    pub(super) fn new(
        p: &Program,
        policy: &Policy,
        forms: Option<&Forms>,
    ) -> Result<Self, WireError> {
        if digest(p)? != policy.program_digest {
            return Err(WireError::Evidence);
        }
        if forms.is_some_and(|f| f.program != policy.program_digest) {
            return Err(WireError::Evidence);
        }
        Ok(Self {
            policy: policy.clone(),
            forms: forms.cloned().unwrap_or(Forms {
                program: policy.program_digest.clone(),
                ..Forms::default()
            }),
            used: BTreeSet::new(),
            occurrences: vec![],
        })
    }
    pub(super) fn reader(mut self) -> Self {
        self.forms.values.clear();
        self.used.clear();
        self.occurrences.clear();
        self
    }
    fn location(&self, path: &[usize]) -> Location {
        Location {
            path: path.to_vec(),
            occurrences: self.occurrences.clone(),
        }
    }
    pub(super) fn read(
        &mut self,
        r: &mut SignalReader<'_>,
        path: &[usize],
        format: &Result<FloatByteOrder, WireError>,
    ) -> Result<f64, WireError> {
        if !self.policy.paths.contains(path) {
            return super::float::read(r, format);
        }
        let ieee = r.clone().peek_tag(128).map_err(WireError::codec)?;
        let n = if ieee {
            r.number_with_float_order(
                *format.as_ref().map_err(Clone::clone)? == FloatByteOrder::Big,
            )
            .map_err(WireError::codec)?
        } else {
            super::float::read(r, format)?
        };
        let key = self.location(path);
        if self
            .forms
            .values
            .insert(
                key.clone(),
                Form {
                    bits: n.to_bits(),
                    ieee,
                },
            )
            .is_some()
        {
            return Err(WireError::Evidence);
        }
        self.used.insert(key);
        Ok(n)
    }
    pub(super) fn write(
        &mut self,
        w: &mut SignalWriter,
        bits: u64,
        path: &[usize],
        format: &Result<FloatByteOrder, WireError>,
    ) -> Result<(), WireError> {
        let key = self.location(path);
        if let Some(f) = self.forms.values.get(&key) {
            if !self.policy.paths.contains(path) || f.bits != bits || !self.used.insert(key) {
                return Err(WireError::Evidence);
            }
            if f.ieee {
                let order = format.as_ref().map_err(Clone::clone)?;
                w.byte(128);
                for b in if *order == FloatByteOrder::Big {
                    bits.to_be_bytes()
                } else {
                    bits.to_le_bytes()
                } {
                    w.byte(b)
                }
                return Ok(());
            }
            if !super::float::integer_domain(f64::from_bits(bits)) {
                return Err(WireError::Evidence);
            }
        }
        super::float::write(w, bits, format)
    }
    pub(super) fn finish(self) -> Result<Forms, WireError> {
        if self.used.len() != self.forms.values.len() {
            return Err(WireError::Evidence);
        }
        Ok(self.forms)
    }
}
fn format(order: FloatByteOrder) -> Result<FloatByteOrder, WireError> {
    if order == FloatByteOrder::Unknown {
        Err(WireError::Unsupported(
            "native float byte order must be explicitly configured".into(),
        ))
    } else {
        Ok(order)
    }
}
pub(crate) fn decode(
    p: &Program,
    bytes: &[u8],
    token: i32,
    order: FloatByteOrder,
    policy: Option<&Policy>,
) -> Result<State, WireError> {
    let execution = policy
        .map(|policy| Execution::new(p, policy, None))
        .transpose()?;
    let (value, forms) = p.decode_source(bytes, token, &format(order), execution, &[])?;
    Ok(State { value, forms })
}
pub(crate) fn encode(
    p: &Program,
    value: &WireValue,
    seed: u32,
    token: i32,
    order: FloatByteOrder,
    policy: Option<&Policy>,
    forms: Option<&Forms>,
) -> Result<Vec<u8>, WireError> {
    if policy.is_none() && forms.is_some_and(|f| !f.values.is_empty()) {
        return Err(WireError::Evidence);
    }
    p.encode_source(
        value,
        seed,
        token,
        &format(order),
        policy.map(|p0| Execution::new(p, p0, forms)).transpose()?,
        &[],
    )
}
/// Retain a form only at the same executed occurrence in an unchanged
/// structural subtree and with exactly equal numeric bits. A resized repeat
/// or changed choice discards the complete replacement subtree's forms.
pub(crate) fn reconcile(forms: &Forms, old: &WireValue, new: &WireValue) -> Forms {
    // Without an explicit occurrence mapping, a changed row is a replacement.
    // Retain forms only for a bit-exact whole occurrence, including numeric IDs.
    fn walk(
        a: &WireValue,
        b: &WireValue,
        path: &mut Vec<usize>,
        occ: &mut Vec<usize>,
        keep: &mut BTreeSet<Location>,
    ) {
        match (a, b) {
            (WireValue::NumberBits(a), WireValue::NumberBits(b)) if a == b => {
                keep.insert(Location {
                    path: path.clone(),
                    occurrences: occ.clone(),
                });
            }
            (WireValue::Sequence(a), WireValue::Sequence(b)) if a.len() == b.len() => {
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    path.push(i);
                    walk(a, b, path, occ, keep);
                    path.pop();
                }
            }
            (WireValue::Repeat(a), WireValue::Repeat(b)) if a.len() == b.len() => {
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    if a != b {
                        continue;
                    }
                    path.push(0);
                    occ.push(i);
                    walk(a, b, path, occ, keep);
                    occ.pop();
                    path.pop();
                }
            }
            (WireValue::Choice { arm: a, value: x }, WireValue::Choice { arm: b, value: y })
                if a == b =>
            {
                path.push(*a);
                walk(x, y, path, occ, keep);
                path.pop();
            }
            _ => {}
        }
    }
    let mut keep = BTreeSet::new();
    walk(old, new, &mut vec![], &mut vec![], &mut keep);
    Forms {
        program: forms.program.clone(),
        values: forms
            .values
            .iter()
            .filter(|(k, _)| keep.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    }
}

#[derive(Clone)]
pub(super) struct Physical {
    numbers: Vec<Option<Form>>,
    index: usize,
}
impl Forms {
    pub(super) fn physical(&self, value: &WireValue) -> Physical {
        fn walk(
            value: &WireValue,
            path: &mut Vec<usize>,
            occ: &mut Vec<usize>,
            forms: &Forms,
            out: &mut Vec<Option<Form>>,
        ) {
            match value {
                WireValue::NumberBits(_) => out.push(
                    forms
                        .values
                        .get(&Location {
                            path: path.clone(),
                            occurrences: occ.clone(),
                        })
                        .cloned(),
                ),
                WireValue::Sequence(v) => {
                    for (i, v) in v.iter().enumerate() {
                        path.push(i);
                        walk(v, path, occ, forms, out);
                        path.pop();
                    }
                }
                WireValue::Repeat(v) => {
                    for (i, v) in v.iter().enumerate() {
                        path.push(0);
                        occ.push(i);
                        walk(v, path, occ, forms, out);
                        occ.pop();
                        path.pop();
                    }
                }
                WireValue::Choice { arm, value } => {
                    path.push(*arm);
                    walk(value, path, occ, forms, out);
                    path.pop();
                }
                _ => {}
            }
        }
        let mut numbers = Vec::new();
        walk(value, &mut vec![], &mut vec![], self, &mut numbers);
        Physical { numbers, index: 0 }
    }
}
impl Physical {
    pub(super) fn read(
        &mut self,
        r: &mut SignalReader<'_>,
        bits: u64,
        format: &Result<FloatByteOrder, WireError>,
    ) -> Result<f64, WireError> {
        let form = self.numbers.get(self.index).ok_or(WireError::Evidence)?;
        self.index += 1;
        if let Some(f) = form {
            if f.bits != bits || r.clone().peek_tag(128).map_err(WireError::codec)? != f.ieee {
                return Err(WireError::Evidence);
            }
            if f.ieee {
                return r
                    .number_with_float_order(
                        *format.as_ref().map_err(Clone::clone)? == FloatByteOrder::Big,
                    )
                    .map_err(WireError::codec);
            }
        }
        super::float::read(r, format)
    }
    pub(super) fn finish(&self) -> Result<(), WireError> {
        if self.index != self.numbers.len() {
            Err(WireError::Evidence)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(body: Op) -> Program {
        Program {
            prefix_byte_order: "be32".into(),
            seed_source: "captured_prefix".into(),
            stream_scope: "emission_shared_cursor".into(),
            string_scope: "emission_local_writer_instance".into(),
            salt: 0,
            body,
        }
    }
    fn number(mask: WireMask) -> Op {
        Op::Number {
            envelope: None,
            mask,
            literal: None,
        }
    }
    fn policy(p: &Program, paths: Vec<Vec<usize>>) -> Policy {
        Policy {
            paths: paths.into_iter().collect(),
            program_digest: digest(p).unwrap(),
        }
    }
    fn ieee(bits: u64, order: FloatByteOrder) -> Vec<u8> {
        let mut v = vec![0, 0, 0, 0, 128];
        v.extend(if order == FloatByteOrder::Big {
            bits.to_be_bytes()
        } else {
            bits.to_le_bytes()
        });
        v
    }
    #[test]
    fn source_number_independent_vectors_keep_primitive_canonical_checks() {
        // IEEE known-answer bit patterns, independent of SignalWriter::number.
        for order in [FloatByteOrder::Little, FloatByteOrder::Big] {
            for mask in [WireMask::None, WireMask::Inherit] {
                let p = p(number(mask));
                let policy = policy(&p, vec![vec![]]);
                for bits in [
                    0,
                    0x8000000000000000,
                    0x3ff0000000000000,
                    0x41dfffffffc00000,
                    0xc1e0000000000000,
                    0x3ff8000000000000,
                    0x41e0000000000000,
                    1,
                    0x7ff0000000000000,
                    0xfff0000000000000,
                    0x7ff8000000000021,
                ] {
                    let raw = ieee(bits, order);
                    let state = decode(&p, &raw, 0, order, Some(&policy)).unwrap();
                    assert_eq!(state.value, WireValue::NumberBits(bits));
                    assert_eq!(
                        encode(
                            &p,
                            &state.value,
                            0,
                            0,
                            order,
                            Some(&policy),
                            Some(&state.forms)
                        )
                        .unwrap(),
                        raw
                    );
                    if super::super::float::integer_domain(f64::from_bits(bits)) {
                        assert_eq!(
                            super::super::explicit_initializer::decode(&p, &raw, 0, order),
                            Err(WireError::Codec)
                        );
                    }
                }
                let raw = vec![0, 0, 0, 0, 1];
                let state = decode(&p, &raw, 0, order, Some(&policy)).unwrap();
                assert_eq!(
                    encode(
                        &p,
                        &state.value,
                        0,
                        0,
                        order,
                        Some(&policy),
                        Some(&state.forms)
                    )
                    .unwrap(),
                    raw
                );
                assert!(decode(
                    &p,
                    &ieee(0, order),
                    0,
                    FloatByteOrder::Unknown,
                    Some(&policy)
                )
                .is_err());
                for end in 4..13 {
                    assert!(decode(&p, &ieee(0, order)[..end], 0, order, Some(&policy)).is_err());
                }
                assert_eq!(
                    decode(&p, &[0, 0, 0, 0, 129], 0, order, Some(&policy)).unwrap_err(),
                    WireError::Codec
                );
                let mut extra = ieee(0, order);
                extra.push(1);
                assert!(matches!(
                    decode(&p, &extra, 0, order, Some(&policy)),
                    Err(WireError::Trailing(1))
                ));
            }
        }
    }
    #[test]
    fn source_number_occurrences_reconcile_bits_and_refuse_stale_forms() {
        let p = p(Op::Sequence {
            items: vec![
                Op::Repeat {
                    count: 3,
                    direction: crate::signals::extract::contracts::LoopDirection::Ascending,
                    body: Box::new(number(WireMask::Inherit)),
                },
                Op::Byte {
                    mask: WireMask::Inherit,
                    xor: 0,
                    allowed: Some(vec![23]),
                },
            ],
        });
        let policy = policy(&p, vec![vec![0, 0]]);
        let mut raw = vec![0, 0, 0, 0, 1, 128];
        raw.extend(1f64.to_le_bytes());
        raw.push(128);
        raw.extend((-0f64).to_le_bytes());
        raw.push(23);
        let state = decode(&p, &raw, 0, FloatByteOrder::Little, Some(&policy)).unwrap();
        assert_eq!(state.forms.values.len(), 3);
        assert_eq!(
            encode(
                &p,
                &state.value,
                0,
                0,
                FloatByteOrder::Little,
                Some(&policy),
                Some(&state.forms)
            )
            .unwrap(),
            raw
        );
        let mut edited = state.value.clone();
        let WireValue::Sequence(items) = &mut edited else {
            panic!()
        };
        let WireValue::Repeat(v) = &mut items[0] else {
            panic!()
        };
        v[0] = WireValue::NumberBits(2f64.to_bits());
        assert!(encode(
            &p,
            &edited,
            0,
            0,
            FloatByteOrder::Little,
            Some(&policy),
            Some(&state.forms)
        )
        .is_err());
        let forms = reconcile(&state.forms, &state.value, &edited);
        let bytes = encode(
            &p,
            &edited,
            0,
            0,
            FloatByteOrder::Little,
            Some(&policy),
            Some(&forms),
        )
        .unwrap();
        let mut want = raw.clone();
        want[4] = 2;
        assert_eq!(bytes, want);
        let WireValue::Sequence(items) = &mut edited else {
            panic!()
        };
        let WireValue::Repeat(v) = &mut items[0] else {
            panic!()
        };
        v.pop();
        assert!(reconcile(&state.forms, &state.value, &edited)
            .values
            .is_empty());
        let mut extra = state.forms.clone();
        extra.values.insert(
            Location {
                path: vec![99],
                occurrences: vec![],
            },
            Form {
                bits: 0,
                ieee: true,
            },
        );
        assert!(encode(
            &p,
            &state.value,
            0,
            0,
            FloatByteOrder::Little,
            Some(&policy),
            Some(&extra)
        )
        .is_err());
        let mut wrong = policy.clone();
        wrong.paths.clear();
        assert!(encode(
            &p,
            &state.value,
            0,
            0,
            FloatByteOrder::Little,
            Some(&wrong),
            Some(&state.forms)
        )
        .is_err());
        let literal = p::literal();
        let literal_policy = self::policy(&literal, vec![vec![]]);
        assert!(decode(
            &literal,
            &ieee(1f64.to_bits(), FloatByteOrder::Little),
            0,
            FloatByteOrder::Little,
            Some(&literal_policy)
        )
        .is_err());
    }
    #[test]
    fn source_number_row_replacements_drop_forms_without_touching_siblings() {
        let row = Op::Sequence {
            items: vec![
                Op::String {
                    mask: WireMask::Inherit,
                    literal: None,
                },
                number(WireMask::Inherit),
            ],
        };
        let p = p(Op::Sequence {
            items: vec![
                Op::Repeat {
                    count: 2,
                    direction: crate::signals::extract::contracts::LoopDirection::Ascending,
                    body: Box::new(row),
                },
                number(WireMask::Inherit),
            ],
        });
        let policy = policy(&p, vec![vec![0, 0, 1], vec![1]]);
        let mut raw = vec![0, 0, 0, 0, 97, 0, 128];
        raw.extend(1f64.to_le_bytes());
        raw.extend([98, 0, 1, 128]);
        raw.extend(1f64.to_le_bytes());
        let state = decode(&p, &raw, 0, FloatByteOrder::Little, Some(&policy)).unwrap();
        assert_eq!(
            encode(
                &p,
                &state.value,
                0,
                0,
                FloatByteOrder::Little,
                Some(&policy),
                Some(&reconcile(&state.forms, &state.value, &state.value))
            )
            .unwrap(),
            raw
        );
        let mut swapped = state.value.clone();
        let WireValue::Sequence(v) = &mut swapped else {
            panic!()
        };
        let WireValue::Repeat(rows) = &mut v[0] else {
            panic!()
        };
        rows.reverse();
        let forms = reconcile(&state.forms, &state.value, &swapped);
        assert_eq!(forms.values.len(), 1);
        let mut want = vec![0, 0, 0, 0, 98, 0, 1, 97, 0, 1, 128];
        want.extend(1f64.to_le_bytes());
        assert_eq!(
            encode(
                &p,
                &swapped,
                0,
                0,
                FloatByteOrder::Little,
                Some(&policy),
                Some(&forms)
            )
            .unwrap(),
            want
        );
    }
    #[test]
    fn source_number_root_and_choice_context_with_unmasked_sentinel() {
        let arm = |tag| Op::Sequence {
            items: vec![
                Op::Byte {
                    mask: WireMask::Inherit,
                    xor: 0,
                    allowed: Some(vec![tag]),
                },
                number(WireMask::Inherit),
            ],
        };
        let inner = p(Op::Sequence {
            items: vec![
                Op::Choice {
                    discriminants: vec![vec![1], vec![2]],
                    mask: WireMask::Inherit,
                    xor: 0,
                    arms: vec![arm(1), arm(2)],
                },
                Op::Byte {
                    mask: WireMask::None,
                    xor: 0,
                    allowed: Some(vec![23]),
                },
            ],
        });
        let case = |arm, minimum, maximum, program| RootCase {
            plan: RootCasePlan {
                source_choice: Some(SourceSpan::new(1, 2)),
                arm,
                prefix: SourceSpan::new(3, 4),
                stream: SourceSpan::new(5, 6),
                seed_binding: SourceSpan::new(7, 8),
                salt: 0,
                minimum_length: minimum,
                maximum_length: Some(maximum),
            },
            support: Box::new(Support::Supported { program }),
        };
        let p = p(Op::RootChoice {
            cases: vec![
                case(0, 15, 15, inner),
                case(
                    1,
                    5,
                    5,
                    p(Op::Byte {
                        mask: WireMask::Inherit,
                        xor: 0,
                        allowed: Some(vec![9]),
                    }),
                ),
            ],
        });
        let policy = policy(&p, vec![vec![0, 0, 0, 1], vec![0, 0, 1, 1]]);
        for tag in [1, 2] {
            let mut raw = vec![0, 0, 0, 0, tag, 128];
            raw.extend(1f64.to_le_bytes());
            raw.push(23);
            let state = decode(&p, &raw, 0, FloatByteOrder::Little, Some(&policy)).unwrap();
            assert_eq!(
                encode(
                    &p,
                    &state.value,
                    0,
                    0,
                    FloatByteOrder::Little,
                    Some(&policy),
                    Some(&state.forms)
                )
                .unwrap(),
                raw
            );
        }
    }
    #[test]
    fn source_number_independent_public_reference_corpus() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/source-number-goldens.json"
        ))
        .unwrap();
        for s in corpus["sequences"].as_array().unwrap() {
            let order = if s["order"] == "big" {
                FloatByteOrder::Big
            } else {
                FloatByteOrder::Little
            };
            let mut paths = Vec::new();
            let mut values = Vec::new();
            let mut ops = Vec::new();
            for (i, op) in s["ops"].as_array().unwrap().iter().enumerate() {
                if op["kind"] == "number" {
                    ops.push(number(WireMask::None));
                    paths.push(vec![i]);
                    values.push(WireValue::NumberBits(
                        u64::from_str_radix(op["bits"].as_str().unwrap(), 16).unwrap(),
                    ));
                } else {
                    let byte = op["byte"].as_u64().unwrap() as u8;
                    ops.push(Op::Byte {
                        mask: WireMask::None,
                        xor: 0,
                        allowed: Some(vec![byte]),
                    });
                    values.push(WireValue::Byte(byte));
                }
            }
            let p = p(Op::Sequence { items: ops });
            let policy = policy(&p, paths);
            let raw: Vec<u8> = s["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            let state = decode(&p, &raw, 0, order, Some(&policy)).unwrap();
            assert_eq!(state.value, WireValue::Sequence(values));
            let seed = u32::from_be_bytes(raw[..4].try_into().unwrap());
            assert_eq!(
                encode(
                    &p,
                    &state.value,
                    seed,
                    0,
                    order,
                    Some(&policy),
                    Some(&state.forms)
                )
                .unwrap(),
                raw
            );
        }
        for v in corpus["bitVectors"].as_array().unwrap() {
            let order = if v["order"] == "big" {
                FloatByteOrder::Big
            } else {
                FloatByteOrder::Little
            };
            let p = p(number(WireMask::None));
            let policy = policy(&p, vec![vec![]]);
            let raw: Vec<u8> = v["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            let state = decode(&p, &raw, 0, order, Some(&policy)).unwrap();
            assert_eq!(
                state.value,
                WireValue::NumberBits(
                    u64::from_str_radix(v["bits"].as_str().unwrap(), 16).unwrap()
                )
            );
            assert_eq!(
                encode(
                    &p,
                    &state.value,
                    u32::from_be_bytes(raw[..4].try_into().unwrap()),
                    0,
                    order,
                    Some(&policy),
                    Some(&state.forms)
                )
                .unwrap(),
                raw
            );
        }
    }
    #[test]
    fn source_number_source_policy_coercing_expressions_and_captured_mutations() {
        crate::run_with_large_stack(|| {
            use crate::signals::codec::{hash_string_token, SignalWriter};
            use crate::signals::decode::DecodeArtifact;
            let stream = include_str!("reference/stream.js")
                .replace("local0(", "streamFactory(")
                .replace("local9", "tokenState");
            let hash = include_str!("reference/hash.js").replace("local0(", "tokenHash(");
            let number = include_str!("reference/number.js").replace("local0(", "integerWriter(");
            let bindings = (0..32)
                .map(|i| format!("scope{i}={i}"))
                .collect::<Vec<_>>()
                .join(",");
            let reads = (0..32)
                .map(|i| format!("scope{i}"))
                .collect::<Vec<_>>()
                .join(",");
            let base=include_str!("../../../tests/fixtures/signal-wire-roundtrip/program.js")
                .replace("__STREAM__",&stream).replace("__HASH__",&hash)
                .replace("function main() {",&format!("function main() {{const local10=new globalThis.Float64Array(1);const local11=globalThis.Uint8Array;const local12=[];{number}"))
                .replace("function Session(token) {",&format!("function Session(token) {{let {bindings};void [{reads}];const navigator=globalThis.navigator;let captured=0;"));
            let session=serde_json::from_value(serde_json::json!({"transactionToken":"ab","alphabet":"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/","shuffleKey":[],"keys":[0,0,0,0,0,0,0,0]})).unwrap();
            for (expr, bits, wide, mutation) in [
                ("true", 1f64.to_bits(), true, "true"),
                ("!0", 1f64.to_bits(), true, "true"),
                ("false", 0, true, "true"),
                ("1===2", 0, true, "true"),
                ("null", 0, true, "true"),
                ("\"1\"", 1f64.to_bits(), true, "true"),
                ("navigator.deviceMemory+\"\"", 1f64.to_bits(), true, "true"),
                ("captured", 1f64.to_bits(), true, "true"),
                ("captured", 1f64.to_bits(), true, "\"1\""),
                ("1", 1f64.to_bits(), false, "true"),
                ("+navigator.deviceMemory", 1f64.to_bits(), false, "true"),
                ("navigator.deviceMemory-0", 1f64.to_bits(), false, "true"),
            ] {
                let source=base.replace("b[4]=9^mask();",&format!("let p=3;p=integerWriter(mask,navigator.deviceMemory,b,p);p=integerWriter(mask,{expr},b,p);b[++p]=7^mask();"))
                    .replace("let registry=",&format!("captured={mutation};let registry="));
                let a = DecodeArtifact::compile(
                    &source,
                    &session,
                    "example".into(),
                    FloatByteOrder::Little,
                )
                .unwrap();
                let a = DecodeArtifact::from_json(&serde_json::to_vec(&a).unwrap()).unwrap();
                let wire = a.schema.wire.as_ref().unwrap();
                let slot = wire
                    .slots
                    .iter()
                    .find(|s| matches!(s.routing, WireRoute::Positional { positional: 1 }))
                    .unwrap();
                let p = program(&slot.programs[0]).unwrap();
                let policies = validate(&a.schema).unwrap();
                let policy = policies.get(slot.contract.descriptor, 0).unwrap();
                let entry = a.schema.diagnostics[KEY]["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|e| e["program"] == serde_json::json!([1]))
                    .unwrap();
                assert_eq!(
                    entry["domain"],
                    serde_json::json!(if wide { "native_input" } else { "canonical" }),
                    "{expr}"
                );
                let token = hash_string_token("ab");
                let mut w = SignalWriter::new(token, 123, 17);
                w.number(1.);
                w.byte(128);
                for b in bits.to_le_bytes() {
                    w.byte(b)
                }
                w.byte(7);
                let raw = w.finish();
                if wide {
                    let state =
                        decode(p, &raw, token, FloatByteOrder::Little, Some(policy)).unwrap();
                    assert_eq!(
                        state.value,
                        WireValue::Sequence(vec![
                            WireValue::NumberBits(1f64.to_bits()),
                            WireValue::NumberBits(bits),
                            WireValue::Byte(7)
                        ])
                    );
                    assert_eq!(
                        encode(
                            p,
                            &state.value,
                            123,
                            token,
                            FloatByteOrder::Little,
                            Some(policy),
                            Some(&state.forms)
                        )
                        .unwrap(),
                        raw
                    );
                } else {
                    assert!(
                        decode(p, &raw, token, FloatByteOrder::Little, Some(policy)).is_err(),
                        "{expr}"
                    );
                }
            }
        });
    }
    #[test]
    fn source_number_numeric_row_ids_replacement_edit_and_unchanged_neighbors() {
        let row = Op::Sequence {
            items: vec![number(WireMask::Inherit), number(WireMask::Inherit)],
        };
        let p = p(Op::Sequence {
            items: vec![
                Op::Repeat {
                    count: 2,
                    direction: crate::signals::extract::contracts::LoopDirection::Ascending,
                    body: Box::new(row),
                },
                number(WireMask::Inherit),
            ],
        });
        let policy = policy(&p, vec![vec![0, 0, 0], vec![0, 0, 1], vec![1]]);
        let mut raw = vec![0, 0, 0, 0, 10, 128];
        raw.extend(1f64.to_le_bytes());
        raw.extend([20, 1, 128]);
        raw.extend(1f64.to_le_bytes());
        let state = decode(&p, &raw, 0, FloatByteOrder::Little, Some(&policy)).unwrap();
        for action in 0..3 {
            let mut edited = state.value.clone();
            let WireValue::Sequence(v) = &mut edited else {
                panic!()
            };
            let WireValue::Repeat(rows) = &mut v[0] else {
                panic!()
            };
            match action {
                0 => rows.reverse(),
                1 => {
                    let WireValue::Sequence(row) = &mut rows[0] else {
                        panic!()
                    };
                    row[0] = WireValue::NumberBits(30f64.to_bits());
                }
                _ => {
                    let WireValue::Sequence(row) = &mut rows[0] else {
                        panic!()
                    };
                    row[1] = WireValue::NumberBits(2f64.to_bits());
                }
            }
            let forms = reconcile(&state.forms, &state.value, &edited);
            assert_eq!(forms.values.len(), if action == 0 { 1 } else { 3 });
            let encoded = encode(
                &p,
                &edited,
                0,
                0,
                FloatByteOrder::Little,
                Some(&policy),
                Some(&forms),
            )
            .unwrap();
            let mut want = vec![0, 0, 0, 0];
            want.extend(match action {
                0 => vec![20, 1, 10, 1],
                1 => vec![30, 1, 20, 1],
                _ => vec![10, 2, 20, 1],
            });
            want.push(128);
            want.extend(1f64.to_le_bytes());
            assert_eq!(encoded, want);
        }
    }
    mod p {
        use super::*;
        pub(super) fn literal() -> Program {
            p(Op::Number {
                envelope: None,
                mask: WireMask::Inherit,
                literal: Some(1f64.to_bits()),
            })
        }
    }
}
