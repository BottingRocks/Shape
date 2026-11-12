//! `FingerprintApiIntegrity`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, native_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `FingerprintApiIntegrity`. Card: [`CARD`].
pub(crate) fn recognize_fingerprint_api_integrity(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if view.leaves().any(|leaf| leaf.op == LeafOp::String || leaf.repeat_depth > 0) {
        return None;
    }
    let info = slot_info(view, ctx);
    // An emission the contract pass left unresolved (no leaves: the
    // tracer's packed-register buffers, `buf = [[], []]; buf[1][i] = …`)
    // says nothing either way; the resolved ones decide (the binder then
    // refuses the unresolved message).
    let counts: Vec<usize> = top_field_counts(view)
        .into_iter()
        .zip(view.emissions())
        .filter(|(_, emission)| !emission.leaves.is_empty())
        .map(|(count, _)| count)
        .collect();
    (integrity_kind(&counts, hash_slots(view), &info.facts) == Some(IntegrityKind::WithSourceHashes))
    .then(|| claim(&["eleven flag bytes of native checks", "source hashes of descriptor getters"]))
    .flatten()
}

/// Hash slot name per hashed property (`Object.getOwnPropertyDescriptor(o, "<p>")`).
const HASHED: &[(&str, &str)] = &[
    ("mimeTypes", "mimeTypesSourceHash"),
    ("toDataURL", "toDataURLSourceHash"),
    ("getImageData", "getImageDataSourceHash"),
    ("fillText", "fillTextSourceHash"),
    ("createElement", "createElementSourceHash"),
    ("userAgent", "userAgentSourceHash"),
    ("font", "fontSourceHash"),
    ("toBlob", "toBlobSourceHash"),
    ("plugins", "pluginsSourceHash"),
    ("strokeStyle", "strokeStyleSourceHash"),
    ("width", "screenWidthSourceHash"),
    ("fillStyle", "fillStyleSourceHash"),
    ("height", "screenHeightSourceHash"),
    ("createElementNS", "createElementNSSourceHash"),
    ("getContext", "getContextSourceHash"),
];

