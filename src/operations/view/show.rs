use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JoinStatus {
    Joined,
    Unmatched,
    Ambiguous,
    Inconsistent,
    Unavailable,
}

impl JoinStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Joined => "joined",
            Self::Unmatched => "unmatched",
            Self::Ambiguous => "ambiguous",
            Self::Inconsistent => "inconsistent",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IdentityStatus {
    Verified,
    Mismatch,
    Unavailable,
}

impl IdentityStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Mismatch => "mismatch",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct JoinReport {
    pub(super) status: JoinStatus,
    pub(super) table_key: Option<u32>,
    pub(super) handler_index: Option<u32>,
    pub(super) table_identity: IdentityStatus,
    pub(super) source_identity: IdentityStatus,
    pub(super) span_identity: IdentityStatus,
    pub(super) reasons: Vec<String>,
}

impl JoinReport {
    pub(super) fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            status: JoinStatus::Unavailable,
            table_key: None,
            handler_index: None,
            table_identity: IdentityStatus::Unavailable,
            source_identity: IdentityStatus::Unavailable,
            span_identity: IdentityStatus::Unavailable,
            reasons: vec![reason.into()],
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct AdmissionReport {
    pub(super) status: &'static str,
    pub(super) basis: Option<&'static str>,
    pub(super) code: Option<&'static str>,
    pub(super) detail: Option<String>,
}

impl AdmissionReport {
    pub(super) fn not_included() -> Self {
        Self {
            status: "not_included",
            basis: None,
            code: None,
            detail: None,
        }
    }

    pub(super) fn unavailable(reason: String) -> Self {
        Self {
            status: "unavailable",
            basis: None,
            code: Some("missing_context"),
            detail: Some(reason),
        }
    }
}

pub(super) struct ShowContext<'a> {
    pub(super) template: Option<&'a HandlerTemplate>,
    pub(super) lifted: Option<&'a LiftedHandler>,
    pub(super) operands_report: Option<&'a JsOperandSeparationReport>,
    pub(super) template_join: JoinReport,
    pub(super) operand_join: JoinReport,
    pub(super) admission: AdmissionReport,
}

pub(super) fn show_context<'a>(
    catalogue: &'a OperationCatalogue,
    analyses: Option<&'a Analyses>,
    handler: &'a OperationRecord,
) -> ShowContext<'a> {
    let Some(analyses) = analyses else {
        return ShowContext {
            template: None,
            lifted: None,
            operands_report: None,
            template_join: JoinReport::unavailable("handler-template analysis not included"),
            operand_join: JoinReport::unavailable("operand-separation analysis not included"),
            admission: AdmissionReport::not_included(),
        };
    };
    let (template, template_join) = join_template(catalogue, analyses, handler);
    let (lifted, operand_join) = join_operands(catalogue, analyses, handler);
    let admission = admission_report(template, lifted, &template_join);
    ShowContext {
        template,
        lifted,
        operands_report: analyses.operands(),
        template_join,
        operand_join,
        admission,
    }
}

