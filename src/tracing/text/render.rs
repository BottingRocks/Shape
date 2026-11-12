use super::*;

pub(super) struct Renderer<'a> {
    pub(super) trace: &'a Trace,
    pub(super) lines: Vec<String>,
    pub(super) map: Vec<LineDetail>,
    pub(super) event_by_id: HashMap<EventId, &'a TraceEvent>,
    pub(super) main_bytes: usize,
    pub(super) scope_by_id: BTreeMap<ScopeId, &'a Scope>,
    pub(super) indent: HashMap<ScopeId, usize>,
    pub(super) pending_finally: HashMap<ScopeId, PendingCompletion>,
}

impl<'a> Renderer<'a> {
    pub(super) fn new(trace: &'a Trace) -> Self {
        Self {
            trace,
            lines: Vec::new(),
            map: Vec::new(),
            event_by_id: trace.events.iter().map(|event| (event.id, event)).collect(),
            main_bytes: 0,
            scope_by_id: trace.scopes.iter().map(|scope| (scope.id, scope)).collect(),
            indent: HashMap::new(),
            pending_finally: HashMap::new(),
        }
    }

    pub(super) fn render(mut self) -> Result<RenderedTrace, TraceError> {
        self.line(
            None,
            (if self.trace.header.illustrative {
                "Illustrative — not recorded execution"
            } else {
                "Trace observation"
            })
            .to_string(),
        )?;
        self.line(
            None,
            format!("sample: {}", escape(&self.trace.header.sample)),
        )?;
        self.line(None, format!("mode: {}", self.trace.header.mode))?;
        self.line(None, format!("scope: {}", escape(&self.trace.header.scope)))?;
        self.line(
            None,
            format!(
                "capture/exploration: {}",
                escape(&self.trace.header.capture)
            ),
        )?;
        self.line(None, String::new())?;
        for event in &self.trace.events {
            self.render_event(event)?;
        }
        let mut text = String::with_capacity(self.main_bytes);
        for line in &self.lines {
            text.push_str(line);
            text.push('\n');
        }
        let details = self.details()?;
        if text.len() + details.len() > self.trace.limits.max_output_bytes {
            return Err(TraceError::OutputLimitExceeded {
                limit: self.trace.limits.max_output_bytes,
            });
        }
        Ok(RenderedTrace {
            text,
            details,
            line_map: self.map,
        })
    }

