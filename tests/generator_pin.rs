//! One traced seed pins its capture facts into a loader artifact.
//!
//! The program fixture is the output of `shape trace program` for one raw seed;
//! the initializer is the shipped `detail.init(...)` call; the headers are the
//! captured `x-gyjwza5z-*` map for that seed. Pinning must recover the cipher,
//! the report layout and the header policy, save them as one artifact, and the
//! artifact must load straight into a codec that decodes the captured `a`/`a0`
//! chunks and re-emits every header byte-for-byte.
use shape::signals::payload::{
    extract::{pin, session_from_initializer, PinnedSeed, SeedLayout, SEED_LAYOUT_FORMAT},
    report::ChannelReport,
    Generator, HeaderReport, Headers, PayloadCodec, PayloadProfile,
};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

const CLI: &str = env!("CARGO_BIN_EXE_shape");

const PROGRAM: &str = include_str!("fixtures/generator/seed-pinned-program.js");
const INITIALIZER: &str = include_str!("fixtures/generator/seed-pinned-init.js");
const HEADERS: &str = include_str!("fixtures/generator/seed-capture-headers.json");
const REPORT: &str = include_str!("fixtures/generator/seed-capture-report.json");
const LAYOUT: &str = include_str!("fixtures/generator/seed-pinned-layout.json");

fn channels_of(report: &HeaderReport) -> ChannelReport {
    report.channels_owned()
}

fn pinned() -> PinnedSeed {
    pin(PROGRAM, &session_from_initializer(INITIALIZER).unwrap()).unwrap()
}

#[test]
fn pinned_artifact_records_the_build_facts_and_report_layout() {
    let pinned = pinned();
    let layout = &pinned.layout;
    assert_eq!(layout.format, SEED_LAYOUT_FORMAT);
    assert_eq!(
        layout.cipher.ivs,
        [2241456886, 1063953896, 3131140720, 3913805869]
    );
    assert_eq!(layout.cipher.counter, 3685634438990593);
    assert_eq!(layout.cipher.double_rounds, 4);
    assert_eq!(layout.header.name, "x-gyjwza5z-a");
    assert_eq!(layout.header.prefix, "x-gyjwza5z-");
    assert_eq!(layout.header.chunk_size, Some(7900));
    assert_eq!(layout.header.z, "q");
    assert_eq!(layout.keysets, Vec::<Vec<String>>::new());
    // The report is the four source channels, as layout rather than bytes:
    // one slot per selected descriptor, contiguous and in filter order.
    let serialized = serde_json::to_string(&layout.report).unwrap();
    let positions: Vec<usize> = [
        "\"errors\"",
        "\"positionalSignals\"",
        "\"signals\"",
        "\"keyedSignals\"",
    ]
    .iter()
    .map(|key| {
        serialized
            .find(key)
            .unwrap_or_else(|| panic!("missing {key}"))
    })
    .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    let report = serde_json::to_value(&layout.report).unwrap();
    let slots = report["positionalSignals"].as_array().unwrap();
    assert_eq!(slots.len(), 77);
    for (index, slot) in slots.iter().enumerate() {
        assert_eq!(slot["slot"], serde_json::json!(index));
        assert_eq!(slot["descriptor"], serde_json::json!(index));
        assert!(slot["ctor"].is_boolean());
        assert!(slot["priority"].is_boolean());
    }
    assert_eq!(report["signals"], serde_json::json!([]));
    assert_eq!(report["keyedSignals"], serde_json::json!([]));
    // Every fact keeps its byte span.
    for role in [
        "stream_cipher_state",
        "stream_cipher_rounds",
        "signal_filter",
        "descriptor_table",
        "descriptor_routing",
        "keyset_mapping",
        "header_prefix",
        "header_chunk_size",
        "header_c_default",
        "header_d_default",
        "header_z_default",
    ] {
        assert!(
            pinned.evidence.iter().any(|e| e.role == role),
            "missing evidence {role}"
        );
    }
    // The saved artifact is byte-for-byte what `shape gen pin` writes.
    let saved: SeedLayout = SeedLayout::from_json(LAYOUT.as_bytes()).unwrap();
    assert_eq!(&saved, layout);
    assert_eq!(pinned.to_json_pretty(), LAYOUT.as_bytes());
}