pub(super) fn join_template<'a>(
    catalogue: &'a OperationCatalogue,
    analyses: &'a Analyses,
    handler: &'a OperationRecord,
) -> (Option<&'a HandlerTemplate>, JoinReport) {
    let Some(report) = analyses.templates() else {
        let reason = analyses
            .templates
            .reason()
            .unwrap_or("handler-template analysis unavailable")
            .to_owned();
        return (None, JoinReport::unavailable(reason));
    };
    let mut reasons = Vec::new();
    let mut table_identity = IdentityStatus::Verified;
    let source_identity = if report.source_hash == analyses.source_hash {
        IdentityStatus::Verified
    } else {
        reasons.push(format!(
            "handler-template source hash {} differs from report source hash {}",
            report.source_hash, analyses.source_hash
        ));
        IdentityStatus::Mismatch
    };
    if let Some(reason) =
        compare_table_identity(catalogue, &report.table, &report.source_hash, analyses)
    {
        reasons.push(reason);
        table_identity = IdentityStatus::Mismatch;
    }
    let rows = report
        .handlers
        .iter()
        .filter(|row| row.table_key == handler.table_key)
        .collect::<Vec<_>>();
    let mut span_identity = IdentityStatus::Unavailable;
    let mut table_key = Some(handler.table_key);
    let mut handler_index = None;
    let status = match rows.as_slice() {
        [] => {
            reasons.push(format!(
                "no handler-template row for table key {}",
                handler.table_key
            ));
            JoinStatus::Unmatched
        }
        [row, ..] => {
            if rows.len() > 1 {
                reasons.push(format!(
                    "{} handler-template rows share table key {}",
                    rows.len(),
                    handler.table_key
                ));
                JoinStatus::Ambiguous
            } else {
                table_key = Some(row.table_key);
                handler_index = Some(row.handler_index);
                if row.handler_index != row.table_key {
                    reasons.push(format!(
                        "handler-template row is self-inconsistent: table_key {} but handler_index {}",
                        row.table_key, row.handler_index
                    ));
                }
                if row.handler_index != handler.handler_index {
                    reasons.push(format!(
                        "handler-template handler_index {} differs from catalogue handler_index {}",
                        row.handler_index, handler.handler_index
                    ));
                }
                if row.source_span.start != handler.source_span.start
                    || row.source_span.end != handler.source_span.end
                {
                    reasons.push(format!(
                        "handler-template span {}..{} differs from catalogue span {}..{}",
                        row.source_span.start,
                        row.source_span.end,
                        handler.source_span.start,
                        handler.source_span.end
                    ));
                    span_identity = IdentityStatus::Mismatch;
                } else {
                    span_identity = IdentityStatus::Verified;
                }
                if reasons.is_empty() {
                    JoinStatus::Joined
                } else {
                    JoinStatus::Inconsistent
                }
            }
        }
    };
    let selected = if status == JoinStatus::Joined {
        rows.first().copied()
    } else {
        None
    };
    (
        selected,
        JoinReport {
            status,
            table_key,
            handler_index,
            table_identity,
            source_identity,
            span_identity,
            reasons,
        },
    )
}

pub(super) fn join_operands<'a>(
    catalogue: &'a OperationCatalogue,
    analyses: &'a Analyses,
    handler: &'a OperationRecord,
) -> (Option<&'a LiftedHandler>, JoinReport) {
    let Some(report) = analyses.operands() else {
        let reason = analyses
            .operands
            .reason()
            .unwrap_or("operand-separation analysis unavailable")
            .to_owned();
        return (None, JoinReport::unavailable(reason));
    };
    let mut reasons = Vec::new();
    let mut table_identity = IdentityStatus::Verified;
    let source_identity = if report.source_hash == analyses.source_hash {
        IdentityStatus::Verified
    } else {
        reasons.push(format!(
            "operand-separation source hash {} differs from report source hash {}",
            report.source_hash, analyses.source_hash
        ));
        IdentityStatus::Mismatch
    };
    if let Some(reason) =
        compare_table_identity(catalogue, &report.table, &report.source_hash, analyses)
    {
        reasons.push(reason);
        table_identity = IdentityStatus::Mismatch;
    }
    let rows = report
        .handlers
        .iter()
        .filter(|row| row.table_key == handler.table_key)
        .collect::<Vec<_>>();
    let mut span_identity = IdentityStatus::Unavailable;
    let mut table_key = Some(handler.table_key);
    let mut handler_index = None;
    let status = match rows.as_slice() {
        [] => {
            reasons.push(format!(
                "no operand-separation row for table key {}",
                handler.table_key
            ));
            JoinStatus::Unmatched
        }
        [row, ..] => {
            if rows.len() > 1 {
                reasons.push(format!(
                    "{} operand-separation rows share table key {}",
                    rows.len(),
                    handler.table_key
                ));
                JoinStatus::Ambiguous
            } else {
                table_key = Some(row.table_key);
                handler_index = Some(row.handler_index);
                if row.handler_index != row.table_key {
                    reasons.push(format!(
                        "operand-separation row is self-inconsistent: table_key {} but handler_index {}",
                        row.table_key, row.handler_index
                    ));
                }
                if row.handler_index != handler.handler_index {
                    reasons.push(format!(
                        "operand-separation handler_index {} differs from catalogue handler_index {}",
                        row.handler_index, handler.handler_index
                    ));
                }
                if row.source_span.start != handler.source_span.start
                    || row.source_span.end != handler.source_span.end
                {
                    reasons.push(format!(
                        "operand-separation span {}..{} differs from catalogue span {}..{}",
                        row.source_span.start,
                        row.source_span.end,
                        handler.source_span.start,
                        handler.source_span.end
                    ));
                    span_identity = IdentityStatus::Mismatch;
                } else {
                    span_identity = IdentityStatus::Verified;
                }
                if row.template.table_key != row.table_key
                    || row.template.handler_index != row.handler_index
                {
                    reasons.push(format!(
                        "lifted template identity ({}/{}) differs from operand row identity ({}/{})",
                        row.template.table_key,
                        row.template.handler_index,
                        row.table_key,
                        row.handler_index
                    ));
                }
                if row.template.source_span.start != row.source_span.start
                    || row.template.source_span.end != row.source_span.end
                {
                    reasons.push("lifted template span differs from the operand row span".into());
                    span_identity = IdentityStatus::Mismatch;
                }
                if reasons.is_empty() {
                    JoinStatus::Joined
                } else {
                    JoinStatus::Inconsistent
                }
            }
        }
    };
    let selected = if status == JoinStatus::Joined {
        rows.first().copied()
    } else {
        None
    };
    (
        selected,
        JoinReport {
            status,
            table_key,
            handler_index,
            table_identity,
            source_identity,
            span_identity,
            reasons,
        },
    )
}

