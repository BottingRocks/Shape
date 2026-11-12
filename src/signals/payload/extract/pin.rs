//! Structural pinning of build facts from one traced program.
//!
//! [`extract`](super::extract) proves whole-body AST equality against reviewed
//! contract templates. That is exact, but a template only covers the exact
//! emission it was written from: another seed can compile the same wire format
//! into a different statement order and still produce valid payloads.
//!
//! This module takes the route the repository uses for captured seeds: trace the
//! seed to a readable program once, then *pinpoint* the few places that define
//! the capture.
//!
//! | fact | where it is pinned |
//! |---|---|
//! | cipher IVs, counter, double rounds | the 16-word stream state and its block loop |
//! | positional count, keyed names | the report route's default filter and descriptor table |
//! | ordered omitted keysets | the constructor's `indices.map(index => table[index])` call |
//! | header prefix, chunk size, `c`/`d`/`z` | the header option assignments |
//! | alphabet, eight keys, token | the initializer, supplied by the caller |
//!
//! Nothing is compared against a template, so a seed that emits the same
//! constants in another statement order still pins. Each fact is still proved
//! structurally: the state array must have four literal IVs, eight masked key
//! slots, two zero counter words and two seed words; the rounds must be whole
//! double rounds; the filter indices must be in range; the descriptor table must
//! be static; the keyset mapping must preserve index order; `c`/`d`/`z` must be
//! literal strings. Program text is parsed, never executed.

use super::{
    ast::{member_name, member_object, within, Index},
    sha, Evidence, Session, Source,
};
use crate::signals::payload::{
    report::{ChannelReport, Report},
    CipherProfile, Error, Generator, HeaderContext, HeaderReport, Headers, PayloadCodec, Profile,
    PROFILE_FORMAT,
};
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Version of the saved seed layout artifact.
pub const SEED_LAYOUT_FORMAT: &str = "shape-seed-layout/1";

/// One traced seed, reduced to the static facts a loader needs.
///
/// The artifact separates build facts from input material: `cipher`, `header`
/// and `report` come from the program, while `session` records the literals the
/// capture was started with. A loader validates the artifact, builds a
/// [`Generator`] from it, and decodes payloads without parsing the seed again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SeedLayout {
    pub format: String,
    pub source: Source,
    pub session: Session,
    pub cipher: CipherProfile,
    pub header: HeaderLayout,
    /// Ordered omitted keysets: a decode fact, not stored on the wire.
    pub keysets: Vec<Vec<String>>,
    pub report: ReportLayout,
}

/// Header policy pinned from the source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeaderLayout {
    /// Emitted `-a` name, in the source's spelling lowercased.
    pub name: String,
    /// Prefix shared by every sibling header.
    pub prefix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
    /// The checksum policy this generator implements; one supported value.
    pub checksum: String,
    pub b: bool,
    pub f: bool,
    pub c: String,
    pub d: String,
    pub z: String,
}

/// The four source report channels, as layout rather than captured bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportLayout {
    /// Legacy layout placeholder, not a statement that runtime errors are empty.
    /// Errors carry a generic superpack array (see the compact seed schema).
    pub errors: Vec<()>,
    #[serde(rename = "positionalSignals")]
    pub positional_signals: Vec<SlotLayout>,
    pub signals: Vec<NamedSignalLayout>,
    #[serde(rename = "keyedSignals")]
    pub keyed_signals: Vec<KeyedSignalLayout>,
}

/// One positional slot: its container index and the descriptor it came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlotLayout {
    pub slot: usize,
    pub descriptor: usize,
    pub ctor: bool,
    pub priority: bool,
}

/// A string-named descriptor routed to the `signals` channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedSignalLayout {
    pub descriptor: usize,
    pub name: String,
}

/// A numeric-named descriptor routed to the `keyedSignals` channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyedSignalLayout {
    pub descriptor: usize,
    pub name: u32,
}

impl SeedLayout {
    pub fn profile(&self) -> Profile {
        Profile {
            format: PROFILE_FORMAT.to_owned(),
            cipher: self.cipher.clone(),
            alphabet: self.session.alphabet.clone(),
            keys: self.session.keys,
            header_name: self.header.name.clone(),
            transaction_token: Some(self.session.transaction_token.clone()),
            chunk_size: self.header.chunk_size,
            header_context: Some(HeaderContext {
                b: self.header.b,
                f: self.header.f,
                c: Some(self.header.c.clone()),
                d: Some(self.header.d.clone()),
                z: Some(self.header.z.clone()),
            }),
            positional_count: Some(self.report.positional_signals.len()),
            keysets: Some(self.keysets.clone()),
        }
    }

    /// Parse and validate a saved artifact.
    pub fn from_json(bytes: &[u8]) -> Result<Self, Error> {
        let layout: Self = serde_json::from_slice(bytes)
            .map_err(|e| Error::Invalid(format!("seed layout JSON: {e}")))?;
        layout.validate()?;
        Ok(layout)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.format != SEED_LAYOUT_FORMAT {
            return Err(Error::Invalid(format!(
                "unsupported seed layout format; expected {SEED_LAYOUT_FORMAT}"
            )));
        }
        for (slot, entry) in self.report.positional_signals.iter().enumerate() {
            if entry.slot != slot {
                return Err(Error::Invalid(
                    "seed layout positional slots must be contiguous and ordered".into(),
                ));
            }
        }
        if self.report.positional_signals.len() > crate::signals::payload::MAX_CONTAINER_ENTRIES {
            return Err(Error::Invalid("seed layout exceeds the entry limit".into()));
        }
        crate::signals::payload::superpack::SerializerOptions::outer_channel(self.keysets.clone())
            .validate()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        Generator::new(self.profile())?;
        Ok(())
    }

