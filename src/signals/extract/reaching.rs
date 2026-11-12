//! The value a local holds at one read: its single reaching definition.
//!
//! The recovered programs coalesce locals: one binding can hold a function
//! first and, after `v = encode(v(filter))`, the encoded text. A symbol-identity
//! check cannot tell those values apart, so a pass that needs "what does this
//! read see" asks [`value_at_use`] instead.
//!
//! The answer is deliberately narrow. A definition is accepted only when it is
//! a preceding statement of the same statement list as the read (the same
//! block, so no loop back edge, branch or labeled jump can bring another value
//! to the read), and it is the nearest statement in that list that writes the
//! binding. Every other shape is refused with a reason:
//!
//! * the binding is not a plain local of the reading function (a parameter, a
//!   function or class declaration, a top-level binding or a captured outer
//!   binding), or it is declared more than once;
//! * any write to the binding sits inside a nested function, so a call could
//!   change it between the definition and the read;
//! * the reading function contains a direct `eval` or a `with`, or sits inside
//!   a `with`;
//! * the nearest writing statement is not exactly `v = value;` or a
//!   declaration `let v = value;` (a write in a branch, loop, short circuit or
//!   compound/update form is refused, not looked through);
//! * the read's own statement writes the binding, other than as the target of
//!   `v = value(v)` with the read in `value`;
//! * a loop/branch writes the local without a single plain reaching definition;
//! * no definition precedes the read in its block.

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

/// The single definition that reaches one read of a local.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReachingDefinition<'a> {
    /// The value expression the definition stores (assignment right-hand side
    /// or declarator initializer).
    pub value: &'a Expression<'a>,
}

/// The value `expression` holds at its own position when it is a read of a
/// local whose single reaching definition is proven; `Err` explains why the
/// value is not proven. Parentheses are looked through.
pub(crate) fn expression_value_at_use<'a>(
    scoping: &Scoping,
    nodes: &AstNodes<'a>,
    expression: &Expression<'_>,
) -> Result<ReachingDefinition<'a>, String> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => value_at_use(scoping, nodes, identifier),
        _ => Err("the expression is not a local read".to_string()),
    }
}

/// The single definition reaching the read `identifier`.
pub(crate) fn value_at_use<'a>(
    scoping: &Scoping,
    nodes: &AstNodes<'a>,
    identifier: &IdentifierReference<'_>,
) -> Result<ReachingDefinition<'a>, String> {
    let mut definitions = definitions_at_use(scoping, nodes, identifier, false)?;
    match definitions.len() {
        1 => Ok(definitions.remove(0)),
        _ => Err("several definitions reach the read".to_string()),
    }
}

/// Every definition reaching the read `expression` (a local read): the single
/// proven definition of [`value_at_use`], or the plain writes that end the
/// arms of the nearest writing `if` before the read
/// (`if (t) { v = a; } else if (u) { v = b; } else { v = c; } f(v)`), joined
/// by the earlier definition when an arm leaves the local unwritten. Arms
/// hold only plain writes, nested `if`s, blocks and statements that do not
/// write the local; any other writing shape is refused as in [`value_at_use`].
pub(crate) fn expression_definitions_at_use<'a>(
    scoping: &Scoping,
    nodes: &AstNodes<'a>,
    expression: &Expression<'_>,
) -> Result<Vec<ReachingDefinition<'a>>, String> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => definitions_at_use(scoping, nodes, identifier, true),
        _ => Err("the expression is not a local read".to_string()),
    }
}

