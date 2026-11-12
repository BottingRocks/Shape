//! `UserAgentData`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, LeafKind, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, layout_nodes::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// Member names and string literals the field's innermost shared guard
/// tests directly (depth 0 of its slice): `hasOwn(o, "wow64")`, `o.wow64`.
fn guard_names(ctx: &FinderCtx, field: &Field) -> Vec<String> {
    let Some(test) = guard_test_of(ctx, field) else {
        return Vec::new();
    };
    let slice = ctx.slice_of(test, 0);
    slice
        .members
        .iter()
        .chain(slice.strings.iter())
        .filter(|(_, depth)| **depth == 0)
        .map(|(name, _)| name.clone())
        .collect()
}

/// High-entropy client-hint names read from the per-hint result object.
const CLIENT_HINTS: &[&str] = &[
    ".platformVersion",
    ".uaFullVersion",
    ".fullVersionList",
    ".architecture",
    ".bitness",
    ".model",
];

/// `UserAgentData`. Card: [`CARD`].
pub(crate) fn recognize_user_agent_data(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let hints = CLIENT_HINTS.iter().filter(|hint| any_transform(&leaves, hint)).count();
    if hints >= 4 {
        return claim(&["values read 4+ high-entropy client hints"]);
    }
    None
}

pub(crate) fn bind_user_agent_data(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // Each hint is one top-level field: the value (or count + entries) when
    // present, else a null marker. The two booleans (wow64, mobile) leave a
    // `CHOICE{empty|marker}` placeholder at their place (named by the hint
    // its guard tests) and go into the flags byte.
    let mut binding = FieldBinding::new();
    for message in 0..layout.messages.len() {
        let fields = message_fields(view, layout, message)?;
        let tested: std::collections::HashMap<&str, Vec<String>> = fields
            .iter()
            .filter(|f| matches!(f.node, LayoutNode::Choice { .. }) && f.literal_only() && !f.writes(LeafKind::Byte))
            .map(|f| (f.key.as_str(), guard_names(ctx, f)))
            .collect();
        let placeholder = |hint: &'static str| {
            let tested = &tested;
            move |f: &Field| tested.get(f.key.as_str()).is_some_and(|names| names.iter().any(|name| name == hint))
        };
        let wow64_placeholder = placeholder("wow64");
        let mobile_placeholder = placeholder("mobile");
        let hint = |name: &'static str, kind: LeafKind| {
            move |f: &Field| !f.is_plain(LeafKind::Byte) && f.writes(kind) && f.tr(name)
        };
        // The packed byte's provenance can lose the booleans through its
        // re-used register; its write-site slice still reads them.
        // Every other card field is a value/marker choice or a placeholder
        // choice: a lone plain computed byte is the flags byte.
        let plain_bytes = fields
            .iter()
            .filter(|f| f.is_plain(LeafKind::Byte) && !f.literal_only())
            .count();
        let flags = |f: &Field| {
            f.is_plain(LeafKind::Byte)
                && (f.tr(".wow64") || f.tr(".mobile") || (plain_bytes == 1 && !f.literal_only()) || {
                    let slice = field_slice(ctx, f, 4);
                    slice.members.contains_key("wow64") || slice.members.contains_key("mobile")
                })
        };
        let brands = hint(".brands", LeafKind::Number);
        let full_version_list = hint(".fullVersionList", LeafKind::Number);
        let platform_version = hint(".platformVersion", LeafKind::String);
        let architecture = hint(".architecture", LeafKind::String);
        let bitness = hint(".bitness", LeafKind::String);
        let platform = hint(".platform", LeafKind::String);
        let ua_full_version = hint(".uaFullVersion", LeafKind::String);
        let model = hint(".model", LeafKind::String);
        solve(
            message,
            &fields,
            &[
                ("flags", &flags),
                ("brands", &brands),
                ("fullVersionList", &full_version_list),
                ("platformVersion", &platform_version),
                ("architecture", &architecture),
                ("bitness", &bitness),
                ("platform", &platform),
                ("uaFullVersion", &ua_full_version),
                ("model", &model),
                ("wow64Placeholder", &wow64_placeholder),
                ("mobilePlaceholder", &mobile_placeholder),
            ],
            &mut binding,
        )?;
        // Each list hint arm is a record {count, entries}.
        for field in &fields {
            let LayoutNode::Choice { arms } = field.node else {
                continue;
            };
            let mut at = 0usize;
            for arm in arms {
                let count = node_leaf_count(&arm.node);
                if let LayoutNode::Record { fields: inner } = &arm.node {
                    let inner = split_fields(inner, &field.leaves[at..at + count]);
                    // Named by the list the field itself was bound to (the
                    // leaves' provenance can carry both list names).
                    let (count_name, entries_name) = match binding.get(message, &field.key) {
                        Some("brands") => ("brands", "brandEntries"),
                        Some("fullVersionList") => ("fullVersionList", "fullVersionEntries"),
                        _ => return Err(BindRefusal::new("UserAgentData: list record under a non-list field")),
                    };
                    let count_rule = |f: &Field| f.is_plain(LeafKind::Number);
                    let entries_rule = |f: &Field| f.is_array();
                    solve(
                        message,
                        &inner,
                        &[(count_name, &count_rule), (entries_name, &entries_rule)],
                        &mut binding,
                    )?;
                    for entries in inner.iter().filter(|f| f.is_array()) {
                        let pair = entries
                            .item_fields()
                            .ok_or_else(|| BindRefusal::new("UserAgentData: list entry is not a record"))?;
                        // The key the entry's write site reads (`list[i].brand`);
                        // provenance alone can carry both keys.
                        let read = |f: &Field| -> Option<String> {
                            let mut keys: Vec<String> = f
                                .leaves
                                .iter()
                                .map(|leaf| ctx.caller_value_node(leaf).and_then(|node| ctx.read_key(node)))
                                .collect::<Option<_>>()?;
                            keys.dedup();
                            (keys.len() == 1).then(|| keys.remove(0))
                        };
                        let entry_key = |f: &Field, key: &str, tr: &str| match read(f) {
                            Some(found) => found == key,
                            None => f.tr(tr),
                        };
                        let brand = |f: &Field| entry_key(f, "brand", ".brand");
                        let version = |f: &Field| entry_key(f, "version", ".version");
                        solve(message, &pair, &[("brand", &brand), ("version", &version)], &mut binding)?;
                    }
                }
                at += count;
            }
        }
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `UserAgentData`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "UserAgentData",
    summary: "Asks the User-Agent Client Hints API (navigator.userAgentData.getHighEntropyValues) for ten hints, one request per hint, and waits for all of them. Each hint that comes back is written; each one that is missing or rejected is replaced by a null marker. The two booleans (wow64, mobile) are packed into one flags byte. Browsers without userAgentData send nothing.",
    why: "Client hints give the exact browser brand/version, OS version, CPU architecture and device model, which can be checked against the User-Agent string to catch spoofed or headless browsers (likely).",
    sample_payload: Some(r#"{
  "architecture": "arm",
  "bitness": "64",
  "brands": [
    {
      "brand": "Google Chrome",
      "version": "156"
    },
    {
      "brand": "Chromium",
      "version": "156"
    },
    {
      "brand": "Not:A-Brand",
      "version": "8"
    }
  ],
  "flags": {
    "mobile": false,
    "wow64": false
  },
  "fullVersionList": [
    {
      "brand": "Google Chrome",
      "version": "156.0.8078.81"
    },
    {
      "brand": "Chromium",
      "version": "156.0.8078.81"
    },
    {
      "brand": "Not:A-Brand",
      "version": "8.0.0.0"
    }
  ],
  "model": "",
  "platform": "macOS",
  "platformVersion": "27.0.1",
  "uaFullVersion": "156.0.8078.81"
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "brands",
            "Number of entries in the low-entropy brand list (e.g. Chromium, Google Chrome, Not A Brand), or a null marker if the hint is missing.",
        ).nullable(),
        CardField::group("brandEntries", &[
            CardField::string("brand", "Brand name, e.g. \"Google Chrome\"."),
            CardField::string("version", "Major version for that brand, e.g. \"120\"."),
        ])
        .repeat("once per brand in the list (only when brands is present)"),
        CardField::string(
            "platformVersion",
            "OS version, e.g. \"15.0.0\" or \"10.0.0\", or a null marker if missing.",
        ).nullable(),
        CardField::string(
            "architecture",
            "CPU architecture, e.g. \"x86\" or \"arm\" (empty string if unknown), or a null marker if missing.",
        ).nullable(),
        CardField::number(
            "fullVersionList",
            "Number of entries in the full-version brand list, or a null marker if missing.",
        ).nullable(),
        CardField::group("fullVersionEntries", &[
            CardField::string("version", "Full version for that brand, e.g. \"120.0.6099.130\"."),
            CardField::string("brand", "Brand name; usually a repeat of a name already sent in brands."),
        ])
        .repeat("once per entry in fullVersionList (only when present)"),
        CardField::string(
            "bitness",
            "CPU bitness, \"64\" or \"32\" (or empty), or a null marker if missing.",
        ).nullable(),
        CardField::string(
            "platform",
            "OS name, e.g. \"Windows\", \"macOS\", \"Android\", or a null marker if missing.",
        ).nullable(),
        CardField::null(
            "wow64Placeholder",
            "Written at wow64's place in the hint list only when the wow64 hint is missing: a null marker byte. When wow64 is present nothing is written here and its value goes into the flags byte.",
        ),
        CardField::null(
            "mobilePlaceholder",
            "Written at mobile's place in the hint list only when the mobile hint is missing: a null marker byte. When mobile is present nothing is written here and its value goes into the flags byte.",
        ),
        CardField::byte(
            "flags",
            "Packed booleans: wow64 (32-bit browser on 64-bit Windows) and mobile (mobile device). A flag is 0 when its hint is missing; then a null marker is also written at that hint's place in the list.",
        )
        .flags(&["wow64", "mobile"]),
        CardField::string(
            "uaFullVersion",
            "Full browser version (deprecated hint), e.g. \"120.0.6099.130\", or a null marker if missing.",
        ).nullable(),
        CardField::string(
            "model",
            "Device model, e.g. \"Pixel 7\"; empty string on desktop, or a null marker if missing.",
        ).nullable(),
    ]),
};
