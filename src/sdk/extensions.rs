//! Additional operation families over public, source-backed engines.
//!
//! These operations never acquire seeds, scan directories, or persist reports
//! implicitly. Header/payload codecs are byte framing primitives, independent
//! of the named-field decode artifact API.

use super::{failed, isolate, Result};

pub use crate::signals::payload::superpack::{
    Decoded as PackDecoded, Limits, Number, SerializerOptions, Value as PackValue,
};

pub fn pack(value: &PackValue, options: &SerializerOptions) -> Result<Vec<u8>> {
    let (value, options) = (value.clone(), options.clone());
    isolate(move || crate::signals::payload::superpack::pack(&value, &options).map_err(failed))
}

pub fn unpack(bytes: &[u8], options: &SerializerOptions) -> Result<PackDecoded> {
    let (bytes, options) = (bytes.to_vec(), options.clone());
    isolate(move || crate::signals::payload::superpack::unpack(&bytes, &options).map_err(failed))
}

pub fn unpack_canonical(bytes: &[u8], options: &SerializerOptions) -> Result<PackValue> {
    let (bytes, options) = (bytes.to_vec(), options.clone());
    isolate(move || {
        crate::signals::payload::superpack::unpack_canonical(&bytes, &options).map_err(failed)
    })
}

pub mod ops {
    pub mod classify {
        use super::super::super::{failed, isolate, Result};
        pub use crate::operations::classify::{
            ClassifiedOp, KindSummary, OpNature, OpScope, SeedClassification, VmVariant,
        };
        pub use crate::{HandlerTemplate, PipelineReport};

        /// Parse and deobfuscate the supplied raw source before classification.
        pub fn source(raw: &str) -> Result<SeedClassification> {
            let raw = raw.to_owned();
            isolate(move || {
                let report = crate::deobfuscate_with_report(&raw).map_err(failed)?;
                crate::operations::classify::classify_report(&report).map_err(failed)
            })
        }

        pub fn report(report: &PipelineReport) -> Result<SeedClassification> {
            let report = report.clone();
            isolate(move || crate::operations::classify::classify_report(&report).map_err(failed))
        }

        pub fn template(template: &HandlerTemplate, source: &str) -> Result<ClassifiedOp> {
            let (template, source) = (template.clone(), source.to_owned());
            isolate(move || {
                Ok(crate::operations::classify::classify_template(
                    &template, source,
                ))
            })
        }

        pub fn seeds(seeds: &[SeedClassification]) -> Result<Vec<KindSummary>> {
            let seeds = seeds.to_vec();
            isolate(move || Ok(crate::operations::classify::classify_seeds(&seeds)))
        }
    }
}

pub mod trace {
    pub mod corpus {
        use super::super::super::{failed, invalid, isolate, Result};
        pub use crate::trace::corpus::{CorpusOptions, Fingerprinted, Function, KernelCachePolicy};
        pub use crate::{KernelArtifact, PrefixTraceReport};
        use serde_json::Value;
        use std::collections::BTreeMap;
        use std::path::Path;

        #[derive(Clone, Debug)]
        pub struct ReachableOptions {
            pub steps: usize,
            pub jobs: usize,
            pub limit: Option<usize>,
            pub cleanup: bool,
        }