    /// Validate and build the codec this artifact describes.
    pub fn generator(&self) -> Result<Generator, Error> {
        self.validate()?;
        Generator::new(self.profile())
    }

    /// Encode the four channels to the bare finalized payload.
    ///
    /// The full wire path runs here: the errors/signals channels and every
    /// positional/keyed `encoded` value are superpacked, the container is framed
    /// with ULEB lengths and the missing-entry sentinel, the eight-byte seed
    /// prefix is added, the payload is encrypted with the pinned ChaCha-style
    /// cipher, and the ciphertext is written as unpadded custom base64.
    pub fn encode_payload(
        &self,
        channels: &ChannelReport,
        seeds: [u32; 2],
    ) -> Result<String, Error> {
        self.generator()?
            .encode_channels(channels, seeds, self.keysets.clone())
    }

    /// Encode the four channels to the session header bundle.
    ///
    /// Runs the same payload path, then splits the payload at the pinned chunk
    /// size and adds the `b` checksum plus the pinned `f`/`c`/`d`/`z` siblings.
    pub fn encode_headers(
        &self,
        channels: &ChannelReport,
        seeds: [u32; 2],
    ) -> Result<Headers, Error> {
        let codec = self.generator()?;
        let document = HeaderReport::from_channels(channels, seeds, self.keysets.clone(), None);
        codec.encode_report_headers(&document)
    }

    /// Decode a bare finalized payload into the four channels.
    ///
    /// Recovers the seeds and plaintext with the pinned cipher and custom
    /// base64, parses the container with this layout's positional count and
    /// keysets, then unpacks every value channel with superpack.
    pub fn decode_payload(&self, value: &str) -> Result<Report, Error> {
        self.validate()?;
        let codec = PayloadCodec::new(self.profile().into())?;
        codec.decode_report(
            value,
            self.report.positional_signals.len(),
            self.keysets.clone(),
        )
    }

    /// Decode a header bundle into the full report (channels, seeds, keysets and
    /// the sibling context the capture carried).
    pub fn decode_headers(&self, headers: &Headers) -> Result<HeaderReport, Error> {
        self.generator()?.decode_report_headers(
            headers,
            self.report.positional_signals.len(),
            self.keysets.clone(),
        )
    }
}

/// A pinned artifact plus the byte spans each fact came from.
#[derive(Clone, Debug)]
pub struct PinnedSeed {
    pub layout: SeedLayout,
    pub evidence: Vec<Evidence>,
}

/// Public build facts pinned without session material.
#[derive(Clone, Debug)]
pub struct SeedSourceFacts {
    pub source: Source,
    pub cipher: CipherProfile,
    pub header: HeaderLayout,
    pub keysets: Vec<Vec<String>>,
    pub report: ReportLayout,
}

pub fn pin_source(source: &str) -> std::result::Result<SeedSourceFacts, PinError> {
    if source.len() > 16 * 1024 * 1024 {
        return Err(pin_fail(Span::default(), "program exceeds 16 MiB"));
    }
    let arena = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&arena, source);
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(pin_fail(Span::default(), "JavaScript parse failed"));
    }
    let (scopes, nodes) = crate::analysis::analyze(&parsed.program);
    let ix = Index::new(source, &scopes, &nodes);
    let model = crate::signals::extract::bindings::Model::new(
        source,
        &parsed.program,
        &scopes,
        &nodes,
        Default::default(),
    )
    .map_err(|e| pin_fail(Span::default(), e.to_string()))?;
    let mut evidence = vec![];
    let cipher = pin_cipher(&ix, &mut evidence)?;
    let layout = pin_layout(&ix, &model, &mut evidence)?;
    let header = pin_header(&ix, &mut evidence)?;
    Ok(SeedSourceFacts {
        source: Source {
            sha256: sha(source),
            bytes: source.len(),
        },
        cipher,
        header,
        keysets: layout.keysets,
        report: layout.report,
    })
}

impl PinnedSeed {
    pub fn to_json_pretty(&self) -> Vec<u8> {
        let mut out = serde_json::to_vec_pretty(&self.layout).expect("layout serializes");
        out.push(b'\n');
        out
    }
}

/// A pinning failure with the source span that did not match.
#[derive(Clone, Debug, thiserror::Error)]
#[error("seed pinning at bytes {start}..{end}: {message}")]
pub struct PinError {
    pub start: u32,
    pub end: u32,
    pub message: String,
}

fn pin_fail(span: Span, message: impl Into<String>) -> PinError {
    PinError {
        start: span.start,
        end: span.end,
        message: message.into(),
    }
}

/// Source literal emitted as the `-c` header, with byte-span/digest evidence
/// for all three defaults that prove its receiver association.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSeedId {
    pub seed_id: String,
    pub evidence: Vec<Evidence>,
}

/// Find the seed ID without session material or captured headers.
///
/// Requires exactly one resolved identifier receiver carrying static `c/d/z`
/// assignments. Dot and literal computed properties are supported; aliases,
/// unknown receivers, dynamic properties and conflicting defaults are not
/// inferred. Spans are UTF-8 byte offsets including the literal's quotes.
pub fn find_seed_id(source: &str) -> Result<SourceSeedId, PinError> {
    if source.len() > 16 * 1024 * 1024 {
        return Err(pin_fail(Span::default(), "program exceeds 16 MiB"));
    }
    let arena = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&arena, source);
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(pin_fail(
            Span::default(),
            format!("JavaScript parse: {}", parsed.errors.join("; ")),
        ));
    }
    let (scopes, nodes) = crate::analysis::analyze(&parsed.program);
    let ix = Index::new(source, &scopes, &nodes);
    let mut evidence = vec![];
    let (seed_id, _, _) = pin_context(&ix, &mut evidence)?;
    Ok(SourceSeedId { seed_id, evidence })
}

