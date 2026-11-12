use serde::Deserialize;
use serde_json::json;
use shape::signals::payload::{
    Container, Data, Generator, Headers, KeyedEntry, Payload, PositionalEntry, Profile,
    MAX_CONTAINER_ENTRIES, MAX_PLAINTEXT_BYTES,
};
use shape::signals::codec::{base64_decode, base64_encode, pack_container, uleb, unpack_container};

#[derive(Deserialize)]
struct KnownAnswer {
    source: String,
    profile: Profile,
    seeds: [u32; 2],
    plaintext_hex: String,
    value: String,
}

fn fixtures() -> Vec<KnownAnswer> {
    serde_json::from_str(include_str!("fixtures/generator/known-answers.json")).unwrap()
}

fn profile() -> Profile {
    fixtures().remove(0).profile
}
fn generator() -> Generator {
    Generator::new(profile()).unwrap()
}
fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn independent_js_and_wasm_known_answers_at_four_five_and_six_double_rounds() {
    for fixture in fixtures() {
        let codec = Generator::new(fixture.profile).unwrap();
        let bytes = hex(&fixture.plaintext_hex);
        assert_eq!(
            codec.encode_bytes(&bytes, fixture.seeds).unwrap(),
            fixture.value,
            "{}",
            fixture.source
        );
        let decoded = codec.decode(&fixture.value).unwrap();
        assert_eq!(decoded, Payload::raw(fixture.seeds, bytes));
        assert_eq!(codec.encode(&decoded).unwrap(), fixture.value);
        let json = serde_json::to_string(&decoded).unwrap();
        assert_eq!(
            codec.encode(&serde_json::from_str(&json).unwrap()).unwrap(),
            fixture.value
        );
        let raw = base64_decode(&fixture.value, &codec.profile().alphabet).unwrap();
        assert_eq!(&raw[..4], fixture.seeds[0].to_be_bytes());
        assert_eq!(&raw[4..8], fixture.seeds[1].to_be_bytes());
    }
}

#[test]
fn raw_bytes_empty_binary_and_multiblock_with_explicit_rounds() {
    for rounds in [1, 4, 5, 6, 10, 20] {
        let mut p = profile();
        p.cipher.double_rounds = rounds;
        let codec = Generator::new(p).unwrap();
        for len in [0, 1, 2, 3, 63, 64, 65, 255, 1025] {
            let bytes: Vec<u8> = (0..len).map(|i| i as u8).collect();
            let payload = Payload::raw([0, u32::MAX], bytes);
            let encoded = codec.encode(&payload).unwrap();
            assert_eq!(codec.decode(&encoded).unwrap(), payload);
        }
    }
}

#[test]
fn ten_double_rounds_match_original_chacha20_zero_key_block() {
    // Published ChaCha20 all-zero key/nonce, counter=0 block. This uses the
    // original 64-bit nonce layout (not the IETF 96-bit nonce construction).
    let mut p = profile();
    p.cipher.ivs = [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574];
    p.cipher.counter = 0;
    p.cipher.double_rounds = 10;
    p.keys = [0; 8];
    let codec = Generator::new(p).unwrap();
    let value = codec.encode_bytes(&[0; 64], [0, 0]).unwrap();
    let raw = base64_decode(&value, &codec.profile().alphabet).unwrap();
    assert_eq!(
        &raw[8..],
        hex(concat!(
            "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7",
            "da41597c5157488d7724e03fb8d84a376a43b8f41518a11cc387b669b2ee6586"
        ))
    );
}

#[test]
fn counter_wraps_at_53_bits_between_blocks() {
    let mut p = profile();
    p.cipher.counter = (1 << 53) - 1;
    let codec = Generator::new(p.clone()).unwrap();
    let full = base64_decode(&codec.encode_bytes(&[0; 128], [1, 2]).unwrap(), &p.alphabet).unwrap();
    p.cipher.counter = 0;
    let zero = Generator::new(p.clone()).unwrap();
    let first = base64_decode(&zero.encode_bytes(&[0; 64], [1, 2]).unwrap(), &p.alphabet).unwrap();
    assert_eq!(&full[72..], &first[8..]);
}