        impl Default for ReachableOptions {
            fn default() -> Self {
                Self {
                    steps: 1_000_000,
                    jobs: 1,
                    limit: None,
                    cleanup: false,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Comparison {
            pub regressions: Vec<String>,
            pub changes: Vec<String>,
        }

        /// Read a seed file and run the core corpus convergence and optional
        /// self checks. The explicit policy overrides `options.report_dir`.
        /// Branch output, when requested by `options.branches`, is written by core.
        pub fn walk_seed(
            seed: &Path,
            options: &CorpusOptions,
            policy: KernelCachePolicy,
        ) -> Result<BTreeMap<String, Value>> {
            let (seed, options) = (seed.to_owned(), options.clone());
            isolate(move || {
                if options.steps == 0 || options.jobs == 0 {
                    return Err(invalid("steps and jobs must be positive"));
                }
                crate::trace::corpus::walk_seed_with_cache(&seed, &options, &policy).map_err(failed)
            })
        }

        /// Load or compile caller-supplied source under the explicit policy.
        /// `seed` names the source; it is not read from disk.
        pub fn load(
            seed: &Path,
            source: &str,
            policy: KernelCachePolicy,
        ) -> Result<KernelArtifact> {
            let (seed, source) = (seed.to_owned(), source.to_owned());
            isolate(move || {
                crate::trace::corpus::load_artifact_with_policy(&seed, &source, &policy)
                    .map_err(failed)
            })
        }

        /// Raw-source reachable tracing. Use Disabled for no cache IO;
        /// Directory/Environment opt into the core cache policies explicitly.
        pub fn source(
            seed: &Path,
            source: &str,
            policy: KernelCachePolicy,
            options: ReachableOptions,
        ) -> Result<BTreeMap<Function, Fingerprinted>> {
            let (seed, source) = (seed.to_owned(), source.to_owned());
            isolate(move || {
                validate_options(&options)?;
                let artifact =
                    crate::trace::corpus::load_artifact_with_policy(&seed, &source, &policy)
                        .map_err(failed)?;
                reachable_inner(&artifact, options)
            })
        }

        fn validate_options(options: &ReachableOptions) -> Result<()> {
            if options.steps == 0 || options.jobs == 0 {
                return Err(invalid("steps and jobs must be positive"));
            }
            Ok(())
        }

        /// Walk the loaded image's roots and newly created functions in memory.
        /// A limit may leave functions unwalked; inspect each fingerprint's stop.
        /// This primitive does not run CLI cross-function cell/return convergence
        /// or the CLI's inline/reversed self checks.
        pub fn reachable(
            artifact: &KernelArtifact,
            options: ReachableOptions,
        ) -> Result<BTreeMap<Function, Fingerprinted>> {
            let artifact = artifact.clone();
            isolate(move || reachable_inner(&artifact, options))
        }

        fn reachable_inner(
            artifact: &KernelArtifact,
            options: ReachableOptions,
        ) -> Result<BTreeMap<Function, Fingerprinted>> {
            use crate::trace::corpus as core;
            validate_options(&options)?;
            let prepared = artifact
                .prepare_with_limits(crate::PrefixLimits {
                    max_dispatch_steps: options.steps,
                    outline_subroutines: true,
                    ..crate::PrefixLimits::default()
                })
                .map_err(failed)?;
            let entry = artifact
                .entry_facts()
                .ok_or_else(|| failed("kernel artifact has no actual entry facts"))?;
            let descriptors = artifact.descriptor_records();
            let slots = artifact.entry_initial_slots();
            let captures = crate::trace::thread::thread_captures(&descriptors);
            let roots = core::roots(
                Function {
                    descriptor: None,
                    pc: entry.pc as usize,
                    state: entry.state as usize,
                },
                &descriptors,
            );
            core::walk_reachable(&roots, options.jobs, options.limit, |function, announce| {
                core::startable(&prepared, function)?;
                let mut setup = core::setup(function, &descriptors, &slots, options.steps)?;
                setup.captures = captures.clone();
                let result = core::invoke_announcing(&prepared, function, &setup, Some(announce))?;
                let created = core::created_functions(&result);
                let (mut print, code) = core::fingerprint(
                    &result,
                    &setup.parameters,
                    &core::Emitting {
                        artifact,
                        cleanup: options.cleanup,
                    },
                );
                core::add_digest(&mut print, &code);
                Ok((
                    Fingerprinted {
                        print,
                        code,
                        created: created.clone(),
                    },
                    created,
                ))
            })
            .map_err(failed)
        }

        pub fn fingerprint(
            result: &PrefixTraceReport,
            parameters: &[String],
            artifact: &KernelArtifact,
            cleanup: bool,
        ) -> Result<(Value, Option<String>)> {
            let (result, parameters, artifact) =
                (result.clone(), parameters.to_vec(), artifact.clone());
            isolate(move || {
                Ok(crate::trace::corpus::fingerprint(
                    &result,
                    &parameters,
                    &crate::trace::corpus::Emitting {
                        artifact: &artifact,
                        cleanup,
                    },
                ))
            })
        }

        /// Compare caller-supplied seed -> function -> fingerprint documents.
        pub fn compare(baseline: &Value, current: &Value) -> Result<Comparison> {
            let (baseline, current) = (baseline.clone(), current.clone());
            isolate(move || {
                validate_document(&baseline)?;
                validate_document(&current)?;
                let (regressions, changes) = crate::trace::corpus::compare(&baseline, &current);
                Ok(Comparison {
                    regressions,
                    changes,
                })
            })
        }

        fn validate_document(value: &Value) -> Result<()> {
            let seeds = value
                .as_object()
                .ok_or_else(|| invalid("corpus must be an object"))?;
            for functions in seeds.values() {
                let functions = functions
                    .as_object()
                    .ok_or_else(|| invalid("seed fingerprints must be an object"))?;
                if functions.values().any(|value| !value.is_object()) {
                    return Err(invalid("function fingerprints must be objects"));
                }
            }
            Ok(())
        }

        pub fn compare_inline(outlined: &Value, inline: &Value) -> Result<Vec<String>> {
            let (outlined, inline) = (outlined.clone(), inline.clone());
            isolate(move || {
                if !outlined.is_object() || !inline.is_object() {
                    return Err(invalid("fingerprints must be objects"));
                }
                Ok(crate::trace::corpus::compare_inline(&outlined, &inline))
            })
        }
    }
}

pub mod naming {
    pub mod corpus {
        use super::super::super::{failed, invalid, isolate, Result};
        pub use crate::naming::program::{SemanticRenameOptions, SemanticRenameResult};
        pub use crate::naming::semantic_names::SemanticNamingPlan;
        use std::collections::{BTreeMap, BTreeSet};