    pub(super) fn line(
        &mut self,
        event: Option<&TraceEvent>,
        text: String,
    ) -> Result<(), TraceError> {
        let next = self.lines.len() + 1;
        let bytes = self.main_bytes.saturating_add(text.len()).saturating_add(1);
        if bytes > self.trace.limits.max_output_bytes {
            return Err(TraceError::OutputLimitExceeded {
                limit: self.trace.limits.max_output_bytes,
            });
        }
        self.main_bytes = bytes;
        self.lines.push(text);
        if let Some(event) = event {
            let can_extend = self
                .map
                .last()
                .map(|previous| {
                    previous.line_end + 1 == next
                        && previous.event_ids == vec![event.id]
                        && previous.provenance == event.provenance
                })
                .unwrap_or(false);
            if can_extend {
                self.map
                    .last_mut()
                    .expect("map entry just checked")
                    .line_end = next;
            } else {
                self.map.push(LineDetail {
                    line_start: next,
                    line_end: next,
                    event_ids: vec![event.id],
                    provenance: event.provenance.clone(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn render_event(&mut self, event: &TraceEvent) -> Result<(), TraceError> {
        let depth = *self.indent.get(&event.scope).unwrap_or(&0);
        if depth > self.trace.limits.max_nesting {
            return Err(TraceError::LimitExceeded {
                what: "nesting depth",
                limit: self.trace.limits.max_nesting,
            });
        }
        let prefix = "  ".repeat(depth);
        let value = |value: &Value| format_value(value, &self.trace.limits);
        match &event.kind {
            EventKind::Input {
                label,
                value: input,
            } => self.line(
                Some(event),
                format!("{prefix}input: {} = {}", escape(label), value(input)),
            )?,
            EventKind::Read {
                expression,
                value: read,
            } => self.line(
                Some(event),
                format!(
                    "{prefix}read {} -> value = {}",
                    escape(expression),
                    value(read)
                ),
            )?,
            EventKind::Compute { expression, result } => self.line(
                Some(event),
                format!(
                    "{prefix}Compute {}: result = {}",
                    escape(expression),
                    value(result)
                ),
            )?,
            EventKind::Test { expression, result } => match result {
                TestResult::Known {
                    value: result,
                    reason,
                } if expression.contains([' ', '>', '<', '=', '!']) => self.line(
                    Some(event),
                    format!(
                        "{prefix}if {} -> {} ({})",
                        escape(expression),
                        result,
                        escape(reason)
                    ),
                )?,
                TestResult::Known {
                    value: result,
                    reason,
                } => self.line(
                    Some(event),
                    format!(
                        "{prefix}compare {} -> {} ({})",
                        escape(expression),
                        result,
                        escape(reason)
                    ),
                )?,
                TestResult::Unknown { reason } => self.line(
                    Some(event),
                    format!(
                        "{prefix}compare {} -> unknown (reason: {})",
                        escape(expression),
                        escape(reason)
                    ),
                )?,
            },
            EventKind::BranchStart { label, status } => {
                match status {
                    BranchStatus::Observed { outcome, alternate } => self.line(
                        Some(event),
                        format!(
                            "{prefix}Take {} branch (observed; {})",
                            if *outcome { "true" } else { "false" },
                            escape(alternate)
                        ),
                    )?,
                    BranchStatus::Assumed {
                        outcome,
                        assumption,
                    } => self.line(
                        Some(event),
                        format!(
                            "{prefix}exploration: assumption {} -> {} (not observed)",
                            escape(assumption),
                            outcome
                        ),
                    )?,
                    BranchStatus::ProvenDead {
                        dead_edge, proof, ..
                    } => self.line(
                        Some(event),
                        format!(
                            "{prefix}Take {} branch; {} ruled out here (reason: {})",
                            escape(label),
                            escape(dead_edge),
                            escape(proof)
                        ),
                    )?,
                    BranchStatus::Suspected {
                        outcome,
                        target,
                        reason,
                    } => {
                        self.line(
                            Some(event),
                            format!(
                                "{prefix}Assume {} branch (not observed; suspected decoy — unconfirmed)",
                                if *outcome { "true" } else { "false" }
                            ),
                        )?;
                        self.line(Some(event), format!("{prefix}  target: {}", escape(target)))?;
                        self.line(Some(event), format!("{prefix}  reason: {}", escape(reason)))?;
                    }
                }
                self.increase(event.scope)?;
            }
            EventKind::BranchEnd { exit } => {
                self.decrease(event.scope);
                let join_prefix = "  ".repeat(depth.saturating_sub(1));
                if let BranchExit::ContinueAfterProvenJoin = exit {
                    self.line(
                        Some(event),
                        format!("{join_prefix}Continue after branch (proven CFG join)"),
                    )?;
                }
                if let BranchExit::Stopped(reason) = exit {
                    self.line(
                        Some(event),
                        format!("{join_prefix}branch exploration stops: {}", escape(reason)),
                    )?;
                }
            }
            EventKind::CallStart {
                callee,
                invocation,
                child_scope,
                capture,
            } => {
                let child_depth = depth + 1;
                if child_depth > self.trace.limits.max_nesting {
                    return Err(TraceError::LimitExceeded {
                        what: "nesting depth",
                        limit: self.trace.limits.max_nesting,
                    });
                }
                let capture_text = match capture {
                    CaptureClaim::NoCapture => String::new(),
                    CaptureClaim::CapturedCell { cell, .. } => {
                        format!(" with captured cell {}", escape(cell))
                    }
                };
                self.line(
                    Some(event),
                    format!(
                        "{prefix}call {} call {}{}",
                        escape(callee),
                        invocation,
                        capture_text
                    ),
                )?;
                self.indent.insert(*child_scope, child_depth);
            }
            EventKind::CallEnd { .. } => {}
            EventKind::Resume {
                caller_scope,
                result,
            } => {
                let thread = self
                    .scope_by_id
                    .get(caller_scope)
                    .map(|scope| scope.thread)
                    .unwrap_or(0);
                self.line(
                    Some(event),
                    format!(
                        "{prefix}Resume thread {} with result {}",
                        thread,
                        value(result)
                    ),
                )?;
            }
            EventKind::Change {
                target,
                before,
                after,
            } => self.line(
                Some(event),
                format!(
                    "{prefix}Change {}: {} -> {}",
                    escape(target),
                    value(before),
                    value(after)
                ),
            )?,
            EventKind::Return {
                value: returned,
                disposition,
            } => {
                let line = match disposition {
                    ReturnDisposition::Complete => format!("{prefix}return {}", value(returned)),
                    ReturnDisposition::PendingFinally => {
                        format!("{prefix}return {} (pending finally)", value(returned))
                    }
                };
                self.line(Some(event), line)?;
            }
            EventKind::Throw { error, destination } => self.line(
                Some(event),
                format!(
                    "{prefix}throws {}; exception delivered to {}",
                    escape(error),
                    escape(destination)
                ),
            )?,
            EventKind::CatchStart { label } => {
                self.line(
                    Some(event),
                    format!("{prefix}catch handler: {}", escape(label)),
                )?;
                self.increase(event.scope)?;
            }
            EventKind::CatchEnd => self.decrease(event.scope),
            EventKind::FinallyStart { pending } => {
                self.line(
                    Some(event),
                    format!(
                        "{prefix}finally, with {} pending",
                        pending_text(pending, &self.trace.limits)
                    ),
                )?;
                self.pending_finally.insert(event.scope, pending.clone());
                self.increase(event.scope)?;
            }
            EventKind::FinallyEnd { completion } => {
                self.decrease(event.scope);
                let completion_prefix = "  ".repeat(depth.saturating_sub(1));
                let pending = self
                    .pending_finally
                    .remove(&event.scope)
                    .unwrap_or_else(|| PendingCompletion::Throw("unavailable".into()));
                let line = match completion {
                    FinallyCompletion::Completed(_) => format!(
                        "{completion_prefix}Resume pending {}; completion: {}",
                        pending_text(&pending, &self.trace.limits),
                        pending_text(&pending, &self.trace.limits)
                    ),
                    FinallyCompletion::ReplacedByReturn(result) => format!(
                        "{completion_prefix}finally return {} replaces pending {}",
                        value(result),
                        pending_text(&pending, &self.trace.limits)
                    ),
                    FinallyCompletion::ReplacedByThrow(error) => format!(
                        "{completion_prefix}finally throw {} replaces pending completion",
                        escape(error)
                    ),
                };
                self.line(Some(event), line)?;
            }
            EventKind::Capture {
                thread,
                cell,
                identity_evidence,
            } => self.line(
                Some(event),
                format!(
                    "{prefix}capture {} closure cell {} (identity evidence: {})",
                    escape(thread),
                    escape(cell),
                    escape(identity_evidence)
                ),
            )?,
            EventKind::OpaqueStop { reason } => {
                self.line(
                    Some(event),
                    format!("{prefix}opaque operation: result not described"),
                )?;
                self.line(
                    Some(event),
                    format!(
                        "{prefix}reason: {}; trace stops at this scope",
                        escape(reason)
                    ),
                )?;
            }
            EventKind::LoopStart { label } => {
                self.line(Some(event), format!("{prefix}loop: {}", escape(label)))?;
                self.increase(event.scope)?;
            }
            EventKind::IterationStart { ordinal } => self.line(
                Some(event),
                format!(
                    "{prefix}iteration {} ({})",
                    ordinal,
                    evidence_label(&event.evidence)
                ),
            )?,
            EventKind::IterationEnd => {}
            EventKind::LoopExit {
                expression,
                result,
                reason,
            } => self.line(
                Some(event),
                format!(
                    "{prefix}exit test: compare {} -> {} ({})",
                    escape(expression),
                    result,
                    escape(reason)
                ),
            )?,
            EventKind::LoopEnd { exit } => {
                self.decrease(event.scope);
                let exit_prefix = "  ".repeat(depth.saturating_sub(1));
                self.line(Some(event), format!("{exit_prefix}exit: {}", escape(exit)))?;
            }
            EventKind::NoUnconditionalResult { reason } => self.line(
                Some(event),
                format!("{prefix}No unconditional parent result: {}", escape(reason)),
            )?,
        }
        Ok(())
    }

    pub(super) fn increase(&mut self, scope: ScopeId) -> Result<(), TraceError> {
        let depth = self.indent.get(&scope).copied().unwrap_or(0) + 1;
        if depth > self.trace.limits.max_nesting {
            return Err(TraceError::LimitExceeded {
                what: "nesting depth",
                limit: self.trace.limits.max_nesting,
            });
        }
        self.indent.insert(scope, depth);
        Ok(())
    }

    pub(super) fn decrease(&mut self, scope: ScopeId) {
        if let Some(depth) = self.indent.get_mut(&scope) {
            *depth = depth.saturating_sub(1);
        }
    }

    pub(super) fn details(&self) -> Result<String, TraceError> {
        let mut out = String::new();
        let detail_limit = self
            .trace
            .limits
            .max_output_bytes
            .saturating_sub(self.main_bytes);
        append_bounded(
            &mut out,
            "rendered line map (line numbers are 1-based; ranges are inclusive)\n".into(),
            detail_limit,
        )?;
        for detail in &self.map {
            let range = if detail.line_start == detail.line_end {
                detail.line_start.to_string()
            } else {
                format!("{}-{}", detail.line_start, detail.line_end)
            };
            append_bounded(
                &mut out,
                format!("  line {range} -> events {:?}\n", detail.event_ids),
                detail_limit,
            )?;
            let p = &detail.provenance;
            append_bounded(
                &mut out,
                format!(
                    "    handler key: {}\n",
                    p.handler_key
                        .as_deref()
                        .map(escape)
                        .unwrap_or_else(|| "unavailable".into())
                ),
                detail_limit,
            )?;
            append_bounded(
                &mut out,
                format!(
                    "    micro-op ordinal: {}\n",
                    p.micro_op_ordinal
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "unavailable".into())
                ),
                detail_limit,
            )?;
            append_bounded(
                &mut out,
                format!(
                    "    source: {}\n",
                    p.source
                        .as_ref()
                        .map(|source| format!(
                            "{}:{}-{}",
                            escape(&source.artifact),
                            source.start,
                            source.end
                        ))
                        .unwrap_or_else(|| "unavailable".into())
                ),
                detail_limit,
            )?;
            append_bounded(
                &mut out,
                format!(
                    "    module revision: {}\n",
                    p.ir_revision
                        .as_deref()
                        .map(escape)
                        .unwrap_or_else(|| "unavailable".into())
                ),
                detail_limit,
            )?;
            append_bounded(
                &mut out,
                format!(
                    "    IR op ref: {}\n",
                    p.ir_op
                        .as_deref()
                        .map(escape)
                        .unwrap_or_else(|| "unavailable".into())
                ),
                detail_limit,
            )?;
            for event_id in &detail.event_ids {
                if let Some(TraceEvent {
                    kind:
                        EventKind::BranchStart {
                            status:
                                BranchStatus::ProvenDead {
                                    proof,
                                    evidence_ref,
                                    ..
                                },
                            ..
                        },
                    ..
                }) = self.event_by_id.get(event_id)
                {
                    append_bounded(
                        &mut out,
                        format!(
                            "    proof claim: producer supplied; not re-certified\n    proof premises: {}\n    proof evidence ref: {}\n",
                            escape(proof),
                            escape(evidence_ref)
                        ),
                        detail_limit,
                    )?;
                }
            }
        }
        Ok(out)
    }
}

pub(super) fn append_bounded(
    out: &mut String,
    text: String,
    limit: usize,
) -> Result<(), TraceError> {
    if out.len() + text.len() > limit {
        return Err(TraceError::OutputLimitExceeded { limit });
    }
    out.push_str(&text);
    Ok(())
}

pub(super) fn pending_text(pending: &PendingCompletion, limits: &Limits) -> String {
    match pending {
        PendingCompletion::Return(value) => format!("return {}", format_value(value, limits)),
        PendingCompletion::Throw(error) => format!("throw {}", escape(error)),
    }
}

pub(super) fn evidence_label(evidence: &Evidence) -> String {
    match evidence {
        Evidence::Observed => "observed".into(),
        Evidence::StaticDerived => "derived".into(),
        Evidence::Assumed { label, .. } => format!("assumed: {}", escape(label)),
    }
}

pub(super) fn format_value(value: &Value, limits: &Limits) -> String {
    match value {
        Value::Undefined => "undefined".into(),
        Value::Null => "null".into(),
        Value::Uninitialized => "uninitialized".into(),
        Value::Unavailable(reason) => format!("unavailable (reason: {})", escape(reason)),
        Value::Bool(value) => value.to_string(),
        Value::I32(value) => value.to_string(),
        Value::I64(value) => value.to_string(),
        Value::Number(value) if value.is_nan() => "NaN".into(),
        Value::Number(value) if value.is_infinite() && value.is_sign_positive() => {
            "Infinity".into()
        }
        Value::Number(value) if value.is_infinite() => "-Infinity".into(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => format_string(value, limits),
        Value::Reference(reference) => format!("ref({})", escape(reference)),
        Value::Preview {
            display,
            full_ref,
            size_bytes,
        } => {
            let reference = full_ref
                .as_deref()
                .map(escape)
                .unwrap_or_else(|| "full reference unavailable".into());
            format!(
                "{} [truncated; size {} bytes; {}]",
                format_string(display, limits),
                size_bytes,
                reference
            )
        }
    }
}

pub(super) fn format_string(value: &str, limits: &Limits) -> String {
    let escaped = escape(value);
    if value.len() <= limits.max_value_preview_bytes {
        return format!("\"{escaped}\"");
    }
    let mut prefix = String::new();
    for character in value.chars() {
        if prefix.len() + character.len_utf8() > limits.max_value_preview_bytes {
            break;
        }
        prefix.push(character);
    }
    format!(
        "\"{}…\" [truncated; size {} bytes; full reference unavailable]",
        escape(&prefix),
        value.len()
    )
}

pub(super) fn escape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{2028}' => output.push_str("\\u{2028}"),
            '\u{2029}' => output.push_str("\\u{2029}"),
            '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{206a}'..='\u{206f}' => {
                output.push_str(&format!("\\u{{{:04x}}}", character as u32))
            }
            character if character.is_control() => {
                output.push_str(&format!("\\x{:02x}", character as u32))
            }
            character => output.push(character),
        }
    }
    output
}

#[cfg(test)]
mod structure_tests {
    use super::*;

    /// Number of seeded, deterministic sequences checked against the fixture.
    const GOLDEN_SEQUENCE_COUNT: usize = 4096;
    const GOLDEN_SEED: u64 = 0x5eed_7e57_0b1e_5a11;

    struct Rng(u64);

    impl Rng {
        fn new(seed: u64) -> Self {
            Self(seed)
        }

        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut value = self.0;
            value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            value ^ (value >> 31)
        }

        fn below(&mut self, upper: usize) -> usize {
            debug_assert!(upper > 0, "random bound must be non-zero");
            (self.next() % upper as u64) as usize
        }

        fn chance(&mut self, percent: usize) -> bool {
            self.below(100) < percent
        }

        fn boolean(&mut self) -> bool {
            self.next() & 1 == 1
        }

        fn pick<T: Copy>(&mut self, values: &[T]) -> T {
            values[self.below(values.len())]
        }
    }

    fn demo_scopes() -> Vec<Scope> {
        vec![
            Scope {
                id: 0,
                thread: 0,
                call: 1,
                parent: None,
            },
            Scope {
                id: 1,
                thread: 7,
                call: 1,
                parent: Some(0),
            },
            Scope {
                id: 2,
                thread: 8,
                call: 1,
                parent: Some(0),
            },
            Scope {
                id: 3,
                thread: 7,
                call: 2,
                parent: Some(1),
            },
            Scope {
                id: 4,
                thread: 9,
                call: 1,
                parent: Some(2),
            },
        ]
    }

    fn random_value(rng: &mut Rng) -> Value {
        match rng.below(9) {
            0 => Value::Undefined,
            1 => Value::Null,
            2 => Value::Uninitialized,
            3 => Value::Bool(rng.boolean()),
            4 => Value::I32(rng.below(7) as i32 - 3),
            5 => Value::I64(rng.below(7) as i64 - 3),
            6 => Value::Number(
                [f64::NAN, 0.0, -0.0, 1.5, f64::INFINITY, f64::NEG_INFINITY][rng.below(6)],
            ),
            7 => Value::String("value".into()),
            _ => Value::Preview {
                display: "preview".into(),
                full_ref: Some("full".into()),
                size_bytes: 7,
            },
        }
    }

    fn random_pending(rng: &mut Rng) -> PendingCompletion {
        if rng.boolean() {
            PendingCompletion::Return(random_value(rng))
        } else {
            PendingCompletion::Throw("child error".into())
        }
    }

    fn random_branch_status(rng: &mut Rng) -> BranchStatus {
        match rng.below(4) {
            0 => BranchStatus::Observed {
                outcome: rng.boolean(),
                alternate: "alternate".into(),
            },
            1 => BranchStatus::Assumed {
                outcome: rng.boolean(),
                assumption: "gate = true".into(),
            },
            2 => BranchStatus::ProvenDead {
                dead_edge: "true edge".into(),
                proof: "word ^ word = 0".into(),
                evidence_ref: "snapshot:word7".into(),
            },
            _ => BranchStatus::Suspected {
                outcome: rng.boolean(),
                target: "offset 8192".into(),
                reason: "heuristic".into(),
            },
        }
    }

    fn random_capture(rng: &mut Rng) -> CaptureClaim {
        if rng.chance(75) {
            CaptureClaim::NoCapture
        } else {
            CaptureClaim::CapturedCell {
                cell: "slot[0]".into(),
                identity_evidence: "same retained cell".into(),
            }
        }
    }

    fn child_for(scope: ScopeId, rng: &mut Rng) -> (ScopeId, u32) {
        match scope {
            0 => {
                if rng.boolean() {
                    (1, 1)
                } else {
                    (2, 1)
                }
            }
            1 => (3, 2),
            2 => (4, 1),
            _ => (1, 1),
        }
    }

    const KIND_WEIGHTS: [u32; 24] = [
        2, 2, 2, 2, // input, read, compute, test
        10, 10, // branch start, branch end
        10, 10, // call start, call end
        6, 2, // resume, change
        8, 6, // return, throw
        5, 5, // catch start, catch end
        5, 5, // finally start, finally end
        2, 3, // capture, opaque stop
        8, 6, 6, 6, 8, // loop start, iteration start/end, loop exit, loop end
        2, // no unconditional result
    ];

    fn random_kind_index(rng: &mut Rng) -> usize {
        let total: u32 = KIND_WEIGHTS.iter().sum();
        let mut pick = rng.below(total as usize) as u32;
        for (index, weight) in KIND_WEIGHTS.iter().enumerate() {
            if pick < *weight {
                return index;
            }
            pick -= weight;
        }
        KIND_WEIGHTS.len() - 1
    }

    fn random_kind(rng: &mut Rng, index: usize, event_scope: ScopeId) -> EventKind {
        match index {
            0 => EventKind::Input {
                label: "input".into(),
                value: random_value(rng),
            },
            1 => EventKind::Read {
                expression: "x".into(),
                value: random_value(rng),
            },
            2 => EventKind::Compute {
                expression: "x + 1".into(),
                result: random_value(rng),
            },
            3 => EventKind::Test {
                expression: "x > 0".into(),
                result: if rng.boolean() {
                    TestResult::Known {
                        value: rng.boolean(),
                        reason: "known".into(),
                    }
                } else {
                    TestResult::Unknown {
                        reason: "unknown".into(),
                    }
                },
            },
            4 => EventKind::BranchStart {
                label: "branch".into(),
                status: random_branch_status(rng),
            },
            5 => EventKind::BranchEnd {
                exit: match rng.below(3) {
                    0 => BranchExit::ContinueAfterProvenJoin,
                    1 => BranchExit::Returned,
                    _ => BranchExit::Stopped("opaque".into()),
                },
            },
            6 => {
                let (child_scope, call) = child_for(event_scope, rng);
                let invocation = if rng.chance(85) {
                    call
                } else {
                    rng.below(3) as u32 + 1
                };
                EventKind::CallStart {
                    callee: "child".into(),
                    invocation,
                    child_scope,
                    capture: random_capture(rng),
                }
            }
            7 => EventKind::CallEnd {
                completion: match rng.below(3) {
                    0 => CallCompletion::Returned(random_value(rng)),
                    1 => CallCompletion::Threw("child error".into()),
                    _ => CallCompletion::Stopped {
                        reason: "opaque".into(),
                    },
                },
            },
            8 => EventKind::Resume {
                caller_scope: if rng.chance(85) {
                    event_scope
                } else {
                    rng.below(5) as ScopeId
                },
                result: random_value(rng),
            },
            9 => EventKind::Change {
                target: "slot[0]".into(),
                before: random_value(rng),
                after: random_value(rng),
            },
            10 => EventKind::Return {
                value: random_value(rng),
                disposition: if rng.boolean() {
                    ReturnDisposition::Complete
                } else {
                    ReturnDisposition::PendingFinally
                },
            },
            11 => EventKind::Throw {
                error: "child error".into(),
                destination: "caller".into(),
            },
            12 => EventKind::CatchStart {
                label: "handler".into(),
            },
            13 => EventKind::CatchEnd,
            14 => EventKind::FinallyStart {
                pending: random_pending(rng),
            },
            15 => EventKind::FinallyEnd {
                completion: match rng.below(3) {
                    0 => FinallyCompletion::Completed(random_pending(rng)),
                    1 => FinallyCompletion::ReplacedByReturn(random_value(rng)),
                    _ => FinallyCompletion::ReplacedByThrow("replacement error".into()),
                },
            },
            16 => EventKind::Capture {
                thread: "thread 7".into(),
                cell: "slot[0]".into(),
                identity_evidence: "same retained cell".into(),
            },
            17 => EventKind::OpaqueStop {
                reason: "no mapping".into(),
            },
            18 => EventKind::LoopStart {
                label: "loop".into(),
            },
            19 => EventKind::IterationStart {
                ordinal: rng.below(3) as u32 + 1,
            },
            20 => EventKind::IterationEnd,
            21 => EventKind::LoopExit {
                expression: "loop".into(),
                result: rng.boolean(),
                reason: "test".into(),
            },
            22 => EventKind::LoopEnd {
                exit: "left".into(),
            },
            _ => EventKind::NoUnconditionalResult {
                reason: "depends on host".into(),
            },
        }
    }

    /// Evidence is kept reference-valid: an assumed reference always names an event
    /// whose kind is `BranchStart` (itself for an assumed branch start). This keeps
    /// `validate_references` off the error path, so the corpus exercises the
    /// structure walk; reference errors are pinned in `validator_tests`.
    fn random_evidence(
        rng: &mut Rng,
        id: EventId,
        is_branch_start: bool,
        assumed_status: bool,
        branch_pool: &[EventId],
    ) -> Evidence {
        if rng.chance(20) {
            let branch = if is_branch_start {
                if assumed_status || branch_pool.is_empty() || rng.chance(40) {
                    id
                } else {
                    rng.pick(branch_pool)
                }
            } else if branch_pool.is_empty() {
                return Evidence::Observed;
            } else {
                rng.pick(branch_pool)
            };
            Evidence::Assumed {
                branch,
                label: "gate = true".into(),
            }
        } else if rng.chance(10) {
            Evidence::StaticDerived
        } else {
            Evidence::Observed
        }
    }

    fn random_sequence(rng: &mut Rng) -> Vec<TraceEvent> {
        let count = rng.below(29);
        let mut current: ScopeId = 0;
        let mut scopes = Vec::with_capacity(count);
        let mut kinds = Vec::with_capacity(count);
        for _ in 0..count {
            let index = random_kind_index(rng);
            let scope = if rng.chance(75) {
                current
            } else {
                rng.below(5) as ScopeId
            };
            kinds.push(random_kind(rng, index, scope));
            scopes.push(scope);
            current = scope;
        }
        let branch_pool: Vec<EventId> = kinds
            .iter()
            .enumerate()
            .filter(|(_, kind)| matches!(kind, EventKind::BranchStart { .. }))
            .map(|(index, _)| index as EventId + 1)
            .collect();
        kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                let id = index as EventId + 1;
                let evidence = random_evidence(
                    rng,
                    id,
                    matches!(kind, EventKind::BranchStart { .. }),
                    matches!(
                        &kind,
                        EventKind::BranchStart {
                            status: BranchStatus::Assumed { .. },
                            ..
                        }
                    ),
                    &branch_pool,
                );
                TraceEvent {
                    id,
                    scope: scopes[index],
                    evidence,
                    provenance: Provenance::default(),
                    kind,
                }
            })
            .collect()
    }

