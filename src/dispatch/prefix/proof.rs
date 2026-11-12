//! Narrow, `SymbolId`-based source proofs for the bounded JavaScript VM prefix.
//!
//! These proofs are deliberately *not* the complete-dispatch proofs in
//! `../proof.rs`. They recognize exactly two structural topologies in the
//! JavaScript VM prefix:
//!
//! 1. the run loop
//!    `var v, O; for (;;) { if (A !== B) { O = A; A = B; return O; }
//!    v = ctx.q(); if (ctx.G.length === 0) ops[v](ctx); else BH(ops[v], ctx); }`
//!    for the already-proven dispatch method name, operation-table symbol, and
//!    constructor. The declaration is exactly two distinct uninitialized
//!    bindings; the completion sentinel is immutable and the completion
//!    register is initialized to exactly that sentinel; the guarded exception
//!    stack is initialized empty by the proven constructor; the caller
//!    receiver originates from that constructor; and the functions must not be
//!    async/generator;
//! 2. the single bounded bytecode-write handler whose body is exactly
//!    `var a = image[pc] | (image[pc + 1] | image[pc + 2] << 8) << 8;
//!    var b = image[pc + 3]; pc += 4; image[a] = b;`
//!    where `image` and `pc` are the proven dispatch bytecode binding and PC
//!    property, the address/value/context bindings are distinct, and every
//!    other reference to the image is a computed read. No selector, name,
//!    address, hash or seed constant is baked in.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::node::NodeId;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator, UpdateOperator};

use super::{PATCH_ADDRESS_BYTES, PATCH_OPERAND_WIDTH};
use crate::operations::operands::OperandSpan;

fn operand_span(span: Span) -> OperandSpan {
    OperandSpan {
        start: span.start,
        end: span.end,
    }
}

fn identifier_symbol(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> Option<SymbolId> {
    crate::analysis::identifier_symbol(identifier, scoping)
}

/// A recognized bounded bytecode-image write handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PatchHandlerProof {
    pub selector: u32,
    pub span: OperandSpan,
    pub operand_width: usize,
}

/// Topology evidence for the JavaScript VM run loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunLoopProof {
    pub span: OperandSpan,
    /// The named function that owns the run loop. Its calls are proved to
    /// receive instances of the dispatch method's proven constructor.
    /// Compile-time source identity. It is intentionally absent in a loaded
    /// artifact: SymbolIds belong to one ephemeral Oxc AST and are never
    /// runtime data.
    pub function_symbol: Option<SymbolId>,
    pub completion_register: String,
    pub completion_register_symbol: Option<SymbolId>,
    pub completion_sentinel_symbol: Option<SymbolId>,
    /// The exception-stack member the guarded operation call tests
    /// (`context.<field>.length === 0`). The proven constructor initializes it
    /// to a fresh empty stack.
    pub exception_stack_field: String,
    pub exception_helper: Option<OperandSpan>,
}

/// Prove exactly one bounded bytecode-write handler against the proven image
/// and PC bindings. `handlers` pairs each operation-table selector with its
/// function span. Any other write to the image, any write in a non-handler
/// function, or any near-miss body is rejected.
pub(super) fn prove_patch_handler(
    source: &str,
    bytecode: SymbolId,
    pc_property: &str,
    handlers: &[(u32, OperandSpan)],
) -> Result<PatchHandlerProof, String> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source)
        .map_err(|error| format!("patch-handler parse failed: {error}"))?;
    let (scoping, _nodes) = crate::analysis::analyze(&program);

    let mut scan = WriteScan {
        scoping: &scoping,
        bytecode,
        functions: Vec::new(),
        writes: Vec::new(),
        top_level: Vec::new(),
        escapes: Vec::new(),
    };
    scan.visit_program(&program);
    if let Some(span) = scan.escapes.first() {
        return Err(format!(
            "bytecode image is aliased or escapes into a callee at bytes {}..{}",
            span.start, span.end
        ));
    }
    if let Some(span) = scan.top_level.first() {
        return Err(format!(
            "bytecode image is written outside any function at bytes {}..{}",
            span.start, span.end
        ));
    }
    let mut by_function: BTreeMap<(u32, u32), Vec<Span>> = BTreeMap::new();
    for (function, target) in &scan.writes {
        by_function
            .entry((function.start, function.end))
            .or_default()
            .push(*target);
    }
    if by_function.is_empty() {
        return Err("no bounded bytecode-image write handler was proven".into());
    }
    if by_function.len() > 1 {
        return Err(format!(
            "bytecode image is written by {} distinct functions; exactly one bounded handler is required",
            by_function.len()
        ));
    }
    let ((start, end), writes) = by_function
        .into_iter()
        .next()
        .expect("one writing function");
    if writes.len() != 1 {
        return Err(format!(
            "bytecode-image handler writes the image {} times; the bounded template writes exactly once",
            writes.len()
        ));
    }
    let function_span = OperandSpan { start, end };
    let mut selectors = handlers
        .iter()
        .filter(|(_, span)| *span == function_span)
        .map(|(selector, _)| *selector);
    let selector = selectors.next().ok_or_else(|| {
        "bytecode-image write is not in a proven operation-table handler".to_string()
    })?;
    if selectors.next().is_some() {
        return Err("bytecode-image handler selector is ambiguous".into());
    }

    let mut validator = PatchBodyValidator {
        target: function_span.clone(),
        scoping: &scoping,
        bytecode,
        pc_property,
        result: Err("bytecode-image handler body was not found".into()),
    };
    validator.visit_program(&program);
    let operand_width = validator.result?;

    // Conservative image-reference whitelist. Every reference to the proven
    // bytecode symbol must be the object of a computed member read/write inside
    // the one proven patch handler, or the single nested dispatch read
    // `image[this.<pc>++]`. This rejects rebinding, `delete`, object/array/
    // conditional aliasing, and escapes that a member-write scan alone misses.
    let mut reference_scan = ImageReferenceScan {
        scoping: &scoping,
        bytecode,
        patch_function: function_span.clone(),
        functions: Vec::new(),
        allowed: BTreeSet::new(),
        violations: Vec::new(),
    };
    reference_scan.visit_program(&program);
    if let Some(violation) = reference_scan.violations.into_iter().next() {
        return Err(violation);
    }

    Ok(PatchHandlerProof {
        selector,
        span: function_span,
        operand_width,
    })
}

