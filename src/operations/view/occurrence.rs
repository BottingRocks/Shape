use super::*;

// ---------------------------------------------------------------------------
// Instruction occurrence projection and decode classifier
// ---------------------------------------------------------------------------

/// Outcome of classifying operand decoding for one occurrence or prospective
/// coordinate. `MissingContext` means a required proof input is absent;
/// `Truncated` means the image ended inside a certified operand; `Unsupported`
/// means the certified descriptor itself cannot be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeStatus {
    Decoded,
    MissingContext,
    Truncated,
    Unsupported,
    NotApplicable,
}

impl DecodeStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Decoded => "decoded",
            Self::MissingContext => "missing_context",
            Self::Truncated => "truncated",
            Self::Unsupported => "unsupported",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeClassification {
    pub status: DecodeStatus,
    pub operands: Vec<DecodedOperand>,
    pub reason: Option<String>,
    pub position: Option<u32>,
    pub required_end: Option<usize>,
    pub image_len: Option<usize>,
}

impl DecodeClassification {
    fn decoded(operands: Vec<DecodedOperand>) -> Self {
        Self {
            status: DecodeStatus::Decoded,
            operands,
            reason: None,
            position: None,
            required_end: None,
            image_len: None,
        }
    }

    fn missing(reason: impl Into<String>) -> Self {
        Self {
            status: DecodeStatus::MissingContext,
            operands: Vec::new(),
            reason: Some(reason.into()),
            position: None,
            required_end: None,
            image_len: None,
        }
    }

    fn not_applicable(reason: impl Into<String>) -> Self {
        Self {
            status: DecodeStatus::NotApplicable,
            operands: Vec::new(),
            reason: Some(reason.into()),
            position: None,
            required_end: None,
            image_len: None,
        }
    }

    fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            status: DecodeStatus::Unsupported,
            operands: Vec::new(),
            reason: Some(reason.into()),
            position: None,
            required_end: None,
            image_len: None,
        }
    }
}

/// Classify recorded operands carried by a committed dispatch record. A
/// completion record has no operands by contract, so it is not a decode
/// failure.
pub fn classify_recorded_operands(record: &DispatchExecutionRecord) -> DecodeClassification {
    if record.completion.is_some() {
        return DecodeClassification::not_applicable("completion record has no operands");
    }
    if record.operands.is_empty() {
        return DecodeClassification::missing(
            "execution record carries no decoded operands for a handler occurrence",
        );
    }
    DecodeClassification::decoded(record.operands.clone())
}

