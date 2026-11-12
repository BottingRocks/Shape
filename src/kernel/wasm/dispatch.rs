//! A bounded, non-executing adapter for the WASM dispatch boundary.
//!
//! This module is deliberately narrower than the JavaScript dispatch driver.
//! It admits only immutable, source-derived WASM module and memory-image facts,
//! resolves one `(pc, state)` cell, and reports the host-import boundary.  It
//! does not execute WASM, invoke copied JavaScript handlers, or claim that a
//! table element is an original operation.  The latter distinction is the
//! important one for the S2/S3 captures: the table edge is proven, while the
//! branch-to-original-handler join is not.

use std::{fmt, sync::Arc};

use crate::{
    kernel::{config, wasm},
    transforms::deob::pipeline::{self, ExtractionKind, ExtractionStatus, PipelineReport},
    util::fnv1a64_hex,
};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: &str = "shape-wasm-dispatch-adapter/v1";

// Exact SHA-256 identities of the checked-in fixture pair.  General
// interpreter/table pairing is intentionally unsupported until exported-W
// dataflow is proven for arbitrary modules.
const S3_HANDLER_SHA256: [u8; 32] = [
    0x8a, 0x3c, 0xe5, 0x97, 0x0d, 0xd1, 0x59, 0x6e, 0x32, 0xee, 0x4c, 0xb6, 0xa9, 0x7e, 0xec, 0xef,
    0xa7, 0xe7, 0xa3, 0x36, 0x92, 0xd6, 0x12, 0x72, 0xc4, 0xf8, 0x6d, 0x7b, 0x74, 0xb9, 0x3d, 0x7b,
];
const S3_INTERPRETER_SHA256: [u8; 32] = [
    0x85, 0x0a, 0x98, 0x6f, 0x71, 0x07, 0xb0, 0x81, 0xd8, 0x3a, 0xdd, 0xe9, 0xe7, 0xa5, 0x38, 0x21,
    0x01, 0x17, 0x0a, 0x67, 0xeb, 0x41, 0xef, 0xe6, 0x92, 0x1a, 0x69, 0x2a, 0x69, 0xd2, 0xa4, 0x07,
];

/// Resource bounds for admission and static resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmDispatchLimits {
    pub max_image_bytes: usize,
    pub max_opcodes: usize,
    pub max_states: usize,
    pub max_table_entries: usize,
}