/// Conservative reference whitelist for the proven bytecode image.
struct ImageReferenceScan<'a> {
    scoping: &'a Scoping,
    bytecode: SymbolId,
    patch_function: OperandSpan,
    functions: Vec<OperandSpan>,
    allowed: BTreeSet<(u32, u32)>,
    violations: Vec<String>,
}

impl<'a> ImageReferenceScan<'a> {
    fn symbol(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        identifier_symbol(identifier, self.scoping)
    }

    fn current_function(&self) -> Option<OperandSpan> {
        self.functions.last().cloned()
    }

    fn in_patch_function(&self) -> bool {
        self.current_function().as_ref() == Some(&self.patch_function)
    }

    fn roots_at_bytecode(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::Identifier(identifier) => self.symbol(identifier) == Some(self.bytecode),
            Expression::StaticMemberExpression(member) => self.roots_at_bytecode(&member.object),
            Expression::ComputedMemberExpression(member) => self.roots_at_bytecode(&member.object),
            _ => false,
        }
    }
}

impl<'a> Visit<'a> for ImageReferenceScan<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.roots_at_bytecode(&call.callee) {
            self.violations
                .push("bytecode image is used as a call receiver or callee".into());
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_new_expression(&mut self, call: &NewExpression<'a>) {
        if self.roots_at_bytecode(&call.callee) {
            self.violations
                .push("bytecode image is used as a constructor callee".into());
        }
        walk::walk_new_expression(self, call);
    }

    fn visit_tagged_template_expression(&mut self, expression: &TaggedTemplateExpression<'a>) {
        if self.roots_at_bytecode(&expression.tag) {
            self.violations
                .push("bytecode image is used as a template callee".into());
        }
        walk::walk_tagged_template_expression(self, expression);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.functions.push(operand_span(function.span));
        walk::walk_function(self, function, flags);
        self.functions.pop();
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        self.functions.push(operand_span(function.span));
        walk::walk_arrow_function_expression(self, function);
        self.functions.pop();
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if let Expression::Identifier(identifier) = &member.object {
            if self.symbol(identifier) == Some(self.bytecode) {
                // A computed read of the image yields a byte value, not an
                // alias, so it is safe anywhere. Computed *writes* are confined
                // separately by the assignment/update visitors below.
                self.allowed
                    .insert((identifier.span.start, identifier.span.end));
            }
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_assignment_target(&mut self, target: &AssignmentTarget<'a>) {
        if let AssignmentTarget::ComputedMemberExpression(member) = target {
            if let Expression::Identifier(identifier) = &member.object {
                if self.symbol(identifier) == Some(self.bytecode) && !self.in_patch_function() {
                    self.violations.push(format!(
                        "bytecode image is written outside the proven patch handler at bytes {}..{}",
                        identifier.span.start, identifier.span.end
                    ));
                }
            }
        }
        walk::walk_assignment_target(self, target);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if let SimpleAssignmentTarget::ComputedMemberExpression(member) = &update.argument {
            if let Expression::Identifier(identifier) = &member.object {
                if self.symbol(identifier) == Some(self.bytecode) && !self.in_patch_function() {
                    self.violations.push(format!(
                        "bytecode image is updated outside the proven patch handler at bytes {}..{}",
                        identifier.span.start, identifier.span.end
                    ));
                }
            }
        }
        walk::walk_update_expression(self, update);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if self.symbol(identifier) == Some(self.bytecode)
            && !self
                .allowed
                .contains(&(identifier.span.start, identifier.span.end))
        {
            self.violations.push(format!(
                "bytecode image identifier is rebound, aliased, or escapes at bytes {}..{}",
                identifier.span.start, identifier.span.end
            ));
        }
        walk::walk_identifier_reference(self, identifier);
    }

    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::Delete && self.roots_at_bytecode(&unary.argument) {
            self.violations
                .push("bytecode image byte is deleted (unsupported mutation)".into());
        }
        walk::walk_unary_expression(self, unary);
    }
}

struct WriteScan<'a> {
    scoping: &'a Scoping,
    bytecode: SymbolId,
    functions: Vec<Span>,
    writes: Vec<(Span, Span)>,
    top_level: Vec<Span>,
    escapes: Vec<Span>,
}

impl WriteScan<'_> {
    fn record(&mut self, target: Span) {
        match self.functions.last().copied() {
            Some(function) => self.writes.push((function, target)),
            None => self.top_level.push(target),
        }
    }

    fn is_bytecode_identifier(&self, expression: &Expression<'_>) -> bool {
        matches!(expression, Expression::Identifier(identifier)
            if identifier_symbol(identifier, self.scoping) == Some(self.bytecode))
    }

    fn roots_at_bytecode(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::Identifier(identifier) => {
                identifier_symbol(identifier, self.scoping) == Some(self.bytecode)
            }
            Expression::StaticMemberExpression(member) => self.roots_at_bytecode(&member.object),
            Expression::ComputedMemberExpression(member) => self.roots_at_bytecode(&member.object),
            _ => false,
        }
    }
}

