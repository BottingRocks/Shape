use super::*;

use crate::naming::support::installer_events::*;

// `using`/`await using` resource bindings are not plain local storage: they get
// a disposable runtime protocol and are never accepted here. The shared
// permissive helper only requires a single-declarator variable statement, so
// this stricter form is kept locally.
fn one_variable<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    if !matches!(
        declaration.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(declarator)
}

// A capture is a plain, immutable variable or function declaration outside the
// owned site. Parameters, catch bindings and loop targets are never captures
// here, and a use cannot precede the declaration that establishes it.
fn capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = symbol(model, e)?;
    let span = model.scoping.symbol_span(s);
    if !unchanged(model, s)
        || site.span.contains_inclusive(span)
        || span.end > site.span.start
        || model.formal_symbol_slot(s).is_some()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    match model.nodes.kind(declaration) {
        AstKind::Function(_) => Some(s),
        AstKind::VariableDeclarator(v) => {
            let valid = matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
                && matches!(
                    model.nodes.parent_kind(declaration),
                    AstKind::VariableDeclaration(d)
                        if matches!(
                            d.kind,
                            VariableDeclarationKind::Var
                                | VariableDeclarationKind::Let
                                | VariableDeclarationKind::Const
                        )
                )
                && !matches!(
                    model.nodes.parent_kind(model.nodes.parent_id(declaration)),
                    AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
                );
            valid.then_some(s)
        }
        _ => None,
    }
}

// The guard must be exactly `input && input.performance &&
// typeof input.performance.now === "function"` in that order. Collecting the
// operands in source order rejects duplicate operands, extra operands and any
// reordering: `input.performance.now` before the null `input` check would
// throw for a missing receiver.
fn performance_guard(model: &Model<'_, '_>, e: &Expression<'_>, input: Symbol) -> bool {
    fn operands<'a>(e: &'a Expression<'a>, out: &mut Vec<&'a Expression<'a>>) -> bool {
        match e {
            Expression::LogicalExpression(l) if l.operator.as_str() == "&&" => {
                operands(&l.left, out) && operands(&l.right, out)
            }
            _ => {
                out.push(e);
                true
            }
        }
    }
    fn performance_of(model: &Model<'_, '_>, e: &Expression<'_>, input: Symbol) -> bool {
        member(e, "performance").is_some_and(|receiver| symbol(model, receiver) == Some(input))
    }
    fn now_of(model: &Model<'_, '_>, e: &Expression<'_>, input: Symbol) -> bool {
        member(e, "now").is_some_and(|performance| performance_of(model, performance, input))
    }
    let mut parts = Vec::new();
    if !operands(e, &mut parts) || parts.len() != 3 {
        return false;
    }
    let [receiver, performance, now_guard] = parts.as_slice() else {
        return false;
    };
    if !matches!(receiver, Expression::Identifier(_)) || symbol(model, receiver) != Some(input) {
        return false;
    }
    let Expression::StaticMemberExpression(m) = performance else {
        return false;
    };
    if m.optional || m.property.name != "performance" || symbol(model, &m.object) != Some(input) {
        return false;
    }
    let Expression::BinaryExpression(binary) = now_guard else {
        return false;
    };
    let Expression::UnaryExpression(unary) = &binary.left else {
        return false;
    };
    binary.operator.as_str() == "==="
        && unary.operator.as_str() == "typeof"
        && now_of(model, &unary.argument, input)
        && string(&binary.right, "function")
}

// The supported numeric-date fallback is exactly a nested function whose whole
// body is `return Number(new Date());` (with an optional unresolved-global
// namespace such as `new b.Date()`). Signatures alone are not a clock proof:
// side effects, extra statements, a different constructor or arithmetic around
// the value are refused.
fn date_clock_body(model: &Model<'_, '_>, function: &Function<'_>) -> bool {
    let Some(body) = function.body.as_ref() else {
        return false;
    };
    if !body.directives.is_empty() {
        return false;
    }
    let [Statement::ReturnStatement(value)] = body.statements.as_slice() else {
        return false;
    };
    let Some(Expression::CallExpression(number)) = value.argument.as_ref() else {
        return false;
    };
    if number.optional {
        return false;
    }
    let args = call_arguments(number);
    if number.arguments.len() != 1 || args.len() != 1 {
        return false;
    }
    if !global_path_ends_with(model, &number.callee, &["Number"]) {
        return false;
    }
    let Expression::NewExpression(date) = args[0] else {
        return false;
    };
    date.arguments.is_empty() && global_path_ends_with(model, &date.callee, &["Date"])
}

