//! Inline data allocations and inert computations with one dynamic use,
//! including references in mutually exclusive branches. Small identity-free
//! computations may be rematerialized at several reads within a byte budget.
//! Identity is preserved by requiring the same execution owner and innermost
//! loop and class region. Literal fields may read initialized lexical inputs
//! only while their version is unchanged. Properties,
//! parameters, shared slots, globals, calls, spreads and closures stay barriers.
use std::collections::{BTreeMap, BTreeSet};

use super::super::adjacent_ssa::LocalReads;
use super::super::index::SpanIndex;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping};
use oxc_span::{GetSpan, Span};
use crate::transforms::emitted_js::support::unwrap;

mod scan;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod flow_pattern_tests;
#[cfg(test)]
mod exceptional_type_tests;
#[cfg(test)]
mod split_value_tests;


pub(crate) fn inline_fresh_values(
    source: &str,
    protected: &BTreeSet<String>,
    checked_captures: bool,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let mut inputs = InputFacts::default();
    inputs.visit_program(&program);
    inputs.owners.build();
    let ordinary_arguments = source.contains("arguments")
        && super::super::dense_arrays::ordinary_prototypes(&program, &scoping);
    let cells = super::super::cell_facts::collect(
        source,
        &program,
        &scoping,
        &nodes,
        &locals,
        protected,
        ordinary_arguments,
    );
    let mut scan = Scan {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        locals: &locals,
        cells: &cells,
        inputs: &inputs,
        ordinary_arguments,
        edits: vec![],
        splits: BTreeMap::new(),
        loop_stores: BTreeSet::new(),
        dynamic: false,
        pending_children: false,
        private_temps: BTreeSet::new(),
        strings: false,
        non_values: BTreeSet::new(),
        exclusive: Vec::new(),
        classes: SpanIndex::default(),
    };
    scan.visit_program(&program);
    scan.classes.build();
    if scan.dynamic {
        return Ok((source.into(), 0));
    }
    // A candidate may occur within another candidate's literal initializer.
    // Apply independent groups only; remaining substitutions run next round.
    let mut edits = vec![];
    let mut count = 0;
    for (declaration, reads, value, rematerializable) in scan.edits {
        if scan.pending_children
            && (!checked_captures || !scan.private_temps.contains(&declaration))
        {
            continue;
        }
        if reads.iter().any(|read| {
            scan.non_values.contains(read)
                || scan.classes.enclosing(*read) != scan.classes.enclosing(declaration)
        }) {
            continue;
        }
        // Allocations need one dynamic use: every reference pair must occur
        // in opposite arms. Identity-free inert computations may instead
        // repeat within the size budget. Each read passed the same version,
        // loop and execution-owner checks below.
        let contains =
            |outer: Span, inner: Span| outer.start <= inner.start && inner.end <= outer.end;
        let exclusive = rematerializable
            || !reads.iter().enumerate().any(|(index, a)| {
                reads.iter().skip(index + 1).any(|b| {
                    !scan.exclusive.iter().any(|(yes, no)| {
                        contains(*yes, *a) && contains(*no, *b)
                            || contains(*no, *a) && contains(*yes, *b)
                    })
                })
            });
        if !exclusive && !rematerializable {
            continue;
        }
        let split = scan.splits.get(&declaration);
        // A value cached in a binding has already performed GetValue. Force
        // that context when a split exposes an argument/member as a callee.
        let text = if let Some(split) = split {
            format!("(0, {})", split.value)
        } else {
            format!("({})", &source[value.start as usize..value.end as usize])
        };
        let old_bytes =
            declaration.size() as usize + reads.iter().map(|s| s.size() as usize).sum::<usize>();
        let new_bytes = text.len() * reads.len() + split.map_or(0, |s| s.effects.len());
        if reads.len() > 1
            && (new_bytes > old_bytes + 128 || text.len() > 256 && new_bytes > old_bytes)
        {
            continue;
        }
        let mut proposal = vec![analysis::Edit::replace(
            declaration,
            if let Some(split) = split {
                &split.effects
            } else if scan.loop_stores.contains(&declaration) {
                "0"
            } else if scan.strings {
                ";"
            } else {
                ""
            },
        )];
        proposal.extend(
            reads
                .into_iter()
                .map(|read| analysis::Edit::replace(read, text.clone())),
        );
        if proposal.iter().any(|e| {
            edits
                .iter()
                .any(|p: &analysis::Edit| e.start < p.end && p.start < e.end)
        }) {
            continue;
        }
        edits.extend(proposal);
        count += 1;
    }
    let result = analysis::apply_edits(source, edits);
    if !result.rejected.is_empty() {
        return Err("fresh value edits overlap".into());
    }
    Ok((result.output, count))
}

// The inert proof separately rules out effects and throws. This structural
// restriction rules out allocating a different identity at each use.
fn identity_free(e: &Expression<'_>) -> bool {
    match unwrap(e) {
        Expression::Identifier(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_) => true,
        Expression::ComputedMemberExpression(_) => super::super::argument_snapshots::input(e),
        Expression::TemplateLiteral(t) => t.expressions.iter().all(identity_free),
        Expression::SequenceExpression(s) => s.expressions.iter().all(identity_free),
        Expression::UnaryExpression(u) => identity_free(&u.argument),
        Expression::BinaryExpression(b) => identity_free(&b.left) && identity_free(&b.right),
        Expression::LogicalExpression(l) => identity_free(&l.left) && identity_free(&l.right),
        Expression::ConditionalExpression(c) => {
            identity_free(&c.test) && identity_free(&c.consequent) && identity_free(&c.alternate)
        }
        _ => false,
    }
}

#[derive(Default)]
struct InputFacts {
    owners: SpanIndex,
    readonly_arguments: BTreeSet<Span>,
    function_names: BTreeSet<String>,
}
impl<'a> Visit<'a> for InputFacts {
    fn visit_function(&mut self, f: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.owners.push(f.span);
        if let Some(id) = &f.id {
            self.function_names.insert(id.name.to_string());
        }
        if super::super::dead_ssa::readonly_arguments(f) {
            self.readonly_arguments.insert(f.span);
        }
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.owners.push(f.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.owners.push(c.span);
        walk::walk_class(self, c);
    }
}

struct Scan<'s, 'a> {
    source: &'s str,
    protected: &'s BTreeSet<String>,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    locals: &'s LocalReads,
    cells: &'s super::super::cell_facts::Facts,
    inputs: &'s InputFacts,
    ordinary_arguments: bool,
    edits: Vec<(Span, Vec<Span>, Span, bool)>,
    splits: BTreeMap<Span, SplitValue>,
    loop_stores: BTreeSet<Span>,
    exclusive: Vec<(Span, Span)>,
    classes: SpanIndex,
    non_values: BTreeSet<Span>,
    dynamic: bool,
    pending_children: bool,
    private_temps: BTreeSet<Span>,
    strings: bool,
}
// A split has no allocation identity. Effects execute once at creation;
// each value leaf independently passed the original version/loop/scope proof.
#[derive(PartialEq, Eq)]
struct SplitValue {
    effects: String,
    value: String,
    scalar: bool,
    null: bool,
}
fn join_effects(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .filter(|v| !v.is_empty())
        .map(|v| format!("({v})"))
        .collect::<Vec<_>>()
        .join(",")
}

