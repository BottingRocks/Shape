//! Packed flag bytes whose booleans are named on their resolved values.
//!
//! The signals here pack booleans held in reused registers, closure state
//! set in another callback, or flags stored under a test. Their write-site
//! values are resolved flow-aware ([`super::bits_flow`]): every read is
//! replaced by the values of the stores that reach it, so a bit is split
//! and named from what the boolean actually holds on every path to the
//! write. Each signal has its own namer over that value (what is probed:
//! the host member, the helper key, the guard a `true` is stored under).
//!
//! * Bit positions come from the written value only (`|` / `+` terms and
//!   their shifts), per build.
//! * A bit is named only when every alternative reaching it names the same
//!   card flag; a bit with alternatives naming different flags fails the
//!   field, and a bit no alternative names is named only by elimination
//!   (the one card flag no other bit of the slot reads).
//! * Like the catalog files, nothing here reads salts, slot addresses,
//!   descriptor indices, generated spellings or build constants.

use std::collections::{BTreeMap, BTreeSet};

use oxc_span::GetSpan;

use crate::signals::flags::flow::{split, Sym};
use crate::signals::schema::{find_key, flag_leaf_offsets};
use crate::signals::analysis::{FieldBinding, FinderCtx, SignalLayout, SlotView};

/// Names one boolean: `Ok(Some(flag))`, `Ok(None)` when nothing it reads
/// names a flag, `Err` when it reads more than one.
type Namer = fn(&FinderCtx, &Sym, &[String]) -> Result<Option<String>, String>;

/// One signal's packed fields.
pub(crate) struct SignalFlags {
    pub signal: &'static str,
    pub fields: &'static [&'static str],
    /// The card fields whose `flags` lists (joined) name the bits.
    pub flags_from: &'static [&'static str],
    pub namer: Namer,
    /// Resolve with each local read kept ([`Sym::Read`]).
    pub reads: bool,
}

pub(crate) const SIGNALS: &[SignalFlags] = &[
    SignalFlags {
        signal: "GrecaptchaAccessorHookFlags",
        fields: &["hookFlagsA", "hookFlagsB"],
        flags_from: &["hookFlagsA", "hookFlagsB"],
        namer: grecaptcha,
        reads: false,
    },
    SignalFlags {
        signal: "NativeApiIntegrityFlags",
        fields: &["integrityFlags1", "integrityFlags2", "integrityFlags3"],
        flags_from: &["integrityFlags1"],
        namer: native_api,
        reads: false,
    },
    SignalFlags {
        signal: "FingerprintApiIntegrity",
        fields: &["integrityFlags1", "integrityFlags2", "integrityFlags3", "integrityFlags4", "integrityFlags5", "integrityFlags6", "integrityFlags7", "integrityFlags8", "integrityFlags9", "integrityFlags10", "integrityFlags11"],
        flags_from: &["integrityFlags1"],
        namer: fingerprint_api,
        reads: false,
    },
    SignalFlags {
        signal: "ConsoleStackAccessFlags",
        fields: &["consoleStackFlags"],
        flags_from: &["consoleStackFlags"],
        namer: console_stack,
        reads: false,
    },
    SignalFlags {
        signal: "CleanConsoleStackAccessFlags",
        fields: &["consoleStackFlags"],
        flags_from: &["consoleStackFlags"],
        namer: clean_console_stack,
        reads: false,
    },
    SignalFlags {
        signal: "NavigatorProperties",
        fields: &["flags"],
        flags_from: &["flags"],
        namer: navigator_properties,
        reads: false,
    },
    SignalFlags {
        signal: "UserAgentData",
        fields: &["flags"],
        flags_from: &["flags"],
        namer: user_agent_data,
        reads: false,
    },
    SignalFlags {
        signal: "WebGlContext",
        fields: &["flags"],
        flags_from: &["flags"],
        namer: webgl_context,
        reads: false,
    },
    SignalFlags {
        signal: "VisibilityEventLog",
        fields: &["state"],
        flags_from: &["state"],
        namer: visibility,
        reads: true,
    },
    SignalFlags {
        signal: "UiEventSamplesMouse",
        fields: &["flags"],
        flags_from: &["flags"],
        namer: ui_event_samples,
        reads: true,
    },
    SignalFlags {
        signal: "UiEventSamplesTouch",
        fields: &["flags"],
        flags_from: &["flags"],
        namer: ui_event_samples,
        reads: true,
    },
    SignalFlags {
        signal: "CurrentScriptAsyncPolicyState",
        fields: &["scriptFlags"],
        flags_from: &["scriptFlags"],
        namer: script_async,
        reads: false,
    },
];

fn signal_flags(signal: &str) -> Option<&'static SignalFlags> {
    SIGNALS.iter().find(|entry| entry.signal == signal)
}

/// The signal's packed fields are named here.
pub(crate) fn handles(signal: &str) -> bool {
    signal_flags(signal).is_some()
}

/// `field` is a packed field of `signal` named here.
pub(crate) fn is_packed(signal: &str, field: &str) -> bool {
    signal_flags(signal).is_some_and(|entry| entry.fields.contains(&field))
}

/// The `flags` list of card field `field` of `signal`: the union over
/// every field of that name (one name in several messages), in card order.
pub(crate) fn card_flags(signal: &str, field: &str) -> Option<Vec<String>> {
    let card = crate::signals::catalog::card(signal)?;
    let mut out: Option<Vec<String>> = None;
    for card_field in card.all_fields() {
        if card_field.name != field || card_field.flags.is_empty() {
            continue;
        }
        let list = out.get_or_insert_with(Vec::new);
        for flag in card_field.flags {
            if !list.iter().any(|known| known == flag) {
                list.push(flag.to_string());
            }
        }
    }
    out
}

fn all_flags(entry: &SignalFlags) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for from in entry.flags_from {
        for flag in card_flags(entry.signal, from)? {
            if !out.contains(&flag) {
                out.push(flag);
            }
        }
    }
    Some(out)
}

/// Name the bits of the packed fields of `signal` bound in `binding` (or
/// only of card field `only`). Fails when any of them cannot be derived or
/// named.
pub(crate) fn apply_flags(
    signal: &str,
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
    binding: &mut FieldBinding,
    only: Option<&str>,
) -> Result<(), String> {
    apply_signal_flags(signal, view, layout, ctx, binding, only)
}