/// Pin every build fact needed to decode and re-encode one capture.
pub fn pin(source: &str, session: &Session) -> std::result::Result<PinnedSeed, PinError> {
    if source.len() > 16 * 1024 * 1024 {
        return Err(pin_fail(Span::default(), "program exceeds 16 MiB"));
    }
    let arena = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&arena, source);
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(pin_fail(
            Span::default(),
            format!("JavaScript parse: {}", parsed.errors.join("; ")),
        ));
    }
    let (scopes, nodes) = crate::analysis::analyze(&parsed.program);
    let ix = Index::new(source, &scopes, &nodes);
    let mut evidence = vec![];
    let cipher = pin_cipher(&ix, &mut evidence)?;
    let model = crate::signals::extract::bindings::Model::new(
        source,
        &parsed.program,
        &scopes,
        &nodes,
        Default::default(),
    )
    .map_err(|e| pin_fail(Span::default(), e.to_string()))?;
    let layout = pin_layout(&ix, &model, &mut evidence)?;
    let header = pin_header(&ix, &mut evidence)?;
    let artifact = SeedLayout {
        format: SEED_LAYOUT_FORMAT.to_owned(),
        source: Source {
            sha256: sha(source),
            bytes: source.len(),
        },
        session: session.clone(),
        cipher,
        header,
        keysets: layout.keysets,
        report: layout.report,
    };
    artifact
        .validate()
        .map_err(|e| pin_fail(Span::default(), format!("pinned layout is invalid: {e}")))?;
    Ok(PinnedSeed {
        layout: artifact,
        evidence,
    })
}

fn record(ix: &Index<'_, '_>, evidence: &mut Vec<Evidence>, role: &str, span: Span) {
    evidence.push(Evidence {
        role: role.to_owned(),
        start: span.start,
        end: span.end,
        sha256: sha(&ix.source[span.start as usize..span.end as usize]),
    });
}

// ===========================================================================
// cipher
// ===========================================================================

fn pin_cipher(
    ix: &Index<'_, '_>,
    evidence: &mut Vec<Evidence>,
) -> std::result::Result<CipherProfile, PinError> {
    let mut candidates = vec![];
    for node in ix.nodes.iter() {
        if let AstKind::ArrayExpression(array) = node.kind() {
            if let Some(candidate) = cipher_candidate(ix, array) {
                candidates.push(candidate);
            }
        }
    }
    if candidates.len() != 1 {
        return Err(pin_fail(
            Span::default(),
            format!(
                "stream cipher state: expected one 16-word state, found {}",
                candidates.len()
            ),
        ));
    }
    let (array, function) = candidates.pop().expect("one candidate");
    let mut ivs = [0u32; 4];
    for (index, slot) in ivs.iter_mut().enumerate() {
        let element = array.elements[index]
            .as_expression()
            .expect("numeric element");
        let value = number_value(masked_member(element).unwrap_or(element))
            .ok_or_else(|| pin_fail(element.span(), "stream cipher IV must be a literal"))?;
        *slot = value as i64 as u32;
    }
    let rounds = pin_rounds(ix, function)?;
    let counter = pin_counter(ix, function, array)?;
    record(ix, evidence, "stream_cipher_state", array.span);
    record(ix, evidence, "stream_cipher_rounds", function.span);
    Ok(CipherProfile {
        ivs,
        counter,
        double_rounds: rounds,
    })
}

/// The state array of `applyStreamCipher`: four literal IVs, eight masked key
/// slots, two placeholder counter words and two seed words.
fn cipher_candidate<'a>(
    ix: &Index<'a, '_>,
    array: &'a ArrayExpression<'a>,
) -> Option<(&'a ArrayExpression<'a>, &'a Function<'a>)> {
    if array.elements.len() != 16 {
        return None;
    }
    let values: Vec<&'_ Expression<'_>> = array
        .elements
        .iter()
        .map(|e| e.as_expression())
        .collect::<Option<_>>()?;
    for value in &values[..4] {
        number_value(masked_member(value)?)?;
    }
    for (index, value) in values.iter().enumerate().take(12).skip(4) {
        let slot = masked_member(value)?;
        let Expression::ComputedMemberExpression(member) = slot else {
            return None;
        };
        ix.symbol(&member.object)?;
        match member.expression.without_parentheses() {
            Expression::NumericLiteral(n) if n.value == (index - 4) as f64 => {}
            _ => return None,
        }
    }
    for value in &values[12..14] {
        if number_value(value) != Some(0.0) {
            return None;
        }
    }
    let function = enclosing_function(ix, array.span)?;
    let looped = ix.nodes.iter().any(|n| {
        within(function.span, n.span())
            && matches!(
                n.kind(),
                AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_)
            )
    });
    looped.then_some((array, function))
}

