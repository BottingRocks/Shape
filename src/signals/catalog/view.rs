//! Compiled logical catalogue and capture presentation. Documentary examples
//! never supply missing captured values; physical JSON remains in legacy keys.
use super::{card::CardField, Card};
use serde_json::{json, Map, Value};
use std::{collections::BTreeMap, sync::OnceLock};

fn manifest() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("view-data.json")).expect("checked manifest")
    })
}
fn entry(card: &Card) -> &'static Value {
    &manifest()["cards"][crate::signals::docs::page_stem(card.signal)]
}

/// Remove only certified controls, retaining unrecognized and dynamic keys.
/// Null placeholders express missing logical values rather than observed false.
pub fn clean(card: &Card, value: &Value) -> Value {
    let value = if value
        .as_object()
        .is_some_and(|o| o.len() == 1 && o.contains_key(card.signal))
    {
        &value[card.signal]
    } else {
        value
    };
    fn walk(card: &Card, value: &Value, path: &str) -> Value {
        match value {
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|v| walk(card, v, &format!("{path}[]")))
                    .collect(),
            ),
            Value::Object(object) => {
                if !object.is_empty()
                    && object
                        .keys()
                        .all(|k| matches!(k.as_str(), "nullMarker" | "nullFlag"))
                {
                    return Value::Null;
                }
                if let Some(special) = object
                    .get("special")
                    .filter(|_| object.len() == 2 && object.contains_key("numberBits"))
                {
                    let special = match special.as_str() {
                        Some("nan") => json!("NaN"),
                        Some("negative_zero") => json!("-0"),
                        Some("negative_infinity") => json!("-Infinity"),
                        Some("positive_infinity") => json!("Infinity"),
                        _ => special.clone(),
                    };
                    return json!({"$special": special});
                }
                let mut out = Map::new();
                for (key, v) in object {
                    let field_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    let forbidden = manifest()["forbidden"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|x| x == key);
                    let control = entry(card)["controls"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|x| x == &field_path);
                    if forbidden
                        || control
                        || (card.signal == "MatchMedia"
                            && card.all_fields().iter().any(|f| {
                                f.name == key
                                    && (f.name.ends_with("Count")
                                        || f.meaning.contains("never written"))
                            }))
                    {
                        continue;
                    }
                    let alias = match (card.signal, key.as_str()) {
                        ("UserAgentData", "brandEntries") => "brands",
                        ("UserAgentData", "fullVersionEntries") => "fullVersionList",
                        _ => key,
                    };
                    out.insert(alias.to_string(), walk(card, v, &field_path));
                }
                if card.signal == "UserAgentData" && path.is_empty() {
                    for (placeholder, flag) in [
                        ("wow64Placeholder", "wow64"),
                        ("mobilePlaceholder", "mobile"),
                    ] {
                        if object.contains_key(placeholder) {
                            let flags = out.entry("flags").or_insert_with(|| json!({}));
                            if let Some(flags) = flags.as_object_mut() {
                                flags.insert(flag.into(), Value::Null);
                            }
                        }
                    }
                }
                if card.signal == "VisibilityEventLog" && object.contains_key("stateNullTag") {
                    let state = out.entry("state").or_insert_with(|| json!({}));
                    if let Some(state) = state.as_object_mut() {
                        state.insert("currentlyVisible".into(), Value::Null);
                    }
                }
                if card.signal == "WebGlContext" && object.contains_key("antialiasMissing") {
                    let flags = out.entry("flags").or_insert_with(|| json!({}));
                    if let Some(flags) = flags.as_object_mut() {
                        flags.insert("antialias".into(), Value::Null);
                    }
                }
                if card.signal == "WebGlContext" && path.is_empty() {
                    // Card-certified absent groups, never documentary values.
                    const PARAMETERS: &[&str] = &[
                        "version",
                        "renderer",
                        "alphaBits",
                        "greenBits",
                        "maxRenderbufferSize",
                        "maxCubeMapTextureSize",
                        "stencilBits",
                        "shadingLanguageVersion",
                        "maxTextureSize",
                        "vendor",
                        "maxVertexUniformVectors",
                        "maxTextureImageUnits",
                        "depthBits",
                        "blueBits",
                        "maxVaryingVectors",
                        "maxVertexTextureImageUnits",
                        "redBits",
                        "maxVertexAttribs",
                        "maxFragmentUniformVectors",
                        "maxCombinedTextureImageUnits",
                    ];
                    if object.contains_key("paramsMissing")
                        || object.contains_key("getParameterMissing")
                    {
                        for key in PARAMETERS {
                            out.insert((*key).into(), Value::Null);
                        }
                    }
                    if object.contains_key("paramsMissing")
                        || object.contains_key("debugInfoMissing")
                    {
                        for key in ["unmaskedVendor", "unmaskedRenderer"] {
                            out.insert(key.into(), Value::Null);
                        }
                    }
                    if object.contains_key("paramsMissing") {
                        for key in [
                            "contextProperties",
                            "supportedExtensions",
                            "shaderPrecisions",
                            "dimensions",
                            "maxAnisotropy",
                        ] {
                            out.insert(key.into(), Value::Null);
                        }
                        let flags = out.entry("flags").or_insert_with(|| json!({}));
                        if let Some(flags) = flags.as_object_mut() {
                            flags.insert("antialias".into(), Value::Null);
                        }
                    }
                }
                // Collapse certified single-child repeated records, retaining
                // records with additional captured or dynamic keys.
                if path.ends_with("[]") && out.len() == 1 {
                    let name = path.trim_end_matches("[]").rsplit('.').next().unwrap_or("");
                    if let Some(group) = card
                        .all_fields()
                        .into_iter()
                        .find(|f| f.name == name && f.repeat.is_some())
                    {
                        if let Some((key, child)) = out.iter().next() {
                            if group.fields().iter().any(|f| f.name == key)
                                && group
                                    .fields()
                                    .iter()
                                    .filter(|f| {
                                        let candidate = format!("{path}.{}", f.name);
                                        !entry(card)["controls"]
                                            .as_array()
                                            .unwrap()
                                            .iter()
                                            .any(|x| x == &candidate)
                                    })
                                    .count()
                                    == 1
                            {
                                return child.clone();
                            }
                        }
                    }
                }
                Value::Object(out)
            }
            _ => value.clone(),
        }
    }
    if card.signal == "CurrentScriptAsyncPolicyState" {
        let asynchronous = value
            .get("async")
            .or_else(|| value.get("scriptFlags").and_then(|v| v.get("async")))
            .cloned()
            .or_else(|| value.get("scriptAsync").map(|_| Value::Null));
        let mut projected = walk(card, value, "");
        if let (Some(asynchronous), Some(fields)) = (asynchronous, projected.as_object_mut()) {
            fields.insert("async".into(), asynchronous);
        }
        return projected;
    }
    let mut out = walk(card, value, "");
    if card.signal == "VmSourceIntegrity" {
        for key in ["bodyLength", "whitespaceCount", "punctuationCount"] {
            if let Some(number) = out.get(key).and_then(Value::as_u64).filter(|n| *n <= 65535) {
                out[key] = json!((number & 255) ^ (number >> 8));
            }
        }
    }
    if let Some(rule) = crate::signals::wire::readable::GROUPS
        .iter()
        .find(|r| r.signal == card.signal)
    {
        if let Some(object) = out.as_object_mut() {
            // Group only a complete tuple; missing captured fields stay missing.
            if !object.contains_key(rule.parent)
                && (0..rule.count).all(|i| object.contains_key(&rule.old(i)))
            {
                let items: Vec<_> = (0..rule.count)
                    .map(|i| object.remove(&rule.old(i)).unwrap())
                    .collect();
                object.insert(rule.parent.into(), json!(items));
            }
        }
    }
    if let Some(object) = out.as_object() {
        if let Some((_, fields)) = card.messages().iter().find(|(_, fields)| {
            fields.len() == 1 && !fields[0].flags.is_empty() && fields[0].repeat.is_none()
        }) {
            if object.len() == 1 {
                if let Some(value) = object.get(fields[0].name).filter(|v| v.is_object()) {
                    return value.clone();
                }
            }
        }
    }
    out
}

