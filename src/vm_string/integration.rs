//! Proof-aware post-emission folding of concrete `vmDecodeString` calls.
//!
//! This module deliberately sits after symbolic emission. It has four layers:
//!
//! 1. Find the report's string-table and string-cache binding identities and
//!    require exactly one resolved declaration for each in `report.output`.
//! 2. Prove the exact helper and manual decoder at fresh spans, then
//!    check that the table, cache, helper calls, and `String.fromCharCode` are
//!    data-only, closed, and not mutated, escaped, shadowed, or captured.
//! 3. Link emitted callee/dependency names back to that proof, evaluate only
//!    concrete literal calls with [`VmStringEvaluator`], and replace each call
//!    span with a lossless UTF-16 literal.
//! 4. Offer a compact [`VmStringFoldPlan`] for artifact-backed emission. The
//!    plan carries no JS or AST; its authority comes from the enclosing
//!    artifact integrity/version checks, not from `verify_source`.
//!
//! No name is evidence by itself. A helper spelled `vmDecodeString` is folded
//! only if the source proof and emitted-call matcher agree; a plan that is
//! structurally valid but source-free can be used by the runtime folder yet
//! cannot establish which capture produced it.
//!
//! Folding is an expression-local edit: statement order, number of statements,
//! and `EmittedJavaScript` maps are preserved. Hard parse/edit failures abort;
//! ordinary proof failures return the original artifact plus a typed report.

use std::{collections::BTreeMap, fmt};

use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::UnaryOperator;
use serde::{Deserialize, Serialize};

use crate::{
    analysis::{self, Edit},
    kernel::config,
    tracing::symbolic::EmittedJavaScript,
    transforms::deob::pipeline::{ExtractionKind, ExtractionStatus, PipelineReport},
    util::fnv1a64_hex,
};

use super::{
    evaluator::decode_proven_base64, CacheAssumption, DependencyAssumptions, Utf16String,
    VmStringCache, VmStringEvaluator, VmStringLimits, VmStringProof, VmStringRequest,
    VmStringTable, URL_SAFE_ALPHABET,
};

/// Stable decline categories for source-backed and plan-backed folding.
///
/// The order is part of the report's `BTreeMap` ordering, not a precedence
/// rule; evaluation returns the first failed stage. Categories are kept
/// distinct so callers can tell a missing proof from a mutable dependency or a
/// concrete evaluator rejection without parsing diagnostic text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum VmStringFoldReason {
    /// Raw capture bytes do not hash to the report's input identity.
    ReportInputMismatch,
    /// No extracted table fact or no binding identity was available.
    MissingStringTable,
    /// No extracted cache fact or no binding identity was available.
    MissingStringCache,
    /// Zero or multiple table declarations match the report identity.
    AmbiguousStringTable,
    /// Zero or multiple cache declarations match the report identity.
    AmbiguousStringCache,
    /// `with` or direct `eval` prevents stable binding resolution.
    DynamicScope,
    /// A table/cache/helper/decoder/native dependency is written, captured,
    /// aliased, escaped, shadowed, or otherwise observed.
    MutableDependency,
    /// No unique exact helper/decoder pair, or the output did not parse.
    HelperOrDecoderNotProven,
    /// A source helper call does not use the proven dependency bindings.
    HelperCallDependencyMismatch,
    /// An emitted call is not linked to the proven names or literal operands.
    EmittedCallNotProven,
    /// The bounded evaluator rejected the concrete input, key, or cache.
    EvaluationRejected,
    /// The result could not be represented as a lossless JS literal.
    Utf16LiteralUnavailable,
}

/// Wire-format version for the source-free fold plan. Bump this when a field
/// changes meaning or a new required proof fact is introduced.
pub const VM_STRING_FOLD_PLAN_VERSION: &str = "shape-vm-string-fold-plan/v1";

/// The compact compile-time handoff for string folding. A proven plan carries
/// only the exact binding contract and decoded table data needed by the
/// emitted-code folder; it never carries helper JavaScript, an AST, or source
/// fragments. `declined` is explicit and bounded so unsupported captures keep
/// the historical typed decline instead of silently disabling folding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmStringFoldPlan {
    pub schema_version: String,
    pub compiler_provenance: String,
    pub helper_name: Option<String>,
    pub decoder_name: Option<String>,
    pub table_name: Option<String>,
    pub cache_name: Option<String>,
    pub table_entries: Vec<String>,
    pub string_table_data_only: bool,
    pub cache_null_prototype_data_only: bool,
    pub decoder_manual_base64_v1: bool,
    pub from_char_code_intrinsic: bool,
    #[serde(default)]
    pub decoder_destination_parameter_index: Option<u32>,
    #[serde(default)]
    pub decoder_accepts_padding: bool,
    /// The decoder's alphabet is URL-safe `-_`, not standard `+/`. Omitted
    /// when false, so a standard plan serializes as it always has.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub decoder_url_safe: bool,
    pub declined: Option<(VmStringFoldReason, String)>,
}

impl VmStringFoldPlan {
    /// Compile source proof once into a source-free plan. This function parses
    /// only at compilation time and is never called by artifact loading.
    pub fn compile(report: &PipelineReport) -> Self {
        let mut plan = Self {
            schema_version: VM_STRING_FOLD_PLAN_VERSION.into(),
            compiler_provenance: "shape-rust-oxc-0.1".into(),
            helper_name: None,
            decoder_name: None,
            table_name: None,
            cache_name: None,
            table_entries: Vec::new(),
            string_table_data_only: false,
            cache_null_prototype_data_only: false,
            decoder_manual_base64_v1: false,
            from_char_code_intrinsic: false,
            decoder_destination_parameter_index: None,
            decoder_accepts_padding: false,
            decoder_url_safe: false,
            declined: None,
        };
        let decline = |plan: &mut Self, reason, detail: &str| {
            plan.declined = Some((reason, detail.to_string()));
        };
        let allocator = oxc_allocator::Allocator::default();
        let parsed = analysis::parse(&allocator, &report.output);
        if parsed.panicked || !parsed.errors.is_empty() {
            decline(
                &mut plan,
                VmStringFoldReason::HelperOrDecoderNotProven,
                "pipeline output did not parse",
            );
            return plan;
        }
        let (scoping, _nodes) = analysis::analyze(&parsed.program);
        let Some(table_fact) = report.extraction.iter().find(|fact| {
            fact.kind == ExtractionKind::StringTable && fact.status == ExtractionStatus::Extracted
        }) else {
            decline(
                &mut plan,
                VmStringFoldReason::MissingStringTable,
                "no extracted string table fact",
            );
            return plan;
        };
        let Some(cache_fact) = report.extraction.iter().find(|fact| {
            fact.kind == ExtractionKind::StringCache && fact.status == ExtractionStatus::Extracted
        }) else {
            decline(
                &mut plan,
                VmStringFoldReason::MissingStringCache,
                "no extracted string cache fact",
            );
            return plan;
        };
        let (Some(table_name), Some(cache_name)) =
            (table_fact.binding.as_deref(), cache_fact.binding.as_deref())
        else {
            decline(
                &mut plan,
                VmStringFoldReason::MissingStringTable,
                "string table/cache lacks binding identity",
            );
            return plan;
        };
        let tables = find_string_bindings(&parsed.program, table_name, &scoping);
        let caches = find_cache_bindings(&parsed.program, cache_name, &scoping);
        if tables.len() != 1 {
            decline(
                &mut plan,
                VmStringFoldReason::AmbiguousStringTable,
                "string table binding is ambiguous",
            );
            return plan;
        }
        if caches.len() != 1 {
            decline(
                &mut plan,
                VmStringFoldReason::AmbiguousStringCache,
                "string cache binding is ambiguous",
            );
            return plan;
        }
        if DynamicScope::find(&parsed.program, &scoping) {
            decline(
                &mut plan,
                VmStringFoldReason::DynamicScope,
                "with/direct eval prevents stable binding resolution",
            );
            return plan;
        }
        let table = tables[0].clone();
        let cache = caches[0].clone();
        let functions = function_bindings(&parsed.program);
        let dependencies = DependencyAssumptions::trusted_static();
        let Some((helper, decoder, proof)) = prove_pair(&report.output, &functions, dependencies)
        else {
            decline(
                &mut plan,
                VmStringFoldReason::HelperOrDecoderNotProven,
                "no unique exact helper/decoder pair",
            );
            return plan;
        };
        if let Err(detail) = source_dependencies_are_safe(
            &parsed.program,
            &scoping,
            &helper,
            &decoder,
            table.symbol,
            cache.symbol,
            cache.init_span,
        ) {
            decline(
                &mut plan,
                VmStringFoldReason::MutableDependency,
                &format!("table/cache/decoder/native binding is reassigned, escaped, or observed ({detail})"),
            );
            return plan;
        }
        if helper_calls_are_proven(
            &parsed.program,
            &scoping,
            &helper,
            &decoder,
            table.symbol,
            cache.symbol,
        )
        .is_err()
        {
            decline(
                &mut plan,
                VmStringFoldReason::HelperCallDependencyMismatch,
                "source helper calls do not resolve to the proven table/cache/decoder/fromCharCode bindings",
            );
            return plan;
        }
        if !native_dependency_is_safe(
            &parsed.program,
            &scoping,
            &helper,
            &decoder,
            table.symbol,
            cache.symbol,
        ) {
            decline(
                &mut plan,
                VmStringFoldReason::MutableDependency,
                "String.fromCharCode is shadowed, mutated, deleted, or escaped",
            );
            return plan;
        }
        plan.helper_name = Some(helper.name);
        plan.decoder_name = Some(decoder.name);
        plan.table_name = Some(table_name.to_string());
        plan.cache_name = Some(cache_name.to_string());
        plan.table_entries = table.entries;
        plan.string_table_data_only = true;
        plan.cache_null_prototype_data_only = true;
        plan.decoder_manual_base64_v1 = true;
        plan.from_char_code_intrinsic = true;
        plan.decoder_destination_parameter_index = proof
            .decoder_proof()
            .destination_parameter_index
            .map(|index| index as u32);
        plan.decoder_accepts_padding = proof.decoder_proof().accepts_padding;
        plan.decoder_url_safe = proof.decoder_proof().alphabet == URL_SAFE_ALPHABET;
        plan
    }

    /// Validate version/provenance, bounds, and completeness before the plan is
    /// allowed to drive folding. A declined plan is complete when it has a
    /// bounded detail; a non-declined plan must carry every proof fact and must
    /// not accept padding.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != VM_STRING_FOLD_PLAN_VERSION {
            return Err("unsupported vm-string fold-plan version".into());
        }
        if self.compiler_provenance != "shape-rust-oxc-0.1" {
            return Err("unsupported vm-string fold-plan compiler provenance".into());
        }
        if self.table_entries.len() > 64 * 1024 {
            return Err("vm-string fold table exceeds bound".into());
        }
        if self
            .table_entries
            .iter()
            .any(|entry| entry.len() > 64 * 1024)
        {
            return Err("vm-string fold table entry exceeds bound".into());
        }
        if self
            .decoder_destination_parameter_index
            .is_some_and(|index| index > 32)
        {
            return Err("vm-string decoder destination parameter exceeds bound".into());
        }
        if let Some((_, detail)) = &self.declined {
            if detail.len() > 4096 {
                return Err("vm-string fold decline detail exceeds bound".into());
            }
        } else if self.helper_name.as_deref().is_none_or(str::is_empty)
            || self.decoder_name.as_deref().is_none_or(str::is_empty)
            || self.table_name.as_deref().is_none_or(str::is_empty)
            || self.cache_name.as_deref().is_none_or(str::is_empty)
            || !self.string_table_data_only
            || !self.cache_null_prototype_data_only
            || !self.decoder_manual_base64_v1
            || !self.from_char_code_intrinsic
        {
            return Err("proven vm-string fold plan is incomplete".into());
        }
        if self.declined.is_none() && self.decoder_accepts_padding {
            return Err("vm-string decoder plan permits unproven padding".into());
        }
        Ok(())
    }
}

