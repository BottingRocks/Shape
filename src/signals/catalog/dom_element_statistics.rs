//! `DomElementStatistics`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, SignalLayout, SlotView, ValueSlice};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `DomElementStatistics`. Card: [`CARD`].
pub(crate) fn recognize_dom_element_statistics(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    // The census call is an API root (`doc.querySelectorAll()`) or, when the
    // provenance roots at the document itself, a call transform on it.
    let census = (any_api(&leaves, "querySelectorAll()") || any_transform(&leaves, ".querySelectorAll()"))
        && any_transform(&leaves, ".childElementCount")
        && any_transform(&leaves, ".tagName");
    if census && max_top_count(&view.shapes()) >= 10 {
        return claim(&["10+ fields", "querySelectorAll census of tagName / childElementCount"]);
    }
    None
}

pub(crate) fn bind_dom_element_statistics(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // Six per-tag counts differ only in the tag-name literal compared where
    // the counter is stepped, and the two distinct-tag counts in the region
    // state (set under the HEAD / BODY test) guarding their list pushes.
    // Both are read from the parse at each field's own write site.
    const TAGS: &[(&str, &str)] = &[
        ("SCRIPT", "scriptCount"),
        ("IMG", "imgCount"),
        ("SVG", "svgCount"),
        ("LINK", "linkCount"),
        ("STYLE", "styleCount"),
        ("IFRAME", "iframeCount"),
    ];
    const REGIONS: &[(&str, &str)] = &[("HEAD", "headDistinctTagCount"), ("BODY", "bodyDistinctTagCount")];
    let lookup = |table: &[(&str, &'static str)], tags: &[String]| -> Option<&'static str> {
        // Every guarding tag must map, and all to the same card name.
        let mut names: Vec<&'static str> = tags
            .iter()
            .map(|tag| table.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map(|(_, name)| *name))
            .collect::<Option<Vec<_>>>()?;
        names.sort_unstable();
        names.dedup();
        match names.as_slice() {
            [name] => Some(*name),
            _ => None,
        }
    };
    let mut binding = FieldBinding::new();
    for message in 0..layout.messages.len() {
        let fields = message_fields(view, layout, message)?;
        let slices: Vec<ValueSlice> = fields.iter().map(|field| field_slice(ctx, field, 2)).collect();
        let mut names: Vec<Option<&'static str>> = Vec::new();
        for (field, slice) in fields.iter().zip(&slices) {
            let value = field.leaves.first().and_then(|leaf| ctx.caller_value_node(leaf));
            // The value once always-truthy list tests are decided and a
            // register's one live definition is followed (junk spellings).
            let live = value.map(|value| dom::live_value(ctx, value, 4));
            let name = if field.writes(LeafKind::String) {
                Some("bodyFirstChildIdPrefix")
            } else if !field.writes(LeafKind::Number) {
                None
            } else if let Some(tags) = value.and_then(|value| dom::stepped_counter_tags(ctx, value)) {
                // A counter first: a junk arm may copy another value into it
                // (`if (!list) { counter = elements; }`), which its slice
                // would otherwise read.
                lookup(TAGS, &tags)
            } else if live.filter(|live| Some(*live) != value).is_some_and(|live| dom::reads_member(ctx, live, "childElementCount")) {
                // The register's one live definition, past a junk arm's copy.
                Some("bodyChildElementCount")
            } else if slice.calls_method("reduce")
                || (slice.members.contains_key("reduce") && slice.calls_method("call"))
            {
                // `list.reduce(...)` or `(list || Object(this)).reduce.call(list, ...)`.
                Some("meanSquaredChildCount")
            } else if slice.calls_method("querySelectorAll") {
                Some("elementCount")
            } else if slice.nearest_members() == ["childElementCount"] {
                Some("bodyChildElementCount")
            } else if let Some(tags) = value.and_then(|value| dom::pushed_list_regions(ctx, value)) {
                lookup(REGIONS, &tags)
            } else if let Some(tags) = live.filter(|live| Some(*live) != value).and_then(|live| dom::stepped_counter_tags(ctx, live)) {
                lookup(TAGS, &tags)
            } else if let Some(tags) = live.filter(|live| Some(*live) != value).and_then(|live| dom::pushed_list_regions(ctx, live)) {
                lookup(REGIONS, &tags)
            } else if live.is_some_and(|live| dom::reads_member(ctx, live, "childElementCount")) {
                Some("bodyChildElementCount")
            } else if live.is_some_and(|live| dom::mean_of_reduce(ctx, live)) {
                Some("meanSquaredChildCount")
            } else {
                None
            };
            names.push(name);
        }
        for (field, name) in fields.iter().zip(&names) {
            let Some(name) = name else {
                return Err(BindRefusal::new(format!(
                    "DomElementStatistics: message {message}: field {} has no structural tell at its write site",
                    field.key
                )));
            };
            if names.iter().filter(|other| *other == &Some(*name)).count() != 1 {
                return Err(BindRefusal::new(format!(
                    "DomElementStatistics: message {message}: {name} is written by several fields"
                )));
            }
            binding.insert(message, field.key.clone(), *name);
        }
    }
    Ok(binding)
}

mod dom {
    //! Parse walks of the `DomElementStatistics` binder.

    use oxc_ast::ast::Expression;
    use oxc_ast::AstKind;
    use oxc_semantic::{NodeId, SymbolId};
    use oxc_span::GetSpan;
    use oxc_syntax::operator::BinaryOperator;

    use crate::signals::analysis::FinderCtx;

    fn contains(outer: oxc_span::Span, inner: oxc_span::Span) -> bool {
        outer.start <= inner.start && inner.end <= outer.end
    }

    /// `x === <literal>` / `<literal> === x` (`==` too): the compared side
    /// and the literal.
    fn equality<'e, 'a>(test: &'e Expression<'a>) -> Option<(&'e Expression<'a>, &'e Expression<'a>)> {
        let Expression::BinaryExpression(binary) = test.without_parentheses() else {
            return None;
        };
        if !matches!(binary.operator, BinaryOperator::StrictEquality | BinaryOperator::Equality) {
            return None;
        }
        let literal = |e: &Expression| matches!(e.without_parentheses(), Expression::StringLiteral(_) | Expression::NumericLiteral(_));
        if literal(&binary.right) {
            Some((&binary.left, &binary.right))
        } else if literal(&binary.left) {
            Some((&binary.right, &binary.left))
        } else {
            None
        }
    }

    /// The equality tests guarding `node` inside its function, innermost
    /// first: `(compared side, literal)` of every `if` whose consequent holds
    /// the node, and every `case` literal.
    fn guards<'a>(ctx: &FinderCtx<'_, '_, 'a>, node: NodeId) -> Vec<(&'a Expression<'a>, &'a Expression<'a>)> {
        let nodes = ctx.parse.nodes;
        let span = nodes.kind(node).span();
        let mut out = Vec::new();
        let mut current = node;
        loop {
            let parent = nodes.parent_id(current);
            if parent == current {
                break;
            }
            current = parent;
            match nodes.kind(current) {
                AstKind::IfStatement(statement) if contains(statement.consequent.span(), span) => {
                    if let Some(pair) = equality(&statement.test) {
                        out.push(pair);
                    }
                }
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => break,
                _ => {}
            }
        }
        out
    }

    fn string_literal(expression: &Expression) -> Option<String> {
        match expression.without_parentheses() {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        }
    }

    fn number_literal(expression: &Expression) -> Option<f64> {
        match expression.without_parentheses() {
            Expression::NumericLiteral(literal) => Some(literal.value),
            _ => None,
        }
    }

    fn identifier_symbol(ctx: &FinderCtx, expression: &Expression) -> Option<SymbolId> {
        match expression.without_parentheses() {
            Expression::Identifier(identifier) => ctx.identifier_symbol(identifier),
            _ => None,
        }
    }

    /// The nearest string literal an enclosing `if` compares against.
    fn guard_string(ctx: &FinderCtx, node: NodeId) -> Option<String> {
        guards(ctx, node).into_iter().find_map(|(_, literal)| string_literal(literal))
    }

    /// For a counter value `c`: the tag literal guarding every `++c` /
    /// `c += …` update.
    pub(crate) fn stepped_counter_tags(ctx: &FinderCtx, value: NodeId) -> Option<Vec<String>> {
        let symbol = ctx.reference_symbol(value)?;
        let nodes = ctx.parse.nodes;
        let mut tags = Vec::new();
        for reference in ctx.parse.scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let update = nodes.parent_id(reference.node_id());
            let update = match nodes.kind(update) {
                AstKind::UpdateExpression(_) => update,
                _ => match nodes.kind(nodes.parent_id(update)) {
                    AstKind::UpdateExpression(_) => nodes.parent_id(update),
                    _ => continue,
                },
            };
            // Junk after an always-taken early return never steps the counter.
            if dead(ctx, update) {
                continue;
            }
            tags.push(guard_string(ctx, update)?);
        }
        tags.sort();
        tags.dedup();
        (!tags.is_empty()).then_some(tags)
    }

    /// For a `list.length` value: the region tags of the state value that
    /// guards every `list.push(…)` (`if (state === 1) list.push(…)` with
    /// `state = 1` set under `if (tag === "HEAD")`).
    pub(crate) fn pushed_list_regions(ctx: &FinderCtx, value: NodeId) -> Option<Vec<String>> {
        let nodes = ctx.parse.nodes;
        let member_object = match nodes.kind(value) {
            AstKind::StaticMemberExpression(member) if member.property.name == "length" => &member.object,
            // `list[list ? "length" : junk]`: an array binding is always
            // truthy, so the key is "length".
            AstKind::ComputedMemberExpression(member) => {
                // The key decided by a list test.
                let key = live_value(ctx, ctx.node_with_span(member.expression.span())?, 3);
                if !matches!(nodes.kind(key), AstKind::StringLiteral(literal) if literal.value == "length") {
                    return None;
                }
                &member.object
            }
            _ => return None,
        };
        // `(list || junk).length`: an array binding is always truthy, so the
        // left operand is the list read.
        let object = match member_object.without_parentheses() {
            Expression::LogicalExpression(logical) if logical.operator == oxc_syntax::operator::LogicalOperator::Or => {
                &logical.left
            }
            other => other,
        };
        let list = match identifier_symbol(ctx, object) {
            Some(list) => list,
            // `(list ? list : junk).length`, or a copy `r = list; r.length`.
            None => {
                let node = ctx.node_with_span(object.span())?;
                ctx.reference_symbol(live_value(ctx, node, 4))?
            }
        };
        let list = match list_binding_symbol(ctx, list) {
            Some(list) => list,
            None => {
                let read = ctx.node_with_span(object.span())?;
                let live = live_value(ctx, read, 4);
                ctx.reference_symbol(live).filter(|copied| list_binding_symbol(ctx, *copied).is_some()).unwrap_or(list)
            }
        };
        let mut regions = Vec::new();
        for reference in ctx.parse.scoping.get_resolved_references(list) {
            let parent = nodes.parent_id(reference.node_id());
            let AstKind::StaticMemberExpression(push) = nodes.kind(parent) else {
                continue;
            };
            if push.property.name != "push" {
                continue;
            }
            let call = nodes.parent_id(parent);
            if !matches!(nodes.kind(call), AstKind::CallExpression(_)) {
                continue;
            }
            // A push in a junk arm (the else of an always-truthy list test)
            // never runs.
            if dead(ctx, call) {
                continue;
            }
            // The state test: `state === k` with a numeric literal.
            let (state, k) = guards(ctx, call).into_iter().find_map(|(side, literal)| {
                Some((identifier_symbol(ctx, side)?, number_literal(literal)?))
            })?;
            // Where the state is set to `k`: the tag test around it.
            for write in ctx.parse.scoping.get_resolved_references(state) {
                if !write.is_write() {
                    continue;
                }
                let mut assignment = write.node_id();
                for _ in 0..3 {
                    assignment = nodes.parent_id(assignment);
                    if matches!(nodes.kind(assignment), AstKind::AssignmentExpression(_)) {
                        break;
                    }
                }
                let AstKind::AssignmentExpression(set) = nodes.kind(assignment) else {
                    continue;
                };
                if number_literal(&set.right) != Some(k) {
                    continue;
                }
                regions.push(guard_string(ctx, assignment)?);
            }
        }
        regions.sort();
        regions.dedup();
        (!regions.is_empty()).then_some(regions)
    }

    /// A binding declared once with an array literal and never reassigned:
    /// always truthy, never null.
    fn list_binding_symbol(ctx: &FinderCtx, symbol: SymbolId) -> Option<SymbolId> {
        list_binding_hops(ctx, symbol, 3)
    }

    /// [`list_binding_symbol`], also through never-reassigned copies
    /// (`let r = list;`): the array binding itself.
    fn list_binding_hops(ctx: &FinderCtx, symbol: SymbolId, hops: usize) -> Option<SymbolId> {
        let scoping = ctx.parse.scoping;
        if scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        let AstKind::VariableDeclarator(declarator) = ctx.parse.nodes.kind(scoping.symbol_declaration(symbol)) else {
            return None;
        };
        if scoping.get_resolved_references(symbol).any(|reference| reference.is_write()) {
            return None;
        }
        match declarator.init.as_ref().map(|init| init.without_parentheses()) {
            Some(Expression::ArrayExpression(_)) => Some(symbol),
            Some(copied @ Expression::Identifier(_)) if hops > 0 => {
                list_binding_hops(ctx, identifier_symbol(ctx, copied)?, hops - 1)
            }
            _ => None,
        }
    }

    fn list_binding(ctx: &FinderCtx, expression: &Expression) -> Option<SymbolId> {
        list_binding_symbol(ctx, identifier_symbol(ctx, expression)?)
    }

    fn nullish(expression: &Expression) -> bool {
        match expression.without_parentheses() {
            Expression::NullLiteral(_) => true,
            Expression::UnaryExpression(unary) => unary.operator == oxc_syntax::operator::UnaryOperator::Void,
            Expression::Identifier(identifier) => identifier.name == "undefined",
            _ => false,
        }
    }

    /// The outcome of a test that only tests a list binding: `L` and
    /// `L != null` hold, `!L` and `L == null` (or `=== void 0`) do not; a
    /// register read is followed to its one live definition.
    fn list_test(ctx: &FinderCtx, test: &Expression) -> Option<bool> {
        list_test_hops(ctx, test, 3)
    }

    fn list_test_hops(ctx: &FinderCtx, test: &Expression, hops: usize) -> Option<bool> {
        match test.without_parentheses() {
            expression @ Expression::Identifier(_) => {
                if list_binding(ctx, expression).is_some() {
                    return Some(true);
                }
                if hops == 0 {
                    return None;
                }
                let read = ctx.node_with_span(expression.span())?;
                list_test_hops(ctx, one_live_definition(ctx, read)?, hops - 1)
            }
            Expression::UnaryExpression(unary) if unary.operator == oxc_syntax::operator::UnaryOperator::LogicalNot => {
                list_test_hops(ctx, &unary.argument, hops).map(|taken| !taken)
            }
            Expression::BinaryExpression(binary) if nullish(&binary.right) => {
                list_binding(ctx, &binary.left)?;
                match binary.operator {
                    BinaryOperator::Equality | BinaryOperator::StrictEquality => Some(false),
                    BinaryOperator::Inequality | BinaryOperator::StrictInequality => Some(true),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn ends_in_exit(statement: &oxc_ast::ast::Statement) -> bool {
        use oxc_ast::ast::Statement;
        match statement {
            Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
            Statement::BlockStatement(block) => block.body.last().is_some_and(ends_in_exit),
            _ => false,
        }
    }

    /// Some statement before `span` in `statements` is `if (L) { …; return; }`
    /// with `L` a list binding: everything after it is unreachable.
    fn after_taken_exit(ctx: &FinderCtx, statements: &[oxc_ast::ast::Statement], span: oxc_span::Span) -> bool {
        statements.iter().take_while(|statement| statement.span().end <= span.start).any(|statement| {
            matches!(statement, oxc_ast::ast::Statement::IfStatement(branch)
                if list_test_hops(ctx, &branch.test, 0) == Some(true) && ends_in_exit(&branch.consequent))
        })
    }

    /// The node never runs: it sits in the arm a list test never takes, or
    /// after an `if (L) { …; return; }` in its function. (Tests read the
    /// list binding itself here: no register is followed, so a definition
    /// lookup never re-enters.)
    pub(crate) fn dead(ctx: &FinderCtx, node: NodeId) -> bool {
        let nodes = ctx.parse.nodes;
        let span = nodes.kind(node).span();
        let mut current = node;
        loop {
            let parent = nodes.parent_id(current);
            if parent == current {
                return false;
            }
            current = parent;
            match nodes.kind(current) {
                AstKind::IfStatement(statement) => {
                    if let Some(taken) = list_test_hops(ctx, &statement.test, 0) {
                        if !taken && contains(statement.consequent.span(), span) {
                            return true;
                        }
                        if taken && statement.alternate.as_ref().is_some_and(|alternate| contains(alternate.span(), span)) {
                            return true;
                        }
                    }
                }
                AstKind::ConditionalExpression(choice) => {
                    if let Some(taken) = list_test_hops(ctx, &choice.test, 0) {
                        if (!taken && contains(choice.consequent.span(), span)) || (taken && contains(choice.alternate.span(), span)) {
                            return true;
                        }
                    }
                }
                AstKind::BlockStatement(block) => {
                    if after_taken_exit(ctx, &block.body, span) {
                        return true;
                    }
                }
                AstKind::FunctionBody(body) => {
                    return after_taken_exit(ctx, &body.statements, span);
                }
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return false,
                _ => {}
            }
        }
    }

    fn function_of(ctx: &FinderCtx, node: NodeId) -> Option<NodeId> {
        ctx.parse
            .nodes
            .ancestor_ids(node)
            .find(|id| matches!(ctx.parse.nodes.kind(*id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
    }

    /// The value `node` denotes once list tests are decided (`L ? a : b`,
    /// `L || b`) and a register read is followed to its one live,
    /// non-literal definition (`hops` deep). A counter (updated in place) or
    /// anything ambiguous stays as it is.
    pub(crate) fn live_value(ctx: &FinderCtx, node: NodeId, hops: usize) -> NodeId {
        let nodes = ctx.parse.nodes;
        let node_of = |expression: &Expression| ctx.node_with_span(expression.span());
        let next = match nodes.kind(node) {
            AstKind::ParenthesizedExpression(inner) => node_of(&inner.expression),
            AstKind::ConditionalExpression(choice) => match list_test(ctx, &choice.test) {
                Some(true) => node_of(&choice.consequent),
                Some(false) => node_of(&choice.alternate),
                None => None,
            },
            AstKind::LogicalExpression(logical)
                if logical.operator == oxc_syntax::operator::LogicalOperator::Or && list_binding(ctx, &logical.left).is_some() =>
            {
                node_of(&logical.left)
            }
            AstKind::IdentifierReference(_) if hops > 0 => one_live_definition(ctx, node).and_then(node_of),
            _ => None,
        };
        match next {
            Some(next) if next != node => live_value(ctx, next, hops.saturating_sub(1)),
            _ => node,
        }
    }

    /// The value a register read holds: its latest definition before the
    /// read in the read's own block (straight-line code), unless a callback
    /// may assign it later; else its one live non-literal definition (a
    /// callback's assignment over a literal initialiser). `None` for a list
    /// binding, a binding updated in place, or several candidates.
    fn one_live_definition<'a>(ctx: &FinderCtx<'_, '_, 'a>, read: NodeId) -> Option<&'a Expression<'a>> {
        let nodes = ctx.parse.nodes;
        let scoping = ctx.parse.scoping;
        let symbol = ctx.reference_symbol(read)?;
        if list_binding_symbol(ctx, symbol).is_some() || scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        let read_at = nodes.kind(read).span().start;
        let read_function = function_of(ctx, read);
        // (where the definition takes effect, its node, same function, value)
        let mut values: Vec<(u32, NodeId, bool, &'a Expression<'a>)> = Vec::new();
        let declaration = scoping.symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) {
            if let Some(init) = &declarator.init {
                let end = declarator.span.end;
                let same = function_of(ctx, declaration) == read_function;
                if !dead(ctx, declaration) && (!same || end <= read_at) {
                    values.push((end, declaration, same, init));
                }
            }
        }
        for reference in scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let at = reference.node_id();
            if dead(ctx, at) {
                continue;
            }
            let assignment = nodes.ancestor_ids(at).take(3).find_map(|id| match nodes.kind(id) {
                AstKind::AssignmentExpression(assignment) => Some((id, assignment)),
                _ => None,
            });
            let Some((id, assignment)) = assignment.filter(|(_, assignment)| {
                assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
            }) else {
                // An update in place (`++r`, `r += …`): a counter.
                return None;
            };
            let same = function_of(ctx, at) == read_function;
            if same && assignment.span.end > read_at {
                continue;
            }
            values.push((assignment.span.end, id, same, &assignment.right));
        }
        let literal = |expression: &Expression| {
            matches!(expression.without_parentheses(), Expression::NumericLiteral(_) | Expression::StringLiteral(_))
        };
        // The latest same-function definition whose enclosing statement list
        // also holds the read runs on every path to it.
        let latest = values.iter().filter(|value| value.2).max_by_key(|value| value.0);
        if let Some((end, node, _, value)) = latest {
            let block = nodes.ancestor_ids(*node).find(|id| {
                matches!(nodes.kind(*id), AstKind::BlockStatement(_) | AstKind::FunctionBody(_))
            });
            let dominates = block.is_some_and(|block| contains(nodes.kind(block).span(), nodes.kind(read).span()));
            let later_callback = values.iter().any(|other| !other.2 && other.0 > *end);
            if dominates && !later_callback {
                return (!literal(value)).then_some(*value);
            }
        }
        let live: Vec<&'a Expression<'a>> = values.iter().map(|value| value.3).filter(|value| !literal(value)).collect();
        match (live.as_slice(), values.as_slice()) {
            ([value], _) => Some(*value),
            // One live definition, a literal (`if (L) { k = "length"; } else
            // { k = junk; } L[k]`).
            ([], [(_, _, _, value)]) => Some(*value),
            _ => None,
        }
    }

    /// A call of `reduce`: `x.reduce(…)`, `x[k ? "reduce" : junk](…)` with
    /// a decided key, or `t[1].call(t[0], …)` with `t = [x, x.reduce]`.
    fn reduce_call(ctx: &FinderCtx, node: NodeId) -> bool {
        let nodes = ctx.parse.nodes;
        let AstKind::CallExpression(call) = nodes.kind(node) else {
            return false;
        };
        let named = |member: NodeId| match nodes.kind(member) {
            AstKind::StaticMemberExpression(read) => read.property.name == "reduce",
            AstKind::ComputedMemberExpression(read) => ctx.node_with_span(read.expression.span()).is_some_and(|key| {
                let key = live_value(ctx, key, 3);
                matches!(nodes.kind(key), AstKind::StringLiteral(literal) if literal.value == "reduce")
            }),
            _ => false,
        };
        let Some(callee) = ctx.node_with_span(call.callee.without_parentheses().span()) else {
            return false;
        };
        if named(callee) {
            return true;
        }
        match call.callee.without_parentheses() {
            Expression::StaticMemberExpression(member) if member.property.name == "call" => ctx
                .node_with_span(member.object.span())
                .is_some_and(|object| named(live_value(ctx, ctx.through_tuple(object), 3))),
            _ => false,
        }
    }

    /// The mean of squared child counts: a `reduce` call, or one divided by
    /// a length.
    pub(crate) fn mean_of_reduce(ctx: &FinderCtx, node: NodeId) -> bool {
        let nodes = ctx.parse.nodes;
        if reduce_call(ctx, node) {
            return true;
        }
        match nodes.kind(node) {
            AstKind::BinaryExpression(division) if division.operator == BinaryOperator::Division => {
                matches!(division.right.without_parentheses(), Expression::StaticMemberExpression(length) if length.property.name == "length")
                    && ctx
                        .node_with_span(division.left.span())
                        .is_some_and(|left| reduce_call(ctx, live_value(ctx, left, 4)))
            }
            _ => false,
        }
    }

    /// `x.<member>` (static).
    pub(crate) fn reads_member(ctx: &FinderCtx, node: NodeId, member: &str) -> bool {
        matches!(ctx.parse.nodes.kind(node), AstKind::StaticMemberExpression(read) if read.property.name == member)
    }

}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `DomElementStatistics`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "DomElementStatistics",
    summary: "Walks every element in the document once and reports a census: total element count, counts of script, img, svg, link, style and iframe elements, how many children BODY has and the first two characters of its first child's id, how many distinct tag names appear in HEAD and in BODY, and the average squared child count per element.",
    why: "The shape of the DOM shows whether the page really loaded and rendered as expected, and reveals injected elements from automation tools or extensions or a stripped page served to a headless bot (likely).",
    sample_payload: Some(r#"{
  "bodyChildElementCount": 16,
  "bodyDistinctTagCount": 14,
  "bodyFirstChildIdPrefix": "",
  "elementCount": 102,
  "headDistinctTagCount": 6,
  "iframeCount": 0,
  "imgCount": 0,
  "linkCount": 28,
  "meanSquaredChildCount": 29.774509803921568,
  "scriptCount": 23,
  "styleCount": 1,
  "svgCount": 1
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "headDistinctTagCount",
            "Number of distinct tag names seen from HEAD up to BODY (HEAD itself included).",
        ),
        CardField::number("svgCount", "Number of <svg> root elements."),
        CardField::number("elementCount", "Total number of elements in the document."),
        CardField::number("bodyChildElementCount", "Number of element children directly under BODY."),
        CardField::number("styleCount", "Number of <style> elements."),
        CardField::number("scriptCount", "Number of <script> elements."),
        CardField::number("linkCount", "Number of <link> elements."),
        CardField::number("iframeCount", "Number of <iframe> elements."),
        CardField::number(
            "meanSquaredChildCount",
            "Average over all elements of (number of child elements) squared; usually a fraction.",
        ),
        CardField::string(
            "bodyFirstChildIdPrefix",
            "First two characters of the id of BODY's first child element; empty string if it has no id or BODY has no element children.",
        ),
        CardField::number(
            "bodyDistinctTagCount",
            "Number of distinct tag names seen from BODY onward (BODY itself included).",
        ),
        CardField::number("imgCount", "Number of <img> elements."),
    ]),
};
