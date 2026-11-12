use super::*;

pub fn parallel_map<T: Sync, R: Send>(
    items: &[T],
    threads: usize,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<Option<R>> {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new(items.iter().map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads.clamp(1, items.len().max(1)) {
            let _ = std::thread::Builder::new()
                .stack_size(SEED_STACK_BYTES)
                .spawn_scoped(scope, || loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    let result = f(item);
                    if let Ok(mut results) = results.lock() {
                        results[index] = Some(result);
                    }
                });
        }
    });
    results
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Walk `roots` and every function the walks create, each once: a function
/// starts as soon as the walk that creates it finishes, on whichever of
/// `jobs` threads is free, until no walk runs and none waits. One job walks
/// them in the order of a queue; `limit` is the most walked.
///
/// A walk may announce functions sooner, even ones its own finished walk
/// does not create (an attempt it started over from). A walk does not depend
/// on what runs when, so only the schedule changes: what stands is what is
/// reachable from `entry` through the functions finished walks create.
/// How a walk tells [`walk_reachable`] the functions it creates.
pub type Announce = Arc<dyn Fn(&[Function]) + Send + Sync>;

pub fn walk_reachable<R: Send + 'static>(
    roots: &[Function],
    jobs: usize,
    limit: Option<usize>,
    walk: impl Fn(Function, &Announce) -> Result<(R, Vec<Function>), String> + Sync,
) -> Result<BTreeMap<Function, R>, String> {
    use std::{
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{Condvar, Mutex},
    };
    struct Work<R> {
        waiting: VecDeque<Function>,
        seen: BTreeSet<Function>,
        started: usize,
        running: usize,
        done: BTreeMap<Function, (R, Vec<Function>)>,
        failed: BTreeMap<Function, String>,
    }
    let shared = Arc::new((
        Mutex::new(Work {
            waiting: roots.iter().copied().collect(),
            seen: roots.iter().copied().collect(),
            started: 0,
            running: 0,
            done: BTreeMap::new(),
            failed: BTreeMap::new(),
        }),
        Condvar::new(),
    ));
    let (work, changed) = (&shared.0, &shared.1);
    // A walk may tell the functions it creates before it is done, so other
    // workers start them while it goes on.
    let announce: Announce = {
        let shared = shared.clone();
        Arc::new(move |created: &[Function]| {
            let (work, changed) = (&shared.0, &shared.1);
            let Ok(mut state) = work.lock() else { return };
            for &created in created {
                if state.seen.insert(created) {
                    state.waiting.push_back(created);
                }
            }
            changed.notify_all();
        })
    };
    let worker = || loop {
        let function = {
            let Ok(mut state) = work.lock() else { return };
            loop {
                if limit.is_some_and(|limit| state.started >= limit) {
                    state.waiting.clear();
                }
                if let Some(function) = state.waiting.pop_front() {
                    state.started += 1;
                    state.running += 1;
                    break function;
                }
                if state.running == 0 {
                    changed.notify_all();
                    return;
                }
                let Ok(next) = changed.wait(state) else {
                    return;
                };
                state = next;
            }
        };
        let walked = catch_unwind(AssertUnwindSafe(|| walk(function, &announce)))
            .unwrap_or_else(|_| Err(format!("{}: the walk panicked", function.key())));
        let Ok(mut state) = work.lock() else { return };
        state.running -= 1;
        match walked {
            Ok((result, created)) => {
                for &created in &created {
                    if state.seen.insert(created) {
                        state.waiting.push_back(created);
                    }
                }
                state.done.insert(function, (result, created));
            }
            // A failure counts only for a function that stands.
            Err(error) => {
                state.failed.insert(function, error);
            }
        }
        changed.notify_all();
    };
    if jobs <= 1 {
        worker();
    } else {
        std::thread::scope(|scope| {
            for _ in 0..jobs {
                let _ = std::thread::Builder::new()
                    .stack_size(SEED_STACK_BYTES)
                    .spawn_scoped(scope, worker);
            }
        });
    }
    drop(announce);
    let work = Arc::try_unwrap(shared)
        .map_err(|_| "a walk's hook outlived it".to_string())?
        .0
        .into_inner()
        .map_err(|_| "a walk's state was poisoned".to_string())?;
    let mut reachable: BTreeSet<Function> = roots.iter().copied().collect();
    let mut pending = roots.to_vec();
    while let Some(function) = pending.pop() {
        for &created in work
            .done
            .get(&function)
            .map_or(&[][..], |(_, created)| created)
        {
            if reachable.insert(created) {
                pending.push(created);
            }
        }
    }
    if let Some(error) = work
        .failed
        .into_iter()
        .find_map(|(function, error)| reachable.contains(&function).then_some(error))
    {
        return Err(error);
    }
    Ok(work
        .done
        .into_iter()
        .filter(|(function, _)| reachable.contains(function))
        .map(|(function, (result, _))| (function, result))
        .collect())
}

/// `code` with the function declarations of its outermost body moved to its
/// end, in name order: a declaration is hoisted, so where the walk first
/// traced the subroutine does not matter.
pub fn functions_last(code: &str) -> String {
    let mut rest = Vec::new();
    let mut functions: Vec<Vec<&str>> = Vec::new();
    let mut open: Option<Vec<&str>> = None;
    for line in code.lines() {
        match &mut open {
            Some(function) => {
                function.push(line);
                if line == "\t}" {
                    functions.extend(open.take());
                }
            }
            None if line.starts_with("\tfunction ") && line.ends_with("{}") => {
                functions.push(vec![line]);
            }
            None if line.starts_with("\tfunction ") => open = Some(vec![line]),
            None => rest.push(line),
        }
    }
    functions.extend(open);
    functions.sort();
    let mut out = rest.join("\n");
    for function in functions {
        out.push('\n');
        out.push_str(&function.join("\n"));
    }
    out
}

