//! `UiEventSamplesMouse`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{top_level_ops, BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, shape_text::*, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `UiEventSamplesMouse`. Card: [`CARD`].
pub(crate) fn recognize_ui_event_samples_mouse(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // Three sections: count + record list each.
    if top_level_ops(view).len() != 3 || repeat_count(view) != 3 || max_repeat_depth(view) != 1 {
        return None;
    }
    let facts = slot_facts(view, ctx);
    let mouse_roles = ctx
        .slot_roles(view)
        .iter()
        .any(|role| role.starts_with("interaction_mouse") || role.starts_with("interaction_throttled_mouse"));
    (facts.has_event(&["mousemove"]) || mouse_roles)
        .then(|| Claim::new(["mouse listeners fill three record rings", "three count + list sections"]))
}

pub(crate) fn bind_ui_event_samples_mouse(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    // Sections: the button list has strings; the throttled list is filtered
    // against the raw list. The filter's own list is the one whose written
    // values are defined from the `.filter(…)` result (definitions followed
    // back); the raw list is only read inside the filter's callback, which
    // the merged leaf descriptions of both lists can mention.
    let filtered_by_definition = |field: &Field| {
        field.sites.iter().flatten().any(|site| {
            ctx.caller_write_value_node(site).is_some_and(|node| {
                ctx.slice_of(node, FILTER_DEFINITION_HOPS).calls.keys().any(|(method, _, _)| method == "filter")
            })
        })
    };
    let arrays: Vec<&Field> = binder.messages.iter().flatten().filter(|field| field.shape == Shape::Array).collect();
    let by_definition = arrays.iter().filter(|field| filtered_by_definition(field)).count() == 1
        && arrays.iter().filter(|field| field.merged().has_call("filter")).count() > 1;
    let section = |field: &Field| -> Option<&'static str> {
        if field.leaf_kinds().contains(&LeafKind::String) {
            Some("button")
        } else if if by_definition { filtered_by_definition(field) } else { field.merged().has_call("filter") } {
            Some("throttled")
        } else {
            Some("raw")
        }
    };
    binder.top_sections(section, &[("button", "buttonEventCount", "buttonEvents"), ("throttled", "throttledMoveCount", "throttledMoves"), ("raw", "rawMoveCount", "rawMoves")])?;
    binder.records(|records, facts| {
        records.name_by_labels(facts, |field, meaning| {
            let coordinate = |axis: &str| meaning.has_member(&format!("page{axis}"));
            let relative = |axis: &str| meaning.has_member(&format!("client{axis}")) || meaning.has_member(&format!("offset{axis}"));
            match field.kind() {
                Some(LeafKind::Byte) => Some("flags"),
                Some(LeafKind::String) if meaning.has_member("name") && !meaning.has_member("id") => Some("targetName"),
                Some(LeafKind::String) if meaning.has_member("id") && !meaning.has_member("name") => Some("targetId"),
                Some(LeafKind::Number) => {
                    if meaning.has_member("button") {
                        Some("button")
                    } else if meaning.has_member("offsetHeight") || meaning.has_member("height") {
                        Some("targetHeight")
                    } else if meaning.has_member("offsetWidth") || meaning.has_member("width") {
                        Some("targetWidth")
                    } else if relative("X") && !coordinate("X") {
                        Some("offsetX")
                    } else if relative("Y") && !coordinate("Y") {
                        Some("offsetY")
                    } else if coordinate("X") && !coordinate("Y") {
                        Some("pageX")
                    } else if coordinate("Y") && !coordinate("X") {
                        Some("pageY")
                    } else if is_clock_value(meaning) {
                        Some("time")
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
    })?;
    binder.finish()
}

/// Definition hops from a mouse list's written values to its `.filter(…)`.
const FILTER_DEFINITION_HOPS: usize = 6;

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `UiEventSamplesMouse`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "UiEventSamplesMouse",
    summary: "Records mouse activity on the document: every mousemove (raw), a second mousemove trail throttled to one sample per 130 ms, and every mousedown, mouseup and click with position, button and details of the clicked element. It keeps the newest records in rings (50 raw moves, 50 throttled moves, 30 button events) and sends all three lists in one message when the report is built. After the first record in each list, times, coordinates and sizes are sent as differences from the previous record.",
    why: "Mouse paths, timing and click placement are among the strongest human-vs-bot behaviour signals; isTrusted and instanceof UIEvent also expose synthetic events dispatched by scripts.",
    sample_payload: Some(r#"{
  "buttonEvents": [
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 0
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": true,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 66.20000004768372
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": true,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": -1089,
      "offsetY": -221,
      "pageX": -387,
      "pageY": 3,
      "targetHeight": -413,
      "targetId": "login",
      "targetName": "",
      "targetWidth": -1405,
      "time": 39779.59999990463
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 0.10000002384185791
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": true,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 107.89999997615814
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": true,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 1082,
      "offsetY": 233,
      "pageX": 380,
      "pageY": 72,
      "targetHeight": 417,
      "targetId": "",
      "targetName": null,
      "targetWidth": 1405,
      "time": 1075
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 0.5
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": true,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 98.70000004768372
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": true,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 261,
      "offsetY": 21,
      "pageX": 963,
      "pageY": 205,
      "targetHeight": 44,
      "targetId": "username",
      "targetName": "username",
      "targetWidth": 500,
      "time": 8689
    }
  ],
  "rawMoves": [
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 971,
      "pageY": 233,
      "time": 49125
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 96.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 81
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 15.799999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 7.899999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 16.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.100000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.599999904632568
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 2,
      "time": 8.100000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 4,
      "time": 8.600000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 3,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 2,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 2,
      "time": 10.100000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 6.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.200000047683716
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 0,
      "time": 8.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.199999928474426
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 34.200000047683716
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.199999928474426
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.100000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 0,
      "time": 7.899999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.200000047683716
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.200000047683716
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.199999928474426
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 26.200000047683716
    }
  ],
  "throttledMoves": [
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 8,
      "pageY": 6,
      "time": 133.29999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 32,
      "pageY": -21,
      "time": 133.60000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 43,
      "pageY": -21,
      "time": 133.39999997615814
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 30,
      "pageY": -15,
      "time": 133.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 6,
      "pageY": -12,
      "time": 133.29999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -151,
      "pageY": 152,
      "time": 131.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -27,
      "pageY": -142,
      "time": 37080.200000047684
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 0,
      "time": 216.70000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": -1,
      "time": 179.79999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -9,
      "pageY": -5,
      "time": 179.10000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -30,
      "pageY": -47,
      "time": 133.39999997615814
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -33,
      "pageY": 0,
      "time": 133
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -108,
      "pageY": 35,
      "time": 133.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -133,
      "pageY": 26,
      "time": 139.29999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 3,
      "pageY": 0,
      "time": 302.2999999523163
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 47,
      "pageY": 5,
      "time": 133.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 207,
      "pageY": -21,
      "time": 131.60000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 122,
      "pageY": -30,
      "time": 303.89999997615814
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 118,
      "time": 613
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 168
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 11,
      "pageY": -1,
      "time": 133.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 20,
      "pageY": -11,
      "time": 133.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 45,
      "pageY": -26,
      "time": 132.39999997615814
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 37,
      "pageY": -6,
      "time": 133.60000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -105,
      "pageY": 43,
      "time": 133.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 20,
      "pageY": -17,
      "time": 133.69999992847443
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 45,
      "pageY": -22,
      "time": 133.60000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 26,
      "pageY": -26,
      "time": 133
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -122,
      "pageY": 59,
      "time": 1263.2999999523163
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 0,
      "time": 245.70000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -6,
      "pageY": -25,
      "time": 132
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -3,
      "pageY": -34,
      "time": 233.79999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 996,
      "pageY": 271,
      "time": 5497.200000047684
    }
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("rawMoveCount", "Number of raw mousemove records that follow (up to 50)."),
        CardField::group("rawMoves", &[
            CardField::number(
                "pageY",
                "Mouse Y position on the page in CSS pixels (not rounded); difference from the previous record after the first. Or a null marker if missing.",
            ).nullable(),
            CardField::byte(
                "flags",
                "Two flag bits: event is an instanceof UIEvent; event.isTrusted is true.",
            )
            .flags(&["instanceOfUIEvent", "isTrusted"]),
            CardField::number(
                "time",
                "Milliseconds since the script started (usually fractional); for later records, time since the previous record.",
            ),
            CardField::number(
                "pageX",
                "Mouse X position on the page in CSS pixels (not rounded); difference from the previous record after the first. Or a null marker if missing.",
            ).nullable(),
            CardField::number(
                "pairedFields",
                "Holder, in some builds, of two neighbouring record fields written under one condition: when the first field is missing it carries that field's null marker and then the second field, otherwise both values. The two fields keep their own names inside it.",
            ),
        ])
        .repeat("once per raw mousemove record, oldest first, up to 50"),
        CardField::number(
            "throttledMoveCount",
            "Number of throttled mousemove records that follow (up to 50). Only samples older than the oldest raw move are sent.",
        ),
        CardField::group("throttledMoves", &[
            CardField::number(
                "pageX",
                "Mouse X on the page, rounded to whole pixels; difference from the previous record after the first. Or a null marker if missing.",
            ).nullable(),
            CardField::byte(
                "flags",
                "Two flag bits: event is an instanceof UIEvent; event.isTrusted is true.",
            )
            .flags(&["instanceOfUIEvent", "isTrusted"]),
            CardField::number(
                "pageY",
                "Mouse Y on the page, rounded to whole pixels; difference from the previous record after the first. Or a null marker if missing.",
            ).nullable(),
            CardField::number(
                "time",
                "Milliseconds since the script started; for later records, time since the previous record.",
            ),
            CardField::number(
                "pairedFields",
                "Holder, in some builds, of two neighbouring record fields written under one condition: when the first field is missing it carries that field's null marker and then the second field, otherwise both values. The two fields keep their own names inside it.",
            ),
        ])
        .repeat("once per throttled mousemove record (at most one per 130 ms), oldest first"),
        CardField::number(
            "buttonEventCount",
            "Number of mousedown / mouseup / click records that follow (up to 30).",
        ),
        CardField::group("buttonEvents", &[
            CardField::number(
                "targetHeight",
                "offsetHeight of the event target in pixels; difference from the previous record after the first. Or a null marker if there is no target.",
            ).nullable(),
            CardField::number(
                "button",
                "event.button: 0 left, 1 middle, 2 right (3/4 back/forward). Or a null marker if missing.",
            ).nullable(),
            CardField::number(
                "targetWidth",
                "offsetWidth of the event target in pixels; difference from the previous record after the first. Or a null marker if there is no target.",
            ).nullable(),
            CardField::string(
                "targetName",
                "The target element's name attribute. Or a marker for 'same as previous record' (omitted) or for null.",
            ),
            CardField::string(
                "targetId",
                "The target element's id. Or a marker for 'same as previous record' (omitted) or for null.",
            ),
            CardField::number(
                "pageX",
                "Click X on the page, rounded; difference from the previous record after the first. Or a null marker if missing.",
            ).nullable(),
            CardField::number(
                "offsetX",
                "Click X relative to the target's left edge, rounded (0 if the target is not an element); difference after the first record. Or a null marker if missing.",
            ).nullable(),
            CardField::number(
                "pageY",
                "Click Y on the page, rounded; difference from the previous record after the first. Or a null marker if missing.",
            ).nullable(),
            CardField::number(
                "time",
                "Milliseconds since the script started; for later records, time since the previous record.",
            ),
            CardField::byte(
                "flags",
                "Flag bits: event is an instanceof UIEvent; is mouseup; is mousedown; event.isTrusted is true. A click sets neither mouseup nor mousedown.",
            )
            .flags(&["instanceOfUIEvent", "isMouseup", "isMousedown", "isTrusted"]),
            CardField::number(
                "offsetY",
                "Click Y relative to the target's top edge, rounded (0 if the target is not an element); difference after the first record. Or a null marker if missing.",
            ).nullable(),
            CardField::number(
                "pairedFields",
                "Holder, in some builds, of two neighbouring record fields written under one condition: when the first field is missing it carries that field's null marker and then the second field, otherwise both values. The two fields keep their own names inside it.",
            ),
        ])
        .repeat("once per mousedown, mouseup or click, oldest first, up to 30"),
    ]),
};