fn field_metadata<'a>(card: &'a Card, name: &str) -> Option<&'a CardField> {
    let name = match name {
        "brands" => "brandEntries",
        "fullVersionList" => "fullVersionEntries",
        "async" => "scriptAsync",
        other => other,
    };
    card.all_fields()
        .into_iter()
        .find(|field| field.name == name)
}

/// Types describe the union of examples, without claiming sample lengths are
/// dimensions or that the documentary union is a runtime schema.
fn logical_fields(card: &Card, values: &[Value]) -> Value {
    fn collect(value: &Value, path: &str, fields: &mut BTreeMap<String, Vec<String>>) {
        let special = value.get("$special").and_then(Value::as_str);
        let kind = if let Some(special) = special {
            special
        } else {
            match value {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            }
        };
        let types = fields
            .entry(if path.is_empty() {
                "$".into()
            } else {
                path.into()
            })
            .or_default();
        if !types.iter().any(|s| s == kind) {
            types.push(kind.into());
            types.sort();
        }
        if special.is_some() {
            return;
        }
        match value {
            Value::Array(items) => {
                for item in items {
                    collect(item, &format!("{path}[]"), fields);
                }
            }
            Value::Object(object) => {
                for (key, value) in object {
                    collect(
                        value,
                        &if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{path}.{key}")
                        },
                        fields,
                    );
                }
            }
            _ => (),
        }
    }
    let mut fields = BTreeMap::new();
    for value in values {
        collect(value, "", &mut fields);
    }
    Value::Array(fields.into_iter().map(|(path, types)| {
        let leaf = path.rsplit('.').next().unwrap().trim_end_matches("[]");
        let metadata = field_metadata(card, leaf);
        let flag = card.all_fields().into_iter().find(|f| f.flags.contains(&leaf));
        json!({"path":path, "types":types, "description":metadata.map(|f| f.meaning).filter(|s| !s.is_empty()).or_else(|| flag.map(|f| f.meaning)).unwrap_or(card.summary),
            "type_basis":"documented variants plus supplied observation; lengths and field presence may vary", "card_field": metadata.map(|f| f.name).or_else(|| flag.map(|f| f.name))})
    }).collect())
}

