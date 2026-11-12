//! Linked-program direct property writes under standard built-in assumptions.
//!
//! `simplify_program_property_writes` keeps the strict descriptor/receiver
//! proof but also accepts a direct fresh array/object or a projection from a
//! zero-argument factory returning fresh allocations. The call must be in the
//! same block after the declaration, with no earlier observation, closure
//! capture, reassignment, or out-of-block reference.
//!
//! `simplify_readable_program_property_writes` is output-only. It assumes an
//! ordinary extensible receiver with no own accessor/non-writable target and a
//! primitive dynamic key. It deliberately skips program-level calls because
//! rewriting their completion value would be visible. This stage is not used
//! by the strict thread/corpus cleanup path.

use super::*;

/// Lower generated data-descriptor statements for readable output. This stage
/// assumes the receiver is an ordinary extensible array/object without an own
/// accessor or non-writable target property, and a dynamic key is primitive.
/// It deliberately stays out of the strict cleanup and corpus workflows.
pub fn simplify_readable_program_property_writes(
    source: &str,
) -> Result<PropertyWriteCleanupOutcome, PropertyWriteCleanupError> {
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, source).map_err(PropertyWriteCleanupError::Parse)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut contexts = ContextCollector::default();
    contexts.visit_program(&program);
    let mut scan = ReadableCallCollector {
        source,
        scoping: &scoping,
        contexts: &contexts,
        edits: Vec::new(),
        considered: 0,
        considered_spans: Vec::new(),
        dynamic_scope: false,
    };
    scan.visit_program(&program);
    let rewritten = scan
        .edits
        .iter()
        .map(|edit| edit.start)
        .collect::<BTreeSet<_>>();
    let skipped = scan
        .considered_spans
        .iter()
        .filter(|span| scan.dynamic_scope || !rewritten.contains(&span.start))
        .map(|span| PropertyWriteSkip {
            span: span_pair(*span),
            reason: if scan.dynamic_scope {
                PropertyWriteSkipReason::DynamicScope
            } else {
                PropertyWriteSkipReason::ProgramRewriteUnproven
            },
            detail: if scan.dynamic_scope {
                "dynamic scope prevents readable property writes"
            } else {
                "call does not match the readable generated data-write shape"
            }
            .into(),
        })
        .collect::<Vec<_>>();
    if scan.dynamic_scope || scan.edits.is_empty() {
        return Ok(PropertyWriteCleanupOutcome {
            code: source.into(),
            changed: 0,
            preseeded_allocations: 0,
            preseeded_keys: 0,
            considered: scan.considered,
            skipped,
            status: if scan.dynamic_scope {
                StageStatus::Unsupported
            } else {
                StageStatus::Unknown
            },
            notes: vec!["readable generated write assumptions did not apply".into()],
            input_bytes: source.len(),
            output_bytes: source.len(),
        });
    }
    let changed = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() || applied.applied != changed {
        return Err(PropertyWriteCleanupError::InvalidOutput(
            "readable program property-write edits overlap".into(),
        ));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(PropertyWriteCleanupError::InvalidOutput)?;
    Ok(PropertyWriteCleanupOutcome {
        output_bytes: applied.output.len(),
        code: applied.output,
        changed,
        preseeded_allocations: 0,
        preseeded_keys: 0,
        considered: scan.considered,
        skipped,
        status: StageStatus::Found,
        notes: vec![
            "assumes ordinary extensible receiver, standard built-ins, and primitive dynamic keys"
                .into(),
        ],
        input_bytes: source.len(),
    })
}

struct ReadableCallCollector<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    contexts: &'s ContextCollector,
    edits: Vec<analysis::Edit>,
    considered: usize,
    considered_spans: Vec<Span>,
    dynamic_scope: bool,
}