/// Classify a prospective decode of one lifted handler at an explicit entry PC.
///
/// The classifier never guesses: absent image, entry, handler, or certified
/// descriptors are missing context; a zero-width descriptor is unsupported;
/// a short image is truncated; only a complete bounded decode is `decoded`.
pub fn classify_decoded_operands(
    image: Option<&[u8]>,
    handler_entry_pc: Option<usize>,
    handler: Option<&LiftedHandler>,
) -> DecodeClassification {
    let Some(handler) = handler else {
        return DecodeClassification::missing("lifted handler context unavailable");
    };
    if handler.status == OperandHandlerStatus::Unresolved {
        return DecodeClassification::missing("operand lifting is unresolved");
    }
    if handler.operands.is_empty() {
        return if handler.status == OperandHandlerStatus::NotApplicable {
            DecodeClassification::not_applicable("handler has no certified inline operands")
        } else {
            DecodeClassification::missing("handler has no certified operand descriptors")
        };
    }
    if handler
        .operands
        .iter()
        .any(|descriptor| descriptor.width == 0)
    {
        return DecodeClassification::unsupported("zero-width operand descriptor");
    }
    let Some(image) = image else {
        return DecodeClassification::missing("bytecode image unavailable");
    };
    let Some(entry) = handler_entry_pc else {
        return DecodeClassification::missing("handler entry PC unavailable");
    };
    match bounded_decode_operands(image, entry, handler) {
        Ok(operands) => DecodeClassification::decoded(operands),
        Err(DecodeOperandsError::Truncated {
            position,
            required_end,
            image_len,
        }) => DecodeClassification {
            status: DecodeStatus::Truncated,
            operands: Vec::new(),
            reason: Some(format!(
                "operand {position} needs byte {required_end}, but image has {image_len} bytes"
            )),
            position: Some(position),
            required_end: Some(required_end),
            image_len: Some(image_len),
        },
        Err(DecodeOperandsError::Unsupported(reason)) => DecodeClassification::unsupported(reason),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccurrenceJoinStatus {
    /// The dispatch cell, selector, handler row, table key, catalogue row, and
    /// spans all agree.
    Joined,
    /// The record is the explicit terminal completion record.
    Completion,
    /// A required join row or catalogue row is absent.
    Unmatched,
    /// More than one row claims the selector or table key.
    Ambiguous,
    /// A row exists but an identity or span disagrees.
    Inconsistent,
}

impl OccurrenceJoinStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Joined => "joined",
            Self::Completion => "completion",
            Self::Unmatched => "unmatched",
            Self::Ambiguous => "ambiguous",
            Self::Inconsistent => "inconsistent",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OccurrenceJoin {
    pub status: OccurrenceJoinStatus,
    pub table_key: Option<u32>,
    pub handler_index: Option<u32>,
    pub detail: String,
    pub evidence: Vec<String>,
    pub missing_context: Vec<String>,
}

/// One instruction occurrence projected from a committed dispatch record.
pub struct OccurrenceProjection<'a> {
    pub record: &'a DispatchExecutionRecord,
    pub handler: Option<&'a OperationRecord>,
    pub template: Option<&'a HandlerTemplate>,
    pub lifted: Option<&'a LiftedHandler>,
    pub join: OccurrenceJoin,
    pub decode: DecodeClassification,
    pub source_hash: &'a str,
    pub artifact_source_hash: &'a str,
    pub image_len: usize,
    pub consumed_end: Option<usize>,
    pub sequential_operand_end: Option<usize>,
}

/// Project one committed dispatch record. The join is
/// `cell selector -> handler_index -> row table_key -> catalogue table_key`,
/// with every step validated; a broken step is reported as missing context.
pub fn project_occurrence<'a>(
    preparation: &'a DispatchPreparation,
    catalogue: &'a OperationCatalogue,
    record: &'a DispatchExecutionRecord,
) -> OccurrenceProjection<'a> {
    if record.completion.is_some() {
        return OccurrenceProjection {
            record,
            handler: None,
            template: None,
            lifted: None,
            join: OccurrenceJoin {
                status: OccurrenceJoinStatus::Completion,
                table_key: None,
                handler_index: None,
                detail: "terminal completion record".into(),
                evidence: vec![format!(
                    "dispatch PC {} committed a terminal completion record",
                    record.dispatch_pc
                )],
                missing_context: Vec::new(),
            },
            decode: classify_recorded_operands(record),
            source_hash: preparation.source_hash(),
            artifact_source_hash: preparation.artifact_source_hash(),
            image_len: preparation.bytecode().len(),
            consumed_end: None,
            sequential_operand_end: None,
        };
    }

    let selector = record.handler_key;
    let mut evidence = Vec::new();
    let mut missing_context = Vec::new();
    let mut status = OccurrenceJoinStatus::Joined;

    let template_rows = preparation
        .templates()
        .handlers
        .iter()
        .filter(|handler| handler.handler_index == selector)
        .collect::<Vec<_>>();
    let template = match template_rows.as_slice() {
        [] => {
            missing_context.push(format!("selector {selector} has no handler-template row"));
            status = OccurrenceJoinStatus::Unmatched;
            None
        }
        [row, ..] if template_rows.len() == 1 => {
            evidence.push(format!(
                "selector {selector} resolved to handler-template row table_key {}",
                row.table_key
            ));
            Some(*row)
        }
        _ => {
            missing_context.push(format!(
                "selector {selector} matches {} handler-template rows",
                template_rows.len()
            ));
            status = OccurrenceJoinStatus::Ambiguous;
            None
        }
    };

    let lifted_rows = preparation
        .operands()
        .handlers
        .iter()
        .filter(|handler| handler.handler_index == selector)
        .collect::<Vec<_>>();
    let lifted = match lifted_rows.as_slice() {
        [] => {
            missing_context.push(format!("selector {selector} has no operand-separation row"));
            if status == OccurrenceJoinStatus::Joined {
                status = OccurrenceJoinStatus::Unmatched;
            }
            None
        }
        [row, ..] if lifted_rows.len() == 1 => {
            evidence.push(format!(
                "selector {selector} resolved to operand-separation row table_key {}",
                row.table_key
            ));
            Some(*row)
        }
        _ => {
            missing_context.push(format!(
                "selector {selector} matches {} operand-separation rows",
                lifted_rows.len()
            ));
            status = OccurrenceJoinStatus::Ambiguous;
            None
        }
    };

    let table_key = template
        .map(|template| template.table_key)
        .or_else(|| lifted.map(|lifted| lifted.table_key));
    if let (Some(template), Some(lifted)) = (template, lifted) {
        if template.table_key != lifted.table_key {
            missing_context.push(format!(
                "template table_key {} differs from operand table_key {}",
                template.table_key, lifted.table_key
            ));
            status = OccurrenceJoinStatus::Inconsistent;
        }
    }
    let catalogue_rows = table_key.map_or_else(Vec::new, |key| {
        catalogue
            .handlers
            .iter()
            .filter(|handler| handler.table_key == key)
            .collect::<Vec<_>>()
    });
    let handler = match catalogue_rows.as_slice() {
        [] => {
            if let Some(key) = table_key {
                missing_context.push(format!("catalogue has no row for table_key {key}"));
            }
            if status == OccurrenceJoinStatus::Joined {
                status = OccurrenceJoinStatus::Unmatched;
            }
            None
        }
        [row, ..] if catalogue_rows.len() == 1 => {
            evidence.push(format!(
                "table_key {} resolved to catalogue handler key {} index {}",
                row.table_key, row.table_key, row.handler_index
            ));
            Some(*row)
        }
        _ => {
            missing_context.push(format!(
                "catalogue has {} rows for table_key {}",
                catalogue_rows.len(),
                table_key.unwrap_or_default()
            ));
            status = OccurrenceJoinStatus::Ambiguous;
            None
        }
    };

    // Validate the dispatch cell that selected the handler.
    match preparation
        .table()
        .get(record.state_before)
        .and_then(|row| row.get(record.opcode as usize))
    {
        Some(cell) if cell.selector == selector => {
            evidence.push(format!(
                "dispatch cell [{}][{}] selects handler {selector}",
                record.state_before, record.opcode
            ));
            if cell.next_state as usize != record.state_after {
                missing_context.push(format!(
                    "dispatch cell next_state {} differs from committed state_after {}",
                    cell.next_state, record.state_after
                ));
                status = OccurrenceJoinStatus::Inconsistent;
            }
        }
        Some(cell) => {
            missing_context.push(format!(
                "dispatch cell [{}][{}] selects handler {} but record committed handler {selector}",
                record.state_before, record.opcode, cell.selector
            ));
            status = OccurrenceJoinStatus::Inconsistent;
        }
        None => {
            missing_context.push(format!(
                "dispatch table has no cell for state {} opcode {}",
                record.state_before, record.opcode
            ));
            status = OccurrenceJoinStatus::Inconsistent;
        }
    }

    // Validate the operand-entry coordinate against the certified convention.
    if let Some(coordinate) = preparation.operands().entry_coordinate {
        let expected = match coordinate {
            OperandEntryCoordinate::AfterOpcode => record.dispatch_pc.saturating_add(1),
            OperandEntryCoordinate::AtOpcode => record.dispatch_pc,
        };
        if expected != record.operand_entry_pc {
            missing_context.push(format!(
                "operand entry PC {} differs from certified {} coordinate {}",
                record.operand_entry_pc,
                entry_coordinate_str(coordinate),
                expected
            ));
            status = OccurrenceJoinStatus::Inconsistent;
        } else {
            evidence.push(format!(
                "operand entry PC {} matches certified {} coordinate",
                record.operand_entry_pc,
                entry_coordinate_str(coordinate)
            ));
        }
    } else {
        missing_context.push("operand entry coordinate is unavailable".into());
        status = OccurrenceJoinStatus::Inconsistent;
    }

    // Validate handler spans across the join.
    if let (Some(handler), Some(template)) = (handler, template) {
        if record.source_span.start != template.source_span.start
            || record.source_span.end != template.source_span.end
        {
            missing_context.push(format!(
                "execution source span {}..{} differs from template span {}..{}",
                record.source_span.start,
                record.source_span.end,
                template.source_span.start,
                template.source_span.end
            ));
            status = OccurrenceJoinStatus::Inconsistent;
        }
        if record.source_span.start != handler.source_span.start
            || record.source_span.end != handler.source_span.end
        {
            missing_context.push(format!(
                "execution source span {}..{} differs from catalogue span {}..{}",
                record.source_span.start,
                record.source_span.end,
                handler.source_span.start,
                handler.source_span.end
            ));
            status = OccurrenceJoinStatus::Inconsistent;
        }
        if template.handler_index != template.table_key {
            missing_context.push(format!(
                "handler-template row is self-inconsistent: table_key {} but handler_index {}",
                template.table_key, template.handler_index
            ));
            status = OccurrenceJoinStatus::Inconsistent;
        }
    }

    let consumed_end = lifted.and_then(|lifted| {
        lifted
            .operands
            .iter()
            .map(|descriptor| descriptor.byte_offset as usize + descriptor.width as usize)
            .max()
            .map(|end| record.operand_entry_pc.saturating_add(end))
    });
    let sequential_operand_end = lifted.map(|lifted| {
        record
            .operand_entry_pc
            .saturating_add(lifted.pc_delta as usize)
    });

    let decode = if !record.operands.is_empty() {
        classify_recorded_operands(record)
    } else {
        classify_decoded_operands(
            Some(preparation.bytecode()),
            Some(record.operand_entry_pc),
            lifted,
        )
    };

    let detail = if status == OccurrenceJoinStatus::Joined {
        format!(
            "selector {selector} -> table_key {} -> catalogue key {}",
            table_key.unwrap_or_default(),
            handler.map(|h| h.table_key).unwrap_or_default()
        )
    } else {
        missing_context.join("; ")
    };

    OccurrenceProjection {
        record,
        handler,
        template,
        lifted,
        join: OccurrenceJoin {
            status,
            table_key,
            handler_index: handler.map(|handler| handler.handler_index),
            detail,
            evidence,
            missing_context,
        },
        decode,
        source_hash: preparation.source_hash(),
        artifact_source_hash: preparation.artifact_source_hash(),
        image_len: preparation.bytecode().len(),
        consumed_end,
        sequential_operand_end,
    }
}

