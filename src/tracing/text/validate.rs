use super::*;

pub(super) fn validate_text(
    value: &str,
    field: &'static str,
    limits: &Limits,
) -> Result<(), TraceError> {
    if value.len() > limits.max_field_bytes {
        Err(TraceError::FieldTooLong {
            field,
            limit: limits.max_field_bytes,
        })
    } else {
        Ok(())
    }
}

/// Map values in ascending key order, so the first reported error does not
/// depend on `HashMap` iteration order.
pub(super) fn in_key_order<K: Ord, V>(map: &HashMap<K, V>) -> Vec<&V> {
    let mut entries: Vec<(&K, &V)> = map.iter().collect();
    entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
    entries.into_iter().map(|(_, value)| value).collect()
}

pub(super) fn validate_scope_graph(scopes: &HashMap<ScopeId, &Scope>) -> Result<(), TraceError> {
    let ordered = in_key_order(scopes);
    let mut state: HashMap<ScopeId, u8> = HashMap::with_capacity(scopes.len());
    for scope in &ordered {
        if scope.id == 0 {
            if let Some(parent) = scope.parent {
                return Err(TraceError::InvalidScopeParent { scope: 0, parent });
            }
        }
        if let Some(parent) = scope.parent {
            if !scopes.contains_key(&parent) {
                return Err(TraceError::InvalidScopeParent {
                    scope: scope.id,
                    parent,
                });
            }
        }
    }
    for scope in &ordered {
        if state.get(&scope.id).copied().unwrap_or(0) == 2 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(scope.id);
        while let Some(id) = current {
            match state.get(&id).copied().unwrap_or(0) {
                1 => return Err(TraceError::ScopeCycle { scope: id }),
                2 => break,
                _ => {
                    state.insert(id, 1);
                    path.push(id);
                    current = scopes.get(&id).and_then(|entry| entry.parent);
                }
            }
        }
        for id in path {
            state.insert(id, 2);
        }
    }
    Ok(())
}