pub(super) fn compare_table_identity(
    catalogue: &OperationCatalogue,
    identity: &ProvenOperationTable,
    source_hash: &str,
    analyses: &Analyses,
) -> Option<String> {
    if catalogue.source_hash != analyses.source_hash {
        return Some(format!(
            "catalogue source hash {} differs from report source hash {}",
            catalogue.source_hash, analyses.source_hash
        ));
    }
    if catalogue.table.declaration_ordinal != identity.declaration_ordinal {
        return Some(format!(
            "declaration ordinal {} differs from catalogue ordinal {}",
            identity.declaration_ordinal, catalogue.table.declaration_ordinal
        ));
    }
    if catalogue.table.canonical_name != identity.canonical_name {
        return Some(format!(
            "canonical table name {} differs from catalogue name {}",
            identity.canonical_name, catalogue.table.canonical_name
        ));
    }
    if catalogue.table.original_name != identity.original_name {
        return Some(format!(
            "original table name {} differs from catalogue name {}",
            identity.original_name, catalogue.table.original_name
        ));
    }
    if catalogue.table.element_count != identity.initializer_elements
        || catalogue.table.function_count != identity.initializer_functions
    {
        return Some(format!(
            "catalogue counts {}/{} differ from analysis counts {}/{}",
            catalogue.table.element_count,
            catalogue.table.function_count,
            identity.initializer_elements,
            identity.initializer_functions
        ));
    }
    if let Some(operation_table) = &analyses.operation_table {
        if operation_table.declaration_ordinal != identity.declaration_ordinal
            || operation_table.canonical_name != identity.canonical_name
            || operation_table.initializer_elements != identity.initializer_elements
            || operation_table.initializer_functions != identity.initializer_functions
        {
            return Some("analysis table identity differs from the pipeline report".into());
        }
    }
    if source_hash != analyses.source_hash {
        return Some(format!(
            "analysis source hash {source_hash} differs from report source hash {}",
            analyses.source_hash
        ));
    }
    None
}

pub(super) fn admission_report(
    template: Option<&HandlerTemplate>,
    lifted: Option<&LiftedHandler>,
    template_join: &JoinReport,
) -> AdmissionReport {
    let Some(template) = template else {
        return AdmissionReport::unavailable(format!(
            "handler template join is {}: {}",
            template_join.status.as_str(),
            template_join.reasons.join("; ")
        ));
    };
    if let Some(lifted) = lifted {
        if lifted.status == OperandHandlerStatus::Lifted
            && admit_recipe(&lifted.template, RecipeBounds::default()).is_ok()
        {
            return AdmissionReport {
                status: "admitted",
                basis: Some("operand_lifted_recipe"),
                code: None,
                detail: None,
            };
        }
    }
    match admit_recipe(template, RecipeBounds::default()) {
        Ok(()) => AdmissionReport {
            status: "admitted",
            basis: Some("extracted_recipe"),
            code: None,
            detail: None,
        },
        Err(error) => AdmissionReport {
            status: "rejected",
            basis: Some("extracted_recipe"),
            code: Some(admission_code(&error)),
            detail: Some(error.to_string()),
        },
    }
}

