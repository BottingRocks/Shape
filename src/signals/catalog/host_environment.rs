//! `HostEnvironment`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::extract::contracts::SourceClass;
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `HostEnvironment`. Card: [`CARD`].
/// A value returned by `getHostEnvironmentValue()`: a transform of the
/// written value, or the host API path itself when the call result is
/// written unchanged.
/// The value is `external.getHostEnvironmentValue(…)`, called directly or
/// read off and invoked through `.call` (`m = [external.getHostEnvironmentValue,
/// …]; m[0].call(external, name)`).
fn host_environment_value(leaf: &LeafView) -> bool {
    has_transform(leaf, ".getHostEnvironmentValue()")
        || has_api(leaf, "getHostEnvironmentValue()")
        || (has_api(leaf, "getHostEnvironmentValue") && has_transform(leaf, ".call()"))
}

pub(crate) fn recognize_host_environment(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // The host value is written as a string; a number whose provenance
    // merely passes through the call (a shared producer's checksum) is not it.
    let strings: Vec<&LeafView> = leaves_of(view).into_iter().filter(|leaf| leaf.op == LeafOp::String).collect();
    if strings.iter().any(|leaf| host_environment_value(leaf)) && top_count_is(&view.shapes(), 2) {
        return claim(&["value is external.getHostEnvironmentValue()", "two fields"]);
    }
    None
}

pub(crate) fn bind_host_environment(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // Each record writes the value or the error message as a string and a
    // marker byte in the other field's place.
    let mut binding = FieldBinding::new();
    for message in 0..layout.messages.len() {
        let fields = message_fields(view, layout, message)?;
        let success = fields.iter().any(|f| f.leaves.iter().any(host_environment_value));
        let failure = fields.iter().any(|f| f.class(SourceClass::CaughtError));
        if success == failure {
            return Err(BindRefusal::new(format!(
                "HostEnvironment message {message}: neither (or both) host value and caught error"
            )));
        }
        let value = move |f: &Field| {
            if success {
                f.writes(LeafKind::String) && f.leaves.iter().any(host_environment_value)
            } else {
                f.is_plain(LeafKind::Byte)
            }
        };
        let error = move |f: &Field| {
            if failure {
                f.writes(LeafKind::String) && f.class(SourceClass::CaughtError)
            } else {
                f.is_plain(LeafKind::Byte)
            }
        };
        solve(
            message,
            &fields,
            &[("hostEnvironmentValue", &value), ("errorMessage", &error)],
            &mut binding,
        )?;
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `HostEnvironment`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "HostEnvironment",
    summary: "If window.external has a getHostEnvironmentValue method (a non-standard host API of legacy Microsoft Edge / Windows web hosts), calls it once with 16 keys: browser-name, browser-platform-version, browser-version, device-sku, device-dpi, device-logical-processors, device-max-touch-points, device-pixel-height, device-pixel-width, device-role, os-architecture, os-build, os-language, os-manufacturer, os-mode, os-sku. It sends the returned text unchanged, or the error message if the call throws. On browsers without that method nothing is sent.",
    why: "Likely a detector for legacy Edge and Windows-hosted web views, and where available a rich device/OS fingerprint (OS build, device model, screen, CPU count) that is hard to fake consistently.",
    sample_payload: None,
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "getHostEnvironmentValue returned a value",
            fields: &[
                CardField::string(
                    "hostEnvironmentValue",
                    "The value returned by getHostEnvironmentValue for the 16 keys, as-is (normally a JSON-formatted string).",
                ),
                CardField::null("errorMessage", "Always a null marker in this message (no error)."),
            ],
        },
        CardMessage {
            when: "the call (or writing its result) threw",
            fields: &[
                CardField::null("hostEnvironmentValue", "Always a null marker in this message (no value)."),
                CardField::string("errorMessage", "The exception's message, or 'No message' if it had none."),
            ],
        },
    ]),
};
