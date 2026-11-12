use super::lift::{analyze_handler, span_record};
use super::*;

pub(super) fn unresolved_report(
    report: &PipelineReport,
    table: ProvenOperationTable,
    templates: &[HandlerTemplate],
    reason: String,
) -> JsOperandSeparationReport {
    let handlers = templates
        .iter()
        .cloned()
        .map(|template| LiftedHandler {
            handler_index: template.handler_index,
            table_key: template.table_key,
            source_span: OperandSpan {
                start: template.source_span.start,
                end: template.source_span.end,
            },
            entry_coordinate: OperandEntryCoordinate::AfterOpcode,
            operands: Vec::new(),
            pc_delta: 0,
            captured_dependencies: Vec::new(),
            template,
            status: OperandHandlerStatus::Unresolved,
            unresolved: vec![reason.clone()],
        })
        .collect::<Vec<_>>();
    JsOperandSeparationReport {
        schema_version: SCHEMA_VERSION.into(),
        source_hash: report.input_hash.clone(),
        table,
        input_identity: None,
        entry_coordinate: None,
        coverage: OperandCoverage {
            lifted: 0,
            partial: 0,
            unresolved: handlers.len() as u32,
            not_applicable: 0,
        },
        handlers,
        unresolved: vec![reason],
    }
}

#[derive(Clone)]
pub(super) struct DecoderProof {
    pub(super) bytecode: SymbolId,
    pub(super) context_property: String,
    pub(super) entry_coordinate: OperandEntryCoordinate,
    pub(super) input_identity: OperandInputIdentity,
    pub(super) unresolved: Vec<String>,
}

pub(super) fn resolve_proof<'a>(
    report: &PipelineReport,
    program: &'a Program<'a>,
    scoping: &Scoping,
    table: &ProvenOperationTable,
    handoff: Option<&JsOperandBindingHandoff>,
) -> Result<DecoderProof, JsOperandSeparationError> {
    if let Some(handoff) = handoff {
        let mut resolver = DeclarationResolver {
            target: handoff.bytecode_declaration_span.clone(),
            result: None,
        };
        resolver.visit_program(program);
        let Some((symbol, name)) = resolver.result else {
            return Err(JsOperandSeparationError::InvalidHandoff(
                "bytecode declaration span does not identify a variable declarator".into(),
            ));
        };
        return Ok(DecoderProof {
            bytecode: symbol,
            context_property: handoff.pc_property.clone(),
            entry_coordinate: handoff.entry_coordinate,
            input_identity: OperandInputIdentity {
                binding_name: name.to_string(),
                declaration_span: span_record(scoping.symbol_span(symbol)),
            },
            unresolved: Vec::new(),
        });
    }
    let context_proven = report.binding_mappings.iter().any(|mapping| {
        mapping.canonical_name == "vmContext"
            && matches!(mapping.role.as_deref(), Some("context") | Some("vmContext"))
    });
    if !context_proven {
        return Err(JsOperandSeparationError::InvalidHandoff(
            "JS dispatch proof requires a pipeline-proven context constructor".into(),
        ));
    }
    let mut context_properties = ContextPropertyFinder {
        properties: BTreeSet::new(),
    };
    context_properties.visit_program(program);
    let mut finder = DispatchProofFinder {
        scoping,
        context_properties: context_properties.properties.clone(),
        candidates: Vec::new(),
    };
    finder.visit_program(program);
    let Some((bytecode, pc_property, name, _function_span)) = finder.candidates.into_iter().next()
    else {
        return Err(JsOperandSeparationError::InvalidHandoff(
            "no JS dispatch method proved the bytecode binding and PC field".into(),
        ));
    };
    let _ = (scoping, table);
    Ok(DecoderProof {
        bytecode,
        context_property: pc_property,
        entry_coordinate: OperandEntryCoordinate::AfterOpcode,
        input_identity: OperandInputIdentity {
            binding_name: name,
            declaration_span: span_record(scoping.symbol_span(bytecode)),
        },
        unresolved: Vec::new(),
    })
}