impl<'a> Visit<'a> for WriteScan<'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.functions.push(function.span);
        walk::walk_function(self, function, flags);
        self.functions.pop();
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        self.functions.push(function.span);
        walk::walk_arrow_function_expression(self, function);
        self.functions.pop();
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        // A simple `alias = image` rebinds the image and can carry writes the
        // bounded proof would not see.
        if assignment.operator == AssignmentOperator::Assign {
            if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                if self.is_bytecode_identifier(&assignment.right) {
                    self.escapes.push(identifier.span);
                }
            }
        }
        match &assignment.left {
            AssignmentTarget::StaticMemberExpression(member) => {
                if self.roots_at_bytecode(&member.object) {
                    self.record(member.span);
                }
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                if self.roots_at_bytecode(&member.object) {
                    self.record(member.span);
                }
            }
            _ => {}
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if update.operator == UpdateOperator::Increment
            || update.operator == UpdateOperator::Decrement
        {
            match &update.argument {
                SimpleAssignmentTarget::StaticMemberExpression(member) => {
                    if self.roots_at_bytecode(&member.object) {
                        self.record(member.span);
                    }
                }
                SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                    if self.roots_at_bytecode(&member.object) {
                        self.record(member.span);
                    }
                }
                _ => {}
            }
        }
        walk::walk_update_expression(self, update);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let Some(init) = declarator.init.as_ref() {
            if self.is_bytecode_identifier(init) {
                self.escapes.push(declarator.id.span());
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        for argument in call.arguments.iter().filter_map(Argument::as_expression) {
            if self.is_bytecode_identifier(argument) {
                self.escapes.push(argument.span());
            }
        }
        walk::walk_call_expression(self, call);
    }
}

struct PatchBodyValidator<'a, 'b> {
    target: OperandSpan,
    scoping: &'a Scoping,
    bytecode: SymbolId,
    pc_property: &'b str,
    result: Result<usize, String>,
}

impl<'a, 'b> PatchBodyValidator<'a, 'b> {
    fn context_symbol(&self, function: &Function<'_>) -> Option<SymbolId> {
        let [parameter] = function.params.items.as_slice() else {
            return None;
        };
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        identifier.symbol_id.get()
    }

    fn is_context_pc_member(&self, member: &StaticMemberExpression<'_>, context: SymbolId) -> bool {
        member.property.name == self.pc_property
            && matches!(&member.object, Expression::Identifier(identifier)
                if identifier_symbol(identifier, self.scoping) == Some(context))
    }

    fn is_context_pc(&self, expression: &Expression<'_>, context: SymbolId) -> bool {
        let Expression::StaticMemberExpression(member) = expression else {
            return false;
        };
        self.is_context_pc_member(member, context)
    }

    fn is_identifier(&self, expression: &Expression<'_>, symbol: SymbolId) -> bool {
        matches!(expression, Expression::Identifier(identifier)
            if identifier_symbol(identifier, self.scoping) == Some(symbol))
    }

    fn is_literal(&self, expression: &Expression<'_>, value: f64) -> bool {
        matches!(expression, Expression::NumericLiteral(literal) if literal.value == value)
    }

