//! `PonosXhrRoundTrip`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, shape_text::*, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `PonosXhrRoundTrip`. Card: [`CARD`].
pub(crate) fn recognize_ponos_xhr_round_trip(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // One response string, a digested-index list, two emissions (sent and
    // finished) and the proven telemetry request roles around the slot.
    if !per_emission_counts(view, 1, 4..=16) || repeat_count(view) != 1 || view.emissions().len() < 2 {
        return None;
    }
    let roles = ctx.slot_roles(view);
    let request = roles.contains(&"send_telemetry_request")
        && (roles.contains(&"capture_telemetry_response_text") || roles.contains(&"mark_telemetry_request_network_error"));
    let xhr = view.constructs("XMLHttpRequest") || view.reads_global("XMLHttpRequest") || request;
    if !xhr {
        return None;
    }
    let facts = slot_facts(view, ctx);
    let record = PONOS_FIELDS.iter().filter(|name| facts.any_member(name)).count() >= 4;
    (record || request).then(|| Claim::new(["telemetry XMLHttpRequest round trip", "sent and finished messages"]))
}

pub(crate) fn bind_ponos_xhr_round_trip(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // The round-trip record keeps every value under its own object key
    // (`record.xhrStatus`); the keys are the record's wire-independent
    // field names and are bound by object key.
    let mut binder = Binder::new(view, layout, ctx)?;
    // The record key nearest the written value in the contract provenance
    // (its first member transform), for a value folded through an inline
    // writer helper whose source-level argument is not the value itself.
    let provenance_key = |field: &Field| -> Option<&'static str> {
        let mut keys = field.sites.iter().flatten().filter_map(|site| {
            let leaf = view.leaves().find(|leaf| leaf.write == *site)?;
            let first = leaf.transforms.iter().find(|t| t.starts_with('.'))?;
            PONOS_FIELDS.iter().copied().find(|name| first[1..] == **name)
        });
        let key = keys.next()?;
        keys.all(|other| other == key).then_some(key)
    };
    // A value read inside an inline helper as `record[key]` with the key
    // passed as a string literal: that key (the slice of the helper call's
    // other arguments may read a neighbouring field).
    let literal_key = |field: &Field| -> Option<&'static str> {
        let mut keys = field.sites.iter().flatten().map(|site| {
            let key = ctx.helper_member_key(site)?;
            PONOS_FIELDS.iter().copied().find(|name| *name == key)
        });
        let key = keys.next()??;
        keys.all(|other| other == Some(key)).then_some(key)
    };
    binder.top(|field| {
        let desc = field.merged();
        let mut keyed: Vec<&'static str> = PONOS_FIELDS.iter().copied().filter(|name| desc.has_member(name)).collect();
        if field.shape != Shape::Array {
            if let Some(key) = literal_key(field) {
                keyed = vec![key];
            }
        }
        if keyed.len() != 1 && field.shape != Shape::Array {
            if let Some(key) = provenance_key(field) {
                keyed = vec![key];
            }
        }
        match (field.shape, keyed.as_slice()) {
            (Shape::Array, _) => Some("signalIndex"),
            (_, [one]) => Some(*one),
            (Shape::Scalar(LeafKind::Number), []) if desc.has_member("length") => Some("signalIndexCount"),
            _ => None,
        }
    })?;
    binder.finish()
}

