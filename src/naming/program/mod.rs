//! Program-level rewrites over recovered (original) JavaScript.
//!
//! This module is the *rewrite engine* of the program layer. It applies two
//! kinds of source-level edits to recovered program text and nothing else:
//!
//! - **Binding renames** ([`ProgramBindingRename`]): give the binding declared
//!   at a documented byte offset a role name, moving the declaration and every
//!   reference that lexically resolves to it. Resolution uses Oxc's lexical
//!   [`Scoping`](oxc_semantic::Scoping), so a shadowed inner binding is renamed
//!   without touching an outer binding that shares its spelling.
//! - **Signal annotations** ([`SignalAnnotation`]): insert explanatory `//`
//!   comments at statement boundaries. Comments are opt-in and never change
//!   how the program parses.
//!
//! The engine is deliberately independent of how roles are decided. It takes
//! *explicit* rename and annotation plans produced by upstream structural
//! identification and never invents a name, a signal index, a key or a
//! constant from its own assumptions. Names are labels the caller supplies;
//! the engine only proves where they must be written.
//!
//! # Coordinates
//!
//! Every offset in this API is a **UTF-8 byte offset into the exact source
//! string passed to [`transform_program`]** — the same coordinate space Oxc
//! uses for [`Span`](oxc_span::Span).
//!
//! - [`ProgramBindingRename::declaration_start`] is the first byte of the
//!   *binding identifier itself*: the `x` in `let x = 1`, the `f` in
//!   `function f() {}`, a parameter name, a destructuring target, a `catch`
//!   parameter or an import binding. It is **not** the offset of the `let`,
//!   `var`, `const`, `function` or `class` keyword, and not the offset of an
//!   enclosing pattern or statement. If no binding identifier begins at that
//!   offset the plan is skipped and reported, never guessed.
//! - [`SignalAnnotation::statement_start`] is the first byte of an existing
//!   statement's span. The comment is inserted just before that byte.
//!
//! # Safety model
//!
//! The engine fails closed. A rename is skipped, with a typed reason, when:
//!
//! - no binding identifier begins at `declaration_start`;
//! - the replacement text is not a valid, non-reserved identifier name;
//! - the replacement name is captured by (or would capture) another binding
//!   visible in the same region, or an unresolved global of that name is used
//!   inside the binding's scope;
//! - another binding of the replacement name, referenced or not, shares a
//!   declaration scope with the renamed one: the same scope, or any block a
//!   `var` (or a sloppy-mode block function) hoists through. Two `var`s would
//!   silently merge and a lexical pair would be a redeclaration error;
//! - a top-level script binding would be renamed to `undefined`, `NaN` or
//!   `Infinity`, which are non-configurable global properties;
//! - a sloppy-mode block-level function is used outside its block through its
//!   Annex B function-scope binding, which Oxc leaves unresolved;
//! - the source uses a dynamic scope construct (`with`, or an `eval`
//!   reference) so lexical resolution cannot be trusted;
//! - a binding site cannot be rewritten exactly (an `import`/`export`
//!   shorthand) — the whole plan is skipped rather than renamed partially;
//! - two plans in the same batch would give *different* bindings the same name
//!   where the two collide (the same scope, or a parent scope whose outer
//!   binding is referenced inside the child scope). Both plans are skipped, so
//!   a batch can never collapse two bindings into one name or introduce a
//!   capture the original program did not have. Sibling scopes that cannot see
//!   each other may still share a name.
//!
//! Edits are applied in one pass through [`crate::analysis::apply_edits`]. If
//! any edit is rejected, the rewritten text no longer parses, or Oxc's
//! semantic checker reports an early error (such as a redeclaration) that the
//! original did not have, the engine discards the whole rewrite and returns
//! the original source with every outcome marked skipped. It never returns a
//! half-applied rename. The semantic check backs up the per-plan collision
//! checks; it cannot see two `var`s silently merging, so those checks remain
//! the primary defence.
//!
//! Renames never use textual search/replace. Only identifier spans from the
//! AST are edited, so string literals, comments, member names, static property
//! keys and *literal* computed keys (`obj["x"]`) keep their spelling. A
//! computed key that is an identifier reference to the renamed binding
//! (`obj[x]`) is a reference, so it moves with the binding. Object/pattern
//! shorthands are expanded (`{ x }` becomes `{ x: canonical }`) so the property
//! *name* is preserved while the binding changes.
//!
//! # Limitations
//!
//! - Each plan is checked against the *original* program and then the whole
//!   batch is checked for simultaneous collisions, so no ordered-name swaps are
//!   needed: a batch that depends on another plan running first (swapping two
//!   names) is reported as a collision and skipped rather than sequenced.
//! - Renaming a symbol declared by an `import`/`export` shorthand is unsupported
//!   and skipped; the engine does not guess how to rewrite the specifier.
//! - When any `with`/`eval` is present, every rename is skipped. Comments are
//!   unaffected because they cannot change resolution.

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::SemanticBuilder;

