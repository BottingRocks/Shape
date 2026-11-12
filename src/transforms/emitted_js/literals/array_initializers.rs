//! Conservative folding of fresh-array construction statements.
//!
//! The emitted tracer commonly creates an array with `void 0` placeholders and
//! immediately fills every index with a separate assignment.  This pass folds
//! only that exact construction shape.  It is intentionally source preserving:
//! RHS text is copied from the input, so escapes, lone surrogates, `-0`, and
//! parentheses are never reconstructed from Oxc's cooked values.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::{operator::AssignmentOperator, operator::UnaryOperator, scope::ScopeFlags};

use crate::analysis;
use crate::diagnostics::StageStatus;
use crate::tracing::symbolic::{EmittedStatementMap, TraceNodeId};

use super::super::{CleanedEmittedPropertyWrites, StatementMapStatus};

/// Why an otherwise array-shaped construction was retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArrayConstructionSkipReason {
    DynamicScope,
    OutsidePrimaryFunction,
    NotFreshArray,
    UnsupportedLiteral,
    NotAdjacentDenseWrites,
    UnsupportedRhs,
    ReceiverObserved,
    ReceiverReassigned,
    Captured,
}

impl ArrayConstructionSkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DynamicScope => "dynamic_scope",
            Self::OutsidePrimaryFunction => "outside_primary_function",
            Self::NotFreshArray => "not_fresh_array",
            Self::UnsupportedLiteral => "unsupported_literal",
            Self::NotAdjacentDenseWrites => "not_adjacent_dense_writes",
            Self::UnsupportedRhs => "unsupported_rhs",
            Self::ReceiverObserved => "receiver_observed",
            Self::ReceiverReassigned => "receiver_reassigned",
            Self::Captured => "captured",
        }
    }
}

impl fmt::Display for ArrayConstructionSkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A retained candidate and the exact original source span that caused its
/// receiver proof to fail. Spans are input coordinates, like the other cleanup
/// reports. This list is deliberately not a complete rejection report:
/// unsupported candidate shapes are omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayConstructionSkip {
    pub span: (u32, u32),
    pub reason: ArrayConstructionSkipReason,
    pub detail: String,
}

/// Result of one source-only array-construction cleanup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayConstructionCleanupOutcome {
    pub code: String,
    pub compacted_arrays: usize,
    pub removed_writes: usize,
    /// Number of declarators in direct function-body variable declarations
    /// visited by the scan. This includes declarations that are not array
    /// candidates and candidates rejected before receiver proof; it is not a
    /// candidate count and does not partition into compacted plus skipped.
    pub considered: usize,
    /// Receiver-proof declines reached after a complete array candidate was
    /// recognized. Shape-unsupported candidates and whole-input unsupported
    /// cases are intentionally not enumerated here.
    pub skipped: Vec<ArrayConstructionSkip>,
    pub status: StageStatus,
    pub statement_map_status: StatementMapStatus,
    pub notes: Vec<String>,
    pub input_bytes: usize,
    pub output_bytes: usize,
    /// `(removed original body statement index, folded declaration index)`.
    /// This is report metadata, not a new provenance identity.
    pub removed_statement_indices: Vec<(usize, usize)>,
}

/// Folded emitted JavaScript and maps remapped after assignment statements are
/// physically removed.  Trace/node IDs are never invented or changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanedEmittedArrayConstructions {
    pub code: String,
    pub statement_map: Vec<EmittedStatementMap>,
    pub node_map: BTreeMap<TraceNodeId, usize>,
    pub object_parameters: BTreeMap<u64, String>,
    pub statement_map_status: StatementMapStatus,
    pub outcome: ArrayConstructionCleanupOutcome,
}

#[derive(Debug, Clone, Copy)]
struct DirectStatement {
    span: Span,
    index: usize,
}

#[derive(Debug, Clone)]
struct Candidate {
    declaration_index: usize,
    binding: SymbolId,
    literal: Span,
    writes: Vec<Write>,
}

#[derive(Debug, Clone)]
struct Write {
    statement: DirectStatement,
    receiver_span: Span,
    rhs: Span,
}

