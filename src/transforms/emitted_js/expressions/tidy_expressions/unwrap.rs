//! The final tidy's walk: unbound calls, kept expressions and every list it rewrites.

use super::*;

impl<'a> Unwrap<'a> {
    /// `(0, o.m)(x)` -> `o.m.call(void 0, x)`.
    pub(super) fn unbound_call(&mut self, call: &mut CallExpression<'a>) {
        let b = self.builder;
        let callee = std::mem::replace(&mut call.callee, b.expression_null_literal(SPAN));
        let Expression::SequenceExpression(mut sequence) = callee else {
            unreachable!("checked by wraps_member");
        };
        let mut member = sequence.expressions.pop().expect("two expressions");
        self.visit_expression(&mut member);
        call.callee = Expression::from(b.member_expression_static(
            SPAN,
            member,
            b.identifier_name(SPAN, "call"),
            false,
        ));
        let undefined = b.expression_unary(
            SPAN,
            UnaryOperator::Void,
            b.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal),
        );
        call.arguments.insert(0, Argument::from(undefined));
        self.unwrapped += 1;
    }

    pub(super) fn visit_kept(&mut self, expression: &mut Expression<'a>) {
        self.keep = true;
        self.visit_expression(expression);
    }
}

impl<'a> VisitMut<'a> for Unwrap<'a> {
    // A bare `;` in a statement list is what a deleted statement leaves.
    fn visit_statements(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>) {
        let discards = self.counter_steps;
        statements.retain(|statement| match statement {
            Statement::EmptyStatement(_) => false,
            // A read the VM left behind with its value unused (`arguments[0];`,
            // `x + 1;`) does nothing.
            Statement::ExpressionStatement(expression) => !(discards && discardable(&expression.expression)),
            _ => true,
        });
        walk_mut::walk_statements(self, statements);
        if self.counter_steps {
            for statement in statements.iter_mut() {
                let Statement::ReturnStatement(ret) = statement else { continue };
                let Some(Expression::ConditionalExpression(choice)) = &ret.argument else { continue };
                let busy = |e: &Expression<'_>| !matches!(e.without_parentheses(),
                    Expression::Identifier(_) | Expression::NumericLiteral(_) | Expression::StringLiteral(_)
                    | Expression::BooleanLiteral(_) | Expression::NullLiteral(_) | Expression::UnaryExpression(_)
                    | Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_));
                if !(busy(&choice.consequent) || busy(&choice.alternate)) {
                    continue;
                }
                let Some(Expression::ConditionalExpression(choice)) = ret.argument.take() else { unreachable!() };
                let choice = choice.unbox();
                let yes = self.builder.statement_return(SPAN, Some(choice.consequent));
                let no = self.builder.statement_return(SPAN, Some(choice.alternate));
                let yes = self.builder.statement_block(SPAN, self.builder.vec1(yes));
                let no = self.builder.statement_block(SPAN, self.builder.vec1(no));
                *statement = self.builder.statement_if(SPAN, choice.test, yes, Some(no));
            }
            // A bare block is its statements when the names it declares are
            // bound nowhere else.
            let movable = |binding: &BindingIdentifier<'a>| {
                binding.symbol_id.get().is_some_and(|symbol| {
                    self.read_counts.contains_key(&format!("\u{0}movable {}", symbol.index()))
                })
            };
            let bare = |statement: &Statement<'a>| {
                matches!(statement, Statement::BlockStatement(block) if !block.body.iter().any(|inner| match inner {
                    Statement::VariableDeclaration(declaration) if declaration.kind != VariableDeclarationKind::Var => {
                        !declaration.declarations.iter().all(|declarator| {
                            declarator.id.get_binding_identifier().is_some_and(movable)
                        })
                    }
                    // A function declared in the block becomes available
                    // from the start of the outer block instead; nothing
                    // there can name it when its name is bound nowhere else.
                    Statement::FunctionDeclaration(function) => !function.id.as_ref().is_some_and(movable),
                    Statement::ClassDeclaration(_) => true,
                    _ => false,
                }))
            };
            if statements.iter().any(bare) {
                let mut out = self.builder.vec_with_capacity(statements.len());
                for statement in statements.drain(..) {
                    if bare(&statement) {
                        let Statement::BlockStatement(block) = statement else {
                            unreachable!();
                        };
                        out.extend(block.unbox().body);
                    } else {
                        out.push(statement);
                    }
                }
                *statements = out;
            }
            let mut index = 0;
            while index < statements.len() {
                if !self.restructure_loop(statements, index) {
                    index += 1;
                }
            }
            self.fold_increments(statements);
        }
    }

    // `else {}` says nothing; `if (t) {} else { X }` is `if (!t) { X }`.
    fn visit_if_statement(&mut self, branch: &mut IfStatement<'a>) {
        walk_mut::walk_if_statement(self, branch);
        let empty = |statement: &Statement<'a>| matches!(statement, Statement::BlockStatement(block) if block.body.is_empty());
        // `if (!t) A else B` is `if (t) B else A` (an `else if` chain stays).
        if self.counter_steps {
            normalize(&mut branch.test, self.builder);
            let swap = matches!(&branch.test, Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot)
                && branch.alternate.is_some()
                && !matches!(branch.alternate, Some(Statement::IfStatement(_)))
                && !matches!(&branch.alternate, Some(Statement::BlockStatement(block)) if matches!(block.body.as_slice(), [Statement::IfStatement(_)]));
            if swap {
                let Expression::UnaryExpression(unary) = &mut branch.test else { unreachable!() };
                let test = std::mem::replace(&mut unary.argument, self.builder.expression_null_literal(SPAN));
                branch.test = test;
                let alternate = branch.alternate.take().expect("an else");
                let consequent = std::mem::replace(&mut branch.consequent, alternate);
                branch.alternate = Some(consequent);
            }
        }
        // `else { if … }` is `else if …`.
        if self.counter_steps {
            if let Some(Statement::BlockStatement(block)) = &mut branch.alternate {
                if block.body.len() == 1 && matches!(block.body[0], Statement::IfStatement(_)) {
                    let inner = block.body.pop().expect("one statement");
                    branch.alternate = Some(inner);
                }
            }
        }
        if branch.alternate.as_ref().is_some_and(empty) {
            branch.alternate = None;
        } else if empty(&branch.consequent) {
            if let Some(alternate) = branch.alternate.take() {
                let test = std::mem::replace(&mut branch.test, self.builder.expression_null_literal(SPAN));
                branch.test = self.builder.expression_unary(SPAN, UnaryOperator::LogicalNot, test);
                branch.consequent = alternate;
            }
        }
    }

    // `x = Number(x) + 1` is what `++x` does, and `x - 1` converts `x` the
    // way `--x` does (BigInt aside, which the program never makes).
    fn visit_expression_statement(&mut self, statement: &mut ExpressionStatement<'a>) {
        walk_mut::walk_expression_statement(self, statement);
        if self.counter_steps {
            if let Some(update) = self.counter_step(&statement.expression, true) {
                statement.expression = update;
            } else if let Expression::AssignmentExpression(assignment) = &mut statement.expression {
                // `i = i + 1 + 1` is `i += 2`.
                let total = match (&assignment.left, offset(&assignment.right)) {
                    (AssignmentTarget::AssignmentTargetIdentifier(target), Some((name, n)))
                        if assignment.operator == AssignmentOperator::Assign && name.name == target.name && n > 1.0 =>
                    {
                        Some(n)
                    }
                    _ => None,
                };
                if let Some(n) = total {
                    assignment.operator = AssignmentOperator::Addition;
                    assignment.right = self.builder.expression_numeric_literal(SPAN, n, None, NumberBase::Decimal);
                }
            }
        }
    }

    // `for (;;) { if (c) break; … }` is `while (!c) { … }`.
    fn visit_statement(&mut self, statement: &mut Statement<'a>) {
        walk_mut::walk_statement(self, statement);
        if self.counter_steps {
            self.reform_loop(statement);
        }
    }

    // `return c ? a : b` with an arm that does real work reads better as
    // `if (c) { return a; } else { return b; }`.
    fn visit_return_statement(&mut self, statement: &mut ReturnStatement<'a>) {
        walk_mut::walk_return_statement(self, statement);
    }

    // A `for` update steps the counter after the body: `i++`.
    fn visit_for_statement(&mut self, statement: &mut ForStatement<'a>) {
        walk_mut::walk_for_statement(self, statement);
        if self.counter_steps {
            if let Some(update) = statement.update.as_ref().and_then(|update| self.counter_step(update, false)) {
                statement.update = Some(update);
            }
        }
    }

    fn visit_expression(&mut self, expression: &mut Expression<'a>) {
        let keep = std::mem::take(&mut self.keep);
        walk_mut::walk_expression(self, expression);
        // `[a, b][1]` is `b` when the other elements read nothing that can
        // throw (literals and generated names): what an expanded tuple helper
        // leaves at a slot read.
        if let Some(element) = literal_slot(expression, self.builder) {
            *expression = element;
            self.unwrapped += 1;
            return;
        }
        if self.counter_steps {
            if let Some(value) = folded_nullish(expression) {
                *expression = self.builder.expression_boolean_literal(SPAN, value);
                return;
            }
            normalize(expression, self.builder);
            // `s + "a" + 2` is `s + "a2"`: once a string is on the left, the
            // rest concatenates.
            if let Some(joined) = joined_strings(expression) {
                let Expression::BinaryExpression(binary) = expression else {
                    unreachable!();
                };
                let literal = self.builder.expression_string_literal(SPAN, self.builder.atom(&joined), None);
                if matches!(binary.left, Expression::StringLiteral(_) | Expression::NumericLiteral(_)) {
                    *expression = literal;
                } else {
                    let Expression::BinaryExpression(inner) = &mut binary.left else {
                        unreachable!();
                    };
                    let rest = std::mem::replace(&mut inner.left, self.builder.expression_null_literal(SPAN));
                    *expression = self.builder.expression_binary(SPAN, rest, BinaryOperator::Addition, literal);
                }
                return;
            }
            // In a bitwise operand every falsy value is 0, like `false`:
            // `!!x && y` there is `x && y`.
            if let Expression::BinaryExpression(binary) = expression {
                if matches!(
                    binary.operator,
                    BinaryOperator::BitwiseOR
                        | BinaryOperator::BitwiseAnd
                        | BinaryOperator::BitwiseXOR
                        | BinaryOperator::ShiftLeft
                        | BinaryOperator::ShiftRight
                        | BinaryOperator::ShiftRightZeroFill
                ) {
                    strip_double_not(&mut binary.left, self.builder);
                    strip_double_not(&mut binary.right, self.builder);
                    drop_nan_guard(&mut binary.left, self.builder);
                    drop_nan_guard(&mut binary.right, self.builder);
                }
            }
        }
        // `i + 1 + 1` is `i + 2` (the VM's offsets are numbers), and `3 + 1`
        // is `4`.
        if self.counter_steps {
            let nested = matches!(&*expression, Expression::BinaryExpression(binary) if offset(&binary.left).is_some());
            let found = offset(expression).map(|(name, n)| (name.name, n));
            if let Some((name, n)) = found {
                if nested && self.own_names.contains(name.as_str()) {
                    let left = self.builder.expression_identifier(SPAN, name);
                    let right = self.builder.expression_numeric_literal(SPAN, n, None, NumberBase::Decimal);
                    *expression = self.builder.expression_binary(SPAN, left, BinaryOperator::Addition, right);
                    return;
                }
            }
            if let Some(value) = folded_number(expression) {
                *expression = self.builder.expression_numeric_literal(SPAN, value, None, NumberBase::Decimal);
                return;
            }
        }
        if keep {
            return;
        }
        let inner = {
            let Expression::SequenceExpression(sequence) = expression else {
                return;
            };
            if sequence.expressions.len() != 2
                || !matches!(sequence.expressions[0], Expression::NumericLiteral(_))
            {
                return;
            }
            sequence.expressions.pop().expect("two expressions")
        };
        *expression = inner;
        self.unwrapped += 1;
    }

    fn visit_call_expression(&mut self, call: &mut CallExpression<'a>) {
        if !call.optional && wraps_member(&call.callee) {
            self.unbound_call(call);
        } else if self.plain_callees && wraps_identifier(&call.callee) {
            self.visit_expression(&mut call.callee);
        } else {
            self.visit_kept(&mut call.callee);
        }
        for argument in call.arguments.iter_mut() {
            self.visit_argument(argument);
        }
    }

    fn visit_tagged_template_expression(&mut self, tagged: &mut TaggedTemplateExpression<'a>) {
        if self.plain_callees && wraps_identifier(&tagged.tag) {
            self.visit_expression(&mut tagged.tag);
        } else {
            self.visit_kept(&mut tagged.tag);
        }
        self.visit_template_literal(&mut tagged.quasi);
    }

    fn visit_unary_expression(&mut self, unary: &mut UnaryExpression<'a>) {
        let observable = unary.operator == UnaryOperator::Delete
            || unary.operator == UnaryOperator::Typeof
                && matches!(&unary.argument, Expression::SequenceExpression(sequence)
                    if matches!(sequence.expressions.last(), Some(Expression::Identifier(_))));
        if observable {
            self.visit_kept(&mut unary.argument);
        } else {
            walk_mut::walk_unary_expression(self, unary);
        }
    }
}