/// The definitions reaching `identifier`; with `join` false, only a single
/// plain one (see [`value_at_use`]).
fn definitions_at_use<'a>(
    scoping: &Scoping,
    nodes: &AstNodes<'a>,
    identifier: &IdentifierReference<'_>,
    join: bool,
) -> Result<Vec<ReachingDefinition<'a>>, String> {
    let reference = identifier
        .reference_id
        .get()
        .map(|id| scoping.get_reference(id))
        .ok_or("the read has no reference")?;
    if !reference.is_read() {
        return Err("the reference is not a read".to_string());
    }
    let symbol = reference
        .symbol_id()
        .ok_or("the read does not resolve to a local")?;
    let use_node = reference.node_id();

    if !scoping.symbol_redeclarations(symbol).is_empty() {
        return Err("the local is declared more than once".to_string());
    }
    let declaration = scoping.symbol_declaration(symbol);
    if !matches!(nodes.kind(declaration), AstKind::VariableDeclarator(_)) {
        return Err("the binding is not a variable declaration".to_string());
    }
    let owner = enclosing_function(nodes, use_node).ok_or("the read is not inside a function")?;
    if enclosing_function(nodes, declaration) != Some(owner) {
        return Err("the local is not declared by the reading function".to_string());
    }

    let mut writes = Vec::new();
    for reference in scoping.get_resolved_references(symbol) {
        if !reference.is_write() {
            continue;
        }
        if enclosing_function(nodes, reference.node_id()) != Some(owner) {
            return Err("a nested function writes the local".to_string());
        }
        writes.push(nodes.get_node(reference.node_id()).span());
    }

    if dynamic_scope(nodes, owner) {
        return Err("the reading function has a direct eval or a with scope".to_string());
    }

    let mut cursor = use_node;
    let mut own_target = None;
    loop {
    let (list, position) = statement_of(nodes, cursor)?;
    let use_span = nodes.get_node(use_node).span();
    let read_statement = &list[position];
    let own_writes: Vec<Span> = writes
        .iter()
        .copied()
        .filter(|write| within(read_statement.span(), *write)
            && (cursor == use_node || write.start < use_span.end)
            && Some(*write) != own_target
            && !in_other_arm(nodes, use_node, read_statement.span(), *write))
        .collect();
    if !own_writes.is_empty() {
        match plain_assignment(read_statement, symbol, scoping) {
            Some((target, value)) if own_writes == [target] && within(value.span(), use_span) => { own_target = Some(target); }
            _ => return Err("the read's own statement writes the local".to_string()),
        }
    }

    let declaration_span = nodes.get_node(declaration).span();
    // Values from arms of an `if` already passed that leave the local
    // unwritten on some path: the earlier definition joins them.
    let mut joined: Vec<ReachingDefinition<'a>> = Vec::new();
    for statement in list[..position].iter().rev() {
        let span = statement.span();
        let writes_here: Vec<Span> = writes
            .iter()
            .copied()
            .filter(|write| within(span, *write))
            .collect();
        let declares_here = within(span, declaration_span);
        if writes_here.is_empty() && !declares_here {
            // Loops that do not write this binding cannot change its value.
            // Captured writes and dynamic scope were rejected above; a loop
            // that does write it is handled by the non-plain-write refusal.
            continue;
        }
        if !declares_here && writes_only_in_exiting_handler(statement, &writes_here) {
            // `try { … } catch { … v = x; …; return … }`: the handler never
            // completes normally, so its writes cannot reach a later read.
            continue;
        }
        if !writes_here.is_empty() {
            match plain_assignment(statement, symbol, scoping) {
                Some((target, value)) if writes_here == [target] && !declares_here => {
                    joined.push(ReachingDefinition { value });
                    return Ok(joined);
                }
                _ if join && !declares_here && matches!(statement, Statement::IfStatement(_)) => {
                    let (values, falls_through) = arm_definitions(statement, symbol, scoping, &writes_here)?;
                    joined.extend(values.into_iter().map(|value| ReachingDefinition { value }));
                    if falls_through {
                        continue;
                    }
                    return Ok(joined);
                }
                _ => return Err("the nearest write is not a plain `local = value` statement".to_string()),
            }
        }
        return match declaration_value(statement, declaration_span) {
            Some(value) => {
                joined.push(ReachingDefinition { value });
                Ok(joined)
            }
            None => Err("the nearest definition declares the local without a value".to_string()),
        };
    }
    if !joined.is_empty() {
        // An arm leaves the local unwritten and no definition precedes the
        // `if` in its block: the value from outside is not looked for.
        return Err("an arm leaves the local unwritten and no definition precedes it".to_string());
    }
    // A nested block may read a definition in its enclosing statement list.
    // Cross only a block/branch boundary with no other write to this binding
    // in that containing statement; loop backedges that write it are refused.
    loop {
        let parent = nodes.parent_id(cursor);
        if parent == cursor || matches!(nodes.kind(parent),
            AstKind::FunctionBody(_) | AstKind::Program(_) | AstKind::Function(_)
            | AstKind::ArrowFunctionExpression(_)) {
            return Err("no definition precedes the read in its block".to_string());
        }
        cursor = parent;
        if matches!(nodes.kind(cursor), AstKind::ForStatement(_) | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_) | AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_))
            && writes.iter().any(|write| within(nodes.get_node(cursor).span(), *write)) {
            return Err("an enclosing loop writes the local".to_string());
        }
        if matches!(nodes.kind(cursor), AstKind::BlockStatement(_) | AstKind::SwitchCase(_)) {
            break;
        }
    }
    }
}