/// A numeric literal, including the unary-minus form Oxc parses for negative
/// constants such as `-2053510410`.
fn number_value(e: &Expression<'_>) -> Option<f64> {
    match e.without_parentheses() {
        Expression::NumericLiteral(n) => Some(n.value),
        Expression::UnaryExpression(u)
            if u.operator == oxc_syntax::operator::UnaryOperator::UnaryNegation =>
        {
            match u.argument.without_parentheses() {
                Expression::NumericLiteral(n) => Some(-n.value),
                _ => None,
            }
        }
        _ => None,
    }
}

fn masked_member<'a>(e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match e.without_parentheses() {
        Expression::BinaryExpression(binary)
            if binary.operator == oxc_syntax::operator::BinaryOperator::BitwiseAnd =>
        {
            // The mask may be written on either side: `v & 0xffffffff` or
            // `0xffffffff & v`.
            for (masked, mask) in [(&binary.left, &binary.right), (&binary.right, &binary.left)] {
                if matches!(mask.without_parentheses(), Expression::NumericLiteral(n) if n.value == 4_294_967_295.0)
                {
                    return Some(masked);
                }
            }
            None
        }
        value => Some(value),
    }
}

fn enclosing_function<'a>(ix: &Index<'a, '_>, span: Span) -> Option<&'a Function<'a>> {
    ix.functions
        .iter()
        .copied()
        .filter(|f| within(f.span, span))
        .min_by_key(|f| f.span.end - f.span.start)
}

/// Count unrolled quarter rounds inside the cipher function. Each quarter round
/// has exactly one rotate-left-by-16, so eight of them make one double round.
fn pin_rounds(ix: &Index<'_, '_>, function: &Function<'_>) -> std::result::Result<u32, PinError> {
    let mut quarters = 0usize;
    for node in ix.nodes.iter() {
        if !within(function.span, node.span()) {
            continue;
        }
        if let AstKind::BinaryExpression(binary) = node.kind() {
            if binary.operator == oxc_syntax::operator::BinaryOperator::ShiftLeft {
                if let Expression::NumericLiteral(n) = binary.right.without_parentheses() {
                    if n.value == 16.0 {
                        quarters += 1;
                    }
                }
            }
        }
    }
    if quarters == 0 || quarters % 8 != 0 {
        return Err(pin_fail(
            function.span,
            format!("stream cipher quarter rounds are not whole double rounds ({quarters})"),
        ));
    }
    let rounds = quarters / 8;
    if !(1..=20).contains(&rounds) {
        return Err(pin_fail(
            function.span,
            format!("stream cipher double rounds out of range: {rounds}"),
        ));
    }
    Ok(rounds as u32)
}

/// The counter is the numeric literal behind the state's word 12 assignment.
fn pin_counter(
    ix: &Index<'_, '_>,
    function: &Function<'_>,
    array: &ArrayExpression<'_>,
) -> std::result::Result<u64, PinError> {
    let mut symbols = BTreeSet::new();
    for node in ix.nodes.iter() {
        if !within(function.span, node.span()) {
            continue;
        }
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            continue;
        };
        let AssignmentTarget::ComputedMemberExpression(target) = &assign.left else {
            continue;
        };
        let Expression::NumericLiteral(index) = target.expression.without_parentheses() else {
            continue;
        };
        if index.value != 12.0 {
            continue;
        }
        let Some(state) = ix.symbol(&target.object) else {
            continue;
        };
        if !binds_array(ix, state, array, assign.span.start) {
            continue;
        }
        collect_symbols(ix, &assign.right, &mut symbols);
    }
    let mut values = BTreeSet::new();
    for symbol in symbols {
        let Some(definitions) = ix.definitions.get(&symbol) else {
            continue;
        };
        for (_, value) in definitions {
            if let Expression::NumericLiteral(n) = value.without_parentheses() {
                if n.value >= 0.0 && n.value < (1u64 << 53) as f64 && n.value.fract() == 0.0 {
                    values.insert(n.value as u64);
                }
            }
        }
    }
    if values.len() != 1 {
        return Err(pin_fail(
            function.span,
            format!(
                "stream cipher counter: expected one literal candidate, found {:?}",
                values.iter().collect::<Vec<_>>()
            ),
        ));
    }
    Ok(values.into_iter().next().expect("one counter"))
}

/// True when `symbol` is the binding whose definition is this array literal.
fn binds_array(ix: &Index<'_, '_>, symbol: SymbolId, array: &ArrayExpression<'_>, at: u32) -> bool {
    ix.definitions.get(&symbol).is_some_and(|definitions| {
        definitions
            .iter()
            .rev()
            .find(|(position, _)| *position < at)
            .or_else(|| (definitions.len() == 1).then(|| &definitions[0]))
            .is_some_and(|(_, value)| value.span() == array.span)
    })
}

fn collect_symbols(ix: &Index<'_, '_>, e: &Expression<'_>, out: &mut BTreeSet<SymbolId>) {
    let mut pending = vec![e];
    while let Some(value) = pending.pop() {
        match value.without_parentheses() {
            Expression::Identifier(_) => {
                if let Some(symbol) = ix.symbol(value) {
                    out.insert(symbol);
                }
            }
            Expression::BinaryExpression(b) => {
                pending.push(&b.left);
                pending.push(&b.right);
            }
            Expression::UnaryExpression(u) => pending.push(&u.argument),
            Expression::ConditionalExpression(c) => {
                pending.push(&c.test);
                pending.push(&c.consequent);
                pending.push(&c.alternate);
            }
            Expression::ParenthesizedExpression(p) => pending.push(&p.expression),
            _ => {}
        }
    }
}

// ===========================================================================
// report layout
// ===========================================================================

struct Layout {
    report: ReportLayout,
    keysets: Vec<Vec<String>>,
}

