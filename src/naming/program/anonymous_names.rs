//! Make the naming plan's anonymous-function names visible in output.
//!
//! [`crate::naming::semantic_names::SemanticNamingPlan::anonymous_functions`] gives a proven
//! name to functions that have no editable binding (inline callbacks, arrows,
//! class and object methods). The package finders populate that list for real
//! captures; the corpus tests in `tests/naming_anonymous_names.rs` drive this
//! module with those entries end to end. The identifier rename cannot use the
//! names, so this module offers two explicit, independent opt-ins:
//!
//! * **name** (`--name-anonymous`): turn an anonymous `function (...) {...}`
//!   expression into the named function expression `function <name>(...)`.
//!   Applied only when the rewrite provably keeps program behavior apart from
//!   `Function.prototype.name` / `toString` of that function (see
//!   [`NameSkip`] for every refusal):
//!   - the node is a plain `FunctionExpression`, not an arrow, method,
//!     accessor, constructor or declaration, and has no name yet;
//!   - the name is a valid, non-reserved identifier;
//!   - nothing inside the function (parameters included) declares or
//!     references that spelling, so the new self-binding cannot capture or
//!     shadow an identifier the function uses; a direct `eval`, a `with`
//!     statement or `arguments.callee` inside it is refused too;
//!   - the expression is not the direct initializer of a binding, property,
//!     field, default value or `export default`, where the engine already
//!     infers a `.name` — unless that inferred name equals the new one.
//!
//!   After the rewrite the output is reparsed: it must parse with no new
//!   early error, and Oxc scoping must show the new self-binding with zero
//!   references. Otherwise the whole batch is refused.
//! * **annotate** (`--annotate-anonymous`): insert `/* <name> */` immediately
//!   before every anonymous function node, arrows and methods included. For a
//!   method or accessor the comment goes before the whole member (key
//!   included), so the method's `toString` source text is unchanged. Comments
//!   only; no code changes.
//!
//! The entries' spans address the source the plan was derived from. When the
//! edits apply to a different source (the output of the identifier rename or of
//! signal annotation, which neither add nor remove functions), each entry is
//! mapped by its pre-order position among all function nodes, with the node
//! kind checked; an entry that does not map is skipped. Nothing here executes
//! the JavaScript.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionExpression, AssignmentExpression, AssignmentOperator, AssignmentPattern, AssignmentTarget,
    AssignmentTargetPropertyIdentifier, AssignmentTargetWithDefault, BindingIdentifier,
    BindingPattern, CallExpression, ComputedMemberExpression, ExportDefaultDeclaration,
    ExportDefaultDeclarationKind, Expression, FormalParameter, Function, FunctionType,
    IdentifierReference, MethodDefinition, MethodDefinitionKind, ObjectProperty, Program,
    PropertyDefinition, PropertyKey, PropertyKind, StaticMemberExpression, VariableDeclarator,
    WithStatement,
};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::SymbolId;
use oxc_syntax::scope::ScopeFlags;
use serde::Serialize;

use crate::analysis;
use crate::naming::semantic_names::AnonymousFunctionName;
use crate::signals::schema::SourceSpan;

use super::ProgramTransformError;

/// Which anonymous-function edits to make. Both default to off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnonymousNameOptions {
    /// Give a provably safe anonymous function expression its proposed name.
    pub name: bool,
    /// Insert `/* <name> */` before every anonymous function node.
    pub annotate: bool,
}

impl AnonymousNameOptions {
    /// True when at least one edit kind is enabled.
    pub const fn any(self) -> bool {
        self.name || self.annotate
    }
}

/// The edit an outcome is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnonymousNameAction {
    /// `function (` -> `function <name>(`.
    Name,
    /// `/* <name> */` before the node.
    Annotate,
}