    /// Match `image[context.PC]` (offset 0) or `image[context.PC + k]`.
    fn bytecode_offset(&self, expression: &Expression<'_>, context: SymbolId) -> Option<u32> {
        let Expression::ComputedMemberExpression(member) = expression else {
            return None;
        };
        if !self.is_identifier(&member.object, self.bytecode) {
            return None;
        }
        match &member.expression {
            Expression::StaticMemberExpression(_)
                if self.is_context_pc(&member.expression, context) =>
            {
                Some(0)
            }
            Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
                if !self.is_context_pc(&binary.left, context) {
                    return None;
                }
                let Expression::NumericLiteral(offset) = &binary.right else {
                    return None;
                };
                if !offset.value.is_finite()
                    || offset.value.fract() != 0.0
                    || offset.value < 1.0
                    || offset.value > 8.0
                {
                    return None;
                }
                Some(offset.value as u32)
            }
            _ => None,
        }
    }

    fn shift_by_eight<'e>(&self, expression: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
        let Expression::BinaryExpression(binary) = expression else {
            return None;
        };
        (binary.operator == BinaryOperator::ShiftLeft && self.is_literal(&binary.right, 8.0))
            .then_some(&binary.left)
    }

    fn address_expression(&self, expression: &Expression<'_>, context: SymbolId) -> bool {
        let Expression::BinaryExpression(outer) = expression else {
            return false;
        };
        if outer.operator != BinaryOperator::BitwiseOR {
            return false;
        }
        if self.bytecode_offset(&outer.left, context) != Some(0) {
            return false;
        }
        // (image[pc + 1] | image[pc + 2] << 8) << 8
        let Some(inner) = self.shift_by_eight(&outer.right) else {
            return false;
        };
        let Expression::BinaryExpression(or) = inner else {
            return false;
        };
        if or.operator != BinaryOperator::BitwiseOR {
            return false;
        }
        if self.bytecode_offset(&or.left, context) != Some(1) {
            return false;
        }
        let Some(high) = self.shift_by_eight(&or.right) else {
            return false;
        };
        self.bytecode_offset(high, context) == Some(PATCH_ADDRESS_BYTES as u32 - 1)
    }

    fn check(&self, function: &Function<'_>) -> Result<usize, String> {
        if function.generator || function.r#async {
            return Err("bytecode-image handler must not be an async or generator function".into());
        }
        let context = self.context_symbol(function).ok_or_else(|| {
            "bytecode-image handler does not take one context parameter".to_string()
        })?;
        let body = function
            .body
            .as_deref()
            .ok_or_else(|| "bytecode-image handler has no body".to_string())?;
        let statements = match body.statements.as_slice() {
            [Statement::BlockStatement(block)] => block.body.as_slice(),
            statements => statements,
        };
        let [first, second, third, fourth] = statements else {
            return Err(format!(
                "bytecode-image handler must have exactly the four-statement template, found {} statements",
                statements.len()
            ));
        };
        let Statement::VariableDeclaration(first) = first else {
            return Err("bytecode-image handler step 0 is not an address declaration".into());
        };
        let [address] = first.declarations.as_slice() else {
            return Err("bytecode-image handler step 0 must declare exactly one address".into());
        };
        let BindingPattern::BindingIdentifier(address_id) = &address.id else {
            return Err("bytecode-image handler address is not a simple binding".into());
        };
        let Some(address_symbol) = address_id.symbol_id.get() else {
            return Err("bytecode-image handler address has no binding identity".into());
        };
        let Some(address_init) = address.init.as_ref() else {
            return Err("bytecode-image handler address has no initializer".into());
        };
        if !self.address_expression(address_init, context) {
            return Err(
                "bytecode-image handler address is not the exact 24-bit little-endian template"
                    .into(),
            );
        }

        let Statement::VariableDeclaration(second) = second else {
            return Err("bytecode-image handler step 1 is not a value declaration".into());
        };
        let [value] = second.declarations.as_slice() else {
            return Err("bytecode-image handler step 1 must declare exactly one value".into());
        };
        let BindingPattern::BindingIdentifier(value_id) = &value.id else {
            return Err("bytecode-image handler value is not a simple binding".into());
        };
        let Some(value_symbol) = value_id.symbol_id.get() else {
            return Err("bytecode-image handler value has no binding identity".into());
        };
        if address_symbol == value_symbol || address_symbol == context || value_symbol == context {
            return Err(
                "bytecode-image handler address, value, and context must be distinct fresh bindings"
                    .into(),
            );
        }
        let Some(value_init) = value.init.as_ref() else {
            return Err("bytecode-image handler value has no initializer".into());
        };
        if self.bytecode_offset(value_init, context) != Some(PATCH_ADDRESS_BYTES as u32) {
            return Err(format!(
                "bytecode-image handler value is not image[pc + {PATCH_ADDRESS_BYTES}]"
            ));
        }

        let Statement::ExpressionStatement(third) = third else {
            return Err("bytecode-image handler step 2 is not a PC advance".into());
        };
        let Expression::AssignmentExpression(advance) = &third.expression else {
            return Err("bytecode-image handler step 2 is not an assignment".into());
        };
        if advance.operator != AssignmentOperator::Addition
            || !matches!(&advance.left, AssignmentTarget::StaticMemberExpression(member)
                if self.is_context_pc_member(member, context))
            || !self.is_literal(&advance.right, PATCH_OPERAND_WIDTH as f64)
        {
            return Err(format!(
                "bytecode-image handler PC advance is not `context.<pc> += {PATCH_OPERAND_WIDTH}`"
            ));
        }

        let Statement::ExpressionStatement(fourth) = fourth else {
            return Err("bytecode-image handler step 3 is not a write".into());
        };
        let Expression::AssignmentExpression(write) = &fourth.expression else {
            return Err("bytecode-image handler step 3 is not an assignment".into());
        };
        if write.operator != AssignmentOperator::Assign {
            return Err("bytecode-image handler step 3 must be a plain assignment".into());
        }
        let AssignmentTarget::ComputedMemberExpression(target) = &write.left else {
            return Err("bytecode-image handler step 3 does not index the image binding".into());
        };
        if !self.is_identifier(&target.object, self.bytecode)
            || !self.is_identifier(&target.expression, address_symbol)
            || !self.is_identifier(&write.right, value_symbol)
        {
            return Err(
                "bytecode-image handler step 3 is not `image[address] = value` on the declared temps"
                    .into(),
            );
        }
        Ok(PATCH_OPERAND_WIDTH)
    }
}

impl<'a, 'b> Visit<'a> for PatchBodyValidator<'a, 'b> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if operand_span(function.span) == self.target {
            self.result = self.check(function);
        }
        walk::walk_function(self, function, flags);
    }
}

/// Prove the JavaScript VM run loop joined to the proven dispatch method, operation
/// table, and dispatch constructor. The operation table is resolved by its
/// code-generation stable declaration ordinal.
pub(super) fn prove_run_loop(
    source: &str,
    method_name: &str,
    operation_ordinal: u32,
    context_constructor: SymbolId,
    handler_spans: &[(u32, OperandSpan)],
) -> Result<RunLoopProof, String> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source)
        .map_err(|error| format!("run-loop parse failed: {error}"))?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let operation_table =
        super::super::proof::operation_table_symbol(&program, &scoping, operation_ordinal)
            .ok_or_else(|| {
                "operation-table declaration was not found for the run-loop join".to_string()
            })?;
    let mut finder = RunLoopFinder {
        scoping: &scoping,
        method_name,
        operation_table,
        found: Vec::new(),
    };
    finder.visit_program(&program);
    let proof = match finder.found.as_slice() {
        [proof] => proof.clone(),
        [] => return Err("the supported JavaScript VM run loop was not found".into()),
        _ => return Err("more than one JavaScript VM run loop was found".into()),
    };

    // The run-loop receiver must originate from the dispatch method's proven
    // constructor, not merely share the dispatch property spelling. Reuse the
    // complete-dispatch origin analysis rather than a loose search.
    super::super::proof::prove_consumer_context_origin(
        &program,
        &scoping,
        &nodes,
        proof
            .function_symbol
            .ok_or_else(|| "run-loop proof has no function identity".to_string())?,
        context_constructor,
    )
    .map_err(|error| format!("run-loop receiver origin rejected: {error}"))?;

    prove_run_loop_provenance(
        &program,
        &scoping,
        &nodes,
        &proof,
        context_constructor,
        handler_spans,
    )
    .map_err(|error| format!("run-loop provenance rejected: {error}"))?;

    Ok(proof)
}

