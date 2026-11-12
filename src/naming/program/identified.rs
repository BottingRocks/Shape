//! High-level identification: structural properties -> rename and comment plans.
//!
//! This adapter is the bridge between [`crate::naming::semantic_names`]'s structural
//! extraction and the rewrite engine in [`super`]. It never looks at a
//! generated name to decide a role:
//!
//! * **Renames** come only from the unified naming plan
//!   ([`crate::naming::semantic_names::plan_semantic_names_with_properties`]): one role, one
//!   name, with the single-origin, function-identity, ambiguity, role-conflict
//!   and alias-uniqueness proofs applied. There is no second vocabulary here
//!   and no first-helper-wins choice.
//! * **Comments** describe the generator *inputs* first: the signal position,
//!   its route, the ordered `STRING` / `NUMBER` / `BYTE` leaves with their
//!   `REPEAT` / `CHOICE` containers, the per-emission mask literal and the
//!   runtime seed roles. A semantic label is optional and never required.
//! * **Unknowns stay explicit**: a wire op or object field the extractor could
//!   not type is counted in the comment and remains in the report diagnostics.
//!
//! Renaming and annotation are independent: each is optional, both operate on
//! the *original* parsed source, and annotation anchors are parsed statement
//! boundaries so a comment can never break a single-statement branch or ASI.
//! Neither mode executes the recovered JavaScript.

use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_ast_visit::{walk, Visit};
use oxc_span::GetSpan;

use crate::analysis;
use crate::signals::extract::layout::{LeafKind, WireOp};
use crate::signals::extract::scanner::extract_program_properties;
use crate::signals::schema::{ProgramProperties, ProgramPropertiesError, SignalRecord};
use crate::naming::semantic_names::{plan_semantic_names_with_properties, SemanticNamesError, SemanticNamingPlan};

use super::local_names::{apply_local_names, LocalNameOptions, LocalNamesResult};
use super::{
    transform_program, ProgramBindingRename, ProgramTransformError, ProgramTransformOptions,
    ProgramTransformResult, SignalAnnotation,
};

/// Options for [`identify_and_transform`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentifyOptions {
    /// Apply the unified naming plan's renames. Defaults to `false`.
    pub rename: bool,
    /// Insert explanatory signal comments. Defaults to `true`.
    pub annotate: bool,
}

impl Default for IdentifyOptions {
    fn default() -> Self {
        Self {
            rename: false,
            annotate: true,
        }
    }
}

impl IdentifyOptions {
    /// Both renames and comments enabled.
    pub const fn full() -> Self {
        Self {
            rename: true,
            annotate: true,
        }
    }

    /// Analysis only; the transformed source equals the input.
    pub const fn analysis_only() -> Self {
        Self {
            rename: false,
            annotate: false,
        }
    }
}

/// The result of the high-level identification pass.
#[derive(Debug, Clone)]
pub struct IdentifiedProgram {
    /// Structural properties extracted from the *original* source.
    pub properties: ProgramProperties,
    /// The unified naming plan the renames come from.
    pub naming_plan: SemanticNamingPlan,
    /// One rename plan per naming candidate, in identifier order.
    pub rename_plans: Vec<ProgramBindingRename>,
    /// One comment plan per signal with a reconstructed wire (in signal order).
    pub annotations: Vec<SignalAnnotation>,
    /// The rewrite result, including per-plan applied/skipped outcomes.
    pub transform: ProgramTransformResult,
    /// SHA-256 of the original input bytes as parsed.
    pub source_sha256: String,
    /// SHA-256 of the produced source (equal to the input when nothing applied).
    pub transformed_sha256: String,
    /// The local-variable readability pass, applied after the renames.
    pub local_names: LocalNamesResult,
}

impl IdentifiedProgram {
    /// The extracted facts remain available after the rewrite.
    pub fn properties(&self) -> &ProgramProperties {
        &self.properties
    }

    /// Number of role renames actually applied.
    pub fn applied_renames(&self) -> usize {
        self.transform.applied_renames()
    }

    /// Number of comments actually inserted.
    pub fn applied_annotations(&self) -> usize {
        self.transform.applied_annotations()
    }
}

