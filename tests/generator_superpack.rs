use serde::Deserialize;
use sha2::{Digest, Sha256};
use shape::signals::payload::superpack::{
    pack, unpack, unpack_canonical, Number, PackRequest, SerializerOptions, Value, DEPTH_EXTENSION,
};

#[derive(Deserialize)]
struct Answer {
    name: String,
    request: PackRequest,
    hex: String,
    decodable: bool,
}
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
fn n(v: f64) -> Value {
    Value::from(v)
}
fn s(v: &str) -> Value {
    Value::String(v.into())
}
#[test]
fn original_source_known_answers() {
    let answers: Vec<Answer> = serde_json::from_str(include_str!(
        "fixtures/generator/superpack-known-answers.json"
    ))
    .unwrap();
    assert_eq!(answers.len(), 95);
    for a in answers {
        let actual = a
            .request
            .pack()
            .unwrap_or_else(|e| panic!("{} pack: {}", a.name, e));
        assert_eq!(actual, hex(&a.hex), "{} source bytes", a.name);
        let decoded = unpack(&actual, &a.request.options);
        if !a.decodable {
            assert!(decoded.is_err(), "{} truncated metadata", a.name);
            continue;
        }
        let decoded = decoded.unwrap_or_else(|e| panic!("{} decode: {}", a.name, e));
        assert!(decoded.canonical, "{} canonical", a.name);
        assert_eq!(
            pack(&decoded.value, &a.request.options).unwrap(),
            actual,
            "{} reencode",
            a.name
        );
        let request_json = serde_json::to_string(&a.request).unwrap();
        assert_eq!(
            serde_json::from_str::<PackRequest>(&request_json)
                .unwrap()
                .pack()
                .unwrap(),
            actual
        );
    }
}
#[test]
fn typed_values_roundtrip_and_object_normalization() {
    let o = SerializerOptions::default();
    for v in [
        Value::Undefined,
        Value::Null,
        Value::Bool(true),
        n(-0.0),
        n(f64::INFINITY),
        n(f64::NEG_INFINITY),
        n(f64::NAN),
        s("hé\0😀"),
        Value::Binary(vec![0, 64, 255]),
        Value::Date((1 << 48) - 1),
        Value::Array(vec![
            n(0.1),
            Value::Undefined,
            Value::Array(vec![Value::Bool(true)]),
        ]),
    ] {
        assert_eq!(unpack_canonical(&pack(&v, &o).unwrap(), &o).unwrap(), v);
    }
    let v = Value::Object(vec![("b".into(), n(2.)), ("a".into(), n(1.))]);
    assert_eq!(
        unpack_canonical(&pack(&v, &o).unwrap(), &o).unwrap(),
        Value::Object(vec![("a".into(), n(1.)), ("b".into(), n(2.))])
    );
    let nan = Value::Number(Number(0xfff8000000000001));
    assert_eq!(pack(&nan, &o).unwrap(), hex("ec7fc00000"));
}
#[test]
fn original_source_oracle_is_reproducible_when_node_available() {
    // Fixture integrity is checked even without Node. Capture paths/hashes are
    // provenance metadata only: regression tests never open ignored reports.
    for (provenance, name, reference) in [
        (
            include_str!("fixtures/generator/superpack-provenance.json"),
            "superpack-reference.cjs",
            include_bytes!("fixtures/generator/superpack-reference.cjs").as_slice(),
        ),
        (
            include_str!("fixtures/generator/superpack-provenance-alternate.json"),
            "superpack-reference-alternate.cjs",
            include_bytes!("fixtures/generator/superpack-reference-alternate.cjs").as_slice(),
        ),
    ] {
        let provenance: serde_json::Value = serde_json::from_str(provenance).unwrap();
        assert_eq!(provenance["reference_file"].as_str(), Some(name));
        let digest: String = Sha256::digest(reference)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            provenance["reference_sha256"].as_str(),
            Some(digest.as_str()),
            "shipped reference integrity: {name}"
        );
    }
    // Only a missing Node executable skips live execution. Literal byte-vector
    // tests and the integrity checks above remain unconditional.
    let version = match std::process::Command::new("node").arg("--version").output() {
        Ok(version) => version,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("cannot launch Node reference oracle: {error}"),
    };
    assert!(version.status.success());
    let script = r#"
