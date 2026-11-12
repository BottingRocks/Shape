//! Source-free, integrity-checked wire artifact for a prepared VM prefix.
//!
//! Compilation is deliberately the only path that touches `PipelineReport`.
//! Loading this module performs bounded JSON decoding, digest verification and
//! structural cross-reference checks, then rebuilds the already-proved Rust
//! runtime indexes. It does not parse JavaScript, run Oxc analysis, or rerun a
//! source matcher. The digest is an integrity check, not source authenticity.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::{Cursor, Read},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::operations::templates::{compile_closure_plan, is_valid_closure_identifier};
use crate::VmStringFoldPlan;
use crate::{
    admit_recipe, ActualProperties, ClosureExpression, ClosureStatement, DispatchCell,
    DispatchStart, HandlerTemplate, HandlerTemplateReport, JsOperandSeparationReport,
    LiftedHandler, OperandCoverage, OperandDescriptor, OperandHandlerStatus, OperandInputIdentity,
    OperandSpan, PrefixLimits, PrefixPreparation, PropertyValue, ProvenOperationTable,
    RecipeBounds, RecipeOperation, SemanticValue, TemplateStatus, ThreadDescriptor,
};
use crate::{PipelineReport, PrefixPrepareError};

use super::super::step::SelectorIndex;
use super::proof::{PatchHandlerProof, RunLoopProof};

pub const ARTIFACT_FORMAT: &str = "shape-kernel-artifact/v1";
pub const COMPILER_PROVENANCE: &str = "shape-rust-oxc-0.1";
pub const MAX_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TABLE_CELLS: usize = 1_000_000;
const MAX_HANDLERS: usize = 100_000;
const MAX_DESCRIPTOR_ROWS: usize = 100_000;
const MAX_DESCRIPTOR_SLOTS: usize = 1_000_000;
const MAX_COMPRESSED_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
const MAX_PAYLOAD_JSON_BYTES: usize = 32 * 1024 * 1024;
const MAX_ZSTD_WINDOW_LOG: u32 = 23;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelArtifactError {
    TooLarge { actual: usize, limit: usize },
    InvalidJson(String),
    WrongFormat(String),
    IntegrityMismatch { expected: String, actual: String },
    Invalid(String),
    Unsupported(String),
}

impl fmt::Display for KernelArtifactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { actual, limit } => {
                write!(f, "artifact is {actual} bytes, over {limit}")
            }
            Self::InvalidJson(error) => write!(f, "invalid kernel artifact JSON: {error}"),
            Self::WrongFormat(format) => write!(f, "unsupported kernel artifact format {format:?}"),
            Self::IntegrityMismatch { expected, actual } => write!(
                f,
                "kernel artifact integrity digest mismatch (expected {expected}, got {actual})"
            ),
            Self::Invalid(error) => write!(f, "invalid kernel artifact: {error}"),
            Self::Unsupported(error) => write!(f, "unsupported kernel artifact: {error}"),
        }
    }
}

impl std::error::Error for KernelArtifactError {}

#[derive(Debug)]
pub enum KernelArtifactCompileError {
    Pipeline(String),
    Preparation(PrefixPrepareError),
    Artifact(KernelArtifactError),
}

impl fmt::Display for KernelArtifactCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pipeline(error) => {
                write!(f, "kernel artifact compilation pipeline failed: {error}")
            }
            Self::Preparation(error) => {
                write!(f, "kernel artifact compilation proof failed: {error}")
            }
            Self::Artifact(error) => write!(f, "kernel artifact construction failed: {error}"),
        }
    }
}
impl std::error::Error for KernelArtifactCompileError {}

/// A compact, source-free compiled prefix. The private v1 wire stores the
/// canonical typed payload as bounded zstd-compressed JSON in one base64
/// string. Handler recipes occur once in `templates`; operand handlers
/// reference them by `handler_index`. The payload remains JSON internally so
/// unknown fields and all existing structural checks stay fail-closed after
/// decompression.
#[derive(Debug, Clone)]
pub struct KernelArtifact {
    format: String,
    compiler_provenance: String,
    source_hash: String,
    artifact_source_hash: String,
    source_basis: String,
    standardized_length: u64,
    integrity_digest: String,
    payload: ArtifactPayload,
    prepared: Option<Box<PrefixPreparation>>,
}

#[derive(Debug, Clone, PartialEq)]
struct ArtifactPayload {
    bytecode: Vec<u8>,
    dispatch_cells: Vec<Vec<DispatchCell>>,
    templates: HandlerTemplateReport,
    operands: ArtifactOperands,
    patch_selector: u32,
    patch_span: OperandSpan,
    patch_operand_width: usize,
    bytecode_span: OperandSpan,
    run_loop_span: OperandSpan,
    completion_register: String,
    exception_stack_field: String,
    exception_helper: Option<OperandSpan>,
    constant_tables: Vec<(String, Vec<String>)>,
    limits: PrefixLimits,
    default_entry: Option<DispatchStart>,
    entry_initial_slots: Vec<u32>,
    thread_descriptors: Vec<ThreadDescriptor>,
    descriptor_records: Vec<DescriptorRecord>,
    initial_slot_inputs: BTreeMap<u32, ArtifactValue>,
    string_fold_plan: Option<VmStringFoldPlan>,
    /// Each number table's entries as IEEE-754 bit patterns: JSON number
    /// text does not round-trip every f64.
    number_tables: Vec<(String, Vec<u64>)>,
    /// The source text of each entry of the kernel's `vmNativeFunctions`
    /// array (`ReferenceError`, `Function.prototype.bind`, …): the program
    /// can be handed the array itself.
    native_functions: Vec<String>,
    wasm: Option<super::wasm::WasmMachine>,
}