/// `code` with its generated names (`$SSA_12`, `$OBJECT_3`, …) numbered in
/// the order they first appear: two walks that bind the same values in
/// another order emit the same text.
pub fn renumbered(code: &str) -> String {
    let mut names: BTreeMap<&str, usize> = BTreeMap::new();
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let prefix = tail[1..]
            .find(|c: char| !(c.is_ascii_uppercase() || c == '_'))
            .map_or(tail.len(), |end| end + 1);
        let digits = tail[prefix..]
            .find(|c: char| !c.is_ascii_digit())
            .map_or(tail.len(), |end| end + prefix);
        if prefix > 1 && tail[..prefix].ends_with('_') && digits > prefix {
            let name = &tail[..digits];
            let next = names.len();
            let number = *names.entry(name).or_insert(next);
            out.push_str(&tail[..prefix]);
            out.push_str(&number.to_string());
            rest = &tail[digits..];
        } else {
            out.push('$');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// `code` with its loop labels (`vm_loop_3`) numbered in the order they
/// first appear.
pub fn loops_renumbered(code: &str) -> String {
    const LABEL: &str = "vm_loop_";
    let mut labels: BTreeMap<&str, usize> = BTreeMap::new();
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find(LABEL) {
        let tail = &rest[at..];
        let end = tail[LABEL.len()..]
            .find(|c: char| !c.is_ascii_digit())
            .map_or(tail.len(), |end| end + LABEL.len());
        out.push_str(&rest[..at]);
        let next = labels.len();
        let number = *labels.entry(&tail[..end]).or_insert(next);
        out.push_str(LABEL);
        out.push_str(&number.to_string());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// Every value a test can have, worked out from the function's code alone:
/// constants, operators on them, and variables through every value any of
/// their assignments gives. A test all of whose values have one truth value
/// can only go one way; one with values of both can go either.
pub struct PossibleValues<'a> {
    pub arena: &'a crate::TraceArena,
    pub assigned: BTreeMap<&'a str, Vec<crate::TraceNodeId>>,
    pub known: BTreeMap<crate::TraceNodeId, Option<Vec<SymbolicValue>>>,
}

/// The most values worked out for one node: past that, unknown.
pub const MAX_POSSIBLE: usize = 64;

impl<'a> PossibleValues<'a> {
    pub fn new(arena: &'a crate::TraceArena) -> Self {
        let mut assigned: BTreeMap<&str, Vec<crate::TraceNodeId>> = BTreeMap::new();
        for statement in &arena.statements {
            if let TraceStatementKind::Let { name, value }
            | TraceStatementKind::Assign { name, value } = &statement.kind
            {
                assigned.entry(name.as_str()).or_default().push(*value);
            }
        }
        Self {
            arena,
            assigned,
            known: BTreeMap::new(),
        }
    }

    /// What each variable the test reads is assigned, following copies a
    /// few deep, as `name = value` lines: what a decision on the test rests
    /// on.
    pub fn definitions(
        &self,
        node: crate::TraceNodeId,
        describer: &crate::tracing::symbolic::TraceDescriber,
    ) -> Vec<String> {
        const MOST: usize = 12;
        let mut lines = Vec::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![(node, 0)];
        while let Some((node, depth)) = pending.pop() {
            if lines.len() >= MOST || depth > 4 || !seen.insert(node) {
                continue;
            }
            match self.arena.nodes.get(node as usize).map(|node| &node.kind) {
                Some(TraceNodeKind::Binary { left, right, .. }) => {
                    pending.push((*right, depth));
                    pending.push((*left, depth));
                }
                Some(TraceNodeKind::Unary { value, .. }) => pending.push((*value, depth)),
                Some(TraceNodeKind::Identifier { name }) => {
                    for value in self.assigned.get(name.as_str()).into_iter().flatten() {
                        let mut text = describer.describe(*value);
                        if text.len() > 600 {
                            let end = (0..=600)
                                .rev()
                                .find(|&end| text.is_char_boundary(end))
                                .unwrap_or(0);
                            text.truncate(end);
                            text.push('…');
                        }
                        lines.push(format!("{name} = {text}"));
                        pending.push((*value, depth + 1));
                    }
                }
                _ => {}
            }
        }
        lines
    }

    /// `"truthy"` or `"falsy"` when every value the test can have is,
    /// `"either"` when it can be both, `"unknown"` when they are not known.
    pub fn truth(&mut self, node: crate::TraceNodeId) -> &'static str {
        let Some(values) = self.of(node) else {
            return "unknown";
        };
        let truth: Option<BTreeSet<bool>> = values
            .iter()
            .map(|value| {
                match crate::tracing::symbolic::fold_unary(
                    crate::operations::templates::PrimitiveOperator::LogicalNot,
                    Some(value),
                ) {
                    Some(SymbolicValue::Boolean(falsy)) => Some(!falsy),
                    _ => None,
                }
            })
            .collect();
        match truth
            .as_ref()
            .map(|truth| truth.iter().copied().collect::<Vec<_>>())
            .as_deref()
        {
            Some([true]) => "truthy",
            Some([false]) => "falsy",
            Some([_, _]) => "either",
            _ => "unknown",
        }
    }

    pub fn of(&mut self, node: crate::TraceNodeId) -> Option<Vec<SymbolicValue>> {
        use crate::tracing::symbolic::{fold_binary, fold_unary, parse_number_text, same_fact};
        let mut pending = vec![(node, false)];
        let mut open = BTreeSet::new();
        while let Some((node, inputs_done)) = pending.pop() {
            if self.known.contains_key(&node) {
                continue;
            }
            let Some(kind) = self.arena.nodes.get(node as usize).map(|node| &node.kind) else {
                self.known.insert(node, None);
                continue;
            };
            let constant = match kind {
                TraceNodeKind::Undefined => Some(SymbolicValue::Undefined),
                TraceNodeKind::Null => Some(SymbolicValue::Null),
                TraceNodeKind::Boolean(value) => Some(SymbolicValue::Boolean(*value)),
                TraceNodeKind::Number(text) => parse_number_text(text).map(SymbolicValue::Number),
                TraceNodeKind::String(text) => Some(SymbolicValue::String(text.clone())),
                _ => None,
            };
            if let Some(constant) = constant {
                self.known.insert(node, Some(vec![constant]));
                continue;
            }
            let inputs: Vec<crate::TraceNodeId> = match kind {
                TraceNodeKind::Binary { left, right, .. } => vec![*left, *right],
                TraceNodeKind::Unary { value, .. } => vec![*value],
                TraceNodeKind::Identifier { name } => match self.assigned.get(name.as_str()) {
                    Some(values) => values.clone(),
                    None => {
                        self.known.insert(node, None);
                        continue;
                    }
                },
                _ => {
                    self.known.insert(node, None);
                    continue;
                }
            };
            if !inputs_done {
                open.insert(node);
                pending.push((node, true));
                for input in inputs {
                    if open.contains(&input) {
                        // A variable computed from itself: a loop's.
                        self.known.insert(node, None);
                    } else if !self.known.contains_key(&input) {
                        pending.push((input, false));
                    }
                }
                continue;
            }
            open.remove(&node);
            if self.known.contains_key(&node) {
                continue;
            }
            let values = |known: &BTreeMap<_, Option<Vec<SymbolicValue>>>,
                          input: crate::TraceNodeId| {
                known.get(&input).cloned().flatten()
            };
            let mut result: Option<Vec<SymbolicValue>> = match kind {
                TraceNodeKind::Binary {
                    operator,
                    left,
                    right,
                } => match (values(&self.known, *left), values(&self.known, *right)) {
                    (Some(lefts), Some(rights))
                        if lefts.len() * rights.len() <= MAX_POSSIBLE * MAX_POSSIBLE =>
                    {
                        let same = left == right;
                        let pairs: Vec<(&SymbolicValue, &SymbolicValue)> = if same {
                            lefts.iter().zip(&lefts).collect()
                        } else {
                            lefts
                                .iter()
                                .flat_map(|l| rights.iter().map(move |r| (l, r)))
                                .collect()
                        };
                        pairs
                            .into_iter()
                            .map(|(l, r)| fold_binary(*operator, Some(l), Some(r)))
                            .collect()
                    }
                    _ => None,
                },
                TraceNodeKind::Unary { operator, value } => {
                    values(&self.known, *value).and_then(|values| {
                        values
                            .iter()
                            .map(|value| fold_unary(*operator, Some(value)))
                            .collect()
                    })
                }
                _ => inputs
                    .iter()
                    .map(|input| values(&self.known, *input))
                    .collect::<Option<Vec<_>>>()
                    .map(|all| all.into_iter().flatten().collect()),
            };
            if let Some(found) = &mut result {
                let mut distinct: Vec<SymbolicValue> = Vec::new();
                for value in found.drain(..) {
                    if !distinct.iter().any(|known| same_fact(known, &value)) {
                        distinct.push(value);
                    }
                }
                *found = distinct;
            }
            let result = result.filter(|values| values.len() <= MAX_POSSIBLE);
            self.known.insert(node, result);
        }
        self.known.get(&node).cloned().flatten()
    }
}

/// VMInstructions a function's walk ran for each one it kept.
pub fn overstep(print: &Value) -> Option<f64> {
    let kept = print["steps"].as_u64().filter(|&kept| kept > 0)?;
    Some(print["handlers"].as_u64()? as f64 / kept as f64)
}

/// The first words of a stop, for grouping.
pub fn stop_kind(stop: &str) -> String {
    stop.split([':', '('])
        .next()
        .unwrap_or(stop)
        .trim()
        .to_string()
}

pub fn text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string)
}