impl VmStringFoldReason {
    /// Stable kebab-case label used in diagnostics and reason counts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReportInputMismatch => "report-input-mismatch",
            Self::MissingStringTable => "missing-string-table-proof",
            Self::MissingStringCache => "missing-string-cache-proof",
            Self::AmbiguousStringTable => "ambiguous-string-table-binding",
            Self::AmbiguousStringCache => "ambiguous-string-cache-binding",
            Self::DynamicScope => "dynamic-scope",
            Self::MutableDependency => "mutable-dependency",
            Self::HelperOrDecoderNotProven => "helper-or-decoder-not-proven",
            Self::HelperCallDependencyMismatch => "helper-call-dependency-mismatch",
            Self::EmittedCallNotProven => "emitted-call-not-proven",
            Self::EvaluationRejected => "evaluation-rejected",
            Self::Utf16LiteralUnavailable => "utf16-literal-unavailable",
        }
    }
}

/// Outcome counts for one fold operation.
///
/// `folded` counts applied call-span edits; `declined` counts calls or the
/// whole source that stayed unchanged. `diagnostics` is bounded by the number
/// of declined calls and keeps the reason plus a short detail for the caller's
/// stderr summary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VmStringFoldReport {
    pub folded: usize,
    pub declined: usize,
    pub reasons: BTreeMap<VmStringFoldReason, usize>,
    pub diagnostics: Vec<String>,
}

impl VmStringFoldReport {
    pub(crate) fn decline(&mut self, reason: VmStringFoldReason, detail: impl Into<String>) {
        self.declined += 1;
        *self.reasons.entry(reason).or_default() += 1;
        self.diagnostics
            .push(format!("{}: {}", reason.as_str(), detail.into()));
    }
}

/// Folded artifact plus its report. On an ordinary decline, `emitted` is the
/// exact input artifact (including maps) and `folded == 0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmStringFoldResult {
    pub emitted: EmittedJavaScript,
    pub report: VmStringFoldReport,
}

/// Hard failures that abort a caller rather than returning unchanged output.
///
/// A report-input mismatch is a caller/identity error; parse and invalid-output
/// errors mean the source or edit pipeline cannot be trusted. Ordinary proof
/// declines are represented by [`VmStringFoldReport`] instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmStringIntegrationError {
    ReportInputMismatch { expected: String, actual: String },
    Parse(String),
    InvalidOutput(String),
}

impl fmt::Display for VmStringIntegrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReportInputMismatch { expected, actual } => write!(
                f,
                "pipeline report belongs to another capture (expected {expected}, got {actual})"
            ),
            Self::Parse(error) => {
                write!(f, "vm-string integration could not parse source: {error}")
            }
            Self::InvalidOutput(error) => {
                write!(f, "vm-string integration produced invalid output: {error}")
            }
        }
    }
}
impl std::error::Error for VmStringIntegrationError {}

/// Fold concrete, source-proven `vmDecodeString` calls in an emitted trace.
///
/// `capture_source` must be the exact raw source used to produce `report`.
/// The proof itself is constructed against `report.output`, because report
/// spans from earlier pipeline passes are intentionally not reused after
/// final presentation formatting.  If no complete proof is available, the
/// returned artifact is unchanged and the typed report records the decline.
pub fn fold_emitted_vm_strings(
    capture_source: &str,
    report: &PipelineReport,
    emitted: EmittedJavaScript,
) -> Result<VmStringFoldResult, VmStringIntegrationError> {
    let actual = fnv1a64_hex(capture_source.as_bytes());
    if actual != report.input_hash {
        return Err(VmStringIntegrationError::ReportInputMismatch {
            expected: report.input_hash.clone(),
            actual,
        });
    }

    fold_emitted_vm_strings_inner(report, emitted)
}

/// Fold concrete calls using only a complete saved pipeline report.  The
/// report's raw-capture hash is a declared identity here, so no raw bytes are
/// claimed or required; helper/decoder, scope, table, and emitted-call proofs
/// are still rebuilt from `report.output`.
pub fn fold_emitted_vm_strings_from_report(
    report: &PipelineReport,
    emitted: EmittedJavaScript,
) -> Result<VmStringFoldResult, VmStringIntegrationError> {
    config::validate_saved_report(report, Default::default()).map_err(|error| {
        VmStringIntegrationError::InvalidOutput(format!("invalid saved report: {error}"))
    })?;
    fold_emitted_vm_strings_inner(report, emitted)
}

