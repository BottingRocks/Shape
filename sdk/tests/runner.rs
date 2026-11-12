//! Native Rust SDK reference; direct loaded operations, no C ABI surrogate.
use serde_json::{json, Value};
use shape::sdk;
use std::{env, fs};

fn document(body: &Value, key: &str) -> sdk::Result<Value> {
    match &body[key] {
        Value::String(text) => serde_json::from_str(text).map_err(invalid),
        value => Ok(value.clone()),
    }
}
fn invalid(error: impl std::fmt::Display) -> sdk::Error {
    sdk::Error {
        code: "invalid_request",
        message: error.to_string(),
    }
}
fn loaded(artifact: &sdk::LoadedArtifact, method: &str, body: &Value) -> sdk::Result<Value> {
    let session = if body["session"].is_null() {
        None
    } else {
        Some(serde_json::from_value(document(body, "session")?).map_err(invalid)?)
    };
    let verify = body["verify"].as_bool().unwrap_or(false);
    match method {
        "decode" => {
            let capture = serde_json::from_value(document(body, "capture")?).map_err(invalid)?;
            let decoded = sdk::decode(
                artifact,
                &capture,
                session.as_ref(),
                sdk::DecodeOptions {
                    verify,
                    metadata: body["metadata"].as_bool().unwrap_or(false),
                },
            )?;
            serde_json::to_value(decoded).map_err(invalid)
        }
        "explain" => {
            let decoded = serde_json::from_value(document(body, "decoded")?).map_err(invalid)?;
            let name = body["signal"]
                .as_str()
                .ok_or_else(|| invalid("missing signal"))?;
            serde_json::to_value(sdk::explain(
                artifact,
                &decoded,
                name,
                session.as_ref(),
                verify,
            )?)
            .map_err(invalid)
        }
        "encode" => {
            let encoded = if body.get("signals").is_some() {
                let signals =
                    serde_json::from_value(document(body, "signals")?).map_err(invalid)?;
                sdk::encode_signals(artifact, &signals, session.as_ref())?
            } else {
                let decoded =
                    serde_json::from_value(document(body, "decoded")?).map_err(invalid)?;
                sdk::encode(artifact, &decoded, session.as_ref())?
            };
            serde_json::to_value(encoded).map_err(invalid)
        }
        "schema" => serde_json::to_value(sdk::schema(artifact)?).map_err(invalid),
        "artifact" => serde_json::from_slice(&artifact.to_json()?).map_err(invalid),
        _ => Err(sdk::Error {
            code: "unknown_command",
            message: "unknown loaded operation".into(),
        }),
    }
}
fn main() {
    let args: Vec<String> = env::args().collect();
    let request = fs::read_to_string(&args[4]).expect("read request");
    let result = match args[2].as_str() {
        "kernel" => {
            let kernel = sdk::LoadedKernel::load(&fs::read(&args[1]).expect("read kernel"))
                .expect("load kernel");
            kernel.call_json(&args[3], &request)
        }
        "loaded" => sdk::envelope((|| {
            let artifact = sdk::artifact::load(&fs::read(&args[1]).expect("read artifact"))?;
            let body: Value = serde_json::from_str(&request).map_err(invalid)?;
            let value = loaded(&artifact, &args[3], &body)?;
            if args[3] != "encode" {
                for _ in 0..2 {
                    assert_eq!(value, loaded(&artifact, &args[3], &body)?);
                }
            }
            Ok(value)
        })()),
        "named" | "v2" => sdk::call_v2_json(&args[3], &request),
        "generator" => {
            let artifact = fs::read(&args[1]).expect("read artifact");
            let generator = sdk::Generator::load(&artifact).expect("load artifact");
            generator.call_json(&args[3], &request)
        }
        _ => sdk::call_json(&args[3], &request),
    };
    // Ensure dependency's exact-integer JSON feature is linked into this driver.
    let _: Value = json!(18446744073709551615u64);
    println!("{result}");
}