pub(super) const fn admission_code(error: &AdmissionError) -> &'static str {
    match error {
        AdmissionError::TemplateStatus { .. } => "template_status",
        AdmissionError::UnsupportedFragment { .. } => "unsupported_fragment",
        AdmissionError::StepLimit { .. } => "step_limit",
        AdmissionError::StepOrder { .. } => "step_order",
        AdmissionError::UnknownOperation { .. } => "unknown_operation",
        AdmissionError::UnknownValue { .. } => "unknown_value",
        AdmissionError::ContextValue { .. } => "context_value",
        AdmissionError::TemporaryReadBeforeLet { .. } => "temporary_read_before_let",
        AdmissionError::TemporaryAssignedBeforeLet { .. } => "temporary_assigned_before_let",
        AdmissionError::DuplicateTemporary { .. } => "duplicate_temporary",
        AdmissionError::ExpressionWork { .. } => "expression_work",
        AdmissionError::ExpressionDepth { .. } => "expression_depth",
    }
}

pub(super) fn template_status_cell(context: &ShowContext<'_>) -> &'static str {
    match context.template_join.status {
        JoinStatus::Joined => context
            .template
            .map(|template| template_status_str(template.status))
            .unwrap_or("unavailable"),
        other => other.as_str(),
    }
}

pub(super) fn template_detail_cell(context: &ShowContext<'_>) -> String {
    match context.template {
        Some(template) => format!(
            "{}; {}; {}",
            count_noun(template.steps.len(), "step"),
            count_noun(template.unsupported.len(), "unsupported fragment"),
            join_detail(
                &context.template_join,
                template.table_key,
                template.handler_index
            )
        ),
        None => format!(
            "{}: {}",
            context.template_join.status.as_str(),
            context.template_join.reasons.join("; ")
        ),
    }
}

pub(super) const fn template_status_str(status: TemplateStatus) -> &'static str {
    match status {
        TemplateStatus::Executable => "executable",
        TemplateStatus::Partial => "partial",
        TemplateStatus::Unsupported => "unsupported",
    }
}

pub(super) fn operand_status_cell(context: &ShowContext<'_>) -> &'static str {
    match context.operand_join.status {
        JoinStatus::Joined => context
            .lifted
            .map(|lifted| operand_status_str(&lifted.status))
            .unwrap_or("unavailable"),
        other => other.as_str(),
    }
}

pub(super) const fn operand_status_str(status: &OperandHandlerStatus) -> &'static str {
    match status {
        OperandHandlerStatus::Lifted => "lifted",
        OperandHandlerStatus::Partial => "partial",
        OperandHandlerStatus::Unresolved => "unresolved",
        OperandHandlerStatus::NotApplicable => "not_applicable",
    }
}

pub(super) fn operand_detail_cell(context: &ShowContext<'_>) -> String {
    match context.lifted {
        Some(lifted) => {
            let mut detail = format!(
                "{}; PC delta {}; {}",
                count_noun(lifted.operands.len(), "operand"),
                lifted.pc_delta,
                join_detail(
                    &context.operand_join,
                    lifted.table_key,
                    lifted.handler_index
                )
            );
            if !lifted.unresolved.is_empty() {
                detail.push_str(&format!("; unresolved: {}", lifted.unresolved.join("; ")));
            }
            detail
        }
        None => format!(
            "{}: {}",
            context.operand_join.status.as_str(),
            context.operand_join.reasons.join("; ")
        ),
    }
}

pub(super) fn admission_detail_cell(admission: &AdmissionReport) -> String {
    let mut detail = String::new();
    if let Some(basis) = admission.basis {
        detail.push_str(&format!("basis: {basis}"));
    }
    if let Some(code) = admission.code {
        if !detail.is_empty() {
            detail.push_str("; ");
        }
        detail.push_str(&format!("code: {code}"));
    }
    if let Some(reason) = &admission.detail {
        if !detail.is_empty() {
            detail.push_str("; ");
        }
        detail.push_str(reason);
    }
    detail
}

