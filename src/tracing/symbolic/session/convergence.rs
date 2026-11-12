use super::*;

impl TraceSession {
    /// What each side of a join knew where it gave the variable `node` its
    /// value (see [`JoinArm`]).
    pub(super) fn note_join_arms(
        &mut self,
        node: Option<TraceNodeId>,
        a: &TraceContext,
        b: &TraceContext,
    ) {
        let executor = &mut self.executor;
        let Some(node) = node else {
            return;
        };
        let Some(arms) = executor.joins.get(&node).cloned() else {
            return;
        };
        if arms.len() != 2 || executor.join_arms.contains_key(&node) {
            return;
        }
        let recorded = arms
            .iter()
            .zip([a, b])
            .map(|(&value, context)| {
                let falsy = match predicate::test_key(&executor.arena, &executor.aliases, value) {
                    Some((key, negated)) => context
                        .branch_facts
                        .get(&key)
                        .is_some_and(|truthy| !(truthy ^ negated)),
                    None => false,
                } || matches!(
                    executor
                        .arena
                        .nodes
                        .get(value as usize)
                        .map(|node| &node.kind),
                    Some(
                        TraceNodeKind::Boolean(false)
                            | TraceNodeKind::Null
                            | TraceNodeKind::Undefined
                    )
                );
                JoinArm {
                    value,
                    falsy,
                    branch_facts: context.branch_facts.clone(),
                    type_facts: context.type_facts.clone(),
                }
            })
            .collect();
        executor.join_arms.insert(node, recorded);
    }

    /// The side a test of the join variable `test` (possibly negated) came
    /// from where it is truthy (`truthy`): when only one side gave it a value
    /// that may be truthy, the path came that way and knows what that side
    /// knew there, and that the value was truthy (an `&&` chain's tests).
    pub(super) fn learn_from_join(
        executor: &Executor,
        context: &mut TraceContext,
        test: TraceNodeId,
        truthy: bool,
    ) {
        let mut node = test;
        let mut negated = false;
        for _ in 0..64 {
            if let Some(&next) = executor.aliases.get(&node) {
                node = next;
                continue;
            }
            match executor
                .arena
                .nodes
                .get(node as usize)
                .map(|node| &node.kind)
            {
                Some(TraceNodeKind::Unary {
                    operator: PrimitiveOperator::LogicalNot,
                    value,
                }) => {
                    negated = !negated;
                    node = *value;
                }
                _ => break,
            }
        }
        if truthy == negated {
            return;
        }
        let Some(arms) = executor.join_arms.get(&node) else {
            return;
        };
        let mut open = arms.iter().filter(|arm| !arm.falsy);
        let (Some(arm), None) = (open.next(), open.next()) else {
            return;
        };
        // Values are never reassigned: facts about them still hold. Facts
        // about cells and variables may not.
        for (key, &known) in arm.branch_facts.iter() {
            Arc::make_mut(&mut context.branch_facts)
                .entry(key.clone())
                .or_insert(known);
        }
        for (key, &types) in arm.type_facts.iter() {
            if key.starts_with("variable ") {
                continue;
            }
            let entry = Arc::make_mut(&mut context.type_facts)
                .entry(key.clone())
                .or_insert(types);
            *entry &= types;
        }
        if let Some((key, negated)) =
            predicate::test_key(&executor.arena, &executor.aliases, arm.value)
        {
            Arc::make_mut(&mut context.branch_facts).insert(key, !negated);
        }
        predicate::learn_type(
            &executor.arena,
            &executor.aliases,
            &executor.call_returns,
            &executor.concrete_reads,
            &mut context.type_facts,
            arm.value,
            true,
        );
        // The side's value may itself be such a join.
        let value = arm.value;
        Self::learn_from_join(executor, context, value, true);
    }

    /// The merged value of one cell, or `None` when the taken branch's cell
    /// can stay as is.
    pub(super) fn merge_cell(
        &mut self,
        fork: TraceFork,
        left: StoredValue,
        right: StoredValue,
        provenance: &TraceProvenance,
    ) -> Result<Option<StoredValue>, TraceJoinError> {
        if left.node.is_some() && left.node == right.node {
            return Ok(None);
        }
        // Two reads of the same variable are the same value.
        let variable = |node: Option<TraceNodeId>| match node
            .and_then(|node| self.executor.arena.nodes.get(node as usize))
            .map(|node| &node.kind)
        {
            Some(TraceNodeKind::Identifier { name }) => Some(name.clone()),
            _ => None,
        };
        if variable(left.node).is_some() && variable(left.node) == variable(right.node) {
            let same = match (&left.fact, &right.fact) {
                (Some(left), Some(right)) => same_fact(left, right),
                (None, None) => true,
                _ => false,
            };
            if same {
                return Ok(None);
            }
            // The variable holds a value known on only one side.
            return Ok(Some(StoredValue {
                value: SymbolicValue::Computed {
                    node: left.node.expect("a variable read has a node"),
                },
                fact: None,
                node: left.node,
            }));
        }
        if let (Some(left_fact), Some(right_fact)) = (&left.fact, &right.fact) {
            if same_fact(left_fact, right_fact) {
                // A concrete value is the same on both paths; rebuild it
                // outside the arms, where their bindings are not in scope.
                return Ok(Some(stored_value(left_fact.clone())));
            }
        }
        let executor = &mut self.executor;
        let name = format!("trace_value_{}", executor.binding_serial);
        executor.binding_serial += 1;
        let saved_block = executor.block;
        let mut assigned = Vec::with_capacity(2);
        for (arm, stored) in [(fork.arms.0, left), (fork.arms.1, right)] {
            executor.block = Some(BlockTarget {
                statement: fork.statement,
                arm,
            });
            // A known value is written as itself: its binding may be one the
            // assignment is not in scope of.
            let node = match (stored.node, stored.fact) {
                (Some(node), None) => node,
                (_, fact) => {
                    executor
                        .value_node(fact.unwrap_or(stored.value), provenance)
                        .map_err(|_| TraceJoinError("a merged value cannot be built".into()))?
                        .node
                }
            };
            assigned.push(node);
            executor.statement(
                TraceStatementKind::Assign {
                    name: name.clone(),
                    value: node,
                },
                provenance.clone(),
            );
        }
        executor.block = saved_block;
        if let TraceStatementKind::Declare { names } =
            &mut executor.arena.statements[fork.declare as usize].kind
        {
            names.push(name.clone());
        }
        let node = executor
            .identifier(&name, provenance)
            .map_err(|_| TraceJoinError("the merged variable cannot be built".into()))?;
        executor.joins.insert(node, assigned);
        Ok(Some(StoredValue {
            value: SymbolicValue::Computed { node },
            fact: None,
            node: Some(node),
        }))
    }

    pub(super) fn with_return_semantics(
        context: TraceContext,
        limits: TraceLimits,
        terminal: bool,
    ) -> Self {
        let mut executor = Executor::new(context, limits);
        executor.handler_return_terminal = terminal;
        Self { executor }
    }

    pub(super) fn check_initial_context(&mut self, limits: TraceLimits) {
        if let Some(reason) = self.executor.context_computed_value() {
            self.executor
                .unsupported(TraceStopCode::ComputedInput, reason, None);
        } else if self.executor.context.stack.len() > limits.max_stack_length {
            self.executor.budget(
                TraceStopCode::InitialStackBudget,
                "initial stack exceeds configured limit",
                None,
            );
        } else if self.executor.context.memory.len() > limits.max_memory_slots {
            self.executor.budget(
                TraceStopCode::InitialMemoryBudget,
                "initial memory exceeds configured limit",
                None,
            );
        }
    }
}
