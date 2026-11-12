//! Salt-free emission shapes as text: the compact form (`N R{N} C{B|N}`)
//! and top-level item splits of raw shapes (`RAW4 NUMBER REPEAT:…{…}`).
//!
//! Helps recognizers that decide by the wire layout of a slot's emissions
//! (item counts, permutations of scalar/repeat items, hoisted choice tails).
//! Used by: `ConstantEmptyString`, `DeviceMemory`, `DomElementStatistics`,
//! `FeaturePolicy`, `HistoricDateToString`, `HostEnvironment`,
//! `IncognitoStorageProbe`, `IntlLocaleTimeZone`, `JsonParseFingerprint`,
//! `KeyboardEventSamples`, `KeystrokeDynamics`, `MatchMedia`,
//! `NavigatorPlugins`, `NavigatorProperties`, `OuterArgsGlobalProcess`,
//! `PonosXhrRoundTrip`, `RememberMeCheckboxState`, `ShuffledDateString`,
//! `UiEventIsTrusted`, `UiEventSamplesMouse`, `VisibilityEventLog`,
//! `WebRtcIceCandidateAddresses`, `WorkerRealmProfile`; helper modules
//! [`render_probes`](super::render_probes).

use super::SlotView;

/// The salt-free shape in compact form: `N` number, `S` string, `B` byte,
/// `R{…}` repeat, `C{…|…}` choice, `O` opaque, `U` unresolved; the raw
/// prefix, loop kinds/directions and choice kinds are dropped.
pub(crate) fn compact(shape: &str) -> String {
    let mut out = String::new();
    for token in shape.split(' ') {
        let mut rest = token;
        loop {
            let (head, tail) = match rest.find(['{', '|', '}']) {
                Some(at) => (&rest[..at], &rest[at..]),
                None => (rest, ""),
            };
            let symbol = match head {
                "" | "RAW4" => "",
                "NUMBER" => "N",
                "STRING" => "S",
                "BYTE" | "BYTE!" => "B",
                "OPAQUE" => "O",
                "UNRESOLVED" => "U",
                other if other.starts_with("REPEAT:") => "R",
                other if other.starts_with("CHOICE:") => "C",
                _ => "?",
            };
            if !symbol.is_empty() {
                if !out.is_empty() && !out.ends_with(['{', '|']) {
                    out.push(' ');
                }
                out.push_str(symbol);
            }
            let Some(delimiter) = tail.chars().next() else { break };
            out.push(delimiter);
            rest = &tail[1..];
        }
    }
    out
}

/// Hoist a tail shared by every arm of a choice out of it: `C{B N R{N}|N N
/// R{N}}` becomes `C{B|N} N R{N}` (the contract pass can copy the rest of a
/// record into both arms of an `if`; the wire is the same). Applied at every
/// nesting level; arms keep at least one item each.
pub(crate) fn hoist_choice_tails(shape: &str) -> String {
    /// Split `items` at top-level `|` (depth 0).
    fn split_arms(body: &str) -> Vec<&str> {
        let mut arms = Vec::new();
        let mut depth = 0usize;
        let mut start = 0usize;
        for (at, character) in body.char_indices() {
            match character {
                '{' => depth += 1,
                '}' => depth = depth.saturating_sub(1),
                '|' if depth == 0 => {
                    arms.push(&body[start..at]);
                    start = at + 1;
                }
                _ => {}
            }
        }
        arms.push(&body[start..]);
        arms
    }
    let mut out: Vec<String> = Vec::new();
    for item in top_items(shape) {
        let Some(open) = item.find('{') else {
            out.push(item.to_string());
            continue;
        };
        let (head, body) = (&item[..open], &item[open + 1..item.len() - 1]);
        let arms: Vec<String> = split_arms(body).into_iter().map(hoist_choice_tails).collect();
        if head != "C" || arms.len() < 2 {
            out.push(format!("{head}{{{}}}", arms.join("|")));
            continue;
        }
        let lists: Vec<Vec<&str>> = arms.iter().map(|arm| top_items(arm)).collect();
        let shortest = lists.iter().map(Vec::len).min().unwrap_or(0);
        let mut shared = 0;
        while shared + 1 < shortest {
            let candidate = lists[0][lists[0].len() - 1 - shared];
            if !lists.iter().all(|list| list[list.len() - 1 - shared] == candidate) {
                break;
            }
            shared += 1;
        }
        let heads: Vec<String> = lists.iter().map(|list| list[..list.len() - shared].join(" ")).collect();
        out.push(format!("C{{{}}}", heads.join("|")));
        out.extend(lists[0][lists[0].len() - shared..].iter().map(|item| item.to_string()));
    }
    out.join(" ")
}

