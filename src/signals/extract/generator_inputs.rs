//! Source-backed semantic contracts for the two small browser collectors that
//! feed the numeric glyph and DOM-census signals.
//!
//! This pass deliberately sits after [`signals`].  It does not discover
//! collectors or helpers and it does not execute JavaScript; it projects the
//! emission-scoped `WireOp` trees supplied by the caller onto the browser
//! operations which establish the meaning of those wires.  Generated names,
//! signal numbers, table lengths, salts and source offsets are evidence in the
//! returned report, never recognition rules.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{AstNodes, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::bindings::{proven_function_node, ParameterSlot, ParameterSlots};
use crate::signals::extract::layout;
use crate::signals::schema::SourceSpan;

pub const GENERATOR_INPUT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeneratorInputError {
    Parse(String),
    TooLarge {
        bytes: usize,
    },
    Ambiguous {
        family: FamilyKind,
        candidates: Vec<SourceSpan>,
    },
    Missing {
        family: FamilyKind,
        reason: String,
    },
    Signal(String),
}

impl fmt::Display for GeneratorInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(message) => write!(f, "generator-input parse failed: {message}"),
            Self::TooLarge { bytes } => {
                write!(f, "generator-input source too large ({bytes} bytes)")
            }
            Self::Ambiguous { family, candidates } => write!(
                f,
                "ambiguous {family:?} producer ({} candidates)",
                candidates.len()
            ),
            Self::Missing { family, reason } => write!(f, "missing {family:?} producer: {reason}"),
            Self::Signal(message) => write!(f, "signal layout unavailable: {message}"),
        }
    }
}

impl std::error::Error for GeneratorInputError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FamilyKind {
    GlyphCanvasPixels,
    DomCensus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    StaticConfig,
    DerivedConfig,
    RuntimeObservation,
    ExternalSeedState,
}

