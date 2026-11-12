//! Apply a [`SemanticNamingPlan`] to recovered JavaScript with the lexical
//! rewrite engine.
//!
//! The plan is produced by [`crate::naming::semantic_names::plan_semantic_names`] from one
//! exact source string and carries, per candidate, the byte offset of the
//! declaring identifier. This module feeds those offsets to
//! [`super::transform_program`] as ordinary [`ProgramBindingRename`] plans, so a
//! rename moves the declaration and every reference that lexically resolves to
//! it and never a string, a member name or a comment. Nothing here searches or
//! replaces text.
//!
//! The transformation consumes the *same* source the plan was derived from; the
//! result is stamped with the input and output SHA-256. Because the plan is
//! built from structure, a second pass over the already-renamed output proposes
//! nothing further: the descriptive names no longer match the generated
//! spellings, and the role recognizers never key on a spelling.

use crate::naming::semantic_names::{plan_semantic_names, NamingCandidate, SemanticNamesError, SemanticNamingPlan};
use crate::signals::schema::SourceSpan;

use super::anonymous_names::{
    apply_anonymous_names_mapped, AnonymousNameOptions, AnonymousNamesResult,
};
use super::local_names::{apply_local_names, LocalNameOptions, LocalNamesResult};
use super::{
    transform_program, ProgramBindingRename, ProgramTransformDiagnostic, ProgramTransformError,
    ProgramTransformOptions, RenameSkipReason,
};

/// A hard failure while producing or applying a naming plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticRenameError {
    /// The plan could not be derived.
    Plan(SemanticNamesError),
    /// The source failed to parse during rewriting.
    Transform(ProgramTransformError),
}

impl std::fmt::Display for SemanticRenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plan(error) => write!(f, "{error}"),
            Self::Transform(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for SemanticRenameError {}

impl From<SemanticNamesError> for SemanticRenameError {
    fn from(value: SemanticNamesError) -> Self {
        Self::Plan(value)
    }
}

impl From<ProgramTransformError> for SemanticRenameError {
    fn from(value: ProgramTransformError) -> Self {
        Self::Transform(value)
    }
}

/// One candidate's outcome after the rewrite engine resolved it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticRenameOutcome {
    pub candidate: NamingCandidate,
    pub applied: bool,
    /// A stable machine label for why the plan was skipped.
    pub skip_reason: Option<String>,
    pub declaration_span: Option<crate::signals::schema::SourceSpan>,
    pub reference_edits: usize,
    pub shorthand_edits: usize,
}

/// The naming plan plus the rewrite it produced.
#[derive(Clone, Debug)]
pub struct SemanticRenameResult {
    pub plan: SemanticNamingPlan,
    pub outcomes: Vec<SemanticRenameOutcome>,
    pub transformed_source: String,
    pub source_sha256: String,
    pub transformed_sha256: String,
    /// Renames the engine applied.
    pub applied: usize,
    /// Renames the engine skipped (collision, unsupported site, and so on).
    pub skipped: usize,
    /// Roles left unnamed by the plan because two bindings tied.
    pub ambiguous: usize,
    pub diagnostics: Vec<ProgramTransformDiagnostic>,
    /// The opt-in anonymous-function edits, applied after the renames
    /// (empty when both options are off).
    pub anonymous: AnonymousNamesResult,
    /// The local-variable readability pass, applied last.
    pub local_names: LocalNamesResult,
}

/// Options for [`rename_semantic_names_with_options`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticRenameOptions {
    /// The opt-in anonymous-function edits.
    pub anonymous: AnonymousNameOptions,
    /// The local-variable readability pass. On by default; `disabled` keeps the
    /// semantic-only output byte-identical to the pre-local-names result.
    pub local_names: LocalNameOptions,
}

impl Default for SemanticRenameOptions {
    fn default() -> Self {
        Self {
            anonymous: AnonymousNameOptions::default(),
            local_names: LocalNameOptions::default(),
        }
    }
}

impl SemanticRenameResult {
    /// True when the rewrite changed the source.
    pub fn changed(&self) -> bool {
        self.source_sha256 != self.transformed_sha256
    }
}

/// Build the descriptive naming plan for `source` and apply it with the lexical
/// rewrite engine.
pub fn rename_semantic_names(source: &str) -> Result<SemanticRenameResult, SemanticRenameError> {
    rename_semantic_names_with_options(source, SemanticRenameOptions::default())
}

