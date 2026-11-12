//! SymbolId-based proofs for the dispatch/operation join.
//!
//! Nothing here trusts a spelling. The dispatch read, the operation-table
//! symbol, the selector value and the context receiver are all resolved through
//! Oxc `SymbolId`s and the code-generation-stable operation-table declaration
//! ordinal. Every relation must be unique: an ambiguous program is rejected
//! rather than resolved to the first match.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::node::NodeId;
use oxc_syntax::operator::AssignmentOperator;

use crate::kernel::config::SourceSpan;
use crate::operations::operands::{JsOperandBindingHandoff, OperandEntryCoordinate, OperandSpan};
use crate::vm_patterns::{
    recognize_nested_dispatch, recognize_nested_dispatch_member, NestedDispatch,
    NestedDispatchPolicy,
};
use crate::ProvenOperationTable;

use super::types::DispatchPrepareError;

/// The proven dispatch read: the nested `table[this.S][bytecode[this.P++]]`
/// shape with the method that consumes it.
pub(super) struct DispatchReadProof {
    pub(super) table: SymbolId,
    pub(super) table_name: String,
    pub(super) bytecode: SymbolId,
    pub(super) bytecode_name: String,
    pub(super) pc_property: String,
    pub(super) method_name: String,
    pub(super) context_constructor: SymbolId,
    pub(super) bytecode_span: OperandSpan,
}

impl DispatchReadProof {
    pub(super) fn handoff(&self) -> JsOperandBindingHandoff {
        JsOperandBindingHandoff {
            bytecode_declaration_span: self.bytecode_span.clone(),
            pc_property: self.pc_property.clone(),
            entry_coordinate: OperandEntryCoordinate::AfterOpcode,
        }
    }
}

fn span(value: oxc_span::Span) -> OperandSpan {
    OperandSpan {
        start: value.start,
        end: value.end,
    }
}

fn parse<'a>(
    allocator: &'a Allocator,
    source: &'a str,
) -> Result<Program<'a>, DispatchPrepareError> {
    crate::analysis::parse_program(allocator, source).map_err(DispatchPrepareError::Parse)
}

/// Prove exactly one nested dispatch read and resolve the method that owns it.
pub(super) fn prove_dispatch_read(source: &str) -> Result<DispatchReadProof, DispatchPrepareError> {
    let allocator = Allocator::default();
    let program = parse(&allocator, source)?;
    let (scoping, _nodes) = crate::analysis::analyze(&program);
    let mut reader = DispatchReadFinder {
        scoping: &scoping,
        functions: Vec::new(),
        candidates: Vec::new(),
    };
    reader.visit_program(&program);
    let match_count = reader.candidates.len();
    if match_count != 1 {
        return Err(DispatchPrepareError::SourceJoin(format!(
            "expected exactly one nested dispatch read, found {match_count}"
        )));
    }
    let candidate = reader.candidates.into_iter().next().expect("one candidate");
    let bytecode_span = span(scoping.symbol_span(candidate.bytecode));
    let mut namer = MethodNameFinder {
        scoping: &scoping,
        target: candidate.method_span.clone(),
        prototype_aliases: prototype_aliases(&program, &scoping),
        names: Vec::new(),
        writes: Vec::new(),
        installations: Vec::new(),
    };
    namer.visit_program(&program);
    if namer.names.len() != 1
        || namer
            .writes
            .iter()
            .filter(|name| *name == &namer.names[0])
            .count()
            != 1
        || namer.installations.len() != 1
    {
        return Err(DispatchPrepareError::SourceJoin(
            "dispatch method is not uniquely defined as one named property".into(),
        ));
    }
    Ok(DispatchReadProof {
        table: candidate.table,
        table_name: candidate.table_name,
        bytecode: candidate.bytecode,
        bytecode_name: candidate.bytecode_name,
        pc_property: candidate.pc_property,
        method_name: namer.names.into_iter().next().expect("one name"),
        context_constructor: namer
            .installations
            .into_iter()
            .next()
            .expect("one installation")
            .constructor,
        bytecode_span,
    })
}

struct DispatchCandidate {
    table: SymbolId,
    table_name: String,
    bytecode: SymbolId,
    bytecode_name: String,
    pc_property: String,
    state_property: String,
    method_span: OperandSpan,
}

struct DispatchReadFinder<'a> {
    scoping: &'a Scoping,
    functions: Vec<OperandSpan>,
    candidates: Vec<DispatchCandidate>,
}

impl<'a> DispatchReadFinder<'a> {
    fn nested_dispatch(&self, expression: &Expression<'_>) -> Option<NestedDispatch> {
        recognize_nested_dispatch(expression, self.scoping, NestedDispatchPolicy::StrictThisPc)
    }

    fn nested_dispatch_member(
        &self,
        member: &ComputedMemberExpression<'_>,
    ) -> Option<NestedDispatch> {
        recognize_nested_dispatch_member(member, self.scoping, NestedDispatchPolicy::StrictThisPc)
    }

    fn enclosing(&self) -> Option<OperandSpan> {
        self.functions.last().cloned()
    }

    fn valid_method_body(
        &self,
        body: Option<&FunctionBody<'a>>,
        candidate: &DispatchCandidate,
    ) -> bool {
        let Some(body) = body else { return false };
        // The JavaScript VM dispatch method has exactly one transparent block wrapper around
        // its body. Only unwrap that non-executable wrapper; the executable
        // statement list must still be exactly the complete three-statement
        // template below. This mirrors `runtime_config.rs`'s extractor.
        let statements = match body.statements.as_slice() {
            [Statement::BlockStatement(block)] => block.body.as_slice(),
            statements => statements,
        };
        if statements.len() != 3 {
            return false;
        }
        let Statement::VariableDeclaration(declaration) = &statements[0] else {
            return false;
        };
        if declaration.declarations.len() != 1 {
            return false;
        }
        let declarator = &declaration.declarations[0];
        let BindingPattern::BindingIdentifier(record) = &declarator.id else {
            return false;
        };
        let Some(initializer) = declarator.init.as_ref() else {
            return false;
        };
        let Some(dispatch) = self.nested_dispatch(initializer) else {
            return false;
        };
        if dispatch.table != candidate.table
            || dispatch.bytecode != Some(candidate.bytecode)
            || dispatch.pc_property.as_deref() != Some(candidate.pc_property.as_str())
            || dispatch.state_property.as_deref() != Some(candidate.state_property.as_str())
        {
            return false;
        }
        let Some(record_symbol) = record.symbol_id.get() else {
            return false;
        };
        let Statement::ExpressionStatement(update) = &statements[1] else {
            return false;
        };
        let Expression::AssignmentExpression(update) = &update.expression else {
            return false;
        };
        if update.operator != AssignmentOperator::Assign {
            return false;
        }
        let AssignmentTarget::StaticMemberExpression(state_target) = &update.left else {
            return false;
        };
        if !matches!(&state_target.object, Expression::ThisExpression(_))
            || state_target.property.name.to_string() != candidate.state_property
            || !self.record_index(&update.right, record_symbol, 0.0)
        {
            return false;
        }
        let Statement::ReturnStatement(return_statement) = &statements[2] else {
            return false;
        };
        return_statement
            .argument
            .as_ref()
            .is_some_and(|value| self.record_index(value, record_symbol, 1.0))
    }