#[derive(Debug, Clone, PartialEq)]
struct ArtifactOperands {
    schema_version: String,
    source_hash: String,
    table: ProvenOperationTable,
    input_identity: Option<OperandInputIdentity>,
    entry_coordinate: Option<crate::OperandEntryCoordinate>,
    handlers: Vec<ArtifactLiftedHandler>,
    coverage: OperandCoverage,
    unresolved: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct ArtifactLiftedHandler {
    handler_index: u32,
    table_key: u32,
    source_span: OperandSpan,
    entry_coordinate: crate::OperandEntryCoordinate,
    operands: Vec<OperandDescriptor>,
    pc_delta: u32,
    captured_dependencies: Vec<crate::CapturedDependency>,
    status: OperandHandlerStatus,
    unresolved: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptorRecord {
    pub descriptor: Option<ThreadDescriptor>,
    pub entry: Option<DispatchStart>,
    pub invalid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryFacts {
    pub pc: u32,
    pub state: u32,
    pub initial_slots: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum ArtifactValue {
    Undefined,
    Null,
    Boolean(bool),
    NumberBits(u64),
    String(String),
    Object { identity: u64 },
    External { name: String },
}

#[derive(Debug, Serialize)]
struct Envelope<'a> {
    format: &'a str,
    compiler_provenance: &'a str,
    source_hash: &'a str,
    artifact_source_hash: &'a str,
    source_basis: &'a str,
    standardized_length: u64,
    integrity_digest: &'a str,
    payload: &'a str,
}

#[derive(Debug, Serialize)]
struct IntegrityMaterial<'a> {
    format: &'a str,
    compiler_provenance: &'a str,
    source_hash: &'a str,
    artifact_source_hash: &'a str,
    source_basis: &'a str,
    standardized_length: u64,
    payload: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEnvelope {
    format: String,
    compiler_provenance: String,
    source_hash: String,
    artifact_source_hash: String,
    #[serde(default)]
    source_basis: String,
    #[serde(default)]
    standardized_length: u64,
    integrity_digest: String,
    payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePayloadOwned {
    bytecode: String,
    dispatch_cells: Vec<Vec<DispatchCell>>,
    templates: HandlerTemplateReport,
    operands: WireOperands,
    patch_selector: u32,
    patch_span: OperandSpan,
    patch_operand_width: usize,
    bytecode_span: OperandSpan,
    run_loop_span: OperandSpan,
    completion_register: String,
    exception_stack_field: String,
    exception_helper: Option<OperandSpan>,
    constant_tables: Vec<(String, Vec<String>)>,
    limits: PrefixLimits,
    default_entry: Option<DispatchStart>,
    #[serde(default)]
    entry_initial_slots: Vec<u32>,
    #[serde(default)]
    descriptor_records: Vec<DescriptorRecord>,
    initial_slot_inputs: BTreeMap<u32, ArtifactValue>,
    string_fold_plan: Option<VmStringFoldPlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    number_tables: Vec<(String, Vec<u64>)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    native_functions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wasm: Option<super::wasm::WasmMachine>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireOperands {
    schema_version: String,
    source_hash: String,
    table: ProvenOperationTable,
    input_identity: Option<OperandInputIdentity>,
    entry_coordinate: Option<crate::OperandEntryCoordinate>,
    handlers: Vec<WireLiftedHandler>,
    coverage: OperandCoverage,
    unresolved: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireLiftedHandler {
    handler_index: u32,
    table_key: u32,
    source_span: OperandSpan,
    entry_coordinate: crate::OperandEntryCoordinate,
    operands: Vec<OperandDescriptor>,
    pc_delta: u32,
    captured_dependencies: Vec<crate::CapturedDependency>,
    status: OperandHandlerStatus,
    unresolved: Vec<String>,
}

#[derive(Debug)]
struct WirePayload<'a> {
    bytecode: String,
    dispatch_cells: &'a [Vec<DispatchCell>],
    templates: &'a HandlerTemplateReport,
    operands: WireOperandsRef<'a>,
    patch_selector: u32,
    patch_span: &'a OperandSpan,
    patch_operand_width: usize,
    bytecode_span: &'a OperandSpan,
    run_loop_span: &'a OperandSpan,
    completion_register: &'a str,
    exception_stack_field: &'a str,
    exception_helper: &'a Option<OperandSpan>,
    constant_tables: &'a [(String, Vec<String>)],
    limits: PrefixLimits,
    default_entry: Option<DispatchStart>,
    entry_initial_slots: &'a [u32],
    descriptor_records: &'a [DescriptorRecord],
    initial_slot_inputs: &'a BTreeMap<u32, ArtifactValue>,
    string_fold_plan: &'a Option<VmStringFoldPlan>,
    number_tables: &'a [(String, Vec<u64>)],
    native_functions: &'a [String],
    wasm: &'a Option<super::wasm::WasmMachine>,
}

impl Serialize for WirePayload<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let fields =
            19 + usize::from(self.wasm.is_some()) + usize::from(!self.number_tables.is_empty()) + usize::from(!self.native_functions.is_empty());
        let mut state = serializer.serialize_struct("payload", fields)?;
        state.serialize_field("bytecode", &self.bytecode)?;
        state.serialize_field("dispatch_cells", self.dispatch_cells)?;
        state.serialize_field("templates", self.templates)?;
        state.serialize_field("operands", &self.operands)?;
        state.serialize_field("patch_selector", &self.patch_selector)?;
        state.serialize_field("patch_span", self.patch_span)?;
        state.serialize_field("patch_operand_width", &self.patch_operand_width)?;
        state.serialize_field("bytecode_span", self.bytecode_span)?;
        state.serialize_field("run_loop_span", self.run_loop_span)?;
        state.serialize_field("completion_register", self.completion_register)?;
        state.serialize_field("exception_stack_field", self.exception_stack_field)?;
        state.serialize_field("exception_helper", self.exception_helper)?;
        state.serialize_field("constant_tables", self.constant_tables)?;
        state.serialize_field("limits", &self.limits)?;
        state.serialize_field("default_entry", &self.default_entry)?;
        state.serialize_field("entry_initial_slots", self.entry_initial_slots)?;
        state.serialize_field("descriptor_records", self.descriptor_records)?;
        state.serialize_field("initial_slot_inputs", self.initial_slot_inputs)?;
        state.serialize_field("string_fold_plan", self.string_fold_plan)?;
        // An artifact without number tables is unchanged.
        if !self.number_tables.is_empty() {
            state.serialize_field("number_tables", self.number_tables)?;
        }
        // Nor is one without a native catalogue.
        if !self.native_functions.is_empty() {
            state.serialize_field("native_functions", self.native_functions)?;
        }
        // A JavaScript kernel's artifact is unchanged.
        if self.wasm.is_some() {
            state.serialize_field("wasm", self.wasm)?;
        }
        state.end()
    }
}

#[derive(Debug)]
struct WireOperandsRef<'a>(&'a ArtifactOperands);

impl Serialize for WireOperandsRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let value = self.0;
        let mut state = serializer.serialize_struct("operands", 8)?;
        state.serialize_field("schema_version", &value.schema_version)?;
        state.serialize_field("source_hash", &value.source_hash)?;
        state.serialize_field("table", &value.table)?;
        state.serialize_field("input_identity", &value.input_identity)?;
        state.serialize_field("entry_coordinate", &value.entry_coordinate)?;
        state.serialize_field(
            "handlers",
            &value
                .handlers
                .iter()
                .map(WireLiftedHandlerRef)
                .collect::<Vec<_>>(),
        )?;
        state.serialize_field("coverage", &value.coverage)?;
        state.serialize_field("unresolved", &value.unresolved)?;
        state.end()
    }
}

struct WireLiftedHandlerRef<'a>(&'a ArtifactLiftedHandler);
impl Serialize for WireLiftedHandlerRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = self.0;
        WireLiftedHandler {
            handler_index: value.handler_index,
            table_key: value.table_key,
            source_span: value.source_span.clone(),
            entry_coordinate: value.entry_coordinate,
            operands: value.operands.clone(),
            pc_delta: value.pc_delta,
            captured_dependencies: value.captured_dependencies.clone(),
            status: value.status.clone(),
            unresolved: value.unresolved.clone(),
        }
        .serialize(serializer)
    }
}

fn encode_wire_payload(payload: &WirePayload<'_>) -> Result<String, KernelArtifactError> {
    let json = serialize_wire_payload(payload)?;
    if json.len() > MAX_PAYLOAD_JSON_BYTES {
        return Err(KernelArtifactError::Invalid(
            "compiled payload exceeds its decompression bound".into(),
        ));
    }
    let compressed = zstd::stream::encode_all(Cursor::new(json), 9).map_err(|error| {
        KernelArtifactError::Invalid(format!("payload compression failed: {error}"))
    })?;
    if compressed.len() > MAX_COMPRESSED_PAYLOAD_BYTES {
        return Err(KernelArtifactError::Invalid(
            "compressed payload exceeds its wire bound".into(),
        ));
    }
    Ok(encode_base64(&compressed))
}

fn serialize_wire_payload(payload: &WirePayload<'_>) -> Result<Vec<u8>, KernelArtifactError> {
    serde_json::to_vec(payload).map_err(|error| KernelArtifactError::InvalidJson(error.to_string()))
}

fn decode_payload(encoded: &str) -> Result<WirePayloadOwned, KernelArtifactError> {
    let compressed = decode_base64_with_limit(
        encoded,
        MAX_COMPRESSED_PAYLOAD_BYTES,
        "invalid compressed payload encoding",
    )?;
    let mut decoder =
        zstd::stream::read::Decoder::new(Cursor::new(compressed)).map_err(|error| {
            KernelArtifactError::Invalid(format!("invalid compressed payload: {error}"))
        })?;
    decoder
        .window_log_max(MAX_ZSTD_WINDOW_LOG)
        .map_err(|error| {
            KernelArtifactError::Invalid(format!("payload window exceeds bound: {error}"))
        })?;
    let mut json = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = decoder.read(&mut buffer).map_err(|error| {
            KernelArtifactError::Invalid(format!("payload decompression failed: {error}"))
        })?;
        if read == 0 {
            break;
        }
        if json.len().saturating_add(read) > MAX_PAYLOAD_JSON_BYTES {
            return Err(KernelArtifactError::Invalid(
                "decompressed payload exceeds its structural bound".into(),
            ));
        }
        json.extend_from_slice(&buffer[..read]);
    }
    serde_json::from_slice(&json)
        .map_err(|error| KernelArtifactError::InvalidJson(format!("compressed payload: {error}")))
}

impl KernelArtifact {
    pub fn compile(
        source: &str,
        report: &PipelineReport,
        limits: PrefixLimits,
    ) -> Result<Self, KernelArtifactCompileError> {
        Self::compile_from_raw(source, report, limits)
    }

    pub fn compile_from_report(
        report: &PipelineReport,
        limits: PrefixLimits,
    ) -> Result<Self, KernelArtifactCompileError> {
        Self::compile_from_existing_pipeline_report(report, limits)
    }

    pub fn compile_source(
        source: &str,
        limits: PrefixLimits,
    ) -> Result<Self, KernelArtifactCompileError> {
        let report = crate::deobfuscate_with_report(source)
            .map_err(|error| KernelArtifactCompileError::Pipeline(error.to_string()))?;
        Self::compile_from_raw(source, &report, limits)
    }

    /// Compile a source-free artifact from already-standardized deobfuscated
    /// JavaScript.
    ///
    /// The standardized-source report projection is rebuilt to obtain the
    /// table facts the artifact format consumes; clean/rename presentation is
    /// not replayed. The projection re-states canonical helper roles and
    /// context/memory fields so the runtime payload matches a compile from the
    /// same source's raw form. Because the input is not the original raw
    /// capture, the artifact is deliberately marked `declared_only` and does
    /// not claim original source identity.
    pub fn compile_from_deobfuscated(
        source: &str,
        limits: PrefixLimits,
    ) -> Result<Self, KernelArtifactCompileError> {
        let mut report = crate::transforms::deob::pipeline::standardized_report(source)
            .map_err(|error| KernelArtifactCompileError::Pipeline(error.to_string()))?;
        if report.proven_operation_tables.is_empty() {
            let table =
                super::super::proof::recover_operation_table(&report.output).map_err(|error| {
                    KernelArtifactCompileError::Preparation(PrefixPrepareError::InvalidFact(error))
                })?;
            report.proven_operation_tables.push(table);
        }
        Self::compile_from_existing_pipeline_report(&report, limits)
    }