/// The element a literal index picks out of an array literal whose other
/// elements run no code (see [`quiet_value`]), taken out of `expression`.
fn literal_slot<'a>(expression: &mut Expression<'a>, builder: AstBuilder<'a>) -> Option<Expression<'a>> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    if member.optional {
        return None;
    }
    let Expression::NumericLiteral(index) = &member.expression else {
        return None;
    };
    let index = index.value;
    let mut object = &mut member.object;
    while let Expression::ParenthesizedExpression(inner) = object {
        object = &mut inner.expression;
    }
    let Expression::ArrayExpression(array) = object else {
        return None;
    };
    if index < 0.0 || index.fract() != 0.0 || index as usize >= array.elements.len() {
        return None;
    }
    let at = index as usize;
    // The picked element runs as before; the others must run nothing.
    let quiet = |element: &ArrayExpressionElement<'_>| element.as_expression().is_some_and(quiet_value);
    if array.elements[at].as_expression().is_none()
        || !array.elements.iter().enumerate().all(|(i, element)| i == at || quiet(element))
    {
        return None;
    }
    let element = array.elements.get_mut(at)?;
    let taken = std::mem::replace(element, ArrayExpressionElement::from(builder.expression_null_literal(SPAN)));
    taken.into_expression().into()
}

/// A literal, a generated name, or `!`/`typeof`/`void` of one:
/// evaluating it runs no code and cannot throw.
fn quiet_value(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Identifier(id) => id.name.starts_with('$'),
        Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
            ) && quiet_value(&unary.argument)
        }
        Expression::ParenthesizedExpression(inner) => quiet_value(&inner.expression),
        _ => false,
    }
}
