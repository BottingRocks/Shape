//! The call sites: where a helper expands and the text it expands to.

use super::*;
use crate::transforms::emitted_js::support::indent_at;

/// One planned expansion: the call (or call statement) span and the text
/// pieces of each argument.
pub(super) struct Site {
    /// What the expansion replaces: the call, or the statement holding it.
    pub(super) span: Span,
    /// The call itself.
    pub(super) call: Span,
    pub(super) helper: SymbolId,
    /// Argument spans in order (missing arguments are `void 0`).
    pub(super) arguments: Vec<Option<Span>>,
    /// Fresh temporaries holding computed arguments, by parameter.
    pub(super) temps: Vec<Option<String>>,
    /// Temporaries holding a pure value (a literal allocation of pure
    /// parts): declared just before the body's first use instead.
    pub(super) lazy: Vec<bool>,
    /// Fresh names for the body's own bindings.
    pub(super) locals: Vec<String>,
    /// The call is a callee itself: a member body needs `(0, …)`.
    pub(super) callee: bool,
    pub(super) context: Context,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Context {
    /// Inside an expression: replaced in place.
    Expression,
    /// `H(…);` on its own: the value is dropped.
    Statement,
    /// `let x = H(…);`, `x = H(…);`, `return H(…);`: the value is used.
    Value,
}

pub(super) struct Sites<'s, 'a> {
    pub(super) names: FreshNames,
    pub(super) scoping: &'s Scoping,
    pub(super) nodes: &'s AstNodes<'a>,
    pub(super) helpers: &'s BTreeMap<SymbolId, Helper>,
    pub(super) planned: Vec<Site>,
    pub(super) exact: bool,
    /// Calls inside an expression that would expand at a statement of
    /// their own (arguments that need temporaries): hoisting candidates.
    pub(super) declined: Vec<Span>,
}

impl Sites<'_, '_> {
    pub(super) fn plan(&mut self, call: &CallExpression<'_>, statement: Option<Span>, context: Context, callee: bool) {
        let Expression::Identifier(identifier) = &call.callee else {
            return;
        };
        let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
            return;
        };
        let Some(helper) = self.helpers.get(&symbol) else {
            return;
        };
        if call.optional
            || self
                .planned
                .iter()
                .any(|site| site.call == call.span || statement == Some(site.span))
            || (helper.shape == Shape::Statements && context == Context::Expression)
            || (helper.declaration.start <= call.span.start && call.span.end <= helper.declaration.end)
        {
            return;
        }
        let mut arguments = Vec::new();
        for argument in &call.arguments {
            let Some(expression) = argument.as_expression() else {
                return;
            };
            arguments.push(expression);
        }
        // Extra arguments still run; only pure ones may be dropped.
        if arguments.iter().skip(helper.parameters).any(|a| !expression_is_pure(a)) {
            return;
        }
        let stable = self.stable_arguments(helper, &arguments);
        let in_order = arguments_in_order(helper, &stable, &arguments);
        if !in_order && context == Context::Expression {
            if self.free_names_resolve(identifier, helper) {
                self.declined.push(call.span);
            }
            return;
        }
        // A temporary for a bare slot name would only be a snapshot copy:
        // leave such a call alone.
        if !in_order
            && !self.exact
            && (0..helper.parameters).any(|index| {
                !stable[index] && matches!(arguments.get(index), Some(Expression::Identifier(_)))
            })
        {
            return;
        }
        if !self.free_names_resolve(identifier, helper) {
            return;
        }
        let temps: Vec<Option<String>> = (0..helper.parameters)
            .map(|index| ((!in_order && !stable[index]) || helper.written[index]).then(|| self.names.fresh()))
            .collect();
        let lazy = (0..helper.parameters)
            .map(|index| temps[index].is_some() && arguments.get(index).is_some_and(|a| expression_is_pure(a)))
            .collect();
        self.planned.push(Site {
            span: statement.unwrap_or(call.span),
            call: call.span,
            helper: symbol,
            arguments: (0..helper.parameters)
                .map(|index| arguments.get(index).map(|a| a.span()))
                .collect(),
            temps,
            lazy,
            locals: (0..helper.local_count).map(|_| self.names.fresh()).collect(),
            callee,
            context,
        });
    }

