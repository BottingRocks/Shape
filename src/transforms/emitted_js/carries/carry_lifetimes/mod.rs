//! Coalesce local copy-out cells when their live ranges do not interfere.
//! Keep the original source name. Earlier writes are legal only when no read,
//! capture, abrupt exit or enclosing exception handler can observe the change.
//! Copy-in initialization can instead keep the existing destination when the
//! source's remaining reads finish before any destination-update statement.
use super::super::{adjacent_ssa::LocalReads, index::SpanIndex};
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;
use std::collections::{BTreeMap, BTreeSet};

mod scan;
mod hazards;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod uninitialized_channels_tests;
#[cfg(test)]
mod finished_channel_semantics_tests;

use hazards::*;

pub(crate) fn coalesce_local_handoffs(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    coalesce(source, protected, false)
}

// Run after existing snapshot coalescing has chosen storage, so a new channel
// does not steal a destination needed by an earlier, longer-lived snapshot.
pub(crate) fn finish_private_channels(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    coalesce(source, protected, true)
}
fn coalesce(
    source: &str,
    protected: &BTreeSet<String>,
    finished: bool,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.visit_program(&program);
    facts.owners.build();
    if facts.dynamic {
        return Ok((source.into(), 0));
    }
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        facts: &facts,
        locals: &locals,
        protected,
        edits: Vec::new(),
        count: 0,
        finished,
    };
    scan.visit_program(&program);
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("local handoff edits overlap".into());
    }
    Ok((applied.output, scan.count))
}
#[derive(Default)]
struct Facts {
    owners: SpanIndex,
    exceptions: Vec<ExceptionRegion>,
    function_writes: Vec<(String, Span)>,
    writes: BTreeMap<Span, Span>,
    shorthand: BTreeSet<Span>,
    dynamic: bool,
}
struct ExceptionRegion {
    span: Span,
    block: Span,
    handler: Option<Span>,
    finalizer: Option<Span>,
    handler_exits: bool,
}
fn exits(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
        Statement::BlockStatement(b) => b.body.last().is_some_and(exits),
        Statement::IfStatement(i) => {
            exits(&i.consequent) && i.alternate.as_ref().is_some_and(exits)
        }
        _ => false,
    }
}
impl<'a> Visit<'a> for Facts {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = &f.id {
            self.function_writes.push((id.name.to_string(), f.span));
        }
        self.owners.push(f.span);
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
    fn visit_try_statement(&mut self, s: &TryStatement<'a>) {
        self.exceptions.push(ExceptionRegion {
            span: s.span,
            block: s.block.span,
            handler: s.handler.as_ref().map(|h| h.body.span),
            finalizer: s.finalizer.as_ref().map(|b| b.span),
            handler_exits: s
                .handler
                .as_ref()
                .is_some_and(|h| h.body.body.last().is_some_and(exits)),
        });
        walk::walk_try_statement(self, s);
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
            self.writes.insert(id.span, a.span);
        }
        walk::walk_assignment_expression(self, a);
    }
    fn visit_update_expression(&mut self, u: &UpdateExpression<'a>) {
        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument {
            self.writes.insert(id.span, u.span);
        }
        walk::walk_update_expression(self, u);
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        if p.shorthand {
            self.shorthand.insert(p.value.span());
        }
        walk::walk_object_property(self, p);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}
struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    facts: &'s Facts,
    locals: &'s LocalReads,
    protected: &'s BTreeSet<String>,
    edits: Vec<analysis::Edit>,
    count: usize,
    finished: bool,
}
// Definite assignment along every evaluated path, including comma and
// short-circuit expressions. An initial undefined observation keeps the reset.
// Complex control is deliberately a barrier; lexical captures were excluded
// separately by the live-range proof.