fn pin_layout(
    ix: &Index<'_, '_>,
    model: &crate::signals::extract::bindings::Model<'_, '_>,
    evidence: &mut Vec<Evidence>,
) -> std::result::Result<Layout, PinError> {
    let (filter, table, route) = pin_route(ix)?;
    let registry = crate::signals::extract::scanner::descriptor_registry_from_model(model)
        .map_err(|e| pin_fail(table.span, e.to_string()))?
        .ok_or_else(|| pin_fail(table.span, "consumed descriptor registry is missing"))?;
    if registry.array_literal_span != Some(table.span.into())
        || registry.elements.len() != registry.length
    {
        return Err(pin_fail(
            table.span,
            "consumed descriptor registry is incomplete or differs from report registry",
        ));
    }
    let indices = numeric_array(ix, filter)?;
    if indices.is_empty() || indices.len() > crate::signals::payload::MAX_CONTAINER_ENTRIES {
        return Err(pin_fail(filter.span, "signal filter size"));
    }
    let mut report = ReportLayout::default();
    for index in &indices {
        use crate::signals::schema::LiteralValue;
        let descriptor = registry
            .elements
            .iter()
            .find(|d| d.index == *index)
            .ok_or_else(|| {
                pin_fail(
                    table.span,
                    format!(
                        "signal filter index {index} is outside the consumed descriptor registry"
                    ),
                )
            })?;
        let ctor = descriptor
            .ctor
            .as_ref()
            .is_some_and(|c| c.literal != Some(LiteralValue::Null));
        let priority = match descriptor.priority {
            None | Some(LiteralValue::Boolean(false)) => false,
            Some(LiteralValue::Boolean(true)) => true,
            _ => {
                return Err(pin_fail(
                    table.span,
                    "descriptor priority must be a boolean",
                ))
            }
        };
        match &descriptor.name {
            Some(LiteralValue::Number(name))
                if name.fract() == 0.0 && *name >= 0.0 && *name <= u32::MAX as f64 =>
            {
                report.keyed_signals.push(KeyedSignalLayout {
                    descriptor: *index,
                    name: *name as u32,
                })
            }
            Some(LiteralValue::String(name)) => report.signals.push(NamedSignalLayout {
                descriptor: *index,
                name: name.clone(),
            }),
            None => report.positional_signals.push(SlotLayout {
                slot: report.positional_signals.len(),
                descriptor: *index,
                ctor,
                priority,
            }),
            _ => {
                return Err(pin_fail(
                    table.span,
                    "dynamic or unsupported descriptor name",
                ))
            }
        }
    }
    let keysets = pin_keysets(ix)?;
    record(ix, evidence, "signal_filter", filter.span);
    record(ix, evidence, "descriptor_table", table.span);
    record(ix, evidence, "descriptor_routing", route.span);
    record(ix, evidence, "keyset_mapping", keysets.span);
    Ok(Layout {
        report,
        keysets: keysets.keysets,
    })
}

/// The report route names its default filter in the `signalFilter` branch and
/// indexes the descriptor table by the selected index.
fn pin_route<'a>(
    ix: &Index<'a, '_>,
) -> std::result::Result<
    (
        &'a ArrayExpression<'a>,
        &'a ArrayExpression<'a>,
        &'a Function<'a>,
    ),
    PinError,
> {
    let mut routes = BTreeMap::new();
    for conditional in ix.conditionals.iter() {
        if !reads_member(&conditional.test, "signalFilter") {
            continue;
        }
        let Some(function) = enclosing_function(ix, conditional.span) else {
            continue;
        };
        let mut filter = None;
        for arm in [&conditional.consequent, &conditional.alternate] {
            if let Some(symbol) = ix.symbol(arm) {
                if let Some(array) = resolve_array(ix, symbol, conditional.span.start) {
                    filter.get_or_insert(array);
                }
            }
        }
        let Some(filter) = filter else { continue };
        let mut table = None;
        for node in ix.nodes.iter() {
            if !within(function.span, node.span()) {
                continue;
            }
            let AstKind::ComputedMemberExpression(member) = node.kind() else {
                continue;
            };
            if enclosing_function(ix, member.span).is_none_or(|owner| owner.span != function.span) {
                continue;
            }
            let Some(symbol) = ix.symbol(&member.object) else {
                continue;
            };
            if let Some(array) = resolve_array(ix, symbol, member.span.start) {
                if array.elements.iter().all(|element| {
                    element.as_expression().is_some_and(|element| {
                        matches!(
                            element.without_parentheses(),
                            Expression::ObjectExpression(_)
                        )
                    })
                }) {
                    table.get_or_insert(array);
                }
            }
        }
        if let Some(table) = table {
            routes
                .entry((function.span.start, function.span.end))
                .or_insert((filter, table, function));
        }
    }
    if routes.len() != 1 {
        return Err(pin_fail(
            Span::default(),
            format!(
                "report route: expected one filter/descriptor pair, found {}",
                routes.len()
            ),
        ));
    }
    Ok(routes.into_values().next().expect("one route"))
}

/// True when an expression reads the named static member anywhere in its test.
fn reads_member(e: &Expression<'_>, name: &str) -> bool {
    let mut pending = vec![e];
    while let Some(value) = pending.pop() {
        match value.without_parentheses() {
            Expression::StaticMemberExpression(member) if member.property.name == name => {
                return true;
            }
            Expression::BinaryExpression(binary) => {
                pending.push(&binary.left);
                pending.push(&binary.right);
            }
            Expression::LogicalExpression(logical) => {
                pending.push(&logical.left);
                pending.push(&logical.right);
            }
            Expression::UnaryExpression(unary) => pending.push(&unary.argument),
            Expression::ParenthesizedExpression(paren) => pending.push(&paren.expression),
            _ => {}
        }
    }
    false
}

