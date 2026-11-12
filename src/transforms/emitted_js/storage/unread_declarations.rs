//! The standalone declaration pass: unread declarations with pure,
//! non-throwing initializers go, keeping statement maps when it can; and the
//! public options, report and error types the cleanup entry points share.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;
use crate::diagnostics::StageStatus;
use crate::tracing::symbolic::{EmittedJavaScript, EmittedStatementMap, TraceNodeId};

/// Options for the standalone post-emission cleanup pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedCleanupOptions {
    /// Remove declarations whose bindings have no references and whose
    /// initializer is provably pure and non-throwing. Defaults to `true`.
    pub remove_unused_pure_declarations: bool,
}

impl Default for EmittedCleanupOptions {
    fn default() -> Self {
        Self {
            remove_unused_pure_declarations: true,
        }
    }
}

/// Validity of the caller's original statement map after cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementMapStatus {
    /// Statement indexes and byte coordinates are unchanged for the primary
    /// emitted function body, or no change affects that body.
    Preserved,
    /// Whole statements were removed, so the original map no longer describes
    /// the returned code.
    Invalidated,
    /// Whole statements were removed and the map was regenerated from the
    /// exact set of removed statement indexes.
    Regenerated,
}

impl StatementMapStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preserved => "preserved",
            Self::Invalidated => "invalidated",
            Self::Regenerated => "regenerated",
        }
    }
}

/// One source range changed by cleanup, in original input byte coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedRange {
    /// Original input byte range. A whole-statement range is replaced by `;`;
    /// a declarator range is deleted.
    pub span: (u32, u32),
    /// Number of declarators removed by this range.
    pub declarators: usize,
    /// Whether the entire declaration statement was removed.
    pub statement: bool,
    /// Binding names in this range. Names are report-only; they never decide a
    /// removal.
    pub bindings: Vec<String>,
}

/// Summary of one cleanup run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedCleanupReport {
    pub status: StageStatus,
    /// A `with` statement or identifier reference named `eval` was present, so
    /// no edit was applied.
    pub dynamic_scope: bool,
    /// Supported `var`/`let`/`const` declaration statements considered.
    pub eligible_declarations: usize,
    /// Whole declaration statements removed.
    pub removed_statements: usize,
    /// Individual declarators removed (including those in whole statements).
    pub removed_declarators: usize,
    /// Eligible declarations that still exist in some form in the output.
    pub retained_declarations: usize,
    pub removed: Vec<RemovedRange>,
    /// Zero-based indexes of whole statements removed from the first top-level
    /// function declaration body, in original order. This is the body shape
    /// produced by `emit_javascript`.
    pub removed_top_level_function_statements: Vec<usize>,
    pub statement_map_status: StatementMapStatus,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub notes: Vec<String>,
}

/// Cleaned source and its report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedCleanupResult {
    pub code: String,
    pub report: EmittedCleanupReport,
}

/// Cleaned emitted code with regenerated provenance maps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanedEmittedJavaScript {
    pub code: String,
    pub statement_map: Vec<EmittedStatementMap>,
    pub node_map: BTreeMap<TraceNodeId, usize>,
    pub object_parameters: BTreeMap<u64, String>,
    pub report: EmittedCleanupReport,
}

/// Parse or output-safety failure. Input is never executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmittedCleanupError {
    Parse(String),
    InvalidOutput(String),
}

impl fmt::Display for EmittedCleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "emitted cleanup could not parse source: {error}"),
            Self::InvalidOutput(error) => {
                write!(f, "emitted cleanup produced invalid output: {error}")
            }
        }
    }
}

impl std::error::Error for EmittedCleanupError {}

/// Run the default conservative cleanup over JavaScript source.
pub fn cleanup(source: &str) -> Result<EmittedCleanupResult, EmittedCleanupError> {
    cleanup_with_options(source, &EmittedCleanupOptions::default())
}