/// An exact source slice. `binding` is a lexical binding witness from this
/// parse, not a portable identity; consumers must use its spans and source
/// hash when joining it to another report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceEvidence {
    pub span: SourceSpan,
    pub source: String,
    pub binding: Option<BindingEvidence>,
    pub literal: Option<TypedLiteral>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypedLiteral {
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingEvidence {
    pub symbol: u32,
    pub label: String,
    pub declaration: SourceSpan,
    pub identifier: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputProvenance {
    pub role: String,
    pub kind: InputKind,
    pub evidence: SourceEvidence,
    pub optional: bool,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableProperty {
    /// The original property spelling is a label only. `role` is assigned by
    /// the consuming call/dataflow position (fillText, coordinates, or pixel
    /// component), not by this spelling.
    pub key: Option<String>,
    pub role: String,
    pub evidence: SourceEvidence,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableEntry {
    pub index: usize,
    pub evidence: SourceEvidence,
    pub properties: Vec<TableProperty>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoopEvidence {
    pub evidence: SourceEvidence,
    pub kind: layout::LoopKind,
    pub control: layout::LoopControl,
    pub reverse: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratorEmission {
    pub signal: u32,
    pub origin: SourceEvidence,
    pub route: layout::ObserverRoute,
    pub variant: layout::EmissionVariant,
    pub wire: layout::WireOp,
    pub prefix: Option<layout::PrefixSeed>,
    pub mask: Option<layout::MaskSeed>,
    pub buffer: Option<SourceEvidence>,
    pub complete: Option<SourceEvidence>,
    pub unresolved: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlyphContract {
    pub table: Option<SourceEvidence>,
    pub entries: Vec<TableEntry>,
    pub render_text: Option<SourceEvidence>,
    pub coordinates: Vec<SourceEvidence>,
    pub pixel_component: Option<SourceEvidence>,
    pub sample_count: Option<SourceEvidence>,
    pub reverse_sample_loop: Option<LoopEvidence>,
    pub draw_configuration: Vec<InputProvenance>,
    pub inputs: Vec<InputProvenance>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DomContract {
    pub selector: Option<SourceEvidence>,
    pub compared_tags: Vec<SourceEvidence>,
    pub id_slice_limit: Option<SourceEvidence>,
    pub statistic: Option<SourceEvidence>,
    pub inputs: Vec<InputProvenance>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratorInputFamily {
    pub kind: FamilyKind,
    pub producer: SourceEvidence,
    pub emissions: Vec<GeneratorEmission>,
    pub inputs: Vec<InputProvenance>,
    pub glyph: Option<GlyphContract>,
    pub dom: Option<DomContract>,
    pub unresolved: Vec<UnresolvedInput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedInput {
    pub fact: String,
    pub reason: String,
    pub span: Option<SourceSpan>,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratorInputReport {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub families: Vec<GeneratorInputFamily>,
    pub unresolved: Vec<UnresolvedInput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceIdentity {
    pub sha256: String,
    pub bytes: usize,
}

impl GeneratorInputReport {
    pub fn family(&self, kind: FamilyKind) -> Option<&GeneratorInputFamily> {
        self.families.iter().find(|family| family.kind == kind)
    }
}

#[derive(Clone, Debug)]
struct BindingInfo {
    symbol: SymbolId,
    name: String,
    declaration: Span,
    identifier: Span,
}

#[derive(Clone, Debug)]
struct TableInfo {
    symbol: SymbolId,
    binding: BindingInfo,
    span: Span,
    entries: Vec<TableEntryRaw>,
}

#[derive(Clone, Debug)]
struct TableEntryRaw {
    span: Span,
    properties: Vec<PropertyRaw>,
}

#[derive(Clone, Debug)]
struct PropertyRaw {
    key: Option<String>,
    value_span: Span,
}

#[derive(Clone, Debug)]
struct CallFact {
    property: Option<String>,
    span: Span,
    args: Vec<ExprFact>,
    callee_root: Option<SymbolId>,
    receiver_root: Option<SymbolId>,
    receiver_source: Option<String>,
    id_slice_receiver: bool,
}

#[derive(Clone, Debug)]
struct ExprFact {
    span: Span,
}

#[derive(Clone, Debug)]
struct MemberFact {
    span: Span,
    root: Option<SymbolId>,
    /// Root of `X` when the object is a row read `X[index]` (traces read a
    /// table row in place instead of through a row local).
    row_root: Option<SymbolId>,
    property: Option<String>,
    computed_root: Option<SymbolId>,
    /// Span of the index expression of a computed member.
    index_span: Option<Span>,
    data_member: bool,
}

#[derive(Clone, Debug)]
struct ComparisonFact {
    tag_member: bool,
    string: Option<ExprFact>,
}

#[derive(Clone, Debug)]
struct CandidateFact {
    span: Span,
    binding: Option<BindingInfo>,
    calls: Vec<CallFact>,
    members: Vec<MemberFact>,
    comparisons: Vec<ComparisonFact>,
    binaries: Vec<Span>,
    assignments: Vec<AssignmentFact>,
    loops: Vec<LoopFact>,
    aliases: BTreeMap<SymbolId, SymbolId>,
    tuple_aliases: BTreeMap<SymbolId, TupleAlias>,
    invalid_symbols: BTreeSet<SymbolId>,
    sync_parent: Option<usize>,
    query_result_symbols: BTreeSet<SymbolId>,
    pixel_data_symbols: BTreeSet<SymbolId>,
    statistics: Vec<StatisticFact>,
    divisions: Vec<DivisionFact>,
}

#[derive(Clone, Debug)]
struct TupleAlias {
    receiver: Option<SymbolId>,
    method: String,
}

#[derive(Clone, Debug)]
struct AssignmentFact {
    span: Span,
    value_span: Span,
    target_root: Option<SymbolId>,
}

#[derive(Clone, Debug)]
struct LoopFact {
    span: Span,
    header: Option<String>,
    kind: layout::LoopKind,
    counter: Option<SymbolId>,
    reverse: bool,
    reads: Vec<LoopRead>,
}

#[derive(Clone, Debug)]
struct LoopRead {
    indexed_span: Span,
    sample_root: Option<SymbolId>,
    call_span: Option<Span>,
    call_root: Option<SymbolId>,
}

#[derive(Clone, Debug)]
struct StatisticFact {
    array_root: Option<SymbolId>,
    reduce_span: Span,
    division_span: Option<Span>,
}

#[derive(Clone, Debug)]
struct DivisionFact {
    span: Span,
    array_root: Option<SymbolId>,
    reduce_span: Span,
}

struct ProgramFacts<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    slots: &'s ParameterSlots<'s, 'a>,
    bindings: BTreeMap<SymbolId, BindingInfo>,
    tables: Vec<TableInfo>,
    candidates: Vec<CandidateFact>,
    stack: Vec<usize>,
    loop_stack: Vec<usize>,
    call_stack: Vec<Span>,
    /// Synchronous `forEach` callbacks, keyed by the callback function's span
    /// start, with every candidate that passes that function. A callback is
    /// merged into its parent only when exactly one parent passes it.
    sync_callback_parents: BTreeMap<u32, BTreeSet<usize>>,
}

impl<'s, 'a> ProgramFacts<'s, 'a> {
    fn source_slice(&self, span: Span) -> String {
        self.source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .to_string()
    }
}

impl<'s, 'a> Visit<'a> for ProgramFacts<'s, 'a> {
    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        if let Some(symbol) = identifier.symbol_id.get() {
            self.bindings.entry(symbol).or_insert_with(|| BindingInfo {
                symbol,
                name: identifier.name.to_string(),
                declaration: identifier.span,
                identifier: identifier.span,
            });
        }
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(identifier) = function.id.as_ref() {
            if let Some(symbol) = identifier.symbol_id.get() {
                self.bindings
                    .entry(symbol)
                    .and_modify(|binding| binding.declaration = function.span)
                    .or_insert_with(|| BindingInfo {
                        symbol,
                        name: identifier.name.to_string(),
                        declaration: function.span,
                        identifier: identifier.span,
                    });
            }
        }
        let binding = function
            .id
            .as_ref()
            .and_then(|identifier| identifier.symbol_id.get())
            .and_then(|symbol| self.bindings.get(&symbol).cloned());
        let binding = binding.or_else(|| {
            function.id.as_ref().and_then(|identifier| {
                identifier.symbol_id.get().map(|symbol| BindingInfo {
                    symbol,
                    name: identifier.name.to_string(),
                    declaration: function.span,
                    identifier: identifier.span,
                })
            })
        });
        let index = self.candidates.len();
        self.candidates.push(CandidateFact {
            span: function.span,
            binding,
            calls: Vec::new(),
            members: Vec::new(),
            comparisons: Vec::new(),
            binaries: Vec::new(),
            assignments: Vec::new(),
            loops: Vec::new(),
            aliases: BTreeMap::new(),
            tuple_aliases: BTreeMap::new(),
            invalid_symbols: BTreeSet::new(),
            sync_parent: None,
            query_result_symbols: BTreeSet::new(),
            pixel_data_symbols: BTreeSet::new(),
            statistics: Vec::new(),
            divisions: Vec::new(),
        });
        self.stack.push(index);
        let loop_depth = self.loop_stack.len();
        walk::walk_function(self, function, flags);
        self.loop_stack.truncate(loop_depth);
        self.stack.pop();
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        let index = self.candidates.len();
        self.candidates.push(CandidateFact {
            span: function.span,
            binding: None,
            calls: Vec::new(),
            members: Vec::new(),
            comparisons: Vec::new(),
            binaries: Vec::new(),
            assignments: Vec::new(),
            loops: Vec::new(),
            aliases: BTreeMap::new(),
            tuple_aliases: BTreeMap::new(),
            invalid_symbols: BTreeSet::new(),
            sync_parent: None,
            query_result_symbols: BTreeSet::new(),
            pixel_data_symbols: BTreeSet::new(),
            statistics: Vec::new(),
            divisions: Vec::new(),
        });
        self.stack.push(index);
        let loop_depth = self.loop_stack.len();
        walk::walk_arrow_function_expression(self, function);
        self.loop_stack.truncate(loop_depth);
        self.stack.pop();
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        let Some(BindingPattern::BindingIdentifier(identifier)) = Some(&declarator.id) else {
            walk::walk_variable_declarator(self, declarator);
            return;
        };
        let Some(symbol) = identifier.symbol_id.get() else {
            walk::walk_variable_declarator(self, declarator);
            return;
        };
        if let Some(init) = declarator.init.as_ref() {
            let declaration = self.bindings.get(&symbol).cloned().unwrap_or(BindingInfo {
                symbol,
                name: identifier.name.to_string(),
                declaration: declarator.span,
                identifier: identifier.span,
            });
            self.record_table(symbol, declaration, init);
        }
        if let Some(candidate) = self
            .stack
            .last()
            .and_then(|index| self.candidates.get_mut(*index))
        {
            if let Some(init) = declarator
                .init
                .as_ref()
                .map(Expression::without_parentheses)
            {
                if let Some(call) = as_call(init) {
                    if member_property(&call.callee).as_deref() == Some("querySelectorAll") {
                        candidate.query_result_symbols.insert(symbol);
                    }
                }
                if is_get_image_data_data(init) {
                    candidate.pixel_data_symbols.insert(symbol);
                }
                if let Some(tuple) = tuple_alias(init, self.scoping) {
                    candidate.tuple_aliases.insert(symbol, tuple);
                }
                if let Expression::ComputedMemberExpression(member) = init {
                    if let Some(root) = expression_root(&member.object, self.scoping) {
                        candidate.aliases.insert(symbol, root);
                    }
                } else if let Some(root) = expression_root(init, self.scoping) {
                    candidate.aliases.insert(symbol, root);
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Some(index) = self.stack.last().copied() {
            let (property, argument_start, receiver_root, receiver_source) =
                resolved_call(call, &self.candidates[index], self.scoping);
            let id_slice_receiver = member_property(&call.callee).as_deref() == Some("slice")
                && member_object(&call.callee).is_some_and(is_first_child_id_expression);
            let expressions = call
                .arguments
                .iter()
                .skip(argument_start)
                .filter_map(|argument| argument.as_expression())
                .collect::<Vec<_>>();
            let fact = CallFact {
                property,
                span: call.span,
                args: expressions
                    .iter()
                    .map(|expression| ExprFact {
                        span: expression.span(),
                    })
                    .collect(),
                callee_root: expression_root(&call.callee, self.scoping),
                receiver_root,
                receiver_source: receiver_source.map(|span| self.source_slice(span)),
                id_slice_receiver,
            };
            if fact.property.as_deref() == Some("forEach")
                && fact
                    .receiver_root
                    .is_some_and(|root| self.candidates[index].query_result_symbols.contains(&root))
            {
                // The callback is the first argument after the receiver: an
                // inline function or arrow, or a binding proven to hold one
                // function. An unproven callback links nothing.
                let callback = call
                    .arguments
                    .iter()
                    .nth(argument_start)
                    .and_then(|argument| argument.as_expression());
                if let Ok((_, span)) = proven_function_node(self.scoping, self.nodes, callback, 0) {
                    self.sync_callback_parents
                        .entry(span.start)
                        .or_default()
                        .insert(index);
                }
            }
            if fact.property.as_deref() == Some("reduce") {
                if let Some(statistic) = statistic_fact(call, self.scoping, self.nodes, self.slots)
                {
                    self.candidates[index].statistics.push(statistic);
                }
            }
            let read_start = self.candidates[index]
                .loops
                .last()
                .map_or(0, |loop_| loop_.reads.len());
            self.candidates[index].calls.push(fact);
            self.call_stack.push(call.span);
            walk::walk_call_expression(self, call);
            self.call_stack.pop();
            let call_root = self.candidates[index]
                .calls
                .last()
                .and_then(|call| call.callee_root);
            if let Some(loop_) = self.candidates[index].loops.last_mut() {
                for read in loop_.reads.iter_mut().skip(read_start) {
                    read.call_span = Some(call.span);
                    read.call_root = call_root;
                }
            }
            return;
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.record_member(
            member.span,
            &member.object,
            Some(member.property.name.to_string()),
            None,
            None,
            None,
        );
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        let computed_root = expression_root(&member.expression, self.scoping);
        // `object["name"]` names the member just as `object.name` does.
        let property = match member.expression.without_parentheses() {
            Expression::StringLiteral(name) => Some(name.value.to_string()),
            _ => None,
        };
        let offset_counter = offset_counter_symbol(&member.expression, self.scoping);
        self.record_member(
            member.span,
            &member.object,
            property,
            computed_root,
            Some(member.expression.span()),
            offset_counter,
        );
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_binary_expression(&mut self, binary: &BinaryExpression<'a>) {
        if let Some(index) = self.stack.last().copied() {
            self.candidates[index].binaries.push(binary.span);
            if binary.operator == BinaryOperator::Division {
                if let Expression::CallExpression(call) = binary.left.without_parentheses() {
                    if member_property(&call.callee).as_deref() == Some("reduce") {
                        let array_root = member_object(&call.callee)
                            .and_then(|object| expression_root(object, self.scoping));
                        let denominator_root = (member_property(&binary.right).as_deref()
                            == Some("length"))
                        .then(|| member_object(&binary.right))
                        .flatten()
                        .and_then(|object| expression_root(object, self.scoping));
                        if array_root.is_some() && array_root == denominator_root {
                            self.candidates[index].divisions.push(DivisionFact {
                                span: binary.span,
                                array_root,
                                reduce_span: call.span,
                            });
                        }
                    }
                }
            }
            if binary.operator == BinaryOperator::StrictEquality
                || binary.operator == BinaryOperator::Equality
                || binary.operator == BinaryOperator::StrictInequality
                || binary.operator == BinaryOperator::Inequality
            {
                let left_tag = member_property(&binary.left) == Some("tagName".to_string());
                let right_tag = member_property(&binary.right) == Some("tagName".to_string());
                let string = if left_tag {
                    expression_fact(&binary.right)
                } else if right_tag {
                    expression_fact(&binary.left)
                } else {
                    None
                };
                self.candidates[index].comparisons.push(ComparisonFact {
                    tag_member: left_tag || right_tag,
                    string,
                });
            }
        }
        walk::walk_binary_expression(self, binary);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
            if assignment.operator == AssignmentOperator::Assign {
                if let Some(symbol) = identifier_symbol(identifier, self.scoping) {
                    if let Some(binding) = self.bindings.get(&symbol).cloned() {
                        self.record_table(symbol, binding, &assignment.right);
                    }
                }
            }
        }
        if let Some(index) = self.stack.last().copied() {
            if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                if let Some(symbol) = identifier_symbol(identifier, self.scoping) {
                    self.candidates[index].invalid_symbols.insert(symbol);
                    self.candidates[index].aliases.remove(&symbol);
                    self.candidates[index].tuple_aliases.remove(&symbol);
                }
            }
            let property = match &assignment.left {
                AssignmentTarget::StaticMemberExpression(member) => {
                    Some(member.property.name.to_string())
                }
                AssignmentTarget::ComputedMemberExpression(member) => {
                    match member.expression.without_parentheses() {
                        Expression::StringLiteral(value) => Some(value.value.to_string()),
                        _ => None,
                    }
                }
                _ => None,
            };
            if matches!(
                property.as_deref(),
                Some("font" | "fillStyle" | "height" | "width")
            ) {
                self.candidates[index].assignments.push(AssignmentFact {
                    span: assignment.span,
                    value_span: assignment.right.span(),
                    target_root: assignment_target_root(&assignment.left, self.scoping),
                });
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        let loop_index = self.new_for_loop(statement);
        self.loop_stack.push(loop_index);
        walk::walk_for_statement(self, statement);
        self.loop_stack.pop();
    }

    fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
        let test_counter = while_counter_symbol(statement, self.scoping);
        let update = while_update(statement, self.scoping);
        let counter = test_counter.or_else(|| update.map(|(counter, _)| counter));
        let reverse = update
            .map(|(_, direction)| direction == LoopDirection::Reverse)
            .unwrap_or(false)
            && comparison_direction(&statement.test, counter, self.scoping)
                == Some(LoopDirection::Reverse);
        let loop_index = self.new_loop(
            statement.span,
            Some(loop_header(
                self.source,
                statement.span.start,
                &statement.body,
            )),
            layout::LoopKind::While,
            counter,
            reverse,
        );
        self.loop_stack.push(loop_index);
        walk::walk_while_statement(self, statement);
        self.loop_stack.pop();
    }

    fn visit_do_while_statement(&mut self, statement: &DoWhileStatement<'a>) {
        let loop_index = self.new_loop(
            statement.span,
            None,
            layout::LoopKind::DoWhile,
            None,
            false,
        );
        self.loop_stack.push(loop_index);
        walk::walk_do_while_statement(self, statement);
        self.loop_stack.pop();
    }
}

impl<'s, 'a> ProgramFacts<'s, 'a> {
    /// Records `binding = [{...}, ...]` as a table of object rows. Traces
    /// declare the binding up front and assign the rows later, so both a
    /// declarator and a plain assignment hold a table.
    fn record_table(&mut self, symbol: SymbolId, binding: BindingInfo, value: &Expression<'a>) {
        let Expression::ArrayExpression(array) = value.without_parentheses() else {
            return;
        };
        let entries = array
            .elements
            .iter()
            .filter_map(|element| element.as_expression())
            .filter_map(|expression| match expression.without_parentheses() {
                Expression::ObjectExpression(object) => Some(TableEntryRaw {
                    span: expression.span(),
                    properties: object
                        .properties
                        .iter()
                        .filter_map(|property| match property {
                            ObjectPropertyKind::ObjectProperty(property) => Some(PropertyRaw {
                                key: property_key(&property.key),
                                value_span: property.value.span(),
                            }),
                            _ => None,
                        })
                        .collect(),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !entries.is_empty() {
            self.tables.push(TableInfo {
                symbol,
                binding,
                span: array.span,
                entries,
            });
        }
    }

    /// Links each synchronous callback to the one candidate that passes it.
    /// A function passed by more than one candidate is left unlinked.
    fn resolve_sync_parents(&mut self) {
        for (index, candidate) in self.candidates.iter_mut().enumerate() {
            let Some(parents) = self.sync_callback_parents.get(&candidate.span.start) else {
                continue;
            };
            if let [parent] = parents.iter().copied().collect::<Vec<_>>().as_slice() {
                if *parent != index {
                    candidate.sync_parent = Some(*parent);
                }
            }
        }
    }

    fn resolve_statistic_divisions(&mut self) {
        for candidate in &mut self.candidates {
            for statistic in &mut candidate.statistics {
                statistic.division_span = candidate
                    .divisions
                    .iter()
                    .find(|division| {
                        division.array_root == statistic.array_root
                            && division.reduce_span == statistic.reduce_span
                    })
                    .map(|division| division.span);
            }
        }
    }

    fn record_member(
        &mut self,
        span: Span,
        object: &Expression<'a>,
        property: Option<String>,
        computed_root: Option<SymbolId>,
        index_span: Option<Span>,
        offset_counter: Option<SymbolId>,
    ) {
        let Some(index) = self.stack.last().copied() else {
            return;
        };
        let object_root = expression_root(object, self.scoping);
        let data_member = is_get_image_data_data(object)
            || self
                .stack
                .last()
                .and_then(|index| self.candidates.get(*index))
                .is_some_and(|candidate| {
                    object_root.is_some_and(|root| candidate.pixel_data_symbols.contains(&root))
                });
        let row_root = match object.without_parentheses() {
            Expression::ComputedMemberExpression(row)
                if !matches!(
                    row.expression.without_parentheses(),
                    Expression::StringLiteral(_)
                ) =>
            {
                expression_root(&row.object, self.scoping)
            }
            _ => None,
        };
        self.candidates[index].members.push(MemberFact {
            span,
            root: object_root,
            row_root,
            property,
            computed_root,
            index_span,
            data_member,
        });
        if let Some(loop_index) = self.loop_stack.last().copied() {
            if let Some(loop_) = self.candidates[index].loops.get_mut(loop_index) {
                // `sample[counter]` or `sample[counter - k]`. A read counts as
                // a sample only once the emission wire repeats its number
                // write under this loop (`wire_has_proven_repeat`).
                let offset_read = loop_.counter.is_some() && loop_.counter == offset_counter;
                if loop_.counter == computed_root || offset_read {
                    loop_.reads.push(LoopRead {
                        indexed_span: span,
                        sample_root: expression_root(object, self.scoping),
                        call_span: None,
                        call_root: None,
                    });
                }
            }
        }
    }

    fn new_loop(
        &mut self,
        span: Span,
        header: Option<String>,
        kind: layout::LoopKind,
        counter: Option<SymbolId>,
        reverse: bool,
    ) -> usize {
        let index = self.stack.last().copied().unwrap_or(0);
        let loop_index = self.candidates[index].loops.len();
        self.candidates[index].loops.push(LoopFact {
            span,
            header,
            kind,
            counter,
            reverse,
            reads: Vec::new(),
        });
        loop_index
    }

    fn new_for_loop(&mut self, statement: &ForStatement<'a>) -> usize {
        let counter = statement.init.as_ref().and_then(|init| match init {
            ForStatementInit::VariableDeclaration(declaration) => declaration
                .declarations
                .first()
                .and_then(|declaration| match &declaration.id {
                    BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
                    _ => None,
                }),
            _ => None,
        });
        let update_counter = statement
            .update
            .as_ref()
            .and_then(|update| update_target_symbol(update, self.scoping));
        // A `for (;;)` with no test or update can spell the same reverse loop
        // in its body: `--counter; if (!(counter >= 0)) break;`. The trailing
        // `counter = Number(counter)` coercion is a write to the same symbol,
        // not a second loop update.
        let body_reverse = (statement.test.is_none() && statement.update.is_none())
            .then(|| for_body_reverse_counter(statement, self.scoping))
            .flatten();
        let counter = body_reverse.or(counter).or(update_counter);
        let reverse = body_reverse.is_some()
            || statement
                .update
                .as_ref()
                .and_then(update_direction)
                .is_some_and(|direction| direction == LoopDirection::Reverse)
                && statement
                    .test
                    .as_ref()
                    .and_then(|test| comparison_direction(test, counter, self.scoping))
                    .is_some_and(|direction| direction == LoopDirection::Reverse);
        let header = self
            .source
            .get(statement.span.start as usize..statement.span.end as usize)
            .map(|source| {
                let mut parens = 0usize;
                let mut end = source.len();
                for (index, character) in source.char_indices() {
                    match character {
                        '(' => parens += 1,
                        ')' => parens = parens.saturating_sub(1),
                        '{' if parens == 0 => {
                            end = index;
                            break;
                        }
                        _ => {}
                    }
                }
                source[..end].trim().to_string()
            });
        self.new_loop(
            statement.span,
            header,
            layout::LoopKind::For,
            counter,
            reverse,
        )
    }
}

fn property_key(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(value) => Some(value.value.to_string()),
        PropertyKey::NumericLiteral(value) => Some(value.value.to_string()),
        _ => None,
    }
}

fn member_property(expression: &Expression<'_>) -> Option<String> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(member.property.name.to_string()),
        Expression::ComputedMemberExpression(member) => {
            match member.expression.without_parentheses() {
                Expression::StringLiteral(value) => Some(value.value.to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn expression_root(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => identifier
            .reference_id
            .get()
            .and_then(|reference| scoping.get_reference(reference).symbol_id()),
        Expression::StaticMemberExpression(member) => expression_root(&member.object, scoping),
        Expression::ComputedMemberExpression(member) => expression_root(&member.object, scoping),
        Expression::PrivateFieldExpression(member) => expression_root(&member.object, scoping),
        _ => None,
    }
}

fn identifier_symbol(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> Option<SymbolId> {
    identifier
        .reference_id
        .get()
        .and_then(|reference| scoping.get_reference(reference).symbol_id())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoopDirection {
    Forward,
    Reverse,
}

fn numeric_index(expression: &Expression<'_>) -> Option<i64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal) => Some(literal.value as i64),
        _ => None,
    }
}

fn tuple_alias(expression: &Expression<'_>, scoping: &Scoping) -> Option<TupleAlias> {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return None;
    };
    if array.elements.len() != 2 {
        return None;
    }
    let receiver = array.elements.first()?.as_expression()?;
    let method = array.elements.get(1)?.as_expression()?;
    let method = match method.without_parentheses() {
        Expression::StaticMemberExpression(member) => member.property.name.to_string(),
        Expression::ComputedMemberExpression(member) => {
            match member.expression.without_parentheses() {
                Expression::StringLiteral(value) => value.value.to_string(),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(TupleAlias {
        receiver: expression_root(receiver, scoping),
        method,
    })
}

/// `array.reduce(callback, ...)` whose callback returns the running sum of
/// squares `acc + value * value`. The callback is an inline function or a
/// binding proven to hold one function. Its accumulator and value are its own
/// parameter slots 0 and 1, read either as never-written named formals or as
/// `arguments[0]` / `arguments[1]`; the shared [`ParameterSlots`] resolver
/// decides both representations alike.
fn statistic_fact<'a>(
    call: &CallExpression<'a>,
    scoping: &Scoping,
    nodes: &AstNodes<'a>,
    slots: &ParameterSlots<'_, 'a>,
) -> Option<StatisticFact> {
    if member_property(&call.callee).as_deref() != Some("reduce") {
        return None;
    }
    let array_root =
        member_object(&call.callee).and_then(|object| expression_root(object, scoping));
    let callback = call
        .arguments
        .first()
        .and_then(|argument| argument.as_expression());
    let (owner, _) = proven_function_node(scoping, nodes, callback, 0).ok()?;
    let body = match nodes.kind(owner) {
        AstKind::Function(function) => function.body.as_deref()?,
        AstKind::ArrowFunctionExpression(arrow) => &arrow.body,
        _ => return None,
    };
    let mut proof = ReductionShape {
        accumulator: ParameterSlot { owner, index: 0 },
        value: ParameterSlot { owner, index: 1 },
        proven: false,
        slots,
    };
    proof.visit_function_body(body);
    proof.proven.then_some(StatisticFact {
        array_root,
        reduce_span: call.span,
        division_span: None,
    })
}

struct ReductionShape<'r, 's, 'a> {
    accumulator: ParameterSlot,
    value: ParameterSlot,
    proven: bool,
    slots: &'r ParameterSlots<'s, 'a>,
}

impl ReductionShape<'_, '_, '_> {
    fn is_slot(&self, expression: &Expression<'_>, slot: ParameterSlot) -> bool {
        self.slots.incoming_slot(expression) == Some(slot)
    }

    fn visit_function_body(&mut self, body: &FunctionBody<'_>) {
        struct Finder<'b, 'r, 's, 'a> {
            proof: &'b mut ReductionShape<'r, 's, 'a>,
        }
        impl<'x> Visit<'x> for Finder<'_, '_, '_, '_> {
            fn visit_binary_expression(&mut self, binary: &BinaryExpression<'x>) {
                if binary.operator == BinaryOperator::Addition
                    && self.proof.is_slot(&binary.left, self.proof.accumulator)
                {
                    if let Expression::BinaryExpression(product) =
                        binary.right.without_parentheses()
                    {
                        if product.operator == BinaryOperator::Multiplication
                            && self.proof.is_slot(&product.left, self.proof.value)
                            && self.proof.is_slot(&product.right, self.proof.value)
                        {
                            self.proof.proven = true;
                        }
                    }
                }
                walk::walk_binary_expression(self, binary);
            }
        }
        Finder { proof: self }.visit_function_body(body);
    }
}

fn member_object<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

fn as_call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expression.without_parentheses() {
        Expression::CallExpression(call) => Some(call),
        _ => None,
    }
}

fn update_target_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression.without_parentheses() {
        Expression::UpdateExpression(update) => {
            simple_assignment_target_symbol(&update.argument, scoping)
        }
        Expression::AssignmentExpression(assignment) => {
            assignment_target_root(&assignment.left, scoping)
        }
        _ => None,
    }
}

fn update_direction(expression: &Expression<'_>) -> Option<LoopDirection> {
    match expression.without_parentheses() {
        Expression::UpdateExpression(update) => match update.operator {
            oxc_syntax::operator::UpdateOperator::Increment => Some(LoopDirection::Forward),
            oxc_syntax::operator::UpdateOperator::Decrement => Some(LoopDirection::Reverse),
        },
        Expression::AssignmentExpression(assignment) => match assignment.operator {
            oxc_syntax::operator::AssignmentOperator::Addition => Some(LoopDirection::Forward),
            oxc_syntax::operator::AssignmentOperator::Subtraction => Some(LoopDirection::Reverse),
            _ => None,
        },
        _ => None,
    }
}

/// A `for (;;)` whose block opens with `--counter;` and then
/// `if (!(counter >= 0)) break;` (or `if (counter < 0) break;`) is the same
/// reverse loop spelled in the body instead of the header. Both statements
/// must agree on the counter symbol and the decrement must come first, so a
/// guard after the write or on another symbol is not a reverse loop.
fn for_body_reverse_counter(statement: &ForStatement<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let Statement::BlockStatement(block) = &statement.body else {
        return None;
    };
    let mut statements = block.body.iter();
    let Statement::ExpressionStatement(decrement) = statements.next()? else {
        return None;
    };
    let counter = update_target_symbol(&decrement.expression, scoping)?;
    if update_direction(&decrement.expression) != Some(LoopDirection::Reverse) {
        return None;
    }
    let Statement::IfStatement(guard) = statements.next()? else {
        return None;
    };
    (guard.alternate.is_none()
        && matches!(&guard.consequent, Statement::BreakStatement(_))
        && guard_breaks_below_zero(&guard.test, counter, scoping))
    .then_some(counter)
}

fn guard_breaks_below_zero(test: &Expression<'_>, counter: SymbolId, scoping: &Scoping) -> bool {
    match test.without_parentheses() {
        // `if (!(counter >= 0)) break;` negates the reverse loop test.
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
            comparison_direction(&unary.argument, Some(counter), scoping)
                == Some(LoopDirection::Reverse)
        }
        // `if (counter < 0) break;` exits below the same zero bound.
        Expression::BinaryExpression(binary) => {
            expression_root(&binary.left, scoping) == Some(counter)
                && binary.operator == BinaryOperator::LessThan
                && numeric_index(&binary.right) == Some(0)
        }
        _ => false,
    }
}

fn simple_assignment_target_symbol(
    target: &SimpleAssignmentTarget<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            identifier_symbol(identifier, scoping)
        }
        _ => None,
    }
}

fn assignment_target_root(target: &AssignmentTarget<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            identifier_symbol(identifier, scoping)
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            expression_root(&member.object, scoping)
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            expression_root(&member.object, scoping)
        }
        _ => None,
    }
}

fn comparison_direction(
    expression: &Expression<'_>,
    counter: Option<SymbolId>,
    scoping: &Scoping,
) -> Option<LoopDirection> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    let left_offset = counter.is_some() && offset_counter_symbol(&binary.left, scoping) == counter;
    if expression_root(&binary.left, scoping) != counter && !left_offset {
        return None;
    }
    match binary.operator {
        BinaryOperator::GreaterThan | BinaryOperator::GreaterEqualThan => {
            Some(LoopDirection::Reverse)
        }
        BinaryOperator::LessThan | BinaryOperator::LessEqualThan => Some(LoopDirection::Forward),
        _ => None,
    }
}

/// The counter of `counter - k` with a numeric literal `k`: a loop that tests
/// `counter - 1 >= 0` reads its samples as `sample[counter - 1]`.
fn offset_counter_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    if binary.operator != BinaryOperator::Subtraction
        || !matches!(
            binary.right.without_parentheses(),
            Expression::NumericLiteral(_)
        )
    {
        return None;
    }
    match binary.left.without_parentheses() {
        Expression::Identifier(identifier) => identifier_symbol(identifier, scoping),
        _ => None,
    }
}

fn loop_header(source: &str, start: u32, body: &Statement<'_>) -> String {
    source
        .get(start as usize..body.span().start as usize)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn while_counter_symbol(statement: &WhileStatement<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let Expression::BinaryExpression(binary) = statement.test.without_parentheses() else {
        return None;
    };
    let left = expression_root(&binary.left, scoping);
    let right = expression_root(&binary.right, scoping);
    match (left, right) {
        (Some(counter), None) => Some(counter),
        (None, Some(counter)) => Some(counter),
        _ => None,
    }
}

fn while_update(
    statement: &WhileStatement<'_>,
    scoping: &Scoping,
) -> Option<(SymbolId, LoopDirection)> {
    struct UpdateFinder<'s> {
        scoping: &'s Scoping,
        updates: Vec<(SymbolId, LoopDirection)>,
    }
    impl<'a> Visit<'a> for UpdateFinder<'_> {
        fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
            if let Some(counter) = simple_assignment_target_symbol(&update.argument, self.scoping) {
                let direction = match update.operator {
                    oxc_syntax::operator::UpdateOperator::Increment => LoopDirection::Forward,
                    oxc_syntax::operator::UpdateOperator::Decrement => LoopDirection::Reverse,
                };
                self.updates.push((counter, direction));
            }
            walk::walk_update_expression(self, update);
        }

        fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
            if let Some(counter) = assignment_target_root(&assignment.left, self.scoping) {
                let direction = match assignment.operator {
                    oxc_syntax::operator::AssignmentOperator::Addition => {
                        Some(LoopDirection::Forward)
                    }
                    oxc_syntax::operator::AssignmentOperator::Subtraction => {
                        Some(LoopDirection::Reverse)
                    }
                    _ => None,
                };
                if let Some(direction) = direction {
                    self.updates.push((counter, direction));
                }
            }
            walk::walk_assignment_expression(self, assignment);
        }
    }
    let mut finder = UpdateFinder {
        scoping,
        updates: Vec::new(),
    };
    finder.visit_statement(&statement.body);
    (finder.updates.len() == 1).then(|| finder.updates[0])
}

fn is_get_image_data_data(expression: &Expression<'_>) -> bool {
    if member_property(expression).as_deref() != Some("data") {
        return false;
    }
    let Some(Expression::CallExpression(call)) =
        member_object(expression).map(Expression::without_parentheses)
    else {
        return false;
    };
    member_property(&call.callee).as_deref() == Some("getImageData")
}

fn is_first_child_id_expression(expression: &Expression<'_>) -> bool {
    if member_property(expression).as_deref() != Some("id") {
        return false;
    }
    let Some(Expression::ComputedMemberExpression(first_child)) =
        member_object(expression).map(Expression::without_parentheses)
    else {
        return false;
    };
    numeric_index(&first_child.expression) == Some(0)
        && member_property(&first_child.object).as_deref() == Some("children")
}

fn tuple_lookup(
    expression: &Expression<'_>,
    candidate: &CandidateFact,
    scoping: &Scoping,
) -> Option<TupleAlias> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    if numeric_index(&member.expression) != Some(1) {
        return None;
    }
    let tuple = expression_root(&member.object, scoping)?;
    if candidate.invalid_symbols.contains(&tuple) {
        return None;
    }
    candidate.tuple_aliases.get(&tuple).cloned()
}

fn resolved_call(
    call: &CallExpression<'_>,
    candidate: &CandidateFact,
    scoping: &Scoping,
) -> (Option<String>, usize, Option<SymbolId>, Option<Span>) {
    if member_property(&call.callee).as_deref() == Some("call") {
        if let Some(inner) = member_object(&call.callee) {
            if let Some(tuple) = tuple_lookup(inner, candidate, scoping) {
                let receiver = call
                    .arguments
                    .first()
                    .and_then(|argument| argument.as_expression());
                let receiver_root = receiver
                    .and_then(|expression| tuple_receiver_root(expression, candidate, scoping));
                if tuple.receiver != receiver_root {
                    return (None, 0, None, None);
                }
                return (
                    Some(tuple.method),
                    1,
                    receiver_root,
                    receiver.map(|expression| expression.span()),
                );
            }
            // `F.call.call(F, receiver, ...)` is `F.call(receiver, ...)`:
            // the explicit `this` of the outer `call` must be the same static
            // path `F` that the inner `call` was read from.
            if member_property(inner).as_deref() == Some("call") {
                if let (Some(function), Some(explicit)) = (
                    member_object(inner),
                    call.arguments
                        .first()
                        .and_then(|argument| argument.as_expression()),
                ) {
                    if same_static_path(function, explicit, scoping) {
                        let receiver = call
                            .arguments
                            .get(1)
                            .and_then(|argument| argument.as_expression());
                        return (
                            member_property(function),
                            2,
                            receiver.and_then(|expression| expression_root(expression, scoping)),
                            receiver.map(|expression| expression.span()),
                        );
                    }
                }
            }
            if let Some(method) = member_property(inner) {
                let receiver = call
                    .arguments
                    .first()
                    .and_then(|argument| argument.as_expression());
                return (
                    Some(method),
                    1,
                    receiver.and_then(|expression| expression_root(expression, scoping)),
                    receiver.map(|expression| expression.span()),
                );
            }
        }
    }
    let property = member_property(&call.callee);
    let receiver = member_object(&call.callee);
    (
        property,
        0,
        receiver.and_then(|expression| expression_root(expression, scoping)),
        receiver.map(|expression| expression.span()),
    )
}

/// True when two expressions read the same static member path: the same
/// root (one binding, or the same unbound global name) followed by the same
/// static property names. Any other expression is not compared.
fn same_static_path(left: &Expression<'_>, right: &Expression<'_>, scoping: &Scoping) -> bool {
    match (left.without_parentheses(), right.without_parentheses()) {
        (Expression::Identifier(left), Expression::Identifier(right)) => {
            match (
                identifier_symbol(left, scoping),
                identifier_symbol(right, scoping),
            ) {
                (Some(left), Some(right)) => left == right,
                (None, None) => left.name == right.name,
                _ => false,
            }
        }
        (
            left
            @ (Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)),
            right @ (Expression::StaticMemberExpression(_)
            | Expression::ComputedMemberExpression(_)),
        ) => {
            let static_name = |expression: &Expression<'_>| match expression {
                Expression::StaticMemberExpression(member) if !member.optional => {
                    Some(member.property.name.to_string())
                }
                Expression::ComputedMemberExpression(member) if !member.optional => {
                    match member.expression.without_parentheses() {
                        Expression::StringLiteral(name) => Some(name.value.to_string()),
                        _ => None,
                    }
                }
                _ => None,
            };
            match (static_name(left), static_name(right)) {
                (Some(left_name), Some(right_name)) if left_name == right_name => {
                    match (member_object(left), member_object(right)) {
                        (Some(left), Some(right)) => same_static_path(left, right, scoping),
                        _ => false,
                    }
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn tuple_receiver_root(
    expression: &Expression<'_>,
    candidate: &CandidateFact,
    scoping: &Scoping,
) -> Option<SymbolId> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return expression_root(expression, scoping);
    };
    if numeric_index(&member.expression) != Some(0) {
        return expression_root(expression, scoping);
    }
    let tuple = expression_root(&member.object, scoping)?;
    if candidate.invalid_symbols.contains(&tuple) {
        return None;
    }
    candidate.tuple_aliases.get(&tuple)?.receiver
}

fn expression_fact(expression: &Expression<'_>) -> Option<ExprFact> {
    Some(ExprFact {
        span: expression.span(),
    })
}

fn evidence(source: &str, span: Span, binding: Option<&BindingInfo>) -> SourceEvidence {
    let source_text = source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default()
        .to_string();
    SourceEvidence {
        span: span.into(),
        literal: typed_literal(&source_text),
        source: source_text,
        binding: binding.map(|binding| BindingEvidence {
            symbol: binding.symbol.index() as u32,
            label: binding.name.clone(),
            declaration: binding.declaration.into(),
            identifier: binding.identifier.into(),
        }),
    }
}

fn typed_literal(source: &str) -> Option<TypedLiteral> {
    let allocator = Allocator::default();
    let expression = Parser::new(&allocator, source, SourceType::unambiguous())
        .parse_expression()
        .ok()?;
    match expression.without_parentheses() {
        Expression::NullLiteral(_) => Some(TypedLiteral::Null),
        Expression::BooleanLiteral(literal) => Some(TypedLiteral::Boolean(literal.value)),
        Expression::NumericLiteral(literal) => Some(TypedLiteral::Number(literal.value)),
        Expression::UnaryExpression(unary)
            if matches!(
                unary.operator,
                oxc_syntax::operator::UnaryOperator::UnaryNegation
                    | oxc_syntax::operator::UnaryOperator::UnaryPlus
            ) =>
        {
            let Expression::NumericLiteral(literal) = unary.argument.without_parentheses() else {
                return None;
            };
            let value = match unary.operator {
                oxc_syntax::operator::UnaryOperator::UnaryNegation => -literal.value,
                oxc_syntax::operator::UnaryOperator::UnaryPlus => literal.value,
                _ => unreachable!("operator was checked above"),
            };
            Some(TypedLiteral::Number(value))
        }
        Expression::StringLiteral(literal) => Some(TypedLiteral::String(literal.value.to_string())),
        _ => None,
    }
}

fn hash_source(source: &str) -> SourceIdentity {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    SourceIdentity {
        sha256: hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        bytes: source.len(),
    }
}

/// The generator inputs of a request whose signal layout report is already
/// extracted (`layout` must be `extract_signal_layouts(request)`).
pub(crate) fn extract_generator_inputs_with_layout(
    request: &layout::SignalLayoutRequest<'_>,
    layout: &layout::SignalLayoutReport,
) -> Result<GeneratorInputReport, GeneratorInputError> {
    if request.source.is_empty() {
        return Err(GeneratorInputError::Parse("empty source".into()));
    }
    if request.source.len() > request.limits.max_source_bytes {
        return Err(GeneratorInputError::TooLarge {
            bytes: request.source.len(),
        });
    }
    generator_inputs_from_layout(request, layout)
}

fn generator_inputs_from_layout(
    request: &layout::SignalLayoutRequest<'_>,
    layout: &layout::SignalLayoutReport,
) -> Result<GeneratorInputReport, GeneratorInputError> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, request.source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        let message = parsed
            .errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(GeneratorInputError::Parse(if message.is_empty() {
            "parser panicked".into()
        } else {
            message
        }));
    }
    let (scoping, nodes) = SemanticBuilder::new()
        .with_check_syntax_error(false)
        .build(&parsed.program)
        .semantic
        .into_scoping_and_nodes();
    let slots = ParameterSlots::new(&parsed.program, &scoping, &nodes);
    let mut facts = ProgramFacts {
        source: request.source,
        scoping: &scoping,
        nodes: &nodes,
        slots: &slots,
        bindings: BTreeMap::new(),
        tables: Vec::new(),
        candidates: Vec::new(),
        stack: Vec::new(),
        loop_stack: Vec::new(),
        call_stack: Vec::new(),
        sync_callback_parents: BTreeMap::new(),
    };
    facts.visit_program(&parsed.program);
    facts.resolve_sync_parents();
    facts.resolve_statistic_divisions();
    let mut families = Vec::new();
    if let Some(candidate) = choose_glyph(&facts, layout)? {
        families.push(build_glyph_family(
            request.source,
            &facts,
            candidate,
            &layout,
        ));
    }
    if let Some(candidate) = choose_dom(&facts, layout)? {
        let candidate = dom_candidate(&facts, candidate);
        families.push(build_dom_family(
            request.source,
            &facts,
            &candidate,
            &layout,
        ));
    }
    if families.is_empty() {
        return Err(GeneratorInputError::Missing {
            family: FamilyKind::GlyphCanvasPixels,
            reason: "no source-backed glyph or DOM producer matched the supplied collectors".into(),
        });
    }
    Ok(GeneratorInputReport {
        schema_version: GENERATOR_INPUT_SCHEMA_VERSION,
        source: hash_source(request.source),
        families,
        unresolved: layout
            .unresolved
            .iter()
            .map(|item| UnresolvedInput {
                fact: item.reason.clone(),
                reason: item.detail.clone(),
                span: Some(item.span.into()),
                required: false,
            })
            .collect(),
    })
}

fn candidate_emissions(
    source: &str,
    facts: &ProgramFacts<'_, '_>,
    candidate: &CandidateFact,
    layout: &layout::SignalLayoutReport,
) -> Vec<GeneratorEmission> {
    let mut emissions = Vec::new();
    for signal in &layout.signals {
        for emission in &signal.emissions {
            if emission.origin.start < candidate.span.start
                || emission.origin.end > candidate.span.end
            {
                continue;
            }
            let buffer = first_leaf(&emission.wire).map(|leaf| evidence(source, leaf.span, None));
            let emission_receiver = emission_receiver_key(source, emission.origin);
            let matching_completions = candidate
                .calls
                .iter()
                .filter(|call| {
                    call.property.as_deref() == Some("complete")
                        && call.span.start >= emission.origin.end
                        && call.receiver_source.as_deref() == emission_receiver.as_deref()
                })
                .collect::<Vec<_>>();
            let complete = (matching_completions.len() == 1)
                .then(|| evidence(source, matching_completions[0].span, None));
            let mut unresolved = Vec::new();
            if complete.is_none() {
                unresolved.push(if matching_completions.is_empty() {
                    "producer completion is not proven in this function".into()
                } else {
                    "producer completion has multiple receiver-matching candidates".into()
                });
            }
            emissions.push(GeneratorEmission {
                signal: emission.signal,
                origin: evidence(source, emission.origin, None),
                route: emission.route.clone(),
                variant: emission.variant.clone(),
                wire: emission.wire.clone(),
                prefix: emission.prefix.clone(),
                mask: emission.mask.clone(),
                buffer,
                complete,
                unresolved,
            });
        }
    }
    emissions.sort_by_key(|emission| (emission.origin.span.start, emission.signal));
    let _ = facts;
    emissions
}

/// The producer's emissions that carry its pixel samples: those whose wire
/// repeats the number write of a sample read in one of the producer's loops.
/// A producer function that also hosts other collectors keeps their emissions
/// out of the glyph family.
fn glyph_emissions(
    source: &str,
    facts: &ProgramFacts<'_, '_>,
    candidate: &CandidateFact,
    layout: &layout::SignalLayoutReport,
) -> Vec<GeneratorEmission> {
    candidate_emissions(source, facts, candidate, layout)
        .into_iter()
        .filter(|emission| {
            candidate.loops.iter().any(|loop_| {
                loop_.reads.iter().any(|read| {
                    read.call_span.is_some_and(|call_span| {
                        wire_has_proven_repeat(
                            &emission.wire,
                            loop_,
                            source,
                            call_span,
                            read.indexed_span,
                        )
                    })
                })
            })
        })
        .collect()
}

fn emission_receiver_key(source: &str, span: Span) -> Option<String> {
    let text = source.get(span.start as usize..span.end as usize)?;
    let next = text.find(".next")?;
    Some(text[..next].trim().to_string())
}

fn first_leaf(wire: &layout::WireOp) -> Option<&layout::Leaf> {
    match wire {
        layout::WireOp::Leaf(leaf) => Some(leaf),
        layout::WireOp::Sequence(parts) | layout::WireOp::Choice(parts) => {
            parts.iter().find_map(first_leaf)
        }
        layout::WireOp::Repeat { body, .. } => first_leaf(body),
        layout::WireOp::Unknown { .. } => None,
    }
}

fn choose_glyph<'a>(
    facts: &'a ProgramFacts<'_, '_>,
    layout: &layout::SignalLayoutReport,
) -> Result<Option<&'a CandidateFact>, GeneratorInputError> {
    for candidate in &facts.candidates {
        let browser_shape = candidate
            .calls
            .iter()
            .any(|call| call.property.as_deref() == Some("fillText"))
            && candidate
                .calls
                .iter()
                .any(|call| call.property.as_deref() == Some("getImageData"))
            && candidate.loops.iter().any(|loop_| loop_.reverse)
            && has_emission(candidate, layout);
        if !browser_shape {
            continue;
        }
        if table_aliases(facts, candidate).count() > 1 {
            return Err(GeneratorInputError::Ambiguous {
                family: FamilyKind::GlyphCanvasPixels,
                candidates: vec![candidate.span.into()],
            });
        }
        // A browser-shaped glyph collector is a valid producer even when no
        // table alias resolves: the signal contracts need the producer's
        // source, not the glyph table itself.
        return Ok(Some(candidate));
    }
    let mut candidates = facts
        .candidates
        .iter()
        .filter(|candidate| {
            let has_fill = candidate
                .calls
                .iter()
                .any(|call| call.property.as_deref() == Some("fillText"));
            let has_pixels = candidate
                .calls
                .iter()
                .any(|call| call.property.as_deref() == Some("getImageData"));
            let has_sample_loop = candidate.loops.iter().any(|loop_| {
                loop_.reads.iter().any(|read| {
                    read.call_span.is_some_and(|call_span| {
                        layout.signals.iter().any(|signal| {
                            signal.emissions.iter().any(|emission| {
                                emission.origin.start >= candidate.span.start
                                    && emission.origin.end <= candidate.span.end
                                    && wire_has_proven_repeat(
                                        &emission.wire,
                                        loop_,
                                        facts.source,
                                        call_span,
                                        read.indexed_span,
                                    )
                            })
                        })
                    })
                })
            });
            let table_count = table_aliases(facts, candidate).count();
            let dynamic_table = table_count == 0 && dynamic_table_flow(candidate);
            let has_table_flow = table_count == 1 || dynamic_table;
            let loop_direction_proven = dynamic_table
                || candidate.loops.iter().any(|loop_| {
                    loop_.reverse
                        && loop_.reads.iter().any(|read| {
                            read.call_span.is_some_and(|call_span| {
                                layout.signals.iter().any(|signal| {
                                    signal.emissions.iter().any(|emission| {
                                        emission.origin.start >= candidate.span.start
                                            && emission.origin.end <= candidate.span.end
                                            && wire_has_proven_repeat(
                                                &emission.wire,
                                                loop_,
                                                facts.source,
                                                call_span,
                                                read.indexed_span,
                                            )
                                    })
                                })
                            })
                        })
                });
            has_fill
                && has_pixels
                && has_sample_loop
                && has_table_flow
                && loop_direction_proven
                && has_emission(candidate, layout)
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| candidate.span.start);
    if candidates.len() > 1 {
        return Err(GeneratorInputError::Ambiguous {
            family: FamilyKind::GlyphCanvasPixels,
            candidates: candidates
                .iter()
                .map(|candidate| candidate.span.into())
                .collect(),
        });
    }
    Ok(candidates.into_iter().next())
}

fn dynamic_table_flow(candidate: &CandidateFact) -> bool {
    let has_renderer = candidate
        .calls
        .iter()
        .any(|call| call.property.as_deref() == Some("fillText"));
    let has_coordinates = candidate
        .calls
        .iter()
        .any(|call| call.property.as_deref() == Some("getImageData") && call.args.len() >= 2);
    has_renderer
        && has_coordinates
        && candidate
            .members
            .iter()
            .any(|member| member.data_member && member.computed_root.is_some())
        && !candidate
            .aliases
            .values()
            .any(|root| candidate.invalid_symbols.contains(root))
}

fn choose_dom(
    facts: &ProgramFacts<'_, '_>,
    layout: &layout::SignalLayoutReport,
) -> Result<Option<usize>, GeneratorInputError> {
    let mut candidates = facts
        .candidates
        .iter()
        .enumerate()
        .filter(|(index, _candidate)| {
            let candidate = dom_candidate(facts, *index);
            let query = candidate
                .calls
                .iter()
                .any(|call| call.property.as_deref() == Some("querySelectorAll"));
            let tags = candidate
                .comparisons
                .iter()
                .any(|comparison| comparison.tag_member);
            let children = candidate.members.iter().any(|member| {
                matches!(
                    member.property.as_deref(),
                    Some("children" | "childElementCount")
                )
            });
            query && tags && children && has_emission(&candidate, layout)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|index| facts.candidates[*index].span.start);
    if candidates.len() > 1 {
        return Err(GeneratorInputError::Ambiguous {
            family: FamilyKind::DomCensus,
            candidates: candidates
                .iter()
                .map(|index| facts.candidates[*index].span.into())
                .collect(),
        });
    }
    Ok(candidates.into_iter().next())
}

fn dom_candidate(facts: &ProgramFacts<'_, '_>, index: usize) -> CandidateFact {
    let mut candidate = facts.candidates[index].clone();
    for child in facts
        .candidates
        .iter()
        .filter(|child| child.sync_parent == Some(index))
    {
        candidate.calls.extend(child.calls.clone());
        candidate.members.extend(child.members.clone());
        candidate.comparisons.extend(child.comparisons.clone());
        candidate.binaries.extend(child.binaries.clone());
        candidate.assignments.extend(child.assignments.clone());
        candidate.statistics.extend(child.statistics.clone());
    }
    candidate
}

fn has_emission(candidate: &CandidateFact, layout: &layout::SignalLayoutReport) -> bool {
    layout
        .signals
        .iter()
        .flat_map(|signal| signal.emissions.iter())
        .any(|emission| {
            emission.origin.start >= candidate.span.start
                && emission.origin.end <= candidate.span.end
        })
}

fn wire_has_proven_repeat(
    wire: &layout::WireOp,
    loop_: &LoopFact,
    source: &str,
    call_span: Span,
    indexed_span: Span,
) -> bool {
    match wire {
        layout::WireOp::Repeat { control, body } => {
            control.source.trim() == loop_.header.as_deref().unwrap_or_default().trim()
                && body_contains_number_write(body, call_span, indexed_span, source)
        }
        layout::WireOp::Sequence(parts) | layout::WireOp::Choice(parts) => parts
            .iter()
            .any(|part| wire_has_proven_repeat(part, loop_, source, call_span, indexed_span)),
        layout::WireOp::Leaf(_) | layout::WireOp::Unknown { .. } => false,
    }
}

fn body_contains_number_write(
    wire: &layout::WireOp,
    call_span: Span,
    indexed_span: Span,
    source: &str,
) -> bool {
    match wire {
        layout::WireOp::Leaf(leaf) => {
            leaf.kind == layout::LeafKind::Number
                && leaf.span == call_span
                && leaf.buffer.is_some()
                && leaf.value.as_deref()
                    == source.get(indexed_span.start as usize..indexed_span.end as usize)
        }
        layout::WireOp::Sequence(parts) | layout::WireOp::Choice(parts) => parts
            .iter()
            .any(|part| body_contains_number_write(part, call_span, indexed_span, source)),
        layout::WireOp::Repeat { body, .. } => {
            body_contains_number_write(body, call_span, indexed_span, source)
        }
        layout::WireOp::Unknown { .. } => false,
    }
}

fn candidate_emission_repeat_control(
    source: &str,
    candidate: &CandidateFact,
    layout: &layout::SignalLayoutReport,
    loop_: &LoopFact,
) -> Option<layout::LoopControl> {
    for read in &loop_.reads {
        let Some(call_span) = read.call_span else {
            continue;
        };
        for signal in &layout.signals {
            for emission in &signal.emissions {
                if emission.origin.start < candidate.span.start
                    || emission.origin.end > candidate.span.end
                {
                    continue;
                }
                if let Some(control) = repeat_control_for_loop(
                    &emission.wire,
                    loop_,
                    source,
                    call_span,
                    read.indexed_span,
                ) {
                    return Some(control);
                }
            }
        }
    }
    None
}

fn repeat_control_for_loop(
    wire: &layout::WireOp,
    loop_: &LoopFact,
    source: &str,
    call_span: Span,
    indexed_span: Span,
) -> Option<layout::LoopControl> {
    match wire {
        layout::WireOp::Repeat { control, body } => {
            if control.source.trim() == loop_.header.as_deref().unwrap_or_default().trim()
                && body_contains_number_write(body, call_span, indexed_span, source)
            {
                Some(control.clone())
            } else {
                None
            }
        }
        layout::WireOp::Sequence(parts) | layout::WireOp::Choice(parts) => parts
            .iter()
            .find_map(|part| repeat_control_for_loop(part, loop_, source, call_span, indexed_span)),
        layout::WireOp::Leaf(_) | layout::WireOp::Unknown { .. } => None,
    }
}

/// The glyph tables a candidate reads. A trace that carries the bundle more
/// than once repeats the table literal byte for byte; such copies are one
/// table (the first in source order), not an ambiguity.
fn table_aliases<'a>(
    facts: &'a ProgramFacts<'_, '_>,
    candidate: &'a CandidateFact,
) -> impl Iterator<Item = &'a TableInfo> + 'a {
    let text = |table: &TableInfo| facts.source.get(table.span.start as usize..table.span.end as usize);
    let mut seen: Vec<&'a str> = Vec::new();
    aliased_tables(facts, candidate).filter(move |table| match text(table) {
        Some(source) if seen.contains(&source) => false,
        Some(source) => {
            seen.push(source);
            true
        }
        None => true,
    })
}

fn aliased_tables<'a>(
    facts: &'a ProgramFacts<'_, '_>,
    candidate: &'a CandidateFact,
) -> impl Iterator<Item = &'a TableInfo> + 'a {
    facts.tables.iter().filter(move |table| {
        candidate
            .aliases
            .iter()
            .any(|(alias, _root)| canonical_alias(candidate, *alias) == Some(table.symbol))
            || candidate.members.iter().any(|member| {
                member
                    .root
                    .and_then(|root| canonical_alias(candidate, root))
                    == Some(table.symbol)
            })
    })
}

fn canonical_alias(candidate: &CandidateFact, symbol: SymbolId) -> Option<SymbolId> {
    let mut current = symbol;
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(current) || candidate.invalid_symbols.contains(&current) {
            return None;
        }
        let Some(next) = candidate.aliases.get(&current).copied() else {
            return Some(current);
        };
        current = next;
    }
}

fn build_glyph_family(
    source: &str,
    facts: &ProgramFacts<'_, '_>,
    candidate: &CandidateFact,
    layout: &layout::SignalLayoutReport,
) -> GeneratorInputFamily {
    let tables = table_aliases(facts, candidate).collect::<Vec<_>>();
    let mut table = tables
        .first()
        .map(|table| evidence(source, table.span, Some(&table.binding)));
    let mut entries = Vec::new();
    let alias_symbols = candidate.aliases.keys().copied().collect::<BTreeSet<_>>();
    for table in &tables {
        for (index, entry) in table.entries.iter().enumerate() {
            let mut properties = Vec::new();
            for property in &entry.properties {
                let role = candidate
                    .members
                    .iter()
                    .filter(|member| {
                        member.property.as_ref() == property.key.as_ref()
                            && (alias_symbols.contains(&member.root.unwrap_or(table.symbol))
                                || member.row_root == Some(table.symbol))
                    })
                    .find_map(|member| {
                        // The entry property is the index of a pixel read
                        // (`data[row.component]`); a property read inside the
                        // indexed object, such as an inline
                        // `getImageData(row.x, ...)["data"][...]`, is not.
                        if candidate.members.iter().any(|outer| {
                            outer.property.is_none()
                                && outer.computed_root == member.root
                                && outer.index_span.is_some_and(|index| {
                                    index.start <= member.span.start && index.end >= member.span.end
                                })
                        }) {
                            return Some("pixel_component");
                        }
                        candidate.calls.iter().find_map(|call| {
                            let position = call.args.iter().position(|arg| {
                                member.span.start >= arg.span.start
                                    && member.span.end <= arg.span.end
                            });
                            match (call.property.as_deref(), position, member.computed_root) {
                                (Some("fillText"), Some(0), _) => Some("renderer_glyph_text"),
                                (Some("getImageData"), Some(0), _) => Some("pixel_coordinate_x"),
                                (Some("getImageData"), Some(1), _) => Some("pixel_coordinate_y"),
                                (_, _, Some(_)) => Some("pixel_component"),
                                _ => None,
                            }
                        })
                    })
                    .map(str::to_string)
                    .unwrap_or_else(|| "table_property".into());
                properties.push(TableProperty {
                    key: property.key.clone(),
                    role,
                    evidence: evidence(source, property.value_span, None),
                });
            }
            entries.push(TableEntry {
                index,
                evidence: evidence(source, entry.span, None),
                properties,
            });
        }
    }
    if entries.is_empty() && table.is_none() && dynamic_table_flow(candidate) {
        let render_call = candidate
            .calls
            .iter()
            .find(|call| call.property.as_deref() == Some("fillText"));
        let pixel_call = candidate
            .calls
            .iter()
            .find(|call| call.property.as_deref() == Some("getImageData"));
        let pixel_component = candidate
            .members
            .iter()
            .find(|member| member.data_member && member.computed_root.is_some())
            .map(|member| evidence(source, member.span, None));
        let mut properties = Vec::new();
        if let Some(call) = render_call {
            if let Some(argument) = call.args.first() {
                properties.push(TableProperty {
                    key: None,
                    role: "renderer_glyph_text".into(),
                    evidence: evidence(source, argument.span, None),
                });
            }
        }
        if let Some(call) = pixel_call {
            for (role, argument) in [
                ("pixel_coordinate_x", call.args.first()),
                ("pixel_coordinate_y", call.args.get(1)),
            ] {
                if let Some(argument) = argument {
                    properties.push(TableProperty {
                        key: None,
                        role: role.into(),
                        evidence: evidence(source, argument.span, None),
                    });
                }
            }
        }
        if let Some(pixel_component) = pixel_component {
            properties.push(TableProperty {
                key: None,
                role: "pixel_component".into(),
                evidence: pixel_component,
            });
        }
        if !properties.is_empty() {
            if table.is_none() {
                table = Some(evidence(source, candidate.span, None));
            }
            entries.push(TableEntry {
                index: 0,
                evidence: evidence(source, candidate.span, None),
                properties,
            });
        }
    }
    let render_text = candidate
        .calls
        .iter()
        .find(|call| call.property.as_deref() == Some("fillText"))
        .and_then(|call| call.args.first())
        .map(|arg| evidence(source, arg.span, None));
    let mut coordinates: Vec<SourceEvidence> = candidate
        .calls
        .iter()
        .find(|call| call.property.as_deref() == Some("getImageData"))
        .map(|call| {
            call.args
                .iter()
                .take(2)
                .map(|arg| evidence(source, arg.span, None))
                .collect()
        })
        .unwrap_or_default();
    if let Some(call) = candidate
        .calls
        .iter()
        .find(|call| call.property.as_deref() == Some("fillText"))
    {
        coordinates.extend(
            call.args
                .iter()
                .skip(1)
                .take(2)
                .map(|arg| evidence(source, arg.span, None)),
        );
    }
    let pixel_component = candidate
        .members
        .iter()
        .find(|member| member.data_member && member.computed_root.is_some())
        .map(|member| evidence(source, member.span, None));
    let reverse = candidate
        .loops
        .iter()
        .filter(|loop_| loop_.reverse)
        .find_map(|loop_| {
            let control = candidate_emission_repeat_control(source, candidate, layout, loop_)?;
            Some(LoopEvidence {
                evidence: evidence(source, loop_.span, None),
                kind: loop_.kind,
                control,
                reverse: true,
            })
        });
    let mut inputs = Vec::new();
    if let Some(table) = &table {
        inputs.push(InputProvenance {
            role: "glyph_configuration_table".into(),
            kind: InputKind::StaticConfig,
            evidence: table.clone(),
            optional: false,
            notes: vec!["table is linked by entry property use, not by property spelling".into()],
        });
    }
    if let Some(render_text) = &render_text {
        inputs.push(InputProvenance {
            role: "renderer_glyph_text".into(),
            kind: InputKind::StaticConfig,
            evidence: render_text.clone(),
            optional: false,
            notes: vec!["renderer input; no STRING wire write is inferred".into()],
        });
    }
    let emissions = glyph_emissions(source, facts, candidate, layout);
    for emission in emissions.iter().cloned() {
        inputs.push(InputProvenance {
            role: "sampled_pixel_values".into(),
            kind: InputKind::RuntimeObservation,
            evidence: emission.origin.clone(),
            optional: false,
            notes: vec![
                "pixel bytes are runtime observations; only their source-derived loop is reported"
                    .into(),
            ],
        });
        if let Some(prefix) = &emission.prefix {
            inputs.push(InputProvenance {
                role: "random_prefix_seed".into(),
                kind: InputKind::ExternalSeedState,
                evidence: evidence(
                    source,
                    Span::new(emission.origin.span.start, emission.origin.span.end),
                    None,
                ),
                optional: false,
                notes: vec![format!(
                    "{} prefix bytes remain emission-scoped",
                    prefix.byte_width
                )],
            });
        }
        if let Some(mask) = &emission.mask {
            if let Some(span) = (!mask.xor_span.is_empty()).then_some(mask.xor_span) {
                inputs.push(InputProvenance {
                    role: "static_mask_operand".into(),
                    kind: InputKind::StaticConfig,
                    evidence: evidence(source, span, None),
                    optional: false,
                    notes: vec!["source literal; runtime seed remains separate".into()],
                });
            }
        }
    }
    let draw_configuration: Vec<InputProvenance> = candidate
        .assignments
        .iter()
        .filter(|assignment| {
            let renderer_root = candidate.calls.iter().find_map(|call| {
                matches!(call.property.as_deref(), Some("fillText" | "getImageData"))
                    .then_some(call.receiver_root)
                    .flatten()
            });
            let render_start = candidate
                .calls
                .iter()
                .filter(|call| call.property.as_deref() == Some("fillText"))
                .map(|call| call.span.start)
                .min();
            renderer_root.is_some()
                && assignment.target_root == renderer_root
                && render_start.is_some_and(|start| assignment.span.start < start)
        })
        .map(|assignment| InputProvenance {
            role: "draw_configuration".into(),
            kind: InputKind::StaticConfig,
            evidence: evidence(source, assignment.value_span, None),
            optional: true,
            notes: Vec::new(),
        })
        .collect();
    let missing_draw_configuration = draw_configuration.is_empty();
    let glyph = GlyphContract {
        table,
        entries,
        render_text,
        coordinates,
        pixel_component,
        sample_count: candidate
            .members
            .iter()
            .find(|member| {
                member.property.as_deref() == Some("length")
                    && candidate.loops.iter().any(|loop_| {
                        loop_
                            .reads
                            .iter()
                            .any(|read| read.sample_root == member.root && read.call_span.is_some())
                    })
            })
            .map(|member| evidence(source, member.span, None)),
        reverse_sample_loop: reverse,
        draw_configuration,
        inputs: inputs.clone(),
    };
    let mut unresolved = if emissions.is_empty() {
        vec![UnresolvedInput {
            fact: "glyph_emission".into(),
            reason:
                "producer operations were found without a matching source-scoped signal emission"
                    .into(),
            span: Some(candidate.span.into()),
            required: true,
        }]
    } else {
        Vec::new()
    };
    if missing_draw_configuration {
        unresolved.push(UnresolvedInput {
            fact: "glyph_draw_configuration".into(),
            reason: "canvas size/font/fill assignments were not proven in the producer".into(),
            span: Some(candidate.span.into()),
            required: false,
        });
    }
    GeneratorInputFamily {
        kind: FamilyKind::GlyphCanvasPixels,
        producer: evidence(source, candidate.span, candidate.binding.as_ref()),
        emissions,
        inputs,
        glyph: Some(glyph),
        dom: None,
        unresolved,
    }
}

fn build_dom_family(
    source: &str,
    facts: &ProgramFacts<'_, '_>,
    candidate: &CandidateFact,
    layout: &layout::SignalLayoutReport,
) -> GeneratorInputFamily {
    let selector = candidate
        .calls
        .iter()
        .find(|call| call.property.as_deref() == Some("querySelectorAll"))
        .and_then(|call| call.args.first())
        .map(|arg| evidence(source, arg.span, None));
    let compared_tags = candidate
        .comparisons
        .iter()
        .filter(|comparison| comparison.tag_member)
        .filter_map(|comparison| {
            comparison
                .string
                .as_ref()
                .map(|string| evidence(source, string.span, None))
        })
        .collect::<Vec<_>>();
    let id_slice_limit = candidate
        .calls
        .iter()
        .find(|call| call.property.as_deref() == Some("slice") && call.id_slice_receiver)
        .and_then(|call| call.args.get(1))
        .map(|arg| evidence(source, arg.span, None));
    let statistic = candidate.statistics.iter().find_map(|statistic| {
        let division_span = statistic.division_span?;
        let sampled_children = candidate.calls.iter().any(|call| {
            call.property.as_deref() == Some("push")
                && call.receiver_root == statistic.array_root
                && call.args.iter().any(|argument| {
                    candidate.members.iter().any(|length| {
                        length.property.as_deref() == Some("length")
                            && length.span.start >= argument.span.start
                            && length.span.end <= argument.span.end
                            && candidate.members.iter().any(|children| {
                                children.property.as_deref() == Some("children")
                                    && children.span.start >= argument.span.start
                                    && children.span.end <= length.span.end
                                    && children.span.start <= length.span.start
                            })
                    })
                })
        });
        sampled_children.then(|| evidence(source, division_span, None))
    });
    let mut inputs = Vec::new();
    if let Some(selector) = &selector {
        inputs.push(InputProvenance {
            role: "dom_selector".into(),
            kind: InputKind::StaticConfig,
            evidence: selector.clone(),
            optional: false,
            notes: Vec::new(),
        });
    }
    for tag in &compared_tags {
        inputs.push(InputProvenance {
            role: "dom_compared_tag".into(),
            kind: InputKind::StaticConfig,
            evidence: tag.clone(),
            optional: false,
            notes: vec!["tag label is source evidence, not a recognition key".into()],
        });
    }
    if let Some(limit) = &id_slice_limit {
        inputs.push(InputProvenance {
            role: "body_first_child_id_prefix_limit".into(),
            kind: InputKind::StaticConfig,
            evidence: limit.clone(),
            optional: true,
            notes: Vec::new(),
        });
    }
    if let Some(statistic) = &statistic {
        inputs.push(InputProvenance {
            role: "children_count_statistic".into(),
            kind: InputKind::DerivedConfig,
            evidence: statistic.clone(),
            optional: false,
            notes: vec!["measured DOM values remain runtime observations".into()],
        });
    }
    let emissions = candidate_emissions(source, facts, candidate, layout);
    for emission in &emissions {
        inputs.push(InputProvenance {
            role: "dom_measurements".into(),
            kind: InputKind::RuntimeObservation,
            evidence: emission.origin.clone(),
            optional: false,
            notes: vec![
                "DOM counts and text remain runtime inputs; no measured value is fabricated".into(),
            ],
        });
        if let Some(prefix) = &emission.prefix {
            inputs.push(InputProvenance {
                role: "random_prefix_seed".into(),
                kind: InputKind::ExternalSeedState,
                evidence: emission.origin.clone(),
                optional: false,
                notes: vec![format!(
                    "{} prefix bytes remain unmeasured runtime state",
                    prefix.byte_width
                )],
            });
        }
        if let Some(mask) = &emission.mask {
            if let Some(span) = (!mask.xor_span.is_empty()).then_some(mask.xor_span) {
                inputs.push(InputProvenance {
                    role: "local_static_salt".into(),
                    kind: InputKind::StaticConfig,
                    evidence: evidence(source, span, None),
                    optional: false,
                    notes: Vec::new(),
                });
            }
        }
    }
    let missing_id_slice_limit = id_slice_limit.is_none();
    let missing_statistic = statistic.is_none();
    let dom = DomContract {
        selector,
        compared_tags,
        id_slice_limit,
        statistic,
        inputs: inputs.clone(),
    };
    let mut unresolved = if emissions.is_empty() {
        vec![UnresolvedInput {
            fact: "dom_emission".into(),
            reason: "DOM producer operations were found without a source-scoped signal emission"
                .into(),
            span: Some(candidate.span.into()),
            required: true,
        }]
    } else {
        Vec::new()
    };
    if missing_id_slice_limit {
        unresolved.push(UnresolvedInput {
            fact: "dom_id_slice_limit".into(),
            reason: "no first-child id slice upper bound was proven; optional path remains unknown"
                .into(),
            span: Some(candidate.span.into()),
            required: false,
        });
    }
    if missing_statistic {
        unresolved.push(UnresolvedInput {
            fact: "dom_children_statistic".into(),
            reason: "children-count statistic expression was not proven".into(),
            span: Some(candidate.span.into()),
            required: true,
        });
    }
    GeneratorInputFamily {
        kind: FamilyKind::DomCensus,
        producer: evidence(source, candidate.span, candidate.binding.as_ref()),
        emissions,
        inputs,
        glyph: None,
        dom: Some(dom),
        unresolved,
    }
}