pub(super) fn join_detail(join: &JoinReport, table_key: u32, handler_index: u32) -> String {
    if join.status == JoinStatus::Joined {
        format!("joined by `table_key` {table_key} (row `handler_index` {handler_index})")
    } else {
        format!("{}: {}", join.status.as_str(), join.reasons.join("; "))
    }
}

pub(super) fn join_reason_suffix(join: &JoinReport) -> String {
    if join.reasons.is_empty() {
        String::new()
    } else {
        format!(": {}", join.reasons.join("; "))
    }
}

pub(super) fn describe_handler(context: &ShowContext<'_>, handler: &OperationRecord) -> String {
    let mut out = format!(
        "{} handler with {} confidence; {} recognized: {}; {}.",
        capitalize(handler.classification.as_str()),
        handler.confidence().level().as_str(),
        count_noun(
            handler.ordered_micro_operations.len(),
            "ordered micro-operation"
        ),
        micro_operation_summary(handler),
        count_noun(handler.unknown_fragments.len(), "unknown fragment")
    );
    match context.template {
        Some(template) => out.push_str(&format!(
            " The extracted recipe is {} ({}, {}).",
            template_status_str(template.status),
            count_noun(template.steps.len(), "step"),
            count_noun(template.unsupported.len(), "unsupported fragment")
        )),
        None => out.push_str(&format!(
            " The extracted recipe is not available ({}).",
            context.template_join.status.as_str()
        )),
    }
    match (context.admission.status, context.admission.basis) {
        ("admitted", Some("operand_lifted_recipe")) => {
            out.push_str(" Recipe admission succeeded on the operand-lifted recipe.");
        }
        ("admitted", Some("extracted_recipe")) => {
            out.push_str(" Recipe admission succeeded on the extracted recipe.");
        }
        ("admitted", _) => out.push_str(" Recipe admission succeeded."),
        ("rejected", _) => out.push_str(&format!(
            " Recipe admission was rejected on the extracted recipe: {}.",
            context
                .admission
                .detail
                .as_deref()
                .unwrap_or("unknown admission failure")
        )),
        ("unavailable", _) => out.push_str(&format!(
            " Recipe admission is unavailable: {}.",
            context
                .admission
                .detail
                .as_deref()
                .unwrap_or("missing context")
        )),
        ("not_included", _) => out.push_str(" Recipe admission was not included."),
        _ => {}
    }
    match context.lifted {
        Some(lifted) => {
            out.push_str(&format!(
                " Operand lifting is {} with {}, PC delta {}",
                operand_status_str(&lifted.status),
                count_noun(lifted.operands.len(), "certified operand"),
                lifted.pc_delta
            ));
            if !lifted.unresolved.is_empty() {
                out.push_str(&format!("; unresolved: {}", lifted.unresolved.join("; ")));
            }
            out.push('.');
            let roles = collect_operand_roles(lifted);
            for descriptor in &lifted.operands {
                let summary = roles
                    .get(&descriptor.position)
                    .map(|uses| unique_role_labels(uses).join(" / "))
                    .filter(|summary| !summary.is_empty())
                    .unwrap_or_else(|| "role unresolved".into());
                out.push_str(&format!(
                    " Operand {} is used as {summary}.",
                    descriptor.position
                ));
            }
        }
        None => out.push_str(&format!(
            " Operand lifting is not available ({}).",
            context.operand_join.status.as_str()
        )),
    }
    out
}

pub(super) fn micro_operation_summary(handler: &OperationRecord) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for operation in &handler.ordered_micro_operations {
        let kind = operation.kind.as_str();
        if let Some(entry) = counts.iter_mut().find(|(seen, _)| *seen == kind) {
            entry.1 += 1;
        } else {
            counts.push((kind, 1));
        }
    }
    counts
        .into_iter()
        .map(|(kind, count)| format!("{kind} ×{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn count_noun(count: usize, singular: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {singular}s")
    }
}

pub(super) fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