/// Fold source without touching program completion values.  Only the first
/// top-level function declaration's direct body is considered, matching the
/// emitted artifact and its public statement maps.
pub fn simplify_array_constructions(
    source: &str,
) -> Result<ArrayConstructionCleanupOutcome, ArrayConstructionCleanupError> {
    let allocator = Allocator::default();
    let program = Parser::new(&allocator, source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: true,
            ..ParseOptions::default()
        })
        .parse();
    if program.panicked || !program.errors.is_empty() {
        return Err(ArrayConstructionCleanupError::Parse(
            program
                .errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    let program = program.program;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut dynamic = DynamicScan::default();
    dynamic.visit_program(&program);
    let mut base = OutcomeBuilder::new(source);
    if dynamic.dynamic_scope {
        base.note("with/eval makes emitted binding observations uncertain");
        return Ok(base.finish(source.to_owned(), StageStatus::Unsupported));
    }

    let Some(function) = program.body.iter().find_map(|statement| match statement {
        Statement::FunctionDeclaration(function) => Some(function),
        _ => None,
    }) else {
        base.note("program completion is observable; no function body was folded");
        return Ok(base.finish(source.to_owned(), StageStatus::Unknown));
    };
    let Some(body) = &function.body else {
        return Ok(base.finish(source.to_owned(), StageStatus::Unknown));
    };
    let statements = body
        .statements
        .iter()
        .enumerate()
        .map(|(index, statement)| DirectStatement {
            span: statement.span(),
            index,
        })
        .collect::<Vec<_>>();
    let parameters = function
        .params
        .iter_bindings()
        .flat_map(|pattern| pattern.get_binding_identifiers())
        .filter_map(|binding| binding.symbol_id.get())
        .collect::<BTreeSet<_>>();
    let mut edits = Vec::new();
    let mut removed = BTreeSet::new();
    for direct in &statements {
        let Statement::VariableDeclaration(declaration) = &body.statements[direct.index] else {
            continue;
        };
        base.considered += declaration.declarations.len();
        let Some(candidate) = collect_candidate(
            &body,
            declaration,
            *direct,
            &statements,
            &scoping,
            &parameters,
        ) else {
            continue;
        };
        if let Some(failure) = receiver_proof_failure(
            &scoping,
            &nodes,
            candidate.binding,
            candidate
                .writes
                .last()
                .map(|write| write.statement.index)
                .unwrap_or(0),
            &statements,
            &candidate
                .writes
                .iter()
                .map(|write| write.receiver_span.start)
                .collect(),
        ) {
            base.skip(failure.span, failure.reason, failure.detail);
            continue;
        }
        let values = candidate
            .writes
            .iter()
            .map(|write| source[write.rhs.start as usize..write.rhs.end as usize].to_owned())
            .collect::<Vec<_>>();
        let literal = format!("[{}]", values.join(","));
        edits.push(analysis::Edit::replace(candidate.literal, literal));
        for write in &candidate.writes {
            edits.push(analysis::Edit::replace(write.statement.span, ""));
            removed.insert(write.statement.index);
            base.removed_statement_indices
                .push((write.statement.index, candidate.declaration_index));
        }
        base.compacted_arrays += 1;
        base.removed_writes += candidate.writes.len();
    }
    if edits.is_empty() {
        return Ok(base.finish(source.to_owned(), StageStatus::Unknown));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err(ArrayConstructionCleanupError::InvalidOutput(
            analysis::describe_rejected_edits(&applied.rejected),
        ));
    }
    let output_allocator = Allocator::default();
    analysis::parse_program(&output_allocator, &applied.output)
        .map_err(ArrayConstructionCleanupError::InvalidOutput)?;
    base.removed_indices = removed;
    Ok(base.finish(applied.output, StageStatus::Found))
}

/// Fold an already property-cleaned emitted artifact and remap its maps.
pub fn simplify_cleaned_emitted_property_writes(
    cleaned: &CleanedEmittedPropertyWrites,
) -> Result<CleanedEmittedArrayConstructions, ArrayConstructionCleanupError> {
    let outcome = simplify_array_constructions(&cleaned.code)?;
    let removed = outcome
        .removed_statement_indices
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    let removed_indices = removed.keys().copied().collect::<Vec<_>>();
    let map_index = |old: usize| {
        let target_old = removed.get(&old).copied().unwrap_or(old);
        let removed_before = removed_indices.partition_point(|index| *index < target_old);
        target_old.saturating_sub(removed_before)
    };
    let statement_map = cleaned
        .statement_map
        .iter()
        .map(|entry| EmittedStatementMap {
            trace_statement: entry.trace_statement,
            emitted_statement: map_index(entry.emitted_statement),
        })
        .collect();
    let node_map = cleaned
        .node_map
        .iter()
        .map(|(node, statement)| (*node, map_index(*statement)))
        .collect();
    let status = if outcome.compacted_arrays == 0 {
        cleaned.statement_map_status
    } else {
        StatementMapStatus::Regenerated
    };
    Ok(CleanedEmittedArrayConstructions {
        code: outcome.code.clone(),
        statement_map,
        node_map,
        object_parameters: cleaned.object_parameters.clone(),
        statement_map_status: status,
        outcome,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayConstructionCleanupError {
    Parse(String),
    InvalidOutput(String),
}

impl fmt::Display for ArrayConstructionCleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(
                f,
                "array construction cleanup could not parse source: {error}"
            ),
            Self::InvalidOutput(error) => write!(
                f,
                "array construction cleanup produced invalid output: {error}"
            ),
        }
    }
}

impl std::error::Error for ArrayConstructionCleanupError {}

struct OutcomeBuilder<'a> {
    source: &'a str,
    considered: usize,
    compacted_arrays: usize,
    removed_writes: usize,
    skipped: Vec<ArrayConstructionSkip>,
    notes: Vec<String>,
    removed_indices: BTreeSet<usize>,
    removed_statement_indices: Vec<(usize, usize)>,
}

impl<'a> OutcomeBuilder<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            considered: 0,
            compacted_arrays: 0,
            removed_writes: 0,
            skipped: Vec::new(),
            notes: Vec::new(),
            removed_indices: BTreeSet::new(),
            removed_statement_indices: Vec::new(),
        }
    }
    fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }
    fn skip(&mut self, span: Span, reason: ArrayConstructionSkipReason, detail: &str) {
        self.skipped.push(ArrayConstructionSkip {
            span: (span.start, span.end),
            reason,
            detail: detail.into(),
        });
    }
    fn finish(&self, code: String, status: StageStatus) -> ArrayConstructionCleanupOutcome {
        let mut notes = self.notes.clone();
        if self.compacted_arrays > 0 {
            notes.push(format!(
                "compacted {} fresh array construction(s) and removed {} write statement(s)",
                self.compacted_arrays, self.removed_writes
            ));
        }
        ArrayConstructionCleanupOutcome {
            code: code.clone(),
            compacted_arrays: self.compacted_arrays,
            removed_writes: self.removed_writes,
            considered: self.considered,
            skipped: self.skipped.clone(),
            status,
            statement_map_status: if self.removed_indices.is_empty() {
                StatementMapStatus::Preserved
            } else {
                StatementMapStatus::Regenerated
            },
            notes,
            input_bytes: self.source.len(),
            output_bytes: code.len(),
            removed_statement_indices: self.removed_statement_indices.clone(),
        }
    }
}

