//! Synthetic session/bundle tests. Private browser captures are never fixtures.
use serde_json::{json, Value};
use shape::signals::payload::{header_checksum, Generator, HeaderContext, Headers, Payload, Profile};

fn profile() -> Profile {
    serde_json::from_str(include_str!("fixtures/sdk/generator/profile.json")).unwrap()
}

fn context() -> HeaderContext {
    HeaderContext {
        b: true,
        f: true,
        c: Some("opaque build".into()),
        d: Some("opaque-descriptor==".into()),
        z: Some("q".into()),
    }
}

fn codec() -> Generator {
    let mut p = profile();
    p.transaction_token = Some("synthetic-transaction-token".into());
    p.chunk_size = Some(7);
    Generator::new(p).unwrap()
}

fn payload() -> Payload {
    let mut payload = Payload::raw([u32::MAX, 0x10203040], (0..256).map(|i| i as u8).collect());
    payload.header_context = Some(context());
    payload
}

fn map(headers: &Headers) -> Value {
    serde_json::to_value(headers).unwrap()
}
fn headers(map: &Value) -> Headers {
    serde_json::from_value(map.clone()).unwrap()
}

#[test]
fn checksum_matches_independent_javascript_literals_including_signed_utf16() {
    // Literal results from the runtime's UTF-16-pair wrapping hash and JS
    // Number.toString(36), calculated independently of the Rust helper.
    assert_eq!(header_checksum("", ""), "0");
    assert_eq!(header_checksum("token", "payload"), "fov8fo");
    assert_eq!(header_checksum("token😀", "binary"), "-1v9we7");
    assert_eq!(header_checksum("negative-checksum-", "example"), "-y5sc26");
    assert_eq!(header_checksum("a", "bc"), "3d6y8u");
}