/// One function to walk: its descriptor (`None` for the entry thread) and
/// entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Function {
    pub descriptor: Option<usize>,
    pub pc: usize,
    pub state: usize,
}

impl Function {
    pub fn key(self) -> String {
        match self.descriptor {
            Some(descriptor) => format!("d{descriptor}@{}/{}", self.pc, self.state),
            None => format!("entry@{}/{}", self.pc, self.state),
        }
    }
}

/// Walk every function of one seed, by key.
pub fn walk_seed(seed: &Path, args: &CorpusOptions) -> Result<BTreeMap<String, Value>, String> {
    let policy = legacy_cache_policy(args.report_dir.as_deref());
    walk_seed_with_cache(seed, args, &policy)
}

/// Walk a seed with an explicit kernel cache policy. `args.report_dir` is
/// ignored; all other corpus walk options retain their existing meaning.
pub fn walk_seed_with_cache(
    seed: &Path,
    args: &CorpusOptions,
    policy: &KernelCachePolicy,
) -> Result<BTreeMap<String, Value>, String> {
    let source = fs::read_to_string(seed)
        .map_err(|error| format!("cannot read {}: {error}", seed.display()))?;
    let artifact = load_artifact_with_policy(seed, &source, policy)?;
    let prepared = Arc::new(
        artifact
            .prepare_with_limits(PrefixLimits {
                max_dispatch_steps: args.steps,
                outline_subroutines: true,
                self_check: false,
                audit_fakes: args.audit,
                ..PrefixLimits::default()
            })
            .map_err(|error| {
                format!(
                    "{}: not a supported kernel artifact: {error}",
                    seed.display()
                )
            })?,
    );
    let inline = if args.self_check {
        Some(
            artifact
                .prepare_with_limits(PrefixLimits {
                    // Every call traced inline takes more steps than the
                    // walk it checks.
                    max_dispatch_steps: args.steps.saturating_mul(4),
                    outline_subroutines: false,
                    self_check: false,
                    ..PrefixLimits::default()
                })
                .map_err(|error| {
                    format!("{}: inline self-check preparation: {error}", seed.display())
                })?,
        )
    } else {
        None
    };
    // The same walk, choosing the other side first at every fork.
    let reversed = if args.self_check {
        Some(
            artifact
                .prepare_with_limits(PrefixLimits {
                    max_dispatch_steps: args.steps,
                    outline_subroutines: true,
                    self_check: false,
                    reverse_sides: true,
                    ..PrefixLimits::default()
                })
                .map_err(|error| {
                    format!(
                        "{}: reversed self-check preparation: {error}",
                        seed.display()
                    )
                })?,
        )
    } else {
        None
    };
    let entry = artifact.entry_facts().ok_or_else(|| {
        format!(
            "{}: kernel artifact has no actual entry facts; refusing a fabricated/default entry",
            seed.display()
        )
    })?;
    let descriptors = artifact.descriptor_records();
    let captures = crate::trace::thread::thread_captures(&descriptors);
    let initial_slots = artifact.entry_initial_slots();
    // What a call of a VM function returns: from the callee's walk, walked
    // first (see `super::return_shapes`).
    let asked = Arc::new(crate::trace::return_shapes::Round::new(
        prepared.clone(),
        descriptors.into(),
        initial_slots.into(),
        captures.clone(),
        args.steps,
        Arc::default(),
        Arc::default(),
        &BTreeMap::new(),
    ));
    let (returns, cell_types) = (asked.asking(), asked.asking_cell_types());
    let entry = Function {
        descriptor: None,
        pc: entry.pc as usize,
        state: entry.state as usize,
    };
    let roots = roots(entry, &descriptors);
    // Every holder of a cell must be walked for its content to be known.
    let (contents, walked) = if args.limit.is_none() {
        converge_cells(
            &prepared,
            &roots,
            &descriptors,
            &initial_slots,
            &captures,
            args.steps,
            args.jobs,
            &Emitting {
                artifact: &artifact,
                cleanup: args.cleanup,
            },
            Prior::new(),
            Some(returns.clone()),
        )?
    } else {
        (
            crate::trace::cell_contents::CellContents::new(),
            Walked::new(),
        )
    };

    // One function: its fingerprint and the functions it creates.
    let walk = |&function: &Function,
                announce: &Announce|
     -> Result<(Value, Vec<Function>), String> {
        let started = Instant::now();
        let setup = setup(function, &descriptors, &initial_slots, args.steps)
            .and_then(|setup| startable(&prepared, function).map(|()| setup))
            .map(|setup| Setup {
                captures: captures.clone(),
                cell_contents: contents.get(&function).cloned().unwrap_or_default(),
                // The cells its round walks found: the walk starts with them, as
                // the round walk it may reuse did, so a walk again is the same.
                known_cells: walked
                    .get(&function)
                    .map(|(_, _, cells, _)| cells.clone())
                    .unwrap_or_default(),
                returns: Some(returns.clone()),
                cell_types: Some(cell_types.clone()),
                ..setup
            });
        let emitting = Emitting {
            artifact: &artifact,
            cleanup: args.cleanup,
        };
        let (mut print, created, code) = match &setup {
            Ok(setup) => {
                // The walk the rounds made with these contents, or with
                // contents the same where it read them: walks are
                // deterministic, so it is this walk.
                let reused = walked
                    .get(&function)
                    .filter(|(used, _, _, reads)| {
                        *used == setup.cell_contents || reads.same(used, &setup.cell_contents)
                    })
                    .and_then(|(_, walk, _, _)| walk.as_ref());
                match reused {
                    Some(walk) => (walk.print.clone(), walk.created.clone(), walk.code.clone()),
                    None => {
                        // With a limit, which functions are walked is that
                        // of one queue: nothing is started early.
                        let early = args.limit.is_none().then_some(announce);
                        let result = invoke_announcing(&prepared, function, setup, early)?;
                        let created = created_functions(&result);
                        announce(&created);
                        let (mut print, code) = fingerprint(&result, &setup.parameters, &emitting);
                        add_digest(&mut print, &code);
                        (print, created, code)
                    }
                }
            }
            Err(error) => (
                json!({ "complete": false, "stop": error }),
                Vec::new(),
                None,
            ),
        };
        if let (Some(inline), Ok(setup)) = (&inline, &setup) {
            let mut differences = Vec::new();
            // The same walk again must emit the same code.
            let again = invoke(&prepared, function, setup)?;
            let quick = Emitting {
                cleanup: false,
                ..emitting
            };
            let (_, again) = fingerprint(&again, &setup.parameters, &quick);
            if again != code {
                differences.push("a second walk emits different code".to_string());
            }
            let result = invoke(inline, function, setup)?;
            let (other, _) = fingerprint(&result, &setup.parameters, &quick);
            differences.extend(compare_inline(&print, &other));
            print["inline_steps"] = other["steps"].clone();
            print["inline_handlers"] = other["handlers"].clone();
            // Not yet a self-check difference: the walk's meeting points
            // still depend on the order (see docs/tracing/corpus-comparison.md).
            if let Some(reversed) = &reversed {
                let result = invoke(reversed, function, setup)?;
                let (_, other) = fingerprint(&result, &setup.parameters, &quick);
                let canonical = |code: &str| loops_renumbered(&renumbered(&functions_last(code)));
                if other.as_deref().map(canonical) != code.as_deref().map(canonical) {
                    print["schedule"] =
                        json!(["the walk stepping the other side first emits different code"]);
                }
            }
            if let Some(list) = print["self_check"].as_array_mut() {
                list.extend(differences.into_iter().map(Value::String));
            }
        }
        print["millis"] = json!(started.elapsed().as_millis() as u64);
        Ok((print, created))
    };
    // With a limit, one at a time: which functions come first is that of
    // one queue.
    let jobs = if args.limit.is_some() { 1 } else { args.jobs };
    let finished = walk_reachable(&roots, jobs, args.limit, |function, announce| {
        walk(&function, announce)
    })?;
    let prints = finished
        .into_iter()
        .map(|(function, print)| (function.key(), print))
        .collect();
    // The seed's walks and tables are freed on another thread: nothing
    // waits for that.
    let garbage = (walked, contents, prepared, inline, reversed, artifact);
    std::thread::spawn(move || drop(garbage));
    Ok(prints)
}

