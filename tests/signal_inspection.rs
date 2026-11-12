use serde_json::{json, Value};
use shape::signals::{
    decode::{CaptureHeaders, DecodeArtifact},
    inspect::inspect,
    payload::{superpack::Value as Raw, Headers, Report},
};

const ARTIFACT: &[u8] = include_bytes!("../sdk/c/tests/fixtures/artifact.json");
const CAPTURE: &[u8] = include_bytes!("../sdk/c/tests/fixtures/capture.json");
fn artifact() -> DecodeArtifact {
    DecodeArtifact::from_json(ARTIFACT).unwrap()
}
fn report() -> Report {
    let a = artifact();
    let c: CaptureHeaders = serde_json::from_slice(CAPTURE).unwrap();
    a.layout
        .decode_headers(&c.expanded(&a.layout.header.prefix).unwrap())
        .unwrap()
        .report()
}
fn capture(a: &DecodeArtifact, r: &Report) -> Vec<u8> {
    let headers = a
        .layout
        .encode_headers(&r.channels_owned(), r.seeds)
        .unwrap();
    serde_json::to_vec(&CaptureHeaders {
        site_id: a.site_id.clone(),
        headers: Headers::from_entries(headers.iter().map(|(k, v)| {
            (
                k.strip_prefix(&a.layout.header.prefix).unwrap().to_string(),
                v.to_string(),
            )
        }))
        .unwrap(),
    })
    .unwrap()
}
fn run(a: &DecodeArtifact, r: &Report) -> std::collections::BTreeMap<String, Value> {
    inspect(&serde_json::to_vec(a).unwrap(), &capture(a, r), None).unwrap()
}

#[test]
fn fixture_sizes_are_full_independent_golden_blobs() {
    let observed = inspect(ARTIFACT, CAPTURE, None).unwrap();
    // Golden raw blobs from the fixture's independently encoded report:
    // Four prefix bytes plus one flags byte / two integer bytes, independently
    // encoded as 0xa3 and [0x82, 0x48], not JSON or header lengths.
    assert_eq!(
        report().positional_signals[0].as_ref().unwrap().1,
        [1, 2, 3, 4, 0xa3]
    );
    assert_eq!(
        report().positional_signals[1].as_ref().unwrap().1,
        [1, 2, 3, 4, 0x82, 0x48]
    );
    assert_eq!(observed["DeviceMemory"]["data"]["deviceMemory"], 32);
    assert_eq!(observed["DeviceMemory"]["status"], "captured");
    assert_eq!(observed["DeviceMemory"]["size"]["bytes"], 6);
    assert_eq!(observed["StorageAccessApiPresence"]["size"]["bytes"], 5);
    assert_eq!(observed["ViewportDimensions"]["status"], "unavailable");
    assert_eq!(observed["SensorReadyEvent"]["status"], "unavailable");
    assert!(observed["SensorReadyEvent"]["size"]["bytes"].is_null());
    assert!(!serde_json::to_string(&observed)
        .unwrap()
        .contains("originalReport"));
}

#[test]
fn rejects_invalid_identity_capture_and_session() {
    let mut a: Value = serde_json::from_slice(ARTIFACT).unwrap();
    a["siteId"] = json!("other");
    assert!(inspect(&serde_json::to_vec(&a).unwrap(), CAPTURE, None).is_err());
    a["siteId"] = json!("example");
    a["schema"]["wire"]["source"]["sha256"] = json!("bad");
    assert!(inspect(&serde_json::to_vec(&a).unwrap(), CAPTURE, None).is_err());
    assert!(inspect(ARTIFACT, b"{}", None).is_err());
    assert!(inspect(
        ARTIFACT,
        br#"{"siteId":"example","headers":{"a":"broken"}}"#,
        None
    )
    .is_err());
    assert!(inspect(ARTIFACT, CAPTURE, Some(b"{}")).is_err());
    let mut session = serde_json::to_value(artifact().layout.session).unwrap();
    session["alphabet"] = json!("x");
    assert!(inspect(
        ARTIFACT,
        CAPTURE,
        Some(&serde_json::to_vec(&session).unwrap())
    )
    .is_err());
    assert!(inspect(
        ARTIFACT,
        br#"{"siteId":"example","siteId":"example","headers":{}}"#,
        None
    )
    .is_err());
}