/// A do-while loop that writes from its own condition (`do { … } while (k
/// >= 0 && (write(x[k]), true))`) has a body of one logical
/// value-or-nothing choice; the nothing arm is the loop exit, so each
/// iteration writes the value: `REPEAT:DoWhile/d{CHOICE:logical{X|}}`
/// (X without nesting) reads as `REPEAT:DoWhile/d{X}`.
pub(crate) fn loop_condition_writes(shape: &str) -> String {
    const HEAD: &str = "REPEAT:DoWhile/";
    const CHOICE: &str = "{CHOICE:logical{";
    let mut out = String::new();
    let mut rest = shape;
    while let Some(at) = rest.find(HEAD) {
        let after = &rest[at + HEAD.len()..];
        let direction_end = after.find('{').unwrap_or(after.len());
        let body = &after[direction_end..];
        let simplified = body.strip_prefix(CHOICE).and_then(|inner| {
            let close = inner.find(['{', '}', '|'])?;
            (inner[close..].starts_with("|}}")).then(|| (&inner[..close], &inner[close + 3..]))
        });
        out.push_str(&rest[..at + HEAD.len() + direction_end]);
        match simplified {
            Some((value, tail)) if !value.is_empty() => {
                out.push('{');
                out.push_str(value);
                out.push('}');
                rest = tail;
            }
            _ => rest = body,
        }
    }
    out.push_str(rest);
    out
}

/// The distinct compact shapes of the slot's emissions, in order (loop
/// condition writes unwrapped, [`loop_condition_writes`]; choice tails
/// hoisted, [`hoist_choice_tails`]).
pub(crate) fn compact_shapes(view: &SlotView) -> Vec<String> {
    let mut shapes: Vec<String> = Vec::new();
    for shape in view.shapes() {
        let shape = hoist_choice_tails(&compact(&loop_condition_writes(shape)));
        if !shapes.contains(&shape) {
            shapes.push(shape);
        }
    }
    shapes
}

/// Split a compact sequence into its top-level items (`N`, `R{N}`, …).
pub(crate) fn top_items(shape: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (at, character) in shape.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
                if at > start {
                    items.push(&shape[start..at]);
                }
                start = at + 1;
            }
            _ => {}
        }
    }
    if start < shape.len() {
        items.push(&shape[start..]);
    }
    items
}

/// `shape`'s top-level items are a permutation of `items`.
pub(crate) fn permutation_of(shape: &str, items: &[&str]) -> bool {
    let mut have = top_items(shape);
    let mut want = items.to_vec();
    have.sort_unstable();
    want.sort_unstable();
    have == want
}