fn elapsed_timer_factory(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if function.params.rest.is_some()
        || parameter.initializer.is_some()
        || function.r#async
        || function.generator
        || !body.directives.is_empty()
    {
        return None;
    }
    let input = immutable_binding(model, &parameter.pattern)?;
    let [holder, branch, alias_statement, baseline_statement, Statement::FunctionDeclaration(reading), Statement::ExpressionStatement(start_assignment), Statement::ReturnStatement(end)] =
        body.statements.as_slice()
    else {
        return None;
    };
    // `let clock;` is a plain storage slot established exactly once in each
    // branch. Two writes are the consequent `.bind` result and the alternate
    // fallback function; any further write would leave a non-clock origin.
    let clock_decl = one_variable(holder)?;
    let clock = unredeclared_binding(model, &clock_decl.id)?;
    if clock_decl.init.is_some()
        || model.formal_symbol_slot(clock).is_some()
        || model.binding_writes(clock) != 2
    {
        return None;
    }
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    if branch.alternate.is_none() || !performance_guard(model, &branch.test, input) {
        return None;
    }
    let [Statement::ExpressionStatement(consequent)] = block(&branch.consequent)? else {
        return None;
    };
    let Expression::AssignmentExpression(consequent) = &consequent.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &consequent.left else {
        return None;
    };
    if consequent.operator.as_str() != "="
        || crate::analysis::identifier_symbol(target, model.scoping) != Some(clock)
    {
        return None;
    }
    let bound = call(&consequent.right)?;
    let now = member(&bound.callee, "bind")?;
    let performance = member(now, "now")?;
    let receiver = member(performance, "performance")?;
    let bound_args = call_arguments(bound);
    if symbol(model, receiver) != Some(input)
        || bound_args.len() != 1
        || !member(bound_args[0], "performance").is_some_and(|e| symbol(model, e) == Some(input))
    {
        return None;
    }
    let alternate = block(branch.alternate.as_ref()?)?;
    let [Statement::FunctionDeclaration(fallback), Statement::ExpressionStatement(assign)] =
        alternate
    else {
        return None;
    };
    let fallback_symbol = fallback.id.as_ref()?.symbol_id.get()?;
    if fallback_symbol == clock
        || model.binding_writes(fallback_symbol) != 0
        || !model
            .scoping
            .symbol_redeclarations(fallback_symbol)
            .is_empty()
        || !fallback.params.items.is_empty()
        || fallback.params.rest.is_some()
        || fallback.r#async
        || fallback.generator
        || !date_clock_body(model, fallback)
    {
        return None;
    }
    let Expression::AssignmentExpression(assign) = &assign.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
        return None;
    };
    if assign.operator.as_str() != "="
        || crate::analysis::identifier_symbol(target, model.scoping) != Some(clock)
        || symbol(model, &assign.right) != Some(fallback_symbol)
    {
        return None;
    }
    // `let read = clock; let baseline = clock();`
    let alias_decl = one_variable(alias_statement)?;
    let alias = immutable_binding(model, &alias_decl.id)?;
    if symbol(model, alias_decl.init.as_ref()?) != Some(clock) {
        return None;
    }
    let baseline_decl = one_variable(baseline_statement)?;
    let baseline = immutable_binding(model, &baseline_decl.id)?;
    let baseline_call = call(baseline_decl.init.as_ref()?)?;
    if symbol(model, &baseline_call.callee) != Some(clock) || !baseline_call.arguments.is_empty() {
        return None;
    }
    // The returned reading closure may only subtract the same two identities.
    let reading_symbol = reading.id.as_ref()?.symbol_id.get()?;
    let reading_body = reading.body.as_ref()?;
    if reading_symbol == clock
        || reading_symbol == alias
        || reading_symbol == baseline
        || model.binding_writes(reading_symbol) != 0
        || !model
            .scoping
            .symbol_redeclarations(reading_symbol)
            .is_empty()
        || !reading.params.items.is_empty()
        || reading.params.rest.is_some()
        || reading.r#async
        || reading.generator
        || !reading_body.directives.is_empty()
    {
        return None;
    }
    let [Statement::ReturnStatement(value)] = reading_body.statements.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(subtract) = value.argument.as_ref()? else {
        return None;
    };
    let Expression::CallExpression(reading_clock) = &subtract.left else {
        return None;
    };
    if subtract.operator.as_str() != "-"
        || reading_clock.optional
        || !reading_clock.arguments.is_empty()
        || symbol(model, &reading_clock.callee) != Some(alias)
        || symbol(model, &subtract.right) != Some(baseline)
    {
        return None;
    }
    let Expression::AssignmentExpression(start) = &start_assignment.expression else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(target) = &start.left else {
        return None;
    };
    if start.operator.as_str() != "="
        || target.optional
        || target.property.name != "start"
        || symbol(model, &target.object) != Some(reading_symbol)
        || symbol(model, &start.right) != Some(baseline)
    {
        return None;
    }
    (end.argument.as_ref().and_then(|e| symbol(model, e)) == Some(reading_symbol)).then_some(())
}