/// Rebuild the full source proof against `report.output`, then fold calls.
///
/// The caller has already established report identity (raw bytes or saved
/// report validation). Every stage here is fail-closed: a missing/ambiguous
/// binding, dynamic scope, unproven pair, mutable dependency, mismatched helper
/// call, or untrusted native binding returns the input artifact with a typed
/// decline. Only genuine parse/edit failures become hard errors.
fn fold_emitted_vm_strings_inner(
    report: &PipelineReport,
    emitted: EmittedJavaScript,
) -> Result<VmStringFoldResult, VmStringIntegrationError> {
    let mut outcome = VmStringFoldReport::default();
    let source = &report.output;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = analysis::parse(&allocator, source);
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(VmStringIntegrationError::Parse(parsed.errors.join("; ")));
    }
    let (scoping, _nodes) = analysis::analyze(&parsed.program);
    let Some(table_fact) = report.extraction.iter().find(|fact| {
        fact.kind == ExtractionKind::StringTable && fact.status == ExtractionStatus::Extracted
    }) else {
        outcome.decline(
            VmStringFoldReason::MissingStringTable,
            "no extracted string table fact",
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    };
    let Some(cache_fact) = report.extraction.iter().find(|fact| {
        fact.kind == ExtractionKind::StringCache && fact.status == ExtractionStatus::Extracted
    }) else {
        outcome.decline(
            VmStringFoldReason::MissingStringCache,
            "no extracted string cache fact",
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    };

    let Some(table_name) = table_fact.binding.as_deref() else {
        outcome.decline(
            VmStringFoldReason::MissingStringTable,
            "string table has no binding identity",
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    };
    let Some(cache_name) = cache_fact.binding.as_deref() else {
        outcome.decline(
            VmStringFoldReason::MissingStringCache,
            "string cache has no binding identity",
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    };
    let tables = find_string_bindings(&parsed.program, table_name, &scoping);
    let caches = find_cache_bindings(&parsed.program, cache_name, &scoping);
    if tables.len() != 1 {
        outcome.decline(
            VmStringFoldReason::AmbiguousStringTable,
            format!(
                "{} candidate bindings for report identity {table_name:?}",
                tables.len()
            ),
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    }
    if caches.len() != 1 {
        outcome.decline(
            VmStringFoldReason::AmbiguousStringCache,
            format!(
                "{} candidate bindings for report identity {cache_name:?}",
                caches.len()
            ),
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    }
    let table = tables[0].clone();
    let cache = caches[0].clone();
    let dynamic_scope = DynamicScope::find(&parsed.program, &scoping);
    if dynamic_scope {
        outcome.decline(
            VmStringFoldReason::DynamicScope,
            "with/direct eval prevents stable binding resolution",
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    }

    let functions = function_bindings(&parsed.program);
    let dependencies = DependencyAssumptions {
        string_table_data_only: true,
        cache: CacheAssumption::TrustedNullPrototypeDataOnly,
        decoder_is_proven_manual_base64: true,
        from_char_code_is_intrinsic: true,
    };
    let Some((helper, decoder, proof)) = prove_pair(source, &functions, dependencies) else {
        outcome.decline(
            VmStringFoldReason::HelperOrDecoderNotProven,
            "no unique exact helper/decoder pair",
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    };
    if let Err(detail) = source_dependencies_are_safe(
        &parsed.program,
        &scoping,
        &helper,
        &decoder,
        table.symbol,
        cache.symbol,
        cache.init_span,
    ) {
        outcome.decline(
            VmStringFoldReason::MutableDependency,
            format!(
                "table/cache/decoder/native binding is reassigned, escaped, or observed ({detail})"
            ),
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    }
    if helper_calls_are_proven(
        &parsed.program,
        &scoping,
        &helper,
        &decoder,
        table.symbol,
        cache.symbol,
    )
    .is_err()
    {
        outcome.decline(
            VmStringFoldReason::HelperCallDependencyMismatch,
            "source helper calls do not resolve to the proven table/cache/decoder/fromCharCode bindings",
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    }
    if !native_dependency_is_safe(
        &parsed.program,
        &scoping,
        &helper,
        &decoder,
        table.symbol,
        cache.symbol,
    ) {
        outcome.decline(
            VmStringFoldReason::MutableDependency,
            "String.fromCharCode is shadowed, mutated, deleted, or escaped",
        );
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    }

    fold_emitted_vm_strings_with_plan_parts(
        emitted,
        &helper.name,
        &decoder.name,
        table_name,
        cache_name,
        table.entries,
        proof,
    )
}

/// Fold from the compact compile-time proof.  Runtime parsing is restricted
/// to emitted JavaScript; no pipeline output or original capture is needed.
pub fn fold_emitted_vm_strings_from_plan(
    plan: &VmStringFoldPlan,
    emitted: EmittedJavaScript,
) -> Result<VmStringFoldResult, VmStringIntegrationError> {
    plan.validate()
        .map_err(VmStringIntegrationError::InvalidOutput)?;
    let Some((reason, detail)) = &plan.declined else {
        let proof = VmStringProof::from_compiled_plan(
            plan.decoder_destination_parameter_index
                .map(|index| index as usize),
            plan.decoder_accepts_padding,
            plan.decoder_url_safe,
            DependencyAssumptions::trusted_static(),
        )
        .map_err(|error| VmStringIntegrationError::InvalidOutput(error.to_string()))?;
        return fold_emitted_vm_strings_with_plan_parts(
            emitted,
            plan.helper_name.as_deref().unwrap_or_default(),
            plan.decoder_name.as_deref().unwrap_or_default(),
            plan.table_name.as_deref().unwrap_or_default(),
            plan.cache_name.as_deref().unwrap_or_default(),
            plan.table_entries.clone(),
            proof,
        );
    };
    let mut report = VmStringFoldReport::default();
    report.decline(*reason, detail.clone());
    Ok(VmStringFoldResult { emitted, report })
}

/// One concrete call of the proven helper: `encoded` with the key at
/// `key_index` of the proven table, as the helper computes it.
fn decode_call(
    proof: &VmStringProof,
    table: &VmStringTable,
    evaluator: &VmStringEvaluator,
    cache_state: &mut VmStringCache,
    encoded: &str,
    key_index: usize,
) -> Result<Utf16String, String> {
    let key = table
        .entries()
        .get(key_index)
        .ok_or("key index is outside the proven table")?;
    let alphabet = proof.decoder_proof().alphabet;
    let key_bytes = decode_proven_base64(key, alphabet, &evaluator.limits())
        .map_err(|reason| format!("key decoder rejected input: {reason}"))?;
    let encoded_bytes = decode_proven_base64(encoded, alphabet, &evaluator.limits())
        .map_err(|reason| format!("input decoder rejected input: {reason}"))?;
    if encoded_bytes.len() < key_bytes.len() {
        return Err("encoded operand is shorter than the proven key".into());
    }
    let evaluator_encoded = encode_unpadded_base64(&encoded_bytes[..key_bytes.len()], alphabet);
    evaluator
        .evaluate(
            Some(proof),
            VmStringRequest {
                encoded: Some(&evaluator_encoded),
                key_index: Some(key_index),
                compute_only: true,
            },
            table,
            cache_state,
        )
        .map(|result| result.value().clone())
        .map_err(|error| error.to_string())
}

/// The proven helper of a [`VmStringFoldPlan`], for decoding its calls while
/// tracing: a call whose operands are known is the string it returns.
#[derive(Debug, Clone)]
pub struct VmStringDecoder {
    pub helper: String,
    pub decoder: String,
    pub table_name: String,
    pub cache: String,
    table: VmStringTable,
    proof: VmStringProof,
}

impl VmStringDecoder {
    /// The plan's helper, when the plan proved it.
    pub fn from_plan(plan: &VmStringFoldPlan) -> Option<Self> {
        if plan.declined.is_some() || plan.validate().is_err() {
            return None;
        }
        let proof = VmStringProof::from_compiled_plan(
            plan.decoder_destination_parameter_index
                .map(|index| index as usize),
            plan.decoder_accepts_padding,
            plan.decoder_url_safe,
            DependencyAssumptions::trusted_static(),
        )
        .ok()?;
        Some(Self {
            helper: plan.helper_name.clone()?,
            decoder: plan.decoder_name.clone()?,
            table_name: plan.table_name.clone()?,
            cache: plan.cache_name.clone()?,
            table: VmStringTable::new(plan.table_entries.clone()),
            proof,
        })
    }

    /// What the helper returns for `encoded` and key `key_index`: `None`
    /// when it declines, or the result is no valid string.
    pub fn decode(&self, encoded: &str, key_index: usize) -> Option<String> {
        let evaluator = VmStringEvaluator::new(VmStringLimits::default());
        let mut cache = VmStringCache::trusted_null_prototype();
        decode_call(
            &self.proof,
            &self.table,
            &evaluator,
            &mut cache,
            encoded,
            key_index,
        )
        .ok()?
        .to_string()
        .ok()
    }
}

/// Link emitted calls to already-proven names and fold concrete literals.
///
/// This is the shared tail for source-backed and plan-backed entry points. The
/// only parsed input is `emitted.code`; the table is data copied from the
/// report/plan, and `proof` authorizes the evaluator but does not by itself
/// prove that an emitted callee is the right function. `call_matches_plan`
/// supplies that final name/dependency link.
fn fold_emitted_vm_strings_with_plan_parts(
    emitted: EmittedJavaScript,
    helper_name: &str,
    decoder_name: &str,
    table_name: &str,
    cache_name: &str,
    table_entries: Vec<String>,
    proof: VmStringProof,
) -> Result<VmStringFoldResult, VmStringIntegrationError> {
    let mut outcome = VmStringFoldReport::default();
    let emitted_allocator = oxc_allocator::Allocator::default();
    let emitted_parsed = analysis::parse(&emitted_allocator, &emitted.code);
    if emitted_parsed.panicked || !emitted_parsed.errors.is_empty() {
        return Err(VmStringIntegrationError::Parse(
            emitted_parsed.errors.join("; "),
        ));
    }
    let (emitted_scoping, _emitted_nodes) = analysis::analyze(&emitted_parsed.program);
    let mut calls = EmittedCallCollector {
        scoping: &emitted_scoping,
        calls: Vec::new(),
    };
    calls.visit_program(&emitted_parsed.program);
    let mut cache_state = VmStringCache::trusted_null_prototype();
    let table = VmStringTable::new(table_entries);
    let evaluator = VmStringEvaluator::new(VmStringLimits::default());
    let mut edits = Vec::new();
    for call in calls.calls {
        if call.callee != helper_name {
            continue;
        }
        if !call_matches_plan(&call, helper_name, decoder_name, table_name, cache_name) {
            outcome.decline(
                VmStringFoldReason::EmittedCallNotProven,
                "callee or dependency argument is not source-linked",
            );
            continue;
        }
        let Some(encoded) = call.encoded.as_deref() else {
            outcome.decline(
                VmStringFoldReason::EmittedCallNotProven,
                "encoded operand is not a string literal",
            );
            continue;
        };
        let Some(key_index) = call.key_index else {
            outcome.decline(
                VmStringFoldReason::EmittedCallNotProven,
                "key operand is not an integer literal",
            );
            continue;
        };
        let value = match decode_call(
            &proof,
            &table,
            &evaluator,
            &mut cache_state,
            encoded,
            key_index,
        ) {
            Ok(value) => value,
            Err(reason) => {
                outcome.decline(VmStringFoldReason::EvaluationRejected, reason);
                continue;
            }
        };
        let Ok(literal) = utf16_literal(&value) else {
            outcome.decline(
                VmStringFoldReason::Utf16LiteralUnavailable,
                "could not construct a lossless JavaScript literal",
            );
            continue;
        };
        edits.push(Edit::replace(call.span, literal));
    }
    if edits.is_empty() {
        return Ok(VmStringFoldResult {
            emitted,
            report: outcome,
        });
    }
    let applied = analysis::apply_edits(&emitted.code, edits);
    if !applied.rejected.is_empty() {
        return Err(VmStringIntegrationError::InvalidOutput(
            analysis::describe_rejected_edits(&applied.rejected),
        ));
    }
    let check_allocator = oxc_allocator::Allocator::default();
    let checked = analysis::parse(&check_allocator, &applied.output);
    if checked.panicked || !checked.errors.is_empty() {
        return Err(VmStringIntegrationError::InvalidOutput(
            checked.errors.join("; "),
        ));
    }
    outcome.folded = applied.applied;
    Ok(VmStringFoldResult {
        emitted: EmittedJavaScript {
            code: applied.output,
            statement_map: emitted.statement_map,
            node_map: emitted.node_map,
            object_parameters: emitted.object_parameters,
        },
        report: outcome,
    })
}

#[derive(Debug, Clone)]
struct BindingCandidate {
    symbol: SymbolId,
    entries: Vec<String>,
    init_span: Span,
}

/// Collect every declarator whose binding name matches and whose initializer is
/// a non-empty dense array of valid string literals. The caller requires
/// exactly one result; zero and multiple both decline.
fn find_string_bindings(
    program: &Program<'_>,
    name: &str,
    scoping: &Scoping,
) -> Vec<BindingCandidate> {
    let mut collector = BindingCollector {
        name,
        scoping,
        tables: Vec::new(),
        caches: Vec::new(),
    };
    collector.visit_program(program);
    collector.tables
}

/// Collect declarators matching the cache name whose initializer is exactly
/// `Object.create(null)` with an unresolved global `Object`. The caller
/// requires exactly one result.
fn find_cache_bindings(
    program: &Program<'_>,
    name: &str,
    scoping: &Scoping,
) -> Vec<BindingCandidate> {
    let mut collector = BindingCollector {
        name,
        scoping,
        tables: Vec::new(),
        caches: Vec::new(),
    };
    collector.visit_program(program);
    collector.caches
}

struct BindingCollector<'a> {
    name: &'a str,
    scoping: &'a Scoping,
    tables: Vec<BindingCandidate>,
    caches: Vec<BindingCandidate>,
}
impl<'a, 'b> Visit<'b> for BindingCollector<'a> {
    fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'b>) {
        let Some(id) = simple_binding(&declaration.id) else {
            walk::walk_variable_declarator(self, declaration);
            return;
        };
        if id.name != self.name {
            walk::walk_variable_declarator(self, declaration);
            return;
        }
        let Some(symbol) = id.symbol_id.get() else {
            walk::walk_variable_declarator(self, declaration);
            return;
        };
        if let Some(Expression::ArrayExpression(array)) = declaration.init.as_ref() {
            let mut entries = Vec::with_capacity(array.elements.len());
            let mut valid = true;
            for element in &array.elements {
                let Some(Expression::StringLiteral(literal)) = element.as_expression() else {
                    valid = false;
                    break;
                };
                let Some(value) = literal_units(literal) else {
                    valid = false;
                    break;
                };
                let Ok(value) = String::from_utf16(&value) else {
                    valid = false;
                    break;
                };
                entries.push(value);
            }
            if valid && !entries.is_empty() {
                self.tables.push(BindingCandidate {
                    symbol,
                    entries,
                    init_span: declaration
                        .init
                        .as_ref()
                        .map(GetSpan::span)
                        .unwrap_or(declaration.span),
                });
            }
        }
        if is_null_cache(declaration.init.as_ref(), self.scoping) {
            self.caches.push(BindingCandidate {
                symbol,
                entries: Vec::new(),
                init_span: declaration
                    .init
                    .as_ref()
                    .map(GetSpan::span)
                    .unwrap_or(declaration.span),
            });
        }
        walk::walk_variable_declarator(self, declaration);
    }
}

#[derive(Debug, Clone)]
struct FunctionBinding {
    span: Span,
    name: String,
    symbol: SymbolId,
}

fn function_bindings(program: &Program<'_>) -> Vec<FunctionBinding> {
    let mut collector = FunctionCollector {
        functions: Vec::new(),
        pending: Vec::new(),
    };
    collector.visit_program(program);
    collector
        .functions
        .sort_by_key(|function| (function.span.start, function.span.end));
    collector.functions.dedup_by(|a, b| a.span == b.span);
    collector.functions
}

struct FunctionCollector {
    functions: Vec<FunctionBinding>,
    pending: Vec<(Span, String, SymbolId)>,
}
impl<'a> Visit<'a> for FunctionCollector {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(id) = &function.id {
            if let Some(symbol) = id.symbol_id.get() {
                self.functions.push(FunctionBinding {
                    span: function.span,
                    name: id.name.to_string(),
                    symbol,
                });
            }
        }
        walk::walk_function(self, function, flags);
    }
    fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'a>) {
        if let (Some(id), Some(init), Some(symbol)) = (
            simple_binding(&declaration.id),
            declaration.init.as_ref(),
            simple_binding(&declaration.id).and_then(|id| id.symbol_id.get()),
        ) {
            if matches!(
                init,
                Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
            ) {
                self.pending
                    .push((init.span(), id.name.to_string(), symbol));
            }
        }
        walk::walk_variable_declarator(self, declaration);
    }
    fn visit_program(&mut self, program: &Program<'a>) {
        walk::walk_program(self, program);
        for (span, name, symbol) in self.pending.drain(..) {
            self.functions.push(FunctionBinding { span, name, symbol });
        }
    }
}

/// Prove one helper and one decoder, using standardized role names only as a
/// search hint. Candidates still go through the exact structural matchers and
/// the result must be unique.
fn prove_pair(
    source: &str,
    functions: &[FunctionBinding],
    dependencies: DependencyAssumptions,
) -> Option<(FunctionBinding, FunctionBinding, VmStringProof)> {
    // Standardized output names the two proven roles.  The name is only a
    // search hint: every candidate still goes through the exact structural
    // proof below.  If the canonical names are absent or do not prove, fall
    // back to scanning every function so renamed helpers remain supported.
    prove_pair_with_names(
        source,
        functions,
        Some("vmDecodeString"),
        Some("vmDecodeBase64"),
        dependencies,
    )
    .or_else(|| prove_pair_with_names(source, functions, None, None, dependencies))
}

fn prove_pair_with_names(
    source: &str,
    functions: &[FunctionBinding],
    helper_name: Option<&str>,
    decoder_name: Option<&str>,
    dependencies: DependencyAssumptions,
) -> Option<(FunctionBinding, FunctionBinding, VmStringProof)> {
    let helpers = functions
        .iter()
        .filter(|function| helper_name.is_none_or(|name| function.name == name))
        .filter(|function| super::prove_helper_candidate(source, function.span).is_ok())
        .cloned()
        .collect::<Vec<_>>();
    let decoders = functions
        .iter()
        .filter(|function| decoder_name.is_none_or(|name| function.name == name))
        .filter_map(|function| {
            super::prove_decoder_candidate(source, function.span).map(|proof| ProvenDecoder {
                function: function.clone(),
                proof,
            })
        })
        .collect::<Vec<_>>();
    if helper_name.is_some() && (helpers.is_empty() || decoders.is_empty()) {
        return None;
    }

    let (helper, decoder) = unique_pair(&helpers, &decoders)?;
    let proof = VmStringProof::from_proven_components(
        source,
        helper.span,
        decoder.function.span,
        decoder.proof,
        dependencies,
    )
    .ok()?;
    Some((helper, decoder.function.clone(), proof))
}

fn unique_pair<'a>(
    helpers: &[FunctionBinding],
    decoders: &'a [ProvenDecoder],
) -> Option<(FunctionBinding, &'a ProvenDecoder)> {
    let mut pair = None;
    for helper in helpers {
        for decoder in decoders {
            if helper.span == decoder.function.span {
                continue;
            }
            if pair.is_some() {
                return None;
            }
            pair = Some((helper.clone(), decoder));
        }
    }
    pair
}

#[derive(Debug, Clone)]
struct ProvenDecoder {
    function: FunctionBinding,
    proof: crate::kernel::decoder_proof::DecoderProof,
}

/// Require every source call to the helper to pass the proven table, cache,
/// decoder, and `String.fromCharCode` bindings in the expected argument slots.
/// A call with unknown operands still needs the proven dependency positions.
fn helper_calls_are_proven(
    program: &Program<'_>,
    scoping: &Scoping,
    helper: &FunctionBinding,
    decoder: &FunctionBinding,
    table: SymbolId,
    cache: SymbolId,
) -> Result<(), String> {
    let mut checker = HelperCallChecker {
        scoping,
        helper: helper.symbol,
        decoder: decoder.symbol,
        table,
        cache,
        calls: 0,
        valid: true,
    };
    checker.visit_program(program);
    if checker.calls > 0 && checker.valid {
        Ok(())
    } else {
        Err("a source helper call did not match the proven ABI".into())
    }
}

fn helper_call_is_proven(
    call: &CallExpression<'_>,
    scoping: &Scoping,
    helper: SymbolId,
    decoder: SymbolId,
    table: SymbolId,
    cache: SymbolId,
) -> bool {
    matches!(&call.callee, Expression::Identifier(id)
        if crate::analysis::identifier_symbol(id, scoping) == Some(helper))
        && (call.arguments.len() == 6 || call.arguments.len() == 7)
        && call
            .arguments
            .get(2)
            .and_then(Argument::as_expression)
            .and_then(|e| crate::analysis::expression_symbol(e, scoping))
            == Some(table)
        && call
            .arguments
            .get(3)
            .and_then(Argument::as_expression)
            .and_then(|e| crate::analysis::expression_symbol(e, scoping))
            == Some(cache)
        && call
            .arguments
            .get(4)
            .and_then(Argument::as_expression)
            .and_then(|e| crate::analysis::expression_symbol(e, scoping))
            == Some(decoder)
        && call
            .arguments
            .get(5)
            .and_then(Argument::as_expression)
            .is_some_and(|e| is_from_char_code(e, scoping))
        && call
            .arguments
            .get(6)
            .and_then(Argument::as_expression)
            .is_none_or(|e| matches!(e, Expression::BooleanLiteral(literal) if !literal.value))
}

struct HelperCallChecker<'a> {
    scoping: &'a Scoping,
    helper: SymbolId,
    decoder: SymbolId,
    table: SymbolId,
    cache: SymbolId,
    calls: usize,
    valid: bool,
}
impl<'a, 'b> Visit<'b> for HelperCallChecker<'a> {
    fn visit_call_expression(&mut self, call: &CallExpression<'b>) {
        if matches!(&call.callee, Expression::Identifier(id)
            if crate::analysis::identifier_symbol(id, self.scoping) == Some(self.helper))
        {
            self.calls += 1;
            self.valid &= helper_call_is_proven(
                call,
                self.scoping,
                self.helper,
                self.decoder,
                self.table,
                self.cache,
            );
        }
        walk::walk_call_expression(self, call);
    }
}

/// Check that table, cache, helper, decoder, and native bindings are closed and
/// data-only: no writes, captures, aliases, member calls, deletes, updates, or
/// unresolved accesses that could observe or replace their values.
fn source_dependencies_are_safe(
    program: &Program<'_>,
    scoping: &Scoping,
    helper: &FunctionBinding,
    decoder: &FunctionBinding,
    table: SymbolId,
    cache: SymbolId,
    cache_init_span: Span,
) -> Result<(), String> {
    let mut checker = DependencyChecker {
        scoping,
        helper: helper.symbol,
        table,
        cache,
        decoder: decoder.symbol,
        invalid: None,
        allowed_references: Vec::new(),
        allowed_decoder_calls: Vec::new(),
        allowed_table_reads: Vec::new(),
        cache_init_span,
    };
    checker.visit_program(program);
    checker.invalid.map_or(Ok(()), Err)
}

/// Prove that the source bindings cannot be observed or changed outside the
/// exact helper call ABI.  This is deliberately reference-based: an alias,
/// return value, member call, delete, or unknown callee all still contain a
/// reference to the protected binding and therefore decline the fold.
struct DependencyChecker<'a> {
    scoping: &'a Scoping,
    helper: SymbolId,
    table: SymbolId,
    cache: SymbolId,
    decoder: SymbolId,
    invalid: Option<String>,
    allowed_references: Vec<Span>,
    allowed_decoder_calls: Vec<Span>,
    allowed_table_reads: Vec<Span>,
    cache_init_span: Span,
}
impl<'a, 'b> Visit<'b> for DependencyChecker<'a> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'b>) {
        match crate::analysis::identifier_symbol(id, self.scoping) {
            Some(symbol)
                if symbol == self.helper
                    || symbol == self.table
                    || symbol == self.cache
                    || symbol == self.decoder =>
            {
                if !self
                    .allowed_references
                    .iter()
                    .any(|span| contains(*span, id.span))
                    && !(symbol == self.decoder
                        && self
                            .allowed_decoder_calls
                            .iter()
                            .any(|span| contains(*span, id.span)))
                    && !(symbol == self.table
                        && self
                            .allowed_table_reads
                            .iter()
                            .any(|span| contains(*span, id.span)))
                {
                    self.invalid = Some(format!(
                        "{} reference at {}..{}",
                        id.name, id.span.start, id.span.end
                    ));
                }
            }
            _ => {}
        }
        walk::walk_identifier_reference(self, id);
    }
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'b>) {
        if crate::analysis::expression_symbol(&member.object, self.scoping) == Some(self.table) {
            self.allowed_table_reads.push(member.object.span());
        }
        walk::walk_static_member_expression(self, member);
    }
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'b>) {
        if crate::analysis::expression_symbol(&member.object, self.scoping) == Some(self.table) {
            self.allowed_table_reads.push(member.object.span());
        }
        walk::walk_computed_member_expression(self, member);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'b>) {
        if member_object_symbol(&call.callee, self.scoping) == Some(self.table) {
            self.invalid = Some("string table member call".into());
        }
        if let Expression::Identifier(id) = &call.callee {
            if crate::analysis::identifier_symbol(id, self.scoping) == Some(self.decoder) {
                self.allowed_decoder_calls.push(id.span);
            }
        }
        if helper_call_is_proven(
            call,
            self.scoping,
            self.helper,
            self.decoder,
            self.table,
            self.cache,
        ) {
            if let Expression::Identifier(id) = &call.callee {
                self.allowed_references.push(id.span);
            }
            for index in 2..=5 {
                if let Some(expression) =
                    call.arguments.get(index).and_then(Argument::as_expression)
                {
                    self.allowed_references.push(expression.span());
                }
            }
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'b>) {
        if target_writes_member_object(&assignment.left, self.scoping, self.table) {
            self.invalid = Some("string table member write".into());
        }
        if target_writes_global_member(&assignment.left, self.scoping, "Object", "create")
            && assignment.span.start < self.cache_init_span.start
        {
            self.invalid = Some("Object.create is reassigned before the cache initializer".into());
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'b>) {
        if !matches!(&statement.left, ForStatementLeft::VariableDeclaration(_))
            && target_writes_member_object(
                statement.left.to_assignment_target(),
                self.scoping,
                self.table,
            )
        {
            self.invalid = Some("string table member write in for-in target".into());
        }
        walk::walk_for_in_statement(self, statement);
    }

    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'b>) {
        if !matches!(&statement.left, ForStatementLeft::VariableDeclaration(_))
            && target_writes_member_object(
                statement.left.to_assignment_target(),
                self.scoping,
                self.table,
            )
        {
            self.invalid = Some("string table member write in for-of target".into());
        }
        walk::walk_for_of_statement(self, statement);
    }
    fn visit_update_expression(&mut self, update: &UpdateExpression<'b>) {
        if simple_target_object_symbol(&update.argument, self.scoping) == Some(self.table) {
            self.invalid = Some("string table member update".into());
        }
        walk::walk_update_expression(self, update);
    }
    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'b>) {
        if unary.operator == UnaryOperator::Delete
            && global_member_matches(&unary.argument, self.scoping, "Object", "create")
            && unary.span.start < self.cache_init_span.start
        {
            self.invalid = Some("Object.create is deleted before the cache initializer".into());
        }
        if unary.operator == UnaryOperator::Delete
            && member_object_symbol(&unary.argument, self.scoping) == Some(self.table)
        {
            self.invalid = Some("string table member delete".into());
        }
        walk::walk_unary_expression(self, unary);
    }
}

