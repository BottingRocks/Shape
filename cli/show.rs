//! Source-independent catalogue inspection.
use std::{fs, process::ExitCode};

const USAGE: &str = "usage: shape show signals [SignalName] [--json] [--artifact PATH --headers PATH [--session PATH]]\n\nShow all registered catalogue cards in alphabetical order, or one exact canonical\nsignal name, including descriptions, logical fields, clean data and exact size when captured.\nNo program.js, seed or capture is required.\n\n  --json       structured cards; legacy sample_payload retained (array for all)\n  --artifact   compiled signal artifact for capture decoding\n  --headers    matching captured headers; requires --artifact\n  --session    optional matching session material; requires artifact + headers\n  -h, --help   show this help\n\nFor generator-input analysis use: shape signals show <program.js>";

pub fn run(args: Vec<String>) -> ExitCode {
    let error = |message: &str| super::usage_error("shape show", message, USAGE);
    if matches!(args.as_slice(), [arg] if arg == "--help" || arg == "-h") {
        return super::print_help(USAGE);
    }
    if args.first().map(String::as_str) != Some("signals") {
        return error("expected signals");
    }
    let mut name = None;
    let mut json = false;
    let (mut artifact, mut headers, mut session) = (None, None, None);
    let mut arguments = args[1..].iter();
    while let Some(arg) = arguments.next() {
        match arg.as_str() {
            "--help" | "-h" => return super::print_help(USAGE),
            "--json" if !json => json = true,
            "--artifact" | "--headers" | "--session" => {
                let slot = match arg.as_str() {
                    "--artifact" => &mut artifact,
                    "--headers" => &mut headers,
                    _ => &mut session,
                };
                if slot.is_some() {
                    return error(&format!("repeated option {arg}"));
                }
                let Some(path) = arguments.next().filter(|p| !p.starts_with('-')) else {
                    return error(&format!("{arg} requires a path"));
                };
                *slot = Some(path);
            }
            flag if flag.starts_with('-') => {
                return error(&format!("unknown or repeated option {flag}"))
            }
            value if name.is_none() => name = Some(value),
            _ => return error("expected at most one canonical signal name"),
        }
    }
    if artifact.is_some() != headers.is_some() || (session.is_some() && artifact.is_none()) {
        return error("--artifact and --headers are required together; --session requires both");
    }
    let mut cards: Vec<_> = shape::signals::catalog::cards().collect();
    cards.sort_by_key(|card| card.signal);
    if let Some(name) = name {
        cards.retain(|card| card.signal == name);
        if cards.is_empty() {
            return error(&format!(
                "unknown signal {name}; use `shape show signals` to list canonical names"
            ));
        }
    }
    let capture_requested = artifact.is_some();
    let observations: std::collections::BTreeMap<String, serde_json::Value> =
        if let (Some(artifact), Some(headers)) = (artifact, headers) {
            let read =
                |path: &String| fs::read(path).map_err(|e| format!("cannot read {path}: {e}"));
            let inspect = || -> Result<_, String> {
                let artifact = read(artifact)?;
                let headers = read(headers)?;
                let session = session.map(read).transpose()?;
                shape::signals::inspect::inspect(&artifact, &headers, session.as_deref())
            };
            match inspect() {
                Ok(observations) => observations,
                Err(message) => {
                    eprintln!("shape show: {message}");
                    return ExitCode::from(1);
                }
            }
        } else {
            Default::default()
        };
    let text = if json {
        let values: Vec<_> = cards
            .iter()
            .map(|card| {
                shape::signals::catalog::view::catalogue_json(
                    card,
                    observations.get(card.signal),
                    capture_requested,
                )
            })
            .collect();
        let value = if name.is_some() {
            values.into_iter().next().unwrap()
        } else {
            serde_json::Value::Array(values)
        };
        serde_json::to_string_pretty(&value).unwrap()
    } else {
        cards
            .iter()
            .map(|card| {
                shape::signals::catalog::view::render(
                    card,
                    observations.get(card.signal),
                    capture_requested,
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    match super::write_stdout_line(&text) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shape show: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}