pub(super) fn unresolved_lines(context: &ShowContext<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    if context.template_join.status != JoinStatus::Joined {
        lines.push(format!(
            "handler templates ({}): {}",
            context.template_join.status.as_str(),
            context.template_join.reasons.join("; ")
        ));
    }
    if context.operand_join.status != JoinStatus::Joined {
        lines.push(format!(
            "operand separation ({}): {}",
            context.operand_join.status.as_str(),
            context.operand_join.reasons.join("; ")
        ));
    }
    if let Some(lifted) = context.lifted {
        for unresolved in &lifted.unresolved {
            lines.push(format!("operand separation: {unresolved}"));
        }
    }
    if context.admission.status == "rejected" {
        if let Some(detail) = &context.admission.detail {
            lines.push(format!("recipe admission: {detail}"));
        }
    }
    lines
}

pub(super) fn recipe_operation_name(operation: &RecipeOperation) -> &'static str {
    match operation {
        RecipeOperation::Let { .. } => "let",
        RecipeOperation::Assign { .. } => "assign",
        RecipeOperation::StackWrite { .. } => "stack_write",
        RecipeOperation::StackLengthSet { .. } => "stack_length_set",
        RecipeOperation::MemoryWrite { .. } => "memory_write",
        RecipeOperation::MemoryInitialize { .. } => "memory_initialize",
        RecipeOperation::PropertyWrite { .. } => "property_write",
        RecipeOperation::PushCallFrame => "push_call_frame",
        RecipeOperation::PopCallFrame => "pop_call_frame",
        RecipeOperation::SetProgramCounter { .. } => "set_program_counter",
        RecipeOperation::SetDispatchState { .. } => "set_dispatch_state",
        RecipeOperation::PushTryFrame { .. } => "push_try_frame",
        RecipeOperation::PopTryFrame => "pop_try_frame",
        RecipeOperation::PushFinallyRecord => "push_finally_record",
        RecipeOperation::PopErrorRecord => "pop_error_record",
        RecipeOperation::RethrowIfHandled => "rethrow_if_handled",
        RecipeOperation::DropThrownIfHandled => "drop_thrown_if_handled",
        RecipeOperation::DropThrownUnderIfHandled => "drop_thrown_under_if_handled",
        RecipeOperation::CompleteThread { .. } => "complete_thread",
        RecipeOperation::JumpIf { .. } => "jump_if",
        RecipeOperation::Evaluate { .. } => "evaluate",
        RecipeOperation::Return { .. } => "return",
        RecipeOperation::Throw { .. } => "throw",
        RecipeOperation::Unknown { .. } => "unknown",
    }
}

// ---------------------------------------------------------------------------
// Operand roles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoleUse {
    label: &'static str,
    step: u32,
    via_temporary: Option<u32>,
}

pub(super) fn collect_operand_roles(lifted: &LiftedHandler) -> BTreeMap<u32, Vec<RoleUse>> {
    collect_template_roles(&lifted.template)
}

pub(super) fn collect_template_roles(template: &HandlerTemplate) -> BTreeMap<u32, Vec<RoleUse>> {
    // A temporary is not an operand: record which operand position(s) its
    // `Let` consumed, so later uses of that temporary report the operand's
    // proven role instead of a temporary id as a fake operand position.
    let mut temporary_origins: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for step in &template.steps {
        if let RecipeOperation::Let { temporary, value } = &step.operation {
            let mut positions = Vec::new();
            operand_positions(value, &mut positions);
            if !positions.is_empty() {
                temporary_origins.insert(*temporary, positions);
            }
        }
    }
    let mut roles: BTreeMap<u32, Vec<RoleUse>> = BTreeMap::new();
    for step in &template.steps {
        for (label, value) in operation_role_values(&step.operation) {
            walk_role_values(
                value,
                label,
                step.order,
                &temporary_origins,
                None,
                &mut roles,
            );
        }
    }
    roles
}

pub(super) fn operand_positions(value: &SemanticValue, out: &mut Vec<u32>) {
    match value {
        SemanticValue::Operand { position } => {
            if !out.contains(position) {
                out.push(*position);
            }
        }
        other => {
            for child in other.children() {
                operand_positions(child, out);
            }
        }
    }
}

