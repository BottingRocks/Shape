//! Decisions over a [`Census`]: whether a slot probes a host name, which
//! single-flag-byte presence probe owns a shared temporary, anchor-key walks,
//! the five-entry namespace path table, and binding written values by the
//! path-table entry or call argument they index.
//!
//! Used by: `AnchorProperties`, `BraveDuckDuckGoNavigatorFlags`,
//! `ConsoleMemberNativenessHashPair`, `ConsolePropertyNamesHash`,
//! `CrossOriginIsolationFlags`, `GlobalPropertiesCount`,
//! `GlobalPropertyChecks`, `IframeRealmPropertyNameHashes`,
//! `MathOwnPropertyNames`, `NamespacePropertyNameHashes`,
//! `NestedFrameAnchorPropertyNamesHash`, `SelectMetaInjectionFlags`,
//! `StorageAccessApiPresence`, `WebAuthnInterfacePresence`.

use super::leaf_checks::value_members;
use super::property_census::{census, CallArgs, Census};
use super::{bind_in_order, BindRefusal, FieldBinding, FinderCtx, SignalLayout, SlotView};

/// The census or the value provenance names `name`.
pub(crate) fn probes(view: &SlotView, census: &Census, name: &str) -> bool {
    census.names(name)
        || value_members(view).contains(name)
        || view.value_apis().iter().any(|api| api.rsplit('.').next() == Some(name))
}

/// The census or value provenance names every one of `names`.
pub(crate) fn probes_all(view: &SlotView, census: &Census, names: &[&str]) -> bool {
    names.iter().all(|name| probes(view, census, name))
}

/// The probe names of the single-flag-byte presence signals.
pub(crate) const FLAG_PROBES: &[(&str, &[&str])] = &[
    ("StorageAccessApiPresence", &["hasStorageAccess", "requestStorageAccess"]),
    ("WebAuthnInterfacePresence", &["PublicKeyCredential", "AuthenticatorAttestationResponse"]),
    ("BraveDuckDuckGoNavigatorFlags", &["brave", "duckduckgo"]),
    ("CrossOriginIsolationFlags", &["crossOriginIsolated", "SharedArrayBuffer"]),
];

/// Nearest evidence wins between flag-byte probes: when the statements that
/// write this slot's buffer name none of `signal`'s probes but do name
/// another flag probe's, a shared temporary carried `signal`'s names in from
/// a neighbouring slot, so `signal` declines.
pub(crate) fn outranked(census: &Census, signal: &str) -> bool {
    let named = |names: &[&str]| names.iter().any(|name| census.direct.contains(*name));
    let own = FLAG_PROBES.iter().find(|(name, _)| *name == signal).map(|(_, names)| *names).unwrap_or_default();
    !named(own) && FLAG_PROBES.iter().any(|(name, names)| *name != signal && named(names))
}

/// An anchor element (`createElement("a")`) whose keys a `for…in` walks.
pub(crate) fn anchor_key_walk(census: &Census) -> bool {
    census.calls_with("createElement", "a") && census.for_in && !census.calls("getComputedStyle")
}

/// The first segment of the `Navigator` path of the namespace table.
pub(crate) const PATHS_NAVIGATOR: &str = "Navigator";

/// The first segment of the `safari` path of the namespace table.
pub(crate) const PATHS_SAFARI: &str = "safari";

/// The census path table of the five namespace walks.
pub(crate) fn namespace_table(census: &Census) -> Option<&Vec<Vec<String>>> {
    census.path_tables.iter().find(|table| {
        table.len() == 5
            && table.iter().any(|path| path.first().map(String::as_str) == Some(PATHS_NAVIGATOR))
            && table.iter().any(|path| path.first().map(String::as_str) == Some(PATHS_SAFARI))
    })
}

/// The card names of each emission's value writes (`name` maps one write,
/// `None` skips it); every emission must agree.
pub(crate) fn agreed_names(
    census: &Census,
    name: impl Fn(&CallArgs) -> Result<Option<&'static str>, BindRefusal>,
) -> Result<Vec<&'static str>, BindRefusal> {
    let mut agreed: Option<Vec<&'static str>> = None;
    for group in &census.write_groups {
        let mut names = Vec::new();
        for args in group {
            if let Some(found) = name(args)? {
                names.push(found);
            }
        }
        if names.is_empty() {
            continue;
        }
        match &agreed {
            None => agreed = Some(names),
            Some(first) if *first == names => {}
            Some(_) => return Err(BindRefusal::new("emissions write the values in different orders")),
        }
    }
    agreed.ok_or_else(|| BindRefusal::new("no value writes found for this slot"))
}

/// Name five path hashes by the path-table entry each written value indexes.
pub(crate) fn bind_by_path(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
    names: &[(&[&str], &'static str)],
) -> Result<FieldBinding, BindRefusal> {
    let census = census(view, ctx);
    let table = namespace_table(&census).ok_or_else(|| BindRefusal::new("no namespace path table"))?;
    let out = agreed_names(&census, |args| {
        let [index] = args.indices.as_slice() else {
            return Ok(None);
        };
        let path = table
            .get(*index as usize)
            .ok_or_else(|| BindRefusal::new("written value indexes past the path table"))?;
        names
            .iter()
            .find(|(want, _)| want.iter().copied().eq(path.iter().map(String::as_str)))
            .map(|(_, name)| Some(*name))
            .ok_or_else(|| BindRefusal::new(format!("path {path:?} has no card name")))
    })?;
    bind_in_order(layout, &out)
}
