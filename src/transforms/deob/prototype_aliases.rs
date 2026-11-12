//! Final-presentation cleanup for read-only VM prototype aliases.
//!
//! A kernel often installs a constructor's prototype into a short-lived local:
//!
//! ```js
//! var c = vmMemory.prototype;
//! Object.defineProperty(c, "slots", vmSlotDescriptor);
//! ```
//!
//! The alias carries no information of its own, so the readable form is the
//! direct call.  This pass rewrites exactly that shape and nothing else:
//!
//! * the constructor must already have a proven `Memory` or `Context` role in
//!   the RENAME artifacts (never a spelling-only match);
//! * the alias must be a `var`/`let`/`const` declarator whose initializer is
//!   `<constructor>.prototype` and whose every reference is the first argument
//!   of an unresolved-global `Object.defineProperty` call;
//! * the alias, the constructor prototype, and the binding must not be mutated,
//!   shadowed, redeclared, escaped, or read for any other purpose.
//!
//! Non-eligible aliases are retained byte-for-byte.  Multi-declarator
//! declarations keep their remaining declarators.  Function bodies, descriptor
//! keys, and the order of installation calls are untouched.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::Span;
use oxc_syntax::scope::ScopeId;

use crate::analysis;
use crate::diagnostics::{Diagnostic, Stage};
use crate::roles::VmRole;
use crate::transforms::deob::pipeline::BindingMapping;

/// Outcome of the narrow prototype-alias presentation cleanup.
pub(crate) struct CleanupReport {
    pub output: String,
    pub changed: usize,
    /// Document-order `VariableDeclarator` ordinals that were removed.  Later
    /// consumers resolve proven operation tables by ordinal, so the pipeline
    /// must shift those identities after a declarator disappears.
    pub removed_declarator_ordinals: Vec<u32>,
}

#[derive(Debug, Clone)]
struct TargetFunction {
    name: String,
    role: VmRole,
}

#[derive(Debug, Clone)]
struct AliasCandidate {
    symbol: SymbolId,
    constructor: SymbolId,
    constructor_name: String,
    role: VmRole,
    ordinal: u32,
    declaration: Span,
    declarator: Span,
    declarator_index: usize,
    declarator_spans: Vec<Span>,
}

struct DeclarationContext {
    span: Span,
    declarators: Vec<Span>,
    index: usize,
    is_statement: bool,
}

struct DeclarationRemoval {
    span: Span,
    declarators: Vec<Span>,
    removed: BTreeSet<usize>,
}

#[derive(Debug, Clone, Copy)]
struct AliasReference {
    span: Span,
    scope: ScopeId,
}

struct AliasProbe<'a> {
    scoping: &'a Scoping,
    targets: &'a BTreeMap<SymbolId, TargetFunction>,
    define_aliases: &'a BTreeSet<SymbolId>,
    statements: &'a BTreeSet<(u32, u32)>,
    declarations: Vec<DeclarationContext>,
    candidates: Vec<AliasCandidate>,
    references: BTreeMap<SymbolId, Vec<AliasReference>>,
    approved: BTreeSet<(SymbolId, u32, u32)>,
    mutated_constructors: BTreeSet<SymbolId>,
    next_declarator: u32,
}