#[test]
fn missing_unsupported_and_truncated_are_distinct() {
    let mut a = artifact();
    let mut r = report();
    r.positional_signals[1] = None;
    let result = run(&a, &r);
    assert_eq!(result["DeviceMemory"]["status"], "missing");
    assert!(result["DeviceMemory"]["size"]["bytes"].is_null());
    r = report();
    a.schema.wire.as_mut().unwrap().slots[1].programs[0] =
        shape::signals::wire::Support::Unsupported {
            reason: "test unsupported".into(),
            minimum_input: None,
        };
    let result = run(&a, &r);
    assert_eq!(result["DeviceMemory"]["status"], "unavailable");
    assert_eq!(result["DeviceMemory"]["size"]["bytes"], 6);
    assert_eq!(result["StorageAccessApiPresence"]["status"], "captured");
    a = artifact();
    r.positional_signals[1].as_mut().unwrap().1.truncate(3);
    assert!(inspect(&serde_json::to_vec(&a).unwrap(), &capture(&a, &r), None).is_err());
}

fn with_named(a: &DecodeArtifact, canonical: &str, name: Value, no_data: bool) -> DecodeArtifact {
    let mut v = serde_json::to_value(a).unwrap();
    let mut slot = v["schema"]["wire"]["slots"][0].clone();
    slot["address"] = json!("direct:named");
    slot["routing"] = json!({"type":"name","name":name});
    slot["contract"]["descriptor"] = json!(2);
    slot["contract"]["emissions"] = json!([]);
    slot["programs"] = json!([]);
    slot.as_object_mut().unwrap().remove("naming");
    v["schema"]["wire"]["slots"]
        .as_array_mut()
        .unwrap()
        .push(slot);
    v["schema"]["signals"][canonical] = json!({"address":"direct:named","type":if no_data {"no_data"}else{"name"},"name":name,"fields":{}});
    let channel = if name.is_string() {
        "signals"
    } else {
        "keyedSignals"
    };
    v["layout"]["report"][channel]
        .as_array_mut()
        .unwrap()
        .push(json!({"descriptor":2,"name":name}));
    DecodeArtifact::from_json(&serde_json::to_vec(&v).unwrap()).unwrap()
}
fn named(name: &str, value: Raw) -> Raw {
    Raw::Array(vec![Raw::String(name.into()), value])
}

