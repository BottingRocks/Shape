//! `CanvasToDataUrl`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::extract::contracts::SourceClass;
use crate::signals::analysis::{api_matches, BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, LeafKind, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


fn has_class(leaves: &[&LeafView], class: SourceClass) -> bool {
    leaves.iter().any(|leaf| leaf.classes.contains(&class))
}

/// `CanvasToDataUrl`. Card: [`CARD`].
///
/// Shape: three scalars, one string, two numbers (the fallback message has a
/// byte for the missing timing). Evidence: `toDataURL` on the value path.
pub(crate) fn recognize_canvas_to_data_url(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "CanvasToDataUrl")
}

/// Per message: the string is the prefix; the timing is the clock-read
/// number (or the null byte of the fallback); the remaining number is the
/// hash.
pub(crate) fn bind_canvas_to_data_url(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    const SIGNAL: &str = "CanvasToDataUrl";
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let fields = &layout_message.fields;
        let leaves = field_leaves(view, layout, message).ok_or_else(|| refuse(SIGNAL, "leaf alignment failed"))?;
        let kind = |index: usize| match &fields[index].node {
            LayoutNode::Leaf(leaf) => Some(leaf.kind),
            _ => None,
        };
        let n = fields.len();
        if !(2..=3).contains(&n) || (0..n).any(|index| kind(index).is_none()) {
            return Err(refuse(SIGNAL, format!("message {message}: expected two or three scalar fields")));
        }
        let strings: Vec<usize> = (0..n).filter(|i| kind(*i) == Some(LeafKind::String)).collect();
        let bytes: Vec<usize> = (0..n).filter(|i| kind(*i) == Some(LeafKind::Byte)).collect();
        let numbers: Vec<usize> = (0..n).filter(|i| kind(*i) == Some(LeafKind::Number)).collect();
        let [prefix] = strings.as_slice() else {
            return Err(refuse(SIGNAL, format!("message {message}: expected one string")));
        };
        if let ([], [hash]) = (bytes.as_slice(), numbers.as_slice()) {
            // Builds without the timing field: prefix and hash only.
            binding.insert(message, fields[*prefix].key.clone(), "dataUrlPrefix");
            binding.insert(message, fields[*hash].key.clone(), "dataUrlHash");
            continue;
        }
        let (elapsed, hash) = match (bytes.as_slice(), numbers.as_slice()) {
            ([elapsed], [hash]) => (*elapsed, *hash),
            ([], [a, b]) => {
                let clock = |index: usize| {
                    has_class(&leaves[index], SourceClass::HostClock)
                        || leaves[index].iter().any(|leaf| {
                            leaf.apis.iter().any(|api| api_matches(api, "now()"))
                                || leaf.transforms.iter().any(|t| t == ".now()")
                        })
                };
                match (clock(*a), clock(*b)) {
                    (true, false) => (*a, *b),
                    (false, true) => (*b, *a),
                    _ => return Err(refuse(SIGNAL, format!("message {message}: no single clock-read number"))),
                }
            }
            _ => return Err(refuse(SIGNAL, format!("message {message}: unexpected field kinds"))),
        };
        binding.insert(message, fields[*prefix].key.clone(), "dataUrlPrefix");
        binding.insert(message, fields[elapsed].key.clone(), "elapsedMs");
        binding.insert(message, fields[hash].key.clone(), "dataUrlHash");
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `CanvasToDataUrl`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "CanvasToDataUrl",
    summary: "Creates a fresh, detached canvas at the default 300x150 size, draws the text \"Hel$&?6%){mZ+#@\" in 18pt Sans at (2, 2), and exports it with toDataURL(\"image/png\"). It hashes the whole data URL in small time-sliced steps and sends the hash, the time since a start stamp, and the first 50 characters of the data URL. If anything throws while drawing or exporting, it sends a fixed placeholder record instead.",
    why: "Text rendering (fonts, anti-aliasing, sub-pixel layout) and PNG encoding differ by OS, GPU, installed fonts and browser, so the image hash is a classic canvas fingerprint. Comparing runs likely also reveals canvas noise injection, and the timing may expose slow or emulated environments.",
    sample_payload: Some(r#"{
  "dataUrlHash": -1952225390,
  "dataUrlPrefix": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAASwA",
  "elapsedMs": 20
}"#),
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "drawing and export worked",
            fields: &[
                CardField::number("dataUrlHash", "Signed 32-bit hash of the full PNG data URL string."),
                CardField::number(
                    "elapsedMs",
                    "Milliseconds from a start stamp taken earlier in the collection step until hashing finished; includes other drawing work, toDataURL and scheduler delay.",
                ),
                CardField::string(
                    "dataUrlPrefix",
                    "First 50 characters of the data URL (normally \"data:image/png;base64,\" plus the start of the base64 PNG).",
                ),
            ],
        },
        CardMessage {
            when: "drawing or export threw an error",
            fields: &[
                CardField::number("dataUrlHash", "Always 92629711, the hash of the placeholder string."),
                CardField::null("elapsedMs", "Always the null marker (no timing)."),
                CardField::string(
                    "dataUrlPrefix",
                    "Always the placeholder \"data:image/png;base64,____________SAFE____________\".",
                ),
            ],
        },
    ]),
};