/// Why an entry was not applied. Serialized as a stable snake_case label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NameSkip {
    /// No function node has the entry's span in the plan source.
    NoFunctionAtSpan,
    /// The edited source has a different function layout than the plan source.
    UnmappedAfterRewrite,
    /// A second entry for a function node another entry already targets.
    DuplicateEntry,
    /// The proposed name is not a valid, non-reserved identifier.
    InvalidName,
    /// The node is an arrow function (only annotation applies).
    ArrowFunction,
    /// The node is a class or object method.
    Method,
    /// The node is a getter.
    Getter,
    /// The node is a setter.
    Setter,
    /// The node is a class constructor.
    Constructor,
    /// The node is a function declaration (or an anonymous `export default`).
    FunctionDeclaration,
    /// The function expression already has a name.
    AlreadyNamed,
    /// Something inside the function declares this spelling.
    NameBoundInside,
    /// Something inside the function references this spelling.
    NameReferencedInside,
    /// A direct `eval` inside the function could read the new binding.
    DirectEval,
    /// A `with` statement inside the function makes resolution dynamic.
    WithStatement,
    /// `arguments.callee` inside the function reads the function itself.
    ArgumentsCallee,
    /// The engine infers a different `.name` for this initializer.
    InferredNameDiffers,
    /// The engine infers a `.name` from a computed key that is not static.
    InferredNameUnknown,
    /// The parameter list was not where the parser placed it.
    UnsupportedShape,
    /// The batch failed reparse, early-error or scoping validation.
    RewriteRejected,
}

impl NameSkip {
    /// The stable machine label.
    pub fn label(self) -> &'static str {
        match self {
            Self::NoFunctionAtSpan => "no_function_at_span",
            Self::UnmappedAfterRewrite => "unmapped_after_rewrite",
            Self::DuplicateEntry => "duplicate_entry",
            Self::InvalidName => "invalid_name",
            Self::ArrowFunction => "arrow_function",
            Self::Method => "method",
            Self::Getter => "getter",
            Self::Setter => "setter",
            Self::Constructor => "constructor",
            Self::FunctionDeclaration => "function_declaration",
            Self::AlreadyNamed => "already_named",
            Self::NameBoundInside => "name_bound_inside",
            Self::NameReferencedInside => "name_referenced_inside",
            Self::DirectEval => "direct_eval",
            Self::WithStatement => "with_statement",
            Self::ArgumentsCallee => "arguments_callee",
            Self::InferredNameDiffers => "inferred_name_differs",
            Self::InferredNameUnknown => "inferred_name_unknown",
            Self::UnsupportedShape => "unsupported_shape",
            Self::RewriteRejected => "rewrite_rejected",
        }
    }
}

/// One entry's outcome for one action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnonymousNameOutcome {
    pub entry: AnonymousFunctionName,
    pub action: AnonymousNameAction,
    pub applied: bool,
    pub skip_reason: Option<NameSkip>,
    /// What the node is (`function_expression`, `arrow`, `method`, ...), when
    /// it was found.
    pub node_kind: Option<&'static str>,
    /// The `.name` the engine infers for the unedited expression, when it is
    /// a direct initializer with a static name.
    pub inferred_name: Option<String>,
    /// The node's span in the source the edits were applied to.
    pub edited_span: Option<SourceSpan>,
}

/// The edited source plus every outcome, in entry order (name before annotate).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnonymousNamesResult {
    pub options: AnonymousNameOptions,
    pub outcomes: Vec<AnonymousNameOutcome>,
    pub transformed_source: String,
    /// Function expressions that received a name.
    pub named: usize,
    /// Comments inserted.
    pub annotated: usize,
    /// Outcomes not applied.
    pub skipped: usize,
}

impl AnonymousNamesResult {
    /// Outcomes for one action.
    pub fn of(&self, action: AnonymousNameAction) -> impl Iterator<Item = &AnonymousNameOutcome> {
        self.outcomes
            .iter()
            .filter(move |outcome| outcome.action == action)
    }
}

/// Apply anonymous-function names to `source`, whose spans are those of the
/// plan source.
pub fn apply_anonymous_names(
    source: &str,
    entries: &[AnonymousFunctionName],
    options: AnonymousNameOptions,
) -> Result<AnonymousNamesResult, ProgramTransformError> {
    apply_anonymous_names_mapped(source, source, entries, options)
}