/// The exact, validated run-loop declarations: one uninitialized selector temp
/// and one uninitialized return temp, as named bindings.
///
/// The readable pipeline may print both declarators in one `var` statement or
/// split them into one `var` statement each as a presentation choice. Both
/// forms are accepted only while they still name exactly those two
/// uninitialized temps, in order, immediately before the loop.
fn declared_loop_temps<'a>(
    statements: &'a [Statement<'a>],
) -> Option<(SymbolId, SymbolId, &'a Statement<'a>)> {
    match statements {
        [declaration, loop_statement] => {
            let declaration = loop_temp_declaration(declaration)?;
            let [first, second] = declaration.declarations.as_slice() else {
                return None;
            };
            let first = plain_uninitialized_binding(first)?;
            let second = plain_uninitialized_binding(second)?;
            distinct_temps(first, second, loop_statement)
        }
        [first_statement, second_statement, loop_statement] => {
            let first = single_loop_temp(first_statement)?;
            let second = single_loop_temp(second_statement)?;
            distinct_temps(first, second, loop_statement)
        }
        _ => None,
    }
}

fn distinct_temps<'a>(
    first: SymbolId,
    second: SymbolId,
    loop_statement: &'a Statement<'a>,
) -> Option<(SymbolId, SymbolId, &'a Statement<'a>)> {
    (first != second).then_some((first, second, loop_statement))
}

fn loop_temp_declaration<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclaration<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    (declaration.kind == VariableDeclarationKind::Var).then_some(declaration)
}

fn single_loop_temp(statement: &Statement<'_>) -> Option<SymbolId> {
    let declaration = loop_temp_declaration(statement)?;
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    plain_uninitialized_binding(declarator)
}

fn plain_uninitialized_binding(declarator: &VariableDeclarator<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
        return None;
    };
    if declarator.init.is_some() {
        return None;
    }
    identifier.symbol_id.get()
}

struct CompletionGuard {
    register_name: String,
    register: SymbolId,
    sentinel: SymbolId,
    temp: SymbolId,
}

struct HandlerJoin {
    exception_helper: Option<OperandSpan>,
    exception_stack_field: String,
}

struct RunLoopFinder<'a, 'b> {
    scoping: &'a Scoping,
    method_name: &'b str,
    operation_table: SymbolId,
    found: Vec<RunLoopProof>,
}

impl<'a, 'b> RunLoopFinder<'a, 'b> {
    fn parameter_symbols(&self, function: &Function<'_>) -> Vec<SymbolId> {
        function
            .params
            .items
            .iter()
            .filter_map(|parameter| match &parameter.pattern {
                BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
                _ => None,
            })
            .collect()
    }

    fn check(&self, function: &Function<'_>) -> Option<RunLoopProof> {
        if function.generator || function.r#async {
            return None;
        }
        let body = function.body.as_deref()?;
        let (selector_temp, return_temp, loop_statement) =
            declared_loop_temps(body.statements.as_slice())?;
        let Statement::ForStatement(loop_statement) = loop_statement else {
            return None;
        };
        if loop_statement.init.is_some()
            || loop_statement.test.is_some()
            || loop_statement.update.is_some()
        {
            return None;
        }
        let Statement::BlockStatement(loop_body) = &loop_statement.body else {
            return None;
        };
        let [guard, dispatch, handler] = loop_body.body.as_slice() else {
            return None;
        };
        let parameters = self.parameter_symbols(function);
        let [context] = parameters.as_slice() else {
            return None;
        };

