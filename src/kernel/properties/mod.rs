//! Bounded extraction of the actual values present in a deobfuscated VM copy.
//!
//! This is intentionally a small value reader, not another evidence or report
//! format.  It walks Oxc's AST, follows resolved lexical bindings, and keeps an
//! expression when static evaluation would require JavaScript execution.

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::kernel::{
    config::ExtractionLimits,
    decoder_proof::{prove_runtime_decoder, DecoderProof},
    wasm_module::{self, Base64Policy, ModuleLimits},
};
use crate::{analysis, util::json_string};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

#[derive(Debug)]
pub enum ActualPropertiesError {
    InvalidInput(String),
    Parse(String),
    Limit { what: String, limit: usize },
}

impl fmt::Display for ActualPropertiesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(value) => write!(f, "invalid actual-properties input: {value}"),
            Self::Parse(value) => write!(f, "actual-properties parse failed: {value}"),
            Self::Limit { what, limit } => {
                write!(f, "actual-properties limit exceeded for {what} ({limit})")
            }
        }
    }
}
impl std::error::Error for ActualPropertiesError {}

/// A value that can be printed without pretending that JavaScript was run.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    Undefined,
    NaN,
    PositiveInfinity,
    NegativeInfinity,
    Hole,
    Array(Vec<PropertyValue>),
    Object(BTreeMap<String, PropertyValue>),
    Function(String),
    Expression(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedValue {
    pub name: String,
    pub value: PropertyValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntryProperties {
    pub function_name: Option<String>,
    pub parameters: Vec<String>,
    pub arguments: Vec<PropertyValue>,
    pub initial_memory_slots: Option<PropertyValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperationSet {
    pub name: String,
    pub keys: Vec<String>,
    pub functions: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlotProperties {
    pub context: BTreeMap<String, PropertyValue>,
    pub memory: BTreeMap<String, PropertyValue>,
    pub stack: BTreeMap<String, PropertyValue>,
    /// Prototype descriptors (`Object.defineProperty(Ctor.prototype, ...)`) kept
    /// separate from the constructor's `this.<field>` initial values so a
    /// descriptor never masquerades as an initialized value.
    pub context_prototype: BTreeMap<String, PropertyValue>,
    pub memory_prototype: BTreeMap<String, PropertyValue>,
    /// Proven VM role constructor names (from RENAME), when the extractor
    /// identified them. These name the function whose source carries the
    /// constructor/prototype semantics; the field-only maps above cannot.
    pub context_constructor: Option<String>,
    pub memory_constructor: Option<String>,
    pub stack_constructor: Option<String>,
    /// Proven constructor parameter names, in source order. These are data the
    /// tracer needs to map member initializers to parameter roles; the
    /// constructor body is not carried in the default bundle.
    pub context_parameters: Vec<String>,
    pub memory_parameters: Vec<String>,
    pub stack_parameters: Vec<String>,
    /// Exact literal returned by the proven stack constructor when it returns a
    /// literal array (for example `var stack = []; return stack;`). This is the
    /// stack's initial value, not a prototype default.
    pub stack_initial: Option<PropertyValue>,
    /// The entry call's fourth argument, which is a list of special slot
    /// indices.  It is deliberately not confused with descriptor initialSlots.
    pub initial_special_indices: Vec<PropertyValue>,
    /// Writes made directly by the selected entry function.
    pub initial_assignments: Vec<SlotAssignment>,
    /// Write/initialize calls found in helper bodies. These are templates, not
    /// claims that the helper ran during startup.
    pub function_write_templates: Vec<SlotAssignment>,
    /// Exact constructor/prototype property maps. Entries remain available
    /// here even when their VM role cannot be proven.
    pub constructor_properties: BTreeMap<String, BTreeMap<String, PropertyValue>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlotAssignment {
    pub function: String,
    pub method: String,
    pub index: PropertyValue,
    pub value: PropertyValue,
}

/// Actual, concise per-sample VM properties.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActualProperties {
    pub entry: Option<EntryProperties>,
    pub thread_descriptors: Vec<NamedValue>,
    pub operations: Vec<OperationSet>,
    pub strings: Vec<NamedValue>,
    pub bytecode: Vec<NamedValue>,
    pub dispatch: Vec<NamedValue>,
    pub decoders: Vec<NamedValue>,
    /// Decoder calls whose callee did not pass a base64-decoder proof, kept
    /// with the exact encoded literal and destination so the payload is never
    /// silently dropped.
    pub unproven_decoder_calls: Vec<NamedValue>,
    pub constants: Vec<NamedValue>,
    pub wasm: Vec<NamedValue>,
    pub slots: SlotProperties,
    pub additional: Vec<NamedValue>,
    pub unresolved: Vec<NamedValue>,
    /// Every binding owned by the selected entry function, preserving its
    /// exact name and bounded literal/function/expression value.
    pub properties: BTreeMap<String, PropertyValue>,
}

impl ActualProperties {
    /// Return the first entry's concrete initial slot setup when it is data,
    /// never by leaking an `Expression` source fallback to consumers.
    pub fn first_entry_initial_slots(&self) -> Option<Vec<u32>> {
        self.entry
            .as_ref()
            .and_then(|entry| entry.arguments.get(3))
            .and_then(property_slot_indices)
    }

    /// Return the concrete outer entry coordinates when the selected entry
    /// invocation carries integer PC/state arguments.
    pub fn entry_coordinates(&self) -> Option<(u32, u32)> {
        let entry = self.entry.as_ref()?;
        Some((
            property_u32(entry.arguments.get(1)?)?,
            property_u32(entry.arguments.get(2)?)?,
        ))
    }

    pub fn first_thread_descriptor_rows(&self) -> Option<&[PropertyValue]> {
        match &self.thread_descriptors.first()?.value {
            PropertyValue::Array(rows) => Some(rows),
            _ => None,
        }
    }

    pub fn json(&self) -> String {
        self.compact_json(&[])
    }

    /// Render the shared human-facing view. Operation bodies are opt-in and
    /// selected by their actual table key (for example, `--op 7`).
    pub fn compact_json(&self, selected_operations: &[String]) -> String {
        properties_json(self, selected_operations)
    }

    /// Render the concise tracer-input bundle. Unlike [`Self::compact_json`],
    /// this carries every operation-table handler's source by default, the
    /// reachable function/decoder bodies with AST-resolved dependencies, the
    /// canonical closure of referenced VM bindings (with globals kept as
    /// external references), and the VM context/memory/stack initializer and
    /// prototype descriptor maps, while dropping the generic binding dumps and
    /// expanded decoded byte arrays.
    pub fn tracer_json(&self, selected_operations: &[String]) -> String {
        crate::kernel::bundle::tracer_json(self, selected_operations)
    }

    pub fn tracer_markdown(&self, selected_operations: &[String]) -> String {
        crate::kernel::bundle::tracer_markdown(self, selected_operations)
    }

    pub fn markdown(&self) -> String {
        self.compact_markdown(&[])
    }

    pub fn compact_markdown(&self, selected_operations: &[String]) -> String {
        let json = self.compact_json(selected_operations);
        let mut out = String::from("# VM properties\n\n");
        if let Some(entry) = &self.entry {
            out.push_str(&format!(
                "- Entry: `{}`; parameters `{}`\n",
                entry.function_name.as_deref().unwrap_or("<anonymous>"),
                entry.parameters.join("`, `")
            ));
        }
        out.push_str("\n```json\n");
        out.push_str(&json);
        out.push_str("\n```\n");
        out
    }

    /// Validate operation selections before rendering a compact view.
    pub fn validate_selected_operations(
        &self,
        selected_operations: &[String],
    ) -> Result<(), String> {
        let known = self
            .operations
            .iter()
            .flat_map(|operation| operation.keys.iter())
            .collect::<BTreeSet<_>>();
        if let Some(missing) = selected_operations
            .iter()
            .find(|selected| !known.contains(selected))
        {
            let available = known.into_iter().cloned().collect::<Vec<_>>().join(", ");
            return Err(if available.is_empty() {
                format!("unknown operation key `{missing}`; no operation keys are available")
            } else {
                format!("unknown operation key `{missing}`; available operation keys: {available}")
            });
        }
        Ok(())
    }
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

fn property_slot_indices(value: &PropertyValue) -> Option<Vec<u32>> {
    let PropertyValue::Array(values) = value else {
        return None;
    };
    values.iter().map(property_u32).collect()
}

pub fn extract_actual_properties(source: &str) -> Result<ActualProperties, ActualPropertiesError> {
    extract_actual_properties_with_limits(source, ExtractionLimits::default())
}

/// Extract only the entry, thread-descriptor, and worker-bootstrap facts that
/// the kernel artifact needs.  This deliberately skips the expensive
/// operation/decoder/string/slot projections in [`Scanner::finish`].
pub(crate) fn extract_actual_entry_facts(
    source: &str,
) -> Result<ActualProperties, ActualPropertiesError> {
    let limits = ExtractionLimits::default();
    if source.is_empty() {
        return Err(ActualPropertiesError::InvalidInput(
            "source is empty".into(),
        ));
    }
    if source.len() > limits.max_source_bytes {
        return Err(ActualPropertiesError::Limit {
            what: "source bytes".into(),
            limit: limits.max_source_bytes,
        });
    }
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, source).map_err(ActualPropertiesError::Parse)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scanner = Scanner::new(source, &scoping, limits);
    scanner.visit_program(&program);
    scanner.finish_entry_facts()
}

/// Extract only the consumed operation tables from a source.  Standardized
/// output can be used as an artifact input even when the pipeline report no
/// longer carries the raw FIND/RENAME operation-table identity; the structural
/// table proof itself is still available here.
pub(crate) fn extract_actual_operation_tables(
    source: &str,
) -> Result<Vec<OperationSet>, ActualPropertiesError> {
    let limits = ExtractionLimits::default();
    if source.is_empty() {
        return Err(ActualPropertiesError::InvalidInput(
            "source is empty".into(),
        ));
    }
    if source.len() > limits.max_source_bytes {
        return Err(ActualPropertiesError::Limit {
            what: "source bytes".into(),
            limit: limits.max_source_bytes,
        });
    }
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, source).map_err(ActualPropertiesError::Parse)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scanner = Scanner::new(source, &scoping, limits);
    scanner.visit_program(&program);
    scanner.finish_operation_tables()
}

pub fn extract_actual_properties_with_limits(
    source: &str,
    limits: ExtractionLimits,
) -> Result<ActualProperties, ActualPropertiesError> {
    if source.is_empty() {
        return Err(ActualPropertiesError::InvalidInput(
            "source is empty".into(),
        ));
    }
    if source.len() > limits.max_source_bytes {
        return Err(ActualPropertiesError::Limit {
            what: "source bytes".into(),
            limit: limits.max_source_bytes,
        });
    }
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, source).map_err(ActualPropertiesError::Parse)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scanner = Scanner::new(source, &scoping, limits);
    scanner.visit_program(&program);
    scanner.finish()
}

mod json;
mod scanner;

#[cfg(test)]
mod tests;

use json::properties_json;
pub(crate) use json::value_json;
use scanner::Scanner;
pub(crate) use scanner::{is_worker_bootstrap_value, wasm_summary};