    fn record_index(&self, expression: &Expression<'_>, record: SymbolId, index: f64) -> bool {
        let Expression::ComputedMemberExpression(member) = expression else {
            return false;
        };
        let Expression::Identifier(identifier) = &member.object else {
            return false;
        };
        crate::analysis::identifier_symbol(identifier, self.scoping) == Some(record)
            && matches!(&member.expression, Expression::NumericLiteral(value) if value.value == index)
    }
}

impl<'a> Visit<'a> for DispatchReadFinder<'a> {
    fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        let function_span = span(it.span);
        let disallowed = it.generator || it.r#async;
        let before = self.candidates.len();
        self.functions.push(function_span.clone());
        walk::walk_function(self, it, flags);
        self.functions.pop();
        let candidates = self.candidates.drain(before..).collect::<Vec<_>>();
        for candidate in candidates {
            let keep = candidate.method_span != function_span
                || (!disallowed && self.valid_method_body(it.body.as_deref(), &candidate));
            if keep {
                self.candidates.push(candidate);
            }
        }
    }
    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.functions.push(span(it.span));
        walk::walk_arrow_function_expression(self, it);
        self.functions.pop();
    }
    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        if let Some(dispatch) = self.nested_dispatch_member(it) {
            if let Some(method_span) = self.enclosing() {
                self.candidates.push(DispatchCandidate {
                    table: dispatch.table,
                    table_name: dispatch.table_name,
                    bytecode: dispatch.bytecode.expect("strict dispatch bytecode"),
                    bytecode_name: dispatch
                        .bytecode_name
                        .expect("strict dispatch bytecode name"),
                    pc_property: dispatch.pc_property.expect("strict dispatch pc property"),
                    state_property: dispatch
                        .state_property
                        .expect("strict dispatch state property"),
                    method_span,
                });
            }
        }
        walk::walk_computed_member_expression(self, it);
    }
}

/// Find the single property name under which the dispatch method function is
/// installed (`Object.defineProperty(target, "name", { value: fn })` or
/// `target.name = fn`).
struct MethodNameFinder<'a> {
    scoping: &'a Scoping,
    target: OperandSpan,
    /// `var alias = Ctor.prototype` edges, validated immutable and unique.
    prototype_aliases: BTreeMap<SymbolId, SymbolId>,
    names: Vec<String>,
    writes: Vec<String>,
    installations: Vec<MethodInstallation>,
}

struct MethodInstallation {
    constructor: SymbolId,
}

impl<'a> MethodNameFinder<'a> {
    fn is_target(&self, expression: &Expression<'_>) -> bool {
        matches!(expression.span().start, start if start == self.target.start)
            && expression.span().end == self.target.end
    }
    fn literal_name(expression: &Expression<'_>) -> Option<String> {
        match expression {
            Expression::StringLiteral(value) => Some(value.value.to_string()),
            _ => None,
        }
    }

    fn constructor_symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        if let Expression::StaticMemberExpression(prototype) = expression {
            if prototype.property.name != "prototype" {
                return None;
            }
            let Expression::Identifier(identifier) = &prototype.object else {
                return None;
            };
            return crate::analysis::identifier_symbol(identifier, self.scoping);
        }
        // `var alias = Ctor.prototype` is installed as an immutable alias: only
        // a unique, single-declaration, never-mutated binding is admitted.
        if let Expression::Identifier(identifier) = expression {
            let symbol = crate::analysis::identifier_symbol(identifier, self.scoping)?;
            return self.prototype_aliases.get(&symbol).copied();
        }
        None
    }
}

/// Collect `var alias = Ctor.prototype` edges and resolve each admitted alias to
/// the constructor at the end of its chain. Every admitted alias is declared
/// exactly once and is never assigned, updated, or member-written, so its
/// constructor identity is immutable. A chain that revisits an alias (a cycle)
/// proves nothing and is left unresolved instead of looping.
fn prototype_aliases(program: &Program<'_>, scoping: &Scoping) -> BTreeMap<SymbolId, SymbolId> {
    struct Collector<'a> {
        scoping: &'a Scoping,
        edges: Vec<(SymbolId, SymbolId)>,
    }
    impl<'a, 'b> Visit<'b> for Collector<'a> {
        fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'b>) {
            if let (BindingPattern::BindingIdentifier(id), Some(init)) =
                (&declarator.id, declarator.init.as_ref())
            {
                if let (Some(left), Some(right)) =
                    (id.symbol_id.get(), prototype_of(init, self.scoping))
                {
                    self.edges.push((left, right));
                }
            }
            walk::walk_variable_declarator(self, declarator);
        }
    }
    fn prototype_of(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
        let Expression::StaticMemberExpression(prototype) = expression else {
            return None;
        };
        if prototype.property.name != "prototype" {
            return None;
        }
        let Expression::Identifier(identifier) = &prototype.object else {
            return None;
        };
        crate::analysis::identifier_symbol(identifier, scoping)
    }
    let mut collector = Collector {
        scoping,
        edges: Vec::new(),
    };
    collector.visit_program(program);

    // Admit only immutable, single-declaration aliases, keyed by one edge each.
    let mut edges = BTreeMap::new();
    for (alias, target) in &collector.edges {
        if scoping.symbol_declarations(*alias).count() == 1 && !scoping.symbol_is_mutated(*alias) {
            edges.insert(*alias, *target);
        }
    }

    let mut resolved = BTreeMap::new();
    for alias in edges.keys().copied() {
        let mut seen = BTreeSet::new();
        let mut current = alias;
        loop {
            if !seen.insert(current) {
                // A cycle cannot name a single immutable constructor identity.
                break;
            }
            match edges.get(&current) {
                // Follow alias -> alias edges one step at a time.
                Some(next) => current = *next,
                None => {
                    resolved.insert(alias, current);
                    break;
                }
            }
        }
    }
    resolved
}