/// Run conservative cleanup with explicit options.
pub fn cleanup_with_options(
    source: &str,
    options: &EmittedCleanupOptions,
) -> Result<EmittedCleanupResult, EmittedCleanupError> {
    let mut report = EmittedCleanupReport {
        status: StageStatus::Unknown,
        dynamic_scope: false,
        eligible_declarations: 0,
        removed_statements: 0,
        removed_declarators: 0,
        retained_declarations: 0,
        removed: Vec::new(),
        removed_top_level_function_statements: Vec::new(),
        statement_map_status: StatementMapStatus::Preserved,
        input_bytes: source.len(),
        output_bytes: source.len(),
        notes: Vec::new(),
    };

    if !options.remove_unused_pure_declarations {
        report
            .notes
            .push("unused pure declaration removal is disabled".into());
        return Ok(EmittedCleanupResult {
            code: source.to_owned(),
            report,
        });
    }

    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, source).map_err(EmittedCleanupError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);

    let mut collector = Collector {
        scoping: &scoping,
        nodes: &nodes,
        primary_function: None,
        dynamic_scope: false,
        eligible_declarations: 0,
        parameter_symbols: BTreeSet::new(),
        declarations: Vec::new(),
    };
    collector.visit_program(&program);
    collector.refine_removability();

    report.eligible_declarations = collector.eligible_declarations;
    report.dynamic_scope = collector.dynamic_scope;

    if collector.dynamic_scope {
        report.status = StageStatus::Unsupported;
        report.retained_declarations = collector.eligible_declarations;
        report.notes.push(
            "with/eval can observe lexical bindings; the input was returned unchanged".into(),
        );
        return Ok(EmittedCleanupResult {
            code: source.to_owned(),
            report,
        });
    }

    let mut declarations = collector.declarations;
    declarations.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then(right.span.end.cmp(&left.span.end))
    });

    let edits = removal_edits(&declarations, &mut report);
    if edits.is_empty() {
        return Ok(EmittedCleanupResult {
            code: source.to_owned(),
            report,
        });
    }
    apply_removals(source, edits, report)
}

/// Edits removing the removable declarators of `declarations`, sorted
/// outermost first, each recorded in `report`.
fn removal_edits(
    declarations: &[DeclarationPlan],
    report: &mut EmittedCleanupReport,
) -> Vec<analysis::Edit> {
    let mut edits = Vec::new();
    let mut changed_ranges: Vec<(u32, u32)> = Vec::new();
    let mut removed_top_level = BTreeSet::new();

    for declaration in declarations {
        // A declaration in a nested function body can live inside a removed
        // initializer. Once the enclosing range is gone, its own edits would
        // overlap and must not be reported as separate removals.
        if changed_ranges
            .iter()
            .any(|(start, end)| declaration.span.start >= *start && declaration.span.end <= *end)
        {
            continue;
        }

        let removable = declaration
            .declarators
            .iter()
            .filter(|declarator| declarator.removable)
            .count();
        if removable == 0 {
            continue;
        }

        if removable == declaration.declarators.len() {
            let span = declaration.span;
            edits.push(analysis::Edit::replace(span, ";"));
            changed_ranges.push((span.start, span.end));
            report.removed_statements += 1;
            report.removed_declarators += removable;
            report.removed.push(RemovedRange {
                span: (span.start, span.end),
                declarators: removable,
                statement: true,
                bindings: declaration
                    .declarators
                    .iter()
                    .map(|declarator| declarator.binding.clone())
                    .collect(),
            });
            if let Some(index) = declaration.root_body_index {
                removed_top_level.insert(index);
            }
            continue;
        }

        for group in removable_groups(declaration) {
            let span = group.span;
            edits.push(analysis::Edit::replace(span, group.replacement.clone()));
            changed_ranges.push((span.start, span.end));
            report.removed_declarators += group.indexes.len();
            report.removed.push(RemovedRange {
                span: (span.start, span.end),
                declarators: group.indexes.len(),
                statement: false,
                bindings: group
                    .indexes
                    .iter()
                    .map(|index| declaration.declarators[*index].binding.clone())
                    .collect(),
            });
        }
    }

    report.retained_declarations = report
        .eligible_declarations
        .saturating_sub(report.removed_statements);
    report.removed_top_level_function_statements = removed_top_level.into_iter().collect();
    if !report.removed_top_level_function_statements.is_empty() {
        report.statement_map_status = StatementMapStatus::Invalidated;
    }
    edits
}

/// Apply `edits` and re-parse the result before reporting it found.
fn apply_removals(
    source: &str,
    edits: Vec<analysis::Edit>,
    mut report: EmittedCleanupReport,
) -> Result<EmittedCleanupResult, EmittedCleanupError> {
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err(EmittedCleanupError::InvalidOutput(
            analysis::describe_rejected_edits(&applied.rejected),
        ));
    }

    // Re-parse the edited text before returning it. The pass never executes
    // JavaScript, and a malformed splice must fail closed.
    let output_allocator = Allocator::default();
    analysis::parse_program(&output_allocator, &applied.output)
        .map_err(EmittedCleanupError::InvalidOutput)?;

    report.status = StageStatus::Found;
    report.output_bytes = applied.output.len();
    Ok(EmittedCleanupResult {
        code: applied.output,
        report,
    })
}

