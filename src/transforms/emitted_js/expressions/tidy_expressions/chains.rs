//! Optional chains written out as `a == null ? void 0 : a.b`, folded back.

use super::*;

impl<'a> Unwrap<'a> {
    pub(super) fn fold_optional_chain(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>, at: usize) -> bool {
        // let x = A;
        let Statement::VariableDeclaration(first) = &statements[at] else {
            return false;
        };
        let [declarator] = first.declarations.as_slice() else {
            return false;
        };
        let (Some(object), Some(_)) = (declarator.id.get_binding_identifier(), declarator.init.as_ref()) else {
            return false;
        };
        let x = object.name;
        // let j;
        let Statement::VariableDeclaration(second) = &statements[at + 1] else {
            return false;
        };
        let [join] = second.declarations.as_slice() else {
            return false;
        };
        let Some(join_binding) = join.id.get_binding_identifier() else {
            return false;
        };
        if join.init.is_some() || second.kind == VariableDeclarationKind::Var {
            return false;
        }
        let j = join_binding.name;
        if self.read_counts.get(x.as_str()) != Some(&5) || !self.own_names.contains(x.as_str()) {
            return false;
        }
        let is_name = |expression: &Expression<'_>, name: &str| matches!(expression.without_parentheses(), Expression::Identifier(id) if id.name == name);
        let null_test = |expression: &Expression<'_>, name: &str| {
            matches!(expression.without_parentheses(), Expression::BinaryExpression(binary)
                if binary.operator == BinaryOperator::Equality
                    && ((is_name(&binary.left, name) && matches!(binary.right, Expression::NullLiteral(_)))
                        || (is_name(&binary.right, name) && matches!(binary.left, Expression::NullLiteral(_)))))
        };
        let store = |statement: &Statement<'_>, name: &str| -> bool {
            matches!(statement, Statement::ExpressionStatement(expression)
                if matches!(&expression.expression, Expression::AssignmentExpression(assignment)
                    if assignment.operator == AssignmentOperator::Assign
                        && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(target) if target.name == name)))
        };
        let member_of = |expression: &Expression<'_>, name: &str| match expression.without_parentheses() {
            Expression::StaticMemberExpression(member) => !member.optional && is_name(&member.object, name),
            Expression::ComputedMemberExpression(member) => !member.optional && is_name(&member.object, name) && !mentions(&member.expression, name),
            _ => false,
        };
        // if (null == x) { j = true; } else { x = x.p; j = x == null; }
        let Statement::IfStatement(branch) = &statements[at + 2] else {
            return false;
        };
        let shape = null_test(&branch.test, &x)
            && matches!(&branch.consequent, Statement::BlockStatement(block) if matches!(block.body.as_slice(), [only]
                if store(only, &j) && matches!(only, Statement::ExpressionStatement(e)
                    if matches!(&e.expression, Expression::AssignmentExpression(a) if matches!(a.right, Expression::BooleanLiteral(ref t) if t.value)))))
            && matches!(&branch.alternate, Some(Statement::BlockStatement(block)) if matches!(block.body.as_slice(), [step, test]
                if store(step, &x) && store(test, &j)
                    && matches!(step, Statement::ExpressionStatement(e) if matches!(&e.expression, Expression::AssignmentExpression(a) if member_of(&a.right, &x)))
                    && matches!(test, Statement::ExpressionStatement(e) if matches!(&e.expression, Expression::AssignmentExpression(a) if null_test(&a.right, &x)))));
        if !shape {
            return false;
        }
        // j = j ? void 0 : x.q;
        let last_fits = matches!(&statements[at + 3], Statement::ExpressionStatement(expression)
            if matches!(&expression.expression, Expression::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign
                    && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(target) if target.name == j)
                    && matches!(assignment.right.without_parentheses(), Expression::ConditionalExpression(choice)
                        if is_name(&choice.test, &j)
                            && matches!(&choice.consequent, Expression::UnaryExpression(u) if u.operator == UnaryOperator::Void)
                            && member_of(&choice.alternate, &x))));
        if !last_fits {
            return false;
        }
        // Take the pieces.
        let mut removed: Vec<Statement<'a>> = statements.drain(at..at + 4).collect();
        let Statement::ExpressionStatement(last) = removed.pop().expect("four") else { unreachable!() };
        let Statement::IfStatement(branch) = removed.pop().expect("three") else { unreachable!() };
        let Statement::VariableDeclaration(mut join_declaration) = removed.pop().expect("two") else { unreachable!() };
        let Statement::VariableDeclaration(mut first) = removed.pop().expect("one") else { unreachable!() };
        let base = first.declarations[0].init.take().expect("an initializer");
        let Some(Statement::BlockStatement(alternate)) = branch.unbox().alternate else { unreachable!() };
        let Some(Statement::ExpressionStatement(step)) = alternate.unbox().body.into_iter().next() else { unreachable!() };
        let Expression::AssignmentExpression(step) = step.unbox().expression else { unreachable!() };
        let Expression::AssignmentExpression(last) = last.unbox().expression else { unreachable!() };
        let Expression::ConditionalExpression(choice) = last.unbox().right.into_inner_expression() else { unreachable!() };
        let optional = |member: Expression<'a>, object: Expression<'a>| -> Expression<'a> {
            match member.into_inner_expression() {
                Expression::StaticMemberExpression(mut member) => {
                    member.object = object;
                    member.optional = true;
                    Expression::StaticMemberExpression(member)
                }
                Expression::ComputedMemberExpression(mut member) => {
                    member.object = object;
                    member.optional = true;
                    Expression::ComputedMemberExpression(member)
                }
                _ => unreachable!(),
            }
        };
        let inner = optional(step.unbox().right, base);
        let outer = optional(choice.unbox().alternate, inner);
        let element = match outer {
            Expression::StaticMemberExpression(member) => ChainElement::StaticMemberExpression(member),
            Expression::ComputedMemberExpression(member) => ChainElement::ComputedMemberExpression(member),
            _ => unreachable!(),
        };
        join_declaration.declarations[0].init = Some(self.builder.expression_chain(SPAN, element));
        let _ = &mut first;
        statements.insert(at, Statement::VariableDeclaration(join_declaration));
        true
    }

    /// `let j, x; if (A == null) { j = true; x = A; } else { x = A.p;
    /// j = null == x; } j = j ? void 0 : x.q;` is `let j = A?.p?.q;` when
    /// `A` is a property path read the same twice.
    pub(super) fn fold_repeated_optional_chain(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>, at: usize) -> bool {
        let Statement::VariableDeclaration(declaration) = &statements[at] else {
            return false;
        };
        if declaration.kind == VariableDeclarationKind::Var
            || declaration.declarations.len() != 2
            || declaration.declarations.iter().any(|d| d.init.is_some() || d.id.get_binding_identifier().is_none())
        {
            return false;
        }
        let names: Vec<_> = declaration.declarations.iter().map(|d| d.id.get_binding_identifier().unwrap().name).collect();
        let is_name = |expression: &Expression<'_>, name: &str| matches!(expression.without_parentheses(), Expression::Identifier(id) if id.name == name);
        fn path(expression: &Expression<'_>) -> bool {
            match expression.without_parentheses() {
                Expression::Identifier(_) => true,
                Expression::StaticMemberExpression(member) => !member.optional && path(&member.object),
                Expression::ComputedMemberExpression(member) => {
                    !member.optional && matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_)) && path(&member.object)
                }
                _ => false,
            }
        }
        let tested = |expression: &Expression<'a>| -> Option<Expression<'a>> {
            match expression.without_parentheses() {
                Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Equality => {
                    if matches!(binary.right, Expression::NullLiteral(_)) {
                        Some(binary.left.clone_in(self.builder.allocator))
                    } else if matches!(binary.left, Expression::NullLiteral(_)) {
                        Some(binary.right.clone_in(self.builder.allocator))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        let Statement::IfStatement(branch) = &statements[at + 1] else {
            return false;
        };
        let Some(base) = tested(&branch.test) else {
            return false;
        };
        if !path(&base) || !base.is_member_expression() {
            return false;
        }
        fn assignment<'s, 'b>(statement: &'s Statement<'b>) -> Option<(&'s str, &'s Expression<'b>)> {
            let Statement::ExpressionStatement(expression) = statement else { return None };
            let Expression::AssignmentExpression(assignment) = &expression.expression else { return None };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else { return None };
            (assignment.operator == AssignmentOperator::Assign).then_some((target.name.as_str(), &assignment.right))
        }
        // Which name is the flag: the one the consequent stores `true` in.
        let Statement::BlockStatement(consequent) = &branch.consequent else { return false };
        let Some(Statement::BlockStatement(alternate)) = &branch.alternate else { return false };
        let mut flag = None;
        let mut copies = 0;
        for statement in &consequent.body {
            match assignment(statement) {
                Some((name, Expression::BooleanLiteral(value))) if value.value => flag = Some(name.to_string()),
                Some((_, value)) if value.content_eq(&base) => copies += 1,
                _ => return false,
            }
        }
        let Some(j) = flag else { return false };
        let Some(x) = names.iter().map(|n| n.as_str()).find(|n| *n != j) else { return false };
        if !names.iter().any(|n| n.as_str() == j) || !self.own_names.contains(x) {
            return false;
        }
        let [step, test] = alternate.body.as_slice() else { return false };
        let step_fits = matches!(assignment(step), Some((name, value)) if name == x && match value.without_parentheses() {
            Expression::StaticMemberExpression(member) => !member.optional && member.object.content_eq(&base),
            Expression::ComputedMemberExpression(member) => !member.optional && member.object.content_eq(&base) && !mentions(&member.expression, x),
            _ => false,
        });
        let test_fits = matches!(assignment(test), Some((name, value)) if name == j
            && tested(value).is_some_and(|tested| is_name(&tested, x)));
        if !step_fits || !test_fits {
            return false;
        }
        let last_fits = matches!(assignment(&statements[at + 2]), Some((name, Expression::ConditionalExpression(choice))) if name == j
            && is_name(&choice.test, &j)
            && matches!(&choice.consequent, Expression::UnaryExpression(u) if u.operator == UnaryOperator::Void)
            && match choice.alternate.without_parentheses() {
                Expression::StaticMemberExpression(member) => !member.optional && is_name(&member.object, x),
                Expression::ComputedMemberExpression(member) => !member.optional && is_name(&member.object, x) && !mentions(&member.expression, x),
                _ => false,
            });
        if !last_fits || self.read_counts.get(x) != Some(&(copies + 3)) {
            return false;
        }
        let j_name = names.iter().copied().find(|n| n.as_str() == j).unwrap();
        let mut removed: Vec<Statement<'a>> = statements.drain(at..at + 3).collect();
        let Statement::ExpressionStatement(last) = removed.pop().expect("three") else { unreachable!() };
        let Statement::IfStatement(branch) = removed.pop().expect("two") else { unreachable!() };
        let Statement::VariableDeclaration(mut declaration) = removed.pop().expect("one") else { unreachable!() };
        let branch = branch.unbox();
        let Some(Statement::BlockStatement(alternate)) = branch.alternate else { unreachable!() };
        let Some(Statement::ExpressionStatement(step)) = alternate.unbox().body.into_iter().next() else { unreachable!() };
        let Expression::AssignmentExpression(step) = step.unbox().expression else { unreachable!() };
        let Expression::AssignmentExpression(last) = last.unbox().expression else { unreachable!() };
        let Expression::ConditionalExpression(choice) = last.unbox().right.into_inner_expression() else { unreachable!() };
        let optional = |member: Expression<'a>, object: Expression<'a>| -> Expression<'a> {
            match member.into_inner_expression() {
                Expression::StaticMemberExpression(mut member) => {
                    member.object = object;
                    member.optional = true;
                    Expression::StaticMemberExpression(member)
                }
                Expression::ComputedMemberExpression(mut member) => {
                    member.object = object;
                    member.optional = true;
                    Expression::ComputedMemberExpression(member)
                }
                _ => unreachable!(),
            }
        };
        let inner = optional(step.unbox().right, base);
        let outer = optional(choice.unbox().alternate, inner);
        let element = match outer {
            Expression::StaticMemberExpression(member) => ChainElement::StaticMemberExpression(member),
            Expression::ComputedMemberExpression(member) => ChainElement::ComputedMemberExpression(member),
            _ => unreachable!(),
        };
        declaration.declarations.retain(|d| d.id.get_binding_identifier().is_some_and(|b| b.name == j_name));
        declaration.declarations[0].init = Some(self.builder.expression_chain(SPAN, element));
        statements.insert(at, Statement::VariableDeclaration(declaration));
        true
    }
}
