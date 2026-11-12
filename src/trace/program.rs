//! `shape trace program`: trace every VM function of one capture and build
//! them into one JavaScript program.
//!
//! The walk is the corpus walk: the entry thread, then every function a
//! traced function creates with `vmCreateThread`. Each function is emitted
//! in program mode and nested where the function that creates it does, as
//! `function $THREAD_<state>_<pc>()`, reading the slots it shares through
//! scope (see [`crate::trace::program_nest`]); the program ends with a call of the
//! entry. A function the walk could not finish becomes one that throws,
//! naming why.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use crate::emitted_cleanup::{
    simplify_cleaned_emitted_property_writes,
    simplify_emitted_property_writes,
};
use crate::trace::cell_contents::{CellContents, CellGraph, Summary};
use crate::trace::corpus::{
    captured_slots, converge_cells, created_functions, load_artifact_with_policy, posted_functions,
    walk_reachable, Emitting, Function, KernelCachePolicy, Prior,
};
use crate::trace::program_nest::{build, check, Part};
use crate::trace::return_shapes::{self, Makers, Round, Walked};
use crate::{
    emit_program_function_in_namespace, EmitOptions, KernelArtifact, PrefixLimits,
    PrefixStopReason, ProgramFacts, PROGRAM_SELF, PROGRAM_THIS,
};

/// What [`link_program`] needs: the `shape trace program` options that change
/// the linked code. `jobs` and `quiet` are for library callers: the command
/// itself uses every core and keeps every walk diagnostic.
pub struct LinkOptions {
    pub seed: PathBuf,
    pub report_dir: Option<PathBuf>,
    pub steps: usize,
    pub dump_stages: Option<PathBuf>,
    pub thread_cleanup: bool,
    pub no_cleanup: bool,
    pub check_reuse: bool,
    pub overrides: crate::BranchOverrides,
    /// How many functions walk at once; `None` uses every core.
    pub jobs: Option<usize>,
    /// Suppress the walk and linking diagnostics. The program summary
    /// (`shape trace program`) is printed by the caller either way.
    pub quiet: bool,
}

/// The linked program and the facts the command reports; no file is written.
pub struct LinkedProgram {
    pub code: String,
    /// Functions whose walk completed and that the program contains.
    pub traced: usize,
    /// Stopped functions by key and why, in function order.
    pub stopped: Vec<(String, String)>,
    /// The kernel globals the code still reads, sorted.
    pub kernel_globals: Vec<String>,
    pub stats: LinkStats,
}

/// Cleanup counts `shape trace program` prints after linking.
#[derive(Clone, Copy, Default)]
pub struct LinkStats {
    pub thread: ThreadCleanupStats,
}

/// The program of `linked`'s functions, the roots and the functions posted
/// to the VM Worker at the top.
fn program_code(
    roots: &[Function],
    linked: &BTreeMap<Function, Linked>,
    posted: &BTreeSet<Function>,
) -> Result<String, String> {
    let parts = linked
        .iter()
        .map(|(&function, outcome)| {
            let part = match outcome {
                Linked::Traced { unit, .. } => Part::Traced(unit),
                Linked::Stopped(reason) => Part::Stopped(reason),
            };
            (function, part)
        })
        .collect();
    let mut roots = roots.to_vec();
    roots.extend(posted.iter().copied());
    build(&roots, &parts)
}

/// One VM function of the program: its emitted code, or why it has none.
#[derive(Clone)]
enum Linked {
    Traced {
        /// Its code and facts, checked (see [`check`]) on the walk's
        /// thread, or why it cannot be nested.
        unit: Result<(String, ProgramFacts), String>,
        /// The names the code reads and does not declare.
        free: Vec<String>,
        thread_cleanup: ThreadCleanupStats,
    },
    Stopped(String),
}

impl Linked {
    /// What the program takes from it: its code, or why it has none.
    fn outcome(&self) -> Result<&str, &str> {
        match self {
            Linked::Traced { unit, .. } => unit
                .as_ref()
                .map(|(code, _)| code.as_str())
                .map_err(String::as_str),
            Linked::Stopped(why) => Err(why.as_str()),
        }
    }
}

/// The functions whose code `a` and `b` give differently, or that only one
/// of them has.
fn differing(a: &BTreeMap<Function, Linked>, b: &BTreeMap<Function, Linked>) -> Vec<Function> {
    a.keys()
        .chain(b.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|function| {
            a.get(function).map(Linked::outcome) != b.get(function).map(Linked::outcome)
        })
        .copied()
        .collect()
}

enum ProgramEmitError {
    Stopped(String),
    PassFailed(String),
}

#[derive(Clone, Copy, Default)]
pub struct ThreadCleanupStats {
    pub properties: usize,
    pub arrays: usize,
    pub methods: usize,
    pub loops: usize,
    pub blocks: usize,
    pub calls: usize,
    pub removed: usize,
    pub remaining: usize,
}

/// Decode `vmDecodeString` calls whose arguments became literals during
/// cleanup; a decline leaves the code as it is.
fn fold_program_strings(artifact: &KernelArtifact, code: String) -> String {
    let emitted = crate::EmittedJavaScript {
        code: code.clone(),
        statement_map: Vec::new(),
        node_map: Default::default(),
        object_parameters: Default::default(),
    };
    match artifact.fold_emitted_vm_strings(emitted) {
        Ok(folded) if folded.report.folded != 0 => folded.emitted.code,
        _ => code,
    }
}