        #[derive(Clone, Debug)]
        pub struct Source {
            pub name: String,
            /// Already traced program text; filenames do not select a mode.
            pub source: String,
        }

        #[derive(Clone, Debug)]
        pub struct Analysis {
            pub name: String,
            pub plan: SemanticNamingPlan,
            pub rename: SemanticRenameResult,
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Difference {
            pub name: String,
            pub baseline_present: bool,
            pub current_present: bool,
            pub missing_roles: Vec<String>,
            pub added_roles: Vec<String>,
            pub name_divergences: Vec<String>,
        }

        /// Derive each source-stamped naming plan and its actual rewrite.
        /// No source acquisition, tracing, cache lookup or filesystem output.
        pub fn analyze(inputs: &[Source]) -> Result<Vec<Analysis>> {
            let inputs = inputs.to_vec();
            isolate(move || {
                validate_names(inputs.iter().map(|input| input.name.as_str()))?;
                inputs
                    .into_iter()
                    .map(|input| {
                        let rename = crate::naming::program::rename_semantic_names_with_options(
                            &input.source,
                            SemanticRenameOptions::default(),
                        )
                        .map_err(failed)?;
                        Ok(Analysis {
                            name: input.name,
                            plan: rename.plan.clone(),
                            rename,
                        })
                    })
                    .collect()
            })
        }

        /// Compare proven role/name sets by caller-assigned source name.
        /// Source hashes/spans remain in the plans; they are not cross-build IDs.
        pub fn compare(baseline: &[Analysis], current: &[Analysis]) -> Result<Vec<Difference>> {
            let (baseline, current) = (baseline.to_vec(), current.to_vec());
            isolate(move || {
                validate_names(baseline.iter().map(|input| input.name.as_str()))?;
                validate_names(current.iter().map(|input| input.name.as_str()))?;
                let old: BTreeMap<_, _> = baseline
                    .iter()
                    .map(|a| (a.name.clone(), roles(&a.plan)))
                    .collect();
                let new: BTreeMap<_, _> = current
                    .iter()
                    .map(|a| (a.name.clone(), roles(&a.plan)))
                    .collect();
                let names: BTreeSet<_> = old.keys().chain(new.keys()).cloned().collect();
                let empty = BTreeMap::new();
                Ok(names
                    .into_iter()
                    .map(|name| {
                        let baseline_present = old.contains_key(&name);
                        let current_present = new.contains_key(&name);
                        let (old, new) = (
                            old.get(&name).unwrap_or(&empty),
                            new.get(&name).unwrap_or(&empty),
                        );
                        Difference {
                            baseline_present,
                            current_present,
                            missing_roles: old
                                .keys()
                                .filter(|role| !new.contains_key(*role))
                                .cloned()
                                .collect(),
                            added_roles: new
                                .keys()
                                .filter(|role| !old.contains_key(*role))
                                .cloned()
                                .collect(),
                            name_divergences: old
                                .iter()
                                .filter_map(|(role, names)| {
                                    new.get(role)
                                        .filter(|new_names| *new_names != names)
                                        .map(|_| role.clone())
                                })
                                .collect(),
                            name,
                        }
                    })
                    .collect())
            })
        }

