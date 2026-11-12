//! Bounded proof for the standalone canvas `measureText` font-presence sampler.
//!
//! This is the leaf probe, not the p10 configuration binding and not an
//! aggregate collector that happens to contain font work. The owned body
//! creates one captured-document canvas, establishes sans-serif/serif
//! baselines for `0-_{w.`, schedules a captured queue over the captured
//! `.fonts` table, emits the boolean vector on queue empty and returns a
//! queue-empty teardown. Every participant is resolved through the scoped
//! model; generated names, source offsets, vendors and old labels are not
//! evidence.
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;
use oxc_syntax::operator::UpdateOperator;

type Symbol = SymbolId;

const NAME: &str = "emitFontMetricsProbe";
const EVALUATOR: &str = "shared_font_metric_canvas_sampler";
const SENTINEL: &str = "0-_{w.";

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn member<'a>(expression: &'a Expression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn target_member<'a>(target: &'a AssignmentTarget<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn computed<'a>(
    expression: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional).then_some((&member.object, &member.expression))
}

fn call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expression.without_parentheses() {
        Expression::CallExpression(call) if !call.optional => Some(call),
        _ => None,
    }
}

// A spread is unknown arity, including at a claimed zero-argument call.
fn arguments<'a>(call: &'a CallExpression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    call.arguments.iter().map(Argument::as_expression).collect()
}

fn string(expression: &Expression<'_>, expected: &str) -> bool {
    matches!(expression.without_parentheses(), Expression::StringLiteral(value) if value.value == expected)
}

fn number(expression: &Expression<'_>, expected: f64) -> bool {
    matches!(expression.without_parentheses(), Expression::NumericLiteral(value) if value.value == expected)
}

fn formal(model: &Model<'_, '_>, pattern: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = pattern else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    (model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty())
        .then_some(symbol)
}

fn local(model: &Model<'_, '_>, pattern: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = pattern else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(symbol)
        .is_empty()
        .then_some(symbol)
}

fn plain_formals(model: &Model<'_, '_>, function: &Function<'_>) -> Option<[Symbol; 3]> {
    if function.params.rest.is_some() || function.params.items.len() != 3 {
        return None;
    }
    let [first, second, third] = function.params.items.as_slice() else {
        return None;
    };
    Some([
        formal(model, &first.pattern)?,
        formal(model, &second.pattern)?,
        formal(model, &third.pattern)?,
    ])
}

fn single_origin(model: &Model<'_, '_>, symbol: Symbol) -> bool {
    let writes = model.binding_writes(symbol);
    writes == 0 || (writes == 1 && model.initializer(symbol).is_none())
}

fn defined_in(model: &Model<'_, '_>, site: FunctionSite, symbol: Symbol) -> bool {
    if model.initializer(symbol).is_some() {
        return model
            .owning_site(model.scoping.symbol_declaration(symbol))
            .is_some_and(|owner| owner.span == site.span);
    }
    let mut writes = model
        .scoping
        .get_resolved_references(symbol)
        .filter(|reference| reference.is_write());
    let Some(reference) = writes.next() else {
        return false;
    };
    writes.next().is_none()
        && model
            .owning_site(reference.node_id())
            .is_some_and(|owner| owner.span == site.span)
}

fn root_symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    let mut current = expression.without_parentheses();
    let mut last = None;
    for _ in 0..8 {
        let Some(found) = symbol(model, current) else {
            break;
        };
        if !single_origin(model, found) {
            return None;
        }
        last = Some(found);
        let Some(value) = model.symbol_value(found) else {
            break;
        };
        if value.span().start >= current.span().start {
            return None;
        }
        current = value.without_parentheses();
    }
    last
}

fn slot_of(model: &Model<'_, '_>, expression: &Expression<'_>, site: FunctionSite) -> Option<u32> {
    let mut current = expression.without_parentheses();
    for _ in 0..8 {
        if let Some(slot) = model.site_parameter_slot(current, site) {
            return Some(slot);
        }
        let found = symbol(model, current)?;
        if !single_origin(model, found) {
            return None;
        }
        current = model.symbol_value(found)?.without_parentheses();
    }
    None
}

fn captured(model: &Model<'_, '_>, site: FunctionSite, symbol: Symbol) -> bool {
    !site
        .span
        .contains_inclusive(model.scoping.symbol_span(symbol))
        && model.scoping.symbol_redeclarations(symbol).is_empty()
}

fn assignment_target(
    model: &Model<'_, '_>,
    assignment: &AssignmentExpression<'_>,
) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

fn call_binding(model: &Model<'_, '_>, call: &CallExpression<'_>) -> Option<Symbol> {
    let parent = model.nodes.parent_id(call.node_id.get());
    match model.nodes.kind(parent) {
        AstKind::VariableDeclarator(variable)
            if variable
                .init
                .as_ref()
                .is_some_and(|init| init.span() == call.span) =>
        {
            local(model, &variable.id)
        }
        AstKind::AssignmentExpression(assignment)
            if assignment.operator == AssignmentOperator::Assign
                && assignment.right.span() == call.span =>
        {
            assignment_target(model, assignment)
        }
        _ => None,
    }
}

fn containing_binding(model: &Model<'_, '_>, call: &CallExpression<'_>) -> Option<Symbol> {
    let mut current = call.node_id.get();
    for _ in 0..32 {
        let parent = model.nodes.parent_id(current);
        if parent == current {
            return None;
        }
        match model.nodes.kind(parent) {
            AstKind::VariableDeclarator(variable)
                if variable
                    .init
                    .as_ref()
                    .is_some_and(|init| init.span().contains_inclusive(call.span)) =>
            {
                return local(model, &variable.id);
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.right.span().contains_inclusive(call.span) =>
            {
                return assignment_target(model, assignment);
            }
            _ => {
                current = parent;
            }
        }
    }
    None
}

fn empty_array(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::ArrayExpression(array) if array.elements.is_empty())
}

fn array_elements<'a>(expression: &'a Expression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return None;
    };
    array
        .elements
        .iter()
        .map(oxc_ast::ast::ArrayExpressionElement::as_expression)
        .collect()
}

#[derive(Default)]
struct Scan<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    values: Vec<(Symbol, &'a Expression<'a>)>,
}

/// Collect only nodes owned by one function. Nested sampler and teardown
/// bodies are intentionally excluded from the outer scan.
fn scan<'a>(model: &Model<'_, 'a>, site: FunctionSite) -> Scan<'a> {
    let mut scan = Scan::default();
    for node in model.nodes.iter() {
        if !model
            .owning_site(node.id())
            .is_some_and(|owner| owner.span == site.span)
        {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call) => scan.calls.push(call),
            AstKind::AssignmentExpression(assignment) => scan.assignments.push(assignment),
            AstKind::VariableDeclarator(variable) => {
                if let Some(symbol) = local(model, &variable.id) {
                    if let Some(value) = variable.init.as_ref() {
                        scan.values.push((symbol, value));
                    }
                }
            }
            _ => {}
        }
    }
    for assignment in &scan.assignments {
        if assignment.operator == AssignmentOperator::Assign {
            if let Some(symbol) = assignment_target(model, assignment) {
                scan.values.push((symbol, &assignment.right));
            }
        }
    }
    scan
}

