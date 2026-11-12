//! `ConsoleStackAccessFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, native_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ConsoleStackAccessFlags`. Card: [`CARD`].
pub(crate) fn recognize_console_stack_access_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    (console_probe(view, ctx) == Some(ConsoleRealm::DirectIframe))
        .then(|| claim(&["stack getter probes logged via console.log and a direct iframe contentWindow.console"]))
        .flatten()
}

pub(crate) fn bind_console_stack_access_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_bytes_in_order(layout, &["consoleStackFlags"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ConsoleStackAccessFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ConsoleStackAccessFlags",
    summary: "Logs a probe Error whose 'stack' property is a trap (a getter) and records whether anything read it. One probe goes through the page's console.log; a second goes through the console of a directly created hidden iframe. It also checks for Sentry's saved original console.log and calls it. It sends a snapshot of the flags at the start and after the Sentry check, plus a new message every time a trap fires, so one run can send several messages, possibly late. If the page has no console, it sends nothing.",
    why: "Error.stack is normally only read inside console.log when DevTools is open or a hook/automation tool formats console arguments, so this likely detects open DevTools or console instrumentation; the page-vs-iframe pair helps tell a page-level console hook from DevTools.",
    sample_payload: Some(r#"{
  "iframeConsoleStackRead": false,
  "pageConsoleStackRead": false,
  "sentryOriginalLogCalled": false
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "consoleStackFlags",
            "Three flags, other bits 0: pageConsoleStackRead (logging through the page's console read the probe's stack), sentryOriginalLogCalled (console.log.__sentry_original__ exists, is a function, and the call returned normally), iframeConsoleStackRead (logging through the new iframe's console read the probe's stack). Each bit is 1 if true, 0 if false.",
        )
        .flags(&["pageConsoleStackRead", "sentryOriginalLogCalled", "iframeConsoleStackRead"]),
    ]),
};