#[test]
fn named_values_are_actual_logical_data_with_honest_shared_sizes() {
    let a = with_named(&artifact(), "ViewportDimensions", json!("vp"), false);
    let mut r = report();
    r.signals = vec![named(
        "vp",
        Raw::Object(vec![("innerWidth".into(), Raw::from(1234.))]),
    )];
    let result = run(&a, &r);
    assert_eq!(result["ViewportDimensions"]["status"], "captured");
    assert_eq!(result["ViewportDimensions"]["data"]["innerWidth"], 1234.);
    assert!(result["ViewportDimensions"]["size"]["bytes"].is_null());
    r.signals = vec![named(
        "vp",
        Raw::Object(vec![
            (
                "signal".into(),
                Raw::Object(vec![("name".into(), Raw::String("vp".into()))]),
            ),
            (
                "value".into(),
                Raw::Object(vec![("innerWidth".into(), Raw::from(1234.))]),
            ),
        ]),
    )];
    assert_eq!(
        run(&a, &r)["ViewportDimensions"]["data"]["innerWidth"],
        1234.
    );
    r.signals = vec![named(
        "vp",
        Raw::Object(vec![("innerWidth".into(), Raw::from(1234.))]),
    )];
    r.signals.push(named("vp", Raw::Null));
    let result = run(&a, &r);
    assert_eq!(
        result["ViewportDimensions"]["data"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(result["ViewportDimensions"]["note"]
        .as_str()
        .unwrap()
        .contains("2 emitted"));
    r.signals = vec![named("vp", Raw::MissingSentinel)];
    assert_eq!(run(&a, &r)["ViewportDimensions"]["status"], "missing");
    for value in [
        Raw::Null,
        Raw::String(String::new()),
        Raw::Object(vec![]),
        Raw::from(0.),
    ] {
        r.signals = vec![named("vp", value)];
        let result = run(&a, &r);
        assert_eq!(result["ViewportDimensions"]["status"], "captured");
        assert!(result["ViewportDimensions"]["size"]["bytes"].is_null());
    }
}

#[test]
fn no_data_requires_build_declaration_and_never_fabricates_zero() {
    let a = with_named(&artifact(), "SensorReadyEvent", json!("ready"), true);
    let result = run(&a, &report());
    assert_eq!(result["SensorReadyEvent"]["status"], "no_data");
    assert!(result["SensorReadyEvent"]["data"].is_null());
    assert!(result["SensorReadyEvent"]["size"]["bytes"].is_null());
}

#[test]
fn duplicate_canonical_routes_keep_both_spans() {
    let mut a = artifact();
    let wire = a.schema.wire.as_mut().unwrap();
    wire.slots[1].naming.as_mut().unwrap().canonical_name = "StorageAccessApiPresence".into();
    // Keep the valid DeviceMemory field mapping; aggregation keys follow canonical identity.
    let result = run(&a, &report());
    let entry = &result["StorageAccessApiPresence"];
    assert_eq!(entry["size"]["bytes"], 11);
    assert_eq!(entry["data"].as_array().unwrap().len(), 2);
    assert!(entry["note"].as_str().unwrap().contains("2 build routes"));
}

#[test]
fn numeric_named_values_and_empty_raw_extras_do_not_fabricate_zero() {
    let a = with_named(&artifact(), "RememberMeCheckboxState", json!(17), false);
    let mut r = report();
    r.keyed_signals = vec![(17, Some((vec![Raw::Bool(false)], vec![])))];
    let result = run(&a, &r);
    assert_eq!(result["RememberMeCheckboxState"]["status"], "captured");
    assert_eq!(
        result["RememberMeCheckboxState"]["data"]["rememberMe"],
        false
    );
    assert!(result["RememberMeCheckboxState"]["size"]["bytes"].is_null());
    let mut a = artifact();
    a.schema.wire.as_mut().unwrap().slots[1].programs[0] =
        shape::signals::wire::Support::Unsupported {
            reason: "no raw decoder".into(),
            minimum_input: None,
        };
    // An empty raw extra does not mean the generic entry emitted zero bytes.
    r.positional_signals[1] = Some((vec![Raw::Null], vec![]));
    let result = run(&a, &r);
    assert_eq!(result["DeviceMemory"]["status"], "unavailable");
    assert!(result["DeviceMemory"]["size"]["bytes"].is_null());
}

// Exercise the actual binary and backend from an isolated directory. All
// generated captures use the public fixture's codec, never illustrative data.
struct CliFixture(std::path::PathBuf);
impl CliFixture {
    fn new(a: &DecodeArtifact, r: &Report) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "shape-inspection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("artifact.json"), serde_json::to_vec(a).unwrap()).unwrap();
        std::fs::write(dir.join("capture.json"), capture(a, r)).unwrap();
        Self(dir)
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_shape"))
            .args(["show", "signals"])
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn show(&self, name: &str, json: bool) -> std::process::Output {
        let mut args = vec![
            name,
            "--artifact",
            "artifact.json",
            "--headers",
            "capture.json",
        ];
        if json {
            args.push("--json");
        }
        let out = self.cli(&args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }
    fn value(&self, name: &str) -> Value {
        serde_json::from_slice(&self.show(name, true).stdout).unwrap()
    }
}
impl Drop for CliFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn integrated_cli_captures_exact_sizes_and_real_values_in_both_formats() {
    let a = artifact();
    let mut r = report();
    let f = CliFixture::new(&a, &r);
    for (name, bytes) in [("DeviceMemory", 6), ("StorageAccessApiPresence", 5)] {
        let value = f.value(name);
        assert_eq!(value["status"], "captured");
        assert_eq!(value["provenance"], "capture");
        assert_eq!(value["size"]["bytes"], bytes);
        assert_eq!(value["size"]["kind"], "exact");
        let human = String::from_utf8(f.show(name, false).stdout).unwrap();
        assert!(human.contains(&format!("Size: {bytes} bytes")));
        assert!(human.contains("Status: captured (capture)"));
        assert!(human.contains("## Capture data"));
    }
    assert_eq!(f.value("DeviceMemory")["data"], json!({"deviceMemory":32}));
    let token = shape::signals::codec::hash_string_token(&a.layout.session.transaction_token);
    r.positional_signals[1].as_mut().unwrap().1 =
        shape::signals::codec::encode_signal(token, 0x01020304, 17, |w| w.number(9.));
    let changed = CliFixture::new(&a, &r);
    assert_eq!(
        changed.value("DeviceMemory")["data"],
        json!({"deviceMemory":9})
    );
    assert_eq!(changed.value("DeviceMemory")["size"]["bytes"], 5);
    let human = String::from_utf8(changed.show("DeviceMemory", false).stdout).unwrap();
    assert!(human.contains("\"deviceMemory\": 9"));
    assert!(!human.contains("\"deviceMemory\": 32"));
}

#[test]
fn integrated_cli_missing_absent_no_data_and_input_errors() {
    let a = with_named(&artifact(), "SensorReadyEvent", json!("ready"), true);
    let mut r = report();
    r.positional_signals[1] = None;
    let f = CliFixture::new(&a, &r);
    for (name, status) in [
        ("DeviceMemory", "missing"),
        ("ViewportDimensions", "unavailable"),
        ("SensorReadyEvent", "no_data"),
    ] {
        let value = f.value(name);
        assert_eq!(value["status"], status);
        assert!(value["data"].is_null());
        assert!(value["size"]["bytes"].is_null());
    }
    let absent = f.value("ViewportDimensions");
    assert!(absent["size"]["basis"]
        .as_str()
        .unwrap()
        .contains("no recognized route"));
    for args in [
        vec!["--artifact", "artifact.json"],
        vec!["--headers", "capture.json"],
        vec!["--session", "session.json"],
        vec!["--artifact"],
    ] {
        let out = f.cli(&args);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    for (path, contents) in [
        (
            "wrong.json",
            br#"{"siteId":"wrong","headers":{}}"#.as_slice(),
        ),
        ("invalid.json", b"{}".as_slice()),
    ] {
        std::fs::write(f.0.join(path), contents).unwrap();
    }
    for (artifact, headers) in [
        ("artifact.json", "wrong.json"),
        ("invalid.json", "capture.json"),
        ("artifact.json", "invalid.json"),
        ("missing.json", "capture.json"),
        ("artifact.json", "missing.json"),
    ] {
        let out = f.cli(&[
            "DeviceMemory",
            "--artifact",
            artifact,
            "--headers",
            headers,
            "--json",
        ]);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        assert!(!out.stderr.is_empty());
    }
}

#[test]
fn integrated_cli_generic_specials_scalar_keys_and_debug_identity() {
    let a = with_named(
        &artifact(),
        "RememberMeCheckboxState",
        json!("remember"),
        false,
    );
    let cases = [
        (Raw::Bool(false), json!(false)),
        (Raw::String(String::new()), json!("")),
        (Raw::Null, Value::Null),
        (Raw::Undefined, json!({"$special":"undefined"})),
        (Raw::from(f64::NAN), json!({"$special":"NaN"})),
        (Raw::from(f64::INFINITY), json!({"$special":"Infinity"})),
        (
            Raw::from(f64::NEG_INFINITY),
            json!({"$special":"-Infinity"}),
        ),
        (Raw::from(-0.0), json!({"$special":"-0"})),
        (Raw::Binary(vec![42]), json!({"$special":"binary"})),
    ];
    let mut r = report();
    for (raw, expected) in cases {
        r.signals = vec![named("remember", raw)];
        let f = CliFixture::new(&a, &r);
        let value = f.value("RememberMeCheckboxState");
        assert_eq!(value["status"], "captured");
        assert_eq!(value["data"], json!({"rememberMe":expected}));
        assert!(value["size"]["bytes"].is_null());
        let text = String::from_utf8(f.show("RememberMeCheckboxState", false).stdout).unwrap();
        assert!(!text.contains("numberBits"));
    }
    let a = with_named(&artifact(), "ViewportDimensions", json!("vp"), false);
    r.signals = vec![named(
        "vp",
        Raw::Object(vec![
            ("signal".into(), Raw::String("application event".into())),
            ("value".into(), Raw::Undefined),
        ]),
    )];
    let f = CliFixture::new(&a, &r);
    assert_eq!(
        f.value("ViewportDimensions")["data"],
        json!({"signal":"application event","value":{"$special":"undefined"}})
    );
    r.signals = vec![named(
        "vp",
        Raw::Object(vec![
            (
                "signal".into(),
                Raw::Object(vec![("name".into(), Raw::String("vp".into()))]),
            ),
            (
                "value".into(),
                Raw::Object(vec![("innerWidth".into(), Raw::from(1234.))]),
            ),
        ]),
    )];
    let f = CliFixture::new(&a, &r);
    assert_eq!(
        f.value("ViewportDimensions")["data"],
        json!({"innerWidth":1234.})
    );
}

#[test]
fn integrated_clean_capture_retains_policy_measurements_null_flags_and_specials() {
    use shape::signals::catalog::{card, view::catalogue_json};
    for (name, data, expected) in [
        (
            "CurrentScriptAsyncPolicyState",
            json!({"status1":130,"scriptFlags":{"async":false},"policyHash":"captured-policy","futureField":7}),
            json!({"async":false,"policyHash":"captured-policy","futureField":7}),
        ),
        (
            "MemoryBreakdownSummary",
            json!({"status":2,"entryCount":0,"entry":[],"totalBytes":42,"breakdownCount":3}),
            json!({"status":2,"entry":[],"totalBytes":42,"breakdownCount":3}),
        ),
        (
            "VmSourceIntegrity",
            json!({"buildMarker":"actual-build","bodyLength":42}),
            json!({"buildMarker":"actual-build","bodyLength":42}),
        ),
        (
            "UserAgentData",
            json!({"flags":{"mobile":null,"wow64":false}}),
            json!({"flags":{"mobile":null,"wow64":false}}),
        ),
        (
            "OfflineAudioRenderSamples",
            json!({"sample":[{"numberBits":9221120237041090560_u64,"special":"nan"},{"$special":"undefined"},null]}),
            json!({"sample":[{"$special":"NaN"},{"$special":"undefined"},null]}),
        ),
    ] {
        let observation = json!({"status":"captured","data":data,"size":{"bytes":null,"kind":"unavailable","basis":"test observation"}});
        let result = catalogue_json(card(name).unwrap(), Some(&observation), true);
        assert_eq!(result["data"], expected, "{name}");
        assert!(!serde_json::to_string(&result["data"])
            .unwrap()
            .contains("numberBits"));
    }
}