/// Trace and link one seed into a program without writing it.
pub fn link_program(args: &LinkOptions) -> Result<LinkedProgram, String> {
    let source = fs::read_to_string(&args.seed)
        .map_err(|error| format!("cannot read {}: {error}", args.seed.display()))?;
    link_source(&source, args)
}

/// [`link_program`] for source already in memory. `args.seed` only names the
/// seed: in messages and as the artifact file name under `args.report_dir`.
pub fn link_source(source: &str, args: &LinkOptions) -> Result<LinkedProgram, String> {
    let policy = crate::trace::corpus::legacy_cache_policy(args.report_dir.as_deref());
    link_source_with_cache(source, args, &policy)
}

/// Trace in-memory source with explicit kernel caching. `args.report_dir` is
/// ignored; `args.seed` still supplies the explicit cache entry's file name.
pub fn link_source_with_cache(
    source: &str,
    args: &LinkOptions,
    policy: &KernelCachePolicy,
) -> Result<LinkedProgram, String> {
    let started = Instant::now();
    let artifact = load_artifact_with_policy(&args.seed, source, policy)?;
    link_source_prepared(source, args, &artifact, started)
}

/// Trace with an already loaded kernel after checking exact raw-source identity.
/// No cache or kernel JSON is read, and the kernel is not recompiled. Raw source
/// remains necessary for whole-program finishing. `args.report_dir` is ignored.
pub fn link_source_with_artifact(
    source: &str,
    args: &LinkOptions,
    artifact: &KernelArtifact,
) -> Result<LinkedProgram, String> {
    artifact
        .verify_source_identity(source)
        .map_err(|error| error.to_string())?;
    link_source_prepared(source, args, artifact, Instant::now())
}