/// The functions a seed starts from: its entry, and each VM Worker it may
/// start, as the descriptor whose initial slots the Worker's bootstrap
/// passes (`vmEntry(self, pc, state, initialSlots)`).
pub fn roots(entry: Function, descriptors: &[DescriptorRecord]) -> Vec<Function> {
    std::iter::once(entry)
        .chain(
            descriptors
                .iter()
                .enumerate()
                .filter_map(|(index, record)| {
                    let start = record.entry?;
                    Some(Function {
                        descriptor: Some(index),
                        pc: start.pc,
                        state: start.state,
                    })
                }),
        )
        .collect()
}

/// The slots `function`'s descriptor captures and does not initialize: the
/// cells it shares with the function that created it.
pub fn captured_slots(function: Function, descriptors: &[DescriptorRecord]) -> Vec<u32> {
    function
        .descriptor
        .and_then(|index| descriptors.get(index))
        .and_then(|record| record.descriptor.as_ref())
        .map(|descriptor| {
            descriptor
                .captured_slots
                .iter()
                .filter(|slot| !descriptor.initial_slots.contains(slot))
                .copied()
                .collect()
        })
        .unwrap_or_default()
}

/// Walk every function reachable from `entry` in rounds, each with the cell
/// contents the last round's walks give (see [`super::cell_contents`]),
/// until they no longer change; a function whose contents did not change is
/// not walked again. Contents guessed from walks that stopped stand only if
/// they hold once every holder completed. When they do not settle, no
/// function gets any.
pub fn converge_cells(
    prepared: &PrefixPreparation,
    roots: &[Function],
    descriptors: &[DescriptorRecord],
    initial_slots: &[u32],
    captures: &Arc<[Vec<u32>]>,
    steps: usize,
    jobs: usize,
    emitting: &Emitting<'_>,
    walked_before: Prior,
    returns: Option<crate::tracing::symbolic::ReturnsHook>,
) -> Result<(crate::trace::cell_contents::CellContents, Walked), String> {
    use crate::trace::cell_contents::{CellContents, CellGraph, Summary, MAX_ROUNDS};
    let mut contents = CellContents::new();
    type Last = (
        BTreeMap<u32, crate::CellContent>,
        Summary,
        Option<Arc<Fingerprinted>>,
        BTreeSet<u32>,
        crate::CellReads,
    );
    // Walks made before, with the contents they were given: the first
    // round needs them again.
    let mut walked: BTreeMap<Function, Last> = walked_before
        .into_iter()
        .map(|(function, (summary, cells, reads, used))| {
            (function, (used, summary, None, cells, reads))
        })
        .collect();
    let mut graph: Option<CellGraph> = None;
    // The functions found to need contents so far: one that completes with
    // them still needs them.
    let mut needing: BTreeSet<Function> = BTreeSet::new();
    // The walks with the last strict contents (only those of holders that
    // completed; none at first), with how many functions completed: the
    // contents given when the rounds end before they settle, or when a
    // guess completes fewer functions.
    let mut strict_walk: Option<(CellContents, Walked, usize)> = None;
    let mut walking_strict = true;
    for _ in 0..MAX_ROUNDS {
        // A function is not walked again when its walk would trace the
        // same: its contents did not change where it read them. Its walk
        // stands for these contents too.
        let round = walk_reachable(roots, jobs, None, |function, announce| {
            let given = contents.get(&function).cloned().unwrap_or_default();
            if let Some((used, summary, report, cells, reads)) = walked.get(&function) {
                if *used == given || reads.same(used, &given) {
                    let creates = summary.creates.clone();
                    let last = (
                        given,
                        summary.clone(),
                        report.clone(),
                        cells.clone(),
                        reads.clone(),
                    );
                    return Ok((last, creates));
                }
            }
            // Walked again with other contents: with the cells it found.
            let known = walked
                .get(&function)
                .map(|(_, _, _, cells, _)| cells.clone())
                .unwrap_or_default();
            let (summary, report, cells, reads) =
                match setup(function, descriptors, initial_slots, steps)
                    .and_then(|setup| startable(prepared, function).map(|()| setup))
                {
                    Ok(setup) => {
                        let setup = Setup {
                            captures: captures.clone(),
                            cell_contents: given.clone(),
                            known_cells: known,
                            returns: returns.clone(),
                            ..setup
                        };
                        let result = invoke_announcing(prepared, function, &setup, Some(announce))?;
                        let created = created_functions(&result);
                        announce(&created);
                        let summary = Summary::of(&result, created.clone());
                        // Kept for the final pass: a later round often finds the
                        // walk stands for its contents too.
                        let (mut print, code) = fingerprint(&result, &setup.parameters, emitting);
                        add_digest(&mut print, &code);
                        let kept = Some(Arc::new(Fingerprinted {
                            print,
                            code,
                            created,
                        }));
                        (
                            summary,
                            kept,
                            result.cells.clone(),
                            result.cell_reads.clone(),
                        )
                    }
                    Err(_) => (Summary::failed(), None, BTreeSet::new(), Default::default()),
                };
            let creates = summary.creates.clone();
            Ok(((given, summary, report, cells, reads), creates))
        })?;
        let mut summaries = BTreeMap::new();
        for (function, (given, summary, report, cells, reads)) in round {
            summaries.insert(function, summary.clone());
            walked.insert(function, (given, summary, report, cells, reads));
        }
        let reports = |walked: &BTreeMap<Function, Last>| -> Walked {
            walked
                .iter()
                .map(|(function, (used, _, report, cells, reads))| {
                    (
                        *function,
                        (used.clone(), report.clone(), cells.clone(), reads.clone()),
                    )
                })
                .collect()
        };
        let completed = summaries
            .values()
            .filter(|summary| summary.complete)
            .count();
        if walking_strict {
            strict_walk = Some((contents.clone(), reports(&walked), completed));
        } else if let Some((_, _, strict)) = &strict_walk {
            if completed < *strict {
                let (contents, reports, _) = strict_walk.take().expect("checked");
                return Ok((contents, reports));
            }
        }
        walking_strict = false;
        let captured = |function| captured_slots(function, descriptors);
        // The cells and their holders, again only if what creates what changed.
        if !graph.as_ref().is_some_and(|graph| graph.is_of(&summaries)) {
            graph = Some(CellGraph::of(&summaries, captured));
        }
        let graph = graph.as_ref().expect("built above");
        // Only the functions that need contents get them: the others keep
        // the walks they completed with their cells unknown.
        let asked: BTreeMap<Function, BTreeSet<u32>> = walked
            .iter()
            .map(|(&function, (.., reads))| {
                (function, reads.sets.union(&reads.data).copied().collect())
            })
            .collect();
        needing.extend(graph.needing(&summaries, |function| asked.get(&function)));
        let only_needing = |contents: CellContents| -> CellContents {
            contents
                .into_iter()
                .filter(|(function, _)| needing.contains(function))
                .collect()
        };
        let guessed = only_needing(graph.contents(&summaries, false));
        if guessed == contents {
            // Settled: only contents every holder completed for stand.
            let strict = only_needing(graph.contents(&summaries, true));
            if strict == contents {
                return Ok((contents, reports(&walked)));
            }
            // A holder that stopped keeps the guess from standing: the walks
            // with the strict contents stand if the rounds end before.
            if strict_walk
                .as_ref()
                .is_some_and(|(last, _, _)| *last == strict)
            {
                let (contents, reports, _) = strict_walk.take().expect("checked");
                return Ok((contents, reports));
            }
            contents = strict;
            walking_strict = true;
        } else {
            contents = guessed;
        }
    }
    Ok(
        strict_walk.map_or_else(Default::default, |(contents, reports, _)| {
            (contents, reports)
        }),
    )
}

