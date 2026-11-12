//! `ProductPageSnapshot`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_by_object_key, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ProductPageSnapshot`. Card: [`CARD`].
/// Wire key -> card name (the producer's own object keys, identical in every build).
const PRODUCT_PAGE_KEYS: [(&str, &str); 6] = [
    ("a", "pageTitle"),
    ("b", "addToCartLabel"),
    ("c", "productTitle"),
    ("d", "storeName"),
    ("e", "itemId"),
    ("f", "specificationsHash"),
];

pub(crate) fn recognize_product_page_snapshot(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // Named object of six DOM reads: querySelector(..).outerText / .ariaLabel /
    // .getAttribute(..), plus a hash over querySelectorAll(..) firstChild text.
    let keyed = view.is_generic_object()
        && view.emissions().iter().all(|emission| {
            emission.leaves.len() == PRODUCT_PAGE_KEYS.len()
                && emission.leaves.iter().all(|leaf| {
                    leaf.generic_key
                        .as_deref()
                        .is_some_and(|key| PRODUCT_PAGE_KEYS.iter().any(|(wire, _)| *wire == key))
                })
        });
    let dom = view.uses_members(&[
        "querySelector",
        "querySelectorAll",
        "outerText",
        "ariaLabel",
        "getAttribute",
        "firstChild",
    ]);
    (keyed && dom).then(|| {
        Claim::new([
            "named object of six keys",
            "querySelector(..).outerText/.ariaLabel/.getAttribute + querySelectorAll(..).firstChild",
        ])
    })
}

pub(crate) fn bind_product_page_snapshot(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_by_object_key(layout, &PRODUCT_PAGE_KEYS)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ProductPageSnapshot`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ProductPageSnapshot",
    summary: "In the collection step, reads six pieces of the current product page with fixed CSS selectors and sends them as one object {a, b, c, d, e, f}: the page title, the add-to-cart button's label, the product title, the selected store name, the item id from the data-tcin attribute, and a 32-bit hash of the product's specification labels. Three texts are cut to their first 20 characters. A missing element gives the empty string. It reads the page only; nothing is changed.",
    why: "Likely records which product and store the session is looking at when it sends data, so that traffic hitting one item (for example bots watching stock or buying up a limited product) can be grouped and told apart from normal shopping. The specifications hash lets the server check that the page content matches the item id without sending the full text (the use is inferred).",
    sample_payload: None,
    by_name: true,
    body: CardBody::Fields(&[
        CardField::string(
            "pageTitle",
            "Wire key a: the first 20 characters of document.querySelector('title').outerText; \"\" if there is no <title>.",
        ),
        CardField::string(
            "addToCartLabel",
            "Wire key b: the first 20 characters of the aria-label (ariaLabel) of the first button whose id starts with \"addToCart\"; \"\" if there is no such button.",
        ),
        CardField::string(
            "productTitle",
            "Wire key c: the first 20 characters of the outerText of h1[data-test=\"product-title\"] > span; \"\" if absent.",
        ),
        CardField::string(
            "storeName",
            "Wire key d: the full outerText of the first button whose id starts with \"store-name\" (the chosen store); \"\" if absent.",
        ),
        CardField::string(
            "itemId",
            "Wire key e: the data-tcin attribute (the retailer's item number) of the first div that has one; \"\" if absent (the -1 default is always overwritten).",
        ),
        CardField::number(
            "specificationsHash",
            "Wire key f: signed 32-bit hash (h = h*31 + pair of UTF-16 units) of the joined first-child text of every div[data-test=\"item-details-specifications\"] > div > div > b label; 0 when there are none (hash of the empty string).",
        ),
    ]),
};