fn apply_signal_flags(
    signal: &str,
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
    binding: &mut FieldBinding,
    only: Option<&str>,
) -> Result<(), String> {
    let entry = signal_flags(signal).ok_or("flags: no namer")?;
    // A field without a card list names no bit (and fails below).
    let flags = all_flags(entry).unwrap_or_default();
    let wanted: Vec<(usize, String, String)> = binding
        .entries()
        .filter(|(message, key, name)| {
            entry.fields.contains(name) && binding.flags(*message, key).is_none() && only.is_none_or(|only| only == *name)
        })
        .map(|(message, key, name)| (message, key.to_string(), name.to_string()))
        .collect();
    let _ = view;
    // Per field: per write site, bit -> name (`None`: constant or unnamed).
    struct Target {
        message: usize,
        key: String,
        field: String,
        sites: Vec<BTreeMap<u32, Option<String>>>,
    }
    let mut targets = Vec::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut unnamed: BTreeSet<(String, u32)> = BTreeSet::new();
    let mut first_unnamed: Option<String> = None;
    let mut constants: Vec<String> = Vec::new();
    for (message, key, field) in wanted {
        let mut sites = Vec::new();
        for (emission_index, emission) in layout.emissions.iter().enumerate() {
            for (part, (_, part_layout)) in emission.parts.iter().enumerate() {
                if layout.part_message(emission_index, part) != Some(message) {
                    continue;
                }
                let Some((node, offset)) = find_key(&part_layout.fields, &key) else { continue };
                let leaves = emission.part_leaves(part);
                for inner in flag_leaf_offsets(node) {
                    let leaf = leaves
                        .get(offset + inner)
                        .and_then(|index| view.emissions().get(emission.emission)?.leaves.get(*index))
                        .ok_or_else(|| format!("flags {field}: leaf not found"))?;
                    let value = ctx
                        .caller_value_node(leaf)
                        .ok_or_else(|| format!("flags {field}: value not found"))?;
                    let sym = ctx.sym_with(value, entry.reads);
                    let terms = split(&sym).map_err(|reason| format!("flags {field}: {reason}"))?;
                    let mut site = BTreeMap::new();
                    for (bit, items) in terms {
                        let items: Vec<Sym> = items.into_iter().flatten().collect();
                        if items.is_empty() {
                            constants.push(format!("{field} bit {bit}"));
                            site.insert(bit, None);
                            continue;
                        }
                        let term = if items.len() == 1 { items[0].clone() } else { Sym::Phi(items) };
                        let named = (entry.namer)(ctx, &term, &flags);
                        match named.map_err(|reason| format!("flags {field}: bit {bit}: {reason}"))? {
                            Some(name) => {
                                if !flags.contains(&name) {
                                    return Err(format!("flags {field}: {name} is not a card flag"));
                                }
                                used.insert(name.clone());
                                site.insert(bit, Some(name));
                            }
                            None => {
                                first_unnamed.get_or_insert_with(|| {
                                    format!("flags {field}: bit {bit} (`{}`) reads no card flag", clip(&term.render()))
                                });
                                site.insert(bit, None);
                            }
                        }
                    }
                    for (bit, name) in &site {
                        if name.is_none() {
                            unnamed.insert((field.clone(), *bit));
                        }
                    }
                    sites.push(site);
                }
            }
        }
        if sites.is_empty() {
            continue;
        }
        targets.push(Target {
            message,
            key,
            field,
            sites,
        });
    }
    // A constant bit or a boolean without evidence is named only by
    // elimination: exactly one card flag of the signal is read by no other
    // bit, and every such bit is that one bit position of one field.
    let leftover: Vec<&String> = flags.iter().filter(|flag| !used.contains(*flag)).collect();
    let elimination = match (unnamed.len(), leftover.as_slice()) {
        (0, _) => None,
        (1, [only]) => Some((*only).clone()),
        _ => {
            if first_unnamed.is_none() && !constants.is_empty() {
                constants.dedup();
                return Err(format!(
                    "flags: constant 1 bit(s) ({}) are folded literals no card flag names ({} card flag(s) left)",
                    constants.join(", "),
                    leftover.len()
                ));
            }
            return Err(first_unnamed.unwrap_or_else(|| {
                format!(
                    "flags: {} constant or unmatched bit(s) with {} unnamed card flag(s)",
                    unnamed.len(),
                    leftover.len()
                )
            })
            .replace("reads no card flag", &format!("reads no card flag ({} unmatched bit(s), unused card flags: {})", unnamed.len(), leftover.iter().map(|flag| flag.as_str()).collect::<Vec<_>>().join(", "))))
        }
    };
    let mut claimed: BTreeMap<(usize, String), String> = BTreeMap::new();
    for target in targets {
        let mut bits: Option<BTreeMap<String, u32>> = None;
        for site in target.sites {
            let mut site_bits = BTreeMap::new();
            for (bit, name) in site {
                let name = match name {
                    Some(name) => name,
                    None => elimination.clone().ok_or("flags: unnamed bit")?,
                };
                if site_bits.insert(name.clone(), bit).is_some() {
                    return Err(format!("flags {}: {name} on two bits", target.field));
                }
            }
            // Sites agree on every bit they both set; a site that does not
            // set a flag writes 0 there (a path where it is constant 0).
            let merged = bits.get_or_insert_with(BTreeMap::new);
            for (name, bit) in site_bits {
                if merged.get(&name).is_some_and(|known| *known != bit)
                    || merged.iter().any(|(other, known)| *known == bit && *other != name)
                {
                    return Err(format!("flags {}: write sites disagree on the bits", target.field));
                }
                merged.insert(name, bit);
            }
        }
        let bits = bits.unwrap_or_default();
        for name in bits.keys() {
            // One boolean is packed once per message (a field repeated per
            // record of a list is one field).
            if let Some(other) = claimed.insert((target.message, name.clone()), target.field.clone()) {
                if other != target.field {
                    return Err(format!("flags: {name} on {other} and {}", target.field));
                }
            }
        }
        binding.set_flags(target.message, target.key, bits);
    }
    Ok(())
}

fn clip(text: &str) -> String {
    if text.chars().count() > 400 {
        format!("{}…", text.chars().take(400).collect::<String>())
    } else {
        text.to_string()
    }
}

// ===========================================================================
// Namers
// ===========================================================================

/// The one flag every alternative names (`None` when none names one).
fn agree(
    ctx: &FinderCtx,
    sym: &Sym,
    flags: &[String],
    name: impl Fn(&FinderCtx, &Sym, &[String]) -> Result<Option<String>, String>,
) -> Result<Option<String>, String> {
    let mut found: Option<String> = None;
    for alternative in sym.alternatives() {
        if let Some(flag) = name(ctx, alternative, flags)? {
            match &found {
                Some(previous) if *previous != flag => {
                    return Err(format!("alternatives read {previous} and {flag}"));
                }
                _ => found = Some(flag),
            }
        }
    }
    Ok(found)
}

/// The flag of a probe table (flag, members / strings the value reads):
/// the flags all of whose probes the value reads; more than one is an
/// error.
fn by_probes(alt: &Sym, table: &[(&str, &[&str])]) -> Result<Option<String>, String> {
    let members = alt.members();
    let strings = alt.strings();
    let matched: Vec<&str> = table
        .iter()
        .filter(|(_, probes)| probes.iter().all(|probe| members.contains(probe) || strings.contains(probe)))
        .map(|(flag, _)| *flag)
        .collect();
    match matched.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one.to_string())),
        many => Err(format!("reads {}", many.join(", "))),
    }
}

fn probes_namer(
    ctx: &FinderCtx,
    sym: &Sym,
    flags: &[String],
    table: &'static [(&'static str, &'static [&'static str])],
) -> Result<Option<String>, String> {
    agree(ctx, sym, flags, |_, alt, _| by_probes(alt, table))
}

fn navigator_properties(ctx: &FinderCtx, sym: &Sym, flags: &[String]) -> Result<Option<String>, String> {
    probes_namer(
        ctx,
        sym,
        flags,
        &[
            ("locationbarVisible", &["locationbar", "visible"]),
            ("toolbarVisible", &["toolbar", "visible"]),
            ("webdriver", &["webdriver"]),
            ("isSecureContext", &["isSecureContext"]),
        ],
    )
}

fn user_agent_data(ctx: &FinderCtx, sym: &Sym, flags: &[String]) -> Result<Option<String>, String> {
    probes_namer(ctx, sym, flags, &[("wow64", &["wow64"]), ("mobile", &["mobile"])])
}

fn webgl_context(ctx: &FinderCtx, sym: &Sym, flags: &[String]) -> Result<Option<String>, String> {
    probes_namer(
        ctx,
        sym,
        flags,
        &[("antialias", &["antialias"]), ("collectedUsingWebWorker", &["collectedUsingWebWorker"])],
    )
}

fn script_async(ctx: &FinderCtx, sym: &Sym, flags: &[String]) -> Result<Option<String>, String> {
    probes_namer(ctx, sym, flags, &[("async", &["currentScript", "async"])])
}

/// `grecaptcha` descriptor walk: what the boolean tests about the
/// descriptor found (and its `get` / `set` accessor), or which global own
/// property it checks.
fn grecaptcha(ctx: &FinderCtx, sym: &Sym, flags: &[String]) -> Result<Option<String>, String> {
    agree(ctx, sym, flags, |_, alt, _| {
        let text = alt.render();
        if !alt.has_string("grecaptcha") {
            return by_probes(
                alt,
                &[
                    ("featurePolicy", &["hasOwnProperty", "FeaturePolicy"]),
                    ("layoutShiftAttribution", &["hasOwnProperty", "LayoutShiftAttribution"]),
                    ("compressionStream", &["hasOwnProperty", "CompressionStream"]),
                ],
            );
        }
        let accessor = match (alt.has_member("get"), alt.has_member("set")) {
            (true, false) => "getter",
            (false, true) => "setter",
            (false, false) => "",
            (true, true) => return Err("reads both accessors".into()),
        };
        let flag = if alt.has_string("BrowserAutomationStudio") {
            match accessor {
                "getter" => "getterHasBasMarker",
                "setter" => "setterHasBasMarker",
                _ => return Ok(None),
            }
        } else if accessor.is_empty() {
            if text.contains("getOwnPropertyDescriptor") {
                "descriptorPresent"
            } else if text.contains("hasOwnProperty") {
                "ownFound"
            } else {
                return Ok(None);
            }
        } else if alt.has_member("toString") {
            if accessor == "getter" {
                "getterHasToString"
            } else {
                "setterHasToString"
            }
        } else if text.contains("typeof") {
            if accessor == "getter" {
                "getterIsFunction"
            } else {
                "setterIsFunction"
            }
        } else {
            return Ok(None);
        };
        Ok(Some(flag.to_string()))
    })
}

/// `P.global.x` / `P.global` spelled without the context parameter.
fn host_text(sym: &Sym) -> String {
    let mut text = sym.render();
    for index in 0..10 {
        text = text.replace(&format!("P{index}.global."), "G.").replace(&format!("P{index}.global"), "G");
    }
    text
}