/// The values the writes of `statement` leave in the local when it completes:
/// the last plain write of each path through nested `if`s and blocks, and
/// whether some path writes nothing (the earlier value then survives). A
/// write that is not a whole `local = value` statement on such a path (in a
/// test, loop, `try`, switch, label or expression) is refused. An arm that
/// exits (`return`) still lists its write: the set may hold more values than
/// reach, never fewer.
fn arm_definitions<'a>(
    statement: &'a Statement<'a>,
    symbol: SymbolId,
    scoping: &Scoping,
    writes: &[Span],
) -> Result<(Vec<&'a Expression<'a>>, bool), String> {
    let writes_in = |span: Span| writes.iter().filter(|write| within(span, **write)).count();
    if writes_in(statement.span()) == 0 {
        return Ok((Vec::new(), true));
    }
    match statement {
        Statement::IfStatement(branch) => {
            if writes_in(branch.test.span()) > 0 {
                return Err("an `if` test writes the local".to_string());
            }
            let (mut values, mut falls_through) = arm_definitions(&branch.consequent, symbol, scoping, writes)?;
            match &branch.alternate {
                Some(alternate) => {
                    let (other, other_falls) = arm_definitions(alternate, symbol, scoping, writes)?;
                    values.extend(other);
                    falls_through |= other_falls;
                }
                None => falls_through = true,
            }
            Ok((values, falls_through))
        }
        Statement::BlockStatement(block) => {
            let mut values = Vec::new();
            for inner in block.body.iter().rev() {
                let (found, falls_through) = arm_definitions(inner, symbol, scoping, writes)?;
                values.extend(found);
                if !falls_through {
                    return Ok((values, false));
                }
            }
            Ok((values, true))
        }
        _ => match plain_assignment(statement, symbol, scoping) {
            Some((target, value)) if writes_in(statement.span()) == 1 && writes.contains(&target) => {
                Ok((vec![value], false))
            }
            _ => Err("an arm writes the local other than as a plain `local = value` statement".to_string()),
        },
    }
}

/// `write` sits in the arm of an `if` (inside `statement`, around the read
/// `use_node`) that does not hold the read: `if (t) { v = a; } else { f(v); }`.
/// That arm and the read never run on one pass, so the write cannot reach
/// the read unless a loop around both (inside `statement`) brings it back.
fn in_other_arm(nodes: &AstNodes<'_>, use_node: NodeId, statement: Span, write: Span) -> bool {
    let mut child = use_node;
    let mut excluded = false;
    loop {
        let parent = nodes.parent_id(child);
        if parent == child {
            return false;
        }
        let parent_span = nodes.get_node(parent).span();
        if !within(statement, parent_span) {
            return excluded;
        }
        let child_span = nodes.get_node(child).span();
        match nodes.kind(parent) {
            AstKind::IfStatement(branch) if within(parent_span, write) => {
                let arms = [Some(branch.consequent.span()), branch.alternate.as_ref().map(|arm| arm.span())];
                let own = arms.iter().flatten().find(|arm| within(**arm, child_span));
                let other = arms.iter().flatten().find(|arm| !within(**arm, child_span));
                if own.is_some() && other.is_some_and(|arm| within(*arm, write)) {
                    excluded = true;
                }
            }
            AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
                if within(parent_span, write) =>
            {
                return false;
            }
            _ => {}
        }
        child = parent;
    }
}

/// The innermost code unit that runs `node` on its own schedule: a function
/// or arrow, or a class static block or field initializer (those run at class
/// evaluation or construction, not where they appear).
fn enclosing_function(nodes: &AstNodes<'_>, node: NodeId) -> Option<NodeId> {
    let mut current = node;
    loop {
        let parent = nodes.parent_id(current);
        if parent == current {
            return None;
        }
        current = parent;
        if matches!(
            nodes.kind(current),
            AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::StaticBlock(_)
                | AstKind::PropertyDefinition(_)
                | AstKind::AccessorProperty(_)
        ) {
            return Some(current);
        }
    }
}

