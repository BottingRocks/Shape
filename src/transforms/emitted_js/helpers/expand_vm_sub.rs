//! Expand linear VM subroutines at their calls, with hygienic local bindings.
//!
//! Arguments are captured left-to-right before any helper statement executes.
//! This is essential for mutable `$KEY` arguments, repeated parameters, and
//! unused/extra arguments with effects. Statement sites receive ordinary local
//! declarations and statements. Embedded calls use a sequence at the original
//! evaluation point, with fresh temporaries declared in the containing function.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, ReferenceId, ScopeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use super::super::index::SourceNames;
use crate::analysis;

pub(crate) struct Expanded {
    pub code: String,
    pub calls: usize,
    pub removed: usize,
    pub remaining: usize,
}

pub(crate) fn expand_vm_subs(source: &str) -> Result<Expanded, String> {
    let mut result = Expanded {
        code: source.into(),
        calls: 0,
        removed: 0,
        remaining: 0,
    };
    // Leaf-first expansion also handles helpers calling other helpers and
    // nested call expressions. Recursive/unsupported helpers stop on no progress.
    for _ in 0..32 {
        let next = expand_round(&result.code)?;
        result.calls += next.calls;
        result.removed += next.removed;
        result.code = next.code;
        result.remaining = next.remaining;
        if next.remaining == 0 || (next.calls == 0 && next.removed == 0) {
            break;
        }
    }
    Ok(result)
}