/// A hard failure: extraction or parsing refused the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifyError {
    /// Structural extraction failed.
    Properties(ProgramPropertiesError),
    /// The naming plan was refused.
    Naming(SemanticNamesError),
    /// Source rewriting could not parse the input.
    Transform(ProgramTransformError),
}

impl std::fmt::Display for IdentifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Properties(error) => write!(f, "program identification failed: {error}"),
            Self::Naming(error) => write!(f, "program identification failed: {error}"),
            Self::Transform(error) => write!(f, "program identification failed: {error}"),
        }
    }
}

impl std::error::Error for IdentifyError {}

impl From<ProgramPropertiesError> for IdentifyError {
    fn from(value: ProgramPropertiesError) -> Self {
        Self::Properties(value)
    }
}

impl From<SemanticNamesError> for IdentifyError {
    fn from(value: SemanticNamesError) -> Self {
        Self::Naming(value)
    }
}

impl From<ProgramTransformError> for IdentifyError {
    fn from(value: ProgramTransformError) -> Self {
        Self::Transform(value)
    }
}

/// Extract properties, take the renames from the unified naming plan, describe
/// every signal wire, and optionally rewrite the original source.
pub fn identify_and_transform(
    source: &str,
    options: IdentifyOptions,
) -> Result<IdentifiedProgram, IdentifyError> {
    identify_and_transform_with_local_names(source, options, LocalNameOptions::default())
}

/// [`identify_and_transform`] with an explicit local-variable naming choice.
/// Locals are named only when renames are enabled and `local_names` is on.
pub fn identify_and_transform_with_local_names(
    source: &str,
    options: IdentifyOptions,
    local_names: LocalNameOptions,
) -> Result<IdentifiedProgram, IdentifyError> {
    let properties = extract_program_properties(source)?;
    let naming_plan = plan_semantic_names_with_properties(source, &properties)?;
    let rename_plans: Vec<ProgramBindingRename> = naming_plan
        .candidates
        .iter()
        .map(|candidate| {
            ProgramBindingRename::new(candidate.identifier.start, candidate.proposed_name.clone())
        })
        .collect();
    let annotations = signal_annotations(source, &properties)?;

    let transform = transform_program(
        source,
        &rename_plans,
        &annotations,
        &ProgramTransformOptions {
            rename_bindings: options.rename,
            annotate_signals: options.annotate,
        },
    )?;
    let local_names = if options.rename {
        apply_local_names(&transform.transformed_source, local_names)?
    } else {
        apply_local_names(
            &transform.transformed_source,
            LocalNameOptions::disabled(),
        )?
    };
    let transformed_source = local_names.transformed_source.clone();

    Ok(IdentifiedProgram {
        source_sha256: properties.source.sha256.clone(),
        transformed_sha256: crate::source::sha256_hex(transformed_source.as_bytes()),
        properties,
        naming_plan,
        rename_plans,
        annotations,
        transform,
        local_names,
    })
}

/// Describe every signal with a reconstructed wire, anchored at the parsed
/// statement that owns its first emission.
pub fn signal_annotations(
    source: &str,
    properties: &ProgramProperties,
) -> Result<Vec<SignalAnnotation>, IdentifyError> {
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, source).map_err(ProgramTransformError::Parse)?;
    let mut boundaries = StatementBoundaries::default();
    boundaries.visit_program(&program);

    let mut annotations = Vec::new();
    for signal in &properties.signals {
        let Some(comment) = signal_comment(signal) else {
            continue;
        };
        let Some(origin) = signal
            .emission_layouts
            .iter()
            .map(|emission| emission.origin)
            .min_by_key(|span| span.start)
        else {
            continue;
        };
        let Some(start) = boundaries.innermost_start(origin.start, origin.end) else {
            continue;
        };
        annotations.push(SignalAnnotation::new(start, comment));
    }
    annotations.sort_by_key(|annotation| annotation.statement_start);
    annotations.dedup_by_key(|annotation| annotation.statement_start);
    Ok(annotations)
}

