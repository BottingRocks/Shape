//! Loops: moving an exit after the loop, counters and their steps, and the jump helpers.

use super::*;

impl<'a> Unwrap<'a> {
    /// `L: for (;;) { S; if (t) { A; continue L; } B; break L; }` (or the
    /// same with `else { B }` where `B` leaves) is
    /// `for (;;) { S; if (!t) break; A } B`: the exit path runs once, after
    /// the loop. A `let` of `S` that `B` reads is declared before the loop
    /// (no closure sees it). Returns whether it rewrote, so the new
    /// statements are looked at again.
    pub(super) fn restructure_loop(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>, at: usize) -> bool {
        let Some((split, flipped, hoist)) = self.loop_exit_plan(&statements[at]) else {
            return false;
        };
        self.move_exit_after_loop(statements, at, split, flipped, &hoist);
        true
    }

    /// Where the loop at `statement` splits (the continuing `if`, and whether
    /// its else arm is the one that stays) and the head lets to hoist, when
    /// the exit path can run after the loop.
    fn loop_exit_plan(&self, statement: &Statement<'a>) -> Option<(usize, bool, Vec<String>)> {
        let (label, loop_) = match statement {
            Statement::LabeledStatement(labeled) => match &labeled.body {
                Statement::ForStatement(loop_) => (Some(labeled.label.name.as_str()), loop_),
                _ => return None,
            },
            Statement::ForStatement(loop_) => (None, loop_),
            _ => return None,
        };
        if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() {
            return None;
        }
        let Statement::BlockStatement(body) = &loop_.body else {
            return None;
        };
        let body = &body.body;
        let (split, flipped) = continuing_split(body, label)?;
        // No other jump to this loop in the exit path, and the stay path's
        // only exits are its own.
        if exit_jumps_back(body, split, flipped, label) {
            return None;
        }
        // The exit runs after the loop, so it must be the loop's only way
        // out: a `break` before the split or on the stay path would run it
        // too.
        let other_breaks = {
            let Statement::IfStatement(branch) = &body[split] else { unreachable!() };
            let stay = if flipped { branch.alternate.as_ref().unwrap() } else { &branch.consequent };
            body[..split].iter().any(|s| breaks_from(s, label)) || breaks_from(stay, label)
        };
        if other_breaks {
            return None;
        }
        let hoist = self.exit_reads(body, split, flipped)?;
        Some((split, flipped, hoist))
    }

