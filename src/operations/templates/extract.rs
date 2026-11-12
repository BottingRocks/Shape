use super::build::{
    helper_handler, span_record, RecipeBuilder, COMPLETION_REGISTER, CREATE_THREAD,
};
use super::*;

/// One handler's legacy recipe plus the micro-op graph derived from the same walk.
struct Extracted {
    template: HandlerTemplate,
    graph: HandlerIR,
}

struct TemplateTableSink<'a> {
    shared_source: &'a Arc<str>,
    semantic_source_hash: &'a str,
    raw_input_hash: &'a str,
    table_declaration_ordinal: u32,
    table_element_count: u32,
    table_function_count: u32,
    scoping: &'a Scoping,
    proof: &'a Proof,
    catalogue_fingerprints: &'a BTreeMap<u32, String>,
    metadata: Option<(u32, u32)>,
    handlers: Vec<Extracted>,
}

impl TableSink for TemplateTableSink<'_> {
    fn table(&mut self, table: TableLiteral<'_>) {
        self.metadata = Some((table.element_count_saturating(), table.function_count));
        for entry in table.entries {
            let Some(handler) = entry.handler else {
                continue;
            };
            let key = entry.key;
            let span = handler.span();
            let params = function_params(handler.params());
            let mut recipe =
                RecipeBuilder::new(self.shared_source, self.scoping, self.proof, params, span);
            if let Some(body) = handler.body() {
                recipe.body(body);
            } else {
                recipe.unknown(span, "handler has no body", UnknownRegionKind::ControlFlow);
            }
            let semantic_identity = self
                .catalogue_fingerprints
                .get(&key)
                .cloned()
                .unwrap_or_else(|| {
                    fnv1a64_hex(
                        self.shared_source
                            .get(span.start as usize..span.end as usize)
                            .unwrap_or_default()
                            .as_bytes(),
                    )
                });
            let graph = recipe.finish(
                self.semantic_source_hash,
                self.raw_input_hash,
                self.table_declaration_ordinal,
                self.table_element_count,
                self.table_function_count,
                key,
                key,
                span_record(span),
            );
            // The builder owns this graph, so extraction projects through the
            // trusted path: no per-handler source rehash or artificial source
            // limit.  Public graphs must pass `validate` first.
            let mut status = graph.derived_status();
            let mut steps = graph.project_steps_trusted().unwrap_or_default();
            let mut unsupported = graph.project_unsupported();
            // A whole handler the recipe subset cannot say, written as a call
            // of an emitted helper that does exactly what it does.
            if status != TemplateStatus::Executable {
                let source = self
                    .shared_source
                    .get(span.start as usize..span.end as usize)
                    .unwrap_or_default();
                if let Some(helper) = helper_handler(source, span_record(span)) {
                    (status, steps, unsupported) = (TemplateStatus::Executable, helper, Vec::new());
                }
            }
            self.handlers.push(Extracted {
                template: HandlerTemplate {
                    handler_index: key,
                    table_key: key,
                    semantic_identity: semantic_identity.into(),
                    source_span: span_record(span),
                    status,
                    steps,
                    unsupported,
                },
                graph,
            });
        }
    }
}

struct ExtractedReport {
    identity: ProvenOperationTable,
    raw_input_hash: String,
    semantic_source_hash: String,
    source: Arc<str>,
    handlers: Vec<Extracted>,
}

