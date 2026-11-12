//! The per-signal pages under `docs/signals/`, rendered from the cards.
//!
//! `shape signals docs <dir>` writes `<signal>.md` for every registered
//! signal and the index `README.md`; `docs/signals/schema.md` and the
//! data-shape `<signal>.js` examples are hand-maintained. The test
//! `pages_match_cards` keeps the checked-in pages equal to the rendering.

use crate::signals::catalog::card::{Card, CardBody, CardField, CardFieldKind};

/// The file stem of a signal's page and sample (`DeviceMemory` ->
/// `device_memory`); equal to the signal's catalog file name.
pub fn page_stem(signal: &str) -> String {
    match signal {
        "WebGlContext" => return "webgl_context".into(),
        "WebGlRenderHash" => return "webgl_render_hash".into(),
        "WebRtcIceCandidateAddresses" => return "webrtc_ice_candidate_addresses".into(),
        "WebAuthnInterfacePresence" => return "webauthn_interface_presence".into(),
        "BraveDuckDuckGoNavigatorFlags" => return "brave_duckduckgo_navigator_flags".into(),
        _ => {}
    }
    // `aB` -> `a_B`, then `ABc` -> `A_Bc`, lower-cased.
    let chars: Vec<char> = signal.chars().collect();
    let mut out = String::new();
    for (index, &c) in chars.iter().enumerate() {
        if index > 0 && c.is_ascii_uppercase() {
            let previous = chars[index - 1];
            let next_lower = chars.get(index + 1).is_some_and(|n| n.is_ascii_lowercase());
            if previous.is_ascii_lowercase() || previous.is_ascii_digit() || (previous.is_ascii_uppercase() && next_lower) {
                out.push('_');
            }
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// HTML-escape and fold line breaks (with their surrounding blanks).
fn esc(text: &str) -> String {
    let escaped = text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut out = String::new();
    let mut pending = String::new();
    let mut newline = false;
    for c in escaped.chars() {
        if c.is_whitespace() {
            pending.push(c);
            newline |= c == '\n';
            continue;
        }
        if newline {
            out.push(' ');
        } else {
            out.push_str(&pending);
        }
        pending.clear();
        newline = false;
        out.push(c);
    }
    if newline {
        out.push(' ');
    } else {
        out.push_str(&pending);
    }
    out
}

fn cell(text: &str) -> String {
    esc(text).replace('|', "\\|")
}

fn code(text: &str) -> String {
    if text.contains('`') {
        format!("`` {text} ``")
    } else {
        format!("`{text}`")
    }
}

/// The first sentence of `text`, cut at a word boundary to at most `max`
/// UTF-16 units plus ` …`.
fn first_sentence(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut sentence: Vec<char> = chars.clone();
    for (index, &c) in chars.iter().enumerate() {
        if c == '\n' {
            break;
        }
        if !matches!(c, '.' | '!' | '?') {
            continue;
        }
        let rest = &chars[index + 1..];
        let blanks = rest.iter().take_while(|c| c.is_whitespace()).count();
        let ends = rest.is_empty() || (blanks > 0 && rest.get(blanks).is_some_and(|c| c.is_ascii_uppercase() || *c == '('));
        if ends {
            sentence = chars[..=index].to_vec();
            break;
        }
    }
    let units: Vec<u16> = sentence.iter().collect::<String>().encode_utf16().collect();
    if units.len() <= max {
        return sentence.into_iter().collect();
    }
    let cut = String::from_utf16_lossy(&units[..max]);
    let cut = &cut[..cut.rfind(' ').unwrap_or(0)];
    let cut = cut.trim_end_matches(|c: char| matches!(c, ',' | ';' | ':' | '(') || c.is_whitespace());
    format!("{cut} …")
}

fn type_label(field: &CardField) -> String {
    let mut label = match field.kind {
        _ if field.boolean => "boolean (tag byte)".to_string(),
        CardFieldKind::Byte if field.null_marker => "null (tag byte)".to_string(),
        CardFieldKind::Number if field.null_marker => "number or null (tag byte)".to_string(),
        CardFieldKind::String if field.null_marker => "string or null (tag byte)".to_string(),
        CardFieldKind::Group(_) => "group".to_string(),
        _ if !field.flags.is_empty() => "flags (byte)".to_string(),
        CardFieldKind::Number => "number".to_string(),
        CardFieldKind::Byte => "byte".to_string(),
        CardFieldKind::String => "string".to_string(),
    };
    if let Some(repeat) = field.repeat.filter(|repeat| !repeat.is_empty()) {
        label.push_str(&format!(", repeated ({repeat})"));
    }
    label
}

/// Table rows of `fields`; nested groups use `parent[].child`.
fn rows(fields: &[CardField], prefix: &str, out: &mut Vec<String>, flag_lists: &mut Vec<(String, &'static [&'static str])>) {
    for field in fields {
        let name = format!("{prefix}{}", field.name);
        let meaning = match field.kind {
            CardFieldKind::Group(_) => "One entry per repetition with the fields below.",
            _ => field.meaning,
        };
        out.push(format!("| {} | {} | {} |", code(&name), cell(&type_label(field)), cell(meaning)));
        if !field.flags.is_empty() {
            flag_lists.push((name.clone(), field.flags));
        }
        let nested = field.fields();
        if !nested.is_empty() {
            let separator = if field.repeat.is_some_and(|r| !r.is_empty()) { "[]." } else { "." };
            rows(nested, &format!("{name}{separator}"), out, flag_lists);
        }
    }
}

fn field_table(fields: &[CardField]) -> String {
    if fields.is_empty() {
        return "No fields: the message is empty.\n".to_string();
    }
    let mut lines = vec!["| Field | Type | Meaning |".to_string(), "| --- | --- | --- |".to_string()];
    let mut flag_lists = Vec::new();
    rows(fields, "", &mut lines, &mut flag_lists);
    let mut text = lines.join("\n") + "\n";
    for (name, flags) in flag_lists {
        text.push_str(&format!(
            "\nFlag names of {} (card order; the bit of each flag is per build, see the schema's `bits`):\n\n",
            code(&name)
        ));
        text.push_str(&flags.iter().map(|flag| format!("- {}", code(flag))).collect::<Vec<_>>().join("\n"));
        text.push('\n');
    }
    text
}

fn schema_note(card: &Card) -> String {
    if card.no_data() {
        return format!(
            "In `shape signals <program.js> --json` output this signal is `signals.{}` (see [schema format](schema.md)). \
             Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).\n\n\
             - No data: `type: \"no_data\"`, the slot's routing `name` (or `positional`), `fields: {{}}` and no \
             `order` or `mask`; `effect` names what it does on the page in that build.\n",
            card.signal
        );
    }
    let fields = card.all_fields();
    let has_flags = fields.iter().any(|field| !field.flags.is_empty());
    let repeats = |field: &CardField| field.repeat.is_some_and(|r| !r.is_empty());
    let has_repeat = fields.iter().any(|field| repeats(field));
    let has_group = fields.iter().any(|field| repeats(field) && !field.fields().is_empty());
    let mut out = vec![
        format!(
            "In `shape signals <program.js> --json` output this signal is `signals.{}` (see [schema format](schema.md)). \
             Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).",
            card.signal
        ),
        String::new(),
    ];
    if card.by_name {
        out.push(
            "- Sent **by name** through the generic encoder: `type: \"name\"` with the build's routing `name` \
             (no `positional`), signal `mask: null`; each value is typed `\"value\"` and keeps its wire `key`."
                .into(),
        );
    } else {
        out.push(
            "- Positional: `type: \"positional\"` with its `positional` index and the signal `mask` \
             (the salt of the stream its fields are XOR-masked with)."
                .into(),
        );
    }
    match card.body {
        CardBody::Messages(messages) => out.push(format!(
            "- It has {} message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) \
             instead of top-level `order`/`fields`.",
            messages.len()
        )),
        CardBody::Fields(_) => out.push(
            "- `order` lists the field names above in this build's write order (it varies per build); \
             `fields` gives each one's wire type."
                .into(),
        ),
    }
    if has_repeat {
        out.push(format!(
            "- Repeated fields are `type: \"array\"` with an `item`{}",
            if has_group { "; a repeated group's item is a `record` with its own `order` and `fields`." } else { "." }
        ));
    }
    if has_flags {
        out.push(
            "- Flag bytes are `type: \"flags\"` with `bits` mapping each flag name above to its bit \
             (0 = least significant) in this build. If the bits cannot all be named the field stays \
             `type: \"byte\"` and is listed under `diagnostics.flags_fallback`."
                .into(),
        );
        if let Some(rule) = crate::signals::wire::readable::GROUPS.iter().find(|r| r.signal == card.signal) {
            out.push(format!("- Fresh schemas/readable payloads group `{}` through `{}` as `{}[0..{}]`. This is a heterogeneous fixed array of named boolean objects; each index retains its original byte's bit map. Physical field names above identify source writes, not transmitted JSON keys.", rule.old(0), rule.old(rule.count - 1), rule.parent, rule.count - 1));
        } else if card.messages().iter().any(|(_, fields)| matches!(*fields, [field] if !field.flags.is_empty() && field.repeat.is_none())) {
            out.push("- When the entire resolved message is one packed flag byte, fresh schemas/readable payloads expose its named booleans at the signal root. The schema retains the original byte under `physicalFlags`; the wrapper is a projection name, not a transmitted object.".into());
        }
    }
    out.join("\n") + "\n"
}

/// Shared boundary between documentary examples and source-derived decoding.
pub const SAMPLE_NOTE: &str = "Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.";

/// Render the card-owned sample for docs and catalogue views.
pub fn render_sample(card: &Card) -> String {
    let body = match card.sample_payload {
        Some(sample) => {
            let value: serde_json::Value = serde_json::from_str(sample)
                .expect("catalogue sample_payload must be valid JSON");
            format!("```json\n{}\n```", serde_json::to_string_pretty(&value).unwrap())
        }
        None if card.no_data() => "No payload emitted.".into(),
        None => "No decoded sample available.".into(),
    };
    format!("## Sample payload\n\n{SAMPLE_NOTE}\n\n{body}\n")
}

/// Catalogue text, without page-relative documentation links.
pub fn render_catalogue(card: &Card) -> String {
    let mut text = format!("# {}\n\n{}\n\n## Why\n\n{}\n\n## Fields\n\n", card.signal, card.summary, card.why);
    if card.no_data() {
        text.push_str("No data: completion only.\n\n");
    } else {
        if card.by_name {
            text.push_str("Sent by name through the generic encoder; routing keys vary by seed.\n\n");
        }
        for (when, fields) in card.messages() {
            if let Some(when) = when {
                text.push_str(&format!("### {when}\n\n"));
            }
            text.push_str(&field_table(fields));
            text.push('\n');
        }
    }
    text.push_str(&render_sample(card));
    text
}

/// Structured catalogue view; samples are JSON values, never quoted JSON text.
pub fn catalogue_json(card: &Card) -> serde_json::Value {
    use serde_json::{json, Value};
    fn field(f: &CardField) -> Value {
        json!({"name": f.name, "type": type_label(f), "meaning": f.meaning,
            "repeat": f.repeat, "flags": f.flags,
            "fields": f.fields().iter().map(field).collect::<Vec<_>>()})
    }
    let mut out = json!({"signal": card.signal, "summary": card.summary, "why": card.why,
        "by_name": card.by_name, "no_data": card.no_data(),
        "messages": card.messages().into_iter().map(|(when, fields)|
            json!({"when": when, "fields": fields.iter().map(field).collect::<Vec<_>>()})
            ).collect::<Vec<_>>()});
    // Null here is metadata absence. `no_data` distinguishes completion-only
    // cards from data cards whose decoded sample has not been captured yet.
    out["sample_payload"] = card.sample_payload.map(|sample| {
        serde_json::from_str(sample).expect("catalogue sample_payload must be valid JSON")
    }).unwrap_or(Value::Null);
    out
}

/// The page `<signal>.md` of one card.
pub fn render_page(card: &Card) -> String {
    let stem = page_stem(card.signal);
    let mut out = vec![
        format!("# {}", card.signal),
        String::new(),
        format!("[Signals index](README.md) · [data-shape example]({stem}.js) · [schema format](schema.md)"),
        String::new(),
        "## What it collects".into(),
        String::new(),
        esc(card.summary),
        String::new(),
        "## Why".into(),
        String::new(),
        esc(card.why),
        String::new(),
        "## Fields".into(),
        String::new(),
    ];
    if card.no_data() {
        out.push(
            "No data: the slot completes without ever collecting a value, so nothing of it reaches the payload \
             but the not-collected marker."
                .into(),
        );
        out.push(String::new());
    } else if card.by_name {
        out.push(
            "Sent by name through the generic encoder: the value is handed to the shared encoder as one named \
             object (or bare value), not written field by field with a positional index."
                .into(),
        );
        out.push(String::new());
    }
    match card.body {
        CardBody::Messages(messages) => {
            out.push(format!("This signal sends one of {} message shapes.", messages.len()));
            out.push(String::new());
            for (index, message) in messages.iter().enumerate() {
                out.push(format!("### Message {}: {}", index + 1, esc(message.when)));
                out.push(String::new());
                out.push(field_table(message.fields));
            }
        }
        CardBody::Fields(_) if card.no_data() => {}
        CardBody::Fields(fields) => out.push(field_table(fields)),
    }
    out.push(render_sample(card));
    out.push("## In a build's schema".into());
    out.push(String::new());
    out.push(schema_note(card));
    let mut text = out.join("\n");
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    text.trim_end().to_string() + "\n"
}

/// The index `README.md`; `has_sample(stem)` tells whether `<stem>.js`
/// exists.
pub fn render_index(cards: &[&Card], has_sample: impl Fn(&str) -> bool) -> String {
    let mut cards = cards.to_vec();
    cards.sort_by(|a, b| index_key(a.signal).cmp(&index_key(b.signal)));
    let mut out = vec![
        "# Signals".to_string(),
        String::new(),
        "A signal is one browser measurement that the Shape client collects and adds to its payload. Each emission \
         starts with a 4-byte random seed, then writes its fields (numbers, strings, single bytes) XOR-masked with a \
         stream seeded from that value and the signal's mask constant (its salt). Field order, masks, flag bit \
         positions and slot addresses change with every build, so the layout of one build comes from the CLI:"
            .into(),
        String::new(),
        "```sh".into(),
        "shape signals <program.js> --json".into(),
        "```".into(),
        String::new(),
        "The input is a deobfuscated program (`shape trace program` output). The report names every recognized slot \
         with one of the canonical signal names below and its fields with the names used on these pages."
            .into(),
        String::new(),
        "## Reading a schema".into(),
        String::new(),
        "See [schema format](schema.md) for the report layout: build header, slot address, routing (positional or by \
         name), masks, field order, field types, flags with bits, and diagnostics."
            .into(),
        String::new(),
        "## Per-signal pages".into(),
        String::new(),
        "Each signal has an explanation page (`<signal>.md`: what it collects, why, its fields, and where it appears \
         in a schema) and a static JavaScript data-shape example (`<signal>.js`). The default export is the payload \
         itself: a data literal, including bare scalars and arrays. These modules contain no collectors or encoding logic."
            .into(),
        String::new(),
        "Comments label captured values versus illustrative values derived from the card and readable projection. \
         A captured default documents one observation, not a complete runtime schema. Additional named data exports \
         illustrate message alternatives, conditional fields and representative items absent from an empty capture. \
         No-data slots export `undefined`; an illustrative `undefined` variant can also document a path that emits \
         nothing. JavaScript numeric constants such as `NaN` preserve special measurement values without byte metadata. \
         The Markdown field tables and [schema format](schema.md) remain the wire reference; data examples omit \
         encoding-only lengths, prefixes and padding, while retaining meaningful measurements.".into(),
        String::new(),
        format!("## Index ({} signals)", cards.len()),
        String::new(),
        "| Signal | Data example | Collects |".into(),
        "| --- | --- | --- |".into(),
    ];
    let mut missing = Vec::new();
    for card in &cards {
        let stem = page_stem(card.signal);
        let sample = if has_sample(&stem) {
            format!("[data]({stem}.js)")
        } else {
            missing.push(card.signal);
            "none".into()
        };
        out.push(format!(
            "| [{}]({stem}.md) | {sample} | {} |",
            card.signal,
            cell(&first_sentence(card.summary, 200))
        ));
    }
    if !missing.is_empty() {
        out.push(String::new());
        out.push(format!("No data-shape example exists for: {}.", missing.join(", ")));
    }
    out.join("\n") + "\n"
}

/// Index order: case-insensitive, then case-sensitive.
fn index_key(signal: &str) -> (String, String) {
    (signal.to_lowercase(), signal.to_string())
}

/// Every page of `docs/signals/` rendered from the registered cards:
/// `(file name, text)` for each `<signal>.md` and `README.md`.
pub fn render_pages(has_sample: impl Fn(&str) -> bool) -> Vec<(String, String)> {
    let cards: Vec<&Card> = crate::signals::catalog::cards().collect();
    let mut pages: Vec<(String, String)> =
        cards.iter().map(|card| (format!("{}.md", page_stem(card.signal)), render_page(card))).collect();
    pages.push(("README.md".into(), render_index(&cards, has_sample)));
    pages
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn docs_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/signals")
    }

    /// The checked-in pages equal the rendering of the cards
    /// (`shape signals docs docs/signals` regenerates them).
    #[test]
    fn pages_match_cards() {
        let dir = docs_dir();
        let mut stale = Vec::new();
        for (name, text) in render_pages(|stem| dir.join(format!("{stem}.js")).exists()) {
            let on_disk = std::fs::read_to_string(dir.join(&name)).unwrap_or_default();
            if on_disk != text {
                stale.push(name);
            }
        }
        assert!(stale.is_empty(), "stale docs/signals pages (run `shape signals docs docs/signals`): {stale:?}");
    }

    /// Every page stem is its signal's catalog file name.
    #[test]
    fn page_stems_are_catalog_files() {
        let catalog = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/signals/catalog");
        for card in crate::signals::catalog::cards() {
            let stem = page_stem(card.signal);
            assert!(catalog.join(format!("{stem}.rs")).exists(), "{}: {stem}.rs", card.signal);
        }
    }
}