fn create_canvas(
    model: &Model<'_, '_>,
    site: FunctionSite,
    calls: &[&CallExpression<'_>],
) -> Option<Symbol> {
    let mut canvas = None;
    for call in calls {
        let Some(receiver) = member(&call.callee, "createElement") else {
            continue;
        };
        let args = arguments(call)?;
        if args.len() != 1
            || !string(args[0], "canvas")
            || slot_of(model, receiver, site) != Some(1)
        {
            continue;
        }
        let target = call_binding(model, call)?;
        if !defined_in(model, site, target) || canvas.replace(target).is_some() {
            return None;
        }
    }
    canvas
}

fn holder_canvas(
    model: &Model<'_, '_>,
    assignments: &[&AssignmentExpression<'_>],
    canvas: Symbol,
) -> BTreeSet<Symbol> {
    let mut holders = BTreeSet::new();
    for assignment in assignments {
        let Some(target) = assignment_target(model, assignment) else {
            continue;
        };
        let Some(elements) = array_elements(&assignment.right) else {
            continue;
        };
        let [receiver, method] = elements.as_slice() else {
            continue;
        };
        if symbol(model, receiver) == Some(canvas)
            && member(method, "getContext")
                .is_some_and(|object| symbol(model, object) == Some(canvas))
        {
            holders.insert(target);
        }
    }
    holders
}

fn context_call(
    model: &Model<'_, '_>,
    call: &CallExpression<'_>,
    canvas: Symbol,
    holders: &BTreeSet<Symbol>,
) -> bool {
    let Some(args) = arguments(call) else {
        return false;
    };
    if let Some(receiver) = member(&call.callee, "getContext") {
        return args.len() == 1 && string(args[0], "2d") && symbol(model, receiver) == Some(canvas);
    }
    let Some(holder_call) = member(&call.callee, "call") else {
        return false;
    };
    let Some((holder_expr, index)) = computed(holder_call) else {
        return false;
    };
    let Some(holder) = symbol(model, holder_expr) else {
        return false;
    };
    if args.len() != 2
        || !string(args[1], "2d")
        || !number(index, 1.0)
        || !holders.contains(&holder)
    {
        return false;
    }
    let Some((this_expr, this_index)) = computed(args[0]) else {
        return false;
    };
    let Some(this_holder) = symbol(model, this_expr) else {
        return false;
    };
    this_holder == holder && number(this_index, 0.0)
}

/// All admitted symbols denote the same context. Setup is the one top-level
/// capability guard whose direct statements execute before the queue is built.
struct Contexts {
    symbols: BTreeSet<Symbol>,
    setup: oxc_span::Span,
    site: FunctionSite,
}

fn statement_value<'a>(
    model: &Model<'_, 'a>,
    statement: &'a Statement<'a>,
) -> Option<(Symbol, &'a Expression<'a>)> {
    match statement {
        Statement::VariableDeclaration(declaration) => {
            let [variable] = declaration.declarations.as_slice() else {
                return None;
            };
            Some((local(model, &variable.id)?, variable.init.as_ref()?))
        }
        Statement::ExpressionStatement(statement) => {
            let Expression::AssignmentExpression(assignment) =
                statement.expression.without_parentheses()
            else {
                return None;
            };
            if assignment.operator != AssignmentOperator::Assign {
                return None;
            }
            Some((assignment_target(model, assignment)?, &assignment.right))
        }
        _ => None,
    }
}

fn context_guard(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    contexts: &BTreeSet<Symbol>,
) -> bool {
    let Expression::LogicalExpression(test) = expression.without_parentheses() else {
        return false;
    };
    if test.operator.as_str() != "&&"
        || !symbol(model, &test.left).is_some_and(|s| contexts.contains(&s))
    {
        return false;
    }
    let Expression::BinaryExpression(check) = test.right.without_parentheses() else {
        return false;
    };
    let Expression::UnaryExpression(kind) = check.left.without_parentheses() else {
        return false;
    };
    check.operator.as_str() == "==="
        && kind.operator.as_str() == "typeof"
        && string(&check.right, "function")
        && member(&kind.argument, "measureText")
            .and_then(|receiver| symbol(model, receiver))
            .is_some_and(|s| contexts.contains(&s))
}

/// Only a call itself or the supported canvas capability && chain can produce
/// the context. Merely containing a call (comma, conditional, wrapper) cannot.
fn direct_context_value(
    model: &Model<'_, '_>,
    value: &Expression<'_>,
    context: &CallExpression<'_>,
    canvas: Symbol,
) -> bool {
    if value.without_parentheses().span() == context.span {
        return true;
    }
    let Expression::LogicalExpression(logical) = value.without_parentheses() else {
        return false;
    };
    if logical.operator.as_str() != "&&" {
        return false;
    }
    let capability = |value: &Expression<'_>| {
        symbol(model, value) == Some(canvas)
            || member(value, "getContext")
                .is_some_and(|receiver| symbol(model, receiver) == Some(canvas))
    };
    fn guards(model: &Model<'_, '_>, value: &Expression<'_>, canvas: Symbol) -> bool {
        if let Expression::LogicalExpression(logical) = value.without_parentheses() {
            return logical.operator.as_str() == "&&"
                && guards(model, &logical.left, canvas)
                && guards(model, &logical.right, canvas);
        }
        symbol(model, value) == Some(canvas)
            || member(value, "getContext")
                .is_some_and(|receiver| symbol(model, receiver) == Some(canvas))
    }
    (capability(&logical.left) || guards(model, &logical.left, canvas))
        && direct_context_value(model, &logical.right, context, canvas)
}

fn context_symbols(
    model: &Model<'_, '_>,
    site: FunctionSite,
    canvas: Symbol,
    assignments: &[&AssignmentExpression<'_>],
    calls: &[&CallExpression<'_>],
) -> Option<Contexts> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let statements = &function.body.as_ref()?.statements;
    let holders = holder_canvas(model, assignments, canvas);
    let mut matching = calls
        .iter()
        .copied()
        .filter(|call| context_call(model, call, canvas, &holders));
    let context = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let target = containing_binding(model, context)?;
    if !defined_in(model, site, target) {
        return None;
    }
    let canvas_origin = statements
        .iter()
        .find_map(|statement| statement_value(model, statement).filter(|(s, _)| *s == canvas))?;
    if call(canvas_origin.1).is_none_or(|call| member(&call.callee, "createElement").is_none()) {
        return None;
    }
    let mut ready = None;
    for statement in statements {
        if let Some((found, value)) = statement_value(model, statement) {
            if found == target
                && single_origin(model, target)
                && single_origin(model, canvas)
                && direct_context_value(model, value, context, canvas)
                && canvas_origin.1.span().end < value.span().start
            {
                ready = Some(statement.span().end);
            }
        }
        // Cached holder form reuses canvas and method bindings. Its only write
        // is the exact guarded conversion; arbitrary historical writes fail.
        let Statement::IfStatement(guard) = statement else {
            continue;
        };
        if guard.alternate.is_some() || symbol(model, &guard.test) != Some(target) {
            continue;
        }
        let Statement::BlockStatement(block) = &guard.consequent else {
            continue;
        };
        let [holder, invocation] = block.body.as_slice() else {
            continue;
        };
        let Some((holder_target, holder_value)) = statement_value(model, holder) else {
            continue;
        };
        let Some((context_target, context_value)) = statement_value(model, invocation) else {
            continue;
        };
        let Some(initial) = model.initializer(target) else {
            continue;
        };
        let Expression::LogicalExpression(capability) = initial.without_parentheses() else {
            continue;
        };
        let Some(elements) = array_elements(holder_value) else {
            continue;
        };
        let [object, method] = elements.as_slice() else {
            continue;
        };
        if holder_target != canvas
            || context_target != target
            || context_value.without_parentheses().span() != context.span
            || model.binding_writes(canvas) != 1
            || model.binding_writes(target) != 1
            || capability.operator.as_str() != "&&"
            || symbol(model, &capability.left) != Some(canvas)
            || !member(&capability.right, "getContext")
                .is_some_and(|receiver| symbol(model, receiver) == Some(canvas))
            || symbol(model, object) != Some(canvas)
            || !member(method, "getContext")
                .is_some_and(|receiver| symbol(model, receiver) == Some(canvas))
            || !statements.iter().any(|s| {
                statement_value(model, s)
                    .is_some_and(|(s, value)| s == target && value.span() == initial.span())
            })
            || canvas_origin.1.span().end >= initial.span().start
            || initial.span().end >= guard.span.start
        {
            continue;
        }
        ready = Some(guard.span.end);
    }
    let ready = ready?;
    let mut contexts = BTreeSet::from([target]);
    for statement in statements {
        if statement.span().start < ready {
            continue;
        }
        if let Some((alias, value)) = statement_value(model, statement) {
            if single_origin(model, alias)
                && defined_in(model, site, alias)
                && symbol(model, value).is_some_and(|source| contexts.contains(&source))
            {
                contexts.insert(alias);
            }
        }
        let Statement::IfStatement(guard) = statement else {
            continue;
        };
        if guard.alternate.is_some() || !context_guard(model, &guard.test, &contexts) {
            continue;
        }
        let Statement::BlockStatement(block) = &guard.consequent else {
            continue;
        };
        return Some(Contexts {
            symbols: contexts,
            setup: block.span,
            site,
        });
    }
    None
}

fn context_is(model: &Model<'_, '_>, expression: &Expression<'_>, contexts: &Contexts) -> bool {
    symbol(model, expression).is_some_and(|symbol| contexts.symbols.contains(&symbol))
}

fn width_call(model: &Model<'_, '_>, expression: &Expression<'_>, contexts: &Contexts) -> bool {
    let width = member(expression, "width").and_then(call);
    let Some(measure) = width else { return false };
    let Some(receiver) = member(&measure.callee, "measureText") else {
        return false;
    };
    let Some(args) = arguments(measure) else {
        return false;
    };
    args.len() == 1 && string(args[0], SENTINEL) && context_is(model, receiver, contexts)
}

fn fixed_font(expression: &Expression<'_>, family: &str) -> bool {
    string(expression, &format!("50px  {family}"))
}

fn dynamic_font(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    font: Symbol,
    family: &str,
) -> bool {
    let Expression::BinaryExpression(outer) = expression.without_parentheses() else {
        return false;
    };
    if outer.operator.as_str() != "+" || !string(&outer.right, family) {
        return false;
    }
    let Expression::BinaryExpression(prefix) = outer.left.without_parentheses() else {
        return false;
    };
    if prefix.operator.as_str() != "+" || !string(&prefix.left, "50px ") {
        return false;
    }
    let Expression::ConditionalExpression(conditional) = prefix.right.without_parentheses() else {
        return false;
    };
    let Expression::BinaryExpression(test) = conditional.test.without_parentheses() else {
        return false;
    };
    let is_null = |expression: &Expression<'_>| {
        matches!(expression.without_parentheses(), Expression::NullLiteral(_))
    };
    if !matches!(test.operator.as_str(), "==" | "===")
        || !((symbol(model, &test.left) == Some(font) && is_null(&test.right))
            || (symbol(model, &test.right) == Some(font) && is_null(&test.left)))
    {
        return false;
    }
    if !string(&conditional.consequent, "") {
        return false;
    }
    let Expression::BinaryExpression(non_null) = conditional.alternate.without_parentheses() else {
        return false;
    };
    let Expression::BinaryExpression(prefix) = non_null.left.without_parentheses() else {
        return false;
    };
    non_null.operator.as_str() == "+"
        && string(&non_null.right, "', ")
        && prefix.operator.as_str() == "+"
        && string(&prefix.left, "'")
        && symbol(model, &prefix.right) == Some(font)
}