/// Walks made before, for [`converge_cells`] to start from: what each told
/// of cells, the cells it found, what it read of them and the contents it
/// was given.
pub type Prior = BTreeMap<
    Function,
    (
        crate::trace::cell_contents::Summary,
        BTreeSet<u32>,
        crate::CellReads,
        BTreeMap<u32, crate::CellContent>,
    ),
>;

/// Each function's last walk in [`converge_cells`], with the contents it was
/// walked with: a walk with those contents again would give the same.
pub type Walked = BTreeMap<
    Function,
    (
        BTreeMap<u32, crate::CellContent>,
        Option<Arc<Fingerprinted>>,
        BTreeSet<u32>,
        crate::CellReads,
    ),
>;

/// What the function emits, for the branch list to compare runs by: once,
/// with the walk's fingerprint.
pub fn add_digest(print: &mut Value, code: &Option<String>) {
    if let Some(code) = code {
        let canonical = loops_renumbered(&renumbered(&functions_last(code)));
        print["digest"] = json!(crate::util::fnv1a64_hex(canonical.as_bytes()));
    }
}

/// What the corpus keeps of a walk: its fingerprint, its code and the
/// functions it creates.
pub struct Fingerprinted {
    pub print: Value,
    pub code: Option<String>,
    pub created: Vec<Function>,
}

