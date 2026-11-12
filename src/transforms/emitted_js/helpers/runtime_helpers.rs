//! Inline the emitter's exact enumeration and worker-operation helpers.
//! Argument evaluation precedes helper work; enumeration remains a key snapshot,
//! including inherited keys, rather than changing to Object.keys or live iteration.
use super::super::index::SourceNames;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{ReferenceId, ScopeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
enum Kind {
    Keys,
    Worker,
    Inherit,
}
struct Helper {
    span: Span,
    scope: ScopeId,
    kind: Kind,
}
struct Site {
    statement: Span,
    call: Span,
    reference: ReferenceId,
    args: Vec<Span>,
    discard: bool,
}
struct Scan {
    helpers: BTreeMap<SymbolId, Helper>,
    sites: Vec<Site>,
    dynamic: bool,
}
impl Scan {
    fn statements(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let (expression, discard) = match statement {
                Statement::VariableDeclaration(d)
                    if d.declarations.len() == 1
                        && matches!(d.declarations[0].id, BindingPattern::BindingIdentifier(_))
                        && !d.kind.is_using() =>
                {
                    (d.declarations[0].init.as_ref(), false)
                }
                Statement::ReturnStatement(r) => (r.argument.as_ref(), false),
                Statement::ExpressionStatement(e) => match &e.expression {
                    Expression::AssignmentExpression(a)
                        if a.operator == oxc_syntax::operator::AssignmentOperator::Assign
                            && matches!(
                                &a.left,
                                AssignmentTarget::AssignmentTargetIdentifier(_)
                            ) =>
                    {
                        (Some(&a.right), false)
                    }
                    e => (Some(e), true),
                },
                _ => (None, false),
            };
            let Some(Expression::CallExpression(call)) = expression else {
                continue;
            };
            let Expression::Identifier(id) = &call.callee else {
                continue;
            };
            if call.optional || call.arguments.iter().any(Argument::is_spread) {
                continue;
            }
            let Some(reference) = id.reference_id.get() else {
                continue;
            };
            self.sites.push(Site {
                statement: statement.span(),
                call: call.span,
                reference,
                args: call.arguments.iter().map(GetSpan::span).collect(),
                discard,
            });
        }
    }
}
impl<'a> Visit<'a> for Scan {
    fn visit_program(&mut self, p: &Program<'a>) {
        self.statements(&p.body);
        walk::walk_program(self, p);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.statements(&b.statements);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.statements(&b.body);
        walk::walk_block_statement(self, b);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}

// Compare generated syntax, not whitespace, with the emitter's source of truth.
struct Definitions<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    canonical: &'s BTreeMap<&'static str, String>,
    allocator: &'a Allocator,
    helpers: BTreeMap<SymbolId, Helper>,
}
impl<'b> Visit<'b> for Definitions<'_, '_> {
    fn visit_function(&mut self, f: &Function<'b>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(id) = &f.id {
            if let (Some(symbol), Some(expected), Some(scope)) = (
                id.symbol_id.get(),
                self.canonical.get(id.name.as_str()),
                f.scope_id.get(),
            ) {
                // Block functions can initialize a second Annex-B binding.
                if self
                    .scoping
                    .scope_flags(self.scoping.symbol_scope_id(symbol))
                    .is_function()
                    && self.scoping.symbol_redeclarations(symbol).is_empty()
                {
                    if let Ok(parsed) = analysis::parse_program(
                        self.allocator,
                        &self.source[f.span.start as usize..f.span.end as usize],
                    ) {
                        if analysis::codegen(&parsed) == *expected {
                            let kind = match id.name.as_str() {
                                "vmForInKeys" => Kind::Keys,
                                "vmPostToWorker" => Kind::Worker,
                                _ => Kind::Inherit,
                            };
                            self.helpers.insert(
                                symbol,
                                Helper {
                                    span: f.span,
                                    scope,
                                    kind,
                                },
                            );
                        }
                    }
                }
            }
        }
        walk::walk_function(self, f, flags);
    }
}

/// Finds the functions whose generated syntax matches a canonical helper, as
/// helpers keyed by their symbol.
fn helper_definitions<'a>(
    source: &str,
    allocator: &'a Allocator,
    program: &Program<'a>,
    scoping: &Scoping,
) -> Result<BTreeMap<SymbolId, Helper>, String> {
    let mut canonical = BTreeMap::new();
    for &(name, text) in crate::tracing::symbolic::HELPERS {
        let parsed = analysis::parse_program(allocator, text)?;
        canonical.insert(name, analysis::codegen(&parsed));
    }
    let mut definitions = Definitions {
        source,
        scoping,
        canonical: &canonical,
        allocator,
        helpers: BTreeMap::new(),
    };
    definitions.visit_program(program);
    Ok(definitions.helpers)
}

