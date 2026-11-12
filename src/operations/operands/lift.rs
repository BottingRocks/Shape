use super::resolve::DecoderProof;
use super::*;

pub(super) fn analyze_handler(
    _source: &str,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    proof: &DecoderProof,
    key: u32,
    function: Handler<'_>,
    mut template: HandlerTemplate,
) -> LiftedHandler {
    let mut unresolved = Vec::new();
    unresolved.extend(proof.unresolved.iter().cloned());
    let Some((context, context_name)) =
        function
            .params()
            .items
            .first()
            .and_then(|parameter| match &parameter.pattern {
                BindingPattern::BindingIdentifier(identifier) => identifier
                    .symbol_id
                    .get()
                    .map(|symbol| (symbol, identifier.name.to_string())),
                _ => None,
            })
    else {
        unresolved.push("handler context parameter has no Oxc SymbolId".into());
        return unresolved_handler(key, function.span(), template, unresolved);
    };
    let mut collector = OperandCollector::new(_source, scoping, proof, context, context_name);
    match function.body() {
        Some(body) => {
            for statement in &body.statements {
                collector.visit_statement(statement);
            }
        }
        None => unresolved.push("handler has no body".into()),
    }
    unresolved.extend(collector.unresolved.clone());
    let parameters = function
        .params()
        .items
        .iter()
        .filter_map(|parameter| match &parameter.pattern {
            BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let dependencies = captured_dependencies(
        scoping,
        nodes,
        &function,
        proof,
        context,
        &collector.locals,
        &parameters,
    );
    if !collector.pc_writes.is_empty() {
        unresolved.extend(collector.pc_writes.iter().map(|write| write.reason.clone()));
    }
    if collector.bytecode_mutated {
        unresolved.push("bytecode binding is mutated or escapes the handler".into());
    }
    // A pushed call frame returns to the PC after this handler's operands, so
    // it must be captured after the final operand advance.
    if collector
        .frame_push_deltas
        .iter()
        .any(|delta| *delta != collector.pc_delta)
    {
        unresolved.push("call frame captures the PC before the operand advance completes".into());
    }
    // A handler without an inline read is outside this opt-in extraction
    // concern. Keep the existing template and status byte-for-byte intact.
    if collector.descriptors.is_empty() {
        let status = if unresolved.is_empty()
            && collector.pc_writes.is_empty()
            && !collector.bytecode_mutated
            && template.status == TemplateStatus::Executable
        {
            OperandHandlerStatus::NotApplicable
        } else {
            OperandHandlerStatus::Partial
        };
        return LiftedHandler {
            handler_index: key,
            table_key: key,
            source_span: span_record(function.span()),
            entry_coordinate: proof.entry_coordinate,
            operands: Vec::new(),
            pc_delta: collector.pc_delta,
            captured_dependencies: dependencies,
            template,
            status,
            unresolved,
        };
    }
    let mut lifted = lift_template(&mut template, &collector);
    let recipe_has_unknown = lifted
        .steps
        .iter()
        .any(|step| recipe_operation_has_unknown(&step.operation));
    if recipe_has_unknown {
        unresolved.push("lifted recipe retains an unknown semantic value or operation".into());
    }
    let status = if unresolved.is_empty() && lifted.unsupported.is_empty() && !recipe_has_unknown {
        lifted.status = TemplateStatus::Executable;
        OperandHandlerStatus::Lifted
    } else if lifted.steps.iter().any(|step| {
        !matches!(step.operation, RecipeOperation::Unknown { .. })
            && !recipe_operation_has_unknown(&step.operation)
    }) {
        lifted.status = TemplateStatus::Partial;
        OperandHandlerStatus::Partial
    } else {
        lifted.status = TemplateStatus::Unsupported;
        OperandHandlerStatus::Unresolved
    };
    let pc_delta = collector.pc_delta;
    let operands = collector.descriptors;
    LiftedHandler {
        handler_index: key,
        table_key: key,
        source_span: span_record(function.span()),
        entry_coordinate: proof.entry_coordinate,
        operands,
        pc_delta,
        captured_dependencies: dependencies,
        template: lifted,
        status,
        unresolved,
    }
}

fn unresolved_handler(
    key: u32,
    span: Span,
    template: HandlerTemplate,
    unresolved: Vec<String>,
) -> LiftedHandler {
    LiftedHandler {
        handler_index: key,
        table_key: key,
        source_span: span_record(span),
        entry_coordinate: OperandEntryCoordinate::AfterOpcode,
        operands: Vec::new(),
        pc_delta: 0,
        captured_dependencies: Vec::new(),
        template,
        status: OperandHandlerStatus::Unresolved,
        unresolved,
    }
}

struct PcWrite {
    reason: String,
}
struct OperandCollector<'a> {
    scoping: &'a Scoping,
    proof: &'a DecoderProof,
    context: SymbolId,
    context_name: String,
    bytecode_aliases: BTreeSet<SymbolId>,
    /// Each alias records the PC offset at its declaration. It is a snapshot,
    /// not a live reference to the context field.
    pc_aliases: BTreeMap<SymbolId, u32>,
    locals: BTreeSet<SymbolId>,
    descriptors: Vec<OperandDescriptor>,
    direct_operand_declarations: Vec<((u32, u32), u32)>,
    pc_snapshot_declarations: BTreeSet<(u32, u32)>,
    decoder_effect_spans: Vec<(u32, u32)>,
    skip_expression: BTreeSet<(u32, u32)>,
    unresolved: Vec<String>,
    pc_writes: Vec<PcWrite>,
    pc_delta: u32,
    bytecode_mutated: bool,
    observable_effect_seen: bool,
    /// An absolute PC write (a VM jump) was seen.
    jump_seen: bool,
    /// The PC advance at each `callFrameStack.push(...)` frame capture.
    frame_push_deltas: Vec<u32>,
    terminal_seen: bool,
    terminal_skip_reported: bool,
}
impl<'a> OperandCollector<'a> {
    fn new(
        _source: &'a str,
        scoping: &'a Scoping,
        proof: &'a DecoderProof,
        context: SymbolId,
        context_name: String,
    ) -> Self {
        Self {
            scoping,
            proof,
            context,
            context_name,
            bytecode_aliases: [proof.bytecode].into_iter().collect(),
            pc_aliases: BTreeMap::new(),
            locals: BTreeSet::new(),
            descriptors: Vec::new(),
            direct_operand_declarations: Vec::new(),
            pc_snapshot_declarations: BTreeSet::new(),
            decoder_effect_spans: Vec::new(),
            skip_expression: BTreeSet::new(),
            unresolved: Vec::new(),
            pc_writes: Vec::new(),
            pc_delta: 0,
            bytecode_mutated: false,
            observable_effect_seen: false,
            jump_seen: false,
            frame_push_deltas: Vec::new(),
            terminal_seen: false,
            terminal_skip_reported: false,
        }
    }
    fn symbol(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        analysis::identifier_symbol(identifier, self.scoping)
    }
    fn direct_alias(&self, expression: &Expression<'_>) -> Option<(bool, SymbolId)> {
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let symbol = self.symbol(identifier)?;
        Some((self.bytecode_aliases.contains(&symbol), symbol))
    }
    fn add_descriptor(
        &mut self,
        expression: &Expression<'_>,
        declaration: Option<&VariableDeclarator<'_>>,
    ) -> bool {
        if let Some(recipe) = decode_recipe(expression, self) {
            if self.jump_seen {
                self.unresolved
                    .push("operand is decoded after a control-flow jump".into());
            }
            self.skip_expression
                .insert((expression.span().start, expression.span().end));
            let position = self.descriptors.len() as u32 + 1;
            let source_spans = recipe.spans.iter().map(|span| span_record(*span)).collect();
            self.descriptors.push(OperandDescriptor {
                position,
                byte_offset: recipe.offset,
                width: recipe.width,
                endian: recipe.endian,
                signed: recipe.signed,
                mask: recipe.mask,
                pc_delta: self.pc_delta,
                source_spans,
                input_identity: self.proof.input_identity.clone(),
            });
            if let Some(declaration) = declaration {
                if let BindingPattern::BindingIdentifier(identifier) = &declaration.id {
                    if let Some(symbol) = identifier.symbol_id.get() {
                        self.locals.insert(symbol);
                    }
                }
                self.direct_operand_declarations
                    .push(((declaration.span.start, declaration.span.end), position));
            }
            true
        } else {
            false
        }
    }
}
impl<'a> Visit<'a> for OperandCollector<'a> {
    fn visit_statement(&mut self, statement: &Statement<'a>) {
        // Dead code that touches neither the bytecode nor the PC cannot
        // change operand decoding, such as a stack pop after a `throw`.
        if self.terminal_seen && !self.mentions_operand_state(statement) {
            return;
        }
        if self.terminal_seen {
            if !self.terminal_skip_reported {
                self.unresolved.push(
                    "unreachable decoder or PC update follows an unconditional terminal".into(),
                );
                self.terminal_skip_reported = true;
            }
            return;
        }
        walk::walk_statement(self, statement);
    }

    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        walk::walk_return_statement(self, statement);
        self.terminal_seen = true;
    }

    fn visit_throw_statement(&mut self, statement: &ThrowStatement<'a>) {
        walk::walk_throw_statement(self, statement);
        self.terminal_seen = true;
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        let symbol = match &declarator.id {
            BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
            _ => None,
        };
        if symbol == Some(self.context)
            || matches!(&declarator.id, BindingPattern::BindingIdentifier(id) if id.name.as_str() == self.context_name)
        {
            self.unresolved
                .push("handler context binding is shadowed or reassigned".into());
        }
        if let Some(init) = declarator.init.as_ref() {
            if let Some((is_bytecode, source_symbol)) = self.direct_alias(init) {
                if is_bytecode {
                    if let Some(symbol) = symbol {
                        self.bytecode_aliases.insert(symbol);
                    }
                }
                let _ = source_symbol;
            }
            if let Some(offset) = pc_offset(init, self) {
                if let Some(symbol) = symbol {
                    if self.pc_aliases.insert(symbol, offset).is_some() {
                        self.pc_writes.push(PcWrite {
                            reason: "PC alias is redeclared ambiguously".into(),
                        });
                    }
                }
                self.decoder_effect_spans
                    .push((init.span().start, init.span().end));
                self.pc_snapshot_declarations
                    .insert((declarator.span.start, declarator.span.end));
            }
            let _ = self.add_descriptor(init, Some(declarator));
        }
        if let Some(symbol) = symbol {
            self.locals.insert(symbol);
        }
        walk::walk_variable_declarator(self, declarator);
    }
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if self
            .skip_expression
            .contains(&(expression.span().start, expression.span().end))
        {
            return;
        }
        let decoded = self.add_descriptor(expression, None);
        if !decoded && self.contains_bytecode_read(expression) && is_decoder_composition(expression)
        {
            self.unresolved
                .push("unsupported or conflicting bytecode decoder composition".into());
            self.skip_expression
                .insert((expression.span().start, expression.span().end));
            return;
        }
        if self
            .skip_expression
            .contains(&(expression.span().start, expression.span().end))
        {
            return;
        }
        walk::walk_expression(self, expression);
    }
    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if let SimpleAssignmentTarget::StaticMemberExpression(member) = &update.argument {
            if self.is_context_pc(&member.object, &member.property.name) {
                if self.observable_effect_seen {
                    self.pc_writes.push(PcWrite {
                        reason:
                            "PC update occurs after an observable or potentially throwing effect"
                                .into(),
                    });
                }
                if update.operator == UpdateOperator::Increment {
                    if let Some(next) = self.pc_delta.checked_add(1) {
                        self.pc_delta = next;
                        self.decoder_effect_spans
                            .push((update.span.start, update.span.end));
                    } else {
                        self.pc_writes.push(PcWrite {
                            reason: "PC increment overflows the bounded offset".into(),
                        });
                    }
                } else {
                    self.pc_writes.push(PcWrite {
                        reason: "PC decrement is not a simple operand advance".into(),
                    });
                }
            } else if self.simple_target_mutates_bytecode(&update.argument) {
                self.bytecode_mutated = true;
                self.observable_effect_seen = true;
            } else {
                self.observable_effect_seen = true;
            }
        }
        walk::walk_update_expression(self, update);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if matches!(
            assignment.left,
            AssignmentTarget::ComputedMemberExpression(_)
        ) {
            self.observable_effect_seen = true;
        }
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
            if let Some(symbol) = self.symbol(identifier) {
                if self.pc_aliases.remove(&symbol).is_some() {
                    self.pc_writes.push(PcWrite {
                        reason: "saved PC alias is reassigned".into(),
                    });
                }
            }
        }
        if let AssignmentTarget::StaticMemberExpression(member) = &assignment.left {
            if self.is_context_pc(&member.object, &member.property.name) {
                if self.observable_effect_seen && assignment.operator != AssignmentOperator::Assign
                {
                    self.pc_writes.push(PcWrite {
                        reason:
                            "PC update occurs after an observable or potentially throwing effect"
                                .into(),
                    });
                }
                if self.jump_seen && assignment.operator != AssignmentOperator::Assign {
                    self.pc_writes.push(PcWrite {
                        reason: "PC advance follows a control-flow jump".into(),
                    });
                }
                match assignment.operator {
                    // An absolute PC write is a VM jump. HandlerIR records it as
                    // `SetProgramCounter`; it ends operand decoding for this
                    // handler but does not change the fall-through advance.
                    AssignmentOperator::Assign => self.jump_seen = true,
                    AssignmentOperator::Addition => {
                        if let Some(delta) = numeric_integer(&assignment.right) {
                            if delta >= 0 {
                                if let Some(next) = self.pc_delta.checked_add(delta as u32) {
                                    self.pc_delta = next;
                                    self.decoder_effect_spans
                                        .push((assignment.span.start, assignment.span.end));
                                } else {
                                    self.pc_writes.push(PcWrite {
                                        reason: "PC increment overflows the bounded offset".into(),
                                    });
                                }
                            } else {
                                self.pc_writes.push(PcWrite {
                                    reason: "PC decrement is not a simple operand advance".into(),
                                });
                            }
                        } else {
                            self.pc_writes.push(PcWrite {
                                reason: "PC increment is not a proven constant advance".into(),
                            });
                        }
                    }
                    _ => self.pc_writes.push(PcWrite {
                        reason: "PC assignment is neither a jump nor a constant advance".into(),
                    }),
                }
            } else {
                self.observable_effect_seen = true;
            }
        }
        if self.assignment_mutates_bytecode(&assignment.left) {
            self.bytecode_mutated = true;
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if crate::operations::templates::is_push_call_frame(call, |expression| {
            matches!(expression, Expression::Identifier(identifier)
                if self.symbol(identifier) == Some(self.context))
        }) {
            self.frame_push_deltas.push(self.pc_delta);
        }
        self.observable_effect_seen = true;
        if call
            .arguments
            .iter()
            .filter_map(Argument::as_expression)
            .any(|argument| self.expression_uses_bytecode(argument))
            || matches!(&call.callee, Expression::StaticMemberExpression(member) if self.expression_uses_bytecode(&member.object))
            || matches!(&call.callee, Expression::ComputedMemberExpression(member) if self.expression_uses_bytecode(&member.object))
        {
            self.bytecode_mutated = true;
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_function(&mut self, _function: &Function<'a>, _flags: oxc_syntax::scope::ScopeFlags) {
        // Nested closures can observe later mutations and are not part of the
        // bounded handler evaluation order.
    }
    fn visit_arrow_function_expression(&mut self, _function: &ArrowFunctionExpression<'a>) {}

    // Do not walk alternate execution paths while maintaining a single PC
    // state. A branch/loop must remain partial rather than being flattened
    // into a source-order fiction.
    fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
        // The host-presence side check never consumes operands or moves the
        // PC; HandlerIR drops it, so it is not a branch for operand purposes.
        if crate::operations::templates::is_presence_guard(statement)
            || crate::operations::templates::is_null_guard(statement)
        {
            return;
        }
        // A conditional jump reads its operands before the `if`, and its body
        // only writes the PC and state from locals: the test is evaluated
        // unconditionally and the body is an ordinary jump.
        if self.is_conditional_jump(statement) {
            walk::walk_if_statement(self, statement);
            return;
        }
        // A branch that neither reads the bytecode nor touches the PC, such
        // as a rethrow of a pending exception, does not affect decoding.
        if !self.mentions(|mentions| mentions.visit_if_statement(statement)) {
            // A `return`/`throw` inside the branch is not unconditional.
            let terminal = self.terminal_seen;
            walk::walk_if_statement(self, statement);
            self.terminal_seen = terminal;
            return;
        }
        self.unresolved
            .push("PC/operand evaluation crosses an ambiguous branch".into());
    }
    fn visit_do_while_statement(&mut self, _statement: &DoWhileStatement<'a>) {
        self.unresolved
            .push("PC/operand evaluation crosses a loop".into());
    }
    fn visit_while_statement(&mut self, _statement: &WhileStatement<'a>) {
        self.unresolved
            .push("PC/operand evaluation crosses a loop".into());
    }
    fn visit_for_statement(&mut self, _statement: &ForStatement<'a>) {
        self.unresolved
            .push("PC/operand evaluation crosses a loop".into());
    }
    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        // A loop over an object's keys that never reads the bytecode or
        // touches the PC does not affect decoding.
        if !self.mentions(|mentions| mentions.visit_for_in_statement(statement)) {
            return;
        }
        self.unresolved
            .push("PC/operand evaluation crosses a loop".into());
    }
    fn visit_for_of_statement(&mut self, _statement: &ForOfStatement<'a>) {
        self.unresolved
            .push("PC/operand evaluation crosses a loop".into());
    }
    fn visit_switch_statement(&mut self, _statement: &SwitchStatement<'a>) {
        self.unresolved
            .push("PC/operand evaluation crosses a switch".into());
    }
    fn visit_try_statement(&mut self, _statement: &TryStatement<'a>) {
        self.unresolved
            .push("PC/operand evaluation crosses exception control flow".into());
    }
    // As for a key loop: a choice that never reads the bytecode or touches
    // the PC (a type test on a stack value, say) does not affect decoding.
    fn visit_conditional_expression(&mut self, expression: &ConditionalExpression<'a>) {
        if !self.mentions(|mentions| mentions.visit_conditional_expression(expression)) {
            return;
        }
        self.unresolved
            .push("operand decoder uses an ambiguous conditional expression".into());
    }
    fn visit_logical_expression(&mut self, expression: &LogicalExpression<'a>) {
        if !self.mentions(|mentions| mentions.visit_logical_expression(expression)) {
            return;
        }
        self.unresolved
            .push("operand decoder uses an ambiguous short-circuit expression".into());
    }
}
/// Finds bytecode reads and PC reads or writes in a subtree.
struct OperandMentions<'c, 'a> {
    collector: &'c OperandCollector<'a>,
    found: bool,
}