/// Inline only aliases whose constructor role was proven by the pipeline.
pub(crate) fn inline(
    source: &str,
    binding_mappings: &[BindingMapping],
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<CleanupReport, String> {
    let targets = proven_constructor_names(binding_mappings);
    if targets.is_empty() {
        return Ok(unchanged(source));
    }

    let allocator = oxc_allocator::Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);

    let targets = collect_target_functions(&nodes, &targets, diagnostics);
    if targets.is_empty() {
        return Ok(unchanged(source));
    }

    let statements = statement_variable_declarations(&nodes);
    let define_aliases = define_property_aliases(&nodes, &scoping);
    let mut probe = AliasProbe {
        scoping: &scoping,
        targets: &targets,
        define_aliases: &define_aliases,
        statements: &statements,
        declarations: Vec::new(),
        candidates: Vec::new(),
        references: BTreeMap::new(),
        approved: BTreeSet::new(),
        mutated_constructors: BTreeSet::new(),
        next_declarator: 0,
    };
    probe.visit_program(&program);

    let mut edits = Vec::new();
    let mut removed_ordinals = Vec::new();
    let mut removals: BTreeMap<(u32, u32), DeclarationRemoval> = BTreeMap::new();
    let mut inlined = 0usize;

    for candidate in &probe.candidates {
        if !eligible(candidate, &probe, &scoping) {
            continue;
        }
        let Some(references) = probe.references.get(&candidate.symbol) else {
            continue;
        };
        for reference in references {
            edits.push(analysis::Edit::replace(
                reference.span,
                format!("{}.prototype", candidate.constructor_name),
            ));
        }
        removals
            .entry((candidate.declaration.start, candidate.declaration.end))
            .or_insert_with(|| DeclarationRemoval {
                span: candidate.declaration,
                declarators: candidate.declarator_spans.clone(),
                removed: BTreeSet::new(),
            })
            .removed
            .insert(candidate.declarator_index);
        removed_ordinals.push(candidate.ordinal);
        inlined += 1;
        diagnostics.push(
            Diagnostic::info(
                Stage::Extract,
                "PROTOTYPE_ALIAS_INLINED",
                format!(
                    "inlined read-only {} prototype alias `{}` -> direct `Object.defineProperty({}.prototype, ...)` target",
                    role_label(candidate.role),
                    candidate.symbol_name(&scoping),
                    candidate.constructor_name
                ),
            )
            .with_span(candidate.declarator.start, candidate.declarator.end),
        );
    }

    for removal in removals.into_values() {
        for span in
            declarator_removal_spans(source, removal.span, &removal.declarators, &removal.removed)
        {
            edits.push(analysis::Edit::delete(span));
        }
    }

    if inlined == 0 {
        return Ok(unchanged(source));
    }

    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        diagnostics.push(Diagnostic::warn(
            Stage::Extract,
            "PROTOTYPE_ALIAS_OVERLAP",
            analysis::describe_rejected_edits(&applied.rejected),
        ));
        // A partial rewrite could leave an alias declaration behind while some
        // references were already replaced.  Fail closed instead of emitting
        // that inconsistent presentation.
        return Err("prototype alias edits overlapped".into());
    }

    // Reparse and format so callers receive ordinary code, exactly as the other
    // final-presentation transforms do.
    let formatted_allocator = oxc_allocator::Allocator::default();
    let formatted = analysis::parse_program(&formatted_allocator, &applied.output)?;
    removed_ordinals.sort_unstable();
    Ok(CleanupReport {
        output: analysis::codegen(&formatted),
        changed: inlined,
        removed_declarator_ordinals: removed_ordinals,
    })
}

impl AliasCandidate {
    fn symbol_name(&self, scoping: &Scoping) -> String {
        scoping.symbol_name(self.symbol).to_string()
    }
}

fn eligible(candidate: &AliasCandidate, probe: &AliasProbe<'_>, scoping: &Scoping) -> bool {
    if probe.mutated_constructors.contains(&candidate.constructor) {
        return false;
    }
    // Replacing `alias` with the constructor spelling is only identity-preserving
    // while the constructor binding keeps the function object captured by the
    // alias.  A later write changes what `ConstructorName.prototype` means, and
    // a redeclaration changes which declaration introduces the binding.
    if scoping.symbol_is_mutated(candidate.constructor)
        || scoping.symbol_declarations(candidate.constructor).count() != 1
    {
        return false;
    }
    // A redeclared binding cannot be removed without changing which declaration
    // introduces it.
    if scoping.symbol_declarations(candidate.symbol).count() != 1 {
        return false;
    }
    if scoping.symbol_is_mutated(candidate.symbol) {
        return false;
    }
    let Some(references) = probe.references.get(&candidate.symbol) else {
        return false;
    };
    if references.is_empty() {
        return false;
    }
    let constructor_ident = scoping.symbol_ident(candidate.constructor);
    if constructor_ident.as_str() != candidate.constructor_name {
        return false;
    }
    references.iter().all(|reference| {
        // Every read must be the defineProperty target, and it must occur after
        // the declaration so removing hoisting/TDZ behaviour cannot change the
        // program.
        probe
            .approved
            .contains(&(candidate.symbol, reference.span.start, reference.span.end))
            && reference.span.start >= candidate.declarator.end
            // The textual replacement must resolve to the same constructor
            // binding at the reference site.  A nested parameter or local with
            // the constructor's spelling would otherwise capture the rewrite.
            && scoping.find_binding(reference.scope, constructor_ident)
                == Some(candidate.constructor)
    })
}

