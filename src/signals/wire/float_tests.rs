use super::*;
use crate::signals::schema::SourceSpan;
use crate::signals::wire::{
    program::{Op, Program, WireMask, WireValue},
    AlgorithmProof,
};

fn metadata(source: &str) -> NumberFloatMetadata {
    let start = source.find("function local0(").unwrap() as u32;
    let end = start + include_str!("reference/number.js").trim_end().len() as u32;
    certify(
        source,
        &AlgorithmProof {
            codec: None,
            span: Some(SourceSpan::new(start, end)),
            sha256: None,
            reason: None,
        },
    )
}
fn reference() -> &'static str {
    include_str!("reference/number-native.js")
}
fn format(order: FloatByteOrder) -> Result<FloatByteOrder, WireError> {
    metadata(reference()).format(TypedDecoderOptions {
        float_byte_order: order,
    })
}
fn program(mask: WireMask) -> Program {
    Program {
        prefix_byte_order: "be32".into(),
        seed_source: "captured_prefix".into(),
        stream_scope: "emission_shared_cursor".into(),
        string_scope: "emission_local_writer_instance".into(),
        salt: 0,
        body: Op::Number {
            envelope: None,
            mask,
            literal: None,
        },
    }
}
fn golden(order: FloatByteOrder, body: [u8; 8]) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0, 128];
    match order {
        FloatByteOrder::Little => bytes.extend(body.into_iter().rev()),
        FloatByteOrder::Big => bytes.extend(body),
        FloatByteOrder::Unknown => unreachable!(),
    }
    bytes
}

#[test]
fn independent_ieee_vectors_both_orders_and_exact_entire_blob() {
    // IEEE hexadecimal known answers, not bytes manufactured by our encoder.
    let vectors = [
        (0x3ff8000000000000, [0x3f, 0xf8, 0, 0, 0, 0, 0, 0]), // 1.5
        (0x41e0000000000000, [0x41, 0xe0, 0, 0, 0, 0, 0, 0]), // 2^31, outside i32
        (0x0000000000000001, [0, 0, 0, 0, 0, 0, 0, 1]),       // minimum subnormal
        (0x8000000000000001, [0x80, 0, 0, 0, 0, 0, 0, 1]),
        (0x7ff0000000000000, [0x7f, 0xf0, 0, 0, 0, 0, 0, 0]),
        (0xfff0000000000000, [0xff, 0xf0, 0, 0, 0, 0, 0, 0]),
        (0x7ff8000000000021, [0x7f, 0xf8, 0, 0, 0, 0, 0, 0x21]),
        (0xfff0000000000021, [0xff, 0xf0, 0, 0, 0, 0, 0, 0x21]), // raw signaling NaN policy
    ];
    for order in [FloatByteOrder::Little, FloatByteOrder::Big] {
        let format = format(order);
        assert_eq!(format, Ok(order));
        for (bits, bytes) in vectors {
            let golden = golden(order, bytes);
            // Zero PRNG state has zero masks; still exercises the stream API.
            let p = program(WireMask::Inherit);
            let value = p.decode_with_float(&golden, 0, &format).unwrap();
            assert_eq!(value, WireValue::NumberBits(bits));
            assert_eq!(p.encode_with_float(&value, 0, 0, &format).unwrap(), golden);
        }
    }
}

#[test]
fn refuses_unknown_order_and_unknown_primitive_separately() {
    let bytes = golden(FloatByteOrder::Big, [0x3f, 0xf8, 0, 0, 0, 0, 0, 0]);
    let p = program(WireMask::Inherit);
    let unknown = format(FloatByteOrder::Unknown);
    assert!(matches!(&unknown, Err(WireError::Unsupported(s)) if s.contains("runtime byte order")));
    assert_eq!(
        p.decode_with_float(&bytes, 0, &unknown),
        unknown.map(|_| unreachable!())
    );
    let no_dependencies = metadata(include_str!("reference/number.js"));
    assert!(no_dependencies.writer.codec.is_some());
    assert!(
        matches!(no_dependencies.format(TypedDecoderOptions { float_byte_order: FloatByteOrder::Little }), Err(WireError::Unsupported(s)) if s.contains("source float primitive"))
    );
    assert_eq!(
        p.decode_with_float(&[0, 0, 0, 0, 64], 0, &format(FloatByteOrder::Unknown))
            .unwrap(),
        WireValue::NumberBits((-0f64).to_bits())
    );
    assert_eq!(
        p.encode_with_float(
            &WireValue::NumberBits((-0f64).to_bits()),
            0,
            0,
            &format(FloatByteOrder::Unknown)
        )
        .unwrap(),
        [0, 0, 0, 0, 64]
    );
}

