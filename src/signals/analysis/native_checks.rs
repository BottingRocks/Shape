//! Native-code and tamper checks: which native-function census a counted
//! number is, which console stack probe, proxy probe or native-integrity
//! check a flag slot is. Each close pair is decided by one pure function
//! over transforms, API paths or [`Facts`] (`native_count_kind`,
//! `console_realm`, `proxy_kind`, `integrity_kind`) with explicit tie tests.
//!
//! Used by: `BrowserApiProxyFlags`, `CleanConsoleStackAccessFlags`,
//! `ConsoleStackAccessFlags`, `FingerprintApiIntegrity`,
//! `GlobalNativeFunctionCount`, `IframeRealmNativeFunctionCount`,
//! `NativeApiIntegrityFlags`, `NonNativeBuiltinCount`, `ProxyTamperFlags`.

use super::buffer_writes::{slot_info, Facts};
use super::leaf_checks::{plain_ops, single_number};
use super::{FinderCtx, LeafOp, SlotView};

/// Which native-code census a single counted number is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeCount {
    /// Non-native entries of a fixed built-in list: `list.filter(!native).length`.
    NonNativeBuiltins,
    /// Native non-enumerable data properties of the page global.
    GlobalRealm,
    /// The same census over a fresh nested-iframe realm.
    IframeRealm,
}

/// The native-code census a counted number's transforms and API paths describe.
pub(crate) fn native_count_kind(transforms: &[String], apis: &[String]) -> Option<NativeCount> {
    let has = |t: &str| transforms.iter().any(|x| x == t);
    let builtin_list = ["Function.prototype.bind", "Function.prototype.call", "Function.prototype.apply"]
        .iter()
        .filter(|api| apis.iter().any(|a| a == **api))
        .count();
    let filtered = has(".filter()") && has(".length") && builtin_list >= 2;
    // A counted native-code test over own property names.
    let census = has("count") && has(".test()") && has(".value");
    let realm = has(".styleNonce") || has("context.csp");
    let global = has("Object.getOwnPropertyNames") && has(".enumerable");
    let nested = has(".getOwnPropertyNames()") && has(".getOwnPropertyDescriptor()");
    match (filtered, census && global && !realm, census && nested && realm) {
        (true, false, false) => Some(NativeCount::NonNativeBuiltins),
        (false, true, false) => Some(NativeCount::GlobalRealm),
        (false, false, true) => Some(NativeCount::IframeRealm),
        _ => None,
    }
}

/// [`native_count_kind`] of a slot writing one plain number.
pub(crate) fn native_count(view: &SlotView) -> Option<NativeCount> {
    let leaf = single_number(view)?;
    native_count_kind(&leaf.transforms, &leaf.apis)
}

/// Which console stack probe a store context set belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConsoleRealm {
    /// The probe goes through a directly created iframe's `contentWindow.console`.
    DirectIframe,
    /// The probe goes through the console of a realm handed to a callback.
    CleanRealm,
}

/// `stores`: the contexts of the flag stores of one slot.
pub(crate) fn console_realm(stores: &[Facts]) -> Option<ConsoleRealm> {
    let probe = |facts: &Facts| facts.string("stack") && (facts.call("defineProperty") || facts.member("defineProperty"));
    let direct = stores
        .iter()
        .any(|facts| probe(facts) && facts.member("contentWindow") && facts.member("console"));
    let clean = stores
        .iter()
        .any(|facts| probe(facts) && facts.param_console && !facts.member("contentWindow"));
    match (direct, clean) {
        (true, false) => Some(ConsoleRealm::DirectIframe),
        (false, true) => Some(ConsoleRealm::CleanRealm),
        _ => None,
    }
}

/// The console probe of a one-byte slot whose value reads a flag array.
pub(crate) fn console_probe(view: &SlotView, ctx: &FinderCtx) -> Option<ConsoleRealm> {
    if !plain_ops(view, &[LeafOp::Byte]) {
        return None;
    }
    let info = slot_info(view, ctx);
    console_realm(&info.stores)
}

/// Which proxy probe a flag-byte slot is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProxyKind {
    /// 17 prototype-cycle detectors plus `typeof Proxy`, three flag bytes.
    ApiDetectors,
    /// `Date.prototype.toString` / `structuredClone` brand errors, one byte.
    BrandErrors,
}