fn baselines(
    model: &Model<'_, '_>,
    contexts: &Contexts,
    assignments: &[&AssignmentExpression<'_>],
) -> Option<(BTreeSet<Symbol>, BTreeSet<Symbol>)> {
    let block = model.nodes.iter().find_map(|node| match node.kind() {
        AstKind::BlockStatement(block) if block.span == contexts.setup => Some(block),
        _ => None,
    })?;
    // Potential aliases can invalidate a font even when their own origin is
    // too weak to justify a measurement. They never establish positive facts.
    let values = scan(model, contexts.site).values;
    let mut receivers = contexts.symbols.clone();
    for _ in 0..8 {
        let before = receivers.len();
        for (target, value) in &values {
            if symbol(model, value).is_some_and(|source| receivers.contains(&source)) {
                receivers.insert(*target);
            }
        }
        if before == receivers.len() {
            break;
        }
    }
    let may_be_context =
        |value: &Expression<'_>| symbol(model, value).is_some_and(|s| receivers.contains(&s));
    let mut sans = BTreeSet::new();
    let mut serif = BTreeSet::new();
    for statement in &block.body {
        let Some((target, value)) = statement_value(model, statement) else {
            continue;
        };
        // A baseline must be the reaching value, not one historical write.
        if !single_origin(model, target) || !defined_in(model, contexts.site, target) {
            continue;
        }
        if !width_call(model, value, contexts) {
            if let Some(source) = symbol(model, value) {
                if sans.contains(&source) {
                    sans.insert(target);
                }
                if serif.contains(&source) {
                    serif.insert(target);
                }
            }
            continue;
        }
        // Alignment belongs to this same reaching context, just as the font
        // does. A sibling/dormant write or a historical "top" followed by a
        // mutation cannot establish the measurement setup.
        let alignment = assignments
            .iter()
            .copied()
            .filter(|assignment| {
                if assignment.span.start >= value.span().start {
                    return false;
                }
                match &assignment.left {
                    AssignmentTarget::StaticMemberExpression(member) => {
                        member.property.name == "textBaseline" && may_be_context(&member.object)
                    }
                    AssignmentTarget::ComputedMemberExpression(member) => {
                        may_be_context(&member.object)
                    }
                    _ => false,
                }
            })
            .max_by_key(|assignment| assignment.span.start);
        let Some(alignment) = alignment else { continue };
        if alignment.operator != AssignmentOperator::Assign || !string(&alignment.right, "top")
            || !target_member(&alignment.left, "textBaseline").is_some_and(|receiver| context_is(model, receiver, contexts))
            || !block.body.iter().any(|statement| matches!(statement, Statement::ExpressionStatement(statement) if statement.expression.span() == alignment.span)) { continue }
        let alignment_changed = model.nodes.iter().any(|node| {
            if !model
                .owning_site(node.id())
                .is_some_and(|owner| owner.span == contexts.site.span)
            {
                return false;
            }
            match node.kind() {
                AstKind::UpdateExpression(update)
                    if alignment.span.end < update.span.start
                        && update.span.start < value.span().start =>
                {
                    match &update.argument {
                        SimpleAssignmentTarget::StaticMemberExpression(member) => {
                            member.property.name == "textBaseline" && may_be_context(&member.object)
                        }
                        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                            may_be_context(&member.object)
                        }
                        _ => false,
                    }
                }
                AstKind::UnaryExpression(unary)
                    if unary.operator.as_str() == "delete"
                        && alignment.span.end < unary.span.start
                        && unary.span.start < value.span().start =>
                {
                    member(&unary.argument, "textBaseline").is_some_and(may_be_context)
                        || computed(&unary.argument)
                            .is_some_and(|(object, _)| may_be_context(object))
                }
                _ => false,
            }
        });
        if alignment_changed {
            continue;
        }
        // Include unknown and conditional property writes as invalidators. A
        // font write only establishes a family when it is a direct preceding
        // statement in this same capability block, on this same context.
        let latest = assignments
            .iter()
            .copied()
            .filter(|assignment| {
                if assignment.span.start >= value.span().start {
                    return false;
                }
                match &assignment.left {
                    AssignmentTarget::StaticMemberExpression(member) => {
                        member.property.name == "font" && may_be_context(&member.object)
                    }
                    AssignmentTarget::ComputedMemberExpression(member) => {
                        may_be_context(&member.object)
                    }
                    _ => false,
                }
            })
            .max_by_key(|assignment| assignment.span.start);
        let Some(font) = latest else { continue };
        if font.operator != AssignmentOperator::Assign
            || !target_member(&font.left, "font").is_some_and(|receiver| context_is(model, receiver, contexts))
            || !block.body.iter().any(|statement| matches!(statement, Statement::ExpressionStatement(statement) if statement.expression.span() == font.span)) { continue }
        let changed = model.nodes.iter().any(|node| {
            if !model
                .owning_site(node.id())
                .is_some_and(|owner| owner.span == contexts.site.span)
            {
                return false;
            }
            match node.kind() {
                AstKind::UpdateExpression(update)
                    if font.span.end < update.span.start
                        && update.span.start < value.span().start =>
                {
                    match &update.argument {
                        SimpleAssignmentTarget::StaticMemberExpression(member) => {
                            member.property.name == "font" && may_be_context(&member.object)
                        }
                        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                            may_be_context(&member.object)
                        }
                        _ => false,
                    }
                }
                AstKind::UnaryExpression(unary)
                    if unary.operator.as_str() == "delete"
                        && font.span.end < unary.span.start
                        && unary.span.start < value.span().start =>
                {
                    member(&unary.argument, "font").is_some_and(may_be_context)
                        || computed(&unary.argument)
                            .is_some_and(|(object, _)| may_be_context(object))
                }
                _ => false,
            }
        });
        if changed {
            continue;
        }
        if fixed_font(&font.right, "sans-serif") {
            sans.insert(target);
        }
        if fixed_font(&font.right, "serif") {
            serif.insert(target);
        }
    }
    (!sans.is_empty() && !serif.is_empty()).then_some((sans, serif))
}

fn comparison_to_baseline(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    contexts: &Contexts,
    baselines: &(BTreeSet<Symbol>, BTreeSet<Symbol>),
) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    if !matches!(binary.operator.as_str(), "!=" | "!==") {
        return false;
    }
    let baseline = |expression: &Expression<'_>| {
        symbol(model, expression)
            .is_some_and(|symbol| baselines.0.contains(&symbol) || baselines.1.contains(&symbol))
    };
    (width_call(model, &binary.left, contexts) && baseline(&binary.right))
        || (width_call(model, &binary.right, contexts) && baseline(&binary.left))
}