    /// Perform source proofs once and emit the source-free runtime artifact.
    pub fn compile_from_raw(
        source: &str,
        report: &PipelineReport,
        limits: PrefixLimits,
    ) -> Result<Self, KernelArtifactCompileError> {
        let preparation = super::prepare_prefix_from_report(source, report, limits)
            .map_err(KernelArtifactCompileError::Preparation)?;
        let mut artifact = preparation
            .export_artifact()
            .map_err(KernelArtifactCompileError::Artifact)?;
        artifact.source_basis = "raw_source_observed".into();
        artifact.standardized_length =
            (report.output.len() as u64).max(artifact.standardized_length);
        artifact.install_actual_facts(report);
        artifact.payload.string_fold_plan = Some(VmStringFoldPlan::compile(report));
        artifact.payload.native_functions = native_catalogue(&report.output);
        artifact
            .payload
            .string_fold_plan
            .as_ref()
            .expect("plan was just installed")
            .validate()
            .map_err(KernelArtifactError::Invalid)
            .map_err(KernelArtifactCompileError::Artifact)?;
        artifact.integrity_digest = artifact
            .payload_digest()
            .map_err(KernelArtifactCompileError::Artifact)?;
        // The plan is the preparation's string decoder too: prepare as a
        // loaded artifact does.
        artifact.prepared = Some(Box::new(
            artifact
                .build_preparation()
                .map_err(KernelArtifactCompileError::Artifact)?,
        ));
        Ok(artifact)
    }

    /// Compile from a complete saved report. The report's source hashes are
    /// preserved, but the artifact records `declared_only` because the raw
    /// bytes are unavailable and are not re-proven here.
    pub fn compile_from_existing_pipeline_report(
        report: &PipelineReport,
        limits: PrefixLimits,
    ) -> Result<Self, KernelArtifactCompileError> {
        let preparation = super::prepare_prefix_from_saved_report(report, limits)
            .map_err(KernelArtifactCompileError::Preparation)?;
        let mut artifact = preparation
            .export_artifact()
            .map_err(KernelArtifactCompileError::Artifact)?;
        artifact.source_basis = "declared_only".into();
        artifact.standardized_length =
            (report.output.len() as u64).max(artifact.standardized_length);
        artifact.install_actual_facts(report);
        artifact.payload.string_fold_plan = Some(VmStringFoldPlan::compile(report));
        artifact
            .payload
            .string_fold_plan
            .as_ref()
            .expect("plan was just installed")
            .validate()
            .map_err(KernelArtifactError::Invalid)
            .map_err(KernelArtifactCompileError::Artifact)?;
        artifact.integrity_digest = artifact
            .payload_digest()
            .map_err(KernelArtifactCompileError::Artifact)?;
        // The plan is the preparation's string decoder too: prepare as a
        // loaded artifact does.
        artifact.prepared = Some(Box::new(
            artifact
                .build_preparation()
                .map_err(KernelArtifactCompileError::Artifact)?,
        ));
        Ok(artifact)
    }

    pub(crate) fn from_preparation(
        preparation: &PrefixPreparation,
    ) -> Result<Self, KernelArtifactError> {
        let mut templates = preparation.templates.clone();
        for template in &mut templates.handlers {
            sanitize_template(template).map_err(KernelArtifactError::Unsupported)?;
        }
        let operands = ArtifactOperands {
            schema_version: preparation.operands.schema_version.clone(),
            source_hash: preparation.operands.source_hash.clone(),
            table: preparation.operands.table.clone(),
            input_identity: preparation.operands.input_identity.clone(),
            entry_coordinate: preparation.operands.entry_coordinate,
            handlers: preparation
                .operands
                .handlers
                .iter()
                .map(|handler| ArtifactLiftedHandler {
                    handler_index: handler.handler_index,
                    table_key: handler.table_key,
                    source_span: handler.source_span.clone(),
                    entry_coordinate: handler.entry_coordinate,
                    operands: handler.operands.clone(),
                    pc_delta: handler.pc_delta,
                    captured_dependencies: handler.captured_dependencies.clone(),
                    status: handler.status.clone(),
                    unresolved: handler.unresolved.clone(),
                })
                .collect(),
            coverage: preparation.operands.coverage.clone(),
            unresolved: preparation.operands.unresolved.clone(),
        };
        let payload = ArtifactPayload {
            bytecode: preparation.initial_image.clone(),
            dispatch_cells: preparation.table.clone(),
            templates,
            operands,
            patch_selector: preparation.patch_handler.selector,
            patch_span: preparation.patch_handler.span.clone(),
            patch_operand_width: preparation.patch_handler.operand_width,
            bytecode_span: preparation.bytecode_span.clone(),
            run_loop_span: preparation.run_loop.span.clone(),
            completion_register: preparation.run_loop.completion_register.clone(),
            exception_stack_field: preparation.run_loop.exception_stack_field.clone(),
            exception_helper: preparation.run_loop.exception_helper.clone(),
            constant_tables: preparation
                .constant_tables
                .iter()
                .map(|(name, values)| (name.clone(), values.iter().cloned().collect()))
                .collect(),
            limits: preparation.limits,
            default_entry: None,
            entry_initial_slots: Vec::new(),
            thread_descriptors: Vec::new(),
            descriptor_records: Vec::new(),
            initial_slot_inputs: BTreeMap::new(),
            string_fold_plan: None,
            number_tables: preparation
                .number_tables
                .iter()
                .map(|(name, values)| {
                    (
                        name.clone(),
                        values.iter().map(|value| value.to_bits()).collect(),
                    )
                })
                .collect(),
            native_functions: Vec::new(),
            wasm: preparation.wasm.clone(),
        };
        let mut artifact = Self {
            format: ARTIFACT_FORMAT.into(),
            compiler_provenance: COMPILER_PROVENANCE.into(),
            source_hash: preparation.source_hash.clone(),
            artifact_source_hash: preparation.artifact_source_hash.clone(),
            source_basis: "declared_only".into(),
            standardized_length: max_span_end(preparation),
            integrity_digest: String::new(),
            payload,
            prepared: None,
        };
        artifact.integrity_digest = artifact.payload_digest()?;
        artifact.validate()?;
        let prepared = artifact.build_preparation()?;
        Ok(Self {
            prepared: Some(Box::new(prepared)),
            ..artifact
        })
    }