/// Apply anonymous-function names to `edited`, a rewrite of `plan_source`
/// (the source `entries` address) that adds and removes no function node.
pub fn apply_anonymous_names_mapped(
    plan_source: &str,
    edited: &str,
    entries: &[AnonymousFunctionName],
    options: AnonymousNameOptions,
) -> Result<AnonymousNamesResult, ProgramTransformError> {
    let mut result = AnonymousNamesResult {
        options,
        transformed_source: edited.to_string(),
        ..AnonymousNamesResult::default()
    };
    if !options.any() || entries.is_empty() {
        return Ok(result);
    }

    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, edited).map_err(ProgramTransformError::Parse)?;
    let mut index = FunctionIndex::default();
    index.visit_program(&program);

    // Entry -> position in the edited source's function order.
    let positions: Vec<Result<usize, NameSkip>> = if plan_source == edited {
        entries
            .iter()
            .map(|entry| index.position_of(entry.function).ok_or(NameSkip::NoFunctionAtSpan))
            .collect()
    } else {
        let plan_allocator = Allocator::default();
        let plan_program = analysis::parse_program(&plan_allocator, plan_source)
            .map_err(ProgramTransformError::Parse)?;
        let mut plan_index = FunctionIndex::default();
        plan_index.visit_program(&plan_program);
        entries
            .iter()
            .map(|entry| {
                let Some(position) = plan_index.position_of(entry.function) else {
                    return Err(NameSkip::NoFunctionAtSpan);
                };
                let same_layout = plan_index.nodes.len() == index.nodes.len()
                    && plan_index.nodes[position].kind == index.nodes[position].kind;
                if same_layout {
                    Ok(position)
                } else {
                    Err(NameSkip::UnmappedAfterRewrite)
                }
            })
            .collect()
    };

    // Resolve duplicates and collect the name candidates for the body scan.
    let mut seen = BTreeSet::new();
    let mut resolved: Vec<Result<usize, NameSkip>> = Vec::with_capacity(entries.len());
    for position in positions {
        resolved.push(match position {
            Ok(position) if !seen.insert(position) => Err(NameSkip::DuplicateEntry),
            other => other,
        });
    }

    // Static refusals for the name action, then the body scan for the rest.
    let mut name_checks: Vec<Result<usize, NameSkip>> = Vec::with_capacity(entries.len());
    for (entry, position) in entries.iter().zip(&resolved) {
        name_checks.push(match position {
            Err(skip) => Err(*skip),
            Ok(position) => static_name_check(&index.nodes[*position], &entry.proposed_name)
                .map(|()| *position),
        });
    }
    if options.name {
        let targets: BTreeMap<usize, BTreeSet<String>> = entries
            .iter()
            .zip(&name_checks)
            .filter_map(|(entry, check)| {
                check
                    .as_ref()
                    .ok()
                    .map(|position| (*position, entry.proposed_name.clone()))
            })
            .fold(BTreeMap::new(), |mut map, (position, name)| {
                map.entry(position).or_insert_with(BTreeSet::new).insert(name);
                map
            });
        let mut scan = BodyScan::new(&index, targets);
        scan.visit_program(&program);
        for (entry, check) in entries.iter().zip(name_checks.iter_mut()) {
            if let Ok(position) = check {
                if let Some(skip) = scan.conflict(*position, &entry.proposed_name) {
                    *check = Err(skip);
                }
            }
        }
    }

    // Build the insertions.
    let bytes = edited.as_bytes();
    let mut inserts: Vec<(u32, usize, String)> = Vec::new();
    let mut outcomes: Vec<AnonymousNameOutcome> = Vec::new();
    for (order, (entry, position)) in entries.iter().zip(&resolved).enumerate() {
        let node = position.as_ref().ok().map(|position| &index.nodes[*position]);
        let base = |action, skip: Option<NameSkip>| AnonymousNameOutcome {
            entry: entry.clone(),
            action,
            applied: skip.is_none(),
            skip_reason: skip,
            node_kind: node.map(|node| node.kind.label()),
            inferred_name: node.and_then(|node| match &node.inferred {
                Some(Inferred::Static(name)) => Some(name.clone()),
                _ => None,
            }),
            edited_span: node.map(|node| node.span),
        };
        if options.name {
            let check = &name_checks[order];
            match (check, node) {
                (Ok(_), Some(node)) => {
                    let at = node.params_start;
                    let glue = at > 0 && is_identifier_byte(bytes[at as usize - 1]);
                    let text = if glue {
                        format!(" {}", entry.proposed_name)
                    } else {
                        entry.proposed_name.clone()
                    };
                    inserts.push((at, order * 2 + 1, text));
                    outcomes.push(base(AnonymousNameAction::Name, None));
                }
                (Err(skip), _) => outcomes.push(base(AnonymousNameAction::Name, Some(*skip))),
                (Ok(_), None) => outcomes.push(base(
                    AnonymousNameAction::Name,
                    Some(NameSkip::NoFunctionAtSpan),
                )),
            }
        }
        if options.annotate {
            let skip = match position {
                Err(skip) => Some(*skip),
                Ok(_) if !analysis::is_identifier_name(&entry.proposed_name) => {
                    Some(NameSkip::InvalidName)
                }
                Ok(_) => None,
            };
            if let (None, Some(node)) = (skip, node) {
                let at = node.comment_anchor;
                let lead = if at > 0 && bytes[at as usize - 1] == b'/' {
                    " "
                } else {
                    ""
                };
                inserts.push((at, order * 2, format!("{lead}/* {} */ ", entry.proposed_name)));
            }
            outcomes.push(base(AnonymousNameAction::Annotate, skip));
        }
    }

    if !inserts.is_empty() {
        // Comments sort before the name at one offset; entry order otherwise.
        inserts.sort_by_key(|(at, order, _)| (*at, *order));
        let mut out = String::with_capacity(edited.len() + inserts.len() * 16);
        let mut cursor = 0usize;
        for (at, _, text) in &inserts {
            out.push_str(&edited[cursor..*at as usize]);
            out.push_str(text);
            cursor = *at as usize;
        }
        out.push_str(&edited[cursor..]);

        let named: Vec<(usize, String)> = entries
            .iter()
            .zip(&name_checks)
            .filter(|_| options.name)
            .filter_map(|(entry, check)| {
                check
                    .as_ref()
                    .ok()
                    .map(|position| (*position, entry.proposed_name.clone()))
            })
            .collect();
        if validate_output(&program, &out, index.nodes.len(), &named) {
            result.transformed_source = out;
        } else {
            for outcome in &mut outcomes {
                if outcome.applied {
                    outcome.applied = false;
                    outcome.skip_reason = Some(NameSkip::RewriteRejected);
                }
            }
        }
    }

    result.named = outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.action == AnonymousNameAction::Name)
        .count();
    result.annotated = outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.action == AnonymousNameAction::Annotate)
        .count();
    result.skipped = outcomes.iter().filter(|outcome| !outcome.applied).count();
    result.outcomes = outcomes;
    Ok(result)
}