pub(crate) fn bind_fingerprint_api_integrity(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let info = slot_info_for_binder(view, ctx)?;
    // A null marker masked by a draw held in a register (`m = ks(); …;
    // buf[i] = m ^ 144`): the contract's logical value is the literal.
    let literal_value = |leaf: &crate::signals::analysis::LeafView| {
        leaf.op == LeafOp::Byte
            && ctx
                .caller_value_node(leaf)
                .is_some_and(|node| matches!(ctx.parse.nodes.kind(node), oxc_ast::AstKind::NumericLiteral(_)))
    };
    let hints = emission_hints(&info, view, |write, leaf| {
        if write.literal || literal_value(leaf) {
            None
        } else if leaf.op == LeafOp::Number {
            let names: Vec<&str> = HASHED
                .iter()
                .filter(|(property, _)| write.facts.string(property))
                .map(|(_, name)| *name)
                .collect();
            match names.as_slice() {
                [name] => Some(Hint::Name(name.to_string())),
                _ => Some(Hint::Name(String::from("?hash"))),
            }
        } else {
            Some(Hint::Flag)
        }
    });
    let fields = message_hints(layout, &hints)?;
    // A message without hashes (init) takes its hash-slot names from a
    // message whose flag bytes sit at the same positions.
    let mut resolved: Vec<Vec<Option<Hint>>> = Vec::new();
    for hints in &fields {
        if hints.iter().all(|hint| hint.is_some()) {
            resolved.push(hints.clone());
            continue;
        }
        let donor = fields.iter().find(|other| {
            other.len() == hints.len()
                && other.iter().all(|hint| hint.is_some())
                && other
                    .iter()
                    .zip(hints)
                    .all(|(a, b)| (a == &Some(Hint::Flag)) == (b == &Some(Hint::Flag)))
        });
        match donor {
            Some(donor) => resolved.push(donor.clone()),
            None => return Err(BindRefusal::new("hash-slot positions have no hashed message to name them")),
        }
    }
    let mut binding = FieldBinding::new();
    for (index, (message, hints)) in layout.messages.iter().zip(&resolved).enumerate() {
        let mut flag = 0;
        for (field, hint) in message.fields.iter().zip(hints) {
            let name = match hint {
                Some(Hint::Flag) => {
                    flag += 1;
                    if flag > 11 {
                        return Err(BindRefusal::new("more than eleven flag bytes"));
                    }
                    format!("integrityFlags{flag}")
                }
                Some(Hint::Name(name)) if !name.starts_with('?') => name.clone(),
                _ => {
                    return Err(BindRefusal::new(format!(
                        "message {index}: field {} hashes no single known property",
                        field.key
                    )))
                }
            };
            binding.insert(index, field.key.clone(), name);
        }
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `FingerprintApiIntegrity`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "FingerprintApiIntegrity",
    summary: "Checks whether 15 browser functions that fingerprinting scripts commonly read (screen width/height, navigator userAgent/plugins/mimeTypes, createElement/createElementNS, canvas getContext/toDataURL/toBlob, 2D getImageData/fillText and the strokeStyle/fillStyle/font getters) are genuine native code. For each function it tests that it is a function, that its toString text looks like native code, that it has no own prototype, and that deleting the prototype does not throw; it also checks that the properties are not shadowed on the instance objects, that plugins/mimeTypes have a real type tag, and whether a clean iframe toString could be used. That gives 72 true/false checks packed 7 per byte into 11 bytes. It sends one message at init with only the flags, and a second at collection that re-checks everything with a clean iframe's toString, keeps a flag only if it passed both times, and adds a hash of each function's source text.",
    why: "To detect anti-detect browsers, bot frameworks and privacy extensions that patch, wrap or proxy the very APIs used for fingerprinting, so spoofed fingerprint values can be distrusted (likely).",
    sample_payload: Some(r#"{
  "createElementNSSourceHash": -1308566534,
  "createElementSourceHash": 77957379,
  "fillStyleSourceHash": 547743182,
  "fillTextSourceHash": 321282129,
  "fontSourceHash": 1428428518,
  "getContextSourceHash": -1752233893,
  "getImageDataSourceHash": 967708324,
  "integrityChecks": [
    {
      "createElementNSLooksNative": true,
      "documentProtoCreateElementNSNotShadowed": true,
      "fillStylePrototypeDeletable": true,
      "fillTextIsFunction": true,
      "getContextLooksNative": true,
      "strokeStyleLooksNative": true,
      "toDataURLPrototypeDeletable": true
    },
    {
      "createElementIsFunction": true,
      "createElementLooksNative": true,
      "createElementNoOwnPrototype": true,
      "documentCreateElementNotShadowed": true,
      "getContextIsFunction": true,
      "navigatorUserAgentNotShadowed": true,
      "screenWidthIsFunction": true
    },
    {
      "createElementNSIsFunction": true,
      "createElementNSNoOwnPrototype": true,
      "fillStyleNoOwnPrototype": true,
      "screenHeightPrototypeDeletable": true,
      "screenWidthLooksNative": true,
      "toBlobLooksNative": true,
      "toDataURLIsFunction": true
    },
    {
      "createElementNSPrototypeDeletable": true,
      "documentCreateElementNSNotShadowed": true,
      "fillTextPrototypeDeletable": true,
      "mimeTypesNoOwnPrototype": true,
      "screenWidthPrototypeDeletable": true,
      "toBlobIsFunction": true,
      "userAgentNoOwnPrototype": true
    },
    {
      "getContextNoOwnPrototype": true,
      "getImageDataIsFunction": true,
      "getImageDataLooksNative": true,
      "mimeTypesPrototypeDeletable": true,
      "mimeTypesTagGenuine": true,
      "screenHeightNoOwnPrototype": true,
      "userAgentIsFunction": true
    },
    {
      "fillStyleIsFunction": true,
      "fontPrototypeDeletable": true,
      "screenHeightIsFunction": true,
      "screenWidthNoOwnPrototype": true,
      "strokeStylePrototypeDeletable": true,
      "toDataURLLooksNative": true,
      "userAgentPrototypeDeletable": true
    },
    {
      "documentProtoCreateElementNotShadowed": true,
      "fillTextNoOwnPrototype": true,
      "fontNoOwnPrototype": true,
      "mimeTypesLooksNative": true,
      "pluginsLooksNative": true,
      "strokeStyleNoOwnPrototype": true,
      "userAgentLooksNative": true
    },
    {
      "fillStyleLooksNative": true,
      "fontLooksNative": true,
      "navigatorPluginsNotShadowed": true,
      "pluginsPrototypeDeletable": true,
      "pluginsTagGenuine": true,
      "screenHeightNotShadowed": true,
      "toDataURLNoOwnPrototype": true
    },
    {
      "fontIsFunction": true,
      "getContextPrototypeDeletable": true,
      "iframeToStringUsed": true,
      "screenHeightLooksNative": true,
      "screenWidthNotShadowed": true,
      "strokeStyleIsFunction": true,
      "toBlobPrototypeDeletable": true
    },
    {
      "getImageDataNoOwnPrototype": true,
      "getImageDataPrototypeDeletable": true,
      "mimeTypesIsFunction": true,
      "navigatorMimeTypesNotShadowed": true,
      "pluginsIsFunction": true,
      "pluginsNoOwnPrototype": true,
      "toBlobNoOwnPrototype": true
    },
    {
      "createElementPrototypeDeletable": true,
      "fillTextLooksNative": true
    }
  ],
  "mimeTypesSourceHash": 1867944940,
  "pluginsSourceHash": -40820508,
  "screenHeightSourceHash": 792389511,
  "screenWidthSourceHash": 627726960,
  "strokeStyleSourceHash": 647317288,
  "toBlobSourceHash": -557684352,
  "toDataURLSourceHash": 1918935229,
  "userAgentSourceHash": -1733642987
}"#),
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "init: sent right away; the 15 hash slots are always null markers.",
            fields: &[
                CardField::byte(
                    "integrityFlags1",
                    "7 packed true/false checks: userAgent getter deleting prototype did not throw; createElementNS not shadowed on the document object; 2D fillText is a function; plugins getter toString looks native; plugins getter has no own prototype; 2D fillText deleting prototype did not throw; screen.width getter is a function. Each flag is 1 if the check passed at init.",
                )
                .flags(&[
                    "screenWidthIsFunction",
                    "screenWidthLooksNative",
                    "screenWidthNoOwnPrototype",
                    "screenWidthPrototypeDeletable",
                    "screenHeightIsFunction",
                    "screenHeightLooksNative",
                    "screenHeightNoOwnPrototype",
                    "screenHeightPrototypeDeletable",
                    "userAgentIsFunction",
                    "userAgentLooksNative",
                    "userAgentNoOwnPrototype",
                    "userAgentPrototypeDeletable",
                    "pluginsIsFunction",
                    "pluginsLooksNative",
                    "pluginsNoOwnPrototype",
                    "pluginsPrototypeDeletable",
                    "mimeTypesIsFunction",
                    "mimeTypesLooksNative",
                    "mimeTypesNoOwnPrototype",
                    "mimeTypesPrototypeDeletable",
                    "createElementIsFunction",
                    "createElementLooksNative",
                    "createElementNoOwnPrototype",
                    "createElementPrototypeDeletable",
                    "createElementNSIsFunction",
                    "createElementNSLooksNative",
                    "createElementNSNoOwnPrototype",
                    "createElementNSPrototypeDeletable",
                    "getContextIsFunction",
                    "getContextLooksNative",
                    "getContextNoOwnPrototype",
                    "getContextPrototypeDeletable",
                    "toDataURLIsFunction",
                    "toDataURLLooksNative",
                    "toDataURLNoOwnPrototype",
                    "toDataURLPrototypeDeletable",
                    "toBlobIsFunction",
                    "toBlobLooksNative",
                    "toBlobNoOwnPrototype",
                    "toBlobPrototypeDeletable",
                    "getImageDataIsFunction",
                    "getImageDataLooksNative",
                    "getImageDataNoOwnPrototype",
                    "getImageDataPrototypeDeletable",
                    "fillTextIsFunction",
                    "fillTextLooksNative",
                    "fillTextNoOwnPrototype",
                    "fillTextPrototypeDeletable",
                    "strokeStyleIsFunction",
                    "strokeStyleLooksNative",
                    "strokeStyleNoOwnPrototype",
                    "strokeStylePrototypeDeletable",
                    "fillStyleIsFunction",
                    "fillStyleLooksNative",
                    "fillStyleNoOwnPrototype",
                    "fillStylePrototypeDeletable",
                    "fontIsFunction",
                    "fontLooksNative",
                    "fontNoOwnPrototype",
                    "fontPrototypeDeletable",
                    "documentCreateElementNotShadowed",
                    "documentCreateElementNSNotShadowed",
                    "documentProtoCreateElementNotShadowed",
                    "documentProtoCreateElementNSNotShadowed",
                    "screenWidthNotShadowed",
                    "screenHeightNotShadowed",
                    "navigatorUserAgentNotShadowed",
                    "navigatorPluginsNotShadowed",
                    "navigatorMimeTypesNotShadowed",
                    "pluginsTagGenuine",
                    "mimeTypesTagGenuine",
                    "iframeToStringUsed",
                ]),
                CardField::null(
                    "mimeTypesSourceHash",
                    "Slot for the mimeTypes getter source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::null(
                    "toDataURLSourceHash",
                    "Slot for the canvas toDataURL source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::null(
                    "getImageDataSourceHash",
                    "Slot for the 2D getImageData source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::byte(
                    "integrityFlags2",
                    "7 packed true/false checks: screen.width getter has no own prototype; canvas toBlob deleting prototype did not throw; width not shadowed on the screen object; createElement has no own prototype; createElement not shadowed on the document object; screen.width getter toString looks native; userAgent getter toString looks native. Each flag is 1 if the check passed at init.",
                ),
                CardField::null(
                    "fillTextSourceHash",
                    "Slot for the 2D fillText source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::null(
                    "createElementSourceHash",
                    "Slot for the createElement source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::byte(
                    "integrityFlags3",
                    "7 packed true/false checks: clean iframe toString was used; plugins not shadowed on the navigator object; createElementNS has no own prototype; 2D fillStyle getter toString looks native; 2D fillStyle getter deleting prototype did not throw; canvas toDataURL toString looks native; canvas getContext toString looks native. Each flag is 1 if the check passed at init.",
                ),
                CardField::null(
                    "userAgentSourceHash",
                    "Slot for the userAgent getter source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::null(
                    "fontSourceHash",
                    "Slot for the 2D font getter source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::byte(
                    "integrityFlags4",
                    "7 packed true/false checks: mimeTypes has a genuine object type tag; userAgent not shadowed on the navigator object; screen.height getter deleting prototype did not throw; 2D strokeStyle getter toString looks native; canvas getContext is a function; plugins getter is a function; canvas toBlob has no own prototype. Each flag is 1 if the check passed at init.",
                ),
                CardField::null(
                    "toBlobSourceHash",
                    "Slot for the canvas toBlob source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::byte(
                    "integrityFlags5",
                    "7 packed true/false checks: canvas getContext has no own prototype; mimeTypes getter deleting prototype did not throw; screen.height getter toString looks native; userAgent getter is a function; 2D fillText has no own prototype; userAgent getter has no own prototype; plugins getter deleting prototype did not throw. Each flag is 1 if the check passed at init.",
                ),
                CardField::null(
                    "pluginsSourceHash",
                    "Slot for the plugins getter source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::byte(
                    "integrityFlags6",
                    "7 packed true/false checks: canvas toDataURL has no own prototype; createElement is a function; height not shadowed on the screen object; 2D font getter has no own prototype; canvas toBlob is a function; 2D getImageData has no own prototype; 2D strokeStyle getter deleting prototype did not throw. Each flag is 1 if the check passed at init.",
                ),
                CardField::null(
                    "strokeStyleSourceHash",
                    "Slot for the 2D strokeStyle getter source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::null(
                    "screenWidthSourceHash",
                    "Slot for the screen.width getter source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::byte(
                    "integrityFlags7",
                    "7 packed true/false checks: 2D font getter deleting prototype did not throw; createElement not overridden on Document.prototype; 2D font getter is a function; mimeTypes getter has no own prototype; screen.height getter is a function; 2D getImageData deleting prototype did not throw; createElementNS is a function. Each flag is 1 if the check passed at init.",
                ),
                CardField::null(
                    "fillStyleSourceHash",
                    "Slot for the 2D fillStyle getter source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::byte(
                    "integrityFlags8",
                    "7 packed true/false checks: 2D font getter toString looks native; canvas toBlob toString looks native; createElement deleting prototype did not throw; mimeTypes getter is a function; 2D fillStyle getter is a function; mimeTypes getter toString looks native; 2D getImageData is a function. Each flag is 1 if the check passed at init.",
                ),
                CardField::null(
                    "screenHeightSourceHash",
                    "Slot for the screen.height getter source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::null(
                    "createElementNSSourceHash",
                    "Slot for the createElementNS source-text hash; at init it is always a null marker (no hash sent).",
                ),
                CardField::byte(
                    "integrityFlags9",
                    "7 packed true/false checks: canvas toDataURL is a function; canvas getContext deleting prototype did not throw; mimeTypes not shadowed on the navigator object; plugins has a genuine object type tag; canvas toDataURL deleting prototype did not throw; screen.height getter has no own prototype; createElement toString looks native. Each flag is 1 if the check passed at init.",
                ),
                CardField::byte(
                    "integrityFlags10",
                    "7 packed true/false checks: 2D getImageData toString looks native; 2D fillText toString looks native; screen.width getter deleting prototype did not throw; 2D strokeStyle getter is a function; createElementNS deleting prototype did not throw; 2D strokeStyle getter has no own prototype; 2D fillStyle getter has no own prototype. Each flag is 1 if the check passed at init.",
                ),
                CardField::byte(
                    "integrityFlags11",
                    "2 packed true/false checks: createElementNS toString looks native; createElementNS not overridden on Document.prototype. Each flag is 1 if the check passed at init.",
                ),
                CardField::null(
                    "getContextSourceHash",
                    "Slot for the canvas getContext source-text hash; at init it is always a null marker (no hash sent).",
                ),
            ],
        },
        CardMessage {
            when: "collection: sent after re-checking with a hidden iframe's clean toString; carries the source hashes. Not sent if the collection callback aborts (for example no 2D canvas with measureText).",
            fields: &[
                CardField::byte(
                    "integrityFlags1",
                    "7 packed true/false checks: userAgent getter deleting prototype did not throw; createElementNS not shadowed on the document object; 2D fillText is a function; plugins getter toString looks native; plugins getter has no own prototype; 2D fillText deleting prototype did not throw; screen.width getter is a function. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::number(
                    "mimeTypesSourceHash",
                    "Signed 32-bit hash of the mimeTypes getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::number(
                    "toDataURLSourceHash",
                    "Signed 32-bit hash of the canvas toDataURL's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::number(
                    "getImageDataSourceHash",
                    "Signed 32-bit hash of the 2D getImageData's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::byte(
                    "integrityFlags2",
                    "7 packed true/false checks: screen.width getter has no own prototype; canvas toBlob deleting prototype did not throw; width not shadowed on the screen object; createElement has no own prototype; createElement not shadowed on the document object; screen.width getter toString looks native; userAgent getter toString looks native. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::number(
                    "fillTextSourceHash",
                    "Signed 32-bit hash of the 2D fillText's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::number(
                    "createElementSourceHash",
                    "Signed 32-bit hash of the createElement's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::byte(
                    "integrityFlags3",
                    "7 packed true/false checks: clean iframe toString was used; plugins not shadowed on the navigator object; createElementNS has no own prototype; 2D fillStyle getter toString looks native; 2D fillStyle getter deleting prototype did not throw; canvas toDataURL toString looks native; canvas getContext toString looks native. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::number(
                    "userAgentSourceHash",
                    "Signed 32-bit hash of the userAgent getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::number(
                    "fontSourceHash",
                    "Signed 32-bit hash of the 2D font getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::byte(
                    "integrityFlags4",
                    "7 packed true/false checks: mimeTypes has a genuine object type tag; userAgent not shadowed on the navigator object; screen.height getter deleting prototype did not throw; 2D strokeStyle getter toString looks native; canvas getContext is a function; plugins getter is a function; canvas toBlob has no own prototype. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::number(
                    "toBlobSourceHash",
                    "Signed 32-bit hash of the canvas toBlob's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::byte(
                    "integrityFlags5",
                    "7 packed true/false checks: canvas getContext has no own prototype; mimeTypes getter deleting prototype did not throw; screen.height getter toString looks native; userAgent getter is a function; 2D fillText has no own prototype; userAgent getter has no own prototype; plugins getter deleting prototype did not throw. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::number(
                    "pluginsSourceHash",
                    "Signed 32-bit hash of the plugins getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::byte(
                    "integrityFlags6",
                    "7 packed true/false checks: canvas toDataURL has no own prototype; createElement is a function; height not shadowed on the screen object; 2D font getter has no own prototype; canvas toBlob is a function; 2D getImageData has no own prototype; 2D strokeStyle getter deleting prototype did not throw. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::number(
                    "strokeStyleSourceHash",
                    "Signed 32-bit hash of the 2D strokeStyle getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::number(
                    "screenWidthSourceHash",
                    "Signed 32-bit hash of the screen.width getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::byte(
                    "integrityFlags7",
                    "7 packed true/false checks: 2D font getter deleting prototype did not throw; createElement not overridden on Document.prototype; 2D font getter is a function; mimeTypes getter has no own prototype; screen.height getter is a function; 2D getImageData deleting prototype did not throw; createElementNS is a function. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::number(
                    "fillStyleSourceHash",
                    "Signed 32-bit hash of the 2D fillStyle getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::byte(
                    "integrityFlags8",
                    "7 packed true/false checks: 2D font getter toString looks native; canvas toBlob toString looks native; createElement deleting prototype did not throw; mimeTypes getter is a function; 2D fillStyle getter is a function; mimeTypes getter toString looks native; 2D getImageData is a function. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::number(
                    "screenHeightSourceHash",
                    "Signed 32-bit hash of the screen.height getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::number(
                    "createElementNSSourceHash",
                    "Signed 32-bit hash of the createElementNS's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
                CardField::byte(
                    "integrityFlags9",
                    "7 packed true/false checks: canvas toDataURL is a function; canvas getContext deleting prototype did not throw; mimeTypes not shadowed on the navigator object; plugins has a genuine object type tag; canvas toDataURL deleting prototype did not throw; screen.height getter has no own prototype; createElement toString looks native. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::byte(
                    "integrityFlags10",
                    "7 packed true/false checks: 2D getImageData toString looks native; 2D fillText toString looks native; screen.width getter deleting prototype did not throw; 2D strokeStyle getter is a function; createElementNS deleting prototype did not throw; 2D strokeStyle getter has no own prototype; 2D fillStyle getter has no own prototype. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::byte(
                    "integrityFlags11",
                    "2 packed true/false checks: createElementNS toString looks native; createElementNS not overridden on Document.prototype. Each flag is 1 if the check passed both at init and now at collection.",
                ),
                CardField::number(
                    "getContextSourceHash",
                    "Signed 32-bit hash of the canvas getContext's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing.",
                ).nullable(),
            ],
        },
    ]),
};