impl<'a> Visit<'a> for AliasProbe<'_> {
    fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'a>) {
        self.declarations.push(DeclarationContext {
            span: declaration.span,
            declarators: declaration
                .declarations
                .iter()
                .map(|item| item.span)
                .collect(),
            index: 0,
            is_statement: self
                .statements
                .contains(&(declaration.span.start, declaration.span.end)),
        });
        walk::walk_variable_declaration(self, declaration);
        self.declarations.pop();
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        let ordinal = self.next_declarator;
        self.next_declarator = self.next_declarator.saturating_add(1);

        let Some(context) = self.declarations.last_mut() else {
            walk::walk_variable_declarator(self, declarator);
            return;
        };
        let index = context.index;
        context.index += 1;
        let is_statement = context.is_statement;
        let declaration_span = context.span;
        let declarator_spans = context.declarators.clone();

        if is_statement {
            if let (Some(binding), Some(init)) =
                (simple_binding(&declarator.id), declarator.init.as_ref())
            {
                if let (Some(symbol), Some(constructor)) = (
                    binding.symbol_id.get(),
                    prototype_constructor(init, self.scoping),
                ) {
                    if let Some(target) = self.targets.get(&constructor) {
                        self.candidates.push(AliasCandidate {
                            symbol,
                            constructor,
                            constructor_name: target.name.clone(),
                            role: target.role,
                            ordinal,
                            declaration: declaration_span,
                            declarator: declarator.span,
                            declarator_index: index,
                            declarator_spans,
                        });
                    }
                }
            }
        }

        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_identifier_reference(&mut self, reference: &IdentifierReference<'a>) {
        if let Some((symbol, scope)) = identifier_reference_use(reference, self.scoping) {
            self.references
                .entry(symbol)
                .or_default()
                .push(AliasReference {
                    span: reference.span,
                    scope,
                });
        }
        walk::walk_identifier_reference(self, reference);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if is_define_property_call(call, self.scoping, self.define_aliases) {
            if let Some(Expression::Identifier(target)) =
                call.arguments.first().map(Argument::to_expression)
            {
                if let Some(symbol) = identifier_symbol(target, self.scoping) {
                    // Read-only target use is approved.
                    self.approved
                        .insert((symbol, target.span.start, target.span.end));
                }
            }
            // `Object.defineProperty(Constructor, "prototype", ...)` replaces
            // the prototype property and invalidates an earlier alias.
            if let (Some(Expression::Identifier(target)), Some(Expression::StringLiteral(key))) = (
                call.arguments.first().map(Argument::to_expression),
                call.arguments.get(1).map(Argument::to_expression),
            ) {
                if key.value == "prototype" {
                    if let Some(symbol) = identifier_symbol(target, self.scoping) {
                        self.mutated_constructors.insert(symbol);
                    }
                }
            }
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if let Some(symbol) = assignment_target_prototype(&assignment.left, self.scoping) {
            self.mutated_constructors.insert(symbol);
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if let Some(symbol) = simple_target_prototype(&update.argument, self.scoping) {
            self.mutated_constructors.insert(symbol);
        }
        walk::walk_update_expression(self, update);
    }

    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::Delete {
            if let Some(symbol) = expression_prototype_symbol(&unary.argument, self.scoping) {
                self.mutated_constructors.insert(symbol);
            }
        }
        walk::walk_unary_expression(self, unary);
    }
}

fn unchanged(source: &str) -> CleanupReport {
    CleanupReport {
        output: source.to_owned(),
        changed: 0,
        removed_declarator_ordinals: Vec::new(),
    }
}

/// Restrict the pass to constructors whose RENAME role is Memory or Context.
fn proven_constructor_names(mappings: &[BindingMapping]) -> BTreeMap<String, VmRole> {
    let mut targets = BTreeMap::new();
    for mapping in mappings {
        let Some(role) = mapping.role.as_deref() else {
            continue;
        };
        for candidate in [VmRole::Memory, VmRole::Context] {
            if candidate.canonical() == role {
                targets.insert(mapping.canonical_name.clone(), candidate);
            }
        }
    }
    targets
}