impl<'b> Visit<'b> for OperandMentions<'_, '_> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'b>) {
        if self
            .collector
            .symbol(identifier)
            .is_some_and(|symbol| self.collector.bytecode_aliases.contains(&symbol))
        {
            self.found = true;
        }
    }
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'b>) {
        if self
            .collector
            .is_context_pc(&member.object, &member.property.name)
        {
            self.found = true;
        }
        walk::walk_static_member_expression(self, member);
    }
}

impl<'a> OperandCollector<'a> {
    /// Whether `statement` reads the bytecode or reads or writes the PC.
    fn mentions_operand_state(&self, statement: &Statement<'_>) -> bool {
        self.mentions(|mentions| mentions.visit_statement(statement))
    }
    fn mentions(&self, visit: impl FnOnce(&mut OperandMentions<'_, 'a>)) -> bool {
        let mut mentions = OperandMentions {
            collector: self,
            found: false,
        };
        visit(&mut mentions);
        mentions.found
    }
    /// `if (test) { context.<pc> = local; context.<state> = local; }`.
    fn is_conditional_jump(&self, statement: &IfStatement<'_>) -> bool {
        if statement.alternate.is_some() {
            return false;
        }
        let Statement::BlockStatement(block) = &statement.consequent else {
            return false;
        };
        let [Statement::ExpressionStatement(first), Statement::ExpressionStatement(second)] =
            block.body.as_slice()
        else {
            return false;
        };
        let context_write = |expression: &Expression<'_>, pc: bool| {
            let Expression::AssignmentExpression(assignment) = expression else {
                return false;
            };
            let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
                return false;
            };
            let is_context = matches!(&member.object, Expression::Identifier(identifier)
                if self.symbol(identifier) == Some(self.context));
            assignment.operator == AssignmentOperator::Assign
                && is_context
                && (member.property.name.as_str() == self.proof.context_property) == pc
                && matches!(
                    assignment.right.without_parentheses(),
                    Expression::Identifier(_) | Expression::NumericLiteral(_)
                )
        };
        context_write(&first.expression, true) && context_write(&second.expression, false)
    }
    fn is_context_pc(&self, object: &Expression<'_>, property: &str) -> bool {
        property == self.proof.context_property
            && matches!(object, Expression::Identifier(identifier) if self.symbol(identifier) == Some(self.context))
    }
    fn assignment_mutates_bytecode(&self, target: &AssignmentTarget<'_>) -> bool {
        match target {
            AssignmentTarget::ComputedMemberExpression(member) => {
                self.expression_uses_bytecode(&member.object)
            }
            AssignmentTarget::StaticMemberExpression(member) => {
                self.expression_uses_bytecode(&member.object)
            }
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => self
                .symbol(identifier)
                .is_some_and(|symbol| self.bytecode_aliases.contains(&symbol)),
            _ => false,
        }
    }
    fn simple_target_mutates_bytecode(&self, target: &SimpleAssignmentTarget<'_>) -> bool {
        match target {
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                self.expression_uses_bytecode(&member.object)
            }
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                self.expression_uses_bytecode(&member.object)
            }
            SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => self
                .symbol(identifier)
                .is_some_and(|symbol| self.bytecode_aliases.contains(&symbol)),
            _ => false,
        }
    }
    fn expression_uses_bytecode(&self, expression: &Expression<'_>) -> bool {
        matches!(expression, Expression::Identifier(identifier) if self.symbol(identifier).is_some_and(|symbol| self.bytecode_aliases.contains(&symbol)))
    }

    fn contains_bytecode_read(&self, expression: &Expression<'_>) -> bool {
        let mut probe = ByteReadProbe {
            collector: self,
            found: false,
        };
        probe.visit_expression(expression);
        probe.found
    }
}