/// Resolve at most eight immutable local origins. An origin must be an entire
/// top-level declaration or plain assignment in this callback, before its use;
/// a write hidden in a branch/loop/conditional is deliberately not an origin.
fn callback_origin<'a>(
    model: &Model<'_, 'a>,
    callback: FunctionSite,
    target: Symbol,
    before: u32,
) -> Option<&'a Expression<'a>> {
    if !single_origin(model, target)
        || !defined_in(model, callback, target)
        || !model.scoping.symbol_redeclarations(target).is_empty()
    {
        return None;
    }
    let AstKind::ArrowFunctionExpression(arrow) = model.nodes.kind(callback.node_id) else {
        return None;
    };
    for statement in &arrow.body.statements {
        if statement.span().end >= before {
            continue;
        }
        match statement {
            Statement::VariableDeclaration(declaration) => {
                for variable in &declaration.declarations {
                    if local(model, &variable.id) == Some(target) {
                        if let Some(value) = variable.init.as_ref() {
                            return Some(value);
                        }
                    }
                }
            }
            Statement::ExpressionStatement(statement) => {
                if let Expression::AssignmentExpression(assignment) = &statement.expression {
                    if assignment.operator == AssignmentOperator::Assign
                        && assignment_target(model, assignment) == Some(target)
                    {
                        return Some(&assignment.right);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn callback_root(
    model: &Model<'_, '_>,
    callback: FunctionSite,
    expression: &Expression<'_>,
    output: Symbol,
) -> bool {
    let mut current = expression;
    for _ in 0..8 {
        let Some(found) = symbol(model, current) else {
            return false;
        };
        if found == output {
            return true;
        }
        let Some(value) = callback_origin(model, callback, found, current.span().start) else {
            return false;
        };
        current = value;
    }
    false
}

fn push_origin(
    model: &Model<'_, '_>,
    callback: FunctionSite,
    expression: &Expression<'_>,
    output: Symbol,
) -> bool {
    let mut current = expression;
    for _ in 0..8 {
        if let Some(receiver) = member(current, "push") {
            return callback_root(model, callback, receiver, output);
        }
        let Some(found) = symbol(model, current) else {
            return false;
        };
        let Some(value) = callback_origin(model, callback, found, current.span().start) else {
            return false;
        };
        current = value;
    }
    false
}

/// The pushed value must be assigned on both arms of the immediately preceding
/// font comparison, with no other writes or initializer. A literal elsewhere
/// in the callback is not proof of a derived presence result.
fn derived_boolean(
    model: &Model<'_, '_>,
    callback: FunctionSite,
    value: &Expression<'_>,
    push: &CallExpression<'_>,
    contexts: &Contexts,
    baselines: &(BTreeSet<Symbol>, BTreeSet<Symbol>),
) -> bool {
    let Some(target) = symbol(model, value) else {
        return false;
    };
    if model.initializer(target).is_some() || model.binding_writes(target) != 2 {
        return false;
    }
    let AstKind::ArrowFunctionExpression(arrow) = model.nodes.kind(callback.node_id) else {
        return false;
    };
    let Some(index) = arrow.body.statements.iter().position(|statement| statement.span() == push.span || matches!(statement, Statement::ExpressionStatement(statement) if statement.expression.span() == push.span)) else { return false };
    let Some(Statement::IfStatement(branch)) = index
        .checked_sub(1)
        .and_then(|index| arrow.body.statements.get(index))
    else {
        return false;
    };
    let [font_pattern, _] = arrow.params.items.as_slice() else {
        return false;
    };
    let Some(font) = formal(model, &font_pattern.pattern) else {
        return false;
    };
    let font_assignment = |statement: &ExpressionStatement<'_>, family: &str| {
        let Expression::AssignmentExpression(assignment) = &statement.expression else {
            return false;
        };
        assignment.operator == AssignmentOperator::Assign
            && target_member(&assignment.left, "font")
                .is_some_and(|receiver| context_is(model, receiver, contexts))
            && dynamic_font(model, &assignment.right, font, family)
    };
    let Some(Statement::ExpressionStatement(sans_font)) = index
        .checked_sub(3)
        .and_then(|index| arrow.body.statements.get(index))
    else {
        return false;
    };
    if !font_assignment(sans_font, " sans-serif")
        || !comparison_to_baseline(
            model,
            &branch.test,
            contexts,
            &(baselines.0.clone(), baselines.0.clone()),
        )
    {
        return false;
    }
    let Statement::BlockStatement(yes) = &branch.consequent else {
        return false;
    };
    let Some(Statement::BlockStatement(no)) = branch.alternate.as_ref() else {
        return false;
    };
    let [Statement::ExpressionStatement(yes)] = yes.body.as_slice() else {
        return false;
    };
    let [Statement::ExpressionStatement(serif_font), Statement::ExpressionStatement(no)] =
        no.body.as_slice()
    else {
        return false;
    };
    let Expression::AssignmentExpression(yes) = &yes.expression else {
        return false;
    };
    let Expression::AssignmentExpression(no) = &no.expression else {
        return false;
    };
    yes.operator == AssignmentOperator::Assign
        && no.operator == AssignmentOperator::Assign
        && assignment_target(model, yes) == Some(target)
        && assignment_target(model, no) == Some(target)
        && matches!(yes.right.without_parentheses(), Expression::BooleanLiteral(flag) if flag.value)
        && font_assignment(serif_font, " serif")
        && comparison_to_baseline(
            model,
            &no.right,
            contexts,
            &(baselines.1.clone(), baselines.1.clone()),
        )
}

fn callback_proven(
    model: &Model<'_, '_>,
    callback: FunctionSite,
    contexts: &Contexts,
    baselines: &(BTreeSet<Symbol>, BTreeSet<Symbol>),
    output: Symbol,
) -> bool {
    let AstKind::ArrowFunctionExpression(arrow) = model.nodes.kind(callback.node_id) else {
        return false;
    };
    if arrow.r#async
        || arrow.params.rest.is_some()
        || arrow.params.items.len() != 2
        || !arrow.body.directives.is_empty()
    {
        return false;
    }
    let [font_pattern, done_pattern] = arrow.params.items.as_slice() else {
        return false;
    };
    if formal(model, &font_pattern.pattern).is_none() {
        return false;
    }
    let Some(done) = formal(model, &done_pattern.pattern) else {
        return false;
    };
    let nested = scan(model, callback);
    let mut pushed = false;
    let mut completions = 0;
    for call in &nested.calls {
        let Some(args) = arguments(call) else {
            return false;
        };
        if let Some(receiver) = member(&call.callee, "push") {
            if args.len() == 1 && callback_root(model, callback, receiver, output) {
                pushed = derived_boolean(model, callback, args[0], call, contexts, baselines);
            }
        }
        if let Some(receiver) = member(&call.callee, "call") {
            if args.len() == 2 && callback_root(model, callback, args[0], output) {
                if derived_boolean(model, callback, args[1], call, contexts, baselines)
                    && push_origin(model, callback, receiver, output)
                {
                    pushed = true;
                }
            }
        }
        if slot_of(model, &call.callee, callback) == Some(1)
            && symbol(model, &call.callee) == Some(done)
            && args.is_empty()
        {
            completions += 1;
        }
    }
    pushed && completions == 1
}

fn on_empty_sink(
    model: &Model<'_, '_>,
    site: FunctionSite,
    callback: FunctionSite,
    output: Symbol,
) -> bool {
    let AstKind::ArrowFunctionExpression(arrow) = model.nodes.kind(callback.node_id) else {
        return false;
    };
    if arrow.params.rest.is_some()
        || !arrow.params.items.is_empty()
        || !arrow.body.directives.is_empty()
    {
        return false;
    }
    let nested = scan(model, callback);
    let [call] = nested.calls.as_slice() else {
        return false;
    };
    let Some(args) = arguments(call) else {
        return false;
    };
    args.len() == 1
        && root_symbol(model, args[0]) == Some(output)
        && slot_of(model, &call.callee, site) == Some(2)
}

fn teardown_proven(model: &Model<'_, '_>, site: FunctionSite, queue: Symbol) -> bool {
    let mut returned = None;
    for node in model.nodes.iter() {
        if !model
            .owning_site(node.id())
            .is_some_and(|owner| owner.span == site.span)
        {
            continue;
        }
        let AstKind::ReturnStatement(statement) = node.kind() else {
            continue;
        };
        let Some(argument) = statement.argument.as_ref() else {
            return false;
        };
        if returned
            .replace(model.function_from_expression(argument))
            .is_some()
        {
            return false;
        }
    }
    let Some(Some(teardown)) = returned else {
        return false;
    };
    if model
        .enclosing_function(teardown.node_id)
        .is_none_or(|parent| parent.span != site.span)
    {
        return false;
    }
    let AstKind::Function(function) = model.nodes.kind(teardown.node_id) else {
        return false;
    };
    let Some(body) = function.body.as_ref() else {
        return false;
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
        || !body.directives.is_empty()
    {
        return false;
    }
    let [Statement::IfStatement(guard)] = body.statements.as_slice() else {
        return false;
    };
    if guard.alternate.is_some() || root_symbol(model, &guard.test) != Some(queue) {
        return false;
    }
    let Statement::BlockStatement(block) = &guard.consequent else {
        return false;
    };
    let [Statement::ExpressionStatement(statement)] = block.body.as_slice() else {
        return false;
    };
    let Some(empty) = call(&statement.expression) else {
        return false;
    };
    empty.arguments.is_empty()
        && member(&empty.callee, "empty")
            .is_some_and(|receiver| root_symbol(model, receiver) == Some(queue))
}

fn prove(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return false;
    };
    if function.r#async
        || function.generator
        || function
            .body
            .as_ref()
            .is_none_or(|body| !body.directives.is_empty())
        || plain_formals(model, function).is_none()
    {
        return false;
    }
    let outer = scan(model, site);
    let Some(canvas) = create_canvas(model, site, &outer.calls) else {
        return false;
    };
    let Some(contexts) = context_symbols(model, site, canvas, &outer.assignments, &outer.calls)
    else {
        return false;
    };
    let Some(baselines) = baselines(model, &contexts, &outer.assignments) else {
        return false;
    };
    let Some(output) = outer.values.iter().find_map(|(symbol, value)| {
        (empty_array(value) && single_origin(model, *symbol) && defined_in(model, site, *symbol))
            .then_some(*symbol)
    }) else {
        return false;
    };
    let mut queue = None;
    for call in &outer.calls {
        let Some(args) = arguments(call) else {
            return false;
        };
        if args.len() != 2 {
            continue;
        }
        let Some(config) = member(args[1], "fonts") else {
            continue;
        };
        if slot_of(model, config, site) != Some(0) {
            continue;
        }
        let Some(callback) = model.function_from_expression(args[0]) else {
            continue;
        };
        if callback.span == site.span
            || model
                .enclosing_function(callback.node_id)
                .is_none_or(|parent| parent.span != site.span)
        {
            continue;
        }
        let Some(factory) = symbol(model, &call.callee) else {
            continue;
        };
        if !captured(model, site, factory) {
            continue;
        }
        let Some(found_queue) = call_binding(model, call) else {
            continue;
        };
        if contexts.setup.contains_inclusive(call.span)
            && model.nodes.iter().any(|node| matches!(node.kind(), AstKind::BlockStatement(block) if block.span == contexts.setup && block.body.iter().any(|statement| statement_value(model, statement).is_some_and(|(target, value)| target == found_queue && value.span() == call.span))))
            && baselines.0.iter().chain(&baselines.1).all(|symbol| model.symbol_value(*symbol).is_some_and(|value| value.span().end < call.span.start))
            && single_origin(model, found_queue) && defined_in(model, site, found_queue) && callback_proven(model, callback, &contexts, &baselines, output) { queue = Some(found_queue); break }
    }
    let Some(queue) = queue else { return false };
    let mut started = false;
    let mut empty_callback = None;
    for call in &outer.calls {
        if member(&call.callee, "start")
            .is_some_and(|receiver| root_symbol(model, receiver) == Some(queue))
            && call.arguments.is_empty()
        {
            started = true;
        }
        if member(&call.callee, "onEmpty")
            .is_some_and(|receiver| root_symbol(model, receiver) == Some(queue))
        {
            empty_callback = arguments(call)
                .filter(|args| args.len() == 1)
                .and_then(|args| model.function_from_expression(args[0]));
        }
    }
    started
        && empty_callback.is_some_and(|callback| on_empty_sink(model, site, callback, output))
        && teardown_proven(model, site, queue)
}

/// The canonical label of the owned six-formal collector. Its returned-one
/// factory parent is named by the existing `returned_function_factory`
/// inheritance pass (`create` + the collector name), so only the collector is
/// proved here and the sampler plus both wrappers stay in one family module.
const COLLECTOR_NAME: &str = "collectFontMetrics";
const COLLECTOR_EVALUATOR: &str = "shared_font_metric_collector";

/// `Math.random()` is scaled by 2^32 before the unsigned coercion.
const SEED_SCALE: f64 = 4_294_967_296.0;

fn path_is(model: &Model<'_, '_>, expression: &Expression<'_>, path: &[&str]) -> bool {
    let actual = expression_path(model, expression.without_parentheses());
    actual.len() == path.len()
        && actual
            .iter()
            .zip(path)
            .all(|(actual, expected)| actual.as_str() == *expected)
}

fn random_product(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(product) = expression.without_parentheses() else {
        return false;
    };
    if product.operator.as_str() != "*" {
        return false;
    }
    let random = if number(&product.left, SEED_SCALE) {
        &product.right
    } else if number(&product.right, SEED_SCALE) {
        &product.left
    } else {
        return false;
    };
    let Some(random) = call(random) else {
        return false;
    };
    random.arguments.is_empty() && path_is(model, &random.callee, &["Math", "random"])
}

fn seed_word(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    matches!(binary.operator.as_str(), "|" | ">>>")
        && number(&binary.right, 0.0)
        && random_product(model, &binary.left)
}

fn seed_reference(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    if symbol(model, expression) == Some(seed) {
        return true;
    }
    matches!(expression.without_parentheses(), Expression::BinaryExpression(binary)
        if binary.operator.as_str() == "|" && symbol(model, &binary.left) == Some(seed) && number(&binary.right, 0.0))
}

fn lane(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    match expression.without_parentheses() {
        Expression::NumericLiteral(_) => true,
        Expression::Identifier(_) => symbol(model, expression) == Some(seed),
        Expression::UnaryExpression(unary) => {
            matches!(unary.operator.as_str(), "-" | "+" | "~") && lane(model, &unary.argument, seed)
        }
        Expression::BinaryExpression(binary) => {
            matches!(
                binary.operator.as_str(),
                ">>" | ">>>" | "&" | "|" | "^" | "<<" | "*" | "+" | "-" | "/"
            ) && lane(model, &binary.left, seed)
                && lane(model, &binary.right, seed)
        }
        _ => false,
    }
}

fn header_lanes(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return false;
    };
    let mut items = Vec::new();
    for element in &array.elements {
        let Some(item) = element.as_expression() else {
            return false;
        };
        items.push(item);
    }
    items.len() == 4 && items.iter().copied().all(|item| lane(model, item, seed))
}

fn xor_seed(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    if binary.operator.as_str() != "^" {
        return false;
    }
    let literal = |value: &Expression<'_>| {
        matches!(value.without_parentheses(), Expression::NumericLiteral(_))
    };
    (seed_reference(model, &binary.left, seed) && literal(&binary.right))
        || (seed_reference(model, &binary.right, seed) && literal(&binary.left))
}

fn seeded_prng(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    seed: Symbol,
) -> bool {
    let Some(prng) = call(expression) else {
        return false;
    };
    let Some(args) = arguments(prng) else {
        return false;
    };
    let [argument] = args.as_slice() else {
        return false;
    };
    xor_seed(model, argument, seed)
        && symbol(model, &prng.callee).is_some_and(|helper| captured(model, site, helper))
}

fn length_of(model: &Model<'_, '_>, expression: &Expression<'_>, target: Symbol) -> bool {
    member(expression, "length").is_some_and(|object| symbol(model, object) == Some(target))
}

fn version_of(model: &Model<'_, '_>, expression: &Expression<'_>, config: Symbol) -> bool {
    member(expression, "version").is_some_and(|object| symbol(model, object) == Some(config))
}

fn prng_draw(model: &Model<'_, '_>, expression: &Expression<'_>, prng: Symbol) -> bool {
    call(expression)
        .is_some_and(|call| call.arguments.is_empty() && symbol(model, &call.callee) == Some(prng))
}

fn or_pair(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    left: Symbol,
    right: Symbol,
) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    binary.operator.as_str() == "|"
        && symbol(model, &binary.left) == Some(left)
        && symbol(model, &binary.right) == Some(right)
}

fn distinct(symbols: &[Symbol]) -> bool {
    for (index, left) in symbols.iter().enumerate() {
        if symbols[index + 1..].contains(left) {
            return false;
        }
    }
    true
}

fn statement_call<'a>(statement: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    call(&statement.expression)
}

fn collector_declarator<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &'a Statement<'a>,
) -> Option<(Symbol, &'a Expression<'a>)> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
        return None;
    };
    let item = identifier.symbol_id.get()?;
    if !site
        .span
        .contains_inclusive(model.scoping.symbol_span(item))
        || !model.scoping.symbol_redeclarations(item).is_empty()
    {
        return None;
    }
    Some((item, variable.init.as_ref()?))
}