#[test]
fn saved_artifact_loads_into_the_captured_report_and_reencodes_headers() {
    // Loading validates the artifact and builds the codec without the seed.
    let layout = SeedLayout::from_json(LAYOUT.as_bytes()).unwrap();
    let codec: Generator = layout.generator().unwrap();
    let headers: Headers = serde_json::from_str(HEADERS).unwrap();
    let report = layout.decode_headers(&headers).unwrap();
    let expected: HeaderReport = HeaderReport::from_json(REPORT.as_bytes()).unwrap();
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    assert_eq!(report.positional_signals.len(), 77);
    assert_eq!(report.seeds, [306906895, 3915760865]);
    assert_eq!(
        layout
            .encode_headers(&channels_of(&report), report.seeds)
            .unwrap(),
        headers
    );
    // The channel projection is exactly the four source keys, in report order.
    // `serde_json::to_value` sorts object keys, so check the serialized text.
    let channels = serde_json::to_string(&report.channels()).unwrap();
    let positions: Vec<usize> = [
        "\"errors\"",
        "\"positionalSignals\"",
        "\"signals\"",
        "\"keyedSignals\"",
    ]
    .iter()
    .map(|key| {
        channels
            .find(key)
            .unwrap_or_else(|| panic!("missing {key}"))
    })
    .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(!channels.contains("\"seeds\""));
    assert!(!channels.contains("\"header_context\""));
    assert_eq!(
        serde_json::to_value(report.channels()).unwrap()["positionalSignals"]
            .as_array()
            .unwrap()
            .len(),
        77
    );
}

#[test]
fn the_four_channel_document_encodes_back_to_the_captured_bytes() {
    let layout = SeedLayout::from_json(LAYOUT.as_bytes()).unwrap();
    let headers: Headers = serde_json::from_str(HEADERS).unwrap();
    let report = layout.decode_headers(&headers).unwrap();
    let channels = channels_of(&report);
    let joined = joined_payload(&headers);
    // Decode output is exactly the four keys; encode consumes it with seeds.
    assert_eq!(
        layout.encode_payload(&channels, report.seeds).unwrap(),
        joined
    );
    assert_eq!(
        layout.encode_headers(&channels, report.seeds).unwrap(),
        headers
    );
}

#[test]
fn encode_runs_superpack_container_cipher_and_base64_in_order() {
    let layout = SeedLayout::from_json(LAYOUT.as_bytes()).unwrap();
    let profile = layout.profile();
    let headers: Headers = serde_json::from_str(HEADERS).unwrap();
    let joined = joined_payload(&headers);
    let report = layout.decode_headers(&headers).unwrap();
    let channels = channels_of(&report);

    // Decode: custom base64 + pinned ChaCha-style cipher recover the seeds and
    // the container plaintext.
    let payload = layout.generator().unwrap().decode(&joined).unwrap();
    let seeds = payload.seeds;
    let plaintext = payload.plaintext().unwrap();
    assert_eq!(seeds, report.seeds);
    // The captured framing is canonical: parsing and repacking keeps bytes.
    assert_eq!(
        payload.parse_container(77).unwrap().plaintext().unwrap(),
        plaintext
    );

    // Encode stage 1: superpack each value channel, then frame the container
    // (ULEB lengths, positional encoded+extra, signals, keyed).
    let rebuilt = channels
        .report(seeds, layout.keysets.clone())
        .to_payload()
        .unwrap();
    assert_eq!(rebuilt.plaintext().unwrap(), plaintext);
    let extras: usize = report
        .positional_signals
        .iter()
        .flatten()
        .map(|entry| entry.1.len())
        .sum();
    assert_eq!(extras, 6760);

    // Encode stage 2: seed prefix + cipher + unpadded custom base64.
    let payload_codec = PayloadCodec::new(PayloadProfile::from(profile.clone())).unwrap();
    assert_eq!(
        payload_codec.encode_bytes(&plaintext, seeds).unwrap(),
        joined
    );
    assert_eq!(
        payload_codec.decode(&joined).unwrap().plaintext().unwrap(),
        plaintext
    );

    // Encode stage 3: chunk the payload and add b/f/c/d/z.
    assert_eq!(layout.encode_headers(&channels, seeds).unwrap(), headers);
    // Decode the payload path end to end.
    assert_eq!(layout.decode_payload(&joined).unwrap(), report.report());
}