fn expand_round(source: &str) -> Result<Expanded, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut catalogue = Catalogue {
        scoping: &scoping,
        nodes: &nodes,
        helpers: BTreeMap::new(),
        calls: Vec::new(),
        sites: BTreeMap::new(),
        owner: None,
        dynamic: false,
    };
    catalogue.visit_program(&program);
    let mut result = Expanded {
        code: source.into(),
        calls: 0,
        removed: 0,
        remaining: catalogue.helpers.len(),
    };
    if catalogue.dynamic || catalogue.helpers.is_empty() {
        return Ok(result);
    }
    let names = SourceNames::new(source, "$SSA_VM_");
    let mut serial = 0;
    let mut fresh = || loop {
        let name = format!("$SSA_VM_{serial}");
        serial += 1;
        if !names.contains(&name) {
            break name;
        }
    };
    let mut edits = Vec::new();
    let mut selected = Vec::new();
    let mut replaced = BTreeSet::<ReferenceId>::new();
    let mut deleted = BTreeSet::new();
    let mut declarations = BTreeMap::<u32, Vec<String>>::new();
    delete_unreferenced_helpers(
        &catalogue.helpers,
        &scoping,
        &mut selected,
        &mut edits,
        &mut deleted,
    );
    catalogue
        .calls
        .sort_by_key(|call| call.span.end - call.span.start);
    for call in &catalogue.calls {
        let Some((plan, owner, site, affected)) =
            expandable_call(&catalogue, &scoping, call, &deleted, &selected)
        else {
            continue;
        };
        let mut renames = BTreeMap::new();
        for symbol in &plan.params {
            renames.insert(*symbol, fresh());
        }
        for local in &plan.locals {
            renames.entry(local.symbol).or_insert_with(&mut fresh);
        }
        let (mut prefix, mut sequence, returned) = expansion_parts(source, plan, call, &renames);
        let replacement = if let Some(site) = site {
            if site.discard {
                if plan.returned.is_some() {
                    prefix.push_str(&format!("({returned});\n"));
                }
            } else {
                prefix.push_str(&source[site.span.start as usize..call.span.start as usize]);
                prefix.push('(');
                prefix.push_str(&returned);
                prefix.push(')');
                prefix.push_str(&source[call.span.end as usize..site.span.end as usize]);
            }
            prefix
        } else {
            declarations
                .entry(owner.insert)
                .or_default()
                .extend(renames.values().cloned());
            sequence.push(format!("({returned})"));
            // Force GetValue even for a zero-argument member-returning helper.
            if sequence.len() == 1 {
                format!("(0, {})", sequence[0])
            } else {
                format!("({})", sequence.join(", "))
            }
        };
        edits.push(analysis::Edit::replace(affected, replacement));
        selected.push(affected);
        replaced.insert(call.reference);
        result.calls += 1;
    }
    delete_replaced_helpers(
        &catalogue.helpers,
        &scoping,
        &replaced,
        &mut selected,
        &mut edits,
        &mut deleted,
    );
    result.removed = deleted.len();
    result.remaining -= result.removed;
    for (at, names) in declarations {
        edits.push(analysis::Edit::replace(
            Span::new(at, at),
            format!("\nlet {};\n", names.join(", ")),
        ));
    }
    if edits.is_empty() {
        return Ok(result);
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("VM helper expansion edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("VM helper expansion: {error}"))?;
    result.code = applied.output;
    Ok(result)
}

/// Delete removable helpers that nothing references.
fn delete_unreferenced_helpers(
    helpers: &BTreeMap<SymbolId, Helper>,
    scoping: &Scoping,
    selected: &mut Vec<Span>,
    edits: &mut Vec<analysis::Edit>,
    deleted: &mut BTreeSet<SymbolId>,
) {
    for (symbol, helper) in helpers {
        if helper.removable
            && scoping.get_resolved_references(*symbol).next().is_none()
            && !selected.iter().any(|span| overlaps(*span, helper.span))
        {
            selected.push(helper.span);
            edits.push(analysis::Edit::delete(helper.span));
            deleted.insert(*symbol);
        }
    }
}

/// The helper plan, owning function, statement site, and replaced span of a
/// call this round can expand, or `None` when the call must stay.
fn expandable_call<'c>(
    catalogue: &'c Catalogue<'_, '_>,
    scoping: &Scoping,
    call: &Call,
    deleted: &BTreeSet<SymbolId>,
    selected: &[Span],
) -> Option<(&'c Plan, Owner, Option<&'c Site>, Span)> {
    let helper = catalogue.helpers.get(&call.symbol)?;
    let plan = helper.plan.as_ref()?;
    if call.spread
        || call.optional
        || deleted.contains(&call.symbol)
        || scoping
            .get_resolved_references(call.symbol)
            .any(|r| r.is_write())
        || scoping.scope_flags(call.scope).is_strict_mode() != plan.strict
        || plan
            .occurrences
            .iter()
            .any(|o| o.symbol.is_some_and(|s| catalogue.helpers.contains_key(&s)))
    {
        return None;
    }
    let owner = call.owner?;
    if call.span.start < owner.body.start {
        return None;
    } // parameter initializers
    let site = catalogue
        .sites
        .get(&call.span.start)
        .filter(|site| site.call == call.span);
    let affected = site.map_or(call.span, |site| site.span);
    if selected.iter().any(|span| overlaps(*span, affected)) {
        return None;
    }
    let locals: BTreeSet<_> = plan
        .params
        .iter()
        .copied()
        .chain(plan.locals.iter().map(|l| l.symbol))
        .collect();
    // Moving the helper body must not capture a same-spelled caller local.
    if plan.occurrences.iter().any(|o| {
        !o.binding
            && !o.symbol.is_some_and(|s| locals.contains(&s))
            && scoping.find_binding(call.scope, o.name.as_str().into()) != o.symbol
    }) {
        return None;
    }
    Some((plan, owner, site, affected))
}