fn link_source_prepared(
    source: &str,
    args: &LinkOptions,
    artifact: &KernelArtifact,
    started: Instant,
) -> Result<LinkedProgram, String> {
    let source = source.to_owned();
    let prepared = std::sync::Arc::new(
        artifact
            .prepare_with_limits(PrefixLimits {
                max_dispatch_steps: args.steps,
                ..PrefixLimits::default()
            })
            .map_err(|error| format!("not a supported kernel artifact: {error}"))?,
    );
    let entry = artifact
        .entry_facts()
        .ok_or("the kernel artifact has no entry facts")?;
    let descriptors: std::sync::Arc<[_]> = artifact.descriptor_records().into();
    let captures = crate::trace::thread::thread_captures(&descriptors);
    let initial_slots: std::sync::Arc<[u32]> = artifact.entry_initial_slots().into();

    let entry = Function {
        descriptor: None,
        pc: entry.pc as usize,
        state: entry.state as usize,
    };
    let roots = crate::trace::corpus::roots(entry, &descriptors);
    // Every function on whichever core is free, each started the moment a
    // walk creates it: a function's walk and code do not depend on when it
    // runs, and the program is linked in function order.
    let jobs = args
        .jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from));
    // Each function's walk of a round, linked: a walk with the contents and
    // answers it had before traces the same (see [`return_shapes`]).
    type Walks = BTreeMap<Function, std::sync::Arc<Walked>>;
    /// The functions' code, the functions posted, the walks' cell summaries,
    /// how many functions the round walked itself and how many of those it
    /// walked again with their ancestors' contents.
    type Linking = (
        BTreeMap<Function, Linked>,
        BTreeSet<Function>,
        Prior,
        usize,
        usize,
    );
    let overrides = std::sync::Arc::new(args.overrides.clone());
    // `emitted` is the code `kept`'s walks gave: a function whose walk is
    // kept keeps its code, emitted from that same walk.
    let walk_program = |contents: &CellContents,
                        makers: &std::sync::Arc<Makers>,
                        kept: &Walks,
                        emitted: &BTreeMap<Function, Linked>|
     -> Result<(Walks, Linking), String> {
        let round = std::sync::Arc::new(
            Round::new(
                prepared.clone(),
                descriptors.clone(),
                initial_slots.clone(),
                captures.clone(),
                args.steps,
                std::sync::Arc::new(contents.clone()),
                makers.clone(),
                kept,
            )
            .with_overrides(overrides.clone()),
        );
        let posted = std::sync::Mutex::new(BTreeSet::<Function>::new());
        let prior = std::sync::Mutex::new(Prior::new());
        let linked = walk_reachable(&roots, jobs, None, |function, announce| {
            let walked = round.walk(function, announce);
            let result = match &walked.report {
                Err(error) => return Ok((Linked::Stopped(error.clone()), Vec::new())),
                Ok(result) => result,
            };
            let created = created_functions(result);
            announce(&created);
            prior
                .lock()
                .map_err(|_| "a walk's cell summaries were poisoned".to_string())?
                .insert(
                    function,
                    (
                        Summary::of(result, created.clone()),
                        result.cells.clone(),
                        result.cell_reads.clone(),
                        walked.contents.clone(),
                    ),
                );
            posted
                .lock()
                .map_err(|_| "a walk's posted functions were poisoned".to_string())?
                .extend(posted_functions(result));
            let unchanged = kept
                .get(&function)
                .is_some_and(|kept| std::sync::Arc::ptr_eq(kept, &walked));
            let outcome = if let Some(outcome) = emitted.get(&function).filter(|_| unchanged) {
                outcome.clone()
            } else if result.stop == PrefixStopReason::Completed {
                match emit(
                    artifact,
                    result,
                    function,
                    args.dump_stages.as_deref(),
                    args.thread_cleanup,
                    !args.no_cleanup,
                ) {
                    Ok((code, facts, thread_cleanup)) => {
                        match crate::tracing::scope::free_and_bound_names(&code) {
                            Ok((free, bound)) => Linked::Traced {
                                unit: check(&facts, &free, &bound).map(|()| (code, facts)),
                                free,
                                thread_cleanup,
                            },
                            Err(error) => Linked::Traced {
                                unit: Err(error),
                                free: Vec::new(),
                                thread_cleanup: ThreadCleanupStats::default(),
                            },
                        }
                    }
                    Err(ProgramEmitError::Stopped(error)) => Linked::Stopped(error),
                    Err(ProgramEmitError::PassFailed(error)) => return Err(error),
                }
            } else {
                Linked::Stopped(result.stop.to_string())
            };
            Ok((outcome, created))
        })?;
        let posted = posted
            .into_inner()
            .map_err(|_| "a walk's posted functions were poisoned".to_string())?;
        let prior = prior
            .into_inner()
            .map_err(|_| "a walk's cell summaries were poisoned".to_string())?;
        // The walks of functions the program reaches.
        let walks = round
            .walks()
            .into_iter()
            .filter(|(function, _)| linked.contains_key(function))
            .collect();
        Ok((
            walks,
            (linked, posted, prior, round.walked(), round.given()),
        ))
    };
    // A call of a VM function every return of which gives a VM function
    // made is a function, to tests; a walk asked of a function walks it
    // first. Only a call of itself is told the round before's answer: the
    // rounds end when a round finds the makers it was told.
    let mut makers = std::sync::Arc::new(Makers::new());
    let mut kept = Walks::new();
    let mut linking: Option<Linking> = None;
    for _ in 0..return_shapes::MAX_ROUNDS {
        let emitted = linking
            .as_ref()
            .map(|(linked, ..)| linked.clone())
            .unwrap_or_default();
        let (walks, linked) = walk_program(&CellContents::new(), &makers, &kept, &emitted)?;
        let found = return_shapes::makers(&walks);
        let settled = walks
            .values()
            .all(|walked| return_shapes::stand(&walked.answers, &found));
        kept = walks;
        linking = Some(linked);
        makers = std::sync::Arc::new(found);
        if settled {
            break;
        }
    }
    let stops = |linked: &BTreeMap<Function, Linked>| {
        linked
            .values()
            .filter(|outcome| matches!(outcome, Linked::Stopped(_)))
            .count()
    };
    let (mut linked, mut posted, prior, _, given) = linking.ok_or("no walk ran")?;
    // A function walked with what its ancestors put in its cells stands if
    // that is what every holder of them puts there, wherever it asked.
    let unsettled = given > 0 && {
        let summaries: BTreeMap<Function, Summary> = prior
            .iter()
            .map(|(&function, (summary, ..))| (function, summary.clone()))
            .collect();
        let strict = CellGraph::of(&summaries, |function| {
            captured_slots(function, &descriptors)
        })
        .contents(&summaries, true);
        prior.iter().any(|(function, (_, _, reads, used))| {
            let every = strict.get(function).cloned().unwrap_or_default();
            !used.is_empty() && *used != every && !reads.same(used, &every)
        })
    };
    if given > 0 && !args.quiet {
        eprintln!(
            "cell contents: {given} functions walked again with their ancestors' contents{}",
            if unsettled {
                ", not what every holder puts there"
            } else {
                ""
            }
        );
    }
    // A VM jump through a cell another function writes needs what every
    // holder of the cell puts there (see `crate::trace::cell_contents`): a
    // program that still stops, or whose contents did not hold, is walked
    // again with them.
    if stops(&linked) > 0 || unsettled {
        let (contents, _) = converge_cells(
            &prepared,
            &roots,
            &descriptors,
            &initial_slots,
            &captures,
            args.steps,
            jobs,
            &Emitting {
                artifact,
                cleanup: false,
            },
            prior,
            Some(return_shapes::told(makers.clone())),
        )?;
        if !contents.is_empty() {
            // The walks without contents stand for a function whose contents
            // are the same wherever its walk read them (see
            // `return_shapes::Walked::stands_for`).
            let (_, (again, again_posted, _, walked, _)) =
                walk_program(&contents, &makers, &kept, &linked)?;
            if !args.quiet {
                eprintln!(
                    "cell contents: {walked} functions walked again, {} kept",
                    again.len().saturating_sub(walked)
                );
            }
            if args.check_reuse {
                let (_, (fresh, fresh_posted, ..)) =
                    walk_program(&contents, &makers, &Walks::new(), &BTreeMap::new())?;
                let differ = differing(&again, &fresh);
                if !differ.is_empty() || again_posted != fresh_posted {
                    return Err(format!(
                        "--check-reuse: a kept walk differs from a fresh one: {}",
                        differ
                            .iter()
                            .map(|function| format!("{function:?}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if !args.quiet {
                    eprintln!("--check-reuse: every kept walk is the fresh walk");
                }
            }
            // The walks with the contents stand if they stop less and still
            // make a program (no function made inside itself, say).
            if stops(&again) < stops(&linked) && program_code(&roots, &again, &again_posted).is_ok()
            {
                (linked, posted) = (again, again_posted);
            }
        }
    }

    let code = program_code(&roots, &linked, &posted)?;
    // Every parse from here on reads code this command printed itself.
    let _own_output = crate::analysis::OwnOutput::enter();
    // With SHAPE_DEOB_PROFILE set, each stage is timed from the one before;
    // `00-linked.js` covers preparation, tracing, emission and linking.
    let stage_clock = std::cell::Cell::new(started);
    let dump_stage = |filename: &'static str, code: &str| -> Result<(), String> {
        if crate::profile::enabled() {
            let now = Instant::now();
            crate::profile::record(filename, now - stage_clock.replace(now));
        }
        let Some(directory) = args.dump_stages.as_ref() else {
            return Ok(());
        };
        fs::create_dir_all(directory)
            .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
        let path = directory.join(filename);
        fs::write(&path, code).map_err(|error| format!("cannot write {}: {error}", path.display()))
    };
    dump_stage("00-linked.js", &code)?;
    // The cleanup passes reparse the whole program each: a run that checks
    // the tracing skips them.
    let code = if args.no_cleanup {
        code
    } else {
        // Every thread arrived cleaned (`clean_program_thread`): the whole
        // program is named and tidied as one.
        let function_names: Vec<_> = linked
            .keys()
            .enumerate()
            .map(|(number, function)| (function_namespace(*function), number))
            .collect();
        crate::emitted_cleanup::finish_program(code, &function_names, artifact.native_functions(), &source, &dump_stage)?
    };

    let code = if args.thread_cleanup {
        let allocator = oxc_allocator::Allocator::default();
        let parsed = crate::analysis::parse_program(&allocator, &code)?;
        crate::analysis::codegen(&parsed)
    } else {
        code
    };

    // Cleaned threads arrive with their arrays laid out already.
    let code = if args.no_cleanup {
        let compact_started = crate::profile::start();
        let code = crate::emitted_cleanup::compact_arrays(&code)?;
        crate::profile::done("14-compact-arrays", compact_started);
        code
    } else {
        code
    };
    // A vm_sub_* helper that survived every cleanup stage still carries a
    // registry name. This last pass turns its calls into arrows and deletes it;
    // it runs after linking because earlier stages still expect those helpers.
    let mut code = code;
    if !args.no_cleanup {
        // One pass per nesting level: a helper called inside another helper's
        // argument has an edit span inside the other's, so they cannot both be
        // rewritten in one round.
        for _ in 0..6 {
            let removed = crate::emitted_cleanup::inline_remaining_vm_subs(&code, false)
                .map_err(|error| format!("cannot remove remaining VM subroutine helpers: {error}"))?;
            if removed.removed == 0 {
                break;
            }
            code = removed.code;
        }
        // Anything still left only survived because the arrow costs more than
        // the declaration it replaces; a few bytes is cheaper than a registry
        // name in the output.
        for _ in 0..6 {
            let removed = crate::emitted_cleanup::inline_remaining_vm_subs(&code, true)
                .map_err(|error| format!("cannot remove remaining VM subroutine helpers: {error}"))?;
            if removed.removed == 0 {
                break;
            }
            code = removed.code;
        }
    }

    // Each thread's code parsed when its header went in, and linking only
    // fills markers: the whole output is checked in debug builds or with
    // `SHAPE_CHECK_EDITS` set.
    if cfg!(debug_assertions) || std::env::var_os("SHAPE_CHECK_EDITS").is_some() {
        let allocator = oxc_allocator::Allocator::default();
        crate::analysis::parse_program(&allocator, &code).map_err(|error| format!("program output does not parse: {error}"))?;
    }

    let stopped: Vec<(String, String)> = linked
        .iter()
        .filter_map(|(function, outcome)| match outcome {
            Linked::Stopped(reason) => Some((function.key(), reason.clone())),
            Linked::Traced { .. } => None,
        })
        .collect();
    // A function reads only the kernel's globals and what its creator gives
    // it (see [`check`]).
    let free: BTreeSet<&String> = linked
        .values()
        .filter_map(|outcome| match outcome {
            Linked::Traced { free, .. } => Some(free),
            Linked::Stopped(_) => None,
        })
        .flatten()
        .filter(|name| !name.starts_with("$KEY_") && *name != "arguments")
        .filter(|name| !JAVASCRIPT_GLOBALS.contains(&name.as_str()))
        .collect();
    let kernel_globals: Vec<String> = free.into_iter().cloned().collect();
    let mut stats = LinkStats::default();
    if args.thread_cleanup {
        let mut thread = ThreadCleanupStats::default();
        for outcome in linked.values() {
            if let Linked::Traced { thread_cleanup, .. } = outcome {
                thread.calls += thread_cleanup.calls;
                thread.properties += thread_cleanup.properties;
                thread.arrays += thread_cleanup.arrays;
                thread.methods += thread_cleanup.methods;
                thread.loops += thread_cleanup.loops;
                thread.blocks += thread_cleanup.blocks;
                thread.removed += thread_cleanup.removed;
                thread.remaining += thread_cleanup.remaining;
            }
        }
        stats.thread = thread;
    }
    Ok(LinkedProgram {
        code,
        traced: linked.len() - stopped.len(),
        stopped,
        kernel_globals,
        stats,
    })
}

/// Standard globals the program may read; any other free name is the
/// kernel's.
const JAVASCRIPT_GLOBALS: &[&str] = &[
    "Array",
    "Boolean",
    "Date",
    "Error",
    "Function",
    "JSON",
    "Math",
    "Number",
    "Object",
    "Promise",
    "RangeError",
    "Reflect",
    "RegExp",
    "String",
    "Symbol",
    "TypeError",
    "undefined",
    "NaN",
    "Infinity",
    "globalThis",
];

/// Emit one completed walk in program mode, cleaned up as `shape trace
/// thread` does.
fn emit(
    artifact: &KernelArtifact,
    result: &crate::PrefixTraceReport,
    function: Function,
    dump_stages: Option<&Path>,
    thread_cleanup: bool,
    pipeline: bool,
) -> Result<(String, ProgramFacts, ThreadCleanupStats), ProgramEmitError> {
    let options = EmitOptions {
        simplify_bindings: true,
        function_name: PROGRAM_SELF.into(),
        ..EmitOptions::default()
    };
    let namespace = function_namespace(function);
    let trace = &result.trace;
    let (emitted, mut facts) = emit_program_function_in_namespace(trace, &options, &namespace)
        .map_err(|error| ProgramEmitError::Stopped(format!("cannot emit: {error}")))?;
    let dump = |name: &str, code: &str| -> Result<(), ProgramEmitError> {
        let Some(directory) = dump_stages else {
            return Ok(());
        };
        let directory = directory.join("functions").join(&namespace);
        fs::create_dir_all(&directory).map_err(|error| {
            ProgramEmitError::PassFailed(format!("cannot create {}: {error}", directory.display()))
        })?;
        let path = directory.join(name);
        fs::write(&path, code).map_err(|error| {
            ProgramEmitError::PassFailed(format!("cannot write {}: {error}", path.display()))
        })
    };
    dump("00-emitted.js", &emitted.code)?;
    let folded = artifact
        .fold_emitted_vm_strings(emitted)
        .map_err(|error| ProgramEmitError::Stopped(format!("string folding: {error}")))?;
    dump("01-vm-strings.js", &folded.emitted.code)?;
    let emitted = simplify_emitted_property_writes(&folded.emitted)
        .map_err(|error| ProgramEmitError::Stopped(format!("property writes: {error}")))?;
    dump("02-property-writes.js", &emitted.code)?;
    let emitted = simplify_cleaned_emitted_property_writes(&emitted)
        .map_err(|error| ProgramEmitError::Stopped(format!("array constructions: {error}")))?;
    dump("03-array-constructions.js", &emitted.code)?;
    // The receiver is `this` from here (see [`PROGRAM_THIS`]).
    let uses = emitted.code.matches(PROGRAM_THIS).count();
    if uses != facts.this_names {
        return Err(ProgramEmitError::Stopped(format!(
            "{PROGRAM_THIS} is written {uses} times after cleanup, not the {} the emitter wrote",
            facts.this_names
        )));
    }
    let code = emitted.code.replace(PROGRAM_THIS, "this");
    dump("04-receiver-this.js", &code)?;
    let mut stats = ThreadCleanupStats::default();
    let code = if thread_cleanup {
        let mut cleaned =
            cleanup_program_thread(&code, &mut facts).map_err(ProgramEmitError::PassFailed)?;
        // Helper expansion and snapshot propagation can expose literal decoder
        // operands. Retry only threads whose initial string fold declined.
        if folded.report.declined != 0 {
            let retry = artifact
                .fold_emitted_vm_strings(crate::EmittedJavaScript {
                    code: cleaned.code.clone(),
                    // Thread cleanup already invalidated source coordinates.
                    statement_map: Vec::new(),
                    node_map: Default::default(),
                    object_parameters: Default::default(),
                })
                .map_err(|error| {
                    ProgramEmitError::Stopped(format!(
                        "string folding after thread cleanup: {error}"
                    ))
                })?;
            if retry.report.folded != 0 {
                let mut next = cleanup_program_thread(&retry.emitted.code, &mut facts)
                    .map_err(ProgramEmitError::PassFailed)?;
                next.rewritten_property_writes += cleaned.rewritten_property_writes;
                next.folded_arrays += cleaned.folded_arrays;
                next.simplified_method_calls += cleaned.simplified_method_calls;
                next.reconstructed_loops += cleaned.reconstructed_loops;
                next.flattened_blocks += cleaned.flattened_blocks;
                next.inlined_helper_calls += cleaned.inlined_helper_calls;
                next.removed_helpers += cleaned.removed_helpers;
                cleaned = next;
            }
        }
        stats = ThreadCleanupStats {
            properties: cleaned.rewritten_property_writes,
            arrays: cleaned.folded_arrays,
            methods: cleaned.simplified_method_calls,
            loops: cleaned.reconstructed_loops,
            blocks: cleaned.flattened_blocks,
            calls: cleaned.inlined_helper_calls,
            removed: cleaned.removed_helpers,
            remaining: cleaned.remaining_helpers,
        };
        dump("05-thread-cleaned.js", &cleaned.code)?;
        cleaned.code
    } else {
        code
    };
    // Each thread comes out cleaned: linking only joins finished pieces.
    let code = if pipeline {
        let dump_dir = dump_stages.map(|directory| directory.join("functions").join(&namespace));
        let cleaned = clean_program_thread(&code, &mut facts, artifact, dump_dir.as_deref())
            .map_err(ProgramEmitError::PassFailed)?;
        dump("06-thread-pipeline.js", &cleaned)?;
        crate::emitted_cleanup::compact_arrays(&cleaned).map_err(ProgramEmitError::PassFailed)?
    } else {
        code
    };
    Ok((code, facts, stats))
}

/// Validate emitter identities before cleanup, then count the surviving self
/// references. Cleanup can remove a snapshot or expand an immutable alias; the
/// linker must check the resulting count, while keeping all placement facts.
fn cleanup_program_thread(
    code: &str,
    facts: &mut ProgramFacts,
) -> Result<crate::emitted_cleanup::ThreadCleanup, String> {
    let validate_scope = |code: &str| -> Result<(), String> {
        let (free, bound) = crate::tracing::scope::free_and_bound_names(code)?;
        check(facts, &free, &bound)?;
        for slot in facts.threads.iter().flat_map(|thread| thread.cells.iter()) {
            if !facts.captured.contains(slot) && !bound.contains(&format!("$KEY_{slot}")) {
                return Err(format!(
                    "thread cleanup: exported slot $KEY_{slot} has no local declaration"
                ));
            }
        }
        Ok(())
    };
    validate_scope(code)?;
    if code.matches(PROGRAM_SELF).count() != facts.self_names + 1 {
        return Err("thread cleanup: emitted self-reference count does not match its facts".into());
    }
    let options = crate::emitted_cleanup::ThreadCleanupOptions {
        shared_slots: facts
            .captured
            .iter()
            .copied()
            .chain(
                facts
                    .threads
                    .iter()
                    .flat_map(|thread| thread.cells.iter().copied()),
            )
            .collect(),
    };
    let cleaned = crate::emitted_cleanup::cleanup_emitted_thread(code, &options)
        .map_err(|error| format!("thread cleanup: {error}"))?;
    validate_scope(&cleaned.code)?;
    facts.self_names = cleaned
        .code
        .matches(PROGRAM_SELF)
        .count()
        .checked_sub(1)
        .ok_or("thread cleanup removed the function identity")?;
    Ok(cleaned)
}


/// What a thread's cleanup stands in for the code it cannot see: the
/// program around it, which can read and write every key it shares.
const OUTSIDE: &str = "$SHAPE_OUTSIDE";

/// Run the cleanup stages on one thread before linking. It is cleaned as a
/// unit that models the rest of the program by its keys: each key it shares
/// with its creator is declared outside with an unknown value and written
/// by [`OUTSIDE`], and each function it creates reads and writes the keys
/// it is given. Only its local keys are then plain variables to the
/// passes. The markers go back to `function NAME() {}` afterwards.
fn clean_program_thread(
    code: &str,
    facts: &mut ProgramFacts,
    artifact: &KernelArtifact,
    dump_dir: Option<&Path>,
) -> Result<String, String> {
    use oxc_ast::ast::Statement;
    use oxc_ast_visit::{walk, Visit};
    use oxc_span::GetSpan;
    // The unit is code this command printed: its parses skip the source
    // scan (this runs on a walk thread, so the guard is set here).
    let _own_output = crate::analysis::OwnOutput::enter();
    let shared: Vec<String> = facts.captured.iter().map(|slot| format!("$KEY_{slot}")).collect();
    let mut unit = String::new();
    for name in &shared {
        unit.push_str(&format!("let {name} = {OUTSIDE}();\n"));
    }
    let mut thread = code.to_string();
    for child in &facts.threads {
        let marker = format!("function {}() {{}}", child.name);
        let Some(at) = thread.find(&marker) else {
            return Err(format!("thread pipeline: no marker for {}", child.name));
        };
        let uses: String = child.cells.iter().map(|slot| format!(" $KEY_{slot} = {OUTSIDE}($KEY_{slot});")).collect();
        thread.replace_range(at..at + marker.len(), &format!("function {}() {{{uses} }}", child.name));
    }
    unit.push_str(&thread);
    unit.push_str(&format!("\nfunction {OUTSIDE}() {{\n"));
    for name in &shared {
        unit.push_str(&format!("\t{name} = {OUTSIDE}({name});\n"));
    }
    unit.push_str(&format!("}}\n{OUTSIDE}({PROGRAM_SELF});\n"));
    // With SHAPE_DEOB_PROFILE set, each stage is timed under `t.<stage>`
    // (`ts.` for threads under 5 KB).
    let clock = std::cell::Cell::new(Instant::now());
    let small = code.len() < 5000;
    let stage = |name: &'static str, stage_code: &str| -> Result<(), String> {
        // With --dump-stages, each stage's unit goes to the thread's folder.
        if let Some(directory) = dump_dir {
            let directory = directory.join("pipeline");
            fs::create_dir_all(&directory).map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
            fs::write(directory.join(name), stage_code).map_err(|error| format!("cannot write {name}: {error}"))?;
        }
        if crate::profile::enabled() {
            let now = Instant::now();
            let label: &'static str = Box::leak(format!("{}{name}", if small { "ts." } else { "t." }).into_boxed_str());
            crate::profile::record(label, now - clock.replace(now));
        }
        Ok(())
    };
    let cleaned = crate::emitted_cleanup::clean_function(unit, &|code| fold_program_strings(artifact, code), &stage)?;
    // Take the thread back out, its markers empty again.
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, &cleaned)?;
    let function = program
        .body
        .iter()
        .find_map(|statement| match statement {
            Statement::FunctionDeclaration(function)
                if function.id.as_ref().is_some_and(|id| id.name == PROGRAM_SELF) =>
            {
                Some(function)
            }
            _ => None,
        })
        .ok_or("thread pipeline: the cleaned unit lost the thread")?;
    struct Markers<'n> {
        names: &'n BTreeSet<&'n str>,
        found: Vec<(oxc_span::Span, String)>,
        /// Per marker, the references its stand-in stores name, in order.
        keys: BTreeMap<String, Vec<oxc_semantic::ReferenceId>>,
        /// Markers holding more than their stand-in stores.
        foreign: Vec<String>,
    }
    impl<'a> Visit<'a> for Markers<'_> {
        fn visit_function(&mut self, function: &oxc_ast::ast::Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
            if let Some(id) = &function.id {
                if self.names.contains(id.name.as_str()) {
                    self.found.push((function.span, id.name.to_string()));
                    let stored = self.keys.entry(id.name.to_string()).or_default();
                    for statement in function.body.iter().flat_map(|body| body.statements.iter()) {
                        if let Statement::ExpressionStatement(expression) = statement {
                            if let oxc_ast::ast::Expression::AssignmentExpression(assignment) = &expression.expression {
                                if let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left {
                                    if let Some(reference) = target.reference_id.get() {
                                        stored.push(reference);
                                        continue;
                                    }
                                }
                            }
                        }
                        // Anything a pass moved in would go with the marker.
                        self.foreign.push(id.name.to_string());
                    }
                    return;
                }
            }
            walk::walk_function(self, function, flags);
        }
    }
    // A pass may have renamed a shared key's stand-in declaration (a loop
    // counter named `i`): its reads in the thread go back to the key's name,
    // which the creator declares.
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let mut renamed: Vec<(oxc_span::Span, String)> = Vec::new();
    let declared: Vec<&oxc_ast::ast::VariableDeclaration<'_>> = program
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::VariableDeclaration(declaration) => Some(&**declaration),
            _ => None,
        })
        .collect();
    if declared.len() != shared.len() {
        return Err("thread pipeline: the shared key declarations changed".into());
    }
    for (declaration, key) in declared.iter().zip(&shared) {
        let [declarator] = declaration.declarations.as_slice() else {
            return Err("thread pipeline: a shared key declaration was merged".into());
        };
        let oxc_ast::ast::BindingPattern::BindingIdentifier(binding) = &declarator.id else {
            return Err("thread pipeline: a shared key declaration was destructured".into());
        };
        if binding.name == key.as_str() {
            continue;
        }
        let symbol = binding.symbol_id.get().ok_or("thread pipeline: unbound shared key")?;
        for reference in scoping.get_resolved_references(symbol) {
            let span = nodes.kind(reference.node_id()).span();
            if function.span.start <= span.start && span.end <= function.span.end {
                renamed.push((span, key.clone()));
            }
        }
    }
    let names: BTreeSet<&str> = facts.threads.iter().map(|thread| thread.name.as_str()).collect();
    let mut markers = Markers { names: &names, found: Vec::new(), keys: BTreeMap::new(), foreign: Vec::new() };
    if let Some(body) = &function.body {
        markers.visit_function_body(body);
    }
    if let Some(name) = markers.foreign.first() {
        return Err(format!("thread pipeline: cleanup moved code into the marker of {name}"));
    }
    // A key handed to a child keeps the name the child reads it by: when a
    // pass renamed it here, its declaration and reads go back.
    for child in &facts.threads {
        let stored = markers.keys.get(&child.name).cloned().unwrap_or_default();
        if stored.len() != child.cells.len() {
            return Err(format!("thread pipeline: the keys {} is given changed", child.name));
        }
        for (reference, slot) in stored.iter().zip(&child.cells) {
            let key = format!("$KEY_{slot}");
            let Some(symbol) = scoping.get_reference(*reference).symbol_id() else {
                continue;
            };
            if scoping.symbol_name(symbol) == key {
                continue;
            }
            // Declared among the outside stand-ins (a pass renamed one, a
            // loop counter say): only this function's text is written out.
            let declared_at = scoping.symbol_span(symbol);
            if function.span.start <= declared_at.start && declared_at.end <= function.span.end {
                renamed.push((declared_at, key.clone()));
            }
            for reference in scoping.get_resolved_references(symbol) {
                let span = nodes.kind(reference.node_id()).span();
                if function.span.start <= span.start && span.end <= function.span.end {
                    renamed.push((span, key.clone()));
                }
            }
        }
    }
    if markers.found.len() != names.len() {
        return Err(format!(
            "thread pipeline: {} of {} markers survived cleanup",
            markers.found.len(),
            names.len()
        ));
    }
    let span = function.span();
    let mut out = String::with_capacity((span.end - span.start) as usize);
    let mut at = span.start as usize;
    let mut edits: Vec<(oxc_span::Span, String)> = markers
        .found
        .into_iter()
        .map(|(span, name)| (span, format!("function {name}() {{}}")))
        .collect();
    // Reads inside an emptied marker go with it (the overlap check below).
    edits.extend(renamed);
    edits.sort_by_key(|(span, _)| span.start);
    let mut kept: Vec<(oxc_span::Span, String)> = Vec::new();
    for edit in edits {
        if kept.last().is_some_and(|(last, _)| edit.0.start < last.end) {
            continue;
        }
        kept.push(edit);
    }
    for (span, text) in kept {
        out.push_str(&cleaned[at..span.start as usize]);
        out.push_str(&text);
        at = span.end as usize;
    }
    out.push_str(&cleaned[at..span.end as usize]);
    out.push('\n');
    if out.contains(OUTSIDE) {
        return Err("thread pipeline: the thread still names the outside stand-in".into());
    }
    facts.self_names = out
        .matches(PROGRAM_SELF)
        .count()
        .checked_sub(1)
        .ok_or("thread pipeline removed the function identity")?;
    Ok(out)
}