fn member_object_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression {
        Expression::StaticMemberExpression(member) => {
            crate::analysis::expression_symbol(&member.object, scoping)
        }
        Expression::ComputedMemberExpression(member) => {
            crate::analysis::expression_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn target_writes_member_object(
    target: &AssignmentTarget<'_>,
    scoping: &Scoping,
    object: SymbolId,
) -> bool {
    let mut checker = TargetMemberChecker {
        scoping,
        object: Some(object),
        global_object: None,
        property: None,
        found: false,
    };
    checker.visit_assignment_target(target);
    checker.found
}

fn target_writes_global_member(
    target: &AssignmentTarget<'_>,
    scoping: &Scoping,
    object: &'static str,
    property: &'static str,
) -> bool {
    let mut checker = TargetMemberChecker {
        scoping,
        object: None,
        global_object: Some(object),
        property: Some(property),
        found: false,
    };
    checker.visit_assignment_target(target);
    checker.found
}

struct TargetMemberChecker<'a> {
    scoping: &'a Scoping,
    object: Option<SymbolId>,
    global_object: Option<&'static str>,
    property: Option<&'static str>,
    found: bool,
}

impl<'a, 'b> Visit<'b> for TargetMemberChecker<'a> {
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'b>) {
        let object_matches = self.object.is_some_and(|object| {
            crate::analysis::expression_symbol(&member.object, self.scoping) == Some(object)
        }) || self
            .global_object
            .is_some_and(|object| is_global_named(&member.object, self.scoping, object));
        let property_matches = self
            .property
            .is_none_or(|property| member.property.name == property);
        self.found |= object_matches && property_matches;
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'b>) {
        let object_matches = self.object.is_some_and(|object| {
            crate::analysis::expression_symbol(&member.object, self.scoping) == Some(object)
        }) || self
            .global_object
            .is_some_and(|object| is_global_named(&member.object, self.scoping, object));
        let property_matches = self.property.is_none_or(|property| {
            matches!(&member.expression, Expression::StringLiteral(key) if key.value == property)
        });
        self.found |= object_matches && property_matches;
        walk::walk_computed_member_expression(self, member);
    }
}

fn global_member_matches(
    expression: &Expression<'_>,
    scoping: &Scoping,
    object: &str,
    property: &str,
) -> bool {
    match expression {
        Expression::StaticMemberExpression(member) => {
            is_global_named(&member.object, scoping, object) && member.property.name == property
        }
        Expression::ComputedMemberExpression(member) => {
            is_global_named(&member.object, scoping, object)
                && matches!(&member.expression, Expression::StringLiteral(key) if key.value == property)
        }
        _ => false,
    }
}

