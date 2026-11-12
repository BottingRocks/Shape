use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    io::Write,
    path::{Path, PathBuf},
};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    naming, native_cleanup, operation_table, prototype_aliases, string_decode,
    variable_declarations, wasm_bindings,
};
use crate::{
    analysis,
    diagnostics::{Diagnostic, Stage, StageStatus},
    roles::VmRole,
    util::fnv1a64_hex,
    vm_patterns::{recognize_nested_dispatch, NestedDispatchPolicy},
};

const PASS_VERSION: &str = "shape-oxc-deob/0.1";

#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineOptions {
    /// Keep the output stable while making it readable with Oxc codegen.
    pub format_output: bool,
}

/// The standardized artifact and the facts proved while producing it.
///
/// In-memory extraction intermediate used by source inspection and the one-time
/// compiler handoff. [`Self::to_json`] / [`Self::from_json`] are retained only
/// as a legacy/debug projection; runtime tracing persists `KernelArtifact`
/// instead and must reject this format. The stage log (`stages`, `diagnostics`)
/// is not saved and loads empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineReport {
    pub output: String,
    pub input_hash: String,
    #[serde(skip)]
    pub stages: Vec<StageReport>,
    #[serde(skip)]
    pub diagnostics: Vec<Diagnostic>,
    /// Stable source-span mapping for every binding renamed by the semantic pass.
    pub binding_mappings: Vec<BindingMapping>,
    /// Stable source-span mapping for every typed receiver property/layout key
    /// the semantic pass renamed (memory slots, context fields, descriptors).
    pub property_mappings: Vec<PropertyMapping>,
    /// Kernel-owned bindings that kept their original spelling, i.e. residue that
    /// the semantic pass could not give a coherent name. Empty when every owned
    /// binding was renamed.
    pub binding_residue: Vec<String>,
    pub extraction: Vec<ExtractionFact>,
    /// Exact operation-table identities proven before code generation. This is
    /// deliberately metadata, not a name-based fallback for arbitrary arrays.
    pub proven_operation_tables: Vec<ProvenOperationTable>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingMapping {
    pub span: (u32, u32),
    pub original_name: String,
    pub canonical_name: String,
    pub role: Option<String>,
}

/// Code-generation-stable identity for a proven operation handler table.
///
/// `canonical_name` is display metadata only. Consumers must use the
/// declaration ordinal and shape checks together; a spelling such as
/// `vmOperations` is never sufficient evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenOperationTable {
    pub declaration_ordinal: u32,
    pub original_name: String,
    pub canonical_name: String,
    pub initializer_elements: u32,
    pub initializer_functions: u32,
}

/// Identity carried from the semantic FIND/RENAME proof into final
/// presentation.  The SymbolId from the pre-rename tree is intentionally not
/// reused after code generation; the presentation pass re-resolves the
/// binding in its fresh AST from code-generation-stable metadata.
///
/// Canonical spelling alone is not identity: an unrelated binding in a
/// non-conflicting scope can share the same name.  `declaration_ordinal`
/// pins the proven `VariableDeclarator` by its document position (the only
/// property that survives renaming and code generation), while the name and
/// initializer fingerprint act as consistency checks.  Any mismatch declines
/// the conversion instead of guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProvenRoleBinding {
    pub role: VmRole,
    pub original_name: String,
    pub canonical_name: String,
    /// Index of the proven binding's `VariableDeclarator` in document order
    /// across the whole program.
    pub declaration_ordinal: u32,
    /// Number of source elements in the proven array initializer, including
    /// elisions.  Code generation preserves element count and structure.
    pub initializer_elements: u32,
    /// Number of function-valued elements in the proven initializer.
    pub initializer_functions: u32,
}

/// A typed receiver property (layout key) renamed on a proven VM object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyMapping {
    pub span: (u32, u32),
    /// Proven owner, e.g. `vmMemory`, `vmContext`, `vmStack`, `threadDescriptor`.
    pub owner: String,
    pub original_name: String,
    pub semantic_name: String,
}

#[derive(Debug, Clone)]
pub struct StageReport {
    pub stage: Stage,
    pub status: StageStatus,
    pub changed_nodes: usize,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StaticValue {
    String(String),
    Number(#[serde(with = "json_number")] f64),
    Boolean(bool),
    Null,
    Array(Vec<StaticValue>),
    Object(BTreeMap<String, StaticValue>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExtractionKind {
    StringTable,
    NumberTable,
    PrototypeTable,
    WasmModule,
    BytecodeImage,
    DispatchTable,
    DispatchOperation,
    ThreadDescriptors,
    InitialMemorySlots,
    StringCache,
}

impl fmt::Display for ExtractionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::StringTable => "StringTable",
            Self::NumberTable => "NumberTable",
            Self::PrototypeTable => "PrototypeTable",
            Self::WasmModule => "WasmModule",
            Self::BytecodeImage => "BytecodeImage",
            Self::DispatchTable => "DispatchTable",
            Self::DispatchOperation => "DispatchOperation",
            Self::ThreadDescriptors => "ThreadDescriptors",
            Self::InitialMemorySlots => "InitialMemorySlots",
            Self::StringCache => "StringCache",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtractionStatus {
    Extracted,
    Unknown,
    Unsupported,
}

impl fmt::Display for ExtractionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Extracted => "extracted",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionFact {
    pub kind: ExtractionKind,
    pub status: ExtractionStatus,
    pub binding: Option<String>,
    #[serde(with = "span_pair")]
    pub span: Span,
    pub length: Option<usize>,
    pub decoded_bytes: Option<Vec<u8>>,
    pub value: Option<StaticValue>,
    pub evidence: String,
}

/// The format tag [`PipelineReport::to_json`] writes, checked on load.
const REPORT_FORMAT: &str = "shape-pipeline-report/1";

#[derive(Serialize)]
struct SavedReport<'a> {
    format: &'a str,
    report: &'a PipelineReport,
}

#[derive(Deserialize)]
struct LoadedReport {
    format: String,
    report: PipelineReport,
}

impl PipelineReport {
    /// Serialize the legacy/debug pipeline report projection.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&SavedReport {
            format: REPORT_FORMAT,
            report: self,
        })
        .expect("a pipeline report always serializes")
    }

    /// Load the legacy/debug pipeline report projection. Runtime artifact
    /// consumers must not use this as a fallback loader.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let loaded: LoadedReport =
            serde_json::from_str(json).map_err(|error| format!("not a saved report: {error}"))?;
        if loaded.format != REPORT_FORMAT {
            return Err(format!(
                "report format {:?} is not {REPORT_FORMAT:?}",
                loaded.format
            ));
        }
        Ok(loaded.report)
    }
}

/// An `f64` as a JSON number, or as `"NaN"`, `"Infinity"` or `"-Infinity"`,
/// which JSON numbers cannot spell.
mod json_number {
    use super::*;

    pub(super) fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        if value.is_finite() {
            serializer.serialize_f64(*value)
        } else if value.is_nan() {
            serializer.serialize_str("NaN")
        } else if *value > 0.0 {
            serializer.serialize_str("Infinity")
        } else {
            serializer.serialize_str("-Infinity")
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match &value {
            serde_json::Value::Number(number) => number.as_f64(),
            serde_json::Value::String(name) => match name.as_str() {
                "NaN" => Some(f64::NAN),
                "Infinity" => Some(f64::INFINITY),
                "-Infinity" => Some(f64::NEG_INFINITY),
                _ => None,
            },
            _ => None,
        }
        .ok_or_else(|| serde::de::Error::custom(format!("{value} is not a number")))
    }
}

/// A [`Span`] as `[start, end]`.
mod span_pair {
    use super::*;

    pub(super) fn serialize<S: Serializer>(span: &Span, serializer: S) -> Result<S::Ok, S::Error> {
        (span.start, span.end).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Span, D::Error> {
        let (start, end) = <(u32, u32)>::deserialize(deserializer)?;
        Ok(Span::new(start, end))
    }
}

#[derive(Debug)]
pub enum PipelineError {
    Parse(String),
    InvalidInput(String),
    Io(io::Error),
    OutputExists(PathBuf),
    OutputIsInput,
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(s) => write!(f, "parse failed: {s}"),
            Self::InvalidInput(s) => write!(f, "invalid input: {s}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::OutputExists(p) => {
                write!(f, "refusing to overwrite existing output {}", p.display())
            }
            Self::OutputIsInput => f.write_str("refusing to overwrite the raw input"),
        }
    }
}