use crate::analysis;

mod annotate;
mod anonymous_names;
mod identified;
mod local_names;
mod rename;
mod apply_semantic_names;

pub use anonymous_names::{
    apply_anonymous_names, apply_anonymous_names_mapped, AnonymousNameAction, AnonymousNameOptions,
    AnonymousNameOutcome, AnonymousNamesResult, NameSkip,
};
pub use identified::{
    identify_and_transform, identify_and_transform_with_local_names, signal_annotations,
    signal_comment, IdentifiedProgram, IdentifyError, IdentifyOptions,
};
pub use local_names::{
    apply_local_names, LocalBindingKind, LocalNameOptions, LocalNameOutcome, LocalNameRefusal,
    LocalNamesResult,
};
pub(crate) use rename::{planned_binding_facts_for, planned_bindings_collide, PlannedBindingFacts};
pub use rename::{RenameOutcome, RenameSkipReason};
pub use apply_semantic_names::{
    rename_semantic_names, rename_semantic_names_with, rename_semantic_names_with_options,
    SemanticRenameError, SemanticRenameOptions, SemanticRenameOutcome, SemanticRenameResult,
};

/// A source region as `(start, end)` byte offsets.
pub type SpanRange = (u32, u32);

/// Name the binding declared at `declaration_start` with `canonical_name`.
///
/// See the module documentation for the exact meaning of `declaration_start`.
/// The plan is resolved against the parsed program by lexical symbol identity;
/// the declaration and all its references move together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramBindingRename {
    /// First byte of the declared binding identifier in the original source.
    pub declaration_start: u32,
    /// The replacement identifier name. Fixed by upstream identification.
    pub canonical_name: String,
}

impl ProgramBindingRename {
    /// Construct a rename plan.
    pub fn new(declaration_start: u32, canonical_name: impl Into<String>) -> Self {
        Self {
            declaration_start,
            canonical_name: canonical_name.into(),
        }
    }
}

/// One leaf kind a signal carries, used only to describe a signal in a
/// comment. This is presentation, not analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalLeafKind {
    String,
    Number,
    Byte,
}

impl SignalLeafKind {
    /// The schema word printed in a comment: `STRING`, `NUMBER` or `BYTE`.
    pub fn schema(self) -> &'static str {
        match self {
            Self::String => "STRING",
            Self::Number => "NUMBER",
            Self::Byte => "BYTE",
        }
    }
}

/// A structured description of one extracted signal input.
///
/// The generator inputs come first: the positional index always leads, the
/// optional semantic label follows as a descriptive name, then the leaf schema
/// and the seed-expression role. Nothing here is inferred by the engine; a
/// field is printed only when the caller supplies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalDescriptor {
    /// Positional signal index, in extraction order.
    pub signal_index: u32,
    /// Descriptive label for the signal's role, when one is known.
    pub semantic_label: Option<String>,
    /// The leaf kind this input contributes.
    pub leaf_kind: SignalLeafKind,
    /// The seed-expression role the input is consumed by, when known.
    pub seed_role: Option<String>,
}

impl SignalDescriptor {
    /// Render the comment body (without the leading `//`).
    pub fn comment_text(&self) -> String {
        let mut text = format!("signal {}", self.signal_index);
        if let Some(label) = &self.semantic_label {
            text.push_str(" (");
            text.push_str(label);
            text.push(')');
        }
        text.push_str(": ");
        text.push_str(self.leaf_kind.schema());
        if let Some(role) = &self.seed_role {
            text.push_str("; seed role: ");
            text.push_str(role);
        }
        text
    }
}