struct ByteReadProbe<'a> {
    collector: &'a OperandCollector<'a>,
    found: bool,
}
impl<'a> Visit<'a> for ByteReadProbe<'a> {
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if byte_read_member(member, self.collector).is_some() {
            self.found = true;
        }
        walk::walk_computed_member_expression(self, member);
    }
}

fn is_decoder_composition(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::BinaryExpression(binary)
            if matches!(
                binary.operator,
                BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRightZeroFill
            )
    )
}

#[derive(Debug)]
struct DecoderRecipe {
    offset: u32,
    width: u8,
    endian: OperandEndian,
    signed: bool,
    mask: Option<u32>,
    spans: Vec<Span>,
}

fn decode_recipe(
    expression: &Expression<'_>,
    collector: &OperandCollector<'_>,
) -> Option<DecoderRecipe> {
    let (terms, mask, unsigned) = flatten_decoder(expression, collector)?;
    if terms.is_empty() {
        return None;
    }
    let min = terms.iter().map(|term| term.offset).min()?;
    let max = terms.iter().map(|term| term.offset).max()?;
    let width = max.checked_sub(min)?.checked_add(1)?;
    if width == 0 || width > 4 {
        return None;
    }
    let mut expected = BTreeMap::new();
    for term in &terms {
        if expected.insert(term.offset, term.shift).is_some() {
            return None;
        }
    }
    if expected.len() != width as usize {
        return None;
    }
    let little = (0..width).all(|offset| expected.get(&(min + offset)) == Some(&(offset * 8)));
    let big =
        (0..width).all(|offset| expected.get(&(min + offset)) == Some(&((width - offset - 1) * 8)));
    if !little && !big {
        return None;
    }
    Some(DecoderRecipe {
        offset: min,
        width: u8::try_from(width).ok()?,
        endian: if little {
            OperandEndian::Little
        } else {
            OperandEndian::Big
        },
        signed: !unsigned && width == 4,
        mask,
        spans: terms.into_iter().map(|term| term.span).collect(),
    })
}
#[derive(Debug)]
struct DecoderTerm {
    offset: u32,
    shift: u32,
    span: Span,
}
fn flatten_decoder(
    expression: &Expression<'_>,
    collector: &OperandCollector<'_>,
) -> Option<(Vec<DecoderTerm>, Option<u32>, bool)> {
    if let Some((_base, offset)) = byte_read(expression, collector) {
        return Some((
            vec![DecoderTerm {
                offset,
                shift: 0,
                span: expression.span(),
            }],
            None,
            false,
        ));
    }
    let Expression::BinaryExpression(binary) = expression else {
        return None;
    };
    match binary.operator {
        BinaryOperator::BitwiseOR => {
            let (mut left, left_mask, left_unsigned) = flatten_decoder(&binary.left, collector)?;
            let (right, right_mask, right_unsigned) = flatten_decoder(&binary.right, collector)?;
            // A descriptor carries one whole-value mask. A mask belonging to
            // only one OR term cannot be represented without changing the
            // operand's meaning, so fail closed for that nested form.
            if left_mask.is_some() || right_mask.is_some() {
                return None;
            }
            left.extend(right);
            Some((left, None, left_unsigned || right_unsigned))
        }
        BinaryOperator::BitwiseAnd => {
            let (terms, _, unsigned) = flatten_decoder(&binary.left, collector)?;
            let mask = numeric_integer(&binary.right).map(|value| value as u32)?;
            Some((terms, Some(mask), unsigned))
        }
        BinaryOperator::ShiftLeft => {
            let (mut terms, mask, unsigned) = flatten_decoder(&binary.left, collector)?;
            if mask.is_some() {
                return None;
            }
            let shift = u32::try_from(numeric_integer(&binary.right)?).ok()?;
            for term in &mut terms {
                term.shift = term.shift.checked_add(shift)?;
            }
            Some((terms, mask, unsigned))
        }
        BinaryOperator::ShiftRightZeroFill => {
            let (terms, mask, _) = flatten_decoder(&binary.left, collector)?;
            if numeric_integer(&binary.right)? != 0 {
                return None;
            }
            Some((terms, mask, true))
        }
        _ => None,
    }
}
fn byte_read(
    expression: &Expression<'_>,
    collector: &OperandCollector<'_>,
) -> Option<(SymbolId, u32)> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    byte_read_member(member, collector)
}
fn byte_read_member(
    member: &ComputedMemberExpression<'_>,
    collector: &OperandCollector<'_>,
) -> Option<(SymbolId, u32)> {
    let Expression::Identifier(object) = &member.object else {
        return None;
    };
    let symbol = collector.symbol(object)?;
    if !collector.bytecode_aliases.contains(&symbol) {
        return None;
    }
    let offset = pc_offset(&member.expression, collector)?;
    Some((symbol, offset))
}
fn pc_offset(expression: &Expression<'_>, collector: &OperandCollector<'_>) -> Option<u32> {
    if let Expression::Identifier(identifier) = expression {
        let symbol = collector.symbol(identifier)?;
        return collector.pc_aliases.get(&symbol).copied();
    }
    let Expression::StaticMemberExpression(member) = expression else {
        if let Expression::BinaryExpression(binary) = expression {
            let left = numeric_integer(&binary.left);
            let right = numeric_integer(&binary.right);
            match binary.operator {
                BinaryOperator::Addition if left.is_some() => {
                    return pc_offset(&binary.right, collector).and_then(|base| {
                        nonnegative_u32(left?).and_then(|offset| base.checked_add(offset))
                    })
                }
                BinaryOperator::Addition if right.is_some() => {
                    return pc_offset(&binary.left, collector).and_then(|base| {
                        nonnegative_u32(right?).and_then(|offset| base.checked_add(offset))
                    })
                }
                _ => {}
            }
        }
        return None;
    };
    collector
        .is_context_pc(&member.object, &member.property.name)
        .then_some(collector.pc_delta)
}
fn numeric_integer(expression: &Expression<'_>) -> Option<i64> {
    match expression {
        Expression::NumericLiteral(number)
            if number.value.is_finite() && number.value.fract() == 0.0 =>
        {
            Some(number.value as i64)
        }
        _ => None,
    }
}