/// A deterministic, provenance-free description of one signal's generator
/// inputs. Binding names, indices and capture constants are deliberately never
/// included, so the text is stable across a role rename and annotating twice is
/// a no-op.
pub fn signal_comment(signal: &SignalRecord) -> Option<String> {
    if signal.emission_layouts.is_empty() {
        return None;
    }
    let mut leaves = LeafCounts::default();
    let mut generic = false;
    let mut masks: Vec<String> = Vec::new();
    let mut prefix_width = 0usize;
    let mut has_runtime_seed = false;
    for emission in &signal.emission_layouts {
        count_wire(&emission.wire, &mut leaves);
        if emission.prefix.is_some() {
            prefix_width = emission
                .prefix
                .as_ref()
                .map_or(0, |prefix| prefix.byte_width);
            has_runtime_seed = true;
        }
        if emission.variant == crate::signals::extract::layout::EmissionVariant::GenericObject {
            generic = true;
        }
        if let Some(mask) = &emission.mask {
            has_runtime_seed = true;
            if let Some(literal) = mask.xor_literal {
                let text = format_mask_literal(literal);
                if !masks.contains(&text) {
                    masks.push(text);
                }
            }
        }
    }
    let route = if signal.direct {
        "direct".to_string()
    } else if let Some(wave) = signal.wave {
        format!("wave {wave} slot {}", signal.wave_position.unwrap_or(0))
    } else {
        "unrouted".to_string()
    };
    let mut text = format!(
        "signal {} [{}] x{} emission(s); leaves STRING={} NUMBER={} BYTE={}",
        signal.signal,
        route,
        signal.emission_layouts.len(),
        leaves.string,
        leaves.number,
        leaves.byte
    );
    if prefix_width > 0 {
        text.push_str(&format!("; prefix {prefix_width} BYTE (runtime seed)"));
    }
    if generic {
        text.push_str("; generic object encoder (no per-signal PRNG)");
    }
    if !masks.is_empty() {
        text.push_str(&format!("; mask PRNG(seed ^ {})", masks.join(" | ")));
    }
    if has_runtime_seed {
        text.push_str("; runtime inputs: PRNG seed word, session token");
    }
    if leaves.repeat > 0 || leaves.choice > 0 {
        text.push_str(&format!(
            "; containers REPEAT={} CHOICE={}",
            leaves.repeat, leaves.choice
        ));
    }
    if leaves.unknown > 0 {
        text.push_str(&format!("; unknown={}", leaves.unknown));
    }
    Some(text)
}

#[derive(Default)]
struct LeafCounts {
    string: usize,
    number: usize,
    byte: usize,
    repeat: usize,
    choice: usize,
    unknown: usize,
}

fn count_wire(wire: &WireOp, counts: &mut LeafCounts) {
    match wire {
        WireOp::Sequence(parts) => {
            for part in parts {
                count_wire(part, counts);
            }
        }
        WireOp::Choice(parts) => {
            counts.choice += 1;
            for part in parts {
                count_wire(part, counts);
            }
        }
        WireOp::Repeat { body, .. } => {
            counts.repeat += 1;
            count_wire(body, counts);
        }
        WireOp::Leaf(leaf) => match leaf.kind {
            LeafKind::String => counts.string += 1,
            LeafKind::Number => counts.number += 1,
            LeafKind::Byte => counts.byte += 1,
        },
        WireOp::Unknown { .. } => counts.unknown += 1,
    }
}

fn format_mask_literal(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e18 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// Statement spans for annotation anchoring.
#[derive(Default)]
struct StatementBoundaries {
    spans: Vec<(u32, u32)>,
}

impl StatementBoundaries {
    fn innermost_start(&self, start: u32, end: u32) -> Option<u32> {
        self.spans
            .iter()
            .filter(|(statement_start, statement_end)| {
                *statement_start <= start && end <= *statement_end
            })
            .min_by_key(|(statement_start, statement_end)| statement_end - statement_start)
            .map(|(statement_start, _)| *statement_start)
    }
}

impl<'a> Visit<'a> for StatementBoundaries {
    fn visit_statement(&mut self, statement: &Statement<'a>) {
        let span = statement.span();
        self.spans.push((span.start, span.end));
        walk::walk_statement(self, statement);
    }
}