        fn roles(plan: &SemanticNamingPlan) -> BTreeMap<String, BTreeSet<String>> {
            let mut roles = BTreeMap::<String, BTreeSet<String>>::new();
            for candidate in &plan.candidates {
                roles
                    .entry(candidate.role.clone())
                    .or_default()
                    .insert(candidate.proposed_name.clone());
            }
            for candidate in &plan.anonymous_functions {
                roles
                    .entry(candidate.role.clone())
                    .or_default()
                    .insert(candidate.proposed_name.clone());
            }
            roles
        }

        fn validate_names<'a>(names: impl Iterator<Item = &'a str>) -> Result<()> {
            let mut seen = BTreeSet::new();
            for name in names {
                if name.trim().is_empty() || !seen.insert(name) {
                    return Err(invalid("source names must be nonempty and unique"));
                }
            }
            Ok(())
        }
    }
}

pub mod seeds {
    use super::{failed, isolate, Result};
    pub use crate::seeds::collect::{FetchOptions, FetchedSeed};
    pub use crate::seeds::http::{ClientConfig, Headers, HttpClient, HttpResponse, ReqwestClient};
    pub use crate::seeds::script::{KernelSummary, Launcher, Script};
    pub use crate::seeds::seed::SeedToken;
    pub use crate::seeds::sites::{Protocol, Site, SiteStatus};
    pub use crate::seeds::store::{StoreOutcome, StoreStatus};
    use std::path::Path;
    pub use std::sync::Arc;
    pub use url::Url;

    /// Explicit factory. Defaults retain core timeouts, body and redirect caps.
    pub fn client(config: ClientConfig) -> Result<ReqwestClient> {
        isolate(move || ReqwestClient::new(&config).map_err(failed))
    }

    pub fn fetch<C: HttpClient + Send + Sync + 'static>(
        client: Arc<C>,
        launcher_url: &Url,
        options: FetchOptions,
    ) -> Result<FetchedSeed> {
        let url = launcher_url.clone();
        isolate(move || {
            crate::seeds::collect::fetch_seed(client.as_ref(), &url, &options).map_err(failed)
        })
    }

    pub fn fetch_direct<C: HttpClient + Send + Sync + 'static>(
        client: Arc<C>,
        kernel_url: &Url,
        options: FetchOptions,
    ) -> Result<FetchedSeed> {
        let url = kernel_url.clone();
        isolate(move || {
            crate::seeds::collect::fetch_direct_kernel(client.as_ref(), &url, &options)
                .map_err(failed)
        })
    }

    pub fn parse_url(input: &str) -> Result<Url> {
        let input = input.to_owned();
        isolate(move || crate::seeds::seed::parse_http_url(&input).map_err(failed))
    }

    pub fn resolve_url(reference: &str, base: Option<&Url>) -> Result<Url> {
        let (reference, base) = (reference.to_owned(), base.cloned());
        isolate(move || {
            crate::seeds::seed::resolve_kernel_url(&reference, base.as_ref()).map_err(failed)
        })
    }

    pub fn token(reference: &str, param: &str) -> Result<SeedToken> {
        let (reference, param) = (reference.to_owned(), param.to_owned());
        isolate(move || crate::seeds::seed::seed_from_reference(&reference, &param).map_err(failed))
    }

    pub fn parse(bytes: &[u8]) -> Result<Script> {
        let bytes = bytes.to_vec();
        isolate(move || crate::seeds::script::classify(&bytes).map_err(failed))
    }

    pub fn parse_launcher(bytes: &[u8]) -> Result<Launcher> {
        let bytes = bytes.to_vec();
        isolate(move || crate::seeds::script::parse_launcher(&bytes).map_err(failed))
    }

    pub fn parse_kernel(bytes: &[u8]) -> Result<KernelSummary> {
        let bytes = bytes.to_vec();
        isolate(move || crate::seeds::script::parse_kernel(&bytes).map_err(failed))
    }

    /// Explicit storage only. The caller supplies the seed identity/association;
    /// source syntax and kernel structure are validated before creating files.
    pub fn import(
        directory: &Path,
        seed: &SeedToken,
        launcher: Option<&[u8]>,
        kernel: &[u8],
    ) -> Result<StoreOutcome> {
        let (directory, seed, launcher, kernel) = (
            directory.to_owned(),
            seed.clone(),
            launcher.map(<[u8]>::to_vec),
            kernel.to_vec(),
        );
        isolate(move || {
            crate::seeds::script::parse_kernel(&kernel).map_err(failed)?;
            if let Some(launcher) = &launcher {
                crate::seeds::script::parse_launcher(launcher).map_err(failed)?;
            }
            crate::seeds::store::SeedStore::new(directory)
                .save(&seed, launcher.as_deref(), &kernel)
                .map_err(failed)
        })
    }