#[test]
fn refuses_noncanonical_integer_floats_truncation_and_trailing_bytes() {
    let p = program(WireMask::Inherit);
    for order in [FloatByteOrder::Little, FloatByteOrder::Big] {
        let f = format(order);
        for body in [
            [0; 8],
            [0x80, 0, 0, 0, 0, 0, 0, 0],
            [0x3f, 0xf0, 0, 0, 0, 0, 0, 0],
            [0x41, 0xdf, 0xff, 0xff, 0xff, 0xc0, 0, 0],
        ] {
            assert_eq!(
                p.decode_with_float(&golden(order, body), 0, &f),
                Err(WireError::Codec)
            );
        }
        let bytes = golden(order, [0x3f, 0xf8, 0, 0, 0, 0, 0, 0]);
        for end in 0..bytes.len() {
            assert_eq!(
                p.decode_with_float(&bytes[..end], 0, &f),
                Err(WireError::Truncated)
            );
        }
        let mut extra = bytes;
        extra.push(0);
        assert_eq!(
            p.decode_with_float(&extra, 0, &f),
            Err(WireError::Trailing(1))
        );
        assert_eq!(
            p.decode_with_float(&[0, 0, 0, 0, 129], 0, &f),
            Err(WireError::Codec)
        );
    }
}

#[test]
fn whole_helper_and_dependency_mutants_cannot_enable_float() {
    let good = metadata(reference());
    assert!(good.writer.codec.is_some());
    assert!(good.native_dependencies.codec.is_some());
    for (from, to) in [
        ("j < 8", "j < 7"),
        ("local9[j]", "local9[7-j]"),
        ("Float64Array(1)", "Float32Array(1)"),
        ("globalThis.Uint8Array", "globalThis.Int8Array"),
        ("local10.buffer", "local2.buffer"),
        ("local10[0] = local2", "local10[0] = -local2"),
        ("return local0", "return anotherWriter"),
        (
            "local9 = new local11(local10.buffer)",
            "local9 = new local11(8)",
        ),
        ("globalThis.Float64Array", "other.Float64Array"),
    ] {
        let changed = reference().replace(from, to);
        assert_ne!(changed, reference());
        assert!(
            metadata(&changed)
                .format(TypedDecoderOptions {
                    float_byte_order: FloatByteOrder::Little
                })
                .is_err(),
            "accepted mutation: {from}"
        );
    }
    let shadowed = format!("function outer(globalThis) {{ {} }}", reference());
    assert!(metadata(&shadowed).native_dependencies.codec.is_none());
    // The legacy integer-only certificate deliberately ignores these mutations.
    let mutated = reference().replace("j < 8", "j < 7");
    let m = metadata(&mutated);
    let span = m.writer.span.unwrap();
    assert!(algorithms::proof(
        Some((span, &mutated[span.start as usize..span.end as usize])),
        include_str!("reference/number.js"),
        "integer",
        true
    )
    .codec
    .is_some());
}

