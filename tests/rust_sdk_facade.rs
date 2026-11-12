//! Named Rust SDK contracts. All inputs are checked-in synthetic fixtures.
use serde::Serialize;
use serde_json::{json, Value};
use shape::sdk;
use std::sync::OnceLock;

const PROGRAM: &str = include_str!("fixtures/sdk/decode/program.js");
const RAW: &str = include_str!("fixtures/javascript-vm-prefix.raw.js");
const FULL: sdk::DecodeOptions = sdk::DecodeOptions {
    verify: true,
    metadata: true,
};
fn value(input: impl Serialize) -> Value {
    serde_json::to_value(input).unwrap()
}
fn without_timings(mut input: Value) -> Value {
    match &mut input {
        Value::Object(map) => {
            map.remove("timings_ms");
            for child in map.values_mut() {
                *child = without_timings(child.take());
            }
        }
        Value::Array(items) => {
            for child in items {
                *child = without_timings(child.take());
            }
        }
        _ => {}
    }
    input
}
fn session() -> sdk::Session {
    serde_json::from_str(include_str!("fixtures/sdk/decode/session.json")).unwrap()
}
fn capture() -> sdk::CaptureHeaders {
    serde_json::from_str(include_str!("fixtures/sdk/decode/headers.json")).unwrap()
}
fn loaded() -> &'static sdk::LoadedArtifact {
    static LOADED: OnceLock<sdk::LoadedArtifact> = OnceLock::new();
    LOADED.get_or_init(|| {
        sdk::artifact::compile(
            sdk::ProgramInput::Program(PROGRAM),
            &session(),
            "example",
            sdk::ArtifactOptions::default(),
        )
        .unwrap()
    })
}
fn kernel() -> &'static sdk::KernelArtifact {
    static KERNEL: OnceLock<sdk::KernelArtifact> = OnceLock::new();
    KERNEL.get_or_init(|| sdk::report(sdk::KernelSource::Raw(RAW)).unwrap())
}

#[test]
fn loaded_lifecycle_parity_and_repeated_edited_roundtrips() {
    fn shareable<T: Send + Sync>() {}
    shareable::<sdk::LoadedArtifact>();
    let artifact = loaded();
    let clone = artifact.clone();
    assert!(std::ptr::eq(artifact.artifact(), clone.artifact()));
    let bytes = artifact.to_json().unwrap();
    let reload = sdk::artifact::load(&bytes).unwrap();
    let from_document = sdk::artifact::from_artifact(artifact.artifact().clone()).unwrap();
    assert_eq!(value(reload.artifact()), value(from_document.artifact()));
    let legacy = sdk::Generator::load(&bytes).unwrap();
    let headers = capture();
    let original = sdk::decode(artifact, &headers, None, FULL).unwrap();
    assert_eq!(
        value(&original),
        legacy
            .call(
                "decode",
                json!({"capture": headers, "verify": true, "metadata": true})
            )
            .unwrap()
    );
    assert_eq!(
        value(sdk::schema(artifact).unwrap()),
        legacy.call("schema", json!({})).unwrap()
    );
    assert_eq!(
        value(sdk::artifact::compact_schema(artifact.artifact()).unwrap()),
        value(sdk::schema(artifact).unwrap())
    );
    assert_eq!(
        value(artifact.artifact()),
        sdk::call(
            "artifact.compile",
            json!({"source": PROGRAM, "session": session(), "site_id": "example"})
        )
        .unwrap()
    );
    for n in [8, 10] {
        let mut edited = original.clone();
        edited
            .positional_signals
            .get_mut("DeviceMemory")
            .unwrap()
            .insert("deviceMemory".into(), json!(n));
        let encoded = sdk::encode(&reload, &edited, None).unwrap();
        let decoded = sdk::decode(&reload, &encoded, None, FULL).unwrap();
        assert!(decoded.metadata.exact_reencode);
        assert_eq!(decoded.positional_signals, edited.positional_signals);
        assert_eq!(
            decoded.metadata.original_report.positional_signals[0],
            original.metadata.original_report.positional_signals[0]
        );
        assert_eq!(
            value(sdk::explain(artifact, &decoded, "DeviceMemory", None, true).unwrap()),
            legacy
                .call(
                    "explain",
                    json!({"decoded": decoded, "signal": "DeviceMemory", "verify": true})
                )
                .unwrap()
        );
        let bare = sdk::encode_signals(&reload, &edited.positional_signals, None).unwrap();
        assert_eq!(
            sdk::decode(&from_document, &bare, None, FULL)
                .unwrap()
                .positional_signals,
            edited.positional_signals
        );
    }
    assert_eq!(
        value(sdk::encode(artifact, &original, None).unwrap()),
        value(&headers)
    );
    assert_eq!(
        sdk::decode(artifact, &headers, None, FULL)
            .unwrap()
            .positional_signals["DeviceMemory"]["deviceMemory"],
        9
    );
}