/// Compile or load one source-free kernel artifact for a seed. A legacy report
/// is never deserialized or converted by this path.
pub fn load_artifact(
    seed: &Path,
    source: &str,
    dir: Option<&Path>,
) -> Result<KernelArtifact, String> {
    load_artifact_with_policy(seed, source, &legacy_cache_policy(dir))
}

pub(crate) fn legacy_cache_policy(dir: Option<&Path>) -> KernelCachePolicy {
    match dir {
        Some(dir) => KernelCachePolicy::Directory(dir.to_owned()),
        None => KernelCachePolicy::Environment,
    }
}

/// Load or compile a kernel with per-call caching. Disabled performs no cache
/// reads or writes and does not consult environment cache configuration.
pub fn load_artifact_with_policy(
    seed: &Path,
    source: &str,
    policy: &KernelCachePolicy,
) -> Result<KernelArtifact, String> {
    let dir = match policy {
        KernelCachePolicy::Directory(dir) => Some(dir.as_path()),
        KernelCachePolicy::Disabled | KernelCachePolicy::Environment => None,
    };
    let path = dir.map(|dir| {
        let stem = seed
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        dir.join(format!("{stem}.kernel.json"))
    });
    if let Some(dir) = dir {
        let stem = seed
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let legacy = dir.join(format!("{stem}.report.json"));
        if legacy.exists() {
            return Err(format!(
                "{}: legacy PipelineReport cache is not a runtime kernel artifact; regenerate from raw source with `shape deob SOURCE --report ARTIFACT`",
                legacy.display()
            ));
        }
    }
    if let Some(path) = path.as_ref().filter(|path| path.exists()) {
        let artifact = crate::trace::read_kernel_artifact(path)?;
        if artifact.source_hash() != fnv1a64_hex(source.as_bytes()) {
            return Err(format!(
                "{} is the artifact of another seed",
                path.display()
            ));
        }
        return Ok(artifact);
    }
    let cached = matches!(policy, KernelCachePolicy::Environment)
        .then(|| default_cache_path(source))
        .flatten();
    if let Some(cached) = cached.as_ref().filter(|path| path.exists()) {
        // A stale or unreadable entry is rebuilt below.
        if let Ok(artifact) = crate::trace::read_kernel_artifact(cached) {
            if artifact.source_hash() == fnv1a64_hex(source.as_bytes()) {
                return Ok(artifact);
            }
        }
    }
    let report = deobfuscate_with_report(source)
        .map_err(|error| format!("{}: deobfuscation failed: {error}", seed.display()))?;
    let artifact = KernelArtifact::compile_from_raw(source, &report, PrefixLimits::default())
        .map_err(|error| {
            format!(
                "{}: kernel artifact compilation failed: {error}",
                seed.display()
            )
        })?;
    if let Some(path) = path {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        }
        let json = artifact
            .to_json()
            .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
        let temporary = write_temporary(&path, &json)?;
        if let Err(error) = fs::hard_link(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("cannot publish {}: {error}", path.display()));
        }
        let _ = fs::remove_file(&temporary);
    } else if let Some(cached) = cached {
        // The default cache only saves time: a write that fails is skipped.
        let _ = write_cache(&cached, &artifact);
    }
    Ok(artifact)
}