fn document_of(model: &Model<'_, '_>, site: FunctionSite, expression: &Expression<'_>) -> bool {
    let Some(global) = member(expression, "document") else {
        return false;
    };
    let Some(base) = member(global, "global") else {
        return false;
    };
    model.site_parameter_slot(base.without_parentheses(), site) == Some(1)
}

fn update_symbol(model: &Model<'_, '_>, target: &SimpleAssignmentTarget<'_>) -> Option<Symbol> {
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

fn reset_pair(model: &Model<'_, '_>, statement: &Statement<'_>, target: Symbol) -> bool {
    let Statement::ExpressionStatement(statement) = statement else {
        return false;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return false;
    };
    assignment.operator == AssignmentOperator::Assign
        && assignment_target(model, assignment) == Some(target)
        && number(&assignment.right, 0.0)
}

fn full_byte(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    header: Symbol,
    accumulator: Symbol,
    chunk: Option<Symbol>,
    byte_cursor: Symbol,
    prng: Symbol,
) -> bool {
    let Statement::ExpressionStatement(statement) = statement else {
        return false;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return false;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return false;
    }
    let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
        return false;
    };
    if symbol(model, &target.object) != Some(header) {
        return false;
    }
    let Expression::UpdateExpression(cursor) = target.expression.without_parentheses() else {
        return false;
    };
    if cursor.operator != UpdateOperator::Increment
        || !cursor.prefix
        || update_symbol(model, &cursor.argument) != Some(byte_cursor)
    {
        return false;
    }
    let Expression::BinaryExpression(xor) = assignment.right.without_parentheses() else {
        return false;
    };
    xor.operator.as_str() == "^"
        && chunk.map_or_else(
            || symbol(model, &xor.left) == Some(accumulator),
            |chunk| or_pair(model, &xor.left, accumulator, chunk),
        )
        && prng_draw(model, &xor.right, prng)
}

