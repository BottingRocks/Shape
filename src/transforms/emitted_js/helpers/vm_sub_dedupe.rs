//! Share identical, closed VM helpers within one linked program root.
//!
//! Candidates are direct `function vm_sub_*` declarations under the same root
//! with identical canonical bodies (locals renamed by source order). Helpers
//! must not capture: every in-body reference resolves inside the helper;
//! unresolved names are treated as the same global under that root. Bodies with
//! `eval`, `with`, `this`, `super`, `new.target`, `arguments`, comments,
//! backticks, nested functions, or shorthand properties decline, as do
//! non-call references. One helper is renamed `$VMHELPER_n`, inserted at the
//! root, and its call sites renamed; originals are deleted. The result must
//! shrink and reparse with every new reference bound. This removes duplicate
//! helper definitions, not calls or allocations.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, ReferenceId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use super::super::index::SourceNames;
use crate::analysis;
use super::super::support::whole_line;

#[derive(Debug)]
pub(crate) struct SharedVmHelpers {
    pub code: String,
    pub removed: usize,
}

/// Move equivalent helpers into their common `$THREAD` root. A candidate must
/// have no captures or globals, and its binding may only be used as a direct
/// call. Keeping each root separate preserves the linked program's boundaries.
pub(crate) fn share_vm_sub_helpers(source: &str) -> Result<SharedVmHelpers, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan::default();
    scan.visit_program(&program);
    scan.references.sort_by_key(|(span, _)| span.start);
    scan.bindings.sort_by_key(|(span, _)| span.start);

    let mut groups: BTreeMap<(u32, String), Vec<&Helper>> = BTreeMap::new();
    for helper in &scan.helpers {
        let Some(symbol) = helper.symbol else {
            continue;
        };
        let Some(local_refs) = closed_references(source, &scoping, &nodes, &scan, helper, symbol)
        else {
            continue;
        };
        let Some(canonical) = canonical_body(source, &scoping, &scan, helper, local_refs) else {
            continue;
        };
        groups
            .entry((helper.root.start, canonical))
            .or_default()
            .push(helper);
    }

    let mut edits = Vec::new();
    let mut insertions = BTreeMap::<u32, String>::new();
    let mut removed = 0;
    let mut next_name = 0;
    let names = SourceNames::new(source, "$VMHELPER_");
    for ((_, _), group) in groups {
        if group.len() < 2 {
            continue;
        }
        let first = group[0];
        let name = loop {
            let name = format!("$VMHELPER_{next_name}");
            next_name += 1;
            if !names.contains(&name) {
                break name;
            }
        };
        let Some((call_edits, declaration)) =
            shared_declaration(source, &scoping, &nodes, &group, &name)
        else {
            continue;
        };
        let root_body = first.root_body.expect("nested helper has root body");
        insertions
            .entry(root_body + 1)
            .or_default()
            .push_str(&format!("\n\t{declaration}"));
        edits.extend(call_edits);
        for helper in group {
            edits.push(analysis::Edit::delete(whole_line(source, helper.span)));
            removed += 1;
        }
    }
    for (at, text) in insertions {
        edits.push(analysis::Edit::replace(Span::new(at, at), text));
    }
    if edits.is_empty() {
        return Ok(SharedVmHelpers {
            code: source.into(),
            removed: 0,
        });
    }
    checked_output(source, edits, removed)
}