    pub fn to_json(&self) -> Result<String, KernelArtifactError> {
        self.validate()?;
        let payload = self.encode_payload()?;
        let envelope = Envelope {
            format: &self.format,
            compiler_provenance: &self.compiler_provenance,
            source_hash: &self.source_hash,
            artifact_source_hash: &self.artifact_source_hash,
            source_basis: &self.source_basis,
            standardized_length: self.standardized_length,
            integrity_digest: &self.integrity_digest,
            payload: &payload,
        };
        serde_json::to_string(&envelope)
            .map_err(|error| KernelArtifactError::InvalidJson(error.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self, KernelArtifactError> {
        if json.len() > MAX_ARTIFACT_BYTES {
            return Err(KernelArtifactError::TooLarge {
                actual: json.len(),
                limit: MAX_ARTIFACT_BYTES,
            });
        }
        let loaded: WireEnvelope = serde_json::from_str(json)
            .map_err(|error| KernelArtifactError::InvalidJson(error.to_string()))?;
        if loaded.format != ARTIFACT_FORMAT {
            return Err(KernelArtifactError::WrongFormat(loaded.format));
        }
        if loaded.compiler_provenance != COMPILER_PROVENANCE {
            return Err(KernelArtifactError::Unsupported(format!(
                "compiler provenance {:?} is not {:?}",
                loaded.compiler_provenance, COMPILER_PROVENANCE
            )));
        }
        let wire_payload = decode_payload(&loaded.payload)?;
        let canonical_payload = serde_json::to_string(&wire_payload)
            .map_err(|error| KernelArtifactError::InvalidJson(error.to_string()))?;
        let WirePayloadOwned {
            bytecode: encoded_bytecode,
            dispatch_cells,
            templates,
            operands: wire_operands,
            patch_selector,
            patch_span,
            patch_operand_width,
            bytecode_span,
            run_loop_span,
            completion_register,
            exception_stack_field,
            exception_helper,
            constant_tables,
            limits,
            default_entry,
            entry_initial_slots,
            descriptor_records,
            initial_slot_inputs,
            string_fold_plan,
            number_tables,
            native_functions,
            wasm,
        } = wire_payload;
        let bytecode = decode_base64(&encoded_bytecode)?;
        let WireOperands {
            schema_version,
            source_hash,
            table,
            input_identity,
            entry_coordinate,
            handlers: wire_handlers,
            coverage,
            unresolved,
        } = wire_operands;
        let operands = ArtifactOperands {
            schema_version,
            source_hash,
            table,
            input_identity,
            entry_coordinate,
            handlers: wire_handlers
                .into_iter()
                .map(|handler| ArtifactLiftedHandler {
                    handler_index: handler.handler_index,
                    table_key: handler.table_key,
                    source_span: handler.source_span,
                    entry_coordinate: handler.entry_coordinate,
                    operands: handler.operands,
                    pc_delta: handler.pc_delta,
                    captured_dependencies: handler.captured_dependencies,
                    status: handler.status,
                    unresolved: handler.unresolved,
                })
                .collect(),
            coverage,
            unresolved,
        };
        let thread_descriptors = descriptor_records
            .iter()
            .filter_map(|record| record.descriptor.clone())
            .collect();
        let artifact = Self {
            format: loaded.format,
            compiler_provenance: loaded.compiler_provenance,
            source_hash: loaded.source_hash,
            artifact_source_hash: loaded.artifact_source_hash,
            source_basis: if loaded.source_basis.is_empty() {
                "declared_only".into()
            } else {
                loaded.source_basis
            },
            standardized_length: loaded.standardized_length,
            integrity_digest: loaded.integrity_digest,
            payload: ArtifactPayload {
                bytecode,
                dispatch_cells,
                templates,
                operands,
                patch_selector,
                patch_span,
                patch_operand_width,
                bytecode_span,
                run_loop_span,
                completion_register,
                exception_stack_field,
                exception_helper,
                constant_tables,
                limits,
                default_entry,
                entry_initial_slots,
                thread_descriptors,
                descriptor_records,
                initial_slot_inputs,
                string_fold_plan,
                number_tables,
                native_functions,
                wasm,
            },
            prepared: None,
        };
        let actual = artifact.payload_digest_for_payload(&canonical_payload)?;
        if actual != artifact.integrity_digest {
            return Err(KernelArtifactError::IntegrityMismatch {
                expected: artifact.integrity_digest,
                actual,
            });
        }
        artifact.validate()?;
        let prepared = artifact.build_preparation()?;
        Ok(Self {
            prepared: Some(Box::new(prepared)),
            ..artifact
        })
    }

    pub fn source_hash(&self) -> &str {
        &self.source_hash
    }
    /// Compare caller-provided source bytes with the compile-time capture
    /// identity. This is an identity check only; it does not re-prove source.
    pub fn verify_source_identity(&self, source: &str) -> Result<(), KernelArtifactError> {
        let actual = crate::util::fnv1a64_hex(source.as_bytes());
        if actual == self.source_hash {
            Ok(())
        } else {
            Err(KernelArtifactError::Invalid(format!(
                "source identity mismatch (expected {}, got {actual})",
                self.source_hash
            )))
        }
    }
    /// The source text of each `vmNativeFunctions` entry, when the kernel
    /// has that catalogue.
    pub fn native_functions(&self) -> &[String] {
        &self.payload.native_functions
    }

    pub fn artifact_source_hash(&self) -> &str {
        &self.artifact_source_hash
    }
    pub fn compiler_provenance(&self) -> &str {
        &self.compiler_provenance
    }
    pub fn integrity_digest(&self) -> &str {
        &self.integrity_digest
    }
    pub fn bytecode(&self) -> &[u8] {
        &self.payload.bytecode
    }
    pub fn dispatch_cells(&self) -> &[Vec<DispatchCell>] {
        &self.payload.dispatch_cells
    }
    pub fn templates(&self) -> &HandlerTemplateReport {
        &self.payload.templates
    }
    pub fn default_entry(&self) -> Option<DispatchStart> {
        self.payload.default_entry
    }

    pub fn entry_facts(&self) -> Option<EntryFacts> {
        self.payload.default_entry.map(|entry| EntryFacts {
            pc: entry.pc as u32,
            state: entry.state as u32,
            initial_slots: self.payload.entry_initial_slots.clone(),
        })
    }
    pub fn thread_descriptors(&self) -> &[ThreadDescriptor] {
        &self.payload.thread_descriptors
    }

    /// Preserve every source descriptor row, including typed invalid rows.
    /// Consumers must use this API instead of reparsing `report.output`.
    pub fn descriptor_records(&self) -> &[DescriptorRecord] {
        &self.payload.descriptor_records
    }

    pub fn entry_initial_slots(&self) -> &[u32] {
        &self.payload.entry_initial_slots
    }

    pub fn source_identity_basis(&self) -> &str {
        &self.source_basis
    }
    pub fn string_fold_plan(&self) -> Option<&VmStringFoldPlan> {
        self.payload.string_fold_plan.as_ref()
    }

    pub fn fold_emitted_vm_strings(
        &self,
        emitted: crate::EmittedJavaScript,
    ) -> Result<crate::vm_string::VmStringFoldResult, crate::vm_string::VmStringIntegrationError>
    {
        let Some(plan) = self.payload.string_fold_plan.as_ref() else {
            let mut report = crate::vm_string::VmStringFoldReport::default();
            report.decline(
                crate::vm_string::VmStringFoldReason::HelperOrDecoderNotProven,
                "artifact has no compiled vm-string fold plan",
            );
            return Ok(crate::vm_string::VmStringFoldResult { emitted, report });
        };
        crate::vm_string::fold_emitted_vm_strings_from_plan(plan, emitted)
    }

    pub fn prepare(&self) -> Result<PrefixPreparation, KernelArtifactError> {
        // `prepare()` preserves the limits captured at compile time. Callers
        // that need a different runtime policy must use `prepare_with_limits`.
        self.validate()?;
        if let Some(prepared) = &self.prepared {
            return Ok((**prepared).clone());
        }
        self.build_preparation()
    }

    /// Rebind preparation policy at runtime. The supplied limits replace the
    /// serialized compile-time defaults after the proven image/table
    /// dimensions and dispatch integer bounds are checked.
    pub fn prepare_with_limits(
        &self,
        limits: PrefixLimits,
    ) -> Result<PrefixPreparation, KernelArtifactError> {
        self.validate_runtime_limits(limits)?;
        let mut prepared = self.build_preparation()?;
        prepared.set_limits(limits);
        Ok(prepared)
    }

    fn build_preparation(&self) -> Result<PrefixPreparation, KernelArtifactError> {
        let template_index = SelectorIndex::from_handler_indices(
            self.payload
                .templates
                .handlers
                .iter()
                .map(|handler| handler.handler_index),
        );
        let operand_index = SelectorIndex::from_handler_indices(
            self.payload
                .operands
                .handlers
                .iter()
                .map(|handler| handler.handler_index),
        );
        let templates = self.payload.templates.clone();
        let operands = self.payload.operands.to_report(&templates)?;
        Ok(PrefixPreparation {
            source_hash: self.source_hash.clone(),
            artifact_source_hash: self.artifact_source_hash.clone(),
            initial_image: self.payload.bytecode.clone(),
            table: self.payload.dispatch_cells.clone(),
            patch_handler: PatchHandlerProof {
                selector: self.payload.patch_selector,
                span: self.payload.patch_span.clone(),
                operand_width: self.payload.patch_operand_width,
            },
            templates,
            operands,
            template_index,
            operand_index,
            limits: self.payload.limits,
            bytecode_span: self.payload.bytecode_span.clone(),
            run_loop: RunLoopProof::runtime_only(
                self.payload.run_loop_span.clone(),
                self.payload.completion_register.clone(),
                self.payload.exception_stack_field.clone(),
                self.payload.exception_helper.clone(),
            ),
            operation_table: self.payload.templates.table.clone(),
            constant_tables: self
                .payload
                .constant_tables
                .iter()
                .map(|(name, values)| (name.clone(), Arc::<[String]>::from(values.clone())))
                .collect(),
            number_tables: self
                .payload
                .number_tables
                .iter()
                .map(|(name, values)| {
                    (
                        name.clone(),
                        values.iter().copied().map(f64::from_bits).collect(),
                    )
                })
                .collect(),
            string_decoder: self
                .payload
                .string_fold_plan
                .as_ref()
                .and_then(crate::vm_string::VmStringDecoder::from_plan)
                .map(Arc::new),
            wasm: self.payload.wasm.clone(),
            junk_entries: Default::default(),
        })
    }

    fn validate_runtime_limits(&self, limits: PrefixLimits) -> Result<(), KernelArtifactError> {
        let cells = self
            .payload
            .dispatch_cells
            .iter()
            .map(Vec::len)
            .sum::<usize>();
        if self.payload.bytecode.len() > limits.max_image_bytes {
            return Err(KernelArtifactError::Invalid(format!(
                "artifact image needs {} bytes but caller allows {}",
                self.payload.bytecode.len(),
                limits.max_image_bytes
            )));
        }
        if cells > limits.max_table_cells {
            return Err(KernelArtifactError::Invalid(format!(
                "artifact dispatch table needs {cells} cells but caller allows {}",
                limits.max_table_cells
            )));
        }
        for (state, row) in self.payload.dispatch_cells.iter().enumerate() {
            for (opcode, cell) in row.iter().enumerate() {
                if cell.next_state > limits.max_integer || cell.selector > limits.max_integer {
                    return Err(KernelArtifactError::Invalid(format!(
                        "dispatch cell [{state}][{opcode}] exceeds the configured integer bound {}",
                        limits.max_integer
                    )));
                }
            }
        }
        if limits.max_dispatch_steps == 0 || limits.max_integer == 0 {
            return Err(KernelArtifactError::Invalid(
                "runtime prefix limits must be nonzero".into(),
            ));
        }
        Ok(())
    }

    fn install_actual_facts(&mut self, report: &PipelineReport) {
        let Ok(properties) = crate::kernel::properties::extract_actual_entry_facts(&report.output)
        else {
            return;
        };
        let (entry, initial_slots, descriptor_index) = actual_entry_facts(&properties);
        self.payload.default_entry = entry;
        self.payload.entry_initial_slots = initial_slots;

        let mut records = Vec::new();
        if let Some(named) = properties.thread_descriptors.first() {
            if let PropertyValue::Array(rows) = &named.value {
                for row in rows {
                    match descriptor_from_value(row) {
                        Ok(descriptor) => records.push(DescriptorRecord {
                            descriptor: Some(descriptor),
                            entry: None,
                            invalid: None,
                        }),
                        Err(error) => records.push(DescriptorRecord {
                            descriptor: None,
                            entry: None,
                            invalid: Some(error),
                        }),
                    }
                }
            }
        }
        if let Some((index, entry)) = descriptor_index {
            if let Some(record) = records.get_mut(index as usize) {
                record.entry = Some(entry);
            }
        }
        self.payload.thread_descriptors = records
            .iter()
            .filter_map(|record| record.descriptor.clone())
            .collect();
        self.payload.descriptor_records = records;
    }

    /// Bind the artifact's prebuilt runtime to a caller-owned thread. This
    /// method has no source or report parameter by design.
    pub fn instantiate<'a>(
        &'a self,
        entry: DispatchStart,
        descriptor: ThreadDescriptor,
        inputs: crate::ThreadInputs,
    ) -> Result<crate::PrefixThreadInvocation<'a>, crate::ThreadStartupError> {
        let preparation = self
            .prepared
            .as_deref()
            .ok_or(crate::ThreadStartupError::Context(
                crate::TraceContextError {
                    what: "artifact preparation",
                    limit: 0,
                },
            ))?;
        preparation.thread(entry, descriptor, inputs)
    }

    fn wire_payload(&self) -> WirePayload<'_> {
        let bytecode = encode_base64(&self.payload.bytecode);
        WirePayload {
            bytecode,
            dispatch_cells: &self.payload.dispatch_cells,
            templates: &self.payload.templates,
            operands: WireOperandsRef(&self.payload.operands),
            patch_selector: self.payload.patch_selector,
            patch_span: &self.payload.patch_span,
            patch_operand_width: self.payload.patch_operand_width,
            bytecode_span: &self.payload.bytecode_span,
            run_loop_span: &self.payload.run_loop_span,
            completion_register: &self.payload.completion_register,
            exception_stack_field: &self.payload.exception_stack_field,
            exception_helper: &self.payload.exception_helper,
            constant_tables: &self.payload.constant_tables,
            limits: self.payload.limits,
            default_entry: self.payload.default_entry,
            entry_initial_slots: &self.payload.entry_initial_slots,
            descriptor_records: &self.payload.descriptor_records,
            initial_slot_inputs: &self.payload.initial_slot_inputs,
            string_fold_plan: &self.payload.string_fold_plan,
            number_tables: &self.payload.number_tables,
            native_functions: &self.payload.native_functions,
            wasm: &self.payload.wasm,
        }
    }