/// Insert an explanatory `//` comment before the statement at
/// `statement_start`.
///
/// See the module documentation for the exact meaning of `statement_start`
/// and for how the comment text is escaped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalAnnotation {
    /// First byte of the statement to annotate, in the original source.
    pub statement_start: u32,
    /// The comment body, without any `//` prefix.
    pub text: String,
}

impl SignalAnnotation {
    /// Construct an annotation from literal comment text.
    pub fn new(statement_start: u32, text: impl Into<String>) -> Self {
        Self {
            statement_start,
            text: text.into(),
        }
    }

    /// Construct an annotation whose text is rendered from a [`SignalDescriptor`].
    pub fn from_descriptor(statement_start: u32, descriptor: &SignalDescriptor) -> Self {
        Self {
            statement_start,
            text: descriptor.comment_text(),
        }
    }
}

/// Which rewrites to attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramTransformOptions {
    /// Apply binding renames. Defaults to `true`.
    pub rename_bindings: bool,
    /// Insert signal comments. Defaults to `false`; comments are opt-in so a
    /// caller can rename first and annotate separately.
    pub annotate_signals: bool,
}

impl Default for ProgramTransformOptions {
    fn default() -> Self {
        Self {
            rename_bindings: true,
            annotate_signals: false,
        }
    }
}

/// Why an annotation plan was not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationSkipReason {
    /// Comments are disabled by the options.
    Disabled,
    /// `statement_start` is not the start of a parsed statement.
    NotAStatementBoundary,
    /// The comment text was empty after escaping.
    EmptyText,
    /// The same comment already precedes the statement; no duplicate inserted.
    AlreadyPresent,
    /// The rewrite was abandoned because an edit overlapped.
    OverlappingEdit,
}

/// Why an annotation plan was not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationOutcome {
    /// The requested statement start.
    pub statement_start: u32,
    /// The escaped comment text.
    pub text: String,
    /// Whether the comment was inserted.
    pub applied: bool,
    /// The reason it was skipped, when `applied` is `false`.
    pub skip_reason: Option<AnnotationSkipReason>,
}

/// A typed diagnostic the engine emits for a skipped plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticCode {
    /// No binding identifier begins at the requested offset.
    UnresolvedDeclaration,
    /// The replacement name is not a usable identifier.
    InvalidCanonicalName,
    /// The replacement name already names this binding; nothing to do.
    Unchanged,
    /// The replacement name would be captured, or capture another name.
    NameCollision,
    /// A binding site cannot be rewritten exactly.
    UnsupportedBindingSite,
    /// The source has a dynamic scope (`with`/`eval`); all renames skipped.
    DynamicScope,
    /// Two plans target the same declaration.
    DuplicateTarget,
    /// Two plans in one batch would rename different bindings to the same name
    /// where they collide; both are skipped.
    SimultaneousCollision,
    /// The requested annotation offset is not a statement boundary.
    NotAStatementBoundary,
    /// The annotation text was empty after escaping.
    EmptyAnnotation,
    /// The applied edit set had an overlap; the whole rewrite was abandoned.
    OverlapRejected,
    /// The rewritten text failed to reparse; the whole rewrite was abandoned.
    ReparseFailed,
    /// The rewritten text has a semantic early error (for example a
    /// redeclaration) the original did not; the whole rewrite was abandoned.
    SemanticCheckFailed,
}

/// A diagnostic describing a plan the engine declined to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramTransformDiagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub span: Option<SpanRange>,
}

/// The result of [`transform_program`]: the rewritten text plus per-plan
/// applied/skipped evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramTransformResult {
    /// The rewritten JavaScript, or the original when nothing was applied.
    pub transformed_source: String,
    /// One outcome per rename plan, in input order.
    pub renames: Vec<RenameOutcome>,
    /// One outcome per annotation plan, in input order.
    pub annotations: Vec<AnnotationOutcome>,
    /// Diagnostics for skipped plans, in the order they were produced.
    pub diagnostics: Vec<ProgramTransformDiagnostic>,
    /// Exact original-coordinate edits, present only after whole-rewrite validation.
    pub edits: Vec<ProgramSourceEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProgramSourceEdit {
    pub start: u32,
    pub end: u32,
    pub replacement: String,
}