/// Add logical presentation while retaining every legacy catalogue JSON key.
/// `capture_requested` differentiates static examples from an absent observation.
pub fn catalogue_json(card: &Card, observation: Option<&Value>, capture_requested: bool) -> Value {
    let mut out = crate::signals::docs::catalogue_json(card);
    let source = entry(card);
    let mut examples = source["examples"].clone();
    for example in examples.as_array_mut().unwrap() {
        example["data"] = clean(card, &example["data"]);
    }
    let values: Vec<_> = examples
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let mut value = e["data"].clone();
            for special in e["special_values"].as_array().unwrap() {
                if let Some(slot) = value.pointer_mut(special["pointer"].as_str().unwrap()) {
                    *slot = json!({"$special": special["value"]});
                }
            }
            value
        })
        .collect();
    let mut fields_values = values;
    if let Some(observation) = observation {
        fields_values.push(clean(card, &observation["data"]));
    }
    out["logical_fields"] = if card.no_data() {
        json!([])
    } else {
        logical_fields(card, &fields_values)
    };
    out["examples"] = examples.clone();
    out["example_source"] =
        json!({"path":source["source"], "sha256":source["sha256"], "embedded":true});
    if capture_requested {
        out["status"] = observation
            .map(|o| o["status"].clone())
            .unwrap_or(json!("absent"));
        out["data"] = observation
            .map(|o| clean(card, &o["data"]))
            .unwrap_or(Value::Null);
        out["provenance"] = json!("capture");
        out["special_values"] = json!([]);
        out["size"] = observation.map(|o| o["size"].clone()).unwrap_or(
            json!({"bytes":null,"kind":"unavailable","basis":"No matching capture observation"}),
        );
        if let Some(note) = observation.and_then(|o| o.get("note")) {
            out["note"] = note.clone();
        }
    } else {
        out["status"] = json!(if card.no_data() { "no_data" } else { "example" });
        out["data"] = examples[0]["data"].clone();
        out["provenance"] = examples[0]["provenance"].clone();
        out["special_values"] = examples[0]["special_values"].clone();
        out["size"] = json!({"bytes":null,"kind":"unavailable","basis":"An exact encoded signal size requires a matching capture; examples are not encoded measurements"});
    }
    out
}