    /// Per parameter, whether its argument can be read where the body uses
    /// it instead of being held in a temporary.
    fn stable_arguments(&self, helper: &Helper, arguments: &[&Expression<'_>]) -> Vec<bool> {
        let mut stable: Vec<bool> = (0..helper.parameters)
            .map(|index| !helper.written[index] && arguments.get(index).is_none_or(|a| self.stable(a)))
            .collect();
        // A plain name may also be read where the body uses it when no code
        // can run between the call's arguments and those uses: no later
        // argument and no earlier body node does more than read or store a
        // property (no getters or setters), so nothing can store the name.
        for index in 0..helper.parameters {
            // A plain name, or any read-only value (`arguments[0]`,
            // `o["k"]`): reading it again, with no code run in between,
            // gives the same value and runs nothing.
            let Some(argument) = arguments.get(index) else {
                continue;
            };
            if !read_only(argument) {
                continue;
            }
            if stable[index] || helper.written[index] || arguments.iter().skip(index + 1).any(|later| self.runs_code(later)) {
                continue;
            }
            let last_use = helper
                .uses
                .iter()
                .filter(|(_, parameter)| *parameter == index)
                .map(|(span, _)| span.start)
                .max();
            if last_use.is_some_and(|last| {
                helper.code.iter().all(|(start, end)| !(*start < last && !(*start <= last && last < *end)))
            }) {
                stable[index] = true;
            }
        }
        stable
    }

    /// Whether every name the helper reads from outside resolves to the same
    /// binding at the call.
    fn free_names_resolve(&self, identifier: &IdentifierReference<'_>, helper: &Helper) -> bool {
        let Some(scope) = identifier
            .reference_id
            .get()
            .map(|reference| self.nodes.get_node(self.scoping.get_reference(reference).node_id()).scope_id())
        else {
            return false;
        };
        !helper
            .free
            .iter()
            .any(|(name, symbol)| self.scoping.find_binding(scope, name.as_str().into()) != *symbol)
    }

    /// Whether evaluating it can run code beyond reading and storing
    /// properties; a call to a helper that does no more counts as such.
    pub(super) fn runs_code(&self, expression: &Expression<'_>) -> bool {
        let mut scan = BodyScan::default();
        scan.visit_expression(expression);
        let mut calls = PropertyOnlyCalls {
            scoping: self.scoping,
            helpers: self.helpers,
            spans: Vec::new(),
        };
        calls.visit_expression(expression);
        scan.code.iter().any(|span| !calls.spans.contains(span))
    }

/// A literal, or a never-written local that is not a VM slot or carry.
    pub(super) fn stable(&self, argument: &Expression<'_>) -> bool {
        match argument {
            Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => true,
            Expression::UnaryExpression(unary) => {
                matches!(unary.operator, UnaryOperator::UnaryNegation | UnaryOperator::Void)
                    && matches!(unary.argument, Expression::NumericLiteral(_))
            }
            Expression::Identifier(identifier) => {
                let name = identifier.name.as_str();
                let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
                    return false;
                };
                let declaration = self.scoping.symbol_declaration(symbol);
                !name.starts_with("$KEY_")
                    && !name.starts_with("$CARRY_")
                    && !self.scoping.symbol_is_mutated(symbol)
                    && !matches!(
                        self.nodes.kind(declaration),
                        AstKind::FormalParameter(_) | AstKind::FormalParameters(_)
                    )
            }
            _ => false,
        }
    }
}