    fn encode_payload(&self) -> Result<String, KernelArtifactError> {
        encode_wire_payload(&self.wire_payload())
    }

    fn payload_digest(&self) -> Result<String, KernelArtifactError> {
        let payload = serde_json::to_string(&self.wire_payload())
            .map_err(|error| KernelArtifactError::InvalidJson(error.to_string()))?;
        self.payload_digest_for_payload(&payload)
    }

    fn payload_digest_for_payload(&self, payload: &str) -> Result<String, KernelArtifactError> {
        let wire = serde_json::to_vec(&IntegrityMaterial {
            format: &self.format,
            compiler_provenance: &self.compiler_provenance,
            source_hash: &self.source_hash,
            artifact_source_hash: &self.artifact_source_hash,
            source_basis: &self.source_basis,
            standardized_length: self.standardized_length,
            payload,
        })
        .map_err(|error| KernelArtifactError::InvalidJson(error.to_string()))?;
        Ok(hex_digest(&wire))
    }

    fn validate(&self) -> Result<(), KernelArtifactError> {
        if self.format != ARTIFACT_FORMAT {
            return Err(KernelArtifactError::WrongFormat(self.format.clone()));
        }
        if let Some(plan) = &self.payload.string_fold_plan {
            plan.validate().map_err(KernelArtifactError::Invalid)?;
        }
        if self.payload.bytecode.len() > MAX_IMAGE_BYTES {
            return Err(KernelArtifactError::Invalid(
                "bytecode exceeds image bound".into(),
            ));
        }
        let cells = self
            .payload
            .dispatch_cells
            .iter()
            .try_fold(0usize, |count, row| count.checked_add(row.len()))
            .ok_or_else(|| KernelArtifactError::Invalid("dispatch cell count overflow".into()))?;
        if cells > MAX_TABLE_CELLS || self.payload.dispatch_cells.iter().any(Vec::is_empty) {
            return Err(KernelArtifactError::Invalid(
                "dispatch table is empty or too large".into(),
            ));
        }
        for (state, row) in self.payload.dispatch_cells.iter().enumerate() {
            for cell in row {
                if cell.next_state as usize >= self.payload.dispatch_cells.len() {
                    return Err(KernelArtifactError::Invalid(format!(
                        "cell in state {state} points outside dispatch table"
                    )));
                }
            }
        }
        if self.payload.templates.handlers.len() > MAX_HANDLERS
            || self.payload.operands.handlers.len() > MAX_HANDLERS
        {
            return Err(KernelArtifactError::Invalid(
                "handler count exceeds bound".into(),
            ));
        }
        if self.payload.templates.table != self.payload.operands.table {
            return Err(KernelArtifactError::Invalid(
                "template and operand table identities differ".into(),
            ));
        }
        if self.payload.patch_operand_width != super::PATCH_OPERAND_WIDTH {
            return Err(KernelArtifactError::Invalid(format!(
                "invalid patch operand width {}; expected {}",
                self.payload.patch_operand_width,
                super::PATCH_OPERAND_WIDTH
            )));
        }
        if !matches!(
            self.source_basis.as_str(),
            "raw_source_observed" | "declared_only"
        ) {
            return Err(KernelArtifactError::Invalid(
                "invalid source identity basis".into(),
            ));
        }
        if self.standardized_length == 0 {
            return Err(KernelArtifactError::Invalid(
                "standardized identity length is missing".into(),
            ));
        }
        if self.standardized_length > u64::from(u32::MAX)
            || !is_lower_hex(&self.source_hash, 16)
            || !is_lower_hex(&self.artifact_source_hash, 16)
        {
            return Err(KernelArtifactError::Invalid(
                "invalid source or standardized identity header".into(),
            ));
        }
        if self
            .payload
            .descriptor_records
            .iter()
            .any(|record| record.descriptor.is_none() == record.invalid.is_none())
        {
            return Err(KernelArtifactError::Invalid(
                "descriptor record must be valid or typed-invalid".into(),
            ));
        }
        let bounds = RecipeBounds::default();
        for template in &self.payload.templates.handlers {
            validate_span_bounds(
                template.source_span.start,
                template.source_span.end,
                self.standardized_length,
            )?;
            if template.status == TemplateStatus::Executable {
                admit_recipe(template, bounds)
                    .map_err(|error| KernelArtifactError::Invalid(error.to_string()))?;
            }
            if template
                .unsupported
                .iter()
                .any(|fragment| fragment.source.len() > 0)
            {
                return Err(KernelArtifactError::Invalid(
                    "source fragment present in artifact".into(),
                ));
            }
            for step in &template.steps {
                validate_span_bounds(step.span.start, step.span.end, self.standardized_length)?;
                if let RecipeOperation::Unknown { source, .. } = &step.operation {
                    if !source.is_empty() {
                        return Err(KernelArtifactError::Invalid(
                            "unknown recipe source fragment is not allowed".into(),
                        ));
                    }
                }
                for value in step.operation.values() {
                    if contains_source(value) {
                        return Err(KernelArtifactError::Invalid(
                            "function/source-bearing recipe value is not allowed".into(),
                        ));
                    }
                    validate_source_free_value(value, 0)?;
                }
            }
        }
        validate_span(&self.payload.patch_span, self.standardized_length)?;
        validate_span(&self.payload.bytecode_span, self.standardized_length)?;
        validate_span(&self.payload.run_loop_span, self.standardized_length)?;
        if let Some(span) = &self.payload.exception_helper {
            validate_span(span, self.standardized_length)?;
        }
        for handler in &self.payload.operands.handlers {
            validate_span(&handler.source_span, self.standardized_length)?;
            for operand in &handler.operands {
                for span in &operand.source_spans {
                    validate_span(span, self.standardized_length)?;
                }
            }
            for dependency in &handler.captured_dependencies {
                validate_span(&dependency.declaration_span, self.standardized_length)?;
            }
        }
        if let Some(entry) = self.payload.default_entry {
            if entry.pc >= self.payload.bytecode.len()
                || entry.state >= self.payload.dispatch_cells.len()
            {
                return Err(KernelArtifactError::Invalid(
                    "default entry is out of bounds".into(),
                ));
            }
        }
        if self.payload.entry_initial_slots.len() > MAX_DESCRIPTOR_SLOTS
            || self.payload.descriptor_records.len() > MAX_DESCRIPTOR_ROWS
        {
            return Err(KernelArtifactError::Invalid(
                "descriptor facts exceed bound".into(),
            ));
        }
        let valid_records = self
            .payload
            .descriptor_records
            .iter()
            .filter(|record| record.descriptor.is_some())
            .count();
        if !self.payload.descriptor_records.is_empty()
            && valid_records != self.payload.thread_descriptors.len()
        {
            return Err(KernelArtifactError::Invalid(
                "descriptor record projection is inconsistent".into(),
            ));
        }
        for record in &self.payload.descriptor_records {
            if let Some(descriptor) = &record.descriptor {
                let count = descriptor
                    .initial_slots
                    .len()
                    .saturating_add(descriptor.captured_slots.len())
                    .saturating_add(descriptor.argument_slots.len());
                if count > MAX_DESCRIPTOR_SLOTS {
                    return Err(KernelArtifactError::Invalid(
                        "descriptor slot facts exceed bound".into(),
                    ));
                }
            }
            if let Some(entry) = record.entry {
                if entry.pc >= self.payload.bytecode.len()
                    || entry.state >= self.payload.dispatch_cells.len()
                {
                    return Err(KernelArtifactError::Invalid(
                        "descriptor entry is out of bounds".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

impl ArtifactOperands {
    fn to_report(
        &self,
        templates: &HandlerTemplateReport,
    ) -> Result<JsOperandSeparationReport, KernelArtifactError> {
        let mut by_index = BTreeMap::new();
        for template in &templates.handlers {
            by_index.insert(template.handler_index, template.clone());
        }
        let handlers = self
            .handlers
            .iter()
            .map(|handler| {
                let template = by_index
                    .get(&handler.handler_index)
                    .cloned()
                    .ok_or_else(|| {
                        KernelArtifactError::Invalid(format!(
                            "operand handler {} has no recipe",
                            handler.handler_index
                        ))
                    })?;
                Ok(LiftedHandler {
                    handler_index: handler.handler_index,
                    table_key: handler.table_key,
                    source_span: handler.source_span.clone(),
                    entry_coordinate: handler.entry_coordinate,
                    operands: handler.operands.clone(),
                    pc_delta: handler.pc_delta,
                    captured_dependencies: handler.captured_dependencies.clone(),
                    template,
                    status: handler.status.clone(),
                    unresolved: handler.unresolved.clone(),
                })
            })
            .collect::<Result<Vec<_>, KernelArtifactError>>()?;
        Ok(JsOperandSeparationReport {
            schema_version: self.schema_version.clone(),
            source_hash: self.source_hash.clone(),
            table: self.table.clone(),
            input_identity: self.input_identity.clone(),
            entry_coordinate: self.entry_coordinate,
            handlers,
            coverage: self.coverage.clone(),
            unresolved: self.unresolved.clone(),
        })
    }
}

fn sanitize_template(template: &mut HandlerTemplate) -> Result<(), String> {
    for fragment in &mut template.unsupported {
        fragment.source.clear();
    }
    for step in &mut template.steps {
        if let RecipeOperation::Unknown { source, .. } = &mut step.operation {
            source.clear();
        }
        sanitize_operation(&mut step.operation)?;
    }
    Ok(())
}

fn sanitize_operation(operation: &mut RecipeOperation) -> Result<(), String> {
    for value in operation.values_mut() {
        sanitize_value(value)?;
    }
    Ok(())
}

fn sanitize_value(value: &mut SemanticValue) -> Result<(), String> {
    if let SemanticValue::FunctionLiteral { source } = value {
        let plan = compile_closure_plan(source)?;
        *value = SemanticValue::ClosurePlan { plan };
        return Ok(());
    }
    for child in value.children_mut() {
        sanitize_value(child)?;
    }
    Ok(())
}

fn contains_source(value: &SemanticValue) -> bool {
    if matches!(value, SemanticValue::FunctionLiteral { .. }) {
        return true;
    }
    value.children().into_iter().any(contains_source)
}

fn validate_source_free_value(
    value: &SemanticValue,
    depth: usize,
) -> Result<(), KernelArtifactError> {
    if depth > 256 {
        return Err(KernelArtifactError::Invalid(
            "closure/value depth exceeds bound".into(),
        ));
    }
    if let SemanticValue::ClosurePlan { plan } = value {
        if plan.params.len() > 32 || plan.body.len() > 64 {
            return Err(KernelArtifactError::Invalid(
                "closure plan exceeds bound".into(),
            ));
        }
        let mut parameters = BTreeSet::new();
        for parameter in &plan.params {
            if !is_valid_closure_parameter(parameter) {
                return Err(KernelArtifactError::Invalid(
                    "closure plan contains an invalid parameter".into(),
                ));
            }
            if !parameters.insert(parameter) {
                return Err(KernelArtifactError::Invalid(
                    "closure plan contains duplicate parameters".into(),
                ));
            }
        }
        for statement in &plan.body {
            match statement {
                ClosureStatement::Expression(expression)
                | ClosureStatement::Return(Some(expression)) => {
                    let parameter = match expression {
                        ClosureExpression::CharCodeAt { parameter }
                        | ClosureExpression::AddOne { parameter } => Some(parameter),
                        ClosureExpression::NullIndexCall | ClosureExpression::External { .. } => {
                            None
                        }
                    };
                    if parameter.is_some_and(|parameter| !parameters.contains(parameter)) {
                        return Err(KernelArtifactError::Invalid(
                            "closure plan references an unknown parameter".into(),
                        ));
                    }
                    if let ClosureExpression::External { name } = expression {
                        if !is_valid_closure_identifier(name) {
                            return Err(KernelArtifactError::Invalid(
                                "closure plan contains an invalid external name".into(),
                            ));
                        }
                    }
                }
                ClosureStatement::Return(None) => {}
            }
        }
    }
    for child in value.children() {
        validate_source_free_value(child, depth + 1)?;
    }
    Ok(())
}

fn is_valid_closure_parameter(parameter: &str) -> bool {
    let mut characters = parameter.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if parameter.len() > 128
        || !(first == '_' || first == '$' || first.is_ascii_alphabetic())
        || !characters.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
    {
        return false;
    }
    !matches!(
        parameter,
        "arguments"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "eval"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

fn max_span_end(preparation: &PrefixPreparation) -> u64 {
    let mut end = preparation.bytecode_span.end as u64;
    end = end.max(preparation.run_loop.span.end as u64);
    end = end.max(preparation.patch_handler.span.end as u64);
    for handler in &preparation.templates.handlers {
        end = end.max(handler.source_span.end as u64);
        for step in &handler.steps {
            end = end.max(step.span.end as u64);
        }
    }
    for handler in &preparation.operands.handlers {
        end = end.max(handler.source_span.end as u64);
        for operand in &handler.operands {
            for span in &operand.source_spans {
                end = end.max(span.end as u64);
            }
        }
        for dependency in &handler.captured_dependencies {
            end = end.max(dependency.declaration_span.end as u64);
        }
    }
    end.max(1)
}

fn actual_entry_facts(
    properties: &ActualProperties,
) -> (
    Option<DispatchStart>,
    Vec<u32>,
    Option<(u32, DispatchStart)>,
) {
    // The outer entry invocation is the program's root. A worker bootstrap
    // (`vmEntry` run again inside a Worker the program starts) is the entry
    // only when there is no outer one; it stays its descriptor's entry.
    let outer = outer_entry_facts(properties);
    let mut worker = None;
    for named in &properties.additional {
        let PropertyValue::Object(fields) = &named.value else {
            continue;
        };
        let (Some(pc), Some(state)) = (
            fields.get("entryPc").and_then(property_u32),
            fields.get("state").and_then(property_u32),
        ) else {
            continue;
        };
        let slots = match fields.get("initialSlots") {
            Some(value) => {
                let Some(slots) = property_slots(value) else {
                    continue;
                };
                slots
            }
            None => Vec::new(),
        };
        let descriptor = fields.get("descriptorIndex").and_then(property_u32);
        worker = Some((
            DispatchStart {
                pc: pc as usize,
                state: state as usize,
            },
            slots,
            descriptor,
        ));
        break;
    }
    // The worker's descriptor records where the worker starts.
    let worker_descriptor = worker
        .as_ref()
        .and_then(|(entry, _, descriptor)| descriptor.map(|index| (index, *entry)));
    match (outer, worker) {
        (Some((entry, slots)), _) => (Some(entry), slots, worker_descriptor),
        (None, Some((entry, slots, _))) => (Some(entry), slots, worker_descriptor),
        (None, None) => (None, Vec::new(), None),
    }
}

/// The outer `vmEntry` invocation's entry and initial slots.
fn outer_entry_facts(properties: &ActualProperties) -> Option<(DispatchStart, Vec<u32>)> {
    let entry = properties.entry.as_ref()?;
    let pc = entry.arguments.get(1).and_then(property_u32)?;
    let state = entry.arguments.get(2).and_then(property_u32)?;
    let slots = match entry.arguments.get(3) {
        Some(value) => property_slots(value)?,
        None => Vec::new(),
    };
    Some((
        DispatchStart {
            pc: pc as usize,
            state: state as usize,
        },
        slots,
    ))
}

fn property_u32(value: &PropertyValue) -> Option<u32> {
    match value {
        PropertyValue::Number(number)
            if number.is_finite()
                && number.fract() == 0.0
                && *number >= 0.0
                && *number <= f64::from(u32::MAX) =>
        {
            Some(*number as u32)
        }
        _ => None,
    }
}

fn property_slots(value: &PropertyValue) -> Option<Vec<u32>> {
    let PropertyValue::Array(values) = value else {
        return None;
    };
    values.iter().map(property_u32).collect()
}

fn descriptor_from_value(value: &PropertyValue) -> Result<ThreadDescriptor, String> {
    let PropertyValue::Object(fields) = value else {
        return Err("the row is not an object".into());
    };
    let list = |name: &str| match fields.get(name) {
        None => Ok(Vec::new()),
        Some(value) => {
            property_slots(value).ok_or_else(|| format!("{name} is not a list of slots"))
        }
    };
    let single = |name: &str| match fields.get(name) {
        None => Ok(None),
        Some(value) => property_u32(value)
            .map(Some)
            .ok_or_else(|| format!("{name} is not a slot")),
    };
    Ok(ThreadDescriptor {
        initial_slots: list("initialSlots")?,
        captured_slots: list("capturedSlots")?,
        argument_slots: list("argumentSlots")?,
        arguments_slot: single("argumentsSlot")?,
        result_slot: single("resultSlot")?,
        ..ThreadDescriptor::default()
    })
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_span(span: &OperandSpan, length: u64) -> Result<(), KernelArtifactError> {
    validate_span_bounds(span.start, span.end, length)
}

fn validate_span_bounds(start: u32, end: u32, length: u64) -> Result<(), KernelArtifactError> {
    if start > end || u64::from(end) > length {
        return Err(KernelArtifactError::Invalid(format!(
            "span [{start}, {end}) is outside standardized identity length {length}"
        )));
    }
    Ok(())
}

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn decode_base64(value: &str) -> Result<Vec<u8>, KernelArtifactError> {
    decode_base64_with_limit(value, MAX_IMAGE_BYTES, "invalid compact bytecode encoding")
}

fn decode_base64_with_limit(
    value: &str,
    max_bytes: usize,
    invalid_message: &str,
) -> Result<Vec<u8>, KernelArtifactError> {
    if value.len() > max_bytes.div_ceil(3).saturating_mul(4) || value.len() % 4 != 0 {
        return Err(KernelArtifactError::Invalid(invalid_message.into()));
    }
    let mut output = Vec::with_capacity(value.len() / 4 * 3);
    let decode = |byte: u8| -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    for chunk in value.as_bytes().chunks(4) {
        let a = decode(chunk[0])
            .ok_or_else(|| KernelArtifactError::Invalid("invalid base64 bytecode".into()))?;
        let b = decode(chunk[1])
            .ok_or_else(|| KernelArtifactError::Invalid("invalid base64 bytecode".into()))?;
        let c = if chunk[2] == b'=' {
            0
        } else {
            decode(chunk[2])
                .ok_or_else(|| KernelArtifactError::Invalid("invalid base64 bytecode".into()))?
        };
        let d = if chunk[3] == b'=' {
            0
        } else {
            decode(chunk[3])
                .ok_or_else(|| KernelArtifactError::Invalid("invalid base64 bytecode".into()))?
        };
        output.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            output.push((b << 4) | (c >> 2));
        }
        if chunk[3] != b'=' {
            output.push((c << 6) | d);
        }
    }
    if output.len() > max_bytes {
        return Err(KernelArtifactError::Invalid(invalid_message.into()));
    }
    Ok(output)
}

impl RunLoopProof {
    pub(super) fn runtime_only(
        span: OperandSpan,
        completion_register: String,
        exception_stack_field: String,
        exception_helper: Option<OperandSpan>,
    ) -> Self {
        Self {
            span,
            function_symbol: None,
            completion_register,
            completion_register_symbol: None,
            completion_sentinel_symbol: None,
            exception_stack_field,
            exception_helper,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::{ClosurePlan, TemplateCoverage};

    fn tiny_preparation() -> PrefixPreparation {
        let table = ProvenOperationTable {
            declaration_ordinal: 0,
            original_name: "ops".into(),
            canonical_name: "ops".into(),
            initializer_elements: 1,
            initializer_functions: 0,
        };
        let templates = HandlerTemplateReport {
            schema_version: "test".into(),
            source_hash: "2222222222222222".into(),
            table: table.clone(),
            handlers: Vec::new(),
            coverage: TemplateCoverage::default(),
        };
        let operands = JsOperandSeparationReport {
            schema_version: "test".into(),
            source_hash: "2222222222222222".into(),
            table: table.clone(),
            input_identity: None,
            entry_coordinate: None,
            handlers: Vec::new(),
            coverage: OperandCoverage::default(),
            unresolved: Vec::new(),
        };
        PrefixPreparation {
            source_hash: "0000000000000000".into(),
            artifact_source_hash: "1111111111111111".into(),
            initial_image: vec![0],
            table: vec![vec![DispatchCell {
                next_state: 0,
                selector: 1,
            }]],
            patch_handler: PatchHandlerProof {
                selector: 9,
                span: OperandSpan { start: 2, end: 3 },
                operand_width: 4,
            },
            templates,
            operands,
            template_index: SelectorIndex::from_handler_indices([]),
            operand_index: SelectorIndex::from_handler_indices([]),
            limits: PrefixLimits::default(),
            bytecode_span: OperandSpan { start: 4, end: 5 },
            run_loop: RunLoopProof::runtime_only(
                OperandSpan { start: 6, end: 7 },
                "pending".into(),
                "errors".into(),
                None,
            ),
            operation_table: table,
            constant_tables: vec![("strings".into(), Arc::<[String]>::from(vec!["ok".into()]))],
            number_tables: Vec::new(),
            string_decoder: None,
            wasm: None,
            junk_entries: Default::default(),
        }
    }

    #[test]
    fn source_free_artifact_round_trips_and_reconstructs_indexes() {
        let artifact = KernelArtifact::from_preparation(&tiny_preparation()).unwrap();
        let json = artifact.to_json().unwrap();
        assert!(json.contains("\"payload\":\""));
        assert!(!json.contains("\"bytecode\""));
        assert!(!json.contains("function_symbol"));
        let loaded = KernelArtifact::from_json(&json).unwrap();
        let prepared = loaded.prepare().unwrap();
        assert_eq!(prepared.initial_image(), &[0]);
        assert_eq!(prepared.table()[0][0].selector, 1);
        assert_eq!(loaded.default_entry(), None);
    }

    #[test]
    fn wire_mutation_fails_integrity_check() {
        let artifact = KernelArtifact::from_preparation(&tiny_preparation()).unwrap();
        let json = artifact.to_json().unwrap().replace(
            "\"source_hash\":\"0000000000000000\"",
            "\"source_hash\":\"1111111111111111\"",
        );
        assert!(matches!(
            KernelArtifact::from_json(&json),
            Err(KernelArtifactError::IntegrityMismatch { .. })
        ));
    }

    #[test]
    fn runtime_integer_limit_matches_source_admission() {
        let mut preparation = tiny_preparation();
        preparation.table[0][0].selector = 2;
        let artifact = KernelArtifact::from_preparation(&preparation).unwrap();
        let limits = PrefixLimits {
            max_integer: 1,
            ..PrefixLimits::default()
        };
        let error = artifact.prepare_with_limits(limits).unwrap_err();
        assert!(error.to_string().contains("dispatch cell [0][0]"));
        assert!(error.to_string().contains("integer bound 1"));
    }

    #[test]
    fn unknown_wire_fields_fail_closed() {
        let artifact = KernelArtifact::from_preparation(&tiny_preparation()).unwrap();
        let json = artifact.to_json().unwrap().replacen(
            "{\"format\"",
            "{\"unexpected\":true,\"format\"",
            1,
        );
        assert!(matches!(
            KernelArtifact::from_json(&json),
            Err(KernelArtifactError::InvalidJson(_))
        ));
    }

    #[test]
    fn malformed_compressed_payload_fails_closed() {
        let artifact = KernelArtifact::from_preparation(&tiny_preparation()).unwrap();
        let mut wire: serde_json::Value =
            serde_json::from_str(&artifact.to_json().unwrap()).unwrap();
        wire["payload"] = serde_json::Value::String("not-base64".into());
        let json = serde_json::to_string(&wire).unwrap();
        assert!(matches!(
            KernelArtifact::from_json(&json),
            Err(KernelArtifactError::Invalid(_))
        ));
    }

    #[test]
    fn injected_descriptor_projection_is_rejected() {
        let artifact = KernelArtifact::from_preparation(&tiny_preparation()).unwrap();
        let mut wire: serde_json::Value =
            serde_json::from_str(&artifact.to_json().unwrap()).unwrap();
        let encoded = wire["payload"].as_str().unwrap();
        let compressed = decode_base64_with_limit(
            encoded,
            MAX_COMPRESSED_PAYLOAD_BYTES,
            "invalid compressed payload encoding",
        )
        .unwrap();
        let mut payload: serde_json::Value =
            serde_json::from_slice(&zstd::stream::decode_all(Cursor::new(compressed)).unwrap())
                .unwrap();
        payload["thread_descriptors"] = serde_json::json!([{
            "initial_slots": [999],
            "captured_slots": [],
            "argument_slots": []
        }]);
        let payload_json = serde_json::to_vec(&payload).unwrap();
        let recompressed = zstd::stream::encode_all(Cursor::new(payload_json), 1).unwrap();
        wire["payload"] = serde_json::Value::String(encode_base64(&recompressed));
        let error = KernelArtifact::from_json(&serde_json::to_string(&wire).unwrap()).unwrap_err();
        assert!(matches!(error, KernelArtifactError::InvalidJson(_)));
    }

    #[test]
    fn integrity_does_not_depend_on_compressed_byte_identity() {
        let artifact = KernelArtifact::from_preparation(&tiny_preparation()).unwrap();
        let mut wire: serde_json::Value =
            serde_json::from_str(&artifact.to_json().unwrap()).unwrap();
        let compressed = decode_base64_with_limit(
            wire["payload"].as_str().unwrap(),
            MAX_COMPRESSED_PAYLOAD_BYTES,
            "invalid compressed payload encoding",
        )
        .unwrap();
        let payload = zstd::stream::decode_all(Cursor::new(compressed)).unwrap();
        let recompressed = zstd::stream::encode_all(Cursor::new(payload), 1).unwrap();
        wire["payload"] = serde_json::Value::String(encode_base64(&recompressed));
        assert!(KernelArtifact::from_json(&serde_json::to_string(&wire).unwrap()).is_ok());
    }

    #[test]
    fn oversized_zstd_window_fails_before_payload_decode() {
        let mut encoder = zstd::stream::Encoder::new(Vec::new(), 1).unwrap();
        encoder.window_log(MAX_ZSTD_WINDOW_LOG + 1).unwrap();
        encoder.write_all(b"{}").unwrap();
        let compressed = encoder.finish().unwrap();
        let error = decode_payload(&encode_base64(&compressed)).unwrap_err();
        assert!(
            matches!(error, KernelArtifactError::Invalid(message) if message.contains("memory"))
        );
    }

    #[test]
    fn closure_plan_names_are_strict_and_references_are_bound() {
        let valid = SemanticValue::ClosurePlan {
            plan: ClosurePlan {
                params: vec!["sourceString".into()],
                body: vec![ClosureStatement::Return(Some(
                    ClosureExpression::CharCodeAt {
                        parameter: "sourceString".into(),
                    },
                ))],
            },
        };
        assert!(validate_source_free_value(&valid, 0).is_ok());

        for parameter in ["1leading", "eval", "arguments"] {
            let value = SemanticValue::ClosurePlan {
                plan: ClosurePlan {
                    params: vec![parameter.into()],
                    body: vec![ClosureStatement::Return(None)],
                },
            };
            assert!(
                validate_source_free_value(&value, 0).is_err(),
                "{parameter}"
            );
        }

        let duplicate = SemanticValue::ClosurePlan {
            plan: ClosurePlan {
                params: vec!["value".into(), "value".into()],
                body: vec![ClosureStatement::Return(None)],
            },
        };
        assert!(validate_source_free_value(&duplicate, 0).is_err());

        let unknown_reference = SemanticValue::ClosurePlan {
            plan: ClosurePlan {
                params: vec!["value".into()],
                body: vec![ClosureStatement::Return(Some(ClosureExpression::AddOne {
                    parameter: "missing".into(),
                }))],
            },
        };
        assert!(validate_source_free_value(&unknown_reference, 0).is_err());

        let invalid_external = SemanticValue::ClosurePlan {
            plan: ClosurePlan {
                params: Vec::new(),
                body: vec![ClosureStatement::Return(Some(
                    ClosureExpression::External {
                        name: "bad-name".into(),
                    },
                ))],
            },
        };
        assert!(validate_source_free_value(&invalid_external, 0).is_err());
    }

    #[test]
    fn reports_wire_size_and_five_warm_loads() {
        let artifact = KernelArtifact::from_preparation(&tiny_preparation()).unwrap();
        let json = artifact.to_json().unwrap();
        let load_start = std::time::Instant::now();
        for _ in 0..5 {
            KernelArtifact::from_json(&json).unwrap();
        }
        let load_elapsed = load_start.elapsed();
        let fresh_start = std::time::Instant::now();
        for _ in 0..5 {
            tiny_preparation().export_artifact().unwrap();
        }
        let fresh_elapsed = fresh_start.elapsed();
        println!(
            "kernel-artifact-size total={} bytecode={} templates={} load_5={:?} fresh_5={:?}",
            json.len(),
            artifact.bytecode().len(),
            artifact.templates().handlers.len(),
            load_elapsed,
            fresh_elapsed
        );
    }

    #[test]
    fn deobfuscated_compile_matches_the_raw_runtime_payload() {
        let source = include_str!("../../../tests/fixtures/kernels/javascript/kernel.raw.js");
        let report = crate::deobfuscate_with_report(source).expect("raw report");
        let limits = PrefixLimits::default();
        let raw = KernelArtifact::compile_from_raw(source, &report, limits).expect("raw compile");
        let deob = KernelArtifact::compile_from_deobfuscated(&report.output, limits)
            .expect("deobfuscated compile");

        assert_eq!(raw.payload.bytecode, deob.payload.bytecode);
        assert_eq!(raw.payload.dispatch_cells, deob.payload.dispatch_cells);
        assert_eq!(
            raw.payload.templates.handlers,
            deob.payload.templates.handlers
        );
        assert_eq!(
            raw.payload.templates.coverage,
            deob.payload.templates.coverage
        );
        assert_eq!(
            raw.payload.operands.handlers,
            deob.payload.operands.handlers
        );
        assert_eq!(
            raw.payload.operands.coverage,
            deob.payload.operands.coverage
        );
        assert_eq!(
            raw.payload.operands.unresolved,
            deob.payload.operands.unresolved
        );
        assert_eq!(
            raw.payload.operands.entry_coordinate,
            deob.payload.operands.entry_coordinate
        );
        assert_eq!(
            raw.payload.operands.input_identity,
            deob.payload.operands.input_identity
        );
        assert_eq!(
            raw.payload.operands.table.canonical_name,
            deob.payload.operands.table.canonical_name
        );
        assert_eq!(
            raw.payload.operands.table.declaration_ordinal,
            deob.payload.operands.table.declaration_ordinal
        );
        assert_eq!(
            raw.payload.operands.table.initializer_elements,
            deob.payload.operands.table.initializer_elements
        );
        assert_eq!(
            raw.payload.operands.table.initializer_functions,
            deob.payload.operands.table.initializer_functions
        );
        assert_eq!(
            raw.payload.templates.table.canonical_name,
            deob.payload.templates.table.canonical_name
        );
        assert_eq!(
            raw.payload.templates.table.declaration_ordinal,
            deob.payload.templates.table.declaration_ordinal
        );
        assert_eq!(
            raw.payload.templates.table.initializer_elements,
            deob.payload.templates.table.initializer_elements
        );
        assert_eq!(
            raw.payload.templates.table.initializer_functions,
            deob.payload.templates.table.initializer_functions
        );
        assert_eq!(raw.payload.patch_selector, deob.payload.patch_selector);
        assert_eq!(raw.payload.patch_span, deob.payload.patch_span);
        assert_eq!(
            raw.payload.patch_operand_width,
            deob.payload.patch_operand_width
        );
        assert_eq!(raw.payload.bytecode_span, deob.payload.bytecode_span);
        assert_eq!(raw.payload.run_loop_span, deob.payload.run_loop_span);
        assert_eq!(
            raw.payload.completion_register,
            deob.payload.completion_register
        );
        assert_eq!(
            raw.payload.exception_stack_field,
            deob.payload.exception_stack_field
        );
        assert_eq!(raw.payload.exception_helper, deob.payload.exception_helper);
        assert_eq!(raw.payload.constant_tables, deob.payload.constant_tables);
        assert_eq!(raw.payload.limits, deob.payload.limits);
        assert_eq!(raw.payload.default_entry, deob.payload.default_entry);
        assert_eq!(
            raw.payload.entry_initial_slots,
            deob.payload.entry_initial_slots
        );
        assert_eq!(
            raw.payload.initial_slot_inputs,
            deob.payload.initial_slot_inputs
        );
        assert_eq!(
            raw.payload.descriptor_records,
            deob.payload.descriptor_records
        );
        assert_eq!(
            raw.payload.thread_descriptors,
            deob.payload.thread_descriptors
        );
        assert_eq!(raw.payload.string_fold_plan, deob.payload.string_fold_plan);
    }

    #[test]
    fn closure_plan_is_bounded_and_source_free() {
        for (source, emitted) in [
            (
                "function (sourceString) { return sourceString.charCodeAt(); }",
                "function (sourceString) { return sourceString.charCodeAt(); }",
            ),
            (
                "function (x) { return x + 1; }",
                "function (x) { return x + 1; }",
            ),
            (
                "function () { return host; }",
                "function () { return host; }",
            ),
        ] {
            let plan = compile_closure_plan(source).expect("supported closed function");
            assert_eq!(plan.to_source(), emitted);
            let wire = serde_json::to_string(&plan).unwrap();
            assert!(!wire.contains("function ("));
        }
        assert!(compile_closure_plan("function (x) { return eval(x); }").is_err());
        assert!(compile_closure_plan("function (x) { return x + 2; }").is_err());
        assert!(compile_closure_plan("function () { return host.value; }").is_err());
        assert!(compile_closure_plan("async function () { return host; }").is_err());
        assert!(compile_closure_plan("function* () { return host; }").is_err());
        assert!(compile_closure_plan("function named() { return host; }").is_err());
        assert!(compile_closure_plan("function () { \"use strict\"; return host; }").is_err());
        assert!(compile_closure_plan("function () { return\n host; }").is_err());
    }
}

/// The entries of `var vmNativeFunctions = [ … ];` in the deobfuscated
/// kernel, as source text; empty when there is no such array.
fn native_catalogue(deobfuscated: &str) -> Vec<String> {
    use oxc_ast::ast::{BindingPattern, Expression, Statement};
    let allocator = oxc_allocator::Allocator::default();
    let Ok(program) = crate::analysis::parse_program(&allocator, deobfuscated) else {
        return Vec::new();
    };
    // The kernel is one function expression: `(function vmEntry(…) { … })(…)`.
    let body = program.body.iter().find_map(|statement| {
        let Statement::ExpressionStatement(statement) = statement else { return None };
        let Expression::CallExpression(call) = statement.expression.get_inner_expression() else { return None };
        let Expression::FunctionExpression(function) = call.callee.get_inner_expression() else { return None };
        function.body.as_ref()
    });
    for statement in body.map(|body| body.statements.as_slice()).unwrap_or_default() {
        let Statement::VariableDeclaration(declaration) = statement else { continue };
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(id) = &declarator.id else { continue };
            if id.name != "vmNativeFunctions" {
                continue;
            }
            let Some(Expression::ArrayExpression(array)) = declarator.init.as_ref().map(|init| init.get_inner_expression()) else {
                return Vec::new();
            };
            let mut entries = Vec::new();
            for element in &array.elements {
                let Some(expression) = element.as_expression() else {
                    return Vec::new();
                };
                let span = oxc_span::GetSpan::span(expression);
                entries.push(deobfuscated[span.start as usize..span.end as usize].to_string());
            }
            return entries;
        }
    }
    Vec::new()
}