impl ProgramTransformResult {
    /// Number of rename plans applied.
    pub fn applied_renames(&self) -> usize {
        self.renames
            .iter()
            .filter(|outcome| outcome.applied)
            .count()
    }

    /// Number of annotation plans applied.
    pub fn applied_annotations(&self) -> usize {
        self.annotations
            .iter()
            .filter(|outcome| outcome.applied)
            .count()
    }
}

/// A hard failure: the input could not be parsed at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramTransformError {
    /// The source failed to parse; the string is the joined parse diagnostic.
    Parse(String),
}

impl std::fmt::Display for ProgramTransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(message) => write!(f, "program transform parse failed: {message}"),
        }
    }
}

impl std::error::Error for ProgramTransformError {}

/// Apply binding renames and optional signal comments to recovered JavaScript.
///
/// Returns the rewritten source together with one applied/skipped outcome per
/// plan and any diagnostics. A parse failure is the only hard error; every
/// per-plan problem is reported in the result instead of aborting the run.
pub fn transform_program(
    source: &str,
    renames: &[ProgramBindingRename],
    annotations: &[SignalAnnotation],
    options: &ProgramTransformOptions,
) -> Result<ProgramTransformResult, ProgramTransformError> {
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, source).map_err(ProgramTransformError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);

    let dynamic_scope = has_dynamic_scope(&program);
    let mut plan = rename::plan_renames(
        &program,
        &scoping,
        &nodes,
        renames,
        dynamic_scope,
        options.rename_bindings,
    );
    let mut annotation_plan =
        annotate::plan_annotations(source, &program, annotations, options.annotate_signals);

    let mut edits = std::mem::take(&mut plan.edits);
    edits.extend(std::mem::take(&mut annotation_plan.edits));

    let mut diagnostics = std::mem::take(&mut plan.diagnostics);
    diagnostics.extend(std::mem::take(&mut annotation_plan.diagnostics));

    Ok(finish_rewrite(
        source,
        &program,
        edits,
        plan.outcomes,
        annotation_plan.outcomes,
        diagnostics,
    ))
}

/// Apply the planned edits and validate the result, abandoning the whole
/// rewrite (original source, every outcome skipped) when an edit overlaps or
/// [`validate_rewrite`] rejects the output.
fn finish_rewrite(
    source: &str,
    program: &Program,
    edits: Vec<analysis::Edit>,
    mut rename_outcomes: Vec<RenameOutcome>,
    mut annotation_outcomes: Vec<AnnotationOutcome>,
    mut diagnostics: Vec<ProgramTransformDiagnostic>,
) -> ProgramTransformResult {
    if edits.is_empty() {
        return ProgramTransformResult {
            edits: Vec::new(),
            transformed_source: source.to_owned(),
            renames: rename_outcomes,
            annotations: annotation_outcomes,
            diagnostics,
        };
    }

    let source_edits = edits
        .iter()
        .map(|e| ProgramSourceEdit {
            start: e.start,
            end: e.end,
            replacement: e.replacement.clone(),
        })
        .collect();
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        abandon(
            &mut rename_outcomes,
            &mut annotation_outcomes,
            &mut diagnostics,
            DiagnosticCode::OverlapRejected,
            format!(
                "{} edit(s) overlapped; the rewrite was abandoned",
                applied.rejected.len()
            ),
        );
        return ProgramTransformResult {
            edits: Vec::new(),
            transformed_source: source.to_owned(),
            renames: rename_outcomes,
            annotations: annotation_outcomes,
            diagnostics,
        };
    }

    if let Err(rejection) = validate_rewrite(program, &applied.output) {
        let (code, message) = match rejection {
            RewriteRejection::Parse(error) => (
                DiagnosticCode::ReparseFailed,
                format!("rewritten source did not parse: {error}"),
            ),
            RewriteRejection::Semantic(errors) => (
                DiagnosticCode::SemanticCheckFailed,
                format!(
                    "rewritten source has new semantic errors: {}",
                    errors.join("; ")
                ),
            ),
        };
        abandon(
            &mut rename_outcomes,
            &mut annotation_outcomes,
            &mut diagnostics,
            code,
            message,
        );
        return ProgramTransformResult {
            edits: Vec::new(),
            transformed_source: source.to_owned(),
            renames: rename_outcomes,
            annotations: annotation_outcomes,
            diagnostics,
        };
    }

    ProgramTransformResult {
        edits: source_edits,
        transformed_source: applied.output,
        renames: rename_outcomes,
        annotations: annotation_outcomes,
        diagnostics,
    }
}