#[test]
fn typed_source_reports_match_legacy_projections() {
    assert_eq!(
        value(sdk::version()),
        sdk::call("version", json!({})).unwrap()
    );
    assert_eq!(sdk::commands(), sdk::COMMANDS);
    assert_eq!(
        value(
            sdk::signals::schema(
                PROGRAM,
                sdk::SchemaOptions {
                    name: Some("path/example.js".into()),
                    ..Default::default()
                }
            )
            .unwrap()
        ),
        sdk::call(
            "signals.schema",
            json!({"source": PROGRAM, "name": "path/example.js"})
        )
        .unwrap()
    );
    assert_eq!(
        value(sdk::signals::show(PROGRAM).unwrap()),
        sdk::call("signals.show", json!({"source": PROGRAM})).unwrap()
    );
    assert_eq!(
        value(sdk::signals::inputs(PROGRAM).unwrap()),
        sdk::call("signals.inputs", json!({"source": PROGRAM})).unwrap()
    );
    assert_eq!(
        value(sdk::signals::contracts(PROGRAM).unwrap()),
        sdk::call("signals.contracts", json!({"source": PROGRAM})).unwrap()
    );
    assert_eq!(
        value(sdk::pin(PROGRAM, sdk::SessionInput::Session(&session())).unwrap()),
        sdk::call("pin", json!({"source": PROGRAM, "session": session()})).unwrap()
    );
    assert_eq!(
        value(sdk::signals::compact_schema(PROGRAM, sdk::CompileOptions::default()).unwrap()),
        value(sdk::CompactSeedSchema::from_source(PROGRAM).unwrap())
    );
    let initializer = include_str!("fixtures/generator/seed-pinned-init.js");
    assert_eq!(
        value(sdk::session::from_initializer(initializer).unwrap()),
        sdk::call("session.from_initializer", json!({"init": initializer})).unwrap()
    );
    assert_eq!(
        value(sdk::pin(PROGRAM, sdk::SessionInput::Initializer(initializer)).unwrap()),
        sdk::call("pin", json!({"source": PROGRAM, "init": initializer})).unwrap()
    );
    let source = "function f(a) { let $SSA_1 = a; return $SSA_1; }";
    assert_eq!(
        sdk::cleanup::thread(source).unwrap().code,
        sdk::call("cleanup.thread", json!({"source": source})).unwrap()["code"]
    );
    // Inventory requires actual reachable main-program wiring, not an isolated f.
    let source = PROGRAM;
    assert_eq!(
        value(sdk::naming::inventory(source).unwrap()),
        sdk::call("naming.inventory", json!({"source": source})).unwrap()
    );
    let named = sdk::naming::threads(source, sdk::NamingThreadsOptions::default()).unwrap();
    assert_eq!(
        without_timings(json!({"source": named.source, "report": named.report})),
        without_timings(sdk::call("naming.threads", json!({"source": source})).unwrap())
    );
    let collectors =
        sdk::naming::collectors(source, sdk::NamingCollectorsOptions::default()).unwrap();
    assert_eq!(
        without_timings(json!({"source": collectors.source, "report": collectors.report})),
        without_timings(sdk::call("naming.collectors", json!({"source": source})).unwrap())
    );
    assert_eq!(
        value(sdk::naming::plan(source).unwrap()),
        sdk::call("naming.plan", json!({"source": source})).unwrap()
    );
}