/// Shared extraction: one parse, one table scan, one walk per handler, from
/// which both the legacy templates and the micro-op graphs are derived.
fn extract_handlers(report: &PipelineReport) -> Result<ExtractedReport, HandlerTemplateError> {
    let identity = report
        .proven_operation_tables
        .first()
        .ok_or(HandlerTemplateError::NoProvenTable)?
        .clone();
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, &report.output).map_err(HandlerTemplateError::Parse)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut proof = Proof::from_report(report);
    // The outer entry's first parameter is the JavaScript global object when
    // the entry call receives a host-presence expression (the same proof the
    // kernel entry scan requires, see `kernel::properties::scanner`). References
    // to that parameter are the tracer's canonical `host`, not an external name.
    proof.host_parameter = entry_host_parameter(&program, &scoping);
    let catalogue_fingerprints = catalogue::catalogue_from_report(report)
        .ok()
        .map(|catalogue| {
            catalogue
                .handlers
                .into_iter()
                .map(|h| (h.table_key, h.structural_fingerprint))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let shared_source: Arc<str> = Arc::from(report.output.as_str());
    let semantic_source_hash =
        crate::operations::graph::semantic_source_hash(shared_source.as_bytes());
    let mut sink = TemplateTableSink {
        shared_source: &shared_source,
        semantic_source_hash: &semantic_source_hash,
        raw_input_hash: &report.input_hash,
        table_declaration_ordinal: identity.declaration_ordinal,
        table_element_count: identity.initializer_elements,
        table_function_count: identity.initializer_functions,
        scoping: &scoping,
        proof: &proof,
        catalogue_fingerprints: &catalogue_fingerprints,
        metadata: None,
        handlers: Vec::new(),
    };
    scan_target_table(
        &program,
        identity.declaration_ordinal,
        TableScanMode::Strict,
        &mut sink,
    );
    let TemplateTableSink {
        metadata,
        handlers: extracted_handlers,
        ..
    } = sink;
    let (elements, functions) = metadata.ok_or_else(|| {
        HandlerTemplateError::TableIdentityMismatch(format!(
            "declaration ordinal {} was not found or is not a numeric handler table",
            identity.declaration_ordinal
        ))
    })?;
    if elements != identity.initializer_elements || functions != identity.initializer_functions {
        return Err(HandlerTemplateError::TableIdentityMismatch(format!(
            "expected {} elements/{} functions, found {}/{}",
            identity.initializer_elements, identity.initializer_functions, elements, functions
        )));
    }
    Ok(ExtractedReport {
        identity,
        raw_input_hash: report.input_hash.clone(),
        semantic_source_hash,
        source: shared_source,
        handlers: extracted_handlers,
    })
}

/// Extract recipes from the standardized output of `deobfuscate`.
pub fn handler_templates_from_report(
    report: &PipelineReport,
) -> Result<HandlerTemplateReport, HandlerTemplateError> {
    let ExtractedReport {
        identity,
        raw_input_hash: source_hash,
        handlers: extracted,
        ..
    } = extract_handlers(report)?;
    let mut coverage = TemplateCoverage::default();
    for entry in &extracted {
        match entry.template.status {
            TemplateStatus::Executable => coverage.executable += 1,
            TemplateStatus::Partial => coverage.partial += 1,
            TemplateStatus::Unsupported => coverage.unsupported += 1,
        }
    }
    let handlers = extracted.into_iter().map(|entry| entry.template).collect();
    Ok(HandlerTemplateReport {
        schema_version: SCHEMA_VERSION.into(),
        source_hash,
        table: identity,
        handlers,
        coverage,
    })
}

/// Extract the canonical per-handler micro-op graphs from the same walk as
/// [`handler_templates_from_report`].  The graphs are additive: the legacy
/// template report and its byte output are unchanged.
///
/// Prefer [`handler_ir_from_report`] in new code; this function is the single
/// implementation and the preferred name is a direct re-export alias, not a
/// second parse path.  Unsupported regions remain explicit and admission is
/// still required before any projection executes.
pub fn handler_graphs_from_report(
    report: &PipelineReport,
) -> Result<HandlerIRReport, HandlerTemplateError> {
    let ExtractedReport {
        identity,
        raw_input_hash,
        semantic_source_hash,
        source,
        handlers: extracted,
    } = extract_handlers(report)?;
    let graphs = extracted.into_iter().map(|entry| entry.graph).collect();
    Ok(HandlerIRReport {
        schema_version: GRAPH_SCHEMA_VERSION.into(),
        source_hash: semantic_source_hash,
        raw_input_hash,
        source_length: source.len(),
        source: Some(source),
        table: identity,
        graphs,
    })
}

/// Preferred entrypoint for the static per-handler `HandlerIR` extraction.
///
/// This is a direct re-export of [`handler_graphs_from_report`], so it uses the
/// same single walk, schema, identities and legacy projection.  Each graph is
/// the recovered program for one selected handler, reused across instruction
/// occurrences; decoded operands, the selected-handler occurrence and dispatch
/// PC/provenance stay in the dispatch record.
pub use self::handler_graphs_from_report as handler_ir_from_report;

/// Extract both surfaces from a single walk.  Prefer this where a caller needs
/// the legacy steps *and* the graph, so the source is parsed and scanned once.
pub fn handler_templates_and_graphs_from_report(
    report: &PipelineReport,
) -> Result<(HandlerTemplateReport, HandlerIRReport), HandlerTemplateError> {
    let ExtractedReport {
        identity,
        raw_input_hash: source_hash,
        semantic_source_hash,
        source,
        handlers: extracted,
    } = extract_handlers(report)?;
    let mut coverage = TemplateCoverage::default();
    let mut handlers = Vec::with_capacity(extracted.len());
    let mut graphs = Vec::with_capacity(extracted.len());
    for entry in extracted {
        match entry.template.status {
            TemplateStatus::Executable => coverage.executable += 1,
            TemplateStatus::Partial => coverage.partial += 1,
            TemplateStatus::Unsupported => coverage.unsupported += 1,
        }
        handlers.push(entry.template);
        graphs.push(entry.graph);
    }
    let templates = HandlerTemplateReport {
        schema_version: SCHEMA_VERSION.into(),
        source_hash: source_hash.clone(),
        table: identity.clone(),
        handlers,
        coverage,
    };
    let graph_report = HandlerIRReport {
        schema_version: GRAPH_SCHEMA_VERSION.into(),
        source_hash: semantic_source_hash,
        raw_input_hash: source_hash,
        source_length: source.len(),
        source: Some(source),
        table: identity,
        graphs,
    };
    Ok((templates, graph_report))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Receiver {
    Stack,
    Memory,
}
pub(super) struct Proof {
    pub(super) context_properties: BTreeSet<String>,
    pub(super) memory_properties: BTreeSet<String>,
    pub(super) context_binding_proven: bool,
    /// The pipeline proved a `vmCreateThread` role binding.
    pub(super) create_thread_proven: bool,
    /// The pipeline proved the `vmPendingResult` completion register.
    pub(super) completion_register_proven: bool,
    /// The outer entry call's first formal parameter, when that parameter is
    /// proven to receive the JavaScript global object (the host). References to
    /// this exact symbol are emitted as the canonical `host` binding.
    pub(super) host_parameter: Option<SymbolId>,
}
impl Proof {
    pub(super) fn from_report(report: &PipelineReport) -> Self {
        let mut context_properties = BTreeSet::new();
        let mut memory_properties = BTreeSet::new();
        for mapping in &report.property_mappings {
            match mapping.owner.as_str() {
                "vmContext" => {
                    context_properties.insert(mapping.semantic_name.clone());
                }
                "vmMemory" => {
                    memory_properties.insert(mapping.semantic_name.clone());
                }
                _ => {}
            }
        }
        let context_binding_proven = report
            .binding_mappings
            .iter()
            .any(|m| m.canonical_name == "context" && m.role.as_deref() == Some("context"))
            || context_properties.contains("stack")
            || context_properties.contains("memory");
        let role_proven = |role: &str| {
            report
                .binding_mappings
                .iter()
                .any(|m| m.canonical_name == role && m.role.as_deref() == Some(role))
        };
        Self {
            context_properties,
            memory_properties,
            context_binding_proven,
            create_thread_proven: role_proven(CREATE_THREAD),
            completion_register_proven: role_proven(COMPLETION_REGISTER),
            host_parameter: None,
        }
    }
    pub(super) fn property(&self, owner: &str, name: &str) -> bool {
        match owner {
            "vmContext" => self.context_properties.contains(name),
            "vmMemory" => self.memory_properties.contains(name),
            _ => false,
        }
    }
}

/// The outer entry call's first formal parameter when that parameter is proven
/// to receive the JavaScript global object.
///
/// The proof mirrors kernel entry extraction (`kernel::properties::scanner`):
/// the program's outermost call invokes a function expression with at least
/// four arguments, the fourth an array literal (the VM entry's initial slots),
/// and the first argument mentions an unshadowed `window`/`global`/`self`. The
/// first formal parameter then receives that global object. Symbol identity,
/// never spelling, decides what is canonicalized.
fn entry_host_parameter(program: &Program<'_>, scoping: &Scoping) -> Option<SymbolId> {
    struct Find<'s> {
        scoping: &'s Scoping,
        function_depth: usize,
        found: Option<SymbolId>,
    }
    impl<'a> Visit<'a> for Find<'_> {
        fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
            self.function_depth += 1;
            walk::walk_function(self, it, flags);
            self.function_depth -= 1;
        }

        fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
            self.function_depth += 1;
            walk::walk_arrow_function_expression(self, it);
            self.function_depth -= 1;
        }

        fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
            if self.found.is_none() && self.function_depth == 0 {
                self.found = entry_host_parameter_of_call(it, self.scoping);
            }
            walk::walk_call_expression(self, it);
        }
    }
    let mut find = Find {
        scoping,
        function_depth: 0,
        found: None,
    };
    find.visit_program(program);
    let host = find.found?;
    // `host` is a reserved canonical name. If any other binding uses it, a
    // re-emitted function could shadow the free host or collide with a distinct
    // captured name, so keep the source spelling instead of guessing.
    if scoping
        .symbol_ids()
        .any(|symbol| symbol != host && scoping.symbol_name(symbol) == HOST)
    {
        return None;
    }
    Some(host)
}