impl<'a> Visit<'a> for MethodNameFinder<'a> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        // Object.defineProperty(TARGET, "NAME", { value: FUNCTION })
        if let Expression::StaticMemberExpression(member) = &call.callee {
            if member.property.name == "defineProperty" {
                let name = call
                    .arguments
                    .get(1)
                    .and_then(Argument::as_expression)
                    .and_then(Self::literal_name);
                if let Some(name) = name.as_ref() {
                    self.writes.push(name.clone());
                }
                let value = call.arguments.get(2).and_then(Argument::as_expression);
                if let (Some(name), Some(value)) = (name, value) {
                    if let Some(function) = object_property(value, "value") {
                        if self.is_target(function) {
                            self.names.push(name);
                            if let Some(constructor) = call
                                .arguments
                                .first()
                                .and_then(Argument::as_expression)
                                .and_then(|target| self.constructor_symbol(target))
                            {
                                self.installations.push(MethodInstallation { constructor });
                            }
                        }
                    }
                }
            }
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if let AssignmentTarget::StaticMemberExpression(member) = &assignment.left {
            self.writes.push(member.property.name.to_string());
        }
        if assignment.operator == AssignmentOperator::Assign {
            if let (AssignmentTarget::StaticMemberExpression(member), true) =
                (&assignment.left, self.is_target(&assignment.right))
            {
                self.names.push(member.property.name.to_string());
                if let Some(constructor) = self.constructor_symbol(&member.object) {
                    self.installations.push(MethodInstallation { constructor });
                }
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
}

fn object_property<'a, 'b>(
    expression: &'b Expression<'a>,
    key: &str,
) -> Option<&'b Expression<'a>> {
    let Expression::ObjectExpression(object) = expression else {
        return None;
    };
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            continue;
        };
        let matches = match &property.key {
            PropertyKey::StaticIdentifier(identifier) => identifier.name == key,
            PropertyKey::StringLiteral(value) => value.value == key,
            _ => false,
        };
        if matches {
            return Some(&property.value);
        }
    }
    None
}

/// Prove the bytecode binding is an immutable, non-escaping literal image.
pub(super) fn prove_direct_bytecode(
    source: &str,
    proof: &DispatchReadProof,
) -> Result<Vec<u8>, DispatchPrepareError> {
    let allocator = Allocator::default();
    let program = parse(&allocator, source)?;
    let (scoping, _nodes) = crate::analysis::analyze(&program);
    if scoping.symbol_declarations(proof.bytecode).count() != 1 {
        return Err(DispatchPrepareError::SourceJoin(
            "bytecode binding is declared more than once".into(),
        ));
    }
    let mut finder = BytecodeFinder {
        target: proof.bytecode,
        bytes: None,
    };
    finder.visit_program(&program);
    let bytes = finder.bytes.ok_or_else(|| {
        DispatchPrepareError::InvalidFact(
            "bytecode binding is not a bounded numeric-literal image".into(),
        )
    })?;
    // Reject any assignment, update, member write, or escape of the bytecode
    // binding or of a binding aliased from it.
    let tracked = tracked_aliases(&program, &scoping, proof.bytecode);
    let mut scanner = MutationScanner {
        scoping: &scoping,
        root: proof.bytecode,
        targets: tracked.full,
        member_targets: tracked.member,
        label: "bytecode image",
        violations: Vec::new(),
    };
    scanner.visit_program(&program);
    if let Some(violation) = scanner.violations.into_iter().next() {
        return Err(DispatchPrepareError::SourceJoin(format!(
            "bytecode image is not immutable: {violation}"
        )));
    }
    Ok(bytes)
}

/// Resolve an artifact's recorded declaration span to exactly one SymbolId.
/// The display binding name is only a diagnostic; this is the source-identity
/// join used by preparation.
pub(super) fn require_binding_at_span(
    source: &str,
    recorded: &SourceSpan,
    expected: SymbolId,
    label: &str,
) -> Result<(), DispatchPrepareError> {
    let allocator = Allocator::default();
    let program = parse(&allocator, source)?;
    let (scoping, _nodes) = crate::analysis::analyze(&program);
    let mut matches = Vec::new();
    let target = OperandSpan {
        start: recorded.start,
        end: recorded.end,
    };
    let mut resolver = DeclarationResolver {
        target,
        matches: Vec::new(),
    };
    resolver.visit_program(&program);
    matches.extend(resolver.matches);
    if matches.len() != 1 || matches[0] != expected {
        return Err(DispatchPrepareError::TableIdentity(format!(
            "{label} artifact span does not resolve to the exact proven declaration"
        )));
    }
    let _ = scoping;
    Ok(())
}

/// Resolve an artifact's recorded decoder-call span to the exact proven
/// bytecode binding. This is the identity join for an image whose bytes came
/// from a proven base64 decoder call rather than a literal array.
pub(super) fn require_decoder_call_binding(
    source: &str,
    recorded: &SourceSpan,
    expected: SymbolId,
    label: &str,
) -> Result<(), DispatchPrepareError> {
    let allocator = Allocator::default();
    let program = parse(&allocator, source)?;
    let (scoping, _nodes) = crate::analysis::analyze(&program);
    let mut finder = InitializerResolver {
        target: OperandSpan {
            start: recorded.start,
            end: recorded.end,
        },
        matches: Vec::new(),
    };
    finder.visit_program(&program);
    if finder.matches.len() != 1 || finder.matches[0] != expected {
        return Err(DispatchPrepareError::TableIdentity(format!(
            "{label} artifact span does not resolve to the exact proven binding initializer"
        )));
    }
    let _ = scoping;
    Ok(())
}

struct InitializerResolver {
    target: OperandSpan,
    matches: Vec<SymbolId>,
}