/// Every `Object.getOwnPropertyDescriptor(target, "prop")`: (target text,
/// prop).
fn descriptor_reads(alt: &Sym) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for node in alt.nodes() {
        if let Sym::Call(callee, args) = node {
            if let (Some(method), [target, Sym::Str(prop)]) = (called_method(callee), args.as_slice()) {
                if method == "getOwnPropertyDescriptor" {
                    let item = (host_text(target), prop.clone());
                    if !out.contains(&item) {
                        out.push(item);
                    }
                }
            }
        }
    }
    out
}

/// The properties of the `getOwnPropertyDescriptor(target, "p")` reads of
/// a value, not looking inside a read's target (a register reused for the
/// global that also held another probe's result: the target's own reads
/// are not what this value probes).
fn outer_descriptor_props(value: &Sym) -> Vec<String> {
    fn walk(sym: &Sym, out: &mut Vec<String>) {
        if let Sym::Call(callee, args) = sym {
            if let (Some("getOwnPropertyDescriptor"), [_, Sym::Str(prop)]) = (called_method(callee), args.as_slice()) {
                if !out.contains(prop) {
                    out.push(prop.clone());
                }
                return;
            }
        }
        match sym {
            Sym::Member(object, _) => walk(object, out),
            Sym::Index(object, index) => {
                walk(object, out);
                walk(index, out);
            }
            Sym::Call(callee, args) | Sym::New(callee, args) => {
                walk(callee, out);
                args.iter().for_each(|arg| walk(arg, out));
            }
            Sym::Unary(_, value) | Sym::Guard(value, _) | Sym::Read(_, value) => walk(value, out),
            Sym::Bin(_, left, right) => {
                walk(left, out);
                walk(right, out);
            }
            Sym::Cond(test, consequent, alternate) => {
                walk(test, out);
                walk(consequent, out);
                walk(alternate, out);
            }
            Sym::Phi(items) | Sym::Array(items) => items.iter().for_each(|item| walk(item, out)),
            Sym::Object(entries) => entries.iter().for_each(|(_, value)| walk(value, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(value, &mut out);
    out
}

/// The method a callee is: `o.m`, or a register holding `o.m` on every
/// path where the call can run (its other values cannot be called: a
/// literal, or a read off `undefined` that throws before the call).
fn called_method(callee: &Sym) -> Option<&str> {
    // A read off `undefined` / `null` (at any depth) throws.
    fn throws(sym: &Sym) -> bool {
        match sym {
            Sym::Member(object, _) | Sym::Index(object, _) => {
                matches!(object.as_ref(), Sym::Undef | Sym::Null) || throws(object)
            }
            _ => false,
        }
    }
    let not_callable =
        |sym: &Sym| matches!(sym, Sym::Num(_) | Sym::Str(_) | Sym::Bool(_) | Sym::Null | Sym::Undef) || throws(sym);
    match callee {
        Sym::Member(_, method) => Some(method),
        Sym::Phi(items) => {
            let mut method = None;
            for item in items.iter().filter(|item| !not_callable(item)) {
                match (item, method) {
                    (Sym::Member(_, name), None) => method = Some(name.as_str()),
                    (Sym::Member(_, name), Some(known)) if name == known => {}
                    _ => return None,
                }
            }
            method
        }
        _ => None,
    }
}

/// What a descriptor target is: `screen` / `navigator` prototypes, the
/// canvas and 2D-context prototypes, `Function.prototype`, or `document`
/// with the number of `getPrototypeOf` steps.
fn target_kind(target: &str) -> Option<String> {
    let steps = target.matches("getPrototypeOf(").count();
    let kind = if target.contains("CanvasRenderingContext2D") {
        "context2d".to_string()
    } else if target.contains("HTMLCanvasElement") {
        "canvas".to_string()
    } else if target.contains("G.screen") {
        "screen".to_string()
    } else if target.contains("G.navigator") {
        "navigator".to_string()
    } else if target.contains("G.document") {
        format!("document{steps}")
    } else if target.starts_with("Function.prototype") {
        "function".to_string()
    } else {
        return None;
    };
    Some(kind)
}

/// The 20 native-shape checks of NativeApiIntegrityFlags.
fn native_api(ctx: &FinderCtx, sym: &Sym, flags: &[String]) -> Result<Option<String>, String> {
    agree(ctx, sym, flags, |_, alt, _| {
        let reads = descriptor_reads(alt);
        let mut found: Vec<&str> = Vec::new();
        for (target, prop) in &reads {
            let flag = match (target_kind(target).as_deref(), prop.as_str()) {
                (Some("screen"), "width") => "screenWidthGetterNative",
                (Some("screen"), "height") => "screenHeightGetterNative",
                (Some("navigator"), "userAgent") => "userAgentGetterNative",
                (Some("navigator"), "plugins") => "pluginsGetterNative",
                (Some("navigator"), "mimeTypes") => "mimeTypesGetterNative",
                (Some("context2d"), "fillText") => "fillTextNative",
                (Some("context2d"), "getImageData") => "getImageDataNative",
                (Some("context2d"), "strokeStyle") => "strokeStyleAccessorNative",
                (Some("context2d"), "fillStyle") => "fillStyleAccessorNative",
                (Some("context2d"), "font") => "fontAccessorNative",
                (Some("canvas"), "toDataURL") => "toDataURLNative",
                (Some("canvas"), "getContext") => "getContextNative",
                (Some("function"), "toString") => "functionToStringNative",
                (Some("document0"), "createElement") => "documentNoOwnCreateElement",
                (Some("document1"), "createElement") => "documentProtoNoOwnCreateElement",
                (Some("document2"), "createElement") => "documentPrototypeHasOwnCreateElement",
                _ => return Err(format!("descriptor of {target}.{prop}")),
            };
            if !found.contains(&flag) {
                found.push(flag);
            }
        }
        if reads.is_empty() {
            let text = host_text(alt);
            if alt.has_string("[object Object]") || alt.has_string("[object Function]") {
                match (alt.has_member("plugins"), alt.has_member("mimeTypes")) {
                    (true, false) => found.push("pluginsTagNotPlain"),
                    (false, true) => found.push("mimeTypesTagNotPlain"),
                    _ => {}
                }
            } else if text.contains("G.document.createElementNS") {
                found.push("createElementNSNative");
            } else if text.contains("G.document.createElement") {
                found.push("createElementNative");
            }
        }
        match found.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(one.to_string())),
            many => Err(format!("reads {}", many.join(", "))),
        }
    })
}

/// `x++`, `--x`, `x += v`, `x |= v`: an update whose value is a number or
/// string (not `x ||= v` / `&&=` / `??=`, which may keep an object).
fn primitive_update(kind: oxc_ast::AstKind) -> bool {
    use oxc_syntax::operator::AssignmentOperator;
    match kind {
        oxc_ast::AstKind::UpdateExpression(_) => true,
        oxc_ast::AstKind::AssignmentExpression(assignment) => !matches!(
            assignment.operator,
            AssignmentOperator::Assign
                | AssignmentOperator::LogicalOr
                | AssignmentOperator::LogicalAnd
                | AssignmentOperator::LogicalNullish
        ),
        _ => false,
    }
}

/// The probe helper's record keys by role: `typeof f === "function"`
/// (`IsFunction`), the native-source pattern test (`LooksNative`), no own
/// `prototype` (`NoOwnPrototype`), `delete f.prototype` succeeded (a `true`
/// stored under a test: `PrototypeDeletable`).
fn helper_roles(ctx: &FinderCtx, function: oxc_semantic::NodeId) -> Option<BTreeMap<String, &'static str>> {
    use oxc_ast::AstKind;
    let nodes = ctx.parse.nodes;
    let span = nodes.kind(function).span();
    let mut record: Option<crate::signals::analysis::write_sites::RecordConstruction> = None;
    for id in ctx.nodes_within(span) {
        let AstKind::ReturnStatement(statement) = nodes.kind(id) else { continue };
        if ctx.flow_root(id) != function || never_runs(ctx, id, function) {
            continue;
        }
        let Some(argument) = statement.argument.as_ref() else { continue };
        // `return (defineProperty(o, "d", …), o)`: the last expression.
        let argument = match argument.without_parentheses() {
            oxc_ast::ast::Expression::SequenceExpression(sequence) => sequence.expressions.last()?,
            other => other,
        };
        let node = ctx.node_with_span(argument.without_parentheses().span())?;
        // `return o` with `o = {…}`; `o.k = v` stores join the record. A
        // return of anything else (a dead `"d"[x]` arm) is not the record.
        let literal = match nodes.kind(node) {
            AstKind::ObjectExpression(_) => node,
            // The record past never-run arms and through copies; else every
            // object literal that can reach, opaque non-object stores dropped.
            AstKind::IdentifierReference(_) => match live_object(ctx, node, function) {
                Some(value) => value,
                None => {
                // `o = {…}` is the record. An opaque guard on a boolean the
                // helper made (`if (d !== void 0) o.d = d; else o = d;`)
                // adds stores of non-object values to the same binding, or
                // a `return x` of one: they are not the record. Exactly one
                // object literal must reach; a store this resolver does not
                // follow (a parameter, `o |= …`) keeps the helper unknown.
                // `r = o` copies the record into another binding: follow it.
                // `r = t ? x : o` stores either arm. A copied read that is
                // not a local (`o = Infinity`) is not the record.
                let mut literals: Vec<oxc_semantic::NodeId> = Vec::new();
                let mut pending = vec![(node, true)];
                let mut seen = 0usize;
                while let Some((value, root)) = pending.pop() {
                    seen += 1;
                    if seen > 16 {
                        return None;
                    }
                    match nodes.kind(value) {
                        AstKind::ObjectExpression(_) => {
                            if !literals.contains(&value) {
                                literals.push(value);
                            }
                        }
                        AstKind::ConditionalExpression(conditional) => {
                            for arm in [&conditional.consequent, &conditional.alternate] {
                                if let Some(arm) = ctx.node_with_span(arm.without_parentheses().span()) {
                                    pending.push((arm, false));
                                }
                            }
                        }
                        AstKind::IdentifierReference(_) => {
                            let Some((_, stores)) = ctx.reaching(value) else {
                                if root {
                                    return None;
                                }
                                continue;
                            };
                            for store in &stores {
                                match store {
                                    crate::signals::flags::flow::Store::Assign(store) | crate::signals::flags::flow::Store::Declare(store) => {
                                        if let Some(stored) = ctx.definition_value(*store) {
                                            pending.push((stored, false));
                                        }
                                    }
                                    // `--x`, `x += 1`: a primitive, not the record.
                                    crate::signals::flags::flow::Store::Other(update) if primitive_update(nodes.kind(*update)) => {}
                                    // A copy's parameter or unfollowed store is not
                                    // this helper's record either.
                                    _ if !root => {}
                                    _ => return None,
                                }
                            }
                        }
                        _ => {}
                    }
                }
                match literals[..] {
                    [value] => value,
                    [] => continue,
                    _ => return None,
                }
            },
            },
            _ => continue,
        };
        let found = ctx.record_at(literal)?;
        match &record {
            Some(known) if known.node != found.node => return None,
            _ => record = Some(found),
        }
    }
    let mut record = record?;
    // `o = {k: void 0, …}; o.k = v`: the later store is the value.
    for id in ctx.nodes_within(span) {
        let AstKind::AssignmentExpression(assignment) = nodes.kind(id) else { continue };
        if ctx.flow_root(id) != function || !assignment.operator.is_assign() {
            continue;
        }
        let oxc_ast::ast::AssignmentTarget::StaticMemberExpression(member) = &assignment.left else { continue };
        let Some(object) = ctx.node_with_span(member.object.without_parentheses().span()) else { continue };
        if !matches!(nodes.kind(object), AstKind::IdentifierReference(_)) || ctx.reaching_value(object) != Some(record.node) {
            continue;
        }
        let key = member.property.name.to_string();
        let placeholder = record.values.get(&key).is_none_or(|value| {
            matches!(nodes.kind(*value), AstKind::UnaryExpression(unary) if unary.operator == oxc_ast::ast::UnaryOperator::Void)
                || matches!(nodes.kind(*value), AstKind::IdentifierReference(identifier) if identifier.name == "undefined")
        });
        if let (true, Some(value)) = (placeholder, ctx.node_with_span(assignment.right.span())) {
            record.values.insert(key, value);
        }
    }
    // `Object.defineProperty(o, "k", {value: v})` inside the helper.
    for id in ctx.nodes_within(span) {
        let AstKind::CallExpression(call) = nodes.kind(id) else { continue };
        if ctx.flow_root(id) != function {
            continue;
        }
        let oxc_ast::ast::Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else { continue };
        if callee.property.name != "defineProperty" || call.arguments.len() != 3 {
            continue;
        }
        let (Some(oxc_ast::ast::Expression::StringLiteral(key)), Some(oxc_ast::ast::Expression::ObjectExpression(descriptor))) = (
            call.arguments[1].as_expression().map(|e| e.without_parentheses()),
            call.arguments[2].as_expression().map(|e| e.without_parentheses()),
        ) else {
            continue;
        };
        for property in &descriptor.properties {
            let oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) = property else { continue };
            if property.key.static_name().as_deref() == Some("value") {
                if let Some(value) = ctx.node_with_span(property.value.span()) {
                    record.values.entry(key.value.to_string()).or_insert(value);
                }
            }
        }
    }
    let mut roles = BTreeMap::new();
    for (key, value) in &record.values {
        let value = live_sym(ctx, *value, function);
        let text = value.render();
        let role = if value.nodes().iter().any(|node| matches!(node, Sym::Regex(_)))
            || text.contains(".test(")
            || text.contains(".exec(")
        {
            "LooksNative"
        } else if value.alternatives().iter().any(|alternative| matches!(unread(alternative), Sym::Guard(..)))
            // Only a boolean: a source-text key that an opaque arm also
            // stores the deletion flag into (`else { o = d; s = d; }`) is
            // not the flag.
            && value
                .alternatives()
                .iter()
                .all(|alternative| matches!(unread(alternative), Sym::Guard(..) | Sym::Bool(_)))
        {
            // `true` stored under the delete's test: a guard arm of the
            // value itself (a guard deep inside another key's value, e.g.
            // the source text read through a guarded original, is not).
            "PrototypeDeletable"
        } else if matches!(value, Sym::Unary("!", _)) && value.has_string("prototype") {
            "NoOwnPrototype"
        } else if text == "(typeof(P0) === \"function\")" {
            "IsFunction"
        } else {
            continue;
        };
        if roles.values().any(|known| *known == role) {
            return None;
        }
        roles.insert(key.clone(), role);
    }
    Some(roles)
}