fn is_global_named(expression: &Expression<'_>, scoping: &Scoping, name: &str) -> bool {
    matches!(expression, Expression::Identifier(id) if id.name == name && crate::analysis::identifier_symbol(id, scoping).is_none())
}

fn simple_target_object_symbol(
    target: &SimpleAssignmentTarget<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match target {
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            crate::analysis::expression_symbol(&member.object, scoping)
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            crate::analysis::expression_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

/// Track only the native object writes/escapes that can invalidate the
/// explicit global `String.fromCharCode` assumption. Ordinary reads of the
/// capture's native-function table remain allowed; passing the native object
/// to an unknown call, assigning/deleting its properties, or mutating an
/// alias declines the fold.
fn native_dependency_is_safe(
    program: &Program<'_>,
    scoping: &Scoping,
    helper: &FunctionBinding,
    decoder: &FunctionBinding,
    table: SymbolId,
    cache: SymbolId,
) -> bool {
    let mut checker = NativeDependencyChecker {
        scoping,
        helper: helper.symbol,
        decoder: decoder.symbol,
        table,
        cache,
        string_aliases: Vec::new(),
        from_char_code_aliases: Vec::new(),
        invalid: false,
    };
    checker.collect_aliases(program);
    checker.visit_program(program);
    !checker.invalid
}

struct NativeDependencyChecker<'a> {
    scoping: &'a Scoping,
    helper: SymbolId,
    decoder: SymbolId,
    table: SymbolId,
    cache: SymbolId,
    string_aliases: Vec<SymbolId>,
    from_char_code_aliases: Vec<SymbolId>,
    invalid: bool,
}

impl<'a, 'b> Visit<'b> for NativeDependencyChecker<'a> {
    fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'b>) {
        if let (Some(init), Some(symbol)) = (
            declaration.init.as_ref(),
            simple_binding(&declaration.id).and_then(|id| id.symbol_id.get()),
        ) {
            let direct_string_alias = is_global_string(init, self.scoping)
                || crate::analysis::expression_symbol(init, self.scoping)
                    .is_some_and(|candidate| self.string_aliases.contains(&candidate));
            let direct_from_char_code_alias =
                is_from_char_code_member(init, self.scoping, &self.string_aliases);
            if !direct_string_alias
                && !direct_from_char_code_alias
                && !is_native_function_table_initializer(init, self.scoping)
                && native_escape_value(
                    init,
                    self.scoping,
                    &self.string_aliases,
                    &self.from_char_code_aliases,
                )
            {
                self.invalid = true;
            }
            if is_global_string(init, self.scoping)
                || crate::analysis::expression_symbol(init, self.scoping)
                    .is_some_and(|candidate| self.string_aliases.contains(&candidate))
            {
                self.string_aliases.push(symbol);
            }
            if is_from_char_code_member(init, self.scoping, &self.string_aliases) {
                self.from_char_code_aliases.push(symbol);
            }
        }
        walk::walk_variable_declarator(self, declaration);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'b>) {
        let proven_helper_call = helper_call_is_proven(
            call,
            self.scoping,
            self.helper,
            self.decoder,
            self.table,
            self.cache,
        );
        if !proven_helper_call
            && call.arguments.iter().any(|argument| {
                argument
                    .as_expression()
                    .is_some_and(|expression| self.native_value(expression))
            })
        {
            self.invalid = true;
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'b>) {
        if self.native_assignment_target(&assignment.left)
            || target_writes_native(
                &assignment.left,
                self.scoping,
                &self.string_aliases,
                &self.from_char_code_aliases,
            )
        {
            self.invalid = true;
        } else if let Some(symbol) =
            assignment_target_identifier_symbol(&assignment.left, self.scoping)
        {
            if is_global_string(&assignment.right, self.scoping)
                || crate::analysis::expression_symbol(&assignment.right, self.scoping)
                    .is_some_and(|candidate| self.string_aliases.contains(&candidate))
            {
                self.string_aliases.push(symbol);
            }
            if is_from_char_code_member(&assignment.right, self.scoping, &self.string_aliases) {
                self.from_char_code_aliases.push(symbol);
            }
        } else if native_escape_value(
            &assignment.right,
            self.scoping,
            &self.string_aliases,
            &self.from_char_code_aliases,
        ) {
            self.invalid = true;
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'b>) {
        if !matches!(&statement.left, ForStatementLeft::VariableDeclaration(_))
            && target_writes_native(
                statement.left.to_assignment_target(),
                self.scoping,
                &self.string_aliases,
                &self.from_char_code_aliases,
            )
        {
            self.invalid = true;
        }
        walk::walk_for_in_statement(self, statement);
    }

    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'b>) {
        if !matches!(&statement.left, ForStatementLeft::VariableDeclaration(_))
            && target_writes_native(
                statement.left.to_assignment_target(),
                self.scoping,
                &self.string_aliases,
                &self.from_char_code_aliases,
            )
        {
            self.invalid = true;
        }
        walk::walk_for_of_statement(self, statement);
    }

    fn visit_return_statement(&mut self, statement: &ReturnStatement<'b>) {
        if statement.argument.as_ref().is_some_and(|expression| {
            native_escape_value(
                expression,
                self.scoping,
                &self.string_aliases,
                &self.from_char_code_aliases,
            )
        }) {
            self.invalid = true;
        }
        walk::walk_return_statement(self, statement);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'b>) {
        if self.native_simple_target(&update.argument) {
            self.invalid = true;
        }
        walk::walk_update_expression(self, update);
    }

    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'b>) {
        if unary.operator == UnaryOperator::Delete && self.native_value(&unary.argument) {
            self.invalid = true;
        }
        walk::walk_unary_expression(self, unary);
    }
}

impl NativeDependencyChecker<'_> {
    fn collect_aliases(&mut self, program: &Program<'_>) {
        loop {
            let before = (self.string_aliases.len(), self.from_char_code_aliases.len());
            let mut collector = NativeAliasCollector {
                scoping: self.scoping,
                string_aliases: &mut self.string_aliases,
                from_char_code_aliases: &mut self.from_char_code_aliases,
            };
            collector.visit_program(program);
            if before == (self.string_aliases.len(), self.from_char_code_aliases.len()) {
                break;
            }
        }
    }

    fn native_value(&self, expression: &Expression<'_>) -> bool {
        is_global_string(expression, self.scoping)
            || crate::analysis::expression_symbol(expression, self.scoping).is_some_and(|symbol| {
                self.string_aliases.contains(&symbol)
                    || self.from_char_code_aliases.contains(&symbol)
            })
            || is_string_member(expression, self.scoping, &self.string_aliases)
    }

    fn native_assignment_target(&self, target: &AssignmentTarget<'_>) -> bool {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                is_global_string_id(id, self.scoping)
            }
            AssignmentTarget::StaticMemberExpression(member) => {
                is_string_owner(&member.object, self.scoping, &self.string_aliases)
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                is_string_owner(&member.object, self.scoping, &self.string_aliases)
            }
            _ => false,
        }
    }

    fn native_simple_target(&self, target: &SimpleAssignmentTarget<'_>) -> bool {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                is_global_string_id(id, self.scoping)
            }
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                is_string_owner(&member.object, self.scoping, &self.string_aliases)
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                is_string_owner(&member.object, self.scoping, &self.string_aliases)
            }
            _ => false,
        }
    }
}

struct NativeAliasCollector<'a> {
    scoping: &'a Scoping,
    string_aliases: &'a mut Vec<SymbolId>,
    from_char_code_aliases: &'a mut Vec<SymbolId>,
}

impl<'a, 'b> Visit<'b> for NativeAliasCollector<'a> {
    fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'b>) {
        if let (Some(init), Some(symbol)) = (
            declaration.init.as_ref(),
            simple_binding(&declaration.id).and_then(|id| id.symbol_id.get()),
        ) {
            if is_global_string(init, self.scoping)
                || crate::analysis::expression_symbol(init, self.scoping)
                    .is_some_and(|candidate| self.string_aliases.contains(&candidate))
            {
                push_unique(&mut self.string_aliases, symbol);
            }
            if is_from_char_code_member(init, self.scoping, self.string_aliases) {
                push_unique(&mut self.from_char_code_aliases, symbol);
            }
        }
        walk::walk_variable_declarator(self, declaration);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'b>) {
        if let Some(symbol) = assignment_target_identifier_symbol(&assignment.left, self.scoping) {
            if is_global_string(&assignment.right, self.scoping)
                || crate::analysis::expression_symbol(&assignment.right, self.scoping)
                    .is_some_and(|candidate| self.string_aliases.contains(&candidate))
            {
                push_unique(&mut self.string_aliases, symbol);
            }
            if is_from_char_code_member(&assignment.right, self.scoping, self.string_aliases) {
                push_unique(&mut self.from_char_code_aliases, symbol);
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
}

fn push_unique(values: &mut Vec<SymbolId>, value: SymbolId) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn assignment_target_identifier_symbol(
    target: &AssignmentTarget<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(id) => {
            crate::analysis::identifier_symbol(id, scoping)
        }
        _ => None,
    }
}

fn target_writes_native(
    target: &AssignmentTarget<'_>,
    scoping: &Scoping,
    string_aliases: &[SymbolId],
    from_char_code_aliases: &[SymbolId],
) -> bool {
    let mut checker = NativeTargetChecker {
        scoping,
        string_aliases,
        from_char_code_aliases,
        found: false,
    };
    checker.visit_assignment_target(target);
    checker.found
}

struct NativeTargetChecker<'a> {
    scoping: &'a Scoping,
    string_aliases: &'a [SymbolId],
    from_char_code_aliases: &'a [SymbolId],
    found: bool,
}

impl<'a, 'b> Visit<'b> for NativeTargetChecker<'a> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'b>) {
        self.found |= is_global_string_id(id, self.scoping);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'b>) {
        self.found |= (member.property.name == "String"
            && is_global_named(&member.object, self.scoping, "globalThis"))
            || is_string_owner(&member.object, self.scoping, self.string_aliases)
            || crate::analysis::expression_symbol(&member.object, self.scoping)
                .is_some_and(|symbol| self.from_char_code_aliases.contains(&symbol));
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'b>) {
        self.found |= (matches!(&member.expression, Expression::StringLiteral(key) if key.value == "String")
            && is_global_named(&member.object, self.scoping, "globalThis"))
            || is_string_owner(&member.object, self.scoping, self.string_aliases)
            || crate::analysis::expression_symbol(&member.object, self.scoping)
                .is_some_and(|symbol| self.from_char_code_aliases.contains(&symbol));
        walk::walk_computed_member_expression(self, member);
    }
}

fn native_escape_value(
    expression: &Expression<'_>,
    scoping: &Scoping,
    string_aliases: &[SymbolId],
    from_char_code_aliases: &[SymbolId],
) -> bool {
    let mut finder = NativeEscapeFinder {
        scoping,
        string_aliases,
        from_char_code_aliases,
        found: false,
    };
    finder.visit_expression(expression);
    finder.found
}

fn is_native_function_table_initializer(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    let Expression::ArrayExpression(array) = expression else {
        return false;
    };
    let mut has_string = false;
    let mut has_from_char_code = false;
    let mut has_object_create = false;
    for element in &array.elements {
        let Some(expression) = element.as_expression() else {
            continue;
        };
        has_string |= is_global_string(expression, scoping);
        has_from_char_code |= is_from_char_code_member(expression, scoping, &[]);
        has_object_create |= global_member_matches(expression, scoping, "Object", "create");
    }
    has_string && has_from_char_code && has_object_create
}

struct NativeEscapeFinder<'a> {
    scoping: &'a Scoping,
    string_aliases: &'a [SymbolId],
    from_char_code_aliases: &'a [SymbolId],
    found: bool,
}

