use super::*;

// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub(super) struct Identity<'a> {
    pub(super) algorithm: &'a str,
    pub(super) digest: &'a str,
}

#[derive(Serialize)]
pub(super) struct CaptureJson<'a> {
    pub(super) input_kind: &'a str,
    pub(super) identity: Identity<'a>,
    pub(super) byte_length: u64,
    pub(super) raw_identity: &'a str,
}

#[derive(Serialize)]
pub(super) struct DerivedFromJson<'a> {
    pub(super) report: &'a str,
    pub(super) catalogue_version: &'a str,
    pub(super) source_hash: Identity<'a>,
}

#[derive(Serialize)]
pub(super) struct SpanSourceJson<'a> {
    pub(super) kind: &'static str,
    pub(super) identity: Identity<'a>,
    pub(super) byte_length: usize,
}

#[derive(Serialize)]
pub(super) struct JoinJson<'a> {
    pub(super) status: &'static str,
    pub(super) table_key: Option<u32>,
    pub(super) handler_index: Option<u32>,
    pub(super) table_identity: &'static str,
    pub(super) source_identity: &'static str,
    pub(super) span_identity: &'static str,
    pub(super) reasons: &'a [String],
}

impl<'a> From<&'a JoinReport> for JoinJson<'a> {
    fn from(join: &'a JoinReport) -> Self {
        Self {
            status: join.status.as_str(),
            table_key: join.table_key,
            handler_index: join.handler_index,
            table_identity: join.table_identity.as_str(),
            source_identity: join.source_identity.as_str(),
            span_identity: join.span_identity.as_str(),
            reasons: &join.reasons,
        }
    }
}

#[derive(Serialize)]
pub(super) struct HandlerTemplatesAnalysisJson<'a> {
    pub(super) status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) schema_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source_hash: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) coverage: Option<&'a TemplateCoverage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) join: Option<JoinJson<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) handler: Option<&'a HandlerTemplate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reason: Option<&'a str>,
}

#[derive(Serialize)]
pub(super) struct OperandSeparationAnalysisJson<'a> {
    pub(super) status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) schema_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source_hash: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) coverage: Option<&'a OperandCoverage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) join: Option<JoinJson<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) handler: Option<&'a LiftedHandler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reason: Option<&'a str>,
}

#[derive(Serialize)]
pub(super) struct RecipeAdmissionJson<'a> {
    pub(super) status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) basis: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) detail: Option<&'a str>,
}

#[derive(Serialize)]
pub(super) struct AnalysesJson<'a> {
    pub(super) handler_templates: HandlerTemplatesAnalysisJson<'a>,
    pub(super) operand_separation: OperandSeparationAnalysisJson<'a>,
    pub(super) recipe_admission: RecipeAdmissionJson<'a>,
}

#[derive(Serialize)]
pub(super) struct AnalystAnnotationJson<'a> {
    pub(super) text: &'a str,
    pub(super) authority: &'static str,
    pub(super) execution_authority: bool,
}

#[derive(Serialize)]
pub(super) struct OpsListJson<'a> {
    pub(super) capture: CaptureJson<'a>,
    pub(super) derived_from: DerivedFromJson<'a>,
    pub(super) table: &'a OperationTable,
    pub(super) inventory: &'a InventoryCounts,
    pub(super) handlers: ListHandlers<'a>,
}

#[derive(Serialize)]
pub(super) struct OpsShowJson<'a> {
    pub(super) capture: CaptureJson<'a>,
    pub(super) derived_from: DerivedFromJson<'a>,
    pub(super) table: &'a OperationTable,
    pub(super) handler: ShowHandlerJson<'a>,
    pub(super) span_source: SpanSourceJson<'a>,
    pub(super) analyses: AnalysesJson<'a>,
    pub(super) analyst_annotation: Option<AnalystAnnotationJson<'a>>,
}