pub(super) fn validate_value(value: &Value, limits: &Limits) -> Result<(), TraceError> {
    match value {
        Value::Unavailable(reason) => validate_text(reason, "value reason", limits),
        Value::String(value) | Value::Reference(value) => validate_text(value, "value", limits),
        Value::Preview {
            display, full_ref, ..
        } => {
            validate_text(display, "value preview", limits)?;
            if let Some(reference) = full_ref {
                validate_text(reference, "value reference", limits)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn validate_event_fields(event: &TraceEvent, limits: &Limits) -> Result<(), TraceError> {
    let text = |value: &str, field| validate_text(value, field, limits);
    match &event.kind {
        EventKind::Input { label, value } => {
            text(label, "input label")?;
            validate_value(value, limits)?;
        }
        EventKind::Read { expression, value } => {
            text(expression, "expression")?;
            validate_value(value, limits)?;
        }
        EventKind::Compute { expression, result } => {
            text(expression, "expression")?;
            validate_value(result, limits)?;
        }
        EventKind::Test { expression, result } => {
            text(expression, "expression")?;
            match result {
                TestResult::Known { reason, .. } | TestResult::Unknown { reason } => {
                    text(reason, "test reason")?
                }
            }
        }
        EventKind::BranchStart { label, status } => {
            text(label, "branch label")?;
            match status {
                BranchStatus::Observed { alternate, .. } => text(alternate, "alternate branch")?,
                BranchStatus::Assumed { assumption, .. } => text(assumption, "assumption")?,
                BranchStatus::ProvenDead {
                    dead_edge,
                    proof,
                    evidence_ref,
                } => {
                    text(dead_edge, "dead edge")?;
                    text(proof, "proof")?;
                    text(evidence_ref, "proof reference")?;
                }
                BranchStatus::Suspected { target, reason, .. } => {
                    text(target, "target")?;
                    text(reason, "suspicion reason")?;
                }
            }
        }
        EventKind::BranchEnd { exit } => {
            if let BranchExit::Stopped(reason) = exit {
                text(reason, "branch stop")?;
            }
        }
        EventKind::CallStart {
            callee, capture, ..
        } => {
            text(callee, "callee")?;
            if let CaptureClaim::CapturedCell {
                cell,
                identity_evidence,
            } = capture
            {
                text(cell, "capture cell")?;
                text(identity_evidence, "capture identity evidence")?;
            }
        }
        EventKind::CallEnd { completion } => match completion {
            CallCompletion::Returned(value) => validate_value(value, limits)?,
            CallCompletion::Threw(error) => text(error, "call error")?,
            CallCompletion::Stopped { reason } => text(reason, "call stop reason")?,
        },
        EventKind::Resume { result, .. } => validate_value(result, limits)?,
        EventKind::Change {
            target,
            before,
            after,
        } => {
            text(target, "change target")?;
            validate_value(before, limits)?;
            validate_value(after, limits)?;
        }
        EventKind::Return { value, .. } => validate_value(value, limits)?,
        EventKind::Throw { error, destination } => {
            text(error, "throw error")?;
            text(destination, "throw destination")?;
        }
        EventKind::CatchStart { label } => text(label, "catch label")?,
        EventKind::FinallyStart { pending } => validate_pending(pending, limits)?,
        EventKind::FinallyEnd { completion } => validate_finally_completion(completion, limits)?,
        EventKind::Capture {
            thread,
            cell,
            identity_evidence,
        } => {
            text(thread, "capture thread")?;
            text(cell, "capture cell")?;
            text(identity_evidence, "capture identity evidence")?;
        }
        EventKind::OpaqueStop { reason } => text(reason, "opaque reason")?,
        EventKind::LoopStart { label } => text(label, "loop label")?,
        EventKind::LoopExit {
            expression, reason, ..
        } => {
            text(expression, "loop expression")?;
            text(reason, "loop reason")?;
        }
        EventKind::LoopEnd { exit } => text(exit, "loop exit")?,
        EventKind::NoUnconditionalResult { reason } => text(reason, "outcome reason")?,
        EventKind::IterationStart { .. } | EventKind::IterationEnd | EventKind::CatchEnd => {}
    }
    match &event.evidence {
        Evidence::Assumed { label, .. } => text(label, "evidence label")?,
        Evidence::Observed | Evidence::StaticDerived => {}
    }
    Ok(())
}

pub(super) fn validate_provenance(
    event: EventId,
    provenance: &Provenance,
    limits: &Limits,
) -> Result<(), TraceError> {
    if let Some(value) = &provenance.handler_key {
        validate_text(value, "handler key", limits)?;
    }
    if let Some(source) = &provenance.source {
        validate_text(&source.artifact, "source artifact", limits)?;
        if source.start > source.end {
            return Err(TraceError::UnsupportedEvidence {
                event,
                reason: "source span starts after it ends".into(),
            });
        }
    }
    if let Some(value) = &provenance.ir_revision {
        validate_text(value, "IR revision", limits)?;
    }
    if let Some(value) = &provenance.ir_op {
        validate_text(value, "IR op reference", limits)?;
    }
    Ok(())
}

pub(super) fn validate_pending(
    value: &PendingCompletion,
    limits: &Limits,
) -> Result<(), TraceError> {
    match value {
        PendingCompletion::Return(value) => validate_value(value, limits),
        PendingCompletion::Throw(error) => validate_text(error, "pending error", limits),
    }
}

pub(super) fn validate_finally_completion(
    value: &FinallyCompletion,
    limits: &Limits,
) -> Result<(), TraceError> {
    match value {
        FinallyCompletion::Completed(value) => validate_pending(value, limits),
        FinallyCompletion::ReplacedByReturn(value) => validate_value(value, limits),
        FinallyCompletion::ReplacedByThrow(error) => validate_text(error, "finally error", limits),
    }
}

pub(super) fn validate_references(ids: &HashMap<EventId, &TraceEvent>) -> Result<(), TraceError> {
    for event in in_key_order(ids) {
        if let EventKind::BranchStart {
            status: BranchStatus::Assumed { .. },
            ..
        } = &event.kind
        {
            let Evidence::Assumed { branch, .. } = event.evidence else {
                return Err(TraceError::UnsupportedEvidence {
                    event: event.id,
                    reason: "an assumed branch must not be marked observed".into(),
                });
            };
            if branch != event.id {
                return Err(TraceError::UnknownEventReference {
                    event: event.id,
                    reference: branch,
                });
            }
        }
        if let Evidence::Assumed { branch, .. } = event.evidence {
            match ids.get(&branch).map(|entry| &entry.kind) {
                Some(EventKind::BranchStart { .. }) => {}
                Some(_) | None => {
                    return Err(TraceError::UnknownEventReference {
                        event: event.id,
                        reference: branch,
                    })
                }
            }
        }
        if let EventKind::BranchStart {
            status:
                BranchStatus::ProvenDead {
                    proof,
                    evidence_ref,
                    ..
                },
            ..
        } = &event.kind
        {
            if proof.trim().is_empty() || evidence_ref.trim().is_empty() {
                return Err(TraceError::MissingProofEvidence { event: event.id });
            }
        }
        if let EventKind::CallStart {
            capture:
                CaptureClaim::CapturedCell {
                    identity_evidence, ..
                },
            ..
        }
        | EventKind::Capture {
            identity_evidence, ..
        } = &event.kind
        {
            if identity_evidence.trim().is_empty() {
                return Err(TraceError::UnsupportedEvidence {
                    event: event.id,
                    reason: "shared-cell claims require identity evidence".into(),
                });
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub(super) enum Terminal {
    Returned(Value),
    Thrown(String),
    Stopped(String),
}

#[derive(Clone, Debug)]
pub(super) enum Awaiting {
    Returned(Value),
    Thrown(String),
}

#[derive(Clone, Debug)]
pub(super) enum ControlFrame {
    Branch {
        event: EventId,
        assumed: bool,
        parent_terminal: Option<Terminal>,
    },
    Loop {
        next_ordinal: u32,
        iteration: Option<u32>,
        exit: Option<bool>,
    },
    Catch,
    Finally {
        pending: PendingCompletion,
    },
}

#[derive(Clone, Debug)]
pub(super) struct InvocationFrame {
    pub(super) scope: ScopeId,
    pub(super) call_start: Option<EventId>,
    pub(super) controls: Vec<ControlFrame>,
    pub(super) terminal: Option<Terminal>,
    pub(super) awaiting: Option<Awaiting>,
    pub(super) pending_finally: Option<PendingCompletion>,
    pub(super) assumed_branches: Vec<EventId>,
}

pub(super) fn same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Undefined, Value::Undefined)
        | (Value::Null, Value::Null)
        | (Value::Uninitialized, Value::Uninitialized) => true,
        (Value::Unavailable(a), Value::Unavailable(b))
        | (Value::String(a), Value::String(b))
        | (Value::Reference(a), Value::Reference(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::I32(a), Value::I32(b)) => a == b,
        (Value::I64(a), Value::I64(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a.to_bits() == b.to_bits(),
        (
            Value::Preview {
                display: ad,
                full_ref: ar,
                size_bytes: asize,
            },
            Value::Preview {
                display: bd,
                full_ref: br,
                size_bytes: bsize,
            },
        ) => ad == bd && ar == br && asize == bsize,
        _ => false,
    }
}

pub(super) fn same_pending(left: &PendingCompletion, right: &PendingCompletion) -> bool {
    match (left, right) {
        (PendingCompletion::Return(a), PendingCompletion::Return(b)) => same_value(a, b),
        (PendingCompletion::Throw(a), PendingCompletion::Throw(b)) => a == b,
        _ => false,
    }
}

pub(super) fn invalid_call(event: EventId, reason: impl Into<String>) -> TraceError {
    TraceError::InvalidCallStructure {
        event,
        reason: reason.into(),
    }
}

pub(super) fn invalid_branch(event: EventId, reason: impl Into<String>) -> TraceError {
    TraceError::InvalidBranchStructure {
        event,
        reason: reason.into(),
    }
}

pub(super) fn invalid_loop(event: EventId, reason: impl Into<String>) -> TraceError {
    TraceError::InvalidLoopStructure {
        event,
        reason: reason.into(),
    }
}

pub(super) fn invalid_finally(event: EventId, reason: impl Into<String>) -> TraceError {
    TraceError::InvalidFinallyStructure {
        event,
        reason: reason.into(),
    }
}

pub(super) fn ensure_normal(frame: &InvocationFrame, event: EventId) -> Result<(), TraceError> {
    if frame.awaiting.is_some() {
        return Err(invalid_call(
            event,
            "caller must resume or handle the child completion first",
        ));
    }
    if frame.terminal.is_some() {
        return Err(invalid_call(
            event,
            "ordinary action follows a terminal path completion",
        ));
    }
    if frame.pending_finally.is_some() {
        return Err(invalid_finally(
            event,
            "ordinary action follows a pending finally completion",
        ));
    }
    Ok(())
}

pub(super) fn validate_event_evidence(
    event: &TraceEvent,
    frame: &InvocationFrame,
) -> Result<(), TraceError> {
    if let EventKind::BranchStart { status, .. } = &event.kind {
        if matches!(status, BranchStatus::Observed { .. })
            && !matches!(event.evidence, Evidence::Observed)
        {
            return Err(TraceError::UnsupportedEvidence {
                event: event.id,
                reason: "observed branch status requires observed evidence".into(),
            });
        }
        match (&event.evidence, status) {
            (
                Evidence::Assumed { branch, .. },
                BranchStatus::Assumed { .. } | BranchStatus::Suspected { .. },
            ) if *branch == event.id => {
                // The opening event may cite itself. It is not yet an ancestor;
                // subsequent events must cite this branch or an outer one.
                return Ok(());
            }
            (Evidence::Assumed { .. }, BranchStatus::Assumed { .. }) => {
                return Err(TraceError::UnsupportedEvidence {
                    event: event.id,
                    reason: "assumed branch start must carry self-referencing assumed evidence"
                        .into(),
                });
            }
            (Evidence::Assumed { .. }, _) => {
                return Err(TraceError::UnsupportedEvidence {
                    event: event.id,
                    reason: "observed/proven branch status cannot use assumed evidence".into(),
                });
            }
            _ => {}
        }
    }

    if frame.assumed_branches.is_empty() {
        if matches!(event.evidence, Evidence::Assumed { .. }) {
            return Err(TraceError::UnknownEventReference {
                event: event.id,
                reference: match event.evidence {
                    Evidence::Assumed { branch, .. } => branch,
                    _ => event.id,
                },
            });
        }
        return Ok(());
    }

    match &event.evidence {
        Evidence::Assumed { branch, .. } if frame.assumed_branches.contains(branch) => Ok(()),
        Evidence::Assumed { branch, .. } => Err(TraceError::UnknownEventReference {
            event: event.id,
            reference: *branch,
        }),
        Evidence::Observed | Evidence::StaticDerived => Err(TraceError::UnsupportedEvidence {
            event: event.id,
            reason: "observed/derived evidence cannot occur inside an assumed path".into(),
        }),
    }
}

/// Stateful structural validator for a trace event sequence.
///
/// The validator owns the active invocation stack, the set of scopes that have already
/// been entered, and a running nesting `depth`. The depth counter is maintained on every
/// frame/control push and pop so each event costs O(1) instead of re-summing the stack.
/// All stack access is fallible (`active`, `active_ref`, `active_with`), so validation
/// contains no `expect` calls whose safety depends on non-local reasoning.
pub(super) struct StructureValidator<'a> {
    pub(super) scopes: &'a HashMap<ScopeId, &'a Scope>,
    pub(super) limits: &'a Limits,
    pub(super) invocations: Vec<InvocationFrame>,
    pub(super) started_scopes: HashSet<ScopeId>,
    pub(super) depth: usize,
}

impl<'a> StructureValidator<'a> {
    pub(super) fn new(scopes: &'a HashMap<ScopeId, &'a Scope>, limits: &'a Limits) -> Self {
        let root = InvocationFrame {
            scope: 0,
            call_start: None,
            controls: Vec::new(),
            terminal: None,
            awaiting: None,
            pending_finally: None,
            assumed_branches: Vec::new(),
        };
        let mut started_scopes = HashSet::with_capacity(scopes.len());
        started_scopes.insert(0);
        Self {
            scopes,
            limits,
            invocations: vec![root],
            started_scopes,
            depth: 0,
        }
    }

    pub(super) fn validate(
        mut self,
        events: &[TraceEvent],
        ids: &HashMap<EventId, &TraceEvent>,
    ) -> Result<(), TraceError> {
        validate_references(ids)?;
        for event in events {
            self.validate_event(event)?;
            #[cfg(debug_assertions)]
            debug_assert_eq!(
                self.depth,
                active_depth(&self.invocations),
                "incremental depth drifted while validating event {}",
                event.id
            );
        }
        self.finish()
    }

    pub(super) fn validate_event(&mut self, event: &TraceEvent) -> Result<(), TraceError> {
        let frame = self.active_ref(event.id)?;
        let active_scope = frame.scope;
        if event.scope != active_scope {
            return Err(invalid_call(
                event.id,
                format!(
                    "event scope {} is not the active invocation scope {}",
                    event.scope, active_scope
                ),
            ));
        }
        validate_event_evidence(event, frame)?;

        let opens_frame = matches!(
            &event.kind,
            EventKind::CallStart { .. }
                | EventKind::BranchStart { .. }
                | EventKind::LoopStart { .. }
                | EventKind::CatchStart { .. }
                | EventKind::FinallyStart { .. }
        );
        if opens_frame && self.depth.saturating_add(1) > self.limits.max_nesting {
            return Err(TraceError::LimitExceeded {
                what: "nesting depth",
                limit: self.limits.max_nesting,
            });
        }

        match &event.kind {
            EventKind::Input { .. }
            | EventKind::Read { .. }
            | EventKind::Compute { .. }
            | EventKind::Test { .. }
            | EventKind::Change { .. }
            | EventKind::Capture { .. }
            | EventKind::NoUnconditionalResult { .. } => self.on_value_event(event.id),
            EventKind::CallStart {
                child_scope,
                invocation,
                ..
            } => self.on_call_start(event, *child_scope, *invocation),
            EventKind::CallEnd { completion } => self.on_call_end(event.id, completion),
            EventKind::Resume {
                caller_scope,
                result,
                ..
            } => self.on_resume(event.id, *caller_scope, result),
            EventKind::Return { value, disposition } => {
                self.on_return(event.id, value, disposition)
            }
            EventKind::Throw { error, .. } => self.on_throw(event.id, error),
            EventKind::CatchStart { .. } => self.on_catch_start(event.id),
            EventKind::CatchEnd => self.on_catch_end(event.id),
            EventKind::FinallyStart { pending } => self.on_finally_start(event.id, pending),
            EventKind::FinallyEnd { completion } => self.on_finally_end(event.id, completion),
            EventKind::BranchStart { status, .. } => self.on_branch_start(event, status),
            EventKind::BranchEnd { exit } => self.on_branch_end(event.id, exit),
            EventKind::LoopStart { .. } => self.on_loop_start(event.id),
            EventKind::IterationStart { ordinal } => self.on_iteration_start(event.id, *ordinal),
            EventKind::IterationEnd => self.on_iteration_end(event.id),
            EventKind::LoopExit { result, .. } => self.on_loop_exit(event.id, *result),
            EventKind::LoopEnd { .. } => self.on_loop_end(event.id),
            EventKind::OpaqueStop { reason } => self.on_opaque_stop(event.id, reason),
        }?;

        // Every push/pop above updated `self.depth`. The pre-push check above already
        // rejects a nesting limit violation, so this only re-establishes the same
        // invariant the original per-event sum enforced after each event.
        if self.depth > self.limits.max_nesting {
            return Err(TraceError::LimitExceeded {
                what: "nesting depth",
                limit: self.limits.max_nesting,
            });
        }
        Ok(())
    }

    /// The active invocation frame, or the same error the validation loop used to
    /// return when the stack was empty. Stack emptiness is unreachable for inputs that
    /// pass the caller-facing checks, but the accessor reports it explicitly instead of
    /// relying on that non-local reasoning.
    pub(super) fn active(&mut self, event: EventId) -> Result<&mut InvocationFrame, TraceError> {
        self.active_with(
            event,
            "event occurs after the active invocation stack ended",
        )
    }

    pub(super) fn active_ref(&self, event: EventId) -> Result<&InvocationFrame, TraceError> {
        self.invocations.last().ok_or_else(|| {
            invalid_call(
                event,
                "event occurs after the active invocation stack ended",
            )
        })
    }

    pub(super) fn active_with(
        &mut self,
        event: EventId,
        reason: &'static str,
    ) -> Result<&mut InvocationFrame, TraceError> {
        match self.invocations.last_mut() {
            Some(frame) => Ok(frame),
            None => Err(invalid_call(event, reason)),
        }
    }

    pub(super) fn on_value_event(&mut self, event: EventId) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        ensure_normal(frame, event)
    }

    pub(super) fn on_call_start(
        &mut self,
        event: &TraceEvent,
        child_scope: ScopeId,
        invocation: u32,
    ) -> Result<(), TraceError> {
        let frame = self.active(event.id)?;
        ensure_normal(frame, event.id)?;
        let inherited_branches = frame.assumed_branches.clone();
        let child = self
            .scopes
            .get(&child_scope)
            .ok_or_else(|| invalid_call(event.id, "child scope is missing"))?;
        if child.parent != Some(event.scope) {
            return Err(invalid_call(
                event.id,
                "child scope parent does not match caller scope",
            ));
        }
        if child.call != invocation {
            return Err(invalid_call(
                event.id,
                "child scope call identity does not match the supplied invocation",
            ));
        }
        if !self.started_scopes.insert(child_scope) {
            return Err(invalid_call(event.id, "child scope was already invoked"));
        }
        self.invocations.push(InvocationFrame {
            scope: child_scope,
            call_start: Some(event.id),
            controls: Vec::new(),
            terminal: None,
            awaiting: None,
            pending_finally: None,
            assumed_branches: inherited_branches,
        });
        self.depth += 1;
        Ok(())
    }

    pub(super) fn on_call_end(
        &mut self,
        event: EventId,
        completion: &CallCompletion,
    ) -> Result<(), TraceError> {
        let Some(frame) = self.invocations.last() else {
            return Err(invalid_call(event, "call end has no active invocation"));
        };
        if frame.call_start.is_none() {
            return Err(invalid_call(event, "root invocation cannot have CallEnd"));
        }
        if !frame.controls.is_empty() {
            return Err(invalid_call(
                event,
                "call ends while a child control frame is open",
            ));
        }
        let Some(terminal) = frame.terminal.clone() else {
            return Err(invalid_call(
                event,
                "call ends without a child return, throw, or stop",
            ));
        };
        let agrees = match (completion, &terminal) {
            (CallCompletion::Returned(expected), Terminal::Returned(actual)) => {
                same_value(expected, actual)
            }
            (CallCompletion::Threw(expected), Terminal::Thrown(actual)) => expected == actual,
            (CallCompletion::Stopped { reason: expected }, Terminal::Stopped(actual)) => {
                expected == actual
            }
            _ => false,
        };
        if !agrees {
            return Err(invalid_call(
                event,
                "CallEnd completion disagrees with the child terminal record",
            ));
        }
        let child_terminal = terminal;
        self.invocations.pop();
        self.depth -= 1;
        let Some(parent) = self.invocations.last_mut() else {
            return Err(invalid_call(event, "child call has no caller frame"));
        };
        if parent.awaiting.is_some() || parent.terminal.is_some() {
            return Err(invalid_call(
                event,
                "caller is not available for this completion",
            ));
        }
        match child_terminal {
            Terminal::Returned(value) => parent.awaiting = Some(Awaiting::Returned(value)),
            Terminal::Thrown(error) => parent.awaiting = Some(Awaiting::Thrown(error)),
            Terminal::Stopped(reason) => parent.terminal = Some(Terminal::Stopped(reason)),
        }
        Ok(())
    }

    pub(super) fn on_resume(
        &mut self,
        event: EventId,
        caller_scope: ScopeId,
        result: &Value,
    ) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        if caller_scope != frame.scope {
            return Err(invalid_call(
                event,
                "resume caller scope differs from event scope",
            ));
        }
        match frame.awaiting.take() {
            Some(Awaiting::Returned(expected)) if same_value(&expected, result) => Ok(()),
            Some(Awaiting::Returned(_)) => Err(invalid_call(
                event,
                "Resume result differs from child return",
            )),
            Some(Awaiting::Thrown(_)) => Err(invalid_call(
                event,
                "a thrown child must be caught or propagated, not resumed normally",
            )),
            None => Err(invalid_call(
                event,
                "Resume has no consumable child completion",
            )),
        }
    }

    pub(super) fn on_return(
        &mut self,
        event: EventId,
        value: &Value,
        disposition: &ReturnDisposition,
    ) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        ensure_normal(frame, event)?;
        if matches!(frame.controls.last(), Some(ControlFrame::Finally { .. })) {
            return Err(invalid_finally(
                event,
                "return inside finally must use FinallyEnd replacement",
            ));
        }
        match disposition {
            ReturnDisposition::Complete => {
                frame.terminal = Some(Terminal::Returned(value.clone()));
            }
            ReturnDisposition::PendingFinally => {
                frame.pending_finally = Some(PendingCompletion::Return(value.clone()));
            }
        }
        Ok(())
    }

    pub(super) fn on_throw(&mut self, event: EventId, error: &str) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        if frame.pending_finally.is_some()
            || matches!(frame.controls.last(), Some(ControlFrame::Finally { .. }))
        {
            return Err(invalid_finally(
                event,
                "throw inside finally must use FinallyEnd replacement",
            ));
        }
        match frame.awaiting.take() {
            Some(Awaiting::Returned(_)) => {
                return Err(invalid_call(
                    event,
                    "normal child completion must be resumed before a throw",
                ));
            }
            Some(Awaiting::Thrown(expected)) if expected != *error => {
                return Err(invalid_call(
                    event,
                    "propagated throw differs from child throw",
                ));
            }
            Some(Awaiting::Thrown(_)) | None => {}
        }
        if frame.terminal.is_some() {
            return Err(invalid_call(
                event,
                "throw follows a terminal path completion",
            ));
        }
        frame.terminal = Some(Terminal::Thrown(error.to_owned()));
        Ok(())
    }

    pub(super) fn on_catch_start(&mut self, event: EventId) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        if matches!(frame.controls.last(), Some(ControlFrame::Finally { .. })) {
            return Err(invalid_finally(
                event,
                "nested catch inside finally is unsupported",
            ));
        }
        let pending_throw = match frame.awaiting.take() {
            Some(Awaiting::Thrown(error)) => error,
            Some(Awaiting::Returned(_)) => {
                return Err(invalid_call(
                    event,
                    "CatchStart cannot consume a normal child return",
                ));
            }
            None => match frame.terminal.take() {
                Some(Terminal::Thrown(error)) => error,
                Some(other) => {
                    frame.terminal = Some(other);
                    return Err(invalid_call(event, "CatchStart has no pending throw"));
                }
                None => return Err(invalid_call(event, "CatchStart has no pending throw")),
            },
        };
        let _ = pending_throw;
        frame.controls.push(ControlFrame::Catch);
        self.depth += 1;
        Ok(())
    }

    pub(super) fn on_catch_end(&mut self, event: EventId) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        match frame.controls.pop() {
            Some(ControlFrame::Catch) if frame.awaiting.is_none() => {
                self.depth -= 1;
                Ok(())
            }
            Some(other) => {
                frame.controls.push(other);
                Err(invalid_call(
                    event,
                    "CatchEnd does not close the innermost control frame",
                ))
            }
            None => Err(invalid_call(event, "CatchEnd has no open catch")),
        }
    }

    pub(super) fn on_finally_start(
        &mut self,
        event: EventId,
        pending: &PendingCompletion,
    ) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        if matches!(frame.controls.last(), Some(ControlFrame::Finally { .. })) {
            return Err(invalid_finally(event, "nested finally is unsupported"));
        }
        let actual = if let Some(actual) = frame.pending_finally.take() {
            actual
        } else if let Some(Terminal::Thrown(error)) = frame.terminal.take() {
            PendingCompletion::Throw(error)
        } else {
            return Err(invalid_finally(
                event,
                "FinallyStart has no pending return or throw",
            ));
        };
        if !same_pending(&actual, pending) {
            return Err(invalid_finally(
                event,
                "FinallyStart differs from pending completion",
            ));
        }
        frame.controls.push(ControlFrame::Finally {
            pending: pending.clone(),
        });
        self.depth += 1;
        Ok(())
    }

    pub(super) fn on_finally_end(
        &mut self,
        event: EventId,
        completion: &FinallyCompletion,
    ) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        let Some(ControlFrame::Finally { pending }) = frame.controls.pop() else {
            return Err(invalid_finally(event, "FinallyEnd has no open finally"));
        };
        if frame.terminal.is_some() || frame.pending_finally.is_some() {
            return Err(invalid_finally(
                event,
                "finally cleanup has an unconsumed completion",
            ));
        }
        if let FinallyCompletion::Completed(actual) = completion {
            if !same_pending(&pending, actual) {
                return Err(invalid_finally(
                    event,
                    "FinallyEnd completion differs from pending",
                ));
            }
        }
        frame.terminal = Some(match completion {
            FinallyCompletion::Completed(PendingCompletion::Return(value))
            | FinallyCompletion::ReplacedByReturn(value) => Terminal::Returned(value.clone()),
            FinallyCompletion::Completed(PendingCompletion::Throw(error))
            | FinallyCompletion::ReplacedByThrow(error) => Terminal::Thrown(error.clone()),
        });
        self.depth -= 1;
        Ok(())
    }

    pub(super) fn on_branch_start(
        &mut self,
        event: &TraceEvent,
        status: &BranchStatus,
    ) -> Result<(), TraceError> {
        let frame = self.active(event.id)?;
        ensure_normal(frame, event.id)?;
        let assumed = matches!(event.evidence, Evidence::Assumed { .. });
        let parent_terminal = frame.terminal.take();
        frame.controls.push(ControlFrame::Branch {
            event: event.id,
            assumed,
            parent_terminal,
        });
        if matches!(status, BranchStatus::Assumed { .. }) && !assumed {
            return Err(invalid_branch(
                event.id,
                "assumed branch status and evidence disagree",
            ));
        }
        if assumed {
            frame.assumed_branches.push(event.id);
        }
        self.depth += 1;
        Ok(())
    }

    pub(super) fn on_branch_end(
        &mut self,
        event: EventId,
        exit: &BranchExit,
    ) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        let Some(ControlFrame::Branch {
            event: branch_event,
            assumed,
            parent_terminal,
        }) = frame.controls.pop()
        else {
            return Err(invalid_branch(
                event,
                "BranchEnd does not close the innermost control frame",
            ));
        };
        let mut terminal = frame.terminal.take();
        let has_pending = frame.pending_finally.is_some();
        match exit {
            BranchExit::ContinueAfterProvenJoin if terminal.is_none() && !has_pending => {}
            BranchExit::ContinueAfterProvenJoin => {
                frame.controls.push(ControlFrame::Branch {
                    event: branch_event,
                    assumed,
                    parent_terminal,
                });
                frame.terminal = terminal;
                return Err(invalid_branch(
                    event,
                    "proven join closes a terminal or pending path",
                ));
            }
            BranchExit::Returned if terminal.is_some() || has_pending => {}
            BranchExit::Returned => {
                frame.controls.push(ControlFrame::Branch {
                    event: branch_event,
                    assumed,
                    parent_terminal,
                });
                return Err(invalid_branch(
                    event,
                    "Returned branch has no recorded completion",
                ));
            }
            BranchExit::Stopped(reason) if terminal.is_none() => {
                terminal = Some(Terminal::Stopped(reason.clone()));
            }
            BranchExit::Stopped(_) if matches!(terminal, Some(Terminal::Stopped(_))) => {}
            BranchExit::Stopped(_) => {
                frame.controls.push(ControlFrame::Branch {
                    event: branch_event,
                    assumed,
                    parent_terminal,
                });
                frame.terminal = terminal;
                return Err(invalid_branch(
                    event,
                    "stopped branch lacks an opaque/stopped path",
                ));
            }
        }
        if assumed {
            if frame.assumed_branches.pop() != Some(branch_event) {
                return Err(invalid_branch(
                    event,
                    "assumed branch ancestry is not innermost",
                ));
            }
            frame.terminal = parent_terminal;
        } else {
            frame.terminal = terminal.or(parent_terminal);
        }
        self.depth -= 1;
        Ok(())
    }

    pub(super) fn on_loop_start(&mut self, event: EventId) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        ensure_normal(frame, event)?;
        frame.controls.push(ControlFrame::Loop {
            next_ordinal: 1,
            iteration: None,
            exit: None,
        });
        self.depth += 1;
        Ok(())
    }

    pub(super) fn on_iteration_start(
        &mut self,
        event: EventId,
        ordinal: u32,
    ) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        ensure_normal(frame, event)?;
        let Some(ControlFrame::Loop {
            next_ordinal,
            iteration,
            ..
        }) = frame.controls.last_mut()
        else {
            return Err(invalid_loop(
                event,
                "IterationStart is not inside the innermost loop",
            ));
        };
        if iteration.is_some() || ordinal != *next_ordinal {
            return Err(invalid_loop(
                event,
                "iteration ordinal is not the next retained iteration",
            ));
        }
        *iteration = Some(ordinal);
        Ok(())
    }

    pub(super) fn on_iteration_end(&mut self, event: EventId) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        let Some(ControlFrame::Loop {
            next_ordinal,
            iteration,
            ..
        }) = frame.controls.last_mut()
        else {
            return Err(invalid_loop(
                event,
                "IterationEnd is not inside the innermost loop",
            ));
        };
        let Some(ordinal) = iteration.take() else {
            return Err(invalid_loop(event, "IterationEnd has no active iteration"));
        };
        *next_ordinal = ordinal + 1;
        Ok(())
    }

    pub(super) fn on_loop_exit(&mut self, event: EventId, result: bool) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        let Some(ControlFrame::Loop {
            iteration, exit, ..
        }) = frame.controls.last_mut()
        else {
            return Err(invalid_loop(
                event,
                "LoopExit is not inside the innermost loop",
            ));
        };
        if iteration.is_some() || exit.is_some() {
            return Err(invalid_loop(
                event,
                "LoopExit is repeated or occurs during an iteration",
            ));
        }
        *exit = Some(result);
        Ok(())
    }

    pub(super) fn on_loop_end(&mut self, event: EventId) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        let Some(ControlFrame::Loop {
            iteration, exit, ..
        }) = frame.controls.pop()
        else {
            return Err(invalid_loop(
                event,
                "LoopEnd does not close the innermost loop",
            ));
        };
        let explicit_abrupt_completion = frame.terminal.is_some() && exit.is_none();
        if iteration.is_some() || (!explicit_abrupt_completion && exit != Some(false)) {
            return Err(invalid_loop(
                event,
                "loop must have a completed iteration and false exit test unless the path has an explicit abrupt completion",
            ));
        }
        self.depth -= 1;
        Ok(())
    }

    pub(super) fn on_opaque_stop(
        &mut self,
        event: EventId,
        reason: &str,
    ) -> Result<(), TraceError> {
        let frame = self.active(event)?;
        ensure_normal(frame, event)?;
        frame.terminal = Some(Terminal::Stopped(reason.to_owned()));
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<(), TraceError> {
        if self.invocations.len() != 1 {
            return Err(invalid_call(
                0,
                "trace ended with an active child invocation",
            ));
        }
        let Some(root) = self.invocations.pop() else {
            return Err(invalid_call(
                0,
                "trace ended with an active child invocation",
            ));
        };
        if !root.controls.is_empty() {
            return Err(invalid_call(0, "trace ended with an open control frame"));
        }
        if root.awaiting.is_some() {
            return Err(invalid_call(
                0,
                "trace ended before caller resumed or handled a child",
            ));
        }
        if root.pending_finally.is_some() {
            return Err(invalid_finally(
                0,
                "trace ended with an unresolved finally completion",
            ));
        }
        Ok(())
    }
}

/// Nesting depth recomputed from the stack. Test/debug-only cross-check for the
/// incrementally maintained [`StructureValidator::depth`] counter.
#[cfg(debug_assertions)]
pub(super) fn active_depth(invocations: &[InvocationFrame]) -> usize {
    invocations.len().saturating_sub(1)
        + invocations
            .iter()
            .map(|frame| frame.controls.len())
            .sum::<usize>()
}

pub(super) fn validate_structure(
    events: &[TraceEvent],
    scopes: &HashMap<ScopeId, &Scope>,
    limits: &Limits,
    ids: &HashMap<EventId, &TraceEvent>,
) -> Result<(), TraceError> {
    StructureValidator::new(scopes, limits).validate(events, ids)
}
