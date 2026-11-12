//! `IntlLocaleTimeZone`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, SignalLayout, SlotView, ValueSlice};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `IntlLocaleTimeZone`. Card: [`CARD`].
pub(crate) fn recognize_intl_locale_time_zone(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = first_emission_leaves(view);
    let supported = leaves
        .iter()
        .filter(|leaf| leaf.repeat_depth == 0 && has_api(leaf, "Intl.supportedValuesOf()"))
        .count();
    let offset = any_transform(&leaves, ".getTimezoneOffset()");
    let locale = leaves.iter().any(|leaf| {
        has_transform(leaf, ".resolvedOptions()") && leaf.apis.iter().any(|api| api.starts_with("new Intl."))
    });
    if supported >= 3 && offset && locale && max_top_count(&view.shapes()) >= 10 {
        return claim(&["3+ Intl.supportedValuesOf counts", "Intl resolvedOptions locales", "getTimezoneOffset"]);
    }
    None
}

pub(crate) fn bind_intl_locale_time_zone(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // The six `Intl.supportedValuesOf(<kind>).length` counts differ only in
    // their string argument and the four locale strings in the constructed
    // `Intl` service; both are read from the value at each field's own write
    // site (leaf provenance shares registers with companion slots).
    const KINDS: &[&str] = &["calendar", "collation", "currency", "numberingSystem", "timeZone", "unit"];
    const SERVICES: &[&str] = &["Segmenter", "DisplayNames", "DateTimeFormat", "Collator"];
    fn kinds(slice: &ValueSlice) -> Vec<&str> {
        let mut out: Vec<&str> = slice
            .string_args("supportedValuesOf")
            .into_iter()
            .filter(|kind| KINDS.contains(kind))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
    fn services(slice: &ValueSlice) -> Vec<&'static str> {
        SERVICES.iter().copied().filter(|service| slice.constructs(service)).collect()
    }
    let count = |kind: &'static str| {
        move |field: &Field, slice: &ValueSlice| field.writes(LeafKind::Number) && kinds(slice) == [kind]
    };
    let locale = |service: &'static str| {
        move |field: &Field, slice: &ValueSlice| {
            field.writes(LeafKind::String) && kinds(slice).is_empty() && services(slice) == [service]
        }
    };
    let calendar = count("calendar");
    let collation = count("collation");
    let currency = count("currency");
    let numbering = count("numberingSystem");
    let time_zones = count("timeZone");
    let unit = count("unit");
    let segmenter = locale("Segmenter");
    let display_names = locale("DisplayNames");
    let date_time_format = locale("DateTimeFormat");
    let collator = locale("Collator");
    let time_zone_hash = |field: &Field, slice: &ValueSlice| {
        field.writes(LeafKind::Number)
            && kinds(slice).is_empty()
            && slice.members.contains_key("timeZone")
            && services(slice) == ["DateTimeFormat"]
    };
    let offset = |field: &Field, slice: &ValueSlice| {
        field.writes(LeafKind::Number) && kinds(slice).is_empty() && slice.calls_method("getTimezoneOffset")
    };
    // Five hops: fresh traces reach a locale through reused registers
    // (`a = k; k = r; r = "" + o.locale; o = new Intl.X(…)`).
    solve_by_slices(
        view,
        layout,
        ctx,
        5,
        &[
            ("calendarCount", &calendar),
            ("collationCount", &collation),
            ("currencyCount", &currency),
            ("numberingSystemCount", &numbering),
            ("timeZoneCount", &time_zones),
            ("unitCount", &unit),
            ("segmenterLocale", &segmenter),
            ("displayNamesLocale", &display_names),
            ("dateTimeFormatLocale", &date_time_format),
            ("collatorLocale", &collator),
            ("timeZoneHash", &time_zone_hash),
            ("timezoneOffsetMin", &offset),
        ],
    )
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `IntlLocaleTimeZone`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "IntlLocaleTimeZone",
    summary: "Reads the browser's internationalization (Intl) setup. It asks four Intl services (DateTimeFormat, Collator, DisplayNames, Segmenter) which default locale they use, hashes the time-zone name, reads the UTC offset, and counts how many calendars, collations, currencies, numbering systems, time zones and units the engine supports.",
    why: "Locale and time zone can be checked against the claimed IP location and the Accept-Language header. The supported-value counts identify the browser's ICU/CLDR data version, which exposes spoofed or mismatched browser builds (likely).",
    sample_payload: Some(r#"{
  "calendarCount": 18,
  "collationCount": 10,
  "collatorLocale": "en-US",
  "currencyCount": 159,
  "dateTimeFormatLocale": "en-US",
  "displayNamesLocale": "en-US",
  "numberingSystemCount": 78,
  "segmenterLocale": "en-US",
  "timeZoneCount": 418,
  "timeZoneHash": 146801107,
  "timezoneOffsetMin": 480,
  "unitCount": 45
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "calendarCount",
            "Number of calendars from Intl.supportedValuesOf(\"calendar\"), or -1 if unsupported.",
        ),
        CardField::string(
            "segmenterLocale",
            "Default locale of Intl.Segmenter, e.g. \"en-US\", or a null marker if Intl.Segmenter is missing or throws.",
        ).nullable(),
        CardField::string(
            "displayNamesLocale",
            "Default locale of Intl.DisplayNames, or a null marker if it is missing or throws.",
        ).nullable(),
        CardField::number(
            "unitCount",
            "Number of units from Intl.supportedValuesOf(\"unit\"), or -1 if unsupported.",
        ),
        CardField::string(
            "dateTimeFormatLocale",
            "Default locale of Intl.DateTimeFormat, or a null marker if it throws.",
        ).nullable(),
        CardField::number(
            "timeZoneHash",
            "32-bit hash of the time-zone name (e.g. \"Europe/Berlin\"), or -1 if it cannot be read.",
        ),
        CardField::number(
            "timeZoneCount",
            "Number of time zones from Intl.supportedValuesOf(\"timeZone\"), or -1 if unsupported.",
        ),
        CardField::string(
            "collatorLocale",
            "Default locale of Intl.Collator, or a null marker if it throws.",
        ).nullable(),
        CardField::number(
            "timezoneOffsetMin",
            "new Date().getTimezoneOffset(): minutes between local time and UTC (positive west of UTC, e.g. 300 for US Eastern winter time, -60 for Central Europe).",
        ),
        CardField::number(
            "currencyCount",
            "Number of currencies from Intl.supportedValuesOf(\"currency\"), or -1 if unsupported.",
        ),
        CardField::number(
            "numberingSystemCount",
            "Number of numbering systems from Intl.supportedValuesOf(\"numberingSystem\"), or -1 if unsupported.",
        ),
        CardField::number(
            "collationCount",
            "Number of collations from Intl.supportedValuesOf(\"collation\"), or -1 if unsupported.",
        ),
    ]),
};