    fn observed(id: EventId, scope: ScopeId, kind: EventKind) -> TraceEvent {
        TraceEvent {
            id,
            scope,
            evidence: Evidence::Observed,
            provenance: Provenance::default(),
            kind,
        }
    }

    fn assumed(id: EventId, scope: ScopeId, branch: EventId, kind: EventKind) -> TraceEvent {
        TraceEvent {
            id,
            scope,
            evidence: Evidence::Assumed {
                branch,
                label: "gate = true".into(),
            },
            provenance: Provenance::default(),
            kind,
        }
    }

    fn observed_branch() -> EventKind {
        EventKind::BranchStart {
            label: "cond".into(),
            status: BranchStatus::Observed {
                outcome: true,
                alternate: "other".into(),
            },
        }
    }

    fn assumed_branch() -> EventKind {
        EventKind::BranchStart {
            label: "assumed".into(),
            status: BranchStatus::Assumed {
                outcome: true,
                assumption: "gate = true".into(),
            },
        }
    }

    fn complete_return(value: i32) -> EventKind {
        EventKind::Return {
            value: Value::I32(value),
            disposition: ReturnDisposition::Complete,
        }
    }

    /// A small set of structurally valid traces that reach deep validation paths.
    fn valid_templates() -> Vec<Vec<TraceEvent>> {
        vec![
            vec![],
            vec![
                observed(
                    1,
                    0,
                    EventKind::Input {
                        label: "a".into(),
                        value: Value::I32(1),
                    },
                ),
                observed(
                    2,
                    0,
                    EventKind::Read {
                        expression: "a".into(),
                        value: Value::I32(1),
                    },
                ),
            ],
            vec![
                observed(1, 0, observed_branch()),
                observed(
                    2,
                    0,
                    EventKind::BranchEnd {
                        exit: BranchExit::ContinueAfterProvenJoin,
                    },
                ),
            ],
            vec![
                observed(1, 0, observed_branch()),
                observed(2, 0, complete_return(3)),
                observed(
                    3,
                    0,
                    EventKind::BranchEnd {
                        exit: BranchExit::Returned,
                    },
                ),
            ],
            vec![
                observed(1, 0, EventKind::LoopStart { label: "l".into() }),
                observed(2, 0, EventKind::IterationStart { ordinal: 1 }),
                observed(3, 0, EventKind::IterationEnd),
                observed(
                    4,
                    0,
                    EventKind::LoopExit {
                        expression: "l".into(),
                        result: false,
                        reason: "done".into(),
                    },
                ),
                observed(
                    5,
                    0,
                    EventKind::LoopEnd {
                        exit: "left".into(),
                    },
                ),
            ],
            vec![
                observed(1, 0, EventKind::LoopStart { label: "l".into() }),
                observed(
                    2,
                    0,
                    EventKind::LoopExit {
                        expression: "l".into(),
                        result: false,
                        reason: "guard false".into(),
                    },
                ),
                observed(
                    3,
                    0,
                    EventKind::LoopEnd {
                        exit: "left".into(),
                    },
                ),
            ],
            vec![
                observed(
                    1,
                    0,
                    EventKind::CallStart {
                        callee: "child".into(),
                        invocation: 1,
                        child_scope: 1,
                        capture: CaptureClaim::NoCapture,
                    },
                ),
                observed(2, 1, complete_return(3)),
                observed(
                    3,
                    1,
                    EventKind::CallEnd {
                        completion: CallCompletion::Returned(Value::I32(3)),
                    },
                ),
                observed(
                    4,
                    0,
                    EventKind::Resume {
                        caller_scope: 0,
                        result: Value::I32(3),
                    },
                ),
            ],
            vec![
                observed(
                    1,
                    0,
                    EventKind::CallStart {
                        callee: "child".into(),
                        invocation: 1,
                        child_scope: 1,
                        capture: CaptureClaim::NoCapture,
                    },
                ),
                observed(
                    2,
                    1,
                    EventKind::Throw {
                        error: "child error".into(),
                        destination: "caller".into(),
                    },
                ),
                observed(
                    3,
                    1,
                    EventKind::CallEnd {
                        completion: CallCompletion::Threw("child error".into()),
                    },
                ),
                observed(
                    4,
                    0,
                    EventKind::CatchStart {
                        label: "handler".into(),
                    },
                ),
                observed(5, 0, EventKind::CatchEnd),
            ],
            vec![
                observed(
                    1,
                    0,
                    EventKind::Return {
                        value: Value::I32(3),
                        disposition: ReturnDisposition::PendingFinally,
                    },
                ),
                observed(
                    2,
                    0,
                    EventKind::FinallyStart {
                        pending: PendingCompletion::Return(Value::I32(3)),
                    },
                ),
                observed(
                    3,
                    0,
                    EventKind::FinallyEnd {
                        completion: FinallyCompletion::Completed(PendingCompletion::Return(
                            Value::I32(3),
                        )),
                    },
                ),
            ],
            vec![
                assumed(1, 0, 1, assumed_branch()),
                assumed(2, 0, 1, complete_return(3)),
                assumed(
                    3,
                    0,
                    1,
                    EventKind::BranchEnd {
                        exit: BranchExit::Returned,
                    },
                ),
                observed(
                    4,
                    0,
                    EventKind::NoUnconditionalResult {
                        reason: "depends on host".into(),
                    },
                ),
            ],
            vec![
                observed(1, 0, observed_branch()),
                observed(2, 0, observed_branch()),
                observed(
                    3,
                    0,
                    EventKind::BranchEnd {
                        exit: BranchExit::ContinueAfterProvenJoin,
                    },
                ),
                observed(
                    4,
                    0,
                    EventKind::BranchEnd {
                        exit: BranchExit::ContinueAfterProvenJoin,
                    },
                ),
            ],
            vec![observed(
                1,
                0,
                EventKind::OpaqueStop {
                    reason: "no mapping".into(),
                },
            )],
            vec![
                observed(1, 0, EventKind::LoopStart { label: "l".into() }),
                observed(2, 0, complete_return(9)),
                observed(
                    3,
                    0,
                    EventKind::LoopEnd {
                        exit: "left".into(),
                    },
                ),
            ],
        ]
    }