impl std::error::Error for PipelineError {}
impl From<io::Error> for PipelineError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Deobfuscate one JavaScript source string and return only its standardized source.
///
/// Call [`deobfuscate_with_report`] when metadata such as stage diagnostics,
/// extraction facts, or proven operation tables is needed. This wrapper runs
/// the pipeline once and moves the report's output out of that single result.
pub fn deobfuscate(source: &str) -> Result<String, PipelineError> {
    Ok(deobfuscate_with_report(source)?.output)
}

/// Run all four static passes over one JavaScript source string and return the
/// standardized source together with its metadata.
pub fn deobfuscate_with_report(source: &str) -> Result<PipelineReport, PipelineError> {
    deobfuscate_report(source, true)
}

/// The same passes without the WASM ABI presentation adapter: its generated
/// import bindings are presentation, so proofs of which JavaScript function a
/// WASM import is read the module instances' original import objects here.
pub(crate) fn wasm_proof_report(source: &str) -> Result<PipelineReport, PipelineError> {
    deobfuscate_report(source, false)
}

/// Build the report projection for source that is already standardized
/// deobfuscated JavaScript.
///
/// Re-running clean/rename presentation over already-standardized output is
/// both unnecessary and potentially quadratic because every surviving binding
/// participates in collision planning again. FIND and EXTRACT are still run so
/// the artifact receives the same table facts it consumes from a raw compile.
/// The standardizer's canonical `vm*` helper names and `vmContext`/`vmMemory`
/// layout fields are re-stated as declared-only provenance: they supply the
/// same helper roles and context/memory fields that RENAME records for a raw
/// compile, without claiming the original raw source or operation-table
/// identity.
pub(crate) fn standardized_report(source: &str) -> Result<PipelineReport, PipelineError> {
    if source.is_empty() {
        return Err(PipelineError::InvalidInput("source is empty".into()));
    }
    let input_hash = fnv1a64_hex(source.as_bytes());
    let mut diagnostics = Vec::new();
    let (findings, find_report) = find(source, &mut diagnostics, Vec::new())?;
    let (extraction, extract_report) = extract(source, &findings, &mut diagnostics)?;
    let output = if source.ends_with('\n') {
        source.to_owned()
    } else {
        format!("{source}\n")
    };
    let canonical = crate::kernel::properties::extract_actual_properties(&output)
        .map_err(|error| PipelineError::Parse(error.to_string()))?;
    let mut binding_mappings = Vec::new();
    for name in canonical
        .properties
        .keys()
        .filter(|name| name.starts_with("vm"))
    {
        binding_mappings.push(BindingMapping {
            span: (0, 0),
            original_name: name.clone(),
            canonical_name: name.clone(),
            role: Some(name.clone()),
        });
    }
    // The constructor and prototype maps together are exactly the field set
    // the semantic pass records for a raw compile. `slots.context` alone omits
    // prototype-defined reads such as `context.dispatch`, and `slots.memory`
    // alone omits the slot methods.
    let mut property_mappings = Vec::new();
    for (owner, properties, prototype) in [
        (
            "vmContext",
            &canonical.slots.context,
            &canonical.slots.context_prototype,
        ),
        (
            "vmMemory",
            &canonical.slots.memory,
            &canonical.slots.memory_prototype,
        ),
    ] {
        let names = properties
            .keys()
            .chain(prototype.keys())
            .collect::<BTreeSet<_>>();
        for name in names {
            property_mappings.push(PropertyMapping {
                span: (0, 0),
                owner: owner.into(),
                original_name: name.clone(),
                semantic_name: name.clone(),
            });
        }
    }
    Ok(PipelineReport {
        output,
        input_hash,
        stages: vec![find_report, extract_report],
        diagnostics,
        binding_mappings,
        property_mappings,
        binding_residue: Vec::new(),
        extraction,
        proven_operation_tables: Vec::new(),
    })
}

fn deobfuscate_report(source: &str, adapt_wasm: bool) -> Result<PipelineReport, PipelineError> {
    if source.is_empty() {
        return Err(PipelineError::InvalidInput("source is empty".into()));
    }
    let input_hash = fnv1a64_hex(source.as_bytes());
    let mut diagnostics = Vec::new();
    let __profile_total = crate::profile::start();
    let __profile = crate::profile::start();
    let (cleaned, clean_report) = clean(source, &mut diagnostics)?;
    crate::profile::done("pipeline.clean", __profile);
    let __profile = crate::profile::start();
    let (renamed, mut rename_report, renames) = rename(&cleaned, &mut diagnostics)?;
    crate::profile::done("pipeline.rename", __profile);
    let __profile = crate::profile::start();
    let (findings, find_report) = find(&renamed, &mut diagnostics, renames.proven_roles.clone())?;
    crate::profile::done("pipeline.find", __profile);
    let __profile = crate::profile::start();
    let (extraction, mut extract_report) = extract(&renamed, &findings, &mut diagnostics)?;
    crate::profile::done("pipeline.extract", __profile);
    let __profile = crate::profile::start();
    let (presented, operation_tables) =
        operation_table::present(&renamed, &findings.proven_roles, &mut diagnostics)
            .map_err(PipelineError::Parse)?;
    crate::profile::done("pipeline.operation_object", __profile);
    if operation_tables > 0 {
        extract_report.evidence.push(format!(
            "final presentation: converted {operation_tables} proven operation table(s) to numeric-keyed objects"
        ));
    }
    let __profile = crate::profile::start();
    let (presented, wasm_report) = if adapt_wasm {
        wasm_bindings::post_rename(&presented)
            .map_err(|error| PipelineError::Parse(error.to_string()))?
    } else {
        (presented, Default::default())
    };
    crate::profile::done("pipeline.wasm_post_rename", __profile);
    if wasm_report.changed {
        diagnostics.push(Diagnostic::info(
            Stage::Rename,
            "WASM_ABI_ADAPTER",
            wasm_report
                .diagnostics
                .first()
                .cloned()
                .unwrap_or_else(|| "emitted validated WASM ABI adapter".into()),
        ));
        rename_report.changed_nodes += 1;
        rename_report.status = StageStatus::Found;
    } else if wasm_report.declined {
        diagnostics.push(Diagnostic::info(
            Stage::Rename,
            "WASM_ABI_DECLINED",
            wasm_report
                .diagnostics
                .first()
                .cloned()
                .unwrap_or_else(|| "WASM ABI relation could not be proven".into()),
        ));
    }
    let __profile = crate::profile::start();
    let prototype_aliases =
        prototype_aliases::inline(&presented, &renames.binding_mappings, &mut diagnostics)
            .map_err(PipelineError::Parse)?;
    crate::profile::done("pipeline.prototype_aliases", __profile);
    let presented = prototype_aliases.output;
    if prototype_aliases.changed > 0 {
        extract_report.evidence.push(format!(
            "final presentation: inlined {} read-only proven VM prototype alias(es)",
            prototype_aliases.changed
        ));
    }
    let removed_declarators = prototype_aliases.removed_declarator_ordinals;
    let __profile = crate::profile::start();
    let one_per_statement =
        variable_declarations::split(&presented).map_err(PipelineError::Parse)?;
    crate::profile::done("pipeline.variable_declarations", __profile);
    let presented = one_per_statement.output;
    if one_per_statement.changed > 0 {
        extract_report.evidence.push(format!(
            "final presentation: split {} multi-declarator statement(s) into one declaration per line",
            one_per_statement.changed
        ));
    }
    let output = if presented.ends_with('\n') {
        presented
    } else {
        format!("{presented}\n")
    };
    crate::profile::done("pipeline.deobfuscate", __profile_total);
    Ok(PipelineReport {
        output,
        input_hash,
        stages: vec![clean_report, rename_report, find_report, extract_report],
        diagnostics,
        binding_mappings: renames.binding_mappings,
        property_mappings: renames.property_mappings,
        binding_residue: renames.binding_residue,
        extraction,
        proven_operation_tables: renames
            .proven_roles
            .iter()
            .filter(|binding| binding.role == VmRole::Operations)
            .map(|binding| ProvenOperationTable {
                declaration_ordinal: adjust_declaration_ordinal(
                    binding.declaration_ordinal,
                    &removed_declarators,
                ),
                original_name: binding.original_name.clone(),
                canonical_name: binding.canonical_name.clone(),
                initializer_elements: binding.initializer_elements,
                initializer_functions: binding.initializer_functions,
            })
            .collect(),
    })
}

/// Shift a proven operation-table ordinal past declarators removed by the
/// final prototype-alias presentation.  Ordinals are document order, so only
/// removals before the table change its position.
fn adjust_declaration_ordinal(ordinal: u32, removed: &[u32]) -> u32 {
    let before = removed.iter().filter(|removed| **removed < ordinal).count() as u32;
    ordinal.saturating_sub(before)
}