/// The value of `node` (the taken arm of a decided `?:`); for a binding
/// read some of whose reaching stores
/// sit in arms that never run ([`never_runs`]), the values of the other
/// stores only (`if (b !== void 0) {o.d = b} else {s = b}; o.e = s`: `s`
/// is never the boolean).
fn live_sym(ctx: &FinderCtx, node: oxc_semantic::NodeId, function: oxc_semantic::NodeId) -> Sym {
    use crate::signals::flags::flow::Store;
    // `b !== void 0 ? b : b + 1`: the arm a decided test takes.
    if let oxc_ast::AstKind::ConditionalExpression(conditional) = ctx.parse.nodes.kind(node) {
        if let Some(taken) = nullish_test(ctx, &conditional.test) {
            let arm = if taken { &conditional.consequent } else { &conditional.alternate };
            if let Some(arm) = ctx.node_with_span(arm.without_parentheses().span()) {
                return live_sym(ctx, arm, function);
            }
        }
    }
    let whole = ctx.sym(node);
    if !matches!(ctx.parse.nodes.kind(node), oxc_ast::AstKind::IdentifierReference(_)) {
        return whole;
    }
    let Some((_, stores)) = ctx.reaching(node) else { return whole };
    let dead = |store: &Store| match store {
        Store::Assign(at) | Store::Declare(at) | Store::Other(at) => never_runs(ctx, *at, function),
        Store::Entry => false,
    };
    if !stores.iter().any(dead) {
        return whole;
    }
    let mut values = Vec::new();
    for store in stores.iter().filter(|store| !dead(store)) {
        let (Store::Assign(at) | Store::Declare(at)) = store else { return whole };
        let Some(value) = ctx.definition_value(*at) else { return whole };
        values.push(ctx.sym(value));
    }
    match values.len() {
        0 => whole,
        1 => values.remove(0),
        _ => Sym::Phi(values),
    }
}

/// Some return of `function` that runs returns an object literal (directly
/// or through a binding).
fn returns_record(ctx: &FinderCtx, function: oxc_semantic::NodeId) -> bool {
    use oxc_ast::AstKind;
    let nodes = ctx.parse.nodes;
    ctx.nodes_within(nodes.kind(function).span()).into_iter().any(|id| {
        let AstKind::ReturnStatement(statement) = nodes.kind(id) else { return false };
        if ctx.flow_root(id) != function || never_runs(ctx, id, function) {
            return false;
        }
        let Some(argument) = statement.argument.as_ref() else { return false };
        let argument = match argument.without_parentheses() {
            oxc_ast::ast::Expression::SequenceExpression(sequence) => match sequence.expressions.last() {
                Some(last) => last,
                None => return false,
            },
            other => other,
        };
        let Some(node) = ctx.node_with_span(argument.without_parentheses().span()) else { return false };
        match nodes.kind(node) {
            AstKind::ObjectExpression(_) => true,
            AstKind::IdentifierReference(_) => live_object(ctx, node, function).is_some(),
            _ => false,
        }
    })
}