impl<'a> Visit<'a> for InitializerResolver {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(id), Some(init)) =
            (&declarator.id, declarator.init.as_ref())
        {
            if init.span().start == self.target.start && init.span().end == self.target.end {
                if let Some(symbol) = id.symbol_id.get() {
                    self.matches.push(symbol);
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}

struct DeclarationResolver {
    target: OperandSpan,
    matches: Vec<SymbolId>,
}

impl<'a> Visit<'a> for DeclarationResolver {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(id) = &declarator.id {
            if (declarator.span.start == self.target.start
                && declarator.span.end == self.target.end)
                || (id.span.start == self.target.start && id.span.end == self.target.end)
            {
                if let Some(symbol) = id.symbol_id.get() {
                    self.matches.push(symbol);
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}

/// Recover the operation-table declaration identity from a standardized source
/// that is already the pipeline output.
///
/// A raw capture carries FIND/RENAME provenance for the operation table. When
/// the input is deobfuscated output, that provenance is gone, but the table is
/// still structurally proven by the property extractor. Re-derive the same
/// code-generation-stable identity only when the source contains exactly one
/// consumed table with exactly that shape.
pub(crate) fn recover_operation_table(source: &str) -> Result<ProvenOperationTable, String> {
    let operations = crate::kernel::properties::extract_actual_operation_tables(source)
        .map_err(|error| error.to_string())?;
    if operations.len() != 1 {
        return Err(format!(
            "expected exactly one proven operation table, found {}",
            operations.len()
        ));
    }
    let expected = &operations[0];
    let allocator = Allocator::default();
    let program = parse(&allocator, source).map_err(|error| error.to_string())?;
    let (scoping, _nodes) = crate::analysis::analyze(&program);
    let mut finder = OperationTableRecovery {
        ordinal: 0,
        name: &expected.name,
        count: expected.count as u32,
        matches: Vec::new(),
    };
    finder.visit_program(&program);
    if finder.matches.len() != 1 {
        return Err(format!(
            "operation-table declaration `{}` is ambiguous or missing",
            expected.name
        ));
    }
    let (symbol, ordinal, elements, functions, name) = &finder.matches[0];
    if name != &expected.name
        || *elements != finder.count
        || *functions != finder.count
        || scoping.symbol_declarations(*symbol).count() != 1
    {
        return Err("operation-table declaration shape or identity changed after proof".into());
    }
    Ok(ProvenOperationTable {
        declaration_ordinal: *ordinal,
        original_name: expected.name.clone(),
        canonical_name: expected.name.clone(),
        initializer_elements: *elements,
        initializer_functions: *functions,
    })
}

struct OperationTableRecovery<'a> {
    ordinal: u32,
    name: &'a str,
    count: u32,
    matches: Vec<(SymbolId, u32, u32, u32, String)>,
}

impl<'a> Visit<'a> for OperationTableRecovery<'_> {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        let ordinal = self.ordinal;
        self.ordinal = self.ordinal.saturating_add(1);
        let (BindingPattern::BindingIdentifier(id), Some(init)) =
            (&declarator.id, declarator.init.as_ref())
        else {
            walk::walk_variable_declarator(self, declarator);
            return;
        };
        if id.name != self.name {
            walk::walk_variable_declarator(self, declarator);
            return;
        }
        let (elements, functions) = match init {
            Expression::ArrayExpression(array) => (
                array.elements.len() as u32,
                array
                    .elements
                    .iter()
                    .filter(|element| {
                        matches!(
                            element.as_expression(),
                            Some(
                                Expression::FunctionExpression(_)
                                    | Expression::ArrowFunctionExpression(_)
                            )
                        )
                    })
                    .count() as u32,
            ),
            Expression::ObjectExpression(object) => (
                object.properties.len() as u32,
                object
                    .properties
                    .iter()
                    .filter_map(|property| match property {
                        ObjectPropertyKind::ObjectProperty(property) => Some(&property.value),
                        _ => None,
                    })
                    .filter(|value| {
                        matches!(
                            value,
                            Expression::FunctionExpression(_)
                                | Expression::ArrowFunctionExpression(_)
                        )
                    })
                    .count() as u32,
            ),
            _ => {
                walk::walk_variable_declarator(self, declarator);
                return;
            }
        };
        if let Some(symbol) = id.symbol_id.get() {
            self.matches
                .push((symbol, ordinal, elements, functions, id.name.to_string()));
        }
        walk::walk_variable_declarator(self, declarator);
    }
}

/// Ensure the code-generation-stable operation-table ordinal still identifies
/// the one expected array of function values. This rejects shadowed/decoy
/// declarations instead of letting a last-match spelling scan choose one.
pub(super) fn validate_operation_table_declaration(
    source: &str,
    expected: &ProvenOperationTable,
) -> Result<(), DispatchPrepareError> {
    let allocator = Allocator::default();
    let program = parse(&allocator, source)?;
    let (scoping, _nodes) = crate::analysis::analyze(&program);
    let mut finder = OperationDeclarationFinder {
        ordinal: 0,
        target: expected.declaration_ordinal,
        matches: Vec::new(),
    };
    finder.visit_program(&program);
    if finder.matches.len() != 1 {
        return Err(DispatchPrepareError::TableIdentity(format!(
            "operation-table declaration ordinal {} is ambiguous or missing",
            expected.declaration_ordinal
        )));
    }
    let (symbol, elements, functions, name) = &finder.matches[0];
    if name != &expected.canonical_name
        || *elements != expected.initializer_elements
        || *functions != expected.initializer_functions
        || scoping.symbol_declarations(*symbol).count() != 1
    {
        return Err(DispatchPrepareError::TableIdentity(
            "operation-table declaration shape or identity changed after proof".into(),
        ));
    }
    Ok(())
}

struct OperationDeclarationFinder {
    ordinal: u32,
    target: u32,
    matches: Vec<(SymbolId, u32, u32, String)>,
}

impl<'a> Visit<'a> for OperationDeclarationFinder {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if self.ordinal == self.target {
            if let (BindingPattern::BindingIdentifier(id), Some(init)) =
                (&declarator.id, declarator.init.as_ref())
            {
                let (elements, functions) = match init {
                    Expression::ArrayExpression(array) => (
                        array.elements.len() as u32,
                        array
                            .elements
                            .iter()
                            .filter(|element| {
                                matches!(
                                    element.as_expression(),
                                    Some(
                                        Expression::FunctionExpression(_)
                                            | Expression::ArrowFunctionExpression(_)
                                    )
                                )
                            })
                            .count() as u32,
                    ),
                    Expression::ObjectExpression(object) => (
                        object.properties.len() as u32,
                        object
                            .properties
                            .iter()
                            .filter_map(|property| match property {
                                ObjectPropertyKind::ObjectProperty(property) => {
                                    Some(&property.value)
                                }
                                _ => None,
                            })
                            .filter(|value| {
                                matches!(
                                    value,
                                    Expression::FunctionExpression(_)
                                        | Expression::ArrowFunctionExpression(_)
                                )
                            })
                            .count() as u32,
                    ),
                    _ => (0, 0),
                };
                if let Some(symbol) = id.symbol_id.get() {
                    self.matches
                        .push((symbol, elements, functions, id.name.to_string()));
                }
            }
        }
        self.ordinal = self.ordinal.saturating_add(1);
        walk::walk_variable_declarator(self, declarator);
    }
}

pub(super) fn prove_immutable_binding(
    source: &str,
    target: SymbolId,
    label: &'static str,
) -> Result<(), DispatchPrepareError> {
    let allocator = Allocator::default();
    let program = parse(&allocator, source)?;
    let (scoping, _nodes) = crate::analysis::analyze(&program);
    let tracked = tracked_aliases(&program, &scoping, target);
    let mut scanner = MutationScanner {
        scoping: &scoping,
        root: target,
        targets: tracked.full,
        member_targets: tracked.member,
        label,
        violations: Vec::new(),
    };
    scanner.visit_program(&program);
    scanner
        .violations
        .into_iter()
        .next()
        .map_or(Ok(()), |violation| {
            Err(DispatchPrepareError::SourceJoin(violation))
        })
}

struct BytecodeFinder {
    target: SymbolId,
    bytes: Option<Vec<u8>>,
}

impl<'a> Visit<'a> for BytecodeFinder {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(identifier), Some(init)) =
            (&declarator.id, declarator.init.as_ref())
        {
            if identifier.symbol_id.get() == Some(self.target) {
                if let Expression::ArrayExpression(array) = init {
                    let mut bytes = Vec::with_capacity(array.elements.len());
                    let mut valid = true;
                    for element in &array.elements {
                        let Some(Expression::NumericLiteral(number)) = element.as_expression()
                        else {
                            valid = false;
                            break;
                        };
                        if !number.value.is_finite()
                            || number.value.fract() != 0.0
                            || number.value < 0.0
                            || number.value > 255.0
                        {
                            valid = false;
                            break;
                        }
                        bytes.push(number.value as u8);
                    }
                    if valid && self.bytes.is_none() {
                        self.bytes = Some(bytes);
                    }
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}

/// Reject writes to, or escapes of, the bytecode image or of any binding aliased
/// from it.
struct MutationScanner<'a> {
    scoping: &'a Scoping,
    root: SymbolId,
    targets: BTreeSet<SymbolId>,
    member_targets: BTreeSet<SymbolId>,
    label: &'static str,
    violations: Vec<String>,
}

impl<'a> MutationScanner<'a> {
    fn symbol(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        crate::analysis::identifier_symbol(identifier, self.scoping)
    }
    fn is_target(&self, identifier: &IdentifierReference<'_>) -> bool {
        self.symbol(identifier)
            .is_some_and(|symbol| self.targets.contains(&symbol))
    }
    fn is_member_target(&self, identifier: &IdentifierReference<'_>) -> bool {
        self.symbol(identifier)
            .is_some_and(|symbol| self.member_targets.contains(&symbol))
    }
    fn is_target_expression(&self, expression: &Expression<'_>) -> bool {
        matches!(expression, Expression::Identifier(identifier) if self.is_target(identifier))
    }
    fn roots_at_target(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::Identifier(identifier) => self.is_target(identifier),
            Expression::StaticMemberExpression(member) => self.roots_at_target(&member.object),
            Expression::ComputedMemberExpression(member) => self.roots_at_target(&member.object),
            _ => false,
        }
    }

    fn roots_at_any_target(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::Identifier(identifier) => {
                self.is_target(identifier) || self.is_member_target(identifier)
            }
            Expression::StaticMemberExpression(member) => self.roots_at_any_target(&member.object),
            Expression::ComputedMemberExpression(member) => {
                self.roots_at_any_target(&member.object)
            }
            _ => false,
        }
    }

    fn is_any_target_expression(&self, expression: &Expression<'_>) -> bool {
        matches!(expression, Expression::Identifier(identifier) if self.is_target(identifier) || self.is_member_target(identifier))
    }

    fn is_allowed_alias_assignment(&self, assignment: &AssignmentExpression<'_>) -> bool {
        if assignment.operator != AssignmentOperator::Assign {
            return false;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(left) = &assignment.left else {
            return false;
        };
        self.symbol(left) != Some(self.root)
            && self.is_target(left)
            && self.is_target_expression(&assignment.right)
    }
}

struct AliasCollector<'a> {
    scoping: &'a Scoping,
    edges: Vec<(SymbolId, SymbolId)>,
    member_edges: Vec<(SymbolId, SymbolId)>,
}