fn nonnegative_u32(value: i64) -> Option<u32> {
    u32::try_from(value).ok()
}

fn semantic_value_has_unknown(value: &SemanticValue) -> bool {
    match value {
        SemanticValue::Unknown { .. } => true,
        other => other.children().into_iter().any(semantic_value_has_unknown),
    }
}

fn recipe_operation_has_unknown(operation: &RecipeOperation) -> bool {
    matches!(operation, RecipeOperation::Unknown { .. })
        || operation
            .values()
            .into_iter()
            .any(semantic_value_has_unknown)
}

fn semantic_value_references_temporary(value: &SemanticValue, temporary: u32) -> bool {
    match value {
        SemanticValue::Temporary { id } => *id == temporary,
        other => other
            .children()
            .into_iter()
            .any(|child| semantic_value_references_temporary(child, temporary)),
    }
}

fn recipe_operation_references_temporary(operation: &RecipeOperation, temporary: u32) -> bool {
    match operation {
        RecipeOperation::Let {
            temporary: target,
            value,
        } => *target != temporary && semantic_value_references_temporary(value, temporary),
        RecipeOperation::Assign {
            temporary: target,
            value,
        } => *target == temporary || semantic_value_references_temporary(value, temporary),
        // An unknown operation may still consume the snapshot; without its
        // semantic arguments, retain the let rather than discard a needed use.
        RecipeOperation::Unknown { .. } => true,
        other => other
            .values()
            .into_iter()
            .any(|value| semantic_value_references_temporary(value, temporary)),
    }
}

