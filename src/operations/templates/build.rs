use super::extract::{Proof, Receiver};
use super::*;

pub(super) struct RecipeBuilder<'a> {
    source: &'a Arc<str>,
    scoping: &'a Scoping,
    proof: &'a Proof,
    params: Vec<SymbolId>,
    locals: BTreeMap<SymbolId, u32>,
    receivers: BTreeMap<SymbolId, Receiver>,
    invalid_context: bool,
    next_temp: u32,
    graph: GraphBuilder,
    /// The handler function's span. A referenced binding declared outside it
    /// is a captured/global name; one declared inside it is a local.
    handler_span: Span,
    /// Locals bound to `context.callFrameStack.pop()`. They are only readable
    /// as `frame.returnByteOffset` / `frame.returnDispatchState`.
    frame_symbols: BTreeSet<SymbolId>,
    /// Locals bound to `context.tryFrameStack.pop()`.
    try_frame_symbols: BTreeSet<SymbolId>,
    /// Locals bound to `context.errorStack.pop()`.
    error_symbols: BTreeSet<SymbolId>,
}

/// The VM exception stacks: active `try` frames, error records of the
/// `catch`/`finally` bodies being run, and pending exceptions.
const TRY_FRAME_STACK: &str = "tryFrameStack";
const ERROR_STACK: &str = "errorStack";
const THROWN_VALUE_STACK: &str = "thrownValueStack";

/// VM context field names used by the control-flow shapes below. They are
/// the pipeline's canonical names and are also checked against the proven
/// `vmContext` property mappings.
pub(crate) const BYTE_OFFSET: &str = "byteOffset";
pub(crate) const DISPATCH_STATE: &str = "dispatchState";
pub(crate) const CALL_FRAME_STACK: &str = "callFrameStack";
/// The pipeline's canonical name and role for the VM closure constructor.
pub(super) const CREATE_THREAD: &str = "vmCreateThread";
/// The register `vmRun` returns from once it is set.
pub(super) const COMPLETION_REGISTER: &str = "vmPendingResult";

/// `<context>.callFrameStack.push({ returnByteOffset: <context>.byteOffset,
/// returnDispatchState: <context>.dispatchState })`, where `is_context` decides
/// which identifier is the VM context.
pub(crate) fn is_push_call_frame(
    call: &CallExpression<'_>,
    is_context: impl Fn(&Expression<'_>) -> bool,
) -> bool {
    let context_field = |expression: &Expression<'_>, field: &str| {
        matches!(expression.without_parentheses(), Expression::StaticMemberExpression(member)
            if member.property.name == field && is_context(member.object.without_parentheses()))
    };
    let Expression::StaticMemberExpression(push) = call.callee.without_parentheses() else {
        return false;
    };
    if push.property.name != "push" || !context_field(&push.object, CALL_FRAME_STACK) {
        return false;
    }
    let [Argument::ObjectExpression(frame)] = call.arguments.as_slice() else {
        return false;
    };
    let mut fields = Vec::with_capacity(2);
    for property in &frame.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        let PropertyKey::StaticIdentifier(key) = &property.key else {
            return false;
        };
        if property.kind != PropertyKind::Init || property.method || property.computed {
            return false;
        }
        fields.push((key.name.as_str(), &property.value));
    }
    matches!(fields.as_slice(), [(pc, pc_value), (state, state_value)]
        if *pc == CallFrameField::ReturnByteOffset.as_str()
            && *state == CallFrameField::ReturnDispatchState.as_str()
            && context_field(pc_value, BYTE_OFFSET)
            && context_field(state_value, DISPATCH_STATE))
}

/// `<context>.callFrameStack.pop()`.
pub(crate) fn is_pop_call_frame(
    expression: &Expression<'_>,
    is_context: impl Fn(&Expression<'_>) -> bool,
) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return false;
    };
    let Expression::StaticMemberExpression(pop) = call.callee.without_parentheses() else {
        return false;
    };
    call.arguments.is_empty()
        && pop.property.name == "pop"
        && matches!(pop.object.without_parentheses(), Expression::StaticMemberExpression(stack)
            if stack.property.name == CALL_FRAME_STACK && is_context(stack.object.without_parentheses()))
}

/// The `[callee, this, arguments]` of `Function.prototype.apply.call(callee,
/// this, arguments)`, where `Function` is the unshadowed global.
pub(crate) fn function_apply_call<'c, 'a>(
    call: &'c CallExpression<'a>,
    scoping: &Scoping,
) -> Option<[&'c Expression<'a>; 3]> {
    let member = |expression: &'c Expression<'a>, name: &str| match expression.without_parentheses()
    {
        Expression::StaticMemberExpression(member) if member.property.name == name => {
            Some(&member.object)
        }
        _ => None,
    };
    let apply = member(&call.callee, "call")?;
    let prototype = member(apply, "apply")?;
    let function = member(prototype, "prototype")?;
    let Expression::Identifier(function) = function.without_parentheses() else {
        return None;
    };
    if function.name != "Function" || analysis::identifier_symbol(function, scoping).is_some() {
        return None;
    }
    match call.arguments.as_slice() {
        [callee, this_value, arguments] => Some([
            callee.as_expression()?,
            this_value.as_expression()?,
            arguments.as_expression()?,
        ]),
        _ => None,
    }
}

/// The recurring host-presence side check
/// `if (!(name in host)) throw new SomeError(...)`.
///
/// HandlerIR deliberately drops it: it only guards the non-throwing path that
/// the handler takes anyway. The shape is matched exactly, so any other `if`
/// stays an unsupported control-flow region.
pub(crate) fn is_presence_guard(statement: &IfStatement<'_>) -> bool {
    if statement.alternate.is_some() {
        return false;
    }
    let Expression::UnaryExpression(not) = statement.test.without_parentheses() else {
        return false;
    };
    if not.operator != UnaryOperator::LogicalNot {
        return false;
    }
    let Expression::BinaryExpression(test) = not.argument.without_parentheses() else {
        return false;
    };
    if test.operator != BinaryOperator::In {
        return false;
    }
    let throw = match &statement.consequent {
        Statement::ThrowStatement(throw) => throw,
        Statement::BlockStatement(block) => match block.body.as_slice() {
            [Statement::ThrowStatement(throw)] => throw,
            _ => return false,
        },
        _ => return false,
    };
    matches!(
        throw.argument.without_parentheses(),
        Expression::NewExpression(new) if matches!(new.callee, Expression::Identifier(_))
    )
}

/// `if (x === null || x === void 0) { throw new TypeError(…); }` (or
/// `x == null`), with no `else`.
pub(crate) fn is_null_guard(statement: &IfStatement<'_>) -> bool {
    if statement.alternate.is_some() {
        return false;
    }
    let is_nullish = |expression: &Expression<'_>| match expression.without_parentheses() {
        Expression::NullLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            unary.operator == UnaryOperator::Void
                && matches!(&unary.argument, Expression::NumericLiteral(_))
        }
        Expression::Identifier(id) => id.name == "undefined",
        _ => false,
    };
    let compares_nullish = |expression: &Expression<'_>| match expression.without_parentheses() {
        Expression::BinaryExpression(test) => {
            matches!(
                test.operator,
                BinaryOperator::StrictEquality | BinaryOperator::Equality
            ) && (is_nullish(&test.right) || is_nullish(&test.left))
        }
        _ => false,
    };
    let test = statement.test.without_parentheses();
    let guarded = compares_nullish(test)
        || matches!(test, Expression::LogicalExpression(or)
            if or.operator == LogicalOperator::Or
                && compares_nullish(&or.left)
                && compares_nullish(&or.right));
    if !guarded {
        return false;
    }
    let throw = match &statement.consequent {
        Statement::ThrowStatement(throw) => throw,
        Statement::BlockStatement(block) => match block.body.as_slice() {
            [Statement::ThrowStatement(throw)] => throw,
            _ => return false,
        },
        _ => return false,
    };
    matches!(
        throw.argument.without_parentheses(),
        Expression::NewExpression(new)
            if matches!(&new.callee, Expression::Identifier(id) if id.name == "TypeError")
    )
}

/// The helper a `for (key in object)` key collection becomes: it returns the
/// object's enumerable keys, in for-in order.
pub(crate) const FOR_IN_KEYS: &str = "vmForInKeys";
/// `typeof value === "object" || typeof value === "function" ?
/// Object.create(value) : {}`.
pub(crate) const INHERIT_OR_OBJECT: &str = "vmInheritOrObject";
/// Post an operation to the VM's Worker, starting it first; `false` when it
/// cannot start.
pub const POST_TO_WORKER: &str = "vmPostToWorker";

/// The recipe of a handler whose whole body is one the emitted helpers
/// [`INHERIT_OR_OBJECT`] or [`POST_TO_WORKER`] say: its `if` is outside the
/// recipe subset, a call of the helper is not.
pub(super) fn helper_handler(source: &str, span: SpanRecord) -> Option<Vec<RecipeStep>> {
    use SemanticValue as V;
    let text = source
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("[ ", "[")
        .replace(" ]", "]");
    let number = |value: f64| {
        V::Literal(LiteralValue::DecodedNumber {
            bits: value.to_bits(),
        })
    };
    let below = |k: f64| V::Binary {
        operator: PrimitiveOperator::Subtract,
        left: Box::new(V::StackLength),
        right: Box::new(number(k)),
    };
    let top = |k: f64| V::StackRead {
        index: Box::new(below(k)),
    };
    let call = |name: &str, arguments: Vec<SemanticValue>| V::Call {
        callee: Box::new(V::Binding { name: name.into() }),
        this_value: Box::new(V::Literal(LiteralValue::Undefined)),
        arguments: Box::new(V::ArrayLiteral {
            elements: arguments,
        }),
    };
    let operations = if text
        == r#"function(context) { if (typeof context.stack[context.stack.length - 1] === "object" || typeof context.stack[context.stack.length - 1] === "function") { context.stack[context.stack.length - 1] = Object.create(context.stack[context.stack.length - 1]); return; } context.stack[context.stack.length - 1] = {}; }"#
    {
        vec![RecipeOperation::StackWrite {
            index: below(1.0),
            value: call(INHERIT_OR_OBJECT, vec![top(1.0)]),
        }]
    } else if text
        == "function(context, operand1) { var value1 = operand1; if (!vmInitializeWorker()) { context.stack[context.stack.length - 4] = false; context.stack.length -= 3; return; } vmWorkerCallbacks.push(context.stack[context.stack.length - 1]); vmWorker.postMessage([context.stack[context.stack.length - 4], context.stack[context.stack.length - 3], context.stack[context.stack.length - 2], value1]); context.stack[context.stack.length - 4] = true; context.stack.length -= 3; }"
    {
        vec![
            RecipeOperation::Let {
                temporary: 0,
                value: call(
                    POST_TO_WORKER,
                    vec![
                        top(4.0),
                        top(3.0),
                        top(2.0),
                        V::Operand { position: 1 },
                        top(1.0),
                    ],
                ),
            },
            RecipeOperation::StackWrite {
                index: below(4.0),
                value: V::Temporary { id: 0 },
            },
            RecipeOperation::StackLengthSet { value: below(3.0) },
        ]
    } else {
        return None;
    };
    Some(
        operations
            .into_iter()
            .enumerate()
            .map(|(order, operation)| RecipeStep {
                order: order as u32,
                span: span.clone(),
                operation,
            })
            .collect(),
    )
}