/// Whether computed arguments can stay inline: each is read once, in
/// argument order, with no body effect before it and no written parameter.
fn arguments_in_order(helper: &Helper, stable: &[bool], arguments: &[&Expression<'_>]) -> bool {
    // In statement position, arguments that would not keep their order
    // inline are held in temporaries instead.
    // A written parameter needs a binding of its own.
    let mut in_order = !helper.written.iter().any(|written| *written);
    let mut last_computed = None;
    for index in 0..helper.parameters {
        let uses: Vec<Span> = helper
            .uses
            .iter()
            .filter(|(_, parameter)| *parameter == index)
            .map(|(span, _)| *span)
            .collect();
        if stable[index] {
            continue;
        }
        let argument = arguments[index];
        if uses.is_empty() {
            if !expression_is_pure(argument) {
                in_order = false;
            }
            continue;
        }
        if uses.len() != 1 {
            in_order = false;
            continue;
        }
        let at = uses[0];
        // Computed arguments keep their order, and nothing in the body
        // runs before they are read.
        if last_computed.is_some_and(|previous: u32| previous >= at.start) {
            in_order = false;
        }
        last_computed = Some(at.start);
        if helper
            .effects
            .iter()
            .any(|(start, end)| *end <= at.start && !(*start <= at.start && at.end <= *end))
        {
            in_order = false;
        }
    }
    in_order
}

impl Sites<'_, '_> {
    /// Calls that are statements, declaration initializers, assignment right
    /// sides, or returned values in a statement list.
    pub(super) fn list(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            match statement {
                Statement::ExpressionStatement(expression) => match &expression.expression {
                    Expression::CallExpression(call) => {
                        self.plan(call, Some(statement.span()), Context::Statement, false);
                    }
                    Expression::AssignmentExpression(assignment)
                        if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
                            && (matches!(assignment.left, AssignmentTarget::AssignmentTargetIdentifier(_))
                                || self.stable_member_target(&assignment.left)) =>
                    {
                        if let Expression::CallExpression(call) = &assignment.right {
                            self.plan(call, Some(statement.span()), Context::Value, false);
                        }
                    }
                    _ => {}
                },
                Statement::VariableDeclaration(declaration) if declaration.declarations.len() == 1 => {
                    if let Some(Expression::CallExpression(call)) = &declaration.declarations[0].init {
                        self.plan(call, Some(statement.span()), Context::Value, false);
                    }
                }
                Statement::ReturnStatement(ret) => {
                    if let Some(Expression::CallExpression(call)) = &ret.argument {
                        self.plan(call, Some(statement.span()), Context::Value, false);
                    }
                }
                _ => {}
            }
        }
    }
}

impl Sites<'_, '_> {
    /// `o[k] = H(…)` or `o.k = H(…)` with `o` (and an identifier `k`) a
    /// never-written binding, `k` otherwise a literal: the body may run
    /// before the target is read, which reads the same then.
    fn stable_member_target(&self, target: &AssignmentTarget<'_>) -> bool {
        let unwritten = |expression: &Expression<'_>| match expression {
            Expression::Identifier(identifier) => identifier
                .reference_id
                .get()
                .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                .is_some_and(|symbol| {
                    !self.scoping.symbol_is_mutated(symbol)
                        && self.scoping.symbol_declarations(symbol).count() == 1
                }),
            _ => false,
        };
        match target {
            AssignmentTarget::ComputedMemberExpression(member) => {
                unwritten(&member.object)
                    && (matches!(
                        member.expression,
                        Expression::NumericLiteral(_) | Expression::StringLiteral(_)
                    ) || unwritten(&member.expression))
            }
            AssignmentTarget::StaticMemberExpression(member) => unwritten(&member.object),
            _ => false,
        }
    }
}

impl<'a> Visit<'a> for Sites<'_, 'a> {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.list(&body.statements);
        walk::walk_function_body(self, body);
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.list(&block.body);
        walk::walk_block_statement(self, block);
    }

    fn visit_switch_case(&mut self, case: &SwitchCase<'a>) {
        self.list(&case.consequent);
        walk::walk_switch_case(self, case);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        // A call whose result is called again passes no `this` to it.
        if let Expression::CallExpression(inner) = &call.callee {
            self.plan(inner, None, Context::Expression, true);
        }
        self.plan(call, None, Context::Expression, false);
        walk::walk_call_expression(self, call);
    }
}

/// The text for planned site `index`, with nested planned sites rendered.
/// Calls to helpers whose bodies only read and store properties.
pub(super) struct PropertyOnlyCalls<'s> {
    pub(super) scoping: &'s Scoping,
    pub(super) helpers: &'s BTreeMap<SymbolId, Helper>,
    pub(super) spans: Vec<(u32, u32)>,
}

impl<'a> Visit<'a> for PropertyOnlyCalls<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::Identifier(identifier) = &call.callee {
            if analysis::identifier_symbol(identifier, self.scoping)
                .and_then(|symbol| self.helpers.get(&symbol))
                .is_some_and(|helper| helper.code.is_empty())
            {
                self.spans.push((call.span.start, call.span.end));
            }
        }
        walk::walk_call_expression(self, call);
    }
}