pub(super) fn operation_role_values(
    operation: &RecipeOperation,
) -> Vec<(&'static str, &SemanticValue)> {
    match operation {
        RecipeOperation::Let { value, .. } | RecipeOperation::Assign { value, .. } => {
            vec![("temporary value", value)]
        }
        RecipeOperation::StackWrite { index, value } => {
            vec![("stack write index", index), ("stack write value", value)]
        }
        RecipeOperation::StackLengthSet { value } => vec![("stack length", value)],
        RecipeOperation::MemoryWrite { slot, value } => {
            vec![("memory write slot", slot), ("memory write value", value)]
        }
        RecipeOperation::MemoryInitialize { slot } => vec![("memory initialize slot", slot)],
        RecipeOperation::PropertyWrite { object, key, value } => vec![
            ("property write object", object),
            ("property write key", key),
            ("property write value", value),
        ],
        RecipeOperation::SetProgramCounter { value } => vec![("next program counter", value)],
        RecipeOperation::SetDispatchState { value } => vec![("next dispatch state", value)],
        RecipeOperation::PushTryFrame {
            byte_offset,
            dispatch_state,
        } => vec![
            ("try handler byte offset", byte_offset),
            ("try handler dispatch state", dispatch_state),
        ],
        RecipeOperation::CompleteThread { value } => vec![("completed thread value", value)],
        RecipeOperation::JumpIf {
            condition,
            byte_offset,
            dispatch_state,
        } => vec![
            ("jump condition", condition),
            ("jump byte offset", byte_offset),
            ("jump dispatch state", dispatch_state),
        ],
        RecipeOperation::Evaluate { value } => vec![("evaluated value", value)],
        RecipeOperation::Return { value } => vec![("return value", value)],
        RecipeOperation::Throw { value } => vec![("thrown value", value)],
        _ => Vec::new(),
    }
}

pub(super) fn walk_role_values(
    value: &SemanticValue,
    label: &'static str,
    step: u32,
    temporary_origins: &BTreeMap<u32, Vec<u32>>,
    via_temporary: Option<u32>,
    roles: &mut BTreeMap<u32, Vec<RoleUse>>,
) {
    match value {
        SemanticValue::Operand { position } => {
            push_role(roles, *position, label, step, via_temporary);
        }
        SemanticValue::Temporary { id } => {
            if let Some(positions) = temporary_origins.get(id) {
                for position in positions {
                    push_role(roles, *position, label, step, Some(*id));
                }
            }
        }
        other => {
            for child in other.children() {
                walk_role_values(child, label, step, temporary_origins, via_temporary, roles);
            }
        }
    }
}

pub(super) fn push_role(
    roles: &mut BTreeMap<u32, Vec<RoleUse>>,
    position: u32,
    label: &'static str,
    step: u32,
    via_temporary: Option<u32>,
) {
    let entry = roles.entry(position).or_default();
    let use_site = RoleUse {
        label,
        step,
        via_temporary,
    };
    if !entry.contains(&use_site) {
        entry.push(use_site);
    }
}

pub(super) fn format_role_uses(uses: &[RoleUse]) -> String {
    uses.iter()
        .map(|use_site| match use_site.via_temporary {
            Some(temporary) => format!(
                "{} (step {}, via temporary {temporary})",
                use_site.label, use_site.step
            ),
            None => format!("{} (step {})", use_site.label, use_site.step),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn unique_role_labels(uses: &[RoleUse]) -> Vec<&'static str> {
    let mut labels = Vec::new();
    for use_site in uses {
        if !labels.contains(&use_site.label) {
            labels.push(use_site.label);
        }
    }
    labels
}

pub(super) fn format_mask(mask: Option<u32>) -> String {
    match mask {
        Some(mask) => format!("0x{mask:x}"),
        None => "none".into(),
    }
}

pub(super) const fn endian_str(endian: OperandEndian) -> &'static str {
    match endian {
        OperandEndian::Little => "little",
        OperandEndian::Big => "big",
    }
}

pub(super) const fn entry_coordinate_str(coordinate: OperandEntryCoordinate) -> &'static str {
    match coordinate {
        OperandEntryCoordinate::AfterOpcode => "after_opcode",
        OperandEntryCoordinate::AtOpcode => "at_opcode",
    }
}

// ---------------------------------------------------------------------------
// JSON
