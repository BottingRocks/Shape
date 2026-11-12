//! `NavigatorProperties`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, LeafKind, SignalLayout, SlotView, ValueSlice};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// Members read by the branch-joined value a guard tests: `let v; if (…)
/// { v = o.p; } else { v = null; } if (v == null) …`. The tested local is
/// declared without an initializer, outside any loop or function around the
/// test; the slice of each non-literal write before the test is merged (the
/// null arm adds nothing).
fn joined_guard_slice(ctx: &FinderCtx, test: oxc_semantic::NodeId, hops: usize) -> ValueSlice {
    use oxc_ast::ast::Expression;
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let scoping = ctx.parse.scoping;
    let test_start = nodes.kind(test).span().start;
    let mut merged = ValueSlice::default();
    let tested = ctx.slice_of(test, 0);
    for (symbol, depth) in &tested.symbols {
        if *depth != 0 {
            continue;
        }
        let AstKind::VariableDeclarator(declarator) = nodes.kind(scoping.symbol_declaration(*symbol)) else {
            continue;
        };
        if declarator.init.is_some() {
            continue;
        }
        // No loop between the declaration and the test: a later write
        // could otherwise reach the test on a next iteration.
        let declared = declarator.span;
        let mut looped = false;
        let mut node = test;
        loop {
            let parent = nodes.parent_id(node);
            if parent == node {
                break;
            }
            node = parent;
            let span = nodes.kind(node).span();
            if span.start <= declared.start && declared.end <= span.end {
                break;
            }
            if matches!(
                nodes.kind(node),
                AstKind::ForStatement(_)
                    | AstKind::WhileStatement(_)
                    | AstKind::DoWhileStatement(_)
                    | AstKind::ForInStatement(_)
                    | AstKind::ForOfStatement(_)
                    | AstKind::Function(_)
                    | AstKind::ArrowFunctionExpression(_)
            ) {
                looped = true;
                break;
            }
        }
        if looped {
            continue;
        }
        let mut values = Vec::new();
        let mut plain = true;
        for reference in scoping.get_resolved_references(*symbol) {
            // Writes after the test (its own arms, later reuse) do not reach it.
            if !reference.is_write() || nodes.kind(reference.node_id()).span().start > test_start {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = nodes.kind(nodes.parent_id(reference.node_id())) else {
                plain = false;
                break;
            };
            if !assignment.operator.is_assign() {
                plain = false;
                break;
            }
            if assignment.span.end > test_start {
                plain = false;
                break;
            }
            if !matches!(
                assignment.right.without_parentheses(),
                Expression::NullLiteral(_) | Expression::NumericLiteral(_) | Expression::BooleanLiteral(_)
            ) {
                values.push(assignment.right.span());
            }
        }
        if !plain {
            continue;
        }
        for value in values {
            if let Some(node) = ctx.node_with_span(value) {
                merged.absorb(&ctx.slice_of(node, hops));
            }
        }
    }
    merged
}

/// Navigator/window/screen property names that only this census writes.
const NAVIGATOR_CENSUS_PROPERTIES: &[&str] = &[
    ".appCodeName",
    ".appVersion",
    ".oscpu",
    ".productSub",
    ".vendorSub",
    ".buildID",
    ".cpuClass",
    ".doNotTrack",
    ".maxTouchPoints",
    ".screenX",
    ".screenY",
    ".pixelDepth",
];

/// `NavigatorProperties`. Card: [`CARD`].
pub(crate) fn recognize_navigator_properties(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let named = NAVIGATOR_CENSUS_PROPERTIES
        .iter()
        .filter(|property| any_transform(&leaves, property))
        .count();
    let wide = max_top_count(&view.shapes()) >= 25;
    if named >= 5 && wide {
        return claim(&["25+ fields", "values read 5+ navigator/screen census properties"]);
    }
    // Values passed through program helpers lose their provenance; the
    // write sites' slices still read the census properties.
    if wide && named >= 2 {
        let mut read = std::collections::BTreeSet::new();
        for leaf in first_emission_leaves(view) {
            if let Some(slice) = ctx.value_slice(leaf, 3) {
                for property in NAVIGATOR_CENSUS_PROPERTIES {
                    if has_transform(leaf, property) || slice.members.contains_key(&property[1..]) {
                        read.insert(*property);
                    }
                }
            }
        }
        if read.len() >= 5 {
            return claim(&["25+ fields", "write sites read 5+ navigator/screen census properties"]);
        }
    }
    None
}

pub(crate) fn bind_navigator_properties(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // The values are copied through one merged object and shared registers,
    // so leaf provenance carries several property names per field. The
    // property a field writes is the member read nearest to its own write
    // site; a marker-only field (the value went into the flags byte) is
    // named by the member its guard tests.
    const PROPERTIES: &[(&str, &str)] = &[
        ("innerWidth", "innerWidth"),
        ("innerHeight", "innerHeight"),
        ("outerWidth", "outerWidth"),
        ("outerHeight", "outerHeight"),
        ("screenX", "screenX"),
        ("screenY", "screenY"),
        ("devicePixelRatio", "devicePixelRatio"),
        ("isSecureContext", "isSecureContext"),
        ("height", "screenHeight"),
        ("width", "screenWidth"),
        ("availWidth", "availWidth"),
        ("availHeight", "availHeight"),
        ("colorDepth", "colorDepth"),
        ("pixelDepth", "pixelDepth"),
        ("appCodeName", "appCodeName"),
        ("appName", "appName"),
        ("appVersion", "appVersion"),
        ("buildID", "buildID"),
        ("cpuClass", "cpuClass"),
        ("hardwareConcurrency", "hardwareConcurrency"),
        ("maxTouchPoints", "maxTouchPoints"),
        ("platform", "platform"),
        ("product", "product"),
        ("productSub", "productSub"),
        ("oscpu", "oscpu"),
        ("userAgent", "userAgent"),
        ("vendor", "vendor"),
        ("vendorSub", "vendorSub"),
        ("doNotTrack", "doNotTrack"),
        ("webdriver", "webdriver"),
        ("locationbar", "locationbarVisible"),
        ("toolbar", "toolbarVisible"),
        ("opera", "operaVersion"),
    ];
    fn mapped<'n>(names: impl IntoIterator<Item = &'n str>) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = names
            .into_iter()
            .filter_map(|name| PROPERTIES.iter().find(|(property, _)| *property == name).map(|(_, card)| *card))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
    let mut binding = FieldBinding::new();
    for message in 0..layout.messages.len() {
        let fields = message_fields(view, layout, message)?;
        let mut slices = Vec::new();
        let mut owns: Vec<Vec<&'static str>> = Vec::new();
        for field in &fields {
            let mut slice = field_slice(ctx, field, 3);
            let mut own = mapped(slice.nearest_members());
            if own.is_empty() && matches!(field.node, LayoutNode::Choice { .. }) {
                // Marker-only choice: the innermost guard shared by every leaf.
                if let Some(test) = guard_test_of(ctx, field) {
                    slice = ctx.slice_of(test, 3);
                    own = mapped(slice.nearest_members());
                    if own.is_empty() {
                        own = mapped(
                            slice.strings.iter().filter(|(_, depth)| **depth == 0).map(|(name, _)| name.as_str()),
                        );
                    }
                    if own.is_empty() {
                        // The guard tests a branch-joined local.
                        let joined = joined_guard_slice(ctx, test, 3);
                        own = mapped(joined.nearest_members());
                        if !own.is_empty() {
                            slice = joined;
                        }
                    }
                }
            }
            slices.push(slice);
            owns.push(own);
        }
        let flags = |field: &Field| {
            // A computed packed byte can have literal-only input classes
            // after branch joins (the branches select literal flag bits).
            // A missing literal byte, rather than provenance classes, proves
            // this is the dynamic flags field among the census properties.
            matches!(field.node, LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte && leaf.byte.is_none())
        };
        for (field, own) in fields.iter().zip(&owns) {
            let name = if flags(field) {
                "flags"
            } else {
                match own.as_slice() {
                    [name] => *name,
                    _ => {
                        return Err(BindRefusal::new(format!(
                            "NavigatorProperties: message {message}: field {} reads {own:?} nearest to its write site",
                            field.key
                        )))
                    }
                }
            };
            if binding.names().any(|bound| bound == name) {
                return Err(BindRefusal::new(format!(
                    "NavigatorProperties: message {message}: {name} is written by two fields"
                )));
            }
            binding.insert(message, field.key.clone(), name);
        }
        // A two-byte null marker (tag 160+): the missing-value arm writes a
        // record of two literal bytes, the tag and then 1.
        // Presence fields nest a choice (absent / present but null).
        fn two_byte_markers(node: &LayoutNode, message: usize, binding: &mut FieldBinding) {
            let LayoutNode::Choice { arms } = node else {
                return;
            };
            let literal = |node: &LayoutNode| match node {
                LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte => leaf.byte,
                _ => None,
            };
            for arm in arms {
                match &arm.node {
                    LayoutNode::Record { fields: inner } => match inner.as_slice() {
                        [marker, flag]
                            if literal(&marker.node).is_some_and(|byte| byte >= 128)
                                && literal(&flag.node) == Some(1) =>
                        {
                            binding.insert(message, marker.key.clone(), "nullMarker");
                            binding.insert(message, flag.key.clone(), "nullFlag");
                        }
                        _ => {}
                    },
                    nested @ LayoutNode::Choice { .. } => two_byte_markers(nested, message, binding),
                    _ => {}
                }
            }
        }
        for field in &fields {
            two_byte_markers(field.node, message, &mut binding);
        }
        // Anything else nested in a marker arm has no card name.
        let mut unbound: Vec<&str> = layout
            .keys(message)
            .into_iter()
            .filter(|key| binding.get(message, key).is_none())
            .filter_map(|key| binding.get(message, key.split('.').next().unwrap_or_default()))
            .collect();
        unbound.dedup();
        if !unbound.is_empty() {
            return Err(BindRefusal::new(format!(
                "NavigatorProperties: all {} top-level fields named at their write sites, but nested \
                 fields of {unbound:?} are not a two-byte null marker (tag + 1)",
                fields.len()
            )));
        }
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `NavigatorProperties`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "NavigatorProperties",
    summary: "Copies a fixed list of basic browser properties and sends them all in one message at startup: window size and position values, screen size and colour depth, the navigator identity strings (userAgent, platform, vendor, appVersion and so on), CPU core count, touch points, doNotTrack, navigator.webdriver, isSecureContext, whether the location bar and toolbar are visible, and Opera's version() if it exists. Missing values are sent as a null marker. The four yes/no values are packed into one flags byte.",
    why: "These are the core fingerprint values. A vendor likely cross-checks them against each other and against the HTTP headers (e.g. userAgent vs platform vs screen size), and navigator.webdriver = true or hidden toolbars directly point to automation or headless browsers.",
    sample_payload: Some(r#"{
  "appCodeName": "Mozilla",
  "appName": "Netscape",
  "appVersion": "5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
  "availHeight": 1050,
  "availWidth": 1920,
  "buildID": null,
  "colorDepth": 24,
  "cpuClass": null,
  "devicePixelRatio": 2,
  "doNotTrack": null,
  "flags": {
    "isSecureContext": true,
    "locationbarVisible": true,
    "toolbarVisible": true,
    "webdriver": false
  },
  "hardwareConcurrency": 18,
  "innerHeight": 287,
  "innerWidth": 1920,
  "maxTouchPoints": 0,
  "operaVersion": null,
  "oscpu": null,
  "outerHeight": 1050,
  "outerWidth": 1920,
  "pixelDepth": 24,
  "platform": "MacIntel",
  "product": "Gecko",
  "productSub": "20030107",
  "screenHeight": 1080,
  "screenWidth": 1920,
  "screenX": 673,
  "screenY": -1050,
  "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
  "vendor": "Google Inc.",
  "vendorSub": ""
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "innerWidth",
            "window.innerWidth, viewport width in CSS pixels, or a null marker if missing.",
        ).nullable_pair(),
        CardField::number("screenHeight", "screen.height in CSS pixels, or a null marker if missing.").nullable_pair(),
        CardField::null(
            "locationbarVisible",
            "window.locationbar.visible. When it is a boolean nothing is written here and its value goes into the flags byte; otherwise a null marker is written.",
        ).nullable_pair(),
        CardField::number(
            "outerWidth",
            "window.outerWidth, browser window width in pixels, or a null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "oscpu",
            "navigator.oscpu (Firefox only, e.g. \"Windows NT 10.0; Win64; x64\"), or a null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "availWidth",
            "screen.availWidth, screen width minus OS bars, or a null marker if missing.",
        ).nullable_pair(),
        CardField::string("userAgent", "navigator.userAgent, or a null marker if missing.").nullable_pair(),
        CardField::null(
            "webdriver",
            "Whether navigator.webdriver === true (automation). When present nothing is written here and the value goes into the flags byte; one null marker if navigator has no webdriver property, another if it is present but null.",
        ).nullable_pair(),
        CardField::number(
            "innerHeight",
            "window.innerHeight, viewport height in CSS pixels, or a null marker if missing.",
        ).nullable_pair(),
        CardField::number("colorDepth", "screen.colorDepth, usually 24 or 30, or a null marker if missing.").nullable_pair(),
        CardField::string(
            "appCodeName",
            "navigator.appCodeName, normally \"Mozilla\", or a null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "appVersion",
            "navigator.appVersion, the user agent without the \"Mozilla/\" prefix, or a null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "platform",
            "navigator.platform, e.g. \"Win32\", \"MacIntel\", \"Linux x86_64\", or a null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "productSub",
            "navigator.productSub, \"20030107\" in Chrome/Safari, \"20100101\" in Firefox, or a null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "pixelDepth",
            "screen.pixelDepth, usually equal to colorDepth, or a null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "cpuClass",
            "navigator.cpuClass (old Internet Explorer only), or a null marker if missing (the normal case).",
        ).nullable_pair(),
        CardField::string("vendorSub", "navigator.vendorSub, normally empty, or a null marker if missing.").nullable_pair(),
        CardField::null(
            "isSecureContext",
            "window.isSecureContext (page loaded over HTTPS or localhost). When it is a boolean nothing is written here and its value goes into the flags byte; otherwise a null marker is written.",
        ).nullable_pair(),
        CardField::string(
            "vendor",
            "navigator.vendor, e.g. \"Google Inc.\", \"Apple Computer, Inc.\" or empty in Firefox, or a null marker if missing.",
        ).nullable_pair(),
        CardField::string("product", "navigator.product, normally \"Gecko\", or a null marker if missing.").nullable_pair(),
        CardField::string(
            "appName",
            "navigator.appName, normally \"Netscape\", or a null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "screenX",
            "window.screenX, window's left position on the screen, or a null marker if missing.",
        ).nullable_pair(),
        CardField::null(
            "toolbarVisible",
            "window.toolbar.visible. When it is a boolean nothing is written here and its value goes into the flags byte; otherwise a null marker is written.",
        ).nullable_pair(),
        CardField::byte(
            "flags",
            "Packed booleans, 1 = true: locationbar visible, navigator.webdriver true, isSecureContext, toolbar visible. A bit is 0 when its value was missing.",
        )
        .flags(&["locationbarVisible", "webdriver", "isSecureContext", "toolbarVisible"]),
        CardField::number(
            "maxTouchPoints",
            "navigator.maxTouchPoints, 0 on non-touch devices, often 5 or 10 on touch screens, or a null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "hardwareConcurrency",
            "navigator.hardwareConcurrency, number of logical CPU cores, or a null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "buildID",
            "navigator.buildID (Firefox only, usually \"20181001000000\"), or a null marker if missing.",
        ).nullable_pair(),
        CardField::number("screenWidth", "screen.width in CSS pixels, or a null marker if missing.").nullable_pair(),
        CardField::number(
            "devicePixelRatio",
            "window.devicePixelRatio, e.g. 1, 1.25, 2 or 3. One null marker if the property is absent, another if it is present but null.",
        ).nullable_pair(),
        CardField::string(
            "doNotTrack",
            "navigator.doNotTrack, e.g. \"1\", \"0\" or \"unspecified\". One null marker if the property is absent, another if it is present but null.",
        ).nullable_pair(),
        CardField::string(
            "operaVersion",
            "Result of window.opera.version() (old Opera only), or a null marker if it does not exist (the normal case).",
        ).nullable_pair(),
        CardField::number(
            "outerHeight",
            "window.outerHeight, browser window height in pixels, or a null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "availHeight",
            "screen.availHeight, screen height minus OS bars, or a null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "screenY",
            "window.screenY, window's top position on the screen, or a null marker if missing.",
        ).nullable_pair(),
        CardField::null(
            "nullMarker",
            "The alternative written in place of a property's value when it is missing, for the properties whose null-marker tag takes two bytes (tag 160 and above, about the last 5-6 null-capable properties of the build): this first byte is the tag. Properties with a smaller tag write a single null-marker byte, described with each property.",
        ),
        CardField::null(
            "nullFlag",
            "Second byte of a two-byte null marker, always 1, written right after nullMarker.",
        ),
    ]),
};