fn elapsed_reading(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    // When the reading closure is returned by a fully proven elapsed-timer
    // factory, the factory proof already established both captured identities
    // and their initialization order; only the exact nested function is named.
    if let Some(parent) = model.enclosing_function(site.node_id) {
        if parent.span != site.span {
            if let AstKind::Function(parent_function) = model.nodes.kind(parent.node_id) {
                if let Some(parent_body) = parent_function.body.as_ref() {
                    if elapsed_timer_factory(model, parent_function, parent_body).is_some()
                        && parent_body.statements.iter().any(|statement| {
                            matches!(statement, Statement::FunctionDeclaration(f)
                                if model.site_by_span(f.span).is_some_and(|found| found.span == site.span))
                        })
                    {
                        return Some(());
                    }
                }
            }
        }
    }
    if !function.params.items.is_empty()
        || function.params.rest.is_some()
        || function.r#async
        || function.generator
        || !body.directives.is_empty()
    {
        return None;
    }
    let [Statement::ReturnStatement(value)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(subtract) = value.argument.as_ref()? else {
        return None;
    };
    let Expression::CallExpression(clock) = &subtract.left else {
        return None;
    };
    if subtract.operator.as_str() != "-"
        || clock.optional
        || !clock.arguments.is_empty()
        || !matches!(&clock.callee, Expression::Identifier(_))
        || !ordered_captured_callable(model, site, &clock.callee)
    {
        return None;
    }
    let clock = capture(model, site, &clock.callee)?;
    let baseline = capture(model, site, &subtract.right)?;
    if clock == baseline {
        return None;
    }
    if model.scoping.symbol_span(clock).end > model.scoping.symbol_span(baseline).start {
        return None;
    }
    let clock_declaration = model.scoping.symbol_declaration(clock);
    let AstKind::VariableDeclarator(clock_decl) = model.nodes.kind(clock_declaration) else {
        return None;
    };
    let AstKind::VariableDeclaration(clock_kind) = model.nodes.parent_kind(clock_declaration)
    else {
        return None;
    };
    if !matches!(&clock_decl.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(clock))
        || clock_decl.init.is_none()
        || !matches!(
            clock_kind.kind,
            VariableDeclarationKind::Var
                | VariableDeclarationKind::Let
                | VariableDeclarationKind::Const
        )
        || matches!(
            model
                .nodes
                .parent_kind(model.nodes.parent_id(clock_declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    let baseline_declaration = model.scoping.symbol_declaration(baseline);
    let AstKind::VariableDeclarator(baseline_decl) = model.nodes.kind(baseline_declaration) else {
        return None;
    };
    let AstKind::VariableDeclaration(baseline_kind) = model.nodes.parent_kind(baseline_declaration)
    else {
        return None;
    };
    let baseline_call = call(baseline_decl.init.as_ref()?)?;
    if !matches!(&baseline_decl.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(baseline))
        || !matches!(
            baseline_kind.kind,
            VariableDeclarationKind::Var
                | VariableDeclarationKind::Let
                | VariableDeclarationKind::Const
        )
        || matches!(
            model
                .nodes
                .parent_kind(model.nodes.parent_id(baseline_declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        || !matches!(
            model
                .nodes
                .parent_kind(model.nodes.parent_id(baseline_declaration)),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        )
        || baseline_call.optional
        || !baseline_call.arguments.is_empty()
        || symbol(model, &baseline_call.callee) != Some(clock)
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    if elapsed_timer_factory(model, function, body).is_some() {
        claims.push(proposal(
            "createElapsedTimer",
            "shared_elapsed_timer",
            "guards formal && formal.performance && typeof formal.performance.now === \"function\" in that exact order, binds it (or a nested clock whose entire body is Number(new Date())) to one storage slot, calls it once for a baseline, and returns an owned closure over immutable alias/baseline identities that subtracts that baseline and carries it on .start",
            f,
        ));
    }
    if elapsed_reading(model, site, function, body).is_some() {
        claims.push(proposal(
            "readElapsedTime",
            "shared_elapsed_timer",
            "returns exactly capturedClock() - capturedBaseline, where the clock has an independently ordered callable origin and the baseline's plain initializer calls that same captured clock; no parameters or other operations",
            f,
        ));
    }
    claims
}