/// Clean emitted JavaScript and regenerate the statement/node maps that the
/// cleanup can prove.
pub fn cleanup_emitted_javascript(
    emitted: &EmittedJavaScript,
) -> Result<CleanedEmittedJavaScript, EmittedCleanupError> {
    cleanup_emitted_javascript_with_options(emitted, &EmittedCleanupOptions::default())
}

/// Clean emitted JavaScript with explicit options and regenerate maps.
pub fn cleanup_emitted_javascript_with_options(
    emitted: &EmittedJavaScript,
    options: &EmittedCleanupOptions,
) -> Result<CleanedEmittedJavaScript, EmittedCleanupError> {
    let mut result = cleanup_with_options(&emitted.code, options)?;
    let removed: BTreeSet<usize> = result
        .report
        .removed_top_level_function_statements
        .iter()
        .copied()
        .collect();

    let statement_map = emitted
        .statement_map
        .iter()
        .filter(|entry| !removed.contains(&entry.emitted_statement))
        .map(|entry| EmittedStatementMap {
            trace_statement: entry.trace_statement,
            // Whole declarations become empty statements, so the output AST
            // keeps the original statement indexes.
            emitted_statement: entry.emitted_statement,
        })
        .collect::<Vec<_>>();
    let node_map = emitted
        .node_map
        .iter()
        .filter(|(_, statement)| !removed.contains(statement))
        .map(|(node, statement)| (*node, *statement))
        .collect::<BTreeMap<_, _>>();

    result.report.statement_map_status = if removed.is_empty() {
        StatementMapStatus::Preserved
    } else {
        StatementMapStatus::Regenerated
    };

    Ok(CleanedEmittedJavaScript {
        code: result.code,
        statement_map,
        node_map,
        object_parameters: emitted.object_parameters.clone(),
        report: result.report,
    })
}

struct Collector<'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    primary_function: Option<Span>,
    dynamic_scope: bool,
    eligible_declarations: usize,
    parameter_symbols: BTreeSet<SymbolId>,
    declarations: Vec<DeclarationPlan>,
}