fn resolve_array<'a>(
    ix: &Index<'a, '_>,
    symbol: SymbolId,
    at: u32,
) -> Option<&'a ArrayExpression<'a>> {
    let (position, value) = ix
        .definitions
        .get(&symbol)?
        .iter()
        .rev()
        .find(|(p, _)| *p < at)?;
    let mut value: &'a Expression<'a> = value;
    let mut at = *position;
    for _ in 0..8 {
        match value.without_parentheses() {
            Expression::ArrayExpression(array) => return Some(array),
            Expression::Identifier(_) => {
                let symbol = ix.symbol(value)?;
                let (position, next) = ix
                    .definitions
                    .get(&symbol)?
                    .iter()
                    .rev()
                    .find(|(p, _)| *p < at)?;
                value = next;
                at = *position;
            }
            _ => return None,
        }
    }
    None
}

fn numeric_array(
    _ix: &Index<'_, '_>,
    array: &ArrayExpression<'_>,
) -> std::result::Result<Vec<usize>, PinError> {
    let mut out = vec![];
    for element in &array.elements {
        let element = element
            .as_expression()
            .ok_or_else(|| pin_fail(array.span, "signal filter hole/spread"))?;
        let Expression::NumericLiteral(n) = element.without_parentheses() else {
            return Err(pin_fail(
                element.span(),
                "signal filter index must be a literal",
            ));
        };
        if n.value.fract() != 0.0 || n.value < 0.0 {
            return Err(pin_fail(
                element.span(),
                "signal filter index must be non-negative",
            ));
        }
        out.push(n.value as usize);
    }
    Ok(out)
}

struct Keysets {
    keysets: Vec<Vec<String>>,
    span: Span,
}

/// The constructor receives `indices.map(index => table[index])`. Only that
/// ordered mapping is accepted: the callback must return the indexed table row
/// and the row must be a literal array of literal strings.
fn pin_keysets(ix: &Index<'_, '_>) -> std::result::Result<Keysets, PinError> {
    let mut found = vec![];
    for call in ix.calls.iter().copied() {
        if member_name(&call.callee) != Some("map") || call.arguments.len() != 1 {
            continue;
        }
        let Some(Expression::ArrowFunctionExpression(arrow)) = call.arguments[0].as_expression()
        else {
            continue;
        };
        if arrow.r#async
            || !arrow.expression
            || arrow.params.items.len() != 1
            || arrow.params.rest.is_some()
            || arrow.params.items[0].initializer.is_some()
        {
            continue;
        }
        let BindingPattern::BindingIdentifier(parameter) = &arrow.params.items[0].pattern else {
            continue;
        };
        let Some(parameter) = parameter.symbol_id.get() else {
            continue;
        };
        let Some(Statement::ExpressionStatement(statement)) = arrow.body.statements.first() else {
            continue;
        };
        let Expression::ComputedMemberExpression(access) =
            statement.expression.without_parentheses()
        else {
            continue;
        };
        if ix.symbol(&access.expression) != Some(parameter) {
            continue;
        }
        let Some(indices_symbol) = member_object(&call.callee).and_then(|o| ix.symbol(o)) else {
            continue;
        };
        let Some(indices) = resolve_array(ix, indices_symbol, call.span.start) else {
            continue;
        };
        let Some(table_symbol) = ix.symbol(&access.object) else {
            continue;
        };
        let Some(table) = resolve_array(ix, table_symbol, call.span.start) else {
            continue;
        };
        let Ok(positions) = numeric_array(ix, indices) else {
            continue;
        };
        let mut keysets = vec![];
        let mut valid = true;
        for index in &positions {
            let Some(row) = table
                .elements
                .get(*index)
                .and_then(ArrayExpressionElement::as_expression)
            else {
                valid = false;
                break;
            };
            let row = match row.without_parentheses() {
                Expression::ArrayExpression(array) => array,
                Expression::Identifier(_) => match ix
                    .symbol(row)
                    .and_then(|symbol| resolve_array(ix, symbol, call.span.start))
                {
                    Some(array) => array,
                    None => {
                        valid = false;
                        break;
                    }
                },
                _ => {
                    valid = false;
                    break;
                }
            };
            let mut keys = vec![];
            for key in &row.elements {
                let Some(key) = key.as_expression() else {
                    valid = false;
                    break;
                };
                let Expression::StringLiteral(string) = key.without_parentheses() else {
                    valid = false;
                    break;
                };
                keys.push(string.value.to_string());
            }
            keysets.push(keys);
        }
        if valid {
            found.push((keysets, call.span));
        }
    }
    if found.len() != 1 {
        return Err(pin_fail(
            Span::default(),
            format!(
                "keyset mapping: expected one ordered table[index] map call, found {}",
                found.len()
            ),
        ));
    }
    let (keysets, span) = found.pop().expect("one keyset mapping");
    crate::signals::payload::superpack::SerializerOptions::outer_channel(keysets.clone())
        .validate()
        .map_err(|e| pin_fail(span, e.to_string()))?;
    Ok(Keysets { keysets, span })
}

// ===========================================================================
// header policy
// ===========================================================================