impl<'a> Visit<'a> for ReadableCallCollector<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let callee = unwrap_parens(&call.callee);
        let object = match callee {
            Expression::StaticMemberExpression(member)
                if !member.optional && member.property.name == "defineProperty" =>
            {
                Some(unwrap_parens(&member.object))
            }
            Expression::ComputedMemberExpression(member)
                if !member.optional
                    && matches!(unwrap_parens(&member.expression), Expression::StringLiteral(key) if key.value == "defineProperty") =>
            {
                Some(unwrap_parens(&member.object))
            }
            _ => None,
        };
        if let Some(Expression::Identifier(object)) = object {
            if object.name == "Object" {
                self.considered += 1;
                self.considered_spans.push(call.span);
                if analysis::identifier_symbol(object, self.scoping).is_none()
                    && !call.optional
                    && call.arguments.len() == 3
                    && call.arguments.iter().all(|argument| !argument.is_spread())
                    && self
                        .contexts
                        .call_contexts
                        .get(&call.span.start)
                        .is_some_and(|context| {
                            Some(context.block_id) != self.contexts.program_block_id
                        })
                {
                    let receiver = unwrap_parens(call.arguments[0].to_expression());
                    let key = unwrap_parens(call.arguments[1].to_expression());
                    let descriptor = unwrap_parens(call.arguments[2].to_expression());
                    if let Expression::Identifier(receiver) = receiver {
                        if analysis::identifier_symbol(receiver, self.scoping).is_some()
                            && (static_property_key(key).is_some()
                                || matches!(key, Expression::Identifier(_)))
                            && !static_property_key(key).is_some_and(|key| {
                                key.canonical == "__proto__" || key.canonical == "length"
                            })
                        {
                            if let Ok(descriptor) = exact_data_descriptor(descriptor) {
                                let receiver_text = &self.source
                                    [receiver.span.start as usize..receiver.span.end as usize];
                                let key_span = key.span();
                                let key_text =
                                    &self.source[key_span.start as usize..key_span.end as usize];
                                let value_span = descriptor.value_span;
                                let value_text = &self.source
                                    [value_span.start as usize..value_span.end as usize];
                                let value = if descriptor.value_needs_parentheses {
                                    format!("({value_text})")
                                } else {
                                    value_text.into()
                                };
                                self.edits.push(analysis::Edit::replace(
                                    call.span,
                                    format!("{receiver_text}[{key_text}] = {value}"),
                                ));
                            }
                        }
                    }
                }
            }
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}

/// Simplify exact generated data writes under the trace-program assumption that
/// the global `Object.defineProperty` and ordinary built-in prototypes retain
/// their standard behavior. Unlike `simplify_property_writes`, this does not
/// pre-seed missing keys: a fresh ordinary allocation is enough under that
/// assumption. The receiver must still be locally proven fresh and unobserved.
pub fn simplify_program_property_writes(
    source: &str,
) -> Result<PropertyWriteCleanupOutcome, PropertyWriteCleanupError> {
    simplify_thread_property_writes(source, &BTreeSet::new())
}

