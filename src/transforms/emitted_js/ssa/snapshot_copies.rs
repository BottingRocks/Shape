//! Share duplicate lexical-input snapshots within an effect-free region.
//! Incoming VM cells are lexical by the tracer contract. Unknown globals and
//! property reads are not lexical reads and always end the region.
use std::collections::{BTreeMap, BTreeSet};

use super::super::index::SpanIndex;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

pub(crate) fn merge_snapshot_copies(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut owners = Owners::default();
    owners.visit_program(&program);
    if owners.dynamic {
        return Ok((source.into(), 0));
    }
    owners.functions.build();
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        owners: &owners.functions,
        protected,
        edits: vec![],
        count: 0,
    };
    scan.visit_program(&program);
    let result = analysis::apply_edits(source, scan.edits);
    if !result.rejected.is_empty() {
        return Err("snapshot copy edits overlap".into());
    }
    Ok((result.output, scan.count))
}
#[derive(Default)]
struct Owners {
    functions: SpanIndex,
    dynamic: bool,
}
impl<'a> Visit<'a> for Owners {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(f.span);
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.functions.push(f.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.functions.push(c.span);
        walk::walk_class(self, c);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Input {
    Local(SymbolId),
    Incoming(String),
}
struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    owners: &'s SpanIndex,
    protected: &'s BTreeSet<String>,
    edits: Vec<analysis::Edit>,
    count: usize,
}
impl Scan<'_, '_> {
    fn input(&self, id: &IdentifierReference<'_>) -> Option<Input> {
        if let Some(s) = analysis::identifier_symbol(id, self.scoping) {
            (self.scoping.symbol_flags(s).is_variable()
                && self.owners.enclosing(self.scoping.symbol_span(s)).is_some())
            .then_some(Input::Local(s))
        } else {
            self.protected
                .contains(id.name.as_str())
                .then(|| Input::Incoming(id.name.to_string()))
        }
    }
    fn body(&mut self, body: &[Statement<'_>], span: Span) {
        let mut available = BTreeMap::<Input, SymbolId>::new();
        let mut shorthand = Shorthand::default();
        for s in body {
            shorthand.visit_statement(s);
        }
        for s in body {
            let Statement::VariableDeclaration(d) = s else {
                available.clear();
                continue;
            };
            if d.declarations.len() != 1
                || !matches!(
                    d.kind,
                    VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                        | VariableDeclarationKind::Var
                )
            {
                available.clear();
                continue;
            }
            let (BindingPattern::BindingIdentifier(id), Some(init)) =
                (&d.declarations[0].id, &d.declarations[0].init)
            else {
                available.clear();
                continue;
            };
            let Some(target) = id.symbol_id.get() else {
                available.clear();
                continue;
            };
            available.retain(|key, value| key != &Input::Local(target) && *value != target);
            let Expression::Identifier(input) = init else {
                if !super::super::purity::expression_is_pure(init) {
                    available.clear();
                }
                continue;
            };
            let Some(key) = self.input(input) else {
                available.clear();
                continue;
            };
            if self.protected.contains(id.name.as_str())
                || !self.scoping.symbol_redeclarations(target).is_empty()
                || self
                    .scoping
                    .get_resolved_references(target)
                    .any(|r| r.is_write())
            {
                continue;
            }
            if let Some(previous) = available.get(&key).copied() {
                let name = self.scoping.symbol_ident(previous);
                let refs = self
                    .scoping
                    .get_resolved_references(target)
                    .collect::<Vec<_>>();
                if !refs.is_empty()
                    && refs.iter().all(|r| {
                        let at = self.nodes.get_node(r.node_id()).kind().span();
                        at.start >= d.span.end
                            && at.end <= span.end
                            && self.owners.enclosing(at) == self.owners.enclosing(d.span)
                            && self.scoping.find_binding(r.scope_id(), name) == Some(previous)
                    })
                    && !self.source[d.span.start as usize..d.span.end as usize].contains("/*")
                    && !self.source[d.span.start as usize..d.span.end as usize].contains("//")
                {
                    // Keep property keys intact when replacing a shorthand value.
                    let mut proposal = vec![analysis::Edit::delete(d.span)];
                    for r in refs {
                        let at = self.nodes.get_node(r.node_id()).kind().span();
                        let replacement = if shorthand.0.contains(&at) {
                            format!("{}: {name}", id.name)
                        } else {
                            name.to_string()
                        };
                        proposal.push(analysis::Edit::replace(at, replacement));
                    }
                    if !proposal.iter().any(|e| {
                        self.edits
                            .iter()
                            .any(|p| e.start < p.end && p.start < e.end)
                    }) {
                        self.edits.extend(proposal);
                        self.count += 1;
                        continue;
                    }
                }
            }
            available.entry(key).or_insert(target);
        }
    }
}
#[derive(Default)]
struct Shorthand(BTreeSet<Span>);
impl<'a> Visit<'a> for Shorthand {
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        if p.shorthand {
            self.0.insert(p.value.span());
        }
        walk::walk_object_property(self, p);
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.body(&b.statements, b.span);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.body(&b.body, b.span);
        walk::walk_block_statement(self, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(source: &str) -> Vec<u8> {
        let result = std::process::Command::new("node")
            .args(["-e", &format!("{source};console.log(JSON.stringify(f()));")])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result.stdout
    }
    #[test]
    fn shares_actual_lexical_snapshots_and_preserves_property_keys() {
        for source in [
            "function f(){let cell=1;let first=cell;let second=cell;cell=2;return {first,second,cell};}",
            "function f(){let cell=1;let first=cell;const pad=[];let second=cell;return [first,second,pad];}",
            "function f(){let cell={};let first=cell;let second=cell;return first===second;}",
        ] {
            let (code,count)=merge_snapshot_copies(source,&BTreeSet::new()).unwrap();
            assert_eq!(count,1,"{code}");
            assert_eq!(run(source),run(&code),"{code}");
            assert!(!code.contains("let second"),"{code}");
        }
        let source = "function f(){let first=$KEY_0;let second=$KEY_0;return {first,second};}";
        let (code, count) =
            merge_snapshot_copies(source, &BTreeSet::from(["$KEY_0".into()])).unwrap();
        assert_eq!(count, 1);
        assert_eq!(
            run(&format!("let $KEY_0=3;{source}")),
            run(&format!("let $KEY_0=3;{code}"))
        );
    }
    #[test]
    fn retains_snapshots_across_effects_redeclarations_captures_and_globals() {
        for source in [
            "function f(){let cell=1;let first=cell;let pad=(cell=2);let second=cell;return [first,second,pad];}",
            "function f(){let cell=1;let first=cell;let pad={get x(){cell=2;return 0}}.x;let second=cell;return [first,second,pad];}",
            "function f(){var cell=1;let first=cell;var cell=2;let second=cell;return [first,second];}",
            "function f(){let cell=1;let first=cell;let second=cell;return (()=>second)();}",
            "function f(){let cell=1;let first=cell;let second=cell;first=2;return [first,second];}",
            "function f(){let cell=1;let first=cell;let second=cell;{let first=9;return [first,second];}}",
            "function f(){let n=0;Object.defineProperty(globalThis,'input',{get(){return ++n},configurable:true});let first=input;let second=input;return [first,second];}",
        ] {
            let (code,count)=merge_snapshot_copies(source,&BTreeSet::new()).unwrap();
            assert_eq!(count,0,"{code}");
            assert_eq!(run(source),run(&code),"{code}");
        }
    }
}