#[derive(Debug, Clone)]
struct DeclarationPlan {
    span: Span,
    declarators: Vec<DeclaratorPlan>,
    /// Present only when this is a direct statement of the first top-level
    /// function declaration body (the emitter's statement-map target).
    root_body_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct DeclaratorPlan {
    span: Span,
    binding: String,
    symbol: Option<SymbolId>,
    removable: bool,
}

#[derive(Debug, Clone)]
struct RemovableGroup {
    span: Span,
    replacement: String,
    indexes: Vec<usize>,
}

impl<'a> Collector<'a> {
    fn collect_body(&mut self, statements: &[Statement<'a>], primary_body: bool) {
        for (index, statement) in statements.iter().enumerate() {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            if declaration.declare
                || !matches!(
                    declaration.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
            {
                continue;
            }
            self.eligible_declarations += 1;

            let declarators = declaration
                .declarations
                .iter()
                .map(|declarator| DeclaratorPlan {
                    span: declarator.span,
                    binding: binding_name(declarator),
                    symbol: binding_symbol(declarator),
                    removable: self.declarator_is_pure(declarator),
                })
                .collect::<Vec<_>>();
            if declarators.iter().any(|declarator| declarator.removable) {
                self.declarations.push(DeclarationPlan {
                    span: statement.span(),
                    declarators,
                    root_body_index: primary_body.then_some(index),
                });
            }
        }
    }

    fn declarator_is_pure(&self, declarator: &VariableDeclarator<'a>) -> bool {
        let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
            // Array/object destructuring invokes iteration, property reads, and
            // default initializers. Retain the whole declaration.
            return false;
        };
        let Some(symbol) = binding.symbol_id.get() else {
            return false;
        };
        // Oxc's reference graph does not model the sloppy-mode alias between a
        // simple parameter and `arguments[index]`. Keep every var declarator
        // whose binding has a parameter role, including conservative cases
        // involving defaults, destructuring, rest, and duplicate parameters.
        if self.parameter_symbols.contains(&symbol) {
            return false;
        }
        match &declarator.init {
            None => true,
            Some(initializer) => super::super::purity::expression_is_pure(initializer),
        }
    }

    fn collect_parameter_symbols(&mut self, parameters: &FormalParameters<'a>) {
        for pattern in parameters.iter_bindings() {
            for binding in pattern.get_binding_identifiers() {
                if let Some(symbol) = binding.symbol_id.get() {
                    self.parameter_symbols.insert(symbol);
                }
            }
        }
    }

    /// References from a removable initializer must not keep an otherwise
    /// dead binding alive. Recompute this to a fixed point so removing an
    /// unused closure can expose an earlier dead binding without requiring a
    /// second public cleanup call.
    fn refine_removability(&mut self) {
        loop {
            let removable_spans = self
                .declarations
                .iter()
                .flat_map(|declaration| declaration.declarators.iter())
                .filter(|declarator| declarator.removable)
                .map(|declarator| declarator.span)
                .collect::<Vec<_>>();
            let mut changed = false;
            for declaration in &mut self.declarations {
                for declarator in &mut declaration.declarators {
                    if !declarator.removable {
                        continue;
                    }
                    let Some(symbol) = declarator.symbol else {
                        declarator.removable = false;
                        changed = true;
                        continue;
                    };
                    let all_references_are_removable = self
                        .scoping
                        .get_resolved_references(symbol)
                        .all(|reference| {
                            let reference_span =
                                self.nodes.get_node(reference.node_id()).kind().span();
                            removable_spans.iter().any(|candidate| {
                                reference_span.start >= candidate.start
                                    && reference_span.end <= candidate.end
                            })
                        });
                    if !all_references_are_removable {
                        declarator.removable = false;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }
}

impl<'a> Visit<'a> for Collector<'a> {
    fn visit_program(&mut self, program: &Program<'a>) {
        self.primary_function = program.body.iter().find_map(|statement| match statement {
            Statement::FunctionDeclaration(function) => Some(function.span),
            _ => None,
        });
        // Program-level `var` declarations are global properties and
        // program-level lexical declarations are visible to later scripts, so
        // they are never candidates. Emitted code is function-scoped.
        walk::walk_program(self, program);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.collect_parameter_symbols(&function.params);
        if let Some(body) = &function.body {
            let primary = self.primary_function == Some(function.span);
            self.collect_body(&body.statements, primary);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.collect_parameter_symbols(&arrow.params);
        self.collect_body(&arrow.body.statements, false);
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        // Covers direct `eval(...)`, `(0, eval)(...)`, aliased-looking reads,
        // and even a non-call read of a binding called `eval`. Over-approximating
        // here is intentional: the pass returns the input unchanged.
        if identifier.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}

fn binding_name(declarator: &VariableDeclarator<'_>) -> String {
    match &declarator.id {
        BindingPattern::BindingIdentifier(binding) => binding.name.to_string(),
        _ => String::new(),
    }
}

fn binding_symbol(declarator: &VariableDeclarator<'_>) -> Option<SymbolId> {
    match &declarator.id {
        BindingPattern::BindingIdentifier(binding) => binding.symbol_id.get(),
        _ => None,
    }
}

/// Build disjoint deletion ranges for the removable declarators of a
/// declaration that also has surviving declarators. The first surviving
/// declarator keeps the leading keyword; the last surviving declarator keeps
/// the statement semicolon.
fn removable_groups(declaration: &DeclarationPlan) -> Vec<RemovableGroup> {
    let len = declaration.declarators.len();
    let mut groups = Vec::new();
    let mut index = 0;
    while index < len {
        if !declaration.declarators[index].removable {
            index += 1;
            continue;
        }
        let first = index;
        while index + 1 < len && declaration.declarators[index + 1].removable {
            index += 1;
        }
        let last = index;
        let (span, replacement) = if first == 0 {
            // Not all declarators are removable, so `last + 1` exists.
            (
                Span::new(
                    declaration.declarators[0].span.start,
                    declaration.declarators[last + 1].span.start,
                ),
                String::new(),
            )
        } else {
            let start = declaration.declarators[first - 1].span.end;
            let end = if last + 1 < len {
                declaration.declarators[last + 1].span.start
            } else {
                declaration.declarators[last].span.end
            };
            let replacement = if last + 1 < len {
                // The deleted range contains both separators. Put one comma
                // back so the previous and next surviving declarators remain
                // one valid declaration.
                ",".to_owned()
            } else {
                String::new()
            };
            (Span::new(start, end), replacement)
        };
        groups.push(RemovableGroup {
            span,
            replacement,
            indexes: (first..=last).collect(),
        });
        index += 1;
    }
    groups
}