fn collect_candidate<'a>(
    body: &FunctionBody<'a>,
    declaration: &VariableDeclaration<'a>,
    direct: DirectStatement,
    statements: &[DirectStatement],
    scoping: &Scoping,
    parameters: &BTreeSet<SymbolId>,
) -> Option<Candidate> {
    if declaration.declarations.len() != 1 || declaration.declare {
        return None;
    }
    let declarator = declaration.declarations.first()?;
    let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
        return None;
    };
    let binding_symbol = binding.symbol_id.get()?;
    if parameters.contains(&binding_symbol) {
        return None;
    }
    let literal_expression = unwrap_parens(declarator.init.as_ref()?);
    let Expression::ArrayExpression(array) = literal_expression else {
        return None;
    };
    let preseed_len = array.elements.len();
    if array
        .elements
        .iter()
        .any(|element| !is_void_zero_element(element))
    {
        return None;
    }
    let mut writes = Vec::new();
    for next_index in direct.index + 1..statements.len() {
        let direct_statement = statements[next_index];
        let statement = &body.statements[next_index];
        let Statement::ExpressionStatement(expression_statement) = statement else {
            break;
        };
        let expression = unwrap_parens(&expression_statement.expression);
        let Expression::AssignmentExpression(assignment) = expression else {
            break;
        };
        if assignment.operator != AssignmentOperator::Assign {
            break;
        }
        let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
            break;
        };
        if member.optional {
            break;
        }
        let receiver = unwrap_parens(&member.object);
        let Expression::Identifier(receiver) = receiver else {
            break;
        };
        if analysis::identifier_symbol(receiver, scoping) != Some(binding_symbol) {
            break;
        }
        let index = numeric_array_index(unwrap_parens(&member.expression))?;
        if index != writes.len() {
            return None;
        }
        if !rhs_is_safe(&assignment.right) {
            return None;
        }
        writes.push(Write {
            statement: direct_statement,
            receiver_span: receiver.span,
            rhs: assignment.right.span(),
        });
    }
    if preseed_len == 0 || writes.len() != preseed_len {
        return None;
    }
    Some(Candidate {
        declaration_index: direct.index,
        binding: binding_symbol,
        literal: array.span,
        writes,
    })
}