const fs=require('node:fs'),crypto=require('node:crypto');
const fixture='tests/fixtures/generator/';
for(const suffix of ['', '-alternate']) {
const p=JSON.parse(fs.readFileSync(fixture+'superpack-provenance'+suffix+'.json'));
const reference=fs.readFileSync(fixture+p.reference_file);
if(crypto.createHash('sha256').update(reference).digest('hex')!==p.reference_sha256) throw Error('shipped reference integrity mismatch');
}
const api=require('./'+fixture+'superpack-reference.cjs');
if(Buffer.from(api.packValue(new Uint8Array([0,64,255]).buffer)).toString('hex')!=='ef0300404040ff') throw Error('binary mismatch');
const out=Buffer.from(api.packValue({b:2,a:1})).toString('hex');
if(out!=='fe00a1a2c161c162f8000102')throw Error(out);
process.argv.push('--check');
require('./'+fixture+'superpack-vectors.cjs');
"#;
    let output = std::process::Command::new("node")
        .args(["-e", script])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn malformed_input_and_limits() {
    let o = SerializerOptions::default();
    for bytes in [
        hex(""),
        hex("00ff"),
        hex("e4"),
        hex("e7ffffffffffffffff"),
        hex("e70020000000000001"),
        hex("f300"),
        hex("f800"),
        hex("f900"),
        hex("f0ffff"),
        hex("c1ff"),
        hex("ef014100"),
        hex("f5e7ffffffffffffffff"),
        hex("f2e7ffffffffffffffff"),
        hex("ff e70000000100000000".replace(' ', "").as_str()),
        hex("fa00"),
        hex("fb00"),
        hex("fc"),
        hex("fd"),
        hex("fe01f300a000"),
        hex("fe00a1a2c161c161f8000000"),
        hex("ff00e2"),
        hex("ff00e3"),
        hex("ff07e2"),
        hex("ef014100"),
    ] {
        assert!(unpack(&bytes, &o).is_err(), "accepted {bytes:02x?}");
    }
    let bytes = pack(&Value::Array(vec![Value::Array(vec![n(1.)])]), &o).unwrap();
    let mut small = o.clone();
    small.limits.max_depth = 0;
    assert!(unpack(&bytes, &small).is_err());
    assert!(pack(&Value::Array(vec![n(1.)]), &small).is_err());
    small = o.clone();
    small.limits.max_entries = 2;
    assert!(unpack(&hex("f664"), &small).is_err());
    assert!(pack(&Value::Array(vec![Value::Bool(true); 3]), &small).is_err());
    small = o.clone();
    small.limits.max_bytes = 2;
    assert!(pack(&s("abc"), &small).is_err());
    assert!(unpack(&hex("c3616263"), &small).is_err());
    small = o.clone();
    small.limits.max_depth = 257;
    assert!(pack(&Value::Null, &small).is_err());
}
#[test]
fn extensions_and_preloads_are_explicit() {
    let o = SerializerOptions::default();
    assert!(pack(&Value::MissingSentinel, &o).is_err());
    let mut opts = SerializerOptions::outer_channel(vec![vec!["a".into()]]);
    let v = Value::Object(vec![("a".into(), Value::MissingSentinel)]);
    let bytes = pack(&v, &opts).unwrap();
    assert_eq!(bytes, hex("f800ff00e3"));
    assert!(unpack(&bytes, &o).is_err());
    assert_eq!(unpack_canonical(&bytes, &opts).unwrap(), v);
    opts.depth_bound = Some(0);
    assert_eq!(
        pack(&Value::Array(vec![]), &opts).unwrap(),
        hex("ffe6deadbeefe2")
    );
    let marker = Value::Extension {
        id: DEPTH_EXTENSION,
        payload: Box::new(Value::Null),
    };
    assert_eq!(
        unpack_canonical(&hex("ffe6deadbeefe2"), &opts).unwrap(),
        marker
    );
    let unknown = Value::Extension {
        id: 7,
        payload: Box::new(s("ok")),
    };
    assert!(pack(&unknown, &o).is_err());
    let mut opaque = o.clone();
    opaque.opaque_extensions = true;
    assert_eq!(pack(&unknown, &opaque).unwrap(), hex("ff07c26f6b"));
    assert_eq!(
        unpack_canonical(&hex("ff07c26f6b"), &opaque).unwrap(),
        unknown
    );
    assert!(pack(
        &Value::Extension {
            id: 0,
            payload: Box::new(Value::Null)
        },
        &opaque
    )
    .is_err());
    assert!(pack(&Value::Date(1 << 48), &o).is_err());
    assert!(pack(
        &Value::Object(vec![("a".into(), n(1.)), ("a".into(), n(2.))]),
        &o
    )
    .is_err());
    assert!(serde_json::from_str::<Value>(r#"{"kind":"date","value":-1}"#).is_err());
    assert!(serde_json::from_str::<Value>(r#"{"kind":"symbol"}"#).is_err());
}
#[test]
fn noncanonical_bytes_remain_available() {
    let o = SerializerOptions::default();
    for b in [
        hex("e40001"),
        hex("ec7fc00001"),
        hex("ed8000000000000000"),
        hex("91ff"),
        hex("80"),
        hex("f10161"),
    ] {
        let decoded = unpack(&b, &o).unwrap();
        assert!(!decoded.canonical);
        assert_eq!(decoded.original_bytes, b);
        assert!(unpack_canonical(&b, &o).is_err());
        assert!(pack(&decoded.value, &o).is_ok());
    }
}
#[test]
fn truncation_at_every_byte_boundary() {
    let o = SerializerOptions::default();
    let v = Value::Array(vec![
        Value::Object(vec![("first".into(), Value::Binary(vec![64, 255]))]),
        s("hello"),
        s("hello"),
    ]);
    let bytes = pack(&v, &o).unwrap();
    for i in 0..bytes.len() {
        assert!(unpack(&bytes[..i], &o).is_err(), "prefix {i}");
    }
}