/// The in-body references of a helper that may be shared: a direct
/// declaration below its root, free of dynamic scope and nested functions,
/// only ever called directly, and closed over nothing outside itself.
fn closed_references<'s>(
    source: &str,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    scan: &'s Scan,
    helper: &Helper,
    symbol: SymbolId,
) -> Option<&'s [(Span, ReferenceId)]> {
    if !scan.direct_declarations.contains(&helper.span) || helper.root == helper.span {
        return None;
    }
    let text = &source[helper.span.start as usize..helper.span.end as usize];
    if [
        "eval",
        "with(",
        "with (",
        "this",
        "super",
        "new.target",
        "arguments",
        "/*",
        "//",
        "`",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        return None;
    }
    if scan.dynamic_roots.contains(&helper.root.start) {
        return None;
    }
    if scan
        .functions
        .iter()
        .any(|span| *span != helper.span && contains(helper.span, *span))
    {
        return None;
    }
    if scan
        .shorthand_properties
        .iter()
        .any(|span| contains(helper.span, *span))
    {
        return None;
    }
    let refs = scoping.get_resolved_references(symbol).collect::<Vec<_>>();
    if refs.is_empty()
        || refs.iter().any(|reference| {
            let span = nodes.get_node(reference.node_id()).kind().span();
            !scan.direct_calls.contains(&span) || contains(helper.span, span)
        })
    {
        return None;
    }
    let start_ref = scan
        .references
        .partition_point(|(span, _)| span.start < helper.span.start);
    let end_ref = scan
        .references
        .partition_point(|(span, _)| span.start < helper.span.end);
    let local_refs = &scan.references[start_ref..end_ref];
    let mut closed = true;
    for (_, reference) in local_refs {
        if let Some(binding) = scoping.get_reference(*reference).symbol_id() {
            if binding == symbol || !contains(helper.span, scoping.symbol_span(binding)) {
                closed = false;
                break;
            }
        }
        // An unresolved name is the same global from every nested scope
        // under this root. A shadowing lexical binding would have resolved
        // above and failed the containment check.
    }
    if !closed {
        return None;
    }
    Some(local_refs)
}

/// The helper's text with its own name and locals renamed by source order
/// and leading indentation dropped, so equivalent helpers compare equal.
fn canonical_body(
    source: &str,
    scoping: &Scoping,
    scan: &Scan,
    helper: &Helper,
    local_refs: &[(Span, ReferenceId)],
) -> Option<String> {
    let start_binding = scan
        .bindings
        .partition_point(|(span, _)| span.start < helper.span.start);
    let end_binding = scan
        .bindings
        .partition_point(|(span, _)| span.start < helper.span.end);
    let mut bindings = scan.bindings[start_binding..end_binding]
        .iter()
        .filter(|(span, _)| contains(helper.span, *span) && *span != helper.id)
        .copied()
        .collect::<Vec<_>>();
    bindings.sort_by_key(|(span, _)| span.start);
    let names = bindings
        .iter()
        .enumerate()
        .map(|(n, (_, id))| (*id, format!("$LOCAL{n}")))
        .collect::<BTreeMap<_, _>>();
    let mut normalized = vec![(helper.id, "$HELPER")];
    for (span, symbol) in &bindings {
        normalized.push((*span, names[symbol].as_str()));
    }
    for (span, reference) in local_refs {
        if let Some(symbol) = scoping.get_reference(*reference).symbol_id() {
            if let Some(name) = names.get(&symbol) {
                normalized.push((*span, name.as_str()));
            }
        }
    }
    normalized.sort_by_key(|(span, _)| span.start);
    let mut canonical = String::new();
    let mut at = helper.span.start;
    for (span, name) in normalized {
        if span.start < at {
            return None;
        }
        canonical.push_str(&source[at as usize..span.start as usize]);
        canonical.push_str(name);
        at = span.end;
    }
    canonical.push_str(&source[at as usize..helper.span.end as usize]);
    let canonical = canonical
        .lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    Some(canonical)
}

/// The call renames and the reindented `name` declaration that share
/// `group`, or `None` when sharing would not shrink the source.
fn shared_declaration(
    source: &str,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    group: &[&Helper],
    name: &str,
) -> Option<(Vec<analysis::Edit>, String)> {
    let first = group[0];
    let mut call_edits = Vec::new();
    for helper in group {
        let symbol = helper.symbol.expect("group member has binding");
        for reference in scoping.get_resolved_references(symbol) {
            let span = nodes.get_node(reference.node_id()).kind().span();
            call_edits.push(analysis::Edit::replace(span, name));
        }
    }
    let original = &source[first.span.start as usize..first.span.end as usize];
    let mut declaration = original.to_string();
    let id_start = (first.id.start - first.span.start) as usize;
    let id_end = (first.id.end - first.span.start) as usize;
    declaration.replace_range(id_start..id_end, name);
    let declaration = reindent(source, first.span, &declaration);
    let added_bytes = declaration.len()
        + 2
        + call_edits
            .iter()
            .map(|edit| name.len() as isize - (edit.end - edit.start) as isize)
            .sum::<isize>()
            .max(0) as usize;
    let removed_bytes: usize = group
        .iter()
        .map(|helper| {
            whole_line(source, helper.span).end as usize
                - whole_line(source, helper.span).start as usize
        })
        .sum();
    if removed_bytes <= added_bytes {
        return None;
    }
    Some((call_edits, declaration))
}