    /// Lets of the statements before the split that the exit reads, to be
    /// declared before the loop; `None` when one cannot be.
    fn exit_reads(&self, body: &[Statement<'a>], split: usize, flipped: bool) -> Option<Vec<String>> {
        let head_lets: Vec<(String, bool)> = body[..split]
            .iter()
            .filter_map(|s| match s {
                Statement::VariableDeclaration(declaration) if declaration.kind != VariableDeclarationKind::Var => Some(
                    declaration
                        .declarations
                        .iter()
                        .filter_map(|d| d.id.get_binding_identifier().map(|b| (b.name.to_string(), declaration.kind == VariableDeclarationKind::Const)))
                        .collect::<Vec<_>>(),
                ),
                Statement::FunctionDeclaration(_) | Statement::ClassDeclaration(_) => Some(vec![("\u{0}".to_string(), false)]),
                _ => None,
            })
            .flatten()
            .collect();
        let exit_mentions = |name: &str| {
            let Statement::IfStatement(branch) = &body[split] else { unreachable!() };
            let mut found = false;
            let mut check = |s: &Statement<'_>| {
                struct Find<'n> {
                    name: &'n str,
                    found: bool,
                }
                impl<'b> Visit<'b> for Find<'_> {
                    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'b>) {
                        if identifier.name == self.name {
                            self.found = true;
                        }
                    }
                }
                let mut find = Find { name, found: false };
                find.visit_statement(s);
                found |= find.found;
            };
            if branch.alternate.is_none() {
                body[split + 1..].iter().for_each(&mut check);
            } else if flipped {
                check(&branch.consequent);
            } else {
                check(branch.alternate.as_ref().unwrap());
            }
            found
        };
        let mut hoist = Vec::new();
        for (name, _) in &head_lets {
            if name == "\u{0}" {
                if head_lets.iter().any(|(n, _)| n == "\u{0}") {
                    return None;
                }
            }
            if exit_mentions(name) {
                if !self.own_names.contains(name.as_str()) {
                    return None;
                }
                hoist.push(name.clone());
            }
        }
        Some(hoist)
    }

    /// The rewrite `restructure_loop` plans: the continuing `if` becomes
    /// `if (!t) break;`, and the exit path follows the loop.
    fn move_exit_after_loop(
        &mut self,
        statements: &mut oxc_allocator::Vec<'a, Statement<'a>>,
        at: usize,
        split: usize,
        flipped: bool,
        hoist: &[String],
    ) {
        let taken = statements.remove(at);
        let (label_node, mut loop_) = match taken {
            Statement::LabeledStatement(labeled) => {
                let labeled = labeled.unbox();
                let Statement::ForStatement(loop_) = labeled.body else { unreachable!() };
                (Some(labeled.label), loop_)
            }
            Statement::ForStatement(loop_) => (None, loop_),
            _ => unreachable!(),
        };
        let Statement::BlockStatement(body) = std::mem::replace(&mut loop_.body, self.builder.statement_empty(SPAN)) else {
            unreachable!()
        };
        let mut body = body.unbox().body;
        let mut tail: Vec<Statement<'a>> = body.drain(split + 1..).collect();
        let Some(Statement::IfStatement(branch)) = body.pop() else { unreachable!() };
        let branch = branch.unbox();
        let (test, mut stay, mut exit) = {
            let consequent: Vec<Statement<'a>> = match branch.consequent {
                Statement::BlockStatement(block) => block.unbox().body.into_iter().collect(),
                _ => unreachable!(),
            };
            let alternate: Option<Vec<Statement<'a>>> = branch.alternate.map(|a| match a {
                Statement::BlockStatement(block) => block.unbox().body.into_iter().collect(),
                _ => unreachable!(),
            });
            match (alternate, flipped) {
                (None, _) => (self.negated(branch.test), consequent, std::mem::take(&mut tail)),
                (Some(alternate), false) => (self.negated(branch.test), consequent, alternate),
                (Some(alternate), true) => (branch.test, alternate, consequent),
            }
        };
        // The stay path's trailing `continue` goes; so does the exit's own
        // trailing `break`.
        stay.pop();
        if matches!(exit.last(), Some(Statement::BreakStatement(jump)) if jump.label.is_none() || jump.label.as_ref().map(|l| l.name.as_str()) == label_node.as_ref().map(|l| l.name.as_str()))
        {
            exit.pop();
        }
        let declared = self.hoisted_stores(&mut body, hoist);
        let exit_test = self.builder.statement_break(SPAN, None);
        body.push(self.builder.statement_if(SPAN, test, exit_test, None));
        body.extend(stay);
        loop_.body = self.builder.statement_block(SPAN, body);
        let mut replacement: Vec<Statement<'a>> = Vec::new();
        for name in declared {
            let binding = self.builder.binding_pattern_binding_identifier(SPAN, name);
            let declarator = self.builder.variable_declarator(SPAN, VariableDeclarationKind::Let, binding, oxc_ast::NONE, None, false);
            replacement.push(Statement::VariableDeclaration(self.builder.alloc_variable_declaration(
                SPAN,
                VariableDeclarationKind::Let,
                self.builder.vec1(declarator),
                false,
            )));
        }
        let mut loop_statement = Statement::ForStatement(loop_);
        self.reform_loop(&mut loop_statement);
        let still_labeled = label_node.as_ref().is_some_and(|l| labeled_jumps_to(&loop_statement, l.name.as_str()));
        replacement.push(match label_node {
            Some(label) if still_labeled => self.builder.statement_labeled(SPAN, label, loop_statement),
            _ => loop_statement,
        });
        replacement.extend(exit);
        for (offset, statement) in replacement.into_iter().enumerate() {
            statements.insert(at + offset, statement);
        }
    }

    /// Hoisted lets become stores; returns the names to declare before the
    /// loop.
    fn hoisted_stores(&self, body: &mut [Statement<'a>], hoist: &[String]) -> Vec<oxc_span::Ident<'a>> {
        let mut declared = Vec::new();
        if !hoist.is_empty() {
            for statement in body.iter_mut() {
                let Statement::VariableDeclaration(declaration) = statement else { continue };
                if !declaration.declarations.iter().any(|d| d.id.get_binding_identifier().is_some_and(|b| hoist.contains(&b.name.to_string()))) {
                    continue;
                }
                if declaration.declarations.len() != 1 {
                    // Leave mixed declarations alone: undo is not needed since
                    // nothing changed yet; give up on hoisting this one.
                    continue;
                }
                let declarator = &mut declaration.declarations[0];
                let name = declarator.id.get_binding_identifier().unwrap().name;
                declared.push(name);
                let init = declarator.init.take().unwrap_or_else(|| self.builder.expression_identifier(SPAN, "undefined"));
                let target = self.builder.simple_assignment_target_assignment_target_identifier(SPAN, name);
                let store = self.builder.expression_assignment(SPAN, AssignmentOperator::Assign, AssignmentTarget::from(target), init);
                *statement = self.builder.statement_expression(SPAN, store);
            }
        }
        declared
    }

    /// `!t`, spelled without a double negation.
    pub(super) fn negated(&self, test: Expression<'a>) -> Expression<'a> {
        match test {
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => unary.unbox().argument,
            test => {
                let mut negated = self.builder.expression_unary(SPAN, UnaryOperator::LogicalNot, test);
                normalize(&mut negated, self.builder);
                negated
            }
        }
    }

    pub(super) fn start_lower(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>, at: usize) -> bool {
        let (name, start) = match &statements[at] {
            Statement::VariableDeclaration(declaration) if declaration.declarations.len() == 1 => {
                let declarator = &declaration.declarations[0];
                match (declarator.id.get_binding_identifier(), &declarator.init) {
                    (Some(binding), Some(Expression::NumericLiteral(start)))
                        if start.value >= 1.0 && start.value.fract() == 0.0 =>
                    {
                        (binding.name, start.value)
                    }
                    _ => return false,
                }
            }
            _ => return false,
        };
        if !self.own_names.contains(name.as_str()) {
            return false;
        }
        let stepped = |statement: &mut Statement<'a>, step: bool| {
            indexed_store(statement).is_some_and(|store| {
                let AssignmentTarget::ComputedMemberExpression(member) = &store.left else {
                    return false;
                };
                let index_fits = if step {
                    matches!(&member.expression, Expression::UpdateExpression(update)
                        if update.prefix && update.operator == UpdateOperator::Increment
                            && matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(target) if target.name == name))
                } else {
                    matches!(&member.expression, Expression::Identifier(index) if index.name == name)
                };
                index_fits && !mentions(&member.object, &name) && !mentions(&store.right, &name)
            })
        };
        if !stepped(&mut statements[at + 1], false) || !stepped(&mut statements[at + 2], true) {
            return false;
        }
        if let Statement::VariableDeclaration(declaration) = &mut statements[at] {
            declaration.declarations[0].init =
                Some(self.builder.expression_numeric_literal(SPAN, start - 1.0, None, NumberBase::Decimal));
        }
        let Some(store) = indexed_store(&mut statements[at + 1]) else {
            unreachable!();
        };
        let AssignmentTarget::ComputedMemberExpression(member) = &mut store.left else {
            unreachable!();
        };
        let argument = self.builder.simple_assignment_target_assignment_target_identifier(SPAN, name);
        member.expression = self.builder.expression_update(SPAN, UpdateOperator::Increment, true, argument);
        false
    }

    /// From `statements[at]`: stores indexed `i + 1`, `i + 2`, … in order,
    /// with nothing else mentioning `i`, up to `i = i + n` (n at least the
    /// stores) become `a[++i] = v` each and `i += n - k`.
    pub(super) fn fold_offsets(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>, at: usize) -> bool {
        let counter = {
            let Some(store) = indexed_store(&mut statements[at]) else {
                return false;
            };
            let AssignmentTarget::ComputedMemberExpression(member) = &store.left else {
                return false;
            };
            match offset(&member.expression) {
                Some((name, 1.0)) => name.name,
                _ => return false,
            }
        };
        if !self.own_names.contains(counter.as_str()) {
            return false;
        }
        let mut planned = Vec::new();
        let mut close = None;
        let mut rebase = None;
        for index in at..statements.len() {
            if let Some(n) = step_of(&statements[index], &counter) {
                close = Some((index, n));
                break;
            }
            // `i = f(…, i + n)`: the store reads the stepped counter.
            if !planned.is_empty() {
                if let Statement::ExpressionStatement(expression) = &statements[index] {
                    if let Expression::AssignmentExpression(assignment) = &expression.expression {
                        if assignment.operator == AssignmentOperator::Assign
                            && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(target) if target.name == counter)
                            && offsets_only(&assignment.right, &counter, planned.len() as f64)
                        {
                            rebase = Some(index);
                            break;
                        }
                    }
                }
            }
            let fits = indexed_store(&mut statements[index]).is_some_and(|store| {
                let AssignmentTarget::ComputedMemberExpression(member) = &store.left else {
                    return false;
                };
                matches!(offset(&member.expression), Some((name, k)) if name.name == counter && k == (planned.len() + 1) as f64)
                    && !mentions(&member.object, &counter)
                    && !mentions(&store.right, &counter)
            });
            if fits {
                planned.push(index);
                continue;
            }
            let mentioned = {
                struct Find<'n> {
                    name: &'n str,
                    found: bool,
                }
                impl<'a> Visit<'a> for Find<'_> {
                    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
                        if identifier.name == self.name {
                            self.found = true;
                        }
                    }
                }
                let mut find = Find { name: &counter, found: false };
                find.visit_statement(&statements[index]);
                find.found
            };
            // Only straight-line statements may sit in between.
            if mentioned
                || !matches!(statements[index], Statement::ExpressionStatement(_) | Statement::VariableDeclaration(_))
            {
                return false;
            }
        }
        if let Some(closing) = rebase {
            for index in &planned {
                let Some(store) = indexed_store(&mut statements[*index]) else {
                    unreachable!();
                };
                let AssignmentTarget::ComputedMemberExpression(member) = &mut store.left else {
                    unreachable!();
                };
                let argument = self.builder.simple_assignment_target_assignment_target_identifier(SPAN, counter);
                member.expression = self.builder.expression_update(SPAN, UpdateOperator::Increment, true, argument);
            }
            let Statement::ExpressionStatement(expression) = &mut statements[closing] else {
                unreachable!();
            };
            let Expression::AssignmentExpression(assignment) = &mut expression.expression else {
                unreachable!();
            };
            let mut shift = Shift { name: &counter, by: planned.len() as f64, builder: self.builder };
            shift.visit_expression(&mut assignment.right);
            return true;
        }
        let Some((closing, n)) = close else {
            return false;
        };
        if planned.is_empty() || n < planned.len() as f64 {
            return false;
        }
        for index in &planned {
            let Some(store) = indexed_store(&mut statements[*index]) else {
                unreachable!();
            };
            let AssignmentTarget::ComputedMemberExpression(member) = &mut store.left else {
                unreachable!();
            };
            let argument = self.builder.simple_assignment_target_assignment_target_identifier(SPAN, counter);
            member.expression = self.builder.expression_update(SPAN, UpdateOperator::Increment, true, argument);
        }
        let left = n - planned.len() as f64;
        if left == 0.0 {
            statements.remove(closing);
        } else {
            let target = self.builder.simple_assignment_target_assignment_target_identifier(SPAN, counter);
            let step = if left == 1.0 {
                self.builder.expression_update(SPAN, UpdateOperator::Increment, true, target)
            } else {
                let target = AssignmentTarget::from(target);
                let amount = self.builder.expression_numeric_literal(SPAN, left, None, NumberBase::Decimal);
                self.builder.expression_assignment(SPAN, AssignmentOperator::Addition, target, amount)
            };
            statements[closing] = self.builder.statement_expression(SPAN, step);
        }
        true
    }

    /// A `for` without a test whose body starts with `if (c) break;` tests
    /// `!c` in its header instead (a `while` when it has no init or update).
    pub(super) fn reform_loop(&mut self, statement: &mut Statement<'a>) {
        let Statement::ForStatement(loop_) = statement else {
            return;
        };
        if loop_.test.is_some() {
            return;
        }
        let Statement::BlockStatement(body) = &mut loop_.body else {
            return;
        };
        let exits = matches!(body.body.first(), Some(Statement::IfStatement(branch))
            if branch.alternate.is_none()
                && match &branch.consequent {
                    Statement::BreakStatement(jump) => jump.label.is_none(),
                    Statement::BlockStatement(block) => matches!(block.body.as_slice(), [Statement::BreakStatement(jump)] if jump.label.is_none()),
                    _ => false,
                });
        if !exits {
            return;
        }
        let Statement::IfStatement(branch) = body.body.remove(0) else {
            unreachable!();
        };
        let test = match branch.unbox().test {
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => unary.unbox().argument,
            test => self.builder.expression_unary(SPAN, UnaryOperator::LogicalNot, test),
        };
        if loop_.init.is_none() && loop_.update.is_none() {
            let body = std::mem::replace(&mut loop_.body, self.builder.statement_empty(SPAN));
            *statement = self.builder.statement_while(SPAN, test, body);
        } else {
            loop_.test = Some(test);
        }
    }

    /// Fold a counter's step into the store it indexes, as the source wrote
    /// it: `++i; a[i] = v;` and `a[i + 1] = v; ++i;` are `a[++i] = v;`, and
    /// `let t = i + 1; ++i; … t …` reads `i`. The counter is a local no call
    /// can read, and the array expression and value do not mention it.
    pub(super) fn fold_increments(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>) {
        let mut index = 0;
        while index + 1 < statements.len() {
            // `let t = i + 1; ++i;` then `t` read once, next: read `i`.
            if index + 2 < statements.len() {
                let pending = match (&statements[index], increment_of(&statements[index + 1])) {
                    (Statement::VariableDeclaration(declaration), Some(counter))
                        if declaration.declarations.len() == 1 && self.own_names.contains(counter) =>
                    {
                        let declarator = &declaration.declarations[0];
                        match (&declarator.id, declarator.init.as_ref()) {
                            (BindingPattern::BindingIdentifier(temp), Some(Expression::BinaryExpression(step)))
                                if step.operator == BinaryOperator::Addition
                                    && matches!(&step.left, Expression::Identifier(name) if name.name == counter)
                                    && matches!(&step.right, Expression::NumericLiteral(one) if one.value == 1.0)
                                    && self.read_counts.get(temp.name.as_str()) == Some(&1) =>
                            {
                                Some((temp.name.to_string(), counter.to_string()))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some((temp, _)) = pending {
                    let Some(counter) = increment_ident(&statements[index + 1]) else {
                        unreachable!();
                    };
                    let mut rename = Rename { from: &temp, to: counter, done: false };
                    rename.visit_statement(&mut statements[index + 2]);
                    if rename.done {
                        statements.remove(index);
                        continue;
                    }
                }
            }
            // `++i; a[i] = v;` (evaluating a property path first cannot
            // observe the counter).
            if let Some(counter) = increment_of(&statements[index]).map(str::to_string) {
                let path_object = matches!(&statements[index + 1], Statement::ExpressionStatement(e)
                    if matches!(&e.expression, Expression::AssignmentExpression(a)
                        if matches!(&a.left, AssignmentTarget::ComputedMemberExpression(m) if plain_path(&m.object))));
                if self.own_names.contains(&counter) || path_object {
                    if let Some(store) = indexed_store(&mut statements[index + 1]) {
                        let AssignmentTarget::ComputedMemberExpression(member) = &mut store.left else {
                            unreachable!();
                        };
                        if matches!(&member.expression, Expression::Identifier(name) if name.name == counter.as_str())
                            && !mentions(&member.object, &counter)
                        {
                            let Statement::ExpressionStatement(step) = statements.remove(index) else {
                                unreachable!();
                            };
                            let Some(store) = indexed_store(&mut statements[index]) else {
                                unreachable!();
                            };
                            let AssignmentTarget::ComputedMemberExpression(member) = &mut store.left else {
                                unreachable!();
                            };
                            member.expression = step.unbox().expression;
                            continue;
                        }
                    }
                }
            }
            // `let x = A; let j; if (null == x) { j = true; } else { x = x.p;
            // j = x == null; } j = j ? void 0 : x.q;` is `let j = A?.p?.q;`.
            if index + 3 < statements.len() && self.fold_optional_chain(statements, index) {
                continue;
            }
            if index + 2 < statements.len() && self.fold_repeated_optional_chain(statements, index) {
                continue;
            }
            // `let i = 4; a[i] = v; a[++i] = w;` starts one lower and steps
            // at every store: `let i = 3; a[++i] = v; a[++i] = w;`.
            if index + 2 < statements.len() && self.start_lower(statements, index) {
                continue;
            }
            // `a[i + 1] = v; a[i + 2] = w; …; i = i + n;`: each store steps
            // the counter, and the closing store keeps what is left.
            if self.fold_offsets(statements, index) {
                continue;
            }
            index += 1;
        }
    }

    /// `x = Number(x) ± 1` (or `x = x - 1`) as `++x`/`--x` (`x++`/`x--`).
    pub(super) fn counter_step(&self, expression: &Expression<'a>, prefix: bool) -> Option<Expression<'a>> {
        let Expression::AssignmentExpression(assignment) = expression else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            return None;
        };
        if assignment.operator != AssignmentOperator::Assign {
            return None;
        }
        // `x = Number(x - 1)`: `x - 1` is already a number.
        let right = match assignment.right.without_parentheses() {
            Expression::CallExpression(call)
                if !call.optional
                    && matches!(&call.callee, Expression::Identifier(callee) if callee.name == "Number")
                    && call.arguments.len() == 1
                    && matches!(call.arguments[0].as_expression(), Some(Expression::BinaryExpression(inner)) if inner.operator == BinaryOperator::Subtraction) =>
            {
                call.arguments[0].as_expression().expect("an expression")
            }
            right => right,
        };
        let Expression::BinaryExpression(step) = right.without_parentheses() else {
            return None;
        };
        let operator = match step.operator {
            BinaryOperator::Addition => UpdateOperator::Increment,
            BinaryOperator::Subtraction => UpdateOperator::Decrement,
            _ => return None,
        };
        if !matches!(&step.right, Expression::NumericLiteral(one) if one.value == 1.0) {
            return None;
        }
        let converted = match step.left.without_parentheses() {
            // `x + 1` concatenates for a string; the VM steps numbers here.
            Expression::Identifier(name) => name.name == target.name,
            Expression::CallExpression(call) => {
                !call.optional
                    && matches!(&call.callee, Expression::Identifier(callee) if callee.name == "Number")
                    && call.arguments.len() == 1
                    && matches!(&call.arguments[0], Argument::Identifier(name) if name.name == target.name)
            }
            _ => false,
        };
        if !converted {
            return None;
        }
        let argument = self.builder.simple_assignment_target_assignment_target_identifier(SPAN, target.name);
        Some(self.builder.expression_update(SPAN, operator, prefix, argument))
    }
}

/// The continuing `if` of a `for (;;)` body: the last top-level `if` whose
/// arm ends in a jump back to this loop while the other path leaves. Returns
/// its index and whether its else arm is the one that stays.
fn continuing_split(body: &[Statement<'_>], label: Option<&str>) -> Option<(usize, bool)> {
    let back = |statements: &[Statement<'_>]| matches!(statements.last(), Some(Statement::ContinueStatement(jump))
        if jump.label.as_ref().map(|l| l.name.as_str()) == label || jump.label.is_none());
    let leaves = |statements: &[Statement<'_>]| matches!(statements.last(),
        Some(Statement::ReturnStatement(_) | Statement::ThrowStatement(_)))
        || matches!(statements.last(), Some(Statement::BreakStatement(jump)) if jump.label.is_none() || jump.label.as_ref().map(|l| l.name.as_str()) == label)
        || matches!(statements.last(), Some(Statement::ContinueStatement(jump)) if jump.label.is_some() && jump.label.as_ref().map(|l| l.name.as_str()) != label);
    let mut plan = None;
    for (index, statement) in body.iter().enumerate() {
        let Statement::IfStatement(branch) = statement else {
            continue;
        };
        let Some(consequent) = block_of(&branch.consequent) else {
            continue;
        };
        let rest = &body[index + 1..];
        match &branch.alternate {
            None if back(consequent) && leaves(rest) => plan = Some((index, false)),
            Some(alternate) if rest.is_empty() => {
                let Some(alternate) = block_of(alternate) else { continue };
                if back(consequent) && leaves(alternate) {
                    plan = Some((index, false));
                } else if back(alternate) && leaves(consequent) {
                    plan = Some((index, true));
                }
            }
            _ => {}
        }
    }
    plan
}

fn block_of<'s, 'b>(statement: &'s Statement<'b>) -> Option<&'s [Statement<'b>]> {
    match statement {
        Statement::BlockStatement(block) => Some(block.body.as_slice()),
        _ => None,
    }
}

/// Whether the exit path jumps to the loop other than by its own trailing
/// `break`.
fn exit_jumps_back(body: &[Statement<'_>], split: usize, flipped: bool, label: Option<&str>) -> bool {
    let Statement::IfStatement(branch) = &body[split] else { unreachable!() };
    let exit: Vec<&Statement<'_>> = if branch.alternate.is_none() {
        body[split + 1..].iter().collect()
    } else if flipped {
        block_of(&branch.consequent).unwrap().iter().collect()
    } else {
        block_of(branch.alternate.as_ref().unwrap()).unwrap().iter().collect()
    };
    let (last, rest) = exit.split_last().expect("an exit");
    let trailing_own = matches!(last, Statement::BreakStatement(jump) if jump.label.is_none() || jump.label.as_ref().map(|l| l.name.as_str()) == label);
    rest.iter().any(|s| jumps_to(s, label))
        || (!trailing_own && jumps_to(last, label))
}

/// `++name;`
pub(super) fn increment_of<'s>(statement: &'s Statement<'_>) -> Option<&'s str> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    let Expression::UpdateExpression(update) = &expression.expression else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(target) = &update.argument else {
        return None;
    };
    (update.prefix && update.operator == UpdateOperator::Increment).then_some(target.name.as_str())
}

/// A `break` or `continue` in `statement` that reaches the loop `label`
/// (an unlabeled one outside any inner loop or switch reaches the
/// enclosing loop).
pub(super) fn jumps_to(statement: &Statement<'_>, label: Option<&str>) -> bool {
    jumps(statement, label, false)
}

/// A `break` in `statement` that leaves the loop `label` names (or the
/// innermost loop, unlabeled): a `continue` stays in it.
pub(super) fn breaks_from(statement: &Statement<'_>, label: Option<&str>) -> bool {
    jumps(statement, label, true)
}

pub(super) fn jumps(statement: &Statement<'_>, label: Option<&str>, breaks_only: bool) -> bool {
    struct Jumps<'l> {
        label: Option<&'l str>,
        breaks_only: bool,
        loops: usize,
        switches: usize,
        found: bool,
    }
    impl<'b> Visit<'b> for Jumps<'_> {
        fn visit_break_statement(&mut self, jump: &BreakStatement<'b>) {
            match &jump.label {
                Some(name) => self.found |= Some(name.name.as_str()) == self.label,
                None => self.found |= self.loops == 0 && self.switches == 0,
            }
        }
        fn visit_continue_statement(&mut self, jump: &ContinueStatement<'b>) {
            if self.breaks_only {
                return;
            }
            match &jump.label {
                Some(name) => self.found |= Some(name.name.as_str()) == self.label,
                None => self.found |= self.loops == 0,
            }
        }
        fn visit_for_statement(&mut self, it: &ForStatement<'b>) {
            self.loops += 1;
            walk::walk_for_statement(self, it);
            self.loops -= 1;
        }
        fn visit_while_statement(&mut self, it: &WhileStatement<'b>) {
            self.loops += 1;
            walk::walk_while_statement(self, it);
            self.loops -= 1;
        }
        fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'b>) {
            self.loops += 1;
            walk::walk_do_while_statement(self, it);
            self.loops -= 1;
        }
        fn visit_for_in_statement(&mut self, it: &ForInStatement<'b>) {
            self.loops += 1;
            walk::walk_for_in_statement(self, it);
            self.loops -= 1;
        }
        fn visit_for_of_statement(&mut self, it: &ForOfStatement<'b>) {
            self.loops += 1;
            walk::walk_for_of_statement(self, it);
            self.loops -= 1;
        }
        fn visit_switch_statement(&mut self, it: &SwitchStatement<'b>) {
            self.switches += 1;
            walk::walk_switch_statement(self, it);
            self.switches -= 1;
        }
        fn visit_function(&mut self, _: &Function<'b>, _: oxc_syntax::scope::ScopeFlags) {}
        fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'b>) {}
    }
    let mut jumps = Jumps { label, breaks_only, loops: 0, switches: 0, found: false };
    // The loop statement itself: look inside its body.
    match statement {
        Statement::ForStatement(loop_) => jumps.visit_statement(&loop_.body),
        statement => jumps.visit_statement(statement),
    }
    jumps.found
}