fn previous_index(model: &Model<'_, '_>, expression: &Expression<'_>, index: Symbol) -> bool {
    matches!(expression.without_parentheses(), Expression::BinaryExpression(binary)
        if binary.operator.as_str() == "-" && symbol(model, &binary.left) == Some(index) && number(&binary.right, 1.0))
}

fn loop_proof<'a>(
    model: &Model<'_, 'a>,
    callback: FunctionSite,
    statement: &'a Statement<'a>,
    result: Symbol,
    header: Symbol,
    index: Symbol,
    bit: Symbol,
    accumulator: Symbol,
    byte_cursor: Symbol,
    prng: Symbol,
) -> bool {
    let (block, reverse) = match statement {
        Statement::ForStatement(loop_) => {
            if loop_.init.is_some() {
                return false;
            }
            let Some(Expression::BinaryExpression(test)) =
                loop_.test.as_ref().map(Expression::without_parentheses)
            else {
                return false;
            };
            let Some(Expression::UpdateExpression(update)) =
                loop_.update.as_ref().map(Expression::without_parentheses)
            else {
                return false;
            };
            if test.operator.as_str() != "<"
                || symbol(model, &test.left) != Some(index)
                || !length_of(model, &test.right, result)
                || update.operator != UpdateOperator::Increment
                || update.prefix
                || update_symbol(model, &update.argument) != Some(index)
            {
                return false;
            }
            let Statement::BlockStatement(block) = &loop_.body else {
                return false;
            };
            (block, false)
        }
        Statement::WhileStatement(loop_) => {
            let Expression::BinaryExpression(test) = loop_.test.without_parentheses() else {
                return false;
            };
            if test.operator.as_str() != ">="
                || !previous_index(model, &test.left, index)
                || !number(&test.right, 0.0)
            {
                return false;
            }
            let Statement::BlockStatement(block) = &loop_.body else {
                return false;
            };
            (block, true)
        }
        _ => return false,
    };
    let statements = if reverse {
        let [body @ .., Statement::ExpressionStatement(decrement)] = block.body.as_slice() else {
            return false;
        };
        let Expression::UpdateExpression(decrement) = decrement.expression.without_parentheses()
        else {
            return false;
        };
        if decrement.operator != UpdateOperator::Decrement
            || !decrement.prefix
            || update_symbol(model, &decrement.argument) != Some(index)
        {
            return false;
        }
        body
    } else {
        block.body.as_slice()
    };
    let [chunk_statement, increment_statement, flush_statement] = statements else {
        return false;
    };
    let chunk_declaration = collector_declarator(model, callback, chunk_statement);
    let (chunk, chunk_value) = if let Some((chunk, value)) = chunk_declaration {
        (Some(chunk), value)
    } else {
        let Statement::ExpressionStatement(statement) = chunk_statement else {
            return false;
        };
        let Expression::AssignmentExpression(assignment) = &statement.expression else {
            return false;
        };
        if assignment.operator != AssignmentOperator::Assign
            || assignment_target(model, assignment) != Some(accumulator)
        {
            return false;
        }
        let Expression::BinaryExpression(or) = assignment.right.without_parentheses() else {
            return false;
        };
        if or.operator.as_str() != "|" || symbol(model, &or.left) != Some(accumulator) {
            return false;
        }
        (None, &or.right)
    };
    let Expression::BinaryExpression(shift) = chunk_value.without_parentheses() else {
        return false;
    };
    let Some((chunk_object, chunk_index)) = computed(&shift.left) else {
        return false;
    };
    if shift.operator.as_str() != "<<"
        || symbol(model, chunk_object) != Some(result)
        || !(if reverse {
            previous_index(model, chunk_index, index)
        } else {
            symbol(model, chunk_index) == Some(index)
        })
        || symbol(model, &shift.right) != Some(bit)
    {
        return false;
    }
    let Statement::ExpressionStatement(increment) = increment_statement else {
        return false;
    };
    let Expression::UpdateExpression(increment) = increment.expression.without_parentheses() else {
        return false;
    };
    if increment.operator != UpdateOperator::Increment
        || !increment.prefix
        || update_symbol(model, &increment.argument) != Some(bit)
    {
        return false;
    }
    let Statement::IfStatement(flush) = flush_statement else {
        return false;
    };
    let Expression::BinaryExpression(test) = flush.test.without_parentheses() else {
        return false;
    };
    if test.operator.as_str() != "==="
        || symbol(model, &test.left) != Some(bit)
        || !number(&test.right, 8.0)
    {
        return false;
    }
    let Statement::BlockStatement(full) = &flush.consequent else {
        return false;
    };
    let [flush_assignment, bit_reset, accumulator_reset] = full.body.as_slice() else {
        return false;
    };
    if !full_byte(
        model,
        flush_assignment,
        header,
        accumulator,
        chunk,
        byte_cursor,
        prng,
    ) || !((reset_pair(model, bit_reset, bit)
        && reset_pair(model, accumulator_reset, accumulator))
        || (reset_pair(model, bit_reset, accumulator) && reset_pair(model, accumulator_reset, bit)))
    {
        return false;
    }
    if chunk.is_none() {
        return flush.alternate.is_none();
    }
    let Some(Statement::BlockStatement(partial)) = flush.alternate.as_ref() else {
        return false;
    };
    let [accumulate] = partial.body.as_slice() else {
        return false;
    };
    let Statement::ExpressionStatement(accumulate) = accumulate else {
        return false;
    };
    let Expression::AssignmentExpression(assignment) = &accumulate.expression else {
        return false;
    };
    assignment.operator == AssignmentOperator::Assign
        && assignment_target(model, assignment) == Some(accumulator)
        && or_pair(model, &assignment.right, accumulator, chunk.unwrap())
}

fn partial_flush(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    header: Symbol,
    accumulator: Symbol,
    byte_cursor: Symbol,
    bit: Symbol,
    prng: Symbol,
    advance: bool,
) -> bool {
    let Statement::IfStatement(branch) = statement else {
        return false;
    };
    if branch.alternate.is_some() {
        return false;
    }
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return false;
    };
    if test.operator.as_str() != "!=="
        || symbol(model, &test.left) != Some(bit)
        || !number(&test.right, 0.0)
    {
        return false;
    }
    let Statement::BlockStatement(block) = &branch.consequent else {
        return false;
    };
    let [flush] = block.body.as_slice() else {
        return false;
    };
    let Statement::ExpressionStatement(flush) = flush else {
        return false;
    };
    let Expression::AssignmentExpression(assignment) = &flush.expression else {
        return false;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return false;
    }
    let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
        return false;
    };
    if symbol(model, &target.object) != Some(header) {
        return false;
    }
    if advance {
        return matches!(target.expression.without_parentheses(), Expression::UpdateExpression(cursor)
            if cursor.operator == UpdateOperator::Increment && cursor.prefix && update_symbol(model, &cursor.argument) == Some(byte_cursor))
            && matches!(assignment.right.without_parentheses(), Expression::BinaryExpression(xor)
                if xor.operator.as_str() == "^" && symbol(model, &xor.left) == Some(accumulator) && prng_draw(model, &xor.right, prng));
    }
    let Expression::BinaryExpression(position) = target.expression.without_parentheses() else {
        return false;
    };
    if position.operator.as_str() != "+"
        || symbol(model, &position.left) != Some(byte_cursor)
        || !number(&position.right, 1.0)
    {
        return false;
    }
    let Expression::BinaryExpression(xor) = assignment.right.without_parentheses() else {
        return false;
    };
    xor.operator.as_str() == "^"
        && symbol(model, &xor.left) == Some(accumulator)
        && prng_draw(model, &xor.right, prng)
}

fn sink_kind(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    observer: Symbol,
    header: Symbol,
) -> Option<&'static str> {
    let call = statement_call(statement)?;
    let args = arguments(call)?;
    if let Some(receiver) = member(&call.callee, "complete") {
        if symbol(model, receiver) == Some(observer) && args.is_empty() {
            return Some("complete");
        }
    }
    if let Some(receiver) = member(&call.callee, "next") {
        if symbol(model, receiver) != Some(observer) {
            return None;
        }
        let [array] = args.as_slice() else {
            return None;
        };
        let Expression::ArrayExpression(literal) = array.without_parentheses() else {
            return None;
        };
        let [first, second] = literal.elements.as_slice() else {
            return None;
        };
        let head = matches!(first.as_expression(), Some(value)
            if matches!(value.without_parentheses(), Expression::ArrayExpression(inner) if inner.elements.is_empty()));
        if head
            && second
                .as_expression()
                .and_then(|value| symbol(model, value))
                == Some(header)
        {
            return Some("next");
        }
    }
    None
}