/// The object literal the binding read `node` holds on every path that
/// runs (stores in never-run arms dropped, through binding copies:
/// `if (b === null) {r = "d" - b} else {o.d = b; r = o}; return r`).
fn live_object(ctx: &FinderCtx, mut node: oxc_semantic::NodeId, function: oxc_semantic::NodeId) -> Option<oxc_semantic::NodeId> {
    use crate::signals::flags::flow::Store;
    use oxc_ast::AstKind;
    let nodes = ctx.parse.nodes;
    for _ in 0..4 {
        let (_, stores) = ctx.reaching(node)?;
        // Stores in an arm that never runs (`if (b !== void 0) {o.d = b}
        // else {o = b}` with `b` only ever a boolean) do not reach the
        // read.
        let live: Vec<_> = stores
            .iter()
            .filter(|store| match store {
                Store::Assign(at) | Store::Declare(at) | Store::Other(at) => !never_runs(ctx, *at, function),
                Store::Entry => true,
            })
            .cloned()
            .collect();
        // A store every path from which to the read goes through a decided
        // `if` whose arm stores again (`if (b !== void 0) {o.d = b; b = o}
        // return b` with `b` only ever a boolean) does not reach it.
        let overwriting: Vec<oxc_semantic::NodeId> = live
            .iter()
            .filter_map(|store| match store {
                Store::Assign(at) => decided_arm_store(ctx, *at),
                _ => None,
            })
            .collect();
        let live: Vec<_> = live
            .into_iter()
            .filter(|store| match store {
                Store::Assign(at) | Store::Declare(at) | Store::Other(at) => {
                    !overwriting.iter().any(|branch| passes_before(ctx, *at, *branch, node))
                }
                Store::Entry => true,
            })
            .collect();
        let [Store::Assign(store) | Store::Declare(store)] = live[..] else { return None };
        let mut value = ctx.definition_value(store)?;
        // `r = b === null ? b : o`: the arm a decided test takes.
        if let AstKind::ConditionalExpression(conditional) = nodes.kind(value) {
            let arm = match nullish_test(ctx, &conditional.test)? {
                true => &conditional.consequent,
                false => &conditional.alternate,
            };
            value = ctx.node_with_span(arm.without_parentheses().span())?;
        }
        match nodes.kind(value) {
            AstKind::ObjectExpression(_) => return Some(value),
            AstKind::IdentifierReference(_) => node = value,
            _ => return None,
        }
    }
    None
}

/// The `if` statement whose test is decided taken ([`nullish_test`]) and
/// whose arm always runs the plain assignment `store`: the store is a
/// statement of the arm, after expression statements only.
fn decided_arm_store(ctx: &FinderCtx, store: oxc_semantic::NodeId) -> Option<oxc_semantic::NodeId> {
    use oxc_ast::ast::Statement;
    use oxc_ast::AstKind;
    let nodes = ctx.parse.nodes;
    let AstKind::AssignmentExpression(assignment) = nodes.kind(store) else { return None };
    if !assignment.operator.is_assign() {
        return None;
    }
    let statement = nodes.parent_id(store);
    let AstKind::ExpressionStatement(_) = nodes.kind(statement) else { return None };
    let statement_span = nodes.kind(statement).span();
    let mut arm = nodes.parent_id(statement);
    if let AstKind::BlockStatement(block) = nodes.kind(arm) {
        for item in &block.body {
            if item.span() == statement_span {
                break;
            }
            if !matches!(item, Statement::ExpressionStatement(_)) {
                return None;
            }
        }
    } else {
        arm = statement;
    }
    let branch = nodes.parent_id(arm);
    let AstKind::IfStatement(statement) = nodes.kind(branch) else { return None };
    (statement.consequent.span() == nodes.kind(arm).span() && nullish_test(ctx, &statement.test) == Some(true)).then_some(branch)
}

/// The statement `branch` lies between `store` and `read` on every path:
/// `store` ends before it, and `read` is inside a later statement of the
/// same statement list.
fn passes_before(ctx: &FinderCtx, store: oxc_semantic::NodeId, branch: oxc_semantic::NodeId, read: oxc_semantic::NodeId) -> bool {
    use oxc_ast::AstKind;
    let nodes = ctx.parse.nodes;
    let branch_span = nodes.kind(branch).span();
    if nodes.kind(store).span().end > branch_span.start {
        return false;
    }
    let list = nodes.parent_id(branch);
    if !matches!(nodes.kind(list), AstKind::BlockStatement(_) | AstKind::FunctionBody(_)) {
        return false;
    }
    let mut child = read;
    loop {
        let parent = nodes.parent_id(child);
        if parent == child {
            return false;
        }
        if parent == list {
            return nodes.kind(child).span().start >= branch_span.end;
        }
        if matches!(nodes.kind(parent), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) {
            return false;
        }
        child = parent;
    }
}

/// `node` (inside `function`) sits where control never gets: the arm of an
/// `if` whose test compares a binding that only ever holds booleans with
/// `null` / `undefined` and so is decided, or after such an `if` whose
/// taken arm returns.
fn never_runs(ctx: &FinderCtx, node: oxc_semantic::NodeId, function: oxc_semantic::NodeId) -> bool {
    use oxc_ast::ast::Statement;
    use oxc_ast::AstKind;
    let nodes = ctx.parse.nodes;
    let returns = |statement: &Statement| match statement {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
        Statement::BlockStatement(block) => {
            matches!(block.body.last(), Some(Statement::ReturnStatement(_) | Statement::ThrowStatement(_)))
        }
        _ => false,
    };
    // Some(arm) of an `if` decided to always take `arm`.
    let decided = |statement: &Statement| -> Option<bool> {
        let Statement::IfStatement(statement) = statement else { return None };
        nullish_test(ctx, &statement.test)
    };
    let mut child = node;
    loop {
        let parent = nodes.parent_id(child);
        if parent == child || parent == function {
            return false;
        }
        let child_span = nodes.kind(child).span();
        let earlier = |body: &[Statement]| {
            body.iter().take_while(|statement| statement.span().end <= child_span.start).any(|statement| {
                let Statement::IfStatement(branch) = statement else { return false };
                match decided(statement) {
                    Some(true) => returns(&branch.consequent),
                    Some(false) => branch.alternate.as_ref().is_some_and(returns),
                    None => false,
                }
            })
        };
        match nodes.kind(parent) {
            AstKind::IfStatement(statement) => {
                let taken = nullish_test(ctx, &statement.test);
                if taken == Some(false) && statement.consequent.span() == child_span {
                    return true;
                }
                if taken == Some(true) && statement.alternate.as_ref().is_some_and(|arm| arm.span() == child_span) {
                    return true;
                }
            }
            AstKind::BlockStatement(block) if earlier(&block.body) => return true,
            AstKind::FunctionBody(body) if earlier(&body.statements) => return true,
            _ => {}
        }
        child = parent;
    }
}

/// `b === null` / `b !== void 0` / `b == undefined` … with `b` a binding
/// whose every reaching value is a boolean: the test's fixed outcome.
fn nullish_test(ctx: &FinderCtx, test: &oxc_ast::ast::Expression) -> Option<bool> {
    use oxc_ast::ast::Expression;
    // A binding holding such a test (`c = b === void 0; … c ? x : y`).
    if let Expression::Identifier(_) = test.without_parentheses() {
        let read = ctx.node_with_span(test.without_parentheses().span())?;
        return crate::signals::flags::flow::decided(&ctx.sym(read));
    }
    direct_nullish_test(ctx, test)
}

fn direct_nullish_test(ctx: &FinderCtx, test: &oxc_ast::ast::Expression) -> Option<bool> {
    use oxc_ast::ast::{BinaryOperator, Expression, UnaryOperator};
    let Expression::BinaryExpression(binary) = test.without_parentheses() else { return None };
    let equal = match binary.operator {
        BinaryOperator::StrictEquality | BinaryOperator::Equality => true,
        BinaryOperator::StrictInequality | BinaryOperator::Inequality => false,
        _ => return None,
    };
    let nullish = |expression: &Expression| match expression.without_parentheses() {
        Expression::NullLiteral(_) => true,
        Expression::Identifier(identifier) => identifier.name == "undefined",
        Expression::UnaryExpression(unary) => {
            unary.operator == UnaryOperator::Void && matches!(unary.argument.without_parentheses(), Expression::NumericLiteral(_))
        }
        _ => false,
    };
    let binding = match (nullish(&binary.left), nullish(&binary.right)) {
        (true, false) => &binary.right,
        (false, true) => &binary.left,
        _ => return None,
    };
    let Expression::Identifier(_) = binding.without_parentheses() else { return None };
    let read = ctx.node_with_span(binding.without_parentheses().span())?;
    let value = ctx.sym(read);
    let alternatives = value.alternatives();
    let boolean = !alternatives.is_empty()
        && alternatives.iter().all(|alternative| matches!(unread(alternative), Sym::Bool(_) | Sym::Guard(..)));
    boolean.then_some(!equal)
}