/// A value produced during the walk: the legacy tree plus the occurrence id of
/// its root in the micro-op graph.
struct Built {
    tree: SemanticValue,
    id: OccurrenceId,
}

impl<'a> RecipeBuilder<'a> {
    pub(super) fn new(
        source: &'a Arc<str>,
        scoping: &'a Scoping,
        proof: &'a Proof,
        params: Vec<SymbolId>,
        handler_span: Span,
    ) -> Self {
        Self {
            source,
            scoping,
            proof,
            params,
            locals: BTreeMap::new(),
            receivers: BTreeMap::new(),
            invalid_context: false,
            next_temp: 0,
            graph: GraphBuilder::default(),
            handler_span,
            frame_symbols: BTreeSet::new(),
            try_frame_symbols: BTreeSet::new(),
            error_symbols: BTreeSet::new(),
        }
    }
    /// Finish the walk, attaching identity/provenance to the graph. Legacy
    /// recipe values are projected from this graph by the caller; the builder
    /// never maintains a second recipe authority.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish(
        self,
        semantic_source_hash: &str,
        raw_input_hash: &str,
        table_declaration_ordinal: u32,
        table_element_count: u32,
        table_function_count: u32,
        table_key: u32,
        handler_index: u32,
        handler_span: SpanRecord,
    ) -> HandlerIR {
        self.graph.finish(
            semantic_source_hash,
            raw_input_hash,
            table_declaration_ordinal,
            table_element_count,
            table_function_count,
            table_key,
            handler_index,
            handler_span,
            self.source,
        )
    }
    pub(super) fn body(&mut self, body: &FunctionBody<'a>) {
        for statement in &body.statements {
            self.statement(statement);
        }
    }
    fn statement(&mut self, statement: &Statement<'a>) {
        match statement {
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    self.variable(declarator);
                }
            }
            Statement::ExpressionStatement(stmt) => self.expression_statement(&stmt.expression),
            Statement::ReturnStatement(stmt) => {
                let Built {
                    tree: value,
                    id: value_id,
                } = if let Some(argument) = stmt.argument.as_ref() {
                    self.value_built(argument)
                } else {
                    let id = self.graph.literal(
                        span_record(stmt.span),
                        SpanGranularity::Statement,
                        LiteralValue::Undefined,
                    );
                    Built {
                        tree: SemanticValue::Literal(LiteralValue::Undefined),
                        id,
                    }
                };
                self.push_op(
                    stmt.span,
                    SpanGranularity::Statement,
                    RecipeOperation::Return { value },
                    vec![value_id],
                );
            }
            Statement::ThrowStatement(stmt) => {
                let Built {
                    tree: value,
                    id: value_id,
                } = self.value_built(&stmt.argument);
                self.push_op(
                    stmt.span,
                    SpanGranularity::Statement,
                    RecipeOperation::Throw { value },
                    vec![value_id],
                );
            }
            Statement::BlockStatement(block) => self.body_from_statements(&block.body),
            Statement::EmptyStatement(_) => {}
            Statement::IfStatement(stmt) if is_presence_guard(stmt) => {}
            // `if (x === null || x === void 0) throw new TypeError(…)`: the
            // VM's own check before `x = Object(x)`; neither is program code.
            Statement::IfStatement(stmt) if is_null_guard(stmt) => {}
            Statement::ForInStatement(stmt) if self.for_in_keys(stmt).is_some() => {
                let (temporary, object) = self.for_in_keys(stmt).expect("checked by the guard");
                let object = self.value_built(object);
                let span = span_record(stmt.span);
                let callee = self.graph.binding(
                    span.clone(),
                    SpanGranularity::Expression,
                    FOR_IN_KEYS.into(),
                );
                let this_value = self.graph.literal(
                    span.clone(),
                    SpanGranularity::Expression,
                    LiteralValue::Undefined,
                );
                let arguments = self.graph.array_literal(
                    span.clone(),
                    SpanGranularity::Expression,
                    vec![object.id],
                );
                let call = self.graph.call(
                    span,
                    SpanGranularity::Expression,
                    callee,
                    this_value,
                    arguments,
                );
                self.push_op(
                    stmt.span,
                    SpanGranularity::Statement,
                    RecipeOperation::Assign {
                        temporary,
                        value: SemanticValue::Call {
                            callee: Box::new(SemanticValue::Binding {
                                name: FOR_IN_KEYS.into(),
                            }),
                            this_value: Box::new(SemanticValue::Literal(LiteralValue::Undefined)),
                            arguments: Box::new(SemanticValue::ArrayLiteral {
                                elements: vec![object.tree],
                            }),
                        },
                    },
                    vec![call],
                );
            }
            Statement::IfStatement(stmt) if self.handled_guard(stmt).is_some() => {
                let operation = self.handled_guard(stmt).expect("checked by the guard");
                self.push_op(stmt.span, SpanGranularity::Statement, operation, Vec::new());
            }
            Statement::IfStatement(stmt) if self.conditional_jump(stmt).is_some() => {
                let (target, state) = self.conditional_jump(stmt).expect("checked by the guard");
                let condition = self.value_built(&stmt.test);
                let target = self.value_built(target);
                let state = self.value_built(state);
                self.push_op(
                    stmt.span,
                    SpanGranularity::Statement,
                    RecipeOperation::JumpIf {
                        condition: condition.tree,
                        byte_offset: target.tree,
                        dispatch_state: state.tree,
                    },
                    vec![condition.id, target.id, state.id],
                );
            }
            _ => self.unknown(
                statement.span(),
                "control flow or statement is outside the bounded recipe subset",
                UnknownRegionKind::ControlFlow,
            ),
        }
    }
    fn body_from_statements(&mut self, statements: &[Statement<'a>]) {
        for statement in statements {
            self.statement(statement);
        }
    }
    fn variable(&mut self, declarator: &VariableDeclarator<'a>) {
        let BindingPattern::BindingIdentifier(id) = &declarator.id else {
            self.unknown(
                declarator.span,
                "destructuring local is unsupported",
                UnknownRegionKind::Local,
            );
            return;
        };
        let Some(symbol) = id.symbol_id.get() else {
            self.unknown(
                declarator.span,
                "local has no semantic binding identity",
                UnknownRegionKind::Local,
            );
            return;
        };
        if let Some(init) = declarator.init.as_ref() {
            for (stack, operation) in [
                (TRY_FRAME_STACK, RecipeOperation::PopTryFrame),
                (ERROR_STACK, RecipeOperation::PopErrorRecord),
            ] {
                if self
                    .context_stack_call(init, stack, "pop")
                    .is_some_and(|call| call.arguments.is_empty())
                {
                    // The record is only read through its fields.
                    if stack == TRY_FRAME_STACK {
                        self.try_frame_symbols.insert(symbol);
                    } else {
                        self.error_symbols.insert(symbol);
                    }
                    self.push_op(
                        declarator.span,
                        SpanGranularity::Statement,
                        operation,
                        Vec::new(),
                    );
                    return;
                }
            }
            if self.has_call_frames() && is_pop_call_frame(init, |e| self.is_context(e)) {
                // The frame record itself never becomes a value; its two
                // fields are read through `PoppedCallFrame`.
                self.frame_symbols.insert(symbol);
                self.push_op(
                    declarator.span,
                    SpanGranularity::Statement,
                    RecipeOperation::PopCallFrame,
                    Vec::new(),
                );
                return;
            }
        }
        if self.params.first().copied() == Some(symbol) {
            self.invalid_context = true;
            self.unknown(
                declarator.span,
                "context binding is reassigned or shadowed",
                UnknownRegionKind::Local,
            );
        }
        let Built {
            tree: value,
            id: value_id,
        } = if let Some(init) = declarator.init.as_ref() {
            self.value_built(init)
        } else {
            self.unknown_value(
                declarator.span,
                "uninitialized local",
                UnknownRegionKind::Local,
                SpanGranularity::Statement,
            )
        };
        if let Some(receiver) = declarator.init.as_ref().and_then(|e| self.receiver(e)) {
            self.receivers.insert(symbol, receiver);
        }
        let temp = self.next_temp;
        self.next_temp += 1;
        // Publish the binding only after its initializer has been analyzed. A
        // self-referential `var x = x` is therefore unknown, as in JavaScript's
        // var-hoisted-but-undefined execution, rather than a false alias.
        self.locals.insert(symbol, temp);
        self.push_op(
            declarator.span,
            SpanGranularity::Statement,
            RecipeOperation::Let {
                temporary: temp,
                value,
            },
            vec![value_id],
        );
    }
    fn expression_statement(&mut self, expression: &Expression<'a>) {
        if let Some(operation) = self.exception_statement(expression) {
            self.push_op(
                expression.span(),
                SpanGranularity::Statement,
                operation.0,
                operation.1,
            );
        } else if let Expression::AssignmentExpression(assign) = expression {
            if self.is_to_object(assign) {
                return;
            }
            self.assignment(assign);
        } else if let Expression::CallExpression(call) = expression {
            let vm_call = (self.has_call_frames()
                && is_push_call_frame(call, |e| self.is_context(e)))
                || self.memory_method(&call.callee).is_some();
            if vm_call {
                self.call_statement(call);
            } else {
                // A program call whose result is discarded.
                let Built {
                    tree: value,
                    id: value_id,
                } = self.value_built(expression);
                self.push_op(
                    expression.span(),
                    SpanGranularity::Statement,
                    RecipeOperation::Evaluate { value },
                    vec![value_id],
                );
            }
        } else {
            let Built {
                tree: value,
                id: value_id,
            } = self.value_built(expression);
            self.push_op(
                expression.span(),
                SpanGranularity::Statement,
                RecipeOperation::Evaluate { value },
                vec![value_id],
            );
        }
    }
    /// `for (var key in object) { Array.prototype.push.call(keys, key); }`
    /// into a local array `keys`: the local and `object`. The loop becomes
    /// `keys = vmForInKeys(object)`.
    fn for_in_keys<'s>(&self, stmt: &'s ForInStatement<'a>) -> Option<(u32, &'s Expression<'a>)> {
        let ForStatementLeft::VariableDeclaration(declaration) = &stmt.left else {
            return None;
        };
        let [declarator] = declaration.declarations.as_slice() else {
            return None;
        };
        let BindingPattern::BindingIdentifier(key) = &declarator.id else {
            return None;
        };
        let key = key.symbol_id.get()?;
        let body = match &stmt.body {
            Statement::ExpressionStatement(statement) => &statement.expression,
            Statement::BlockStatement(block) => match block.body.as_slice() {
                [Statement::ExpressionStatement(statement)] => &statement.expression,
                _ => return None,
            },
            _ => return None,
        };
        let Expression::CallExpression(call) = body.without_parentheses() else {
            return None;
        };
        // `Array.prototype.push.call`
        let Expression::StaticMemberExpression(call_member) = &call.callee else {
            return None;
        };
        let Expression::StaticMemberExpression(push) = &call_member.object else {
            return None;
        };
        let Expression::StaticMemberExpression(prototype) = &push.object else {
            return None;
        };
        let is_array = matches!(&prototype.object, Expression::Identifier(id)
            if id.name == "Array" && analysis::identifier_symbol(id, self.scoping).is_none());
        if call_member.property.name != "call"
            || push.property.name != "push"
            || prototype.property.name != "prototype"
            || !is_array
        {
            return None;
        }
        let [Argument::Identifier(keys), Argument::Identifier(pushed)] = call.arguments.as_slice()
        else {
            return None;
        };
        if analysis::identifier_symbol(pushed, self.scoping) != Some(key) {
            return None;
        }
        let keys = analysis::identifier_symbol(keys, self.scoping)?;
        let temporary = *self.locals.get(&keys)?;
        Some((temporary, &stmt.right))
    }
    /// `x = Object(x)` after a null guard: the VM boxes a primitive it is
    /// about to use as an object. The value stays as it is.
    fn is_to_object(&self, assignment: &AssignmentExpression<'a>) -> bool {
        if assignment.operator != AssignmentOperator::Assign {
            return false;
        }
        let Expression::CallExpression(call) = assignment.right.without_parentheses() else {
            return false;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return false;
        };
        if callee.name != "Object" || analysis::identifier_symbol(callee, self.scoping).is_some() {
            return false;
        }
        let [argument] = call.arguments.as_slice() else {
            return false;
        };
        let Some(argument) = argument.as_expression() else {
            return false;
        };
        let text = |span: Span| &self.source[span.start as usize..span.end as usize];
        text(argument.without_parentheses().span()) == text(assignment.left.span())
    }
    fn assignment(&mut self, assignment: &AssignmentExpression<'a>) {
        match &assignment.left {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                let Some(symbol) = analysis::identifier_symbol(id, self.scoping) else {
                    self.unknown(
                        assignment.span,
                        "assignment target is not a bound local",
                        UnknownRegionKind::Assignment,
                    );
                    return;
                };
                if assignment.operator == AssignmentOperator::Assign
                    && self.proof.completion_register_proven
                    && id.name == COMPLETION_REGISTER
                    && self.declared_outside_handler(symbol)
                {
                    let value = self.value_built(&assignment.right);
                    self.push_op(
                        assignment.span,
                        SpanGranularity::Statement,
                        RecipeOperation::CompleteThread { value: value.tree },
                        vec![value.id],
                    );
                    return;
                }
                let rhs = self.value_built(&assignment.right);
                if self.params.first().copied() == Some(symbol) {
                    self.invalid_context = true;
                }
                let Some(&temporary) = self.locals.get(&symbol) else {
                    self.unknown(
                        assignment.span,
                        "assignment to parameter or external binding is unsupported",
                        UnknownRegionKind::Assignment,
                    );
                    return;
                };
                let combined = if assignment.operator == AssignmentOperator::Assign {
                    rhs
                } else {
                    let left_id = self.graph.value_version(temporary).unwrap_or(rhs.id);
                    self.combine_assignment(
                        assignment.operator,
                        (SemanticValue::Temporary { id: temporary }, left_id),
                        rhs,
                        assignment.span,
                    )
                };
                self.push_op(
                    assignment.span,
                    SpanGranularity::Statement,
                    RecipeOperation::Assign {
                        temporary,
                        value: combined.tree,
                    },
                    vec![combined.id],
                );
            }
            AssignmentTarget::StaticMemberExpression(member)
                if assignment.operator == AssignmentOperator::Assign
                    && self.is_context(member.object.without_parentheses())
                    && matches!(member.property.name.as_str(), BYTE_OFFSET | DISPATCH_STATE)
                    && self
                        .proof
                        .property("vmContext", member.property.name.as_str()) =>
            {
                // An absolute PC or dispatch-state write is a VM control
                // transfer; relative advances stay with operand separation.
                let value = self.value_built(&assignment.right);
                let operation = if member.property.name == BYTE_OFFSET {
                    RecipeOperation::SetProgramCounter { value: value.tree }
                } else {
                    RecipeOperation::SetDispatchState { value: value.tree }
                };
                self.push_op(
                    assignment.span,
                    SpanGranularity::Statement,
                    operation,
                    vec![value.id],
                );
            }
            AssignmentTarget::StaticMemberExpression(member) => {
                if self.is_stack_length(&member.object, &member.property.name) {
                    let rhs = self.value_built(&assignment.right);
                    let combined = if assignment.operator == AssignmentOperator::Assign {
                        rhs
                    } else {
                        let left_id = self
                            .graph
                            .stack_length(span_record(member.span), SpanGranularity::Expression);
                        self.combine_assignment(
                            assignment.operator,
                            (SemanticValue::StackLength, left_id),
                            rhs,
                            assignment.span,
                        )
                    };
                    self.push_op(
                        assignment.span,
                        SpanGranularity::Statement,
                        RecipeOperation::StackLengthSet {
                            value: combined.tree,
                        },
                        vec![combined.id],
                    );
                } else if self.is_vm_internal(&member.object) {
                    // PC advances and other context fields are owned by
                    // operand separation and dispatch.
                    self.unknown(
                        assignment.span,
                        "member assignment is not a proven stack length write",
                        UnknownRegionKind::Assignment,
                    );
                } else {
                    let object = self.value_built(&member.object);
                    let key = self.string_key(member.property.span, member.property.name.as_str());
                    let previous = (assignment.operator != AssignmentOperator::Assign).then(|| {
                        let object = self.value_built(&member.object);
                        let key =
                            self.string_key(member.property.span, member.property.name.as_str());
                        self.property_read(member.span, object, key)
                    });
                    self.property_write(assignment, object, key, previous);
                }
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                if self.receiver(&member.object) == Some(Receiver::Stack) {
                    let Built {
                        tree: index_tree,
                        id: index_id,
                    } = self.value_built(&member.expression);
                    let index_temp = self.next_temp;
                    self.next_temp += 1;
                    self.push_op(
                        member.expression.span(),
                        SpanGranularity::Expression,
                        RecipeOperation::Let {
                            temporary: index_temp,
                            value: index_tree,
                        },
                        vec![index_id],
                    );
                    let index_ref = self.graph.temporary_ref(
                        span_record(member.expression.span()),
                        SpanGranularity::Expression,
                        index_temp,
                    );
                    let index = SemanticValue::Temporary { id: index_temp };
                    let rst = self.value_built(&assignment.right);
                    let combined = if assignment.operator == AssignmentOperator::Assign {
                        rst
                    } else {
                        let previous = SemanticValue::StackRead {
                            index: Box::new(index.clone()),
                        };
                        let previous_id = self.graph.stack_read(
                            span_record(member.span),
                            SpanGranularity::Expression,
                            index_ref,
                        );
                        self.combine_assignment(
                            assignment.operator,
                            (previous, previous_id),
                            rst,
                            assignment.span,
                        )
                    };
                    self.push_op(
                        assignment.span,
                        SpanGranularity::Statement,
                        RecipeOperation::StackWrite {
                            index,
                            value: combined.tree,
                        },
                        vec![index_ref, combined.id],
                    );
                } else if self.is_vm_internal(&member.object)
                    || self.mentions_vm_internal(&member.expression)
                {
                    self.unknown(
                        assignment.span,
                        "computed member assignment receiver is not a proven context stack",
                        UnknownRegionKind::Assignment,
                    );
                } else {
                    // Covers `object[key] = value` and putting an item into an
                    // array, including `array[array.length] = value`.
                    let object = self.value_built(&member.object);
                    let key = self.value_built(&member.expression);
                    let previous = (assignment.operator != AssignmentOperator::Assign).then(|| {
                        let object = self.value_built(&member.object);
                        let key = self.value_built(&member.expression);
                        self.property_read(member.span, object, key)
                    });
                    self.property_write(assignment, object, key, previous);
                }
            }
            _ => self.unknown(
                assignment.span,
                "assignment target is unsupported",
                UnknownRegionKind::Assignment,
            ),
        }
    }
    fn combine_assignment(
        &mut self,
        operator: AssignmentOperator,
        left: (SemanticValue, OccurrenceId),
        right: Built,
        span: Span,
    ) -> Built {
        if operator == AssignmentOperator::Assign {
            return right;
        }
        let Some(operator) = assignment_operator(operator) else {
            self.unknown(
                span,
                "logical or unsupported assignment operator",
                UnknownRegionKind::Assignment,
            );
            let reason = "unsupported assignment operator";
            let id =
                self.graph
                    .unknown_value(span_record(span), SpanGranularity::Expression, reason);
            return Built {
                tree: SemanticValue::Unknown {
                    span: span_record(span),
                    reason: reason.into(),
                },
                id,
            };
        };
        let id = self.graph.binary(
            span_record(span),
            SpanGranularity::Expression,
            operator,
            left.1,
            right.id,
        );
        Built {
            tree: SemanticValue::Binary {
                operator,
                left: Box::new(left.0),
                right: Box::new(right.tree),
            },
            id,
        }
    }
    fn call_statement(&mut self, call: &CallExpression<'a>) {
        if self.has_call_frames() && is_push_call_frame(call, |e| self.is_context(e)) {
            self.push_op(
                call.span,
                SpanGranularity::Statement,
                RecipeOperation::PushCallFrame,
                Vec::new(),
            );
            return;
        }
        if let (Some((Receiver::Stack, "push")), [value]) =
            (self.memory_method(&call.callee), call.arguments.as_slice())
        {
            if let Some(value) = value.as_expression() {
                self.stack_push(call.span, value);
                return;
            }
        }
        let Some((receiver, method)) = self
            .memory_method(&call.callee)
            .map(|(receiver, method)| (receiver, method.to_string()))
        else {
            self.unknown(
                call.span,
                "call target is unknown or not a proven memory helper",
                UnknownRegionKind::Call,
            );
            return;
        };
        let Some(args) = call
            .arguments
            .iter()
            .map(Argument::as_expression)
            .collect::<Option<Vec<_>>>()
        else {
            self.unknown(
                call.span,
                "memory helper has an unsupported or spread argument",
                UnknownRegionKind::Call,
            );
            return;
        };
        if receiver != Receiver::Memory || !self.proof.property("vmMemory", &method) {
            self.unknown(
                call.span,
                "memory helper role or arity is not proven",
                UnknownRegionKind::Call,
            );
            return;
        }
        match method.as_str() {
            "writeSlot" if args.len() == 2 => {
                let Built {
                    tree: slot,
                    id: slot_id,
                } = self.value_built(args[0]);
                let Built {
                    tree: value,
                    id: value_id,
                } = self.value_built(args[1]);
                self.push_op(
                    call.span,
                    SpanGranularity::Statement,
                    RecipeOperation::MemoryWrite { slot, value },
                    vec![slot_id, value_id],
                );
            }
            "initializeSlot" if args.len() == 1 => {
                let Built {
                    tree: slot,
                    id: slot_id,
                } = self.value_built(args[0]);
                self.push_op(
                    call.span,
                    SpanGranularity::Statement,
                    RecipeOperation::MemoryInitialize { slot },
                    vec![slot_id],
                );
            }
            _ => self.unknown(
                call.span,
                "memory helper role or arity is not proven",
                UnknownRegionKind::Call,
            ),
        }
    }
    fn value_built(&mut self, expression: &Expression<'a>) -> Built {
        match expression {
            Expression::Identifier(id) => {
                // An unresolved identifier named `undefined` is the global
                // binding.  A local/parameter with that spelling has a
                // SymbolId and continues through the ordinary binding path,
                // so shadowing cannot be mistaken for the global value.
                if id.name == "undefined" && analysis::identifier_symbol(id, self.scoping).is_none()
                {
                    let gid = self.graph.literal(
                        span_record(id.span),
                        SpanGranularity::Expression,
                        LiteralValue::Undefined,
                    );
                    return Built {
                        tree: SemanticValue::Literal(LiteralValue::Undefined),
                        id: gid,
                    };
                }
                let Some(symbol) = analysis::identifier_symbol(id, self.scoping) else {
                    // An unresolved name is a global (for example `host`):
                    // an abstract object expression.
                    return self.binding(id.span, id.name.as_str());
                };
                if self.params.first().copied() == Some(symbol) {
                    return if self.proof.context_binding_proven && !self.invalid_context {
                        let gid = self
                            .graph
                            .context(span_record(id.span), SpanGranularity::Expression);
                        Built {
                            tree: SemanticValue::Context,
                            id: gid,
                        }
                    } else {
                        self.unknown_value(
                            id.span,
                            "context parameter role is not proven or was shadowed",
                            UnknownRegionKind::Expression,
                            SpanGranularity::Expression,
                        )
                    };
                }
                if let Some(position) = self
                    .params
                    .iter()
                    .position(|candidate| *candidate == symbol)
                {
                    let gid = self.graph.operand(
                        span_record(id.span),
                        SpanGranularity::Expression,
                        position as u32,
                    );
                    return Built {
                        tree: SemanticValue::Operand {
                            position: position as u32,
                        },
                        id: gid,
                    };
                }
                if let Some(&temporary) = self.locals.get(&symbol) {
                    let gid = self.graph.temporary_ref(
                        span_record(id.span),
                        SpanGranularity::Expression,
                        temporary,
                    );
                    return Built {
                        tree: SemanticValue::Temporary { id: temporary },
                        id: gid,
                    };
                }
                if self.declared_outside_handler(symbol) {
                    // A captured binding such as `vmStringTable`. The outer
                    // entry's proven host parameter is the JavaScript global
                    // object, so it takes the canonical `host` name; every
                    // other captured name keeps its source spelling.
                    let name = if self.proof.host_parameter == Some(symbol) {
                        HOST
                    } else {
                        id.name.as_str()
                    };
                    return self.binding(id.span, name);
                }
                self.unknown_value(
                    id.span,
                    "identifier binding is outside the handler proof",
                    UnknownRegionKind::Expression,
                    SpanGranularity::Expression,
                )
            }
            Expression::BooleanLiteral(lit) => {
                let value = LiteralValue::Boolean(lit.value);
                let gid = self.graph.literal(
                    span_record(lit.span),
                    SpanGranularity::Expression,
                    value.clone(),
                );
                Built {
                    tree: SemanticValue::Literal(value),
                    id: gid,
                }
            }
            Expression::NullLiteral(lit) => {
                let gid = self.graph.literal(
                    span_record(lit.span),
                    SpanGranularity::Expression,
                    LiteralValue::Null,
                );
                Built {
                    tree: SemanticValue::Literal(LiteralValue::Null),
                    id: gid,
                }
            }
            Expression::NumericLiteral(lit) => {
                // Oxc has already decoded the literal with JavaScript numeric
                // semantics (legacy octal, separators, bases, overflow to
                // binary64).  Carry the decoded value verbatim instead of the
                // raw spelling so the tracer never reparses source text.
                let value = LiteralValue::DecodedNumber {
                    bits: lit.value.to_bits(),
                };
                let gid = self.graph.literal(
                    span_record(lit.span),
                    SpanGranularity::Expression,
                    value.clone(),
                );
                Built {
                    tree: SemanticValue::Literal(value),
                    id: gid,
                }
            }
            Expression::StringLiteral(lit) => {
                if lit.lone_surrogates {
                    return self.unknown_value(
                        lit.span,
                        "string literal contains a lone UTF-16 surrogate and cannot be represented losslessly",
                        UnknownRegionKind::Expression,
                        SpanGranularity::Expression,
                    );
                }
                let value = LiteralValue::String(lit.value.to_string());
                let gid = self.graph.literal(
                    span_record(lit.span),
                    SpanGranularity::Expression,
                    value.clone(),
                );
                Built {
                    tree: SemanticValue::Literal(value),
                    id: gid,
                }
            }
            Expression::StaticMemberExpression(member)
                if self.exception_field(member).is_some() =>
            {
                let tree = self.exception_field(member).expect("checked by the guard");
                let id = self.graph.exception_field(
                    span_record(member.span),
                    SpanGranularity::Expression,
                    tree.clone(),
                );
                Built { tree, id }
            }
            Expression::CallExpression(call)
                if self
                    .context_stack_call(expression, THROWN_VALUE_STACK, "pop")
                    .is_some_and(|call| call.arguments.is_empty()) =>
            {
                let id = self.graph.exception_field(
                    span_record(call.span),
                    SpanGranularity::Expression,
                    SemanticValue::PopThrownValue,
                );
                Built {
                    tree: SemanticValue::PopThrownValue,
                    id,
                }
            }
            Expression::StaticMemberExpression(member) if self.frame_field(member).is_some() => {
                let field = self.frame_field(member).expect("checked by the guard");
                let id = self.graph.popped_call_frame(
                    span_record(member.span),
                    SpanGranularity::Expression,
                    field,
                );
                Built {
                    tree: SemanticValue::PoppedCallFrame { field },
                    id,
                }
            }
            Expression::StaticMemberExpression(member) if self.receiver_field(member).is_some() => {
                let field = self.receiver_field(member).expect("checked by the guard");
                let id = self.graph.receiver(
                    span_record(member.span),
                    SpanGranularity::Expression,
                    field,
                );
                Built {
                    tree: SemanticValue::Receiver { field },
                    id,
                }
            }
            Expression::StaticMemberExpression(member) => {
                if self.is_stack_length(&member.object, &member.property.name) {
                    let gid = self
                        .graph
                        .stack_length(span_record(member.span), SpanGranularity::Expression);
                    Built {
                        tree: SemanticValue::StackLength,
                        id: gid,
                    }
                } else if self.is_vm_internal(&member.object) {
                    // Context fields such as the PC belong to operand
                    // separation and dispatch, not to a generic property read.
                    self.unknown_value(
                        member.span,
                        "static member is not a proven stack length",
                        UnknownRegionKind::Property,
                        SpanGranularity::Expression,
                    )
                } else {
                    let object = self.value_built(&member.object);
                    let key = self.string_key(member.property.span, member.property.name.as_str());
                    self.property_read(member.span, object, key)
                }
            }
            Expression::ComputedMemberExpression(member) => {
                if self.receiver(&member.object) == Some(Receiver::Stack) {
                    let Built {
                        tree: index,
                        id: index_id,
                    } = self.value_built(&member.expression);
                    let gid = self.graph.stack_read(
                        span_record(member.span),
                        SpanGranularity::Expression,
                        index_id,
                    );
                    Built {
                        tree: SemanticValue::StackRead {
                            index: Box::new(index),
                        },
                        id: gid,
                    }
                } else if self.is_vm_internal(&member.object)
                    || self.mentions_vm_internal(&member.expression)
                {
                    // Bytecode reads such as `bytecode[context.pc]` stay one
                    // opaque leaf; operand separation replaces them by span.
                    self.unknown_value(
                        member.span,
                        "computed read receiver is not a proven context stack",
                        UnknownRegionKind::Property,
                        SpanGranularity::Expression,
                    )
                } else {
                    let object = self.value_built(&member.object);
                    let key = self.value_built(&member.expression);
                    self.property_read(member.span, object, key)
                }
            }
            Expression::ObjectExpression(object) => self.object_literal(object),
            Expression::ArrayExpression(array) => self.array_literal(array),
            Expression::BinaryExpression(binary) => {
                let Some(operator) = primitive_binary(binary.operator) else {
                    return self.unknown_value(
                        binary.span,
                        "binary operator is outside the selected primitive subset",
                        UnknownRegionKind::Expression,
                        SpanGranularity::Expression,
                    );
                };
                let Built {
                    tree: left,
                    id: left_id,
                } = self.value_built(&binary.left);
                let Built {
                    tree: right,
                    id: right_id,
                } = self.value_built(&binary.right);
                let gid = self.graph.binary(
                    span_record(binary.span),
                    SpanGranularity::Expression,
                    operator,
                    left_id,
                    right_id,
                );
                Built {
                    tree: SemanticValue::Binary {
                        operator,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    id: gid,
                }
            }
            Expression::UnaryExpression(unary) => {
                if unary.operator == UnaryOperator::Void {
                    if matches!(&unary.argument, Expression::NumericLiteral(literal) if literal.value == 0.0)
                    {
                        let gid = self.graph.literal(
                            span_record(unary.span),
                            SpanGranularity::Expression,
                            LiteralValue::Undefined,
                        );
                        return Built {
                            tree: SemanticValue::Literal(LiteralValue::Undefined),
                            id: gid,
                        };
                    }
                    let Built {
                        tree: value,
                        id: value_id,
                    } = self.value_built(&unary.argument);
                    let gid = self.graph.unary(
                        span_record(unary.span),
                        SpanGranularity::Expression,
                        PrimitiveOperator::Void,
                        value_id,
                    );
                    return Built {
                        tree: SemanticValue::Unary {
                            operator: PrimitiveOperator::Void,
                            value: Box::new(value),
                        },
                        id: gid,
                    };
                }
                if unary.operator == UnaryOperator::Delete {
                    if let Some(built) = self.delete(unary) {
                        return built;
                    }
                }
                let Some(operator) = primitive_unary(unary.operator) else {
                    return self.unknown_value(
                        unary.span,
                        "unary operator is outside the selected primitive subset",
                        UnknownRegionKind::Expression,
                        SpanGranularity::Expression,
                    );
                };
                let Built {
                    tree: value,
                    id: value_id,
                } = self.value_built(&unary.argument);
                let gid = self.graph.unary(
                    span_record(unary.span),
                    SpanGranularity::Expression,
                    operator,
                    value_id,
                );
                Built {
                    tree: SemanticValue::Unary {
                        operator,
                        value: Box::new(value),
                    },
                    id: gid,
                }
            }
            Expression::FunctionExpression(function)
                if self.is_closed_function(function)
                    || self.is_external_return_closure(function) =>
            {
                let source = self.source[function.span.start as usize..function.span.end as usize]
                    .to_string();
                let (tree, graph_source) = match compile_closure_plan_from_function(function) {
                    Ok(plan) => {
                        let canonical = plan.to_source();
                        (SemanticValue::ClosurePlan { plan }, canonical)
                    }
                    // The raw library continues to represent admitted but
                    // unsupported legacy forms. Artifact compilation then
                    // rejects those forms explicitly in its compiler-side
                    // lowering instead of serializing arbitrary source.
                    Err(_) => {
                        let graph_source = source.clone();
                        (SemanticValue::FunctionLiteral { source }, graph_source)
                    }
                };
                let id = self.graph.function_literal(
                    span_record(function.span),
                    SpanGranularity::Expression,
                    graph_source,
                );
                Built { tree, id }
            }
            Expression::NewExpression(new) => match self.construct(new) {
                Some(built) => built,
                None => self.unknown_value(
                    new.span,
                    "constructor call",
                    UnknownRegionKind::Call,
                    SpanGranularity::Expression,
                ),
            },
            Expression::CallExpression(call) if self.stack_slice_start(call).is_some() => {
                let start = self.stack_slice_start(call).expect("checked by the guard");
                let start = self.value_built(start);
                let id = self.graph.stack_slice(
                    span_record(call.span),
                    SpanGranularity::Expression,
                    start.id,
                );
                Built {
                    tree: SemanticValue::StackSlice {
                        start: Box::new(start.tree),
                    },
                    id,
                }
            }
            Expression::CallExpression(call) if self.create_thread_arguments(call).is_some() => {
                let [descriptor, byte_offset, dispatch_state] = self
                    .create_thread_arguments(call)
                    .expect("checked by the guard");
                let descriptor = self.value_built(descriptor);
                let byte_offset = self.value_built(byte_offset);
                let dispatch_state = self.value_built(dispatch_state);
                let id = self.graph.create_thread(
                    span_record(call.span),
                    SpanGranularity::Expression,
                    descriptor.id,
                    byte_offset.id,
                    dispatch_state.id,
                );
                Built {
                    tree: SemanticValue::CreateThread {
                        descriptor: Box::new(descriptor.tree),
                        byte_offset: Box::new(byte_offset.tree),
                        dispatch_state: Box::new(dispatch_state.tree),
                    },
                    id,
                }
            }
            Expression::CallExpression(call)
                if function_apply_call(call, self.scoping).is_some() =>
            {
                let [callee, this_value, arguments] =
                    function_apply_call(call, self.scoping).expect("checked by the guard");
                let callee = self.value_built(callee);
                let this_value = self.value_built(this_value);
                let arguments = self.value_built(arguments);
                let id = self.graph.call(
                    span_record(call.span),
                    SpanGranularity::Expression,
                    callee.id,
                    this_value.id,
                    arguments.id,
                );
                Built {
                    tree: SemanticValue::Call {
                        callee: Box::new(callee.tree),
                        this_value: Box::new(this_value.tree),
                        arguments: Box::new(arguments.tree),
                    },
                    id,
                }
            }
            Expression::CallExpression(call) => {
                if let Some(method) = self.proven_memory_method(&call.callee) {
                    let Some(args) = call
                        .arguments
                        .iter()
                        .map(Argument::as_expression)
                        .collect::<Option<Vec<_>>>()
                    else {
                        return self.unknown_value(
                            call.span,
                            "memory helper has an unsupported or spread argument",
                            UnknownRegionKind::Call,
                            SpanGranularity::Expression,
                        );
                    };
                    if method == "readSlot" && args.len() == 1 {
                        let Built {
                            tree: slot,
                            id: slot_id,
                        } = self.value_built(args[0]);
                        let gid = self.graph.memory_read(
                            span_record(call.span),
                            SpanGranularity::Expression,
                            slot_id,
                        );
                        Built {
                            tree: SemanticValue::MemoryRead {
                                slot: Box::new(slot),
                            },
                            id: gid,
                        }
                    } else {
                        self.unknown_value(
                            call.span,
                            "memory call is not a proven readSlot expression",
                            UnknownRegionKind::Call,
                            SpanGranularity::Expression,
                        )
                    }
                } else if let Some(built) = self.general_call(call) {
                    built
                } else {
                    self.unknown_value(
                        call.span,
                        "call expression may throw and has no bounded recipe",
                        UnknownRegionKind::Call,
                        SpanGranularity::Expression,
                    )
                }
            }
            _ => self.unknown_value(
                expression.span(),
                "expression is outside the bounded recipe subset",
                UnknownRegionKind::Expression,
                SpanGranularity::Expression,
            ),
        }
    }
    fn receiver(&self, expression: &Expression<'a>) -> Option<Receiver> {
        match expression {
            Expression::Identifier(id) => analysis::identifier_symbol(id, self.scoping)
                .and_then(|symbol| self.receivers.get(&symbol).copied()),
            Expression::StaticMemberExpression(member) if self.is_context(&member.object) => {
                match member.property.name.as_str() {
                    "stack" if self.proof.property("vmContext", "stack") => Some(Receiver::Stack),
                    "memory" if self.proof.property("vmContext", "memory") => {
                        Some(Receiver::Memory)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
    fn is_context(&self, expression: &Expression<'a>) -> bool {
        matches!(
            self.value_without_unknown(expression),
            Some(SemanticValue::Context)
        )
    }
    fn value_without_unknown(&self, expression: &Expression<'a>) -> Option<SemanticValue> {
        match expression {
            Expression::Identifier(id) => {
                let symbol = analysis::identifier_symbol(id, self.scoping)?;
                (self.params.first().copied() == Some(symbol)
                    && self.proof.context_binding_proven
                    && !self.invalid_context)
                    .then_some(SemanticValue::Context)
            }
            _ => None,
        }
    }
    /// `context.stack.push(value)`: the value is evaluated before the push
    /// reads the length it appends at.
    fn stack_push(&mut self, span: Span, value: &Expression<'a>) {
        let value = self.value_built(value);
        let index_temp = self.next_temp;
        self.next_temp += 1;
        let length = self
            .graph
            .stack_length(span_record(span), SpanGranularity::Expression);
        self.push_op(
            span,
            SpanGranularity::Expression,
            RecipeOperation::Let {
                temporary: index_temp,
                value: SemanticValue::StackLength,
            },
            vec![length],
        );
        let index_ref =
            self.graph
                .temporary_ref(span_record(span), SpanGranularity::Expression, index_temp);
        self.push_op(
            span,
            SpanGranularity::Statement,
            RecipeOperation::StackWrite {
                index: SemanticValue::Temporary { id: index_temp },
                value: value.tree,
            },
            vec![index_ref, value.id],
        );
    }
    /// A program call `f(args)` or `object[key](args)` as a [`SemanticValue::Call`].
    ///
    /// A method call's receiver is evaluated once for the callee and once as
    /// `this`, so it must be side-effect free: a local, a captured or global
    /// name, a stack read or a literal. Calls that touch VM internals stay
    /// unknown.
    fn general_call(&mut self, call: &CallExpression<'a>) -> Option<Built> {
        let arguments = call
            .arguments
            .iter()
            .map(Argument::as_expression)
            .collect::<Option<Vec<_>>>()?;
        let callee = call.callee.without_parentheses();
        if self.is_vm_internal(callee) || self.mentions_vm_internal(callee) {
            return None;
        }
        let receiver = match callee {
            Expression::Identifier(_) => None,
            Expression::StaticMemberExpression(member) => Some(&member.object),
            Expression::ComputedMemberExpression(member) => Some(&member.object),
            _ => return None,
        };
        if receiver.is_some_and(|receiver| !self.is_pure_receiver(receiver)) {
            return None;
        }
        let callee = self.value_built(callee);
        let this_value = match receiver {
            Some(receiver) => self.value_built(receiver),
            None => {
                let id = self.graph.literal(
                    span_record(call.callee.span()),
                    SpanGranularity::Expression,
                    LiteralValue::Undefined,
                );
                Built {
                    tree: SemanticValue::Literal(LiteralValue::Undefined),
                    id,
                }
            }
        };
        let mut trees = Vec::with_capacity(arguments.len());
        let mut ids = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let built = self.value_built(argument);
            trees.push(built.tree);
            ids.push(built.id);
        }
        let arguments_id =
            self.graph
                .array_literal(span_record(call.span), SpanGranularity::Expression, ids);
        let id = self.graph.call(
            span_record(call.span),
            SpanGranularity::Expression,
            callee.id,
            this_value.id,
            arguments_id,
        );
        Some(Built {
            tree: SemanticValue::Call {
                callee: Box::new(callee.tree),
                this_value: Box::new(this_value.tree),
                arguments: Box::new(SemanticValue::ArrayLiteral { elements: trees }),
            },
            id,
        })
    }
    /// `context.<stack>.<method>(…)` on a proven context stack field.
    fn context_stack_call<'c>(
        &self,
        expression: &'c Expression<'a>,
        stack: &str,
        method: &str,
    ) -> Option<&'c CallExpression<'a>> {
        let Expression::CallExpression(call) = expression.without_parentheses() else {
            return None;
        };
        let Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else {
            return None;
        };
        let Expression::StaticMemberExpression(field) = callee.object.without_parentheses() else {
            return None;
        };
        (callee.property.name == method
            && field.property.name == stack
            && self.proof.property("vmContext", stack)
            && self.is_context(field.object.without_parentheses()))
        .then_some(call)
    }
    /// `{ key: value, … }` with plain static keys, in order.
    fn record_fields<'c>(argument: &'c Argument<'a>) -> Option<Vec<(&'c str, &'c Expression<'a>)>> {
        let Argument::ObjectExpression(object) = argument else {
            return None;
        };
        object
            .properties
            .iter()
            .map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property)
                    if property.kind == PropertyKind::Init
                        && !property.method
                        && !property.computed =>
                {
                    match &property.key {
                        PropertyKey::StaticIdentifier(key) => {
                            Some((key.name.as_str(), &property.value))
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect()
    }
    /// `context.<field>` on the proven context.
    fn is_context_field(&self, expression: &Expression<'a>, field: &str) -> bool {
        matches!(expression.without_parentheses(), Expression::StaticMemberExpression(member)
            if member.property.name == field
                && self.proof.property("vmContext", field)
                && self.is_context(member.object.without_parentheses()))
    }
    /// A statement of the VM exception machinery, with its input values.
    fn exception_statement(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<(RecipeOperation, Vec<OccurrenceId>)> {
        if let Some(call) = self.context_stack_call(expression, TRY_FRAME_STACK, "push") {
            let [frame] = call.arguments.as_slice() else {
                return None;
            };
            let fields = Self::record_fields(frame)?;
            let [("handlerByteOffset", target), ("handlerDispatchState", state), ("errorEntryCount", Expression::NumericLiteral(count))] =
                fields.as_slice()
            else {
                return None;
            };
            if count.value != 0.0 || !self.is_pure_receiver(target) || !self.is_pure_receiver(state)
            {
                return None;
            }
            let target = self.value_built(target);
            let state = self.value_built(state);
            return Some((
                RecipeOperation::PushTryFrame {
                    byte_offset: target.tree,
                    dispatch_state: state.tree,
                },
                vec![target.id, state.id],
            ));
        }
        if let Some(call) = self.context_stack_call(expression, ERROR_STACK, "push") {
            let [record] = call.arguments.as_slice() else {
                return None;
            };
            let fields = Self::record_fields(record)?;
            let [("wasExceptionHandled", Expression::BooleanLiteral(handled)), ("savedByteOffset", target), ("savedDispatchState", state)] =
                fields.as_slice()
            else {
                return None;
            };
            return (!handled.value
                && self.is_context_field(target, BYTE_OFFSET)
                && self.is_context_field(state, DISPATCH_STATE))
            .then_some((RecipeOperation::PushFinallyRecord, Vec::new()));
        }
        for (stack, operation) in [
            (TRY_FRAME_STACK, RecipeOperation::PopTryFrame),
            (ERROR_STACK, RecipeOperation::PopErrorRecord),
        ] {
            if self
                .context_stack_call(expression, stack, "pop")
                .is_some_and(|call| call.arguments.is_empty())
            {
                return Some((operation, Vec::new()));
            }
        }
        None
    }
    /// `record.field` on a local bound to a popped try frame or error record.
    fn exception_field(&self, member: &StaticMemberExpression<'a>) -> Option<SemanticValue> {
        let Expression::Identifier(id) = member.object.without_parentheses() else {
            return None;
        };
        let symbol = analysis::identifier_symbol(id, self.scoping)?;
        if self.try_frame_symbols.contains(&symbol) {
            [
                TryFrameField::HandlerByteOffset,
                TryFrameField::HandlerDispatchState,
            ]
            .into_iter()
            .find(|field| member.property.name == field.as_str())
            .map(|field| SemanticValue::PoppedTryFrame { field })
        } else if self.error_symbols.contains(&symbol) {
            [
                ErrorRecordField::WasExceptionHandled,
                ErrorRecordField::SavedByteOffset,
                ErrorRecordField::SavedDispatchState,
            ]
            .into_iter()
            .find(|field| member.property.name == field.as_str())
            .map(|field| SemanticValue::PoppedErrorRecord { field })
        } else {
            None
        }
    }
    /// `if (record.wasExceptionHandled) { throw context.thrownValueStack.pop(); }`
    /// or `{ context.thrownValueStack.pop(); }` on a popped error record.
    fn handled_guard(&self, statement: &IfStatement<'a>) -> Option<RecipeOperation> {
        if statement.alternate.is_some() {
            return None;
        }
        let Expression::StaticMemberExpression(test) = statement.test.without_parentheses() else {
            return None;
        };
        if self.exception_field(test)
            != Some(SemanticValue::PoppedErrorRecord {
                field: ErrorRecordField::WasExceptionHandled,
            })
        {
            return None;
        }
        let body = match &statement.consequent {
            Statement::BlockStatement(block) => match block.body.as_slice() {
                [only] => only,
                _ => return None,
            },
            only => only,
        };
        let pops = |expression: &Expression<'a>| {
            self.context_stack_call(expression, THROWN_VALUE_STACK, "pop")
                .is_some_and(|call| call.arguments.is_empty())
        };
        match body {
            Statement::ThrowStatement(throw) if pops(&throw.argument) => {
                Some(RecipeOperation::RethrowIfHandled)
            }
            Statement::ExpressionStatement(drop) if pops(&drop.expression) => {
                Some(RecipeOperation::DropThrownIfHandled)
            }
            _ => None,
        }
    }
    /// `new callee(arguments…)` on a program value with plain arguments.
    fn construct(&mut self, new: &NewExpression<'a>) -> Option<Built> {
        let arguments = new
            .arguments
            .iter()
            .map(Argument::as_expression)
            .collect::<Option<Vec<_>>>()?;
        let callee = new.callee.without_parentheses();
        if self.is_vm_internal(callee) || self.mentions_vm_internal(callee) {
            return None;
        }
        let callee = self.value_built(callee);
        let mut trees = Vec::with_capacity(arguments.len());
        let mut ids = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let built = self.value_built(argument);
            trees.push(built.tree);
            ids.push(built.id);
        }
        let arguments_id =
            self.graph
                .array_literal(span_record(new.span), SpanGranularity::Expression, ids);
        let id = self.graph.construct(
            span_record(new.span),
            SpanGranularity::Expression,
            callee.id,
            arguments_id,
        );
        Some(Built {
            tree: SemanticValue::Construct {
                callee: Box::new(callee.tree),
                arguments: Box::new(SemanticValue::ArrayLiteral { elements: trees }),
            },
            id,
        })
    }
    /// `delete object[key]` or `delete object.key` on a program object.
    fn delete(&mut self, unary: &UnaryExpression<'a>) -> Option<Built> {
        let (object, key) = match unary.argument.without_parentheses() {
            Expression::ComputedMemberExpression(member) => {
                let object = member.object.without_parentheses();
                if self.is_vm_internal(object) {
                    return None;
                }
                (
                    self.value_built(object),
                    self.value_built(&member.expression),
                )
            }
            Expression::StaticMemberExpression(member) => {
                let object = member.object.without_parentheses();
                if self.is_vm_internal(object) {
                    return None;
                }
                let object = self.value_built(object);
                let name = member.property.name.to_string();
                let id = self.graph.literal(
                    span_record(member.property.span),
                    SpanGranularity::Expression,
                    LiteralValue::String(name.clone()),
                );
                (
                    object,
                    Built {
                        tree: SemanticValue::Literal(LiteralValue::String(name)),
                        id,
                    },
                )
            }
            _ => return None,
        };
        let id = self.graph.delete(
            span_record(unary.span),
            SpanGranularity::Expression,
            object.id,
            key.id,
        );
        Some(Built {
            tree: SemanticValue::Delete {
                object: Box::new(object.tree),
                key: Box::new(key.tree),
            },
            id,
        })
    }
    /// A function expression that uses no binding from outside itself, so its
    /// source means the same anywhere: parameters, locals and globals only.
    fn is_closed_function(&self, function: &Function<'a>) -> bool {
        struct FreeNames<'s> {
            scoping: &'s Scoping,
            span: Span,
            closed: bool,
        }
        impl<'a> Visit<'a> for FreeNames<'_> {
            fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
                if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                    let declared = self.scoping.symbol_span(symbol);
                    if declared.start < self.span.start || declared.end > self.span.end {
                        self.closed = false;
                    }
                }
            }
        }
        let mut names = FreeNames {
            scoping: self.scoping,
            span: function.span,
            closed: true,
        };
        names.visit_function(function, oxc_syntax::scope::ScopeFlags::Function);
        names.closed
    }
    /// A deliberately narrow closure exception for the branch fixture's
    /// `function () { return host; }`: the body is only a returned outer
    /// binding, so the emitter can preserve its external name without
    /// serializing the enclosing source or an AST.
    fn is_external_return_closure(&self, function: &Function<'a>) -> bool {
        if !function.params.items.is_empty()
            || function.params.rest.is_some()
            || function.params.items.iter().any(|parameter| {
                parameter.initializer.is_some()
                    || !matches!(parameter.pattern, BindingPattern::BindingIdentifier(_))
            })
        {
            return false;
        }
        let Some(body) = function.body.as_deref() else {
            return false;
        };
        let [Statement::ReturnStatement(returned)] = body.statements.as_slice() else {
            return false;
        };
        let Some(Expression::Identifier(identifier)) = returned.argument.as_ref() else {
            return false;
        };
        let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
            return false;
        };
        let declared = self.scoping.symbol_span(symbol);
        declared.start < function.span.start || declared.end > function.span.end
    }
    /// `if (test) { context.byteOffset = target; context.dispatchState = state; }`
    /// on the proven context, with side-effect-free targets: a conditional
    /// jump. Returns the two targets.
    fn conditional_jump<'s>(
        &self,
        statement: &'s IfStatement<'a>,
    ) -> Option<(&'s Expression<'a>, &'s Expression<'a>)> {
        if statement.alternate.is_some()
            || !self.proof.property("vmContext", BYTE_OFFSET)
            || !self.proof.property("vmContext", DISPATCH_STATE)
        {
            return None;
        }
        let Statement::BlockStatement(block) = &statement.consequent else {
            return None;
        };
        let [Statement::ExpressionStatement(first), Statement::ExpressionStatement(second)] =
            block.body.as_slice()
        else {
            return None;
        };
        let target = self.context_assignment(&first.expression, BYTE_OFFSET)?;
        let state = self.context_assignment(&second.expression, DISPATCH_STATE)?;
        // The targets are only evaluated when the jump is taken.
        (self.is_pure_receiver(target) && self.is_pure_receiver(state)).then_some((target, state))
    }
    /// The right-hand side of `context.<field> = value`.
    fn context_assignment<'s>(
        &self,
        expression: &'s Expression<'a>,
        field: &str,
    ) -> Option<&'s Expression<'a>> {
        let Expression::AssignmentExpression(assignment) = expression else {
            return None;
        };
        let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
            return None;
        };
        (assignment.operator == AssignmentOperator::Assign
            && member.property.name == field
            && self.is_context(member.object.without_parentheses()))
        .then_some(&assignment.right)
    }
    fn is_pure_receiver(&self, expression: &Expression<'a>) -> bool {
        match expression.without_parentheses() {
            Expression::Identifier(_) => !self.is_context_parameter(expression),
            Expression::ComputedMemberExpression(member) => {
                self.receiver(&member.object) == Some(Receiver::Stack)
                    && !self.mentions_vm_internal(&member.expression)
            }
            Expression::StringLiteral(_) | Expression::NumericLiteral(_) => true,
            _ => false,
        }
    }
    /// The first three arguments of
    /// `vmCreateThread(descriptor, byteOffset, dispatchState, context.memory)`,
    /// where `vmCreateThread` is the proven captured role binding.
    fn create_thread_arguments<'c>(
        &self,
        call: &'c CallExpression<'a>,
    ) -> Option<[&'c Expression<'a>; 3]> {
        let Expression::Identifier(callee) = call.callee.without_parentheses() else {
            return None;
        };
        let symbol = analysis::identifier_symbol(callee, self.scoping)?;
        if !self.proof.create_thread_proven
            || callee.name != CREATE_THREAD
            || !self.declared_outside_handler(symbol)
        {
            return None;
        }
        match call.arguments.as_slice() {
            [descriptor, byte_offset, dispatch_state, memory]
                if memory
                    .as_expression()
                    .is_some_and(|memory| self.receiver(memory) == Some(Receiver::Memory)) =>
            {
                Some([
                    descriptor.as_expression()?,
                    byte_offset.as_expression()?,
                    dispatch_state.as_expression()?,
                ])
            }
            _ => None,
        }
    }
    /// The `start` of `context.stack.slice(start)` on the proven stack.
    fn stack_slice_start<'c>(&self, call: &'c CallExpression<'a>) -> Option<&'c Expression<'a>> {
        let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else {
            return None;
        };
        if member.property.name != "slice" || self.receiver(&member.object) != Some(Receiver::Stack)
        {
            return None;
        }
        match call.arguments.as_slice() {
            [start] => start.as_expression(),
            _ => None,
        }
    }
    fn is_stack_length(&self, object: &Expression<'a>, property: &str) -> bool {
        property == "length" && self.receiver(object) == Some(Receiver::Stack)
    }
    fn memory_method(&self, callee: &Expression<'a>) -> Option<(Receiver, &str)> {
        let Expression::StaticMemberExpression(member) = callee else {
            return None;
        };
        self.receiver(&member.object)
            .map(|r| (r, member.property.name.as_str()))
    }
    fn proven_memory_method(&self, callee: &Expression<'a>) -> Option<&str> {
        let (receiver, method) = self.memory_method(callee)?;
        (receiver == Receiver::Memory && self.proof.property("vmMemory", method)).then_some(method)
    }
    /// Whether the proven context has a call-frame stack.
    fn has_call_frames(&self) -> bool {
        self.proof.property("vmContext", CALL_FRAME_STACK)
    }
    /// `frame.returnByteOffset` / `frame.returnDispatchState` on a local bound
    /// to a popped call frame.
    fn frame_field(&self, member: &StaticMemberExpression<'a>) -> Option<CallFrameField> {
        let Expression::Identifier(id) = member.object.without_parentheses() else {
            return None;
        };
        let symbol = analysis::identifier_symbol(id, self.scoping)?;
        if !self.frame_symbols.contains(&symbol) {
            return None;
        }
        [
            CallFrameField::ReturnByteOffset,
            CallFrameField::ReturnDispatchState,
        ]
        .into_iter()
        .find(|field| member.property.name == field.as_str())
    }
    /// `context.receiverObject` / `context.receiverValue` on the proven
    /// context, when the pipeline proved that field.
    fn receiver_field(&self, member: &StaticMemberExpression<'a>) -> Option<ReceiverField> {
        if !self.is_context(&member.object) {
            return None;
        }
        [ReceiverField::Object, ReceiverField::Value]
            .into_iter()
            .find(|field| {
                member.property.name == field.as_str()
                    && self.proof.property("vmContext", field.as_str())
            })
    }
    fn declared_outside_handler(&self, symbol: SymbolId) -> bool {
        let declaration = self.scoping.symbol_span(symbol);
        declaration.end <= self.handler_span.start || declaration.start >= self.handler_span.end
    }
    /// The context parameter, a member of it, or a proven stack/memory
    /// receiver. These keep their dedicated VM nodes and are never generic
    /// objects, even when the context role is unproven or shadowed.
    fn is_vm_internal(&self, expression: &Expression<'a>) -> bool {
        let expression = expression.without_parentheses();
        self.is_context_parameter(expression)
            || self.receiver(expression).is_some()
            || match expression {
                Expression::StaticMemberExpression(member) => {
                    self.is_context_parameter(member.object.without_parentheses())
                }
                Expression::ComputedMemberExpression(member) => {
                    self.is_context_parameter(member.object.without_parentheses())
                }
                _ => false,
            }
    }
    /// The first handler parameter, regardless of whether its context role is
    /// proven or later invalidated.
    fn is_context_parameter(&self, expression: &Expression<'a>) -> bool {
        matches!(expression, Expression::Identifier(id)
            if analysis::identifier_symbol(id, self.scoping).is_some()
                && analysis::identifier_symbol(id, self.scoping) == self.params.first().copied())
    }
    /// Whether an index expression touches context state other than the proven
    /// stack/memory receivers (for example `context.pc`) at its own level. Such
    /// a read is an operand/PC concern and stays opaque. Nested `x[...]` reads
    /// are not inspected: they make their own decision when built, so
    /// `table[bytecode[context.pc]]` keeps `table[...]` and leaves only the byte
    /// read opaque.
    fn mentions_vm_internal(&self, expression: &Expression<'a>) -> bool {
        let mut probe = ContextProbe {
            builder: self,
            found: false,
        };
        probe.visit_expression(expression);
        probe.found
    }
    fn binding(&mut self, span: Span, name: &str) -> Built {
        let id = self
            .graph
            .binding(span_record(span), SpanGranularity::Expression, name.into());
        Built {
            tree: SemanticValue::Binding { name: name.into() },
            id,
        }
    }
    fn string_key(&mut self, span: Span, name: &str) -> Built {
        let value = LiteralValue::String(name.into());
        let id = self.graph.literal(
            span_record(span),
            SpanGranularity::Expression,
            value.clone(),
        );
        Built {
            tree: SemanticValue::Literal(value),
            id,
        }
    }
    fn property_read(&mut self, span: Span, object: Built, key: Built) -> Built {
        let id = self.graph.property_read(
            span_record(span),
            SpanGranularity::Expression,
            object.id,
            key.id,
        );
        Built {
            tree: SemanticValue::PropertyRead {
                object: Box::new(object.tree),
                key: Box::new(key.tree),
            },
            id,
        }
    }
    fn property_write(
        &mut self,
        assignment: &AssignmentExpression<'a>,
        object: Built,
        key: Built,
        previous: Option<Built>,
    ) {
        let rhs = self.value_built(&assignment.right);
        let value = match previous {
            Some(previous) => self.combine_assignment(
                assignment.operator,
                (previous.tree, previous.id),
                rhs,
                assignment.span,
            ),
            None => rhs,
        };
        self.push_op(
            assignment.span,
            SpanGranularity::Statement,
            RecipeOperation::PropertyWrite {
                object: object.tree,
                key: key.tree,
                value: value.tree,
            },
            vec![object.id, key.id, value.id],
        );
    }
    fn object_literal(&mut self, object: &ObjectExpression<'a>) -> Built {
        let mut entries = Vec::with_capacity(object.properties.len());
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return self.unknown_value(
                    object.span,
                    "object literal spread is unsupported",
                    UnknownRegionKind::Expression,
                    SpanGranularity::Expression,
                );
            };
            let key = match &property.key {
                _ if property.kind != PropertyKind::Init
                    || property.method
                    || property.computed =>
                {
                    None
                }
                PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
                PropertyKey::StringLiteral(literal) if !literal.lone_surrogates => {
                    Some(literal.value.to_string())
                }
                PropertyKey::NumericLiteral(literal) => {
                    Some(crate::js_number::number_to_js_string(literal.value))
                }
                _ => None,
            };
            let Some(key) = key else {
                return self.unknown_value(
                    object.span,
                    "object literal property is not a plain static key",
                    UnknownRegionKind::Expression,
                    SpanGranularity::Expression,
                );
            };
            entries.push((key, &property.value));
        }
        let mut keys = Vec::with_capacity(entries.len());
        let mut properties = Vec::with_capacity(entries.len());
        let mut ids = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            let built = self.value_built(value);
            keys.push(key.clone());
            properties.push((key, built.tree));
            ids.push(built.id);
        }
        let id = self.graph.object_literal(
            span_record(object.span),
            SpanGranularity::Expression,
            keys,
            ids,
        );
        Built {
            tree: SemanticValue::ObjectLiteral { properties },
            id,
        }
    }
    fn array_literal(&mut self, array: &ArrayExpression<'a>) -> Built {
        let Some(elements) = array
            .elements
            .iter()
            .map(ArrayExpressionElement::as_expression)
            .collect::<Option<Vec<_>>>()
        else {
            return self.unknown_value(
                array.span,
                "array literal hole or spread is unsupported",
                UnknownRegionKind::Expression,
                SpanGranularity::Expression,
            );
        };
        let mut trees = Vec::with_capacity(elements.len());
        let mut ids = Vec::with_capacity(elements.len());
        for element in elements {
            let built = self.value_built(element);
            trees.push(built.tree);
            ids.push(built.id);
        }
        let id =
            self.graph
                .array_literal(span_record(array.span), SpanGranularity::Expression, ids);
        Built {
            tree: SemanticValue::ArrayLiteral { elements: trees },
            id,
        }
    }
    /// Record one operation occurrence in the graph and project the matching
    /// legacy step.  Both are produced from the same data; the graph is the
    /// semantic record and the step is the compatibility projection.
    fn push_op(
        &mut self,
        span: Span,
        granularity: SpanGranularity,
        operation: RecipeOperation,
        inputs: Vec<OccurrenceId>,
    ) {
        self.push_op_with(span, granularity, operation, inputs, None, None);
    }
    fn push_op_with(
        &mut self,
        span: Span,
        granularity: SpanGranularity,
        operation: RecipeOperation,
        inputs: Vec<OccurrenceId>,
        region: Option<UnknownRegionKind>,
        source: Option<String>,
    ) {
        let order = self.graph.next_step_order();
        self.graph.operation(
            span_record(span),
            granularity,
            order,
            &operation,
            &inputs,
            region,
            source,
        );
    }
    fn unknown_value(
        &mut self,
        span: Span,
        reason: &str,
        kind: UnknownRegionKind,
        granularity: SpanGranularity,
    ) -> Built {
        let id = self
            .graph
            .unknown_value(span_record(span), granularity, reason);
        self.unknown(span, reason, kind);
        Built {
            tree: SemanticValue::Unknown {
                span: span_record(span),
                reason: reason.into(),
            },
            id,
        }
    }
    pub(super) fn unknown(&mut self, span: Span, reason: &str, kind: UnknownRegionKind) {
        let source = self.text(span);
        self.push_op_with(
            span,
            SpanGranularity::Statement,
            RecipeOperation::Unknown {
                reason: reason.into(),
                source: source.clone(),
            },
            Vec::new(),
            Some(kind),
            Some(source),
        );
    }
    fn text(&self, span: Span) -> String {
        self.source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .into()
    }
}

