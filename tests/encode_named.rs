//! Named-signal encoding against a real capture (opt-in, gitignored input).
//!
//! Run with a capture whose saved artifact and header bundle are local:
//!
//! ```sh
//! SHAPE_TEST_ARTIFACT=reports/pasted-capture/artifact.json \
//! SHAPE_TEST_HEADERS=reports/pasted-capture/headers.json \
//! cargo test --test encode_named
//! ```
//!
//! The encoded header bundle must equal the captured bundle byte for byte,
//! including every signal on its own.
use serde_json::Value;
use shape::signals::decode::{CaptureHeaders, DecodeOptions, DecodedPayload, ValidatedArtifact};
use shape::signals::wire::WireRoute;
use std::fs;

const FULL: DecodeOptions = DecodeOptions {
    verify: false,
    metadata: true,
};

fn fixture(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|path| !path.is_empty())
}

fn artifact_and_capture() -> Option<(ValidatedArtifact, CaptureHeaders)> {
    let artifact = fixture("SHAPE_TEST_ARTIFACT")?;
    let headers = fixture("SHAPE_TEST_HEADERS")?;
    let artifact = ValidatedArtifact::from_json(&fs::read(artifact).ok()?).ok()?;
    let capture: CaptureHeaders = serde_json::from_slice(&fs::read(headers).ok()?).ok()?;
    Some((artifact, capture))
}

fn positional(artifact: &ValidatedArtifact, signal: &str) -> usize {
    artifact
        .artifact()
        .schema
        .wire
        .as_ref()
        .unwrap()
        .slots
        .iter()
        .find_map(|slot| match (&slot.naming, &slot.routing) {
            (Some(naming), WireRoute::Positional { positional }) if naming.schema_key == signal => {
                Some(*positional)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no positional slot for {signal}"))
}

fn headers_equal(encoded: &CaptureHeaders, capture: &CaptureHeaders) {
    assert_eq!(encoded.site_id, capture.site_id);
    assert_eq!(
        encoded.headers.iter().collect::<Vec<_>>(),
        capture.headers.iter().collect::<Vec<_>>()
    );
}

fn single(payload: &DecodedPayload, signal: &str) -> Value {
    let mut one = payload.clone();
    one.positional_signals = one
        .positional_signals
        .iter()
        .filter(|(name, _)| name.as_str() == signal)
        .map(|(name, fields)| (name.clone(), fields.clone()))
        .collect();
    serde_json::to_value(&one).unwrap()
}

#[test]
fn capture_round_trips_byte_identically_and_per_signal() {
    let Some((artifact, capture)) = artifact_and_capture() else {
        return;
    };
    shape::run_with_large_stack(move || {
        let decoded = artifact.decode(&capture, None, FULL).unwrap();
        let payload = serde_json::to_value(&decoded).unwrap();
        let encoded = artifact.encode_named(&payload, None).unwrap();
        headers_equal(&encoded, &capture);
        assert_eq!(
            artifact
                .decode(&encoded, None, FULL)
                .unwrap()
                .positional_signals,
            decoded.positional_signals
        );

        // Lazy decode (no per-signal metadata) keeps the original report and
        // encodes byte-identically too.
        let lazy = artifact
            .decode(
                &capture,
                None,
                DecodeOptions {
                    verify: false,
                    metadata: false,
                },
            )
            .unwrap();
        assert!(lazy.metadata.signals.is_empty());
        let lazy_encoded = artifact
            .encode_named(&serde_json::to_value(&lazy).unwrap(), None)
            .unwrap();
        headers_equal(&lazy_encoded, &capture);

        for (name, fields) in &decoded.positional_signals {
            let value = single(&decoded, name);
            let one = artifact.encode_named(&value, None).unwrap();
            let decoded_one = artifact.decode(&one, None, FULL).unwrap();
            assert_eq!(
                decoded_one.positional_signals.get(name),
                Some(fields),
                "{name} did not project back"
            );
            let position = positional(&artifact, name);
            assert_eq!(
                decoded_one.metadata.original_report.positional_signals[position]
                    .as_ref()
                    .map(|entry| entry.1.clone()),
                decoded.metadata.original_report.positional_signals[position]
                    .as_ref()
                    .map(|entry| entry.1.clone()),
                "{name} did not reproduce its bytes"
            );
        }
    });
}

#[test]
fn bare_fields_encode_and_decode_back() {
    let Some((artifact, capture)) = artifact_and_capture() else {
        return;
    };
    shape::run_with_large_stack(move || {
        let decoded = artifact.decode(&capture, None, FULL).unwrap();
        let mut bare = serde_json::to_value(&decoded).unwrap();
        bare.as_object_mut().unwrap().remove("metadata");
        bare.as_object_mut().unwrap().remove("siteId");
        let encoded = artifact.encode_named(&bare, None).unwrap();
        let decoded_again = artifact.decode(&encoded, None, FULL).unwrap();
        // The per-signal prefixes are fresh, so only the named fields must
        // agree; unprojected bytes (empty flag maps) are zero-filled.
        assert_eq!(decoded_again.positional_signals, decoded.positional_signals);
    });
}

#[test]
fn edited_field_changes_only_its_signal() {
    let Some((artifact, capture)) = artifact_and_capture() else {
        return;
    };
    shape::run_with_large_stack(move || {
        let decoded = artifact.decode(&capture, None, FULL).unwrap();
        let Some((name, fields)) = decoded
            .positional_signals
            .iter()
            .find(|(_, fields)| fields.values().any(|value| value.is_number()))
        else {
            return;
        };
        let (field, value) = fields
            .iter()
            .find(|(_, value)| value.is_number())
            .map(|(field, value)| (field.clone(), value.as_i64().unwrap()))
            .unwrap();
        let mut edited = decoded.clone();
        edited
            .positional_signals
            .get_mut(name)
            .unwrap()
            .insert(field.clone(), Value::from(value + 1));
        let payload = serde_json::to_value(&edited).unwrap();
        let encoded = artifact.encode_named(&payload, None).unwrap();
        let decoded_again = artifact.decode(&encoded, None, FULL).unwrap();
        assert_eq!(
            decoded_again.positional_signals[name][&field],
            Value::from(value + 1),
            "edit did not survive decoding"
        );
        // Only this signal's bytes changed.
        let position = positional(&artifact, name);
        for slot in artifact
            .artifact()
            .schema
            .wire
            .as_ref()
            .unwrap()
            .slots
            .iter()
        {
            let WireRoute::Positional { positional } = slot.routing else {
                continue;
            };
            let before = decoded.metadata.original_report.positional_signals[positional]
                .as_ref()
                .map(|entry| entry.1.clone());
            let after = decoded_again.metadata.original_report.positional_signals[positional]
                .as_ref()
                .map(|entry| entry.1.clone());
            if positional == position {
                assert_ne!(before, after, "the edited signal is unchanged");
            } else {
                assert_eq!(before, after, "an unrelated signal changed");
            }
        }
    });
}