struct DeclarationResolver {
    target: OperandSpan,
    result: Option<(SymbolId, String)>,
}
impl<'a> Visit<'a> for DeclarationResolver {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(id) = &declarator.id {
            let matches_declaration = declarator.span.start == self.target.start
                && declarator.span.end == self.target.end;
            let matches_binding =
                id.span.start == self.target.start && id.span.end == self.target.end;
            if matches_declaration || matches_binding {
                if let Some(symbol) = id.symbol_id.get() {
                    self.result = Some((symbol, id.name.to_string()));
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}

struct DispatchProofFinder<'a> {
    scoping: &'a Scoping,
    context_properties: BTreeSet<String>,
    candidates: Vec<(SymbolId, String, String, OperandSpan)>,
}
impl<'a> Visit<'a> for DispatchProofFinder<'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        let Some(body) = function.body.as_deref() else {
            return;
        };
        let mut visitor = DispatchBodyVisitor {
            scoping: self.scoping,
            context_properties: &self.context_properties,
            candidate: None,
            name: None,
        };
        for statement in &body.statements {
            visitor.visit_statement(statement);
        }
        if let Some((symbol, property)) = visitor.candidate {
            let name = visitor.name.unwrap_or_else(|| "<bytecode>".into());
            self.candidates
                .push((symbol, property, name, span_record(function.span)));
        }
        // Nested functions are separate dispatch candidates and are visited by
        // the recursive AST walk; don't merge their body into this one.
        walk::walk_function(self, function, flags);
    }
}
struct DispatchBodyVisitor<'a> {
    scoping: &'a Scoping,
    context_properties: &'a BTreeSet<String>,
    candidate: Option<(SymbolId, String)>,
    name: Option<String>,
}
impl<'a> Visit<'a> for DispatchBodyVisitor<'a> {
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        let Expression::Identifier(object) = &member.object else {
            walk::walk_computed_member_expression(self, member);
            return;
        };
        let Some(symbol) = analysis::identifier_symbol(object, self.scoping) else {
            walk::walk_computed_member_expression(self, member);
            return;
        };
        let mut probe = PcIndexProbe { property: None };
        probe.visit_expression(&member.expression);
        if let Some(property) = probe
            .property
            .filter(|property| self.context_properties.contains(property))
        {
            self.candidate = Some((symbol, property));
            self.name = Some(object.name.to_string());
        }
        walk::walk_computed_member_expression(self, member);
    }
}

struct ContextPropertyFinder {
    properties: BTreeSet<String>,
}
impl<'a> Visit<'a> for ContextPropertyFinder {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(body) = function.body.as_deref() {
            let mut assignments = ContextAssignmentFinder {
                properties: &mut self.properties,
            };
            for statement in &body.statements {
                assignments.visit_statement(statement);
            }
        }
        walk::walk_function(self, function, flags);
    }
}
struct ContextAssignmentFinder<'a> {
    properties: &'a mut BTreeSet<String>,
}
impl<'a> Visit<'a> for ContextAssignmentFinder<'a> {
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if let AssignmentTarget::StaticMemberExpression(member) = &assignment.left {
            if matches!(member.object, Expression::ThisExpression(_))
                && assignment.operator == AssignmentOperator::Assign
            {
                self.properties.insert(member.property.name.to_string());
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
}
struct PcIndexProbe {
    property: Option<String>,
}
impl<'a> Visit<'a> for PcIndexProbe {
    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if update.operator != UpdateOperator::Increment {
            return;
        }
        if let SimpleAssignmentTarget::StaticMemberExpression(member) = &update.argument {
            if matches!(member.object, Expression::ThisExpression(_)) {
                self.property = Some(member.property.name.to_string());
            }
        }
        walk::walk_update_expression(self, update);
    }
}

pub(super) struct OperandTableSink<'a> {
    pub(super) source: &'a str,
    pub(super) scoping: &'a Scoping,
    pub(super) nodes: &'a AstNodes<'a>,
    pub(super) proof: DecoderProof,
    pub(super) templates: &'a [HandlerTemplate],
    pub(super) handlers: Vec<LiftedHandler>,
}

impl TableSink for OperandTableSink<'_> {
    fn table(&mut self, table: TableLiteral<'_>) {
        for entry in table.entries {
            let Some(function) = entry.handler else {
                continue;
            };
            let template = self
                .templates
                .iter()
                .find(|handler| handler.handler_index == entry.key)
                .cloned();
            let Some(template) = template else { continue };
            self.handlers.push(analyze_handler(
                self.source,
                self.scoping,
                self.nodes,
                &self.proof,
                entry.key,
                function,
                template,
            ));
        }
    }
}