fn lift_template(
    template: &mut HandlerTemplate,
    collector: &OperandCollector<'_>,
) -> HandlerTemplate {
    let mut out = template.clone();
    for step in &mut out.steps {
        if let RecipeOperation::Let { value, .. } = &mut step.operation {
            if let Some((_, position)) = collector
                .direct_operand_declarations
                .iter()
                .find(|((start, end), _)| *start == step.span.start && *end == step.span.end)
            {
                *value = SemanticValue::Operand {
                    position: *position,
                };
            }
        }
    }
    // A decoder nested inside a larger expression, such as the string-table
    // index in `table[bytecode[pc] | bytecode[pc + 1] << 8]`, is replaced by its
    // operand in place. The builder leaves each byte read as an `Unknown` leaf
    // carrying that read's exact span, so a subtree made only of those leaves
    // and decoder arithmetic, whose leaf spans are exactly one descriptor's
    // byte-read spans, is that operand.
    let nested = collector
        .descriptors
        .iter()
        .map(|descriptor| {
            (
                descriptor
                    .source_spans
                    .iter()
                    .map(|span| (span.start, span.end))
                    .collect::<BTreeSet<_>>(),
                descriptor.position,
            )
        })
        .collect::<Vec<_>>();
    for step in &mut out.steps {
        for value in operation_values_mut(&mut step.operation) {
            substitute_nested_operands(value, &nested);
        }
    }
    let decoder_spans: BTreeSet<(u32, u32)> = collector
        .descriptors
        .iter()
        .flat_map(|descriptor| {
            descriptor
                .source_spans
                .iter()
                .map(|span| (span.start, span.end))
        })
        .collect();
    out.steps.retain(|step| {
        !matches!(&step.operation, RecipeOperation::Unknown { .. }
            if decoder_spans.contains(&(step.span.start, step.span.end))
                || collector.decoder_effect_spans.contains(&(step.span.start, step.span.end)))
    });
    let removable_snapshots = out
        .steps
        .iter()
        .filter_map(|step| {
            let RecipeOperation::Let { temporary, value } = &step.operation else {
                return None;
            };
            (collector
                .pc_snapshot_declarations
                .contains(&(step.span.start, step.span.end))
                && semantic_value_has_unknown(value)
                && !out.steps.iter().any(|other| {
                    other.span != step.span
                        && recipe_operation_references_temporary(&other.operation, *temporary)
                }))
            .then_some((step.span.start, step.span.end))
        })
        .collect::<BTreeSet<_>>();
    out.steps
        .retain(|step| !removable_snapshots.contains(&(step.span.start, step.span.end)));
    out.unsupported.retain(|fragment| {
        !decoder_spans.contains(&(fragment.span.start, fragment.span.end))
            && !collector
                .decoder_effect_spans
                .contains(&(fragment.span.start, fragment.span.end))
            && !collector
                .direct_operand_declarations
                .iter()
                .any(|((start, end), _)| *start == fragment.span.start && *end == fragment.span.end)
    });
    out
}