/// The output parses, has no new early error, has the same function layout,
/// and every new self-binding exists with zero references.
fn validate_output(
    original: &Program,
    output: &str,
    function_count: usize,
    named: &[(usize, String)],
) -> bool {
    if super::validate_rewrite(original, output).is_err() {
        return false;
    }
    let allocator = Allocator::default();
    let Ok(program) = analysis::parse_program(&allocator, output) else {
        return false;
    };
    let (scoping, _nodes) = analysis::analyze(&program);
    let mut index = FunctionIndex::default();
    index.visit_program(&program);
    if index.nodes.len() != function_count {
        return false;
    }
    named.iter().all(|(position, name)| {
        let node = &index.nodes[*position];
        node.own_name.as_deref() == Some(name.as_str())
            && node
                .own_symbol
                .is_some_and(|symbol| scoping.get_resolved_reference_ids(symbol).is_empty())
    })
}

/// Refusals that need only the node itself.
fn static_name_check(node: &FunctionNode, name: &str) -> Result<(), NameSkip> {
    match node.kind {
        NodeKind::Arrow => return Err(NameSkip::ArrowFunction),
        NodeKind::Method => return Err(NameSkip::Method),
        NodeKind::Getter => return Err(NameSkip::Getter),
        NodeKind::Setter => return Err(NameSkip::Setter),
        NodeKind::Constructor => return Err(NameSkip::Constructor),
        NodeKind::Declaration => return Err(NameSkip::FunctionDeclaration),
        NodeKind::FunctionExpression => {}
    }
    if node.own_name.is_some() {
        return Err(NameSkip::AlreadyNamed);
    }
    if !super::rename::is_valid_name(name) {
        return Err(NameSkip::InvalidName);
    }
    match &node.inferred {
        None => {}
        Some(Inferred::Static(inferred)) if inferred == name => {}
        Some(Inferred::Static(_)) => return Err(NameSkip::InferredNameDiffers),
        Some(Inferred::Computed) => return Err(NameSkip::InferredNameUnknown),
    }
    Ok(())
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 0x80
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeKind {
    FunctionExpression,
    Declaration,
    Arrow,
    Method,
    Getter,
    Setter,
    Constructor,
}

impl NodeKind {
    fn label(self) -> &'static str {
        match self {
            Self::FunctionExpression => "function_expression",
            Self::Declaration => "function_declaration",
            Self::Arrow => "arrow",
            Self::Method => "method",
            Self::Getter => "getter",
            Self::Setter => "setter",
            Self::Constructor => "constructor",
        }
    }
}