#[derive(Serialize)]
struct OccurrenceCaptureJson<'a> {
    source_hash: Identity<'a>,
    artifact_source_hash: Identity<'a>,
    bytecode_byte_length: usize,
}

#[derive(Serialize)]
struct OccurrenceRecordJson<'a> {
    dispatch_pc: usize,
    operand_entry_pc: usize,
    state_before: usize,
    state_after: usize,
    opcode: u8,
    handler_key: u32,
    actual_next_pc: usize,
    recipe_identity: &'a str,
    source_span: &'a crate::operations::operands::OperandSpan,
    completion: Option<String>,
    consumed_end: Option<usize>,
    sequential_operand_end: Option<usize>,
}

#[derive(Serialize)]
struct OccurrenceJoinJson<'a> {
    status: &'static str,
    table_key: Option<u32>,
    handler_index: Option<u32>,
    detail: &'a str,
    evidence: &'a [String],
    missing_context: &'a [String],
}

#[derive(Serialize)]
struct OccurrenceOperandJson {
    position: u32,
    value: i64,
}

#[derive(Serialize)]
struct DecodeJson<'a> {
    status: &'static str,
    operands: Vec<OccurrenceOperandJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required_end: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_len: Option<usize>,
}

#[derive(Serialize)]
struct OccurrenceJson<'a> {
    capture: OccurrenceCaptureJson<'a>,
    occurrence: OccurrenceRecordJson<'a>,
    join: OccurrenceJoinJson<'a>,
    handler: Option<&'a OperationRecord>,
    template: Option<&'a HandlerTemplate>,
    operand_lifting: Option<&'a LiftedHandler>,
    decode: DecodeJson<'a>,
}

