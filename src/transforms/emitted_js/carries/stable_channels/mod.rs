//! Eliminate argument and private-cell forwarding channels using path state.
//! A read is replaceable only when every reaching normal path has copied the
//! current input. Source stores invalidate that equality after evaluating RHS.
use super::super::{adjacent_ssa::LocalReads, index::SpanIndex};
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Reference, Scoping, SymbolId};
use oxc_span::{GetSpan, Ident, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};
use std::collections::{BTreeMap, BTreeSet};

mod facts;
mod flow;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod exceptional_flow_tests;

use facts::*;
use flow::*;
use crate::transforms::emitted_js::support::unwrap;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Input {
    Local(SymbolId),
    Incoming(u32),
    Argument(u32),
}
fn input(e: &Expression<'_>, scoping: &Scoping) -> Option<Input> {
    match unwrap(e) {
        Expression::Identifier(id) => analysis::identifier_symbol(id, scoping)
            .map(Input::Local)
            .or_else(|| {
                id.name
                    .strip_prefix("$KEY_")?
                    .parse()
                    .ok()
                    .map(Input::Incoming)
            }),
        Expression::ComputedMemberExpression(m) if super::super::argument_snapshots::input(e) => {
            let Expression::Identifier(id) = unwrap(&m.object) else {
                return None;
            };
            if analysis::identifier_symbol(id, scoping).is_some() {
                return None;
            }
            let Expression::NumericLiteral(n) = unwrap(&m.expression) else {
                return None;
            };
            Some(Input::Argument(n.value as u32))
        }
        _ => None,
    }
}

// Borrow AST nodes through the semantic arena rather than storing visitor
// borrows (Visit does not promise a node borrow lasting for the AST lifetime).
pub(crate) fn eliminate_stable_channels(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
) -> Result<(String, usize, usize), String> {
    let alloc = Allocator::default();
    let program = analysis::parse_program(&alloc, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.visit_program(&program);
    facts.owners.build();
    if facts.dynamic {
        return Ok((source.into(), 0, 0));
    }
    for node in nodes.iter() {
        match node.kind() {
            AstKind::ExpressionStatement(s) => {
                if let Expression::AssignmentExpression(a) = unwrap(&s.expression) {
                    facts.discarded.insert(a.span, (s.span, true));
                }
            }
            AstKind::SequenceExpression(s) => {
                for value in s
                    .expressions
                    .iter()
                    .take(s.expressions.len().saturating_sub(1))
                {
                    if let Expression::AssignmentExpression(a) = unwrap(value) {
                        facts.discarded.insert(a.span, (value.span(), false));
                    }
                }
            }
            AstKind::BlockStatement(b) => {
                facts.blocks.insert(b.span, b);
                facts.list(b.span, &b.body, &scoping);
            }
            AstKind::Function(f) => {
                if let Some(b) = &f.body {
                    facts.bodies.insert(f.span, b);
                    facts.list(b.span, &b.statements, &scoping);
                }
            }
            AstKind::VariableDeclaration(d) => {
                for item in &d.declarations {
                    if let BindingPattern::BindingIdentifier(id) = &item.id {
                        if let Some(s) = id.symbol_id.get() {
                            facts.declarations.insert(s, (d, item));
                            facts.stores.entry(s).or_default();
                        }
                    }
                }
            }
            AstKind::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    if let (Some(s), Some(value)) = (
                        analysis::identifier_symbol(id, &scoping),
                        input(&a.right, &scoping),
                    ) {
                        facts
                            .stores
                            .entry(s)
                            .or_default()
                            .push((a.span, a.right.span(), value));
                    }
                }
            }
            _ => {}
        }
    }
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let ordinary = super::super::dense_arrays::ordinary_prototypes(&program, &scoping);
    let cells = super::super::cell_facts::collect(
        source, &program, &scoping, &nodes, &locals, protected, ordinary,
    );
    let mut edits = Vec::<analysis::Edit>::new();
    let (mut arguments, mut aliases) = (0, 0);
    let channels = Channels { source, scoping: &scoping, nodes: &nodes, facts: &facts, locals: &locals, ordinary, cells: &cells, protected, checked };
    for (&symbol, stores) in &facts.stores {
        let Some((proposal, value)) = channels.channel(symbol, stores, &edits) else {
            continue;
        };
        edits.extend(proposal);
        match value {
            Input::Argument(_) => arguments += 1,
            Input::Local(_) | Input::Incoming(_) => aliases += 1,
        };
    }
    let result = analysis::apply_edits(source, edits);
    if !result.rejected.is_empty() {
        return Err("stable channel edits overlap".into());
    }
    Ok((result.output, arguments, aliases))
}