fn empty_pair_sink(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    observer: Symbol,
    header: Symbol,
    head: Symbol,
) -> bool {
    let Some(next) = statement_call(statement) else {
        return false;
    };
    if !member(&next.callee, "next")
        .is_some_and(|receiver| symbol(model, receiver) == Some(observer))
    {
        return false;
    }
    let Some(args) = arguments(next) else {
        return false;
    };
    let [pair] = args.as_slice() else {
        return false;
    };
    let Some(elements) = array_elements(pair) else {
        return false;
    };
    let [first, second] = elements.as_slice() else {
        return false;
    };
    symbol(model, first) == Some(head) && symbol(model, second) == Some(header)
}

fn callback_proof<'a>(
    model: &Model<'_, 'a>,
    callback: FunctionSite,
    config: Symbol,
    document: Symbol,
    observer: Symbol,
) -> Option<()> {
    callback_proof_variant(model, callback, config, document, observer, false)
        .or_else(|| callback_proof_variant(model, callback, config, document, observer, true))
}

fn callback_proof_variant<'a>(
    model: &Model<'_, 'a>,
    callback: FunctionSite,
    config: Symbol,
    document: Symbol,
    observer: Symbol,
    lowered: bool,
) -> Option<()> {
    let AstKind::ArrowFunctionExpression(arrow) = model.nodes.kind(callback.node_id) else {
        return None;
    };
    if arrow.r#async
        || arrow.params.rest.is_some()
        || arrow.params.items.len() != 1
        || !arrow.body.directives.is_empty()
    {
        return None;
    }
    let [parameter] = arrow.params.items.as_slice() else {
        return None;
    };
    let result = formal(model, &parameter.pattern)?;
    let statements = arrow.body.statements.as_slice();
    let Some(loop_index) = statements.iter().position(|statement| {
        matches!(statement, Statement::ForStatement(_))
            || (lowered && matches!(statement, Statement::WhileStatement(_)))
    }) else {
        return None;
    };
    let (prefix, effects) = statements.split_at(loop_index);
    let mut declarations = Vec::new();
    let mut placeholder = None;
    for statement in prefix {
        if lowered
            && collector_declarator(model, callback, statement)
                .is_some_and(|(_, value)| empty_array(value))
        {
            let (target, _) = collector_declarator(model, callback, statement)?;
            // A fresh empty pair head may only be read at the final next call.
            if !single_origin(model, target)
                || model.scoping.get_resolved_references(target).count() != 1
                || placeholder.replace(target).is_some()
            {
                return None;
            }
        } else {
            declarations.push(statement);
        }
    }
    let declarations = declarations.as_slice();
    let [seed_statement, header_statement, prng_statement, rest @ ..] = declarations else {
        return None;
    };
    if rest.len() != 4 {
        return None;
    }
    let (seed, seed_value) = collector_declarator(model, callback, seed_statement)?;
    if !seed_word(model, seed_value) && !random_product(model, seed_value) {
        return None;
    }
    let phase_reuse = lowered && model.binding_writes(seed) == 1;
    if model.binding_writes(seed) != 0 && !phase_reuse {
        return None;
    }
    let (header, header_value) = collector_declarator(model, callback, header_statement)?;
    if !header_lanes(model, header_value, seed) {
        return None;
    }
    let (prng, prng_value) = if phase_reuse {
        // Exactly one straight-line transition, after the seed header has
        // consumed the random value. Later statements are fully matched as
        // PRNG contracts; no branch can retain the numeric seed.
        let (target, value) = statement_value(model, prng_statement)?;
        if target != seed || !matches!(prng_statement, Statement::ExpressionStatement(_)) {
            return None;
        }
        (target, value)
    } else {
        collector_declarator(model, callback, prng_statement)?
    };
    if !seeded_prng(model, callback, prng_value, seed)
        || header_value.span().end >= prng_value.span().start
    {
        return None;
    }
    let [loop_statement, partial_statement, tail @ ..] = effects else {
        return None;
    };
    let (version_statement, next_statement, complete_statement) = match tail {
        [next, complete] => (None, next, complete),
        [version, next, complete] => (Some(version), next, complete),
        _ => return None,
    };
    // Declaration order varies, but index is first, and the loop fixes the
    // bit counter and accumulator identities before their zero origins are read.
    let (index, index_value) = collector_declarator(model, callback, &rest[0])?;
    let reverse = matches!(loop_statement, Statement::WhileStatement(_));
    if !(if reverse {
        length_of(model, index_value, result)
    } else {
        number(index_value, 0.0)
    }) {
        return None;
    }
    let loop_body = match loop_statement {
        Statement::ForStatement(loop_) => &loop_.body,
        Statement::WhileStatement(loop_) => &loop_.body,
        _ => return None,
    };
    let Statement::BlockStatement(block) = loop_body else {
        return None;
    };
    let [_, Statement::ExpressionStatement(increment), _, ..] = block.body.as_slice() else {
        return None;
    };
    let Expression::UpdateExpression(increment) = increment.expression.without_parentheses() else {
        return None;
    };
    let bit = update_symbol(model, &increment.argument)?;
    let mut zeroes = Vec::new();
    let mut cursor = None;
    for statement in &rest[1..] {
        let (target, value) = collector_declarator(model, callback, statement)?;
        if number(value, 0.0) {
            zeroes.push(target)
        } else {
            let writer = cursor_write(
                model,
                value,
                config,
                result,
                prng,
                header,
                version_statement.is_some(),
                reverse.then_some(index),
            )?;
            if !captured(model, callback, writer)
                || writer == prng
                || cursor.replace((target, writer)).is_some()
            {
                return None;
            }
        }
    }
    if zeroes.len() != 2 || !zeroes.contains(&bit) {
        return None;
    }
    let accumulator = *zeroes.iter().find(|&&target| target != bit)?;
    let (byte_cursor, writer) = cursor?;
    if let Some(statement) = version_statement {
        let version = statement_call(statement)?;
        let values = arguments(version)?;
        let [p, v, h, c] = values.as_slice() else {
            return None;
        };
        if symbol(model, &version.callee) != Some(writer)
            || symbol(model, p) != Some(prng)
            || !version_of(model, v, config)
            || symbol(model, h) != Some(header)
            || symbol(model, c) != Some(byte_cursor)
        {
            return None;
        }
    }
    if !distinct(&[header, prng, index, bit, accumulator, byte_cursor])
        || (!phase_reuse && [header, prng, index, bit, accumulator, byte_cursor].contains(&seed))
    {
        return None;
    }
    if !loop_proof(
        model,
        callback,
        loop_statement,
        result,
        header,
        index,
        bit,
        accumulator,
        byte_cursor,
        prng,
    ) {
        return None;
    }
    if !partial_flush(
        model,
        partial_statement,
        header,
        accumulator,
        byte_cursor,
        bit,
        prng,
        version_statement.is_some(),
    ) {
        return None;
    }
    if !(sink_kind(model, next_statement, observer, header) == Some("next")
        || placeholder
            .is_some_and(|head| empty_pair_sink(model, next_statement, observer, header, head)))
        || sink_kind(model, complete_statement, observer, header) != Some("complete")
    {
        return None;
    }
    Some(())
}

/// `WRITER(prng, count, header, WRITER(prng, config.version, header, 3))`: both
/// writes go through one captured numeric writer over the same PRNG and header;
/// the inner cursor constant is any numeric literal. The other bounded form
/// writes count first, advances the partial-byte cursor, then writes version
/// using that exact cursor and the same captured writer.
fn cursor_write(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    config: Symbol,
    result: Symbol,
    prng: Symbol,
    header: Symbol,
    trailing_version: bool,
    count_index: Option<Symbol>,
) -> Option<Symbol> {
    let outer = call(expression)?;
    let writer = symbol(model, &outer.callee)?;
    let args = arguments(outer)?;
    let [first, second, third, fourth] = args.as_slice() else {
        return None;
    };
    if symbol(model, first) != Some(prng)
        || !(length_of(model, second, result)
            || count_index.is_some_and(|index| symbol(model, second) == Some(index)))
        || symbol(model, third) != Some(header)
    {
        return None;
    }
    if trailing_version {
        return matches!(fourth.without_parentheses(), Expression::NumericLiteral(_))
            .then_some(writer);
    }
    let inner = call(fourth)?;
    if symbol(model, &inner.callee) != Some(writer) {
        return None;
    }
    let inner_args = arguments(inner)?;
    let [inner_first, inner_second, inner_third, inner_fourth] = inner_args.as_slice() else {
        return None;
    };
    if symbol(model, inner_first) != Some(prng)
        || !version_of(model, inner_second, config)
        || symbol(model, inner_third) != Some(header)
        || !matches!(
            inner_fourth.without_parentheses(),
            Expression::NumericLiteral(_)
        )
    {
        return None;
    }
    Some(writer)
}