/// The statement list holding the statement that contains `node`, and that
/// statement's position in it.
pub(crate) fn statement_of<'a>(
    nodes: &AstNodes<'a>,
    node: NodeId,
) -> Result<(&'a [Statement<'a>], usize), String> {
    let mut child = node;
    loop {
        let parent = nodes.parent_id(child);
        if parent == child {
            return Err("the read is not inside a statement list".to_string());
        }
        let list: Option<&'a [Statement<'a>]> = match nodes.kind(parent) {
            AstKind::FunctionBody(body) => Some(&body.statements),
            AstKind::BlockStatement(block) => Some(&block.body),
            AstKind::SwitchCase(case) => Some(&case.consequent),
            AstKind::StaticBlock(block) => Some(&block.body),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_) => {
                return Err("the read is not inside a statement list".to_string())
            }
            _ => None,
        };
        if let Some(list) = list {
            let span = nodes.get_node(child).span();
            return list
                .iter()
                .position(|statement| statement.span() == span)
                .map(|position| (list, position))
                .ok_or_else(|| "the read's statement is not in its list".to_string());
        }
        child = parent;
    }
}

/// `local = value;` as a whole statement: the target identifier's span and
/// the stored value.
fn plain_assignment<'a>(
    statement: &'a Statement<'a>,
    symbol: SymbolId,
    scoping: &Scoping,
) -> Option<(Span, &'a Expression<'a>)> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
        return None;
    };
    (crate::analysis::identifier_symbol(target, scoping) == Some(symbol))
        .then_some((target.span, &assignment.right))
}

/// The initializer of the declarator `declaration` when `statement` is the
/// declaration that holds it directly and binds the local by name (a
/// destructuring declarator's initializer is not the local's value).
fn declaration_value<'a>(
    statement: &'a Statement<'a>,
    declaration: Span,
) -> Option<&'a Expression<'a>> {
    let Statement::VariableDeclaration(declaration_statement) = statement else {
        return None;
    };
    declaration_statement
        .declarations
        .iter()
        .find(|declarator| {
            declarator.span == declaration
                && matches!(declarator.id, BindingPattern::BindingIdentifier(_))
        })?
        .init
        .as_ref()
}

/// `statement` is a `try` whose every write in `writes` sits in the catch
/// body, and that body ends in `return`/`throw` (it never completes
/// normally, so control after the `try` never saw those writes).
fn writes_only_in_exiting_handler(statement: &Statement<'_>, writes: &[Span]) -> bool {
    let Statement::TryStatement(try_statement) = statement else {
        return false;
    };
    let Some(handler) = &try_statement.handler else {
        return false;
    };
    let exits = matches!(
        handler.body.body.last(),
        Some(Statement::ReturnStatement(_) | Statement::ThrowStatement(_))
    );
    exits && !writes.is_empty() && writes.iter().all(|write| within(handler.body.span, *write))
}