/// Write the default `<input>.deobfuscated.js` sibling atomically.
pub fn write_deobfuscated_file(
    input: impl AsRef<Path>,
) -> Result<(PathBuf, PipelineReport), PipelineError> {
    let input = input.as_ref();
    let output = default_output_path(input)?;
    write_deobfuscated_file_to(input, output)
}

/// Write a deobfuscated JavaScript file atomically without overwriting either
/// an existing output or the input file.
pub fn write_deobfuscated_file_to(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(PathBuf, PipelineReport), PipelineError> {
    write_deobfuscated_file_to_mode(input.as_ref(), output.as_ref(), false)
}

/// Write a deobfuscated JavaScript file atomically, replacing an existing
/// output when `shape deob --force` requested it. The input itself is never
/// replaced, including through an equivalent path or hard link.
pub fn write_deobfuscated_file_to_force(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(PathBuf, PipelineReport), PipelineError> {
    write_deobfuscated_file_to_mode(input.as_ref(), output.as_ref(), true)
}

/// Write a deobfuscated source file without imposing a filename suffix on the
/// input. The unified CLI uses explicit source modes, so a path's spelling is
/// never used to infer whether it is raw JavaScript.
pub fn write_deobfuscated_source_to(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(PathBuf, PipelineReport), PipelineError> {
    write_deobfuscated_source_to_mode(input.as_ref(), output.as_ref(), false)
}

/// Force-write a deobfuscated source file without imposing a filename suffix.
pub fn write_deobfuscated_source_to_force(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(PathBuf, PipelineReport), PipelineError> {
    write_deobfuscated_source_to_mode(input.as_ref(), output.as_ref(), true)
}

fn write_deobfuscated_file_to_mode(
    input: &Path,
    output: &Path,
    replace: bool,
) -> Result<(PathBuf, PipelineReport), PipelineError> {
    validate_javascript_input(input)?;
    write_deobfuscated_source_to_mode(input, output, replace)
}

fn write_deobfuscated_source_to_mode(
    input: &Path,
    output: &Path,
    replace: bool,
) -> Result<(PathBuf, PipelineReport), PipelineError> {
    if paths_refer_to_same_file(input, output) {
        return Err(PipelineError::OutputIsInput);
    }
    if !replace && path_exists(output) {
        return Err(PipelineError::OutputExists(output.to_owned()));
    }

    let raw = fs::read_to_string(input)?;
    let report = deobfuscate_with_report(&raw)?;
    let temporary = temporary_output_path(output);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    if let Err(error) = file.write_all(report.output.as_bytes()) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    drop(file);

    let result = if replace {
        // The temporary file is complete and on the same filesystem. Rename is
        // the atomic replacement step for `--force`; a failed rename leaves the
        // destination untouched and removes the temporary file below.
        fs::rename(&temporary, output)
    } else {
        // Linking the complete temporary file into place is an atomic, no-replace
        // publication on the same filesystem and cannot overwrite a raced output.
        fs::hard_link(&temporary, output)
    };
    let _ = fs::remove_file(&temporary);
    if let Err(error) = result {
        if !replace && error.kind() == io::ErrorKind::AlreadyExists {
            return Err(PipelineError::OutputExists(output.to_owned()));
        }
        return Err(error.into());
    }
    Ok((output.to_owned(), report))
}

fn validate_javascript_input(input: &Path) -> Result<(), PipelineError> {
    let name = input
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| PipelineError::InvalidInput("input has no UTF-8 filename".into()))?;
    if !name.ends_with(".js") {
        return Err(PipelineError::InvalidInput("input must end in .js".into()));
    }
    Ok(())
}

fn default_output_path(input: &Path) -> Result<PathBuf, PipelineError> {
    validate_javascript_input(input)?;
    let name = input
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| PipelineError::InvalidInput("input has no UTF-8 filename".into()))?;
    let stem = name
        .strip_suffix(".raw.js")
        .or_else(|| name.strip_suffix(".js"))
        .expect("validated JavaScript filename");
    Ok(input.with_file_name(format!("{stem}.deobfuscated.js")))
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn paths_refer_to_same_file(input: &Path, output: &Path) -> bool {
    if input == output {
        return true;
    }

    if let (Ok(input_metadata), Ok(output_metadata)) = (fs::metadata(input), fs::metadata(output)) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if input_metadata.dev() == output_metadata.dev()
                && input_metadata.ino() == output_metadata.ino()
            {
                return true;
            }
        }
    }

    let Ok(input_canonical) = fs::canonicalize(input) else {
        return false;
    };
    if let Ok(output_canonical) = fs::canonicalize(output) {
        return input_canonical == output_canonical;
    }

    // The output may not exist yet. Canonicalize only its existing parent so
    // `input` and `dir/../input` still compare equal without inventing a
    // canonical path for a nonexistent output.
    let Some(file_name) = output.file_name() else {
        return false;
    };
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::canonicalize(parent)
        .map(|parent| parent.join(file_name) == input_canonical)
        .unwrap_or(false)
}

fn temporary_output_path(output: &Path) -> PathBuf {
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output.js");
    output.with_file_name(format!(".{name}.shape.tmp.{}", std::process::id()))
}

fn clean(
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(String, StageReport), PipelineError> {
    let __profile = crate::profile::start();
    let native = native_cleanup::run_readable(source, diagnostics).map_err(PipelineError::Parse)?;
    crate::profile::done("clean.native", __profile);
    let counts = &native.report;
    // The string-decoder collapse runs on the native-cleaned text. Both are
    // structural CLEAN rewrites and must precede the single fresh RENAME
    // semantic analysis, so FIND/EXTRACT operate on renamed semantic identity
    // rather than stale bindings left over from either rewrite.
    let __profile = crate::profile::start();
    let decode = string_decode::run(&native.output).map_err(PipelineError::Parse)?;
    crate::profile::done("clean.string_decode", __profile);
    if decode.report.changed > 0 {
        diagnostics.push(Diagnostic::info(
            Stage::Clean,
            "STRING_DECODER_HELPER",
            format!(
                "collapsed {} string-decoder region(s) ({} single, {} paired handler(s)) into `{}`",
                decode.report.matched + decode.report.paired * 2,
                decode.report.matched,
                decode.report.paired,
                decode.report.helper.as_deref().unwrap_or("vmDecodeString")
            ),
        ));
    }
    for reason in &decode.report.skipped {
        diagnostics.push(Diagnostic::info(
            Stage::Clean,
            "STRING_DECODER_SKIPPED",
            reason.clone(),
        ));
    }
    let output = if decode.report.changed > 0 {
        decode.output
    } else {
        native.output
    };
    let changed_nodes = native.changed_nodes + decode.report.changed;
    let status = if changed_nodes > 0 {
        StageStatus::Found
    } else {
        StageStatus::Unknown
    };
    Ok((
        output,
        StageReport {
            stage: Stage::Clean,
            status,
            changed_nodes,
            evidence: vec![
                "directive-safe empty-statement cleanup".into(),
                format!("direct native aliases: {}", counts.direct_native_aliases),
                format!("native member aliases: {}", counts.member_aliases),
                format!("OO indexed references: {}", counts.object_indexed_refs),
                format!("uncurry normalized: {}", counts.uncurry_normalized),
                format!("native cleanup skips: {}", counts.skipped),
                format!("string decoder regions: {}", decode.report.matched),
                format!("paired decoder handlers: {}", decode.report.paired),
            ],
        },
    ))
}

/// Everything RENAME produced besides the rewritten source.
#[derive(Debug, Default)]
pub struct RenameArtifacts {
    pub binding_mappings: Vec<BindingMapping>,
    pub property_mappings: Vec<PropertyMapping>,
    pub binding_residue: Vec<String>,
    pub proven_roles: Vec<ProvenRoleBinding>,
}

fn rename(
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(String, StageReport, RenameArtifacts), PipelineError> {
    let allocator = Allocator::default();
    let __profile = crate::profile::start();
    let program = analysis::parse_program(&allocator, source).map_err(PipelineError::Parse)?;
    crate::profile::done("rename.parse", __profile);
    let mut program = program;
    let dynamic = has_dynamic_scope(&program);
    if dynamic {
        diagnostics.push(Diagnostic::warn(
            Stage::Rename,
            "UNSUPPORTED_DYNAMIC_SCOPE",
            "with/direct eval prevents binding renames whose lexical effects cannot be proven",
        ));
    }
    let (changed_nodes, artifacts) = if dynamic || entry_function(&program).is_none() {
        (0, RenameArtifacts::default())
    } else {
        let __profile = crate::profile::start();
        let (scoping, nodes) = analysis::analyze(&program);
        crate::profile::done("rename.analyze", __profile);
        let __profile = crate::profile::start();
        let plan = naming::plan(&program, &scoping, &nodes);
        crate::profile::done("rename.plan", __profile);
        drop(nodes);
        let __profile = crate::profile::start();
        let changed = naming::apply(&mut program, &scoping, &allocator, &plan);
        crate::profile::done("rename.apply", __profile);
        let artifacts = RenameArtifacts {
            binding_mappings: plan.binding_mappings().to_vec(),
            property_mappings: plan.property_mappings().to_vec(),
            binding_residue: plan.binding_residue().to_vec(),
            proven_roles: plan.proven_roles().to_vec(),
        };
        diagnostics.extend(plan.report().iter().cloned());
        (changed, artifacts)
    };
    if changed_nodes == 0 {
        diagnostics.push(Diagnostic::warn(
            Stage::Rename,
            "NO_PROVEN_ROLES",
            "no binding-safe kernel binding was renamed",
        ));
    }
    let output = {
        let __profile = crate::profile::start();
        let output = analysis::codegen(&program);
        crate::profile::done("rename.codegen", __profile);
        output
    };
    Ok((
        output,
        StageReport {
            stage: Stage::Rename,
            status: if changed_nodes == 0 {
                StageStatus::Unknown
            } else {
                StageStatus::Found
            },
            changed_nodes,
            evidence: vec![
                "Oxc SymbolId/reference-id binding renames".into(),
                "receiver-typed layout key renames".into(),
                PASS_VERSION.into(),
            ],
        },
        artifacts,
    ))
}

#[derive(Clone)]
struct Mapping {
    symbol: SymbolId,
    role: VmRole,
    old_name: String,
    skip: bool,
}

struct Detector<'a> {
    scoping: &'a Scoping,
    entry_span: Option<Span>,
    entry_symbol: Option<(SymbolId, String)>,
    vars: Vec<VarFact>,
    wasm_inputs: BTreeSet<SymbolId>,
    decoder_symbols: BTreeSet<SymbolId>,
    operation_symbols: BTreeSet<SymbolId>,
    proven_operation_symbols: BTreeSet<SymbolId>,
    dynamic_operation_symbols: BTreeSet<SymbolId>,
    dispatch_symbols: BTreeSet<SymbolId>,
    record_aliases: BTreeMap<SymbolId, SymbolId>,
    config_fields: BTreeMap<SymbolId, BTreeSet<String>>,
    config_call_symbols: BTreeSet<SymbolId>,
    mappings: Vec<Mapping>,
}
#[derive(Clone)]
struct VarFact {
    symbol: Option<SymbolId>,
    name: String,
    shape: InitShape,
}

#[derive(Clone)]
enum InitShape {
    Strings(usize),
    Numbers(usize),
    Functions(usize),
    FunctionSequences(usize),
    Records(usize),
    Cache,
    Base64(Option<String>),
    Other,
}

impl<'a> Detector<'a> {
    fn new(scoping: &'a Scoping, program: &Program<'a>) -> Self {
        let entry = entry_function(program);
        let entry_span = entry.map(|(_, function)| function.span);
        let entry_symbol = entry.and_then(|(_, function)| {
            function.id.as_ref().and_then(|id| {
                id.symbol_id
                    .get()
                    .map(|symbol| (symbol, id.name.to_string()))
            })
        });
        let mut decoder_collector = DecoderCollector::default();
        decoder_collector.visit_program(program);
        Self {
            scoping,
            entry_span,
            entry_symbol,
            vars: Vec::new(),
            wasm_inputs: BTreeSet::new(),
            decoder_symbols: decoder_collector.symbols,
            operation_symbols: BTreeSet::new(),
            proven_operation_symbols: BTreeSet::new(),
            dynamic_operation_symbols: BTreeSet::new(),
            dispatch_symbols: BTreeSet::new(),
            record_aliases: BTreeMap::new(),
            config_fields: BTreeMap::new(),
            config_call_symbols: BTreeSet::new(),
            mappings: Vec::new(),
        }
    }
    fn classify(&mut self) {
        for var in &self.vars {
            let Some(symbol) = var.symbol else {
                continue;
            };
            let role = match &var.shape {
                InitShape::Base64(_) if self.wasm_inputs.contains(&symbol) => Some((
                    VmRole::WasmModule,
                    "binding is consumed by WebAssembly.Module; VM bytecode relationship is unproven",
                )),
                InitShape::Strings(n) if *n > 32 => Some((VmRole::StringTable, "large literal array of strings")),
                InitShape::Numbers(n) if *n >= 4 && !self.wasm_inputs.contains(&symbol) => {
                    Some((VmRole::NumberTable, "literal numeric table"))
                }
                InitShape::Functions(n)
                    if *n >= 4
                        && self.operation_symbols.contains(&symbol)
                        && self.proven_operation_symbols.contains(&symbol)
                        && self.dynamic_operation_symbols.contains(&symbol) => {
                    Some((VmRole::Operations, "indexed function values are invoked as operations"))
                }
                InitShape::FunctionSequences(n)
                    if *n >= 1 && self.dispatch_symbols.contains(&symbol) => {
                    Some((VmRole::DispatchTable, "row and byte indexing select an operation"))
                }
                InitShape::Records(n)
                    if *n >= 1 && self.dispatch_symbols.contains(&symbol) => {
                    Some((VmRole::DispatchTable, "row selection and byte selection resolve an operation"))
                }
                InitShape::Records(n)
                    if *n >= 1
                        && self
                            .config_fields
                            .get(&symbol)
                            .is_some_and(|fields| fields.len() >= 3)
                        && self.config_call_symbols.contains(&symbol) =>
                {
                    Some((VmRole::ThreadDescriptors, "indexed records provide thread descriptor fields"))
                }
                InitShape::Cache => Some((VmRole::StringCache, "Object.create(null) cache with no executable evaluation")),
                _ => None,
            };
            if let Some((role, _evidence)) = role {
                self.mappings.push(Mapping {
                    symbol,
                    role,
                    old_name: var.name.clone(),
                    skip: false,
                });
            }
        }
        if let Some((symbol, old_name)) = &self.entry_symbol {
            self.mappings.push(Mapping {
                symbol: *symbol,
                role: VmRole::Entry,
                old_name: old_name.clone(),
                skip: false,
            });
        }
    }
}

impl<'a> Visit<'a> for Detector<'a> {
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if self
            .entry_span
            .is_none_or(|span| it.span.start >= span.start && it.span.end <= span.end)
        {
            if let Some(binding) = simple_binding(&it.id) {
                if let (Some(symbol), Some(init)) = (binding.symbol_id.get(), it.init.as_ref()) {
                    if let Some(source_symbol) = computed_member_base_symbol(init, self.scoping) {
                        self.record_aliases.insert(symbol, source_symbol);
                    }
                }
                let shape = it.init.as_ref().map_or(InitShape::Other, |init| {
                    if let Some(n) = array_all_strings(init) {
                        InitShape::Strings(n)
                    } else if let Some(n) = array_all_numbers(init) {
                        InitShape::Numbers(n)
                    } else if let Some(n) = function_table_length(init) {
                        InitShape::Functions(n)
                    } else if let Some(n) = array_of_function_arrays(init) {
                        InitShape::FunctionSequences(n)
                    } else if let Some(n) = array_of_numeric_records(init) {
                        InitShape::Records(n)
                    } else if is_null_object_cache(init, self.scoping) {
                        InitShape::Cache
                    } else if is_base64_call(init, self.scoping, &self.decoder_symbols) {
                        InitShape::Base64(base64_call_string(init))
                    } else {
                        InitShape::Other
                    }
                });
                if let Some(symbol) = binding.symbol_id.get() {
                    if it.init.as_ref().is_some_and(proven_operation_array) {
                        self.proven_operation_symbols.insert(symbol);
                    }
                }
                self.vars.push(VarFact {
                    symbol: binding.symbol_id.get(),
                    name: binding.name.to_string(),
                    shape: shape.clone(),
                });
                if let Some(Expression::NewExpression(new)) = it.init.as_ref() {
                    if is_webassembly_module(&new.callee, self.scoping) {
                        if let Some(Argument::Identifier(id)) = new.arguments.first() {
                            if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                                self.wasm_inputs.insert(symbol);
                            }
                        }
                    }
                }
            }
        }
        walk::walk_variable_declarator(self, it);
    }
    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        if is_webassembly_module(&it.callee, self.scoping) {
            if let Some(Argument::Identifier(id)) = it.arguments.first() {
                if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                    self.wasm_inputs.insert(symbol);
                }
            }
        }
        walk::walk_new_expression(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Some(symbol) = computed_member_base_symbol(&it.callee, self.scoping) {
            self.operation_symbols.insert(symbol);
            if let Expression::ComputedMemberExpression(member) = &it.callee {
                if !matches!(
                    &member.expression,
                    Expression::NumericLiteral(_) | Expression::StringLiteral(_)
                ) {
                    self.dynamic_operation_symbols.insert(symbol);
                }
            }
        }
        if let Some(symbol) = nested_dispatch_base_symbol(&it.callee, self.scoping) {
            self.dispatch_symbols.insert(symbol);
        }
        for argument in &it.arguments {
            if let Some(expression) = argument.as_expression() {
                if let Some(symbol) =
                    config_member_source(expression, self.scoping, &self.record_aliases)
                {
                    self.config_call_symbols.insert(symbol);
                }
            }
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        if let Expression::Identifier(identifier) = &it.object {
            if let Some(alias_symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                if let Some(source_symbol) = self.record_aliases.get(&alias_symbol) {
                    self.config_fields
                        .entry(*source_symbol)
                        .or_default()
                        .insert(it.property.name.to_string());
                }
            }
        }
        walk::walk_static_member_expression(self, it);
    }
}