/// A name, `arguments`, or property reads off one with literal keys.
pub(super) fn plain_path(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::Identifier(_) => true,
        Expression::StaticMemberExpression(member) => !member.optional && plain_path(&member.object),
        Expression::ComputedMemberExpression(member) => {
            !member.optional
                && matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_))
                && plain_path(&member.object)
        }
        _ => false,
    }
}

/// A labeled `break`/`continue` naming `label` anywhere in `statement`.
pub(super) fn labeled_jumps_to(statement: &Statement<'_>, label: &str) -> bool {
    struct Named<'l>(&'l str, bool);
    impl<'b> Visit<'b> for Named<'_> {
        fn visit_break_statement(&mut self, jump: &BreakStatement<'b>) {
            self.1 |= jump.label.as_ref().is_some_and(|l| l.name == self.0);
        }
        fn visit_continue_statement(&mut self, jump: &ContinueStatement<'b>) {
            self.1 |= jump.label.as_ref().is_some_and(|l| l.name == self.0);
        }
    }
    let mut named = Named(label, false);
    named.visit_statement(statement);
    named.1
}

/// `name + 1 + 1 …` (or `name + n`): the counter and its offset.
pub(super) fn offset<'s, 'a>(expression: &'s Expression<'a>) -> Option<(&'s IdentifierReference<'a>, f64)> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    if binary.operator != BinaryOperator::Addition {
        return None;
    }
    let Expression::NumericLiteral(step) = &binary.right else {
        return None;
    };
    if step.value.fract() != 0.0 || step.value < 1.0 {
        return None;
    }
    match binary.left.without_parentheses() {
        Expression::Identifier(name) => Some((name, step.value)),
        left => offset(left).map(|(name, inner)| (name, inner + step.value)),
    }
}