/// The probe calls a value reads a key of: `probe(fn)` itself, the right
/// side of `d && probe(fn)`, both arms of `t ? probe(a) : d && probe(b)`.
fn probe_calls(sym: &Sym) -> Vec<(oxc_semantic::NodeId, &Sym)> {
    match sym {
        Sym::Call(callee, args) => match (callee.as_ref(), args.as_slice()) {
            (Sym::Func(function), [argument]) => vec![(*function, argument)],
            _ => Vec::new(),
        },
        Sym::Bin("&&", _, right) => probe_calls(right),
        Sym::Cond(_, consequent, alternate) => {
            let mut out = probe_calls(consequent);
            out.extend(probe_calls(alternate));
            out
        }
        Sym::Phi(items) => items.iter().flat_map(probe_calls).collect(),
        _ => Vec::new(),
    }
}

/// Which of the 15 probed functions the probe arguments are: the property
/// of the descriptor they read (or the instrumented original preferred to
/// it); every argument must read the same one.
fn probed_api(arguments: &[&Sym]) -> Option<&'static str> {
    let mut props: Vec<String> = Vec::new();
    for argument in arguments {
        for prop in outer_descriptor_props(argument) {
            if !props.contains(&prop) {
                props.push(prop);
            }
        }
    }
    // `instrumented.createElement.originals.value` is preferred over the
    // descriptor of the same method.
    for node in arguments.iter().flat_map(|argument| argument.nodes()) {
        if let Sym::Member(object, name) = node {
            if name == "originals" {
                if let Sym::Member(_, method) = object.as_ref() {
                    if !props.contains(method) {
                        props.push(method.clone());
                    }
                }
            }
        }
    }
    let api = match props.as_slice() {
        [one] => one.as_str(),
        _ => return None,
    };
    Some(match api {
        "width" => "screenWidth",
        "height" => "screenHeight",
        "userAgent" => "userAgent",
        "plugins" => "plugins",
        "mimeTypes" => "mimeTypes",
        "createElement" => "createElement",
        "createElementNS" => "createElementNS",
        "getContext" => "getContext",
        "toDataURL" => "toDataURL",
        "toBlob" => "toBlob",
        "getImageData" => "getImageData",
        "fillText" => "fillText",
        "strokeStyle" => "strokeStyle",
        "fillStyle" => "fillStyle",
        "font" => "font",
        _ => return None,
    })
}

/// The 72 checks of FingerprintApiIntegrity: four probe results per probed
/// function, the own-property shadow checks, the plugins / mimeTypes type
/// tags (the iframe-realm bit is the one left, by elimination).
fn fingerprint_api(ctx: &FinderCtx, sym: &Sym, flags: &[String]) -> Result<Option<String>, String> {
    agree(ctx, sym, flags, |ctx, alt, _| {
        let mut found: Vec<String> = Vec::new();
        for node in alt.nodes() {
            let Sym::Member(object, key) = node else { continue };
            let calls = probe_calls(object);
            let Some((function, _)) = calls.first() else { continue };
            if calls.iter().any(|(other, _)| other != function) {
                return Err("two probe helpers".into());
            }
            // A call of a function that returns no record (a key list
            // helper reached through a reused register) is not a probe.
            if !returns_record(ctx, *function) {
                continue;
            }
            let roles = helper_roles(ctx, *function).ok_or("probe helper roles")?;
            let Some(role) = roles.get(key) else { continue };
            let arguments: Vec<&Sym> = calls.iter().map(|(_, argument)| *argument).collect();
            let api = probed_api(&arguments).ok_or_else(|| format!("probed function of `{}`", clip(&object.render())))?;
            let flag = format!("{api}{role}");
            if !found.contains(&flag) {
                found.push(flag);
            }
        }
        if found.is_empty() {
            // `!hasOwn(owner, "prop")`.
            for node in alt.nodes() {
                let Sym::Call(callee, args) = node else { continue };
                let [owner, Sym::Str(prop)] = args.as_slice() else { continue };
                if !callee.render().contains("hasOwnProperty") || prop == "prototype" || prop == "value" {
                    continue;
                }
                let owner = host_text(owner);
                let owner = match owner.as_str() {
                    "G.navigator" => "navigator",
                    "G.screen" => "screen",
                    "G.document" => "document",
                    "Object.getPrototypeOf(G.document)" => "documentProto",
                    _ => return Err(format!("own check on {owner}")),
                };
                let mut prop = prop.clone();
                if let Some(first) = prop.get(..1) {
                    prop = first.to_uppercase() + &prop[1..];
                }
                let flag = format!("{owner}{prop}NotShadowed");
                if !found.contains(&flag) {
                    found.push(flag);
                }
            }
        }
        if found.is_empty() && (alt.has_string("[object Object]") || alt.has_string("[object Function]")) {
            match (alt.has_member("plugins"), alt.has_member("mimeTypes")) {
                (true, false) => found.push("pluginsTagGenuine".into()),
                (false, true) => found.push("mimeTypesTagGenuine".into()),
                _ => {}
            }
        }
        match found.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(one.clone())),
            many => Err(format!("reads {}", many.join(", "))),
        }
    })
}

/// Visibility state: the visibility sampled once into a binding never
/// stored again (initially visible), or stored again in a later callback
/// (currently visible).
fn visibility(ctx: &FinderCtx, sym: &Sym, flags: &[String]) -> Result<Option<String>, String> {
    agree(ctx, sym, flags, |ctx, alt, _| {
        // Copies through registers: the innermost binding holding the
        // sampled value.
        let Sym::Read(first, first_value) = alt else { return Ok(None) };
        let (mut symbol, mut value): (oxc_semantic::SymbolId, &Sym) = (*first, first_value);
        while let [Sym::Read(inner, inner_value)] = value.alternatives().as_slice() {
            symbol = *inner;
            value = inner_value;
        }
        let reads_state = value
            .alternatives()
            .iter()
            .all(|item| item.has_member("visibilityState") && item.has_string("visible"));
        if !reads_state || value.alternatives().is_empty() {
            return Ok(None);
        }
        let scoping = ctx.parse.scoping;
        let declaration = scoping.symbol_declaration(symbol);
        let declared_in = ctx.flow_root(declaration);
        let writes: Vec<_> = scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .map(|reference| reference.node_id())
            .collect();
        let initialised = matches!(
            ctx.parse.nodes.kind(declaration),
            oxc_ast::AstKind::VariableDeclarator(declarator) if declarator.init.is_some()
        );
        // Sampled where the binding lives (the producer's setup) and
        // never stored again from a callback.
        if (initialised || !writes.is_empty()) && writes.iter().all(|write| ctx.flow_root(*write) == declared_in) {
            return Ok(Some("initiallyVisible".into()));
        }
        if !writes.is_empty() && writes.iter().all(|write| ctx.flow_root(*write) != declared_in) {
            return Ok(Some("currentlyVisible".into()));
        }
        Ok(None)
    })
}

/// Console probe flags: `flags[k] = true` set in the `stack` getter of a
/// probe error logged through a console (the page's own, a created
/// iframe's `contentWindow`, or one handed to a callback by a clean-realm
/// helper), or after calling `console.log.__sentry_original__`.
fn console_roles(ctx: &FinderCtx, sym: &Sym) -> Result<Option<&'static str>, String> {
    let alternatives = sym.alternatives();
    let [Sym::Slot(symbol, index)] = alternatives.as_slice() else { return Ok(None) };
    let nodes = ctx.parse.nodes;
    let mut roles: Vec<&'static str> = Vec::new();
    for reference in ctx.parse.scoping.get_resolved_references(*symbol) {
        let member = nodes.parent_id(reference.node_id());
        let oxc_ast::AstKind::ComputedMemberExpression(element) = nodes.kind(member) else { continue };
        if !matches!(element.expression.without_parentheses(), oxc_ast::ast::Expression::NumericLiteral(literal) if literal.value == *index as f64) {
            continue;
        }
        let store = nodes.parent_id(member);
        let oxc_ast::AstKind::AssignmentExpression(assignment) = nodes.kind(store) else { continue };
        if assignment.left.span() != element.span || !assignment.operator.is_assign() {
            continue;
        }
        let Some(right) = ctx.node_with_span(assignment.right.without_parentheses().span()) else { continue };
        if ctx.sym(right).is_unset() {
            continue;
        }
        let role = console_store_role(ctx, store).ok_or("console flag store context")?;
        if !roles.contains(&role) {
            roles.push(role);
        }
    }
    match roles.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(*one)),
        many => Err(format!("stores read {}", many.join(", "))),
    }
}

