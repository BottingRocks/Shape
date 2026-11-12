//! Source-bound signal programs in the existing per-seed schema.
//!
//! The contract grammar is the wire IR. Source expressions are evidence,
//! never code executed by the consumer. Unproved operations fail closed.
mod algorithms;
mod consumer;
mod compact;
mod counted;
pub use compact::CompactSchema;
mod control;
mod float;
pub mod named;
pub mod semantic;
mod position;
mod program;
pub(crate) mod source_number;

pub use algorithms::{
    AlgorithmProof, AlgorithmReport, InitializationContext, InitializationEpoch, StreamReadiness,
    StreamReadinessError,
};
pub use consumer::{
    DecodedSlot, Decoder, Outcome, SchemaDocument, Selection, SelectionProof, WireError,
};
pub use control::{
    BitPredicate, ChoiceDomainProof, ChoiceSemantics, ControlPath, ControlStep, CountExpr,
    CountRef, CountRefusal, CounterChoiceProof, CounterInput, CounterRef, EnvelopeProof,
    NumberEnvelope, PackedPhase, RootCasePlan, SourcePlan, UnprovenCount,
};
pub use float::{FloatByteOrder, NumberFloatMetadata, TypedDecoderOptions};
pub(crate) use named::emitted_key;
pub(crate) mod readable;
pub use named::{NamedDocument, NamedSignal, NamedStatus, Naming};
pub use program::WireValue;
pub use program::{Op, Program, RootCase, Support, TagConsumption, WireMask};

use crate::signals::{
    analysis::SlotSummary,
    extract::{
        contracts::{SignalContract, SignalContractReport},
        encoding::EncodingProperties,
        inputs::ProgramInputReport,
    },
    schema::{Routing, SourceIdentity},
};
use serde::{Deserialize, Serialize};

/// Additive wire metadata version; independent of the descriptive schema.
pub const WIRE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireRoute {
    Positional { positional: usize },
    Name { name: serde_json::Value },
}

impl From<&Routing> for WireRoute {
    fn from(route: &Routing) -> Self {
        match route {
            Routing::Positional(positional) => Self::Positional {
                positional: *positional,
            },
            Routing::Name(name) => Self::Name { name: name.clone() },
        }
    }
}

/// Every descriptor survives, including unrecognized/refused descriptive slots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireSlot {
    pub address: String,
    pub routing: WireRoute,
    pub contract: SignalContract,
    pub programs: Vec<Support>,
    #[serde(default)]
    pub source_plan: Vec<SourcePlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naming: Option<Naming>,
}

/// The full ordered contract grammar retains raw prefixes, masks, loop
/// direction/count/header, choices and unresolved facts without projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireSchema {
    pub version: u32,
    pub source: SourceIdentity,
    pub algorithms: algorithms::AlgorithmReport,
    pub encoding: EncodingProperties,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_float: Option<NumberFloatMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization: Option<InitializationCensus>,
    pub slots: Vec<WireSlot>,
}