/// Top-level grammar items of a salt-free shape (`RAW4 NUMBER CHOICE:if{BYTE|STRING}`
/// -> `["NUMBER", "CHOICE:if{BYTE|STRING}"]`), raw prefix dropped.
pub(crate) fn shape_items(shape: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (index, c) in shape.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ' ' if depth == 0 => {
                items.push(&shape[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    items.push(&shape[start..]);
    items.retain(|item| !item.is_empty() && *item != "RAW4");
    items
}

/// Every emission has exactly `count` top-level items.
pub(crate) fn top_count_is(shapes: &[&str], count: usize) -> bool {
    !shapes.is_empty() && shapes.iter().all(|shape| shape_items(shape).len() == count)
}

/// The largest top-level item count over the emissions.
pub(crate) fn max_top_count(shapes: &[&str]) -> usize {
    shapes.iter().map(|shape| shape_items(shape).len()).max().unwrap_or(0)
}

/// Number of distinct top-level repeats across the first emission.
pub(crate) fn repeat_count(view: &SlotView) -> usize {
    view.emissions()
        .first()
        .map(|emission| emission.shape.matches("REPEAT:").count())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{compact, permutation_of, shape_items, top_items};

    #[test]
    fn loop_condition_writes() {
        use super::loop_condition_writes as unwrap;
        assert_eq!(
            unwrap("RAW4 NUMBER REPEAT:DoWhile/desc{CHOICE:logical{NUMBER|}} NUMBER NUMBER"),
            "RAW4 NUMBER REPEAT:DoWhile/desc{NUMBER} NUMBER NUMBER"
        );
        assert_eq!(
            unwrap("RAW4 NUMBER REPEAT:While/asc{NUMBER REPEAT:DoWhile/desc{CHOICE:logical{NUMBER|}}} NUMBER"),
            "RAW4 NUMBER REPEAT:While/asc{NUMBER REPEAT:DoWhile/desc{NUMBER}} NUMBER"
        );
        assert_eq!(unwrap("REPEAT:DoWhile/desc{CHOICE:if{NUMBER|}}"), "REPEAT:DoWhile/desc{CHOICE:if{NUMBER|}}");
        assert_eq!(unwrap("REPEAT:DoWhile/desc{CHOICE:logical{NUMBER|} NUMBER}"), "REPEAT:DoWhile/desc{CHOICE:logical{NUMBER|} NUMBER}");
        assert_eq!(unwrap("REPEAT:For/desc{CHOICE:logical{NUMBER|}}"), "REPEAT:For/desc{CHOICE:logical{NUMBER|}}");
    }

    #[test]
    fn hoist_choice_tails() {
        use super::hoist_choice_tails as hoist;
        assert_eq!(
            hoist("N R{N N R{N N N N} C{B N N R{N N N N}|N N N R{N N N N}}} N"),
            "N R{N N R{N N N N} C{B|N} N N R{N N N N}} N"
        );
        assert_eq!(hoist("C{B|N} N"), "C{B|N} N");
        assert_eq!(hoist("C{|B} N"), "C{|B} N");
        assert_eq!(hoist("C{B N|N}"), "C{B N|N}");
    }

    #[test]
    fn compact_shapes() {
        assert_eq!(compact("RAW4 NUMBER REPEAT:For/desc{NUMBER}"), "N R{N}");
        assert_eq!(compact("RAW4 CHOICE:cond{|BYTE} STRING BYTE! NUMBER"), "C{|B} S B N");
        assert_eq!(
            compact("RAW4 NUMBER REPEAT:While/asc{CHOICE:if{BYTE|NUMBER} NUMBER REPEAT:For/desc{NUMBER NUMBER NUMBER NUMBER}} NUMBER"),
            "N R{C{B|N} N R{N N N N}} N"
        );
        assert_eq!(top_items("N R{N R{N}} N"), vec!["N", "R{N R{N}}", "N"]);
        assert!(permutation_of("N S N", &["S", "N", "N"]));
        assert!(!permutation_of("N S", &["S", "N", "N"]));
    }

    #[test]
    fn shape_items_split_top_level() {
        assert_eq!(
            shape_items("RAW4 NUMBER REPEAT:For/desc{STRING NUMBER} CHOICE:if{BYTE|STRING}"),
            vec!["NUMBER", "REPEAT:For/desc{STRING NUMBER}", "CHOICE:if{BYTE|STRING}"]
        );
        assert_eq!(shape_items("NUMBER NUMBER"), vec!["NUMBER", "NUMBER"]);
    }
}