fn operation_values_mut(operation: &mut RecipeOperation) -> Vec<&mut SemanticValue> {
    operation.values_mut()
}

/// Collect the byte-read leaf spans of a pure decoder subtree. Returns false
/// when the subtree contains anything other than unknown byte reads, literals
/// and binary decoder arithmetic.
fn decoder_leaf_spans(value: &SemanticValue, spans: &mut BTreeSet<(u32, u32)>) -> bool {
    match value {
        SemanticValue::Unknown { span, .. } => {
            spans.insert((span.start, span.end));
            true
        }
        SemanticValue::Literal(_) => true,
        SemanticValue::Binary { left, right, .. } => {
            decoder_leaf_spans(left, spans) && decoder_leaf_spans(right, spans)
        }
        _ => false,
    }
}

fn substitute_nested_operands(
    value: &mut SemanticValue,
    descriptors: &[(BTreeSet<(u32, u32)>, u32)],
) {
    let mut spans = BTreeSet::new();
    if decoder_leaf_spans(value, &mut spans) && !spans.is_empty() {
        if let Some((_, position)) = descriptors.iter().find(|(leaves, _)| *leaves == spans) {
            *value = SemanticValue::Operand {
                position: *position,
            };
            return;
        }
    }
    for child in value.children_mut() {
        substitute_nested_operands(child, descriptors);
    }
}