/// The start registration on formal slot 3: `slot3(start, true)` where `start`
/// stores the exact font-sampler call with the captured config, the resolved
/// document and the proven boolean callback as its cleanup value.
fn start_dispatch<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    statement: &'a Statement<'a>,
    cleanup: Symbol,
    document: Symbol,
    observer: Symbol,
) -> Option<()> {
    let register = statement_call(statement)?;
    if model.site_parameter_slot(register.callee.without_parentheses(), site) != Some(3) {
        return None;
    }
    if register.arguments.len() != 2 {
        return None;
    }
    let values = arguments(register)?;
    let [start, enabled] = values.as_slice() else {
        return None;
    };
    if !matches!(enabled.without_parentheses(), Expression::BooleanLiteral(flag) if flag.value) {
        return None;
    }
    let Expression::ArrowFunctionExpression(start) = start.without_parentheses() else {
        return None;
    };
    if start.r#async
        || start.params.rest.is_some()
        || !start.params.items.is_empty()
        || !start.body.directives.is_empty()
    {
        return None;
    }
    let [statement] = start.body.statements.as_slice() else {
        return None;
    };
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign
        || assignment_target(model, assignment) != Some(cleanup)
    {
        return None;
    }
    let sampler = call(&assignment.right)?;
    let values = arguments(sampler)?;
    let [config, target_document, callback] = values.as_slice() else {
        return None;
    };
    let config = symbol(model, config)?;
    if !captured(model, site, config) || symbol(model, target_document) != Some(document) {
        return None;
    }
    let target = super::stable_function(model, &sampler.callee, 0)?;
    if target.span == site.span || !prove(model, target) {
        return None;
    }
    let callback = model.function_from_expression(callback)?;
    if !site.span.contains_inclusive(callback.span) {
        return None;
    }
    callback_proof(model, callback, config, document, observer)
}

/// The teardown registration on formal slot 5: `slot5(teardown)` where the
/// callback calls the stored cleanup when it is a function.
fn teardown_dispatch<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    statement: &'a Statement<'a>,
    cleanup: Symbol,
) -> bool {
    let Some(register) = statement_call(statement) else {
        return false;
    };
    if register.arguments.len() != 1
        || model.site_parameter_slot(register.callee.without_parentheses(), site) != Some(5)
    {
        return false;
    }
    let Some(values) = arguments(register) else {
        return false;
    };
    let [teardown] = values.as_slice() else {
        return false;
    };
    let Expression::ArrowFunctionExpression(teardown) = teardown.without_parentheses() else {
        return false;
    };
    if teardown.r#async
        || teardown.params.rest.is_some()
        || !teardown.params.items.is_empty()
        || !teardown.body.directives.is_empty()
    {
        return false;
    }
    let [statement] = teardown.body.statements.as_slice() else {
        return false;
    };
    let Statement::IfStatement(branch) = statement else {
        return false;
    };
    if branch.alternate.is_some() {
        return false;
    }
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return false;
    };
    if test.operator.as_str() != "===" {
        return false;
    }
    let Expression::UnaryExpression(typeof_) = test.left.without_parentheses() else {
        return false;
    };
    if typeof_.operator.as_str() != "typeof" || symbol(model, &typeof_.argument) != Some(cleanup) {
        return false;
    }
    if !matches!(test.right.without_parentheses(), Expression::StringLiteral(value) if value.value == "function")
    {
        return false;
    }
    let Statement::BlockStatement(block) = &branch.consequent else {
        return false;
    };
    let [invoke] = block.body.as_slice() else {
        return false;
    };
    let Some(invoke) = statement_call(invoke) else {
        return false;
    };
    invoke.arguments.is_empty() && symbol(model, &invoke.callee) == Some(cleanup)
}

fn six_formals(model: &Model<'_, '_>, function: &Function<'_>) -> Option<[Symbol; 6]> {
    if function.params.rest.is_some() || function.params.items.len() != 6 {
        return None;
    }
    let [a, b, c, d, e, f] = function.params.items.as_slice() else {
        return None;
    };
    Some([
        formal(model, &a.pattern)?,
        formal(model, &b.pattern)?,
        formal(model, &c.pattern)?,
        formal(model, &d.pattern)?,
        formal(model, &e.pattern)?,
        formal(model, &f.pattern)?,
    ])
}

fn collector_proof_original<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    body: &'a FunctionBody<'a>,
) -> Option<()> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if function.r#async || function.generator {
        return None;
    }
    six_formals(model, function)?;
    let [observer_statement, document_statement, cleanup_statement, start_statement, teardown_statement] =
        body.statements.as_slice()
    else {
        return None;
    };
    let (observer, observer_value) = collector_declarator(model, site, observer_statement)?;
    if model.site_parameter_slot(observer_value.without_parentheses(), site) != Some(0) {
        return None;
    }
    let (document, document_value) = collector_declarator(model, site, document_statement)?;
    if !document_of(model, site, document_value) {
        return None;
    }
    let (cleanup, cleanup_value) = collector_declarator(model, site, cleanup_statement)?;
    if !matches!(
        cleanup_value.without_parentheses(),
        Expression::NullLiteral(_)
    ) {
        return None;
    }
    if !distinct(&[observer, document, cleanup]) {
        return None;
    }
    start_dispatch(model, site, start_statement, cleanup, document, observer)?;
    if !teardown_dispatch(model, site, teardown_statement, cleanup) {
        return None;
    }
    Some(())
}

/// Split declarations are admitted only as a straight-line prefix whose three
/// values are observer, document and null cleanup, in that order. Bare locals
/// must be initialized exactly once here; cleanup has one additional write in
/// the proven start callback. No unrelated prefix effect is ignored.
fn collector_proof_lowered<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    body: &'a FunctionBody<'a>,
) -> Option<()> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if function.r#async || function.generator {
        return None;
    }
    six_formals(model, function)?;
    let [prefix @ .., start, teardown] = body.statements.as_slice() else {
        return None;
    };
    let mut declared = BTreeSet::new();
    let mut bare = BTreeSet::new();
    let mut values = Vec::new();
    for statement in prefix {
        if let Statement::VariableDeclaration(declaration) = statement {
            for variable in &declaration.declarations {
                let target = local(model, &variable.id)?;
                if !declared.insert(target) {
                    return None;
                }
                if let Some(value) = variable.init.as_ref() {
                    values.push((target, value));
                } else {
                    bare.insert(target);
                }
            }
        } else {
            let (target, value) = statement_value(model, statement)?;
            if !bare.remove(&target) {
                return None;
            }
            values.push((target, value));
        }
    }
    let [(observer, observer_value), (document, document_value), (cleanup, cleanup_value)] =
        values.as_slice()
    else {
        return None;
    };
    if !bare.is_empty()
        || declared.len() != 3
        || !distinct(&[*observer, *document, *cleanup])
        || !single_origin(model, *observer)
        || !single_origin(model, *document)
        || model.site_parameter_slot(observer_value.without_parentheses(), site) != Some(0)
        || !document_of(model, site, document_value)
        || !matches!(
            cleanup_value.without_parentheses(),
            Expression::NullLiteral(_)
        )
        || model.binding_writes(*cleanup)
            != if model.initializer(*cleanup).is_some() {
                1
            } else {
                2
            }
    {
        return None;
    }
    start_dispatch(model, site, start, *cleanup, *document, *observer)?;
    teardown_dispatch(model, site, teardown, *cleanup).then_some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if prove(model, site) {
        return vec![proposal(
            NAME,
            EVALUATOR,
            "standalone captured-document canvas sampler: establishes sans-serif and serif measureText baselines for 0-_{w., queues the captured .fonts table, emits the boolean font-presence vector on queue empty, starts the queue, and returns its empty teardown",
            facts,
        )];
    }
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if collector_proof_original(model, site, body)
        .or_else(|| collector_proof_lowered(model, site, body))
        .is_none()
    {
        return Vec::new();
    }
    vec![proposal(
        COLLECTOR_NAME,
        COLLECTOR_EVALUATOR,
        "owned six-formal font-metric collector: a captured config; the document read from formal slot 1's .global.document; the already-proven font sampler invoked with the captured config, that document and a boolean-result callback; a start registration on formal slot 3 that stores the sampler result as the cleanup; the callback packs the sampler's boolean vector into a seeded bitset header, writes the result count and config.version through one captured numeric writer and one seeded PRNG, flushes full and partial bytes, and delivers the [[], header] pair plus completion to the slot 0 observer; and a teardown registration on formal slot 5 that calls the stored cleanup when it is a function. Its returned-one parent is named by the existing returned-function factory inheritance",
        facts,
    )]
}
