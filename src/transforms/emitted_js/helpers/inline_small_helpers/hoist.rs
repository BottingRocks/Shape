//! Hoisting a statement helper's call out of an expression into its own declaration.

use super::*;
use crate::transforms::emitted_js::support::indent_at;

/// Calls to statement-shaped helpers inside an expression.
pub(super) struct NestedCalls<'h> {
    pub(super) scoping: &'h Scoping,
    pub(super) helpers: &'h BTreeMap<SymbolId, Helper>,
    /// Expression helper calls whose arguments need temporaries.
    pub(super) declined: &'h [Span],
    pub(super) calls: Vec<Span>,
    /// Name reads that a call could change (written or unresolved names).
    pub(super) names: Vec<Span>,
    /// Code-running nodes and property reads: (span, member root names).
    pub(super) code: Vec<Span>,
    pub(super) reads: Vec<(Span, Option<String>)>,
    pub(super) conditional: Vec<Span>,
    /// Store targets `o[k]` / `o.k` with a never-written `o` and a literal
    /// key: evaluating one reads no property and runs no code, and the
    /// object it names is the same before and after any call, so a call in
    /// the stored value may move ahead of it even when handed `o`.
    pub(super) store_targets: Vec<Span>,
    /// Conditionals and logical expressions with the arms that may not run.
    pub(super) choices: Vec<Choice>,
    /// Optional chains: what they skip is not a plain arm.
    pub(super) chains: Vec<Span>,
    /// Nested functions: their statements are edited on their own.
    pub(super) functions: Vec<Span>,
    /// Where a bound piece may not sit: a method callee (bound alone it
    /// loses its `this`), a `delete` or `typeof` operand, or a store target
    /// other than `x = …` and `o.k = …` / `o[literal] = …`.
    pub(super) unbindable: Vec<Span>,
    /// Conversions that run where they stand, not where their operands are
    /// read: spread elements, templates with substitutions, computed keys.
    pub(super) opaque: Vec<Span>,
    /// `x = …` (resolved `x`) and `o.k = …` / `o[literal] = …` targets:
    /// the store itself runs last and reads nothing, only `o` is evaluated
    /// first.
    pub(super) targets: Vec<Span>,
}

impl<'h> NestedCalls<'h> {
    pub(super) fn new(scoping: &'h Scoping, helpers: &'h BTreeMap<SymbolId, Helper>, declined: &'h [Span]) -> Self {
        NestedCalls {
            scoping,
            helpers,
            declined,
            calls: Vec::new(),
            names: Vec::new(),
            code: Vec::new(),
            reads: Vec::new(),
            conditional: Vec::new(),
            store_targets: Vec::new(),
            choices: Vec::new(),
            chains: Vec::new(),
            functions: Vec::new(),
            unbindable: Vec::new(),
            opaque: Vec::new(),
            targets: Vec::new(),
        }
    }
}

/// `t ? a : b`, `l && r`, `l || r` or `l ?? r`: the test always runs, the
/// arms only by its value.
#[derive(Clone, Copy)]
pub(super) struct Choice {
    pub(super) span: Span,
    pub(super) test: Span,
    pub(super) consequent: Span,
    /// `None` for a logical expression.
    pub(super) alternate: Option<Span>,
    pub(super) operator: Option<LogicalOperator>,
}

impl Choice {
    fn arms(&self) -> impl Iterator<Item = Span> + '_ {
        std::iter::once(self.consequent).chain(self.alternate)
    }
}

/// Standard namespaces whose methods a VM helper does not replace: reading
/// `Object.defineProperty` before or after a call gives the same function.
const FIXED_NAMESPACES: &[&str] = &["Object", "Math", "JSON", "Reflect", "Array", "Number", "String", "Symbol"];

