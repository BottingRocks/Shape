//! `SensorReadyEvent`: the slot recognizer and the effect reader of a
//! declared-not-emitted slot (no data on the wire).

#![allow(unused_variables)]
use oxc_ast::ast::{AssignmentTarget, Expression};
use oxc_ast::AstKind;
use oxc_span::GetSpan;
use serde_json::{json, Map, Value};

use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::leaf_checks::*;
use crate::signals::catalog::card::{Card, CardBody};

/// `SensorReadyEvent`. Card: [`CARD`].
pub(crate) fn recognize_sensor_ready_event(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // Completes without `next`, then raises a global flag to `true` and
    // dispatches a `CustomEvent` on the global object.
    if !view.declared_not_emitted() || !view.uses_member("dispatchEvent") {
        return None;
    }
    ready_effect(view, ctx)?;
    claim(&[
        "declared, not emitted: completes without next",
        "global flag = true",
        "dispatchEvent(new CustomEvent(<type>))",
    ])
}

/// Never called: a no-data signal has no emission to bind; its entry is
/// rendered from [`effect_sensor_ready_event`].
pub(crate) fn bind_sensor_ready_event(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    Err(BindRefusal::new("SensorReadyEvent sends no data"))
}

/// The page-visible effect: `{windowFlag, event}`, read from this build.
pub(crate) fn effect_sensor_ready_event(view: &SlotView, ctx: &FinderCtx) -> Option<Map<String, Value>> {
    let (flag, event) = ready_effect(view, ctx)?;
    let mut effect = Map::new();
    effect.insert("windowFlag".into(), json!(flag));
    effect.insert("event".into(), json!(event));
    Some(effect)
}

/// The one global property the producer sets to `true` and the one
/// `CustomEvent` type it constructs; `None` unless each is unique.
fn ready_effect(view: &SlotView, ctx: &FinderCtx) -> Option<(String, String)> {
    let producer = view.producer_span()?;
    let mut events = view
        .host()
        .calls
        .iter()
        .filter(|call| call.construct && call.method == "CustomEvent")
        .map(|call| call.string_args.first().cloned());
    let event = events.next()??;
    if events.next().is_some() {
        return None;
    }
    let mut flags = Vec::new();
    for node in ctx.parse.nodes.iter() {
        let AstKind::AssignmentExpression(assignment) = node.kind() else {
            continue;
        };
        let span = assignment.span();
        if span.start < producer.start || span.end > producer.end {
            continue;
        }
        if !matches!(assignment.right.without_parentheses(), Expression::BooleanLiteral(value) if value.value) {
            continue;
        }
        let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
            continue;
        };
        let on_global = match target.object.without_parentheses() {
            Expression::StaticMemberExpression(object) => object.property.name == "global",
            Expression::Identifier(identifier) => {
                matches!(identifier.name.as_str(), "window" | "self" | "globalThis")
            }
            _ => false,
        };
        if on_global {
            flags.push(target.property.name.to_string());
        }
    }
    match flags.as_slice() {
        [flag] => Some((flag.clone(), event)),
        _ => None,
    }
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `SensorReadyEvent`: what it does and why. It has no
/// fields: the slot never carries a collected value.
pub(crate) const CARD: Card = Card {
    signal: "SensorReadyEvent",
    summary: "Sends no data. When the collector starts, this named slot completes at once without a value, so it is always reported as not collected. Its only effect is on the page: it sets a property on window to true and, on the next tick, dispatches a CustomEvent on window (in every build seen so far both are named f5cd836ea16e4a77). The report lists the property (windowFlag) and the event type (event) read from the build.",
    why: "Likely a readiness hook for the protected site's own scripts: the page can check the window flag or listen for the event to know the sensor script has loaded and started collecting (the listener is not in the collector source).",
    sample_payload: None,
    by_name: true,
    body: CardBody::Fields(&[]),
};
