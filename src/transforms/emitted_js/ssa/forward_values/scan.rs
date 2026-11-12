//! The scan: candidates, stores, effects and reads, per owner.

use super::*;

impl<'s> Scan<'s> {
    pub(super) fn scoping(&self) -> &'s Scoping {
        self.scoping.expect("scan has scoping")
    }

    pub(super) fn owner(&self) -> u32 {
        *self.owners.last().expect("owner stack")
    }

    pub(super) fn effect(&mut self, span: Span) {
        let owner = self.owner();
        self.effects.push((span.start, span.end, owner));
        self.mutations.push((span.start, span.end, owner));
    }

    /// An effect that can run other code (see `runs`).
    pub(super) fn runs(&mut self, span: Span) {
        self.effect(span);
        let owner = self.owner();
        self.runs.push((span.start, span.end, owner));
    }

    /// The name a member chain starts from.
    pub(super) fn base_symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        match expression {
            Expression::Identifier(identifier) => {
                self.scoping().get_reference(identifier.reference_id()).symbol_id()
            }
            Expression::StaticMemberExpression(member) => self.base_symbol(&member.object),
            Expression::ComputedMemberExpression(member) => self.base_symbol(&member.object),
            _ => None,
        }
    }

    /// An effect that changes no property: a property read, or a store to
    /// a plain name.
    pub(super) fn read_effect(&mut self, span: Span) {
        let owner = self.owner();
        self.effects.push((span.start, span.end, owner));
    }

    pub(super) fn list(&mut self, statements: &[oxc_ast::ast::Statement<'_>]) {
        let list = self.lists.len();
        let owner = self.owner();
        let mut entries = Vec::with_capacity(statements.len());
        for (position, statement) in statements.iter().enumerate() {
            // A discarded fixed `arguments[n]` read is also a no-op.
            let noop = matches!(statement, oxc_ast::ast::Statement::ExpressionStatement(expression)
                if self.types.is_some_and(|types| types.pure(&expression.expression))
                    || matches!(&expression.expression, Expression::ComputedMemberExpression(member) if self.is_fixed_argument(member)))
                && !self.has_comment(statement.span());
            if noop {
                self.noops.push(statement.span());
            }
            entries.push(Statement {
                span: statement.span(),
                owner,
                writes: if noop { Some(Vec::new()) } else { self.inert_writes(statement) },
            });
            if let Some(candidate) = self.candidate(statement, list, position) {
                self.candidates.push(candidate);
            }
            let aliases = self.tuple_aliases(statement, list, position);
            self.candidates.extend(aliases);
            let slots = self.passthrough_slots(statement, list, position);
            self.candidates.extend(slots);
            if let Some(store) = self.slot_store(statement, list, position) {
                self.stores.push(store);
            }
        }
        self.lists.push(entries);
    }

    pub(super) fn slot_store(&self, statement: &oxc_ast::ast::Statement<'_>, list: usize, position: usize) -> Option<SlotStore> {
        let (target, value) = match statement {
            oxc_ast::ast::Statement::ExpressionStatement(expression) => {
                let Expression::AssignmentExpression(assignment) = &expression.expression else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                    return None;
                };
                if assignment.operator != AssignmentOperator::Assign {
                    return None;
                }
                (analysis::identifier_symbol(target, self.scoping())?, &assignment.right)
            }
            oxc_ast::ast::Statement::VariableDeclaration(declaration)
                if declaration.kind == VariableDeclarationKind::Var && declaration.declarations.len() == 1 =>
            {
                let declarator = &declaration.declarations[0];
                let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                    return None;
                };
                (binding.symbol_id.get()?, declarator.init.as_ref()?)
            }
            _ => return None,
        };
        let name = self.scoping().symbol_name(target);
        if !(name.starts_with("$KEY_") || name.starts_with("$CARRY_")) || self.has_comment(statement.span()) {
            return None;
        }
        let (kind, value_reference) = self.classify(value);
        (kind != InitKind::Computed).then(|| SlotStore {
            list,
            position,
            statement: statement.span(),
            slot: target,
            value: value.span(),
            kind,
            member: value.is_member_expression(),
            value_reference,
        })
    }

    pub(super) fn candidate(
        &self,
        statement: &oxc_ast::ast::Statement<'_>,
        list: usize,
        position: usize,
    ) -> Option<Candidate> {
        let oxc_ast::ast::Statement::VariableDeclaration(declaration) = statement else {
            return None;
        };
        if declaration.declarations.len() != 1 {
            return None;
        }
        let declarator = &declaration.declarations[0];
        let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
            return None;
        };
        // A `var` candidate is an internal slot; Plan checks it is one.
        let generated = match declaration.kind {
            VariableDeclarationKind::Let | VariableDeclarationKind::Const => {
                binding.name.starts_with("$SSA_")
                    || binding.name.starts_with("$CARRY_")
                    || binding.name.starts_with("$MERGE_")
                    // A slot made a local `let` is a value like any other.
                    || binding.name.starts_with("$KEY_")
            }
            VariableDeclarationKind::Var => binding.name.starts_with("$KEY_"),
            _ => false,
        };
        if !generated {
            return None;
        }
        let init = declarator.init.as_ref()?;
        let symbol = binding.symbol_id.get()?;
        if self.has_comment(statement.span()) {
            return None;
        }
        let (kind, init_reference) = self.classify(init);
        Some(Candidate {
            list,
            position,
            statement: statement.span(),
            symbol,
            init: init.span(),
            kind,
            member: init.is_member_expression(),
            sequence: matches!(init, Expression::SequenceExpression(_)),
            init_reference,
            alias_of: None,
            slot: None,
            helper: None,
            pure: self.types.is_some_and(|types| types.pure(init)) && !primitives::allocates(init),
            fresh: self.types.is_some_and(|types| types.pure(init)) && primitives::allocates(init),
            property_read: self.property_reads && self.types.is_some_and(|types| property_read(init, types)),
            named: keeps_name(init),
        })
    }

    /// A parsed comment inside `span` (a `//` in a string is not one).
    pub(super) fn has_comment(&self, span: Span) -> bool {
        let at = self.comments.partition_point(|start| *start < span.start);
        self.comments.get(at).is_some_and(|start| *start < span.end)
    }

    pub(super) fn classify(&self, init: &Expression<'_>) -> (InitKind, Option<ReferenceId>) {
        match init {
            Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => (InitKind::Word, None),
            Expression::NumericLiteral(_) => (InitKind::Number, None),
            Expression::UnaryExpression(unary)
                if matches!(
                    unary.operator,
                    UnaryOperator::UnaryNegation | UnaryOperator::Void
                ) && matches!(unary.argument, Expression::NumericLiteral(_)) =>
            {
                (InitKind::UnaryLiteral, None)
            }
            Expression::Identifier(identifier)
                if identifier.reference_id.get().is_some_and(|reference| {
                    self.scoping().get_reference(reference).symbol_id().is_none()
                }) && is_constant_global(&identifier.name) =>
            {
                (InitKind::Word, None)
            }
            Expression::Identifier(identifier) => {
                (InitKind::Identifier, identifier.reference_id.get())
            }
            _ if init
                .as_member_expression()
                .is_some_and(|member| primitives::is_builtin_member(member, self.scoping())) =>
            {
                (InitKind::Word, None)
            }
            Expression::ComputedMemberExpression(member) if self.is_tuple_slot(member) => {
                (InitKind::TupleSlot, None)
            }
            _ => (InitKind::Computed, None),
        }
    }

    /// Slots of `let [a, b] = H(x, y)` that `H` fills with a parameter: each
    /// is a candidate whose value is that argument.
    pub(super) fn passthrough_slots(
        &mut self,
        statement: &oxc_ast::ast::Statement<'_>,
        list: usize,
        position: usize,
    ) -> Vec<Candidate> {
        let oxc_ast::ast::Statement::VariableDeclaration(declaration) = statement else {
            return Vec::new();
        };
        if declaration.declarations.len() != 1 {
            return Vec::new();
        }
        let declarator = &declaration.declarations[0];
        let (BindingPattern::ArrayPattern(pattern), Some(Expression::CallExpression(call))) =
            (&declarator.id, declarator.init.as_ref())
        else {
            return Vec::new();
        };
        let Expression::Identifier(callee) = &call.callee else {
            return Vec::new();
        };
        let Some(helper_symbol) = analysis::identifier_symbol(callee, self.scoping()) else {
            return Vec::new();
        };
        let Some(helper) = self.passthrough.get(&helper_symbol) else {
            return Vec::new();
        };
        if pattern.rest.is_some()
            || call.optional
            || call.arguments.iter().any(|argument| argument.as_expression().is_none())
        {
            return Vec::new();
        }
        if self.has_comment(statement.span()) {
            return Vec::new();
        }
        let mut candidates = Vec::new();
        let mut named = 0;
        for (index, element) in pattern.elements.iter().enumerate() {
            let Some(element) = element else {
                continue;
            };
            let BindingPattern::BindingIdentifier(id) = element else {
                return Vec::new();
            };
            named += 1;
            let Some(Some(parameter)) = helper.slots.get(index) else {
                continue;
            };
            let Some(argument) = call.arguments.get(*parameter).and_then(|a| a.as_expression())
            else {
                continue;
            };
            let (kind, init_reference) = self.classify(argument);
            if kind == InitKind::Computed
                || !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
            {
                continue;
            }
            let Some(symbol) = id.symbol_id.get() else {
                continue;
            };
            candidates.push(Candidate {
                list,
                position,
                statement: statement.span(),
                symbol,
                init: argument.span(),
                kind,
                member: argument.is_member_expression(),
                sequence: false,
                init_reference,
                alias_of: None,
                slot: Some(id.span),
                helper: Some(helper_symbol),
                pure: false,
                fresh: false,
                property_read: false,
                named: false,
            });
        }
        // A pure shuffle of stable arguments: losing every slot loses the call.
        let removable = helper.shuffle
            && call
                .arguments
                .iter()
                .all(|argument| argument.as_expression().is_some_and(|e| self.classify(e).0 != InitKind::Computed));
        self.groups.insert(statement.span().start, (named, removable));
        candidates
    }

    /// The later named slots of `let [a, b] = H(…)` for a duplicate helper.
    pub(super) fn tuple_aliases(
        &self,
        statement: &oxc_ast::ast::Statement<'_>,
        list: usize,
        position: usize,
    ) -> Vec<Candidate> {
        let oxc_ast::ast::Statement::VariableDeclaration(declaration) = statement else {
            return Vec::new();
        };
        if declaration.declarations.len() != 1 {
            return Vec::new();
        }
        let declarator = &declaration.declarations[0];
        let (BindingPattern::ArrayPattern(pattern), Some(Expression::CallExpression(call))) =
            (&declarator.id, declarator.init.as_ref())
        else {
            return Vec::new();
        };
        let Expression::Identifier(callee) = &call.callee else {
            return Vec::new();
        };
        if pattern.rest.is_some()
            || !analysis::identifier_symbol(callee, self.scoping())
                .is_some_and(|helper| self.duplicate_helpers.contains(&helper))
        {
            return Vec::new();
        }
        let mut names = Vec::new();
        for element in &pattern.elements {
            match element {
                None => {}
                Some(BindingPattern::BindingIdentifier(id)) => names.push(id),
                Some(_) => return Vec::new(),
            }
        }
        let Some((first, rest)) = names.split_first() else {
            return Vec::new();
        };
        let Some(representative) = first.symbol_id.get() else {
            return Vec::new();
        };
        rest.iter()
            .filter(|id| id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
            .filter_map(|id| {
                Some(Candidate {
                    list,
                    position,
                    statement: statement.span(),
                    symbol: id.symbol_id.get()?,
                    init: id.span,
                    kind: InitKind::Identifier,
                    member: false,
                    sequence: false,
                    init_reference: None,
                    alias_of: Some(representative),
                    slot: Some(id.span),
                    helper: None,
                    pure: false,
                    fresh: false,
                    property_read: false,
                        named: false,
                })
            })
            .collect()
    }

    /// `arguments[n]` in a function whose arguments cannot change.
    pub(super) fn is_fixed_argument(&self, member: &ComputedMemberExpression<'_>) -> bool {
        matches!((&member.object, &member.expression), (Expression::Identifier(object), Expression::NumericLiteral(_))
            if object.name == "arguments"
                && object.reference_id.get().is_some_and(|reference| {
                    self.scoping().get_reference(reference).symbol_id().is_none()
                }))
            && !member.optional
            && self.fixed_arguments.contains(&self.owner())
    }

    /// `x[k]` of a read-only fresh tuple: no getter, no change.
    pub(super) fn is_tuple_slot(&self, member: &ComputedMemberExpression<'_>) -> bool {
        if self.is_fixed_argument(member) {
            return true;
        }
        let (Expression::Identifier(object), Expression::NumericLiteral(index)) =
            (&member.object, &member.expression)
        else {
            return false;
        };
        !member.optional
            && analysis::identifier_symbol(object, self.scoping())
                .and_then(|symbol| self.tuple_slots.get(&symbol))
                .is_some_and(|length| index.value.fract() == 0.0 && index.value >= 0.0 && index.value < *length as f64)
    }

    /// The symbols a statement writes when all it does is store literals or
    /// identifiers into plain locals.
    pub(super) fn inert_writes(&self, statement: &oxc_ast::ast::Statement<'_>) -> Option<Vec<SymbolId>> {
        let scoping = self.scoping();
        let simple = |expression: &Expression<'_>| {
            self.types.is_some_and(|types| types.pure(expression))
                || matches!(expression, Expression::ComputedMemberExpression(member) if self.is_fixed_argument(member))
                || matches!(
                expression,
                Expression::StringLiteral(_)
                    | Expression::BooleanLiteral(_)
                    | Expression::NullLiteral(_)
                    | Expression::NumericLiteral(_)
                    | Expression::Identifier(_)
            ) || matches!(expression, Expression::UnaryExpression(unary)
                if matches!(unary.operator, UnaryOperator::UnaryNegation | UnaryOperator::Void)
                    && matches!(unary.argument, Expression::NumericLiteral(_)))
                || property_path(expression)
        };
        match statement {
            oxc_ast::ast::Statement::VariableDeclaration(declaration) => {
                let mut writes = Vec::new();
                for declarator in &declaration.declarations {
                    let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                        return None;
                    };
                    if !declarator.init.as_ref().is_none_or(simple) {
                        return None;
                    }
                    writes.push(binding.symbol_id.get()?);
                }
                Some(writes)
            }
            oxc_ast::ast::Statement::ExpressionStatement(expression) => {
                let Expression::AssignmentExpression(assignment) = &expression.expression else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                    return None;
                };
                if assignment.operator != AssignmentOperator::Assign || !simple(&assignment.right)
                {
                    return None;
                }
                Some(vec![analysis::identifier_symbol(target, scoping)?])
            }
            oxc_ast::ast::Statement::FunctionDeclaration(_)
            | oxc_ast::ast::Statement::EmptyStatement(_) => Some(Vec::new()),
            _ => None,
        }
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_program(&mut self, program: &Program<'a>) {
        self.list(&program.body);
        walk::walk_program(self, program);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.owners.push(function.span.start);
        if let Some(body) = &function.body {
            self.list(&body.statements);
        }
        walk::walk_function(self, function, flags);
        self.owners.pop();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.owners.push(arrow.span.start);
        // An expression body's one statement is the returned value, not a
        // statement to drop.
        if !arrow.expression {
            self.list(&arrow.body.statements);
        }
        walk::walk_arrow_function_expression(self, arrow);
        self.owners.pop();
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.list(&block.body);
        walk::walk_block_statement(self, block);
    }

    fn visit_switch_case(&mut self, case: &SwitchCase<'a>) {
        self.list(&case.consequent);
        walk::walk_switch_case(self, case);
    }

    fn visit_static_block(&mut self, block: &StaticBlock<'a>) {
        self.list(&block.body);
        walk::walk_static_block(self, block);
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_if_statement(&mut self, branch: &IfStatement<'a>) {
        if let Some((reference, negated)) = tested_name(&branch.test) {
            let owner = self.owner();
            self.branches.push((
                reference,
                negated,
                branch.consequent.span(),
                branch.alternate.as_ref().map(GetSpan::span),
                owner,
            ));
        }
        walk::walk_if_statement(self, branch);
    }

    fn visit_conditional_expression(&mut self, conditional: &ConditionalExpression<'a>) {
        if let Some((reference, negated)) = tested_name(&conditional.test) {
            let owner = self.owner();
            self.branches.push((
                reference,
                negated,
                conditional.consequent.span(),
                Some(conditional.alternate.span()),
                owner,
            ));
        }
        walk::walk_conditional_expression(self, conditional);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic = true;
        }
        if identifier.name == "arguments" {
            self.arguments.push(identifier.span.start);
        }
        if let Some(reference) = identifier.reference_id.get() {
            let owner = self.owner();
            self.idents.push((identifier.span.start, reference, owner));
        }
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        // `Number(x)` / `String(x)` / `Boolean(x)` of a primitive of known
        // type converts without running code.
        let converts = !call.optional
            && call.arguments.len() == 1
            && matches!(&call.callee, Expression::Identifier(callee)
                if matches!(callee.name.as_str(), "Number" | "String" | "Boolean")
                    && self.scoping().get_reference(callee.reference_id()).symbol_id().is_none())
            && call.arguments[0]
                .as_expression()
                .is_some_and(|argument| self.types.is_some_and(|types| types.type_of(argument).is_some()));
        if converts {
            self.read_effect(call.span);
        } else {
            self.runs(call.span);
            self.calls.insert((call.span.start, call.span.end));
        }
        // `f.call(…)` / `f["call"](…)`: the method read itself runs nothing;
        // the call does.
        let method_object = match &call.callee {
            Expression::StaticMemberExpression(member)
                if !member.optional && matches!(member.property.name.as_str(), "call" | "apply") =>
            {
                Some(&member.object)
            }
            Expression::ComputedMemberExpression(member)
                if !member.optional
                    && matches!(&member.expression, Expression::StringLiteral(key)
                        if matches!(key.value.as_str(), "call" | "apply")) =>
            {
                Some(&member.object)
            }
            _ => None,
        };
        if let Some(object) = method_object.filter(|object| matches!(object, Expression::Identifier(_))) {
            self.visit_expression(object);
            for argument in &call.arguments {
                self.visit_argument(argument);
            }
            return;
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_new_expression(&mut self, call: &NewExpression<'a>) {
        self.runs(call.span);
        self.calls.insert((call.span.start, call.span.end));
        walk::walk_new_expression(self, call);
    }

    fn visit_tagged_template_expression(&mut self, tagged: &TaggedTemplateExpression<'a>) {
        self.runs(tagged.span);
        walk::walk_tagged_template_expression(self, tagged);
    }

    fn visit_import_expression(&mut self, import: &ImportExpression<'a>) {
        self.runs(import.span);
        walk::walk_import_expression(self, import);
    }

    fn visit_member_expression(&mut self, member: &MemberExpression<'a>) {
        // A fixed `arguments[n]` read is neither an effect nor a changing name.
        if matches!(member, MemberExpression::ComputedMemberExpression(computed) if self.is_fixed_argument(computed)) {
            return;
        }
        // A built-in's named property reads a fixed value.
        if primitives::is_builtin_member(member, self.scoping()) {
            return;
        }
        if !matches!(member, MemberExpression::ComputedMemberExpression(computed) if self.is_tuple_slot(computed))
        {
            self.read_effect(member.span());
        }
        walk::walk_member_expression(self, member);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if matches!(assignment.left, AssignmentTarget::AssignmentTargetIdentifier(_)) {
            self.read_effect(assignment.span);
        } else {
            self.effect(assignment.span);
            if let Some(member) = assignment.left.as_member_expression() {
                let base = self.base_symbol(member.object());
                self.member_stores.insert((assignment.span.start, assignment.span.end), base);
            }
        }
        // `o[k] = v` evaluates `o` and `k`, then `v`, then stores: the target
        // performs no read before the value, unlike `o[k] += v`.
        if assignment.operator == AssignmentOperator::Assign {
            if let Some(member) = assignment.left.as_member_expression() {
                match member {
                    MemberExpression::ComputedMemberExpression(computed) => {
                        self.visit_expression(&computed.object);
                        self.visit_expression(&computed.expression);
                    }
                    MemberExpression::StaticMemberExpression(member) => {
                        self.visit_expression(&member.object);
                    }
                    MemberExpression::PrivateFieldExpression(member) => {
                        self.visit_expression(&member.object);
                    }
                }
                self.visit_expression(&assignment.right);
                return;
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if matches!(update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(_)) {
            self.read_effect(update.span);
        } else {
            self.effect(update.span);
        }
        walk::walk_update_expression(self, update);
    }

    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        // `-x`, `+x`, `~x` may call `valueOf`; `delete` writes. Over a
        // primitive of known type they run nothing.
        if !matches!(
            unary.operator,
            UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
        ) && !self.types.is_some_and(|types| types.unary_inert(unary))
        {
            self.effect(unary.span);
        }
        walk::walk_unary_expression(self, unary);
    }

    fn visit_binary_expression(&mut self, binary: &BinaryExpression<'a>) {
        // Only strict equality never runs user code, unless both operands
        // are primitives of known type.
        if binary.operator == BinaryOperator::In {
            // `k in o` asks `o` without running code: no proxies, and keys
            // that convert without code, as for property reads.
            self.read_effect(binary.span);
        } else if !matches!(
            binary.operator,
            BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
        ) && !self.types.is_some_and(|types| types.operator_inert(binary))
        {
            self.effect(binary.span);
            if let Some(types) = self.types {
                let left = types.type_of(&binary.left).is_some();
                let right = types.type_of(&binary.right).is_some();
                if left != right {
                    let untyped = if left { binary.right.span() } else { binary.left.span() };
                    self.conversions.insert((binary.span.start, binary.span.end), untyped);
                }
            }
        }
        walk::walk_binary_expression(self, binary);
    }

    fn visit_template_literal(&mut self, template: &TemplateLiteral<'a>) {
        if !template.expressions.is_empty() {
            self.effect(template.span);
        }
        walk::walk_template_literal(self, template);
    }

    fn visit_spread_element(&mut self, spread: &SpreadElement<'a>) {
        self.runs(spread.span);
        walk::walk_spread_element(self, spread);
    }

    fn visit_await_expression(&mut self, expression: &AwaitExpression<'a>) {
        self.runs(expression.span);
        let owner = self.owner();
        self.suspends.push((expression.span.start, expression.span.end, owner));
        walk::walk_await_expression(self, expression);
    }

    fn visit_yield_expression(&mut self, expression: &YieldExpression<'a>) {
        self.runs(expression.span);
        let owner = self.owner();
        self.suspends.push((expression.span.start, expression.span.end, owner));
        walk::walk_yield_expression(self, expression);
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        self.runs(class.span);
        walk::walk_class(self, class);
    }
}