/// The renamed helper body for one call: its statement form, its sequence
/// form, and the returned expression.
fn expansion_parts(
    source: &str,
    plan: &Plan,
    call: &Call,
    renames: &BTreeMap<SymbolId, String>,
) -> (String, Vec<String>, String) {
    let text = |span: Span| &source[span.start as usize..span.end as usize];
    let render = |span: Span| rename_text(source, span, renames, &plan.occurrences);
    let mut prefix = String::new();
    let mut sequence = Vec::new();
    for (index, symbol) in plan.params.iter().enumerate() {
        let arg = call.args.get(index).map_or("void 0", |span| text(*span));
        let name = &renames[symbol];
        prefix.push_str(&format!("let {name} = ({arg});\n"));
        sequence.push(format!("{name} = ({arg})"));
    }
    for arg in call.args.iter().skip(plan.params.len()) {
        prefix.push_str(&format!("({});\n", text(*arg)));
        sequence.push(format!("({})", text(*arg)));
    }
    // `var` locals initialize at function entry, once per original call.
    // Embedded expansions reset every local before each evaluation too.
    for local in &plan.locals {
        if plan.params.contains(&local.symbol) {
            continue;
        }
        let name = &renames[&local.symbol];
        if local.var {
            prefix.push_str(&format!("let {name} = void 0;\n"));
        }
        sequence.push(format!("{name} = void 0"));
    }
    for step in &plan.steps {
        match step {
            Step::Declaration {
                span,
                var,
                bindings,
            } => {
                if *var {
                    for (symbol, init) in bindings {
                        if let Some(init) = init {
                            prefix.push_str(&format!(
                                "{} = ({});\n",
                                renames[symbol],
                                render(*init)
                            ));
                        }
                    }
                } else {
                    prefix.push_str(&render(*span));
                    prefix.push('\n');
                }
                for (symbol, init) in bindings {
                    if let Some(init) = init {
                        sequence.push(format!("{} = ({})", renames[symbol], render(*init)));
                    }
                }
            }
            Step::Expression(span) => {
                let expression = render(*span);
                prefix.push_str(&format!("({expression});\n"));
                sequence.push(format!("({expression})"));
            }
        }
    }
    let returned = plan.returned.map_or_else(|| "void 0".into(), render);
    (prefix, sequence, returned)
}

/// Delete helpers whose every reference was expanded this round.
fn delete_replaced_helpers(
    helpers: &BTreeMap<SymbolId, Helper>,
    scoping: &Scoping,
    replaced: &BTreeSet<ReferenceId>,
    selected: &mut Vec<Span>,
    edits: &mut Vec<analysis::Edit>,
    deleted: &mut BTreeSet<SymbolId>,
) {
    for (symbol, helper) in helpers {
        if deleted.contains(symbol) {
            continue;
        }
        let refs: Vec<_> = scoping
            .get_resolved_reference_ids(*symbol)
            .iter()
            .copied()
            .collect();
        if !refs.is_empty()
            && refs.iter().all(|r| replaced.contains(r))
            && !selected.iter().any(|span| overlaps(*span, helper.span))
        {
            selected.push(helper.span);
            edits.push(analysis::Edit::delete(helper.span));
            deleted.insert(*symbol);
        }
    }
}

fn overlaps(a: Span, b: Span) -> bool {
    a.start < b.end && b.start < a.end
}

#[derive(Clone, Copy)]
struct Owner {
    body: Span,
    insert: u32,
}
impl Owner {
    fn new(body: &FunctionBody<'_>) -> Self {
        Self {
            body: body.span,
            insert: body
                .directives
                .last()
                .map_or(body.span.start + 1, |d| d.span.end),
        }
    }
}
struct Call {
    span: Span,
    symbol: SymbolId,
    reference: ReferenceId,
    scope: ScopeId,
    owner: Option<Owner>,
    args: Vec<Span>,
    spread: bool,
    optional: bool,
}
struct Site {
    span: Span,
    call: Span,
    discard: bool,
}
struct Helper {
    span: Span,
    plan: Option<Plan>,
    removable: bool,
}
struct Plan {
    params: Vec<SymbolId>,
    locals: Vec<Local>,
    occurrences: Vec<Occurrence>,
    steps: Vec<Step>,
    returned: Option<Span>,
    strict: bool,
}
struct Local {
    symbol: SymbolId,
    var: bool,
}
enum Step {
    Declaration {
        span: Span,
        var: bool,
        bindings: Vec<(SymbolId, Option<Span>)>,
    },
    Expression(Span),
}
struct Occurrence {
    span: Span,
    symbol: Option<SymbolId>,
    name: String,
    binding: bool,
    shorthand: bool,
}