/// How far a statement steps `name`: `++name;` or `name = name + n;`.
pub(super) fn step_of(statement: &Statement<'_>, name: &str) -> Option<f64> {
    if increment_of(statement) == Some(name) {
        return Some(1.0);
    }
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &expression.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
        return None;
    };
    if target.name != name {
        return None;
    }
    match (assignment.operator, &assignment.right) {
        (AssignmentOperator::Addition, Expression::NumericLiteral(step)) if step.value.fract() == 0.0 && step.value >= 1.0 => {
            Some(step.value)
        }
        (AssignmentOperator::Assign, right) => offset(right).filter(|(counter, _)| counter.name == name).map(|(_, n)| n),
        _ => None,
    }
}

/// Every read of `name` in `expression` is `name + n` with `n` at least
/// `least`.
pub(super) fn offsets_only(expression: &Expression<'_>, name: &str, least: f64) -> bool {
    struct Count<'n> {
        name: &'n str,
        least: f64,
        reads: usize,
        offsets: usize,
    }
    impl<'a> Visit<'a> for Count<'_> {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            if identifier.name == self.name {
                self.reads += 1;
            }
        }
        fn visit_binary_expression(&mut self, binary: &BinaryExpression<'a>) {
            if binary.operator == BinaryOperator::Addition
                && matches!(&binary.left, Expression::Identifier(left) if left.name == self.name)
                && matches!(&binary.right, Expression::NumericLiteral(n) if n.value >= self.least && n.value.fract() == 0.0)
            {
                self.offsets += 1;
            }
            walk::walk_binary_expression(self, binary);
        }
    }
    let mut count = Count { name, least, reads: 0, offsets: 0 };
    count.visit_expression(expression);
    count.reads > 0 && count.reads == count.offsets
}