impl<'a> AliasCollector<'a> {
    fn symbol(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        crate::analysis::identifier_symbol(identifier, self.scoping)
    }

    fn member_root(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        match expression {
            Expression::StaticMemberExpression(member) => self.member_root(&member.object),
            Expression::ComputedMemberExpression(member) => self.member_root(&member.object),
            Expression::Identifier(identifier) => self.symbol(identifier),
            _ => None,
        }
    }
}

impl<'a, 'b> Visit<'b> for AliasCollector<'a> {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'b>) {
        if let BindingPattern::BindingIdentifier(left) = &declarator.id {
            if let Some(init) = declarator.init.as_ref() {
                let right = match init {
                    Expression::Identifier(right) => self.symbol(right),
                    _ => None,
                };
                if let (Some(left), Some(right)) = (left.symbol_id.get(), right) {
                    self.edges.push((left, right));
                }
                if let (Some(left), Some(right)) = (left.symbol_id.get(), self.member_root(init)) {
                    self.member_edges.push((left, right));
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'b>) {
        if assignment.operator == AssignmentOperator::Assign {
            if let (
                AssignmentTarget::AssignmentTargetIdentifier(left),
                Expression::Identifier(right),
            ) = (&assignment.left, &assignment.right)
            {
                if let (Some(left), Some(right)) = (self.symbol(left), self.symbol(right)) {
                    self.edges.push((left, right));
                }
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
}

struct TrackedAliases {
    full: BTreeSet<SymbolId>,
    member: BTreeSet<SymbolId>,
}

fn tracked_aliases(program: &Program<'_>, scoping: &Scoping, root: SymbolId) -> TrackedAliases {
    let mut collector = AliasCollector {
        scoping,
        edges: Vec::new(),
        member_edges: Vec::new(),
    };
    collector.visit_program(program);
    let mut targets = [root].into_iter().collect::<BTreeSet<_>>();
    let mut changed = true;
    let mut member = BTreeSet::new();
    while changed {
        changed = false;
        for (left, right) in &collector.edges {
            if targets.contains(right) {
                changed |= targets.insert(*left);
            }
            if member.contains(right) {
                changed |= member.insert(*left);
            }
        }
        for (left, right) in &collector.member_edges {
            if targets.contains(right) || member.contains(right) {
                changed |= member.insert(*left);
            }
        }
    }
    TrackedAliases {
        full: targets,
        member,
    }
}

impl<'a> Visit<'a> for MutationScanner<'a> {
    fn visit_new_expression(&mut self, call: &NewExpression<'a>) {
        if self.roots_at_any_target(&call.callee) {
            self.violations
                .push(format!("{} is used as a constructor callee", self.label));
        }
        walk::walk_new_expression(self, call);
    }

    fn visit_tagged_template_expression(&mut self, expression: &TaggedTemplateExpression<'a>) {
        if self.roots_at_any_target(&expression.tag) {
            self.violations
                .push(format!("{} is used as a template callee", self.label));
        }
        walk::walk_tagged_template_expression(self, expression);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let Some(init) = declarator.init.as_ref() {
            let simple_alias = matches!(
                (&declarator.id, init),
                (
                    BindingPattern::BindingIdentifier(_),
                    Expression::Identifier(_)
                )
            );
            if !simple_alias
                && !matches!(&declarator.id, BindingPattern::BindingIdentifier(_))
                && self.is_target_expression(init)
            {
                self.violations.push(format!(
                    "{} uses an unsupported destructuring alias",
                    self.label
                ));
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        match &assignment.left {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                if (self.is_target(identifier) || self.is_member_target(identifier))
                    && !self.is_allowed_alias_assignment(assignment)
                {
                    self.violations
                        .push(format!("{} binding is reassigned", self.label));
                }
            }
            AssignmentTarget::StaticMemberExpression(member) => {
                if self.roots_at_any_target(&member.object) {
                    self.violations
                        .push(format!("{} property is written", self.label));
                }
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                if self.roots_at_any_target(&member.object) {
                    self.violations
                        .push(format!("{} index is written", self.label));
                }
            }
            _ => {
                if self.is_target_expression(&assignment.right) {
                    self.violations.push(format!(
                        "{} uses an unsupported destructuring alias",
                        self.label
                    ));
                }
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        match &update.argument {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                if self.is_target(identifier) || self.is_member_target(identifier) {
                    self.violations
                        .push(format!("{} binding is updated", self.label));
                }
            }
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                if self.roots_at_any_target(&member.object) {
                    self.violations
                        .push(format!("{} property is updated", self.label));
                }
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                if self.roots_at_any_target(&member.object) {
                    self.violations
                        .push(format!("{} index is updated", self.label));
                }
            }
            _ => {}
        }
        walk::walk_update_expression(self, update);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.roots_at_any_target(&call.callee) {
            self.violations.push(format!(
                "{} is used as a call receiver or callee",
                self.label
            ));
        }
        for argument in call.arguments.iter().filter_map(Argument::as_expression) {
            if self.is_any_target_expression(argument) || self.roots_at_target(argument) {
                self.violations
                    .push(format!("{} escapes into a call argument", self.label));
            }
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        if statement.argument.as_ref().is_some_and(|argument| {
            self.is_any_target_expression(argument) || self.roots_at_target(argument)
        }) {
            self.violations
                .push(format!("{} escapes as a return value", self.label));
        }
        walk::walk_return_statement(self, statement);
    }
}

/// Prove the single consumer join: the selector produced by the proven dispatch
/// method invoked on a context is the selector indexing the proven operation
/// table, called with that same context.
pub(super) fn prove_selector_join(
    source: &str,
    operation_ordinal: u32,
    dispatch: &DispatchReadProof,
) -> Result<OperandSpan, DispatchPrepareError> {
    let allocator = Allocator::default();
    let program = parse(&allocator, source)?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let operation_symbol = operation_table_symbol(&program, &scoping, operation_ordinal)
        .ok_or_else(|| {
            DispatchPrepareError::SourceJoin(
                "proven operation table declaration was not found".into(),
            )
        })?;
    let mut finder = SelectorJoinFinder {
        scoping: &scoping,
        operation_symbol,
        method_name: &dispatch.method_name,
        method_calls: Vec::new(),
        joins: Vec::new(),
        functions: Vec::new(),
    };
    finder.visit_program(&program);
    let method_calls = finder.method_calls;
    let joins = finder.joins;
    if method_calls.len() != 1 {
        return Err(DispatchPrepareError::SourceJoin(
            "dispatch method invocation is absent or ambiguous".into(),
        ));
    }
    let (selector, receiver, method_function, consumer_symbol) = &method_calls[0];
    if joins.len() != 1 {
        return Err(DispatchPrepareError::SourceJoin(
            "operation-table selector call is absent or ambiguous".into(),
        ));
    }
    let join = &joins[0];
    if &join.function_span != method_function {
        return Err(DispatchPrepareError::SourceJoin(
            "dispatch method and operation-table call are not in one consumer function".into(),
        ));
    }
    if join.selector != *selector {
        return Err(DispatchPrepareError::SourceJoin(
            "selectors of the dispatch method and the operation-table call differ".into(),
        ));
    }
    if join.receiver != *receiver {
        return Err(DispatchPrepareError::SourceJoin(
            "operation-table call is not invoked on the same context as the dispatch method".into(),
        ));
    }
    if scoping.symbol_is_mutated(*selector) {
        return Err(DispatchPrepareError::SourceJoin(
            "selector value is reassigned between dispatch and handler invocation".into(),
        ));
    }
    prove_consumer_context_origin(
        &program,
        &scoping,
        &nodes,
        *consumer_symbol,
        dispatch.context_constructor,
    )?;
    Ok(join.function.clone())
}

struct JoinCall {
    selector: SymbolId,
    receiver: SymbolId,
    function: OperandSpan,
    function_span: OperandSpan,
}

struct SelectorJoinFinder<'a, 'b> {
    scoping: &'a Scoping,
    operation_symbol: SymbolId,
    method_name: &'b str,
    method_calls: Vec<(SymbolId, SymbolId, OperandSpan, SymbolId)>,
    joins: Vec<JoinCall>,
    functions: Vec<OperandSpan>,
}

impl<'a, 'b> SelectorJoinFinder<'a, 'b> {
    fn symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        crate::analysis::identifier_symbol(identifier, self.scoping)
    }

    fn identifier_symbol(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        crate::analysis::identifier_symbol(identifier, self.scoping)
    }

    /// The accepted consumer is intentionally tiny: an unconditional
    /// `for (;;)` body containing exactly `selector = context.next()`, one
    /// selector-sentinel return guard, and one `operations[selector](context)`
    /// call. This prevents a matching pair hidden under an arbitrary branch or
    /// after an early return from becoming execution authority.
    fn supported_consumer(
        &self,
        function: &Function<'a>,
    ) -> Option<(SymbolId, SymbolId, SymbolId)> {
        let body = function.body.as_deref()?;
        if body.statements.len() != 1 {
            return None;
        }
        let Statement::ForStatement(loop_stmt) = &body.statements[0] else {
            return None;
        };
        if loop_stmt.init.is_some() || loop_stmt.test.is_some() || loop_stmt.update.is_some() {
            return None;
        }
        let Statement::BlockStatement(loop_body) = &loop_stmt.body else {
            return None;
        };
        if loop_body.body.len() != 3 {
            return None;
        }
        let Statement::VariableDeclaration(declaration) = &loop_body.body[0] else {
            return None;
        };
        if declaration.declarations.len() != 1 {
            return None;
        }
        let declarator = &declaration.declarations[0];
        let BindingPattern::BindingIdentifier(selector_id) = &declarator.id else {
            return None;
        };
        let Some(Expression::CallExpression(next_call)) = declarator.init.as_ref() else {
            return None;
        };
        let Expression::StaticMemberExpression(next_member) = &next_call.callee else {
            return None;
        };
        if next_member.property.name != self.method_name || !next_call.arguments.is_empty() {
            return None;
        }
        let selector = selector_id.symbol_id.get()?;
        let receiver = self.symbol(&next_member.object)?;
        let consumer_symbol = function.id.as_ref()?.symbol_id.get()?;
        if !function.params.items.iter().any(|parameter| {
            matches!(
                &parameter.pattern,
                BindingPattern::BindingIdentifier(identifier)
                    if identifier.symbol_id.get() == Some(receiver)
            )
        }) {
            return None;
        }

        let Statement::IfStatement(guard) = &loop_body.body[1] else {
            return None;
        };
        if guard.alternate.is_some()
            || !matches!(guard.consequent, Statement::ReturnStatement(_))
            || !matches!(
                &guard.test,
                Expression::BinaryExpression(binary)
                    if (matches!(&binary.left, Expression::Identifier(id) if self.identifier_symbol(id) == Some(selector))
                        || matches!(&binary.right, Expression::Identifier(id) if self.identifier_symbol(id) == Some(selector)))
            )
        {
            return None;
        }

        let Statement::ExpressionStatement(call_statement) = &loop_body.body[2] else {
            return None;
        };
        let Expression::CallExpression(operation_call) = &call_statement.expression else {
            return None;
        };
        let Expression::ComputedMemberExpression(operation_member) = &operation_call.callee else {
            return None;
        };
        if self.symbol(&operation_member.object) != Some(self.operation_symbol)
            || self.symbol(&operation_member.expression) != Some(selector)
        {
            return None;
        }
        let arguments = operation_call
            .arguments
            .iter()
            .filter_map(Argument::as_expression)
            .collect::<Vec<_>>();
        if arguments.len() != 1 || self.symbol(arguments[0]) != Some(receiver) {
            return None;
        }
        Some((selector, receiver, consumer_symbol))
    }
}

impl<'a, 'b> Visit<'a> for SelectorJoinFinder<'a, 'b> {
    fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some((selector, receiver, consumer_symbol)) = self.supported_consumer(it) {
            let function_span = span(it.span);
            self.method_calls
                .push((selector, receiver, function_span.clone(), consumer_symbol));
            self.joins.push(JoinCall {
                selector,
                receiver,
                function: function_span.clone(),
                function_span,
            });
        }
        self.functions.push(span(it.span));
        walk::walk_function(self, it, flags);
        self.functions.pop();
    }
    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.functions.push(span(it.span));
        walk::walk_arrow_function_expression(self, it);
        self.functions.pop();
    }
}

/// Prove that every call to the selected consumer receives an instance of the
/// constructor whose prototype owns the installed dispatch method. This is a
/// deliberately small origin analysis: direct `new C(...)`, transparent local
/// bindings, and one bounded chain of function parameters are supported. An
/// object with a method of the right spelling is never enough.
pub(super) fn prove_consumer_context_origin<'a>(
    program: &Program<'a>,
    scoping: &Scoping,
    nodes: &AstNodes<'a>,
    consumer: SymbolId,
    constructor: SymbolId,
) -> Result<(), DispatchPrepareError> {
    let mut index = OriginIndex {
        scoping,
        nodes,
        consumer,
        variables: BTreeMap::new(),
        parameters: BTreeMap::new(),
        calls: BTreeMap::new(),
        consumer_calls: Vec::new(),
        consumer_references: Vec::new(),
        allowed_consumer_references: BTreeSet::new(),
        function_stack: Vec::new(),
    };
    index.visit_program(program);
    if index.consumer_calls.is_empty()
        || index
            .consumer_calls
            .iter()
            .any(|call| call.arguments.is_none())
        || index
            .consumer_references
            .iter()
            .any(|reference| !index.allowed_consumer_references.contains(reference))
    {
        return Err(DispatchPrepareError::SourceJoin(
            "dispatch consumer call is unknown, aliased, or escaping".into(),
        ));
    }
    if index.consumer_calls.len() != 1 {
        return Err(DispatchPrepareError::SourceJoin(
            "dispatch consumer has ambiguous call sites".into(),
        ));
    }
    let call = &index.consumer_calls[0];
    let arguments = call.arguments.as_ref().expect("checked above");
    if arguments.len() != 1
        || !index.prove_expression(
            arguments[0],
            call.function,
            constructor,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
        )
    {
        return Err(DispatchPrepareError::SourceJoin(
            "dispatch consumer context does not originate from its proven constructor".into(),
        ));
    }
    Ok(())
}

struct ConsumerCall {
    function: Option<SymbolId>,
    arguments: Option<Vec<NodeId>>,
}

struct OriginIndex<'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    consumer: SymbolId,
    variables: BTreeMap<SymbolId, (NodeId, Option<SymbolId>)>,
    parameters: BTreeMap<SymbolId, (SymbolId, usize)>,
    calls: BTreeMap<SymbolId, Vec<ConsumerCall>>,
    consumer_calls: Vec<ConsumerCall>,
    consumer_references: Vec<(u32, u32)>,
    allowed_consumer_references: BTreeSet<(u32, u32)>,
    function_stack: Vec<Option<SymbolId>>,
}