/// One result for every source emission, including explicit proof refusals.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitializationCensus {
    pub initializer_sha256: String,
    pub required_context: InitializationEpoch,
    pub emissions: Vec<EmissionReadiness>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmissionReadiness {
    pub descriptor: u32,
    pub emission: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_case: Option<usize>,
    pub outcome: InstanceReadiness,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstanceReadiness {
    Ready { witness: StreamReadiness },
    Refused { reason: String },
}

impl WireSchema {
    pub(crate) fn from_analysis(
        source: &str,
        inputs: &ProgramInputReport,
        contracts: &SignalContractReport,
        slots: &[SlotSummary],
    ) -> Result<Self, WireError> {
        if slots.len() != contracts.signals.len()
            || slots
                .iter()
                .zip(&contracts.signals)
                .any(|(slot, contract)| {
                    slot.descriptor != contract.descriptor
                        || slot.address != named::contract_address(contract)
                })
        {
            return Err(WireError::Evidence);
        }
        let algorithms = algorithms::certify(source, &inputs.encoding, &inputs.wiring);
        Ok(Self {
            version: WIRE_VERSION,
            source: contracts.source.clone(),
            number_float: Some(float::certify(source, &algorithms.integer)),
            initialization: None,
            algorithms: algorithms.clone(),
            encoding: inputs.encoding.clone(),
            slots: slots
                .iter()
                .zip(&contracts.signals)
                .map(|(slot, contract)| {
                    Ok(WireSlot {
                        address: slot.address.clone(),
                        routing: (&slot.routing).into(),
                        contract: contract.clone(),
                        programs: programs_for(contract, &algorithms, source),
                        source_plan: contract
                            .emissions
                            .iter()
                            .map(|e| control::source_plan(source, e, &algorithms))
                            .collect(),
                        naming: named::from_slot(slot, contract)?,
                    })
                })
                .collect::<Result<_, WireError>>()?,
        })
    }
}

fn programs_for(
    contract: &SignalContract,
    algorithms: &AlgorithmReport,
    source: &str,
) -> Vec<Support> {
    contract
        .emissions
        .iter()
        .map(|e| {
            let result = if contract.status
                != crate::signals::extract::contracts::ContractStatus::Complete
            {
                Err(WireError::Unsupported(
                    "contract write accounting is incomplete".into(),
                ))
            } else {
let body = conditional_stream_body(source, algorithms);
program::compile_with_stream(e, algorithms, source, &body)

            };
            match result {
Ok(program) if algorithms.stream.codec.is_some() => Support::Supported { program },
Ok(program) => Support::RequiresInitializer {
    program,
    stream_body: conditional_stream_body(source, algorithms),
    required_context: InitializationEpoch::SuppliedPublicInitializerWithoutInterveningInitialization,
},

                Err(error) => {
                    let reason = match error {
                        WireError::Unsupported(reason) => reason,
                        other => other.to_string(),
                    };
                    let minimum_input = if reason.contains("external branch")
                        || reason.contains("no unique common-mask discriminant")
                    {
                        Some("source branch outcome at this grammar choice".into())
                    } else if reason.contains("repeat needs") {
                        Some("iteration count for this source loop".into())
                    } else if reason.contains("prior state") {
                        Some("prior stream cursor and UTF-16 intern table for this instance".into())
                    } else {
                        None
                    };
                    Support::Unsupported {
                        reason,
                        minimum_input,
                    }
                }
            }
        })
        .collect()
}

fn conditional_stream_body(source: &str, algorithms: &AlgorithmReport) -> AlgorithmProof {
    if algorithms.stream.codec.is_some() {
        return algorithms.stream.clone();
    }
    algorithms::proof(
        algorithms.stream.span.and_then(|span| {
            source
                .get(span.start as usize..span.end as usize)
                .map(|text| (span, text))
        }),
        include_str!("reference/stream.js"),
        "sum-rot26-9-13-le-burn4/1",
        false,
    )
}

/// Low-level operations for an explicitly configured source-format adapter.
///
/// These do not certify initializer lifecycle or a source program. Callers must
/// validate their source bindings and supply the float order and token hash.
/// The strict [`Decoder`] loaders retain their independent evidence gates.
#[doc(hidden)]
pub mod explicit_initializer {
    use super::*;
    fn float_format(order: FloatByteOrder) -> Result<FloatByteOrder, WireError> {
        match order {
            FloatByteOrder::Unknown => Err(WireError::Unsupported(
                "native float byte order must be explicitly configured".into(),
            )),
            order => Ok(order),
        }
    }
    pub fn decode(
        p: &Program,
        bytes: &[u8],
        token: i32,
        order: FloatByteOrder,
    ) -> Result<WireValue, WireError> {
        p.decode_with_float(bytes, token, &float_format(order))
    }
    pub fn encode(
        p: &Program,
        value: &WireValue,
        seed: u32,
        token: i32,
        order: FloatByteOrder,
    ) -> Result<Vec<u8>, WireError> {
        p.encode_with_float(value, seed, token, &float_format(order))
    }
    pub fn project(
        slot: &WireSlot,
        decoded: &DecodedSlot,
        token: i32,
        order: FloatByteOrder,
    ) -> NamedSignal {
        named::project(slot, decoded, token, &float_format(order))
    }
    /// Project an explicitly supplied emission recipe while borrowing the slot evidence.
    pub fn project_with_program(
        slot: &WireSlot,
        decoded: &DecodedSlot,
        emission: usize,
        program: &Program,
        token: i32,
        order: FloatByteOrder,
    ) -> NamedSignal {
        named::project_with_program(
            slot,
            decoded,
            token,
            &float_format(order),
            Some((emission, program)),
        )
    }
    /// [`project_with_program`] with explicit metadata/verification options.
    pub fn project_with_options(
        slot: &WireSlot,
        decoded: &DecodedSlot,
        emission: usize,
        program: &Program,
        token: i32,
        order: FloatByteOrder,
        options: named::ProjectOptions<'_>,
    ) -> NamedSignal {
        named::project_with_options(
            slot,
            decoded,
            token,
            &float_format(order),
            Some((emission, program)),
            options,
        )
    }
    pub fn node(
        n: &crate::signals::extract::contracts::GrammarNode,
        e: &crate::signals::extract::contracts::EmissionContract,
        a: &AlgorithmReport,
        plan: &SourcePlan,
        source: &str,
    ) -> Result<Op, WireError> {
        program::compile_node(n, e, a, plan, source, 0)
    }
}
