//! `CleanConsoleStackAccessFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, native_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `CleanConsoleStackAccessFlags`. Card: [`CARD`].
pub(crate) fn recognize_clean_console_stack_access_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    (console_probe(view, ctx) == Some(ConsoleRealm::CleanRealm))
        .then(|| claim(&["stack getter probes logged via console.log and a realm callback's console"]))
        .flatten()
}

pub(crate) fn bind_clean_console_stack_access_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_bytes_in_order(layout, &["consoleStackFlags"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `CleanConsoleStackAccessFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "CleanConsoleStackAccessFlags",
    summary: "Logs a probe Error whose 'stack' property is a trap (a getter) and records whether anything read it. One probe goes through the page's own console.log; another goes through the console of a freshly created, hidden nested iframe, which page scripts have not wrapped. It also checks for Sentry's saved original console.log and calls it. It sends a snapshot of the flags at the start and after the Sentry check, plus a new message every time a trap fires, so one run can send several messages. If the page has no console, it sends nothing.",
    why: "Reading Error.stack inside console.log normally only happens when DevTools is open or an automation tool (Chrome DevTools Protocol with Runtime enabled) serializes console arguments, so this likely detects open DevTools, automation, or console-wrapping scripts. The clean-iframe bit separates real inspector activity from a page-level console hook.",
    sample_payload: Some(r#"{
  "cleanRealmStackRead": false,
  "pageConsoleStackRead": false,
  "sentryOriginalLogCalled": false
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "consoleStackFlags",
            "Three flags, other bits 0: cleanRealmStackRead (logging through the fresh nested iframe's console read the probe's stack), sentryOriginalLogCalled (console.log.__sentry_original__ exists, is a function, and the call returned normally), pageConsoleStackRead (logging through the page's console read the probe's stack). Each bit is 1 if true, 0 if false.",
        )
        .flags(&["cleanRealmStackRead", "sentryOriginalLogCalled", "pageConsoleStackRead"]),
    ]),
};