pub(crate) fn simplify_thread_property_writes(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<PropertyWriteCleanupOutcome, PropertyWriteCleanupError> {
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, source).map_err(PropertyWriteCleanupError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut contexts = ContextCollector::default();
    contexts.visit_program(&program);
    let mut scan = ScanCollector {
        scoping: &scoping,
        raw_calls: Vec::new(),
        define_property_members: Vec::new(),
        write_spans: Vec::new(),
        saw_with: false,
        saw_eval_reference: false,
        saw_this: false,
        saw_constructor_access: false,
        saw_unknown_computed_call: false,
    };
    scan.visit_program(&program);
    let calls = build_calls(&scan.raw_calls, &contexts, &scoping);
    let mut returned = ReturnedFreshCollector {
        scoping: &scoping,
        factories: BTreeMap::new(),
        calls: BTreeMap::new(),
        projections: BTreeMap::new(),
    };
    returned.visit_program(&program);
    let mut assignments = ConstructionWrites {
        scoping: &scoping,
        writes: BTreeMap::new(),
    };
    assignments.visit_program(&program);
    let returned_plans = returned.proven_receivers(&scoping, &nodes, &contexts);
    let considered = calls.len();
    let mut skipped = Vec::new();
    let mut rewritten = BTreeSet::new();
    let mut edits = Vec::new();
    let mut changed = 0;
    if scan.saw_with || scan.saw_eval_reference {
        return Ok(dynamic_scope_outcome(source, &calls, considered));
    }
    let mut groups: BTreeMap<SymbolId, Vec<usize>> = BTreeMap::new();
    for (index, call) in calls.iter().enumerate() {
        if let Some(skip) = &call.skip {
            skipped.push(skip.clone());
        } else if let Some(symbol) = call.receiver_symbol {
            groups.entry(symbol).or_default().push(index);
        }
    }
    let proof = ThreadReceiverProof {
        scoping: &scoping,
        nodes: &nodes,
        contexts: &contexts,
        calls: &calls,
        returned_plans: &returned_plans,
        assignments: &assignments,
    };
    for (symbol, indices) in groups {
        if protected.contains(scoping.symbol_name(symbol)) {
            continue;
        }
        let Some(same_block) = proof.proven_writes(symbol, indices, &mut skipped) else {
            continue;
        };
        for index in same_block {
            let call = &calls[index];
            edits.push(analysis::Edit::replace(call.span, direct_write(source, call)));
            rewritten.insert(index);
            changed += 1;
        }
    }
    skip_unproven(&calls, &rewritten, &mut skipped);
    skipped.sort_by_key(|skip| skip.span.0);
    if edits.is_empty() {
        return Ok(PropertyWriteCleanupOutcome {
            code: source.into(),
            changed: 0,
            preseeded_allocations: 0,
            preseeded_keys: 0,
            considered,
            skipped,
            status: StageStatus::Unknown,
            notes: vec!["no generated receiver met the direct-write proof".into()],
            input_bytes: source.len(),
            output_bytes: source.len(),
        });
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() || applied.applied != changed {
        return Err(PropertyWriteCleanupError::InvalidOutput(
            "program property-write edits overlap".into(),
        ));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(PropertyWriteCleanupError::InvalidOutput)?;
    Ok(PropertyWriteCleanupOutcome {
        output_bytes: applied.output.len(),
        code: applied.output,
        changed,
        preseeded_allocations: 0,
        preseeded_keys: 0,
        considered,
        skipped,
        status: StageStatus::Found,
        notes: vec!["assumes standard Object.defineProperty and prototypes".into()],
        input_bytes: source.len(),
    })
}

/// The unchanged outcome when `with` or `eval` declines every call.
fn dynamic_scope_outcome(
    source: &str,
    calls: &[CandidateCall],
    considered: usize,
) -> PropertyWriteCleanupOutcome {
    PropertyWriteCleanupOutcome {
        code: source.into(),
        changed: 0,
        preseeded_allocations: 0,
        preseeded_keys: 0,
        considered,
        skipped: calls
            .iter()
            .map(|call| PropertyWriteSkip {
                span: span_pair(call.span),
                reason: PropertyWriteSkipReason::DynamicScope,
                detail: "dynamic scope prevents generated receiver proof".into(),
            })
            .collect(),
        status: StageStatus::Unsupported,
        notes: vec!["dynamic scope prevents generated receiver proof".into()],
        input_bytes: source.len(),
        output_bytes: source.len(),
    }
}

/// What the per-receiver direct-write proof reads.
struct ThreadReceiverProof<'r, 'a> {
    scoping: &'r Scoping,
    nodes: &'r AstNodes<'a>,
    contexts: &'r ContextCollector,
    calls: &'r [CandidateCall],
    returned_plans: &'r BTreeMap<SymbolId, ReturnedFreshPlan>,
    assignments: &'r ConstructionWrites<'r>,
}

impl ThreadReceiverProof<'_, '_> {
    /// The receiver's calls that may become direct writes: eligible keys in
    /// its fresh allocation's block, with no earlier observation of it. A
    /// receiver with no fresh allocation records a skip for each call.
    fn proven_writes(
        &self,
        symbol: SymbolId,
        indices: Vec<usize>,
        skipped: &mut Vec<PropertyWriteSkip>,
    ) -> Option<Vec<usize>> {
        let ThreadReceiverProof {
            scoping,
            nodes,
            contexts,
            calls,
            returned_plans,
            assignments,
        } = *self;
        let direct_plan = contexts.declarations.get(&symbol);
        let returned_plan = returned_plans.get(&symbol);
        let Some((block_id, declaration_index, is_array, eligible)) = direct_plan
            .map(|plan| {
                let (is_array, eligible) = match &plan.literal {
                    AllocationLiteral::Array { preseedable, .. } => (true, *preseedable),
                    AllocationLiteral::Object { preseedable, .. } => (false, *preseedable),
                };
                (plan.block_id, plan.statement_index, is_array, eligible)
            })
            .or_else(|| {
                returned_plan.map(|plan| (plan.block_id, plan.statement_index, plan.is_array, true))
            })
        else {
            for index in indices {
                skipped.push(PropertyWriteSkip {
                    span: span_pair(calls[index].span),
                    reason: PropertyWriteSkipReason::ReceiverNotFreshAllocation,
                    detail: "receiver is not a direct fresh ordinary array/object allocation"
                        .into(),
                });
            }
            return None;
        };
        if !eligible
            || contexts.parameter_symbols.contains(&symbol)
            || contexts.duplicate_symbols.contains(&symbol)
        {
            return None;
        }
        let mut same_block = Vec::new();
        for index in indices {
            let call = &calls[index];
            if call.context.is_some_and(|context| {
                context.block_id == block_id && context.statement_index > declaration_index
            }) && call.key.canonical != "__proto__"
                && !(is_array && call.key.canonical == "length")
            {
                same_block.push(index);
            }
        }
        if same_block.is_empty() {
            return None;
        }
        let last_write = same_block
            .iter()
            .filter_map(|index| calls[*index].context)
            .map(|context| context.statement_index)
            .max()
            .unwrap();
        let mut receivers: BTreeSet<u32> = same_block
            .iter()
            .filter_map(|index| calls[*index].receiver_ident_span)
            .map(|span| span.start)
            .collect();
        // Earlier rounds may already have lowered some construction writes.
        // Only a direct, static-key assignment in this allocation block is
        // harmless here. RHS references and all escapes still fail the proof.
        for statement in
            &contexts.blocks[block_id].statement_spans[declaration_index + 1..=last_write]
        {
            if let Some(&(receiver, span)) = assignments.writes.get(&statement.start) {
                if receiver == symbol {
                    receivers.insert(span.start);
                }
            }
        }
        if receiver_references_are_safe(
            scoping,
            nodes,
            symbol,
            last_write,
            &contexts.blocks[block_id].statement_spans,
            &receivers,
        )
        .is_err()
        {
            return None;
        }
        Some(same_block)
    }
}

/// `receiver[key] = value` for one proven call.
fn direct_write(source: &str, call: &CandidateCall) -> String {
    let receiver =
        &source[call.receiver_expr_span.start as usize..call.receiver_expr_span.end as usize];
    let key = &source[call.key.span.start as usize..call.key.span.end as usize];
    let value = &source
        [call.descriptor.value_span.start as usize..call.descriptor.value_span.end as usize];
    let value = if call.descriptor.value_needs_parentheses {
        format!("({value})")
    } else {
        value.into()
    };
    format!("{receiver}[{key}] = {value}")
}

/// Skip every call that was neither rewritten nor already skipped.
fn skip_unproven(
    calls: &[CandidateCall],
    rewritten: &BTreeSet<usize>,
    skipped: &mut Vec<PropertyWriteSkip>,
) {
    let already_skipped = skipped
        .iter()
        .map(|skip| skip.span.0)
        .collect::<BTreeSet<_>>();
    for (index, call) in calls.iter().enumerate() {
        if !rewritten.contains(&index)
            && call.skip.is_none()
            && !already_skipped.contains(&call.span.start)
        {
            skipped.push(PropertyWriteSkip {
                span: span_pair(call.span),
                reason: PropertyWriteSkipReason::ProgramRewriteUnproven,
                detail: "generated direct-write proof did not establish a fresh, unobserved ordinary receiver and eligible key".into(),
            });
        }
    }
}

struct ConstructionWrites<'s> {
    scoping: &'s Scoping,
    writes: BTreeMap<u32, (SymbolId, Span)>,
}

impl<'a> Visit<'a> for ConstructionWrites<'_> {
    fn visit_expression_statement(&mut self, statement: &ExpressionStatement<'a>) {
        if let Expression::AssignmentExpression(assignment) = unwrap_parens(&statement.expression) {
            if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign {
                let member = match &assignment.left {
                    AssignmentTarget::StaticMemberExpression(member)
                        if !matches!(member.property.name.as_str(), "__proto__" | "length") =>
                    {
                        Some(&member.object)
                    }
                    AssignmentTarget::ComputedMemberExpression(member)
                        if static_property_key(&member.expression).is_some_and(|key| {
                            key.canonical != "__proto__" && key.canonical != "length"
                        }) =>
                    {
                        Some(&member.object)
                    }
                    _ => None,
                };
                if let Some(Expression::Identifier(id)) = member.map(unwrap_parens) {
                    if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                        self.writes.insert(statement.span.start, (symbol, id.span));
                    }
                }
            }
        }
        walk::walk_expression_statement(self, statement);
    }
}