/// `bytes`: top-level flag bytes per message; `slice`: write slices;
/// `stores`: flag-store contexts.
pub(crate) fn proxy_kind(bytes: &[usize], slice: &Facts, stores: &Facts) -> Option<ProxyKind> {
    const DETECTED: &[&str] = &[
        "canPlayType",
        "speechSynthesis",
        "languages",
        "hardwareConcurrency",
        "plugins",
        "toDataURL",
        "mimeTypes",
        "getBattery",
        "navigator",
        "toString",
        "Intl",
        "supportedValuesOf",
        "createElement",
        "getParameter",
        "getImageData",
    ];
    let detectors = slice.string("object")
        && slice.string("function")
        && (slice.count_members(DETECTED) + DETECTED.iter().filter(|g| slice.global(g)).count()) >= 3;
    let brand = stores.string("Proxy.toString") || stores.string("incompatible Proxy");
    let three = !bytes.is_empty() && bytes.iter().all(|count| *count == 3);
    let one = !bytes.is_empty() && bytes.iter().all(|count| *count == 1);
    match (detectors && three && !brand, brand && one) {
        (true, false) => Some(ProxyKind::ApiDetectors),
        (false, true) => Some(ProxyKind::BrandErrors),
        _ => None,
    }
}

/// Which native-integrity check a flag slot is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntegrityKind {
    /// 20 checks in three bytes (init + last chance).
    ThreeBytes,
    /// 72 checks in eleven bytes plus fifteen source-hash slots.
    WithSourceHashes,
}

/// Host API names the native-integrity checks test.
pub(crate) const INTEGRITY_APIS: &[&str] = &[
    "createElement",
    "createElementNS",
    "toDataURL",
    "toBlob",
    "getContext",
    "fillText",
    "getImageData",
    "userAgent",
    "plugins",
    "mimeTypes",
    "strokeStyle",
    "fillStyle",
    "font",
];