impl<'a> Visit<'a> for NestedCalls<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let helper = matches!(&call.callee, Expression::Identifier(callee)
            if analysis::identifier_symbol(callee, self.scoping)
                .and_then(|symbol| self.helpers.get(&symbol))
                .is_some_and(|helper| helper.shape == Shape::Statements));
        if (helper || self.declined.contains(&call.span)) && !call.optional {
            self.calls.push(call.span);
        } else {
            self.code.push(call.span);
        }
        if matches!(call.callee.without_parentheses(), Expression::ChainExpression(_))
            || call.callee.without_parentheses().is_member_expression()
        {
            self.unbindable.push(call.callee.span());
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_new_expression(&mut self, call: &NewExpression<'a>) {
        self.code.push(call.span);
        walk::walk_new_expression(self, call);
    }
    fn visit_await_expression(&mut self, expression: &AwaitExpression<'a>) {
        self.code.push(expression.span);
        walk::walk_await_expression(self, expression);
    }
    fn visit_yield_expression(&mut self, expression: &YieldExpression<'a>) {
        self.code.push(expression.span);
        walk::walk_yield_expression(self, expression);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if analysis::identifier_symbol(identifier, self.scoping)
            .is_none_or(|symbol| self.scoping.symbol_is_mutated(symbol))
        {
            self.names.push(identifier.span);
        }
    }
    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if matches!(unary.operator, UnaryOperator::Delete | UnaryOperator::Typeof) {
            self.unbindable.push(unary.argument.span());
        }
        walk::walk_unary_expression(self, unary);
    }
    fn visit_spread_element(&mut self, spread: &SpreadElement<'a>) {
        self.opaque.push(spread.span);
        walk::walk_spread_element(self, spread);
    }
    fn visit_template_literal(&mut self, template: &TemplateLiteral<'a>) {
        if !template.expressions.is_empty() {
            self.opaque.push(template.span);
        }
        walk::walk_template_literal(self, template);
    }
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.computed {
            self.opaque.push(property.key.span());
        }
        // `{x}`: the name is also the key.
        if property.shorthand {
            self.unbindable.push(property.value.span());
        }
        walk::walk_object_property(self, property);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        self.code.push(assignment.span);
        let plain = assignment.operator == AssignmentOperator::Assign;
        match &assignment.left {
            AssignmentTarget::AssignmentTargetIdentifier(target)
                if plain && analysis::identifier_symbol(target, self.scoping).is_some() =>
            {
                self.targets.push(target.span);
            }
            left => match left.as_member_expression() {
                Some(member)
                    if plain
                        && match member {
                            MemberExpression::StaticMemberExpression(_) => true,
                            MemberExpression::ComputedMemberExpression(computed) => matches!(
                                computed.expression,
                                Expression::NumericLiteral(_) | Expression::StringLiteral(_)
                            ),
                            MemberExpression::PrivateFieldExpression(_) => false,
                        } =>
                {
                    self.targets.push(member.span());
                }
                _ => self.unbindable.push(left.span()),
            },
        }
        if let Some(member) = assignment.left.as_member_expression() {
            let literal_key = match member {
                MemberExpression::StaticMemberExpression(_) => true,
                MemberExpression::ComputedMemberExpression(computed) => matches!(
                    computed.expression,
                    Expression::NumericLiteral(_) | Expression::StringLiteral(_)
                ),
                MemberExpression::PrivateFieldExpression(_) => false,
            };
            let fixed_object = match member.object() {
                Expression::Identifier(object) => analysis::identifier_symbol(object, self.scoping)
                    .is_some_and(|symbol| !self.scoping.get_resolved_references(symbol).any(|r| r.is_write())),
                _ => false,
            };
            if literal_key && fixed_object {
                self.store_targets.push(member.span());
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        self.code.push(update.span);
        walk::walk_update_expression(self, update);
    }
    fn visit_tagged_template_expression(&mut self, tagged: &TaggedTemplateExpression<'a>) {
        self.code.push(tagged.span);
        walk::walk_tagged_template_expression(self, tagged);
    }
    fn visit_member_expression(&mut self, member: &MemberExpression<'a>) {
        let mut object = member.object();
        while let Some(inner) = object.as_member_expression() {
            object = inner.object();
        }
        let root = match object {
            Expression::Identifier(identifier) => Some(identifier.name.to_string()),
            _ => None,
        };
        // `Object.defineProperty`: a standard method of a global namespace.
        let namespace_object = match member {
            MemberExpression::StaticMemberExpression(read) => Some(&read.object),
            MemberExpression::ComputedMemberExpression(read) if matches!(read.expression, Expression::StringLiteral(_)) => {
                Some(&read.object)
            }
            _ => None,
        };
        let fixed = namespace_object.is_some_and(|object| matches!(object, Expression::Identifier(namespace)
            if FIXED_NAMESPACES.contains(&namespace.name.as_str())
                && analysis::identifier_symbol(namespace, self.scoping).is_none()));
        // `arguments[0][…]` reads through the first argument only.
        let root = match (root.as_deref(), positional_argument(member)) {
            (Some("arguments"), Some(index)) => Some(format!("arguments[{index}]")),
            _ => root,
        };
        if !fixed && !self.store_targets.contains(&member.span()) {
            self.reads.push((member.span(), root));
        }
        walk::walk_member_expression(self, member);
    }
    fn visit_conditional_expression(&mut self, conditional: &ConditionalExpression<'a>) {
        self.choices.push(Choice {
            span: conditional.span,
            test: conditional.test.span(),
            consequent: conditional.consequent.span(),
            alternate: Some(conditional.alternate.span()),
            operator: None,
        });
        self.visit_expression(&conditional.test);
        self.conditional.push(conditional.consequent.span());
        self.conditional.push(conditional.alternate.span());
        self.visit_expression(&conditional.consequent);
        self.visit_expression(&conditional.alternate);
    }
    fn visit_logical_expression(&mut self, logical: &LogicalExpression<'a>) {
        self.choices.push(Choice {
            span: logical.span,
            test: logical.left.span(),
            consequent: logical.right.span(),
            alternate: None,
            operator: Some(logical.operator),
        });
        self.visit_expression(&logical.left);
        self.conditional.push(logical.right.span());
        self.visit_expression(&logical.right);
    }
    fn visit_chain_expression(&mut self, chain: &ChainExpression<'a>) {
        self.conditional.push(chain.span);
        self.chains.push(chain.span);
        walk::walk_chain_expression(self, chain);
    }
    fn visit_function(&mut self, function: &Function<'a>, _: ScopeFlags) {
        self.functions.push(function.span);
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push(arrow.span);
    }
}