    pub fn sites() -> &'static [Site] {
        crate::seeds::sites::SITES
    }

    /// Lookup the registry entry, including its status. Does not fetch it.
    pub fn site(name: &str) -> Result<&'static Site> {
        let name = name.to_owned();
        isolate(move || crate::seeds::sites::find(&name).map_err(failed))
    }
}

pub mod headers {
    use super::{failed, isolate, Result};
    pub use crate::signals::payload::{
        CipherProfile, Generator as HeaderCodec, HeaderContext, HeaderReport, Headers, Payload,
        Profile,
    };

    pub fn codec(profile: Profile) -> Result<HeaderCodec> {
        isolate(move || HeaderCodec::new(profile).map_err(failed))
    }

    pub fn parse(text: &str) -> Result<Headers> {
        let text = text.to_owned();
        isolate(move || Headers::from_text(&text).map_err(failed))
    }

    pub fn encode(codec: &HeaderCodec, report: &HeaderReport) -> Result<Headers> {
        let (codec, report) = (codec.clone(), report.clone());
        isolate(move || codec.encode_report_headers(&report).map_err(failed))
    }

    pub fn decode(
        codec: &HeaderCodec,
        headers: &Headers,
        positional_count: usize,
        keysets: Vec<Vec<String>>,
    ) -> Result<HeaderReport> {
        let (codec, headers) = (codec.clone(), headers.clone());
        isolate(move || {
            codec
                .decode_report_headers(&headers, positional_count, keysets)
                .map_err(failed)
        })
    }

    pub fn encode_bytes(codec: &HeaderCodec, payload: &Payload) -> Result<Headers> {
        let (codec, payload) = (codec.clone(), payload.clone());
        isolate(move || codec.encode_headers(&payload).map_err(failed))
    }

    pub fn decode_bytes(codec: &HeaderCodec, headers: &Headers) -> Result<Payload> {
        let (codec, headers) = (codec.clone(), headers.clone());
        isolate(move || codec.decode_headers(&headers).map_err(failed))
    }

    pub fn encode_finalized(
        codec: &HeaderCodec,
        text: &str,
        context: Option<&HeaderContext>,
    ) -> Result<Headers> {
        let (codec, text, context) = (codec.clone(), text.to_owned(), context.cloned());
        isolate(move || {
            codec
                .encode_finalized_headers(&text, context.as_ref())
                .map_err(failed)
        })
    }

    pub fn decode_finalized(
        codec: &HeaderCodec,
        headers: &Headers,
    ) -> Result<(String, HeaderContext)> {
        let (codec, headers) = (codec.clone(), headers.clone());
        isolate(move || codec.decode_finalized_headers(&headers).map_err(failed))
    }
}

pub mod payload {
    use super::{failed, isolate, Result};
    pub use crate::signals::payload::{
        ChannelReport, CipherProfile, Container, Data, KeyedEntry, Payload, PayloadCodec,
        PayloadProfile, PositionalEntry, Report,
    };

    pub fn codec(profile: PayloadProfile) -> Result<PayloadCodec> {
        isolate(move || PayloadCodec::new(profile).map_err(failed))
    }

    pub fn encode(codec: &PayloadCodec, report: &Report) -> Result<String> {
        let (codec, report) = (codec.clone(), report.clone());
        isolate(move || codec.encode_report(&report).map_err(failed))
    }

    pub fn decode(
        codec: &PayloadCodec,
        text: &str,
        positional_count: usize,
        keysets: Vec<Vec<String>>,
    ) -> Result<Report> {
        let (codec, text) = (codec.clone(), text.to_owned());
        isolate(move || {
            codec
                .decode_report(&text, positional_count, keysets)
                .map_err(failed)
        })
    }

    pub fn encode_bytes(codec: &PayloadCodec, bytes: &[u8], seeds: [u32; 2]) -> Result<String> {
        let (codec, bytes) = (codec.clone(), bytes.to_vec());
        isolate(move || codec.encode_bytes(&bytes, seeds).map_err(failed))
    }

    pub fn decode_bytes(codec: &PayloadCodec, text: &str) -> Result<Payload> {
        let (codec, text) = (codec.clone(), text.to_owned());
        isolate(move || codec.decode(&text).map_err(failed))
    }
}