fn container() -> Container {
    Container {
        errors: vec![0, 255, 128],
        positional: vec![
            Some(PositionalEntry {
                encoded: vec![255, 0, 42],
                extra: vec![0, 128, 1, 2, 3],
            }),
            None,
            Some(PositionalEntry {
                encoded: vec![9],
                extra: vec![],
            }),
        ],
        signals: vec![200, 0, 201],
        // Deliberately unsorted and duplicate names: no map/sort/dedup allowed.
        keyed: vec![
            KeyedEntry {
                name: u32::MAX,
                encoded: vec![],
                extra: vec![255],
            },
            KeyedEntry {
                name: 1,
                encoded: vec![0, 254],
                extra: vec![],
            },
            KeyedEntry {
                name: 1,
                encoded: vec![],
                extra: vec![],
            },
        ],
    }
}

#[test]
fn opaque_container_sentinel_key_order_and_json_are_lossless() {
    let codec = generator();
    for container in [container(), Container::default()] {
        let count = container.positional.len();
        let payload = Payload {
            header_context: None,
            seeds: [7, 9],
            data: Data::Container {
                container: container.clone(),
            },
        };
        let value = codec.encode(&payload).unwrap();
        let raw = codec.decode(&value).unwrap();
        assert_eq!(raw.plaintext().unwrap(), pack_container(&container));
        let parsed = raw.parse_container(count).unwrap();
        assert_eq!(parsed, payload);
        assert_eq!(codec.encode(&parsed).unwrap(), value);
        let input: Payload =
            serde_json::from_str(&serde_json::to_string(&parsed).unwrap()).unwrap();
        assert_eq!(codec.encode(&input).unwrap(), value);
    }
}

#[test]
fn structured_known_answer_matches_existing_js_oracle_without_signal_semantics() {
    let fixture = fixtures().remove(1);
    let codec = Generator::new(fixture.profile).unwrap();
    let parsed = codec
        .decode(&fixture.value)
        .unwrap()
        .parse_container(1)
        .unwrap();
    assert!(
        matches!(&parsed.data, Data::Container { container } if container.positional.len() == 1)
    );
    assert_eq!(codec.encode(&parsed).unwrap(), fixture.value);
}

#[test]
fn noncanonical_framing_preserved_raw_and_refused_as_structured() {
    // Overlong zero for the errors length, then empty signals.
    let payload = Payload::raw([1, 2], vec![0x80, 0, 0]);
    let value = generator().encode(&payload).unwrap();
    let raw = generator().decode(&value).unwrap();
    assert_eq!(raw, payload);
    assert!(raw
        .parse_container(0)
        .unwrap_err()
        .to_string()
        .contains("noncanonical"));
    assert_eq!(generator().encode(&raw).unwrap(), value);
    let mut c = Container::default();
    c.positional.push(Some(PositionalEntry::default()));
    assert!(generator()
        .encode(&Payload {
            header_context: None,
            seeds: [1, 2],
            data: Data::Container { container: c }
        })
        .is_err());
}

#[test]
fn explicit_profile_validation_rejects_bad_geometry_alphabets_and_header_policy() {
    let original = serde_json::to_value(profile()).unwrap();
    for (path, value) in [
        ("/format", json!("unknown")),
        ("/cipher/double_rounds", json!(0)),
        ("/cipher/double_rounds", json!(21)),
        ("/cipher/counter", json!(1u64 << 53)),
        ("/alphabet", json!("a".repeat(64))),
        ("/alphabet", json!("short")),
        ("/alphabet", json!("x".repeat(66))),
        (
            "/alphabet",
            json!(format!("\n{}", &profile().alphabet[1..])),
        ),
        ("/alphabet", json!(format!("é{}", &profile().alphabet[2..]))),
        ("/header_name", json!("X-foo-b")),
        ("/header_name", json!("X-test-a\r\nInjected: 1")),
        ("/header_name", json!("X bad-a")),
        ("/header_name", json!("")),
        ("/header_name", json!(format!("{}-a", "x".repeat(256)))),
    ] {
        let mut value_json = original.clone();
        *value_json.pointer_mut(path).unwrap() = value;
        let parsed = serde_json::from_value(value_json);
        assert!(
            parsed.map(Generator::new).map_or(true, |v| v.is_err()),
            "{path}"
        );
    }
}