/// The outermost choice whose arm holds the first helper call that runs
/// conditionally.
fn armed_choice(nested: &NestedCalls<'_>) -> Option<Choice> {
    let inside = |outer: Span, inner: Span| outer.start <= inner.start && inner.end <= outer.end;
    let call = nested
        .calls
        .iter()
        .copied()
        .filter(|call| nested.choices.iter().any(|choice| choice.arms().any(|arm| inside(arm, *call))))
        .min_by_key(|call| call.start)?;
    nested
        .choices
        .iter()
        .filter(|choice| choice.arms().any(|arm| inside(arm, call)))
        .min_by_key(|choice| (choice.span.start, std::cmp::Reverse(choice.span.end)))
        .copied()
}

/// `arguments[N]` at the root of a member chain: `N`.
fn positional_argument(member: &MemberExpression<'_>) -> Option<u32> {
    let mut current = member;
    loop {
        match current.object().as_member_expression() {
            Some(inner) => current = inner,
            None => break,
        }
    }
    match current {
        MemberExpression::ComputedMemberExpression(computed) => match (&computed.object, &computed.expression) {
            (Expression::Identifier(object), Expression::NumericLiteral(index))
                if object.name == "arguments" && index.value.fract() == 0.0 && index.value >= 0.0 =>
            {
                Some(index.value as u32)
            }
            _ => None,
        },
        _ => None,
    }
}