    /// Keep every generated sequence reference-valid so the corpus exercises only
    /// the event-ordered structure walk. Reference errors are reported in event-id
    /// order and pinned separately in `validator_tests`.
    fn sanitize_references(events: &mut [TraceEvent]) {
        let branch_ids: Vec<EventId> = events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::BranchStart { .. }))
            .map(|event| event.id)
            .collect();
        for event in events.iter_mut() {
            let assumed_status = matches!(
                &event.kind,
                EventKind::BranchStart {
                    status: BranchStatus::Assumed { .. },
                    ..
                }
            );
            if assumed_status {
                if !matches!(&event.evidence, Evidence::Assumed { branch, .. } if *branch == event.id)
                {
                    event.evidence = Evidence::Assumed {
                        branch: event.id,
                        label: "gate = true".into(),
                    };
                }
            } else if let Evidence::Assumed { branch, .. } = &event.evidence {
                if !branch_ids.contains(branch) {
                    event.evidence = Evidence::Observed;
                }
            }
        }
    }

    struct Case {
        scopes: Vec<Scope>,
        events: Vec<TraceEvent>,
        limits: Limits,
    }

    fn case_at(index: usize) -> Case {
        let mut rng = Rng::new(GOLDEN_SEED ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut limits = Limits::default();
        if rng.chance(5) {
            limits.max_nesting = rng.below(8);
        }
        if index.is_multiple_of(4) {
            let templates = valid_templates();
            let mut events = templates[(index / 4) % templates.len()].clone();
            if !events.is_empty() {
                let keep = rng.below(events.len() + 1);
                events.truncate(keep);
            }
            if !events.is_empty() && rng.chance(50) {
                let position = rng.below(events.len());
                let scope = events[position].scope;
                let kind = random_kind_index(&mut rng);
                events[position].kind = random_kind(&mut rng, kind, scope);
            }
            sanitize_references(&mut events);
            Case {
                scopes: demo_scopes(),
                events,
                limits,
            }
        } else {
            let mut events = random_sequence(&mut rng);
            sanitize_references(&mut events);
            Case {
                scopes: demo_scopes(),
                events,
                limits,
            }
        }
    }

    fn run_structure(case: &Case) -> Result<(), TraceError> {
        let scope_map: HashMap<ScopeId, &Scope> =
            case.scopes.iter().map(|scope| (scope.id, scope)).collect();
        let ids: HashMap<EventId, &TraceEvent> =
            case.events.iter().map(|event| (event.id, event)).collect();
        validate_structure(&case.events, &scope_map, &case.limits, &ids)
    }

    fn error_key(error: &TraceError) -> String {
        match error {
            TraceError::LimitExceeded { what, limit } => format!("limit|{what}|{limit}"),
            TraceError::OutputLimitExceeded { limit } => format!("output|{limit}"),
            TraceError::FieldTooLong { field, limit } => format!("field|{field}|{limit}"),
            TraceError::DuplicateEventId { id } => format!("duplicate_event|{id}"),
            TraceError::DuplicateScopeId { id } => format!("duplicate_scope|{id}"),
            TraceError::UnknownScope { event, scope } => {
                format!("unknown_scope|{event}|{scope}")
            }
            TraceError::InvalidScopeParent { scope, parent } => {
                format!("scope_parent|{scope}|{parent}")
            }
            TraceError::ScopeCycle { scope } => format!("scope_cycle|{scope}"),
            TraceError::UnknownEventReference { event, reference } => {
                format!("unknown_reference|{event}|{reference}")
            }
            TraceError::InvalidBranchStructure { event, reason } => {
                format!("branch|{event}|{reason}")
            }
            TraceError::InvalidCallStructure { event, reason } => {
                format!("call|{event}|{reason}")
            }
            TraceError::InvalidFinallyStructure { event, reason } => {
                format!("finally|{event}|{reason}")
            }
            TraceError::InvalidLoopStructure { event, reason } => {
                format!("loop|{event}|{reason}")
            }
            TraceError::EventAfterOpaqueStop { event, scope } => {
                format!("after_stop|{event}|{scope}")
            }
            TraceError::MissingProofEvidence { event } => format!("missing_proof|{event}"),
            TraceError::UnsupportedEvidence { event, reason } => {
                format!("evidence|{event}|{reason}")
            }
        }
    }

    fn result_key(result: &Result<(), TraceError>) -> String {
        match result {
            Ok(()) => "ok".into(),
            Err(error) => format!("err|{}", error_key(error)),
        }
    }

    fn fixture_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/trace_text_structure_golden.txt")
    }

    fn render_golden() -> String {
        let mut output = String::new();
        output.push_str("# text-trace structure golden v1\n");
        output.push_str(&format!(
            "# sequences: {GOLDEN_SEQUENCE_COUNT}; seed: {GOLDEN_SEED:#x}; generator: structure_tests::case_at\n"
        ));
        for index in 0..GOLDEN_SEQUENCE_COUNT {
            let result = run_structure(&case_at(index));
            output.push_str(&format!("{index}\t{}\n", result_key(&result)));
        }
        output
    }

    /// The golden fixture was captured from a corpus that never fails the reference
    /// checks; this test keeps the corpus that way so the fixture stays valid.
    #[test]
    fn seeded_corpus_avoids_reference_layer_errors() {
        for index in 0..GOLDEN_SEQUENCE_COUNT {
            let case = case_at(index);
            let ids: HashMap<EventId, &TraceEvent> =
                case.events.iter().map(|event| (event.id, event)).collect();
            if let Err(error) = validate_references(&ids) {
                panic!("seeded sequence {index} hits the reference layer: {error:?}");
            }
        }
    }

    /// Reproducible pinned results for the same seeded corpus. The fixture was
    /// captured from the original inline validator (a 4096-sequence differential run
    /// verified the refactor against it with zero mismatches before that
    /// implementation was deleted). It stays checked in so the behavior remains
    /// pinned without keeping a copy of the old code.
    ///
    /// Regenerate (only when behavior is intentionally changed):
    /// `UPDATE_TRACE_STRUCTURE_GOLDEN=1 cargo test --lib structure_golden`
    #[test]
    fn structure_golden_results_are_stable() {
        let actual = render_golden();
        if std::env::var_os("UPDATE_TRACE_STRUCTURE_GOLDEN").is_some() {
            std::fs::write(fixture_path(), actual).expect("write golden fixture");
            return;
        }
        let expected = std::fs::read_to_string(fixture_path()).unwrap_or_else(|error| {
            panic!(
                "read {}: {error}; regenerate with UPDATE_TRACE_STRUCTURE_GOLDEN=1",
                fixture_path().display()
            )
        });
        let expected: Vec<&str> = expected
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect();
        let actual: Vec<&str> = actual
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect();
        assert_eq!(
            expected.len(),
            actual.len(),
            "golden fixture has {} sequences but the generator produced {}",
            expected.len(),
            actual.len()
        );
        for (index, (expected, actual)) in expected.iter().zip(&actual).enumerate() {
            assert_eq!(
                expected, actual,
                "seeded sequence {index} does not match the checked-in golden fixture"
            );
        }
    }
}