fn captured_dependencies(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    function: &Handler<'_>,
    proof: &DecoderProof,
    context: SymbolId,
    locals: &BTreeSet<SymbolId>,
    parameters: &BTreeSet<SymbolId>,
) -> Vec<CapturedDependency> {
    let mut collector = DependencyCollector {
        scoping,
        nodes,
        dependencies: BTreeMap::new(),
        proof,
        context,
        locals,
        parameters,
    };
    if let Some(body) = function.body() {
        for statement in &body.statements {
            collector.visit_statement(statement);
        }
    }
    collector.dependencies.into_values().collect()
}
struct DependencyCollector<'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    dependencies: BTreeMap<SymbolId, CapturedDependency>,
    proof: &'a DecoderProof,
    context: SymbolId,
    locals: &'a BTreeSet<SymbolId>,
    parameters: &'a BTreeSet<SymbolId>,
}
impl<'a> DependencyCollector<'a> {
    /// A captured binding is a helper only when its resolved declaration is
    /// structurally a function (function declaration, function expression, or
    /// arrow function), optionally reached through a chain of plain, immutable
    /// aliases.  The spelling of the binding never participates.
    fn captured_role(&self, symbol: SymbolId) -> CapturedDependencyRole {
        if self.is_function_binding(symbol, &mut BTreeSet::new()) {
            CapturedDependencyRole::Helper
        } else {
            CapturedDependencyRole::External
        }
    }