fn is_void_zero_element(element: &ArrayExpressionElement<'_>) -> bool {
    let ArrayExpressionElement::UnaryExpression(unary) = element else {
        return false;
    };
    if unary.operator != UnaryOperator::Void {
        return false;
    }
    matches!(
        unwrap_parens(&unary.argument),
        Expression::NumericLiteral(literal) if literal.value == 0.0
    )
}

fn numeric_array_index(expression: &Expression<'_>) -> Option<usize> {
    let Expression::NumericLiteral(literal) = expression else {
        return None;
    };
    if !literal.value.is_finite()
        || literal.value < 0.0
        || literal.value.fract() != 0.0
        || literal.value >= u32::MAX as f64
    {
        return None;
    }
    Some(literal.value as usize)
}

fn rhs_is_safe(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::ParenthesizedExpression(parenthesized) => {
            rhs_is_safe(&parenthesized.expression)
        }
        Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BigIntLiteral(_) => true,
        Expression::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::Void => matches!(
                unwrap_parens(&unary.argument),
                Expression::NumericLiteral(literal) if literal.value == 0.0
            ),
            UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => {
                matches!(
                    unwrap_parens(&unary.argument),
                    Expression::NumericLiteral(_)
                )
            }
            _ => false,
        },
        _ => false,
    }
}

struct ReceiverProofFailure {
    span: Span,
    reason: ArrayConstructionSkipReason,
    detail: &'static str,
}

fn receiver_proof_failure(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    symbol: SymbolId,
    last_write: usize,
    statements: &[DirectStatement],
    safe_receivers: &BTreeSet<u32>,
) -> Option<ReceiverProofFailure> {
    let symbol_scope = scoping.symbol_scope_id(symbol);
    for reference in scoping.get_resolved_references(symbol) {
        let span = nodes.get_node(reference.node_id()).kind().span();
        if reference.is_write() || safe_receivers.contains(&span.start) {
            if reference.is_write() && !safe_receivers.contains(&span.start) {
                return Some(ReceiverProofFailure {
                    span,
                    reason: ArrayConstructionSkipReason::ReceiverReassigned,
                    detail:
                        "the array binding is reassigned outside the consumed construction writes",
                });
            }
            continue;
        }
        if reference_is_nested_closure(scoping, reference.scope_id(), symbol_scope) {
            return Some(ReceiverProofFailure {
                span,
                reason: ArrayConstructionSkipReason::Captured,
                detail: "the array binding is captured by a nested function",
            });
        }
        if statements
            .iter()
            .position(|statement| {
                statement.span.start <= span.start && span.end <= statement.span.end
            })
            .is_some_and(|index| index > last_write)
        {
            continue;
        }
        return Some(ReceiverProofFailure {
            span,
            reason: ArrayConstructionSkipReason::ReceiverObserved,
            detail: "the array is read before construction completes",
        });
    }
    None
}

fn reference_is_nested_closure(
    scoping: &Scoping,
    reference_scope: oxc_syntax::scope::ScopeId,
    symbol_scope: oxc_syntax::scope::ScopeId,
) -> bool {
    for scope in scoping.scope_ancestors(reference_scope) {
        if scope == symbol_scope {
            return false;
        }
        if scoping
            .scope_flags(scope)
            .intersects(ScopeFlags::Function | ScopeFlags::Arrow)
        {
            return true;
        }
    }
    true
}

fn unwrap_parens<'a>(mut expression: &'a Expression<'a>) -> &'a Expression<'a> {
    while let Expression::ParenthesizedExpression(parenthesized) = expression {
        expression = &parenthesized.expression;
    }
    expression
}

#[derive(Default)]
struct DynamicScan {
    dynamic_scope: bool,
}

impl<'a> Visit<'a> for DynamicScan {
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
