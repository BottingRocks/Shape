//! Host-side-effect probes that finish an observer without a payload.
//!
//! A direct signal's task callback can be a probe rather than a collector: it
//! hands the observer to a scheduler, completes it, and records its result as a
//! host side effect (here, dispatching a `CustomEvent` and setting a global
//! flag). The observer never receives a `next`, so the wire model has nothing
//! to reconstruct for that signal.
//!
//! The detector is structural, not literal-keyed:
//!
//! * a `new CustomEvent(<string literal>, …)` expression,
//! * inside a function/arrow whose lexical subtree calls `.complete()` and
//!   `.dispatchEvent`, and
//! * never mentions a `.next` property anywhere in that subtree.
//!
//! When the owning callback is the task of a direct descriptor slot, the probe
//! carries that signal index. The event literal is data, not identity: a
//! renamed probe is still detected.

use std::collections::BTreeSet;

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_span::Span;
use serde::{Deserialize, Serialize};

use crate::signals::extract::layout::span_serde;
use crate::signals::schema::{FunctionRef, WaveRegistry};
use super::bindings::Model;

/// What a host probe does besides completing its observer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostProbeKind {
    /// Constructs a `CustomEvent`, sets a global flag with the same name, and
    /// schedules `dispatchEvent`.
    CustomEventDispatch,
}

/// One observer route whose collector performs a host side effect and never
/// emits a value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostProbe {
    pub kind: HostProbeKind,
    /// The event name literal (`CustomEvent("…")`).
    pub event: String,
    /// The global flag assigned `true` under the same name, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_flag: Option<String>,
    /// The probe callback (the scheduler callback that receives the observer).
    #[serde(with = "span_serde")]
    pub span: Span,
    pub function: Option<FunctionRef>,
    /// The direct descriptor signal this probe belongs to, when the callback is
    /// a resolved task of a direct slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<u32>,
}

/// Find every CustomEvent-dispatch probe in the parsed program.
pub(crate) fn detect(model: &Model<'_, '_>, waves: Option<&WaveRegistry>) -> Vec<HostProbe> {
    let mut probes = Vec::new();
    let mut seen = BTreeSet::new();
    for node in model.nodes().iter() {
        let AstKind::NewExpression(new) = node.kind() else {
            continue;
        };
        let Expression::Identifier(callee) = new.callee.without_parentheses() else {
            continue;
        };
        if callee.name != "CustomEvent" || reference_symbol(callee, model).is_some() {
            continue;
        }
        let Some(event) = new
            .arguments
            .first()
            .and_then(Argument::as_expression)
            .and_then(|argument| match argument.without_parentheses() {
                Expression::StringLiteral(literal) => Some(literal.value.to_string()),
                _ => None,
            })
        else {
            continue;
        };
        let Some(site) = model.owning_site(node.id()) else {
            continue;
        };
        if !seen.insert((site.span.start, site.span.end, event.clone())) {
            continue;
        }
        let mut has_next = false;
        let mut has_complete = false;
        let mut has_dispatch = false;
        let mut global_flag = None;
        for inner in model.nodes().iter() {
            if !model.node_within(inner.id(), site) {
                continue;
            }
            let AstKind::StaticMemberExpression(member) = inner.kind() else {
                continue;
            };
            match member.property.name.as_str() {
                "next" => has_next = true,
                "complete" => has_complete = true,
                "dispatchEvent" => has_dispatch = true,
                _ => {}
            }
            if member.property.name.as_str() == event {
                let parent = model.nodes().parent_id(inner.id());
                if let AstKind::AssignmentExpression(assignment) = model.nodes().kind(parent) {
                    if matches!(
                        assignment.right.without_parentheses(),
                        Expression::BooleanLiteral(literal) if literal.value
                    ) {
                        global_flag = Some(member.property.name.to_string());
                    }
                }
            }
        }
        if !has_complete || !has_dispatch || has_next {
            continue;
        }
        let signal = waves.and_then(|waves| {
            waves
                .direct_slots
                .iter()
                .find(|slot| {
                    slot.callback.as_ref().is_some_and(|callback| {
                        callback.span.start <= site.span.start
                            && site.span.end <= callback.span.end
                    })
                })
                .map(|slot| slot.signal as u32)
        });
        probes.push(HostProbe {
            kind: HostProbeKind::CustomEventDispatch,
            event,
            global_flag,
            span: site.span,
            function: Some(model.function_ref(site)),
            signal,
        });
    }
    probes
}

fn reference_symbol(identifier: &IdentifierReference<'_>, model: &Model<'_, '_>) -> Option<oxc_semantic::SymbolId> {
    identifier
        .reference_id
        .get()
        .and_then(|reference| model.scoping().get_reference(reference).symbol_id())
}