/// Resolve a canonical name to exactly one function binding.  A duplicated
/// spelling is ambiguous and is left untouched rather than guessed at.
fn collect_target_functions(
    nodes: &AstNodes<'_>,
    targets: &BTreeMap<String, VmRole>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<SymbolId, TargetFunction> {
    let mut by_name: BTreeMap<String, BTreeSet<SymbolId>> = BTreeMap::new();
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(identifier) = function.id.as_ref() else {
            continue;
        };
        if !targets.contains_key(identifier.name.as_str()) {
            continue;
        }
        if let Some(symbol) = identifier.symbol_id.get() {
            by_name
                .entry(identifier.name.to_string())
                .or_default()
                .insert(symbol);
        }
    }

    let mut resolved = BTreeMap::new();
    for (name, symbols) in by_name {
        if symbols.len() != 1 {
            diagnostics.push(Diagnostic::warn(
                Stage::Extract,
                "PROTOTYPE_ALIAS_DECLINED",
                format!(
                    "proven constructor name `{name}` resolves to {} function bindings; prototype aliases were retained",
                    symbols.len()
                ),
            ));
            continue;
        }
        let Some(symbol) = symbols.into_iter().next() else {
            continue;
        };
        let Some(role) = targets.get(&name).copied() else {
            continue;
        };
        resolved.insert(symbol, TargetFunction { name, role });
    }
    resolved
}

/// Only a declaration that is a statement can be removed with a text edit.
fn statement_variable_declarations(nodes: &AstNodes<'_>) -> BTreeSet<(u32, u32)> {
    let mut statements = BTreeSet::new();
    for node in nodes.iter() {
        let AstKind::VariableDeclaration(declaration) = node.kind() else {
            continue;
        };
        if matches!(
            nodes.parent_kind(node.id()),
            AstKind::Program(_)
                | AstKind::FunctionBody(_)
                | AstKind::BlockStatement(_)
                | AstKind::StaticBlock(_)
                | AstKind::SwitchCase(_)
        ) {
            statements.insert((declaration.span.start, declaration.span.end));
        }
    }
    statements
}

fn simple_binding<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a BindingIdentifier<'a>> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier),
        _ => None,
    }
}