        let completion = self.check_completion_guard(guard)?;
        let selector = self.check_dispatch(dispatch, *context)?;
        if selector != selector_temp || completion.temp != return_temp {
            return None;
        }
        let HandlerJoin {
            exception_helper,
            exception_stack_field,
        } = self.check_handler(handler, *context, selector)?;
        let function_symbol = function.id.as_ref()?.symbol_id.get()?;
        Some(RunLoopProof {
            span: operand_span(function.span),
            function_symbol: Some(function_symbol),
            completion_register: completion.register_name,
            completion_register_symbol: Some(completion.register),
            completion_sentinel_symbol: Some(completion.sentinel),
            exception_stack_field,
            exception_helper,
        })
    }

    /// `if (A !== B) { C = A; A = B; return C; }`
    fn check_completion_guard(&self, statement: &Statement<'_>) -> Option<CompletionGuard> {
        let Statement::IfStatement(guard) = statement else {
            return None;
        };
        if guard.alternate.is_some() {
            return None;
        }
        let Expression::BinaryExpression(test) = &guard.test else {
            return None;
        };
        if test.operator != BinaryOperator::StrictInequality {
            return None;
        }
        let (Expression::Identifier(register), Expression::Identifier(not_complete)) =
            (&test.left, &test.right)
        else {
            return None;
        };
        let register_symbol = identifier_symbol(register, self.scoping)?;
        let not_complete_symbol = identifier_symbol(not_complete, self.scoping)?;
        let Statement::BlockStatement(block) = &guard.consequent else {
            return None;
        };
        let [first, second, returned] = block.body.as_slice() else {
            return None;
        };
        // C = A
        let Statement::ExpressionStatement(first) = first else {
            return None;
        };
        let Expression::AssignmentExpression(first) = &first.expression else {
            return None;
        };
        if first.operator != AssignmentOperator::Assign
            || !matches!(&first.right, Expression::Identifier(id)
                if identifier_symbol(id, self.scoping) == Some(register_symbol))
        {
            return None;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(temp) = &first.left else {
            return None;
        };
        let temp_symbol = identifier_symbol(temp, self.scoping)?;
        // A = B
        let Statement::ExpressionStatement(second) = second else {
            return None;
        };
        let Expression::AssignmentExpression(second) = &second.expression else {
            return None;
        };
        if second.operator != AssignmentOperator::Assign
            || !matches!(&second.left, AssignmentTarget::AssignmentTargetIdentifier(id)
                if identifier_symbol(id, self.scoping) == Some(register_symbol))
            || !matches!(&second.right, Expression::Identifier(id)
                if identifier_symbol(id, self.scoping) == Some(not_complete_symbol))
        {
            return None;
        }
        // return C
        let Statement::ReturnStatement(returned) = returned else {
            return None;
        };
        if !matches!(returned.argument.as_ref(), Some(Expression::Identifier(id))
            if identifier_symbol(id, self.scoping) == Some(temp_symbol))
        {
            return None;
        }
        Some(CompletionGuard {
            register_name: register.name.to_string(),
            register: register_symbol,
            sentinel: not_complete_symbol,
            temp: temp_symbol,
        })
    }

    /// `selector = context.q()`
    fn check_dispatch(&self, statement: &Statement<'_>, context: SymbolId) -> Option<SymbolId> {
        let Statement::ExpressionStatement(statement) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &statement.expression else {
            return None;
        };
        if assignment.operator != AssignmentOperator::Assign {
            return None;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(selector) = &assignment.left else {
            return None;
        };
        let selector_symbol = identifier_symbol(selector, self.scoping)?;
        let Expression::CallExpression(call) = &assignment.right else {
            return None;
        };
        if !call.arguments.is_empty() {
            return None;
        }
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        if member.property.name != self.method_name
            || !matches!(&member.object, Expression::Identifier(id)
                if identifier_symbol(id, self.scoping) == Some(context))
        {
            return None;
        }
        Some(selector_symbol)
    }

    /// Extract the single call expression of a statement, unwrapping one
    /// transparent block (`{ call(); }`).
    fn statement_call<'x>(&self, statement: &'x Statement<'a>) -> Option<&'x CallExpression<'a>> {
        let statement = match statement {
            Statement::BlockStatement(block) => {
                let [only] = block.body.as_slice() else {
                    return None;
                };
                only
            }
            statement => statement,
        };
        let Statement::ExpressionStatement(statement) = statement else {
            return None;
        };
        let Expression::CallExpression(call) = &statement.expression else {
            return None;
        };
        Some(call)
    }

    /// `if (context.G.length === 0) operations[selector](context);
    ///  else exceptionHelper(operations[selector], context);`
    fn check_handler(
        &self,
        statement: &Statement<'_>,
        context: SymbolId,
        selector: SymbolId,
    ) -> Option<HandlerJoin> {
        let Statement::IfStatement(statement) = statement else {
            return None;
        };
        let exception_stack_field = self.check_stack_guard(&statement.test, context)?;
        let consequent = self.statement_call(&statement.consequent)?;
        if !self.is_operation_call(consequent, context, selector) {
            return None;
        }
        let alternate = self.statement_call(statement.alternate.as_ref()?)?;
        let expressions = alternate
            .arguments
            .iter()
            .map(Argument::as_expression)
            .collect::<Option<Vec<_>>>()?;
        let [argument, receiver] = expressions.as_slice() else {
            return None;
        };
        if !self.is_operation_member(argument, selector) || !self.is_identifier(receiver, context) {
            return None;
        }
        Some(HandlerJoin {
            exception_helper: alternate_callee_span(&alternate.callee),
            exception_stack_field,
        })
    }

    /// `context.<field>.length === 0`, returning the guarded member name.
    fn check_stack_guard(&self, test: &Expression<'_>, context: SymbolId) -> Option<String> {
        let Expression::BinaryExpression(test) = test else {
            return None;
        };
        if test.operator != BinaryOperator::StrictEquality {
            return None;
        }
        let Expression::StaticMemberExpression(length) = &test.left else {
            return None;
        };
        if length.property.name != "length" {
            return None;
        }
        let Expression::StaticMemberExpression(exception_stack) = &length.object else {
            return None;
        };
        if !matches!(&exception_stack.object, Expression::Identifier(id)
            if identifier_symbol(id, self.scoping) == Some(context))
        {
            return None;
        }
        if !matches!(&test.right, Expression::NumericLiteral(literal) if literal.value == 0.0) {
            return None;
        }
        Some(exception_stack.property.name.to_string())
    }

    fn is_operation_call(
        &self,
        call: &CallExpression<'_>,
        context: SymbolId,
        selector: SymbolId,
    ) -> bool {
        if !self.is_operation_member(&call.callee, selector) {
            return false;
        }
        let expressions = call
            .arguments
            .iter()
            .filter_map(Argument::as_expression)
            .collect::<Vec<_>>();
        expressions.len() == 1 && self.is_identifier(expressions[0], context)
    }

    fn is_operation_member(&self, expression: &Expression<'_>, selector: SymbolId) -> bool {
        let Expression::ComputedMemberExpression(member) = expression else {
            return false;
        };
        matches!(&member.object, Expression::Identifier(id)
            if identifier_symbol(id, self.scoping) == Some(self.operation_table))
            && self.is_identifier(&member.expression, selector)
    }

    fn is_identifier(&self, expression: &Expression<'_>, symbol: SymbolId) -> bool {
        matches!(expression, Expression::Identifier(id)
            if identifier_symbol(id, self.scoping) == Some(symbol))
    }
}

fn alternate_callee_span(callee: &Expression<'_>) -> Option<OperandSpan> {
    match callee {
        Expression::Identifier(identifier) => Some(operand_span(identifier.span)),
        _ => None,
    }
}

impl<'a, 'b> Visit<'a> for RunLoopFinder<'a, 'b> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(proof) = self.check(function) {
            self.found.push(proof);
        }
        walk::walk_function(self, function, flags);
    }
}

/// Index the completion-protocol declarations/writes and the named functions
/// needed by [`prove_run_loop_provenance`]. Node ids (not borrowed AST nodes)
/// are stored so the visitor's reference lifetime never has to be tied to the
/// arena lifetime.
struct RunLoopProvenanceIndex<'a> {
    scoping: &'a Scoping,
    register: SymbolId,
    sentinel: SymbolId,
    declarations: BTreeMap<SymbolId, Vec<NodeId>>,
    functions: Vec<Span>,
    functions_by_symbol: BTreeMap<SymbolId, NodeId>,
    writes: Vec<(SymbolId, Option<Span>)>,
}

