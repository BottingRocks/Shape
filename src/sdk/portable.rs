//! Versioned, portable operation boundary. Legacy `call` retains CLI defaults.
use super::*;
use std::path::Path;

pub const OPERATION_CONTRACT: &str = include_str!("../../sdk/operations.json");

pub fn call_v2(command: &str, request: Value) -> Result<Value> {
    let command = command.to_owned();
    isolate(move || dispatch_v2(&command, &request))
}
pub fn call_v2_json(command: &str, request: &str) -> String {
    envelope(parse_request(request).and_then(|request| call_v2(command, request)))
}
/// Compile directly to immutable validated state, without exporting its document.
pub fn compile_artifact_json(request: &str) -> Result<LoadedArtifact> {
    let request = parse_request(request)?;
    isolate(move || compile_artifact_request(&request))
}
fn compile_artifact_request(r: &Value) -> Result<LoadedArtifact> {
    validate_request(r)?;
    let compile = artifact_compile_options(r)?;
    let options = ArtifactOptions {
        name: name(r)?,
        cache: cache(r)?,
        float_byte_order: float_order(r)?.unwrap_or(FloatByteOrder::Little),
        direct: compile.direct,
    };
    let source = text(r, "source")?;
    let input = match optional_text(r, "input_kind")? {
        None | Some("program") => ProgramInput::Program(source),
        Some("raw") => ProgramInput::Raw(source),
        _ => return Err(invalid("input_kind must be raw or program")),
    };
    artifact::compile(
        input,
        &required(r, "session")?,
        text(r, "site_id")?,
        options,
    )
}
fn required<T: DeserializeOwned>(r: &Value, key: &str) -> Result<T> {
    typed(r, key)?.ok_or_else(|| invalid(format!("missing \"{key}\"")))
}
fn cache(r: &Value) -> Result<CachePolicy> {
    let Some(c) = field(r, "cache") else {
        return Ok(CachePolicy::Disabled);
    };
    if !c.is_object() {
        return Err(invalid("cache must be an object"));
    }
    match text(c, "mode")? {
        "disabled" if c.as_object().unwrap().len() == 1 => Ok(CachePolicy::Disabled),
        "environment" if c.as_object().unwrap().len() == 1 => Ok(CachePolicy::Environment),
        "directory" if c.as_object().unwrap().len() == 2 => {
            let directory = text(c, "directory")?;
            if directory.is_empty() { return Err(invalid("cache.directory must not be empty")); }
            Ok(CachePolicy::Directory(directory.into()))
        }
        _ => Err(invalid("cache requires {mode:disabled}, {mode:environment}, or {mode:directory,directory:string}")),
    }
}
fn program_options(r: &Value) -> Result<ProgramOptions> {
    if field(r, "report_dir").is_some() {
        return Err(invalid("v2 uses cache, not report_dir"));
    }
    let options = ProgramOptions {
        name: name(r)?,
        cache: cache(r)?,
        steps: number(r, "steps")?.unwrap_or(1_000_000),
        thread_cleanup: flag(r, "thread_cleanup", true)?,
        no_cleanup: flag(r, "no_cleanup", false)?,
        check_reuse: flag(r, "check_reuse", false)?,
        overrides: overrides(r)?,
        jobs: number(r, "jobs")?,
        quiet: flag(r, "quiet", true)?,
    };
    if options.steps == 0 || options.jobs == Some(0) {
        return Err(invalid("steps and jobs must be positive"));
    }
    Ok(options)
}
fn thread_options(r: &Value) -> Result<ThreadOptions> {
    let mut o = ThreadOptions::new(
        number(r, "pc")?.ok_or_else(|| invalid("missing \"pc\""))?,
        number(r, "state")?.ok_or_else(|| invalid("missing \"state\""))?,
    );
    o.descriptor = number(r, "descriptor")?;
    o.initial_slots = typed(r, "initial_slots")?;
    o.inputs = typed(r, "inputs")?;
    o.steps = number(r, "steps")?.unwrap_or(1_000_000);
    o.self_check = flag(r, "self_check", false)?;
    o.outline = flag(r, "outline", true)?;
    o.reverse_sides = flag(r, "reverse_sides", false)?;
    o.partial = flag(r, "partial", false)?;
    o.overrides = overrides(r)?;
    if o.steps == 0 {
        return Err(invalid("steps must be positive"));
    }
    Ok(o)
}
fn kernel_document(r: &Value) -> Result<KernelArtifact> {
    let value = document(r, "artifact")?.ok_or_else(|| invalid("missing \"artifact\""))?;
    kernel::load(&value.to_string())
}
fn reachable_options(r: &Value) -> Result<trace::corpus::ReachableOptions> {
    let o = trace::corpus::ReachableOptions {
        steps: number(r, "steps")?.unwrap_or(1_000_000),
        jobs: number(r, "jobs")?.unwrap_or(1),
        limit: number(r, "limit")?,
        cleanup: flag(r, "cleanup", false)?,
    };
    if o.steps == 0 || o.jobs == 0 {
        return Err(invalid("steps and jobs must be positive"));
    }
    Ok(o)
}
fn function(f: trace::corpus::Function) -> Value {
    json!({"descriptor":f.descriptor,"pc":f.pc,"state":f.state})
}
fn reachable_value(
    map: std::collections::BTreeMap<trace::corpus::Function, trace::corpus::Fingerprinted>,
) -> Value {
    Value::Array(map.into_iter().map(|(f,v)|json!({"function":function(f),"print":v.print,"code":v.code,"created":v.created.into_iter().map(function).collect::<Vec<_>>()})).collect())
}

