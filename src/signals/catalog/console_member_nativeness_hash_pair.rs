//! `ConsoleMemberNativenessHashPair`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ConsoleMemberNativenessHashPair`. Card: [`CARD`].
pub(crate) fn recognize_console_member_nativeness_hash_pair(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    // A first-char-code sum reduced `% m` (the built-in probe checksum, whose
    // probe table also lists `console` members) is not a names hash.
    let char_code_modulus = view.leaves().any(|leaf| {
        leaf.transforms.iter().any(|t| t == "%") && leaf.transforms.iter().any(|t| t == ".charCodeAt()" || t == ".charCodeAt")
    });
    (plain_ops(view, &[LeafOp::Number, LeafOp::Number])
        && !char_code_modulus
        && probes(view, &census, "console")
        && census.calls("getOwnPropertyDescriptor")
        && census.list_native.len() >= 2)
        .then(|| Claim::new(["console member descriptors", "native-code test splits two lists", "two hashes"]))
}

pub(crate) fn bind_console_member_nativeness_hash_pair(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let census = census(view, ctx);
    let names = agreed_names(&census, |args| {
        let [list] = args.sorted.as_slice() else {
            return Ok(None);
        };
        let native = census
            .list_native
            .get(list)
            .ok_or_else(|| BindRefusal::new("hashed list is not one of the classified lists"))?;
        Ok(Some(match native.iter().copied().collect::<Vec<bool>>().as_slice() {
            [true] => "nativeNamesHash",
            [false] => "nonNativeNamesHash",
            _ => return Err(BindRefusal::new("hashed list is pushed under both native-test polarities")),
        }))
    })?;
    bind_in_order(layout, &names)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ConsoleMemberNativenessHashPair`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ConsoleMemberNativenessHashPair",
    summary: "Takes the own property names of the page's console object (up to 2000) and checks each member's function (value, else setter) with Function.prototype.toString: if the text does not end in '[native code]' the name goes in the non-native list, otherwise in the native-or-other list. The work is done in small time slices; when finished it sorts each list, joins it with NUL characters and sends one hash per list. If console is missing, both hashes are 0.",
    why: "Non-native console methods reveal wrappers added by monitoring libraries, extensions, anti-debugging code or automation tools, while the full name set reflects the browser; this likely helps detect tampered or instrumented environments.",
    sample_payload: Some(r#"{
  "nativeNamesHash": -930873437,
  "nonNativeNamesHash": 0
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "nonNativeNamesHash",
            "32-bit hash of the sorted, NUL-joined names of console members whose function is not native code (wrapped, patched or added by scripts); 0 if there are none.",
        ),
        CardField::number(
            "nativeNamesHash",
            "32-bit hash of the sorted, NUL-joined names of all other console members (native functions and non-function properties); 0 if there are none.",
        ),
    ]),
};