/// Why a rewritten source was rejected by [`validate_rewrite`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RewriteRejection {
    /// The rewritten text no longer parses.
    Parse(String),
    /// The rewritten text has semantic early errors the original did not.
    Semantic(Vec<String>),
}

/// Check that `rewritten` parses and that Oxc's semantic checker, with
/// syntax-error checks enabled, reports no early error that `original` did not
/// already have.
///
/// [`analysis::analyze`] builds semantics with syntax-error checks off, and a
/// bare reparse accepts `let a; let a;`, so without this a redeclaration the
/// per-plan collision checks missed would be returned as applied. Errors are compared by
/// message with backquoted names blanked, since a rename changes the names a
/// message quotes; a kind of error appearing more often than in the original
/// is new. The original is only analysed when the rewrite has errors.
pub(crate) fn validate_rewrite(
    original: &Program,
    rewritten: &str,
) -> Result<(), RewriteRejection> {
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, rewritten).map_err(RewriteRejection::Parse)?;
    let errors = semantic_errors(&program);
    if errors.is_empty() {
        return Ok(());
    }
    let mut budget: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for error in semantic_errors(original) {
        *budget.entry(error_kind(&error)).or_default() += 1;
    }
    let new: Vec<String> = errors
        .into_iter()
        .filter(|error| match budget.get_mut(&error_kind(error)) {
            Some(count) if *count > 0 => {
                *count -= 1;
                false
            }
            _ => true,
        })
        .collect();
    if new.is_empty() {
        Ok(())
    } else {
        Err(RewriteRejection::Semantic(new))
    }
}