fn pin_header(
    ix: &Index<'_, '_>,
    evidence: &mut Vec<Evidence>,
) -> std::result::Result<HeaderLayout, PinError> {
    let mut prefixes = BTreeSet::new();
    for node in ix.nodes.iter() {
        let AstKind::BinaryExpression(binary) = node.kind() else {
            continue;
        };
        if binary.operator != oxc_syntax::operator::BinaryOperator::Addition {
            continue;
        }
        let Expression::StringLiteral(left) = binary.left.without_parentheses() else {
            continue;
        };
        if !left.value.ends_with('-') || left.value.len() > 64 {
            continue;
        }
        let parent = ix.nodes.parent_id(node.id());
        if matches!(ix.nodes.kind(parent), AstKind::CallExpression(_)) {
            prefixes.insert((left.value.to_string(), left.span));
        }
    }
    let mut values = prefixes
        .iter()
        .map(|(value, _)| value.clone())
        .collect::<BTreeSet<_>>();
    if values.len() != 1 {
        return Err(pin_fail(
            Span::default(),
            format!(
                "header prefix: expected one distinct literal, found {}",
                values.len()
            ),
        ));
    }
    let prefix = values.pop_first().expect("one prefix");
    let span = prefixes
        .iter()
        .find(|(value, _)| *value == prefix)
        .map(|(_, span)| *span)
        .expect("one prefix span");
    record(ix, evidence, "header_prefix", span);
    let chunk_size = pin_chunk_size(ix, evidence)?;
    let (c, d, z) = pin_context(ix, evidence)?;
    let prefix = prefix.to_ascii_lowercase();
    Ok(HeaderLayout {
        name: if prefix.ends_with('-') {
            format!("{prefix}a")
        } else {
            format!("{prefix}-a")
        },
        prefix,
        chunk_size,
        checksum: CHECKSUM_POLICY.to_owned(),
        b: true,
        f: true,
        c,
        d,
        z,
    })
}

/// The only checksum policy this generator implements:
/// `hash(token + full payload)` over paired UTF-16, signed base 36.
const CHECKSUM_POLICY: &str = "signed-paired-utf16-base36-token-plus-full-payload";

fn pin_chunk_size(
    ix: &Index<'_, '_>,
    evidence: &mut Vec<Evidence>,
) -> std::result::Result<Option<usize>, PinError> {
    let mut sizes = BTreeSet::new();
    let mut span = None;
    for call in ix.calls.iter().copied() {
        if call.arguments.len() != 3 {
            continue;
        }
        let Some(left) = call.arguments[0].as_expression() else {
            continue;
        };
        let Expression::BinaryExpression(binary) = left.without_parentheses() else {
            continue;
        };
        let Expression::StringLiteral(prefix) = binary.left.without_parentheses() else {
            continue;
        };
        if !prefix.value.ends_with('-') {
            continue;
        }
        let Some(Expression::NumericLiteral(size)) = call.arguments[2]
            .as_expression()
            .map(Expression::without_parentheses)
        else {
            continue;
        };
        if size.value.fract() != 0.0 || size.value <= 0.0 {
            continue;
        }
        sizes.insert(size.value as usize);
        span.get_or_insert(call.span);
    }
    if sizes.len() > 1 {
        return Err(pin_fail(
            span.unwrap_or_default(),
            "header chunk sizes disagree",
        ));
    }
    if let Some(span) = span {
        record(ix, evidence, "header_chunk_size", span);
    }
    Ok(sizes.into_iter().next())
}

fn pin_context(
    ix: &Index<'_, '_>,
    evidence: &mut Vec<Evidence>,
) -> std::result::Result<(String, String, String), PinError> {
    // c/d/z are three literal defaults set on one option object. Other `.c`
    // assignments (counters, geometry temporaries) are a different object and
    // must not make the policy ambiguous.
    let mut groups: BTreeMap<SymbolId, BTreeMap<&str, (String, Span)>> = BTreeMap::new();
    let mut seen: BTreeMap<SymbolId, BTreeSet<&str>> = BTreeMap::new();
    let mut invalid = BTreeMap::new();
    for node in ix.nodes.iter() {
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            continue;
        };
        let (object, name) = match &assign.left {
            AssignmentTarget::StaticMemberExpression(member) => {
                (&member.object, Some(member.property.name.as_str()))
            }
            AssignmentTarget::ComputedMemberExpression(member) => (
                &member.object,
                match member.expression.without_parentheses() {
                    Expression::StringLiteral(s) => Some(s.value.as_str()),
                    _ => None,
                },
            ),
            _ => continue,
        };
        // Never pool unresolved references under a shared `None` identity.
        let Some(receiver) = ix.symbol(object) else {
            continue;
        };
        let Some(name) = name else {
            invalid.insert(
                receiver,
                (assign.span, "dynamic header property".to_owned()),
            );
            continue;
        };
        if !matches!(name, "c" | "d" | "z") {
            continue;
        }
        let group = groups.entry(receiver).or_default();
        seen.entry(receiver).or_default().insert(name);
        if assign.operator != oxc_syntax::operator::AssignmentOperator::Assign {
            invalid.insert(
                receiver,
                (assign.span, format!("non-static header {name} assignment")),
            );
            continue;
        }
        let value = match (name, assign.right.without_parentheses()) {
            ("d", Expression::ConditionalExpression(conditional))
                if descriptor_default(ix, conditional) =>
            {
                conditional.consequent.without_parentheses()
            }
            (_, value) => value,
        };
        let Expression::StringLiteral(literal) = value else {
            invalid.insert(
                receiver,
                (assign.span, format!("non-literal header {name} default")),
            );
            continue;
        };
        if let Some(previous) = group.get(name) {
            if previous.0 != literal.value.as_str() {
                invalid.insert(
                    receiver,
                    (assign.span, format!("conflicting header {name} defaults")),
                );
            }
            continue;
        }
        group.insert(name, (literal.value.to_string(), literal.span));
    }
    let mut complete = groups
        .into_iter()
        .filter(|(receiver, _)| seen[receiver].len() == 3);
    let Some((receiver, mut values)) = complete.next() else {
        return Err(pin_fail(
            Span::default(),
            "header context: no object carries literal c, d and z defaults",
        ));
    };
    if complete.next().is_some() {
        return Err(pin_fail(
            Span::default(),
            "header context: more than one object carries c, d and z defaults",
        ));
    }
    if let Some((span, reason)) = invalid.remove(&receiver) {
        return Err(pin_fail(span, reason));
    }
    // A binding reused for a different object cannot prove object identity.
    if ix.nodes.iter().any(|node| {
        matches!(node.kind(), AstKind::AssignmentExpression(a)
            if matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id)
                if crate::analysis::identifier_symbol(id, ix.scopes) == Some(receiver)))
    }) {
        return Err(pin_fail(Span::default(), "header receiver is reassigned"));
    }
    let mut take = |name: &str| {
        values
            .remove(name)
            .map(|(value, span)| {
                record(ix, evidence, &format!("header_{name}_default"), span);
                value
            })
            .ok_or_else(|| pin_fail(Span::default(), format!("missing header {name} default")))
    };
    Ok((take("c")?, take("d")?, take("z")?))
}