/// `fields`: top-level field count per message; `hashes`: hash slots seen.
pub(crate) fn integrity_kind(fields: &[usize], hashes: usize, slice: &Facts) -> Option<IntegrityKind> {
    let integrity = slice.string("[object Object]") && slice.count_strings(INTEGRITY_APIS) >= 4;
    if !integrity || fields.is_empty() {
        return None;
    }
    if fields.iter().all(|count| *count == 3) && hashes == 0 {
        return Some(IntegrityKind::ThreeBytes);
    }
    if fields.iter().all(|count| *count >= 20) && hashes > 0 {
        return Some(IntegrityKind::WithSourceHashes);
    }
    // Builds without the source hashes send the eleven flag bytes only.
    if fields.iter().all(|count| *count == 11) && hashes == 0 {
        return Some(IntegrityKind::WithSourceHashes);
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{console_realm, integrity_kind, native_count_kind, proxy_kind, ConsoleRealm, Facts, IntegrityKind, NativeCount, ProxyKind};

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn facts(members: &[&str], strings_: &[&str], calls: &[&str], globals: &[&str], param_console: bool) -> Facts {
        Facts {
            members: members.iter().map(|s| s.to_string()).collect(),
            strings: strings_.iter().map(|s| s.to_string()).collect(),
            regex: BTreeSet::new(),
            globals: globals.iter().map(|s| s.to_string()).collect(),
            calls: calls.iter().map(|s| s.to_string()).collect(),
            param_console,
            ..Facts::default()
        }
    }

    /// GlobalNativeFunctionCount vs IframeRealmNativeFunctionCount vs
    /// NonNativeBuiltinCount: one counted number each.
    #[test]
    fn tie_native_counts() {
        let builtins = strings(&["Function.prototype.bind", "Function.prototype.call", "JSON"]);
        let filtered = strings(&[".length", ".filter()", "array", ".push", "select", "===", "typeof"]);
        let global = strings(&[
            "count", "guard", ".value", ".test()", ".enumerable", "Object.getOwnPropertyNames", "context.global",
        ]);
        let iframe = strings(&[
            "count", "guard", ".value", ".test()", ".getOwnPropertyDescriptor()", ".getOwnPropertyNames()",
            ".styleNonce", "context.csp",
        ]);
        assert_eq!(native_count_kind(&filtered, &builtins), Some(NativeCount::NonNativeBuiltins));
        assert_eq!(native_count_kind(&global, &[]), Some(NativeCount::GlobalRealm));
        assert_eq!(native_count_kind(&iframe, &[]), Some(NativeCount::IframeRealm));
        // The realm helper's nonce turns a global census into no decision.
        let mut mixed = global.clone();
        mixed.push(".styleNonce".into());
        assert_eq!(native_count_kind(&mixed, &[]), None);
        // A filter without the captured built-in list is not the built-in count.
        assert_eq!(native_count_kind(&filtered, &[]), None);
        // Census and filter evidence together: ambiguous, no claim.
        let mut both = filtered.clone();
        both.extend(global.clone());
        assert_eq!(native_count_kind(&both, &builtins), None);
    }

    /// ConsoleStackAccessFlags vs CleanConsoleStackAccessFlags.
    #[test]
    fn tie_console_realms() {
        let local = facts(&["log", "name"], &["stack"], &["defineProperty"], &[], false);
        let sentry = facts(&["log", "__sentry_original__"], &[], &[], &[], false);
        let direct = facts(&["contentWindow", "console", "log"], &["stack"], &["defineProperty"], &[], false);
        let clean = facts(&["console", "log"], &["stack"], &["defineProperty"], &[], true);
        assert_eq!(
            console_realm(&[local.clone(), sentry.clone(), direct.clone()]),
            Some(ConsoleRealm::DirectIframe)
        );
        assert_eq!(
            console_realm(&[local.clone(), sentry.clone(), clean.clone()]),
            Some(ConsoleRealm::CleanRealm)
        );
        // Both probes in one slot's stores: equal evidence, no claim.
        assert_eq!(console_realm(&[local.clone(), direct.clone(), clean.clone()]), None);
        // The page-console and Sentry probes alone prove neither.
        assert_eq!(console_realm(&[local, sentry]), None);
        // A realm console without the stack trap is not a probe.
        let no_trap = facts(&["console", "log"], &[], &[], &[], true);
        assert_eq!(console_realm(&[no_trap]), None);
    }

    /// BrowserApiProxyFlags vs ProxyTamperFlags.
    #[test]
    fn tie_proxy_kinds() {
        let detectors = facts(
            &["speechSynthesis", "getBattery", "mimeTypes", "plugins"],
            &["object", "function"],
            &[],
            &["Proxy", "navigator"],
            false,
        );
        let brand = facts(&["stack", "message", "indexOf"], &["Proxy.toString", "incompatible Proxy"], &[], &[], false);
        let none = Facts::default();
        assert_eq!(proxy_kind(&[3], &detectors, &none), Some(ProxyKind::ApiDetectors));
        assert_eq!(proxy_kind(&[3, 3], &detectors, &none), Some(ProxyKind::ApiDetectors));
        assert_eq!(proxy_kind(&[1], &none, &brand), Some(ProxyKind::BrandErrors));
        // Byte count decides nothing on its own.
        assert_eq!(proxy_kind(&[1], &detectors, &none), None);
        assert_eq!(proxy_kind(&[3], &none, &brand), None);
        // Both kinds of evidence on three bytes: no claim.
        assert_eq!(proxy_kind(&[3], &detectors, &brand), None);
    }

    /// NativeApiIntegrityFlags vs FingerprintApiIntegrity.
    #[test]
    fn tie_integrity_kinds() {
        let slice = facts(
            &["get", "set"],
            &["[object Object]", "createElement", "userAgent", "plugins", "mimeTypes", "toDataURL"],
            &[],
            &[],
            false,
        );
        assert_eq!(integrity_kind(&[3, 3], 0, &slice), Some(IntegrityKind::ThreeBytes));
        assert_eq!(integrity_kind(&[26, 26], 15, &slice), Some(IntegrityKind::WithSourceHashes));
        // Three bytes with hash slots, or many fields without: neither.
        // Builds without source hashes: eleven flag bytes.
        assert_eq!(integrity_kind(&[11, 11], 0, &slice), Some(IntegrityKind::WithSourceHashes));
        assert_eq!(integrity_kind(&[3], 2, &slice), None);
        assert_eq!(integrity_kind(&[26], 0, &slice), None);
        assert_eq!(integrity_kind(&[3, 11], 0, &slice), None);
        // No class-tag / API evidence: neither.
        assert_eq!(integrity_kind(&[3], 0, &Facts::default()), None);
    }
}