#[test]
fn explicit_kernel_modes_trace_reuse_and_source_identity() {
    let artifact = kernel();
    let saved = sdk::kernel::to_json(artifact).unwrap();
    let restored = sdk::kernel::load(&saved).unwrap();
    assert_eq!(restored.source_hash(), artifact.source_hash());
    let deob = sdk::deob(RAW, sdk::DeobOptions { artifact: true }).unwrap();
    assert_eq!(
        value(
            sdk::report(sdk::KernelSource::Deobfuscated(&deob.code))
                .unwrap()
                .bytecode()
        ),
        value(artifact.bytecode())
    );
    assert_eq!(
        sdk::deob(RAW, sdk::DeobOptions::default()).unwrap().code,
        deob.code
    );
    assert_eq!(
        serde_json::from_str::<Value>(&saved).unwrap(),
        sdk::call("report", json!({"source": RAW})).unwrap()
    );
    assert_eq!(
        sdk::kernel::show(
            sdk::KernelSource::Raw(RAW),
            sdk::KernelShowOptions::default()
        )
        .unwrap(),
        sdk::kernel::show(
            sdk::KernelSource::Deobfuscated(&deob.code),
            sdk::KernelShowOptions::default()
        )
        .unwrap()
    );
    assert_eq!(
        sdk::ops::list(RAW).unwrap(),
        sdk::call("ops.list", json!({"source": RAW})).unwrap()
    );
    assert_eq!(
        sdk::ops::show(RAW, 0),
        sdk::call("ops.show", json!({"source": RAW, "op": 0}))
    );
    let entry = artifact.entry_facts().unwrap();
    let options = sdk::ThreadOptions {
        steps: 16,
        partial: true,
        ..sdk::ThreadOptions::new(entry.pc as usize, entry.state as usize)
    };
    let loaded_trace =
        sdk::trace::thread(sdk::KernelInput::Artifact(&restored), options.clone()).unwrap();
    let source_trace = sdk::trace::thread(sdk::KernelInput::Source(RAW), options).unwrap();
    assert_eq!(value(&loaded_trace), value(source_trace));
    assert_eq!(value(loaded_trace), sdk::call("trace.thread", json!({"artifact": saved, "pc": entry.pc, "state": entry.state, "steps": 16, "partial": true})).unwrap());
    let options = sdk::ProgramOptions {
        steps: 16,
        jobs: Some(1),
        thread_cleanup: false,
        ..Default::default()
    };
    let trace = sdk::trace::program(RAW, options.clone()).unwrap();
    assert_eq!(
        value(&trace),
        value(sdk::trace::program_with_artifact(RAW, artifact, options.clone()).unwrap())
    );
    let wrong = format!("{RAW}\n// altered source identity");
    assert_eq!(
        sdk::trace::program_with_artifact(&wrong, artifact, options)
            .err()
            .unwrap()
            .code,
        "failed"
    );
    // Malformed raw input must fail instead of producing invented signal facts.
    assert!(sdk::artifact::compile(
        sdk::ProgramInput::Raw(""),
        &session(),
        "example",
        sdk::ArtifactOptions::default()
    )
    .is_err());
}