fn console_store_role(ctx: &FinderCtx, store: oxc_semantic::NodeId) -> Option<&'static str> {
    use oxc_ast::AstKind;
    let nodes = ctx.parse.nodes;
    // After the Sentry original call: an enclosing test reads it.
    let root = ctx.flow_root(store);
    let mut child = store;
    loop {
        let parent = nodes.parent_id(child);
        if parent == child || parent == root {
            break;
        }
        if let AstKind::IfStatement(statement) = nodes.kind(parent) {
            if let Some(test) = ctx.node_with_span(statement.test.without_parentheses().span()) {
                if ctx.sym(test).has_string("__sentry_original__") || ctx.sym(test).has_member("__sentry_original__") {
                    return Some("sentry");
                }
            }
        }
        child = parent;
    }
    // In a `stack` getter: `defineProperty(probe, "stack", {get: f})`.
    if !matches!(nodes.kind(root), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) {
        return None;
    }
    let outer = ctx.flow_root(nodes.parent_id(root));
    let outer_span = nodes.kind(outer).span();
    // `defineProperty(probe, "stack", {get})`, also through `.call(o, …)`.
    let mut define = None;
    for id in ctx.nodes_within(outer_span) {
        let AstKind::CallExpression(call) = nodes.kind(id) else { continue };
        if ctx.flow_root(id) != outer {
            continue;
        }
        let arguments: Vec<_> = call.arguments.iter().filter_map(|argument| argument.as_expression()).collect();
        for index in 1..arguments.len().saturating_sub(1) {
            let oxc_ast::ast::Expression::StringLiteral(literal) = arguments[index].without_parentheses() else { continue };
            if literal.value != "stack" {
                continue;
            }
            let Some(descriptor) = ctx.node_with_span(arguments[index + 1].without_parentheses().span()) else { continue };
            let Sym::Object(entries) = ctx.sym(descriptor) else { continue };
            if entries.iter().any(|(key, value)| key == "get" && *value == Sym::Func(root)) {
                if define.is_some() {
                    return None;
                }
                define = Some((id, arguments[index - 1].span()));
            }
        }
    }
    let (define, probe_span) = define?;
    let probe = ctx.node_with_span(probe_span)?;
    let probe_symbol = ctx.reference_symbol(probe)?;
    let after = nodes.kind(define).span().end;
    // The first `x.log(probe)` after it.
    let mut log = None;
    for reference in ctx.parse.scoping.get_resolved_references(probe_symbol) {
        let read = reference.node_id();
        let span = nodes.kind(read).span();
        if span.start < after || ctx.flow_root(read) != outer {
            continue;
        }
        let call = nodes.parent_id(read);
        let AstKind::CallExpression(call_expression) = nodes.kind(call) else { continue };
        let Some(receiver) = log_receiver(ctx, call_expression, span) else { continue };
        if log.as_ref().is_none_or(|(start, _)| span.start < *start) {
            log = Some((span.start, receiver));
        }
    }
    let (_, receiver) = log?;
    let receiver = ctx.sym(receiver);
    let text = host_text(&receiver);
    if text.contains("contentWindow") {
        return Some("iframe");
    }
    if let Sym::Member(object, name) = &receiver {
        if name == "console" && matches!(object.as_ref(), Sym::Param(..)) {
            return Some("clean");
        }
    }
    if text == "G.console" || text == "console" {
        return Some("page");
    }
    None
}

/// The console a `log` call logs on, the probe (`probe`) its argument:
/// `c.log(probe)`, or the method extracted first and called on the
/// console, `x = [c, c.log]; x[1].call(x[0], probe)` / `f = c.log;
/// f.call(c, probe)`.
fn log_receiver(ctx: &FinderCtx, call: &oxc_ast::ast::CallExpression, probe: oxc_span::Span) -> Option<oxc_semantic::NodeId> {
    use oxc_ast::ast::Expression;
    use oxc_ast::AstKind;
    let oxc_ast::ast::Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else { return None };
    if callee.property.name == "log" {
        return ctx.node_with_span(callee.object.without_parentheses().span());
    }
    if callee.property.name != "call" || call.arguments.get(1).map(|argument| argument.span()) != Some(probe) {
        return None;
    }
    let receiver = call.arguments.first()?.as_expression()?.without_parentheses();
    let method = ctx.node_with_span(callee.object.without_parentheses().span())?;
    // The extracted method's value: `c.log` (through `x[1]` of a pair).
    let value = match ctx.parse.nodes.kind(method) {
        AstKind::ComputedMemberExpression(element) => {
            let pair = ctx.node_with_span(element.object.without_parentheses().span())?;
            let Expression::ComputedMemberExpression(first) = receiver else { return None };
            let first_pair = ctx.node_with_span(first.object.without_parentheses().span())?;
            if ctx.reference_symbol(first_pair)? != ctx.reference_symbol(pair)?
                || !matches!(first.expression.without_parentheses(), Expression::NumericLiteral(index) if index.value == 0.0)
                || !matches!(element.expression.without_parentheses(), Expression::NumericLiteral(index) if index.value == 1.0)
            {
                return None;
            }
            let value = ctx.reaching_value(pair)?;
            let AstKind::ArrayExpression(array) = ctx.parse.nodes.kind(value) else { return None };
            let [Some(object), Some(log), None] = [array.elements.first(), array.elements.get(1), array.elements.get(2)] else { return None };
            let (object, log) = (object.as_expression()?.without_parentheses(), log.as_expression()?.without_parentheses());
            let Expression::StaticMemberExpression(log) = log else { return None };
            if log.property.name != "log" || log.object.without_parentheses().span() != object.span() && ctx.sym(ctx.node_with_span(log.object.span())?) != ctx.sym(ctx.node_with_span(object.span())?) {
                return None;
            }
            return ctx.node_with_span(object.span());
        }
        AstKind::IdentifierReference(_) => ctx.reaching_value(method)?,
        _ => return None,
    };
    let AstKind::StaticMemberExpression(log) = ctx.parse.nodes.kind(value) else { return None };
    if log.property.name != "log" {
        return None;
    }
    let object = ctx.node_with_span(log.object.without_parentheses().span())?;
    let given = ctx.node_with_span(receiver.span())?;
    (ctx.sym(object) == ctx.sym(given)).then_some(given)
}

fn console_stack(ctx: &FinderCtx, sym: &Sym, _flags: &[String]) -> Result<Option<String>, String> {
    Ok(match console_roles(ctx, sym)? {
        Some("page") => Some("pageConsoleStackRead".into()),
        Some("sentry") => Some("sentryOriginalLogCalled".into()),
        Some("iframe") => Some("iframeConsoleStackRead".into()),
        Some(other) => return Err(format!("{other} console in this signal")),
        None => None,
    })
}

fn clean_console_stack(ctx: &FinderCtx, sym: &Sym, _flags: &[String]) -> Result<Option<String>, String> {
    Ok(match console_roles(ctx, sym)? {
        Some("page") => Some("pageConsoleStackRead".into()),
        Some("sentry") => Some("sentryOriginalLogCalled".into()),
        Some("clean") => Some("cleanRealmStackRead".into()),
        Some(other) => return Err(format!("{other} console in this signal")),
        None => None,
    })
}

/// `rec.k` with `rec` an object literal (through copies): the stored value.
fn fold_member(sym: &Sym) -> Sym {
    match sym {
        Sym::Member(object, key) => {
            let mut values = Vec::new();
            for alternative in object.alternatives() {
                let alternative = unread(alternative);
                match alternative {
                    Sym::Object(entries) => match entries.iter().find(|(name, _)| name == key) {
                        Some((_, value)) => values.push(value.clone()),
                        None => return sym.clone(),
                    },
                    _ => return sym.clone(),
                }
            }
            match values.len() {
                0 => sym.clone(),
                1 => values.pop().unwrap_or(Sym::Undef),
                _ => Sym::Phi(values),
            }
        }
        other => other.clone(),
    }
}

/// The value under [`Sym::Read`] wrappers.
fn unread(sym: &Sym) -> &Sym {
    match sym {
        Sym::Read(_, value) => match value.alternatives().as_slice() {
            [one] => unread(one),
            _ => value,
        },
        other => other,
    }
}