#[test]
fn independent_masked_float_and_following_byte_preserve_cursor_and_prefix() {
    // Independent recurrence from A=0, B=0x01020304, one word burned:
    // masks 24 8b 64 45 b4 5b cb 2d 88 0a; plaintext 80 3f f8 00...00 05.
    let golden = vec![
        1, 2, 3, 4, 0xa4, 0xb4, 0x9c, 0x45, 0xb4, 0x5b, 0xcb, 0x2d, 0x88, 0x0f,
    ];
    let mut p = program(WireMask::Inherit);
    p.body = Op::Sequence {
        items: vec![
            p.body,
            Op::Byte {
                mask: WireMask::Inherit,
                xor: 0,
                allowed: Some(vec![5]),
            },
        ],
    };
    let value = WireValue::Sequence(vec![
        WireValue::NumberBits(0x3ff8000000000000),
        WireValue::Byte(5),
    ]);
    let f = format(FloatByteOrder::Big);
    assert_eq!(p.decode_with_float(&golden, 0, &f).unwrap(), value);
    assert_eq!(
        p.encode_with_float(&value, 0x01020304, 0, &f).unwrap(),
        golden
    );
    // Static/unmasked body bytes never burn a mask; first following stream
    // byte uses the first mask (0x24), not the tenth (0x0a).
    p.body = Op::Sequence {
        items: vec![
            Op::Number {
                envelope: None,
                mask: WireMask::None,
                literal: None,
            },
            Op::Byte {
                mask: WireMask::Inherit,
                xor: 7,
                allowed: Some(vec![5]),
            },
        ],
    };
    let unmasked = vec![1, 2, 3, 4, 128, 0x3f, 0xf8, 0, 0, 0, 0, 0, 0, 0x26];
    assert_eq!(p.decode_with_float(&unmasked, 0, &f).unwrap(), value);
    assert_eq!(
        p.encode_with_float(&value, 0x01020304, 0, &f).unwrap(),
        unmasked
    );
}

#[test]
fn low_level_legacy_number_stays_little_endian() {
    let mut w = SignalWriter::new(0, 0, 0);
    w.number(1.5);
    assert_eq!(w.finish(), [0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0xf8, 0x3f]);
    let bytes = [0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0xf8, 0x3f];
    let mut r = SignalReader::new(&bytes, 0, 0).unwrap();
    assert_eq!(r.number().unwrap(), 1.5);
    assert!(r.at_end());
}