pub fn occurrence_json(projection: &OccurrenceProjection<'_>) -> String {
    let completion = projection
        .record
        .completion
        .map(|completion| match completion {
            crate::dispatch::CompletionValue::Stack { index } => format!("stack[{index}]"),
            crate::dispatch::CompletionValue::Undefined => "undefined".into(),
        });
    to_json(&OccurrenceJson {
        capture: OccurrenceCaptureJson {
            source_hash: Identity {
                algorithm: "fnv1a64",
                digest: projection.source_hash,
            },
            artifact_source_hash: Identity {
                algorithm: "fnv1a64",
                digest: projection.artifact_source_hash,
            },
            bytecode_byte_length: projection.image_len,
        },
        occurrence: OccurrenceRecordJson {
            dispatch_pc: projection.record.dispatch_pc,
            operand_entry_pc: projection.record.operand_entry_pc,
            state_before: projection.record.state_before,
            state_after: projection.record.state_after,
            opcode: projection.record.opcode,
            handler_key: projection.record.handler_key,
            actual_next_pc: projection.record.actual_next_pc,
            recipe_identity: &projection.record.recipe_identity,
            source_span: &projection.record.source_span,
            completion,
            consumed_end: projection.consumed_end,
            sequential_operand_end: projection.sequential_operand_end,
        },
        join: OccurrenceJoinJson {
            status: projection.join.status.as_str(),
            table_key: projection.join.table_key,
            handler_index: projection.join.handler_index,
            detail: &projection.join.detail,
            evidence: &projection.join.evidence,
            missing_context: &projection.join.missing_context,
        },
        handler: projection.handler,
        template: projection.template,
        operand_lifting: projection.lifted,
        decode: DecodeJson {
            status: projection.decode.status.as_str(),
            operands: projection
                .decode
                .operands
                .iter()
                .map(|operand| OccurrenceOperandJson {
                    position: operand.position,
                    value: operand.value,
                })
                .collect(),
            reason: projection.decode.reason.as_deref(),
            position: projection.decode.position,
            required_end: projection.decode.required_end,
            image_len: projection.decode.image_len,
        },
    })
}