impl<'a, 'b> Visit<'b> for NativeEscapeFinder<'a> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'b>) {
        self.found |= is_global_string_id(id, self.scoping)
            || crate::analysis::identifier_symbol(id, self.scoping).is_some_and(|symbol| {
                self.string_aliases.contains(&symbol)
                    || self.from_char_code_aliases.contains(&symbol)
            });
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'b>) {
        self.found |= (member.property.name == "String"
            && is_global_named(&member.object, self.scoping, "globalThis"))
            || (member.property.name == "fromCharCode"
                && is_string_owner(&member.object, self.scoping, self.string_aliases));
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'b>) {
        self.found |= (matches!(&member.expression, Expression::StringLiteral(key) if key.value == "String")
            && is_global_named(&member.object, self.scoping, "globalThis"))
            || (matches!(&member.expression, Expression::StringLiteral(key) if key.value == "fromCharCode")
                && is_string_owner(&member.object, self.scoping, self.string_aliases));
    }

    fn visit_call_expression(&mut self, _call: &CallExpression<'b>) {}
}

fn is_global_string(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(expression, Expression::Identifier(id) if is_global_string_id(id, scoping))
        || global_member_matches(expression, scoping, "globalThis", "String")
}

fn is_global_string_id(id: &IdentifierReference<'_>, scoping: &Scoping) -> bool {
    id.name == "String" && crate::analysis::identifier_symbol(id, scoping).is_none()
}

fn is_string_owner(
    expression: &Expression<'_>,
    scoping: &Scoping,
    string_aliases: &[SymbolId],
) -> bool {
    is_global_string(expression, scoping)
        || crate::analysis::expression_symbol(expression, scoping)
            .is_some_and(|symbol| string_aliases.contains(&symbol))
}

fn is_string_member(
    expression: &Expression<'_>,
    scoping: &Scoping,
    string_aliases: &[SymbolId],
) -> bool {
    match expression {
        Expression::StaticMemberExpression(member) => {
            is_string_owner(&member.object, scoping, string_aliases)
        }
        Expression::ComputedMemberExpression(member) => {
            is_string_owner(&member.object, scoping, string_aliases)
        }
        _ => false,
    }
}

fn is_from_char_code_member(
    expression: &Expression<'_>,
    scoping: &Scoping,
    string_aliases: &[SymbolId],
) -> bool {
    match expression {
        Expression::StaticMemberExpression(member) => {
            member.property.name == "fromCharCode"
                && is_string_owner(&member.object, scoping, string_aliases)
        }
        Expression::ComputedMemberExpression(member) => {
            matches!(&member.expression, Expression::StringLiteral(key) if key.value == "fromCharCode")
                && is_string_owner(&member.object, scoping, string_aliases)
        }
        _ => false,
    }
}

#[derive(Default)]
/// Detects `with` or a direct `eval` reference in the source proof scope.
///
/// Any hit makes every name-based proof unstable, so callers decline the whole
/// fold rather than attempting to reason around the dynamic scope.
struct DynamicScope;
impl DynamicScope {
    fn find(program: &Program<'_>, scoping: &Scoping) -> bool {
        let mut finder = DynamicFinder {
            scoping,
            found: false,
        };
        finder.visit_program(program);
        finder.found
    }
}
struct DynamicFinder<'a> {
    scoping: &'a Scoping,
    found: bool,
}
impl<'a, 'b> Visit<'b> for DynamicFinder<'a> {
    fn visit_with_statement(&mut self, statement: &WithStatement<'b>) {
        self.found = true;
        walk::walk_with_statement(self, statement);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'b>) {
        if matches!(&call.callee, Expression::Identifier(id) if id.name == "eval" && crate::analysis::identifier_symbol(id, self.scoping).is_none())
        {
            self.found = true;
        }
        walk::walk_call_expression(self, call);
    }
}

/// Collects emitted calls that look like the helper and extracts only literal
/// or proven-name operands. No call is evaluated during collection; the folder
/// applies `call_matches_plan` before asking the evaluator.
struct EmittedCallCollector<'a> {
    scoping: &'a Scoping,
    calls: Vec<EmittedCall>,
}
struct EmittedCall {
    span: Span,
    callee: String,
    callee_symbol: Option<SymbolId>,
    argument_count: usize,
    encoded: Option<String>,
    key_index: Option<usize>,
    table: Option<String>,
    table_symbol: Option<SymbolId>,
    cache: Option<String>,
    cache_symbol: Option<SymbolId>,
    decoder: Option<String>,
    decoder_symbol: Option<SymbolId>,
    from_char_code: bool,
    compute_only: Option<bool>,
}
impl<'a, 'b> Visit<'b> for EmittedCallCollector<'a> {
    fn visit_call_expression(&mut self, call: &CallExpression<'b>) {
        if let Expression::Identifier(id) = &call.callee {
            let encoded = call
                .arguments
                .first()
                .and_then(Argument::as_expression)
                .and_then(|e| match e {
                    Expression::StringLiteral(literal) => Some(literal.value.to_string()),
                    _ => None,
                });
            let key_index = call
                .arguments
                .get(1)
                .and_then(Argument::as_expression)
                .and_then(|e| match e {
                    Expression::NumericLiteral(literal)
                        if literal.value.is_finite()
                            && literal.value >= 0.0
                            && literal.value.fract() == 0.0
                            && literal.value <= usize::MAX as f64 =>
                    {
                        Some(literal.value as usize)
                    }
                    _ => None,
                });
            let table = call
                .arguments
                .get(2)
                .and_then(Argument::as_expression)
                .and_then(identifier_name);
            let table_symbol = call
                .arguments
                .get(2)
                .and_then(Argument::as_expression)
                .and_then(|e| crate::analysis::expression_symbol(e, self.scoping));
            let cache = call
                .arguments
                .get(3)
                .and_then(Argument::as_expression)
                .and_then(identifier_name);
            let cache_symbol = call
                .arguments
                .get(3)
                .and_then(Argument::as_expression)
                .and_then(|e| crate::analysis::expression_symbol(e, self.scoping));
            let decoder = call
                .arguments
                .get(4)
                .and_then(Argument::as_expression)
                .and_then(identifier_name);
            let decoder_symbol = call
                .arguments
                .get(4)
                .and_then(Argument::as_expression)
                .and_then(|e| crate::analysis::expression_symbol(e, self.scoping));
            let from_char_code = call
                .arguments
                .get(5)
                .and_then(Argument::as_expression)
                .is_some_and(|e| is_emitted_from_char_code(e, self.scoping));
            let compute_only = call
                .arguments
                .get(6)
                .and_then(Argument::as_expression)
                .and_then(|e| match e {
                    Expression::BooleanLiteral(literal) => Some(literal.value),
                    _ => None,
                });
            self.calls.push(EmittedCall {
                span: call.span,
                callee: id.name.to_string(),
                callee_symbol: crate::analysis::identifier_symbol(id, self.scoping),
                argument_count: call.arguments.len(),
                encoded,
                key_index,
                table,
                table_symbol,
                cache,
                cache_symbol,
                decoder,
                decoder_symbol,
                from_char_code,
                compute_only,
            });
        }
        walk::walk_call_expression(self, call);
    }
}

/// Final emitted-call link: callee and dependency arguments must be exactly the
/// proven names. The name check is a presentation check, not source authority.
fn call_matches_plan(
    call: &EmittedCall,
    helper: &str,
    decoder: &str,
    table: &str,
    cache: &str,
) -> bool {
    call.callee == helper
        && call.callee_symbol.is_none()
        && (call.argument_count == 6 || (call.argument_count == 7 && call.compute_only.is_some()))
        && call.encoded.is_some()
        && call.key_index.is_some()
        && call.table.as_deref() == Some(table)
        && call.table_symbol.is_none()
        && call.cache.as_deref() == Some(cache)
        && call.cache_symbol.is_none()
        && call.decoder.as_deref() == Some(decoder)
        && call.decoder_symbol.is_none()
        && call.from_char_code
}