fn function_namespace(function: Function) -> String {
    match function.descriptor {
        Some(descriptor) => format!("{}_{}_d{descriptor}", function.state, function.pc),
        None => format!("{}_{}_entry", function.state, function.pc),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_cleanup_preserves_markers_and_updates_self_reference_facts() {
        let source = "function $THREAD_SELF() { function $THREAD_0_1() {} let $SSA_1 = $THREAD_SELF; return [$SSA_1, $SSA_1]; }";
        let mut facts = ProgramFacts {
            self_names: 1,
            ..ProgramFacts::default()
        };
        let code = cleanup_program_thread(source, &mut facts).unwrap().code;
        assert!(code.contains("function $THREAD_0_1() {}"), "{code}");
        assert_eq!(facts.self_names, 2);
        assert_eq!(code.matches(PROGRAM_SELF).count(), 3);
        let mut bad_facts = ProgramFacts::default();
        assert!(cleanup_program_thread(source, &mut bad_facts).is_err());
    }

    #[test]
    fn cleanup_checks_inherited_and_exported_cell_declarations_before_and_after() {
        let source = "function $THREAD_SELF() { let $KEY_7=readInitialValue(); function $THREAD_1_2() {} let $SSA_1=$KEY_7; $THREAD_1_2(); $KEY_9=$SSA_1; return $KEY_9; }";
        let mut facts = ProgramFacts {
            captured: BTreeSet::from([9]),
            threads: vec![crate::ProgramThread {
                name: "$THREAD_1_2".into(),
                descriptor: 0,
                pc: 2,
                state: 1,
                cells: vec![7],
            }],
            ..ProgramFacts::default()
        };
        let cleaned = cleanup_program_thread(source, &mut facts).unwrap();
        assert!(
            cleaned.code.contains("let $SSA_1 = $KEY_7"),
            "{}",
            cleaned.code
        );
        assert!(!cleaned.code.contains("let $KEY_9") && !cleaned.code.contains("var $KEY_9"));
        // A known literal at capture needs no snapshot cell. It still must
        // never become a live shared-cell read after the child can change it.
        let literal = source.replace("readInitialValue()", "1");
        let literal = cleanup_program_thread(&literal, &mut facts).unwrap().code;
        assert!(literal.contains("$KEY_9 = 1"), "{literal}");
        assert!(!literal.contains("$KEY_9 = $KEY_7"), "{literal}");
        let shadowed = "function $THREAD_SELF(){let $KEY_9=1;return $KEY_9;}";
        assert!(cleanup_program_thread(shadowed, &mut facts)
            .unwrap_err()
            .contains("declares $KEY_9"));
        let missing = "function $THREAD_SELF(){function $THREAD_1_2() {}}";
        assert!(cleanup_program_thread(missing, &mut facts)
            .unwrap_err()
            .contains("exported slot $KEY_7"));
    }

    #[test]
    fn checked_child_captures_allow_inert_private_data_to_inline() {
        let source = "function $THREAD_SELF(){let $KEY_7=0;let $SSA_0=[];function $THREAD_1_2(){}$THREAD_1_2();$KEY_7=$SSA_0;return $KEY_7;}";
        let mut facts = ProgramFacts {
            threads: vec![crate::ProgramThread {
                name: "$THREAD_1_2".into(),
                descriptor: 0,
                pc: 2,
                state: 1,
                cells: vec![7],
            }],
            ..ProgramFacts::default()
        };
        let cleaned = cleanup_program_thread(source, &mut facts).unwrap();
        assert!(!cleaned.code.contains("$SSA_0"), "{}", cleaned.code);
        assert!(cleaned.code.contains("let $KEY_7"), "{}", cleaned.code);
        assert!(check(&ProgramFacts::default(), &["$SSA_0".into()], &[]).is_err());
        assert!(check(&ProgramFacts::default(), &["$CARRY_0".into()], &[]).is_err());
    }
}