/// Where a seed's kernel artifact is cached when no `--report-dir` is given:
/// `$SHAPE_KERNEL_CACHE` (`off` turns caching off), else
/// `$XDG_CACHE_HOME/shape/kernels`, else `~/.cache/shape/kernels`. The file
/// is named by a hash of the seed and of this executable (its size and
/// modification time), so rebuilding `shape` never reuses an artifact an
/// older build compiled.
fn default_cache_path(source: &str) -> Option<std::path::PathBuf> {
    let directory = match std::env::var_os("SHAPE_KERNEL_CACHE") {
        Some(value) if value == "off" => return None,
        Some(value) => std::path::PathBuf::from(value),
        None => match std::env::var_os("XDG_CACHE_HOME") {
            Some(cache) => std::path::PathBuf::from(cache).join("shape").join("kernels"),
            None => std::path::PathBuf::from(std::env::var_os("HOME")?).join(".cache").join("shape").join("kernels"),
        },
    };
    let executable = std::env::current_exe().ok()?.metadata().ok()?;
    let modified = executable
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let build = fnv1a64_hex(format!("{}:{modified}", executable.len()).as_bytes());
    Some(directory.join(format!("{}-{build}.kernel.json", fnv1a64_hex(source.as_bytes()))))
}

fn write_cache(path: &Path, artifact: &KernelArtifact) -> Result<(), String> {
    let dir = path.parent().ok_or("no cache directory")?;
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let json = artifact.to_json().map_err(|error| error.to_string())?;
    let temporary = write_temporary(path, &json)?;
    // Another run may publish the same entry first: either copy is the same.
    let published = fs::rename(&temporary, path).map_err(|error| error.to_string());
    let _ = fs::remove_file(&temporary);
    // Entries older builds made for this seed are never read again.
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    if let Some((seed, _)) = name.split_once('-') {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let other = entry.file_name();
            let other = other.to_string_lossy();
            if other != name && other.starts_with(&format!("{seed}-")) && other.ends_with(".kernel.json") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    published
}

/// Reserve a distinct temporary file for each concurrent writer. Readers only
/// see complete files published by hard-link (explicit) or rename (default).
fn write_temporary(path: &Path, json: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    loop {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("tmp-{}-{id}", std::process::id()));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("cannot create {}: {error}", temporary.display())),
        };
        if let Err(error) = file.write_all(json.as_bytes()) {
            drop(file);
            let _ = fs::remove_file(&temporary);
            return Err(format!("cannot write {}: {error}", temporary.display()));
        }
        return Ok(temporary);
    }
}

/// How to start one function: its slots, inputs and the names of its
/// parameters.
pub struct Setup {
    pub descriptor: ThreadDescriptor,
    pub inputs: ThreadInputs,
    pub arguments: Vec<SymbolicValue>,
    pub parameters: Vec<String>,
    pub captures: Arc<[Vec<u32>]>,
    /// The slots each thread descriptor initializes (see
    /// [`crate::trace::thread::thread_starts`]).
    pub(super) starts: Arc<[Vec<(u32, bool)>]>,
    /// What each of its shared cells can hold; see
    /// [`super::cell_contents`].
    pub cell_contents: BTreeMap<u32, crate::CellContent>,
    /// The cells an earlier walk of the function found.
    pub known_cells: BTreeSet<u32>,
    /// What a call of a VM function returns (see `super::return_shapes`).
    pub returns: Option<crate::tracing::symbolic::ReturnsHook>,
    /// What the closures it makes put in their cells (see
    /// `super::return_shapes`).
    pub cell_types: Option<crate::tracing::symbolic::CellTypesHook>,
    /// Branches the user decides.
    pub overrides: Arc<crate::BranchOverrides>,
}

/// The most trace nodes one walk builds, whatever the step budget: about five
/// times the largest function of the JavaScript corpus. A walk whose forks
/// never meet otherwise grows its trace (and memory) without bound before
/// any other limit stops it.
pub const MAX_WALK_NODES: usize = 1_000_000;