#[test]
fn fast_trace_diagnostics_and_cleaned_fixture_signal_extraction() {
    let traced = sdk::trace::program(
        RAW,
        sdk::ProgramOptions {
            steps: 16,
            jobs: Some(2),
            thread_cleanup: true,
            no_cleanup: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!traced.code.is_empty());
    assert!(traced.traced + traced.stopped.len() > 0);
    // This small VM fixture has no signal writers. Exercise the real fast
    // output, but do not claim it proves a full raw-to-header signal recovery.
    let error = sdk::signals::schema(&traced.code, sdk::SchemaOptions::default())
        .err()
        .expect("this VM fixture has no collector requests");
    assert_eq!(error.code, "failed");
    assert!(error
        .message
        .contains("no source-derived signal collector requests"));
    // The separate public traced-program fixture contains actual writers.
    // Per-thread cleanup of its main function retains signal extraction. This
    // synthetic fixture's pinning counter scaffold does not survive cleanup;
    // full recipe roundtrips use its unchanged program in the lifecycle test.
    let (main_function, invocation) = PROGRAM.rsplit_once("\nmain()").unwrap();
    let cleaned = sdk::cleanup::thread(main_function).unwrap();
    let program = format!("{}\nmain(){}", cleaned.code, invocation);
    let schema = sdk::signals::schema(&program, sdk::SchemaOptions::default()).unwrap();
    assert!(schema.signals.contains_key("DeviceMemory"));
}

#[test]
fn policy_defaults_catalogue_and_stable_input_errors() {
    let program = sdk::ProgramOptions::default();
    assert!(matches!(program.cache, sdk::CachePolicy::Disabled));
    assert!(program.thread_cleanup && program.quiet);
    assert!(!program.no_cleanup);
    assert_eq!(program.jobs, None);
    assert_eq!(program.steps, 1_000_000);
    let artifact = sdk::ArtifactOptions::default();
    assert!(matches!(artifact.cache, sdk::CachePolicy::Disabled));
    assert!(!artifact.direct && !sdk::DeobOptions::default().artifact);
    let thread = sdk::ThreadOptions::new(7, 3);
    assert_eq!((thread.pc, thread.state, thread.steps), (7, 3, 1_000_000));
    assert!(thread.outline && !thread.partial && !thread.self_check);
    let cards = sdk::signals::catalog().unwrap();
    assert!(cards
        .windows(2)
        .all(|pair| pair[0]["signal"].as_str().unwrap() < pair[1]["signal"].as_str().unwrap()));
    for card in cards {
        assert_eq!(
            sdk::signals::describe(card["signal"].as_str().unwrap()).unwrap(),
            card
        );
    }
    assert_eq!(
        sdk::signals::describe("devicememory").unwrap_err().code,
        "invalid_request"
    );
    for bytes in [b"".as_slice(), b"{", b"null", b"{}"] {
        assert_eq!(sdk::artifact::load(bytes).err().unwrap().code, "failed");
    }
    let artifact = loaded();
    let mut wrong = capture();
    wrong.site_id = "wrong".into();
    assert_eq!(
        sdk::decode(artifact, &wrong, None, FULL).unwrap_err().code,
        "invalid_request"
    );
    let mut decoded = sdk::decode(artifact, &capture(), None, FULL).unwrap();
    assert_eq!(
        sdk::explain(artifact, &decoded, "Unknown", None, false)
            .unwrap_err()
            .code,
        "invalid_request"
    );
    decoded.site_id = "wrong".into();
    assert_eq!(
        sdk::encode(artifact, &decoded, None).unwrap_err().code,
        "invalid_request"
    );
    assert_eq!(
        sdk::explain(artifact, &decoded, "DeviceMemory", None, false)
            .unwrap_err()
            .code,
        "invalid_request"
    );
    let compact = serde_json::to_vec(&sdk::schema(artifact).unwrap()).unwrap();
    assert_eq!(sdk::artifact::load(&compact).err().unwrap().code, "failed");
}

#[test]
fn every_command_has_a_named_typed_entrypoint() {
    // Each symbol is resolved by Rust; a new COMMANDS entry requires an explicit
    // mapping here. Execution parity is covered by representative families above.
    macro_rules! operation {
        ($id:literal, $path:path) => {{
            let _ = $path;
            $id
        }};
    }
    let named = [
        operation!("version", sdk::version),
        operation!("deob", sdk::deob),
        operation!("report", sdk::report),
        operation!("kernel.show", sdk::kernel::show),
        operation!("ops.list", sdk::ops::list),
        operation!("ops.show", sdk::ops::show),
        operation!("trace.program", sdk::trace::program),
        operation!("trace.thread", sdk::trace::thread),
        operation!("cleanup.thread", sdk::cleanup::thread),
        operation!("naming.threads", sdk::naming::threads),
        operation!("naming.collectors", sdk::naming::collectors),
        operation!("naming.plan", sdk::naming::plan),
        operation!("naming.inventory", sdk::naming::inventory),
        operation!("signals.schema", sdk::signals::schema),
        operation!("signals.show", sdk::signals::show),
        operation!("signals.inputs", sdk::signals::inputs),
        operation!("signals.contracts", sdk::signals::contracts),
        operation!("pin", sdk::pin),
        operation!("session.from_initializer", sdk::session::from_initializer),
        operation!("artifact.compile", sdk::artifact::compile),
        operation!("artifact.compact_schema", sdk::artifact::compact_schema),
    ];
    assert_eq!(named, sdk::COMMANDS);
}