/// The supported source default is `descriptor === void 0 ? literal : descriptor`.
fn descriptor_default(ix: &Index<'_, '_>, c: &ConditionalExpression<'_>) -> bool {
    let Expression::BinaryExpression(test) = c.test.without_parentheses() else {
        return false;
    };
    if test.operator != oxc_syntax::operator::BinaryOperator::StrictEquality {
        return false;
    }
    let is_undefined = |e: &Expression<'_>| {
        matches!(e.without_parentheses(),
        Expression::UnaryExpression(u)
        if u.operator == oxc_syntax::operator::UnaryOperator::Void
            && matches!(u.argument.without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.0))
    };
    let descriptor = if is_undefined(&test.right) {
        &test.left
    } else if is_undefined(&test.left) {
        &test.right
    } else {
        return false;
    };
    ix.symbol(descriptor)
        .is_some_and(|symbol| ix.symbol(&c.alternate) == Some(symbol))
}

/// Discover the session literals in one `detail.init(...)` call by shape, not
/// by position: one 64/65-symbol alphabet, one eight-word numeric key list, and
/// a leading token string.
pub fn session_from_initializer(source: &str) -> std::result::Result<Session, PinError> {
    if source.len() > 128 * 1024 {
        return Err(pin_fail(Span::default(), "initializer exceeds 128 KiB"));
    }
    let arena = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&arena, source);
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(pin_fail(
            Span::default(),
            format!("JavaScript parse: {}", parsed.errors.join("; ")),
        ));
    }
    let (_, nodes) = crate::analysis::analyze(&parsed.program);
    let calls: Vec<_> = nodes
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::CallExpression(call)
                if member_name(&call.callee) == Some("init")
                    && member_object(&call.callee)
                        .is_some_and(|object| member_name(object) == Some("detail")) =>
            {
                Some(call)
            }
            _ => None,
        })
        .collect();
    if calls.len() != 1 {
        return Err(pin_fail(
            Span::default(),
            format!("detail.init: expected one call, found {}", calls.len()),
        ));
    }
    let call = calls[0];
    let mut alphabet = None;
    let mut keys = None;
    let mut token = None;
    for argument in &call.arguments {
        let Some(expression) = argument.as_expression() else {
            return Err(pin_fail(call.span, "initializer spread argument"));
        };
        match expression.without_parentheses() {
            Expression::StringLiteral(string) => {
                let value = string.value.to_string();
                if is_alphabet(&value) {
                    alphabet.get_or_insert(value);
                } else if token.is_none() {
                    token = Some(value);
                }
            }
            Expression::ArrayExpression(array) if array.elements.len() == 8 => {
                let words: Option<Vec<u32>> = array
                    .elements
                    .iter()
                    .map(|element| {
                        let Expression::NumericLiteral(n) =
                            element.as_expression()?.without_parentheses()
                        else {
                            return None;
                        };
                        if n.value.fract() != 0.0 || n.value < 0.0 || n.value > 4_294_967_295.0 {
                            return None;
                        }
                        Some(n.value as u64 as u32)
                    })
                    .collect();
                if let Some(words) = words {
                    keys.get_or_insert(words);
                }
            }
            _ => {}
        }
    }
    let alphabet =
        alphabet.ok_or_else(|| pin_fail(call.span, "initializer has no alphabet literal"))?;
    let keys = keys.ok_or_else(|| pin_fail(call.span, "initializer has no eight key words"))?;
    let token = token.ok_or_else(|| pin_fail(call.span, "initializer has no token string"))?;
    Ok(Session {
        transaction_token: token,
        alphabet,
        shuffle_key: vec![],
        keys: keys.try_into().expect("eight words"),
        nonce: None,
    })
}

fn is_alphabet(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !(64..=65).contains(&bytes.len()) {
        return false;
    }
    let mut seen = BTreeSet::new();
    bytes
        .iter()
        .all(|b| (0x21..=0x7e).contains(b) && seen.insert(*b))
}