fn find(
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    proven_roles: Vec<ProvenRoleBinding>,
) -> Result<(Findings, StageReport), PipelineError> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).map_err(PipelineError::Parse)?;
    let __profile = crate::profile::start();
    let (scoping, _) = analysis::analyze(&program);
    crate::profile::done("find.analyze", __profile);
    let __profile = crate::profile::start();
    let mut detector = Detector::new(&scoping, &program);
    detector.visit_program(&program);
    detector.classify();
    crate::profile::done("find.detect", __profile);
    let dynamic = has_dynamic_scope(&program);
    if dynamic {
        diagnostics.push(Diagnostic::warn(
            Stage::Find,
            "UNSUPPORTED_DYNAMIC_SCOPE",
            "with/direct eval prevents complete kernel proof",
        ));
    }
    let mut roles = BTreeMap::new();
    for mapping in &detector.mappings {
        if !mapping.skip {
            roles.insert(mapping.role, mapping.old_name.clone());
        }
    }
    for binding in &proven_roles {
        if binding.role == VmRole::Operations {
            roles.insert(binding.role, binding.canonical_name.clone());
        }
    }
    if !roles.contains_key(&VmRole::Entry) {
        diagnostics.push(Diagnostic::warn(
            Stage::Find,
            "ENTRY_UNKNOWN",
            "top-level entry binding is anonymous or ambiguous",
        ));
    }
    let status = if roles.is_empty() {
        StageStatus::Unknown
    } else if dynamic {
        StageStatus::Unsupported
    } else {
        StageStatus::Found
    };
    let findings = Findings {
        roles,
        dynamic,
        entry_call: entry_function(&program).is_some(),
        proven_roles,
    };
    Ok((
        findings,
        StageReport {
            stage: Stage::Find,
            status,
            changed_nodes: 0,
            evidence: vec![
                "AST/dataflow table shape recognition".into(),
                "role evidence recorded from initializer and consumers".into(),
            ],
        },
    ))
}