/// Object keys of the round-trip record (card names).
const PONOS_FIELDS: &[&str] = &["cpHash", "xhrStatus", "xhrResponse", "hasNetworkError", "xhrSendTs", "xhrLoadEndTs"];

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `PonosXhrRoundTrip`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "PonosXhrRoundTrip",
    summary: "Hashes the property names of the console object, then 100 ms later sends a GET request to https://ponos.zeronaught.com carrying the site key, the transaction token, and either that console hash or a SHA-256 digest of other signals' outputs. It reports the request's round trip: status, response body, network error, and send and load-end times. A provisional message is sent right after send() and replaced by the final one when the request finishes.",
    why: "A real network round trip likely checks that the client can reach the vendor's server and ties the timing, response and signal digest to the transaction; the console hash likely detects devtools or patched console objects.",
    sample_payload: Some(r#"{
  "cpHash": -930873437,
  "hasNetworkError": false,
  "signalIndex": [
    49,
    2,
    46,
    18,
    17,
    42,
    69,
    53,
    30,
    3,
    51,
    74,
    66,
    58,
    35,
    33,
    76,
    56,
    26,
    75,
    70,
    44,
    11,
    16,
    20,
    61,
    64,
    48,
    22
  ],
  "xhrLoadEndTs": 1794273727273,
  "xhrResponse": " 7JeluLeQw1PdTB+9n6ScfzhKETP7Mv0ghQgfW5+qbdH1e3dRVQiXcnO0z4nGP1Dr8FAR4wT8Qtgkpk2MmihT0havbdU81Syl4Pv7rxFfbs4nuaog1EOPhU1/lTAet+DryF/vv/xOYMrJSZ9oBEYftahSOXmLIhJAL/zk/As3lp58f9BiklKilT4770dZhU1peb60XkIPgUPRVcRD2R9rQ6diTk/CkgDeQBv2tQLfdUz1LMuIDEyCtlxdlmWSDhXHiiMmOC1OtnjAn0jfhfk2CwkgcwpunQPfl7RGEjfFFyiLOkiyZB72bTnn29LgEFXjOkr0jr10OJMNjVTbFLgoOUli9dy/UUaxbS/XOqbEal/vb0XfR5N4aDt35jONYrEkjvm+0th2UE6pdzyi3fMOid8Ky99ExU1TM7jkfgEbDUsZUcsLMPdXUiQPdKwVa+F4ZWn+qXrzMa4lX5A9xJHFvCXZu5WGrBihWQEL8ogA9F2uYv0441sor9ef+nwo8BJWMI0cMvwakrxT0I66y9CyxSprtFt4uyrkcMGvjTS870vPA8sUQDdHKyQFKLJAwQT9RbZgKPeCuevAKsRTvO7Ptls4sPkBbV3TvuPZDvh9r+LX7uc+Wm4WzZ2bB0idVm+gz5st5xsCh0aABdNa9N/IItuVEhDU5E4AiebylGAr55qW8ndublugsmlQ7cz+ngQVduWaKWwH+WcNEToCmdfwUtU+dQoLKg+j72VREWXYuyy1hZn9TuSQRbaodQ7kiTen4RbYeYpWkHpgcKRntwl2YjqO8olQmwPN2BFAzmbWDofeGiP1CNpyce6cHWWZdAQTbCn+tRxH6F+3yynBllZwfFNqug0cRtf8GORANE4S4QvyEgdUYWRAj5G92Du6f915pRDMk1+ODuL97n/vobsong==",
  "xhrSendTs": 1794273727104,
  "xhrStatus": 200
}"#),
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "provisional, sent immediately after the request is sent",
            fields: &[
                CardField::number("cpHash", "32-bit hash of the sorted property names of console."),
                CardField::number("xhrStatus", "xhr.status right after send(), normally 0."),
                CardField::string("xhrResponse", "Response text, empty at this point."),
                CardField::number("signalIndexCount", "Always 0 here."),
                CardField::boolean(
                    "hasNetworkError",
                    "Tag value: 50 if the request's error handler fired, 94 otherwise (values per build).",
                ),
                CardField::number("xhrSendTs", "Time in ms (Date) just before send()."),
                CardField::number("xhrLoadEndTs", "-1, the request has not finished yet."),
            ],
        },
        CardMessage {
            when: "final, sent when the request finishes (loadend)",
            fields: &[
                CardField::number("cpHash", "32-bit hash of the sorted property names of console."),
                CardField::number(
                    "xhrStatus",
                    "HTTP status of the response, or the status at the time of a network error (0).",
                ),
                CardField::string("xhrResponse", "Response body text, or empty if the load did not succeed."),
                CardField::number(
                    "signalIndexCount",
                    "How many signal indices follow (signals whose outputs were digested into the request); 0 if no digest was made.",
                ),
                CardField::number(
                    "signalIndex",
                    "Index of a signal whose output was included in the digest.",
                )
                .repeat("once per digested signal, in order"),
                CardField::boolean(
                    "hasNetworkError",
                    "Tag value: 50 if the request's error handler fired, 94 otherwise (values per build).",
                ),
                CardField::number("xhrSendTs", "Time in ms (Date) just before send()."),
                CardField::number("xhrLoadEndTs", "Time in ms (Date) when the request finished or failed."),
            ],
        },
    ]),
};