fn extracted(source: &str) -> NumberFloatMetadata {
    let encoding = crate::signals::extract::encoding::extract_encoding_properties(source).unwrap();
    let selected = encoding
        .number
        .expect("source extractor must identify the actual writer")
        .function
        .span;
    let text = &source[selected.start as usize..selected.end as usize];
    let selected_writer = algorithms::proof(
        Some((selected, text)),
        include_str!("reference/number.js"),
        "integer",
        true,
    );
    certify(source, &selected_writer)
}
fn module_source() -> String {
    let writer = include_str!("reference/number.js")
        .replace("local10", "scratch")
        .replace("local11", "Bytes")
        .replace("local12", "nanBytes");
    format!("(function module(){{ let scratch, Bytes, nanBytes; const unrelated=42; {writer}\nconst U8=Uint8Array; Bytes=typeof U8==='function'?U8:null; const F64=Float64Array; scratch=Bytes&&typeof F64==='function'?new F64(1):null; nanBytes=scratch?null:[]; const exported=local0; return exported; }})();")
}
#[test]
fn extraction_links_module_captures_with_separate_initialization_and_aliases() {
    let source = module_source();
    let metadata = extracted(&source);
    assert!(
        metadata.writer.codec.is_some(),
        "{:?}",
        metadata.writer.reason
    );
    assert!(
        metadata.native_dependencies.codec.is_some(),
        "{:?}",
        metadata.native_dependencies.reason
    );
    assert!(metadata
        .provenance
        .iter()
        .any(|p| p.role == "scratch_origin"));
    assert!(metadata
        .provenance
        .iter()
        .any(|p| p.role == "selected_writer_binding_link"));
    assert!(
        matches!(metadata.format(TypedDecoderOptions::default()),Err(WireError::Unsupported(s)) if s.contains("runtime byte order"))
    );
    for site in &metadata.provenance {
        assert_eq!(
            site.sha256,
            algorithms::digest(source[site.span.start as usize..site.span.end as usize].as_bytes())
        );
    }
    // Software arithmetic is outside this conditional native certificate.
    // A different fallback must neither invalidate proven native bytes nor
    // acquire a whole-domain software arithmetic certificate.
    let variant = source.replace("Math.abs(local2)", "unknownSoftware(local2)");
    assert_ne!(variant, source);
    let conditional = extracted(&variant);
    assert!(conditional
        .writer
        .codec
        .as_deref()
        .is_some_and(|c| c.contains("conditional")));
    assert!(conditional.native_dependencies.codec.is_some());
    assert_ne!(conditional.writer.sha256, metadata.writer.sha256);
    assert!(conditional
        .fallback_byte_order
        .contains("selected source software fallback is not certified"));
    let recycled = source.replace(
        "const exported=local0;",
        "let recycled=local0; recycled=unrelated; const exported=local0;",
    );
    let independent = extracted(&recycled);
    assert!(independent.native_dependencies.codec.is_some());
    let recycled_start = recycled.find("recycled=local0").unwrap() as u32;
    assert!(!independent
        .provenance
        .iter()
        .any(|s| s.role == "selected_writer_alias"
            && s.span.start <= recycled_start
            && recycled_start < s.span.end));
    let saved = serde_json::to_vec(&metadata).unwrap();
    let loaded: NumberFloatMetadata = serde_json::from_slice(&saved).unwrap();
    assert_eq!(loaded, extracted(&source));
}
#[test]
fn module_capture_counterexamples_refuse_without_borrowing_neighbor_factory() {
    let good = module_source();
    for (from, to) in [
        ("const F64=Float64Array", "const F64=Float32Array"),
        ("const U8=Uint8Array", "const U8=Int16Array"),
        ("const U8=Uint8Array", "const U8=Int8Array"),
        ("new F64(1)", "new F64(0)"),
        ("new F64(1)", "new F64(2)"),
        ("scratch.buffer", "otherScratch.buffer"),
        ("local9[j]", "local9[7-j]"),
        ("return exported", "Bytes=Uint8Array; return exported"),
        ("return exported", "scratch[0]=7; return exported"),
        ("return exported", "unknown(scratch); return exported"),
        ("return exported", "U8=Int16Array; return exported"),
        ("return exported", "Uint8Array=Int16Array; return exported"),
        (
            "return exported",
            "globalThis.Float64Array=Float32Array; return exported",
        ),
        ("return exported", "exported=otherWriter; return exported"),
        ("return exported", "local0=otherWriter; return exported"),
        ("return exported", "[Bytes]=[Int16Array]; return exported"),
        ("return exported", "({x:Bytes}=unknown); return exported"),
        ("typeof F64==='function'", "runtimeFlag"),
        ("new F64(1)", "allocateFloat()"),
        ("scratch[0] = local2", "scratch[0] = Number(local2)"),
    ] {
        let changed = good.replace(from, to);
        assert_ne!(changed, good);
        let metadata = extracted(&changed);
        assert!(
            metadata.native_dependencies.codec.is_none(),
            "accepted {from} -> {to}"
        );
    }
    let decoy = format!(
        "{}\n{}",
        good.replace("const U8=Uint8Array", "const U8=Int16Array"),
        include_str!("reference/number-native.js")
    );
    // Select the first writer's actual source span; a separate compatible
    // factory is irrelevant to its concrete captured symbols.
    let start = decoy.find("function local0(").unwrap() as u32;
    let end = start
        + include_str!("reference/number.js")
            .replace("local10", "scratch")
            .replace("local11", "Bytes")
            .replace("local12", "nanBytes")
            .len() as u32;
    let metadata = certify(
        &decoy,
        &AlgorithmProof {
            span: Some(SourceSpan::new(start, end)),
            codec: None,
            sha256: None,
            reason: None,
        },
    );
    assert!(metadata.native_dependencies.codec.is_none());
}
#[test]
fn extracted_returned_and_assigned_literals_retain_their_own_captures() {
    let writer = include_str!("reference/number.js");
    for binding in [
        format!("{writer}\nconst alias=local0; return alias;"),
        format!(
            "const held={}; return held;",
            writer.replacen("function local0", "function", 1)
        ),
        format!(
            "let held; held={}; return held;",
            writer.replacen("function local0", "function named", 1)
        ),
        format!(
            "return {};",
            writer.replacen("function local0", "function", 1)
        ),
    ] {
        let source=format!("function factory(){{ const local10=new Float64Array(1); const C=Uint8Array; const local11=C; const local12=[]; const extra=19; {binding} }} const writer=factory();");
        let metadata = extracted(&source);
        assert!(
            metadata.writer.codec.is_some(),
            "{:?}",
            metadata.writer.reason
        );
        assert!(
            metadata.native_dependencies.codec.is_some(),
            "{:?}",
            metadata.native_dependencies.reason
        );
    }
}