#[derive(Default)]
struct Findings {
    roles: BTreeMap<VmRole, String>,
    dynamic: bool,
    entry_call: bool,
    proven_roles: Vec<ProvenRoleBinding>,
}

fn extract(
    source: &str,
    findings: &Findings,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(Vec<ExtractionFact>, StageReport), PipelineError> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).map_err(PipelineError::Parse)?;
    let (scoping, _) = analysis::analyze(&program);
    let __profile = crate::profile::start();
    let mut role_detector = Detector::new(&scoping, &program);
    role_detector.visit_program(&program);
    role_detector.classify();
    crate::profile::done("extract.detect", __profile);
    let proven_roles: BTreeMap<SymbolId, VmRole> = role_detector
        .mappings
        .iter()
        .filter(|mapping| !mapping.skip)
        .map(|mapping| (mapping.symbol, mapping.role))
        .collect();
    let mut extractor = Extractor {
        scoping: &scoping,
        decoder_symbols: role_detector.decoder_symbols,
        proven_roles,
        wasm_inputs: BTreeSet::new(),
        wasm_consumers: Vec::new(),
        facts: Vec::new(),
        variables: BTreeMap::new(),
    };
    let __profile = crate::profile::start();
    extractor.visit_program(&program);
    extractor.finish(&program, findings);
    crate::profile::done("extract.run", __profile);
    let extracted = extractor
        .facts
        .iter()
        .filter(|fact| fact.status == ExtractionStatus::Extracted)
        .count();
    if extractor.facts.is_empty() {
        diagnostics.push(Diagnostic::warn(
            Stage::Extract,
            "NO_STATIC_FACTS",
            "no supported literal kernel tables were proven",
        ));
    }
    if findings.dynamic {
        diagnostics.push(Diagnostic::warn(
            Stage::Extract,
            "DYNAMIC_EXTRACTION",
            "dynamic scope means extraction is partial",
        ));
    }
    if extractor
        .facts
        .iter()
        .any(|fact| fact.kind == ExtractionKind::WasmModule)
    {
        diagnostics.push(Diagnostic::warn(
            Stage::Extract,
            "UNSUPPORTED_VM_INTERNALS",
            "embedded WebAssembly module bytes are distinct from VM bytecode; VM internals remain unsupported without a proven dispatch relationship",
        ));
    }
    let status = if findings.dynamic {
        StageStatus::Unsupported
    } else if extracted > 0 {
        StageStatus::Found
    } else {
        StageStatus::Unknown
    };
    Ok((
        extractor.facts,
        StageReport {
            stage: Stage::Extract,
            status,
            changed_nodes: 0,
            evidence: vec![
                format!(
                    "{extracted} typed static facts from {} proven roles",
                    findings.roles.len()
                ),
                "no input evaluation; bounded literals/base64 only".into(),
            ],
        },
    ))
}

