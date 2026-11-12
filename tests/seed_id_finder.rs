use serde_json::{json, Value};
use shape::signals::payload::extract::find_seed_id;

const SOURCE: &str = include_str!("fixtures/generator/seed-id-source.js");

#[test]
fn source_only_finder_proves_receiver_and_literal_spans() {
    for source in [
        SOURCE.to_owned(),
        SOURCE
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("")
            .replace("options", "a")
            .replace("descriptor", "b"),
    ] {
        let found = find_seed_id(&source).unwrap();
        assert_eq!(found.seed_id, "source-seed-id");
        assert_eq!(found.evidence.len(), 3);
        let c = &found.evidence[0];
        assert_eq!(c.role, "header_c_default");
        assert_eq!(
            &source[c.start as usize..c.end as usize],
            "\"source-seed-id\""
        );
        assert_eq!(
            c.sha256,
            digest(&source.as_bytes()[c.start as usize..c.end as usize])
        );
    }
    for source in [
        "let a={};(a).c='id';a['d']='d';a.z='z';",
        "function f(a){a.c='id';a.d='d';a.z='z';}function g(a){a.c='noise';}",
        "let a={};a.c='id';a.c='id';a.d='d';a.z='z';",
        "unknown.c='decoy';let a={};a.c='id';a.d='d';a.z='z';",
        "/* UTF-8: λ */let a={};a[('c')]='i\\u0064';a.d='d';a.z='z';",
    ] {
        assert_eq!(find_seed_id(source).unwrap().seed_id, "id");
    }
}

#[test]
fn missing_conflicting_ambiguous_and_unknown_contexts_fail_closed() {
    for source in [
        "",
        "let a={};a.c='id';a.d='d';",
        "let a={},b={};a.c='id';b.d='d';b.z='z';",
        "unknown.c='id';other.d='d';third.z='z';",
        "unknown.c='id';unknown.d='d';unknown.z='z';",
        "get().c='id';get().d='d';get().z='z';",
        "function f(){let a={};a.c='id'}function g(){let a={};a.d='d';a.z='z'}",
        "let a={};a.c='id';a.c='other';a.d='d';a.z='z';",
        "let a={};a.c='id';a.d='d';a.z='z';a.c=dynamic;",
        "let a={};a.c+='id';a.d='d';a.z='z';",
        "let a={};a.c='id';a.d='d';a.z='z';a[key]='other';",
        "let a={};a.c='id';a={};a.d='d';a.z='z';",
        "function f(a){a.c='id';a={};a.d='d';a.z='z';}",
        "let a={},b={};a.c='id';a.d='d';a.z='z';b.c='id';b.d='d';b.z='z';",
        "let a={};a.c='id';a.d=true?'d':'other';a.z='z';",
        "function f(x,y){let a={};a.c='id';a.d=x===void 0?'d':y;a.z='z';}",
        "let a={};a.c='unterminated",
    ] {
        assert!(find_seed_id(source).is_err(), "accepted {source}");
    }
}

#[test]
fn finder_and_pin_share_header_evidence() {
    use shape::signals::payload::extract::{pin, session_from_initializer};
    let source = include_str!("fixtures/generator/seed-pinned-program.js");
    let session =
        session_from_initializer(include_str!("fixtures/generator/seed-pinned-init.js")).unwrap();
    let found = find_seed_id(source).unwrap();
    let pinned = pin(source, &session).unwrap();
    assert_eq!(found.seed_id, pinned.layout.header.c);
    for evidence in found.evidence {
        assert!(pinned.evidence.contains(&evidence));
    }
}

fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Opt-in capture reproduction; all JSON comparisons retain arbitrary precision.
#[test]
fn private_capture_seed_id_and_exports() {
    let Some(root) = std::env::var_os("SHAPE_SEED_ID_CAPTURE") else {
        return;
    };
    shape::run_with_large_stack(|| {
        use shape::signals::decode::{CaptureHeaders, DecodeArtifact, DecodedPayload};
        use std::{fs, process::Command, time::Instant};
        let root = std::path::PathBuf::from(root);
        let out = root.join("seed-id-finder");
        fs::create_dir_all(&out).unwrap();
        let read = |path: &std::path::Path| -> Value {
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
        };
        let source = fs::read_to_string(root.join("program.js")).unwrap();
        let started = Instant::now();
        let found = find_seed_id(&source).unwrap();
        let finder_ms = started.elapsed().as_millis();
        assert_eq!(
            found.seed_id,
            "AIAS-MKhAQAAkyucRYRALFjH3XcQx4rjNTWvijfz1euqd-DeMASZxWvUG5OR"
        );
        let run = |args: &[&str]| {
            let started = Instant::now();
            let output = Command::new(env!("CARGO_BIN_EXE_shape"))
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            started.elapsed().as_millis()
        };
        let p = |name: &str| root.join(name).to_str().unwrap().to_owned();
        let fresh_ms = run(&[
            "decode",
            "--seed",
            &p("program.js"),
            "--input-kind",
            "program",
            "--session",
            &p("session.json"),
            "--headers",
            &p("headers.json"),
            "--float-byte-order",
            "little",
            "--save-artifact",
            &p("seed-id-finder/artifact.json"),
            "--schema-out",
            &p("seed-id-finder/schema.json"),
            "--out",
            &p("seed-id-finder/decoded.json"),
            "--force",
        ]);
        // Reload the OLD artifact: no source access and no newly required field.
        let reload_ms = run(&[
            "decode",
            "--artifact",
            &p("artifact.json"),
            "--headers",
            &p("headers.json"),
            "--schema-out",
            &p("seed-id-finder/reload-schema.json"),
            "--out",
            &p("seed-id-finder/reload-decoded.json"),
            "--force",
        ]);
        for name in ["schema.json", "decoded.json"] {
            assert_eq!(
                fs::read(out.join(name)).unwrap(),
                fs::read(out.join(format!("reload-{name}"))).unwrap()
            );
        }
        let mut artifact =
            DecodeArtifact::from_json(&fs::read(out.join("artifact.json")).unwrap()).unwrap();
        let baseline = DecodeArtifact::from_json(&fs::read(root.join("artifact.json")).unwrap()).unwrap();
        // Source provenance can grow when extraction changes. The source pin,
        // executable codecs and named projections must remain compatible,
        // alongside the fresh/reload byte comparisons above.
        assert!(artifact.layout == baseline.layout, "source/session layout changed");
        assert_eq!(json!(artifact.schema.signals), json!(baseline.schema.signals));
        let fresh_slots = &artifact.schema.wire.as_ref().unwrap().slots;
        let old_slots = &baseline.schema.wire.as_ref().unwrap().slots;
        assert_eq!(fresh_slots.len(), old_slots.len());
        for (fresh, old) in fresh_slots.iter().zip(old_slots) {
            assert_eq!(fresh.address, old.address);
            assert_eq!(fresh.routing, old.routing);
            assert_eq!(json!(fresh.programs), json!(old.programs));
            assert_eq!(json!(fresh.naming), json!(old.naming));
            assert_eq!(json!(fresh.source_plan), json!(old.source_plan));
        }
        let baseline_schema = json!(baseline.compact_schema().unwrap());
        let schema = read(&out.join("schema.json"));
        assert_eq!(schema["seedId"], found.seed_id);
        assert!(schema.get("siteId").is_none());
        assert_eq!(
            schema["encoding"]["header"]["c"],
            json!({"$ref":"#/seedId"})
        );
        let mut header = schema["encoding"]["header"].clone();
        header["c"] = schema
            .pointer(
                schema["encoding"]["header"]["c"]["$ref"]
                    .as_str()
                    .unwrap()
                    .strip_prefix('#')
                    .unwrap(),
            )
            .unwrap()
            .clone();
        assert_eq!(header, json!(artifact.layout.header));
        assert_eq!(
            schema["signals"],
            baseline_schema["signals"]
        );
        assert_eq!(schema["keysets"], json!(artifact.layout.keysets));
        assert_eq!(
            schema["reportRoutes"],
            baseline_schema["reportRoutes"]
        );
        assert_eq!(
            read(&out.join("decoded.json")),
            read(&root.join("decoded.json"))
        );
        let decoded: DecodedPayload =
            serde_json::from_slice(&fs::read(out.join("decoded.json")).unwrap()).unwrap();
        assert_eq!(decoded.metadata.decoded, 75);
        assert!(decoded.metadata.exact_reencode);
        assert_eq!(
            json!(decoded.positional_signals),
            read(&root.join("signals.json"))
        );
        fs::copy(root.join("signals.json"), out.join("signals.json")).unwrap();
        let capture: CaptureHeaders =
            serde_json::from_slice(&fs::read(root.join("headers.json")).unwrap()).unwrap();
        let expanded = capture.expanded(&artifact.layout.header.prefix).unwrap();
        let original = &decoded.metadata.original_report;
        assert_eq!(
            &artifact.layout.decode_headers(&expanded).unwrap(),
            original
        );
        assert_eq!(
            artifact
                .layout
                .generator()
                .unwrap()
                .encode_report_headers(original)
                .unwrap(),
            expanded
        );
        let joined = std::iter::once("a".to_owned())
            .chain((0..100).map(|i| format!("a{i}")))
            .filter_map(|suffix| capture.headers.get(&suffix))
            .collect::<String>();
        assert_eq!(
            artifact
                .layout
                .encode_payload(&original.channels_owned(), original.seeds)
                .unwrap(),
            joined
        );
        artifact.site_id = "external changed".into();
        artifact.layout.session = serde_json::from_value(
            json!({"alphabet":"private alphabet", "keys":[1,2,3,4,5,6,7,8],
            "transactionToken":"private token", "shuffleKey":[999], "nonce":"private nonce"}),
        )
        .unwrap();
        assert_eq!(
            schema,
            serde_json::to_value(artifact.compact_schema().unwrap()).unwrap()
        );
        let hashes: serde_json::Map<String, Value> = [
            "program.js",
            "headers.json",
            "session.json",
            "artifact.json",
            "decoded.json",
            "signals.json",
        ]
        .into_iter()
        .map(|name| {
            (
                name.into(),
                json!(digest(&fs::read(root.join(name)).unwrap())),
            )
        })
        .collect();
        let sizes: serde_json::Map<String, Value> = [
            "artifact.json",
            "decoded.json",
            "signals.json",
            "schema.json",
        ]
        .into_iter()
        .map(|name| {
            (
                name.into(),
                json!(fs::metadata(out.join(name)).unwrap().len()),
            )
        })
        .collect();
        let verification = json!({"format":"shape-seed-id-verification/1", "finder":found, "baselineSha256":hashes,
            "outputBytes":sizes, "runtimeMs":{"sourceFinder":finder_ms,"freshDebugCompileAndDecode":fresh_ms,"oldArtifactReloadAndDecode":reload_ms},
            "checks":{"seedIdExact":true,"sourceReceiverAssociated":true,"oldArtifactLoads":true,"artifactUnchanged":true,
                "artifactOnlyExportByteIdentical":true,"decodedByteIdenticalOnReload":true,"decodedValuesUnchanged":true,
                "all75PresentSignalsExact":true,"completeReportExact":true,"allHeadersByteIdentical":true,"payloadByteIdentical":true,
                "headerReferenceLossless":true,"sessionAndSiteIndependent":true,"contractsKeysetsRoutesUnchanged":true},
            "limits":["Finder parses static source only; no execution or arbitrary alias/control-flow analysis.","16 MiB source bound; use run_with_large_stack for large programs.","Float byte order is caller-selected external input.","Compact schema is descriptive; session data and executable recipes remain external."]});
        fs::write(
            out.join("verification.json"),
            serde_json::to_vec_pretty(&verification).unwrap(),
        )
        .unwrap();
    });
}