#[test]
fn strict_json_requires_fields_exact_widths_and_one_data_representation() {
    let original = serde_json::to_value(profile()).unwrap();
    for field in ["format", "cipher", "alphabet", "keys", "header_name"] {
        let mut value = original.clone();
        value.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Profile>(value).is_err(), "{field}");
    }
    for field in ["ivs", "counter", "double_rounds"] {
        let mut value = original.clone();
        value["cipher"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Profile>(value).is_err(), "{field}");
    }
    for (path, value) in [
        ("/keys", json!([1, 2])),
        ("/cipher/ivs", json!([1, 2, 3])),
        ("/cipher/counter", json!(-1)),
        ("/cipher/counter", json!(1.5)),
        ("/cipher/double_rounds", json!(4294967296u64)),
        ("/keys/0", json!(4294967296u64)),
        ("/keys/0", json!(-1)),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = value;
        assert!(serde_json::from_value::<Profile>(v).is_err(), "{path}");
    }
    let mut v = original;
    v["token"] = json!("unused token must not silently pass");
    assert!(serde_json::from_value::<Profile>(v).is_err());
    for value in [
        json!({"data":{"kind":"raw","bytes":[]}}),
        json!({"seeds":[1],"data":{"kind":"raw","bytes":[]}}),
        json!({"seeds":[1,2,3],"data":{"kind":"raw","bytes":[]}}),
        json!({"seeds":[1,-1],"data":{"kind":"raw","bytes":[]}}),
        json!({"seeds":[1,4294967296u64],"data":{"kind":"raw","bytes":[]}}),
        json!({"seeds":[1,2],"data":{"kind":"raw","bytes":[256]}}),
        json!({"seeds":[1,2],"data":{"kind":"raw","bytes":[],"container":{}}}),
        json!({"seeds":[1,2],"data":{"kind":"raw","bytes":[]},"extra":1}),
        json!({"seeds":[1,2],"data":{"kind":"container","container":{"errors":[],"signals":[],"positional":[],"keyed":[],"extra":1}}}),
        json!({"seeds":[1,2],"data":{"kind":"container","container":{"errors":[],"signals":[],"positional":[{"encoded":[],"extra":[],"ignored":1}],"keyed":[]}}}),
    ] {
        assert!(serde_json::from_value::<Payload>(value).is_err());
    }
    assert!(serde_json::from_str::<Payload>(
        r#"{"seeds":[1,2],"seeds":[3,4],"data":{"kind":"raw","bytes":[]}}"#
    )
    .is_err());
}

#[test]
fn canonical_base64_only_including_equals_as_data() {
    let codec = generator();
    let value = codec.encode_bytes(&[], [0, 0]).unwrap();
    for invalid in [
        format!(" {value}"),
        format!("{value}\n"),
        "!".into(),
        "A".into(),
    ] {
        assert!(codec.decode(&invalid).is_err(), "{invalid:?}");
    }
    for len in 0..8 {
        let short = base64_encode(&vec![0; len], &codec.profile().alphabet).unwrap();
        assert!(codec
            .decode(&short)
            .unwrap_err()
            .to_string()
            .contains("8-byte seed prefix"));
    }
    let mut p = profile();
    p.alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=".into();
    let standard = Generator::new(p).unwrap();
    let value = standard.encode_bytes(&[], [0, 0]).unwrap();
    assert_eq!(value, "AAAAAAAAAAA");
    for bad in [
        "AAAAAAAAAAB",
        "AAAAAAAAAAA=",
        "AAAA=AAAAAAA",
        "AAAA AAA AAA",
    ] {
        assert!(standard.decode(bad).is_err(), "{bad}");
    }
    let f = fixtures().remove(2);
    assert!(f.value.ends_with('=')); // Data, not external padding.
    assert!(Generator::new(f.profile).unwrap().decode(&f.value).is_ok());
}