struct Extractor<'a> {
    scoping: &'a Scoping,
    decoder_symbols: BTreeSet<SymbolId>,
    proven_roles: BTreeMap<SymbolId, VmRole>,
    wasm_inputs: BTreeSet<SymbolId>,
    wasm_consumers: Vec<Span>,
    facts: Vec<ExtractionFact>,
    variables: BTreeMap<SymbolId, (String, Span, InitShape)>,
}
impl<'a> Visit<'a> for Extractor<'a> {
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let Some(binding) = simple_binding(&it.id) {
            if let Some(symbol) = binding.symbol_id.get() {
                if let Some(init) = it.init.as_ref() {
                    let shape = if let Some(n) = array_all_strings(init) {
                        InitShape::Strings(n)
                    } else if let Some(n) = array_all_numbers(init) {
                        InitShape::Numbers(n)
                    } else if let Some(n) = function_table_length(init) {
                        InitShape::Functions(n)
                    } else if let Some(n) = array_of_function_arrays(init) {
                        InitShape::FunctionSequences(n)
                    } else if let Some(n) = array_of_numeric_records(init) {
                        InitShape::Records(n)
                    } else if is_null_object_cache(init, self.scoping) {
                        InitShape::Cache
                    } else if is_base64_call(init, self.scoping, &self.decoder_symbols) {
                        InitShape::Base64(base64_call_string(init))
                    } else {
                        InitShape::Other
                    };
                    self.variables
                        .insert(symbol, (binding.name.to_string(), it.span, shape));
                }
            }
        }
        walk::walk_variable_declarator(self, it);
    }
    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        if is_webassembly_module(&it.callee, self.scoping) {
            if let Some(Argument::Identifier(id)) = it.arguments.first() {
                if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                    self.wasm_inputs.insert(symbol);
                }
            } else {
                self.wasm_consumers.push(it.span);
            }
        }
        walk::walk_new_expression(self, it);
    }
}
impl<'a> Extractor<'a> {
    fn finish(&mut self, program: &Program<'a>, findings: &Findings) {
        for (symbol, (name, span, shape)) in &self.variables {
            if self.wasm_inputs.contains(symbol) {
                match shape {
                    InitShape::Base64(Some(encoded)) => match decode_base64(encoded) {
                        Ok(bytes) if is_wasm_module_bytes(&bytes) => self.facts.push(ExtractionFact {
                            kind: ExtractionKind::WasmModule,
                            status: ExtractionStatus::Extracted,
                            binding: Some(name.clone()),
                            span: *span,
                            length: Some(bytes.len()),
                            decoded_bytes: Some(bytes),
                            value: None,
                            evidence: "WebAssembly.Module consumer + proven literal decoder + wasm magic"
                                .into(),
                        }),
                        Ok(bytes) => self.facts.push(ExtractionFact {
                            kind: ExtractionKind::WasmModule,
                            status: ExtractionStatus::Unsupported,
                            binding: Some(name.clone()),
                            span: *span,
                            length: Some(bytes.len()),
                            decoded_bytes: None,
                            value: None,
                            evidence: "decoded bytes are not a WebAssembly module; VM bytecode role is unproven".into(),
                        }),
                        Err(reason) => self.facts.push(ExtractionFact {
                            kind: ExtractionKind::WasmModule,
                            status: ExtractionStatus::Unknown,
                            binding: Some(name.clone()),
                            span: *span,
                            length: None,
                            decoded_bytes: None,
                            value: None,
                            evidence: reason,
                        }),
                    },
                    _ => self.facts.push(ExtractionFact {
                        kind: ExtractionKind::WasmModule,
                        status: ExtractionStatus::Unknown,
                        binding: Some(name.clone()),
                        span: *span,
                        length: None,
                        decoded_bytes: None,
                        value: None,
                        evidence: "WebAssembly.Module input is not a proven static base64 decoder call".into(),
                    }),
                }
            }
            match shape {
                InitShape::Strings(array) if *array > 32 => self.facts.push(table_fact(
                    ExtractionKind::StringTable,
                    name,
                    *span,
                    *array,
                    "all elements are string literals",
                )),
                InitShape::Numbers(array) if *array >= 4 && !self.wasm_inputs.contains(symbol) => {
                    self.facts.push(table_fact(
                        ExtractionKind::NumberTable,
                        name,
                        *span,
                        *array,
                        "all elements are bounded numeric literals",
                    ))
                }
                InitShape::Functions(array)
                    if *array >= 4
                        && self.proven_roles.get(symbol) == Some(&VmRole::Operations) =>
                {
                    self.facts.push(table_fact(
                        ExtractionKind::DispatchOperation,
                        name,
                        *span,
                        *array,
                        "indexed function values are invoked as operations",
                    ));
                }
                InitShape::FunctionSequences(array)
                    if self.proven_roles.get(symbol) == Some(&VmRole::DispatchTable) =>
                {
                    self.facts.push(table_fact(
                        ExtractionKind::DispatchTable,
                        name,
                        *span,
                        *array,
                        "opsSequence[row][heap[byte++]] selects an operation",
                    ));
                }
                InitShape::Records(array)
                    if self.proven_roles.get(symbol) == Some(&VmRole::DispatchTable) =>
                {
                    self.facts.push(table_fact(
                        ExtractionKind::DispatchTable,
                        name,
                        *span,
                        *array,
                        "row and byte indexing select an operation; record shape is not used as proof",
                    ));
                }
                InitShape::Records(array)
                    if self.proven_roles.get(symbol) == Some(&VmRole::ThreadDescriptors) =>
                {
                    self.facts.push(table_fact(
                        ExtractionKind::ThreadDescriptors,
                        name,
                        *span,
                        *array,
                        "indexed records provide thread descriptor fields",
                    ));
                }
                InitShape::Cache => self.facts.push(ExtractionFact {
                    kind: ExtractionKind::StringCache,
                    status: ExtractionStatus::Extracted,
                    binding: Some(name.clone()),
                    span: *span,
                    length: None,
                    decoded_bytes: None,
                    value: Some(StaticValue::Object(BTreeMap::new())),
                    evidence: "Object.create(null) cache initializer".into(),
                }),
                _ => {}
            }
        }
        for span in &self.wasm_consumers {
            self.facts.push(ExtractionFact {
                kind: ExtractionKind::WasmModule,
                status: ExtractionStatus::Unknown,
                binding: None,
                span: *span,
                length: None,
                decoded_bytes: None,
                value: None,
                evidence: "WebAssembly.Module argument is not a statically tracked binding".into(),
            });
        }
        if let Some(call) = entry_call(program) {
            if let Some(argument) = call.arguments.get(3).and_then(Argument::as_expression) {
                if let Some(value) = static_value(argument) {
                    self.facts.push(ExtractionFact {
                        kind: ExtractionKind::InitialMemorySlots,
                        status: ExtractionStatus::Extracted,
                        binding: None,
                        span: argument.span(),
                        length: None,
                        decoded_bytes: None,
                        value: Some(value),
                        evidence:
                            "fourth argument of the proven entry call is initial memory-slot keys"
                                .into(),
                    });
                } else {
                    self.facts.push(ExtractionFact {
                        kind: ExtractionKind::InitialMemorySlots,
                        status: ExtractionStatus::Unknown,
                        binding: None,
                        span: argument.span(),
                        length: None,
                        decoded_bytes: None,
                        value: None,
                        evidence:
                            "proven entry call has an unsupported initial memory-slot expression"
                                .into(),
                    });
                }
            }
        }
        if !findings.entry_call {
            self.facts.push(ExtractionFact {
                kind: ExtractionKind::InitialMemorySlots,
                status: ExtractionStatus::Unknown,
                binding: None,
                span: Span::new(0, 0),
                length: None,
                decoded_bytes: None,
                value: None,
                evidence: "entry call was not proven".into(),
            });
        }
    }
}

fn table_fact(
    kind: ExtractionKind,
    name: &str,
    span: Span,
    length: usize,
    evidence: &str,
) -> ExtractionFact {
    ExtractionFact {
        kind,
        status: ExtractionStatus::Extracted,
        binding: Some(name.into()),
        span,
        length: Some(length),
        decoded_bytes: None,
        value: None,
        evidence: evidence.into(),
    }
}

fn simple_binding<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a BindingIdentifier<'a>> {
    match pattern {
        BindingPattern::BindingIdentifier(id) => Some(id),
        _ => None,
    }
}
#[derive(Default)]
struct DecoderCollector {
    symbols: BTreeSet<SymbolId>,
}

#[derive(Default)]
struct Base64ShapeProbe {
    alphabet: bool,
    loop_count: usize,
}

impl<'a> Visit<'a> for Base64ShapeProbe {
    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        if it.value == crate::runtime_decoder::STANDARD_ALPHABET
            || it.value == crate::runtime_decoder::URL_SAFE_ALPHABET
        {
            self.alphabet = true;
        }
    }

    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        self.loop_count += 1;
        walk::walk_for_statement(self, it);
    }
}

impl<'a> Visit<'a> for DecoderCollector {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        let mut probe = Base64ShapeProbe::default();
        probe.visit_function(it, flags);
        if probe.alphabet && probe.loop_count > 0 {
            if let Some(symbol) = it.id.as_ref().and_then(|id| id.symbol_id.get()) {
                self.symbols.insert(symbol);
            }
        }
        walk::walk_function(self, it, flags);
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let (Some(binding), Some(init)) = (simple_binding(&it.id), it.init.as_ref()) {
            let function = match init {
                Expression::FunctionExpression(function) => Some(&**function),
                Expression::ArrowFunctionExpression(function) => {
                    let mut probe = Base64ShapeProbe::default();
                    probe.visit_arrow_function_expression(function);
                    if probe.alphabet && probe.loop_count > 0 {
                        binding.symbol_id.get().into_iter().for_each(|symbol| {
                            self.symbols.insert(symbol);
                        });
                    }
                    None
                }
                _ => None,
            };
            if let Some(function) = function {
                let mut probe = Base64ShapeProbe::default();
                probe.visit_function(function, ScopeFlags::empty());
                if probe.alphabet && probe.loop_count > 0 {
                    binding.symbol_id.get().into_iter().for_each(|symbol| {
                        self.symbols.insert(symbol);
                    });
                }
            }
        }
        walk::walk_variable_declarator(self, it);
    }
}

pub(crate) fn entry_call<'a>(program: &'a Program<'a>) -> Option<&'a CallExpression<'a>> {
    program.body.iter().find_map(|statement| match statement {
        Statement::ExpressionStatement(es) => match &es.expression {
            Expression::CallExpression(call)
                if matches!(
                    call.callee,
                    Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
                ) && is_proven_entry_call(call) =>
            {
                Some(&**call)
            }
            _ => None,
        },
        _ => None,
    })
}

fn is_proven_entry_call(call: &CallExpression<'_>) -> bool {
    if call.arguments.len() < 4 {
        return false;
    }
    let Some(host) = call.arguments.first().and_then(Argument::as_expression) else {
        return false;
    };
    let Some(thread_config) = call.arguments.get(3).and_then(Argument::as_expression) else {
        return false;
    };
    let mut probe = HostArgumentProbe::default();
    probe.visit_expression(host);
    probe.has_host
        && probe.has_typeof_host
        && array_all_numbers_allow_empty(thread_config).is_some()
}