/// `name + n` as `name + (n - by)` (just `name` when equal).
pub(super) struct Shift<'n, 'a> {
    name: &'n str,
    by: f64,
    builder: AstBuilder<'a>,
}

impl<'a> VisitMut<'a> for Shift<'_, 'a> {
    fn visit_expression(&mut self, expression: &mut Expression<'a>) {
        if let Expression::BinaryExpression(binary) = expression {
            if binary.operator == BinaryOperator::Addition
                && matches!(&binary.left, Expression::Identifier(left) if left.name == self.name)
            {
                if let Expression::NumericLiteral(n) = &mut binary.right {
                    let left = n.value - self.by;
                    if left == 0.0 {
                        let name = std::mem::replace(&mut binary.left, self.builder.expression_null_literal(SPAN));
                        *expression = name;
                    } else {
                        n.value = left;
                        n.raw = None;
                    }
                    return;
                }
            }
        }
        walk_mut::walk_expression(self, expression);
    }
}

/// Renames the one read of a temporary to the counter it equals.
pub(super) struct Rename<'n, 'a> {
    from: &'n str,
    to: oxc_span::Ident<'a>,
    done: bool,
}

impl<'a> VisitMut<'a> for Rename<'_, 'a> {
    fn visit_identifier_reference(&mut self, identifier: &mut IdentifierReference<'a>) {
        if identifier.name == self.from && !self.done {
            identifier.name = self.to;
            self.done = true;
        }
    }
}

/// The counter of `++name;`, in the arena.
pub(super) fn increment_ident<'a>(statement: &Statement<'a>) -> Option<oxc_span::Ident<'a>> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    let Expression::UpdateExpression(update) = &expression.expression else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(target) = &update.argument else {
        return None;
    };
    Some(target.name)
}

/// `x[k] = v;` with its parts.
pub(super) fn indexed_store<'s, 'a>(statement: &'s mut Statement<'a>) -> Option<&'s mut AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &mut expression.expression else {
        return None;
    };
    (assignment.operator == AssignmentOperator::Assign
        && matches!(assignment.left, AssignmentTarget::ComputedMemberExpression(_)))
    .then_some(assignment)
}