impl<'a> OriginIndex<'a> {
    fn identifier_symbol(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        crate::analysis::identifier_symbol(identifier, self.scoping)
    }

    fn current_function(&self) -> Option<SymbolId> {
        self.function_stack.last().copied().flatten()
    }

    fn prove_expression(
        &self,
        expression: NodeId,
        function: Option<SymbolId>,
        constructor: SymbolId,
        active_parameters: &mut BTreeSet<SymbolId>,
        active_variables: &mut BTreeSet<SymbolId>,
    ) -> bool {
        match expression {
            _ if matches!(self.nodes.kind(expression), AstKind::NewExpression(_)) => {
                let AstKind::NewExpression(new) = self.nodes.kind(expression) else {
                    unreachable!()
                };
                matches!(&new.callee, Expression::Identifier(identifier)
                    if self.identifier_symbol(identifier) == Some(constructor))
            }
            _ if matches!(self.nodes.kind(expression), AstKind::IdentifierReference(_)) => {
                let AstKind::IdentifierReference(identifier) = self.nodes.kind(expression) else {
                    unreachable!()
                };
                let Some(symbol) = self.identifier_symbol(identifier) else {
                    return false;
                };
                if let Some((owner, index)) = self.parameters.get(&symbol).copied() {
                    if !active_parameters.insert(symbol) {
                        return false;
                    }
                    let result = self.calls.get(&owner).is_some_and(|calls| {
                        calls.len() == 1
                            && calls[0]
                                .arguments
                                .as_ref()
                                .and_then(|arguments| arguments.get(index).copied())
                                .is_some_and(|argument| {
                                    self.prove_expression(
                                        argument,
                                        calls[0].function,
                                        constructor,
                                        active_parameters,
                                        active_variables,
                                    )
                                })
                    });
                    active_parameters.remove(&symbol);
                    return result;
                }
                let Some((initializer, owner)) = self.variables.get(&symbol).copied() else {
                    return false;
                };
                if !active_variables.insert(symbol) {
                    return false;
                }
                let result = self.prove_expression(
                    initializer,
                    owner.or(function),
                    constructor,
                    active_parameters,
                    active_variables,
                );
                active_variables.remove(&symbol);
                result
            }
            _ => false,
        }
    }
}

