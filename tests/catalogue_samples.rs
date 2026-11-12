//! Captured documentary values, independent of runtime codecs and schemas.
use serde_json::{json, Value};
use shape::signals::{catalog, docs};
use std::process::{Command, Stdio};

fn sample(name: &str) -> Value {
    serde_json::from_str(catalog::card(name).unwrap().sample_payload.unwrap()).unwrap()
}

#[test]
fn registered_samples_are_valid_json() {
    for card in catalog::cards() {
        if let Some(payload) = card.sample_payload {
            serde_json::from_str::<Value>(payload).expect(card.signal);
            assert!(!card.no_data(), "{} emits no payload", card.signal);
        }
    }
}

/// Set SHAPE_CATALOG_SAMPLES_FIXTURE to the private approved signals map.
/// The capture stays outside the repository's public fixtures.
#[test]
fn captured_samples_match_optional_private_fixture_exactly() {
    let Some(path) = std::env::var_os("SHAPE_CATALOG_SAMPLES_FIXTURE") else {
        return;
    };
    let input: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let input = input.as_object().expect("canonical signal-name map");
    for name in input.keys() {
        assert!(catalog::card(name).is_some(), "unregistered input {name}");
    }
    let mut captured = 0;
    for card in catalog::cards() {
        match input.get(card.signal) {
            Some(expected) => {
                assert_eq!(&sample(card.signal), expected, "{}", card.signal);
                captured += 1;
            }
            None => assert!(card.sample_payload.is_none(), "{} uncaptured", card.signal),
        }
    }
    assert_eq!(captured, input.len());
}

#[test]
fn captured_shapes_and_values_are_preserved() {
    assert_eq!(sample("DeviceMemory"), json!({"deviceMemory": 32}));
    let media = sample("MatchMedia");
    assert_eq!(
        media
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, v)| k.ends_with("Bits") && **v == json!({}))
            .count(),
        20
    );
    let hooks = sample("GrecaptchaAccessorHookFlags");
    let hooks = hooks["hookChecks"].as_array().unwrap();
    assert_eq!(hooks.len(), 2);
    assert_eq!(hooks[0].as_object().unwrap().len(), 7);
    assert_eq!(hooks[1].as_object().unwrap().len(), 4);
    assert_eq!(hooks[0]["compressionStream"], true);
    assert_eq!(hooks[0]["descriptorPresent"], false);
    assert_eq!(sample("ShuffledDateString")["zero"], 0);
    assert_eq!(sample("MemoryBreakdownSummary")["status"], 3);
    assert!(sample("MemoryBreakdownSummary")["entriesPlaceholder"].is_null());
    assert!(sample("FontAvailability")["fontResults"].is_array());
    assert!(sample("FontAvailability").get("fontCount").is_none());
    assert!(sample("KeyboardEventSamples").get("recordCount").is_none());
    assert_eq!(
        sample("ParseIntHexConstant")["parsedValue"].as_u64(),
        Some(18_446_744_073_709_549_568)
    );
}

#[test]
fn uncaptured_no_payload_and_real_values_remain_distinct() {
    let uncaptured = catalog::card("AnchorIdnaUnicodeVersion").unwrap();
    assert!(uncaptured.sample_payload.is_none());
    assert!(!uncaptured.no_data());
    assert!(docs::render_catalogue(uncaptured).contains("No decoded sample available"));
    let no_payload = catalog::card("SensorReadyEvent").unwrap();
    assert!(no_payload.sample_payload.is_none());
    assert!(no_payload.no_data());
    assert!(docs::render_catalogue(no_payload).contains("No payload emitted"));
    for card in [uncaptured, no_payload] {
        let exported = docs::catalogue_json(card);
        assert_eq!(exported.get("sample_payload"), Some(&Value::Null));
        assert_eq!(exported["no_data"], card.no_data());
    }
    // Logical nulls and empty objects/arrays inside captured payloads survive.
    assert_eq!(sample("UiEventSamplesTouch")["records"], json!([]));
    let exported = docs::catalogue_json(catalog::card("NavigatorProperties").unwrap());
    assert!(exported["sample_payload"]["buildID"].is_null());
    assert!(sample("MatchMedia")["anyHoverBits"]
        .as_object()
        .unwrap()
        .is_empty());
}

#[test]
fn cli_preserves_integer_precision_and_absence_metadata() {
    let out = cli(&["show", "signals", "ParseIntHexConstant", "--json"]);
    assert!(out.status.success());
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        value["sample_payload"]["parsedValue"].as_u64(),
        Some(18_446_744_073_709_549_568)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("18446744073709549568"));
    for (name, no_data, message) in [
        (
            "AnchorIdnaUnicodeVersion",
            false,
            "illustrative",
        ),
        ("SensorReadyEvent", true, "no payload emitted"),
    ] {
        let text = cli(&["show", "signals", name]);
        assert!(text.status.success());
        assert!(String::from_utf8_lossy(&text.stdout).contains(message));
        let out = cli(&["show", "signals", name, "--json"]);
        assert!(out.status.success());
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value.get("sample_payload"), Some(&Value::Null));
        assert_eq!(value["no_data"], no_data);
    }
}

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_shape"))
        .args(args)
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap()
}
#[test]
fn catalogue_cli_all_single_json_help_and_usage_errors() {
    let all = cli(&["show", "signals", "--json"]);
    assert!(all.status.success());
    let cards: Value = serde_json::from_slice(&all.stdout).unwrap();
    let cards = cards.as_array().unwrap();
    assert_eq!(cards.len(), catalog::cards().count());
    assert!(cards
        .windows(2)
        .all(|w| w[0]["signal"].as_str() < w[1]["signal"].as_str()));
    let one = cli(&["show", "signals", "DeviceMemory", "--json"]);
    assert!(one.status.success());
    let one: Value = serde_json::from_slice(&one.stdout).unwrap();
    assert_eq!(one["sample_payload"]["deviceMemory"], 32);
    let text = cli(&["show", "signals", "UserAgentData"]);
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("## Why"));
    assert!(text.contains("Logical fields"));
    assert!(!text.contains("wow64Placeholder"));
    assert!(text.contains("boolean | null"));
    assert!(text.contains("```json"));
    let all = cli(&["show", "signals"]);
    assert!(all.status.success());
    assert!(all.stdout.len() > 10000);
    for args in [
        vec!["show", "--help"],
        vec!["show", "signals", "--help"],
        vec!["--help"],
    ] {
        let output = cli(&args);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("show signals"));
    }
    for args in [
        vec!["show"],
        vec!["show", "wat"],
        vec!["show", "signals", "devicememory"],
        vec!["show", "signals", "DeviceMemory", "extra"],
        vec!["show", "signals", "--wat"],
        vec!["show", "signals", "--artifact", "a"],
        vec!["show", "signals", "--headers", "h"],
        vec!["show", "signals", "--session", "s"],
        vec!["show", "signals", "--artifact"],
        vec!["show", "signals", "--artifact", "a", "--artifact", "b"],
        vec!["show", "signals", "--json", "--json"],
    ] {
        let out = cli(&args);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        assert!(!out.stderr.is_empty());
    }
    let failed = cli(&["show", "signals", "--artifact", "/nonexistent/artifact", "--headers", "/nonexistent/headers"]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(failed.stdout.is_empty());
    let old = cli(&["signals", "show", "--help"]);
    assert!(old.status.success());
    assert!(String::from_utf8_lossy(&old.stdout).contains("program.js"));
}
#[test]
fn catalogue_cli_closed_stdout_is_success() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shape"))
        .args(["show", "signals", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}