/// Apply the sharing edits, keeping the input unless the output shrinks,
/// reparses, and binds every `$VMHELPER_` call.
fn checked_output(
    source: &str,
    edits: Vec<analysis::Edit>,
    removed: usize,
) -> Result<SharedVmHelpers, String> {
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("shared VM helper edits overlap".into());
    }
    if applied.output.len() >= source.len() {
        return Ok(SharedVmHelpers {
            code: source.into(),
            removed: 0,
        });
    }
    let check = Allocator::default();
    let parsed = analysis::parse_program(&check, &applied.output)
        .map_err(|error| format!("shared VM helper output: {error}"))?;
    let (new_scoping, _) = analysis::analyze(&parsed);
    // Parsing alone would also accept a call to an accidentally unbound helper.
    let mut bindings = BindingCheck::default();
    bindings.visit_program(&parsed);
    if bindings.refs.iter().any(|(name, reference)| {
        name.starts_with("$VMHELPER_")
            && new_scoping.get_reference(*reference).symbol_id().is_none()
    }) {
        return Err("shared VM helper call lost its binding".into());
    }
    Ok(SharedVmHelpers {
        code: applied.output,
        removed,
    })
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn reindent(source: &str, span: Span, declaration: &str) -> String {
    let line_start = source[..span.start as usize]
        .rfind('\n')
        .map_or(0, |p| p + 1);
    let old_indent = &source[line_start..span.start as usize];
    declaration
        .lines()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                return line.to_string();
            }
            format!("\n\t{}", line.strip_prefix(old_indent).unwrap_or(line))
        })
        .collect()
}

struct Helper {
    span: Span,
    id: Span,
    symbol: Option<SymbolId>,
    root: Span,
    root_body: Option<u32>,
}

#[derive(Default)]
struct Scan {
    stack: Vec<(Span, Option<u32>)>,
    functions: Vec<Span>,
    direct_declarations: BTreeSet<Span>,
    direct_calls: BTreeSet<Span>,
    helpers: Vec<Helper>,
    bindings: Vec<(Span, SymbolId)>,
    references: Vec<(Span, ReferenceId)>,
    dynamic_roots: BTreeSet<u32>,
    shorthand_properties: Vec<Span>,
}

impl<'a> Visit<'a> for Scan {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(function.span);
        if let Some(body) = &function.body {
            for statement in &body.statements {
                if let Statement::FunctionDeclaration(nested) = statement {
                    self.direct_declarations.insert(nested.span);
                }
            }
        }
        let root = self.stack.first().copied().unwrap_or((
            function.span,
            function.body.as_ref().map(|body| body.span.start),
        ));
        if let Some(id) = &function.id {
            if id.name.starts_with("vm_sub_") && !function.r#async && !function.generator {
                self.helpers.push(Helper {
                    span: function.span,
                    id: id.span,
                    symbol: id.symbol_id.get(),
                    root: root.0,
                    root_body: root.1,
                });
            }
        }
        self.stack.push((
            function.span,
            function.body.as_ref().map(|body| body.span.start),
        ));
        walk::walk_function(self, function, flags);
        self.stack.pop();
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::Identifier(id) = &call.callee {
            self.direct_calls.insert(id.span);
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        if let Some(symbol) = id.symbol_id.get() {
            self.bindings.push((id.span, symbol));
        }
    }

    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "eval" {
            if let Some((root, _)) = self.stack.first() {
                self.dynamic_roots.insert(root.start);
            }
        }
        if let Some(reference) = id.reference_id.get() {
            self.references.push((id.span, reference));
        }
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        if let Some((root, _)) = self.stack.first() {
            self.dynamic_roots.insert(root.start);
        }
        walk::walk_with_statement(self, statement);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.shorthand {
            self.shorthand_properties.push(property.span);
        }
        walk::walk_object_property(self, property);
    }
}

#[derive(Default)]
struct BindingCheck {
    refs: Vec<(String, ReferenceId)>,
}
impl<'a> Visit<'a> for BindingCheck {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if let Some(reference) = id.reference_id.get() {
            self.refs.push((id.name.to_string(), reference));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::share_vm_sub_helpers;