/// Returns the helper a call site may expand, or None when the site overlaps an
/// earlier expansion, the helper is reassigned, or its strictness or
/// dependency bindings differ at the call.
fn site_helper<'h>(
    site: &Site,
    selected: &[Span],
    scoping: &Scoping,
    helpers: &'h BTreeMap<SymbolId, Helper>,
) -> Option<&'h Helper> {
    if selected
        .iter()
        .any(|span| span.start < site.statement.end && site.statement.start < span.end)
    {
        return None;
    }
    let reference = scoping.get_reference(site.reference);
    let Some(symbol) = reference.symbol_id() else {
        return None;
    };
    let Some(helper) = helpers.get(&symbol) else {
        return None;
    };
    if scoping
        .get_resolved_references(symbol)
        .any(|r| r.is_write())
    {
        return None;
    }
    let scope = reference.scope_id();
    if scoping.scope_flags(scope).is_strict_mode()
        != scoping.scope_flags(helper.scope).is_strict_mode()
    {
        return None;
    }
    let dependencies: &[&str] = match helper.kind {
        Kind::Keys => &[],
        Kind::Worker => &["vmInitializeWorker", "vmWorkerCallbacks", "vmWorker"],
        Kind::Inherit => &["Object"],
    };
    if dependencies.iter().any(|name| {
        scoping.find_binding(scope, (*name).into())
            != scoping.find_binding(helper.scope, (*name).into())
    }) {
        return None;
    }
    Some(helper)
}

/// Builds the statements replacing a helper call: fresh argument bindings in
/// order, the helper body inlined, then the statement with the call replaced.
fn expand_site(
    site: &Site,
    helper: &Helper,
    source: &str,
    fresh: &mut impl FnMut() -> String,
) -> String {
    let count = match helper.kind {
        Kind::Worker => 5,
        _ => 1,
    };
    let text = |s: Span| &source[s.start as usize..s.end as usize];
    let mut prefix = String::new();
    let args: Vec<_> = (0..count)
        .map(|i| {
            let name = fresh();
            prefix.push_str(&format!(
                "let {name} = ({});\n",
                site.args.get(i).map_or("void 0", |s| text(*s))
            ));
            name
        })
        .collect();
    for arg in site.args.iter().skip(count) {
        prefix.push_str(&format!("({});\n", text(*arg)));
    }
    let result = fresh();
    match helper.kind {
        Kind::Keys => {
            let key = fresh();
            prefix.push_str(&format!("let {result} = [];\nfor (let {key} in {}) {{ {result}[{result}.length] = {key}; }}\n",args[0]));
        }
        Kind::Worker => {
            prefix.push_str(&format!("let {result} = false;\nif (vmInitializeWorker()) {{ vmWorkerCallbacks.push({}); vmWorker.postMessage([{}, {}, {}, {}]); {result} = true; }}\n", args[4],args[0],args[1],args[2],args[3]));
        }
        Kind::Inherit => {
            prefix.push_str(&format!("let {result} = typeof {} === \"object\" || typeof {} === \"function\" ? Object.create({}) : {{}};\n",args[0],args[0],args[0]));
        }
    }
    if !site.discard {
        let statement = text(site.statement);
        let start = (site.call.start - site.statement.start) as usize;
        let end = (site.call.end - site.statement.start) as usize;
        prefix.push_str(&format!(
            "{}{}{}",
            &statement[..start],
            result,
            &statement[end..]
        ));
    }
    prefix
}

/// Returns source, expanded calls and removed declarations. Unsupported uses
/// retain their original helper and are never substituted merely by name.
pub(crate) fn inline_runtime_helpers(source: &str) -> Result<(String, usize, usize), String> {
    // Most emitted functions do not contain any operation helper.
    if !source.contains("function vmForInKeys")
        && !source.contains("function vmPostToWorker")
        && !source.contains("function vmInheritOrObject")
    {
        return Ok((source.into(), 0, 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let helpers = helper_definitions(source, &allocator, &program, &scoping)?;
    let mut scan = Scan {
        helpers,
        sites: Vec::new(),
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok((source.into(), 0, 0));
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
    let mut replaced = BTreeSet::new();
    let mut selected: Vec<Span> = Vec::new();
    for site in &scan.sites {
        let Some(helper) = site_helper(site, &selected, &scoping, &scan.helpers) else {
            continue;
        };
        let prefix = expand_site(site, helper, source, &mut fresh);
        edits.push(analysis::Edit::replace(site.statement, prefix));
        selected.push(site.statement);
        replaced.insert(site.reference);
    }
    let mut removed = 0;
    for (&symbol, helper) in &scan.helpers {
        if scoping
            .get_resolved_reference_ids(symbol)
            .iter()
            .all(|r| replaced.contains(r))
        {
            edits.push(analysis::Edit::delete(helper.span));
            removed += 1;
        }
    }
    if edits.is_empty() {
        return Ok((source.into(), 0, 0));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err(analysis::describe_rejected_edits(&applied.rejected));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)?;
    Ok((applied.output, replaced.len(), removed))
}