/// The binding a value is copied from (`y = x; y.put(…)` -> `x`).
fn alias_root(ctx: &FinderCtx, mut node: oxc_semantic::NodeId) -> Option<oxc_semantic::SymbolId> {
    use crate::signals::flags::flow::Store;
    for _ in 0..8 {
        let (symbol, stores) = ctx.reaching(node)?;
        let stores: Vec<Store> = stores.into_iter().collect();
        let value = match stores.as_slice() {
            [Store::Assign(store) | Store::Declare(store)] => ctx.definition_value(*store),
            _ => None,
        };
        match value {
            Some(value) if matches!(ctx.parse.nodes.kind(value), oxc_ast::AstKind::IdentifierReference(_)) => node = value,
            _ => return Some(symbol),
        }
    }
    None
}

/// The object literal a value is (through one copy into a binding).
fn object_literal(ctx: &FinderCtx, node: oxc_semantic::NodeId) -> Option<oxc_semantic::NodeId> {
    use crate::signals::flags::flow::Store;
    match ctx.parse.nodes.kind(node) {
        oxc_ast::AstKind::ObjectExpression(_) => Some(node),
        oxc_ast::AstKind::IdentifierReference(_) => {
            let (_, stores) = ctx.reaching(node)?;
            let stores: Vec<Store> = stores.into_iter().collect();
            let [Store::Assign(store) | Store::Declare(store)] = stores.as_slice() else { return None };
            let value = ctx.definition_value(*store)?;
            matches!(ctx.parse.nodes.kind(value), oxc_ast::AstKind::ObjectExpression(_)).then_some(value)
        }
        _ => None,
    }
}

#[derive(Hash, PartialEq, Eq)]
struct RingPutsKey;

/// Every `x.put({…})` record by the binding `x` is copied from.
fn ring_puts(ctx: &FinderCtx) -> std::rc::Rc<BTreeMap<oxc_semantic::SymbolId, Vec<crate::signals::analysis::write_sites::RecordConstruction>>> {
    ctx.memo(RingPutsKey, || {
        use oxc_ast::AstKind;
        let mut out: BTreeMap<oxc_semantic::SymbolId, Vec<crate::signals::analysis::write_sites::RecordConstruction>> = BTreeMap::new();
        for node in ctx.parse.nodes.iter() {
            let AstKind::CallExpression(call) = node.kind() else { continue };
            let oxc_ast::ast::Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else { continue };
            // `ring.put(rec)`, or the method read first and called on the
            // ring: `f = ring.put; f.call(ring, rec)`.
            let (receiver, argument) = if callee.property.name == "put" && call.arguments.len() == 1 {
                (callee.object.without_parentheses().span(), &call.arguments[0])
            } else if callee.property.name == "call" && call.arguments.len() == 2 {
                let Some(method) = ctx.node_with_span(callee.object.without_parentheses().span()) else { continue };
                if !matches!(ctx.parse.nodes.kind(method), AstKind::IdentifierReference(_)) {
                    continue;
                }
                let Some(value) = ctx.reaching_value(method) else { continue };
                let AstKind::StaticMemberExpression(put) = ctx.parse.nodes.kind(value) else { continue };
                let Some(this) = call.arguments[0].as_expression() else { continue };
                let (Some(owner), Some(this)) = (
                    ctx.node_with_span(put.object.without_parentheses().span()),
                    ctx.node_with_span(this.without_parentheses().span()),
                ) else {
                    continue;
                };
                if put.property.name != "put" || alias_root(ctx, owner).is_none() || alias_root(ctx, owner) != alias_root(ctx, this) {
                    continue;
                }
                (this_span(ctx, this), &call.arguments[1])
            } else {
                continue;
            };
            let Some(receiver) = ctx.node_with_span(receiver) else { continue };
            let Some(argument) = argument.as_expression() else { continue };
            let Some(argument) = ctx.node_with_span(argument.without_parentheses().span()) else { continue };
            let (Some(ring), Some(literal)) = (alias_root(ctx, receiver), object_literal(ctx, argument)) else { continue };
            if let Some(record) = ctx.record_at(literal) {
                out.entry(ring).or_default().push(record);
            }
        }
        out
    })
}

fn this_span(ctx: &FinderCtx, node: oxc_semantic::NodeId) -> oxc_span::Span {
    ctx.parse.nodes.kind(node).span()
}

fn ring_records(ctx: &FinderCtx, ring: oxc_semantic::SymbolId) -> Vec<crate::signals::analysis::write_sites::RecordConstruction> {
    ring_puts(ctx).get(&ring).cloned().unwrap_or_default()
}

/// Event names a handler function is registered (or removed) for: a call
/// passing an event-name string and then the handler.
fn handler_events(ctx: &FinderCtx, function: oxc_semantic::NodeId) -> BTreeSet<String> {
    use oxc_ast::AstKind;
    const EVENTS: &[&str] = &["mousedown", "mouseup", "click", "mousemove", "touchstart", "touchmove", "touchend", "touchcancel"];
    let mut out = BTreeSet::new();
    for node in ctx.parse.nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else { continue };
        let arguments: Vec<_> = call.arguments.iter().filter_map(|argument| argument.as_expression()).collect();
        for (index, argument) in arguments.iter().enumerate() {
            let oxc_ast::ast::Expression::StringLiteral(literal) = argument.without_parentheses() else { continue };
            if !EVENTS.contains(&literal.value.as_str()) {
                continue;
            }
            let Some(handler) = arguments.get(index + 1) else { continue };
            let Some(handler) = ctx.node_with_span(handler.without_parentheses().span()) else { continue };
            let value = ctx.sym(handler);
            if value.alternatives().iter().any(|item| **item == Sym::Func(function)) {
                out.insert(literal.value.to_string());
            }
        }
    }
    out
}

/// UI event ring records: `ring[i].k` read at the write, named by what the
/// listeners store under `k` (the trust record's `instanceof UIEvent` /
/// `isTrusted`), and `ring[i].k[j]` by the event of the listener storing
/// `true` at `j` in its literal kind pair.
fn ui_event_samples(ctx: &FinderCtx, sym: &Sym, flags: &[String]) -> Result<Option<String>, String> {
    agree(ctx, sym, flags, |ctx, alt, _| {
        let alt = unread(alt);
        let (record, key, element) = match alt {
            Sym::Member(record, key) => (record.as_ref(), key, None),
            Sym::Index(list, index) => match (unread(list), index.as_ref()) {
                (Sym::Member(record, key), Sym::Num(index)) => (record.as_ref(), key, Some(*index as usize)),
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        // The ring: the read binding with `put` records.
        let mut rings = BTreeSet::new();
        for node in record.nodes() {
            if let Sym::Read(symbol, _) = node {
                if !ring_records(ctx, *symbol).is_empty() {
                    rings.insert(*symbol);
                }
            }
        }
        let [ring] = rings.into_iter().collect::<Vec<_>>()[..] else { return Ok(None) };
        let mut found: BTreeSet<String> = BTreeSet::new();
        for record in ring_records(ctx, ring) {
            let Some(value) = record.values.get(key) else { continue };
            match element {
                None => {
                    let stored = fold_member(&ctx.sym(*value));
                    let text = stored.render();
                    if text.contains("instanceof") && stored.has_global("UIEvent") {
                        found.insert("instanceOfUIEvent".into());
                    } else if stored.has_member("isTrusted") {
                        found.insert("isTrusted".into());
                    } else {
                        return Err(format!("record value `{}`", clip(&text)));
                    }
                }
                Some(index) => {
                    let Sym::Array(items) = ctx.sym(*value) else { return Err("kind pair".into()) };
                    match items.get(index) {
                        Some(Sym::Bool(true)) => {}
                        Some(Sym::Bool(false)) => continue,
                        _ => return Err("kind pair element".into()),
                    }
                    let function = ctx.flow_root(record.node);
                    if !matches!(ctx.parse.nodes.kind(function), oxc_ast::AstKind::Function(_) | oxc_ast::AstKind::ArrowFunctionExpression(_)) {
                        return Err("listener".into());
                    }
                    let events = handler_events(ctx, function);
                    let [event] = events.iter().collect::<Vec<_>>()[..] else {
                        return Err(format!("listener events {events:?}"));
                    };
                    let mut name = event.clone();
                    let rest = name.split_off(5);
                    found.insert(format!("is{}{}", name[..1].to_uppercase() + &name[1..], rest));
                }
            }
        }
        match found.len() {
            0 => Ok(None),
            1 => Ok(found.into_iter().next()),
            _ => Err(format!("records read {found:?}")),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every registered field's flag list exists on its card.
    #[test]
    fn signal_fields_have_card_flags() {
        for entry in SIGNALS {
            let flags = all_flags(entry).unwrap_or_else(|| panic!("{}: card has no flags list", entry.signal));
            assert!(!flags.is_empty(), "{}: empty flags", entry.signal);
        }
    }
}