/// White-box coverage for defensive validator branches that the public
/// `Trace::validate` entry point cannot reach: they require a stack state the
/// caller-facing checks already exclude. They still return exact `TraceError`
/// values, so they are pinned at the unit level.
#[cfg(test)]
mod validator_tests {
    use super::*;

    fn call_error(event: EventId, reason: &str) -> TraceError {
        TraceError::InvalidCallStructure {
            event,
            reason: reason.into(),
        }
    }

    fn invocation_frame(scope: ScopeId) -> InvocationFrame {
        InvocationFrame {
            scope,
            call_start: None,
            controls: Vec::new(),
            terminal: None,
            awaiting: None,
            pending_finally: None,
            assumed_branches: Vec::new(),
        }
    }

    fn empty_scopes() -> HashMap<ScopeId, &'static Scope> {
        HashMap::new()
    }

    fn structure_event(id: EventId, evidence: Evidence, kind: EventKind) -> TraceEvent {
        TraceEvent {
            id,
            scope: 0,
            evidence,
            provenance: Provenance::default(),
            kind,
        }
    }

    /// Several reference violations report the lowest event id. Each fresh
    /// `HashMap` gets a new random iteration order, so repeating the check
    /// catches a dependence on that order.
    #[test]
    fn reference_errors_report_the_lowest_event_id() {
        let events: Vec<TraceEvent> = [9, 3, 6]
            .into_iter()
            .map(|id| {
                structure_event(
                    id,
                    Evidence::Assumed {
                        branch: 100 + id,
                        label: "gate = true".into(),
                    },
                    EventKind::OpaqueStop {
                        reason: "no mapping".into(),
                    },
                )
            })
            .collect();
        for _ in 0..64 {
            let ids: HashMap<EventId, &TraceEvent> =
                events.iter().map(|event| (event.id, event)).collect();
            assert_eq!(
                validate_references(&ids).unwrap_err(),
                TraceError::UnknownEventReference {
                    event: 3,
                    reference: 103,
                }
            );
        }
    }

    #[test]
    fn scope_errors_report_the_lowest_scope_id() {
        let scopes: Vec<Scope> = [(0, None), (8, Some(80)), (2, Some(20)), (5, Some(50))]
            .into_iter()
            .map(|(id, parent)| Scope {
                id,
                thread: 0,
                call: 1,
                parent,
            })
            .collect();
        for _ in 0..64 {
            let map: HashMap<ScopeId, &Scope> =
                scopes.iter().map(|scope| (scope.id, scope)).collect();
            assert_eq!(
                validate_scope_graph(&map).unwrap_err(),
                TraceError::InvalidScopeParent {
                    scope: 2,
                    parent: 20,
                }
            );
        }

        // Two disjoint cycles: the walk starts from the lowest scope id.
        let cycles: Vec<Scope> = [
            (0, None),
            (7, Some(6)),
            (6, Some(7)),
            (4, Some(3)),
            (3, Some(4)),
        ]
        .into_iter()
        .map(|(id, parent)| Scope {
            id,
            thread: 0,
            call: 1,
            parent,
        })
        .collect();
        for _ in 0..64 {
            let map: HashMap<ScopeId, &Scope> =
                cycles.iter().map(|scope| (scope.id, scope)).collect();
            assert_eq!(
                validate_scope_graph(&map).unwrap_err(),
                TraceError::ScopeCycle { scope: 3 }
            );
        }
    }

    #[test]
    fn active_accessors_report_an_ended_invocation_stack() {
        let scopes = empty_scopes();
        let limits = Limits::default();
        let mut validator = StructureValidator::new(&scopes, &limits);
        validator.invocations.clear();
        assert_eq!(
            validator.active_ref(7).unwrap_err(),
            call_error(7, "event occurs after the active invocation stack ended")
        );
        assert_eq!(
            validator.active(7).unwrap_err(),
            call_error(7, "event occurs after the active invocation stack ended")
        );
    }

    #[test]
    fn call_end_without_an_active_invocation_is_defended() {
        let scopes = empty_scopes();
        let limits = Limits::default();
        let mut validator = StructureValidator::new(&scopes, &limits);
        validator.invocations.clear();
        let completion = CallCompletion::Returned(Value::I32(1));
        assert_eq!(
            validator.on_call_end(7, &completion).unwrap_err(),
            call_error(7, "call end has no active invocation")
        );
    }

    #[test]
    fn call_end_without_a_caller_frame_is_defended() {
        let scopes = empty_scopes();
        let limits = Limits::default();
        let mut validator = StructureValidator::new(&scopes, &limits);
        let mut child = invocation_frame(1);
        child.call_start = Some(1);
        child.terminal = Some(Terminal::Returned(Value::I32(1)));
        validator.invocations = vec![child];
        validator.depth = 1;
        let completion = CallCompletion::Returned(Value::I32(1));
        assert_eq!(
            validator.on_call_end(7, &completion).unwrap_err(),
            call_error(7, "child call has no caller frame")
        );
    }

    #[test]
    fn call_end_with_unavailable_caller_is_defended() {
        let scopes = empty_scopes();
        let limits = Limits::default();
        let mut validator = StructureValidator::new(&scopes, &limits);
        let mut root = invocation_frame(0);
        root.terminal = Some(Terminal::Returned(Value::I32(9)));
        let mut child = invocation_frame(1);
        child.call_start = Some(1);
        child.terminal = Some(Terminal::Returned(Value::I32(1)));
        validator.invocations = vec![root, child];
        validator.depth = 1;
        let completion = CallCompletion::Returned(Value::I32(1));
        assert_eq!(
            validator.on_call_end(7, &completion).unwrap_err(),
            call_error(7, "caller is not available for this completion")
        );
    }

    #[test]
    fn assumed_branch_ancestry_mismatch_is_defended() {
        let scopes = empty_scopes();
        let limits = Limits::default();
        let mut validator = StructureValidator::new(&scopes, &limits);
        let mut root = invocation_frame(0);
        root.controls.push(ControlFrame::Branch {
            event: 1,
            assumed: true,
            parent_terminal: None,
        });
        root.assumed_branches.push(2);
        validator.invocations = vec![root];
        validator.depth = 1;
        assert_eq!(
            validator
                .on_branch_end(7, &BranchExit::ContinueAfterProvenJoin)
                .unwrap_err(),
            TraceError::InvalidBranchStructure {
                event: 7,
                reason: "assumed branch ancestry is not innermost".into(),
            }
        );
    }

    #[test]
    fn assumed_branch_status_with_observed_evidence_is_defended() {
        let scopes = empty_scopes();
        let limits = Limits::default();
        let mut validator = StructureValidator::new(&scopes, &limits);
        let event = structure_event(
            1,
            Evidence::Observed,
            EventKind::BranchStart {
                label: "branch".into(),
                status: BranchStatus::Assumed {
                    outcome: true,
                    assumption: "gate = true".into(),
                },
            },
        );
        let EventKind::BranchStart { status, .. } = &event.kind else {
            unreachable!("constructed as a branch start");
        };
        assert_eq!(
            validator.on_branch_start(&event, status).unwrap_err(),
            TraceError::InvalidBranchStructure {
                event: 1,
                reason: "assumed branch status and evidence disagree".into(),
            }
        );
    }

    #[test]
    fn assumed_branch_start_requires_self_referencing_evidence() {
        let frame = invocation_frame(0);
        let event = structure_event(
            1,
            Evidence::Assumed {
                branch: 5,
                label: "gate = true".into(),
            },
            EventKind::BranchStart {
                label: "branch".into(),
                status: BranchStatus::Assumed {
                    outcome: true,
                    assumption: "gate = true".into(),
                },
            },
        );
        assert_eq!(
            validate_event_evidence(&event, &frame).unwrap_err(),
            TraceError::UnsupportedEvidence {
                event: 1,
                reason: "assumed branch start must carry self-referencing assumed evidence".into(),
            }
        );
    }
}