    #[test]
    fn shares_different_names_that_read_the_same_unresolved_global() {
        let source = "function $THREAD_0() { function a() { function vm_sub_1_1(x) { return Number(x) + 1; } return vm_sub_1_1(2); } function b() { function vm_sub_2_2(y) { return Number(y) + 1; } return vm_sub_2_2(3); } return a() + b(); }";
        let result = share_vm_sub_helpers(source).unwrap();
        assert_eq!(result.removed, 2, "{}", result.code);
        assert_eq!(result.code.matches("function $VMHELPER_0").count(), 1);
    }

    #[test]
    fn shares_closed_helpers_with_distinct_local_names() {
        let source = r#"function $THREAD_0() {
  function $THREAD_1() {
    function vm_sub_2_3($PARAMA) {
      let $SSA_1 = $PARAMA + 1;
      return $SSA_1;
    }
    return vm_sub_2_3(2);
  }
  function $THREAD_2() {
    function vm_sub_2_3($PARAMA) {
      let $SSA_2 = $PARAMA + 1;
      return $SSA_2;
    }
    return vm_sub_2_3(3);
  }
  return $THREAD_1() + $THREAD_2();
}"#;
        let result = share_vm_sub_helpers(source).unwrap();
        assert_eq!(result.removed, 2, "{}", result.code);
        assert_eq!(result.code.matches("function $VMHELPER_0").count(), 1);
        assert_eq!(result.code.matches("$VMHELPER_0(").count(), 3);
        assert!(result.code.len() < source.len());
    }

    #[test]
    fn preserves_captures_and_observable_function_identity() {
        let source = "function $THREAD_0() { let outer = 1; function a() { function vm_sub_2_3(x) { return x + outer; } return vm_sub_2_3(1); } function b() { function vm_sub_2_3(x) { return x + outer; } return vm_sub_2_3(2); } return a() + b(); }";
        assert_eq!(share_vm_sub_helpers(source).unwrap().removed, 0);
        let identity = "function $THREAD_0() { function a() { function vm_sub_2_3(x) { return x + 1; } return vm_sub_2_3; } function b() { function vm_sub_2_3(x) { return x + 1; } return vm_sub_2_3; } return a() === b(); }";
        assert_eq!(share_vm_sub_helpers(identity).unwrap().removed, 0);
    }

    #[test]
    fn keeps_separate_roots_and_distinct_bodies() {
        let roots = "function $THREAD_0() { function vm_sub_2_3(x) { return x + 1; } return vm_sub_2_3(1); } function $THREAD_1() { function vm_sub_2_3(x) { return x + 1; } return vm_sub_2_3(2); }";
        assert_eq!(share_vm_sub_helpers(roots).unwrap().removed, 0);
        let bodies = "function $THREAD_0() { function a() { function vm_sub_2_3(x) { return x + 1; } return vm_sub_2_3(1); } function b() { function vm_sub_2_3(x) { return x + 2; } return vm_sub_2_3(2); } return a() + b(); }";
        assert_eq!(share_vm_sub_helpers(bodies).unwrap().removed, 0);
    }

    #[test]
    fn keeps_dynamic_scope_and_mutable_helper_bindings() {
        let dynamic = "function $THREAD_0() { function a() { function vm_sub_2_3(x) { return x + 1; } return eval('vm_sub_2_3(1)'); } function b() { function vm_sub_2_3(x) { return x + 1; } return vm_sub_2_3(2); } return a() + b(); }";
        assert_eq!(share_vm_sub_helpers(dynamic).unwrap().removed, 0);
        let mutable = "function $THREAD_0() { function a() { function vm_sub_2_3(x) { return x + 1; } vm_sub_2_3 = other; return vm_sub_2_3(1); } function b() { function vm_sub_2_3(x) { return x + 1; } return vm_sub_2_3(2); } return a() + b(); }";
        assert_eq!(share_vm_sub_helpers(mutable).unwrap().removed, 0);
    }

    #[test]
    fn does_not_equate_different_shorthand_property_names() {
        let source = "function $THREAD_0() { function a() { function vm_sub_2_3(x) { let first = x; return {first}; } return vm_sub_2_3(1); } function b() { function vm_sub_2_3(x) { let second = x; return {second}; } return vm_sub_2_3(2); } return a(); }";
        assert_eq!(share_vm_sub_helpers(source).unwrap().removed, 0);
    }
}