pub(super) fn render_site(source: &str, helpers: &BTreeMap<SymbolId, Helper>, planned: &[Site], index: usize) -> String {
    let site = &planned[index];
    let helper = &helpers[&site.helper];
    let rendered: Vec<String> = site
        .arguments
        .iter()
        .map(|argument| match argument {
            None => "(void 0)".to_string(),
            Some(span) => format!("({})", render_span(source, helpers, planned, *span, index)),
        })
        .collect();
    let argument_text: Vec<String> = rendered
        .iter()
        .zip(&site.temps)
        .map(|(text, temp)| temp.clone().unwrap_or_else(|| text.clone()))
        .collect();
    let mut names: Vec<(Span, &str)> = helper
        .uses
        .iter()
        .map(|(span, parameter)| (*span, argument_text[*parameter].as_str()))
        .chain(helper.locals.iter().map(|(span, local)| (*span, site.locals[*local].as_str())))
        .collect();
    names.sort_by_key(|(span, _)| span.start);
    let piece = |span: Span| -> String {
        let mut out = String::new();
        let mut at = span.start as usize;
        for (use_span, text) in &names {
            if use_span.start < span.start || use_span.end > span.end {
                continue;
            }
            out.push_str(&source[at..use_span.start as usize]);
            out.push_str(text);
            at = use_span.end as usize;
        }
        out.push_str(&source[at..span.end as usize]);
        out
    };
    if site.context == Context::Expression {
        let body = piece(helper.pieces[0]);
        return if site.callee && helper.member {
            format!("(0, {body})")
        } else {
            format!("({body})")
        };
    }
    // Statement position: temporaries in argument order, the body, then the
    // statement itself with the helper's value. A pure value's temporary
    // waits for the first body piece that reads it: building it later runs
    // nothing, and inner values then come before the stores that nest them.
    let mut lines: Vec<String> = Vec::new();
    let mut waiting: Vec<usize> = Vec::new();
    for (index, (temp, text)) in site.temps.iter().zip(&rendered).enumerate() {
        if let Some(temp) = temp {
            if site.lazy[index] && helper.shape == Shape::Statements {
                waiting.push(index);
            } else {
                lines.push(format!("let {temp} = {text};"));
            }
        }
    }
    let declare_for = |span: Span, lines: &mut Vec<String>, waiting: &mut Vec<usize>| {
        waiting.retain(|index| {
            let used = helper
                .uses
                .iter()
                .any(|(use_span, parameter)| parameter == index && span.start <= use_span.start && use_span.end <= span.end);
            if used {
                lines.push(format!("let {} = {};", site.temps[*index].as_ref().unwrap(), rendered[*index]));
            }
            !used
        });
    };
    let value = match helper.shape {
        Shape::Expression => format!("({})", piece(helper.pieces[0])),
        Shape::Statements => {
            for span in &helper.pieces {
                declare_for(*span, &mut lines, &mut waiting);
                lines.push(piece(*span));
            }
            if let Some(span) = helper.value {
                declare_for(span, &mut lines, &mut waiting);
            }
            helper.value.map_or_else(|| "(void 0)".to_string(), |span| format!("({})", piece(span)))
        }
    };
    match site.context {
        Context::Statement => {
            // A returned parameter or empty literal does nothing; any other
            // returned value still runs.
            let quiet = helper.reads_only || helper.value.is_none_or(|span| {
                let text = &source[span.start as usize..span.end as usize];
                text == "[]" || text == "{}" || helper.uses.iter().any(|(use_span, _)| *use_span == span)
            });
            if helper.shape == Shape::Expression || !quiet {
                lines.push(format!("{value};"));
            }
        }
        _ => lines.push(format!(
            "{}{value}{}",
            &source[site.span.start as usize..site.call.start as usize],
            &source[site.call.end as usize..site.span.end as usize]
        )),
    }
    lines.join(&format!("\n{}", indent_at(source, site.span)))
}

/// Source text of `span` with the planned sites nested inside it rendered.
pub(super) fn render_span(source: &str, helpers: &BTreeMap<SymbolId, Helper>, planned: &[Site], span: Span, owner: usize) -> String {
    let mut out = String::new();
    let mut at = span.start;
    for (index, site) in planned.iter().enumerate() {
        if index == owner || site.span.start < at || site.span.end > span.end || site.span.start < span.start {
            continue;
        }
        out.push_str(&source[at as usize..site.span.start as usize]);
        out.push_str(&render_site(source, helpers, planned, index));
        at = site.span.end;
    }
    out.push_str(&source[at as usize..span.end as usize]);
    out
}