#[derive(Default)]
struct HostArgumentProbe {
    has_host: bool,
    has_typeof_host: bool,
}

impl<'a> Visit<'a> for HostArgumentProbe {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        if matches!(it.name.as_str(), "window" | "global" | "self") {
            self.has_host = true;
        }
        walk::walk_identifier_reference(self, it);
    }

    fn visit_unary_expression(&mut self, it: &UnaryExpression<'a>) {
        if it.operator == UnaryOperator::Typeof
            && matches!(&it.argument, Expression::Identifier(identifier) if matches!(identifier.name.as_str(), "window" | "global" | "self"))
        {
            self.has_typeof_host = true;
        }
        walk::walk_unary_expression(self, it);
    }
}

pub(crate) fn entry_function<'a>(
    program: &'a Program<'a>,
) -> Option<(&'a CallExpression<'a>, &'a Function<'a>)> {
    entry_call(program).and_then(|call| match &call.callee {
        Expression::FunctionExpression(function) => Some((call, &**function)),
        _ => None,
    })
}
fn is_webassembly_module(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(expression, Expression::StaticMemberExpression(member) if is_unresolved_global(&member.object, "WebAssembly", scoping) && member.property.name == "Module")
}

fn is_unresolved_global(expression: &Expression<'_>, name: &str, scoping: &Scoping) -> bool {
    let Expression::Identifier(identifier) = expression else {
        return false;
    };
    identifier.name == name
        && identifier.reference_id.get().is_some()
        && analysis::identifier_symbol(identifier, scoping).is_none()
}

fn is_wasm_module_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[..4] == [0, 97, 115, 109] && bytes[4..8] == [1, 0, 0, 0]
}

fn is_base64_call(
    expression: &Expression<'_>,
    scoping: &Scoping,
    decoder_symbols: &BTreeSet<SymbolId>,
) -> bool {
    matches!(expression, Expression::CallExpression(call)
        if call.arguments.len() == 1
            && matches!(call.arguments[0].to_expression(), Expression::StringLiteral(_))
            && matches!(&call.callee, Expression::Identifier(identifier)
                if analysis::identifier_symbol(identifier, scoping)
                    .is_some_and(|symbol| decoder_symbols.contains(&symbol)))
    )
}
fn base64_call_string(expression: &Expression<'_>) -> Option<String> {
    if let Expression::CallExpression(call) = expression {
        if call.arguments.len() == 1 {
            if let Expression::StringLiteral(value) = call.arguments[0].to_expression() {
                return Some(value.value.to_string());
            }
        }
    }
    None
}
fn array_elements<'a>(expression: &'a Expression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    if let Expression::ArrayExpression(array) = expression {
        array
            .elements
            .iter()
            .map(|element| match element {
                ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => {
                    None
                }
                other => Some(other.to_expression()),
            })
            .collect()
    } else {
        None
    }
}
fn array_all_strings(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    (!elements.is_empty()
        && elements
            .iter()
            .all(|e| matches!(e, Expression::StringLiteral(_))))
    .then_some(elements.len())
}
fn array_all_numbers(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    (!elements.is_empty() && elements.iter().all(|e| static_number(e).is_some()))
        .then_some(elements.len())
}
fn array_all_functions(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    (!elements.is_empty()
        && elements.iter().all(|e| {
            matches!(
                e,
                Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
            )
        }))
    .then_some(elements.len())
}

fn function_table_length(expression: &Expression<'_>) -> Option<usize> {
    match expression {
        Expression::ArrayExpression(array) => {
            let mut functions = 0;
            for element in &array.elements {
                match element {
                    ArrayExpressionElement::FunctionExpression(_)
                    | ArrayExpressionElement::ArrowFunctionExpression(_) => functions += 1,
                    ArrayExpressionElement::Elision(_) => {}
                    ArrayExpressionElement::SpreadElement(_) => return None,
                    _ => return None,
                }
            }
            (functions > 0).then_some(array.elements.len())
        }
        Expression::ObjectExpression(object) => {
            let mut keys = BTreeSet::new();
            let mut functions = 0;
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                let index = numeric_property_index(&property.key)?;
                if property.computed || !keys.insert(index) {
                    return None;
                }
                if !matches!(
                    property.value,
                    Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
                ) {
                    return None;
                }
                functions += 1;
            }
            (functions > 0).then_some(functions)
        }
        _ => None,
    }
}

fn numeric_property_index(key: &PropertyKey<'_>) -> Option<u32> {
    match key {
        PropertyKey::NumericLiteral(number)
            if number.value.is_finite()
                && number.value >= 0.0
                && number.value.fract() == 0.0
                && number.value < u32::MAX as f64 =>
        {
            Some(number.value as u32)
        }
        PropertyKey::StringLiteral(string) => {
            let index = string.value.parse::<u32>().ok()?;
            (index < u32::MAX && string.value == index.to_string()).then_some(index)
        }
        _ => None,
    }
}

fn proven_operation_array(expression: &Expression<'_>) -> bool {
    let Expression::ArrayExpression(array) = expression else {
        return false;
    };
    let mut functions = 0;
    for element in &array.elements {
        match element {
            ArrayExpressionElement::FunctionExpression(function) => {
                functions += 1;
                if function.params.items.is_empty() {
                    return false;
                }
            }
            ArrayExpressionElement::ArrowFunctionExpression(function) => {
                functions += 1;
                if function.params.items.is_empty() {
                    return false;
                }
            }
            ArrayExpressionElement::Elision(_) => {}
            _ => return false,
        }
    }
    functions >= 2
}

fn array_of_function_arrays(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    (!elements.is_empty()
        && elements
            .iter()
            .all(|element| array_all_functions(element).is_some()))
    .then_some(elements.len())
}

fn array_of_numeric_records(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    if elements.is_empty() {
        return None;
    }
    let valid = elements.iter().all(|e| {
        if let Expression::ObjectExpression(object) = e {
            let numeric_arrays = object
                .properties
                .iter()
                .filter(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        array_all_numbers_allow_empty(&property.value).is_some()
                    }
                    ObjectPropertyKind::SpreadProperty(_) => false,
                })
                .count();
            numeric_arrays >= 2
        } else {
            false
        }
    });
    valid.then_some(elements.len())
}

fn computed_member_base_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    let Expression::Identifier(identifier) = &member.object else {
        return None;
    };
    analysis::identifier_symbol(identifier, scoping)
}

fn config_member_source(
    expression: &Expression<'_>,
    scoping: &Scoping,
    aliases: &BTreeMap<SymbolId, SymbolId>,
) -> Option<SymbolId> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    let Expression::Identifier(identifier) = &member.object else {
        return None;
    };
    let alias = analysis::identifier_symbol(identifier, scoping)?;
    aliases.get(&alias).copied()
}

fn nested_dispatch_base_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    recognize_nested_dispatch(expression, scoping, NestedDispatchPolicy::PipelineHeuristic)
        .map(|dispatch| dispatch.table)
}

fn array_all_numbers_allow_empty(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    elements
        .iter()
        .all(|e| static_number(e).is_some())
        .then_some(elements.len())
}
fn is_null_object_cache(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(expression, Expression::CallExpression(call) if matches!(&call.callee, Expression::StaticMemberExpression(member) if is_unresolved_global(&member.object, "Object", scoping) && member.property.name == "create") && matches!(call.arguments.first().map(Argument::to_expression), Some(Expression::NullLiteral(_))))
}
fn static_number(expression: &Expression<'_>) -> Option<f64> {
    match expression {
        Expression::NumericLiteral(number) => Some(number.value),
        Expression::UnaryExpression(unary) => {
            let value = static_number(&unary.argument)?;
            match unary.operator {
                UnaryOperator::UnaryNegation => Some(-value),
                UnaryOperator::UnaryPlus => Some(value),
                UnaryOperator::BitwiseNot => Some((!crate::js_number::to_int32(value)) as f64),
                _ => None,
            }
        }
        _ => None,
    }
}