/// The `.name` NamedEvaluation gives an anonymous function in this position.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Inferred {
    Static(String),
    /// From a computed key evaluated at runtime.
    Computed,
}

#[derive(Clone, Debug)]
struct FunctionNode {
    span: SourceSpan,
    kind: NodeKind,
    own_name: Option<String>,
    own_symbol: Option<SymbolId>,
    /// Offset of the parameter list's `(`.
    params_start: u32,
    /// Where a comment goes: the member start for a method, else the node.
    comment_anchor: u32,
    inferred: Option<Inferred>,
}

/// Every function and arrow node in pre-order, with its context.
#[derive(Default)]
struct FunctionIndex {
    nodes: Vec<FunctionNode>,
    by_span: BTreeMap<(u32, u32), usize>,
    /// Context recorded by a parent before its function child is visited.
    method_of: BTreeMap<(u32, u32), (NodeKind, u32)>,
    inferred_for: BTreeMap<(u32, u32), Inferred>,
}

impl FunctionIndex {
    fn position_of(&self, span: SourceSpan) -> Option<usize> {
        self.by_span.get(&(span.start, span.end)).copied()
    }

    fn infer(&mut self, expression: &Expression, name: Inferred) {
        let span = match expression {
            Expression::FunctionExpression(function) if function.id.is_none() => function.span,
            Expression::ArrowFunctionExpression(arrow) => arrow.span,
            _ => return,
        };
        self.inferred_for.insert((span.start, span.end), name);
    }

    fn infer_key(&mut self, key: &PropertyKey, computed: bool, value: &Expression) {
        let name = if let PropertyKey::PrivateIdentifier(private) = key {
            Some(format!("#{}", private.name))
        } else {
            key.static_name().map(|name| name.to_string())
        };
        match name {
            Some(name) => self.infer(value, Inferred::Static(name)),
            None if computed => self.infer(value, Inferred::Computed),
            None => {}
        }
    }

    fn push(&mut self, node: FunctionNode) {
        self.by_span
            .insert((node.span.start, node.span.end), self.nodes.len());
        self.nodes.push(node);
    }
}

fn binding_name(pattern: &BindingPattern) -> Option<String> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