pub fn occurrence_markdown(projection: &OccurrenceProjection<'_>) -> String {
    let record = projection.record;
    let mut out = String::from("# Instruction occurrence\n\n");
    out.push_str(&format!(
        "- Capture: `fnv1a64:{}` (artifact `fnv1a64:{}`; {} bytecode bytes)\n",
        projection.source_hash, projection.artifact_source_hash, projection.image_len
    ));
    out.push_str(&format!(
        "- Dispatch: PC {}, state {} -> {}\n",
        record.dispatch_pc, record.state_before, record.state_after
    ));
    out.push_str(&format!("- Opcode: {}\n", record.opcode));
    out.push_str(&format!(
        "- Selector / handler key: {}\n",
        record.handler_key
    ));
    out.push_str(&format!(
        "- Join: {} (table key {}, catalogue index {})\n",
        projection.join.status.as_str(),
        projection
            .join
            .table_key
            .map_or_else(|| "none".into(), |key| key.to_string()),
        projection
            .join
            .handler_index
            .map_or_else(|| "none".into(), |index| index.to_string())
    ));
    out.push_str(&format!(
        "- Operand entry PC: {}; consumed end: {}; sequential operand end: {}\n",
        record.operand_entry_pc,
        projection
            .consumed_end
            .map_or_else(|| "unknown".into(), |end| end.to_string()),
        projection
            .sequential_operand_end
            .map_or_else(|| "unknown".into(), |end| end.to_string())
    ));
    out.push_str(&format!(
        "- Actual next PC: {}; recipe identity: `{}`\n",
        record.actual_next_pc, record.recipe_identity
    ));
    out.push_str(&format!(
        "- Source span: {}..{}\n",
        record.source_span.start, record.source_span.end
    ));
    if let Some(completion) = &projection.record.completion {
        let value = match completion {
            crate::dispatch::CompletionValue::Stack { index } => format!("stack[{index}]"),
            crate::dispatch::CompletionValue::Undefined => "undefined".into(),
        };
        out.push_str(&format!("- Completion: {value}\n"));
    }

    out.push_str("\n## Decoded operands\n\n");
    out.push_str(&format!(
        "Decode status: `{}`.\n",
        projection.decode.status.as_str()
    ));
    if let Some(reason) = &projection.decode.reason {
        out.push_str(&format!("\n{reason}\n"));
    }
    if !projection.decode.operands.is_empty() {
        out.push_str("\n| Position | Value |\n|---:|---:|\n");
        for operand in &projection.decode.operands {
            out.push_str(&format!("| {} | {} |\n", operand.position, operand.value));
        }
    }

    out.push_str("\n## Join evidence\n\n");
    if projection.join.evidence.is_empty() {
        out.push_str("None.\n");
    } else {
        for evidence in &projection.join.evidence {
            out.push_str(&format!("- {evidence}\n"));
        }
    }
    out.push_str("\n## Missing context\n\n");
    if projection.join.missing_context.is_empty() {
        out.push_str("None.\n");
    } else {
        for missing in &projection.join.missing_context {
            out.push_str(&format!("- {missing}\n"));
        }
    }
    out
}
