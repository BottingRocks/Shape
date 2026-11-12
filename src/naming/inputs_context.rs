//! The whole-program context view of the program inputs (`shape signals
//! inputs`): the signal-side input contract joined to the function index /
//! role graph inventory and the outer output pipeline by source spans.

use serde::{Deserialize, Serialize};

use crate::naming::function_index::extract_function_index;
use crate::naming::output::{extract_output_properties_with_context, OutputProperties};
use crate::naming::role_graph::{extract_program_role_graph_with_context, ProgramRoleGraph};
use crate::signals::extract::inputs::{
    extract_program_inputs, source_identity, verify_identity, ProgramInputError,
    ProgramInputReport,
};

pub const PROGRAM_INPUT_CONTEXT_SCHEMA_VERSION: u32 = 1;

/// A joined, additive whole-program view of [`ProgramInputReport`].  The base
/// extractor remains the small glyph/DOM API; this report keeps the inventory
/// and outer-output facts alongside it so later naming/commenting work can
/// follow source evidence instead of treating two signal families as the whole
/// program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramInputContextReport {
    pub schema_version: u32,
    pub inputs: ProgramInputReport,
    pub role_graph: ProgramRoleGraph,
    pub output: OutputProperties,
    pub joins: Vec<ProgramInputJoin>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramInputJoinStatus {
    Confirmed,
    Unresolved,
}

/// A source-span join between independently typed passes.  `symbol` fields
/// are intentionally absent: symbol ordinals are local to one parse and are
/// never portable join keys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramInputJoin {
    pub relationship: String,
    pub from: crate::signals::schema::SourceSpan,
    pub to: Option<crate::signals::schema::SourceSpan>,
    pub status: ProgramInputJoinStatus,
    pub evidence: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgramInputContextError {
    Inputs(ProgramInputError),
    FunctionIndex(String),
    RoleGraph(String),
    Output(String),
    SourceIdentity { stage: String, message: String },
}

impl std::fmt::Display for ProgramInputContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inputs(error) => error.fmt(f),
            Self::FunctionIndex(message) => write!(f, "whole-program function index: {message}"),
            Self::RoleGraph(message) => write!(f, "whole-program role graph: {message}"),
            Self::Output(message) => write!(f, "outer output pipeline: {message}"),
            Self::SourceIdentity { stage, message } => {
                write!(f, "whole-program {stage} source identity: {message}")
            }
        }
    }
}

impl std::error::Error for ProgramInputContextError {}

impl From<ProgramInputError> for ProgramInputContextError {
    fn from(error: ProgramInputError) -> Self {
        Self::Inputs(error)
    }
}

/// Extract the input contract and join it to the source-derived whole-program
/// inventory and outer output pipeline.  Every pass is checked against the
/// exact SHA-256/byte identity before span joins are emitted.
pub fn extract_program_inputs_with_context(
    source: &str,
) -> Result<ProgramInputContextReport, ProgramInputContextError> {
    let inputs = extract_program_inputs(source)?;
    let index = extract_function_index(source)
        .map_err(|error| ProgramInputContextError::FunctionIndex(error.to_string()))?;
    let expected = source_identity(source);
    verify_identity("function_index", &expected, &index.source).map_err(|error| {
        ProgramInputContextError::SourceIdentity {
            stage: "function_index".to_string(),
            message: error.to_string(),
        }
    })?;
    let role_graph =
        extract_program_role_graph_with_context(source, &index, &inputs.encoding, &inputs.wiring)
            .map_err(|error| ProgramInputContextError::RoleGraph(error.to_string()))?;
    let output = extract_output_properties_with_context(source, &inputs.encoding, &inputs.wiring)
        .map_err(|error| ProgramInputContextError::Output(error.to_string()))?;
    verify_identity("role_graph", &expected, &role_graph.source).map_err(|error| {
        ProgramInputContextError::SourceIdentity {
            stage: "role_graph".to_string(),
            message: error.to_string(),
        }
    })?;
    verify_identity("output", &expected, &output.source).map_err(|error| {
        ProgramInputContextError::SourceIdentity {
            stage: "output".to_string(),
            message: error.to_string(),
        }
    })?;

    Ok(ProgramInputContextReport {
        schema_version: PROGRAM_INPUT_CONTEXT_SCHEMA_VERSION,
        joins: build_context_joins(&inputs, &role_graph, &output),
        inputs,
        role_graph,
        output,
    })
}

fn build_context_joins(
    inputs: &ProgramInputReport,
    role_graph: &ProgramRoleGraph,
    output: &OutputProperties,
) -> Vec<ProgramInputJoin> {
    let mut joins = Vec::new();
    let contains = |outer: crate::signals::schema::SourceSpan, inner: crate::signals::schema::SourceSpan| {
        inner.start >= outer.start && inner.end <= outer.end
    };
    for request in &inputs.wiring.collector_requests {
        let collector = request.function;
        let mut matched = false;
        for family in &inputs.generator_inputs.families {
            for emission in &family.emissions {
                if contains(collector, emission.origin.span) {
                    matched = true;
                    joins.push(ProgramInputJoin {
                        relationship: format!("collector_to_{:?}_emission", family.kind),
                        from: collector,
                        to: Some(emission.origin.span),
                        status: ProgramInputJoinStatus::Confirmed,
                        evidence: format!(
                            "collector source contains signal {} origin",
                            emission.signal
                        ),
                    });
                    if let Some(buffer) = &emission.buffer {
                        joins.push(ProgramInputJoin {
                            relationship: "emission_to_owned_buffer".to_string(),
                            from: emission.origin.span,
                            to: Some(buffer.span),
                            status: ProgramInputJoinStatus::Confirmed,
                            evidence: "emission and buffer are retained in the same source report"
                                .to_string(),
                        });
                    }
                }
            }
        }
        if !matched {
            joins.push(ProgramInputJoin {
                relationship: "collector_to_signal_emission".to_string(),
                from: collector,
                to: None,
                status: ProgramInputJoinStatus::Unresolved,
                evidence: "no family emission origin was proven inside this collector".to_string(),
            });
        }
    }
    for family in &inputs.generator_inputs.families {
        for emission in &family.emissions {
            let producer = family.producer.span;
            joins.push(ProgramInputJoin {
                relationship: format!("{:?}_producer_to_emission", family.kind),
                from: producer,
                to: Some(emission.origin.span),
                status: if contains(producer, emission.origin.span) {
                    ProgramInputJoinStatus::Confirmed
                } else {
                    ProgramInputJoinStatus::Unresolved
                },
                evidence: "family producer and emission retain exact source evidence".to_string(),
            });
        }
    }
    for link in &output.context_links {
        if let Some(span) = link.report_span {
            joins.push(ProgramInputJoin {
                relationship: format!("output_context_{}", link.role),
                from: span,
                to: Some(span),
                status: if link.matched {
                    ProgramInputJoinStatus::Confirmed
                } else {
                    ProgramInputJoinStatus::Unresolved
                },
                evidence: format!("output context link from {}", link.context_source),
            });
        }
    }
    if let Some(main) = role_graph.main.as_ref().and_then(|id| role_graph.node(id)) {
        joins.push(ProgramInputJoin {
            relationship: "wiring_main_to_role_inventory".to_string(),
            from: inputs.wiring.main.span,
            to: Some(main.span),
            status: ProgramInputJoinStatus::Confirmed,
            evidence: "the selected MAIN span is shared by wiring and role inventory".to_string(),
        });
    }
    joins
}