fn within(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// True when `function` contains a direct `eval` call or a `with` statement
/// anywhere (nested functions included), or sits inside a `with`.
fn dynamic_scope(nodes: &AstNodes<'_>, function: NodeId) -> bool {
    let mut current = function;
    loop {
        let parent = nodes.parent_id(current);
        if parent == current {
            break;
        }
        current = parent;
        if matches!(nodes.kind(current), AstKind::WithStatement(_)) {
            return true;
        }
    }
    let mut find = DynamicScope { found: false };
    match nodes.kind(function) {
        AstKind::Function(function) => find.visit_function(function, ScopeFlags::Function),
        AstKind::ArrowFunctionExpression(arrow) => find.visit_arrow_function_expression(arrow),
        _ => return true,
    }
    find.found
}

struct DynamicScope {
    found: bool,
}

impl<'a> Visit<'a> for DynamicScope {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if matches!(call.callee.without_parentheses(), Expression::Identifier(callee) if callee.name == "eval")
        {
            self.found = true;
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_with_statement(&mut self, with: &WithStatement<'a>) {
        self.found = true;
        walk::walk_with_statement(self, with);
    }
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use oxc_ast::AstKind;
    use oxc_parser::Parser;
    use oxc_semantic::SemanticBuilder;
    use oxc_span::{GetSpan, SourceType};

    use super::{definitions_at_use, value_at_use};

    /// The value the last read of `v` in `source` sees, as source text.
    fn last_read_value(source: &str) -> Result<String, String> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let read = semantic
            .nodes()
            .iter()
            .filter_map(|node| match node.kind() {
                AstKind::IdentifierReference(identifier) if identifier.name == "v" => Some(identifier),
                _ => None,
            })
            .filter(|identifier| {
                identifier
                    .reference_id
                    .get()
                    .is_some_and(|id| semantic.scoping().get_reference(id).is_read())
            })
            .max_by_key(|identifier| identifier.span.start)
            .expect("a read of v");
        value_at_use(semantic.scoping(), semantic.nodes(), read)
            .map(|definition| source[definition.value.span().start as usize..definition.value.span().end as usize].to_string())
    }

    #[test]
    fn exiting_catch_writes_do_not_reach() {
        let source = "function f(g) { let v = 1; v = g; try { g(); } catch (e) { v = 2; return 0; } return v; }";
        assert_eq!(last_read_value(source).as_deref(), Ok("g"));
    }

    #[test]
    fn a_write_in_the_other_arm_does_not_reach() {
        // `v` is reused: the then arm stores a position, the else arm reads
        // the value stored before the `if`.
        let source = "function f(g, w, p) { let v; v = g.data; if (v == null) { v = p + 1; } else { v = w(v, p); } return 0; }";
        assert_eq!(last_read_value(source).as_deref(), Ok("g.data"));
    }

    fn last_read_values(source: &str) -> Result<Vec<String>, String> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let read = semantic
            .nodes()
            .iter()
            .filter_map(|node| match node.kind() {
                AstKind::IdentifierReference(identifier) if identifier.name == "v" => Some(identifier),
                _ => None,
            })
            .filter(|identifier| {
                identifier
                    .reference_id
                    .get()
                    .is_some_and(|id| semantic.scoping().get_reference(id).is_read())
            })
            .max_by_key(|identifier| identifier.span.start)
            .expect("a read of v");
        definitions_at_use(semantic.scoping(), semantic.nodes(), read, true).map(|definitions| {
            definitions
                .iter()
                .map(|definition| source[definition.value.span().start as usize..definition.value.span().end as usize].to_string())
                .collect()
        })
    }

    #[test]
    fn nested_arm_writes_all_reach_a_reused_local() {
        // A reused register: earlier values never reach past an `if` whose
        // every path writes it.
        let source = "function f(g, h, t, u) { let v = g.dimensions.length; h(v); \
            if (t) { v = g.antialias; if (v == null) { v = h.w | 0; } else { v = h.w | 0 | v << 1; } } \
            else { v = h.w | 0; } return v; }";
        assert_eq!(
            last_read_values(source),
            Ok(vec!["h.w | 0".to_string(), "h.w | 0 | v << 1".to_string(), "h.w | 0".to_string()])
        );
        // The single-definition answer still refuses.
        assert!(last_read_value(source).is_err());
    }

    #[test]
    fn an_unwritten_arm_joins_the_earlier_definition() {
        let source = "function f(g, t) { let v = g.a; if (t) { v = g.b; } return v; }";
        assert_eq!(last_read_values(source), Ok(vec!["g.b".to_string(), "g.a".to_string()]));
        let no_earlier = "function f(g, t) { let v; if (t) { v = g.b; } return v; }";
        assert!(last_read_values(no_earlier).is_err());
    }

    #[test]
    fn arm_writes_in_other_shapes_refuse() {
        let looped = "function f(g, t) { let v = g.a; if (t) { for (;;) { v = g.b; } } else { v = 1; } return v; }";
        assert!(last_read_values(looped).is_err());
        let compound = "function f(g, t) { let v = g.a; if (t) { v += 1; } else { v = 1; } return v; }";
        assert!(last_read_values(compound).is_err());
        let in_test = "function f(g, t) { let v = g.a; if (v = t) { v = 2; } else { v = 1; } return v; }";
        assert!(last_read_values(in_test).is_err());
    }

    #[test]
    fn a_loop_around_both_arms_still_refuses() {
        let source = "function f(g, w, p) { let v = g; for (;;) { if (p) { v = p + 1; } else { w(v); } } }";
        assert!(last_read_value(source).is_err());
        let test_write = "function f(g, w, p) { let v = g; if (v = p) { v = 1; } else { w(v); } }";
        assert!(last_read_value(test_write).is_err());
    }

    #[test]
    fn falling_through_catch_writes_still_refuse() {
        let source = "function f(g) { let v = 1; v = g; try { g(); } catch (e) { v = 2; } return v; }";
        assert!(last_read_value(source).is_err());
        let in_block = "function f(g) { let v = 1; v = g; try { v = 3; } catch (e) { return 0; } return v; }";
        assert!(last_read_value(in_block).is_err());
    }
}