/// Whether `text` (a call) is handed what `root` names: the name itself as
/// a whole word, or for `arguments[N]` any `arguments` other than another
/// literal position `arguments[M]`.
fn hands(text: &str, root: &str) -> bool {
    let word = |at: usize, len: usize| {
        let before = text[..at].chars().next_back();
        let after = text[at + len..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
    };
    if let Some(index) = root.strip_prefix("arguments[").and_then(|rest| rest.strip_suffix(']')) {
        return text.match_indices("arguments").any(|(at, _)| {
            if !word(at, "arguments".len()) {
                return false;
            }
            let rest = &text[at + "arguments".len()..];
            let other = rest
                .strip_prefix('[')
                .and_then(|rest| rest.split_once(']'))
                .filter(|(position, _)| !position.is_empty() && position.bytes().all(|b| b.is_ascii_digit()))
                .is_some_and(|(position, _)| position != index);
            !other
        });
    }
    text.match_indices(root).any(|(at, _)| word(at, root.len()))
}

/// Every outermost piece the statement evaluates before `call` that runs
/// code, reads a property or reads a changeable name, in order, when each
/// may be bound to a temporary ahead of the statement: an unconditional part
/// of `value`, not a method callee, `delete`/`typeof` operand or other store
/// target, with no spread, template or computed key evaluated before the
/// call outside them. A store target `x = …` or `o.k = …` is no piece (its
/// `o` is).
fn bound_pieces(nested: &NestedCalls<'_>, call: Span, value: Span) -> Option<Vec<Span>> {
    let inside = |outer: Span, inner: Span| outer.start <= inner.start && inner.end <= outer.end;
    let mut candidates: Vec<Span> = nested
        .code
        .iter()
        .copied()
        .chain(nested.reads.iter().map(|(span, _)| *span))
        .chain(nested.names.iter().copied())
        .filter(|span| span.end <= call.start && !nested.targets.contains(span))
        .collect();
    candidates.sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
    candidates.dedup();
    let pieces: Vec<Span> = candidates
        .iter()
        .copied()
        .filter(|span| !candidates.iter().any(|outer| outer != span && inside(*outer, *span)))
        .collect();
    if pieces.is_empty()
        || pieces.iter().any(|piece| {
            piece.start < value.start
                || nested.conditional.iter().any(|arm| inside(*arm, *piece))
                || nested.unbindable.iter().any(|outer| inside(*outer, *piece))
        })
        || nested
            .opaque
            .iter()
            .any(|span| span.start < call.start && !pieces.iter().any(|piece| inside(*piece, *span)))
    {
        return None;
    }
    Some(pieces)
}

impl Hoists<'_, '_> {
    pub(super) fn statement(&mut self, statement: &Statement<'_>) {
        if self.exact && self.restructure_loop(statement) {
            return;
        }
        // An `if` is scanned in its test only: its arms run later, or not.
        let (whole, value, test): (Span, Option<Span>, Option<&Expression<'_>>) = match statement {
            Statement::ExpressionStatement(expression) => (expression.span, Some(expression.expression.span()), None),
            Statement::VariableDeclaration(declaration) if declaration.declarations.len() == 1 => {
                (declaration.span, declaration.declarations[0].init.as_ref().map(GetSpan::span), None)
            }
            Statement::ReturnStatement(ret) => (ret.span, ret.argument.as_ref().map(GetSpan::span), None),
            Statement::IfStatement(branch) => (branch.span, Some(branch.test.span()), Some(&branch.test)),
            _ => return,
        };
        let Some(value) = value else {
            return;
        };
        let mut nested = NestedCalls::new(self.scoping, self.helpers, self.declined);
        match test {
            Some(test) => nested.visit_expression(test),
            None => nested.visit_statement(statement),
        }
        if self.exact && test.is_none() && self.split_sequence(statement, &nested) {
            return;
        }
        // The innermost call that is not the statement's own value or a
        // store's right side, and runs unconditionally.
        let inside = |outer: Span, inner: Span| outer.start <= inner.start && inner.end <= outer.end;
        let call = nested
            .calls
            .iter()
            .copied()
            .filter(|call| test.is_some() || *call != value)
            // A plain store's right side expands where it is; a property
            // store's goes first, its target evaluating no code.
            .filter(|call| !matches!(statement, Statement::ExpressionStatement(e)
                if matches!(&e.expression, Expression::AssignmentExpression(a)
                    if a.right.span() == *call
                        && matches!(a.left, AssignmentTarget::AssignmentTargetIdentifier(_)))))
            .filter(|call| !nested.calls.iter().any(|other| other != call && inside(*call, *other)))
            .find(|call| !nested.conditional.iter().any(|arm| inside(*arm, *call)));
        let call = match call {
            Some(call) => call,
            // A call that runs in one arm only: the choice holding it splits
            // into an `if` where it is the statement, else it moves out
            // whole like a call (its test still runs first).
            None if self.exact => {
                let Some(choice) = armed_choice(&nested) else {
                    return;
                };
                if test.is_none() && self.split_choice(statement, &choice, &nested) {
                    return;
                }
                // What did not split moves into a fresh temporary, which
                // splits next round; a statement that is nothing but the
                // choice has no other place to go.
                let fresh_temporary = matches!(statement, Statement::VariableDeclaration(declaration)
                    if matches!(&declaration.declarations[0].id, BindingPattern::BindingIdentifier(id) if id.name.starts_with("$SSA_x")));
                if (test.is_none()
                    && choice.span == value
                    && (fresh_temporary || matches!(statement, Statement::ExpressionStatement(_))))
                    || nested.functions.iter().any(|function| inside(choice.span, *function))
                {
                    return;
                }
                choice.span
            }
            None => return,
        };
        let call_text = &self.source[call.start as usize..call.end as usize];
        let inside_any = |spans: &[Span], span: Span| spans.iter().any(|outer| inside(*outer, span));
        // Code the statement runs before the call. The exact round binds
        // each outermost piece, in order, to a temporary ahead of the call
        // (`let t = f(); let u = H(x); g(t, u)`); otherwise nothing may run.
        let before: Vec<Span> = nested.code.iter().copied().filter(|span| span.end <= call.start).collect();
        let bound: Vec<Span> = before.iter().copied().filter(|span| !before.iter().any(|outer| outer != span && inside(*outer, *span))).collect();
        let mut fits = true;
        if !bound.is_empty() {
            let last = bound.iter().map(|span| span.start).max().unwrap_or(0);
            // Only unconditional pieces of the statement's value bind; no
            // property or changeable name read before one may move past it.
            if !self.exact
                || bound.iter().any(|span| span.start < value.start || nested.conditional.iter().any(|arm| inside(*arm, *span)))
                || nested.reads.iter().any(|(span, _)| span.end <= last && !inside_any(&bound, *span))
                || nested.names.iter().any(|span| span.end <= last && !inside_any(&bound, *span))
            {
                fits = false;
            }
        }
        // Nothing left before the call reads through a name it is handed.
        if fits
            && nested.reads.iter().any(|(span, root)| {
                span.end <= call.start
                    && !inside_any(&bound, *span)
                    && root.as_ref().is_none_or(|root| hands(call_text, root))
            })
        {
            fits = false;
        }
        // Otherwise the exact round binds every read and changeable name
        // before the call too, so the call moves across none of them.
        let bound = match fits {
            true => bound,
            false if self.exact => match bound_pieces(&nested, call, value) {
                Some(pieces) => pieces,
                None => return,
            },
            false => return,
        };
        let indent = indent_at(self.source, whole);
        let mut lines = String::new();
        for span in bound.iter().copied().chain(std::iter::once(call)) {
            let name = self.names.fresh();
            lines.push_str(&format!("let {name} = {};\n{indent}", &self.source[span.start as usize..span.end as usize]));
            self.edits.push(analysis::Edit::replace(span, name));
        }
        self.edits.push(analysis::Edit::replace(Span::new(whole.start, whole.start), lines));
        self.done += 1;
    }

    /// `a, H(x), b` holding a helper call, as a whole statement, an
    /// identifier store's or a declaration's value or the returned value,
    /// becomes one statement per element in the same order, the last one
    /// carrying the store, declaration or `return`:
    /// `x = (i = k, H(i), true)` → `i = k; H(i); x = true;`. Returns whether
    /// it split.
    fn split_sequence(&mut self, statement: &Statement<'_>, nested: &NestedCalls<'_>) -> bool {
        let source = self.source;
        let (sequence, prefix): (&SequenceExpression<'_>, String) = match statement {
            Statement::ExpressionStatement(expression) => match expression.expression.without_parentheses() {
                Expression::SequenceExpression(sequence) => (sequence, String::new()),
                Expression::AssignmentExpression(assignment) => {
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                        return false;
                    };
                    let Expression::SequenceExpression(sequence) = assignment.right.without_parentheses() else {
                        return false;
                    };
                    if assignment.operator != AssignmentOperator::Assign
                        || analysis::identifier_symbol(target, self.scoping).is_none()
                    {
                        return false;
                    }
                    (sequence, format!("{} = ", target.name))
                }
                _ => return false,
            },
            Statement::ReturnStatement(ret) => match ret.argument.as_ref().map(|argument| argument.without_parentheses()) {
                Some(Expression::SequenceExpression(sequence)) => (sequence, "return ".to_string()),
                _ => return false,
            },
            Statement::VariableDeclaration(declaration)
                if declaration.declarations.len() == 1
                    && matches!(
                        declaration.kind,
                        VariableDeclarationKind::Let | VariableDeclarationKind::Const | VariableDeclarationKind::Var
                    ) =>
            {
                let declarator = &declaration.declarations[0];
                let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                    return false;
                };
                let Some(Expression::SequenceExpression(sequence)) =
                    declarator.init.as_ref().map(|init| init.without_parentheses())
                else {
                    return false;
                };
                let kind = &source[declaration.span.start as usize..id.span.start as usize];
                (sequence, format!("{kind}{} = ", id.name))
            }
            _ => return false,
        };
        let inside = |outer: Span, inner: Span| outer.start <= inner.start && inner.end <= outer.end;
        if !nested.calls.iter().any(|call| inside(sequence.span, *call)) {
            return false;
        }
        let whole = statement.span();
        let indent = indent_at(source, whole);
        let elements: Vec<Span> = sequence.expressions.iter().map(GetSpan::span).collect();
        let count = elements.len();
        // An element on its own as a statement must not read as a block,
        // declaration or function.
        let parens: Vec<(&str, &str)> = elements
            .iter()
            .enumerate()
            .map(|(index, span)| {
                let bare = index + 1 < count || prefix.is_empty();
                let text = source[span.start as usize..span.end as usize].trim_start();
                if bare && ["{", "function", "class", "let", "async"].iter().any(|start| text.starts_with(start)) {
                    ("(", ")")
                } else {
                    ("", "")
                }
            })
            .collect();
        let lead = |index: usize| if index + 1 == count { prefix.as_str() } else { "" };
        self.edits.push(analysis::Edit::replace(
            Span::new(whole.start, elements[0].start),
            format!("{}{}", lead(0), parens[0].0),
        ));
        for index in 1..count {
            self.edits.push(analysis::Edit::replace(
                Span::new(elements[index - 1].end, elements[index].start),
                format!("{};\n{indent}{}{}", parens[index - 1].1, lead(index), parens[index].0),
            ));
        }
        self.edits.push(analysis::Edit::replace(
            Span::new(elements[count - 1].end, whole.end),
            format!("{};", parens[count - 1].1),
        ));
        self.done += 1;
        true
    }

    /// `t ? a : b` (or `l && r`, …) as a whole statement, an identifier
    /// store's or a declaration's value, or the returned value becomes an
    /// `if` whose arms are statements of their own, so a helper call in an
    /// arm expands there, and only there. The test still runs first and
    /// each arm's text stays as it is. A declared name is assigned in the
    /// arms only when nothing but the code after it can read it (no
    /// closure, no read inside the statement). Returns whether it split.
    fn split_choice(&mut self, statement: &Statement<'_>, choice: &Choice, nested: &NestedCalls<'_>) -> bool {
        let inside = |outer: Span, inner: Span| outer.start <= inner.start && inner.end <= outer.end;
        if nested.chains.iter().any(|chain| inside(*chain, choice.span)) {
            return false;
        }
        let source = self.source;
        let text = |span: Span| &source[span.start as usize..span.end as usize];
        let whole = statement.span();
        let indent = indent_at(source, whole);
        let inner = format!("{indent}\t");
        let unparenthesized = |expression: &Expression<'_>| expression.without_parentheses().span();
        // What goes before each arm (`x = `, `return `), and the text the
        // statement starts with ahead of the `if`.
        let (prefix, store) = match statement {
            Statement::ExpressionStatement(expression) => {
                if unparenthesized(&expression.expression) == choice.span {
                    (String::new(), String::new())
                } else {
                    // `x = t ? a : b`: `x` names the same binding either way.
                    let Expression::AssignmentExpression(assignment) = expression.expression.without_parentheses() else {
                        return false;
                    };
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                        return false;
                    };
                    if assignment.operator != AssignmentOperator::Assign
                        || unparenthesized(&assignment.right) != choice.span
                        || choice.operator.is_some()
                        || analysis::identifier_symbol(target, self.scoping).is_none()
                    {
                        return false;
                    }
                    (String::new(), format!("{} = ", target.name))
                }
            }
            Statement::ReturnStatement(ret) => {
                if choice.operator.is_some() || ret.argument.as_ref().is_none_or(|a| unparenthesized(a) != choice.span) {
                    return false;
                }
                (String::new(), "return ".to_string())
            }
            Statement::VariableDeclaration(declaration) => {
                let declarator = &declaration.declarations[0];
                let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                    return false;
                };
                let Some(symbol) = id.symbol_id.get() else {
                    return false;
                };
                if !matches!(declaration.kind, VariableDeclarationKind::Let | VariableDeclarationKind::Var)
                    || declarator.init.as_ref().is_none_or(|init| unparenthesized(init) != choice.span)
                    || self.captured_or_read_in(symbol, whole)
                {
                    return false;
                }
                let kind = text(Span::new(declaration.span.start, id.span.start));
                let name = id.name.as_str();
                if let Some(operator) = choice.operator {
                    // `let x = l && r` is `let x = l; if (x) x = r;`.
                    let condition = match operator {
                        LogicalOperator::And => name.to_string(),
                        LogicalOperator::Or => format!("!{name}"),
                        LogicalOperator::Coalesce => format!("{name} == null"),
                    };
                    self.edits.push(analysis::Edit::replace(Span::new(whole.start, choice.test.start), format!("{kind}{name} = ")));
                    self.edits.push(analysis::Edit::replace(
                        Span::new(choice.test.end, choice.consequent.start),
                        format!(";\n{indent}if ({condition}) {{\n{inner}{name} = "),
                    ));
                    self.edits.push(analysis::Edit::replace(Span::new(choice.consequent.end, whole.end), format!(";\n{indent}}}")));
                    self.done += 1;
                    return true;
                }
                (format!("{kind}{name};\n{indent}"), format!("{name} = "))
            }
            _ => return false,
        };
        // An arm on its own as a statement must not read as a block,
        // declaration or function.
        let wrap = |span: Span| {
            let arm = text(span).trim_start();
            store.is_empty() && ["{", "function", "class", "let", "async"].iter().any(|start| arm.starts_with(start))
        };
        let open = |span: Span| if wrap(span) { "(" } else { "" };
        let close = |span: Span| if wrap(span) { ")" } else { "" };
        let (test_open, test_close) = match choice.operator {
            None | Some(LogicalOperator::And) => ("", ""),
            Some(LogicalOperator::Or) => ("!(", ")"),
            Some(LogicalOperator::Coalesce) => ("(", ") == null"),
        };
        self.edits.push(analysis::Edit::replace(Span::new(whole.start, choice.test.start), format!("{prefix}if ({test_open}")));
        self.edits.push(analysis::Edit::replace(
            Span::new(choice.test.end, choice.consequent.start),
            format!("{test_close}) {{\n{inner}{store}{}", open(choice.consequent)),
        ));
        let last = match choice.alternate {
            Some(alternate) => {
                self.edits.push(analysis::Edit::replace(
                    Span::new(choice.consequent.end, alternate.start),
                    format!("{};\n{indent}}} else {{\n{inner}{store}{}", close(choice.consequent), open(alternate)),
                ));
                alternate
            }
            None => choice.consequent,
        };
        self.edits.push(analysis::Edit::replace(Span::new(last.end, whole.end), format!("{};\n{indent}}}", close(last))));
        self.done += 1;
        true
    }

    /// A read of `symbol` inside `span`, or from a function nested below the
    /// one declaring it.
    fn captured_or_read_in(&self, symbol: SymbolId, span: Span) -> bool {
        let owner = self.scoping.symbol_scope_id(symbol);
        self.scoping.get_resolved_references(symbol).any(|reference| {
            let node_span = self.nodes.get_node(reference.node_id()).kind().span();
            if span.start <= node_span.start && node_span.end <= span.end {
                return true;
            }
            for scope in self.scoping.scope_ancestors(reference.scope_id()) {
                if scope == owner {
                    return false;
                }
                if self.scoping.scope_flags(scope).intersects(ScopeFlags::Function | ScopeFlags::Arrow) {
                    return true;
                }
            }
            false
        })
    }

    pub(super) fn list(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            self.statement(statement);
        }
    }
}