/// The semantic early errors Oxc reports for `program`.
fn semantic_errors(program: &Program) -> Vec<String> {
    SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(program)
        .errors
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// An error message with every backquoted name blanked, so the same error on
/// a renamed binding compares equal.
fn error_kind(message: &str) -> String {
    let mut kind = String::with_capacity(message.len());
    let mut quoted = false;
    for character in message.chars() {
        if character == '`' {
            if !quoted {
                kind.push_str("`_`");
            }
            quoted = !quoted;
        } else if !quoted {
            kind.push(character);
        }
    }
    kind
}

fn abandon(
    renames: &mut [RenameOutcome],
    annotations: &mut [AnnotationOutcome],
    diagnostics: &mut Vec<ProgramTransformDiagnostic>,
    code: DiagnosticCode,
    message: String,
) {
    // An overlap is an edit-set conflict; a reparse or semantic failure is a
    // rejection of the finished rewrite. Each keeps its own skip reason.
    let rename_reason = match code {
        DiagnosticCode::ReparseFailed | DiagnosticCode::SemanticCheckFailed => {
            RenameSkipReason::RewriteRejected
        }
        _ => RenameSkipReason::OverlappingEdit,
    };
    for outcome in renames.iter_mut() {
        if outcome.applied {
            outcome.applied = false;
            outcome.skip_reason = Some(rename_reason.clone());
        }
    }
    for outcome in annotations.iter_mut() {
        if outcome.applied {
            outcome.applied = false;
            outcome.skip_reason = Some(AnnotationSkipReason::OverlappingEdit);
        }
    }
    diagnostics.push(ProgramTransformDiagnostic {
        code,
        message,
        span: None,
    });
}

/// True when the program has a construct that breaks lexical resolution:
/// a `with` statement or an `eval` reference.
pub(crate) fn has_dynamic_scope(program: &Program) -> bool {
    struct Dynamic {
        found: bool,
    }
    impl<'a> Visit<'a> for Dynamic {
        fn visit_with_statement(&mut self, statement: &oxc_ast::ast::WithStatement<'a>) {
            self.found = true;
            walk::walk_with_statement(self, statement);
        }

        fn visit_identifier_reference(
            &mut self,
            identifier: &oxc_ast::ast::IdentifierReference<'a>,
        ) {
            if identifier.name == "eval" {
                self.found = true;
            }
        }
    }
    let mut dynamic = Dynamic { found: false };
    dynamic.visit_program(program);
    dynamic.found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejection(original: &str, rewritten: &str) -> Result<(), RewriteRejection> {
        let allocator = Allocator::default();
        let program = analysis::parse_program(&allocator, original).expect("original parses");
        validate_rewrite(&program, rewritten)
    }

    #[test]
    fn validator_rejects_a_new_lexical_redeclaration() {
        let result = rejection("let foo = 5; let x = 1;", "let foo = 5; let foo = 1;");
        assert!(
            matches!(&result, Err(RewriteRejection::Semantic(errors)) if errors.len() == 1),
            "{result:?}"
        );
    }

    #[test]
    fn validator_rejects_lexical_and_var_forms() {
        for (original, rewritten) in [
            ("function m(a){ let x; }", "function m(a){ let a; }"),
            (
                "function m(){ { let foo; var x; } }",
                "function m(){ { let foo; var foo; } }",
            ),
            (
                "function m(){ function foo(){} let x; }",
                "function m(){ function foo(){} let foo; }",
            ),
            (
                "function m(){ class foo {} var x; }",
                "function m(){ class foo {} var foo; }",
            ),
            (
                "function m(){ try {} catch (e) { let x; } }",
                "function m(){ try {} catch (e) { let e; } }",
            ),
            (
                "const foo = 1; const x = 2;",
                "const foo = 1; const foo = 2;",
            ),
        ] {
            assert!(
                matches!(
                    rejection(original, rewritten),
                    Err(RewriteRejection::Semantic(_))
                ),
                "{rewritten}"
            );
        }
    }

    #[test]
    fn validator_rejects_unparseable_output() {
        assert!(matches!(
            rejection("let x = 1;", "let x = ;"),
            Err(RewriteRejection::Parse(_))
        ));
    }

    #[test]
    fn validator_accepts_valid_output_and_var_merges() {
        assert_eq!(rejection("let x = 1; x;", "let y = 1; y;"), Ok(()));
        // A silent `var` merge is valid syntax: only the collision checks in
        // `rename` can refuse it.
        assert_eq!(rejection("var foo; var x;", "var foo; var foo;"), Ok(()));
    }

    #[test]
    fn validator_allows_errors_the_original_already_had() {
        // The original already redeclares `a`; renaming it keeps that one
        // error, which is not new.
        assert_eq!(
            rejection("let a; let a; let b;", "let c; let c; let b;"),
            Ok(())
        );
        // A second redeclaration is new.
        assert!(matches!(
            rejection("let a; let a; let b; let d;", "let a; let a; let b; let b;"),
            Err(RewriteRejection::Semantic(errors)) if errors.len() == 1
        ));
    }

    /// Drive the post-edit stage with a redeclaring edit that never went
    /// through the per-plan collision checks.
    #[test]
    fn a_rewrite_the_precheck_never_saw_is_abandoned() {
        let source = "function m(){ let foo = 5; let x = 1; return x; } m();";
        let allocator = Allocator::default();
        let program = analysis::parse_program(&allocator, source).expect("parses");
        let x = source.find("x =").expect("x") as u32;
        let use_x = source.find("return x").expect("use") as u32 + 7;
        let edits = vec![
            analysis::Edit::replace(oxc_span::Span::new(x, x + 1), "foo".to_owned()),
            analysis::Edit::replace(oxc_span::Span::new(use_x, use_x + 1), "foo".to_owned()),
        ];
        let outcome = RenameOutcome {
            declaration_start: x,
            original_name: Some("x".into()),
            canonical_name: "foo".into(),
            applied: true,
            skip_reason: None,
            declaration_span: Some((x, x + 1)),
            reference_edits: 1,
            shorthand_edits: 0,
        };
        let result = finish_rewrite(
            source,
            &program,
            edits,
            vec![outcome],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(result.transformed_source, source);
        assert!(!result.renames[0].applied);
        assert_eq!(
            result.renames[0].skip_reason,
            Some(RenameSkipReason::RewriteRejected)
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::SemanticCheckFailed
        );
    }
}