impl<'a> RunLoopProvenanceIndex<'a> {
    fn record_write(&mut self, identifier: &IdentifierReference<'_>) {
        let Some(symbol) = identifier_symbol(identifier, self.scoping) else {
            return;
        };
        if symbol == self.register || symbol == self.sentinel {
            self.writes.push((symbol, self.functions.last().copied()));
        }
    }
}

impl<'a> Visit<'a> for RunLoopProvenanceIndex<'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
            self.functions_by_symbol
                .entry(symbol)
                .or_insert_with(|| function.node_id.get());
        }
        self.functions.push(function.span);
        walk::walk_function(self, function, flags);
        self.functions.pop();
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        self.functions.push(function.span);
        walk::walk_arrow_function_expression(self, function);
        self.functions.pop();
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(identifier) = &declarator.id {
            if let Some(symbol) = identifier.symbol_id.get() {
                if symbol == self.register || symbol == self.sentinel {
                    self.declarations
                        .entry(symbol)
                        .or_default()
                        .push(declarator.node_id.get());
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_assignment_target(&mut self, target: &AssignmentTarget<'a>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            self.record_write(identifier);
        }
        walk::walk_assignment_target(self, target);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &update.argument {
            self.record_write(identifier);
        }
        walk::walk_update_expression(self, update);
    }
}

/// Prove the run loop's completion protocol: an immutable sentinel, a
/// completion register initialized to exactly that sentinel, an empty guarded
/// exception stack on the proven constructor, and register/sentinel writes
/// confined to the run loop guard or proven operation-table handlers.
fn prove_run_loop_provenance(
    program: &Program<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    proof: &RunLoopProof,
    context_constructor: SymbolId,
    handler_spans: &[(u32, OperandSpan)],
) -> Result<(), String> {
    let register = proof
        .completion_register_symbol
        .ok_or_else(|| "run-loop proof has no completion-register identity".to_string())?;
    let sentinel = proof
        .completion_sentinel_symbol
        .ok_or_else(|| "run-loop proof has no completion-sentinel identity".to_string())?;
    let mut index = RunLoopProvenanceIndex {
        scoping,
        register,
        sentinel,
        declarations: BTreeMap::new(),
        functions: Vec::new(),
        functions_by_symbol: BTreeMap::new(),
        writes: Vec::new(),
    };
    index.visit_program(program);

    if scoping.symbol_declarations(sentinel).count() != 1 {
        return Err("completion sentinel binding is not declared exactly once".into());
    }
    if scoping.symbol_is_mutated(sentinel) {
        return Err("completion sentinel binding is mutated".into());
    }

    let register_declarations = index
        .declarations
        .get(&register)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if register_declarations.len() != 1 {
        return Err("completion register is not declared exactly once".into());
    }
    let AstKind::VariableDeclarator(register_declarator) = nodes.kind(register_declarations[0])
    else {
        return Err("completion register declaration cannot be resolved".into());
    };
    let Some(Expression::Identifier(initializer)) = register_declarator.init.as_ref() else {
        return Err("completion register initial value is not the exact sentinel binding".into());
    };
    if identifier_symbol(initializer, scoping) != Some(sentinel) {
        return Err("completion register initial value is not the exact sentinel binding".into());
    }

    prove_constructor_empty_stack(
        nodes,
        scoping,
        &index.functions_by_symbol,
        context_constructor,
        &proof.exception_stack_field,
    )?;

    let mut allowed: BTreeSet<(u32, u32)> = handler_spans
        .iter()
        .map(|(_, span)| (span.start, span.end))
        .collect();
    allowed.insert((proof.span.start, proof.span.end));
    for (symbol, function) in &index.writes {
        if *symbol == sentinel {
            return Err("completion sentinel binding is written".into());
        }
        let Some(function) = function else {
            return Err("completion register is written at the top level".into());
        };
        if !allowed.contains(&(function.start, function.end)) {
            return Err(
                "completion register is written outside the run loop guard or a proven operation handler"
                    .into(),
            );
        }
    }
    Ok(())
}

/// The proven constructor must initialize the guarded exception stack to a
/// fresh empty array. The *complete* constructor body is validated first: every
/// statement must be a narrow `this.member = value` assignment, exactly one
/// assigns the guarded field, and that assignment's value is either `[]` or a
/// source-proven zero-argument fresh-stack helper. Other fields may read plain
/// values, call only a proven fresh-stack helper, use the intrinsic unshadowed
/// `Object(receiver)` coercion, or build simple comparisons/conditionals from
/// those. No `this` value escapes, and no assignment/update/sequence, computed
/// mutation, alias, early return or control flow is admitted, so a later
/// `this.G = [1]`, `this.G.push(1)`, `this.G.length = 1`, alias, return of a
/// different object, or unknown/side-effecting call fails closed.
fn prove_constructor_empty_stack(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    functions_by_symbol: &BTreeMap<SymbolId, NodeId>,
    constructor: SymbolId,
    field: &str,
) -> Result<(), String> {
    let node = functions_by_symbol
        .get(&constructor)
        .ok_or_else(|| "proven constructor function was not found".to_string())?;
    let AstKind::Function(function) = nodes.kind(*node) else {
        return Err("proven constructor declaration cannot be resolved".into());
    };
    if function.generator || function.r#async {
        return Err("proven constructor must be a synchronous non-generator function".into());
    }
    let body = function
        .body
        .as_deref()
        .ok_or_else(|| "proven constructor has no body".to_string())?;

    let mut guarded_assignments = 0usize;
    for statement in &body.statements {
        let Statement::ExpressionStatement(expression_statement) = statement else {
            return Err(
                "proven constructor body must contain only this.member = value assignments".into(),
            );
        };
        let Expression::AssignmentExpression(assignment) = &expression_statement.expression else {
            return Err(
                "proven constructor body must contain only this.member = value assignments".into(),
            );
        };
        if assignment.operator != AssignmentOperator::Assign {
            return Err("proven constructor assignments must use plain = assignment".into());
        }
        let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
            return Err("proven constructor field target must be a static this.member".into());
        };
        if !matches!(&member.object, Expression::ThisExpression(_)) {
            return Err("proven constructor field target must be a direct this.member".into());
        }
        if member.property.name == field {
            guarded_assignments += 1;
            if guarded_assignments > 1 {
                return Err(format!(
                    "proven constructor assigns the guarded field {field} more than once"
                ));
            }
            if !prove_fresh_empty_array(&assignment.right, scoping, nodes, functions_by_symbol) {
                return Err(format!(
                    "constructor field {field} is not a proven fresh empty stack"
                ));
            }
        } else if !constructor_value_is_bounded(
            &assignment.right,
            scoping,
            nodes,
            functions_by_symbol,
        ) {
            return Err(format!(
                "proven constructor value for field {} is not a bounded side-effect-free value",
                member.property.name
            ));
        }
    }
    if guarded_assignments != 1 {
        return Err(format!(
            "proven constructor does not initialize the guarded field {field} to an empty stack"
        ));
    }
    Ok(())
}