pub fn setup(
    function: Function,
    descriptors: &[DescriptorRecord],
    initial_slots: &[u32],
    steps: usize,
) -> Result<Setup, String> {
    let inputs = ThreadInputs::new().limits(TraceLimits {
        max_steps: steps.saturating_mul(32).max(10_000),
        max_nodes: steps.saturating_mul(64).clamp(50_000, MAX_WALK_NODES),
        ..TraceLimits::default()
    });
    let Some(index) = function.descriptor else {
        return Ok(Setup {
            descriptor: ThreadDescriptor::new().initial_slots(initial_slots.iter().copied()),
            inputs,
            arguments: Vec::new(),
            parameters: Vec::new(),
            captures: Arc::from([]),
            starts: crate::trace::thread::thread_starts(descriptors),
            cell_contents: BTreeMap::new(),
            known_cells: BTreeSet::new(),
            returns: None,
            cell_types: None,
            overrides: Arc::default(),
        });
    };
    let record = descriptors
        .get(index)
        .ok_or_else(|| format!("descriptor {index} is out of range"))?;
    let descriptor = match (&record.descriptor, &record.invalid) {
        (Some(descriptor), _) => descriptor.clone(),
        (None, Some(error)) => return Err(format!("descriptor {index} is invalid: {error}")),
        (None, None) => return Err(format!("descriptor {index} has no descriptor facts")),
    };
    let (inputs, arguments) = unknown_inputs(&descriptor, inputs);
    let parameters = arguments
        .iter()
        .filter_map(|argument| match argument {
            SymbolicValue::External { name } => Some(name.clone()),
            _ => None,
        })
        .chain(external_names(&descriptor))
        .collect();
    Ok(Setup {
        descriptor,
        inputs,
        arguments,
        parameters,
        captures: Arc::from([]),
        starts: crate::trace::thread::thread_starts(descriptors),
        cell_contents: BTreeMap::new(),
        known_cells: BTreeSet::new(),
        returns: None,
        cell_types: None,
        overrides: Arc::default(),
    })
}

pub fn invoke(
    prepared: &PrefixPreparation,
    function: Function,
    setup: &Setup,
) -> Result<PrefixTraceReport, String> {
    invoke_announcing(prepared, function, setup, None)
}

/// Whether `function`'s thread can start. A function the code creates at an
/// entry outside the image (an operand a patch on one path rewrote, say)
/// cannot: it is that function's stop, not the seed's.
pub fn startable(prepared: &PrefixPreparation, function: Function) -> Result<(), String> {
    prepared
        .validate_entry(DispatchStart {
            pc: function.pc,
            state: function.state,
        })
        .map_err(|error| {
            format!(
                "{}: thread startup rejected: {}",
                function.key(),
                crate::ThreadStartupError::from(error)
            )
        })
}

/// [`invoke`], telling `announce` each function the walk creates, in any
/// attempt, the moment it does.
pub fn invoke_announcing(
    prepared: &PrefixPreparation,
    function: Function,
    setup: &Setup,
    announce: Option<&Announce>,
) -> Result<PrefixTraceReport, String> {
    let thread = prepared
        .thread(
            DispatchStart {
                pc: function.pc,
                state: function.state,
            },
            setup.descriptor.clone(),
            setup.inputs.clone(),
        )
        .map_err(|error| format!("{}: thread startup rejected: {error}", function.key()))?
        .with_thread_captures(setup.captures.clone())
        .with_thread_starts(setup.starts.clone())
        .with_cell_contents(setup.cell_contents.clone())
        .with_known_cells(setup.known_cells.clone())
        .with_returns_hook(setup.returns.clone())
        .with_cell_types_hook(setup.cell_types.clone())
        .with_branch_overrides(setup.overrides.clone());
    let thread = match announce {
        Some(announce) => {
            let announce = announce.clone();
            thread.on_created(Arc::new(move |descriptor, pc, state| {
                announce(&[Function {
                    descriptor: Some(descriptor),
                    pc,
                    state,
                }]);
            }))
        }
        None => thread,
    };
    thread
        .invoke(&setup.arguments)
        .map_err(|error| format!("{}: thread invocation rejected: {error}", function.key()))
}

/// The VM functions `result`'s trace creates.
/// The functions a walk posts to the VM Worker (see
/// [`created_functions`]).
pub fn posted_functions(result: &PrefixTraceReport) -> Vec<Function> {
    let nodes = &result.trace.arena.nodes;
    created_functions(result)
        .into_iter()
        .filter(|function| {
            !nodes.iter().any(|node| {
                matches!(node.kind,
                TraceNodeKind::VmFunction { descriptor, byte_offset, dispatch_state, .. }
                    if Some(descriptor) == function.descriptor
                        && byte_offset == function.pc
                        && dispatch_state == function.state)
            })
        })
        .collect()
}

pub fn created_functions(result: &PrefixTraceReport) -> Vec<Function> {
    let nodes = &result.trace.arena.nodes;
    // Only what the code creates: a path the walk went back from creates
    // nothing.
    let live = result.trace.arena.live_nodes();
    let number = |node: &crate::TraceNodeId| match nodes.get(*node as usize).map(|node| &node.kind)
    {
        Some(TraceNodeKind::Number(raw)) => raw.parse::<usize>().ok(),
        _ => None,
    };
    nodes
        .iter()
        .filter(|node| live[node.id as usize])
        .filter_map(|node| match &node.kind {
            TraceNodeKind::VmFunction {
                descriptor,
                byte_offset,
                dispatch_state,
                ..
            } => Some(Function {
                descriptor: Some(*descriptor),
                pc: *byte_offset,
                state: *dispatch_state,
            }),
            // A function the page posts to the VM Worker runs there: the
            // program has it too (state, pc, argument, descriptor, callback).
            TraceNodeKind::Call {
                callee, arguments, ..
            } if matches!(nodes.get(*callee as usize).map(|node| &node.kind),
                Some(TraceNodeKind::Global { name }) if name == crate::operations::templates::POST_TO_WORKER) =>
            {
                Some(Function {
                    descriptor: Some(number(arguments.get(3)?)?),
                    pc: number(arguments.get(1)?)?,
                    state: number(arguments.first()?)?,
                })
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// What the emitted code is checked with: the seed and its report, for the
/// output cleanup passes, and whether to run them (slow: they analyse the
/// seed again for every function).
pub struct Emitting<'a> {
    pub artifact: &'a KernelArtifact,
    pub cleanup: bool,
}