#[test]
fn numeric_chunks_over_ten_and_session_context_roundtrip() {
    let codec = codec();
    let original = payload();
    let bundle = codec.encode_headers(&original).unwrap();
    assert!(bundle.get("x-example-a10").is_some());
    assert_eq!(
        bundle.get("x-example-f"),
        Some("synthetic-transaction-token")
    );
    let full = codec.encode(&original).unwrap();
    assert_eq!(
        bundle.get("x-example-b"),
        Some(header_checksum("synthetic-transaction-token", &full).as_str())
    );
    assert_eq!(codec.decode_headers(&bundle).unwrap(), original);
    let decoded: Payload = serde_json::from_value(
        serde_json::to_value(codec.decode_headers(&bundle).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(map(&codec.encode_headers(&decoded).unwrap()), map(&bundle));
    // Lexical map iteration puts a10 before a2. Decoder must join numerically.
    let upper = Headers::from_entries(
        bundle
            .iter()
            .map(|(n, v)| (n.to_ascii_uppercase(), v.to_owned())),
    )
    .unwrap();
    assert_eq!(codec.decode_headers(&upper).unwrap(), original);
    assert_eq!(
        map(&codec
            .encode_headers(&codec.decode_headers(&upper).unwrap())
            .unwrap()),
        map(&bundle)
    );
    // New opaque bytes reuse the same session/context, but recompute b and a.
    let mut changed = original;
    changed.data = shape::signals::payload::Data::Raw {
        bytes: vec![255, 0, 128],
    };
    changed.seeds = [5, 6];
    let next = codec.encode_headers(&changed).unwrap();
    assert_ne!(next.get("x-example-b"), bundle.get("x-example-b"));
    assert_eq!(next.get("x-example-d"), bundle.get("x-example-d"));
    assert_eq!(codec.decode_headers(&next).unwrap(), changed);
}

#[test]
fn missing_chunks_holes_aliases_bad_suffixes_and_lengths_rejected() {
    let codec = codec();
    let good = map(&codec.encode_headers(&payload()).unwrap());
    for missing in [
        "X-example-a",
        "X-example-a0",
        "X-example-a2",
        "X-example-a10",
    ] {
        let mut bad = good.clone();
        bad.as_object_mut().unwrap().remove(missing);
        assert!(codec.decode_headers(&headers(&bad)).is_err(), "{missing}");
    }
    let mut missing_tail = good.clone();
    let tail = good
        .as_object()
        .unwrap()
        .keys()
        .filter_map(|name| {
            name.strip_prefix("X-example-a")
                .filter(|s| !s.is_empty())
                .and_then(|s| s.parse::<usize>().ok())
                .map(|index| (index, name))
        })
        .max_by_key(|(index, _)| *index)
        .unwrap()
        .1;
    missing_tail.as_object_mut().unwrap().remove(tail);
    assert!(codec.decode_headers(&headers(&missing_tail)).is_err());
    for suffix in [
        "a00",
        "a01",
        "a-1",
        "a+1",
        "a1x",
        "a999999999999999999999999",
        "b0",
        "e",
        "nonce",
    ] {
        let mut bad = good.clone();
        bad[format!("X-example-{suffix}")] = json!("x");
        assert!(codec.decode_headers(&headers(&bad)).is_err(), "{suffix}");
    }
    for value in ["", "short", "toolongchunk"] {
        let mut bad = good.clone();
        bad["X-example-a0"] = json!(value);
        assert!(codec.decode_headers(&headers(&bad)).is_err());
    }
    let mut bad = good.clone();
    bad["x-other-a0"] = json!("x");
    assert!(codec.decode_headers(&headers(&bad)).is_err());
    let mut p = codec.profile().clone();
    p.chunk_size = None;
    assert!(Generator::new(p)
        .unwrap()
        .decode_headers(&headers(&good))
        .is_err());
    for text in [
        r#"{"X-example-a":"abc","x-example-A":"abc"}"#,
        r#"{"X-example-a0":"abc","X-example-A0":"abc"}"#,
        r#"{"X-example-c":"x","x-example-C":"y"}"#,
    ] {
        assert!(serde_json::from_str::<Headers>(text).is_err());
    }
}

#[test]
fn checksum_token_and_context_conflicts_fail_explicitly() {
    let codec = codec();
    let good = map(&codec.encode_headers(&payload()).unwrap());
    for (name, value) in [("X-example-b", "wrong"), ("X-example-f", "wrong-token")] {
        let mut bad = good.clone();
        bad[name] = json!(value);
        assert!(codec.decode_headers(&headers(&bad)).is_err());
    }
    let mut p = codec.profile().clone();
    p.transaction_token = None;
    let no_session = Generator::new(p).unwrap();
    assert!(no_session.decode_headers(&headers(&good)).is_err());
    assert!(no_session.encode_headers(&payload()).is_err());
    let mut p = codec.profile().clone();
    p.header_context = Some(context());
    let explicit = Generator::new(p).unwrap();
    assert_eq!(
        map(&explicit
            .encode_headers(&Payload::raw(
                payload().seeds,
                payload().plaintext().unwrap()
            ))
            .unwrap()),
        good
    );
    for key in [
        "X-example-b",
        "X-example-f",
        "X-example-c",
        "X-example-d",
        "X-example-z",
    ] {
        let mut bad = good.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(explicit.decode_headers(&headers(&bad)).is_err(), "{key}");
    }
    let mut bad = payload();
    bad.header_context.as_mut().unwrap().d = Some("different".into());
    assert!(explicit.encode_headers(&bad).is_err());
    assert!(explicit.encode(&bad).is_err());
    let mut bad = good;
    bad["X-example-d"] = json!("different");
    assert!(explicit.decode_headers(&headers(&bad)).is_err());
}

#[test]
fn absent_siblings_are_not_fabricated_even_with_supplied_token() {
    let codec = codec();
    let raw = Payload::raw([1, 2], vec![]);
    let bundle = codec.encode_headers(&raw).unwrap();
    assert!(bundle
        .iter()
        .all(|(name, _)| name.to_ascii_lowercase().contains("-a")));
    assert_eq!(codec.decode_headers(&bundle).unwrap(), raw);
    for context in [
        HeaderContext {
            c: Some("opaque".into()),
            ..HeaderContext::default()
        },
        HeaderContext {
            b: true,
            ..HeaderContext::default()
        },
        HeaderContext {
            f: true,
            ..HeaderContext::default()
        },
    ] {
        let mut input = raw.clone();
        input.header_context = Some(context);
        let encoded = codec.encode_headers(&input).unwrap();
        assert_eq!(codec.decode_headers(&encoded).unwrap(), input);
        assert_eq!(
            map(&codec
                .encode_headers(&codec.decode_headers(&encoded).unwrap())
                .unwrap()),
            map(&encoded)
        );
    }
    let bare = Generator::new(profile()).unwrap();
    assert_eq!(
        bare.encode_bytes(&[0, 255], [1, 2]).unwrap(),
        codec.encode_bytes(&[0, 255], [1, 2]).unwrap()
    );
}

#[test]
fn profile_and_context_strictness_and_bounds() {
    for size in [0, shape::signals::payload::MAX_HEADER_BYTES + 1] {
        let mut p = profile();
        p.chunk_size = Some(size);
        assert!(Generator::new(p).is_err());
    }
    for text in [
        r#"{"c":"x","C":"alias"}"#,
        r#"{"c":"x","c":"y"}"#,
        r#"{"a":"passthrough"}"#,
        r#"{"b":"stale-checksum"}"#,
        r#"{"script_nonce":"not a payload seed"}"#,
    ] {
        assert!(serde_json::from_str::<HeaderContext>(text).is_err());
    }
    for value in ["bad\r\nheader".to_owned(), "x".repeat(65537)] {
        let mut p = profile();
        p.header_context = Some(HeaderContext {
            c: Some(value),
            ..HeaderContext::default()
        });
        assert!(Generator::new(p).is_err());
    }
    let mut p = profile();
    p.header_context = Some(context());
    assert!(Generator::new(p.clone()).is_err()); // b/f require a token
    p.transaction_token = Some("bad\nheader".into());
    assert!(Generator::new(p).is_err());
    let mut p = profile();
    p.chunk_size = Some(1);
    let codec = Generator::new(p).unwrap();
    assert!(codec
        .encode_headers(&Payload::raw([1, 2], vec![0; 50_000]))
        .is_err());
    assert!(Headers::from_entries((0..65_537).map(|i| (format!("x-{i}"), String::new()))).is_err());
    let big = "x".repeat(shape::signals::payload::MAX_HEADER_BYTES);
    assert!(Headers::from_entries([("x-a".into(), big.clone()), ("x-a0".into(), big)]).is_err());
}

#[test]
fn maximum_chunk_count_is_accepted_and_reencodable_without_phantom_siblings() {
    let mut p = profile();
    p.chunk_size = Some(1);
    let codec = Generator::new(p).unwrap();
    // 49,152 prefix+cipher bytes become exactly 65,536 unpadded base64 symbols.
    let input = Payload::raw([1, 2], vec![0; 49_152 - 8]);
    let bundle = codec.encode_headers(&input).unwrap();
    assert_eq!(bundle.iter().count(), 65_536);
    let decoded = codec.decode_headers(&bundle).unwrap();
    assert_eq!(decoded, input);
    assert_eq!(codec.encode_headers(&decoded).unwrap(), bundle);
}

#[test]
fn chunk_boundary_and_canonical_base64_remain_strict() {
    for size in [1, 2, 3, 7, 16, 7900] {
        let mut p = profile();
        p.chunk_size = Some(size);
        let codec = Generator::new(p).unwrap();
        for len in [0, 1, 2, 3, 63, 64, 65, 7900] {
            let input = Payload::raw([3, 4], vec![255; len]);
            let encoded = codec.encode_headers(&input).unwrap();
            assert_eq!(codec.decode_headers(&encoded).unwrap(), input);
        }
    }
    let mut p = profile();
    p.chunk_size = Some(4);
    let codec = Generator::new(p).unwrap();
    let good = map(&codec.encode_headers(&Payload::raw([0, 0], vec![])).unwrap());
    for value in ["A A", "AA=", "AAB"] {
        let mut bad = good.clone();
        bad["X-example-a1"] = json!(value);
        assert!(codec.decode_headers(&headers(&bad)).is_err());
    }
}