fn prove_fresh_empty_array(
    expression: &Expression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    functions_by_symbol: &BTreeMap<SymbolId, NodeId>,
) -> bool {
    match expression {
        Expression::ArrayExpression(array) => array.elements.is_empty(),
        Expression::CallExpression(call) => {
            if !call.arguments.is_empty() {
                return false;
            }
            let Expression::Identifier(callee) = &call.callee else {
                return false;
            };
            let Some(symbol) = identifier_symbol(callee, scoping) else {
                return false;
            };
            let Some(node) = functions_by_symbol.get(&symbol) else {
                return false;
            };
            let AstKind::Function(function) = nodes.kind(*node) else {
                return false;
            };
            helper_returns_fresh_empty_array(function, scoping)
        }
        _ => false,
    }
}

/// A constructor field value is a narrow, side-effect-free expression: literal
/// or identifier reads, an intrinsic unshadowed `Object(x)` coercion, a
/// source-proven zero-argument fresh-stack helper call, or a comparison or
/// conditional built only from those. `this`, assignments, updates, sequences,
/// member reads, nested functions and any unknown call fail closed.
fn constructor_value_is_bounded(
    expression: &Expression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    functions_by_symbol: &BTreeMap<SymbolId, NodeId>,
) -> bool {
    match expression {
        Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::Identifier(_) => true,
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
            ) && constructor_value_is_bounded(&unary.argument, scoping, nodes, functions_by_symbol)
        }
        Expression::ArrayExpression(array) => array.elements.iter().all(|element| {
            element.as_expression().is_some_and(|expression| {
                constructor_value_is_bounded(expression, scoping, nodes, functions_by_symbol)
            })
        }),
        Expression::BinaryExpression(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::Equality
                    | BinaryOperator::Inequality
                    | BinaryOperator::StrictEquality
                    | BinaryOperator::StrictInequality
            ) && constructor_value_is_bounded(&binary.left, scoping, nodes, functions_by_symbol)
                && constructor_value_is_bounded(&binary.right, scoping, nodes, functions_by_symbol)
        }
        Expression::ConditionalExpression(conditional) => {
            constructor_value_is_bounded(&conditional.test, scoping, nodes, functions_by_symbol)
                && constructor_value_is_bounded(
                    &conditional.consequent,
                    scoping,
                    nodes,
                    functions_by_symbol,
                )
                && constructor_value_is_bounded(
                    &conditional.alternate,
                    scoping,
                    nodes,
                    functions_by_symbol,
                )
        }
        Expression::CallExpression(call) => {
            constructor_call_is_bounded(call, scoping, nodes, functions_by_symbol)
        }
        _ => false,
    }
}

/// A constructor value call is either the intrinsic unshadowed `Object(x)`
/// coercion with exactly one bounded argument, or a zero-argument call to a
/// source-proven fresh-stack helper. A shadowed `Object`, a method call, an
/// argument-taking or unknown helper all fail closed.
fn constructor_call_is_bounded(
    call: &CallExpression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    functions_by_symbol: &BTreeMap<SymbolId, NodeId>,
) -> bool {
    if call.optional || call.type_arguments.is_some() {
        return false;
    }
    let Expression::Identifier(callee) = &call.callee else {
        return false;
    };
    if callee.name == "Object" && is_unshadowed_global(callee, scoping) {
        let [argument] = call.arguments.as_slice() else {
            return false;
        };
        let Some(argument) = argument.as_expression() else {
            return false;
        };
        return constructor_value_is_bounded(argument, scoping, nodes, functions_by_symbol);
    }
    if !call.arguments.is_empty() {
        return false;
    }
    let Some(symbol) = identifier_symbol(callee, scoping) else {
        return false;
    };
    let Some(node) = functions_by_symbol.get(&symbol) else {
        return false;
    };
    let AstKind::Function(function) = nodes.kind(*node) else {
        return false;
    };
    helper_returns_fresh_empty_array(function, scoping)
}

/// An identifier is the builtin global only when it has a resolved reference
/// that binds to no local symbol; a missing reference id is not proof.
fn is_unshadowed_global(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> bool {
    identifier.reference_id.get().is_some()
        && crate::analysis::identifier_symbol(identifier, scoping).is_none()
}

/// Delegate to the standalone strict utility. The utility proves the complete
/// helper template: exactly one `var stack = [];`, only exact
/// `Object.defineProperty(stack, "<native>", { value: Array.prototype.<native> })`
/// decorations (a subset in any order), and a final `return stack;`. A helper
/// with an empty declaration but a later `stack.push(...)`, alias, escape or
/// arbitrary call therefore fails closed.
fn helper_returns_fresh_empty_array(function: &Function<'_>, scoping: &Scoping) -> bool {
    super::fresh_stack::helper_returns_fresh_empty_array(function, scoping)
}
