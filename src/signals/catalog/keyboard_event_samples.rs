//! `KeyboardEventSamples`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{top_level_ops, BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, shape_text::*, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


const KEY_EVENTS: &[&str] = &["keydown", "keyup", "keypress"];

/// `KeyboardEventSamples`. Card: [`CARD`].
pub(crate) fn recognize_keyboard_event_samples(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // RAW4 NUMBER REPEAT{record with id/name strings}.
    if top_level_ops(view) != [LeafOp::Number]
        || repeat_count(view) != 1
        || view.leaf_count(LeafOp::String) < 2
        || max_repeat_depth(view) != 1
    {
        return None;
    }
    let facts = slot_facts(view, ctx);
    let keyboard_roles = ctx
        .slot_roles(view)
        .iter()
        .any(|role| role.starts_with("interaction_key") && role.ends_with("_sample_handler") || *role == "interaction_keyboard_event_sample_recorder");
    let keyed = facts.has_event(KEY_EVENTS) || keyboard_roles;
    keyed
        .then(|| Claim::new(["keyboard listeners fill the record ring", "RAW4 NUMBER REPEAT{record with strings}"]))
}

pub(crate) fn bind_keyboard_event_samples(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| match field.shape {
        Shape::Array => Some("events"),
        Shape::Scalar(_) if field.merged().has_member("length") => Some("eventCount"),
        _ => None,
    })?;
    binder.records(|records, facts| {
        records.name_by_labels(facts, |field, meaning| {
            let own = field.merged();
            match field.kind() {
                Some(LeafKind::String) if meaning.has_member("name") && !meaning.has_member("id") => Some("targetName"),
                Some(LeafKind::String) if meaning.has_member("id") && !meaning.has_member("name") => Some("targetId"),
                Some(LeafKind::Number) if meaning.has_member("keyCode") || meaning.has_member("which") || meaning.has_member("key") => {
                    Some("keyCode")
                }
                Some(LeafKind::Number) if is_clock_value(meaning) => Some("timeMs"),
                Some(LeafKind::Byte) if own.labels.len() >= 2 || own.indexed.len() >= 2 => Some("flags"),
                // One boolean on its own byte (which one varies per build).
                Some(LeafKind::Byte) if own.labels.len() <= 1 && own.indexed.len() <= 1 => Some("ownFlag"),
                _ => None,
            }
        })
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `KeyboardEventSamples`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "KeyboardEventSamples",
    summary: "Listens to every keydown, keyup and keypress on the page and records each one: when it happened, which modifier keys were held, whether the event was real (isTrusted, a UIEvent), the id and name of the element that had focus, and a key code. Typed characters are hidden: any key that produces a character is recorded as code 1, so only special keys (Enter, Tab, arrows, F-keys, Backspace…) and Ctrl/Meta shortcuts keep their real code. It keeps the last 40 events and sends them just before the payload is sent.",
    why: "Keyboard timing, the trusted flag and the target fields show whether a human typed into the form or a script filled it with synthetic events (likely).",
    sample_payload: Some(r#"{
  "events": [
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": true,
        "isKeyup": false,
        "isTrusted": false,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": "username",
      "targetName": "username",
      "timeMs": 693.2999999523163
    },
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": false,
        "isKeyup": true,
        "isTrusted": false,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": 132,
      "targetName": 130,
      "timeMs": 0
    },
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": true,
        "isKeyup": false,
        "isTrusted": true,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": 132,
      "targetName": 130,
      "timeMs": 8662.100000023842
    },
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": false,
        "isKeyup": false,
        "isTrusted": true,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": 132,
      "targetName": 130,
      "timeMs": 0.30000007152557373
    },
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": false,
        "isKeyup": true,
        "isTrusted": true,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": 132,
      "targetName": 130,
      "timeMs": 48.699999928474426
    }
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("eventCount", "Number of keyboard event records that follow (0 to 40)."),
        CardField::group("events", &[
            CardField::string(
                "targetName",
                "The name attribute of the focused element. Or an 'unchanged' marker if it is the same as in the previous (older) record, or a null marker if the element has no name or there is no target.",
            ),
            CardField::number(
                "timeMs",
                "For the oldest record, milliseconds since the script started. For every other record, milliseconds since the previous (older) event. Usually fractional.",
            ),
            CardField::string(
                "targetId",
                "The id of the focused element. Or an 'unchanged' marker if it is the same as in the previous (older) record, or a null marker if there is no id or no target.",
            ),
            CardField::byte(
                "flags",
                "Packed flags: isKeydown, isKeyup (both 0 means keypress), Ctrl held, Alt held, Meta held, event is a UIEvent, event isTrusted.",
            )
            .flags(&[
                "isKeydown",
                "isKeyup",
                "ctrlKey",
                "altKey",
                "metaKey",
                "shiftKey",
                "instanceOfUIEvent",
                "isTrusted",
            ]),
            CardField::byte(
                "ownFlag",
                "One event boolean written as its own 0/1 byte instead of being packed into flags. Which of the eight booleans (isKeydown, isKeyup, Ctrl, Alt, Meta, Shift held, UIEvent, isTrusted) it is varies per build: Shift in the main build, others in other builds; flags then packs the remaining seven.",
            )
            .flags(&[
                "isKeydown",
                "isKeyup",
                "ctrlKey",
                "altKey",
                "metaKey",
                "shiftKey",
                "instanceOfUIEvent",
                "isTrusted",
            ]),
            CardField::number(
                "keyCode",
                "Hidden key code: 1 for any key that types a character (or an unknown key). Otherwise the real keyCode (e.g. 13 Enter, 9 Tab), or a 32-bit hash of the key name for keypress events with Ctrl/Meta.",
            ),
        ])
        .repeat("once per keyboard event, newest first, up to 40"),
    ]),
};