/// What one channel's proof reads: the program's facts, gathered once.
struct Channels<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    facts: &'s Facts<'a>,
    locals: &'s LocalReads,
    ordinary: bool,
    cells: &'s super::super::cell_facts::Facts,
    protected: &'s BTreeSet<String>,
    checked: bool,
}

/// One channel under proof: its declaration, and the copy-out and input the
/// first proofs settled on.
struct Channel<'c, 'a> {
    symbol: SymbolId,
    stores: &'c [(Span, Span, Input)],
    decl: &'a VariableDeclaration<'a>,
    item: &'a VariableDeclarator<'a>,
    owner: Span,
    body: &'a FunctionBody<'a>,
    branch_copy: Option<BranchCopy<'a>>,
    copy_out: Option<CopyOut<'a>>,
    adjacent_copy: bool,
    value: Input,
}

impl<'s, 'a> Channels<'s, 'a> {
    /// One channel: the edits that remove it and the input it carries, or
    /// `None` when a proof fails or its edits would overlap `edits`, the ones
    /// already accepted.
    fn channel(
        &self,
        symbol: SymbolId,
        stores: &[(Span, Span, Input)],
        edits: &[analysis::Edit],
    ) -> Option<(Vec<analysis::Edit>, Input)> {
        let (decl, item, owner, body) = self.declaration(symbol)?;
        let branch_copy = self.branch_copy(symbol, decl, item);
        let copy_out = self.copy_out(symbol, stores, decl, item, owner);
        let copy_out = copy_out.or_else(|| branch_copy.as_ref().map(|b| b.copy));
        let adjacent_copy = branch_copy.is_none()
            && copy_out
                .as_ref()
                .is_some_and(|c| self.facts.adjacent(decl.span, c.statement));
        let value = self.value(stores, item, &branch_copy, copy_out)?;
        let channel = Channel {
            symbol,
            stores,
            decl,
            item,
            owner,
            body,
            branch_copy,
            copy_out,
            adjacent_copy,
            value,
        };
        let (refs, reads) = self.references(&channel)?;
        self.carried_input(&channel, &refs)?;
        self.flow(&channel, &reads)?;
        let proposal = self.proposal(&channel, &reads);
        if self.overlaps(&proposal, edits) {
            return None;
        }
        Some((proposal, value))
    }

    /// The channel's `let`/`const` declaration and the function body that
    /// owns it, when its name and declaration shape are ones we eliminate.
    fn declaration(
        &self,
        symbol: SymbolId,
    ) -> Option<(
        &'a VariableDeclaration<'a>,
        &'a VariableDeclarator<'a>,
        Span,
        &'a FunctionBody<'a>,
    )> {
        let Channels { scoping, facts, protected, checked, .. } = *self;
        let name = scoping.symbol_name(symbol);
        if !(name.starts_with("$SSA_") || name.starts_with("$CARRY_"))
            || protected.contains(name)
            || !scoping.symbol_redeclarations(symbol).is_empty()
        {
            return None;
        }
        let Some(&(decl, item)) = facts.declarations.get(&symbol) else {
            return None;
        };
        if !matches!(
            decl.kind,
            VariableDeclarationKind::Let | VariableDeclarationKind::Const
        ) {
            return None;
        }
        let Some(owner) = facts.owners.enclosing(item.span) else {
            return None;
        };
        let Some(body) = facts.bodies.get(&owner) else {
            return None;
        };
        if facts.pending && !checked {
            return None;
        }
        Some((decl, item, owner, *body))
    }