/// Immutable kernel handle: parsing and validation occur once, outside calls.
pub struct LoadedKernel {
    artifact: KernelArtifact,
}
impl LoadedKernel {
    pub fn load(bytes: &[u8]) -> Result<Self> {
        let bytes = bytes.to_vec();
        isolate(move || {
            let json = std::str::from_utf8(&bytes).map_err(|e| invalid(e.to_string()))?;
            Ok(Self {
                artifact: kernel::load(json)?,
            })
        })
    }
    pub fn call(&self, operation: &str, request: Value) -> Result<Value> {
        validate_request(&request)?;
        match operation {
            "trace.program" => to_value(&trace::program_with_artifact(
                text(&request, "source")?,
                &self.artifact,
                program_options(&request)?,
            )?),
            "trace.thread" => to_value(&trace::thread(
                KernelInput::Artifact(&self.artifact),
                thread_options(&request)?,
            )?),
            "trace.corpus.reachable" => Ok(reachable_value(trace::corpus::reachable(
                &self.artifact,
                reachable_options(&request)?,
            )?)),
            "kernel.to_json" => Ok(Value::String(kernel::to_json(&self.artifact)?)),
            _ => Err(Error {
                code: "unknown_command",
                message: format!("unknown loaded kernel operation {operation:?}"),
            }),
        }
    }
    pub fn call_json(&self, operation: &str, request: &str) -> String {
        envelope(parse_request(request).and_then(|r| self.call(operation, r)))
    }
}