fn rename_text(
    source: &str,
    span: Span,
    renames: &BTreeMap<SymbolId, String>,
    occurrences: &[Occurrence],
) -> String {
    let mut output = String::new();
    let mut cursor = span.start;
    let at = occurrences.partition_point(|o| o.span.start < span.start);
    for occurrence in &occurrences[at..] {
        if occurrence.span.start >= span.end {
            break;
        }
        let Some(name) = occurrence.symbol.and_then(|symbol| renames.get(&symbol)) else {
            continue;
        };
        output.push_str(&source[cursor as usize..occurrence.span.start as usize]);
        if occurrence.shorthand {
            output.push_str(&source[occurrence.span.start as usize..occurrence.span.end as usize]);
            output.push_str(": ");
        }
        output.push_str(name);
        cursor = occurrence.span.end;
    }
    output.push_str(&source[cursor as usize..span.end as usize]);
    output
}

struct Catalogue<'s, 'a> {
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    helpers: BTreeMap<SymbolId, Helper>,
    calls: Vec<Call>,
    sites: BTreeMap<u32, Site>,
    owner: Option<Owner>,
    dynamic: bool,
}
impl Catalogue<'_, '_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            if let Statement::FunctionDeclaration(function) = statement {
                if let Some(id) = &function.id {
                    if id.name.starts_with("vm_sub_") {
                        if let Some(symbol) = id.symbol_id.get() {
                            // Annex-B block declarations also initialize an
                            // outer binding; do not erase that separate effect.
                            let removable = self
                                .scoping
                                .scope_flags(self.scoping.symbol_scope_id(symbol))
                                .is_function();
                            self.helpers.insert(
                                symbol,
                                Helper {
                                    span: function.span,
                                    removable,
                                    plan: removable
                                        .then(|| make_plan(function, self.scoping, self.nodes))
                                        .flatten(),
                                },
                            );
                        }
                    }
                }
            }
            let (expression, discard) = match statement {
                Statement::VariableDeclaration(d) if d.declarations.len() == 1 => {
                    (d.declarations[0].init.as_ref(), false)
                }
                Statement::ReturnStatement(r) => (r.argument.as_ref(), false),
                Statement::ThrowStatement(r) => (Some(&r.argument), false),
                Statement::ExpressionStatement(s) => match &s.expression {
                    Expression::AssignmentExpression(a)
                        if a.operator == AssignmentOperator::Assign
                            && matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id)
                            if analysis::identifier_symbol(id, self.scoping).is_some()) =>
                    {
                        (Some(&a.right), false)
                    }
                    expression => (Some(expression), true),
                },
                _ => (None, false),
            };
            if let Some(Expression::CallExpression(call)) = expression {
                self.sites.insert(
                    call.span.start,
                    Site {
                        span: statement.span(),
                        call: call.span,
                        discard,
                    },
                );
            }
        }
    }
}
impl<'a> Visit<'a> for Catalogue<'_, 'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let prior = self.owner;
        self.owner = function.body.as_ref().map(|body| Owner::new(body));
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
        self.owner = prior;
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        let prior = self.owner;
        self.owner = (!arrow.expression).then(|| Owner::new(&arrow.body));
        if !arrow.expression {
            self.body(&arrow.body.statements);
        }
        walk::walk_arrow_function_expression(self, arrow);
        self.owner = prior;
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::Identifier(id) = &call.callee {
            if let Some(reference) = id.reference_id.get() {
                let r = self.scoping.get_reference(reference);
                if let Some(symbol) = r.symbol_id() {
                    self.calls.push(Call {
                        span: call.span,
                        symbol,
                        reference,
                        scope: r.scope_id(),
                        owner: self.owner,
                        args: call.arguments.iter().map(GetSpan::span).collect(),
                        spread: call.arguments.iter().any(Argument::is_spread),
                        optional: call.optional,
                    });
                }
            }
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}