pub fn render(card: &Card, observation: Option<&Value>, capture_requested: bool) -> String {
    let view = catalogue_json(card, observation, capture_requested);
    let mut out = format!(
        "# {}\n\n{}\n\n## Why\n\n{}\n\n## Logical fields\n\n",
        card.signal, card.summary, card.why
    );
    if card.no_data() {
        out.push_str("No data: no payload emitted.\n");
    } else {
        for field in view["logical_fields"].as_array().unwrap() {
            out.push_str(&format!(
                "- {}: {} — {}\n",
                field["path"].as_str().unwrap(),
                field["types"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" | "),
                field["description"].as_str().unwrap()
            ));
        }
        out.push_str(
            "\nTypes reflect documented variants; array lengths and field presence may vary.\n",
        );
    }
    out.push_str(&format!(
        "\nStatus: {} ({})\nSize: {}\n",
        view["status"].as_str().unwrap(),
        view["provenance"].as_str().unwrap(),
        if let Some(bytes) = view["size"]["bytes"].as_u64() {
            format!(
                "{bytes} bytes ({})",
                view["size"]["basis"]
                    .as_str()
                    .unwrap_or("exact signal blob")
            )
        } else {
            format!(
                "unavailable — {}",
                view["size"]["basis"]
                    .as_str()
                    .unwrap_or("no exact measurement")
            )
        }
    ));
    if let Some(note) = view["note"].as_str() {
        out.push_str(&format!("{note}\n"));
    }
    if capture_requested {
        out.push_str(&format!(
            "\n## Capture data\n\n```json\n{}\n```\n",
            serde_json::to_string_pretty(&view["data"]).unwrap()
        ));
    } else {
        for example in view["examples"].as_array().unwrap() {
            out.push_str(&format!(
                "\n## Data: {} ({})\n\n{}\n\n```json\n{}\n```\n",
                example["name"].as_str().unwrap(),
                example["provenance"].as_str().unwrap(),
                example["note"].as_str().unwrap(),
                serde_json::to_string_pretty(&example["data"]).unwrap()
            ));
            if !example["special_values"].as_array().unwrap().is_empty() {
                out.push_str(&format!(
                    "Special values (JSON pointers; null above is a transport placeholder): {}\n",
                    example["special_values"]
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card(name: &str) -> &'static Card {
        super::super::card(name).unwrap()
    }
    #[test]
    fn complete_compiled_logical_catalogue() {
        assert_eq!(manifest()["cards"].as_object().unwrap().len(), 93);
        for card in super::super::cards() {
            let view = catalogue_json(card, None, false);
            assert!(
                !view["examples"].as_array().unwrap().is_empty(),
                "{}",
                card.signal
            );
            assert_eq!(
                view["sample_payload"],
                crate::signals::docs::catalogue_json(card)["sample_payload"]
            );
            assert_eq!(view["size"]["kind"], "unavailable");
            for example in view["examples"].as_array().unwrap() {
                assert_eq!(
                    example["data"],
                    clean(card, &example["data"]),
                    "{}",
                    card.signal
                );
            }
        }
    }
    #[test]
    fn cleanup_preserves_measurements_unknown_keys_and_absence() {
        assert_eq!(
            clean(
                card("VmSourceIntegrity"),
                &json!({"bodyLength":36773,"whitespaceCount":6660,"punctuationCount":29783,"buildMarker":"marker"})
            ),
            json!({"bodyLength":42,"whitespaceCount":30,"punctuationCount":35,"buildMarker":"marker"})
        );
        assert_eq!(
            clean(
                card("MemoryBreakdownSummary"),
                &json!({"status":3,"entriesPlaceholder":null,"breakdownCountPlaceholder":null})
            ),
            json!({"status":3})
        );
        assert_eq!(
            clean(
                card("MemoryBreakdownSummary"),
                &json!({"status":2,"entryCount":1,"totalBytes":42,"breakdownCount":3,"entry":[{"entryBytes":12,"attributionCount":0,"attribution":[]}]})
            ),
            json!({"status":2,"totalBytes":42,"breakdownCount":3,"entry":[{"entryBytes":12,"attribution":[]}]})
        );
        assert_eq!(
            clean(
                card("UserAgentData"),
                &json!({"wow64Placeholder":null,"flags":{"mobile":false},"futureHint":{"dynamic":12}})
            ),
            json!({"flags":{"mobile":false,"wow64":null},"futureHint":{"dynamic":12}})
        );
        assert_eq!(
            clean(
                card("CurrentScriptAsyncPolicyState"),
                &json!({"status1":130,"scriptAsync":null})
            ),
            json!({"async":null})
        );
        assert_eq!(
            clean(
                card("CurrentScriptAsyncPolicyState"),
                &json!({"scriptFlags":{"async":false},"status1":130,"futureField":7})
            ),
            json!({"async":false,"futureField":7})
        );
    }
    #[test]
    fn capture_never_fills_from_example_and_retains_exact_size() {
        let observation = json!({"status":"captured","data":{"deviceMemory":null},"size":{"bytes":8,"kind":"exact","basis":"positional blob including 4-byte prefix"}});
        let view = catalogue_json(card("DeviceMemory"), Some(&observation), true);
        assert_eq!(view["data"], json!({"deviceMemory":null}));
        assert_eq!(view["size"], observation["size"]);
        let missing = catalogue_json(card("DeviceMemory"), None, true);
        assert_eq!(missing["status"], "absent");
        assert_eq!(missing["data"], Value::Null);
        assert!(!render(card("DeviceMemory"), None, true).contains("\"deviceMemory\": 32"));
    }
    #[test]
    fn variants_special_values_and_variable_arrays_are_explicit() {
        let ua = catalogue_json(card("UserAgentData"), None, false);
        assert!(ua["logical_fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "flags.wow64" && f["types"] == json!(["boolean", "null"])));
        assert!(ua["examples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["special_values"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["value"] == "undefined")));
        let webgl = catalogue_json(card("WebGlContext"), None, false);
        assert!(webgl["examples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["data"]["dimensions"] == json!([])));
        assert!(webgl["logical_fields"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f.get("length").is_none()));
        let no_data = super::super::cards().find(|c| c.no_data()).unwrap();
        assert_eq!(catalogue_json(no_data, None, false)["status"], "no_data");
    }

    #[test]
    fn certified_controls_arrays_and_nonfinite_values() {
        assert_eq!(
            clean(
                card("MatchMedia"),
                &json!({"gridBits":{},"gridCount":2,"gridFlush":{"0":true},"futureCount":7})
            ),
            json!({"gridFlush":{"0":true},"futureCount":7})
        );
        assert_eq!(
            clean(
                card("KeystrokeDynamics"),
                &json!({"specialKeySlotCount":2,"specialKeyCounts":[{"count":3},{"count":4,"future":true}]})
            ),
            json!({"specialKeyCounts":[3,{"count":4,"future":true}]})
        );
        let webgl = clean(
            card("WebGlContext"),
            &json!({"debugInfoMissing":null,"flags":{"collectedUsingWebWorker":false},"maxTextureSize":42}),
        );
        assert_eq!(webgl["unmaskedVendor"], Value::Null);
        assert_eq!(webgl["maxTextureSize"], 42);
        assert!(webgl.get("renderer").is_none());
        assert_eq!(
            clean(
                card("OfflineAudioRenderSamples"),
                &json!({"sample":[{"numberBits":9221120237041090560_u64,"special":"nan"}]})
            ),
            json!({"sample":[{"$special":"NaN"}]})
        );
        let audio = catalogue_json(card("OfflineAudioRenderSamples"), None, false);
        assert!(audio["examples"][0]["special_values"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["pointer"] == "/sample/1" && v["value"] == "NaN"));
        assert_eq!(audio["data"]["sample"].as_array().unwrap().len(), 9);
        let integer = catalogue_json(card("ParseIntHexConstant"), None, false);
        assert_eq!(
            integer["data"]["parsedValue"].as_u64(),
            Some(18_446_744_073_709_549_568)
        );
    }
}
