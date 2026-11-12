//! Caller-supplied values through generic serialization, outer framing and headers.
use shape::signals::payload::{
    superpack::{
        pack, unpack, unpack_canonical, Number, SerializerOptions, Value, DEPTH_EXTENSION,
    },
    Container, Data, Generator, HeaderContext, KeyedEntry, Payload, PositionalEntry, Profile,
};

#[test]
fn generic_values_to_session_chunks_and_back_preserve_all_channel_bytes() {
    let mut profile: Profile =
        serde_json::from_str(include_str!("fixtures/sdk/generator/profile.json")).unwrap();
    profile.transaction_token = Some("synthetic-pipeline-session".into());
    profile.chunk_size = Some(7);
    let codec = Generator::new(profile).unwrap();
    let defaults = SerializerOptions::default();
    let signals_options = SerializerOptions::outer_channel(vec![vec!["a".into(), "b".into()]]);
    let errors_value = Value::Array(vec![
        Value::Undefined,
        Value::Null,
        Value::String("synthetic error".into()),
    ]);
    let encoded_value = Value::Array(vec![
        Value::Number(Number(0)),
        Value::Number(Number(0x8000000000000000)),
        Value::Number(Number(0x7ff8000000000000)),
        Value::Number(Number(0x7ff0000000000000)),
        Value::Number(Number(0xfff0000000000000)),
        Value::Binary(vec![0, 128, 255]),
    ]);
    let signals_value = Value::Object(vec![
        ("a".into(), Value::MissingSentinel),
        (
            "b".into(),
            Value::Extension {
                id: DEPTH_EXTENSION,
                payload: Box::new(Value::Null),
            },
        ),
    ]);
    let errors = pack(&errors_value, &defaults).unwrap();
    let encoded = pack(&encoded_value, &defaults).unwrap();
    let signals = pack(&signals_value, &signals_options).unwrap();
    // Already-produced raw detector bytes, including a synthetic seed prefix.
    // These bypass superpack entirely and must not acquire another prefix/frame.
    let extra = vec![0x11, 0x22, 0x33, 0x44, 0, 128, 255, 0];
    let keyed_extra = vec![255, 0, 128, 42];
    let container = Container {
        errors: errors.clone(),
        positional: vec![
            Some(PositionalEntry {
                encoded: encoded.clone(),
                extra: extra.clone(),
            }),
            None,
        ],
        signals: signals.clone(),
        keyed: vec![KeyedEntry {
            name: 7,
            encoded: encoded.clone(),
            extra: keyed_extra.clone(),
        }],
    };
    let payload = Payload {
        seeds: [0x11223344, 0x55667788],
        data: Data::Container {
            container: container.clone(),
        },
        header_context: Some(HeaderContext {
            b: true,
            f: true,
            c: Some("caller metadata".into()),
            d: None,
            z: Some("marker".into()),
        }),
    };
    let plaintext = payload.plaintext().unwrap();
    let headers = codec.encode_headers(&payload).unwrap();
    assert!(headers.get("X-example-a10").is_some());
    assert!(headers.get("X-example-b").is_some());
    assert_eq!(
        headers.get("X-example-f"),
        Some("synthetic-pipeline-session")
    );
    let raw = codec.decode_headers(&headers).unwrap();
    assert_eq!(raw.seeds, payload.seeds);
    assert_eq!(raw.header_context, payload.header_context);
    assert_eq!(raw.plaintext().unwrap(), plaintext);
    let parsed = raw.parse_container(2).unwrap();
    assert_eq!(parsed, payload);
    assert_eq!(parsed.plaintext().unwrap(), plaintext);
    let Data::Container {
        container: recovered,
    } = &parsed.data
    else {
        panic!("expected container")
    };
    assert_eq!(recovered, &container);
    assert_eq!(recovered.positional[0].as_ref().unwrap().extra, extra);
    assert_eq!(recovered.keyed[0].extra, keyed_extra);
    for (bytes, value, options) in [
        (&recovered.errors, &errors_value, &defaults),
        (
            &recovered.positional[0].as_ref().unwrap().encoded,
            &encoded_value,
            &defaults,
        ),
        (&recovered.keyed[0].encoded, &encoded_value, &defaults),
        (&recovered.signals, &signals_value, &signals_options),
    ] {
        assert_eq!(&unpack_canonical(bytes, options).unwrap(), value);
        let decoded = unpack(bytes, options).unwrap();
        assert!(decoded.canonical);
        assert_eq!(&decoded.original_bytes, bytes);
        assert_eq!(pack(&decoded.value, options).unwrap(), *bytes);
    }
    assert_eq!(codec.encode_headers(&parsed).unwrap(), headers);
}