#[test]
fn public_schema_emission_and_load_revalidate_relational_float_certificate() {
    use crate::signals::{
        analysis::analyze_program_signals,
        payload::{extract::Session, pin, Report},
        wire::{Decoder, Outcome, SchemaDocument, WireValue},
    };
    let source = include_str!("reference/float-module.js")
        .replace(
            "__HASH__",
            &include_str!("reference/hash.js").replace("local0(", "tokenHash("),
        )
        .replace(
            "__STREAM__",
            &include_str!("reference/stream.js")
                .replace("local0(", "streamFactory(")
                .replace("local9", "tokenState"),
        )
        .replace(
            "__NUMBER__",
            &include_str!("reference/number.js").replace("local0(", "numberWriter("),
        );
    let session = Session {
        transaction_token: "ab".into(),
        alphabet: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".into(),
        shuffle_key: vec![],
        keys: [0; 8],
        nonce: None,
    };
    let pinned = pin(&source, &session).unwrap().layout;
    let analysis = analyze_program_signals(
        &source,
        &crate::signals::analysis::SignalFinderOptions {
            layouts_only: true,
            finder_roles: true,
        },
    )
    .unwrap();
    let saved = serde_json::to_vec(&analysis.report).unwrap();
    let document = SchemaDocument::from_json(&saved).unwrap();
    let metadata = document
        .wire
        .as_ref()
        .unwrap()
        .number_float
        .as_ref()
        .unwrap();
    assert!(
        metadata.writer.codec.is_some(),
        "{:?}",
        metadata.writer.reason
    );
    assert!(
        metadata.native_dependencies.codec.is_some(),
        "{:?}",
        metadata.native_dependencies.reason
    );
    assert_eq!(metadata, &extracted(&source));
    let options = TypedDecoderOptions {
        float_byte_order: FloatByteOrder::Big,
    };
    let configured = Decoder::load_with_options(&saved, &source, &pinned, options).unwrap();
    let default = Decoder::load(&saved, &source, &pinned).unwrap();
    // Seed 0x01020304, paired UTF-16 hash of "ab", salt 17; known recurrence
    // masks computed independently of SignalWriter. Plaintext is BE 1.5.
    let golden = vec![
        1, 2, 3, 4, 0x22, 0x76, 0x2b, 0xaf, 0xe0, 0xf2, 0x50, 0x34, 0xee,
    ];
    let report = Report {
        errors: vec![],
        positional_signals: vec![Some((vec![], golden.clone())), None],
        signals: vec![],
        keyed_signals: vec![],
        seeds: [0, 0],
        keysets: vec![],
    };
    assert!(
        matches!(default.decode_report(&report).unwrap()[0].outcome,Outcome::Unsupported{reason:WireError::Unsupported(ref s)} if s.contains("runtime byte order"))
    );
    let decoded = configured.decode_report(&report).unwrap();
    let value = WireValue::Sequence(vec![WireValue::NumberBits(0x3ff8000000000000)]);
    assert_eq!(
        decoded[0].outcome,
        Outcome::Decoded {
            value: value.clone(),
            exact_reencode: true
        }
    );
    assert_eq!(
        configured.encode_slot(0, &value, 0x01020304).unwrap(),
        golden
    );
    let mut forged = serde_json::to_value(&document).unwrap();
    forged["wire"]["number_float"]["provenance"][0]["sha256"] = serde_json::json!("forged");
    assert!(matches!(
        Decoder::load_with_options(
            &serde_json::to_vec(&forged).unwrap(),
            &source,
            &pinned,
            options
        ),
        Err(WireError::Evidence)
    ));
}

#[test]
fn conditional_try_allocation_requires_the_native_runtime_path() {
    let source = module_source().replace(
        "scratch=Bytes&&typeof F64==='function'?new F64(1):null;",
        "try { scratch=new F64(1); } catch(error) { scratch=null; }",
    );
    let metadata = extracted(&source);
    assert!(
        metadata.native_dependencies.codec.is_some(),
        "{:?}",
        metadata.native_dependencies.reason
    );
    assert!(metadata
        .provenance
        .iter()
        .any(|s| s.role == "conditional_native_allocation"));
    assert!(metadata
        .runtime_prerequisites
        .iter()
        .any(|s| s.contains("native branch execution")));
    for replacement in ["scratch=[]", "scratch=new F64(2)", "scratch=otherScratch"] {
        let changed = source.replace("scratch=null", replacement);
        assert!(extracted(&changed).native_dependencies.codec.is_none());
    }
}
