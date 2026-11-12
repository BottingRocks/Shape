//! File boundary for the focused collector API. Outputs are always create-new.
use shape::naming::collectors::identify_collectors;
use shape::naming::labels::{build_label_reference, LabelReference};
use shape::naming::name_threads_with_labels;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    process::ExitCode,
};

const USAGE: &str = "usage: shape naming collectors <traced.js> [--map-out <report.json>] [--rename --out <focused.js>]\nReports original function identities, collector rows and source-proven payload routing.\nRenames only proven collector bindings and resolved references. Outputs must be new files.\nNo tracing, recovered JavaScript execution, general naming, comments or formatting.";

pub(crate) fn run(args: Vec<String>) -> ExitCode {
    run_mode(args, false)
}
pub(crate) fn run_threads(args: Vec<String>) -> ExitCode {
    run_mode(args, true)
}
fn run_mode(args: Vec<String>, threads: bool) -> ExitCode {
    match execute(args, threads) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "shape naming {}: {error}",
                if threads { "threads" } else { "collectors" }
            );
            ExitCode::from(1)
        }
    }
}
fn execute(args: Vec<String>, threads: bool) -> Result<(), String> {
    let usage = if threads {
        "usage: shape naming threads <readable.js> [--map-out <report.json>] [--rename --out <output.js>] [--labels <reference.json>]\nReference creation: threads <unpruned-readable.js> --original <trace.js> --save-labels <new.json>\nNames only remaining generated THREAD function bindings using static evaluators; keeps all readable names and other source bytes. No clobber."
    } else {
        USAGE
    };
    let mut input = None;
    let mut output = None;
    let mut map = None;
    let mut rename = false;
    let mut labels = None;
    let mut save_labels = None;
    let mut original = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{usage}");
                return Ok(());
            }
            "--rename" if !rename => rename = true,
            "--labels" | "--save-labels" | "--original" if threads => {
                let target = match arg.as_str() {
                    "--labels" => &mut labels,
                    "--save-labels" => &mut save_labels,
                    _ => &mut original,
                };
                let value = args.next().ok_or_else(|| format!("{arg} needs a path"))?;
                if target.replace(PathBuf::from(value)).is_some() {
                    return Err(format!("duplicate {arg}"));
                }
            }
            "--out" | "--map-out" => {
                let target = if arg == "--out" {
                    &mut output
                } else {
                    &mut map
                };
                let value = args.next().ok_or_else(|| format!("{arg} needs a path"))?;
                if target.replace(PathBuf::from(value)).is_some() {
                    return Err(format!("duplicate {arg}"));
                }
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}\n{usage}"))
            }
            value => {
                if input.replace(PathBuf::from(value)).is_some() {
                    return Err("more than one input".into());
                }
            }
        }
    }
    let input = input.ok_or(usage)?;
    if save_labels.is_some() || original.is_some() {
        if rename || output.is_some() || map.is_some() || labels.is_some() {
            return Err(
                "--save-labels --original is a separate report-only reference creation mode".into(),
            );
        }
        let target = save_labels.ok_or("--original requires --save-labels")?;
        let original = original.ok_or("--save-labels requires --original")?;
        let _ = destination(&target)?;
        if fs::symlink_metadata(&target).is_ok() {
            return Err("label reference output already exists (no clobber)".into());
        }
        let reference = build_label_reference(
            &crate::cli::read_source(original)
                .map_err(|e| e.to_string())?
                .text,
            &crate::cli::read_source(input)
                .map_err(|e| e.to_string())?
                .text,
        )?;
        let json = serde_json::to_vec_pretty(&reference).map_err(|e| e.to_string())?;
        if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| e.to_string())?;
        file.write_all(&json).map_err(|e| e.to_string())?;
        eprintln!(
            "saved {} structural labels ({} unsupported)",
            reference.labels.len(),
            reference.excluded.len()
        );
        return Ok(());
    }
    if rename != output.is_some() {
        return Err("--rename and --out must be supplied together".into());
    }
    let input_path = fs::canonicalize(&input).map_err(|e| e.to_string())?;
    let paths: Vec<_> = [output.as_deref(), map.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    let mut destinations = Vec::new();
    for path in &paths {
        let canonical = destination(path)?;
        if canonical == input_path || destinations.contains(&canonical) {
            return Err("input and output/map paths must be distinct".into());
        }
        if fs::symlink_metadata(path).is_ok() {
            return Err(format!(
                "output {} already exists (no clobber)",
                path.display()
            ));
        }
        destinations.push(canonical);
    }
    let read_start = std::time::Instant::now();
    let source = crate::cli::read_source(&input).map_err(|e| e.to_string())?;
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
    let (output_source, mut report) = if threads {
        let reference: Option<LabelReference> = labels
            .map(|p| {
                fs::read(p)
                    .map_err(|e| e.to_string())
                    .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
            })
            .transpose()?;
        let result = name_threads_with_labels(&source.text, rename, reference.as_ref())?;
        (
            result.source,
            serde_json::to_value(result.report).map_err(|e| e.to_string())?,
        )
    } else {
        let result = identify_collectors(&source.text, rename).map_err(|e| e.to_string())?;
        (
            result.source,
            serde_json::to_value(result.report).map_err(|e| e.to_string())?,
        )
    };
    report["timings_ms"]["read_input"] = serde_json::json!(read_ms);
    let serialize_start = std::time::Instant::now();
    let json = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    let serialize_ms = serialize_start.elapsed().as_secs_f64() * 1000.0;
    // Reserve all outputs before writing either. create_new also rejects races,
    // symlinks and hard links created after the preflight check.
    let mut reserved = Vec::new();
    let writes = (|| -> Result<(), String> {
        for path in &paths {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            reserved.push((*path, file));
        }
        for (path, file) in &mut reserved {
            let bytes = if output.as_deref() == Some(*path) {
                output_source.as_bytes()
            } else {
                &json
            };
            file.write_all(bytes)
                .and_then(|_| file.flush())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if writes.is_err() {
        for (path, _) in &reserved {
            let _ = fs::remove_file(path);
        }
    }
    writes?;
    if map.is_none() {
        crate::cli::write_stdout(&json).map_err(|e| e.to_string())?;
    }
    eprintln!(
        "{}: extraction/rewrite {}ms, read {:.3}ms, JSON {:.3}ms",
        if threads { "threads" } else { "collectors" },
        report["timings_ms"]["total"],
        read_ms,
        serialize_ms
    );
    Ok(())
}
fn destination(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    if absolute
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err("output paths containing '..' are refused".into());
    }
    let mut parent = absolute.as_path();
    let mut suffix = Vec::new();
    while !parent.exists() {
        suffix.push(parent.file_name().ok_or("invalid output path")?.to_owned());
        parent = parent.parent().ok_or("invalid output parent")?;
    }
    let mut canonical = fs::canonicalize(parent).map_err(|e| e.to_string())?;
    for part in suffix.iter().rev() {
        canonical.push(part);
    }
    Ok(canonical)
}
