//! `NavigatorPlugins`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `NavigatorPlugins`. Card: [`CARD`].
pub(crate) fn recognize_navigator_plugins(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let repeat_string = leaves.iter().any(|leaf| leaf.repeat_depth == 1 && leaf.op == LeafOp::String);
    let repeat_number = leaves.iter().any(|leaf| leaf.repeat_depth == 1 && leaf.op == LeafOp::Number);
    let length = leaves.iter().any(|leaf| {
        leaf.repeat_depth == 0 && has_api(leaf, "navigator.plugins") && has_transform(leaf, ".length")
    });
    if length && repeat_string && repeat_number && top_count_is(&view.shapes(), 3) {
        return claim(&["navigator.plugins.length", "repeat of {STRING, NUMBER}"]);
    }
    None
}

pub(crate) fn bind_navigator_plugins(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binding = FieldBinding::new();
    for message in 0..layout.messages.len() {
        let fields = message_fields(view, layout, message)?;
        let plugins_length = |f: &Field| f.is_plain(LeafKind::Number) && f.api("navigator.plugins");
        let entry_count = |f: &Field| f.is_plain(LeafKind::Number) && !f.api("navigator.plugins");
        let plugin = |f: &Field| f.is_array();
        solve(
            message,
            &fields,
            &[("pluginsLength", &plugins_length), ("entryCount", &entry_count), ("plugin", &plugin)],
            &mut binding,
        )?;
        for field in fields.iter().filter(|field| field.is_array()) {
            let item = field
                .item_fields()
                .ok_or_else(|| BindRefusal::new("NavigatorPlugins: plugin item is not a record"))?;
            let name_prefix = |f: &Field| f.is_plain(LeafKind::String);
            let details_hash = |f: &Field| f.is_plain(LeafKind::Number);
            solve(
                message,
                &item,
                &[("namePrefix", &name_prefix), ("detailsHash", &details_hash)],
                &mut binding,
            )?;
        }
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `NavigatorPlugins`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "NavigatorPlugins",
    summary: "Summarises navigator.plugins. For each of the first five plugins it sends the first 12 characters of the plugin's name and a hash of all its details (name, description, filename, version and the type, suffixes and description of each of its MIME types). It also sends how many plugins it summarised and the full navigator.plugins.length. A plugin whose details cannot be read is skipped.",
    why: "The plugin list is a classic browser fingerprint: real Chrome, Edge, Firefox and Safari each expose a known fixed list (e.g. the five PDF viewer entries), while headless browsers and some bots likely expose none or a fake list.",
    sample_payload: Some(r#"{
  "plugin": [
    {
      "detailsHash": -1716625998,
      "namePrefix": "WebKit built"
    },
    {
      "detailsHash": 1387368353,
      "namePrefix": "Microsoft Ed"
    },
    {
      "detailsHash": 2126051627,
      "namePrefix": "Chromium PDF"
    },
    {
      "detailsHash": 1061675194,
      "namePrefix": "Chrome PDF V"
    },
    {
      "detailsHash": 975495497,
      "namePrefix": "PDF Viewer"
    }
  ],
  "pluginsLength": 5
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "entryCount",
            "Number of plugin entries that follow, normally min(plugins.length, 5); can be lower if a plugin could not be read.",
        ),
        CardField::group("plugin", &[
            CardField::string(
                "namePrefix",
                "First 12 characters of the plugin name, e.g. \"PDF Viewer\" or \"Chrome PDF V\".",
            ),
            CardField::number(
                "detailsHash",
                "Signed 32-bit hash of the plugin's name, description, filename, version and its MIME types' type/suffixes/description, serialised as JSON.",
            ),
        ])
        .repeat("once per summarised plugin, entryCount times (last plugin first)"),
        CardField::number(
            "pluginsLength",
            "navigator.plugins.length, not capped (0 in many headless browsers, 5 in current desktop Chrome).",
        ),
    ]),
};