fn static_value(expression: &Expression<'_>) -> Option<StaticValue> {
    match expression {
        Expression::StringLiteral(value) => Some(StaticValue::String(value.value.to_string())),
        Expression::NumericLiteral(value) => Some(StaticValue::Number(value.value)),
        Expression::BooleanLiteral(value) => Some(StaticValue::Boolean(value.value)),
        Expression::NullLiteral(_) => Some(StaticValue::Null),
        Expression::UnaryExpression(_) => static_number(expression).map(StaticValue::Number),
        Expression::ArrayExpression(array) => Some(StaticValue::Array(
            array
                .elements
                .iter()
                .map(|element| match element {
                    ArrayExpressionElement::SpreadElement(_)
                    | ArrayExpressionElement::Elision(_) => None,
                    other => static_value(other.to_expression()),
                })
                .collect::<Option<Vec<_>>>()?,
        )),
        Expression::ObjectExpression(object) => {
            let mut values = BTreeMap::new();
            for property in object.properties.iter() {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                if property.computed || property.kind != PropertyKind::Init {
                    return None;
                };
                let key = match &property.key {
                    PropertyKey::StaticIdentifier(id) => id.name.to_string(),
                    PropertyKey::StringLiteral(value) => value.value.to_string(),
                    PropertyKey::NumericLiteral(value) => value.value.to_string(),
                    _ => return None,
                };
                if key == "__proto__" || values.contains_key(&key) {
                    return None;
                }
                values.insert(key, static_value(&property.value)?);
            }
            Some(StaticValue::Object(values))
        }
        _ => None,
    }
}

/// URL-safe base64 decoder used by the kernel.  It accepts padded and unpadded
/// input, rejects malformed input, and never substitutes a guessed byte.
pub fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    crate::kernel::wasm_module::decode_base64(
        input,
        crate::kernel::wasm_module::Base64Policy::UrlSafe,
    )
}

/// Decode the bounded XOR form used by the legacy string decoder. Both inputs
/// must be proven base64 constants of equal, non-zero length; the first byte is
/// the checked third-XOR seed and is not emitted as text.
pub fn decode_xor(left: &str, right: &str) -> Result<Vec<u8>, String> {
    let left = decode_base64(left)?;
    let right = decode_base64(right)?;
    if left.is_empty() || right.is_empty() {
        return Err("XOR operands decode to empty bytes".into());
    }
    if left.len() != right.len() {
        return Err("XOR operands have unequal decoded lengths".into());
    }
    let seed = left[0].wrapping_add(right[0]);
    Ok(left[1..]
        .iter()
        .zip(&right[1..])
        .map(|(a, b)| a ^ b ^ seed)
        .collect())
}
fn has_dynamic_scope(program: &Program<'_>) -> bool {
    let (scoping, _) = analysis::analyze(program);
    let mut visitor = DynamicVisitor {
        dynamic: false,
        scoping: &scoping,
    };
    visitor.visit_program(program);
    visitor.dynamic
}
struct DynamicVisitor<'a> {
    dynamic: bool,
    scoping: &'a Scoping,
}
impl<'a> Visit<'a> for DynamicVisitor<'a> {
    fn visit_with_statement(&mut self, _: &WithStatement<'a>) {
        self.dynamic = true;
    }
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Expression::Identifier(identifier) = &it.callee {
            let unresolved = analysis::identifier_symbol(identifier, self.scoping).is_none();
            if identifier.name == "eval" && unresolved {
                self.dynamic = true;
            }
        }
        walk::walk_call_expression(self, it);
    }
}
#[cfg(test)]
mod nested_dispatch_adapter_tests {
    use super::*;

    fn dispatch_role(source: &str) -> Option<String> {
        let allocator = Allocator::default();
        let parsed = analysis::parse(&allocator, source);
        assert!(parsed.errors.is_empty(), "{source:?}: {:?}", parsed.errors);
        let (scoping, _) = analysis::analyze(&parsed.program);
        let mut detector = Detector::new(&scoping, &parsed.program);
        detector.visit_program(&parsed.program);
        detector.classify();
        detector
            .mappings
            .into_iter()
            .find(|mapping| mapping.role == VmRole::DispatchTable)
            .map(|mapping| mapping.old_name)
    }

    /// Exercise the `Detector` adapter that calls `nested_dispatch_base_symbol`,
    /// not just the shared recognizer.  The pipeline policy must keep its
    /// historical looseness while the strict callers tighten their own copy.
    #[test]
    fn pipeline_heuristic_keeps_historical_acceptance_through_detector() {
        for (label, source) in [
            (
                "exact",
                "var TABLE = [{ p: [0], S: [1] }]; var BYTES = [1]; function f() { TABLE[this.S][BYTES[this.P++]](); } f();",
            ),
            (
                "identifier cursor",
                "var TABLE = [{ p: [0], S: [1] }]; var BYTES = [1]; function f() { TABLE[this.S][BYTES[cursor++]](); } f();",
            ),
            (
                "other state receiver",
                "var TABLE = [{ p: [0], S: [1] }]; var BYTES = [1]; function f() { TABLE[receiver.S][BYTES[this.P++]](); } f();",
            ),
            (
                "call bytecode object",
                "var TABLE = [{ p: [0], S: [1] }]; function f() { TABLE[this.S][makeBytes()[this.P++]](); } f();",
            ),
        ] {
            assert_eq!(
                dispatch_role(source).as_deref(),
                Some("TABLE"),
                "{label}"
            );
        }

        for (label, source) in [
            (
                "prefix cursor increment",
                "var TABLE = [{ p: [0], S: [1] }]; var BYTES = [1]; function f() { TABLE[this.S][BYTES[++this.P]](); } f();",
            ),
            (
                "cursor decrement",
                "var TABLE = [{ p: [0], S: [1] }]; var BYTES = [1]; function f() { TABLE[this.S][BYTES[this.P--]](); } f();",
            ),
            (
                "compound cursor assignment",
                "var TABLE = [{ p: [0], S: [1] }]; var BYTES = [1]; function f() { TABLE[this.S][BYTES[this.P += 1]](); } f();",
            ),
            (
                "non-identifier table base",
                "var TABLE = [{ p: [0], S: [1] }]; var BYTES = [1]; var holder = { TABLE: TABLE }; function f() { holder.TABLE[this.S][BYTES[this.P++]](); } f();",
            ),
            (
                "unresolved table",
                "var BYTES = [1]; function f() { MISSING[this.S][BYTES[this.P++]](); } f();",
            ),
        ] {
            assert_eq!(dispatch_role(source), None, "{label}");
        }
    }
}

#[cfg(test)]
mod saved_report_tests {
    use super::*;

    #[test]
    fn a_saved_report_loads_with_every_fact() {
        let fact = |value| ExtractionFact {
            kind: ExtractionKind::NumberTable,
            status: ExtractionStatus::Extracted,
            binding: Some("numbers".into()),
            span: Span::new(3, 9),
            length: Some(1),
            decoded_bytes: Some(vec![0, 255]),
            value: Some(StaticValue::Array(vec![StaticValue::Number(value)])),
            evidence: "test".into(),
        };
        let report = PipelineReport {
            output: "var numbers = [1];".into(),
            input_hash: "abc".into(),
            stages: Vec::new(),
            diagnostics: Vec::new(),
            binding_mappings: Vec::new(),
            property_mappings: Vec::new(),
            binding_residue: vec!["Bx".into()],
            extraction: [0.1, -0.0, 1e300, f64::INFINITY, f64::NEG_INFINITY, f64::NAN]
                .into_iter()
                .map(fact)
                .collect(),
            proven_operation_tables: Vec::new(),
        };
        let loaded = PipelineReport::from_json(&report.to_json()).unwrap();
        assert_eq!(loaded.output, report.output);
        assert_eq!(loaded.binding_residue, report.binding_residue);
        assert_eq!(loaded.extraction[0].span, Span::new(3, 9));
        let numbers = loaded
            .extraction
            .iter()
            .map(|fact| match &fact.value {
                Some(StaticValue::Array(items)) => match items[0] {
                    StaticValue::Number(n) => n,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            numbers[..5],
            [0.1, -0.0, 1e300, f64::INFINITY, f64::NEG_INFINITY]
        );
        assert!(numbers[1].is_sign_negative() && numbers[5].is_nan());
        assert!(PipelineReport::from_json("{\"format\":\"other\",\"report\":{}}").is_err());
    }
}

#[cfg(test)]
mod entry_call_tests {
    use super::*;

    fn accepted(slots: &str) -> bool {
        let source = format!(
            "(function(host, seed, mode, initialMemorySlots){{}})(\
             typeof window !== \"undefined\" && window.window === window ? window : this, 0, 0, {slots})"
        );
        let allocator = Allocator::default();
        let parsed = analysis::parse(&allocator, &source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        entry_call(&parsed.program).is_some()
    }

    #[test]
    fn entry_call_accepts_a_numeric_slot_array_even_when_empty() {
        assert!(accepted("[1, 0]"));
        assert!(accepted("[]"));
        assert!(!accepted("[1, \"zero\"]"));
        assert!(!accepted("{}"));
    }
}