#[derive(Debug, Clone, Copy)]
struct ReturnedFreshPlan {
    block_id: usize,
    statement_index: usize,
    is_array: bool,
}

struct ReturnedFreshCollector<'s> {
    scoping: &'s Scoping,
    // A zero-argument function whose entire body creates ordinary local
    // allocations and returns them in a new array.
    factories: BTreeMap<SymbolId, Vec<bool>>,
    calls: BTreeMap<SymbolId, SymbolId>,
    projections: BTreeMap<SymbolId, (SymbolId, usize, Span, Span)>,
}

impl ReturnedFreshCollector<'_> {
    fn proven_receivers(
        &self,
        scoping: &Scoping,
        nodes: &AstNodes<'_>,
        contexts: &ContextCollector,
    ) -> BTreeMap<SymbolId, ReturnedFreshPlan> {
        let mut plans = BTreeMap::new();
        for (&receiver, &(pair, index, _, declaration_span)) in &self.projections {
            let Some(&factory) = self.calls.get(&pair) else {
                continue;
            };
            let Some(kinds) = self.factories.get(&factory) else {
                continue;
            };
            let Some(&is_array) = kinds.get(index) else {
                continue;
            };
            if scoping.symbol_scope_id(receiver) != scoping.symbol_scope_id(pair)
                || scoping
                    .get_resolved_references(factory)
                    .any(|reference| reference.is_write())
            {
                continue;
            }
            // The returned pair must only be indexed by recognized local
            // projections. No call, escape, assignment, or mutation may occur.
            let safe_pair_reads = self
                .projections
                .values()
                .filter(|(candidate, _, _, _)| *candidate == pair)
                .map(|(_, _, span, _)| span.start)
                .collect::<BTreeSet<_>>();
            let projected_indices = self
                .projections
                .values()
                .filter(|(candidate, _, _, _)| *candidate == pair)
                .map(|(_, index, _, _)| *index)
                .collect::<Vec<_>>();
            if projected_indices
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != projected_indices.len()
            {
                continue;
            }
            if scoping.get_resolved_references(pair).any(|reference| {
                let span = nodes.get_node(reference.node_id()).kind().span();
                reference.is_write() || !safe_pair_reads.contains(&span.start)
            }) {
                continue;
            }
            let Some((block_id, statement_index)) =
                contexts
                    .blocks
                    .iter()
                    .enumerate()
                    .find_map(|(block_id, block)| {
                        block
                            .statement_spans
                            .iter()
                            .enumerate()
                            .find(|(_, span)| **span == declaration_span)
                            .map(|(index, _)| (block_id, index))
                    })
            else {
                continue;
            };
            plans.insert(
                receiver,
                ReturnedFreshPlan {
                    block_id,
                    statement_index,
                    is_array,
                },
            );
        }
        plans
    }
}