impl<'a> Visit<'a> for OriginIndex<'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        let symbol = function.id.as_ref().and_then(|id| id.symbol_id.get());
        if let Some(symbol) = symbol {
            for (index, parameter) in function.params.items.iter().enumerate() {
                if let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern {
                    if let Some(parameter_symbol) = identifier.symbol_id.get() {
                        self.parameters.insert(parameter_symbol, (symbol, index));
                    }
                }
            }
        }
        self.function_stack.push(symbol);
        walk::walk_function(self, function, flags);
        self.function_stack.pop();
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        self.function_stack.push(None);
        walk::walk_arrow_function_expression(self, function);
        self.function_stack.pop();
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(identifier), Some(initializer)) =
            (&declarator.id, declarator.init.as_ref())
        {
            if let (Some(symbol), Some(initializer)) =
                (identifier.symbol_id.get(), origin_node_id(initializer))
            {
                self.variables
                    .insert(symbol, (initializer, self.current_function()));
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let function = self.current_function();
        if let Expression::Identifier(callee) = &call.callee {
            if let Some(callee_symbol) = self.identifier_symbol(callee) {
                let arguments = call
                    .arguments
                    .iter()
                    .map(Argument::as_expression)
                    .map(|argument| argument.and_then(origin_node_id))
                    .collect::<Option<Vec<_>>>();
                let call_site = ConsumerCall {
                    function,
                    arguments,
                };
                self.calls.entry(callee_symbol).or_default().push(call_site);
                if callee_symbol == self.consumer {
                    self.consumer_calls.push(ConsumerCall {
                        function,
                        arguments: call
                            .arguments
                            .iter()
                            .map(Argument::as_expression)
                            .map(|argument| argument.and_then(origin_node_id))
                            .collect::<Option<Vec<_>>>(),
                    });
                    self.allowed_consumer_references
                        .insert((callee.span.start, callee.span.end));
                }
            }
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if self.identifier_symbol(identifier) == Some(self.consumer) {
            self.consumer_references
                .push((identifier.span.start, identifier.span.end));
        }
        walk::walk_identifier_reference(self, identifier);
    }
}

fn origin_node_id(expression: &Expression<'_>) -> Option<NodeId> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.node_id.get()),
        Expression::NewExpression(new) => Some(new.node_id.get()),
        _ => None,
    }
}