fn identifier_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(id) => Some(id.name.to_string()),
        _ => None,
    }
}
fn is_from_char_code(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    match expression {
        Expression::ComputedMemberExpression(member) => {
            matches!((&member.object, &member.expression), (Expression::Identifier(id), Expression::StringLiteral(key)) if id.name == "String" && key.value == "fromCharCode" && crate::analysis::identifier_symbol(id, scoping).is_none())
        }
        Expression::StaticMemberExpression(member) => {
            matches!(&member.object, Expression::Identifier(id) if id.name == "String" && crate::analysis::identifier_symbol(id, scoping).is_none())
                && member.property.name == "fromCharCode"
        }
        _ => false,
    }
}
fn is_emitted_from_char_code(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    match expression {
        Expression::ComputedMemberExpression(member) => {
            matches!((&member.object, &member.expression), (Expression::Identifier(id), Expression::StringLiteral(key)) if id.name == "String" && key.value == "fromCharCode" && crate::analysis::identifier_symbol(id, scoping).is_none())
        }
        Expression::StaticMemberExpression(member) => {
            matches!(&member.object, Expression::Identifier(id) if id.name == "String" && crate::analysis::identifier_symbol(id, scoping).is_none())
                && member.property.name == "fromCharCode"
        }
        _ => false,
    }
}
fn simple_binding<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a BindingIdentifier<'a>> {
    match pattern {
        BindingPattern::BindingIdentifier(id) => Some(id),
        _ => None,
    }
}
fn is_null_cache(init: Option<&Expression<'_>>, scoping: &Scoping) -> bool {
    matches!(init, Some(Expression::CallExpression(call)) if matches!(&call.callee, Expression::StaticMemberExpression(member) if matches!(&member.object, Expression::Identifier(id) if id.name == "Object" && crate::analysis::identifier_symbol(id, scoping).is_none()) && member.property.name == "create") && call.arguments.len() == 1 && matches!(call.arguments.first().and_then(Argument::as_expression), Some(Expression::NullLiteral(_))))
}
fn literal_units(literal: &StringLiteral<'_>) -> Option<Vec<u16>> {
    if literal.lone_surrogates {
        literal
            .raw
            .as_ref()
            .and_then(|raw| decode_raw_literal(raw.as_str()))
    } else {
        Some(literal.value.encode_utf16().collect())
    }
}
/// Decode the raw source slice of a string literal into exact UTF-16 units.
///
/// This is intentionally a literal reader, not a JavaScript evaluator: it
/// accepts only the escape forms needed by generated tables and rejects
/// malformed, unsupported, or surrogate-unsafe input. Keeping the raw slice is
/// what preserves a lone surrogate instead of re-cooking the value.
fn decode_raw_literal(raw: &str) -> Option<Vec<u16>> {
    let bytes = raw.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let mut units = Vec::new();
    let mut i = 1;
    while i + 1 < bytes.len() {
        if bytes[i] != b'\\' {
            let c = raw[i..].chars().next()?;
            push_char_units(&mut units, c);
            i += c.len_utf8();
            continue;
        }
        i += 1;
        let c = *bytes.get(i)?;
        i += 1;
        match c {
            b'u' => {
                if bytes.get(i) == Some(&b'{') {
                    i += 1;
                    let end = bytes[i..].iter().position(|b| *b == b'}')? + i;
                    let value = u32::from_str_radix(&raw[i..end], 16).ok()?;
                    push_char_units(&mut units, char::from_u32(value)?);
                    i = end + 1;
                } else {
                    let end = i.checked_add(4)?;
                    let value = u16::from_str_radix(raw.get(i..end)?, 16).ok()?;
                    units.push(value);
                    i = end;
                }
            }
            b'x' => {
                let end = i.checked_add(2)?;
                units.push(u16::from_str_radix(raw.get(i..end)?, 16).ok()?);
                i = end;
            }
            b'n' => units.push(b'\n' as u16),
            b'r' => units.push(b'\r' as u16),
            b't' => units.push(b'\t' as u16),
            b'b' => units.push(8),
            b'f' => units.push(12),
            b'v' => units.push(11),
            b'0' => units.push(0),
            b'\\' | b'\'' | b'"' => units.push(u16::from(c)),
            _ => units.push(u16::from(c)),
        }
    }
    Some(units)
}
/// Emit a lossless JavaScript string literal from UTF-16 units.
///
/// Printable characters stay readable, quotes/backslashes are escaped, controls
/// and unpaired surrogates use `\uXXXX`, and a valid surrogate pair is emitted
/// as its real character. Failure means the output cannot be represented
/// losslessly; callers report `Utf16LiteralUnavailable` rather than emit `?`.
fn utf16_literal(value: &Utf16String) -> Result<String, ()> {
    let units = value.units();
    let mut out = String::from("\"");
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        if (0xD800..=0xDBFF).contains(&unit)
            && units
                .get(index + 1)
                .is_some_and(|next| (0xDC00..=0xDFFF).contains(next))
        {
            let low = units[index + 1];
            let scalar = 0x1_0000 + ((u32::from(unit) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
            let character = char::from_u32(scalar).ok_or(())?;
            push_literal_character(&mut out, character);
            index += 2;
            continue;
        }
        if (0xD800..=0xDFFF).contains(&unit) {
            out.push_str(&format!("\\u{unit:04X}"));
        } else {
            push_literal_character(&mut out, char::from_u32(u32::from(unit)).ok_or(())?);
        }
        index += 1;
    }
    out.push('"');
    Ok(out)
}

fn push_literal_character(out: &mut String, character: char) {
    match character {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        character if character.is_control() || matches!(character, '\u{2028}' | '\u{2029}') => {
            out.push_str(&format!("\\u{:04X}", character as u32));
        }
        character => out.push(character),
    }
}

fn encode_unpadded_base64(bytes: &[u8], alphabet: &str) -> String {
    let alphabet = alphabet.as_bytes();
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut index = 0;
    while index < bytes.len() {
        let a = bytes[index];
        let b = bytes.get(index + 1).copied().unwrap_or(0);
        let c = bytes.get(index + 2).copied().unwrap_or(0);
        output.push(alphabet[(a >> 2) as usize] as char);
        output.push(alphabet[((a & 3) << 4 | b >> 4) as usize] as char);
        if index + 1 < bytes.len() {
            output.push(alphabet[((b & 15) << 2 | c >> 6) as usize] as char);
        }
        if index + 2 < bytes.len() {
            output.push(alphabet[(c & 63) as usize] as char);
        }
        index += 3;
    }
    output
}
fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
fn push_char_units(units: &mut Vec<u16>, character: char) {
    let mut buffer = [0; 2];
    units.extend(character.encode_utf16(&mut buffer).iter().copied());
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tracing::symbolic::{EmittedJavaScript, EmittedStatementMap},
        transforms::deob::pipeline::{
            ExtractionFact, ExtractionKind, ExtractionStatus, PipelineReport,
        },
    };

    use super::super::STANDARD_ALPHABET;

    fn source() -> String {
        format!(
            "{}\n{}\nvar vmStringTable = [\"AAAA\"];\nvar vmStringCache = Object.create(null);\nfunction use() {{ return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String[\"fromCharCode\"]); }}\n",
            include_str!("../../tests/fixtures/retained-decoder-alias.js"),
            super::super::canonical_helper_source()
        )
    }

    fn report(source: &str) -> PipelineReport {
        PipelineReport {
            output: source.into(),
            input_hash: fnv1a64_hex(source.as_bytes()),
            stages: Vec::new(),
            diagnostics: Vec::new(),
            binding_mappings: Vec::new(),
            property_mappings: Vec::new(),
            binding_residue: Vec::new(),
            extraction: vec![
                ExtractionFact {
                    kind: ExtractionKind::StringTable,
                    status: ExtractionStatus::Extracted,
                    binding: Some("vmStringTable".into()),
                    span: Span::new(0, 0),
                    length: Some(1),
                    decoded_bytes: None,
                    value: None,
                    evidence: "test literal table".into(),
                },
                ExtractionFact {
                    kind: ExtractionKind::StringCache,
                    status: ExtractionStatus::Extracted,
                    binding: Some("vmStringCache".into()),
                    span: Span::new(0, 0),
                    length: None,
                    decoded_bytes: None,
                    value: None,
                    evidence: "test null cache".into(),
                },
            ],
            proven_operation_tables: Vec::new(),
        }
    }

    fn emitted_call() -> EmittedJavaScript {
        EmittedJavaScript {
            code: "function trace() { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String[\"fromCharCode\"]); }".into(),
            statement_map: Vec::new(),
            node_map: BTreeMap::new(),
            object_parameters: BTreeMap::new(),
        }
    }

    fn fold_source(source: &str, emitted: &str) -> VmStringFoldResult {
        fold_emitted_vm_strings(
            source,
            &report(source),
            EmittedJavaScript {
                code: emitted.into(),
                statement_map: Vec::new(),
                node_map: BTreeMap::new(),
                object_parameters: BTreeMap::new(),
            },
        )
        .expect("integration")
    }

    #[test]
    fn folds_proven_call_and_preserves_emitted_maps() {
        let source = source();
        let report = report(&source);
        let emitted = EmittedJavaScript {
            code: "function trace() { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String[\"fromCharCode\"]); }".into(),
            statement_map: vec![EmittedStatementMap { trace_statement: 7, emitted_statement: 0 }],
            node_map: BTreeMap::from([(9, 0)]),
            object_parameters: BTreeMap::new(),
        };
        let folded = fold_emitted_vm_strings(&source, &report, emitted).expect("integration");
        assert_eq!(folded.report.folded, 1);
        assert_eq!(folded.report.declined, 0);
        assert!(folded.emitted.code.contains("\\u0003\\u0002"));
        assert!(!folded.emitted.code.contains("vmDecodeString(\"AQID\""));
        assert_eq!(folded.emitted.statement_map.len(), 1);
        assert_eq!(folded.emitted.node_map.get(&9), Some(&0));
    }

    #[test]
    fn compiled_fold_plan_matches_source_backed_fold() {
        let source = source();
        let report = report(&source);
        let plan = VmStringFoldPlan::compile(&report);
        let source_result = fold_emitted_vm_strings(&source, &report, emitted_call()).unwrap();
        let plan_result = fold_emitted_vm_strings_from_plan(&plan, emitted_call()).unwrap();
        assert_eq!(plan_result.emitted.code, source_result.emitted.code);
        assert_eq!(plan_result.report.folded, source_result.report.folded);
        assert_eq!(plan_result.report.declined, source_result.report.declined);
        assert_eq!(plan_result.report.reasons, source_result.report.reasons);
    }

    fn url_safe_source() -> String {
        source().replace("0123456789+/", "0123456789-_")
    }

    fn call_with(encoded: &str) -> EmittedJavaScript {
        EmittedJavaScript {
            code: emitted_call().code.replace("AQID", encoded),
            ..emitted_call()
        }
    }

    #[test]
    fn folds_a_url_safe_decoder_from_source_and_from_its_plan() {
        let source = url_safe_source();
        let report = report(&source);
        let plan = VmStringFoldPlan::compile(&report);
        assert!(plan.decoder_url_safe, "{:?}", plan.declined);
        let from_source = fold_emitted_vm_strings(&source, &report, call_with("-_8A")).unwrap();
        let from_plan = fold_emitted_vm_strings_from_plan(&plan, call_with("-_8A")).unwrap();
        assert_eq!(from_source.report.folded, 1, "{:?}", from_source.report);
        assert_eq!(from_plan.report.folded, 1, "{:?}", from_plan.report);
        assert_eq!(from_plan.emitted.code, from_source.emitted.code);
        // Bytes [251, 255, 0] under the zero key: mask 251, then 4 and 251.
        assert!(from_plan.emitted.code.contains("\\u0004\u{fb}"));
        assert!(!from_plan.emitted.code.contains("vmDecodeString"));
        let decoder = VmStringDecoder::from_plan(&plan).unwrap();
        assert_eq!(decoder.decode("-_8A", 0).as_deref(), Some("\u{4}\u{fb}"));
    }

    #[test]
    fn a_call_in_the_other_alphabet_is_declined() {
        let url_safe = VmStringFoldPlan::compile(&report(&url_safe_source()));
        let folded = fold_emitted_vm_strings_from_plan(&url_safe, call_with("+/8A")).unwrap();
        assert_eq!(folded.report.folded, 0);
        assert!(folded.emitted.code.contains("vmDecodeString"));

        let standard = VmStringFoldPlan::compile(&report(&source()));
        let folded = fold_emitted_vm_strings_from_plan(&standard, call_with("-_8A")).unwrap();
        assert_eq!(folded.report.folded, 0);
        assert!(folded.emitted.code.contains("vmDecodeString"));
    }

    #[test]
    fn a_standard_plan_serializes_without_the_alphabet() {
        let standard = VmStringFoldPlan::compile(&report(&source()));
        let json = serde_json::to_string(&standard).unwrap();
        assert!(!json.contains("decoder_url_safe"));
        assert_eq!(serde_json::from_str::<VmStringFoldPlan>(&json).unwrap(), standard);

        let url_safe = VmStringFoldPlan::compile(&report(&url_safe_source()));
        let json = serde_json::to_string(&url_safe).unwrap();
        assert_eq!(serde_json::from_str::<VmStringFoldPlan>(&json).unwrap(), url_safe);
    }

    #[test]
    fn compiled_plan_folds_strings_longer_than_cache_key_limit() {
        let key = encode_unpadded_base64(&[0; 300], STANDARD_ALPHABET);
        let encoded = encode_unpadded_base64(&[1; 300], STANDARD_ALPHABET);
        let source = source().replace("\"AAAA\"", &format!("\"{key}\""));
        let plan = VmStringFoldPlan::compile(&report(&source));
        let emitted = EmittedJavaScript {
            code: emitted_call().code.replace("AQID", &encoded),
            ..emitted_call()
        };
        let folded = fold_emitted_vm_strings_from_plan(&plan, emitted).unwrap();
        assert_eq!(folded.report.folded, 1, "{:?}", folded.report);
        assert_eq!(folded.report.declined, 0);
        assert!(!folded.emitted.code.contains("vmDecodeString"));
        assert!(folded.emitted.code.contains(&"\\u0000".repeat(299)));
    }

    #[test]
    fn thread_helper_expansion_exposes_decoder_literals() {
        let plan = VmStringFoldPlan::compile(&report(&source()));
        let mut emitted = emitted_call();
        emitted.code = "function trace() { function vm_sub_0(encoded) { return vmDecodeString(encoded, 0, vmStringTable, vmStringCache, decode, String.fromCharCode); } return vm_sub_0(\"AQID\"); }".into();
        let first = fold_emitted_vm_strings_from_plan(&plan, emitted).unwrap();
        assert_eq!(first.report.folded, 0);
        assert_eq!(first.report.declined, 1);
        let cleaned = crate::emitted_cleanup::cleanup_thread(&first.emitted.code).unwrap();
        let retry = fold_emitted_vm_strings_from_plan(
            &plan,
            EmittedJavaScript {
                code: cleaned.code,
                ..emitted_call()
            },
        )
        .unwrap();
        assert_eq!(retry.report.folded, 1, "{:?}", retry.report);
        assert_eq!(retry.report.declined, 0);
        assert!(!retry.emitted.code.contains("vmDecodeString"));
        assert!(retry.emitted.code.contains("\\u0003\\u0002"));
    }

    #[test]
    fn saved_report_folding_does_not_require_raw_capture_bytes() {
        let folded = fold_emitted_vm_strings_from_report(&report(&source()), emitted_call())
            .expect("report-only integration");
        assert_eq!(folded.report.folded, 1);
        assert_eq!(folded.report.declined, 0);
        assert!(folded.emitted.code.contains("\\u0003\\u0002"));
    }

    #[test]
    fn declines_source_aliases_escapes_member_calls_and_native_mutation() {
        for suffix in [
            "var tableAlias = vmStringTable;",
            "function leak() { return vmStringCache; }",
            "Object.defineProperty(vmStringTable, \"0\", { value: \"AAAA\" });",
            "delete vmStringCache.x;",
            "var decoderAlias = decode;",
            "String.fromCharCode = function () { return 0; };",
            "function leakNative(target) { target(String.fromCharCode); }",
        ] {
            let source = format!("{}\n{}\n", source(), suffix);
            let folded = fold_source(
                &source,
                "function trace() { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String[\"fromCharCode\"]); }",
            );
            assert_eq!(folded.report.folded, 0, "unexpected fold for {suffix}");
            assert_eq!(folded.report.declined, 1, "missing decline for {suffix}");
            assert!(
                folded
                    .report
                    .reasons
                    .contains_key(&VmStringFoldReason::MutableDependency)
                    || folded
                        .report
                        .reasons
                        .contains_key(&VmStringFoldReason::HelperOrDecoderNotProven),
                "missing conservative reason for {suffix}: {:?}",
                folded.report.reasons
            );
        }
    }

    #[test]
    fn rejects_nested_table_assignment_targets_and_native_alias_mutation() {
        for suffix in [
            "[vmStringTable[0]] = [\"changed\"];",
            "({ x: vmStringTable[0] } = { x: \"changed\" });",
            "for (vmStringTable[0] of [\"changed\"]) {}",
            "for (vmStringTable[0] in { changed: 1 }) {}",
            "let S; S = String; S.fromCharCode = 0;",
            "let S; S = String; function leak() { return S; }",
            "let S; S = String; var boxed = { value: S };",
            "let S; S = String; ({ fromCharCode: S.fromCharCode } = {});",
            "globalThis.String.fromCharCode = 0;",
        ] {
            let source = format!("{}\n{}\n", source(), suffix);
            let folded = fold_source(
                &source,
                "function trace() { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String[\"fromCharCode\"]); }",
            );
            assert_eq!(folded.report.folded, 0, "unexpected fold for {suffix}");
            assert_eq!(folded.report.declined, 1, "missing decline for {suffix}");
            assert!(
                folded
                    .report
                    .reasons
                    .contains_key(&VmStringFoldReason::MutableDependency)
                    || folded
                        .report
                        .reasons
                        .contains_key(&VmStringFoldReason::HelperOrDecoderNotProven),
                "missing conservative reason for {suffix}: {:?}",
                folded.report.reasons
            );
        }
    }

    #[test]
    fn rejects_object_create_mutation_before_cache_initializer_and_orders_post_init_check() {
        for mutation in [
            "Object.create = function () { return {}; };",
            "delete Object.create;",
        ] {
            let source = source().replace(
                "var vmStringCache = Object.create(null);",
                &format!("{}\nvar vmStringCache = Object.create(null);", mutation),
            );
            let folded = fold_source(
                &source,
                "function trace() { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String.fromCharCode); }",
            );
            assert_eq!(folded.report.folded, 0, "unexpected fold for {mutation}");
            assert!(
                folded
                    .report
                    .reasons
                    .contains_key(&VmStringFoldReason::MutableDependency)
                    || folded
                        .report
                        .reasons
                        .contains_key(&VmStringFoldReason::HelperOrDecoderNotProven),
                "missing cache mutation decline for {mutation}: {:?}",
                folded.report.reasons
            );
        }

        let source = format!("{}\nObject.create = 0;\n", source());
        let folded = fold_source(
            &source,
            "function trace() { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String.fromCharCode); }",
        );
        // The broader helper proof conservatively declines the host mutation,
        // while the dependency checker itself correctly ignores it after the
        // cache object has already been initialized (asserted below).
        assert_eq!(folded.report.folded, 0);
        assert_eq!(
            folded
                .report
                .reasons
                .get(&VmStringFoldReason::HelperOrDecoderNotProven),
            Some(&1)
        );

        let allocator = oxc_allocator::Allocator::default();
        let parsed = analysis::parse(&allocator, &source);
        let (scoping, _) = analysis::analyze(&parsed.program);
        let table = find_string_bindings(&parsed.program, "vmStringTable", &scoping);
        let cache = find_cache_bindings(&parsed.program, "vmStringCache", &scoping);
        let functions = function_bindings(&parsed.program);
        let helper = functions
            .iter()
            .find(|function| function.name == "vmDecodeString")
            .expect("canonical helper");
        let decoder = functions
            .iter()
            .find(|function| function.name == "decode")
            .expect("retained decoder");
        assert!(source_dependencies_are_safe(
            &parsed.program,
            &scoping,
            helper,
            decoder,
            table[0].symbol,
            cache[0].symbol,
            cache[0].init_span,
        )
        .is_ok());
    }

    #[test]
    fn preserves_safe_source_table_member_reads() {
        let source = format!("{}\nvar tableLength = vmStringTable.length;\n", source());
        let folded = fold_source(
            &source,
            "function trace() { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String.fromCharCode); }",
        );
        assert_eq!(folded.report.folded, 1);
        assert_eq!(folded.report.declined, 0);
    }

    #[test]
    fn rejects_shadowed_cache_intrinsic_and_emitted_dependencies() {
        let shadowed_source = source().replace(
            "var vmStringCache = Object.create(null);",
            "var Object = { create: function () { return {}; } };\nvar vmStringCache = Object.create(null);",
        );
        let folded = fold_source(
            &shadowed_source,
            "function trace() { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String[\"fromCharCode\"]); }",
        );
        assert_eq!(folded.report.folded, 0);
        assert_eq!(
            folded
                .report
                .reasons
                .get(&VmStringFoldReason::AmbiguousStringCache),
            Some(&1)
        );

        for emitted in [
            "function trace(vmDecodeString) { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String.fromCharCode); }",
            "function trace(vmStringTable) { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String.fromCharCode); }",
            "function trace(String) { return vmDecodeString(\"AQID\", 0, vmStringTable, vmStringCache, decode, String.fromCharCode); }",
        ] {
            let folded = fold_source(&source(), emitted);
            assert_eq!(folded.report.folded, 0, "unexpected fold for {emitted}");
            assert_eq!(
                folded.report.reasons.get(&VmStringFoldReason::EmittedCallNotProven),
                Some(&1),
                "missing emitted-scope decline for {emitted}"
            );
        }
    }

    #[test]
    fn emits_readable_lossless_utf16_literals() {
        assert_eq!(
            utf16_literal(&Utf16String::from_units(
                b"GET".iter().map(|&unit| u16::from(unit)).collect()
            ))
            .unwrap(),
            "\"GET\""
        );
        assert_eq!(
            utf16_literal(&Utf16String::from_units(vec![
                b'\n' as u16,
                b'\"' as u16,
                b'\\' as u16
            ]))
            .unwrap(),
            "\"\\u000A\\\"\\\\\""
        );
        assert_eq!(
            utf16_literal(&Utf16String::from_units(vec![0xD83C, 0xDF0D, 0xD800])).unwrap(),
            "\"🌍\\uD800\""
        );
    }

    #[test]
    fn rejects_report_for_different_raw_capture() {
        let source = source();
        let error =
            fold_emitted_vm_strings(&format!("{source}x"), &report(&source), emitted_call())
                .expect_err("mismatched raw source must not use the report");
        assert!(matches!(
            error,
            VmStringIntegrationError::ReportInputMismatch { .. }
        ));
    }

    #[test]
    fn arbitrary_same_spelling_is_not_authorized_without_report_proof() {
        let source = "function vmDecodeString() {}\n";
        let report = report(source);
        let emitted = emitted_call();
        let folded = fold_emitted_vm_strings(&source, &report, emitted).expect("integration");
        assert_eq!(folded.report.folded, 0);
        assert_eq!(
            folded
                .report
                .reasons
                .get(&VmStringFoldReason::AmbiguousStringTable),
            Some(&1)
        );
    }

    fn decoder_source(name: &str) -> String {
        let fixture = include_str!("../../tests/fixtures/retained-decoder-alias.js");
        let start = fixture
            .find("function decode(")
            .expect("decoder fixture function");
        format!(
            "{}{}",
            &fixture[..start],
            fixture[start..].replacen("function decode(", &format!("function {name}("), 1)
        )
    }

    fn helper_source(name: &str) -> String {
        super::super::canonical_helper_source()
            .replace("function vmDecodeString(", &format!("function {name}("))
    }

    fn function_spans(source: &str) -> Vec<FunctionBinding> {
        let allocator = oxc_allocator::Allocator::default();
        let parsed = analysis::parse(&allocator, source);
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let _ = analysis::analyze(&parsed.program);
        function_bindings(&parsed.program)
    }

    fn baseline_pair_spans(
        source: &str,
        functions: &[FunctionBinding],
        dependencies: DependencyAssumptions,
    ) -> Option<(Span, Span)> {
        let mut matches = Vec::new();
        for helper in functions {
            for decoder in functions {
                if helper.span == decoder.span {
                    continue;
                }
                if VmStringProof::prove(source, helper.span, decoder.span, dependencies).is_ok() {
                    matches.push((helper.span, decoder.span));
                }
            }
        }
        (matches.len() == 1).then(|| matches[0])
    }

    #[test]
    fn factorized_pair_search_matches_cartesian_proof_search() {
        let dependencies = DependencyAssumptions::trusted_static();
        let decoder = decoder_source("nobodyDecoder");
        let decoder_two = decoder_source("anotherNobodyDecoder");
        let helper = helper_source("nobodyHelper");
        let helper_two = helper_source("anotherNobodyHelper");

        let cases = [
            (
                "renamed decoys",
                format!(
                    "function vmDecodeString() {{}}\nfunction decode() {{}}\n{decoder}\n{helper}\n"
                ),
            ),
            (
                "duplicate helpers",
                format!("{decoder}\n{helper}\n{helper_two}\n"),
            ),
            (
                "duplicate valid decoders",
                format!("{decoder}\n{decoder_two}\n{helper}\n"),
            ),
        ];

        for (label, source) in cases {
            let functions = function_spans(&source);
            let expected = baseline_pair_spans(&source, &functions, dependencies);
            let actual = prove_pair(&source, &functions, dependencies)
                .map(|(found_helper, found_decoder, _)| (found_helper.span, found_decoder.span));
            assert_eq!(actual, expected, "pair-search mismatch for {label}");
            if label == "renamed decoys" {
                assert!(
                    actual.is_some(),
                    "renamed exact candidates were not admitted: expected={expected:?}, functions={:?}",
                    functions.iter().map(|function| &function.name).collect::<Vec<_>>()
                );
                let (helper_span, decoder_span) = actual.expect("renamed pair");
                assert_ne!(
                    functions
                        .iter()
                        .find(|function| function.name == "vmDecodeString")
                        .map(|function| function.span),
                    Some(helper_span),
                    "a name-only helper decoy was admitted"
                );
                assert_ne!(
                    functions
                        .iter()
                        .find(|function| function.name == "decode")
                        .map(|function| function.span),
                    Some(decoder_span),
                    "a name-only decoder decoy was admitted"
                );
            }
        }

        let source = format!("{decoder}\n{helper}\n");
        let functions = function_spans(&source);
        let helper = functions
            .iter()
            .find(|function| function.name == "nobodyHelper")
            .expect("renamed helper");
        let decoder = functions
            .iter()
            .find(|function| function.name == "nobodyDecoder")
            .expect("renamed decoder");
        let decoder_proof = super::super::prove_decoder_candidate(&source, decoder.span)
            .expect("exact decoder proof");
        let same_span = [ProvenDecoder {
            function: helper.clone(),
            proof: decoder_proof,
        }];
        assert!(
            unique_pair(std::slice::from_ref(helper), &same_span).is_none(),
            "a helper/decoder candidate sharing one span must be excluded"
        );
    }
}