fn make_plan(function: &Function<'_>, scoping: &Scoping, nodes: &AstNodes<'_>) -> Option<Plan> {
    if function.r#async || function.generator || function.params.rest.is_some() {
        return None;
    }
    let body = function.body.as_ref()?;
    if body.directives.iter().any(|d| d.directive != "use strict") {
        return None;
    }
    let params = function
        .params
        .items
        .iter()
        .map(|p| match &p.pattern {
            BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if params.iter().collect::<BTreeSet<_>>().len() != params.len() {
        return None;
    }
    let mut locals = Vec::new();
    let mut steps = Vec::new();
    let mut returned = None;
    for (index, statement) in body.statements.iter().enumerate() {
        match statement {
            Statement::ReturnStatement(r) if index + 1 == body.statements.len() => {
                returned = r.argument.as_ref().map(GetSpan::span)
            }
            Statement::VariableDeclaration(d) => {
                let var = d.kind == VariableDeclarationKind::Var;
                if !matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                ) {
                    return None;
                }
                let mut bindings = Vec::new();
                for binding in &d.declarations {
                    let BindingPattern::BindingIdentifier(id) = &binding.id else {
                        return None;
                    };
                    let symbol = id.symbol_id.get()?;
                    if !var
                        && scoping.get_resolved_references(symbol).any(|r| {
                            (d.kind == VariableDeclarationKind::Const && r.is_write())
                                || nodes.get_node(r.node_id()).kind().span().start < d.span.end
                        })
                    {
                        return None;
                    }
                    if !locals.iter().any(|local: &Local| local.symbol == symbol) {
                        locals.push(Local { symbol, var });
                    }
                    bindings.push((symbol, binding.init.as_ref().map(GetSpan::span)));
                }
                steps.push(Step::Declaration {
                    span: d.span,
                    var,
                    bindings,
                });
            }
            Statement::ExpressionStatement(e) => steps.push(Step::Expression(e.expression.span())),
            Statement::EmptyStatement(_) => {}
            _ => return None,
        }
    }
    let mut uses = BodyUses {
        scoping,
        occurrences: Vec::new(),
        unsupported: false,
        shorthand: BTreeSet::new(),
    };
    uses.visit_function_body(body);
    if uses.unsupported {
        return None;
    }
    let own: BTreeSet<_> = params
        .iter()
        .copied()
        .chain(locals.iter().map(|l| l.symbol))
        .collect();
    if uses.occurrences.iter().any(|o| {
        !o.binding
            && o.symbol.is_some_and(|s| !own.contains(&s))
            && scoping
                .get_resolved_references(o.symbol.unwrap())
                .any(|r| nodes.get_node(r.node_id()).kind().span() == o.span && r.is_write())
    }) {
        return None;
    }
    for occurrence in &mut uses.occurrences {
        occurrence.shorthand = uses.shorthand.contains(&occurrence.span);
    }
    uses.occurrences.sort_by_key(|o| o.span.start);
    Some(Plan {
        params,
        locals,
        occurrences: uses.occurrences,
        steps,
        returned,
        strict: scoping
            .scope_flags(function.scope_id.get()?)
            .is_strict_mode(),
    })
}

struct BodyUses<'s> {
    scoping: &'s Scoping,
    occurrences: Vec<Occurrence>,
    unsupported: bool,
    shorthand: BTreeSet<Span>,
}
impl<'a> Visit<'a> for BodyUses<'_> {
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        // Destructuring shorthand has a property key distinct from its binding.
        // Keep these uncommon bodies until target-key rewriting is supported.
        self.unsupported |= matches!(
            assignment.left,
            AssignmentTarget::ArrayAssignmentTarget(_)
                | AssignmentTarget::ObjectAssignmentTarget(_)
        );
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.unsupported |= id.name == "arguments" || id.name == "eval";
        self.occurrences.push(Occurrence {
            span: id.span,
            symbol: analysis::identifier_symbol(id, self.scoping),
            name: id.name.to_string(),
            binding: false,
            shorthand: false,
        });
    }
    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        self.occurrences.push(Occurrence {
            span: id.span,
            symbol: id.symbol_id.get(),
            name: id.name.to_string(),
            binding: true,
            shorthand: false,
        });
    }
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.shorthand {
            self.shorthand.insert(property.value.span());
        }
        walk::walk_object_property(self, property);
    }
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        self.unsupported |= matches!(
            expression,
            Expression::ThisExpression(_)
                | Expression::MetaProperty(_)
                | Expression::Super(_)
                | Expression::AwaitExpression(_)
                | Expression::YieldExpression(_)
                | Expression::ClassExpression(_)
        );
        walk::walk_expression(self, expression);
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {
        self.unsupported = true;
    }
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {
        self.unsupported = true;
    }
}