impl<'a> Visit<'a> for FunctionIndex {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let key = (function.span.start, function.span.end);
        let (kind, comment_anchor) = match self.method_of.get(&key) {
            Some(context) => *context,
            None if function.r#type == FunctionType::FunctionExpression => {
                (NodeKind::FunctionExpression, function.span.start)
            }
            None => (NodeKind::Declaration, function.span.start),
        };
        self.push(FunctionNode {
            span: SourceSpan::new(function.span.start, function.span.end),
            kind,
            own_name: function.id.as_ref().map(|id| id.name.to_string()),
            own_symbol: function
                .id
                .as_ref()
                .and_then(|id| id.symbol_id.get()),
            params_start: function.params.span.start,
            comment_anchor,
            inferred: self.inferred_for.get(&key).cloned(),
        });
        walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        let key = (arrow.span.start, arrow.span.end);
        self.push(FunctionNode {
            span: SourceSpan::new(arrow.span.start, arrow.span.end),
            kind: NodeKind::Arrow,
            own_name: None,
            own_symbol: None,
            params_start: arrow.params.span.start,
            comment_anchor: arrow.span.start,
            inferred: self.inferred_for.get(&key).cloned(),
        });
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        let kind = match method.kind {
            MethodDefinitionKind::Constructor => NodeKind::Constructor,
            MethodDefinitionKind::Method => NodeKind::Method,
            MethodDefinitionKind::Get => NodeKind::Getter,
            MethodDefinitionKind::Set => NodeKind::Setter,
        };
        self.method_of.insert(
            (method.value.span.start, method.value.span.end),
            (kind, method.span.start),
        );
        walk::walk_method_definition(self, method);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        let kind = match property.kind {
            PropertyKind::Get => Some(NodeKind::Getter),
            PropertyKind::Set => Some(NodeKind::Setter),
            PropertyKind::Init if property.method => Some(NodeKind::Method),
            PropertyKind::Init => None,
        };
        if let (Some(kind), Expression::FunctionExpression(function)) = (kind, &property.value) {
            self.method_of.insert(
                (function.span.start, function.span.end),
                (kind, property.span.start),
            );
        } else if !property.shorthand {
            self.infer_key(&property.key, property.computed, &property.value);
        }
        walk::walk_object_property(self, property);
    }

    fn visit_property_definition(&mut self, property: &PropertyDefinition<'a>) {
        if let Some(value) = &property.value {
            self.infer_key(&property.key, property.computed, value);
        }
        walk::walk_property_definition(self, property);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let (Some(name), Some(init)) = (binding_name(&declarator.id), &declarator.init) {
            self.infer(init, Inferred::Static(name));
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        let named = matches!(
            assignment.operator,
            AssignmentOperator::Assign
                | AssignmentOperator::LogicalAnd
                | AssignmentOperator::LogicalOr
                | AssignmentOperator::LogicalNullish
        );
        if let (true, AssignmentTarget::AssignmentTargetIdentifier(identifier)) =
            (named, &assignment.left)
        {
            self.infer(&assignment.right, Inferred::Static(identifier.name.to_string()));
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_formal_parameter(&mut self, parameter: &FormalParameter<'a>) {
        if let (Some(name), Some(init)) = (binding_name(&parameter.pattern), &parameter.initializer)
        {
            self.infer(init, Inferred::Static(name));
        }
        walk::walk_formal_parameter(self, parameter);
    }

    fn visit_assignment_pattern(&mut self, pattern: &AssignmentPattern<'a>) {
        if let Some(name) = binding_name(&pattern.left) {
            self.infer(&pattern.right, Inferred::Static(name));
        }
        walk::walk_assignment_pattern(self, pattern);
    }

    fn visit_assignment_target_with_default(&mut self, target: &AssignmentTargetWithDefault<'a>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &target.binding {
            self.infer(&target.init, Inferred::Static(identifier.name.to_string()));
        }
        walk::walk_assignment_target_with_default(self, target);
    }

    fn visit_assignment_target_property_identifier(
        &mut self,
        target: &AssignmentTargetPropertyIdentifier<'a>,
    ) {
        if let Some(init) = &target.init {
            self.infer(init, Inferred::Static(target.binding.name.to_string()));
        }
        walk::walk_assignment_target_property_identifier(self, target);
    }

    fn visit_export_default_declaration(&mut self, export: &ExportDefaultDeclaration<'a>) {
        if let Some(expression) = export.declaration.as_expression() {
            self.infer(expression, Inferred::Static("default".to_string()));
        } else if let ExportDefaultDeclarationKind::FunctionDeclaration(function) =
            &export.declaration
        {
            // An anonymous `export default function () {}` is a declaration
            // whose `.name` is "default"; it is never named here.
            self.inferred_for.insert(
                (function.span.start, function.span.end),
                Inferred::Static("default".to_string()),
            );
        }
        walk::walk_export_default_declaration(self, export);
    }
}


/// Per target function: does anything inside use a candidate spelling?
struct BodyScan {
    /// Target function span -> position.
    spans: BTreeMap<(u32, u32), usize>,
    names: BTreeMap<usize, BTreeSet<String>>,
    /// Targets whose span contains the node being visited.
    active: Vec<usize>,
    bound: BTreeSet<(usize, String)>,
    referenced: BTreeSet<(usize, String)>,
    eval: BTreeSet<usize>,
    with: BTreeSet<usize>,
    callee: BTreeSet<usize>,
}

impl BodyScan {
    fn new(index: &FunctionIndex, names: BTreeMap<usize, BTreeSet<String>>) -> Self {
        Self {
            spans: names
                .keys()
                .map(|position| {
                    let span = index.nodes[*position].span;
                    ((span.start, span.end), *position)
                })
                .collect(),
            names,
            active: Vec::new(),
            bound: BTreeSet::new(),
            referenced: BTreeSet::new(),
            eval: BTreeSet::new(),
            with: BTreeSet::new(),
            callee: BTreeSet::new(),
        }
    }

    fn conflict(&self, position: usize, name: &str) -> Option<NameSkip> {
        let key = (position, name.to_string());
        if self.bound.contains(&key) {
            Some(NameSkip::NameBoundInside)
        } else if self.referenced.contains(&key) {
            Some(NameSkip::NameReferencedInside)
        } else if self.eval.contains(&position) {
            Some(NameSkip::DirectEval)
        } else if self.with.contains(&position) {
            Some(NameSkip::WithStatement)
        } else if self.callee.contains(&position) {
            Some(NameSkip::ArgumentsCallee)
        } else {
            None
        }
    }

    fn note(&mut self, name: &str, bound: bool) {
        for position in &self.active {
            if self.names[position].contains(name) {
                let key = (*position, name.to_string());
                if bound {
                    self.bound.insert(key);
                } else {
                    self.referenced.insert(key);
                }
            }
        }
    }

    fn enter(&mut self, start: u32, end: u32) -> bool {
        if let Some(position) = self.spans.get(&(start, end)).copied() {
            self.active.push(position);
            true
        } else {
            false
        }
    }
}

impl<'a> Visit<'a> for BodyScan {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let entered = self.enter(function.span.start, function.span.end);
        walk::walk_function(self, function, flags);
        if entered {
            self.active.pop();
        }
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if !self.active.is_empty() {
            self.note(identifier.name.as_str(), false);
        }
    }

    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        if !self.active.is_empty() {
            self.note(identifier.name.as_str(), true);
        }
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if matches!(&call.callee, Expression::Identifier(callee) if callee.name == "eval") {
            self.eval.extend(self.active.iter().copied());
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.with.extend(self.active.iter().copied());
        walk::walk_with_statement(self, statement);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if matches!(&member.object, Expression::Identifier(object) if object.name == "arguments")
            && member.property.name == "callee"
        {
            self.callee.extend(self.active.iter().copied());
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if matches!(&member.object, Expression::Identifier(object) if object.name == "arguments")
        {
            // Any computed read of `arguments` may be `arguments["callee"]`.
            let callee = match &member.expression {
                Expression::StringLiteral(literal) => literal.value == "callee",
                Expression::NumericLiteral(_) => false,
                _ => true,
            };
            if callee {
                self.callee.extend(self.active.iter().copied());
            }
        }
        walk::walk_computed_member_expression(self, member);
    }
}