impl<'a> Visit<'a> for ReturnedFreshCollector<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let (Some(name), Some(body)) = (&function.id, &function.body) {
            if function.params.items.is_empty() && !function.r#async && !function.generator {
                let statements = &body.statements;
                if let Some(Statement::ReturnStatement(ret)) = statements.last() {
                    if let Some(Expression::ArrayExpression(array)) = &ret.argument {
                        if statements.len() == array.elements.len() + 1
                            && !array.elements.is_empty()
                        {
                            let mut allocations = BTreeMap::new();
                            for statement in &statements[..statements.len() - 1] {
                                let Statement::VariableDeclaration(declaration) = statement else {
                                    break;
                                };
                                let [binding] = declaration.declarations.as_slice() else {
                                    break;
                                };
                                let (BindingPattern::BindingIdentifier(id), Some(init)) =
                                    (&binding.id, &binding.init)
                                else {
                                    break;
                                };
                                if id.symbol_id.get().is_none() {
                                    break;
                                }
                                let kind = match init {
                                    Expression::ArrayExpression(value)
                                        if value.elements.is_empty() =>
                                    {
                                        true
                                    }
                                    Expression::ObjectExpression(value)
                                        if value.properties.is_empty() =>
                                    {
                                        false
                                    }
                                    _ => break,
                                };
                                allocations.insert(id.name.to_string(), kind);
                            }
                            if allocations.len() == array.elements.len() {
                                let names: Option<Vec<String>> = array
                                    .elements
                                    .iter()
                                    .map(|element| {
                                        let Expression::Identifier(id) = element.as_expression()?
                                        else {
                                            return None;
                                        };
                                        Some(id.name.to_string())
                                    })
                                    .collect();
                                if let (Some(symbol), Some(names)) = (name.symbol_id.get(), names) {
                                    if names.iter().collect::<BTreeSet<_>>().len() == names.len() {
                                        let kinds: Option<Vec<bool>> = names
                                            .iter()
                                            .map(|name| allocations.get(name).copied())
                                            .collect();
                                        if let Some(kinds) = kinds {
                                            self.factories.insert(symbol, kinds);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'a>) {
        if let [binding] = declaration.declarations.as_slice() {
            if let (BindingPattern::BindingIdentifier(name), Some(init)) =
                (&binding.id, &binding.init)
            {
                if let Some(symbol) = name.symbol_id.get() {
                    match init {
                        Expression::CallExpression(call) if call.arguments.is_empty() => {
                            if let Expression::Identifier(callee) = &call.callee {
                                if let Some(factory) =
                                    analysis::identifier_symbol(callee, self.scoping)
                                {
                                    self.calls.insert(symbol, factory);
                                }
                            }
                        }
                        Expression::ComputedMemberExpression(member) if !member.optional => {
                            if let (
                                Expression::Identifier(object),
                                Expression::NumericLiteral(index),
                            ) = (&member.object, &member.expression)
                            {
                                if index.value.fract() == 0.0 && index.value >= 0.0 {
                                    if let Some(pair) =
                                        analysis::identifier_symbol(object, self.scoping)
                                    {
                                        self.projections.insert(
                                            symbol,
                                            (
                                                pair,
                                                index.value as usize,
                                                object.span,
                                                declaration.span,
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        walk::walk_variable_declaration(self, declaration);
    }
}