fn joined_payload(headers: &Headers) -> String {
    format!(
        "{}{}",
        headers.get("x-gyjwza5z-a").unwrap(),
        headers.get("x-gyjwza5z-a0").unwrap()
    )
}

#[test]
fn a_malformed_layout_is_rejected_instead_of_guessed() {
    let value: serde_json::Value = serde_json::from_str(LAYOUT).unwrap();
    assert!(SeedLayout::from_json(&serde_json::to_vec(&value).unwrap()).is_ok());
    // Noncontiguous slots are a loader error, not an implicit recount.
    let mut broken = value.clone();
    broken["report"]["positionalSignals"][3]["slot"] = serde_json::json!(99);
    assert!(SeedLayout::from_json(&serde_json::to_vec(&broken).unwrap()).is_err());
    // A tampered session no longer matches the `b` checksum.
    let mut tampered = value;
    tampered["session"]["transactionToken"] = serde_json::json!("other-token");
    let layout = SeedLayout::from_json(&serde_json::to_vec(&tampered).unwrap()).unwrap();
    let codec = layout.generator().unwrap();
    let headers: Headers = serde_json::from_str(HEADERS).unwrap();
    assert!(codec.decode_report_headers(&headers, 77, vec![]).is_err());
}

#[test]
fn the_cli_pins_a_program_and_round_trips_the_capture() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "shape-pin-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(CLI)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    let program = dir.join("program.js");
    let init = dir.join("init.js");
    let seed = dir.join("seed.json");
    let report = dir.join("report.json");
    let channels = dir.join("channels.json");
    let captured = dir.join("captured.json");
    let headers = dir.join("headers.json");
    let payload = dir.join("payload.txt");
    fs::write(&program, PROGRAM).unwrap();
    fs::write(&init, INITIALIZER).unwrap();
    fs::write(&captured, HEADERS).unwrap();

    run(&[
        "pin",
        "--input",
        program.to_str().unwrap(),
        "--init",
        init.to_str().unwrap(),
        "--out",
        seed.to_str().unwrap(),
    ]);
    let pinned: SeedLayout = SeedLayout::from_json(&fs::read(&seed).unwrap()).unwrap();
    assert_eq!(pinned.format, SEED_LAYOUT_FORMAT);

    // Decode with only the artifact: count and keysets come from it.
    run(&[
        "headers",
        "decode",
        "--seed",
        seed.to_str().unwrap(),
        "--input",
        captured.to_str().unwrap(),
        "--out",
        report.to_str().unwrap(),
    ]);
    run(&[
        "headers",
        "decode",
        "--seed",
        seed.to_str().unwrap(),
        "--input",
        captured.to_str().unwrap(),
        "--channels",
        "--out",
        channels.to_str().unwrap(),
    ]);
    let decoded: HeaderReport = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    let seed_pair: [u32; 2] = decoded.seeds;
    let seed_words = format!("{},{}", seed_pair[0], seed_pair[1]);

    // Encode the four-channel document back with the artifact and the seeds.
    run(&[
        "headers",
        "encode",
        "--seed",
        seed.to_str().unwrap(),
        "--seeds",
        &seed_words,
        "--input",
        channels.to_str().unwrap(),
        "--out",
        headers.to_str().unwrap(),
    ]);
    let written: Headers = serde_json::from_slice(&fs::read(&headers).unwrap()).unwrap();
    let captured: Headers = serde_json::from_str(HEADERS).unwrap();
    assert_eq!(written, captured);
    run(&[
        "payload",
        "encode",
        "--seed",
        seed.to_str().unwrap(),
        "--seeds",
        &seed_words,
        "--input",
        channels.to_str().unwrap(),
        "--out",
        payload.to_str().unwrap(),
    ]);
    let expected: Headers = serde_json::from_str(HEADERS).unwrap();
    assert_eq!(
        fs::read_to_string(&payload).unwrap().trim(),
        format!(
            "{}{}",
            expected.get("x-gyjwza5z-a").unwrap(),
            expected.get("x-gyjwza5z-a0").unwrap()
        )
    );

    // Payload-only decode returns the four channels.
    run(&[
        "payload",
        "decode",
        "--seed",
        seed.to_str().unwrap(),
        "--input",
        payload.to_str().unwrap(),
        "--channels",
        "--out",
        channels
            .with_file_name("payload-channels.json")
            .to_str()
            .unwrap(),
    ]);
    let text = fs::read_to_string(channels.with_file_name("payload-channels.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        value.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["errors", "keyedSignals", "positionalSignals", "signals"]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_first_encode_or_decode_builds_the_artifact() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "shape-first-run-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    let program = dir.join("program.js");
    let init = dir.join("init.js");
    let captured = dir.join("captured.json");
    let seed = dir.join("seed.json");
    let report = dir.join("report.json");
    let channels = dir.join("channels.json");
    fs::write(&program, PROGRAM).unwrap();
    fs::write(&init, INITIALIZER).unwrap();
    fs::write(&captured, HEADERS).unwrap();
    let run = |args: &[&str]| {
        Command::new(CLI)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap()
    };
    let ok = |args: &[&str]| {
        let output = run(args);
        assert!(
            output.status.success(),
            "{} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };

    // The first decode pins the program, saves the artifact and decodes.
    ok(&[
        "headers",
        "decode",
        "--program",
        program.to_str().unwrap(),
        "--init",
        init.to_str().unwrap(),
        "--save-seed",
        seed.to_str().unwrap(),
        "--input",
        captured.to_str().unwrap(),
        "--out",
        report.to_str().unwrap(),
    ]);
    let layout = SeedLayout::from_json(&fs::read(&seed).unwrap()).unwrap();
    assert_eq!(layout.report.positional_signals.len(), 77);
    let decoded: HeaderReport = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(decoded.positional_signals.len(), 77);

    // Later runs load only the artifact.
    ok(&[
        "headers",
        "decode",
        "--seed",
        seed.to_str().unwrap(),
        "--input",
        captured.to_str().unwrap(),
        "--channels",
        "--out",
        channels.to_str().unwrap(),
    ]);
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&channels).unwrap()).unwrap();
    assert_eq!(
        value.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["errors", "keyedSignals", "positionalSignals", "signals"]
    );

    // The program needs its session literals, and cannot be combined with a
    // saved artifact or used with --save-seed alone.
    let missing = run(&[
        "headers",
        "decode",
        "--program",
        program.to_str().unwrap(),
        "--input",
        captured.to_str().unwrap(),
    ]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr)
        .contains("--program needs exactly one of --init or --session"));
    let both = run(&[
        "headers",
        "decode",
        "--program",
        program.to_str().unwrap(),
        "--seed",
        seed.to_str().unwrap(),
        "--init",
        init.to_str().unwrap(),
    ]);
    assert!(!both.status.success());
    assert!(String::from_utf8_lossy(&both.stderr).contains("cannot be used with"));
    let orphan = run(&["headers", "decode", "--save-seed", "x.json"]);
    assert!(!orphan.status.success());
    assert!(String::from_utf8_lossy(&orphan.stderr).contains("--program"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn interleaved_named_and_numeric_routes_keep_channel_local_positional_indices() {
    use shape::signals::payload::extract::pin_source;
    let baseline = pinned();
    let span = baseline.evidence.iter().find(|e|e.role=="descriptor_table").unwrap();
    let mut entries = vec!["{ctor:null,priority:false}";77];
    entries[1] = "{name:'17',ctor:null,priority:true}";
    entries[3] = "{name:17,ctor:null,priority:false}";
    entries[4] = "{name:'17',ctor:null,priority:false}";
    let mut source = PROGRAM.to_owned();
    source.replace_range(span.start as usize..span.end as usize,&format!("[{}]",entries.join(",")));
    let facts = pin_source(&source).unwrap();
    assert_eq!(facts.report.positional_signals.len(),74);
    assert_eq!(facts.report.signals.len(),2);
    assert_eq!(facts.report.keyed_signals.len(),1);
    assert_eq!(facts.report.signals[0].name,"17");
    assert_eq!(facts.report.keyed_signals[0].name,17);
    assert_eq!(facts.report.signals[1].name,"17");
    assert_eq!(facts.report.positional_signals[1].descriptor,2);
    for (index,slot) in facts.report.positional_signals.iter().enumerate() { assert_eq!(slot.slot,index); }
}