#[test]
fn header_names_case_insensitive_duplicates_and_other_families_refused() {
    let codec = generator();
    let payload = Payload::raw([1, 2], vec![0, 255]);
    let headers = codec.encode_headers(&payload).unwrap();
    assert_eq!(codec.decode_headers(&headers).unwrap(), payload);
    let lower = Headers::new(
        headers.as_parts().unwrap().0.to_ascii_lowercase(),
        headers.as_parts().unwrap().1.to_owned(),
    )
    .unwrap();
    assert_eq!(codec.decode_headers(&lower).unwrap(), payload);
    assert!(codec
        .decode_headers(
            &Headers::new("X-other-a".into(), headers.as_parts().unwrap().1.into()).unwrap()
        )
        .is_err());
    for text in [
        r#"{}"#,
        r#"{"X-fixture-a":"x","X-fixture-a":"y"}"#,
        r#"{"X-fixture-a":"x","x-fixture-A":"y"}"#,
        r#"{"X-fixture-a":"x","X-other-b":"y"}"#,
        r#"{"X-fixture-a":["x"]}"#,
        r#"{"X-fixture-a":"x\r\ny"}"#,
    ] {
        assert!(
            serde_json::from_str::<Headers>(text)
                .map(|h| codec.decode_headers(&h).is_err())
                .unwrap_or(true),
            "{text}"
        );
    }
}

#[test]
fn malformed_framing_and_resource_limits_fail_without_large_allocations() {
    for bytes in [
        vec![],
        vec![0x80],
        vec![100],
        vec![0, 2, 1],
        [vec![0x80; 9], vec![2]].concat(),
        [vec![0, 0], uleb(u64::from(u32::MAX) + 1), vec![0, 0]].concat(),
    ] {
        assert!(Payload::raw([1, 2], bytes).parse_container(0).is_err());
    }
    let payload = Payload::raw([1, 2], vec![0, 0]);
    assert!(payload.parse_container(usize::MAX).is_err());
    // The adjacent legacy function must also reject impossible counts before allocation.
    assert!(unpack_container(&[0, 0], Some(usize::MAX)).is_err());
    let many_keyed = [vec![0, 0], vec![0; 3 * (MAX_CONTAINER_ENTRIES + 1)]].concat();
    assert!(Payload::raw([1, 2], many_keyed).parse_container(0).is_err());
    let too_big = vec![0; MAX_PLAINTEXT_BYTES + 1];
    assert!(generator().encode_bytes(&too_big, [1, 2]).is_err());
    let c = Container {
        signals: too_big,
        ..Container::default()
    };
    assert!(generator()
        .encode(&Payload {
            header_context: None,
            seeds: [1, 2],
            data: Data::Container { container: c }
        })
        .is_err());
}

#[test]
fn wrong_key_returns_unauthenticated_bytes_and_parse_is_separate() {
    let codec = generator();
    let value = codec
        .encode_bytes(&pack_container(&container()), [1, 2])
        .unwrap();
    let mut p = profile();
    p.keys[0] ^= 1;
    let wrong = Generator::new(p).unwrap();
    let decoded = wrong.decode(&value).unwrap();
    assert!(matches!(&decoded.data, Data::Raw { .. }));
    assert_ne!(decoded.plaintext().unwrap(), pack_container(&container()));
    assert!(decoded.parse_container(3).is_err());
    assert_eq!(wrong.encode(&decoded).unwrap(), value);
}