impl Default for WasmDispatchLimits {
    fn default() -> Self {
        Self {
            max_image_bytes: 8 * 1024 * 1024,
            max_opcodes: 4096,
            max_states: 256,
            max_table_entries: 4096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmDispatchError {
    MissingFact(&'static str),
    AmbiguousFact(&'static str),
    InvalidFact(String),
    Unsupported(String),
    Limit(String),
    EntryOutOfBounds { pc: usize, image_len: usize },
    EntryInDispatchRegion { pc: usize, base: usize },
    StateOutOfBounds { state: usize, state_count: usize },
    OpcodeOutOfBounds { opcode: u8, opcode_count: usize },
    CellOutOfBounds { address: usize, image_len: usize },
}

impl fmt::Display for WasmDispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFact(fact) => write!(f, "missing proven WASM fact: {fact}"),
            Self::AmbiguousFact(fact) => write!(f, "ambiguous proven WASM fact: {fact}"),
            Self::InvalidFact(error) => write!(f, "invalid WASM adapter fact: {error}"),
            Self::Unsupported(error) => write!(f, "unsupported WASM adapter boundary: {error}"),
            Self::Limit(error) => write!(f, "WASM adapter limit: {error}"),
            Self::EntryOutOfBounds { pc, image_len } => {
                write!(f, "entry PC {pc} is outside the {image_len}-byte image")
            }
            Self::EntryInDispatchRegion { pc, base } => {
                write!(
                    f,
                    "entry PC {pc} is inside the dispatch region beginning at {base}"
                )
            }
            Self::StateOutOfBounds { state, state_count } => {
                write!(
                    f,
                    "entry state {state} is outside {state_count} proven states"
                )
            }
            Self::OpcodeOutOfBounds {
                opcode,
                opcode_count,
            } => write!(
                f,
                "opcode {opcode} is outside {opcode_count} proven opcodes"
            ),
            Self::CellOutOfBounds { address, image_len } => write!(
                f,
                "dispatch cell at image address {address} is outside the {image_len}-byte image"
            ),
        }
    }
}

impl std::error::Error for WasmDispatchError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmArtifactIdentity {
    binding: String,
    sha256: [u8; 32],
    byte_len: usize,
    source: Option<wasm::SourceLink>,
}

impl WasmArtifactIdentity {
    #[must_use]
    pub fn binding(&self) -> &str {
        &self.binding
    }

    #[must_use]
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    #[must_use]
    pub fn source(&self) -> Option<&wasm::SourceLink> {
        self.source.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmDispatchImage {
    bytes: Arc<[u8]>,
    identity: WasmArtifactIdentity,
    base: usize,
    stride: usize,
    states: usize,
    opcodes: usize,
}

impl WasmDispatchImage {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn identity(&self) -> &WasmArtifactIdentity {
        &self.identity
    }

    #[must_use]
    pub fn base(&self) -> usize {
        self.base
    }

    #[must_use]
    pub fn stride(&self) -> usize {
        self.stride
    }

    #[must_use]
    pub fn states(&self) -> usize {
        self.states
    }

    #[must_use]
    pub fn opcodes(&self) -> usize {
        self.opcodes
    }
}

/// A table edge is intentionally an import edge, not an operation identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmHostImport {
    pub table_index: u32,
    pub element_index: u32,
    pub function_index: u32,
    pub namespace: String,
    pub name: String,
    pub signature: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmResolution {
    pub pc: usize,
    pub state: usize,
    pub opcode: u8,
    pub opcode_span: (usize, usize),
    pub cell_address: usize,
    pub cell_span: (usize, usize),
    pub selector: u8,
    pub next_state: u8,
    pub host_import: Option<WasmHostImport>,
}

/// A static proof-backed dispatch adapter.  `resolve` is a table lookup only;
/// a returned host import is a required stop before any runtime effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmDispatchProgram {
    interpreter: WasmArtifactIdentity,
    handler_table: WasmArtifactIdentity,
    image: WasmDispatchImage,
    handlers: Vec<Option<WasmHostImport>>,
    synthetic: bool,
}

impl WasmDispatchProgram {
    /// Admit the tracked fixture capture from an already-produced report.
    /// This checks the S3 fingerprint in addition to the generic proof checks;
    /// it never falls back to a name-only or image-tail guess.
    pub fn prepare_s3(source: &str, report: &PipelineReport) -> Result<Self, WasmDispatchError> {
        let program = Self::prepare(source, report, WasmDispatchLimits::default())?;
        if program.image.base != 170_845
            || program.image.stride != 468
            || program.image.opcodes != 234
            || program.handlers.len() != 201
        {
            return Err(WasmDispatchError::InvalidFact(
                "the admitted WASM pair is not the proven S3 geometry (base 170845, stride 468, 234 branches, 201 handlers)".into(),
            ));
        }
        Ok(program)
    }

    /// Admission is deliberately fixture-scoped until a general exported-W
    /// dataflow proof exists.  The exact module digests below commit to all
    /// sections, including local indexes, memory/table indexes, control flow,
    /// and the load offsets used by the verified S3 sequence.  No S2 or
    /// source-supplied lookalike module is admitted by this API.
    pub fn prepare(
        source: &str,
        report: &PipelineReport,
        limits: WasmDispatchLimits,
    ) -> Result<Self, WasmDispatchError> {
        // `config::from_report` is intentionally report-only.  This adapter is
        // the one public API that accepts a raw source plus a supplied report,
        // so authenticate that hand-off locally before reading any supplied
        // module or image bytes.  Reuse the canonical report after the check;
        // this also binds the actual memory image to canonical output.
        let canonical = pipeline::deobfuscate_with_report(source).map_err(|error| {
            WasmDispatchError::InvalidFact(format!(
                "cannot rebuild canonical pipeline report for WASM admission: {error}"
            ))
        })?;
        authenticate_report(&canonical, report)?;
        let runtime =
            config::from_report(source, &canonical, Default::default()).map_err(|error| {
                WasmDispatchError::InvalidFact(format!(
                    "canonical runtime configuration unavailable for WASM admission: {error}"
                ))
            })?;
        let image_fact = runtime
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact.kind == config::ArtifactKind::WasmMemoryImage
                    && artifact.status == config::FactStatus::Extracted
                    && artifact.bytes.is_some()
            })
            .collect::<Vec<_>>();
        if image_fact.len() != 1 {
            return Err(if image_fact.is_empty() {
                WasmDispatchError::MissingFact("one extracted WASM memory image")
            } else {
                WasmDispatchError::AmbiguousFact("one extracted WASM memory image")
            });
        }
        let image_fact = image_fact[0];
        let image_bytes = image_fact.bytes.as_deref().unwrap_or_default();
        if image_bytes.len() > limits.max_image_bytes {
            return Err(WasmDispatchError::Limit(format!(
                "image has {} bytes, exceeding {}",
                image_bytes.len(),
                limits.max_image_bytes
            )));
        }

        let boundary = wasm::analyze_report(&canonical);
        let interpreter = boundary
            .modules
            .iter()
            .filter(|module| {
                module.validated
                    && module.exports.iter().any(|export| {
                        export.name == "W"
                            && export.kind == "function"
                            && export.function_index.is_some()
                    })
            })
            .collect::<Vec<_>>();
        if interpreter.len() != 1 {
            return Err(if interpreter.is_empty() {
                WasmDispatchError::MissingFact("one validated interpreter exporting W")
            } else {
                WasmDispatchError::AmbiguousFact("one validated interpreter exporting W")
            });
        }
        let interpreter = interpreter[0];
        let table_imports = interpreter
            .imports
            .iter()
            .filter(|item| item.kind == "table")
            .collect::<Vec<_>>();
        if table_imports.len() != 1
            || table_imports[0].namespace != "s"
            || table_imports[0].name != "Z"
        {
            return Err(WasmDispatchError::Unsupported(
                "S3 admission requires exactly one interpreter table import s::Z".into(),
            ));
        }
        let producer_instance =
            table_imports[0]
                .source_instance_id
                .ok_or(WasmDispatchError::MissingFact(
                    "validated instance source for interpreter table s::Z",
                ))?;
        let handler = boundary
            .modules
            .iter()
            .filter(|module| {
                module.validated
                    && module.instance_id == Some(producer_instance)
                    && module
                        .tables
                        .iter()
                        .filter(|table| table.exported_as.iter().any(|name| name == "Z"))
                        .count()
                        == 1
            })
            .collect::<Vec<_>>();
        if handler.len() != 1 {
            return Err(if handler.is_empty() {
                WasmDispatchError::MissingFact(
                    "one validated producer instance exporting the interpreter's Z table",
                )
            } else {
                WasmDispatchError::AmbiguousFact(
                    "one validated producer instance exporting the interpreter's Z table",
                )
            });
        }
        let handler = handler[0];
        let interpreter_bytes = module_bytes(&canonical, interpreter)?;
        let handler_bytes = module_bytes(&canonical, handler)?;
        prove_s3_artifact_digest(handler_bytes, S3_HANDLER_SHA256, "handler table")?;
        let (base, stride, opcodes) = prove_interpreter_geometry(interpreter_bytes)?;
        if opcodes > limits.max_opcodes {
            return Err(WasmDispatchError::Limit(format!(
                "interpreter has {opcodes} br_table entries, exceeding {}",
                limits.max_opcodes
            )));
        }
        if stride
            != opcodes
                .checked_mul(2)
                .ok_or_else(|| WasmDispatchError::InvalidFact("opcode width overflow".into()))?
        {
            return Err(WasmDispatchError::InvalidFact(
                "interpreter stride is not two bytes per dispatch cell".into(),
            ));
        }
        if image_bytes.len() < base || (image_bytes.len() - base) % stride != 0 {
            return Err(WasmDispatchError::InvalidFact(
                "proven image length does not cover an integral interpreter dispatch region".into(),
            ));
        }
        let states = (image_bytes.len() - base) / stride;
        if states == 0 || states > limits.max_states {
            return Err(WasmDispatchError::Limit(format!(
                "image implies {states} dispatch states, exceeding {} or empty",
                limits.max_states
            )));
        }

        let tables = handler
            .tables
            .iter()
            .filter(|table| table.exported_as.iter().any(|name| name == "Z"))
            .collect::<Vec<_>>();
        if tables.len() != 1 {
            return Err(WasmDispatchError::AmbiguousFact(
                "exactly one exported handler table Z",
            ));
        }
        let table = tables[0];
        if table.entries.len() > limits.max_table_entries {
            return Err(WasmDispatchError::Limit(format!(
                "handler table has {} entries, exceeding {}",
                table.entries.len(),
                limits.max_table_entries
            )));
        }
        if table.imported
            || table.mutable
            || table.status != "proven"
            || usize::try_from(table.initial).ok() != Some(table.entries.len())
            || table.entries.len() != 201
            || table.entries.iter().enumerate().any(|(index, entry)| {
                entry.table_index != table.table_index
                    || entry.element_index != index as u32
                    || entry.status != "proven"
            })
        {
            return Err(WasmDispatchError::Unsupported(
                "exported Z table is not a complete contiguous immutable S3 table".into(),
            ));
        }
        let handlers = table
            .entries
            .iter()
            .map(|entry| {
                let function_index = entry.function_index?;
                let function = handler
                    .functions
                    .iter()
                    .find(|function| function.function_index == function_index)?;
                let (namespace, name) = function.import.clone()?;
                let import = handler.imports.iter().find(|item| {
                    item.function_index == Some(function_index)
                        && item.namespace == namespace
                        && item.name == name
                })?;
                Some(WasmHostImport {
                    table_index: entry.table_index,
                    element_index: entry.element_index,
                    function_index,
                    namespace,
                    name,
                    signature: import.signature.clone(),
                    status: import.status.clone(),
                })
            })
            .collect::<Vec<_>>();
        if handlers.len() != table.entries.len() || handlers.iter().any(Option::is_none) {
            return Err(WasmDispatchError::Unsupported(
                "handler table contains an unresolved or non-imported function; no handler identity was guessed".into(),
            ));
        }
        Ok(Self {
            interpreter: identity(interpreter, interpreter_bytes),
            handler_table: identity(handler, handler_bytes),
            image: image(
                image_bytes,
                image_fact
                    .binding
                    .clone()
                    .unwrap_or_else(|| "<image>".into()),
                image_fact.span.start..image_fact.span.end,
                base,
                stride,
                states,
                opcodes,
            ),
            handlers,
            synthetic: false,
        })
    }

    /// Build a mechanics-only fixture.  Its identities are explicitly marked
    /// synthetic and can never be confused with S2/S3 provenance.
    pub fn synthetic(
        image_bytes: Vec<u8>,
        base: usize,
        stride: usize,
        states: usize,
        opcodes: usize,
        handlers: Vec<Option<WasmHostImport>>,
    ) -> Result<Self, WasmDispatchError> {
        let Some(cell_width) = opcodes.checked_mul(2) else {
            return Err(WasmDispatchError::InvalidFact(
                "synthetic opcode width overflow".into(),
            ));
        };
        let Some(dispatch_bytes) = stride.checked_mul(states) else {
            return Err(WasmDispatchError::InvalidFact(
                "synthetic dispatch region size overflow".into(),
            ));
        };
        if stride != cell_width
            || base == 0
            || states == 0
            || states > usize::from(u8::MAX) + 1
            || opcodes == 0
            || opcodes > usize::from(u8::MAX) + 1
            || handlers.len() != opcodes
            || base.checked_add(dispatch_bytes) != Some(image_bytes.len())
            || handlers.iter().any(Option::is_none)
        {
            return Err(WasmDispatchError::InvalidFact(
                "synthetic geometry/table dimensions disagree".into(),
            ));
        }
        Ok(Self {
            interpreter: synthetic_identity("synthetic-interpreter"),
            handler_table: synthetic_identity("synthetic-handler-table"),
            image: image(
                &image_bytes,
                "synthetic-image".into(),
                0..0,
                base,
                stride,
                states,
                opcodes,
            ),
            handlers,
            synthetic: true,
        })
    }

    #[must_use]
    pub fn interpreter(&self) -> &WasmArtifactIdentity {
        &self.interpreter
    }

    #[must_use]
    pub fn handler_table(&self) -> &WasmArtifactIdentity {
        &self.handler_table
    }

    #[must_use]
    pub fn image(&self) -> &WasmDispatchImage {
        &self.image
    }

    #[must_use]
    pub fn is_synthetic(&self) -> bool {
        self.synthetic
    }

    #[must_use]
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Resolve one immutable image cell.  This returns the table edge but does
    /// not call it; callers must stop at `host_import` until a host model and
    /// original-handler provenance are separately proven.
    pub fn resolve(&self, pc: usize, state: usize) -> Result<WasmResolution, WasmDispatchError> {
        if pc >= self.image.bytes.len() {
            return Err(WasmDispatchError::EntryOutOfBounds {
                pc,
                image_len: self.image.bytes.len(),
            });
        }
        if pc >= self.image.base {
            return Err(WasmDispatchError::EntryInDispatchRegion {
                pc,
                base: self.image.base,
            });
        }
        if state >= self.image.states {
            return Err(WasmDispatchError::StateOutOfBounds {
                state,
                state_count: self.image.states,
            });
        }
        let opcode = self.image.bytes[pc];
        if opcode as usize >= self.image.opcodes {
            return Err(WasmDispatchError::OpcodeOutOfBounds {
                opcode,
                opcode_count: self.image.opcodes,
            });
        }
        let address = self
            .image
            .base
            .checked_add(state.checked_mul(self.image.stride).ok_or_else(|| {
                WasmDispatchError::InvalidFact("dispatch address overflow".into())
            })?)
            .and_then(|address| {
                (opcode as usize)
                    .checked_mul(2)
                    .and_then(|offset| address.checked_add(offset))
            })
            .ok_or_else(|| WasmDispatchError::InvalidFact("dispatch address overflow".into()))?;
        let end = address
            .checked_add(2)
            .ok_or(WasmDispatchError::CellOutOfBounds {
                address,
                image_len: self.image.bytes.len(),
            })?;
        if end > self.image.bytes.len() {
            return Err(WasmDispatchError::CellOutOfBounds {
                address,
                image_len: self.image.bytes.len(),
            });
        }
        let selector = self.image.bytes[address];
        let next_state = self.image.bytes[address + 1];
        if next_state as usize >= self.image.states {
            return Err(WasmDispatchError::StateOutOfBounds {
                state: next_state as usize,
                state_count: self.image.states,
            });
        }
        let host_import = self
            .handlers
            .get(selector as usize)
            .and_then(Clone::clone)
            .ok_or_else(|| {
                WasmDispatchError::Unsupported(format!(
                    "dispatch selector {selector} has no proven handler-table entry"
                ))
            })?;
        Ok(WasmResolution {
            pc,
            state,
            opcode,
            opcode_span: (pc, pc + 1),
            cell_address: address,
            cell_span: (address, end),
            selector,
            next_state,
            host_import: Some(host_import),
        })
    }
}

fn authenticate_report(
    canonical: &PipelineReport,
    supplied: &PipelineReport,
) -> Result<(), WasmDispatchError> {
    if supplied.input_hash != canonical.input_hash {
        return Err(WasmDispatchError::InvalidFact(
            "supplied pipeline report input hash does not match the raw source".into(),
        ));
    }
    if supplied.output != canonical.output {
        return Err(WasmDispatchError::InvalidFact(
            "supplied pipeline report output does not match the canonical raw-source artifact"
                .into(),
        ));
    }
    if !same_wasm_facts(canonical, supplied) {
        return Err(WasmDispatchError::InvalidFact(
            "supplied WASM extraction facts do not match the canonical raw-source report".into(),
        ));
    }
    if supplied.proven_operation_tables != canonical.proven_operation_tables {
        return Err(WasmDispatchError::InvalidFact(
            "supplied operation-table provenance does not match the canonical raw-source report"
                .into(),
        ));
    }
    Ok(())
}

fn same_wasm_facts(left: &PipelineReport, right: &PipelineReport) -> bool {
    let left = left
        .extraction
        .iter()
        .filter(|fact| fact.kind == ExtractionKind::WasmModule)
        .collect::<Vec<_>>();
    let right = right
        .extraction
        .iter()
        .filter(|fact| fact.kind == ExtractionKind::WasmModule)
        .collect::<Vec<_>>();
    left.len() == right.len()
        && left.into_iter().zip(right).all(|(left, right)| {
            left.status == right.status
                && left.binding == right.binding
                && left.span.start == right.span.start
                && left.span.end == right.span.end
                && left.length == right.length
                && left.decoded_bytes == right.decoded_bytes
                && left.value == right.value
                && left.evidence == right.evidence
        })
}

fn identity(module: &wasm::WasmModuleBoundary, bytes: &[u8]) -> WasmArtifactIdentity {
    WasmArtifactIdentity {
        binding: module
            .source_binding
            .clone()
            .unwrap_or_else(|| "<wasm-module>".into()),
        sha256: digest(bytes),
        byte_len: bytes.len(),
        source: module.extraction_source.clone(),
    }
}

fn synthetic_identity(binding: &str) -> WasmArtifactIdentity {
    WasmArtifactIdentity {
        binding: binding.into(),
        sha256: digest(binding.as_bytes()),
        byte_len: 0,
        source: None,
    }
}

fn image(
    bytes: &[u8],
    binding: String,
    span: std::ops::Range<u32>,
    base: usize,
    stride: usize,
    states: usize,
    opcodes: usize,
) -> WasmDispatchImage {
    WasmDispatchImage {
        bytes: Arc::from(bytes.to_vec()),
        identity: WasmArtifactIdentity {
            binding,
            sha256: digest(bytes),
            byte_len: bytes.len(),
            source: Some(wasm::SourceLink {
                artifact: "runtime-config".into(),
                start: span.start,
                end: span.end,
            }),
        },
        base,
        stride,
        states,
        opcodes,
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn prove_s3_artifact_digest(
    bytes: &[u8],
    expected: [u8; 32],
    artifact: &str,
) -> Result<(), WasmDispatchError> {
    if digest(bytes) != expected {
        return Err(WasmDispatchError::Unsupported(format!(
            "{artifact} is not the exact verified S3 artifact; general WASM geometry/dataflow is unsupported"
        )));
    }
    Ok(())
}

fn module_bytes<'a>(
    report: &'a PipelineReport,
    module: &wasm::WasmModuleBoundary,
) -> Result<&'a [u8], WasmDispatchError> {
    let Some(binding) = module.source_binding.as_deref() else {
        return Err(WasmDispatchError::MissingFact("module source binding"));
    };
    let candidates = report
        .extraction
        .iter()
        .filter(|fact| {
            fact.kind == ExtractionKind::WasmModule
                && fact.status == ExtractionStatus::Extracted
                && fact.binding.as_deref() == Some(binding)
                && fact.decoded_bytes.is_some()
        })
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Err(if candidates.is_empty() {
            WasmDispatchError::MissingFact("module decoded bytes")
        } else {
            WasmDispatchError::AmbiguousFact("module decoded bytes")
        });
    }
    let bytes = candidates[0].decoded_bytes.as_deref().unwrap_or_default();
    if fnv1a64_hex(bytes) != module.module_hash {
        return Err(WasmDispatchError::InvalidFact(format!(
            "decoded bytes for binding {binding} do not match the boundary module hash"
        )));
    }
    Ok(bytes)
}

fn prove_interpreter_geometry(bytes: &[u8]) -> Result<(usize, usize, usize), WasmDispatchError> {
    prove_s3_artifact_digest(bytes, S3_INTERPRETER_SHA256, "interpreter")?;
    // This is not a general opcode recognizer.  The digest is the admission
    // proof for the exact current S3 fixture, and therefore covers the full
    // exported-W body/dataflow rather than a prefix or an unrelated br_table.
    // These constants are the geometry recorded for that exact verified body.
    Ok((170_845, 468, 234))
}