fn dispatch_v2(command: &str, r: &Value) -> Result<Value> {
    validate_request(r)?;
    match command {
        "version" => {
            let contract: Value = serde_json::from_str(OPERATION_CONTRACT).map_err(failed)?;
            Ok(
                json!({"version":VERSION,"contract_version":2,"commands":contract["operations"].as_array().unwrap().iter().map(|o|o["id"].clone()).collect::<Vec<_>>()}),
            )
        }
        "trace.program" => {
            let options = program_options(r)?;
            let raw = text(r, "source")?;
            match field(r, "artifact") {
                Some(_) => to_value(&trace::program_with_artifact(
                    raw,
                    &kernel_document(r)?,
                    options,
                )?),
                None => to_value(&trace::program(raw, options)?),
            }
        }
        "trace.thread" => {
            let options = thread_options(r)?;
            match (field(r, "source"), field(r, "artifact")) {
                (Some(_), None) => to_value(&trace::thread(
                    KernelInput::Source(text(r, "source")?),
                    options,
                )?),
                (None, Some(_)) => to_value(&trace::thread(
                    KernelInput::Artifact(&kernel_document(r)?),
                    options,
                )?),
                _ => Err(invalid(
                    "trace.thread needs exactly one of source or artifact",
                )),
            }
        }
        "artifact.compile" => {
            let loaded = compile_artifact_request(r)?;
            to_value(loaded.artifact())
        }
        "kernel.load" => to_value_json(kernel::to_json(&kernel_document(r)?)?),
        "kernel.to_json" => Ok(Value::String(kernel::to_json(&kernel_document(r)?)?)),
        "signals.catalog" => to_value(&signals::catalog()?),
        "signals.describe" => signals::describe(text(r, "name")?),
        "signals.compact_schema" => to_value(&signals::compact_schema(
            text(r, "source")?,
            artifact_compile_options(r)?,
        )?),
        "ops.classify.source" => Ok(classification(&ops::classify::source(text(r, "source")?)?)),
        "ops.classify.report" => match (field(r, "report"), field(r, "source")) {
            (Some(_), None) => Ok(classification(&ops::classify::report(&required(
                r, "report",
            )?)?)),
            (None, Some(_)) => Ok(classification(&ops::classify::source(text(r, "source")?)?)),
            _ => Err(invalid(
                "classify.report needs exactly one of report or source",
            )),
        },
        "ops.classify.template" => {
            if field(r, "template").is_some() {
                if field(r, "handler_index").is_some() {
                    return Err(invalid("template and handler_index are mutually exclusive"));
                }
                return Ok(classified_op(&ops::classify::template(
                    &required(r, "template")?,
                    text(r, "source")?,
                )?));
            }
            let index =
                number(r, "handler_index")?.ok_or_else(|| invalid("missing handler_index"))?;
            let index = u32::try_from(index).map_err(|_| invalid("handler_index exceeds u32"))?;
            let report = crate::deobfuscate_with_report(text(r, "source")?).map_err(failed)?;
            let templates = crate::operations::templates::handler_templates_from_report(&report)
                .map_err(failed)?;
            let template = templates
                .handlers
                .iter()
                .find(|t| t.handler_index == index)
                .ok_or_else(|| invalid("unknown handler_index"))?;
            let source = report
                .output
                .get(template.source_span.start as usize..template.source_span.end as usize)
                .ok_or_else(|| failed("invalid handler span"))?;
            Ok(classified_op(&ops::classify::template(template, source)?))
        }
        "ops.classify.seeds" => {
            let seeds: Vec<ClassificationInput> = required(r, "seeds")?;
            let seeds = seeds
                .into_iter()
                .map(ClassificationInput::domain)
                .collect::<Result<Vec<_>>>()?;
            Ok(Value::Array(ops::classify::seeds(&seeds)?.into_iter().map(|s|json!({"kind":s.kind,"nature":s.nature.as_str(),"scope":s.scope.as_str(),"handlers":s.handlers})).collect()))
        }
        "trace.corpus.load" => {
            let policy = cache(r)?;
            let artifact = trace::corpus::load(&name(r)?, text(r, "source")?, policy)?;
            to_value_json(kernel::to_json(&artifact)?)
        }
        "trace.corpus.source" => {
            let policy = cache(r)?;
            let options = reachable_options(r)?;
            Ok(reachable_value(trace::corpus::source(
                &name(r)?,
                text(r, "source")?,
                policy,
                options,
            )?))
        }
        "trace.corpus.reachable" => {
            let options = reachable_options(r)?;
            Ok(reachable_value(trace::corpus::reachable(
                &kernel_document(r)?,
                options,
            )?))
        }
        "trace.corpus.walk_seed" => {
            let policy = cache(r)?;
            let o = reachable_options(r)?;
            let options = trace::corpus::CorpusOptions {
                json: true,
                seeds: vec![],
                report_dir: None,
                baseline: None,
                update: false,
                self_check: flag(r, "self_check", false)?,
                cleanup: o.cleanup,
                jobs: o.jobs,
                branches: optional_text(r, "branches")?.map(PathBuf::from),
                audit: flag(r, "audit", false)?,
                steps: o.steps,
                limit: o.limit,
            };
            to_value(&trace::corpus::walk_seed(
                Path::new(text(r, "seed")?),
                &options,
                policy,
            )?)
        }
        "trace.corpus.fingerprint" => {
            let o = thread_options(r)?;
            let artifact = kernel_document(r)?;
            let walked = crate::trace::thread::walk_thread(
                &artifact,
                &crate::trace::thread::ThreadOptions {
                    pc: o.pc,
                    state: o.state,
                    descriptor: o.descriptor,
                    initial_slots: o.initial_slots,
                    inputs: o.inputs,
                    steps: o.steps,
                    self_check: o.self_check,
                    outline: o.outline,
                    reverse_sides: o.reverse_sides,
                    partial: o.partial,
                    overrides: o.overrides,
                },
            )
            .map_err(failed)?;
            let (print, code) = trace::corpus::fingerprint(
                &walked.report,
                &typed::<Vec<String>>(r, "parameters")?.unwrap_or_default(),
                &artifact,
                flag(r, "cleanup", false)?,
            )?;
            Ok(json!({"print":print,"code":code}))
        }
        "trace.corpus.compare" => {
            let c = trace::corpus::compare(
                &required::<Value>(r, "baseline")?,
                &required::<Value>(r, "current")?,
            )?;
            Ok(json!({"regressions":c.regressions,"changes":c.changes}))
        }
        "trace.corpus.compare_inline" => to_value(&trace::corpus::compare_inline(
            &required::<Value>(r, "outlined")?,
            &required::<Value>(r, "inline")?,
        )?),
        "naming.corpus.analyze" => {
            let sources = naming_sources(r, "sources")?;
            Ok(Value::Array(naming::corpus::analyze(&sources)?.into_iter().map(|a|json!({"name":a.name,"plan":a.plan,"rename":{"source":a.rename.transformed_source,"source_sha256":a.rename.source_sha256,"transformed_sha256":a.rename.transformed_sha256,"applied":a.rename.applied,"skipped":a.rename.skipped,"ambiguous":a.rename.ambiguous,"plan":a.rename.plan}})).collect()))
        }
        "naming.corpus.compare" => {
            let baseline = naming::corpus::analyze(&naming_sources(r, "baseline")?)?;
            let current = naming::corpus::analyze(&naming_sources(r, "current")?)?;
            Ok(Value::Array(naming::corpus::compare(&baseline,&current)?.into_iter().map(|d|json!({"name":d.name,"baseline_present":d.baseline_present,"current_present":d.current_present,"missing_roles":d.missing_roles,"added_roles":d.added_roles,"name_divergences":d.name_divergences})).collect()))
        }
        "pack" => to_value(&pack(
            &required(r, "value")?,
            &typed(r, "options")?.unwrap_or_default(),
        )?),
        "unpack" => to_value(&unpack(
            &required::<Vec<u8>>(r, "bytes")?,
            &typed(r, "options")?.unwrap_or_default(),
        )?),
        "unpack_canonical" => to_value(&unpack_canonical(
            &required::<Vec<u8>>(r, "bytes")?,
            &typed(r, "options")?.unwrap_or_default(),
        )?),
        c if c.starts_with("headers.") => dispatch_headers(c, r),
        c if c.starts_with("payload.") => dispatch_payload(c, r),
        c if c.starts_with("seeds.") => dispatch_seeds(c, r),
        _ => dispatch(command, r),
    }
}
fn to_value_json(json: String) -> Result<Value> {
    serde_json::from_str(&json).map_err(failed)
}
fn classified_op(op: &ops::classify::ClassifiedOp) -> Value {
    json!({"handler_index":op.handler_index,"kind":op.kind,"nature":op.nature.as_str(),"pops":op.pops,"pushes":op.pushes,"source":op.source})
}
fn classification(c: &ops::classify::SeedClassification) -> Value {
    json!({"variant":c.variant.as_str(),"ops":c.ops.iter().map(classified_op).collect::<Vec<_>>()})
}
#[derive(serde::Deserialize)]
struct ClassificationInput {
    variant: String,
    ops: Vec<ClassifiedInput>,
}
#[derive(serde::Deserialize)]
struct ClassifiedInput {
    handler_index: u32,
    kind: String,
    nature: String,
    pops: u32,
    pushes: u32,
    source: String,
}
impl ClassificationInput {
    fn domain(self) -> Result<ops::classify::SeedClassification> {
        use ops::classify::*;
        let variant = match self.variant.as_str() {
            "javascript-vm" => VmVariant::JavaScript,
            "webassembly-vm" => VmVariant::WebAssembly,
            _ => return Err(invalid("unknown VM variant")),
        };
        let ops = self
            .ops
            .into_iter()
            .map(|o| {
                Ok(ClassifiedOp {
                    handler_index: o.handler_index,
                    kind: o.kind,
                    nature: match o.nature.as_str() {
                        "javascript" => OpNature::JavaScript,
                        "vm" => OpNature::Vm,
                        "fused" => OpNature::Fused,
                        "unrecognized" => OpNature::Unrecognized,
                        _ => return Err(invalid("unknown op nature")),
                    },
                    pops: o.pops,
                    pushes: o.pushes,
                    source: o.source,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(SeedClassification { variant, ops })
    }
}
#[derive(serde::Deserialize)]
struct NamingSource {
    name: String,
    source: String,
}
fn naming_sources(r: &Value, key: &str) -> Result<Vec<naming::corpus::Source>> {
    Ok(required::<Vec<NamingSource>>(r, key)?
        .into_iter()
        .map(|s| naming::corpus::Source {
            name: s.name,
            source: s.source,
        })
        .collect())
}

fn dispatch_headers(c: &str, r: &Value) -> Result<Value> {
    if !matches!(
        c,
        "headers.codec"
            | "headers.parse"
            | "headers.encode"
            | "headers.decode"
            | "headers.encode_bytes"
            | "headers.decode_bytes"
            | "headers.encode_finalized"
            | "headers.decode_finalized"
    ) {
        return unknown(c);
    }
    if c == "headers.parse" {
        return to_value(&headers::parse(text(r, "text")?)?);
    }
    let profile: headers::Profile = required(r, "profile")?;
    let codec = headers::codec(profile.clone())?;
    match c {
        "headers.codec" => to_value(&profile),
        "headers.encode" => to_value(&headers::encode(&codec, &required(r, "report")?)?),
        "headers.decode" => to_value(&headers::decode(
            &codec,
            &required(r, "headers")?,
            number(r, "positional_count")?.ok_or_else(|| invalid("missing positional_count"))?,
            typed(r, "keysets")?.unwrap_or_default(),
        )?),
        "headers.encode_bytes" => {
            to_value(&headers::encode_bytes(&codec, &required(r, "payload")?)?)
        }
        "headers.decode_bytes" => {
            to_value(&headers::decode_bytes(&codec, &required(r, "headers")?)?)
        }
        "headers.encode_finalized" => to_value(&headers::encode_finalized(
            &codec,
            text(r, "text")?,
            typed::<headers::HeaderContext>(r, "context")?.as_ref(),
        )?),
        "headers.decode_finalized" => {
            let (text, context) = headers::decode_finalized(&codec, &required(r, "headers")?)?;
            Ok(json!({"text":text,"context":context}))
        }
        _ => unknown(c),
    }
}
fn dispatch_payload(c: &str, r: &Value) -> Result<Value> {
    if !matches!(
        c,
        "payload.codec"
            | "payload.encode"
            | "payload.decode"
            | "payload.encode_bytes"
            | "payload.decode_bytes"
    ) {
        return unknown(c);
    }
    let profile: payload::PayloadProfile = required(r, "profile")?;
    let codec = payload::codec(profile.clone())?;
    match c {
        "payload.codec" => to_value(&profile),
        "payload.encode" => to_value(&payload::encode(&codec, &required(r, "report")?)?),
        "payload.decode" => to_value(&payload::decode(
            &codec,
            text(r, "text")?,
            number(r, "positional_count")?.ok_or_else(|| invalid("missing positional_count"))?,
            typed(r, "keysets")?.unwrap_or_default(),
        )?),
        "payload.encode_bytes" => to_value(&payload::encode_bytes(
            &codec,
            &required::<Vec<u8>>(r, "bytes")?,
            required(r, "seeds")?,
        )?),
        "payload.decode_bytes" => to_value(&payload::decode_bytes(&codec, text(r, "text")?)?),
        _ => unknown(c),
    }
}
fn unknown(c: &str) -> Result<Value> {
    Err(Error {
        code: "unknown_command",
        message: format!("unknown operation {c:?}"),
    })
}
fn kernel_summary(k: &seeds::KernelSummary) -> Value {
    json!({"bytes":k.bytes,"sha256":k.sha256,"statements":k.statements,"functions":k.functions})
}
fn site(s: &seeds::Site) -> Value {
    json!({"name":s.name,"launcher_url":s.launcher_url,"base_url":s.base_url,"seed_param":s.seed_param,"headers":s.headers,"protocol":match s.protocol {seeds::Protocol::AsyncLauncher=>"async_launcher",seeds::Protocol::SingleFile=>"single_file",seeds::Protocol::DirectKernel=>"direct_kernel"},"status":match s.status {seeds::SiteStatus::Validated=>"validated",seeds::SiteStatus::Reference=>"reference"}})
}
fn client_config(r: &Value) -> Result<seeds::ClientConfig> {
    validate_request(r)?;
    let mut c = seeds::ClientConfig::default();
    if let Some(v) = number(r, "timeout_ms")? {
        c.timeout = std::time::Duration::from_millis(v as u64)
    }
    if let Some(v) = number(r, "connect_timeout_ms")? {
        c.connect_timeout = std::time::Duration::from_millis(v as u64)
    }
    if let Some(v) = number(r, "max_body_bytes")? {
        c.max_body_bytes = v as u64
    }
    if let Some(v) = number(r, "max_redirects")? {
        c.max_redirects = v
    }
    if c.timeout.is_zero() || c.connect_timeout.is_zero() || c.max_body_bytes == 0 {
        return Err(invalid("timeouts and max_body_bytes must be positive"));
    }
    Ok(c)
}
fn dispatch_seeds(c: &str, r: &Value) -> Result<Value> {
    match c {
        "seeds.parse_url" => Ok(Value::String(
            seeds::parse_url(text(r, "input")?)?.to_string(),
        )),
        "seeds.resolve_url" => {
            let base = optional_text(r, "base")?
                .map(seeds::parse_url)
                .transpose()?;
            Ok(Value::String(
                seeds::resolve_url(text(r, "reference")?, base.as_ref())?.to_string(),
            ))
        }
        "seeds.token" => Ok(Value::String(
            seeds::token(
                text(r, "reference")?,
                optional_text(r, "param")?.unwrap_or("seed"),
            )?
            .as_str()
            .to_owned(),
        )),
        "seeds.parse" => match seeds::parse(&required::<Vec<u8>>(r, "bytes")?)? {
            seeds::Script::Launcher(l) => {
                Ok(json!({"kind":"launcher","launcher":{"kernel_ref":l.kernel_ref}}))
            }
            seeds::Script::Kernel(k) => Ok(json!({"kind":"kernel","kernel":kernel_summary(&k)})),
        },
        "seeds.parse_launcher" => Ok(
            json!({"kernel_ref":seeds::parse_launcher(&required::<Vec<u8>>(r,"bytes")?)?.kernel_ref}),
        ),
        "seeds.parse_kernel" => Ok(kernel_summary(&seeds::parse_kernel(&required::<Vec<u8>>(
            r, "bytes",
        )?)?)),
        "seeds.sites" => Ok(Value::Array(seeds::sites().iter().map(site).collect())),
        "seeds.site" => Ok(site(seeds::site(text(r, "name")?)?)),
        "seeds.client" => {
            let c = client_config(r)?;
            let _ = seeds::client(c.clone())?;
            Ok(
                json!({"timeout_ms":c.timeout.as_millis() as u64,"connect_timeout_ms":c.connect_timeout.as_millis() as u64,"max_body_bytes":c.max_body_bytes,"max_redirects":c.max_redirects}),
            )
        }
        "seeds.fetch" | "seeds.fetch_direct" => {
            let url = seeds::parse_url(text(r, "url")?)?;
            let config = client_config(&document(r, "client")?.unwrap_or_else(|| json!({})))?;
            let mut options = seeds::FetchOptions::default();
            if let Some(pairs) = typed::<Vec<(String, String)>>(r, "headers")? {
                for (n, v) in pairs {
                    options
                        .headers
                        .set_line(&format!("{n}: {v}"))
                        .map_err(|e| invalid(e.to_string()))?
                }
            }
            options.base_url = optional_text(r, "base_url")?
                .map(seeds::parse_url)
                .transpose()?;
            if let Some(p) = optional_text(r, "seed_param")? {
                options.seed_param = p.to_owned()
            }
            let client = Arc::new(seeds::client(config)?);
            let fetched = if c == "seeds.fetch" {
                seeds::fetch(client, &url, options)?
            } else {
                seeds::fetch_direct(client, &url, options)?
            };
            Ok(
                json!({"seed":fetched.seed.as_str(),"launcher_url":fetched.launcher_url.as_str(),"kernel_url":fetched.kernel_url.as_str(),"launcher":fetched.launcher,"kernel":fetched.kernel,"kernel_summary":kernel_summary(&fetched.kernel_summary)}),
            )
        }
        "seeds.import" => {
            let seed = crate::seeds::seed::SeedToken::new(text(r, "seed")?)
                .map_err(|e| invalid(e.to_string()))?;
            let launcher: Option<Vec<u8>> = typed(r, "launcher")?;
            let o = seeds::import(
                Path::new(text(r, "directory")?),
                &seed,
                launcher.as_deref(),
                &required::<Vec<u8>>(r, "kernel")?,
            )?;
            Ok(
                json!({"status":o.status.as_str(),"kernel_path":o.kernel_path,"launcher_path":o.launcher_path,"launcher_differs":o.launcher_differs}),
            )
        }
        _ => unknown(c),
    }
}