fn identifier_symbol(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> Option<SymbolId> {
    crate::analysis::identifier_symbol(identifier, scoping)
}

/// Resolve a reference to its symbol and the lexical scope where the read
/// occurs, so a later textual replacement can be proved against that scope.
fn identifier_reference_use(
    identifier: &IdentifierReference<'_>,
    scoping: &Scoping,
) -> Option<(SymbolId, ScopeId)> {
    let reference_id = identifier.reference_id.get()?;
    let reference = scoping.get_reference(reference_id);
    Some((reference.symbol_id()?, reference.scope_id()))
}

/// `<Constructor>.prototype` (or `Constructor["prototype"]`) read.
fn prototype_constructor(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression {
        Expression::StaticMemberExpression(member)
            if !member.optional && member.property.name == "prototype" =>
        {
            expression_identifier_symbol(&member.object, scoping)
        }
        Expression::ComputedMemberExpression(member)
            if !member.optional && is_prototype_key(&member.expression) =>
        {
            expression_identifier_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn expression_prototype_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    prototype_constructor(expression, scoping)
}

fn assignment_target_prototype(
    target: &AssignmentTarget<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match target {
        AssignmentTarget::StaticMemberExpression(member)
            if !member.optional && member.property.name == "prototype" =>
        {
            expression_identifier_symbol(&member.object, scoping)
        }
        AssignmentTarget::ComputedMemberExpression(member)
            if !member.optional && is_prototype_key(&member.expression) =>
        {
            expression_identifier_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn simple_target_prototype(
    target: &SimpleAssignmentTarget<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match target {
        SimpleAssignmentTarget::StaticMemberExpression(member)
            if !member.optional && member.property.name == "prototype" =>
        {
            expression_identifier_symbol(&member.object, scoping)
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member)
            if !member.optional && is_prototype_key(&member.expression) =>
        {
            expression_identifier_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn is_prototype_key(expression: &Expression<'_>) -> bool {
    matches!(expression, Expression::StringLiteral(key) if key.value == "prototype")
}

fn expression_identifier_symbol(
    expression: &Expression<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression, scoping)
}

/// Global `Object.defineProperty(...)`, or a proven native-alias binding such as
/// `var Base = Object, define = Base.defineProperty; define(...)`. The real seed
/// retains its native catalogue (dynamic use), so the final presentation must
/// still recognize the retained `defineProperty` binding without inlining it.
fn is_define_property_call(
    call: &CallExpression<'_>,
    scoping: &Scoping,
    define_aliases: &BTreeSet<SymbolId>,
) -> bool {
    match &call.callee {
        Expression::StaticMemberExpression(member) => {
            if member.optional || member.property.name != "defineProperty" {
                return false;
            }
            let Expression::Identifier(object) = &member.object else {
                return false;
            };
            object.name == "Object"
                && object
                    .reference_id
                    .get()
                    .is_none_or(|reference| scoping.get_reference(reference).symbol_id().is_none())
        }
        Expression::Identifier(callee) => {
            identifier_symbol(callee, scoping).is_some_and(|symbol| {
                define_aliases.contains(&symbol) && !scoping.symbol_is_mutated(symbol)
            })
        }
        _ => false,
    }
}

/// Resolve `var Base = Object` and `var define = Base.defineProperty` chains.
/// Only proven, unshadowed global-`Object` receivers qualify.
fn define_property_aliases(nodes: &AstNodes<'_>, scoping: &Scoping) -> BTreeSet<SymbolId> {
    let mut object_aliases: BTreeSet<SymbolId> = BTreeSet::new();
    let mut define_aliases: BTreeSet<SymbolId> = BTreeSet::new();
    loop {
        let mut changed = false;
        for node in nodes.iter() {
            let AstKind::VariableDeclarator(declarator) = node.kind() else {
                continue;
            };
            let Some(binding) = simple_binding(&declarator.id) else {
                continue;
            };
            let Some(symbol) = binding.symbol_id.get() else {
                continue;
            };
            let Some(init) = declarator.init.as_ref() else {
                continue;
            };
            match init {
                Expression::Identifier(identifier) => {
                    let global_object = identifier.name == "Object"
                        && identifier.reference_id.get().is_none_or(|reference| {
                            scoping.get_reference(reference).symbol_id().is_none()
                        });
                    let is_object = global_object
                        || identifier_symbol(identifier, scoping)
                            .is_some_and(|source| object_aliases.contains(&source));
                    if is_object {
                        changed |= object_aliases.insert(symbol);
                    }
                }
                Expression::StaticMemberExpression(member)
                    if !member.optional && member.property.name == "defineProperty" =>
                {
                    let Expression::Identifier(object) = &member.object else {
                        continue;
                    };
                    let global_object = object.name == "Object"
                        && object.reference_id.get().is_none_or(|reference| {
                            scoping.get_reference(reference).symbol_id().is_none()
                        });
                    let is_object = global_object
                        || identifier_symbol(object, scoping)
                            .is_some_and(|source| object_aliases.contains(&source));
                    if is_object && !scoping.symbol_is_mutated(symbol) {
                        changed |= define_aliases.insert(symbol);
                    }
                }
                _ => {}
            }
        }
        if !changed {
            break;
        }
    }
    define_aliases
}

/// Mirror the CLEAN declaration-removal ranges: keep sibling declarators and
/// their separating commas intact.
fn declarator_removal_spans(
    source: &str,
    declaration: Span,
    declarators: &[Span],
    removed: &BTreeSet<usize>,
) -> Vec<Span> {
    if removed.len() == declarators.len() {
        return vec![include_trailing_semicolon(source, declaration)];
    }
    let mut spans = Vec::new();
    let mut index = 0;
    while index < declarators.len() {
        if !removed.contains(&index) {
            index += 1;
            continue;
        }
        let start = index;
        while index + 1 < declarators.len() && removed.contains(&(index + 1)) {
            index += 1;
        }
        let end = index;
        let span = if start == 0 {
            Span::new(declarators[start].start, declarators[end + 1].start)
        } else {
            Span::new(declarators[start - 1].end, declarators[end].end)
        };
        spans.push(span);
        index += 1;
    }
    spans
}

fn include_trailing_semicolon(source: &str, span: Span) -> Span {
    if source.as_bytes().get(span.end as usize) == Some(&b';') {
        Span::new(span.start, span.end.saturating_add(1))
    } else {
        span
    }
}

fn role_label(role: VmRole) -> &'static str {
    match role {
        VmRole::Memory => "memory",
        VmRole::Context => "context",
        _ => "VM",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transforms::deob::pipeline::BindingMapping;

    fn mapping(canonical: &str, role: VmRole) -> BindingMapping {
        BindingMapping {
            span: (0, 0),
            original_name: canonical.into(),
            canonical_name: canonical.into(),
            role: Some(role.canonical().into()),
        }
    }

    fn inline_with(source: &str, constructor: &str) -> CleanupReport {
        let mut diagnostics = Vec::new();
        inline(
            source,
            &[mapping(constructor, VmRole::Memory)],
            &mut diagnostics,
        )
        .expect("test source parses")
    }

    fn assert_retained(source: &str, constructor: &str) {
        let report = inline_with(source, constructor);
        assert_eq!(
            report.changed, 0,
            "unsafe alias was inlined:\n{}",
            report.output
        );
        assert_eq!(report.output, source, "declined alias was rewritten");
        assert!(report.removed_declarator_ordinals.is_empty());
    }

    #[test]
    fn direct_read_only_alias_is_still_inlined() {
        let source = "function vmMemory() {}\nvar alias = vmMemory.prototype;\nObject.defineProperty(alias, \"slot\", { value: 1 });\n";
        let report = inline_with(source, "vmMemory");
        assert_eq!(report.changed, 1, "{}", report.output);
        assert!(
            report
                .output
                .contains("Object.defineProperty(vmMemory.prototype, \"slot\""),
            "{}",
            report.output
        );
        assert!(!report.output.contains("var alias"), "{}", report.output);
    }

    #[test]
    fn retained_native_define_property_alias_still_proves_the_call() {
        // The real seed retains its native catalogue, so the final presentation
        // sees `define(alias, ...)` rather than a literal `Object.defineProperty`.
        let source = "function vmMemory() {}\nvar Base = Object;\nvar define = Base.defineProperty;\nvar alias = vmMemory.prototype;\ndefine(alias, \"slot\", { value: 1 });\n";
        let report = inline_with(source, "vmMemory");
        assert_eq!(report.changed, 1, "{}", report.output);
        assert!(
            report
                .output
                .contains("define(vmMemory.prototype, \"slot\""),
            "{}",
            report.output
        );
        assert!(!report.output.contains("var alias"), "{}", report.output);
    }

    #[test]
    fn mutated_native_define_property_alias_keeps_the_prototype_alias() {
        let source = "function vmMemory() {}\nvar Base = Object;\nvar define = Base.defineProperty;\ndefine = evil;\nvar alias = vmMemory.prototype;\ndefine(alias, \"slot\", { value: 1 });\n";
        assert_retained(source, "vmMemory");
    }

    #[test]
    fn constructor_reassignment_after_alias_capture_keeps_alias() {
        for constructor in ["Op", "vmMemory"] {
            let source = format!(
                "function {constructor}() {{}}\nvar alias = {constructor}.prototype;\n{constructor} = function () {{}};\nObject.defineProperty(alias, \"slot\", {{ value: 1 }});\n"
            );
            assert_retained(&source, constructor);
        }
    }

    #[test]
    fn constructor_redeclaration_keeps_alias() {
        for constructor in ["Op", "vmMemory"] {
            let source = format!(
                "function {constructor}() {{}}\nvar {constructor};\nvar alias = {constructor}.prototype;\nObject.defineProperty(alias, \"slot\", {{ value: 1 }});\n"
            );
            assert_retained(&source, constructor);
        }
    }

    #[test]
    fn constructor_name_shadowed_at_target_keeps_alias() {
        for constructor in ["Op", "vmMemory"] {
            let source = format!(
                "function {constructor}() {{}}\nvar alias = {constructor}.prototype;\nfunction install({constructor}) {{\n  Object.defineProperty(alias, \"slot\", {{ value: {constructor} }});\n}}\n"
            );
            assert_retained(&source, constructor);
        }
    }

    #[test]
    fn one_shadowed_reference_keeps_every_alias_reference() {
        for constructor in ["Op", "vmMemory"] {
            let source = format!(
                "function {constructor}() {{}}\nvar alias = {constructor}.prototype;\nObject.defineProperty(alias, \"slot\", {{ value: 1 }});\nfunction install({constructor}) {{\n  Object.defineProperty(alias, \"other\", {{ value: {constructor} }});\n}}\n"
            );
            let report = inline_with(&source, constructor);
            assert_eq!(report.changed, 0, "{}", report.output);
            assert_eq!(report.output, source);
        }
    }
}