/// Finds context state other than the proven stack/memory receivers.
struct ContextProbe<'b, 'a> {
    builder: &'b RecipeBuilder<'a>,
    found: bool,
}

impl<'a> Visit<'a> for ContextProbe<'_, 'a> {
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if self
            .builder
            .is_context_parameter(member.object.without_parentheses())
        {
            let proven_receiver = matches!(member.property.name.as_str(), "stack" | "memory")
                && self
                    .builder
                    .proof
                    .property("vmContext", member.property.name.as_str());
            if !proven_receiver {
                self.found = true;
            }
            return;
        }
        walk::walk_static_member_expression(self, member);
    }
    fn visit_computed_member_expression(&mut self, _member: &ComputedMemberExpression<'a>) {}
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        // A bare context reference reaching here is not a proven receiver use.
        if analysis::identifier_symbol(identifier, self.builder.scoping)
            .is_some_and(|symbol| self.builder.params.first().copied() == Some(symbol))
        {
            self.found = true;
        }
    }
    fn visit_function(&mut self, _function: &Function<'a>, _flags: oxc_syntax::scope::ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _function: &ArrowFunctionExpression<'a>) {}
}

fn assignment_operator(operator: AssignmentOperator) -> Option<PrimitiveOperator> {
    match operator {
        AssignmentOperator::Addition => Some(PrimitiveOperator::JsAdd),
        AssignmentOperator::Subtraction => Some(PrimitiveOperator::Subtract),
        AssignmentOperator::Multiplication => Some(PrimitiveOperator::Multiply),
        AssignmentOperator::Division => Some(PrimitiveOperator::Divide),
        AssignmentOperator::Remainder => Some(PrimitiveOperator::Remainder),
        AssignmentOperator::ShiftLeft => Some(PrimitiveOperator::ShiftLeft),
        AssignmentOperator::ShiftRight => Some(PrimitiveOperator::ShiftRight),
        AssignmentOperator::ShiftRightZeroFill => Some(PrimitiveOperator::ShiftRightZeroFill),
        AssignmentOperator::BitwiseOR => Some(PrimitiveOperator::BitwiseOr),
        AssignmentOperator::BitwiseXOR => Some(PrimitiveOperator::BitwiseXor),
        AssignmentOperator::BitwiseAnd => Some(PrimitiveOperator::BitwiseAnd),
        _ => None,
    }
}
fn primitive_binary(operator: BinaryOperator) -> Option<PrimitiveOperator> {
    match operator {
        BinaryOperator::Addition => Some(PrimitiveOperator::JsAdd),
        BinaryOperator::Subtraction => Some(PrimitiveOperator::Subtract),
        BinaryOperator::Multiplication => Some(PrimitiveOperator::Multiply),
        BinaryOperator::Division => Some(PrimitiveOperator::Divide),
        BinaryOperator::Remainder => Some(PrimitiveOperator::Remainder),
        BinaryOperator::ShiftLeft => Some(PrimitiveOperator::ShiftLeft),
        BinaryOperator::ShiftRight => Some(PrimitiveOperator::ShiftRight),
        BinaryOperator::ShiftRightZeroFill => Some(PrimitiveOperator::ShiftRightZeroFill),
        BinaryOperator::BitwiseOR => Some(PrimitiveOperator::BitwiseOr),
        BinaryOperator::BitwiseXOR => Some(PrimitiveOperator::BitwiseXor),
        BinaryOperator::BitwiseAnd => Some(PrimitiveOperator::BitwiseAnd),
        BinaryOperator::Exponential => Some(PrimitiveOperator::Exponent),
        BinaryOperator::Equality => Some(PrimitiveOperator::Equal),
        BinaryOperator::Inequality => Some(PrimitiveOperator::NotEqual),
        BinaryOperator::StrictEquality => Some(PrimitiveOperator::StrictEqual),
        BinaryOperator::StrictInequality => Some(PrimitiveOperator::StrictNotEqual),
        BinaryOperator::LessThan => Some(PrimitiveOperator::LessThan),
        BinaryOperator::LessEqualThan => Some(PrimitiveOperator::LessThanOrEqual),
        BinaryOperator::GreaterThan => Some(PrimitiveOperator::GreaterThan),
        BinaryOperator::GreaterEqualThan => Some(PrimitiveOperator::GreaterThanOrEqual),
        BinaryOperator::In => Some(PrimitiveOperator::In),
        BinaryOperator::Instanceof => Some(PrimitiveOperator::InstanceOf),
    }
}
fn primitive_unary(operator: UnaryOperator) -> Option<PrimitiveOperator> {
    match operator {
        UnaryOperator::UnaryPlus => Some(PrimitiveOperator::UnaryPlus),
        UnaryOperator::UnaryNegation => Some(PrimitiveOperator::UnaryMinus),
        UnaryOperator::BitwiseNot => Some(PrimitiveOperator::BitwiseNot),
        UnaryOperator::LogicalNot => Some(PrimitiveOperator::LogicalNot),
        UnaryOperator::Typeof => Some(PrimitiveOperator::Typeof),
        _ => None,
    }
}
pub(super) fn span_record(span: Span) -> SpanRecord {
    SpanRecord {
        start: span.start,
        end: span.end,
    }
}