    /// An uninitialized channel that both arms of the preceding `if` store,
    /// copied out right after into a slot every arm's store already sees.
    fn branch_copy(
        &self,
        symbol: SymbolId,
        decl: &VariableDeclaration<'a>,
        item: &VariableDeclarator<'a>,
    ) -> Option<BranchCopy<'a>> {
        let Channels { scoping, facts, .. } = *self;
        // A computed snapshot may already have an existing slot copy-out.
        // Store its producer directly there, then prove every remaining use
        // still observes that same value. No new slot/name is introduced.
        let branch_copy = item
            .init
            .is_none()
            .then_some(())
            .and_then(|_| facts.copies.get(&symbol))
            .and_then(|copies| {
                copies
                    .iter()
                    .filter(|c| {
                        c.statement.start >= decl.span.end
                            && facts.lists.get(&decl.span) == facts.lists.get(&c.statement)
                    })
                    .find_map(|copy| {
                        let Statement::IfStatement(branch) =
                            *facts.previous.get(&copy.statement)?
                        else {
                            return None;
                        };
                        let mut leaves = Vec::new();
                        branch_leaves(&branch.consequent, symbol, &scoping, &mut leaves)?;
                        branch_leaves(branch.alternate.as_ref()?, symbol, &scoping, &mut leaves)?;
                        if leaves.is_empty()
                            || leaves.iter().any(|leaf| {
                                scoping.find_binding(leaf.scope, copy.name)
                                    != match copy.value {
                                        Input::Local(s) => Some(s),
                                        _ => None,
                                    }
                            })
                        {
                            return None;
                        }
                        Some(BranchCopy {
                            copy: *copy,
                            leaves,
                        })
                    })
            });
        branch_copy
    }

    /// The first copy of a never-stored, initialized channel into a slot,
    /// either adjacent to it or of a producer no statement between can change.
    fn copy_out(
        &self,
        symbol: SymbolId,
        stores: &[(Span, Span, Input)],
        decl: &VariableDeclaration<'a>,
        item: &VariableDeclarator<'a>,
        owner: Span,
    ) -> Option<CopyOut<'a>> {
        let Channels { scoping, nodes, facts, locals, ordinary, protected, .. } = *self;
        let copy_out = (decl.declarations.len() == 1 && stores.is_empty())
            .then_some(())
            .and(item.init.as_ref())
            .filter(|init| {
                !matches!(
                    unwrap(init),
                    Expression::FunctionExpression(_)
                        | Expression::ArrowFunctionExpression(_)
                        | Expression::ClassExpression(_)
                )
            })
            .and_then(|init| {
                facts.copies.get(&symbol).and_then(|copies| {
                    copies
                        .iter()
                        .filter(|c| {
                            c.statement.start >= decl.span.end
                                && facts.lists.get(&decl.span) == facts.lists.get(&c.statement)
                        })
                        .filter(|c| {
                            facts.adjacent(decl.span, c.statement)
                                || immutable_producer(
                                    init,
                                    &scoping,
                                    &nodes,
                                    &locals,
                                    protected,
                                    ordinary && facts.arguments.contains(&owner),
                                )
                        })
                        .min_by_key(|c| c.statement.start)
                        .copied()
                })
            });
        copy_out
    }

    /// The input the channel carries: its copy-out's, else its initializer's
    /// or first store's. Without a branch copy, every store carries it too.
    fn value(
        &self,
        stores: &[(Span, Span, Input)],
        item: &VariableDeclarator<'a>,
        branch_copy: &Option<BranchCopy<'a>>,
        copy_out: Option<CopyOut<'a>>,
    ) -> Option<Input> {
        let scoping = self.scoping;
        let value = if let Some(copy) = &copy_out {
            copy.value
        } else {
            match &item.init {
                Some(init) => match input(init, &scoping) {
                    Some(i) => i,
                    None => return None,
                },
                None => match stores.first() {
                    Some(s) => s.2,
                    None => return None,
                },
            }
        };
        if branch_copy.is_none() && stores.iter().any(|s| s.2 != value) {
            return None;
        }
        Some(value)
    }

    /// The channel's references, and the reads to replace: every reference
    /// is in the owner, initialized, a value, and a write only as a store.
    fn references(
        &self,
        channel: &Channel<'_, 'a>,
    ) -> Option<(Vec<&'s Reference>, Vec<Span>)> {
        let Channels { scoping, nodes, facts, locals, .. } = *self;
        let Channel { symbol, stores, owner, ref branch_copy, copy_out, .. } = *channel;
        let refs: Vec<_> = scoping.get_resolved_references(symbol).collect();
        if (copy_out.is_some() && branch_copy.is_none()
            || scoping.symbol_flags(symbol).is_const_variable())
            && refs.iter().any(|r| r.is_write())
        {
            // Even a same-value store must retain its const-assignment error.
            return None;
        }
        if refs.iter().any(|r|{
            let at=nodes.get_node(r.node_id()).kind().span();
            facts.owners.enclosing(at)!=Some(owner)||!locals.initialized(symbol,at)
                ||facts.nonvalues.contains(&at)
                || r.is_write() && (!matches!(nodes.parent_kind(r.node_id()),AstKind::AssignmentExpression(a) if stores.iter().any(|s|s.0==a.span)||branch_copy.as_ref().is_some_and(|b|b.leaves.iter().any(|l|l.assignment==a.span)))||r.is_read())
        }){return None;}
        if let Some(branch) = &branch_copy {
            if refs.iter().any(|r| {
                let span = nodes.get_node(r.node_id()).kind().span();
                r.is_write() && !branch.leaves.iter().any(|l| l.left == span)
                    || r.is_read()
                        && span != branch.copy.read
                        && span.start < branch.copy.statement.end
            }) {
                return None;
            }
        }
        let reads: Vec<_> = refs
            .iter()
            .filter(|r| r.is_read())
            .map(|r| nodes.get_node(r.node_id()).kind().span())
            .filter(|span| copy_out.as_ref().is_none_or(|c| c.read != *span))
            .collect();
        if reads.is_empty() && copy_out.is_none() {
            return None;
        }
        Some((refs, reads))
    }

    /// What the carried input itself must satisfy at every read: a protected
    /// `$KEY_` slot not shadowed there, an `arguments` the owner keeps
    /// ordinary, or a plain local initialized wherever it is read.
    fn carried_input(&self, channel: &Channel<'_, 'a>, refs: &[&Reference]) -> Option<()> {
        let Channels { scoping, nodes, facts, locals, ordinary, cells, protected, .. } = *self;
        let Channel { symbol, stores, decl, item, owner, ref branch_copy, copy_out, value, .. } =
            *channel;
        match value {
            Input::Incoming(index) => {
                let key = format!("$KEY_{index}");
                let source_name = if let Some(copy) = &copy_out {
                    copy.name
                } else {
                    let Some(Expression::Identifier(id)) = item.init.as_ref().map(unwrap) else {
                        return None;
                    };
                    id.name
                };
                if !protected.contains(&key)
                    || branch_copy.is_none()
                        && (item.init.is_none()
                            || decl.declarations.len() != 1
                            || !stores.is_empty())
                    || refs.iter().any(|r| {
                        branch_copy.is_none() && r.is_write()
                            || scoping.find_binding(r.scope_id(), source_name).is_some()
                    })
                {
                    return None;
                }
                let expression = facts.following.get(&decl.span).and_then(|s| match s {
                    Statement::ExpressionStatement(s) => Some(&s.expression),
                    Statement::ReturnStatement(s) => s.argument.as_ref(),
                    Statement::ThrowStatement(s) => Some(&s.argument),
                    Statement::IfStatement(s) => Some(&s.test),
                    Statement::VariableDeclaration(s)
                        if s.declarations.len() == 1
                            && matches!(
                                s.declarations[0].id,
                                BindingPattern::BindingIdentifier(_)
                            ) =>
                    {
                        s.declarations[0].init.as_ref()
                    }
                    _ => None,
                });
                if copy_out.is_none()
                    && !cells.ready.contains(&item.span)
                    && !expression.is_some_and(|e| {
                        super::super::adjacent_ssa::begins_with_read(e, symbol, &scoping, &locals)
                    })
                {
                    return None;
                }
            }
            Input::Argument(_) if !ordinary || !facts.arguments.contains(&owner) => return None,
            Input::Local(s) => {
                if !matches!(nodes.kind(scoping.symbol_declaration(s)),AstKind::VariableDeclarator(d) if matches!(d.id,BindingPattern::BindingIdentifier(_)))
                    || s == symbol
                    || scoping.symbol_flags(s).is_function_scoped_declaration()
                        && facts.function_names.contains(scoping.symbol_name(s))
                    || item
                        .init
                        .as_ref()
                        .is_some_and(|e| !locals.plain_initialized(s, e.span()))
                    || stores
                        .iter()
                        .any(|store| !locals.plain_initialized(s, store.0))
                    || refs.iter().filter(|r| r.is_read()).any(|r| {
                        let at = nodes.get_node(r.node_id()).kind().span();
                        !locals.plain_initialized(s, at)
                            || scoping.find_binding(r.scope_id(), scoping.symbol_ident(s))
                                != Some(s)
                    })
                {
                    return None;
                }
            }
            _ => {}
        }
        Some(())
    }

    /// Walk the channel's activation with `Flow`: every read must see the
    /// input still equal to what the channel last copied.
    fn flow(&self, channel: &Channel<'_, 'a>, reads: &[Span]) -> Option<()> {
        let Channels { scoping, nodes, facts, locals, ordinary, cells, protected, .. } = *self;
        let Channel { symbol, stores, decl, item, owner, body, ref branch_copy, copy_out, value, .. } =
            *channel;
        let value_spans: Vec<_> = if let Some(branch) = &branch_copy {
            branch.leaves.iter().map(|l| l.rhs).collect()
        } else {
            item.init
                .iter()
                .map(GetSpan::span)
                .chain(stores.iter().map(|s| s.1))
                .collect()
        };
        let copy_values: Vec<_> = value_spans
            .iter()
            .map(|s| cells.values.get(s).cloned().unwrap_or_default())
            .collect();
        let token = item
            .init
            .as_ref()
            .and_then(|_| copy_values.first())
            .and_then(|v| v.token.clone())
            .filter(|t| copy_values.iter().all(|v| v.token.as_ref() == Some(t)));
        let primitive_snapshot = copy_values.iter().all(|v| v.primitive);
        let mut flow = Flow {
            cells: &cells,
            copy_out: copy_out.as_ref().map(|c| c.point),
            token,
            primitive_snapshot,
            scoping: &scoping,
            target: symbol,
            input: value,
            effects: match value {
                Input::Incoming(_) => true,
                Input::Argument(_) => false,
                Input::Local(s) => {
                    protected.contains(scoping.symbol_name(s)) || !locals.initialized(s, item.span)
                }
            },
            lexical: protected,
            locals: &locals,
            safe_arguments: ordinary && facts.arguments.contains(&owner),
            reads: &reads,
            mutations: match value {
                Input::Argument(_) | Input::Incoming(_) => vec![],
                Input::Local(s) => std::iter::once(scoping.symbol_span(s))
                    .chain(
                        scoping
                            .get_resolved_references(s)
                            .filter(|r| r.is_write())
                            .map(|r| nodes.get_node(r.node_id()).kind().span()),
                    )
                    .collect(),
            },
            equal: false,
            live: true,
            valid: true,
        };
        // A lexical channel inside a loop is a new cell on every block
        // entry. Prove its complete activation, not the enclosing function's
        // loop backedge. All reads/writes already passed initialization and
        // execution-owner checks above.
        if let Some((_, block)) = facts
            .blocks
            .iter()
            .filter(|(span, _)| {
                owner.start <= span.start
                    && span.end <= owner.end
                    && span.start <= decl.span.start
                    && decl.span.end <= span.end
            })
            .min_by_key(|(span, _)| span.size())
        {
            flow.visit_block_statement(block);
        } else {
            flow.visit_function_body(body);
        }
        if !flow.valid {
            return None;
        }
        Some(())
    }

    /// The edits that remove the channel: its declaration, its branch stores
    /// or copy-out, its stores, and each read replaced by the input.
    fn proposal(&self, channel: &Channel<'_, 'a>, reads: &[Span]) -> Vec<analysis::Edit> {
        let Channels { source, facts, .. } = *self;
        let Channel { stores, decl, item, ref branch_copy, copy_out, adjacent_copy, value, .. } =
            *channel;
        let input_span = copy_out.as_ref().map(|c| c.dest).unwrap_or_else(|| {
            item.init
                .as_ref()
                .map(GetSpan::span)
                .unwrap_or_else(|| stores[0].1)
        });
        let text = &source[input_span.start as usize..input_span.end as usize];
        let retained = decl
            .declarations
            .iter()
            .filter(|d| d.span != item.span)
            .map(|d| &source[d.span.start as usize..d.span.end as usize])
            .collect::<Vec<_>>();
        let mut proposal = vec![analysis::Edit::replace(
            decl.span,
            if adjacent_copy {
                let init = item.init.as_ref().unwrap().span();
                format!(
                    "{}{text}=({});",
                    if copy_out.as_ref().is_some_and(|c| c.var) {
                        "var "
                    } else {
                        ""
                    },
                    &source[init.start as usize..init.end as usize]
                )
            } else if retained.is_empty() {
                ";".into()
            } else {
                format!(
                    "{} {};",
                    if decl.kind == VariableDeclarationKind::Const {
                        "const"
                    } else {
                        "let"
                    },
                    retained.join(", ")
                )
            },
        )];
        if let Some(branch) = &branch_copy {
            proposal.extend(
                branch
                    .leaves
                    .iter()
                    .map(|leaf| analysis::Edit::replace(leaf.left, text)),
            );
            proposal.push(analysis::Edit::replace(
                branch.copy.statement,
                if branch.copy.var {
                    format!("var {text};")
                } else {
                    String::new()
                },
            ));
        } else if let Some(copy) = &copy_out {
            if adjacent_copy {
                proposal.push(analysis::Edit::delete(copy.statement));
            } else {
                let init = item.init.as_ref().unwrap().span();
                proposal.push(analysis::Edit::replace(
                    copy.statement,
                    format!(
                        "{}{text}=({});",
                        if copy.var { "var " } else { "" },
                        &source[init.start as usize..init.end as usize]
                    ),
                ));
            }
        }
        proposal.extend(stores.iter().filter(|_| branch_copy.is_none()).map(|s| {
            if let Some((span, statement)) = facts.discarded.get(&s.0) {
                // Both input kinds have already been proved inert and
                // initialized. A discarded copy need not leave a junk read.
                return analysis::Edit::replace(*span, if *statement { ";" } else { "0" });
            }
            analysis::Edit::replace(
                s.0,
                // Assignment returns a value, including in delete/tag/call
                // contexts. Preserve GetValue when forwarding a member read.
                format!("(0, {})", &source[s.1.start as usize..s.1.end as usize]),
            )
        }));
        proposal.extend(reads.iter().map(|at| {
            analysis::Edit::replace(
                *at,
                if facts.bare.contains(at)
                    && matches!(value, Input::Argument(_) | Input::Incoming(_))
                {
                    format!("(0, {text})")
                } else {
                    format!("({text})")
                },
            )
        }));
        proposal
    }

    /// Whether an edit would drop a comment or overlap one already accepted.
    fn overlaps(&self, proposal: &[analysis::Edit], edits: &[analysis::Edit]) -> bool {
        let source = self.source;
        proposal.iter().any(|e| {
            source[e.start as usize..e.end as usize].contains("/*")
                || source[e.start as usize..e.end as usize].contains("//")
                || edits.iter().any(|p| e.start < p.end && p.start < e.end)
        })
    }
}