/// Resolve the SymbolId of the VariableDeclarator at `ordinal` in document
/// order — the identity `ProvenOperationTable` carries across code generation.
pub(super) fn operation_table_symbol(
    program: &Program<'_>,
    _scoping: &Scoping,
    ordinal: u32,
) -> Option<SymbolId> {
    let mut finder = OrdinalFinder {
        ordinal: 0,
        target: ordinal,
        symbol: None,
    };
    finder.visit_program(program);
    finder.symbol
}

struct OrdinalFinder {
    ordinal: u32,
    target: u32,
    symbol: Option<SymbolId>,
}

impl<'a> Visit<'a> for OrdinalFinder {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if self.ordinal == self.target {
            if let BindingPattern::BindingIdentifier(identifier) = &declarator.id {
                self.symbol = identifier.symbol_id.get();
            }
        }
        self.ordinal += 1;
        walk::walk_variable_declarator(self, declarator);
    }
}

#[cfg(test)]
mod dispatch_read_traversal_tests {
    use super::*;

    fn candidate_method_spans(source: &str) -> Vec<OperandSpan> {
        let allocator = Allocator::default();
        let parsed = crate::analysis::parse(&allocator, source);
        assert!(parsed.errors.is_empty(), "{source:?}: {:?}", parsed.errors);
        let (scoping, _) = crate::analysis::analyze(&parsed.program);
        let mut reader = DispatchReadFinder {
            scoping: &scoping,
            functions: Vec::new(),
            candidates: Vec::new(),
        };
        reader.visit_program(&parsed.program);
        reader
            .candidates
            .into_iter()
            .map(|candidate| candidate.method_span)
            .collect()
    }

    /// The shared recognizer is member-based so a visitor cannot lose the
    /// computed-member nodes that appear only as assignment, update, or
    /// destructuring targets.
    #[test]
    fn computed_member_targets_contribute_dispatch_candidates() {
        let cases = [
            (
                "assignment target",
                "var TABLE = [[0]]; var BYTES = [0]; var run = () => TABLE[this.S][BYTES[this.P++]] = 0;",
            ),
            (
                "update target",
                "var TABLE = [[0]]; var BYTES = [0]; var run = () => TABLE[this.S][BYTES[this.P++]]++;",
            ),
            (
                "array destructuring target",
                "var TABLE = [[0]]; var BYTES = [0]; var run = () => { [TABLE[this.S][BYTES[this.P++]]] = [0]; };",
            ),
            (
                "destructuring target with default",
                "var TABLE = [[0]]; var BYTES = [0]; var run = () => { [TABLE[this.S][BYTES[this.P++]] = 0] = []; };",
            ),
            (
                "for-of target",
                "var TABLE = [[0]]; var BYTES = [0]; var run = () => { for (TABLE[this.S][BYTES[this.P++]] of [0]); };",
            ),
        ];
        for (label, source) in cases {
            assert_eq!(candidate_method_spans(source).len(), 1, "{label}");
        }
    }

    /// Target traversal must still resolve Oxc symbols rather than spellings:
    /// a shadowed binding is a candidate, an unresolved binding is not, and a
    /// non-`this` cursor receiver is still strict-proof invalid.
    #[test]
    fn target_candidates_respect_symbol_resolution_and_this_receiver() {
        let shadowed = "var TABLE = [[0]]; var BYTES = [0]; var run = () => { var BYTES = [1]; return TABLE[this.S][BYTES[this.P++]] = 0; };";
        assert_eq!(candidate_method_spans(shadowed).len(), 1, "shadowed");

        let unresolved_table =
            "var BYTES = [0]; var run = () => MISSING[this.S][BYTES[this.P++]] = 0;";
        assert!(candidate_method_spans(unresolved_table).is_empty());

        let unresolved_bytecode =
            "var TABLE = [[0]]; var run = () => TABLE[this.S][MISSING[this.P++]] = 0;";
        assert!(candidate_method_spans(unresolved_bytecode).is_empty());

        let non_this_cursor = "var TABLE = [[0]]; var BYTES = [0]; var run = () => TABLE[this.S][BYTES[other.P++]] = 0;";
        assert!(candidate_method_spans(non_this_cursor).is_empty());
    }

    const VALID_METHOD: &str = r#"var TABLE = [[0]];
var BYTES = [0];
function Oz() {}
Object.defineProperty(Oz.prototype, "next", { value: function () {
  var record = TABLE[this.S][BYTES[this.P++]];
  this.S = record[0];
  return record[1];
} });
"#;

    /// A decoy arrow target is an independent candidate under the original
    /// `visit_computed_member_expression` traversal.  Preparation must keep
    /// rejecting the program as ambiguous instead of silently accepting the
    /// one valid method.
    #[test]
    fn prove_dispatch_read_preserves_arrow_target_uniqueness_error() {
        for (label, target) in [
            ("assignment", "TABLE[this.S][BYTES[this.P++]] = 0"),
            ("update", "TABLE[this.S][BYTES[this.P++]]++"),
            ("destructuring", "[TABLE[this.S][BYTES[this.P++]]] = [0]"),
        ] {
            let source = format!("{VALID_METHOD}var decoy = () => {{ {target}; }};");
            let error = prove_dispatch_read(&source)
                .err()
                .unwrap_or_else(|| panic!("{label} decoy must not be accepted"));
            assert!(
                error.to_string().contains("found 2"),
                "{label}: expected two candidates, got {error}"
            );
        }
    }
}