/// [`rename_semantic_names`], then the opt-in anonymous-function edits for the
/// plan's [`crate::naming::semantic_names::AnonymousFunctionName`] entries, applied to the
/// renamed output (see `anonymous_names`).
///
/// Local-variable names are on by default; use
/// [`rename_semantic_names_with_options`] with
/// [`LocalNameOptions::disabled`] for the semantic-only output.
pub fn rename_semantic_names_with(
    source: &str,
    anonymous: AnonymousNameOptions,
) -> Result<SemanticRenameResult, SemanticRenameError> {
    rename_semantic_names_with_options(
        source,
        SemanticRenameOptions {
            anonymous,
            local_names: LocalNameOptions::default(),
        },
    )
}

/// [`rename_semantic_names`], then the opt-in anonymous-function edits and the
/// local-variable readability pass. The local pass runs last, after the
/// identifier rename and the anonymous names, so it sees the final
/// function/MAIN names.
pub fn rename_semantic_names_with_options(
    source: &str,
    options: SemanticRenameOptions,
) -> Result<SemanticRenameResult, SemanticRenameError> {
    let plan = plan_semantic_names(source)?;
    let renames: Vec<ProgramBindingRename> = plan
        .candidates
        .iter()
        .map(|candidate| {
            ProgramBindingRename::new(candidate.identifier.start, candidate.proposed_name.clone())
        })
        .collect();

    let transform = transform_program(
        source,
        &renames,
        &[],
        &ProgramTransformOptions {
            rename_bindings: true,
            annotate_signals: false,
        },
    )?;

    // `transform_program` yields one outcome per rename plan, in input order.
    let mut by_start = std::collections::BTreeMap::new();
    for outcome in &transform.renames {
        by_start.insert(outcome.declaration_start, outcome);
    }

    let mut outcomes = Vec::with_capacity(plan.candidates.len());
    let mut applied = 0usize;
    for candidate in &plan.candidates {
        let outcome = by_start.get(&candidate.identifier.start);
        let (is_applied, skip_reason, declaration_span, reference_edits, shorthand_edits) =
            match outcome {
                Some(outcome) => (
                    outcome.applied,
                    outcome.skip_reason.as_ref().map(skip_label),
                    outcome
                        .declaration_span
                        .map(|(start, end)| SourceSpan::new(start, end)),
                    outcome.reference_edits,
                    outcome.shorthand_edits,
                ),
                None => (false, Some("unresolved".to_string()), None, 0, 0),
            };
        if is_applied {
            applied += 1;
        }
        outcomes.push(SemanticRenameOutcome {
            candidate: candidate.clone(),
            applied: is_applied,
            skip_reason,
            declaration_span,
            reference_edits,
            shorthand_edits,
        });
    }

    let skipped = outcomes.len() - applied;
    let mut anonymous = apply_anonymous_names_mapped(
        source,
        &transform.transformed_source,
        &plan.anonymous_functions,
        options.anonymous,
    )?;
    let anonymous_source = std::mem::take(&mut anonymous.transformed_source);
    let local_names = apply_local_names(&anonymous_source, options.local_names)?;
    let transformed_source = local_names.transformed_source.clone();
    Ok(SemanticRenameResult {
        transformed_sha256: crate::source::sha256_hex(transformed_source.as_bytes()),
        source_sha256: plan.source.sha256.clone(),
        plan,
        outcomes,
        transformed_source,
        applied,
        skipped,
        ambiguous: transform
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic.code,
                    super::DiagnosticCode::SimultaneousCollision
                        | super::DiagnosticCode::NameCollision
                        | super::DiagnosticCode::DuplicateTarget
                )
            })
            .count(),
        diagnostics: transform.diagnostics,
        anonymous,
        local_names,
    })
}

/// A stable machine label for a rename skip reason.
fn skip_label(reason: &RenameSkipReason) -> String {
    match reason {
        RenameSkipReason::Disabled => "disabled",
        RenameSkipReason::NoDeclarationAtOffset => "no_declaration_at_offset",
        RenameSkipReason::InvalidCanonicalName => "invalid_canonical_name",
        RenameSkipReason::Unchanged => "unchanged",
        RenameSkipReason::NameCollision => "name_collision",
        RenameSkipReason::UnsupportedBindingSite(_) => "unsupported_binding_site",
        RenameSkipReason::DynamicScope => "dynamic_scope",
        RenameSkipReason::DuplicateTarget => "duplicate_target",
        RenameSkipReason::SimultaneousCollision => "simultaneous_collision",
        RenameSkipReason::OverlappingEdit => "overlapping_edit",
        RenameSkipReason::RewriteRejected => "rewrite_rejected",
    }
    .to_string()
}