/// The first parameter symbol of one candidate entry call, when the call has
/// the proven outer-entry host shape.
fn entry_host_parameter_of_call(
    call: &CallExpression<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    let Expression::FunctionExpression(function) = call.callee.without_parentheses() else {
        return None;
    };
    if call.arguments.len() < 4 {
        return None;
    }
    let slots = call.arguments.get(3).and_then(Argument::as_expression)?;
    if !matches!(slots.without_parentheses(), Expression::ArrayExpression(_)) {
        return None;
    }
    let first = call.arguments.first().and_then(Argument::as_expression)?;
    if !mentions_unshadowed_host(first, scoping) {
        return None;
    }
    match &function.params.items.first()?.pattern {
        BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
        _ => None,
    }
}

/// Whether `expression` reads an unshadowed `window`/`global`/`self` global.
fn mentions_unshadowed_host(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    struct Probe<'s> {
        scoping: &'s Scoping,
        found: bool,
    }
    impl<'a> Visit<'a> for Probe<'_> {
        fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
            if matches!(it.name.as_str(), "window" | "global" | "self")
                && analysis::identifier_symbol(it, self.scoping).is_none()
            {
                self.found = true;
            }
        }
    }
    let mut probe = Probe {
        scoping,
        found: false,
    };
    probe.visit_expression(expression);
    probe.found
}

fn function_params(params: &FormalParameters<'_>) -> Vec<SymbolId> {
    params
        .items
        .iter()
        .filter_map(|p| match &p.pattern {
            BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
            _ => None,
        })
        .collect()
}