pub(super) fn analyses_json<'a>(
    analyses: Option<&'a Analyses>,
    context: &'a ShowContext<'a>,
) -> AnalysesJson<'a> {
    let Some(analyses) = analyses else {
        return AnalysesJson {
            handler_templates: HandlerTemplatesAnalysisJson {
                status: "not_included",
                schema_version: None,
                source_hash: None,
                coverage: None,
                join: None,
                handler: None,
                reason: None,
            },
            operand_separation: OperandSeparationAnalysisJson {
                status: "not_included",
                schema_version: None,
                source_hash: None,
                coverage: None,
                join: None,
                handler: None,
                reason: None,
            },
            recipe_admission: RecipeAdmissionJson {
                status: "not_included",
                basis: None,
                code: None,
                detail: None,
            },
        };
    };
    let handler_templates = match &analyses.templates {
        AnalysisData::Included(report) => HandlerTemplatesAnalysisJson {
            status: "included",
            schema_version: Some(&report.schema_version),
            source_hash: Some(&report.source_hash),
            coverage: Some(&report.coverage),
            join: Some(JoinJson::from(&context.template_join)),
            handler: context.template,
            reason: None,
        },
        AnalysisData::Unavailable { reason } => HandlerTemplatesAnalysisJson {
            status: "unavailable",
            schema_version: None,
            source_hash: None,
            coverage: None,
            join: None,
            handler: None,
            reason: Some(reason),
        },
    };
    let operand_separation = match &analyses.operands {
        AnalysisData::Included(report) => OperandSeparationAnalysisJson {
            status: "included",
            schema_version: Some(&report.schema_version),
            source_hash: Some(&report.source_hash),
            coverage: Some(&report.coverage),
            join: Some(JoinJson::from(&context.operand_join)),
            handler: context.lifted,
            reason: None,
        },
        AnalysisData::Unavailable { reason } => OperandSeparationAnalysisJson {
            status: "unavailable",
            schema_version: None,
            source_hash: None,
            coverage: None,
            join: None,
            handler: None,
            reason: Some(reason),
        },
    };
    AnalysesJson {
        handler_templates,
        operand_separation,
        recipe_admission: RecipeAdmissionJson {
            status: context.admission.status,
            basis: context.admission.basis,
            code: context.admission.code,
            detail: context.admission.detail.as_deref(),
        },
    }
}

pub(super) struct ListHandlers<'a>(pub(super) &'a [OperationRecord]);

impl Serialize for ListHandlers<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut handlers = serializer.serialize_seq(Some(self.0.len()))?;
        for handler in self.0 {
            handlers.serialize_element(&ListHandlerJson(handler))?;
        }
        handlers.end()
    }
}

pub(super) struct ListHandlerJson<'a>(pub(super) &'a OperationRecord);

impl Serialize for ListHandlerJson<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let handler = self.0;
        let mut record = serializer.serialize_struct("Handler", 8)?;
        record.serialize_field("key", &handler.table_key)?;
        record.serialize_field("index", &handler.handler_index)?;
        record.serialize_field("family", handler.semantic_family.as_str())?;
        record.serialize_field("classification", handler.classification.as_str())?;
        record.serialize_field(
            "micro_ops",
            &MicroOperationKinds(&handler.ordered_micro_operations),
        )?;
        record.serialize_field("confidence", handler.confidence().level().as_str())?;
        record.serialize_field("unknown_fragments", &handler.unknown_fragments.len())?;
        record.serialize_field("fingerprint", &handler.structural_fingerprint)?;
        record.end()
    }
}

pub(super) struct MicroOperationKinds<'a>(pub(super) &'a [MicroOperation]);

impl Serialize for MicroOperationKinds<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut kinds = serializer.serialize_seq(Some(self.0.len()))?;
        for operation in self.0 {
            kinds.serialize_element(operation.kind.as_str())?;
        }
        kinds.end()
    }
}

pub(super) struct ShowHandlerJson<'a>(pub(super) &'a OperationRecord);

impl Serialize for ShowHandlerJson<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let handler = self.0;
        let mut record = serializer.serialize_struct("Handler", 13)?;
        record.serialize_field("key", &handler.table_key)?;
        record.serialize_field("index", &handler.handler_index)?;
        record.serialize_field("kernel_identity", &handler.kernel_identity)?;
        record.serialize_field("source_span", &handler.source_span)?;
        record.serialize_field("source_snippet", &handler.source_snippet)?;
        record.serialize_field("structural_fingerprint", &handler.structural_fingerprint)?;
        record.serialize_field(
            "canonical_normalized_body",
            &handler.canonical_normalized_body,
        )?;
        record.serialize_field("semantic_family", &handler.semantic_family)?;
        record.serialize_field("classification", &handler.classification)?;
        record.serialize_field(
            "ordered_micro_operations",
            &handler.ordered_micro_operations,
        )?;
        record.serialize_field("effects", &handler.effects)?;
        record.serialize_field("confidence", &handler.confidence())?;
        record.serialize_field("unknown_fragments", &handler.unknown_fragments)?;
        record.end()
    }
}

pub(super) fn capture_json<'a>(capture: &Capture<'a>) -> CaptureJson<'a> {
    CaptureJson {
        input_kind: capture.input_kind,
        identity: Identity {
            algorithm: "sha256",
            digest: capture.digest,
        },
        byte_length: capture.byte_length,
        raw_identity: capture.raw_identity,
    }
}

pub(super) fn derived_from_json<'a>(catalogue: &'a OperationCatalogue) -> DerivedFromJson<'a> {
    DerivedFromJson {
        report: &catalogue.schema_version,
        catalogue_version: &catalogue.catalogue_version,
        source_hash: Identity {
            algorithm: "fnv1a64",
            digest: &catalogue.source_hash,
        },
    }
}

pub(super) fn span_source_json<'a>(source: &SpanSource<'a>) -> SpanSourceJson<'a> {
    SpanSourceJson {
        kind: "deobfuscated",
        identity: Identity {
            algorithm: "sha256",
            digest: source.digest,
        },
        byte_length: source.byte_length,
    }
}