    fn is_function_binding(&self, symbol: SymbolId, visiting: &mut BTreeSet<SymbolId>) -> bool {
        // Conservative: a reassigned binding may hold a non-function value, so
        // its role is not proven.
        if self.scoping.symbol_is_mutated(symbol) || !visiting.insert(symbol) {
            return false;
        }
        match self.nodes.kind(self.scoping.symbol_declaration(symbol)) {
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => true,
            AstKind::VariableDeclarator(declarator) => match declarator.init.as_ref() {
                Some(Expression::FunctionExpression(_))
                | Some(Expression::ArrowFunctionExpression(_)) => true,
                // An alias is only proven when it is a plain reference to
                // another binding, and that binding itself has a function
                // declaration shape.
                Some(Expression::Identifier(identifier)) => {
                    analysis::identifier_symbol(identifier, self.scoping)
                        .is_some_and(|target| self.is_function_binding(target, visiting))
                }
                _ => false,
            },
            _ => false,
        }
    }
}
impl<'a> Visit<'a> for DependencyCollector<'a> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
            return;
        };
        if symbol == self.context
            || self.locals.contains(&symbol)
            || self.parameters.contains(&symbol)
            || symbol == self.proof.bytecode
        {
            return;
        }
        let declaration_span = OperandSpan {
            start: self.scoping.symbol_span(symbol).start,
            end: self.scoping.symbol_span(symbol).end,
        };
        let role = self.captured_role(symbol);
        self.dependencies
            .entry(symbol)
            .or_insert(CapturedDependency {
                name: identifier.name.to_string(),
                role,
                declaration_span,
            });
    }
    fn visit_function(&mut self, _function: &Function<'a>, _flags: oxc_syntax::scope::ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _function: &ArrowFunctionExpression<'a>) {}
}

pub(super) fn span_record(span: Span) -> OperandSpan {
    OperandSpan {
        start: span.start,
        end: span.end,
    }
}
pub(super) fn js_int32(value: u64) -> i32 {
    (value as u32) as i32
}
